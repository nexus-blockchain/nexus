# pallet-ads-primitives

> **纯类型 + Trait 定义层 — 无 Storage、无 Extrinsic**

## 概述

`pallet-ads-primitives` 定义了广告系统所有子模块共享的类型别名、枚举和 Trait 接口。
核心目标是将 **通用广告概念** (Campaign 状态、审核流程、偏好控制) 与
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
| `CampaignStatus` | `Active` · `Paused` · `Exhausted` · `Expired` · `Cancelled` | `Active` | 广告活动生命周期 |
| `AdReviewStatus` | `Pending` · `Approved` · `Rejected` · `Flagged` | `Pending` | 广告内容审核流程 |
| `AdPreference` | `Allow` · `Blocked` · `Preferred` | `Allow` | 双向偏好控制 (广告主 ⇄ 广告位) |

---

## Trait 接口

### 核心 Trait (适配层必须实现)

| Trait | 方法签名 | 说明 | GroupRobot 实现 | Entity 实现 |
|-------|---------|------|-----------------|-------------|
| `DeliveryVerifier<AccountId>` | `verify_and_cap_audience(who, placement_id, audience_size) → Result<u32>` | 验证投放合法性 + 裁切受众 | TEE 节点验证 + 订阅门控 + 突增暂停检查 + audience_cap 裁切 | 广告位注册/激活检查 + Entity 活跃检查 + 每日展示量上限 |
| `PlacementAdminProvider<AccountId>` | `placement_admin(placement_id) → Option<AccountId>` | 查询广告位管理员 | CommunityAdmin → Bot Owner 回退 | Shop Owner → Entity Owner 回退 |
| | `is_placement_banned(placement_id) → bool` | 广告位是否被禁止 | 恒返回 `false` (委托 ads-core) | 查询 BannedEntities |
| `RevenueDistributor<AccountId, Balance>` | `distribute(placement_id, total_cost, advertiser) → Result<Balance>` | 分配广告收入, 返回广告位方可提取份额 | 三方分成 (社区默认 80% / 节点默认 15% / 国库 5%), 节点份额转入奖励池 | 二方分成 (Entity Owner 默认 80% / 平台默认 20%), 基点制 |

### 辅助 Trait

| Trait | 方法签名 | 说明 |
|-------|---------|------|
| `DeliveryMethod` | `cpm_multiplier_bps() → u32` | 投放方式的 CPM 定价系数 (百分比整数, 100 = 1.0x, 200 = 2.0x)。ads-core 公式: `bid × audience × multiplier / 100_000` |
| `PlacementStakeProvider<Balance>` | `audience_cap(placement_id) → u32` | 查询质押决定的受众上限 |
| | `stake_amount(placement_id) → Balance` | 查询质押额 |
| `AdScheduleProvider` | `is_ads_enabled(placement_id) → bool` | 广告位是否启用广告 (有收入记录或本 Era 有投放) |
| | `placement_ad_revenue(placement_id) → u128` | 累计广告收入 |
| `AdDeliveryCountProvider` | `era_delivery_count(placement_id) → u32` | 当前 Era 投放次数 |
| | `reset_era_deliveries(placement_id)` | 重置 Era 投放计数 |

### `()` 空实现

所有 Trait 均提供 `()` 空实现，用于单元测试和最小化配置:

| Trait | `()` 行为 |
|-------|-----------|
| `DeliveryVerifier` | 直接返回原始 `audience_size` (不裁切、不校验) |
| `PlacementAdminProvider` | `None` / `false` |
| `RevenueDistributor` | 返回 `Balance::default()` (零份额, 全归国库) |
| `PlacementStakeProvider` | `audience_cap = 0` / `stake = 0` |
| `AdScheduleProvider` | `false` / `0` |
| `AdDeliveryCountProvider` | `0` / no-op |

> ⚠️ **生产环境必须提供具体实现** — `()` 的零值默认会导致广告位无法投放或无法获得收入。

---

## 架构关系

```
                    ┌──────────────────────────────┐
                    │    pallet-ads-primitives      │
                    │  Types + Traits (本 crate)     │
                    └────────────┬─────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              ▼                  ▼                   ▼
   ┌──────────────────┐ ┌───────────────┐ ┌──────────────────┐
   │  pallet-ads-core │ │ pallet-ads-   │ │  pallet-ads-     │
   │  (通用广告引擎)   │ │ grouprobot    │ │  entity          │
   │  Campaign CRUD   │ │ (适配层: TEE) │ │  (适配层: DApp)  │
   │  Escrow/结算/偏好│ │ 实现 3 核心    │ │  实现 3 核心      │
   │  依赖 Trait 注入 │ │ Trait         │ │  Trait            │
   └──────────────────┘ └───────────────┘ └──────────────────┘
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

## 许可证

Apache-2.0
