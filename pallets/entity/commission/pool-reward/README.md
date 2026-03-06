# pallet-commission-pool-reward

> 沉淀池奖励插件 — 周期性等额分配模型（Periodic Equal-Share Claim）

## 概述

`pallet-commission-pool-reward` 是返佣系统的**沉淀池奖励插件**。当 `POOL_REWARD` 模式启用后，每笔订单中未被其他插件（Referral / LevelDiff / SingleLine / Team）分配的佣金余额自动沉淀入 **Entity 级沉淀资金池**（由 `pallet-commission-core` Phase 1.5 管理）。

本插件采用**周期性等额领取**模型：按固定区块间隔（`round_duration`）划分轮次，每轮开始时快照池余额和各等级会员数量，按比率切分后平均分配给该等级会员。用户在轮次窗口内签名调用 `claim_pool_reward` 领取份额。

**核心设计原则：**

- **Entity Owner 不可提取** — 沉淀池资金完全由算法驱动分配
- **NEX + Token 双池** — 同时支持原生 NEX 和 Entity Token 两种资产分配
- **Lazy 轮次创建** — 首个 claim 或 force_new_round 触发新轮快照，无需定时任务
- **未领取金额留存** — 本轮未被领取的金额自然留在池中，下一轮继续分配
- **暂停机制** — 支持 per-entity 暂停和全局紧急暂停
- **KYC 合规** — 通过 `ParticipationGuard` 检查参与权

## 分配模型

### 轮次生命周期

```
   轮次 N                               轮次 N+1
├──────────── round_duration ──────────┤──────────────────────┤
│  首次 claim 或 force_new_round       │  轮次过期后首次      │
│  → 快照创建（F10: 旧轮归档）        │  claim → 新轮快照    │
│  → 用户逐个 claim                    │                      │
│  → 未领取份额留在池中                │                      │
```

### 快照 + 分配计算

```
快照时刻：
  NEX 池余额 = 10,000
  配置 level_ratios: [(level_1, 5000bps), (level_2, 5000bps)]  ← 总和必须 = 10000

分配计算：
  level_1 份额 = 10,000 × 50% = 5,000 NEX ÷ 5 人 = 1,000 NEX/人
  level_2 份额 = 10,000 × 50% = 5,000 NEX ÷ 2 人 = 2,500 NEX/人

未领取的金额留在池中，下一轮重新纳入。
```

### NEX + Token 双池

当 `token_pool_enabled = true` 时，快照同时记录 Token 沉淀池余额，按相同 `level_ratios` 分配。领取时 **NEX 为主、Token 为辅**（best-effort）：Token 转账失败不影响 NEX 领取。

快照构建由泛型 `build_level_snapshots<B>` 完成，NEX 与 Token 共用等级会员计数缓存，避免重复存储读取。

| 场景 | NEX | Token |
|------|-----|-------|
| `token_pool_enabled = false` | 正常 | 不分配 |
| Token 余额充足 | 正常 | 正常 |
| Token 转账失败 | **正常** | 跳过 |
| Token pool 扣减失败 | 正常 | **回滚转账** |

## 数据结构

### PoolRewardConfig

```rust
pub struct PoolRewardConfig<MaxLevels: Get<u32>, BlockNumber> {
    pub level_ratios: BoundedVec<(u8, u16), MaxLevels>, // (level_id, ratio_bps), sum = 10000
    pub round_duration: BlockNumber,                     // 轮次持续区块数
    pub token_pool_enabled: bool,                        // 是否启用 Token 池
}
```

### LevelSnapshot

```rust
pub struct LevelSnapshot<Balance> {
    pub level_id: u8,
    pub member_count: u32,          // 快照时该等级会员数
    pub per_member_reward: Balance, // 每人可领取数量
    pub claimed_count: u32,         // 已领取人数
}
```

### RoundInfo

```rust
pub struct RoundInfo<MaxLevels, Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,              // 单调递增，上限 u64::MAX
    pub start_block: BlockNumber,
    pub pool_snapshot: Balance,     // 快照时 NEX 池余额
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
}
```

### ClaimRecord

```rust
pub struct ClaimRecord<Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub amount: Balance,            // NEX 领取数量
    pub level_id: u8,
    pub claimed_at: BlockNumber,
    pub token_amount: TokenBalance, // Token 领取数量（0 = 无）
}
```

### CompletedRoundSummary

```rust
pub struct CompletedRoundSummary<MaxLevels, Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub start_block: BlockNumber,
    pub end_block: BlockNumber,
    pub pool_snapshot: Balance,
    pub token_pool_snapshot: Option<TokenBalance>,
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
    pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
}
```

### DistributionStats

```rust
pub struct DistributionStats<Balance, TokenBalance> {
    pub total_nex_distributed: Balance,
    pub total_token_distributed: TokenBalance,
    pub total_rounds_completed: u64,
    pub total_claims: u64,
}
```

### Traits

```rust
// 领取回调（F12），供 commission-core 统一记录
pub trait PoolRewardClaimCallback<AccountId, Balance, TokenBalance> {
    fn on_pool_reward_claimed(
        entity_id: u64, account: &AccountId,
        nex_amount: Balance, token_amount: TokenBalance,
        round_id: u64, level_id: u8,
    );
}

// KYC/合规参与权检查
pub trait ParticipationGuard<AccountId> {
    fn can_participate(entity_id: u64, account: &AccountId) -> bool;
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
    type PoolBalanceProvider: PoolBalanceProvider<BalanceOf<Self>>;

    #[pallet::constant]
    type MaxPoolRewardLevels: Get<u32>;    // 最大等级配置数
    #[pallet::constant]
    type MaxClaimHistory: Get<u32>;        // 每用户最大领取历史数
    #[pallet::constant]
    type MinRoundDuration: Get<BlockNumberFor<Self>>; // 最小轮次间隔
    #[pallet::constant]
    type MaxRoundHistory: Get<u32>;        // 每 Entity 保留最近 N 轮历史

    // Token 多资产扩展
    type TokenBalance: FullCodec + MaxEncodedLen + TypeInfo + Copy + Default
        + Debug + AtLeast32BitUnsigned + From<u32> + Into<u128>;
    type TokenPoolBalanceProvider: TokenPoolBalanceProvider<TokenBalanceOf<Self>>;
    type TokenTransferProvider: TokenTransferProvider<Self::AccountId, TokenBalanceOf<Self>>;

    type ParticipationGuard: ParticipationGuard<Self::AccountId>;
    type WeightInfo: WeightInfo;
    type ClaimCallback: PoolRewardClaimCallback<...>;
}
```

`integrity_test` 校验: `MaxPoolRewardLevels >= 1`, `MaxClaimHistory >= 1`, `MinRoundDuration > 0`, `MaxRoundHistory >= 1`。

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PoolRewardConfigs` | `StorageMap<u64, PoolRewardConfig>` | per-entity 奖励配置 |
| `CurrentRound` | `StorageMap<u64, RoundInfo>` | 当前轮次快照 |
| `LastRoundId` | `StorageMap<u64, u64, ValueQuery>` | 保持 round_id 单调递增 |
| `LastClaimedRound` | `StorageDoubleMap<u64, AccountId, u64>` | 用户上次领取轮次（防双领） |
| `ClaimRecords` | `StorageDoubleMap<u64, AccountId, BoundedVec<ClaimRecord>>` | 用户领取历史（滚动窗口） |
| `PoolRewardPaused` | `StorageMap<u64, bool, ValueQuery>` | per-entity 暂停标志 |
| `GlobalPoolRewardPaused` | `StorageValue<bool, ValueQuery>` | 全局紧急暂停标志 |
| `RoundHistory` | `StorageMap<u64, BoundedVec<CompletedRoundSummary, MaxRoundHistory>>` | 轮次历史（FIFO） |
| `DistributionStatistics` | `StorageMap<u64, DistributionStats, ValueQuery>` | 累计分配统计 |

## Extrinsics（12 个）

| call_index | 方法 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `set_pool_reward_config` | Owner/Admin | 设置配置（保留 `token_pool_enabled`） |
| 1 | `claim_pool_reward` | Signed（会员） | 领取当前轮次 NEX + Token 奖励 |
| 2 | `force_new_round` | Owner/Admin | 手动开启新轮次（需 Entity 活跃） |
| 3 | `set_token_pool_enabled` | Owner/Admin | 启用/禁用 Token 池（幂等保护） |
| 4 | `force_set_pool_reward_config` | Root | 强制设置配置（绕过权限 + EntityLocked） |
| 5 | `force_set_token_pool_enabled` | Root | 强制 Token 开关（绕过权限 + EntityLocked） |
| 6 | `force_start_new_round` | Root | 强制新轮次（绕过权限 + EntityLocked，仍检查 active） |
| 7 | `clear_pool_reward_config` | Owner/Admin | 清除配置 + 暂停状态（不清历史） |
| 8 | `force_clear_pool_reward_config` | Root | 强制清除（无配置时静默成功） |
| 9 | `pause_pool_reward` | Owner/Admin | 暂停该 Entity 池奖励 |
| 10 | `resume_pool_reward` | Owner/Admin | 恢复该 Entity 池奖励 |
| 11 | `set_global_pool_reward_paused` | Root | 全局暂停/恢复所有 Entity |

> **权限模型：** Owner/Admin = Entity Owner 或 Admin(COMMISSION_MANAGE)，受 `EntityLocked` 保护。Root `force_*` 绕过锁定和权限。`PoolRewardPlanWriter` trait 也绕过权限。

### set_pool_reward_config 校验

由 `validate_level_ratios` 共享方法完成（extrinsic + PlanWriter 统一）：

- `round_duration > 0` 且 `>= MinRoundDuration`
- `level_ratios` 无重复 `level_id`
- 每个 `ratio` ∈ (0, 10000]，总和 = 10000
- 自动保留现有 `token_pool_enabled` 值
- 配置变更后调用 `invalidate_current_round`（保持 round_id 单调递增）

### claim_pool_reward 流程

```
 1. is_entity_active
 2. !GlobalPoolRewardPaused
 3. !PoolRewardPaused[entity_id]
 4. is_member + ParticipationGuard::can_participate
 5. custom_level_id 在 level_ratios 中
 6. ensure_current_round（过期则创新轮 + F10 归档 + F11 详细事件）
 7. last_claimed_round < current_round_id（防双领）
 8. claimed_count < member_count（配额检查）
 9. NEX: deduct_pool → Currency::transfer（先扣记账后转实物）
10. Token: best-effort（失败不影响 NEX；扣池失败则回滚转账）
11. claimed_count++, ClaimRecords 写入（滚动窗口）
12. DistributionStatistics 累加
13. ClaimCallback::on_pool_reward_claimed
```

### clear_pool_reward_config 清除范围

extrinsic（call_index 7/8）清除：`PoolRewardConfigs` + `CurrentRound` + `LastRoundId` + `PoolRewardPaused`。`PoolRewardPlanWriter::clear_config` 额外清除 `LastClaimedRound` + `ClaimRecords` + `RoundHistory` + `DistributionStatistics`。

## 内部方法

| 方法 | 说明 |
|------|------|
| `validate_level_ratios` | 校验等级比率（重复、范围、总和） |
| `build_level_snapshots<B>` | 泛型快照构建，NEX/Token 通用 |
| `ensure_current_round` | 轮次有效则返回，否则创建新轮 |
| `create_new_round` | 缓存会员数 → 快照 → 归档旧轮 → 事件 |
| `invalidate_current_round` | 保存 round_id 到 `LastRoundId` 后移除轮次 |
| `ensure_owner_or_admin` | Entity 活跃 + Owner/Admin(COMMISSION_MANAGE) |
| `get_claimable` | 预查询可领取 `(nex, token)`，只读不写 |
| `simulate_claimable` | 模拟新轮次快照计算（轮次过期时） |
| `get_round_statistics` | 当前轮次各等级领取进度 |

## PoolRewardPlanWriter

供 `pallet-commission-core` / 治理写入配置，绕过权限检查：

```rust
trait PoolRewardPlanWriter {
    fn set_pool_reward_config(entity_id, level_ratios, round_duration) -> DispatchResult;
    fn clear_config(entity_id: u64) -> DispatchResult;
    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> DispatchResult; // 默认 no-op
}
```

- `set_pool_reward_config`: 共用 `validate_level_ratios`，保留 `token_pool_enabled`，发出 `PoolRewardConfigUpdated` 事件
- `clear_config`: 清除全部 8 项存储（含 `LastClaimedRound`/`ClaimRecords` 的 `clear_prefix`），发出 `PoolRewardConfigCleared` 事件
- `set_token_pool_enabled`: 仅值变更时失效轮次，发出 `TokenPoolEnabledUpdated` 事件

## Events（13 个）

| 事件 | 字段 | 说明 |
|------|------|------|
| `PoolRewardConfigUpdated` | entity_id | 配置已更新 |
| `NewRoundStarted` | entity_id, round_id, pool_snapshot, token_pool_snapshot | 新轮次创建 |
| `PoolRewardClaimed` | entity_id, account, amount, token_amount, round_id, level_id | 用户领取 |
| `TokenPoolEnabledUpdated` | entity_id, enabled | Token 池开关变更 |
| `RoundForced` | entity_id, round_id | 强制新轮次 |
| `TokenTransferRollbackFailed` | entity_id, account, amount | Token 回滚失败（需人工干预） |
| `PoolRewardConfigCleared` | entity_id | 配置已清除 |
| `PoolRewardPausedEvent` | entity_id | Entity 池奖励已暂停 |
| `PoolRewardResumedEvent` | entity_id | Entity 池奖励已恢复 |
| `GlobalPoolRewardPausedEvent` | — | 全局已暂停 |
| `GlobalPoolRewardResumedEvent` | — | 全局已恢复 |
| `RoundArchived` | entity_id, round_id | 旧轮次已归档 |
| `NewRoundDetails` | entity_id, round_id, pool_snapshot, token_pool_snapshot, level_snapshots, token_level_snapshots | 新轮详细快照 |

## Errors（22 个）

| 错误 | 说明 |
|------|------|
| `InvalidRatio` | ratio 不在 (0, 10000] |
| `RatioSumMismatch` | 比率总和 ≠ 10000 |
| `DuplicateLevelId` | 重复 level_id |
| `InvalidRoundDuration` | round_duration = 0 |
| `RoundDurationTooShort` | round_duration < MinRoundDuration |
| `ConfigNotFound` | 无沉淀池配置 |
| `EntityNotActive` | Entity 不存在或未激活 |
| `NotAuthorized` | 非 Owner/Admin |
| `EntityLocked` | 实体已锁定 |
| `NotMember` | 非会员 |
| `ParticipationRequirementNotMet` | 未满足参与要求（KYC） |
| `LevelNotConfigured` | 用户等级未在配置中 |
| `LevelNotInSnapshot` | 等级未在快照中 |
| `AlreadyClaimed` | 本轮已领取 |
| `LevelQuotaExhausted` | 等级配额已满 |
| `NothingToClaim` | 可领取金额 = 0 |
| `InsufficientPool` | NEX 池余额不足 |
| `RoundIdOverflow` | round_id 达 u64::MAX |
| `PoolRewardIsPaused` | Entity 池奖励已暂停 |
| `PoolRewardNotPaused` | Entity 池奖励未暂停 |
| `GlobalPaused` | 全局已暂停 |
| `GlobalNotPaused` | 全局未暂停 |

## Weight

基于 DB read/write 分析估算（`weights.rs`），后续可通过 benchmark 替换。

| Extrinsic | Reads | Writes | ref_time | proof_size |
|-----------|-------|--------|----------|------------|
| `set_pool_reward_config` | 4 | 3 | 50M | 6K |
| `claim_pool_reward` | 10 | 7 | 150M | 15K |
| `force_new_round` | 7+N | 1 | 110M | 11K |
| `set_token_pool_enabled` | 4 | 3 | 45M | 5K |
| `clear_pool_reward_config` | 4 | 4 | 40M | 5K |
| `pause_pool_reward` | 4 | 1 | 30M | 4K |
| `resume_pool_reward` | 4 | 1 | 30M | 4K |
| `set_global_pool_reward_paused` | 1 | 1 | 15M | 2K |

## 依赖

```toml
[dependencies]
codec = { features = ["derive"], workspace = true }
scale-info = { features = ["derive"], workspace = true }
frame-support = { workspace = true }
frame-system = { workspace = true }
sp-runtime = { workspace = true }
pallet-entity-common = { path = "../../common", default-features = false }
pallet-commission-common = { path = "../common", default-features = false }

[features]
runtime-benchmarks = ["frame-support/..", "frame-system/..", "sp-runtime/..", "pallet-entity-common/..", "pallet-commission-common/.."]
try-runtime = ["frame-support/..", "frame-system/..", "sp-runtime/..", "pallet-entity-common/..", "pallet-commission-common/.."]

```

## 测试覆盖（136 tests）

共 **136** 个测试，覆盖全部 12 个 extrinsic、3 个 PlanWriter 方法、3 个查询方法和 8 轮审计回归。

### 按类别统计

| 类别 | 数量 | 说明 |
|------|------|------|
| 配置测试 | 7 | set_config 正常/异常路径 |
| 轮次测试 | 4 | 创建/复用/过期/强制新轮 |
| 领取测试 | 9 | 正常 claim + 权限/配额/余额异常 |
| 领取历史 | 3 | 记录写入/跨轮/滚动淘汰 |
| PlanWriter | 4 | set_config/clear/token_enabled + 新存储清除 |
| Token 双池 | 6 | 开关/快照/双池 claim/best-effort |
| 边界与集成 | 10 | 多实体隔离/跨轮领取/Token 回滚/空池 |
| 权限 (P0) | 7 | Admin/Owner/Root 权限验证 |
| EntityLocked (P1) | 10 | 锁定保护 + Root force 绕过 |
| clear_config (P2) | 8 | Owner/Admin/Root 清除 + 异常 |
| 暂停 (F3) | 9 | pause/resume + claim 阻塞 |
| MinRoundDuration (F4) | 3 | duration 校验 |
| get_claimable (F1) | 5 | 预查询各场景 |
| get_round_statistics (F5) | 2 | 无轮/有轮进度 |
| 全局暂停 (F8) | 6 | 全局 pause/resume + claim 阻塞 |
| 分配统计 (F9) | 3 | claim/累加/轮次完成统计 |
| 轮次历史 (F10) | 3 | 归档/FIFO 淘汰/事件 |
| NewRoundDetails (F11) | 1 | 详细快照事件 |
| 审计回归 R1 | 8 | H1-H3/M1 修复验证 |
| 审计回归 R2 | 5 | H2-R2/M2-R2 修复验证 |
| 审计回归 R3 | 6 | M1-R3/M2-R3/L1-R3 修复验证 |
| 审计回归 R4 | 4 | M1-R4/L1-R4 修复验证 |
| 审计回归 R5 | 3 | M1-R5/M3-R5 修复验证 |
| 审计回归 R7 | 4 | M1-R7/M2-R7 修复验证 |
| 审计回归 R8 | 4 | M1-R8 封禁/冻结会员 claim 阻塞 + get_claimable 返零 |
| 全局+实体暂停交互 | 1 | 全局暂停优先于实体恢复 |
| 参与权检查 | 1 | ParticipationGuard 拒绝 claim |

---

## 审计记录摘要

共经历 **8 轮审计** + **3 次功能增强批次**（P0/P1/P2 权限 + F1-F12 功能），累计修复 **4 High + 11 Medium + 13 Low** 问题。当前 **136 tests** 全部通过。

### 已修复的安全问题

| Round | ID | 严重度 | 描述 |
|-------|----|--------|------|
| R1 | H1 | High | PlanWriter 绕过 `validate_level_ratios` 校验 |
| R1 | H2 | High | `set_pool_reward_config` 硬编码 `token_pool_enabled: false` |
| R1 | H3 | High | `clear_config` 不清理 `LastClaimedRound`/`ClaimRecords` |
| R2 | H2-R2 | High | 配置更新后不清除 `CurrentRound`，旧快照与新配置不一致 |
| R1 | M1 | Medium | `round_id = u64::MAX` 时 `saturating_add` 产生重复 ID |
| R2 | M2-R2 | Medium | `claim_pool_reward` 不检查 Entity 是否激活 |
| R2 | M3-R2 | Medium | NEX 转账顺序：先转后扣 → 改为先扣后转 |
| R3 | M1-R3 | Medium | Token 开关不使当前轮次失效 |
| R3 | M2-R3 | Medium | 配置更新用 `clear_prefix` O(n)；round_id 重置为 1 |
| R4 | M1-R4 | Medium | weights.rs DB 计数不同步 |
| R5 | M1-R5 | Medium | `force_new_round` 不检查 Entity 激活 |
| R5 | M3-R5 | Medium | `clear_config` 发出错误事件类型 |
| R7 | M1-R7 | Medium | `clear_pool_reward_config` 不清除 `PoolRewardPaused` 残留 |
| R7 | M2-R7 | Medium | 3 个 extrinsic 复用错误权重函数 |
| R8 | M1-R8 | Medium | `claim_pool_reward`/`get_claimable` 缺 `is_banned`/`is_member_active` 检查 — 封禁或冻结会员仍可领取。修复: 添加检查 |
| R8 | L1-R8 | Low | Cargo.toml 缺 `pallet-entity-common`/`pallet-commission-common` 的 `runtime-benchmarks`/`try-runtime` feature 传播。修复: 已添加 |

### 记录但未修复（设计权衡）

| ID | 严重度 | 描述 |
|----|--------|------|
| M1-R2 | Medium | `build_level_snapshots` 整除截断尘埃累积（滚入下轮，无资金丢失） |
| M2-R5 | Medium | 双整除可合并为单除避免精度损失（同 M1-R2） |
| L1-R5 | Low | `claim_history` 淘汰 `remove(0)` O(n)，MaxClaimHistory ≤ 5 可忽略 |
| L1-R7 | Low | claim 时读当前等级非快照等级（设计权衡） |
| L2-R7 | Low | `force_new_round` 暂停期间仍可创建新轮（force 操作不受暂停限制） |
| L3-R7 | Low | Root `force_*` 的 `is_entity_active` 检查不一致 |

### 功能增强批次

| 批次 | 内容 | 新增 extrinsics | 新增测试 |
|------|------|-----------------|----------|
| P0 | 权限下放: Root → Owner/Admin(COMMISSION_MANAGE) | 0（改造现有） | 7 |
| P1 | EntityLocked 保护 + Root `force_*` 覆写 | 3 (call_index 4/5/6) | 10 |
| P2 | `clear_pool_reward_config` + `force_clear` | 2 (call_index 7/8) | 8 |
| F1-F12 | 预查询/暂停/统计/历史/回调等 10 项功能 | 3 (call_index 9/10/11) | 34 |

### 版本演进

| 阶段 | 测试数 | 关键变更 |
|------|--------|----------|
| 初始版本 | 52 | 基础 claim + Token 双池 |
| R1 修复 | 58 | H1-H3 安全修复 + 冗余清理 |
| R2 修复 | 58 | H2-R2 快照一致性 + Entity 激活检查 |
| R3 修复 | 64 | `LastRoundId` 单调递增 + Token 开关生效 |
| R4 修复 | 68 | Weight 同步 + 幂等保护 |
| R5 修复 | 71 | force_new_round 激活检查 + Cleared 事件 |
| P0 权限 | 76 | Owner/Admin 权限模型 |
| P1 锁定 | 86 | EntityLocked + Root force_* |
| P2 清除 | 94 | clear_config extrinsics |
| F1-F12 | 126 | 暂停/查询/统计/历史/回调 |
| R7 修复 | 130 | 暂停状态清除 + 专用权重函数 |
| R8 修复 | 136 | 封禁/冻结会员 claim 阻塞 + Cargo features |
