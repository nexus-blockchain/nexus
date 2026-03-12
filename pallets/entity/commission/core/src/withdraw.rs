//! 提现计算子模块
//!
//! 从 lib.rs 提取的提现/复购/奖励分配相关函数：
//! - calc_withdrawal_split — NEX 提现分配计算
//! - calc_token_withdrawal_split — Token 提现分配计算
//! - is_pool_reward_locked — 沉淀池锁定判断
//! - ensure_owner_or_admin — 权限校验（Owner 或 Admin）
//! - ensure_entity_owner — 权限校验（仅 Owner）
//! - sweep_token_free_balance — Token 外部转入归集

use crate::pallet::*;
use frame_support::pallet_prelude::*;
use pallet_entity_common::{AdminPermission, EntityProvider};
use pallet_commission_common::{CommissionModes, WithdrawalMode, MemberProvider, TokenTransferProvider};
use sp_runtime::traits::{Saturating, Zero};

impl<T: Config> Pallet<T> {
    /// 判断沉淀池是否锁定（不可被 Entity Owner 提取）
    ///
    /// 锁定条件：
    /// - POOL_REWARD 开启 → 锁定（资金用于会员领奖）
    /// - POOL_REWARD 关闭但 cooldown 未满 → 锁定（防套利）
    /// - POOL_REWARD 关闭且 cooldown 已满 → 不锁定（可提取）
    /// - 从未配置 POOL_REWARD → 不锁定
    pub(crate) fn is_pool_reward_locked(entity_id: u64) -> bool {
        let pool_reward_on = CommissionConfigs::<T>::get(entity_id)
            .map(|c| c.enabled && c.enabled_modes.contains(CommissionModes::POOL_REWARD))
            .unwrap_or(false);

        if pool_reward_on {
            return true;
        }

        // POOL_REWARD 未开启，检查是否有 cooldown
        if let Some(disabled_at) = PoolRewardDisabledAt::<T>::get(entity_id) {
            let now = <frame_system::Pallet<T>>::block_number();
            let cooldown = T::PoolRewardWithdrawCooldown::get();
            if now < disabled_at.saturating_add(cooldown) {
                return true; // cooldown 期内仍锁定
            }
        }

        false
    }

    // L1 审计修复: 移除死代码 resolve_entity_id（未被任何代码路径调用）

    /// F1: 验证 Entity Owner 或 Admin(COMMISSION_MANAGE) 权限
    pub(crate) fn ensure_owner_or_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult {
        let owner = T::EntityProvider::entity_owner(entity_id)
            .ok_or(Error::<T>::EntityNotFound)?;
        if *who == owner {
            return Ok(());
        }
        ensure!(
            T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::COMMISSION_MANAGE),
            Error::<T>::NotEntityOwnerOrAdmin
        );
        Ok(())
    }

    /// 验证 Entity Owner（仅 Owner，不含 Admin — 用于资金提取等敏感操作）
    pub(crate) fn ensure_entity_owner(entity_id: u64, who: &T::AccountId) -> Result<(), DispatchError> {
        let owner = T::EntityProvider::entity_owner(entity_id)
            .ok_or(Error::<T>::EntityNotFound)?;
        ensure!(*who == owner, Error::<T>::NotEntityOwner);
        Ok(())
    }

    /// 检测并归集外部直接转入 entity_account 的 Token 到沉淀池。
    ///
    /// `incoming`: 本次已知的合法入账金额（如 token_platform_fee），
    /// 在调用前已到达 entity_account，不应被视为外部转入。
    ///
    /// 原理：EntityTokenAccountedBalance 记录 entity_account 通过已知渠道应有的余额，
    /// actual_balance - (accounted + incoming) > 0 即为外部转入。
    pub(crate) fn sweep_token_free_balance(entity_id: u64, incoming: TokenBalanceOf<T>) {
        let entity_account = T::EntityProvider::entity_account(entity_id);
        let actual = T::TokenTransferProvider::token_balance_of(
            entity_id, &entity_account,
        );
        let accounted = EntityTokenAccountedBalance::<T>::get(entity_id)
            .unwrap_or_else(|| actual.saturating_sub(incoming));
        let expected = accounted.saturating_add(incoming);
        let external = actual.saturating_sub(expected);
        if !external.is_zero() {
            UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
                *pool = pool.saturating_add(external);
            });
            Self::deposit_event(Event::TokenUnallocatedPooled {
                entity_id,
                order_id: 0,
                amount: external,
            });
        }
        // 快照当前实际余额（含 incoming + external）
        EntityTokenAccountedBalance::<T>::insert(entity_id, actual);
    }

    /// 计算提现/复购/奖励分配（Entity 级，四种模式）
    ///
    /// 三层约束模型：
    /// ```text
    /// Governance 底线（强制）
    ///     ↓ max()
    /// Entity 模式设定（FullWithdrawal / FixedRate / LevelBased / MemberChoice）
    ///     ↓ max()
    /// 会员选择（MemberChoice 模式下的 requested_rate）
    ///     ↓
    /// 最终复购比率
    /// ```
    ///
    /// 自愿多复购奖励：超出强制最低线的部分 × voluntary_bonus_rate 额外计入购物余额
    pub(crate) fn calc_withdrawal_split(
        entity_id: u64,
        who: &T::AccountId,
        total_amount: BalanceOf<T>,
        requested_repurchase_rate: Option<u16>,
    ) -> WithdrawalSplit<BalanceOf<T>> {
        let zero = BalanceOf::<T>::zero();
        let config = WithdrawalConfigs::<T>::get(entity_id);

        // Step 1: 根据模式确定 Entity 层面的复购比率
        // - mandatory_base_rate: 模式强制最低线（不含 governance）
        // - mode_final_rate: 模式最终值（MemberChoice 允许高于 mandatory_base_rate）
        let (mandatory_base_rate, mode_final_rate, voluntary_bonus_rate) = match config {
            Some(ref config) if config.enabled => {
                match &config.mode {
                    WithdrawalMode::FullWithdrawal => (0u16, 0u16, config.voluntary_bonus_rate),
                    WithdrawalMode::FixedRate { repurchase_rate } => {
                        (*repurchase_rate, *repurchase_rate, config.voluntary_bonus_rate)
                    },
                    WithdrawalMode::LevelBased => {
                        let level_id = T::MemberProvider::custom_level_id(entity_id, who);
                        let tier = config.level_overrides
                            .iter()
                            .find(|(id, _)| *id == level_id)
                            .map(|(_, t)| t.clone())
                            .unwrap_or(config.default_tier.clone());
                        (tier.repurchase_rate, tier.repurchase_rate, config.voluntary_bonus_rate)
                    },
                    WithdrawalMode::MemberChoice { min_repurchase_rate } => {
                        let requested = requested_repurchase_rate
                            .unwrap_or(*min_repurchase_rate)
                            .min(10000);
                        let mode_rate = requested.max(*min_repurchase_rate);
                        (*min_repurchase_rate, mode_rate, config.voluntary_bonus_rate)
                    },
                }
            },
            _ => (0u16, 0u16, 0u16),
        };

        // Step 2: Governance 底线兜底
        let gov_min_rate = GlobalMinRepurchaseRate::<T>::get(entity_id);
        let mandatory_min_rate = mandatory_base_rate.max(gov_min_rate).min(10000);
        let final_repurchase_rate = mode_final_rate.max(gov_min_rate).min(10000);

        // Step 4: 计算金额
        let final_withdrawal_rate = 10000u16.saturating_sub(final_repurchase_rate);
        let withdrawal = total_amount
            .saturating_mul(final_withdrawal_rate.into())
            / 10000u32.into();
        let repurchase = total_amount.saturating_sub(withdrawal);

        // Step 5: 计算自愿多复购奖励
        // 超出强制最低线的部分 × voluntary_bonus_rate
        let bonus = if voluntary_bonus_rate > 0 && final_repurchase_rate > mandatory_min_rate {
            let mandatory_repurchase = total_amount
                .saturating_mul(mandatory_min_rate.into())
                / 10000u32.into();
            let voluntary_extra = repurchase.saturating_sub(mandatory_repurchase);
            voluntary_extra
                .saturating_mul(voluntary_bonus_rate.into())
                / 10000u32.into()
        } else {
            zero
        };

        WithdrawalSplit { withdrawal, repurchase, bonus }
    }

    /// Token 提现分配计算（与 NEX calc_withdrawal_split 对称，使用 Token 独立配置）
    pub(crate) fn calc_token_withdrawal_split(
        entity_id: u64,
        who: &T::AccountId,
        total_amount: TokenBalanceOf<T>,
        requested_repurchase_rate: Option<u16>,
    ) -> WithdrawalSplit<TokenBalanceOf<T>> {
        let zero = TokenBalanceOf::<T>::zero();
        let config = TokenWithdrawalConfigs::<T>::get(entity_id);

        let (mandatory_base_rate, mode_final_rate, voluntary_bonus_rate) = match config {
            Some(ref config) if config.enabled => {
                match &config.mode {
                    WithdrawalMode::FullWithdrawal => (0u16, 0u16, config.voluntary_bonus_rate),
                    WithdrawalMode::FixedRate { repurchase_rate } => {
                        (*repurchase_rate, *repurchase_rate, config.voluntary_bonus_rate)
                    },
                    WithdrawalMode::LevelBased => {
                        let level_id = T::MemberProvider::custom_level_id(entity_id, who);
                        let tier = config.level_overrides
                            .iter()
                            .find(|(id, _)| *id == level_id)
                            .map(|(_, t)| t.clone())
                            .unwrap_or(config.default_tier.clone());
                        (tier.repurchase_rate, tier.repurchase_rate, config.voluntary_bonus_rate)
                    },
                    WithdrawalMode::MemberChoice { min_repurchase_rate } => {
                        let requested = requested_repurchase_rate
                            .unwrap_or(*min_repurchase_rate)
                            .min(10000);
                        let mode_rate = requested.max(*min_repurchase_rate);
                        (*min_repurchase_rate, mode_rate, config.voluntary_bonus_rate)
                    },
                }
            },
            _ => (0u16, 0u16, 0u16),
        };

        // Governance 底线兜底（Token 独立配置）
        let gov_min_rate = GlobalMinTokenRepurchaseRate::<T>::get(entity_id);
        let mandatory_min_rate = mandatory_base_rate.max(gov_min_rate).min(10000);
        let final_repurchase_rate = mode_final_rate.max(gov_min_rate).min(10000);

        // 计算金额
        let final_withdrawal_rate = 10000u16.saturating_sub(final_repurchase_rate);
        let withdrawal = total_amount
            .saturating_mul(final_withdrawal_rate.into())
            / 10000u32.into();
        let repurchase = total_amount.saturating_sub(withdrawal);

        // 自愿多复购奖励
        let bonus = if voluntary_bonus_rate > 0 && final_repurchase_rate > mandatory_min_rate {
            let mandatory_repurchase = total_amount
                .saturating_mul(mandatory_min_rate.into())
                / 10000u32.into();
            let voluntary_extra = repurchase.saturating_sub(mandatory_repurchase);
            voluntary_extra
                .saturating_mul(voluntary_bonus_rate.into())
                / 10000u32.into()
        } else {
            zero
        };

        WithdrawalSplit { withdrawal, repurchase, bonus }
    }
}
