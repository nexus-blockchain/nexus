//! 订单簿查询：深度快照、价格查询、交易历史、统计

use crate::pallet::*;
use alloc::vec::Vec;
use sp_runtime::traits::Saturating;
use sp_runtime::SaturatedConversion;

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
        use sp_runtime::traits::Zero;

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
    pub(crate) fn calculate_best_ask(entity_id: u64) -> Option<BalanceOf<T>> {
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
    pub(crate) fn calculate_best_bid(entity_id: u64) -> Option<BalanceOf<T>> {
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
        let (best_ask, best_bid) = Self::get_best_prices(entity_id);
        match (best_ask, best_bid) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask.saturating_sub(bid)),
            _ => None,
        }
    }

    /// 获取市场摘要
    pub fn get_market_summary(entity_id: u64) -> MarketSummary<BalanceOf<T>, T::TokenBalance> {
        use sp_runtime::traits::Zero;

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
