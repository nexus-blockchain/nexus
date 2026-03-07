//! # 实体代币交易市场模块 (pallet-entity-market)
//!
//! ## 概述
//!
//! 本模块实现实体代币的 P2P 交易市场，支持：
//! - NEX 通道：使用原生 NEX 代币买卖实体代币（链上即时结算）
//!
//! ## 交易模式
//!
//! - 限价单：挂单等待撮合
//! - 吃单：直接成交对手盘订单
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-02-01): 初始版本，实现 NEX 通道限价单
//! - v0.2.0 (2026-02-01): Phase 2，移除 USDT 通道，仅保留 NEX 交易

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

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
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{DisclosureProvider, EntityProvider, EntityTokenProvider, KycProvider, PricingProvider};
    use sp_runtime::traits::{CheckedAdd, CheckedMul, CheckedSub, Saturating, Zero};
    use sp_runtime::SaturatedConversion;

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


    /// 订单类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum OrderType {
        /// 限价单（挂单等待撮合）
        #[default]
        Limit,
        /// 市价单（立即以最优价成交）
        Market,
        /// IOC (Immediate or Cancel) — 立即成交能成交的部分，剩余取消
        ImmediateOrCancel,
        /// FOK (Fill or Kill) — 全部成交或全部取消
        FillOrKill,
        /// Post-Only — 仅挂单，不立即撮合（做市商常用）
        PostOnly,
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
        /// 代币数量（总量）
        pub token_amount: T::TokenBalance,
        /// 已成交数量
        pub filled_amount: T::TokenBalance,
        /// 价格（每个 Token 的 NEX 价格）
        /// 市价单时为 0
        pub price: BalanceOf<T>,
        /// 订单状态
        pub status: OrderStatus,
        /// 创建区块
        pub created_at: BlockNumberFor<T>,
        /// 过期区块
        pub expires_at: BlockNumberFor<T>,
    }

    /// 实体市场配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MarketConfig {
        /// 是否启用 NEX 交易
        pub nex_enabled: bool,
        /// 最小订单 Token 数量
        pub min_order_amount: u128,
        /// 订单有效期（区块数）
        pub order_ttl: u32,
        /// 市场是否暂停（轻量暂停开关）
        pub paused: bool,
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

    // ==================== P1: 交易历史数据结构 ====================

    /// 成交记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct TradeRecord<T: Config> {
        /// 成交 ID
        pub trade_id: u64,
        /// 订单 ID
        pub order_id: u64,
        /// 实体 ID
        pub entity_id: u64,
        /// 挂单方 (maker)
        pub maker: T::AccountId,
        /// 吃单方 (taker)
        pub taker: T::AccountId,
        /// 交易方向（从 taker 视角）
        pub side: OrderSide,
        /// 成交 Token 数量
        pub token_amount: T::TokenBalance,
        /// 成交价格
        pub price: BalanceOf<T>,
        /// NEX 总额
        pub nex_amount: BalanceOf<T>,
        /// 成交区块
        pub block_number: BlockNumberFor<T>,
    }

    // ==================== P3: 周期统计数据结构 ====================

    /// 日统计数据
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DailyStats<Balance> {
        /// 开盘价
        pub open_price: Balance,
        /// 最高价
        pub high_price: Balance,
        /// 最低价
        pub low_price: Balance,
        /// 收盘价（最新成交价）
        pub close_price: Balance,
        /// 24h 成交量 (NEX)
        pub volume_nex: u128,
        /// 24h 成交笔数
        pub trade_count: u32,
        /// 统计起始区块
        pub period_start: u32,
    }

    // ==================== P6: 市场状态枚举 ====================

    /// 市场状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum MarketStatus {
        /// 活跃（暂停通过 MarketConfig.paused 控制）
        #[default]
        Active,
        /// 已关闭（不可恢复，所有订单已清退）
        Closed,
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

        /// 披露查询接口（黑窗口期内幕人员交易限制）
        type DisclosureProvider: DisclosureProvider<Self::AccountId>;

        /// P4: KYC 查询接口（交易前 KYC 级别检查）
        type KycProvider: pallet_entity_common::KycProvider<Self::AccountId>;

        /// P1: 每用户最大交易历史条数
        #[pallet::constant]
        type MaxTradeHistoryPerUser: Get<u32>;

        /// P2: 每用户最大订单历史条数
        #[pallet::constant]
        type MaxOrderHistoryPerUser: Get<u32>;

        /// NEX/USDT 定价接口（用于 Token→NEX→USDT 间接换算）
        type PricingProvider: PricingProvider;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: crate::weights::WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// P1 修复: on_idle 批量清理过期订单，释放 BoundedVec 名额
        /// 审计修复 M1-R8: 使用游标扫描替代 last-1000 限制，确保所有过期订单最终被清理
        /// 审计修复 M2-R8: 权重包含 proof_size 估算
        fn on_idle(_n: BlockNumberFor<T>, mut remaining_weight: Weight) -> Weight {
            let base_weight = Weight::from_parts(5_000, 64);
            let per_order_weight = Weight::from_parts(30_000, 512);
            let now = <frame_system::Pallet<T>>::block_number();
            let mut cleaned = 0u32;
            // 审计修复 M1-R9: 跟踪实际消耗的权重，返回准确值
            let mut consumed_weight = Weight::zero();
            const MAX_CLEAN_PER_BLOCK: u32 = 20;
            const SCAN_BATCH: u64 = 200;

            // 审计修复 M1-R6: 记录受影响的实体，清理后更新 BestAsk/BestBid 缓存
            let mut affected_entities: sp_std::vec::Vec<u64> = sp_std::vec::Vec::new();

            // 审计修复 M1-R8: 游标扫描 — 每块从上次位置继续，循环覆盖所有订单
            let next_id = NextOrderId::<T>::get();
            let cursor = OnIdleCursor::<T>::get();
            let start = if cursor >= next_id { 0 } else { cursor };
            let scan_end = start.saturating_add(SCAN_BATCH).min(next_id);

            for order_id in start..scan_end {
                if cleaned >= MAX_CLEAN_PER_BLOCK { break; }
                // 审计修复 L1-R9: 同时检查 ref_time 和 proof_size
                if remaining_weight.ref_time() < per_order_weight.ref_time()
                    || remaining_weight.proof_size() < per_order_weight.proof_size() { break; }

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

                        let mut expired_order = order.clone();
                        expired_order.status = OrderStatus::Expired;
                        Orders::<T>::insert(order_id, &expired_order);

                        Self::remove_from_order_book(order.entity_id, order_id, order.side);
                        UserOrders::<T>::mutate(&order.maker, |orders| {
                            orders.retain(|&id| id != order_id);
                        });
                        // 审计修复 M4-R5: 添加到已完结订单历史
                        Self::add_to_order_history(&order.maker, order_id);

                        if !affected_entities.contains(&order.entity_id) {
                            affected_entities.push(order.entity_id);
                        }

                        cleaned += 1;
                        remaining_weight = remaining_weight.saturating_sub(per_order_weight);
                        consumed_weight = consumed_weight.saturating_add(per_order_weight);
                    }
                }
                remaining_weight = remaining_weight.saturating_sub(base_weight);
                consumed_weight = consumed_weight.saturating_add(base_weight);
            }

            // 审计修复 M1-R8: 更新游标位置（到达末尾时归零重新扫描）
            if scan_end >= next_id {
                OnIdleCursor::<T>::put(0);
            } else {
                OnIdleCursor::<T>::put(scan_end);
            }

            // 审计修复 M1-R6: 更新受影响实体的最优价格缓存
            for entity_id in affected_entities {
                Self::update_best_prices(entity_id);
            }

            // 审计修复 C2-R10: on_idle 清理过期订单时发出事件，供链下索引器追踪
            if cleaned > 0 {
                Self::deposit_event(Event::ExpiredOrdersAutoCleaned { count: cleaned });
            }

            // 审计修复 M1-R9: 返回实际消耗的权重（包含所有扫描 + 清理的开销）
            consumed_weight
        }

        /// 存储迁移框架 — 检查版本并执行必要的迁移
        ///
        /// 当前版本: v0 (初始版本)
        /// 未来升级时在此添加 v0→v1, v1→v2 等迁移逻辑
        fn on_runtime_upgrade() -> Weight {
            let on_chain = Pallet::<T>::on_chain_storage_version();
            let current = Pallet::<T>::in_code_storage_version();

            if on_chain == current {
                // 版本一致，无需迁移
                return Weight::zero();
            }

            log::info!(
                target: "pallet-entity-market",
                "Running storage migration from {:?} to {:?}",
                on_chain,
                current,
            );

            let mut weight = T::DbWeight::get().reads(1); // on_chain_storage_version read

            // === 未来迁移占位 ===
            // if on_chain < 1 {
            //     weight = weight.saturating_add(migrations::v1::migrate::<T>());
            // }

            // 更新链上版本号
            current.put::<Pallet<T>>();
            weight = weight.saturating_add(T::DbWeight::get().writes(1));

            log::info!(
                target: "pallet-entity-market",
                "Storage migration completed to version {:?}",
                current,
            );

            weight
        }

        /// P6: Config 常量合理性校验
        #[cfg(test)]
        fn integrity_test() {
            assert!(T::DefaultOrderTTL::get() >= 10, "DefaultOrderTTL must be >= 10 blocks");
            assert!(T::BlocksPerHour::get() > 0, "BlocksPerHour must be > 0");
            assert!(T::BlocksPerDay::get() > T::BlocksPerHour::get(), "BlocksPerDay must be > BlocksPerHour");
            assert!(T::BlocksPerWeek::get() > T::BlocksPerDay::get(), "BlocksPerWeek must be > BlocksPerDay");
            assert!(T::CircuitBreakerDuration::get() > 0, "CircuitBreakerDuration must be > 0");
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
        StorageMap<_, Blake2_128Concat, u64, MarketConfig>;

    /// 实体市场统计
    #[pallet::storage]
    #[pallet::getter(fn market_stats)]
    pub type MarketStatsStorage<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, MarketStats, ValueQuery>;

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

    /// 全局市场暂停开关（Root 控制）
    #[pallet::storage]
    #[pallet::getter(fn global_market_paused)]
    pub type GlobalMarketPaused<T> = StorageValue<_, bool, ValueQuery>;

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

    // ==================== P1: 交易历史存储 ====================

    /// 下一个成交 ID
    #[pallet::storage]
    pub type NextTradeId<T> = StorageValue<_, u64, ValueQuery>;

    /// 成交记录存储
    #[pallet::storage]
    pub type TradeRecords<T: Config> = StorageMap<_, Blake2_128Concat, u64, TradeRecord<T>>;

    /// 用户交易历史索引（最近 N 笔，环形覆盖）
    #[pallet::storage]
    pub type UserTradeHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<200>>,
        ValueQuery,
    >;

    /// 实体交易历史索引（最近 N 笔）
    #[pallet::storage]
    pub type EntityTradeHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        BoundedVec<u64, ConstU32<500>>,
        ValueQuery,
    >;

    // ==================== P2: 订单历史存储 ====================

    /// 用户已完结订单历史（Filled/Cancelled/Expired）
    #[pallet::storage]
    pub type UserOrderHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<200>>,
        ValueQuery,
    >;

    // ==================== P3: 周期统计存储 ====================

    /// 实体当日统计
    #[pallet::storage]
    pub type EntityDailyStats<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        DailyStats<BalanceOf<T>>,
        ValueQuery,
    >;

    // ==================== P4: KYC 门槛存储 ====================

    /// 实体市场最低 KYC 级别要求（0 = 无要求）
    #[pallet::storage]
    pub type MarketKycRequirement<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        u8,
        ValueQuery,
    >;

    /// 审计修复 M1-R8: on_idle 过期订单扫描游标（cursor-based，替代 last-1000 限制）
    #[pallet::storage]
    pub type OnIdleCursor<T> = StorageValue<_, u64, ValueQuery>;

    // ==================== P6: 市场状态存储 ====================

    /// 实体市场状态（Active / Closed）
    #[pallet::storage]
    pub type MarketStatusStorage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // entity_id
        MarketStatus,
        ValueQuery,
    >;

    // ==================== P11: 全局统计存储 ====================

    /// 全局市场统计（所有市场累计）
    #[pallet::storage]
    pub type GlobalStats<T: Config> = StorageValue<_, MarketStats, ValueQuery>;

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
            entity_id: u64,
            maker: T::AccountId,
            taker: T::AccountId,
            filled_amount: T::TokenBalance,
            total_next: BalanceOf<T>,
        },
        /// 订单已取消
        OrderCancelled { order_id: u64, entity_id: u64 },
        /// 市场配置已更新
        MarketConfigured { entity_id: u64 },
        /// 市价单已执行
        MarketOrderExecuted {
            entity_id: u64,
            trader: T::AccountId,
            side: OrderSide,
            filled_amount: T::TokenBalance,
            total_next: BalanceOf<T>,
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
        /// 市场已暂停
        MarketPausedEvent { entity_id: u64 },
        /// 市场已恢复
        MarketResumedEvent { entity_id: u64 },
        /// 订单被 Root 强制取消
        OrderForceCancelled { order_id: u64 },
        /// 订单已修改
        OrderModified {
            order_id: u64,
            new_price: BalanceOf<T>,
            new_amount: T::TokenBalance,
        },
        /// 过期订单已清理（带激励）
        ExpiredOrdersCleaned {
            entity_id: u64,
            count: u32,
            cleaner: T::AccountId,
        },
        /// 全局市场暂停状态变更
        GlobalMarketPauseToggled { paused: bool },
        /// 批量订单已取消
        BatchOrdersCancelled {
            cancelled_count: u32,
            failed_count: u32,
        },
        /// P1: 成交记录已创建
        TradeExecuted {
            trade_id: u64,
            order_id: u64,
            entity_id: u64,
            maker: T::AccountId,
            taker: T::AccountId,
            side: OrderSide,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
            nex_amount: BalanceOf<T>,
        },
        /// P4: KYC 要求已设置
        KycRequirementSet {
            entity_id: u64,
            min_kyc_level: u8,
        },
        /// P6: 市场已关闭（所有订单已清退）
        MarketClosed {
            entity_id: u64,
            orders_cancelled: u32,
        },
        /// P7: 用户在指定实体的所有订单已取消
        AllEntityOrdersCancelled {
            entity_id: u64,
            user: T::AccountId,
            cancelled_count: u32,
        },
        /// P10: 市场被 Root 强制关闭
        MarketForceClosed {
            entity_id: u64,
            orders_cancelled: u32,
        },
        /// 审计修复 C2-R10: on_idle 自动清理过期订单
        ExpiredOrdersAutoCleaned {
            count: u32,
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
        /// 没有可用订单
        NoOrdersAvailable,
        /// 滑点超限
        SlippageExceeded,
        /// 价格偏离 TWAP 过大
        PriceDeviationTooHigh,
        /// 市场处于熔断状态
        MarketCircuitBreakerActive,
        /// TWAP 数据不足
        InsufficientTwapData,
        /// 基点参数无效（超过 10000）
        InvalidBasisPoints,
        /// 实体未激活（Banned/Closed）
        EntityNotActive,
        /// 订单 TTL 过短
        OrderTtlTooShort,
        /// 内幕人员黑窗口期内禁止交易
        InsiderTradingRestricted,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// 市场已暂停
        MarketPaused,
        /// 全局市场已暂停
        GlobalMarketPausedError,
        /// 订单数量低于最小值
        OrderAmountBelowMinimum,
        /// 熔断未激活
        CircuitBreakerNotActive,
        /// 修改后数量不能大于原始数量
        ModifyAmountExceedsOriginal,
        /// 订单状态无效
        InvalidOrderStatus,
        /// 批量操作数量过多
        TooManyOrders,
        /// P4: KYC 级别不足
        InsufficientKycLevel,
        /// P6: 市场已永久关闭
        MarketAlreadyClosed,
        /// P15: FOK 订单无法全部成交
        FokNotFullyFillable,
        /// P15: PostOnly 订单会立即撮合，被拒绝
        PostOnlyWouldMatch,
        /// 初始价格已设置且市场已有真实成交，不可重复设置
        InitialPriceAlreadySet,
        /// 审计修复 L1-R8: 市场未暂停（resume_market 对称性检查）
        MarketNotPaused,
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
        #[pallet::weight(T::WeightInfo::place_sell_order())]
        pub fn place_sell_order(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体和市场
            Self::ensure_market_enabled(entity_id)?;

            // P4: KYC 级别检查
            Self::ensure_kyc_requirement(entity_id, &who)?;

            // P0-a6: 内幕人员黑窗口期限制
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );

            // 验证参数
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // P1: min_order_amount 强制执行
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }

            // Phase 5: 价格偏离检查
            Self::check_price_deviation(entity_id, price)?;

            // 检查用户 Token 余额
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            ensure!(balance >= token_amount, Error::<T>::InsufficientTokenBalance);

            // 锁定 Token
            T::TokenProvider::reserve(entity_id, &who, token_amount)?;

            // P0 修复: 自动撮合价格交叉的买单
            let (crossed, _nex_received) = Self::do_cross_match(
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
                    remaining,
                    price,
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
        #[pallet::weight(T::WeightInfo::place_buy_order())]
        pub fn place_buy_order(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体和市场
            Self::ensure_market_enabled(entity_id)?;

            // P4: KYC 级别检查
            Self::ensure_kyc_requirement(entity_id, &who)?;

            // P0-a6: 内幕人员黑窗口期限制
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );

            // 验证参数
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // P1: min_order_amount 强制执行
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }

            // Phase 5: 价格偏离检查
            Self::check_price_deviation(entity_id, price)?;

            // 计算需要锁定的 NEX 总量
            let token_u128: u128 = token_amount.into();
            let total_next = Self::calculate_total_next(token_u128, price)?;

            // 锁定 NEX
            T::Currency::reserve(&who, total_next).map_err(|_| Error::<T>::InsufficientBalance)?;

            // P0 修复: 自动撮合价格交叉的卖单
            let (crossed, nex_spent) = Self::do_cross_match(
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
                    remaining,
                    price,
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
        #[pallet::weight(T::WeightInfo::take_order())]
        pub fn take_order(
            origin: OriginFor<T>,
            order_id: u64,
            amount: Option<T::TokenBalance>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取订单
            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

            // 审计修复 H1: 吃单前验证市场状态（暂停/封禁/未启用时不可吃单）
            Self::ensure_market_enabled(order.entity_id)?;
            // 审计修复 H2-R7: 吃单也需检查熔断器（不经过 check_price_deviation）
            Self::ensure_circuit_breaker_inactive(order.entity_id)?;

            // P4: KYC 级别检查
            Self::ensure_kyc_requirement(order.entity_id, &who)?;

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

            // P0-a6: 内幕人员黑窗口期限制
            ensure!(
                T::DisclosureProvider::can_insider_trade(order.entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );

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

            // 执行交易（无手续费，全额转账）
            match order.side {
                OrderSide::Sell => {
                    // 卖单：taker(买方) 支付 NEX，获得 Token
                    T::Currency::transfer(
                        &who, &order.maker, total_next,
                        ExistenceRequirement::KeepAlive,
                    )?;

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

                    T::Currency::repatriate_reserved(
                        &order.maker, &who, total_next,
                        frame_support::traits::BalanceStatus::Free,
                    )?;

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
                // P2: 添加到已完结订单历史
                Self::add_to_order_history(&order.maker, order_id);
            } else {
                order.status = OrderStatus::PartiallyFilled;
            }

            Orders::<T>::insert(order_id, &order);

            // 更新统计
            Self::update_trade_stats(order.entity_id, total_next);

            // P1: 记录成交
            let taker_side = match order.side {
                OrderSide::Sell => OrderSide::Buy,
                OrderSide::Buy => OrderSide::Sell,
            };
            Self::record_trade(
                order_id, order.entity_id, order.maker.clone(), who.clone(),
                taker_side, fill_amount, order.price, total_next,
            );

            // 更新最优价格和 TWAP
            Self::update_best_prices(order.entity_id);
            Self::on_trade_completed(order.entity_id, order.price);

            Self::deposit_event(Event::OrderFilled {
                order_id,
                entity_id: order.entity_id,
                maker: order.maker,
                taker: who,
                filled_amount: fill_amount,
                total_next,
            });

            Ok(())
        }

        /// 取消订单
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::cancel_order())]
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

            // 更新订单状态
            order.status = OrderStatus::Cancelled;
            Orders::<T>::insert(order_id, &order);

            // 从订单簿移除
            Self::remove_from_order_book(order.entity_id, order_id, order.side);

            // M2: 从用户订单列表移除
            UserOrders::<T>::mutate(&who, |orders| {
                orders.retain(|&id| id != order_id);
            });

            // P2: 添加到已完结订单历史
            Self::add_to_order_history(&who, order_id);

            // 更新最优价格
            Self::update_best_prices(order.entity_id);

            Self::deposit_event(Event::OrderCancelled { order_id, entity_id: order.entity_id });

            Ok(())
        }

        /// 配置实体市场
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::configure_market())]
        pub fn configure_market(
            origin: OriginFor<T>,
            entity_id: u64,
            nex_enabled: bool,
            min_order_amount: u128,
            order_ttl: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // P6: 市场已关闭时不允许配置
            ensure!(MarketStatusStorage::<T>::get(entity_id) != MarketStatus::Closed, Error::<T>::MarketAlreadyClosed);

            // H6 审计修复: TTL 最小值验证（防止立即过期）
            ensure!(order_ttl >= 10, Error::<T>::OrderTtlTooShort);

            let was_enabled = MarketConfigs::<T>::get(entity_id)
                .map(|c| c.nex_enabled)
                .unwrap_or(false);

            MarketConfigs::<T>::mutate(entity_id, |maybe_config| {
                let config = maybe_config.get_or_insert_with(Default::default);
                config.nex_enabled = nex_enabled;
                config.min_order_amount = min_order_amount;
                config.order_ttl = order_ttl;
            });

            // 审计修复 H2-R10: 禁用市场时自动取消所有活跃订单，退还锁定资产
            if was_enabled && !nex_enabled {
                Self::do_cancel_all_entity_orders(entity_id);
            }

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
        #[pallet::weight(T::WeightInfo::configure_price_protection())]
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
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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
        #[pallet::weight(T::WeightInfo::lift_circuit_breaker())]
        pub fn lift_circuit_breaker(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            // M3: 检查熔断是否活跃且已到期
            let config = PriceProtection::<T>::get(entity_id).unwrap_or_default();
            // 审计修复 H3: 未激活时使用正确的错误类型
            ensure!(config.circuit_breaker_active, Error::<T>::CircuitBreakerNotActive);
            ensure!(current_block >= config.circuit_breaker_until, Error::<T>::MarketCircuitBreakerActive);

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
        /// 仅在市场无真实成交时可调用（一次性设置），防止覆盖真实价格数据。
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::set_initial_price())]
        pub fn set_initial_price(
            origin: OriginFor<T>,
            entity_id: u64,
            initial_price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 验证价格
            ensure!(!initial_price.is_zero(), Error::<T>::ZeroPrice);

            // H1: 一次性限制 — 已有真实成交后禁止再设初始价格
            if let Some(acc) = TwapAccumulators::<T>::get(entity_id) {
                ensure!(acc.trade_count == 0, Error::<T>::InitialPriceAlreadySet);
            }

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
                } else if let Some(acc) = maybe_acc.as_mut() {
                    // 无真实成交时允许更新 last_price
                    // 先将旧价格的累积量写入，再切换到新价格
                    let blocks_elapsed = current_block.saturating_sub(acc.current_block);
                    if blocks_elapsed > 0 {
                        let old_price_u128: u128 = acc.last_price.into();
                        acc.current_cumulative = acc.current_cumulative
                            .saturating_add(old_price_u128.saturating_mul(blocks_elapsed as u128));
                    }
                    acc.last_price = initial_price;
                    acc.current_block = current_block;
                }
            });

            // H1: 不写 LastTradePrice — 初始价格仅用于 PriceProtection fallback，
            // 不污染 LastTradePrice（该值应仅由真实成交写入）

            Self::deposit_event(Event::InitialPriceSet { entity_id, initial_price });

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
        #[pallet::weight(T::WeightInfo::market_buy())]
        pub fn market_buy(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            max_cost: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证市场
            Self::ensure_market_enabled(entity_id)?;
            // 审计修复 H2-R7: 市价单也需检查熔断器（不经过 check_price_deviation）
            Self::ensure_circuit_breaker_inactive(entity_id)?;

            // P4: KYC 级别检查
            Self::ensure_kyc_requirement(entity_id, &who)?;

            // 审计修复 H2: 内幕人员黑窗口期限制（与限价单一致）
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );

            // 验证参数
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);
            ensure!(!max_cost.is_zero(), Error::<T>::ZeroPrice);

            // 审计修复 S3-R11: 市价单也需要 min_order_amount 检查
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }

            // 获取卖单列表（按价格升序排列）
            let mut sell_orders = Self::get_sorted_sell_orders(entity_id);
            ensure!(!sell_orders.is_empty(), Error::<T>::NoOrdersAvailable);

            // 执行市价买入
            let (filled, total_next) = Self::do_market_buy(
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
        #[pallet::weight(T::WeightInfo::market_sell())]
        pub fn market_sell(
            origin: OriginFor<T>,
            entity_id: u64,
            token_amount: T::TokenBalance,
            min_receive: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证市场
            Self::ensure_market_enabled(entity_id)?;
            // 审计修复 H2-R7: 市价单也需检查熔断器（不经过 check_price_deviation）
            Self::ensure_circuit_breaker_inactive(entity_id)?;

            // P4: KYC 级别检查
            Self::ensure_kyc_requirement(entity_id, &who)?;

            // 审计修复 H2: 内幕人员黑窗口期限制（与限价单一致）
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );

            // 验证参数
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 审计修复 S3-R11: 市价单也需要 min_order_amount 检查
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }

            // 检查用户 Token 余额
            let balance = T::TokenProvider::token_balance(entity_id, &who);
            ensure!(balance >= token_amount, Error::<T>::InsufficientTokenBalance);

            // 获取买单列表（按价格降序排列）
            let mut buy_orders = Self::get_sorted_buy_orders(entity_id);
            ensure!(!buy_orders.is_empty(), Error::<T>::NoOrdersAvailable);

            // 执行市价卖出
            let (filled, total_receive) = Self::do_market_sell(
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
            });

            Ok(())
        }

        // ==================== 新增 Extrinsics ====================

        /// Root 强制取消订单
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::force_cancel_order())]
        pub fn force_cancel_order(
            origin: OriginFor<T>,
            order_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(
                order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled,
                Error::<T>::InvalidOrderStatus
            );

            // 退还未成交部分
            let remaining = order.token_amount.saturating_sub(order.filled_amount);
            match order.side {
                OrderSide::Sell => {
                    if !remaining.is_zero() {
                        T::TokenProvider::unreserve(order.entity_id, &order.maker, remaining);
                    }
                }
                OrderSide::Buy => {
                    if !remaining.is_zero() {
                        let remaining_u128: u128 = remaining.into();
                        let refund = Self::calculate_total_next(remaining_u128, order.price)
                            .unwrap_or_else(|_| Zero::zero());
                        if !refund.is_zero() {
                            T::Currency::unreserve(&order.maker, refund);
                        }
                    }
                }
            }

            order.status = OrderStatus::Cancelled;
            Orders::<T>::insert(order_id, &order);

            Self::remove_from_order_book(order.entity_id, order_id, order.side);
            UserOrders::<T>::mutate(&order.maker, |orders| {
                orders.retain(|&id| id != order_id);
            });
            // 审计修复 M2-R5: 添加到已完结订单历史
            Self::add_to_order_history(&order.maker, order_id);

            // 审计修复 M2: 更新最优价格缓存
            Self::update_best_prices(order.entity_id);

            Self::deposit_event(Event::OrderForceCancelled { order_id });

            Ok(())
        }

        /// 暂停实体市场（实体所有者）
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::pause_market())]
        pub fn pause_market(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::entity_owner(entity_id) == Some(who.clone()), Error::<T>::NotEntityOwner);
            // 审计修复 L2-R5: 与其他 owner extrinsics 一致
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            MarketConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::MarketNotEnabled)?;
                ensure!(!config.paused, Error::<T>::MarketPaused);
                config.paused = true;
                Ok(())
            })?;

            Self::deposit_event(Event::MarketPausedEvent { entity_id });
            Ok(())
        }

        /// 恢复实体市场（实体所有者）
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::resume_market())]
        pub fn resume_market(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::entity_owner(entity_id) == Some(who.clone()), Error::<T>::NotEntityOwner);
            // 审计修复 L2-R5: 与其他 owner extrinsics 一致
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            MarketConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::MarketNotEnabled)?;
                // 审计修复 L1-R8: 与 pause_market 对称 — 未暂停时不允许 resume
                ensure!(config.paused, Error::<T>::MarketNotPaused);
                config.paused = false;
                Ok(())
            })?;

            Self::deposit_event(Event::MarketResumedEvent { entity_id });
            Ok(())
        }

        /// 批量取消用户自己的订单
        // 审计修复 M3-R9: 使用 BoundedVec 在解码阶段即限制长度
        #[pallet::call_index(28)]
        #[pallet::weight(T::WeightInfo::batch_cancel_orders(order_ids.len() as u32))]
        pub fn batch_cancel_orders(
            origin: OriginFor<T>,
            order_ids: BoundedVec<u64, ConstU32<50>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut cancelled = 0u32;
            let mut failed = 0u32;
            let mut affected_entities: Vec<u64> = Vec::new();

            for order_id in order_ids.iter() {
                if let Some(mut order) = Orders::<T>::get(order_id) {
                    if order.maker != who ||
                       (order.status != OrderStatus::Open && order.status != OrderStatus::PartiallyFilled) {
                        failed += 1;
                        continue;
                    }

                    let remaining = order.token_amount.saturating_sub(order.filled_amount);
                    match order.side {
                        OrderSide::Sell => {
                            if !remaining.is_zero() {
                                T::TokenProvider::unreserve(order.entity_id, &order.maker, remaining);
                            }
                        }
                        OrderSide::Buy => {
                            if !remaining.is_zero() {
                                let remaining_u128: u128 = remaining.into();
                                if let Ok(refund) = Self::calculate_total_next(remaining_u128, order.price) {
                                    if !refund.is_zero() {
                                        T::Currency::unreserve(&order.maker, refund);
                                    }
                                }
                            }
                        }
                    }

                    if !affected_entities.contains(&order.entity_id) {
                        affected_entities.push(order.entity_id);
                    }

                    order.status = OrderStatus::Cancelled;
                    Orders::<T>::insert(order_id, &order);
                    Self::remove_from_order_book(order.entity_id, *order_id, order.side);
                    UserOrders::<T>::mutate(&order.maker, |orders| {
                        orders.retain(|id| id != order_id);
                    });
                    // 审计修复 M1-R5: 添加到已完结订单历史
                    Self::add_to_order_history(&who, *order_id);
                    cancelled += 1;
                } else {
                    failed += 1;
                }
            }

            // 审计修复 M2: 更新受影响实体的最优价格缓存
            for &eid in affected_entities.iter() {
                Self::update_best_prices(eid);
            }

            Self::deposit_event(Event::BatchOrdersCancelled {
                cancelled_count: cancelled,
                failed_count: failed,
            });

            Ok(())
        }

        /// 清理过期订单（带激励，任何人可调用）
        #[pallet::call_index(29)]
        #[pallet::weight(T::WeightInfo::cleanup_expired_orders())]
        pub fn cleanup_expired_orders(
            origin: OriginFor<T>,
            entity_id: u64,
            max_count: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(max_count > 0 && max_count <= 100, Error::<T>::TooManyOrders);

            let now = <frame_system::Pallet<T>>::block_number();
            let mut cleaned = 0u32;

            // 清理过期卖单
            let sell_ids: Vec<u64> = EntitySellOrders::<T>::get(entity_id).into_iter().collect();
            for order_id in sell_ids {
                if cleaned >= max_count { break; }
                if let Some(mut order) = Orders::<T>::get(order_id) {
                    if now > order.expires_at &&
                       (order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled) {
                        let remaining = order.token_amount.saturating_sub(order.filled_amount);
                        if !remaining.is_zero() {
                            T::TokenProvider::unreserve(order.entity_id, &order.maker, remaining);
                        }
                        order.status = OrderStatus::Expired;
                        Orders::<T>::insert(order_id, &order);
                        Self::remove_from_order_book(entity_id, order_id, OrderSide::Sell);
                        UserOrders::<T>::mutate(&order.maker, |orders| {
                            orders.retain(|&id| id != order_id);
                        });
                        // 审计修复 M3-R5: 添加到已完结订单历史
                        Self::add_to_order_history(&order.maker, order_id);
                        cleaned += 1;
                    }
                }
            }

            // 清理过期买单
            let buy_ids: Vec<u64> = EntityBuyOrders::<T>::get(entity_id).into_iter().collect();
            for order_id in buy_ids {
                if cleaned >= max_count { break; }
                if let Some(mut order) = Orders::<T>::get(order_id) {
                    if now > order.expires_at &&
                       (order.status == OrderStatus::Open || order.status == OrderStatus::PartiallyFilled) {
                        let remaining = order.token_amount.saturating_sub(order.filled_amount);
                        if !remaining.is_zero() {
                            let remaining_u128: u128 = remaining.into();
                            if let Ok(refund) = Self::calculate_total_next(remaining_u128, order.price) {
                                if !refund.is_zero() {
                                    T::Currency::unreserve(&order.maker, refund);
                                }
                            }
                        }
                        order.status = OrderStatus::Expired;
                        Orders::<T>::insert(order_id, &order);
                        Self::remove_from_order_book(entity_id, order_id, OrderSide::Buy);
                        UserOrders::<T>::mutate(&order.maker, |orders| {
                            orders.retain(|&id| id != order_id);
                        });
                        // 审计修复 M3-R5: 添加到已完结订单历史
                        Self::add_to_order_history(&order.maker, order_id);
                        cleaned += 1;
                    }
                }
            }

            // 审计修复 M2: 更新最优价格缓存
            if cleaned > 0 {
                Self::update_best_prices(entity_id);
            }

            Self::deposit_event(Event::ExpiredOrdersCleaned {
                entity_id,
                count: cleaned,
                cleaner: who,
            });

            Ok(())
        }

        /// 修改挂单价格/数量（仅限 Open 状态）
        #[pallet::call_index(30)]
        #[pallet::weight(T::WeightInfo::modify_order())]
        pub fn modify_order(
            origin: OriginFor<T>,
            order_id: u64,
            new_price: BalanceOf<T>,
            new_amount: T::TokenBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
            ensure!(order.maker == who, Error::<T>::NotOrderOwner);
            ensure!(order.status == OrderStatus::Open, Error::<T>::InvalidOrderStatus);
            ensure!(!new_price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!new_amount.is_zero(), Error::<T>::AmountTooSmall);

            // 审计修复 M2-R9: 修改订单需验证市场状态和内幕交易限制
            Self::ensure_market_enabled(order.entity_id)?;
            ensure!(
                T::DisclosureProvider::can_insider_trade(order.entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );

            // 审计修复 M2-R6: 修改后数量不得低于最小订单量
            let config = MarketConfigs::<T>::get(order.entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(new_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }
            // 不允许增加数量（防止不锁定额外资产）
            ensure!(new_amount <= order.token_amount, Error::<T>::ModifyAmountExceedsOriginal);

            // 价格变更时检查偏离限制（防止通过 modify_order 绕过 check_price_deviation）
            if new_price != order.price {
                Self::check_price_deviation(order.entity_id, new_price)?;
            }

            // 退还差额（如果数量减少）
            let diff = order.token_amount.saturating_sub(new_amount);
            if !diff.is_zero() {
                match order.side {
                    OrderSide::Sell => {
                        T::TokenProvider::unreserve(order.entity_id, &who, diff);
                    }
                    OrderSide::Buy => {
                        let diff_u128: u128 = diff.into();
                        let refund = Self::calculate_total_next(diff_u128, order.price)?;
                        if !refund.is_zero() {
                            T::Currency::unreserve(&who, refund);
                        }
                    }
                }
            }

            // 如果价格改变且是买单，需要调整锁定的 NEX
            if new_price != order.price && order.side == OrderSide::Buy {
                let old_total = Self::calculate_total_next(new_amount.into(), order.price)?;
                let new_total = Self::calculate_total_next(new_amount.into(), new_price)?;
                if new_total > old_total {
                    let extra = new_total.saturating_sub(old_total);
                    T::Currency::reserve(&who, extra).map_err(|_| Error::<T>::InsufficientBalance)?;
                } else {
                    let refund = old_total.saturating_sub(new_total);
                    T::Currency::unreserve(&who, refund);
                }
            }

            order.price = new_price;
            order.token_amount = new_amount;
            Orders::<T>::insert(order_id, &order);

            Self::update_best_prices(order.entity_id);

            Self::deposit_event(Event::OrderModified {
                order_id,
                new_price,
                new_amount,
            });

            Ok(())
        }

        /// Root 切换全局市场暂停
        #[pallet::call_index(32)]
        #[pallet::weight(T::WeightInfo::global_market_pause())]
        pub fn global_market_pause(
            origin: OriginFor<T>,
            paused: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            GlobalMarketPaused::<T>::put(paused);
            Self::deposit_event(Event::GlobalMarketPauseToggled { paused });
            Ok(())
        }

        /// P4: 设置实体市场 KYC 要求
        #[pallet::call_index(33)]
        #[pallet::weight(T::WeightInfo::set_kyc_requirement())]
        pub fn set_kyc_requirement(
            origin: OriginFor<T>,
            entity_id: u64,
            min_kyc_level: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            MarketKycRequirement::<T>::insert(entity_id, min_kyc_level);
            Self::deposit_event(Event::KycRequirementSet { entity_id, min_kyc_level });
            Ok(())
        }

        /// P6: 关闭市场（永久关闭，强制取消所有订单并退还资产）
        #[pallet::call_index(34)]
        #[pallet::weight(T::WeightInfo::close_market())]
        pub fn close_market(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotEntityOwner);
            // 审计修复 L1-R5: 与其他 owner extrinsics 一致
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(MarketStatusStorage::<T>::get(entity_id) != MarketStatus::Closed, Error::<T>::MarketAlreadyClosed);

            let cancelled = Self::do_cancel_all_entity_orders(entity_id);
            MarketStatusStorage::<T>::insert(entity_id, MarketStatus::Closed);

            Self::deposit_event(Event::MarketClosed { entity_id, orders_cancelled: cancelled });
            Ok(())
        }

        /// P7: 取消用户在指定实体的所有订单
        #[pallet::call_index(35)]
        #[pallet::weight(T::WeightInfo::cancel_all_entity_orders())]
        pub fn cancel_all_entity_orders(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);

            let order_ids: Vec<u64> = UserOrders::<T>::get(&who)
                .iter()
                .copied()
                .filter(|&oid| {
                    Orders::<T>::get(oid)
                        .map(|o| o.entity_id == entity_id)
                        .unwrap_or(false)
                })
                .collect();

            let mut cancelled = 0u32;
            for order_id in order_ids.iter() {
                if let Some(mut order) = Orders::<T>::get(order_id) {
                    if order.status != OrderStatus::Open && order.status != OrderStatus::PartiallyFilled {
                        continue;
                    }
                    let unfilled = order.token_amount.saturating_sub(order.filled_amount);
                    match order.side {
                        OrderSide::Sell => {
                            T::TokenProvider::unreserve(entity_id, &who, unfilled);
                        }
                        OrderSide::Buy => {
                            if let Ok(refund) = Self::calculate_total_next(unfilled.into(), order.price) {
                                T::Currency::unreserve(&who, refund);
                            }
                        }
                    }
                    order.status = OrderStatus::Cancelled;
                    Orders::<T>::insert(order_id, &order);
                    Self::remove_from_order_book(entity_id, *order_id, order.side);
                    Self::add_to_order_history(&who, *order_id);
                    cancelled += 1;
                }
            }

            UserOrders::<T>::mutate(&who, |orders| {
                orders.retain(|oid| {
                    Orders::<T>::get(oid)
                        .map(|o| o.entity_id != entity_id)
                        .unwrap_or(true)
                });
            });

            Self::update_best_prices(entity_id);

            Self::deposit_event(Event::AllEntityOrdersCancelled {
                entity_id,
                user: who,
                cancelled_count: cancelled,
            });
            Ok(())
        }

        /// P8: 治理配置市场（Root 或治理调用）
        #[pallet::call_index(36)]
        #[pallet::weight(T::WeightInfo::governance_configure_market())]
        pub fn governance_configure_market(
            origin: OriginFor<T>,
            entity_id: u64,
            nex_enabled: bool,
            min_order_amount: u128,
            order_ttl: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(MarketStatusStorage::<T>::get(entity_id) != MarketStatus::Closed, Error::<T>::MarketAlreadyClosed);
            ensure!(order_ttl >= 10, Error::<T>::OrderTtlTooShort);

            let was_enabled = MarketConfigs::<T>::get(entity_id)
                .map(|c| c.nex_enabled)
                .unwrap_or(false);

            MarketConfigs::<T>::mutate(entity_id, |maybe_config| {
                let config = maybe_config.get_or_insert_with(Default::default);
                config.nex_enabled = nex_enabled;
                config.min_order_amount = min_order_amount;
                config.order_ttl = order_ttl;
            });

            // 审计修复 S1-R11: 与 configure_market H2-R10 一致，禁用时自动取消订单
            if was_enabled && !nex_enabled {
                Self::do_cancel_all_entity_orders(entity_id);
            }

            Self::deposit_event(Event::MarketConfigured { entity_id });
            Ok(())
        }

        /// P10: Root 强制关闭实体市场（取消所有订单）
        #[pallet::call_index(37)]
        #[pallet::weight(T::WeightInfo::force_close_market())]
        pub fn force_close_market(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(MarketStatusStorage::<T>::get(entity_id) != MarketStatus::Closed, Error::<T>::MarketAlreadyClosed);

            let cancelled = Self::do_cancel_all_entity_orders(entity_id);
            MarketStatusStorage::<T>::insert(entity_id, MarketStatus::Closed);

            Self::deposit_event(Event::MarketForceClosed { entity_id, orders_cancelled: cancelled });
            Ok(())
        }

        /// P15: IOC 订单（立即成交或取消）
        #[pallet::call_index(38)]
        #[pallet::weight(T::WeightInfo::place_ioc_order())]
        pub fn place_ioc_order(
            origin: OriginFor<T>,
            entity_id: u64,
            side: OrderSide,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_market_enabled(entity_id)?;
            Self::ensure_kyc_requirement(entity_id, &who)?;
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }
            Self::check_price_deviation(entity_id, price)?;

            // 预锁定资产
            match side {
                OrderSide::Sell => {
                    ensure!(
                        T::TokenProvider::token_balance(entity_id, &who) >= token_amount,
                        Error::<T>::InsufficientTokenBalance
                    );
                    T::TokenProvider::reserve(entity_id, &who, token_amount)?;
                }
                OrderSide::Buy => {
                    let total = Self::calculate_total_next(token_amount.into(), price)?;
                    T::Currency::reserve(&who, total).map_err(|_| Error::<T>::InsufficientBalance)?;
                }
            }

            // 尝试立即撮合
            let (crossed, nex_spent) = Self::do_cross_match(
                &who, entity_id, side, price, token_amount,
            )?;
            let remaining = token_amount.saturating_sub(crossed);

            // IOC: 剩余部分直接取消，退还锁定资产
            // 审计修复 H2-R6: 买单需同时退还未成交部分和价格改善多余的 NEX
            match side {
                OrderSide::Sell => {
                    if !remaining.is_zero() {
                        T::TokenProvider::unreserve(entity_id, &who, remaining);
                    }
                }
                OrderSide::Buy => {
                    let total_reserved = Self::calculate_total_next(token_amount.into(), price)?;
                    let excess = total_reserved.saturating_sub(nex_spent);
                    if !excess.is_zero() {
                        T::Currency::unreserve(&who, excess);
                    }
                }
            }

            Self::update_best_prices(entity_id);

            Self::deposit_event(Event::MarketOrderExecuted {
                entity_id,
                trader: who,
                side,
                filled_amount: crossed,
                total_next: nex_spent,
            });

            Ok(())
        }

        /// P15: FOK 订单（全部成交或全部取消）
        #[pallet::call_index(39)]
        #[pallet::weight(T::WeightInfo::place_fok_order())]
        pub fn place_fok_order(
            origin: OriginFor<T>,
            entity_id: u64,
            side: OrderSide,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_market_enabled(entity_id)?;
            Self::ensure_kyc_requirement(entity_id, &who)?;
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }
            Self::check_price_deviation(entity_id, price)?;

            // 审计修复 H1-R6: 排除自己的订单（do_cross_match 会跳过自撮合）
            let available = Self::check_fillable_amount(entity_id, side, price, &who);
            ensure!(available >= token_amount, Error::<T>::FokNotFullyFillable);

            // 预锁定资产
            match side {
                OrderSide::Sell => {
                    ensure!(
                        T::TokenProvider::token_balance(entity_id, &who) >= token_amount,
                        Error::<T>::InsufficientTokenBalance
                    );
                    T::TokenProvider::reserve(entity_id, &who, token_amount)?;
                }
                OrderSide::Buy => {
                    let total = Self::calculate_total_next(token_amount.into(), price)?;
                    T::Currency::reserve(&who, total).map_err(|_| Error::<T>::InsufficientBalance)?;
                }
            }

            // 执行撮合
            let (crossed, nex) = Self::do_cross_match(
                &who, entity_id, side, price, token_amount,
            )?;

            // 审计修复 H1-R6: 退还因价格改善或未完全成交而多余的锁定资产
            match side {
                OrderSide::Sell => {
                    let unfilled = token_amount.saturating_sub(crossed);
                    if !unfilled.is_zero() {
                        T::TokenProvider::unreserve(entity_id, &who, unfilled);
                    }
                }
                OrderSide::Buy => {
                    let total_reserved = Self::calculate_total_next(token_amount.into(), price)?;
                    let excess = total_reserved.saturating_sub(nex);
                    if !excess.is_zero() {
                        T::Currency::unreserve(&who, excess);
                    }
                }
            }

            Self::update_best_prices(entity_id);

            Self::deposit_event(Event::MarketOrderExecuted {
                entity_id,
                trader: who,
                side,
                filled_amount: crossed,
                total_next: nex,
            });

            Ok(())
        }

        /// P15: Post-Only 订单（仅挂单，不立即撮合）
        #[pallet::call_index(40)]
        #[pallet::weight(T::WeightInfo::place_post_only_order())]
        pub fn place_post_only_order(
            origin: OriginFor<T>,
            entity_id: u64,
            side: OrderSide,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_market_enabled(entity_id)?;
            Self::ensure_kyc_requirement(entity_id, &who)?;
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );
            ensure!(!price.is_zero(), Error::<T>::ZeroPrice);
            ensure!(!token_amount.is_zero(), Error::<T>::AmountTooSmall);
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            if config.min_order_amount > 0 {
                ensure!(token_amount.into() >= config.min_order_amount, Error::<T>::OrderAmountBelowMinimum);
            }
            Self::check_price_deviation(entity_id, price)?;

            // Post-Only: 检查价格不会立即撮合
            // 审计修复 R2-R10: 使用动态计算替代缓存，避免过期订单导致误判
            let would_match = match side {
                OrderSide::Buy => {
                    Self::calculate_best_ask(entity_id)
                        .map(|ask| price >= ask)
                        .unwrap_or(false)
                }
                OrderSide::Sell => {
                    Self::calculate_best_bid(entity_id)
                        .map(|bid| price <= bid)
                        .unwrap_or(false)
                }
            };
            ensure!(!would_match, Error::<T>::PostOnlyWouldMatch);

            // 锁定资产并创建订单
            match side {
                OrderSide::Sell => {
                    ensure!(
                        T::TokenProvider::token_balance(entity_id, &who) >= token_amount,
                        Error::<T>::InsufficientTokenBalance
                    );
                    T::TokenProvider::reserve(entity_id, &who, token_amount)?;
                }
                OrderSide::Buy => {
                    let total = Self::calculate_total_next(token_amount.into(), price)?;
                    T::Currency::reserve(&who, total).map_err(|_| Error::<T>::InsufficientBalance)?;
                }
            }

            let order_id = Self::do_create_order(
                entity_id,
                who.clone(),
                side,
                OrderType::PostOnly,
                token_amount,
                price,
            )?;

            Self::update_best_prices(entity_id);

            Self::deposit_event(Event::OrderCreated {
                order_id,
                entity_id,
                maker: who,
                side,
                token_amount,
                price,
            });

            Ok(())
        }

        /// 审计修复 H3-R10: 治理级价格保护配置（Root 调用，绕过 owner 检查）
        #[pallet::call_index(41)]
        #[pallet::weight(T::WeightInfo::governance_configure_price_protection())]
        pub fn governance_configure_price_protection(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
            max_price_deviation: u16,
            max_slippage: u16,
            circuit_breaker_threshold: u16,
            min_trades_for_twap: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);

            ensure!(max_price_deviation <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(max_slippage <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(circuit_breaker_threshold <= 10000, Error::<T>::InvalidBasisPoints);

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

        /// 审计修复 H4-R10: Root 强制解除熔断（无需等待到期）
        #[pallet::call_index(42)]
        #[pallet::weight(T::WeightInfo::force_lift_circuit_breaker())]
        pub fn force_lift_circuit_breaker(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);

            let config = PriceProtection::<T>::get(entity_id).unwrap_or_default();
            ensure!(config.circuit_breaker_active, Error::<T>::CircuitBreakerNotActive);

            PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.circuit_breaker_active = false;
                    config.circuit_breaker_until = 0;
                }
            });

            Self::deposit_event(Event::CircuitBreakerLifted { entity_id });

            Ok(())
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// 验证市场是否启用
        /// H4 审计修复: 添加 is_entity_active 检查，Banned/Closed 实体不允许新订单
        fn ensure_market_enabled(entity_id: u64) -> DispatchResult {
            // 全局暂停检查
            ensure!(!GlobalMarketPaused::<T>::get(), Error::<T>::GlobalMarketPausedError);
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::TokenProvider::is_token_enabled(entity_id),
                Error::<T>::TokenNotEnabled
            );

            // P6: 市场已关闭检查
            ensure!(MarketStatusStorage::<T>::get(entity_id) != MarketStatus::Closed, Error::<T>::MarketAlreadyClosed);

            // M6: 检查市场配置（必须显式配置并启用，与 Default nex_enabled=false 一致）
            let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
            ensure!(config.nex_enabled, Error::<T>::MarketNotEnabled);
            ensure!(!config.paused, Error::<T>::MarketPaused);

            Ok(())
        }

        /// 审计修复 H2-R7: 独立的熔断器检查（市价单/吃单不经过 check_price_deviation）
        /// 审计修复 S2-R11: 到期后自动清理存储状态，避免 circuit_breaker_active 残留
        fn ensure_circuit_breaker_inactive(entity_id: u64) -> DispatchResult {
            if let Some(config) = PriceProtection::<T>::get(entity_id) {
                if config.enabled && config.circuit_breaker_active {
                    let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                    if current_block < config.circuit_breaker_until {
                        return Err(Error::<T>::MarketCircuitBreakerActive.into());
                    }
                    // 到期后自动清理存储
                    PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                        if let Some(c) = maybe_config {
                            c.circuit_breaker_active = false;
                            c.circuit_breaker_until = 0;
                        }
                    });
                    Self::deposit_event(Event::CircuitBreakerLifted { entity_id });
                }
            }
            Ok(())
        }

        /// P4: 检查用户 KYC 级别是否满足市场要求
        fn ensure_kyc_requirement(entity_id: u64, who: &T::AccountId) -> DispatchResult {
            let min_level = MarketKycRequirement::<T>::get(entity_id);
            if min_level > 0 {
                let user_level = T::KycProvider::kyc_level(entity_id, who);
                ensure!(user_level >= min_level, Error::<T>::InsufficientKycLevel);
            }
            Ok(())
        }

        /// P1: 记录成交历史
        #[allow(clippy::too_many_arguments)]
        fn record_trade(
            order_id: u64,
            entity_id: u64,
            maker: T::AccountId,
            taker: T::AccountId,
            side: OrderSide,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
            nex_amount: BalanceOf<T>,
        ) {
            let trade_id = NextTradeId::<T>::get();
            NextTradeId::<T>::put(trade_id.saturating_add(1));

            let block_number = <frame_system::Pallet<T>>::block_number();

            let record = TradeRecord::<T> {
                trade_id,
                order_id,
                entity_id,
                maker: maker.clone(),
                taker: taker.clone(),
                side,
                token_amount,
                price,
                nex_amount,
                block_number,
            };
            TradeRecords::<T>::insert(trade_id, record);

            // 更新用户交易历史索引（环形覆盖）
            UserTradeHistory::<T>::mutate(&maker, |history| {
                if history.len() as u32 >= T::MaxTradeHistoryPerUser::get() {
                    history.remove(0);
                }
                let _ = history.try_push(trade_id);
            });
            UserTradeHistory::<T>::mutate(&taker, |history| {
                if history.len() as u32 >= T::MaxTradeHistoryPerUser::get() {
                    history.remove(0);
                }
                let _ = history.try_push(trade_id);
            });

            // 更新实体交易历史索引
            EntityTradeHistory::<T>::mutate(entity_id, |history| {
                if history.len() >= 500 {
                    history.remove(0);
                }
                let _ = history.try_push(trade_id);
            });

            // P3: 更新日统计
            Self::update_daily_stats(entity_id, price, nex_amount);

            Self::deposit_event(Event::TradeExecuted {
                trade_id,
                order_id,
                entity_id,
                maker,
                taker,
                side,
                token_amount,
                price,
                nex_amount,
            });
        }

        /// P2: 添加到用户已完结订单历史
        fn add_to_order_history(who: &T::AccountId, order_id: u64) {
            UserOrderHistory::<T>::mutate(who, |history| {
                if history.len() as u32 >= T::MaxOrderHistoryPerUser::get() {
                    history.remove(0);
                }
                let _ = history.try_push(order_id);
            });
        }

        /// P1/P11: 更新交易统计（实体 + 全局）
        fn update_trade_stats(entity_id: u64, nex_amount: BalanceOf<T>) {
            MarketStatsStorage::<T>::mutate(entity_id, |stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_nex = stats.total_volume_nex.saturating_add(nex_amount.into());
            });

            // P11: 更新全局统计
            GlobalStats::<T>::mutate(|stats| {
                stats.total_trades = stats.total_trades.saturating_add(1);
                stats.total_volume_nex = stats.total_volume_nex.saturating_add(nex_amount.into());
            });
        }

        /// P3: 更新实体日统计
        fn update_daily_stats(entity_id: u64, price: BalanceOf<T>, nex_amount: BalanceOf<T>) {
            let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
            let blocks_per_day = T::BlocksPerDay::get();

            let zero_balance: BalanceOf<T> = Zero::zero();
            EntityDailyStats::<T>::mutate(entity_id, |stats| {
                // 检查是否需要重置（新的一天）
                if current_block.saturating_sub(stats.period_start) >= blocks_per_day {
                    stats.open_price = price;
                    stats.high_price = price;
                    stats.low_price = price;
                    stats.close_price = price;
                    stats.volume_nex = 0;
                    stats.trade_count = 0;
                    stats.period_start = current_block;
                } else {
                    if stats.open_price == zero_balance {
                        stats.open_price = price;
                    }
                    if price > stats.high_price {
                        stats.high_price = price;
                    }
                    if stats.low_price == zero_balance || price < stats.low_price {
                        stats.low_price = price;
                    }
                    stats.close_price = price;
                }
                stats.volume_nex = stats.volume_nex.saturating_add(nex_amount.into());
                stats.trade_count = stats.trade_count.saturating_add(1);
            });
        }

        /// P6/P10: 取消实体所有活跃订单，退还锁定资产
        fn do_cancel_all_entity_orders(entity_id: u64) -> u32 {
            let mut cancelled = 0u32;

            // 取消卖单
            let sell_ids: Vec<u64> = EntitySellOrders::<T>::get(entity_id).into_inner();
            for order_id in sell_ids.iter() {
                if let Some(mut order) = Orders::<T>::get(order_id) {
                    if order.status != OrderStatus::Open && order.status != OrderStatus::PartiallyFilled {
                        continue;
                    }
                    let unfilled = order.token_amount.saturating_sub(order.filled_amount);
                    T::TokenProvider::unreserve(entity_id, &order.maker, unfilled);
                    order.status = OrderStatus::Cancelled;
                    Orders::<T>::insert(order_id, &order);
                    UserOrders::<T>::mutate(&order.maker, |orders| {
                        orders.retain(|&id| id != *order_id);
                    });
                    Self::add_to_order_history(&order.maker, *order_id);
                    cancelled += 1;
                }
            }
            EntitySellOrders::<T>::mutate(entity_id, |orders| orders.clear());

            // 取消买单
            let buy_ids: Vec<u64> = EntityBuyOrders::<T>::get(entity_id).into_inner();
            for order_id in buy_ids.iter() {
                if let Some(mut order) = Orders::<T>::get(order_id) {
                    if order.status != OrderStatus::Open && order.status != OrderStatus::PartiallyFilled {
                        continue;
                    }
                    let unfilled = order.token_amount.saturating_sub(order.filled_amount);
                    if let Ok(refund) = Self::calculate_total_next(unfilled.into(), order.price) {
                        T::Currency::unreserve(&order.maker, refund);
                    }
                    order.status = OrderStatus::Cancelled;
                    Orders::<T>::insert(order_id, &order);
                    UserOrders::<T>::mutate(&order.maker, |orders| {
                        orders.retain(|&id| id != *order_id);
                    });
                    Self::add_to_order_history(&order.maker, *order_id);
                    cancelled += 1;
                }
            }
            EntityBuyOrders::<T>::mutate(entity_id, |orders| orders.clear());

            Self::update_best_prices(entity_id);
            cancelled
        }

        /// P15: 检查在给定价格限制下可填充的最大数量（用于 FOK 验证）
        /// 审计修复 H1-R6: 排除 taker 自己的订单（do_cross_match 会跳过自撮合）
        fn check_fillable_amount(
            entity_id: u64,
            side: OrderSide,
            price: BalanceOf<T>,
            exclude_maker: &T::AccountId,
        ) -> T::TokenBalance {
            let counter_orders = match side {
                OrderSide::Sell => Self::get_sorted_buy_orders(entity_id),
                OrderSide::Buy => Self::get_sorted_sell_orders(entity_id),
            };
            let mut total: T::TokenBalance = Zero::zero();
            for order in counter_orders {
                let price_ok = match side {
                    OrderSide::Sell => order.price >= price,
                    OrderSide::Buy => order.price <= price,
                };
                if !price_ok { break; }
                if order.maker == *exclude_maker { continue; }
                let available = order.token_amount.saturating_sub(order.filled_amount);
                total = total.saturating_add(available);
            }
            total
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
        /// 返回 (已撮合数量, NEX 总额)
        fn do_cross_match(
            taker: &T::AccountId,
            entity_id: u64,
            taker_side: OrderSide,
            limit_price: BalanceOf<T>,
            mut remaining: T::TokenBalance,
        ) -> Result<(T::TokenBalance, BalanceOf<T>), DispatchError> {
            let mut total_filled: T::TokenBalance = Zero::zero();
            let mut total_nex: BalanceOf<T> = Zero::zero();

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

                // NEX 转账: 买方(reserved) → 卖方（无手续费，全额转账）
                let (nex_payer, nex_receiver) = match taker_side {
                    OrderSide::Sell => (&counter_order.maker, taker),
                    OrderSide::Buy => (taker, &counter_order.maker),
                };
                T::Currency::repatriate_reserved(
                    nex_payer, nex_receiver, nex_amount,
                    frame_support::traits::BalanceStatus::Free,
                )?;

                // Token 转账: 卖方(reserved) → 买方
                let (token_from, token_to) = match taker_side {
                    OrderSide::Sell => (taker, &counter_order.maker),
                    OrderSide::Buy => (&counter_order.maker, taker),
                };
                T::TokenProvider::repatriate_reserved(
                    entity_id, token_from, token_to, fill_amount,
                )?;

                Self::update_order_fill(&counter_order, entity_id, fill_amount);

                Self::update_trade_stats(entity_id, nex_amount);

                // P1: 记录成交
                Self::record_trade(
                    counter_order.order_id, entity_id,
                    counter_order.maker.clone(), taker.clone(),
                    taker_side, fill_amount, counter_order.price, nex_amount,
                );

                Self::deposit_event(Event::OrderFilled {
                    order_id: counter_order.order_id,
                    entity_id,
                    maker: counter_order.maker.clone(),
                    taker: taker.clone(),
                    filled_amount: fill_amount,
                    total_next: nex_amount,
                });
                Self::on_trade_completed(entity_id, counter_order.price);

                total_filled = total_filled.saturating_add(fill_amount);
                total_nex = total_nex.saturating_add(nex_amount);
                remaining = remaining.saturating_sub(fill_amount);
            }

            Ok((total_filled, total_nex))
        }

        /// 创建订单
        fn do_create_order(
            entity_id: u64,
            maker: T::AccountId,
            side: OrderSide,
            order_type: OrderType,
            token_amount: T::TokenBalance,
            price: BalanceOf<T>,
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
                token_amount,
                filled_amount: Zero::zero(),
                price,
                status: OrderStatus::Open,
                created_at: now,
                expires_at,
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
                // P2: 添加到已完结订单历史
                Self::add_to_order_history(&order.maker, order.order_id);
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
        ) -> Result<(T::TokenBalance, BalanceOf<T>), DispatchError> {
            let mut total_filled: T::TokenBalance = Zero::zero();
            let mut total_next: BalanceOf<T> = Zero::zero();

            for order in sell_orders.iter_mut() {
                if remaining.is_zero() {
                    break;
                }

                // 审计修复 H1-R7: 跳过自己的订单（防止洗盘 + TWAP 操纵）
                if order.maker == *buyer { continue; }

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

                // NEX: buyer → maker（无手续费，全额转账）
                T::Currency::transfer(
                    buyer,
                    &order.maker,
                    cost,
                    ExistenceRequirement::KeepAlive,
                )?;

                // Token: maker → buyer（从 maker 的 reserved 转出）
                T::TokenProvider::repatriate_reserved(
                    entity_id,
                    &order.maker,
                    buyer,
                    fill_amount,
                )?;

                // 更新订单
                Self::update_order_fill(order, entity_id, fill_amount);

                // P1: 记录成交 + P11: 更新统计
                Self::update_trade_stats(entity_id, cost);
                Self::record_trade(
                    order.order_id, entity_id, order.maker.clone(), buyer.clone(),
                    OrderSide::Buy, fill_amount, order.price, cost,
                );

                // P12: OrderFilled 包含 maker
                Self::deposit_event(Event::OrderFilled {
                    order_id: order.order_id,
                    entity_id,
                    maker: order.maker.clone(),
                    taker: buyer.clone(),
                    filled_amount: fill_amount,
                    total_next: cost,
                });
                Self::on_trade_completed(entity_id, order.price);

                // 累计
                total_filled = total_filled.saturating_add(fill_amount);
                total_next = total_next.saturating_add(cost);
                remaining = remaining.saturating_sub(fill_amount);
            }

            // 更新最优价格
            if !total_filled.is_zero() {
                Self::update_best_prices(entity_id);
            }

            Ok((total_filled, total_next))
        }

        /// 执行市价卖出
        fn do_market_sell(
            seller: &T::AccountId,
            entity_id: u64,
            mut remaining: T::TokenBalance,
            min_receive: BalanceOf<T>,
            buy_orders: &mut Vec<TradeOrder<T>>,
        ) -> Result<(T::TokenBalance, BalanceOf<T>), DispatchError> {
            let mut total_filled: T::TokenBalance = Zero::zero();
            let mut total_receive: BalanceOf<T> = Zero::zero();

            for order in buy_orders.iter_mut() {
                if remaining.is_zero() {
                    break;
                }

                // 审计修复 H1-R7: 跳过自己的订单（防止洗盘 + TWAP 操纵）
                if order.maker == *seller { continue; }

                // 计算可成交数量
                let available = order.token_amount.saturating_sub(order.filled_amount);
                let fill_amount = remaining.min(available);

                // 计算收入（无手续费，全额转账）
                let fill_u128: u128 = fill_amount.into();
                let gross = Self::calculate_total_next(fill_u128, order.price)?;

                // P1 修复: 滑点检查移到转账前（与 market_buy 一致）
                let projected_receive = total_receive.saturating_add(gross);
                let projected_remaining = remaining.saturating_sub(fill_amount);
                if projected_remaining.is_zero() && projected_receive < min_receive {
                    return Err(Error::<T>::SlippageExceeded.into());
                }

                // NEX: maker(reserved) → seller（全额转账）
                T::Currency::repatriate_reserved(
                    &order.maker, seller, gross,
                    frame_support::traits::BalanceStatus::Free,
                )?;

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

                // P1: 记录成交 + P11: 更新统计
                Self::update_trade_stats(entity_id, gross);
                Self::record_trade(
                    order.order_id, entity_id, order.maker.clone(), seller.clone(),
                    OrderSide::Sell, fill_amount, order.price, gross,
                );

                // P12: OrderFilled 包含 maker
                Self::deposit_event(Event::OrderFilled {
                    order_id: order.order_id,
                    entity_id,
                    maker: order.maker.clone(),
                    taker: seller.clone(),
                    filled_amount: fill_amount,
                    total_next: gross,
                });
                Self::on_trade_completed(entity_id, order.price);

                // 累计
                total_filled = total_filled.saturating_add(fill_amount);
                total_receive = total_receive.saturating_add(gross);
                remaining = remaining.saturating_sub(fill_amount);
            }

            // 更新最优价格
            if !total_filled.is_zero() {
                Self::update_best_prices(entity_id);
            }

            Ok((total_filled, total_receive))
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
                let _blocks_per_week = T::BlocksPerWeek::get();

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
            if config.circuit_breaker_active {
                if current_block < config.circuit_breaker_until {
                    return Err(Error::<T>::MarketCircuitBreakerActive);
                }
                // 审计修复 S2-R11: 到期后自动清理存储
                PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                    if let Some(c) = maybe_config {
                        c.circuit_breaker_active = false;
                        c.circuit_breaker_until = 0;
                    }
                });
                Self::deposit_event(Event::CircuitBreakerLifted { entity_id });
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
    /// 审计修复 L1-R6: 过滤过期订单（与 calculate_best_ask/bid 一致）
    pub fn get_sell_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
        let now = <frame_system::Pallet<T>>::block_number();
        EntitySellOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                    && now <= o.expires_at
            })
            .collect()
    }

    /// 获取实体买单列表
    /// 审计修复 L1-R6: 过滤过期订单（与 calculate_best_bid 一致）
    pub fn get_buy_orders(entity_id: u64) -> Vec<TradeOrder<T>> {
        let now = <frame_system::Pallet<T>>::block_number();
        EntityBuyOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                    && now <= o.expires_at
            })
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

        let now = <frame_system::Pallet<T>>::block_number();

        // 审计修复 M5-R5: 过滤过期订单（与 calculate_best_ask/bid 一致）
        // 计算卖单总量
        let total_ask_amount: T::TokenBalance = EntitySellOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                    && now <= o.expires_at
            })
            .fold(Zero::zero(), |acc: T::TokenBalance, o| {
                acc.saturating_add(o.token_amount.saturating_sub(o.filled_amount))
            });

        // 计算买单总量
        let total_bid_amount: T::TokenBalance = EntityBuyOrders::<T>::get(entity_id)
            .iter()
            .filter_map(|&id| Orders::<T>::get(id))
            .filter(|o| {
                (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                    && now <= o.expires_at
            })
            .fold(Zero::zero(), |acc: T::TokenBalance, o| {
                acc.saturating_add(o.token_amount.saturating_sub(o.filled_amount))
            });

        MarketSummary {
            best_ask,
            best_bid,
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

    // ==================== P13: 分页查询接口 ====================

    /// P13: 分页获取用户交易历史
    pub fn get_user_trade_history(
        user: &T::AccountId,
        page: u32,
        page_size: u32,
    ) -> Vec<TradeRecord<T>> {
        let history = UserTradeHistory::<T>::get(user);
        let start = (page * page_size) as usize;
        history.iter()
            .rev()
            .skip(start)
            .take(page_size as usize)
            .filter_map(|&id| TradeRecords::<T>::get(id))
            .collect()
    }

    /// P13: 分页获取实体交易历史
    pub fn get_entity_trade_history(
        entity_id: u64,
        page: u32,
        page_size: u32,
    ) -> Vec<TradeRecord<T>> {
        let history = EntityTradeHistory::<T>::get(entity_id);
        let start = (page * page_size) as usize;
        history.iter()
            .rev()
            .skip(start)
            .take(page_size as usize)
            .filter_map(|&id| TradeRecords::<T>::get(id))
            .collect()
    }

    /// P13: 分页获取用户已完结订单历史
    pub fn get_user_order_history(
        user: &T::AccountId,
        page: u32,
        page_size: u32,
    ) -> Vec<TradeOrder<T>> {
        let history = UserOrderHistory::<T>::get(user);
        let start = (page * page_size) as usize;
        history.iter()
            .rev()
            .skip(start)
            .take(page_size as usize)
            .filter_map(|&id| Orders::<T>::get(id))
            .collect()
    }

    /// P3: 获取实体日统计
    pub fn get_daily_stats(entity_id: u64) -> DailyStats<BalanceOf<T>> {
        EntityDailyStats::<T>::get(entity_id)
    }

    /// P11: 获取全局统计
    pub fn get_global_stats() -> MarketStats {
        GlobalStats::<T>::get()
    }

    /// P6: 获取市场状态
    pub fn get_market_status(entity_id: u64) -> MarketStatus {
        MarketStatusStorage::<T>::get(entity_id)
    }

    /// P4: 获取市场 KYC 要求
    pub fn get_kyc_requirement(entity_id: u64) -> u8 {
        MarketKycRequirement::<T>::get(entity_id)
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
        use pallet_entity_common::PricingProvider as _;
        // 间接换算: Token → NEX → USDT
        // token_nex_price 精度 10^12, nex_usdt_price 精度 10^6
        // result (精度 10^6) = token_nex_price × nex_usdt_price / 10^12
        let token_nex_price: u128 = Self::get_token_price(entity_id)?.into();
        let nex_usdt_price: u128 = T::PricingProvider::get_nex_usdt_price() as u128;
        if nex_usdt_price == 0 { return None; }
        let usdt = token_nex_price
            .saturating_mul(nex_usdt_price)
            .checked_div(1_000_000_000_000u128)?;
        Some(usdt as u64)
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
