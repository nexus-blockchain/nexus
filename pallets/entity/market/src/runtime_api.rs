//! Runtime API：实体代币交易市场查询接口
//!
//! 将原先需要付费 extrinsic 的查询操作迁移为免费 Runtime API，
//! 前端可通过 RPC 直接调用，无需支付 gas 费。
//!
//! 提供接口：
//! - 订单簿查询（卖单、买单、深度、快照、最优价格）
//! - 用户订单 / 交易历史（分页）
//! - 市场摘要、日统计、全局统计
//! - TWAP 价格预言机查询
//! - 市场配置 / 状态 / KYC 要求

use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

extern crate alloc;
use alloc::vec::Vec;

// ==================== DTO 数据结构 ====================

/// 订单信息（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct OrderInfo<AccountId, Balance, TokenBalance> {
    pub order_id: u64,
    pub entity_id: u64,
    pub maker: AccountId,
    pub side: u8,
    pub order_type: u8,
    pub token_amount: TokenBalance,
    pub filled_amount: TokenBalance,
    pub price: Balance,
    pub status: u8,
    pub created_at: u64,
    pub expires_at: u64,
}

/// 成交记录（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct TradeInfo<AccountId, Balance, TokenBalance> {
    pub trade_id: u64,
    pub order_id: u64,
    pub entity_id: u64,
    pub maker: AccountId,
    pub taker: AccountId,
    pub side: u8,
    pub token_amount: TokenBalance,
    pub price: Balance,
    pub nex_amount: Balance,
    pub block_number: u64,
}

/// 价格档位（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PriceLevelInfo<Balance, TokenBalance> {
    pub price: Balance,
    pub total_amount: TokenBalance,
    pub order_count: u32,
}

/// 订单簿深度（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct OrderBookDepthInfo<Balance, TokenBalance> {
    pub entity_id: u64,
    pub asks: Vec<PriceLevelInfo<Balance, TokenBalance>>,
    pub bids: Vec<PriceLevelInfo<Balance, TokenBalance>>,
    pub best_ask: Option<Balance>,
    pub best_bid: Option<Balance>,
    pub spread: Option<Balance>,
    pub block_number: u32,
}

/// 市场摘要（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MarketSummaryInfo<Balance, TokenBalance> {
    pub best_ask: Option<Balance>,
    pub best_bid: Option<Balance>,
    pub last_price: Option<Balance>,
    pub total_ask_amount: TokenBalance,
    pub total_bid_amount: TokenBalance,
}

/// 日统计（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct DailyStatsInfo<Balance> {
    pub open_price: Balance,
    pub high_price: Balance,
    pub low_price: Balance,
    pub close_price: Balance,
    pub volume_nex: u128,
    pub trade_count: u32,
    pub period_start: u32,
}

/// 全局/实体统计（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MarketStatsInfo {
    pub total_orders: u64,
    pub total_trades: u64,
    pub total_volume_nex: u128,
}

/// 市场配置（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MarketConfigInfo {
    pub nex_enabled: bool,
    pub min_order_amount: u128,
    pub order_ttl: u32,
    pub paused: bool,
}

/// TWAP 信息（Runtime API 返回值）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct TwapInfo<Balance> {
    pub twap_1h: Option<Balance>,
    pub twap_24h: Option<Balance>,
    pub twap_7d: Option<Balance>,
    pub last_price: Option<Balance>,
    pub trade_count: u64,
}

// ==================== Runtime API 声明 ====================

sp_api::decl_runtime_apis! {
    /// Entity Market Runtime API
    ///
    /// 实体代币交易市场查询接口，供前端通过 RPC 免费调用。
    pub trait EntityMarketApi<AccountId, Balance, TokenBalance>
    where
        AccountId: Codec,
        Balance: Codec,
        TokenBalance: Codec,
    {
        /// 查询实体卖单列表（过滤过期订单）
        fn get_sell_orders(entity_id: u64) -> Vec<OrderInfo<AccountId, Balance, TokenBalance>>;

        /// 查询实体买单列表（过滤过期订单）
        fn get_buy_orders(entity_id: u64) -> Vec<OrderInfo<AccountId, Balance, TokenBalance>>;

        /// 查询用户所有活跃订单
        fn get_user_orders(user: AccountId) -> Vec<OrderInfo<AccountId, Balance, TokenBalance>>;

        /// 查询单个订单详情
        fn get_order(order_id: u64) -> Option<OrderInfo<AccountId, Balance, TokenBalance>>;

        /// 查询订单簿深度（每边 max_depth 档）
        fn get_order_book_depth(entity_id: u64, max_depth: u32) -> OrderBookDepthInfo<Balance, TokenBalance>;

        /// 查询最优买卖价
        fn get_best_prices(entity_id: u64) -> (Option<Balance>, Option<Balance>);

        /// 查询买卖价差
        fn get_spread(entity_id: u64) -> Option<Balance>;

        /// 查询市场摘要
        fn get_market_summary(entity_id: u64) -> MarketSummaryInfo<Balance, TokenBalance>;

        /// 查询订单簿快照（简化版，最多 20 档）
        fn get_order_book_snapshot(entity_id: u64) -> (Vec<(Balance, TokenBalance)>, Vec<(Balance, TokenBalance)>);

        /// 分页查询用户交易历史
        fn get_user_trade_history(user: AccountId, page: u32, page_size: u32) -> Vec<TradeInfo<AccountId, Balance, TokenBalance>>;

        /// 分页查询实体交易历史
        fn get_entity_trade_history(entity_id: u64, page: u32, page_size: u32) -> Vec<TradeInfo<AccountId, Balance, TokenBalance>>;

        /// 分页查询用户已完结订单历史
        fn get_user_order_history(user: AccountId, page: u32, page_size: u32) -> Vec<OrderInfo<AccountId, Balance, TokenBalance>>;

        /// 查询日统计（OHLCV）
        fn get_daily_stats(entity_id: u64) -> DailyStatsInfo<Balance>;

        /// 查询全局市场统计
        fn get_global_stats() -> MarketStatsInfo;

        /// 查询市场状态（Active / Closed）
        fn get_market_status(entity_id: u64) -> u8;

        /// 查询市场配置
        fn get_market_config(entity_id: u64) -> Option<MarketConfigInfo>;

        /// 查询市场 KYC 要求
        fn get_kyc_requirement(entity_id: u64) -> u8;

        /// 查询 TWAP 价格信息（1h/24h/7d）
        fn get_twap_info(entity_id: u64) -> TwapInfo<Balance>;
    }
}
