# pallet-commission-referral

> 推荐链返佣插件 — 直推奖励、固定金额、首单奖励、复购奖励

## 概述

`pallet-commission-referral` 是返佣系统的**推荐链插件**，基于会员直接推荐关系计算返佣，包含 4 种返佣模式：

- **直推奖励** (DirectReward) — 直接推荐人获得比例返佣
- **固定金额** (FixedAmount) — 每单固定金额返佣给推荐人
- **首单奖励** (FirstOrder) — 被推荐人首单时给推荐人额外奖励（固定金额或比例）
- **复购奖励** (RepeatPurchase) — 被推荐人达到最低订单数后按比例返佣给推荐人

> 注: 多级分销 (MultiLevel) 已分离为独立 pallet: `pallet-commission-multi-level`

## 数据结构

### ReferralConfig — 推荐链返佣总配置（per-entity）

```rust
pub struct ReferralConfig<Balance> {
    pub direct_reward: DirectRewardConfig,
    pub fixed_amount: FixedAmountConfig<Balance>,
    pub first_order: FirstOrderConfig<Balance>,
    pub repeat_purchase: RepeatPurchaseConfig,
}
```

### 各模式配置

```rust
/// 直推奖励 — 推荐人获得 rate 基点的返佣
pub struct DirectRewardConfig {
    pub rate: u16,  // 基点，500 = 5%
}

/// 固定金额 — 每单固定金额
pub struct FixedAmountConfig<Balance> {
    pub amount: Balance,
}

/// 首单奖励 — 固定金额或比例
pub struct FirstOrderConfig<Balance> {
    pub amount: Balance,
    pub rate: u16,
    pub use_amount: bool,  // true=使用固定金额, false=使用比例
}

/// 复购奖励 — 达到最低订单数后按比例返佣
pub struct RepeatPurchaseConfig {
    pub rate: u16,
    pub min_orders: u32,
}
```

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;
    type EntityProvider: EntityProvider<Self::AccountId>;
}
```

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ReferralConfigs` | `Map<u64, ReferralConfig>` | 推荐链返佣配置（entity_id → config） |

## Extrinsics

### Owner/Admin 操作（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_direct_reward_config` | Owner/Admin | 设置直推奖励率（rate ≤ 10000 bps） |
| 2 | `set_fixed_amount_config` | Owner/Admin | 设置固定金额 |
| 3 | `set_first_order_config` | Owner/Admin | 设置首单奖励（rate ≤ 10000 bps） |
| 4 | `set_repeat_purchase_config` | Owner/Admin | 设置复购奖励（rate ≤ 10000 bps） |
| 5 | `clear_referral_config` | Owner/Admin | 清除推荐链配置（配置必须存在） |

### Root 紧急覆写

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 6 | `force_set_direct_reward_config` | Root | 强制设置直推奖励率 |
| 7 | `force_set_fixed_amount_config` | Root | 强制设置固定金额 |
| 8 | `force_set_first_order_config` | Root | 强制设置首单奖励 |
| 9 | `force_set_repeat_purchase_config` | Root | 强制设置复购奖励 |
| 10 | `force_clear_referral_config` | Root | 强制清除推荐链配置（幻影事件守卫） |

> call_index 1 已移除（原为 MultiLevel，现分离为 `pallet-commission-multi-level`）。
> 配置也可通过 `ReferralPlanWriter` trait 进行设置（治理/core pallet 路径）。

## 计算逻辑

### 插件调度顺序

```
CommissionPlugin::calculate()
├── 1. 直推奖励（DIRECT_REWARD 位启用时）
├── 2. 固定金额（FIXED_AMOUNT 位启用时）
├── 3. 首单奖励（FIRST_ORDER 位启用且 is_first_order 时）
└── 4. 复购奖励（REPEAT_PURCHASE 位启用时）
```

- 每种模式从 `remaining` 额度中扣除，避免超发
- 所有模式仅查找买家的直接推荐人（单层）
- 无推荐人时跳过，不产生输出
- **推荐人被封禁时跳过** — `is_banned` 检查，与 team/level-diff 插件一致 (X1)

## Token 多资产支持

提供与 NEX 版对称的 Token 计算函数（泛型 `TB: AtLeast32BitUnsigned`），复用同一份 `ReferralConfig` 的 rate 配置：

- `process_direct_reward_token` / `process_first_order_token` / `process_repeat_purchase_token`
- **固定金额模式不生效**：`FIXED_AMOUNT` 和 `FIRST_ORDER`（`use_amount=true`）为 NEX 计价，Token 版跳过

## Trait 实现

- **`CommissionPlugin`** — 由 core 调度引擎调用，配置按 `entity_id` 查询
- **`TokenCommissionPlugin`** — Token 多资产返佣计算，复用 NEX 配置的 rate 参数
- **`ReferralPlanWriter`** — 供 core 的 `init_commission_plan` 写入配置，包含防御性校验 + 事件发射

## Events

| 事件 | 说明 |
|------|------|
| `ReferralConfigUpdated` | 推荐链配置更新（extrinsic + PlanWriter 路径均发射） |
| `ReferralConfigCleared` | 推荐链配置清除（PlanWriter clear_config 路径发射） |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 返佣率超过 10000 基点 |
| `EntityNotFound` | 实体不存在 |
| `NotEntityOwnerOrAdmin` | 非实体所有者或无 COMMISSION_MANAGE 权限 |
| `ConfigNotFound` | 配置不存在（清除时） |

## 审计记录

### 已修复

| ID | 级别 | 描述 |
|----|------|------|
| H2 | High | `set_first_order_config` / `set_repeat_purchase_config` 未校验 rate 上限。修复: 添加 `ensure!(rate <= 10000)` + PlanWriter 路径同步 |
| H3 | High | `process_first_order` 零值无早返回，浪费存储读取。修复: 添加 `use_amount && amount.is_zero()` / `!use_amount && rate == 0` 早返回 |
| M1-R2 | Medium | `ReferralPlanWriter` 5个方法不发射事件 — governance 路径静默修改存储。修复: 所有 set 方法发射 `ReferralConfigUpdated`，`clear_config` 发射 `ReferralConfigCleared` |
| M2-R2 | Medium | README 严重过时 — 仍描述已分离的 MultiLevel。修复: 全面重写 |
| L1-R2 | Low | Cargo.toml 缺 `sp-runtime/runtime-benchmarks` 和 `sp-runtime/try-runtime`。修复: 已添加 |
| L2-R2 | Low | `pallet-entity-common` 死 dev-dependency。修复: 已移除 |

### 记录但未修复

| ID | 级别 | 描述 |
|----|------|------|
| L-weight | Low | 4 个 extrinsic 硬编码 Weight，无 WeightInfo trait（需完整 benchmark 框架） |
| L-dup | Low | Token 版 `_token` 函数与 NEX 版逻辑大量重复（维护风险） |
| X1 | High | 推荐人无 `is_banned` 检查 — 被封禁会员仍获返佣。修复: 7 个 process 函数添加 `is_banned` 守卫 |
| X2 | Medium | `clear_config` / `force_clear` 幻影事件 — 配置不存在时仍发射 `ReferralConfigCleared`。修复: 添加 `contains_key` 前置检查 |
| X3 | Medium | 所有 extrinsic 为 Root-only — Entity Owner 无法自主管理。修复: 日常操作改为 Owner/Admin (COMMISSION_MANAGE)，新增 `force_*` Root 后备 |

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.1.0 | 初始版本（含 MultiLevel） |
| v0.2.0 | MultiLevel 分离为 `pallet-commission-multi-level` |
| v0.3.0 | 深度审计 Round 2: M1(事件发射) + M2(README) + L1(Cargo) + L2(死依赖) |
| v0.4.0 | Phase 1 功能增强: X1(is_banned守卫) + X2(幻影事件守卫) + X3(Owner/Admin权限+force_*) — 55 tests |

## 依赖

```toml
[dependencies]
pallet-commission-common = { path = "../common" }
pallet-entity-common = { path = "../../common" }
```
