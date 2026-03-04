//! # NEX/USDT P2P 交易市场模块 (pallet-nex-market)
//!
//! ## 概述
//!
//! 无做市商的 NEX/USDT 订单簿交易市场。任何人可挂单/吃单。
//! - USDT 通道：TRC20 链下支付 + OCW 验证 + 多档判定
//! - 买家保证金：防不付款风险
//! - 三周期 TWAP 预言机：1h / 24h / 7d 防操纵
//! - 熔断机制：价格偏离 7d TWAP 超阈值自动暂停
//!
//! ## 交易对
//!
//! NEX (链上原生代币) ↔ USDT (TRC20, 链下)
//!
//! - 卖 NEX: 卖家锁定 NEX → 买家链下转 USDT → OCW 验证 → 释放 NEX
//! - 买 NEX: 买家挂单 → 卖家接受(锁 NEX) → 买家链下转 USDT → OCW 验证

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{CheckedAdd, CheckedSub, Saturating, Zero};
    use sp_runtime::SaturatedConversion;
    use sp_runtime::transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
    };
    use weights::WeightInfo;

    /// Balance 类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ==================== 数据结构 ====================

    /// 订单方向
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum OrderSide {
        /// 买单（用 USDT 买 NEX）
        Buy,
        /// 卖单（卖 NEX 得 USDT）
        Sell,
    }

    /// 订单状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum OrderStatus {
        /// 挂单中
        Open,
        /// 部分成交
        PartiallyFilled,
        /// 完全成交
        Filled,
        /// 已取消
        Cancelled,
        /// 🆕 H3: 已过期（on_idle GC 自动清理）
        Expired,
    }

    /// USDT 交易状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum UsdtTradeStatus {
        /// 等待买家支付 USDT
        AwaitingPayment,
        /// 等待 OCW 验证
        AwaitingVerification,
        /// 少付等待补付（50%-99.5%，买家可在窗口内补足差额）
        UnderpaidPending,
        /// 已完成
        Completed,
        /// 已退款（超时）
        Refunded,
    }

    /// 买家保证金状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum BuyerDepositStatus {
        /// 无保证金
        #[default]
        None,
        /// 已锁定
        Locked,
        /// 已退还
        Released,
        /// 已没收
        Forfeited,
    }

    // 付款金额验证结果（从 pallet-trading-common 导入）
    pub use pallet_trading_common::PaymentVerificationResult;

    /// TRON 地址类型（34 字节 Base58）
    pub type TronAddress = BoundedVec<u8, ConstU32<34>>;

    /// 🆕 C3: TRON 交易哈希类型（64 hex 字符 = 64 字节 UTF-8，128 安全余量）
    pub type TxHash = BoundedVec<u8, ConstU32<128>>;


    /// 订单
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct Order<T: Config> {
        pub order_id: u64,
        /// 挂单者
        pub maker: T::AccountId,
        /// 方向
        pub side: OrderSide,
        /// NEX 数量
        pub nex_amount: BalanceOf<T>,
        /// 已成交 NEX 数量
        pub filled_amount: BalanceOf<T>,
        /// USDT 单价（每 NEX 的 USDT 价格，精度 10^6）
        pub usdt_price: u64,
        /// 挂单者的 TRON 地址（卖单=卖家收款地址，买单=买家付款地址）
        pub tron_address: Option<TronAddress>,
        /// 状态
        pub status: OrderStatus,
        /// 创建区块
        pub created_at: BlockNumberFor<T>,
        /// 过期区块
        pub expires_at: BlockNumberFor<T>,
        /// 买家预锁定保证金（仅买单，卖单为 Zero）
        pub buyer_deposit: BalanceOf<T>,
        /// 是否免保证金（仅 seed_liquidity 创建的卖单）
        pub deposit_waived: bool,
    }

    /// USDT 交易记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct UsdtTrade<T: Config> {
        pub trade_id: u64,
        pub order_id: u64,
        /// 卖 NEX 方（收 USDT，提供 TRON 地址）
        pub seller: T::AccountId,
        /// 买 NEX 方（付 USDT）
        pub buyer: T::AccountId,
        /// NEX 数量
        pub nex_amount: BalanceOf<T>,
        /// USDT 金额（精度 10^6）
        pub usdt_amount: u64,
        /// 卖家 TRON 地址
        pub seller_tron_address: TronAddress,
        /// 买家 TRON 地址（付款方，用于 OCW 匹配 from/to/amount）
        pub buyer_tron_address: Option<TronAddress>,
        /// 状态
        pub status: UsdtTradeStatus,
        /// 创建区块
        pub created_at: BlockNumberFor<T>,
        /// 超时区块
        pub timeout_at: BlockNumberFor<T>,
        /// 买家保证金
        pub buyer_deposit: BalanceOf<T>,
        /// 保证金状态
        pub deposit_status: BuyerDepositStatus,
        /// 首次 OCW 验证时间（区块号）
        pub first_verified_at: Option<BlockNumberFor<T>>,
        /// 首次检测金额
        pub first_actual_amount: Option<u64>,
        /// 少付补付截止区块
        pub underpaid_deadline: Option<BlockNumberFor<T>>,
    }

    /// 市场统计
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MarketStats {
        pub total_orders: u64,
        pub total_trades: u64,
        pub total_volume_usdt: u64,
    }

    /// 价格快照
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct PriceSnapshot {
        pub cumulative_price: u128,
        pub block_number: u32,
    }

    /// TWAP 累积器
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct TwapAccumulator {
        pub current_cumulative: u128,
        pub current_block: u32,
        /// 最新成交价 (USDT per NEX, 精度 10^6)
        pub last_price: u64,
        pub trade_count: u64,
        pub hour_snapshot: PriceSnapshot,
        pub day_snapshot: PriceSnapshot,
        pub week_snapshot: PriceSnapshot,
        pub last_hour_update: u32,
        pub last_day_update: u32,
        pub last_week_update: u32,
    }

    /// TWAP 周期
    #[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum TwapPeriod {
        OneHour,
        OneDay,
        OneWeek,
    }

    /// 价格保护配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct PriceProtectionConfig {
        pub enabled: bool,
        /// 限价单最大偏离（bps, 2000=20%）
        pub max_price_deviation: u16,
        /// 熔断阈值（bps, 5000=50%）
        pub circuit_breaker_threshold: u16,
        /// 启用 TWAP 的最小成交数
        pub min_trades_for_twap: u64,
        pub circuit_breaker_active: bool,
        pub circuit_breaker_until: u32,
        /// 初始参考价格（USDT per NEX, 精度 10^6）
        pub initial_price: Option<u64>,
    }

    impl Default for PriceProtectionConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                max_price_deviation: 2000,
                circuit_breaker_threshold: 5000,
                min_trades_for_twap: 100,
                circuit_breaker_active: false,
                circuit_breaker_until: 0,
                initial_price: None,
            }
        }
    }

    // ==================== Config ====================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 原生货币（NEX）
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// 权重
        type WeightInfo: WeightInfo;

        /// 默认订单有效期（区块数, 默认 14400 = 24h）
        #[pallet::constant]
        type DefaultOrderTTL: Get<u32>;

        /// 每用户最大活跃订单数
        #[pallet::constant]
        type MaxActiveOrdersPerUser: Get<u32>;

        /// USDT 交易超时（区块数）
        #[pallet::constant]
        type UsdtTimeout: Get<u32>;

        /// 1 小时区块数
        #[pallet::constant]
        type BlocksPerHour: Get<u32>;

        /// 24 小时区块数
        #[pallet::constant]
        type BlocksPerDay: Get<u32>;

        /// 7 天区块数
        #[pallet::constant]
        type BlocksPerWeek: Get<u32>;

        /// 熔断持续时间（区块数）
        #[pallet::constant]
        type CircuitBreakerDuration: Get<u32>;

        /// OCW 验证奖励（NEX）
        #[pallet::constant]
        type VerificationReward: Get<BalanceOf<Self>>;

        /// 奖励来源账户
        type RewardSource: Get<Self::AccountId>;

        /// 买家保证金比例（bps, 1000=10%）
        #[pallet::constant]
        type BuyerDepositRate: Get<u16>;

        /// 最低保证金（NEX）
        #[pallet::constant]
        type MinBuyerDeposit: Get<BalanceOf<Self>>;

        /// 保证金没收比例（bps, 10000=100%）
        #[pallet::constant]
        type DepositForfeitRate: Get<u16>;

        /// USDT→NEX 汇率（精度 10^6, 用于保证金计算）
        /// 例: 10_000_000_000 表示 1 USDT = 10 NEX
        #[pallet::constant]
        type UsdtToNexRate: Get<u64>;

        /// 国库账户
        type TreasuryAccount: Get<Self::AccountId>;

        /// 种子流动性账户（独立于国库，专用于 seed_liquidity 挂单）
        type SeedLiquidityAccount: Get<Self::AccountId>;

        /// 市场管理 Origin（委员会或 Root）
        /// 用于 seed_liquidity / set_initial_price / configure_price_protection / lift_circuit_breaker
        type MarketAdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 免保证金交易超时（区块数，远小于 UsdtTimeout）
        #[pallet::constant]
        type FirstOrderTimeout: Get<u32>;

        /// 免保证金单笔最大 NEX 数量
        #[pallet::constant]
        type MaxFirstOrderAmount: Get<BalanceOf<Self>>;

        /// seed_liquidity 单次最多挂单数
        #[pallet::constant]
        type MaxWaivedSeedOrders: Get<u32>;

        /// seed_liquidity 溢价比例（bps, 2000=20%）：seed 价格 ≥ 基准价 × (1 + premium)
        #[pallet::constant]
        type SeedPricePremiumBps: Get<u16>;

        /// seed_liquidity 每笔订单固定 USDT 金额（精度 10^6，10_000_000 = 10 USDT）
        #[pallet::constant]
        type SeedOrderUsdtAmount: Get<u64>;

        /// 种子账户固定 TRON 收款地址（34 字节 Base58）
        type SeedTronAddress: Get<[u8; 34]>;

        /// AwaitingVerification 超时宽限期（区块数）
        /// OCW 验证可能因 API 延迟而慢，给买家额外时间
        #[pallet::constant]
        type VerificationGracePeriod: Get<u32>;

        /// 少付补付窗口（区块数）
        /// 买家少付 50%-99.5% 时，给予补付时间
        #[pallet::constant]
        type UnderpaidGracePeriod: Get<u32>;

        /// 🆕 H2: OCW 待验证队列最大容量
        #[pallet::constant]
        type MaxPendingTrades: Get<u32>;

        /// 🆕 H2: 待付款跟踪队列最大容量
        #[pallet::constant]
        type MaxAwaitingPaymentTrades: Get<u32>;

        /// 🆕 H2: 少付补付跟踪队列最大容量
        #[pallet::constant]
        type MaxUnderpaidTrades: Get<u32>;

        /// 🆕 H3: on_idle 每次过期订单 GC 最大处理数
        #[pallet::constant]
        type MaxExpiredOrdersPerBlock: Get<u32>;

        /// 🆕 M7: UsedTxHashes 条目保留时间（区块数）
        /// 超过此 TTL 的 tx_hash 在 on_idle 中被清理，防止无限增长
        #[pallet::constant]
        type TxHashTtlBlocks: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== Genesis ====================

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// 初始 NEX/USDT 价格（精度 10^6，例 1 = 0.000001 USDT/NEX）
        /// None = 不预设价格（需链上调用 set_initial_price）
        pub initial_price: Option<u64>,
        #[serde(skip)]
        pub _phantom: core::marker::PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(price) = self.initial_price {
                if price > 0 {
                    PriceProtectionStore::<T>::mutate(|maybe| {
                        let config = maybe.get_or_insert_with(Default::default);
                        config.initial_price = Some(price);
                    });

                    TwapAccumulatorStore::<T>::put(TwapAccumulator {
                        last_price: price,
                        current_block: 0,
                        hour_snapshot: PriceSnapshot { cumulative_price: 0, block_number: 0 },
                        day_snapshot: PriceSnapshot { cumulative_price: 0, block_number: 0 },
                        week_snapshot: PriceSnapshot { cumulative_price: 0, block_number: 0 },
                        last_hour_update: 0,
                        last_day_update: 0,
                        last_week_update: 0,
                        ..Default::default()
                    });

                    LastTradePrice::<T>::put(price);
                }
            }
        }
    }

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let db_weight = T::DbWeight::get();
            let base_weight = db_weight.reads_writes(1, 1);
            if remaining_weight.all_lt(base_weight) {
                return Weight::zero();
            }

            // 仅在有 TWAP 数据时刷新
            let did_work = TwapAccumulatorStore::<T>::mutate(|maybe_acc| {
                let acc = match maybe_acc.as_mut() {
                    Some(a) if a.trade_count > 0 => a,
                    _ => return false,
                };

                let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                let blocks_elapsed = current_block.saturating_sub(acc.current_block);
                if blocks_elapsed == 0 {
                    return false;
                }

                // 用 last_price 填充空白区间的 cumulative
                acc.current_cumulative = acc.current_cumulative
                    .saturating_add((acc.last_price as u128).saturating_mul(blocks_elapsed as u128));
                acc.current_block = current_block;

                // 推进 snapshots
                Self::advance_snapshots(acc, current_block);
                true
            });

            let mut consumed = if did_work { base_weight } else { db_weight.reads(1) };

            // 🆕 H3: 过期订单 GC — 每个 on_idle 清理有限数量的过期订单
            let gc_per_order = db_weight.reads_writes(1, 3); // read order + write order + write order_book + write user_orders
            let max_gc = T::MaxExpiredOrdersPerBlock::get();
            let now = <frame_system::Pallet<T>>::block_number();
            let mut gc_count: u32 = 0;

            // 清理过期卖单
            let sell_ids = SellOrders::<T>::get();
            let mut expired_sell_ids = Vec::new();
            for &id in sell_ids.iter() {
                if gc_count >= max_gc { break; }
                if remaining_weight.all_lt(consumed.saturating_add(gc_per_order)) { break; }
                if let Some(order) = Orders::<T>::get(id) {
                    if now > order.expires_at
                        && (order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled)
                    {
                        // 退还未成交锁定资产
                        let unfilled = order.nex_amount.saturating_sub(order.filled_amount);
                        if !unfilled.is_zero() {
                            T::Currency::unreserve(&order.maker, unfilled);
                        }
                        let mut closed = order;
                        closed.status = OrderStatus::Expired;
                        Orders::<T>::insert(id, &closed);
                        UserOrders::<T>::mutate(&closed.maker, |orders| { orders.retain(|&oid| oid != id); });
                        expired_sell_ids.push(id);
                        gc_count += 1;
                        consumed = consumed.saturating_add(gc_per_order);
                    }
                }
            }
            if !expired_sell_ids.is_empty() {
                SellOrders::<T>::mutate(|orders| {
                    orders.retain(|id| !expired_sell_ids.contains(id));
                });
            }

            // 清理过期买单
            let buy_ids = BuyOrders::<T>::get();
            let mut expired_buy_ids = Vec::new();
            for &id in buy_ids.iter() {
                if gc_count >= max_gc { break; }
                if remaining_weight.all_lt(consumed.saturating_add(gc_per_order)) { break; }
                if let Some(order) = Orders::<T>::get(id) {
                    if now > order.expires_at
                        && (order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled)
                    {
                        if !order.buyer_deposit.is_zero() {
                            T::Currency::unreserve(&order.maker, order.buyer_deposit);
                        }
                        let mut closed = order;
                        closed.status = OrderStatus::Expired;
                        Orders::<T>::insert(id, &closed);
                        UserOrders::<T>::mutate(&closed.maker, |orders| { orders.retain(|&oid| oid != id); });
                        expired_buy_ids.push(id);
                        gc_count += 1;
                        consumed = consumed.saturating_add(gc_per_order);
                    }
                }
            }
            if !expired_buy_ids.is_empty() {
                BuyOrders::<T>::mutate(|orders| {
                    orders.retain(|id| !expired_buy_ids.contains(id));
                });
            }

            // GC 后若有清理，刷新 best prices
            if gc_count > 0 {
                Self::refresh_best_prices();
                consumed = consumed.saturating_add(db_weight.reads_writes(2, 2));
            }

            // 🆕 M7: UsedTxHashes TTL 清理 — cursor-based 有界遍历
            let ttl: BlockNumberFor<T> = T::TxHashTtlBlocks::get().into();
            let gc_per_hash = db_weight.reads_writes(1, 1);
            let max_hash_gc: u32 = 10; // 每区块最多清理 10 条过期 tx_hash
            let mut hash_gc_count: u32 = 0;

            if remaining_weight.all_lt(consumed.saturating_add(gc_per_hash)) {
                return consumed;
            }

            let cursor = TxHashGcCursor::<T>::get();
            let mut iter = if let Some(ref start) = cursor {
                UsedTxHashes::<T>::iter_from(
                    UsedTxHashes::<T>::hashed_key_for(start)
                )
            } else {
                UsedTxHashes::<T>::iter()
            };

            let mut last_key: Option<TxHash> = None;
            let mut exhausted = true;

            while hash_gc_count < max_hash_gc {
                if remaining_weight.all_lt(consumed.saturating_add(gc_per_hash)) {
                    exhausted = false;
                    break;
                }
                match iter.next() {
                    Some((key, (_trade_id, inserted_at))) => {
                        consumed = consumed.saturating_add(gc_per_hash);
                        if now > inserted_at.saturating_add(ttl) {
                            UsedTxHashes::<T>::remove(&key);
                            hash_gc_count += 1;
                        }
                        last_key = Some(key);
                    }
                    None => {
                        exhausted = true;
                        break;
                    }
                }
            }

            if exhausted {
                TxHashGcCursor::<T>::kill();
            } else if let Some(key) = last_key {
                TxHashGcCursor::<T>::put(key);
            }

            consumed
        }

        fn offchain_worker(block_number: BlockNumberFor<T>) {
            // ── 1. 处理 AwaitingVerification 的交易（正常 OCW 验证）──
            let pending = PendingUsdtTrades::<T>::get();
            if !pending.is_empty() {
                log::info!(target: "nex-market-ocw",
                    "Processing {} pending USDT trades at block {:?}", pending.len(), block_number);

                for trade_id in pending.iter() {
                    if let Some(trade) = UsdtTrades::<T>::get(trade_id) {
                        if trade.status == UsdtTradeStatus::AwaitingVerification {
                            if let Some(ref buyer_tron) = trade.buyer_tron_address {
                                Self::process_verification(
                                    *trade_id, &trade,
                                    buyer_tron.as_slice(),
                                    trade.seller_tron_address.as_slice(),
                                );
                            }
                        }
                    }
                }
            }

            // ── 2. 扫描 UnderpaidPending（补付窗口内持续检查新转账）──
            let underpaid = PendingUnderpaidTrades::<T>::get();
            if !underpaid.is_empty() {
                for trade_id in underpaid.iter() {
                    if let Some(trade) = UsdtTrades::<T>::get(trade_id) {
                        if trade.status == UsdtTradeStatus::UnderpaidPending {
                            if let Some(ref buyer_tron) = trade.buyer_tron_address {
                                Self::check_underpaid_topup(
                                    *trade_id, &trade,
                                    buyer_tron.as_slice(),
                                    trade.seller_tron_address.as_slice(),
                                );
                            }
                        }
                    }
                }
            }

            // ── 3. 预检扫描 AwaitingPayment（买家忘记 confirm_payment 的兜底）──
            let awaiting = AwaitingPaymentTrades::<T>::get();
            if awaiting.is_empty() {
                return;
            }

            for trade_id in awaiting.iter() {
                if let Some(trade) = UsdtTrades::<T>::get(trade_id) {
                    if trade.status != UsdtTradeStatus::AwaitingPayment {
                        continue;
                    }
                    // 仅对超过 50% 超时期的交易做预检（避免过早扫描浪费 API 配额）
                    let elapsed = block_number.saturating_sub(trade.created_at);
                    let total = trade.timeout_at.saturating_sub(trade.created_at);
                    if elapsed < total / 2u32.into() {
                        continue;
                    }
                    if let Some(ref buyer_tron) = trade.buyer_tron_address {
                        Self::auto_check_awaiting_payment(
                            *trade_id, &trade,
                            buyer_tron.as_slice(),
                            trade.seller_tron_address.as_slice(),
                        );
                    }
                }
            }
        }
    }

    // ==================== 存储 ====================

    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub type NextOrderId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, u64, Order<T>>;

    /// 卖单索引（卖 NEX 收 USDT）
    #[pallet::storage]
    #[pallet::getter(fn sell_orders)]
    pub type SellOrders<T: Config> = StorageValue<_, BoundedVec<u64, ConstU32<1000>>, ValueQuery>;

    /// 买单索引（买 NEX 付 USDT）
    #[pallet::storage]
    #[pallet::getter(fn buy_orders)]
    pub type BuyOrders<T: Config> = StorageValue<_, BoundedVec<u64, ConstU32<1000>>, ValueQuery>;

    /// 用户订单索引
    #[pallet::storage]
    #[pallet::getter(fn user_orders)]
    pub type UserOrders<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId,
        BoundedVec<u64, ConstU32<100>>, ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn next_usdt_trade_id)]
    pub type NextUsdtTradeId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn usdt_trades)]
    pub type UsdtTrades<T: Config> = StorageMap<_, Blake2_128Concat, u64, UsdtTrade<T>>;

    /// OCW 待验证队列（AwaitingVerification 状态）
    /// 🆕 H2: 容量改为 Config::MaxPendingTrades 可配置
    #[pallet::storage]
    #[pallet::getter(fn pending_usdt_trades)]
    pub type PendingUsdtTrades<T: Config> = StorageValue<_, BoundedVec<u64, T::MaxPendingTrades>, ValueQuery>;

    /// 待付款跟踪队列（OCW 预检扫描 AwaitingPayment 状态）
    /// 🆕 H2: 容量改为 Config::MaxAwaitingPaymentTrades 可配置
    #[pallet::storage]
    #[pallet::getter(fn awaiting_payment_trades)]
    pub type AwaitingPaymentTrades<T: Config> = StorageValue<_, BoundedVec<u64, T::MaxAwaitingPaymentTrades>, ValueQuery>;

    /// 少付补付跟踪队列
    /// 🆕 H2: 容量改为 Config::MaxUnderpaidTrades 可配置
    #[pallet::storage]
    #[pallet::getter(fn pending_underpaid_trades)]
    pub type PendingUnderpaidTrades<T: Config> = StorageValue<_, BoundedVec<u64, T::MaxUnderpaidTrades>, ValueQuery>;

    /// OCW 验证结果
    #[pallet::storage]
    #[pallet::getter(fn ocw_verification_results)]
    pub type OcwVerificationResults<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, (PaymentVerificationResult, u64), OptionQuery,
    >;

    /// 最优卖价 (USDT/NEX)
    #[pallet::storage]
    #[pallet::getter(fn best_ask)]
    pub type BestAsk<T> = StorageValue<_, u64>;

    /// 最优买价
    #[pallet::storage]
    #[pallet::getter(fn best_bid)]
    pub type BestBid<T> = StorageValue<_, u64>;

    /// 最新成交价
    #[pallet::storage]
    #[pallet::getter(fn last_trade_price)]
    pub type LastTradePrice<T> = StorageValue<_, u64>;

    /// 市场统计
    #[pallet::storage]
    #[pallet::getter(fn market_stats)]
    pub type MarketStatsStore<T> = StorageValue<_, MarketStats, ValueQuery>;

    /// TWAP 累积器
    #[pallet::storage]
    #[pallet::getter(fn twap_accumulator)]
    pub type TwapAccumulatorStore<T> = StorageValue<_, TwapAccumulator>;

    /// 价格保护配置
    #[pallet::storage]
    #[pallet::getter(fn price_protection)]
    pub type PriceProtectionStore<T> = StorageValue<_, PriceProtectionConfig>;

    /// 已完成首笔交易的买家（L3 Sybil 防御：完成后不再享受免保证金）
    #[pallet::storage]
    pub type CompletedBuyers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

    /// 🆕 C3: 已使用的 TRON 交易哈希（防重放攻击）
    /// 同一 TRON tx 不能用于多笔 nex-market 交易的支付证明
    /// 🆕 M7: 值改为 (trade_id, inserted_at_block) 用于 TTL 清理
    #[pallet::storage]
    #[pallet::getter(fn used_tx_hashes)]
    pub type UsedTxHashes<T: Config> = StorageMap<_, Blake2_128Concat, TxHash, (u64, BlockNumberFor<T>)>;

    /// 🆕 M7: UsedTxHashes GC 游标（cursor-based 遍历避免全表扫描）
    #[pallet::storage]
    pub type TxHashGcCursor<T: Config> = StorageValue<_, TxHash>;

    /// 买家当前活跃的免保证金交易数（L2 防 grief：每账户最多 1 笔）
    #[pallet::storage]
    pub type ActiveWaivedTrades<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    /// seed_liquidity 累计成交 USDT 总额（审计用，链上 vs TRON 多签钱包对账）
    #[pallet::storage]
    #[pallet::getter(fn cumulative_seed_usdt_sold)]
    pub type CumulativeSeedUsdtSold<T> = StorageValue<_, u64, ValueQuery>;

    // ==================== Events ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 订单已创建
        OrderCreated {
            order_id: u64,
            maker: T::AccountId,
            side: OrderSide,
            nex_amount: BalanceOf<T>,
            usdt_price: u64,
        },
        /// 订单已取消
        OrderCancelled { order_id: u64 },
        /// USDT 交易已创建
        UsdtTradeCreated {
            trade_id: u64,
            order_id: u64,
            seller: T::AccountId,
            buyer: T::AccountId,
            nex_amount: BalanceOf<T>,
            usdt_amount: u64,
        },
        /// USDT 支付已确认（买家声明已付款）
        UsdtPaymentSubmitted {
            trade_id: u64,
        },
        /// USDT 交易已完成
        UsdtTradeCompleted {
            trade_id: u64,
            order_id: u64,
        },
        /// 验证失败
        UsdtTradeVerificationFailed {
            trade_id: u64,
            reason: Vec<u8>,
        },
        /// 超时退款（AwaitingPayment 阶段超时）
        UsdtTradeRefunded { trade_id: u64 },
        /// 验证超时退款（AwaitingVerification 阶段超时，宽限期后仍无结果）
        VerificationTimeoutRefunded {
            trade_id: u64,
            buyer: T::AccountId,
            seller: T::AccountId,
            usdt_amount: u64,
        },
        /// TWAP 已更新
        TwapUpdated {
            new_price: u64,
            twap_1h: Option<u64>,
            twap_24h: Option<u64>,
            twap_7d: Option<u64>,
        },
        /// 熔断触发
        CircuitBreakerTriggered {
            current_price: u64,
            twap_7d: u64,
            deviation_bps: u16,
            until_block: u32,
        },
        /// 熔断解除
        CircuitBreakerLifted,
        /// 价格保护已配置
        PriceProtectionConfigured {
            enabled: bool,
            max_deviation: u16,
        },
        /// 初始价格已设置
        InitialPriceSet { initial_price: u64 },
        /// OCW 验证结果已提交
        OcwResultSubmitted {
            trade_id: u64,
            verification_result: PaymentVerificationResult,
            actual_amount: u64,
        },
        /// 少付自动处理
        UnderpaidAutoProcessed {
            trade_id: u64,
            expected_amount: u64,
            actual_amount: u64,
            payment_ratio: u32,
            nex_released: BalanceOf<T>,
            deposit_forfeited: BalanceOf<T>,
        },
        /// 保证金已锁定
        BuyerDepositLocked {
            trade_id: u64,
            buyer: T::AccountId,
            deposit: BalanceOf<T>,
        },
        /// 保证金已退还
        BuyerDepositReleased {
            trade_id: u64,
            buyer: T::AccountId,
            deposit: BalanceOf<T>,
        },
        /// 保证金已没收
        BuyerDepositForfeited {
            trade_id: u64,
            buyer: T::AccountId,
            forfeited: BalanceOf<T>,
            to_treasury: BalanceOf<T>,
        },
        /// 验证奖励已领取
        VerificationRewardClaimed {
            trade_id: u64,
            claimer: T::AccountId,
            reward: BalanceOf<T>,
            reward_paid: bool,
        },
        /// 流动性种子已注入（seed_liquidity）
        LiquiditySeeded {
            order_count: u32,
            total_nex: BalanceOf<T>,
            source: T::AccountId,
        },
        /// 种子账户已注资（国库 → 种子账户）
        SeedAccountFunded {
            amount: BalanceOf<T>,
            treasury: T::AccountId,
            seed_account: T::AccountId,
        },
        /// 免保证金交易已创建
        WaivedDepositTradeCreated {
            trade_id: u64,
            buyer: T::AccountId,
            nex_amount: BalanceOf<T>,
        },
        /// OCW 自动检测到 USDT 到账（买家未手动确认）
        AutoPaymentDetected {
            trade_id: u64,
            actual_amount: u64,
        },
        /// 少付检测到，进入补付窗口
        UnderpaidDetected {
            trade_id: u64,
            expected_amount: u64,
            actual_amount: u64,
            payment_ratio: u32,
            deadline: BlockNumberFor<T>,
        },
        /// 补付窗口内金额已更新
        UnderpaidAmountUpdated {
            trade_id: u64,
            previous_amount: u64,
            new_amount: u64,
        },
        /// 少付终裁完成
        UnderpaidFinalized {
            trade_id: u64,
            final_amount: u64,
            payment_ratio: u32,
            deposit_forfeit_rate: u16,
        },
    }

    // ==================== Errors ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 订单不存在
        OrderNotFound,
        /// 不是订单所有者
        NotOrderOwner,
        /// 订单已关闭
        OrderClosed,
        /// 余额不足
        InsufficientBalance,
        /// 数量过小
        AmountTooSmall,
        /// 价格为零
        ZeroPrice,
        /// 订单簿已满
        OrderBookFull,
        /// 用户订单数已满
        UserOrdersFull,
        /// 不能吃自己的单
        CannotTakeOwnOrder,
        /// 算术溢出
        ArithmeticOverflow,
        /// 订单方向不匹配
        OrderSideMismatch,
        /// TRON 地址无效
        InvalidTronAddress,
        /// USDT 交易不存在
        UsdtTradeNotFound,
        /// 不是交易参与者
        NotTradeParticipant,
        /// 交易状态无效
        InvalidTradeStatus,
        /// 交易已超时
        TradeTimeout,
        /// 待验证队列已满
        PendingQueueFull,
        /// 价格偏离过大
        PriceDeviationTooHigh,
        /// 市场熔断中
        MarketCircuitBreakerActive,
        /// OCW 验证结果不存在
        OcwResultNotFound,
        /// 保证金余额不足
        InsufficientDepositBalance,
        /// 基点参数无效
        InvalidBasisPoints,
        /// 免保证金首单超限（每账户仅 1 笔活跃）
        FirstOrderLimitReached,
        /// 首单金额超过上限
        FirstOrderAmountTooLarge,
        /// 该买家已完成过交易，不再享受免保证金
        BuyerAlreadyCompleted,
        /// seed_liquidity 订单数超限
        TooManySeedOrders,
        /// 无可用的基准价格（需先 set_initial_price）
        NoPriceReference,
        /// 仍在验证宽限期内，不允许超时
        StillInGracePeriod,
        /// 补付窗口尚未到期
        UnderpaidGraceNotExpired,
        /// 交易不在 UnderpaidPending 状态
        NotUnderpaidPending,
        /// 订单已过期
        OrderExpired,
        /// 熔断未激活（无需解除）
        CircuitBreakerNotActive,
        /// 🆕 L2修复: 熔断持续时间未到期
        CircuitBreakerNotExpired,
        /// 待付款跟踪队列已满
        AwaitingPaymentQueueFull,
        /// 少付跟踪队列已满
        UnderpaidQueueFull,
        /// 🆕 C3: TRON 交易哈希已被使用（防重放）
        TxHashAlreadyUsed,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 挂卖单（卖 NEX 收 USDT）
        ///
        /// 卖家锁定 NEX，提供 TRON 地址接收 USDT
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::place_sell_order())]
        pub fn place_sell_order(
            origin: OriginFor<T>,
            nex_amount: BalanceOf<T>,
            usdt_price: u64,
            tron_address: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(usdt_price > 0, Error::<T>::ZeroPrice);
            ensure!(!nex_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 价格偏离检查
            Self::check_price_deviation(usdt_price)?;

            // 验证 TRON 地址（Base58Check 完整校验）
            ensure!(pallet_trading_common::is_valid_tron_address(&tron_address), Error::<T>::InvalidTronAddress);
            let tron_addr: TronAddress = tron_address.try_into().map_err(|_| Error::<T>::InvalidTronAddress)?;

            // 锁定 NEX
            T::Currency::reserve(&who, nex_amount)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            // 创建订单
            let order_id = Self::do_create_order(
                who.clone(), OrderSide::Sell, nex_amount, usdt_price, Some(tron_addr), Zero::zero(), false,
            )?;

            Self::update_best_price_on_new_order(usdt_price, OrderSide::Sell);

            Self::deposit_event(Event::OrderCreated {
                order_id, maker: who, side: OrderSide::Sell, nex_amount, usdt_price,
            });

            Ok(())
        }

        /// 挂买单（买 NEX 付 USDT）
        ///
        /// 买家声明愿意用 USDT 购买 NEX，无需锁定链上资产
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::place_buy_order())]
        pub fn place_buy_order(
            origin: OriginFor<T>,
            nex_amount: BalanceOf<T>,
            usdt_price: u64,
            buyer_tron_address: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(usdt_price > 0, Error::<T>::ZeroPrice);
            ensure!(!nex_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 验证买家 TRON 地址（Base58Check 完整校验）
            ensure!(pallet_trading_common::is_valid_tron_address(&buyer_tron_address), Error::<T>::InvalidTronAddress);
            let buyer_tron: TronAddress = buyer_tron_address.try_into().map_err(|_| Error::<T>::InvalidTronAddress)?;

            Self::check_price_deviation(usdt_price)?;

            // 计算并预锁定买家保证金
            let nex_u128: u128 = nex_amount.saturated_into();
            let usdt_total = nex_u128
                .checked_mul(usdt_price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(1_000_000_000_000u128)
                .ok_or(Error::<T>::ArithmeticOverflow)? as u64;
            let buyer_deposit = Self::calculate_buyer_deposit(usdt_total);
            if !buyer_deposit.is_zero() {
                T::Currency::reserve(&who, buyer_deposit)
                    .map_err(|_| Error::<T>::InsufficientDepositBalance)?;
            }

            let order_id = Self::do_create_order(
                who.clone(), OrderSide::Buy, nex_amount, usdt_price, Some(buyer_tron), buyer_deposit, false,
            )?;

            Self::update_best_price_on_new_order(usdt_price, OrderSide::Buy);

            Self::deposit_event(Event::OrderCreated {
                order_id, maker: who, side: OrderSide::Buy, nex_amount, usdt_price,
            });

            Ok(())
        }

        /// 取消订单
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::cancel_order())]
        pub fn cancel_order(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.maker == who, Error::<T>::NotOrderOwner);
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );

            // 退还未成交的锁定资产
            if order.side == OrderSide::Sell {
                let unfilled = order.nex_amount.saturating_sub(order.filled_amount);
                if !unfilled.is_zero() {
                    T::Currency::unreserve(&who, unfilled);
                }
            } else if order.side == OrderSide::Buy {
                // 退还剩余的预锁定保证金
                if !order.buyer_deposit.is_zero() {
                    T::Currency::unreserve(&who, order.buyer_deposit);
                }
            }

            order.status = OrderStatus::Cancelled;
            Orders::<T>::insert(order_id, &order);

            Self::remove_from_order_book(order_id, order.side);
            UserOrders::<T>::mutate(&who, |orders| { orders.retain(|&id| id != order_id); });
            Self::update_best_price_on_remove(order.usdt_price, order.side);

            Self::deposit_event(Event::OrderCancelled { order_id });

            Ok(())
        }

        /// 预锁定卖单（买家吃卖单）
        ///
        /// 买家看到卖单后调用：锁定保证金 → 创建 USDT 交易
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::reserve_sell_order())]
        pub fn reserve_sell_order(
            origin: OriginFor<T>,
            order_id: u64,
            amount: Option<BalanceOf<T>>,
            buyer_tron_address: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 🆕 H1修复: 验证买家 TRON 地址（Base58Check 完整校验，与其他 extrinsic 一致）
            ensure!(pallet_trading_common::is_valid_tron_address(&buyer_tron_address), Error::<T>::InvalidTronAddress);
            let buyer_tron: TronAddress = buyer_tron_address.try_into().map_err(|_| Error::<T>::InvalidTronAddress)?;

            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.side == OrderSide::Sell, Error::<T>::OrderSideMismatch);
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );
            ensure!(order.maker != who, Error::<T>::CannotTakeOwnOrder);

            // M1: 订单过期检查
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= order.expires_at, Error::<T>::OrderExpired);

            // M4: 吃单时也检查价格偏离 + 熔断
            Self::check_price_deviation(order.usdt_price)?;

            let available = order.nex_amount.checked_sub(&order.filled_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            let fill_amount = amount.unwrap_or(available).min(available);
            ensure!(!fill_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 计算 USDT 金额
            let nex_u128: u128 = fill_amount.saturated_into();
            let usdt_amount = nex_u128
                .checked_mul(order.usdt_price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(1_000_000_000_000u128) // NEX 精度 10^12, USDT 精度 10^6
                .ok_or(Error::<T>::ArithmeticOverflow)? as u64;
            ensure!(usdt_amount > 0, Error::<T>::AmountTooSmall);

            let seller_tron_address = order.tron_address.clone()
                .ok_or(Error::<T>::InvalidTronAddress)?;

            // 保证金逻辑：免保证金卖单 vs 普通卖单
            let (buyer_deposit, is_waived_trade) = if order.deposit_waived {
                // L3: 已完成过交易的买家不再享受免保证金
                ensure!(!CompletedBuyers::<T>::get(&who), Error::<T>::BuyerAlreadyCompleted);
                // L2: 每账户最多 1 笔活跃免保证金交易
                ensure!(ActiveWaivedTrades::<T>::get(&who) == 0, Error::<T>::FirstOrderLimitReached);
                // L2: 单笔上限
                ensure!(fill_amount <= T::MaxFirstOrderAmount::get(), Error::<T>::FirstOrderAmountTooLarge);
                (Zero::zero(), true)
            } else {
                let deposit = Self::calculate_buyer_deposit(usdt_amount);
                if !deposit.is_zero() {
                    T::Currency::reserve(&who, deposit)
                        .map_err(|_| Error::<T>::InsufficientDepositBalance)?;
                }
                (deposit, false)
            };

            let trade_id = Self::do_create_usdt_trade_ex(
                order_id, order.maker.clone(), who.clone(),
                fill_amount, usdt_amount, seller_tron_address, Some(buyer_tron), buyer_deposit, is_waived_trade,
            )?;

            // L2: 记录活跃免保证金交易
            if is_waived_trade {
                ActiveWaivedTrades::<T>::mutate(&who, |count| *count = count.saturating_add(1));
            }

            // 更新订单
            order.filled_amount = order.filled_amount.checked_add(&fill_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            if order.filled_amount >= order.nex_amount {
                order.status = OrderStatus::Filled;
                Self::remove_from_order_book(order_id, OrderSide::Sell);
                UserOrders::<T>::mutate(&order.maker, |orders| { orders.retain(|&id| id != order_id); });
            } else {
                order.status = OrderStatus::PartiallyFilled;
            }
            Orders::<T>::insert(order_id, &order);

            Self::deposit_event(Event::UsdtTradeCreated {
                trade_id, order_id, seller: order.maker.clone(), buyer: who.clone(),
                nex_amount: fill_amount, usdt_amount,
            });

            if !buyer_deposit.is_zero() {
                Self::deposit_event(Event::BuyerDepositLocked {
                    trade_id, buyer: who, deposit: buyer_deposit,
                });
            }

            Ok(())
        }

        /// 接受买单（卖家接受买单）
        ///
        /// 卖家看到买单后调用：锁定 NEX + 锁定买家保证金 → 创建 USDT 交易
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::accept_buy_order())]
        pub fn accept_buy_order(
            origin: OriginFor<T>,
            order_id: u64,
            amount: Option<BalanceOf<T>>,
            tron_address: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            let buyer = order.maker.clone();
            ensure!(order.side == OrderSide::Buy, Error::<T>::OrderSideMismatch);
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );
            ensure!(buyer != who, Error::<T>::CannotTakeOwnOrder);

            // M1: 订单过期检查
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= order.expires_at, Error::<T>::OrderExpired);

            // M4: 吃单时也检查价格偏离 + 熔断
            Self::check_price_deviation(order.usdt_price)?;

            // 验证 TRON 地址（Base58Check 完整校验）
            ensure!(pallet_trading_common::is_valid_tron_address(&tron_address), Error::<T>::InvalidTronAddress);
            let tron_addr: TronAddress = tron_address.try_into().map_err(|_| Error::<T>::InvalidTronAddress)?;

            let available = order.nex_amount.checked_sub(&order.filled_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            let fill_amount = amount.unwrap_or(available).min(available);
            ensure!(!fill_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 计算 USDT
            let nex_u128: u128 = fill_amount.saturated_into();
            let usdt_amount = nex_u128
                .checked_mul(order.usdt_price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(1_000_000_000_000u128)
                .ok_or(Error::<T>::ArithmeticOverflow)? as u64;
            ensure!(usdt_amount > 0, Error::<T>::AmountTooSmall);

            // 从预锁定保证金中按比例分配给本次交易（已在 place_buy_order 时 reserve）
            let trade_deposit = if !order.buyer_deposit.is_zero() && !available.is_zero() {
                let deposit_u128: u128 = order.buyer_deposit.saturated_into();
                let fill_u128: u128 = fill_amount.saturated_into();
                let avail_u128: u128 = available.saturated_into();
                let result = deposit_u128.saturating_mul(fill_u128).saturating_div(avail_u128);
                let trade_dep: BalanceOf<T> = result.saturated_into();
                trade_dep
            } else {
                Zero::zero()
            };
            order.buyer_deposit = order.buyer_deposit.saturating_sub(trade_deposit);

            // 锁定卖家 NEX
            T::Currency::reserve(&who, fill_amount)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            // 从买单中取出买家 TRON 地址
            let buyer_tron = order.tron_address.clone();

            let trade_id = Self::do_create_usdt_trade(
                order_id, who.clone(), buyer.clone(),
                fill_amount, usdt_amount, tron_addr, buyer_tron, trade_deposit,
            )?;

            // 更新订单
            order.filled_amount = order.filled_amount.checked_add(&fill_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            if order.filled_amount >= order.nex_amount {
                order.status = OrderStatus::Filled;
                Self::remove_from_order_book(order_id, OrderSide::Buy);
                UserOrders::<T>::mutate(&buyer, |orders| { orders.retain(|&id| id != order_id); });
            } else {
                order.status = OrderStatus::PartiallyFilled;
            }
            Orders::<T>::insert(order_id, &order);

            Self::deposit_event(Event::UsdtTradeCreated {
                trade_id, order_id, seller: who, buyer: buyer.clone(),
                nex_amount: fill_amount, usdt_amount,
            });

            if !trade_deposit.is_zero() {
                Self::deposit_event(Event::BuyerDepositLocked {
                    trade_id, buyer, deposit: trade_deposit,
                });
            }

            Ok(())
        }

        /// 买家确认 USDT 支付
        ///
        /// 买家声明已向卖家 TRON 地址转账，OCW 将按 (from, to, amount) 验证。
        /// buyer_tron_address 已在挂单/预锁定阶段提供，此处仅触发验证流程。
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::confirm_payment())]
        pub fn confirm_payment(
            origin: OriginFor<T>,
            trade_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(trade.buyer == who, Error::<T>::NotTradeParticipant);
            ensure!(trade.status == UsdtTradeStatus::AwaitingPayment, Error::<T>::InvalidTradeStatus);

            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= trade.timeout_at, Error::<T>::TradeTimeout);

            // buyer_tron_address 必须已在上游设置（reserve_sell_order 或 place_buy_order）
            ensure!(trade.buyer_tron_address.is_some(), Error::<T>::InvalidTronAddress);

            trade.status = UsdtTradeStatus::AwaitingVerification;
            UsdtTrades::<T>::insert(trade_id, &trade);

            // 从待付款队列移到待验证队列
            AwaitingPaymentTrades::<T>::mutate(|list| { list.retain(|&id| id != trade_id); });
            PendingUsdtTrades::<T>::try_mutate(|pending| {
                pending.try_push(trade_id).map_err(|_| Error::<T>::PendingQueueFull)
            })?;

            Self::deposit_event(Event::UsdtPaymentSubmitted { trade_id });

            Ok(())
        }

        /// 处理超时 USDT 交易
        ///
        /// ## 分阶段超时策略
        /// - **AwaitingPayment**: 买家未付款 → 直接超时，没收保证金
        /// - **AwaitingVerification**: 买家已声明付款 → 需等宽限期结束
        ///   - 若 OCW 已提交验证结果 → 按正常流程结算（不走超时）
        ///   - 若宽限期后仍无结果 → 退款 + 没收保证金 + 发出事件供链下仲裁
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::process_timeout())]
        pub fn process_timeout(
            origin: OriginFor<T>,
            trade_id: u64,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(
                trade.status == UsdtTradeStatus::AwaitingPayment ||
                trade.status == UsdtTradeStatus::AwaitingVerification ||
                trade.status == UsdtTradeStatus::UnderpaidPending,
                Error::<T>::InvalidTradeStatus
            );

            let now = <frame_system::Pallet<T>>::block_number();

            // ── UnderpaidPending: 补付窗口到期后按最终金额终裁 ──
            if trade.status == UsdtTradeStatus::UnderpaidPending {
                let deadline = trade.underpaid_deadline.ok_or(Error::<T>::InvalidTradeStatus)?;
                ensure!(now > deadline, Error::<T>::UnderpaidGraceNotExpired);

                if let Some((_, final_amount)) = OcwVerificationResults::<T>::get(trade_id) {
                    let final_result = Self::calculate_payment_verification_result(
                        trade.usdt_amount, final_amount,
                    );
                    match final_result {
                        PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid => {
                            Self::process_full_payment(&mut trade, trade_id)?;
                        }
                        _ => {
                            Self::process_underpaid(&mut trade, trade_id, final_amount)?;
                        }
                    }
                    let payment_ratio = pallet_trading_common::compute_payment_ratio_bps(trade.usdt_amount, final_amount);
                    let deposit_forfeit_rate = Self::calculate_deposit_forfeit_rate(payment_ratio);

                    PendingUnderpaidTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
                    OcwVerificationResults::<T>::remove(trade_id);

                    Self::deposit_event(Event::UnderpaidFinalized {
                        trade_id, final_amount, payment_ratio, deposit_forfeit_rate,
                    });
                    return Ok(());
                }
                // 无 OCW 结果 → 走通用超时退款
            }

            ensure!(now > trade.timeout_at, Error::<T>::InvalidTradeStatus);

            // ── AwaitingVerification: 宽限期 + 已有结果检查 ──
            if trade.status == UsdtTradeStatus::AwaitingVerification {
                let grace: BlockNumberFor<T> = T::VerificationGracePeriod::get().into();
                let deadline_with_grace = trade.timeout_at.saturating_add(grace);

                // 宽限期未结束 → 拒绝超时
                ensure!(now > deadline_with_grace, Error::<T>::StillInGracePeriod);

                // 宽限期已过，但 OCW 已提交了验证结果 → 按正常流程结算
                if let Some((verification_result, actual_amount)) = OcwVerificationResults::<T>::get(trade_id) {
                    match verification_result {
                        PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid => {
                            Self::process_full_payment(&mut trade, trade_id)?;
                        }
                        PaymentVerificationResult::Underpaid |
                        PaymentVerificationResult::SeverelyUnderpaid => {
                            Self::process_underpaid(&mut trade, trade_id, actual_amount)?;
                        }
                        PaymentVerificationResult::Invalid => {
                            Self::process_underpaid(&mut trade, trade_id, 0)?;
                        }
                    }
                    PendingUsdtTrades::<T>::mutate(|pending| { pending.retain(|&id| id != trade_id); });
                    OcwVerificationResults::<T>::remove(trade_id);
                    return Ok(());
                }
            }

            // ── 执行超时退款（AwaitingPayment 或 AwaitingVerification 无结果）──

            // 退还锁定的 NEX 给卖家
            T::Currency::unreserve(&trade.seller, trade.nex_amount);

            // 回滚父订单
            Self::rollback_order_filled_amount(trade.order_id, trade.nex_amount);

            // 没收买家保证金
            Self::forfeit_buyer_deposit(&mut trade, trade_id);

            let is_verification_timeout = trade.status == UsdtTradeStatus::AwaitingVerification;
            let buyer_clone = trade.buyer.clone();
            let seller_clone = trade.seller.clone();
            let usdt_amount = trade.usdt_amount;

            trade.status = UsdtTradeStatus::Refunded;
            UsdtTrades::<T>::insert(trade_id, &trade);

            // 清理活跃免保证金计数
            Self::cleanup_waived_trade_counter(&trade.buyer, trade.order_id);

            // 清理所有队列
            AwaitingPaymentTrades::<T>::mutate(|list| { list.retain(|&id| id != trade_id); });
            PendingUsdtTrades::<T>::mutate(|pending| { pending.retain(|&id| id != trade_id); });
            PendingUnderpaidTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
            OcwVerificationResults::<T>::remove(trade_id);

            if is_verification_timeout {
                // AwaitingVerification 超时 — 发出专用事件，便于链下仲裁
                Self::deposit_event(Event::VerificationTimeoutRefunded {
                    trade_id,
                    buyer: buyer_clone,
                    seller: seller_clone,
                    usdt_amount,
                });
            } else {
                Self::deposit_event(Event::UsdtTradeRefunded { trade_id });
            }

            Ok(())
        }

        /// OCW 提交验证结果
        ///
        /// - Exact/Overpaid: 存储结果，等待 claim_verification_reward
        /// - Underpaid (50%-99.5%): 进入 UnderpaidPending 补付窗口
        /// - SeverelyUnderpaid (<50%) / Invalid: 直接存储结果，终裁
        ///
        /// 🆕 C3: tx_hash 用于防重放。同一 TRON 交易不能验证多笔订单。
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::submit_ocw_result())]
        pub fn submit_ocw_result(
            origin: OriginFor<T>,
            trade_id: u64,
            actual_amount: u64,
            tx_hash: Option<TxHash>,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(trade.status == UsdtTradeStatus::AwaitingVerification, Error::<T>::InvalidTradeStatus);

            // 🆕 C3: 检查 tx_hash 是否已被使用
            if let Some(ref hash) = tx_hash {
                ensure!(!UsedTxHashes::<T>::contains_key(hash), Error::<T>::TxHashAlreadyUsed);
            }

            let verification_result = Self::calculate_payment_verification_result(
                trade.usdt_amount, actual_amount,
            );

            match verification_result {
                PaymentVerificationResult::Underpaid => {
                    // 50%-99.5%: 进入补付窗口
                    let now = <frame_system::Pallet<T>>::block_number();
                    let grace: BlockNumberFor<T> = T::UnderpaidGracePeriod::get().into();
                    let deadline = now.saturating_add(grace);

                    trade.status = UsdtTradeStatus::UnderpaidPending;
                    trade.first_verified_at = Some(now);
                    trade.first_actual_amount = Some(actual_amount);
                    trade.underpaid_deadline = Some(deadline);
                    UsdtTrades::<T>::insert(trade_id, &trade);

                    // 从 PendingUsdtTrades 移到 PendingUnderpaidTrades
                    PendingUsdtTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
                    PendingUnderpaidTrades::<T>::try_mutate(|p| {
                        p.try_push(trade_id).map_err(|_| Error::<T>::UnderpaidQueueFull)
                    })?;

                    // 同时存储当前结果（claim 可在补付窗口到期后使用最新值）
                    OcwVerificationResults::<T>::insert(trade_id, (verification_result, actual_amount));

                    let payment_ratio = pallet_trading_common::compute_payment_ratio_bps(trade.usdt_amount, actual_amount);
                    let deposit_forfeit_rate = Self::calculate_deposit_forfeit_rate(payment_ratio);
                    Self::deposit_event(Event::UnderpaidDetected {
                        trade_id,
                        expected_amount: trade.usdt_amount,
                        actual_amount,
                        payment_ratio,
                        deadline,
                    });
                }
                _ => {
                    // Exact/Overpaid/SeverelyUnderpaid/Invalid: 直接存储结果
                    OcwVerificationResults::<T>::insert(trade_id, (verification_result, actual_amount));

                    Self::deposit_event(Event::OcwResultSubmitted {
                        trade_id, verification_result, actual_amount,
                    });
                }
            }

            // 🆕 C3: 记录已使用的 tx_hash（防重放）
            // 🆕 M7: 同时记录插入区块号，用于 TTL 清理
            if let Some(hash) = tx_hash {
                let now = <frame_system::Pallet<T>>::block_number();
                UsedTxHashes::<T>::insert(hash, (trade_id, now));
            }

            Ok(())
        }

        /// 领取验证奖励（任何人）
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::claim_reward())]
        pub fn claim_verification_reward(
            origin: OriginFor<T>,
            trade_id: u64,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::do_claim_verification_reward(&caller, trade_id)
        }

        /// 配置价格保护（Root）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::configure_price_protection())]
        pub fn configure_price_protection(
            origin: OriginFor<T>,
            enabled: bool,
            max_price_deviation: u16,
            circuit_breaker_threshold: u16,
            min_trades_for_twap: u64,
        ) -> DispatchResult {
            T::MarketAdminOrigin::ensure_origin(origin)?;

            ensure!(max_price_deviation <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(circuit_breaker_threshold <= 10000, Error::<T>::InvalidBasisPoints);

            let mut config = PriceProtectionStore::<T>::get().unwrap_or_default();
            config.enabled = enabled;
            config.max_price_deviation = max_price_deviation;
            config.circuit_breaker_threshold = circuit_breaker_threshold;
            config.min_trades_for_twap = min_trades_for_twap;
            PriceProtectionStore::<T>::put(config);

            Self::deposit_event(Event::PriceProtectionConfigured {
                enabled, max_deviation: max_price_deviation,
            });

            Ok(())
        }

        /// 设置初始价格（Root, TWAP 冷启动）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::set_initial_price())]
        pub fn set_initial_price(
            origin: OriginFor<T>,
            initial_price: u64,
        ) -> DispatchResult {
            T::MarketAdminOrigin::ensure_origin(origin)?;
            ensure!(initial_price > 0, Error::<T>::ZeroPrice);

            PriceProtectionStore::<T>::mutate(|maybe| {
                let config = maybe.get_or_insert_with(Default::default);
                config.initial_price = Some(initial_price);
            });

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
            if TwapAccumulatorStore::<T>::get().is_none() {
                TwapAccumulatorStore::<T>::put(TwapAccumulator {
                    last_price: initial_price,
                    current_block,
                    hour_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    day_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    week_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    last_hour_update: current_block,
                    last_day_update: current_block,
                    last_week_update: current_block,
                    ..Default::default()
                });
            }

            LastTradePrice::<T>::put(initial_price);

            Self::deposit_event(Event::InitialPriceSet { initial_price });

            Ok(())
        }

        /// 解除熔断（Root）
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::lift_circuit_breaker())]
        pub fn lift_circuit_breaker(origin: OriginFor<T>) -> DispatchResult {
            T::MarketAdminOrigin::ensure_origin(origin)?;

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
            let config = PriceProtectionStore::<T>::get().unwrap_or_default();
            ensure!(config.circuit_breaker_active, Error::<T>::CircuitBreakerNotActive);
            // 🆕 L2修复: 使用语义正确的错误名（原 InvalidTradeStatus 不匹配）
            ensure!(current_block >= config.circuit_breaker_until, Error::<T>::CircuitBreakerNotExpired);

            PriceProtectionStore::<T>::mutate(|maybe| {
                if let Some(c) = maybe {
                    c.circuit_breaker_active = false;
                    c.circuit_breaker_until = 0;
                }
            });

            Self::deposit_event(Event::CircuitBreakerLifted);

            Ok(())
        }

        /// 委员会批准：国库 → 种子账户注资（委员会审批）
        ///
        /// 用于补充 seed_liquidity 所需的 NEX 资金。
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::fund_seed_account())]
        pub fn fund_seed_account(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            T::MarketAdminOrigin::ensure_origin(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::AmountTooSmall);

            let treasury = T::TreasuryAccount::get();
            let seed = T::SeedLiquidityAccount::get();

            T::Currency::transfer(&treasury, &seed, amount, ExistenceRequirement::KeepAlive)?;

            Self::deposit_event(Event::SeedAccountFunded {
                amount,
                treasury,
                seed_account: seed,
            });

            Ok(())
        }

        /// 种子流动性挂单（委员会/Root）
        ///
        /// 批量挂免保证金卖单，为新用户提供 NEX 购买入口。
        /// 使用 Config 中固定的 SeedTronAddress 作为收款地址。
        /// 每笔订单 USDT 金额默认 SeedOrderUsdtAmount，可通过 usdt_override 临时覆盖。
        /// NEX 数量和价格自动计算：
        ///   seed_price = ref_price × (1 + premium)
        ///   nex_amount = usdt_amount × 10^12 / seed_price
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::seed_liquidity())]
        pub fn seed_liquidity(
            origin: OriginFor<T>,
            order_count: u32,
            usdt_override: Option<u64>,
        ) -> DispatchResult {
            T::MarketAdminOrigin::ensure_origin(origin)?;

            ensure!(order_count > 0, Error::<T>::AmountTooSmall);
            ensure!(order_count <= T::MaxWaivedSeedOrders::get(), Error::<T>::TooManySeedOrders);

            let seed_account = T::SeedLiquidityAccount::get();
            let tron_addr: TronAddress = T::SeedTronAddress::get().to_vec()
                .try_into().map_err(|_| Error::<T>::InvalidTronAddress)?;

            // 瀑布式基准价 + 溢价
            let ref_price = Self::get_seed_reference_price()
                .ok_or(Error::<T>::NoPriceReference)?;
            let premium = T::SeedPricePremiumBps::get() as u64;
            let seed_price = ref_price
                .saturating_mul(10000u64.saturating_add(premium))
                / 10000u64;
            ensure!(seed_price > 0, Error::<T>::ZeroPrice);

            // 计算每笔 NEX 数量：usdt_amount × 10^12 / seed_price
            let usdt_amount = usdt_override.unwrap_or_else(|| T::SeedOrderUsdtAmount::get());
            ensure!(usdt_amount > 0, Error::<T>::AmountTooSmall);
            let nex_per_order: BalanceOf<T> = ((usdt_amount as u128)
                .saturating_mul(1_000_000_000_000u128) // NEX 精度 10^12
                / (seed_price as u128))
                .try_into()
                .map_err(|_| Error::<T>::AmountTooSmall)?;
            ensure!(!nex_per_order.is_zero(), Error::<T>::AmountTooSmall);

            let mut total_nex: BalanceOf<T> = Zero::zero();

            for _ in 0..order_count {
                T::Currency::reserve(&seed_account, nex_per_order)
                    .map_err(|_| Error::<T>::InsufficientBalance)?;

                let order_id = Self::do_create_order(
                    seed_account.clone(), OrderSide::Sell, nex_per_order, seed_price,
                    Some(tron_addr.clone()), Zero::zero(), true,
                )?;

                total_nex = total_nex.saturating_add(nex_per_order);

                Self::deposit_event(Event::OrderCreated {
                    order_id, maker: seed_account.clone(), side: OrderSide::Sell,
                    nex_amount: nex_per_order, usdt_price: seed_price,
                });
            }

            Self::refresh_best_prices();

            Self::deposit_event(Event::LiquiditySeeded {
                order_count, total_nex, source: seed_account,
            });

            Ok(())
        }

        /// OCW 自动确认付款 + 提交验证结果（unsigned）
        ///
        /// 当 OCW 预检扫描发现 AwaitingPayment 的交易已有 USDT 到账，
        /// 但买家忘记调用 confirm_payment 时，sidecar 可调用此函数
        /// 一步完成：确认付款 + 存储验证结果。
        ///
        /// 🆕 C3: tx_hash 用于防重放。
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::auto_confirm_payment())]
        pub fn auto_confirm_payment(
            origin: OriginFor<T>,
            trade_id: u64,
            actual_amount: u64,
            tx_hash: Option<TxHash>,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(trade.status == UsdtTradeStatus::AwaitingPayment, Error::<T>::InvalidTradeStatus);
            ensure!(trade.buyer_tron_address.is_some(), Error::<T>::InvalidTronAddress);

            // 🆕 C3: 检查 tx_hash 是否已被使用
            if let Some(ref hash) = tx_hash {
                ensure!(!UsedTxHashes::<T>::contains_key(hash), Error::<T>::TxHashAlreadyUsed);
            }

            let verification_result = Self::calculate_payment_verification_result(
                trade.usdt_amount, actual_amount,
            );

            Self::deposit_event(Event::AutoPaymentDetected { trade_id, actual_amount });

            // 从待付款队列移除
            AwaitingPaymentTrades::<T>::mutate(|list| { list.retain(|&id| id != trade_id); });

            match verification_result {
                PaymentVerificationResult::Underpaid => {
                    // 少付 → 进入 UnderpaidPending 补付窗口（与 submit_ocw_result 一致）
                    let now = <frame_system::Pallet<T>>::block_number();
                    let grace: BlockNumberFor<T> = T::UnderpaidGracePeriod::get().into();
                    let deadline = now.saturating_add(grace);

                    trade.status = UsdtTradeStatus::UnderpaidPending;
                    trade.first_verified_at = Some(now);
                    trade.first_actual_amount = Some(actual_amount);
                    trade.underpaid_deadline = Some(deadline);
                    UsdtTrades::<T>::insert(trade_id, &trade);

                    PendingUnderpaidTrades::<T>::try_mutate(|p| {
                        p.try_push(trade_id).map_err(|_| Error::<T>::UnderpaidQueueFull)
                    })?;
                    OcwVerificationResults::<T>::insert(trade_id, (verification_result, actual_amount));

                    let payment_ratio = pallet_trading_common::compute_payment_ratio_bps(trade.usdt_amount, actual_amount);
                    Self::deposit_event(Event::UnderpaidDetected {
                        trade_id,
                        expected_amount: trade.usdt_amount,
                        actual_amount,
                        payment_ratio,
                        deadline,
                    });
                }
                _ => {
                    // Exact/Overpaid/SeverelyUnderpaid/Invalid → 直接存储
                    trade.status = UsdtTradeStatus::AwaitingVerification;
                    UsdtTrades::<T>::insert(trade_id, &trade);

                    PendingUsdtTrades::<T>::try_mutate(|pending| {
                        pending.try_push(trade_id).map_err(|_| Error::<T>::PendingQueueFull)
                    })?;
                    OcwVerificationResults::<T>::insert(trade_id, (verification_result, actual_amount));

                    Self::deposit_event(Event::OcwResultSubmitted {
                        trade_id, verification_result, actual_amount,
                    });
                }
            }

            // 🆕 C3: 记录已使用的 tx_hash（防重放）
            // 🆕 M7: 同时记录插入区块号，用于 TTL 清理
            if let Some(hash) = tx_hash {
                let now = <frame_system::Pallet<T>>::block_number();
                UsedTxHashes::<T>::insert(hash, (trade_id, now));
            }

            Ok(())
        }

        /// OCW 更新少付交易的累计金额（unsigned）
        ///
        /// 补付窗口内，OCW 持续扫描 TronGrid，发现新转账则更新金额。
        /// 若累计金额达到 99.5%，直接升级为 Exact 结算。
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::submit_underpaid_update())]
        pub fn submit_underpaid_update(
            origin: OriginFor<T>,
            trade_id: u64,
            new_actual_amount: u64,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(trade.status == UsdtTradeStatus::UnderpaidPending, Error::<T>::NotUnderpaidPending);

            let previous_amount = OcwVerificationResults::<T>::get(trade_id)
                .map(|(_, amt)| amt).unwrap_or(0);

            // 只接受递增的金额（防止恶意回退）
            if new_actual_amount <= previous_amount {
                return Ok(());
            }

            let new_result = Self::calculate_payment_verification_result(
                trade.usdt_amount, new_actual_amount,
            );

            // 更新存储的验证结果
            OcwVerificationResults::<T>::insert(trade_id, (new_result, new_actual_amount));

            Self::deposit_event(Event::UnderpaidAmountUpdated {
                trade_id, previous_amount, new_amount: new_actual_amount,
            });

            // 补齐了！直接升级回 AwaitingVerification + Exact/Overpaid 结果
            if matches!(new_result, PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid) {
                trade.status = UsdtTradeStatus::AwaitingVerification;
                UsdtTrades::<T>::insert(trade_id, &trade);

                // 移回 PendingUsdtTrades
                PendingUnderpaidTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
                PendingUsdtTrades::<T>::try_mutate(|p| {
                    p.try_push(trade_id).map_err(|_| Error::<T>::PendingQueueFull)
                })?;
            }

            Ok(())
        }

        /// 少付终裁（任何人，补付窗口到期后）
        ///
        /// 补付窗口到期后，按最终累计金额做终裁：
        /// - 按比例释放 NEX
        /// - 保证金按梯度没收
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::finalize_underpaid())]
        pub fn finalize_underpaid(
            origin: OriginFor<T>,
            trade_id: u64,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(trade.status == UsdtTradeStatus::UnderpaidPending, Error::<T>::NotUnderpaidPending);

            let deadline = trade.underpaid_deadline.ok_or(Error::<T>::InvalidTradeStatus)?;
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now > deadline, Error::<T>::UnderpaidGraceNotExpired);

            // 读取最新的验证结果（可能被 submit_underpaid_update 更新过）
            let (_, final_amount) = OcwVerificationResults::<T>::get(trade_id)
                .ok_or(Error::<T>::OcwResultNotFound)?;

            let final_result = Self::calculate_payment_verification_result(
                trade.usdt_amount, final_amount,
            );

            match final_result {
                PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid => {
                    // 补付窗口内补齐了
                    Self::process_full_payment(&mut trade, trade_id)?;
                }
                _ => {
                    // 仍然少付 → 按梯度终裁
                    Self::process_underpaid(&mut trade, trade_id, final_amount)?;
                }
            }

            let payment_ratio = pallet_trading_common::compute_payment_ratio_bps(trade.usdt_amount, final_amount);
            let deposit_forfeit_rate = Self::calculate_deposit_forfeit_rate(payment_ratio);

            // 清理队列
            PendingUnderpaidTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
            OcwVerificationResults::<T>::remove(trade_id);

            Self::deposit_event(Event::UnderpaidFinalized {
                trade_id, final_amount, payment_ratio, deposit_forfeit_rate,
            });

            Ok(())
        }
    }

    // ==================== ValidateUnsigned ====================

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::submit_ocw_result { trade_id, actual_amount, tx_hash } => {
                    let trade = match UsdtTrades::<T>::get(trade_id) {
                        Some(t) => t,
                        None => return InvalidTransaction::Custom(10).into(),
                    };
                    if trade.status != UsdtTradeStatus::AwaitingVerification {
                        return InvalidTransaction::Custom(11).into();
                    }
                    if OcwVerificationResults::<T>::contains_key(trade_id) {
                        return InvalidTransaction::Custom(12).into();
                    }
                    // 🆕 C3: 在验证阶段即检查 tx_hash 重放
                    if let Some(ref hash) = tx_hash {
                        if UsedTxHashes::<T>::contains_key(hash) {
                            return InvalidTransaction::Custom(13).into();
                        }
                    }
                    // 🆕 C4: actual_amount 安全边界检查
                    // 拒绝明显伪造的金额（超过预期 10 倍）防止恶意强制 Overpaid
                    let amount_cap = trade.usdt_amount.saturating_mul(10);
                    if *actual_amount > amount_cap {
                        return InvalidTransaction::Custom(14).into();
                    }
                    // 安全：拒绝 External 来源，防止任意用户注入伪造 OCW 结果
                    if matches!(source, TransactionSource::External) {
                        return InvalidTransaction::BadSigner.into();
                    }
                    let priority = match source {
                        TransactionSource::Local => 100,
                        TransactionSource::InBlock => 80,
                        TransactionSource::External => 0,
                    };
                    // 🆕 M3: propagate(false) — unsigned 仅来自本地 OCW，不应广播给节点
                    ValidTransaction::with_tag_prefix("NexMarketOcwResult")
                        .priority(priority)
                        .longevity(10)
                        .and_provides([&(b"nex_ocw", trade_id)])
                        .propagate(false)
                        .build()
                },
                Call::auto_confirm_payment { trade_id, actual_amount, tx_hash } => {
                    let trade = match UsdtTrades::<T>::get(trade_id) {
                        Some(t) => t,
                        None => return InvalidTransaction::Custom(20).into(),
                    };
                    if trade.status != UsdtTradeStatus::AwaitingPayment {
                        return InvalidTransaction::Custom(21).into();
                    }
                    if trade.buyer_tron_address.is_none() {
                        return InvalidTransaction::Custom(22).into();
                    }
                    // 🆕 C3: 在验证阶段即检查 tx_hash 重放
                    if let Some(ref hash) = tx_hash {
                        if UsedTxHashes::<T>::contains_key(hash) {
                            return InvalidTransaction::Custom(23).into();
                        }
                    }
                    // 🆕 C4: actual_amount 安全边界检查
                    let amount_cap = trade.usdt_amount.saturating_mul(10);
                    if *actual_amount > amount_cap {
                        return InvalidTransaction::Custom(24).into();
                    }
                    // 安全：拒绝 External 来源，防止任意用户注入伪造自动确认
                    if matches!(source, TransactionSource::External) {
                        return InvalidTransaction::BadSigner.into();
                    }
                    let priority = match source {
                        TransactionSource::Local => 90,
                        TransactionSource::InBlock => 70,
                        TransactionSource::External => 0,
                    };
                    // 🆕 M3: propagate(false) — unsigned 仅来自本地 OCW
                    ValidTransaction::with_tag_prefix("NexMarketAutoConfirm")
                        .priority(priority)
                        .longevity(10)
                        .and_provides([&(b"nex_auto", trade_id)])
                        .propagate(false)
                        .build()
                },
                Call::submit_underpaid_update { trade_id, new_actual_amount } => {
                    let trade = match UsdtTrades::<T>::get(trade_id) {
                        Some(t) => t,
                        None => return InvalidTransaction::Custom(30).into(),
                    };
                    if trade.status != UsdtTradeStatus::UnderpaidPending {
                        return InvalidTransaction::Custom(31).into();
                    }
                    // 🆕 C4: new_actual_amount 安全边界检查
                    let amount_cap = trade.usdt_amount.saturating_mul(10);
                    if *new_actual_amount > amount_cap {
                        return InvalidTransaction::Custom(32).into();
                    }
                    // 🆕 C4: 单调递增检查（避免无效更新浪费区块空间）
                    let previous_amount = OcwVerificationResults::<T>::get(trade_id)
                        .map(|(_, amt)| amt).unwrap_or(0);
                    if *new_actual_amount <= previous_amount {
                        return InvalidTransaction::Custom(33).into();
                    }
                    // 安全：拒绝 External 来源，防止任意用户注入伪造补付金额
                    if matches!(source, TransactionSource::External) {
                        return InvalidTransaction::BadSigner.into();
                    }
                    let priority = match source {
                        TransactionSource::Local => 80,
                        TransactionSource::InBlock => 60,
                        TransactionSource::External => 0,
                    };
                    // 🆕 M3: propagate(false) — unsigned 仅来自本地 OCW
                    ValidTransaction::with_tag_prefix("NexMarketUnderpaidUpdate")
                        .priority(priority)
                        .longevity(5)
                        .and_provides([&(b"nex_upd", trade_id)])
                        .propagate(false)
                        .build()
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 创建订单
        fn do_create_order(
            maker: T::AccountId,
            side: OrderSide,
            nex_amount: BalanceOf<T>,
            usdt_price: u64,
            tron_address: Option<TronAddress>,
            buyer_deposit: BalanceOf<T>,
            deposit_waived: bool,
        ) -> Result<u64, DispatchError> {
            let order_id = NextOrderId::<T>::get();
            ensure!(order_id < u64::MAX, Error::<T>::ArithmeticOverflow);
            NextOrderId::<T>::put(order_id.saturating_add(1));

            let now = <frame_system::Pallet<T>>::block_number();
            let ttl: u32 = T::DefaultOrderTTL::get();
            let expires_at = now.saturating_add(ttl.into());

            let order = Order {
                order_id,
                maker: maker.clone(),
                side,
                nex_amount,
                filled_amount: Zero::zero(),
                usdt_price,
                tron_address,
                status: OrderStatus::Open,
                created_at: now,
                expires_at,
                buyer_deposit,
                deposit_waived,
            };

            Orders::<T>::insert(order_id, order);

            match side {
                OrderSide::Sell => {
                    SellOrders::<T>::try_mutate(|orders| {
                        orders.try_push(order_id).map_err(|_| Error::<T>::OrderBookFull)
                    })?;
                }
                OrderSide::Buy => {
                    BuyOrders::<T>::try_mutate(|orders| {
                        orders.try_push(order_id).map_err(|_| Error::<T>::OrderBookFull)
                    })?;
                }
            }

            UserOrders::<T>::try_mutate(&maker, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::UserOrdersFull)
            })?;

            MarketStatsStore::<T>::mutate(|stats| {
                stats.total_orders = stats.total_orders.saturating_add(1);
            });

            Ok(order_id)
        }

        /// 创建 USDT 交易（标准超时）
        fn do_create_usdt_trade(
            order_id: u64,
            seller: T::AccountId,
            buyer: T::AccountId,
            nex_amount: BalanceOf<T>,
            usdt_amount: u64,
            seller_tron_address: TronAddress,
            buyer_tron_address: Option<TronAddress>,
            buyer_deposit: BalanceOf<T>,
        ) -> Result<u64, DispatchError> {
            Self::do_create_usdt_trade_ex(
                order_id, seller, buyer, nex_amount, usdt_amount,
                seller_tron_address, buyer_tron_address, buyer_deposit, false,
            )
        }

        /// 创建 USDT 交易（扩展版，支持免保证金短超时）
        fn do_create_usdt_trade_ex(
            order_id: u64,
            seller: T::AccountId,
            buyer: T::AccountId,
            nex_amount: BalanceOf<T>,
            usdt_amount: u64,
            seller_tron_address: TronAddress,
            buyer_tron_address: Option<TronAddress>,
            buyer_deposit: BalanceOf<T>,
            waived_deposit: bool,
        ) -> Result<u64, DispatchError> {
            let trade_id = NextUsdtTradeId::<T>::get();
            ensure!(trade_id < u64::MAX, Error::<T>::ArithmeticOverflow);
            NextUsdtTradeId::<T>::put(trade_id.saturating_add(1));

            let now = <frame_system::Pallet<T>>::block_number();
            // 免保证金交易使用短超时，普通交易使用标准超时
            let timeout: u32 = if waived_deposit {
                T::FirstOrderTimeout::get()
            } else {
                T::UsdtTimeout::get()
            };
            let timeout_at = now.saturating_add(timeout.into());

            let deposit_status = if buyer_deposit.is_zero() {
                BuyerDepositStatus::None
            } else {
                BuyerDepositStatus::Locked
            };

            let trade = UsdtTrade {
                trade_id, order_id, seller, buyer,
                nex_amount, usdt_amount, seller_tron_address,
                buyer_tron_address,
                status: UsdtTradeStatus::AwaitingPayment,
                created_at: now, timeout_at,
                buyer_deposit, deposit_status,
                first_verified_at: None,
                first_actual_amount: None,
                underpaid_deadline: None,
            };

            UsdtTrades::<T>::insert(trade_id, trade);

            // 加入待付款跟踪队列
            AwaitingPaymentTrades::<T>::try_mutate(|list| {
                list.try_push(trade_id).map_err(|_| Error::<T>::AwaitingPaymentQueueFull)
            })?;

            Ok(trade_id)
        }

        /// 计算买家保证金
        fn calculate_buyer_deposit(usdt_amount: u64) -> BalanceOf<T> {
            let rate = T::BuyerDepositRate::get(); // bps
            let usdt_to_nex = T::UsdtToNexRate::get(); // 精度 10^6

            // deposit_nex = usdt_amount * rate/10000 * usdt_to_nex/10^6
            let deposit_u128 = (usdt_amount as u128)
                .saturating_mul(rate as u128)
                .saturating_div(10000)
                .saturating_mul(usdt_to_nex as u128)
                .saturating_div(1_000_000);
            let calculated: BalanceOf<T> = deposit_u128.saturated_into();
            let min_deposit = T::MinBuyerDeposit::get();

            if calculated > min_deposit { calculated } else { min_deposit }
        }

        /// 多档判定（委托给 pallet-trading-common）
        fn calculate_payment_verification_result(
            expected_amount: u64,
            actual_amount: u64,
        ) -> PaymentVerificationResult {
            pallet_trading_common::calculate_payment_verification_result(expected_amount, actual_amount)
        }

        /// 领取验证奖励
        fn do_claim_verification_reward(
            caller: &T::AccountId,
            trade_id: u64,
        ) -> DispatchResult {
            let mut trade = UsdtTrades::<T>::get(trade_id)
                .ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(
                trade.status == UsdtTradeStatus::AwaitingVerification,
                Error::<T>::InvalidTradeStatus
            );

            let (verification_result, actual_amount) = OcwVerificationResults::<T>::get(trade_id)
                .ok_or(Error::<T>::OcwResultNotFound)?;

            match verification_result {
                PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid => {
                    Self::process_full_payment(&mut trade, trade_id)?;
                }
                PaymentVerificationResult::Underpaid |
                PaymentVerificationResult::SeverelyUnderpaid => {
                    Self::process_underpaid(&mut trade, trade_id, actual_amount)?;
                }
                PaymentVerificationResult::Invalid => {
                    Self::process_underpaid(&mut trade, trade_id, 0)?;
                }
            }

            PendingUsdtTrades::<T>::mutate(|pending| { pending.retain(|&id| id != trade_id); });
            OcwVerificationResults::<T>::remove(trade_id);

            // 支付奖励（非核心操作，失败不阻断交易结算）
            let reward = T::VerificationReward::get();
            let reward_paid = if reward > Zero::zero() {
                let reward_source = T::RewardSource::get();
                T::Currency::transfer(
                    &reward_source, caller, reward, ExistenceRequirement::KeepAlive,
                ).is_ok()
            } else {
                true
            };

            Self::deposit_event(Event::VerificationRewardClaimed {
                trade_id, claimer: caller.clone(), reward, reward_paid,
            });

            Ok(())
        }

        /// 处理全额付款
        fn process_full_payment(
            trade: &mut UsdtTrade<T>,
            trade_id: u64,
        ) -> DispatchResult {
            // 释放锁定的 NEX 给买家
            T::Currency::repatriate_reserved(
                &trade.seller, &trade.buyer, trade.nex_amount,
                frame_support::traits::BalanceStatus::Free,
            )?;

            // 退还保证金
            if !trade.buyer_deposit.is_zero() && trade.deposit_status == BuyerDepositStatus::Locked {
                T::Currency::unreserve(&trade.buyer, trade.buyer_deposit);
                trade.deposit_status = BuyerDepositStatus::Released;

                Self::deposit_event(Event::BuyerDepositReleased {
                    trade_id, buyer: trade.buyer.clone(), deposit: trade.buyer_deposit,
                });
            }

            trade.status = UsdtTradeStatus::Completed;
            let usdt_amount = trade.usdt_amount;
            let order_id = trade.order_id;
            UsdtTrades::<T>::insert(trade_id, &*trade);

            // L3: 标记买家已完成交易 + 清理活跃免保证金计数
            Self::finalize_waived_trade_tracking(&trade.buyer, order_id);

            // 🆕 M2: 缓存 Orders 读取，避免双读存储
            let maybe_order = Orders::<T>::get(order_id);

            // 审计：累计 seed_liquidity 成交 USDT
            if let Some(ref order) = maybe_order {
                if order.deposit_waived {
                    CumulativeSeedUsdtSold::<T>::mutate(|total| {
                        *total = total.saturating_add(usdt_amount);
                    });
                }
            }

            MarketStatsStore::<T>::mutate(|stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_usdt = stats.total_volume_usdt.saturating_add(usdt_amount);
            });

            // 更新 TWAP
            if let Some(ref order) = maybe_order {
                Self::on_trade_completed(order.usdt_price);
            }

            Self::deposit_event(Event::UsdtTradeCompleted { trade_id, order_id });

            Ok(())
        }

        /// 处理少付（保证金按梯度没收）
        fn process_underpaid(
            trade: &mut UsdtTrade<T>,
            trade_id: u64,
            actual_amount: u64,
        ) -> DispatchResult {
            let payment_ratio = pallet_trading_common::compute_payment_ratio_bps(trade.usdt_amount, actual_amount);

            // 按比例释放 NEX
            let nex_u128: u128 = trade.nex_amount.saturated_into();
            let nex_to_release_u128 = nex_u128
                .saturating_mul(payment_ratio as u128)
                .saturating_div(10000);
            let nex_to_release: BalanceOf<T> = nex_to_release_u128.saturated_into();
            let nex_to_refund = trade.nex_amount.saturating_sub(nex_to_release);

            // 释放部分 NEX 给买家
            if !nex_to_release.is_zero() {
                T::Currency::repatriate_reserved(
                    &trade.seller, &trade.buyer, nex_to_release,
                    frame_support::traits::BalanceStatus::Free,
                )?;
            }

            // 退还剩余 NEX 给卖家
            if !nex_to_refund.is_zero() {
                T::Currency::unreserve(&trade.seller, nex_to_refund);
                Self::rollback_order_filled_amount(trade.order_id, nex_to_refund);
            }

            // 保证金按梯度没收
            let forfeit_rate = Self::calculate_deposit_forfeit_rate(payment_ratio);
            let mut deposit_forfeited = BalanceOf::<T>::zero();
            if !trade.buyer_deposit.is_zero() && trade.deposit_status == BuyerDepositStatus::Locked {
                let deposit_u128: u128 = trade.buyer_deposit.saturated_into();
                let forfeit_u128 = deposit_u128
                    .saturating_mul(forfeit_rate as u128)
                    .saturating_div(10000);
                deposit_forfeited = forfeit_u128.saturated_into();

                if !deposit_forfeited.is_zero() {
                    let treasury = T::TreasuryAccount::get();
                    // repatriate_reserved 返回 Ok(remaining) 表示未能转移的余额
                    let actually_forfeited = match T::Currency::repatriate_reserved(
                        &trade.buyer, &treasury, deposit_forfeited,
                        frame_support::traits::BalanceStatus::Free,
                    ) {
                        Ok(remaining) => deposit_forfeited.saturating_sub(remaining),
                        Err(_) => Zero::zero(),
                    };
                    deposit_forfeited = actually_forfeited;
                }

                // 退还未没收部分
                let refund = trade.buyer_deposit.saturating_sub(deposit_forfeited);
                if !refund.is_zero() {
                    T::Currency::unreserve(&trade.buyer, refund);
                }

                trade.deposit_status = BuyerDepositStatus::Forfeited;
            }

            // L2: actual_amount=0 (Invalid) 用 Refunded 更准确
            trade.status = if actual_amount > 0 {
                UsdtTradeStatus::Completed
            } else {
                UsdtTradeStatus::Refunded
            };
            let expected_amount = trade.usdt_amount;
            let order_id = trade.order_id;
            UsdtTrades::<T>::insert(trade_id, &*trade);

            // 清理活跃免保证金计数（不标记 CompletedBuyers，少付不算完成）
            Self::cleanup_waived_trade_counter(&trade.buyer, order_id);

            // 审计：累计 seed_liquidity 实际收到的 USDT
            if let Some(order) = Orders::<T>::get(order_id) {
                if order.deposit_waived && actual_amount > 0 {
                    CumulativeSeedUsdtSold::<T>::mutate(|total| {
                        *total = total.saturating_add(actual_amount);
                    });
                }
            }

            MarketStatsStore::<T>::mutate(|stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_usdt = stats.total_volume_usdt.saturating_add(actual_amount);
            });

            Self::deposit_event(Event::UnderpaidAutoProcessed {
                trade_id, expected_amount, actual_amount, payment_ratio,
                nex_released: nex_to_release, deposit_forfeited,
            });

            Ok(())
        }

        /// 保证金没收梯度（委托给 pallet-trading-common）
        fn calculate_deposit_forfeit_rate(payment_ratio: u32) -> u16 {
            pallet_trading_common::calculate_deposit_forfeit_rate(payment_ratio)
        }

        /// 没收买家保证金（提取公共逻辑）
        fn forfeit_buyer_deposit(trade: &mut UsdtTrade<T>, trade_id: u64) {
            if trade.buyer_deposit.is_zero() || trade.deposit_status != BuyerDepositStatus::Locked {
                return;
            }

            let forfeit_rate = T::DepositForfeitRate::get();
            let deposit_u128: u128 = trade.buyer_deposit.saturated_into();
            let forfeit_u128 = deposit_u128
                .saturating_mul(forfeit_rate as u128)
                .saturating_div(10000);
            let forfeit_amount: BalanceOf<T> = forfeit_u128.saturated_into();

            let actually_forfeited = if !forfeit_amount.is_zero() {
                let treasury = T::TreasuryAccount::get();
                match T::Currency::repatriate_reserved(
                    &trade.buyer, &treasury, forfeit_amount,
                    frame_support::traits::BalanceStatus::Free,
                ) {
                    Ok(remaining) => forfeit_amount.saturating_sub(remaining),
                    Err(_) => Zero::zero(),
                }
            } else {
                Zero::zero()
            };

            let refund = trade.buyer_deposit.saturating_sub(actually_forfeited);
            if !refund.is_zero() {
                T::Currency::unreserve(&trade.buyer, refund);
            }

            trade.deposit_status = BuyerDepositStatus::Forfeited;

            Self::deposit_event(Event::BuyerDepositForfeited {
                trade_id, buyer: trade.buyer.clone(),
                forfeited: actually_forfeited, to_treasury: actually_forfeited,
            });
        }

        /// 回滚订单 filled_amount
        ///
        /// M2 修复：当订单从 Filled 恢复时，重新加入订单簿和用户索引，
        /// 防止订单成为幽灵记录（状态 Open/PartiallyFilled 但不在索引中）。
        fn rollback_order_filled_amount(order_id: u64, amount: BalanceOf<T>) {
            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(order) = maybe_order {
                    let was_filled = order.status == OrderStatus::Filled;
                    order.filled_amount = order.filled_amount.saturating_sub(amount);

                    if order.filled_amount < order.nex_amount {
                        if order.filled_amount.is_zero() {
                            order.status = OrderStatus::Open;
                        } else {
                            order.status = OrderStatus::PartiallyFilled;
                        }

                        // 如果从 Filled 恢复，重新加入订单簿和用户索引
                        if was_filled {
                            let book_ok = match order.side {
                                OrderSide::Sell => {
                                    SellOrders::<T>::try_mutate(|orders| {
                                        orders.try_push(order_id)
                                    }).is_ok()
                                }
                                OrderSide::Buy => {
                                    BuyOrders::<T>::try_mutate(|orders| {
                                        orders.try_push(order_id)
                                    }).is_ok()
                                }
                            };
                            if !book_ok {
                                log::warn!(target: "nex-market",
                                    "Order {} rollback: order book full, order is a ghost record", order_id);
                            }
                            let user_ok = UserOrders::<T>::try_mutate(&order.maker, |orders| {
                                orders.try_push(order_id)
                            }).is_ok();
                            if !user_ok {
                                log::warn!(target: "nex-market",
                                    "Order {} rollback: user orders full for {:?}", order_id, order.maker);
                            }
                        }
                    }
                }
            });
        }

        /// 从订单簿移除
        fn remove_from_order_book(order_id: u64, side: OrderSide) {
            match side {
                OrderSide::Sell => {
                    SellOrders::<T>::mutate(|orders| { orders.retain(|&id| id != order_id); });
                }
                OrderSide::Buy => {
                    BuyOrders::<T>::mutate(|orders| { orders.retain(|&id| id != order_id); });
                }
            }
        }

        /// L3: 成功完成交易后标记买家 + 清理活跃免保证金计数
        fn finalize_waived_trade_tracking(buyer: &T::AccountId, order_id: u64) {
            if let Some(order) = Orders::<T>::get(order_id) {
                if order.deposit_waived {
                    CompletedBuyers::<T>::insert(buyer, true);
                    ActiveWaivedTrades::<T>::mutate(buyer, |count| {
                        *count = count.saturating_sub(1);
                    });
                }
            }
        }

        /// 清理活跃免保证金计数（超时/少付，不标记 CompletedBuyers）
        fn cleanup_waived_trade_counter(buyer: &T::AccountId, order_id: u64) {
            if let Some(order) = Orders::<T>::get(order_id) {
                if order.deposit_waived {
                    ActiveWaivedTrades::<T>::mutate(buyer, |count| {
                        *count = count.saturating_sub(1);
                    });
                }
            }
        }

        /// 保护性瀑布式 seed 基准价格：
        ///   成熟期（≥min_trades）: 7d TWAP（24~48h 窗口，最抗操纵）
        ///   过渡期（≥30 笔）:      max(24h TWAP, InitialPrice)（只涨不跌）
        ///   冷启动（<30 笔）:       InitialPrice（人为设定兜底）
        pub fn get_seed_reference_price() -> Option<u64> {
            let config = PriceProtectionStore::<T>::get().unwrap_or_default();
            let initial = config.initial_price?;

            if let Some(acc) = TwapAccumulatorStore::<T>::get() {
                // 成熟期：交易数充足，信任 7d TWAP
                if acc.trade_count >= config.min_trades_for_twap {
                    if let Some(twap_7d) = Self::calculate_twap(TwapPeriod::OneWeek) {
                        if twap_7d > 0 {
                            return Some(twap_7d);
                        }
                    }
                }
                // 过渡期：取 max(24h TWAP, InitialPrice)，基准只涨不跌
                const TRANSITION_TRADES: u64 = 30;
                if acc.trade_count >= TRANSITION_TRADES {
                    if let Some(twap_24h) = Self::calculate_twap(TwapPeriod::OneDay) {
                        if twap_24h > initial {
                            return Some(twap_24h);
                        }
                    }
                }
            }

            // 冷启动 / 交易不足 / 市场下跌：InitialPrice 兜底
            Some(initial)
        }

        /// 🆕 H4: 全量刷新最优价格（仅在 GC / seed_liquidity 批量操作后调用）
        fn refresh_best_prices() {
            let now = <frame_system::Pallet<T>>::block_number();

            let best_ask = SellOrders::<T>::get().iter()
                .filter_map(|&id| Orders::<T>::get(id))
                .filter(|o| {
                    (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                    && now <= o.expires_at
                })
                .map(|o| o.usdt_price)
                .min();
            match best_ask {
                Some(p) => BestAsk::<T>::put(p),
                None => BestAsk::<T>::kill(),
            }

            let best_bid = BuyOrders::<T>::get().iter()
                .filter_map(|&id| Orders::<T>::get(id))
                .filter(|o| {
                    (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                    && now <= o.expires_at
                })
                .map(|o| o.usdt_price)
                .max();
            match best_bid {
                Some(p) => BestBid::<T>::put(p),
                None => BestBid::<T>::kill(),
            }
        }

        /// 🆕 H4: 增量更新 — 新订单创建时 O(1) 比较
        fn update_best_price_on_new_order(price: u64, side: OrderSide) {
            match side {
                OrderSide::Sell => {
                    // 卖单取最低价
                    let should_update = BestAsk::<T>::get()
                        .map_or(true, |current| price < current);
                    if should_update {
                        BestAsk::<T>::put(price);
                    }
                }
                OrderSide::Buy => {
                    // 买单取最高价
                    let should_update = BestBid::<T>::get()
                        .map_or(true, |current| price > current);
                    if should_update {
                        BestBid::<T>::put(price);
                    }
                }
            }
        }

        /// 🆕 H4: 增量更新 — 订单移除/取消/成交时，仅在影响当前最优价时重扫
        fn update_best_price_on_remove(price: u64, side: OrderSide) {
            match side {
                OrderSide::Sell => {
                    if BestAsk::<T>::get() == Some(price) {
                        // 移除的恰好是最优卖价 → 需要重扫卖单找新最优
                        let now = <frame_system::Pallet<T>>::block_number();
                        let new_best = SellOrders::<T>::get().iter()
                            .filter_map(|&id| Orders::<T>::get(id))
                            .filter(|o| {
                                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                                && now <= o.expires_at
                            })
                            .map(|o| o.usdt_price)
                            .min();
                        match new_best {
                            Some(p) => BestAsk::<T>::put(p),
                            None => BestAsk::<T>::kill(),
                        }
                    }
                }
                OrderSide::Buy => {
                    if BestBid::<T>::get() == Some(price) {
                        let now = <frame_system::Pallet<T>>::block_number();
                        let new_best = BuyOrders::<T>::get().iter()
                            .filter_map(|&id| Orders::<T>::get(id))
                            .filter(|o| {
                                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                                && now <= o.expires_at
                            })
                            .map(|o| o.usdt_price)
                            .max();
                        match new_best {
                            Some(p) => BestBid::<T>::put(p),
                            None => BestBid::<T>::kill(),
                        }
                    }
                }
            }
        }

        /// 价格偏离检查
        pub fn check_price_deviation(usdt_price: u64) -> Result<(), Error<T>> {
            let config = PriceProtectionStore::<T>::get().unwrap_or_default();
            if !config.enabled { return Ok(()); }

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
            if config.circuit_breaker_active && current_block < config.circuit_breaker_until {
                return Err(Error::<T>::MarketCircuitBreakerActive);
            }

            let reference_price: Option<u64> = {
                let acc = TwapAccumulatorStore::<T>::get();
                match acc {
                    Some(ref a) if Self::is_twap_data_sufficient(a, current_block, &config) => {
                        Self::calculate_twap(TwapPeriod::OneHour)
                    }
                    _ => config.initial_price,
                }
            };

            let ref_price = match reference_price {
                Some(p) if p > 0 => p,
                _ => return Ok(()),
            };

            // 🆕 M4: 使用 saturating cast 防止 u16 截断（极端偏离 >655% 时不会回绕为小值）
            let deviation_raw = if usdt_price > ref_price {
                (usdt_price as u128 - ref_price as u128) * 10000 / ref_price as u128
            } else {
                (ref_price as u128 - usdt_price as u128) * 10000 / ref_price as u128
            };
            let deviation_bps = deviation_raw.min(u16::MAX as u128) as u16;

            if deviation_bps > config.max_price_deviation {
                return Err(Error::<T>::PriceDeviationTooHigh);
            }

            Ok(())
        }

        // ==================== TWAP ====================

        /// 更新 TWAP 累积器
        fn update_twap_accumulator(trade_price: u64) {
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            TwapAccumulatorStore::<T>::mutate(|maybe_acc| {
                let acc = maybe_acc.get_or_insert_with(|| TwapAccumulator {
                    last_price: trade_price,
                    current_block,
                    hour_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    day_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    week_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    last_hour_update: current_block,
                    last_day_update: current_block,
                    last_week_update: current_block,
                    ..Default::default()
                });

                // 异常价格过滤
                let filtered_price = if acc.trade_count > 0 && acc.last_price > 0 {
                    let deviation = if trade_price > acc.last_price {
                        trade_price - acc.last_price
                    } else {
                        acc.last_price - trade_price
                    };
                    if deviation > acc.last_price {
                        if trade_price > acc.last_price {
                            acc.last_price.saturating_mul(3) / 2
                        } else {
                            acc.last_price / 2
                        }
                    } else {
                        trade_price
                    }
                } else {
                    trade_price
                };

                let blocks_elapsed = current_block.saturating_sub(acc.current_block);
                if blocks_elapsed > 0 {
                    acc.current_cumulative = acc.current_cumulative
                        .saturating_add((acc.last_price as u128).saturating_mul(blocks_elapsed as u128));
                }

                acc.current_block = current_block;
                acc.last_price = filtered_price;
                acc.trade_count = acc.trade_count.saturating_add(1);

                Self::advance_snapshots(acc, current_block);
            });
        }

        /// 按时间间隔推进 TWAP snapshots（交易时 + on_idle 共用）
        fn advance_snapshots(acc: &mut TwapAccumulator, current_block: u32) {
            let bph = T::BlocksPerHour::get();
            let bpd = T::BlocksPerDay::get();

            let hour_interval = bph / 6;
            if current_block.saturating_sub(acc.last_hour_update) >= hour_interval {
                acc.hour_snapshot = PriceSnapshot { cumulative_price: acc.current_cumulative, block_number: current_block };
                acc.last_hour_update = current_block;
            }
            if current_block.saturating_sub(acc.last_day_update) >= bph {
                acc.day_snapshot = PriceSnapshot { cumulative_price: acc.current_cumulative, block_number: current_block };
                acc.last_day_update = current_block;
            }
            if current_block.saturating_sub(acc.last_week_update) >= bpd {
                acc.week_snapshot = PriceSnapshot { cumulative_price: acc.current_cumulative, block_number: current_block };
                acc.last_week_update = current_block;
            }
        }

        /// 计算 TWAP
        pub fn calculate_twap(period: TwapPeriod) -> Option<u64> {
            let acc = TwapAccumulatorStore::<T>::get()?;
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            let snapshot = match period {
                TwapPeriod::OneHour => &acc.hour_snapshot,
                TwapPeriod::OneDay => &acc.day_snapshot,
                TwapPeriod::OneWeek => &acc.week_snapshot,
            };

            let blocks_since = current_block.saturating_sub(acc.current_block);
            let current_cumulative = acc.current_cumulative
                .saturating_add((acc.last_price as u128).saturating_mul(blocks_since as u128));

            let block_diff = current_block.saturating_sub(snapshot.block_number);
            if block_diff == 0 {
                return Some(acc.last_price);
            }

            let cumulative_diff = current_cumulative.saturating_sub(snapshot.cumulative_price);
            Some((cumulative_diff / (block_diff as u128)) as u64)
        }

        fn is_twap_data_sufficient(
            acc: &TwapAccumulator,
            current_block: u32,
            config: &PriceProtectionConfig,
        ) -> bool {
            if acc.trade_count < config.min_trades_for_twap { return false; }
            let bph = T::BlocksPerHour::get();
            let bpd = T::BlocksPerDay::get();
            let bpw = T::BlocksPerWeek::get();
            current_block.saturating_sub(acc.hour_snapshot.block_number) >= bph
                && current_block.saturating_sub(acc.day_snapshot.block_number) >= bpd
                && current_block.saturating_sub(acc.week_snapshot.block_number) >= bpw
        }

        fn check_circuit_breaker(current_price: u64) {
            let config = match PriceProtectionStore::<T>::get() {
                Some(c) if c.enabled => c,
                _ => return,
            };

            let twap_7d = match Self::calculate_twap(TwapPeriod::OneWeek) {
                Some(t) if t > 0 => t,
                _ => return,
            };

            // 🆕 M4: saturating cast 防止 u16 截断
            let deviation_raw = if current_price > twap_7d {
                (current_price as u128 - twap_7d as u128) * 10000 / twap_7d as u128
            } else {
                (twap_7d as u128 - current_price as u128) * 10000 / twap_7d as u128
            };
            let deviation_bps = deviation_raw.min(u16::MAX as u128) as u16;

            if deviation_bps > config.circuit_breaker_threshold {
                let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                let until_block = current_block.saturating_add(T::CircuitBreakerDuration::get());

                PriceProtectionStore::<T>::mutate(|maybe| {
                    if let Some(c) = maybe {
                        c.circuit_breaker_active = true;
                        c.circuit_breaker_until = until_block;
                    }
                });

                Self::deposit_event(Event::CircuitBreakerTriggered {
                    current_price, twap_7d, deviation_bps, until_block,
                });
            }
        }

        fn on_trade_completed(trade_price: u64) {
            Self::update_twap_accumulator(trade_price);
            LastTradePrice::<T>::put(trade_price);

            let twap_1h = Self::calculate_twap(TwapPeriod::OneHour);
            let twap_24h = Self::calculate_twap(TwapPeriod::OneDay);
            let twap_7d = Self::calculate_twap(TwapPeriod::OneWeek);
            Self::deposit_event(Event::TwapUpdated {
                new_price: trade_price, twap_1h, twap_24h, twap_7d,
            });

            Self::check_circuit_breaker(trade_price);
        }

        /// OCW: 处理验证（按 from/to/amount 匹配 TRON 链上转账）
        ///
        /// 调用 pallet-trading-trc20-verifier 查询 TronGrid API，
        /// 搜索 buyer→seller 的 USDT TRC20 转账记录，
        /// 将结果写入 offchain local storage，供外部服务读取后
        /// 调用 submit_ocw_result unsigned 交易提交到链上。
        fn process_verification(trade_id: u64, trade: &UsdtTrade<T>, buyer_tron: &[u8], seller_tron: &[u8]) {
            log::info!(target: "nex-market-ocw",
                "Verifying trade {} by (from={} bytes, to={} bytes, amount={})",
                trade_id, buyer_tron.len(), seller_tron.len(), trade.usdt_amount);

            // 搜索起始时间：保守回溯 24 小时
            let now_ms = sp_io::offchain::timestamp().unix_millis();
            let min_timestamp = now_ms.saturating_sub(24 * 3600 * 1000);

            let (actual_amount, tx_hash_bytes) = match pallet_trading_trc20_verifier::verify_trc20_by_transfer(
                buyer_tron, seller_tron, trade.usdt_amount, min_timestamp,
            ) {
                Ok(result) => {
                    if result.found {
                        log::info!(target: "nex-market-ocw",
                            "Trade {} verified: actual_amount={:?}, status={:?}",
                            trade_id, result.actual_amount, result.amount_status);
                        (result.actual_amount.unwrap_or(0), result.tx_hash)
                    } else {
                        log::warn!(target: "nex-market-ocw",
                            "Trade {} verification: no matching transfer found (error={:?})",
                            trade_id, result.error);
                        (0, None)
                    }
                }
                Err(e) => {
                    log::error!(target: "nex-market-ocw",
                        "Trade {} verification HTTP error: {}", trade_id, e);
                    // HTTP 失败 — 不写 storage，等待下一个区块重试
                    return;
                }
            };

            // 将验证结果写入 offchain local storage
            // 外部服务（sidecar）读取后调用 submit_ocw_result
            // 🆕 C3: 包含 tx_hash 用于防重放
            let key = Self::ocw_result_key(trade_id);
            let tx_hash_for_storage: Vec<u8> = tx_hash_bytes.unwrap_or_default();
            let value = (true, actual_amount, tx_hash_for_storage);
            sp_io::offchain::local_storage_set(
                sp_core::offchain::StorageKind::PERSISTENT,
                &key,
                &codec::Encode::encode(&value),
            );

            log::info!(target: "nex-market-ocw",
                "Trade {} result stored: actual_amount={}", trade_id, actual_amount);
        }

        fn ocw_result_key(trade_id: u64) -> Vec<u8> {
            let mut key = b"nex_market_ocw::".to_vec();
            key.extend_from_slice(&trade_id.to_le_bytes());
            key
        }

        /// OCW: 预检 AwaitingPayment 交易是否已有 USDT 到账
        ///
        /// 当买家忘记 confirm_payment 但 USDT 已到时，
        /// 将 (trade_id, actual_amount) 写入 offchain storage，
        /// sidecar 读取后调用 auto_confirm_payment unsigned 交易。
        fn auto_check_awaiting_payment(trade_id: u64, trade: &UsdtTrade<T>, buyer_tron: &[u8], seller_tron: &[u8]) {
            log::info!(target: "nex-market-ocw",
                "Auto-checking AwaitingPayment trade {} (amount={})",
                trade_id, trade.usdt_amount);

            let now_ms = sp_io::offchain::timestamp().unix_millis();
            let min_timestamp = now_ms.saturating_sub(24 * 3600 * 1000);

            match pallet_trading_trc20_verifier::verify_trc20_by_transfer(
                buyer_tron, seller_tron, trade.usdt_amount, min_timestamp,
            ) {
                Ok(result) if result.found => {
                    let actual_amount = result.actual_amount.unwrap_or(0);
                    if actual_amount > 0 {
                        log::info!(target: "nex-market-ocw",
                            "Auto-detected payment for trade {}: actual_amount={}",
                            trade_id, actual_amount);

                        // 写入 offchain storage，sidecar 调用 auto_confirm_payment
                        // 🆕 C3: 包含 tx_hash 用于防重放
                        let key = Self::ocw_auto_confirm_key(trade_id);
                        let tx_hash_for_storage: Vec<u8> = result.tx_hash.unwrap_or_default();
                        let value = (true, actual_amount, tx_hash_for_storage);
                        sp_io::offchain::local_storage_set(
                            sp_core::offchain::StorageKind::PERSISTENT,
                            &key,
                            &codec::Encode::encode(&value),
                        );
                    }
                }
                Ok(_) => {
                    // 未找到转账 — 正常情况（买家可能真没付）
                }
                Err(e) => {
                    log::warn!(target: "nex-market-ocw",
                        "Auto-check trade {} HTTP error: {}", trade_id, e);
                }
            }
        }

        fn ocw_auto_confirm_key(trade_id: u64) -> Vec<u8> {
            let mut key = b"nex_market_auto::".to_vec();
            key.extend_from_slice(&trade_id.to_le_bytes());
            key
        }

        /// OCW: 补付窗口内持续检查 UnderpaidPending 交易
        ///
        /// 重新查询 TronGrid 累计金额，若金额增加则写入 offchain storage，
        /// sidecar 调用 submit_underpaid_update 更新链上。
        fn check_underpaid_topup(trade_id: u64, trade: &UsdtTrade<T>, buyer_tron: &[u8], seller_tron: &[u8]) {
            log::info!(target: "nex-market-ocw",
                "Checking underpaid topup for trade {} (expected={})",
                trade_id, trade.usdt_amount);

            let now_ms = sp_io::offchain::timestamp().unix_millis();
            let min_timestamp = now_ms.saturating_sub(48 * 3600 * 1000); // 回溯 48 小时

            match pallet_trading_trc20_verifier::verify_trc20_by_transfer(
                buyer_tron, seller_tron, trade.usdt_amount, min_timestamp,
            ) {
                Ok(result) if result.found => {
                    let actual_amount = result.actual_amount.unwrap_or(0);
                    if actual_amount > 0 {
                        // 写入 offchain storage，sidecar 调用 submit_underpaid_update
                        let key = Self::ocw_underpaid_key(trade_id);
                        let value = (true, actual_amount);
                        sp_io::offchain::local_storage_set(
                            sp_core::offchain::StorageKind::PERSISTENT,
                            &key,
                            &codec::Encode::encode(&value),
                        );
                        log::info!(target: "nex-market-ocw",
                            "Underpaid trade {} updated amount: {}", trade_id, actual_amount);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!(target: "nex-market-ocw",
                        "Underpaid check trade {} HTTP error: {}", trade_id, e);
                }
            }
        }

        fn ocw_underpaid_key(trade_id: u64) -> Vec<u8> {
            let mut key = b"nex_market_undp::".to_vec();
            key.extend_from_slice(&trade_id.to_le_bytes());
            key
        }
    }
}

// ==================== PriceOracle 实现 ====================

impl<T: pallet::Config> pallet_trading_common::PriceOracle for pallet::Pallet<T> {
    fn get_twap(window: pallet_trading_common::TwapWindow) -> Option<u64> {
        use pallet_trading_common::TwapWindow;
        let period = match window {
            TwapWindow::OneHour => pallet::TwapPeriod::OneHour,
            TwapWindow::OneDay => pallet::TwapPeriod::OneDay,
            TwapWindow::OneWeek => pallet::TwapPeriod::OneWeek,
        };
        Self::calculate_twap(period)
    }

    fn get_last_trade_price() -> Option<u64> {
        pallet::LastTradePrice::<T>::get()
    }

    fn is_price_stale(max_age_blocks: u32) -> bool {
        use sp_runtime::SaturatedConversion;
        match pallet::TwapAccumulatorStore::<T>::get() {
            Some(acc) => {
                let now: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                now.saturating_sub(acc.current_block) > max_age_blocks
            }
            None => true,
        }
    }

    fn get_trade_count() -> u64 {
        pallet::TwapAccumulatorStore::<T>::get()
            .map(|acc| acc.trade_count)
            .unwrap_or(0)
    }
}

// ==================== 公共查询接口 ====================

impl<T: Config> Pallet<T> {
    /// 获取卖单列表（过滤已过期订单，与撮合逻辑一致）
    pub fn get_sell_order_list() -> Vec<Order<T>> {
        let now = <frame_system::Pallet<T>>::block_number();
        SellOrders::<T>::get().iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                && now <= o.expires_at
            })
            .collect()
    }

    /// 获取买单列表（过滤已过期订单，与撮合逻辑一致）
    pub fn get_buy_order_list() -> Vec<Order<T>> {
        let now = <frame_system::Pallet<T>>::block_number();
        BuyOrders::<T>::get().iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                && now <= o.expires_at
            })
            .collect()
    }

    /// 获取用户订单
    pub fn get_user_order_list(user: &T::AccountId) -> Vec<Order<T>> {
        UserOrders::<T>::get(user).iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .collect()
    }

    /// 获取最优价格
    pub fn get_best_prices() -> (Option<u64>, Option<u64>) {
        (BestAsk::<T>::get(), BestBid::<T>::get())
    }
}
