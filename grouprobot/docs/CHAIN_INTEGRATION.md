# 链上集成

> 版本: 0.1.0 | 最后更新: 2026-02-23

## 1. 概述

GroupRobot 通过 [subxt](https://github.com/paritytech/subxt) 动态 API 与 Nexus 区块链 (Substrate) 交互。所有交互均为**异步非阻塞**，链客户端在后台连接，连接成功后注入到消息路由器。

```
GroupRobot 进程
    │
    ├─ ChainClient::connect(rpc_url, signer)     ← 后台 tokio::spawn
    │       │
    │       ├─ queries.rs    ← 链上读取 (storage query)
    │       │   ├─ fetch_bot()             → GroupRobotRegistry::Bots
    │       │   ├─ query_tee_status()      → GroupRobotRegistry::Attestations
    │       │   ├─ query_community_config() → GroupRobotCommunity::CommunityConfigs
    │       │   ├─ query_attestation_guard() → 验证 TEE 证明状态
    │       │   └─ query_attestation_nonce() → AttestationNonces (硬件模式)
    │       │
    │       └─ transactions.rs  ← 链上写入 (extrinsic submit)
    │           ├─ submit_attestation()         → 首次 TEE 证明
    │           ├─ refresh_attestation()        → 软件模式刷新
    │           ├─ submit_verified_attestation() → 硬件 Level 1
    │           ├─ submit_dcap_full_attestation() → 硬件 Level 4 (全证书链)
    │           ├─ request_attestation_nonce()  → 请求链上 nonce
    │           ├─ mark_sequence_processed()    → 序列号去重标记
    │           └─ ActionLogBatcher::run()      → 动作日志批量提交
    │
    └─ ConfigManager::sync_loop(chain)  ← 群配置同步循环
```

## 2. 链客户端 (ChainClient)

### 2.1 连接与认证

```rust
// chain/client.rs
pub struct ChainClient {
    api: OnlineClient<SubstrateConfig>,
    signer: PairSigner<SubstrateConfig, sr25519::Pair>,
}
```

- **RPC 连接**: WebSocket (`ws://` 或 `wss://`)，由 `CHAIN_RPC` 环境变量配置
- **签名密钥**: SR25519，优先从密封存储加载，其次从 `CHAIN_SIGNER_SEED` 环境变量生成
- **安全**: 连接成功后立即清除 `CHAIN_SIGNER_SEED` 环境变量，防止 `/proc/pid/environ` 泄漏

### 2.2 动态 API

GroupRobot 使用 subxt 的**动态 API** (`subxt::dynamic`)，无需编译时生成 metadata 类型:

```rust
let query = subxt::dynamic::storage(
    "GroupRobotRegistry",  // pallet 名
    "Bots",                // storage 名
    vec![Value::from_bytes(bot_id_hash)],  // 键
);
let result = self.api().storage().at_latest().await?.fetch(&query).await?;
```

优势: 无需在 GroupRobot crate 中引入 runtime 依赖，降低编译复杂度。

## 3. 链上查询 (queries.rs)

### 3.1 Bot 信息查询

```rust
pub async fn fetch_bot(&self, bot_id_hash: &[u8; 32]) -> BotResult<Option<BotInfoCache>>
```

**读取**: `GroupRobotRegistry::Bots`

**返回字段**:
| 字段 | 说明 |
|------|------|
| `bot_id_hash` | Bot ID 哈希 (hex) |
| `owner` | 所有者账户 |
| `public_key` | Ed25519 公钥 `[u8; 32]` |
| `is_active` | 是否激活 |
| `is_tee_node` | 是否 TEE 节点 (node_type ≥ 1) |

### 3.2 TEE 状态查询

```rust
pub async fn query_tee_status(&self, bot_id_hash: &[u8; 32]) -> BotResult<Option<TeeNodeStatus>>
```

**读取**: `GroupRobotRegistry::Attestations`

**返回字段**:
| 字段 | 说明 |
|------|------|
| `is_attested` | 是否已提交证明 |
| `is_expired` | 证明是否过期 |
| `expires_at` | 过期区块号 |

### 3.3 群配置查询

```rust
pub async fn query_community_config(&self, community_id_hash: &[u8; 32])
    -> BotResult<Option<ChainCommunityConfig>>
```

**读取**: `GroupRobotCommunity::CommunityConfigs`

**ChainCommunityConfig 字段** (26 个):

| 分类 | 字段 | 类型 | 默认值 |
|------|------|------|--------|
| 基础 | `node_requirement` | u8 | — |
| | `version` | u32 | — |
| 防刷屏 | `anti_flood_enabled` | bool | false |
| | `flood_limit` | u16 | — |
| 重复检测 | `anti_duplicate_enabled` | bool | false |
| | `duplicate_window_secs` | u64 | 300 |
| | `duplicate_threshold` | u16 | 3 |
| 内容过滤 | `stop_words` | String | "" |
| | `max_emoji` | u16 | 0 |
| | `max_links` | u16 | 0 |
| 警告系统 | `warn_limit` | u8 | — |
| | `warn_action` | u8 | 0 (Mute) |
| | `warn_mute_duration` | u64 | 3600 |
| 欢迎 | `welcome_enabled` | bool | false |
| | `welcome_template` | String | "" |
| | `goodbye_template` | String | "" |
| 相似度 | `spam_samples` | String | "" |
| | `similarity_threshold` | u8 | 70 |
| 审计 | `log_channel_id` | String | "" |
| CAPTCHA | `captcha_enabled` | bool | false |
| | `captcha_timeout_secs` | u64 | 120 |
| 反钓鱼 | `antiphishing_enabled` | bool | false |
| | `bayes_threshold` | u8 | 80 |
| 自定义命令 | `custom_commands_csv` | String | "" |
| 消息锁定 | `locked_types_csv` | String | "" |

### 3.4 AttestationGuard 查询

```rust
pub async fn query_attestation_guard(&self, requester_pk_hex: &str)
    -> BotResult<(bool, bool)>  // (is_tee, quote_verified)
```

**用途**: Ceremony (Share 分发) 中验证请求方是否为已认证的 TEE 节点。

**流程**: `SHA256(pk)` → `bot_id_hash` → 查询 `Attestations` → 返回 `(is_tee, quote_verified)`

### 3.5 Nonce 查询 (硬件模式)

```rust
pub async fn query_attestation_nonce(&self, bot_id_hash: &[u8; 32])
    -> BotResult<Option<[u8; 32]>>
```

**读取**: `GroupRobotRegistry::AttestationNonces` (SCALE raw bytes 解码)

## 4. 链上交易 (transactions.rs)

### 4.1 TEE 证明提交

| 方法 | Pallet::Call | 场景 |
|------|-------------|------|
| `submit_attestation` | `GroupRobotRegistry::submit_attestation` | 首次启动 |
| `refresh_attestation` | `GroupRobotRegistry::refresh_attestation` | 软件模式 24h 刷新 |
| `submit_verified_attestation` | `GroupRobotRegistry::submit_verified_attestation` | 硬件 Level 1 (nonce 防重放) |
| `submit_dcap_full_attestation` | `GroupRobotRegistry::submit_dcap_full_attestation` | 硬件 Level 4 (全证书链 ECDSA) |
| `request_attestation_nonce` | `GroupRobotRegistry::request_attestation_nonce` | 请求链上生成 nonce |

**证明数据** (`AttestationBundle`):

| 字段 | 类型 | 说明 |
|------|------|------|
| `tdx_quote_hash` | [u8; 32] | TDX Quote SHA-256 哈希 |
| `sgx_quote_hash` | [u8; 32] | SGX Quote SHA-256 哈希 |
| `mrtd` | [u8; 48] | TDX 度量值 (MRTD) |
| `mrenclave` | [u8; 32] | SGX 度量值 (MRENCLAVE) |
| `is_simulated` | bool | 是否为软件模拟 |
| `tdx_quote_raw` | Option\<Vec\<u8\>\> | 原始 TDX Quote (硬件模式) |
| `nonce` | Option\<[u8; 32]\> | 链上 nonce (嵌入 report_data[32..64]) |
| `pck_cert_der` | Option\<Vec\<u8\>\> | PCK 证书 DER (Level 4) |
| `intermediate_cert_der` | Option\<Vec\<u8\>\> | 中间 CA 证书 DER (Level 4) |

### 4.2 硬件模式刷新流程

```
           GroupRobot                        Nexus Chain
               │                                 │
               │  request_attestation_nonce ──►   │
               │                                 │ ← 生成随机 nonce 存入 AttestationNonces
               │                                 │
               │  ◄── 等待 7s (1 个区块) ──►      │
               │                                 │
               │  query_attestation_nonce ────►   │
               │  ◄──── nonce [u8; 32] ────       │
               │                                 │
               │  generate_attestation_with_nonce │
               │  (nonce 嵌入 report_data[32..64]) │
               │                                 │
               │  submit_dcap_full_attestation ──► │
               │  (Quote + PCK cert + Intermediate) │ ← 4 级 ECDSA 验证
               │                                 │    Level 4: Intel Root → Intermediate
               │  ◄──── 上链成功 ────              │    → PCK → QE Report → AK → Body
```

### 4.3 序列号去重

```rust
pub async fn mark_sequence_processed(&self, bot_id_hash: [u8; 32], sequence: u64) -> BotResult<()>
```

**写入**: `GroupRobotConsensus::mark_sequence_processed(bot_id_hash, sequence)`

每条消息处理后异步标记，链上通过 `ProcessedSequences` 存储防止重放。

### 4.4 动作日志批量提交

```rust
pub struct ActionLogBatcher {
    receiver: mpsc::Receiver<PendingActionLog>,
    chain: Arc<ChainClient>,
    interval_secs: u64,     // 批量间隔 (默认 30s)
    batch_size: usize,      // 批量大小 (默认 50)
}
```

**工作模式**: 收集 `PendingActionLog`，当积累达到 `batch_size` 或超过 `interval_secs` 时批量提交。

**PendingActionLog**:
| 字段 | 类型 | 说明 |
|------|------|------|
| `community_id_hash` | [u8; 32] | 群组 ID SHA-256 哈希 |
| `action_type` | u8 | 动作类型 (ActionType 枚举值) |
| `target_hash` | [u8; 32] | 目标用户 ID SHA-256 哈希 |
| `sequence` | u64 | 递增序列号 |
| `message_hash` | [u8; 32] | 消息+动作 SHA-256 哈希 |
| `signature` | [u8; 64] | Ed25519 签名 |

**隐私保护**: `community_id_hash` 和 `target_hash` 都是 SHA-256 哈希，链上不存储明文群/用户 ID。

## 5. 配置同步 (ConfigManager)

```rust
// infra/group_config.rs
pub struct ConfigManager {
    cache: DashMap<String, CachedConfig>,
    ttl_minutes: u64,  // 缓存 TTL (默认 30 分钟)
}
```

### 同步循环

`ConfigManager::sync_loop(chain)` 启动后台循环:

1. 每 `ttl_minutes` 遍历缓存中的所有 `group_id`
2. 对每个 `group_id` 调用 `chain.query_community_config(hash)`
3. 更新本地缓存
4. 如果链上配置的 `version` 变化，重建 `RuleEngine`

### 首次查询

当 Webhook 收到新群的消息时，`ConfigManager` 首次从链上拉取配置并缓存。后续请求直接使用缓存。

## 6. 与链上 Pallet 的关系图

```
┌────────────────────────────────────────────────────────────┐
│                     Nexus Blockchain                        │
│                                                            │
│  ┌─────────────────────────┐  ┌──────────────────────────┐ │
│  │ pallet-grouprobot-      │  │ pallet-grouprobot-       │ │
│  │ registry (150)          │  │ consensus (151)          │ │
│  │                         │  │                          │ │
│  │ Storage:                │  │ Storage:                 │ │
│  │  Bots                   │  │  ProcessedSequences      │ │
│  │  Attestations           │  │  EraRewards              │ │
│  │  AttestationNonces      │  │  ActionLogs              │ │
│  │  ApprovedApiServerMrtd  │  │  SequenceCleanupCursor   │ │
│  │  RegisteredPckKeys      │  │                          │ │
│  │                         │  │ Anti-bloat:              │ │
│  │ Calls:                  │  │  TTL cleanup (24h)       │ │
│  │  submit_attestation     │  │  EraRewards window (365) │ │
│  │  refresh_attestation    │  │                          │ │
│  │  submit_dcap_*          │  │                          │ │
│  │  request_nonce          │  │                          │ │
│  └─────────────────────────┘  └──────────────────────────┘ │
│                                                            │
│  ┌─────────────────────────┐  ┌──────────────────────────┐ │
│  │ pallet-grouprobot-      │  │ pallet-grouprobot-       │ │
│  │ community (152)         │  │ ceremony (153)           │ │
│  │                         │  │                          │ │
│  │ Storage:                │  │ Storage:                 │ │
│  │  CommunityConfigs       │  │  CeremonyState           │ │
│  │  (26 个配置字段)         │  │  ShareRegistry           │ │
│  │                         │  │                          │ │
│  │ 群主通过治理交易配置     │  │ Shamir 仪式协调          │ │
│  └─────────────────────────┘  └──────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```
