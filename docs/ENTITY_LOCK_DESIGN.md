# Entity Global Lock Design — `None + lock = 全局冻结`

> 版本: v1.0 | 日期: 2026-03-04

## 1. 概述

当 Entity Owner 调用 `lock_governance` 且治理模式为 `None`（或未配置）时，实体进入**永久全局冻结**状态：

- **所有 Owner 配置操作**永久不可用
- **无 DAO 提案**可恢复（None 模式无投票机制）
- **不可逆** — 一旦锁定，永远无法解锁
- **Root/Sudo** 仍保留紧急权力（ban_entity 等）

### 适用场景

| 场景 | 说明 |
|------|------|
| 慈善/公益实体 | 配置完成后永久冻结，防止管理员挪用或篡改规则 |
| 自动化佣金网络 | 佣金规则确定后锁定，保证参与者利益不受变更 |
| 信任声明 | Owner 主动放弃控制权，向会员证明"规则永远不变" |

### 与 FullDAO + lock 的对比

| | None + lock | FullDAO + lock |
|--|-------------|----------------|
| Owner 操作 | ❌ 全部锁定 | ❌ 全部锁定 |
| 提案修改 | ❌ 不可能（无 DAO） | ✅ 通过提案投票修改 |
| 可恢复性 | 永久冻结 | 提案可调整参数 |
| Root 权力 | ✅ 保留 | ✅ 保留 |

---

## 2. 设计原则

1. **最小侵入** — 通过已有 `EntityProvider` trait 传递锁定状态，下游 pallet 无需新增 Config bound
2. **显式分类** — 每个 extrinsic 明确标注 🔒LOCKED 或 ✅EXEMPT，无隐式行为
3. **统一错误** — 所有 pallet 使用 `EntityLocked` 错误码（各 pallet 独立定义，语义一致）
4. **豁免最小化** — 仅资金注入和用户自发操作豁免，Owner 配置操作全部锁定

---

## 3. 架构

### 3.1 数据流

```
GovernanceLocked<T> (pallet-entity-governance storage)
        │
        ▼
GovernanceProvider::is_governance_locked(entity_id) -> bool
        │  (trait in pallet-entity-common, impl by pallet-entity-governance)
        ▼
EntityProvider::is_entity_locked(entity_id) -> bool   ← 新增
        │  (trait in pallet-entity-common, impl by pallet-entity-registry)
        │  (registry 委托给 GovernanceProvider)
        ▼
所有下游 pallet 已有 T::EntityProvider，直接调用
```

### 3.2 Trait 变更

#### pallet-entity-common — EntityProvider 新增方法

```rust
pub trait EntityProvider<AccountId> {
    // ... 已有方法 ...

    /// 实体是否被全局锁定（governance lock 生效时返回 true）
    ///
    /// 锁定后所有 Owner 配置操作被拒绝。
    /// 默认返回 false（向后兼容未实现的 mock/测试）。
    fn is_entity_locked(entity_id: u64) -> bool {
        let _ = entity_id;
        false
    }
}
```

#### pallet-entity-registry — EntityProvider impl

```rust
// Config 新增:
type GovernanceProvider: GovernanceProvider;

// EntityProvider impl:
fn is_entity_locked(entity_id: u64) -> bool {
    T::GovernanceProvider::is_governance_locked(entity_id)
}
```

#### Runtime wiring

```rust
// runtime/src/configs/mod.rs
impl pallet_entity_registry::Config for Runtime {
    // ... 已有 ...
    type GovernanceProvider = EntityGovernance;  // ← 新增
}
```

### 3.3 下游 Pallet 使用模式

```rust
// 每个 pallet 的 owner-only extrinsic 入口添加:
pub fn some_owner_action(origin, entity_id, ...) -> DispatchResult {
    let who = ensure_signed(origin)?;
    // 🔒 全局锁定检查（在 owner 权限检查之后）
    ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
    // ... 原有逻辑 ...
}
```

---

## 4. Extrinsic 分类（完整清单）

### 4.1 pallet-entity-registry

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 1 | `update_entity` | Owner/Admin | 🔒 | 修改实体元数据 |
| 3 | `close_entity` | Owner | 🔒 | 结构性变更 |
| 5 | `top_up_fund` | Owner/Admin | ✅ | **豁免**: 资金注入，维持实体运营 |
| 11 | `transfer_ownership` | Owner | 🔒 | 权限变更 |
| 12 | `upgrade_entity_type` | Owner/Governance | 🔒 | 结构性变更 |
| 15 | `add_admin` | Owner | 🔒 | 权限变更 |
| 16 | `remove_admin` | Owner | 🔒 | 权限变更 |
| 17 | `update_admin_permissions` | Owner | 🔒 | 权限变更 |
| 18 | `set_referral_code` | Owner | 🔒 | 配置变更 |
| 19 | `cancel_close_entity` | Owner | 🔒 | 结构性变更 |
| 20 | `set_entity_metadata` | Owner/Admin | 🔒 | 配置变更 |
| 21 | `owner_pause_entity` | Owner | 🔒 | 状态变更（Root 仍可 pause_entity） |
| 22 | `owner_resume_entity` | Owner | 🔒 | 状态变更（Root 仍可 resume_entity） |
| 14 | `reopen_entity` | Owner | 🔒 | 结构性变更 |

> **top_up_fund 豁免理由**: 充值仅增加 Entity 派生账户余额，不改变任何配置或权限。锁定充值会导致实体因运营资金耗尽而被强制暂停，违背"永久运行"的设计意图。

### 4.2 pallet-entity-shop

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `create_shop` | Owner | 🔒 | 结构性变更 |
| 3 | `add_shop_admin` | Owner | 🔒 | 权限变更 |
| 4 | `remove_shop_admin` | Owner | 🔒 | 权限变更 |
| 8 | `close_shop` | Owner | 🔒 | 结构性变更 |
| 10 | `withdraw_shop_funds` | Owner | 🔒 | 资金提取 |
| 12 | `transfer_shop` | Owner | 🔒 | 结构性变更 |
| 13 | `set_primary_shop` | Owner | 🔒 | 配置变更 |
| 14 | `change_shop_type` | Owner | 🔒 | 配置变更 |
| 15 | `cancel_close_shop` | Owner | 🔒 | 结构性变更 |

> Shop 管理操作（update_shop, pause_shop 等）如果由 Manager 发起，不受锁定影响。仅 Owner 路径被阻断。

### 4.3 pallet-entity-token

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `create_shop_token` | Owner | 🔒 | 创建代币 |
| 1 | `update_token_config` | Owner | 🔒 | 配置变更 |
| 2 | `mint_tokens` | Owner | 🔒 | 资产发行 |
| 3 | `configure_dividend` | Owner | 🔒 | 配置变更 |
| 4 | `distribute_dividend` | Owner | 🔒 | 资产操作 |
| 9 | `change_token_type` | Owner | 🔒 | 配置变更 |
| 10 | `set_max_supply` | Owner | 🔒 | 配置变更 |
| 11 | `set_transfer_restriction` | Owner | 🔒 | 配置变更 |
| 12 | `add_to_whitelist` | Owner | 🔒 | 配置变更 |
| 13 | `remove_from_whitelist` | Owner | 🔒 | 配置变更 |
| 14 | `add_to_blacklist` | Owner | 🔒 | 配置变更 |
| 15 | `remove_from_blacklist` | Owner | 🔒 | 配置变更 |

> 用户操作（transfer_tokens, redeem_for_discount, lock_tokens, unlock_tokens, claim_dividend）不受影响。

### 4.4 pallet-entity-commission-core

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `set_commission_modes` | Owner | 🔒 | 配置变更 |
| 1 | `set_commission_rate` | Owner | 🔒 | 配置变更 |
| 2 | `enable_commission` | Owner | 🔒 | 配置变更 |
| 3 | `withdraw_commission` | Member | ✅ | 用户操作 |
| 4 | `set_withdrawal_config` | Owner | 🔒 | 配置变更 |
| 7 | `set_platform_fee_rate` | Root | ✅ | Root 操作 |
| 8 | `withdraw_unallocated` | Owner | 🔒 | 资金提取 |
| 9 | `withdraw_unallocated_tokens` | Owner | 🔒 | 资金提取 |
| 10 | `set_token_withdrawal_config` | Owner | 🔒 | 配置变更 |
| 11 | `withdraw_token_commission` | Member | ✅ | 用户操作 |

> **withdraw_unallocated 锁定理由**: 沉淀池资金属于实体生态，Owner 锁定后不应再提取。

### 4.5 pallet-entity-commission-team

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `set_team_performance_config` | Owner/Admin | 🔒 | 配置变更 |
| 1 | `clear_team_performance_config` | Owner/Admin | 🔒 | 配置变更 |
| 2 | `update_team_performance_config` | Owner/Admin | 🔒 | 配置变更 |
| 3 | `force_set_team_performance_config` | Root | ✅ | Root 操作 |
| 4 | `force_clear_team_performance_config` | Root | ✅ | Root 操作 |

### 4.6 commission 其他子 pallet（referral / multi-level / level-diff / single-line / pool-reward）

| Pallet | 所有 extrinsic 权限 | 锁定 | 理由 |
|--------|---------------------|:----:|------|
| referral | Root | ✅ | 全部 ensure_root，不受 entity lock 影响 |
| multi-level | Root | ✅ | 全部 ensure_root |
| level-diff | Root | ✅ | 全部 ensure_root |
| single-line | Root | ✅ | 全部 ensure_root |
| pool-reward | Root（+ claim 是用户操作） | ✅ | Root / 用户操作 |

### 4.7 pallet-entity-market

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 4 | `configure_market` | Owner | 🔒 | 配置变更 |
| 15 | `configure_price_protection` | Owner | 🔒 | 配置变更 |
| 16 | `lift_circuit_breaker` | Owner | 🔒 | 状态变更 |
| 17 | `set_initial_price` | Owner | 🔒 | 配置变更 |

> 交易操作（place_order, cancel_order, verify_usdt 等）不受影响。

### 4.8 pallet-entity-tokensale

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `create_sale_round` | Owner/Admin | 🔒 | 创建发售 |
| 1 | `add_payment_option` | Creator | 🔒 | 配置变更 |
| 2 | `set_vesting_config` | Creator | 🔒 | 配置变更 |
| 3 | `configure_dutch_auction` | Creator | 🔒 | 配置变更 |
| 4 | `add_to_whitelist` | Creator | 🔒 | 配置变更 |
| 5 | `start_sale` | Creator | 🔒 | 状态变更 |
| 6 | `subscribe` | User | ✅ | 用户操作 |
| 7 | `end_sale` | Creator | 🔒 | 状态变更 |
| 8 | `claim_tokens` | User | ✅ | 用户操作 |
| 9 | `unlock_tokens` | User | ✅ | 用户操作 |
| 10 | `cancel_sale` | Creator | 🔒 | 状态变更 |
| 11 | `claim_refund` | User | ✅ | 用户操作 |
| 12 | `withdraw_funds` | Creator | 🔒 | 资金提取 |
| 13 | `reclaim_unclaimed_tokens` | Creator | 🔒 | 资金提取 |

> **注意**: tokensale 的 Creator 权限检查为 `round.creator == who`，需额外从 round 关联到 entity_id 再检查 lock。

### 4.9 pallet-entity-member

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `register_member` | User | ✅ | 用户操作 |
| 1 | `bind_referrer` | User | ✅ | 用户操作 |
| 4 | `init_level_system` | Owner/Admin | 🔒 | 配置变更 |
| 5 | `add_custom_level` | Owner/Admin | 🔒 | 配置变更 |
| 6 | `update_custom_level` | Owner/Admin | 🔒 | 配置变更 |
| 7 | `remove_custom_level` | Owner/Admin | 🔒 | 配置变更 |
| 8 | `manual_set_member_level` | Owner/Admin | 🔒 | 管理操作 |
| 9 | `set_use_custom_levels` | Owner/Admin | 🔒 | 配置变更 |
| 10 | `set_upgrade_mode` | Owner/Admin | 🔒 | 配置变更 |
| 11 | `init_upgrade_rule_system` | Owner/Admin | 🔒 | 配置变更 |
| 12 | `add_upgrade_rule` | Owner/Admin | 🔒 | 配置变更 |
| 13 | `update_upgrade_rule` | Owner/Admin | 🔒 | 配置变更 |
| 14 | `remove_upgrade_rule` | Owner/Admin | 🔒 | 配置变更 |
| 15 | `set_upgrade_rule_system_enabled` | Owner/Admin | 🔒 | 配置变更 |
| 16 | `set_conflict_strategy` | Owner/Admin | 🔒 | 配置变更 |
| 17 | `set_member_policy` | Owner/Admin | 🔒 | 配置变更 |
| 18 | `approve_member` | Owner/Admin | 🔒 | 管理操作 |
| 19 | `reject_member` | Owner/Admin | 🔒 | 管理操作 |
| 20 | `set_member_stats_policy` | Owner/Admin | 🔒 | 配置变更 |
| 21 | `cancel_pending_member` | User (applicant) | ✅ | 用户操作 |
| 22 | `cleanup_expired_pending` | Anyone | ✅ | 公共清理 |
| 23 | `batch_approve_members` | Owner/Admin | 🔒 | 管理操作 |
| 24 | `batch_reject_members` | Owner/Admin | 🔒 | 管理操作 |
| 25 | `ban_member` | Owner/Admin | 🔒 | 管理操作 |
| 26 | `unban_member` | Owner/Admin | 🔒 | 管理操作 |

> **approve/reject_member 锁定理由**: 会员审批属于管理权力。锁定后，实体应预先设置 `policy_bits` 为开放注册（无需审批），否则新会员无法加入。**Owner 锁定前必须确认 member_policy 已配置妥当。**

### 4.10 pallet-entity-kyc

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `submit_kyc` | User | ✅ | 用户操作 |
| 6 | `set_entity_requirement` | Owner/Admin | 🔒 | 配置变更 |
| 8 | `expire_kyc` | Anyone | ✅ | 公共清理 |
| 9 | `cancel_kyc` | User | ✅ | 用户操作 |
| 1-5, 7, 10-15 | Provider/Admin ops | Root/Admin | ✅ | 非 Owner 操作 |

### 4.11 pallet-entity-governance

| call_index | Extrinsic | 权限 | 锁定 | 理由 |
|:----------:|-----------|------|:----:|------|
| 0 | `configure_governance` | Owner | 🔒 | 已有 GovernanceConfigIsLocked 检查 |
| 10 | `lock_governance` | Owner | 🔒 | 已有 GovernanceAlreadyLocked 检查 |
| 1-9, 11-13 | Proposal/Vote ops | Member/Owner | ✅ | DAO 操作（None 模式下本身不可用） |

---

## 5. 统计总览

| 分类 | 数量 | 说明 |
|------|------|------|
| 🔒 LOCKED | **~73** | Owner/Admin 配置、权限、结构、资金提取操作 |
| ✅ EXEMPT（用户） | ~15 | register_member, subscribe, claim, withdraw_commission 等 |
| ✅ EXEMPT（Root） | ~20 | ensure_root 操作（commission 子 pallet、admin KYC 等） |
| ✅ EXEMPT（公共） | ~5 | cleanup, expire 等任何人可调用 |
| ✅ EXEMPT（资金注入） | 1 | top_up_fund |

---

## 6. 实现步骤

### Phase 1: 基础设施（2 文件）

#### Step 1.1: pallet-entity-common — EntityProvider 新增方法

```rust
// pallets/entity/common/src/lib.rs — EntityProvider trait
fn is_entity_locked(entity_id: u64) -> bool {
    let _ = entity_id;
    false  // 默认不锁定（向后兼容）
}
```

#### Step 1.2: pallet-entity-registry — Config + impl

```rust
// pallets/entity/registry/src/lib.rs — Config trait 新增
type GovernanceProvider: pallet_entity_common::GovernanceProvider;

// EntityProvider impl 新增
fn is_entity_locked(entity_id: u64) -> bool {
    T::GovernanceProvider::is_governance_locked(entity_id)
}
```

#### Step 1.3: Runtime wiring

```rust
// runtime/src/configs/mod.rs
impl pallet_entity_registry::Config for Runtime {
    type GovernanceProvider = EntityGovernance;
    // ...
}
```

### Phase 2: 各 Pallet 添加锁定检查（8 pallet）

每个 pallet 的变更模式相同：

1. **新增错误码**: `EntityLocked` — "实体已被全局锁定，所有配置操作不可用"
2. **在每个 🔒 extrinsic 中添加检查**:

```rust
// 在 owner 权限验证之后、业务逻辑之前:
ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
```

#### 各 Pallet 改动量估算

| Pallet | 🔒 extrinsic 数 | 新增 Error | entity_id 获取方式 |
|--------|:---------------:|:----------:|-------------------|
| registry | 13 | EntityLocked | 直接参数 |
| shop | 9 | EntityLocked | `shop.entity_id` 从 Shop 存储获取 |
| token | 12 | EntityLocked | 直接参数 |
| commission-core | 7 | EntityLocked | 直接参数 |
| commission-team | 3 | EntityLocked | 直接参数 |
| market | 4 | EntityLocked | 直接参数 |
| tokensale | 10 | EntityLocked | `round.entity_id` 从 SaleRound 获取 |
| member | 18 | EntityLocked | `shop_entity_id(shop_id)` |
| kyc | 1 | EntityLocked | 直接参数 |

### Phase 3: Mock 更新（8 pallet）

每个 pallet 的 mock `EntityProvider` 实现新增:

```rust
fn is_entity_locked(entity_id: u64) -> bool {
    // 使用 thread_local 控制测试状态
    ENTITY_LOCKED.with(|m| m.borrow().contains(&entity_id))
}
```

### Phase 4: 测试（8 pallet）

每个 pallet 至少新增 2 个测试:

1. `entity_locked_rejects_owner_action` — 锁定后 owner 操作被拒绝
2. `entity_locked_allows_exempt_action` — 锁定后豁免操作仍可执行

---

## 7. 特殊场景与边界条件

### 7.1 锁定前的前置检查

`lock_governance` 应在锁定前**警告**（但不阻止）以下情况:

| 条件 | 后果 | 建议 |
|------|------|------|
| `member_policy` 包含 APPROVAL_REQUIRED | 新会员永远无法被审批通过 | 锁定前改为开放注册 |
| 运营资金不足 | 实体可能被暂停 | 锁定前充足 top_up_fund |
| 有未完成的 tokensale 轮次 | Creator 无法 end_sale/cancel_sale | 锁定前完成或取消所有轮次 |
| 有待关闭的 Shop | 无法 cancel 或完成关闭 | 锁定前处理完毕 |

> **设计决策**: 这些检查作为**前端提示**（链下），不在链上强制。Owner 有权选择接受这些后果。

### 7.2 Admin 操作的处理

Entity Lock 同时阻止 **Admin** 的操作（Admin 的权限来源于 Owner，Owner 锁定 = Admin 也被锁定）。

例外: Root/Governance Origin 的操作不受影响（ensure_root 路径绑定的是链级治理，不是实体级权限）。

### 7.3 tokensale Creator 的处理

tokensale 的权限检查为 `round.creator == who`，不直接使用 `entity_owner`。需要:

```rust
// tokensale 中，通过 round.entity_id 检查锁定状态
let round = SaleRounds::<T>::get(round_id).ok_or(Error::<T>::RoundNotFound)?;
ensure!(!T::EntityProvider::is_entity_locked(round.entity_id), Error::<T>::EntityLocked);
```

### 7.4 withdraw_shop_funds vs top_up_fund

| 操作 | 方向 | 锁定? | 理由 |
|------|------|:-----:|------|
| `top_up_fund` | 外部 → Entity | ✅ 豁免 | 注入资金，维持运营 |
| `withdraw_shop_funds` | Shop → Owner | 🔒 锁定 | 提取资金，改变财务状态 |
| `withdraw_unallocated` | 沉淀池 → Owner | 🔒 锁定 | 提取沉淀池资金 |

### 7.5 lift_circuit_breaker 锁定

市场熔断后 Owner 无法手动解除。但熔断有到期时间（`circuit_breaker_until`），到期后市场自动恢复交易。因此锁定 `lift_circuit_breaker` 不会导致市场永久停摆。

### 7.6 owner_pause_entity 锁定

Owner 锁定后无法紧急暂停实体。但:
- Root 仍可通过 `ban_entity` 或 governance 的 `pause_entity` 处理紧急情况
- 这符合"Owner 完全放弃控制权"的语义

---

## 8. 新增类型 / 错误码

### 每个受影响 pallet 新增:

```rust
#[pallet::error]
pub enum Error<T> {
    // ... 已有错误 ...
    /// 实体已被全局锁定，配置操作不可用
    EntityLocked,
}
```

> 各 pallet 独立定义 `EntityLocked`，而非共享错误类型。这符合 Substrate pallet 设计规范（各 pallet 的 Error 类型独立）。

---

## 9. 影响范围汇总

### 修改文件清单

| 文件 | 变更类型 |
|------|---------|
| `pallets/entity/common/src/lib.rs` | EntityProvider 新增 `is_entity_locked` |
| `pallets/entity/registry/src/lib.rs` | Config 新增 GovernanceProvider + impl |
| `pallets/entity/registry/src/mock.rs` | Mock GovernanceProvider |
| `pallets/entity/shop/src/lib.rs` | 9 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/token/src/lib.rs` | 12 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/commission/core/src/lib.rs` | 7 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/commission/team/src/lib.rs` | 3 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/market/src/lib.rs` | 4 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/tokensale/src/lib.rs` | 10 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/member/src/lib.rs` | 18 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/kyc/src/lib.rs` | 1 处 lock 检查 + EntityLocked 错误 |
| `pallets/entity/governance/src/lib.rs` | 无变更（已有锁定检查） |
| `runtime/src/configs/mod.rs` | GovernanceProvider wiring |
| 8× `mock.rs` | is_entity_locked mock |
| 8× `tests.rs` | 每 pallet 至少 +2 测试 |
| **总计** | **~27 文件** |

### 新增 lock 检查点: **77 处**

### 新增测试: **至少 16 个**（8 pallet × 2）

---

## 10. 迁移 / 兼容性

- **存储迁移**: 无。锁定状态复用 `GovernanceLocked<T>` 存储，无新增存储。
- **call_index 稳定**: 无新增 extrinsic，仅在现有 extrinsic 中添加检查。
- **向后兼容**: `is_entity_locked()` 默认返回 `false`，未升级的 mock / 测试不受影响。
- **已锁定实体**: 升级后已锁定的实体立即进入全局冻结状态。

---

## 11. 测试策略

### 集成测试（governance pallet）

```rust
#[test]
fn none_lock_triggers_global_freeze() {
    // 1. 配置 None 模式
    // 2. lock_governance
    // 3. 验证 is_governance_locked() == true
    // 4. 验证跨 pallet 的 owner 操作被拒绝（通过 EntityProvider mock）
}
```

### 各 Pallet 回归测试

```rust
#[test]
fn entity_locked_rejects_<action>() {
    // 1. 设置 entity_locked = true (via thread_local)
    // 2. 调用 owner extrinsic
    // 3. 断言 Error::EntityLocked
}

#[test]
fn entity_locked_allows_exempt_<action>() {
    // 1. 设置 entity_locked = true
    // 2. 调用豁免 extrinsic（如 top_up_fund / register_member）
    // 3. 断言成功
}
```

---

## 12. 里程碑

| 阶段 | 内容 | 预计测试数 |
|------|------|-----------|
| Phase 1 | EntityProvider trait + registry impl + runtime wiring | +2 |
| Phase 2a | registry + shop + token lock checks | +6 |
| Phase 2b | commission-core + team + market lock checks | +6 |
| Phase 2c | tokensale + member + kyc lock checks | +6 |
| Phase 3 | 全量 cargo test + cargo check nexus-runtime | — |
| **Total** | | **+20** |
