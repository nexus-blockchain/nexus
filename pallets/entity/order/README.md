# pallet-entity-order

> NEXUS 订单交易管理模块 — 按商品类别区分订单流程，集成 Escrow 托管与超时自动处理

## 概述

`pallet-entity-order` 管理订单完整生命周期：下单支付、发货、确认收货、取消、退款、超时自动处理。根据商品类别（Digital / Physical / Service / Other）自动选择不同流程。

## 按类别区分的订单流程

| 类别 | 流程 | 可取消 | 可退款 |
|------|------|--------|--------|
| **Digital** | 支付 → 自动完成 | 否 | 否 |
| **Physical** | 支付 → 发货 → 确认收货 | 发货前可取消 | 可申请退款 |
| **Service** | 支付 → 开始服务 → 完成服务 → 确认 | 服务前可取消 | 可申请退款 |
| **Other** | 支付 → 发货 → 确认收货 | 发货前可取消 | 可申请退款 |

## 架构依赖

```
pallet-entity-order
├── pallet-escrow           资金托管（锁定 / 释放 / 退款）
├── ShopProvider            Shop 查询（shop_exists, shop_owner, update_shop_stats）
├── ProductProvider         商品查询（price, stock, category, deduct_stock, restore_stock）
├── EntityTokenProvider     积分抵扣（redeem_for_discount, reward_on_purchase）
└── CommissionHandler       佣金触发（on_order_completed, on_order_cancelled）
```

## 订单资金流

```
下单支付:    买家 ──→ Escrow 托管
确认收货:    Escrow ──→ 卖家 (总额 - 平台费)
                   ──→ 平台账户 (平台费)
                   ──→ 触发佣金计算 (CommissionHandler::on_order_completed)
                   ──→ 发放购物积分 (EntityToken::reward_on_purchase)
取消/退款:   Escrow ──→ 买家 (全额退回)
                   ──→ 恢复库存 (ProductProvider::restore_stock)
                   ──→ 通知佣金取消 (CommissionHandler::on_order_cancelled)
```

## Config 配置

```rust
impl pallet_entity_order::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Escrow = EscrowPallet;
    type ShopProvider = EntityShop;
    type ProductProvider = EntityService;
    type EntityToken = EntityTokenPallet;
    type CommissionHandler = CommissionCore;
    type PlatformAccount = PlatformAccountId;
    type PlatformFeeRate = ConstU16<200>;          // 2% (基点)
    type ShipTimeout = ConstU32<100800>;            // 7 天 (6s/块)
    type ConfirmTimeout = ConstU32<201600>;         // 14 天
    type ServiceConfirmTimeout = ConstU32<100800>;  // 7 天
    type MaxCidLength = ConstU32<64>;
}
```

| 参数 | 说明 |
|------|------|
| `Currency` | 货币类型 |
| `Escrow` | Escrow 托管接口（lock_from / transfer_from_escrow / refund_all） |
| `ShopProvider` | Shop 查询 + 统计更新 |
| `ProductProvider` | 商品查询 + 库存管理 |
| `EntityToken` | 积分抵扣 + 购物奖励 |
| `CommissionHandler` | 订单完成时触发佣金返还 |
| `PlatformAccount` | 平台费收款地址 |
| `PlatformFeeRate` | 平台费率（基点，200 = 2%） |
| `ShipTimeout` | 发货超时（区块数） |
| `ConfirmTimeout` | 确认收货超时（区块数） |
| `ServiceConfirmTimeout` | 服务确认超时（区块数） |
| `MaxCidLength` | IPFS CID 最大字节数 |

## 数据结构

### Order

```rust
pub struct Order<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,
    pub shop_id: u64,
    pub product_id: u64,
    pub buyer: AccountId,
    pub seller: AccountId,
    pub quantity: u32,
    pub unit_price: Balance,
    pub total_amount: Balance,          // 积分抵扣后的实际支付金额
    pub platform_fee: Balance,
    pub product_category: ProductCategory,
    pub requires_shipping: bool,
    pub shipping_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub tracking_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub status: OrderStatus,
    pub created_at: BlockNumber,
    pub paid_at: Option<BlockNumber>,
    pub shipped_at: Option<BlockNumber>,
    pub completed_at: Option<BlockNumber>,
    pub service_started_at: Option<BlockNumber>,
    pub service_completed_at: Option<BlockNumber>,
    pub escrow_id: u64,
}
```

### OrderStatus（定义于 pallet-entity-common）

| 状态 | 说明 |
|------|------|
| `Created` | 已创建（当前实现中直接跳到 Paid） |
| `Paid` | 已支付，等待发货/服务 |
| `Shipped` | 已发货 / 服务进行中 |
| `Completed` | 已完成，资金已释放 |
| `Cancelled` | 已取消，资金已退回 |
| `Refunded` | 已退款 |
| `Disputed` | 争议中（买家申请退款） |
| `Expired` | 已过期 |

### OrderStatistics

```rust
pub struct OrderStatistics<Balance: Default> {
    pub total_orders: u64,
    pub completed_orders: u64,
    pub total_volume: Balance,
    pub total_platform_fees: Balance,
}
```

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `NextOrderId` | `StorageValue<u64>` | 下一个订单 ID（自增） |
| `Orders` | `StorageMap<u64, MallOrder>` | 订单主表 |
| `BuyerOrders` | `StorageMap<AccountId, BoundedVec<u64, 1000>>` | 买家订单索引 |
| `ShopOrders` | `StorageMap<u64, BoundedVec<u64, 10000>>` | 店铺订单索引 |
| `OrderStats` | `StorageValue<OrderStatistics>` | 全局订单统计 |
| `ExpiryQueue` | `StorageMap<BlockNumber, BoundedVec<u64, 500>>` | 超时检查队列 |

## Extrinsics

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 0 | `place_order(product_id, quantity, shipping_cid, use_tokens)` | 任意用户 | 下单支付，资金锁入 Escrow |
| 1 | `cancel_order(order_id)` | 买家 | 取消订单，仅 Paid 状态且非 Digital，通知佣金取消 |
| 2 | `ship_order(order_id, tracking_cid)` | 卖家 | 填写物流信息，状态 Paid → Shipped |
| 3 | `confirm_receipt(order_id)` | 买家 | 确认收货，释放资金 |
| 4 | `request_refund(order_id, reason_cid)` | 买家 | 申请退款，状态 → Disputed |
| 5 | `approve_refund(order_id)` | 卖家 | 同意退款，全额退回买家，通知佣金取消 |
| 6 | `start_service(order_id)` | 卖家 | 开始服务（仅 Service 类），写入 ServiceConfirmTimeout 超时队列 |
| 7 | `complete_service(order_id)` | 卖家 | 标记服务完成（仅 Service 类） |
| 8 | `confirm_service(order_id)` | 买家 | 确认服务完成，释放资金 |

### place_order 详细流程

1. 校验 `quantity > 0`
2. 验证商品存在、在售、库存充足
3. 验证 Shop 存在且活跃，获取卖家地址
4. 确保买家 ≠ 卖家
5. 计算总额 `price * quantity`，积分抵扣（可选）
6. 校验抵扣后金额 > 0（`InvalidAmount`）
7. 计算平台费 `final_amount * PlatformFeeRate / 10000`
8. `Escrow::lock_from` 锁定资金
9. 扣减库存 + 增加销量
10. Digital 商品立即调用 `do_complete_order` 完成
11. 其他类别写入 `ExpiryQueue[now + ShipTimeout]`（队列满返回 `ExpiryQueueFull`）

### do_complete_order 内部流程

1. `Escrow::transfer_from_escrow` 释放卖家金额（总额 - 平台费）
2. `Escrow::transfer_from_escrow` 平台费转平台账户
3. `ShopProvider::update_shop_stats` 更新店铺统计
4. `CommissionHandler::on_order_completed` 触发佣金计算
5. `EntityToken::reward_on_purchase` 发放购物积分
6. 更新全局 `OrderStats`

## 超时自动处理（on_idle）

利用 `ExpiryQueue` 实现 O(K) 复杂度的精确超时检查（K = 当前区块到期订单数）：

| 场景 | 超时参数 | 自动处理 |
|------|----------|----------|
| 发货超时 | `ShipTimeout` | 退款给买家，恢复库存 |
| 确认超时 | `ConfirmTimeout` | 自动确认收货，释放资金 |
| 服务开始超时 | `ShipTimeout` | 退款给买家（卖家未开始服务） |
| 服务完成超时 | `ServiceConfirmTimeout` | 退款给买家（卖家已开始但未完成服务） |
| 服务确认超时 | `ServiceConfirmTimeout` | 自动确认服务，释放资金 |

- 每次 `on_idle` 最多处理 20 个订单
- 已被手动处理的订单（取消/退款/确认）自动跳过
- 超时退款同样触发 `CommissionHandler::on_order_cancelled` 和 `ProductProvider::restore_stock`
- `ExpiryQueue` 每个区块上限 500 个订单，溢出时返回 `ExpiryQueueFull`

## 订单状态机

### Physical / Other

```
place_order ──→ [Paid] ──→ ship_order ──→ [Shipped] ──→ confirm_receipt ──→ [Completed]
                  │                           │
                  ├── cancel_order ──→ [Cancelled]
                  ├── 发货超时 ──→ [Refunded]
                  │                           ├── 确认超时 ──→ [Completed]
                  └── request_refund ──→ [Disputed] ──→ approve_refund ──→ [Refunded]
```

### Service

```
place_order ──→ [Paid] ──→ start_service ──→ [Shipped] ──→ complete_service ──→ confirm_service ──→ [Completed]
                  │                                                                    │
                  ├── cancel_order ──→ [Cancelled]                                     └── 确认超时 ──→ [Completed]
                  ├── 服务开始超时 ──→ [Refunded]
                  └── request_refund ──→ [Disputed] ──→ approve_refund ──→ [Refunded]
```

### Digital

```
place_order ──→ [Completed]  (即时完成，不可取消/退款)
```

## OrderProvider Trait 实现

供其他模块（如 commission）查询订单信息：

```rust
impl OrderProvider<AccountId, Balance> for Pallet<T> {
    fn order_exists(order_id: u64) -> bool;
    fn order_buyer(order_id: u64) -> Option<AccountId>;
    fn order_shop_id(order_id: u64) -> Option<u64>;
    fn is_order_completed(order_id: u64) -> bool;
}
```

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `OrderCreated` | order_id, buyer, seller, amount | place_order |
| `OrderPaid` | order_id, escrow_id | place_order |
| `OrderShipped` | order_id | ship_order |
| `OrderCompleted` | order_id, seller_received | confirm_receipt / 超时自动确认 |
| `OrderCancelled` | order_id | cancel_order |
| `OrderRefunded` | order_id, amount | approve_refund / 超时自动退款 |
| `OrderDisputed` | order_id | request_refund |
| `ServiceStarted` | order_id | start_service |
| `ServiceCompleted` | order_id | complete_service |

## Errors

| 错误 | 说明 |
|------|------|
| `OrderNotFound` | 订单不存在 |
| `ProductNotFound` | 商品不存在 |
| `ShopNotFound` | 店铺不存在或未激活 |
| `NotOrderBuyer` | 调用者不是买家 |
| `NotOrderSeller` | 调用者不是卖家 |
| `InvalidOrderStatus` | 当前状态不允许此操作 |
| `CannotCancelOrder` | 已发货，无法取消 |
| `CannotBuyOwnProduct` | 不能购买自己店铺的商品 |
| `ProductNotOnSale` | 商品未上架 |
| `InsufficientStock` | 库存不足 |
| `InvalidQuantity` | 数量为 0 |
| `CidTooLong` | CID 超过 MaxCidLength |
| `Overflow` | 索引列表容量溢出 |
| `DigitalProductCannotCancel` | 数字商品不可取消 |
| `DigitalProductCannotRefund` | 数字商品不可退款 |
| `NotServiceOrder` | 非 Service 类订单调用了服务接口 |
| `InvalidAmount` | 积分抵扣后支付金额为 0 |
| `ExpiryQueueFull` | 该区块超时队列已满（500 上限） |

## 测试

```bash
cargo test -p pallet-entity-order
# 56 tests: place_order(6), cancel(4), ship(2), confirm(2), refund(4),
#           service(5), timeout(3), OrderProvider(1), stats(1), fee(1),
#           member(3), token(8), audit_regression(5), expiry_queue(1)
```

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，初始版本 |
| v0.1.1 | 2026-02-05 | 重命名为 pallet-entity-order，适配 Entity-Shop 分离架构 |
| v0.2.0 | 2026-02-09 | 深度审查修复：积分抵扣零额校验、佣金取消通知、DecodeWithMemTracking、ExpiryQueue 溢出保护、Weight 修正、服务类库存恢复、创建 mock + 29 个测试 |
| v0.3.0 | 2026-03-04 | 深度审计修复：库存零值校验(H2)、空物流CID校验(H3)、服务超时保护(H4)、reward_on_purchase entity_id修复(M3)、log/std features(M2)、README数据结构名称更正(M4/M5) + 56 个测试 |

## 许可证

MIT License
