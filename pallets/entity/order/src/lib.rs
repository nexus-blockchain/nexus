//! # 商城订单管理模块 (pallet-entity-order)
//!
//! ## 概述
//!
//! 本模块负责订单的完整生命周期管理，包括：
//! - 下单并支付（资金托管）
//! - 取消订单
//! - 发货
//! - 确认收货
//! - 退款流程
//! - 超时自动处理
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-01-31): 从 pallet-mall 拆分

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use pallet_entity_common::OrderStatus;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use frame_system::ensure_root;
    use pallet_escrow::pallet::Escrow as EscrowTrait;
    use pallet_entity_common::{OrderStatus, OrderCommissionHandler, OrderMemberHandler, OrderProvider, PaymentAsset, PricingProvider, ProductCategory, ProductProvider, ProductStatus, ProductVisibility, EntityTokenProvider, EntityTokenPriceProvider, MemberProvider, ShopProvider, ShoppingBalanceProvider, TokenOrderCommissionHandler};
    use sp_runtime::{traits::{Saturating, Zero}, SaturatedConversion};

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// 订单信息
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct Order<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
        pub id: u64,
        /// 订单所属 Entity ID（创建时快照，避免后续通过 shop 间接查询）
        pub entity_id: u64,
        pub shop_id: u64,
        pub product_id: u64,
        pub buyer: AccountId,
        pub seller: AccountId,
        pub quantity: u32,
        pub unit_price: Balance,
        /// 实际支付金额（积分/购物余额/会员折扣后）
        pub total_amount: Balance,
        pub platform_fee: Balance,
        /// 商品类别（决定订单流程，是否需要物流由此推导）
        pub product_category: ProductCategory,
        pub shipping_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub tracking_cid: Option<BoundedVec<u8, MaxCidLen>>,
        pub status: OrderStatus,
        pub created_at: BlockNumber,
        pub shipped_at: Option<BlockNumber>,
        pub completed_at: Option<BlockNumber>,
        pub service_started_at: Option<BlockNumber>,
        /// 服务完成时间（卖家标记，限设置一次）
        pub service_completed_at: Option<BlockNumber>,
        pub payment_asset: PaymentAsset,
        /// Token 支付金额（仅 EntityToken 时有效，u128 避免泛型膨胀）
        pub token_payment_amount: u128,
        /// 买家是否已延长确认收货期限（限延一次）
        pub confirm_extended: bool,
        /// 卖家是否已拒绝退款（限一次，防无限延期）
        pub dispute_rejected: bool,
        pub dispute_deadline: Option<BlockNumber>,
        pub note_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 退款/争议理由 CID（request_refund 时存储）
        pub refund_reason_cid: Option<BoundedVec<u8, MaxCidLen>>,
    }

    /// 订单类型别名
    pub type OrderOf<T> = Order<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    /// 订单统计
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct OrderStatistics<Balance: Default> {
        /// 总订单数
        pub total_orders: u64,
        /// 已完成订单数
        pub completed_orders: u64,
        /// 总交易额（NEX）
        pub total_volume: Balance,
        /// 总平台费收入（NEX）
        pub total_platform_fees: Balance,
        /// 总 Token 交易额（u128 避免泛型膨胀）
        pub total_token_volume: u128,
        /// 总 Token 平台费（u128）
        pub total_token_platform_fees: u128,
    }

    /// 订单附属操作类型（用于失败事件追踪）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum OrderOperation {
        /// Escrow 退款
        EscrowRefund,
        /// 库存恢复
        StockRestore,
        /// 佣金取消
        CommissionCancel,
        /// 佣金结算
        CommissionComplete,
        /// 店铺统计更新
        ShopStatsUpdate,
        /// 积分奖励
        TokenReward,
        /// 会员注册/消费更新
        MemberUpdate,
        /// 订单自动完成
        AutoComplete,
        /// 升级规则检查
        UpgradeRuleCheck,
        /// Token 平台费分配失败
        TokenPlatformFee,
        /// 会员自动注册失败
        MemberAutoRegister,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 货币类型
        type Currency: Currency<Self::AccountId>;

        /// 托管接口
        type Escrow: EscrowTrait<Self::AccountId, BalanceOf<Self>>;

        /// Shop 查询接口（Entity-Shop 分离架构）
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// 商品查询接口
        type ProductProvider: ProductProvider<Self::AccountId, BalanceOf<Self>>;

        /// 实体代币接口
        type EntityToken: EntityTokenProvider<Self::AccountId, BalanceOf<Self>>;

        /// 平台账户
        #[pallet::constant]
        type PlatformAccount: Get<Self::AccountId>;


        /// 发货超时（区块数）
        #[pallet::constant]
        type ShipTimeout: Get<BlockNumberFor<Self>>;

        /// 确认收货超时（区块数）
        #[pallet::constant]
        type ConfirmTimeout: Get<BlockNumberFor<Self>>;

        /// 服务确认超时（区块数）
        #[pallet::constant]
        type ServiceConfirmTimeout: Get<BlockNumberFor<Self>>;

        /// 争议超时（区块数）— 卖家在此期限内必须响应（approve/reject），否则自动退款
        #[pallet::constant]
        type DisputeTimeout: Get<BlockNumberFor<Self>>;

        /// 确认收货延长时间（区块数）— 买家可延长一次
        #[pallet::constant]
        type ConfirmExtension: Get<BlockNumberFor<Self>>;

        /// 佣金处理接口（订单完成时触发返佣）
        type CommissionHandler: OrderCommissionHandler<Self::AccountId, BalanceOf<Self>>;

        /// 购物余额接口（下单时抵扣复购余额）
        type ShoppingBalance: ShoppingBalanceProvider<Self::AccountId, BalanceOf<Self>>;

        /// Token 佣金处理接口（Entity Token 订单完成时触发 Token 返佣）
        type TokenCommissionHandler: TokenOrderCommissionHandler<Self::AccountId>;

        /// 会员处理接口（订单完成时自动注册 + 更新消费金额）
        type MemberHandler: OrderMemberHandler<Self::AccountId>;

        /// NEX/USDT 定价接口（用于将 NEX 金额转换为 USDT 以更新会员消费统计）
        type PricingProvider: PricingProvider;

        /// Token 价格查询接口（Entity Token → NEX 价格，用于间接换算 USDT）
        type TokenPriceProvider: EntityTokenPriceProvider<Balance = BalanceOf<Self>>;

        /// 会员查询接口（用于商品可见性校验：MembersOnly / LevelGated）
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// NEX 平台费率（基点，100 = 1%）
    /// 可通过 set_platform_fee_rate 治理调整，0 = 关闭平台费
    #[pallet::storage]
    pub type PlatformFeeRate<T> = StorageValue<_, u16, ValueQuery, DefaultPlatformFeeRate>;

    /// NEX 平台费率默认值（100 bps = 1%）
    #[pallet::type_value]
    pub fn DefaultPlatformFeeRate() -> u16 { 100 }

    /// 下一个订单 ID
    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub type NextOrderId<T> = StorageValue<_, u64, ValueQuery>;

    /// 订单存储
    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, u64, OrderOf<T>>;

    /// 买家订单索引
    #[pallet::storage]
    #[pallet::getter(fn buyer_orders)]
    pub type BuyerOrders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<1000>>,
        ValueQuery,
    >;

    /// 店铺订单索引
    #[pallet::storage]
    #[pallet::getter(fn shop_orders)]
    pub type ShopOrders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        BoundedVec<u64, ConstU32<10000>>,
        ValueQuery,
    >;

    /// 订单统计
    #[pallet::storage]
    #[pallet::getter(fn order_stats)]
    pub type OrderStats<T: Config> = StorageValue<_, OrderStatistics<BalanceOf<T>>, ValueQuery>;

    /// 过期检查队列：到期区块号 → 待检查订单 ID 列表
    #[pallet::storage]
    pub type ExpiryQueue<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<u64, ConstU32<500>>,
        ValueQuery,
    >;

    /// 订单推荐人（place_order 时记录，完成时传递给 MemberHandler::auto_register）
    #[pallet::storage]
    pub type OrderReferrer<T: Config> = StorageMap<_, Blake2_128Concat, u64, T::AccountId>;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 订单已创建并支付
        OrderCreated {
            order_id: u64,
            entity_id: u64,
            buyer: T::AccountId,
            seller: T::AccountId,
            amount: BalanceOf<T>,
            payment_asset: PaymentAsset,
            token_amount: u128,
        },
        OrderShipped { order_id: u64 },
        OrderCompleted {
            order_id: u64,
            seller_received: BalanceOf<T>,
            token_seller_received: u128,
        },
        OrderCancelled { order_id: u64, amount: BalanceOf<T>, token_amount: u128 },
        OrderRefunded {
            order_id: u64,
            amount: BalanceOf<T>,
            token_amount: u128,
        },
        OrderDisputed { order_id: u64 },
        OrderOperationFailed { order_id: u64, operation: OrderOperation },
        ServiceStarted { order_id: u64 },
        ServiceCompleted { order_id: u64 },
        PlatformFeeRateUpdated { old_rate: u16, new_rate: u16 },
        BuyerOrdersCleaned { buyer: T::AccountId, removed: u32 },
        RefundRejected { order_id: u64, reason_cid: Vec<u8> },
        OrderSellerCancelled { order_id: u64, amount: BalanceOf<T>, token_amount: u128, reason_cid: Vec<u8> },
        OrderForceRefunded { order_id: u64, reason_cid: Option<Vec<u8>> },
        OrderForceCompleted { order_id: u64, reason_cid: Option<Vec<u8>> },
        ShippingAddressUpdated { order_id: u64 },
        ConfirmTimeoutExtended { order_id: u64, new_deadline: BlockNumberFor<T> },
        ShopOrdersCleaned { shop_id: u64, removed: u32 },
        TrackingInfoUpdated { order_id: u64 },
        /// 卖家主动退款（Shipped 状态）
        OrderSellerRefunded { order_id: u64, amount: BalanceOf<T>, token_amount: u128, reason_cid: Vec<u8> },
        /// 管理员部分退款
        OrderPartialRefunded { order_id: u64, refund_bps: u16, reason_cid: Option<Vec<u8>> },
        /// 买家撤回争议
        DisputeWithdrawn { order_id: u64 },
        /// 管理员手动处理指定区块的过期订单
        StaleExpirationsProcessed { target_block: BlockNumberFor<T>, processed: u32 },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        OrderNotFound,
        ProductNotFound,
        ShopNotFound,
        NotOrderBuyer,
        NotOrderSeller,
        InvalidOrderStatus,
        CannotCancelOrder,
        CannotBuyOwnProduct,
        ProductNotOnSale,
        InsufficientStock,
        CidTooLong,
        Overflow,
        DigitalProductCannotCancel,
        InvalidQuantity,
        DigitalProductCannotRefund,
        /// 非服务类/订阅类订单
        NotServiceLikeOrder,
        InvalidAmount,
        ExpiryQueueFull,
        ShippingCidRequired,
        /// 服务/订阅类订单不可使用发货/收货流程
        ServiceLikeOrderCannotShip,
        EmptyReasonCid,
        EntityTokenNotEnabled,
        InsufficientTokenBalance,
        EmptyTrackingCid,
        PlatformFeeRateTooHigh,
        NothingToClean,
        NotShopOwner,
        AlreadyExtended,
        CannotForceOrder,
        QuantityBelowMinimum,
        QuantityAboveMaximum,
        ProductMembersOnly,
        MemberLevelInsufficient,
        DisputeAlreadyRejected,
        /// 买家已被该 Entity 封禁
        BuyerBanned,
        /// 部分退款比例无效（需 1-9999 bps）
        InvalidRefundBps,
        /// Token 订单不支持部分退款
        PartialRefundNotSupported,
        /// 推荐人不能是买家或卖家自己
        InvalidReferrer,
        /// Subscription 类商品暂不支持下单（与 Service 流程等价，请使用 Service 类别）
        SubscriptionNotSupported,
        /// 店铺未激活（存在但处于暂停/关闭状态）
        ShopInactive,
    }

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 空闲时处理超时订单（基于 ExpiryQueue 精确索引）
        fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let per_order_weight = Weight::from_parts(200_000_000, 8_000);
            if remaining_weight.ref_time() < per_order_weight.ref_time().saturating_add(50_000_000) {
                return Weight::zero();
            }

            let max_count = (remaining_weight.ref_time() / per_order_weight.ref_time()).min(20) as u32;
            Self::process_expired_orders(now, now, max_count)
        }
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 下单并支付
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(350_000_000, 16_000))]
        pub fn place_order(
            origin: OriginFor<T>,
            product_id: u64,
            quantity: u32,
            shipping_cid: Option<Vec<u8>>,
            use_tokens: Option<BalanceOf<T>>,
            use_shopping_balance: Option<BalanceOf<T>>,
            payment_asset: Option<PaymentAsset>,
            note_cid: Option<Vec<u8>>,
            referrer: Option<T::AccountId>,
        ) -> DispatchResult {
            let buyer = ensure_signed(origin)?;

            ensure!(quantity > 0, Error::<T>::InvalidQuantity);

            // M2-fix: 一次 storage read 获取商品全部下单所需字段（替代 9 次独立查询）
            let product_info = T::ProductProvider::get_product_info(product_id)
                .ok_or(Error::<T>::ProductNotFound)?;
            ensure!(
                product_info.status == ProductStatus::OnSale,
                Error::<T>::ProductNotOnSale
            );
            let shop_id = product_info.shop_id;
            let price = product_info.price;

            if product_info.min_order_quantity > 0 {
                ensure!(quantity >= product_info.min_order_quantity, Error::<T>::QuantityBelowMinimum);
            }
            if product_info.max_order_quantity > 0 {
                ensure!(quantity <= product_info.max_order_quantity, Error::<T>::QuantityAboveMaximum);
            }

            ensure!(T::ShopProvider::shop_exists(shop_id), Error::<T>::ShopNotFound);
            ensure!(T::ShopProvider::is_shop_active(shop_id), Error::<T>::ShopInactive);
            let seller = T::ShopProvider::shop_owner(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(seller != buyer, Error::<T>::CannotBuyOwnProduct);

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // M2-R11: referrer 不能是买家自己或卖家
            if let Some(ref r) = referrer {
                ensure!(*r != buyer, Error::<T>::InvalidReferrer);
                ensure!(*r != seller, Error::<T>::InvalidReferrer);
            }

            // P1: 封禁检查
            ensure!(
                !T::MemberProvider::is_banned(entity_id, &buyer),
                Error::<T>::BuyerBanned
            );

            // L3-R11: 统一获取 buyer_level，可见性校验和折扣计算复用
            let buyer_level = T::MemberProvider::get_effective_level(entity_id, &buyer);

            match product_info.visibility {
                ProductVisibility::Public => {},
                ProductVisibility::MembersOnly => {
                    ensure!(
                        T::MemberProvider::is_member(entity_id, &buyer),
                        Error::<T>::ProductMembersOnly
                    );
                },
                ProductVisibility::LevelGated(required_level) => {
                    ensure!(
                        T::MemberProvider::is_member(entity_id, &buyer),
                        Error::<T>::ProductMembersOnly
                    );
                    ensure!(
                        buyer_level >= required_level,
                        Error::<T>::MemberLevelInsufficient
                    );
                },
            }

            // stock == 0 表示无限库存
            if product_info.stock > 0 {
                ensure!(product_info.stock >= quantity, Error::<T>::InsufficientStock);
            }

            let total_amount = price.checked_mul(&quantity.into())
                .ok_or(Error::<T>::Overflow)?;

            let resolved_payment_asset = payment_asset.unwrap_or(PaymentAsset::Native);

            let mut final_amount = total_amount;
            if buyer_level > 0 {
                let discount_bps: u32 = T::MemberProvider::get_level_discount(entity_id, buyer_level).into();
                if discount_bps > 0 && discount_bps < 10000 {
                    let discount = final_amount.saturating_mul(discount_bps.into()) / 10000u32.into();
                    final_amount = final_amount.saturating_sub(discount);
                }
            }

            // 积分/购物余额抵扣（仅 Native）
            if resolved_payment_asset == PaymentAsset::Native {
                if let Some(tokens) = use_tokens {
                    if !tokens.is_zero() && T::EntityToken::is_token_enabled(entity_id) {
                        let discount = T::EntityToken::redeem_for_discount(entity_id, &buyer, tokens)?;
                        final_amount = final_amount.saturating_sub(discount);
                    }
                }
                if let Some(shopping_amount) = use_shopping_balance {
                    if !shopping_amount.is_zero() {
                        ensure!(shopping_amount <= final_amount, Error::<T>::InvalidAmount);
                        T::ShoppingBalance::consume_shopping_balance(entity_id, &buyer, shopping_amount)?;
                        final_amount = final_amount.saturating_sub(shopping_amount);
                    }
                }
            }

            ensure!(!final_amount.is_zero(), Error::<T>::InvalidAmount);

            let platform_fee = match resolved_payment_asset {
                PaymentAsset::Native => {
                    final_amount
                        .saturating_mul(PlatformFeeRate::<T>::get().into())
                        / 10000u32.into()
                },
                PaymentAsset::EntityToken => Zero::zero(),
            };

            let shipping_cid: Option<BoundedVec<u8, T::MaxCidLength>> = shipping_cid
                .map(|c| c.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            let bounded_note_cid: Option<BoundedVec<u8, T::MaxCidLength>> = note_cid
                .map(|c| c.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            let product_category = product_info.category;

            ensure!(
                product_category != ProductCategory::Subscription,
                Error::<T>::SubscriptionNotSupported
            );

            let requires_shipping = Self::category_requires_shipping(&product_category);

            if requires_shipping {
                ensure!(shipping_cid.is_some(), Error::<T>::ShippingCidRequired);
            }

            let order_id = NextOrderId::<T>::get();
            let now = <frame_system::Pallet<T>>::block_number();

            let token_payment_amount: u128 = match resolved_payment_asset {
                PaymentAsset::Native => {
                    T::Escrow::lock_from(&buyer, order_id, final_amount)?;
                    0u128
                },
                PaymentAsset::EntityToken => {
                    ensure!(T::EntityToken::is_token_enabled(entity_id), Error::<T>::EntityTokenNotEnabled);
                    let buyer_token_balance = T::EntityToken::token_balance(entity_id, &buyer);
                    ensure!(buyer_token_balance >= final_amount, Error::<T>::InsufficientTokenBalance);
                    T::EntityToken::reserve(entity_id, &buyer, final_amount)?;
                    final_amount.saturated_into::<u128>()
                },
            };

            T::ProductProvider::deduct_stock(product_id, quantity)?;
            T::ProductProvider::add_sold_count(product_id, quantity)?;

            let initial_status = if product_category == ProductCategory::Digital {
                OrderStatus::Completed
            } else {
                OrderStatus::Paid
            };

            let order = Order {
                id: order_id,
                entity_id,
                shop_id,
                product_id,
                buyer: buyer.clone(),
                seller: seller.clone(),
                quantity,
                unit_price: price,
                total_amount: final_amount,
                platform_fee,
                product_category,
                shipping_cid,
                tracking_cid: None,
                status: initial_status,
                created_at: now,
                shipped_at: None,
                completed_at: if product_category == ProductCategory::Digital { Some(now) } else { None },
                service_started_at: None,
                service_completed_at: None,
                payment_asset: resolved_payment_asset,
                token_payment_amount,
                confirm_extended: false,
                dispute_rejected: false,
                dispute_deadline: None,
                note_cid: bounded_note_cid,
                refund_reason_cid: None,
            };

            Orders::<T>::insert(order_id, &order);
            BuyerOrders::<T>::try_mutate(&buyer, |ids| ids.try_push(order_id))
                .map_err(|_| Error::<T>::Overflow)?;
            ShopOrders::<T>::try_mutate(shop_id, |ids| ids.try_push(order_id))
                .map_err(|_| Error::<T>::Overflow)?;
            let next_id = order_id.checked_add(1).ok_or(Error::<T>::Overflow)?;
            NextOrderId::<T>::put(next_id);

            if product_category != ProductCategory::Digital {
                let expiry_block = now.saturating_add(T::ShipTimeout::get());
                ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                    ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
                })?;
            }

            // P2: 存储推荐人（完成时用于 auto_register）
            if let Some(ref r) = referrer {
                OrderReferrer::<T>::insert(order_id, r);
            }

            OrderStats::<T>::mutate(|stats| {
                stats.total_orders = stats.total_orders.saturating_add(1);
            });

            Self::deposit_event(Event::OrderCreated {
                order_id,
                entity_id,
                buyer: buyer.clone(),
                seller: seller.clone(),
                amount: final_amount,
                payment_asset: resolved_payment_asset,
                token_amount: token_payment_amount,
            });

            if product_category == ProductCategory::Digital {
                Self::do_complete_order(order_id, &order)?;
            }

            Ok(())
        }

        /// 取消订单（数字商品不可取消）
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn cancel_order(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
            ensure!(
                order.product_category != ProductCategory::Digital,
                Error::<T>::DigitalProductCannotCancel
            );
            ensure!(order.status == OrderStatus::Paid, Error::<T>::CannotCancelOrder);

            Self::do_cancel_or_refund(&order, order_id, OrderStatus::Cancelled)?;

            Self::deposit_event(Event::OrderCancelled {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
            });
            Ok(())
        }

        /// 发货（服务/订阅类不可用，须走 start_service）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(200_000_000, 8_000))]
        pub fn ship_order(
            origin: OriginFor<T>,
            order_id: u64,
            tracking_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let created_at = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<BlockNumberFor<T>, DispatchError> {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                ensure!(
                    !Self::is_service_like(&order.product_category),
                    Error::<T>::ServiceLikeOrderCannotShip
                );
                ensure!(order.status == OrderStatus::Paid, Error::<T>::InvalidOrderStatus);

                ensure!(!tracking_cid.is_empty(), Error::<T>::EmptyTrackingCid);
                order.tracking_cid = Some(
                    tracking_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?
                );
                let ca = order.created_at;
                order.status = OrderStatus::Shipped;
                order.shipped_at = Some(<frame_system::Pallet<T>>::block_number());
                Ok(ca)
            })?;

            // H4: 清理 place_order 创建的旧 ShipTimeout 条目
            let old_expiry = created_at.saturating_add(T::ShipTimeout::get());
            ExpiryQueue::<T>::mutate(old_expiry, |ids| {
                ids.retain(|&id| id != order_id);
            });

            let now = <frame_system::Pallet<T>>::block_number();
            let expiry_block = now.saturating_add(T::ConfirmTimeout::get());
            ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Self::deposit_event(Event::OrderShipped { order_id });
            Ok(())
        }

        /// 确认收货
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(300_000_000, 12_000))]
        pub fn confirm_receipt(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
            ensure!(
                !Self::is_service_like(&order.product_category),
                Error::<T>::ServiceLikeOrderCannotShip
            );
            ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);

            Self::do_complete_order(order_id, &order)
        }

        /// 申请退款（数字商品不可退款）
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn request_refund(
            origin: OriginFor<T>,
            order_id: u64,
            reason_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let bounded_reason = Self::validate_reason_cid(reason_cid)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let expiry_block = now.saturating_add(T::DisputeTimeout::get());

            let payment_asset = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<PaymentAsset, DispatchError> {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
                ensure!(
                    order.product_category != ProductCategory::Digital,
                    Error::<T>::DigitalProductCannotRefund
                );
                ensure!(
                    order.status == OrderStatus::Paid || order.status == OrderStatus::Shipped,
                    Error::<T>::InvalidOrderStatus
                );

                let asset = order.payment_asset.clone();
                order.status = OrderStatus::Disputed;
                order.refund_reason_cid = Some(bounded_reason);
                order.dispute_deadline = Some(expiry_block);
                Ok(asset)
            })?;

            ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            if payment_asset == PaymentAsset::Native {
                T::Escrow::set_disputed(order_id)?;
            }

            Self::deposit_event(Event::OrderDisputed { order_id });
            Ok(())
        }

        /// 同意退款（卖家）
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn approve_refund(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.seller == who, Error::<T>::NotOrderSeller);
            ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);

            if order.payment_asset == PaymentAsset::Native {
                T::Escrow::set_resolved(order_id)?;
            }

            Self::do_cancel_or_refund(&order, order_id, OrderStatus::Refunded)?;

            Self::deposit_event(Event::OrderRefunded {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
            });
            Ok(())
        }

        /// 开始服务（卖家，服务类订单）
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn start_service(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let created_at = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<BlockNumberFor<T>, DispatchError> {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                ensure!(Self::is_service_like(&order.product_category), Error::<T>::NotServiceLikeOrder);
                ensure!(order.status == OrderStatus::Paid, Error::<T>::InvalidOrderStatus);

                let ca = order.created_at;
                order.status = OrderStatus::Shipped;
                order.service_started_at = Some(<frame_system::Pallet<T>>::block_number());
                Ok(ca)
            })?;

            // H4: 清理 place_order 创建的旧 ShipTimeout 条目
            let old_expiry = created_at.saturating_add(T::ShipTimeout::get());
            ExpiryQueue::<T>::mutate(old_expiry, |ids| {
                ids.retain(|&id| id != order_id);
            });

            let now = <frame_system::Pallet<T>>::block_number();
            let expiry_block = now.saturating_add(T::ServiceConfirmTimeout::get());
            ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Self::deposit_event(Event::ServiceStarted { order_id });
            Ok(())
        }

        /// 标记服务完成（卖家，服务类订单）
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(175_000_000, 8_000))]
        pub fn complete_service(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let service_started_at = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<Option<BlockNumberFor<T>>, DispatchError> {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                ensure!(Self::is_service_like(&order.product_category), Error::<T>::NotServiceLikeOrder);
                ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);
                ensure!(order.service_completed_at.is_none(), Error::<T>::InvalidOrderStatus);

                let sa = order.service_started_at;
                order.service_completed_at = Some(<frame_system::Pallet<T>>::block_number());
                Ok(sa)
            })?;

            // H4: 清理 start_service 创建的旧 ServiceConfirmTimeout 条目
            if let Some(sa) = service_started_at {
                let old_expiry = sa.saturating_add(T::ServiceConfirmTimeout::get());
                ExpiryQueue::<T>::mutate(old_expiry, |ids| {
                    ids.retain(|&id| id != order_id);
                });
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let expiry_block = now.saturating_add(T::ServiceConfirmTimeout::get());
            ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Self::deposit_event(Event::ServiceCompleted { order_id });
            Ok(())
        }

        /// 确认服务完成（买家，服务类订单）
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(300_000_000, 12_000))]
        pub fn confirm_service(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
            ensure!(Self::is_service_like(&order.product_category), Error::<T>::NotServiceLikeOrder);
            ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);
            ensure!(order.service_completed_at.is_some(), Error::<T>::InvalidOrderStatus);

            Self::do_complete_order(order_id, &order)
        }

        /// 设置 NEX 平台费率（Root / 治理）
        ///
        /// rate 为基点，0 = 关闭平台费，上限 1000 bps（10%）
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(20_000_000, 2_000))]
        pub fn set_platform_fee_rate(
            origin: OriginFor<T>,
            new_rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(new_rate <= 1000, Error::<T>::PlatformFeeRateTooHigh);
            let old_rate = PlatformFeeRate::<T>::get();
            PlatformFeeRate::<T>::put(new_rate);
            Self::deposit_event(Event::PlatformFeeRateUpdated { old_rate, new_rate });
            Ok(())
        }

        /// 清理买家订单索引（移除已终态的订单 ID，释放 BoundedVec 容量）
        ///
        /// 终态 = Completed / Cancelled / Refunded
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(100_000_000, 8_000))]
        pub fn cleanup_buyer_orders(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let orders = BuyerOrders::<T>::get(&who);
            let before = orders.len() as u32;

            // 保留非终态订单
            let retained: Vec<u64> = orders.iter().copied().filter(|&oid| {
                Orders::<T>::get(oid)
                    .map(|o| !matches!(o.status, OrderStatus::Completed | OrderStatus::Cancelled | OrderStatus::Refunded))
                    .unwrap_or(false) // 订单不存在也移除
            }).collect();

            let after = retained.len() as u32;
            let removed = before.saturating_sub(after);
            ensure!(removed > 0, Error::<T>::NothingToClean);

            let bounded: BoundedVec<u64, ConstU32<1000>> = retained
                .try_into()
                .expect("retained is subset of original bounded vec");
            BuyerOrders::<T>::insert(&who, bounded);

            Self::deposit_event(Event::BuyerOrdersCleaned { buyer: who, removed });
            Ok(())
        }

        /// 拒绝退款（卖家）— 订单保持 Disputed，写入争议超时队列
        ///
        /// 卖家拒绝后，争议进入 DisputeTimeout 倒计时。
        /// 超时未仲裁则自动退款给买家。
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn reject_refund(
            origin: OriginFor<T>,
            order_id: u64,
            reason_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let bounded_reason = Self::validate_reason_cid(reason_cid)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.seller == who, Error::<T>::NotOrderSeller);
            ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);
            ensure!(!order.dispute_rejected, Error::<T>::DisputeAlreadyRejected);

            let now = <frame_system::Pallet<T>>::block_number();
            let expiry_block = now.saturating_add(T::DisputeTimeout::get());

            // H4: 清理 request_refund 创建的旧超时条目
            if let Some(old_deadline) = order.dispute_deadline {
                ExpiryQueue::<T>::mutate(old_deadline, |ids| {
                    ids.retain(|&id| id != order_id);
                });
            }

            // H3-fix: ExpiryQueue 写入先于 Orders 更新，确保队列满时不会产生不一致状态
            ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.dispute_rejected = true;
                    o.dispute_deadline = Some(expiry_block);
                }
            });

            Self::deposit_event(Event::RefundRejected { order_id, reason_cid: bounded_reason.into_inner() });
            Ok(())
        }

        /// 卖家主动取消订单（仅 Paid 状态，非数字商品）
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn seller_cancel_order(
            origin: OriginFor<T>,
            order_id: u64,
            reason_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let bounded_reason = Self::validate_reason_cid(reason_cid)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.seller == who, Error::<T>::NotOrderSeller);
            ensure!(
                order.product_category != ProductCategory::Digital,
                Error::<T>::DigitalProductCannotCancel
            );
            ensure!(order.status == OrderStatus::Paid, Error::<T>::CannotCancelOrder);

            Self::do_cancel_or_refund(&order, order_id, OrderStatus::Cancelled)?;

            Self::deposit_event(Event::OrderSellerCancelled {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
                reason_cid: bounded_reason.into_inner(),
            });
            Ok(())
        }

        /// 管理员强制退款（Root / 治理）
        ///
        /// 可对 Paid / Shipped / Disputed 状态的订单强制退款
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(300_000_000, 12_000))]
        pub fn force_refund(origin: OriginFor<T>, order_id: u64, reason_cid: Option<Vec<u8>>) -> DispatchResult {
            ensure_root(origin)?;

            let reason = Self::validate_optional_reason_cid(&reason_cid)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(
                matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
                Error::<T>::CannotForceOrder
            );

            // M4-fix: 传播 set_resolved 错误，避免后续 transfer_from_escrow 因 disputed 状态失败
            if order.status == OrderStatus::Disputed && order.payment_asset == PaymentAsset::Native {
                T::Escrow::set_resolved(order_id)?;
            }

            Self::do_cancel_or_refund(&order, order_id, OrderStatus::Refunded)?;

            Self::deposit_event(Event::OrderForceRefunded { order_id, reason_cid: reason });
            Self::deposit_event(Event::OrderRefunded {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
            });
            Ok(())
        }

        /// 管理员强制完成订单（Root / 治理）
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(350_000_000, 16_000))]
        pub fn force_complete(origin: OriginFor<T>, order_id: u64, reason_cid: Option<Vec<u8>>) -> DispatchResult {
            ensure_root(origin)?;

            let reason = Self::validate_optional_reason_cid(&reason_cid)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(
                matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
                Error::<T>::CannotForceOrder
            );

            // M4-fix: 传播 set_resolved 错误
            if order.status == OrderStatus::Disputed && order.payment_asset == PaymentAsset::Native {
                T::Escrow::set_resolved(order_id)?;
            }

            Self::do_complete_order(order_id, &order)?;
            Self::deposit_event(Event::OrderForceCompleted { order_id, reason_cid: reason });
            Ok(())
        }

        /// 买家修改收货地址（仅 Paid 状态，发货前）
        #[pallet::call_index(15)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn update_shipping_address(
            origin: OriginFor<T>,
            order_id: u64,
            new_shipping_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!new_shipping_cid.is_empty(), Error::<T>::ShippingCidRequired);
            let bounded_cid: BoundedVec<u8, T::MaxCidLength> =
                new_shipping_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
                ensure!(order.status == OrderStatus::Paid, Error::<T>::InvalidOrderStatus);
                ensure!(Self::category_requires_shipping(&order.product_category), Error::<T>::ServiceLikeOrderCannotShip);

                order.shipping_cid = Some(bounded_cid);
                Ok(())
            })?;

            Self::deposit_event(Event::ShippingAddressUpdated { order_id });
            Ok(())
        }

        /// 买家延长确认收货期限（仅 Shipped 状态，限延一次）
        ///
        /// 在 ExpiryQueue 中追加一条新的超时条目
        #[pallet::call_index(16)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn extend_confirm_timeout(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let new_deadline = now.saturating_add(T::ConfirmExtension::get());

            let shipped_at = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<Option<BlockNumberFor<T>>, DispatchError> {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
                ensure!(
                    !Self::is_service_like(&order.product_category),
                    Error::<T>::ServiceLikeOrderCannotShip
                );
                ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);
                ensure!(!order.confirm_extended, Error::<T>::AlreadyExtended);

                let sa = order.shipped_at;
                order.confirm_extended = true;
                Ok(sa)
            })?;

            // H4: 清理 ship_order 创建的旧 ConfirmTimeout 条目
            if let Some(sa) = shipped_at {
                let old_expiry = sa.saturating_add(T::ConfirmTimeout::get());
                ExpiryQueue::<T>::mutate(old_expiry, |ids| {
                    ids.retain(|&id| id != order_id);
                });
            }

            ExpiryQueue::<T>::try_mutate(new_deadline, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Self::deposit_event(Event::ConfirmTimeoutExtended { order_id, new_deadline });
            Ok(())
        }

        /// 清理店铺订单索引（移除已终态的订单 ID，释放 BoundedVec 容量）
        ///
        /// 仅店铺 owner 可调用
        #[pallet::call_index(17)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn cleanup_shop_orders(origin: OriginFor<T>, shop_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::ShopProvider::shop_owner(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let orders = ShopOrders::<T>::get(shop_id);
            let before = orders.len() as u32;

            let retained: Vec<u64> = orders.iter().copied().filter(|&oid| {
                Orders::<T>::get(oid)
                    .map(|o| !matches!(o.status, OrderStatus::Completed | OrderStatus::Cancelled | OrderStatus::Refunded))
                    .unwrap_or(false)
            }).collect();

            let after = retained.len() as u32;
            let removed = before.saturating_sub(after);
            ensure!(removed > 0, Error::<T>::NothingToClean);

            let bounded: BoundedVec<u64, ConstU32<10000>> = retained
                .try_into()
                .expect("retained is subset of original bounded vec");
            ShopOrders::<T>::insert(shop_id, bounded);

            Self::deposit_event(Event::ShopOrdersCleaned { shop_id, removed });
            Ok(())
        }

        /// 卖家更新物流信息（仅 Shipped 状态）
        ///
        /// 允许卖家在发货后修改/更新物流追踪 CID（如更换快递单号）
        #[pallet::call_index(18)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn update_tracking(
            origin: OriginFor<T>,
            order_id: u64,
            new_tracking_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!new_tracking_cid.is_empty(), Error::<T>::EmptyTrackingCid);
            let bounded_cid: BoundedVec<u8, T::MaxCidLength> =
                new_tracking_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);

                order.tracking_cid = Some(bounded_cid);
                Ok(())
            })?;

            Self::deposit_event(Event::TrackingInfoUpdated { order_id });
            Ok(())
        }

        /// 卖家主动退款（Shipped 状态，含发货后发现问题等场景）
        #[pallet::call_index(19)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn seller_refund_order(
            origin: OriginFor<T>,
            order_id: u64,
            reason_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let bounded_reason = Self::validate_reason_cid(reason_cid)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.seller == who, Error::<T>::NotOrderSeller);
            ensure!(
                order.product_category != ProductCategory::Digital,
                Error::<T>::DigitalProductCannotCancel
            );
            ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);

            Self::do_cancel_or_refund(&order, order_id, OrderStatus::Refunded)?;

            Self::deposit_event(Event::OrderSellerRefunded {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
                reason_cid: bounded_reason.into_inner(),
            });
            Ok(())
        }

        /// 管理员部分退款（Root，仅 NEX 订单）
        ///
        /// refund_bps: 退给买家的比例（基点，1-9999），剩余归卖家
        #[pallet::call_index(20)]
        #[pallet::weight(Weight::from_parts(300_000_000, 12_000))]
        pub fn force_partial_refund(
            origin: OriginFor<T>,
            order_id: u64,
            refund_bps: u16,
            reason_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(refund_bps >= 1 && refund_bps <= 9999, Error::<T>::InvalidRefundBps);
            let reason = Self::validate_optional_reason_cid(&reason_cid)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(
                order.payment_asset == PaymentAsset::Native,
                Error::<T>::PartialRefundNotSupported
            );
            ensure!(
                matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
                Error::<T>::CannotForceOrder
            );

            if order.status == OrderStatus::Disputed {
                let _ = T::Escrow::set_resolved(order_id);
            }

            let release_bps = 10000u16.saturating_sub(refund_bps);
            T::Escrow::split_partial(order_id, &order.seller, &order.buyer, release_bps)?;

            Self::cancel_commission_by_asset(&order, order_id);

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Refunded;
                }
            });

            OrderReferrer::<T>::remove(order_id);

            Self::deposit_event(Event::OrderPartialRefunded { order_id, refund_bps, reason_cid: reason });
            Ok(())
        }

        /// 买家撤回争议（仅卖家尚未拒绝时可用）
        ///
        /// 恢复订单到争议前状态（Paid / Shipped），重建相应超时队列
        #[pallet::call_index(21)]
        #[pallet::weight(Weight::from_parts(200_000_000, 8_000))]
        pub fn withdraw_dispute(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
            ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);
            ensure!(!order.dispute_rejected, Error::<T>::DisputeAlreadyRejected);

            let restored_status = if order.shipped_at.is_some() {
                OrderStatus::Shipped
            } else {
                OrderStatus::Paid
            };

            if order.payment_asset == PaymentAsset::Native {
                T::Escrow::set_resolved(order_id)?;
            }

            // 清理争议超时队列条目
            if let Some(deadline) = order.dispute_deadline {
                ExpiryQueue::<T>::mutate(deadline, |ids| {
                    ids.retain(|&id| id != order_id);
                });
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let new_expiry = match restored_status {
                OrderStatus::Shipped => {
                    if Self::is_service_like(&order.product_category) {
                        now.saturating_add(T::ServiceConfirmTimeout::get())
                    } else {
                        now.saturating_add(T::ConfirmTimeout::get())
                    }
                },
                _ => now.saturating_add(T::ShipTimeout::get()),
            };

            ExpiryQueue::<T>::try_mutate(new_expiry, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = restored_status;
                    o.dispute_deadline = None;
                    o.dispute_rejected = false;
                    o.refund_reason_cid = None;
                }
            });

            Self::deposit_event(Event::DisputeWithdrawn { order_id });
            Ok(())
        }

        /// 管理员手动处理指定区块的过期订单（解决 C1 孤立条目问题）
        ///
        /// 当 on_idle weight 不足导致某区块的超时订单未被完全处理时，
        /// Root 可调用此接口指定区块号进行补偿处理。
        #[pallet::call_index(22)]
        #[pallet::weight(Weight::from_parts(500_000_000, 20_000))]
        pub fn force_process_expirations(
            origin: OriginFor<T>,
            target_block: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let weight = Self::process_expired_orders(now, target_block, 500);
            let _ = weight;

            let remaining = ExpiryQueue::<T>::get(target_block).len() as u32;

            Self::deposit_event(Event::StaleExpirationsProcessed {
                target_block,
                processed: 500u32.saturating_sub(remaining),
            });
            Ok(())
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 商品类别是否需要物流
        fn category_requires_shipping(cat: &ProductCategory) -> bool {
            matches!(cat, ProductCategory::Physical | ProductCategory::Bundle | ProductCategory::Other)
        }

        /// 是否为服务类/订阅类（共享 start_service/complete_service/confirm_service 流程）
        fn is_service_like(cat: &ProductCategory) -> bool {
            matches!(cat, ProductCategory::Service | ProductCategory::Subscription)
        }

        fn validate_reason_cid(cid: Vec<u8>) -> Result<BoundedVec<u8, T::MaxCidLength>, DispatchError> {
            ensure!(!cid.is_empty(), Error::<T>::EmptyReasonCid);
            let bounded: BoundedVec<u8, T::MaxCidLength> = cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            Ok(bounded)
        }

        fn validate_optional_reason_cid(cid: &Option<Vec<u8>>) -> Result<Option<Vec<u8>>, DispatchError> {
            if let Some(c) = cid {
                ensure!(!c.is_empty(), Error::<T>::EmptyReasonCid);
                let _: BoundedVec<u8, T::MaxCidLength> = c.clone().try_into().map_err(|_| Error::<T>::CidTooLong)?;
            }
            Ok(cid.clone())
        }

        fn do_cancel_or_refund(order: &OrderOf<T>, order_id: u64, final_status: OrderStatus) -> DispatchResult {
            Self::refund_by_asset(order, order_id)?;
            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }
            Self::cancel_commission_by_asset(order, order_id);
            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = final_status;
                }
            });
            OrderReferrer::<T>::remove(order_id);
            Ok(())
        }

        fn do_complete_order(order_id: u64, order: &OrderOf<T>) -> DispatchResult {
            let seller_amount = order.total_amount.saturating_sub(order.platform_fee);
            let entity_id = order.entity_id;

            let token_platform_fee: u128 = match order.payment_asset {
                PaymentAsset::Native => 0u128,
                PaymentAsset::EntityToken => {
                    let ta: u128 = order.token_payment_amount;
                    let tfr = T::TokenCommissionHandler::token_platform_fee_rate(entity_id) as u128;
                    // M3-R8: 防御性上限 — 费率不超过 10000 bps (100%)，防止外部错误配置导致卖家收入为 0
                    let safe_rate = tfr.min(10000u128);
                    ta.saturating_mul(safe_rate) / 10000u128
                },
            };

            match order.payment_asset {
                PaymentAsset::Native => {
                    // NEX 支付：从托管释放资金给卖家
                    T::Escrow::transfer_from_escrow(order_id, &order.seller, seller_amount)?;
                    // 平台费转给平台账户
                    if !order.platform_fee.is_zero() {
                        T::Escrow::transfer_from_escrow(order_id, &T::PlatformAccount::get(), order.platform_fee)?;
                    }
                },
                PaymentAsset::EntityToken => {
                    // Token 支付：使用预先计算的平台费拆分转账
                    let token_amount: u128 = order.token_payment_amount;
                    let token_seller_amount = token_amount.saturating_sub(token_platform_fee);

                    // 卖家获得扣除平台费后的金额
                    let seller_token: BalanceOf<T> = token_seller_amount.saturated_into();
                    T::EntityToken::repatriate_reserved(
                        entity_id, &order.buyer, &order.seller, seller_token,
                    )?;

                    // M1-fix: 平台费转入 entity_account，失败时发事件而非静默吞错
                    if token_platform_fee > 0 {
                        let entity_account = T::TokenCommissionHandler::entity_account(entity_id);
                        let fee_token: BalanceOf<T> = token_platform_fee.saturated_into();
                        if T::EntityToken::repatriate_reserved(
                            entity_id, &order.buyer, &entity_account, fee_token,
                        ).is_err() {
                            Self::deposit_event(Event::OrderOperationFailed {
                                order_id,
                                operation: OrderOperation::TokenPlatformFee,
                            });
                        }
                    }
                },
            }

            let now = <frame_system::Pallet<T>>::block_number();

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Completed;
                    o.completed_at = Some(now);
                }
            });

            // 自动注册买家为会员 + 更新消费金额（best-effort）
            // auto_register: 首次购买时注册会员（PURCHASE_REQUIRED 策略触发点）
            // update_spent: 更新消费金额 + 激活待激活会员 + 触发等级升级
            let referrer = OrderReferrer::<T>::take(order_id);
            if T::MemberHandler::auto_register(entity_id, &order.buyer, referrer).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::MemberAutoRegister });
            }
            // H1-R5-fix: Token 订单未花费 NEX，不应将 token 数量当作 NEX 做 USDT 转换
            let amount_usdt: u64 = match order.payment_asset {
                PaymentAsset::Native => {
                    // NEX → USDT 转换: price 精度 10^6, NEX 精度 10^12
                    // amount_usdt (6 dec) = amount_nex (12 dec) * price (6 dec) / 10^12
                    let amount_nex: u128 = order.total_amount.saturated_into();
                    let nex_price: u128 = T::PricingProvider::get_nex_usdt_price() as u128;
                    amount_nex.saturating_mul(nex_price)
                        .checked_div(1_000_000_000_000u128)
                        .unwrap_or(0) as u64
                },
                PaymentAsset::EntityToken => {
                    // F2-fix: Token → NEX → USDT 间接换算
                    // 仅在 Token 价格可靠（confidence ≥ 30）时换算，否则安全降级为 0
                    if T::TokenPriceProvider::is_token_price_reliable(entity_id) {
                        if let Some(token_nex_price) = T::TokenPriceProvider::get_token_price(entity_id) {
                            let nex_usdt: u128 = T::PricingProvider::get_nex_usdt_price() as u128;
                            if nex_usdt > 0 {
                                let token_nex_u128: u128 = token_nex_price.saturated_into();
                                // amount_usdt = token_amount × (NEX/Token) × (USDT/NEX) / 10^12
                                // M1-audit: checked_mul 防止三路乘法溢出（saturating_mul + as u64 会产生垃圾值）
                                order.token_payment_amount
                                    .checked_mul(token_nex_u128)
                                    .and_then(|v| v.checked_mul(nex_usdt))
                                    .and_then(|v| v.checked_div(1_000_000_000_000u128))
                                    .unwrap_or(0) as u64
                            } else { 0u64 }
                        } else { 0u64 }
                    } else {
                        0u64
                    }
                },
            };
            if T::MemberHandler::update_spent(
                entity_id,
                &order.buyer,
                amount_usdt,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::MemberUpdate });
            }

            // 触发升级规则引擎（best-effort，失败发事件）
            // update_spent 先执行，确保 total_spent 已含本单；规则引擎读取最新 member 快照
            if T::MemberHandler::check_order_upgrade_rules(
                entity_id,
                &order.buyer,
                order.product_id,
                amount_usdt,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::UpgradeRuleCheck });
            }

            // H2-fix: 更新店铺统计 — Token 订单使用 token_payment_amount，NEX 订单使用 seller_amount
            let shop_stats_amount: u128 = match order.payment_asset {
                PaymentAsset::Native => seller_amount.saturated_into(),
                PaymentAsset::EntityToken => order.token_payment_amount,
            };
            if T::ShopProvider::update_shop_stats(
                order.shop_id,
                shop_stats_amount,
                1,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::ShopStatsUpdate });
            }

            // 触发佣金计算（best-effort，失败发事件）
            // 根据支付资产类型分支：Native → NEX 佣金，EntityToken → Token 佣金
            match order.payment_asset {
                PaymentAsset::Native => {
                    if T::CommissionHandler::on_order_completed(
                        entity_id,
                        order.shop_id,
                        order_id,
                        &order.buyer,
                        order.total_amount,
                        order.platform_fee,
                    ).is_err() {
                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionComplete });
                    }
                },
                PaymentAsset::EntityToken => {
                    if T::TokenCommissionHandler::on_token_order_completed(
                        entity_id,
                        order.shop_id,
                        order_id,
                        &order.buyer,
                        order.token_payment_amount,
                        token_platform_fee,
                    ).is_err() {
                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionComplete });
                    }
                },
            }

            // M3-fix: 发放购物积分奖励 — Token 订单使用 token_payment_amount 转换为 Balance
            let reward_amount: BalanceOf<T> = match order.payment_asset {
                PaymentAsset::Native => order.total_amount,
                PaymentAsset::EntityToken => order.token_payment_amount.saturated_into(),
            };
            if T::EntityToken::reward_on_purchase(
                entity_id,
                &order.buyer,
                reward_amount,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::TokenReward });
            }

            // M2-R5-fix + M2-R8-fix: NEX/Token 统计分别追踪
            OrderStats::<T>::mutate(|stats| {
                stats.completed_orders = stats.completed_orders.saturating_add(1);
                match order.payment_asset {
                    PaymentAsset::Native => {
                        stats.total_volume = stats.total_volume.saturating_add(order.total_amount);
                        stats.total_platform_fees = stats.total_platform_fees.saturating_add(order.platform_fee);
                    },
                    PaymentAsset::EntityToken => {
                        stats.total_token_volume = stats.total_token_volume.saturating_add(order.token_payment_amount);
                        stats.total_token_platform_fees = stats.total_token_platform_fees.saturating_add(token_platform_fee);
                    },
                }
            });

            // M1-R7-fix: Token 订单卖家 NEX 收入为 0，seller_received 应反映实际 NEX 收入
            let token_seller_received: u128 = match order.payment_asset {
                PaymentAsset::Native => 0u128,
                PaymentAsset::EntityToken => order.token_payment_amount.saturating_sub(token_platform_fee),
            };
            let nex_seller_received: BalanceOf<T> = match order.payment_asset {
                PaymentAsset::Native => seller_amount,
                PaymentAsset::EntityToken => Zero::zero(),
            };
            Self::deposit_event(Event::OrderCompleted {
                order_id,
                seller_received: nex_seller_received,
                token_seller_received,
            });

            Ok(())
        }

        /// 根据支付资产类型取消佣金（best-effort，失败发事件）
        fn cancel_commission_by_asset(order: &OrderOf<T>, order_id: u64) {
            match order.payment_asset {
                PaymentAsset::Native => {
                    if T::CommissionHandler::on_order_cancelled(order_id).is_err() {
                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionCancel });
                    }
                },
                PaymentAsset::EntityToken => {
                    if T::TokenCommissionHandler::on_token_order_cancelled(order_id).is_err() {
                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionCancel });
                    }
                },
            }
        }

        /// 根据支付资产类型退款（Token 用 unreserve，NEX 用 Escrow refund）
        /// 返回 Ok(()) 表示成功，Err 表示 NEX escrow 退款失败
        fn refund_by_asset(order: &OrderOf<T>, order_id: u64) -> DispatchResult {
            match order.payment_asset {
                PaymentAsset::Native => {
                    T::Escrow::refund_all(order_id, &order.buyer)?;
                },
                PaymentAsset::EntityToken => {
                    T::EntityToken::unreserve(order.entity_id, &order.buyer, order.total_amount);
                },
            }
            Ok(())
        }

        fn do_auto_refund(order: &OrderOf<T>, order_id: u64) -> bool {
            if Self::do_cancel_or_refund(order, order_id, OrderStatus::Refunded).is_ok() {
                Self::deposit_event(Event::OrderRefunded {
                    order_id,
                    amount: order.total_amount,
                    token_amount: order.token_payment_amount,
                });
                true
            } else {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                false
            }
        }

        /// 处理过期订单（基于 ExpiryQueue 精确索引）
        ///
        /// now: 当前区块号（用于判断 deadline 是否到达）
        /// target_block: 要处理的 ExpiryQueue key（通常 = now，force 时可指定过去区块）
        /// 二次确认订单状态：可能已被手动确认/取消/退款
        fn process_expired_orders(now: BlockNumberFor<T>, target_block: BlockNumberFor<T>, max_count: u32) -> Weight {
            let order_ids = ExpiryQueue::<T>::get(target_block);
            if order_ids.is_empty() {
                // M2-R9-fix: 仅消耗 1 次 storage read，补充 proof_size
                return Weight::from_parts(5_000, 64);
            }

            let mut processed = 0u32;
            let mut iterated = 0usize;

            for &order_id in order_ids.iter() {
                if processed >= max_count {
                    // 未遍历的全部保留
                    break;
                }
                iterated = iterated.saturating_add(1);

                if let Some(order) = Orders::<T>::get(order_id) {
                    match order.status {
                        // 发货超时：自动退款（L1-R9-fix: 统一使用 do_auto_refund 消除重复代码）
                        OrderStatus::Paid => {
                            if Self::do_auto_refund(&order, order_id) {
                                processed = processed.saturating_add(1);
                            }
                        }
                        // 确认超时：自动确认收货/服务
                        OrderStatus::Shipped => {
                            if Self::is_service_like(&order.product_category)
                                && order.service_completed_at.is_none()
                            {
                                // H4+H5: 服务已开始但未完成 — 检查是否超过 ServiceConfirmTimeout
                                if let Some(started_at) = order.service_started_at {
                                    let deadline = started_at.saturating_add(T::ServiceConfirmTimeout::get());
                                    if now >= deadline {
                                        // 卖家超时未完成服务 → 自动退款
                                        if Self::do_auto_refund(&order, order_id) {
                                            processed = processed.saturating_add(1);
                                        }
                                    }
                                    // else: 服务期限内，跳过（start_service 已在正确的 deadline 区块创建了独立条目）
                                }
                                // else: service_started_at 为 None（理论上不应出现），跳过
                            } else if Self::do_complete_order(order_id, &order).is_ok() {
                                processed = processed.saturating_add(1);
                            } else {
                                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::AutoComplete });
                            }
                        }
                        // 争议超时：仅在 dispute_deadline 到期后自动退款
                        OrderStatus::Disputed => {
                            let deadline_reached = order.dispute_deadline
                                .map(|d| now >= d)
                                .unwrap_or(false);
                            if deadline_reached {
                                // 解除争议锁定（仅 Native 需要）
                                if order.payment_asset == PaymentAsset::Native {
                                    let _ = T::Escrow::set_resolved(order_id);
                                }
                                if Self::do_auto_refund(&order, order_id) {
                                    processed = processed.saturating_add(1);
                                }
                            }
                            // else: 非争议超时条目（如 ShipTimeout），跳过
                        }
                        // 已被手动处理（取消/退款/确认等），跳过（从队列移除）
                        _ => {}
                    }
                }
            }

            if iterated >= order_ids.len() {
                ExpiryQueue::<T>::remove(target_block);
            } else {
                let remaining: Vec<u64> = order_ids.iter().skip(iterated).copied().collect();
                let bounded: BoundedVec<u64, ConstU32<500>> = remaining
                    .try_into()
                    .expect("remaining is subset of original bounded vec");
                ExpiryQueue::<T>::insert(target_block, bounded);
            }

            // M1-R8: 精确报告 weight：读队列 + 每个处理订单读写 + 每个跳过订单读开销
            let skipped = (iterated as u64).saturating_sub(processed as u64);
            Weight::from_parts(
                50_000_000u64
                    .saturating_add(200_000_000u64.saturating_mul(processed as u64))
                    .saturating_add(25_000_000u64.saturating_mul(skipped)),
                4_000u64
                    .saturating_add(8_000u64.saturating_mul(processed as u64))
                    .saturating_add(2_000u64.saturating_mul(skipped)),
            )
        }
    }

    // ==================== OrderProvider 实现 ====================

    impl<T: Config> OrderProvider<T::AccountId, BalanceOf<T>> for Pallet<T> {
        fn order_exists(order_id: u64) -> bool {
            Orders::<T>::contains_key(order_id)
        }

        fn order_buyer(order_id: u64) -> Option<T::AccountId> {
            Orders::<T>::get(order_id).map(|o| o.buyer)
        }

        fn order_seller(order_id: u64) -> Option<T::AccountId> {
            Orders::<T>::get(order_id).map(|o| o.seller)
        }

        fn order_amount(order_id: u64) -> Option<BalanceOf<T>> {
            Orders::<T>::get(order_id).map(|o| o.total_amount)
        }

        fn order_shop_id(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id).map(|o| o.shop_id)
        }

        fn is_order_completed(order_id: u64) -> bool {
            Orders::<T>::get(order_id)
                .map(|o| o.status == OrderStatus::Completed)
                .unwrap_or(false)
        }

        fn is_order_disputed(order_id: u64) -> bool {
            Orders::<T>::get(order_id)
                .map(|o| o.status == OrderStatus::Disputed)
                .unwrap_or(false)
        }

        fn can_dispute(order_id: u64, who: &T::AccountId) -> bool {
            Orders::<T>::get(order_id)
                .map(|o| {
                    let is_buyer = o.buyer == *who;
                    let status_ok = matches!(o.status, OrderStatus::Paid | OrderStatus::Shipped);
                    is_buyer && status_ok
                })
                .unwrap_or(false)
        }

        fn order_token_amount(order_id: u64) -> Option<u128> {
            Orders::<T>::get(order_id).map(|o| o.token_payment_amount)
        }

        fn order_payment_asset(order_id: u64) -> Option<PaymentAsset> {
            Orders::<T>::get(order_id).map(|o| o.payment_asset)
        }

        fn order_completed_at(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id)
                .and_then(|o| o.completed_at)
                .map(|b| b.try_into().unwrap_or(u64::MAX))
        }

        fn order_status(order_id: u64) -> Option<OrderStatus> {
            Orders::<T>::get(order_id).map(|o| o.status)
        }

        fn order_entity_id(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id).map(|o| o.entity_id)
        }

        fn order_product_id(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id).map(|o| o.product_id)
        }

        fn order_quantity(order_id: u64) -> Option<u32> {
            Orders::<T>::get(order_id).map(|o| o.quantity)
        }

        fn order_created_at(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id)
                .map(|o| o.created_at.try_into().unwrap_or(u64::MAX))
        }

        fn order_paid_at(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id)
                .map(|o| o.created_at.try_into().unwrap_or(u64::MAX))
        }

        fn order_shipped_at(order_id: u64) -> Option<u64> {
            Orders::<T>::get(order_id)
                .and_then(|o| o.shipped_at)
                .map(|b| b.try_into().unwrap_or(u64::MAX))
        }
    }
}
