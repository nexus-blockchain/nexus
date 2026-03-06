# pallet-commission-pool-reward

> 沉淀池奖励插件 — 周期性等额分配模型（Periodic Equal-Share Claim）

## 概述

`pallet-commission-pool-reward` 是返佣系统的**沉淀池奖励插件**。当 `POOL_REWARD` 模式启用后，每笔订单中未被其他插件（Referral / LevelDiff / SingleLine / Team）分配的佣金余额自动沉淀入 **Entity 级沉淀资金池**（由 `pallet-commission-core` Phase 1.5 管理）。

本插件采用**周期性等额领取**模型：按固定区块间隔（`round_duration`）划分轮次，每轮开始时快照池余额和各等级会员数量，按比率切分后平均分配给该等级会员。用户在轮次窗口内签名调用 `claim_pool_reward` 领取份额。

## 架构

```
pallet-commission-core (Phase 1.5)
  └── UnallocatedPool / UnallocatedTokenPool
         ↓ 自动沉淀
  ┌─────────────────────────────────────────────────────────────┐
  │  pallet-commission-pool-reward                              │
  │                                                             │
  │  PoolRewardConfig ──→ round_duration 区块为一轮             │
  │       │                                                     │
  │       ├── 首次 claim / start_new_round                      │
  │       │     → 快照池余额 + 各等级会员数                     │
  │       │     → 按 level_ratios 切分 → per_member_reward      │
  │       │                                                     │
  │       ├── claim_pool_reward (会员签名)                      │
  │       │     → NEX: deduct_pool → Currency::transfer         │
  │       │     → Token: best-effort (失败不影响 NEX)           │
  │       │     → ClaimRecord 写入 + 统计累加 + 回调            │
  │       │                                                     │
  │       └── 轮次过期 → 下次 claim 自动创新轮                  │
  │             → 旧轮归档到 RoundHistory (FIFO)                │
  │             → 未领取金额留在池中，下轮重新分配              │
  └─────────────────────────────────────────────────────────────┘
         ↓ 事件 / 回调
  Off-chain Indexer / Frontend / pallet-entity-member (OnMemberRemoved)
```

## 核心设计原则

| 原则 | 说明 |
|------|------|
| Entity Owner 不可提取 | 沉淀池资金完全由算法驱动分配，Owner 无法直接提取 |
| NEX + Token 双池 | 同时支持原生 NEX 和 Entity Token 两种资产，按相同比率分配 |
| Lazy 轮次创建 | 首个 claim 或 `start_new_round` 触发新轮快照，无需定时任务 |
| 未领取金额留存 | 本轮未被领取的金额自然留在池中，下一轮重新分配 |
| 三层暂停 | per-entity 暂停 + Root 强制暂停 + 全局紧急暂停 |
| KYC 合规 | `ParticipationGuard` 在 claim 前检查参与权 |
| 延时配置变更 | 配置变更可预约后延迟 `ConfigChangeDelay` 区块生效 |
| 等级回退 | 会员等级不在配置中时，自动回退到最近的已配置低等级 |
| round_id 单调递增 | 配置变更不重置轮次 ID，通过 `LastRoundId` 保证全局单调 |
| 会员移除自动清理 | 通过 `OnMemberRemoved` 回调，会员被移除时自动清理 per-user 存储 |

## 分配模型

### 轮次生命周期

```
   轮次 N                               轮次 N+1
├──────────── round_duration ──────────┤──────────────────────┤
│  首次 claim 或 start_new_round      │  轮次过期后首次      │
│  → 快照创建（旧轮归档到 History）   │  claim → 新轮快照    │
│  → 用户逐个 claim                    │                      │
│  → 未领取份额留在池中                │                      │
```

### 快照与分配计算

```
快照时刻：
  NEX 池余额 = 10,000
  配置 level_ratios: [(level_1, 5000bps), (level_2, 5000bps)]  ← 总和 = 10000 bps

分配计算：
  level_1 份额 = 10,000 × 50% = 5,000 NEX ÷ 5人 = 1,000 NEX/人
  level_2 份额 = 10,000 × 50% = 5,000 NEX ÷ 2人 = 2,500 NEX/人

per_member_reward = pool_balance × ratio / (10000 × member_count)
```

### 等级回退

当会员当前等级不在 `level_ratios` 中时，`resolve_effective_level` 自动回退到 `level_id ≤ 当前等级` 的最高已配置等级。无可回退等级则拒绝领取。

| 场景 | 当前等级 | 配置等级 | 有效等级 |
|------|----------|----------|----------|
| 精确匹配 | 2 | [1, 2, 3] | 2 |
| 升级超出配置 | 5 | [1, 2, 3] | 3（回退） |
| 低于所有配置 | 1 | [3, 5, 8] | 拒绝（`LevelNotConfigured`） |

### NEX + Token 双池

当 `token_pool_enabled = true` 时，快照同时记录 Token 池余额，按相同 `level_ratios` 分配。领取时 **NEX 为主、Token 为辅**（best-effort）：

- NEX: `deduct_pool` → `Currency::transfer`（deduct-first，失败整体回滚）
- Token: `token_transfer` → `deduct_token_pool`（transfer-first，失败仅跳过 Token 部分）

Token 转账成功但记账扣减失败时，尝试反向转账回滚。若回滚也失败，累加到 `TokenPoolDeficit`，发出 `TokenTransferRollbackFailed` 事件，由 Root 通过 `correct_token_pool_deficit` 修正。

## 权限模型

```
┌─────────────────────────────────────────────────────────┐
│  Root (Governance)                                      │
│  ├── force_set_pool_reward_config    绕过全部限制       │
│  ├── force_set_token_pool_enabled    绕过全部限制       │
│  ├── force_start_new_round           绕过暂停和锁定     │
│  ├── force_clear_pool_reward_config  完整清理全部存储   │
│  ├── force_pause_pool_reward         绕过 EntityLocked  │
│  ├── force_resume_pool_reward        绕过 EntityLocked  │
│  ├── set_global_pool_reward_paused   全局紧急暂停/恢复  │
│  └── correct_token_pool_deficit      修正 Token 差额    │
├─────────────────────────────────────────────────────────┤
│  Owner / Admin(COMMISSION_MANAGE)                       │
│  ├── set_pool_reward_config          需 entity_active   │
│  ├── start_new_round                 需 !paused         │
│  ├── set_token_pool_enabled          幂等保护           │
│  ├── clear_pool_reward_config        部分清理           │
│  ├── pause_pool_reward / resume_pool_reward              │
│  ├── schedule_pool_reward_config_change   延迟配置变更  │
│  ├── apply_pending_pool_reward_config     延迟到期触发  │
│  └── cancel_pending_pool_reward_config                   │
│  所有操作受 EntityLocked 保护                            │
├─────────────────────────────────────────────────────────┤
│  Signed (任何用户)                                      │
│  └── claim_pool_reward               需会员 + KYC       │
├─────────────────────────────────────────────────────────┤
│  PlanWriter (pallet-commission-core / 治理)              │
│  ├── set_pool_reward_config          绕过权限检查       │
│  ├── clear_config                    完整清理           │
│  └── set_token_pool_enabled          绕过权限检查       │
└─────────────────────────────────────────────────────────┘
```

## 数据结构（7 个）

```rust
pub struct PoolRewardConfig<MaxLevels, BlockNumber> {
    pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,  // (level_id, ratio_bps)，总和 = 10000
    pub round_duration: BlockNumber,                      // 轮次区块间隔
    pub token_pool_enabled: bool,                         // Token 池分配开关
}

pub struct RoundInfo<MaxLevels, Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub start_block: BlockNumber,
    pub pool_snapshot: Balance,
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
}

pub struct LevelSnapshot<Balance> {
    pub level_id: u8,
    pub member_count: u32,
    pub per_member_reward: Balance,
    pub claimed_count: u32,
}

pub struct ClaimRecord<Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub amount: Balance,
    pub level_id: u8,
    pub claimed_at: BlockNumber,
    pub token_amount: TokenBalance,
}

pub struct CompletedRoundSummary<MaxLevels, Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub start_block: BlockNumber,
    pub end_block: BlockNumber,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
    pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
}

pub struct DistributionStats<Balance, TokenBalance> {
    pub total_nex_distributed: Balance,
    pub total_token_distributed: TokenBalance,
    pub total_rounds_completed: u64,
    pub total_claims: u64,
}

pub struct PendingConfigChange<MaxLevels, BlockNumber> {
    pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
    pub round_duration: BlockNumber,
    pub apply_after: BlockNumber,
}
```

## Pallet Config

| 类型参数 | 约束 | 说明 |
|----------|------|------|
| `Currency` | `Currency<AccountId>` | NEX 原生资产 |
| `MemberProvider` | `MemberProvider<AccountId>` | 会员查询：等级、人数、状态 |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 状态、Owner、Admin、锁定 |
| `PoolBalanceProvider` | `PoolBalanceProvider<Balance>` | NEX 沉淀池余额与扣减 |
| `TokenBalance` | `AtLeast32BitUnsigned + ...` | Token 余额类型 |
| `TokenPoolBalanceProvider` | `TokenPoolBalanceProvider<TokenBalance>` | Token 沉淀池余额与扣减 |
| `TokenTransferProvider` | `TokenTransferProvider<AccountId, TokenBalance>` | Token 转账 |
| `ParticipationGuard` | `ParticipationGuard<AccountId>` | KYC/合规参与权检查 |
| `ClaimCallback` | `PoolRewardClaimCallback<...>` | 领取后回调（写入 core 统一佣金体系） |
| `WeightInfo` | `WeightInfo` | 权重 |

| 常量 | 约束 | 说明 |
|------|------|------|
| `MaxPoolRewardLevels` | `≥ 1` | 配置中最大等级数 |
| `MaxClaimHistory` | `≥ 1` | 每用户领取记录滚动窗口大小 |
| `MinRoundDuration` | `> 0` | 最小轮次区块间隔 |
| `MaxRoundHistory` | `≥ 1` | 轮次历史 FIFO 容量 |
| `ConfigChangeDelay` | `> 0` | 延时配置变更最小等待区块数 |

`integrity_test` 校验以上所有常量约束。

## Storage（11 项）

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PoolRewardConfigs` | `StorageMap<u64, PoolRewardConfig>` | per-entity 奖励配置 |
| `CurrentRound` | `StorageMap<u64, RoundInfo>` | 当前轮次快照 |
| `LastRoundId` | `StorageMap<u64, u64, ValueQuery>` | round_id 单调递增锚点 |
| `LastClaimedRound` | `StorageDoubleMap<u64, AccountId, u64, ValueQuery>` | 用户上次领取轮次（防双领） |
| `ClaimRecords` | `StorageDoubleMap<u64, AccountId, BoundedVec<ClaimRecord>>` | 用户领取历史（滚动窗口） |
| `PoolRewardPaused` | `StorageMap<u64, bool, ValueQuery>` | per-entity 暂停标志 |
| `GlobalPoolRewardPaused` | `StorageValue<bool, ValueQuery>` | 全局紧急暂停标志 |
| `RoundHistory` | `StorageMap<u64, BoundedVec<CompletedRoundSummary>>` | 已完成轮次归档（FIFO） |
| `DistributionStatistics` | `StorageMap<u64, DistributionStats, ValueQuery>` | 累计分配统计 |
| `PendingPoolRewardConfig` | `StorageMap<u64, PendingConfigChange>` | 待生效配置变更 |
| `TokenPoolDeficit` | `StorageMap<u64, TokenBalance, ValueQuery>` | Token 回滚失败累计差额 |

## Extrinsics（18 个）

| idx | 方法 | 权限 | 说明 |
|-----|------|------|------|
| 0 | `set_pool_reward_config` | Owner/Admin | 立即设置配置（自动清除 pending） |
| 1 | `claim_pool_reward` | Signed（会员） | 领取当前轮次 NEX + Token（支持等级回退） |
| 2 | `start_new_round` | Owner/Admin | 手动开启新轮次（检查暂停 + 轮次未过期则拒绝） |
| 3 | `set_token_pool_enabled` | Owner/Admin | 启用/禁用 Token 池（幂等保护） |
| 4 | `force_set_pool_reward_config` | Root | 强制设置配置（绕过权限和锁定） |
| 5 | `force_set_token_pool_enabled` | Root | 强制 Token 开关 |
| 6 | `force_start_new_round` | Root | 强制新轮次（绕过暂停，需 entity_active） |
| 7 | `clear_pool_reward_config` | Owner/Admin | 部分清除（不清用户级记录） |
| 8 | `force_clear_pool_reward_config` | Root | 完整清除全部存储（含 `clear_prefix` O(n)） |
| 9 | `pause_pool_reward` | Owner/Admin | 暂停该 Entity 池奖励 |
| 10 | `resume_pool_reward` | Owner/Admin | 恢复该 Entity 池奖励 |
| 11 | `set_global_pool_reward_paused` | Root | 全局暂停/恢复所有 Entity |
| 12 | `force_pause_pool_reward` | Root | 强制暂停 per-entity（绕过 EntityLocked） |
| 13 | `force_resume_pool_reward` | Root | 强制恢复 per-entity（绕过 EntityLocked） |
| 14 | `schedule_pool_reward_config_change` | Owner/Admin | 计划配置变更（延迟 ConfigChangeDelay 生效） |
| 15 | `apply_pending_pool_reward_config` | Owner/Admin | 应用待生效配置（延迟到期 + entity_active） |
| 16 | `cancel_pending_pool_reward_config` | Owner/Admin | 取消待生效配置 |
| 17 | `correct_token_pool_deficit` | Root | 修正 Token 池账本差额（清零 TokenPoolDeficit） |

## 关键流程

### claim_pool_reward

```
 1. ensure_signed
 2. is_entity_active
 3. !GlobalPoolRewardPaused
 4. !PoolRewardPaused[entity_id]
 5. is_member + !is_banned + is_member_active
 6. ParticipationGuard::can_participate
 7. resolve_effective_level（精确匹配 → 等级回退）
 8. ensure_current_round（过期则 create_new_round + 归档旧轮）
 9. last_claimed_round < current_round_id（防双领）
10. claimed_count < member_count（配额检查，回退用户跳过）
11. NEX: deduct_pool → Currency::transfer
12. Token: best-effort（transfer → deduct → 失败则回滚 → 回滚失败则累计 deficit）
13. claimed_count++
14. ClaimRecords 写入（滚动窗口 MaxClaimHistory）
15. DistributionStatistics 累加
16. ClaimCallback::on_pool_reward_claimed
17. 发出 PoolRewardClaimed 事件
```

### 延时配置变更

```
  Owner/Admin: schedule_pool_reward_config_change
    → 校验 level_ratios + round_duration
    → 存入 PendingPoolRewardConfig { apply_after: now + ConfigChangeDelay }
    → 发出 PoolRewardConfigScheduled

       ├──→ 等待 ConfigChangeDelay 区块后
       │    Owner/Admin: apply_pending_pool_reward_config
       │      → 校验 entity_active + !entity_locked + now >= apply_after
       │      → do_set_pool_reward_config（写入 + 失效轮次 + 清除 pending）
       │      → 发出 PendingPoolRewardConfigApplied
       │
       └──→ 或 Owner/Admin: cancel_pending_pool_reward_config
            → 移除 PendingPoolRewardConfig
            → 发出 PendingPoolRewardConfigCancelled

  注：直接调用 set_pool_reward_config / clear_pool_reward_config 会自动清除 pending。
```

### 清除行为对比

| 操作 | 清除范围 |
|------|----------|
| `clear_pool_reward_config` (Owner/Admin) | Config + PoolRewardPaused + PendingConfig + CurrentRound（失效） + RoundHistory + DistributionStatistics（LastRoundId **保留**以维持 round_id 单调递增） |
| `force_clear_pool_reward_config` (Root) | **全部**：Config + CurrentRound + LastRoundId + LastClaimedRound(`clear_prefix`) + ClaimRecords(`clear_prefix`) + PoolRewardPaused + PendingConfig + RoundHistory + DistributionStatistics + TokenPoolDeficit |
| `PoolRewardPlanWriter::clear_config` | 同 Root force_clear |
| `OnMemberRemoved::on_member_removed` | 仅单用户：LastClaimedRound + ClaimRecords（该 entity + 该用户） |

### 暂停层级

```
GlobalPoolRewardPaused (Root)        ← 优先级最高，阻塞所有 Entity
  └── PoolRewardPaused[entity_id]    ← Owner/Admin 或 Root force
        └── claim_pool_reward 被阻塞
        └── start_new_round 被阻塞（force_start_new_round 绕过）
```

## Events（17 个）

| 事件 | 字段 | 说明 |
|------|------|------|
| `PoolRewardConfigUpdated` | `entity_id` | 配置已更新 |
| `NewRoundStarted` | `entity_id, round_id, pool_snapshot, token_pool_snapshot, level_snapshots, token_level_snapshots` | 新轮次创建（含各等级人数和 per_member_reward） |
| `PoolRewardClaimed` | `entity_id, account, amount, token_amount, round_id, level_id` | 用户领取 |
| `TokenPoolEnabledUpdated` | `entity_id, enabled` | Token 池开关实际变更（幂等调用不发出） |
| `RoundForced` | `entity_id, round_id` | 手动/强制新轮次 |
| `TokenTransferRollbackFailed` | `entity_id, account, amount` | Token 转账回滚失败（已累计到 deficit，需 Root 干预） |
| `TokenClaimTransferFailed` | `entity_id, account, amount` | Token 初始转账失败（Token 部分跳过，NEX 不受影响） |
| `PoolRewardConfigCleared` | `entity_id` | 配置已清除 |
| `PoolRewardPaused` | `entity_id` | Entity 池奖励已暂停 |
| `PoolRewardResumed` | `entity_id` | Entity 池奖励已恢复 |
| `GlobalPoolRewardPaused` | — | 全局已暂停 |
| `GlobalPoolRewardResumed` | — | 全局已恢复 |
| `RoundArchived` | `entity_id, round_id` | 旧轮次已归档到 RoundHistory |
| `PoolRewardConfigScheduled` | `entity_id, apply_after` | 配置变更已计划 |
| `PendingPoolRewardConfigApplied` | `entity_id` | 待生效配置已应用 |
| `PendingPoolRewardConfigCancelled` | `entity_id` | 待生效配置已取消 |
| `TokenPoolDeficitCorrected` | `entity_id, amount` | Token 池差额已被 Root 修正 |

## Errors（27 个）

| 错误 | 触发条件 |
|------|----------|
| `InvalidRatio` | ratio 不在 (0, 10000] |
| `RatioSumMismatch` | 比率总和 ≠ 10000 |
| `DuplicateLevelId` | level_ratios 中重复 level_id |
| `InvalidRoundDuration` | round_duration = 0 |
| `RoundDurationTooShort` | round_duration < MinRoundDuration |
| `ConfigNotFound` | Entity 无沉淀池配置 |
| `EntityNotActive` | Entity 不存在或未激活 |
| `NotAuthorized` | 非 Owner 且非 Admin(COMMISSION_MANAGE) |
| `EntityLocked` | Entity 已锁定（Root force_* 绕过） |
| `NotMember` | 非会员 / 已封禁(banned) / 已冻结(frozen) |
| `ParticipationRequirementNotMet` | ParticipationGuard 拒绝（KYC 未通过） |
| `LevelNotConfigured` | 用户等级及所有低等级均不在 level_ratios 中 |
| `LevelNotInSnapshot` | 有效等级未在当前轮次快照中 |
| `AlreadyClaimed` | 本轮已领取（last_claimed_round >= round_id） |
| `LevelQuotaExhausted` | 该等级已领满（claimed_count >= member_count） |
| `NothingToClaim` | per_member_reward = 0 |
| `InsufficientPool` | NEX 池实时余额不足（快照后可能被外部消耗） |
| `RoundIdOverflow` | round_id 达 u64::MAX |
| `PoolRewardIsPaused` | Entity 池奖励已暂停 |
| `PoolRewardNotPaused` | Entity 池奖励未暂停（resume 时检查） |
| `GlobalPaused` | 全局已暂停 |
| `GlobalNotPaused` | 全局未暂停（resume 时检查） |
| `RoundNotExpired` | 当前轮次尚未过期（start_new_round 时拒绝） |
| `PendingConfigExists` | 已存在待生效配置变更（一次只能存一个） |
| `NoPendingConfig` | 无待生效配置变更 |
| `ConfigChangeDelayNotMet` | 当前区块 < apply_after |
| `NoDeficit` | Token 池无差额可修正 |

## 内部方法

| 方法 | 可见性 | 说明 |
|------|--------|------|
| `ensure_owner_or_admin` | private | entity_active + Owner/Admin(COMMISSION_MANAGE) 检查 |
| `validate_level_ratios` | pub(crate) | 校验等级比率：无重复、每项 (0,10000]、总和 = 10000 |
| `resolve_effective_level` | private | 精确匹配 → 回退到 ≤ user_level 的最高已配置等级 |
| `do_set_pool_reward_config` | pub(crate) | 共享配置设置（校验 + 写入 + 失效轮次 + 保留 token_pool_enabled + 清除 pending） |
| `do_set_token_pool_enabled` | pub(crate) | 共享 Token 开关（幂等保护 + 变更时失效轮次） |
| `do_clear_pool_reward_config` | pub(crate) | Owner 级部分清理 |
| `do_full_clear_pool_reward` | pub(crate) | Root 级完整清理（含 clear_prefix 用户记录 + TokenPoolDeficit） |
| `build_level_snapshots<B>` | private | 泛型快照构建（NEX/Token 通用），checked_mul + 合并除法减少精度损失 |
| `ensure_current_round` | private | 轮次有效则返回，过期则 create_new_round |
| `create_new_round` | private | 归档旧轮 → 快照余额 + 会员数 → 写入 CurrentRound → 发出 NewRoundStarted |
| `invalidate_current_round` | pub(crate) | 保存 round_id 到 LastRoundId 后移除 CurrentRound |
| `simulate_claimable` | private | 基于当前池余额和会员数模拟新轮次快照计算 |

## 查询方法（只读）

| 方法 | 返回值 | 说明 |
|------|--------|------|
| `get_claimable(entity_id, who)` | `(Balance, TokenBalance)` | 预查询可领取金额（含暂停/权限/等级回退检查，轮次过期时 simulate） |
| `get_round_statistics(entity_id)` | `Option<Vec<(level_id, member_count, claimed_count, per_member_reward)>>` | 当前轮次各等级领取进度 |

## Trait 实现

### PoolRewardPlanWriter

供 `pallet-commission-core` / 治理使用，绕过权限检查，调用 `do_*` 共享方法：

```rust
impl PoolRewardPlanWriter for Pallet<T> {
    fn set_pool_reward_config(entity_id, level_ratios, round_duration) -> DispatchResult;
    fn clear_config(entity_id) -> DispatchResult;           // do_full_clear_pool_reward
    fn set_token_pool_enabled(entity_id, enabled) -> DispatchResult;
}
```

### PoolRewardQueryProvider

供 `pallet-commission-core` Dashboard API 使用的摘要查询：

```rust
impl PoolRewardQueryProvider<AccountId, Balance, TokenBalance> for Pallet<T> {
    fn claimable(entity_id, account) -> (Balance, TokenBalance);
    fn is_paused(entity_id) -> bool;
    fn current_round_id(entity_id) -> u64;
}
```

### OnMemberRemoved（P2-14）

会员被移除（`remove_member` / `leave_entity`）时，由 `pallet-entity-member` 的 `do_remove_member` 自动调用，清理该用户的 per-entity 存储：

```rust
impl OnMemberRemoved<AccountId> for Pallet<T> {
    fn on_member_removed(entity_id: u64, account: &AccountId) {
        LastClaimedRound::<T>::remove(entity_id, account);
        ClaimRecords::<T>::remove(entity_id, account);
    }
}
```

Runtime 配置：`type OnMemberRemoved = CommissionPoolReward;`（支持元组组合多个回调）。

## Runtime API（PoolRewardDetailApi）

补充 `CommissionDashboardApi` 的摘要视图，为沉淀池详情页提供完整数据。

### 接口定义

| 方法 | 参数 | 返回值 | 说明 |
|------|------|--------|------|
| `get_pool_reward_member_view` | `(entity_id, account)` | `Option<PoolRewardMemberView>` | 会员沉淀池详情：个人状态 + 轮次进度 + 领取历史 |
| `get_pool_reward_admin_view` | `(entity_id)` | `Option<PoolRewardAdminView>` | 管理者沉淀池总览：配置 + 统计 + 历史 + 待生效变更 |

### PoolRewardMemberView

```rust
pub struct PoolRewardMemberView<Balance, TokenBalance> {
    // 配置摘要
    pub round_duration: u64,
    pub token_pool_enabled: bool,
    pub level_ratios: Vec<(u8, u16)>,
    // 当前轮次
    pub current_round_id: u64,
    pub round_start_block: u64,
    pub round_end_block: u64,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,
    // 个人状态
    pub effective_level: u8,
    pub claimable_nex: Balance,
    pub claimable_token: TokenBalance,
    pub already_claimed: bool,
    pub round_expired: bool,
    pub last_claimed_round: u64,
    // 各等级领取进度
    pub level_progress: Vec<LevelProgressInfo<Balance>>,
    pub token_level_progress: Option<Vec<LevelProgressInfo<TokenBalance>>>,
    // 领取历史
    pub claim_history: Vec<ClaimRecordInfo<Balance, TokenBalance>>,
    // 状态
    pub is_paused: bool,
    pub has_pending_config: bool,
}
```

### PoolRewardAdminView

```rust
pub struct PoolRewardAdminView<Balance, TokenBalance> {
    // 完整配置
    pub level_ratios: Vec<(u8, u16)>,
    pub round_duration: u64,
    pub token_pool_enabled: bool,
    // 当前轮次
    pub current_round: Option<RoundDetailInfo<Balance, TokenBalance>>,
    // 累计统计
    pub total_nex_distributed: Balance,
    pub total_token_distributed: TokenBalance,
    pub total_rounds_completed: u64,
    pub total_claims: u64,
    // 轮次历史
    pub round_history: Vec<CompletedRoundInfo<Balance, TokenBalance>>,
    // 待生效配置
    pub pending_config: Option<PendingConfigInfo>,
    // 状态
    pub is_paused: bool,
    pub is_global_paused: bool,
    // 池实时余额
    pub current_pool_balance: Balance,
    pub current_token_pool_balance: TokenBalance,
    // Token 差额
    pub token_pool_deficit: TokenBalance,
}
```

### 辅助 DTO

```rust
pub struct LevelProgressInfo<Balance> {
    pub level_id: u8,
    pub ratio_bps: u16,
    pub member_count: u32,
    pub claimed_count: u32,
    pub per_member_reward: Balance,
}

pub struct ClaimRecordInfo<Balance, TokenBalance> {
    pub round_id: u64,
    pub amount: Balance,
    pub token_amount: TokenBalance,
    pub level_id: u8,
    pub claimed_at: u64,
}

pub struct RoundDetailInfo<Balance, TokenBalance> {
    pub round_id: u64,
    pub start_block: u64,
    pub end_block: u64,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub level_snapshots: Vec<LevelProgressInfo<Balance>>,
    pub token_level_snapshots: Option<Vec<LevelProgressInfo<TokenBalance>>>,
}

pub struct PendingConfigInfo {
    pub level_ratios: Vec<(u8, u16)>,
    pub round_duration: u64,
    pub apply_after: u64,
}
```

### 与 CommissionDashboardApi 的关系

| 页面 | API | 说明 |
|------|-----|------|
| 佣金总览页 | `CommissionDashboardApi::get_member_commission_dashboard` | 返回 `PoolRewardSnapshot`（4 字段摘要，不变） |
| 沉淀池详情页 | `PoolRewardDetailApi::get_pool_reward_member_view` | 完整会员视图（一次 RPC） |
| 沉淀池管理页 | `PoolRewardDetailApi::get_pool_reward_admin_view` | 管理者总览（一次 RPC） |

## Weight

基于 DB read/write 分析估算，后续可通过 benchmark 替换：

| Extrinsic | Reads | Writes | ref_time | proof_size |
|-----------|-------|--------|----------|------------|
| `set_pool_reward_config` | 7 | 4 | 55M | 7K |
| `claim_pool_reward` | 20 | 13 | 250M | 25K |
| `start_new_round` | 8 | 4 | 110M | 11K |
| `set_token_pool_enabled` | 4 | 3 | 45M | 5K |
| `clear_pool_reward_config` | 4 | 7 | 45M | 6K |
| `force_clear_pool_reward_config` | 2 | 1010 | 500M | 100K |
| `pause_pool_reward` | 4 | 1 | 30M | 4K |
| `resume_pool_reward` | 4 | 1 | 30M | 4K |
| `set_global_pool_reward_paused` | 1 | 1 | 15M | 2K |
| `force_pause_pool_reward` | 2 | 1 | 25M | 3K |
| `force_resume_pool_reward` | 2 | 1 | 25M | 3K |
| `schedule_pool_reward_config_change` | 4 | 1 | 45M | 5K |
| `apply_pending_pool_reward_config` | 6 | 4 | 55M | 7K |
| `cancel_pending_pool_reward_config` | 3 | 1 | 30M | 4K |
| `correct_token_pool_deficit` | 1 | 1 | 25M | 3K |

> `claim_pool_reward`: worst case 含轮次过期自动创建 + TokenPoolDeficit 写入。
> `force_clear_pool_reward_config`: 含 2 次 `clear_prefix(u32::MAX)` O(n) 操作，按最多 500 用户估权。

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-runtime = { workspace = true }
sp-api = { workspace = true }
sp-std = { workspace = true }
pallet-entity-common = { path = "../../common", default-features = false }
pallet-commission-common = { path = "../common", default-features = false }

[dev-dependencies]
pallet-balances = { workspace = true, features = ["std"] }
sp-io = { workspace = true, features = ["std"] }
```

## 测试覆盖（193 tests）

覆盖全部 18 个 extrinsic、3 个 PlanWriter 方法、3 个查询方法、OnMemberRemoved 回调及全部审计回归。

| 类别 | 数量 | 覆盖范围 |
|------|------|----------|
| 配置管理 | 7 | set_config 正常/异常路径（ratio 校验、duration 校验、权限、重复 level） |
| 轮次生命周期 | 4 | 创建/复用/过期/手动新轮 |
| 领取核心 | 9 | claim 金额计算、权限、配额耗尽、余额不足、空池 |
| 领取历史 | 3 | 记录写入/跨轮累积/滚动窗口淘汰 |
| PlanWriter | 4 | set_config/clear/token_enabled + 新存储清除验证 |
| Token 双池 | 6 | 开关/快照/双池 claim/best-effort/回滚 |
| 边界与集成 | 10 | 多实体隔离/跨轮领取/Token 回滚/空池/round_id 溢出 |
| Owner/Admin 权限 | 7 | Admin(COMMISSION_MANAGE)/Owner/Root 区分 |
| EntityLocked | 10 | 锁定保护 + Root force 绕过 |
| 配置清除 | 8 | Owner/Admin/Root 清除 + 异常路径 |
| 暂停/恢复 | 9 | pause/resume + claim 阻塞 + 状态检查 |
| MinRoundDuration | 3 | duration 校验（低于/等于/force） |
| get_claimable 预查询 | 5 | 正常/暂停/已领/非会员场景 |
| get_round_statistics | 2 | 无轮/有轮进度 |
| 全局暂停 | 6 | 全局 pause/resume + claim 阻塞 + 幂等 |
| 分配统计 | 3 | claim 累加/轮次完成计数 |
| 轮次历史 | 3 | 归档/FIFO 淘汰/事件 |
| 审计回归 | 34 | R1-R8 历轮安全修复验证 |
| 延时配置变更 | 12 | schedule/apply/cancel/参数校验/锁定/inactive/交互 |
| Root 强制暂停/恢复 | 7 | force_pause/resume + 锁定绕过 + claim 阻塞 |
| Root 完整清理 | 1 | force_clear 全量存储验证 |
| start_new_round 暂停 | 3 | per-entity/全局暂停 + Root 豁免 |
| 等级回退 | 3 | 回退成功/无低等级失败/get_claimable 回退 |
| Weight 合理性 | 2 | ref_time/proof_size 范围 + 相对大小 |
| 深度审计修复 | 8 | P1-1 回退配额保护 · P1-2 Token 回滚记录 · P2-6 清理泄漏 · P2-7 权限收紧 · P2-9 精度 · P2-10 最小轮龄 · P2-11 force_clear 报错 |
| Token deficit 修正 | 6 | correct_token_pool_deficit + deficit 累计 + admin_view 展示 |
| Runtime API 视图 | 8 | member_view / admin_view 完整数据 + 边界场景 |
| OnMemberRemoved 回调 | 4 | 存储清理 · 未知用户 no-op · 其他用户不受影响 · 多实体隔离 |

---

## 深度审计修复（Phase Audit）

| 编号 | 优先级 | 修复内容 |
|------|--------|----------|
| P1-1 | **P1** | 等级回退用户跳过配额检查（精确匹配用户仍受 `claimed_count < member_count` 保护），消除回退抢占正常配额 |
| P1-2 | **P1** | Token 转账回滚失败时仍记录 `claimed_count` + `token_reward`，保持分配记录与资产流一致；累计到 `TokenPoolDeficit` 供 Root 修正 |
| P1-3 | **P1** | `claim_pool_reward` 权重覆盖轮次过期自动创建路径（worst case 20R/13W, 250M ref_time） |
| P2-4 | P2 | `start_new_round` 写入数修正为 4（CurrentRound + RoundHistory + DistributionStats + LastRoundId） |
| P2-5 | P2 | `force_clear` 权重按 500 用户估权（2 × clear_prefix O(n)，500M ref_time, 1010 writes） |
| P2-6 | P2 | `do_clear_pool_reward_config`（Owner）同时清理 `RoundHistory` + `DistributionStatistics`（消除存储泄漏） |
| P2-7 | P2 | `apply_pending_pool_reward_config` 收紧为 Owner/Admin 权限（防止任意用户提前失效活跃轮次） |
| P2-8 | P2 | `build_level_snapshots` 检测 `checked_mul` 溢出（`defensive!` 日志） |
| P2-9 | P2 | 合并双重整除为单次除法（`pool * ratio / (10000 * count)`），减少精度损失 |
| P2-10 | P2 | `start_new_round` 新增 `RoundNotExpired` 错误码，当前轮次未过期时拒绝（防频繁空转） |
| P2-11 | P2 | `force_clear_pool_reward_config` 无配置时返回 `ConfigNotFound`（消除静默 no-op） |
| P2-12 | P2 | 新增 `TokenClaimTransferFailed` 事件，区分 Token 初始转账失败和回滚失败 |
| P2-13 | P2 | 权重计算纳入 `BoundedVec::remove(0)` O(n) shift 成本 |
| P2-14 | **已实现** | 会员移除时自动清理 `LastClaimedRound` + `ClaimRecords`（通过 `OnMemberRemoved` 回调，由 `pallet-entity-member` 的 `do_remove_member` 触发） |
