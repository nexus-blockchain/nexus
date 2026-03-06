# pallet-entity-governance

> 实体代币治理模块 — 多模式 DAO · 87 种提案 · 委托投票 · 闪电贷防护 · 紧急恢复

**version** `0.13.0` · **storage_version** `1` · **extrinsics** `16` · **tests** `213`

---

## 目录

- [功能概览](#功能概览)
- [治理模式](#治理模式)
- [治理流程](#治理流程)
- [数据结构](#数据结构)
- [提案类型（87 种）](#提案类型87-种)
- [Extrinsics（16 个）](#extrinsics16-个)
- [Storage（18 项）](#storage18-项)
- [Events（24 个）](#events24-个)
- [Errors（47 个）](#errors47-个)
- [Runtime 配置](#runtime-配置)
- [安全机制](#安全机制)
- [权重系统 WeightInfo](#权重系统-weightinfo)
- [Hooks](#hooks)
- [存储迁移](#存储迁移)
- [依赖](#依赖)
- [测试](#测试)
- [版本历史](#版本历史)
- [相关模块](#相关模块)

---

## 功能概览

| 功能 | 说明 |
|------|------|
| 多模式治理 | None（管理员全权）/ FullDAO（代币投票 + 可选管理员否决权） |
| 87 种提案类型 | 商品·店铺·代币·财务·治理参数·返佣·提现·会员等级·团队业绩·披露·紧急权限·社区·市场·单线·KYC·推荐人·积分 |
| 委托投票 | Compound 模型，委托后不可直投，代理人拥有委托者权重（含双重计票防护） |
| 投票修改 | 投票期内可改投（权重不变） |
| 闪电贷防护 | 投票权快照 + FirstHoldTime 时间门控 |
| 代币锁定 | reserve/unreserve + max-lock 引用计数，reserve 失败直接阻断投票 |
| 时间加权投票 | 持有越久权重越大（最高 3x） |
| 提案冷却期 | 同一用户在同一实体连续创建提案需间隔 N 个区块（可配置，0 = 禁用） |
| 治理暂停 | Owner 可紧急暂停治理 + 批量取消提案 |
| 治理锁定 | 永久不可逆锁定（None = 永不 DAO / FullDAO = 完全放权） |
| FullDAO 死锁恢复 | EmergencyOrigin（sudo/多签）可强制解锁治理，防止 DAO 死锁 |
| DAO 可控紧急权限 | FullDAO 锁定后 Owner 的 pause/batch_cancel 权限受 DAO 提案控制 |
| 自动过期 | `on_idle` hook 自动 finalize 超时提案 + expire 执行窗口 |
| 终态清理 | `cleanup_proposal` 释放存储（安全增量清理） |
| WeightInfo | 全部 16 个 extrinsic 使用参数化 weight（委托数量、提案数量因子） |
| GovernanceProvider | 对外提供 `governance_mode` / `has_active_proposals` / `is_governance_locked` / `is_governance_paused` 查询 |

### 设计约束

- **一实体一配置** — 治理配置绑定 Entity，1:N 多 Shop 共享同一套治理
- **参数快照** — 提案创建时快照 quorum/pass/execution_delay/total_supply，投票期间不可被篡改
- **通过阈值排除弃权** — `yes > (yes + no) × pass%`，弃权票不稀释通过率
- **C4 取消权限** — FullDAO 模式下仅提案者可取消，Owner 需走 veto 通道
- **DAO 可控紧急权限** — FullDAO 锁定后 Owner 的 pause/batch_cancel 权限默认开启，DAO 可通过提案关闭/重新开启
- **前置业务校验** — 提案创建时即校验 shop_id/product_id 归属，避免无效提案浪费投票周期
- **参数安全边界** — 治理参数提案强制上下限（MinVotingPeriod ≤ period ≤ MaxVotingPeriod 等）
- **执行失败优雅降级** — 提案执行 provider 返回错误时进入 ExecutionFailed 终态，不回滚
- **Reserve 强制阻断** — 投票时 reserve 失败直接返回 `TokenLockFailed`，确保代币锁定完整性

---

## 治理模式

| 模式 | 说明 | 提案创建 | 管理员否决 | 紧急恢复 |
|------|------|---------|----------|---------|
| **None** | 管理员全权控制 | ❌ 禁止 | — | — |
| **FullDAO** | 代币投票决策 | ✅ 持有 ≥ 门槛 | 可选（`admin_veto_enabled`） | EmergencyOrigin 可强制解锁 |

---

## 治理流程

```
代币持有者 create_proposal (持有 ≥ 1% 总供应量 + 冷却期检查)
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

**过期处理：** 投票超时 → `on_idle` 自动 finalize；执行窗口 = 2 × execution_delay，超时 → Expired

**死锁恢复：** FullDAO 无法达成法定人数 → EmergencyOrigin 调用 `force_unlock_governance` → Owner 恢复配置权限

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
    pub voter_count: u32,
    // 快照（防篡改）
    pub snapshot_quorum: u8,
    pub snapshot_pass: u8,
    pub snapshot_execution_delay: BlockNumberFor<T>,
    pub snapshot_total_supply: BalanceOf<T>,
}
```

### ProposalStatus

```
Voting ─→ Passed ─→ Executed
     │         ├─→ ExecutionFailed (链上执行失败)
     │         └─→ Expired (执行窗口超时)
     ├─→ Failed (法定人数或通过率不足)
     └─→ Cancelled (提案者取消 / Owner 否决)
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

---

## 提案类型（87 种）

### 商品管理 (4)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `PriceChange` | 商品价格调整 | 链上 `ProductProvider::update_price` |
| `ProductListing` | 新商品上架 | 链下 CID 解析 |
| `ProductDelisting` | 商品下架 | 链上 `ProductProvider::delist_product` |
| `InventoryAdjustment` | 库存调整 | 链上 `ProductProvider::set_inventory` |

### 店铺运营 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `Promotion` | 促销活动 | 事件记录 |
| `ShopNameChange` | 修改店铺名称 | 链下确认 |
| `ShopDescriptionChange` | 修改店铺描述 | 链下确认 |
| `ShopPause { shop_id }` | 暂停指定店铺 | 链上 `ShopProvider::pause_shop` |
| `ShopResume { shop_id }` | 恢复指定店铺 | 链上 `ShopProvider::resume_shop` |

### 代币经济 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TokenConfigChange` | 代币配置修改 | 事件记录 |
| `TokenMint` | 增发代币 | 链下执行 |
| `TokenBurn` | 销毁代币 | 链上 `TokenProvider::governance_burn` |
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
| `VotingPeriodChange` | 投票期调整 | 链上 GovernanceConfig |
| `QuorumChange` | 法定人数调整 | 链上 GovernanceConfig |
| `ProposalThresholdChange` | 提案门槛调整 | 链上 GovernanceConfig |
| `ExecutionDelayChange` | 执行延迟调整 | 链上 GovernanceConfig |
| `PassThresholdChange` | 通过阈值调整 | 链上 GovernanceConfig |
| `AdminVetoToggle` | 管理员否决权开关 | 链上 GovernanceConfig |

### 返佣配置 (7)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommissionModesChange` | 启用/禁用返佣模式 | 链上 `CommissionProvider` |
| `DirectRewardChange` | 直推奖励费率 | 链上 `CommissionProvider` |
| `MultiLevelChange` | 多级分销（内联 tiers 数据） | 链上 `MultiLevelWriter` |
| `LevelDiffChange` | 等级差价配置 | 链上 `CommissionProvider` |
| `FixedAmountChange` | 固定金额配置 | 链上 `CommissionProvider` |
| `FirstOrderChange` | 首单奖励配置 | 链上 `CommissionProvider` |
| `RepeatPurchaseChange` | 复购奖励配置 | 链上 `CommissionProvider` |

### 提现配置 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `WithdrawalConfigChange` | 分级提现配置 | 链上 `CommissionProvider` |
| `MinRepurchaseRateChange` | 全局最低复购比例底线 | 链上 `CommissionProvider` |

### 会员等级体系 (5)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `AddCustomLevel` | 添加自定义等级 | 链上 `MemberProvider` |
| `UpdateCustomLevel` | 更新自定义等级 | 链上 `MemberProvider` |
| `RemoveCustomLevel` | 删除自定义等级 | 链上 `MemberProvider` |
| `SetUpgradeMode` | 升级模式 (Auto/Manual/PeriodReset) | 链上 `MemberProvider` |
| `EnableCustomLevels` | 启用/禁用自定义等级 | 链上 `MemberProvider` |

### 团队业绩 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TeamPerformanceChange` | 团队业绩阶梯配置 | 链上 `TeamWriter` |
| `TeamPerformancePause` | 暂停团队业绩返佣 | 链下执行 |
| `TeamPerformanceResume` | 恢复团队业绩返佣 | 链下执行 |

### 披露管理 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `DisclosureLevelChange` | 披露级别 + 内幕交易管控 | 链上 `DisclosureProvider` |
| `DisclosureResetViolations` | 重置披露违规记录 | 链上 `DisclosureProvider` |

### DAO 可控紧急权限 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `EmergencyPauseToggle` | Owner 紧急暂停权限开关 | 链上 `EmergencyPauseEnabled` |
| `BatchCancelToggle` | Owner 批量取消权限开关 | 链上 `BatchCancelEnabled` |

### 社区 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommunityEvent` | 社区活动 | 仅记录 |
| `RuleSuggestion` | 规则建议 | 仅记录 |
| `General` | 通用提案 | 仅记录 |

### 市场管理 (7)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `MarketConfigChange` | 最小下单额 + 挂单有效期 | 链下 |
| `MarketPause` | 冻结交易 | 链下 |
| `MarketResume` | 恢复交易 | 链下 |
| `MarketClose` | 永久关闭（不可逆） | 链下 |
| `PriceProtectionChange` | 偏差/滑点/熔断/TWAP 参数 | 链下 |
| `MarketKycChange` | 市场 KYC 门槛 | 链下 |
| `CircuitBreakerLift` | 解除熔断 | 链下 |

### 单线收益 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `SingleLineConfigChange` | 上下线费率 + 层级配置 | 链下 |
| `SingleLinePause` | 暂停单线收益 | 链下 |
| `SingleLineResume` | 恢复单线收益 | 链下 |

### 代币扩展 (4) — v0.13 升级为链上

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `TokenMaxSupplyChange` | 代币最大供应量 | 链上 `TokenProvider::governance_set_max_supply` |
| `TokenTypeChange` | 代币类型 | 链上 `TokenProvider::governance_set_token_type` |
| `TransferRestrictionChange` | 转账限制模式 + KYC 门槛 | 链上 `TokenProvider::governance_set_transfer_restriction` |
| `TokenBlacklistManage` | 黑名单添加/移除 | 链下 |

### 返佣核心配置 (6) — 3 个 v0.13 升级为链上

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `CommissionRateChange` | 最大返佣比率 | 链上 `CommissionProvider::governance_set_commission_rate` |
| `CommissionToggle` | 返佣总开关 | 链上 `CommissionProvider::governance_toggle_commission` |
| `CreatorRewardRateChange` | 创建者分成比率 | 链上 `CommissionProvider::set_creator_reward_rate` |
| `WithdrawalCooldownChange` | NEX/代币提现冷却期 | 链下 |
| `TokenWithdrawalConfigChange` | 代币提现开关 | 链下 |
| `WithdrawalPauseToggle` | 提现暂停/恢复 | 链下 |

### 推荐人配置 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `ReferrerGuardChange` | 推荐人资格门槛 | 链下 |
| `CommissionCapChange` | 返佣上限（单笔+累计） | 链下 |
| `ReferralValidityChange` | 推荐有效期 | 链下 |

### 多级暂停 (2)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `MultiLevelPause` | 暂停多级分销 | 链下 |
| `MultiLevelResume` | 恢复多级分销 | 链下 |

### 会员管理 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `MemberPolicyChange` | 注册策略 (0-3) | 链下 |
| `UpgradeRuleToggle` | 升级规则系统开关 | 链下 |
| `MemberStatsPolicyChange` | 统计策略 | 链下 |

### KYC 管理 (3)

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `KycRequirementChange` | KYC 等级要求 + 强制 + 宽限期 | 链下 |
| `KycProviderAuthorize` | 授权 KYC 提供者 | 链下 |
| `KycProviderDeauthorize` | 取消 KYC 提供者授权 | 链下 |

### 店铺扩展 (5) — 2 个 v0.13 升级为链上

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `PointsConfigChange` | 积分配置 | 链下 |
| `PointsToggle` | 积分系统开关 | 链下 |
| `ShopPoliciesChange` | 店铺政策（退换货/物流） | 链下 |
| `ShopClose` | 店铺永久关闭（不可逆） | 链上 `ShopProvider::governance_close_shop` |
| `ShopTypeChange` | 店铺类型变更 | 链上 `ShopProvider::governance_set_shop_type` |

### 商品扩展 (1) — v0.13 升级为链上

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `ProductVisibilityChange` | 商品可见性 | 链上 `ProductProvider::governance_set_visibility` |

### 披露扩展 (2) — 1 个 v0.13 升级为链上

| 类型 | 说明 | 执行方式 |
|------|------|---------|
| `DisclosureInsiderManage` | 内部人添加/移除 | 链下 |
| `DisclosurePenaltyChange` | 处罚级别 | 链上 `DisclosureProvider::governance_set_penalty_level` |

### 执行方式统计

| 方式 | 数量 | 说明 |
|------|------|------|
| 链上执行 | 48 | 通过 Provider trait 直接写入链上状态 |
| 链下执行 | 36 | 发出 `ProposalExecutionNote` 事件，链下工作者响应 |
| 仅记录 | 3 | 社区类提案，不需要执行 |

---

## Extrinsics（16 个）

### `create_proposal` — call_index(0)

创建治理提案。

| 项目 | 内容 |
|------|------|
| 签名 | `Signed(proposer)` |
| 参数 | `entity_id: u64, proposal_type: ProposalType, title: Vec<u8>, description_cid: Option<Vec<u8>>` |
| Weight | `T::WeightInfo::create_proposal()` |

**流程：**
1. Entity 存在 · 活跃 · 治理未暂停 · 模式 ≠ None
2. 提案参数校验（费率 ≤ 10000、百分比 ≤ 100、阶梯递增等）
3. 代币启用 · 持有 ≥ 总供应量 × proposal_threshold
4. 活跃提案 < MaxActiveProposals · 标题/CID 长度校验
5. 冷却期检查 — 距上次创建 ≥ ProposalCooldown 区块
6. 快照治理参数 + 总供应量 → 写入 Proposals + EntityProposals
7. 记录 LastProposalCreatedAt

### `vote` — call_index(1)

对提案投票。

| 项目 | 内容 |
|------|------|
| 签名 | `Signed(voter)` |
| 参数 | `proposal_id: ProposalId, vote: VoteType` |
| Weight | `T::WeightInfo::vote(MaxDelegatorsPerDelegate)` |

**流程：**
1. Voting 状态 · 投票期内 · Entity 活跃 · 治理未暂停
2. 未委托投票权 · 未重复投票 · TokenType 有投票权
3. 权重 = min(当前余额 + 委托权重, 快照余额) × 时间加权
4. reserve 代币 — 失败直接返回 `TokenLockFailed`
5. 锁定委托者代币（同样 reserve 失败即阻断）

### `finalize_voting` — call_index(2)

结束投票并计算结果。任何人可调用。

法定人数 = `total_votes ≥ snapshot_total_supply × snapshot_quorum%`
通过率 = `yes > (yes + no) × snapshot_pass%`（弃权不计入）

### `execute_proposal` — call_index(3)

执行已通过提案。任何人可调用。

前置：Passed 状态 · `execution_time` 已到 · 未超执行窗口（2 × execution_delay）

### `cancel_proposal` — call_index(4)

取消提案（仅 Voting 状态）。FullDAO 模式下 Owner 非提案者时需走 veto 通道。

### `configure_governance` — call_index(5)

配置实体治理参数。前置：治理未锁定 · FullDAO 需代币已启用。

### `veto_proposal` — call_index(9)

管理员否决提案。前置：`admin_veto_enabled` · 提案 Voting 或 Passed 状态。

### `lock_governance` — call_index(10)

永久锁定治理配置（不可逆）。

- **None 锁定** = 永久冻结，"永不启用 DAO"
- **FullDAO 锁定** = 放弃控制权，仅可通过提案修改；Owner 紧急权限受 DAO 控制

### `cleanup_proposal` — call_index(11)

清理终态提案，释放存储。增量清理（每次最多 500 条），未完成保留供重试。

### `delegate_vote` — call_index(12)

委托投票权（Compound 模型）。不可自我委托 · 委托目标不可再委托。

### `undelegate_vote` — call_index(13)

取消投票委托，恢复直接投票能力。

### `change_vote` — call_index(14)

修改已有投票（权重不变）。前置：有投票记录 · Voting 状态 · 投票期内。

### `pause_governance` — call_index(15)

紧急暂停治理。FullDAO 锁定后需 `EmergencyPauseEnabled = true`。

### `resume_governance` — call_index(16)

恢复治理。前置：治理已暂停。

### `batch_cancel_proposals` — call_index(17)

批量取消所有活跃提案 + 解锁投票者代币。FullDAO 锁定后需 `BatchCancelEnabled = true`。

### `force_unlock_governance` — call_index(18)

FullDAO 死锁紧急恢复。仅 `EmergencyOrigin` 可调用。

同时执行：解除 `GovernanceLocked` + 解除 `GovernancePaused`。Owner 恢复 `configure_governance` 权限。

---

## Storage（18 项）

| 存储项 | 类型 | Key | Value | Query |
|--------|------|-----|-------|-------|
| `NextProposalId` | Value | — | `u64` | ValueQuery (0) |
| `Proposals` | Map | `ProposalId` | `Proposal<T>` | Option |
| `EntityProposals` | Map | `u64` | `BoundedVec<ProposalId, MaxActiveProposals>` | ValueQuery |
| `VoteRecords` | DoubleMap | `(ProposalId, AccountId)` | `VoteRecord` | Option |
| `FirstHoldTime` | DoubleMap | `(u64, AccountId)` | `BlockNumber` | Option |
| `VotingPowerSnapshot` | DoubleMap | `(ProposalId, AccountId)` | `Balance` | Option |
| `GovernanceConfigs` | Map | `u64` | `GovernanceConfig` | Option |
| `GovernanceLocked` | Map | `u64` | `bool` | ValueQuery (false) |
| `VoterTokenLocks` | DoubleMap | `(ProposalId, AccountId)` | `()` | Option |
| `GovernanceLockCount` | DoubleMap | `(u64, AccountId)` | `u32` | ValueQuery (0) |
| `GovernanceLockAmount` | DoubleMap | `(u64, AccountId)` | `Balance` | ValueQuery (0) |
| `ProposalScanCursor` | Value | — | `u64` | ValueQuery (0) |
| `GovernancePaused` | Map | `u64` | `bool` | ValueQuery (false) |
| `EmergencyPauseEnabled` | Map | `u64` | `bool` | Option (None = true) |
| `BatchCancelEnabled` | Map | `u64` | `bool` | Option (None = true) |
| `LastProposalCreatedAt` | DoubleMap | `(u64, AccountId)` | `BlockNumber` | Option |
| `VoteDelegation` | DoubleMap | `(u64, AccountId)` | `AccountId` | Option |
| `DelegatedVoters` | DoubleMap | `(u64, AccountId)` | `BoundedVec<AccountId, MaxDelegatorsPerDelegate>` | ValueQuery |

---

## Events（24 个）

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
| `ProposalPartialCleaned` | `proposal_id` | 部分清理（需再次调用） |
| `ProposalExecutionNote` | `proposal_id, note` | 链下执行备注 |
| `ProposalExecutionFailed` | `proposal_id` | 提案链上执行失败 |
| `GovernanceConfigUpdated` | `entity_id, mode` | 治理配置变更 |
| `GovernanceConfigLocked` | `entity_id` | 治理配置永久锁定 |
| `GovernanceSyncFailed` | `entity_id, mode` | Registry 同步失败 |
| `GovernancePausedEvent` | `entity_id` | 治理已暂停 |
| `GovernanceResumedEvent` | `entity_id` | 治理已恢复 |
| `GovernanceForceUnlocked` | `entity_id` | 紧急恢复：治理被强制解锁 |
| `GovernanceForceResumed` | `entity_id` | 紧急恢复：暂停被强制解除 |
| `BatchProposalsCancelled` | `entity_id, cancelled_count` | 批量取消提案 |
| `VoteDelegated` | `entity_id, delegator, delegate` | 投票权已委托 |
| `VoteUndelegated` | `entity_id, delegator` | 投票委托已撤销 |

---

## Errors（47 个）

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `NotEntityOwner` | 不是实体所有者 |
| `ShopNotFound` | 店铺不属于该实体 |
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
| `GovernanceModeNotAllowed` | 治理模式不允许此操作 |
| `NoVetoRight` | 无否决权 |
| `InvalidParameter` | 参数无效 |
| `ProposalTypeNotImplemented` | 提案类型暂未实现链上执行 |
| `ProposalTypeNotSupported` | 提案类型暂不支持创建 |
| `GovernanceConfigIsLocked` | 治理配置已锁定 |
| `GovernanceAlreadyLocked` | 治理配置已经锁定过 |
| `VotingPeriodTooShort` | 投票期低于 MinVotingPeriod |
| `VotingPeriodTooLong` | 投票期超过 MaxVotingPeriod |
| `ExecutionDelayTooShort` | 执行延迟低于 MinExecutionDelay |
| `ExecutionDelayTooLong` | 执行延迟超过 MaxExecutionDelay |
| `QuorumTooLow` | 法定人数 < 1 |
| `PassThresholdTooLow` | 通过阈值 < 1 |
| `ProposalIdOverflow` | 提案 ID u64 溢出 |
| `ProposalNotTerminal` | 提案未处于终态，不可清理 |
| `GovernanceIsPaused` | 治理已暂停 |
| `GovernanceNotPaused` | 治理未暂停 |
| `VotePowerDelegated` | 已委托投票权，不可直接投票 |
| `AlreadyDelegated` | 已有委托关系 |
| `DelegateAlreadyDelegated` | 委托目标自身已委托他人 |
| `NotDelegated` | 无委托关系 |
| `SelfDelegation` | 不可自我委托 |
| `TooManyDelegators` | 委托接收者已达上限 |
| `EmergencyPauseDisabled` | DAO 已关闭 Owner 紧急暂停权限 |
| `BatchCancelDisabled` | DAO 已关闭 Owner 批量取消权限 |
| `TokenLockFailed` | 代币锁定失败（reserve 不足） |
| `ProposalCooldownActive` | 提案创建冷却期未满 |

---

## Runtime 配置

### Config Trait — Provider 依赖

| 类型 | 来源 | 用途 |
|------|------|------|
| `EntityProvider` | pallet-entity-common | 实体查询：存在性、活跃状态、所有权、店铺列表、治理模式同步 |
| `ShopProvider` | pallet-entity-common | 店铺暂停/恢复/关闭/类型变更 |
| `TokenProvider` | pallet-entity-common | 代币余额/总供应量/reserve/unreserve/类型/最大供应量/转账限制 |
| `CommissionProvider` | pallet-entity-commission | 返佣模式/费率/提现/创建者分成/返佣开关 |
| `MemberProvider` | pallet-entity-commission | 自定义等级/升级模式 |
| `ProductProvider` | pallet-entity-common | 商品价格/库存/可见性 |
| `MultiLevelWriter` | pallet-entity-commission | 多级分销阶梯配置 |
| `TeamWriter` | pallet-entity-commission | 团队业绩阶梯配置 |
| `DisclosureProvider` | pallet-entity-common | 披露级别/违规记录/处罚级别 |
| `EmergencyOrigin` | frame_support | 紧急恢复 origin（通常配置为 EnsureRoot 或多签） |
| `WeightInfo` | 本模块 | 16 个 extrinsic 的权重函数 |

### 常量参数

| 常量 | 类型 | 说明 |
|------|------|------|
| `VotingPeriod` | BlockNumber | 默认投票期 |
| `ExecutionDelay` | BlockNumber | 默认执行延迟 |
| `PassThreshold` | u8 | 通过阈值 %（50） |
| `QuorumThreshold` | u8 | 法定人数 %（10） |
| `MinProposalThreshold` | u16 | 提案门槛（基点，100 = 1%） |
| `MaxTitleLength` | u32 | 标题最大字节 |
| `MaxCidLength` | u32 | CID 最大字节 |
| `MaxActiveProposals` | u32 | 每实体最大活跃提案数 |
| `MaxDelegatorsPerDelegate` | u32 | 每委托接收者最大委托人数 |
| `MinVotingPeriod` | BlockNumber | 投票期下限 |
| `MaxVotingPeriod` | BlockNumber | 投票期上限 |
| `MinExecutionDelay` | BlockNumber | 执行延迟下限 |
| `MaxExecutionDelay` | BlockNumber | 执行延迟上限 |
| `TimeWeightFullPeriod` | BlockNumber | 时间加权满额持有区块（0 = 禁用） |
| `TimeWeightMaxMultiplier` | u32 | 时间加权最大倍率（万分比，30000 = 3x） |
| `ProposalCooldown` | BlockNumber | 提案创建冷却期（0 = 禁用） |

---

## 安全机制

| # | 机制 | 说明 |
|---|------|------|
| 1 | **闪电贷防护** | 首次投票快照余额到 VotingPowerSnapshot，后续取 min(当前, 快照) |
| 2 | **代币锁定** | 投票时 reserve 代币，失败直接阻断投票（`TokenLockFailed`） |
| 3 | **时间加权** | 持有越久权重越大（最高 3x），抑制短期投机 |
| 4 | **提案门槛** | 持有 ≥ 1% 总供应量才能创建提案 |
| 5 | **提案冷却期** | 同一用户连续创建需间隔 N 区块，防垃圾提案 |
| 6 | **法定人数** | 总投票 ≥ 10% 总供应量 |
| 7 | **执行延迟 + 过期窗口** | 通过后延迟执行，窗口 = 2 × delay，超时 → Expired |
| 8 | **参数快照** | 创建时快照 quorum/pass/delay/supply，投票期不可篡改 |
| 9 | **参数验证** | 费率 ≤ 10000，百分比 ≤ 100，阶梯严格递增等 |
| 10 | **活跃提案上限** | 每实体最多 MaxActiveProposals 个，防 DoS |
| 11 | **on_idle 自动清理** | 游标扫描超时提案，自动 finalize + expire |
| 12 | **增量清理** | cleanup_proposal 每次最多 500 条，未完全清理则保留供重试 |
| 13 | **委托链深度限制** | 委托目标不可再委托（防投票权黑洞） |
| 14 | **双重计票防护** | 代理投票时标记 VoterTokenLocks，取消委托后阻止直投 |
| 15 | **DAO 可控紧急权限** | FullDAO 锁定后 Owner 紧急暂停/批量取消受 DAO 提案控制 |
| 16 | **前置业务校验** | 创建时校验 shop_id/product_id 归属，避免无效提案 |
| 17 | **参数安全边界** | 治理参数提案强制上下限 |
| 18 | **执行失败优雅降级** | 链上执行失败进入 ExecutionFailed 终态，不回滚 |
| 19 | **死锁紧急恢复** | EmergencyOrigin 可强制解锁 FullDAO 死锁（代币丢失/法定人数不可达） |

---

## 权重系统 WeightInfo

所有 16 个 extrinsic 通过 `T::WeightInfo` trait 获取动态权重，替代硬编码常量。

| 方法 | 参数 | 说明 |
|------|------|------|
| `create_proposal()` | — | 11 reads + 4 writes |
| `vote(d)` | `d` = 委托者数量 | 基础 10R+7W + per delegator 3R+4W |
| `finalize_voting()` | — | 6 reads + 5 writes |
| `execute_proposal()` | — | 6 reads + 4 writes |
| `cancel_proposal()` | — | 5 reads + 4 writes |
| `configure_governance()` | — | 4 reads + 2 writes |
| `lock_governance()` | — | 3 reads + 1 write |
| `cleanup_proposal()` | — | 2 reads + 502 writes (clear_prefix) |
| `delegate_vote()` | — | 6 reads + 2 writes |
| `undelegate_vote()` | — | 2 reads + 2 writes |
| `veto_proposal()` | — | 5 reads + 4 writes |
| `change_vote()` | — | 5 reads + 2 writes |
| `pause_governance()` | — | 5 reads + 1 write |
| `resume_governance()` | — | 3 reads + 1 write |
| `batch_cancel_proposals(p, v)` | `p` = 提案数, `v` = 总投票者数 | 基础 4R+2W + per proposal 2R+2W + per voter 3R+3W |
| `force_unlock_governance()` | — | 3 reads + 2 writes |

实现：`SubstrateWeight<T>` 基于 DB 访问路径分析，`()` 用于测试。

---

## Hooks

### `on_idle`

每个空闲块从 `ProposalScanCursor` 位置开始扫描（最多扫描 100 个 ID、处理 5 个提案），weight-bounded：

- **Voting 超时** → finalize（Passed / Failed）
- **Passed 执行窗口超时** → Expired

同时检查 `ref_time` 和 `proof_size`，确保不超出 remaining_weight。

### `integrity_test`

Runtime 启动时校验配置参数一致性：

- VotingPeriod ≥ MinVotingPeriod，ExecutionDelay ≥ MinExecutionDelay
- MaxVotingPeriod ≥ MinVotingPeriod，MaxExecutionDelay ≥ MinExecutionDelay
- QuorumThreshold ∈ [1, 100]，PassThreshold ∈ [1, 100]
- MinProposalThreshold ≤ 10000
- TimeWeightMaxMultiplier ≥ 10000 (1x)
- MaxActiveProposals > 0，MaxDelegatorsPerDelegate > 0

---

## 存储迁移

### v0 → v1 (`migration::MigrateV0ToV1<T>`)

| 项目 | 内容 |
|------|------|
| 触发条件 | on-chain STORAGE_VERSION == 0 |
| 操作 | 设置 STORAGE_VERSION = 1 |
| 数据迁移 | 无（新增 `LastProposalCreatedAt` 自动为空 map） |
| Weight | 1 read + 1 write |
| try-runtime | pre_upgrade 读取当前版本，post_upgrade 断言版本 = 1 |

**Runtime 集成示例：**

```rust
type Migrations = (
    pallet_entity_governance::migration::MigrateV0ToV1<Runtime>,
);
```

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
cargo test -p pallet-entity-governance    # 226 tests
```

测试覆盖：
- 提案完整生命周期（创建→投票→finalize→执行→清理）
- 全部 87 种提案类型的参数校验
- 委托投票 + 双重计票防护
- 时间加权投票
- on_idle 自动 finalize + expire
- 治理暂停/恢复/锁定
- DAO 可控紧急权限（EmergencyPauseToggle/BatchCancelToggle）
- Reserve 失败阻断投票
- 提案冷却期
- force_unlock_governance 紧急恢复 + 事件验证
- **深度审计修复验证**（BUG-1 ~ BUG-5, R-1/R-2）

---

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.1.0 | 初始版本：5 extrinsics，22 种提案类型 |
| v0.2.0 | Phase 5：治理模式、管理员否决、快照防护、41 种提案类型 |
| v0.3.0 | 审计 R1-R2：通过阈值排除弃权、过期优雅转 Expired |
| v0.4.0 | 审计 R3-R4：代币锁定、ShopPause 指定 shop_id、cleanup_proposal |
| v0.5.0 | 审计 R5：移除死代码/死错误码/死依赖 |
| v0.6.0 | F1-F6: 治理参数提案 · 委托投票 · 治理暂停 · 改投 · 团队/披露提案 · 115 tests |
| v0.7.0 | 审计 R6：inactive entity 修正 · cleanup 增量清理 · 161 tests |
| v0.8.0 | 审计 R7：委托双重计票修复 · cleanup 区分完全/部分清理 · 168 tests |
| v0.9.0 | R8: DAO 可控紧急权限 — EmergencyPauseToggle/BatchCancelToggle · 180 tests |
| v0.10.0 | R9 安全审计全面修复 — 双重计票(S1) · 参数边界(S2-S3) · 实体活跃检查(S4) · ExecutionFailed 优雅降级(F5) · 189 tests |
| v0.11.0 | R10 治理覆盖全面扩展 — 新增 39 种 ProposalType(48→87) · 200 tests |
| v0.12.0 | R11 二次审计修复 — 商品存在校验(S1) · 零值校验(S2-S4) · 费率范围(S5) · u64 溢出保护(S6) · emit_offchain_note 重构(R1) · 207 tests |
| v0.13.0 | **Phase 1-3 全面升级** — WeightInfo 体系(16 extrinsic 参数化 weight) · reserve 失败阻断投票 · 提案冷却期(ProposalCooldown) · FullDAO 死锁恢复(force_unlock_governance + EmergencyOrigin) · 11 个 R10 提案升级为链上执行 · STORAGE_VERSION 0→1 + migration · 213 tests |
| v0.14.0 | **深度审计修复** — on_idle 动态估权(BUG-1) · configure_governance 强制 MinProposalThreshold(BUG-2) · batch_cancel 清理 VotingPowerSnapshot(BUG-3) · GovernanceLockCount 消除双重读取(BUG-4) · MarketConfigChange/WithdrawalCooldownChange 参数校验(BUG-5) · finalize/cancel/veto 权重包含投票者解锁代价(BUG-6) · unlock_voters_for_proposal 共用 helper(R-1) · batch_cancel retain 简化(R-2) · 226 tests |

---

## 相关模块

| 模块 | 说明 |
|------|------|
| [pallet-entity-common](../common/README.md) | GovernanceMode · EntityProvider · ShopProvider · TokenProvider · ProductProvider · DisclosureProvider |
| [pallet-entity-commission](../commission/README.md) | CommissionProvider · MultiLevelWriter · TeamWriter · MemberProvider |
| [pallet-entity-token](../token/README.md) | 代币发行/余额/销毁/类型/最大供应量/转账限制 |
| [pallet-entity-member](../member/README.md) | 会员等级体系 |
