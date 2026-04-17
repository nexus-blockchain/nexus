# pallet-grouprobot-ceremony

> 路径：`pallets/grouprobot/ceremony/`

RA-TLS 仪式审计系统，管理 Shamir 密钥分割仪式的链上记录、Enclave 白名单、自动过期与 AtRisk 风险检测。

## 设计理念

- **Enclave 白名单**：仪式必须使用经治理审批的 Ceremony Enclave（mrenclave）
- **Shamir 参数验证**：链上校验 k-of-n 参数合法性（k>0, k≤n, n≤254, k≤participant_count≤n）
- **仪式替代**：新仪式自动标记旧仪式为 `Superseded`
- **Tier Gate**：Free 层级不允许发起仪式，需付费订阅
- **参与者去重**：O(n²) 去重检测在上限检查之后执行，防止无界输入 DoS
- **自动过期**：`on_initialize` 从 `ExpiryQueue` 游标化扫描，过期仪式自动清除活跃状态
- **AtRisk 检测**：peer 数量 ≤ k 时发出风险事件，游标分批避免无界迭代
- **强制重仪式**：安全事件时 Root 可强制撤销并触发 re-ceremony

## Extrinsics

| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `record_ceremony` | Signed | 记录仪式（Tier gate + Enclave 白名单 + Shamir 参数 + 去重 + 自动替代旧仪式，历史满时 FIFO） |
| 1 | `revoke_ceremony` | Root | 撤销仪式 |
| 2 | `approve_ceremony_enclave` | Root | 添加 Ceremony Enclave 到白名单 |
| 3 | `remove_ceremony_enclave` | Root | 移除 Ceremony Enclave |
| 4 | `force_re_ceremony` | Root | 强制 re-ceremony（安全事件响应） |
| 5 | `cleanup_ceremony` | Signed | M3-R6: 清理终态仪式记录（Expired/Revoked/Superseded） |
| 6 | `owner_revoke_ceremony` | Signed | F1: Bot Owner 主动撤销自己 Bot 的活跃仪式 |
| 7 | `revoke_by_mrenclave` | Root | F7: 按 mrenclave 批量撤销所有活跃仪式（漏洞响应，受 MaxProcessPerBlock 限制） |
| 8 | `trigger_expiry` | Signed | F12: 手动触发已过期仪式的状态标记（任何人可调用） |
| 9 | `batch_cleanup_ceremonies` | Signed | F11: 批量清理终态仪式（上限 MaxProcessPerBlock 条，原子性） |
| 10 | `renew_ceremony` | Signed | F2: 轻量仪式续期（仅延长 expires_at，Bot Owner + Tier gate） |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Ceremonies` | `Map<[u8;32], CeremonyRecord>` | 仪式记录（ceremony_hash → 详情） |
| `ActiveCeremony` | `Map<[u8;32], [u8;32]>` | Bot 公钥 → 当前活跃仪式哈希 |
| `CeremonyHistory` | `Map<[u8;32], BoundedVec<[u8;32]>>` | Bot 公钥 → 仪式历史列表 |
| `ApprovedEnclaves` | `Map<[u8;32], CeremonyEnclaveInfo>` | 审批的 Ceremony Enclave 白名单 |
| `CeremonyCount` | `u64` | 仪式总数 |
| `ExpiryQueue` | `BoundedVec<(BlockNumber, [u8;32], [u8;32]), 1000>` | L2-R3: 按 expires_at 排序的过期队列 |
| `AtRiskCursor` | `Option<[u8;32]>` | L2-R3: AtRisk 检测游标（分批处理） |

## 事件

| 事件 | 说明 |
|------|------|
| `CeremonyRecorded` | 仪式记录成功（含 ceremony_hash, bot_public_key, k, n） |
| `CeremonyRevoked` | 仪式已被治理撤销 |
| `CeremonySuperseded` | 旧仪式被新仪式替代（含 old_hash, new_hash） |
| `CeremonyExpired` | 仪式已过期（on_initialize 自动触发） |
| `CeremonyAtRisk` | F14: 仪式存在风险（peer 数量 ≤ k，含 bot_public_key, required_k, current_peer_count） |
| `EnclaveApproved` | Ceremony Enclave 已审批（含 mrenclave, version） |
| `EnclaveRemoved` | Ceremony Enclave 已从白名单移除 |
| `ForcedReCeremony` | 仪式已被强制 re-ceremony |
| `CeremonyCleaned` | M3-R6: 终态仪式已清理 |
| `OwnerCeremonyRevoked` | F1: Bot Owner 主动撤销仪式（含 ceremony_hash, bot_public_key） |
| `CeremoniesRevokedByMrenclave` | F7: 按 mrenclave 批量撤销（含 mrenclave, count） |
| `CeremonyManuallyExpired` | F12: 仪式被手动触发过期（含 ceremony_hash） |
| `CeremonyRenewed` | F2: 仪式续期（含 ceremony_hash, new_expires_at） |

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
| `CeremonyAlreadyExists` | 仪式哈希已存在 |
| `InvalidShamirParams` | k=0 或 k>n 或 n>254 |
| `EnclaveAlreadyApproved` | Enclave 已在白名单 |
| `EnclaveNotFound` | Enclave 不在白名单 |
| `EmptyParticipants` | 参与者列表为空 |
| `TooManyParticipants` | 参与者超过 MaxParticipants |
| `FreeTierNotAllowed` | Free 层级不允许使用此功能 |
| `NotBotOwner` | 调用者不是 Bot 所有者 |
| `BotNotFound` | Bot 不存在 |
| `BotPublicKeyMismatch` | Bot 公钥不匹配 |
| `InsufficientParticipants` | 参与者数量不足以恢复 secret (< k) |
| `CeremonyNotActive` | 仪式不是活跃状态 |
| `DescriptionTooLong` | 描述超过 128 bytes |
| `DuplicateParticipant` | M2-audit: 参与者 Enclave 列表含重复项 |
| `ExpiryQueueFull` | M1-R4: ExpiryQueue 已满 (1000 条上限) |
| `BotNotActive` | M1-R6: Bot 未激活（停用/banned） |
| `ParticipantCountExceedsN` | M2-R6: 参与者数量超过 Shamir n 参数 |
| `CeremonyNotTerminal` | M3-R6: 仪式不是终态（无法清理） |
| `CeremonyNotExpired` | F12: 仪式尚未过期（无法手动触发） |
| `NoCeremoniesToCleanup` | F11: 批量清理列表为空 |
| `TooManyCeremonies` | F11: 批量清理数量超出 MaxProcessPerBlock 上限 |

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxParticipants` | 最大参与节点数 |
| `MaxCeremonyHistory` | 每个 Bot 仪式历史最大数 |
| `CeremonyValidityBlocks` | 仪式有效期（区块数） |
| `CeremonyCheckInterval` | 过期检查间隔（区块数） |
| `BotRegistry` | Bot 注册查询（`BotRegistryProvider`） |
| `Subscription` | 订阅层级查询（`SubscriptionProvider`，Tier gate） |
| `MaxProcessPerBlock` | L2-R3: 每次 on_initialize 最多处理的仪式数 |

## Hooks

- **`on_initialize`**：每 `CeremonyCheckInterval` 个区块执行两阶段处理（L2-R3 改进）：
  - **Phase 1 — 过期队列处理**：从 `ExpiryQueue` 头部取出已过期条目（按 expires_at 升序），标记为 `Expired`，清除 `ActiveCeremony`，发出 `CeremonyExpired` 事件
  - **Phase 2 — 游标 AtRisk 检测**：从 `AtRiskCursor` 继续遍历 `ActiveCeremony`，peer 数量 ≤ k 时发出 `CeremonyAtRisk` 风险事件
  - 两阶段共享 `MaxProcessPerBlock` 预算上限，避免无界迭代

## 公共查询方法

`Pallet<T>` 提供以下公共方法供其他模块查询：
- `is_ceremony_active(bot_public_key)` — 仪式是否活跃
- `ceremony_shamir_params(bot_public_key)` — 获取 (k, n) 参数
- `get_active_ceremony(bot_public_key)` — 获取活跃仪式哈希
- `is_enclave_approved(mrenclave)` — Enclave 是否已审批
- `ceremony_health(bot_public_key)` — F3: 仪式健康状态 (expires_at, peer_count, k)
- `ceremony_expires_at(bot_public_key)` — F13: 活跃仪式过期区块
- `ceremony_participant_enclaves(bot_public_key)` — F13: 活跃仪式参与者 Enclave 列表

## 仪式状态机

```
Active → Superseded { replaced_by }  (新仪式替代)
Active → Revoked { revoked_at }      (治理撤销 / 强制 re-ceremony)
Active → Expired                     (on_initialize 自动过期)
```

## 相关模块

- [primitives/](../primitives/) — CeremonyStatus、BotRegistryProvider、SubscriptionProvider
- [registry/](../registry/) — Bot 注册（提供 BotRegistryProvider 实现 + peer_count 查询）
- [subscription/](../subscription/) — 订阅管理（提供 SubscriptionProvider 实现，Tier gate）

## 审计修复记录

| 编号 | 级别 | 修复 | 说明 |
|------|------|------|------|
| C1 | Critical | `CeremonyRecord.bot_id_hash` | 存储 bot_id_hash 以供 on_initialize peer_count 查询（避免哈希函数不一致） |
| H1 | High | `record_ceremony` | 参与者数量必须 >= k（否则无法恢复 secret） |
| H2 | High | `force_re_ceremony` | 仅活跃仪式可被强制 re-ceremony（CeremonyNotActive 错误） |
| M1-prev | Medium | `approve_ceremony_enclave` | 描述过长时返回错误而非静默截断（DescriptionTooLong） |
| CH2 | Medium | `on_initialize` | 仅迭代 ActiveCeremony (O(A)) 而非全表 Ceremonies (O(N)) |
| M1-R2 | Medium | `on_initialize` AtRisk | 移除 peer_count > 0 守卫 — 0 个 peer 意味着 secret 完全不可恢复，应同样触发 CeremonyAtRisk |
| M2-R2 | Medium | `record_ceremony` | 参与者 Enclave 不允许重复（DuplicateParticipant 错误），防止膨胀 participant_count |
| L1-R3 | Low | `revoke_ceremony` | 仅允许撤销 Active 仪式（与 `force_re_ceremony` 保持一致，使用 CeremonyNotActive 错误） |
| L2-R3 | Low | `on_initialize` | 新增 ExpiryQueue 按 expires_at 优先序 + AtRiskCursor 游标分批 + MaxProcessPerBlock 上限，消除无界迭代 |
| M1-R4 | Medium | `record_ceremony` | ExpiryQueue 满时返回 `ExpiryQueueFull` 错误（原 `let _ = try_insert` 静默丢弃，导致仪式永不自动过期） |
| M2-R4 | Medium | `record_ceremony` | CeremonyHistory 满时 FIFO 移除最旧条目（原 `CeremonyHistoryFull` 永久阻塞新仪式） |
| L1-R4 | Low | `Cargo.toml` | 移除死依赖 `sp-core` 和 `log`（lib.rs 无引用） |
| L2-R4 | Low | `Cargo.toml` | `try-runtime` feature 补充 `frame-system/try-runtime` 和 `sp-runtime/try-runtime` 传播 |
| L3-R4 | Low | `Error<T>` | 移除死错误码 `CeremonyAlreadyRevoked`（已被 `CeremonyNotActive` 替代） |
| M1-R5 | Medium | `on_initialize` | AtRisk 游标 `skip_while` 使用 `<=` 导致游标位置的仪式被永久跳过。修复: `<=` → `<` |
| M2-R5 | Medium | `record_ceremony` | O(n²) 去重检测在 `TooManyParticipants` 上限检查之前执行，无界输入可 DoS。修复: 先检查上限再去重 |
| L1-R5 | Low | `Error<T>` | 移除死错误码 `CeremonyHistoryFull`（M2-R4 FIFO 后永不返回） |
| L2-R5 | Low | README | `CeremonyProvider` trait 已从 primitives 移除，README 仍引用。修复: 更正为公共查询方法 |
| M1-R6 | Medium | `record_ceremony` | Bot 未激活时拒绝发起仪式（`BotNotActive` 错误） |
| M2-R6 | Medium | `record_ceremony` | 参与者数量不得超过 Shamir n 参数（`ParticipantCountExceedsN` 错误） |
| M3-R6 | Medium | `cleanup_ceremony` | 新增终态仪式清理 extrinsic，解决存储无界增长（call_index 5） |
| L1-R6 | Low | `revoke_ceremony`/`force_re_ceremony` | 提取 `do_revoke` 共享 helper，消除代码重复 |

## 已解决的遗留项

| 编号 | 级别 | 修复 | 说明 |
|------|------|------|------|
| L3 | Low | `weights.rs` + `benchmarking.rs` | 所有 extrinsic 已接入 `WeightInfo` trait，`record_ceremony(p)` 和 `batch_cleanup_ceremonies(n)` 按参数线性缩放 |

## 测试覆盖

88 个测试（`cargo test -p pallet-grouprobot-ceremony`）：

- **Enclave 白名单**: approve_works / fails_duplicate / fails_not_root / remove_works / remove_fails_not_found
- **record_ceremony**: works / fails_invalid_shamir / fails_enclave_not_approved / fails_duplicate / supersedes_old / fails_empty_participants / fails_not_bot_owner / fails_bot_not_found / first_is_not_re_ceremony / second_is_re_ceremony / fails_free_tier
- **revoke_ceremony**: works / fails_not_found / fails_already_revoked
- **force_re_ceremony**: works
- **on_initialize**: ceremony_expires / ceremony_not_expired_before_time
- **Helpers**: helper_ceremony_shamir_params
- **C1**: c1_record_stores_bot_id_hash
- **H1**: h1_fails_insufficient_participants / h1_ok_participants_equal_k
- **H2**: h2_rejects_already_revoked / h2_rejects_expired
- **M1-prev**: m1_rejects_too_long_description / m1_accepts_max_length_description
- **Round 2 回归**: m1_audit_peer_count_zero / m1_audit_peer_count_equal_k / at_risk_not_triggered_above_k / m2_duplicate_rejected / m2_unique_accepted / l3_bot_pk_mismatch_rejected
- **Round 3 回归 (L1)**: l1_r3_revoke_rejects_expired / l1_r3_revoke_rejects_superseded
- **Round 3 回归 (L2)**: l2_r3_expiry_queue_populated / l2_r3_expiry_queue_sorted / l2_r3_queue_cleaned_on_revoke / l2_r3_queue_cleaned_on_force / l2_r3_on_initialize_uses_expiry_queue
- **Round 4 回归 (M1)**: m1_r4_expiry_queue_full_rejects_record / m1_r4_expiry_queue_not_full_allows_record
- **Round 4 回归 (M2)**: m2_r4_ceremony_history_fifo_when_full / m2_r4_ceremony_history_fifo_preserves_order
- **Round 5 回归 (M1)**: m1_r5_at_risk_cursor_does_not_skip_boundary_ceremony
- **Round 5 回归 (M2)**: m2_r5_too_many_participants_rejected_before_dup_check / m2_r5_max_participants_boundary_accepted
- **Round 5 回归 (L1)**: l1_r5_ceremony_history_fifo_works_without_history_full_error
- **Round 6 回归 (M1)**: m1_r6_record_ceremony_rejects_inactive_bot / m1_r6_record_ceremony_allows_active_bot
- **Round 6 回归 (M2)**: m2_r6_participants_exceeding_n_rejected / m2_r6_participants_equal_n_accepted / m2_r6_participants_between_k_and_n_accepted
- **Round 6 回归 (M3)**: m3_r6_cleanup_expired_ceremony / m3_r6_cleanup_revoked_ceremony / m3_r6_cleanup_superseded_ceremony / m3_r6_cleanup_active_ceremony_rejected / m3_r6_cleanup_nonexistent_ceremony_rejected
- **F1 (owner_revoke_ceremony)**: f1_owner_revoke_ceremony_works / f1_owner_revoke_rejects_non_owner / f1_owner_revoke_rejects_not_found / f1_owner_revoke_rejects_not_active
- **F7 (revoke_by_mrenclave)**: f7_revoke_by_mrenclave_works / f7_revoke_by_mrenclave_no_match / f7_revoke_by_mrenclave_rejects_non_root
- **F12 (trigger_expiry)**: f12_trigger_expiry_works / f12_trigger_expiry_rejects_not_expired / f12_trigger_expiry_rejects_not_active / f12_trigger_expiry_rejects_not_found
- **F11 (batch_cleanup)**: f11_batch_cleanup_works / f11_batch_cleanup_rejects_empty / f11_batch_cleanup_rejects_too_many / f11_batch_cleanup_rejects_active
- **F2 (renew_ceremony)**: f2_renew_ceremony_works / f2_renew_rejects_non_owner / f2_renew_rejects_not_active / f2_renew_rejects_inactive_bot / f2_renew_prevents_expiry
- **F3 (ceremony_health)**: f3_ceremony_health_works / f3_ceremony_health_none_when_no_ceremony
- **F13 (查询方法)**: f13_ceremony_expires_at_works / f13_ceremony_participant_enclaves_works
