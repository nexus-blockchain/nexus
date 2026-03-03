# pallet-grouprobot-primitives

> 路径：`pallets/grouprobot/primitives/`

GroupRobot 模块组的共享类型库与 Trait 接口定义。无 Storage、无 Extrinsic，纯类型 + Trait。

所有 grouprobot 子 pallet（registry、ceremony、community、consensus、subscription、rewards）均依赖此 crate。

## 类型别名

| 类型 | 定义 | 说明 |
|------|------|------|
| `NodeId` | `[u8; 32]` | 节点 ID |
| `BotIdHash` | `[u8; 32]` | Bot ID 哈希 |
| `CommunityIdHash` | `[u8; 32]` | 社区 ID 哈希 |

## 枚举类型

### Platform（社交平台）
```rust
pub enum Platform {
    Telegram, Discord, Slack, Matrix, Farcaster,
}
```

### BotStatus（Bot 状态）
```rust
pub enum BotStatus {
    Active,       // 活跃
    Suspended,    // 暂停
    Deactivated,  // 停用
}
```

### TeeType（TEE 硬件类型）
```rust
pub enum TeeType {
    Tdx,          // TDX-Only
    Sgx,          // SGX-Only
    TdxPlusSgx,   // TDX + SGX 双证明
}
```

### NodeType（节点类型）
```rust
pub enum NodeType {
    StandardNode,                            // 普通节点
    TeeNode {                                // V1: TEE 节点 (向后兼容)
        mrtd: [u8; 48],
        mrenclave: Option<[u8; 32]>,
        tdx_attested_at: u64,
        sgx_attested_at: Option<u64>,
        expires_at: u64,
    },
    TeeNodeV2 {                              // V2: 三模式统一 TEE 节点
        primary_measurement: [u8; 48],       // MRTD 或 MRENCLAVE(+zero-pad)
        tee_type: TeeType,
        mrenclave: Option<[u8; 32]>,
        attested_at: u64,
        sgx_attested_at: Option<u64>,
        expires_at: u64,
    },
}
```

### OperatorStatus（运营商状态）
```rust
pub enum OperatorStatus {
    Active, Suspended, Deactivated,
}
```

### NodeStatus（节点状态）
```rust
pub enum NodeStatus {
    Active, Probation, Suspended, Exiting,
}
```

### SuspendReason（暂停原因）
```rust
pub enum SuspendReason {
    LowReputation, Equivocation, Offline, Manual,
}
```

### SubscriptionTier（订阅层级）
```rust
pub enum SubscriptionTier {
    Free,        // 免费 (默认, 无链上记录)
    Basic,       // 基础版
    Pro,         // 专业版
    Enterprise,  // 企业版
}
```

附带 `is_paid()` 和 `feature_gate()` 方法。

### TierFeatureGate（层级功能限制）
```rust
pub struct TierFeatureGate {
    pub max_rules: u16,
    pub log_retention_days: u16,       // 0 = 永久
    pub forced_ads_per_day: u8,        // 0 = 无强制
    pub can_disable_ads: bool,
    pub tee_access: bool,
}
```

### SubscriptionStatus（订阅状态）
```rust
pub enum SubscriptionStatus {
    Active, PastDue, Suspended, Cancelled,
}
```

### AdCommitmentStatus（广告承诺状态）
```rust
pub enum AdCommitmentStatus {
    Active, Underdelivery, Cancelled,
}
```

### ActionType（动作类型）
```rust
pub enum ActionType {
    Kick, Ban, Mute, Warn, Unmute, Unban, Promote, Demote, Welcome,
    ConfigUpdate(ConfigUpdateAction),
}
```

### ConfigUpdateAction（配置更新动作）
```rust
pub enum ConfigUpdateAction {
    AddBlacklistWord, RemoveBlacklistWord, LockType, UnlockType,
    SetWelcome, SetFloodLimit, SetWarnLimit, SetWarnAction,
}
```

### NodeRequirement（节点准入策略）
```rust
pub enum NodeRequirement {
    Any, TeeOnly, TeePreferred, MinTee(u32),
}
```

### WarnAction（警告达限动作）
```rust
pub enum WarnAction { Kick, Ban, Mute }
```

### AdDeliveryType（广告投放类型）
```rust
pub enum AdDeliveryType {
    ScheduledPost,   // CPM 1.0x
    ReplyFooter,     // CPM 0.5x
    WelcomeEmbed,    // CPM 0.3x
}
```

### AdTargetTag（广告目标标签）
```rust
pub enum AdTargetTag {
    TargetPlatform(Platform),
    MinMembers(u32),
    Language([u8; 2]),
    All,
}
```

### CeremonyStatus（仪式状态）
```rust
pub enum CeremonyStatus {
    Active,
    Superseded { replaced_by: [u8; 32] },
    Revoked { revoked_at: u64 },
    Expired,
}
```

## Trait 接口

所有 Trait 均提供 `()` 空实现，方便子 pallet 独立测试。

### BotRegistryProvider（Bot 注册查询）

> 实现: registry pallet | 消费: ceremony, consensus, subscription, ads-grouprobot

```rust
pub trait BotRegistryProvider<AccountId> {
    fn is_bot_active(bot_id_hash: &BotIdHash) -> bool;
    fn is_tee_node(bot_id_hash: &BotIdHash) -> bool;
    fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool;
    fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool;
    fn bot_owner(bot_id_hash: &BotIdHash) -> Option<AccountId>;
    fn bot_public_key(bot_id_hash: &BotIdHash) -> Option<[u8; 32]>;
    fn peer_count(bot_id_hash: &BotIdHash) -> u32;
    fn bot_operator(bot_id_hash: &BotIdHash) -> Option<AccountId>;
}
```

### NodeConsensusProvider（节点共识查询）

> 实现: consensus pallet | 消费: rewards, ads-grouprobot

```rust
pub trait NodeConsensusProvider<AccountId> {
    fn is_node_active(node_id: &NodeId) -> bool;
    fn node_operator(node_id: &NodeId) -> Option<AccountId>;
    fn is_tee_node_by_operator(operator: &AccountId) -> bool;
}
```

### SubscriptionProvider（订阅层级查询）

> 实现: subscription pallet | 消费: registry, consensus, ads-grouprobot

```rust
pub trait SubscriptionProvider {
    fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier;
    fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate;
}
```

### AdDeliveryProvider（广告投放计数查询）

> 实现: runtime 桥接 ads-core | 消费: subscription pallet

```rust
pub trait AdDeliveryProvider {
    fn era_delivery_count(community_id_hash: &CommunityIdHash) -> u32;
    fn reset_era_deliveries(community_id_hash: &CommunityIdHash);
}
```

### RewardAccruer（统一奖励写入）

> 实现: rewards pallet | 消费: ads-grouprobot

```rust
pub trait RewardAccruer {
    fn accrue_node_reward(node_id: &NodeId, amount: u128);
}
```

### PeerUptimeRecorder（Peer Uptime 记录）

> 实现: registry pallet | 消费: consensus on_era_end

```rust
pub trait PeerUptimeRecorder {
    fn record_era_uptime(era: u64);
}
```

### EraSettlementResult（结算结果）

```rust
pub struct EraSettlementResult {
    pub total_income: u128,
    pub treasury_share: u128,
}
```

### SubscriptionSettler（订阅结算）

> 实现: subscription pallet | 消费: consensus on_era_end

```rust
pub trait SubscriptionSettler {
    fn settle_era() -> EraSettlementResult;
}
```

### OrphanRewardClaimer（孤儿奖励领取）

> 实现: rewards pallet | 消费: consensus finalize_exit

```rust
pub trait OrphanRewardClaimer<AccountId> {
    fn try_claim_orphan_rewards(node_id: &NodeId, operator: &AccountId);
}
```

### EraRewardDistributor（Era 奖励分配）

> 实现: rewards pallet | 消费: consensus on_era_end

```rust
pub trait EraRewardDistributor {
    fn distribute_and_record(era, total_pool, subscription_income, inflation, treasury_share, node_weights, node_count) -> u128;
    fn prune_old_eras(current_era: u64);
}
```

## Re-exports (from pallet-ads-primitives)

通过 `pub use pallet_ads_primitives::{...}` 重导出以下类型，保持向后兼容:

| 重导出名 | 原名 | 类型 |
|----------|------|------|
| `CampaignStatus` | 同名 | enum |
| `AdReviewStatus` | 同名 | enum |
| `AdPreference` | 同名 | enum |
| `PlacementId` | 同名 | type alias |
| `DeliveryVerifier` | 同名 | trait |
| `PlacementAdminProvider` | 同名 | trait |
| `RevenueDistributor` | 同名 | trait |
| `PlacementStakeProvider` | 同名 | trait |
| `DeliveryMethod` | 同名 | trait |

## 依赖关系

```
                         primitives (本 crate)
                   /     |      |       |         \         \
            registry  ceremony  community  consensus  subscription  rewards
```

## 相关模块

- [registry/](../registry/) — Bot 注册 + TEE 证明
- [ceremony/](../ceremony/) — RA-TLS 仪式审计
- [community/](../community/) — 社区管理 + 声誉
- [consensus/](../consensus/) — 节点质押 + Era 编排
- [subscription/](../subscription/) — 订阅管理 + 广告承诺
- [rewards/](../rewards/) — 节点奖励池

## 审计记录

| 轮次 | 日期 | 修复 | 说明 |
|------|------|------|------|
| R1 | 2026-03-03 | L1, L2, L3 | 移除死 `extern crate alloc`; Cargo.toml 补充 try-runtime/runtime-benchmarks features; README 全面同步 |
| R1.1 | 2026-03-03 | L4, L5/M1 | 移除死别名 GenericAdScheduleProvider/GenericAdDeliveryCountProvider; 移除死 AdScheduleProvider trait (grouprobot 版, 零消费者), 消除与 ads-primitives 同名冲突 |

**记录但未修复:**
- M2: 5 个 ads-primitives 重导出 (CampaignStatus, AdReviewStatus, AdPreference, PlacementId, DeliveryMethod) 无下游消费者通过 grouprobot-primitives 路径引用 (保留向后兼容)
