# GroupRobot Pallet 模块设计文档

> 日期: 2026-02-22
> 版本: v1.0
> 状态: 设计阶段
>
> **基于 [TEE_SGX_TDX_BOT_ANALYSIS.md](./TEE_SGX_TDX_BOT_ANALYSIS.md) 架构设计，将 nexus-tee-bot 离链功能重构为 Substrate Pallet 模块族**

---

## 目录

1. [设计目标](#1-设计目标)
2. [架构总览](#2-架构总览)
3. [模块划分](#3-模块划分)
4. [pallet-grouprobot-registry](#4-pallet-grouprobot-registry) — Bot 注册 + TEE 节点 + 平台绑定
5. [pallet-grouprobot-consensus](#5-pallet-grouprobot-consensus) — 节点共识 + TEE 加权奖励 + 去重
6. [pallet-grouprobot-community](#6-pallet-grouprobot-community) — 社区管理 + 群规则 + 动作日志
7. [pallet-grouprobot-ceremony](#7-pallet-grouprobot-ceremony) — RA-TLS 仪式审计 + Enclave 白名单
8. [pallet-grouprobot-primitives](#8-pallet-grouprobot-primitives) — 共享类型 + Trait 接口
9. [离链组件 (Off-chain Worker + Bot Client)](#9-离链组件)
10. [与现有 pallet 的迁移关系](#10-与现有-pallet-的迁移关系)
11. [Runtime 集成](#11-runtime-集成)
12. [开发路线图](#12-开发路线图)
13. [测试策略](#13-测试策略)

---

## 1. 设计目标

### 1.1 核心目标

将 `nexus-tee-bot` 离链二进制 + 现有 4 个 nexus pallet 统一重构为 **`grouprobot`** 模块族：

| 目标 | 说明 |
|------|------|
| **统一命名** | `pallets/grouprobot/*` 替代分散的 `pallets/nexus/bot-*` |
| **职责清晰** | 每个子 pallet 对应一个功能域，不再交叉 |
| **链上链下分离** | Pallet 只管链上逻辑，离链 Bot 通过 subxt 交互 |
| **TEE 原生支持** | TEE 证明、Shamir 参数、仪式审计作为一等公民 |
| **可扩展** | Trait 接口解耦，支持未来新平台 / 新 TEE 厂商 |

### 1.2 设计原则

1. **链上最小化** — 只存必须上链的数据（注册、证明、奖励、审计），消息内容和执行逻辑保持离链
2. **Trait 解耦** — 子 pallet 通过 trait 交互，可独立测试和升级
3. **向后兼容** — 提供 storage migration 从现有 pallet 迁移
4. **渐进采纳** — 普通节点 (StandardNode) 和 TEE 节点 (TeeNode) 共存

---

## 2. 架构总览

### 2.1 模块架构图

```
pallets/grouprobot/
├── primitives/          ← 共享类型 + Trait (无状态 crate)
├── registry/            ← Bot 注册 + TEE 节点 + 平台绑定
├── consensus/           ← 节点共识 + TEE 加权奖励 + 去重 + 订阅
├── community/           ← 社区管理 + 群规则 + 节点准入 + 动作日志
└── ceremony/            ← RA-TLS 仪式审计 + Enclave 白名单 + 风险检测
```

### 2.2 模块交互关系

```
                    ┌──────────────────────┐
                    │   primitives (types)  │
                    │  Platform, NodeType,  │
                    │  BotId, Traits...     │
                    └──────┬───────────────┘
                           │ 被所有子 pallet 依赖
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
  │   registry   │ │  consensus   │ │  community   │
  │              │◀│              │ │              │
  │ Bot 注册     │ │ 节点奖励     │ │ 群规则       │
  │ TEE 证明     │ │ 消息去重     │ │ 节点准入     │
  │ 平台绑定     │ │ 订阅管理     │ │ 动作日志     │
  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘
         │                │                │
         │     ┌──────────┴────────┐       │
         └────▶│    ceremony       │◀──────┘
               │                   │
               │ 仪式记录/撤销     │
               │ Enclave 白名单    │
               │ Shamir 参数验证   │
               │ 自动风险检测      │
               └───────────────────┘
```

### 2.3 链上 vs 离链职责划分

| 层级 | 组件 | 职责 |
|------|------|------|
| **链上 Pallet** | `grouprobot/*` | 注册、证明存证、奖励分配、去重、仪式审计 |
| **离链 Bot** | `nexus-tee-bot` | TDX+SGX 加密执行、Webhook 接收、消息处理、TG/DC API 调用 |
| **离链 OCW** | Off-chain Worker | Quote 过期扫描、自动风险检测（可选） |

---

## 3. 模块划分

### 3.1 从 nexus-tee-bot 功能到 Pallet 的映射

| nexus-tee-bot 模块 | 功能 | 目标 Pallet | 链上/链下 |
|---|---|---|---|
| `config.rs` | Bot 配置 | registry | 部分上链 (公钥, bot_id_hash) |
| `enclave_bridge.rs` + `enclave/` | SGX Enclave 桥接 | — | **纯离链** (Bot Client 内部) |
| `attestation.rs` | TDX+SGX 双证明 | registry | 证明数据上链, 生成离链 |
| `signer.rs` | Ed25519 签名 | — | **纯离链** (SGX Enclave 内) |
| `shamir.rs` | Shamir 密钥分片 | ceremony | 参数 (K,N) 上链, 分片操作离链 |
| `ceremony.rs` | RA-TLS 安全仪式 | ceremony | 仪式记录上链, 仪式执行离链 |
| `chain_client.rs` | 链交互 (subxt) | — | **纯离链** (Bot Client) |
| `metrics.rs` | Prometheus 监控 | — | **纯离链** |
| `executor.rs` | Telegram API 执行 | — | **纯离链** |
| `discord_executor.rs` | Discord API 执行 | — | **纯离链** |
| `webhook.rs` | HTTP Webhook 处理 | — | **纯离链** |
| `local_processor.rs` | 本地消息处理 | community | 动作日志上链, 处理逻辑离链 |
| `local_store.rs` | 本地状态缓存 | — | **纯离链** |
| `group_config.rs` | 群配置管理 | community | 群规则上链 |
| `types.rs` | 共享类型 | primitives | 类型定义上链 |
| `rate_limiter.rs` | 速率限制 | — | **纯离链** |
| `crypto.rs` | 加密工具 | — | **纯离链** |
| `platform/` | 平台适配器 | registry | 平台枚举上链, 适配器离链 |
| `gateway/` | 网关适配器 | — | **纯离链** |

### 3.2 从现有 Pallet 的映射

| 现有 Pallet | 功能 | 目标 grouprobot 子模块 |
|---|---|---|
| `pallet-bot-registry` | Bot 注册, 公钥, 平台绑定, TEE 证明 | **registry** |
| `pallet-bot-consensus` | 节点注册/质押, 奖励, 去重, 订阅, Equivocation | **consensus** |
| `pallet-bot-group-mgmt` | 群规则, 动作日志, 节点准入 | **community** |
| `pallet-ceremony-audit` | 仪式记录, Enclave 白名单 | **ceremony** |

---

## 4. pallet-grouprobot-registry

> **职责: Bot 注册 + TEE 节点管理 + 平台身份绑定 + 证明管理**
>
> 整合现有 `pallet-bot-registry` + `attestation.rs` 相关链上逻辑

### 4.1 Types

```rust
/// 社交平台枚举
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum Platform {
    Telegram, Discord, Slack, Matrix, Farcaster,
}

/// 节点类型 (标准 vs TEE)
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum NodeType {
    /// 普通节点 (无 TEE 证明)
    StandardNode,
    /// TEE 节点 (TDX + 可选 SGX)
    TeeNode {
        /// TDX Trust Domain 度量值 (48 bytes)
        mrtd: [u8; 48],
        /// SGX Enclave 度量值 (32 bytes, 可选)
        mrenclave: Option<[u8; 32]>,
        /// TDX Quote 提交区块
        tdx_attested_at: u64,
        /// SGX Quote 提交区块
        sgx_attested_at: Option<u64>,
        /// 证明过期区块
        expires_at: u64,
    },
}

/// Bot 注册信息
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct BotInfo<T: Config> {
    pub owner: T::AccountId,
    pub bot_id_hash: [u8; 32],
    pub public_key: [u8; 32],
    pub status: BotStatus,
    pub registered_at: BlockNumberFor<T>,
    pub node_type: NodeType,
    /// 绑定的社区列表数量
    pub community_count: u32,
}

/// Bot 状态
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum BotStatus { Active, Suspended, Deactivated }

/// 社区绑定记录
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct CommunityBinding<T: Config> {
    pub community_id_hash: [u8; 32],
    pub platform: Platform,
    pub bot_id_hash: [u8; 32],
    pub bound_by: T::AccountId,
    pub bound_at: BlockNumberFor<T>,
}

/// TEE 证明记录
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct AttestationRecord<T: Config> {
    pub bot_id_hash: [u8; 32],
    /// TDX Quote 摘要 (完整 Quote 离链存储)
    pub tdx_quote_hash: [u8; 32],
    /// SGX Quote 摘要 (可选)
    pub sgx_quote_hash: Option<[u8; 32]>,
    /// MRTD
    pub mrtd: [u8; 48],
    /// MRENCLAVE (可选)
    pub mrenclave: Option<[u8; 32]>,
    /// 提交者
    pub attester: T::AccountId,
    /// 提交区块
    pub attested_at: BlockNumberFor<T>,
    /// 过期区块
    pub expires_at: BlockNumberFor<T>,
    /// 是否双证明
    pub is_dual_attestation: bool,
}
```

### 4.2 Storage

```rust
/// Bot 注册表: bot_id_hash → BotInfo
pub type Bots<T> = StorageMap<_, Blake2_128Concat, [u8; 32], BotInfo<T>>;

/// 所有者的 Bot 列表: owner → Vec<bot_id_hash>
pub type OwnerBots<T> = StorageMap<_, Blake2_128Concat, T::AccountId,
    BoundedVec<[u8; 32], T::MaxBotsPerOwner>, ValueQuery>;

/// 社区绑定: community_id_hash → CommunityBinding
pub type CommunityBindings<T> = StorageMap<_, Blake2_128Concat, [u8; 32], CommunityBinding<T>>;

/// 用户平台身份绑定: (account, platform) → platform_user_id_hash
pub type UserPlatformBindings<T> = StorageDoubleMap<_,
    Blake2_128Concat, T::AccountId,
    Blake2_128Concat, Platform, [u8; 32]>;

/// TEE 证明记录: bot_id_hash → AttestationRecord
pub type Attestations<T> = StorageMap<_, Blake2_128Concat, [u8; 32], AttestationRecord<T>>;

/// 审批的 MRTD 白名单: mrtd → version
pub type ApprovedMrtd<T> = StorageMap<_, Blake2_128Concat, [u8; 48], u32>;

/// 审批的 MRENCLAVE 白名单: mrenclave → version
pub type ApprovedMrenclave<T> = StorageMap<_, Blake2_128Concat, [u8; 32], u32>;

/// Bot 总数
pub type BotCount<T> = StorageValue<_, u64, ValueQuery>;
```

### 4.3 Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    /// 单个所有者最大 Bot 数
    type MaxBotsPerOwner: Get<u32>;
    /// 单个社区最大平台绑定数
    type MaxPlatformsPerCommunity: Get<u32>;
    /// TEE 证明有效期 (区块数)
    type AttestationValidityBlocks: Get<BlockNumberFor<Self>>;
    /// 社区管理 pallet (trait 接口)
    type CommunityProvider: CommunityProvider<Self::AccountId>;
}
```

### 4.4 Extrinsics (10 个)

| call_index | 函数 | 权限 | 说明 |
|---|---|---|---|
| 0 | `register_bot` | signed | 注册 Bot (bot_id_hash, public_key) |
| 1 | `update_public_key` | signed (owner) | 更换 Bot 公钥 (密钥轮换) |
| 2 | `deactivate_bot` | signed (owner) | 停用 Bot |
| 3 | `bind_community` | signed (owner) | 绑定社区到 Bot |
| 4 | `unbind_community` | signed (owner) | 解绑社区 |
| 5 | `bind_user_platform` | signed | 用户绑定平台身份 |
| 6 | `submit_attestation` | signed | 提交 TEE 双证明 (TDX+SGX Quote) |
| 7 | `refresh_attestation` | signed | 刷新 TEE 证明 (24h 周期) |
| 8 | `approve_mrtd` | root | 审批 MRTD 到白名单 |
| 9 | `approve_mrenclave` | root | 审批 MRENCLAVE 到白名单 |

### 4.5 Events

```rust
pub enum Event<T: Config> {
    BotRegistered { bot_id_hash: [u8; 32], owner: T::AccountId },
    PublicKeyUpdated { bot_id_hash: [u8; 32], new_key: [u8; 32] },
    BotDeactivated { bot_id_hash: [u8; 32] },
    CommunityBound { community_id_hash: [u8; 32], bot_id_hash: [u8; 32] },
    CommunityUnbound { community_id_hash: [u8; 32] },
    UserPlatformBound { account: T::AccountId, platform: Platform },
    AttestationSubmitted { bot_id_hash: [u8; 32], is_dual: bool },
    AttestationRefreshed { bot_id_hash: [u8; 32] },
    AttestationExpired { bot_id_hash: [u8; 32] },
    MrtdApproved { mrtd: [u8; 48], version: u32 },
    MrenclaveApproved { mrenclave: [u8; 32], version: u32 },
}
```

### 4.6 Helper Functions

```rust
impl<T: Config> Pallet<T> {
    /// Bot 是否已注册且活跃
    pub fn is_bot_active(bot_id_hash: &[u8; 32]) -> bool;
    /// Bot 是否为 TEE 节点
    pub fn is_tee_node(bot_id_hash: &[u8; 32]) -> bool;
    /// Bot 是否有 SGX 双证明
    pub fn has_dual_attestation(bot_id_hash: &[u8; 32]) -> bool;
    /// TEE 证明是否在有效期内
    pub fn is_attestation_fresh(bot_id_hash: &[u8; 32]) -> bool;
    /// 获取 Bot 所有者
    pub fn bot_owner(bot_id_hash: &[u8; 32]) -> Option<T::AccountId>;
}
```

### 4.7 Hooks

```rust
fn on_initialize(n: BlockNumberFor<T>) -> Weight {
    // 每 AttestationCheckInterval 扫描过期证明
    // 过期 → BotInfo.node_type 降级为 StandardNode
    // 发出 AttestationExpired 事件
}
```

---

## 5. pallet-grouprobot-consensus

> **职责: 节点质押 + TEE 加权奖励 + 消息去重 + 订阅管理 + Equivocation**
>
> 整合现有 `pallet-bot-consensus` 全部功能，简化多节点 Gossip 相关逻辑

### 5.1 Types

```rust
/// 节点信息
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct ProjectNode<T: Config> {
    pub operator: T::AccountId,
    pub node_id: NodeId,
    pub status: NodeStatus,
    pub reputation: u32,
    pub uptime_blocks: u64,
    pub stake: BalanceOf<T>,
    pub registered_at: BlockNumberFor<T>,
    pub last_active: BlockNumberFor<T>,
    /// 是否为 TEE 节点 (通过 registry pallet 查询)
    pub is_tee_node: bool,
}

/// 节点状态
pub enum NodeStatus { Active, Probation, Suspended, Exiting }

/// 暂停原因
pub enum SuspendReason { LowReputation, Equivocation, Offline, Manual }

/// 订阅层级
pub enum SubscriptionTier { Basic, Pro, Enterprise }

/// 订阅信息
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct Subscription<T: Config> {
    pub owner: T::AccountId,
    pub bot_id_hash: [u8; 32],
    pub tier: SubscriptionTier,
    pub fee_per_era: BalanceOf<T>,
    pub started_at: BlockNumberFor<T>,
    pub paid_until_era: u64,
    pub status: SubscriptionStatus,
}

/// Equivocation 证据
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct EquivocationRecord<T: Config> {
    pub owner: T::AccountId,
    pub sequence: u64,
    pub msg_hash_a: [u8; 32],
    pub signature_a: [u8; 64],
    pub msg_hash_b: [u8; 32],
    pub signature_b: [u8; 64],
    pub reporter: T::AccountId,
    pub reported_at: BlockNumberFor<T>,
    pub resolved: bool,
}
```

### 5.2 Storage

```rust
/// 节点表: node_id → ProjectNode
pub type Nodes<T> = StorageMap<_, Blake2_128Concat, NodeId, ProjectNode<T>>;

/// 活跃节点列表
pub type ActiveNodeList<T> = StorageValue<_, BoundedVec<NodeId, T::MaxActiveNodes>, ValueQuery>;

/// 消息去重: (bot_id_hash, sequence) → 处理区块
pub type ProcessedSequences<T> = StorageDoubleMap<_,
    Blake2_128Concat, [u8; 32],
    Blake2_128Concat, u64,
    BlockNumberFor<T>>;

/// 订阅表: (owner, bot_id_hash) → Subscription
pub type Subscriptions<T> = StorageDoubleMap<_,
    Blake2_128Concat, T::AccountId,
    Blake2_128Concat, [u8; 32],
    Subscription<T>>;

/// Equivocation 记录
pub type EquivocationRecords<T> = StorageDoubleMap<_,
    Blake2_128Concat, T::AccountId,
    Blake2_128Concat, u64,
    EquivocationRecord<T>>;

/// Era 信息
pub type CurrentEra<T> = StorageValue<_, u64, ValueQuery>;
pub type EraStartBlock<T> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

/// 节点待领取奖励
pub type NodePendingRewards<T> = StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

/// TEE 奖励倍数 (basis points: 10000=1.0x, 15000=1.5x)
pub type TeeRewardMultiplier<T> = StorageValue<_, u32, ValueQuery>;

/// SGX 双证明额外奖励 (basis points)
pub type SgxEnclaveBonus<T> = StorageValue<_, u32, ValueQuery>;
```

### 5.3 Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: ReservableCurrency<Self::AccountId>;
    /// 最大活跃节点数
    type MaxActiveNodes: Get<u32>;
    /// 最小质押额
    type MinStake: Get<BalanceOf<Self>>;
    /// Era 长度 (区块数)
    type EraLength: Get<BlockNumberFor<Self>>;
    /// 每 Era 通胀铸币
    type InflationPerEra: Get<BalanceOf<Self>>;
    /// Bot 注册查询 (trait 接口)
    type BotRegistry: BotRegistryProvider<Self::AccountId>;
    /// 社区查询
    type CommunityProvider: CommunityProvider<Self::AccountId>;
}
```

### 5.4 Extrinsics (12 个)

| call_index | 函数 | 权限 | 说明 |
|---|---|---|---|
| 0 | `register_node` | signed | 注册节点 + 质押 |
| 1 | `request_exit` | signed (operator) | 申请退出 (冷却期) |
| 2 | `finalize_exit` | signed (operator) | 完成退出 + 退还质押 |
| 3 | `report_equivocation` | signed | 举报 Equivocation |
| 4 | `slash_equivocation` | root | 执行 Slash |
| 5 | `subscribe` | signed (bot owner) | 订阅 Bot 服务 |
| 6 | `deposit_subscription` | signed | 充值订阅 |
| 7 | `cancel_subscription` | signed | 取消订阅 |
| 8 | `change_tier` | signed | 变更订阅层级 |
| 9 | `claim_rewards` | signed (operator) | 领取节点奖励 |
| 10 | `mark_sequence_processed` | signed | 标记消息序列已处理 (去重) |
| 11 | `set_node_tee_status` | signed (operator) | 设置节点 TEE 状态 |

### 5.5 Hooks

```rust
fn on_initialize(n: BlockNumberFor<T>) -> Weight {
    // Era 边界检测 → on_era_end()
    //   1. 收取订阅费 (从 escrow unreserve)
    //   2. 拆分: TeeOnly 群收入 → TEE 独占池; 其余 → 混合池
    //   3. 铸币通胀
    //   4. 按权重分配: compute_node_weight() × tee_multiplier
    //   5. 记录 EraRewardInfo
}
```

### 5.6 TEE 加权奖励公式

```rust
fn compute_effective_weight(node: &ProjectNode<T>) -> u128 {
    let base = node.reputation as u128 * uptime_factor(node) as u128
        * leader_bonus(node) as u128 / 1_000_000_000_000;

    let tee_factor = if node.is_tee_node {
        let mut f = TeeRewardMultiplier::<T>::get(); // 15000 = 1.5x
        if T::BotRegistry::has_dual_attestation(&node.operator) {
            f = f.saturating_add(SgxEnclaveBonus::<T>::get()); // +1000
        }
        if T::BotRegistry::is_attestation_fresh(&node.operator) {
            f = f.saturating_add(500); // freshness bonus
        }
        f
    } else {
        10_000 // 1.0x
    };

    base * tee_factor as u128 / 10_000
}
```

---

## 6. pallet-grouprobot-community

> **职责: 社区管理 + 群规则配置 + 节点准入策略 + 动作日志**
>
> 整合现有 `pallet-bot-group-mgmt` + `group_config.rs` + `local_processor.rs` 链上部分

### 6.1 Types

```rust
/// 动作类型
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum ActionType {
    Kick, Ban, Mute, Warn, Unmute, Unban,
    Promote, Demote, Welcome,
    ConfigUpdate(ConfigUpdateAction),
}

/// 配置更新动作
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum ConfigUpdateAction {
    AddBlacklistWord, RemoveBlacklistWord,
    LockType, UnlockType,
    SetWelcome, SetFloodLimit, SetWarnLimit, SetWarnAction,
}

/// 节点准入策略
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum NodeRequirement {
    /// 任意节点
    Any,
    /// 仅 TEE 节点
    TeeOnly,
    /// TEE 优先 (有 TEE 时优先调度)
    TeePreferred,
    /// 最低 TEE 节点数
    MinTee(u32),
}

/// 群规则配置 (链上精简版)
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct CommunityConfig {
    /// 节点准入策略
    pub node_requirement: NodeRequirement,
    /// 是否启用防刷屏
    pub anti_flood_enabled: bool,
    /// 防刷屏阈值 (条/分钟)
    pub flood_limit: u16,
    /// 最大警告次数
    pub warn_limit: u8,
    /// 警告达限后动作
    pub warn_action: WarnAction,
    /// 是否启用欢迎消息
    pub welcome_enabled: bool,
    /// 配置版本 (CAS 乐观锁)
    pub version: u32,
}

/// 警告达限后动作
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum WarnAction { Kick, Ban, Mute }

/// 动作日志记录
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct ActionLog<T: Config> {
    pub community_id_hash: [u8; 32],
    pub action_type: ActionType,
    pub operator: T::AccountId,
    pub target_hash: [u8; 32],
    pub sequence: u64,
    pub message_hash: [u8; 32],
    pub signature: [u8; 64],
    pub block_number: BlockNumberFor<T>,
}
```

### 6.2 Storage

```rust
/// 社区配置: community_id_hash → CommunityConfig
pub type CommunityConfigs<T> = StorageMap<_, Blake2_128Concat, [u8; 32], CommunityConfig>;

/// 节点准入策略 (快速查询): community_id_hash → NodeRequirement
pub type CommunityNodeRequirement<T> = StorageMap<_,
    Blake2_128Concat, [u8; 32], NodeRequirement, ValueQuery>;

/// 动作日志: community_id_hash → BoundedVec<ActionLog>
pub type ActionLogs<T> = StorageMap<_,
    Blake2_128Concat, [u8; 32],
    BoundedVec<ActionLog<T>, T::MaxLogsPerCommunity>, ValueQuery>;

/// 日志总数
pub type LogCount<T> = StorageValue<_, u64, ValueQuery>;
```

### 6.3 Extrinsics (5 个)

| call_index | 函数 | 权限 | 说明 |
|---|---|---|---|
| 0 | `submit_action_log` | signed | 提交动作日志 (Bot 执行后上链存证) |
| 1 | `set_node_requirement` | signed (owner) | 设置社区节点准入策略 |
| 2 | `update_community_config` | signed (owner) | 更新社区群规则配置 |
| 3 | `batch_submit_logs` | signed | 批量提交动作日志 |
| 4 | `clear_expired_logs` | signed | 清理过期日志 (释放 storage) |

---

## 7. pallet-grouprobot-ceremony

> **职责: RA-TLS 仪式审计 + Ceremony Enclave 白名单 + 自动风险检测**
>
> 整合现有 `pallet-ceremony-audit` + `ceremony.rs` 链上部分

### 7.1 Types

```rust
/// 仪式状态
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum CeremonyStatus {
    Active,
    Superseded { replaced_by: [u8; 32] },
    Revoked { revoked_at: u64 },
    Expired,
}

/// 仪式记录
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct CeremonyRecord<T: Config> {
    /// Ceremony Enclave MRENCLAVE
    pub ceremony_mrenclave: [u8; 32],
    /// Shamir 门限 K
    pub k: u8,
    /// Shamir 总分片 N
    pub n: u8,
    /// Bot 公钥
    pub bot_public_key: [u8; 32],
    /// 参与节点数
    pub participant_count: u8,
    /// 参与节点 Enclave 度量列表
    pub participant_enclaves: BoundedVec<[u8; 32], T::MaxParticipants>,
    /// 发起者
    pub initiator: T::AccountId,
    /// 创建区块
    pub created_at: BlockNumberFor<T>,
    /// 仪式状态
    pub status: CeremonyStatus,
    /// 仪式有效期 (区块数)
    pub expires_at: BlockNumberFor<T>,
}

/// Ceremony Enclave 信息
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct CeremonyEnclaveInfo {
    pub version: u32,
    pub approved_at: u64,
    pub description: BoundedVec<u8, ConstU32<128>>,
}
```

### 7.2 Storage

```rust
/// 仪式记录: ceremony_hash → CeremonyRecord
pub type Ceremonies<T> = StorageMap<_, Blake2_128Concat, [u8; 32], CeremonyRecord<T>>;

/// Bot 公钥 → 当前活跃仪式哈希
pub type ActiveCeremony<T> = StorageMap<_, Blake2_128Concat, [u8; 32], [u8; 32]>;

/// 仪式历史: bot_public_key → Vec<ceremony_hash>
pub type CeremonyHistory<T> = StorageMap<_,
    Blake2_128Concat, [u8; 32],
    BoundedVec<[u8; 32], T::MaxCeremonyHistory>, ValueQuery>;

/// 审批的 Ceremony Enclave: mrenclave → CeremonyEnclaveInfo
pub type ApprovedEnclaves<T> = StorageMap<_, Blake2_128Concat, [u8; 32], CeremonyEnclaveInfo>;

/// 仪式总数
pub type CeremonyCount<T> = StorageValue<_, u64, ValueQuery>;

/// 仪式检查间隔 (区块数)
pub type CeremonyCheckInterval<T> = StorageValue<_, u32, ValueQuery>;
```

### 7.3 Extrinsics (5 个)

| call_index | 函数 | 权限 | 说明 |
|---|---|---|---|
| 0 | `record_ceremony` | signed | 记录仪式 (验证 Enclave 白名单 + Shamir 参数) |
| 1 | `revoke_ceremony` | root | 撤销仪式 (Bot 暂停, 需 re-ceremony) |
| 2 | `approve_ceremony_enclave` | root | 添加 Ceremony Enclave 到白名单 |
| 3 | `remove_ceremony_enclave` | root | 移除 Ceremony Enclave |
| 4 | `force_re_ceremony` | root | 强制触发 re-ceremony (安全事件) |

### 7.4 Hooks — 自动风险检测

```rust
fn on_initialize(n: BlockNumberFor<T>) -> Weight {
    // 每 CeremonyCheckInterval 执行:
    // 1. 检查活跃仪式对应的 TEE 节点是否 >= K → CeremonyAtRisk 事件
    // 2. 检查仪式是否过期 (180天) → CeremonyExpired 事件
    // 3. 过期仪式自动标记为 Expired
}
```

### 7.5 Events

```rust
pub enum Event<T: Config> {
    CeremonyRecorded { ceremony_hash: [u8; 32], bot_public_key: [u8; 32], k: u8, n: u8 },
    CeremonyRevoked { ceremony_hash: [u8; 32] },
    CeremonySuperseded { old_hash: [u8; 32], new_hash: [u8; 32] },
    CeremonyExpired { ceremony_hash: [u8; 32] },
    CeremonyAtRisk { ceremony_hash: [u8; 32], active_nodes: u8, required_k: u8 },
    EnclaveApproved { mrenclave: [u8; 32], version: u32 },
    EnclaveRemoved { mrenclave: [u8; 32] },
    ForcedReCeremony { ceremony_hash: [u8; 32], reason: Vec<u8> },
}
```

---

## 8. pallet-grouprobot-primitives

> **共享类型库 — 无 Storage、无 Extrinsic，纯类型 + Trait 定义**
>
> 所有子 pallet 依赖此 crate

### 8.1 共享类型

```rust
// pallets/grouprobot/primitives/src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]

/// 节点 ID (32 bytes)
pub type NodeId = [u8; 32];

/// Bot ID Hash (32 bytes)
pub type BotIdHash = [u8; 32];

/// 社区 ID Hash (32 bytes)
pub type CommunityIdHash = [u8; 32];

/// 平台枚举 (re-export)
pub use platform::Platform;

/// 节点准入策略 (re-export)
pub use community::NodeRequirement;

/// 节点类型 (re-export)
pub use registry::NodeType;
```

### 8.2 Trait 接口

```rust
/// Bot 注册查询 (consensus/ceremony 依赖 registry)
pub trait BotRegistryProvider<AccountId> {
    fn is_bot_active(bot_id_hash: &[u8; 32]) -> bool;
    fn is_tee_node(bot_id_hash: &[u8; 32]) -> bool;
    fn has_dual_attestation(bot_id_hash: &[u8; 32]) -> bool;
    fn is_attestation_fresh(bot_id_hash: &[u8; 32]) -> bool;
    fn bot_owner(bot_id_hash: &[u8; 32]) -> Option<AccountId>;
    fn bot_public_key(bot_id_hash: &[u8; 32]) -> Option<[u8; 32]>;
}

/// 社区管理查询 (consensus 依赖 community)
pub trait CommunityProvider<AccountId> {
    fn get_node_requirement(community_id_hash: &[u8; 32]) -> NodeRequirement;
    fn is_community_bound(community_id_hash: &[u8; 32]) -> bool;
}

/// 仪式查询 (registry 可选依赖 ceremony)
pub trait CeremonyProvider {
    fn is_ceremony_active(bot_public_key: &[u8; 32]) -> bool;
    fn ceremony_shamir_params(bot_public_key: &[u8; 32]) -> Option<(u8, u8)>;
}

/// 节点共识查询 (community 可选依赖 consensus)
pub trait NodeConsensusProvider<AccountId> {
    fn is_node_active(node_id: &NodeId) -> bool;
    fn node_operator(node_id: &NodeId) -> Option<AccountId>;
    fn is_tee_node_by_operator(operator: &AccountId) -> bool;
}
```

---

## 9. 离链组件

### 9.1 nexus-tee-bot (离链 Bot Client)

离链 Bot Client 保持为独立 Rust 二进制，不纳入 Pallet。通过 `subxt` 与链上 Pallet 交互。

```
nexus-tee-bot/
├── src/
│   ├── main.rs              ← 统一入口
│   ├── config.rs            ← 配置管理
│   ├── enclave_bridge.rs    ← SGX Enclave 桥接 (8 ecall)
│   ├── enclave/             ← SGX Enclave 代码 (~500行)
│   ├── attestation.rs       ← TDX+SGX 双证明生成
│   ├── signer.rs            ← Ed25519 签名 (通过 Enclave)
│   ├── shamir.rs            ← Shamir 分片/恢复 (GF(256))
│   ├── ceremony.rs          ← RA-TLS 仪式客户端
│   ├── chain_client.rs      ← subxt 链交互 (调用 grouprobot pallet)
│   ├── metrics.rs           ← Prometheus 监控
│   ├── executor.rs          ← Telegram API 执行
│   ├── discord_executor.rs  ← Discord API 执行
│   ├── webhook.rs           ← HTTP Webhook 处理
│   ├── local_processor.rs   ← 本地消息处理 + 规则引擎
│   ├── local_store.rs       ← 本地状态缓存
│   ├── group_config.rs      ← 群配置管理
│   ├── types.rs             ← 共享类型
│   └── ...
```

### 9.2 chain_client.rs 调用映射

| Bot 操作 | 调用的 Pallet Extrinsic |
|---|---|
| 启动注册 | `registry::register_bot` |
| 提交双证明 | `registry::submit_attestation` |
| 刷新证明 (24h) | `registry::refresh_attestation` |
| 消息去重 | `consensus::mark_sequence_processed` |
| 提交动作日志 | `community::submit_action_log` |
| 记录仪式 | `ceremony::record_ceremony` |

### 9.3 Off-chain Worker (可选)

```rust
// 在 registry pallet 中实现 OCW
fn offchain_worker(block_number: BlockNumberFor<T>) {
    // 1. 扫描即将过期的 TEE 证明 → 通知 Bot 刷新
    // 2. 检查 MRTD/MRENCLAVE 白名单更新
}
```

---

## 10. 与现有 Pallet 的迁移关系

### 10.1 迁移映射表

| 现有 Pallet | 现有 Storage | 目标 grouprobot 子模块 | 目标 Storage |
|---|---|---|---|
| `bot-registry` | `RegisteredBots` | registry | `Bots` |
| `bot-registry` | `CommunityBindings` | registry | `CommunityBindings` |
| `bot-registry` | `UserPlatformBindings` | registry | `UserPlatformBindings` |
| `bot-registry` | `ApprovedMrtd` | registry | `ApprovedMrtd` |
| `bot-consensus` | `Nodes` | consensus | `Nodes` |
| `bot-consensus` | `ActiveNodeList` | consensus | `ActiveNodeList` |
| `bot-consensus` | `ProcessedSequences` | consensus | `ProcessedSequences` |
| `bot-consensus` | `Subscriptions` | consensus | `Subscriptions` |
| `bot-consensus` | `EquivocationRecords` | consensus | `EquivocationRecords` |
| `bot-consensus` | `NodePendingRewards` | consensus | `NodePendingRewards` |
| `bot-group-mgmt` | `ActionLogs` | community | `ActionLogs` |
| `bot-group-mgmt` | `CommunityNodeRequirement` | community | `CommunityNodeRequirement` |
| `ceremony-audit` | `Ceremonies` | ceremony | `Ceremonies` |
| `ceremony-audit` | `ApprovedEnclaves` | ceremony | `ApprovedEnclaves` |

### 10.2 Storage Migration 策略

```rust
// 每个子 pallet 实现 storage migration
pub mod v2 {
    use super::*;

    pub struct MigrateFromBotRegistry<T>(PhantomData<T>);

    impl<T: Config> OnRuntimeUpgrade for MigrateFromBotRegistry<T> {
        fn on_runtime_upgrade() -> Weight {
            // 1. 读取旧 pallet-bot-registry 的 RegisteredBots
            // 2. 转换为新 BotInfo 格式 (添加 node_type 字段)
            // 3. 写入新 grouprobot-registry 的 Bots
            // 4. 清理旧 storage
        }
    }
}
```

---

## 11. Runtime 集成

### 11.1 Runtime 配置

```rust
// runtime/src/configs/mod.rs

impl pallet_grouprobot_registry::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxBotsPerOwner = ConstU32<20>;
    type MaxPlatformsPerCommunity = ConstU32<5>;
    type AttestationValidityBlocks = AttestationValidityBlocks; // 43200 (24h)
    type CommunityProvider = GroupRobotCommunity;
}

impl pallet_grouprobot_consensus::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxActiveNodes = ConstU32<100>;
    type MinStake = MinNodeStake; // 1000 NEX
    type EraLength = EraLength; // DAYS
    type InflationPerEra = InflationPerEra; // 100 NEX
    type BotRegistry = GroupRobotRegistry;
    type CommunityProvider = GroupRobotCommunity;
}

impl pallet_grouprobot_community::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxLogsPerCommunity = ConstU32<10000>;
    type BotRegistry = GroupRobotRegistry;
}

impl pallet_grouprobot_ceremony::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxParticipants = ConstU32<20>;
    type MaxCeremonyHistory = ConstU32<10>;
    type CeremonyValidityBlocks = CeremonyValidityBlocks; // 180 days
    type BotRegistry = GroupRobotRegistry;
}
```

### 11.2 Pallet Index 分配

```rust
// runtime/src/lib.rs — construct_runtime!

#[runtime::pallet_index(150)]
pub type GroupRobotRegistry = pallet_grouprobot_registry;

#[runtime::pallet_index(151)]
pub type GroupRobotConsensus = pallet_grouprobot_consensus;

#[runtime::pallet_index(152)]
pub type GroupRobotCommunity = pallet_grouprobot_community;

#[runtime::pallet_index(153)]
pub type GroupRobotCeremony = pallet_grouprobot_ceremony;
```

### 11.3 目录结构

```
pallets/grouprobot/
├── primitives/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs           (Types + Traits, ~300 行)
├── registry/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           (Pallet 主体, ~800 行)
│       ├── mock.rs
│       └── tests.rs         (~40 tests)
├── consensus/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           (Pallet 主体, ~1200 行)
│       ├── mock.rs
│       └── tests.rs         (~50 tests)
├── community/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           (Pallet 主体, ~600 行)
│       ├── mock.rs
│       └── tests.rs         (~20 tests)
├── ceremony/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           (Pallet 主体, ~500 行)
│       ├── mock.rs
│       └── tests.rs         (~20 tests)
└── README.md
```

---

## 12. 开发路线图

### Phase 1: primitives + registry (5 天)

```
Sprint GR-1 (2天): primitives + 基础 registry
├── pallets/grouprobot/primitives/ — 共享类型 + Trait 接口
├── pallets/grouprobot/registry/ — Bot 注册 + 平台绑定
├── 迁移: 从 pallet-bot-registry 迁移 Storage
└── 测试: 40+ tests

Sprint GR-2 (3天): TEE 证明管理
├── registry: submit_attestation / refresh_attestation
├── registry: MRTD + MRENCLAVE 白名单
├── registry: on_initialize 过期扫描
└── 测试: +20 tests
```

### Phase 2: consensus (5 天)

```
Sprint GR-3 (3天): 节点管理 + 奖励
├── pallets/grouprobot/consensus/ — 节点注册/质押/奖励
├── TEE 加权奖励: TeeRewardMultiplier + SgxEnclaveBonus
├── 迁移: 从 pallet-bot-consensus 迁移 Storage
└── 测试: 30+ tests

Sprint GR-4 (2天): 去重 + 订阅
├── consensus: ProcessedSequences + mark_sequence_processed
├── consensus: 订阅管理 (subscribe/deposit/cancel/change_tier)
├── consensus: on_era_end() 奖励分配
└── 测试: +20 tests
```

### Phase 3: community + ceremony (4 天)

```
Sprint GR-5 (2天): 社区管理
├── pallets/grouprobot/community/ — 群规则 + 动作日志 + 节点准入
├── 迁移: 从 pallet-bot-group-mgmt 迁移 Storage
└── 测试: 20+ tests

Sprint GR-6 (2天): 仪式审计
├── pallets/grouprobot/ceremony/ — 仪式记录 + 白名单 + 风险检测
├── ceremony: on_initialize 自动风险检测
├── 迁移: 从 pallet-ceremony-audit 迁移 Storage
└── 测试: 20+ tests
```

### Phase 4: 集成 + 迁移 (3 天)

```
Sprint GR-7 (3天): Runtime 集成
├── runtime 配置: 4 个 pallet Config
├── construct_runtime 注册: index 150-153
├── Storage Migration: v2 迁移脚本
├── 旧 pallet 移除 (保留兼容期)
├── nexus-tee-bot chain_client 适配新 pallet
└── 全量回归测试: 150+ tests
```

### 工期总结

| Phase | 工期 | 新增代码 | 测试数 |
|---|---|---|---|
| Phase 1: primitives + registry | 5 天 | ~1,100 行 | ~60 |
| Phase 2: consensus | 5 天 | ~1,200 行 | ~50 |
| Phase 3: community + ceremony | 4 天 | ~1,100 行 | ~40 |
| Phase 4: 集成 + 迁移 | 3 天 | ~500 行 | ~20 |
| **合计** | **17 天** | **~3,900 行** | **~170** |

---

## 13. 测试策略

### 13.1 单元测试 (每个子 pallet)

```rust
// 每个子 pallet 独立 mock.rs + tests.rs
// 使用 construct_runtime! 构造测试环境
// 通过 mock struct 实现 trait 依赖

// 示例: registry 测试
#[test] fn register_bot_works() { ... }
#[test] fn submit_attestation_works() { ... }
#[test] fn attestation_expires_on_initialize() { ... }
#[test] fn approve_mrtd_requires_root() { ... }
```

### 13.2 集成测试 (跨 pallet)

```rust
// 构造完整 mock runtime 包含所有 4 个子 pallet
// 测试跨 pallet 的交互:
#[test] fn tee_node_gets_higher_reward() { ... }
#[test] fn tee_only_community_rejects_standard_node() { ... }
#[test] fn ceremony_at_risk_when_nodes_below_k() { ... }
```

### 13.3 测试覆盖目标

| 子 Pallet | 测试数 | 覆盖重点 |
|---|---|---|
| registry | ~60 | 注册/证明/白名单/过期/绑定 |
| consensus | ~50 | 质押/奖励/去重/订阅/Equivocation |
| community | ~20 | 群规则/动作日志/节点准入 |
| ceremony | ~20 | 仪式记录/撤销/风险检测 |
| 集成 | ~20 | 跨 pallet 交互 |
| **合计** | **~170** | |

---

## 附录 A: Cargo.toml 依赖关系

```toml
# pallets/grouprobot/primitives/Cargo.toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
sp-runtime = { workspace = true }

# pallets/grouprobot/registry/Cargo.toml
[dependencies]
grouprobot-primitives = { path = "../primitives", default-features = false }
# + substrate deps

# pallets/grouprobot/consensus/Cargo.toml
[dependencies]
grouprobot-primitives = { path = "../primitives", default-features = false }
# + substrate deps

# pallets/grouprobot/community/Cargo.toml
[dependencies]
grouprobot-primitives = { path = "../primitives", default-features = false }
# + substrate deps

# pallets/grouprobot/ceremony/Cargo.toml
[dependencies]
grouprobot-primitives = { path = "../primitives", default-features = false }
# + substrate deps
```

## 附录 B: 与现有 Pallet 的 call_index 兼容

为保证过渡期兼容，新 pallet 的 `call_index` 分配与旧 pallet **不重叠**。旧 pallet 在迁移完成后通过 runtime upgrade 移除。

## 附录 C: 术语对照

| 旧名称 | 新名称 | 说明 |
|---|---|---|
| `pallet-bot-registry` | `pallet-grouprobot-registry` | Bot 注册 + TEE 证明 |
| `pallet-bot-consensus` | `pallet-grouprobot-consensus` | 节点共识 + 奖励 |
| `pallet-bot-group-mgmt` | `pallet-grouprobot-community` | 社区管理 |
| `pallet-ceremony-audit` | `pallet-grouprobot-ceremony` | 仪式审计 |
| `nexus-tee-bot` | `nexus-tee-bot` (不变) | 离链 Bot Client |
| `nexus-agent` | (废弃) | 被 nexus-tee-bot 替代 |
| `nexus-node` | (废弃) | 被 nexus-tee-bot 替代 |
