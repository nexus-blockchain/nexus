# pallet-commission-referral

> 推荐链返佣插件 — 基于直接推荐关系的佣金计算

## 概述

`pallet-commission-referral` 是 NEXUS 返佣系统的**推荐链插件**，由 `pallet-commission-core` 调度引擎调用，根据买家的直接推荐人关系计算返佣。每个 Entity 可独立配置 4 种返佣模式（可同时启用多种），并通过一系列附加功能实现精细化控制。

### 返佣模式

| 模式 | 标志位 | 说明 |
|------|--------|------|
| **DirectReward** | `DIRECT_REWARD` | 推荐人按订单金额比例获佣（`rate` bps） |
| **FixedAmount** | `FIXED_AMOUNT` | 推荐人每单获得固定金额（NEX 计价） |
| **FirstOrder** | `FIRST_ORDER` | 被推荐人首单额外奖励（固定金额或比例，二选一） |
| **RepeatPurchase** | `REPEAT_PURCHASE` | 被推荐人累计订单数 ≥ `min_orders` 后按比例返佣 |

### 附加功能

| 编号 | 功能 | 说明 |
|------|------|------|
| F1 | 推荐人激活条件 | 推荐人需满足最低消费额 / 最低完成订单数才可获佣 |
| F2 | 返佣上限封顶 | 单笔上限 (`max_per_order`) + 推荐人累计上限 (`max_total_earned`) |
| F3 | 配置生效时间 | 配置变更延迟到指定区块后才生效 |
| F5 | 推荐关系有效期 | 按区块数 (`validity_blocks`) 或订单数 (`valid_orders`) 限制 |
| F6 | 推荐人冻结检查 | 冻结 / 暂停状态的推荐人不获佣 |
| F7 | 首单精确判定 | 使用 `completed_order_count`（排除取消/退款）判断首单 |
| F8 | 全局返佣率上限 | `MaxTotalReferralRate` 常量限制单笔总返佣占订单金额比例 |
| F9 | 完整性检查 | `integrity_test` 校验 `MaxTotalReferralRate ≤ 10000` |
| F10 | 事件粒度增强 | `ReferralConfigMode` 枚举标识每次配置变更的具体类型 |

> **注:** 多级分销 (MultiLevel) 已分离为独立 pallet `pallet-commission-multi-level`。

## 数据结构

### 主配置 — `ReferralConfig<Balance>`（per-entity）

```rust
pub struct ReferralConfig<Balance> {
    pub direct_reward: DirectRewardConfig,
    pub fixed_amount: FixedAmountConfig<Balance>,
    pub first_order: FirstOrderConfig<Balance>,
    pub repeat_purchase: RepeatPurchaseConfig,
}
```

### 四种模式配置

```rust
pub struct DirectRewardConfig {
    pub rate: u16,               // 基点 (bps)，500 = 5%，上限 10000
}

pub struct FixedAmountConfig<Balance> {
    pub amount: Balance,         // 每单固定返佣金额（NEX 计价）
}

pub struct FirstOrderConfig<Balance> {
    pub amount: Balance,         // 固定金额（use_amount=true 时使用）
    pub rate: u16,               // 比例 bps（use_amount=false 时使用）
    pub use_amount: bool,        // true=固定金额，false=按比例
}

pub struct RepeatPurchaseConfig {
    pub rate: u16,               // 基点 (bps)
    pub min_orders: u32,         // 买家最低累计订单数
}
```

### 附加配置

```rust
/// F1: 推荐人激活条件
pub struct ReferrerGuardConfig {
    pub min_referrer_spent: u128,   // 推荐人最低累计消费（0=不限）
    pub min_referrer_orders: u32,   // 推荐人最低完成订单数（0=不限）
}

/// F2: 返佣上限
pub struct CommissionCapConfig<Balance> {
    pub max_per_order: Balance,     // 单笔上限（0=不限）
    pub max_total_earned: Balance,  // 推荐人累计上限（0=不限）
}

/// F5: 推荐关系有效期
pub struct ReferralValidityConfig {
    pub validity_blocks: u32,       // 有效区块数（0=永久有效）
    pub valid_orders: u32,          // 有效订单数（0=不限）
}

/// F10: 配置变更类型枚举（事件标识）
pub enum ReferralConfigMode {
    DirectReward,
    FixedAmount,
    FirstOrder,
    RepeatPurchase,
    ReferrerGuard,
    CommissionCap,
    ReferralValidity,
}
```

## Pallet Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;

    /// 原生货币
    type Currency: Currency<Self::AccountId>;

    /// 会员查询接口（推荐人关系、封禁状态、消费统计、订单数等）
    type MemberProvider: MemberProvider<Self::AccountId>;

    /// 实体查询接口（Owner/Admin 权限、锁定/活跃状态）
    type EntityProvider: EntityProvider<Self::AccountId>;

    /// F8: 全局推荐返佣率上限（基点，10000 = 100% = 不限制）
    #[pallet::constant]
    type MaxTotalReferralRate: Get<u16>;
}
```

## Storage

| 存储项 | 键 | 值 | 说明 |
|--------|-----|-----|------|
| `ReferralConfigs` | `u64` | `ReferralConfig<Balance>` | 主配置（entity_id → config） |
| `ReferrerGuardConfigs` | `u64` | `ReferrerGuardConfig` | F1: 推荐人激活条件 |
| `CommissionCapConfigs` | `u64` | `CommissionCapConfig<Balance>` | F2: 返佣上限 |
| `ReferrerTotalEarned` | `(u64, AccountId)` | `Balance` | F2: 推荐人累计获佣跟踪 |
| `ConfigEffectiveAfter` | `u64` | `BlockNumber` | F3: 配置生效区块号 |
| `ReferralValidityConfigs` | `u64` | `ReferralValidityConfig` | F5: 推荐关系有效期 |

所有 `StorageMap` 使用 `Blake2_128Concat` 哈希。`ReferrerTotalEarned` 为 `StorageDoubleMap`，第一键为 `entity_id`，第二键为推荐人 `AccountId`。

清除配置时（`clear_referral_config` / `force_clear` / `ReferralPlanWriter::clear_config`），`do_clear_all_config` 统一删除上述全部 6 项存储，包括通过 `clear_prefix` 批量删除 `ReferrerTotalEarned` 的所有条目，确保无孤立数据残留。

## Extrinsics

### Owner/Admin 操作

需要 Entity Owner 或持有 `COMMISSION_MANAGE` 权限的 Admin 身份。所有 Owner/Admin extrinsic 均执行以下前置检查：
- `ensure_signed` + `ensure_owner_or_admin`
- `ensure!(!is_entity_locked)` → `EntityLocked`
- `ensure!(is_entity_active)` → `EntityNotActive`

| call_index | 方法 | 参数 | 说明 |
|------------|------|------|------|
| 0 | `set_direct_reward_config` | `entity_id, rate` | 设置直推比例（rate ≤ 10000 bps） |
| 2 | `set_fixed_amount_config` | `entity_id, amount` | 设置固定金额 |
| 3 | `set_first_order_config` | `entity_id, amount, rate, use_amount` | 设置首单奖励（rate ≤ 10000 bps） |
| 4 | `set_repeat_purchase_config` | `entity_id, rate, min_orders` | 设置复购奖励（rate ≤ 10000 bps） |
| 5 | `clear_referral_config` | `entity_id` | 清除全部配置 + 附属存储 + 累计获佣 |
| 11 | `set_referrer_guard_config` | `entity_id, min_referrer_spent, min_referrer_orders` | F1: 推荐人激活条件 |
| 12 | `set_commission_cap_config` | `entity_id, max_per_order, max_total_earned` | F2: 返佣上限 |
| 13 | `set_referral_validity_config` | `entity_id, validity_blocks, valid_orders` | F5: 推荐关系有效期 |
| 14 | `set_config_effective_after` | `entity_id, block_number` | F3: 配置生效时间 |

> `call_index(1)` 已预留（原 MultiLevel，已分离为独立 pallet）。

### Root 紧急覆写

仅 `ensure_root`，不检查实体锁定/活跃状态。

| call_index | 方法 | 参数 | 说明 |
|------------|------|------|------|
| 6 | `force_set_direct_reward_config` | `entity_id, rate` | 强制设置直推比例 |
| 7 | `force_set_fixed_amount_config` | `entity_id, amount` | 强制设置固定金额 |
| 8 | `force_set_first_order_config` | `entity_id, amount, rate, use_amount` | 强制设置首单奖励 |
| 9 | `force_set_repeat_purchase_config` | `entity_id, rate, min_orders` | 强制设置复购奖励 |
| 10 | `force_clear_referral_config` | `entity_id` | 强制清除（无配置时静默跳过，不发射事件） |

## 计算逻辑

### NEX 路径 — `CommissionPlugin::calculate()`

```
calculate(entity_id, buyer, order_amount, remaining, enabled_modes, _is_first_order, buyer_order_count)
│
├── F3: ConfigEffectiveAfter 检查 → 未到生效时间则返回空
├── 读取 ReferralConfigs → 不存在则返回空
├── F7: is_first_order = completed_order_count(buyer) == 0
│
├── [DIRECT_REWARD]  → process_direct_reward()
├── [FIXED_AMOUNT]   → process_fixed_amount()
├── [FIRST_ORDER]    → process_first_order()  (仅 is_first_order=true 时)
├── [REPEAT_PURCHASE] → process_repeat_purchase()
│
└── F8: 全局返佣率上限裁剪 (max_rate < 10000 时生效)
```

### 每个 `process_*` 函数内部流程

```
1. 零值早返回（rate==0 或 amount.is_zero()）
2. 获取推荐人 → get_referrer(entity_id, buyer)
3. is_referrer_qualified() 资格验证:
   ├── X1: is_banned → 跳过
   ├── is_activated → 跳过
   ├── F6: is_member_active → 跳过（冻结/暂停）
   ├── F1: ReferrerGuardConfig → 消费/订单数不达标则跳过
   └── F5: ReferralValidityConfig → 超期/超单数则跳过
4. 计算原始佣金 (rate * order_amount / 10000 或固定金额)
5. F2: apply_commission_cap() → 单笔上限 + 累计上限裁剪
6. min(capped, remaining) → 不超过剩余可分配额
7. track_referrer_earned() → 更新累计获佣
8. 输出 CommissionOutput { beneficiary, amount, commission_type, level: 1 }
```

### Token 路径 — `TokenCommissionPlugin::calculate_token()`

与 NEX 路径结构对称，使用泛型 `TB: AtLeast32BitUnsigned + Copy`，复用同一份 `ReferralConfig` 的 rate 配置。

**差异：**
- **跳过固定金额模式**：`FIXED_AMOUNT` 完全不参与；`FIRST_ORDER` 当 `use_amount=true` 时跳过（固定金额以 NEX 计价，不适用于 Token）
- **不应用 F2 返佣上限**：`CommissionCapConfig` / `ReferrerTotalEarned` 以 NEX 计价，不适用于 Token 路径
- **共享 F1/F3/F5/F6/F7/F8**：推荐人资格、生效时间、有效期、冻结检查、首单判定、全局率上限均与 NEX 路径一致

## Trait 实现

### `CommissionPlugin<AccountId, Balance>`

由 `pallet-commission-core` 调度引擎在 NEX 订单结算时调用。

```rust
fn calculate(
    entity_id: u64,
    buyer: &AccountId,
    order_amount: Balance,
    remaining: Balance,
    enabled_modes: CommissionModes,
    _is_first_order: bool,    // 忽略，F7 内部重新判定
    buyer_order_count: u32,
) -> (Vec<CommissionOutput<AccountId, Balance>>, Balance);
```

### `TokenCommissionPlugin<AccountId, TB>`

由 `pallet-commission-core` 在 Token 订单结算时调用。泛型 `TB` 支持任意 Token 精度。

```rust
fn calculate_token(
    entity_id: u64,
    buyer: &AccountId,
    order_amount: TB,
    remaining: TB,
    enabled_modes: CommissionModes,
    _is_first_order: bool,
    buyer_order_count: u32,
) -> (Vec<CommissionOutput<AccountId, TB>>, TB);
```

### `ReferralPlanWriter<Balance>`

供 `pallet-commission-core` 的 `init_commission_plan` 治理路径写入配置。所有方法包含防御性校验（rate ≤ 10000）并发射事件。

```rust
fn set_direct_rate(entity_id: u64, rate: u16) -> DispatchResult;
fn set_fixed_amount(entity_id: u64, amount: Balance) -> DispatchResult;
fn set_first_order(entity_id: u64, amount: Balance, rate: u16, use_amount: bool) -> DispatchResult;
fn set_repeat_purchase(entity_id: u64, rate: u16, min_orders: u32) -> DispatchResult;
fn clear_config(entity_id: u64) -> DispatchResult;  // X2: 仅存在时才清除+发事件
```

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `ReferralConfigUpdated` | `entity_id: u64, mode: ReferralConfigMode` | 配置变更（F10: mode 标识具体类型） |
| `ReferralConfigCleared` | `entity_id: u64` | 配置清除（含全部附属存储） |
| `ConfigEffectiveAfterSet` | `entity_id: u64, block_number: BlockNumber` | F3: 生效时间设置 |

## Errors

| 错误 | 触发条件 |
|------|----------|
| `InvalidRate` | rate 参数 > 10000 bps |
| `EntityNotFound` | `EntityProvider::entity_owner()` 返回 `None` |
| `NotEntityOwnerOrAdmin` | 调用者非 Entity Owner 且无 `COMMISSION_MANAGE` 权限 |
| `ConfigNotFound` | `clear_referral_config` 时 `ReferralConfigs` 不存在 |
| `EntityLocked` | 实体处于全局锁定状态 |
| `EntityNotActive` | 实体未激活（暂停/封禁/关闭） |

## 内部辅助函数

| 函数 | 可见性 | 说明 |
|------|--------|------|
| `do_clear_all_config` | `pub(crate)` | 统一清除 6 项存储（主配置 + 5 项附属 + `ReferrerTotalEarned` 前缀清除） |
| `ensure_owner_or_admin` | private | 验证 Entity Owner 或 Admin(`COMMISSION_MANAGE`) 权限 |
| `is_referrer_qualified` | `pub(crate)` | 综合验证推荐人资格（X1 + F1 + F5 + F6） |
| `apply_commission_cap` | private | F2: 单笔 + 累计上限裁剪 |
| `track_referrer_earned` | private | F2: 更新 `ReferrerTotalEarned` |

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-runtime = { workspace = true }
pallet-commission-common = { path = "../common", default-features = false }
pallet-entity-common = { path = "../../common", default-features = false }

[dev-dependencies]
sp-io = { workspace = true }
pallet-balances = { workspace = true, features = ["std"] }
```

## 测试

89 个单元测试，覆盖：

- 4 种返佣模式的正常/边界/零值场景
- Owner/Admin/Root 权限控制 + `force_*` 后备
- 推荐人封禁 (X1)、幻影事件 (X2)、权限体系 (X3)
- F1 激活条件、F2 上限封顶、F3 生效时间、F5 有效期、F6 冻结、F7 首单、F8 全局上限、F10 事件粒度
- Entity 锁定/未激活拒绝
- `ReferralPlanWriter` 事件发射 + 防御性校验
- 清除操作清理全部附属存储 + 累计获佣 (H1 + M1-R4)

```bash
cargo test -p pallet-commission-referral
```

## 审计记录

### 已修复

| ID | 级别 | 描述 |
|----|------|------|
| H1 | High | `clear` 操作仅删除主配置，附属存储成为孤立数据。修复: `do_clear_all_config` 统一清除 |
| H2 | High | `set_first_order_config` / `set_repeat_purchase_config` 未校验 rate ≤ 10000。修复: 添加 `ensure!` + PlanWriter 同步 |
| H3 | High | `process_first_order` 零值无早返回。修复: 添加 `use_amount && amount.is_zero()` / `!use_amount && rate == 0` 早返回 |
| X1 | High | 推荐人无 `is_banned` 检查。修复: `is_referrer_qualified` 统一检查 |
| M1-R2 | Medium | `ReferralPlanWriter` 5 个方法不发射事件。修复: 全部补充事件发射 |
| X2 | Medium | `clear_config` / `force_clear` 幻影事件。修复: `contains_key` 前置检查 |
| X3 | Medium | 所有 extrinsic 为 Root-only。修复: Owner/Admin (COMMISSION_MANAGE) + `force_*` Root 后备 |
| M1-R5 | Medium | `is_referrer_qualified` 无 `is_member` 检查 — 移除的会员推荐链未清理时仍可获佣。修复: 添加 `is_member` 前置检查 |
| M2-R5 | Medium | F8 全局返佣率上限裁剪后 `ReferrerTotalEarned` 未修正 — 累计上限(F2b)虚高导致推荐人提前触顶。修复: F8 裁剪时同步扣减 excess |
| L1-R5 | Low | Cargo.toml 缺 `pallet-commission-common/runtime-benchmarks` 和 `pallet-commission-common/try-runtime`。修复: 已添加 |
| M1-R3 | Medium | `new_test_ext()` 不清理 thread-local 状态。修复: 添加 `clear_thread_locals()` |
| M1-R4 | Medium | `do_clear_all_config` 未清除 `ReferrerTotalEarned`。修复: 添加 `clear_prefix` |
| L1-R2 | Low | Cargo.toml 缺 `sp-runtime` feature 传播。修复: 已添加 |
| L1-R3 | Low | `sp-std` 依赖未使用。修复: 已移除 |
| L1-R4 | Low | README 类型标注错误 (`validity_blocks: u64` → `u32`)。修复: 已更正 |
| L-weight | Low | Extrinsic 硬编码 Weight，无 WeightInfo trait。修复: 添加 `WeightInfo` trait + `weights.rs` + `benchmarking.rs`（v2 宏） |

### 记录但未修复

| ID | 级别 | 描述 |
|----|------|------|
| L-dup | Low | Token 版 `_token` 函数与 NEX 版逻辑大量重复（维护风险） |

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.1.0 | 初始版本（含 MultiLevel） |
| v0.2.0 | MultiLevel 分离为 `pallet-commission-multi-level` |
| v0.3.0 | 审计 R2: M1(事件发射) + L1(Cargo features) |
| v0.4.0 | 功能增强: X1(is_banned 守卫) + X2(幻影事件守卫) + X3(Owner/Admin 权限体系) |
| v0.5.0 | 功能增强: F1-F8, F10 |
| v0.6.0 | 审计 R3: H1(clear 清理附属存储) + M1(thread-local 清理) + L1(sp-std 移除) — 88 tests |
| v0.7.0 | 审计 R4: M1-R4(clear 清理 ReferrerTotalEarned) + L1-R4(README 类型修正) — 89 tests |
| v0.8.0 | 审计 R5: M1-R5(is_member 检查) + M2-R5(F8 裁剪修正 ReferrerTotalEarned) + L1-R5(Cargo features) — 94 tests |
| v0.9.0 | WeightInfo trait + benchmarking 框架 + 移除硬编码 Weight — 108 tests (含 14 benchmark tests) |
