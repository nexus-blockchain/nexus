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
            RuntimeOrigin::signed(1), key, pallet_crypto_common::KeyType::Ed25519,
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
        RuntimeOrigin::signed(account), key, pallet_crypto_common::KeyType::Ed25519,
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
        encryption_method: pallet_crypto_common::EncryptionMethod::Aes256Gcm,
        creator: 1u64,
        access_policy: pallet_crypto_common::AccessPolicy::OwnerOnly,
        encrypted_keys,
        status: pallet_crypto_common::ContentStatus::Active,
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
        assert!(<EvidencePallet as pallet_crypto_common::PrivateContentProvider<u64>>::can_access(content_id, &1));
        // Non-authorized cannot
        assert!(!<EvidencePallet as pallet_crypto_common::PrivateContentProvider<u64>>::can_access(content_id, &2));
    });
}

#[test]
fn private_content_provider_get_decryption_key() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        // Creator should get their key
        let key = <EvidencePallet as pallet_crypto_common::PrivateContentProvider<u64>>::get_encrypted_key(content_id, &1);
        assert!(key.is_some());
        assert_eq!(key.unwrap(), vec![0xFFu8; 32]);
        // Non-authorized gets None
        let key2 = <EvidencePallet as pallet_crypto_common::PrivateContentProvider<u64>>::get_encrypted_key(content_id, &2);
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
        assert_eq!(method, pallet_crypto_common::EncryptionMethod::Aes256Gcm);
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

// ==================== H1: archive_old_evidences skips sealed evidence ====================

#[test]
fn h1_archive_skips_sealed_evidence() {
    new_test_ext().execute_with(|| {
        // Create two evidences
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 101,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        // Seal evidence 0
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0));
        assert!(SealedEvidences::<Test>::contains_key(0));

        // Advance blocks past archive delay (50 in mock)
        System::set_block_number(100);

        // Run archive — should skip sealed evidence 0, archive evidence 1
        let archived = EvidencePallet::archive_old_evidences(10);
        // Evidence 0 should NOT be archived (sealed)
        assert!(Evidences::<Test>::get(0).is_some(), "sealed evidence must not be archived");
        assert!(ArchivedEvidences::<Test>::get(0).is_none());
        // Evidence 1 should be archived
        assert!(Evidences::<Test>::get(1).is_none(), "unsealed evidence should be archived");
        assert!(ArchivedEvidences::<Test>::get(1).is_some());
        assert_eq!(archived, 1);
    });
}

// ==================== H2: append_evidence rejects sealed/withdrawn parent ====================

#[test]
fn h2_append_rejects_sealed_parent() {
    new_test_ext().execute_with(|| {
        // Create parent evidence
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Seal it
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0));

        // Try to append to sealed parent — should fail
        assert_noop!(
            EvidencePallet::append_evidence(
                RuntimeOrigin::signed(1), 0,
                imgs_n(&[2]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::EvidenceSealed
        );
    });
}

#[test]
fn h2_append_rejects_withdrawn_parent() {
    new_test_ext().execute_with(|| {
        // Create parent evidence
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Withdraw it
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));

        // Try to append to withdrawn parent — should fail
        assert_noop!(
            EvidencePallet::append_evidence(
                RuntimeOrigin::signed(1), 0,
                imgs_n(&[2]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::InvalidEvidenceStatus
        );
    });
}

#[test]
fn h2_append_works_for_active_parent() {
    new_test_ext().execute_with(|| {
        // Create parent evidence
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Append to active parent — should succeed
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        assert_eq!(NextEvidenceId::<Test>::get(), 2);
        let children = EvidenceChildren::<Test>::get(0);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], 1);
    });
}

// ==================== M1: revoke_access rejects non-existent user ====================

#[test]
fn m1_revoke_access_rejects_non_authorized_user() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        // Try to revoke access for user 5 who was never granted access
        assert_noop!(
            EvidencePallet::revoke_access(RuntimeOrigin::signed(1), content_id, 5),
            Error::<Test>::NotFound
        );
    });
}

// ==================== H1-R3: CommitIndex cleaned on reveal ====================

#[test]
fn h1r3_reveal_cleans_commit_index() {
    new_test_ext().execute_with(|| {
        let ns = *b"evidence";
        let cid: BoundedVec<u8, <Test as Config>::MaxCidLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000077".to_vec());
        let salt: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"salt123".to_vec());
        let version = 1u32;
        let commit_hash = EvidencePallet::compute_evidence_commitment(
            &ns, 200, cid.as_slice(), salt.as_slice(), version,
        );
        // commit_hash creates CommitIndex entry
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000088".to_vec());
        assert_ok!(EvidencePallet::commit_hash(
            RuntimeOrigin::signed(1), ns, 200, commit_hash, Some(memo),
        ));
        assert!(CommitIndex::<Test>::get(commit_hash).is_some());

        // reveal should clean CommitIndex
        assert_ok!(EvidencePallet::reveal_commitment(
            RuntimeOrigin::signed(1), 0, cid, salt, version,
        ));
        assert!(CommitIndex::<Test>::get(commit_hash).is_none(), "CommitIndex must be cleaned after reveal");
    });
}

// ==================== H1-R3: CommitIndex cleaned on archive ====================

#[test]
fn h1r3_archive_cleans_commit_index() {
    new_test_ext().execute_with(|| {
        let commit_hash = sp_core::H256::repeat_byte(0x55);
        let memo: BoundedVec<u8, <Test as Config>::MaxMemoLen> =
            BoundedVec::truncate_from(b"bafkreiaaaaaaaaaa000066".to_vec());
        assert_ok!(EvidencePallet::commit_hash(
            RuntimeOrigin::signed(1), *b"evidence", 300, commit_hash, Some(memo),
        ));
        assert!(CommitIndex::<Test>::get(commit_hash).is_some());

        // Advance past archive delay
        System::set_block_number(100);
        let archived = EvidencePallet::archive_old_evidences(10);
        assert_eq!(archived, 1);
        // CommitIndex must be cleaned
        assert!(CommitIndex::<Test>::get(commit_hash).is_none(), "CommitIndex must be cleaned after archive");
    });
}

// ==================== M1-R3: append_evidence respects global CID dedup ====================

#[test]
fn m1r3_append_rejects_duplicate_cid() {
    new_test_ext().execute_with(|| {
        // Create parent with CID [1]
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Append with the same CID [1] — should fail due to global dedup
        assert_noop!(
            EvidencePallet::append_evidence(
                RuntimeOrigin::signed(1), 0,
                imgs_n(&[1]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::DuplicateCidGlobal
        );
    });
}

#[test]
fn m1r3_append_registers_cid_in_index() {
    new_test_ext().execute_with(|| {
        // Create parent with CID [1]
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Append with CID [2] — should succeed
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        // Now committing with CID [2] should fail — it was registered by append
        assert_noop!(
            EvidencePallet::commit(
                RuntimeOrigin::signed(1), 1, 101,
                imgs_n(&[2]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::DuplicateCidGlobal
        );
    });
}

// ==================== M2-R3: link rejects sealed/withdrawn evidence ====================

#[test]
fn m2r3_link_rejects_sealed_evidence() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::seal_evidence(RuntimeOrigin::signed(1), 0));
        assert_noop!(
            EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0),
            Error::<Test>::EvidenceSealed
        );
    });
}

#[test]
fn m2r3_link_rejects_withdrawn_evidence() {
    new_test_ext().execute_with(|| {
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        assert_ok!(EvidencePallet::withdraw_evidence(RuntimeOrigin::signed(1), 0));
        assert_noop!(
            EvidencePallet::link(RuntimeOrigin::signed(1), 2, 200, 0),
            Error::<Test>::InvalidEvidenceStatus
        );
    });
}

// ==================== M3-R3: archive cleans children refs both directions ====================

#[test]
fn m3r3_archive_cleans_children_parent_refs() {
    new_test_ext().execute_with(|| {
        // Create parent evidence (id=0)
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Append child (id=1)
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        assert!(EvidenceParent::<Test>::get(1).is_some());
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 1);

        // Archive parent
        System::set_block_number(100);
        let archived = EvidencePallet::archive_old_evidences(10);
        assert!(archived >= 1);

        // Child's EvidenceParent should be cleaned (M3-R3 fix)
        assert!(EvidenceParent::<Test>::get(1).is_none(), "child parent ref must be cleaned after parent archived");
        // Parent's EvidenceChildren should be removed
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 0, "archived evidence children list must be cleaned");
    });
}

// ==================== H1-R4: force_archive_evidence cleans children parent refs ====================

#[test]
fn h1r4_force_archive_cleans_children_parent_refs() {
    new_test_ext().execute_with(|| {
        // Create parent evidence (id=0)
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Append child (id=1)
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[2]), empty_vids(), empty_docs(), None,
        ));
        // Append another child (id=2)
        assert_ok!(EvidencePallet::append_evidence(
            RuntimeOrigin::signed(1), 0,
            imgs_n(&[3]), empty_vids(), empty_docs(), None,
        ));
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 2);
        assert!(EvidenceParent::<Test>::get(1).is_some());
        assert!(EvidenceParent::<Test>::get(2).is_some());

        // Force archive parent
        assert_ok!(EvidencePallet::force_archive_evidence(RuntimeOrigin::root(), 0));

        // Children's EvidenceParent should be cleaned (H1-R4 fix)
        assert!(EvidenceParent::<Test>::get(1).is_none(), "child 1 parent ref must be cleaned");
        assert!(EvidenceParent::<Test>::get(2).is_none(), "child 2 parent ref must be cleaned");
        // Parent's EvidenceChildren should be removed
        assert_eq!(EvidenceChildren::<Test>::get(0).len(), 0, "archived evidence children list must be cleaned");
        // Archived record should exist
        assert!(ArchivedEvidences::<Test>::get(0).is_some());
    });
}

// ==================== M3-R4: append_evidence respects MaxPerSubjectTarget quota ====================

#[test]
fn m3r4_append_rejects_when_subject_quota_exceeded() {
    new_test_ext().execute_with(|| {
        // MaxPerSubjectTarget = 100, MaxPerWindow = 50, WindowBlocks = 100
        // Create parent evidence (id=0) for (domain=1, target_id=100)
        assert_ok!(EvidencePallet::commit(
            RuntimeOrigin::signed(1), 1, 100,
            imgs_n(&[1]), empty_vids(), empty_docs(), None,
        ));
        // Fill up quota in batches, advancing blocks to avoid rate limiting
        for i in 1..50u32 {
            assert_ok!(EvidencePallet::commit(
                RuntimeOrigin::signed(1), 1, 100,
                imgs_n(&[1000 + i]), empty_vids(), empty_docs(), None,
            ));
        }
        // Advance past window to reset rate limit
        System::set_block_number(200);
        for i in 50..100u32 {
            assert_ok!(EvidencePallet::commit(
                RuntimeOrigin::signed(1), 1, 100,
                imgs_n(&[1000 + i]), empty_vids(), empty_docs(), None,
            ));
        }
        assert_eq!(EvidenceCountByTarget::<Test>::get((1u8, 100u64)), 100);

        // Advance again to reset rate limit for the append call
        System::set_block_number(400);
        // Append should now fail due to MaxPerSubjectTarget quota
        assert_noop!(
            EvidencePallet::append_evidence(
                RuntimeOrigin::signed(1), 0,
                imgs_n(&[9999]), empty_vids(), empty_docs(), None,
            ),
            Error::<Test>::TooManyForSubject
        );
    });
}

// ==================== M2-R5: grant_access cleans AccessRequests (functional regression) ====================

#[test]
fn m2r5_grant_access_always_removes_access_request() {
    new_test_ext().execute_with(|| {
        let content_id = store_test_private_content();
        register_key(2);
        register_key(3);

        // Two users request access
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(2), content_id));
        assert_ok!(EvidencePallet::request_access(RuntimeOrigin::signed(3), content_id));
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_some());
        assert!(AccessRequests::<Test>::get(content_id, 3u64).is_some());

        // Grant access to user 2 — should auto-remove their request
        let enc_key: BoundedVec<u8, frame_support::traits::ConstU32<512>> =
            BoundedVec::truncate_from(vec![0xDDu8; 32]);
        assert_ok!(EvidencePallet::grant_access(
            RuntimeOrigin::signed(1), content_id, 2, enc_key,
        ));
        // User 2's request cleaned, user 3's remains
        assert!(AccessRequests::<Test>::get(content_id, 2u64).is_none(),
            "M2-R5: grant_access must remove pending AccessRequest");
        assert!(AccessRequests::<Test>::get(content_id, 3u64).is_some(),
            "unrelated request must survive");
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
