//! 佣金计算引擎子模块
//!
//! 从 lib.rs 提取的佣金计算/记账/取消相关函数：
//! - process_commission — NEX 佣金调度引擎
//! - credit_commission — NEX 佣金记账
//! - process_token_commission — Token 佣金调度引擎
//! - credit_token_commission — Token 佣金记账
//! - cancel_commission — 取消 NEX 佣金
//! - do_cancel_token_commission — 取消 Token 佣金
//! - do_settle_order_records — 结算订单佣金记录

use crate::pallet::*;
use frame_support::pallet_prelude::*;
use frame_support::traits::{Currency, ExistenceRequirement};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_entity_common::{EntityProvider, LoyaltyTokenReadPort, ShopProvider};
use pallet_commission_common::{
    CommissionModes, CommissionStatus, CommissionType, CommissionRecord, TokenCommissionRecord,
    CommissionPlugin, TokenCommissionPlugin, EntityReferrerProvider,
    TokenTransferProvider,
};
use sp_runtime::traits::{Saturating, Zero};

impl<T: Config> Pallet<T> {
    /// BUG-1 修复: 将订单的所有 Pending 佣金记录结算为 Withdrawn
    ///
    /// 由订单模块在订单完结（确认收货/超时完成）时通过
    /// CommissionProvider::settle_order_commission 调用。
    /// 标记后记录可被 archive_order_records 归档释放存储。
    pub(crate) fn do_settle_order_records(order_id: u64) -> DispatchResult {
        OrderCommissionRecords::<T>::mutate(order_id, |records| {
            for record in records.iter_mut() {
                if record.status == CommissionStatus::Pending {
                    record.status = CommissionStatus::Withdrawn;
                }
            }
        });
        OrderTokenCommissionRecords::<T>::mutate(order_id, |records| {
            for record in records.iter_mut() {
                if record.status == CommissionStatus::Pending {
                    record.status = CommissionStatus::Withdrawn;
                }
            }
        });
        Self::deposit_event(Event::OrderRecordsSettled { order_id });
        Ok(())
    }

    /// 调度引擎：处理订单返佣（双来源架构）
    ///
    /// 订单来自 shop_id，佣金记账在 entity_id 级。
    /// 双来源并行：
    /// - 池 A（平台费池）：platform_fee × ReferrerShareBps → 招商推荐人奖金（EntityReferral）
    /// - 池 B（卖家池）：seller_balance × max_commission_rate → 会员返佣（4 个插件）
    pub(crate) fn process_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        order_amount: BalanceOf<T>,
        available_pool: BalanceOf<T>,
        platform_fee: BalanceOf<T>,
    ) -> DispatchResult {
        // F17: 全局紧急暂停检查
        ensure!(!GlobalCommissionPaused::<T>::get(), Error::<T>::GlobalCommissionPaused);

        let platform_account = T::PlatformAccount::get();

        // ── 平台费无条件转国库（无论佣金是否配置，保障平台收入） ──
        // 全局固定规则：referrer 拿 ReferrerShareBps%，剩余进国库
        let config = CommissionConfigs::<T>::get(entity_id)
            .filter(|c| c.enabled);

        // 计算推荐人奖金占比（有 referrer 时才预留）
        let global_referrer_bps = T::ReferrerShareBps::get();
        let has_referrer = global_referrer_bps > 0
            && T::EntityReferrerProvider::entity_referrer(entity_id).is_some();
        let referrer_quota = if has_referrer {
            platform_fee
                .saturating_mul(global_referrer_bps.into())
                / 10000u32.into()
        } else {
            BalanceOf::<T>::zero()
        };
        let treasury_portion = platform_fee.saturating_sub(referrer_quota);

        if !treasury_portion.is_zero() {
            let treasury_account = T::TreasuryAccount::get();
            let platform_balance = T::Currency::free_balance(&platform_account);
            let min_balance = T::Currency::minimum_balance();
            let platform_transferable = platform_balance.saturating_sub(min_balance);
            // 为推荐人预留额度：先扣除 referrer_quota，剩余才给国库
            let treasury_cap = platform_transferable.saturating_sub(referrer_quota);
            let actual_treasury = treasury_portion.min(treasury_cap);
            if !actual_treasury.is_zero() {
                T::Currency::transfer(
                    &platform_account,
                    &treasury_account,
                    actual_treasury,
                    ExistenceRequirement::KeepAlive,
                )?;
                OrderTreasuryTransfer::<T>::insert(order_id, actual_treasury);
                Self::deposit_event(Event::PlatformFeeToTreasury {
                    order_id,
                    amount: actual_treasury,
                });
            }
        }

        // 未配置佣金或未启用 → 平台费已入库，直接返回
        let config = match config {
            Some(c) => c,
            None => return Ok(()),
        };
        let seller = T::ShopProvider::shop_owner(shop_id)
            .ok_or(Error::<T>::ShopNotFound)?;
        let entity_account = T::EntityProvider::entity_account(entity_id);
        let now = <frame_system::Pallet<T>>::block_number();
        let buyer_stats = MemberCommissionStats::<T>::get(entity_id, buyer);
        let is_first_order = buyer_stats.order_count == 0;
        let enabled_modes = config.enabled_modes;

        let mut total_from_platform = BalanceOf::<T>::zero();
        let mut total_from_seller = BalanceOf::<T>::zero();

        // ── 池 A：招商推荐人奖金（从平台费扣除，比例由全局常量控制） ──
        // L1-R5 审计修复: 复用上方已计算的 referrer_quota 和 has_referrer，避免重复存储读取和计算
        if has_referrer {
            if let Some(referrer) = T::EntityReferrerProvider::entity_referrer(entity_id) {
                // KeepAlive 要求转账后余额 >= ED
                let platform_balance = T::Currency::free_balance(&platform_account);
                let min_balance = T::Currency::minimum_balance();
                let transferable = platform_balance.saturating_sub(min_balance);
                let referrer_amount = referrer_quota.min(transferable);
                if !referrer_amount.is_zero() {
                    Self::credit_commission(
                        entity_id, shop_id, order_id, buyer, &referrer,
                        referrer_amount, CommissionType::EntityReferral, 0, now,
                    )?;
                    total_from_platform = referrer_amount;
                }
            }
        }

        // ── 池 B：会员返佣（从卖家货款扣除） ──
        let max_commission = available_pool
            .saturating_mul(config.max_commission_rate.into())
            / 10000u32.into();
        let seller_balance = T::Currency::free_balance(&seller);
        let seller_min = T::Currency::minimum_balance();
        let seller_transferable = seller_balance.saturating_sub(seller_min);
        let mut remaining = max_commission.min(seller_transferable);

        if !remaining.is_zero() {
            let initial_remaining = remaining;

            // ── 创建人收益（从 Pool B 预算中优先扣除，在所有插件之前） ──
            if enabled_modes.contains(CommissionModes::CREATOR_REWARD) && config.creator_reward_rate > 0 {
                if let Some(creator) = T::EntityProvider::entity_owner(entity_id) {
                    let creator_amount = remaining
                        .saturating_mul(config.creator_reward_rate.into())
                        / 10000u32.into();
                    let creator_amount = creator_amount.min(remaining);
                    if !creator_amount.is_zero() {
                        Self::credit_commission(
                            entity_id, shop_id, order_id, buyer, &creator,
                            creator_amount, CommissionType::CreatorReward, 0, now,
                        )?;
                        remaining = remaining.saturating_sub(creator_amount);
                    }
                }
            }

            // 1. Referral Plugin
            let (outputs, new_remaining) = T::ReferralPlugin::calculate(
                entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 2. MultiLevel Plugin
            let (outputs, new_remaining) = T::MultiLevelPlugin::calculate(
                entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 3. LevelDiff Plugin
            let (outputs, new_remaining) = T::LevelDiffPlugin::calculate(
                entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 4. SingleLine Plugin
            let (outputs, new_remaining) = T::SingleLinePlugin::calculate(
                entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            // 5. Team Plugin
            let (outputs, new_remaining) = T::TeamPlugin::calculate(
                entity_id, buyer, order_amount, remaining, enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_commission(
                    entity_id, shop_id, order_id, buyer, &output.beneficiary, output.amount,
                    output.commission_type, output.level, now,
                )?;
            }

            total_from_seller = initial_remaining.saturating_sub(remaining);
        }

        // ── Phase 1.5：未分配佣金 → 沉淀资金池 ──
        let mut pool_funded = BalanceOf::<T>::zero();
        if enabled_modes.contains(CommissionModes::POOL_REWARD) && !remaining.is_zero() {
            let seller_balance_now = T::Currency::free_balance(&seller);
            let seller_min = T::Currency::minimum_balance();
            let seller_transferable_now = seller_balance_now.saturating_sub(seller_min);
            let actual_pool = remaining.min(seller_transferable_now);
            if !actual_pool.is_zero() {
                T::Currency::transfer(
                    &seller,
                    &entity_account,
                    actual_pool,
                    ExistenceRequirement::KeepAlive,
                )?;
                UnallocatedPool::<T>::mutate(entity_id, |pool| {
                    *pool = pool.saturating_add(actual_pool);
                });
                OrderUnallocated::<T>::insert(order_id, (entity_id, shop_id, actual_pool));
                pool_funded = actual_pool;
                Self::deposit_event(Event::UnallocatedCommissionPooled {
                    entity_id,
                    order_id,
                    amount: actual_pool,
                });
            }
        }

        // Phase 2 已移除：沉淀池奖励改为用户主动 claim（pool-reward v2）
        let total_pool_distributed = BalanceOf::<T>::zero();

        // 更新买家订单数（Entity 级）
        MemberCommissionStats::<T>::mutate(entity_id, buyer, |stats| {
            stats.order_count = stats.order_count.saturating_add(1);
        });

        // total_distributed 仅统计从外部转入的佣金（不含池内循环）
        let total_distributed = total_from_platform.saturating_add(total_from_seller);

        // 更新 Entity 统计（含池奖励）
        ShopCommissionTotals::<T>::mutate(entity_id, |(total, orders)| {
            *total = total.saturating_add(total_distributed).saturating_add(total_pool_distributed);
            *orders = orders.saturating_add(1);
        });

        // 将佣金资金转入 Entity 账户（双来源分别转；池资金已在 entity_account 中）
        if !total_from_platform.is_zero() {
            T::Currency::transfer(
                &platform_account,
                &entity_account,
                total_from_platform,
                ExistenceRequirement::KeepAlive,
            )?;
        }

        if !total_from_seller.is_zero() {
            T::Currency::transfer(
                &seller,
                &entity_account,
                total_from_seller,
                ExistenceRequirement::KeepAlive,
            )?;
        }

        if !total_distributed.is_zero() || !pool_funded.is_zero() {
            Self::deposit_event(Event::CommissionFundsTransferred {
                entity_id,
                shop_id,
                amount: total_distributed.saturating_add(pool_funded),
            });
        }

        Ok(())
    }

    /// 记录并发放返佣（Entity 级记账）
    pub(crate) fn credit_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        beneficiary: &T::AccountId,
        amount: BalanceOf<T>,
        commission_type: CommissionType,
        level: u8,
        now: BlockNumberFor<T>,
    ) -> DispatchResult {
        let record = CommissionRecord {
            entity_id,
            shop_id,
            order_id,
            buyer: buyer.clone(),
            beneficiary: beneficiary.clone(),
            amount,
            commission_type,
            level,
            status: CommissionStatus::Pending,
            created_at: now,
        };

        OrderCommissionRecords::<T>::try_mutate(order_id, |records| {
            records.try_push(record).map_err(|_| Error::<T>::RecordsFull)
        })?;

        MemberCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
            stats.total_earned = stats.total_earned.saturating_add(amount);
            stats.pending = stats.pending.saturating_add(amount);
        });

        // 更新最后入账时间（用于冻结期检查）
        MemberLastCredited::<T>::insert(entity_id, beneficiary, now);

        ShopPendingTotal::<T>::mutate(entity_id, |total| {
            *total = total.saturating_add(amount);
        });

        // 推荐链类型佣金：维护 ReferrerEarnedByBuyer 索引
        if matches!(commission_type,
            CommissionType::DirectReward
            | CommissionType::FirstOrder
            | CommissionType::RepeatPurchase
            | CommissionType::FixedAmount
        ) {
            ReferrerEarnedByBuyer::<T>::mutate(
                (entity_id, beneficiary, buyer),
                |earned| { *earned = earned.saturating_add(amount); },
            );
        }

        // F19: 记录会员佣金关联订单 ID（去重，满则丢弃最旧）
        MemberCommissionOrderIds::<T>::mutate(entity_id, beneficiary, |ids| {
            if !ids.contains(&order_id) {
                if ids.try_push(order_id).is_err() {
                    // 满了 → 移除最旧的，腾出空间
                    if !ids.is_empty() {
                        ids.remove(0);
                    }
                    let _ = ids.try_push(order_id);
                }
            }
        });

        Self::deposit_event(Event::CommissionDistributed {
            entity_id,
            order_id,
            beneficiary: beneficiary.clone(),
            amount,
            commission_type,
            level,
        });

        Ok(())
    }

    // ====================================================================
    // Token 多资产管线
    // ====================================================================

    /// Token 调度引擎：处理 Token 订单返佣（双源架构）
    ///
    /// 池 A（Token 平台费池）：token_platform_fee → 招商推荐人 Token 奖金 + Entity 留存
    /// 池 B（Entity Token 池）：entity_account Token × max_rate → 4 插件 → 沉淀池
    pub(crate) fn process_token_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        token_order_amount: TokenBalanceOf<T>,
        token_available_pool: TokenBalanceOf<T>,
        token_platform_fee: TokenBalanceOf<T>,
    ) -> DispatchResult {
        // F17: 全局紧急暂停检查
        ensure!(!GlobalCommissionPaused::<T>::get(), Error::<T>::GlobalCommissionPaused);

        // M2-R5 审计修复: 先 sweep 再检查配置，未配置时优雅返回（与 NEX 版 process_commission 对称）
        Self::sweep_token_free_balance(entity_id, token_platform_fee);

        let config = match CommissionConfigs::<T>::get(entity_id).filter(|c| c.enabled) {
            Some(c) => c,
            None => return Ok(()),
        };

        let enabled_modes = config.enabled_modes;
        let entity_account = T::EntityProvider::entity_account(entity_id);
        let now = <frame_system::Pallet<T>>::block_number();
        let buyer_stats = MemberTokenCommissionStats::<T>::get(entity_id, buyer);
        let is_first_order = buyer_stats.order_count == 0;

        // ── 池 A：Token 招商推荐人奖金（从 Token 平台费中分配） ──
        let mut pool_a_distributed = TokenBalanceOf::<T>::zero();
        let referrer_share_bps = T::ReferrerShareBps::get();
        if referrer_share_bps > 0 && !token_platform_fee.is_zero() {
            if let Some(referrer) = T::EntityReferrerProvider::entity_referrer(entity_id) {
                let referrer_quota = token_platform_fee
                    .saturating_mul(referrer_share_bps.into())
                    / 10000u32.into();
                if !referrer_quota.is_zero() {
                    Self::credit_token_commission(
                        entity_id, order_id, buyer, &referrer,
                        referrer_quota, CommissionType::EntityReferral, 0, now,
                    )?;
                    pool_a_distributed = referrer_quota;
                }
            }
        }
        // 池 A 剩余部分计入沉淀池（不留为 FREE_BALANCE）
        let pool_a_retention = token_platform_fee.saturating_sub(pool_a_distributed);
        if !pool_a_retention.is_zero() {
            UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
                *pool = pool.saturating_add(pool_a_retention);
            });
            // M2-R6 审计修复: 记录 Pool A 留存，供 cancel 时回退
            OrderTokenPlatformRetention::<T>::insert(order_id, (entity_id, pool_a_retention));
            Self::deposit_event(Event::TokenUnallocatedPooled {
                entity_id, order_id, amount: pool_a_retention,
            });
        }

        // ── 池 B：会员 Token 返佣（从 entity_account Token 余额中分配） ──
        let max_commission = token_available_pool
            .saturating_mul(config.max_commission_rate.into())
            / 10000u32.into();

        let entity_token_balance = T::TokenTransferProvider::token_balance_of(
            entity_id, &entity_account,
        );
        // H1 审计修复: Token 佣金预算必须扣除已承诺的 Token 额度
        // （包括待提现佣金、购物余额、沉淀池）避免跨订单重复承诺
        // NEX 管线无此问题——转账即时发生，seller 余额自然递减；
        // Token 管线是纯记账模式，entity_token_balance 不变，需手动扣除。
        let committed = TokenPendingTotal::<T>::get(entity_id)
            .saturating_add(T::LoyaltyToken::token_shopping_total(entity_id))
            .saturating_add(UnallocatedTokenPool::<T>::get(entity_id));
        let available_token = entity_token_balance.saturating_sub(committed);
        let mut remaining = max_commission.min(available_token);

        if !remaining.is_zero() {
            // ── 创建人收益（从 Token Pool B 预算中优先扣除） ──
            if enabled_modes.contains(CommissionModes::CREATOR_REWARD) && config.creator_reward_rate > 0 {
                if let Some(creator) = T::EntityProvider::entity_owner(entity_id) {
                    let creator_amount = remaining
                        .saturating_mul(config.creator_reward_rate.into())
                        / 10000u32.into();
                    let creator_amount = creator_amount.min(remaining);
                    if !creator_amount.is_zero() {
                        Self::credit_token_commission(
                            entity_id, order_id, buyer, &creator,
                            creator_amount, CommissionType::CreatorReward, 0, now,
                        )?;
                        remaining = remaining.saturating_sub(creator_amount);
                    }
                }
            }

            // 1. Token Referral Plugin
            let (outputs, new_remaining) = T::TokenReferralPlugin::calculate_token(
                entity_id, buyer, token_order_amount, remaining,
                enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_token_commission(
                    entity_id, order_id, buyer, &output.beneficiary,
                    output.amount, output.commission_type, output.level, now,
                )?;
            }

            // 2. Token MultiLevel Plugin
            let (outputs, new_remaining) = T::TokenMultiLevelPlugin::calculate_token(
                entity_id, buyer, token_order_amount, remaining,
                enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_token_commission(
                    entity_id, order_id, buyer, &output.beneficiary,
                    output.amount, output.commission_type, output.level, now,
                )?;
            }

            // 3. Token LevelDiff Plugin
            let (outputs, new_remaining) = T::TokenLevelDiffPlugin::calculate_token(
                entity_id, buyer, token_order_amount, remaining,
                enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_token_commission(
                    entity_id, order_id, buyer, &output.beneficiary,
                    output.amount, output.commission_type, output.level, now,
                )?;
            }

            // 4. Token SingleLine Plugin
            let (outputs, new_remaining) = T::TokenSingleLinePlugin::calculate_token(
                entity_id, buyer, token_order_amount, remaining,
                enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_token_commission(
                    entity_id, order_id, buyer, &output.beneficiary,
                    output.amount, output.commission_type, output.level, now,
                )?;
            }

            // 5. Token Team Plugin
            let (outputs, new_remaining) = T::TokenTeamPlugin::calculate_token(
                entity_id, buyer, token_order_amount, remaining,
                enabled_modes, is_first_order, buyer_stats.order_count,
            );
            remaining = new_remaining;
            for output in outputs {
                Self::credit_token_commission(
                    entity_id, order_id, buyer, &output.beneficiary,
                    output.amount, output.commission_type, output.level, now,
                )?;
            }
        }

        // 剩余 Token → 沉淀池
        if enabled_modes.contains(CommissionModes::POOL_REWARD) && !remaining.is_zero() {
            UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
                *pool = pool.saturating_add(remaining);
            });
            OrderTokenUnallocated::<T>::insert(order_id, (entity_id, shop_id, remaining));
            Self::deposit_event(Event::TokenUnallocatedPooled {
                entity_id, order_id, amount: remaining,
            });
        }

        // 更新买家订单数（Token 版）
        MemberTokenCommissionStats::<T>::mutate(entity_id, buyer, |stats| {
            stats.order_count = stats.order_count.saturating_add(1);
        });

        Ok(())
    }

    /// Token 佣金记账（纯记账，不转账——Token 在 entity_account 中托管直到提现）
    pub(crate) fn credit_token_commission(
        entity_id: u64,
        order_id: u64,
        buyer: &T::AccountId,
        beneficiary: &T::AccountId,
        amount: TokenBalanceOf<T>,
        commission_type: CommissionType,
        level: u8,
        now: BlockNumberFor<T>,
    ) -> DispatchResult {
        let record = TokenCommissionRecord {
            entity_id,
            order_id,
            buyer: buyer.clone(),
            beneficiary: beneficiary.clone(),
            amount,
            commission_type,
            level,
            status: CommissionStatus::Pending,
            created_at: now,
        };

        OrderTokenCommissionRecords::<T>::try_mutate(order_id, |records| {
            records.try_push(record).map_err(|_| Error::<T>::RecordsFull)
        })?;

        MemberTokenCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
            stats.total_earned = stats.total_earned.saturating_add(amount);
            stats.pending = stats.pending.saturating_add(amount);
        });

        TokenPendingTotal::<T>::mutate(entity_id, |total| {
            *total = total.saturating_add(amount);
        });

        // P3 审计修复: Token 入账更新独立冻结时间（与 NEX 的 MemberLastCredited 解耦）
        MemberTokenLastCredited::<T>::insert(entity_id, beneficiary, now);

        // F19: 记录会员 Token 佣金关联订单 ID（去重，满则丢弃最旧）
        MemberTokenCommissionOrderIds::<T>::mutate(entity_id, beneficiary, |ids| {
            if !ids.contains(&order_id) {
                if ids.try_push(order_id).is_err() {
                    if !ids.is_empty() {
                        ids.remove(0);
                    }
                    let _ = ids.try_push(order_id);
                }
            }
        });

        Self::deposit_event(Event::TokenCommissionDistributed {
            entity_id, order_id,
            beneficiary: beneficiary.clone(),
            amount, commission_type, level,
        });

        Ok(())
    }

    /// 取消订单返佣（双来源架构）
    ///
    /// 按 CommissionType 决定退款目标：
    /// - `EntityReferral`: Entity 账户 → 平台账户
    /// - 其余: Entity 账户 → 卖家 (shop_owner)
    ///
    /// H2 审计修复: 先尝试转账，成功后再取消记录和更新统计，
    /// 防止转账失败但记录已被标记为 Cancelled 导致资金丢失。
    pub(crate) fn cancel_commission(order_id: u64) -> DispatchResult {
        let records = OrderCommissionRecords::<T>::get(order_id);
        let platform_account = T::PlatformAccount::get();

        // 第一步：按 (entity_id, shop_id, is_platform) 分组汇总待退还金额
        // is_platform = true → EntityReferral（退平台），false → 会员返佣（退卖家）
        // PoolReward 记录不参与转账退款（资金回池）
        let mut refund_groups: alloc::vec::Vec<(u64, u64, bool, BalanceOf<T>)> = alloc::vec::Vec::new();
        let mut pool_return_groups: alloc::vec::Vec<(u64, BalanceOf<T>)> = alloc::vec::Vec::new();

        for record in records.iter() {
            if record.status == CommissionStatus::Pending {
                if record.commission_type == CommissionType::PoolReward {
                    if let Some(entry) = pool_return_groups.iter_mut().find(|(e, _)| *e == record.entity_id) {
                        entry.1 = entry.1.saturating_add(record.amount);
                    } else {
                        pool_return_groups.push((record.entity_id, record.amount));
                    }
                } else {
                    let is_platform = record.commission_type == CommissionType::EntityReferral;
                    if let Some(entry) = refund_groups.iter_mut().find(|(e, s, p, _)| *e == record.entity_id && *s == record.shop_id && *p == is_platform) {
                        entry.3 = entry.3.saturating_add(record.amount);
                    } else {
                        refund_groups.push((record.entity_id, record.shop_id, is_platform, record.amount));
                    }
                }
            }
        }

        // 第二步：尝试转账
        let mut refund_succeeded: alloc::vec::Vec<(u64, u64, bool)> = alloc::vec::Vec::new();

        for (entity_id, shop_id, is_platform, refund_amount) in refund_groups.iter() {
            if refund_amount.is_zero() {
                refund_succeeded.push((*entity_id, *shop_id, *is_platform));
                continue;
            }
            let entity_account = T::EntityProvider::entity_account(*entity_id);

            let refund_target = if *is_platform {
                platform_account.clone()
            } else {
                match T::ShopProvider::shop_owner(*shop_id) {
                    Some(seller) => seller,
                    None => {
                        Self::deposit_event(Event::CommissionRefundFailed {
                            entity_id: *entity_id,
                            shop_id: *shop_id,
                            amount: *refund_amount,
                        });
                        continue;
                    }
                }
            };

            if T::Currency::transfer(
                &entity_account,
                &refund_target,
                *refund_amount,
                ExistenceRequirement::KeepAlive,
            ).is_ok() {
                refund_succeeded.push((*entity_id, *shop_id, *is_platform));
            } else {
                Self::deposit_event(Event::CommissionRefundFailed {
                    entity_id: *entity_id,
                    shop_id: *shop_id,
                    amount: *refund_amount,
                });
            }
        }

        // 第三步：仅取消转账成功的记录，更新统计
        // PoolReward 记录无需转账，直接回池并取消
        for (entity_id, return_amount) in pool_return_groups.iter() {
            if !return_amount.is_zero() {
                UnallocatedPool::<T>::mutate(entity_id, |pool| {
                    *pool = pool.saturating_add(*return_amount);
                });
            }
        }

        OrderCommissionRecords::<T>::mutate(order_id, |records| {
            for record in records.iter_mut() {
                if record.status == CommissionStatus::Pending {
                    if record.commission_type == CommissionType::PoolReward {
                        MemberCommissionStats::<T>::mutate(record.entity_id, &record.beneficiary, |stats| {
                            stats.pending = stats.pending.saturating_sub(record.amount);
                            stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                        });
                        ShopPendingTotal::<T>::mutate(record.entity_id, |total| {
                            *total = total.saturating_sub(record.amount);
                        });
                        record.status = CommissionStatus::Cancelled;
                    } else {
                        let is_platform = record.commission_type == CommissionType::EntityReferral;
                        if refund_succeeded.iter().any(|(e, s, p)| *e == record.entity_id && *s == record.shop_id && *p == is_platform) {
                            MemberCommissionStats::<T>::mutate(record.entity_id, &record.beneficiary, |stats| {
                                stats.pending = stats.pending.saturating_sub(record.amount);
                                stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                            });
                            ShopPendingTotal::<T>::mutate(record.entity_id, |total| {
                                *total = total.saturating_sub(record.amount);
                            });
                            // 推荐链类型：同步扣减 ReferrerEarnedByBuyer
                            if matches!(record.commission_type,
                                CommissionType::DirectReward
                                | CommissionType::FirstOrder
                                | CommissionType::RepeatPurchase
                                | CommissionType::FixedAmount
                            ) {
                                ReferrerEarnedByBuyer::<T>::mutate(
                                    (record.entity_id, &record.beneficiary, &record.buyer),
                                    |earned| { *earned = earned.saturating_sub(record.amount); },
                                );
                            }
                            record.status = CommissionStatus::Cancelled;
                        }
                    }
                }
            }
        });

        // 第四步：退还国库部分（Treasury → PlatformAccount）
        let treasury_refund = OrderTreasuryTransfer::<T>::get(order_id);
        if !treasury_refund.is_zero() {
            let treasury_account = T::TreasuryAccount::get();
            if T::Currency::transfer(
                &treasury_account,
                &platform_account,
                treasury_refund,
                ExistenceRequirement::AllowDeath,
            ).is_ok() {
                OrderTreasuryTransfer::<T>::remove(order_id);
                Self::deposit_event(Event::TreasuryRefund {
                    order_id,
                    amount: treasury_refund,
                });
            } else {
                // 国库余额不足时记录事件，保留 Storage 供后续重试
                Self::deposit_event(Event::CommissionRefundFailed {
                    entity_id: 0,
                    shop_id: 0,
                    amount: treasury_refund,
                });
            }
        }

        // 第五步：退还本订单沉淀池贡献（entity_account → seller）
        let (unalloc_entity_id, unalloc_shop_id, unalloc_amount) = OrderUnallocated::<T>::get(order_id);
        if !unalloc_amount.is_zero() {
            let entity_account = T::EntityProvider::entity_account(unalloc_entity_id);
            if let Some(seller) = T::ShopProvider::shop_owner(unalloc_shop_id) {
                if T::Currency::transfer(
                    &entity_account,
                    &seller,
                    unalloc_amount,
                    ExistenceRequirement::KeepAlive,
                ).is_ok() {
                    UnallocatedPool::<T>::mutate(unalloc_entity_id, |pool| {
                        *pool = pool.saturating_sub(unalloc_amount);
                    });
                    OrderUnallocated::<T>::remove(order_id);
                    Self::deposit_event(Event::UnallocatedPoolRefunded {
                        entity_id: unalloc_entity_id,
                        order_id,
                        amount: unalloc_amount,
                    });
                }
            }
        }

        // CC-M1: 汇总退款结果
        let succeeded = refund_succeeded.len() as u32;
        let failed = (refund_groups.len() as u32).saturating_sub(succeeded);
        Self::deposit_event(Event::CommissionCancelled { order_id, refund_succeeded: succeeded, refund_failed: failed });

        // M3 审计修复: 复用 do_cancel_token_commission 消除代码重复（原第六、七步）
        Self::do_cancel_token_commission(order_id)?;

        Ok(())
    }

    /// Token 佣金独立取消（供 TokenCommissionProvider::cancel_token_commission 调用）
    pub(crate) fn do_cancel_token_commission(order_id: u64) -> DispatchResult {
        let mut token_cancelled: u32 = 0;
        OrderTokenCommissionRecords::<T>::mutate(order_id, |records| {
            for record in records.iter_mut() {
                if record.status == CommissionStatus::Pending {
                    MemberTokenCommissionStats::<T>::mutate(
                        record.entity_id, &record.beneficiary, |stats| {
                            stats.pending = stats.pending.saturating_sub(record.amount);
                            stats.total_earned = stats.total_earned.saturating_sub(record.amount);
                        }
                    );
                    TokenPendingTotal::<T>::mutate(record.entity_id, |total| {
                        *total = total.saturating_sub(record.amount);
                    });
                    record.status = CommissionStatus::Cancelled;
                    token_cancelled = token_cancelled.saturating_add(1);
                }
            }
        });

        // H2 审计修复: 退还 Token 沉淀池 — 仅在转账成功时扣减池余额
        let (te_id, ts_id, t_amount) = OrderTokenUnallocated::<T>::get(order_id);
        if !t_amount.is_zero() {
            let mut refund_ok = false;
            let entity_account = T::EntityProvider::entity_account(te_id);
            if let Some(seller) = T::ShopProvider::shop_owner(ts_id) {
                if T::TokenTransferProvider::token_transfer(
                    te_id, &entity_account, &seller, t_amount,
                ).is_ok() {
                    refund_ok = true;
                }
            }
            if refund_ok {
                UnallocatedTokenPool::<T>::mutate(te_id, |pool| {
                    *pool = pool.saturating_sub(t_amount);
                });
                EntityTokenAccountedBalance::<T>::mutate(te_id, |b| {
                    *b = b.map(|v| v.saturating_sub(t_amount));
                });
                OrderTokenUnallocated::<T>::remove(order_id);
                Self::deposit_event(Event::TokenUnallocatedPoolRefunded {
                    entity_id: te_id, order_id, amount: t_amount,
                });
            }
        }

        // M2-R6 审计修复: 回退 Pool A 留存（token_platform_fee 中未分配给 referrer 的部分）
        // 与 NEX cancel 退还 OrderTreasuryTransfer 对称
        let (retention_entity_id, retention_amount) = OrderTokenPlatformRetention::<T>::get(order_id);
        if !retention_amount.is_zero() {
            UnallocatedTokenPool::<T>::mutate(retention_entity_id, |pool| {
                *pool = pool.saturating_sub(retention_amount);
            });
            OrderTokenPlatformRetention::<T>::remove(order_id);
        }

        if token_cancelled > 0 {
            Self::deposit_event(Event::TokenCommissionCancelled {
                order_id, cancelled_count: token_cancelled,
            });
        }

        Ok(())
    }
}
