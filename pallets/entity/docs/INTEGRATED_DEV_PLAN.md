# Entity 模块整合开发文档 — 模块边界重构 + 代付功能

> 版本: 1.0
> 日期: 2026-03-12
> 整合来源: ENTITY_MODULE_BOUNDARY_V3_FINAL + PROXY_PAY_OPTIMAL_DESIGN
> 范围: `pallets/entity/`

---

## 一、整合背景

两项任务存在深度交叉：

| 任务 | 核心变更 | 涉及模块 |
|------|---------|----------|
| **模块边界重构** | 新增 loyalty/ pallet，从 shop/commission/token 迁出激励功能 | common, shop, commission, token, order, loyalty(新) |
| **代付功能** | order 新增 payer 角色，分离"付款人"与"下单人" | common, order |

**交叉点**: order 模块同时受到两项变更影响——
1. 边界重构改变了 order 调用折扣/购物余额/奖励的 **trait 来源**
2. 代付功能改变了 order 内部资金操作的 **账户对象**

如果分开做，order 模块要改两轮；整合做，只改一轮，且能保证两项设计在 trait 层面一致。

### 1.1 交叉影响矩阵

```
                    ┌─────────────────────────────────┐
                    │        order/src/lib.rs          │
                    ├─────────────────────────────────┤
  边界重构 →        │  ① trait 依赖变更                │
  改变调用来源      │    EntityToken → LoyaltyReadPort │
                    │    ShoppingBalance → LoyaltyWrite│
                    │                                  │
  代付功能 →        │  ② 账户对象变更                  │
  改变谁的钱        │    &buyer → fund_account()       │
                    │    &buyer → actual_payer         │
                    │                                  │
  两者交叉 →        │  ③ 折扣从 buyer 扣，             │
  do_place_order    │    差额从 payer 锁               │
                    │    奖励发给 buyer                 │
                    │    退款退给 payer                 │
                    └─────────────────────────────────┘
```

### 1.2 整合原则

| 原则 | 说明 |
|------|------|
| **一次到位** | order 的 trait 重绑定和 payer 注入在同一个 PR 中完成 |
| **trait 先行** | 先定义 Port trait（含 payer 语义），再迁移实现，最后注入 order |
| **向前兼容** | 所有新 trait 方法带默认实现，旧 adapter 零改动可编译 |
| **独立可测** | 每个 Phase 结束后系统可编译+测试，不出现半成品 |

---

## 二、目标架构全景

### 2.1 五平面架构（含代付标注）

```
╔══════════════════════════════════════════════════════════════════════╗
║                      FOUNDATION (基础层)                             ║
║                                                                      ║
║  ┌────────────────────────────────────────────────────────────────┐  ║
║  │  common/                                                       │  ║
║  │  ├── types/       领域枚举 & DTO（按域分文件）                  │  ║
║  │  ├── traits/      最小能力 Port trait（按角色分文件）            │  ║
║  │  │   ├── order.rs   OrderQueryPort ← +order_payer              │  ║
║  │  │   │                              ← +order_fund_account      │  ║
║  │  │   └── loyalty.rs LoyaltyReadPort / LoyaltyWritePort【新增】 │  ║
║  │  ├── errors.rs    CommonError                                  │  ║
║  │  ├── pagination.rs                                             │  ║
║  │  └── admin.rs     AdminPermission bitmask                      │  ║
║  └────────────────────────────────────────────────────────────────┘  ║
╚══════════════════════════════════════════════════════════════════════╝
        │
        ▼
╔══════════════════════════════════════════════════════════════════════╗
║                  CONTROL PLANE (组织控制平面)                        ║
║  registry(25) │ governance(18) │ kyc(25) │ disclosure(39)           ║
╚══════════════════════════════════════════════════════════════════════╝
        │
        ▼
╔══════════════════════════════════════════════════════════════════════╗
║                COMMERCE PLANE (交易履约平面)                          ║
║                                                                      ║
║  ┌────────────┐ ┌────────────┐ ┌─────────────────────┐ ┌────────┐  ║
║  │  shop       │ │  product   │ │  order               │ │ review │  ║
║  │  ~23 ext    │ │  9 ext     │ │  25 ext              │ │ 5 ext  │  ║
║  │             │ │            │ │  (23 现有             │ │        │  ║
║  │ ×积分移出   │ │            │ │  +place_order_for    │ │        │  ║
║  │             │ │            │ │  +cleanup_payer_orders│ │        │  ║
║  │             │ │            │ │  +PayerOrders storage)│ │        │  ║
║  │             │ │            │ │                       │ │        │  ║
║  │             │ │            │ │  争议子域(dispute.rs) │ │        │  ║
║  └────────────┘ └────────────┘ └─────────────────────┘ └────────┘  ║
╚══════════════════════════════════════════════════════════════════════╝
        │
        ▼
╔══════════════════════════════════════════════════════════════════════╗
║                GROWTH PLANE (增长激励平面)                            ║
║                                                                      ║
║  ┌────────────────┐ ┌──────────────────┐ ┌─────────────────────┐   ║
║  │  member         │ │  loyalty 【新增】  │ │  commission         │   ║
║  │  33 ext         │ │  ~15 ext          │ │  ~27 ext + 6 plugin │   ║
║  │                 │ │                   │ │                     │   ║
║  │ 身份/推荐链     │ │ ■ 积分(shop→)     │ │ 返佣配置/结算        │   ║
║  │ 等级/升级规则   │ │ ■ 购物余额        │ │ 佣金记录/奖池        │   ║
║  │ 审批/封禁       │ │   (commission→)   │ │ 提现/冷却期          │   ║
║  │                 │ │ ■ 消费激励        │ │                     │   ║
║  │                 │ │   (token→)        │ │                     │   ║
║  └────────────────┘ └──────────────────┘ └─────────────────────┘   ║
╚══════════════════════════════════════════════════════════════════════╝
        │
        ▼
╔══════════════════════════════════════════════════════════════════════╗
║                CAPITAL PLANE (资本市场平面)                           ║
║  token(~23) │ tokensale(27) │ market(24+)                           ║
║  ×reward/redeem 移出到 loyalty                                      ║
╚══════════════════════════════════════════════════════════════════════╝
```

### 2.2 order 模块内部改造后全景

```
order/src/
├── lib.rs
│   ├── Order 结构体        ← +payer: Option<AccountId>
│   ├── PayerOrders storage ← 新增
│   ├── Config trait         ← EntityToken/ShoppingBalance → LoyaltyRead/Write
│   │
│   ├── place_order()       ← 委托 do_place_order(buyer, None)
│   ├── place_order_for()   ← 新增，委托 do_place_order(buyer, Some(payer))
│   ├── do_place_order()    ← 提取的内部函数：
│   │   │                      折扣/购物余额 → LoyaltyPort(&buyer)
│   │   │                      资金锁定      → Escrow/Token(actual_payer)
│   │   │                      创建订单      → Order { payer: Option }
│   │   └                      索引写入      → BuyerOrders + PayerOrders
│   │
│   ├── cancel_order()      ← 权限: is_order_participant
│   ├── request_refund()    ← 权限: is_order_participant
│   ├── withdraw_dispute()  ← 权限: is_order_participant
│   │
│   ├── do_complete_order() ← Token 结算: fund_account()
│   │                          激励: LoyaltyPort(&order.buyer)
│   │                          佣金: CommissionHandler(&order.buyer)
│   │
│   ├── refund_by_asset()   ← fund_account() 统一退款对象
│   │
│   ├── fund_account()      ← 新增 helper
│   ├── is_order_participant() ← 新增 helper
│   │
│   ├── cleanup_buyer_orders()  ← 不变
│   └── cleanup_payer_orders()  ← 新增
│
└── dispute.rs               ← Phase 3 内部拆出
```

---

## 三、Trait 依赖变更详细设计

### 3.1 order Config trait 变更对照

| 现有 trait bound | 迁移后 trait bound | 调用方 | 说明 |
|-----------------|-------------------|--------|------|
| `EntityToken: EntityTokenProvider` | `LoyaltyReward: LoyaltyReadPort` | `redeem_for_discount`, `reward_on_purchase` | 从 token 迁到 loyalty |
| `ShoppingBalance: ShoppingBalanceProvider` | `LoyaltyBalance: LoyaltyWritePort` | `consume_shopping_balance` | 从 commission 迁到 loyalty |
| `CommissionHandler: OrderCommissionHandler` | 不变 | `on_order_completed` | commission 保留 |
| `TokenCommissionHandler: TokenOrderCommissionHandler` | 不变 | `on_token_order_completed` | commission 保留 |
| `MemberProvider: MemberProvider` | 不变 | `auto_register`, `update_spent` | member 保留 |
| `Escrow: EscrowTrait` | 不变 | `lock_from`, `refund_all` | Escrow 保留 |

### 3.2 新增 LoyaltyReadPort trait

```rust
// common/src/traits/loyalty.rs

/// 激励系统只读查询接口
pub trait LoyaltyReadPort<AccountId, Balance> {
    /// 查询用户在某 entity 下的 token 折扣余额
    fn token_discount_balance(entity_id: u64, who: &AccountId) -> Balance;

    /// 查询用户在某 entity 下的购物余额
    fn shopping_balance(entity_id: u64, who: &AccountId) -> Balance;

    /// 查询 entity 是否启用了 token
    fn is_token_enabled(entity_id: u64) -> bool;
}
```

### 3.3 新增 LoyaltyWritePort trait

```rust
/// 激励系统写入接口
pub trait LoyaltyWritePort<AccountId, Balance>: LoyaltyReadPort<AccountId, Balance> {
    /// Token 折扣抵扣（从 buyer 的 token 中扣减，返回实际折扣金额）
    fn redeem_for_discount(
        entity_id: u64, who: &AccountId, tokens: Balance,
    ) -> Result<Balance, DispatchError>;

    /// 消费购物余额（从 buyer 的购物余额中扣减）
    fn consume_shopping_balance(
        entity_id: u64, who: &AccountId, amount: Balance,
    ) -> DispatchResult;

    /// 购物奖励发放（mint token 给 buyer）
    fn reward_on_purchase(
        entity_id: u64, who: &AccountId, amount: Balance,
    ) -> DispatchResult;

    /// 写入购物余额（commission 结算后调用）
    fn credit_shopping_balance(
        entity_id: u64, who: &AccountId, amount: Balance,
    ) -> DispatchResult;
}
```

### 3.4 OrderQueryPort 扩展（代付相关）

```rust
// common/src/traits/order.rs — 在现有 OrderProvider 基础上新增

/// 获取订单代付方（无代付则返回 None）
fn order_payer(order_id: u64) -> Option<AccountId> { None }

/// 获取订单资金方（有代付返回 payer，否则返回 buyer）
fn order_fund_account(order_id: u64) -> Option<AccountId> { None }
```

### 3.5 order Config 最终形态

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    type Currency: Currency<Self::AccountId>;
    type Escrow: EscrowTrait<Self::AccountId, BalanceOf<Self>>;

    // 交易平面
    type ShopProvider: ShopQueryPort<Self::AccountId>;
    type ProductProvider: CatalogPort<Self::AccountId, BalanceOf<Self>>;

    // 增长平面 — 激励接口统一收口到 loyalty
    type Loyalty: LoyaltyWritePort<Self::AccountId, BalanceOf<Self>>;
    type MemberProvider: MemberQueryPort<Self::AccountId>;
    type CommissionHandler: OrderCommissionHandler<Self::AccountId, BalanceOf<Self>>;
    type TokenCommissionHandler: TokenOrderCommissionHandler<Self::AccountId>;

    // 资本平面 — Token 资产操作（reserve/unreserve/repatriate）
    type EntityToken: AssetLedgerPort<Self::AccountId, BalanceOf<Self>>;

    // 定价
    type PricingProvider: PricingProvider;
    type TokenPriceProvider: EntityTokenPriceProvider<Balance = BalanceOf<Self>>;

    // 常量
    #[pallet::constant]
    type MaxBuyerOrders: Get<u32>;
    // ...
}
```

**关键分离**:
- `Loyalty` — 折扣/购物余额/购物奖励（激励语义）
- `EntityToken` — reserve/unreserve/repatriate（资产账本语义）

两者在代付场景下的调用对象不同：
- `Loyalty` 始终以 `&buyer` 调用（buyer 的权益）
- `EntityToken` 以 `actual_payer`/`fund_account()` 调用（payer 的资金）

---

## 四、代付功能完整设计

### 4.1 Order 结构体

```rust
pub struct Order<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,
    pub entity_id: u64,
    pub shop_id: u64,
    pub product_id: u64,
    pub buyer: AccountId,
    pub seller: AccountId,
    pub payer: Option<AccountId>,  // 新增: None = buyer 自付
    pub quantity: u32,
    pub unit_price: Balance,
    pub total_amount: Balance,
    pub platform_fee: Balance,
    pub product_category: ProductCategory,
    pub shipping_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub tracking_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub status: OrderStatus,
    pub created_at: BlockNumber,
    pub shipped_at: Option<BlockNumber>,
    pub completed_at: Option<BlockNumber>,
    pub service_started_at: Option<BlockNumber>,
    pub service_completed_at: Option<BlockNumber>,
    pub payment_asset: PaymentAsset,
    pub token_payment_amount: u128,
    pub confirm_extended: bool,
    pub dispute_rejected: bool,
    pub dispute_deadline: Option<BlockNumber>,
    pub note_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub refund_reason_cid: Option<BoundedVec<u8, MaxCidLen>>,
}
```

### 4.2 辅助函数

```rust
impl<T: Config> Pallet<T> {
    /// 获取订单资金方（代付→payer，普通→buyer）
    fn fund_account(order: &OrderOf<T>) -> &T::AccountId {
        order.payer.as_ref().unwrap_or(&order.buyer)
    }

    /// 判断 who 是否为订单参与方（buyer 或 payer）
    fn is_order_participant(order: &OrderOf<T>, who: &T::AccountId) -> bool {
        order.buyer == *who || order.payer.as_ref().map_or(false, |p| p == who)
    }
}
```

### 4.3 do_place_order 核心逻辑（整合后）

```rust
fn do_place_order(
    buyer: &T::AccountId,
    payer: Option<&T::AccountId>,  // None = buyer 自付
    product_id: u64, quantity: u32,
    shipping_cid: Option<Vec<u8>>,
    use_tokens: Option<BalanceOf<T>>,
    use_shopping_balance: Option<BalanceOf<T>>,
    payment_asset: Option<PaymentAsset>,
    note_cid: Option<Vec<u8>>,
    referrer: Option<T::AccountId>,
) -> DispatchResult {
    let actual_payer = payer.unwrap_or(buyer);

    // ... 验证逻辑（商品/库存/可见性/会员等级，均按 buyer 查）...

    // 安全校验
    ensure!(actual_payer != &seller, Error::<T>::PayerCannotBeSeller);

    // ═══════════════════════════════════════════════
    // 阶段 1: buyer 权益抵扣（调用 Loyalty trait）
    //         始终以 &buyer 调用 — buyer 的权益
    // ═══════════════════════════════════════════════
    if resolved_payment_asset == PaymentAsset::Native {
        if let Some(tokens) = use_tokens {
            if !tokens.is_zero() && T::Loyalty::is_token_enabled(entity_id) {
                let discount = T::Loyalty::redeem_for_discount(entity_id, &buyer, tokens)?;
                final_amount = final_amount.saturating_sub(discount);
            }
        }
        if let Some(shopping_amount) = use_shopping_balance {
            if !shopping_amount.is_zero() {
                ensure!(shopping_amount <= final_amount, Error::<T>::InvalidAmount);
                T::Loyalty::consume_shopping_balance(entity_id, &buyer, shopping_amount)?;
                final_amount = final_amount.saturating_sub(shopping_amount);
            }
        }
    }

    // ═══════════════════════════════════════════════
    // 阶段 2: 资金锁定（调用 Escrow/EntityToken）
    //         以 actual_payer 调用 — payer 的资金
    // ═══════════════════════════════════════════════
    match resolved_payment_asset {
        PaymentAsset::Native => {
            T::Escrow::lock_from(actual_payer, order_id, final_amount)?;
        },
        PaymentAsset::EntityToken => {
            let balance = T::EntityToken::token_balance(entity_id, actual_payer);
            ensure!(balance >= final_amount, Error::<T>::InsufficientTokenBalance);
            T::EntityToken::reserve(entity_id, actual_payer, final_amount)?;
        },
    };

    // ═══════════════════════════════════════════════
    // 阶段 3: 创建订单 + 写索引
    // ═══════════════════════════════════════════════
    let order = Order {
        buyer: buyer.clone(),
        seller: seller.clone(),
        payer: payer.map(|p| p.clone()),
        // ... 其余字段不变 ...
    };

    BuyerOrders::<T>::try_mutate(buyer, |ids| ids.try_push(order_id))?;
    if let Some(p) = payer {
        PayerOrders::<T>::try_mutate(p, |ids| {
            ids.try_push(order_id).map_err(|_| Error::<T>::PayerOrdersFull)
        })?;
    }

    Ok(())
}
```

### 4.4 do_complete_order 核心逻辑（整合后）

```rust
fn do_complete_order(order: &mut OrderOf<T>, order_id: u64) -> DispatchResult {
    let entity_id = order.entity_id;
    let fund_acct = Self::fund_account(order);

    // ═══════════════════════════════════════════════
    // 阶段 1: 资金结算（用 fund_account，payer 的钱）
    // ═══════════════════════════════════════════════
    match order.payment_asset {
        PaymentAsset::Native => {
            // Escrow 按 order_id 管理，不受 payer 影响
            T::Escrow::transfer_from_escrow(order_id, &order.seller, seller_amount)?;
            T::Escrow::transfer_from_escrow(order_id, &platform_account, platform_fee)?;
        },
        PaymentAsset::EntityToken => {
            // Token: 从 fund_account 的 reserved 转出
            T::EntityToken::repatriate_reserved(entity_id, fund_acct, &order.seller, seller_token)?;
            T::EntityToken::repatriate_reserved(entity_id, fund_acct, &entity_account, fee_token)?;
        },
    };

    // ═══════════════════════════════════════════════
    // 阶段 2: 激励发放（用 &order.buyer，buyer 的权益）
    // ═══════════════════════════════════════════════
    // 会员注册/消费额/等级
    T::MemberProvider::auto_register(entity_id, &order.buyer, referrer);
    T::MemberProvider::update_spent(entity_id, &order.buyer, amount_usdt);
    T::MemberProvider::check_order_upgrade_rules(entity_id, &order.buyer, ...);

    // 佣金（buyer 推荐链获佣）
    match order.payment_asset {
        PaymentAsset::Native => {
            let _ = T::CommissionHandler::on_order_completed(
                entity_id, order.shop_id, order_id, &order.buyer,
                order.total_amount, order.platform_fee,
            );
        },
        PaymentAsset::EntityToken => {
            let _ = T::TokenCommissionHandler::on_token_order_completed(
                entity_id, order.shop_id, order_id, &order.buyer,
                order.token_payment_amount, token_platform_fee,
            );
        },
    };

    // 购物奖励（Loyalty trait，发给 buyer）
    let reward_amount = match order.payment_asset {
        PaymentAsset::Native => order.total_amount,
        PaymentAsset::EntityToken => order.token_payment_amount.saturated_into(),
    };
    let _ = T::Loyalty::reward_on_purchase(entity_id, &order.buyer, reward_amount);

    Ok(())
}
```

### 4.5 资金触点完整改动表

#### 必改（12 处资金 + 3 处权限 = 15 处）

| 位置 | 函数 | 改动 | 类型 |
|------|------|------|------|
| do_place_order | Escrow::lock_from | `&buyer` → `actual_payer` | 资金 |
| do_place_order | token_balance | `&buyer` → `actual_payer` | 资金 |
| do_place_order | EntityToken::reserve | `&buyer` → `actual_payer` | 资金 |
| refund_by_asset | Escrow::refund_all | `&order.buyer` → `fund_account()` | 资金 |
| refund_by_asset | EntityToken::unreserve | `&order.buyer` → `fund_account()` | 资金 |
| force_partial_refund | Escrow::split_partial | `&order.buyer` → `fund_account()` | 资金 |
| do_complete_order | repatriate_reserved(seller) | `&order.buyer` → `fund_account()` | 资金 |
| do_complete_order | repatriate_reserved(platform) | `&order.buyer` → `fund_account()` | 资金 |
| cancel_order | ensure!(buyer == who) | → `is_order_participant()` | 权限 |
| request_refund | ensure!(buyer == who) | → `is_order_participant()` | 权限 |
| withdraw_dispute | ensure!(buyer == who) | → `is_order_participant()` | 权限 |

#### 同步改为调用 Loyalty trait（3 处 trait 迁移）

| 位置 | 原调用 | 新调用 | 调用对象 |
|------|--------|--------|---------|
| do_place_order | `T::EntityToken::redeem_for_discount(entity_id, &buyer, ...)` | `T::Loyalty::redeem_for_discount(entity_id, &buyer, ...)` | buyer（不变） |
| do_place_order | `T::ShoppingBalance::consume_shopping_balance(entity_id, &buyer, ...)` | `T::Loyalty::consume_shopping_balance(entity_id, &buyer, ...)` | buyer（不变） |
| do_complete_order | `T::EntityToken::reward_on_purchase(entity_id, &order.buyer, ...)` | `T::Loyalty::reward_on_purchase(entity_id, &order.buyer, ...)` | buyer（不变） |

#### 保持不变（15 处激励/履约操作）

| 位置 | 函数 | 理由 |
|------|------|------|
| confirm_receipt | buyer == who | 确认收货是买家行为 |
| extend_confirm_timeout | buyer == who | 延长确认是买家行为 |
| update_shipping_address | buyer == who | 收货地址属于 buyer |
| do_complete_order | auto_register | buyer 获会员身份 |
| do_complete_order | update_spent | buyer 累计消费 |
| do_complete_order | check_order_upgrade_rules | buyer 等级升级 |
| do_complete_order | CommissionHandler | buyer 推荐链获佣 |
| do_complete_order | TokenCommissionHandler | buyer 推荐链获佣 |

---

## 五、新增 Storage / Event / Error

### 5.1 新增 Storage

```rust
/// 按 payer 索引代付订单
#[pallet::storage]
#[pallet::getter(fn payer_orders)]
pub type PayerOrders<T: Config> = StorageMap<
    _, Blake2_128Concat, T::AccountId,
    BoundedVec<u64, T::MaxBuyerOrders>,  // 复用同一上限
    ValueQuery,
>;
```

### 5.2 Event 变更

```rust
OrderCreated {
    order_id: u64,
    entity_id: u64,
    buyer: T::AccountId,
    seller: T::AccountId,
    payer: Option<T::AccountId>,  // 新增
    amount: BalanceOf<T>,
    payment_asset: PaymentAsset,
    token_amount: u128,
},
PayerOrdersCleaned { payer: T::AccountId, removed: u32 },  // 新增
```

### 5.3 新增 Error

```rust
PayerCannotBeSeller,    // payer 不能是卖家
NotOrderParticipant,    // 调用者不是 buyer 或 payer
PayerOrdersFull,        // payer 订单列表已满
```

### 5.4 新增 Extrinsic

| call_index | 名称 | 签名者 | 说明 |
|------------|------|--------|------|
| 23 | `place_order_for` | payer | 代付下单 |
| 24 | `cleanup_payer_orders` | payer | 清理终态代付订单索引 |

---

## 六、权限矩阵

| 操作 | buyer | payer | seller | Root |
|------|:-----:|:-----:|:------:|:----:|
| place_order | ✅ | — | — | — |
| place_order_for | — | ✅ | — | — |
| cancel_order | ✅ | ✅ | — | — |
| ship_order | — | — | ✅ | — |
| confirm_receipt | ✅ | — | — | — |
| request_refund | ✅ | ✅ | — | — |
| approve_refund | — | — | ✅ | — |
| reject_refund | — | — | ✅ | — |
| withdraw_dispute | ✅ | ✅ | — | — |
| extend_confirm_timeout | ✅ | — | — | — |
| update_shipping_address | ✅ | — | — | — |
| force_refund | — | — | — | ✅ |
| force_partial_refund | — | — | — | ✅ |
| force_complete | — | — | — | ✅ |
| cleanup_payer_orders | — | ✅ | — | — |

**规则**: 资金相关(取消/退款/争议) → buyer+payer | 履约相关(收货/延期/地址) → buyer | 卖家操作 → seller | 治理 → Root

---

## 七、资金流向全景图

### 7.1 下单

```
  place_order_for(payer签名, buyer, product, ...)
         │
         ├── buyer 权益抵扣 ← Loyalty trait (始终 &buyer)
         │   ├── Token折扣: Loyalty::redeem_for_discount(entity, &buyer, 10)
         │   └── 购物余额:  Loyalty::consume_shopping_balance(entity, &buyer, 20)
         │
         │   total=100, 折扣后 final_amount=70
         │
         └── 资金锁定 ← Escrow/EntityToken (actual_payer)
             ├── NEX:   Escrow::lock_from(actual_payer, order_id, 70)
             └── Token: EntityToken::reserve(entity, actual_payer, 70)
```

### 7.2 结算

```
  do_complete_order(order)
         │
         ├── 资金结算 ← fund_account() = payer (payer 的钱)
         │   ├── NEX:   Escrow → seller(63) + platform(7)
         │   └── Token: repatriate_reserved(fund_account→seller, fund_account→platform)
         │
         └── 激励发放 ← 始终 &order.buyer (buyer 的权益)
             ├── 会员: auto_register + update_spent + upgrade_check
             ├── 佣金: CommissionHandler(&buyer) → buyer 推荐链获佣
             └── 奖励: Loyalty::reward_on_purchase(entity, &buyer, amount)
```

### 7.3 退款

```
  refund_by_asset(order)
         │
         └── fund_account() → payer
             ├── NEX:   Escrow::refund_all(order_id, fund_account) → payer
             └── Token: EntityToken::unreserve(entity, fund_account, amount) → payer

  buyer 已消费的 Token折扣/购物余额不退（与普通订单一致）
```

---

## 八、Storage Migration

### 8.1 Order 结构体 Migration

Order 新增 `payer: Option<AccountId>` 字段，SCALE 编码位置敏感，需要正式 migration：

```rust
pub mod migration {
    use super::*;
    use frame_support::traits::OnRuntimeUpgrade;

    pub struct AddPayerField<T>(sp_std::marker::PhantomData<T>);

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct OldOrder<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
        pub id: u64,
        pub entity_id: u64,
        pub shop_id: u64,
        pub product_id: u64,
        pub buyer: AccountId,
        pub seller: AccountId,
        // 无 payer 字段
        pub quantity: u32,
        pub unit_price: Balance,
        pub total_amount: Balance,
        pub platform_fee: Balance,
        pub product_category: ProductCategory,
        pub shipping_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub tracking_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub status: OrderStatus,
        pub created_at: BlockNumber,
        pub shipped_at: Option<BlockNumber>,
        pub completed_at: Option<BlockNumber>,
        pub service_started_at: Option<BlockNumber>,
        pub service_completed_at: Option<BlockNumber>,
        pub payment_asset: PaymentAsset,
        pub token_payment_amount: u128,
        pub confirm_extended: bool,
        pub dispute_rejected: bool,
        pub dispute_deadline: Option<BlockNumber>,
        pub note_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub refund_reason_cid: Option<BoundedVec<u8, MaxCidLen>>,
    }

    impl<T: Config> OnRuntimeUpgrade for AddPayerField<T> {
        fn on_runtime_upgrade() -> Weight {
            let mut count = 0u64;
            Orders::<T>::translate(|_key, old: OldOrder<
                T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::MaxCidLen,
            >| {
                count += 1;
                Some(Order {
                    id: old.id, entity_id: old.entity_id,
                    shop_id: old.shop_id, product_id: old.product_id,
                    buyer: old.buyer, seller: old.seller,
                    payer: None,  // 历史订单无代付方
                    quantity: old.quantity, unit_price: old.unit_price,
                    total_amount: old.total_amount, platform_fee: old.platform_fee,
                    product_category: old.product_category,
                    shipping_cid: old.shipping_cid, tracking_cid: old.tracking_cid,
                    status: old.status, created_at: old.created_at,
                    shipped_at: old.shipped_at, completed_at: old.completed_at,
                    service_started_at: old.service_started_at,
                    service_completed_at: old.service_completed_at,
                    payment_asset: old.payment_asset,
                    token_payment_amount: old.token_payment_amount,
                    confirm_extended: old.confirm_extended,
                    dispute_rejected: old.dispute_rejected,
                    dispute_deadline: old.dispute_deadline,
                    note_cid: old.note_cid, refund_reason_cid: old.refund_reason_cid,
                })
            });
            T::DbWeight::get().reads_writes(count, count)
        }
    }
}
```

### 8.2 Loyalty Storage 迁移策略

积分/购物余额从 shop/commission 迁到 loyalty，使用 `#[pallet::storage_prefix]` 保持原 storage key：

```rust
// loyalty/src/lib.rs
#[pallet::storage]
#[pallet::storage_prefix = "ShopPointsConfigs"]  // 保持原 shop 的 key
pub type PointsConfigs<T: Config> = StorageMap<...>;

#[pallet::storage]
#[pallet::storage_prefix = "MemberShoppingBalance"]  // 保持原 commission 的 key
pub type ShoppingBalances<T: Config> = StorageDoubleMap<...>;
```

**零链上迁移**: storage_prefix 不变，链上数据无需搬运。

---

## 九、安全分析

### 9.1 代付特有风险

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| payer == seller 刷单 | 高 | `ensure!(actual_payer != &seller)` |
| Token reserve/repatriate 不一致 | 高 | Order 持久化 payer + fund_account 统一 |
| 退款退错人 | 高 | refund_by_asset 通过 fund_account 收口 |
| 自动过期退款退错人 | 中 | do_auto_refund → refund_by_asset → fund_account 链路统一 |
| 洗钱 | 中 | 退款始终退 payer，堵死转移路径 |

### 9.2 模块迁移风险

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| storage_prefix 不匹配导致数据丢失 | 高 | 逐个验证 prefix 与原模块一致 |
| trait 调用链断裂 | 中 | adapter blanket impl 兼容旧接口 |
| 积分/余额状态在迁移中不一致 | 中 | 同一 runtime upgrade 原子切换 |

### 9.3 金额一致性（代付+折扣）

```
下单: total=100, buyer折扣=-10, buyer购物余额=-20, payer锁定=70
退款: payer收回 70, buyer折扣/余额不退
结算: seller=70-fee, platform=fee, buyer获激励, payer无激励
```

自洽，无遗漏。

---

## 十、测试覆盖矩阵

### 10.1 代付基础 (6 cases)

| # | 场景 | 校验点 |
|---|------|--------|
| 1 | payer 代付 NEX 成功 | payer 扣款, buyer 不扣, Order.payer=Some |
| 2 | payer 代付 Token 成功 | payer reserved, buyer 不变 |
| 3 | payer 余额不足 | InsufficientBalance, 无状态变化 |
| 4 | payer == seller | PayerCannotBeSeller |
| 5 | payer == buyer 退化 | Order.payer=None, 等同 place_order |
| 6 | buyer折扣 + payer补差 | buyer token扣10, 购物余额扣20, payer锁70 |

### 10.2 代付退款 (6 cases)

| # | 场景 | 校验点 |
|---|------|--------|
| 7 | buyer 取消代付订单 | payer 收到退款 |
| 8 | payer 取消代付订单 | payer 收到退款 |
| 9 | seller 同意退款 | payer 收到退款 |
| 10 | 发货超时自动退款 | payer 收到退款 |
| 11 | 争议超时自动退款 | payer 收到退款 |
| 12 | force_partial_refund | payer 收到比例部分 |

### 10.3 代付结算 (4 cases)

| # | 场景 | 校验点 |
|---|------|--------|
| 13 | 代付完成(NEX) | seller收款, buyer获激励, payer无激励 |
| 14 | 代付完成(Token) | fund_account reserved→seller |
| 15 | 自动确认收货 | 结算给seller, 退款(如有)给payer |
| 16 | 代付+数字商品 | 立即完成, payer扣款+buyer获奖励 |

### 10.4 代付权限 (6 cases)

| # | 场景 | 校验点 |
|---|------|--------|
| 17 | payer 调 confirm_receipt | NotOrderBuyer |
| 18 | payer 调 extend_confirm_timeout | NotOrderBuyer |
| 19 | payer 调 update_shipping_address | NotOrderBuyer |
| 20 | 第三方调 cancel_order | NotOrderParticipant |
| 21 | buyer 调 request_refund | 成功 |
| 22 | payer 调 request_refund | 成功 |

### 10.5 索引与清理 (5 cases)

| # | 场景 | 校验点 |
|---|------|--------|
| 23 | 代付→BuyerOrders 包含 | buyer列表有此订单 |
| 24 | 代付→PayerOrders 包含 | payer列表有此订单 |
| 25 | payer==buyer→PayerOrders不写入 | 退化不占索引 |
| 26 | PayerOrders上限 | PayerOrdersFull |
| 27 | cleanup_payer_orders | 终态清理 |

### 10.6 Loyalty 迁移 (8 cases)

| # | 场景 | 校验点 |
|---|------|--------|
| 28 | 积分发放/转移/过期 | loyalty pallet 正确处理 |
| 29 | 购物余额消费 | order→Loyalty::consume_shopping_balance 成功 |
| 30 | 购物奖励发放 | order→Loyalty::reward_on_purchase 成功 |
| 31 | Token折扣抵扣 | order→Loyalty::redeem_for_discount 成功 |
| 32 | commission结算→写入购物余额 | commission→Loyalty::credit_shopping_balance |
| 33 | storage_prefix 一致性 | 升级前后数据完整 |
| 34 | 代付+折扣通过Loyalty | buyer折扣用Loyalty, payer锁定用EntityToken |
| 35 | migration后旧订单退款 | payer=None → 退给buyer |

---

## 十一、整合落地计划

### Phase 1: 基础层准备（低风险）

**目标**: 拆 common 文件，定义所有新 trait，旧接口 adapter 兼容

| 步骤 | 内容 | 涉及模块 |
|------|------|---------|
| 1.1 | common/src/lib.rs 拆为 types/ + traits/ + errors.rs + pagination.rs + admin.rs | common |
| 1.2 | traits/loyalty.rs: 定义 LoyaltyReadPort / LoyaltyWritePort | common |
| 1.3 | traits/order.rs: 新增 order_payer / order_fund_account（带默认实现） | common |
| 1.4 | traits/entity.rs: 新增 EntityTreasuryPort | common |
| 1.5 | traits/shop.rs: 新增 ShopFundPort | common |
| 1.6 | 对现有 EntityProvider / ShopProvider / EntityTokenProvider / ShoppingBalanceProvider 做 blanket impl adapter | common |
| 1.7 | 全量 re-export，外部 import 路径不变 | common |

**验收**: `cargo check --workspace` 通过，零运行时影响

### Phase 2: 代付功能 + loyalty 落地（中风险，核心阶段）

**目标**: 创建 loyalty crate + order 代付改造，一次完成

| 步骤 | 内容 | 涉及模块 |
|------|------|---------|
| **2a: loyalty crate** | | |
| 2a.1 | 创建 pallets/entity/loyalty/ crate 骨架 | loyalty |
| 2a.2 | 从 shop 迁出 10 个积分 ext + 3 storage（storage_prefix 保持原值） | shop → loyalty |
| 2a.3 | 从 commission/core 迁出 4 个购物余额 storage + 1 ext + ShoppingBalanceProvider impl | commission → loyalty |
| 2a.4 | 从 token 迁出 reward_on_purchase / redeem_for_discount（改为通过 AssetLedgerPort 调 pallet-assets） | token → loyalty |
| 2a.5 | loyalty 实现 LoyaltyReadPort + LoyaltyWritePort | loyalty |
| **2b: order 代付** | | |
| 2b.1 | Order 结构体新增 payer 字段 + Storage migration | order |
| 2b.2 | 新增 fund_account / is_order_participant helper | order |
| 2b.3 | 提取 do_place_order 内部函数，place_order 委托调用 | order |
| 2b.4 | do_place_order 中注入 payer 逻辑（资金锁定 + 索引写入 + PayerOrders） | order |
| 2b.5 | 新增 place_order_for + cleanup_payer_orders extrinsic | order |
| 2b.6 | 替换 12 处资金触点 + 3 处权限校验 | order |
| **2c: trait 切换** | | |
| 2c.1 | order Config: EntityToken(折扣/奖励) → Loyalty; ShoppingBalance → Loyalty | order |
| 2c.2 | order Config: EntityToken(reserve/unreserve/repatriate) 保留，改名为 AssetLedgerPort | order |
| 2c.3 | commission: 写购物余额改为调 LoyaltyWritePort::credit_shopping_balance | commission |
| 2c.4 | OrderProvider impl 新增 order_payer / order_fund_account | order |
| **2d: runtime** | | |
| 2d.1 | runtime 注册 loyalty pallet，更新 construct_runtime! | runtime |
| 2d.2 | runtime Order Config 绑定 Loyalty = LoyaltyPallet | runtime |
| **2e: 测试** | | |
| 2e.1 | 代付全路径测试（case 1-27） | order tests |
| 2e.2 | loyalty 迁移测试（case 28-35） | loyalty tests |
| 2e.3 | migration 测试（Order payer field） | order tests |
| 2e.4 | 全量回归: 现有 order/shop/commission 测试通过 | all |

**验收**: 全部测试通过 + `try-runtime` 验证 migration

### Phase 3: 内部模块化（低风险）

| 步骤 | 内容 |
|------|------|
| 3.1 | order/src/ 内部拆出 dispute.rs（7 个争议 ext） |
| 3.2 | commission/core/src/ 内部拆出 engine.rs / withdraw.rs / settlement.rs |
| 3.3 | 渐进切换残余大 Provider → 细粒度 Port |

### Phase 4: 治理归口 + 资金规则（低风险）

| 步骤 | 内容 |
|------|------|
| 4.1 | governance ProposalType 按领域分组 |
| 4.2 | 提案执行改为调用 XxxGovernancePort |
| 4.3 | 统一资金保护、阈值和预警规则 |

---

## 十二、改动量汇总

| 项目 | 新增代码 | 重构/迁移 | 说明 |
|------|---------|----------|------|
| common/ 拆文件 | ~50 | ~200（搬运） | Phase 1 |
| LoyaltyReadPort / LoyaltyWritePort | ~60 | — | Phase 1 |
| OrderQueryPort 扩展 | ~10 | — | Phase 1 |
| loyalty/ crate 骨架 | ~100 | — | Phase 2a |
| loyalty/ 迁入积分 | — | ~400（搬运） | Phase 2a |
| loyalty/ 迁入购物余额 | — | ~150（搬运） | Phase 2a |
| loyalty/ 迁入 reward/redeem | — | ~100（搬运） | Phase 2a |
| Order payer 字段 + migration | ~80 | — | Phase 2b |
| do_place_order 提取 | — | ~180（重构） | Phase 2b |
| place_order_for + cleanup_payer_orders | ~60 | — | Phase 2b |
| fund_account / is_order_participant | ~10 | — | Phase 2b |
| 资金触点替换 | ~15 | — | Phase 2b |
| PayerOrders storage + Event/Error | ~30 | — | Phase 2b |
| order Config trait 切换 | ~20 | ~30 | Phase 2c |
| runtime 配置 | ~30 | — | Phase 2d |
| 测试 | ~500 | — | Phase 2e |
| **合计** | **~965** | **~1060** | |

**净新增逻辑代码**: ~465 行（去除搬运/重构/测试）
**总 PR 体量**: ~2000 行（含测试+搬运），属于大型但结构清晰的 PR

---

## 十三、Extrinsic 统计（最终状态）

| 模块 | 当前 | 最终 | 变化 |
|------|------|------|------|
| registry | 25 | 25 | — |
| governance | 18 | 18 | — |
| kyc | 25 | 25 | — |
| disclosure | 39 | 39 | — |
| shop | 33 | **~23** | -10（积分迁出） |
| product | 9 | 9 | — |
| **order** | 23 | **25** | **+2**（place_order_for, cleanup_payer_orders） |
| review | 5 | 5 | — |
| member | 33 | 33 | — |
| **loyalty** | 0 | **~15** | **+15**（新模块） |
| token | 25 | **~23** | -2（reward/redeem 迁出） |
| market | 24+ | 24+ | — |
| tokensale | 27 | 27 | — |
| commission/core | 28 | **~27** | -1（use_shopping_balance 迁出） |
| commission/plugins | ~30 | ~30 | — |
| **合计** | ~300+ | **~302+** | 净+2（代付新增） |

---

## 十四、模块边界红线（7+2 条）

### 原 7 条（模块边界）

1. **order 不直接读写积分/购物余额 storage** → 调用 LoyaltyReadPort / LoyaltyWritePort
2. **commission 不保存消费余额** → 只结算"应得奖励"，通过 LoyaltyWritePort 写入
3. **shop 不拥有用户级激励账本** → 积分 storage 全归 loyalty
4. **token 不承担消费激励语义** → 纯资产账本（发行/限制/分红/锁仓）
5. **governance 不直接读写业务 storage** → 调用 XxxGovernancePort
6. **loyalty 不做返佣计算** → 只提供余额记账和消费服务
7. **member 不持有消费激励数据** → 只管身份、关系、等级

### 新增 2 条（代付相关）

8. **order 内所有资金操作通过 fund_account() 获取账户** → 杜绝 payer/buyer 遗漏
9. **激励操作始终以 &order.buyer 调用** → payer 不参与激励体系，即使代付也不改变激励对象

---

## 十五、总结

```
整合策略: 两项任务在 Phase 2 中一次完成，order 只改一轮

模块变更总览:
  +1 crate:  loyalty (15 ext)
  +2 ext:    order (place_order_for, cleanup_payer_orders)
  -10 ext:   shop (积分 → loyalty)
  -1 ext:    commission (购物余额 → loyalty)
  -2 helper: token (reward/redeem → loyalty)

核心保证:
  ✅ trait 语义清晰:
     Loyalty  = 折扣/余额/奖励（激励语义，始终 &buyer）
     EntityToken = reserve/unreserve/repatriate（资产语义，用 fund_account）

  ✅ 代付安全:
     fund_account() 统一资金操作收口
     payer ≠ seller 防刷单
     退款回 payer，激励归 buyer

  ✅ 模块边界:
     9 条红线确保职责不越界
     commission 只算不存，loyalty 只存不算

  ✅ 迁移安全:
     storage_prefix 保持原值 → 零链上数据搬运
     Order migration 仅新增 payer=None → 最小改动

  ✅ 一次到位:
     order trait 重绑定 + payer 注入同步完成
     不存在中间不一致状态
```
