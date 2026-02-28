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
| `SEAL_POLICY` | `dual` | 密封策略: `mrenclave` / `mrsigner` / `dual` (见下方说明) |
| `MIGRATION_SOURCE` | — | 旧版本 Enclave 端点 (Migration Ceremony, 例: `https://old:3000`) |
| `VAULT_MODE` | `inprocess` | Token 保险库模式: `inprocess` / `spawn` / `connect` |
| `VAULT_SOCKET` | 自动 | Vault Unix socket 路径 |
| `SHAMIR_THRESHOLD` | `2` | Shamir 门限 K (默认 K=2, N=4) |
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

## 密封策略 (SEAL_POLICY)

控制密封存储的密钥绑定方式, 直接影响升级兼容性:

| 策略 | 密钥绑定 | 升级兼容 | 安全性 | 适用场景 |
|------|----------|----------|--------|----------|
| `mrenclave` | 代码度量 (MRENCLAVE) | 代码变更后旧文件不可读 | 最高 | 安全性优先, 无跨版本需求 |
| `mrsigner` | 签名者 (MRSIGNER) | Gramine 签名密钥不变即兼容 | 高 | 频繁升级, 单签名密钥管控 |
| `dual` | 写 MRSIGNER / 读兼容两种 | 跨版本兼容 + V0 向后兼容 | 高 | **推荐生产环境** |

密封文件格式:
- V0 (旧): `[nonce:12][ciphertext]` — 仅 MRENCLAVE 密钥
- V1 (新): `[0x01][key_type:1][nonce:12][ciphertext]` — 标识密钥类型

启动时自动将 V0 文件迁移到 V1 (原文件备份为 `.v0.bak`)。

## 5 级恢复链

启动时按优先级尝试恢复 Token + 签名密钥:

| 级别 | 来源 | 条件 | 场景 |
|------|------|------|------|
| 1 | 本地密封 share (K=1) | 本地 `.sealed` 文件可解密 | 常规重启, MRSIGNER 跨版本 |
| 2 | 本地 + Peer share (K>1) | peer 端点可达, K 个 share 收集成功 | 滚动升级, 多节点部署 |
| 3 | Migration Ceremony | `MIGRATION_SOURCE` 已配置, 旧版本运行中 | Gramine 签名密钥变更 |
| 4 | RA-TLS Provision | DApp 管理员通过 `/provision/inject-token` 注入 | 管理员干预 |
| 5 | 环境变量 fallback | `BOT_TOKEN` 环境变量存在 | 紧急恢复, 首次部署 |

### Migration Ceremony 协议

用于 Gramine 签名密钥变更等 MRSIGNER 也变的场景:

```
旧 Enclave (运行中) ←── HTTP POST /migration/export-secret ──── 新 Enclave (启动中)
    │                                                              │
    │  1. 解密本地 share → 恢复 secret                              │
    │  2. ECDH(ephemeral_sk, new_pk) 加密 → 响应                   │
    │                                                              │
    │  ──────── { encrypted_secret, ephemeral_pk } ──────────►    │
    │                                                              │
    │                              3. ECDH 解密 → 获得 secret      │
    │                              4. Auto-seal → 本地密封保存      │
```

安全约束:
- 单次使用: 导出后标记 `exported`, 拒绝重复导出
- ECDH 端到端: secret 明文仅在 TEE 内存中
- 身份保持: 迁移的 Ed25519 签名密钥保持链上身份不变

## 测试

```bash
cargo test
# 398 tests passing
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

## 升级 SOP

### 场景 A: 小版本升级 (同 Gramine 签名密钥)

MRSIGNER 不变, `SEAL_POLICY=dual` 时 Level 1 直接解密旧密封文件:

```bash
# 1. 编译新版本
cargo build --release

# 2. 提取新 MRTD + 链上预批准
./scripts/extract-mrtd.sh
substrate-cli tx groupRobotRegistry approve_mrtd --mrtd <NEW_MRTD>

# 3. 滚动升级 (自动逐台)
./scripts/rolling-upgrade.sh upgrade-config.json

# 4. 验证: 所有节点 public_key 不变, 版本更新
curl http://node:3000/v1/status
```

### 场景 B: 多节点滚动升级 (K-of-N)

约束: 同时停止的节点数 ≤ N - K

```bash
# 1. 链上预批准新 MRTD (同上)
# 2. 编写 upgrade-config.json:
cat > upgrade-config.json <<'EOF'
{
  "nodes": [
    { "name": "node-a", "host": "10.0.1.1", "port": 3000, "ssh": "user@10.0.1.1" },
    { "name": "node-b", "host": "10.0.1.2", "port": 3000, "ssh": "user@10.0.1.2" },
    { "name": "node-c", "host": "10.0.1.3", "port": 3000, "ssh": "user@10.0.1.3" },
    { "name": "node-d", "host": "10.0.1.4", "port": 3000, "ssh": "user@10.0.1.4" }
  ],
  "shamir_k": 2,
  "binary_path": "/opt/grouprobot/grouprobot",
  "new_binary": "./target/release/grouprobot",
  "health_timeout": 60
}
EOF

# 3. 执行
./scripts/rolling-upgrade.sh upgrade-config.json
```

### 场景 C: Gramine 签名密钥变更 (Migration Ceremony)

MRSIGNER 也变, 需通过旧版本传递密钥:

```bash
# 1. 保持旧版本运行
# 2. 启动新版本, 指定 MIGRATION_SOURCE:
MIGRATION_SOURCE=https://old-node:3000 \
SEAL_POLICY=dual \
./grouprobot

# 新版本启动时自动:
#   Level 1: 本地 share 解密失败 (MRSIGNER 变了)
#   Level 3: 向旧版本发起 Migration Ceremony
#   → ECDH 获取 secret → auto-seal → 身份保持
#
# 3. 验证后停止旧版本
```

### CI/CD 集成

```yaml
# GitHub Actions 示例
- name: Build SGX Enclave
  run: |
    cargo build --release
    cp target/release/grouprobot gramine/
    cd gramine
    gramine-manifest -Dlog_level=error token-vault.manifest.template token-vault.manifest
    gramine-sgx-sign --manifest token-vault.manifest --output token-vault.manifest.sgx

- name: Extract MRTD
  run: ./scripts/extract-mrtd.sh

- name: Upload measurements
  uses: actions/upload-artifact@v4
  with:
    name: mrtd-measurements
    path: gramine/mrtd-measurements.json
```

### 升级验证清单

| 检查项 | 命令 | 期望 |
|--------|------|------|
| 节点健康 | `curl /health` | `{"status": "ok"}` |
| 版本更新 | `curl /health` | `.version` = 新版本 |
| 身份保持 | `curl /v1/status` | `.public_key` 不变 |
| 密封兼容 | `curl /v1/status` | `.upgrade_compat.cross_version_seal` = true |
| 链上证明 | `substrate-cli query attestations` | `quote_verified=true`, `mrtd=新值` |

## 许可证

Apache-2.0
