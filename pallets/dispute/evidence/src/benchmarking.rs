//! Benchmarks for pallet-evidence extrinsics.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;
use sp_core::H256;

fn make_memo<T: Config>() -> Option<BoundedVec<u8, T::MaxMemoLen>> {
    Some(BoundedVec::try_from(Vec::from("bafyaaaaaaaaaa000001".as_bytes())).unwrap())
}

fn make_cid_bounded<L: Get<u32>>(seed: u8) -> BoundedVec<u8, L> {
    let mut v = Vec::from("bafkreiaaaaaaaaaa000000".as_bytes());
    if let Some(last) = v.last_mut() { *last = seed; }
    BoundedVec::try_from(v).unwrap()
}

fn setup_evidence<T: Config>(caller: &T::AccountId) -> u64 {
    let ns = T::EvidenceNsBytes::get();
    let hash = H256::repeat_byte(99);
    Pallet::<T>::commit_hash(
        RawOrigin::Signed(caller.clone()).into(), ns, 1, hash, make_memo::<T>(),
    ).unwrap();
    NextEvidenceId::<T>::get().saturating_sub(1)
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn commit() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        let imgs: Vec<BoundedVec<u8, T::MaxCidLen>> = vec![make_cid_bounded::<T::MaxCidLen>(1), make_cid_bounded::<T::MaxCidLen>(2)];
        let vids: Vec<BoundedVec<u8, T::MaxCidLen>> = vec![];
        let docs: Vec<BoundedVec<u8, T::MaxCidLen>> = vec![];
        let memo: Option<BoundedVec<u8, T::MaxMemoLen>> = None;
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ns, 1u8, 1u64, imgs, vids, docs, memo);
    }

    #[benchmark]
    fn commit_hash() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ns, 1u64, H256::repeat_byte(7), make_memo::<T>());
    }

    #[benchmark]
    fn link() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1u8, 100u64, id);
    }

    #[benchmark]
    fn link_by_ns() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        let id = setup_evidence::<T>(&caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ns, 100u64, id);
    }

    #[benchmark]
    fn unlink() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        Pallet::<T>::link(RawOrigin::Signed(caller.clone()).into(), 1, 100, id).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1u8, 100u64, id);
    }

    #[benchmark]
    fn unlink_by_ns() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        let id = setup_evidence::<T>(&caller);
        Pallet::<T>::link_by_ns(RawOrigin::Signed(caller.clone()).into(), ns, 100, id).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ns, 100u64, id);
    }

    #[benchmark]
    fn seal_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, None);
    }

    #[benchmark]
    fn unseal_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        Pallet::<T>::seal_evidence(RawOrigin::Signed(caller.clone()).into(), id, None).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, None);
    }

    #[benchmark]
    fn withdraw_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id);
    }

    #[benchmark]
    fn reveal_commitment() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        let cid: BoundedVec<u8, T::MaxCidLen> = BoundedVec::try_from(Vec::from("bafkreiaaaaaaaaaa000001".as_bytes())).unwrap();
        let salt: BoundedVec<u8, T::MaxMemoLen> = BoundedVec::try_from(Vec::from("benchsalt".as_bytes())).unwrap();
        let version = 1u32;
        let evidence_commit = Pallet::<T>::compute_evidence_commitment(&ns, 1, cid.as_slice(), salt.as_slice(), version);
        Pallet::<T>::commit_hash(RawOrigin::Signed(caller.clone()).into(), ns, 1, evidence_commit, make_memo::<T>()).unwrap();
        let id = NextEvidenceId::<T>::get().saturating_sub(1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, cid, salt, version);
    }

    #[benchmark]
    fn register_public_key() {
        let caller: T::AccountId = whitelisted_caller();
        let key: BoundedVec<u8, T::MaxKeyLen> = BoundedVec::try_from(vec![0xABu8; 32]).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), key, pallet_crypto_common::KeyType::Ed25519);
    }

    #[benchmark]
    fn force_remove_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        #[extrinsic_call]
        _(RawOrigin::Root, id, None);
    }

    #[benchmark]
    fn force_archive_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let id = setup_evidence::<T>(&caller);
        #[extrinsic_call]
        _(RawOrigin::Root, id, None);
    }

    #[benchmark]
    fn commit_v2() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        let content_cid: BoundedVec<u8, T::MaxContentCidLen> = BoundedVec::try_from(Vec::from("bafkreiaaaaaaaaaa000001".as_bytes())).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ns, 1u8, 1u64, content_cid, crate::pallet::ContentType::Document);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
