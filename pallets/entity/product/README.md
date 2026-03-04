# pallet-entity-product

> NEXUS 商品管理模块 — 商品生命周期管理、IPFS 元数据、USDT 等值押金机制

## 概述

`pallet-entity-product` 管理 Entity 商城系统中的商品全生命周期：创建（含押金）、更新、上架、下架、删除（退押金）、库存管理。商品元数据（名称、图片、详情）存储为 IPFS CID，链上仅记录引用。

## 架构依赖

```
pallet-entity-product
├── EntityProvider          Entity 查询（保留，当前由 ShopProvider::is_shop_active 隐式检查）
├── ShopProvider            Shop 查询（shop_exists, shop_owner, shop_account, is_shop_active）
├── PricingProvider         NEX/USDT 价格（get_nex_usdt_price，用于计算等值押金）
├── IpfsPinner              IPFS Pin 管理（pallet-storage-service 提供，商品 CID 持久化）
└── ProductProvider trait   供 pallet-entity-order 调用库存/价格/类别查询
```

> **注意**：`EntityProvider` 在 Config 中声明但当前未直接调用。`ShopProvider::is_shop_active` 的 runtime 实现中已隐式检查 Entity 状态（`is_entity_active`），因此无需重复检查。保留此关联类型供未来扩展使用。

## 押金机制

创建商品时从**店铺派生账户**扣取 **1 USDT 等值 NEX** 押金，转入 Pallet 托管账户 `PalletId(*b"et/prod/")`。删除商品时原路退还（押金记录必须存在，否则拒绝删除）。

```
创建商品:   Shop 派生账户 ──KeepAlive──→ Product Pallet 账户 (押金)
删除商品:   Product Pallet 账户 ──AllowDeath──→ Shop 派生账户 (退还，押金记录必须存在)
```

- 创建时使用 `ExistenceRequirement::KeepAlive` 防止 reap 店铺派生账户
- 删除时使用 `ExistenceRequirement::AllowDeath`（Pallet 账户可清零）
- 删除时押金记录缺失将返回 `DepositNotFound` 错误，防止静默跳过退款（v0.4.0）

### 押金计算公式

```
NEX 押金 = ProductDepositUsdt × 10^12 / cos_usdt_price
最终押金 = clamp(NEX 押金, MinProductDepositCos, MaxProductDepositCos)
```

## Config 配置

```rust
impl pallet_entity_product::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EntityProvider = EntityRegistry;       // 保留，当前未直接调用
    type ShopProvider = EntityShop;
    type PricingProvider = EntityPricingProvider;
    type MaxProductsPerShop = ConstU32<1000>;
    type MaxCidLength = ConstU32<64>;
    type ProductDepositUsdt = ConstU64<1_000_000>;       // 1 USDT (精度 10^6)
    type MinProductDepositCos = ConstU128<{ UNIT / 100 }>; // 0.01 NEX
    type MaxProductDepositCos = ConstU128<{ 10 * UNIT }>;  // 10 NEX
    type IpfsPinner = pallet_storage_service::Pallet<Runtime>; // IPFS Pin 管理
}
```

| 参数 | 说明 |
|------|------|
| `Currency` | 货币类型 |
| `EntityProvider` | Entity 查询接口（保留，由 ShopProvider 隐式使用） |
| `ShopProvider` | Shop 查询 + 派生账户 + 权限验证 |
| `PricingProvider` | NEX/USDT 实时价格（`get_nex_usdt_price()`） |
| `MaxProductsPerShop` | 每店铺最大商品数上限 |
| `MaxCidLength` | IPFS CID 最大字节数 |
| `ProductDepositUsdt` | 押金 USDT 金额（精度 10^6） |
| `MinProductDepositCos` | 押金 NEX 下限 |
| `MaxProductDepositCos` | 押金 NEX 上限 |
| `IpfsPinner` | IPFS Pin 管理接口（`pallet-storage-service` 提供，用于商品元数据 CID 持久化） |

## 数据结构

### Product

```rust
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct Product<Balance, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,
    pub shop_id: u64,
    pub name_cid: BoundedVec<u8, MaxCidLen>,
    pub images_cid: BoundedVec<u8, MaxCidLen>,
    pub detail_cid: BoundedVec<u8, MaxCidLen>,
    pub price: Balance,
    pub stock: u32,              // 0 = 无限库存
    pub sold_count: u32,
    pub status: ProductStatus,
    pub category: ProductCategory,
    pub min_order_quantity: u32,       // 最小购买数量（v0.7.0）
    pub max_order_quantity: u32,       // 最大购买数量，0=不限（v0.7.0）
    pub visibility: ProductVisibility, // 可见性控制（v0.7.0）
    pub created_at: BlockNumber,
    pub updated_at: BlockNumber,
}
```

### ProductDepositInfo

```rust
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct ProductDepositInfo<AccountId, Balance> {
    pub shop_id: u64,
    pub amount: Balance,
    pub source_account: AccountId,  // 店铺派生账户
}
```

### ProductStatistics

```rust
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct ProductStatistics {
    pub total_products: u64,
    pub on_sale_products: u64,
}
```

> 所有存储 struct 均已添加 `DecodeWithMemTracking`（v0.3.0 审查修复）。

### ProductStatus（定义于 pallet-entity-common）

| 状态 | 说明 |
|------|------|
| `Draft` | 草稿，未上架 |
| `OnSale` | 在售 |
| `OffShelf` | 已下架 |
| `SoldOut` | 售罄（stock 归零时自动设置） |

### ProductVisibility（定义于 pallet-entity-common，v0.7.0）

| 变体 | 说明 |
|------|------|
| `Public` | 对所有用户可见 |
| `MembersOnly` | 仅会员可见 |
| `LevelGated(u8)` | 仅指定等级及以上会员可见 |

### ProductCategory（定义于 pallet-entity-common）

| 类别 | 说明 | 订单流程 |
|------|------|----------|
| `Digital` | 数字/虚拟商品 | 支付即完成 |
| `Physical` | 实物商品 | 需发货+确认 |
| `Service` | 服务类 | 需开始+完成+确认 |
| `Other` | 其他 | 同 Physical |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextProductId` | `StorageValue<u64>` | 下一个商品 ID（自增） |
| `Products` | `StorageMap<u64, Product>` | 商品主表 |
| `ShopProducts` | `StorageMap<u64, BoundedVec<u64, MaxProductsPerShop>>` | 店铺→商品索引 |
| `ProductStats` | `StorageValue<ProductStatistics>` | 全局商品统计（on_sale_products 由所有状态变更路径同步维护） |
| `ProductDeposits` | `StorageMap<u64, ProductDepositInfo>` | 商品→押金记录（删除时必须存在） |

## Extrinsics

| # | 调用 | Weight | 权限 | 说明 |
|---|------|--------|------|------|
| 0 | `create_product(shop_id, name_cid, images_cid, detail_cid, price, usdt_price, stock, category, sort_weight, tags_cid, sku_cid, min_order_quantity, max_order_quantity, visibility)` | 250M / 12k | 店主/Admin/Manager | 创建商品，price>0，name_cid 非空，从店铺账户扣押金（KeepAlive） |
| 1 | `update_product(product_id, ...)` | 150M / 8k | 店主/Admin/Manager | 更新商品（所有字段可选，补货可恢复 SoldOut→OnSale，需 Shop 激活） |
| 2 | `publish_product(product_id)` | 120M / 6k | 店主/Admin/Manager | 上架（需 Shop 激活，状态须为 Draft/OffShelf） |
| 3 | `unpublish_product(product_id)` | 120M / 6k | 店主/Admin/Manager | 下架（状态须为 OnSale/SoldOut） |
| 4 | `delete_product(product_id)` | 200M / 10k | 店主/Admin | 删除商品并退还押金（状态须为 Draft/OffShelf，押金记录必须存在） |
| 5 | `force_unpublish_product(product_id)` | 120M / 6k | Root | 强制下架任意商品 |
| 6 | `batch_publish_products(product_ids)` | 变量 | 店主/Admin/Manager | 批量上架（best-effort，跳过失败项） |
| 7 | `batch_unpublish_products(product_ids)` | 变量 | 店主/Admin/Manager | 批量下架（best-effort，跳过失败项） |
| 8 | `batch_delete_products(product_ids)` | 变量 | 店主/Admin | 批量删除（best-effort，跳过失败项） |

### create_product 详细流程

1. 验证 `price > 0`（`InvalidPrice`）
2. 验证 `name_cid` 非空（`EmptyCid`）
3. 验证 Shop 存在且激活，调用者 == 店主
4. 检查店铺商品数 < `MaxProductsPerShop`
5. CID 转为 `BoundedVec`（校验长度，失败返回 `CidTooLong`）
6. `calculate_product_deposit()` 计算押金（PricingProvider 报价 + clamp）
7. 检查店铺派生账户余额 >= 押金
8. `Currency::transfer`（KeepAlive）从店铺账户转押金到 Pallet 账户
9. 创建商品（状态 `Draft`），写入 `Products` + `ShopProducts` + `ProductDeposits`
10. `NextProductId` 自增，更新 `ProductStats.total_products`
11. **IPFS Pin**: 固定 3 个 CID（name/images/detail）到存储网络（best-effort，失败不阻断创建）

### delete_product 详细流程

1. 验证调用者 == 店主
2. 状态必须为 `Draft` 或 `OffShelf`
3. 从 `ProductDeposits` 取出押金记录（`take` 同时删除），**记录不存在则返回 `DepositNotFound`**（v0.4.0）
4. `Currency::transfer`（AllowDeath）从 Pallet 账户退还押金到店铺派生账户
5. **IPFS Unpin**: 取消固定 3 个 CID（best-effort）
6. 删除 `Products`、从 `ShopProducts` 移除
7. 更新 `ProductStats`（total_products -1）

## 商品状态机

```
                                                    unpublish_product
create_product ──→ [Draft] ──→ publish_product ──→ [OnSale] ──────────────→ [OffShelf]
                                    ↑                 │                         ↑
                                    │                 │ deduct_stock            │
                                    │                 │ (stock→0)              │
                                    │                 ↓                         │
                                    │            [SoldOut] ─── unpublish ──────┘
                                    │                 │
                                    │                 │ restore_stock /
                                    │                 │ update_product(补货)
                                    │                 ↓
                                    └─────────── [OnSale]
                                                      ↑
                       [OffShelf] ─── publish_product ┘

                  [Draft] 或 [OffShelf] ──→ delete_product ──→ (已删除，退押金)
```

### 状态约束（v0.4.0 强化）

| 操作 | 允许的源状态 | 拒绝的状态 |
|------|-------------|-----------|
| `deduct_stock` | `OnSale` | Draft / OffShelf / SoldOut → `InvalidProductStatus` |
| `restore_stock` | `OnSale` / `OffShelf` / `SoldOut` | Draft → `InvalidProductStatus` |
| `publish_product` | `Draft` / `OffShelf` | OnSale / SoldOut → `InvalidProductStatus` |
| `unpublish_product` | `OnSale` / `SoldOut` | Draft / OffShelf → `InvalidProductStatus` |
| `delete_product` | `Draft` / `OffShelf` | OnSale / SoldOut → `InvalidProductStatus` |

### on_sale_products 统计同步

`ProductStats.on_sale_products` 由以下路径精确维护：

| 路径 | 统计变化 |
|------|----------|
| `publish_product` (Draft/OffShelf → OnSale) | +1 |
| `unpublish_product` (OnSale → OffShelf) | -1 |
| `unpublish_product` (SoldOut → OffShelf) | 不变（SoldOut 时已减过） |
| `deduct_stock` (OnSale → SoldOut, stock 归零) | -1 |
| `restore_stock` (SoldOut → OnSale) | +1 |
| `restore_stock` (OffShelf, 仅增加库存) | 不变（状态不变） |
| `update_product` 补货 (SoldOut → OnSale) | +1 |

## 库存管理

| 场景 | 行为 | 统计影响 |
|------|------|----------|
| `stock = 0` 创建时 | 无限库存，`deduct_stock` 不扣减 | 无 |
| `deduct_stock` | **须 OnSale 状态**，扣到 0 自动设为 `SoldOut` | `on_sale_products` -1 |
| `restore_stock` | **拒绝 Draft 状态**，从 SoldOut 自动恢复为 `OnSale`；OffShelf 仅增加库存不变状态 | SoldOut→OnSale 时 +1 |
| `update_product` 补货 | stock > 0 且 SoldOut → 恢复为 `OnSale`（需 Shop 激活） | `on_sale_products` +1 |

## ProductProvider Trait 实现

供 `pallet-entity-order` 调用：

```rust
impl ProductProvider<AccountId, Balance> for Pallet<T> {
    fn product_exists(product_id: u64) -> bool;
    fn is_product_on_sale(product_id: u64) -> bool;
    fn product_shop_id(product_id: u64) -> Option<u64>;
    fn product_price(product_id: u64) -> Option<Balance>;
    fn product_usdt_price(product_id: u64) -> Option<u64>;
    fn product_stock(product_id: u64) -> Option<u32>;
    fn product_category(product_id: u64) -> Option<ProductCategory>;
    fn product_visibility(product_id: u64) -> Option<ProductVisibility>;           // v0.7.0
    fn product_min_order_quantity(product_id: u64) -> Option<u32>;                 // v0.7.0
    fn product_max_order_quantity(product_id: u64) -> Option<u32>;                 // v0.7.0
    fn deduct_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn restore_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn add_sold_count(product_id: u64, quantity: u32) -> DispatchResult;
    fn delist_product(product_id: u64) -> DispatchResult;                          // 治理下架
    fn update_price(product_id: u64, new_price: Balance) -> DispatchResult;        // v0.7.0 治理
    fn set_inventory(product_id: u64, new_stock: u32) -> DispatchResult;           // v0.7.0 治理
}
```

### deduct_stock 内部逻辑（v0.4.0）

1. 校验商品存在
2. **校验 `status == OnSale`**（否则返回 `InvalidProductStatus`）
3. 若 `stock > 0`（有限库存）：校验 `stock >= quantity`，扣减库存
4. 若扣减后 `stock == 0`：自动设为 `SoldOut`，`on_sale_products` -1
5. 发出 `StockUpdated` 事件

### restore_stock 内部逻辑（v0.4.0）

1. 校验商品存在
2. **校验 `status != Draft`**（否则返回 `InvalidProductStatus`）
3. 若 `stock > 0` 或 `status == SoldOut`：增加库存
4. 若原状态为 `SoldOut`：恢复为 `OnSale`，`on_sale_products` +1
5. 发出 `StockUpdated` 事件

## 辅助函数

| 函数 | 说明 |
|------|------|
| `pallet_account()` | 返回押金托管 Pallet 账户 `PalletId(*b"et/prod/")` |
| `calculate_product_deposit()` | 计算当前 1 USDT 等值 NEX 押金（含 clamp） |
| `get_current_deposit()` | 供前端查询当前押金金额（调用 `calculate_product_deposit`） |
| `pin_product_cid()` | IPFS Pin 商品 CID（best-effort，失败仅记录日志） |
| `unpin_product_cid()` | IPFS Unpin 商品 CID（best-effort，失败仅记录日志） |

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ProductCreated` | `product_id`, `shop_id`, `deposit` | `create_product` |
| `ProductUpdated` | `product_id` | `update_product` |
| `ProductStatusChanged` | `product_id`, `status` | `publish_product` / `unpublish_product` |
| `ProductDeleted` | `product_id`, `deposit_refunded` | `delete_product` |
| `StockUpdated` | `product_id`, `new_stock` | `deduct_stock` / `restore_stock`（trait 内部） |
| `SoldCountUpdated` | `product_id`, `sold_count` | `add_sold_count`（trait 内部） |
| `BatchCompleted` | `operation`, `succeeded`, `failed`, `failed_ids` | 批量操作完成（v0.7.0，best-effort 模式） |

## Errors

| 错误 | 触发位置 | 说明 |
|------|----------|------|
| `ProductNotFound` | 所有需要商品的操作 | 商品不存在 |
| `ShopNotFound` | `create_product`, `publish_product` 等 | 店铺不存在 |
| `NotShopOwner` | 所有 extrinsic | 调用者不是店主 |
| `ShopNotActive` | `create_product`, `publish_product`, `update_product`(补货) | 店铺未激活（含 Entity 状态检查） |
| `InsufficientStock` | `deduct_stock` | 库存不足（有限库存时 stock < quantity） |
| `MaxProductsReached` | `create_product` | 店铺商品数达到上限 |
| `InvalidProductStatus` | `publish_product`, `unpublish_product`, `delete_product`, `deduct_stock`, `restore_stock` | 当前状态不允许此操作 |
| `CidTooLong` | `create_product`, `update_product` | CID 超过 `MaxCidLength` |
| `InsufficientShopFund` | `create_product` | 店铺派生账户余额不足以支付押金 |
| `DepositNotFound` | `delete_product` | 押金记录不存在（数据一致性保护） |
| `PriceUnavailable` | `calculate_product_deposit` | NEX/USDT 价格为 0 |
| `ArithmeticOverflow` | `calculate_product_deposit` | 押金计算溢出 |
| `InvalidPrice` | `create_product`, `update_product` | 商品价格不能为 0 |
| `EmptyCid` | `create_product`, `update_product` | CID 不能为空（name/images/detail） |
| `CannotClearStockWhileOnSale` | `update_product` | 在售商品不可将 stock 设为 0（stock=0 仅创建时表示无限库存） |
| `InvalidOrderQuantity` | `create_product`, `update_product` | max_order_quantity > 0 且 < min_order_quantity（v0.7.0） |
| `NotAuthorized` | 所有 extrinsic | 调用者无权限（非店主/Admin/Manager） |
| `EntityLocked` | 所有 extrinsic | Entity 已被全局锁定 |
| `StockOverflow` | `restore_stock` | 库存恢复溢出 u32 上限 |
| `BatchTooLarge` | 批量操作 | 批次大小超过 `MaxBatchSize` |

## 权限模型

| 操作 | 调用方 | 前置条件 |
|------|--------|----------|
| `create_product` | 店主/Admin(SHOP_MANAGE)/Manager | Shop 激活 + 商品数未满 + price > 0 + name_cid 非空 + CID 合法 + 运营资金充足 |
| `update_product` | 店主/Admin(SHOP_MANAGE)/Manager | price > 0（若更新）+ name_cid 非空（若更新）+ 补货需 Shop 激活 |
| `publish_product` | 店主/Admin(SHOP_MANAGE)/Manager | Shop 激活 + 状态为 Draft/OffShelf |
| `unpublish_product` | 店主/Admin(SHOP_MANAGE)/Manager | 状态为 OnSale/SoldOut |
| `delete_product` | 店主/Admin(SHOP_MANAGE) | 状态为 Draft/OffShelf + 押金记录存在 |
| `batch_*` | 同上 | best-effort 模式：跳过失败项，发出 BatchCompleted 汇总事件 |
| `deduct_stock` | 系统（trait 调用） | 状态须为 OnSale（v0.4.0），由 `pallet-entity-order` 下单时调用 |
| `restore_stock` | 系统（trait 调用） | 状态不可为 Draft（v0.4.0），由 `pallet-entity-order` 取消/退款/超时时调用 |
| `add_sold_count` | 系统（trait 调用） | 由 `pallet-entity-order` 订单完成时调用 |

## 测试

```bash
cargo test -p pallet-entity-product
# 117 tests (mock runtime + 单元测试)
```

### 测试覆盖

| 类别 | 测试数 | 覆盖内容 |
|------|--------|----------|
| `create_product` | 13 | 正常创建、零价格、空 CID（name/images/detail）、店铺不存在/未激活/非店主、CID 过长、数量上限、无限库存、**购买数量限制**、**可见性设置** |
| `update_product` | 12 | 部分更新、非店主、零价格、空 CID、补货 SoldOut→OnSale（含 Shop 激活检查）、stock=0 防护、**购买数量更新**、**可见性更新**、**数量限制校验** |
| `publish_product` | 5 | 正常上架、重复上架、SoldOut 不可上架、从 OffShelf 上架、Shop 未激活 |
| `unpublish_product` | 4 | 正常下架、Draft 不可下架、重复下架、SoldOut 下架 |
| `delete_product` | 6 | Draft 删除、OffShelf 删除、OnSale 不可删、SoldOut 不可删、非店主、押金记录缺失 |
| `ProductProvider` | 9 | 基本查询、扣库存、售罄、库存不足、无限库存扣减/恢复、销量累加、**可见性查询**、**购买数量查询** |
| **治理接口** | **6** | update_price 成功/拒绝零价/拒绝不存在、set_inventory 成功/归零→SoldOut/SoldOut恢复→OnSale |
| **批量操作** | **3** | batch_publish/unpublish/delete best-effort 部分成功 |
| 押金机制 | 6 | 押金计算、min/max clamp、价格为零、删除退还、stale price 返回 min |
| 统计一致性 | 1 | 多商品混合操作后 on_sale_products 精确验证 |
| ShopProducts 索引 | 1 | 创建+删除后索引正确 |
| 事件发出 | 3 | deduct_stock / restore_stock 发出 StockUpdated 事件、add_sold_count 发出 SoldCountUpdated 事件 |
| 审计回归 | 8 | deduct_stock 拒绝 Draft/OffShelf/SoldOut、restore_stock 拒绝 Draft、restore_stock OffShelf 仅增库存、SoldOut→OnSale 恢复、delete_product 押金缺失、OffShelf+stock=0 恢复库存 |
| IPFS Pin 集成 | 5 | 创建固定 3 CID、更新 unpin 旧+pin 新、未变 CID 不触发 pin、删除 unpin 全部、pin 失败不阻断创建 |
| **权限模型** | **8** | Admin(SHOP_MANAGE) 创建/更新/上下架、Manager 创建/上下架/不可删除、无 SHOP_MANAGE 权限拒绝 |
| **Entity 锁定** | **4** | 锁定后拒绝 create/update/publish/unpublish/delete |

## 审查修复清单

### v0.3.0 审查修复

| 编号 | 优先级 | 修复内容 |
|------|--------|----------|
| C1 | Critical | 创建 `mock.rs` + `tests.rs`（42 tests） |
| C2 | Critical | `Product`/`ProductStatistics`/`ProductDepositInfo` 添加 `DecodeWithMemTracking` |
| C3 | Critical | `publish_product` 添加源状态检查（仅 Draft/OffShelf → OnSale） |
| C4 | Critical | `unpublish_product` 添加源状态检查（仅 OnSale/SoldOut → OffShelf），精确统计 |
| H1 | High | `create_product` CID 校验移到 `Currency::transfer` 之前 |
| H3 | High | `EntityProvider` 未使用 — 确认由 `ShopProvider` 隐式检查，添加说明注释 |
| H4 | High | `create_product` 添加 `price > 0` 校验 + `InvalidPrice` 错误 |
| M1 | Medium | 5 个 extrinsic Weight 修正（120M-250M ref_time + 6k-12k proof_size） |
| M3 | Medium | `update_product` 补货 SoldOut→OnSale 时同步 `on_sale_products` |
| M4 | Medium | `deduct_stock`/`restore_stock` 状态变更时同步 `on_sale_products` |
| L2 | Low | `create_product` 押金转账 `AllowDeath` → `KeepAlive` |

### v0.4.0 审查修复

| 编号 | 优先级 | 修复内容 |
|------|--------|----------|
| M1 | Medium | `deduct_stock` 添加 `ensure!(status == OnSale)` 状态校验，防止对 Draft/OffShelf/SoldOut 商品扣库存导致统计错乱 |
| M2 | Medium | `restore_stock` 添加 `ensure!(status != Draft)` 防御性校验，防止对 Draft 商品静默修改库存 |
| M3 | Medium | `delete_product` 押金记录缺失时返回 `DepositNotFound` 错误，替代原有静默 fallback 为 0 的行为，保障数据一致性 |

### v0.5.0 审查修复

| 编号 | 优先级 | 修复内容 |
|------|--------|----------|
| H1 | High | `restore_stock` 条件扩展包含 OffShelf — 售罄后下架的商品（stock=0, OffShelf）订单取消时库存可正确恢复，之前静默丢弃 |
| H2 | High | `add_sold_count` 发出新的 `SoldCountUpdated` 事件替代语义错误的 `StockUpdated`（sold_count 变更却报告 stock 值） |
| M1 | Medium | `update_product` 在售商品禁止设置 stock=0（`CannotClearStockWhileOnSale`），防止有限库存隐式转为无限库存 |
| M2 | Medium | `create_product`/`update_product` 添加 `images_cid`/`detail_cid` 非空校验（与 `name_cid` 一致） |
| M3 | Medium | 补充 `is_price_stale()` 路径测试覆盖（Mock 添加 stale 状态） |
| L1 | Low | README 函数名 `get_cos_usdt_price` → `get_nex_usdt_price` |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，初始版本 |
| v0.2.0 | 2026-02-01 | 实现店铺派生账户押金机制 |
| v0.2.1 | 2026-02-05 | 重命名为 pallet-entity-product，适配 Entity-Shop 分离架构 |
| v0.3.0 | 2026-02-09 | 深度审查修复（11 项），创建 mock+tests（42 tests） |
| v0.4.0 | 2026-02-26 | 深度审查修复（3 项），强化 trait 接口防御性校验 + 押金一致性保护（56 tests） |
| v0.5.0 | 2026-03-02 | 深度审查修复（6 项）：H1 restore_stock OffShelf 恢复、H2 SoldCountUpdated 事件、M1 OnSale stock=0 防护、M2 空 CID 校验、M3 stale price 测试、L1 README typo（65 tests） |
| v0.6.0 | 2026-03-02 | IPFS Pin 集成：商品 CID 自动 pin/unpin（IpfsPinner trait + SubjectType::Product），创建 pin 3 CID、更新 unpin 旧+pin 新、删除 unpin 全部，best-effort 不阻断业务流程（70 tests） |
| v0.7.0 | 2026-03-04 | **生产环境必备功能**：购买数量限制（min/max_order_quantity）、可见性控制（ProductVisibility: Public/MembersOnly/LevelGated）、治理接口实现（update_price/set_inventory）、integrity_test 配置校验、批量操作 best-effort 模式（BatchCompleted 事件 + 部分成功报告）、STORAGE_VERSION 0→1（117 tests） |

## 许可证

MIT License
