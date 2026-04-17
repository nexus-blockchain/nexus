//! 买家信用系统
//!
//! 在现有保证金体系之上增加信用分 + 并发控制 + 保证金折扣 + 自动封禁。
//!
//! - 新用户默认 500 分，范围 0-1000
//! - 完成交易加分，违约扣分（指数递增）
//! - 连续 3 次违约 → 暂停 7 天
//! - 信用分归零 → 永久 ban
//! - 高信用享受保证金折扣（900+ → 50%）
//! - 并发交易数随完成量阶梯提升

use crate::pallet::*;
use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_trading_common::DepositCalculator;
use sp_runtime::traits::Saturating;
use sp_runtime::SaturatedConversion;

/// 信用分变更原因
#[derive(
    Encode,
    Decode,
    codec::DecodeWithMemTracking,
    Clone,
    Copy,
    PartialEq,
    Eq,
    TypeInfo,
    MaxEncodedLen,
    RuntimeDebug,
)]
pub enum CreditChangeReason {
    /// 交易完成
    OrderCompleted,
    /// 超时违约
    TimeoutViolation,
    /// 少付违约
    UnderpaidViolation,
    /// 连续完成奖励
    ConsecutiveBonus,
    /// 30 天自然恢复
    NaturalRecovery,
}

/// 买家信用档案
#[derive(
    Encode,
    Decode,
    codec::DecodeWithMemTracking,
    Clone,
    PartialEq,
    Eq,
    TypeInfo,
    MaxEncodedLen,
    RuntimeDebug,
)]
pub struct BuyerCreditProfile<BlockNumber: codec::Codec + MaxEncodedLen> {
    /// 信用分（初始 500, 范围 0-1000）
    pub credit_score: u16,
    /// 累计完成交易数
    pub completed_orders: u32,
    /// 累计违约次数
    pub total_violations: u32,
    /// 连续成功交易数（+5 奖励 @每 10 单）
    pub consecutive_good_orders: u32,
    /// 连续违约次数（3 次 → 自动暂停）
    pub consecutive_violations: u32,
    /// 是否被暂停
    pub is_suspended: bool,
    /// 暂停截止区块
    pub suspension_until: Option<BlockNumber>,
    /// 上次违约区块
    pub last_violation_at: Option<BlockNumber>,
}

impl<BlockNumber: codec::Codec + MaxEncodedLen> Default for BuyerCreditProfile<BlockNumber> {
    fn default() -> Self {
        Self {
            credit_score: 500,
            completed_orders: 0,
            total_violations: 0,
            consecutive_good_orders: 0,
            consecutive_violations: 0,
            is_suspended: false,
            suspension_until: None,
            last_violation_at: None,
        }
    }
}

// ==================== 纯函数 ====================

/// 最大折扣比例（BPS）— 保证金折扣的硬上限。
/// 5000 = 50%，即保证金率 10% 折扣后最低 5%。
/// `deposit_discount_bps` 的返回值不得低于此值，`apply_credit_discount` 的比例地板也基于此计算。
const MAX_DISCOUNT_BPS: u16 = 5000;

/// 并发交易上限（基于累计完成数）
pub fn max_concurrent_trades(completed_orders: u32) -> u32 {
    match completed_orders {
        0..=2 => 1,
        3..=9 => 2,
        10..=49 => 3,
        _ => 5,
    }
}

/// 保证金折扣率（BPS, 10000 = 100% 即无折扣）
///
/// 安全约束：返回值 >= MAX_DISCOUNT_BPS，确保保证金率不低于 BuyerDepositRate 的一半。
/// apply_credit_discount 中有 floor 兜底，此处为第一道防线。
pub fn deposit_discount_bps(credit_score: u16) -> u16 {
    match credit_score {
        900..=1000 => MAX_DISCOUNT_BPS, // 50% — 硬上限，不可再低
        800..=899 => 7000,              // 70%
        700..=799 => 9000,              // 90%
        _ => 10000,                     // 100% 无折扣
    }
}

/// 违约扣分（连续违约指数递增）
pub fn violation_penalty(consecutive_violations: u32, is_underpaid: bool) -> u16 {
    if is_underpaid {
        30 // 少付固定 -30
    } else {
        // 超时: -50, -100, -200, -400（最多左移 3 次）
        let shift = consecutive_violations.saturating_sub(1).min(3);
        50u16.checked_shl(shift).unwrap_or(400)
    }
}

/// 完成交易加分
pub fn completion_reward(completed_orders: u32, consecutive_good_orders: u32) -> u16 {
    let base: u16 = if completed_orders <= 3 { 50 } else { 10 };
    let bonus: u16 = if consecutive_good_orders > 0 && consecutive_good_orders % 10 == 0 {
        5
    } else {
        0
    };
    base + bonus
}

// ==================== 有状态方法 ====================

impl<T: Config> Pallet<T> {
    /// 读取 profile（不存在则返回默认 score=500）
    pub(crate) fn get_or_default_credit(
        buyer: &T::AccountId,
    ) -> BuyerCreditProfile<BlockNumberFor<T>> {
        BuyerCreditProfiles::<T>::get(buyer).unwrap_or_default()
    }

    /// M-4 审计修复: 重置信用档案到默认状态
    ///
    /// 保留 completed_orders（历史记录），重置其他字段。
    /// 被 unban_user 和 admin_reset_credit 共用。
    pub(crate) fn do_reset_credit(account: &T::AccountId) {
        let old_profile = Self::get_or_default_credit(account);
        let old_score = old_profile.credit_score;
        let new_profile = BuyerCreditProfile {
            credit_score: 500,
            completed_orders: old_profile.completed_orders,
            total_violations: old_profile.total_violations, // 保留历史记录
            consecutive_good_orders: 0,
            consecutive_violations: 0,
            is_suspended: false,
            suspension_until: None,
            last_violation_at: None,
        };
        BuyerCreditProfiles::<T>::insert(account, &new_profile);
        Self::deposit_event(Event::CreditReset {
            account: account.clone(),
            old_score,
            new_score: 500,
        });
    }

    /// 完成交易 → 加分
    pub(crate) fn credit_on_completion(buyer: &T::AccountId) {
        let mut profile = Self::get_or_default_credit(buyer);
        let old_score = profile.credit_score;

        profile.completed_orders = profile.completed_orders.saturating_add(1);
        profile.consecutive_good_orders = profile.consecutive_good_orders.saturating_add(1);
        profile.consecutive_violations = 0; // 重置连续违约

        let reward = completion_reward(profile.completed_orders, profile.consecutive_good_orders);
        profile.credit_score = old_score.saturating_add(reward).min(1000);

        BuyerCreditProfiles::<T>::insert(buyer, &profile);

        Self::deposit_event(Event::CreditScoreUpdated {
            buyer: buyer.clone(),
            old_score,
            new_score: profile.credit_score,
            reason: CreditChangeReason::OrderCompleted,
        });

        // 每 10 连续成功额外 +5（已包含在 completion_reward 中，发独立事件便于追踪）
        if profile.consecutive_good_orders > 0 && profile.consecutive_good_orders % 10 == 0 {
            Self::deposit_event(Event::CreditScoreUpdated {
                buyer: buyer.clone(),
                old_score: profile.credit_score.saturating_sub(5),
                new_score: profile.credit_score,
                reason: CreditChangeReason::ConsecutiveBonus,
            });
        }
    }

    /// 超时违约 → 减分 + 检查自动封禁
    pub(crate) fn credit_on_timeout_violation(buyer: &T::AccountId) {
        Self::apply_violation(buyer, false);
    }

    /// 少付违约 → 减分（较轻）
    pub(crate) fn credit_on_underpaid_violation(buyer: &T::AccountId) {
        Self::apply_violation(buyer, true);
    }

    /// 内部：应用违约扣分逻辑
    fn apply_violation(buyer: &T::AccountId, is_underpaid: bool) {
        let mut profile = Self::get_or_default_credit(buyer);
        let old_score = profile.credit_score;
        let now = <frame_system::Pallet<T>>::block_number();

        profile.total_violations = profile.total_violations.saturating_add(1);
        profile.consecutive_violations = profile.consecutive_violations.saturating_add(1);
        profile.consecutive_good_orders = 0; // 重置连续成功
        profile.last_violation_at = Some(now);

        let penalty = violation_penalty(profile.consecutive_violations, is_underpaid);
        profile.credit_score = old_score.saturating_sub(penalty);

        let reason = if is_underpaid {
            CreditChangeReason::UnderpaidViolation
        } else {
            CreditChangeReason::TimeoutViolation
        };

        // 连续 3 次违约 → 暂停 7 天
        if profile.consecutive_violations >= 3 {
            profile.is_suspended = true;
            let seven_days: BlockNumberFor<T> =
                (T::BlocksPerDay::get().saturating_mul(7u32)).into();
            profile.suspension_until = Some(now.saturating_add(seven_days));
            // P7: 保留 1 而非归零，解除暂停后再犯 2 次即触发（而非 3 次）
            profile.consecutive_violations = 1;

            BuyerCreditProfiles::<T>::insert(buyer, &profile);

            Self::deposit_event(Event::CreditScoreUpdated {
                buyer: buyer.clone(),
                old_score,
                new_score: profile.credit_score,
                reason,
            });
            Self::deposit_event(Event::BuyerSuspended {
                buyer: buyer.clone(),
                credit_score: profile.credit_score,
                until: profile.suspension_until,
            });
        } else {
            BuyerCreditProfiles::<T>::insert(buyer, &profile);

            Self::deposit_event(Event::CreditScoreUpdated {
                buyer: buyer.clone(),
                old_score,
                new_score: profile.credit_score,
                reason,
            });
        }

        // 信用分 = 0 → 永久 ban（复用现有 BannedAccounts）
        if profile.credit_score == 0 {
            BannedAccounts::<T>::insert(buyer, true);
            Self::deposit_event(Event::BuyerPermanentlyBanned {
                buyer: buyer.clone(),
            });
        }
    }

    /// 检查暂停状态（含 lazy 30 天恢复）
    pub(crate) fn ensure_not_suspended(buyer: &T::AccountId) -> DispatchResult {
        if let Some(mut profile) = BuyerCreditProfiles::<T>::get(buyer) {
            let now = <frame_system::Pallet<T>>::block_number();

            // 1. 时间暂停到期 → 自动解除（只看时间，不看 score）
            if profile.is_suspended {
                if let Some(until) = profile.suspension_until {
                    if now > until {
                        profile.is_suspended = false;
                        profile.suspension_until = None;
                        BuyerCreditProfiles::<T>::insert(buyer, &profile);
                        Self::deposit_event(Event::BuyerSuspensionLifted {
                            buyer: buyer.clone(),
                            credit_score: profile.credit_score,
                        });
                        // 解除后继续走 score < 500 检查（步骤 3）
                    } else {
                        return Err(Error::<T>::BuyerSuspended.into());
                    }
                } else {
                    return Err(Error::<T>::BuyerSuspended.into());
                }
            }

            // 2. Lazy 30 天恢复：每过 30 天 +10，补偿缺勤期间所有窗口，上限恢复到 500
            if let Some(last_vio) = profile.last_violation_at {
                let thirty_days: BlockNumberFor<T> =
                    (T::BlocksPerDay::get().saturating_mul(30u32)).into();
                if now > last_vio.saturating_add(thirty_days) && profile.credit_score < 500 {
                    let elapsed = now.saturating_sub(last_vio);
                    // 整数除法：完整经过的 30 天窗口数
                    let thirty_days_u64: u64 = thirty_days.try_into().unwrap_or(1u64).max(1);
                    let elapsed_u64: u64 = elapsed.try_into().unwrap_or(u64::MAX);
                    let periods = elapsed_u64 / thirty_days_u64;
                    let old = profile.credit_score;
                    let recovery = (periods as u16).saturating_mul(10);
                    // 自然恢复上限 500 — 超过 500 的信用必须通过完成交易获得
                    profile.credit_score = old.saturating_add(recovery).min(500);
                    // P6: 对齐到最后一个完整窗口的结束点（而非 now），避免频繁检查跳跃恢复
                    let consumed_blocks: BlockNumberFor<T> =
                        (thirty_days_u64.saturating_mul(periods)).saturated_into();
                    profile.last_violation_at = Some(last_vio.saturating_add(consumed_blocks));
                    BuyerCreditProfiles::<T>::insert(buyer, &profile);
                    if profile.credit_score != old {
                        Self::deposit_event(Event::CreditScoreUpdated {
                            buyer: buyer.clone(),
                            old_score: old,
                            new_score: profile.credit_score,
                            reason: CreditChangeReason::NaturalRecovery,
                        });
                    }
                }
            }

            // 3. score < 500 且未暂停 → 也视为不可交易
            if profile.credit_score < 500 {
                return Err(Error::<T>::BuyerSuspended.into());
            }
        }
        // 无 profile = 新用户，允许
        Ok(())
    }

    /// 检查并发交易限制
    pub(crate) fn ensure_trade_limit(buyer: &T::AccountId) -> DispatchResult {
        let profile = Self::get_or_default_credit(buyer);
        let active = ActiveBuyerTrades::<T>::get(buyer);
        let limit = max_concurrent_trades(profile.completed_orders);
        ensure!(active < limit, Error::<T>::TradeLimitExceeded);
        Ok(())
    }

    /// 计算折扣后保证金
    ///
    /// 双重地板保护：
    /// 1. base × 50% — 保证金率不低于 BuyerDepositRate 的一半（10% → 5%）
    /// 2. min_deposit（MinBuyerDepositUsd 换算）— 绝对底线不被折扣击穿
    ///
    /// 效果：大额交易享受折扣，小额交易（base ≈ min_deposit）折扣受限或不折扣。
    pub(crate) fn apply_credit_discount(
        buyer: &T::AccountId,
        base_deposit: BalanceOf<T>,
    ) -> BalanceOf<T> {
        let profile = Self::get_or_default_credit(buyer);
        let discount_bps = deposit_discount_bps(profile.credit_score);
        if discount_bps >= 10000 {
            return base_deposit;
        }
        let base_u128: u128 = base_deposit.saturated_into();
        let discounted = base_u128
            .saturating_mul(discount_bps as u128)
            .saturating_div(10000);
        // 地板 1：比例地板 — 折扣后不低于 base × MAX_DISCOUNT_BPS / 10000
        let ratio_floor = base_u128
            .saturating_mul(MAX_DISCOUNT_BPS as u128)
            .saturating_div(10000);
        // 地板 2：绝对地板 — 折扣后不低于 MinBuyerDepositUsd 换算的 NEX 值
        // 注：此函数在 calculate_buyer_deposit（已验证 Oracle）之后调用，Oracle 必定可用
        let abs_floor: u128 =
            T::DepositCalculator::calculate_deposit(T::MinBuyerDepositUsd::get(), Zero::zero())
                .saturated_into();
        let floor = ratio_floor.max(abs_floor);
        // 如果地板 >= base，折扣无意义，直接返回 base（不加价）
        if floor >= base_u128 {
            return base_deposit;
        }
        let result = discounted.max(floor);
        result.saturated_into()
    }
}
