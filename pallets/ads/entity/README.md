# pallet-ads-entity

> **Entity DApp 广告适配层 — 展示量验证 · Entity/Shop 级广告位管理 · 二方收入分成 · 保证金机制**

## 概述

`pallet-ads-entity` 是 Entity DApp 生态的广告适配层，为 `pallet-ads-core` 提供领域特定的投放验证、管理员映射和收入分配实现。本 pallet **不包含 Campaign CRUD 等核心广告逻辑**（由 ads-core 提供），仅实现 Entity 专属的适配 Trait 和额外 extrinsic。

## 架构定位

```
pallet-ads-core (核心引擎)
    │
    ├── DeliveryVerifier       ← pallet-ads-entity 实现 (注册/激活/Entity 状态/展示量上限)
    ├── PlacementAdminProvider ← pallet-ads-entity 实现 (Entity Owner / Shop Owner)
    └── RevenueDistributor     ← pallet-ads-entity 实现 (二方分成: Entity Owner / 平台)
```

---

## 核心功能

### 1. PlacementId 映射

Entity 广告系统使用确定性哈希生成广告位 ID:

| 级别 | 算法 | 说明 |
|------|------|------|
| Entity 级 | `blake2_256(b"entity-ad:" ++ entity_id.to_le_bytes())` | 整个 Entity 的所有 Shop 共享 |
| Shop 级 | `blake2_256(b"shop-ad:" ++ shop_id.to_le_bytes())` | 仅限单个 Shop |

公开函数 `entity_placement_id(entity_id)` 和 `shop_placement_id(shop_id)` 可供外部调用。

### 2. 广告位注册 (Opt-in)

Entity 需主动注册广告位才能接入广告系统:

- **Entity 级广告位** — Entity Owner 或拥有 `ADS_MANAGE` 权限的 Admin 注册, 所有 Shop 共享
- **Shop 级广告位** — Entity Owner、Admin (`ADS_MANAGE`) 或 Shop Manager 注册, 仅限单个 Shop
- 注册时需缴纳保证金 (`AdPlacementDeposit`), 注销时退还给注册者
- 每个 Entity 最多注册 `MaxPlacementsPerEntity` 个广告位
- 被禁止的 Entity (`BannedEntities`) 无法注册新广告位

### 3. 展示量验证 (DeliveryVerifier)

`verify_and_cap_audience` 的验证流程:

1. **广告位注册与激活检查** — 必须已注册且 `active = true`
2. **Entity 状态检查** — Entity 必须活跃且未被禁止 (`BannedEntities`)
3. **调用者权限验证** — Entity Owner / Admin (`ADS_MANAGE`) / Shop Manager (Shop 级)
4. **每日展示量上限** — 自动按 `BlocksPerDay` 周期重置, 剩余配额为 `cap - current`, 超限则拒绝; cap=0 表示无限制
5. **展示量计数更新** — 递增 `DailyImpressions` 和 `TotalImpressions`

### 4. 二方收入分成 (RevenueDistributor)

ads-core `settle_era_ads` 将广告费转入国库后, 调用 `distribute`:

| 接收方 | 计算方式 | 说明 |
|--------|----------|------|
| Entity Owner | `effective_entity_share_bps` 基点 | 默认 = 10000 - PlatformAdShareBps (即 80%) |
| 平台 (国库) | 剩余部分 (total_cost - entity_share) | 默认 PlatformAdShareBps = 2000 (20%) |

- Entity Owner 可通过 `set_entity_ad_share` 自定义分成比例 (不可超过 10000 - PlatformAdShareBps)
- 自定义值为 0 时使用默认比例
- Entity Owner 份额返回给 ads-core 记入 `PlacementClaimable`

### 5. 广告位管理员映射 (PlacementAdminProvider)

管理员解析逻辑:
- **Shop 级** — 优先 `ShopProvider::shop_owner`, 回退到 `EntityProvider::entity_owner`
- **Entity 级** — `EntityProvider::entity_owner`

`is_placement_banned` 通过 `BannedEntities` 存储判断。

---

## 数据结构

### AdPlacementInfo

广告位注册信息:

| 字段 | 类型 | 说明 |
|------|------|------|
| `entity_id` | `u64` | 所属 Entity ID |
| `shop_id` | `u64` | 所属 Shop ID (0 = Entity 级) |
| `level` | `PlacementLevel` | `Entity` / `Shop` |
| `daily_impression_cap` | `u32` | 每日展示量上限 (0 = 无限制) |
| `registered_by` | `AccountId` | 注册者 |
| `registered_at` | `BlockNumber` | 注册区块号 |
| `active` | `bool` | 是否活跃 |

### PlacementLevel

| 变体 | 默认值 | 说明 |
|------|--------|------|
| `Entity` | ✅ | Entity 级 (整个 Entity 的所有 Shop 共享) |
| `Shop` | | Shop 级 (仅限单个 Shop) |

---

## Config

### 适配层依赖

| Config 类型 | Trait | 说明 |
|-------------|-------|------|
| `Currency` | `ReservableCurrency` | 保证金锁定/释放 |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 状态/所有者/管理员查询 (含 `AdminPermission::ADS_MANAGE`) |
| `ShopProvider` | `ShopProvider<AccountId>` | Shop 状态/所属 Entity/管理员查询 |
| `TreasuryAccount` | `Get<AccountId>` | 平台国库账户 |

### 常量

| 常量 | 类型 | 说明 |
|------|------|------|
| `PlatformAdShareBps` | `u16` | 平台广告分成 (基点, 默认 2000 = 20%) |
| `AdPlacementDeposit` | `Balance` | 注册广告位所需最低保证金 |
| `MaxPlacementsPerEntity` | `u32` | 每个 Entity 最大广告位数 |
| `DefaultDailyImpressionCap` | `u32` | 默认每日展示量上限 |
| `BlocksPerDay` | `u32` | 每日区块数 (用于展示量重置周期, 默认 14400 ≈ 24h @ 6s/block) |

---

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `RegisteredPlacements` | `StorageMap<PlacementId, AdPlacementInfo>` | 已注册的广告位信息 |
| `EntityPlacementIds` | `StorageMap<u64, BoundedVec<PlacementId, MaxPlacementsPerEntity>>` | Entity 下注册的广告位 ID 列表 |
| `DailyImpressions` | `StorageMap<PlacementId, u32>` | 广告位今日展示量计数 |
| `TotalImpressions` | `StorageMap<PlacementId, u64>` | 广告位累计展示量 |
| `PlacementDeposits` | `StorageMap<PlacementId, Balance>` | 广告位保证金存入额 |
| `EntityAdShareBps` | `StorageMap<u64, u16>` | Entity 自定义分成比例 (基点, 0 = 使用默认) |
| `BannedEntities` | `StorageMap<u64, bool>` | 被禁止投放广告的 Entity |
| `ImpressionResetBlock` | `StorageMap<PlacementId, BlockNumber>` | 展示量计数器最后重置区块 |

---

## Extrinsics

| call_index | 函数名 | 权限 | 说明 |
|------------|--------|------|------|
| 0 | `register_entity_placement` | Signed (entity owner/admin) | 注册 Entity 级广告位, 缴纳保证金 |
| 1 | `register_shop_placement` | Signed (entity owner/admin/shop manager) | 注册 Shop 级广告位, 验证 Shop 属于该 Entity |
| 2 | `deregister_placement` | Signed (entity owner/admin/registrar) | 注销广告位, 退还保证金, 清理所有相关存储 |
| 3 | `set_placement_active` | Signed (entity owner/admin/registrar) | 激活/禁用广告位, 状态未变则报错 |
| 4 | `set_impression_cap` | Signed (entity owner/admin/registrar) | 设置广告位每日展示量上限, 值未变则报错 |
| 5 | `set_entity_ad_share` | Signed (entity owner) | 设置 Entity 自定义广告分成比例 (≤ 10000 - PlatformAdShareBps) |
| 6 | `ban_entity` | Root | 禁止 Entity 参与广告, 已禁止则报错 |
| 7 | `unban_entity` | Root | 解除 Entity 广告禁令, 未禁止则报错 |

---

## Events

| 事件 | 字段 | 触发条件 |
|------|------|----------|
| `PlacementRegistered` | entity_id, shop_id, placement_id, level, deposit | 广告位注册 |
| `PlacementDeregistered` | placement_id, deposit_returned | 广告位注销 |
| `PlacementStatusUpdated` | placement_id, active | 广告位激活/禁用 |
| `ImpressionCapUpdated` | placement_id, daily_cap | 每日展示量上限更新 |
| `EntityShareUpdated` | entity_id, share_bps | Entity 分成比例更新 |
| `EntityBanned` | entity_id | Entity 被禁止广告 |
| `EntityUnbanned` | entity_id | Entity 禁令解除 |

---

## Errors

| 错误码 | 说明 |
|--------|------|
| `EntityNotFound` | Entity 不存在 |
| `EntityNotActive` | Entity 未激活 |
| `ShopNotFound` | Shop 不存在 |
| `ShopNotActive` | Shop 未激活 |
| `ShopEntityMismatch` | Shop 不属于该 Entity |
| `NotEntityAdmin` | 调用者不是 Entity Owner 或 Admin (`ADS_MANAGE`) |
| `NotShopManager` | 调用者不是 Shop 管理员 (且非 Entity Owner/Admin) |
| `PlacementAlreadyRegistered` | 广告位已注册 |
| `PlacementNotRegistered` | 广告位未注册 |
| `PlacementNotActive` | 广告位未激活 |
| `MaxPlacementsReached` | Entity 广告位数量达上限 |
| `InvalidShareBps` | 分成比例无效 (超过 10000 - PlatformAdShareBps) |
| `EntityBanned` | Entity 已被禁止 |
| `DailyImpressionCapReached` | 每日展示量已达上限 (剩余配额为零) |
| `EntityAlreadyBanned` | Entity 已被禁止 (重复禁止) |
| `EntityNotBanned` | Entity 未被禁止 (无需解禁) |
| `PlacementStatusUnchanged` | 广告位激活状态未变更 |
| `ImpressionCapUnchanged` | 每日展示量上限未变更 |

---

## 内部函数

| 函数 | 说明 |
|------|------|
| `entity_placement_id(entity_id)` | 生成 Entity 级 PlacementId (公开) |
| `shop_placement_id(shop_id)` | 生成 Shop 级 PlacementId (公开) |
| `bps_of(amount, bps)` | 基点百分比计算: `amount × bps / 10000` |
| `effective_entity_share_bps(entity_id)` | Entity 有效分成比例 (自定义 > 0 则使用, 否则 10000 - PlatformAdShareBps) |
| `check_and_reset_daily(placement_id)` | 检查并重置每日展示量计数器 (按 BlocksPerDay 周期) |
| `placement_entity_id(placement_id)` | 查找 PlacementId 对应的 entity_id |

---

> **⚠ 注意**: `deregister_placement` 不检查 ads-core 中的 `PlacementClaimable` 存储。若广告位有未领取的广告收入, 注销后 `PlacementAdminProvider::placement_admin` 返回 None, 收入将永久滞留国库。**请确保在注销广告位前先通过 ads-core 的 `claim_ad_revenue` 提取所有收入。**

---

## 依赖关系

```
pallet-ads-entity (本 crate)
├── pallet-ads-primitives (共享类型 + Trait)
├── pallet-entity-common (EntityProvider / ShopProvider Trait + AdminPermission)
├── frame-support / frame-system
├── sp-runtime
└── sp-core (blake2_256 哈希)
```

## 许可证

Apache-2.0
