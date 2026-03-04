# pallet-ads-grouprobot

> **GroupRobot 广告适配层 — TEE 投放验证 · 节点奖励 · 质押 audience_cap · 反作弊 · 三方收入分成 · 质押者分成 · 多级暂停 · 解锁冷却**

## 概述

`pallet-ads-grouprobot` 是 GroupRobot 生态的广告适配层，为 `pallet-ads-core` 提供领域特定的投放验证、管理员映射和收入分配实现。本 pallet **不包含 Campaign CRUD 等核心广告逻辑**（由 ads-core 提供），仅实现 GroupRobot 专属的适配 Trait 和额外 extrinsic。

## 架构定位

```
pallet-ads-core (核心引擎)
    │
    ├── DeliveryVerifier       ← pallet-ads-grouprobot 实现 (全局暂停 + Bot禁用 + 管理员暂停 + TEE + 订阅 + 突增 + cap)
    ├── PlacementAdminProvider ← pallet-ads-grouprobot 实现 (CommunityAdmin / BotOwner)
    └── RevenueDistributor     ← pallet-ads-grouprobot 实现 (四方分成: 社区/节点/质押者/国库)
```

---

## 核心功能

### 1. TEE 投放验证 (DeliveryVerifier)

`verify_and_cap_audience` 的验证流程：

1. **全局暂停检查** — `GlobalAdsPaused` 为 true 时拒绝所有投放
2. **Bot Owner 禁用检查** — `BotAdsDisabled` 为 true 时拒绝该社区投放
3. **管理员暂停检查** — `AdminPausedAds` 为 true 时拒绝该社区投放
4. **订阅层级检查** — 通过 `SubscriptionProvider::effective_feature_gate` 检查 Bot 的 TEE 访问权限; 若层级允许禁用广告且无质押则拒绝
5. **TEE 节点验证** — 调用者必须是 TEE 节点运营者 (`NodeConsensusProvider::is_tee_node_by_operator`)
6. **突增暂停检查** — 若社区因 audience 突增被暂停 (`AudienceSurgePaused != 0`), 拒绝投放
7. **audience_cap 裁切** — 根据社区质押额对 audience_size 取 `min(audience_size, cap)`; cap=0 时不裁切

### 2. 社区质押 → audience_cap

质押额通过阶梯函数映射为 audience 上限 (默认硬编码, 可通过 `set_stake_tiers` 自定义):

| 质押额 (UNIT) | audience_cap |
|---------------|-------------|
| ≥ 1,000 | 10,000 |
| ≥ 100 | 5,000 |
| ≥ 10 | 1,000 |
| ≥ 1 | 200 |
| < 1 | 0 (无法投放) |

- 首个质押者自动成为社区管理员 (`CommunityAdmin`)
- 取消质押进入 **解锁冷却期** (`UnbondingPeriod`), 期满后通过 `withdraw_unbonded` 提取
- 管理员可由当前管理员或 Bot Owner 变更; 管理员可辞职 (`resign_community_admin`), 回退到 Bot Owner

### 3. 四方收入分成 (RevenueDistributor)

ads-core `settle_era_ads` 将广告费从广告主转入国库后, 调用 `distribute`:

| 接收方 | 默认比例 | 资金流向 |
|--------|----------|----------|
| 社区 | 80% × (1 - StakerRewardPct%) | 返回给 ads-core 记入 `PlacementClaimable`, 管理员可提取 |
| 质押者 | 80% × StakerRewardPct% | 按质押比例分配到 `StakerClaimable`, 质押者通过 `claim_staker_reward` 提取 |
| 节点 | 15% | 国库 → `RewardPoolAccount`, 通过 `RewardAccruer::accrue_node_reward` 写入 |
| 国库 | 5% | 留在国库 (100% - 社区% - 节点%) |

- 分成百分比可通过 Root 治理调整 (`set_tee_ad_pct` / `set_community_ad_pct`), 接受 `Option<u32>` — `None` 恢复默认, `Some(v)` 设置具体值
- 校验: tee_pct + community_pct ≤ 100
- 全局暂停 / Bot 禁用 / 管理员暂停 / 突增暂停的社区不参与收入分配

### 4. 多级暂停机制

| 暂停类型 | 控制者 | 影响范围 | Extrinsic |
|----------|--------|----------|-----------|
| 全局暂停 | Root | 所有社区 | `set_global_ads_pause` |
| Bot Owner 禁用 | Bot Owner | 单个社区 | `set_bot_ads_enabled` |
| 管理员暂停 | Admin / Bot Owner | 单个社区 | `admin_pause_ads` / `admin_resume_ads` |
| 突增暂停 | Root (自动检测) | 单个社区 | `check_audience_surge` / `resume_audience_surge` |

### 5. 反作弊机制

- **L3 — audience 突增检测**: `check_audience_surge` (Root) 对比当前 Era 与上一 Era 的 audience, 超过 `AudienceSurgeThresholdPct` 阈值时自动暂停社区广告投放
- **L5 — 多节点交叉验证**: `report_node_audience` 收集多个节点的 audience 上报 (同一 node prefix 更新而非追加), `cross_validate_nodes` (Root) 检查偏差

### 6. 广告位管理员映射 (PlacementAdminProvider)

管理员解析优先级: `CommunityAdmin` → `BotRegistry::bot_owner`

### 7. Slash 机制

`slash_community` (Root) 按 `AdSlashPercentage` 计算 slash 金额, 从各质押者按比例 unreserve 并转入国库。Slash 后自动更新总质押、audience_cap 和 `CommunitySlashCount`。

---

## Config

### 适配层依赖

| Config 类型 | Trait | 说明 |
|-------------|-------|------|
| `Currency` | `ReservableCurrency` | 质押锁定/释放 |
| `NodeConsensus` | `NodeConsensusProvider` | 节点状态查询 (活跃/TEE/运营者) |
| `Subscription` | `SubscriptionProvider` | Bot 订阅层级门控 |
| `RewardPool` | `RewardAccruer` | 统一奖励写入 |
| `BotRegistry` | `BotRegistryProvider` | Bot Owner 查询 |
| `TreasuryAccount` | `Get<AccountId>` | 国库账户 |
| `RewardPoolAccount` | `Get<AccountId>` | 奖励池账户 (节点份额转入) |

### 常量

| 常量 | 类型 | 说明 |
|------|------|------|
| `AudienceSurgeThresholdPct` | `u32` | L3 突增阈值百分比 (e.g. 100 = 允许 100% 增长) |
| `NodeDeviationThresholdPct` | `u32` | L5 多节点偏差阈值百分比 (e.g. 20 = 20%) |
| `AdSlashPercentage` | `u32` | Slash 百分比 (e.g. 30 = 30%) |
| `UnbondingPeriod` | `BlockNumber` | 取消质押后的解锁冷却区块数 (e.g. 14400 ≈ 24h) |
| `StakerRewardPct` | `u32` | 社区份额中分给质押者的百分比 (e.g. 10 = 10%) |

---

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CommunityAdStake` | `StorageMap<CommunityIdHash, Balance>` | 社区广告质押总额 |
| `CommunityAudienceCap` | `StorageMap<CommunityIdHash, u32>` | 社区 audience 上限 (由质押决定) |
| `CommunityStakers` | `StorageDoubleMap<CommunityIdHash, AccountId, Balance>` | 每个质押者在每个社区的质押额 |
| `CommunityAdmin` | `StorageMap<CommunityIdHash, AccountId>` | 社区管理员 (首个质押者自动成为) |
| `TeeNodeAdPct` | `StorageValue<Option<u32>>` | TEE 节点广告分成百分比 (None=默认15%) |
| `CommunityAdPct` | `StorageValue<Option<u32>>` | 社区广告分成百分比 (None=默认80%) |
| `PreviousEraAudience` | `StorageMap<CommunityIdHash, u32>` | 上一 Era 社区活跃人数 (L3 突增检测) |
| `AudienceSurgePaused` | `StorageMap<CommunityIdHash, u32>` | 社区因突增被暂停 (非零=暂停中) |
| `NodeAudienceReports` | `StorageMap<CommunityIdHash, BoundedVec<(u32, u32), 10>>` | 多节点 audience 上报 |
| `UnbondingRequests` | `StorageDoubleMap<CommunityIdHash, AccountId, (Balance, BlockNumber)>` | 解锁冷却中的请求 (金额, 到期区块) |
| `AdminPausedAds` | `StorageMap<CommunityIdHash, bool>` | 管理员暂停标志 |
| `StakeTiers` | `StorageValue<Option<BoundedVec<(u128, u32), 10>>>` | 可配置质押阶梯 (threshold 降序) |
| `GlobalAdsPaused` | `StorageValue<bool>` | 全局广告暂停开关 |
| `CommunitySlashCount` | `StorageMap<CommunityIdHash, u32>` | 社区累计 Slash 次数 |
| `BotAdsDisabled` | `StorageMap<CommunityIdHash, bool>` | Bot Owner 禁用广告标志 |
| `StakerClaimable` | `StorageDoubleMap<CommunityIdHash, AccountId, Balance>` | 质押者可提取的广告分成 |

---

## Extrinsics

| call_index | 函数名 | 权限 | 说明 |
|------------|--------|------|------|
| 0 | `stake_for_ads` | Signed | 为社区质押以接入广告, 首个质押者成为管理员 |
| 1 | `unstake_for_ads` | Signed | 取消社区质押, 进入解锁冷却期 |
| 2 | `set_tee_ad_pct` | Root | 设置 TEE 分成 `Option<u32>` (None=默认15%), 校验 tee+community ≤ 100 |
| 3 | `set_community_ad_pct` | Root | 设置社区分成 `Option<u32>` (None=默认80%), 校验 tee+community ≤ 100 |
| 4 | `set_community_admin` | Signed (admin/bot_owner) | 设置社区管理员 |
| 5 | `report_node_audience` | Signed (node_operator) | 上报节点 audience, 同 prefix 更新, 发射 `NodeAudienceReported` 事件 |
| 6 | `check_audience_surge` | Root | 检测 audience 突增 (L3), 超阈值自动暂停 |
| 7 | `resume_audience_surge` | Root | 恢复因突增被暂停的社区广告投放 |
| 8 | `cross_validate_nodes` | Root | L5 多节点交叉验证, 偏差过大时发射 `NodeDeviationRejected`; 始终 Ok 并清理报告 |
| 9 | `slash_community` | Root | Slash 社区质押, 按比例扣除并转入国库, 累加 `CommunitySlashCount` |
| 10 | `admin_pause_ads` | Signed (admin/bot_owner) | 管理员暂停社区广告 |
| 11 | `admin_resume_ads` | Signed (admin/bot_owner) | 管理员恢复社区广告 |
| 12 | `resign_community_admin` | Signed (current admin) | 管理员辞职, 回退到 Bot Owner (无则清空) |
| 13 | `withdraw_unbonded` | Signed | 提取解锁冷却到期的质押 |
| 14 | `set_stake_tiers` | Root | 设置自定义质押阶梯 (threshold 降序, 最多 10 级) |
| 15 | `force_set_community_admin` | Root | 强制设置社区管理员 |
| 16 | `set_global_ads_pause` | Root | 全局广告暂停开关 |
| 17 | `set_bot_ads_enabled` | Signed (bot_owner) | Bot Owner 禁用/启用社区广告 |
| 18 | `claim_staker_reward` | Signed | 质押者提取广告分成 |
| 19 | `force_unstake` | Root | 强制解除社区全部质押, 清理所有关联存储 |

---

## Events

| 事件 | 字段 | 触发条件 |
|------|------|----------|
| `AdStaked` | community_id_hash, amount, audience_cap | 质押成功 |
| `AdUnstaked` | community_id_hash, amount | 取消质押 |
| `CommunitySlashed` | community_id_hash, slashed_amount, slash_count | 社区被 Slash |
| `TeeAdPercentUpdated` | tee_pct | TEE 分成百分比更新 |
| `CommunityAdPercentUpdated` | community_pct | 社区分成百分比更新 |
| `CommunityAdminUpdated` | community_id_hash, new_admin | 社区管理员变更 |
| `AudienceSurgePausedEvent` | community_id_hash, previous, current | L3 突增暂停 |
| `AudienceSurgeResumed` | community_id_hash | 突增暂停恢复 |
| `NodeDeviationRejected` | community_id_hash, min_audience, max_audience | L5 偏差过大 |
| `NodeAdRewardAccrued` | node_id, amount | 节点广告奖励分配 |
| `SlashTransferFailed` | community_id_hash, staker, amount | Slash 转账失败 (资金已回退 reserve) |
| `UnbondingStarted` | community_id_hash, who, amount, unlock_at | 解锁冷却开始 |
| `UnbondedWithdrawn` | community_id_hash, who, amount | 解锁到期提取 |
| `AdsPausedByAdmin` | community_id_hash | 管理员暂停广告 |
| `AdsResumedByAdmin` | community_id_hash | 管理员恢复广告 |
| `StakeTiersUpdated` | tier_count | 质押阶梯更新 |
| `CommunityAdminResigned` | community_id_hash, resigned | 管理员辞职 |
| `GlobalAdsPauseToggled` | paused | 全局暂停切换 |
| `BotAdsToggled` | community_id_hash, disabled | Bot 广告开关 |
| `StakerRewardClaimed` | community_id_hash, who, amount | 质押者提取分成 |
| `ForceAdminSet` | community_id_hash, new_admin | Root 强制设置管理员 |
| `ForceUnstaked` | community_id_hash, total_amount, staker_count | Root 强制解除全部质押 |
| `NodeAudienceReported` | community_id_hash, node_id, audience_size | 节点 audience 上报 |

---

## Errors

| 错误码 | 说明 |
|--------|------|
| `ZeroStakeAmount` | 质押金额为零 |
| `InsufficientStake` | 质押不足 (unstake 超出已质押 / slash 时总质押为零) |
| `NotCommunityAdmin` | 非社区管理员 (也非 Bot Owner) |
| `InvalidPercentage` | 无效的百分比值 (>100 或 tee+community>100) |
| `NodeNotActive` | 节点不存在或未激活 |
| `NodeNotTee` | 节点非 TEE (调用者不是 TEE 节点运营者) |
| `CommunityAdsPaused` | 社区因突增被暂停广告 |
| `NodeReportsFull` | 节点 audience 报告已满 (上限 10) |
| `AdsDisabledByTier` | Bot 订阅层级允许禁用广告且无质押 |
| `TeeNotAvailableForTier` | Bot 订阅层级不支持 TEE 功能 |
| `NodeOperatorMismatch` | 调用者不是该 node_id 的运营者 |
| `CommunityNotPaused` | 社区未被暂停 (resume 前置检查) |
| `NothingToWithdraw` | 无待提取的解锁质押 |
| `UnbondingNotReady` | 解锁冷却期未到 |
| `AdsPausedByAdmin` | 社区已被管理员暂停 (重复暂停检查) |
| `AdsNotPausedByAdmin` | 社区未被管理员暂停 (resume 前置检查) |
| `InvalidStakeTiers` | 无效的质押阶梯 (空或未降序) |
| `GlobalAdsPausedErr` | 全局广告已暂停 |
| `BotAdsDisabledErr` | Bot 广告已被 Owner 禁用 |
| `NoClaimableReward` | 无可提取的质押者分成 |
| `NoStakeInCommunity` | 社区无质押 (force_unstake 前置检查) |
| `UnbondingAlreadyPending` | 已有待处理的解锁请求 |
| `NotBotOwner` | 调用者非 Bot Owner |

---

## 内部函数

| 函数 | 说明 |
|------|------|
| `compute_audience_cap(stake)` | 根据质押额计算 audience_cap (优先使用 `StakeTiers`, 否则硬编码阶梯) |
| `percent_of(amount, pct)` | 百分比计算: `amount × pct / 100` |
| `validate_node_reports(community_id_hash)` | 验证多节点报告偏差 (L5), 返回 `Err((min, max))` 表示偏差过大 |
| `effective_community_pct()` | 获取社区分成百分比 (None 时返回默认 80) |
| `effective_tee_pct()` | 获取 TEE 分成百分比 (None 时返回默认 15) |
| `ensure_admin_or_bot_owner(who, community_id_hash)` | 验证调用者是社区管理员或 Bot Owner |

---

## 依赖关系

```
pallet-ads-grouprobot (本 crate)
├── pallet-ads-primitives (共享类型 + Trait)
├── pallet-grouprobot-primitives (GroupRobot 共享类型 + Trait)
│   └── NodeId, CommunityIdHash, NodeConsensusProvider,
│       SubscriptionProvider, RewardAccruer, BotRegistryProvider
├── frame-support / frame-system
├── sp-runtime
└── log
```

## 版本历史

### v0.2.0

- **新增**: 解锁冷却期 (`UnbondingPeriod`) — unstake 后需等待冷却期才可 withdraw
- **新增**: 质押者广告分成 (`StakerRewardPct`) — distribute 时按质押比例分配给质押者
- **新增**: 多级暂停 — 全局暂停 / Bot Owner 禁用 / 管理员暂停
- **新增**: 可配置质押阶梯 (`set_stake_tiers`)
- **新增**: 管理员辞职 (`resign_community_admin`)
- **新增**: Root 强制操作 (`force_set_community_admin`, `force_unstake`)
- **新增**: 质押者提取分成 (`claim_staker_reward`)
- **新增**: Slash 历史追踪 (`CommunitySlashCount`)
- **变更**: `set_tee_ad_pct` / `set_community_ad_pct` 接受 `Option<u32>` (None=恢复默认)
- **变更**: `TeeNodeAdPct` / `CommunityAdPct` 改为 `OptionQuery`
- **变更**: `report_node_audience` 现在发射 `NodeAudienceReported` 事件
- **移除**: 死代码 `NodeDeviationTooHigh` 错误

## 许可证

Apache-2.0
