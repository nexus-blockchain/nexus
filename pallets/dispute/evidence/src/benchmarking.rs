//! Benchmarks for pallet-dispute-evidence extrinsics.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::{vec, vec::Vec};
use frame_benchmarking::v2::*;
use frame_support::{traits::{ConstU32, Get}, BoundedVec};
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

fn setup_private_content<T: Config>(caller: &T::AccountId) {
    let ns = T::EvidenceNsBytes::get();
    Pallet::<T>::register_public_key(
        RawOrigin::Signed(caller.clone()).into(),
        BoundedVec::try_from(vec![0xABu8; 32]).unwrap(),
        pallet_crypto_common::KeyType::Ed25519,
    ).unwrap();
    let cid: BoundedVec<u8, T::MaxCidLen> =
        BoundedVec::try_from(Vec::from("enc-bafkreiaaaaaaaaaa000001".as_bytes())).unwrap();
    let encrypted_keys: crate::private_content::EncryptedKeyBundles<T> =
        BoundedVec::try_from(vec![
            (caller.clone(), BoundedVec::try_from(vec![0xFFu8; 32]).unwrap()),
        ]).unwrap();
    Pallet::<T>::store_private_content(
        RawOrigin::Signed(caller.clone()).into(),
        ns, 1u64, cid,
        H256::repeat_byte(0x11),
        pallet_crypto_common::EncryptionMethod::Aes256Gcm,
        pallet_crypto_common::AccessPolicy::OwnerOnly,
        encrypted_keys,
    ).unwrap();
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn commit() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        let imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg> =
            BoundedVec::try_from(vec![make_cid_bounded::<T::MaxCidLen>(1), make_cid_bounded::<T::MaxCidLen>(2)]).unwrap();
        let vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid> = BoundedVec::default();
        let docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc> = BoundedVec::default();
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
        let parent_id = setup_evidence::<T>(&caller);
        let ns = T::EvidenceNsBytes::get();
        for seed in 1..=3u8 {
            let imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg> =
                BoundedVec::try_from(vec![make_cid_bounded::<T::MaxCidLen>(100 + seed)]).unwrap();
            let vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid> = BoundedVec::default();
            let docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc> = BoundedVec::default();
            Pallet::<T>::append_evidence(
                RawOrigin::Signed(caller.clone()).into(),
                parent_id, imgs, vids, docs, None,
            ).unwrap();
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), parent_id, None);
    }

    #[benchmark]
    fn unseal_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let parent_id = setup_evidence::<T>(&caller);
        let ns = T::EvidenceNsBytes::get();
        for seed in 1..=3u8 {
            let imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg> =
                BoundedVec::try_from(vec![make_cid_bounded::<T::MaxCidLen>(110 + seed)]).unwrap();
            let vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid> = BoundedVec::default();
            let docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc> = BoundedVec::default();
            Pallet::<T>::append_evidence(
                RawOrigin::Signed(caller.clone()).into(),
                parent_id, imgs, vids, docs, None,
            ).unwrap();
        }
        Pallet::<T>::seal_evidence(RawOrigin::Signed(caller.clone()).into(), parent_id, None).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), parent_id, None);
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
    fn append_evidence() {
        let caller: T::AccountId = whitelisted_caller();
        let parent_id = setup_evidence::<T>(&caller);
        let imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg> =
            BoundedVec::try_from(vec![make_cid_bounded::<T::MaxCidLen>(50)]).unwrap();
        let vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid> = BoundedVec::default();
        let docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc> = BoundedVec::default();
        let memo: Option<BoundedVec<u8, T::MaxMemoLen>> = None;
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), parent_id, imgs, vids, docs, memo);
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

    #[benchmark]
    fn store_private_content() {
        let caller: T::AccountId = whitelisted_caller();
        let ns = T::EvidenceNsBytes::get();
        Pallet::<T>::register_public_key(
            RawOrigin::Signed(caller.clone()).into(),
            BoundedVec::try_from(vec![0xABu8; 32]).unwrap(),
            pallet_crypto_common::KeyType::Ed25519,
        ).unwrap();
        let cid: BoundedVec<u8, T::MaxCidLen> =
            BoundedVec::try_from(Vec::from("enc-bafkreiaaaaaaaaaa000001".as_bytes())).unwrap();
        let encrypted_keys: crate::private_content::EncryptedKeyBundles<T> =
            BoundedVec::try_from(vec![
                (caller.clone(), BoundedVec::try_from(vec![0xFFu8; 32]).unwrap()),
            ]).unwrap();
        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller), ns, 1u64, cid,
            H256::repeat_byte(0x11),
            pallet_crypto_common::EncryptionMethod::Aes256Gcm,
            pallet_crypto_common::AccessPolicy::OwnerOnly,
            encrypted_keys,
        );
    }

    #[benchmark]
    fn grant_access() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let grantee: T::AccountId = frame_benchmarking::account("grantee", 0, 1);
        Pallet::<T>::register_public_key(
            RawOrigin::Signed(grantee.clone()).into(),
            BoundedVec::try_from(vec![0xCDu8; 32]).unwrap(),
            pallet_crypto_common::KeyType::Ed25519,
        ).unwrap();
        let enc_key: BoundedVec<u8, ConstU32<512>> =
            BoundedVec::try_from(vec![0xEEu8; 32]).unwrap();
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), content_id, grantee, enc_key);
    }

    #[benchmark]
    fn revoke_access() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        let target: T::AccountId = frame_benchmarking::account("target", 0, 2);
        Pallet::<T>::register_public_key(
            RawOrigin::Signed(target.clone()).into(),
            BoundedVec::try_from(vec![0xCDu8; 32]).unwrap(),
            pallet_crypto_common::KeyType::Ed25519,
        ).unwrap();
        Pallet::<T>::grant_access(
            RawOrigin::Signed(caller.clone()).into(),
            content_id, target.clone(),
            BoundedVec::try_from(vec![0xEEu8; 32]).unwrap(),
        ).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), content_id, target);
    }

    #[benchmark]
    fn rotate_content_keys() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        let new_keys: BoundedVec<
            (T::AccountId, BoundedVec<u8, ConstU32<512>>),
            T::MaxAuthorizedUsers,
        > = BoundedVec::try_from(vec![
            (caller.clone(), BoundedVec::try_from(vec![0xAAu8; 32]).unwrap()),
        ]).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), content_id, H256::repeat_byte(0x22), new_keys);
    }

    #[benchmark]
    fn request_access() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        let requester: T::AccountId = frame_benchmarking::account("requester", 0, 3);
        Pallet::<T>::register_public_key(
            RawOrigin::Signed(requester.clone()).into(),
            BoundedVec::try_from(vec![0xBBu8; 32]).unwrap(),
            pallet_crypto_common::KeyType::Ed25519,
        ).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(requester), content_id);
    }

    #[benchmark]
    fn update_access_policy() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        let new_policy: crate::private_content::AccessPolicy<T> =
            pallet_crypto_common::AccessPolicy::OwnerOnly;
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), content_id, new_policy);
    }

    #[benchmark]
    fn delete_private_content() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), content_id);
    }

    #[benchmark]
    fn revoke_public_key() {
        let caller: T::AccountId = whitelisted_caller();
        Pallet::<T>::register_public_key(
            RawOrigin::Signed(caller.clone()).into(),
            BoundedVec::try_from(vec![0xABu8; 32]).unwrap(),
            pallet_crypto_common::KeyType::Ed25519,
        ).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn cancel_access_request() {
        let caller: T::AccountId = whitelisted_caller();
        setup_private_content::<T>(&caller);
        let content_id = NextPrivateContentId::<T>::get().saturating_sub(1);
        let requester: T::AccountId = frame_benchmarking::account("requester", 0, 4);
        Pallet::<T>::register_public_key(
            RawOrigin::Signed(requester.clone()).into(),
            BoundedVec::try_from(vec![0xBBu8; 32]).unwrap(),
            pallet_crypto_common::KeyType::Ed25519,
        ).unwrap();
        Pallet::<T>::request_access(
            RawOrigin::Signed(requester.clone()).into(),
            content_id,
        ).unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(requester), content_id);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
