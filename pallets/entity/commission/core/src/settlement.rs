//! 购物余额结算子模块
//!
//! NEX 购物余额 + Token 购物余额均已迁移至 Loyalty 模块，
//! 本模块通过 T::Loyalty / T::LoyaltyToken Port 委托。
//!
//! - do_use_shopping_balance — 委托 Loyalty（纯记账，已废弃，仅保留兼容）
//! - do_consume_shopping_balance — 委托 Loyalty（记账 + NEX 转账）
//! - do_consume_token_shopping_balance — 委托 LoyaltyToken（记账 + Token 转账）

use crate::pallet::*;
use frame_support::pallet_prelude::*;
use pallet_entity_common::{LoyaltyWritePort, LoyaltyTokenWritePort};
use sp_runtime::traits::{Saturating, Zero};

#[allow(dead_code)]
impl<T: Config> Pallet<T> {
    /// 使用购物余额内部实现（委托给 Loyalty 模块）
    ///
    /// 已废弃：extrinsic 已禁用，order 模块直接调用 T::Loyalty。
    /// 保留仅用于 CommissionProvider trait 兼容。
    pub(crate) fn do_use_shopping_balance(
        entity_id: u64,
        account: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        T::Loyalty::consume_shopping_balance(entity_id, account, amount)?;

        Self::deposit_event(Event::ShoppingBalanceUsed {
            entity_id,
            account: account.clone(),
            amount,
        });

        Ok(())
    }

    /// 消费购物余额（委托给 Loyalty 模块，包含 KYC 检查 + NEX 转账）
    pub(crate) fn do_consume_shopping_balance(
        entity_id: u64,
        account: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

        T::Loyalty::consume_shopping_balance(entity_id, account, amount)?;

        Self::deposit_event(Event::ShoppingBalanceUsed {
            entity_id,
            account: account.clone(),
            amount,
        });

        Ok(())
    }

    /// 消费 Token 购物余额（委托给 LoyaltyToken 模块，包含 KYC 检查 + Token 转账）
    ///
    /// Loyalty 模块负责: 余额扣减 + Token 从 Entity 账户转入会员钱包。
    /// commission-core 负责: 更新 EntityTokenAccountedBalance 跟踪。
    pub(crate) fn do_consume_token_shopping_balance(
        entity_id: u64,
        account: &T::AccountId,
        amount: TokenBalanceOf<T>,
    ) -> DispatchResult {
        ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawalAmount);

        // 委托给 Loyalty 模块（余额扣减 + KYC 检查 + Token 转账）
        T::LoyaltyToken::consume_token_shopping_balance(entity_id, account, amount)?;

        // 更新 commission-core 的 Token 记账追踪
        EntityTokenAccountedBalance::<T>::mutate(entity_id, |b| {
            *b = b.map(|v| v.saturating_sub(amount));
        });

        Self::deposit_event(Event::TokenShoppingBalanceUsed {
            entity_id,
            account: account.clone(),
            amount,
        });

        Ok(())
    }
}
