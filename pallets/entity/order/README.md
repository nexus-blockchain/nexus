# pallet-entity-order

> NEXUS Entity 订单交易管理模块 — 支持 NEX / EntityToken 双资产支付，按商品类别自动选择订单流程，集成 Escrow 托管、超时自动处理、争议退款、会员消费追踪与佣金分配

## 概述

`pallet-entity-order` 是 NEXUS Entity 电商体系的核心交易模块，管理订单从下单到结算的完整生命周期。模块经过 9 轮深度审计，覆盖 19 个 extrinsics、7 个存储项、20 个事件、25 个错误码，共 136 个单元测试。

**核心能力：**

- **双资产支付** — 买家可选择 NEX（原生代币）或 EntityToken（实体代币）支付
- **按类别自动流程** — Digital 即时完成、Physical 需发货确认、Service 有独立服务流程
- **资金安全托管** — NEX 通过 pallet-escrow 锁定，EntityToken 通过 reserve 冻结
- **超时自动处理** — 基于 ExpiryQueue 的 O(K) 精确超时，发货/确认/服务/争议均自动兜底
- **争议机制** — 买家申请退款 → 卖家同意/拒绝 → 超时自动退款（reject 限一次，防无限延期）
- **会员联动** — 订单完成自动注册会员、更新消费额（NEX→USDT / Token→NEX→USDT 换算）、触发升级规则
- **佣金联动** — NEX 订单触发 CommissionHandler，Token 订单触发 TokenCommissionHandler
- **商品可见性** — 支持 Public / MembersOnly / LevelGated 三级访问控制

## 商品类别与订单流程

| 类别 | 需物流 | 流程 | 可取消 | 可退款 |
|------|--------|------|--------|--------|
| **Digital** | 否 | 支付 → 自动完成 | 否 | 否 |
| **Physical** | 是 | 支付 → 发货 → 确认收货 | 发货前 | 是 |
| **Service** | 否 | 支付 → 开始服务 → 完成服务 → 确认 | 服务前 | 是 |
| **Subscription** | 否 | 支付 → 超时自动退款 | 发货前 | 是 |
| **Bundle** | 是 | 支付 → 发货 → 确认收货 | 发货前 | 是 |
| **Other** | 是 | 支付 → 发货 → 确认收货 | 发货前 | 是 |

## 架构依赖

```
pallet-entity-order
│
├── pallet-escrow              NEX 资金托管（lock / transfer / refund / dispute）
├── pallet-entity-common       共享类型（OrderStatus, PaymentAsset, ProductCategory, traits）
│
├── ShopProvider               店铺查询（exists, active, owner, entity_id, update_stats）
├── ProductProvider            商品查询（price, stock, category, visibility, deduct/restore）
├── EntityTokenProvider        代币操作（reserve, unreserve, repatriate, redeem, reward）
├── CommissionHandler          NEX 佣金（on_order_completed / on_order_cancelled）
├── TokenCommissionHandler     Token 佣金（on_token_order_completed / cancelled, fee_rate）
├── ShoppingBalanceProvider    购物余额抵扣（consume_shopping_balance）
├── MemberHandler              会员管理（auto_register, update_spent, check_upgrade_rules）
├── MemberProvider             会员查询（is_member, get_effective_level）
├── PricingProvider            NEX/USDT 定价（get_nex_usdt_price）
└── TokenPriceProvider         Token/NEX 定价（get_token_price, is_reliable）
```

## 资金流

### NEX 支付

```
下单:   买家钱包 ──lock_from──→ Escrow 托管
             ↑
        购物余额 ──consume──→ 买家钱包（先到账再锁入 Escrow）
        积分抵扣 ──redeem──→ 减少 final_amount

完成:   Escrow ──transfer──→ 卖家 (total_amount - platform_fee)
        Escrow ──transfer──→ PlatformAccount (platform_fee)
        ──→ ShopProvider::update_shop_stats
        ──→ CommissionHandler::on_order_completed（触发 NEX 返佣）
        ──→ EntityToken::reward_on_purchase（发放购物积分）
        ──→ MemberHandler::auto_register + update_spent + check_upgrade_rules（会员联动）

取消:   Escrow ──refund_all──→ 买家（全额退回）
        ──→ ProductProvider::restore_stock
        ──→ CommissionHandler::on_order_cancelled
```

### EntityToken 支付

```
下单:   EntityToken::reserve（冻结买家 Token）

完成:   EntityToken::repatriate_reserved ──→ 卖家 (token_amount - token_fee)
        EntityToken::repatriate_reserved ──→ entity_account (token_fee)
        ──→ TokenCommissionHandler::on_token_order_completed（触发 Token 返佣）
        ──→ 其余同 NEX（shop_stats / reward / member）

取消:   EntityToken::unreserve ──→ 买家（解冻 Token）
        ──→ ProductProvider::restore_stock
        ──→ TokenCommissionHandler::on_token_order_cancelled
```

> **平台费计算**：NEX 订单使用全局 `PlatformFeeRate`（StorageValue，默认 100 bps = 1%）；Token 订单使用 Entity 级 `TokenCommissionHandler::token_platform_fee_rate(entity_id)`，上限防御性截断至 10000 bps（100%）。

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent;
    type Currency: Currency<Self::AccountId>;
    type Escrow: EscrowTrait<Self::AccountId, BalanceOf<Self>>;
    type ShopProvider: ShopProvider<Self::AccountId>;
    type ProductProvider: ProductProvider<Self::AccountId, BalanceOf<Self>>;
    type EntityToken: EntityTokenProvider<Self::AccountId, BalanceOf<Self>>;
    type CommissionHandler: OrderCommissionHandler<Self::AccountId, BalanceOf<Self>>;
    type TokenCommissionHandler: TokenOrderCommissionHandler<Self::AccountId>;
    type ShoppingBalance: ShoppingBalanceProvider<Self::AccountId, BalanceOf<Self>>;
    type MemberHandler: OrderMemberHandler<Self::AccountId>;
    type PricingProvider: PricingProvider;
    type TokenPriceProvider: EntityTokenPriceProvider<Balance = BalanceOf<Self>>;
    type MemberProvider: MemberProvider<Self::AccountId>;

    #[pallet::constant]
    type PlatformAccount: Get<Self::AccountId>;
    #[pallet::constant]
    type ShipTimeout: Get<BlockNumberFor<Self>>;          // 发货超时
    #[pallet::constant]
    type ConfirmTimeout: Get<BlockNumberFor<Self>>;       // 确认收货超时
    #[pallet::constant]
    type ServiceConfirmTimeout: Get<BlockNumberFor<Self>>; // 服务确认超时
    #[pallet::constant]
    type DisputeTimeout: Get<BlockNumberFor<Self>>;       // 争议超时（reject_refund 后自动退款）
    #[pallet::constant]
    type ConfirmExtension: Get<BlockNumberFor<Self>>;     // 确认延长时间（限一次）
    #[pallet::constant]
    type MaxCidLength: Get<u32>;                          // IPFS CID 最大字节数
}
```

## 数据结构

### Order

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u64` | 订单 ID（全局自增，checked_add 防溢出） |
| `shop_id` | `u64` | 店铺 ID |
| `product_id` | `u64` | 商品 ID |
| `buyer` | `AccountId` | 买家账户 |
| `seller` | `AccountId` | 卖家账户 |
| `quantity` | `u32` | 购买数量 |
| `unit_price` | `Balance` | 单价 |
| `total_amount` | `Balance` | 实际支付金额（积分/购物余额抵扣后，checked_mul 防溢出） |
| `platform_fee` | `Balance` | NEX 平台费（Token 订单此字段为 0） |
| `product_category` | `ProductCategory` | 商品类别（决定流程） |
| `requires_shipping` | `bool` | 是否需要物流 |
| `shipping_cid` | `Option<BoundedVec<u8, MaxCidLength>>` | 收货地址 IPFS CID |
| `tracking_cid` | `Option<BoundedVec<u8, MaxCidLength>>` | 物流信息 IPFS CID |
| `status` | `OrderStatus` | 订单状态 |
| `created_at` | `BlockNumber` | 创建区块 |
| `paid_at` | `Option<BlockNumber>` | 支付区块 |
| `shipped_at` | `Option<BlockNumber>` | 发货区块 |
| `completed_at` | `Option<BlockNumber>` | 完成区块 |
| `service_started_at` | `Option<BlockNumber>` | 服务开始区块 |
| `service_completed_at` | `Option<BlockNumber>` | 服务完成区块（卖家标记，限设置一次） |
| `escrow_id` | `u64` | Escrow 托管 ID（= order_id） |
| `payment_asset` | `PaymentAsset` | 支付资产类型：`Native` / `EntityToken` |
| `token_payment_amount` | `u128` | Token 支付金额（仅 EntityToken 有效，u128 避免泛型膨胀） |
| `confirm_extended` | `bool` | 买家是否已延长确认期限（限一次） |
| `dispute_deadline` | `Option<BlockNumber>` | 争议超时截止区块（reject_refund 时设置，限一次） |
| `note_cid` | `Option<BoundedVec<u8, MaxCidLength>>` | 买家备注 IPFS CID |

### OrderStatus（pallet-entity-common）

| 状态 | 说明 |
|------|------|
| `Paid` | 已支付，等待发货/服务 |
| `Shipped` | 已发货 / 服务进行中 |
| `Completed` | 已完成，资金已释放 |
| `Cancelled` | 已取消，资金已退回 |
| `Refunded` | 已退款 |
| `Disputed` | 争议中（买家申请退款） |

### PaymentAsset（pallet-entity-common）

| 变体 | 说明 |
|------|------|
| `Native` | NEX 原生代币支付（通过 Escrow 托管） |
| `EntityToken` | 实体代币支付（通过 reserve/unreserve 冻结） |

### OrderStatistics

| 字段 | 类型 | 说明 |
|------|------|------|
| `total_orders` | `u64` | 总下单数（含所有状态） |
| `completed_orders` | `u64` | 已完成订单数 |
| `total_volume` | `Balance` | NEX 累计交易额 |
| `total_platform_fees` | `Balance` | NEX 累计平台费 |
| `total_token_volume` | `u128` | Token 累计交易额 |
| `total_token_platform_fees` | `u128` | Token 累计平台费 |

### OrderOperation（失败追踪枚举）

用于 `OrderOperationFailed` 事件，标识哪个 best-effort 操作失败：

| 变体 | 说明 |
|------|------|
| `EscrowRefund` | Escrow 退款失败 |
| `StockRestore` | 库存恢复失败 |
| `CommissionCancel` | 佣金取消通知失败 |
| `CommissionComplete` | 佣金结算通知失败 |
| `ShopStatsUpdate` | 店铺统计更新失败 |
| `TokenReward` | 积分奖励发放失败 |
| `MemberUpdate` | 会员消费更新失败 |
| `AutoComplete` | 超时自动完成失败 |
| `UpgradeRuleCheck` | 升级规则检查失败 |
| `TokenPlatformFee` | Token 平台费分配失败 |
| `MemberAutoRegister` | 会员自动注册失败 |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `PlatformFeeRate` | `StorageValue<u16>` | NEX 平台费率（基点，默认 100 = 1%，上限 1000 = 10%） |
| `NextOrderId` | `StorageValue<u64>` | 下一个订单 ID（checked_add 防溢出） |
| `Orders` | `StorageMap<u64, Order>` | 订单主表 |
| `BuyerOrders` | `StorageMap<AccountId, BoundedVec<u64, 1000>>` | 买家订单索引（可 cleanup） |
| `ShopOrders` | `StorageMap<u64, BoundedVec<u64, 10000>>` | 店铺订单索引（可 cleanup） |
| `OrderStats` | `StorageValue<OrderStatistics>` | 全局统计（NEX + Token 分别追踪） |
| `ExpiryQueue` | `StorageMap<BlockNumber, BoundedVec<u64, 500>>` | 超时检查队列（per-block 上限 500） |

## Extrinsics

### 买家操作

| # | 调用 | Weight (ref_time / proof_size) | 说明 |
|---|------|------|------|
| 0 | `place_order(product_id, quantity, shipping_cid, use_tokens, use_shopping_balance, payment_asset, note_cid)` | 350M / 16K | 下单支付 |
| 1 | `cancel_order(order_id)` | 250M / 12K | 取消订单（Paid 状态，非 Digital） |
| 3 | `confirm_receipt(order_id)` | 300M / 12K | 确认收货（Shipped，非 Service） |
| 4 | `request_refund(order_id, reason_cid)` | 150M / 8K | 申请退款（Paid/Shipped → Disputed） |
| 8 | `confirm_service(order_id)` | 300M / 12K | 确认服务完成（Service 类，service_completed_at 已设置） |
| 10 | `cleanup_buyer_orders()` | 100M / 8K | 清理终态订单索引，释放 BoundedVec 容量 |
| 15 | `update_shipping_address(order_id, new_cid)` | 100M / 4K | 修改收货地址（Paid 状态，需物流） |
| 16 | `extend_confirm_timeout(order_id)` | 100M / 4K | 延长确认期限（Shipped，非 Service，限一次） |

### 卖家操作

| # | 调用 | Weight (ref_time / proof_size) | 说明 |
|---|------|------|------|
| 2 | `ship_order(order_id, tracking_cid)` | 200M / 8K | 发货（Paid → Shipped，写入 ConfirmTimeout） |
| 5 | `approve_refund(order_id)` | 250M / 12K | 同意退款（Disputed → Refunded） |
| 6 | `start_service(order_id)` | 150M / 8K | 开始服务（Service 类，Paid → Shipped） |
| 7 | `complete_service(order_id)` | 175M / 8K | 标记服务完成（Service 类，限一次） |
| 11 | `reject_refund(order_id, reason_cid)` | 150M / 8K | 拒绝退款（设置 dispute_deadline，限一次） |
| 12 | `seller_cancel_order(order_id, reason_cid)` | 250M / 12K | 卖家取消（Paid 状态，非 Digital） |
| 17 | `cleanup_shop_orders(shop_id)` | 150M / 8K | 清理店铺终态订单索引 |
| 18 | `update_tracking(order_id, tracking_cid)` | 100M / 4K | 更新物流信息（Shipped 状态） |

### 治理操作（Root）

| # | 调用 | Weight (ref_time / proof_size) | 说明 |
|---|------|------|------|
| 9 | `set_platform_fee_rate(new_rate)` | 20M / 2K | 设置 NEX 平台费率（≤ 1000 bps） |
| 13 | `force_refund(order_id, reason_cid?)` | 300M / 12K | 强制退款（Paid/Shipped/Disputed） |
| 14 | `force_complete(order_id, reason_cid?)` | 350M / 16K | 强制完成（Paid/Shipped/Disputed） |

### place_order 详细流程

```
 1. quantity > 0
 2. 商品校验：exists, on_sale, min/max_order_quantity
 3. 可见性校验：复用已解析 shop_id → entity_id（M1-R9 优化）
    Public 放行 / MembersOnly 需会员 / LevelGated 需等级
 4. 库存校验：stock >= quantity（None = 无限）
 5. 店铺校验：exists, active, buyer ≠ seller
 6. 金额计算：price × quantity → total_amount（checked_mul 防溢出）
 7. 积分抵扣（仅 Native）：redeem_for_discount → final_amount
 8. 购物余额抵扣（仅 Native）：consume → 买家钱包到账 NEX
 9. final_amount > 0 校验
10. 平台费：Native = final_amount × PlatformFeeRate / 10000，Token = 0
11. 资金锁定：Native → Escrow::lock_from / Token → EntityToken::reserve
12. 扣减库存 + 增加销量
13. 创建 Order 写入 Orders + BuyerOrders + ShopOrders
14. NextOrderId checked_add 防溢出
15. Digital → 直接 do_complete_order
16. 其他 → ExpiryQueue[now + ShipTimeout]
```

### do_complete_order 内部流程

```
1. 资金释放
   Native:  Escrow → 卖家(total - fee) + 平台(fee)
   Token:   repatriate_reserved → 卖家(token - token_fee) + entity_account(token_fee)
            token_fee = token_amount × token_platform_fee_rate / 10000（上限 10000 bps 防御）
2. 订单标记 Completed + completed_at
3. MemberHandler::auto_register（首购自动注册会员）
4. MemberHandler::update_spent（消费额 → USDT 换算追踪）
   Native:  amount_nex × nex_usdt_price / 10^12
   Token:   token_amount × token_nex_price × nex_usdt_price / 10^12（需价格可靠）
5. MemberHandler::check_order_upgrade_rules（升级规则检查）
6. ShopProvider::update_shop_stats（店铺统计）
7. CommissionHandler / TokenCommissionHandler（佣金分配）
8. EntityToken::reward_on_purchase（积分奖励）
9. OrderStats 更新（NEX / Token 分别追踪）
```

> 步骤 3-8 均为 best-effort：失败发射 `OrderOperationFailed` 事件，不阻塞主流程。

## 内部辅助函数

| 函数 | 说明 |
|------|------|
| `do_complete_order(order_id, &order)` | 完成订单：释放资金 + 会员 + 佣金 + 统计 |
| `do_auto_refund(&order, order_id) → bool` | 统一自动退款：refund → restore_stock → cancel_commission → set Refunded → emit event |
| `refund_by_asset(&order, order_id)` | 按支付资产退款：Native → Escrow refund，Token → unreserve |
| `cancel_commission_by_asset(&order, order_id)` | 按支付资产取消佣金通知（best-effort） |

## 状态机

### Physical / Bundle / Other（需物流）

```
place_order ──→ [Paid] ──┬── ship_order ──→ [Shipped] ──┬── confirm_receipt ──→ [Completed]
                         │                              │
                         │                              ├── 确认超时 ──→ [Completed]
                         │                              │
                         │                              └── extend_confirm_timeout（延长一次）
                         │
                         ├── cancel_order ──→ [Cancelled]
                         ├── seller_cancel_order ──→ [Cancelled]
                         ├── 发货超时 ──→ [Refunded]（do_auto_refund）
                         │
                         └── request_refund ──→ [Disputed] ──┬── approve_refund ──→ [Refunded]
                                                             ├── reject_refund（设 deadline，限一次）
                                                             │     └── 争议超时 ──→ [Refunded]（do_auto_refund）
                                                             ├── force_refund ──→ [Refunded]
                                                             └── force_complete ──→ [Completed]
```

### Service

```
place_order ──→ [Paid] ──┬── start_service ──→ [Shipped] ──┬── complete_service（限一次）
                         │                                  │     └── confirm_service ──→ [Completed]
                         │                                  │           └── 服务确认超时 ──→ [Completed]
                         │                                  │
                         │                                  └── 服务完成超时 ──→ [Refunded]（do_auto_refund）
                         │
                         ├── cancel_order ──→ [Cancelled]
                         ├── 服务开始超时 ──→ [Refunded]（do_auto_refund）
                         │
                         └── request_refund ──→ [Disputed] ──┬── approve_refund ──→ [Refunded]
                                                             └── reject_refund → 争议超时 ──→ [Refunded]
```

### Digital

```
place_order ──→ [Completed]（即时完成，不可取消/退款）
```

## 超时自动处理（on_idle）

`ExpiryQueue` 是以区块号为键的 `StorageMap`，每个区块槽位最多存储 500 个待检查订单 ID。`on_idle` 仅读取当前区块对应的队列，O(K) 复杂度。

| 订单状态 | 触发条件 | 自动处理 |
|----------|----------|----------|
| `Paid`（任何非 Digital） | now ≥ created_at + ShipTimeout | `do_auto_refund`（退款 + 恢复库存） |
| `Shipped` + Service（未完成） | now ≥ started_at + ServiceConfirmTimeout | `do_auto_refund`（卖家超时未完成服务） |
| `Shipped`（已完成/非 Service） | now ≥ shipped_at + ConfirmTimeout | `do_complete_order`（自动确认） |
| `Disputed` | now ≥ dispute_deadline | 解除争议锁定 + `do_auto_refund`（争议超时） |
| 其他终态 | — | 跳过（已手动处理） |

**运行约束：**

- 每次最多处理 **20 个**订单
- 最小剩余 weight：250M ref_time + 50M 余量
- 空队列返回：`Weight::from_parts(5_000, 64)`（含 proof_size）
- 权重精确报告（含 ref_time + proof_size 双维度）：
  - 基础：50M ref_time + 4K proof_size
  - 每个已处理订单：+200M ref_time + 8K proof_size
  - 每个跳过订单：+25M ref_time + 2K proof_size

## OrderProvider Trait

供其他模块（commission、arbitration 等）查询订单信息：

```rust
impl OrderProvider<AccountId, Balance> for Pallet<T> {
    fn order_exists(order_id: u64) -> bool;
    fn order_buyer(order_id: u64) -> Option<AccountId>;
    fn order_seller(order_id: u64) -> Option<AccountId>;
    fn order_amount(order_id: u64) -> Option<Balance>;
    fn order_shop_id(order_id: u64) -> Option<u64>;
    fn is_order_completed(order_id: u64) -> bool;
    fn is_order_disputed(order_id: u64) -> bool;
    fn can_dispute(order_id: u64, who: &AccountId) -> bool;
    fn order_token_amount(order_id: u64) -> Option<u128>;
    fn order_payment_asset(order_id: u64) -> Option<PaymentAsset>;
    fn order_completed_at(order_id: u64) -> Option<u64>;
}
```

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `OrderCreated` | order_id, buyer, seller, amount, payment_asset, token_amount | place_order |
| `OrderPaid` | order_id, escrow_id | place_order |
| `OrderShipped` | order_id | ship_order |
| `OrderCompleted` | order_id, seller_received, token_seller_received | confirm_receipt / confirm_service / 超时自动确认 / Digital 自动完成 |
| `OrderCancelled` | order_id, amount, token_amount | cancel_order |
| `OrderSellerCancelled` | order_id, amount, token_amount | seller_cancel_order |
| `OrderRefunded` | order_id, amount, token_amount | approve_refund / force_refund / do_auto_refund |
| `OrderDisputed` | order_id | request_refund |
| `RefundRejected` | order_id | reject_refund |
| `OrderForceRefunded` | order_id | force_refund |
| `OrderForceCompleted` | order_id | force_complete |
| `ServiceStarted` | order_id | start_service |
| `ServiceCompleted` | order_id | complete_service |
| `PlatformFeeRateUpdated` | old_rate, new_rate | set_platform_fee_rate |
| `ShippingAddressUpdated` | order_id | update_shipping_address |
| `TrackingInfoUpdated` | order_id | update_tracking |
| `ConfirmTimeoutExtended` | order_id, new_deadline | extend_confirm_timeout |
| `BuyerOrdersCleaned` | buyer, removed | cleanup_buyer_orders |
| `ShopOrdersCleaned` | shop_id, removed | cleanup_shop_orders |
| `OrderOperationFailed` | order_id, operation | best-effort 操作失败（需人工干预） |

## Errors

| 错误 | 说明 |
|------|------|
| `OrderNotFound` | 订单不存在 |
| `ProductNotFound` | 商品不存在 |
| `ShopNotFound` | 店铺不存在或未激活 |
| `NotOrderBuyer` | 调用者不是买家 |
| `NotOrderSeller` | 调用者不是卖家 |
| `InvalidOrderStatus` | 当前状态不允许此操作 |
| `CannotCancelOrder` | 已发货 / 非 Paid 状态 |
| `CannotBuyOwnProduct` | 买家 = 卖家 |
| `ProductNotOnSale` | 商品未上架 |
| `InsufficientStock` | 库存不足 |
| `InvalidQuantity` | 数量为 0 |
| `InvalidAmount` | 抵扣后支付金额为 0 |
| `CidTooLong` | CID 超过 MaxCidLength |
| `Overflow` | 索引列表 / NextOrderId 溢出 |
| `DigitalProductCannotCancel` | 数字商品不可取消 |
| `DigitalProductCannotRefund` | 数字商品不可退款 |
| `NotServiceOrder` | 非 Service 类调用服务接口 |
| `ServiceOrderCannotShip` | Service 类不可使用发货/收货/延长确认流程 |
| `ShippingCidRequired` | 需物流但未提供收货地址 |
| `EmptyTrackingCid` | 物流 CID 为空 |
| `EmptyReasonCid` | 退款理由 CID 为空 |
| `EntityTokenNotEnabled` | 实体代币未启用 |
| `InsufficientTokenBalance` | Token 余额不足 |
| `PlatformFeeRateTooHigh` | 费率超上限（max 1000 bps = 10%） |
| `ExpiryQueueFull` | 该区块超时队列已满（500 上限） |
| `NothingToClean` | 无可清理的终态订单 |
| `NotShopOwner` | 不是店铺 Owner |
| `AlreadyExtended` | 已延长过确认期限 |
| `CannotForceOrder` | 订单不在可强制操作的状态 |
| `QuantityBelowMinimum` | 购买数量 < 商品最小限制 |
| `QuantityAboveMaximum` | 购买数量 > 商品最大限制 |
| `ProductMembersOnly` | 商品仅对会员可见 |
| `MemberLevelInsufficient` | 会员等级不足 |
| `DisputeAlreadyRejected` | 争议已被拒绝（不可重复） |

## 安全设计

| 防护 | 说明 |
|------|------|
| **溢出保护** | NextOrderId checked_add、total_amount checked_mul、BuyerOrders/ShopOrders try_push |
| **重入防护** | complete_service 限调一次（service_completed_at guard）、reject_refund 限调一次（dispute_deadline guard） |
| **自购保护** | buyer ≠ seller 校验 |
| **空值防护** | tracking_cid / reason_cid 非空校验、CID 长度上界 MaxCidLength |
| **Token 争议兼容** | Token 订单 request_refund 不调用 Escrow::set_disputed（Token 未使用 Escrow） |
| **best-effort 模式** | 会员/佣金/统计等附属操作失败仅发事件，不回滚主流程 |
| **权重精确报告** | on_idle 分 ref_time + proof_size 双维度报告，空队列也报告 proof_size |

## 测试

```bash
cargo test -p pallet-entity-order
# 136 tests
```

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，初始版本 |
| v0.1.1 | 2026-02-05 | 重命名为 pallet-entity-order，适配 Entity-Shop 分离架构 |
| v0.2.0 | 2026-02-09 | 深度审查修复：积分抵扣零额校验、佣金取消通知、DecodeWithMemTracking、ExpiryQueue 溢出保护、Weight 修正、服务类库存恢复 + 29 测试 |
| v0.3.0 | 2026-03-04 | 深度审计 R4-R5：库存零值校验(H2)、空物流CID(H3)、服务超时保护(H4)、reward entity_id修复(M3)、log/std(M2)、Token消费额安全降级(H1-R5) + 71 测试 |
| v0.4.0 | 2026-03-05 | 深度审计 R6-R8：complete_service防重复(M1-R6)、entity_id去重(L1-R6)、reject_refund防重复(H1-R8)、on_idle weight含跳过开销(M1-R8)、Token统计分别追踪(M2-R8)、token_fee_rate防御上限(M3-R8)、extend拒绝Service(L1-R8) + 131 测试 |
| v0.5.0 | 2026-03-05 | 深度审计 R9：复用shop_id避免冗余调用(M1-R9)、空队列proof_size修复(M2-R9)、提取do_auto_refund消除重复代码(L1-R9)、测试注释修正(L2-R9) + 136 测试 |

## 许可证

MIT License
