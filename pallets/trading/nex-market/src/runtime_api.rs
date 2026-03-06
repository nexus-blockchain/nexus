//! Runtime API 定义：用于前端查询 NEX/USDT 市场数据

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
    pub min_fill_amount: Balance,
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
    /// 4=UnderpaidPending
    pub status: u8,
    pub created_at: u64,
    pub timeout_at: u64,
    pub buyer_deposit: Balance,
    /// 0=None, 1=Locked, 2=Released, 3=Forfeited
    pub deposit_status: u8,
    pub underpaid_deadline: Option<u64>,
    /// W5: 交易终态时间（区块号），用于精确争议窗口
    pub completed_at: Option<u64>,
    /// W6: 买家是否已确认/检测到付款
    pub payment_confirmed: bool,
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
    pub trait NexMarketApi<AccountId, Balance>
    where
        AccountId: Codec,
        Balance: Codec,
    {
        /// 获取活跃卖单列表（按价格升序，支持分页）
        fn get_sell_orders(offset: u32, limit: u32) -> Vec<OrderInfo<AccountId, Balance>>;

        /// 获取活跃买单列表（按价格降序，支持分页）
        fn get_buy_orders(offset: u32, limit: u32) -> Vec<OrderInfo<AccountId, Balance>>;

        /// 获取用户的所有订单
        fn get_user_orders(user: AccountId) -> Vec<OrderInfo<AccountId, Balance>>;

        /// 获取用户交易历史（支持分页）
        fn get_user_trades(user: AccountId, offset: u32, limit: u32) -> Vec<TradeInfo<AccountId, Balance>>;

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

        /// 获取单个订单详情
        fn get_order_by_id(order_id: u64) -> Option<OrderInfo<AccountId, Balance>>;

        /// 获取单个交易详情
        fn get_trade_by_id(trade_id: u64) -> Option<TradeInfo<AccountId, Balance>>;
    }
}
