# pallet-entity-product

> NEXUS 商品管理模块 — 商品全生命周期、IPFS 元数据持久化、USDT 等值押金机制

## 概述

`pallet-entity-product` 管理 Entity 商城系统中商品的完整生命周期：

- **创建** — 从店铺派生账户扣取 USDT 等值 NEX 押金，元数据以 IPFS CID 存储
- **更新** — 部分字段可选更新，CID 变更时自动 unpin 旧值 + pin 新值
- **上架 / 下架** — 状态机驱动，统计计数器精确同步
- **删除** — 退还押金（best-effort），IPFS unpin 全部 CID（含 tags/sku）
- **库存管理** — 有限库存自动售罄 / 补货恢复，无限库存（stock=0）不受影响
- **批量操作** — best-effort 模式，部分失败不回滚，返回汇总事件
- **治理接口** — Root 强制下架、`ProductProvider` trait 供 order/governance 调用

## 架构依赖

```
pallet-entity-product
├── EntityProvider          Entity 查询 + EntityLocked 检查 + Admin 权限验证
├── ShopProvider            Shop 查询（exists, owner, account, active, manager, entity_id）
├── PricingProvider         NEX/USDT 价格（get_nex_usdt_price, is_price_stale）
├── IpfsPinner              IPFS Pin/Unpin（pallet-storage-service, SubjectType::Product）
└── ProductProvider trait   供 pallet-entity-order / governance 调用
```

> **注意**：`EntityProvider` 主要用于 `is_entity_locked` 检查和 `is_entity_admin` 权限验证。`ShopProvider::is_shop_active` 的 runtime 实现已隐式检查 Entity 存活性，因此商品操作无需额外检查。

## 押金机制

创建商品时从**店铺派生账户**扣取 **1 USDT 等值 NEX** 押金，转入 Pallet 托管账户 `PalletId(*b"et/prod/")`。删除商品时 best-effort 退还。

```
创建: Shop 派生账户 ──KeepAlive──→ Product Pallet 账户（押金）
删除: Product Pallet 账户 ──AllowDeath──→ Shop 派生账户（退还, best-effort）
```

- **创建**使用 `KeepAlive` — 确保店铺派生账户不被 reap，余额须 ≥ 押金 + ED
- **删除**使用 `AllowDeath` — Pallet 账户余额可清零
- 押金退还失败（Pallet 偿付能力不足）时仅记录日志，不阻断删除
- 押金记录不存在时同样继续删除，记录警告日志

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

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<...>;
    type Currency: Currency<Self::AccountId>;
    type EntityProvider: EntityProvider<Self::AccountId>;
    type ShopProvider: ShopProvider<Self::AccountId>;
    type PricingProvider: PricingProvider;
    type MaxProductsPerShop: Get<u32>;
    type MaxCidLength: Get<u32>;
    type ProductDepositUsdt: Get<u64>;
    type MinProductDepositCos: Get<BalanceOf<Self>>;
    type MaxProductDepositCos: Get<BalanceOf<Self>>;
    type IpfsPinner: IpfsPinner<Self::AccountId, BalanceOf<Self>>;
    type MaxBatchSize: Get<u32>;
    type MaxReasonLength: Get<u32>;
}
```

| 参数 | 说明 | integrity_test |
|------|------|----------------|
| `MaxProductsPerShop` | 每店铺最大商品数 | > 0 |
| `MaxCidLength` | IPFS CID 最大字节数 | > 0 |
| `ProductDepositUsdt` | 押金 USDT 金额（精度 10^6，1_000_000 = 1 USDT） | > 0 |
| `MinProductDepositCos` | 押金 NEX 下限 | ≤ Max |
| `MaxProductDepositCos` | 押金 NEX 上限 | ≥ Min |
| `MaxBatchSize` | 批量操作最大数量 | > 0 |
| `MaxReasonLength` | 强制下架原因最大字节数 | > 0 |

## 数据结构

### Product

```rust
pub struct Product<Balance, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,                                    // 商品 ID
    pub shop_id: u64,                               // 所属店铺 ID
    pub name_cid: BoundedVec<u8, MaxCidLen>,        // 名称 IPFS CID（必填，pin）
    pub images_cid: BoundedVec<u8, MaxCidLen>,      // 图片 IPFS CID（必填，pin）
    pub detail_cid: BoundedVec<u8, MaxCidLen>,      // 详情 IPFS CID（必填，pin）
    pub price: Balance,                             // 单价 NEX（> 0）
    pub usdt_price: u64,                            // USDT 价格（精度 10^6，0 = 未设置）
    pub stock: u32,                                 // 库存（0 = 无限）
    pub sold_count: u32,                            // 累计已售
    pub status: ProductStatus,                      // 商品状态
    pub category: ProductCategory,                  // 商品类别
    pub sort_weight: u32,                           // 排序权重（越大越靠前）
    pub tags_cid: BoundedVec<u8, MaxCidLen>,        // 标签 CID（空 = 无标签，不 pin）
    pub sku_cid: BoundedVec<u8, MaxCidLen>,         // SKU 变体 CID（空 = 无 SKU，不 pin）
    pub min_order_quantity: u32,                    // 最小购买数量（0 = 不限）
    pub max_order_quantity: u32,                    // 最大购买数量（0 = 不限）
    pub visibility: ProductVisibility,              // 可见性控制
    pub created_at: BlockNumber,
    pub updated_at: BlockNumber,
}
```

### ProductDepositInfo

```rust
pub struct ProductDepositInfo<AccountId, Balance> {
    pub shop_id: u64,
    pub amount: Balance,
    pub source_account: AccountId,  // 店铺派生账户
}
```

### ProductStatistics

```rust
pub struct ProductStatistics {
    pub total_products: u64,
    pub on_sale_products: u64,
}
```

### BatchOperation

```rust
pub enum BatchOperation { Publish, Unpublish, Delete }
```

> 所有存储 struct 均已添加 `DecodeWithMemTracking`。

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

| 类别 | 订单流程 |
|------|----------|
| `Digital` | 支付即完成 |
| `Physical` | 需发货 + 确认收货 |
| `Service` | 需开始 + 完成 + 确认 |
| `Other` | 同 Physical |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextProductId` | `StorageValue<u64>` | 下一个商品 ID（自增，checked_add 防溢出） |
| `Products` | `StorageMap<u64, Product>` | 商品主表 |
| `ShopProducts` | `StorageMap<u64, BoundedVec<u64, MaxProductsPerShop>>` | 店铺 → 商品 ID 索引 |
| `ProductStats` | `StorageValue<ProductStatistics>` | 全局统计（try_state 校验一致性） |
| `ProductDeposits` | `StorageMap<u64, ProductDepositInfo>` | 商品 → 押金记录 |

**StorageVersion**: 1

## Extrinsics

| # | 调用 | Weight | 权限 | 说明 |
|---|------|--------|------|------|
| 0 | `create_product(...)` | 250M / 12k | Owner/Admin/Manager | 创建商品 + 扣押金 + pin CID |
| 1 | `update_product(product_id, ...)` | 150M / 8k | Owner/Admin/Manager | 部分更新，CID 变更自动 pin/unpin |
| 2 | `publish_product(product_id)` | 120M / 6k | Owner/Admin/Manager | Draft/OffShelf → OnSale |
| 3 | `unpublish_product(product_id)` | 120M / 6k | Owner/Admin/Manager | OnSale/SoldOut → OffShelf |
| 4 | `delete_product(product_id)` | 200M / 10k | Owner/Admin | 删除 + 退押金 + unpin 全部 CID |
| 5 | `force_unpublish_product(product_id, reason)` | 120M / 6k | Root | 强制下架（OnSale/SoldOut → OffShelf） |
| 6 | `batch_publish_products(ids)` | 120M×N / 6k×N | Owner/Admin/Manager | 批量上架（best-effort） |
| 7 | `batch_unpublish_products(ids)` | 120M×N / 6k×N | Owner/Admin/Manager | 批量下架（best-effort） |
| 8 | `batch_delete_products(ids)` | 200M×N / 10k×N | Owner/Admin | 批量删除（best-effort） |

### create_product 参数

```
shop_id, name_cid, images_cid, detail_cid, price, usdt_price, stock,
category, sort_weight, tags_cid, sku_cid, min_order_quantity, max_order_quantity, visibility
```

### create_product 流程

1. 校验 `price > 0`、`name_cid`/`images_cid`/`detail_cid` 非空
2. 校验购买数量限制（`max > 0 && min > 0` 时须 `max >= min`）
3. 验证 Shop 存在 + 激活，调用者有操作权限
4. 检查商品数 < `MaxProductsPerShop`
5. CID 转为 `BoundedVec`（长度校验）
6. 计算押金（`calculate_product_deposit`）
7. 校验店铺派生账户余额 ≥ 押金 + ED
8. `Currency::transfer(KeepAlive)` 扣押金
9. 创建商品（状态 `Draft`），写入 Products / ShopProducts / ProductDeposits
10. `NextProductId` 自增，`total_products` +1，`increment_product_count`
11. IPFS Pin：固定 name/images/detail 3 个 CID（best-effort）

### delete_product 流程

1. 校验权限（Owner/Admin，**Manager 不可删除**）
2. 状态须为 `Draft` 或 `OffShelf`
3. 取出押金记录 + best-effort 退还
4. IPFS Unpin：name/images/detail + tags/sku（非空时）共最多 5 个 CID
5. 删除 Products，从 ShopProducts 移除，`decrement_product_count`
6. `total_products` -1

### force_unpublish_product 流程

1. `ensure_root` 验证 Root 权限
2. **先校验 `reason` 长度**（`ReasonTooLong`），避免状态变更后回滚
3. 状态须为 OnSale 或 SoldOut
4. 设为 OffShelf，OnSale 时 `on_sale_products` -1
5. 发出 `ProductForceUnpublished` 事件（含 reason）

### 批量操作规则

- 批次大小 ≤ `MaxBatchSize`，超出返回 `BatchTooLarge`
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

                [Draft] 或 [OffShelf] ──→ delete_product ──→ (已删除，退押金)
```

### 状态约束

| 操作 | 允许的源状态 | 拒绝 |
|------|-------------|------|
| `publish_product` | Draft / OffShelf | OnSale / SoldOut |
| `unpublish_product` | OnSale / SoldOut | Draft / OffShelf |
| `delete_product` | Draft / OffShelf | OnSale / SoldOut |
| `force_unpublish_product` | OnSale / SoldOut | Draft / OffShelf |
| `deduct_stock` | OnSale | Draft / OffShelf / SoldOut |
| `restore_stock` | OnSale / OffShelf / SoldOut | Draft |
| `delist_product` | OnSale / SoldOut（其他静默 noop） | — |

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

> `try_state` hook 在 try-runtime 模式下校验 `on_sale_products` 和 `total_products` 与实际存储一致性。

## 库存管理

| 场景 | 行为 |
|------|------|
| `stock = 0` 创建时 | 无限库存，`deduct_stock` 不扣减 |
| `deduct_stock` | 须 OnSale，扣到 0 自动 SoldOut |
| `restore_stock` | 拒绝 Draft；SoldOut 且 Shop 激活时自动恢复 OnSale；OffShelf 仅增库存；checked_add 防溢出 |
| `update_product` 补货 | stock > 0 且 SoldOut → OnSale（需 Shop 激活） |
| `set_inventory` | 治理接口，可直接设置库存并触发状态转换 |

## ProductProvider Trait

供 `pallet-entity-order` 及治理模块调用：

```rust
impl ProductProvider<AccountId, Balance> for Pallet<T> {
    // 查询
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

    // 库存操作（由 order 模块调用）
    fn deduct_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn restore_stock(product_id: u64, quantity: u32) -> DispatchResult;
    fn add_sold_count(product_id: u64, quantity: u32) -> DispatchResult;

    // 治理操作
    fn update_price(product_id: u64, new_price: Balance) -> DispatchResult;
    fn delist_product(product_id: u64) -> DispatchResult;
    fn set_inventory(product_id: u64, new_stock: u32) -> DispatchResult;
}
```

## IPFS 集成

商品元数据 CID 通过 `IpfsPinner` trait（`pallet-storage-service`）持久化：

| 操作 | Pin | Unpin |
|------|-----|-------|
| `create_product` | name + images + detail（3 CID） | — |
| `update_product` | 变更的 CID pin 新值 | 变更的 CID unpin 旧值 |
| `delete_product` | — | name + images + detail + tags + sku（非空，最多 5 CID） |
| `batch_delete` | — | 同 delete_product |

- **best-effort**：pin/unpin 失败仅记录日志（`log::warn!`），不阻断业务
- tags_cid / sku_cid 创建时不 pin（可选字段），删除时 unpin（如非空）
- Pin 参数：`SubjectType::Product`，`PinTier::Standard`

## 辅助函数

| 函数 | 说明 |
|------|------|
| `ensure_product_operator(who, shop_id, allow_manager)` | 权限检查：EntityLocked → Owner → Admin(SHOP_MANAGE) → Manager（可选） |
| `pallet_account()` | 返回押金托管 Pallet 账户 |
| `calculate_product_deposit()` | 计算当前 1 USDT 等值 NEX 押金（含 clamp + stale 兜底） |
| `get_current_deposit()` | 供前端查询当前押金金额 |
| `pin_product_cid(caller, product_id, cid)` | IPFS Pin（best-effort） |
| `unpin_product_cid(caller, cid)` | IPFS Unpin（best-effort） |

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ProductCreated` | `product_id, shop_id, deposit` | `create_product` |
| `ProductUpdated` | `product_id` | `update_product`, `update_price` |
| `ProductStatusChanged` | `product_id, status` | `publish_product`, `unpublish_product`, batch publish/unpublish |
| `ProductDeleted` | `product_id, deposit_refunded` | `delete_product`, `batch_delete` |
| `StockUpdated` | `product_id, new_stock` | `deduct_stock`, `restore_stock`, `set_inventory` |
| `SoldCountUpdated` | `product_id, sold_count` | `add_sold_count` |
| `ProductForceUnpublished` | `product_id, reason: Option<BoundedVec>` | `force_unpublish_product` |
| `BatchCompleted` | `operation, succeeded, failed, failed_ids` | batch_publish/unpublish/delete |

## Errors

| 错误 | 说明 |
|------|------|
| `ProductNotFound` | 商品不存在 |
| `ShopNotFound` | 店铺不存在 |
| `NotShopOwner` | 调用者不是店主 |
| `ShopNotActive` | 店铺未激活 |
| `InsufficientStock` | 库存不足（有限库存时 stock < quantity） |
| `MaxProductsReached` | 店铺商品数达到上限 |
| `InvalidProductStatus` | 当前状态不允许此操作 |
| `CidTooLong` | CID 超过 MaxCidLength |
| `InsufficientShopFund` | 店铺派生账户余额不足（< 押金 + ED） |
| `DepositNotFound` | 押金记录不存在 |
| `PriceUnavailable` | NEX/USDT 价格为 0 |
| `ArithmeticOverflow` | 押金计算溢出 / NextProductId 溢出 |
| `InvalidPrice` | 商品价格不能为 0 |
| `EmptyCid` | name/images/detail CID 不能为空 |
| `CannotClearStockWhileOnSale` | 在售商品不可将 stock 设为 0 |
| `StockOverflow` | restore_stock 溢出 u32 上限 |
| `NotAuthorized` | 无操作权限（非 Owner/Admin/Manager） |
| `EntityLocked` | Entity 已被全局锁定 |
| `InvalidOrderQuantity` | max > 0 且 < min |
| `BatchTooLarge` | 批次大小超过 MaxBatchSize |
| `ReasonTooLong` | 强制下架原因超过 MaxReasonLength |

## 权限模型

| 角色 | create | update | publish | unpublish | delete | force_unpublish |
|------|--------|--------|---------|-----------|--------|-----------------|
| Owner | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Admin(SHOP_MANAGE) | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Manager | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| Root | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

- **EntityLocked** 时所有 Owner/Admin/Manager 操作被拒绝
- **batch_\*** 操作权限与对应单项操作一致
- **ProductProvider trait** 方法为系统内部调用，无签名权限检查

## 测试

```bash
cargo test -p pallet-entity-product
# 129 tests
```

### 测试覆盖

| 类别 | 数量 | 覆盖内容 |
|------|------|----------|
| create_product | 13 | 正常创建、零价格、空 CID、店铺校验、CID 过长、数量上限、无限库存、购买数量限制、可见性 |
| update_product | 12 | 部分更新、权限、零价格、空 CID、补货 SoldOut→OnSale、stock=0 防护、数量/可见性更新 |
| publish_product | 5 | 正常上架、重复上架、SoldOut 不可上架、OffShelf 上架、Shop 未激活 |
| unpublish_product | 4 | 正常下架、Draft 不可下架、重复下架、SoldOut 下架 |
| delete_product | 6 | Draft/OffShelf 删除、OnSale/SoldOut 不可删、非店主、押金缺失 |
| force_unpublish | 3 | 正常强制下架、非 Root 拒绝、非 OnSale 拒绝 |
| 治理 delist | 4 | OnSale/SoldOut/Draft 下架行为、统计精确性 |
| 治理 update_price | 3 | 成功、拒绝零价、拒绝不存在 |
| 治理 set_inventory | 3 | 设置库存、归零→SoldOut、SoldOut→OnSale 恢复 |
| ProductProvider 查询 | 9 | 基本查询、可见性、购买数量、状态、owner、shop_product_ids |
| 库存操作 | 6 | deduct_stock/restore_stock 各状态行为 |
| 批量操作 | 9 | publish/unpublish/delete best-effort + 空列表短路 + 部分成功 |
| 押金机制 | 6 | 计算、clamp、零价格、退还、stale price、余额不足 |
| 统计一致性 | 1 | 混合操作后 on_sale_products 精确验证 |
| IPFS Pin 集成 | 8 | 创建 pin、更新 unpin+pin、删除 unpin 全部（含 tags/sku）、pin 失败不阻断 |
| 权限模型 | 8 | Admin/Manager 各操作权限、无 SHOP_MANAGE 拒绝 |
| Entity 锁定 | 5 | 锁定后拒绝 create/update/publish/unpublish/delete |
| 购买数量 | 5 | 创建/更新数量限制、无效校验、零 max 不限 |
| 可见性 | 3 | Public/MembersOnly/LevelGated 创建和更新 |
| 审计回归 M1 | 2 | force_unpublish reason 过长拒绝（状态未变）、恰好 max 长度成功 |
| 其他回归 | 12 | deduct_stock 状态校验、restore_stock 状态校验/溢出、OffShelf 恢复、ShopProducts 索引 |

## 审查修复清单

### v0.3.0

| # | 级别 | 修复 |
|---|------|------|
| C1 | Critical | 创建 mock.rs + tests.rs（42 tests） |
| C2 | Critical | 所有存储 struct 添加 `DecodeWithMemTracking` |
| C3 | Critical | `publish_product` 添加源状态检查（仅 Draft/OffShelf） |
| C4 | Critical | `unpublish_product` 添加源状态检查（仅 OnSale/SoldOut），精确统计 |
| H1 | High | `create_product` CID 校验移到 `Currency::transfer` 之前 |
| H3 | High | `EntityProvider` 未使用 — 确认由 ShopProvider 隐式检查 |
| H4 | High | `create_product` 添加 `price > 0` 校验 |
| M1 | Medium | 5 个 extrinsic Weight 修正 |
| M3 | Medium | `update_product` 补货 SoldOut→OnSale 同步 on_sale_products |
| M4 | Medium | `deduct_stock`/`restore_stock` 状态变更同步 on_sale_products |
| L2 | Low | 创建押金转账 AllowDeath → KeepAlive |

### v0.4.0

| # | 级别 | 修复 |
|---|------|------|
| M1 | Medium | `deduct_stock` 添加 `status == OnSale` 校验 |
| M2 | Medium | `restore_stock` 添加 `status != Draft` 校验 |
| M3 | Medium | `delete_product` 押金记录缺失返回 `DepositNotFound` |

### v0.5.0

| # | 级别 | 修复 |
|---|------|------|
| H1 | High | `restore_stock` 条件扩展包含 OffShelf |
| H2 | High | `add_sold_count` 发出 `SoldCountUpdated` 替代语义错误的 `StockUpdated` |
| M1 | Medium | 在售商品禁止设置 stock=0（`CannotClearStockWhileOnSale`） |
| M2 | Medium | images_cid/detail_cid 非空校验 |
| M3 | Medium | is_price_stale() 路径测试覆盖 |

### v0.8.0

| # | 级别 | 修复 |
|---|------|------|
| H2 | High | `delist_product` 支持 SoldOut 下架（与 force_unpublish 一致） |
| M1 | Medium | 批量操作空列表短路返回 |
| M2 | Medium | `batch_delete` 补充 unpin tags_cid/sku_cid |
| M3 | Medium | `delete_product` 补充 unpin tags_cid/sku_cid |
| L2 | Low | Cargo.toml feature 传播 |
| L3 | Low | try_state 统计一致性验证 |

### v0.9.0

| # | 级别 | 修复 |
|---|------|------|
| M1 | Medium | `force_unpublish_product` reason 校验移至状态变更之前，避免无效回滚 |
| L1 | Low | Cargo.toml 补充 `sp-runtime/runtime-benchmarks`、`pallet-entity-common/{runtime-benchmarks,try-runtime}` |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，初始版本 |
| v0.2.0 | 2026-02-01 | 实现店铺派生账户押金机制 |
| v0.2.1 | 2026-02-05 | 重命名 pallet-entity-product，适配 Entity-Shop 架构 |
| v0.3.0 | 2026-02-09 | 深度审查（11 项），创建 mock+tests（42 tests） |
| v0.4.0 | 2026-02-26 | 深度审查（3 项），trait 接口防御性校验 + 押金一致性（56 tests） |
| v0.5.0 | 2026-03-02 | 深度审查（6 项），restore_stock/SoldCountUpdated/空 CID 校验（65 tests） |
| v0.6.0 | 2026-03-02 | IPFS Pin 集成，CID 自动 pin/unpin，best-effort（70 tests） |
| v0.7.0 | 2026-03-04 | 购买数量限制、可见性控制、治理接口、批量 best-effort、STORAGE_VERSION 1（117 tests） |
| v0.8.0 | 2026-03-05 | 深度审查（7 项），delist SoldOut、批量空列表、tags/sku unpin、try_state（127 tests） |
| v0.9.0 | 2026-03-05 | 深度审查（2 项），force_unpublish reason 校验顺序、Cargo.toml feature 传播（129 tests） |

## 许可证

MIT License
