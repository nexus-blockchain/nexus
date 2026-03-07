use crate::{mock::*, pallet::*};
use frame_support::{assert_ok, assert_noop, BoundedVec};
use frame_support::traits::{Currency, ReservableCurrency, Hooks};

const NS: [u8; 8] = *b"evidence";

fn make_cid(index: u32) -> BoundedVec<u8, <Test as Config>::MaxCidLen> {
    let s = alloc::format!("bafkreiaaaaaaaaaa{:06}", index);
    BoundedVec::truncate_from(s.into_bytes())
}

fn imgs_n(indices: &[u32]) -> BoundedVec<BoundedVec<u8, <Test as Config>::MaxCidLen>, <Test as Config>::MaxImg> {
    let v: Vec<_> = indices.iter().map(|i| make_cid(*i)).collect();
    BoundedVec::truncate_from(v)
}

fn empty_vids() -> BoundedVec<BoundedVec<u8, <Test as Config>::MaxCidLen>, <Test as Config>::MaxVid> {
    BoundedVec::default()
}

fn empty_docs() -> BoundedVec<BoundedVec<u8, <Test as Config>::MaxCidLen>, <Test as Config>::MaxDoc> {
    BoundedVec::default()
}

fn vids_n(indices: &[u32]) -> BoundedVec<BoundedVec<u8, <Test as Config>::MaxCidLen>, <Test as Config>::MaxVid> {
    let v: Vec<_> = indices.iter().map(|i| make_cid(*i)).collect();
    BoundedVec::truncate_from(v)
}

fn docs_n(indices: &[u32]) -> BoundedVec<BoundedVec<u8, <Test as Config>::MaxCidLen>, <Test as Config>::MaxDoc> {
    let v: Vec<_> = indices.iter().map(|i| make_cid(*i)).collect();
    BoundedVec::truncate_from(v)
}

// ==================== commit (call_index 0) ====================

#[test]
fn commit_works() {
    new_test_ext().execute_with(|| {
        let bal_before = Balances::free_balance(1);
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        let ev = Evidences::<Test>::get(0).expect("evidence should exist");
        assert_eq!(ev.id, 0);
        assert_eq!(ev.domain, 1);
        assert_eq!(ev.target_id, 100);
        assert_eq!(ev.owner, 1);
        assert_eq!(NextEvidenceId::<Test>::get(), 1);
        // Deposit reserved
        assert_eq!(Balances::free_balance(1), bal_before - 10);
        assert_eq!(Balances::reserved_balance(1), 10);
        assert!(EvidenceDeposits::<Test>::get(0).is_some());
    });
}

#[test]
fn commit_not_authorized() {
    new_test_ext().execute_with(|| {
        set_authorized(false);
        assert_noop!(
            EvidencePallet::commit(
                RuntimeOrigin::signed(1), NS, 1, 100,
                imgs_n(&[1]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn commit_rejects_empty_media() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EvidencePallet::commit(
                RuntimeOrigin::signed(1), NS, 1, 100,
                BoundedVec::default(), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::InvalidCidFormat
        );
    });
}

#[test]
fn commit_increments_target_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert_eq!(EvidenceCountByTarget::<Test>::get((1u8, 100u64)), 2);
    });
}

// P1-#8: Dynamic content_type
#[test]
fn commit_sets_content_type_correctly() {
    new_test_ext().execute_with(|| {
        // Only images → Image
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_eq!(Evidences::<Test>::get(0).unwrap().content_type, ContentType::Image);
        // Only videos → Video
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 101, BoundedVec::default(), vids_n(&[2]), empty_docs(), None));
        assert_eq!(Evidences::<Test>::get(1).unwrap().content_type, ContentType::Video);
        // Only docs → Document
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 102, BoundedVec::default(), empty_vids(), docs_n(&[3]), None));
        assert_eq!(Evidences::<Test>::get(2).unwrap().content_type, ContentType::Document);
        // Mixed → Mixed
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 103, imgs_n(&[4]), vids_n(&[5]), empty_docs(), None));
        assert_eq!(Evidences::<Test>::get(3).unwrap().content_type, ContentType::Mixed);
    });
}

// ==================== commit_hash (call_index 1) ====================

#[test]
fn commit_hash_works() {
    new_test_ext().execute_with(|| {
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000099".to_vec());
        assert_ok!(EvidencePallet::commit_hash(
            RuntimeOrigin::signed(1), NS, 200,
            sp_core::H256::repeat_byte(0x42), Some(memo),
        ));
        let ev = Evidences::<Test>::get(0).expect("evidence should exist");
        assert_eq!(ev.target_id, 200);
        assert!(ev.commit.is_some());
        assert!(EvidenceDeposits::<Test>::get(0).is_some());
    });
}

// ==================== link / unlink (call_index 2/4) ====================

#[test]
fn link_and_unlink_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0));
        assert!(EvidenceByTarget::<Test>::get((2u8, 200u64), 0).is_some());
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 1);

        assert_ok!(EvidencePallet::unlink(RuntimeOrigin::signed(1), 2, 200, 0));
        assert!(EvidenceByTarget::<Test>::get((2u8, 200u64), 0).is_none());
    });
}

#[test]
fn link_not_authorized() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        set_authorized(false);
        assert_noop!(
            EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0),
            Error::<Test>::NotAuthorized
        );
    });
}

// P0-#4: link count limit
#[test]
fn link_rejects_when_max_links_exceeded() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        // MaxLinksPerEvidence = 50
        for i in 0..50u64 {
            assert_ok!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, i, 0));
        }
        assert_noop!(
            EvidencePallet::link(RuntimeOrigin::signed(1), 2, 999, 0),
            Error::<Test>::TooManyLinks
        );
    });
}

// ==================== link_by_ns / unlink_by_ns (call_index 3/5) ====================

#[test]
fn link_by_ns_and_unlink_by_ns_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::link_by_ns(RuntimeOrigin::signed(1), NS, 200, 0));
        assert!(EvidenceByNs::<Test>::get((NS, 200u64), 0).is_some());
        assert_ok!(EvidencePallet::unlink_by_ns(RuntimeOrigin::signed(1), NS, 200, 0));
        assert!(EvidenceByNs::<Test>::get((NS, 200u64), 0).is_none());
    });
}

// ==================== register_public_key (call_index 6) ====================

#[test]
fn register_public_key_works() {
    new_test_ext().execute_with(|| {
        let key: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
            BoundedVec::truncate_from([0xABu8; 32].to_vec());
        assert_ok!(EvidencePallet::register_public_key(
            RuntimeOrigin::signed(1), key, pallet_crypto_common::KeyType::Ed25519,
        ));
        assert!(UserPublicKeys::<Test>::get(1).is_some());
    });
}

// ==================== store_private_content (call_index 7) ====================

fn register_key(account: u64) {
    let key: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
        BoundedVec::truncate_from([0xABu8; 32].to_vec());
    assert_ok!(EvidencePallet::register_public_key(
        RuntimeOrigin::signed(account), key, pallet_crypto_common::KeyType::Ed25519,
    ));
}

fn store_test_private_content() -> u64 {
    register_key(1);
    let cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
        BoundedVec::truncate_from(b"enc-bafybeigdyrzt5sfp7udm7hu".to_vec());
    let encrypted_keys: BoundedVec<
        (u64, BoundedVec<u8, <Test as Config>::MaxKeyLen>),
        <Test as Config>::MaxAuthorizedUsers,
    > = BoundedVec::truncate_from(vec![
        (1u64, BoundedVec::truncate_from(vec![0xFFu8; 32])),
    ]);
    let content_id = NextPrivateContentId::<Test>::mutate(|id| { let current = *id; *id += 1; current });
    let content = pallet_crypto_common::PrivateContent::<
        <Test as frame_system::Config>::AccountId, u64,
        <Test as Config>::MaxCidLen, <Test as Config>::MaxAuthorizedUsers, <Test as Config>::MaxKeyLen,
    > {
        id: content_id, ns: NS, subject_id: 500,
        cid: cid.clone(),
        content_hash: sp_core::H256::repeat_byte(0x11),
        encryption_method: pallet_crypto_common::EncryptionMethod::Aes256Gcm,
        creator: 1u64,
        access_policy: pallet_crypto_common::AccessPolicy::OwnerOnly,
        encrypted_keys,
        status: pallet_crypto_common::ContentStatus::Active,
        created_at: 1u64, updated_at: 1u64,
    };
    PrivateContents::<Test>::insert(content_id, &content);
    PrivateContentByCid::<Test>::insert(&cid, content_id);
    PrivateContentBySubject::<Test>::insert((NS, 500u64), content_id, ());
    content_id
}

// ==================== store_private_content extrinsic tests (call_index 7) ====================

fn make_enc_cid() -> BoundedVec<u8, <Test as Config>::MaxCidLen> {
    BoundedVec::truncate_from(b"enc-bafkreiaaaaaaaaaa000099".to_vec())
}

fn make_encrypted_keys(creator: u64) -> BoundedVec<
    (u64, BoundedVec<u8, <Test as Config>::MaxKeyLen>),
    <Test as Config>::MaxAuthorizedUsers,
> {
    BoundedVec::truncate_from(vec![
        (creator, BoundedVec::truncate_from(vec![0xFFu8; 32])),
    ])
}

#[test]
fn store_private_content_works() {
    new_test_ext().execute_with(|| {
        register_key(1);
        let cid = make_enc_cid();
        let keys = make_encrypted_keys(1);
        assert_ok!(EvidencePallet::store_private_content(
            RuntimeOrigin::signed(1), NS, 500, cid.clone(),
            sp_core::H256::repeat_byte(0x11),
            pallet_crypto_common::EncryptionMethod::Aes256Gcm,
            pallet_crypto_common::AccessPolicy::OwnerOnly,
            keys,
        ));
        let content = PrivateContents::<Test>::get(0).expect("content stored");
        assert_eq!(content.creator, 1u64);
        assert_eq!(content.ns, NS);
        assert_eq!(content.subject_id, 500);
        assert_eq!(content.cid, cid);
        assert!(PrivateContentByCid::<Test>::get(&cid).is_some());
        assert!(PrivateContentBySubject::<Test>::contains_key((NS, 500u64), 0u64));
        assert!(PrivateContentDeposits::<Test>::get(0).is_some());
    });
}

#[test]
fn store_private_content_rejects_not_authorized() {
    new_test_ext().execute_with(|| {
        set_authorized(false);
        register_key(1);
        assert_noop!(
            EvidencePallet::store_private_content(
                RuntimeOrigin::signed(1), NS, 500, make_enc_cid(),
                sp_core::H256::repeat_byte(0x11),
                pallet_crypto_common::EncryptionMethod::Aes256Gcm,
                pallet_crypto_common::AccessPolicy::OwnerOnly,
                make_encrypted_keys(1),
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn store_private_content_rejects_insufficient_balance() {
    new_test_ext().execute_with(|| {
        // Account 99 has zero balance (not funded in genesis)
        register_key(99);
        assert_noop!(
            EvidencePallet::store_private_content(
                RuntimeOrigin::signed(99), NS, 500, make_enc_cid(),
                sp_core::H256::repeat_byte(0x11),
                pallet_crypto_common::EncryptionMethod::Aes256Gcm,
                pallet_crypto_common::AccessPolicy::OwnerOnly,
                make_encrypted_keys(99),
            ),
            Error::<Test>::InsufficientDeposit
        );
    });
}

#[test]
fn store_private_content_rejects_non_encrypted_cid() {
    new_test_ext().execute_with(|| {
        register_key(1);
        let plain_cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000001".to_vec());
        assert_noop!(
            EvidencePallet::store_private_content(
                RuntimeOrigin::signed(1), NS, 500, plain_cid,
                sp_core::H256::repeat_byte(0x11),
                pallet_crypto_common::EncryptionMethod::Aes256Gcm,
                pallet_crypto_common::AccessPolicy::OwnerOnly,
                make_encrypted_keys(1),
            ),
            Error::<Test>::InvalidCidFormat
        );
    });
}

#[test]
fn store_private_content_rejects_duplicate_cid() {
    new_test_ext().execute_with(|| {
        register_key(1);
        let cid = make_enc_cid();
        let keys = make_encrypted_keys(1);
        assert_ok!(EvidencePallet::store_private_content(
            RuntimeOrigin::signed(1), NS, 500, cid.clone(),
            sp_core::H256::repeat_byte(0x11),
            pallet_crypto_common::EncryptionMethod::Aes256Gcm,
            pallet_crypto_common::AccessPolicy::OwnerOnly,
            keys.clone(),
        ));
        assert_noop!(
            EvidencePallet::store_private_content(
                RuntimeOrigin::signed(1), NS, 501, cid,
                sp_core::H256::repeat_byte(0x22),
                pallet_crypto_common::EncryptionMethod::Aes256Gcm,
                pallet_crypto_common::AccessPolicy::OwnerOnly,
                keys,
            ),
            Error::<Test>::CidAlreadyExists
        );
    });
}

#[test]
fn store_private_content_rejects_missing_creator_key() {
    new_test_ext().execute_with(|| {
        register_key(1);
        register_key(2);
        // encrypted_keys contains account 2 but not account 1 (the caller)
        let wrong_keys: BoundedVec<
            (u64, BoundedVec<u8, <Test as Config>::MaxKeyLen>),
            <Test as Config>::MaxAuthorizedUsers,
        > = BoundedVec::truncate_from(vec![
            (2u64, BoundedVec::truncate_from(vec![0xFFu8; 32])),
        ]);
        assert_noop!(
            EvidencePallet::store_private_content(
                RuntimeOrigin::signed(1), NS, 500, make_enc_cid(),
                sp_core::H256::repeat_byte(0x11),
                pallet_crypto_common::EncryptionMethod::Aes256Gcm,
                pallet_crypto_common::AccessPolicy::OwnerOnly,
                wrong_keys,
            ),
            Error::<Test>::InvalidEncryptedKey
        );
    });
}

#[test]
fn store_private_content_rejects_unregistered_user_key() {
    new_test_ext().execute_with(|| {
        register_key(1);
        // include account 3 who has no registered public key
        let keys: BoundedVec<
            (u64, BoundedVec<u8, <Test as Config>::MaxKeyLen>),
            <Test as Config>::MaxAuthorizedUsers,
        > = BoundedVec::truncate_from(vec![
            (1u64, BoundedVec::truncate_from(vec![0xFFu8; 32])),
            (3u64, BoundedVec::truncate_from(vec![0xAAu8; 32])),
        ]);
        assert_noop!(
            EvidencePallet::store_private_content(
                RuntimeOrigin::signed(1), NS, 500, make_enc_cid(),
                sp_core::H256::repeat_byte(0x11),
                pallet_crypto_common::EncryptionMethod::Aes256Gcm,
                pallet_crypto_common::AccessPolicy::OwnerOnly,
                keys,
            ),
            Error::<Test>::PublicKeyNotRegistered
        );
    });
}

// ==================== request_access (call_index 13) ====================

#[test]
fn request_access_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_some());
        assert_eq!(AccessRequestCount::<Test>::get(content_id), 1);
    });
}

#[test]
fn request_access_rejects_without_public_key() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert_noop!(
            EvidencePallet::request_access(RuntimeOrigin::signed(3), content_id),
            Error::<Test>::PublicKeyNotRegistered
        );
    });
}

#[test]
fn request_access_rejects_nonexistent_content() {
    new_test_ext().execute_with(|| {
        register_key(2);
        assert_noop!(
            EvidencePallet::request_access(RuntimeOrigin::signed(2), 999),
            Error::<Test>::PrivateContentNotFound
        );
    });
}

#[test]
fn request_access_rejects_self() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert_noop!(
            EvidencePallet::request_access(RuntimeOrigin::signed(1), content_id),
            Error::<Test>::SelfAccessRequest
        );
    });
}

#[test]
fn request_access_rejects_already_authorized() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_ok!(EvidencePallet::grant_access(RuntimeOrigin::signed(1), content_id, 2, enc_key));
        assert_noop!(
            EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id),
            Error::<Test>::AlreadyAuthorized
        );
    });
}

#[test]
fn request_access_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert_noop!(
            EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id),
            Error::<Test>::AlreadyRequested
        );
    });
}

// P1-#10: max pending requests
#[test]
fn request_access_rejects_too_many_requests() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        // MaxPendingRequestsPerContent = 20, fill up
        for i in 10..30u64 {
            let key: BoundedVec<u8, <Test as Config>::MaxKeyLen> = BoundedVec::truncate_from([0xABu8; 32].to_vec());
            assert_ok!(EvidencePallet::register_public_key(RuntimeOrigin::signed(i), key, pallet_crypto_common::KeyType::Ed25519));
            assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(i), content_id));
        }
        register_key(99);
        assert_noop!(
            EvidencePallet::request_access(RuntimeOrigin::signed(99), content_id),
            Error::<Test>::TooManyAccessRequests
        );
    });
}

#[test]
fn grant_access_auto_removes_request() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert_eq!(AccessRequestCount::<Test>::get(content_id), 1);

        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_ok!(EvidencePallet::grant_access(RuntimeOrigin::signed(1), content_id, 2, enc_key));
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_none());
        assert_eq!(AccessRequestCount::<Test>::get(content_id), 0);
    });
}

// ==================== cancel_access_request (call_index 23) ====================

#[test]
fn cancel_access_request_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert_eq!(AccessRequestCount::<Test>::get(content_id), 1);
        assert_ok!(EvidencePallet::cancel_access_request(RuntimeOrigin::signed(2), content_id));
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_none());
        assert_eq!(AccessRequestCount::<Test>::get(content_id), 0);
    });
}

// ==================== revoke_public_key (call_index 22) ====================

#[test]
fn revoke_public_key_works() {
    new_test_ext().execute_with(|| {
        register_key(1);
        assert!(UserPublicKeys::<Test>::get(1).is_some());
        assert_ok!(EvidencePallet::revoke_public_key(RuntimeOrigin::signed(1)));
        assert!(UserPublicKeys::<Test>::get(1).is_none());
    });
}

#[test]
fn revoke_public_key_rejects_unregistered() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EvidencePallet::revoke_public_key(RuntimeOrigin::signed(99)),
            Error::<Test>::PublicKeyNotRegistered
        );
    });
}

// ==================== update_access_policy (call_index 14) ====================

#[test]
fn update_access_policy_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        let new_policy = pallet_crypto_common::AccessPolicy::SharedWith(
            BoundedVec::truncate_from(vec![2u64, 3u64]),
        );
        assert_ok!(EvidencePallet::update_access_policy(RuntimeOrigin::signed(1), content_id, new_policy));
        let content = PrivateContents::<Test>::get(content_id).unwrap();
        match &content.access_policy {
            pallet_crypto_common::AccessPolicy::SharedWith(users) => assert_eq!(users.len(), 2),
            _ => panic!("expected SharedWith policy"),
        }
    });
}

#[test]
fn update_access_policy_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert_noop!(
            EvidencePallet::update_access_policy(
                RuntimeOrigin::signed(2), content_id,
                pallet_crypto_common::AccessPolicy::GovernanceControlled,
            ),
            Error::<Test>::AccessDenied
        );
    });
}

// ==================== PrivateContentProvider impl ====================

#[test]
fn private_content_provider_can_access() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert!(<EvidencePallet as pallet_crypto_common::PrivateContentProvider<u64>>::can_access(content_id, &1));
        assert!(!<EvidencePallet as pallet_crypto_common::PrivateContentProvider<u64>>::can_access(content_id, &2));
    });
}

#[test]
fn get_decryption_info_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        let info = EvidencePallet::get_decryption_info(content_id, &1);
        assert!(info.is_some());
        let (cid, hash, method, key) = info.unwrap();
        assert_eq!(cid, b"enc-bafybeigdyrzt5sfp7udm7hu".to_vec());
        assert_eq!(hash, sp_core::H256::repeat_byte(0x11));
        assert_eq!(method, pallet_crypto_common::EncryptionMethod::Aes256Gcm);
        assert_eq!(key, vec![0xFFu8; 32]);
    });
}

// ==================== delete_private_content (call_index 20) ====================

#[test]
fn delete_private_content_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert!(PrivateContents::<Test>::get(content_id).is_some());
        assert_ok!(EvidencePallet::delete_private_content(RuntimeOrigin::signed(1), content_id));
        assert!(PrivateContents::<Test>::get(content_id).is_none());
    });
}

#[test]
fn delete_private_content_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert_noop!(
            EvidencePallet::delete_private_content(RuntimeOrigin::signed(2), content_id),
            Error::<Test>::AccessDenied
        );
    });
}

// ==================== rotate_content_keys (call_index 10) ====================

#[test]
fn rotate_content_keys_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        let new_keys: BoundedVec<(u64, BoundedVec<u8, frame_support::traits::ConstU32<512>>), <Test as Config>::MaxAuthorizedUsers> =
            BoundedVec::truncate_from(vec![
                (1u64, BoundedVec::truncate_from(vec![0xAAu8; 32])),
            ]);
        assert_ok!(EvidencePallet::rotate_content_keys(
            RuntimeOrigin::signed(1), content_id,
            sp_core::H256::repeat_byte(0x22), new_keys,
        ));
        let content = PrivateContents::<Test>::get(content_id).unwrap();
        assert_eq!(content.content_hash, sp_core::H256::repeat_byte(0x22));
    });
}

// ==================== seal/unseal (call_index 16/17) ====================

// P0-#1: SealAuthorizer
#[test]
fn seal_evidence_requires_seal_authorization() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        set_seal_authorized(false);
        assert_noop!(
            EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn seal_and_unseal_evidence_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Sealed);

        assert_ok!(EvidencePallet::unseal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Active);
    });
}

// ==================== force_remove_evidence (call_index 18) ====================

#[test]
fn force_remove_evidence_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        let bal_before = Balances::free_balance(1);
        assert_ok!(EvidencePallet::force_remove_evidence(RuntimeOrigin::root(), 0, None));
        assert!(Evidences::<Test>::get(0).is_none());
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Removed);
        // Deposit slashed (not refunded)
        assert!(Balances::free_balance(1) <= bal_before);
    });
}

// ==================== withdraw_evidence (call_index 19) ====================

#[test]
fn withdraw_evidence_refunds_deposit_and_cleans_indexes() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        let bal_after_commit = Balances::free_balance(1);
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        // P1-#9: indexes cleaned
        assert!(EvidenceByTarget::<Test>::get((1u8, 100u64), 0).is_none());
        assert_eq!(EvidenceCountByTarget::<Test>::get((1u8, 100u64)), 0);
        // Deposit refunded
        assert_eq!(Balances::free_balance(1), bal_after_commit + 10);
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Withdrawn);
    });
}

#[test]
fn withdraw_rejects_sealed_evidence() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_noop!(
            EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0),
            Error::<Test>::EvidenceSealed
        );
    });
}

// ==================== force_archive_evidence (call_index 21) ====================

#[test]
fn force_archive_evidence_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        let bal_before = Balances::free_balance(1);
        assert_ok!(EvidencePallet::force_archive_evidence(RuntimeOrigin::root(), 0, None));
        assert!(Evidences::<Test>::get(0).is_none());
        assert!(ArchivedEvidences::<Test>::get(0).is_some());
        // Deposit refunded (non-punitive)
        assert_eq!(Balances::free_balance(1), bal_before + 10);
    });
}

// ==================== append_evidence (call_index 11) ====================

// P0-#5: owner check
#[test]
fn append_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_noop!(
            EvidencePallet::append_evidence(RuntimeOrigin::signed(2), 0, imgs_n(&[2]), empty_vids(), empty_docs(), None),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn append_works_for_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::append_evidence(RuntimeOrigin::signed(1), 0, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert_eq!(NextEvidenceId::<Test>::get(), 2);
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 1);
    });
}

#[test]
fn append_rejects_sealed_parent() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_noop!(
            EvidencePallet::append_evidence(RuntimeOrigin::signed(1), 0, imgs_n(&[2]), empty_vids(), empty_docs(), None),
            Error::<Test>::EvidenceSealed
        );
    });
}

// ==================== reveal_commitment (call_index 15) ====================

#[test]
fn reveal_commitment_works_and_cleans_commit_index() {
    new_test_ext().execute_with(|| {
        let cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000077".to_vec());
        let salt: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"salt123".to_vec());
        let version = 1u32;
        let commit_hash = EvidencePallet::compute_evidence_commitment(&NS, 200, cid.as_slice(), salt.as_slice(), version);
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000088".to_vec());
        assert_ok!(EvidencePallet::commit_hash(RuntimeOrigin::signed(1), NS, 200, commit_hash, Some(memo)));
        assert!(CommitIndex::<Test>::get(commit_hash).is_some());
        assert_ok!(EvidencePallet::reveal_commitment(RuntimeOrigin::signed(1), 0, cid, salt, version));
        assert!(CommitIndex::<Test>::get(commit_hash).is_none());
    });
}

// ==================== archive ====================

#[test]
fn archive_skips_sealed_evidence() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 101, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        System::set_block_number(100);
        let archived = EvidencePallet::archive_old_evidences(10);
        assert!(Evidences::<Test>::get(0).is_some(), "sealed evidence must not be archived");
        assert!(ArchivedEvidences::<Test>::get(1).is_some());
        assert_eq!(archived, 1);
    });
}

#[test]
fn archive_cleans_commit_index() {
    new_test_ext().execute_with(|| {
        let commit_hash = sp_core::H256::repeat_byte(0x55);
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000066".to_vec());
        assert_ok!(EvidencePallet::commit_hash(RuntimeOrigin::signed(1), NS, 300, commit_hash, Some(memo)));
        assert!(CommitIndex::<Test>::get(commit_hash).is_some());
        System::set_block_number(100);
        assert_eq!(EvidencePallet::archive_old_evidences(10), 1);
        assert!(CommitIndex::<Test>::get(commit_hash).is_none());
    });
}

// ==================== link rejects sealed/withdrawn ====================

#[test]
fn link_rejects_sealed_evidence() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_noop!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0), Error::<Test>::EvidenceSealed);
    });
}

#[test]
fn link_rejects_withdrawn_evidence() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        // P0修复后 withdraw 会删除 Evidences 记录，link 返回 NotFound
        assert_noop!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0), Error::<Test>::NotFound);
    });
}

// ==================== Rate limiting ====================

#[test]
fn commit_respects_window_limit() {
    new_test_ext().execute_with(|| {
        for i in 0..50u32 {
            assert_ok!(EvidencePallet::commit(
                RuntimeOrigin::signed(1), NS, 1, i as u64,
                imgs_n(&[200 + i]), empty_vids(), empty_docs(), None,
            ));
        }
        assert_noop!(
            EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 999, imgs_n(&[999]), empty_vids(), empty_docs(), None),
            Error::<Test>::RateLimited
        );
    });
}

// ==================== Deposit mechanism ====================

#[test]
fn commit_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        // Account 99 has no balance
        assert_noop!(
            EvidencePallet::commit(RuntimeOrigin::signed(99), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None),
            Error::<Test>::InsufficientDeposit
        );
    });
}

// ==================== M1: revoke_access rejects non-existent user ====================

#[test]
fn revoke_access_rejects_non_authorized_user() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert_noop!(
            EvidencePallet::revoke_access(RuntimeOrigin::signed(1), content_id, 5),
            Error::<Test>::NotFound
        );
    });
}

// ==================== Evidence ID increments ====================

#[test]
fn evidence_id_increments() {
    new_test_ext().execute_with(|| {
        for i in 0..3u32 {
            assert_ok!(EvidencePallet::commit(
                RuntimeOrigin::signed(1), NS, 1, 100,
                imgs_n(&[100 + i]), empty_vids(), empty_docs(), None,
            ));
        }
        assert_eq!(NextEvidenceId::<Test>::get(), 3);
    });
}

#[test]
fn content_type_variants() {
    let types = vec![
        ContentType::Image, ContentType::Video, ContentType::Document,
        ContentType::Mixed, ContentType::Text,
    ];
    for ct in types {
        let encoded = codec::Encode::encode(&ct);
        let decoded = <ContentType as codec::Decode>::decode(&mut &encoded[..]).unwrap();
        assert_eq!(ct, decoded);
    }
}

// ==================== Round-2 fixes ====================

#[test]
fn commit_creates_ns_index() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert!(EvidenceByNs::<Test>::contains_key((NS, 100), 0));
        assert_eq!(EvidenceCountByNs::<Test>::get((NS, 100)), 1);
    });
}

#[test]
fn link_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0));
        assert_noop!(
            EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0),
            Error::<Test>::DuplicateLink
        );
    });
}

#[test]
fn link_by_ns_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::link_by_ns(RuntimeOrigin::signed(1), NS, 200, 0));
        assert_noop!(
            EvidencePallet::link_by_ns(RuntimeOrigin::signed(1), NS, 200, 0),
            Error::<Test>::DuplicateLink
        );
    });
}

#[test]
fn unlink_decrements_link_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0));
        assert_ok!(EvidencePallet::link(RuntimeOrigin::signed(1), 3, 300, 0));
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 2);
        assert_ok!(EvidencePallet::unlink(RuntimeOrigin::signed(1), 2, 200, 0));
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 1);
    });
}

#[test]
fn unlink_by_ns_decrements_link_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::link_by_ns(RuntimeOrigin::signed(1), NS, 200, 0));
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 1);
        assert_ok!(EvidencePallet::unlink_by_ns(RuntimeOrigin::signed(1), NS, 200, 0));
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 0);
    });
}

#[test]
fn withdraw_cleans_cid_index_and_commit_index() {
    new_test_ext().execute_with(|| {
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000099".to_vec());
        let commit = sp_core::H256::from([1u8; 32]);
        assert_ok!(EvidencePallet::commit_hash(
            RuntimeOrigin::signed(1), NS, 100, commit, Some(memo),
        ));
        assert!(CommitIndex::<Test>::get(commit).is_some());
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        assert!(CommitIndex::<Test>::get(commit).is_none());
    });
}

#[test]
fn withdraw_cleans_parent_children_links() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 1);
        assert!(EvidenceParent::<Test>::get(1).is_some());
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 1));
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 0);
        assert!(EvidenceParent::<Test>::get(1).is_none());
    });
}

#[test]
fn append_evidence_charges_deposit() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        let bal_after_commit = Balances::free_balance(1);
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        assert_eq!(Balances::free_balance(1), bal_after_commit - 10);
        assert!(EvidenceDeposits::<Test>::get(1).is_some());
    });
}

#[test]
fn append_evidence_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // drain account to leave insufficient funds for deposit
        let free = Balances::free_balance(1);
        let _ = Balances::slash(&1, free - 5);
        assert_noop!(
            EvidencePallet::append_evidence(
                RuntimeOrigin::signed(1), 0,
                imgs_n(&[2]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::InsufficientDeposit
        );
    });
}

#[test]
fn commit_hash_uses_document_content_type() {
    new_test_ext().execute_with(|| {
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000099".to_vec());
        let commit = sp_core::H256::from([1u8; 32]);
        assert_ok!(EvidencePallet::commit_hash(
            RuntimeOrigin::signed(1), NS, 100, commit, Some(memo),
        ));
        let ev = Evidences::<Test>::get(0).unwrap();
        assert_eq!(ev.content_type, ContentType::Document);
    });
}

#[test]
fn commit_cleanup_cursor_persists_across_calls() {
    new_test_ext().execute_with(|| {
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000099".to_vec());
        for i in 0..3u8 {
            let commit = sp_core::H256::from([i + 1; 32]);
            assert_ok!(EvidencePallet::commit_hash(
                RuntimeOrigin::signed(1), NS, 100 + i as u64, commit, Some(memo.clone()),
            ));
        }
        assert_eq!(CommitCleanupCursor::<Test>::get(), 0);
        assert_eq!(Evidences::<Test>::get(0).is_some(), true);

        System::set_block_number(600);

        // trigger on_idle which runs cleanup_unrevealed_commitments internally
        EvidencePallet::on_idle(600, frame_support::weights::Weight::MAX);
        // cursor should have advanced past all 3 entries
        assert!(CommitCleanupCursor::<Test>::get() > 0);
        // expired commitments should be cleaned
        assert!(Evidences::<Test>::get(0).is_none());
    });
}

#[test]
fn withdraw_evidence_cleans_link_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), NS, 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0));
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 1);
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        assert_eq!(EvidenceLinkCount::<Test>::get(0), 0);
    });
}

// ==================== P0: withdraw removes Evidences record ====================

#[test]
fn withdraw_removes_evidence_from_storage() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert!(Evidences::<Test>::get(0).is_some());
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        // P0修复: Evidence 记录应被删除（不再存储泄漏）
        assert!(Evidences::<Test>::get(0).is_none());
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Withdrawn);
    });
}

// ==================== P0: unlink checks existence ====================

#[test]
fn unlink_rejects_nonexistent_link() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        // Never linked to (2, 999), so unlink should fail
        assert_noop!(
            EvidencePallet::unlink(RuntimeOrigin::signed(1), 2, 999, 0),
            Error::<Test>::LinkNotFound
        );
    });
}

#[test]
fn unlink_by_ns_rejects_nonexistent_link() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_noop!(
            EvidencePallet::unlink_by_ns(RuntimeOrigin::signed(1), NS, 999, 0),
            Error::<Test>::LinkNotFound
        );
    });
}

// ==================== P0: seal cascades to children ====================

#[test]
fn seal_cascades_to_children() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::append_evidence(RuntimeOrigin::signed(1), 0, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::append_evidence(RuntimeOrigin::signed(1), 0, imgs_n(&[3]), empty_vids(), empty_docs(), None));
        // Seal parent → children should also be sealed
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Sealed);
        assert_eq!(EvidenceStatuses::<Test>::get(1), EvidenceStatus::Sealed);
        assert_eq!(EvidenceStatuses::<Test>::get(2), EvidenceStatus::Sealed);
    });
}

#[test]
fn unseal_cascades_to_children() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::append_evidence(RuntimeOrigin::signed(1), 0, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_eq!(EvidenceStatuses::<Test>::get(1), EvidenceStatus::Sealed);
        // Unseal parent → children should also be unsealed
        assert_ok!(EvidencePallet::unseal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Active);
        assert_eq!(EvidenceStatuses::<Test>::get(1), EvidenceStatus::Active);
    });
}

// ==================== P0: commit_v2 ====================

#[test]
fn commit_v2_works() {
    new_test_ext().execute_with(|| {
        let cid: BoundedVec<u8, <Test as Config>::MaxContentCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000001".to_vec());
        assert_ok!(EvidencePallet::commit_v2(
            RuntimeOrigin::signed(1), NS, 1, 100, cid, ContentType::Document,
        ));
        let ev = Evidences::<Test>::get(0).expect("evidence should exist");
        assert_eq!(ev.content_type, ContentType::Document);
        assert_eq!(ev.domain, 1);
        assert_eq!(ev.target_id, 100);
        assert!(EvidenceByOwner::<Test>::contains_key(1u64, 0u64));
    });
}

#[test]
fn commit_v2_rejects_empty_cid() {
    new_test_ext().execute_with(|| {
        let cid: BoundedVec<u8, <Test as Config>::MaxContentCidLen> = BoundedVec::default();
        assert_noop!(
            EvidencePallet::commit_v2(RuntimeOrigin::signed(1), NS, 1, 100, cid, ContentType::Document),
            Error::<Test>::InvalidCidFormat
        );
    });
}

// ==================== P1: rotate_content_keys creator key check ====================

#[test]
fn rotate_content_keys_rejects_missing_creator_key() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        // New keys without creator (account 1)
        let new_keys: BoundedVec<(u64, BoundedVec<u8, frame_support::traits::ConstU32<512>>), <Test as Config>::MaxAuthorizedUsers> =
            BoundedVec::truncate_from(vec![
                (2u64, BoundedVec::truncate_from(vec![0xAAu8; 32])),
            ]);
        assert_noop!(
            EvidencePallet::rotate_content_keys(
                RuntimeOrigin::signed(1), content_id,
                sp_core::H256::repeat_byte(0x22), new_keys,
            ),
            Error::<Test>::CreatorKeyMissing
        );
    });
}

// ==================== P1: owner index ====================

#[test]
fn owner_index_created_on_commit() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert!(EvidenceByOwner::<Test>::contains_key(1u64, 0u64));
    });
}

#[test]
fn owner_index_cleaned_on_withdraw() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert!(EvidenceByOwner::<Test>::contains_key(1u64, 0u64));
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        assert!(!EvidenceByOwner::<Test>::contains_key(1u64, 0u64));
    });
}

#[test]
fn list_ids_by_owner_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 101, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(2), NS, 1, 102, imgs_n(&[3]), empty_vids(), empty_docs(), None));
        let ids = EvidencePallet::list_ids_by_owner(1u64, 0, 50);
        assert_eq!(ids.len(), 2);
        let ids2 = EvidencePallet::list_ids_by_owner(2u64, 0, 50);
        assert_eq!(ids2.len(), 1);
    });
}

// ==================== P1: seal with reason ====================

#[test]
fn seal_with_reason_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        let reason: BoundedVec<u8, <Test as Config>::MaxReasonLen> =
            BoundedVec::truncate_from(b"dispute #42".to_vec());
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, Some(reason)));
        assert_eq!(EvidenceStatuses::<Test>::get(0), EvidenceStatus::Sealed);
    });
}

// ==================== P1: force operations with reason ====================

#[test]
fn force_remove_with_reason_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        let reason: BoundedVec<u8, <Test as Config>::MaxReasonLen> =
            BoundedVec::truncate_from(b"legal compliance".to_vec());
        assert_ok!(EvidencePallet::force_remove_evidence(RuntimeOrigin::root(), 0, Some(reason)));
        assert!(Evidences::<Test>::get(0).is_none());
        assert!(!EvidenceByOwner::<Test>::contains_key(1u64, 0u64));
    });
}

#[test]
fn force_archive_with_reason_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        let reason: BoundedVec<u8, <Test as Config>::MaxReasonLen> =
            BoundedVec::truncate_from(b"content takedown".to_vec());
        assert_ok!(EvidencePallet::force_archive_evidence(RuntimeOrigin::root(), 0, Some(reason)));
        assert!(Evidences::<Test>::get(0).is_none());
        assert!(ArchivedEvidences::<Test>::get(0).is_some());
        assert!(!EvidenceByOwner::<Test>::contains_key(1u64, 0u64));
    });
}

// ==================== P1: append_evidence fixes EvidenceByNs ====================

#[test]
fn append_evidence_creates_ns_index() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(RuntimeOrigin::signed(1), NS, 1, 100, imgs_n(&[1]), empty_vids(), empty_docs(), None));
        assert_ok!(EvidencePallet::append_evidence(RuntimeOrigin::signed(1), 0, imgs_n(&[2]), empty_vids(), empty_docs(), None));
        assert!(EvidenceByNs::<Test>::contains_key((NS, 100), 1));
        assert_eq!(EvidenceCountByNs::<Test>::get((NS, 100)), 2);
    });
}

// ==================== P1-3: grant_access 补充测试 ====================

#[test]
fn grant_access_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_noop!(
            EvidencePallet::grant_access(RuntimeOrigin::signed(2), content_id, 2, enc_key),
            Error::<Test>::AccessDenied
        );
    });
}

#[test]
fn grant_access_rejects_unregistered_grantee() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_noop!(
            EvidencePallet::grant_access(RuntimeOrigin::signed(1), content_id, 2, enc_key),
            Error::<Test>::PublicKeyNotRegistered
        );
    });
}

#[test]
fn grant_access_updates_existing_key() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        let key_v1: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xAAu8; 32]);
        assert_ok!(EvidencePallet::grant_access(RuntimeOrigin::signed(1), content_id, 2, key_v1));
        let key_v2: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xBBu8; 32]);
        assert_ok!(EvidencePallet::grant_access(RuntimeOrigin::signed(1), content_id, 2, key_v2.clone()));
        let content = PrivateContents::<Test>::get(content_id).unwrap();
        let user_key = content.encrypted_keys.iter().find(|(u, _)| *u == 2).unwrap();
        assert_eq!(user_key.1.as_slice(), &[0xBBu8; 32]);
    });
}

#[test]
fn grant_access_rejects_nonexistent_content() {
    new_test_ext().execute_with(|| {
        register_key(2);
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_noop!(
            EvidencePallet::grant_access(RuntimeOrigin::signed(1), 999, 2, enc_key),
            Error::<Test>::PrivateContentNotFound
        );
    });
}

// ==================== P1-3: reveal_commitment 补充测试 ====================

#[test]
fn reveal_rejects_wrong_hash() {
    new_test_ext().execute_with(|| {
        let cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000077".to_vec());
        let salt: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"salt123".to_vec());
        let version = 1u32;
        let commit_hash = EvidencePallet::compute_evidence_commitment(&NS, 200, cid.as_slice(), salt.as_slice(), version);
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000088".to_vec());
        assert_ok!(EvidencePallet::commit_hash(RuntimeOrigin::signed(1), NS, 200, commit_hash, Some(memo)));
        let wrong_cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa999999".to_vec());
        assert_noop!(
            EvidencePallet::reveal_commitment(RuntimeOrigin::signed(1), 0, wrong_cid, salt, version),
            Error::<Test>::CommitMismatch
        );
    });
}

#[test]
fn reveal_rejects_sealed_evidence() {
    new_test_ext().execute_with(|| {
        let cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000077".to_vec());
        let salt: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"salt123".to_vec());
        let version = 1u32;
        let commit_hash = EvidencePallet::compute_evidence_commitment(&NS, 200, cid.as_slice(), salt.as_slice(), version);
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000088".to_vec());
        assert_ok!(EvidencePallet::commit_hash(RuntimeOrigin::signed(1), NS, 200, commit_hash, Some(memo)));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0, None));
        assert_noop!(
            EvidencePallet::reveal_commitment(RuntimeOrigin::signed(1), 0, cid, salt, version),
            Error::<Test>::EvidenceSealed
        );
    });
}

#[test]
fn reveal_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        let cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000077".to_vec());
        let salt: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"salt123".to_vec());
        let version = 1u32;
        let commit_hash = EvidencePallet::compute_evidence_commitment(&NS, 200, cid.as_slice(), salt.as_slice(), version);
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000088".to_vec());
        assert_ok!(EvidencePallet::commit_hash(RuntimeOrigin::signed(1), NS, 200, commit_hash, Some(memo)));
        assert_noop!(
            EvidencePallet::reveal_commitment(RuntimeOrigin::signed(2), 0, cid, salt, version),
            Error::<Test>::NotAuthorized
        );
    });
}

// ==================== P1-3: register_public_key 补充测试 ====================

#[test]
fn register_public_key_rejects_invalid_key_length() {
    new_test_ext().execute_with(|| {
        let bad_key: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
            BoundedVec::truncate_from(vec![0xABu8; 5]);
        assert_noop!(
            EvidencePallet::register_public_key(
                RuntimeOrigin::signed(1), bad_key, pallet_crypto_common::KeyType::Ed25519,
            ),
            Error::<Test>::InvalidEncryptedKey
        );
    });
}

#[test]
fn register_public_key_overwrites_existing() {
    new_test_ext().execute_with(|| {
        let key1: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
            BoundedVec::truncate_from(vec![0xAAu8; 32]);
        assert_ok!(EvidencePallet::register_public_key(
            RuntimeOrigin::signed(1), key1, pallet_crypto_common::KeyType::Ed25519,
        ));
        let key2: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
            BoundedVec::truncate_from(vec![0xBBu8; 32]);
        assert_ok!(EvidencePallet::register_public_key(
            RuntimeOrigin::signed(1), key2.clone(), pallet_crypto_common::KeyType::Ed25519,
        ));
        let pk = UserPublicKeys::<Test>::get(1).unwrap();
        assert_eq!(pk.key_data.as_slice(), &[0xBBu8; 32]);
    });
}
