# pallet-grouprobot-ceremony

> 路径：`pallets/grouprobot/ceremony/`

RA-TLS 仪式审计系统，管理 Shamir 密钥分割仪式的链上记录、Enclave 白名单、自动过期与风险检测。

## 设计理念

- **Enclave 白名单**：仪式必须使用经治理审批的 Ceremony Enclave（mrenclave）
- **Shamir 参数验证**：链上校验 k-of-n 参数合法性（k>0, k≤n, n≤254, participant_count≥k）
- **仪式替代**：新仪式自动标记旧仪式为 `Superseded`
- **自动过期**：`on_initialize` 周期扫描，过期仪式自动清除活跃状态
- **强制重仪式**：安全事件时 Root 可强制撤销并触发 re-ceremony

## Extrinsics

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `record_ceremony` | Signed | 记录仪式（验证 Enclave 白名单 + Shamir 参数，自动替代旧仪式） |
| 1 | `revoke_ceremony` | Root | 撤销仪式 |
| 2 | `approve_ceremony_enclave` | Root | 添加 Ceremony Enclave 到白名单 |
| 3 | `remove_ceremony_enclave` | Root | 移除 Ceremony Enclave |
| 4 | `force_re_ceremony` | Root | 强制 re-ceremony（安全事件响应） |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Ceremonies` | `Map<[u8;32], CeremonyRecord>` | 仪式记录（ceremony_hash → 详情） |
| `ActiveCeremony` | `Map<[u8;32], [u8;32]>` | Bot 公钥 → 当前活跃仪式哈希 |
| `CeremonyHistory` | `Map<[u8;32], BoundedVec<[u8;32]>>` | Bot 公钥 → 仪式历史列表 |
| `ApprovedEnclaves` | `Map<[u8;32], CeremonyEnclaveInfo>` | 审批的 Ceremony Enclave 白名单 |
| `CeremonyCount` | `u64` | 仪式总数 |

## 主要类型

### CeremonyRecord
```rust
pub struct CeremonyRecord<T: Config> {
    pub ceremony_mrenclave: [u8; 32],
    pub k: u8,                          // Shamir 门限
    pub n: u8,                          // Shamir 总份数
    pub bot_public_key: [u8; 32],
    pub participant_count: u8,
    pub participant_enclaves: BoundedVec<[u8; 32], T::MaxParticipants>,
    pub initiator: T::AccountId,
    pub created_at: BlockNumberFor<T>,
    pub status: CeremonyStatus,         // Active/Superseded/Revoked/Expired
    pub expires_at: BlockNumberFor<T>,
    pub is_re_ceremony: bool,           // 是否为 Re-ceremony
    pub supersedes: Option<[u8; 32]>,   // 替代的旧仪式哈希
    pub bot_id_hash: [u8; 32],          // Bot ID 哈希 (供 on_initialize 查询)
}
```

### CeremonyEnclaveInfo
```rust
pub struct CeremonyEnclaveInfo {
    pub version: u32,
    pub approved_at: u64,
    pub description: BoundedVec<u8, ConstU32<128>>,
}
```

## 错误

| 错误 | 说明 |
|------|------|
| `EnclaveNotApproved` | Enclave 未在白名单中 |
| `CeremonyNotFound` | 仪式不存在 |
| `CeremonyAlreadyRevoked` | 仪式已撤销 |
| `CeremonyAlreadyExists` | 仪式哈希已存在 |
| `InvalidShamirParams` | k=0 或 k>n 或 n>254 |
| `EnclaveAlreadyApproved` | Enclave 已在白名单 |
| `EnclaveNotFound` | Enclave 不在白名单 |
| `EmptyParticipants` | 参与者列表为空 |
| `TooManyParticipants` | 参与者超过 MaxParticipants |
| `CeremonyHistoryFull` | 仪式历史已满 |
| `FreeTierNotAllowed` | Free 层级不允许使用此功能 |
| `NotBotOwner` | 调用者不是 Bot 所有者 |
| `BotNotFound` | Bot 不存在 |
| `BotPublicKeyMismatch` | Bot 公钥不匹配 |
| `InsufficientParticipants` | 参与者数量不足以恢复 secret (< k) |
| `CeremonyNotActive` | 仪式不是活跃状态 |
| `DescriptionTooLong` | 描述超过 128 bytes |

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxParticipants` | 最大参与节点数 |
| `MaxCeremonyHistory` | 每个 Bot 仪式历史最大数 |
| `CeremonyValidityBlocks` | 仪式有效期（区块数） |
| `CeremonyCheckInterval` | 过期检查间隔（区块数） |
| `BotRegistry` | Bot 注册查询（`BotRegistryProvider`） |
| `Subscription` | 订阅层级查询（`SubscriptionProvider`，Tier gate） |

## Hooks

- **`on_initialize`**：每 `CeremonyCheckInterval` 个区块扫描活跃仪式：
  - 过期仪式标记为 `Expired` 并清除 `ActiveCeremony`，发出 `CeremonyExpired` 事件
  - 活跃仪式的 peer 数量 ≤ k 时发出 `CeremonyAtRisk` 风险事件

## Trait 实现

实现 `CeremonyProvider`，供其他子 pallet 查询：
- `is_ceremony_active(bot_public_key)` — 仪式是否活跃
- `ceremony_shamir_params(bot_public_key)` — 获取 (k, n) 参数
- `active_ceremony_hash(bot_public_key)` — 获取活跃仪式哈希
- `ceremony_participant_count(bot_public_key)` — 获取参与者数量

## 仪式状态机

```
Active → Superseded { replaced_by }  (新仪式替代)
Active → Revoked { revoked_at }      (治理撤销 / 强制 re-ceremony)
Active → Expired                     (on_initialize 自动过期)
```

## 相关模块

- [primitives/](../primitives/) — CeremonyStatus、BotRegistryProvider
- [registry/](../registry/) — Bot 注册（提供 BotRegistryProvider 实现）
