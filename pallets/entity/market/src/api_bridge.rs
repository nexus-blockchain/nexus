//! Runtime API 桥接：内部类型 → DTO 转换方法

use crate::pallet::*;
use crate::runtime_api;
use alloc::vec::Vec;
use sp_runtime::SaturatedConversion;

impl<T: Config> Pallet<T> {
    // ==================== 订单转换 ====================

    fn order_to_info(order: &TradeOrder<T>) -> runtime_api::OrderInfo<T::AccountId, BalanceOf<T>, T::TokenBalance> {
        runtime_api::OrderInfo {
            order_id: order.order_id,
            entity_id: order.entity_id,
            maker: order.maker.clone(),
            side: match order.side {
                OrderSide::Buy => 0,
                OrderSide::Sell => 1,
            },
            order_type: match order.order_type {
                OrderType::Limit => 0,
                OrderType::Market => 1,
                OrderType::ImmediateOrCancel => 2,
                OrderType::FillOrKill => 3,
                OrderType::PostOnly => 4,
            },
            token_amount: order.token_amount,
            filled_amount: order.filled_amount,
            price: order.price,
            status: match order.status {
                OrderStatus::Open => 0,
                OrderStatus::PartiallyFilled => 1,
                OrderStatus::Filled => 2,
                OrderStatus::Cancelled => 3,
                OrderStatus::Expired => 4,
            },
            created_at: order.created_at.saturated_into(),
            expires_at: order.expires_at.saturated_into(),
        }
    }

    fn trade_to_info(trade: &TradeRecord<T>) -> runtime_api::TradeInfo<T::AccountId, BalanceOf<T>, T::TokenBalance> {
        runtime_api::TradeInfo {
            trade_id: trade.trade_id,
            order_id: trade.order_id,
            entity_id: trade.entity_id,
            maker: trade.maker.clone(),
            taker: trade.taker.clone(),
            side: match trade.side {
                OrderSide::Buy => 0,
                OrderSide::Sell => 1,
            },
            token_amount: trade.token_amount,
            price: trade.price,
            nex_amount: trade.nex_amount,
            block_number: trade.block_number.saturated_into(),
        }
    }

    // ==================== API 方法 ====================

    pub fn api_get_sell_orders(entity_id: u64) -> Vec<runtime_api::OrderInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Self::get_sell_orders(entity_id)
            .iter()
            .map(|o| Self::order_to_info(o))
            .collect()
    }

    pub fn api_get_buy_orders(entity_id: u64) -> Vec<runtime_api::OrderInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Self::get_buy_orders(entity_id)
            .iter()
            .map(|o| Self::order_to_info(o))
            .collect()
    }

    pub fn api_get_user_orders(user: &T::AccountId) -> Vec<runtime_api::OrderInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Self::get_user_orders(user)
            .iter()
            .map(|o| Self::order_to_info(o))
            .collect()
    }

    pub fn api_get_order(order_id: u64) -> Option<runtime_api::OrderInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Orders::<T>::get(order_id).as_ref().map(Self::order_to_info)
    }

    pub fn api_get_order_book_depth(entity_id: u64, max_depth: u32) -> runtime_api::OrderBookDepthInfo<BalanceOf<T>, T::TokenBalance> {
        let depth = Self::get_order_book_depth(entity_id, max_depth);
        runtime_api::OrderBookDepthInfo {
            entity_id: depth.entity_id,
            asks: depth.asks.into_iter().map(|l| runtime_api::PriceLevelInfo {
                price: l.price,
                total_amount: l.total_amount,
                order_count: l.order_count,
            }).collect(),
            bids: depth.bids.into_iter().map(|l| runtime_api::PriceLevelInfo {
                price: l.price,
                total_amount: l.total_amount,
                order_count: l.order_count,
            }).collect(),
            best_ask: depth.best_ask,
            best_bid: depth.best_bid,
            spread: depth.spread,
            block_number: depth.block_number,
        }
    }

    pub fn api_get_best_prices(entity_id: u64) -> (Option<BalanceOf<T>>, Option<BalanceOf<T>>) {
        Self::get_best_prices(entity_id)
    }

    pub fn api_get_spread(entity_id: u64) -> Option<BalanceOf<T>> {
        Self::get_spread(entity_id)
    }

    pub fn api_get_market_summary(entity_id: u64) -> runtime_api::MarketSummaryInfo<BalanceOf<T>, T::TokenBalance> {
        let s = Self::get_market_summary(entity_id);
        runtime_api::MarketSummaryInfo {
            best_ask: s.best_ask,
            best_bid: s.best_bid,
            last_price: s.last_price,
            total_ask_amount: s.total_ask_amount,
            total_bid_amount: s.total_bid_amount,
        }
    }

    pub fn api_get_order_book_snapshot(entity_id: u64) -> (Vec<(BalanceOf<T>, T::TokenBalance)>, Vec<(BalanceOf<T>, T::TokenBalance)>) {
        Self::get_order_book_snapshot(entity_id)
    }

    pub fn api_get_user_trade_history(
        user: &T::AccountId,
        page: u32,
        page_size: u32,
    ) -> Vec<runtime_api::TradeInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Self::get_user_trade_history(user, page, page_size)
            .iter()
            .map(|t| Self::trade_to_info(t))
            .collect()
    }

    pub fn api_get_entity_trade_history(
        entity_id: u64,
        page: u32,
        page_size: u32,
    ) -> Vec<runtime_api::TradeInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Self::get_entity_trade_history(entity_id, page, page_size)
            .iter()
            .map(|t| Self::trade_to_info(t))
            .collect()
    }

    pub fn api_get_user_order_history(
        user: &T::AccountId,
        page: u32,
        page_size: u32,
    ) -> Vec<runtime_api::OrderInfo<T::AccountId, BalanceOf<T>, T::TokenBalance>> {
        Self::get_user_order_history(user, page, page_size)
            .iter()
            .map(|o| Self::order_to_info(o))
            .collect()
    }

    pub fn api_get_daily_stats(entity_id: u64) -> runtime_api::DailyStatsInfo<BalanceOf<T>> {
        let s = Self::get_daily_stats(entity_id);
        runtime_api::DailyStatsInfo {
            open_price: s.open_price,
            high_price: s.high_price,
            low_price: s.low_price,
            close_price: s.close_price,
            volume_nex: s.volume_nex,
            trade_count: s.trade_count,
            period_start: s.period_start,
        }
    }

    pub fn api_get_global_stats() -> runtime_api::MarketStatsInfo {
        let s = Self::get_global_stats();
        runtime_api::MarketStatsInfo {
            total_orders: s.total_orders,
            total_trades: s.total_trades,
            total_volume_nex: s.total_volume_nex,
        }
    }

    pub fn api_get_market_status(entity_id: u64) -> u8 {
        match Self::get_market_status(entity_id) {
            MarketStatus::Active => 0,
            MarketStatus::Closed => 1,
        }
    }

    pub fn api_get_market_config(entity_id: u64) -> Option<runtime_api::MarketConfigInfo> {
        MarketConfigs::<T>::get(entity_id).map(|c| runtime_api::MarketConfigInfo {
            nex_enabled: c.nex_enabled,
            min_order_amount: c.min_order_amount,
            order_ttl: c.order_ttl,
            paused: c.paused,
        })
    }

    pub fn api_get_kyc_requirement(entity_id: u64) -> u8 {
        Self::get_kyc_requirement(entity_id)
    }

    pub fn api_get_twap_info(entity_id: u64) -> runtime_api::TwapInfo<BalanceOf<T>> {
        let twap_1h = Self::calculate_twap(entity_id, TwapPeriod::OneHour);
        let twap_24h = Self::calculate_twap(entity_id, TwapPeriod::OneDay);
        let twap_7d = Self::calculate_twap(entity_id, TwapPeriod::OneWeek);
        let acc = TwapAccumulators::<T>::get(entity_id);
        runtime_api::TwapInfo {
            twap_1h,
            twap_24h,
            twap_7d,
            last_price: acc.as_ref().map(|a| a.last_price),
            trade_count: acc.map(|a| a.trade_count).unwrap_or(0),
        }
    }
}
