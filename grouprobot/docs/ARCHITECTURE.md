# GroupRobot 架构概览

> 版本: 0.1.0 | 最后更新: 2026-02-23

## 1. 项目定位

GroupRobot 是一个运行在 **TEE (Trusted Execution Environment)** 内的离链群组管理机器人。它通过 Webhook / Gateway 接收 Telegram 和 Discord 的实时事件，在 TDX/SGX 可信执行环境中处理消息、执行管理动作，并将操作日志签名后批量提交到 Nexus 区块链，实现**可审计、防篡改、去中心化**的群组治理。

```
┌─────────────────────────────────────────────────────────────────┐
│                       GroupRobot 进程                            │
│                                                                 │
│  ┌───────────┐  ┌──────────────┐  ┌──────────┐  ┌───────────┐  │
│  │  Webhook   │  │  规则引擎     │  │ 平台执行器│  │ 链客户端   │  │
│  │  (Axum)    │→ │  (RuleEngine)│→ │ (TG/DC)  │  │ (subxt)   │  │
│  └───────────┘  └──────────────┘  └──────────┘  └───────────┘  │
│         ↑              ↑                              ↑         │
│  ┌──────┴──────┐ ┌─────┴──────┐         ┌────────────┴───────┐ │
│  │ 平台适配器   │ │ 链上群配置  │         │ TEE 安全层          │ │
│  │ (TG/DC)     │ │ (ConfigMgr)│         │ (Enclave+Vault+    │ │
│  └─────────────┘ └────────────┘         │  Shamir+DCAP)      │ │
│                                         └────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
         ↕ Webhook/Gateway           ↕ subxt RPC
  ┌─────────────┐             ┌─────────────────┐
  │ Telegram    │             │ Nexus Blockchain │
  │ Discord     │             │ (Substrate)      │
  └─────────────┘             └─────────────────┘
```

## 2. 目录结构

```
grouprobot/src/
├── main.rs              # 统一入口: 初始化 + HTTP 服务器 + 后台任务
├── config.rs            # 环境变量配置 (BotConfig)
├── error.rs             # 统一错误类型 (BotError / BotResult)
├── webhook.rs           # Axum HTTP 处理器 (/webhook, /health, /v1/status)
│
├── platform/            # 平台适配层 (→ PLATFORM_ADAPTERS.md)
│   ├── mod.rs           #   抽象接口: PlatformAdapter, PlatformExecutor
│   ├── telegram/        #   Telegram: Adapter + Executor (20 API 方法)
│   └── discord/         #   Discord:  Adapter + Executor + Gateway
│
├── processing/          # 消息处理层 (→ RULE_ENGINE.md)
│   ├── router.rs        #   核心流水线: 规则评估 → 执行 → 签名 → 审计
│   ├── rule_engine.rs   #   可插拔规则引擎 (15 种规则)
│   ├── action.rs        #   ActionDecision / RuleDecision 数据结构
│   ├── rules/           #   15 个规则实现 (详见 RULE_ENGINE.md)
│   ├── automod/         #   AutoMod 三段式引擎 (Trigger→Condition→Effect)
│   ├── captcha.rs       #   CAPTCHA 验证 (数学题 + RA-TLS 预留)
│   ├── custom_commands.rs  # 自定义命令/关键词过滤器
│   ├── audit_logger.rs  #   审计日志 (格式化 + 频道转发)
│   └── normalizer.rs    #   隐私哈希 (group_id / user_id → SHA-256)
│
├── chain/               # 链上集成层 (→ CHAIN_INTEGRATION.md)
│   ├── client.rs        #   ChainClient: subxt 动态 API 连接
│   ├── queries.rs       #   链上查询: Bot 信息、TEE 状态、群配置、Nonce
│   ├── transactions.rs  #   链上交易: 证明提交、日志批量上链
│   └── types.rs         #   数据类型: ChainCommunityConfig 等
│
├── tee/                 # TEE 安全层 (→ TEE_SECURITY.md)
│   ├── enclave_bridge.rs   # Enclave 桥接 (Hardware/Software 双模式)
│   ├── sealed_storage.rs   # AES-256-GCM 密封存储
│   ├── key_manager.rs      # Ed25519 密钥管理 + 序列号管理
│   ├── attestor.rs         # TDX+SGX 双证明生成
│   ├── dcap_verify.rs      # DCAP ECDSA 本地验证
│   ├── shamir.rs           # Shamir 秘密共享 + X25519 ECDH 加密
│   ├── ceremony.rs         # Share 分发/收集仪式 (HTTP + AttestationGuard)
│   ├── share_recovery.rs   # Token 统一恢复 (本地→Peer→ENV fallback)
│   ├── token_vault.rs      # TokenVault (Zeroizing 内存安全)
│   ├── vault_client.rs     # Vault IPC 客户端 (加密通道)
│   ├── vault_server.rs     # Vault IPC 服务端 (独立进程)
│   ├── vault_ipc.rs        # IPC 协议 + AES-GCM 加密帧
│   ├── peer_client.rs      # Peer TEE 节点通信 (RA-TLS)
│   ├── ra_tls.rs           # RA-TLS Token 注入端点
│   └── mem_security.rs     # 进程内存加固 (core dump 禁用, mlock)
│
└── infra/               # 基础设施
    ├── local_store.rs   #   DashMap: 防刷屏计数、消息去重、指纹
    ├── group_config.rs  #   ConfigManager: 链上群配置缓存 + 同步循环
    ├── rate_limiter.rs  #   令牌桶限流 (全局 + per-group)
    └── metrics.rs       #   Prometheus 指标 (消息数/动作数/链上TX)
```

## 3. 核心处理流水线

一条消息从接收到处理的完整路径:

```
Telegram Webhook POST /webhook
        │
        ▼
  ① Secret 验证 (X-Telegram-Bot-Api-Secret-Token)
        │
        ▼
  ② 全局限流 (RateLimiter::allow)
        │
        ▼
  ③ 平台适配器解析 (TelegramAdapter::parse_event → PlatformEvent)
        │
        ▼
  ④ per-group 限流 (RateLimiter::allow_for)
        │
        ▼
  ⑤ 指纹去重 (LocalStore::check_fingerprint, 300s 窗口)
        │
        ▼
  ⑥ 上下文提取 (extract_context → MessageContext)
        │
        ▼
  ⑦ 管理员身份查询 (仅命令消息, getChatMember)
        │
        ▼
  ⑧ 规则引擎评估 (RuleEngine::evaluate → RuleDecision)
        │
        ├─ 无动作 → 返回 200 OK
        │
        ▼ 有动作
  ⑨ 平台执行器执行 (TelegramExecutor::execute → ExecutionReceipt)
        │
        ▼
  ⑩ Ed25519 签名 + 序列号 → PendingActionLog 入队
        │
        ▼
  ⑪ 审计日志记录 + 可选频道转发
        │
        ▼
  ⑫ 链上序列号标记 (异步, mark_sequence_processed)
        │
        ▼
  ⑬ ActionLogBatcher 批量提交链上 (configurable interval/size)
```

## 4. 启动流程

`main()` 按顺序初始化以下组件:

| 顺序 | 组件 | 说明 |
|------|------|------|
| 1 | `mem_security::harden_process_memory` | 禁用 core dump、清除 dumpable、提升 mlock 限制 |
| 2 | `BotConfig::from_env` | 从环境变量加载配置 |
| 3 | `EnclaveBridge::init` | 初始化 TEE Enclave (自动检测 Hardware/Software) |
| 4 | `KeyManager` + `SequenceManager` | Ed25519 密钥 + 持久化序列号 |
| 5 | `Attestor::generate_attestation` | 生成 TDX+SGX 双证明 |
| 6 | `share_recovery::recover_token` | Token 恢复 (本地 Shamir → Peer 收集 → ENV fallback) |
| 7 | `TokenVault` (3 种模式) | inprocess / spawn / connect |
| 8 | `TelegramExecutor` / `DiscordExecutor` | 平台执行器 |
| 9 | `RuleEngine` + `MessageRouter` | 规则引擎 + 消息路由 |
| 10 | 后台任务 | 定时清理(60s)、Webhook 注册、Discord Gateway |
| 11 | `ChainClient::connect` | 链客户端 (后台异步) |
| 12 | `ConfigManager::sync_loop` | 链上群配置定时同步 |
| 13 | `ActionLogBatcher::run` | 动作日志批量提交 |
| 14 | TEE 证明 24h 刷新循环 | Hardware: nonce 防重放; Software: 直接刷新 |
| 15 | Axum HTTP 服务器 | /webhook + /health + /v1/status + /metrics + /ceremony + /provision |

## 5. 数据流分类

### 5.1 热数据 (进程内存, DashMap)

| 存储 | 用途 | 生命周期 |
|------|------|---------|
| `LocalStore` 计数器 | 防刷屏频率统计 | 自动过期 (TTL) |
| `LocalStore` 指纹 | 消息去重 | 300s |
| `ConfigManager` 缓存 | 群配置缓存 | 30min + 链上同步 |
| `WarnTracker` 计数 | 累积警告次数 | 进程生命周期 |
| `CustomCommandManager` | 自定义命令 | 进程生命周期 |

### 5.2 持久化数据 (TEE 密封存储)

| 文件 | 用途 | 加密方式 |
|------|------|---------|
| `enclave_ed25519.sealed` | Ed25519 签名密钥 | AES-256-GCM |
| `shamir_share.sealed` | Shamir secret share | AES-256-GCM |
| `ipc_key.sealed` | Vault IPC 加密密钥 | AES-256-GCM |
| `sequence.dat` | 序列号计数器 | 明文 (非敏感) |

### 5.3 链上数据 (Substrate Storage)

| Pallet | 存储项 | 说明 |
|--------|--------|------|
| `GroupRobotRegistry` | `Bots`, `Attestations` | Bot 注册、TEE 证明 |
| `GroupRobotConsensus` | `ProcessedSequences`, `EraRewards` | 序列号去重、Era 奖励 |
| `GroupRobotCommunity` | `CommunityConfigs` | 群管理规则配置 |

## 6. 安全架构

```
┌────────────────────────────────────────────────┐
│              TEE Enclave (TDX/SGX)              │
│                                                │
│  ┌─────────────┐  ┌──────────────────────────┐ │
│  │ Ed25519 密钥 │  │ TokenVault               │ │
│  │ (密封存储)   │  │ ├─ TG Bot Token (mlock)  │ │
│  │             │  │ ├─ DC Bot Token (mlock)  │ │
│  │             │  │ └─ Zeroizing<String>     │ │
│  └─────────────┘  └──────────────────────────┘ │
│                                                │
│  ┌────────────────────────────────────────────┐ │
│  │ Shamir Secret Sharing (K-of-N)             │ │
│  │ ├─ 本地 share (密封存储)                    │ │
│  │ ├─ Peer share (RA-TLS + ECDH 加密传输)     │ │
│  │ └─ ENV fallback (首次自动密封)              │ │
│  └────────────────────────────────────────────┘ │
│                                                │
│  ┌────────────────────────────────────────────┐ │
│  │ DCAP 远程证明                               │ │
│  │ ├─ TDX Quote v4 (MRTD + report_data)      │ │
│  │ ├─ ECDSA P-256 4 级签名链验证              │ │
│  │ └─ 24h 自动刷新 (nonce 防重放)             │ │
│  └────────────────────────────────────────────┘ │
└────────────────────────────────────────────────┘

防护措施:
  ✓ jemalloc zero-on-free (堆内存自动清零)
  ✓ core dump 禁用 (RLIMIT_CORE=0)
  ✓ PR_SET_DUMPABLE=0 (防止 /proc/pid/mem 读取)
  ✓ mlock 锁定 (防止 Token 被 swap 到磁盘)
  ✓ 环境变量清除 (CHAIN_SIGNER_SEED)
  ✓ AES-256-GCM 密封 (硬件绑定密钥)
```

## 7. 与链上 Pallet 的关系

GroupRobot 离链进程与以下链上 Pallet 交互 (pallet_index 150-153):

| Pallet | Index | 交互方式 | 用途 |
|--------|-------|---------|------|
| `pallet-grouprobot-registry` | 150 | 读取 + 写入 | Bot 注册、TEE 证明提交/刷新、DCAP 验证 |
| `pallet-grouprobot-consensus` | 151 | 读取 + 写入 | 序列号去重、动作日志上链、Era 奖励 |
| `pallet-grouprobot-community` | 152 | 读取 | 群规则配置同步 |
| `pallet-grouprobot-ceremony` | 153 | 读取 | Shamir 仪式协调 |

## 8. 相关文档

- [平台适配层](PLATFORM_ADAPTERS.md) — Telegram / Discord 适配器与执行器
- [规则引擎](RULE_ENGINE.md) — 15 种规则 + AutoMod + 自定义命令
- [链上集成](CHAIN_INTEGRATION.md) — subxt 查询/交易 + 配置同步
- [TEE 安全](TEE_SECURITY.md) — Enclave + Vault + Shamir + DCAP
- [API 参考](API_REFERENCE.md) — HTTP 端点 + 配置参数
