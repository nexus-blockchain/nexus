# GroupRobot

GroupRobot 离链 TEE 执行程序 — 在 TDX+SGX 可信执行环境中运行的群管理机器人，与链上 `grouprobot-*` Pallet (index 150-153) 通过 subxt 交互。

## 架构

```
┌─────────────────────────────────────────────────────┐
│                  HTTP Entry (Axum)                   │
│  POST /webhook   GET /health   GET /v1/status       │
│  GET /metrics    POST /ceremony/*                   │
├─────────────────────────────────────────────────────┤
│              Message Processing Layer               │
│  Router → RuleEngine → [Flood│Blacklist│Command│    │
│                          Join│Default]              │
├───────────────────┬─────────────────────────────────┤
│  Platform Layer   │       Chain Interaction          │
│  ┌──────────────┐ │  ┌─────────────────────────┐    │
│  │  Telegram    │ │  │  subxt dynamic queries  │    │
│  │  Adapter     │ │  │  + transactions         │    │
│  │  Executor    │ │  │  ActionLogBatcher (6s)  │    │
│  ├──────────────┤ │  └─────────────────────────┘    │
│  │  Discord     │ │                                 │
│  │  Adapter     │ │       TEE Security Layer        │
│  │  Executor    │ │  ┌─────────────────────────┐    │
│  │  Gateway     │ │  │  EnclaveBridge (Ed25519) │    │
│  └──────────────┘ │  │  SealedStorage (AES-GCM) │    │
│                   │  │  TokenVault (Zeroizing)  │    │
│                   │  │  Attestor (TDX+SGX)     │    │
│                   │  │  Shamir / Ceremony      │    │
│                   │  │  VaultIPC (加密 Unix)    │    │
│                   │  │  MemSecurity (mlock)    │    │
│                   │  └─────────────────────────┘    │
├───────────────────┴─────────────────────────────────┤
│                Infrastructure                       │
│  Metrics (Prometheus) │ RateLimiter │ LocalStore    │
│  ConfigManager        │ GroupConfig chain sync      │
└─────────────────────────────────────────────────────┘
```

## 目录结构

```
grouprobot/
├── Cargo.toml
├── .env.example
├── README.md
├── gramine/                        # SGX Gramine 部署
│   ├── token-vault.manifest.template
│   └── README.md
└── src/                            # 47 源文件
    ├── main.rs                     # 入口 + AppState + 任务编排
    ├── config.rs                   # 环境变量配置 (敏感字段 Debug 脱敏)
    ├── error.rs                    # 统一错误类型 (thiserror)
    ├── webhook.rs                  # HTTP 入口 (Secret 验证/限流/去重/路由)
    │
    ├── chain/                      # Substrate 链客户端 (subxt)
    │   ├── client.rs               #   ChainClient 连接 + 签名者管理
    │   ├── queries.rs              #   链上查询 (Bot/TEE/序列/社区配置)
    │   ├── transactions.rs         #   链上交易 + ActionLogBatcher
    │   └── types.rs                #   链交互类型 (BotInfoCache 等)
    │
    ├── tee/                        # TEE 安全层 (13 文件)
    │   ├── enclave_bridge.rs       #   SGX/Software Enclave 桥接 (Ed25519)
    │   ├── sealed_storage.rs       #   AES-256-GCM 密封存储
    │   ├── key_manager.rs          #   Ed25519 密钥 + 原子序列号
    │   ├── attestor.rs             #   TDX+SGX 双 Quote 证明 (24h 有效)
    │   ├── token_vault.rs          #   Token 保险库 (Zeroizing + mlock)
    │   ├── shamir.rs               #   GF(256) Shamir K-of-N 秘密分片
    │   ├── ceremony.rs             #   RA-TLS 仪式 (Share 分发/接收)
    │   ├── share_recovery.rs       #   Token 恢复 (Share→Peer→Env fallback)
    │   ├── peer_client.rs          #   Peer TEE 节点 HTTP 通信
    │   ├── vault_ipc.rs            #   IPC 加密协议 (AES-GCM + 方向前缀)
    │   ├── vault_server.rs         #   Vault Unix socket 服务端
    │   ├── vault_client.rs         #   Vault Unix socket 客户端
    │   └── mem_security.rs         #   内存加固 (core dump/mlock/dumpable)
    │
    ├── platform/                   # 多平台适配层
    │   ├── mod.rs                  #   PlatformEvent + PlatformAdapter trait
    │   │                           #   PlatformExecutor trait + ActionType
    │   ├── telegram/
    │   │   ├── adapter.rs          #   TG Update → PlatformEvent
    │   │   └── executor.rs         #   TG Bot API 执行器
    │   └── discord/
    │       ├── adapter.rs          #   DC Gateway Event → PlatformEvent
    │       ├── executor.rs         #   DC REST API 执行器
    │       └── gateway.rs          #   DC WebSocket (IDENTIFY/RESUME/心跳)
    │
    ├── processing/                 # 消息处理层
    │   ├── router.rs               #   MessageRouter (事件→规则→执行→日志)
    │   ├── rule_engine.rs          #   可插拔规则链
    │   ├── action.rs               #   ActionDecision → ExecuteAction
    │   ├── normalizer.rs           #   SHA256 哈希工具
    │   └── rules/
    │       ├── flood.rs            #   防刷屏 (滑动窗口计数)
    │       ├── blacklist.rs        #   关键词黑名单 (正则)
    │       ├── command.rs          #   命令解析 + 管理员权限校验
    │       ├── join.rs             #   加群请求审批
    │       └── default.rs          #   默认放行
    │
    └── infra/                      # 基础设施
        ├── metrics.rs              #   Prometheus /metrics (OpenMetrics)
        ├── rate_limiter.rs         #   滑动窗口限流器
        ├── local_store.rs          #   DashMap 计数器 + 消息指纹去重
        └── group_config.rs         #   群配置缓存 + 链上定期同步
```

## 模块概览

| 模块 | 文件数 | 说明 |
|------|:------:|------|
| `chain/` | 4 | Substrate 链客户端 (subxt)、查询、交易、ActionLogBatcher 批量提交 |
| `tee/` | 13 | Enclave 桥接、密封存储、密钥管理、双证明、Shamir、仪式、Token 保险库、IPC |
| `platform/` | 6 | PlatformAdapter/PlatformExecutor trait、Telegram 适配+执行、Discord 适配+执行+Gateway |
| `processing/` | 8 | 消息路由、规则引擎 (5 条可插拔规则链)、动作决策、归一化 |
| `infra/` | 4 | Prometheus 指标、限流器、本地缓存 (DashMap)、群配置链上同步 |
| 根 | 4 | main.rs (启动编排)、config.rs (环境变量)、error.rs (统一错误)、webhook.rs (HTTP 入口) |

## 快速开始

```bash
# 复制环境配置
cp .env.example .env
# 编辑 .env 填入 BOT_TOKEN 等

# 编译
cargo build --release

# 运行
cargo run --release
```

## 链上交互

与 `grouprobot-*` Pallet (index 150-153) 通过 subxt 动态调用交互:

| Pallet | Index | 功能 |
|--------|:-----:|------|
| **GroupRobotRegistry** | 150 | Bot 注册、TEE 证明提交/刷新、MRTD/MRENCLAVE 白名单 |
| **GroupRobotConsensus** | 151 | 序列去重、动作日志提交 (单条/批量)、订阅/奖励 |
| **GroupRobotCommunity** | 152 | 群规则配置查询、CAS 乐观锁更新 |
| **GroupRobotCeremony** | 153 | RA-TLS 仪式记录、Enclave 审批 |

## TEE 模式

| 模式 | 说明 |
|------|------|
| `auto` | 自动检测 `/dev/attestation/quote` 决定硬件/软件 |
| `hardware` | 强制 TDX+SGX 硬件模式 |
| `software` | 纯软件模拟 (开发/测试用) |

## Vault 模式

Token 保护的三种进程隔离模式:

| 模式 | 说明 |
|------|------|
| `inprocess` | 默认。Token 在当前进程内 (Zeroizing + mlock) |
| `spawn` | 恢复 Token → 启动内嵌 Vault 服务端 → 加密 IPC 连接 |
| `connect` | 连接外部 Vault 进程 (Gramine SGX Enclave, 推荐生产环境) |

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PLATFORM` | `telegram` | 平台模式: `telegram` / `discord` / `both` |
| `BOT_TOKEN` | — | Telegram Bot Token (用于派生 bot_id_hash) |
| `BOT_ID_HASH` | 自动派生 | 32 字节 hex (优先于 BOT_TOKEN 派生) |
| `WEBHOOK_PORT` | `3000` | HTTP 服务端口 |
| `WEBHOOK_URL` | — | Telegram Webhook 注册 URL |
| `WEBHOOK_SECRET` | — | Webhook Secret Token 验证 |
| `DISCORD_BOT_TOKEN` | — | Discord Bot Token |
| `DISCORD_APPLICATION_ID` | — | Discord 应用 ID |
| `DISCORD_INTENTS` | `33281` | Discord Gateway Intents |
| `CHAIN_RPC` | `ws://127.0.0.1:9944` | Substrate RPC 端点 |
| `CHAIN_SIGNER_SEED` | 自动生成 | sr25519 签名密钥种子 (启动后自动从 environ 清除) |
| `TEE_MODE` | `auto` | TEE 模式: `auto` / `hardware` / `software` |
| `DATA_DIR` | `./data` | 数据目录 (密封密钥/share/序列号) |
| `VAULT_MODE` | `inprocess` | Token 保险库模式: `inprocess` / `spawn` / `connect` |
| `VAULT_SOCKET` | 自动 | Vault Unix socket 路径 |
| `SHAMIR_THRESHOLD` | `1` | Shamir 门限 K |
| `PEER_ENDPOINTS` | — | Peer TEE 节点端点 (逗号分隔) |
| `CEREMONY_PORT` | `0` | Ceremony 独立端口 (0 = 共用主端口) |
| `WEBHOOK_RATE_LIMIT` | `200` | Webhook 每分钟最大请求数 |
| `EXECUTE_RATE_LIMIT` | `100` | 执行动作每分钟最大数 |
| `CHAIN_LOG_BATCH_INTERVAL` | `6` | 链上日志批量提交间隔 (秒) |
| `CHAIN_LOG_BATCH_SIZE` | `50` | 链上日志批量提交大小 |
| `METRICS_ENABLED` | `true` | 是否启用 Prometheus /metrics |
| `RUST_LOG` | `grouprobot=info` | 日志级别 |

## HTTP API

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/webhook` | Telegram Webhook 入口 (Secret 验证 + 限流 + 去重) |
| `GET` | `/health` | 健康检查 (status/version/uptime/platform/tee_mode) |
| `GET` | `/v1/status` | 详细状态 (bot_id_hash/public_key/缓存统计) |
| `GET` | `/metrics` | Prometheus 指标 (OpenMetrics 格式) |
| `POST` | `/ceremony/*` | RA-TLS 仪式 Share 接收/服务 |

## 安全特性

- **jemalloc zero-on-free**: 全局分配器, 堆内存释放后自动清零
- **内存加固**: `RLIMIT_CORE=0` + `PR_SET_DUMPABLE=0` (禁用 core dump)
- **TokenVault**: 所有 token 使用 `Zeroizing<String>` + mlock 锁定内存页
- **Vault IPC**: Unix socket 0o600 权限 + AES-256-GCM 加密通道 + 方向前缀 + 单调 nonce
- **密封存储**: AES-256-GCM (Software) / SGX Seal Key (Hardware)
- **Debug 脱敏**: BotConfig 手动实现 Debug, 敏感字段显示 `<REDACTED>`
- **环境变量清除**: BOT_TOKEN / CHAIN_SIGNER_SEED 使用后立即 `remove_var`
- **Shamir 恢复**: 本地 sealed share → K-1 peer RA-TLS 收集 → 环境变量 fallback (auto-seal)
- **证明刷新**: 24h 有效期, 23h 自动刷新 (1h 安全边际)

## 测试

```bash
cargo test
# 131 tests passing
```

## SGX 部署 (Gramine)

参见 [gramine/README.md](gramine/README.md)。

```bash
# 编译 + 复制到 gramine 目录
cargo build --release
cp target/release/grouprobot gramine/

# 生成 manifest + 签名
cd gramine
gramine-manifest -Dlog_level=error token-vault.manifest.template token-vault.manifest
gramine-sgx-sign --manifest token-vault.manifest --output token-vault.manifest.sgx
```

## 许可证

Apache-2.0
