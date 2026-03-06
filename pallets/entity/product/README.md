# pallet-entity-product

> NEXUS Entity 商品全生命周期管理 — USDT 等值押金、IPFS 元数据持久化、多角色权限、库存自动状态机、批量操作、治理接口

## 概述

`pallet-entity-product` 管理 Entity 商城中商品从创建到删除的完整生命周期。模块覆盖 10 个 extrinsics、5 个存储项、11 个事件、21 个错误码，共 166 个单元测试。

**核心能力：**

- **USDT 等值押金** — 创建时从店铺派生账户扣取 1 USDT 等值 NEX，删除时 best-effort 退还
- **IPFS 元数据** — 名称/图片/详情/标签/SKU 五类 CID 的 pin/unpin 生命周期管理
- **状态机驱动** — Draft → OnSale ⇌ OffShelf / SoldOut，统计计数器精确同步
- **多角色权限** — Owner / Admin(SHOP_MANAGE) / Manager 三级分权，Entity 锁定全局拦截
- **库存管理** — 有限库存自动售罄/补货恢复，无限库存（stock=0）不受影响
- **批量操作** — best-effort 模式，部分失败不回滚，返回汇总事件
- **治理接口** — Root 强制下架/删除、`ProductProvider` trait 供 order/governance/shop 调用

## 依赖关系

```
pallet-entity-product
├── pallet-entity-common
│   ├── EntityProvider    Entity 查询 + EntityLocked 检查 + Admin 权限
│   ├── ShopProvider      Shop 查询（exists, owner, account, active, manager, entity_id）
│   ├── PricingProvider   NEX/USDT 报价（get_nex_usdt_price, is_price_stale）
│   └── ProductProvider   trait 定义（供 order/governance 调用）
└── pallet-storage-service
    └── StoragePin         IPFS Pin/Unpin（SubjectType::Product）
```

> `ShopProvider::is_shop_active` 的 runtime 实现已隐式检查 Entity 存活性，商品操作无需额外调用 `EntityProvider` 验证。

## 押金机制

创建商品时从**店铺派生账户**扣取 **1 USDT 等值 NEX** 押金，转入 Pallet 托管账户 `PalletId(*b"et/prod/")`。删除时 best-effort 退还。

```
创建: Shop 派生账户 ──KeepAlive──→ Product Pallet 账户（押金）
删除: Product Pallet 账户 ──AllowDeath──→ Shop 派生账户（退还, best-effort）
```

- **创建**使用 `KeepAlive` — 确保店铺派生账户不被 reap，余额须 >= 押金 + ED
- **删除**使用 `AllowDeath` — Pallet 账户余额可清零
- 退还失败仅 `log::warn!`，不阻断删除
- 押金记录不存在时同样继续删除

### 押金计算

```
NEX 押金 = ProductDepositUsdt × 10^12 / nex_usdt_price
最终押金 = clamp(NEX 押金, MinProductDepositCos, MaxProductDepositCos)

特殊情况:
  价格过时 (is_price_stale) → 保守返回 MinProductDepositCos
  价格为零                  → PriceUnavailable 错误
  乘法溢出                  → ArithmeticOverflow 错误
```

## Config 配置

| 参数 | 类型 | 说明 | integrity_test |
|------|------|------|----------------|
| `Currency` | `Currency<AccountId>` | 链上货币 | — |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 查询/锁定/Admin | — |
| `ShopProvider` | `ShopProvider<AccountId>` | Shop 查询/权限 | — |
| `PricingProvider` | `PricingProvider` | NEX/USDT 报价 | — |
| `StoragePin` | `StoragePin<AccountId>` | IPFS Pin/Unpin | — |
| `MaxProductsPerShop` | `Get<u32>` | 每店铺最大商品数 | > 0 |
| `MaxCidLength` | `Get<u32>` | IPFS CID 最大字节数 | > 0 |
| `ProductDepositUsdt` | `Get<u64>` | 押金 USDT 金额（精度 10^6） | > 0 |
| `MinProductDepositCos` | `Get<Balance>` | 押金 NEX 下限 | <= Max |
| `MaxProductDepositCos` | `Get<Balance>` | 押金 NEX 上限 | >= Min |
| `MaxBatchSize` | `Get<u32>` | 批量操作最大数量 | > 0 |
| `MaxReasonLength` | `Get<u32>` | 强制下架/删除原因最大字节数 | > 0 |

## 数据结构

### Product

```rust
pub struct Product<Balance, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,
    pub shop_id: u64,
    pub name_cid: BoundedVec<u8, MaxCidLen>,     // 必填，IPFS pin
    pub images_cid: BoundedVec<u8, MaxCidLen>,    // 必填，IPFS pin
    pub detail_cid: BoundedVec<u8, MaxCidLen>,    // 必填，IPFS pin
    pub price: Balance,                           // 单价 NEX（> 0）
    pub usdt_price: u64,                          // USDT 价格（精度 10^6，0 = 未设置）
    pub stock: u32,                               // 库存（0 = 无限）
    pub sold_count: u32,                          // 累计已售
    pub status: ProductStatus,
    pub category: ProductCategory,
    pub sort_weight: u32,                         // 排序权重（越大越靠前）
    pub tags_cid: BoundedVec<u8, MaxCidLen>,      // 空 = 无标签，非空时 pin
    pub sku_cid: BoundedVec<u8, MaxCidLen>,       // 空 = 无 SKU，非空时 pin
    pub min_order_quantity: u32,                  // 0 = 不限
    pub max_order_quantity: u32,                  // 0 = 不限
    pub visibility: ProductVisibility,
    pub created_at: BlockNumber,
    pub updated_at: BlockNumber,
}
```

### ProductQueryInfo（pallet-entity-common）

```rust
pub struct ProductQueryInfo<Balance> {
    pub shop_id: u64,
    pub price: Balance,
    pub usdt_price: u64,
    pub stock: u32,
    pub status: ProductStatus,
    pub category: ProductCategory,
    pub visibility: ProductVisibility,
    pub min_order_quantity: u32,
    pub max_order_quantity: u32,
}
```

> `get_product_info` 单次 storage read 返回下单所需全部字段，替代 9 次独立查询。

### ProductDepositInfo

```rust
pub struct ProductDepositInfo<AccountId, Balance> {
    pub shop_id: u64,
    pub amount: Balance,
    pub source_account: AccountId,
}
```

### ProductStatistics

```rust
pub struct ProductStatistics {
    pub total_products: u64,
    pub on_sale_products: u64,
}
```

> `try_state` hook 在 try-runtime 模式下校验 `on_sale_products` 和 `total_products` 与实际存储一致性。

### 枚举类型（pallet-entity-common）

**ProductStatus**

| 状态 | 说明 |
|------|------|
| `Draft` | 草稿，初始状态 |
| `OnSale` | 在售 |
| `OffShelf` | 已下架 |
| `SoldOut` | 售罄（有限库存归零时自动设置） |

**ProductVisibility**

| 变体 | 说明 |
|------|------|
| `Public` | 所有用户可见 |
| `MembersOnly` | 仅会员可见 |
| `LevelGated(u8)` | 指定等级及以上会员可见 |

**ProductCategory**

| 类别 | 订单流程 | 状态 |
|------|----------|------|
| `Digital` | 支付即完成 | 可用 |
| `Physical` | 需发货 + 确认收货 | 可用 |
| `Service` | 需开始 + 完成 + 确认 | 可用 |
| `Other` | 同 Physical | 可用 |
| `Subscription` | — | 创建/更新时拒绝 |
| `Bundle` | — | 创建/更新时拒绝 |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextProductId` | `StorageValue<u64>` | 下一个商品 ID（自增，checked_add 防溢出） |
| `Products` | `StorageMap<u64, Product>` | 商品主表 |
| `ShopProducts` | `StorageMap<u64, BoundedVec<u64>>` | 店铺 → 商品 ID 索引（上限 MaxProductsPerShop） |
| `ProductStats` | `StorageValue<ProductStatistics>` | 全局统计（try_state 校验一致性） |
| `ProductDeposits` | `StorageMap<u64, ProductDepositInfo>` | 商品 → 押金记录 |

**StorageVersion**: 1

## Extrinsics

### 商家操作（Owner / Admin / Manager）

| # | 调用 | Weight | 说明 |
|---|------|--------|------|
| 0 | `create_product(shop_id, name_cid, images_cid, detail_cid, price, usdt_price, stock, category, sort_weight, tags_cid, sku_cid, min_order_quantity, max_order_quantity, visibility)` | 250M / 12K | 创建商品 + 扣押金 + pin CID |
| 1 | `update_product(product_id, [name_cid], [images_cid], [detail_cid], [price], [usdt_price], [stock], [category], [sort_weight], [tags_cid], [sku_cid], [min_order_quantity], [max_order_quantity], [visibility])` | 150M / 8K | 部分更新，CID 事务安全 pin/unpin |
| 2 | `publish_product(product_id)` | 120M / 6K | Draft/OffShelf → OnSale |
| 3 | `unpublish_product(product_id)` | 120M / 6K | OnSale/SoldOut → OffShelf |
| 4 | `delete_product(product_id)` | 200M / 10K | 删除 + 退押金 + unpin（Owner/Admin，Manager 不可） |
| 6 | `batch_publish_products(ids)` | 120M×N / 6K×N | 批量上架（best-effort） |
| 7 | `batch_unpublish_products(ids)` | 120M×N / 6K×N | 批量下架（best-effort） |
| 8 | `batch_delete_products(ids)` | 200M×N / 10K×N | 批量删除（Owner/Admin，best-effort） |

### Root 治理操作

| # | 调用 | Weight | 说明 |
|---|------|--------|------|
| 5 | `force_unpublish_product(product_id, reason?)` | 120M / 6K | 强制下架（OnSale/SoldOut → OffShelf） |
| 9 | `force_delete_product(product_id, reason?)` | 250M / 12K | 强制删除（任意状态，含退押金） |

### create_product 流程

1. `price > 0`、`name_cid`/`images_cid`/`detail_cid` 非空
2. 购买数量限制校验（`max > 0 && min > 0` 时须 `max >= min`）
3. 有限库存时 `stock >= min_order_quantity`（防止僵尸商品）
4. 拒绝 `Subscription` / `Bundle` 类别
5. Shop 存在 + 激活 + 调用者有操作权限
6. 商品数 < `MaxProductsPerShop`
7. CID 转 `BoundedVec`（长度校验）
8. 计算押金 + 余额校验（>= 押金 + ED）
9. `Currency::transfer(KeepAlive)` 扣押金
10. 创建商品（状态 `Draft`）→ Products / ShopProducts / ProductDeposits
11. `NextProductId` 自增（checked_add），`total_products` +1，`increment_product_count`
12. IPFS Pin: name + images + detail + 非空 tags/sku（以 shop_owner 为 owner）

### update_product 流程

1. 全部参数为 None 时拒绝（`NoChangesProvided`）
2. 权限检查 + Shop 激活校验（非活跃店铺禁止更新）
3. 在 `try_mutate` 闭包内收集 CID 变更（同值跳过），验证所有字段
4. `try_mutate` 成功后，在闭包外执行 unpin old + pin new（事务安全）
5. OnSale 状态禁止更改 category，category 更新拒绝 Subscription/Bundle
6. 补货（stock > 0 且 SoldOut → OnSale）需 Shop 激活
7. 更新 stock/min_order 时交叉校验 `stock >= min_order_quantity`（有限库存）

### delete_product 流程

1. 权限：Owner/Admin（**Manager 不可删除**，涉及押金退还）
2. 状态须为 `Draft` 或 `OffShelf`
3. best-effort 退还押金
4. IPFS Unpin: name + images + detail + 非空 tags/sku
5. 删除 Products、从 ShopProducts 移除、`decrement_product_count`、`total_products` -1

### force_delete_product 流程

1. `ensure_root` + reason 长度校验
2. **不限制商品状态**（可删 Draft / OffShelf / OnSale / SoldOut）
3. best-effort 退还押金（无记录时 warn log）
4. IPFS Unpin 全部 CID
5. 删除存储 + 更新统计（OnSale 时 `on_sale_products` -1）

### 批量操作规则

- 批次大小 <= `MaxBatchSize`，超出返回 `BatchTooLarge`
- **空列表短路返回**，不发射事件
- 逐项处理，失败项跳过并记录到 `failed_ids`
- 完成后发出 `BatchCompleted` 汇总事件

## 商品状态机

```
                                                  unpublish_product
create_product ──→ [Draft] ──→ publish ──→ [OnSale] ───────────────→ [OffShelf]
                                  ↑             │                        ↑
                                  │             │ deduct_stock           │
                                  │             │ (stock→0)             │
                                  │             ↓                        │
                                  │        [SoldOut] ─── unpublish ─────┘
                                  │             │
                                  │             │ restore_stock /
                                  │             │ update_product(补货) /
                                  │             │ set_inventory
                                  │             ↓
                                  └────────── [OnSale]
                                                ↑
                     [OffShelf] ─── publish ────┘

                [Draft] 或 [OffShelf] ──→ delete_product ──→ (已删除)

                [任意状态] ──→ force_delete_product (Root) ──→ (已删除)
```

### 状态转换约束

| 操作 | 允许的源状态 | 拒绝 |
|------|-------------|------|
| `publish_product` | Draft / OffShelf | OnSale / SoldOut |
| `unpublish_product` | OnSale / SoldOut | Draft / OffShelf |
| `delete_product` | Draft / OffShelf | OnSale / SoldOut |
| `force_delete_product` | 任意 | — |
| `force_unpublish_product` | OnSale / SoldOut | Draft / OffShelf |
| `deduct_stock` | OnSale | Draft / OffShelf / SoldOut |
| `restore_stock` | OnSale / OffShelf / SoldOut | Draft |
| `delist_product` | OnSale / SoldOut（其他静默 noop） | — |
| `force_remove_all_shop_products` | 任意 | — |
| `force_delist_all_shop_products` | OnSale / SoldOut（其他跳过） | — |

### on_sale_products 统计维护

| 路径 | 变化 |
|------|------|
| `publish_product` (Draft/OffShelf → OnSale) | +1 |
| `unpublish_product` (OnSale → OffShelf) | -1 |
| `unpublish_product` (SoldOut → OffShelf) | 不变 |
| `force_unpublish_product` (OnSale → OffShelf) | -1 |
| `force_unpublish_product` (SoldOut → OffShelf) | 不变 |
| `deduct_stock` (OnSale → SoldOut) | -1 |
| `restore_stock` (SoldOut → OnSale) | +1 |
| `restore_stock` (OffShelf, 仅增库存) | 不变 |
| `update_product` 补货 (SoldOut → OnSale) | +1 |
| `delist_product` (OnSale → OffShelf) | -1 |
| `delist_product` (SoldOut → OffShelf) | 不变 |
| `set_inventory` (OnSale → SoldOut) | -1 |
| `set_inventory` (SoldOut → OnSale) | +1 |
| `force_delete_product` (OnSale → 已删除) | -1 |
| `force_remove_all_shop_products` (OnSale → 已删除) | -N |
| `force_delist_all_shop_products` (OnSale → OffShelf) | -N |

## IPFS 集成

商品元数据 CID 通过 `StoragePin` trait（`pallet-storage-service`）持久化。所有 pin/unpin 操作统一以 **shop_owner** 作为 owner（fallback 到 shop_account），确保跨角色操作（Manager 创建 → Owner 删除、Root 强删）时 owner 一致，避免 CID 泄漏。

| 操作 | Pin | Unpin |
|------|-----|-------|
| `create_product` | name + images + detail + 非空 tags/sku（3–5 CID） | — |
| `update_product` | 变更且不同值的 CID | 被替换的旧 CID |
| `delete_product` | — | name + images + detail + 非空 tags/sku |
| `force_delete_product` | — | 同 delete_product |
| `batch_delete` | — | 同 delete_product |
| `force_remove_all_shop_products` | — | 店铺下全部商品的全部 CID |
| `force_unpin_shop_products` | — | 店铺下全部商品的全部 CID（仅 unpin，不删除商品） |

### update_product CID 事务安全

两阶段设计确保链上状态与 IPFS 状态一致：

1. **收集阶段**（`try_mutate` 闭包内）：比较新旧 CID，相同值跳过，不同值收集到 `to_unpin` / `to_pin`
2. **执行阶段**（`try_mutate` 成功后）：遍历向量执行 unpin/pin

若闭包内任何验证失败，存储回滚且不执行任何 pin/unpin。

### pin/unpin 参数

- **owner**: `shop_owner` (fallback `shop_account`)
- **subject_type**: `b"product"`
- **subject_id**: `product_id`
- **entity_id**: `ShopProvider::shop_entity_id(shop_id)`
- **tier**: `PinTier::Standard`
- **策略**: best-effort，失败仅 `log::warn!`

## 库存管理

| 场景 | 行为 |
|------|------|
| `stock = 0` 创建时 | 无限库存，`deduct_stock` 不扣减 |
| `deduct_stock` | 须 OnSale，扣到 0 自动 SoldOut（`on_sale_products` -1） |
| `restore_stock` | 拒绝 Draft；SoldOut 且 Shop 激活时自动恢复 OnSale；OffShelf 仅增库存；checked_add 防溢出 |
| `update_product` 补货 | stock > 0 且 SoldOut → OnSale（需 Shop 激活），OnSale 时不可设 stock=0 |
| `set_inventory` | 治理接口，直接设置库存并触发状态转换（SoldOut⇌OnSale） |

## ProductProvider Trait

供 `pallet-entity-order`、`pallet-entity-governance`、`pallet-entity-shop` 调用：

```rust
pub trait ProductProvider<AccountId, Balance> {
    // ── 单字段查询 ──
    fn product_exists(product_id: u64) -> bool;
    fn is_product_on_sale(product_id: u64) -> bool;
    fn product_shop_id(product_id: u64) -> Option<u64>;
    fn product_price(product_id: u64) -> Option<Balance>;
    fn product_usdt_price(product_id: u64) -> Option<u64>;
    fn product_stock(product_id: u64) -> Option<u32>;
    fn product_category(product_id: u64) -> Option<ProductCategory>;
    fn product_status(product_id: u64) -> Option<ProductStatus>;
    fn product_owner(product_id: u64) -> Option<AccountId>;
    fn product_visibility(product_id: u64) -> Option<ProductVisibility>;
    fn product_min_order_quantity(product_id: u64) -> Option<u32>;
    fn product_max_order_quantity(product_id: u64) -> Option<u32>;
    fn shop_product_ids(shop_id: u64) -> Vec<u64>;

    // ── 聚合查询（单次 storage read） ──
    fn get_product_info(product_id: u64) -> Option<ProductQueryInfo<Balance>>;

    // ── 库存操作 ──
    fn deduct_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn restore_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn add_sold_count(product_id: u64, quantity: u32) -> DispatchResult;

    // ── 治理操作 ──
    fn update_price(product_id: u64, new_price: Balance) -> DispatchResult;
    fn delist_product(product_id: u64) -> DispatchResult;
    fn set_inventory(product_id: u64, new_stock: u32) -> DispatchResult;

    // ── 店铺级操作 ──
    fn force_unpin_shop_products(shop_id: u64) -> DispatchResult;
    fn force_remove_all_shop_products(shop_id: u64) -> DispatchResult;
    fn force_delist_all_shop_products(shop_id: u64) -> DispatchResult;
}
```

## 权限模型

| 角色 | create | update | publish | unpublish | delete | force_unpublish | force_delete |
|------|--------|--------|---------|-----------|--------|-----------------|--------------|
| Owner | ✓ | ✓ | ✓ | ✓ | ✓ | — | — |
| Admin(SHOP_MANAGE) | ✓ | ✓ | ✓ | ✓ | ✓ | — | — |
| Manager | ✓ | ✓ | ✓ | ✓ | — | — | — |
| Root | — | — | — | — | — | ✓ | ✓ |

- **EntityLocked** 时所有 Owner/Admin/Manager 操作被拒绝
- **batch_\*** 操作权限与对应单项操作一致
- **ProductProvider trait** 方法为系统内部调用，无签名权限检查

### ensure_product_operator 逻辑

```
EntityLocked? → 拒绝
Owner?        → 通过
Admin(SHOP_MANAGE)? → 通过
allow_manager && Manager? → 通过
→ NotAuthorized
```

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ProductCreated` | `product_id, shop_id, deposit` | create_product |
| `ProductUpdated` | `product_id` | update_product, update_price |
| `ProductStatusChanged` | `product_id, status` | publish, unpublish, batch publish/unpublish, delist_product |
| `ProductDeleted` | `product_id, deposit_refunded` | delete_product, batch_delete |
| `StockUpdated` | `product_id, new_stock` | deduct_stock, restore_stock, set_inventory |
| `SoldCountUpdated` | `product_id, sold_count` | add_sold_count |
| `ProductForceUnpublished` | `product_id, reason` | force_unpublish_product |
| `ProductForceDeleted` | `product_id, deposit_refunded, reason` | force_delete_product |
| `BatchCompleted` | `operation, succeeded, failed, failed_ids` | batch_publish/unpublish/delete |
| `ShopProductsRemoved` | `shop_id, count, deposits_refunded` | force_remove_all_shop_products |
| `ShopProductsDelisted` | `shop_id, count` | force_delist_all_shop_products |

## Errors

| 错误 | 说明 |
|------|------|
| `ProductNotFound` | 商品不存在 |
| `ShopNotFound` | 店铺不存在 |
| `ShopNotActive` | 店铺未激活（暂停/封禁/关闭） |
| `InsufficientStock` | 库存不足（有限库存时 stock < quantity） |
| `MaxProductsReached` | 店铺商品数达到 MaxProductsPerShop |
| `InvalidProductStatus` | 当前状态不允许此操作 |
| `CidTooLong` | CID 超过 MaxCidLength |
| `InsufficientShopFund` | 店铺派生账户余额不足（< 押金 + ED） |
| `PriceUnavailable` | NEX/USDT 价格为 0 |
| `ArithmeticOverflow` | 押金计算溢出 / NextProductId 溢出 |
| `InvalidPrice` | 商品价格不能为 0 |
| `EmptyCid` | name/images/detail CID 不能为空 |
| `CannotClearStockWhileOnSale` | 在售商品不可将 stock 设为 0 |
| `StockOverflow` | restore_stock 溢出 u32 上限 |
| `NotAuthorized` | 无操作权限（非 Owner/Admin/Manager） |
| `BatchTooLarge` | 批次大小超过 MaxBatchSize |
| `ReasonTooLong` | 强制下架/删除原因超过 MaxReasonLength |
| `EntityLocked` | Entity 已被全局锁定 |
| `InvalidOrderQuantity` | max > 0 且 < min |
| `CategoryNotSupported` | 暂不支持的类别（Subscription/Bundle） |
| `NoChangesProvided` | update_product 未提供任何变更 |
| `MinOrderExceedsStock` | 有限库存时 min_order_quantity > stock |

## 内部函数

| 函数 | 说明 |
|------|------|
| `ensure_product_operator(who, shop_id, allow_manager)` | EntityLocked → Owner → Admin → Manager 权限检查链 |
| `pallet_account()` | 押金托管 Pallet 账户 |
| `calculate_product_deposit()` | 1 USDT 等值 NEX 押金（含 clamp + stale 兜底） |
| `pin_product_cid(shop_id, product_id, cid)` | IPFS Pin（shop_owner 为 owner，best-effort） |
| `unpin_product_cid(shop_id, cid)` | IPFS Unpin（shop_owner 为 owner，best-effort） |

## 测试

```bash
cargo test -p pallet-entity-product
# 166 tests
```

| 类别 | 数量 | 覆盖内容 |
|------|------|----------|
| create_product | 13 | 正常创建、零价格、空 CID、店铺校验、CID 过长、数量上限、无限库存、数量限制、可见性 |
| update_product | 12 | 部分更新、权限、零价格、空 CID、补货 SoldOut→OnSale、stock=0 防护、数量/可见性 |
| publish / unpublish | 9 | 正常上/下架、重复操作拒绝、SoldOut 下架、Shop 未激活拒绝 |
| delete_product | 6 | Draft/OffShelf 删除、OnSale/SoldOut 不可删、非店主、押金缺失 |
| force_unpublish | 3 | 正常强制下架、非 Root 拒绝、非 OnSale/SoldOut 拒绝 |
| force_delete | 8 | 任意状态删除、非 Root 拒绝、reason 过长、押金退还、unpin CID |
| 批量操作 | 9 | publish/unpublish/delete best-effort + 空列表短路 + 部分成功 |
| ProductProvider 查询 | 11 | 基本查询、可见性、购买数量、状态、owner、shop_product_ids、get_product_info |
| 库存操作 | 6 | deduct_stock/restore_stock 各状态行为 |
| 治理 delist / update_price / set_inventory | 12 | 状态转换、零价拒绝、SoldOut⇌OnSale、OffShelf noop |
| 押金机制 | 6 | 计算、clamp、零价格、退还、stale price、余额不足 |
| IPFS Pin | 14 | 创建 pin、更新 unpin+pin、删除 unpin、tags/sku 全流程、同值跳过、事务安全 |
| 权限模型 | 8 | Admin/Manager 各操作权限、无 SHOP_MANAGE 拒绝 |
| Entity 锁定 | 5 | 锁定后拒绝 create/update/publish/unpublish/delete |
| 购买数量 / 可见性 / 类别 | 12 | 数量限制校验、Subscription/Bundle 拒绝、OnSale category guard |
| 统计一致性 | 1 | 混合操作后 on_sale_products 精确验证 |
| 店铺联动 | 10 | force_remove_all / force_delist_all、update shop_active 拒绝、min_order > stock 拒绝 |
| 审计回归 | 14 | reason 校验、restore_stock 溢出、ShopProducts 索引、积分/库存/可见性边界 |

## 已知限制

- **Subscription / Bundle 类别**：`pallet-entity-common` 定义但本模块拒绝创建/更新，预留给未来扩展
- **押金金额固定**：创建时锁定，NEX 价格波动不影响已创建商品的押金数额
- **批量操作不保证顺序**：`batch_delete` 中个别失败不会阻止后续项处理

## 许可证

MIT License
