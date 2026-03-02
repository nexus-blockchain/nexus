# pallet-grouprobot-ads

> **⚠️ DEPRECATED** — 此 pallet 已拆分为三层模块化架构:
>
> | 新 crate | 路径 | 职责 |
> |---------|------|------|
> | `pallet-ads-primitives` | `pallets/ads/primitives/` | 通用广告类型 + 核心 trait |
> | `pallet-ads-core` | `pallets/ads/core/` | 通用广告引擎 (Campaign CRUD, Escrow, 偏好, Slash, 收入) |
> | `pallet-ads-grouprobot` | `pallets/ads/grouprobot/` | GroupRobot 适配层 (TEE 验证, audience, 反作弊, 节点奖励) |
>
> 新代码请使用上述 crate。Runtime 已注册 `AdsCore`(index 160) + `AdsGroupRobot`(index 161)。
> 此 crate 保留仅供历史参考，不再注册到 runtime。

---

群组广告竞价 + CPM 结算 + 质押反作弊 + 双向偏好控制。

## 概述 (历史参考)

本 Pallet 为 GroupRobot 生态提供完整的去中心化广告系统，支持广告主创建 Campaign 并通过 CPM（每千人触达）竞价模式向社区投放广告。社区通过质押获取 audience 上限，Bot 节点上报投放收据后按 Era 结算，收入在社区、国库和节点之间按比例分成。

## 核心功能

### 1. Campaign 生命周期管理

| 操作 | call_index | 权限 | 说明 |
|------|-----------|------|------|
| `create_campaign` | 0 | signed | 创建广告活动，锁定预算至 escrow |
| `fund_campaign` | 1 | 广告主 | 追加预算，预算耗尽的 Campaign 自动恢复 |
| `pause_campaign` | 2 | 广告主 | 暂停投放 |
| `cancel_campaign` | 3 | 广告主 | 取消并退还剩余预算 |
| `review_campaign` | 4 | Root | 审核广告内容（通过/拒绝） |
| `flag_campaign` | 7 | signed | 举报广告，标记为 Flagged 待审核 |

**Campaign 状态流转:**
```
Active → Paused → Active (恢复)
Active → Cancelled (退款)
Active → Exhausted (预算耗尽) → Active (追加预算后恢复)
Pending → Approved / Rejected (审核)
```

### 2. CPM 投放与结算

- **投放收据** (`submit_delivery_receipt`, call_index 5): Bot 节点上报每次广告投放的 audience 数据和签名
- **Era 结算** (`settle_era_ads`, call_index 6): 任何人可触发，按 CPM 公式计费并分配收入

**CPM 计费公式:**
```
cost = bid_per_mille × min(audience_size, audience_cap) / 1000
```

**收入分成 (默认):**

| 分配对象 | 比例 |
|---------|------|
| 社区 (群主) | 60% |
| 国库 + 节点 | 40% |

### 3. 质押与 Audience Cap

| 操作 | call_index | 说明 |
|------|-----------|------|
| `stake_for_ads` | 9 | 质押代币获取 audience 上限 |
| `unstake_from_ads` | 10 | 取消质押，降低上限 |

**质押阶梯函数:**

| 质押区间 | 增长率 | 累计上限 |
|---------|--------|---------|
| 0–50 UNIT | 20 人/UNIT | 1,000 |
| 50–200 UNIT | ~27 人/UNIT | 5,000 |
| 200+ UNIT | ~17 人/UNIT | 10,000 (硬上限) |

### 4. 双向偏好控制

广告主和社区均可设置黑名单/白名单，实现双向选择：

**广告主侧:**

| 操作 | call_index | 说明 |
|------|-----------|------|
| `advertiser_block_community` | 11 | 拉黑社区 |
| `advertiser_unblock_community` | 12 | 取消拉黑 |
| `advertiser_prefer_community` | 13 | 加入白名单（指定社区） |

**社区侧:**

| 操作 | call_index | 说明 |
|------|-----------|------|
| `community_block_advertiser` | 14 | 拉黑广告主 |
| `community_unblock_advertiser` | 15 | 取消拉黑 |
| `community_prefer_advertiser` | 16 | 加入白名单（指定广告主） |

### 5. Slash 与惩罚机制

- **`slash_community`** (call_index 17, Root): 扣除质押的一定百分比（由 `AdSlashPercentage` 配置），50% 奖励举报者，50% 入国库
- **audience_cap 砍半**: 每次 Slash 后上限减半
- **永久封禁**: 连续 3 次 Slash → 社区被永久禁止接入广告

### 6. 反作弊系统 (Phase 5)

#### L3: Audience 突增检测

- **`check_audience_surge`** (call_index 20): 检测社区活跃人数是否异常突增
- 突增超过 `AudienceSurgeThresholdPct` 阈值 → 自动暂停广告 2 个 Era
- 暂停期间 `submit_delivery_receipt` 和 `settle_era_ads` 均被阻止

#### L5: 多节点交叉验证

- **`report_node_audience`** (call_index 19): 每个节点独立上报 audience 统计
- **`flag_community`** (call_index 18): 社区级别作弊举报
- Era 结算时验证多节点上报偏差，超过 `NodeDeviationThresholdPct` → 拒绝结算

## 数据结构

### AdCampaign

```rust
pub struct AdCampaign<T: Config> {
    pub advertiser: T::AccountId,
    pub text: BoundedVec<u8, T::MaxAdTextLength>,      // 广告文本 ≤280 字节
    pub url: BoundedVec<u8, T::MaxAdUrlLength>,         // 链接 ≤256 字节
    pub bid_per_mille: BalanceOf<T>,                    // CPM 出价
    pub daily_budget: BalanceOf<T>,                     // 每日预算
    pub total_budget: BalanceOf<T>,                     // 总预算
    pub spent: BalanceOf<T>,                            // 已花费
    pub target: AdTargetTag,                            // 目标标签
    pub delivery_types: u8,                             // 投放类型 bitmask
    pub status: CampaignStatus,                         // Active/Paused/Cancelled/Exhausted
    pub review_status: AdReviewStatus,                  // Pending/Approved/Rejected/Flagged
    pub total_deliveries: u64,                          // 累计投放次数
    pub created_at: BlockNumberFor<T>,
    pub expires_at: BlockNumberFor<T>,
}
```

**投放类型 bitmask:**

| bit | 类型 | 说明 |
|-----|------|------|
| 0 | ScheduledPost | 定时推送广告消息 |
| 1 | ReplyFooter | 回复消息底部附加广告 |
| 2 | WelcomeEmbed | 新成员欢迎消息嵌入广告 |

### DeliveryReceipt

```rust
pub struct DeliveryReceipt<T: Config> {
    pub campaign_id: u64,
    pub community_id_hash: CommunityIdHash,
    pub delivery_type: AdDeliveryType,
    pub audience_size: u32,
    pub node_signature: [u8; 64],
    pub delivered_at: BlockNumberFor<T>,
    pub settled: bool,
}
```

### CommunityAdSchedule

```rust
pub struct CommunityAdSchedule {
    pub community_id_hash: CommunityIdHash,
    pub scheduled_campaigns: BoundedVec<u64, ConstU32<10>>,  // 每 Era 最多 10 个 Campaign
    pub daily_limit: u8,
    pub delivered_this_era: u32,
}
```

## Storage

| 名称 | 类型 | 说明 |
|------|------|------|
| `NextCampaignId` | `StorageValue<u64>` | 下一个 Campaign ID |
| `Campaigns` | `StorageMap<u64, AdCampaign>` | 广告活动 |
| `CampaignEscrow` | `StorageMap<u64, Balance>` | Campaign 锁定预算 |
| `CommunitySchedules` | `StorageMap<Hash, CommunityAdSchedule>` | 社区排期 |
| `DeliveryReceipts` | `StorageMap<Hash, BoundedVec<Receipt>>` | 投放收据（Era 级） |
| `EraAdRevenue` | `StorageMap<Hash, Balance>` | 每 Era 社区广告收入 |
| `CommunityTotalRevenue` | `StorageMap<Hash, Balance>` | 社区累计总收入 |
| `CommunityClaimable` | `StorageMap<Hash, Balance>` | 社区待提取收入 |
| `CommunityAdStake` | `StorageMap<Hash, Balance>` | 社区广告质押 |
| `CommunityAudienceCap` | `StorageMap<Hash, u32>` | 质押决定的 audience 上限 |
| `AdvertiserBlacklist` | `StorageMap<AccountId, BoundedVec<Hash>>` | 广告主拉黑的社区 |
| `AdvertiserWhitelist` | `StorageMap<AccountId, BoundedVec<Hash>>` | 广告主指定的社区 |
| `CommunityBlacklist` | `StorageMap<Hash, BoundedVec<AccountId>>` | 社区拉黑的广告主 |
| `CommunityWhitelist` | `StorageMap<Hash, BoundedVec<AccountId>>` | 社区指定的广告主 |
| `SlashCount` | `StorageMap<Hash, u32>` | 连续 Slash 次数 |
| `BannedCommunities` | `StorageMap<Hash, bool>` | 永久禁止广告的社区 |
| `PreviousEraAudience` | `StorageMap<Hash, u32>` | 上一 Era 社区活跃人数 |
| `AudienceSurgePaused` | `StorageMap<Hash, u32>` | 突增暂停 Era 计数 |
| `CommunityFlagCount` | `StorageMap<Hash, u32>` | 社区举报次数 |
| `NodeAudienceReports` | `StorageMap<Hash, BoundedVec<(u32,u32),10>>` | 多节点 audience 上报 |

## Config 常量

| 常量 | 说明 | 运行时建议值 |
|------|------|------------|
| `MaxAdTextLength` | 广告文本最大字节数 | 280 |
| `MaxAdUrlLength` | 广告 URL 最大字节数 | 256 |
| `MaxReceiptsPerCommunity` | 每社区最大收据数 | 100 |
| `MaxAdvertiserBlacklist` | 广告主黑名单上限 | 50 |
| `MaxAdvertiserWhitelist` | 广告主白名单上限 | 50 |
| `MaxCommunityBlacklist` | 社区黑名单上限 | 50 |
| `MaxCommunityWhitelist` | 社区白名单上限 | 50 |
| `MinBidPerMille` | 最低 CPM 出价 | 视经济模型 |
| `MinAudienceSize` | 接入广告的最低活跃人数 | 50 |
| `AudienceSurgeThresholdPct` | L3 突增阈值（%） | 100 |
| `NodeDeviationThresholdPct` | L5 多节点偏差阈值（%） | 20 |
| `AdSlashPercentage` | Slash 扣除比例（%） | 30 |
| `TreasuryAccount` | 国库账户 | — |

## 广告投放完整流程

```
广告主                    链上                       Bot 节点
  │                        │                          │
  ├─ create_campaign ─────►│                          │
  │   (锁定预算)            │                          │
  │                        │                          │
  │              Root ─── review_campaign              │
  │                (审核通过)│                          │
  │                        │                          │
  │                        │◄── submit_delivery_receipt │
  │                        │     (上报投放收据)          │
  │                        │                          │
  │    任何人 ─── settle_era_ads                       │
  │               (Era 结算) │                          │
  │                        │                          │
  │                        ├─ 60% → CommunityClaimable │
  │                        ├─ 40% → Treasury           │
  │                        │                          │
  群主 ─── claim_ad_revenue │                          │
           (提取收入)       │                          │
```

## Trait 实现

### AdScheduleProvider

```rust
impl<T: Config> AdScheduleProvider for Pallet<T> {
    fn is_ads_enabled(community_id_hash: &CommunityIdHash) -> bool;
    fn community_ad_revenue(community_id_hash: &CommunityIdHash) -> u128;
}
```

供 `pallet-grouprobot-consensus` 等外部模块查询社区广告状态和收入。

## 测试

```bash
cargo test -p pallet-grouprobot-ads
```

覆盖范围：Campaign CRUD、CPM 结算、质押/解质押、双向黑白名单、Slash 惩罚、L3 突增检测、L5 多节点交叉验证。

## License

Apache-2.0
