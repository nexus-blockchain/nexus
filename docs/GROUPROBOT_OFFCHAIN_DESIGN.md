# GroupRobot 离链程序开发设计文档

> 日期: 2026-02-22
> 版本: v1.0
> 状态: 设计阶段
>
> **关联文档:**
> - [GROUPROBOT_PALLET_DESIGN.md](./GROUPROBOT_PALLET_DESIGN.md) — 链上 Pallet 设计 (已实现)
> - [GROUPROBOT_OFFCHAIN_ANALYSIS.md](./GROUPROBOT_OFFCHAIN_ANALYSIS.md) — 离链可行性分析
> - [TEE_BLOCKCHAIN_COMPARISON.md](./TEE_BLOCKCHAIN_COMPARISON.md) — TEE 架构对比

---

## 目录

1. [项目概述](#1-项目概述)
2. [架构设计](#2-架构设计)
3. [目录结构](#3-目录结构)
4. [核心模块设计](#4-核心模块设计)
5. [链上交互层 — chain_client](#5-链上交互层--chain_client)
6. [TEE 安全层](#6-tee-安全层)
7. [平台适配层](#7-平台适配层)
8. [消息处理层](#8-消息处理层)
9. [运行时基础设施](#9-运行时基础设施)
10. [配置与环境变量](#10-配置与环境变量)
11. [部署架构](#11-部署架构)
12. [开发路线图](#12-开发路线图)
13. [测试策略](#13-测试策略)
14. [与 nexus-tee-bot 的差异对比](#14-与-nexus-tee-bot-的差异对比)

---

## 1. 项目概述

### 1.1 定位

`grouprobot` 是 GroupRobot 系统的**离链执行程序**，运行在 TDX+SGX TEE 环境中，负责：

- 接收社交平台消息（Telegram Webhook / Discord Gateway）
- 在 TEE 内进行消息处理和规则引擎判定
- 通过平台 API 执行管理动作（踢人/封禁/禁言等）
- 通过 `subxt` 与链上 `grouprobot-*` Pallet 交互（注册/证明/去重/日志）

### 1.2 与链上 Pallet 的关系

```
链上 Pallet (~20% 功能):                     离链 Bot (~80% 功能):
┌──────────────────────────┐                ┌──────────────────────────┐
│ grouprobot-registry      │◀── subxt ─────│ chain_client.rs          │
│ grouprobot-consensus     │◀── subxt ─────│   register / attest      │
│ grouprobot-community     │◀── subxt ─────│   dedup / log            │
│ grouprobot-ceremony      │◀── subxt ─────│   ceremony record        │
└──────────────────────────┘                ├──────────────────────────┤
                                            │ enclave_bridge.rs (SGX)  │
 确定性 · 公开透明 · 不可篡改                │ attestation.rs (TDX)     │
                                            │ signer.rs (Ed25519)      │
                                            │ shamir.rs (密钥分片)      │
                                            │ ceremony.rs (RA-TLS)     │
                                            ├──────────────────────────┤
                                            │ executor.rs (Telegram)   │
                                            │ discord_executor.rs (DC) │
                                            │ webhook.rs (HTTP 服务)    │
                                            │ local_processor.rs (规则) │
                                            └──────────────────────────┘
                                             隐私保护 · 毫秒响应 · TEE 信任
```

### 1.3 与 nexus-tee-bot 的关系

`grouprobot` 是 `nexus-tee-bot` 的**重构版本**，核心变更：

| 维度 | nexus-tee-bot | grouprobot |
|------|---------------|----------------|
| 链上目标 | `BotRegistry` / `BotConsensus` / `BotGroupMgmt` / `CeremonyAudit` | `GroupRobotRegistry` / `GroupRobotConsensus` / `GroupRobotCommunity` / `GroupRobotCeremony` |
| 链客户端 | 硬编码旧 Pallet 名 + 占位解码 | 完整 subxt metadata + 类型安全解码 |
| 模块化 | 单体 `main.rs` 所有初始化 | 分层架构: core / platform / chain / tee |
| 错误处理 | `anyhow` 贯穿 | 分层错误类型 + `thiserror` |
| 测试 | 199 tests (模块级) | 目标 200+ (模块 + 集成 + E2E) |
| 多平台 | TG + DC (后加) | TG + DC + 可扩展 `PlatformAdapter` trait |

---

## 2. 架构设计

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 5: HTTP 入口层                                                │
│  ┌─────────────────────┐ ┌──────────────────────┐                   │
│  │ Webhook Server      │ │ Discord Gateway       │                  │
│  │ (Axum: /webhook,    │ │ (WebSocket: IDENTIFY, │                  │
│  │  /health, /metrics) │ │  RESUME, heartbeat)   │                  │
│  └────────┬────────────┘ └──────────┬───────────┘                   │
├───────────┼─────────────────────────┼───────────────────────────────┤
│  Layer 4: 消息处理层                                                  │
│  ┌────────▼─────────────────────────▼───────────┐                   │
│  │ MessageRouter                                 │                  │
│  │  PlatformEvent → normalize → RuleEngine       │                  │
│  │  → ActionDecision → PlatformExecutor          │                  │
│  └──────────┬──────────────────────┬────────────┘                   │
│             │ 动作日志              │ 去重                           │
├─────────────┼──────────────────────┼────────────────────────────────┤
│  Layer 3: 链交互层                                                    │
│  ┌──────────▼──────────────────────▼────────────┐                   │
│  │ ChainClient (subxt)                           │                  │
│  │  • register_bot / submit_attestation          │                  │
│  │  • mark_sequence_processed                    │                  │
│  │  • submit_action_log / batch_submit_logs      │                  │
│  │  • record_ceremony                            │                  │
│  │  • query: bot_status, tee_status, config      │                  │
│  └──────────────────────┬───────────────────────┘                   │
├─────────────────────────┼───────────────────────────────────────────┤
│  Layer 2: TEE 安全层                                                  │
│  ┌──────────────────────┼───────────────────────┐                   │
│  │ EnclaveBridge (SGX)  │ Attestor (TDX+SGX)    │                  │
│  │ KeyManager (Ed25519) │ Shamir (GF(256))       │                  │
│  │ SealedStorage (AES)  │ CeremonyClient (RA-TLS)│                  │
│  └──────────────────────┘───────────────────────┘                   │
├─────────────────────────────────────────────────────────────────────┤
│  Layer 1: 运行时基础设施                                               │
│  ┌─────────────┐ ┌─────────────┐ ┌───────────┐ ┌────────────────┐  │
│  │ Config      │ │ Metrics     │ │ RateLimiter│ │ LocalStore     │  │
│  │ (.env)      │ │ (Prometheus)│ │ (滑动窗口)  │ │ (内存缓存)     │  │
│  └─────────────┘ └─────────────┘ └───────────┘ └────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 数据流

```
                    平台消息流 (毫秒级)
                    ═══════════════════

  Telegram ──POST /webhook──▶ webhook.rs ──▶ MessageRouter
                                                │
  Discord ──WebSocket──▶ gateway.rs ──────▶ MessageRouter
                                                │
                                        ┌───────▼────────┐
                                        │ normalize()    │
                                        │ PlatformEvent  │
                                        │ → InternalMsg  │
                                        └───────┬────────┘
                                                │
                                ┌───────────────▼────────────────┐
                                │ RuleEngine.evaluate()          │
                                │  FloodRule → BlacklistRule     │
                                │  → CommandRule → JoinRule      │
                                │  → DefaultRule                 │
                                └───────────────┬────────────────┘
                                                │ ActionDecision
                                ┌───────────────▼────────────────┐
                                │ PlatformExecutor.execute()     │
                                │  TG: sendMessage/ban/mute/...  │
                                │  DC: REST API + slash commands │
                                └───────────────┬────────────────┘
                                                │
                    链上交互流 (秒级, 异步)
                    ═══════════════════════
                                                │
                        ┌───────────────────────▼───────────────┐
                        │ ChainClient (后台任务)                  │
                        │  1. mark_sequence_processed (去重)     │
                        │  2. submit_action_log (存证)           │
                        │  3. refresh_attestation (24h 定时)     │
                        └───────────────────────────────────────┘
```

### 2.3 并发模型

```
tokio runtime (multi-thread)
├── main task: HTTP server (Axum)
│   ├── /webhook handler (Telegram)
│   ├── /health handler
│   └── /metrics handler (Prometheus)
├── discord_gateway task (WebSocket 长连接)
│   ├── heartbeat loop
│   └── event → mpsc::channel → event_consumer
├── event_consumer task (Discord 事件消费)
├── chain_task (链客户端 + 证明刷新)
│   ├── startup: connect → register → submit_attestation
│   ├── 23h loop: refresh_attestation
│   └── action_log_queue: mpsc → batch_submit (6s 间隔)
├── cleanup_task (60s 间隔: LocalStore GC)
└── ceremony_task (按需: RA-TLS 仪式)
```

---

## 3. 目录结构

```
grouprobot/
├── Cargo.toml
├── .env.example
├── Dockerfile
├── README.md
└── src/
    ├── main.rs                     ← 入口 + AppState + 任务编排
    │
    ├── config.rs                   ← 配置加载 (.env + 环境变量)
    ├── error.rs                    ← 统一错误类型 (thiserror)
    │
    │── chain/                      ← Layer 3: 链交互层
    │   ├── mod.rs                  ← re-exports
    │   ├── client.rs               ← ChainClient (subxt 连接 + 交易提交)
    │   ├── queries.rs              ← 链上查询 (bot_status, tee_status, config)
    │   ├── transactions.rs         ← 链上交易 (register, attest, dedup, log)
    │   └── types.rs                ← 链上类型映射 (BotInfo, Attestation 等)
    │
    │── tee/                        ← Layer 2: TEE 安全层
    │   ├── mod.rs
    │   ├── enclave_bridge.rs       ← SGX Enclave 桥接 (ecall/ocall)
    │   ├── sealed_storage.rs       ← AES-256-GCM 密封存储
    │   ├── key_manager.rs          ← Ed25519 密钥管理 (Enclave 内)
    │   ├── attestor.rs             ← TDX+SGX 双证明生成
    │   ├── shamir.rs               ← Shamir 秘密分享 (GF(256))
    │   └── ceremony.rs             ← RA-TLS 仪式客户端
    │
    │── platform/                   ← Layer 5+7: 平台适配
    │   ├── mod.rs                  ← PlatformAdapter trait + PlatformEvent
    │   ├── telegram/
    │   │   ├── mod.rs
    │   │   ├── adapter.rs          ← TelegramAdapter: webhook 解析
    │   │   ├── executor.rs         ← TelegramExecutor: Bot API 调用
    │   │   └── types.rs            ← Telegram 专有类型 (Update, Message)
    │   ├── discord/
    │   │   ├── mod.rs
    │   │   ├── adapter.rs          ← DiscordAdapter: Gateway 事件解析
    │   │   ├── executor.rs         ← DiscordExecutor: REST API 调用
    │   │   ├── gateway.rs          ← Discord Gateway (WebSocket)
    │   │   └── types.rs            ← Discord 专有类型
    │   └── matrix/                 ← (预留: Matrix 支持)
    │       └── mod.rs
    │
    │── processing/                 ← Layer 4: 消息处理
    │   ├── mod.rs
    │   ├── router.rs               ← MessageRouter: 平台事件 → 规则 → 执行
    │   ├── rule_engine.rs          ← RuleEngine: 可插拔规则链
    │   ├── rules/
    │   │   ├── mod.rs              ← Rule trait
    │   │   ├── flood.rs            ← FloodRule: 防刷屏
    │   │   ├── blacklist.rs        ← BlacklistRule: 关键词过滤
    │   │   ├── command.rs          ← CommandRule: /ban /mute 等指令
    │   │   ├── join.rs             ← JoinRequestRule: 入群审批
    │   │   └── default.rs          ← DefaultRule: 兜底
    │   ├── action.rs               ← ActionDecision + ActionType
    │   └── normalizer.rs           ← PlatformEvent → InternalMessage
    │
    │── infra/                      ← Layer 1: 基础设施
    │   ├── mod.rs
    │   ├── metrics.rs              ← Prometheus 指标 + /metrics 端点
    │   ├── rate_limiter.rs         ← 滑动窗口限流器
    │   ├── local_store.rs          ← 内存缓存 (flood 计数/指纹去重)
    │   └── group_config.rs         ← 群配置本地缓存 + 链上同步
    │
    └── webhook.rs                  ← HTTP 路由处理 (/webhook, /health, /execute)
```

---

## 4. 核心模块设计

### 4.1 统一错误类型 — `error.rs`

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    // 链交互
    #[error("Chain connection failed: {0}")]
    ChainConnection(String),
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),

    // TEE
    #[error("Enclave error: {0}")]
    EnclaveError(String),
    #[error("Attestation failed: {0}")]
    AttestationFailed(String),
    #[error("Ceremony error: {0}")]
    CeremonyError(String),

    // 平台
    #[error("Platform API error: {platform} - {message}")]
    PlatformApi { platform: String, message: String },
    #[error("Webhook validation failed: {0}")]
    WebhookValidation(String),

    // 通用
    #[error("Configuration error: {0}")]
    Config(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type BotResult<T> = Result<T, BotError>;
```

### 4.2 AppState — `main.rs`

```rust
pub struct AppState {
    // 配置
    pub config: BotConfig,

    // TEE 安全层
    pub enclave: Arc<EnclaveBridge>,
    pub attestor: Arc<Attestor>,
    pub key_manager: KeyManager,

    // 链交互层
    pub chain: Arc<ChainClient>,

    // 平台执行器
    pub telegram: Option<TelegramExecutor>,
    pub discord: Option<DiscordExecutor>,

    // 消息处理
    pub router: Arc<MessageRouter>,

    // 基础设施
    pub metrics: SharedMetrics,
    pub local_store: LocalStore,
    pub rate_limiter: RateLimiter,

    // 运行时状态
    pub sequence: SequenceManager,
    pub start_time: Instant,
}
```

---

## 5. 链上交互层 — chain_client

### 5.1 ChainClient 核心结构

```rust
// chain/client.rs
pub struct ChainClient {
    api: OnlineClient<SubstrateConfig>,
    signer: Keypair,
    /// 待提交的动作日志队列
    log_queue: mpsc::Sender<PendingActionLog>,
}
```

### 5.2 链上交易映射 — 新 Pallet 接口

| Bot 操作 | 链上 Pallet | Extrinsic | 时机 |
|---|---|---|---|
| 启动注册 | `GroupRobotRegistry` | `register_bot(bot_id_hash, public_key)` | 首次启动 |
| 提交双证明 | `GroupRobotRegistry` | `submit_attestation(bot_id_hash, tdx_quote_hash, sgx_quote_hash, mrtd, mrenclave)` | 首次启动 |
| 刷新证明 | `GroupRobotRegistry` | `refresh_attestation(bot_id_hash, tdx_quote_hash, sgx_quote_hash, mrtd, mrenclave)` | 23h 定时 |
| 消息去重 | `GroupRobotConsensus` | `mark_sequence_processed(bot_id_hash, sequence)` | 每条消息处理后 |
| 提交日志 | `GroupRobotCommunity` | `submit_action_log(community_id_hash, action_type, target_hash, sequence, message_hash, signature)` | 每次动作执行后 |
| 批量日志 | `GroupRobotCommunity` | `batch_submit_logs(community_id_hash, logs)` | 6s 批量间隔 |
| 记录仪式 | `GroupRobotCeremony` | `record_ceremony(ceremony_hash, mrenclave, k, n, bot_pk, participants)` | 仪式完成后 |

### 5.3 链上查询映射

| 查询 | Storage | 用途 |
|---|---|---|
| Bot 是否注册 | `GroupRobotRegistry::Bots` | 启动检查 |
| TEE 证明状态 | `GroupRobotRegistry::Attestations` | 证明到期检查 |
| 序列号是否已处理 | `GroupRobotConsensus::ProcessedSequences` | 去重前查询 |
| 群规则配置 | `GroupRobotCommunity::CommunityConfigs` | 本地配置同步 |
| 节点准入策略 | `GroupRobotCommunity::CommunityNodeRequirement` | 准入检查 |
| 仪式是否活跃 | `GroupRobotCeremony::ActiveCeremony` | 仪式状态检查 |
| MRTD 白名单 | `GroupRobotRegistry::ApprovedMrtd` | 证明提交前检查 |

### 5.4 交易批量提交策略

```rust
// chain/transactions.rs

/// 动作日志批量提交器
/// 策略: 收集 6 秒窗口内的日志，batch_submit_logs 一次性提交
pub struct ActionLogBatcher {
    queue: mpsc::Receiver<PendingActionLog>,
    client: Arc<ChainClient>,
    batch_interval: Duration,  // 6s (= 1 区块时间)
    max_batch_size: usize,     // 50
}

impl ActionLogBatcher {
    pub async fn run(mut self) {
        let mut interval = tokio::time::interval(self.batch_interval);
        let mut buffer: Vec<PendingActionLog> = Vec::new();

        loop {
            tokio::select! {
                Some(log) = self.queue.recv() => {
                    buffer.push(log);
                    if buffer.len() >= self.max_batch_size {
                        self.flush(&mut buffer).await;
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        self.flush(&mut buffer).await;
                    }
                }
            }
        }
    }

    async fn flush(&self, buffer: &mut Vec<PendingActionLog>) {
        // 按 community_id_hash 分组
        // 每组调用 batch_submit_logs
        // 失败的日志重入队列
    }
}
```

### 5.5 链上配置同步

```rust
// infra/group_config.rs

/// 群配置管理器
/// - 本地缓存 + 链上定期同步
/// - 链上 CommunityConfig.version 变化时拉取最新
pub struct ConfigManager {
    /// 本地缓存: community_id_hash → (CommunityConfig, last_sync_block)
    cache: DashMap<[u8; 32], (CommunityConfig, u64)>,
    chain: Arc<ChainClient>,
    sync_interval: Duration,  // 30s
}

impl ConfigManager {
    /// 获取群配置 (优先本地缓存)
    pub fn get_config(&self, community_id_hash: &[u8; 32]) -> Option<CommunityConfig> {
        self.cache.get(community_id_hash).map(|v| v.0.clone())
    }

    /// 后台同步任务
    pub async fn sync_loop(&self) {
        // 每 30s 查询链上 CommunityConfigs 版本
        // 版本号变化则拉取完整配置
        // 更新本地缓存
    }
}
```

---

## 6. TEE 安全层

### 6.1 模块功能矩阵

| 模块 | 核心功能 | 硬件依赖 | 安全级别 |
|---|---|---|---|
| `enclave_bridge.rs` | SGX Enclave 桥接 (sign, seal, unseal) | Intel SGX | 最高: Enclave 内执行 |
| `sealed_storage.rs` | AES-256-GCM 密封/解封 | 无 (纯软件) | 高: 静态密钥保护 |
| `key_manager.rs` | Ed25519 密钥生命周期 | SGX (可选) | 高: Enclave 或文件密封 |
| `attestor.rs` | TDX+SGX Quote 生成 | Intel TDX + SGX | 最高: 硬件证明 |
| `shamir.rs` | K-of-N 秘密分片 | 无 (纯数学) | 高: GF(256) 密码学 |
| `ceremony.rs` | RA-TLS 仪式协议 | TLS + SGX | 高: 双向远程证明 |

### 6.2 TEE 启动流程

```
启动 grouprobot
    │
    ▼
detect_tee_environment()
    ├── /dev/attestation/quote 存在 → TDX 模式
    ├── SGX SDK 可用 → SGX+TDX 混合模式
    └── 无 TEE 硬件 → Software 模式 (开发/测试)
    │
    ▼
EnclaveBridge::init(data_dir)
    ├── 尝试加载密封密钥 (enclave_key.sealed)
    │   ├── 成功 → 导入密钥
    │   └── 失败/不存在 → 生成新密钥 → 密封保存
    │
    ▼
Attestor::new(enclave_bridge)
    ├── generate_attestation()
    │   ├── TDX Quote: report_data[0..32] = SHA256(public_key)
    │   ├── SGX Quote: report_data[0..32] = SHA256(public_key)
    │   └── AttestationBundle { tdx_quote, sgx_quote, mrtd, mrenclave }
    │
    ▼
ChainClient::connect() → submit_attestation(bundle)
    │
    ▼
24h 循环: generate_attestation() → refresh_attestation()
```

### 6.3 RA-TLS 仪式流程

```
仪式发起者 (Enclave A)                     参与者 (Enclave B, C, ...)
─────────────────────                     ──────────────────────────
1. 生成 Ed25519 密钥对
2. Shamir split(secret, K, N)
3. 获取参与者列表 (链上 ActiveNodeList)
4. 对每个参与者:
   ├── RA-TLS 握手 (双向验证 MRENCLAVE)
   ├── 验证对方在链上白名单
   └── 加密发送 share_i ──────────────▶ 5. 验证发起者 MRENCLAVE
                                          6. 解密 share_i
                                          7. seal_to_file(share_i)
                                          8. ACK
9. 收到所有 ACK
10. record_ceremony(hash, K, N, participants) ──▶ 链上
11. zeroize(private_key) — 发起者不保留完整密钥
```

---

## 7. 平台适配层

### 7.1 PlatformAdapter Trait

```rust
// platform/mod.rs

/// 平台适配器 — 统一不同平台的事件解析和上下文提取
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// 平台标识
    fn platform_name(&self) -> &'static str;

    /// 原始事件 → 标准化 PlatformEvent
    fn parse_event(&self, raw: &serde_json::Value) -> Option<PlatformEvent>;

    /// 提取执行上下文 (群 ID, 用户 ID, 消息内容等)
    fn extract_context(&self, event: &PlatformEvent) -> MessageContext;
}

/// 平台执行器 — 统一不同平台的管理动作
#[async_trait]
pub trait PlatformExecutor: Send + Sync {
    /// 执行管理动作
    async fn execute(&self, action: &ExecuteAction) -> BotResult<ExecutionReceipt>;

    /// 生成执行收据签名
    fn sign_receipt(&self, receipt: &ExecutionReceipt, key: &KeyManager) -> [u8; 64];
}

/// 标准化平台事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEvent {
    pub platform: String,           // "telegram" | "discord"
    pub event_type: String,         // "message" | "member_join" | "command" | ...
    pub group_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub message_id: Option<String>,
    pub content: Option<String>,
    pub raw_event: serde_json::Value,
    pub timestamp: u64,
}

/// 消息上下文 (规则引擎输入)
#[derive(Debug, Clone)]
pub struct MessageContext {
    pub platform: String,
    pub group_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub message_text: String,
    pub is_command: bool,
    pub command: Option<String>,
    pub command_args: Vec<String>,
    pub is_join_request: bool,
    pub is_admin: bool,
}
```

### 7.2 Telegram 适配

| 组件 | 功能 | API |
|---|---|---|
| `TelegramAdapter` | Webhook JSON → PlatformEvent | 接收 Telegram Update |
| `TelegramExecutor` | 执行管理动作 | `sendMessage`, `banChatMember`, `restrictChatMember`, `pinChatMessage`, `deleteChatMessage`, `approveChatJoinRequest`, `declineChatJoinRequest`, `setWebhook` 等 |

### 7.3 Discord 适配

| 组件 | 功能 | API |
|---|---|---|
| `DiscordGateway` | WebSocket 长连接，事件推送 | HELLO→IDENTIFY/RESUME, heartbeat, reconnect |
| `DiscordAdapter` | Gateway 事件 → PlatformEvent | MESSAGE_CREATE, GUILD_MEMBER_ADD/REMOVE, INTERACTION_CREATE |
| `DiscordExecutor` | 执行管理动作 | REST API v10: `/channels/{id}/messages`, `/guilds/{id}/bans/{user}`, `/guilds/{id}/members/{user}`, slash commands 等 |

### 7.4 新平台扩展 (Matrix 示例)

```rust
// platform/matrix/adapter.rs
pub struct MatrixAdapter { /* ... */ }

impl PlatformAdapter for MatrixAdapter {
    fn platform_name(&self) -> &'static str { "matrix" }
    fn parse_event(&self, raw: &Value) -> Option<PlatformEvent> { /* Matrix 事件解析 */ }
    fn extract_context(&self, event: &PlatformEvent) -> MessageContext { /* ... */ }
}

// 注册: 在 main.rs 中按配置加载
match config.platform {
    Platform::Matrix => router.register(Box::new(MatrixAdapter::new(config))),
    // ...
}
```

---

## 8. 消息处理层

### 8.1 MessageRouter

```rust
// processing/router.rs

pub struct MessageRouter {
    rule_engine: RuleEngine,
    chain: Arc<ChainClient>,
    executors: HashMap<String, Box<dyn PlatformExecutor>>,
    key_manager: KeyManager,
    sequence: SequenceManager,
    log_sender: mpsc::Sender<PendingActionLog>,
}

impl MessageRouter {
    /// 处理一条平台事件 (核心流水线)
    pub async fn handle_event(&self, event: PlatformEvent) -> BotResult<()> {
        // 1. 标准化
        let ctx = self.normalize(&event);

        // 2. 规则引擎
        let decision = self.rule_engine.evaluate(&ctx).await?;

        // 3. 执行动作
        if let Some(action) = decision.action {
            let executor = self.executors.get(&event.platform)
                .ok_or(BotError::PlatformApi { platform: event.platform.clone(), message: "no executor".into() })?;
            let receipt = executor.execute(&action).await?;

            // 4. 签名 + 入队链上日志
            let sequence = self.sequence.next()?;
            let sig = executor.sign_receipt(&receipt, &self.key_manager);
            self.log_sender.send(PendingActionLog {
                community_id_hash: hash_group_id(&event.group_id),
                action_type: action.action_type,
                target_hash: hash_user_id(&action.target_user),
                sequence,
                message_hash: receipt.message_hash,
                signature: sig,
            }).await?;
        }

        // 5. 去重标记 (异步, 不阻塞响应)
        let chain = self.chain.clone();
        let bot_hash = self.key_manager.bot_id_hash();
        let seq = self.sequence.current();
        tokio::spawn(async move {
            let _ = chain.mark_sequence_processed(bot_hash, seq).await;
        });

        Ok(())
    }
}
```

### 8.2 RuleEngine — 可插拔规则链

```rust
// processing/rule_engine.rs

#[async_trait]
pub trait Rule: Send + Sync {
    /// 规则名称
    fn name(&self) -> &'static str;
    /// 评估: 返回 Some(decision) 终止链; None 继续下一条规则
    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision>;
}

pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
    store: Arc<LocalStore>,
}

impl RuleEngine {
    pub fn new(store: Arc<LocalStore>, config: &CommunityConfig) -> Self {
        let mut rules: Vec<Box<dyn Rule>> = vec![];

        // 按配置组装规则链
        if config.anti_flood_enabled {
            rules.push(Box::new(FloodRule::new(config.flood_limit)));
        }
        rules.push(Box::new(BlacklistRule::new()));
        rules.push(Box::new(CommandRule::new()));
        rules.push(Box::new(JoinRequestRule::new()));
        rules.push(Box::new(DefaultRule));

        Self { rules, store }
    }

    pub async fn evaluate(&self, ctx: &MessageContext) -> BotResult<RuleDecision> {
        for rule in &self.rules {
            if let Some(decision) = rule.evaluate(ctx, &self.store).await {
                return Ok(RuleDecision { matched_rule: rule.name(), action: Some(decision) });
            }
        }
        Ok(RuleDecision { matched_rule: "none", action: None })
    }
}
```

### 8.3 规则详情

| 规则 | 优先级 | 触发条件 | 动作 |
|---|---|---|---|
| `FloodRule` | 1 | 用户在窗口内消息数 > `flood_limit` | Mute (临时) |
| `BlacklistRule` | 2 | 消息内容匹配黑名单 regex | DeleteMessage + Warn |
| `CommandRule` | 3 | 消息以 `/ban`, `/mute` 等指令开头 (发送者是管理员) | 执行对应动作 |
| `JoinRequestRule` | 4 | `is_join_request == true` | 根据 JoinApprovalPolicy 审批 |
| `DefaultRule` | 5 | 兜底: 所有消息 | 无动作 (仅记录) |

---

## 9. 运行时基础设施

### 9.1 Prometheus 指标

| 指标名 | 类型 | 说明 |
|---|---|---|
| `grouprobot_messages_total` | Counter | 处理的消息总数 |
| `grouprobot_actions_total` | Counter (label: action_type) | 执行的管理动作数 |
| `grouprobot_chain_tx_total` | Counter (label: status) | 链上交易提交数 |
| `grouprobot_chain_tx_latency_seconds` | Histogram | 链上交易延迟 |
| `grouprobot_quote_refresh_total` | Counter (label: success) | TEE 证明刷新次数 |
| `grouprobot_active_groups` | Gauge | 活跃群组数 |
| `grouprobot_rule_matches_total` | Counter (label: rule) | 规则命中次数 |
| `grouprobot_webhook_latency_seconds` | Histogram | Webhook 处理延迟 |

### 9.2 HTTP 路由

| 路径 | 方法 | 功能 | 鉴权 |
|---|---|---|---|
| `/webhook` | POST | Telegram Webhook 入口 | X-Telegram-Bot-Api-Secret-Token |
| `/health` | GET | 健康检查 | 无 |
| `/metrics` | GET | Prometheus 指标 | 无 |
| `/v1/execute` | POST | 内部执行指令 (管理面板) | Ed25519 签名 |
| `/v1/group-config` | GET | 获取群配置 | API Key |
| `/v1/group-config` | POST | 更新群配置 (→ 链上) | Ed25519 签名 |
| `/v1/status` | GET | Bot 状态 (TEE, 链上, 平台) | API Key |

---

## 10. 配置与环境变量

### 10.1 .env.example

```bash
# ══════════════════════════════════════════
# GroupRobot Bot 配置
# ══════════════════════════════════════════

# 平台模式: telegram | discord | both
PLATFORM=telegram

# Bot 标识
BOT_ID_HASH=0x...   # SHA256(bot_token) 或手动指定
BOT_TOKEN=           # Telegram Bot Token

# Discord (PLATFORM=discord 或 both 时必填)
DISCORD_BOT_TOKEN=
DISCORD_APPLICATION_ID=
DISCORD_INTENTS=33281  # GUILDS | GUILD_MEMBERS | GUILD_MESSAGES | MESSAGE_CONTENT

# Webhook
WEBHOOK_PORT=3000
WEBHOOK_URL=https://your-domain.com/webhook
WEBHOOK_SECRET=your-random-secret

# 链上
CHAIN_RPC=ws://127.0.0.1:9944
CHAIN_SIGNER_SEED=     # 可选: 指定签名密钥种子 (留空自动生成)

# TEE
TEE_MODE=auto          # auto | tdx | sgx | software
DATA_DIR=./data        # 密封密钥、序列号、配置缓存

# 性能
WEBHOOK_RATE_LIMIT=200       # 200 请求/分钟
EXECUTE_RATE_LIMIT=100       # 100 执行/分钟
CHAIN_LOG_BATCH_INTERVAL=6   # 批量日志提交间隔 (秒)
CHAIN_LOG_BATCH_SIZE=50      # 每批最大日志数

# 监控
METRICS_ENABLED=true
LOG_LEVEL=info               # trace | debug | info | warn | error
```

---

## 11. 部署架构

### 11.1 单实例模式 (开发/小型部署)

```
┌────────────────────────────────────────────┐
│  TDX VM / Docker Container                  │
│  ┌────────────────────────────────────────┐ │
│  │ grouprobot                         │ │
│  │  :3000 ← Telegram Webhook             │ │
│  │  :3000/metrics ← Prometheus            │ │
│  │  ws:// ← Discord Gateway              │ │
│  └──────────────┬─────────────────────────┘ │
│                 │ subxt (ws://localhost:9944) │
│  ┌──────────────▼─────────────────────────┐ │
│  │ Substrate Node                          │ │
│  └─────────────────────────────────────────┘ │
└────────────────────────────────────────────┘
```

### 11.2 高可用模式 (生产)

```
                Telegram Cloud / Discord Gateway
                         │
                    ┌────▼────┐
                    │  Nginx  │ (TLS 终止 + 负载均衡)
                    └────┬────┘
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │ TEE Bot #1  │ │ TEE Bot #2  │ │ TEE Bot #3  │
    │ (Active)    │ │ (Active)    │ │ (Standby)   │
    └──────┬──────┘ └──────┬──────┘ └──────┬──────┘
           │               │               │
           └───────────────┼───────────────┘
                           │ 链上去重
                    ┌──────▼──────┐
                    │ Substrate   │
                    │ Full Node   │
                    └─────────────┘

    去重机制: mark_sequence_processed 确保同一消息只执行一次
    - Bot #1 先提交 → 成功
    - Bot #2 后提交 → SequenceDuplicate 事件 (不执行)
    - Bot #3 Standby → 仅监听, 不执行
```

### 11.3 docker-compose.yml 示例

```yaml
version: "3.9"
services:
  chain:
    image: nexus-node:latest
    ports: ["9944:9944", "9933:9933"]
    command: --dev --tmp

  bot:
    build: ./grouprobot
    ports: ["3000:3000"]
    environment:
      - PLATFORM=telegram
      - BOT_TOKEN=${BOT_TOKEN}
      - WEBHOOK_PORT=3000
      - WEBHOOK_URL=${WEBHOOK_URL}
      - WEBHOOK_SECRET=${WEBHOOK_SECRET}
      - CHAIN_RPC=ws://chain:9944
      - TEE_MODE=auto
      - DATA_DIR=/data
      - LOG_LEVEL=info
    volumes:
      - bot-data:/data
    depends_on: [chain]

  prometheus:
    image: prom/prometheus
    ports: ["9090:9090"]
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml

volumes:
  bot-data:
```

---

## 12. 开发路线图

### Phase OC-1: 核心骨架 (3 天)

```
Sprint OC-1a (2天): 项目初始化 + 链交互层
├── Cargo.toml + 依赖配置
├── src/main.rs — AppState + 启动流程
├── src/config.rs — 配置加载
├── src/error.rs — 统一错误类型
├── src/chain/client.rs — ChainClient (subxt connect)
├── src/chain/transactions.rs — register_bot, submit_attestation, mark_sequence_processed
├── src/chain/queries.rs — fetch_bot, query_tee_status, is_sequence_processed
└── 测试: 链客户端单元测试 (mock subxt) ~15 tests

Sprint OC-1b (1天): TEE 安全层
├── src/tee/enclave_bridge.rs — SGX 桥接 (SoftwareEnclave 模式优先)
├── src/tee/sealed_storage.rs — AES-256-GCM 密封
├── src/tee/key_manager.rs — Ed25519 密钥管理
├── src/tee/attestor.rs — 双证明生成
└── 测试: TEE 层单元测试 ~20 tests
```

### Phase OC-2: 平台适配 (3 天)

```
Sprint OC-2a (2天): Telegram 全功能
├── src/platform/mod.rs — PlatformAdapter + PlatformExecutor traits
├── src/platform/telegram/adapter.rs — Webhook → PlatformEvent
├── src/platform/telegram/executor.rs — Bot API 10 个方法
├── src/webhook.rs — Axum 路由 (/webhook, /health, /execute)
└── 测试: Telegram 适配测试 ~15 tests

Sprint OC-2b (1天): Discord Gateway + 执行器
├── src/platform/discord/gateway.rs — WebSocket + heartbeat + reconnect
├── src/platform/discord/adapter.rs — Gateway 事件 → PlatformEvent
├── src/platform/discord/executor.rs — REST API 20 个方法
└── 测试: Discord 适配测试 ~10 tests
```

### Phase OC-3: 消息处理 (2 天)

```
Sprint OC-3 (2天): 规则引擎 + 消息路由
├── src/processing/router.rs — MessageRouter (核心流水线)
├── src/processing/normalizer.rs — PlatformEvent → InternalMessage
├── src/processing/rule_engine.rs — RuleEngine (可插拔规则链)
├── src/processing/rules/*.rs — 5 条规则实现
├── src/processing/action.rs — ActionDecision + ActionType
└── 测试: 规则引擎测试 ~20 tests
```

### Phase OC-4: 基础设施 + 集成 (2 天)

```
Sprint OC-4a (1天): 基础设施
├── src/infra/metrics.rs — Prometheus 指标 + /metrics
├── src/infra/rate_limiter.rs — 滑动窗口限流
├── src/infra/local_store.rs — 内存缓存 (flood/fingerprint)
├── src/infra/group_config.rs — 群配置缓存 + 链上同步
└── 测试: 基础设施测试 ~10 tests

Sprint OC-4b (1天): 集成 + E2E
├── src/chain/transactions.rs — batch_submit_logs + ActionLogBatcher
├── 全流程集成: Webhook → Router → Executor → ChainLog
├── Dockerfile + docker-compose.yml
└── 测试: 集成测试 ~10 tests
```

### Phase OC-5: Shamir + 仪式 (2 天)

```
Sprint OC-5 (2天): 高级 TEE 功能
├── src/tee/shamir.rs — GF(256) split/recover + AES 加密分片
├── src/tee/ceremony.rs — RA-TLS 仪式客户端
├── src/chain/transactions.rs — record_ceremony
└── 测试: Shamir + 仪式测试 ~15 tests
```

### 工期总结

| Phase | 工期 | 新增代码 | 测试数 |
|---|---|---|---|
| OC-1: 核心骨架 | 3 天 | ~2,000 行 | ~35 |
| OC-2: 平台适配 | 3 天 | ~2,500 行 | ~25 |
| OC-3: 消息处理 | 2 天 | ~1,500 行 | ~20 |
| OC-4: 基础设施+集成 | 2 天 | ~1,000 行 | ~20 |
| OC-5: Shamir+仪式 | 2 天 | ~1,500 行 | ~15 |
| **合计** | **12 天** | **~8,500 行** | **~115** |

---

## 13. 测试策略

### 13.1 测试分层

| 层级 | 范围 | 工具 | 数量 |
|---|---|---|---|
| 单元测试 | 每个模块独立 | `#[tokio::test]`, `mockall` | ~80 |
| 集成测试 | 跨模块交互 | `tests/` 目录, `testcontainers` | ~20 |
| E2E 测试 | 完整流程 | bash + curl + docker-compose | ~15 |

### 13.2 Mock 策略

```rust
// 链客户端 Mock (用于不依赖真实节点的测试)
#[cfg(test)]
pub struct MockChainClient {
    pub registered_bots: DashMap<[u8; 32], bool>,
    pub processed_sequences: DashMap<([u8; 32], u64), bool>,
    pub action_logs: Mutex<Vec<PendingActionLog>>,
}

// 平台执行器 Mock (用于不依赖真实 API 的测试)
#[cfg(test)]
pub struct MockExecutor {
    pub executed_actions: Mutex<Vec<ExecuteAction>>,
}
```

### 13.3 E2E 测试脚本

```bash
#!/bin/bash
# test-e2e.sh

# 1. 启动 docker-compose (chain + bot)
# 2. 等待健康检查
# 3. 模拟 Webhook 推送
# 4. 验证链上状态 (action_log 已提交)
# 5. 验证 /metrics 端点
# 6. 模拟 Discord slash command
# 7. 验证去重 (重复序列号不执行)
# 8. 清理
```

---

## 14. 与 nexus-tee-bot 的差异对比

### 14.1 架构变更

| 维度 | nexus-tee-bot | grouprobot |
|---|---|---|
| **链上目标** | `BotRegistry` (旧) | `GroupRobotRegistry` (新, index 150) |
| **链上目标** | `BotConsensus` (旧) | `GroupRobotConsensus` (新, index 151) |
| **链上目标** | `BotGroupMgmt` (旧) | `GroupRobotCommunity` (新, index 152) |
| **链上目标** | `CeremonyAudit` (旧) | `GroupRobotCeremony` (新, index 153) |
| **日志提交** | 逐条提交 | 批量提交 (`batch_submit_logs`, 6s 窗口) |
| **配置同步** | 纯本地文件 | 本地缓存 + 链上 `CommunityConfigs` 定期同步 |
| **代码组织** | 扁平: 所有 `.rs` 在 `src/` | 分层: `chain/` `tee/` `platform/` `processing/` `infra/` |
| **错误处理** | `anyhow` 贯穿 | 分层 `thiserror` + `BotError` |
| **多平台** | 后期添加 Discord, 逻辑分散 | `PlatformAdapter` / `PlatformExecutor` trait 统一 |

### 14.2 可复用模块

以下模块可从 `nexus-tee-bot` 直接迁移（仅需调整命名和导入）：

| nexus-tee-bot 模块 | grouprobot 目标 | 复用度 |
|---|---|---|
| `enclave/` + `enclave_bridge.rs` | `tee/enclave_bridge.rs` | 95% |
| `sealed_storage.rs` | `tee/sealed_storage.rs` | 100% |
| `signer.rs` | `tee/key_manager.rs` | 90% |
| `attestation.rs` | `tee/attestor.rs` | 90% |
| `shamir.rs` | `tee/shamir.rs` | 100% |
| `ceremony.rs` | `tee/ceremony.rs` | 85% |
| `rate_limiter.rs` | `infra/rate_limiter.rs` | 100% |
| `metrics.rs` | `infra/metrics.rs` | 80% (新增指标) |
| `executor.rs` | `platform/telegram/executor.rs` | 90% |
| `discord_executor.rs` | `platform/discord/executor.rs` | 90% |
| `gateway/discord.rs` | `platform/discord/gateway.rs` | 95% |

### 14.3 需重写的模块

| 模块 | 原因 |
|---|---|
| `chain_client.rs` | Pallet 名称全变 + 添加批量提交 + 类型安全解码 |
| `local_processor.rs` | 拆分为 `router.rs` + `rule_engine.rs` + `rules/` |
| `group_config.rs` | 添加链上 `CommunityConfigs` 同步逻辑 |
| `webhook.rs` | 重构为平台无关路由 + 平台适配器分发 |
| `config.rs` | 新增配置项 + 结构化配置 |
| `main.rs` | 全新启动流程 + 分层初始化 |

---

## 附录 A: 依赖清单

```toml
[dependencies]
# Async
tokio = { version = "1", features = ["full"] }

# HTTP
axum = { version = "0.7", features = ["json"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Cryptography
ed25519-dalek = { version = "2", features = ["rand_core"] }
aes-gcm = "0.10"
sha2 = "0.10"
rand = "0.8"

# Substrate chain client
subxt = { version = "0.38", features = ["substrate-compat"] }
subxt-signer = { version = "0.38", features = ["sr25519", "subxt"] }
codec = { package = "parity-scale-codec", version = "3.6", features = ["derive"] }

# WebSocket (Discord)
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-webpki-roots"] }
futures-util = "0.3"

# Concurrent state
dashmap = "6"

# Monitoring
prometheus-client = "0.22"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
dotenvy = "0.15"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
regex = "1"
url = "2"
async-trait = "0.1"
```

## 附录 B: 性能指标目标

| 指标 | 目标值 | 说明 |
|---|---|---|
| Webhook 响应延迟 (P99) | < 50ms | 消息接收到处理完成 |
| 规则引擎判定 | < 5ms | 单条消息规则评估 |
| 平台 API 调用 | < 500ms | 发消息/踢人等 |
| 链上去重查询 | < 200ms | `is_sequence_processed` RPC |
| 链上交易提交 | < 12s | 等待 finalized (2 区块) |
| TEE 证明生成 | < 2s | TDX + SGX Quote |
| 内存占用 | < 256MB | 单 Bot 实例 |
| 并发群组 | > 100 | 单实例支持 |

---

*文档版本: v1.0 · 2026-02-22*
*维护者: Nexus Team*
