# pallet-commission-pool-reward

> 沉淀池奖励插件 — 未分配佣金回馈高等级会员

## 概述

`pallet-commission-pool-reward` 是返佣系统的**沉淀池奖励插件**。当启用 `POOL_REWARD` 模式后，每笔订单中未被其他插件（Referral / LevelDiff / SingleLine / Team）分配的佣金余额，不再留在卖家账户，而是自动转入 **Entity 级沉淀资金池**。后续订单触发时，插件从池中取出一部分，以领奖奖金形式发放给买家推荐链上的高等级会员。

**核心约束：Entity Owner 不可提取沉淀池资金，资金完全由算法驱动分配。**

## 设计动机

```
现有问题：
  订单佣金预算 (max_commission) 经 4 个插件分配后，剩余部分 (remaining) 留在卖家账户
  → 这部分资金没有被有效利用

沉淀池方案：
  remaining → 沉淀资金池 → 高等级会员奖金
  → 形成 "消费 → 沉淀 → 奖励高等级 → 激励升级 → 更多消费" 的正向循环
```

## 两阶段调度模型

插件与 `pallet-commission-core` 的 `process_commission` 协作，分两个阶段运行：

```
订单触发 process_commission:

Phase 1（卖家资金池 — 现有逻辑不变）
  ┌─ ReferralPlugin ──→ remaining↓
  ├─ LevelDiffPlugin ─→ remaining↓
  ├─ SingleLinePlugin → remaining↓
  └─ TeamPlugin ──────→ remaining↓
                        │
Phase 1.5（沉淀）       ▼
  remaining > 0 且 POOL_REWARD 启用？
  │ YES
  seller ──transfer──→ entity_account
  UnallocatedPool[entity_id] += remaining
  OrderUnallocated[order_id] = (entity_id, shop_id, remaining)

Phase 2（池奖励分配）
  pool_balance = UnallocatedPool[entity_id]
  ┌─ PoolRewardPlugin::calculate(remaining=pool_balance, order_amount, buyer)
  │  → (outputs, new_remaining)
  └─ distributed = pool_balance - new_remaining
     UnallocatedPool -= distributed
     credit_commission(outputs)  ← 资金已在 entity_account 中
```

## 不启用 vs 启用对比

| 场景 | `remaining`（未分配佣金）去向 |
|------|------|
| **未启用 POOL_REWARD** | 留在卖家账户（从未被转出） |
| **已启用 POOL_REWARD** | 卖家 → Entity 账户 → `UnallocatedPool` → 高等级会员奖金 |

## 奖励计算原理

```
推荐链：买家 → A(level_1) → B(level_2) → C(level_0) → D(level_3)

沉淀池余额 10,000 NEX，订单金额 100,000 NEX
max_drain_rate = 500 (5%)
→ 本次可分配上限 cap = 10,000 × 5% = 500 NEX

配置：
  level_0 = 0 bps (不参与)
  level_1 = 50 bps (0.5%)
  level_2 = 100 bps (1.0%)
  level_3 = 200 bps (2.0%)

计算过程：
├── A: level_1, rate=50 → 100,000 × 0.5% = 500, actual=min(500, cap=500) = 500
│   cap=0, 提前退出
├── B: 跳过（cap 已耗尽）
├── C: 跳过
└── D: 跳过

结果：A 获得 500 NEX 奖金，池余额 → 9,500 NEX
```

核心规则：
- 沿推荐链向上遍历，最多 `max_depth` 层
- 每个祖先按其 `custom_level_id` 查找对应 `rate`
- 奖金 = `order_amount × rate / 10000`，受 `cap` 和 `remaining` 双重约束
- `cap = pool_balance × max_drain_rate / 10000`，防止单笔大单耗尽池子
- 未配置等级或 rate=0 的会员跳过（不获得奖金）

## 数据结构

### PoolRewardConfig — 沉淀池奖励配置（per-entity）

```rust
pub struct PoolRewardConfig<MaxLevels: Get<u32>> {
    /// 各等级奖励比例（基点），按 (level_id, rate_bps) 索引
    /// 仅配置了的等级可获得奖励，未配置的等级不参与
    pub level_rates: BoundedVec<(u8, u16), MaxLevels>,
    /// 沿推荐链向上最大遍历深度（1-30）
    pub max_depth: u8,
    /// 单次订单最大可消耗池余额比例（基点，如 500 = 5%）
    pub max_drain_rate: u16,
}
```

### 配置示例

```
Entity 配置沉淀池奖励：
├── level_rates:
│   ├── level_0 = 0 bps    (普通会员，不参与)
│   ├── level_1 = 50 bps   (0.5%)
│   ├── level_2 = 100 bps  (1.0%)
│   └── level_3 = 200 bps  (2.0%)
├── max_depth: 10           (最多遍历 10 层)
└── max_drain_rate: 500     (每笔订单最多消耗池余额的 5%)
```

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;
    #[pallet::constant]
    type MaxPoolRewardLevels: Get<u32>;  // 最大等级配置数
}
```

## Storage

### 本插件 Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PoolRewardConfigs` | `Map<u64, PoolRewardConfig>` | 沉淀池奖励配置（entity_id → config） |

### Core 新增 Storage（由 pallet-commission-core 管理）

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `UnallocatedPool` | `Map<u64, Balance>` | 沉淀资金池余额（entity_id → balance） |
| `OrderUnallocated` | `Map<u64, (u64, u64, Balance)>` | 订单沉淀记录（order_id → (entity_id, shop_id, amount)），用于取消退款 |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_pool_reward_config` | Root | 设置沉淀池奖励配置 |

参数校验：
- `level_rates` 中每个 `rate` ≤ 10000 基点
- `max_depth` 范围 1-30
- `max_drain_rate` 范围 1-10000

## 订单取消处理

订单取消时 core 的 `cancel_commission` 处理两类资金：

| 资金类型 | 取消时归还目标 | 处理方式 |
|----------|---------------|----------|
| **OrderUnallocated**（本订单沉淀贡献） | 卖家 | entity_account → seller; `UnallocatedPool -= amount` |
| **PoolReward 记录**（已从池中发放的奖金） | 沉淀池（不退卖家） | 不转账，仅 `UnallocatedPool += amount`; 更新 stats |

```
cancel_commission(order_id):
  1. PoolReward 类型记录 → 金额回池（UnallocatedPool += amount）
  2. OrderUnallocated → entity_account 退还卖家（UnallocatedPool -= amount）
  3. 其他类型记录 → 正常退款（不变）
```

## 偿付安全

Entity 账户的偿付检查已更新，包含沉淀池余额：

```
required_reserve = pending_commission + shopping_balance + unallocated_pool
entity_balance >= withdrawal + required_reserve
```

确保提现不会挪用沉淀池资金。

## CommissionModes 位标志

```
POOL_REWARD = 0b10_0000_0000 (0x200)
```

通过 `set_commission_modes` 启用，与其他模式可自由组合。

## Trait 实现

- **`CommissionPlugin`** — 由 core Phase 2 调度，`remaining` 参数为池余额
- **`PoolRewardPlanWriter`** — 供 core 的 `init_commission_plan` 写入/清除配置

## Events

### 本插件

| 事件 | 说明 |
|------|------|
| `PoolRewardConfigUpdated` | 沉淀池奖励配置更新 |

### Core 新增事件

| 事件 | 说明 |
|------|------|
| `UnallocatedCommissionPooled` | 未分配佣金转入沉淀池（Phase 1.5） |
| `PoolRewardDistributed` | 沉淀池奖励发放完成（Phase 2） |
| `UnallocatedPoolRefunded` | 订单取消时沉淀池退还卖家 |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRate` | 返佣率超过 10000 基点 |
| `InvalidMaxDepth` | max_depth 不在 1-30 范围内 |
| `InvalidDrainRate` | max_drain_rate 不在 1-10000 范围内 |

## 风险与对策

| 风险 | 对策 |
|------|------|
| 池耗尽 | `max_drain_rate` 限制单次最大消耗比例（建议 ≤ 5%） |
| 冷启动无奖励 | 池初始为空，需若干订单积累后才有奖金发放 |
| Gas 成本 | `max_depth` 限制遍历深度（同其他插件），可控 |
| Owner 挪用 | 偿付检查计入池余额，withdraw 无法侵占池资金 |

## 依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```

## 测试覆盖

| 测试 | 覆盖场景 |
|------|----------|
| `set_config_works` | 正常设置配置 |
| `set_config_rejects_invalid_rate` | 拒绝超 10000 的费率 |
| `set_config_rejects_invalid_depth` | 拒绝 depth=0 |
| `set_config_rejects_invalid_drain_rate` | 拒绝 drain_rate=0 |
| `set_config_requires_root` | 非 Root 调用被拒 |
| `no_config_returns_empty` | 无配置时返回空 |
| `mode_not_enabled_returns_empty` | 模式未启用时返回空 |
| `basic_pool_reward_distribution` | 基础多等级分配 |
| `max_drain_rate_caps_distribution` | drain_rate 上限约束 |
| `pool_balance_caps_distribution` | 池余额不足时约束 |
| `max_depth_limits_traversal` | 深度限制截断 |
| `plan_writer_works` | PlanWriter 写入/清除 |
