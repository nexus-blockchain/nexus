//! # Escrow Pallet Benchmarking
//!
//! 托管模块基准测试
//! 🆕 L3-R3修复: 修正 5 个签名错误 + 补齐 8 个缺失基准（共 20 个 extrinsic）

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;

#[benchmarks]
mod benchmarks {
    use super::*;

    // call_index(0): lock(origin, id, payer, amount)
    #[benchmark]
    fn lock() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let payer: T::AccountId = account("payer", 1, 0);
        let amount: BalanceOf<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, payer, amount);
    }

    // call_index(1): release(origin, id, to)
    #[benchmark]
    fn release() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let to: T::AccountId = account("receiver", 1, 0);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, to);
    }

    // call_index(2): refund(origin, id, to)
    #[benchmark]
    fn refund() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let to: T::AccountId = account("refundee", 1, 0);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, to);
    }

    // call_index(3): lock_with_nonce(origin, id, payer, amount, nonce: u64)
    #[benchmark]
    fn lock_with_nonce() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let payer: T::AccountId = account("payer", 1, 0);
        let amount: BalanceOf<T> = 1000u32.into();
        let nonce: u64 = 1;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, payer, amount, nonce);
    }

    // call_index(4): release_split(origin, id, entries: BoundedVec<...>)
    #[benchmark]
    fn release_split() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let entries: BoundedVec<(T::AccountId, BalanceOf<T>), T::MaxSplitEntries> =
            BoundedVec::default();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, entries);
    }

    // call_index(5): dispute(origin, id, reason, detail: BoundedVec<u8, MaxReasonLen>)
    #[benchmark]
    fn dispute() {
        let id: u64 = 1;
        let reason: u16 = 1;
        let detail: BoundedVec<u8, T::MaxReasonLen> = BoundedVec::default();

        #[extrinsic_call]
        _(RawOrigin::Root, id, reason, detail);
    }

    // call_index(6): apply_decision_release_all(origin, id, to)
    #[benchmark]
    fn apply_decision_release_all() {
        let id: u64 = 1;
        let to: T::AccountId = account("receiver", 1, 0);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);
    }

    // call_index(7): apply_decision_refund_all(origin, id, to)
    #[benchmark]
    fn apply_decision_refund_all() {
        let id: u64 = 1;
        let to: T::AccountId = account("refundee", 1, 0);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);
    }

    // call_index(8): apply_decision_partial_bps(origin, id, release_to, refund_to, bps)
    #[benchmark]
    fn apply_decision_partial_bps() {
        let id: u64 = 1;
        let release_to: T::AccountId = account("receiver", 1, 0);
        let refund_to: T::AccountId = account("refundee", 2, 0);
        let release_bps: u16 = 5000;

        #[extrinsic_call]
        _(RawOrigin::Root, id, release_to, refund_to, release_bps);
    }

    // call_index(9): set_pause(origin, paused)
    #[benchmark]
    fn set_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);
    }

    // call_index(10): schedule_expiry(origin, id, at)
    #[benchmark]
    fn schedule_expiry() {
        let id: u64 = 1;
        let at: BlockNumberFor<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, id, at);
    }

    // call_index(11): cancel_expiry(origin, id)
    #[benchmark]
    fn cancel_expiry() {
        let id: u64 = 1;

        #[extrinsic_call]
        _(RawOrigin::Root, id);
    }

    // call_index(12): force_release(origin, id, to) — AdminOrigin
    #[benchmark]
    fn force_release() {
        let id: u64 = 1;
        let to: T::AccountId = account("receiver", 1, 0);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);
    }

    // call_index(13): force_refund(origin, id, to) — AdminOrigin
    #[benchmark]
    fn force_refund() {
        let id: u64 = 1;
        let to: T::AccountId = account("refundee", 1, 0);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);
    }

    // call_index(14): refund_partial(origin, id, to, amount)
    #[benchmark]
    fn refund_partial() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let to: T::AccountId = account("refundee", 1, 0);
        let amount: BalanceOf<T> = 500u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, to, amount);
    }

    // call_index(15): release_partial(origin, id, to, amount)
    #[benchmark]
    fn release_partial() {
        let caller: T::AccountId = whitelisted_caller();
        let id: u64 = 1;
        let to: T::AccountId = account("receiver", 1, 0);
        let amount: BalanceOf<T> = 500u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, to, amount);
    }

    // call_index(16): cleanup_closed(origin, ids: BoundedVec<u64, MaxCleanupPerCall>)
    #[benchmark]
    fn cleanup_closed() {
        let caller: T::AccountId = whitelisted_caller();
        let ids: BoundedVec<u64, T::MaxCleanupPerCall> = BoundedVec::default();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ids);
    }

    // call_index(17): token_lock(origin, entity_id, escrow_id, payer, amount: u128)
    #[benchmark]
    fn token_lock() {
        let caller: T::AccountId = whitelisted_caller();
        let entity_id: u64 = 1;
        let escrow_id: u64 = 1;
        let payer: T::AccountId = account("payer", 1, 0);
        let amount: u128 = 1000;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, escrow_id, payer, amount);
    }

    // call_index(18): token_release(origin, entity_id, escrow_id, to, amount: u128)
    #[benchmark]
    fn token_release() {
        let caller: T::AccountId = whitelisted_caller();
        let entity_id: u64 = 1;
        let escrow_id: u64 = 1;
        let to: T::AccountId = account("receiver", 1, 0);
        let amount: u128 = 500;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, escrow_id, to, amount);
    }

    // call_index(19): token_refund(origin, entity_id, escrow_id, to, amount: u128)
    #[benchmark]
    fn token_refund() {
        let caller: T::AccountId = whitelisted_caller();
        let entity_id: u64 = 1;
        let escrow_id: u64 = 1;
        let to: T::AccountId = account("refundee", 1, 0);
        let amount: u128 = 500;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id, escrow_id, to, amount);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
