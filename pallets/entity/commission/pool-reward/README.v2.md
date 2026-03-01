# pallet-commission-pool-reward（v2）

> 沉淀池奖励插件 — 周期性等额分配模型（Periodic Equal-Share Claim）

---

## 一、概述

`pallet-commission-pool-reward` 是返佣系统的 **Entity 级沉淀资金池** 插件。当启用 `POOL_REWARD` 模式后，每笔订单中未被其他插件（Referral / LevelDiff / SingleLine / Team）分配的佣金余额，自动转入 Entity 级沉淀资金池。

**v2 核心变更：** 分配方式由 v1 的「订单触发 → 推荐链遍历」改为「**每 3 天一轮 → 按会员等级等额分配 → 用户自行签名领取**」。

**核心约束：Entity Owner 不可提取沉淀池资金，资金完全由算法驱动分配。**

### 功能需求摘要

| # | 需求 | 说明 |
|---|------|------|
| 1 | 每 3 天周期 | 有资格的用户按会员等级自己签名领取 |
| 2 | 等级比率分配 | 按设定级别的分配比率（所有级别比率之和 = 100%），每个级别的分配总量 = 池余额 × 该级别比率；再除以同级别会员数量 = 每人可领取数量 |
| 3 | 未领取滚入 | 3 天内未领取的部分，计入下一轮重新分配 |

---

## 二、v1 → v2 对比

| 维度 | v1（订单触发） | v2（周期领取） |
|------|----------------|----------------|
| **触发方式** | 每笔订单自动触发 Phase 2 | 每 3 天一轮，用户主动 `claim` |
| **分配对象** | 买家推荐链上的祖先（按深度遍历） | 全 Entity 内所有符合条件的会员（按等级） |
| **分配算法** | `order_amount × rate`，受 `cap` / `remaining` 约束 | `pool × level_ratio / level_member_count` |
| **未分配处理** | 池余额留存，下次订单继续分配 | 未领取金额自动滚入下一轮 |
| **Gas 承担** | 卖家/买家（订单交易内） | 领取者自行承担（独立 extrinsic） |
| **CommissionPlugin** | 实现 `calculate()`，由 core Phase 2 调用 | **不再作为 Phase 2 插件**；改为独立领取流程 |

---

## 三、可行性分析

### 3.1 技术可行性

| 关注点 | 评估 | 说明 |
|--------|------|------|
| **3 天周期** | ✅ 可行 | 使用区块高度计算。假设 6s/block，3 天 ≈ 43,200 blocks。配置项 `round_duration` 可调 |
| **用户主动 Claim** | ✅ 可行 | 标准 Substrate extrinsic，用户签名调用 `claim_pool_reward` |
| **等级比率求和 = 100%** | ✅ 可行 | 配置时校验 `sum(ratios) == 10000` (基点) |
| **同级别会员计数** | ⚠️ 需新增 | 当前 `pallet-member` 无 `LevelMemberCount` 存储。需在 member 模块新增 per-level 计数器，并在 `MemberProvider` trait 中暴露 `member_count_by_level(entity_id, level_id) -> u32` |
| **防双领** | ✅ 可行 | `LastClaimedRound` 存储记录每用户最后领取的轮次 ID |
| **未领取滚入** | ✅ 天然实现 | 未被领取的金额留在池中，下一轮快照自然包含 |
| **轮次快照** | ✅ 可行 | 懒触发：首个 claim 触发新轮次快照，O(L) 复杂度（L = 等级数，通常 ≤ 10） |

### 3.2 依赖变更

| 模块 | 变更内容 | 影响范围 |
|------|----------|----------|
| `pallet-member` | 新增 `LevelMemberCount` 存储（DoubleMap: entity_id × level_id → u32），在等级变更路径维护计数 | member 模块内部 + 存储迁移 |
| `pallet-commission-common` | `MemberProvider` trait 新增 `fn member_count_by_level(entity_id: u64, level_id: u8) -> u32` | 所有 MemberProvider 实现需补充 |
| `pallet-commission-core` | Phase 2 调度逻辑移除对 `PoolRewardPlugin::calculate` 的调用；保留 Phase 1.5 沉淀逻辑不变 | core 的 `process_commission` |
| `pallet-commission-pool-reward` | 整体重构：移除 `CommissionPlugin` 实现，新增轮次/快照/Claim 机制 | 本模块 |

### 3.3 链上约束

| 约束 | 处理方式 |
|------|----------|
| 存储成本 | 轮次数据 `RoundInfo` 仅保留当前轮（O(1) per entity）；`LastClaimedRound` 为 DoubleMap（O(N) 会员数） |
| 计算复杂度 | 快照创建 O(L)；Claim O(1) 查表 |
| 轮次间隔强制性 | 区块高度硬约束，无法提前开启新轮 |

---

## 四、合理性分析

### 4.1 经济模型

```
正向循环：
  消费 → 佣金剩余沉淀入池 → 周期性分配给高等级会员
  → 激励用户升级等级 → 更多消费 → 更多沉淀 → 池子增长

等级权重分配：
  高等级比率更高 → 每个高等级会员获得更多
  低等级比率较低 → 但人数众多，仍有参与感
  → 既奖励头部，又普惠基层
```

### 4.2 激励分析

| 行为 | 激励效果 |
|------|----------|
| **升级等级** | 高等级分配比率更高，且同级别人数更少 → 人均奖励更高 → 强烈升级动力 |
| **及时领取** | 3 天窗口限制 → 用户需保持活跃，定期签名领取 → 提升用户粘性 |
| **不领取** | 个人份额滚入下一轮 → 不惩罚休眠用户，但活跃用户可从更大池中受益 |
| **持续消费** | 消费产生佣金沉淀 → 池子增长 → 所有等级受益 → 正向激励 |

### 4.3 公平性

| 维度 | 说明 |
|------|------|
| **同级别等额** | 同一等级的会员获得完全相同的金额（`level_allocation / member_count`） |
| **比率可调** | Entity Owner 可根据业务需要调整各等级比率，灵活适配不同激励策略 |
| **透明可审计** | 轮次快照上链，任何人可验证分配计算 |

### 4.4 潜在风险

| 风险 | 严重程度 | 说明 |
|------|----------|------|
| 等级套利 | 中 | 用户可能在轮次快照前升级，快照后降级。缓解：等级升降有消费门槛，操纵成本高 |
| 大户垄断 | 低 | 同级别等额分配，大户无法多领。若最高级别仅少数人，每人分配较多 → 可通过调低高等级比率缓解 |
| 冷启动期 | 低 | 池初始为空，需若干订单积累后才有奖金 → 可由 Entity Owner 手动注入初始资金 |
| Gas 费负担 | 低 | 用户需自行付 Gas 领取。若奖金小于 Gas 成本，用户可能弃领 → 自然滚入下轮 |

### 4.5 结论

> **方案可行且合理。** 周期性等额分配模型简单透明、激励清晰，与现有佣金系统兼容性好（Phase 1.5 沉淀机制不变）。
> 主要开发成本在于 member 模块的 `LevelMemberCount` 新增和 pool-reward 模块的重构。

---

## 五、设计方案

### 5.1 核心概念

| 概念 | 说明 |
|------|------|
| **Round（轮次）** | 以 `round_duration` 区块为周期的分配时间窗口。每个 Entity 独立计轮 |
| **Snapshot（快照）** | 轮次开始时记录的 `pool_balance` 和各等级 `member_count`，用于计算每人可领取数量 |
| **Claim（领取）** | 用户在轮次窗口内签名调用 `claim_pool_reward`，领取属于自己等级的份额 |
| **Rollover（滚入）** | 轮次结束时未被领取的金额留在池中，下一轮快照自然包含 |

### 5.2 架构变更

```
v1 架构（订单触发）：
  process_commission → Phase 1 → Phase 1.5(沉淀) → Phase 2(PoolRewardPlugin)

v2 架构（周期领取）：
  process_commission → Phase 1 → Phase 1.5(沉淀) → [Phase 2 移除]
                                                         |
  独立流程: claim_pool_reward(entity_id)
    → 检查/创建轮次快照
    → 计算当前用户份额
    → 从 entity_account 转账给用户
    → 扣减 UnallocatedPool
```

### 5.3 轮次生命周期

```
+-------------------------------------------------------+
|                    Round N                              |
|                                                        |
|  start_block <-- 首个 claim 触发快照                    |
|  |                                                     |
|  |  pool_snapshot = UnallocatedPool[entity_id]         |
|  |  level_snapshots = [                                |
|  |    (level_1, count=10, per_member=40),              |
|  |    (level_2, count=5,  per_member=120),             |
|  |    (level_3, count=2,  per_member=300),             |
|  |  ]                                                  |
|  |                                                     |
|  +-- User A(level_1) claims -> gets 40                 |
|  +-- User B(level_2) claims -> gets 120                |
|  +-- User C(level_1) claims -> gets 40                 |
|  +-- ...                                               |
|  |   (3天内未领取的份额留在池中)                         |
|  |                                                     |
|  end_block = start_block + round_duration              |
+-------------------------------------------------------+
                        |
                        v
+-------------------------------------------------------+
|                    Round N+1                            |
|  首个 claim 触发新快照                                  |
|  pool_snapshot = 剩余池余额(含未领取 + 新沉淀)          |
|  重新统计各等级人数                                     |
|  ...                                                   |
+-------------------------------------------------------+
```

### 5.4 奖励计算公式

```
输入：
  pool_balance      = UnallocatedPool[entity_id]（快照时刻）
  level_ratios      = [(level_1, 2000), (level_2, 3000), (level_3, 5000)]
                      // 基点, 2000+3000+5000 = 10000 (100%)
  member_counts     = {level_1: 100, level_2: 50, level_3: 10}

计算：
  level_1_allocation = pool_balance * 2000 / 10000 = pool_balance * 20%
  level_1_per_member = level_1_allocation / 100

  level_2_allocation = pool_balance * 3000 / 10000 = pool_balance * 30%
  level_2_per_member = level_2_allocation / 50

  level_3_allocation = pool_balance * 5000 / 10000 = pool_balance * 50%
  level_3_per_member = level_3_allocation / 10

示例（pool_balance = 10,000 NEX）：
  level_1: 10,000 * 20% = 2,000 / 100人 = 20 NEX/人
  level_2: 10,000 * 30% = 3,000 / 50人  = 60 NEX/人
  level_3: 10,000 * 50% = 5,000 / 10人  = 500 NEX/人

  -> 等级越高，人数越少，每人分得越多
```

### 5.5 资格判定

用户需同时满足以下条件才可领取：

| 条件 | 检查方式 |
|------|----------|
| 是 Entity 的注册会员 | `MemberProvider::is_member(entity_id, &account)` |
| 已激活 | `MemberProvider::is_activated(entity_id, &account)` |
| 等级已配置分配比率 | `level_ratios` 中存在对应 `level_id` 且 `ratio > 0` |
| 本轮未领取过 | `LastClaimedRound[entity_id][account] < current_round_id` |

### 5.6 轮次内等级变更处理

**策略：快照计数 + 当前等级领取 + per-level 领取上限**

| 步骤 | 说明 |
|------|------|
| 快照时 | 记录每个等级的 `member_count` 和 `per_member_reward` |
| 领取时 | 使用用户 **当前等级** 查找对应 `per_member_reward` |
| 上限控制 | 每个等级记录 `claimed_count`，不得超过快照时的 `member_count` |
| 超限处理 | 若某等级 `claimed_count >= snapshot_member_count`，拒绝该等级的后续领取（`LevelQuotaExhausted`） |

```
合理性说明：
  快照 member_count 决定了该等级最多有多少份额可被领取。
  即使有人在轮次内升级到该等级，总领取量不会超过快照分配，
  保证了池的偿付安全。
  被拒的用户可在下一轮以新等级正常领取。
```

---

## 六、数据结构

### 6.1 PoolRewardConfig — 沉淀池奖励配置（per-entity）

```rust
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[scale_info(skip_type_params(MaxLevels))]
pub struct PoolRewardConfig<MaxLevels: Get<u32>, BlockNumber> {
    /// 各等级分配比率（基点），sum 必须等于 10000
    /// (level_id, ratio_bps)
    pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
    /// 轮次持续时间（区块数，默认 43200 约等于 3 天 @6s/block）
    pub round_duration: BlockNumber,
}
```

### 6.2 RoundInfo — 轮次快照数据（per-entity）

```rust
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[scale_info(skip_type_params(MaxLevels))]
pub struct RoundInfo<MaxLevels: Get<u32>, Balance, BlockNumber> {
    /// 轮次 ID（单调递增）
    pub round_id: u64,
    /// 轮次开始区块
    pub start_block: BlockNumber,
    /// 快照时池余额
    pub pool_snapshot: Balance,
    /// 各等级快照
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct LevelSnapshot<Balance> {
    pub level_id: u8,
    /// 快照时该等级会员数量
    pub member_count: u32,
    /// 每人可领取数量
    pub per_member_reward: Balance,
    /// 已领取人数
    pub claimed_count: u32,
}
```

### 6.3 ClaimRecord — 领取记录（per-user per-round）

```rust
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct ClaimRecord<Balance, BlockNumber> {
    /// 领取的轮次 ID
    pub round_id: u64,
    /// 领取数量
    pub amount: Balance,
    /// 领取时的会员等级
    pub level_id: u8,
    /// 领取时区块高度
    pub claimed_at: BlockNumber,
}
```

### 6.4 配置示例

```
Entity 配置沉淀池奖励（v2）：
+-- level_ratios:
|   +-- level_1 = 2000 bps  (20%)
|   +-- level_2 = 3000 bps  (30%)
|   +-- level_3 = 5000 bps  (50%)
|   (sum = 10000 bps = 100%)
|
+-- round_duration: 43200 blocks (约等于 3 天 @6s/block)

注意：level_0 未配置 -> 普通会员不参与分配
```

---

## 七、Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;

    /// Entity 查询接口（获取 entity_account 用于转账）
    type EntityProvider: EntityProvider<Self::AccountId>;

    /// 池余额读写接口（访问 commission-core 的 UnallocatedPool）
    type PoolBalanceProvider: PoolBalanceProvider<BalanceOf<Self>>;

    /// 最大等级配置数
    #[pallet::constant]
    type MaxPoolRewardLevels: Get<u32>;

    /// 默认轮次持续区块数（可被 per-entity 配置覆盖）
    #[pallet::constant]
    type DefaultRoundDuration: Get<BlockNumberFor<Self>>;

    /// 每用户最大领取历史记录数
    #[pallet::constant]
    type MaxClaimHistory: Get<u32>;
}
```

---

## 八、Storage

### 8.1 本插件 Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PoolRewardConfigs` | `Map<u64, PoolRewardConfig>` | 沉淀池奖励配置（entity_id -> config） |
| `CurrentRound` | `Map<u64, RoundInfo>` | 当前轮次快照数据（entity_id -> round） |
| `LastClaimedRound` | `DoubleMap<u64, AccountId, u64>` | 用户最后领取的轮次 ID（entity_id x account -> round_id），用于防双领 |
| `ClaimRecords` | `DoubleMap<u64, AccountId, BoundedVec<ClaimRecord, MaxClaimHistory>>` | 用户每轮领取历史（entity_id x account -> Vec<ClaimRecord>），记录轮数和每轮领取数量 |

### 8.2 Core Storage（不变）

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `UnallocatedPool` | `Map<u64, Balance>` | 沉淀资金池余额（entity_id -> balance） |
| `OrderUnallocated` | `Map<u64, (u64, u64, Balance)>` | 订单沉淀记录 |

### 8.3 新增 Trait — PoolBalanceProvider

```rust
/// 沉淀池余额读写接口（由 commission-core 实现）
pub trait PoolBalanceProvider<Balance> {
    fn pool_balance(entity_id: u64) -> Balance;
    fn deduct_pool(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;
}
```

commission-core 实现此 trait，pool-reward 通过 Config 关联类型访问，避免直接跨 pallet 存储读写。

---

## 九、Extrinsics

### 9.1 set_pool_reward_config（管理接口）

```rust
#[pallet::call_index(0)]
pub fn set_pool_reward_config(
    origin: OriginFor<T>,
    entity_id: u64,
    level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
    round_duration: BlockNumberFor<T>,
) -> DispatchResult
```

| 项目 | 说明 |
|------|------|
| **权限** | Root / Governance |
| **校验** | `level_ratios` 中所有 `ratio` 之和 == 10000；每个 `ratio` 在 (0, 10000] 范围内；`round_duration` > 0；无重复 `level_id` |
| **效果** | 写入 `PoolRewardConfigs`；**不影响进行中的轮次**，下一轮生效 |

### 9.2 claim_pool_reward（用户领取）

```rust
#[pallet::call_index(1)]
pub fn claim_pool_reward(
    origin: OriginFor<T>,
    entity_id: u64,
) -> DispatchResult
```

| 项目 | 说明 |
|------|------|
| **权限** | Signed（任何注册会员） |
| **流程** | 1. `ensure_signed` 2. 资格检查 3. 检查/创建轮次 4. 查用户等级 5. 查快照中对应等级的 `per_member_reward` 6. 检查 `claimed_count < member_count` 7. 转账 8. 更新状态 |
| **资金流** | `entity_account -> caller`（实际代币转移） |
| **失败条件** | 非会员、未激活、等级无配置、本轮已领取、等级配额已满、池余额不足 |

### 9.3 force_new_round（可选管理接口）

```rust
#[pallet::call_index(2)]
pub fn force_new_round(
    origin: OriginFor<T>,
    entity_id: u64,
) -> DispatchResult
```

| 项目 | 说明 |
|------|------|
| **权限** | Root |
| **效果** | 强制结束当前轮次，创建新的快照。用于紧急重置或配置变更后立即生效 |

---

## 十、核心流程

### 10.1 Claim 完整流程（伪代码）

```rust
fn claim_pool_reward(origin, entity_id) {
    let who = ensure_signed(origin)?;

    // 1. 资格检查
    ensure!(MemberProvider::is_member(entity_id, &who));
    ensure!(MemberProvider::is_activated(entity_id, &who));

    let config = PoolRewardConfigs::get(entity_id)?;
    let user_level = MemberProvider::custom_level_id(entity_id, &who);

    // 检查用户等级是否在配置中且比率 > 0
    let _user_ratio = config.level_ratios.iter()
        .find(|(id, _)| *id == user_level)
        .map(|(_, r)| *r)
        .ok_or(Error::LevelNotConfigured)?;

    // 2. 轮次检查 / 创建
    let now = current_block();
    let mut round = Self::ensure_current_round(entity_id, &config, now)?;

    // 3. 防双领
    let last_round = LastClaimedRound::get(entity_id, &who);
    ensure!(last_round < round.round_id, Error::AlreadyClaimed);

    // 4. 查找等级快照
    let snapshot = round.level_snapshots.iter_mut()
        .find(|s| s.level_id == user_level)
        .ok_or(Error::LevelNotInSnapshot)?;

    // 5. 配额检查
    ensure!(snapshot.claimed_count < snapshot.member_count,
            Error::LevelQuotaExhausted);

    let reward = snapshot.per_member_reward;
    ensure!(!reward.is_zero(), Error::NothingToClaim);

    // 6. 池偿付检查
    let pool = PoolBalanceProvider::pool_balance(entity_id);
    ensure!(pool >= reward, Error::InsufficientPool);

    // 7. 转账: entity_account -> caller
    let entity_account = EntityProvider::entity_account(entity_id);
    Currency::transfer(&entity_account, &who, reward, KeepAlive)?;

    // 8. 状态更新
    PoolBalanceProvider::deduct_pool(entity_id, reward)?;
    snapshot.claimed_count += 1;
    CurrentRound::insert(entity_id, round);
    LastClaimedRound::insert(entity_id, &who, round.round_id);

    // 9. 写入领取历史（记录轮数 + 每轮领取数量）
    ClaimRecords::try_mutate(entity_id, &who, |history| {
        let record = ClaimRecord {
            round_id: round.round_id,
            amount: reward,
            level_id: user_level,
            claimed_at: now,
        };
        // 超出 MaxClaimHistory 时移除最早记录
        if history.is_full() {
            history.remove(0);
        }
        history.try_push(record)
    })?;

    Self::deposit_event(Event::PoolRewardClaimed {
        entity_id, account: who, amount: reward, round_id, level_id: user_level
    });
}
```

### 10.2 轮次快照创建（伪代码）

```rust
fn ensure_current_round(entity_id, config, now) -> Result<RoundInfo> {
    if let Some(round) = CurrentRound::get(entity_id) {
        if now < round.start_block + config.round_duration {
            return Ok(round);  // 当前轮次仍有效
        }
    }

    // 创建新轮次
    let pool_balance = PoolBalanceProvider::pool_balance(entity_id);
    let old_round_id = CurrentRound::get(entity_id)
        .map(|r| r.round_id).unwrap_or(0);

    let mut level_snapshots = BoundedVec::new();
    for (level_id, ratio) in config.level_ratios.iter() {
        let count = MemberProvider::member_count_by_level(entity_id, *level_id);
        let per_member = if count > 0 {
            pool_balance * (*ratio as Balance) / 10000 / (count as Balance)
        } else {
            0  // 该等级无会员，分配额留在池中
        };
        level_snapshots.try_push(LevelSnapshot {
            level_id: *level_id,
            member_count: count,
            per_member_reward: per_member,
            claimed_count: 0,
        })?;
    }

    let new_round = RoundInfo {
        round_id: old_round_id + 1,
        start_block: now,
        pool_snapshot: pool_balance,
        level_snapshots,
    };
    CurrentRound::insert(entity_id, &new_round);

    Self::deposit_event(Event::NewRoundStarted {
        entity_id, round_id: new_round.round_id, pool_snapshot: pool_balance
    });
    Ok(new_round)
}
```

### 10.3 Phase 1.5 沉淀（不变）

```
process_commission 中（commission-core 管理）：
  Phase 1 插件分配后 remaining > 0 且 POOL_REWARD 启用
  -> seller 转账到 entity_account
  -> UnallocatedPool[entity_id] += remaining
  -> 写入 OrderUnallocated[order_id]
```

### 10.4 订单取消处理

```
cancel_commission(order_id):
  OrderUnallocated -> entity_account 退还卖家, UnallocatedPool -= amount
  已领取的 PoolReward 不退（已转到用户钱包，无法回收）

注意：v2 中已领取的 Claim 不记入 OrderCommissionRecords，
因此订单取消不影响已领取的池奖励。取消只影响 OrderUnallocated（减少池中金额）。
```

---

## 十一、Events

### 本插件

| 事件 | 字段 | 说明 |
|------|------|------|
| `PoolRewardConfigUpdated` | `entity_id` | 配置更新 |
| `NewRoundStarted` | `entity_id, round_id, pool_snapshot` | 新轮次开始，快照已创建 |
| `PoolRewardClaimed` | `entity_id, account, amount, round_id, level_id` | 用户成功领取池奖励（含轮次和等级） |
| `RoundForced` | `entity_id, round_id` | 管理员强制开启新轮次 |

### Core 事件（不变）

| 事件 | 说明 |
|------|------|
| `UnallocatedCommissionPooled` | 未分配佣金转入沉淀池（Phase 1.5） |
| `UnallocatedPoolRefunded` | 订单取消时沉淀池退还卖家 |

---

## 十二、Errors

| 错误 | 说明 |
|------|------|
| `InvalidRatio` | 单个比率超出 (0, 10000] 范围 |
| `RatioSumMismatch` | 所有等级比率之和不等于 10000 |
| `DuplicateLevelId` | 配置中存在重复的 level_id |
| `InvalidRoundDuration` | round_duration 为 0 |
| `NotMember` | 调用者不是该 Entity 的会员 |
| `MemberNotActivated` | 会员未激活 |
| `LevelNotConfigured` | 用户等级未在配置中或比率为 0 |
| `AlreadyClaimed` | 本轮已领取过 |
| `LevelQuotaExhausted` | 该等级本轮领取名额已满（mid-round 等级变更导致） |
| `NothingToClaim` | 可领取金额为 0（等级无会员导致 per_member=0） |
| `InsufficientPool` | 沉淀池余额不足 |
| `ConfigNotFound` | Entity 未配置沉淀池奖励 |

---

## 十三、MemberProvider 扩展

### 13.1 新增 trait 方法

```rust
// pallet-commission-common/src/lib.rs
pub trait MemberProvider<AccountId> {
    // ... 现有方法 ...

    /// 查询指定等级的会员数量
    fn member_count_by_level(entity_id: u64, level_id: u8) -> u32;
}
```

### 13.2 member 模块实现

```rust
// pallet-member 新增存储
#[pallet::storage]
pub type LevelMemberCount<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, u64, Blake2_128Concat, u8, u32, ValueQuery,
>;

// 在以下路径维护计数器：
// 1. register_member / auto_register
//    -> LevelMemberCount[entity_id][0] += 1（新会员默认 level_0）
// 2. upgrade_member_level (manual / auto)
//    -> LevelMemberCount[entity_id][old_level] -= 1
//    -> LevelMemberCount[entity_id][new_level] += 1
// 3. remove_member（如有）
//    -> LevelMemberCount[entity_id][current_level] -= 1
// 4. level_expiry（等级过期降级）
//    -> LevelMemberCount[entity_id][expired_level] -= 1
//    -> LevelMemberCount[entity_id][calculated_level] += 1
```

### 13.3 空实现补充

```rust
impl<AccountId> MemberProvider<AccountId> for NullMemberProvider {
    fn member_count_by_level(_: u64, _: u8) -> u32 { 0 }
}
```

---

## 十四、PoolBalanceProvider Trait（新增）

```rust
// pallet-commission-common/src/lib.rs
pub trait PoolBalanceProvider<Balance> {
    /// 查询沉淀池余额
    fn pool_balance(entity_id: u64) -> Balance;
    /// 从沉淀池扣减指定金额
    fn deduct_pool(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;
}

/// 空实现
impl<Balance: Default> PoolBalanceProvider<Balance> for () {
    fn pool_balance(_: u64) -> Balance { Balance::default() }
    fn deduct_pool(_: u64, _: Balance) -> Result<(), DispatchError> { Ok(()) }
}
```

commission-core 实现：

```rust
impl<T: Config> PoolBalanceProvider<BalanceOf<T>> for Pallet<T> {
    fn pool_balance(entity_id: u64) -> BalanceOf<T> {
        UnallocatedPool::<T>::get(entity_id)
    }
    fn deduct_pool(entity_id: u64, amount: BalanceOf<T>) -> Result<(), DispatchError> {
        UnallocatedPool::<T>::try_mutate(entity_id, |pool| {
            *pool = pool.checked_sub(&amount)
                .ok_or(DispatchError::Other("InsufficientPool"))?;
            Ok(())
        })
    }
}
```

---

## 十五、偿付安全

Entity 账户偿付检查（commission-core `withdraw_commission`）已包含沉淀池余额：

```
required_reserve = pending_commission + shopping_balance + unallocated_pool
entity_balance >= withdrawal + required_reserve
```

**v2 无额外变更**：Claim 操作从 `UnallocatedPool` 扣减并从 `entity_account` 转出，两者同步减少，偿付等式不变。

---

## 十六、CommissionModes 位标志

```
POOL_REWARD = 0b10_0000_0000 (0x200)
```

通过 `set_commission_modes` 启用，与其他模式可自由组合。

**v2 中 POOL_REWARD 的含义：**
- Phase 1.5 沉淀：`remaining > 0 && modes.contains(POOL_REWARD)` -> 转入沉淀池 (保留)
- Phase 2 分配：**移除**（不再由订单触发）
- Claim 分配：`PoolRewardConfigs` 存在 && 用户有资格 -> 允许领取 (新增)

---

## 十七、Trait 实现变更

### 移除

- **`CommissionPlugin`** — v2 不再作为订单处理插件

### 保留

- **`PoolRewardPlanWriter`** — 供 core 的 `init_commission_plan` 写入/清除配置（签名需更新以适配新配置结构）

### 更新后的 PlanWriter

```rust
pub trait PoolRewardPlanWriter {
    fn set_pool_reward_config(
        entity_id: u64,
        level_ratios: Vec<(u8, u16)>,    // (level_id, ratio_bps), sum=10000
        round_duration: u32,              // 区块数
    ) -> Result<(), DispatchError>;

    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;
}
```

---

## 十八、风险与对策

| 风险 | 严重程度 | 对策 |
|------|----------|------|
| 池耗尽 | 低 | 每轮最多分配 100% 池余额；若全员领取则池清零。可通过调比率或增等级缓冲 |
| 等级套利 | 中 | per-level `claimed_count` 上限 = 快照 `member_count`，限制跨等级套利；等级升级需真实消费门槛 |
| 冷启动期 | 低 | 池初始为空，需若干订单积累后才有奖金。Entity Owner 可手动注入启动资金 |
| Gas 费 > 奖励 | 低 | 小额奖励用户可选择不领取，自动滚入下轮积累 |
| 快照时 0 会员等级 | 无 | `member_count == 0` 时 `per_member_reward = 0`，该等级分配额留在池中滚入下轮 |
| Owner 挪用 | 无 | 偿付检查计入 `UnallocatedPool`，`withdraw` 无法侵占池资金 |
| 并发 Claim | 无 | 链上交易串行执行，无竞态条件 |
| 轮次间隔配置错误 | 低 | `round_duration` 最小值校验（建议 >= 100 blocks） |

---

## 十九、依赖

```toml
[dependencies]
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
# commission-core 通过 PoolBalanceProvider trait 提供池余额访问
```

---

## 二十、测试覆盖

### 20.1 配置测试

| 测试 | 覆盖场景 |
|------|----------|
| `set_config_works` | 正常设置配置，比率之和 = 10000 |
| `set_config_rejects_ratio_sum_mismatch` | 比率之和 != 10000 被拒 |
| `set_config_rejects_zero_ratio` | 单个比率为 0 被拒 |
| `set_config_rejects_duplicate_level` | 重复 level_id 被拒 |
| `set_config_rejects_zero_duration` | round_duration = 0 被拒 |
| `set_config_requires_root` | 非 Root 调用被拒 |

### 20.2 轮次测试

| 测试 | 覆盖场景 |
|------|----------|
| `first_claim_creates_round` | 首次 claim 触发轮次创建 + 快照 |
| `round_persists_within_duration` | 轮次有效期内多次 claim 使用同一快照 |
| `round_rolls_over_after_expiry` | 过期后首个 claim 触发新轮次 |
| `unclaimed_rolls_to_next_round` | 未领取金额包含在下一轮快照中 |
| `new_deposits_included_in_next_round` | 轮次内新沉淀金额在下一轮快照中体现 |
| `force_new_round_works` | Root 可强制创建新轮次 |

### 20.3 Claim 测试

| 测试 | 覆盖场景 |
|------|----------|
| `basic_claim_works` | 基础领取流程：资格 -> 快照 -> 转账 -> 状态更新 |
| `claim_correct_amount_per_level` | 不同等级用户领取各自正确的 per_member_reward |
| `claim_rejects_non_member` | 非会员被拒 |
| `claim_rejects_inactive_member` | 未激活会员被拒 |
| `claim_rejects_unconfigured_level` | 等级不在配置中被拒 |
| `double_claim_rejected` | 同一轮次重复领取被拒 |
| `level_quota_exhausted` | 等级领取名额满后被拒（模拟 mid-round 等级变更） |
| `claim_deducts_pool_balance` | 领取后 UnallocatedPool 正确扣减 |
| `claim_transfers_to_caller` | 资金正确从 entity_account 转到 caller |

### 20.4 边界测试

| 测试 | 覆盖场景 |
|------|----------|
| `zero_member_level_no_reward` | 某等级 0 会员时 per_member = 0，分配额留在池中 |
| `pool_empty_no_claim` | 池余额为 0 时，无法创建有效快照（所有 per_member = 0） |
| `all_members_claim_drains_pool` | 全员领取后池余额接近 0（整除余数除外） |
| `rounding_dust_stays_in_pool` | 整除产生的余数留在池中，不丢失 |
| `config_change_takes_effect_next_round` | 配置变更不影响当前轮次 |

### 20.5 领取历史测试

| 测试 | 覆盖场景 |
|------|----------|
| `claim_history_recorded` | 领取后 ClaimRecords 正确写入 round_id + amount + level_id |
| `claim_history_multi_rounds` | 跨多轮领取后历史按轮次顺序记录 |
| `claim_history_evicts_oldest` | 超出 MaxClaimHistory 时最早记录被移除 |
| `claim_history_queryable` | 可按 (entity_id, account) 查询完整领取历史 |

### 20.6 PlanWriter 测试

| 测试 | 覆盖场景 |
|------|----------|
| `plan_writer_set_config` | PlanWriter 正确写入配置 |
| `plan_writer_clear_config` | PlanWriter 正确清除配置 |

---

## 二十一、开发排期建议

| 阶段 | 任务 | 预估工作量 |
|------|------|-----------|
| **P0** | `pallet-member` 新增 `LevelMemberCount` 存储 + 维护逻辑 | 1-2 天 |
| **P0** | `MemberProvider` trait 新增 `member_count_by_level` + 所有实现 | 0.5 天 |
| **P1** | `pallet-commission-common` 新增 `PoolBalanceProvider` trait | 0.5 天 |
| **P1** | `pallet-commission-core` 实现 `PoolBalanceProvider` + Phase 2 调度移除 | 1 天 |
| **P2** | `pallet-commission-pool-reward` v2 重构（Config / Storage / Extrinsics） | 2-3 天 |
| **P2** | `PoolRewardPlanWriter` 签名更新 + core 适配 | 0.5 天 |
| **P3** | 单元测试（全量覆盖） | 2 天 |
| **P3** | 集成测试（与 commission-core / member 联调） | 1 天 |
| **合计** | | **8-10 天** |
