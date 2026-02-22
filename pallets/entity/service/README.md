# pallet-entity-service

> NEXUS 商品管理模块 — 商品生命周期管理、IPFS 元数据、USDT 等值押金机制

## 概述

`pallet-entity-service` 管理 Entity 商城系统中的商品全生命周期：创建（含押金）、更新、上架、下架、删除（退押金）、库存管理。商品元数据（名称、图片、详情）存储为 IPFS CID，链上仅记录引用。

## 架构依赖

```
pallet-entity-service
├── EntityProvider          Entity 查询（保留，当前由 ShopProvider::is_shop_active 隐式检查）
├── ShopProvider            Shop 查询（shop_exists, shop_owner, shop_account, is_shop_active）
├── PricingProvider         NEX/USDT 价格（get_cos_usdt_price，用于计算等值押金）
└── ProductProvider trait   供 pallet-entity-transaction 调用库存/价格/类别查询
```

> **注意**：`EntityProvider` 在 Config 中声明但当前未直接调用。`ShopProvider::is_shop_active` 的 runtime 实现中已隐式检查 Entity 状态（`is_entity_active`），因此无需重复检查。保留此关联类型供未来扩展使用。

## 押金机制

创建商品时从**店铺派生账户**扣取 **1 USDT 等值 NEX** 押金，转入 Pallet 托管账户 `PalletId(*b"et/prod/")`。删除商品时原路退还。

```
创建商品:   Shop 派生账户 ──KeepAlive──→ Product Pallet 账户 (押金)
删除商品:   Product Pallet 账户 ──AllowDeath──→ Shop 派生账户 (退还)
```

- 创建时使用 `ExistenceRequirement::KeepAlive` 防止 reap 店铺派生账户
- 删除时使用 `ExistenceRequirement::AllowDeath`（Pallet 账户可清零）

### 押金计算公式

```
NEX 押金 = ProductDepositUsdt × 10^12 / cos_usdt_price
最终押金 = clamp(NEX 押金, MinProductDepositCos, MaxProductDepositCos)
```

## Config 配置

```rust
impl pallet_entity_service::Config for Runtime {
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
}
```

| 参数 | 说明 |
|------|------|
| `Currency` | 货币类型 |
| `EntityProvider` | Entity 查询接口（保留，由 ShopProvider 隐式使用） |
| `ShopProvider` | Shop 查询 + 派生账户 + 权限验证 |
| `PricingProvider` | NEX/USDT 实时价格（`get_cos_usdt_price()`） |
| `MaxProductsPerShop` | 每店铺最大商品数上限 |
| `MaxCidLength` | IPFS CID 最大字节数 |
| `ProductDepositUsdt` | 押金 USDT 金额（精度 10^6） |
| `MinProductDepositCos` | 押金 NEX 下限 |
| `MaxProductDepositCos` | 押金 NEX 上限 |

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
| `ProductDeposits` | `StorageMap<u64, ProductDepositInfo>` | 商品→押金记录 |

## Extrinsics

| # | 调用 | Weight | 权限 | 说明 |
|---|------|--------|------|------|
| 0 | `create_product(shop_id, name_cid, images_cid, detail_cid, price, stock, category)` | 250M / 12k | 店主 | 创建商品，price>0，从店铺账户扣押金（KeepAlive） |
| 1 | `update_product(product_id, name_cid?, images_cid?, detail_cid?, price?, stock?, category?)` | 150M / 8k | 店主 | 更新商品（所有字段可选，补货可恢复 SoldOut→OnSale） |
| 2 | `publish_product(product_id)` | 120M / 6k | 店主 | 上架（需 Shop 激活，状态须为 Draft/OffShelf） |
| 3 | `unpublish_product(product_id)` | 120M / 6k | 店主 | 下架（状态须为 OnSale/SoldOut） |
| 4 | `delete_product(product_id)` | 200M / 10k | 店主 | 删除商品并退还押金（状态须为 Draft/OffShelf） |

### create_product 详细流程

1. 验证 `price > 0`（`InvalidPrice`）
2. 验证 Shop 存在且激活，调用者 == 店主
3. 检查店铺商品数 < `MaxProductsPerShop`
4. CID 转为 `BoundedVec`（校验长度，失败返回 `CidTooLong`）
5. `calculate_product_deposit()` 计算押金（PricingProvider 报价 + clamp）
6. 检查店铺派生账户余额 >= 押金
7. `Currency::transfer`（KeepAlive）从店铺账户转押金到 Pallet 账户
8. 创建商品（状态 `Draft`），写入 `Products` + `ShopProducts` + `ProductDeposits`
9. `NextProductId` 自增，更新 `ProductStats.total_products`

### delete_product 详细流程

1. 验证调用者 == 店主
2. 状态必须为 `Draft` 或 `OffShelf`
3. 从 `ProductDeposits` 取出押金记录（`take` 同时删除）
4. `Currency::transfer`（AllowDeath）从 Pallet 账户退还押金到店铺派生账户
5. 删除 `Products`、从 `ShopProducts` 移除
6. 更新 `ProductStats`（total_products -1；若状态为 OnSale 则 on_sale_products -1）

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

### on_sale_products 统计同步

`ProductStats.on_sale_products` 由以下路径精确维护：

| 路径 | 统计变化 |
|------|----------|
| `publish_product` (Draft/OffShelf → OnSale) | +1 |
| `unpublish_product` (OnSale → OffShelf) | -1 |
| `unpublish_product` (SoldOut → OffShelf) | 不变（SoldOut 时已减过） |
| `deduct_stock` (OnSale → SoldOut) | -1 |
| `restore_stock` (SoldOut → OnSale) | +1 |
| `update_product` 补货 (SoldOut → OnSale) | +1 |
| `delete_product` (OnSale 状态删除) | -1（但实际不可达，删除需 Draft/OffShelf） |

## 库存管理

| 场景 | 行为 | 统计影响 |
|------|------|----------|
| `stock = 0` 创建时 | 无限库存，`deduct_stock` 不扣减 | 无 |
| `deduct_stock` 扣到 0 | 自动设为 `SoldOut` | `on_sale_products` -1 |
| `restore_stock` 从 SoldOut | 自动恢复为 `OnSale` | `on_sale_products` +1 |
| `update_product` 补货 | stock > 0 且 SoldOut → 恢复为 `OnSale` | `on_sale_products` +1 |

## ProductProvider Trait 实现

供 `pallet-entity-transaction` 调用：

```rust
impl ProductProvider<AccountId, Balance> for Pallet<T> {
    fn product_exists(product_id: u64) -> bool;
    fn is_product_on_sale(product_id: u64) -> bool;
    fn product_shop_id(product_id: u64) -> Option<u64>;
    fn product_price(product_id: u64) -> Option<Balance>;
    fn product_stock(product_id: u64) -> Option<u32>;
    fn product_category(product_id: u64) -> Option<ProductCategory>;
    fn deduct_stock(product_id: u64, quantity: u32) -> DispatchResult;   // 同步 on_sale 统计
    fn restore_stock(product_id: u64, quantity: u32) -> DispatchResult;  // 同步 on_sale 统计
    fn add_sold_count(product_id: u64, quantity: u32) -> DispatchResult;
}
```

## 辅助函数

| 函数 | 说明 |
|------|------|
| `pallet_account()` | 返回押金托管 Pallet 账户 `PalletId(*b"et/prod/")` |
| `calculate_product_deposit()` | 计算当前 1 USDT 等值 NEX 押金（含 clamp） |
| `get_current_deposit()` | 供前端查询当前押金金额（调用 `calculate_product_deposit`） |

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ProductCreated` | `product_id`, `shop_id`, `deposit` | `create_product` |
| `ProductUpdated` | `product_id` | `update_product` |
| `ProductStatusChanged` | `product_id`, `status` | `publish_product` / `unpublish_product` |
| `ProductDeleted` | `product_id`, `deposit_refunded` | `delete_product` |
| `StockUpdated` | `product_id`, `new_stock` | `deduct_stock` / `restore_stock`（trait 内部） |

## Errors

| 错误 | 触发位置 | 说明 |
|------|----------|------|
| `ProductNotFound` | 所有需要商品的操作 | 商品不存在 |
| `ShopNotFound` | `create_product`, `publish_product` 等 | 店铺不存在 |
| `NotShopOwner` | 所有 extrinsic | 调用者不是店主 |
| `ShopNotActive` | `create_product`, `publish_product` | 店铺未激活（含 Entity 状态检查） |
| `InsufficientStock` | `deduct_stock` | 库存不足 |
| `MaxProductsReached` | `create_product` | 店铺商品数达到上限 |
| `InvalidProductStatus` | `publish_product`, `unpublish_product`, `delete_product` | 状态不允许此操作 |
| `CidTooLong` | `create_product`, `update_product` | CID 超过 `MaxCidLength` |
| `InsufficientShopFund` | `create_product` | 店铺派生账户余额不足以支付押金 |
| `DepositNotFound` | `delete_product`（未使用，代码中 fallback 为 0） | 押金记录不存在 |
| `PriceUnavailable` | `calculate_product_deposit` | NEX/USDT 价格为 0 |
| `ArithmeticOverflow` | `calculate_product_deposit` | 押金计算溢出 |
| `InvalidPrice` | `create_product` | 商品价格不能为 0 |

## 权限模型

| 操作 | 调用方 | 前置条件 |
|------|--------|----------|
| `create_product` | 店主（signed） | Shop 激活 + 商品数未满 + price > 0 + CID 合法 + 运营资金充足 |
| `update_product` | 店主（signed） | — |
| `publish_product` | 店主（signed） | Shop 激活 + 状态为 Draft/OffShelf |
| `unpublish_product` | 店主（signed） | 状态为 OnSale/SoldOut |
| `delete_product` | 店主（signed） | 状态为 Draft/OffShelf |
| `deduct_stock` | 系统（trait 调用） | 由 `pallet-entity-transaction` 下单时调用 |
| `restore_stock` | 系统（trait 调用） | 由 `pallet-entity-transaction` 取消/退款时调用 |
| `add_sold_count` | 系统（trait 调用） | 由 `pallet-entity-transaction` 订单完成时调用 |

## 测试

```bash
cargo test -p pallet-entity-service
# 42 tests (mock runtime + 单元测试)
```

### 测试覆盖

| 类别 | 测试数 | 覆盖内容 |
|------|--------|----------|
| `create_product` | 8 | 正常创建、零价格、店铺不存在/未激活/非店主、CID 过长、数量上限、无限库存 |
| `update_product` | 3 | 部分更新、非店主、补货 SoldOut→OnSale 统计 |
| `publish_product` | 5 | 正常上架、重复上架、SoldOut 不可上架、从 OffShelf 上架、Shop 未激活 |
| `unpublish_product` | 4 | 正常下架、Draft 不可下架、重复下架、SoldOut 下架 |
| `delete_product` | 5 | Draft 删除、OffShelf 删除、OnSale 不可删、SoldOut 不可删、非店主 |
| `ProductProvider` | 7 | 基本查询、扣库存、售罄、库存不足、无限库存扣减/恢复、销量累加 |
| 押金机制 | 5 | 押金计算、min/max clamp、价格为零、删除退还 |
| 统计一致性 | 1 | 多商品混合操作后 on_sale_products 精确验证 |
| ShopProducts 索引 | 1 | 创建+删除后索引正确 |

## v0.3.0 审查修复清单

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

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，初始版本 |
| v0.2.0 | 2026-02-01 | 实现店铺派生账户押金机制 |
| v0.2.1 | 2026-02-05 | 重命名为 pallet-entity-service，适配 Entity-Shop 分离架构 |
| v0.3.0 | 2026-02-09 | 深度审查修复（11 项），创建 mock+tests（42 tests） |

## 许可证

MIT License
