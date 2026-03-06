//! Benchmarking for pallet-commission-core
//!
//! MISSING-1 修复: 权重基准测试骨架，覆盖所有重量级 extrinsic。
//! 使用 `cargo bench -p pallet-commission-core --features runtime-benchmarks` 运行。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;
use pallet_commission_common::{CommissionModes, WithdrawalMode, WithdrawalTierConfig};

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn set_commission_modes() {
        let entity_id = 1u64;
        let caller: T::AccountId = whitelisted_caller();
        // Setup: ensure entity exists with caller as owner
        // TODO: wire up mock EntityProvider for benchmarks

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, CommissionModes(CommissionModes::DIRECT_REWARD));
    }

    #[benchmark]
    fn set_commission_rate() {
        let entity_id = 1u64;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, 5000u16);
    }

    #[benchmark]
    fn enable_commission() {
        let entity_id = 1u64;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, true);
    }

    #[benchmark]
    fn set_withdrawal_config() {
        let entity_id = 1u64;
        let caller: T::AccountId = whitelisted_caller();
        let default_tier = WithdrawalTierConfig {
            withdrawal_rate: 7000,
            repurchase_rate: 3000,
        };

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            entity_id,
            WithdrawalMode::FullWithdrawal,
            default_tier,
            BoundedVec::default(),
            0u16,
            true,
        );
    }

    #[benchmark]
    fn force_enable_entity_commission() {
        let entity_id = 1u64;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);
    }

    #[benchmark]
    fn force_disable_entity_commission() {
        let entity_id = 1u64;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);
    }

    #[benchmark]
    fn force_global_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);
    }

    #[benchmark]
    fn set_global_max_commission_rate() {
        let entity_id = 1u64;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 8000u16);
    }

    #[benchmark]
    fn set_global_max_token_commission_rate() {
        let entity_id = 1u64;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 8000u16);
    }

    #[benchmark]
    fn set_min_withdrawal_interval() {
        let entity_id = 1u64;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, 100u32);
    }

    #[benchmark]
    fn set_token_platform_fee_rate() {
        #[extrinsic_call]
        _(RawOrigin::Root, 200u16);
    }

    #[benchmark]
    fn set_global_min_repurchase_rate() {
        let entity_id = 1u64;

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, 2000u16);
    }

    #[benchmark]
    fn pause_withdrawals() {
        let entity_id = 1u64;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, true);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
