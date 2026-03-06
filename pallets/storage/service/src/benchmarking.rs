//! # Storage Service Pallet Benchmarking
//!
//! 存储服务模块基准测试

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn fund_user_account() {
        let caller: T::AccountId = whitelisted_caller();
        let target: T::AccountId = account("target", 1, 0);
        let amount: BalanceOf<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), target, amount);
    }

    #[benchmark]
    fn request_pin_for_subject() {
        let caller: T::AccountId = whitelisted_caller();
        let subject_id: u64 = 1;
        let cid: Vec<u8> = b"QmCID".to_vec();
        let size_bytes: u64 = 1024;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), subject_id, cid, size_bytes, None);
    }

    #[benchmark]
    fn charge_due(n: Linear<1, 100>) {
        #[extrinsic_call]
        _(RawOrigin::Root, n);
    }

    #[benchmark]
    fn set_billing_params() {
        #[extrinsic_call]
        _(RawOrigin::Root, None, None, None, None, None);
    }

    #[benchmark]
    fn set_replicas_config() {
        #[extrinsic_call]
        _(RawOrigin::Root, None, None);
    }

    #[benchmark]
    fn distribute_to_operators() {
        let max_amount: BalanceOf<T> = 10000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, max_amount);
    }

    #[benchmark]
    fn mark_pinned() {
        let cid: Vec<u8> = b"QmCID".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Root, cid);
    }

    #[benchmark]
    fn mark_pin_failed() {
        let cid: Vec<u8> = b"QmCID".to_vec();
        let error_code: u16 = 1;

        #[extrinsic_call]
        _(RawOrigin::Root, cid, error_code);
    }

    #[benchmark]
    fn join_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let capacity: u64 = 1000;
        let bond: BalanceOf<T> = 10000u32.into();
        let endpoint: Vec<u8> = b"https://ipfs.example.com".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), capacity, bond, endpoint);
    }

    #[benchmark]
    fn update_operator() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), None, None);
    }

    #[benchmark]
    fn leave_operator() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn set_operator_status() {
        let operator: T::AccountId = account("operator", 1, 0);
        let status: u8 = 0;

        #[extrinsic_call]
        _(RawOrigin::Root, operator, status);
    }

    #[benchmark]
    fn pause_operator() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn resume_operator() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn report_probe() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), true);
    }

    #[benchmark]
    fn slash_operator() {
        let operator: T::AccountId = account("operator", 1, 0);
        let amount: BalanceOf<T> = 100u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, operator, amount);
    }

    #[benchmark]
    fn update_tier_config() {
        #[extrinsic_call]
        _(RawOrigin::Root, 0, None, None, None);
    }

    #[benchmark]
    fn operator_claim_rewards() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn emergency_pause_billing() {
        #[extrinsic_call]
        _(RawOrigin::Root);
    }

    #[benchmark]
    fn resume_billing() {
        #[extrinsic_call]
        _(RawOrigin::Root);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
