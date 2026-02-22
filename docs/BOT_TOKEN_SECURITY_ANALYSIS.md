# BOT_TOKEN 存储流程安全分析

> 日期: 2026-02-22 · 版本: v1.0
> 范围: 群组管理员通过 DApp 输入 Telegram API Key → TEE 存储的完整流转路径

---

## 1. 完整流转路径

```
┌────────┐    HTTPS/RA-TLS     ┌──────────┐    Shamir Share     ┌──────────┐
│  DApp  │ ──────────────────▶ │ Ceremony │ ──────────────────▶ │ TEE Node │
│ 浏览器  │  (用户输入 Token)    │ Enclave  │  (ECDH 加密分发)    │ (运行时)  │
└────────┘                     └──────────┘                     └──────────┘
   ①                              ②③④                           ⑤⑥⑦
```

| 阶段 | 环节 | Token 形态 |
|:----:|------|-----------|
| ① | 用户在 DApp 浏览器中输入 | **明文** (浏览器内存) |
| ② | 网络传输 (浏览器 → 服务器) | **TLS 加密** (RA-TLS 设计) |
| ③ | Ceremony Enclave 接收处理 | **明文** (SGX 内存, 短暂) |
| ④ | Shamir Split + 分发到各 TEE 节点 | **分片+ECDH 加密** |
| ⑤ | TEE 节点本地密封存储 | **AES-GCM 密封** (磁盘) |
| ⑥ | TEE 节点启动恢复 (K 个 Share 重构) | **明文** (SGX 内存) |
| ⑦ | 运行时使用 (API 调用) | **⚠️ 当前为明文 String** |

---

## 2. 已发现的泄露风险

### 🔴 严重风险 (3 个)

#### 风险 S1: 环境变量明文加载

**位置**: `grouprobot/src/config.rs:76-77`

```rust
let bot_token = std::env::var("BOT_TOKEN")
    .map_err(|_| BotError::Config("BOT_TOKEN is required".into()))?;
```

**问题**: BOT_TOKEN 通过环境变量以明文形式传入。

**泄露路径**:
- Linux: `cat /proc/<PID>/environ` 可读取所有环境变量
- Docker: `docker inspect <container>` 显示环境变量
- K8s: `kubectl describe pod` 显示 env 配置
- 运维日志: 部署脚本、CI/CD 管道可能记录环境变量
- 进程列表: 部分工具 (如 `ps eww`) 可显示环境变量

**严重程度**: 🔴 **即使在 TDX 内, Host 管理员仍可能通过容器编排工具看到环境变量配置**

---

#### 风险 S2: 运行时全生命周期明文 String

**位置**: `grouprobot/src/config.rs:36` + `main.rs:115,124`

```rust
// config.rs:36 — 作为 BotConfig 字段, 进程生命周期内持续存在
pub struct BotConfig {
    pub bot_token: String,   // ← 明文 String, 永不清零
    ...
}

// main.rs:115 — 克隆到 TelegramExecutor
TelegramExecutor::new(cfg.bot_token.clone(), ...)

// main.rs:124 — 克隆到 DiscordExecutor
DiscordExecutor::new(dc_cfg.bot_token.clone(), ...)
```

**问题**:
- `String` 是堆分配, Token 明文在 TDX 内存中存在多份副本
- Rust `String` 被 drop 时**不会 zeroize**, 堆内存可能残留
- `clone()` 每次创建新副本, 扩散到 `BotConfig`、`TelegramExecutor`、`DiscordExecutor` 共 3+ 份
- 如果 TDX 内存被 dump (如 TDX 侧信道攻击), Token 明文暴露

**严重程度**: 🔴 **Token 在整个进程生命周期内以明文存在于内存中, 完全绕过了 Ceremony/Shamir 的安全设计**

---

#### 风险 S3: Ceremony 设计与运行时实现脱节

**问题**: 设计文档中 Ceremony 流程的产出 (Shamir Shares) 并未被运行时代码使用。

| 设计意图 | 实际实现 |
|---------|---------|
| Ceremony 产出 K-of-N Shares → 节点启动时重构 Token | 直接从环境变量 `BOT_TOKEN` 读取 |
| Token 仅在 SGX Enclave 内存中, 外部不可见 | Token 在 TDX 内存中 (BotConfig.bot_token) |
| Token 使用后 zeroize | Token 从不清零, 进程生命周期持续 |
| API 调用时 Enclave 内拼接 URL | TDX 中直接 `format!("...bot{}/...", self.bot_token)` |

**严重程度**: 🔴 **安全仪式的核心价值 (Token 仅 Enclave 可见) 在运行时被完全架空**

---

### 🟡 中等风险 (3 个)

#### 风险 M1: SealedStorage 软件模式密钥可预测

**位置**: `grouprobot/src/tee/sealed_storage.rs:27-42`

```rust
fn derive_machine_key() -> [u8; 32] {
    let mut hasher = Sha256::new();
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        hasher.update(hostname.as_bytes());
    }
    hasher.update(b"grouprobot-seal-key-v1");
    if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
        hasher.update(machine_id.trim().as_bytes());
    }
    ...
}
```

**问题**: 密钥 = `SHA256(HOSTNAME + 固定盐 + machine-id)`, 这三个值对同机器上的攻击者完全已知, 密封形同虚设。

**注**: 文档标注 "Hardware 模式使用 SGX seal key", 但当前代码未实现条件分支。

---

#### 风险 M2: API URL 中拼接 Token

**位置**: `platform/telegram/executor.rs:20`

```rust
fn api_url(&self, method: &str) -> String {
    format!("https://api.telegram.org/bot{}/{}", self.bot_token, method)
}
```

**问题**:
- 每次 API 调用生成包含 Token 的 URL String
- HTTP 客户端库可能在 debug 模式记录完整 URL
- 如果 reqwest 启用 trace 日志, Token 会出现在日志中
- URL String 被 drop 后堆内存残留 Token

---

#### 风险 M3: 前端 DApp Token 输入安全

**位置**: 用户浏览器端

**问题**:
- 用户在 `<input>` 中输入 Token, 浏览器内存中存在明文
- 浏览器扩展 (如密码管理器、恶意插件) 可读取输入框内容
- 如果 DApp 使用 React 等框架, state 中可能保留 Token 副本
- 浏览器 DevTools Console 可直接读取
- 剪贴板历史 (如果用户 copy-paste)

---

### 🟢 低风险 (2 个)

#### 风险 L1: Ceremony Enclave 中 Token 未显式 zeroize

**位置**: `nexus-tee-bot/src/ceremony.rs:249-252`

```rust
// ── Step 7: Zeroize secrets ──
// 在真实 SGX 环境中使用 zeroize crate
// 这里 signing_key 和 secrets 在函数结束时 drop
debug!("明文 secrets 将在函数退出时释放");
```

**问题**: 依赖 Rust drop 语义而非显式 `zeroize()`, 编译器优化可能跳过清零操作。

---

#### 风险 L2: Shamir Share 分发期间的时间窗口

**问题**: 在 Ceremony 执行 Shamir Split 到所有 Share 分发完成之间, 完整 Token 在 Enclave 内存中存在。如果此时 Enclave 被中断或崩溃, Token 可能残留在 EPC 页中。

---

## 3. 攻击路径分析

### 3.1 最简单攻击路径 (当前实现)

```
攻击者获得 TDX VM root 权限
  → cat /proc/<PID>/environ
  → 获得 BOT_TOKEN 明文
  → 完全控制 Bot
```

**成本**: 低 (仅需 TDX VM 内提权)
**收益**: 完全控制 Bot (发消息、踢人、获取群信息)

### 3.2 内存扫描路径

```
攻击者利用 TDX 侧信道
  → dump TDX 内存
  → 搜索 Telegram Token 格式 (数字:字母数字_)
  → 找到 BotConfig.bot_token / TelegramExecutor.bot_token
```

### 3.3 日志路径

```
reqwest 开启 debug/trace 日志
  → API URL 包含 Token
  → 日志文件写入磁盘
  → 攻击者读取日志
```

---

## 4. 根因分析

```
Ceremony (设计) ─────── ✅ Token 仅 SGX 内可见, Shamir 分发, zeroize
        │
        │ 【断裂】
        ▼
Runtime (实现) ──────── ❌ Token 从环境变量明文读入, 存 String, 不清零
```

**核心问题**: 当前 `grouprobot` 的运行时没有实现 "从 Shamir Share 重构 Token" 的路径。Token 的安全仅依赖 TDX 内存加密, 未利用 SGX Enclave 保护。

---

## 5. 修复方案

### 方案 A: SGX Enclave 内 Token 解密 + API 代理 (推荐)

```
┌─── TDX Trust Domain ───────────────────────────────────────┐
│                                                             │
│  grouprobot (无 Token 明文)                              │
│    │                                                        │
│    │ ecall_build_api_url(method) → 返回完整 URL              │
│    ▼                                                        │
│  ┌─── SGX Enclave ─────────────────────────────────────┐   │
│  │  sealed_token: [u8]  (SGX Seal 加密)                 │   │
│  │                                                       │   │
│  │  ecall_unseal_token() → 内部解密, 不返回明文           │   │
│  │  ecall_build_api_url(method) → 内部拼接, 返回完整 URL  │   │
│  │  ecall_sign_request(payload) → 签名 (如需 HMAC)       │   │
│  └───────────────────────────────────────────────────────┘   │
│                                                             │
│  reqwest.get(url)  ← url 由 Enclave 产生, TDX 中仅短暂持有   │
└─────────────────────────────────────────────────────────────┘
```

**关键改动**:
1. 移除 `BotConfig.bot_token: String`, 替换为 `bot_token_sealed: Vec<u8>`
2. 新增 ecall: `ecall_build_api_url(method: &str) -> String`
3. Token 仅在 SGX Enclave 内解密, 拼接完 URL 后立即 zeroize
4. TDX 侧只拿到最终 URL (使用后立即 drop+zeroize)

**安全收益**:
- Token 明文仅在 SGX Enclave 内存中 (微秒级)
- TDX 内存中不存在持久 Token 明文
- 即使 TDX 被攻破, 攻击者拿不到 Token

---

### 方案 B: Ceremony 产出直连运行时 (中期)

```
启动流程:
  1. 从密封文件加载本地 Shamir Share
  2. RA-TLS 连接 K-1 个 peer, 获取其他 Share
  3. SGX Enclave 内 Shamir Recover → Token
  4. Token 仅存于 SGX Enclave 内存
  5. 所有 API 调用通过 ecall 代理

不再需要环境变量 BOT_TOKEN
```

---

### 方案 C: 紧急缓解措施 (短期)

如果无法立即实现方案 A/B, 以下措施可降低风险:

| 措施 | 改动量 | 风险降低 |
|------|:-----:|:-------:|
| **C1**: `BotConfig.bot_token` 使用 `secrecy::SecretString` 替代 `String` | 小 | 中 (防 Debug/Display 泄露, 自动 zeroize) |
| **C2**: 移除环境变量, 改从密封文件加载 Token | 中 | 高 (/proc/environ 不再暴露) |
| **C3**: TelegramExecutor/DiscordExecutor 内使用 `Zeroizing<String>` 包装 | 小 | 中 (drop 时清零) |
| **C4**: 禁止 reqwest debug 日志记录 URL | 小 | 中 (防日志泄露) |
| **C5**: SealedStorage Hardware 模式使用真实 SGX Seal Key | 中 | 高 (密封不可伪造) |

---

## 6. 与设计文档的差距

| 设计文档 (TEE_SGX_TDX_BOT_ANALYSIS.md) | 当前实现 | 差距 |
|--------------------------------------|---------|-----|
| RA-TLS 仪式: 用户浏览器 → SGX Enclave | ❌ 未实现完整 RA-TLS | 🔴 |
| Token 仅 Enclave 内可见 | ❌ 环境变量 + BotConfig 明文 | 🔴 |
| Shamir Share 启动时重构 | ❌ 直接读环境变量 | 🔴 |
| 使用后 zeroize | ❌ String 永不清零 | 🟡 |
| SGX Seal 密封 Share | ⚠️ Software 模式用弱密钥 | 🟡 |
| ecall_decrypt_bot_token (SGX 内解密) | ❌ TDX 中明文使用 | 🔴 |

---

## 7. 修复优先级路线图

```
Sprint 1 (紧急, 1-2 天):
  C1: secrecy::SecretString 替换 bot_token
  C3: Zeroizing<String> 包装 Executor 内 token
  C4: 日志过滤 (redact URL 中的 token)

Sprint 2 (短期, 3-5 天):
  C2: 密封文件加载 Token (移除环境变量)
  C5: Hardware 模式 SGX Seal Key

Sprint 3 (中期, 1-2 周):
  方案 A: ecall_build_api_url — Token 仅在 SGX 内拼接
  
Sprint 4 (完整, 2-3 周):
  方案 B: Ceremony 产出直连运行时
  完整 Shamir Share 启动重构流程
```

---

## 8. 结论

### 当前风险等级: 🔴 高

**核心问题**: BOT_TOKEN 的安全保障在 Ceremony (设计层) 和 Runtime (实现层) 之间存在断裂。Ceremony 设计了完善的 RA-TLS + Shamir + SGX 保护链, 但运行时直接从环境变量读取明文 Token 并以 `String` 形式在 TDX 内存中全生命周期持有, 完全架空了 Ceremony 的安全价值。

**最关键的一句话**: 群管理员通过 DApp 输入的 Token, 经过 Ceremony 安全仪式的保护后, 在运行时阶段又以明文形式暴露在 TDX 内存和环境变量中, **相当于前门上锁后门敞开**。

### 需要修复的核心目标

1. **Token 永远不以明文出现在 SGX Enclave 之外**
2. **移除环境变量 BOT_TOKEN, 改用密封文件 + Shamir 恢复**
3. **API 调用通过 ecall 代理, Token 拼接在 SGX 内完成**

---

*文档版本: v1.0 · 2026-02-22*
*分析范围: grouprobot 完整代码库*
