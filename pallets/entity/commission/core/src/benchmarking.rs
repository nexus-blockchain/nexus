//! Benchmarking for pallet-commission-core
#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;
use pallet_commission_common::{CommissionModes, CommissionStatus, WithdrawalMode, WithdrawalTierConfig};

fn setup_entity<T: Config>(eid: u64, caller: &T::AccountId) {
    #[cfg(test)]
    {
        use crate::mock;
        mock::set_entity_owner(eid, 0);
        mock::set_shop_entity(100, eid);
        mock::set_shop_owner(100, 0);
    }
    CommissionConfigs::<T>::insert(eid, CoreCommissionConfig {
        enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
        max_commission_rate: 5000,
        enabled: true,
        withdrawal_cooldown: 0,
        creator_reward_rate: 0,
        token_withdrawal_cooldown: 0,
    });
    WithdrawalConfigs::<T>::insert(eid, EntityWithdrawalConfig::<T::MaxCustomLevels> {
        mode: WithdrawalMode::FullWithdrawal,
        default_tier: WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
        level_overrides: BoundedVec::default(),
        voluntary_bonus_rate: 0,
        enabled: true,
    });
    let _ = caller;
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn set_commission_modes() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, CommissionModes(CommissionModes::DIRECT_REWARD));
    }

    #[benchmark]
    fn set_commission_rate() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, 5000u16);
    }

    #[benchmark]
    fn enable_commission() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, true);
    }

    #[benchmark]
    fn withdraw_commission() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[cfg(test)]
        {
            use crate::mock;
            mock::fund(mock::entity_account(1), 1_000_000);
        }
        MemberCommissionStats::<T>::mutate(1u64, &c, |s| {
            s.pending = 10_000u32.into();
            s.total_earned = 10_000u32.into();
        });
        ShopPendingTotal::<T>::insert(1u64, BalanceOf::<T>::from(10_000u32));
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, None, None, None);
    }

    #[benchmark]
    fn set_withdrawal_config() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, WithdrawalMode::FullWithdrawal,
          WithdrawalTierConfig { withdrawal_rate: 7000, repurchase_rate: 3000 },
          BoundedVec::default(), 0u16, true);
    }

    #[benchmark]
    fn withdraw_token_commission() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[cfg(test)]
        {
            use crate::mock;
            let ea = mock::entity_account(1);
            mock::set_token_balance(1, ea, 1_000_000);
        }
        EntityTokenAccountedBalance::<T>::insert(1u64, TokenBalanceOf::<T>::from(1_000_000u32));
        MemberTokenCommissionStats::<T>::mutate(1u64, &c, |s| {
            s.pending = 10_000u32.into();
            s.total_earned = 10_000u32.into();
        });
        TokenPendingTotal::<T>::insert(1u64, TokenBalanceOf::<T>::from(10_000u32));
        TokenWithdrawalConfigs::<T>::insert(1u64, EntityWithdrawalConfig::<T::MaxCustomLevels> {
            mode: WithdrawalMode::FullWithdrawal,
            default_tier: WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            level_overrides: BoundedVec::default(),
            voluntary_bonus_rate: 0,
            enabled: true,
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, None, None, None);
    }

    #[benchmark]
    fn set_token_withdrawal_config() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, WithdrawalMode::FullWithdrawal,
          WithdrawalTierConfig { withdrawal_rate: 7000, repurchase_rate: 3000 },
          BoundedVec::default(), 0u16, true);
    }

    #[benchmark]
    fn set_creator_reward_rate() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, 2500u16);
    }

    #[benchmark]
    fn set_withdrawal_cooldown() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, 100u32, 200u32);
    }

    #[benchmark]
    fn clear_commission_config() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64);
    }

    #[benchmark]
    fn clear_withdrawal_config() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64);
    }

    #[benchmark]
    fn clear_token_withdrawal_config() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        TokenWithdrawalConfigs::<T>::insert(1u64, EntityWithdrawalConfig::<T::MaxCustomLevels> {
            mode: WithdrawalMode::FullWithdrawal,
            default_tier: WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            level_overrides: BoundedVec::default(),
            voluntary_bonus_rate: 0,
            enabled: true,
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64);
    }

    #[benchmark]
    fn pause_withdrawals() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, true);
    }

    #[benchmark]
    fn set_min_withdrawal_interval() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, 100u32);
    }

    #[benchmark]
    fn archive_order_records() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        OrderCommissionRecords::<T>::mutate(9999u64, |r| {
            let _ = r.try_push(CommissionRecordOf::<T> {
                entity_id: 1,
                shop_id: 100,
                order_id: 9999,
                buyer: c.clone(),
                beneficiary: c.clone(),
                amount: 1000u32.into(),
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Withdrawn,
                created_at: 1u32.into(),
            });
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, 9999u64);
    }

    #[benchmark]
    fn withdraw_entity_funds() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[cfg(test)]
        {
            use crate::mock;
            mock::fund(mock::entity_account(1), 1_000_000);
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, 1000u32.into());
    }

    #[benchmark]
    fn withdraw_entity_token_funds() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity::<T>(1, &c);
        #[cfg(test)]
        {
            use crate::mock;
            let ea = mock::entity_account(1);
            mock::set_token_balance(1, ea, 1_000_000);
        }
        EntityTokenAccountedBalance::<T>::insert(1u64, TokenBalanceOf::<T>::from(1_000_000u32));
        #[extrinsic_call]
        _(RawOrigin::Signed(c), 1u64, TokenBalanceOf::<T>::from(1000u32));
    }

    // Root extrinsics
    #[benchmark]
    fn force_enable_entity_commission() {
        setup_entity::<T>(1, &whitelisted_caller::<T::AccountId>());
        #[extrinsic_call]
        _(RawOrigin::Root, 1u64);
    }

    #[benchmark]
    fn force_disable_entity_commission() {
        setup_entity::<T>(1, &whitelisted_caller::<T::AccountId>());
        #[extrinsic_call]
        _(RawOrigin::Root, 1u64);
    }

    #[benchmark]
    fn force_global_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);
    }

    #[benchmark]
    fn set_global_max_commission_rate() {
        #[extrinsic_call]
        _(RawOrigin::Root, 1u64, 8000u16);
    }

    #[benchmark]
    fn set_global_max_token_commission_rate() {
        #[extrinsic_call]
        _(RawOrigin::Root, 1u64, 8000u16);
    }

    #[benchmark]
    fn set_token_platform_fee_rate() {
        #[extrinsic_call]
        _(RawOrigin::Root, 200u16);
    }

    #[benchmark]
    fn set_global_min_repurchase_rate() {
        #[extrinsic_call]
        _(RawOrigin::Root, 1u64, 2000u16);
    }

    #[benchmark]
    fn set_global_min_token_repurchase_rate() {
        #[extrinsic_call]
        _(RawOrigin::Root, 1u64, 2000u16);
    }

    #[benchmark]
    fn retry_cancel_commission() {
        #[extrinsic_call]
        _(RawOrigin::Root, 8888u64);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
