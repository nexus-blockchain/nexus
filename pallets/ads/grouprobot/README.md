# pallet-ads-grouprobot

> **GroupRobot 广告适配层 — TEE 投放验证 · 节点奖励 · 质押 audience_cap · 反作弊 · 三方收入分成**

## 概述

`pallet-ads-grouprobot` 是 GroupRobot 生态的广告适配层，为 `pallet-ads-core` 提供领域特定的投放验证、管理员映射和收入分配实现。本 pallet **不包含 Campaign CRUD 等核心广告逻辑**（由 ads-core 提供），仅实现 GroupRobot 专属的适配 Trait 和额外 extrinsic。

## 架构定位

```
pallet-ads-core (核心引擎)
    │
    ├── DeliveryVerifier       ← pallet-ads-grouprobot 实现 (TEE + 订阅 + 突增 + cap)
    ├── PlacementAdminProvider ← pallet-ads-grouprobot 实现 (CommunityAdmin / BotOwner)
    └── RevenueDistributor     ← pallet-ads-grouprobot 实现 (三方分成: 社区/节点/国库)
```

---

## 核心功能

### 1. TEE 投放验证 (DeliveryVerifier)

`verify_and_cap_audience` 的验证流程：

1. **订阅层级检查** — 通过 `SubscriptionProvider::effective_feature_gate` 检查 Bot 的 TEE 访问权限; 若层级允许禁用广告且无质押则拒绝
2. **TEE 节点验证** — 调用者必须是 TEE 节点运营者 (`NodeConsensusProvider::is_tee_node_by_operator`)
3. **突增暂停检查** — 若社区因 audience 突增被暂停 (`AudienceSurgePaused != 0`), 拒绝投放
4. **audience_cap 裁切** — 根据社区质押额对 audience_size 取 `min(audience_size, cap)`; cap=0 时不裁切

### 2. 社区质押 → audience_cap

质押额通过阶梯函数映射为 audience 上限：

| 质押额 (UNIT) | audience_cap |
|---------------|-------------|
| ≥ 1,000 | 10,000 |
| ≥ 100 | 5,000 |
| ≥ 10 | 1,000 |
| ≥ 1 | 200 |
| < 1 | 0 (无法投放) |

- 首个质押者自动成为社区管理员 (`CommunityAdmin`)
- 取消质押时自动清理零余额质押者; 总质押归零时清理管理员
- 管理员可由当前管理员或 Bot Owner 变更

### 3. 三方收入分成 (RevenueDistributor)

ads-core `settle_era_ads` 将广告费从广告主转入国库后, 调用 `distribute`:

| 接收方 | 默认比例 | 资金流向 |
|--------|----------|----------|
| 社区 | 80% | 返回给 ads-core 记入 `PlacementClaimable`, 管理员可提取 |
| 节点 | 15% | 国库 → `RewardPoolAccount`, 通过 `RewardAccruer::accrue_node_reward` 写入 |
| 国库 | 5% | 留在国库 (100% - 社区% - 节点%) |

- 分成百分比可通过 Root 治理调整 (`set_tee_ad_pct` / `set_community_ad_pct`)
- 校验: tee_pct + community_pct ≤ 100
- 被暂停的社区不参与收入分配 (`CommunityAdsPaused` 检查)

### 4. 反作弊机制

- **L3 — audience 突增检测**: `check_audience_surge` (Root) 对比当前 Era 与上一 Era 的 audience, 超过 `AudienceSurgeThresholdPct` 阈值时自动暂停社区广告投放; `resume_audience_surge` (Root) 恢复
- **L5 — 多节点交叉验证**: `report_node_audience` 收集多个节点的 audience 上报 (同一 node prefix 更新而非追加, 使用前 4 字节降低碰撞), `cross_validate_nodes` (Root) 检查 min/max 偏差是否超过 `NodeDeviationThresholdPct`

### 5. 广告位管理员映射 (PlacementAdminProvider)

管理员解析优先级: `CommunityAdmin` → `BotRegistry::bot_owner`

`is_placement_banned` 恒返回 `false` (ban 逻辑委托给 ads-core 的 `BannedPlacements`)。

### 6. Slash 机制

`slash_community` (Root) 按 `AdSlashPercentage` 计算 slash 金额, 从各质押者按比例 unreserve 并转入国库。转账失败时 re-reserve 回退并发射 `SlashTransferFailed` 事件。Slash 后自动更新总质押和 audience_cap。

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

---

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `CommunityAdStake` | `StorageMap<CommunityIdHash, Balance>` | 社区广告质押总额 |
| `CommunityAudienceCap` | `StorageMap<CommunityIdHash, u32>` | 社区 audience 上限 (由质押决定) |
| `CommunityStakers` | `StorageDoubleMap<CommunityIdHash, AccountId, Balance>` | 每个质押者在每个社区的质押额 |
| `CommunityAdmin` | `StorageMap<CommunityIdHash, AccountId>` | 社区管理员 (首个质押者自动成为) |
| `TeeNodeAdPct` | `StorageValue<u32>` | TEE 节点广告分成百分比 (默认 15%, 0=使用默认) |
| `CommunityAdPct` | `StorageValue<u32>` | 社区广告分成百分比 (默认 80%, 0=使用默认) |
| `PreviousEraAudience` | `StorageMap<CommunityIdHash, u32>` | 上一 Era 社区活跃人数 (L3 突增检测) |
| `AudienceSurgePaused` | `StorageMap<CommunityIdHash, u32>` | 社区因突增被暂停 (非零=暂停中) |
| `NodeAudienceReports` | `StorageMap<CommunityIdHash, BoundedVec<(u32, u32), 10>>` | 多节点 audience 上报 (node_prefix, audience_size), 上限 10 条 |

---

## Extrinsics

| call_index | 函数名 | 权限 | 说明 |
|------------|--------|------|------|
| 0 | `stake_for_ads` | Signed | 为社区质押以接入广告, 首个质押者成为管理员 |
| 1 | `unstake_for_ads` | Signed | 取消社区质押, 自动清理零余额 |
| 2 | `set_tee_ad_pct` | Root | 设置 TEE 节点广告分成百分比, 校验 tee+community ≤ 100 |
| 3 | `set_community_ad_pct` | Root | 设置社区广告分成百分比, 校验 tee+community ≤ 100 |
| 4 | `set_community_admin` | Signed (admin/bot_owner) | 设置社区管理员 |
| 5 | `report_node_audience` | Signed (node_operator) | 上报节点 audience (L5 交叉验证), 同 prefix 更新而非追加 |
| 6 | `check_audience_surge` | Root | 检测 audience 突增 (L3), 超阈值自动暂停 |
| 7 | `resume_audience_surge` | Root | 恢复因突增被暂停的社区广告投放 |
| 8 | `cross_validate_nodes` | Root | L5 多节点交叉验证, 偏差过大时发射 `NodeDeviationRejected` 事件; 始终返回 Ok 并清理报告 |
| 9 | `slash_community` | Root | Slash 社区质押, 按比例从各质押者扣除并转入国库 |

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
| `NodeDeviationTooHigh` | L5 多节点偏差过大 |
| `NodeReportsFull` | 节点 audience 报告已满 (上限 10) |
| `AdsDisabledByTier` | Bot 订阅层级允许禁用广告且无质押 |
| `TeeNotAvailableForTier` | Bot 订阅层级不支持 TEE 功能 |
| `NodeOperatorMismatch` | 调用者不是该 node_id 的运营者 |
| `CommunityNotPaused` | 社区未被暂停 (resume 前置检查) |

---

## 内部函数

| 函数 | 说明 |
|------|------|
| `compute_audience_cap(stake)` | 根据质押额计算 audience_cap (阶梯函数, 1 UNIT = 1_000_000_000_000) |
| `percent_of(amount, pct)` | 百分比计算: `amount × pct / 100` |
| `validate_node_reports(community_id_hash)` | 验证多节点报告偏差 (L5), 返回 `Err((min, max))` 表示偏差过大 |
| `effective_community_pct()` | 获取社区分成百分比 (存储值为 0 时返回默认 80) |
| `effective_tee_pct()` | 获取 TEE 分成百分比 (存储值为 0 时返回默认 15) |

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

## 许可证

Apache-2.0
