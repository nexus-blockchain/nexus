# pallet-entity-governance

> 实体代币治理模块 — 多模式 DAO · 51 种提案 · 委托投票 · 闪电贷防护

**pallet_index** `125` · **version** `0.9.0` · **tests** `180`

---

## 功能概览

| 功能 | 说明 |
|------|------|
| 多模式治理 | None（管理员全权）/ FullDAO（代币投票 + 可选管理员否决权） |
| 51 种提案类型 | 商品·店铺·代币·财务·治理参数·返佣·提现·会员等级·团队业绩·披露·紧急权限·社区 |
| 委托投票 | Compound 模型，委托后不可直投，代理人拥有委托者权重 |
| 投票修改 | 投票期内可改投（权重不变） |
| 闪电贷防护 | 投票权快照 + FirstHoldTime 时间门控 |
| 代币锁定 | reserve/unreserve + max-lock 引用计数，防复投 |
| 时间加权投票 | 持有越久权重越大（最高 3x） |
| 治理暂停 | Owner 可紧急暂停治理 + 批量取消提案 |
| 治理锁定 | 永久不可逆锁定（None = 永不 DAO / FullDAO = 完全放权） |
| DAO 可控紧急权限 | FullDAO 锁定后 Owner 的 pause/batch_cancel 权限受 DAO 提案控制 |
| 自动过期 | `on_idle` hook 自动 finalize 超时提案 + expire 执行窗口 |
| 终态清理 | `cleanup_proposal` 释放存储（安全增量清理） |
| GovernanceProvider | 对外提供 `governance_mode` / `has_active_proposals` / `is_governance_locked` / `is_governance_paused` 查询 |

### 设计约束

- **一实体一配置** — 治理配置绑定 Entity，1:N 多 Shop 共享同一套治理
- **参数快照** — 提案创建时快照 quorum/pass/execution_delay/total_supply，投票期间不可被篡改
- **通过阈值排除弃权** — `yes > (yes + no) × pass%`，弃权票不稀释通过率
- **C4 取消权限** — FullDAO 模式下仅提案者可取消，Owner 需走 veto 通道
- **3 种类型拒绝创建** — SingleLineChange / AddUpgradeRule / RemoveUpgradeRule（链上执行未实现）
- **DAO 可控紧急权限** — FullDAO 锁定后 Owner 的 pause/batch_cancel 权限默认开启，DAO 可通过提案关闭/重新开启

---

## 治理模式

| 模式 | 说明 | 提案创建 | 管理员否决 |
|------|------|---------|----------|
| **None** | 管理员全权控制 | ❌ 禁止 | — |
| **FullDAO** | 代币投票决策 | ✅ 持有 ≥ 门槛 | 可选（`admin_veto_enabled`） |

---

## 治理流程

```
代币持有者 create_proposal (持有 ≥ 1% 总供应量)
    ↓
投票期 (默认 7 天) — vote / change_vote / delegate_vote
    ↓
finalize_voting — 法定人数 ≥ 10% 且 赞成 > 50%
    ↓                               ┐
执行延迟 (默认 2 天)                 │ admin_veto_enabled 时
    ↓                               │ Owner 可 veto_proposal
execute_proposal                    ┘
    ↓
cleanup_proposal — 增量释放存储
```

过期处理：投票超时 → `on_idle` 自动 finalize；执行窗口 = 2 × execution_delay，超时 → Expired。

---

## 数据结构

### GovernanceConfig

```rust
pub struct GovernanceConfig<BlockNumber> {
    pub mode: GovernanceMode,       // None / FullDAO
    pub voting_period: BlockNumber, // 0 = 使用全局默认
    pub execution_delay: BlockNumber,
    pub quorum_threshold: u8,       // % (0 = 全局默认)
    pub pass_threshold: u8,
    pub proposal_threshold: u16,    // 基点 (100 = 1%)
    pub admin_veto_enabled: bool,
}
```

### Proposal

```rust
pub struct Proposal<T: Config> {
    pub id: ProposalId,              // u64
    pub entity_id: u64,
    pub proposer: T::AccountId,
    pub proposal_type: ProposalType<BalanceOf<T>>,
    pub title: BoundedVec<u8, T::MaxTitleLength>,
    pub description_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
    pub status: ProposalStatus,
    pub created_at: BlockNumberFor<T>,
    pub voting_start: BlockNumberFor<T>,
    pub voting_end: BlockNumberFor<T>,
    pub execution_time: Option<BlockNumberFor<T>>,
    pub yes_votes: BalanceOf<T>,
    pub no_votes: BalanceOf<T>,
    pub abstain_votes: BalanceOf<T>,
    // 快照（防篡改）
    pub snapshot_quorum: u8,
    pub snapshot_pass: u8,
    pub snapshot_execution_delay: BlockNumberFor<T>,
    pub snapshot_total_supply: BalanceOf<T>,
}
```

### VoteRecord

```rust
pub struct VoteRecord<AccountId, Balance, BlockNumber> {
    pub voter: AccountId,
    pub vote: VoteType,       // Yes / No / Abstain
    pub weight: Balance,
    pub voted_at: BlockNumber,
}
```

### ProposalStatus

```
Voting ─→ Passed ─→ Executed
     │         └─→ Expired (执行窗口超时)
     ├─→ Failed (法定人数或通过率不足)
     └─→ Cancelled (提案者取消 / Owner 否决)
```

---

## 提案类型（共 51 种）

### 商品管理 (4)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `PriceChange` | 商品价格调整 | **链上** `ProductProvider::update_price` |
| `ProductListing` | 新商品上架 | 链下 CID 解析 |
| `ProductDelisting` | 商品下架 | 事件记录 |
| `InventoryAdjustment` | 库存调整 | **链上** `ProductProvider::set_inventory` |

### 店铺运营 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `Promotion` | 促销活动 | 事件记录 |
| `ShopNameChange` | 修改店铺名称 | 链下确认 |
| `ShopDescriptionChange` | 修改店铺描述 | 链下确认 |
| `ShopPause { shop_id }` | 暂停指定店铺 | **链上** `ShopProvider::pause_shop` |
| `ShopResume { shop_id }` | 恢复指定店铺 | **链上** `ShopProvider::resume_shop` |

### 代币经济 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TokenConfigChange` | 代币配置修改 | 事件记录 |
| `TokenMint` | 增发代币 | 链下执行 |
| `TokenBurn` | 销毁代币 | **链上** `TokenProvider::governance_burn` |
| `AirdropDistribution` | 空投分发 | 链下执行 |
| `Dividend` | 分红提案 | 事件记录 |

### 财务管理 (4)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TreasurySpend` | 金库支出 | 链下执行 |
| `FeeAdjustment` | 手续费调整 | 事件记录 |
| `RevenueShare` | 收益分配比例 | 事件记录 |
| `RefundPolicy` | 退款政策调整 | 事件记录 |

### 治理参数 (6) — lock 后仍可通过提案修改

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `VotingPeriodChange` | 投票期调整 | **链上** GovernanceConfig |
| `QuorumChange` | 法定人数调整 | **链上** GovernanceConfig |
| `ProposalThresholdChange` | 提案门槛调整 | **链上** GovernanceConfig |
| `ExecutionDelayChange` | 执行延迟调整 | **链上** GovernanceConfig |
| `PassThresholdChange` | 通过阈值调整 | **链上** GovernanceConfig |
| `AdminVetoToggle` | 管理员否决权开关 | **链上** GovernanceConfig |

### 返佣配置 (8)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommissionModesChange` | 启用/禁用返佣模式 | **链上** `CommissionProvider` |
| `DirectRewardChange` | 直推奖励费率 | **链上** `CommissionProvider` |
| `MultiLevelChange` | 多级分销（内联 tiers 数据） | **链上** `MultiLevelWriter` |
| `LevelDiffChange` | 等级差价配置 | **链上** `CommissionProvider` |
| `FixedAmountChange` | 固定金额配置 | **链上** `CommissionProvider` |
| `FirstOrderChange` | 首单奖励配置 | **链上** `CommissionProvider` |
| `RepeatPurchaseChange` | 复购奖励配置 | **链上** `CommissionProvider` |
| `SingleLineChange` | 单线收益配置 | ⛔ 创建时拒绝 |

### 提现配置 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `WithdrawalConfigChange` | 分级提现配置 | **链上** `CommissionProvider` |
| `MinRepurchaseRateChange` | 全局最低复购比例底线 | **链上** `CommissionProvider` |

### 会员等级体系 (7)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `AddCustomLevel` | 添加自定义等级 | **链上** `MemberProvider` |
| `UpdateCustomLevel` | 更新自定义等级 | **链上** `MemberProvider` |
| `RemoveCustomLevel` | 删除自定义等级 | **链上** `MemberProvider` |
| `SetUpgradeMode` | 升级模式 (Auto/Manual/PeriodReset) | **链上** `MemberProvider` |
| `EnableCustomLevels` | 启用/禁用自定义等级 | **链上** `MemberProvider` |
| `AddUpgradeRule` | 添加升级规则 | ⛔ 创建时拒绝 |
| `RemoveUpgradeRule` | 删除升级规则 | ⛔ 创建时拒绝 |

### 团队业绩 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TeamPerformanceChange` | 团队业绩阶梯配置 | **链上** `TeamWriter` |
| `TeamPerformancePause` | 暂停团队业绩返佣 | 链下执行 |
| `TeamPerformanceResume` | 恢复团队业绩返佣 | 链下执行 |

### 披露管理 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `DisclosureLevelChange` | 披露级别 + 内幕交易管控 | **链上** `DisclosureProvider` |
| `DisclosureResetViolations` | 重置披露违规记录 | **链上** `DisclosureProvider` |

### DAO 可控紧急权限 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `EmergencyPauseToggle` | Owner 紧急暂停权限开关 | **链上** `EmergencyPauseEnabled` |
| `BatchCancelToggle` | Owner 批量取消权限开关 | **链上** `BatchCancelEnabled` |

### 社区 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommunityEvent` | 社区活动 | 仅记录 |
| `RuleSuggestion` | 规则建议 | 仅记录 |
| `General` | 通用提案 | 仅记录 |

---

## Storage

| 存储项 | Key | Value | Query |
|--------|-----|-------|-------|
| `NextProposalId` | — | `u64` | ValueQuery (0) |
| `Proposals` | `ProposalId` | `Proposal<T>` | Option |
| `EntityProposals` | `u64` (entity_id) | `BoundedVec<ProposalId, MaxActiveProposals>` | ValueQuery |
| `VoteRecords` | `(ProposalId, AccountId)` | `VoteRecord` | Option |
| `FirstHoldTime` | `(u64, AccountId)` | `BlockNumber` | Option |
| `VotingPowerSnapshot` | `(ProposalId, AccountId)` | `Balance` | Option |
| `GovernanceConfigs` | `u64` (entity_id) | `GovernanceConfig` | Option |
| `GovernanceLocked` | `u64` (entity_id) | `bool` | ValueQuery (false) |
| `VoterTokenLocks` | `(ProposalId, AccountId)` | `()` | Option |
| `GovernanceLockCount` | `(u64, AccountId)` | `u32` | ValueQuery (0) |
| `GovernanceLockAmount` | `(u64, AccountId)` | `Balance` | ValueQuery (0) |
| `GovernancePaused` | `u64` (entity_id) | `bool` | ValueQuery (false) |
| `EmergencyPauseEnabled` | `u64` (entity_id) | `bool` | Option (None = true) |
| `BatchCancelEnabled` | `u64` (entity_id) | `bool` | Option (None = true) |
| `VoteDelegation` | `(u64, AccountId)` | `AccountId` (delegate) | Option |
| `DelegatedVoters` | `(u64, AccountId)` | `BoundedVec<AccountId, MaxDelegatorsPerDelegate>` | ValueQuery |
| `ProposalScanCursor` | — | `u64` | ValueQuery (0) |

---

## Extrinsics

### `create_proposal` — call_index 0

创建治理提案。

```
签名: Signed(proposer)
参数: entity_id: u64, proposal_type: ProposalType, title: Vec<u8>, description_cid: Option<Vec<u8>>
```

**流程:**
1. Entity 存在 · 活跃 · 治理未暂停 · 模式 ≠ None
2. 提案参数校验（费率 ≤ 10000、百分比 ≤ 100、阶梯递增等）
3. 代币启用 · 持有 ≥ 总供应量 × proposal_threshold
4. 活跃提案 < MaxActiveProposals · 标题/CID 长度校验
5. 快照治理参数 + 总供应量 → 写入 Proposals + EntityProposals
6. 事件: `ProposalCreated { proposal_id, entity_id, proposer, title }`

### `vote` — call_index 1

对提案投票。

```
签名: Signed(voter)
参数: proposal_id: ProposalId, vote: VoteType
```

**流程:**
1. Voting 状态 · 投票期内 · Entity 活跃 · 治理未暂停
2. 未委托投票权 · 未重复投票 · TokenType 有投票权
3. 权重 = min(当前余额 + 委托权重, 快照余额) × 时间加权
4. reserve 代币（max-lock 引用计数）+ 锁定委托者代币
5. 事件: `Voted { proposal_id, voter, vote, weight }`

### `finalize_voting` — call_index 2

结束投票并计算结果。任何人可调用。

```
签名: Signed(anyone)
参数: proposal_id: ProposalId
前置: 投票期已结束 · Entity 活跃
```

法定人数 = `total_votes ≥ snapshot_total_supply × snapshot_quorum%`，通过率 = `yes > (yes + no) × snapshot_pass%`（弃权不计入）。

### `execute_proposal` — call_index 3

执行已通过提案。任何人可调用。

```
签名: Signed(anyone)
参数: proposal_id: ProposalId
前置: Passed 状态 · execution_time 已到 · 未超执行窗口 · Entity 活跃
```

超过执行窗口（2 × execution_delay）自动转为 Expired。

### `cancel_proposal` — call_index 4

取消提案（仅 Voting 状态）。

```
签名: Signed(proposer 或 owner)
参数: proposal_id: ProposalId
```

FullDAO 模式下 Owner 非提案者时返回 `GovernanceModeNotAllowed`（需走 veto 通道）。

### `configure_governance` — call_index 5

配置实体治理参数。

```
签名: Signed(owner)
参数: entity_id, mode, voting_period?, execution_delay?, quorum_threshold?, pass_threshold?, proposal_threshold?, admin_veto_enabled?
前置: 治理未锁定 · FullDAO 需代币已启用
```

参数下限: voting_period ≥ MinVotingPeriod, execution_delay ≥ MinExecutionDelay, quorum/pass ≥ 1。同步 registry 侧治理模式。

### `veto_proposal` — call_index 9

管理员否决提案（紧急制动）。

```
签名: Signed(owner)
参数: proposal_id: ProposalId
前置: admin_veto_enabled · 提案 Voting 或 Passed 状态
```

### `lock_governance` — call_index 10

永久锁定治理配置（不可逆）。

```
签名: Signed(owner)
参数: entity_id: u64
```

- **None 锁定** = 永久冻结，明确"永不启用 DAO"
- **FullDAO 锁定** = 放弃控制权，仅可通过提案修改治理参数
  - Owner 紧急暂停权限受 `EmergencyPauseEnabled` 控制（默认开启，DAO 可关闭）
  - Owner 批量取消权限受 `BatchCancelEnabled` 控制（默认开启，DAO 可关闭）
  - Owner 否决权受 `admin_veto_enabled` 控制（DAO 可通过 `AdminVetoToggle` 关闭）

### `cleanup_proposal` — call_index 11

清理终态提案，释放存储空间。

```
签名: Signed(anyone)
参数: proposal_id: ProposalId
前置: Executed / Failed / Cancelled / Expired
```

增量清理 VoteRecords / VotingPowerSnapshot / VoterTokenLocks（每次最多 500 条），全部清理完毕后删除 Proposal；否则保留供再次调用。

### `delegate_vote` — call_index 12

委托投票权（Compound 模型）。

```
签名: Signed(delegator)
参数: entity_id: u64, delegate: AccountId
前置: Entity 存在 · 活跃 · 代币启用 · 不可自我委托 · 无现有委托 · 委托目标未委托他人
```

### `undelegate_vote` — call_index 13

取消投票委托，恢复直接投票能力。

```
签名: Signed(delegator)
参数: entity_id: u64
前置: 有现有委托关系
```

### `change_vote` — call_index 14

修改已有投票（权重不变）。

```
签名: Signed(voter)
参数: proposal_id: ProposalId, new_vote: VoteType
前置: 有投票记录 · Voting 状态 · 投票期内 · Entity 活跃 · 治理未暂停
```

### `pause_governance` — call_index 15

紧急暂停治理（阻止新提案创建和投票）。

```
签名: Signed(owner)
参数: entity_id: u64
前置: 治理未暂停 · FullDAO 锁定后需 EmergencyPauseEnabled = true
```

### `resume_governance` — call_index 16

恢复治理。

```
签名: Signed(owner)
参数: entity_id: u64
前置: 治理已暂停
```

### `batch_cancel_proposals` — call_index 17

批量取消所有活跃提案 + 解锁投票者代币。

```
签名: Signed(owner)
参数: entity_id: u64
前置: FullDAO 锁定后需 BatchCancelEnabled = true
```

---

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ProposalCreated` | `proposal_id, entity_id, proposer, title` | 提案创建成功 |
| `Voted` | `proposal_id, voter, vote, weight` | 投票成功 |
| `VoteChanged` | `proposal_id, voter, old_vote, new_vote, weight` | 修改投票 |
| `ProposalPassed` | `proposal_id` | 投票通过 |
| `ProposalFailed` | `proposal_id` | 投票未通过 |
| `ProposalExecuted` | `proposal_id` | 提案执行成功 |
| `ProposalCancelled` | `proposal_id` | 提案被取消 |
| `ProposalVetoed` | `proposal_id, by` | 管理员否决 |
| `ProposalExpired` | `proposal_id` | 执行窗口超时 |
| `ProposalAutoFinalized` | `proposal_id, new_status` | `on_idle` 自动 finalize |
| `ProposalCleaned` | `proposal_id` | 终态提案存储已清理 |
| `ProposalExecutionNote` | `proposal_id, note` | 链下执行备注 |
| `GovernanceConfigUpdated` | `entity_id, mode` | 治理配置变更 |
| `GovernanceConfigLocked` | `entity_id` | 治理配置永久锁定 |
| `GovernanceSyncFailed` | `entity_id, mode` | Registry 同步失败 |
| `GovernancePausedEvent` | `entity_id` | 治理已暂停 |
| `GovernanceResumedEvent` | `entity_id` | 治理已恢复 |
| `BatchProposalsCancelled` | `entity_id, cancelled_count` | 批量取消提案 |
| `VoteDelegated` | `entity_id, delegator, delegate` | 投票权已委托 |
| `VoteUndelegated` | `entity_id, delegator` | 投票委托已撤销 |
| `ProposalPartialCleaned` | `proposal_id` | 终态提案部分清理（需再次调用） |

---

## Errors

| 错误 | 说明 |
|------|------|
| `ShopNotFound` | 实体/店铺不存在 |
| `NotShopOwner` | 不是实体所有者 |
| `EntityNotActive` | 实体未激活 |
| `TokenNotEnabled` | 代币未启用 |
| `TokenNotEnabledForDAO` | FullDAO 需要先发行代币 |
| `TokenTypeNoVotingPower` | 代币类型不具有投票权 |
| `ProposalNotFound` | 提案不存在 |
| `InsufficientTokensForProposal` | 代币不足以创建提案 |
| `TooManyActiveProposals` | 已达最大活跃提案数 |
| `InvalidProposalStatus` | 状态不允许此操作 |
| `AlreadyVoted` | 已投过票 |
| `NotVoted` | 未投过票（change_vote 时） |
| `NoVotingPower` | 没有投票权 |
| `VotingEnded` | 投票期已结束 |
| `VotingNotEnded` | 投票期未结束 |
| `ExecutionTimeNotReached` | 执行时间未到 |
| `TitleTooLong` | 标题超过 MaxTitleLength |
| `CidTooLong` | CID 超过 MaxCidLength |
| `CannotCancel` | 无权取消提案 |
| `GovernanceModeNotAllowed` | 治理模式不允许此操作（FullDAO 下 Owner 非提案者取消） |
| `NoVetoRight` | 无否决权（非 Owner 或未启用 admin_veto） |
| `InvalidParameter` | 参数无效（费率超范围等） |
| `ProposalTypeNotImplemented` | 提案类型暂未实现链上执行 |
| `ProposalTypeNotSupported` | 提案类型暂不支持创建 |
| `GovernanceConfigIsLocked` | 治理配置已锁定 |
| `GovernanceAlreadyLocked` | 治理配置已经锁定过 |
| `VotingPeriodTooShort` | 投票期低于 MinVotingPeriod |
| `ExecutionDelayTooShort` | 执行延迟低于 MinExecutionDelay |
| `QuorumTooLow` | 法定人数 < 1 |
| `PassThresholdTooLow` | 通过阈值 < 1 |
| `ProposalIdOverflow` | 提案 ID u64 溢出 |
| `ProposalNotTerminal` | 提案未处于终态，不可清理 |
| `GovernanceIsPaused` | 治理已暂停 |
| `GovernanceNotPaused` | 治理未暂停（resume 时） |
| `VotePowerDelegated` | 已委托投票权，不可直接投票 |
| `AlreadyDelegated` | 已有委托关系 |
| `DelegateAlreadyDelegated` | 委托目标自身已委托他人 |
| `NotDelegated` | 无委托关系 |
| `SelfDelegation` | 不可自我委托 |
| `TooManyDelegators` | 委托接收者已达 MaxDelegatorsPerDelegate |
| `EmergencyPauseDisabled` | DAO 已关闭 Owner 紧急暂停权限（FullDAO 锁定后） |
| `BatchCancelDisabled` | DAO 已关闭 Owner 批量取消权限（FullDAO 锁定后） |

---

## Runtime 配置

### Config Trait 依赖

| 类型 | 来源 | 用途 |
|------|------|------|
| `EntityProvider` | pallet-entity-common | 实体存在性、活跃状态、所有权、店铺列表查询 |
| `ShopProvider` | pallet-entity-common | 店铺暂停/恢复 |
| `TokenProvider` | pallet-entity-common | 代币余额、总供应量、启用状态、TokenType、reserve/unreserve |
| `CommissionProvider` | pallet-entity-commission | 返佣模式/费率/提现配置的链上写入 |
| `MemberProvider` | pallet-entity-commission | 自定义等级/升级模式的链上写入 |
| `ProductProvider` | pallet-entity-common | 商品价格/库存链上写入 |
| `MultiLevelWriter` | pallet-entity-commission | 多级分销阶梯配置链上写入 |
| `TeamWriter` | pallet-entity-commission | 团队业绩阶梯配置链上写入 |
| `DisclosureProvider` | pallet-entity-common | 披露级别/违规记录链上写入 |

### 常量参数

| 常量 | 类型 | 说明 |
|------|------|------|
| `VotingPeriod` | BlockNumber | 默认投票期（~7 天） |
| `ExecutionDelay` | BlockNumber | 默认执行延迟（~2 天） |
| `PassThreshold` | u8 | 通过阈值 %（50） |
| `QuorumThreshold` | u8 | 法定人数 %（10） |
| `MinProposalThreshold` | u16 | 提案门槛（基点，100 = 1%） |
| `MaxTitleLength` | u32 | 标题最大字节（128） |
| `MaxCidLength` | u32 | CID 最大字节（64） |
| `MaxActiveProposals` | u32 | 每实体最大活跃提案数（10） |
| `MaxDelegatorsPerDelegate` | u32 | 每委托接收者最大委托人数（50） |
| `MinVotingPeriod` | BlockNumber | 投票期下限（~1 天） |
| `MinExecutionDelay` | BlockNumber | 执行延迟下限（~12 小时） |
| `TimeWeightFullPeriod` | BlockNumber | 时间加权满额持有区块（0 = 禁用） |
| `TimeWeightMaxMultiplier` | u32 | 时间加权最大倍率（万分比，30000 = 3x） |

---

## 安全机制

| # | 机制 | 说明 |
|---|------|------|
| 1 | **闪电贷防护** | 首次投票快照余额到 VotingPowerSnapshot，后续取 min(当前, 快照) |
| 2 | **代币锁定** | 投票时 reserve 代币，max-lock + 引用计数，防止复投 |
| 3 | **时间加权** | 持有越久权重越大（最高 3x），抑制短期投机 |
| 4 | **提案门槛** | 持有 ≥ 1% 总供应量才能创建提案 |
| 5 | **法定人数** | 总投票 ≥ 10% 总供应量 |
| 6 | **执行延迟 + 过期窗口** | 通过后延迟执行，窗口 = 2 × delay，超时 → Expired |
| 7 | **参数快照** | 创建时快照 quorum/pass/delay/supply，投票期不可篡改 |
| 8 | **参数验证** | 费率 ≤ 10000，百分比 ≤ 100，阶梯严格递增等 |
| 9 | **活跃提案上限** | 每实体最多 10 个，防 DoS |
| 10 | **on_idle 自动清理** | 游标扫描超时提案，自动 finalize + expire |
| 11 | **增量清理** | cleanup_proposal 检查 clear_prefix cursor，未完全清理则保留供重试 |
| 12 | **委托链深度限制** | 委托目标不可再委托（防投票权黑洞） |
| 13 | **委托投票双重计票防护** | 代理投票时标记委托者 VoterTokenLocks，取消委托后仍阻止直投 |
| 14 | **DAO 可控紧急权限** | FullDAO 锁定后 Owner 紧急暂停/批量取消受 DAO 提案控制，实现渐进放权 |

---

## Hooks

### `on_idle`

每个空闲块扫描提案（从 `ProposalScanCursor` 游标位置开始，每批最多 10 个），自动处理：
- **Voting 超时** → finalize 逻辑（Passed / Failed）
- **Passed 执行窗口超时** → Expired

### `integrity_test`

Runtime 启动时校验配置参数：
- MinVotingPeriod > 0, MinExecutionDelay > 0
- QuorumThreshold ∈ [1, 100], PassThreshold ∈ [1, 100]
- MaxActiveProposals ≥ 1, MaxDelegatorsPerDelegate ≥ 1

---

## 依赖

```toml
[dependencies]
pallet-entity-common = { workspace = true }
pallet-entity-commission = { workspace = true }

[dev-dependencies]
sp-core = { workspace = true }
sp-io = { workspace = true }
pallet-balances = { workspace = true }
```

---

## 测试

```bash
cargo test -p pallet-entity-governance    # 180 tests
```

---

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.1.0 | 初始版本：5 extrinsics，22 种提案类型 |
| v0.2.0 | Phase 5：治理模式、管理员否决、快照防护、41 种提案类型 |
| v0.3.0 | 审计 R1-R2：通过阈值排除弃权、过期优雅转 Expired、VoteRecords 清理 |
| v0.4.0 | 审计 R3-R4：投票代币锁定、clear_prefix 有界、ShopPause 指定 shop_id、cleanup_proposal |
| v0.5.0 | 审计 R5：移除死代码/死错误码/死依赖、模块文档同步 |
| v0.6.0 | F1: 3 种治理参数提案 · R3: 拒绝未实现类型 · F5: 委托投票 · F6: 治理暂停 · F1: 改投 · 5 种新提案类型（团队业绩/披露管理）· 115 tests |
| v0.7.0 | 审计 R6：M1-R2 inactive entity 错误修正 · M2-R2 cleanup 增量清理 · L1-R2 防御性 VoterTokenLocks 清理 · 161 tests |
| v0.8.0 | 审计 R7：H1 修复委托投票双重计票漏洞 · M2 cleanup 区分完全/部分清理事件 · L1 DisclosureLevelChange 参数校验 · L3 GovernanceProvider 暴露 is_governance_paused · 168 tests |
| v0.9.0 | R8: DAO 可控紧急权限 — EmergencyPauseToggle/BatchCancelToggle 提案类型 · FullDAO 锁定后 pause/batch_cancel 受 DAO 控制 · 2 新存储项 · 2 新错误码 · 180 tests |

---

## 相关模块

| 模块 | 说明 |
|------|------|
| [pallet-entity-common](../common/README.md) | GovernanceMode · EntityProvider · ShopProvider · TokenProvider · ProductProvider · DisclosureProvider |
| [pallet-entity-commission](../commission/README.md) | CommissionProvider · MultiLevelWriter · TeamWriter · MemberProvider |
| [pallet-entity-token](../token/README.md) | 代币发行/余额/销毁 |
| [pallet-entity-member](../member/README.md) | 会员等级体系 |
