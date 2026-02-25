# pallet-grouprobot-primitives

> 路径：`pallets/grouprobot/primitives/`

GroupRobot 模块组的共享类型库与 Trait 接口定义。无 Storage、无 Extrinsic，纯类型 + Trait。

所有 grouprobot 子 pallet（registry、ceremony、community、consensus）均依赖此 crate。

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
    Telegram,
    Discord,
    Slack,
    Matrix,
    Farcaster,
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

### NodeType（节点类型）
```rust
pub enum NodeType {
    StandardNode,                     // 普通节点
    TeeNode {                         // TEE 节点
        mrtd: [u8; 48],               // TDX Trust Domain 度量值
        mrenclave: Option<[u8; 32]>,  // SGX Enclave 度量值
        tdx_attested_at: u64,         // TDX 证明区块
        sgx_attested_at: Option<u64>, // SGX 证明区块
        expires_at: u64,              // 证明过期区块
    },
}
```

### NodeStatus（节点状态）
```rust
pub enum NodeStatus {
    Active,     // 活跃
    Probation,  // 观察期
    Suspended,  // 暂停
    Exiting,    // 退出中
}
```

### SubscriptionTier（订阅层级）
```rust
pub enum SubscriptionTier {
    Basic,       // 基础版
    Pro,         // 专业版
    Enterprise,  // 企业版
}
```

### SubscriptionStatus（订阅状态）
```rust
pub enum SubscriptionStatus {
    Active,     // 活跃
    PastDue,    // 欠费
    Suspended,  // 暂停
    Cancelled,  // 已取消
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
    Any,            // 任意节点
    TeeOnly,        // 仅 TEE 节点
    TeePreferred,   // TEE 优先调度
    MinTee(u32),    // 最低 TEE 节点数
}
```

### WarnAction（警告达限动作）
```rust
pub enum WarnAction { Kick, Ban, Mute }
```

### CeremonyStatus（仪式状态）
```rust
pub enum CeremonyStatus {
    Active,                              // 活跃
    Superseded { replaced_by: [u8; 32] },// 被新仪式替代
    Revoked { revoked_at: u64 },         // 已撤销
    Expired,                             // 已过期
}
```

### SuspendReason（暂停原因）
```rust
pub enum SuspendReason {
    LowReputation, Equivocation, Offline, Manual,
}
```

## Trait 接口

### BotRegistryProvider（Bot 注册查询）

> 被 ceremony、community、consensus 依赖

```rust
pub trait BotRegistryProvider<AccountId> {
    fn is_bot_active(bot_id_hash: &BotIdHash) -> bool;
    fn is_tee_node(bot_id_hash: &BotIdHash) -> bool;
    fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool;
    fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool;
    fn bot_owner(bot_id_hash: &BotIdHash) -> Option<AccountId>;
    fn bot_public_key(bot_id_hash: &BotIdHash) -> Option<[u8; 32]>;
}
```

### CommunityProvider（社区管理查询）

> 被 consensus 依赖

```rust
pub trait CommunityProvider<AccountId> {
    fn get_node_requirement(community_id_hash: &CommunityIdHash) -> NodeRequirement;
    fn is_community_bound(community_id_hash: &CommunityIdHash) -> bool;
}
```

### CeremonyProvider（仪式查询）

> 被 registry 可选依赖

```rust
pub trait CeremonyProvider {
    fn is_ceremony_active(bot_public_key: &[u8; 32]) -> bool;
    fn ceremony_shamir_params(bot_public_key: &[u8; 32]) -> Option<(u8, u8)>;
}
```

### ReputationProvider（声誉查询）

```rust
pub trait ReputationProvider {
    fn get_reputation(community_id_hash: &CommunityIdHash, user_hash: &[u8; 32]) -> i64;
    fn get_global_reputation(user_hash: &[u8; 32]) -> i64;
}
```

### NodeConsensusProvider（节点共识查询）

> 被 community 可选依赖

```rust
pub trait NodeConsensusProvider<AccountId> {
    fn is_node_active(node_id: &NodeId) -> bool;
    fn node_operator(node_id: &NodeId) -> Option<AccountId>;
    fn is_tee_node_by_operator(operator: &AccountId) -> bool;
}
```

所有 Trait 均提供 `()` 空实现，方便子 pallet 独立测试。

## 依赖关系

```
                    primitives (本 crate)
                   /     |      \       \
            registry  ceremony  community  consensus
```

## 相关模块

- [registry/](../registry/) — Bot 注册 + TEE 证明
- [ceremony/](../ceremony/) — RA-TLS 仪式审计
- [community/](../community/) — 社区管理 + 声誉
- [consensus/](../consensus/) — 节点质押 + 奖励
