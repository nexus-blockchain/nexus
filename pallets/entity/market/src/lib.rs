//! # 实体代币交易市场模块 (pallet-entity-market)
//!
//! ## 概述
//!
//! 本模块实现实体代币的 P2P 交易市场，支持：
//! - NEX 通道：使用原生 NEX 代币买卖实体代币（链上即时结算）
//! - USDT 通道：使用 TRC20 USDT 买卖实体代币（需 OCW 验证）
//!
//! ## 交易模式
//!
//! - 限价单：挂单等待撮合
//! - 吃单：直接成交对手盘订单
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-02-01): 初始版本，实现 NEX 通道限价单
//! - v0.2.0 (2026-02-01): Phase 2，实现 USDT 通道 + OCW 验证

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod ocw;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityProvider, EntityTokenProvider};
    use sp_runtime::traits::{CheckedAdd, CheckedMul, CheckedSub, Saturating, Zero};
    use sp_runtime::SaturatedConversion;
    use sp_runtime::transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
    };

    /// Balance 类型别名
    pub type BalanceOf<T> = <T as Config>::Balance;

    // ==================== 数据结构 ====================

    /// 订单方向
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum OrderSide {
        /// 买单（用 NEX 买 Token）
        Buy,
        /// 卖单（卖 Token 得 NEX）
        Sell,
    }

    /// 支付通道
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum PaymentChannel {
        /// 原生 NEX
        NEX,
        /// TRC20 USDT（Phase 2）
        USDT,
    }

    /// 订单类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum OrderType {
        /// 限价单（挂单等待撮合）
        #[default]
        Limit,
        /// 市价单（立即以最优价成交）
        Market,
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
        /// 已过期
        Expired,
    }

    // USDT 交易共享类型（从 pallet-trading-common 导入）
    pub use pallet_trading_common::{
        UsdtTradeStatus, BuyerDepositStatus, PaymentVerificationResult,
    };

    /// TRON 地址类型（34 字节 Base58）
    pub type TronAddress = pallet_trading_common::TronAddress;

    /// TRON 交易哈希类型（64 字节 hex）
    pub type TronTxHash = pallet_trading_common::TronTxHash;

    /// 交易订单
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct TradeOrder<T: Config> {
        /// 订单 ID
        pub order_id: u64,
        /// 实体 ID
        pub entity_id: u64,
        /// 挂单者
        pub maker: T::AccountId,
        /// 订单方向
        pub side: OrderSide,
        /// 订单类型
        pub order_type: OrderType,
        /// 支付通道
        pub channel: PaymentChannel,
        /// 代币数量（总量）
        pub token_amount: T::TokenBalance,
        /// 已成交数量
        pub filled_amount: T::TokenBalance,
        /// 价格（NEX 通道：每个 Token 的 NEX 价格；USDT 通道：每个 Token 的 USDT 价格，精度 10^6）
        /// 市价单时为 0
        pub price: BalanceOf<T>,
        /// USDT 价格（仅 USDT 通道使用，精度 10^6）
        pub usdt_price: u64,
        /// TRON 收款地址（仅 USDT 卖单使用）
        pub tron_address: Option<TronAddress>,
        /// 订单状态
        pub status: OrderStatus,
        /// 创建区块
        pub created_at: BlockNumberFor<T>,
        /// 过期区块
        pub expires_at: BlockNumberFor<T>,
        /// 挂单保证金（USDT 买单锁定的 NEX 保证金，其他订单为 0）
        pub maker_deposit: BalanceOf<T>,
    }

    /// USDT 交易记录（等待验证）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct UsdtTrade<T: Config> {
        /// 交易 ID
        pub trade_id: u64,
        /// 关联订单 ID
        pub order_id: u64,
        /// 实体 ID
        pub entity_id: u64,
        /// 卖家
        pub seller: T::AccountId,
        /// 买家
        pub buyer: T::AccountId,
        /// 代币数量
        pub token_amount: T::TokenBalance,
        /// USDT 金额（精度 10^6）
        pub usdt_amount: u64,
        /// 卖家 TRON 收款地址
        pub seller_tron_address: TronAddress,
        /// 买家提交的 TRON 交易哈希
        pub tron_tx_hash: Option<TronTxHash>,
        /// 交易状态
        pub status: UsdtTradeStatus,
        /// 创建区块
        pub created_at: BlockNumberFor<T>,
        /// 超时区块
        pub timeout_at: BlockNumberFor<T>,
        /// 🆕 买家保证金金额（NEX）
        pub buyer_deposit: BalanceOf<T>,
        /// 🆕 保证金状态
        pub deposit_status: BuyerDepositStatus,
        /// 首次检测到少付的区块
        pub first_verified_at: Option<BlockNumberFor<T>>,
        /// 首次检测到的实际金额
        pub first_actual_amount: Option<u64>,
        /// 补付窗口截止区块
        pub underpaid_deadline: Option<BlockNumberFor<T>>,
    }

    /// 实体市场配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MarketConfig<Balance> {
        /// 是否启用 NEX 交易
        pub cos_enabled: bool,
        /// 是否启用 USDT 交易
        pub usdt_enabled: bool,
        /// 交易手续费率（基点，100 = 1%）
        pub fee_rate: u16,
        /// 最小订单 Token 数量
        pub min_order_amount: u128,
        /// 订单有效期（区块数）
        pub order_ttl: u32,
        /// USDT 交易超时（区块数）
        pub usdt_timeout: u32,
        /// 手续费接收账户（None = 实体金库账户）
        pub fee_recipient: Option<Balance>,
    }

    /// 市场统计数据
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MarketStats {
        /// 总订单数
        pub total_orders: u64,
        /// 总成交数
        pub total_trades: u64,
        /// NEX 总交易量
        pub total_volume_nex: u128,
        /// USDT 总交易量（精度 10^6）
        pub total_volume_usdt: u64,
        /// 总手续费（NEX）
        pub total_fees_cos: u128,
        /// 总手续费（USDT，精度 10^6）
        pub total_fees_usdt: u64,
    }

    /// 市场操作类型（用于失败事件追踪）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum MarketOperation {
        /// 保证金没收转国库
        DepositForfeit,
        /// 验证奖励发放
        VerificationReward,
        /// 队列 push 失败（交易可能脱离跟踪队列）
        QueuePushFailed,
    }

    // ==================== Phase 4: 订单簿深度数据结构 ====================

    /// 价格档位（聚合同一价格的订单）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct PriceLevel<Balance, TokenBalance> {
        /// 价格
        pub price: Balance,
        /// 该价格的总数量
        pub total_amount: TokenBalance,
        /// 订单数量
        pub order_count: u32,
    }

    /// 订单簿深度（买卖盘）
    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
    pub struct OrderBookDepth<Balance, TokenBalance> {
        /// 实体 ID
        pub entity_id: u64,
        /// 卖盘（按价格升序，最优卖价在前）
        pub asks: Vec<PriceLevel<Balance, TokenBalance>>,
        /// 买盘（按价格降序，最优买价在前）
        pub bids: Vec<PriceLevel<Balance, TokenBalance>>,
        /// 最优卖价
        pub best_ask: Option<Balance>,
        /// 最优买价
        pub best_bid: Option<Balance>,
        /// 买卖价差
        pub spread: Option<Balance>,
        /// 快照区块
        pub block_number: u32,
    }

    /// 市场摘要
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MarketSummary<Balance, TokenBalance> {
        /// 最优卖价
        pub best_ask: Option<Balance>,
        /// 最优买价
        pub best_bid: Option<Balance>,
        /// 24h 最高价
        pub high_24h: Balance,
        /// 24h 最低价
        pub low_24h: Balance,
        /// 24h 成交量
        pub volume_24h: TokenBalance,
        /// 最新成交价
        pub last_price: Option<Balance>,
        /// 卖单总量
        pub total_ask_amount: TokenBalance,
        /// 买单总量
        pub total_bid_amount: TokenBalance,
    }

    // ==================== Phase 5: TWAP 价格预言机数据结构 ====================

    /// 价格快照（用于 TWAP 计算）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct PriceSnapshot {
        /// 累积价格 (price × blocks)
        pub cumulative_price: u128,
        /// 快照区块号
        pub block_number: u32,
    }

    /// TWAP 累积器（三周期：1小时、24小时、7天）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct TwapAccumulator<Balance> {
        /// 当前累积价格
        pub current_cumulative: u128,
        /// 当前区块号
        pub current_block: u32,
        /// 最新成交价
        pub last_price: Balance,
        /// 总成交次数（用于判断市场活跃度）
        pub trade_count: u64,

        /// 1小时前快照（用于 1小时 TWAP）
        pub hour_snapshot: PriceSnapshot,
        /// 24小时前快照（用于 24小时 TWAP）
        pub day_snapshot: PriceSnapshot,
        /// 7天前快照（用于 7天 TWAP）
        pub week_snapshot: PriceSnapshot,

        /// 上次更新 1小时快照的区块
        pub last_hour_update: u32,
        /// 上次更新 24小时快照的区块
        pub last_day_update: u32,
        /// 上次更新 7天快照的区块
        pub last_week_update: u32,
    }

    /// TWAP 周期类型
    #[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum TwapPeriod {
        /// 1小时（600 区块，假设 6秒/区块）
        OneHour,
        /// 24小时（14400 区块）
        OneDay,
        /// 7天（100800 区块）
        OneWeek,
    }

    /// 价格保护配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct PriceProtectionConfig<Balance> {
        /// 是否启用价格保护
        pub enabled: bool,
        /// 限价单最大价格偏离（基点，1000 = 10%）
        pub max_price_deviation: u16,
        /// 市价单最大滑点（基点，500 = 5%）
        pub max_slippage: u16,
        /// 熔断触发阈值（基点，5000 = 50%）
        pub circuit_breaker_threshold: u16,
        /// 启用 TWAP 保护的最小成交数
        pub min_trades_for_twap: u64,
        /// 市场是否处于熔断状态
        pub circuit_breaker_active: bool,
        /// 熔断结束区块
        pub circuit_breaker_until: u32,
        /// 实体主设定的初始参考价格（用于 TWAP 冷启动）
        pub initial_price: Option<Balance>,
    }

    impl<Balance: Default> Default for PriceProtectionConfig<Balance> {
        fn default() -> Self {
            Self {
                enabled: true,
                max_price_deviation: 2000,        // 20%
                max_slippage: 500,                // 5%
                circuit_breaker_threshold: 5000,  // 50%
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
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 原生货币（NEX）
        type Currency: Currency<Self::AccountId, Balance = Self::Balance> + ReservableCurrency<Self::AccountId>;

        /// Balance 类型（需要支持 u128 转换）
        type Balance: Member
            + Parameter
            + Copy
            + Default
            + MaxEncodedLen
            + From<u128>
            + Into<u128>
            + From<u32>
            + From<u16>
            + Saturating
            + Zero
            + Ord
            + sp_runtime::traits::CheckedDiv;

        /// 实体代币余额类型
        type TokenBalance: Member
            + Parameter
            + Copy
            + Default
            + MaxEncodedLen
            + From<u128>
            + Into<u128>
            + CheckedAdd
            + CheckedSub
            + CheckedMul
            + Saturating
            + Zero
            + Ord;

        /// 实体查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 实体代币接口
        type TokenProvider: EntityTokenProvider<Self::AccountId, Self::TokenBalance>;

        /// 默认订单有效期（区块数）
        #[pallet::constant]
        type DefaultOrderTTL: Get<u32>;

        /// 最大活跃订单数（每用户每实体）
        #[pallet::constant]
        type MaxActiveOrdersPerUser: Get<u32>;

        /// 默认手续费率（基点）
        #[pallet::constant]
        type DefaultFeeRate: Get<u16>;

        /// USDT 交易默认超时（区块数）
        #[pallet::constant]
        type DefaultUsdtTimeout: Get<u32>;

        /// 1小时对应的区块数（默认 600，假设 6秒/区块）
        #[pallet::constant]
        type BlocksPerHour: Get<u32>;

        /// 24小时对应的区块数（默认 14400）
        #[pallet::constant]
        type BlocksPerDay: Get<u32>;

        /// 7天对应的区块数（默认 100800）
        #[pallet::constant]
        type BlocksPerWeek: Get<u32>;

        /// 熔断持续时间（区块数，默认 600 = 1小时）
        #[pallet::constant]
        type CircuitBreakerDuration: Get<u32>;

        /// 验证确认奖励（激励任何人调用 claim_verification_reward）
        /// 默认 0.1 NEX
        #[pallet::constant]
        type VerificationReward: Get<BalanceOf<Self>>;

        /// 奖励来源账户（通常是实体账户或财库）
        type RewardSource: Get<Self::AccountId>;

        // ==================== 🆕 买家保证金配置 ====================

        /// 买家保证金比例（bps，1000 = 10%）
        /// USDT 金额 × 此比例 = 需锁定的 NEX 保证金
        #[pallet::constant]
        type BuyerDepositRate: Get<u16>;

        /// 最低买家保证金金额（NEX）
        /// 保证金 = max(MinBuyerDeposit, usdt_amount × BuyerDepositRate)
        #[pallet::constant]
        type MinBuyerDeposit: Get<BalanceOf<Self>>;

        /// 保证金没收比例（bps，10000 = 100%）
        /// 超时时没收的保证金比例，剩余退还买家
        #[pallet::constant]
        type DepositForfeitRate: Get<u16>;

        /// USDT 转 NEX 价格（精度 10^6，用于保证金计算）
        /// 例如：100_000 表示 1 USDT = 0.1 NEX
        /// 实际应从 pricing 模块获取，这里简化为常量
        #[pallet::constant]
        type UsdtToNexRate: Get<u64>;

        /// 🆕 国库账户（没收的保证金归入国库）
        type TreasuryAccount: Get<Self::AccountId>;

        /// AwaitingVerification 超时宽限期（区块数）
        #[pallet::constant]
        type VerificationGracePeriod: Get<u32>;

        /// 少付补付窗口（区块数）
        #[pallet::constant]
        type UnderpaidGracePeriod: Get<u32>;

        /// NEX/USDT 价格查询（用于代币 USDT 间接换算）
        type NexUsdtPrice: pallet_entity_common::PricingProvider;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// OCW: 自动验证待处理的 USDT 交易
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            log::info!(target: "entity-market-ocw", 
                "Running offchain worker at block {:?}", block_number);

            // 获取待验证队列
            let pending = PendingUsdtTrades::<T>::get();
            
            if pending.is_empty() {
                return;
            }

            log::info!(target: "entity-market-ocw", 
                "Processing {} pending USDT trades", pending.len());

            for trade_id in pending.iter() {
                if let Some(trade) = UsdtTrades::<T>::get(trade_id) {
                    if trade.status == UsdtTradeStatus::AwaitingVerification {
                        if let Some(ref tx_hash) = trade.tron_tx_hash {
                            Self::process_verification(*trade_id, &trade, tx_hash.as_slice());
                        }
                    }
                }
            }

            // ── 2. 扫描 UnderpaidPending（补付窗口内持续检查新转账）──
            let underpaid = PendingUnderpaidTrades::<T>::get();
            if !underpaid.is_empty() {
                log::info!(target: "entity-market-ocw",
                    "Checking {} underpaid trades for topup", underpaid.len());

                for trade_id in underpaid.iter() {
                    if let Some(trade) = UsdtTrades::<T>::get(trade_id) {
                        if trade.status == UsdtTradeStatus::UnderpaidPending {
                            if let Some(ref tx_hash) = trade.tron_tx_hash {
                                Self::check_underpaid_topup(
                                    *trade_id, &trade, tx_hash.as_slice(),
                                );
                            }
                        }
                    }
                }
            }
        }

        /// P1 修复: on_idle 批量清理过期订单，释放 BoundedVec 名额
        fn on_idle(_n: BlockNumberFor<T>, mut remaining_weight: Weight) -> Weight {
            let base_weight = Weight::from_parts(5_000, 0);
            let per_order_weight = Weight::from_parts(30_000, 0);
            let now = <frame_system::Pallet<T>>::block_number();
            let mut cleaned = 0u32;
            const MAX_CLEAN_PER_BLOCK: u32 = 20;

            let next_id = NextOrderId::<T>::get();
            let start = next_id.saturating_sub(1000); // 最多回溯 1000 个

            for order_id in start..next_id {
                if cleaned >= MAX_CLEAN_PER_BLOCK { break; }
                if remaining_weight.ref_time() < per_order_weight.ref_time() { break; }

                if let Some(order) = Orders::<T>::get(order_id) {
                    if (order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled)
                        && now > order.expires_at
                    {
                        let unfilled = order.token_amount.saturating_sub(order.filled_amount);
                        match order.side {
                            OrderSide::Sell => {
                                T::TokenProvider::unreserve(order.entity_id, &order.maker, unfilled);
                            }
                            OrderSide::Buy => {
                                if let Ok(refund) = Self::calculate_total_next(unfilled.into(), order.price) {
                                    T::Currency::unreserve(&order.maker, refund);
                                }
                            }
                        }
                        if !order.maker_deposit.is_zero() {
                            T::Currency::unreserve(&order.maker, order.maker_deposit);
                        }

                        let mut expired_order = order.clone();
                        expired_order.status = OrderStatus::Expired;
                        Orders::<T>::insert(order_id, &expired_order);

                        Self::remove_from_order_book(order.entity_id, order_id, order.side);
                        UserOrders::<T>::mutate(&order.maker, |orders| {
                            orders.retain(|&id| id != order_id);
                        });

                        cleaned += 1;
                        remaining_weight = remaining_weight.saturating_sub(per_order_weight);
                    }
                }
                remaining_weight = remaining_weight.saturating_sub(base_weight);
            }

            Weight::from_parts(base_weight.ref_time() * (cleaned as u64 + 1), 0)
        }
    }

    // ==================== 存储项 ====================

    /// 下一个订单 ID
    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub type NextOrderId<T> = StorageValue<_, u64, ValueQuery>;

    /// 订单存储
    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, u64, TradeOrder<T>>;

    /// 实体订单簿 - 卖单（按实体索引）
    #[pallet::storage]
    #[pallet::getter(fn entity_sell_orders)]
    pub type EntitySellOrders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        BoundedVec<u64, ConstU32<1000>>,
        ValueQuery,
    >;

    /// 实体订单簿 - 买单（按实体索引）
    #[pallet::storage]
    #[pallet::getter(fn entity_buy_orders)]
    pub type EntityBuyOrders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        BoundedVec<u64, ConstU32<1000>>,
        ValueQuery,
    >;

    /// 用户订单（按用户索引）
    #[pallet::storage]
    #[pallet::getter(fn user_orders)]
    pub type UserOrders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<100>>,
        ValueQuery,
    >;

    /// 实体市场配置
    #[pallet::storage]
    #[pallet::getter(fn market_configs)]
    pub type MarketConfigs<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, MarketConfig<BalanceOf<T>>>;

    /// 实体市场统计
    #[pallet::storage]
    #[pallet::getter(fn market_stats)]
    pub type MarketStatsStorage<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, MarketStats, ValueQuery>;

    /// 下一个 USDT 交易 ID
    #[pallet::storage]
    #[pallet::getter(fn next_usdt_trade_id)]
    pub type NextUsdtTradeId<T> = StorageValue<_, u64, ValueQuery>;

    /// USDT 交易记录存储
    #[pallet::storage]
    #[pallet::getter(fn usdt_trades)]
    pub type UsdtTrades<T: Config> = StorageMap<_, Blake2_128Concat, u64, UsdtTrade<T>>;

    /// 待验证的 USDT 交易列表（供 OCW 使用）
    #[pallet::storage]
    #[pallet::getter(fn pending_usdt_trades)]
    pub type PendingUsdtTrades<T: Config> = StorageValue<_, BoundedVec<u64, ConstU32<100>>, ValueQuery>;

    /// 少付补付跟踪队列（UnderpaidPending 状态，OCW 持续扫描）
    #[pallet::storage]
    #[pallet::getter(fn pending_underpaid_trades)]
    pub type PendingUnderpaidTrades<T: Config> = StorageValue<_, BoundedVec<u64, ConstU32<100>>, ValueQuery>;

    /// OCW 验证结果（链上存储，用于 claim_verification_reward）
    /// 
    /// ## 安全说明
    /// - OCW 通过 submit_ocw_result 提交验证结果
    /// - claim_verification_reward 必须匹配此存储的结果
    /// - 防止伪造验证结果
    /// 
    /// 🆕 存储格式: (PaymentVerificationResult, actual_amount)
    #[pallet::storage]
    #[pallet::getter(fn ocw_verification_results)]
    pub type OcwVerificationResults<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // trade_id
        (PaymentVerificationResult, u64),  // 🆕 (验证结果, 实际金额)
        OptionQuery,
    >;

    // ==================== Phase 4: 订单簿深度存储 ====================

    /// 实体最优卖价
    #[pallet::storage]
    #[pallet::getter(fn best_ask)]
    pub type BestAsk<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>>;

    /// 实体最优买价
    #[pallet::storage]
    #[pallet::getter(fn best_bid)]
    pub type BestBid<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>>;

    /// 实体最新成交价
    #[pallet::storage]
    #[pallet::getter(fn last_trade_price)]
    pub type LastTradePrice<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>>;

    /// 实体市场摘要
    #[pallet::storage]
    #[pallet::getter(fn market_summary)]
    pub type MarketSummaryStorage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        MarketSummary<BalanceOf<T>, T::TokenBalance>,
    >;

    // ==================== Phase 5: TWAP 价格预言机存储 ====================

    /// TWAP 累积器（每个实体一个）
    #[pallet::storage]
    #[pallet::getter(fn twap_accumulator)]
    pub type TwapAccumulators<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        TwapAccumulator<BalanceOf<T>>,
    >;

    /// 价格保护配置（每个实体一个）
    #[pallet::storage]
    #[pallet::getter(fn price_protection)]
    pub type PriceProtection<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        PriceProtectionConfig<BalanceOf<T>>,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 订单已创建
        OrderCreated {
            order_id: u64,
            entity_id: u64,
            maker: T::AccountId,
            side: OrderSide,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        },
        /// 订单已成交（部分或全部）
        OrderFilled {
            order_id: u64,
            taker: T::AccountId,
            filled_amount: T::TokenBalance,
            total_next: BalanceOf<T>,
            fee: BalanceOf<T>,
        },
        /// 订单已取消
        OrderCancelled { order_id: u64 },
        /// 市场配置已更新
        MarketConfigured { entity_id: u64 },
        /// USDT 卖单已创建
        UsdtSellOrderCreated {
            order_id: u64,
            entity_id: u64,
            maker: T::AccountId,
            token_amount: T::TokenBalance,
            usdt_price: u64,
            tron_address: TronAddress,
        },
        /// USDT 买单已创建
        UsdtBuyOrderCreated {
            order_id: u64,
            entity_id: u64,
            maker: T::AccountId,
            token_amount: T::TokenBalance,
            usdt_price: u64,
        },
        /// USDT 交易已创建（等待支付）
        UsdtTradeCreated {
            trade_id: u64,
            order_id: u64,
            seller: T::AccountId,
            buyer: T::AccountId,
            token_amount: T::TokenBalance,
            usdt_amount: u64,
        },
        /// USDT 支付已提交（等待验证）
        UsdtPaymentSubmitted {
            trade_id: u64,
            tron_tx_hash: TronTxHash,
        },
        /// USDT 交易已完成（OCW 验证通过）
        UsdtTradeCompleted {
            trade_id: u64,
            order_id: u64,
        },
        /// USDT 交易验证失败
        UsdtTradeVerificationFailed {
            trade_id: u64,
            reason: Vec<u8>,
        },
        /// USDT 交易已超时退款
        UsdtTradeRefunded {
            trade_id: u64,
        },
        /// 市价单已执行
        MarketOrderExecuted {
            entity_id: u64,
            trader: T::AccountId,
            side: OrderSide,
            filled_amount: T::TokenBalance,
            total_next: BalanceOf<T>,
            total_fee: BalanceOf<T>,
        },
        /// TWAP 价格已更新
        TwapUpdated {
            entity_id: u64,
            new_price: BalanceOf<T>,
            twap_1h: Option<BalanceOf<T>>,
            twap_24h: Option<BalanceOf<T>>,
            twap_7d: Option<BalanceOf<T>>,
        },
        /// 熔断已触发
        CircuitBreakerTriggered {
            entity_id: u64,
            current_price: BalanceOf<T>,
            twap_7d: BalanceOf<T>,
            deviation_bps: u16,
            until_block: u32,
        },
        /// 熔断已解除
        CircuitBreakerLifted {
            entity_id: u64,
        },
        /// 价格保护配置已更新
        PriceProtectionConfigured {
            entity_id: u64,
            enabled: bool,
            max_deviation: u16,
            max_slippage: u16,
        },
        /// 初始价格已设置
        InitialPriceSet {
            entity_id: u64,
            initial_price: BalanceOf<T>,
        },
        /// 验证奖励已领取
        VerificationRewardClaimed {
            trade_id: u64,
            claimer: T::AccountId,
            reward: BalanceOf<T>,
        },
        /// OCW 验证结果已提交（🆕 多档判定）
        OcwResultSubmitted {
            trade_id: u64,
            verification_result: PaymentVerificationResult,
            actual_amount: u64,
        },
        /// 🆕 少付自动处理（按比例释放）
        UnderpaidAutoProcessed {
            trade_id: u64,
            expected_amount: u64,
            actual_amount: u64,
            payment_ratio: u32,  // 实际付款比例 (bps)
            token_released: T::TokenBalance,
            deposit_forfeited: BalanceOf<T>,
        },
        /// 🆕 买家保证金已锁定
        BuyerDepositLocked {
            trade_id: u64,
            buyer: T::AccountId,
            deposit: BalanceOf<T>,
        },
        /// 🆕 买家保证金已退还
        BuyerDepositReleased {
            trade_id: u64,
            buyer: T::AccountId,
            deposit: BalanceOf<T>,
        },
        /// 🆕 买家保证金已没收（归入国库）
        BuyerDepositForfeited {
            trade_id: u64,
            buyer: T::AccountId,
            forfeited: BalanceOf<T>,
            to_treasury: BalanceOf<T>,
        },
        /// 市场操作失败（需人工干预）
        MarketOperationFailed { trade_id: u64, operation: MarketOperation },
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
        /// AwaitingVerification 超时退款（宽限期后仍无结果）
        VerificationTimeoutRefunded {
            trade_id: u64,
            buyer: T::AccountId,
            seller: T::AccountId,
            usdt_amount: u64,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 实体不存在
        EntityNotFound,
        /// 不是实体所有者
        NotEntityOwner,
        /// 实体代币未启用
        TokenNotEnabled,
        /// 市场未启用
        MarketNotEnabled,
        /// 订单不存在
        OrderNotFound,
        /// 不是订单所有者
        NotOrderOwner,
        /// 订单已关闭
        OrderClosed,
        /// 余额不足
        InsufficientBalance,
        /// Token 余额不足
        InsufficientTokenBalance,
        /// 数量过小
        AmountTooSmall,
        /// 数量超过可用
        AmountExceedsAvailable,
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
        /// USDT 市场未启用
        UsdtMarketNotEnabled,
        /// 无效的 TRON 地址
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
        /// 无效的交易哈希
        InvalidTxHash,
        /// 通道不匹配
        ChannelMismatch,
        /// 没有可用订单
        NoOrdersAvailable,
        /// 滑点超限
        SlippageExceeded,
        /// 价格偏离 TWAP 过大
        PriceDeviationTooHigh,
        /// 市场处于熔断状态
        MarketCircuitBreakerActive,
        /// OCW 验证结果不存在
        OcwResultNotFound,
        /// TWAP 数据不足
        InsufficientTwapData,
        /// 🆕 买家保证金余额不足
        InsufficientDepositBalance,
        /// 手续费率无效（超过 5000 bps = 50%）
        InvalidFeeRate,
        /// 基点参数无效（超过 10000）
        InvalidBasisPoints,
        /// 仍在验证宽限期内，不允许超时
        StillInGracePeriod,
        /// 补付窗口尚未到期
        UnderpaidGraceNotExpired,
        /// 交易不在 UnderpaidPending 状态
        NotUnderpaidPending,
        /// 实体未激活（Banned/Closed）
        EntityNotActive,
        /// 订单 TTL 过短
        OrderTtlTooShort,
        /// USDT 超时时间过短
        UsdtTimeoutTooShort,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 挂卖单（卖 Token 得 NEX）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `token_amount`: 出售的 Token 数量
        /// - `price`: 每个 Token 的 NEX 价格
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn place_sell_order(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体和市场
            Self::ensure_market_enabled(entity_id)?;

            // 验证参数
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // Phase 5: 价格偏离检查
            Self::check_price_deviation(entity_id, price)?;

            // 检查用户 Token 余额
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            ensure!(balance >= token_amount, Error::<T>::InsufficientTokenBalance);

            // 锁定 Token
            T::TokenProvider::reserve(entity_id, &who, token_amount)?;

            // P0 修复: 自动撮合价格交叉的买单
            let (crossed, _nex_received, _fees) = Self::do_cross_match(
                &who, entity_id, OrderSide::Sell, price, token_amount,
            )?;
            let remaining = token_amount.saturating_sub(crossed);

            // 剩余部分挂为限价卖单
            if !remaining.is_zero() {
                let order_id = Self::do_create_order(
                    entity_id,
                    who.clone(),
                    OrderSide::Sell,
                    OrderType::Limit,
                    PaymentChannel::NEX,
                    remaining,
                    price,
                    0,
                    None,
                    Zero::zero(),
                )?;

                Self::deposit_event(Event::OrderCreated {
                    order_id,
                    entity_id,
                    maker: who,
                    side: OrderSide::Sell,
                    token_amount: remaining,
                    price,
                });
            }

            // 更新最优价格
            Self::update_best_prices(entity_id);

            Ok(())
        }

        /// 挂买单（用 NEX 买 Token）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `token_amount`: 想购买的 Token 数量
        /// - `price`: 每个 Token 愿意支付的 NEX 价格
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 5_000))]
        pub fn place_buy_order(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体和市场
            Self::ensure_market_enabled(entity_id)?;

            // 验证参数
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // Phase 5: 价格偏离检查
            Self::check_price_deviation(entity_id, price)?;

            // 计算需要锁定的 NEX 总量
            let token_u128: u128 = token_amount.into();
            let total_next = Self::calculate_total_next(token_u128, price)?;

            // 锁定 NEX
            T::Currency::reserve(&who, total_next).map_err(|_| Error::<T>::InsufficientBalance)?;

            // P0 修复: 自动撮合价格交叉的卖单
            let (crossed, nex_spent, _fees) = Self::do_cross_match(
                &who, entity_id, OrderSide::Buy, price, token_amount,
            )?;
            let remaining = token_amount.saturating_sub(crossed);

            // 释放因价格改善节省的多余 NEX
            // nex_spent 是以卖方价格成交的实际支出（<= crossed * buy_price）
            // 剩余挂单需锁定 remaining * buy_price
            let remaining_u128: u128 = remaining.into();
            let nex_for_remaining = Self::calculate_total_next(remaining_u128, price)?;
            let excess = total_next.saturating_sub(nex_spent).saturating_sub(nex_for_remaining);
            if !excess.is_zero() {
                T::Currency::unreserve(&who, excess);
            }

            // 剩余部分挂为限价买单
            if !remaining.is_zero() {
                let order_id = Self::do_create_order(
                    entity_id,
                    who.clone(),
                    OrderSide::Buy,
                    OrderType::Limit,
                    PaymentChannel::NEX,
                    remaining,
                    price,
                    0,
                    None,
                    Zero::zero(),
                )?;

                Self::deposit_event(Event::OrderCreated {
                    order_id,
                    entity_id,
                    maker: who,
                    side: OrderSide::Buy,
                    token_amount: remaining,
                    price,
                });
            } else if !excess.is_zero() || !nex_spent.is_zero() {
                // 全部交叉撮合完成，多余 NEX 已退还
            }

            // 更新最优价格
            Self::update_best_prices(entity_id);

            Ok(())
        }

        /// 吃单（成交对手盘订单）
        ///
        /// # 参数
        /// - `order_id`: 要吃的订单 ID
        /// - `amount`: 成交数量（None = 全部）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(80_000_000, 8_000))]
        pub fn take_order(
            origin: OriginFor<T>,
            order_id: u64,
            amount: Option<T::TokenBalance>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取订单
            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

            // 验证订单状态
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );
            // H2 审计修复: 过期订单不可吃单（on_idle 清理可能滞后）
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= order.expires_at, Error::<T>::OrderClosed);

            // 不能吃自己的单
            ensure!(order.maker != who, Error::<T>::CannotTakeOwnOrder);

            // 计算可成交数量
            let available = order
                .token_amount
                .checked_sub(&order.filled_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            let fill_amount = amount.unwrap_or(available).min(available);
            ensure!(!fill_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 计算成交金额
            let fill_u128: u128 = fill_amount.into();
            let total_next = Self::calculate_total_next(fill_u128, order.price)?;

            // 计算手续费
            let fee_rate = Self::get_fee_rate(order.entity_id);
            let fee = Self::calculate_fee(total_next, fee_rate);

            // P2 修复: 手续费买卖双方对称承担
            // fee 从交易额 total_next 中扣除:
            //   seller 净收入 = total_next - fee (seller 承担 fee 的一半体现在少收)
            //   buyer 净支出 = total_next      (buyer 承担 fee 的一半体现在多付)
            //   entity_owner 收入 = fee
            // 两侧逻辑对称: buyer 付 total_next, seller 收 total_next - fee, fee 给 owner
            let net_amount = total_next.saturating_sub(fee);

            // 执行交易
            match order.side {
                OrderSide::Sell => {
                    // 卖单：taker(买方) 支付 NEX，获得 Token
                    T::Currency::transfer(
                        &who, &order.maker, net_amount,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    if !fee.is_zero() {
                        if let Some(ref entity_owner) = T::EntityProvider::entity_owner(order.entity_id) {
                            T::Currency::transfer(
                                &who, entity_owner, fee,
                                ExistenceRequirement::KeepAlive,
                            )?;
                        }
                    }

                    // Token: maker(卖方) → taker(买方)
                    T::TokenProvider::repatriate_reserved(
                        order.entity_id, &order.maker, &who, fill_amount,
                    )?;
                }
                OrderSide::Buy => {
                    // 买单：taker(卖方) 提供 Token，获得 NEX
                    let taker_balance = T::TokenProvider::token_balance(order.entity_id, &who);
                    ensure!(
                        taker_balance >= fill_amount,
                        Error::<T>::InsufficientTokenBalance
                    );

                    // P2 修复: 使用 repatriate_reserved 替代 unreserve→transfer
                    T::Currency::repatriate_reserved(
                        &order.maker, &who, net_amount,
                        frame_support::traits::BalanceStatus::Free,
                    )?;
                    if !fee.is_zero() {
                        if let Some(ref entity_owner) = T::EntityProvider::entity_owner(order.entity_id) {
                            T::Currency::repatriate_reserved(
                                &order.maker, entity_owner, fee,
                                frame_support::traits::BalanceStatus::Free,
                            )?;
                        }
                    }

                    // Token: taker(卖方) → maker(买方)
                    T::TokenProvider::reserve(order.entity_id, &who, fill_amount)?;
                    T::TokenProvider::repatriate_reserved(
                        order.entity_id, &who, &order.maker, fill_amount,
                    )?;
                }
            }

            // 更新订单状态
            order.filled_amount = order
                .filled_amount
                .checked_add(&fill_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            if order.filled_amount >= order.token_amount {
                order.status = OrderStatus::Filled;
                // 从订单簿移除
                Self::remove_from_order_book(order.entity_id, order_id, order.side);
                // M2: 从用户订单列表移除
                UserOrders::<T>::mutate(&order.maker, |orders| {
                    orders.retain(|&id| id != order_id);
                });
            } else {
                order.status = OrderStatus::PartiallyFilled;
            }

            Orders::<T>::insert(order_id, &order);

            // 更新统计
            MarketStatsStorage::<T>::mutate(order.entity_id, |stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_nex = stats.total_volume_nex.saturating_add(total_next.into());
                stats.total_fees_cos = stats.total_fees_cos.saturating_add(fee.into());
            });

            // 更新最优价格和 TWAP
            Self::update_best_prices(order.entity_id);
            Self::on_trade_completed(order.entity_id, order.price);

            Self::deposit_event(Event::OrderFilled {
                order_id,
                taker: who,
                filled_amount: fill_amount,
                total_next,
                fee,
            });

            Ok(())
        }

        /// 取消订单
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(40_000_000, 5_000))]
        pub fn cancel_order(origin: OriginFor<T>, order_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

            // 验证所有权
            ensure!(order.maker == who, Error::<T>::NotOrderOwner);

            // 验证状态
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );

            // 计算未成交数量
            let unfilled = order
                .token_amount
                .checked_sub(&order.filled_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            // 退还锁定资产
            match order.side {
                OrderSide::Sell => {
                    // 退还锁定的 Token
                    T::TokenProvider::unreserve(order.entity_id, &who, unfilled);
                }
                OrderSide::Buy => {
                    // 退还锁定的 NEX
                    let unfilled_u128: u128 = unfilled.into();
                    let refund = Self::calculate_total_next(unfilled_u128, order.price)?;
                    T::Currency::unreserve(&who, refund);
                }
            }

            // P0 修复: 退还 USDT 买单的 maker_deposit 保证金
            if !order.maker_deposit.is_zero() {
                T::Currency::unreserve(&who, order.maker_deposit);
            }

            // 更新订单状态
            order.status = OrderStatus::Cancelled;
            Orders::<T>::insert(order_id, &order);

            // 从订单簿移除
            Self::remove_from_order_book(order.entity_id, order_id, order.side);

            // M2: 从用户订单列表移除
            UserOrders::<T>::mutate(&who, |orders| {
                orders.retain(|&id| id != order_id);
            });

            // 更新最优价格
            Self::update_best_prices(order.entity_id);

            Self::deposit_event(Event::OrderCancelled { order_id });

            Ok(())
        }

        /// 配置实体市场
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn configure_market(
            origin: OriginFor<T>,
            entity_id: u64,
            cos_enabled: bool,
            usdt_enabled: bool,
            fee_rate: u16,
            min_order_amount: u128,
            order_ttl: u32,
            usdt_timeout: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            // H8: 手续费率上限验证（最高 50%）
            ensure!(fee_rate <= 5000, Error::<T>::InvalidFeeRate);

            // H6 审计修复: TTL 和超时最小值验证（防止立即过期）
            ensure!(order_ttl >= 10, Error::<T>::OrderTtlTooShort);
            ensure!(usdt_timeout >= 10, Error::<T>::UsdtTimeoutTooShort);

            let config = MarketConfig {
                cos_enabled,
                usdt_enabled,
                fee_rate,
                min_order_amount,
                order_ttl,
                usdt_timeout,
                fee_recipient: None,
            };

            MarketConfigs::<T>::insert(entity_id, config);

            Self::deposit_event(Event::MarketConfigured { entity_id });

            Ok(())
        }

        /// 配置价格保护（实体所有者调用）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `enabled`: 是否启用价格保护
        /// - `max_price_deviation`: 最大价格偏离（基点，2000 = 20%）
        /// - `max_slippage`: 最大滑点（基点，500 = 5%）
        /// - `circuit_breaker_threshold`: 熔断阈值（基点，5000 = 50%）
        /// - `min_trades_for_twap`: 启用 TWAP 的最小成交数
        #[pallet::call_index(15)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn configure_price_protection(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
            max_price_deviation: u16,
            max_slippage: u16,
            circuit_breaker_threshold: u16,
            min_trades_for_twap: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            // M4: 参数验证（基点不超过 10000）
            ensure!(max_price_deviation <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(max_slippage <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(circuit_breaker_threshold <= 10000, Error::<T>::InvalidBasisPoints);

            // 获取现有配置或创建新配置
            let mut config = PriceProtection::<T>::get(entity_id).unwrap_or_default();

            config.enabled = enabled;
            config.max_price_deviation = max_price_deviation;
            config.max_slippage = max_slippage;
            config.circuit_breaker_threshold = circuit_breaker_threshold;
            config.min_trades_for_twap = min_trades_for_twap;

            PriceProtection::<T>::insert(entity_id, config);

            Self::deposit_event(Event::PriceProtectionConfigured {
                entity_id,
                enabled,
                max_deviation: max_price_deviation,
                max_slippage,
            });

            Ok(())
        }

        /// 手动解除熔断（实体所有者调用，仅在熔断时间到期后）
        #[pallet::call_index(16)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn lift_circuit_breaker(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            // M3: 检查熔断是否活跃且已到期
            let config = PriceProtection::<T>::get(entity_id).unwrap_or_default();
            ensure!(config.circuit_breaker_active, Error::<T>::MarketCircuitBreakerActive);
            ensure!(current_block >= config.circuit_breaker_until, Error::<T>::InvalidTradeStatus);

            PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.circuit_breaker_active = false;
                    config.circuit_breaker_until = 0;
                }
            });

            Self::deposit_event(Event::CircuitBreakerLifted { entity_id });

            Ok(())
        }

        /// 设置实体代币初始价格（实体所有者调用，用于 TWAP 冷启动）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `initial_price`: 初始参考价格（每个 Token 的 NEX 价格）
        ///
        /// # 说明
        /// 初始价格用于 TWAP 冷启动期间的价格偏离检查。
        /// 当市场成交量不足时，将使用此价格作为参考。
        /// 一旦成交量达到 `min_trades_for_twap`，将自动切换到 TWAP 价格。
        #[pallet::call_index(17)]
        #[pallet::weight(Weight::from_parts(30_000_000, 4_000))]
        pub fn set_initial_price(
            origin: OriginFor<T>,
            entity_id: u64,
            initial_price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);

            // 验证价格
            ensure!(!initial_price.is_zero(), Error::<T>::ZeroPrice);

            // 更新价格保护配置中的初始价格
            PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                let config = maybe_config.get_or_insert_with(Default::default);
                config.initial_price = Some(initial_price);
            });

            // 初始化 TWAP 累积器（如果不存在）
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
            TwapAccumulators::<T>::mutate(entity_id, |maybe_acc| {
                if maybe_acc.is_none() {
                    *maybe_acc = Some(TwapAccumulator {
                        current_cumulative: 0,
                        current_block,
                        last_price: initial_price,
                        trade_count: 0,
                        hour_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                        day_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                        week_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                        last_hour_update: current_block,
                        last_day_update: current_block,
                        last_week_update: current_block,
                    });
                }
            });

            // 设置最新成交价为初始价格
            LastTradePrice::<T>::insert(entity_id, initial_price);

            Self::deposit_event(Event::InitialPriceSet { entity_id, initial_price });

            Ok(())
        }

        // ==================== USDT 通道 Extrinsics ====================

        /// 挂 USDT 卖单（卖 Token 收 USDT）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `token_amount`: 出售的 Token 数量
        /// - `usdt_price`: 每个 Token 的 USDT 价格（精度 10^6）
        /// - `tron_address`: 卖家的 TRON 收款地址
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(55_000_000, 6_000))]
        pub fn place_usdt_sell_order(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            usdt_price: u64,
            tron_address: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证 USDT 市场
            Self::ensure_usdt_market_enabled(entity_id)?;

            // 验证参数
            ensure!(usdt_price > 0, Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            let tron_addr = Self::validate_tron_address(tron_address)?;

            // 检查用户 Token 余额
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            ensure!(balance >= token_amount, Error::<T>::InsufficientTokenBalance);

            // 锁定 Token
            T::TokenProvider::reserve(entity_id, &who, token_amount)?;

            // 创建订单
            let order_id = Self::do_create_order(
                entity_id,
                who.clone(),
                OrderSide::Sell,
                OrderType::Limit,
                PaymentChannel::USDT,
                token_amount,
                Zero::zero(), // NEX price (USDT 通道不使用)
                usdt_price,
                Some(tron_addr.clone()),
                Zero::zero(), // maker_deposit (USDT 卖单已锁定 Token)
            )?;

            Self::deposit_event(Event::UsdtSellOrderCreated {
                order_id,
                entity_id,
                maker: who,
                token_amount,
                usdt_price,
                tron_address: tron_addr,
            });

            Ok(())
        }

        /// 挂 USDT 买单（用 USDT 买 Token）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `token_amount`: 想购买的 Token 数量
        /// - `usdt_price`: 每个 Token 愿意支付的 USDT 价格（精度 10^6）
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(45_000_000, 5_000))]
        pub fn place_usdt_buy_order(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            usdt_price: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证 USDT 市场
            Self::ensure_usdt_market_enabled(entity_id)?;

            // 验证参数
            ensure!(usdt_price > 0, Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // P0 修复: USDT 买单必须锁定 NEX 保证金，防止零成本 DoS 订单簿
            let token_u128: u128 = token_amount.into();
            let total_usdt = token_u128
                .checked_mul(usdt_price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            // H1 审计修复: 防止 u128→u64 截断导致保证金不足
            ensure!(total_usdt <= u64::MAX as u128, Error::<T>::ArithmeticOverflow);
            let maker_deposit = Self::calculate_buyer_deposit(total_usdt as u64);
            if !maker_deposit.is_zero() {
                T::Currency::reserve(&who, maker_deposit)
                    .map_err(|_| Error::<T>::InsufficientDepositBalance)?;
            }

            // 创建订单
            let order_id = Self::do_create_order(
                entity_id,
                who.clone(),
                OrderSide::Buy,
                OrderType::Limit,
                PaymentChannel::USDT,
                token_amount,
                Zero::zero(), // NEX price (USDT 通道不使用)
                usdt_price,
                None, // 买单不需要 TRON 地址
                maker_deposit,
            )?;

            Self::deposit_event(Event::UsdtBuyOrderCreated {
                order_id,
                entity_id,
                maker: who,
                token_amount,
                usdt_price,
            });

            Ok(())
        }

        /// 预锁定 USDT 卖单（买家发起）🆕
        ///
        /// # 流程（两阶段安全模式）
        /// 1. 买家调用此函数预锁定订单份额
        /// 2. 锁定买家的 NEX 保证金
        /// 3. 锁定订单对应的 Token 份额
        /// 4. 创建 UsdtTrade (status: AwaitingPayment)
        /// 5. 买家链下支付 USDT
        /// 6. 买家调用 confirm_usdt_payment 提交 tx_hash
        ///
        /// # 安全
        /// 先链上锁定，后链下支付，避免多人同时支付的并发问题
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(70_000_000, 8_000))]
        pub fn reserve_usdt_sell_order(
            origin: OriginFor<T>,
            order_id: u64,
            amount: Option<T::TokenBalance>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取订单
            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

            // 验证订单
            ensure!(order.channel == PaymentChannel::USDT, Error::<T>::ChannelMismatch);
            ensure!(order.side == OrderSide::Sell, Error::<T>::OrderSideMismatch);
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );
            // H2 审计修复: 过期订单不可预锁定
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= order.expires_at, Error::<T>::OrderClosed);
            ensure!(order.maker != who, Error::<T>::CannotTakeOwnOrder);

            // 计算成交数量
            let available = order.token_amount.checked_sub(&order.filled_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            let fill_amount = amount.unwrap_or(available).min(available);
            ensure!(!fill_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 计算 USDT 金额
            let fill_u128: u128 = fill_amount.into();
            let usdt_amount_u128 = fill_u128
                .checked_mul(order.usdt_price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            // H5 审计修复: 防止 u128→u64 截断
            ensure!(usdt_amount_u128 <= u64::MAX as u128, Error::<T>::ArithmeticOverflow);
            let usdt_amount = usdt_amount_u128 as u64;

            // 获取卖家 TRON 地址
            let seller_tron_address = order.tron_address.clone()
                .ok_or(Error::<T>::InvalidTronAddress)?;

            // 计算并锁定买家保证金
            let buyer_deposit = Self::calculate_buyer_deposit(usdt_amount);
            if !buyer_deposit.is_zero() {
                let buyer_balance = T::Currency::free_balance(&who);
                ensure!(buyer_balance >= buyer_deposit, Error::<T>::InsufficientDepositBalance);
                T::Currency::reserve(&who, buyer_deposit)?;
            }

            // 创建 USDT 交易记录（含保证金信息，状态为 AwaitingPayment）
            let trade_id = Self::do_create_usdt_trade_with_deposit(
                order_id,
                order.entity_id,
                order.maker.clone(),
                who.clone(),
                fill_amount,
                usdt_amount,
                seller_tron_address,
                buyer_deposit,
            )?;

            // 注意：状态保持为 AwaitingPayment，等待买家支付后调用 confirm_usdt_payment

            // 更新订单已成交数量（Token 仍锁定，等待验证通过后释放）
            order.filled_amount = order.filled_amount.checked_add(&fill_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            if order.filled_amount >= order.token_amount {
                order.status = OrderStatus::Filled;
            } else {
                order.status = OrderStatus::PartiallyFilled;
            }
            Orders::<T>::insert(order_id, &order);

            Self::deposit_event(Event::UsdtTradeCreated {
                trade_id,
                order_id,
                seller: order.maker.clone(),
                buyer: who.clone(),
                token_amount: fill_amount,
                usdt_amount,
            });

            // 发出保证金锁定事件
            if !buyer_deposit.is_zero() {
                Self::deposit_event(Event::BuyerDepositLocked {
                    trade_id,
                    buyer: who,
                    deposit: buyer_deposit,
                });
            }

            Ok(())
        }

        /// 接受 USDT 买单（卖家发起）
        ///
        /// # 流程
        /// 1. 卖家看到买单，调用此函数接受
        /// 2. 🆕 锁定买家的 NEX 保证金
        /// 3. 锁定卖家的 Token
        /// 4. 创建 USDT 交易记录，等待买家支付
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(75_000_000, 8_000))]
        pub fn accept_usdt_buy_order(
            origin: OriginFor<T>,
            order_id: u64,
            amount: Option<T::TokenBalance>,
            tron_address: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取订单
            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            let buyer = order.maker.clone();

            // 验证订单
            ensure!(order.channel == PaymentChannel::USDT, Error::<T>::ChannelMismatch);
            ensure!(order.side == OrderSide::Buy, Error::<T>::OrderSideMismatch);
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::OrderClosed
            );
            // H2 审计修复: 过期订单不可接受
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= order.expires_at, Error::<T>::OrderClosed);
            ensure!(buyer != who, Error::<T>::CannotTakeOwnOrder);

            let tron_addr = Self::validate_tron_address(tron_address)?;

            // 计算成交数量
            let available = order.token_amount.checked_sub(&order.filled_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            let fill_amount = amount.unwrap_or(available).min(available);
            ensure!(!fill_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 计算 USDT 金额
            let fill_u128: u128 = fill_amount.into();
            let usdt_amount_u128 = fill_u128
                .checked_mul(order.usdt_price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            // H5 审计修复: 防止 u128→u64 截断
            ensure!(usdt_amount_u128 <= u64::MAX as u128, Error::<T>::ArithmeticOverflow);
            let usdt_amount = usdt_amount_u128 as u64;

            // P0 修复: 买家在 place_usdt_buy_order 时已锁定 maker_deposit 保证金
            // H2 审计修复: 按比例分配保证金，防止多个 partial fill 引用同一份保证金
            let buyer_deposit = if fill_amount >= available {
                // 全部成交，取剩余全部保证金
                order.maker_deposit
            } else {
                // 部分成交，按比例分配
                let deposit_u128: u128 = order.maker_deposit.into();
                let fill_u128: u128 = fill_amount.into();
                let available_u128: u128 = available.into();
                let proportional: u128 = deposit_u128.saturating_mul(fill_u128) / available_u128;
                proportional.into()
            };
            // 扣减订单剩余保证金
            order.maker_deposit = order.maker_deposit.saturating_sub(buyer_deposit);

            // 检查卖家 Token 余额并锁定
            let seller_balance = T::TokenProvider::token_balance(order.entity_id, &who);
            ensure!(seller_balance >= fill_amount, Error::<T>::InsufficientTokenBalance);
            T::TokenProvider::reserve(order.entity_id, &who, fill_amount)?;

            // 创建 USDT 交易记录（等待买家支付，含保证金信息）
            let trade_id = Self::do_create_usdt_trade_with_deposit(
                order_id,
                order.entity_id,
                who.clone(),        // 卖家
                buyer.clone(),      // 买家
                fill_amount,
                usdt_amount,
                tron_addr,
                buyer_deposit,
            )?;

            // 更新订单已成交数量
            order.filled_amount = order.filled_amount.checked_add(&fill_amount)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            if order.filled_amount >= order.token_amount {
                order.status = OrderStatus::Filled;
            } else {
                order.status = OrderStatus::PartiallyFilled;
            }
            Orders::<T>::insert(order_id, &order);

            Self::deposit_event(Event::UsdtTradeCreated {
                trade_id,
                order_id,
                seller: who,
                buyer: buyer.clone(),
                token_amount: fill_amount,
                usdt_amount,
            });

            // 🆕 发出保证金锁定事件
            if !buyer_deposit.is_zero() {
                Self::deposit_event(Event::BuyerDepositLocked {
                    trade_id,
                    buyer,
                    deposit: buyer_deposit,
                });
            }

            Ok(())
        }

        /// 买家确认 USDT 支付（提交交易哈希）
        ///
        /// # 说明
        /// 用于 accept_usdt_buy_order 流程，买家支付后提交交易哈希
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
        pub fn confirm_usdt_payment(
            origin: OriginFor<T>,
            trade_id: u64,
            tron_tx_hash: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;

            // 验证是买家
            ensure!(trade.buyer == who, Error::<T>::NotTradeParticipant);

            // 验证状态
            ensure!(trade.status == UsdtTradeStatus::AwaitingPayment, Error::<T>::InvalidTradeStatus);

            // 检查是否超时
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now <= trade.timeout_at, Error::<T>::TradeTimeout);

            // 验证交易哈希格式
            ensure!(tron_tx_hash.len() == 64, Error::<T>::InvalidTxHash);
            let tx_hash: TronTxHash = tron_tx_hash.try_into().map_err(|_| Error::<T>::InvalidTxHash)?;

            // 更新交易记录
            trade.tron_tx_hash = Some(tx_hash.clone());
            trade.status = UsdtTradeStatus::AwaitingVerification;
            UsdtTrades::<T>::insert(trade_id, &trade);

            // 添加到待验证队列
            PendingUsdtTrades::<T>::try_mutate(|pending| {
                pending.try_push(trade_id).map_err(|_| Error::<T>::PendingQueueFull)
            })?;

            Self::deposit_event(Event::UsdtPaymentSubmitted {
                trade_id,
                tron_tx_hash: tx_hash,
            });

            Ok(())
        }

        /// OCW 验证 USDT 支付结果（无签名交易）
        ///
        /// # 说明
        /// 由 OCW 调用，验证 TRON 交易是否有效
        /// 
        /// # 安全
        /// 使用 ValidateUnsigned 验证，只有 OCW 可以提交
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(60_000_000, 7_000))]
        pub fn verify_usdt_payment(
            origin: OriginFor<T>,
            trade_id: u64,
            verified: bool,
            actual_amount: u64,
        ) -> DispatchResult {
            // 确保是无签名交易（由 OCW 提交）
            ensure_none(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;

            // 验证状态
            ensure!(trade.status == UsdtTradeStatus::AwaitingVerification, Error::<T>::InvalidTradeStatus);

            if verified {
                // 验证通过，完成交易
                // 将锁定的 Token 转给买家
                T::TokenProvider::repatriate_reserved(
                    trade.entity_id,
                    &trade.seller,
                    &trade.buyer,
                    trade.token_amount,
                )?;

                // C1 审计修复: 释放买家保证金（与 process_full_payment 逻辑一致）
                if !trade.buyer_deposit.is_zero() && trade.deposit_status == BuyerDepositStatus::Locked {
                    T::Currency::unreserve(&trade.buyer, trade.buyer_deposit);
                    trade.deposit_status = BuyerDepositStatus::Released;

                    Self::deposit_event(Event::BuyerDepositReleased {
                        trade_id,
                        buyer: trade.buyer.clone(),
                        deposit: trade.buyer_deposit,
                    });
                }

                trade.status = UsdtTradeStatus::Completed;
                UsdtTrades::<T>::insert(trade_id, &trade);

                // 更新统计
                MarketStatsStorage::<T>::mutate(trade.entity_id, |stats| {
                    stats.total_trades = stats.total_trades.saturating_add(1);
                    stats.total_volume_usdt = stats.total_volume_usdt.saturating_add(trade.usdt_amount);
                });

                Self::deposit_event(Event::UsdtTradeCompleted {
                    trade_id,
                    order_id: trade.order_id,
                });
            } else {
                // 验证失败，退还 Token 给卖家
                T::TokenProvider::unreserve(trade.entity_id, &trade.seller, trade.token_amount);

                // H6 修复: 退还买家保证金（OCW 验证失败不是买家的过错）
                if !trade.buyer_deposit.is_zero() && trade.deposit_status == BuyerDepositStatus::Locked {
                    T::Currency::unreserve(&trade.buyer, trade.buyer_deposit);
                    trade.deposit_status = BuyerDepositStatus::Released;

                    Self::deposit_event(Event::BuyerDepositReleased {
                        trade_id,
                        buyer: trade.buyer.clone(),
                        deposit: trade.buyer_deposit,
                    });
                }

                // H7 修复: 回滚父订单的 filled_amount
                Self::rollback_order_filled_amount(trade.order_id, trade.token_amount);

                trade.status = UsdtTradeStatus::Cancelled;
                UsdtTrades::<T>::insert(trade_id, &trade);

                Self::deposit_event(Event::UsdtTradeVerificationFailed {
                    trade_id,
                    reason: b"OCW verification failed".to_vec(),
                });
            }

            // 从待验证队列移除
            PendingUsdtTrades::<T>::mutate(|pending| {
                pending.retain(|&id| id != trade_id);
            });

            Ok(())
        }

        /// 处理超时的 USDT 交易（任何人可调用）
        /// 
        /// 🆕 超时时买家保证金将按 DepositForfeitRate 比例没收给卖家
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(55_000_000, 7_000))]
        pub fn process_usdt_timeout(
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

            // ── 执行超时退款（AwaitingPayment 或 无结果）──
            T::TokenProvider::unreserve(trade.entity_id, &trade.seller, trade.token_amount);
            Self::rollback_order_filled_amount(trade.order_id, trade.token_amount);

            // 没收买家保证金
            Self::forfeit_buyer_deposit(&mut trade, trade_id, T::DepositForfeitRate::get());

            let is_verification_timeout = trade.status == UsdtTradeStatus::AwaitingVerification;
            let buyer_clone = trade.buyer.clone();
            let seller_clone = trade.seller.clone();
            let usdt_amount = trade.usdt_amount;

            trade.status = UsdtTradeStatus::Refunded;
            UsdtTrades::<T>::insert(trade_id, &trade);

            // 清理所有队列
            PendingUsdtTrades::<T>::mutate(|pending| { pending.retain(|&id| id != trade_id); });
            PendingUnderpaidTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
            OcwVerificationResults::<T>::remove(trade_id);

            if is_verification_timeout {
                Self::deposit_event(Event::VerificationTimeoutRefunded {
                    trade_id, buyer: buyer_clone, seller: seller_clone, usdt_amount,
                });
            } else {
                Self::deposit_event(Event::UsdtTradeRefunded { trade_id });
            }

            Ok(())
        }

        // ==================== 激励机制 ====================

        /// 提交 OCW 验证结果（无签名交易）
        /// 
        /// OCW 验证完成后调用此函数将结果提交到链上
        /// 🆕 支持多档判定结果
        #[pallet::call_index(18)]
        #[pallet::weight(Weight::from_parts(30_000_000, 4_000))]
        pub fn submit_ocw_result(
            origin: OriginFor<T>,
            trade_id: u64,
            actual_amount: u64,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let mut trade = UsdtTrades::<T>::get(trade_id).ok_or(Error::<T>::UsdtTradeNotFound)?;
            ensure!(trade.status == UsdtTradeStatus::AwaitingVerification, Error::<T>::InvalidTradeStatus);

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
                    if PendingUnderpaidTrades::<T>::try_mutate(|p| { p.try_push(trade_id) }).is_err() {
                        Self::deposit_event(Event::MarketOperationFailed { trade_id, operation: MarketOperation::QueuePushFailed });
                    }

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
                    // Exact/Overpaid/SeverelyUnderpaid/Invalid: 直接存储结果
                    OcwVerificationResults::<T>::insert(trade_id, (verification_result, actual_amount));

                    Self::deposit_event(Event::OcwResultSubmitted {
                        trade_id, verification_result, actual_amount,
                    });
                }
            }

            Ok(())
        }

        /// 领取验证奖励（任何人可调用）
        /// 
        /// ## 功能说明
        /// 当 OCW 验证结果已提交到链上后，任何人可调用此函数：
        /// 1. 完成验证确认（调用 verify_usdt_payment 逻辑）
        /// 2. 获得 VerificationReward 奖励
        /// 
        /// ## 安全机制
        /// - 验证结果必须已存储在 OcwVerificationResults
        /// - 调用者无法伪造验证结果
        /// - 只有 AwaitingVerification 状态的交易可以确认
        #[pallet::call_index(19)]
        #[pallet::weight(Weight::from_parts(80_000_000, 10_000))]
        pub fn claim_verification_reward(
            origin: OriginFor<T>,
            trade_id: u64,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::do_claim_verification_reward(&caller, trade_id)
        }

        /// OCW 更新少付交易的累计金额（unsigned）
        ///
        /// 补付窗口内，OCW 持续扫描 TronGrid，发现新转账则更新金额。
        /// 若累计金额达到 99.5%，直接升级为 Exact 结算。
        #[pallet::call_index(20)]
        #[pallet::weight(Weight::from_parts(30_000_000, 4_000))]
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

            OcwVerificationResults::<T>::insert(trade_id, (new_result, new_actual_amount));

            Self::deposit_event(Event::UnderpaidAmountUpdated {
                trade_id, previous_amount, new_amount: new_actual_amount,
            });

            // 补齐了！直接升级回 AwaitingVerification
            if matches!(new_result, PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid) {
                trade.status = UsdtTradeStatus::AwaitingVerification;
                UsdtTrades::<T>::insert(trade_id, &trade);

                PendingUnderpaidTrades::<T>::mutate(|p| { p.retain(|&id| id != trade_id); });
                if PendingUsdtTrades::<T>::try_mutate(|p| { p.try_push(trade_id) }).is_err() {
                    Self::deposit_event(Event::MarketOperationFailed { trade_id, operation: MarketOperation::QueuePushFailed });
                }
            }

            Ok(())
        }

        /// 少付终裁（任何人，补付窗口到期后）
        #[pallet::call_index(21)]
        #[pallet::weight(Weight::from_parts(80_000_000, 10_000))]
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

            let (_, final_amount) = OcwVerificationResults::<T>::get(trade_id)
                .ok_or(Error::<T>::OcwResultNotFound)?;

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

            Ok(())
        }

        // ==================== Phase 3: 市价单 ====================

        /// 市价买单（立即以最优卖价成交）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `token_amount`: 想购买的 Token 数量
        /// - `max_cost`: 最大愿意支付的 NEX 总额（滑点保护）
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(120_000_000, 12_000))]
        pub fn market_buy(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            max_cost: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证市场
            Self::ensure_market_enabled(entity_id)?;

            // 验证参数
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);
            ensure!(!max_cost.is_zero(), Error::<T>::ZeroPrice);

            // 获取卖单列表（按价格升序排列）
            let mut sell_orders = Self::get_sorted_sell_orders(entity_id);
            ensure!(!sell_orders.is_empty(), Error::<T>::NoOrdersAvailable);

            // 执行市价买入
            let (filled, total_next, fees) = Self::do_market_buy(
                &who,
                entity_id,
                token_amount,
                max_cost,
                &mut sell_orders,
            )?;

            ensure!(!filled.is_zero(), Error::<T>::AmountTooSmall);

            Self::deposit_event(Event::MarketOrderExecuted {
                entity_id,
                trader: who,
                side: OrderSide::Buy,
                filled_amount: filled,
                total_next,
                total_fee: fees,
            });

            Ok(())
        }

        /// 市价卖单（立即以最优买价成交）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `token_amount`: 想出售的 Token 数量
        /// - `min_receive`: 最低愿意收到的 NEX 总额（滑点保护）
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(120_000_000, 12_000))]
        pub fn market_sell(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            min_receive: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证市场
            Self::ensure_market_enabled(entity_id)?;

            // 验证参数
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 检查用户 Token 余额
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            ensure!(balance >= token_amount, Error::<T>::InsufficientTokenBalance);

            // 获取买单列表（按价格降序排列）
            let mut buy_orders = Self::get_sorted_buy_orders(entity_id);
            ensure!(!buy_orders.is_empty(), Error::<T>::NoOrdersAvailable);

            // 执行市价卖出
            let (filled, total_receive, fees) = Self::do_market_sell(
                &who,
                entity_id,
                token_amount,
                min_receive,
                &mut buy_orders,
            )?;

            ensure!(!filled.is_zero(), Error::<T>::AmountTooSmall);

            // H3 审计修复: 最终滑点检查（防止 partial fill 绕过 min_receive）
            ensure!(total_receive >= min_receive, Error::<T>::SlippageExceeded);

            Self::deposit_event(Event::MarketOrderExecuted {
                entity_id,
                trader: who,
                side: OrderSide::Sell,
                filled_amount: filled,
                total_next: total_receive,
                total_fee: fees,
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
                Call::verify_usdt_payment { trade_id, verified: _, actual_amount: _ } => {
                    // H1 审计修复: 拒绝 External 来源，防止外部节点伪造验证结果
                    match source {
                        TransactionSource::Local | TransactionSource::InBlock => {},
                        TransactionSource::External => {
                            log::warn!(target: "entity-market-ocw", 
                                "Rejected external unsigned tx for trade {}", trade_id);
                            return InvalidTransaction::BadSigner.into();
                        }
                    }

                    // 安全检查 2: 验证 trade 存在且状态正确
                    let trade = match UsdtTrades::<T>::get(trade_id) {
                        Some(t) => t,
                        None => {
                            log::warn!(target: "entity-market-ocw", "Trade {} not found", trade_id);
                            return InvalidTransaction::Custom(1).into();
                        }
                    };

                    if trade.status != UsdtTradeStatus::AwaitingVerification {
                        log::warn!(target: "entity-market-ocw", 
                            "Trade {} invalid status: {:?}", trade_id, trade.status);
                        return InvalidTransaction::Custom(2).into();
                    }

                    // 安全检查 3: 验证待验证队列包含该 trade
                    let pending = PendingUsdtTrades::<T>::get();
                    if !pending.contains(trade_id) {
                        log::warn!(target: "entity-market-ocw", 
                            "Trade {} not in pending queue", trade_id);
                        return InvalidTransaction::Custom(3).into();
                    }

                    let priority = match source {
                        TransactionSource::Local => 100,
                        TransactionSource::InBlock => 80,
                        _ => 50,
                    };

                    ValidTransaction::with_tag_prefix("EntityMarketTRC20")
                        .priority(priority)
                        .longevity(10)
                        .and_provides([&(b"verify", trade_id)])
                        .propagate(true)
                        .build()
                },
                Call::submit_ocw_result { trade_id, actual_amount: _ } => {
                    // H1 审计修复: 拒绝 External 来源
                    match source {
                        TransactionSource::Local | TransactionSource::InBlock => {},
                        TransactionSource::External => {
                            return InvalidTransaction::BadSigner.into();
                        }
                    }

                    // 安全检查：验证 trade 存在且状态正确
                    let trade = match UsdtTrades::<T>::get(trade_id) {
                        Some(t) => t,
                        None => {
                            return InvalidTransaction::Custom(10).into();
                        }
                    };

                    if trade.status != UsdtTradeStatus::AwaitingVerification {
                        return InvalidTransaction::Custom(11).into();
                    }

                    // 检查是否已有验证结果
                    if OcwVerificationResults::<T>::contains_key(trade_id) {
                        return InvalidTransaction::Custom(12).into();
                    }

                    let priority = match source {
                        TransactionSource::Local => 100,
                        TransactionSource::InBlock => 80,
                        _ => 50,
                    };

                    ValidTransaction::with_tag_prefix("EntityMarketOcwResult")
                        .priority(priority)
                        .longevity(10)
                        .and_provides([&(b"ocw_result", trade_id)])
                        .propagate(true)
                        .build()
                },
                Call::submit_underpaid_update { trade_id, new_actual_amount: _ } => {
                    match source {
                        TransactionSource::Local | TransactionSource::InBlock => {},
                        TransactionSource::External => {
                            return InvalidTransaction::BadSigner.into();
                        }
                    }

                    let trade = match UsdtTrades::<T>::get(trade_id) {
                        Some(t) => t,
                        None => return InvalidTransaction::Custom(30).into(),
                    };
                    if trade.status != UsdtTradeStatus::UnderpaidPending {
                        return InvalidTransaction::Custom(31).into();
                    }

                    let priority = match source {
                        TransactionSource::Local => 80,
                        TransactionSource::InBlock => 60,
                        _ => 30,
                    };
                    ValidTransaction::with_tag_prefix("EntityMarketUnderpaidUpdate")
                        .priority(priority)
                        .longevity(5)
                        .and_provides([&(b"undp_upd", trade_id)])
                        .propagate(true)
                        .build()
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 验证市场是否启用
        /// H4 审计修复: 添加 is_entity_active 检查，Banned/Closed 实体不允许新订单
        fn ensure_market_enabled(entity_id: u64) -> DispatchResult {
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::TokenProvider::is_token_enabled(entity_id),
                Error::<T>::TokenNotEnabled
            );

            // M6: 检查市场配置（必须显式配置并启用，与 Default cos_enabled=false 一致）
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            ensure!(config.cos_enabled, Error::<T>::MarketNotEnabled);

            Ok(())
        }

        /// 获取实体市场的手续费率 (bps, 10000 = 100%)
        fn get_fee_rate(entity_id: u64) -> u16 {
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.fee_rate > 0 { config.fee_rate } else { T::DefaultFeeRate::get() }
        }

        /// 计算手续费 = amount × fee_rate / 10000
        fn calculate_fee(amount: BalanceOf<T>, fee_rate: u16) -> BalanceOf<T> {
            amount
                .saturating_mul(fee_rate.into())
                .checked_div(&10000u32.into())
                .unwrap_or_else(Zero::zero)
        }

        /// 计算总成本 (NEX) = token_amount × price
        ///
        /// P2 注意: price 为每单位 Token 的 NEX 价格（最小精度单位）
        /// 例如: 买 1000 Token，每个 100 NEX → total = 100_000 NEX
        /// 前端需注意: Token 和 NEX 均以最小单位（planck）表示
        /// 如需支持小数价格，前端应将价格乘以精度因子后传入
        fn calculate_total_next(token_amount: u128, price: BalanceOf<T>) -> Result<BalanceOf<T>, DispatchError> {
            let price_u128: u128 = price.into();
            let total = token_amount
                .checked_mul(price_u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            Ok(total.into())
        }

        /// P0 修复: 挂单时自动撮合交叉订单
        /// - taker_side=Sell → 与买单撮合（buy_price >= limit_price）
        /// - taker_side=Buy  → 与卖单撮合（sell_price <= limit_price）
        /// 返回 (已撮合数量, NEX 总额, 手续费)
        fn do_cross_match(
            taker: &T::AccountId,
            entity_id: u64,
            taker_side: OrderSide,
            limit_price: BalanceOf<T>,
            mut remaining: T::TokenBalance,
        ) -> Result<(T::TokenBalance, BalanceOf<T>, BalanceOf<T>), DispatchError> {
            let mut total_filled: T::TokenBalance = Zero::zero();
            let mut total_nex: BalanceOf<T> = Zero::zero();
            let mut total_fees: BalanceOf<T> = Zero::zero();

            let fee_rate = Self::get_fee_rate(entity_id);

            // 获取对手方订单（卖单时取买单，买单时取卖单）
            let counter_orders = match taker_side {
                OrderSide::Sell => Self::get_sorted_buy_orders(entity_id),
                OrderSide::Buy => Self::get_sorted_sell_orders(entity_id),
            };

            for counter_order in counter_orders {
                if remaining.is_zero() { break; }
                // 交叉条件检查
                let price_ok = match taker_side {
                    OrderSide::Sell => counter_order.price >= limit_price,
                    OrderSide::Buy => counter_order.price <= limit_price,
                };
                if !price_ok { break; }
                if counter_order.maker == *taker { continue; }

                let available = counter_order.token_amount.saturating_sub(counter_order.filled_amount);
                let fill_amount = remaining.min(available);
                if fill_amount.is_zero() { continue; }

                // 以挂单方价格成交
                let fill_u128: u128 = fill_amount.into();
                let nex_amount = Self::calculate_total_next(fill_u128, counter_order.price)?;
                let fee = Self::calculate_fee(nex_amount, fee_rate);
                let net = nex_amount.saturating_sub(fee);

                // NEX 转账: 买方(reserved) → 卖方(net) + entity_owner(fee)
                let (nex_payer, nex_receiver) = match taker_side {
                    OrderSide::Sell => (&counter_order.maker, taker),
                    OrderSide::Buy => (taker, &counter_order.maker),
                };
                T::Currency::repatriate_reserved(
                    nex_payer, nex_receiver, net,
                    frame_support::traits::BalanceStatus::Free,
                )?;
                if !fee.is_zero() {
                    if let Some(entity_owner) = T::EntityProvider::entity_owner(entity_id) {
                        T::Currency::repatriate_reserved(
                            nex_payer, &entity_owner, fee,
                            frame_support::traits::BalanceStatus::Free,
                        )?;
                    }
                }

                // Token 转账: 卖方(reserved) → 买方
                let (token_from, token_to) = match taker_side {
                    OrderSide::Sell => (taker, &counter_order.maker),
                    OrderSide::Buy => (&counter_order.maker, taker),
                };
                T::TokenProvider::repatriate_reserved(
                    entity_id, token_from, token_to, fill_amount,
                )?;

                Self::update_order_fill(&counter_order, entity_id, fill_amount);

                MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                    stats.total_trades = stats.total_trades.saturating_add(1);
                    stats.total_volume_nex = stats.total_volume_nex.saturating_add(nex_amount.into());
                    stats.total_fees_cos = stats.total_fees_cos.saturating_add(fee.into());
                });

                Self::deposit_event(Event::OrderFilled {
                    order_id: counter_order.order_id,
                    taker: taker.clone(),
                    filled_amount: fill_amount,
                    total_next: nex_amount,
                    fee,
                });
                Self::on_trade_completed(entity_id, counter_order.price);

                total_filled = total_filled.saturating_add(fill_amount);
                total_nex = total_nex.saturating_add(nex_amount);
                total_fees = total_fees.saturating_add(fee);
                remaining = remaining.saturating_sub(fill_amount);
            }

            Ok((total_filled, total_nex, total_fees))
        }

        /// 创建订单（通用）
        fn do_create_order(
            entity_id: u64,
            maker: T::AccountId,
            side: OrderSide,
            order_type: OrderType,
            channel: PaymentChannel,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
            usdt_price: u64,
            tron_address: Option<TronAddress>,
            maker_deposit: BalanceOf<T>,
        ) -> Result<u64, DispatchError> {
            let order_id = NextOrderId::<T>::get();
            NextOrderId::<T>::put(order_id.saturating_add(1));

            let now = <frame_system::Pallet<T>>::block_number();
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            let ttl = if config.order_ttl > 0 {
                config.order_ttl
            } else {
                T::DefaultOrderTTL::get()
            };
            let expires_at = now.saturating_add(ttl.into());

            let order = TradeOrder {
                order_id,
                entity_id,
                maker: maker.clone(),
                side,
                order_type,
                channel,
                token_amount,
                filled_amount: Zero::zero(),
                price,
                usdt_price,
                tron_address,
                status: OrderStatus::Open,
                created_at: now,
                expires_at,
                maker_deposit,
            };

            Orders::<T>::insert(order_id, order);

            // 添加到订单簿
            match side {
                OrderSide::Sell => {
                    EntitySellOrders::<T>::try_mutate(entity_id, |orders| {
                        orders.try_push(order_id).map_err(|_| Error::<T>::OrderBookFull)
                    })?;
                }
                OrderSide::Buy => {
                    EntityBuyOrders::<T>::try_mutate(entity_id, |orders| {
                        orders.try_push(order_id).map_err(|_| Error::<T>::OrderBookFull)
                    })?;
                }
            }

            // 添加到用户订单
            UserOrders::<T>::try_mutate(&maker, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::UserOrdersFull)
            })?;

            // 更新统计
            MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                stats.total_orders = stats.total_orders.saturating_add(1);
            });

            Ok(order_id)
        }

        /// 没收买家保证金（通用）
        /// 返回实际没收金额。forfeit_rate 为万分比 (10000 = 100%)
        fn forfeit_buyer_deposit(
            trade: &mut UsdtTrade<T>,
            trade_id: u64,
            forfeit_rate: u16,
        ) -> BalanceOf<T> {
            if trade.buyer_deposit.is_zero() || trade.deposit_status != BuyerDepositStatus::Locked {
                return Zero::zero();
            }

            let deposit_u128: u128 = trade.buyer_deposit.into();
            let forfeit_u128 = deposit_u128
                .saturating_mul(forfeit_rate as u128)
                .saturating_div(10000);
            let forfeit_amount: BalanceOf<T> = forfeit_u128.into();

            if !forfeit_amount.is_zero() {
                let treasury = T::TreasuryAccount::get();
                if T::Currency::repatriate_reserved(
                    &trade.buyer, &treasury, forfeit_amount,
                    frame_support::traits::BalanceStatus::Free,
                ).is_err() {
                    Self::deposit_event(Event::MarketOperationFailed { trade_id, operation: MarketOperation::DepositForfeit });
                }
            }

            let refund = trade.buyer_deposit.saturating_sub(forfeit_amount);
            if !refund.is_zero() {
                T::Currency::unreserve(&trade.buyer, refund);
            }

            if forfeit_rate >= 10000 {
                trade.deposit_status = BuyerDepositStatus::Forfeited;
            } else if forfeit_rate > 0 {
                trade.deposit_status = BuyerDepositStatus::PartiallyForfeited;
            } else {
                trade.deposit_status = BuyerDepositStatus::Released;
            }

            Self::deposit_event(Event::BuyerDepositForfeited {
                trade_id,
                buyer: trade.buyer.clone(),
                forfeited: forfeit_amount,
                to_treasury: forfeit_amount,
            });

            forfeit_amount
        }

        /// 验证并转换 TRON 地址（Base58Check 完整校验）
        fn validate_tron_address(raw: Vec<u8>) -> Result<TronAddress, DispatchError> {
            ensure!(pallet_trading_common::is_valid_tron_address(&raw), Error::<T>::InvalidTronAddress);
            raw.try_into().map_err(|_| Error::<T>::InvalidTronAddress.into())
        }

        /// 验证 USDT 市场是否启用
        /// H4 审计修复: 添加 is_entity_active 检查
        fn ensure_usdt_market_enabled(entity_id: u64) -> DispatchResult {
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::TokenProvider::is_token_enabled(entity_id),
                Error::<T>::TokenNotEnabled
            );

            // 检查 USDT 市场配置
            if let Some(config) = MarketConfigs::<T>::get(entity_id) {
                ensure!(config.usdt_enabled, Error::<T>::UsdtMarketNotEnabled);
            } else {
                // 默认不启用 USDT 市场
                return Err(Error::<T>::UsdtMarketNotEnabled.into());
            }

            Ok(())
        }

        /// 获取 USDT 交易超时区块数
        fn get_usdt_timeout(entity_id: u64) -> u32 {
            MarketConfigs::<T>::get(entity_id)
                .map(|c| if c.usdt_timeout > 0 { c.usdt_timeout } else { T::DefaultUsdtTimeout::get() })
                .unwrap_or_else(|| T::DefaultUsdtTimeout::get())
        }

        /// OCW: 处理单笔 USDT 交易验证
        /// 
        /// 验证结果存储在 offchain storage，供外部服务读取并提交
        fn process_verification(trade_id: u64, trade: &UsdtTrade<T>, tx_hash: &[u8]) {
            use crate::ocw;

            log::info!(target: "entity-market-ocw", 
                "Verifying trade {} with tx_hash len={}", trade_id, tx_hash.len());

            // 调用 OCW 验证 TRC20 交易
            let result = ocw::verify_trc20_transaction(
                tx_hash,
                trade.seller_tron_address.as_slice(),
                trade.usdt_amount,
            );

            match result {
                Ok(verification) => {
                    let verified = verification.is_valid;
                    let actual_amount = verification.actual_amount.unwrap_or(0);

                    log::info!(target: "entity-market-ocw", 
                        "Trade {} verification result: verified={}, actual_amount={}", 
                        trade_id, verified, actual_amount);

                    // 存储验证结果到 offchain storage
                    // 外部服务可读取并调用 verify_usdt_payment
                    let key = Self::ocw_result_key(trade_id);
                    let value = (verified, actual_amount);
                    sp_io::offchain::local_storage_set(
                        sp_core::offchain::StorageKind::PERSISTENT,
                        &key,
                        &codec::Encode::encode(&value),
                    );

                    log::info!(target: "entity-market-ocw", 
                        "Stored verification result for trade {}", trade_id);
                },
                Err(e) => {
                    log::error!(target: "entity-market-ocw", 
                        "Verification failed for trade {}: {}", trade_id, e);
                }
            }
        }

        /// 生成 OCW 结果存储键
        fn ocw_result_key(trade_id: u64) -> alloc::vec::Vec<u8> {
            let mut key = b"entity_market_ocw_result::".to_vec();
            key.extend_from_slice(&trade_id.to_le_bytes());
            key
        }

        /// OCW: 补付窗口内重新查询 UnderpaidPending 交易的累计金额
        fn check_underpaid_topup(trade_id: u64, trade: &UsdtTrade<T>, tx_hash: &[u8]) {
            use crate::ocw;

            log::info!(target: "entity-market-ocw",
                "Checking underpaid topup for trade {} (expected={})",
                trade_id, trade.usdt_amount);

            match ocw::verify_trc20_transaction(
                tx_hash,
                trade.seller_tron_address.as_slice(),
                trade.usdt_amount,
            ) {
                Ok(verification) => {
                    let actual_amount = verification.actual_amount.unwrap_or(0);
                    if actual_amount > 0 {
                        // 写入 offchain storage，sidecar 调用 submit_underpaid_update
                        let key = Self::ocw_underpaid_key(trade_id);
                        let value = (true, actual_amount);
                        sp_io::offchain::local_storage_set(
                            sp_core::offchain::StorageKind::PERSISTENT,
                            &key,
                            &codec::Encode::encode(&value),
                        );
                        log::info!(target: "entity-market-ocw",
                            "Underpaid trade {} updated amount: {}", trade_id, actual_amount);
                    }
                }
                Err(e) => {
                    log::warn!(target: "entity-market-ocw",
                        "Underpaid check trade {} error: {}", trade_id, e);
                }
            }
        }

        /// 生成少付 OCW 结果存储键
        fn ocw_underpaid_key(trade_id: u64) -> alloc::vec::Vec<u8> {
            let mut key = b"entity_market_undp::".to_vec();
            key.extend_from_slice(&trade_id.to_le_bytes());
            key
        }

        /// 获取 OCW 验证结果（供外部服务调用）
        pub fn get_ocw_result(trade_id: u64) -> Option<(bool, u64)> {
            let key = Self::ocw_result_key(trade_id);
            sp_io::offchain::local_storage_get(
                sp_core::offchain::StorageKind::PERSISTENT,
                &key,
            ).and_then(|data| codec::Decode::decode(&mut &data[..]).ok())
        }

        /// 领取验证奖励内部实现
        /// 
        /// ## 流程
        /// 1. 验证 trade 存在且状态正确
        /// 2. 从链上存储读取 OCW 验证结果
        /// 3. 执行验证确认逻辑
        /// 4. 支付奖励给调用者
        /// 🆕 多档判定 + 自动处理
        fn do_claim_verification_reward(
            caller: &T::AccountId,
            trade_id: u64,
        ) -> DispatchResult {
            // 1. 获取交易记录
            let mut trade = UsdtTrades::<T>::get(trade_id)
                .ok_or(Error::<T>::UsdtTradeNotFound)?;

            // 2. 验证状态必须是 AwaitingVerification
            ensure!(
                trade.status == UsdtTradeStatus::AwaitingVerification,
                Error::<T>::InvalidTradeStatus
            );

            // 3. 从链上存储读取 OCW 验证结果（🆕 多档判定）
            let (verification_result, actual_amount) = OcwVerificationResults::<T>::get(trade_id)
                .ok_or(Error::<T>::OcwResultNotFound)?;

            // 4. 🆕 根据多档判定结果执行不同处理（全部按比例自动处理）
            match verification_result {
                PaymentVerificationResult::Exact | PaymentVerificationResult::Overpaid => {
                    // ✅ 验证通过，全额释放
                    Self::process_full_payment(&mut trade, trade_id)?;
                }
                PaymentVerificationResult::Underpaid | 
                PaymentVerificationResult::SeverelyUnderpaid => {
                    // ⚠️ 少付，按比例自动处理（包括严重少付）
                    Self::process_underpaid(&mut trade, trade_id, actual_amount)?;
                }
                PaymentVerificationResult::Invalid => {
                    // ❌ 无效（actual_amount = 0），按 0% 处理
                    Self::process_underpaid(&mut trade, trade_id, 0)?;
                }
            }

            // 5. 从待验证队列移除
            PendingUsdtTrades::<T>::mutate(|pending| {
                pending.retain(|&id| id != trade_id);
            });

            // 6. 清理 OCW 验证结果存储
            OcwVerificationResults::<T>::remove(trade_id);

            // 7. 支付奖励给调用者（失败发事件）
            let reward = T::VerificationReward::get();
            if reward > BalanceOf::<T>::zero() {
                let reward_source = T::RewardSource::get();
                
                if T::Currency::transfer(
                    &reward_source,
                    caller,
                    reward,
                    ExistenceRequirement::KeepAlive,
                ).is_ok() {
                    log::info!(target: "entity-market", 
                        "Paid verification reward to {:?} for trade {}", caller, trade_id);
                } else {
                    Self::deposit_event(Event::MarketOperationFailed { trade_id, operation: MarketOperation::VerificationReward });
                }
            }

            // 8. 发出事件
            Self::deposit_event(Event::VerificationRewardClaimed {
                trade_id,
                claimer: caller.clone(),
                reward,
            });

            Ok(())
        }

        /// 🆕 处理全额付款（验证通过）
        fn process_full_payment(
            trade: &mut UsdtTrade<T>,
            trade_id: u64,
        ) -> DispatchResult {
            // 全额释放 Token 给买家
            T::TokenProvider::repatriate_reserved(
                trade.entity_id,
                &trade.seller,
                &trade.buyer,
                trade.token_amount,
            )?;

            // 退还买家保证金
            if !trade.buyer_deposit.is_zero() && trade.deposit_status == BuyerDepositStatus::Locked {
                T::Currency::unreserve(&trade.buyer, trade.buyer_deposit);
                trade.deposit_status = BuyerDepositStatus::Released;

                Self::deposit_event(Event::BuyerDepositReleased {
                    trade_id,
                    buyer: trade.buyer.clone(),
                    deposit: trade.buyer_deposit,
                });
            }

            trade.status = UsdtTradeStatus::Completed;
            
            // 先提取需要的值
            let entity_id = trade.entity_id;
            let usdt_amount = trade.usdt_amount;
            let order_id = trade.order_id;
            
            UsdtTrades::<T>::insert(trade_id, trade);

            // 更新统计
            MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_usdt = stats.total_volume_usdt.saturating_add(usdt_amount);
            });

            Self::deposit_event(Event::UsdtTradeCompleted {
                trade_id,
                order_id,
            });

            Ok(())
        }

        /// 🆕 处理少付 - 按比例自动处理（包括严重少付）
        fn process_underpaid(
            trade: &mut UsdtTrade<T>,
            trade_id: u64,
            actual_amount: u64,
        ) -> DispatchResult {
            // 计算付款比例 (bps)
            let payment_ratio = pallet_trading_common::compute_payment_ratio_bps(trade.usdt_amount, actual_amount);

            // 按比例计算释放的 Token 数量
            // 使用 u128 中间计算（TokenBalance 实现了 From<u128> 和 Into<u128>）
            let token_u128: u128 = trade.token_amount.into();
            let token_to_release_u128 = token_u128
                .saturating_mul(payment_ratio as u128)
                .saturating_div(10000);
            let token_to_release: T::TokenBalance = token_to_release_u128.into();
            let token_to_refund = trade.token_amount.saturating_sub(token_to_release);

            // 释放部分 Token 给买家
            if !token_to_release.is_zero() {
                T::TokenProvider::repatriate_reserved(
                    trade.entity_id,
                    &trade.seller,
                    &trade.buyer,
                    token_to_release,
                )?;
            }

            // 退还剩余 Token 给卖家
            if !token_to_refund.is_zero() {
                T::TokenProvider::unreserve(trade.entity_id, &trade.seller, token_to_refund);
                // H7 修复: 回滚父订单中未实际成交的部分
                Self::rollback_order_filled_amount(trade.order_id, token_to_refund);
            }

            // 保证金按梯度没收
            let forfeit_rate = Self::calculate_deposit_forfeit_rate(payment_ratio);
            let deposit_forfeited = Self::forfeit_buyer_deposit(trade, trade_id, forfeit_rate);

            trade.status = UsdtTradeStatus::Completed;
            
            // 先提取需要的值
            let entity_id = trade.entity_id;
            let expected_amount = trade.usdt_amount;
            
            UsdtTrades::<T>::insert(trade_id, trade);

            // 更新统计（按实际付款金额）
            MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_usdt = stats.total_volume_usdt.saturating_add(actual_amount);
            });

            Self::deposit_event(Event::UnderpaidAutoProcessed {
                trade_id,
                expected_amount,
                actual_amount,
                payment_ratio,
                token_released: token_to_release,
                deposit_forfeited,
            });

            Ok(())
        }

        /// 保证金没收梯度（委托给 pallet-trading-common）
        fn calculate_deposit_forfeit_rate(payment_ratio: u32) -> u16 {
            pallet_trading_common::calculate_deposit_forfeit_rate(payment_ratio)
        }

        /// 创建 USDT 交易记录（含保证金）
        fn do_create_usdt_trade_with_deposit(
            order_id: u64,
            entity_id: u64,
            seller: T::AccountId,
            buyer: T::AccountId,
            token_amount: T::TokenBalance,
            usdt_amount: u64,
            seller_tron_address: TronAddress,
            buyer_deposit: BalanceOf<T>,
        ) -> Result<u64, DispatchError> {
            let trade_id = NextUsdtTradeId::<T>::get();
            NextUsdtTradeId::<T>::put(trade_id.saturating_add(1));

            let now = <frame_system::Pallet<T>>::block_number();
            let timeout = Self::get_usdt_timeout(entity_id);
            let timeout_at = now.saturating_add(timeout.into());

            let deposit_status = if buyer_deposit.is_zero() {
                BuyerDepositStatus::None
            } else {
                BuyerDepositStatus::Locked
            };

            let trade = UsdtTrade {
                trade_id,
                order_id,
                entity_id,
                seller,
                buyer,
                token_amount,
                usdt_amount,
                seller_tron_address,
                tron_tx_hash: None,
                status: UsdtTradeStatus::AwaitingPayment,
                created_at: now,
                timeout_at,
                buyer_deposit,
                deposit_status,
                first_verified_at: None,
                first_actual_amount: None,
                underpaid_deadline: None,
            };

            UsdtTrades::<T>::insert(trade_id, trade);

            Ok(trade_id)
        }

        /// 🆕 计算买家保证金金额
        /// 
        /// 简化版：使用 MinBuyerDeposit 作为固定保证金
        /// 实际项目中应从 pricing 模块获取实时汇率
        fn calculate_buyer_deposit(_usdt_amount: u64) -> BalanceOf<T> {
            // 返回最低保证金（简化实现）
            // 复杂的比例计算可在后续版本中通过 pricing 模块实现
            T::MinBuyerDeposit::get()
        }

        /// 计算付款金额验证结果（委托给 pallet-trading-common）
        fn calculate_payment_verification_result(
            expected_amount: u64,
            actual_amount: u64,
        ) -> PaymentVerificationResult {
            pallet_trading_common::calculate_payment_verification_result(expected_amount, actual_amount)
        }

        /// H7: 回滚父订单的 filled_amount（USDT 交易失败/超时时调用）
        /// H1 审计修复: 仅对 Filled/PartiallyFilled 订单回滚，防止复活 Expired/Cancelled 订单
        fn rollback_order_filled_amount(order_id: u64, amount: T::TokenBalance) {
            Orders::<T>::mutate(order_id, |maybe_order| {
                if let Some(order) = maybe_order {
                    // H1: 仅允许回滚活跃状态的订单
                    if order.status != OrderStatus::Filled
                        && order.status != OrderStatus::PartiallyFilled
                        && order.status != OrderStatus::Open
                    {
                        return;
                    }

                    order.filled_amount = order.filled_amount.saturating_sub(amount);
                    // 如果回滚后 filled_amount < token_amount，重新开放订单
                    if order.status == OrderStatus::Filled {
                        if order.filled_amount < order.token_amount {
                            order.status = OrderStatus::PartiallyFilled;
                        }
                    }
                    // 如果 filled_amount 归零，恢复为 Open
                    if order.filled_amount.is_zero() {
                        order.status = OrderStatus::Open;
                    }
                }
            });
        }

        /// 更新订单成交量，自动处理 Filled/PartiallyFilled 状态转换
        fn update_order_fill(
            order: &TradeOrder<T>,
            entity_id: u64,
            fill_amount: T::TokenBalance,
        ) {
            let mut updated = order.clone();
            updated.filled_amount = updated.filled_amount.saturating_add(fill_amount);
            if updated.filled_amount >= updated.token_amount {
                updated.status = OrderStatus::Filled;
                Self::remove_from_order_book(entity_id, order.order_id, order.side);
                UserOrders::<T>::mutate(&order.maker, |orders| {
                    orders.retain(|&id| id != order.order_id);
                });
            } else {
                updated.status = OrderStatus::PartiallyFilled;
            }
            Orders::<T>::insert(order.order_id, &updated);
        }

        /// 从订单簿移除订单
        fn remove_from_order_book(entity_id: u64, order_id: u64, side: OrderSide) {
            match side {
                OrderSide::Sell => {
                    EntitySellOrders::<T>::mutate(entity_id, |orders| {
                        orders.retain(|&id| id != order_id);
                    });
                }
                OrderSide::Buy => {
                    EntityBuyOrders::<T>::mutate(entity_id, |orders| {
                        orders.retain(|&id| id != order_id);
                    });
                }
            }
        }

        /// 获取排序后的订单列表（Sell=升序, Buy=降序）
        pub fn get_sorted_orders(entity_id: u64, side: OrderSide) -> Vec<TradeOrder<T>> {
            let order_ids = match side {
                OrderSide::Sell => EntitySellOrders::<T>::get(entity_id),
                OrderSide::Buy => EntityBuyOrders::<T>::get(entity_id),
            };
            // H3 审计修复: 过滤过期订单，防止 on_idle 清理滞后导致过期单参与撮合
            let now = <frame_system::Pallet<T>>::block_number();
            let mut orders: Vec<TradeOrder<T>> = order_ids
                .iter()
                .filter_map(|&id| Orders::<T>::get(id))
                .filter(|o| {
                    o.channel == PaymentChannel::NEX &&
                    (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled) &&
                    now <= o.expires_at
                })
                .collect();
            match side {
                OrderSide::Sell => orders.sort_by(|a, b| a.price.cmp(&b.price)),
                OrderSide::Buy => orders.sort_by(|a, b| b.price.cmp(&a.price)),
            }
            orders
        }

        /// 获取排序后的卖单列表（按价格升序）
        pub fn get_sorted_sell_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
            Self::get_sorted_orders(entity_id, OrderSide::Sell)
        }

        /// 获取排序后的买单列表（按价格降序）
        pub fn get_sorted_buy_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
            Self::get_sorted_orders(entity_id, OrderSide::Buy)
        }

        /// 执行市价买入
        fn do_market_buy(
            buyer: &T::AccountId,
            entity_id: u64,
            mut remaining: T::TokenBalance,
            max_cost: BalanceOf<T>,
            sell_orders: &mut Vec<TradeOrder<T>>,
        ) -> Result<(T::TokenBalance, BalanceOf<T>, BalanceOf<T>), DispatchError> {
            let mut total_filled: T::TokenBalance = Zero::zero();
            let mut total_next: BalanceOf<T> = Zero::zero();
            let mut total_fees: BalanceOf<T> = Zero::zero();

            let fee_rate = Self::get_fee_rate(entity_id);

            for order in sell_orders.iter_mut() {
                if remaining.is_zero() {
                    break;
                }

                // 计算可成交数量
                let available = order.token_amount.saturating_sub(order.filled_amount);
                let fill_amount = remaining.min(available);

                // 计算成本
                let fill_u128: u128 = fill_amount.into();
                let cost = Self::calculate_total_next(fill_u128, order.price)?;

                // P1 修复: 滑点边界时部分成交而非跳单
                let (cost, fill_amount) = if total_next.saturating_add(cost) > max_cost {
                    let budget_left = max_cost.saturating_sub(total_next);
                    if budget_left.is_zero() {
                        break;
                    }
                    // 用剩余预算反算能买多少 Token
                    let price_u128: u128 = order.price.into();
                    if price_u128 == 0 { break; }
                    let affordable_tokens: u128 = budget_left.into() / price_u128;
                    if affordable_tokens == 0 { break; }
                    let partial: T::TokenBalance = affordable_tokens.min(fill_amount.into()).into();
                    let partial_cost = Self::calculate_total_next(partial.into(), order.price)?;
                    (partial_cost, partial)
                } else {
                    (cost, fill_amount)
                };

                if fill_amount.is_zero() { break; }

                // 计算手续费
                let fee = Self::calculate_fee(cost, fee_rate);

                // 执行转账
                // buyer 支付 NEX → maker
                T::Currency::transfer(
                    buyer,
                    &order.maker,
                    cost.saturating_sub(fee),
                    ExistenceRequirement::KeepAlive,
                )?;

                // 手续费转给实体所有者
                if !fee.is_zero() {
                    if let Some(entity_owner) = T::EntityProvider::entity_owner(entity_id) {
                        T::Currency::transfer(
                            buyer,
                            &entity_owner,
                            fee,
                            ExistenceRequirement::KeepAlive,
                        )?;
                    }
                }

                // Token: maker → buyer（从 maker 的 reserved 转出）
                T::TokenProvider::repatriate_reserved(
                    entity_id,
                    &order.maker,
                    buyer,
                    fill_amount,
                )?;

                // 更新订单
                Self::update_order_fill(order, entity_id, fill_amount);

                // 累计
                total_filled = total_filled.saturating_add(fill_amount);
                total_next = total_next.saturating_add(cost);
                total_fees = total_fees.saturating_add(fee);
                remaining = remaining.saturating_sub(fill_amount);
            }

            // 更新统计和最优价格
            if !total_filled.is_zero() {
                MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                    stats.total_trades = stats.total_trades.saturating_add(1);
                    stats.total_volume_nex = stats.total_volume_nex.saturating_add(total_next.into());
                    stats.total_fees_cos = stats.total_fees_cos.saturating_add(total_fees.into());
                });

                // 更新最优价格和 TWAP（使用加权平均价格）
                Self::update_best_prices(entity_id);
                if !total_filled.is_zero() {
                    let avg_price = total_next.checked_div(&total_filled.into().into()).unwrap_or_else(Zero::zero);
                    Self::on_trade_completed(entity_id, avg_price);
                }
            }

            Ok((total_filled, total_next, total_fees))
        }

        /// 执行市价卖出
        fn do_market_sell(
            seller: &T::AccountId,
            entity_id: u64,
            mut remaining: T::TokenBalance,
            min_receive: BalanceOf<T>,
            buy_orders: &mut Vec<TradeOrder<T>>,
        ) -> Result<(T::TokenBalance, BalanceOf<T>, BalanceOf<T>), DispatchError> {
            let mut total_filled: T::TokenBalance = Zero::zero();
            let mut total_receive: BalanceOf<T> = Zero::zero();
            let mut total_fees: BalanceOf<T> = Zero::zero();

            let fee_rate = Self::get_fee_rate(entity_id);

            for order in buy_orders.iter_mut() {
                if remaining.is_zero() {
                    break;
                }

                // 计算可成交数量
                let available = order.token_amount.saturating_sub(order.filled_amount);
                let fill_amount = remaining.min(available);

                // 计算收入
                let fill_u128: u128 = fill_amount.into();
                let gross = Self::calculate_total_next(fill_u128, order.price)?;

                // 计算手续费
                let fee = Self::calculate_fee(gross, fee_rate);
                let net = gross.saturating_sub(fee);

                // P1 修复: 滑点检查移到转账前（与 market_buy 一致）
                // 预估成交后总收入是否满足 min_receive
                let projected_receive = total_receive.saturating_add(net);
                let projected_remaining = remaining.saturating_sub(fill_amount);
                if projected_remaining.is_zero() && projected_receive < min_receive {
                    // 最后一笔成交后仍不满足 min_receive，直接失败
                    return Err(Error::<T>::SlippageExceeded.into());
                }

                // P2 修复: 使用 repatriate_reserved 替代 unreserve→transfer
                // seller 收到 net (= gross - fee)
                T::Currency::repatriate_reserved(
                    &order.maker, seller, net,
                    frame_support::traits::BalanceStatus::Free,
                )?;
                // fee 给 entity_owner
                if !fee.is_zero() {
                    if let Some(entity_owner) = T::EntityProvider::entity_owner(entity_id) {
                        T::Currency::repatriate_reserved(
                            &order.maker, &entity_owner, fee,
                            frame_support::traits::BalanceStatus::Free,
                        )?;
                    }
                }

                // Token: seller → maker（先锁定 seller 的 Token，再转给 maker）
                T::TokenProvider::reserve(entity_id, seller, fill_amount)?;
                T::TokenProvider::repatriate_reserved(
                    entity_id,
                    seller,
                    &order.maker,
                    fill_amount,
                )?;

                // 更新订单
                Self::update_order_fill(order, entity_id, fill_amount);

                // 累计
                total_filled = total_filled.saturating_add(fill_amount);
                total_receive = total_receive.saturating_add(net);
                total_fees = total_fees.saturating_add(fee);
                remaining = remaining.saturating_sub(fill_amount);
            }

            // 更新统计和最优价格
            if !total_filled.is_zero() {
                MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                    stats.total_trades = stats.total_trades.saturating_add(1);
                    stats.total_volume_nex = stats.total_volume_nex.saturating_add(total_receive.saturating_add(total_fees).into());
                    stats.total_fees_cos = stats.total_fees_cos.saturating_add(total_fees.into());
                });

                // 更新最优价格和 TWAP
                Self::update_best_prices(entity_id);
                let total_gross = total_receive.saturating_add(total_fees);
                if !total_gross.is_zero() {
                    let avg_price = total_gross.checked_div(&total_filled.into().into()).unwrap_or_else(Zero::zero);
                    Self::on_trade_completed(entity_id, avg_price);
                }
            }

            Ok((total_filled, total_receive, total_fees))
        }

        /// 更新最优买卖价格
        fn update_best_prices(entity_id: u64) {
            // 更新最优卖价
            if let Some(best_ask) = Self::calculate_best_ask(entity_id) {
                BestAsk::<T>::insert(entity_id, best_ask);
            } else {
                BestAsk::<T>::remove(entity_id);
            }

            // 更新最优买价
            if let Some(best_bid) = Self::calculate_best_bid(entity_id) {
                BestBid::<T>::insert(entity_id, best_bid);
            } else {
                BestBid::<T>::remove(entity_id);
            }
        }

        /// 更新最新成交价
        fn update_last_trade_price(entity_id: u64, price: BalanceOf<T>) {
            LastTradePrice::<T>::insert(entity_id, price);
        }

        // ==================== Phase 5: TWAP 价格预言机内部函数 ====================

        /// 更新 TWAP 累积器（每次成交时调用）
        /// P1 安全修复: 添加异常价格过滤，防止价格操纵
        fn update_twap_accumulator(entity_id: u64, trade_price: BalanceOf<T>) {
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            TwapAccumulators::<T>::mutate(entity_id, |maybe_acc| {
                let acc = maybe_acc.get_or_insert_with(|| TwapAccumulator {
                    current_cumulative: 0,
                    current_block,
                    last_price: trade_price,
                    trade_count: 0,
                    hour_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    day_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    week_snapshot: PriceSnapshot { cumulative_price: 0, block_number: current_block },
                    last_hour_update: current_block,
                    last_day_update: current_block,
                    last_week_update: current_block,
                });

                // P1: 异常价格过滤 - 如果价格偏离上次价格超过 100%，使用加权平均
                let filtered_price = if acc.trade_count > 0 && !acc.last_price.is_zero() {
                    let last_price_u128: u128 = acc.last_price.into();
                    let trade_price_u128: u128 = trade_price.into();
                    let max_deviation = last_price_u128; // 100% 偏离
                    
                    let deviation = if trade_price_u128 > last_price_u128 {
                        trade_price_u128.saturating_sub(last_price_u128)
                    } else {
                        last_price_u128.saturating_sub(trade_price_u128)
                    };
                    
                    if deviation > max_deviation {
                        // 异常价格: 限制价格变动幅度为上次价格的 50%
                        // 如果新价格过高，使用 last_price * 1.5
                        // 如果新价格过低，使用 last_price * 0.5
                        if trade_price_u128 > last_price_u128 {
                            // 价格上涨过快，限制为 +50%
                            acc.last_price.saturating_mul(3u32.into()) / 2u32.into()
                        } else {
                            // 价格下跌过快，限制为 -50%
                            acc.last_price / 2u32.into()
                        }
                    } else {
                        trade_price
                    }
                } else {
                    trade_price
                };

                // 计算自上次更新以来经过的区块数
                let blocks_elapsed = current_block.saturating_sub(acc.current_block);

                // 更新累积价格: cumulative += last_price × blocks_elapsed
                if blocks_elapsed > 0 {
                    let price_u128: u128 = acc.last_price.into();
                    acc.current_cumulative = acc.current_cumulative
                        .saturating_add(price_u128.saturating_mul(blocks_elapsed as u128));
                }

                // 更新当前状态（使用过滤后的价格）
                acc.current_block = current_block;
                acc.last_price = filtered_price;
                acc.trade_count = acc.trade_count.saturating_add(1);

                // 滚动更新快照
                let blocks_per_hour = T::BlocksPerHour::get();
                let blocks_per_day = T::BlocksPerDay::get();
                let blocks_per_week = T::BlocksPerWeek::get();

                // 更新 1小时快照（每 10 分钟更新一次，即 blocks_per_hour / 6）
                let hour_update_interval = blocks_per_hour / 6;
                if current_block.saturating_sub(acc.last_hour_update) >= hour_update_interval {
                    acc.hour_snapshot = PriceSnapshot {
                        cumulative_price: acc.current_cumulative,
                        block_number: current_block,
                    };
                    acc.last_hour_update = current_block;
                }

                // 更新 24小时快照（每小时更新一次）
                if current_block.saturating_sub(acc.last_day_update) >= blocks_per_hour {
                    acc.day_snapshot = PriceSnapshot {
                        cumulative_price: acc.current_cumulative,
                        block_number: current_block,
                    };
                    acc.last_day_update = current_block;
                }

                // 更新 7天快照（每天更新一次）
                if current_block.saturating_sub(acc.last_week_update) >= blocks_per_day {
                    acc.week_snapshot = PriceSnapshot {
                        cumulative_price: acc.current_cumulative,
                        block_number: current_block,
                    };
                    acc.last_week_update = current_block;
                }
            });
        }

        /// 计算指定周期的 TWAP
        pub fn calculate_twap(entity_id: u64, period: TwapPeriod) -> Option<BalanceOf<T>> {
            let acc = TwapAccumulators::<T>::get(entity_id)?;
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            // 获取对应周期的快照
            let snapshot = match period {
                TwapPeriod::OneHour => &acc.hour_snapshot,
                TwapPeriod::OneDay => &acc.day_snapshot,
                TwapPeriod::OneWeek => &acc.week_snapshot,
            };

            // 计算当前累积价格（包含自上次更新以来的部分）
            let blocks_since_update = current_block.saturating_sub(acc.current_block);
            let price_u128: u128 = acc.last_price.into();
            let current_cumulative = acc.current_cumulative
                .saturating_add(price_u128.saturating_mul(blocks_since_update as u128));

            // 计算区块差
            let block_diff = current_block.saturating_sub(snapshot.block_number);
            if block_diff == 0 {
                return Some(acc.last_price);
            }

            // 计算累积价格差
            let cumulative_diff = current_cumulative.saturating_sub(snapshot.cumulative_price);

            // TWAP = 累积价格差 / 区块差
            let twap_u128 = cumulative_diff / (block_diff as u128);

            Some(twap_u128.into())
        }

        /// 检查价格是否偏离参考价格过大
        ///
        /// 参考价格优先级：
        /// 1. 如果三周期 TWAP 数据都充足，使用 1小时 TWAP
        /// 2. 如果 TWAP 数据不足但有初始价格，使用实体所有者设定的初始价格
        /// 3. 如果都没有，跳过检查
        ///
        /// 三周期 TWAP 充足条件：
        /// - 成交量 >= min_trades_for_twap
        /// - 1小时快照已更新（距离当前 >= 1小时）
        /// - 24小时快照已更新（距离当前 >= 24小时）
        /// - 7天快照已更新（距离当前 >= 7天）
        pub fn check_price_deviation(
            entity_id: u64,
            price: BalanceOf<T>,
        ) -> Result<(), Error<T>> {
            // 获取价格保护配置
            let config = PriceProtection::<T>::get(entity_id).unwrap_or_default();

            // 如果未启用价格保护，直接通过
            if !config.enabled {
                return Ok(());
            }

            // 检查熔断状态
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
            if config.circuit_breaker_active && current_block < config.circuit_breaker_until {
                return Err(Error::<T>::MarketCircuitBreakerActive);
            }

            // 获取参考价格
            let reference_price: Option<BalanceOf<T>> = {
                // 获取 TWAP 累积器
                let acc = TwapAccumulators::<T>::get(entity_id);

                match acc {
                    Some(ref a) if Self::is_twap_data_sufficient(a, current_block, &config) => {
                        // 三周期 TWAP 数据充足，使用 1小时 TWAP
                        Self::calculate_twap(entity_id, TwapPeriod::OneHour)
                    }
                    _ => {
                        // TWAP 数据不足，使用实体所有者设定的初始价格
                        config.initial_price
                    }
                }
            };

            // 如果没有参考价格，跳过检查
            let ref_price = match reference_price {
                Some(p) => p,
                None => return Ok(()),
            };

            // 计算偏离度 (基点)
            let price_u128: u128 = price.into();
            let ref_price_u128: u128 = ref_price.into();

            if ref_price_u128 == 0 {
                return Ok(());
            }

            let deviation_bps = if price_u128 > ref_price_u128 {
                ((price_u128 - ref_price_u128) * 10000 / ref_price_u128).min(u16::MAX as u128) as u16
            } else {
                ((ref_price_u128 - price_u128) * 10000 / ref_price_u128).min(u16::MAX as u128) as u16
            };

            // 检查是否超过最大偏离
            if deviation_bps > config.max_price_deviation {
                return Err(Error::<T>::PriceDeviationTooHigh);
            }

            Ok(())
        }

        /// 检查三周期 TWAP 数据是否充足
        ///
        /// 条件：
        /// 1. 成交量 >= min_trades_for_twap
        /// 2. 1小时快照已有足够历史（当前区块 - 快照区块 >= BlocksPerHour）
        /// 3. 24小时快照已有足够历史（当前区块 - 快照区块 >= BlocksPerDay）
        /// 4. 7天快照已有足够历史（当前区块 - 快照区块 >= BlocksPerWeek）
        fn is_twap_data_sufficient(
            acc: &TwapAccumulator<BalanceOf<T>>,
            current_block: u32,
            config: &PriceProtectionConfig<BalanceOf<T>>,
        ) -> bool {
            // 检查成交量
            if acc.trade_count < config.min_trades_for_twap {
                return false;
            }

            let blocks_per_hour = T::BlocksPerHour::get();
            let blocks_per_day = T::BlocksPerDay::get();
            let blocks_per_week = T::BlocksPerWeek::get();

            // 检查 1小时快照是否有足够历史
            let hour_history = current_block.saturating_sub(acc.hour_snapshot.block_number);
            if hour_history < blocks_per_hour {
                return false;
            }

            // 检查 24小时快照是否有足够历史
            let day_history = current_block.saturating_sub(acc.day_snapshot.block_number);
            if day_history < blocks_per_day {
                return false;
            }

            // 检查 7天快照是否有足够历史
            let week_history = current_block.saturating_sub(acc.week_snapshot.block_number);
            if week_history < blocks_per_week {
                return false;
            }

            true
        }

        /// 检查并触发熔断机制
        fn check_circuit_breaker(entity_id: u64, current_price: BalanceOf<T>) {
            let config = match PriceProtection::<T>::get(entity_id) {
                Some(c) => c,
                None => return,
            };

            if !config.enabled {
                return;
            }

            // 使用 7天 TWAP 判断熔断
            let twap_7d = match Self::calculate_twap(entity_id, TwapPeriod::OneWeek) {
                Some(t) => t,
                None => return,
            };

            let price_u128: u128 = current_price.into();
            let twap_u128: u128 = twap_7d.into();

            if twap_u128 == 0 {
                return;
            }

            let deviation_bps = if price_u128 > twap_u128 {
                ((price_u128 - twap_u128) * 10000 / twap_u128).min(u16::MAX as u128) as u16
            } else {
                ((twap_u128 - price_u128) * 10000 / twap_u128).min(u16::MAX as u128) as u16
            };

            // 如果偏离超过熔断阈值，触发熔断
            if deviation_bps > config.circuit_breaker_threshold {
                let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                let until_block = current_block.saturating_add(T::CircuitBreakerDuration::get());

                PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                    if let Some(c) = maybe_config {
                        c.circuit_breaker_active = true;
                        c.circuit_breaker_until = until_block;
                    }
                });

                Self::deposit_event(Event::CircuitBreakerTriggered {
                    entity_id,
                    current_price,
                    twap_7d,
                    deviation_bps,
                    until_block,
                });
            }
        }

        /// 在成交后更新 TWAP 并检查熔断
        fn on_trade_completed(entity_id: u64, trade_price: BalanceOf<T>) {
            // 更新 TWAP 累积器
            Self::update_twap_accumulator(entity_id, trade_price);

            // 更新最新成交价
            Self::update_last_trade_price(entity_id, trade_price);

            // L1: 发出 TwapUpdated 事件
            let twap_1h = Self::calculate_twap(entity_id, TwapPeriod::OneHour);
            let twap_24h = Self::calculate_twap(entity_id, TwapPeriod::OneDay);
            let twap_7d = Self::calculate_twap(entity_id, TwapPeriod::OneWeek);
            Self::deposit_event(Event::TwapUpdated {
                entity_id,
                new_price: trade_price,
                twap_1h,
                twap_24h,
                twap_7d,
            });

            // 检查熔断
            Self::check_circuit_breaker(entity_id, trade_price);
        }
    }
}

// ==================== 公共查询接口 ====================

impl<T: Config> Pallet<T> {
    /// 获取实体卖单列表
    pub fn get_sell_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
        EntitySellOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
            .collect()
    }

    /// 获取实体买单列表
    pub fn get_buy_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
        EntityBuyOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
            .collect()
    }

    /// 获取用户订单列表
    pub fn get_user_orders(user: &T::AccountId) -> Vec<TradeOrder<T>> {
        UserOrders::<T>::get(user)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .collect()
    }

    // ==================== Phase 4: 订单簿深度查询接口 ====================

    /// 获取订单簿深度
    ///
    /// # 参数
    /// - `entity_id`: 实体 ID
    /// - `depth`: 返回的档位数量（每边）
    pub fn get_order_book_depth(entity_id: u64, depth: u32) -> OrderBookDepth<BalanceOf<T>, T::TokenBalance> {
        use sp_runtime::traits::{Saturating, SaturatedConversion};

        let asks = Self::aggregate_price_levels(entity_id, OrderSide::Sell, depth);
        let bids = Self::aggregate_price_levels(entity_id, OrderSide::Buy, depth);

        let best_ask = asks.first().map(|l| l.price);
        let best_bid = bids.first().map(|l| l.price);

        let spread = match (best_ask, best_bid) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask.saturating_sub(bid)),
            _ => None,
        };

        let block_number = <frame_system::Pallet<T>>::block_number();

        OrderBookDepth {
            entity_id,
            asks,
            bids,
            best_ask,
            best_bid,
            spread,
            block_number: block_number.saturated_into(),
        }
    }

    /// 聚合价格档位
    fn aggregate_price_levels(
        entity_id: u64,
        side: OrderSide,
        max_levels: u32,
    ) -> Vec<PriceLevel<BalanceOf<T>, T::TokenBalance>> {
        use alloc::collections::BTreeMap;
        use sp_runtime::traits::{Saturating, Zero};

        let orders = match side {
            OrderSide::Sell => Self::get_sorted_sell_orders(entity_id),
            OrderSide::Buy => Self::get_sorted_buy_orders(entity_id),
        };

        // 按价格聚合
        let mut price_map: BTreeMap<u128, (T::TokenBalance, u32)> = BTreeMap::new();

        for order in orders.iter() {
            let available = order.token_amount.saturating_sub(order.filled_amount);
            if available.is_zero() {
                continue;
            }

            let price_key: u128 = order.price.into();
            let entry = price_map.entry(price_key).or_insert((Zero::zero(), 0));
            entry.0 = entry.0.saturating_add(available);
            entry.1 = entry.1.saturating_add(1);
        }

        // 转换为 Vec 并限制数量
        let mut levels: Vec<PriceLevel<BalanceOf<T>, T::TokenBalance>> = price_map
            .into_iter()
            .map(|(price, (amount, count))| PriceLevel {
                price: price.into(),
                total_amount: amount,
                order_count: count,
            })
            .collect();

        // 卖单按价格升序，买单按价格降序（已在 get_sorted_* 中排序）
        if side == OrderSide::Buy {
            levels.reverse();
        }

        levels.truncate(max_levels as usize);
        levels
    }

    /// 获取最优买卖价
    pub fn get_best_prices(entity_id: u64) -> (Option<BalanceOf<T>>, Option<BalanceOf<T>>) {
        let best_ask = Self::calculate_best_ask(entity_id);
        let best_bid = Self::calculate_best_bid(entity_id);
        (best_ask, best_bid)
    }

    /// 计算最优卖价
    fn calculate_best_ask(entity_id: u64) -> Option<BalanceOf<T>> {
        let now = <frame_system::Pallet<T>>::block_number();
        EntitySellOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                o.channel == PaymentChannel::NEX &&
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled) &&
                now <= o.expires_at
            })
            .map(|o| o.price)
            .min()
    }

    /// 计算最优买价
    fn calculate_best_bid(entity_id: u64) -> Option<BalanceOf<T>> {
        let now = <frame_system::Pallet<T>>::block_number();
        EntityBuyOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                o.channel == PaymentChannel::NEX &&
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled) &&
                now <= o.expires_at
            })
            .map(|o| o.price)
            .max()
    }

    /// 获取买卖价差
    pub fn get_spread(entity_id: u64) -> Option<BalanceOf<T>> {
        use sp_runtime::traits::Saturating;

        let (best_ask, best_bid) = Self::get_best_prices(entity_id);
        match (best_ask, best_bid) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask.saturating_sub(bid)),
            _ => None,
        }
    }

    /// 获取市场摘要
    pub fn get_market_summary(entity_id: u64) -> MarketSummary<BalanceOf<T>, T::TokenBalance> {
        use sp_runtime::traits::{Saturating, Zero};

        let (best_ask, best_bid) = Self::get_best_prices(entity_id);
        let last_price = LastTradePrice::<T>::get(entity_id);

        // 计算卖单总量
        let total_ask_amount: T::TokenBalance = EntitySellOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| o.channel == PaymentChannel::NEX &&
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled))
            .fold(Zero::zero(), |acc: T::TokenBalance, o| {
                acc.saturating_add(o.token_amount.saturating_sub(o.filled_amount))
            });

        // 计算买单总量
        let total_bid_amount: T::TokenBalance = EntityBuyOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| o.channel == PaymentChannel::NEX &&
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled))
            .fold(Zero::zero(), |acc: T::TokenBalance, o| {
                acc.saturating_add(o.token_amount.saturating_sub(o.filled_amount))
            });

        MarketSummary {
            best_ask,
            best_bid,
            high_24h: Zero::zero(), // TODO: 需要历史数据
            low_24h: Zero::zero(),  // TODO: 需要历史数据
            volume_24h: Zero::zero(), // TODO: 需要历史数据
            last_price,
            total_ask_amount,
            total_bid_amount,
        }
    }

    /// 获取订单簿快照（简化版）
    pub fn get_order_book_snapshot(entity_id: u64) -> (Vec<(BalanceOf<T>, T::TokenBalance)>, Vec<(BalanceOf<T>, T::TokenBalance)>) {
        let depth = Self::get_order_book_depth(entity_id, 20);

        let asks: Vec<(BalanceOf<T>, T::TokenBalance)> = depth.asks
            .into_iter()
            .map(|l| (l.price, l.total_amount))
            .collect();

        let bids: Vec<(BalanceOf<T>, T::TokenBalance)> = depth.bids
            .into_iter()
            .map(|l| (l.price, l.total_amount))
            .collect();

        (asks, bids)
    }

    // ==================== P2 修复: USDT 订单簿查询接口 ====================

    /// 获取 USDT 卖单列表（按 usdt_price 升序）
    pub fn get_usdt_sell_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
        let mut orders: Vec<TradeOrder<T>> = EntitySellOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                o.channel == PaymentChannel::USDT &&
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
            })
            .collect();
        orders.sort_by(|a, b| a.usdt_price.cmp(&b.usdt_price));
        orders
    }

    /// 获取 USDT 买单列表（按 usdt_price 降序）
    pub fn get_usdt_buy_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
        let mut orders: Vec<TradeOrder<T>> = EntityBuyOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                o.channel == PaymentChannel::USDT &&
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
            })
            .collect();
        orders.sort_by(|a, b| b.usdt_price.cmp(&a.usdt_price));
        orders
    }

    /// 获取 USDT 最优买卖价
    pub fn get_usdt_best_prices(entity_id: u64) -> (Option<u64>, Option<u64>) {
        let best_ask = Self::get_usdt_sell_orders(entity_id).first().map(|o| o.usdt_price);
        let best_bid = Self::get_usdt_buy_orders(entity_id).first().map(|o| o.usdt_price);
        (best_ask, best_bid)
    }

    /// 获取 USDT 订单簿深度（按 usdt_price 聚合）
    pub fn get_usdt_order_book_depth(entity_id: u64, depth: u32)
        -> (Vec<(u64, T::TokenBalance, u32)>, Vec<(u64, T::TokenBalance, u32)>)
    {
        use alloc::collections::BTreeMap;
        use sp_runtime::traits::{Saturating, Zero};

        // 卖单按价格聚合
        let sell_orders = Self::get_usdt_sell_orders(entity_id);
        let mut ask_map: BTreeMap<u64, (T::TokenBalance, u32)> = BTreeMap::new();
        for o in sell_orders {
            let available = o.token_amount.saturating_sub(o.filled_amount);
            if !available.is_zero() {
                let entry = ask_map.entry(o.usdt_price).or_insert((Zero::zero(), 0));
                entry.0 = entry.0.saturating_add(available);
                entry.1 += 1;
            }
        }
        let asks: Vec<(u64, T::TokenBalance, u32)> = ask_map.into_iter()
            .take(depth as usize)
            .map(|(price, (amount, count))| (price, amount, count))
            .collect();

        // 买单按价格聚合（降序）
        let buy_orders = Self::get_usdt_buy_orders(entity_id);
        let mut bid_map: BTreeMap<u64, (T::TokenBalance, u32)> = BTreeMap::new();
        for o in buy_orders {
            let available = o.token_amount.saturating_sub(o.filled_amount);
            if !available.is_zero() {
                let entry = bid_map.entry(o.usdt_price).or_insert((Zero::zero(), 0));
                entry.0 = entry.0.saturating_add(available);
                entry.1 += 1;
            }
        }
        let bids: Vec<(u64, T::TokenBalance, u32)> = bid_map.into_iter()
            .rev()
            .take(depth as usize)
            .map(|(price, (amount, count))| (price, amount, count))
            .collect();

        (asks, bids)
    }
}

// ==================== EntityTokenPriceProvider 实现 ====================

impl<T: Config> pallet_entity_common::EntityTokenPriceProvider for Pallet<T> {
    type Balance = BalanceOf<T>;

    fn get_token_price(entity_id: u64) -> Option<BalanceOf<T>> {
        use pallet::{TwapPeriod, LastTradePrice, PriceProtection};
        // 优先级: 1h TWAP → LastTradePrice → initial_price
        Self::calculate_twap(entity_id, TwapPeriod::OneHour)
            .or_else(|| LastTradePrice::<T>::get(entity_id))
            .or_else(|| {
                PriceProtection::<T>::get(entity_id)
                    .and_then(|config| config.initial_price)
            })
    }

    fn get_token_price_usdt(entity_id: u64) -> Option<u64> {
        use pallet_entity_common::PricingProvider;
        // token_nex_price × nex_usdt_rate / 10^12
        let token_nex: u128 = <Self as pallet_entity_common::EntityTokenPriceProvider>::get_token_price(entity_id)?.into();
        let nex_usdt = T::NexUsdtPrice::get_nex_usdt_price();
        if nex_usdt == 0 {
            return None;
        }
        // token_nex (10^12 精度) × nex_usdt (10^6 精度) / 10^12 = USDT per Token (10^6 精度)
        let usdt_price = token_nex
            .checked_mul(nex_usdt as u128)?
            .checked_div(1_000_000_000_000u128)?;
        Some(usdt_price as u64)
    }

    fn token_price_confidence(entity_id: u64) -> u8 {
        use sp_runtime::SaturatedConversion;
        use pallet::{TwapAccumulators, TwapPeriod, LastTradePrice, PriceProtection};

        let acc = match TwapAccumulators::<T>::get(entity_id) {
            Some(a) => a,
            None => {
                // 无累积器：仅检查 initial_price
                return if PriceProtection::<T>::get(entity_id)
                    .and_then(|c| c.initial_price)
                    .is_some()
                {
                    35
                } else {
                    0
                };
            }
        };

        let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
        let blocks_since = current_block.saturating_sub(acc.current_block);

        // 超过 ~4h 无交易视为过时
        let stale = blocks_since > 2400;

        let has_twap = Self::calculate_twap(entity_id, TwapPeriod::OneHour).is_some();
        let has_last_trade = LastTradePrice::<T>::get(entity_id).is_some();

        if stale {
            if has_twap { 25 } else if has_last_trade { 15 } else { 10 }
        } else if has_twap && acc.trade_count >= 100 {
            95
        } else if has_twap {
            80
        } else if has_last_trade {
            65
        } else {
            35
        }
    }

    fn is_token_price_stale(entity_id: u64, max_age_blocks: u32) -> bool {
        use sp_runtime::SaturatedConversion;
        use pallet::TwapAccumulators;

        match TwapAccumulators::<T>::get(entity_id) {
            Some(acc) => {
                let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                current_block.saturating_sub(acc.current_block) > max_age_blocks
            }
            None => true, // 无累积器 = 无交易数据 = 过时
        }
    }
}
