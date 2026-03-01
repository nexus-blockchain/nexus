# Entity Token 支付与佣金双轨架构设计

> 方案 C：Entity Token 支付时，佣金也用 Entity Token 发放

## 一、资金流全景图

### NEX 支付（现有流程，不变）

```text
Buyer ──NEX──→ Escrow ──complete──→ Seller (seller_amount)
                 │                    │
                 └──→ Platform (fee)  └──→ Entity Account (Pool B, NEX)
                        │                       │
                        ├─→ Referrer (Pool A)    ├─→ Beneficiary₁ pending
                        └─→ Treasury             ├─→ Beneficiary₂ pending
                                                 └─→ Beneficiary₃ pending
                                                        │
                                                 withdraw_commission
                                                        │
                                                 ┌──→ NEX 提现
                                                 └──→ NEX 购物余额
```

### Entity Token 支付（新增流程）

```text
Buyer ─Token─→ reserve ──complete──→ repatriate → Seller (全额 Token)
                                        │
                                  ┌─────┘ (无平台费, 无 Pool A)
                                  │
                            seller Token × max_rate
                                  │
                            EntityTokenProvider::transfer
                                  │
                            Entity Account Token 余额 (Pool B only)
                                  │
                            ┌─→ Beneficiary₁ token_pending
                            ├─→ Beneficiary₂ token_pending
                            └─→ Beneficiary₃ token_pending
                                     │
                              withdraw_token_commission
                                     │
                              ┌──→ Entity Token 提现
                              └──→ Token 购物余额
```

### 混合支付

```text
place_order(Mixed { token_amount }):
  NEX 部分  → Escrow::lock_from (现有流程)
  Token 部分 → EntityTokenProvider::reserve (新流程)

do_complete_order:
  NEX 部分  → 现有佣金流程 (Pool A + Pool B in NEX)
  Token 部分 → Token 佣金流程 (Pool B only, in Token)
```

## 二、核心设计原则

| 原则 | 说明 |
|------|------|
| **双轨并行** | NEX 佣金和 Token 佣金完全独立存储、独立提现、独立偿付检查 |
| **零迁移** | 现有 Storage 不改结构，新增 Token 专用 Storage |
| **插件复用** | 4 个 CommissionPlugin 的 `calculate` 逻辑完全复用，只是输入金额和分发货币不同 |
| **无平台费** | Entity Token 支付不收平台费，不触发 Pool A（平台不希望积累各种 Entity Token） |
| **原子安全** | Token 支付 + Token 佣金分发在同一 extrinsic 内原子完成 |

## 三、类型变更

### 3.1 `pallet-entity-common` — 新增枚举

```rust
/// 支付方式
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq,
         TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum PaymentMethod {
    #[default]
    Native,           // 主网币 NEX
    EntityToken,      // Entity 代币全额支付
    Mixed,            // 混合支付（Order 中分别记录两部分金额）
}

/// 佣金货币类型
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Copy, PartialEq, Eq,
         TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum CommissionCurrency {
    #[default]
    Native,       // NEX
    EntityToken,  // Entity 代币
}
```

### 3.2 `pallet-entity-common` — 扩展 `OrderCommissionHandler`

```rust
pub trait OrderCommissionHandler<AccountId, Balance> {
    // 现有方法，不变
    fn on_order_completed(
        shop_id: u64, order_id: u64, buyer: &AccountId,
        order_amount: Balance, platform_fee: Balance,
    ) -> Result<(), DispatchError>;

    fn on_order_cancelled(order_id: u64) -> Result<(), DispatchError>;

    // ★ 新增：Entity Token 支付时的佣金处理
    fn on_order_completed_token(
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        token_amount: Balance,   // Token 支付金额
    ) -> Result<(), DispatchError> {
        Ok(()) // 默认 no-op，向后兼容
    }
}
```

### 3.3 `pallet-entity-shop` — Shop 支付策略

```rust
/// 店铺支付策略
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq,
         TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum PaymentPolicy {
    #[default]
    NativeOnly,                             // 仅主网币
    EntityTokenOnly { token_rate: u32 },    // 仅 Entity Token
    Both { token_rate: u32 },               // 两者均可（买家选择一种）
    MixedAllowed { token_rate: u32 },       // 允许混合支付
}
```

**`token_rate` 含义**: 每 1 NEX 等值需要多少 Token（基点，10000 = 1:1）

示例：`token_rate = 20000` → 2 Token = 1 NEX → 商品价 100 NEX = 200 Token

### 3.4 `pallet-commission-common` — CommissionRecord 加字段

```rust
pub struct CommissionRecord<AccountId, Balance, BlockNumber> {
    // ... 现有字段不变 ...
    pub entity_id: u64,
    pub shop_id: u64,
    pub order_id: u64,
    pub buyer: AccountId,
    pub beneficiary: AccountId,
    pub amount: Balance,
    pub commission_type: CommissionType,
    pub level: u8,
    pub status: CommissionStatus,
    pub created_at: BlockNumber,
    // ★ 新增
    pub currency: Option<CommissionCurrency>,  // None = Native（零迁移）
}
```

> **迁移策略**: 使用 `Option<CommissionCurrency>`，`None` 等同 `Native`，所有现有记录无需迁移。

## 四、Storage 变更

### 4.1 `pallet-entity-order` — Order 扩展

```rust
pub struct Order<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
    // ... 现有字段不变 ...

    // ★ 新增
    /// 支付方式
    pub payment_method: PaymentMethod,
    /// Token 支付金额（PaymentMethod::EntityToken 或 Mixed 时有值）
    pub token_amount: Balance,
}
```

### 4.2 `pallet-entity-shop` — Shop 扩展

```rust
pub struct Shop<...> {
    // ... 现有字段不变 ...

    // ★ 新增
    /// 支付策略
    pub payment_policy: PaymentPolicy,
}
```

### 4.3 `pallet-commission-core` — 新增 Token 佣金 Storage（6 项）

```rust
/// Entity Token 待提取佣金总额 entity_id -> Balance
#[pallet::storage]
pub type TokenPendingTotal<T: Config> = StorageMap<
    _, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery,
>;

/// Entity Token 购物余额总额 entity_id -> Balance
#[pallet::storage]
pub type TokenShoppingTotal<T: Config> = StorageMap<
    _, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery,
>;

/// 会员 Token 佣金统计 (entity_id, account) -> MemberCommissionStatsData
#[pallet::storage]
pub type MemberTokenCommissionStats<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, u64, Blake2_128Concat, T::AccountId,
    MemberCommissionStatsOf<T>, ValueQuery,
>;

/// 会员 Token 购物余额 (entity_id, account) -> Balance
#[pallet::storage]
pub type MemberTokenShoppingBalance<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, u64, Blake2_128Concat, T::AccountId,
    BalanceOf<T>, ValueQuery,
>;

/// 会员 Token 最后入账区块 (entity_id, account) -> BlockNumber
#[pallet::storage]
pub type MemberTokenLastCredited<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, u64, Blake2_128Concat, T::AccountId,
    BlockNumberFor<T>, ValueQuery,
>;

/// Entity Token 佣金统计 entity_id -> (total_distributed, total_orders)
#[pallet::storage]
pub type TokenCommissionTotals<T: Config> = StorageMap<
    _, Blake2_128Concat, u64, (BalanceOf<T>, u64), ValueQuery,
>;
```

## 五、流程变更详解

### 5.1 `place_order` — 双轨支付

```rust
pub fn place_order(
    origin: OriginFor<T>,
    product_id: u64,
    quantity: u32,
    shipping_cid: Option<Vec<u8>>,
    use_tokens: Option<BalanceOf<T>>,     // 现有：积分抵扣（仅 Native 模式）
    payment_method: PaymentMethod,         // ★ 新增
) -> DispatchResult {
    // ... 现有校验（商品、库存、店铺状态） ...

    // ★ 新增：校验 Shop 支付策略
    let payment_policy = T::ShopProvider::payment_policy(shop_id);
    match (&payment_method, &payment_policy) {
        (PaymentMethod::Native, PaymentPolicy::EntityTokenOnly { .. }) =>
            return Err(Error::<T>::PaymentMethodNotAllowed.into()),
        (PaymentMethod::EntityToken, PaymentPolicy::NativeOnly) =>
            return Err(Error::<T>::PaymentMethodNotAllowed.into()),
        (PaymentMethod::Mixed, PaymentPolicy::NativeOnly) |
        (PaymentMethod::Mixed, PaymentPolicy::EntityTokenOnly { .. }) =>
            return Err(Error::<T>::PaymentMethodNotAllowed.into()),
        (PaymentMethod::Mixed, PaymentPolicy::Both { .. }) =>
            return Err(Error::<T>::PaymentMethodNotAllowed.into()),
        _ => {}
    };

    let total_amount = price.saturating_mul(quantity.into());  // NEX 计价
    let entity_id = T::ShopProvider::shop_entity_id(shop_id)
        .ok_or(Error::<T>::ShopNotFound)?;

    let (final_nex_amount, token_amount, platform_fee) = match payment_method {
        PaymentMethod::Native => {
            // 现有流程：积分抵扣 + Escrow 托管
            let mut final_amount = total_amount;
            if let Some(tokens) = use_tokens {
                if !tokens.is_zero() && T::EntityToken::is_token_enabled(entity_id) {
                    let discount = T::EntityToken::redeem_for_discount(
                        entity_id, &buyer, tokens
                    )?;
                    final_amount = final_amount.saturating_sub(discount);
                }
            }
            ensure!(!final_amount.is_zero(), Error::<T>::InvalidAmount);
            let fee = final_amount
                .saturating_mul(T::PlatformFeeRate::get().into()) / 10000u32.into();
            T::Escrow::lock_from(&buyer, order_id, final_amount)?;
            (final_amount, BalanceOf::<T>::zero(), fee)
        }
        PaymentMethod::EntityToken => {
            // ★ Token 全额支付
            ensure!(
                T::EntityToken::is_token_enabled(entity_id),
                Error::<T>::PaymentMethodNotAllowed
            );
            let token_rate = payment_policy.token_rate()
                .ok_or(Error::<T>::PaymentMethodNotAllowed)?;
            let token_amt = total_amount
                .saturating_mul(token_rate.into()) / 10000u32.into();
            ensure!(!token_amt.is_zero(), Error::<T>::InvalidAmount);

            T::EntityToken::reserve(entity_id, &buyer, token_amt)?;
            // 不走 Escrow, 不收平台费
            (BalanceOf::<T>::zero(), token_amt, BalanceOf::<T>::zero())
        }
        PaymentMethod::Mixed => {
            // ★ 混合支付
            ensure!(
                T::EntityToken::is_token_enabled(entity_id),
                Error::<T>::PaymentMethodNotAllowed
            );
            let token_pay = use_tokens.ok_or(Error::<T>::InvalidAmount)?;
            ensure!(!token_pay.is_zero(), Error::<T>::InvalidAmount);
            let token_rate = payment_policy.token_rate()
                .ok_or(Error::<T>::PaymentMethodNotAllowed)?;

            // Token 部分等值的 NEX
            let token_nex_value = token_pay
                .saturating_mul(10000u32.into()) / token_rate.into();
            let native_amount = total_amount.saturating_sub(token_nex_value);
            ensure!(!native_amount.is_zero(), Error::<T>::InvalidAmount);

            let fee = native_amount
                .saturating_mul(T::PlatformFeeRate::get().into()) / 10000u32.into();

            T::EntityToken::reserve(entity_id, &buyer, token_pay)?;
            T::Escrow::lock_from(&buyer, order_id, native_amount)?;
            (native_amount, token_pay, fee)
        }
    };

    let order = Order {
        // ... 现有字段 ...
        total_amount: final_nex_amount,
        platform_fee,
        payment_method,        // ★ 新增
        token_amount,          // ★ 新增
        // ...
    };

    // ... 写入存储、事件、超时队列 ...
}
```

### 5.2 `do_complete_order` — 双轨释放 + 双轨佣金

```rust
fn do_complete_order(order_id: u64, order: &OrderOf<T>) -> DispatchResult {
    let entity_id = T::ShopProvider::shop_entity_id(order.shop_id)
        .ok_or(Error::<T>::ShopNotFound)?;

    // ──── NEX 部分（与现有逻辑相同） ────
    if !order.total_amount.is_zero() {
        let seller_amount = order.total_amount.saturating_sub(order.platform_fee);

        // Escrow 释放
        T::Escrow::transfer_from_escrow(order_id, &order.seller, seller_amount)?;
        if !order.platform_fee.is_zero() {
            T::Escrow::transfer_from_escrow(
                order_id, &T::PlatformAccount::get(), order.platform_fee
            )?;
        }

        // NEX 佣金（Pool A + Pool B）
        if T::CommissionHandler::on_order_completed(
            order.shop_id, order_id, &order.buyer,
            order.total_amount, order.platform_fee,
        ).is_err() {
            Self::deposit_event(Event::OrderOperationFailed {
                order_id, operation: OrderOperation::CommissionComplete
            });
        }
    }

    // ──── Entity Token 部分（★ 新增） ────
    if !order.token_amount.is_zero() {
        // Token 释放给卖家
        T::EntityToken::repatriate_reserved(
            entity_id, &order.buyer, &order.seller, order.token_amount
        )?;

        // Token 佣金（仅 Pool B，无平台费）
        if T::CommissionHandler::on_order_completed_token(
            order.shop_id, order_id, &order.buyer, order.token_amount,
        ).is_err() {
            Self::deposit_event(Event::OrderOperationFailed {
                order_id, operation: OrderOperation::CommissionComplete
            });
        }
    }

    // Token 购物奖励：仅对 NEX 部分发放（避免 Token 支付循环发 Token）
    if !order.total_amount.is_zero() {
        if let Some(eid) = T::ShopProvider::shop_entity_id(order.shop_id) {
            if T::EntityToken::reward_on_purchase(
                eid, &order.buyer, order.total_amount,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed {
                    order_id, operation: OrderOperation::TokenReward
                });
            }
        }
    }

    // ... 更新订单状态、店铺统计、平台统计 ...
    Ok(())
}
```

### 5.3 `cancel_order` / `approve_refund` — 双轨退款

```rust
// 退款部分（在 cancel_order、approve_refund、process_expired_orders 中统一处理）

// NEX 部分退款
if !order.total_amount.is_zero() {
    T::Escrow::refund_all(order_id, &order.buyer)?;
}

// Token 部分退款
if !order.token_amount.is_zero() {
    let entity_id = T::ShopProvider::shop_entity_id(order.shop_id)
        .ok_or(Error::<T>::ShopNotFound)?;
    T::EntityToken::unreserve(entity_id, &order.buyer, order.token_amount);
}

// 佣金取消（同时处理两种货币的记录）
if T::CommissionHandler::on_order_cancelled(order_id).is_err() {
    Self::deposit_event(Event::OrderOperationFailed {
        order_id, operation: OrderOperation::CommissionCancel
    });
}
```

### 5.4 `process_token_commission` — Token 佣金引擎（commission-core 新增）

```rust
/// Token 佣金引擎（仅 Pool B，无平台费/推荐人）
///
/// 与 process_commission 对称，但：
/// - 不处理平台费（Entity Token 支付不收平台费）
/// - 不触发 Pool A（招商推荐人奖金来自平台费）
/// - 使用 EntityTokenProvider::transfer 替代 T::Currency::transfer
/// - 写入 Token 专用 Storage
pub fn process_token_commission(
    shop_id: u64,
    order_id: u64,
    buyer: &T::AccountId,
    token_amount: BalanceOf<T>,
) -> DispatchResult {
    let entity_id = Self::resolve_entity_id(shop_id)?;

    let config = match CommissionConfigs::<T>::get(entity_id) {
        Some(c) if c.enabled => c,
        _ => return Ok(()), // 未配置佣金，Token 全归卖家
    };

    let seller = T::ShopProvider::shop_owner(shop_id)
        .ok_or(Error::<T>::ShopNotFound)?;
    let entity_account = T::EntityProvider::entity_account(entity_id);
    let now = <frame_system::Pallet<T>>::block_number();

    // 使用 NEX 统计中的 order_count（两轨共享买家订单计数）
    let buyer_stats = MemberCommissionStats::<T>::get(entity_id, buyer);
    let is_first_order = buyer_stats.order_count == 0;
    let enabled_modes = config.enabled_modes;

    // 佣金池 = 卖家 Token 收入 × max_commission_rate
    let max_commission = token_amount
        .saturating_mul(config.max_commission_rate.into())
        / 10000u32.into();

    // 检查卖家 Token 余额（刚 repatriate 收到，原子操作内必定够）
    let seller_token_balance = T::EntityToken::token_balance(entity_id, &seller);
    let mut remaining = max_commission.min(seller_token_balance);

    if remaining.is_zero() {
        return Ok(());
    }

    let initial_remaining = remaining;

    // ★ 复用完全相同的 4 个插件计算逻辑
    // 插件 calculate() 只关心金额数值，不关心货币类型

    // 1. Referral Plugin
    let (outputs, new_remaining) = T::ReferralPlugin::calculate(
        entity_id, shop_id, buyer, token_amount,
        remaining, enabled_modes, is_first_order, buyer_stats.order_count,
    );
    remaining = new_remaining;
    for output in outputs {
        Self::credit_token_commission(
            entity_id, shop_id, order_id, buyer,
            &output.beneficiary, output.amount,
            output.commission_type, output.level, now,
        )?;
    }

    // 2. LevelDiff Plugin
    let (outputs, new_remaining) = T::LevelDiffPlugin::calculate(
        entity_id, shop_id, buyer, token_amount,
        remaining, enabled_modes, is_first_order, buyer_stats.order_count,
    );
    remaining = new_remaining;
    for output in outputs {
        Self::credit_token_commission(
            entity_id, shop_id, order_id, buyer,
            &output.beneficiary, output.amount,
            output.commission_type, output.level, now,
        )?;
    }

    // 3. SingleLine Plugin
    let (outputs, new_remaining) = T::SingleLinePlugin::calculate(
        entity_id, shop_id, buyer, token_amount,
        remaining, enabled_modes, is_first_order, buyer_stats.order_count,
    );
    remaining = new_remaining;
    for output in outputs {
        Self::credit_token_commission(
            entity_id, shop_id, order_id, buyer,
            &output.beneficiary, output.amount,
            output.commission_type, output.level, now,
        )?;
    }

    // 4. Team Plugin
    let (outputs, new_remaining) = T::TeamPlugin::calculate(
        entity_id, shop_id, buyer, token_amount,
        remaining, enabled_modes, is_first_order, buyer_stats.order_count,
    );
    remaining = new_remaining;
    for output in outputs {
        Self::credit_token_commission(
            entity_id, shop_id, order_id, buyer,
            &output.beneficiary, output.amount,
            output.commission_type, output.level, now,
        )?;
    }

    let total_distributed = initial_remaining.saturating_sub(remaining);

    // 将佣金 Token 从卖家转到 Entity 账户（集中管理）
    if !total_distributed.is_zero() {
        T::EntityToken::transfer(
            entity_id, &seller, &entity_account, total_distributed
        )?;

        Self::deposit_event(Event::CommissionFundsTransferred {
            entity_id, shop_id, amount: total_distributed,
        });
    }

    // 更新 Token 佣金统计
    TokenCommissionTotals::<T>::mutate(entity_id, |(total, orders)| {
        *total = total.saturating_add(total_distributed);
        *orders = orders.saturating_add(1);
    });

    Ok(())
}
```

### 5.5 `credit_token_commission` — Token 佣金记账

```rust
/// 记录 Token 佣金（写入 Token 专用 Storage）
pub fn credit_token_commission(
    entity_id: u64, shop_id: u64, order_id: u64,
    buyer: &T::AccountId, beneficiary: &T::AccountId,
    amount: BalanceOf<T>, commission_type: CommissionType,
    level: u8, now: BlockNumberFor<T>,
) -> DispatchResult {
    let record = CommissionRecord {
        entity_id, shop_id, order_id,
        buyer: buyer.clone(),
        beneficiary: beneficiary.clone(),
        amount, commission_type, level,
        status: CommissionStatus::Pending,
        created_at: now,
        currency: Some(CommissionCurrency::EntityToken),  // ★ 标记为 Token 佣金
    };

    // 与 NEX 佣金共享同一个 OrderCommissionRecords（通过 currency 字段区分）
    OrderCommissionRecords::<T>::try_mutate(order_id, |records| {
        records.try_push(record).map_err(|_| Error::<T>::RecordsFull)
    })?;

    // ★ 写入 Token 专用统计
    MemberTokenCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
        stats.total_earned = stats.total_earned.saturating_add(amount);
        stats.pending = stats.pending.saturating_add(amount);
    });

    MemberTokenLastCredited::<T>::insert(entity_id, beneficiary, now);

    TokenPendingTotal::<T>::mutate(entity_id, |total| {
        *total = total.saturating_add(amount);
    });

    Self::deposit_event(Event::CommissionDistributed {
        entity_id, shop_id, order_id,
        beneficiary: beneficiary.clone(),
        amount, commission_type, level,
    });

    Ok(())
}
```

### 5.6 `withdraw_token_commission` — Token 佣金提现（新 extrinsic）

```rust
/// 提取 Entity Token 佣金
///
/// 与 withdraw_commission 对称，但：
/// - 从 MemberTokenCommissionStats 扣减
/// - 偿付检查基于 EntityTokenProvider::token_balance
/// - 转账使用 EntityTokenProvider::transfer
/// - 购物余额写入 MemberTokenShoppingBalance
#[pallet::call_index(7)]
#[pallet::weight(Weight::from_parts(80_000_000, 6_000))]
pub fn withdraw_token_commission(
    origin: OriginFor<T>,
    shop_id: u64,
    amount: Option<BalanceOf<T>>,
    requested_repurchase_rate: Option<u16>,
    repurchase_target: Option<T::AccountId>,
) -> DispatchResult {
    let who = ensure_signed(origin)?;
    let entity_id = Self::resolve_entity_id(shop_id)?;
    let target = repurchase_target.unwrap_or_else(|| who.clone());

    MemberTokenCommissionStats::<T>::try_mutate(
        entity_id, &who, |stats| -> DispatchResult
    {
        let total_amount = amount.unwrap_or(stats.pending);
        ensure!(stats.pending >= total_amount, Error::<T>::InsufficientCommission);
        ensure!(!total_amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

        // 推荐关系校验（复用与 NEX 提现相同的逻辑）
        if target != who {
            if T::MemberProvider::is_member(shop_id, &target) {
                let referrer = T::MemberProvider::get_referrer(shop_id, &target);
                ensure!(referrer.as_ref() == Some(&who), Error::<T>::NotDirectReferral);
            } else {
                T::MemberProvider::auto_register(shop_id, &target, Some(who.clone()))
                    .map_err(|_| Error::<T>::AutoRegisterFailed)?;
            }
        }

        // 冻结期检查
        if let Some(config) = CommissionConfigs::<T>::get(entity_id) {
            if config.withdrawal_cooldown > 0 {
                let now = <frame_system::Pallet<T>>::block_number();
                let last_credited = MemberTokenLastCredited::<T>::get(entity_id, &who);
                let cooldown: BlockNumberFor<T> = config.withdrawal_cooldown.into();
                let earliest = last_credited.saturating_add(cooldown);
                ensure!(now >= earliest, Error::<T>::WithdrawalCooldownNotMet);
            }
        }

        // 复购分配（复用 calc_withdrawal_split 逻辑）
        let split = Self::calc_withdrawal_split(
            entity_id, shop_id, &who, total_amount, requested_repurchase_rate,
        );

        // ★ 偿付安全检查（Token 版本）
        let entity_account = T::EntityProvider::entity_account(entity_id);
        let entity_token_balance = T::EntityToken::token_balance(
            entity_id, &entity_account
        );
        let remaining_pending = TokenPendingTotal::<T>::get(entity_id)
            .saturating_sub(total_amount);
        let total_to_shopping = split.repurchase.saturating_add(split.bonus);
        let new_shopping_total = TokenShoppingTotal::<T>::get(entity_id)
            .saturating_add(total_to_shopping);
        let required_reserve = remaining_pending.saturating_add(new_shopping_total);
        ensure!(
            entity_token_balance >= split.withdrawal.saturating_add(required_reserve),
            Error::<T>::InsufficientCommission
        );

        // ★ Token 转账提现（Entity 账户 → 用户）
        if !split.withdrawal.is_zero() {
            T::EntityToken::transfer(
                entity_id, &entity_account, &who, split.withdrawal
            )?;
        }

        // Token 购物余额
        if !total_to_shopping.is_zero() {
            MemberTokenShoppingBalance::<T>::mutate(entity_id, &target, |b| {
                *b = b.saturating_add(total_to_shopping);
            });
            TokenShoppingTotal::<T>::mutate(entity_id, |total| {
                *total = total.saturating_add(total_to_shopping);
            });
        }

        // 更新统计
        stats.pending = stats.pending.saturating_sub(total_amount);
        stats.withdrawn = stats.withdrawn.saturating_add(split.withdrawal);
        stats.repurchased = stats.repurchased.saturating_add(split.repurchase);

        TokenPendingTotal::<T>::mutate(entity_id, |total| {
            *total = total.saturating_sub(total_amount);
        });

        Self::deposit_event(Event::TieredWithdrawal {
            entity_id,
            account: who.clone(),
            withdrawn_amount: split.withdrawal,
            repurchase_amount: split.repurchase,
            bonus_amount: split.bonus,
        });

        Ok(())
    })
}
```

### 5.7 `cancel_commission` — 双货币感知

```rust
pub fn cancel_commission(order_id: u64) -> DispatchResult {
    let records = OrderCommissionRecords::<T>::get(order_id);
    let platform_account = T::PlatformAccount::get();

    // 按 (entity_id, shop_id, is_platform, currency) 分组汇总
    let mut refund_groups: Vec<(u64, u64, bool, CommissionCurrency, BalanceOf<T>)> = Vec::new();

    for record in records.iter() {
        if record.status == CommissionStatus::Pending {
            let is_platform = record.commission_type == CommissionType::EntityReferral;
            let currency = record.currency.unwrap_or(CommissionCurrency::Native);
            if let Some(entry) = refund_groups.iter_mut().find(
                |(e, s, p, c, _)| *e == record.entity_id && *s == record.shop_id
                    && *p == is_platform && *c == currency
            ) {
                entry.4 = entry.4.saturating_add(record.amount);
            } else {
                refund_groups.push((
                    record.entity_id, record.shop_id,
                    is_platform, currency, record.amount
                ));
            }
        }
    }

    // 尝试转账
    let mut refund_succeeded: Vec<(u64, u64, bool, CommissionCurrency)> = Vec::new();

    for (entity_id, shop_id, is_platform, currency, refund_amount) in refund_groups.iter() {
        if refund_amount.is_zero() {
            refund_succeeded.push((*entity_id, *shop_id, *is_platform, *currency));
            continue;
        }

        let entity_account = T::EntityProvider::entity_account(*entity_id);

        match currency {
            CommissionCurrency::EntityToken => {
                // ★ Token 退款：Entity 账户 → 卖家
                let seller = match T::ShopProvider::shop_owner(*shop_id) {
                    Some(s) => s,
                    None => {
                        Self::deposit_event(Event::CommissionRefundFailed { .. });
                        continue;
                    }
                };
                if T::EntityToken::transfer(
                    *entity_id, &entity_account, &seller, *refund_amount
                ).is_ok() {
                    refund_succeeded.push((
                        *entity_id, *shop_id, *is_platform, *currency
                    ));
                } else {
                    Self::deposit_event(Event::CommissionRefundFailed { .. });
                }
            }
            CommissionCurrency::Native => {
                // 现有 NEX 退款逻辑（不变）
                let refund_target = if *is_platform {
                    platform_account.clone()
                } else {
                    match T::ShopProvider::shop_owner(*shop_id) {
                        Some(seller) => seller,
                        None => { /* ... */ continue; }
                    }
                };
                if T::Currency::transfer(
                    &entity_account, &refund_target, *refund_amount,
                    ExistenceRequirement::KeepAlive,
                ).is_ok() {
                    refund_succeeded.push((
                        *entity_id, *shop_id, *is_platform, *currency
                    ));
                } else {
                    Self::deposit_event(Event::CommissionRefundFailed { .. });
                }
            }
        }
    }

    // 更新统计时按 currency 分流
    OrderCommissionRecords::<T>::mutate(order_id, |records| {
        for record in records.iter_mut() {
            if record.status == CommissionStatus::Pending {
                let is_platform = record.commission_type == CommissionType::EntityReferral;
                let currency = record.currency.unwrap_or(CommissionCurrency::Native);
                if refund_succeeded.iter().any(|(e, s, p, c)|
                    *e == record.entity_id && *s == record.shop_id
                    && *p == is_platform && *c == currency
                ) {
                    match currency {
                        CommissionCurrency::EntityToken => {
                            // ★ Token 统计
                            MemberTokenCommissionStats::<T>::mutate(
                                record.entity_id, &record.beneficiary, |stats| {
                                    stats.pending = stats.pending
                                        .saturating_sub(record.amount);
                                    stats.total_earned = stats.total_earned
                                        .saturating_sub(record.amount);
                                }
                            );
                            TokenPendingTotal::<T>::mutate(record.entity_id, |total| {
                                *total = total.saturating_sub(record.amount);
                            });
                        }
                        CommissionCurrency::Native => {
                            // 现有 NEX 统计（不变）
                            MemberCommissionStats::<T>::mutate(
                                record.entity_id, &record.beneficiary, |stats| {
                                    stats.pending = stats.pending
                                        .saturating_sub(record.amount);
                                    stats.total_earned = stats.total_earned
                                        .saturating_sub(record.amount);
                                }
                            );
                            ShopPendingTotal::<T>::mutate(record.entity_id, |total| {
                                *total = total.saturating_sub(record.amount);
                            });
                        }
                    }
                    record.status = CommissionStatus::Cancelled;
                }
            }
        }
    });

    // 退还国库部分（仅 NEX，Token 支付无国库转入）
    // ... 现有逻辑不变 ...
}
```

## 六、Config 变更

### 6.1 `pallet-commission-core` Config — 新增 EntityToken 依赖

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    // ... 现有配置不变 ...

    /// ★ 新增：实体代币接口（用于 Token 佣金分发和提现）
    type EntityToken: EntityTokenProvider<Self::AccountId, BalanceOf<Self>>;
}
```

### 6.2 Runtime 配置

```rust
// runtime/src/configs/mod.rs
impl pallet_commission_core::Config for Runtime {
    // ... 现有配置不变 ...
    type EntityToken = EntityToken;  // ★ 新增绑定
}
```

### 6.3 `ShopProvider` trait 扩展

```rust
// pallet-entity-common 中
pub trait ShopProvider<AccountId> {
    // ... 现有方法不变 ...

    /// ★ 新增：查询店铺支付策略
    fn payment_policy(shop_id: u64) -> PaymentPolicy {
        PaymentPolicy::NativeOnly // 默认值，向后兼容
    }
}
```

## 七、改动清单

| 文件 | 变更 | 行数估计 |
|------|------|----------|
| `pallets/entity/common/src/lib.rs` | `PaymentMethod`, `CommissionCurrency`, `PaymentPolicy` 枚举 + `OrderCommissionHandler` 扩展 + `ShopProvider` 扩展 | ~50 |
| `pallets/entity/shop/src/lib.rs` | `Shop` 新字段 `payment_policy` + `set_payment_policy` extrinsic + `ShopProvider` impl | ~60 |
| `pallets/entity/order/src/lib.rs` | `Order` 新字段 + `place_order` 双轨 + `do_complete_order` 双轨 + cancel/refund/expired 双轨 | ~150 |
| `pallets/entity/commission/common/src/lib.rs` | `CommissionRecord` 加 `currency` 字段 | ~10 |
| `pallets/entity/commission/core/src/lib.rs` | 6 新 Storage + `process_token_commission` + `credit_token_commission` + `withdraw_token_commission` + `cancel_commission` 扩展 + Config 扩展 | ~250 |
| `runtime/src/configs/mod.rs` | commission-core Config 加 `EntityToken` 绑定 | ~5 |
| **合计** | | **~525** |

## 八、迁移策略

| 项目 | 策略 |
|------|------|
| `CommissionRecord.currency` | `Option<CommissionCurrency>`，`None` = `Native`，**零迁移** |
| `Order.payment_method` | 新字段 `Default::default()` = `Native`，需 storage migration |
| `Order.token_amount` | 新字段默认 0，需 storage migration |
| `Shop.payment_policy` | 新字段 `Default::default()` = `NativeOnly`，需 storage migration |
| Token 专用 Storage（6 项） | 全部新增 `ValueQuery` / `StorageMap`，**无迁移** |

> 建议: Order 和 Shop 的 migration 可合并为一次 runtime upgrade。

## 九、安全考量

| 风险 | 应对 |
|------|------|
| **卖家收到 Token 后立即转走，佣金扣款失败** | 原子操作：`repatriate_reserved` + `process_token_commission` 在同一 extrinsic 内完成，卖家无机会在中间操作 |
| **Entity Token 供应量不足支付佣金** | 佣金从卖家刚收到的 Token 中扣除，不依赖额外铸造 |
| **Token 佣金偿付安全** | `withdraw_token_commission` 检查 Entity 账户 Token 余额 >= `token_pending_total + token_shopping_total` |
| **`EntityTokenProvider::transfer` 绕过转账限制** | 已知设计决策（M2 审计记录），佣金分发需要此特性 |
| **Mixed 支付的 `buyer_order_count` 重复计数** | NEX 部分和 Token 部分共享同一个 `order_count`（在 NEX 的 `MemberCommissionStats` 中），仅在 NEX 佣金流程中 +1；纯 Token 支付时在 Token 流程中 +1 |
| **Token 购物余额使用** | 新增 `use_token_shopping_balance`，从 `MemberTokenShoppingBalance` 和 `TokenShoppingTotal` 扣减 |
| **佣金配置共享** | `CoreCommissionConfig`（enabled_modes, max_commission_rate）NEX 和 Token 共用同一套配置，简化管理 |

## 十、前端交互流程

```text
1. 查询 Shop → payment_policy
   ├─ NativeOnly       → 隐藏 Token 支付选项
   ├─ EntityTokenOnly  → 仅显示 Token 支付
   ├─ Both             → 显示切换按钮（NEX / Token）
   └─ MixedAllowed     → 显示滑块（调整 Token/NEX 比例）

2. Token 支付时计算 Token 数量
   token_amount = nex_price × token_rate / 10000
   显示: "100 NEX ≈ 200 SHOP-TOKEN"

3. 提现页面
   显示两个余额:
   ├─ NEX 佣金待提取:   xxx NEX     [提现]
   └─ Token 佣金待提取: xxx TOKEN   [提现]
   分别提现，提现规则（复购/冻结期）相同

4. 购物余额页面
   ├─ NEX 购物余额:   xxx NEX     [去消费]
   └─ Token 购物余额: xxx TOKEN   [去消费]
```

## 十一、版本与时间线

| 阶段 | 内容 | 预估工作量 |
|------|------|------------|
| Phase 1 | 类型定义 + Shop 支付策略 + Order 双轨支付 | 2-3 天 |
| Phase 2 | Commission 双轨引擎 + Token 佣金记账 | 2-3 天 |
| Phase 3 | Token 提现 + cancel 双轨 + 偿付安全 | 1-2 天 |
| Phase 4 | Storage migration + 集成测试 | 1-2 天 |
| **合计** | | **6-10 天** |
