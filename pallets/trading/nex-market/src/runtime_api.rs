//! Runtime API 定义：用于前端查询 NEX/USDT 市场数据
//!
//! 提供以下接口：
//! - `get_sell_orders`: 获取活跃卖单列表
//! - `get_buy_orders`: 获取活跃买单列表
//! - `get_user_orders`: 获取用户订单列表
//! - `get_user_trades`: 获取用户交易历史
//! - `get_order_trades`: 获取订单关联交易
//! - `get_active_trades`: 获取用户活跃交易
//! - `get_order_depth`: 获取订单深度图数据
//! - `get_best_prices`: 获取最优买卖价格
//! - `get_market_stats`: 获取市场统计信息
//! - `is_market_paused`: 查询市场是否暂停

use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// 订单摘要（Runtime API 返回用，不含泛型 BlockNumber）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct OrderInfo<AccountId, Balance> {
    pub order_id: u64,
    /// 0 = Sell, 1 = Buy
    pub side: u8,
    pub owner: AccountId,
    pub nex_amount: Balance,
    pub filled_amount: Balance,
    pub usdt_price: u64,
    /// 0=Open, 1=PartiallyFilled, 2=Filled, 3=Cancelled, 4=Expired
    pub status: u8,
    pub created_at: u64,
    pub expires_at: u64,
}

/// 交易摘要（Runtime API 返回用）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct TradeInfo<AccountId, Balance> {
    pub trade_id: u64,
    pub order_id: u64,
    pub seller: AccountId,
    pub buyer: AccountId,
    pub nex_amount: Balance,
    pub usdt_amount: u64,
    /// 0=AwaitingPayment, 1=AwaitingVerification, 2=Completed, 3=Refunded,
    /// 4=Disputed, 5=UnderpaidPending
    pub status: u8,
    pub created_at: u64,
}

/// 市场统计摘要
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct MarketSummary {
    pub best_ask: Option<u64>,
    pub best_bid: Option<u64>,
    pub last_trade_price: Option<u64>,
    pub is_paused: bool,
    pub trading_fee_bps: u16,
    pub pending_trades_count: u32,
}

/// 深度图条目
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct DepthEntry<Balance> {
    pub price: u64,
    pub amount: Balance,
}

sp_api::decl_runtime_apis! {
    /// NEX Market Runtime API
    ///
    /// 提供订单簿查询、交易历史、市场统计等前端集成接口
    pub trait NexMarketApi<AccountId, Balance>
    where
        AccountId: Codec,
        Balance: Codec,
    {
        /// 获取活跃卖单列表（按价格升序）
        fn get_sell_orders() -> Vec<OrderInfo<AccountId, Balance>>;

        /// 获取活跃买单列表（按价格降序）
        fn get_buy_orders() -> Vec<OrderInfo<AccountId, Balance>>;

        /// 获取用户的所有订单
        fn get_user_orders(user: AccountId) -> Vec<OrderInfo<AccountId, Balance>>;

        /// 获取用户交易历史
        fn get_user_trades(user: AccountId) -> Vec<TradeInfo<AccountId, Balance>>;

        /// 获取订单关联的交易列表
        fn get_order_trades(order_id: u64) -> Vec<TradeInfo<AccountId, Balance>>;

        /// 获取用户活跃交易（待付款/待验证/待补付）
        fn get_active_trades(user: AccountId) -> Vec<TradeInfo<AccountId, Balance>>;

        /// 获取订单深度图（asks 升序, bids 降序）
        fn get_order_depth() -> (Vec<DepthEntry<Balance>>, Vec<DepthEntry<Balance>>);

        /// 获取最优买卖价格
        fn get_best_prices() -> (Option<u64>, Option<u64>);

        /// 获取市场统计摘要
        fn get_market_summary() -> MarketSummary;
    }
}
