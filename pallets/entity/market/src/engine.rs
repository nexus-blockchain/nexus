//! 交易引擎：撮合、订单创建、市价单执行、订单簿管理

use crate::pallet::*;
use alloc::vec::Vec;
use frame_support::pallet_prelude::*;
use frame_support::traits::{Currency, ExistenceRequirement, ReservableCurrency};
use pallet_entity_common::EntityTokenProvider;
use sp_runtime::traits::{Saturating, Zero};
use sp_runtime::SaturatedConversion;

impl<T: Config> Pallet<T> {
        pub(crate) fn record_trade(
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
        pub(crate) fn add_to_order_history(who: &T::AccountId, order_id: u64) {
            UserOrderHistory::<T>::mutate(who, |history| {
                if history.len() as u32 >= T::MaxOrderHistoryPerUser::get() {
                    history.remove(0);
                }
                let _ = history.try_push(order_id);
            });
        }

        /// P1/P11: 更新交易统计（实体 + 全局）
        pub(crate) fn update_trade_stats(entity_id: u64, nex_amount: BalanceOf<T>) {
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
        pub fn do_cancel_all_entity_orders(entity_id: u64) -> u32 {
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
        pub(crate) fn check_fillable_amount(
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
        pub(crate) fn calculate_total_next(token_amount: u128, price: BalanceOf<T>) -> Result<BalanceOf<T>, DispatchError> {
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
        pub(crate) fn do_cross_match(
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
        pub(crate) fn do_create_order(
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
        pub(crate) fn update_order_fill(
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
        pub(crate) fn remove_from_order_book(entity_id: u64, order_id: u64, side: OrderSide) {
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
        pub(crate) fn do_market_buy(
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
        pub(crate) fn do_market_sell(
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
        pub(crate) fn update_best_prices(entity_id: u64) {
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
        pub(crate) fn update_last_trade_price(entity_id: u64, price: BalanceOf<T>) {
            LastTradePrice::<T>::insert(entity_id, price);
        }
}
