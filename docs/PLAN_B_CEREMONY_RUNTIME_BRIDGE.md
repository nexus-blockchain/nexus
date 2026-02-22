# 方案 B 深度分析: Ceremony 产出直连运行时

> 日期: 2026-02-22 · 版本: v1.0
> 前置: BOT_TOKEN_SECURITY_ANALYSIS.md

---

## 1. 现状诊断

### 1.1 当前架构中的两条断裂链

```
═══ 链条 A: Ceremony (nexus-tee-bot) ═══

  用户 DApp
    │ RA-TLS
    ▼
  CeremonyServer.run_ceremony(bot_token)     ← nexus-tee-bot/src/ceremony.rs:177
    │ 接收 Token 明文
    │ 生成 Ed25519 密钥
    │ encode_secrets(token + signing_key)     ← 将 Token 编码进 secrets
    │ shamir::split(secrets, K, N)            ← 分片
    │ encrypt_share + 分发到 peer             ← ECDH 加密分发
    │ save_local_share()                      ← 本地密封保存
    ▼
  仪式完成, Token 仅存于 N 个 TEE 节点的 Sealed Share 中

═══ 链条 B: Runtime (grouprobot) ═══

  main.rs:68   → BotConfig::from_env()       ← 读环境变量 BOT_TOKEN
  main.rs:115  → TelegramExecutor::new(cfg.bot_token.clone())   ← 明文 String
  main.rs:124  → DiscordExecutor::new(dc_cfg.bot_token.clone()) ← 明文 String
  main.rs:187  → DiscordGateway::new(dc_token)                  ← 明文 String
    │
    ▼
  进程生命周期: Token 以 String 持续存在于 TDX 内存
```

**断裂点**: 链条 A 的产出 (Shamir Shares) 从未被链条 B 消费。链条 B 完全绕过 A，直接从环境变量获取 Token。

### 1.2 nexus-tee-bot Ceremony 已实现的能力

| 能力 | 文件 | 状态 |
|------|------|:----:|
| 接收 Token + 生成 Ed25519 | `ceremony.rs:177-274` | ✅ |
| encode_secrets(token + sk) | `ceremony.rs:338-346` | ✅ |
| decode_secrets → (sk, token) | `ceremony.rs:349-366` | ✅ |
| Shamir split (GF256) | `shamir.rs` | ✅ |
| Shamir recover | `shamir.rs` | ✅ |
| encrypt_share (AES-GCM) | `shamir.rs` | ✅ |
| decrypt_share | `shamir.rs` | ✅ |
| save_local_share / load_local_share | `enclave_bridge.rs` | ✅ |
| seal_key / unseal (SGX/Software) | `enclave_bridge.rs` | ✅ |
| RA-TLS peer 验证 | `ceremony.rs:277-301` | ⚠️ 骨架 |
| ECDH 密钥协商 | — | ❌ 未实现 |

### 1.3 grouprobot 已实现的能力

| 能力 | 文件 | 状态 |
|------|------|:----:|
| EnclaveBridge (Ed25519 + seal/unseal) | `tee/enclave_bridge.rs` | ✅ |
| SealedStorage (AES-GCM) | `tee/sealed_storage.rs` | ✅ |
| ShamirSplitter (split/recover 骨架) | `tee/shamir.rs` | ⚠️ 简化 |
| CeremonyClient (骨架) | `tee/ceremony.rs` | ⚠️ 仅分片 Ed25519 |
| Attestor (TDX+SGX Quote) | `tee/attestor.rs` | ✅ |
| KeyManager (sign/verify) | `tee/key_manager.rs` | ✅ |
| Platform Executors (TG/DC) | `platform/` | ✅ (但 Token 为 String) |

---

## 2. 方案 B 目标架构

```
┌─── Phase 1: 仪式阶段 (一次性) ──────────────────────────────────┐
│                                                                   │
│  DApp 浏览器                                                       │
│    │ (1) RA-TLS + 验证 MRENCLAVE                                  │
│    ▼                                                               │
│  Ceremony Enclave                                                  │
│    │ (2) 接收 bot_token 明文                                       │
│    │ (3) 生成 Ed25519 keypair                                      │
│    │ (4) secrets = encode(token + signing_key)                     │
│    │ (5) shares = Shamir.split(secrets, K=2, N=3)                 │
│    │ (6) 对每个 TEE 节点: RA-TLS 验证 Quote → ECDH → 分发 share    │
│    │ (7) zeroize(secrets, token, signing_key)                     │
│    │ (8) record_ceremony() 到链上                                  │
│    ▼                                                               │
│  每个 TEE 节点: sealed_share.dat (AES-GCM / SGX Seal)             │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘

┌─── Phase 2: 运行时阶段 (每次启动) ───────────────────────────────┐
│                                                                   │
│  grouprobot 启动                                               │
│    │                                                               │
│    │ (A) 加载本地 sealed_share.dat → 解密得到 local_share          │
│    │ (B) RA-TLS 连接 K-1 个 peer TEE 节点                         │
│    │     ├─ 验证对方 SGX Quote (MRENCLAVE 白名单)                  │
│    │     ├─ ECDH 密钥协商                                          │
│    │     └─ 接收对方的 encrypted_share → 解密                      │
│    │ (C) Shamir.recover(K shares) → secrets                       │
│    │ (D) decode_secrets(secrets) → (signing_key, bot_token)       │
│    │ (E) signing_key → EnclaveBridge.keypair                      │
│    │ (F) bot_token → TokenVault (Enclave 内部, 不暴露)             │
│    │ (G) zeroize(secrets, shares)                                  │
│    │                                                               │
│    │ ═══ 运行时: Token 永不出 Enclave ═══                          │
│    │                                                               │
│    │ TelegramExecutor.call(method)                                 │
│    │    → TokenVault.build_api_url(method) → url (短暂持有)        │
│    │    → reqwest.get(url)                                         │
│    │    → zeroize(url)                                             │
│    │                                                               │
│    │ DiscordExecutor.call(path)                                    │
│    │    → TokenVault.build_auth_header() → header (短暂持有)       │
│    │    → reqwest.request().header(auth)                           │
│    │    → zeroize(header)                                          │
│    ▼                                                               │
│  Token 明文在内存中仅存在微秒级                                     │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

---

## 3. 需要改造的模块清单

### 3.1 新增模块

| 模块 | 文件 | 说明 |
|------|------|------|
| **TokenVault** | `tee/token_vault.rs` | Token 安全容器, 仅通过 ecall 接口暴露 URL/Header |
| **ShareRecovery** | `tee/share_recovery.rs` | 启动时 K-of-N share 收集 + Shamir 恢复 |
| **PeerClient** | `tee/peer_client.rs` | RA-TLS 客户端, 连接 peer 获取 share |

### 3.2 改造现有模块

| 模块 | 改动 | 影响 |
|------|------|------|
| `config.rs` | 移除 `bot_token: String`, 新增 `sealed_share_path`, `peer_endpoints` | **S1 修复** |
| `main.rs` | 启动流程重写: 先 ShareRecovery → 再构建 Executor | **S3 修复** |
| `tee/enclave_bridge.rs` | 新增 `store_token()`, `build_api_url()`, `build_auth_header()` | **核心** |
| `tee/shamir.rs` | 升级为完整 GF(256) (可移植 nexus-tee-bot 实现) | **S3 依赖** |
| `tee/ceremony.rs` | 改为 Ceremony 接收端 (接收 share + 密封存储) | **S3 依赖** |
| `platform/telegram/executor.rs` | `bot_token: String` → `vault: Arc<TokenVault>` | **S2 修复** |
| `platform/discord/executor.rs` | 同上 | **S2 修复** |
| `platform/discord/gateway.rs` | `token: String` → `vault: Arc<TokenVault>` | **S2 修复** |

### 3.3 不需改动

| 模块 | 原因 |
|------|------|
| `chain/` | 链交互与 Token 无关 |
| `processing/` | 规则引擎不接触 Token |
| `infra/` | 指标/限流与 Token 无关 |
| `webhook.rs` | Webhook 处理不涉及 Token |

---

## 4. TokenVault 设计

### 4.1 核心接口

```rust
/// Token 安全容器 — Token 永不以 String 形式暴露到容器外部
pub struct TokenVault {
    /// Telegram Token (Zeroizing 包装)
    tg_token: Option<Zeroizing<String>>,
    /// Discord Token (Zeroizing 包装)
    dc_token: Option<Zeroizing<String>>,
    /// EnclaveBridge 引用 (Hardware 模式下 Token 存于 Enclave)
    enclave: Arc<EnclaveBridge>,
}

impl TokenVault {
    /// 从 Shamir 恢复结果注入 Token (仅调用一次)
    pub fn inject_token(&mut self, platform: &str, token: Zeroizing<String>);

    /// 构建 Telegram API URL (Token 在内部拼接, 返回完整 URL)
    /// 返回值使用 Zeroizing<String>, drop 时自动清零
    pub fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>>;

    /// 构建 Discord Authorization header
    pub fn build_dc_auth_header(&self) -> BotResult<Zeroizing<String>>;

    /// 构建 Discord Gateway Identify payload
    pub fn build_dc_identify_payload(&self, intents: u64) -> BotResult<Zeroizing<String>>;

    /// 安全清零所有 Token
    pub fn zeroize_all(&mut self);
}
```

### 4.2 安全属性

| 属性 | 保证 |
|------|------|
| Token **不实现** `Display`/`Debug` | 防止日志泄露 |
| Token 使用 `Zeroizing<String>` | drop 时内存清零 |
| `build_*` 返回 `Zeroizing<String>` | 调用方用完即清零 |
| 不提供 `get_token()` 方法 | Token 永远不以原始形式暴露 |
| Hardware 模式: Token 存于 SGX Enclave | TDX 内存无 Token |

### 4.3 Hardware vs Software 模式差异

| | Software 模式 | Hardware 模式 |
|-|--------------|--------------|
| Token 存储位置 | `Zeroizing<String>` (TDX 内存) | SGX EPC (Enclave 内存) |
| `build_*` 实现 | Rust 内存中拼接 | ecall 在 Enclave 内拼接 |
| 安全保证 | 进程内保护 (zeroize) | 硬件级隔离 (SGX) |

---

## 5. ShareRecovery 设计

### 5.1 启动恢复流程

```
┌──────────────────────────────────────────────┐
│ ShareRecovery::recover_token()               │
│                                              │
│ 1. load sealed_share.dat                     │
│    ├─ 存在 → unseal → local_share            │
│    └─ 不存在 → 进入 "首次仪式" 模式          │
│                                              │
│ 2. 查询链上 ActiveCeremony(bot_pk)           │
│    ├─ 存在 → 获取 K, N, 参与者列表            │
│    └─ 不存在 → Error: 需要先执行仪式          │
│                                              │
│ 3. 连接 K-1 个 peer (需要总共 K 个 share)     │
│    for peer in peers:                        │
│      ├─ RA-TLS 握手                          │
│      ├─ 验证 peer SGX Quote                  │
│      ├─ ECDH 密钥协商 → session_key          │
│      ├─ 请求: ShareRequest{ceremony_hash}     │
│      ├─ 接收: EncryptedShare                  │
│      ├─ decrypt_share(share, session_key)     │
│      └─ collected_shares.push(share)          │
│      if collected_shares.len() >= K: break   │
│                                              │
│ 4. Shamir.recover(collected_shares)          │
│    → secrets: Vec<u8>                        │
│                                              │
│ 5. decode_secrets(secrets)                   │
│    → (signing_key, bot_token)                │
│                                              │
│ 6. signing_key → EnclaveBridge.inject_key()  │
│    bot_token  → TokenVault.inject_token()    │
│                                              │
│ 7. zeroize(secrets, collected_shares)        │
│                                              │
│ return Ok(token_vault)                       │
└──────────────────────────────────────────────┘
```

### 5.2 Fallback 策略

| 场景 | 行为 |
|------|------|
| 本地 share 存在 + K-1 peer 可达 | ✅ 正常恢复 |
| 本地 share 存在 + peer 不足 | ⏳ 等待重试 (指数退避, 最多 5min) |
| 本地 share 不存在 | 🔴 需要重新执行 Ceremony |
| Shamir recover 失败 (数据损坏) | 🔴 需要重新执行 Ceremony |
| 链上无 ActiveCeremony | 🔴 首次部署, 需要 Ceremony |

### 5.3 首次部署 (无 Share) 兼容

首次部署时尚无 Ceremony 产出，需要一个过渡方案:

```
if sealed_share.dat 不存在 && 环境变量 BOT_TOKEN 存在:
    warn!("使用环境变量 Token (不安全, 仅用于首次 Ceremony 前的过渡)")
    TokenVault.inject_token(env_token)
    标记 needs_ceremony = true
    
    // 后台: 启动 Ceremony 流程
    // Ceremony 完成后: 删除环境变量 Token 依赖
```

---

## 6. Platform Executor 改造

### 6.1 TelegramExecutor (改造前 vs 改造后)

```rust
// ═══ 改造前 (不安全) ═══
pub struct TelegramExecutor {
    bot_token: String,          // ← Token 明文, 永久持有
    http: reqwest::Client,
}

fn api_url(&self, method: &str) -> String {
    format!("https://api.telegram.org/bot{}/{}", self.bot_token, method)
}


// ═══ 改造后 (安全) ═══
pub struct TelegramExecutor {
    vault: Arc<TokenVault>,     // ← 不持有 Token, 通过 vault 获取
    http: reqwest::Client,
}

async fn call_api(&self, method: &str, body: Option<Value>) -> BotResult<Value> {
    let url = self.vault.build_tg_api_url(method)?;  // Zeroizing<String>
    let resp = self.http.post(url.as_str())
        .json(&body)
        .send()
        .await?;
    // url 在此 drop, 自动 zeroize
    Ok(resp.json().await?)
}
```

### 6.2 DiscordExecutor (改造前 vs 改造后)

```rust
// ═══ 改造前 ═══
pub struct DiscordExecutor {
    bot_token: String,          // ← 明文
    ...
}
fn call_api(&self, method: Method, path: &str, body: Option<Value>) -> ... {
    self.http.request(method, self.api_url(path))
        .header("Authorization", format!("Bot {}", self.bot_token))  // ← 明文
}


// ═══ 改造后 ═══
pub struct DiscordExecutor {
    vault: Arc<TokenVault>,
    ...
}
async fn call_api(&self, method: Method, path: &str, body: Option<Value>) -> ... {
    let auth = self.vault.build_dc_auth_header()?;  // Zeroizing<String>
    self.http.request(method, &format!("https://discord.com/api/v10{}", path))
        .header("Authorization", auth.as_str())
        .send().await?;
    // auth 在此 drop, 自动 zeroize
}
```

### 6.3 DiscordGateway (改造前 vs 改造后)

```rust
// ═══ 改造前 ═══
pub struct DiscordGateway {
    token: String,  // ← 明文持续存在
}
// Identify 中发送 token 明文

// ═══ 改造后 ═══
pub struct DiscordGateway {
    vault: Arc<TokenVault>,
}
// Identify 时: let payload = self.vault.build_dc_identify_payload(intents)?;
// payload 使用后立即 zeroize
```

---

## 7. main.rs 启动流程改造

### 7.1 改造后启动序列

```
main()
  │
  ├─ 1. 初始化日志、加载配置 (config 不再包含 bot_token)
  │
  ├─ 2. EnclaveBridge::init()
  │
  ├─ 3. ShareRecovery::recover_token()
  │     ├─ 加载本地 sealed_share
  │     ├─ RA-TLS 连接 peer, 收集 K 个 share
  │     ├─ Shamir recover → (signing_key, bot_token)
  │     ├─ → EnclaveBridge.inject_key(signing_key)
  │     └─ → TokenVault { tg_token, dc_token }
  │
  ├─ 4. KeyManager::new(enclave)
  │     (signing_key 已由 ShareRecovery 注入)
  │
  ├─ 5. Attestor::generate_attestation()
  │
  ├─ 6. TelegramExecutor::new(vault.clone(), http)
  │     DiscordExecutor::new(vault.clone(), http)
  │     DiscordGateway::new(vault.clone(), intents, event_tx)
  │     (所有 Executor 共享 TokenVault, 不持有 Token)
  │
  ├─ 7. MessageRouter, RuleEngine, Infra ...
  │
  ├─ 8. 后台任务: 链连接, 证明刷新, 日志批量提交
  │
  └─ 9. HTTP 服务器启动
```

### 7.2 BotConfig 改造

```rust
// ═══ 改造前 ═══
pub struct BotConfig {
    pub bot_token: String,             // 🔴 明文 Token
    pub discord: Option<DiscordConfig>,
    ...
}

// ═══ 改造后 ═══
pub struct BotConfig {
    // bot_token 已移除!
    pub bot_id_hash: [u8; 32],         // 从链上或密封文件加载
    pub sealed_share_path: String,     // 本地 share 路径
    pub peer_endpoints: Vec<String>,   // K-1 个 peer 的 RA-TLS 端点
    pub discord_application_id: Option<String>,
    ...
}
```

---

## 8. 依赖项变更

### 8.1 新增 Cargo 依赖

```toml
# 安全清零
zeroize = { version = "1.8", features = ["zeroize_derive"] }
secrecy = "0.8"

# RA-TLS 客户端 (连接 peer)
rustls = "0.23"
tokio-rustls = "0.26"

# ECDH 密钥协商
x25519-dalek = { version = "2.0", features = ["static_secrets"] }
```

### 8.2 从 nexus-tee-bot 移植

| 组件 | 源文件 | 可复用度 |
|------|--------|:--------:|
| Shamir GF(256) 完整实现 | `nexus-tee-bot/src/shamir.rs` | 95% |
| encode_secrets / decode_secrets | `nexus-tee-bot/src/ceremony.rs:338-366` | 100% |
| EncryptedShare + encrypt/decrypt | `nexus-tee-bot/src/shamir.rs` | 90% |
| save_local_share / load_local_share | `nexus-tee-bot/src/enclave_bridge.rs` | 80% |

---

## 9. 安全属性对照

| 风险 | 改造前 | 改造后 |
|------|:------:|:------:|
| **S1**: 环境变量明文 | 🔴 `std::env::var("BOT_TOKEN")` | ✅ 无环境变量, 从 Shamir Share 恢复 |
| **S2**: 多份明文 String | 🔴 4 处 String 永久持有 | ✅ `Zeroizing<String>` 微秒级存在 |
| **S3**: Ceremony 断裂 | 🔴 Ed25519 seed, 非 Token | ✅ secrets = token + sk, 完整闭环 |
| **M1**: 软件密封弱密钥 | 🟡 SHA256(hostname+salt+machine-id) | 🟡 不变 (需 Hardware 模式彻底修复) |
| **M2**: URL 含 Token | 🟡 format! 产生持久 String | ✅ `Zeroizing<String>` 用完即清零 |
| **L1**: Ceremony 未 zeroize | 🟢 依赖 drop | ✅ 显式 `zeroize()` |

---

## 10. 开发计划

### Sprint B1: TokenVault + Zeroize 基础 (1-2 天)

```
1. 添加 zeroize + secrecy 依赖
2. 创建 tee/token_vault.rs — TokenVault 结构体 + build_*_url/header 方法
3. 单元测试: inject → build → zeroize 验证内存清零
4. 改造 TelegramExecutor: bot_token → vault
5. 改造 DiscordExecutor: bot_token → vault
6. 改造 DiscordGateway: token → vault
7. 临时: TokenVault 从环境变量加载 (兼容现有部署)
```

### Sprint B2: Shamir 完整实现 + secrets 编解码 (1-2 天)

```
1. 从 nexus-tee-bot 移植完整 GF(256) Shamir 到 tee/shamir.rs
2. 移植 encode_secrets / decode_secrets
3. 移植 EncryptedShare + encrypt/decrypt
4. 扩展 EnclaveBridge: save_local_share / load_local_share
5. 单元测试: split → encrypt → save → load → decrypt → recover → decode
```

### Sprint B3: ShareRecovery 启动恢复 (2-3 天)

```
1. 创建 tee/share_recovery.rs
2. 创建 tee/peer_client.rs — RA-TLS 连接 + share 交换协议
3. 实现启动恢复流程 (load local + connect peers + recover)
4. 改造 main.rs 启动序列
5. 改造 config.rs: 移除 bot_token, 新增 sealed_share_path + peer_endpoints
6. Fallback: 环境变量过渡模式
7. 集成测试: 3 节点本地启动, K=2 恢复
```

### Sprint B4: Ceremony 接收端 (1-2 天)

```
1. 改造 tee/ceremony.rs 为 Share 接收端 (非发起端, 发起端在 DApp/专用仪式服务)
2. 新增 HTTP 端点: POST /ceremony/receive-share
3. 接收: RA-TLS 验证 → ECDH 解密 → 密封存储
4. 链上确认: record_share_received()
```

### Sprint B5: 端到端测试 + 清理 (1-2 天)

```
1. 端到端: DApp 模拟提交 Token → Ceremony → 分发 → 节点重启 → 恢复 → API 调用
2. 移除环境变量 BOT_TOKEN 兼容代码
3. 更新 .env.example + README.md
4. 安全审计: grep 确认无 Token 明文残留
```

**总工期: 6-11 天**

---

## 11. 关键技术决策点

### Q1: Token 在 Software 模式下能否实现硬件级隔离?

**答: 不能。** Software 模式下 TokenVault 的 `Zeroizing<String>` 仍在 TDX 内存中。但对比现状:

| | 现状 | 改造后 Software | 改造后 Hardware |
|-|------|:---------------:|:---------------:|
| /proc/environ 可读 | ✅ 可读 | ❌ 不可读 | ❌ 不可读 |
| Token 持续存在 | ✅ 全生命周期 | ❌ 微秒级 | ❌ 仅 SGX EPC |
| Token 副本数 | 4+ 份 | 1 份 (vault) | 0 份 (Enclave 内) |
| 日志泄露风险 | 高 | 低 (Zeroizing) | 无 |

**结论**: Software 模式下改造仍有显著安全提升, 但完整保证需 Hardware 模式。

### Q2: 首次部署 (无 Share) 如何处理?

**方案**: 首次部署通过 Ceremony 服务接收 Token 并分发 Share。在 Ceremony 服务就绪前, 保留环境变量 fallback 但标记为 `deprecated`, 日志告警。

### Q3: 节点重启时 peer 不可达怎么办?

**方案**: 
1. 允许配置多于 K-1 个 peer (冗余)
2. 指数退避重试, 最多 5 分钟
3. 超时后拒绝启动 (不降级到不安全模式)
4. 监控告警: `share_recovery_failed` 指标

### Q4: Token 轮换 (Revoke + 新 Token) 如何处理?

**方案**: 重新执行 Ceremony。新 Token → 新 secrets → 新 Shamir Shares → 分发。旧 Share 自动失效 (Shamir 恢复出的是旧 Token, 已在 Telegram 侧被 revoke)。

---

*文档版本: v1.0 · 2026-02-22*
*范围: grouprobot 方案 B 架构分析*
