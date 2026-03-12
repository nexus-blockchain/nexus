//! 风险控制：市场验证、熔断器检查、KYC 门控

use crate::pallet::*;
use frame_support::pallet_prelude::*;
use pallet_entity_common::{EntityProvider, EntityTokenProvider, KycProvider};
use sp_runtime::SaturatedConversion;

impl<T: Config> Pallet<T> {
    /// 验证市场是否启用
    /// H4 审计修复: 添加 is_entity_active 检查，Banned/Closed 实体不允许新订单
    pub(crate) fn ensure_market_enabled(entity_id: u64) -> DispatchResult {
        // 全局暂停检查
        ensure!(!GlobalMarketPaused::<T>::get(), Error::<T>::GlobalMarketPausedError);
        ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
        ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
        ensure!(
            T::TokenProvider::is_token_enabled(entity_id),
            Error::<T>::TokenNotEnabled
        );

        // P6: 市场已关闭检查
        ensure!(MarketStatusStorage::<T>::get(entity_id) != MarketStatus::Closed, Error::<T>::MarketAlreadyClosed);

        // M6: 检查市场配置（必须显式配置并启用，与 Default nex_enabled=false 一致）
        let config = MarketConfigs::<T>::get(entity_id).unwrap_or_default();
        ensure!(config.nex_enabled, Error::<T>::MarketNotEnabled);
        ensure!(!config.paused, Error::<T>::MarketPaused);

        Ok(())
    }

    /// 审计修复 H2-R7: 独立的熔断器检查（市价单/吃单不经过 check_price_deviation）
    /// 审计修复 S2-R11: 到期后自动清理存储状态，避免 circuit_breaker_active 残留
    pub(crate) fn ensure_circuit_breaker_inactive(entity_id: u64) -> DispatchResult {
        if let Some(config) = PriceProtection::<T>::get(entity_id) {
            if config.enabled && config.circuit_breaker_active {
                let current_block: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
                if current_block < config.circuit_breaker_until {
                    return Err(Error::<T>::MarketCircuitBreakerActive.into());
                }
                // 到期后自动清理存储
                PriceProtection::<T>::mutate(entity_id, |maybe_config| {
                    if let Some(c) = maybe_config {
                        c.circuit_breaker_active = false;
                        c.circuit_breaker_until = 0;
                    }
                });
                Self::deposit_event(Event::CircuitBreakerLifted { entity_id });
            }
        }
        Ok(())
    }

    /// P4: 检查用户 KYC 级别是否满足市场要求
    pub(crate) fn ensure_kyc_requirement(entity_id: u64, who: &T::AccountId) -> DispatchResult {
        let min_level = MarketKycRequirement::<T>::get(entity_id);
        if min_level > 0 {
            let user_level = T::KycProvider::kyc_level(entity_id, who);
            ensure!(user_level >= min_level, Error::<T>::InsufficientKycLevel);
        }
        Ok(())
    }
}
