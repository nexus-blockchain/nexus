# pallet-commission-pool-reward

> 沉淀池奖励插件 — 周期性等额分配模型（Periodic Equal-Share Claim）

## 概述

`pallet-commission-pool-reward` 是返佣系统的**沉淀池奖励插件**。当 `POOL_REWARD` 模式启用后，每笔订单中未被其他插件（Referral / LevelDiff / SingleLine / Team）分配的佣金余额自动沉淀入 **Entity 级沉淀资金池**（由 `pallet-commission-core` Phase 1.5 管理）。

本插件采用**周期性等额领取**模型：按固定区块间隔（`round_duration`）划分轮次，每轮开始时快照池余额和各等级会员数量，按比率切分后平均分配给该等级会员。用户在轮次窗口内主动调用 `claim_pool_reward` 领取份额。

支持 **NEX + Entity Token 双池**同步分配。

**核心约束：Entity Owner 不可直接提取沉淀池资金，资金完全由算法驱动分配。**

## 设计动机

```
现有问题：
  订单佣金预算 (max_commission) 经 4 个插件分配后，剩余部分 (remaining) 留在卖家账户
  → 这部分资金没有被有效利用

沉淀池方案：
  remaining → 沉淀资金池 → 按等级比率等额分配
  → 形成 "消费 → 沉淀 → 奖励高等级 → 激励升级 → 更多消费" 的正向循环
```

## 分配模型

### 轮次生命周期

```
   轮次 N                               轮次 N+1
├──────────── round_duration ──────────┤──────────────────────┤
│                                      │                      │
│  首次 claim 或 force_new_round       │  轮次过期后首次      │
│  → 快照创建                          │  claim → 新轮快照    │
│  → 用户逐个 claim                    │                      │
│  → 未领取份额留在池中                │                      │
```

### 快照 + 分配计算

```
快照时刻：
  NEX 池余额 = 10,000
  配置 level_ratios: [(level_1, 5000bps), (level_2, 5000bps)]  ← 总和必须 = 10000

分配计算：
  level_1 份额 = 10,000 × 50% = 5,000 NEX
    该等级会员数 = 5 人
    → 每人可领 = 5,000 / 5 = 1,000 NEX

  level_2 份额 = 10,000 × 50% = 5,000 NEX
    该等级会员数 = 2 人
    → 每人可领 = 5,000 / 2 = 2,500 NEX

未领取的金额自然留在池中，下一轮快照时重新纳入。
```

### NEX + Token 双池

当 `token_pool_enabled = true` 时，快照同时记录 Token 沉淀池余额，按相同 `level_ratios` 分配 Token 奖励。领取时 **NEX 为主、Token 为辅**（best-effort）：Token 转账失败不影响 NEX 领取。

快照构建由通用 `build_level_snapshots<B>` 内部方法完成，NEX 与 Token 共用同一份等级会员计数缓存，避免重复存储读取。

## 沉淀入池流程（Core 管理）

### NEX 沉淀池入金

```
订单触发 process_commission:

Phase 1（卖家资金池 — 现有 4 插件）
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
```

### Token 沉淀池入金（三路来源）

Token 沉淀池 `UnallocatedTokenPool` 有 **3 条独立入金路径**，全部由 `pallet-commission-core` 管理：

```
                     Token 订单触发 process_token_commission
                     ┌──────────────────────────────────────┐
                     │                                      │
                     ▼                                      ▼
              ┌─────────────┐                     ┌──────────────────┐
              │  池 A        │                     │  池 B             │
              │  Token       │                     │  Entity Token    │
              │  平台费      │                     │  返佣预算        │
              │  (platform   │                     │  order_amount    │
              │   _fee)      │                     │  × max_rate      │
              └──────┬───────┘                     └────────┬─────────┘
                     │                                      │
          ┌──────────┴──────────┐              ┌────────────┴────────────┐
          │ 有招商推荐人？      │              │  4 个 Token 插件分配     │
          │                     │              │  Referral → LevelDiff   │
          ├─ YES: 推荐人分成    │              │  → SingleLine → Team    │
          │   (ReferrerShareBps)│              └────────────┬────────────┘
          │                     │                           │
          └─────────┬───────────┘                           ▼
                    │                              remaining > 0 且
                    ▼                              POOL_REWARD 启用？
           池 A 留存部分                                    │ YES
           (fee - 推荐人分成)                               ▼
                    │                          ┌─────────────────────────┐
                    │        路径 ②            │       路径 ①            │
                    └────────────┐              │  Token 4插件剩余沉淀   │
                                 │              │                         │
                                 ▼              ▼                         │
                    ┌────────────────────────────────┐                    │
                    │   UnallocatedTokenPool[entity] │ ◄─────────────────┘
                    │   (Token 沉淀池)               │
                    └───────────────┬────────────────┘
                                    │         ▲
                                    │         │ 路径 ③
                                    │  ┌──────┴────────────────────────┐
                                    │  │  sweep_token_free_balance     │
                                    │  │  外部直接转入 entity_account  │
                                    │  │  的 Token 自动归集            │
                                    │  │  (actual - accounted > 0)     │
                                    │  └───────────────────────────────┘
                                    ▼
                          pool-reward 轮次快照
                          → 会员 claim 领取
```

**三路来源详解：**

| 路径 | 来源 | 触发时机 | 代码位置 |
|------|------|----------|----------|
| **① 4插件剩余沉淀** | `process_token_commission` 池 B：Token 返佣预算经 4 个插件分配后的 `remaining` | 每笔 Token 订单 | `core::process_token_commission` 末尾 |
| **② 平台费留存** | `process_token_commission` 池 A：`token_platform_fee` 扣除招商推荐人分成后的剩余 | 每笔 Token 订单 | `core::process_token_commission` 池 A 段 |
| **③ 外部转入归集** | 第三方直接向 `entity_account` 转入的 Token（非订单/返佣渠道） | `withdraw_entity_token_funds` 或 `process_token_commission` 时自动 sweep | `core::sweep_token_free_balance` |

> **路径 ③ 原理：** Core 维护 `EntityTokenAccountedBalance` 记录已知渠道的 Token 余额。当 `actual_balance - accounted > 0` 时，差额视为外部转入，自动归入沉淀池。

## 数据结构

### PoolRewardConfig — 沉淀池奖励配置（per-entity）

```rust
pub struct PoolRewardConfig<MaxLevels: Get<u32>, BlockNumber> {
    /// 各等级分配比率（基点），(level_id, ratio_bps)，sum 必须 = 10000
    pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
    /// 轮次持续时间（区块数）
    pub round_duration: BlockNumber,
    /// 是否启用 Entity Token 池分配（默认 false）
    pub token_pool_enabled: bool,
}
```

### LevelSnapshot — 等级快照

```rust
pub struct LevelSnapshot<Balance> {
    pub level_id: u8,
    pub member_count: u32,       // 快照时该等级会员数量
    pub per_member_reward: Balance, // 每人可领取数量
    pub claimed_count: u32,      // 已领取人数
}
```

### RoundInfo — 轮次快照数据（per-entity）

```rust
pub struct RoundInfo<MaxLevels, Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,           // 轮次 ID（单调递增，上限 u64::MAX）
    pub start_block: BlockNumber,
    pub pool_snapshot: Balance,  // 快照时 NEX 池余额
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
    pub token_pool_snapshot: Option<TokenBalance>,   // None = Token 池未启用
    pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
}
```

### ClaimRecord — 领取记录

```rust
pub struct ClaimRecord<Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub amount: Balance,         // NEX 领取数量
    pub level_id: u8,
    pub claimed_at: BlockNumber,
    pub token_amount: TokenBalance, // Token 领取数量（0 = 无 Token 奖励）
}
```

### 配置示例

```
Entity 沉淀池奖励配置：
├── level_ratios:
│   ├── level_1 = 3000 bps  (30% 分配给 level_1 全体成员)
│   └── level_2 = 7000 bps  (70% 分配给 level_2 全体成员)
│   （level_0 未配置 → 不参与分配）
├── round_duration: 43200    (约 3 天 @ 6s/block)
└── token_pool_enabled: true (同步分配 Token)
```

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;
    type MemberProvider: MemberProvider<Self::AccountId>;
    type EntityProvider: EntityProvider<Self::AccountId>;
    type PoolBalanceProvider: PoolBalanceProvider<BalanceOf<Self>>;

    #[pallet::constant]
    type MaxPoolRewardLevels: Get<u32>;  // 最大等级配置数
    #[pallet::constant]
    type MaxClaimHistory: Get<u32>;      // 每用户最大领取历史数

    // Token 多资产扩展
    type TokenBalance: FullCodec + MaxEncodedLen + TypeInfo + Copy + Default
        + Debug + AtLeast32BitUnsigned + From<u32> + Into<u128>;
    type TokenPoolBalanceProvider: TokenPoolBalanceProvider<TokenBalanceOf<Self>>;
    type TokenTransferProvider: TokenTransferProvider<Self::AccountId, TokenBalanceOf<Self>>;
}
```

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PoolRewardConfigs` | `StorageMap<u64, PoolRewardConfig>` | 沉淀池奖励配置（entity_id → config） |
| `CurrentRound` | `StorageMap<u64, RoundInfo>` | 当前轮次快照（entity_id → round） |
| `LastClaimedRound` | `StorageDoubleMap<u64, AccountId, u64>` | 用户上次领取轮次 ID（防双领） |
| `ClaimRecords` | `StorageDoubleMap<u64, AccountId, BoundedVec<ClaimRecord>>` | 用户领取历史（滚动窗口） |

## Extrinsics

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_pool_reward_config` | Root | 设置沉淀池奖励配置（保留现有 `token_pool_enabled`） |
| 1 | `claim_pool_reward` | Signed（会员） | 领取当前轮次奖励（NEX + Token） |
| 2 | `force_new_round` | Root | 强制开启新轮次 |
| 3 | `set_token_pool_enabled` | Root | 启用/禁用 Token 池分配 |

### set_pool_reward_config 校验规则

校验由共享 `validate_level_ratios` 内部方法完成，extrinsic 和 `PoolRewardPlanWriter` 共用同一逻辑：

- `round_duration > 0`
- `level_ratios` 无重复 `level_id`
- 每个 `ratio` ∈ (0, 10000]
- 所有 `ratio` 之和**必须等于 10000**
- 更新配置时自动保留现有 `token_pool_enabled` 值

### claim_pool_reward 流程

```
1. 资格检查: is_member + is_activated
2. 配置检查: 用户 custom_level_id 在 level_ratios 中
3. 轮次检查: 当前轮次有效？过期则自动创建新轮
4. 防双领: last_claimed_round < current_round_id
5. 配额检查: claimed_count < member_count
6. NEX 转账: entity_account → 用户, pool -= reward
7. Token 转账: best-effort (失败不影响 NEX; 扣池失败则回滚转账)
8. 记录更新: claimed_count++, 写入 ClaimRecords (滚动窗口)
```

## 内部方法

| 方法 | 说明 |
|------|------|
| `validate_level_ratios(&[(u8, u16)])` | 校验等级比率配置（重复、范围、总和），extrinsic + PlanWriter 共用 |
| `build_level_snapshots<B>(pool_balance, &[(u8, u16, u32)])` | 泛型快照构建，NEX/Token 通用，避免重复代码 |
| `ensure_current_round(entity_id, config, now)` | 若当前轮次有效则返回，否则调用 `create_new_round` |
| `create_new_round(entity_id, config, now)` | 缓存等级会员数 → 构建 NEX/Token 快照 → 写入存储 |

## Trait 实现

### PoolRewardPlanWriter

供 `pallet-commission-core` 的 `init_commission_plan` / 治理写入配置：

```rust
trait PoolRewardPlanWriter {
    fn set_pool_reward_config(entity_id, level_ratios: Vec<(u8, u16)>, round_duration: u32) -> DispatchResult;
    fn clear_config(entity_id: u64) -> DispatchResult;
    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> DispatchResult;
}
```

- `set_pool_reward_config`: 与 extrinsic 共用 `validate_level_ratios` 校验，保留现有 `token_pool_enabled`
- `clear_config`: 清除 `PoolRewardConfigs` + `CurrentRound` + `LastClaimedRound` + `ClaimRecords` 全部 4 项存储

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `PoolRewardConfigUpdated` | entity_id | 配置更新 |
| `NewRoundStarted` | entity_id, round_id, pool_snapshot, token_pool_snapshot | 新轮次快照创建 |
| `PoolRewardClaimed` | entity_id, account, amount, token_amount, round_id, level_id | 用户领取奖励 |
| `TokenPoolEnabledUpdated` | entity_id, enabled | Token 池开关变更 |
| `RoundForced` | entity_id, round_id | 管理员强制开启新轮次 |

## Errors

| 错误 | 说明 |
|------|------|
| `InvalidRatio` | 单个比率不在 (0, 10000] 范围 |
| `RatioSumMismatch` | 所有等级比率之和不等于 10000 |
| `DuplicateLevelId` | 配置中存在重复的 level_id |
| `InvalidRoundDuration` | round_duration 为 0 |
| `NotMember` | 调用者不是该 Entity 的会员 |
| `MemberNotActivated` | 会员未激活 |
| `LevelNotConfigured` | 用户等级未在配置中 |
| `AlreadyClaimed` | 本轮已领取过 |
| `LevelQuotaExhausted` | 该等级本轮领取名额已满 |
| `NothingToClaim` | 可领取金额为 0 |
| `InsufficientPool` | NEX 沉淀池余额不足 |
| `ConfigNotFound` | Entity 未配置沉淀池奖励 |
| `LevelNotInSnapshot` | 等级未在当前轮次快照中 |
| `RoundIdOverflow` | round_id 已达 u64::MAX，无法创建新轮次 |

## Token 双池行为细节

| 场景 | NEX 领取 | Token 领取 |
|------|----------|------------|
| `token_pool_enabled = false` | 正常 | 不分配，快照无 Token |
| Token 余额充足 | 正常 | 正常 |
| Token 转账失败 | **正常（不受影响）** | 跳过，pool 不扣减 |
| Token pool 扣减失败 | 正常 | **回滚转账**，保持一致性 |

## 风险与对策

| 风险 | 对策 |
|------|------|
| 池余额为零 | 快照时 per_member_reward = 0，claim 返回 `NothingToClaim` |
| 冷启动无奖励 | 池初始为空，需若干订单积累后才有分配 |
| 等级人数暴增稀释 | 快照锁定当轮人数，新会员需等下一轮 |
| 未领取份额 | 留在池中，下一轮自然纳入重新分配 |
| Owner 挪用 | 偿付检查计入池余额 + POOL_REWARD cooldown 机制 |
| 双领攻击 | `LastClaimedRound` + `claimed_count` 双重防护 |
| round_id 溢出 | `create_new_round` 拒绝 `old_round_id == u64::MAX`，返回 `RoundIdOverflow` |

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-runtime = { workspace = true }
pallet-entity-common = { path = "../../common" }
pallet-commission-common = { path = "../common" }
```

## 测试覆盖（52 tests）

### 配置测试（7）

| 测试 | 覆盖场景 |
|------|----------|
| `set_config_works` | 正常设置配置 |
| `set_config_rejects_ratio_sum_mismatch` | 比率总和 ≠ 10000 |
| `set_config_rejects_zero_ratio` | 单个比率为 0 |
| `set_config_rejects_duplicate_level` | 重复 level_id |
| `set_config_rejects_zero_duration` | round_duration = 0 |
| `set_config_requires_root` | 非 Root 调用被拒 |
| `set_config_rejects_ratio_over_10000` | ratio > 10000 拒绝 |

### 轮次测试（4）

| 测试 | 覆盖场景 |
|------|----------|
| `first_claim_creates_round` | 首次 claim 触发轮次创建 |
| `round_persists_within_duration` | 轮次窗口内复用同一轮 |
| `round_rolls_over_after_expiry` | 过期后自动创建新轮 |
| `force_new_round_works` | Root 强制新轮 |

### 领取测试（9）

| 测试 | 覆盖场景 |
|------|----------|
| `basic_claim_works` | 基础领取 + 余额变化 |
| `claim_correct_amount_per_level` | 多等级按比率分配 |
| `claim_rejects_non_member` | 非会员被拒 |
| `claim_rejects_unconfigured_level` | 未配置等级被拒 |
| `double_claim_rejected` | 同轮双领被拒 |
| `level_quota_exhausted` | 等级配额耗尽 |
| `claim_deducts_pool_balance` | 池余额扣减验证 |
| `zero_member_level_no_reward` | 空等级份额留池 |
| `config_not_found_error` | 无配置时报错 |

### 领取历史测试（3）

| 测试 | 覆盖场景 |
|------|----------|
| `claim_history_recorded` | 历史记录写入 |
| `claim_history_multi_rounds` | 跨轮次历史 |
| `claim_history_evicts_oldest` | 滚动窗口淘汰最旧记录（MaxClaimHistory=5） |

### PlanWriter 测试（3）

| 测试 | 覆盖场景 |
|------|----------|
| `plan_writer_set_config` | Trait 写入配置 |
| `plan_writer_clear_config` | Trait 清除配置 + 轮次 |
| `plan_writer_set_token_pool_enabled` | Trait 设置 Token 开关 |

### Token 双池测试（6）

| 测试 | 覆盖场景 |
|------|----------|
| `set_token_pool_enabled_works` | Token 池开关 |
| `set_token_pool_enabled_requires_config` | 无配置时拒绝 |
| `round_includes_token_snapshot_when_enabled` | Token 快照正确生成 |
| `round_no_token_snapshot_when_disabled` | 禁用时无 Token 快照 |
| `claim_dual_pool_nex_and_token` | NEX + Token 同步领取 |
| `claim_token_best_effort_nex_still_works` | Token 失败不影响 NEX |

### 审计回归测试 Round 1（8）

| 测试 | 覆盖场景 |
|------|----------|
| `h1_plan_writer_rejects_invalid_ratio_sum` | PlanWriter 拒绝 sum≠10000 |
| `h1_plan_writer_rejects_zero_ratio` | PlanWriter 拒绝 ratio=0 |
| `h1_plan_writer_rejects_duplicate_level` | PlanWriter 拒绝重复 level_id |
| `h1_plan_writer_rejects_zero_duration` | PlanWriter 拒绝 duration=0 |
| `h2_set_config_preserves_token_pool_enabled` | extrinsic 更新配置保留 token 开关 |
| `h2_plan_writer_preserves_token_pool_enabled` | PlanWriter 更新配置保留 token 开关 |
| `h3_clear_config_resets_last_claimed_round` | clear_config 清理全部 4 项存储 |
| `m1_round_id_overflow_rejected` | round_id=u64::MAX 时拒绝创建新轮 |

### 审计回归测试 Round 2（5）

| 测试 | 覆盖场景 |
|------|----------|
| `h2_config_update_invalidates_current_round` | 配置变更清除旧快照，新 claim 创建含新 level_id 的快照 |
| `h2_plan_writer_config_update_invalidates_round` | PlanWriter 路径同样清除旧快照 |
| `h2_config_update_mid_round_allows_reclaim` | 配置更新后 LastClaimedRound 被清除，用户可立即 claim 新轮 |
| `m2_claim_rejects_entity_not_active` | Banned/Closed Entity 的会员不能领取 |
| `m2_claim_works_when_entity_active` | 正常 Entity 领取不受影响 |

### 边界与集成测试（12）

| 测试 | 覆盖场景 |
|------|----------|
| `claim_rejects_inactive_member` | 未激活会员拒绝领取 |
| `force_new_round_requires_root` | 非 Root 拒绝强制新轮 |
| `force_new_round_rejects_no_config` | 无配置时拒绝强制新轮 |
| `set_token_pool_enabled_requires_root` | 非 Root 拒绝设置 Token 开关 |
| `claim_zero_pool_balance_nothing_to_claim` | 零池余额返回 NothingToClaim |
| `claim_insufficient_pool_after_snapshot` | 快照后池被消耗返回 InsufficientPool |
| `multi_entity_isolation` | 多实体互不影响 |
| `claim_after_round_rollover_allowed` | 跨轮次连续领取 |
| `token_deduct_fail_rolls_back_transfer` | Token 扣减失败回滚转账 |
| `snapshot_with_empty_pool_produces_zero_rewards` | 空池快照 per_member=0 |

---

## 审计与优化记录

### Round 1

**审计日期**: 2026-03-02

#### 安全修复

| ID | 严重度 | 描述 | 修复 |
|----|--------|------|------|
| H1 | High | `PoolRewardPlanWriter::set_pool_reward_config` 绕过所有校验 | 提取 `validate_level_ratios` 共享方法，extrinsic + PlanWriter 统一校验 |
| H2 | High | `set_pool_reward_config` 更新配置时硬编码 `token_pool_enabled: false`，静默禁用已启用的 Token 池 | 读取已有配置保留 `token_pool_enabled` |
| H3 | High | `clear_config` 仅清理 2 项存储，`LastClaimedRound` 残留导致用户无法领取新轮奖励 | 清理全部 4 项存储（含 `LastClaimedRound` + `ClaimRecords`） |
| M1 | Medium | `create_new_round` 当 `round_id = u64::MAX` 时 `saturating_add(1)` 不变，产生重复 ID | 添加 `ensure!(old_round_id < u64::MAX, RoundIdOverflow)` |

#### 冗余清理

| ID | 类型 | 描述 |
|----|------|------|
| R1 | 代码重复 | extrinsic 和 PlanWriter 校验逻辑重复 → 提取 `validate_level_ratios` 共享方法 |
| R2 | 代码重复 | NEX/Token 快照构建逻辑重复 → 提取泛型 `build_level_snapshots<B>` 方法 |
| R3 | 冗余存储读 | `token_pool_enabled=true` 时 `member_count_by_level` 每级调用 2 次 → 缓存 `level_counts` Vec |
| R4 | 死绑定 | `_user_ratio` 计算后从未使用 → 替换为 `ensure!(..any(..))` 存在性检查 |
| R5 | 死 Config | `DefaultRoundDuration` 声明但 pallet 从未读取 → 移除（pallet + runtime） |
| R6 | 死 Error | `InsufficientTokenPool` 声明但从未抛出（Token 用 best-effort） → 移除 |
| R7 | 多余属性 | 测试辅助函数上无用的 `#[allow(dead_code)]` → 移除 |
| R8 | 死依赖 | `sp-std` 在 Cargo.toml 中声明但代码未使用 → 移除 |

### Round 2

**审计日期**: 2026-03-03

#### 安全修复

| ID | 严重度 | 描述 | 修复 |
|----|--------|------|------|
| H2-R2 | High | `set_pool_reward_config` 更新配置（level_ratios / round_duration）后不清除 `CurrentRound`，旧快照中的 level_id 集合/比率与新配置不一致。用户可能因 `LevelNotInSnapshot` 无法领取，或按旧比率领取错误金额 | extrinsic 和 PlanWriter 两条路径均清除 `CurrentRound` + `LastClaimedRound`，强制下次 claim 创建新快照 |
| M2-R2 | Medium | `claim_pool_reward` 不检查 Entity 是否存在/激活，Banned/Closed Entity 的会员仍可继续领取沉淀池奖励 | 添加 `ensure!(EntityProvider::is_entity_active(entity_id), EntityNotActive)` 前置检查 |

#### 其他修复

| ID | 类型 | 描述 |
|----|------|------|
| L1-R2 | Low | `Cargo.toml` `try-runtime` feature 缺少 `sp-runtime/try-runtime` |
| L1 | Low | 4 个 extrinsic 硬编码 Weight → 新建 `weights.rs`，定义 `WeightInfo` trait + `SubstrateWeight` 估算实现（基于 DB read/write 分析），Config 新增 `type WeightInfo` |
| L2-R2 | Low | `PoolRewardDefaultRoundDuration` parameter_types 在 runtime 声明但 pallet 从未使用 → 从 runtime 删除死常量 |
| M3-R2 | Medium | NEX 转账在 `deduct_pool` 之前执行 → 调整为「先扣记账（deduct_pool）、后转实物（Currency::transfer）」。Token 路径保持 transfer-first 顺序（best-effort 无法事务回滚） |

#### 未修复（记录）

| ID | 严重度 | 描述 |
|----|--------|------|
| M1-R2 | Medium | `build_level_snapshots` 整除截断导致尘埃累积：`pool * ratio / 10000 / count` 的截断余额永久留池。高等级人数少时损失比例可观（设计权衡：尘埃自动滚入下轮，无资金丢失） |

### Round 3

**审计日期**: 2026-03-03

#### 安全修复

| ID | 严重度 | 描述 | 修复 |
|----|--------|------|------|
| M1-R3 | Medium | `set_token_pool_enabled` (extrinsic + PlanWriter) 不使当前轮次失效。mid-round 启用→本轮无 token 快照用户无法领 token；mid-round 禁用→本轮仍可领 token | extrinsic 和 PlanWriter 切换后调用 `invalidate_current_round`，立即生效 |
| M2-R3 | Medium | `set_pool_reward_config` 使用 `clear_prefix(u32::MAX)` 清除 `LastClaimedRound`，写入量 O(n) 随用户数增长，weight 仅声明 2 writes 严重低估。根因：`CurrentRound::remove` 后 round_id 重置为 1 | 新增 `LastRoundId` 存储保持 round_id 单调递增；`invalidate_current_round` helper 保存 round_id 后移除轮次；消除 `clear_prefix` |

#### 其他修复

| ID | 类型 | 描述 |
|----|------|------|
| L1-R3 | Low | PlanWriter 三个方法（`set_pool_reward_config`, `clear_config`, `set_token_pool_enabled`）不 emit 事件 → off-chain indexer 无法感知 governance 配置变更。修复：每个方法末尾 `deposit_event` |
| L2-R3 | Low | `Cargo.toml` `runtime-benchmarks` feature 缺 `sp-runtime/runtime-benchmarks` → 已补充 |

#### 新增存储

| 名称 | 类型 | 说明 |
|------|------|------|
| `LastRoundId<T>` | `StorageMap<u64, u64, ValueQuery>` | 配置变更后保留上一轮次 ID，保持 round_id 单调递增 |

#### 新增测试 (6)

| 测试名 | 覆盖 |
|--------|------|
| `m1_r3_token_enable_invalidates_round_and_adds_token_snapshot` | M1-R3 启用 token 池立即生效 |
| `m1_r3_token_disable_invalidates_round_removes_token_snapshot` | M1-R3 禁用 token 池立即生效 |
| `m2_r3_config_update_round_id_monotonic` | M2-R3 配置更新后 round_id 递增 + LastClaimedRound 保留 |
| `m2_r3_multiple_config_updates_round_id_keeps_increasing` | M2-R3 多次配置更新 round_id 始终递增 |
| `l1_r3_plan_writer_emits_config_event` | L1-R3 PlanWriter emit PoolRewardConfigUpdated |
| `l1_r3_plan_writer_emits_token_event` | L1-R3 PlanWriter emit TokenPoolEnabledUpdated |

#### 修改已有测试 (2)

| 测试名 | 变更 |
|--------|------|
| `h2_config_update_invalidates_current_round` | round_id 断言 1→2（单调递增） |
| `h2_config_update_mid_round_allows_reclaim` | LastClaimedRound 不再清零，round_id 2 |

**累计测试**: 64 (was 58) ✅ · `cargo check -p nexus-runtime` ✅

### Round 4

**审计日期**: 2026-03-03

#### 修复

| ID | 严重度 | 描述 | 修复 |
|----|--------|------|------|
| M1-R4 | Medium | `weights.rs` DB read/write 计数在 R3 修复后未同步更新。`set_pool_reward_config` 实际 reads(2)+writes(3) 但声明 reads(1)+writes(2)；`set_token_pool_enabled` 实际 reads(2)+writes(3) 但声明 reads(1)+writes(1)；`force_new_round` 缺少 `member_count_by_level` 读取；注释引用已删除的 `clear_prefix` | 更新全部 4 个 extrinsic 的 DB 计数、proof_size、注释 |
| L1-R4 | Low | `set_token_pool_enabled` (extrinsic + PlanWriter) 幂等调用（如 enabled=true 时再次设 true）仍触发 `invalidate_current_round`，浪费有效快照 | 添加 `changed` 标志，仅在值实际变更时才失效轮次 |
| L2-R4 | Low | `clear_config` (PlanWriter) 使用 `clear_prefix(u32::MAX)` 清除 `LastClaimedRound` + `ClaimRecords`，写入量 O(n)。调用方需在自身 weight 中计入此开销 | 添加文档注释说明 weight 责任归属 |

#### 新增测试 (4)

| 测试名 | 覆盖 |
|--------|------|
| `l1_r4_idempotent_token_toggle_preserves_round` | L1-R4 幂等 extrinsic 调用不失效轮次 |
| `l1_r4_plan_writer_idempotent_token_toggle_preserves_round` | L1-R4 幂等 PlanWriter 调用不失效轮次 |
| `l1_r4_actual_change_still_invalidates_round` | L1-R4 实际变更仍正确失效轮次 |
| `m1_r4_weight_values_are_reasonable` | M1-R4 Weight 值非零且在预期范围内 |

**累计测试**: 68 (was 64) ✅ · `cargo check -p nexus-runtime` ✅
