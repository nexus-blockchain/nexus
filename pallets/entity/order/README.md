# pallet-entity-order

> NEXUS Entity 订单交易管理模块 — 支持 NEX / EntityToken 双资产支付，按商品类别自动选择订单流程，集成 Escrow 托管、超时自动处理、争议退款、会员消费追踪与佣金分配

## 概述

`pallet-entity-order` 是 NEXUS Entity 电商体系的核心交易模块，管理订单从下单到结算的完整生命周期。覆盖 25 个 extrinsics、9 个存储项、24 个事件、43 个错误码，共 204 个单元测试。

**核心能力：**

- **双资产支付** — 买家可选择 NEX（原生代币）或 EntityToken（实体代币）支付
- **按类别自动流程** — Digital 即时完成、Physical 需发货确认、Service 有独立服务流程
- **资金安全托管** — NEX 通过 pallet-dispute-escrow 锁定，EntityToken 通过 reserve 冻结
- **四重超时保护** — 发货 / 确认 / 服务 / 争议均有独立超时，基于 ExpiryQueue O(K) 精确调度
- **争议机制** — 买家申请退款即设置初始超时 → 卖家可同意/拒绝（限一次） → 超时自动退款
- **部分退款** — 治理可按比例部分退款（仅 NEX，通过 `Escrow::split_partial`）
- **买家撤回争议** — 争议期间（卖家拒绝前）买家可主动撤回，恢复原状态
- **会员联动** — 订单完成自动注册会员（含推荐人传递）、更新消费额、触发升级规则
- **会员折扣** — 高等级会员自动享受价格折扣
- **积分 / 购物余额抵扣** — NEX 订单支持用积分换折扣、购物余额直接抵扣
- **商品可见性** — 支持 Public / MembersOnly / LevelGated 三级访问控制
- **封禁检查** — 被 Entity 封禁的买家无法下单
- **代付支付** — 第三方代付人（payer）可为买家支付订单，资金从 payer 扣除
- **佣金联动** — NEX 订单触发 `CommissionHandler`，Token 订单触发 `TokenCommissionHandler`

## 商品类别与订单流程

| 类别 | 需物流 | 流程 | 可取消 | 可退款 |
|------|--------|------|--------|--------|
| **Digital** | 否 | 支付 → 自动完成 | 否 | 否 |
| **Physical** | 是 | 支付 → 发货 → 确认收货 | 发货前 | 是 |
| **Service** | 否 | 支付 → 开始服务 → 完成服务 → 确认 | 服务前 | 是 |
| **Bundle** | 是 | 同 Physical | 发货前 | 是 |
| **Other** | 是 | 同 Physical | 发货前 | 是 |
| **Subscription** | — | 暂不支持（返回 `SubscriptionNotSupported`） | — | — |

> `category_requires_shipping` 判定：Physical / Bundle / Other 需物流；`is_service_like` 判定：Service / Subscription 走服务流程。

## 架构依赖

```
pallet-entity-order
├── pallet-dispute-escrow ────────── NEX 资金托管（lock / transfer / refund / split_partial / disputed / resolved）
├── pallet-entity-common
│   ├── ShopProvider ─────── 店铺查询（exists / active / owner / entity_id / stats）
│   ├── ProductProvider ──── 商品查询（info / stock / deduct / restore）
│   ├── AssetLedgerPort ─── 实体代币细粒度接口（is_token_enabled / token_balance / reserve / unreserve / repatriate）
│   ├── LoyaltyWritePort ─── Token 折扣 + 购物余额抵扣 + 奖励（redeem_for_discount / consume / reward）
│   ├── MemberProvider ───── 会员查询（is_member / level / discount / banned）
│   ├── OrderCommissionHandler ── NEX 佣金回调（on_order_completed / cancelled）
│   ├── TokenOrderCommissionHandler ─ Token 佣金回调 + fee_rate + entity_account
│   ├── OrderMemberHandler ── 会员写入（auto_register / update_spent / check_upgrade_rules）
│   ├── PricingProvider ──── NEX/USDT 价格（get_nex_usdt_price）
│   └── EntityTokenPriceProvider ─ Token/NEX 价格 + 可靠性判断
└── pallet-balances ──────── 原生代币余额
```

## 资金流

### NEX 支付

```
买家下单（或代付人代付）
  ├─ 会员折扣: final_amount = total × (1 - discount_bps/10000)
  ├─ 积分抵扣: final_amount -= redeem_for_discount(tokens)     [仅 token 启用时]
  ├─ 购物余额: final_amount -= shopping_amount                 [抵扣后减少]
  └─ Escrow:   lock_from(payer或buyer, order_id, final_amount)

订单完成
  ├─ 卖家收入:  transfer_from_escrow(seller, amount - platform_fee)
  ├─ 平台费:    transfer_from_escrow(platform_account, platform_fee)
  ├─ 佣金触发:  CommissionHandler::on_order_completed
  ├─ 购物奖励:  reward_on_purchase(buyer, total_amount)
  ├─ 会员注册:  auto_register(buyer, referrer)
  ├─ 消费更新:  update_spent(buyer, amount_usdt)    [NEX→USDT 换算]
  ├─ 升级规则:  check_order_upgrade_rules
  └─ 店铺统计:  update_shop_stats(seller_amount)

取消/退款
  ├─ Escrow:   refund_all(payer或buyer) 或 split_partial(seller, payer或buyer, bps)
  ├─ 库存恢复: restore_stock(product_id, quantity)
  ├─ 佣金取消: on_order_cancelled(order_id)
  └─ 推荐人清理: OrderReferrer::remove
```

### EntityToken 支付

```
买家下单
  └─ reserve(entity_id, buyer, final_amount)

订单完成
  ├─ 卖家收入:  repatriate_reserved(buyer → seller, amount - token_fee)
  ├─ 平台费:    repatriate_reserved(buyer → entity_account, token_fee)
  │             [token_fee = amount × token_platform_fee_rate / 10000]
  ├─ 佣金触发:  TokenCommissionHandler::on_token_order_completed
  ├─ 消费更新:  Token→NEX→USDT 间接换算（仅 price reliable 时）
  └─ 其余同 NEX

取消/退款
  └─ unreserve(entity_id, buyer, total_amount)
```

## place_order / place_order_for 详细流程

```
1. ensure signed(buyer 或 payer)
   └─ place_order_for: payer ≠ seller（PayerCannotBeSeller）
2. ensure quantity > 0
3. get_product_info(product_id)                    ─ 一次 read 获取全部字段
4. ensure status == OnSale
5. ensure quantity ∈ [min_order_quantity, max_order_quantity]  ─ 0 表示不限制
6. ensure shop_exists(shop_id)
7. ensure is_shop_active(shop_id)                  ─ ShopInactive 错误
8. ensure seller ≠ buyer
9. ensure referrer ≠ buyer && referrer ≠ seller
10. ensure !is_banned(entity_id, buyer)
11. get_effective_level → 可见性校验
    ├─ Public: 通过
    ├─ MembersOnly: ensure is_member
    └─ LevelGated(n): ensure is_member && level ≥ n
12. ensure stock ≥ quantity                        ─ stock=0 表示无限库存
13. total_amount = price × quantity (checked_mul)
14. 会员折扣 → 积分抵扣 → 购物余额抵扣（仅 Native）
15. ensure final_amount > 0
16. platform_fee = final_amount × PlatformFeeRate / 10000  ─ Token 订单 fee=0
17. ensure product_category ≠ Subscription
18. 需物流商品 ensure shipping_cid.is_some()
19. 资金锁定: NEX→Escrow.lock_from(payer或buyer) / Token→reserve
20. deduct_stock + add_sold_count
21. Digital → 直接 Completed + do_complete_order
    其他 → Paid + ExpiryQueue(ShipTimeout)
22. 存储 referrer → OrderReferrer
23. 代付时追加 PayerOrders 索引
24. 更新 OrderStats.total_orders
25. emit OrderCreated
```

## 状态机

### Physical / Bundle / Other

```
[Paid] ──── buyer cancel_order ────→ [Cancelled]
  │  ├───── seller seller_cancel ──→ [Cancelled]
  │  ├───── buyer request_refund ──→ [Disputed]
  │  ├───── Root force_refund ─────→ [Refunded]
  │  ├───── Root force_complete ───→ [Completed]
  │  ├───── Root force_partial ────→ [Refunded]
  │  └───── ShipTimeout 到期 ──────→ [Refunded]  (auto)
  │
  └── seller ship_order ──→ [Shipped]
       │  ├─ buyer confirm_receipt ──→ [Completed]
       │  ├─ buyer request_refund ───→ [Disputed]
       │  ├─ seller seller_refund ───→ [Refunded]
       │  ├─ seller update_tracking
       │  ├─ buyer extend_confirm
       │  ├─ Root force_refund ──────→ [Refunded]
       │  ├─ Root force_complete ────→ [Completed]
       │  ├─ Root force_partial ─────→ [Refunded]
       │  └─ ConfirmTimeout 到期 ────→ [Completed]  (auto)

[Disputed]
  ├─── seller approve_refund ────→ [Refunded]
  ├─── seller reject_refund ─────→ [Disputed]  (设 deadline, 限一次)
  ├─── buyer withdraw_dispute ───→ [Paid/Shipped]  (卖家拒绝前可撤回)
  ├─── Root force_refund ────────→ [Refunded]
  ├─── Root force_complete ──────→ [Completed]
  ├─── Root force_partial ───────→ [Refunded]
  └─── DisputeTimeout 到期 ──────→ [Refunded]  (auto, 含卖家不响应场景)
```

### Digital

```
[Completed]  (place_order 时直接完成，不可取消/退款)
```

### Service

```
[Paid] ──── buyer cancel_order ────→ [Cancelled]
  │  ├───── seller seller_cancel ──→ [Cancelled]
  │  ├───── buyer request_refund ──→ [Disputed]
  │  ├───── Root force_* ──────────→ [Refunded/Completed]
  │  └───── ShipTimeout 到期 ──────→ [Refunded]  (auto)
  │
  └── seller start_service ──→ [Shipped]
       │  (service_started_at 记录)
       │
       ├── seller complete_service (service_completed_at 记录, 限一次)
       │    │
       │    ├── buyer confirm_service ──→ [Completed]
       │    ├── buyer request_refund ───→ [Disputed]
       │    └── ServiceConfirmTimeout ──→ [Completed]  (auto)
       │
       ├── buyer request_refund ───→ [Disputed]
       └── ServiceConfirmTimeout ──→ [Refunded]  (服务未完成时 auto)
```

## 配置项

| 参数 | 类型 | 说明 |
|------|------|------|
| `Currency` | trait | 原生代币 |
| `Escrow` | trait | 托管接口 |
| `ShopProvider` | trait | 店铺查询 |
| `ProductProvider` | trait | 商品查询（含 stock / visibility / quantity limits） |
| `EntityToken` | trait | 实体代币细粒度接口（AssetLedgerPort：is_token_enabled / reserve / unreserve / repatriate） |
| `PlatformAccount` | AccountId | 平台费接收账户 |
| `ShipTimeout` | BlockNumber | 发货超时（未发货自动退款） |
| `ConfirmTimeout` | BlockNumber | 确认收货超时（未确认自动完成） |
| `ServiceConfirmTimeout` | BlockNumber | 服务确认超时 |
| `DisputeTimeout` | BlockNumber | 争议超时（卖家未响应/拒绝后自动退款） |
| `ConfirmExtension` | BlockNumber | 确认延长时间（买家可延长一次） |
| `CommissionHandler` | trait | NEX 佣金回调 |
| `TokenCommissionHandler` | trait | Token 佣金回调 + fee_rate |
| `Loyalty` | trait | LoyaltyWritePort：Token 折扣 + 购物余额抵扣 + 购物奖励 |
| `MemberHandler` | trait | 会员注册 / 消费更新 / 升级规则 |
| `PricingProvider` | trait | NEX/USDT 价格 |
| `TokenPriceProvider` | trait | Token/NEX 价格 + 可靠性 |
| `MemberProvider` | trait | 会员查询（等级/折扣/封禁/可见性） |
| `MaxCidLength` | u32 | CID 最大长度 |
| `MaxBuyerOrders` | u32 | 每买家最大订单索引数 |
| `MaxPayerOrders` | u32 | 每代付人最大订单索引数 |
| `MaxShopOrders` | u32 | 每店铺最大订单索引数 |
| `MaxExpiryQueueSize` | u32 | 每区块过期队列最大订单数 |

## 数据结构

### Order

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u64` | 订单 ID（自增） |
| `entity_id` | `u64` | 所属 Entity ID（创建时快照） |
| `shop_id` | `u64` | 店铺 ID |
| `product_id` | `u64` | 商品 ID |
| `buyer` | `AccountId` | 买家 |
| `seller` | `AccountId` | 卖家（shop_owner 快照） |
| `payer` | `Option<AccountId>` | 代付人（第三方付款时 Some，自付时 None） |
| `quantity` | `u32` | 购买数量 |
| `unit_price` | `Balance` | 商品原始标价 |
| `total_amount` | `Balance` | 实际支付金额（折扣/抵扣后） |
| `platform_fee` | `Balance` | 平台费（NEX: amount×rate/10000, Token: 0） |
| `product_category` | `ProductCategory` | 商品类别（决定流程） |
| `shipping_cid` | `Option<BoundedVec>` | 收货地址 CID（IPFS） |
| `tracking_cid` | `Option<BoundedVec>` | 物流追踪 CID |
| `status` | `OrderStatus` | 订单状态 |
| `created_at` | `BlockNumber` | 创建区块 |
| `shipped_at` | `Option<BlockNumber>` | 发货区块 |
| `completed_at` | `Option<BlockNumber>` | 完成区块 |
| `service_started_at` | `Option<BlockNumber>` | 服务开始区块 |
| `service_completed_at` | `Option<BlockNumber>` | 服务完成区块（限设一次） |
| `payment_asset` | `PaymentAsset` | 支付资产（Native / EntityToken） |
| `token_payment_amount` | `u128` | Token 支付金额（避免泛型膨胀） |
| `confirm_extended` | `bool` | 是否已延长确认期限（限一次） |
| `dispute_rejected` | `bool` | 卖家是否已拒绝退款（限一次） |
| `dispute_deadline` | `Option<BlockNumber>` | 争议截止区块（request_refund 设初始值，reject_refund 重置） |
| `note_cid` | `Option<BoundedVec>` | 买家备注 CID |
| `refund_reason_cid` | `Option<BoundedVec>` | 退款理由 CID |

### OrderStatistics

| 字段 | 类型 | 说明 |
|------|------|------|
| `total_orders` | `u64` | 总订单数 |
| `completed_orders` | `u64` | 已完成数 |
| `total_volume` | `Balance` | NEX 总交易额 |
| `total_platform_fees` | `Balance` | NEX 总平台费 |
| `total_token_volume` | `u128` | Token 总交易额 |
| `total_token_platform_fees` | `u128` | Token 总平台费 |

### OrderOperation（失败事件追踪）

```
EscrowRefund | StockRestore | CommissionCancel | CommissionComplete
ShopStatsUpdate | TokenReward | MemberUpdate | AutoComplete
UpgradeRuleCheck | TokenPlatformFee | MemberAutoRegister
```

## 存储项

| 存储 | Key | Value | 说明 |
|------|-----|-------|------|
| `PlatformFeeRate` | — | `u16` (默认 100 bps) | NEX 平台费率，0=关闭 |
| `NextOrderId` | — | `u64` | 自增订单 ID |
| `Orders` | `u64` | `Order` | 订单详情 |
| `BuyerOrders` | `AccountId` | `BoundedVec<u64, MaxBuyerOrders>` | 买家订单索引 |
| `PayerOrders` | `AccountId` | `BoundedVec<u64, MaxPayerOrders>` | 代付人订单索引 |
| `ShopOrders` | `u64` | `BoundedVec<u64, MaxShopOrders>` | 店铺订单索引 |
| `OrderStats` | — | `OrderStatistics` | 全局统计 |
| `ExpiryQueue` | `BlockNumber` | `BoundedVec<u64, MaxExpiryQueueSize>` | 到期检查队列 |
| `OrderReferrer` | `u64` | `AccountId` | 订单推荐人（完成时消费） |

## Extrinsics

| # | 名称 | 调用者 | 说明 |
|---|------|--------|------|
| 0 | `place_order` | Buyer | 下单并支付，含可见性/库存/封禁/数量校验 |
| 1 | `cancel_order` | Buyer/Payer | 取消订单（Paid 状态，非 Digital，buyer 或 payer 均可调用） |
| 2 | `ship_order` | Seller | 发货（含 tracking_cid，非 Service 类） |
| 3 | `confirm_receipt` | Buyer | 确认收货（Shipped 状态，非 Service 类） |
| 4 | `request_refund` | Buyer | 申请退款 → Disputed + 设置 dispute_deadline |
| 5 | `approve_refund` | Seller | 同意退款（Disputed 状态） |
| 6 | `start_service` | Seller | 开始服务（Service 类，Paid 状态） |
| 7 | `complete_service` | Seller | 标记服务完成（限一次） |
| 8 | `confirm_service` | Buyer | 确认服务完成 |
| 9 | `set_platform_fee_rate` | Root | 设置 NEX 平台费率（0-1000 bps） |
| 10 | `cleanup_buyer_orders` | Buyer | 清理终态订单 ID，释放 BoundedVec 容量 |
| 11 | `reject_refund` | Seller | 拒绝退款（限一次），重置 dispute_deadline |
| 12 | `seller_cancel_order` | Seller | 卖家主动取消（Paid 状态，含 reason_cid） |
| 13 | `force_refund` | Root | 强制退款（Paid/Shipped/Disputed） |
| 14 | `force_complete` | Root | 强制完成（Paid/Shipped/Disputed） |
| 15 | `update_shipping_address` | Buyer | 修改收货地址（Paid 状态） |
| 16 | `extend_confirm_timeout` | Buyer | 延长确认期限（Shipped 状态，限一次） |
| 17 | `cleanup_shop_orders` | ShopOwner | 清理店铺终态订单索引 |
| 18 | `update_tracking` | Seller | 更新物流信息（Shipped 状态） |
| 19 | `seller_refund_order` | Seller | 卖家主动退款（Shipped 状态） |
| 20 | `force_partial_refund` | Root | 部分退款（仅 NEX，1-9999 bps） |
| 21 | `withdraw_dispute` | Buyer | 撤回争议（卖家拒绝前），恢复原状态 |
| 22 | `force_process_expirations` | Root | 手动处理指定区块的过期订单 |
| 23 | `place_order_for` | Payer | 代付下单：payer 签名为 buyer 付款，资金从 payer 扣除 |
| 24 | `cleanup_payer_orders` | Payer | 清理代付人终态订单索引，释放 BoundedVec 容量 |

## Events

| 事件 | 字段 | 触发时机 |
|------|------|---------|
| `OrderCreated` | order_id, entity_id, buyer, seller, payer, amount, payment_asset, token_amount | 下单成功 |
| `OrderShipped` | order_id | 卖家发货 |
| `OrderCompleted` | order_id, seller_received, token_seller_received | 订单完成（手动/自动） |
| `OrderCancelled` | order_id, amount, token_amount | 买家取消 |
| `OrderRefunded` | order_id, amount, token_amount | 退款完成（approve/auto/force） |
| `OrderDisputed` | order_id | 买家申请退款 |
| `OrderOperationFailed` | order_id, operation | best-effort 操作失败 |
| `ServiceStarted` | order_id | 卖家开始服务 |
| `ServiceCompleted` | order_id | 卖家标记服务完成 |
| `PlatformFeeRateUpdated` | old_rate, new_rate | 治理更新费率 |
| `BuyerOrdersCleaned` | buyer, removed | 买家清理订单索引 |
| `PayerOrdersCleaned` | payer, removed | 代付人清理订单索引 |
| `RefundRejected` | order_id, reason_cid | 卖家拒绝退款 |
| `OrderSellerCancelled` | order_id, amount, token_amount, reason_cid | 卖家主动取消 |
| `OrderForceRefunded` | order_id, reason_cid? | 管理员强制退款 |
| `OrderForceCompleted` | order_id, reason_cid? | 管理员强制完成 |
| `ShippingAddressUpdated` | order_id | 买家修改收货地址 |
| `ConfirmTimeoutExtended` | order_id, new_deadline | 买家延长确认期限 |
| `ShopOrdersCleaned` | shop_id, removed | 店铺清理订单索引 |
| `TrackingInfoUpdated` | order_id | 卖家更新物流信息 |
| `OrderSellerRefunded` | order_id, amount, token_amount, reason_cid | 卖家主动退款 |
| `OrderPartialRefunded` | order_id, refund_bps, reason_cid? | 管理员部分退款 |
| `DisputeWithdrawn` | order_id | 买家撤回争议 |
| `StaleExpirationsProcessed` | target_block, processed | 管理员手动处理过期 |

## Errors

| 错误 | 说明 |
|------|------|
| `OrderNotFound` | 订单不存在 |
| `ProductNotFound` | 商品不存在 |
| `ShopNotFound` | 店铺不存在 |
| `ShopInactive` | 店铺存在但未激活（暂停/关闭） |
| `NotOrderBuyer` | 调用者不是买家 |
| `NotOrderSeller` | 调用者不是卖家 |
| `NotShopOwner` | 不是店铺 Owner |
| `InvalidOrderStatus` | 当前状态不允许此操作 |
| `CannotCancelOrder` | 已发货 / 非 Paid 状态 |
| `CannotBuyOwnProduct` | 买家 = 卖家 |
| `CannotForceOrder` | 订单不在可强制操作的状态 |
| `ProductNotOnSale` | 商品未上架 |
| `InsufficientStock` | 库存不足 |
| `InvalidQuantity` | 数量为 0 |
| `QuantityBelowMinimum` | 低于最小购买数量 |
| `QuantityAboveMaximum` | 超过最大购买数量 |
| `InvalidAmount` | 抵扣后支付金额为 0 |
| `CidTooLong` | CID 超过 MaxCidLength |
| `EmptyReasonCid` | 退款理由 CID 为空 |
| `EmptyTrackingCid` | 物流 CID 为空 |
| `ShippingCidRequired` | 需物流但未提供收货地址 |
| `Overflow` | 索引列表 / NextOrderId 溢出 |
| `ExpiryQueueFull` | 该区块超时队列已满（500 上限） |
| `DigitalProductCannotCancel` | 数字商品不可取消 |
| `DigitalProductCannotRefund` | 数字商品不可退款 |
| `NotServiceLikeOrder` | 非 Service 类调用服务接口 |
| `ServiceLikeOrderCannotShip` | Service 类不可使用发货/收货流程 |
| `EntityTokenNotEnabled` | 实体代币未启用 |
| `InsufficientTokenBalance` | Token 余额不足 |
| `PlatformFeeRateTooHigh` | 费率超上限（max 1000 bps = 10%） |
| `NothingToClean` | 无可清理的终态订单 |
| `AlreadyExtended` | 已延长过确认期限 |
| `ProductMembersOnly` | 商品仅对会员可见 |
| `MemberLevelInsufficient` | 会员等级不足 |
| `DisputeAlreadyRejected` | 争议已被拒绝（不可重复） |
| `BuyerBanned` | 买家已被 Entity 封禁 |
| `InvalidRefundBps` | 部分退款比例无效（需 1-9999 bps） |
| `PartialRefundNotSupported` | Token 订单不支持部分退款 |
| `InvalidReferrer` | 推荐人不能是买家或卖家自己 |
| `SubscriptionNotSupported` | Subscription 类暂不支持 |
| `PayerCannotBeSeller` | 代付人不能是卖家 |
| `NotOrderParticipant` | 非订单参与者（buyer 或 payer） |
| `PayerOrdersFull` | 代付人订单索引已满 |

## ExpiryQueue 超时处理

```
on_idle(now, remaining_weight)
  │
  ├─ 每个订单约 200M ref_time / 8KB proof_size
  ├─ 每区块最多处理 20 个订单
  │
  └─ ExpiryQueue::get(now) → 遍历订单
       │
       ├─ [Paid]     → do_auto_refund（发货超时）
       ├─ [Shipped]  → do_complete_order（确认超时自动完成）
       │   └─ Service 未完成 → do_auto_refund（服务超时）
       ├─ [Disputed] → 检查 dispute_deadline
       │   └─ deadline 到达 → set_resolved + do_auto_refund
       └─ 其他状态   → 跳过（已手动处理）
```

**队列条目生命周期：**

| 操作 | 清理旧条目 | 写入新条目 |
|------|-----------|-----------|
| `place_order` | — | ShipTimeout（非 Digital） |
| `ship_order` | ShipTimeout | ConfirmTimeout |
| `start_service` | ShipTimeout | ServiceConfirmTimeout |
| `complete_service` | ServiceConfirmTimeout | ServiceConfirmTimeout（重新计时） |
| `extend_confirm_timeout` | ConfirmTimeout | ConfirmExtension |
| `request_refund` | — | DisputeTimeout |
| `reject_refund` | 旧 DisputeTimeout | 新 DisputeTimeout |
| `withdraw_dispute` | DisputeTimeout | Ship/Confirm/ServiceTimeout（恢复） |

## OrderProvider Trait 实现

为外部 pallet 提供 18 个只读查询方法：

```
order_exists / order_buyer / order_seller / order_amount / order_shop_id
is_order_completed / is_order_disputed / can_dispute
order_token_amount / order_payment_asset / order_completed_at / order_status
order_entity_id / order_product_id / order_quantity
order_created_at / order_paid_at / order_shipped_at
```

## 内部函数

> **代码组织：** 7 个争议/退款相关实现已提取到 `dispute.rs` 子模块（request_refund / approve_refund / reject_refund / force_refund / force_complete / force_partial_refund / withdraw_dispute），extrinsic 入口仍在 lib.rs 中通过 `Self::do_xxx` 委托调用。

| 函数 | 说明 |
|------|------|
| `category_requires_shipping` | Physical / Bundle / Other → true |
| `is_service_like` | Service / Subscription → true |
| `validate_reason_cid` | 非空 + 长度校验，返回 BoundedVec |
| `validate_optional_reason_cid` | Option 版 CID 校验 |
| `do_cancel_or_refund` | 退款 + 恢复库存 + 取消佣金 + 更新状态 + 清理推荐人 |
| `do_complete_order` | 资金结算 + 会员 + 佣金 + 统计 + 奖励（best-effort） |
| `cancel_commission_by_asset` | 按支付类型取消佣金 |
| `refund_by_asset` | NEX→Escrow.refund_all / Token→unreserve |
| `do_auto_refund` | do_cancel_or_refund + emit OrderRefunded |
| `is_order_participant` | buyer 或 payer → true（用于 cancel_order 等权限校验） |
| `process_expired_orders` | ExpiryQueue 遍历 + 状态分支处理 |

## 安全设计

| 防护 | 说明 |
|------|------|
| **溢出保护** | NextOrderId checked_add、total_amount checked_mul、BoundedVec try_push |
| **重入防护** | complete_service 限一次、reject_refund 限一次（dispute_rejected 标志） |
| **争议超时保护** | request_refund 即设 dispute_deadline，卖家不响应也自动退款，reject 限一次防无限延期 |
| **自购保护** | buyer ≠ seller |
| **推荐人校验** | referrer ≠ buyer && referrer ≠ seller |
| **封禁保护** | is_banned 校验阻止被封禁买家下单 |
| **可见性保护** | MembersOnly / LevelGated 在 is_member 校验后才检查等级 |
| **Token 争议兼容** | Token 订单 request_refund 不调用 Escrow::set_disputed |
| **best-effort 模式** | 会员/佣金/统计/奖励等附属操作失败仅发事件，不回滚主流程 |
| **entity_id 快照** | 订单创建时快照 entity_id，后续不再依赖 shop→entity 查询 |
| **队列一致性** | ExpiryQueue 写入先于 Orders 更新，队列满时事务回滚 |
| **费率上限** | NEX 平台费率 ≤ 1000 bps (10%)，Token 费率 ≤ 10000 bps (100%) 防御性上限 |

## 已知限制

- **ExpiryQueue 旧条目** — `ship_order` / `complete_service` / `extend_confirm_timeout` 会主动清理旧条目再追加新条目，但 `request_refund` 不清理 ShipTimeout 条目（在 on_idle 中被跳过，25M ref_time 开销）
- **on_idle 处理上限** — 每区块最多处理 20 个到期订单。极端情况可积压，可通过 `force_process_expirations` 补偿
- **ExpiryQueue 每区块上限** — 同一到期区块超过 MaxExpiryQueueSize 个订单时 place_order 失败（`ExpiryQueueFull`）
- **BuyerOrders 上限 MaxBuyerOrders** — 有 `cleanup_buyer_orders` 清理终态订单释放容量
- **PayerOrders 上限 MaxPayerOrders** — 有 `cleanup_payer_orders` 清理终态订单释放容量
- **ShopOrders 上限 MaxShopOrders** — 有 `cleanup_shop_orders` 清理终态订单释放容量
- **unit_price vs total_amount** — `unit_price` 是商品原始标价，`total_amount` 是实际扣款金额（经折扣/抵扣后），两者可能不同
- **部分退款佣金处理** — `force_partial_refund` 全额取消佣金（而非按比例），因部分退款发生在完成前，佣金尚未发放。不恢复库存
- **Subscription 暂不支持** — `place_order` 直接拒绝 Subscription 类别
- **权重未 benchmark** — 当前使用硬编码估算值，上线前需完成 `frame_benchmarking`

## 测试

```bash
cargo test -p pallet-entity-order
# 204 tests
```

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，初始版本 |
| v0.1.1 | 2026-02-05 | 重命名为 pallet-entity-order |
| v0.2.0 | 2026-02-09 | 深度审查修复 + 29 测试 |
| v0.3.0 | 2026-03-04 | 审计 R4-R5 + 71 测试 |
| v0.4.0 | 2026-03-05 | 审计 R6-R8 + 131 测试 |
| v0.5.0 | 2026-03-05 | 审计 R9 + 136 测试 |
| v0.6.0 | 2026-03-06 | 审计 R10：entity_id 快照、冗余字段移除、OrderProvider 18 方法、可见性/封禁/折扣、部分退款、卖家退款、推荐人传递 |
| v0.7.0 | 2026-03-06 | 审计 R11：force_partial_refund 修复、seller_refund 限 Shipped、referrer 校验、OrderReferrer 全路径清理 + 158 测试 |
| v0.8.0 | 2026-03-06 | 审计 R12：dispute_deadline 初始化防资金锁定、购物余额抵扣修复、reason_cid 事件持久化、do_cancel_or_refund 重构 + 170 测试 |
| v0.9.0 | 2026-03-06 | 审计 R13：ShopInactive 错误码、Mock 增强（可配置 visibility/quantity/shop_active/stock/redeem）、34 新测试覆盖积分抵扣/可见性/数量/店铺/队列/note_cid/库存跟踪 + 204 测试 |
| v0.10.0 | 2026-03-12 | 模块边界重构：EntityToken→AssetLedgerPort 细粒度 Port、dispute.rs 提取 7 个争议 extrinsic、代付功能（place_order_for + cleanup_payer_orders + PayerOrders 存储）、Loyalty 端口集成、Config 常量化（MaxBuyerOrders/MaxPayerOrders/MaxShopOrders/MaxExpiryQueueSize） |

## 许可证

Apache-2.0
