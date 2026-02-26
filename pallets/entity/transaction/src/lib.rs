//! # 商城订单管理模块 (pallet-entity-transaction)
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
    use pallet_escrow::pallet::Escrow as EscrowTrait;
    use pallet_entity_common::{OrderStatus, OrderCommissionHandler, OrderProvider, ProductCategory, ProductProvider, EntityTokenProvider, ShopProvider};
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
        /// 总交易额
        pub total_volume: Balance,
        /// 总平台费收入
        pub total_platform_fees: Balance,
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
        /// 订单自动完成
        AutoComplete,
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

        /// 平台费率（基点，200 = 2%）
        #[pallet::constant]
        type PlatformFeeRate: Get<u16>;

        /// 发货超时（区块数）
        #[pallet::constant]
        type ShipTimeout: Get<BlockNumberFor<Self>>;

        /// 确认收货超时（区块数）
        #[pallet::constant]
        type ConfirmTimeout: Get<BlockNumberFor<Self>>;

        /// 服务确认超时（区块数）
        #[pallet::constant]
        type ServiceConfirmTimeout: Get<BlockNumberFor<Self>>;

        /// 佣金处理接口（订单完成时触发返佣）
        type CommissionHandler: OrderCommissionHandler<Self::AccountId, BalanceOf<Self>>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

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
        },
        /// 订单已支付
        OrderPaid { order_id: u64, escrow_id: u64 },
        /// 订单已发货
        OrderShipped { order_id: u64 },
        /// 订单已完成
        OrderCompleted {
            order_id: u64,
            seller_received: BalanceOf<T>,
        },
        /// 订单已取消
        OrderCancelled { order_id: u64 },
        /// 订单已退款
        OrderRefunded {
            order_id: u64,
            amount: BalanceOf<T>,
        },
        /// 订单进入争议
        OrderDisputed { order_id: u64 },
        /// 订单附属操作失败（主流程已完成，需人工干预）
        OrderOperationFailed { order_id: u64, operation: OrderOperation },
        /// 服务已开始
        ServiceStarted { order_id: u64 },
        /// 服务已完成（卖家标记）
        ServiceCompleted { order_id: u64 },
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
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(350_000_000, 16_000))]
        pub fn place_order(
            origin: OriginFor<T>,
            product_id: u64,
            quantity: u32,
            shipping_cid: Option<Vec<u8>>,
            use_tokens: Option<BalanceOf<T>>,
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

            // 检查库存
            if let Some(stock) = T::ProductProvider::product_stock(product_id) {
                if stock > 0 {
                    ensure!(stock >= quantity, Error::<T>::InsufficientStock);
                }
            }

            // 获取店铺信息
            ensure!(T::ShopProvider::shop_exists(shop_id), Error::<T>::ShopNotFound);
            ensure!(T::ShopProvider::is_shop_active(shop_id), Error::<T>::ShopNotFound);
            let seller = T::ShopProvider::shop_owner(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(seller != buyer, Error::<T>::CannotBuyOwnProduct);

            // 计算金额
            let total_amount = price.saturating_mul(quantity.into());
            
            // 获取 entity_id 用于积分/token 操作
            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 积分抵扣
            let mut final_amount = total_amount;
            if let Some(tokens) = use_tokens {
                if !tokens.is_zero() && T::EntityToken::is_token_enabled(entity_id) {
                    let discount = T::EntityToken::redeem_for_discount(entity_id, &buyer, tokens)?;
                    final_amount = final_amount.saturating_sub(discount);
                }
            }
            // C2: 积分抵扣后金额不能为零
            ensure!(!final_amount.is_zero(), Error::<T>::InvalidAmount);
            
            let platform_fee = final_amount
                .saturating_mul(T::PlatformFeeRate::get().into())
                / 10000u32.into();

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
                ProductCategory::Other => true,
            };

            // M6: 实物商品必须提供收货地址
            if requires_shipping {
                ensure!(shipping_cid.is_some(), Error::<T>::ShippingCidRequired);
            }

            let order_id = NextOrderId::<T>::get();
            let now = <frame_system::Pallet<T>>::block_number();

            // 锁定资金到托管（扣除积分抵扣后的金额）
            T::Escrow::lock_from(&buyer, order_id, final_amount)?;

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
            };

            Orders::<T>::insert(order_id, &order);
            BuyerOrders::<T>::try_mutate(&buyer, |ids| ids.try_push(order_id))
                .map_err(|_| Error::<T>::Overflow)?;
            ShopOrders::<T>::try_mutate(shop_id, |ids| ids.try_push(order_id))
                .map_err(|_| Error::<T>::Overflow)?;
            NextOrderId::<T>::put(order_id.saturating_add(1));

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

            // 退款
            T::Escrow::refund_all(order_id, &order.buyer)?;

            // 恢复库存（best-effort，失败发事件）
            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }

            // C3: 通知佣金系统订单已取消（best-effort，失败发事件）
            if T::CommissionHandler::on_order_cancelled(order_id).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionCancel });
            }

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Cancelled;
                }
            });

            Self::deposit_event(Event::OrderCancelled { order_id });
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

            Orders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
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

                order.status = OrderStatus::Disputed;
                Ok(())
            })?;

            // 通知 Escrow 进入争议状态，阻止未经仲裁的资金操作
            T::Escrow::set_disputed(order_id)?;

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
            T::Escrow::set_resolved(order_id)?;
            T::Escrow::refund_all(order_id, &order.buyer)?;

            // 恢复库存（best-effort，失败发事件）
            if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
            }

            // C3: 通知佣金系统订单已取消（best-effort，失败发事件）
            if T::CommissionHandler::on_order_cancelled(order_id).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionCancel });
            }

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Refunded;
                }
            });

            Self::deposit_event(Event::OrderRefunded {
                order_id,
                amount: order.total_amount,
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
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 完成订单（释放资金）
        fn do_complete_order(order_id: u64, order: &OrderOf<T>) -> DispatchResult {
            let seller_amount = order.total_amount.saturating_sub(order.platform_fee);

            // 释放资金给卖家
            T::Escrow::transfer_from_escrow(order_id, &order.seller, seller_amount)?;

            // 平台费转给平台账户
            if !order.platform_fee.is_zero() {
                T::Escrow::transfer_from_escrow(order_id, &T::PlatformAccount::get(), order.platform_fee)?;
            }

            let now = <frame_system::Pallet<T>>::block_number();

            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(o) = maybe_order {
                    o.status = OrderStatus::Completed;
                    o.completed_at = Some(now);
                }
            });

            // 更新店铺统计（best-effort，失败发事件）
            if T::ShopProvider::update_shop_stats(
                order.shop_id,
                seller_amount.saturated_into(),
                1,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::ShopStatsUpdate });
            }

            // 触发佣金计算（best-effort，失败发事件）
            if T::CommissionHandler::on_order_completed(
                order.shop_id,
                order_id,
                &order.buyer,
                order.total_amount,
                order.platform_fee,
            ).is_err() {
                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionComplete });
            }

            // 发放购物积分奖励（需要 entity_id，不是 shop_id）
            if let Some(entity_id) = T::ShopProvider::shop_entity_id(order.shop_id) {
                if T::EntityToken::reward_on_purchase(
                    entity_id,
                    &order.buyer,
                    order.total_amount,
                ).is_err() {
                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::TokenReward });
                }
            }

            OrderStats::<T>::mutate(|stats| {
                stats.completed_orders = stats.completed_orders.saturating_add(1);
                stats.total_volume = stats.total_volume.saturating_add(order.total_amount);
                stats.total_platform_fees = stats.total_platform_fees.saturating_add(order.platform_fee);
            });

            Self::deposit_event(Event::OrderCompleted {
                order_id,
                seller_received: seller_amount,
            });

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
            let mut iterated = 0usize; // C1: 实际遍历位置（含跳过和失败的）

            for &order_id in order_ids.iter() {
                if processed >= max_count {
                    break;
                }
                iterated = iterated.saturating_add(1);

                if let Some(order) = Orders::<T>::get(order_id) {
                    match order.status {
                        // 发货超时：自动退款
                        OrderStatus::Paid => {
                            if order.requires_shipping {
                                // 实物商品：未发货 → 退款（Escrow 失败则跳过，避免状态不一致）
                                if T::Escrow::refund_all(order_id, &order.buyer).is_ok() {
                                    if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                    }
                                    if T::CommissionHandler::on_order_cancelled(order_id).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionCancel });
                                    }
                                    Orders::<T>::mutate(order_id, |o| {
                                        if let Some(ord) = o {
                                            ord.status = OrderStatus::Refunded;
                                        }
                                    });
                                    processed = processed.saturating_add(1);
                                } else {
                                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                }
                            } else if order.product_category == ProductCategory::Service {
                                // 服务类商品：卖家未开始服务 → 退款（Escrow 失败则跳过）
                                if T::Escrow::refund_all(order_id, &order.buyer).is_ok() {
                                    if T::ProductProvider::restore_stock(order.product_id, order.quantity).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::StockRestore });
                                    }
                                    if T::CommissionHandler::on_order_cancelled(order_id).is_err() {
                                        Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::CommissionCancel });
                                    }
                                    Orders::<T>::mutate(order_id, |o| {
                                        if let Some(ord) = o {
                                            ord.status = OrderStatus::Refunded;
                                        }
                                    });
                                    processed = processed.saturating_add(1);
                                } else {
                                    Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::EscrowRefund });
                                }
                            }
                        }
                        // 确认超时：自动确认收货/服务
                        OrderStatus::Shipped => {
                            // H5: 服务类订单必须 service_completed_at 已设置才能自动完成
                            // 否则这是来自 place_order 的 ShipTimeout 触发，应跳过
                            if order.product_category == ProductCategory::Service
                                && order.service_completed_at.is_none()
                            {
                                // 服务进行中，不自动完成，跳过
                            } else if Self::do_complete_order(order_id, &order).is_ok() {
                                processed = processed.saturating_add(1);
                            } else {
                                Self::deposit_event(Event::OrderOperationFailed { order_id, operation: OrderOperation::AutoComplete });
                            }
                        }
                        // 已被手动处理（取消/退款/确认等），跳过
                        _ => {}
                    }
                }
            }

            // C1: 按实际遍历位置截断，保留未遍历的订单
            if iterated >= order_ids.len() {
                // 全部遍历完毕，清空队列
                ExpiryQueue::<T>::remove(now);
            } else {
                // 部分遍历，保留未遍历的条目
                let remaining: BoundedVec<u64, ConstU32<500>> = order_ids
                    .into_iter()
                    .skip(iterated)
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("remaining is subset of original bounded vec");
                if remaining.is_empty() {
                    ExpiryQueue::<T>::remove(now);
                } else {
                    ExpiryQueue::<T>::insert(now, remaining);
                }
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
    }
}
