# pallet-ads-primitives

> **纯类型 + Trait 定义层 — 无 Storage、无 Extrinsic**

## 概述

`pallet-ads-primitives` 定义了广告系统所有子模块共享的类型别名、枚举、结构体和 Trait 接口。
核心目标是将 **通用广告概念** (Campaign 状态、审核流程、投放类型) 与
**领域特定概念** (GroupRobot 的 TEE 节点, Entity 的 Shop 展示) 解耦,
使 `pallet-ads-core` 可在不同业务场景下复用。

### 设计目标

| 目标 | 说明 |
|------|------|
| **解耦** | 核心引擎不依赖任何具体领域 (GroupRobot / Entity) |
| **可扩展** | 新业务场景只需实现 Trait 即可接入广告系统 |
| **安全默认值** | `()` 空实现提供最保守行为 (禁投放、零分成) |

---

## 类型定义

| 类型 | 定义 | 说明 |
|------|------|------|
| `PlacementId` | `[u8; 32]` | 广告位泛化标识 — GroupRobot 对应 `CommunityIdHash`, Entity 对应 `blake2_256("entity-ad:" ++ entity_id)` 或 `blake2_256("shop-ad:" ++ shop_id)` |

### 枚举

| 枚举 | 变体 | 默认值 | 用途 |
|------|------|--------|------|
| `CampaignStatus` | `Active` · `Paused` · `Exhausted` · `Expired` · `Cancelled` · `Suspended` · `UnderReview` | `Active` | 广告活动生命周期 |
| `AdReviewStatus` | `Pending` · `Approved` · `Rejected` · `Flagged` | `Pending` | 广告内容审核流程 |
| `CampaignType` | `Cpm` · `Cpc` · `Fixed` · `Private` | `Cpm` | 广告活动计费类型 (CPM 展示 / CPC 点击 / 固定费用 / 私有广告) |
| `PlacementStatus` | `Active` · `Paused` · `Banned` · `Unknown` | `Unknown` | 广告位状态 (由适配层报告) |
| `AdsRouterError` | `CpcNotSupportedForPath` | — | 路由层错误, 实现 `Into<DispatchError>` |

### 结构体

| 结构体 | 泛型 | 字段 | 说明 |
|--------|------|------|------|
| `ClickAttestation<AccountId>` | `AccountId` | `clicker`, `proxy`, `campaign_id: u64`, `placement_id`, `clicked_at: u64` | C2b Proxy Account 点击证明 — 用户主账户委托 proxy 签名的点击事件 |
| `RevenueBreakdown<Balance>` | `Balance` | `placement_share`, `node_share`, `platform_share` | 收入分配明细 — 广告位方/节点/平台三方份额 |

---

## Trait 接口

### 核心 Trait (适配层必须实现)

| Trait | 方法签名 | 说明 | GroupRobot 实现 | Entity 实现 |
|-------|---------|------|-----------------|-------------|
| `DeliveryVerifier<AccountId>` | `verify_and_cap_audience(who, placement_id, audience_size, node_id) → Result<u32>` | 验证投放合法性 + 裁切受众; `node_id: Option<[u8; 32]>` 用于 TEE 节点标识 | TEE 节点验证 + 订阅门控 + 突增暂停检查 + audience_cap 裁切 | 广告位注册/激活检查 + Entity 活跃检查 + 每日展示量上限 |
| `ClickVerifier<AccountId>` | `verify_and_cap_clicks(who, placement_id, click_count, verified_clicks) → Result<u32>` | 验证点击收据合法性 + 每日上限裁切; `verified_clicks` 为经 proxy 签名验证的点击数 (C2b) | — | Entity 活跃检查 + 每日点击量上限 + 权限验证 |
| `PlacementAdminProvider<AccountId>` | `placement_admin(placement_id) → Option<AccountId>` | 查询广告位管理员 | CommunityAdmin → Bot Owner 回退 | Shop Owner → Entity Owner 回退 |
| | `is_placement_banned(placement_id) → bool` | 广告位是否被禁止 | 恒返回 `false` (委托 ads-core) | 查询 BannedEntities |
| | `placement_status(placement_id) → PlacementStatus` | 查询广告位当前状态 | 查询社区状态映射 | 查询 Entity/Shop 状态映射 |
| `RevenueDistributor<AccountId, Balance>` | `distribute(placement_id, total_cost, advertiser) → Result<RevenueBreakdown<Balance>>` | 分配广告收入, 返回三方份额明细 | 三方分成 (社区默认 80% / 节点默认 15% / 国库 5%), 节点份额转入奖励池 | 二方分成 (Entity Owner 默认 80% / 平台默认 20%), 基点制 |

### 辅助 Trait

| Trait | 方法签名 | 说明 |
|-------|---------|------|
| `AdScheduleProvider` | `is_ads_enabled(placement_id) → bool` | 广告位是否启用广告 |
| | `placement_ad_revenue(placement_id) → u128` | 累计广告收入 |
| | `placement_era_revenue(placement_id) → u128` | 当前 Era 广告收入 |
| `AdDeliveryCountProvider` | `era_delivery_count(placement_id) → u32` | 当前 Era 投放次数 |
| | `reset_era_deliveries(placement_id)` | 重置 Era 投放计数 |
| `AdPolicyProvider` | `max_campaigns_per_placement(placement_id) → u32` | 广告位允许的最大并发活动数 (0 = 无限制) |
| | `min_campaign_budget(placement_id) → u128` | 创建活动的最低预算 (0 = 无门槛) |
| | `requires_review(placement_id) → bool` | 新活动是否需要审核 |
| `PlacementConfigProvider` | `daily_impression_cap(placement_id) → u32` | 每日展示量上限 (0 = 无限制) |
| | `revenue_share_bps(placement_id) → u32` | 收入分成比例 (基点, 10000 = 100%) |
| | `supports_private_ads(placement_id) → bool` | 是否支持私有广告 |

### `()` 空实现

所有 Trait 均提供 `()` 空实现，用于单元测试和最小化配置:

| Trait | `()` 行为 |
|-------|-----------|
| `DeliveryVerifier` | 直接返回原始 `audience_size` (不裁切、不校验) |
| `ClickVerifier` | 直接返回原始 `click_count` (不裁切、不校验) |
| `PlacementAdminProvider` | `None` / `false` / `PlacementStatus::Unknown` |
| `RevenueDistributor` | `placement_share = 0`, `node_share = 0`, `platform_share = total_cost` (全归国库) |
| `AdScheduleProvider` | `false` / `0` / `0` |
| `AdDeliveryCountProvider` | `0` / no-op |
| `AdPolicyProvider` | `0` / `0` / `false` (无限制、无门槛、无需审核) |
| `PlacementConfigProvider` | `0` / `0` / `false` (无上限、零分成、不支持私有) |

> ⚠️ **生产环境必须提供具体实现** — `()` 的零值默认会导致广告位无法投放或无法获得收入。

---

## 架构关系

```
                    ┌──────────────────────────────┐
                    │    pallet-ads-primitives      │
                    │  Types + Traits (本 crate)     │
                    └────────────┬─────────────────┘
                                 │
          ┌──────────────────────┼──────────────────────┐
          ▼                      ▼                       ▼
┌──────────────────┐   ┌─────────────────┐   ┌──────────────────┐
│  pallet-ads-core │   │  pallet-ads-    │   │  pallet-ads-     │
│  (通用广告引擎)   │   │  grouprobot     │   │  entity          │
│  Campaign CRUD   │   │  (适配层: TEE)  │   │  (适配层: DApp)  │
│  Escrow/结算     │   │  实现核心 Trait  │   │  实现核心 Trait   │
│  依赖 Trait 注入 │   │                 │   │                  │
└──────────────────┘   └─────────────────┘   └──────────────────┘
          │                                           │
          └────────────► pallet-ads-router ◄──────────┘
                         (路由分发层)
```

## 依赖

```toml
[dependencies]
codec       = { package = "parity-scale-codec" }
scale-info  = { ... }
frame-support = { ... }
sp-runtime  = { ... }
```

无外部 pallet 依赖。

---

## 审计记录

| 轮次 | 编号 | 级别 | 描述 | 状态 |
|------|------|------|------|------|
| R1 | L1 | Low | `DeliveryMethod::cpm_multiplier_bps` 注释歧义 — 补充说明非金融基点, 而是百分比整数 | ✅ 已修复 |
| R1 | L2 | Low | `PlacementStakeProvider::()` 返回 0 — 添加文档说明安全默认语义 | ✅ 已修复 |
| R1 | M1 | Medium | `PlacementId` 类型定义不明确 — 补充说明 GroupRobot 和 Entity 的不同定义 | ✅ 已修复 |
| R2 | M1-R2 | Medium | `community_ad_revenue` 重命名为 `placement_ad_revenue` (领域中立) | ✅ 已修复 |
| R2 | L1-R2 | Low | 移除死代码 `extern crate alloc` | ✅ 已修复 |
| R2 | L2-R2 | Low | 记录: `DeliveryMethod` trait 无下游实现者 (设计预留) | ✅ 已修复 |
| R2 | L3-R2 | Low | 记录: `PlacementStakeProvider` trait 仅 `()` 实现，无生产消费者 (设计预留) | ✅ 已修复 |
| R2 | L4-R2 | Low | 记录: `AdPreference` 枚举无下游使用 (设计预留) | ✅ 已修复 |
| R3 | R3-1 | Medium | 移除未使用的设计预留: `DeliveryMethod`, `PlacementStakeProvider`, `AdPreference` — R2 标记为预留后确认无下游需求, 清理死代码 | ✅ 已修复 |
| R3 | R3-2 | Medium | 新增 `CampaignType` 枚举 (Cpm/Cpc/Fixed/Private) — 支持多计费模式 | ✅ 已实现 |
| R3 | R3-3 | Medium | 新增 `PlacementStatus` 枚举 + `PlacementAdminProvider::placement_status()` — 广告位状态查询 | ✅ 已实现 |
| R3 | R3-4 | Medium | 新增 `ClickAttestation` 结构体 + `ClickVerifier` trait — C2b proxy 点击证明与验证 | ✅ 已实现 |
| R3 | R3-5 | Medium | 新增 `RevenueBreakdown` 结构体, `RevenueDistributor::distribute` 返回值从 `Balance` 改为 `RevenueBreakdown<Balance>` — 三方分成明细 | ✅ 已实现 |
| R3 | R3-6 | Low | 新增 `AdScheduleProvider::placement_era_revenue()` — 当前 Era 收入查询 | ✅ 已实现 |
| R3 | R3-7 | Low | 新增 `AdPolicyProvider` trait — 广告策略参数 (最大活动数/最低预算/审核要求) | ✅ 已实现 |
| R3 | R3-8 | Low | 新增 `PlacementConfigProvider` trait — 广告位配置参数 (展示上限/分成比例/私有广告) | ✅ 已实现 |
| R3 | R3-9 | Low | 新增 `AdsRouterError` 枚举 + `Into<DispatchError>` — 路由层错误类型 | ✅ 已实现 |
| R3 | R3-10 | Low | `DeliveryVerifier::verify_and_cap_audience` 新增 `node_id: Option<[u8; 32]>` 参数 — TEE 节点标识 | ✅ 已实现 |
| R3 | R3-11 | Low | `CampaignStatus` 新增 `Suspended` (治理暂停) 和 `UnderReview` (审核中) 变体 | ✅ 已实现 |
| R3 | R3-12 | Low | 补充枚举 Encode/Decode 往返单元测试 | ✅ 已实现 |
| R3 | R3-13 | Low | README 全面同步至代码实际状态 | ✅ 已修复 |

## 许可证

Apache-2.0
