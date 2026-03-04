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
    use pallet_entity_common::{OrderStatus, OrderCommissionHandler, OrderMemberHandler, OrderProvider, PaymentAsset, PricingProvider, ProductCategory, ProductProvider, EntityTokenProvider, ShopProvider, ShoppingBalanceProvider, TokenOrderCommissionHandler};
    use sp_runtime::{traits::{Saturating, Zero}, SaturatedConversion};

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// 订单信息
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct Order<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
        /// 订单 ID
        pub id: u64,
        /// 店铺 ID
        pub shop_id: u64,
        /// 商品 ID
        pub product_id: u64,
        /// 买家账户
        pub buyer: AccountId,
        /// 卖家账户
        pub seller: AccountId,
        /// 购买数量
        pub quantity: u32,
        /// 单价
        pub unit_price: Balance,
        /// 总金额
        pub total_amount: Balance,
        /// 平台费
        pub platform_fee: Balance,
        /// 商品类别（决定订单流程）
        pub product_category: ProductCategory,
        /// 是否需要物流
        pub requires_shipping: bool,
        /// 收货地址 IPFS CID
        pub shipping_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 物流信息 IPFS CID
        pub tracking_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 订单状态
        pub status: OrderStatus,
        /// 创建时间
        pub created_at: BlockNumber,
        /// 支付时间
        pub paid_at: Option<BlockNumber>,
        /// 发货时间
        pub shipped_at: Option<BlockNumber>,
        /// 完成时间
        pub completed_at: Option<BlockNumber>,
        /// 服务开始时间（服务类订单）
        pub service_started_at: Option<BlockNumber>,
        /// 服务完成时间（服务类订单，卖家标记）
        pub service_completed_at: Option<BlockNumber>,
        /// 托管 ID
        pub escrow_id: u64,
        /// 支付资产类型（默认 Native = NEX）
        pub payment_asset: PaymentAsset,
        /// Token 支付金额（仅 EntityToken 时有效，u128 避免泛型膨胀）
        pub token_payment_amount: u128,
        /// 买家是否已延长确认收货期限（限延一次）
        pub confirm_extended: bool,
        /// 争议超时截止区块（reject_refund 时设置）
        pub dispute_deadline: Option<BlockNumber>,
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
        /// Token 平台费转移失败
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
    /// place_order 写入 [now + ShipTimeout]，ship_order 写入 [now + ConfirmTimeout]
    /// on_idle 仅检查当前区块对应的队列，O(K) 复杂度
    #[pallet::storage]
    pub type ExpiryQueue<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<u64, ConstU32<500>>,
        ValueQuery,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 订单已创建
        OrderCreated {
            order_id: u64,
            buyer: T::AccountId,
            seller: T::AccountId,
            amount: BalanceOf<T>,
            payment_asset: PaymentAsset,
            token_amount: u128,
        },
        /// 订单已支付
        OrderPaid { order_id: u64, escrow_id: u64 },
        /// 订单已发货
        OrderShipped { order_id: u64 },
        /// 订单已完成
        OrderCompleted {
            order_id: u64,
            seller_received: BalanceOf<T>,
            token_seller_received: u128,
        },
        /// 订单已取消
        OrderCancelled { order_id: u64, amount: BalanceOf<T>, token_amount: u128 },
        /// 订单已退款
        OrderRefunded {
            order_id: u64,
            amount: BalanceOf<T>,
            token_amount: u128,
        },
        /// 订单进入争议
        OrderDisputed { order_id: u64 },
        /// 订单附属操作失败（主流程已完成，需人工干预）
        OrderOperationFailed { order_id: u64, operation: OrderOperation },
        /// 服务已开始
        ServiceStarted { order_id: u64 },
        /// 服务已完成（卖家标记）
        ServiceCompleted { order_id: u64 },
        /// NEX 平台费率已更新
        PlatformFeeRateUpdated { old_rate: u16, new_rate: u16 },
        /// 买家订单索引已清理
        BuyerOrdersCleaned { buyer: T::AccountId, removed: u32 },
        /// 卖家拒绝退款
        RefundRejected { order_id: u64 },
        /// 卖家取消订单
        OrderSellerCancelled { order_id: u64, amount: BalanceOf<T>, token_amount: u128 },
        /// 管理员强制退款
        OrderForceRefunded { order_id: u64 },
        /// 管理员强制完成
        OrderForceCompleted { order_id: u64 },
        /// 买家修改收货地址
        ShippingAddressUpdated { order_id: u64 },
        /// 买家延长确认收货期限
        ConfirmTimeoutExtended { order_id: u64, new_deadline: BlockNumberFor<T> },
        /// 店铺订单索引已清理
        ShopOrdersCleaned { shop_id: u64, removed: u32 },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 订单不存在
        OrderNotFound,
        /// 商品不存在
        ProductNotFound,
        /// 店铺不存在
        ShopNotFound,
        /// 不是订单买家
        NotOrderBuyer,
        /// 不是订单卖家
        NotOrderSeller,
        /// 无效的订单状态
        InvalidOrderStatus,
        /// 无法取消订单
        CannotCancelOrder,
        /// 不能购买自己店铺的商品
        CannotBuyOwnProduct,
        /// 商品不在售
        ProductNotOnSale,
        /// 库存不足
        InsufficientStock,
        /// CID 过长
        CidTooLong,
        /// 数值溢出
        Overflow,
        /// 数字商品不可取消
        DigitalProductCannotCancel,
        /// 无效数量（不能为 0）
        InvalidQuantity,
        /// 数字商品不可退款
        DigitalProductCannotRefund,
        /// 非服务类订单
        NotServiceOrder,
        /// 支付金额无效（不能为 0）
        InvalidAmount,
        /// 超时队列已满
        ExpiryQueueFull,
        /// 实物商品必须提供收货地址
        ShippingCidRequired,
        /// 服务类订单不可使用发货/收货流程
        ServiceOrderCannotShip,
        /// 退款理由 CID 不能为空
        EmptyReasonCid,
        /// 实体代币未启用
        EntityTokenNotEnabled,
        /// Token 余额不足
        InsufficientTokenBalance,
        /// 物流信息 CID 不能为空
        EmptyTrackingCid,
        /// 平台费率超过上限（最大 1000 bps = 10%）
        PlatformFeeRateTooHigh,
        /// 无可清理的终态订单
        NothingToClean,
        /// 不是店铺 Owner
        NotShopOwner,
        /// 订单已延长过
        AlreadyExtended,
        /// 订单不在可强制操作的状态
        CannotForceOrder,
    }

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 空闲时处理超时订单（基于 ExpiryQueue 精确索引）
        fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            // 每个订单处理约 200M weight（读 + 写 + escrow + commission 操作）
            let per_order_weight = Weight::from_parts(200_000_000, 8_000);
            // 至少能处理 1 个订单才值得进入
            if remaining_weight.ref_time() < per_order_weight.ref_time().saturating_add(50_000_000) {
                return Weight::zero();
            }

            let max_count = (remaining_weight.ref_time() / per_order_weight.ref_time()).min(20) as u32;
            Self::process_expired_orders(now, max_count)
        }
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 下单并支付
        ///
        /// # 参数
        /// - `product_id`: 商品 ID
        /// - `quantity`: 购买数量
        /// - `shipping_cid`: 收货地址 IPFS CID
        /// - `use_tokens`: 使用积分抵扣金额（可选）
        /// - `use_shopping_balance`: 使用购物余额抵扣（可选，NEX 从 Entity 转入买家后锁入托管）
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
        ) -> DispatchResult {
            let buyer = ensure_signed(origin)?;

            // 校验参数
            ensure!(quantity > 0, Error::<T>::InvalidQuantity);

            // 获取商品信息
            ensure!(T::ProductProvider::product_exists(product_id), Error::<T>::ProductNotFound);
            ensure!(T::ProductProvider::is_product_on_sale(product_id), Error::<T>::ProductNotOnSale);

            let shop_id = T::ProductProvider::product_shop_id(product_id)
                .ok_or(Error::<T>::ProductNotFound)?;
            let price = T::ProductProvider::product_price(product_id)
                .ok_or(Error::<T>::ProductNotFound)?;

            // 检查库存（None = 无限库存，Some(n) = 有限库存）
            if let Some(stock) = T::ProductProvider::product_stock(product_id) {
                ensure!(stock >= quantity, Error::<T>::InsufficientStock);
            }

            // 获取店铺信息
            ensure!(T::ShopProvider::shop_exists(shop_id), Error::<T>::ShopNotFound);
            ensure!(T::ShopProvider::is_shop_active(shop_id), Error::<T>::ShopNotFound);
            let seller = T::ShopProvider::shop_owner(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(seller != buyer, Error::<T>::CannotBuyOwnProduct);

            // H3-fix: 计算金额（checked_mul 防溢出）
            let total_amount = price.checked_mul(&quantity.into())
                .ok_or(Error::<T>::Overflow)?;
            
            // 获取 entity_id 用于积分/token 操作
            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            let resolved_payment_asset = payment_asset.unwrap_or(PaymentAsset::Native);

            // 积分抵扣（仅 Native 支付时可用）
            let mut final_amount = total_amount;
            if resolved_payment_asset == PaymentAsset::Native {
                if let Some(tokens) = use_tokens {
                    if !tokens.is_zero() && T::EntityToken::is_token_enabled(entity_id) {
                        let discount = T::EntityToken::redeem_for_discount(entity_id, &buyer, tokens)?;
                        final_amount = final_amount.saturating_sub(discount);
                    }
                }
                // 购物余额抵扣（NEX 从 Entity 账户转入买家钱包，随后由 Escrow 锁定）
                if let Some(shopping_amount) = use_shopping_balance {
                    if !shopping_amount.is_zero() {
                        ensure!(shopping_amount <= final_amount, Error::<T>::InvalidAmount);
                        T::ShoppingBalance::consume_shopping_balance(entity_id, &buyer, shopping_amount)?;
                        // final_amount 不变：买家钱包已收到 NEX，Escrow 将从中锁定全额
                    }
                }
            }

            // C2: 积分抵扣后金额不能为零
            ensure!(!final_amount.is_zero(), Error::<T>::InvalidAmount);
            
            // 平台费计算（NEX 用全局费率，Token 用 Entity 级费率）
            // Token 平台费记录在 order.platform_fee 中但以 NEX 单位为 0（实际费用在完成时从 Token 中扣除）
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

            // 获取商品类别
            let product_category = T::ProductProvider::product_category(product_id)
                .ok_or(Error::<T>::ProductNotFound)?;

            // 根据类别决定是否需要物流
            let requires_shipping = match product_category {
                ProductCategory::Digital => false,
                ProductCategory::Physical => true,
                ProductCategory::Service => false,
                ProductCategory::Subscription => false,
                ProductCategory::Bundle => true,
                ProductCategory::Other => true,
            };

            // M6: 实物商品必须提供收货地址
            if requires_shipping {
                ensure!(shipping_cid.is_some(), Error::<T>::ShippingCidRequired);
            }

            let order_id = NextOrderId::<T>::get();
            let now = <frame_system::Pallet<T>>::block_number();

            // 锁定资金：根据支付资产类型选择不同的锁定方式
            let token_payment_amount: u128 = match resolved_payment_asset {
                PaymentAsset::Native => {
                    // NEX 支付：锁定 NEX 到托管
                    T::Escrow::lock_from(&buyer, order_id, final_amount)?;
                    0u128
                },
                PaymentAsset::EntityToken => {
                    // Token 支付：验证并锁定 Entity Token
                    ensure!(T::EntityToken::is_token_enabled(entity_id), Error::<T>::EntityTokenNotEnabled);
                    let buyer_token_balance = T::EntityToken::token_balance(entity_id, &buyer);
                    ensure!(buyer_token_balance >= final_amount, Error::<T>::InsufficientTokenBalance);
                    T::EntityToken::reserve(entity_id, &buyer, final_amount)?;
                    final_amount.saturated_into::<u128>()
                },
            };

            // 扣减库存
            T::ProductProvider::deduct_stock(product_id, quantity)?;
            T::ProductProvider::add_sold_count(product_id, quantity)?;

            // 数字商品：支付后立即完成
            let initial_status = if product_category == ProductCategory::Digital {
                OrderStatus::Completed
            } else {
                OrderStatus::Paid
            };

            let order = Order {
                id: order_id,
                shop_id,
                product_id,
                buyer: buyer.clone(),
                seller: seller.clone(),
                quantity,
                unit_price: price,
                total_amount: final_amount,
                platform_fee,
                product_category,
                requires_shipping,
                shipping_cid,
                tracking_cid: None,
                status: initial_status,
                created_at: now,
                paid_at: Some(now),
                shipped_at: None,
                completed_at: if product_category == ProductCategory::Digital { Some(now) } else { None },
                service_started_at: None,
                service_completed_at: None,
                escrow_id: order_id,
                payment_asset: resolved_payment_asset,
                token_payment_amount,
                confirm_extended: false,
                dispute_deadline: None,
            };

            Orders::<T>::insert(order_id, &order);
            BuyerOrders::<T>::try_mutate(&buyer, |ids| ids.try_push(order_id))
                .map_err(|_| Error::<T>::Overflow)?;
            ShopOrders::<T>::try_mutate(shop_id, |ids| ids.try_push(order_id))
                .map_err(|_| Error::<T>::Overflow)?;
            // L1-fix: checked_add 防止 u64 溢出导致 ID 覆盖（与 tokensale 一致）
            let next_id = order_id.checked_add(1).ok_or(Error::<T>::Overflow)?;
            NextOrderId::<T>::put(next_id);

            // 写入过期检查队列（非数字商品才需要超时检查）
            if product_category != ProductCategory::Digital {
                let expiry_block = if requires_shipping {
                    now.saturating_add(T::ShipTimeout::get())
                } else {
                    // 服务类：等待卖家开始服务的超时（复用 ShipTimeout）
                    now.saturating_add(T::ShipTimeout::get())
                };
                ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                    ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
                })?;
            }

            OrderStats::<T>::mutate(|stats| {
                stats.total_orders = stats.total_orders.saturating_add(1);
            });

            // L9: 事件金额使用实际支付金额（积分抵扣后）
            Self::deposit_event(Event::OrderCreated {
                order_id,
                buyer: buyer.clone(),
                seller: seller.clone(),
                amount: final_amount,
                payment_asset: resolved_payment_asset,
                token_amount: token_payment_amount,
            });
            Self::deposit_event(Event::OrderPaid {
                order_id,
                escrow_id: order_id,
            });

            // 数字商品：立即完成订单
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
            
            // 数字商品不可取消
            ensure!(
                order.product_category != ProductCategory::Digital,
                Error::<T>::DigitalProductCannotCancel
            );
            
            // L1: place_order 直接设为 Paid，移除 Created 死分支
            ensure!(
                order.status == OrderStatus::Paid,
                Error::<T>::CannotCancelOrder
            );

            // 退款：根据支付资产类型选择不同的退款方式
            match order.payment_asset {
                PaymentAsset::Native => {
                    T::Escrow::refund_all(order_id, &order.buyer)?;
                },
                PaymentAsset::EntityToken => {
                    // L2-R7-fix: 显式报错而非回退到 shop_id
                    let entity_id = T::ShopProvider::shop_entity_id(order.shop_id)
                        .ok_or(Error::<T>::ShopNotFound)?;
                    T::EntityToken::unreserve(entity_id, &order.buyer, order.total_amount);
                },
            }

            // 恢复库存（best-effort，失败发事件）
            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }

            // C3: 通知佣金系统订单已取消（best-effort，失败发事件）
            Self::cancel_commission_by_asset(&order, order_id);

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Cancelled;
                }
            });

            Self::deposit_event(Event::OrderCancelled {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
            });
            Ok(())
        }

        /// 发货
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(200_000_000, 8_000))]
        pub fn ship_order(
            origin: OriginFor<T>,
            order_id: u64,
            tracking_cid: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                // C2: 服务类订单必须走 start_service 流程
                ensure!(
                    order.product_category != ProductCategory::Service,
                    Error::<T>::ServiceOrderCannotShip
                );
                ensure!(order.status == OrderStatus::Paid, Error::<T>::InvalidOrderStatus);

                ensure!(!tracking_cid.is_empty(), Error::<T>::EmptyTrackingCid);
                order.tracking_cid = Some(
                    tracking_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?
                );
                order.status = OrderStatus::Shipped;
                order.shipped_at = Some(<frame_system::Pallet<T>>::block_number());
                Ok(())
            })?;

            // 写入过期检查队列：发货后 ConfirmTimeout 区块自动确认
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
            // C2: 服务类订单必须走 confirm_service 流程
            ensure!(
                order.product_category != ProductCategory::Service,
                Error::<T>::ServiceOrderCannotShip
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

            // M5: 校验退款理由 CID
            ensure!(!reason_cid.is_empty(), Error::<T>::EmptyReasonCid);
            let _bounded_reason: BoundedVec<u8, T::MaxCidLength> =
                reason_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            // C1-fix: 捕获 payment_asset 避免额外存储读取
            let payment_asset = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<PaymentAsset, DispatchError> {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
                
                // 数字商品不可退款
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
                Ok(asset)
            })?;

            // C1-fix: 仅 Native 支付才通知 Escrow 进入争议状态
            // EntityToken 订单未使用 Escrow（资金通过 EntityToken::reserve 锁定），
            // 调用 set_disputed 会因 NoLock 而失败，导致 Token 订单买家无法申请退款
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

            // 解除争议锁定后退款给买家
            match order.payment_asset {
                PaymentAsset::Native => {
                    T::Escrow::set_resolved(order_id)?;
                    T::Escrow::refund_all(order_id, &order.buyer)?;
                },
                PaymentAsset::EntityToken => {
                    // L2-R7-fix: 显式报错而非回退到 shop_id
                    let entity_id = T::ShopProvider::shop_entity_id(order.shop_id)
                        .ok_or(Error::<T>::ShopNotFound)?;
                    T::EntityToken::unreserve(entity_id, &order.buyer, order.total_amount);
                },
            }

            // 恢复库存（best-effort，失败发事件）
            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }

            // C3: 通知佣金系统订单已取消（best-effort，失败发事件）
            Self::cancel_commission_by_asset(&order, order_id);

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Refunded;
                }
            });

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

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                ensure!(order.product_category == ProductCategory::Service, Error::<T>::NotServiceOrder);
                ensure!(order.status == OrderStatus::Paid, Error::<T>::InvalidOrderStatus);

                order.status = OrderStatus::Shipped;  // 复用 Shipped 状态表示服务进行中
                order.service_started_at = Some(<frame_system::Pallet<T>>::block_number());
                Ok(())
            })?;

            // H4: 写入过期检查队列，服务开始后 ServiceConfirmTimeout 内未完成则可超时处理
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

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.seller == who, Error::<T>::NotOrderSeller);
                ensure!(order.product_category == ProductCategory::Service, Error::<T>::NotServiceOrder);
                ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);
                // M1-R6-fix: 防止重复调用 complete_service 填充 ExpiryQueue
                ensure!(order.service_completed_at.is_none(), Error::<T>::InvalidOrderStatus);

                order.service_completed_at = Some(<frame_system::Pallet<T>>::block_number());
                Ok(())
            })?;

            // 写入过期检查队列：服务完成后 ServiceConfirmTimeout 区块自动确认
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
            ensure!(order.product_category == ProductCategory::Service, Error::<T>::NotServiceOrder);
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

            ensure!(!reason_cid.is_empty(), Error::<T>::EmptyReasonCid);
            let _bounded_reason: BoundedVec<u8, T::MaxCidLength> =
                reason_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.seller == who, Error::<T>::NotOrderSeller);
            ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);

            // 设置争议截止时间并写入超时队列
            let now = <frame_system::Pallet<T>>::block_number();
            let expiry_block = now.saturating_add(T::DisputeTimeout::get());

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.dispute_deadline = Some(expiry_block);
                }
            });

            ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
                ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
            })?;

            Self::deposit_event(Event::RefundRejected { order_id });
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

            ensure!(!reason_cid.is_empty(), Error::<T>::EmptyReasonCid);
            let _bounded_reason: BoundedVec<u8, T::MaxCidLength> =
                reason_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.seller == who, Error::<T>::NotOrderSeller);
            ensure!(
                order.product_category != ProductCategory::Digital,
                Error::<T>::DigitalProductCannotCancel
            );
            ensure!(order.status == OrderStatus::Paid, Error::<T>::CannotCancelOrder);

            // 退款给买家
            Self::refund_by_asset(&order, order_id)?;

            // 恢复库存（best-effort）
            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }

            // 通知佣金系统
            Self::cancel_commission_by_asset(&order, order_id);

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Cancelled;
                }
            });

            Self::deposit_event(Event::OrderSellerCancelled {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
            });
            Ok(())
        }

        /// 管理员强制退款（Root / 治理）
        ///
        /// 可对 Paid / Shipped / Disputed 状态的订单强制退款
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(300_000_000, 12_000))]
        pub fn force_refund(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            ensure_root(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(
                matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
                Error::<T>::CannotForceOrder
            );

            // 解除争议锁定（仅 Native + Disputed 需要）
            if order.status == OrderStatus::Disputed && order.payment_asset == PaymentAsset::Native {
                let _ = T::Escrow::set_resolved(order_id);
            }

            Self::refund_by_asset(&order, order_id)?;

            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }

            Self::cancel_commission_by_asset(&order, order_id);

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Refunded;
                }
            });

            Self::deposit_event(Event::OrderForceRefunded { order_id });
            Self::deposit_event(Event::OrderRefunded {
                order_id,
                amount: order.total_amount,
                token_amount: order.token_payment_amount,
            });
            Ok(())
        }

        /// 管理员强制完成订单（Root / 治理）
        ///
        /// 可对 Paid / Shipped / Disputed 状态的订单强制完成（资金归卖家）
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(350_000_000, 16_000))]
        pub fn force_complete(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            ensure_root(origin)?;

            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(
                matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
                Error::<T>::CannotForceOrder
            );

            // 解除争议锁定（仅 Native + Disputed 需要）
            if order.status == OrderStatus::Disputed && order.payment_asset == PaymentAsset::Native {
                let _ = T::Escrow::set_resolved(order_id);
            }

            Self::do_complete_order(order_id, &order)?;
            Self::deposit_event(Event::OrderForceCompleted { order_id });
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
                ensure!(order.requires_shipping, Error::<T>::ServiceOrderCannotShip);

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

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
                ensure!(order.buyer == who, Error::<T>::NotOrderBuyer);
                ensure!(order.status == OrderStatus::Shipped, Error::<T>::InvalidOrderStatus);
                ensure!(!order.confirm_extended, Error::<T>::AlreadyExtended);

                order.confirm_extended = true;
                Ok(())
            })?;

            // 写入新的超时队列条目
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
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 完成订单（释放资金）
        fn do_complete_order(order_id: u64, order: &OrderOf<T>) -> DispatchResult {
            let seller_amount = order.total_amount.saturating_sub(order.platform_fee);

            // L2-R7-fix: 显式报错而非回退到 shop_id
            let entity_id = T::ShopProvider::shop_entity_id(order.shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // M2-R7-fix: 预先计算 Token 平台费（消除重复的 token_platform_fee_rate 存储读取）
            let token_platform_fee: u128 = match order.payment_asset {
                PaymentAsset::Native => 0u128,
                PaymentAsset::EntityToken => {
                    let ta: u128 = order.token_payment_amount;
                    let tfr = T::TokenCommissionHandler::token_platform_fee_rate(entity_id) as u128;
                    ta.saturating_mul(tfr) / 10000u128
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
            if T::MemberHandler::auto_register(entity_id, &order.buyer, None).is_err() {
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
                    // Token 订单：无 NEX 消费，amount_usdt 为 0
                    0u64
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

            // M2-R5-fix: total_volume 仅追踪 NEX，Token 走 total_token_volume
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
                    // L2-R7-fix: 显式报错而非回退到 shop_id
                    let entity_id = T::ShopProvider::shop_entity_id(order.shop_id)
                        .ok_or(Error::<T>::ShopNotFound)?;
                    T::EntityToken::unreserve(entity_id, &order.buyer, order.total_amount);
                },
            }
            Ok(())
        }

        /// 处理过期订单（基于 ExpiryQueue 精确索引）
        ///
        /// 仅检查当前区块到期的订单，O(K) 复杂度（K = 到期订单数）
        /// 二次确认订单状态：可能已被手动确认/取消/退款
        fn process_expired_orders(now: BlockNumberFor<T>, max_count: u32) -> Weight {
            let order_ids = ExpiryQueue::<T>::get(now);
            if order_ids.is_empty() {
                // 仅消耗 1 次 storage read
                return Weight::from_parts(5_000, 0);
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
                        // 发货超时：自动退款
                        OrderStatus::Paid => {
                            if order.requires_shipping {
                                // 实物商品：未发货 → 退款（失败则跳过，避免状态不一致）
                                if Self::refund_by_asset(&order, order_id).is_ok() {
                                    if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                    }
                                    Self::cancel_commission_by_asset(&order, order_id);
                                    Orders::<T>::mutate(order_id, |o| {
                                        if let Some(ord) = o {
                                            ord.status = OrderStatus::Refunded;
                                        }
                                    });
                                    // M1-R5-fix: 发射退款事件（之前遗漏）
                                    Self::deposit_event(Event::OrderRefunded {
                                        order_id,
                                        amount: order.total_amount,
                                        token_amount: order.token_payment_amount,
                                    });
                                    processed = processed.saturating_add(1);
                                } else {
                                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                }
                            } else if order.product_category == ProductCategory::Service {
                                // 服务类商品：卖家未开始服务 → 退款（失败则跳过）
                                if Self::refund_by_asset(&order, order_id).is_ok() {
                                    if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                    }
                                    Self::cancel_commission_by_asset(&order, order_id);
                                    Orders::<T>::mutate(order_id, |o| {
                                        if let Some(ord) = o {
                                            ord.status = OrderStatus::Refunded;
                                        }
                                    });
                                    // M1-R5-fix: 发射退款事件（之前遗漏）
                                    Self::deposit_event(Event::OrderRefunded {
                                        order_id,
                                        amount: order.total_amount,
                                        token_amount: order.token_payment_amount,
                                    });
                                    processed = processed.saturating_add(1);
                                } else {
                                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                }
                            } else {
                                // L4-R7-fix: 防御性兜底 — 未匹配的 Paid 订单也退款（避免未来新增 ProductCategory 时被静默丢弃）
                                if Self::refund_by_asset(&order, order_id).is_ok() {
                                    if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                    }
                                    Self::cancel_commission_by_asset(&order, order_id);
                                    Orders::<T>::mutate(order_id, |o| {
                                        if let Some(ord) = o {
                                            ord.status = OrderStatus::Refunded;
                                        }
                                    });
                                    Self::deposit_event(Event::OrderRefunded {
                                        order_id,
                                        amount: order.total_amount,
                                        token_amount: order.token_payment_amount,
                                    });
                                    processed = processed.saturating_add(1);
                                } else {
                                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                }
                            }
                        }
                        // 确认超时：自动确认收货/服务
                        OrderStatus::Shipped => {
                            if order.product_category == ProductCategory::Service
                                && order.service_completed_at.is_none()
                            {
                                // H4+H5: 服务已开始但未完成 — 检查是否超过 ServiceConfirmTimeout
                                if let Some(started_at) = order.service_started_at {
                                    let deadline = started_at.saturating_add(T::ServiceConfirmTimeout::get());
                                    if now >= deadline {
                                        // 卖家超时未完成服务 → 自动退款
                                        if Self::refund_by_asset(&order, order_id).is_ok() {
                                            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                            }
                                            Self::cancel_commission_by_asset(&order, order_id);
                                            Orders::<T>::mutate(order_id, |o| {
                                                if let Some(ord) = o {
                                                    ord.status = OrderStatus::Refunded;
                                                }
                                            });
                                            Self::deposit_event(Event::OrderRefunded {
                                                order_id,
                                                amount: order.total_amount,
                                                token_amount: order.token_payment_amount,
                                            });
                                            processed = processed.saturating_add(1);
                                        } else {
                                            Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                        }
                                    } else {
                                        // L1-R7-fix: 服务期限内，跳过此条目（start_service 已在正确的
                                        // deadline 区块创建了独立的 ExpiryQueue 条目，无需在此保留）
                                    }
                                } else {
                                    // service_started_at 为 None（理论上不应出现），跳过
                                }
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
                                if Self::refund_by_asset(&order, order_id).is_ok() {
                                    if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                    }
                                    Self::cancel_commission_by_asset(&order, order_id);
                                    Orders::<T>::mutate(order_id, |o| {
                                        if let Some(ord) = o {
                                            ord.status = OrderStatus::Refunded;
                                        }
                                    });
                                    Self::deposit_event(Event::OrderRefunded {
                                        order_id,
                                        amount: order.total_amount,
                                        token_amount: order.token_payment_amount,
                                    });
                                    processed = processed.saturating_add(1);
                                } else {
                                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                }
                            }
                            // else: 非争议超时条目（如 ShipTimeout），跳过
                        }
                        // 已被手动处理（取消/退款/确认等），跳过（从队列移除）
                        _ => {}
                    }
                }
            }

            // L1-R7-fix: 仅保留 weight 受限未遍历的订单（不再保留服务中订单的孤立条目）
            if iterated >= order_ids.len() {
                ExpiryQueue::<T>::remove(now);
            } else {
                let remaining: Vec<u64> = order_ids.iter().skip(iterated).copied().collect();
                let bounded: BoundedVec<u64, ConstU32<500>> = remaining
                    .try_into()
                    .expect("remaining is subset of original bounded vec");
                ExpiryQueue::<T>::insert(now, bounded);
            }

            // 精确报告 weight：读队列 + 每个订单读写 + escrow + commission 操作
            Weight::from_parts(
                50_000_000u64.saturating_add(200_000_000u64.saturating_mul(processed as u64)),
                4_000u64.saturating_add(8_000u64.saturating_mul(processed as u64)),
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
                    // 必须是买家或卖家
                    let is_party = o.buyer == *who || o.seller == *who;
                    // 订单状态必须是 Paid 或 Shipped（未完成且未争议）
                    let status_ok = matches!(o.status, OrderStatus::Paid | OrderStatus::Shipped);
                    is_party && status_ok
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
    }
}
