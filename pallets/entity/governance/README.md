# pallet-entity-governance

> 实体代币治理模块 — 多模式去中心化决策系统

- **Runtime Pallet Index**: 125
- **版本**: v0.6.0

## 概述

`pallet-entity-governance` 实现基于实体代币的治理系统。支持 2 种治理模式、44 种提案类型、可选管理员否决权、委托投票，以及首次持有时间校验防护机制。

### 核心能力

- **2 种治理模式** — None（管理员全权）/ FullDAO（代币投票，可选管理员否决权作为紧急制动）
- **44 种提案类型** — 商品、店铺、代币、财务、返佣、提现、会员等级、治理参数、社区
- **管理员否决权** — FullDAO 模式下可启用，作为紧急制动机制
- **委托投票** — Compound 模型，委托后不可直投，代理人拥有委托者投票权重
- **闪电贷防护** — 首次持有时间校验 + 投票权快照
- **投票代币锁定** — reserve/unreserve + max-lock 引用计数
- **终态提案清理** — `cleanup_proposal` 释放存储空间

## 架构

```
┌────────────────────────────────────────────────────────────────────┐
│                    pallet-entity-governance                        │
│                       (Runtime Index: 125)                         │
├────────────────────────────────────────────────────────────────────┤
│  提案创建 → 代币投票 → 结果判定 → 执行/否决 → 清理                 │
│  治理配置 → 锁定治理                                                │
└───────┬──────────┬──────────────┬──────────────┬──────────────────┘
        │          │              │              │
   EntityProvider  ShopProvider   TokenProvider  CommissionProvider
        │          │              │         + MemberProvider
        ▼          ▼              ▼              ▼
   entity-registry entity-shop  entity-token  commission-core
                                              + entity-member
```

### 依赖 Trait

| Trait | 来源 | 用途 |
|-------|------|------|
| `EntityProvider` | pallet-entity-common | 实体所有权查询 |
| `ShopProvider` | pallet-entity-common | 店铺存在性、所有权、暂停/恢复操作 |
| `EntityTokenProvider` | pallet-entity-common | 代币余额、总供应量、启用状态、TokenType 查询 |
| `CommissionProvider` | pallet-entity-commission | 返佣模式/费率/提现配置的链上写入 |
| `MemberProvider` | pallet-entity-commission | 自定义等级/升级模式/升级规则的链上写入 |

## 治理模式

2 种模式定义在 `pallet-entity-common::GovernanceMode`，每个实体可独立配置：

| 模式 | 说明 | 管理员否决 |
|------|------|----------|
| **None** | 无治理，管理员全权控制，禁止创建提案 | - |
| **FullDAO** | 完全 DAO，所有决策需代币投票，可选管理员否决权（紧急制动） | 可选 |

## 数据结构

### GovernanceConfig — 实体治理配置

```rust
pub struct GovernanceConfig<BlockNumber> {
    pub mode: GovernanceMode,          // 治理模式 (None / FullDAO)
    pub voting_period: BlockNumber,    // 投票期（0 = 使用全局默认）
    pub execution_delay: BlockNumber,  // 执行延迟（0 = 使用全局默认）
    pub quorum_threshold: u8,          // 法定人数阈值 (%)
    pub pass_threshold: u8,            // 通过阈值 (%)
    pub proposal_threshold: u16,       // 提案创建门槛（基点）
    pub admin_veto_enabled: bool,      // 管理员否决权（FullDAO 可选紧急制动）
}
```

### Proposal — 提案

```rust
pub struct Proposal<T: Config> {
    pub id: ProposalId,                              // 提案 ID (u64)
    pub entity_id: u64,                              // 实体 ID（1:N 多店铺架构）
    pub proposer: T::AccountId,                      // 提案者
    pub proposal_type: ProposalType<BalanceOf<T>>,   // 提案类型
    pub title: BoundedVec<u8, T::MaxTitleLength>,    // 标题
    pub description_cid: Option<BoundedVec<u8, T::MaxCidLength>>, // 描述 CID
    pub status: ProposalStatus,                      // 状态
    pub created_at: BlockNumberFor<T>,               // 创建时间
    pub voting_start: BlockNumberFor<T>,             // 投票开始
    pub voting_end: BlockNumberFor<T>,               // 投票结束
    pub execution_time: Option<BlockNumberFor<T>>,   // 执行时间
    pub yes_votes: BalanceOf<T>,                     // 赞成票
    pub no_votes: BalanceOf<T>,                      // 反对票
    pub abstain_votes: BalanceOf<T>,                 // 弃权票
    // ========== 治理参数快照（防止投票期间参数被篡改）==========
    pub snapshot_quorum: u8,                         // 快照: 法定人数阈值
    pub snapshot_pass: u8,                           // 快照: 通过阈值
    pub snapshot_execution_delay: BlockNumberFor<T>, // 快照: 执行延迟
    pub snapshot_total_supply: BalanceOf<T>,         // 快照: 代币总供应量
}
```

### VoteRecord — 投票记录

```rust
pub struct VoteRecord<AccountId, Balance, BlockNumber> {
    pub voter: AccountId,       // 投票者
    pub vote: VoteType,         // 投票类型 (Yes/No/Abstain)
    pub weight: Balance,        // 投票权重
    pub voted_at: BlockNumber,  // 投票时间
}
```

### ProposalStatus — 提案状态

```
Voting → Passed → Executed
       → Failed
       → Cancelled (提案者/实体所有者取消, 或被否决)
       Passed → Expired (执行窗口过期)
```

| 状态 | 说明 |
|------|------|
| `Voting` | 投票中（提案创建后的初始状态） |
| `Passed` | 投票通过，等待执行 |
| `Failed` | 投票未通过（未达法定人数或通过阈值） |
| `Executed` | 已执行 |
| `Cancelled` | 已取消 / 被否决 |
| `Expired` | 执行窗口已过期（Passed 后未及时执行） |

## 提案类型（共 44 种）

### 商品管理类 (4)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `PriceChange` | 商品价格调整 | 事件记录 |
| `ProductListing` | 新商品上架 | 链下 CID 解析 |
| `ProductDelisting` | 商品下架 | 事件记录 |
| `InventoryAdjustment` | 库存调整 | 事件记录 |

### 店铺运营类 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `Promotion` | 促销活动 | 事件记录 |
| `ShopNameChange` | 修改店铺名称 | 链下确认 |
| `ShopDescriptionChange` | 修改店铺描述 | 链下确认 |
| `ShopPause { shop_id }` | 暂停指定店铺营业 | **链上执行** `ShopProvider::pause_shop` |
| `ShopResume { shop_id }` | 恢复指定店铺营业 | **链上执行** `ShopProvider::resume_shop` |

### 代币经济类 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TokenConfigChange` | 代币配置修改 | 事件记录 |
| `TokenMint` | 增发代币 | 链下执行 |
| `TokenBurn` | 销毁代币 | 事件记录 |
| `AirdropDistribution` | 空投分发 | 链下执行 |
| `Dividend` | 分红提案 | 事件记录 |

### 财务管理类 (4)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TreasurySpend` | 金库支出 | 链下执行 |
| `FeeAdjustment` | 手续费调整 | 事件记录 |
| `RevenueShare` | 收益分配比例 | 事件记录 |
| `RefundPolicy` | 退款政策调整 | 事件记录 |

### 治理参数类 (6)

| 类型 | 说明 | 执行方式 |
|------|------|--------|
| `VotingPeriodChange` | 投票期调整 | **链上执行** 更新 GovernanceConfig |
| `QuorumChange` | 法定人数调整 | **链上执行** 更新 GovernanceConfig |
| `ProposalThresholdChange` | 提案门槛调整 | **链上执行** 更新 GovernanceConfig |
| `ExecutionDelayChange` | 执行延迟调整（lock 后仍可通过提案修改） | **链上执行** 更新 GovernanceConfig |
| `PassThresholdChange` | 通过阈值调整（lock 后仍可通过提案修改） | **链上执行** 更新 GovernanceConfig |
| `AdminVetoToggle` | 管理员否决权开关（lock 后仍可通过提案修改） | **链上执行** 更新 GovernanceConfig |

### 返佣配置类 (8)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommissionModesChange` | 启用/禁用返佣模式 | **链上执行** `CommissionProvider` |
| `DirectRewardChange` | 直推奖励费率 | **链上执行** `CommissionProvider` |
| `MultiLevelChange` | 多级分销配置 | ⛔ 创建时拒绝（链上执行未实现） |
| `LevelDiffChange` | 等级差价配置（5 级费率） | **链上执行** `CommissionProvider` |
| `FixedAmountChange` | 固定金额配置 | **链上执行** `CommissionProvider` |
| `FirstOrderChange` | 首单奖励配置 | **链上执行** `CommissionProvider` |
| `RepeatPurchaseChange` | 复购奖励配置 | **链上执行** `CommissionProvider` |
| `SingleLineChange` | 单线收益配置 | ⛔ 创建时拒绝（链上执行未实现） |

### 提现配置类 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `WithdrawalConfigChange` | 分级提现配置 | **链上执行** `CommissionProvider` |
| `MinRepurchaseRateChange` | 全局最低复购比例底线 | **链上执行** `CommissionProvider` |

### 会员等级体系类 (7)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `AddCustomLevel` | 添加自定义等级 | **链上执行** `MemberProvider` |
| `UpdateCustomLevel` | 更新自定义等级 | **链上执行** `MemberProvider` |
| `RemoveCustomLevel` | 删除自定义等级 | **链上执行** `MemberProvider` |
| `SetUpgradeMode` | 设置升级模式 (Auto/Manual/PeriodReset) | **链上执行** `MemberProvider` |
| `EnableCustomLevels` | 启用/禁用自定义等级 | **链上执行** `MemberProvider` |
| `AddUpgradeRule` | 添加升级规则 | ⛔ 创建时拒绝（链上执行未实现） |
| `RemoveUpgradeRule` | 删除升级规则 | ⛔ 创建时拒绝（链上执行未实现） |

### 社区类 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommunityEvent` | 社区活动 | 仅记录 |
| `RuleSuggestion` | 规则建议 | 仅记录 |
| `General` | 通用提案 | 仅记录 |

## 存储项

| 存储 | 类型 | Key | 说明 |
|------|------|-----|------|
| `NextProposalId` | ValueQuery | - | 下一个提案 ID |
| `Proposals` | StorageMap | proposal_id | 提案详情 |
| `EntityProposals` | StorageMap | entity_id | 实体活跃提案列表 (BoundedVec) |
| `VoteRecords` | StorageDoubleMap | (proposal_id, account) | 投票记录 |
| `FirstHoldTime` | StorageDoubleMap | (entity_id, account) | 用户首次持有代币时间 |
| `VotingPowerSnapshot` | StorageDoubleMap | (proposal_id, account) | 投票权快照余额 |
| `GovernanceConfigs` | StorageMap | entity_id | 实体治理配置 |
| `GovernanceLocked` | StorageMap | entity_id | 治理配置是否已锁定 |
| `VoterTokenLocks` | StorageDoubleMap | (proposal_id, account) | H2: 投票者参与记录（用于批量解锁） |
| `GovernanceLockCount` | StorageDoubleMap | (entity_id, account) | H2: 活跃投票提案引用计数 |
| `GovernanceLockAmount` | StorageDoubleMap | (entity_id, account) | H2: 最大锁定代币数 |
| `VoteDelegation` | StorageDoubleMap | (entity_id, delegator) | F5: 投票委托映射 (delegator → delegate) |
| `DelegatedVoters` | StorageDoubleMap | (entity_id, delegate) | F5: 委托者列表 (BoundedVec) |

## Extrinsics

### call_index(0) — create_proposal

创建治理提案。

```rust
fn create_proposal(
    origin: OriginFor<T>,
    shop_id: u64,
    proposal_type: ProposalType<BalanceOf<T>>,
    title: Vec<u8>,
    description_cid: Option<Vec<u8>>,
) -> DispatchResult
```

- **权限**: 持有店铺代币 >= 总供应量 × `MinProposalThreshold` (默认 1%)
- **校验**: 店铺存在、治理模式 ≠ None、代币启用、参数有效性、活跃提案数 < MaxActiveProposals
- **参数验证**: 费率/比例类字段 ≤ 10000 (basis points)，百分比 ≤ 100，RevenueShare 之和 ≤ 10000

### call_index(1) — vote

对提案投票。

```rust
fn vote(
    origin: OriginFor<T>,
    proposal_id: ProposalId,
    vote: VoteType,
) -> DispatchResult
```

- **权限**: 持有店铺代币且 `FirstHoldTime <= 提案创建时间`
- **投票权重**: `min(当前余额 + 委托权重, 快照余额)` × 时间加权
- **校验**: 代币 TokenType 具有投票权 (`has_voting_power()`)、未重复投票、投票期内、未委托投票权
- **快照**: 首次投票时锁定当前余额到 `VotingPowerSnapshot`

### call_index(2) — finalize_voting

结束投票并计算结果。任何人可调用（投票期结束后）。

- **法定人数**: `总投票 >= 总供应量 × QuorumThreshold%`
- **通过阈值**: `赞成票 > 总投票 × PassThreshold%`
- 通过后设置 `execution_time = now + ExecutionDelay`

### call_index(3) — execute_proposal

执行通过的提案。任何人可调用（执行时间到达后）。

- 根据 `ProposalType` 调用对应的 Provider 方法
- 链上可直接执行的提案类型立即生效
- 需要链下解析的提案发出 `ProposalExecutionNote` 事件

### call_index(4) — cancel_proposal

取消提案。

- **权限**: 提案者或实体所有者
- **限制**: 仅 Voting 状态

### call_index(5) — configure_governance

配置实体治理模式。

```rust
fn configure_governance(
    origin: OriginFor<T>,
    entity_id: u64,
    mode: GovernanceMode,
    voting_period: Option<BlockNumberFor<T>>,
    execution_delay: Option<BlockNumberFor<T>>,
    quorum_threshold: Option<u8>,
    pass_threshold: Option<u8>,
    proposal_threshold: Option<u16>,
    admin_veto_enabled: Option<bool>,
) -> DispatchResult
```

- **权限**: 实体（店铺）所有者
- **校验**: 投票期 >= MinVotingPeriod，执行延迟 >= MinExecutionDelay，quorum/pass ≤ 100，proposal_threshold ≤ 10000
- **限制**: 治理未锁定（`GovernanceLocked == false`）

### call_index(9) — veto_proposal

管理员否决提案。

```rust
fn veto_proposal(
    origin: OriginFor<T>,
    proposal_id: ProposalId,
) -> DispatchResult
```

- **权限**: 实体所有者
- **限制**: `admin_veto_enabled == true`（FullDAO 模式下可选紧急制动）
- **适用状态**: Voting 或 Passed

### call_index(10) — lock_governance

锁定治理配置（永久不可逆）。

- **权限**: 实体所有者
- **效果**: 锁定后 Owner 不可再修改治理参数，此操作不可撤销
- **None 锁定**: 永久冻结治理配置，明确"永不启用 DAO"（适用于未发代币实体）
- **FullDAO 锁定**: 放弃控制权，仅可通过提案修改治理参数

### call_index(11) — cleanup_proposal

清理终态提案（Executed/Failed/Cancelled/Expired），释放存储空间。

- **权限**: 任何人可调用
- **限制**: 提案必须处于终态

### call_index(12) — delegate_vote

委托投票权。将自己在某实体的投票权委托给另一个账户。委托后不可直接投票（Compound 模型）。

```rust
fn delegate_vote(
    origin: OriginFor<T>,
    entity_id: u64,
    delegate: T::AccountId,
) -> DispatchResult
```

- **权限**: 代币持有者
- **校验**: 实体存在且活跃、代币已启用、不可自我委托、未已有委托关系、委托接收者未达上限

### call_index(13) — undelegate_vote

取消投票委托，恢复直接投票能力。

```rust
fn undelegate_vote(
    origin: OriginFor<T>,
    entity_id: u64,
) -> DispatchResult
```

- **权限**: 已委托的用户
- **限制**: 必须有现有委托关系

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `ProposalCreated` | proposal_id, entity_id, proposer, title | 提案已创建 |
| `Voted` | proposal_id, voter, vote, weight | 已投票 |
| `ProposalPassed` | proposal_id | 提案通过 |
| `ProposalFailed` | proposal_id | 提案未通过 |
| `ProposalExecuted` | proposal_id | 提案已执行 |
| `ProposalCancelled` | proposal_id | 提案已取消 |
| `GovernanceConfigUpdated` | entity_id, mode | 治理配置已更新 |
| `ProposalVetoed` | proposal_id, by | 提案被否决 |
| `ProposalExecutionNote` | proposal_id, note | 执行备注（链下执行） |
| `GovernanceConfigLocked` | entity_id | 治理配置已锁定（不可再修改） |
| `GovernanceSyncFailed` | entity_id, mode | 治理模式同步到 registry 失败 |
| `ProposalExpired` | proposal_id | 提案执行窗口已过期 |
| `ProposalCleaned` | proposal_id | 终态提案已被清理（释放存储） |
| `VoteDelegated` | entity_id, delegator, delegate | F5: 投票权已委托 |
| `VoteUndelegated` | entity_id, delegator | F5: 投票委托已撤销 |

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 店铺不存在 |
| `NotShopOwner` | 不是店主 |
| `TokenNotEnabled` | 代币未启用 |
| `ProposalNotFound` | 提案不存在 |
| `InsufficientTokensForProposal` | 代币不足以创建提案 |
| `TooManyActiveProposals` | 已达最大活跃提案数 |
| `InvalidProposalStatus` | 状态不允许此操作 |
| `AlreadyVoted` | 已投过票 |
| `NoVotingPower` | 没有投票权 |
| `VotingEnded` | 投票期已结束 |
| `VotingNotEnded` | 投票期未结束 |
| `ExecutionTimeNotReached` | 执行时间未到 |
| `TitleTooLong` | 标题过长 |
| `CidTooLong` | CID 过长 |
| `CannotCancel` | 无权取消 |
| `GovernanceModeNotAllowed` | 治理模式不允许此操作 |
| `NoVetoRight` | 无否决权 |
| `TokenTypeNoVotingPower` | 代币类型不具有投票权 |
| `InvalidParameter` | 参数无效（费率超范围等） |
| `ProposalTypeNotImplemented` | 提案类型暂未实现链上执行 |
| `GovernanceConfigIsLocked` | 治理配置已锁定，不可修改 |
| `GovernanceAlreadyLocked` | 治理配置已经锁定过 |
| `VotingPeriodTooShort` | 投票期低于最小值 |
| `ExecutionDelayTooShort` | 执行延迟低于最小值 |
| `TokenNotEnabledForDAO` | FullDAO 需要先发行代币 |
| `ProposalIdOverflow` | 提案 ID 溢出 |
| `ProposalNotTerminal` | 提案未处于终态，不可清理 |
| `VotePowerDelegated` | F5: 已委托投票权，不可直接投票（需先取消委托） |
| `AlreadyDelegated` | F5: 已有委托关系 |
| `NotDelegated` | F5: 无委托关系 |
| `SelfDelegation` | F5: 不可自我委托 |
| `TooManyDelegators` | F5: 委托接收者已达上限 |
| `ProposalTypeNotSupported` | R3: 提案类型暂不支持创建（链上执行未实现） |

## Runtime 配置

```rust
parameter_types! {
    pub const GovernanceVotingPeriod: BlockNumber = 100800;      // 7 天
    pub const GovernanceExecutionDelay: BlockNumber = 28800;     // 2 天
    pub const GovernancePassThreshold: u8 = 50;                 // 50%
    pub const GovernanceQuorumThreshold: u8 = 10;               // 10%
    pub const GovernanceMinProposalThreshold: u16 = 100;        // 1% (基点)
}

impl pallet_entity_governance::Config for Runtime {
    type Balance = Balance;
    type EntityProvider = EntityRegistry;
    type ShopProvider = EntityShop;
    type TokenProvider = EntityTokenProvider;
    type CommissionProvider = EntityCommissionProvider;
    type MemberProvider = EntityMemberProvider;
    type VotingPeriod = GovernanceVotingPeriod;
    type ExecutionDelay = GovernanceExecutionDelay;
    type PassThreshold = GovernancePassThreshold;
    type QuorumThreshold = GovernanceQuorumThreshold;
    type MinProposalThreshold = GovernanceMinProposalThreshold;
    type MaxTitleLength = ConstU32<128>;
    type MaxCidLength = ConstU32<64>;
    type MaxActiveProposals = ConstU32<10>;
    type MaxDelegatorsPerDelegate = ConstU32<50>;
    type MinVotingPeriod = GovernanceMinVotingPeriod;
    type MinExecutionDelay = GovernanceMinExecutionDelay;
    type TimeWeightFullPeriod = GovernanceTimeWeightFullPeriod;
    type TimeWeightMaxMultiplier = GovernanceTimeWeightMaxMultiplier;
}
```

### 配置参数说明

| 参数 | 类型 | 说明 | 值 |
|------|------|------|-----|
| `VotingPeriod` | BlockNumber | 投票期长度 | 100800 (~7天) |
| `ExecutionDelay` | BlockNumber | 执行延迟 | 28800 (~2天) |
| `PassThreshold` | u8 | 通过阈值 (%) | 50 |
| `QuorumThreshold` | u8 | 法定人数阈值 (%) | 10 |
| `MinProposalThreshold` | u16 | 提案创建门槛 (基点) | 100 (1%) |
| `MaxTitleLength` | u32 | 标题最大长度 | 128 |
| `MaxCidLength` | u32 | CID 最大长度 | 64 |
| `MaxActiveProposals` | u32 | 每实体最大活跃提案数 | 10 |
| `MaxDelegatorsPerDelegate` | u32 | 每个委托接收者最大委托人数 | 50 |
| `MinVotingPeriod` | BlockNumber | 最小投票期 | 14400 (~1天) |
| `MinExecutionDelay` | BlockNumber | 最小执行延迟 | 7200 (~12小时) |
| `TimeWeightFullPeriod` | BlockNumber | 时间加权满周期 | 604800 (~6周) |
| `TimeWeightMaxMultiplier` | u32 | 时间加权最大倍率（万分比） | 30000 (3x) |

## 安全机制

### 1. 闪电贷防护（快照机制）

```
创建提案 → 记录 created_at
投票时 → 检查 FirstHoldTime <= created_at
       → 投票权重 = min(当前余额, 快照余额) × 时间加权
```

攻击者无法通过借入代币→投票→归还来操纵投票结果。

### 2. TokenType 投票权检查

投票前校验代币类型的 `has_voting_power()` 方法，确保仅具备投票权的代币类型可参与治理。

### 3. 创建提案门槛

需持有 >= 1% 总供应量的代币才能创建提案，防止垃圾提案。

### 4. 法定人数

总投票需 >= 10% 总供应量，确保足够参与度。

### 5. 执行延迟与过期窗口

通过后需等待执行延迟才能执行，给社区反应时间。FullDAO 模式下管理员可在此窗口内否决（需启用 admin_veto）。执行窗口 = 2 × execution_delay，过期后提案转为 Expired 状态。

### 6. 投票代币锁定（H2）

投票时自动 reserve 投票者的原始代币余额，防止投票后转让给其他账户复投。使用 max-lock + 引用计数模式，支持同时投票多个提案而不重复锁定。提案结束时（finalize/cancel/veto）自动 unreserve。

### 7. 时间加权投票

持有代币时间越长，投票权重越大（最高 3x）。通过 `TimeWeightFullPeriod` 和 `TimeWeightMaxMultiplier` 配置。

### 8. 活跃提案限制

每店铺最多 10 个活跃提案，防止 DoS 攻击。

### 9. 治理模式检查

`GovernanceMode::None` 下禁止创建提案。无配置时向后兼容（允许使用全局默认参数）。

### 10. 提案参数验证

创建提案时自动校验参数有效性：费率/比例类字段 ≤ 10000 (basis points)，百分比字段 ≤ 100，`RevenueShare` 两项之和 ≤ 10000，`SetUpgradeMode` ≤ 2。

## 治理流程

### FullDAO 标准流程

```
代币持有者 create_proposal (持有 >= 1%)
    ↓
投票期 (7天) — 代币持有者 vote (权重 = 余额 × 时间加权)
    ↓
finalize_voting — 法定人数 >= 10% 且 赞成 > 50%
    ↓                              ┐
执行延迟 (2天)                     │ admin_veto_enabled 时
    ↓                              │ 管理员可否决
execute_proposal                   ┘
    ↓
cleanup_proposal — 释放存储空间
```

## 依赖

```toml
[dependencies]
pallet-entity-common = { workspace = true }
pallet-entity-commission = { workspace = true }

[dev-dependencies]
pallet-balances = { workspace = true }
```

## 测试

```bash
cargo test -p pallet-entity-governance
```

## 待实现功能

| 功能 | 说明 |
|------|------|
| 链上直接执行扩展 | 部分提案类型仅发出事件，待集成更多 Provider |
| ~~委托投票~~ | ✅ 已实现（Compound 模型，委托后不可直投） |
| ~~时间加权投票~~ | ✅ 已实现（max 3x，基于 FirstHoldTime） |
| ~~投票代币锁定~~ | ✅ 已实现（reserve/unreserve + max-lock 引用计数） |
| ~~终态提案清理~~ | ✅ 已实现 `cleanup_proposal` |
| ~~单元测试~~ | ✅ 已完成 115 个测试 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 初始版本：5 个 extrinsics，22 种提案类型 |
| v0.2.0 | 2026-02-03 | Phase 5 增强：治理模式、管理员否决、快照防护、41 种提案类型 |
| v0.2.0-audit | 2026-02-09 | 审计 R1：C1 RuntimeEvent、H1 治理模式检查、H2 提案参数验证、39 个测试 |
| v0.2.1-audit | 2026-02-16 | 审计 R2：H1 通过阈值排除弃权、H2-R2 过期优雅转 Expired、H3-R2 VoteRecords 清理、72 个测试 |
| v0.3.0-audit | 2026-03 | 审计 R3：H2 投票代币锁定、L5 移除死代码 ProposalStatus、84 个测试 |
| v0.4.0-audit | 2026-03 | 审计 R4：M1 clear_prefix 有界限制、M2 ShopPause/ShopResume 指定 shop_id、M3 移除 snapshot_block 死字段、L1 README 全面同步、L2 cleanup_proposal extrinsic、90 个测试 |
| v0.5.0-audit | 2026-03 | 审计 R5：M1 修复 lock_governance 误导性文档、L1 模块文档同步 2 种治理模式、L2 README 修正、L3 移除 3 个死错误码、L4 移除死代码、L5 移除死依赖 |
| v0.6.0 | 2026-03 | F1: 新增 3 种治理参数提案类型 (ExecutionDelayChange/PassThresholdChange/AdminVetoToggle)、R3: 创建阶段拒绝 4 种未实现提案类型、F5: 委托投票 (Compound 模型)、44 种提案类型、115 个测试 |

## 相关模块

- [pallet-entity-common](../common/README.md) — GovernanceMode、EntityProvider、ShopProvider、EntityTokenProvider
- [pallet-entity-token](../token/README.md) — 代币余额查询
- [pallet-entity-commission](../commission/README.md) — CommissionProvider 返佣配置
- [pallet-entity-member](../member/README.md) — MemberProvider 会员等级
