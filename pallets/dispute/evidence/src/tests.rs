use crate::{mock::*, pallet::*};
use frame_support::{assert_ok, assert_noop, BoundedVec};

/// Generate a valid CIDv1-like string (starts with 'b', 20+ chars, unique per index)
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

// ==================== commit (call_index 0) ====================

#[test]
fn commit_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1),
            1, // domain
            100, // target_id
            imgs_n(&[1]),
            empty_vids(),
            empty_docs(),
            None,
        ));
        let ev = Evidences::<Test>::get(0).expect("evidence should exist");
        assert_eq!(ev.id, 0);
        assert_eq!(ev.domain, 1);
        assert_eq!(ev.target_id, 100);
        assert_eq!(ev.owner, 1);
        assert_eq!(NextEvidenceId::<Test>::get(), 1);
    });
}

#[test]
fn commit_not_authorized() {
    new_test_ext().execute_with(|| {
        set_authorized(false);
        assert_noop!(
            EvidencePallet::commit(
                RuntimeOrigin::signed(1),
                1, 100,
                imgs_n(&[1]),
                empty_vids(),
                empty_docs(),
                None,
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
                RuntimeOrigin::signed(1),
                1, 100,
                BoundedVec::default(), // empty imgs
                empty_vids(),
                empty_docs(),
                None,
            ),
            Error::<Test>::InvalidCidFormat
        );
    });
}

#[test]
fn commit_increments_target_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        let count = EvidenceCountByTarget::<Test>::get((1u8, 100u64));
        assert_eq!(count, 2);
    });
}

// ==================== commit_hash (call_index 1) ====================

#[test]
fn commit_hash_works() {
    new_test_ext().execute_with(|| {
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000099".to_vec());
        assert_ok!(EvidencePallet::commit_hash(
            RuntimeOrigin::signed(1),
            *b"evidence", // ns
            200, // subject_id
            sp_core::H256::repeat_byte(0x42), // commit hash
            Some(memo),
        ));
        let ev = Evidences::<Test>::get(0).expect("evidence should exist");
        assert_eq!(ev.target_id, 200);
        assert!(ev.commit.is_some());
    });
}

// ==================== link / unlink (call_index 2/4) ====================

#[test]
fn link_and_unlink_works() {
    new_test_ext().execute_with(|| {
        // First commit evidence
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Link to another target
        assert_ok!(EvidencePallet::link(
            RuntimeOrigin::signed(1), 2, 200, 0,
        ));
        assert!(EvidenceByTarget::<Test>::get((2u8, 200u64), 0).is_some());

        // Unlink
        assert_ok!(EvidencePallet::unlink(
            RuntimeOrigin::signed(1), 2, 200, 0,
        ));
        assert!(EvidenceByTarget::<Test>::get((2u8, 200u64), 0).is_none());
    });
}

#[test]
fn link_not_authorized() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        set_authorized(false);
        assert_noop!(
            EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0),
            Error::<Test>::NotAuthorized
        );
    });
}

// ==================== register_public_key (call_index 6) ====================

#[test]
fn register_public_key_works() {
    new_test_ext().execute_with(|| {
        let key: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
            BoundedVec::truncate_from([0xABu8; 32].to_vec()); // Ed25519 = exactly 32 bytes
        assert_ok!(EvidencePallet::register_public_key(
            RuntimeOrigin::signed(1), key, 2, // key_type: Ed25519
        ));
        let stored = UserPublicKeys::<Test>::get(1);
        assert!(stored.is_some());
    });
}

// ==================== Evidence existence checker ====================

#[test]
fn evidence_id_increments() {
    new_test_ext().execute_with(|| {
        for i in 0..3u32 {
            assert_ok!(EvidencePallet::commit(
                RuntimeOrigin::signed(1), 1, 100,
                imgs_n(&[100 + i]),
                empty_vids(), empty_docs(), None,
            ));
        }
        assert_eq!(NextEvidenceId::<Test>::get(), 3);
        assert!(Evidences::<Test>::get(0).is_some());
        assert!(Evidences::<Test>::get(1).is_some());
        assert!(Evidences::<Test>::get(2).is_some());
        assert!(Evidences::<Test>::get(3).is_none());
    });
}

// ==================== ContentType ====================

#[test]
fn content_type_variants() {
    // Ensure all variants can be encoded/decoded
    let types = vec![
        ContentType::Image,
        ContentType::Video,
        ContentType::Document,
        ContentType::Mixed,
        ContentType::Text,
    ];
    for ct in types {
        let encoded = codec::Encode::encode(&ct);
        let decoded = <ContentType as codec::Decode>::decode(&mut &encoded[..]).unwrap();
        assert_eq!(ct, decoded);
    }
}

// ==================== request_access (call_index 13) ====================

/// Helper: register Ed25519 public key for an account
fn register_key(account: u64) {
    let key: BoundedVec<u8, <Test as Config>::MaxKeyLen> =
        BoundedVec::truncate_from([0xABu8; 32].to_vec());
    assert_ok!(EvidencePallet::register_public_key(
        RuntimeOrigin::signed(account), key, 2,
    ));
}

/// Helper: store a private content as account 1 directly into storage, returns content_id.
/// Bypasses extrinsic CID validation (encrypted prefix + IPFS format check conflict is a
/// pre-existing issue in store_private_content — not in scope here).
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
    let content_id = NextPrivateContentId::<Test>::mutate(|id| {
        let current = *id;
        *id += 1;
        current
    });
    let content = pallet_crypto_common::PrivateContent::<
        <Test as frame_system::Config>::AccountId,
        u64,
        <Test as Config>::MaxCidLen,
        <Test as Config>::MaxAuthorizedUsers,
        <Test as Config>::MaxKeyLen,
    > {
        id: content_id,
        ns: *b"evidence",
        subject_id: 500,
        cid: cid.clone(),
        content_hash: sp_core::H256::repeat_byte(0x11),
        encryption_method: 1, // AES-256-GCM
        creator: 1u64,
        access_policy: pallet_crypto_common::AccessPolicy::OwnerOnly,
        encrypted_keys,
        created_at: 1u64,
        updated_at: 1u64,
    };
    PrivateContents::<Test>::insert(content_id, &content);
    PrivateContentByCid::<Test>::insert(&cid, content_id);
    PrivateContentBySubject::<Test>::insert((*b"evidence", 500u64), content_id, ());
    content_id
}

#[test]
fn request_access_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);

        assert_ok!(EvidencePallet::request_access(
            RuntimeOrigin::signed(2), content_id,
        ));
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_some());

        // Check event emitted
        System::assert_has_event(RuntimeEvent::EvidencePallet(
            crate::pallet::Event::AccessRequested { content_id, requester: 2 },
        ));
    });
}

#[test]
fn request_access_rejects_without_public_key() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        // Account 3 has NOT registered a public key
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
        // Account 1 is the creator
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

        // Grant access to account 2 first
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_ok!(EvidencePallet::grant_access(
            RuntimeOrigin::signed(1), content_id, 2, enc_key,
        ));

        // Now request_access should fail
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

#[test]
fn grant_access_auto_removes_request() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);

        // Request access
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_some());

        // Creator grants access
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xCCu8; 32]);
        assert_ok!(EvidencePallet::grant_access(
            RuntimeOrigin::signed(1), content_id, 2, enc_key,
        ));

        // Request should be auto-removed
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_none());
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
        assert_ok!(EvidencePallet::update_access_policy(
            RuntimeOrigin::signed(1), content_id, new_policy,
        ));

        // Verify policy updated
        let content = PrivateContents::<Test>::get(content_id).unwrap();
        match &content.access_policy {
            pallet_crypto_common::AccessPolicy::SharedWith(users) => {
                assert_eq!(users.len(), 2);
            }
            _ => panic!("expected SharedWith policy"),
        }
    });
}

#[test]
fn update_access_policy_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        let new_policy = pallet_crypto_common::AccessPolicy::GovernanceControlled;
        assert_noop!(
            EvidencePallet::update_access_policy(RuntimeOrigin::signed(2), content_id, new_policy),
            Error::<Test>::AccessDenied
        );
    });
}

// ==================== PrivateContentProvider impl ====================

#[test]
fn private_content_provider_can_access() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        // Creator can access
        assert!(<EvidencePallet as crate::pallet::PrivateContentProvider<u64>>::can_access(content_id, &1));
        // Non-authorized cannot
        assert!(!<EvidencePallet as crate::pallet::PrivateContentProvider<u64>>::can_access(content_id, &2));
    });
}

#[test]
fn private_content_provider_get_decryption_key() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        // Creator should get their key
        let key = <EvidencePallet as crate::pallet::PrivateContentProvider<u64>>::get_decryption_key(content_id, &1);
        assert!(key.is_some());
        assert_eq!(key.unwrap(), vec![0xFFu8; 32]);
        // Non-authorized gets None
        let key2 = <EvidencePallet as crate::pallet::PrivateContentProvider<u64>>::get_decryption_key(content_id, &2);
        assert!(key2.is_none());
    });
}

// ==================== get_decryption_info ====================

#[test]
fn get_decryption_info_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        let info = EvidencePallet::get_decryption_info(content_id, &1);
        assert!(info.is_some());
        let (cid, hash, method, key) = info.unwrap();
        assert_eq!(cid, b"enc-bafybeigdyrzt5sfp7udm7hu".to_vec());
        assert_eq!(hash, sp_core::H256::repeat_byte(0x11));
        assert_eq!(method, 1);
        assert_eq!(key, vec![0xFFu8; 32]);
    });
}

#[test]
fn get_decryption_info_returns_none_for_unauthorized() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        assert!(EvidencePallet::get_decryption_info(content_id, &2).is_none());
    });
}

// ==================== list_access_requests ====================

#[test]
fn list_access_requests_works() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        register_key(3);
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(3), content_id));
        let requests = EvidencePallet::list_access_requests(content_id);
        assert_eq!(requests.len(), 2);
    });
}

// ==================== Rate limiting (window) ====================

#[test]
fn commit_respects_window_limit() {
    new_test_ext().execute_with(|| {
        // MaxPerWindow = 50, so we can commit up to 50 in the same window
        for i in 0..50u32 {
            assert_ok!(EvidencePallet::commit(
                RuntimeOrigin::signed(1), 1, i as u64,
                imgs_n(&[200 + i]),
                empty_vids(), empty_docs(), None,
            ));
        }
        // 51st should fail
        assert_noop!(
            EvidencePallet::commit(
                RuntimeOrigin::signed(1), 1, 999,
                imgs_n(&[999]),
                empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::RateLimited
        );
    });
}
