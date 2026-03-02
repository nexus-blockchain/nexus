# pallet-entity-commission

> Entity 返佣管理模块 — 插件化架构，支持 NEX + Token 双资产多选返佣

## 概述

`pallet-entity-commission` 是 Entity 商城系统的返佣管理模块，采用**插件化架构**，由 1 个核心引擎 + 4 个返佣插件 + 1 个沉淀池插件 + 1 个共享类型库组成。Entity 可同时启用多种返佣模式，返佣按固定顺序叠加计算，同时支持 **NEX 和 Entity Token 双资产**返佣。

## 模块结构

```
pallet-entity-commission/          ← re-export wrapper
├── common/                        ← 共享类型 + trait 定义
├── core/                          ← 调度引擎 + 记账 + 提现 + 偿付安全
├── referral/                      ← 推荐链返佣（5 种子模式）
├── level-diff/                    ← 等级极差返佣
├── single-line/                   ← 单线收益（上线/下线）
├── team/                          ← 团队业绩阶梯奖金
└── pool-reward/                   ← 沉淀池奖励（未分配佣金周期性分配）
```

| 子模块 | Crate 名称 | 说明 |
|--------|------------|------|
| [common](common/README.md) | `pallet-commission-common` | 共享类型、枚举、trait（CommissionPlugin / MemberProvider / PlanWriter 等） |
| [core](core/README.md) | `pallet-commission-core` | 核心调度引擎：配置管理、`process_commission` 分发、提现系统、偿付安全 |
| [referral](referral/README.md) | `pallet-commission-referral` | 推荐链返佣：直推(DirectReward)、多级(MultiLevel)、固定金额(FixedAmount)、首单(FirstOrder)、复购(RepeatPurchase) |
| [level-diff](level-diff/README.md) | `pallet-commission-level-diff` | 等级极差返佣：基于自定义等级差价，沿推荐链分配 |
| [single-line](single-line/README.md) | `pallet-commission-single-line` | 单线收益：基于全局消费注册顺序的上线/下线佣金 |
| [team](team/README.md) | `pallet-commission-team` | 团队业绩返佣：基于团队累计销售额的阶梯奖金 |
| [pool-reward](pool-reward/README.md) | `pallet-commission-pool-reward` | 沉淀池奖励：未分配佣金周期性等额分配给高等级会员 |

## 返佣模式（可多选，位标志）

| 模式 | 位标志 | 插件 | 说明 |
|------|--------|------|------|
| `DIRECT_REWARD` | `0x01` | referral | 直推奖励 |
| `MULTI_LEVEL` | `0x02` | referral | 多级分销（N 层 + 激活条件） |
| `TEAM_PERFORMANCE` | `0x04` | team | 团队业绩阶梯奖金 |
| `LEVEL_DIFF` | `0x08` | level-diff | 等级极差 |
| `FIXED_AMOUNT` | `0x10` | referral | 固定金额 |
| `FIRST_ORDER` | `0x20` | referral | 首单奖励 |
| `REPEAT_PURCHASE` | `0x40` | referral | 复购奖励 |
| `SINGLE_LINE_UPLINE` | `0x80` | single-line | 单线上线收益 |
| `SINGLE_LINE_DOWNLINE` | `0x100` | single-line | 单线下线收益 |
| `POOL_REWARD` | `0x200` | pool-reward | 沉淀池奖励 |
| `ENTITY_REFERRAL` | — | core 内置 | 招商推荐人奖金（从平台费扣除） |

## 双来源佣金架构

每笔订单产生两个独立的佣金资金池：

```
订单完成
├── 池 A: 平台费 (platform_fee)
│   ├── 招商推荐人 → platform_fee × ReferrerShareBps(50%)
│   └── 国库 → 剩余部分
│
└── 池 B: 卖家货款 × max_commission_rate
    ├── Referral 插件 → remaining↓
    ├── LevelDiff 插件 → remaining↓
    ├── SingleLine 插件 → remaining↓
    ├── Team 插件 → remaining↓
    └── 沉淀池 ← remaining（POOL_REWARD 启用时）
```

Token 版完全对称：`process_token_commission` 使用相同流程分配 Entity Token。

## 提现系统

四种提现模式（NEX 和 Token 独立配置）：

| 模式 | 说明 |
|------|------|
| `FullWithdrawal` | 不强制复购（Governance 底线仍生效） |
| `FixedRate` | 所有会员统一复购比率 |
| `LevelBased` | 按会员等级查 default_tier / level_overrides |
| `MemberChoice` | 会员自选比率，不低于 min_repurchase_rate |

三层约束叠加：`Governance 底线 ≥ Entity 配置 ≥ 会员选择`

购物余额仅可用于订单抵扣（`do_consume_shopping_balance`），不可直接提取为 NEX。

## 安全机制

- **偿付安全** — `withdraw_entity_funds` 检查 `balance ≥ PendingTotal + ShoppingTotal + UnallocatedPool`
- **循环检测** — referral / level-diff 推荐链遍历使用 `BTreeSet<AccountId>` 防环
- **KYC 守卫** — `ParticipationGuard` trait 在提现和购物余额消费时强制合规检查
- **取消安全** — `cancel_commission` 先读后写，转账失败不修改记录状态
- **沉淀池冷却** — 关闭 `POOL_REWARD` 后 `PoolRewardWithdrawCooldown` 区块内不可提取沉淀池资金

## 安装

```toml
[dependencies]
pallet-entity-commission = { path = "pallets/entity/commission", default-features = false }

[features]
std = ["pallet-entity-commission/std"]
```

本 crate 是 re-export wrapper，各子模块也可单独引用。

## CommissionProvider Trait

供订单模块调用：

```rust
pub trait CommissionProvider<AccountId, Balance> {
    fn process_commission(
        entity_id: u64, shop_id: u64, order_id: u64,
        buyer: &AccountId, seller: &AccountId,
        order_amount: Balance, platform_fee: Balance,
        is_first_order: bool, buyer_order_count: u32,
    ) -> DispatchResult;

    fn cancel_commission(order_id: u64) -> DispatchResult;
    fn pending_commission(entity_id: u64, account: &AccountId) -> Balance;
}
```

Token 版：`TokenCommissionProvider` 提供 `process_token_commission` / `cancel_token_commission`。

## 返佣模式组合推荐

| 场景 | 推荐组合 | 说明 |
|------|----------|------|
| 社交电商 | 直推 + 多级分销 | 激励分享裂变 |
| 代理商体系 | 等级差价 + 团队业绩 | 激励代理升级和团队达标 |
| 拉新活动 | 直推 + 首单奖励 + 固定金额 | 快速拉新 |
| 复购型 | 直推 + 复购奖励 | 提高复购率 |
| 被动收益型 | 单线上线 + 单线下线 | 无需推荐即可获益 |
| 高等级回馈 | 上述任意 + 沉淀池奖励 | 未分配佣金回馈高级会员 |

## 子模块详情

各子模块的完整 API、数据结构、计算逻辑、审计记录请参阅对应 README：

- [common/README.md](common/README.md) — 共享类型与 trait
- [core/README.md](core/README.md) — 核心引擎与提现系统
- [referral/README.md](referral/README.md) — 5 种推荐链返佣模式
- [level-diff/README.md](level-diff/README.md) — 等级极差返佣
- [single-line/README.md](single-line/README.md) — 单线上下线收益
- [team/README.md](team/README.md) — 团队业绩阶梯奖金
- [pool-reward/README.md](pool-reward/README.md) — 沉淀池周期性分配
