#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::{vec, vec::Vec};
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_support::BoundedVec;
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use pallet::*;
use sp_runtime::Saturating;

const SEED: u32 = 0;

fn fund_escrow_account<T: Config>(amount: BalanceOf<T>) {
    let escrow_acct = Pallet::<T>::account();
    let _ = T::Currency::make_free_balance_be(&escrow_acct, amount);
}

fn setup_locked<T: Config>(id: u64, amount: BalanceOf<T>) {
    fund_escrow_account::<T>(amount.saturating_mul(10u32.into()));
    Locked::<T>::insert(id, amount);
    LockStateOf::<T>::insert(id, 0u8);
}

fn setup_disputed<T: Config>(id: u64, amount: BalanceOf<T>) {
    setup_locked::<T>(id, amount);
    LockStateOf::<T>::insert(id, 1u8);
    DisputedAt::<T>::insert(id, frame_system::Pallet::<T>::block_number());
}

fn setup_closed<T: Config>(id: u64) {
    Locked::<T>::insert(id, BalanceOf::<T>::from(0u32));
    LockStateOf::<T>::insert(id, 3u8);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn lock() {
        let payer: T::AccountId = account("payer", 1, SEED);
        let amount: BalanceOf<T> = 1000u32.into();
        let _ = T::Currency::make_free_balance_be(&payer, amount.saturating_mul(10u32.into()));
        fund_escrow_account::<T>(amount);
        let id: u64 = 1;

        #[extrinsic_call]
        _(RawOrigin::Root, id, payer, amount);

        assert!(Locked::<T>::get(id) > BalanceOf::<T>::from(0u32));
    }

    #[benchmark]
    fn release() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        let to: T::AccountId = account("receiver", 1, SEED);
        setup_locked::<T>(id, amount);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
    }

    #[benchmark]
    fn refund() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        let to: T::AccountId = account("refundee", 1, SEED);
        setup_locked::<T>(id, amount);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
    }

    #[benchmark]
    fn release_split() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_locked::<T>(id, amount);
        let r1: T::AccountId = account("r1", 1, SEED);
        let r2: T::AccountId = account("r2", 2, SEED);
        let half: BalanceOf<T> = 500u32.into();
        let entries: BoundedVec<(T::AccountId, BalanceOf<T>), T::MaxSplitEntries> =
            BoundedVec::try_from(vec![(r1, half), (r2, half)]).unwrap();

        #[extrinsic_call]
        _(RawOrigin::Root, id, entries);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
    }

    #[benchmark]
    fn dispute() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_locked::<T>(id, amount);
        let reason: u16 = 1;
        let detail: BoundedVec<u8, T::MaxReasonLen> = BoundedVec::default();

        #[extrinsic_call]
        _(RawOrigin::Root, id, reason, detail);

        assert_eq!(LockStateOf::<T>::get(id), 1u8);
    }

    #[benchmark]
    fn apply_decision_release() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_disputed::<T>(id, amount);
        let at: BlockNumberFor<T> = 100_000u32.into();
        ExpiryOf::<T>::insert(id, at);
        ExpiringAt::<T>::try_mutate(at, |ids| -> Result<(), ()> {
            ids.try_push(id).map_err(|_| ())
        }).ok();
        let to: T::AccountId = account("receiver", 1, SEED);

        #[extrinsic_call]
        apply_decision_release_all(RawOrigin::Root, id, to);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
        assert_eq!(ExpiryOf::<T>::get(id), None);
    }

    #[benchmark]
    fn apply_decision_refund() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_disputed::<T>(id, amount);
        let at: BlockNumberFor<T> = 100_000u32.into();
        ExpiryOf::<T>::insert(id, at);
        ExpiringAt::<T>::try_mutate(at, |ids| -> Result<(), ()> {
            ids.try_push(id).map_err(|_| ())
        }).ok();
        let to: T::AccountId = account("refundee", 1, SEED);

        #[extrinsic_call]
        apply_decision_refund_all(RawOrigin::Root, id, to);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
        assert_eq!(ExpiryOf::<T>::get(id), None);
    }

    #[benchmark]
    fn apply_decision_partial() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_disputed::<T>(id, amount);
        let at: BlockNumberFor<T> = 100_000u32.into();
        ExpiryOf::<T>::insert(id, at);
        ExpiringAt::<T>::try_mutate(at, |ids| -> Result<(), ()> {
            ids.try_push(id).map_err(|_| ())
        }).ok();
        let release_to: T::AccountId = account("receiver", 1, SEED);
        let refund_to: T::AccountId = account("refundee", 2, SEED);

        #[extrinsic_call]
        apply_decision_partial_bps(RawOrigin::Root, id, release_to, refund_to, 5000u16);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
        assert_eq!(ExpiryOf::<T>::get(id), None);
    }

    #[benchmark]
    fn set_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);

        assert!(Paused::<T>::get());
    }

    #[benchmark]
    fn schedule_expiry() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_locked::<T>(id, amount);
        let at: BlockNumberFor<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, id, at);

        assert_eq!(ExpiryOf::<T>::get(id), Some(at));
    }

    #[benchmark]
    fn cancel_expiry() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_locked::<T>(id, amount);
        let at: BlockNumberFor<T> = 1000u32.into();
        ExpiryOf::<T>::insert(id, at);
        ExpiringAt::<T>::try_mutate(at, |ids| -> Result<(), ()> {
            ids.try_push(id).map_err(|_| ())
        }).ok();

        #[extrinsic_call]
        _(RawOrigin::Root, id);

        assert_eq!(ExpiryOf::<T>::get(id), None);
    }

    #[benchmark]
    fn force_release() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_disputed::<T>(id, amount);
        let to: T::AccountId = account("receiver", 1, SEED);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
    }

    #[benchmark]
    fn force_refund() {
        let id: u64 = 1;
        let amount: BalanceOf<T> = 1000u32.into();
        setup_disputed::<T>(id, amount);
        let to: T::AccountId = account("refundee", 1, SEED);

        #[extrinsic_call]
        _(RawOrigin::Root, id, to);

        assert_eq!(LockStateOf::<T>::get(id), 3u8);
    }

    #[benchmark]
    fn cleanup_closed() {
        for i in 0..5u64 {
            setup_closed::<T>(100 + i);
        }
        let ids: BoundedVec<u64, T::MaxCleanupPerCall> =
            BoundedVec::try_from(vec![100, 101, 102, 103, 104]).unwrap();
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ids);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
