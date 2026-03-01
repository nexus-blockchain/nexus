# 方案 A：全插件多资产化 — 详细开发文档

> 目标：让 Entity Token 订单的佣金经过与 NEX 完全对称的插件管线分配，
> 包括 Referral / LevelDiff / SingleLine / Team 四个插件 + 沉淀池。

---

## 1. 架构概览

### 1.1 核心设计思路

**不修改现有 NEX 管线**，通过以下方式并行添加 Token 管线：

1. 新增 `TokenCommissionPlugin` trait（与 `CommissionPlugin` 签名相同，Balance → TokenBalance）
2. 每个插件提取计算逻辑为泛型内部函数 `do_calculate<B>()`，两个 trait impl 共用
3. Core 新增 `process_token_commission` 调度 Token 插件管线
4. 新增 Token 记账/提现/取消系统（独立于 NEX，更简洁）

### 1.2 对称管线

```
NEX 订单                              Token 订单
    │                                      │
    ▼                                      ▼
process_commission()               process_token_commission()
    │                                      │
    ├─ ReferralPlugin::calculate()     ├─ ReferralPlugin::calculate_token()
    ├─ LevelDiffPlugin::calculate()    ├─ LevelDiffPlugin::calculate_token()
    ├─ SingleLinePlugin::calculate()   ├─ SingleLinePlugin::calculate_token()
    ├─ TeamPlugin::calculate()         ├─ TeamPlugin::calculate_token()
    │                                      │
    ├─ credit_commission() [NEX]       ├─ credit_token_commission() [Token]
    ├─ → UnallocatedPool [NEX]         ├─ → UnallocatedTokenPool [Token]
    │                                      │
    ▼                                      ▼
withdraw_commission()              withdraw_token_commission()
    (NEX + 复购分流)                   (Token 全额提取，无复购)
```

---

## 2. commission-common 改动

### 2.1 新增 TokenCommissionPlugin trait

```rust
/// Token 佣金插件接口（与 CommissionPlugin 对称）
pub trait TokenCommissionPlugin<AccountId, TokenBalance> {
    fn calculate_token(
        entity_id: u64,
        buyer: &AccountId,
        order_amount: TokenBalance,
        remaining: TokenBalance,
        enabled_modes: CommissionModes,
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<AccountId, TokenBalance>>, TokenBalance);
}

/// 空实现
impl<AccountId, TokenBalance> TokenCommissionPlugin<AccountId, TokenBalance> for () {
    fn calculate_token(
        _: u64, _: &AccountId, _: TokenBalance, remaining: TokenBalance,
        _: CommissionModes, _: bool, _: u32,
    ) -> (Vec<CommissionOutput<AccountId, TokenBalance>>, TokenBalance) {
        (Vec::new(), remaining)
    }
}
```

### 2.2 新增 Token 记账结构

```rust
/// Token 佣金记录
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct TokenCommissionRecord<AccountId, TokenBalance, BlockNumber> {
    pub entity_id: u64,
    pub order_id: u64,
    pub buyer: AccountId,
    pub beneficiary: AccountId,
    pub amount: TokenBalance,
    pub commission_type: CommissionType,
    pub level: u8,
    pub status: CommissionStatus,
    pub created_at: BlockNumber,
}

/// Token 佣金统计
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct MemberTokenCommissionStatsData<TokenBalance: Default> {
    pub total_earned: TokenBalance,
    pub pending: TokenBalance,
    pub withdrawn: TokenBalance,
    pub order_count: u32,
    // 注意：无 repurchased 字段（Token 无复购概念）
}
```

### 2.3 新增 TokenPoolBalanceProvider

```rust
pub trait TokenPoolBalanceProvider<TokenBalance> {
    fn token_pool_balance(entity_id: u64) -> TokenBalance;
    fn deduct_token_pool(entity_id: u64, amount: TokenBalance) -> Result<(), DispatchError>;
}

impl<TokenBalance: Default> TokenPoolBalanceProvider<TokenBalance> for () {
    fn token_pool_balance(_: u64) -> TokenBalance { TokenBalance::default() }
    fn deduct_token_pool(_: u64, _: TokenBalance) -> Result<(), DispatchError> { Ok(()) }
}
```

### 2.4 新增 TokenCommissionProvider trait

```rust
pub trait TokenCommissionProvider<AccountId, TokenBalance> {
    fn process_token_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        token_order_amount: TokenBalance,
    ) -> Result<(), DispatchError>;

    fn cancel_token_commission(order_id: u64) -> Result<(), DispatchError>;

    fn pending_token_commission(entity_id: u64, account: &AccountId) -> TokenBalance;
}
```

---

## 3. 插件改造（4 个插件统一模式）

### 3.1 改造模式

每个插件按以下模式改造，以 **referral** 为例：

#### Step 1: 提取泛型计算内核

```rust
// pallets/entity/commission/referral/src/lib.rs

impl<T: Config> Pallet<T> {
    /// 泛型计算内核——对任意 Balance 类型执行相同逻辑
    fn do_calculate_generic<B>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: B,
        remaining: B,
        enabled_modes: CommissionModes,
        is_first_order: bool,
        buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<T::AccountId, B>>, B)
    where
        B: AtLeast32BitUnsigned + Copy + Saturating + Zero + From<u32>,
    {
        let config = match ReferralConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = Vec::new();

        if enabled_modes.contains(CommissionModes::DIRECT_REWARD) {
            Self::process_direct_reward_generic::<B>(
                entity_id, buyer, order_amount,
                &mut remaining, &config.direct_reward, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::MULTI_LEVEL) {
            Self::process_multi_level_generic::<B>(
                entity_id, buyer, order_amount,
                &mut remaining, &config.multi_level, &mut outputs,
            );
        }

        // FIXED_AMOUNT 和 FIRST_ORDER(use_amount=true) 跳过
        // 因为固定金额以 NEX 计价，不适用于 Token
        // rate-based 的 FIRST_ORDER / REPEAT_PURCHASE 仍然支持

        if enabled_modes.contains(CommissionModes::FIRST_ORDER) && is_first_order {
            Self::process_first_order_generic::<B>(
                entity_id, buyer, order_amount,
                &mut remaining, &config.first_order, &mut outputs,
            );
        }

        if enabled_modes.contains(CommissionModes::REPEAT_PURCHASE) {
            Self::process_repeat_purchase_generic::<B>(
                entity_id, buyer, order_amount,
                &mut remaining, &config.repeat_purchase,
                buyer_order_count, &mut outputs,
            );
        }

        (outputs, remaining)
    }
}
```

#### Step 2: 现有 CommissionPlugin 委托给泛型内核

```rust
impl<T: Config> CommissionPlugin<T::AccountId, BalanceOf<T>> for Pallet<T> {
    fn calculate(
        entity_id: u64, buyer: &T::AccountId, order_amount: BalanceOf<T>,
        remaining: BalanceOf<T>, enabled_modes: CommissionModes,
        is_first_order: bool, buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>, BalanceOf<T>) {
        Self::do_calculate_generic::<BalanceOf<T>>(
            entity_id, buyer, order_amount, remaining,
            enabled_modes, is_first_order, buyer_order_count,
        )
    }
}
```

#### Step 3: 新增 TokenCommissionPlugin 实现

```rust
impl<T: Config> TokenCommissionPlugin<T::AccountId, TokenBalanceOf<T>> for Pallet<T>
where
    TokenBalanceOf<T>: AtLeast32BitUnsigned + Copy + Saturating + Zero + From<u32>,
{
    fn calculate_token(
        entity_id: u64, buyer: &T::AccountId, order_amount: TokenBalanceOf<T>,
        remaining: TokenBalanceOf<T>, enabled_modes: CommissionModes,
        is_first_order: bool, buyer_order_count: u32,
    ) -> (Vec<CommissionOutput<T::AccountId, TokenBalanceOf<T>>>, TokenBalanceOf<T>) {
        Self::do_calculate_generic::<TokenBalanceOf<T>>(
            entity_id, buyer, order_amount, remaining,
            enabled_modes, is_first_order, buyer_order_count,
        )
    }
}
```

#### Step 4: Config 新增 TokenBalance 类型

每个插件的 Config 新增：

```rust
pub trait Config: frame_system::Config {
    // ... 现有类型 ...

    /// Token 余额类型（与 pallet-assets 对齐）
    type TokenBalance: Member + Parameter + AtLeast32BitUnsigned
        + Default + Copy + MaxEncodedLen + From<u32> + Into<u128>;
}

pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;
```

### 3.2 四个插件具体处理

| 插件 | rate-based 模式 | 固定金额模式 | Token 处理 |
|---|---|---|---|
| **referral** | DIRECT_REWARD ✅, MULTI_LEVEL ✅, REPEAT_PURCHASE ✅ | FIXED_AMOUNT ❌, FIRST_ORDER(use_amount) ❌ | rate 模式共用 NEX 配置（bps），固定金额跳过 |
| **level-diff** | LEVEL_DIFF ✅ | 无 | 完全共用 NEX 配置（bps） |
| **single-line** | UPLINE ✅, DOWNLINE ✅ | 无 | rate 共用配置 |
| **team** | TEAM_PERFORMANCE ✅ | 无 | rate 共用配置 |

**关键决策：固定金额模式（FIXED_AMOUNT / FIRST_ORDER.use_amount=true）对 Token 订单不生效。**
理由：固定金额以 NEX 绝对值计价（如 "每单奖励 5 NEX"），Token 汇率不同，直接套用无意义。

### 3.3 泛型辅助函数示例

以 `process_direct_reward_generic` 为例：

```rust
fn process_direct_reward_generic<B>(
    entity_id: u64,
    buyer: &T::AccountId,
    order_amount: B,
    remaining: &mut B,
    config: &DirectRewardConfig,
    outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
) where
    B: AtLeast32BitUnsigned + Copy + Saturating + Zero + From<u32>,
{
    if config.rate == 0 { return; }

    if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
        let reward = order_amount
            .saturating_mul(B::from(config.rate as u32))
            / B::from(10000u32);
        let actual = reward.min(*remaining);
        if !actual.is_zero() {
            outputs.push(CommissionOutput {
                beneficiary: referrer,
                amount: actual,
                commission_type: CommissionType::DirectReward,
                level: 0,
            });
            *remaining = remaining.saturating_sub(actual);
        }
    }
}
```

现有的 `process_direct_reward` 保持不变（向后兼容），仅新增 `_generic` 版本。
后续可将旧版函数体改为调用 generic 版本以消除重复。

---

## 4. commission-core 改动

### 4.1 Config 新增

```rust
pub trait Config: frame_system::Config {
    // ... 所有现有类型保持不变 ...

    /// Entity Token 余额类型
    type TokenBalance: Member + Parameter + AtLeast32BitUnsigned
        + Default + Copy + MaxEncodedLen + From<u32> + Into<u128>;

    /// Entity Token Asset ID 类型
    type AssetId: Member + Parameter + Copy + MaxEncodedLen + From<u64> + Into<u64>;

    /// Token 版推荐链插件
    type TokenReferralPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

    /// Token 版等级极差插件
    type TokenLevelDiffPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

    /// Token 版单线收益插件
    type TokenSingleLinePlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

    /// Token 版团队业绩插件
    type TokenTeamPlugin: TokenCommissionPlugin<Self::AccountId, TokenBalanceOf<Self>>;

    /// Token 转账接口 (fungibles::Transfer)
    type TokenTransfer: frame_support::traits::fungibles::Transfer<
        Self::AccountId,
        AssetId = Self::AssetId,
        Balance = TokenBalanceOf<Self>,
    >;

    /// Entity Token 查询（获取 asset_id）
    type EntityTokenProvider: pallet_entity_common::EntityTokenProvider<
        Self::AccountId, BalanceOf<Self>,
    >;

    /// Token 偏移量（entity_id → asset_id 映射）
    #[pallet::constant]
    type ShopTokenOffset: Get<u64>;
}

pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;
```

### 4.2 新增 Storage（Token 专用）

```rust
/// Token 佣金统计 (entity_id, account) → MemberTokenCommissionStatsData
#[pallet::storage]
pub type MemberTokenCommissionStats<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, u64, Blake2_128Concat, T::AccountId,
    MemberTokenCommissionStatsData<TokenBalanceOf<T>>,
    ValueQuery,
>;

/// Token 佣金记录 order_id → Vec<TokenCommissionRecord>
#[pallet::storage]
pub type OrderTokenCommissionRecords<T: Config> = StorageMap<
    _, Blake2_128Concat, u64,
    BoundedVec<TokenCommissionRecordOf<T>, T::MaxCommissionRecordsPerOrder>,
    ValueQuery,
>;

/// Token 待提取总额 entity_id → TokenBalance
#[pallet::storage]
pub type TokenPendingTotal<T: Config> = StorageMap<
    _, Blake2_128Concat, u64, TokenBalanceOf<T>, ValueQuery,
>;

/// Token 未分配沉淀池 entity_id → TokenBalance
#[pallet::storage]
pub type UnallocatedTokenPool<T: Config> = StorageMap<
    _, Blake2_128Concat, u64, TokenBalanceOf<T>, ValueQuery,
>;

/// Token 订单沉淀池记录 order_id → (entity_id, shop_id, TokenBalance)
#[pallet::storage]
pub type OrderTokenUnallocated<T: Config> = StorageMap<
    _, Blake2_128Concat, u64,
    (u64, u64, TokenBalanceOf<T>),
    ValueQuery,
>;
```

### 4.3 process_token_commission 核心逻辑

```rust
pub fn process_token_commission(
    entity_id: u64,
    shop_id: u64,
    order_id: u64,
    buyer: &T::AccountId,
    token_order_amount: TokenBalanceOf<T>,
) -> DispatchResult {
    let config = CommissionConfigs::<T>::get(entity_id)
        .filter(|c| c.enabled)
        .ok_or(Error::<T>::CommissionNotConfigured)?;

    // Token 订单不收平台费（平台费仅针对 NEX）
    let enabled_modes = config.enabled_modes;
    let entity_account = T::EntityProvider::entity_account(entity_id);
    let asset_id: T::AssetId = (entity_id.saturating_add(T::ShopTokenOffset::get())).into();
    let now = <frame_system::Pallet<T>>::block_number();
    let buyer_stats = MemberTokenCommissionStats::<T>::get(entity_id, buyer);
    let is_first_order = buyer_stats.order_count == 0;

    // 计算可分配上限
    let max_commission = token_order_amount
        .saturating_mul(config.max_commission_rate.into())
        / 10000u32.into();

    // 检查 entity_account 的 Token 余额
    let entity_token_balance = T::TokenTransfer::reducible_balance(
        asset_id, &entity_account, Preservation::Preserve, Fortitude::Polite,
    );
    let mut remaining = max_commission.min(entity_token_balance);

    if !remaining.is_zero() {
        let initial_remaining = remaining;

        // 1. Token Referral Plugin
        let (outputs, new_remaining) = T::TokenReferralPlugin::calculate_token(
            entity_id, buyer, token_order_amount, remaining,
            enabled_modes, is_first_order, buyer_stats.order_count,
        );
        remaining = new_remaining;
        for output in outputs {
            Self::credit_token_commission(
                entity_id, order_id, buyer, &output.beneficiary,
                output.amount, output.commission_type, output.level, now,
            )?;
        }

        // 2. Token LevelDiff Plugin
        let (outputs, new_remaining) = T::TokenLevelDiffPlugin::calculate_token(
            entity_id, buyer, token_order_amount, remaining,
            enabled_modes, is_first_order, buyer_stats.order_count,
        );
        remaining = new_remaining;
        for output in outputs {
            Self::credit_token_commission(
                entity_id, order_id, buyer, &output.beneficiary,
                output.amount, output.commission_type, output.level, now,
            )?;
        }

        // 3. Token SingleLine Plugin
        let (outputs, new_remaining) = T::TokenSingleLinePlugin::calculate_token(
            entity_id, buyer, token_order_amount, remaining,
            enabled_modes, is_first_order, buyer_stats.order_count,
        );
        remaining = new_remaining;
        for output in outputs {
            Self::credit_token_commission(
                entity_id, order_id, buyer, &output.beneficiary,
                output.amount, output.commission_type, output.level, now,
            )?;
        }

        // 4. Token Team Plugin
        let (outputs, new_remaining) = T::TokenTeamPlugin::calculate_token(
            entity_id, buyer, token_order_amount, remaining,
            enabled_modes, is_first_order, buyer_stats.order_count,
        );
        remaining = new_remaining;
        for output in outputs {
            Self::credit_token_commission(
                entity_id, order_id, buyer, &output.beneficiary,
                output.amount, output.commission_type, output.level, now,
            )?;
        }
    }

    // Phase 1.5: 剩余 Token → 沉淀池
    if enabled_modes.contains(CommissionModes::POOL_REWARD) && !remaining.is_zero() {
        UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
            *pool = pool.saturating_add(remaining);
        });
        OrderTokenUnallocated::<T>::insert(order_id, (entity_id, shop_id, remaining));
        Self::deposit_event(Event::TokenUnallocatedPooled {
            entity_id, order_id, amount: remaining,
        });
    }

    Ok(())
}
```

### 4.4 credit_token_commission

```rust
pub fn credit_token_commission(
    entity_id: u64,
    order_id: u64,
    buyer: &T::AccountId,
    beneficiary: &T::AccountId,
    amount: TokenBalanceOf<T>,
    commission_type: CommissionType,
    level: u8,
    now: BlockNumberFor<T>,
) -> DispatchResult {
    let record = TokenCommissionRecord {
        entity_id, order_id,
        buyer: buyer.clone(),
        beneficiary: beneficiary.clone(),
        amount, commission_type, level,
        status: CommissionStatus::Pending,
        created_at: now,
    };

    OrderTokenCommissionRecords::<T>::try_mutate(order_id, |records| {
        records.try_push(record).map_err(|_| Error::<T>::RecordsFull)
    })?;

    MemberTokenCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
        stats.total_earned = stats.total_earned.saturating_add(amount);
        stats.pending = stats.pending.saturating_add(amount);
    });

    TokenPendingTotal::<T>::mutate(entity_id, |total| {
        *total = total.saturating_add(amount);
    });

    Self::deposit_event(Event::TokenCommissionDistributed {
        entity_id, order_id,
        beneficiary: beneficiary.clone(),
        amount, commission_type, level,
    });

    Ok(())
}
```

### 4.5 withdraw_token_commission

```rust
/// Token 佣金提现（无复购分流，全额 Token 转账）
#[pallet::call_index(8)]
#[pallet::weight(Weight::from_parts(100_000_000, 8_000))]
pub fn withdraw_token_commission(
    origin: OriginFor<T>,
    entity_id: u64,
    amount: Option<TokenBalanceOf<T>>,
) -> DispatchResult {
    let who = ensure_signed(origin)?;

    MemberTokenCommissionStats::<T>::try_mutate(entity_id, &who, |stats| -> DispatchResult {
        let total_amount = amount.unwrap_or(stats.pending);
        ensure!(stats.pending >= total_amount, Error::<T>::InsufficientCommission);
        ensure!(!total_amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

        // 冻结期检查（共用 NEX 冻结期配置）
        if let Some(config) = CommissionConfigs::<T>::get(entity_id) {
            if config.withdrawal_cooldown > 0 {
                let now = <frame_system::Pallet<T>>::block_number();
                let last = MemberLastCredited::<T>::get(entity_id, &who);
                let cooldown: BlockNumberFor<T> = config.withdrawal_cooldown.into();
                ensure!(now >= last.saturating_add(cooldown),
                    Error::<T>::WithdrawalCooldownNotMet);
            }
        }

        // Token 转账: entity_account → who
        let entity_account = T::EntityProvider::entity_account(entity_id);
        let asset_id: T::AssetId = (entity_id + T::ShopTokenOffset::get()).into();
        T::TokenTransfer::transfer(
            asset_id, &entity_account, &who, total_amount,
            frame_support::traits::tokens::Preservation::Preserve,
        )?;

        stats.pending = stats.pending.saturating_sub(total_amount);
        stats.withdrawn = stats.withdrawn.saturating_add(total_amount);

        TokenPendingTotal::<T>::mutate(entity_id, |total| {
            *total = total.saturating_sub(total_amount);
        });

        Self::deposit_event(Event::TokenCommissionWithdrawn {
            entity_id, account: who.clone(), amount: total_amount,
        });

        Ok(())
    })
}
```

### 4.6 cancel_token_commission（扩展现有 cancel_commission）

在现有 `cancel_commission` 末尾追加 Token 退还逻辑：

```rust
// 第六步：退还 Token 佣金记录
let token_records = OrderTokenCommissionRecords::<T>::get(order_id);
OrderTokenCommissionRecords::<T>::mutate(order_id, |records| {
    for record in records.iter_mut() {
        if record.status == CommissionStatus::Pending {
            MemberTokenCommissionStats::<T>::mutate(
                record.entity_id, &record.beneficiary, |stats| {
                    stats.pending = stats.pending.saturating_sub(record.amount);
                    stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                }
            );
            TokenPendingTotal::<T>::mutate(record.entity_id, |total| {
                *total = total.saturating_sub(record.amount);
            });
            record.status = CommissionStatus::Cancelled;
        }
    }
});

// 第七步：退还 Token 沉淀池
let (te_id, ts_id, t_amount) = OrderTokenUnallocated::<T>::get(order_id);
if !t_amount.is_zero() {
    let entity_account = T::EntityProvider::entity_account(te_id);
    let asset_id: T::AssetId = (te_id + T::ShopTokenOffset::get()).into();
    if let Some(seller) = T::ShopProvider::shop_owner(ts_id) {
        let _ = T::TokenTransfer::transfer(
            asset_id, &entity_account, &seller, t_amount,
            Preservation::Preserve,
        );
    }
    UnallocatedTokenPool::<T>::mutate(te_id, |pool| {
        *pool = pool.saturating_sub(t_amount);
    });
    OrderTokenUnallocated::<T>::remove(order_id);
}
```

### 4.7 新增 Events

```rust
// Token 佣金分发
TokenCommissionDistributed {
    entity_id: u64, order_id: u64,
    beneficiary: T::AccountId, amount: TokenBalanceOf<T>,
    commission_type: CommissionType, level: u8,
},
// Token 佣金提现
TokenCommissionWithdrawn {
    entity_id: u64, account: T::AccountId, amount: TokenBalanceOf<T>,
},
// Token 沉淀池入账
TokenUnallocatedPooled {
    entity_id: u64, order_id: u64, amount: TokenBalanceOf<T>,
},
// Token 沉淀池退还
TokenUnallocatedPoolRefunded {
    entity_id: u64, order_id: u64, amount: TokenBalanceOf<T>,
},
```

### 4.8 PoolBalanceProvider for Token

```rust
impl<T: Config> TokenPoolBalanceProvider<TokenBalanceOf<T>> for Pallet<T> {
    fn token_pool_balance(entity_id: u64) -> TokenBalanceOf<T> {
        UnallocatedTokenPool::<T>::get(entity_id)
    }
    fn deduct_token_pool(entity_id: u64, amount: TokenBalanceOf<T>) -> Result<(), DispatchError> {
        UnallocatedTokenPool::<T>::try_mutate(entity_id, |pool| {
            ensure!(*pool >= amount, DispatchError::Other("InsufficientTokenPool"));
            *pool -= amount;
            Ok(())
        })
    }
}
```

---

## 5. pool-reward 多资产扩展

与原 README.v3-multiasset.md §4.4 设计一致：
- `RoundInfo` 新增 `token_pool_snapshot` + `token_level_snapshots`
- `ClaimRecord` 新增 `token_amount`
- `claim_pool_reward` 同时领取 NEX + Token
- Config 新增 `TokenPoolBalanceProvider`, `TokenTransfer`, `AssetId`, `TokenBalance`

不再赘述（参见 v3-multiasset.md §4.4-4.6）。

---

## 6. transaction 层改动

### 6.1 Order 结构新增 payment_asset

```rust
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum PaymentAsset {
    #[default]
    Native,      // NEX
    EntityToken, // Entity Token (pallet-assets)
}
```

Order struct 新增：
```rust
pub payment_asset: PaymentAsset,
pub token_payment_amount: TokenBalanceOf<T>, // Token 支付金额（仅 EntityToken 时有效）
```

### 6.2 place_order 分支

```rust
if pay_with_entity_token {
    // Token 支付流程
    let asset_id = entity_id + T::ShopTokenOffset::get();
    let token_amount = Self::nex_to_token(entity_id, final_amount)?;
    T::TokenTransfer::transfer(asset_id, &buyer, &escrow, token_amount, Preserve)?;
    // ... 创建 Order { payment_asset: EntityToken, token_payment_amount: token_amount }
} else {
    // 原有 NEX 支付流程（不变）
}
```

### 6.3 订单完成时分支

```rust
match order.payment_asset {
    PaymentAsset::Native => {
        T::CommissionProvider::process_commission(...)?;
    },
    PaymentAsset::EntityToken => {
        T::TokenCommissionProvider::process_token_commission(
            entity_id, shop_id, order_id, &buyer, order.token_payment_amount,
        )?;
    },
}
```

---

## 7. Runtime 配置

```rust
impl pallet_commission_core::Config for Runtime {
    // ... 现有配置不变 ...

    // 新增 Token 类型
    type TokenBalance = u128;
    type AssetId = u32;

    // Token 插件指向同一 pallet 实例（通过 TokenCommissionPlugin trait）
    type TokenReferralPlugin = CommissionReferral;
    type TokenLevelDiffPlugin = CommissionLevelDiff;
    type TokenSingleLinePlugin = CommissionSingleLine;
    type TokenTeamPlugin = CommissionTeam;

    // Token 转账
    type TokenTransfer = Assets; // pallet-assets
    type EntityTokenProvider = EntityToken;
    type ShopTokenOffset = ShopTokenOffset; // 已有常量
}

// 每个插件 Config 也需新增 TokenBalance
impl pallet_commission_referral::Config for Runtime {
    // ... 现有配置不变 ...
    type TokenBalance = u128;
}
// level-diff, single-line, team 同理
```

---

## 8. 存储迁移

### 零迁移策略

| 存储 | 类型 | 策略 |
|---|---|---|
| `MemberTokenCommissionStats` | 新增 | 无旧数据 |
| `OrderTokenCommissionRecords` | 新增 | 无旧数据 |
| `TokenPendingTotal` | 新增 | 无旧数据 |
| `UnallocatedTokenPool` | 新增 | 无旧数据 |
| `OrderTokenUnallocated` | 新增 | 无旧数据 |
| pool-reward `RoundInfo` | 扩展 | Option 字段，默认 None |
| pool-reward `ClaimRecord` | 扩展 | 默认 0 |

**所有新增存储无旧数据；所有扩展字段使用 Option/Default → 无需链上迁移。**

---

## 9. 文件改动清单

| 文件 | 改动类型 | 估计行数 |
|---|---|---|
| `commission-common/src/lib.rs` | 新增 traits + structs | +120 |
| `commission-core/src/lib.rs` | Config + Storage + 3 个新函数 + Events | +350 |
| `commission-core/src/mock.rs` | 新增 Token mock 类型 | +40 |
| `commission-core/src/tests.rs` | 新增 Token 佣金测试 | +200 |
| `commission-referral/src/lib.rs` | 泛型重构 + TokenCommissionPlugin impl | +150 |
| `commission-level-diff/src/lib.rs` | 泛型重构 + TokenCommissionPlugin impl | +80 |
| `commission-single-line/src/lib.rs` | 泛型重构 + TokenCommissionPlugin impl | +80 |
| `commission-team/src/lib.rs` | 泛型重构 + TokenCommissionPlugin impl | +60 |
| `pool-reward/src/lib.rs` | Token 池扩展 | +100 |
| `transaction/src/lib.rs` | 支付分支 + Order 扩展 | +80 |
| `entity-common/src/lib.rs` | PaymentAsset 枚举 | +15 |
| `runtime/src/configs/mod.rs` | 新增 Config 类型 | +30 |
| **总计** | | **+1305** |

---

## 10. 开发顺序（增量可编译）

### Phase 1: 基础 trait 和类型（1 天）

```
1.1 commission-common: 新增 TokenCommissionPlugin trait + 空实现
1.2 commission-common: 新增 TokenCommissionRecord / MemberTokenCommissionStatsData
1.3 commission-common: 新增 TokenPoolBalanceProvider trait + 空实现
1.4 commission-common: 新增 TokenCommissionProvider trait
→ cargo check -p pallet-commission-common ✅
```

### Phase 2: 插件泛型化（2 天）

```
2.1 referral: Config 新增 TokenBalance; 提取 do_calculate_generic
2.2 referral: 实现 TokenCommissionPlugin; 保留原 CommissionPlugin 不变
2.3 level-diff: 同上
2.4 single-line: 同上
2.5 team: 同上
→ cargo check 所有插件 ✅
```

### Phase 3: Core Token 管线（2 天）

```
3.1 core Config 新增 Token 关联类型
3.2 core 新增 5 个 Token Storage
3.3 core 实现 process_token_commission
3.4 core 实现 credit_token_commission
3.5 core 实现 withdraw_token_commission extrinsic
3.6 core 扩展 cancel_commission
3.7 core 实现 TokenPoolBalanceProvider
3.8 core mock 适配
→ cargo check -p pallet-commission-core ✅
```

### Phase 4: Pool Reward 双池（1 天）

```
4.1 pool-reward Config 新增 Token 类型
4.2 RoundInfo / ClaimRecord 扩展
4.3 create_new_round 双池快照
4.4 claim_pool_reward 双币种领取
→ cargo check -p pallet-commission-pool-reward ✅
```

### Phase 5: Transaction 层集成（1 天）

```
5.1 entity-common: PaymentAsset 枚举
5.2 transaction: Order 结构扩展
5.3 transaction: place_order 支持 Token 支付
5.4 transaction: 订单完成分支
→ cargo check -p pallet-entity-transaction ✅
```

### Phase 6: Runtime + 测试（1.5 天）

```
6.1 runtime Config 适配
6.2 core Token 佣金测试（≥10 用例）
6.3 插件 Token 计算测试（每个插件 ≥3 用例）
6.4 pool-reward 双池测试（≥5 用例）
6.5 集成测试
→ cargo test --workspace ✅
```

**总计：~8.5 天**

---

## 11. 与方案 F 对比

| 维度 | 方案 F（直推+入池） | 方案 A（全插件多资产） |
|---|---|---|
| 实现量 | ~100 行 | ~1300 行 |
| 工期 | 1 天 | 8.5 天 |
| 推荐人激励 | 仅直推 1 层 | 全管线对称 |
| 级差/单线/团队 | ❌ 不支持 | ✅ 完全支持 |
| Token 提现 | 无（池 claim 即提） | 独立 withdraw_token_commission |
| Token pending 记账 | 无 | 完整记账系统 |
| 订单取消退还 | 仅退池 | 退记录 + 退池 |
| 向后兼容 | ✅ | ✅（新增不改旧） |
| 可渐进交付 | 一次交付 | 6 Phase 增量 |

---

## 12. 风险和应对

| 风险 | 影响 | 应对 |
|---|---|---|
| 泛型 `B: From<u32>` 不满足所有 TokenBalance 类型 | 编译失败 | 使用 `Into<u128> + TryFrom<u128>` 替代 |
| Plugin 内部有 NEX 绝对值操作（如 min_balance） | Token 计算错误 | 固定金额模式跳过 Token |
| Token entity_account 余额不足覆盖 pending | 提现失败 | 偿付安全检查（与 NEX 一致） |
| Rust coherence: 同一 pallet 实现两个 trait | 编译失败 | NEX 和 Token 是不同 trait，无冲突 |
| 存储膨胀（双倍记录） | 链上存储增长 | Token 记录独立存储，可按需清理 |
