# API 参考

> 版本: 0.1.0 | 最后更新: 2026-02-24

## 1. HTTP 端点

GroupRobot 使用 Axum 框架提供 HTTP 服务，默认监听端口 `3000`。

### 1.1 POST /webhook — Telegram 事件入口

接收 Telegram Bot API 推送的 Webhook 事件。

**Headers**:

| Header | 必须 | 说明 |
|--------|------|------|
| `X-Telegram-Bot-Api-Secret-Token` | 条件 | 当 `WEBHOOK_SECRET` 非空时必须匹配 |
| `Content-Type` | 是 | `application/json` |

**请求体**: Telegram Update JSON (由 Telegram 服务器推送)

**处理流程**:
1. Secret 验证 → 失败返回 `401 Unauthorized`
2. 全局限流 → 超限返回 `429 Too Many Requests`
3. 平台事件解析 → 无法解析返回 `200 OK` (静默丢弃)
4. Per-group 限流 → 超限返回 `429`
5. 指标记录 (`messages_total` +1)
6. 上下文提取 + 管理员身份查询 (仅命令消息)
7. 指纹去重 (300s 窗口) → 重复返回 `200 OK`
8. 规则引擎评估 + 动作执行
9. 返回 `200 OK`

**响应**:

| 状态码 | 说明 |
|--------|------|
| 200 | 正常处理 (含静默丢弃) |
| 401 | Webhook Secret 验证失败 |
| 429 | 限流 (全局或 per-group) |

### 1.2 GET /health — 健康检查

**响应** (JSON):

```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_secs": 3600,
  "platform": "Telegram",
  "tee_mode": "hardware"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `status` | string | 始终 `"ok"` |
| `version` | string | Cargo.toml 版本号 |
| `uptime_secs` | u64 | 进程运行秒数 |
| `platform` | string | `"Telegram"` / `"Discord"` / `"Both"` |
| `tee_mode` | string | `"hardware"` / `"software"` / `"auto"` |

### 1.3 GET /v1/status — 详细状态

**响应** (JSON):

```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_secs": 3600,
  "bot_id_hash": "0xabcdef...",
  "public_key": "abcdef1234...",
  "tee_mode": "Hardware",
  "cached_groups": 42,
  "local_store_counters": 128,
  "local_store_fingerprints": 256
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `bot_id_hash` | string | Bot ID 的 SHA-256 哈希 (hex) |
| `public_key` | string | Ed25519 公钥 (hex, 64 字符) |
| `tee_mode` | string | TEE 运行模式 (Display 格式) |
| `cached_groups` | usize | ConfigManager 缓存的群数量 |
| `local_store_counters` | usize | LocalStore 中的计数器条目数 |
| `local_store_fingerprints` | usize | LocalStore 中的指纹条目数 |

### 1.4 GET /metrics — Prometheus 指标

返回 Prometheus 文本格式的指标数据。仅在 `METRICS_ENABLED=true` 时注册。

**指标列表**:

| 指标名 | 类型 | 标签 | 说明 |
|--------|------|------|------|
| `grouprobot_messages` | Counter | — | 处理的消息总数 |
| `grouprobot_actions` | Counter | `action_type` | 执行的管理动作数 (按类型) |
| `grouprobot_chain_tx` | Counter | `status` | 链上交易数 (`success` / `failed`) |
| `grouprobot_quote_refresh` | Counter | `status` | TEE 证明刷新次数 |
| `grouprobot_rule_matches` | Counter | `rule` | 规则匹配次数 (按规则名) |
| `grouprobot_active_groups` | Gauge | — | 当前活跃群组数 |

**示例输出**:

```
# HELP grouprobot_messages Total messages processed
# TYPE grouprobot_messages counter
grouprobot_messages_total 1234
# HELP grouprobot_actions Total actions executed
# TYPE grouprobot_actions counter
grouprobot_actions_total{action_type="ban"} 5
grouprobot_actions_total{action_type="mute"} 12
grouprobot_actions_total{action_type="delete_message"} 89
```

### 1.5 Ceremony 端点

当 `CEREMONY_PORT > 0` 时，在独立端口启动 Ceremony HTTP 服务:

| 路径 | 方法 | 说明 |
|------|------|------|
| `/share/request` | POST | 请求 Shamir share (需 AttestationGuard) |
| `/share/distribute` | POST | 主动分发 share 给 peer |
| `/ceremony/status` | GET | 当前仪式状态 |

详见 [TEE_SECURITY.md](TEE_SECURITY.md) § 8.

### 1.6 RA-TLS Token 注入

| 路径 | 方法 | 说明 |
|------|------|------|
| `/provision` | POST | RA-TLS Token 安全注入 (首次部署) |

详见 [TEE_SECURITY.md](TEE_SECURITY.md) § 11.

## 2. 环境变量配置

所有配置通过环境变量注入，由 `BotConfig::from_env()` 解析。

### 2.1 平台配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PLATFORM` | `telegram` | 平台模式: `telegram` / `discord` / `both` |
| `BOT_TOKEN` | — | Telegram Bot Token (过渡用, 优先用 Shamir) |
| `DISCORD_TOKEN` | — | Discord Bot Token (过渡用) |
| `DISCORD_APPLICATION_ID` | — | Discord Application ID (Discord 模式必须) |
| `DISCORD_INTENTS` | `33281` | Discord Gateway Intents 位掩码 |
| `BOT_ID_HASH` | — | Bot ID 哈希 (hex, 64字符); 未设则从 BOT_TOKEN 派生 |

### 2.2 网络配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `WEBHOOK_PORT` | `3000` | HTTP 服务监听端口 |
| `WEBHOOK_URL` | `""` | Telegram Webhook 回调 URL (如 `https://bot.example.com/webhook`) |
| `WEBHOOK_SECRET` | `""` | Webhook Secret Token (空=不验证) |
| `WEBHOOK_RATE_LIMIT` | `200` | 全局每秒请求上限 |

### 2.3 链上配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `CHAIN_RPC` | `ws://127.0.0.1:9944` | Substrate RPC WebSocket 地址 |
| `CHAIN_SIGNER_SEED` | — | SR25519 签名密钥 seed (hex; 使用后自动清除) |
| `CHAIN_LOG_BATCH_INTERVAL` | `6` | 动作日志批量提交间隔 (秒) |
| `CHAIN_LOG_BATCH_SIZE` | `50` | 动作日志批量大小 |

### 2.4 TEE 配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `TEE_MODE` | `auto` | TEE 模式: `hardware` / `tdx` / `sgx` / `software` / `auto` |
| `DATA_DIR` | `./data` | 密封存储数据目录 |
| `VAULT_MODE` | `inprocess` | Vault 模式: `inprocess` / `spawn` / `connect` |
| `VAULT_SOCKET` | `""` | Vault Unix Socket 路径 (spawn/connect 模式) |

### 2.5 Shamir 配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `SHAMIR_THRESHOLD` | `2` | Shamir 门限 K (需要 K 个 share 恢复, 默认 K=2, N=4) |
| `PEER_ENDPOINTS` | `""` | Peer TEE 节点端点 (逗号分隔, 如 `https://peer1:8443,https://peer2:8443`) |
| `CEREMONY_PORT` | `0` | Ceremony HTTP 端口 (0=禁用) |

### 2.6 监控配置

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `METRICS_ENABLED` | `true` | 是否启用 Prometheus 指标端点 |
| `LOG_LEVEL` | `info` | 日志级别: `trace` / `debug` / `info` / `warn` / `error` |

## 3. 内部数据结构

### 3.1 AppState — 全局共享状态

```rust
pub struct AppState {
    pub config: BotConfig,
    pub enclave: Arc<EnclaveBridge>,
    pub router: Arc<MessageRouter>,
    pub telegram_executor: Option<Arc<TelegramExecutor>>,
    pub discord_executor: Option<Arc<DiscordExecutor>>,
    pub local_store: Arc<LocalStore>,
    pub config_manager: Arc<ConfigManager>,
    pub rate_limiter: Arc<RateLimiter>,
    pub metrics: SharedMetrics,
    pub start_time: std::time::Instant,
}
```

通过 `Arc<AppState>` 注入 Axum 路由处理器。

### 3.2 LocalStore — 进程内键值存储

基于 `DashMap` 的并发安全存储，用于防刷屏计数、消息去重等:

| 方法 | 说明 |
|------|------|
| `increment(key, ttl_secs) → u64` | 递增计数器 (自动过期) |
| `get_count(key) → u64` | 获取计数 |
| `check_fingerprint(fp, ttl_secs) → bool` | 检查指纹是否已存在 (true=重复) |
| `cleanup()` | 清理过期条目 (每 60s 后台执行) |
| `counter_count() → usize` | 计数器条目数 |
| `fingerprint_count() → usize` | 指纹条目数 |

### 3.3 RateLimiter — 令牌桶限流

| 方法 | 说明 |
|------|------|
| `allow() → bool` | 全局限流检查 |
| `allow_for(group_id) → bool` | Per-group 限流检查 |

全局容量由 `WEBHOOK_RATE_LIMIT` 配置; per-group 容量默认为全局容量的 1/10。

### 3.4 ConfigManager — 链上配置缓存

| 方法 | 说明 |
|------|------|
| `get(group_id) → Option<ChainCommunityConfig>` | 获取群配置 (缓存优先) |
| `fetch_and_cache(group_id, chain) → Result<>` | 从链上拉取并缓存 |
| `cached_count() → usize` | 缓存群数量 |
| `sync_loop(chain)` | 后台同步循环 (30 分钟间隔) |

## 4. 管理命令参考

通过平台消息触发的管理命令 (由 `CommandRule` 处理):

### 4.1 公开命令

| 命令 | 说明 | 示例 |
|------|------|------|
| `/help` | 显示帮助信息 | `/help` |
| `/id` | 显示当前群/用户 ID | `/id` |
| `/rules` | 显示群规则 | `/rules` |

### 4.2 管理员命令

| 命令 | 参数 | 说明 | 示例 |
|------|------|------|------|
| `/ban` | `<user_id>` | 永久封禁用户 | `/ban 123456` |
| `/kick` | `<user_id>` | 踢出用户 (可重新加入) | `/kick 123456` |
| `/mute` | `<user_id> [秒数]` | 禁言 (默认 3600s) | `/mute 123456 600` |
| `/unmute` | `<user_id>` | 解除禁言 | `/unmute 123456` |
| `/unban` | `<user_id>` | 解除封禁 | `/unban 123456` |
| `/warn` | `<user_id>` | 发出警告 (累积触发升级) | `/warn 123456` |
| `/promote` | `<user_id>` | 提升为管理员 | `/promote 123456` |
| `/demote` | `<user_id>` | 降级管理员 | `/demote 123456` |
| `/lock` | `<type>` | 锁定消息类型 | `/lock photo` |
| `/unlock` | `<type>` | 解锁消息类型 | `/unlock photo` |
| `/locks` | — | 查看当前锁定列表 | `/locks` |

**可锁定类型**: `photo`, `video`, `audio`, `document`, `sticker`, `animation`, `voice`, `forward`, `contact`, `location`, `poll`, `game`

**权限**: 管理员命令需要 `is_admin=true`。非管理员发出的管理命令被静默忽略。

**Discord 双前缀**: Discord 平台同时支持 `/` 和 `!` 前缀 (如 `!ban 123456`)。

## 5. 链上群配置字段

群主通过链上治理交易配置，GroupRobot 自动同步到本地:

| 分类 | 字段 | 类型 | 默认值 | 说明 |
|------|------|------|--------|------|
| **基础** | `node_requirement` | u8 | — | 最低节点要求 |
| | `version` | u32 | — | 配置版本号 (变化时重建 RuleEngine) |
| **防刷屏** | `anti_flood_enabled` | bool | false | 启用防刷屏 |
| | `flood_limit` | u16 | — | 窗口内消息上限 |
| **重复检测** | `anti_duplicate_enabled` | bool | false | 启用重复消息检测 |
| | `duplicate_window_secs` | u64 | 300 | 检测窗口 (秒) |
| | `duplicate_threshold` | u16 | 3 | 重复次数阈值 |
| **内容过滤** | `stop_words` | String | "" | 停用词 (逗号分隔) |
| | `max_emoji` | u16 | 0 | 最大 Emoji 数 (0=不限) |
| | `max_links` | u16 | 0 | 最大链接数 (0=不限) |
| **警告** | `warn_limit` | u8 | — | 警告累积上限 (0=禁用) |
| | `warn_action` | u8 | 0 | 达到上限动作: 0=Mute, 1=Kick, 2=Ban |
| | `warn_mute_duration` | u64 | 3600 | Mute 时长 (秒) |
| **欢迎** | `welcome_enabled` | bool | false | 启用欢迎消息 |
| | `welcome_template` | String | "" | 欢迎模板 (`{user}`, `{group}`) |
| | `goodbye_template` | String | "" | 告别模板 |
| **反垃圾** | `spam_samples` | String | "" | Spam 样本 (换行分隔) |
| | `similarity_threshold` | u8 | 70 | TF-IDF 相似度阈值 (0-100) |
| **审计** | `log_channel_id` | String | "" | 审计日志转发频道 ID |
| **CAPTCHA** | `captcha_enabled` | bool | false | 启用 CAPTCHA |
| | `captcha_timeout_secs` | u64 | 120 | CAPTCHA 超时 (秒) |
| **反钓鱼** | `antiphishing_enabled` | bool | false | 启用反钓鱼 |
| | `bayes_threshold` | u8 | 80 | 贝叶斯分类阈值 |
| **自定义** | `custom_commands_csv` | String | "" | 自定义命令 (`trigger\|type\|response`) |
| | `locked_types_csv` | String | "" | 锁定消息类型 (逗号分隔) |

## 6. 部署示例

### 6.1 最小 Telegram 部署

```bash
export PLATFORM=telegram
export BOT_TOKEN=123456:ABC-DEF1234...
export WEBHOOK_URL=https://bot.example.com/webhook
export WEBHOOK_SECRET=my-secret-token
export CHAIN_RPC=ws://localhost:9944
export CHAIN_SIGNER_SEED=0xabcdef...
export DATA_DIR=./data
export TEE_MODE=auto
export LOG_LEVEL=info

./grouprobot
```

### 6.2 双平台 + 硬件 TEE

```bash
export PLATFORM=both
export BOT_ID_HASH=0xabcdef...
export DISCORD_APPLICATION_ID=123456789
export WEBHOOK_URL=https://bot.example.com/webhook
export WEBHOOK_SECRET=my-secret-token
export CHAIN_RPC=wss://chain.example.com:443
export CHAIN_SIGNER_SEED=0xabcdef...
export DATA_DIR=/data/grouprobot
export TEE_MODE=hardware
export VAULT_MODE=inprocess
export SHAMIR_THRESHOLD=2
export PEER_ENDPOINTS=https://peer1:8443,https://peer2:8443,https://peer3:8443
export CEREMONY_PORT=8443
export METRICS_ENABLED=true
export LOG_LEVEL=info

./grouprobot
```

### 6.3 Docker Compose (参考)

```yaml
services:
  grouprobot:
    image: grouprobot:latest
    ports:
      - "3000:3000"
    environment:
      PLATFORM: telegram
      WEBHOOK_URL: https://bot.example.com/webhook
      WEBHOOK_SECRET: ${WEBHOOK_SECRET}
      CHAIN_RPC: ws://chain:9944
      DATA_DIR: /data
      TEE_MODE: auto
      METRICS_ENABLED: "true"
    volumes:
      - grouprobot-data:/data
    # TDX 设备映射 (硬件模式)
    devices:
      - /dev/attestation:/dev/attestation
    security_opt:
      - no-new-privileges:true
    read_only: true
    tmpfs:
      - /tmp
```

## 7. 相关文档

- [架构概览](ARCHITECTURE.md) — 整体架构、目录结构、启动流程
- [平台适配层](PLATFORM_ADAPTERS.md) — Telegram / Discord 适配器与执行器
- [规则引擎](RULE_ENGINE.md) — 15 种规则 + AutoMod + 自定义命令
- [链上集成](CHAIN_INTEGRATION.md) — subxt 查询/交易 + 配置同步
- [TEE 安全](TEE_SECURITY.md) — Enclave + Vault + Shamir + DCAP
