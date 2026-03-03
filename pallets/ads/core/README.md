# pallet-ads-core

> **通用广告引擎 — Campaign CRUD · Escrow 预算托管 · 投放收据 · Era 结算 · 双向偏好 · Slash/惩罚 · 收入提取 · 私域广告**

## 概述

`pallet-ads-core` 是广告模块组的核心引擎，提供完整的广告活动生命周期管理。本 pallet **不包含任何领域特定逻辑**（TEE 节点、Bot 订阅、Entity/Shop 等），通过 Config trait 中的适配层 trait 注入领域行为，实现跨业务场景复用。

## 架构设计

```
┌─────────────────────────────────────────────────┐
│               pallet-ads-core                    │
│  Campaign CRUD / Escrow / 投放 / 结算 / 偏好     │
├───────────────┬───────────────┬──────────────────┤
│DeliveryVerifier│PlacementAdmin │RevenueDistributor│  ← Config trait 注入
└───────────────┴───────────────┴──────────────────┘
       ▲                ▲                ▲
       │                │                │
  GroupRobot 适配    Entity 适配      其他适配层...
```

核心引擎通过三个关键 trait 解耦领域行为：
- **`DeliveryVerifier`** — 投放验证（TEE 签名 / Entity 活跃检查）
- **`PlacementAdminProvider`** — 广告位管理员映射（Bot Owner / Entity Owner）
- **`RevenueDistributor`** — 收入分配策略（三方分成 / 二方分成）

---

## 数据结构

### AdCampaign

广告活动核心结构：

| 字段 | 类型 | 说明 |
|------|------|------|
| `advertiser` | `AccountId` | 广告主账户 |
| `text` | `BoundedVec<u8, MaxAdTextLength>` | 广告文本 (≤280 字节) |
| `url` | `BoundedVec<u8, MaxAdUrlLength>` | 链接 URL (≤256 字节) |
| `bid_per_mille` | `Balance` | 每千人触达出价 (CPM) |
| `daily_budget` | `Balance` | 每日预算上限 |
| `total_budget` | `Balance` | 总预算 |
| `spent` | `Balance` | 已花费 |
| `delivery_types` | `u8` | 投放类型 bitmask (1~7) |
| `status` | `CampaignStatus` | 活动状态 |
| `review_status` | `AdReviewStatus` | 审核状态 |
| `total_deliveries` | `u64` | 累计投放次数 |
| `created_at` | `BlockNumber` | 创建区块 |
| `expires_at` | `BlockNumber` | 过期区块 |

### DeliveryReceipt

投放收据：

| 字段 | 类型 | 说明 |
|------|------|------|
| `campaign_id` | `u64` | 关联的 Campaign ID |
| `placement_id` | `PlacementId` | 广告位 |
| `audience_size` | `u32` | 受众规模 (已裁切) |
| `cpm_multiplier_bps` | `u32` | CPM 倍率 (基点) |
| `delivered_at` | `BlockNumber` | 投放区块 |
| `settled` | `bool` | 是否已结算 |
| `submitter` | `AccountId` | 提交者 |

---

## Config

### 常量

| 常量 | 类型 | 说明 |
|------|------|------|
| `MaxAdTextLength` | `u32` | 广告文本最大长度 |
| `MaxAdUrlLength` | `u32` | 广告 URL 最大长度 |
| `MaxReceiptsPerPlacement` | `u32` | 每广告位最大收据数 |
| `MaxAdvertiserBlacklist` | `u32` | 广告主黑名单上限 |
| `MaxAdvertiserWhitelist` | `u32` | 广告主白名单上限 |
| `MaxPlacementBlacklist` | `u32` | 广告位黑名单上限 |
| `MaxPlacementWhitelist` | `u32` | 广告位白名单上限 |
| `MinBidPerMille` | `Balance` | 最低 CPM 出价 |
| `MinAudienceSize` | `u32` | 接入广告的最低受众人数 |
| `AdSlashPercentage` | `u32` | Slash 百分比 (e.g. 30 = 30%) |
| `PrivateAdRegistrationFee` | `Balance` | 私有广告注册费用 |

### 适配层 Trait

| 类型 | Trait | 说明 |
|------|-------|------|
| `Currency` | `ReservableCurrency` | 预算锁定/释放 |
| `DeliveryVerifier` | `DeliveryVerifier<AccountId>` | 投放验证 + 受众裁切 |
| `PlacementAdmin` | `PlacementAdminProvider<AccountId>` | 广告位管理员查询 |
| `RevenueDistributor` | `RevenueDistributor<AccountId, Balance>` | 收入分配策略 |
| `TreasuryAccount` | `Get<AccountId>` | 国库账户 |

---

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextCampaignId` | `StorageValue<u64>` | 下一个 Campaign ID |
| `Campaigns` | `StorageMap<u64, AdCampaign>` | 广告活动 |
| `CampaignEscrow` | `StorageMap<u64, Balance>` | Campaign 锁定预算 (escrow) |
| `DeliveryReceipts` | `StorageMap<PlacementId, BoundedVec<DeliveryReceipt>>` | 投放收据 (per placement, 每 Era 结算后清空) |
| `EraAdRevenue` | `StorageMap<PlacementId, Balance>` | 每 Era 广告位广告收入 |
| `PlacementTotalRevenue` | `StorageMap<PlacementId, Balance>` | 广告位累计广告总收入 |
| `PlacementClaimable` | `StorageMap<PlacementId, Balance>` | 广告位待提取收入 (claimable) |
| `PlacementEraDeliveries` | `StorageMap<PlacementId, u32>` | 广告位本 Era 投放次数 |
| `PrivateAdCount` | `StorageMap<PlacementId, u32>` | 私有广告注册次数 |
| `AdvertiserBlacklist` | `StorageMap<AccountId, BoundedVec<PlacementId>>` | 广告主拉黑的广告位列表 |
| `AdvertiserWhitelist` | `StorageMap<AccountId, BoundedVec<PlacementId>>` | 广告主指定 (白名单) 的广告位列表 |
| `PlacementBlacklist` | `StorageMap<PlacementId, BoundedVec<AccountId>>` | 广告位拉黑的广告主列表 |
| `PlacementWhitelist` | `StorageMap<PlacementId, BoundedVec<AccountId>>` | 广告位指定 (白名单) 的广告主列表 |
| `SlashCount` | `StorageMap<PlacementId, u32>` | 连续 Slash 次数 |
| `BannedPlacements` | `StorageMap<PlacementId, bool>` | 被永久禁止的广告位 |
| `PlacementFlagCount` | `StorageMap<PlacementId, u32>` | 广告位被举报次数 |
| `PlacementFlaggedBy` | `StorageDoubleMap<PlacementId, AccountId, bool>` | 广告位举报去重 |

---

## Extrinsics

| call_index | 函数名 | 权限 | 说明 |
|------------|--------|------|------|
| 0 | `create_campaign` | Signed | 创建广告活动, 锁定预算 (reserve) |
| 1 | `fund_campaign` | Signed (owner) | 追加预算, Exhausted→Active 自动恢复; 阻止对已过期 Campaign 追加 |
| 2 | `pause_campaign` | Signed (owner) | 暂停广告活动 (Active→Paused) |
| 3 | `cancel_campaign` | Signed (owner) | 取消广告活动, 退还剩余预算 |
| 4 | `review_campaign` | Root | 审核广告内容 (approve/reject), 已 Rejected 不可重审 |
| 5 | `submit_delivery_receipt` | Signed | 提交投放收据, 委托 `DeliveryVerifier` 验证 + 双向黑名单检查 |
| 6 | `settle_era_ads` | Signed (any) | 结算广告位 Era 收入, 委托 `RevenueDistributor` 分配 |
| 7 | `flag_campaign` | Signed | 举报广告活动 (仅 Pending 审核状态) |
| 8 | `claim_ad_revenue` | Signed (admin) | 广告位管理员提取广告收入 |
| 9 | `advertiser_block_placement` | Signed | 广告主拉黑广告位 |
| 10 | `advertiser_unblock_placement` | Signed | 广告主取消拉黑广告位 |
| 11 | `advertiser_prefer_placement` | Signed | 广告主指定广告位 (白名单) |
| 12 | `advertiser_unprefer_placement` | Signed | 广告主取消指定广告位 |
| 13 | `placement_block_advertiser` | Signed (admin) | 广告位拉黑广告主 |
| 14 | `placement_unblock_advertiser` | Signed (admin) | 广告位取消拉黑广告主 |
| 15 | `placement_prefer_advertiser` | Signed (admin) | 广告位指定广告主 (白名单) |
| 16 | `placement_unprefer_advertiser` | Signed (admin) | 广告位取消指定广告主 |
| 17 | `flag_placement` | Signed (any) | 举报广告位 (同一用户不可重复举报) |
| 18 | `slash_placement` | Root | Slash 广告位, 连续 3 次→永久禁止 |
| 19 | `register_private_ad` | Signed (admin) | 私域广告自助登记, 扣除注册费 |
| 20 | `resume_campaign` | Signed (owner) | 恢复已暂停的广告活动 (Paused→Active), 阻止恢复已过期 Campaign |
| 21 | `expire_campaign` | Signed (any) | 标记已过期的广告活动, 退还剩余预算 (任何人可调用) |

---

## Events

| 事件 | 字段 | 触发条件 |
|------|------|----------|
| `CampaignCreated` | campaign_id, advertiser, total_budget, bid_per_mille | 广告活动创建 |
| `CampaignFunded` | campaign_id, amount | 追加预算 |
| `CampaignPaused` | campaign_id | 暂停 |
| `CampaignCancelled` | campaign_id, refunded | 取消并退款 |
| `CampaignReviewed` | campaign_id, approved | 审核完成 |
| `DeliveryReceiptSubmitted` | campaign_id, placement_id, audience_size | 投放收据提交 |
| `EraAdsSettled` | placement_id, total_cost, placement_share | Era 结算完成 |
| `CampaignFlagged` | campaign_id, reporter | 广告被举报 |
| `AdRevenueClaimed` | placement_id, amount, claimer | 广告收入提取 |
| `PlacementSlashed` | placement_id, slashed_amount, slash_count | 广告位被 Slash |
| `PlacementBannedFromAds` | placement_id | 广告位被永久禁止 (连续 Slash ≥ 3) |
| `AdvertiserBlockedPlacement` | advertiser, placement_id | 广告主拉黑广告位 |
| `AdvertiserUnblockedPlacement` | advertiser, placement_id | 广告主取消拉黑 |
| `AdvertiserPreferredPlacement` | advertiser, placement_id | 广告主指定广告位 |
| `AdvertiserUnpreferredPlacement` | advertiser, placement_id | 广告主取消指定 |
| `PlacementBlockedAdvertiser` | placement_id, advertiser | 广告位拉黑广告主 |
| `PlacementUnblockedAdvertiser` | placement_id, advertiser | 广告位取消拉黑 |
| `PlacementPreferredAdvertiser` | placement_id, advertiser | 广告位指定广告主 |
| `PlacementUnpreferredAdvertiser` | placement_id, advertiser | 广告位取消指定 |
| `PlacementFlagged` | placement_id, reporter, flag_count | 广告位被举报 |
| `PrivateAdRegistered` | placement_id, registrar, count | 私有广告注册 |
| `CampaignResumed` | campaign_id | 广告活动恢复 |
| `CampaignMarkedExpired` | campaign_id, refunded | 广告活动已过期, 剩余预算已退还 |

---

## Errors

| 错误码 | 说明 |
|--------|------|
| `CampaignNotFound` | Campaign 不存在 |
| `NotCampaignOwner` | 非广告主本人 |
| `CampaignNotActive` | Campaign 非 Active 状态 |
| `BidTooLow` | 出价低于最低 CPM |
| `ZeroBudget` | 预算为零 |
| `EmptyAdText` | 广告文本为空 |
| `InvalidDeliveryTypes` | 投放类型无效 (必须 1~7) |
| `ReceiptsFull` | 收据已满 |
| `BlacklistFull` | 黑名单已满 |
| `WhitelistFull` | 白名单已满 |
| `AlreadyBlacklisted` | 已在黑名单中 |
| `NotBlacklisted` | 不在黑名单中 |
| `AlreadyWhitelisted` | 已在白名单中 |
| `NotWhitelisted` | 不在白名单中 |
| `PlacementBanned` | 广告位已被永久禁止 |
| `NothingToClaim` | 无可提取收入 |
| `CampaignInactive` | Campaign 已取消/过期 (不可追加预算或取消) |
| `AlreadyReviewed` | 已 Rejected 的 Campaign 不可重复审核 |
| `CampaignNotApproved` | Campaign 需先通过审核才能投放 |
| `AudienceBelowMinimum` | 有效受众低于 MinAudienceSize 门槛 |
| `DeliveryVerificationFailed` | 投放验证失败 (适配层返回错误) |
| `NotPlacementAdmin` | 非广告位管理员 |
| `CampaignExpired` | Campaign 已过期 (当前区块 > expires_at) |
| `ZeroPrivateAdCount` | 私有广告注册次数必须 > 0 |
| `CampaignIdOverflow` | Campaign ID 溢出 (u64::MAX) |
| `InvalidExpiry` | 过期时间无效 (必须晚于当前区块) |
| `AlreadyFlaggedPlacement` | 已举报过该广告位 |
| `CampaignNotPaused` | Campaign 非 Paused 状态 (resume 前置检查) |
| `AdvertiserBlacklistedPlacement` | 广告主已拉黑该广告位 (投放被拒) |
| `PlacementBlacklistedAdvertiser` | 广告位已拉黑该广告主 (投放被拒) |
| `CampaignNotExpired` | Campaign 尚未过期 (expire 前置检查) |

---

## Trait 实现

本 pallet 同时实现了以下对外 Trait，供其他 pallet 查询广告状态：

| Trait | 方法 | 说明 |
|-------|------|------|
| `AdScheduleProvider` | `is_ads_enabled` | 广告位是否有投放活动 (有收入记录或本 Era 有投放) |
| `AdScheduleProvider` | `placement_ad_revenue` | 广告位累计收入 (u128) |
| `AdDeliveryCountProvider` | `era_delivery_count` | 当前 Era 投放次数 |
| `AdDeliveryCountProvider` | `reset_era_deliveries` | 重置 Era 投放计数 |

---

## 内部函数

| 函数 | 说明 |
|------|------|
| `compute_cpm_cost(bid_per_mille, audience, multiplier_bps)` | CPM 费用: `bid × audience × multiplier / 100_000` (其中 /1000 为 CPM 标准, /100 为系数归一化) |

---

## 结算流程

`settle_era_ads` 的完整流程：

1. 收集广告位所有未结算收据快照
2. 对每笔收据计算 CPM 费用 (取 escrow 余额与计算值的较小值)
3. `unreserve` 广告主的 reserve 资金, 检查返回的 deficit
4. 全额转入国库 (转账失败则 re-reserve 回退, skip 该收据)
5. 委托 `RevenueDistributor::distribute()` 从国库分配收入给各方
6. 记录广告位可提取份额 (`PlacementClaimable`)
7. 更新 Campaign escrow 和 spent (预算耗尽时自动标记 `Exhausted`)
8. 清空本 Era 收据并更新收入统计

> 审计改进: 单个 Campaign 转账失败不会阻塞整体结算 (skip + log::warn)。

---

## 依赖关系

```
pallet-ads-core (本 crate)
├── pallet-ads-primitives (共享类型 + Trait)
├── frame-support / frame-system
├── sp-runtime
└── log
```

## 许可证

Apache-2.0
