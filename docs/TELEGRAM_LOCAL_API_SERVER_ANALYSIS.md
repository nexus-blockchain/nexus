# Telegram Bot API 本地服务器 — 深度分析

> 分析日期: 2026-02-22
> 上下文: GroupRobot TEE Bot API Key 安全加固 (参见 GROUPROBOT_APIKEY_LEAK_AUDIT.md)
> 参考: [tdlib/telegram-bot-api](https://github.com/tdlib/telegram-bot-api) (Boost Software License)

---

## 一、问题本质：为什么 Token 会泄漏？

### Telegram Bot API 的协议缺陷

Telegram 的 Bot API 要求将 Token **嵌入 HTTP URL 路径**：

```
https://api.telegram.org/bot<TOKEN>/sendMessage
                          ^^^^^^^^^^^^
                          Token 明文出现在 URL path 中
```

这意味着每次 API 调用，Token 都会出现在：

| 位置 | 风险等级 | 说明 |
|------|---------|------|
| reqwest `http::Uri` 解析缓存 | 🟡 中 | 连接池保留 URL 直到空闲超时 |
| TLS Client Hello SNI | ✅ 安全 | SNI 只含域名 `api.telegram.org`，不含 path |
| HTTPS 加密层内 | ✅ 安全 | path 在 TLS 层内加密 |
| Telegram 服务器端日志 | ⚠️ 不可控 | Telegram 服务端是否记录完整 URL 未知 |
| 网络中间设备 (代理/CDN) | ✅ 安全 | HTTPS 保护，但若使用 HTTP 代理则完全泄漏 |
| 进程内存 (reqwest 内部) | 🟡 中 | 依赖 jemalloc zero-on-free 缓解 |

**核心矛盾**: GroupRobot 在 TEE (TDX/SGX) 中精心保护 Token，但每次调用 Telegram API 时 Token 必须以明文形式离开 TEE，通过公网 HTTPS 发送到 `api.telegram.org`。

---

## 二、Telegram Bot API 本地服务器是什么？

### 架构对比

**当前架构 (直连云端)**:
```
┌─────────────────────────────┐        公网 HTTPS         ┌──────────────────────┐
│  GroupRobot (TDX Enclave)   │ ─── bot<TOKEN>/method ──→ │ api.telegram.org     │
│  Token 在 URL 中跨越公网    │                           │ (Telegram 云服务器)   │
└─────────────────────────────┘                           └──────────────────────┘
     Token 暴露在: reqwest 内部缓存、HTTPS 请求 URL path
```

**本地服务器架构**:
```
┌──────────────────────┐   localhost HTTP    ┌──────────────────────┐   MTProto    ┌──────────────┐
│  GroupRobot (TDX)    │ ── bot<TK>/method → │ telegram-bot-api     │ ──────────→ │ Telegram DC  │
│  Token 仅到 localhost │    127.0.0.1:8081   │ (本地服务器, 同机)    │  (加密协议)  │ (数据中心)    │
└──────────────────────┘                     └──────────────────────┘            └──────────────┘
     Token 暴露在: 本地 loopback 接口 (不离开机器)
```

### 本地服务器工作原理

1. **telegram-bot-api** 是 Telegram 官方开源的 C++ 服务器 ([tdlib/telegram-bot-api](https://github.com/tdlib/telegram-bot-api))
2. 运行在本机 `127.0.0.1:8081`，接受与云端完全相同的 HTTP API 请求
3. 内部使用 **MTProto 协议** (Telegram 的原生加密协议) 与 Telegram 数据中心通信
4. Token 仅用于本地服务器的认证，**不通过公网传输**
5. 需要 `--api-id` 和 `--api-hash` (从 [my.telegram.org](https://my.telegram.org) 获取)

### 关键安全差异

| 维度 | 云端 API | 本地服务器 |
|------|---------|-----------|
| Token 传输路径 | 公网 HTTPS (TLS 内) | localhost loopback |
| 通信协议 | HTTPS (Token-in-URL) | MTProto (无 Token-in-URL) |
| Token 暴露到网络 | ✅ 是 (TLS 保护) | ❌ 否 (仅本机) |
| 中间人攻击面 | HTTPS 证书验证 | 无 (loopback) |
| Telegram 服务端日志 | 可能记录 URL | MTProto 不含 Token URL |
| reqwest 内部缓存 | 含 Token 的 URL | 含 Token 的 URL (但仅本地) |
| 网络嗅探 | 需突破 TLS | 需 root + loopback 抓包 |

---

## 三、对 GroupRobot 的安全收益评估

### 消除的攻击向量

1. **Token 不再离开物理机器** — 即使 TLS 被中间人攻击 (供应链证书攻击等)，Token 也不会泄漏
2. **消除 Telegram 服务端日志风险** — MTProto 连接不包含 `bot<TOKEN>/` URL
3. **消除网络代理/CDN 风险** — 不经过任何网络中间设备
4. **减少 reqwest URL 缓存风险** — URL 目标从 `api.telegram.org` 变为 `127.0.0.1:8081`，即使缓存泄漏也仅在本机

### 未消除的攻击向量

1. **Token 仍然嵌入 URL path** — `http://127.0.0.1:8081/bot<TOKEN>/sendMessage` 格式不变
2. **reqwest 内部仍然缓存 URL** — 但 URL 的 host 是 localhost，风险大幅降低
3. **本地服务器进程内存** — telegram-bot-api 进程内存中持有 Token
4. **loopback 抓包** — root 用户仍可通过 `tcpdump -i lo` 抓取 HTTP 明文

### 安全收益量化

```
当前风险面 (云端):
  公网 HTTPS 传输 ──────── [高] Token 在 TLS 内跨越互联网
  Telegram 服务端日志 ──── [中] 不可控
  reqwest 连接池缓存 ───── [中] Token URL 存活数分钟
  网络中间人 ────────────── [低] 需突破 TLS

本地服务器后风险面:
  localhost HTTP 传输 ──── [极低] 仅 loopback，需 root 权限抓包
  本地服务器进程内存 ───── [低] 与 GroupRobot 同机，受 TDX 保护
  reqwest 连接池缓存 ───── [低] URL 仅到 localhost
  网络中间人 ────────────── [无] 不经过网络

总体风险降低: ≈ 70-80%
```

---

## 四、与 TDX/SGX 深度集成方案

### 方案 A: 同 TDX VM 内运行 (推荐)

```
┌─────────────────── TDX Confidential VM ───────────────────────┐
│                                                                │
│  ┌─────────────────┐  localhost  ┌──────────────────────────┐ │
│  │  GroupRobot      │ ─────────→ │  telegram-bot-api        │ │
│  │  (Rust, Axum)    │   :8081    │  (C++, 同一 VM)          │ │
│  └─────────────────┘             └──────────────────────────┘ │
│                                                                │
│  Token 从不离开 TDX VM 的加密内存空间                          │
│  MTProto 流量虽然出 VM，但不含 Token URL                      │
│                                                                │
└────────────────────────────────────────────────────────────────┘
         │ MTProto (加密)
         ▼
   Telegram 数据中心
```

**优点**:
- Token 完全限制在 TDX VM 内
- loopback 流量也受 TDX 内存加密保护
- 部署简单 (同一 VM 内两个进程)

**缺点**:
- telegram-bot-api 需要在 TDX VM 内编译/运行 (C++ 依赖)
- VM 内存消耗增加 ~100-200MB

### 方案 B: SGX Enclave 内运行 (最高安全)

```
┌──────────── Host Machine ──────────────────────────────────────┐
│                                                                 │
│  ┌─── SGX Enclave (Gramine) ──┐   IPC    ┌──── SGX Enclave ──┐│
│  │  GroupRobot                 │ ───────→ │ telegram-bot-api   ││
│  │  (token_vault.rs)           │  :8081   │ (Gramine 包裹)     ││
│  └─────────────────────────────┘          └────────────────────┘│
│                                                                 │
│  两个 Enclave 之间通过 localhost 通信                           │
│  MRENCLAVE 保证双方代码未被篡改                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**优点**: 最高安全等级，Token 受双层 SGX 保护
**缺点**: Gramine 包裹 C++ 程序复杂度高，可能存在兼容性问题

### 方案 C: 独立容器 + Unix Socket (折中)

```
┌─── Docker Compose ────────────────────────────────────────────────┐
│                                                                    │
│  ┌─────────────────┐   /shared/tg-api.sock   ┌─────────────────┐ │
│  │  grouprobot      │ ─────────────────────→  │ telegram-bot-api │ │
│  │  (TDX container) │    Unix socket          │ (独立容器)       │ │
│  └─────────────────┘                          └─────────────────┘ │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

**优点**: 容器隔离，标准 Docker 部署
**缺点**: Unix socket 跨容器需要 shared volume；telegram-bot-api 容器不在 TDX 内

---

## 五、GroupRobot 代码改动评估

### 需要修改的文件 (4 个)

#### 1. `config.rs` — 添加 TG API base URL 配置

```rust
// 新增字段
pub struct BotConfig {
    ...
    /// Telegram Bot API 服务器地址 (默认: https://api.telegram.org)
    /// 本地服务器: http://127.0.0.1:8081
    pub tg_api_base_url: String,
    ...
}

// from_env() 中添加:
tg_api_base_url: std::env::var("TG_API_BASE_URL")
    .unwrap_or_else(|_| "https://api.telegram.org".into()),
```

**工作量**: ~5 行

#### 2. `token_vault.rs` — `build_tg_api_url()` 支持自定义 base URL

```rust
// 当前:
pub fn build_tg_api_url(&self, method: &str) -> BotResult<Zeroizing<String>> {
    Ok(Zeroizing::new(format!(
        "https://api.telegram.org/bot{}/{}",
        token.as_str(), method
    )))
}

// 改为:
pub fn build_tg_api_url(&self, method: &str, base_url: &str) -> BotResult<Zeroizing<String>> {
    Ok(Zeroizing::new(format!(
        "{}/bot{}/{}",
        base_url.trim_end_matches('/'), token.as_str(), method
    )))
}
```

**问题**: `base_url` 参数需要穿透 `VaultProvider` trait → `VaultClient` → `VaultServer`

**替代方案**: 在 `TokenVault` 内部存储 `base_url`，初始化时注入：

```rust
pub struct TokenVault {
    tg_token: Option<Zeroizing<String>>,
    dc_token: Option<Zeroizing<String>>,
    tg_api_base_url: String,  // 新增: 不包含 token, 非敏感
}
```

**工作量**: ~20 行 (含 VaultProvider trait 签名不变)

#### 3. `.env.example` — 添加新配置项

```env
# Telegram Bot API 服务器 (可选)
# 默认: https://api.telegram.org (Telegram 云端)
# 本地服务器: http://127.0.0.1:8081
# TG_API_BASE_URL=http://127.0.0.1:8081
```

**工作量**: ~3 行

#### 4. `vault_ipc.rs` — IPC 协议可能需要扩展

如果 `build_tg_api_url` 签名改为接受 `base_url` 参数，IPC 协议的 `BuildTgApiUrl` 请求也需要传递 `base_url`。但如果 base_url 存储在 TokenVault 内部，IPC 协议无需修改。

**工作量**: 0 行 (如果 base_url 存在 TokenVault 内)

### 总改动量: ~30 行代码

---

## 六、部署方案 (Docker Compose)

### 最小化部署

```yaml
# docker-compose.yml
version: "3.8"
services:
  # Telegram Bot API 本地服务器
  telegram-bot-api:
    image: aiogram/telegram-bot-api:latest  # 社区维护的镜像
    container_name: tg-local-api
    restart: unless-stopped
    environment:
      TELEGRAM_API_ID: ${TELEGRAM_API_ID}
      TELEGRAM_API_HASH: ${TELEGRAM_API_HASH}
    command:
      - --api-id=${TELEGRAM_API_ID}
      - --api-hash=${TELEGRAM_API_HASH}
      - --local                              # 启用本地模式
      - --http-port=8081
    ports:
      - "127.0.0.1:8081:8081"               # 仅 localhost 暴露
    volumes:
      - tg-api-data:/var/lib/telegram-bot-api

  # GroupRobot TEE Bot
  grouprobot:
    build: ./grouprobot
    container_name: grouprobot
    restart: unless-stopped
    depends_on:
      - telegram-bot-api
    environment:
      PLATFORM: telegram
      TG_API_BASE_URL: http://telegram-bot-api:8081  # 指向本地服务器
      WEBHOOK_PORT: 3000
      WEBHOOK_URL: http://grouprobot:3000/webhook     # 本地 HTTP (无需 HTTPS)
      TEE_MODE: auto
    ports:
      - "3000:3000"

volumes:
  tg-api-data:
```

### 首次迁移步骤

```bash
# 1. 先让 Bot 从 Telegram 云端 logOut (必须!)
curl https://api.telegram.org/bot<TOKEN>/logOut

# 2. 等待几秒后启动本地服务器
docker-compose up -d telegram-bot-api

# 3. 验证本地服务器
curl http://127.0.0.1:8081/bot<TOKEN>/getMe

# 4. 启动 GroupRobot (使用本地服务器)
docker-compose up -d grouprobot
```

### 前置依赖

| 依赖 | 获取方式 | 说明 |
|------|---------|------|
| `TELEGRAM_API_ID` | [my.telegram.org](https://my.telegram.org/auth) → API Development Tools | 每个 Telegram 账号可创建 |
| `TELEGRAM_API_HASH` | 同上 | 与 API_ID 配对 |
| Docker | 已有 | Dockerfile 已存在 |

---

## 七、本地服务器的额外优势

### 1. Webhook 可使用 HTTP (无需 HTTPS)

当前 GroupRobot 需要:
- HTTPS 证书 (Let's Encrypt 或自签)
- 端口限制 (443/80/88/8443)

本地服务器允许:
- **HTTP** Webhook (无需证书)
- **任意端口**
- **任意 IP** (包括 `127.0.0.1`)

这意味着 GroupRobot 的 Webhook 可以完全运行在 localhost，无需暴露到公网。

### 2. 文件上传/下载限制提升

| 操作 | 云端 API | 本地服务器 |
|------|---------|-----------|
| 上传文件 | ≤ 50 MB | ≤ 2000 MB |
| 下载文件 | ≤ 20 MB | 无限制 |
| 文件路径 | 需 HTTP 下载 | 本地绝对路径 |

### 3. Webhook 连接数

| 配置 | 云端 API | 本地服务器 |
|------|---------|-----------|
| `max_webhook_connections` | ≤ 100 | ≤ 100,000 |

---

## 八、风险和注意事项

### 1. api_id / api_hash 成为新的敏感凭据

本地服务器需要 `--api-id` 和 `--api-hash` 参数。这两个凭据:
- 绑定到 Telegram 用户账号 (非 Bot)
- 如果泄漏，攻击者可以创建自己的本地服务器实例
- **但无法**仅凭 api_id/hash 控制 Bot (仍需 Bot Token)

**建议**: 与 Bot Token 使用相同级别的保护措施 (环境变量 + 读取后删除)。

### 2. 本地服务器不验证 Bot Token 来源

本地服务器不区分来自不同客户端的请求。同机任何进程只要知道 Token 就能调用 API。

**缓解**: `ports: "127.0.0.1:8081:8081"` 限制仅 localhost 访问。

### 3. logOut 后的过渡期

调用 `logOut` 后:
- Bot 立即从云端 API 断开
- 需要等待几秒才能在本地服务器上使用
- 过渡期间 Bot **不接收任何更新** (消息丢失)

**建议**: 在低流量时段执行迁移。

### 4. 本地服务器自身的可靠性

- telegram-bot-api 需要稳定的互联网连接到 Telegram DC
- 如果本地服务器崩溃，Bot 完全停止工作 (不像云端 API 有 Telegram 的 SLA)
- 需要监控和自动重启 (`restart: unless-stopped`)

### 5. MTProto 连接的 IP 暴露

虽然 Token 不在 MTProto 流量中，但本地服务器的 **出口 IP** 对 Telegram DC 可见。这与直接使用云端 API 时 GroupRobot 的出口 IP 相同，不是新增风险。

---

## 九、决策矩阵

| 维度 | 权重 | 云端 API | 本地服务器 |
|------|------|---------|-----------|
| Token 网络暴露 | 35% | 🟡 3/5 | ✅ 5/5 |
| 部署复杂度 | 20% | ✅ 5/5 | 🟡 3/5 |
| 运维负担 | 15% | ✅ 5/5 | 🟡 3/5 |
| 功能限制 | 10% | 🟡 3/5 | ✅ 5/5 |
| TDX 集成 | 20% | 🟡 3/5 | ✅ 5/5 |
| **加权总分** | **100%** | **3.6** | **4.4** |

---

## 十、结论与建议

### 推荐: 生产环境必须使用本地服务器

**理由**:
1. **消除最大攻击面**: Token 不再通过公网传输，风险降低 70-80%
2. **代码改动极小**: 仅需 ~30 行，核心是让 `TG_API_BASE_URL` 可配置
3. **与 TDX 天然契合**: 本地服务器运行在同一 TDX VM 内，Token 永远不离开加密内存
4. **额外功能收益**: HTTP Webhook、大文件支持、更高连接数

### 实施路线图

```
Phase 1 (立即): 代码改动 — 让 base_url 可配置 (~30 行, 1h)
Phase 2 (本周): Docker Compose 集成 — telegram-bot-api 容器 + GroupRobot
Phase 3 (部署): 生产迁移 — logOut → 本地服务器启动 → 验证
Phase 4 (可选): SGX Enclave 包裹 — 对 telegram-bot-api 使用 Gramine
```
