//! Benchmarking setup for pallet-grouprobot-registry
//!
//! Every benchmark seeds only this pallet's own storage items.
//! External trait dependencies (`Subscription`) are satisfied by
//! the mock implementation in tests.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as GroupRobotRegistry;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet_grouprobot_primitives::*;

/// Helper: create a bot_id_hash with `tag` as first byte.
fn bot_hash(tag: u8) -> BotIdHash {
    let mut h = [0u8; 32];
    h[0] = tag;
    h
}

/// Helper: seed a registered active bot in storage owned by `owner`.
fn seed_bot<T: Config>(bot_id_hash: &BotIdHash, owner: &T::AccountId) {
    let now = frame_system::Pallet::<T>::block_number();
    let bot = BotInfo::<T> {
        owner: owner.clone(),
        bot_id_hash: *bot_id_hash,
        public_key: [2u8; 32],
        status: BotStatus::Active,
        registered_at: now,
        node_type: NodeType::StandardNode,
        community_count: 0,
    };
    Bots::<T>::insert(bot_id_hash, bot);
    OwnerBots::<T>::mutate(owner, |bots| {
        let _ = bots.try_push(*bot_id_hash);
    });
    BotCount::<T>::mutate(|c| *c = c.saturating_add(1));
}

/// Helper: seed a suspended bot.
fn seed_suspended_bot<T: Config>(bot_id_hash: &BotIdHash, owner: &T::AccountId) {
    seed_bot::<T>(bot_id_hash, owner);
    Bots::<T>::mutate(bot_id_hash, |maybe_bot| {
        if let Some(bot) = maybe_bot {
            bot.status = BotStatus::Suspended;
        }
    });
}

/// Helper: seed a deactivated bot.
fn seed_deactivated_bot<T: Config>(bot_id_hash: &BotIdHash, owner: &T::AccountId) {
    seed_bot::<T>(bot_id_hash, owner);
    Bots::<T>::mutate(bot_id_hash, |maybe_bot| {
        if let Some(bot) = maybe_bot {
            bot.status = BotStatus::Deactivated;
        }
    });
}

/// Helper: seed a community binding for the given bot.
fn seed_community_binding<T: Config>(
    community_id_hash: &CommunityIdHash,
    bot_id_hash: &BotIdHash,
    who: &T::AccountId,
) {
    let now = frame_system::Pallet::<T>::block_number();
    CommunityBindings::<T>::insert(
        community_id_hash,
        CommunityBinding::<T> {
            community_id_hash: *community_id_hash,
            platform: Platform::Telegram,
            bot_id_hash: *bot_id_hash,
            bound_by: who.clone(),
            bound_at: now,
        },
    );
    Bots::<T>::mutate(bot_id_hash, |maybe_bot| {
        if let Some(bot) = maybe_bot {
            bot.community_count = bot.community_count.saturating_add(1);
        }
    });
}

/// Helper: seed a V1 attestation record.
fn seed_attestation<T: Config>(bot_id_hash: &BotIdHash, who: &T::AccountId) {
    let now = frame_system::Pallet::<T>::block_number();
    let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());
    let mrtd = [3u8; 48];
    ApprovedMrtd::<T>::insert(&mrtd, 1u32);
    let record = AttestationRecord::<T> {
        bot_id_hash: *bot_id_hash,
        tdx_quote_hash: [4u8; 32],
        sgx_quote_hash: None,
        mrtd,
        mrenclave: None,
        attester: who.clone(),
        attested_at: now,
        expires_at,
        is_dual_attestation: false,
        quote_verified: false,
        dcap_level: 1,
        api_server_mrtd: None,
        api_server_quote_hash: None,
    };
    Attestations::<T>::insert(bot_id_hash, record);
}

/// Helper: seed a user platform binding.
fn seed_user_platform_binding<T: Config>(who: &T::AccountId, platform: Platform) {
    UserPlatformBindings::<T>::insert(who, platform, [0xAAu8; 32]);
}

/// Helper: seed a peer endpoint into PeerRegistry for a bot.
fn seed_peer<T: Config>(bot_id_hash: &BotIdHash, peer_pk: [u8; 32]) {
    let now = frame_system::Pallet::<T>::block_number();
    PeerRegistry::<T>::mutate(bot_id_hash, |peers| {
        let endpoint: BoundedVec<u8, T::MaxEndpointLen> =
            b"https://node:8443".to_vec().try_into().expect("fits");
        let _ = peers.try_push(PeerEndpoint::<T> {
            public_key: peer_pk,
            endpoint,
            registered_at: now,
            last_seen: now,
        });
    });
    PeerHeartbeatCount::<T>::insert(bot_id_hash, &peer_pk, 1u32);
}

/// Helper: seed an operator.
fn seed_operator<T: Config>(who: &T::AccountId, platform: Platform) {
    let now = frame_system::Pallet::<T>::block_number();
    let name: BoundedVec<u8, T::MaxOperatorNameLen> = b"TestOp".to_vec().try_into().expect("fits");
    let contact: BoundedVec<u8, T::MaxOperatorContactLen> =
        b"@test".to_vec().try_into().expect("fits");
    let app_hash = [0x11u8; 32];
    Operators::<T>::insert(
        who,
        platform,
        OperatorInfo::<T> {
            owner: who.clone(),
            platform,
            platform_app_hash: app_hash,
            name,
            contact,
            status: OperatorStatus::Active,
            registered_at: now,
            bot_count: 0,
            sla_level: 0,
            reputation_score: 100,
        },
    );
    PlatformAppHashIndex::<T>::insert(platform, &app_hash, who);
    OperatorCount::<T>::mutate(|c| *c = c.saturating_add(1));
}

/// Helper: seed a suspended operator.
fn seed_suspended_operator<T: Config>(who: &T::AccountId, platform: Platform) {
    seed_operator::<T>(who, platform);
    Operators::<T>::mutate(who, platform, |maybe_op| {
        if let Some(op) = maybe_op {
            op.status = OperatorStatus::Suspended;
        }
    });
}

/// Helper: assign a bot to an operator.
fn seed_bot_operator_assignment<T: Config>(
    bot_id_hash: &BotIdHash,
    operator: &T::AccountId,
    platform: Platform,
) {
    BotOperator::<T>::insert(bot_id_hash, (operator.clone(), platform));
    OperatorBots::<T>::mutate(operator, platform, |bots| {
        let _ = bots.try_push(*bot_id_hash);
    });
    Operators::<T>::mutate(operator, platform, |maybe_op| {
        if let Some(op) = maybe_op {
            op.bot_count = op.bot_count.saturating_add(1);
        }
    });
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ========================================================================
    // register_bot (call_index 0)
    // ========================================================================
    #[benchmark]
    fn register_bot() {
        let caller: T::AccountId = whitelisted_caller();
        let bot_id_hash: BotIdHash = [1u8; 32];
        let public_key: [u8; 32] = [2u8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bot_id_hash, public_key);

        assert!(Bots::<T>::contains_key(&bot_id_hash));
    }

    // ========================================================================
    // update_public_key (call_index 1)
    // ========================================================================
    #[benchmark]
    fn update_public_key() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let new_key: [u8; 32] = [9u8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, new_key);

        assert_eq!(Bots::<T>::get(&bid).unwrap().public_key, new_key);
    }

    // ========================================================================
    // deactivate_bot (call_index 2)
    // ========================================================================
    #[benchmark]
    fn deactivate_bot() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid);

        assert_eq!(Bots::<T>::get(&bid).unwrap().status, BotStatus::Deactivated);
    }

    // ========================================================================
    // bind_community (call_index 3)
    // ========================================================================
    #[benchmark]
    fn bind_community() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let cid: CommunityIdHash = [0xAAu8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, cid, Platform::Telegram);

        assert!(CommunityBindings::<T>::contains_key(&cid));
    }

    // ========================================================================
    // unbind_community (call_index 4)
    // ========================================================================
    #[benchmark]
    fn unbind_community() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let cid: CommunityIdHash = [0xAAu8; 32];
        seed_community_binding::<T>(&cid, &bid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);

        assert!(!CommunityBindings::<T>::contains_key(&cid));
    }

    // ========================================================================
    // bind_user_platform (call_index 5)
    // ========================================================================
    #[benchmark]
    fn bind_user_platform() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), Platform::Telegram, [0xBBu8; 32]);

        assert!(UserPlatformBindings::<T>::contains_key(&caller, Platform::Telegram));
    }

    // ========================================================================
    // submit_attestation (call_index 6) — deprecated, always returns Err(Deprecated)
    // Weight is pre-calibrated in weights.rs since the extrinsic is minimal.
    // ========================================================================
    #[benchmark]
    fn submit_attestation() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);

        #[block]
        {
            let _result = GroupRobotRegistry::<T>::submit_attestation(
                RawOrigin::Signed(caller).into(),
                bid,
                [0u8; 32],
                None,
                [0u8; 48],
                None,
            );
        }
        Ok(())
    }

    // ========================================================================
    // refresh_attestation (call_index 7)
    // ========================================================================
    #[benchmark]
    fn refresh_attestation() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        seed_attestation::<T>(&bid, &caller);
        let mrtd = [3u8; 48]; // Same as seeded, must be in approved list

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            bid,
            [0x55u8; 32], // new tdx_quote_hash
            None,
            mrtd,
            None,
        );

        assert!(Attestations::<T>::contains_key(&bid));
    }

    // ========================================================================
    // approve_mrtd (call_index 8)
    // ========================================================================
    #[benchmark]
    fn approve_mrtd() {
        let mrtd = [0xFFu8; 48];

        #[extrinsic_call]
        _(RawOrigin::Root, mrtd, 1u32);

        assert!(ApprovedMrtd::<T>::contains_key(&mrtd));
    }

    // ========================================================================
    // approve_mrenclave (call_index 9)
    // ========================================================================
    #[benchmark]
    fn approve_mrenclave() {
        let mrenclave = [0xEEu8; 32];

        #[extrinsic_call]
        _(RawOrigin::Root, mrenclave, 1u32);

        assert!(ApprovedMrenclave::<T>::contains_key(&mrenclave));
    }

    // ========================================================================
    // submit_verified_attestation (call_index 10)
    //   Requires: active bot, nonce, quote with valid report_data and MRTD.
    //   This benchmark seeds storage directly to bypass complex quote verification.
    //   The extrinsic_call will fail due to quote length/content check,
    //   but benchmark captures the weight envelope.
    // ========================================================================
    #[benchmark]
    fn submit_verified_attestation() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);

        // Build a minimal valid-structure TDX quote (>= TDX_MIN_QUOTE_LEN = 632 bytes)
        let mut quote = alloc::vec![0u8; 700];
        // Set MRTD at offset 184..232
        let mrtd = [0xABu8; 48];
        quote[TDX_MRTD_OFFSET..TDX_MRTD_OFFSET + TDX_MRTD_LEN].copy_from_slice(&mrtd);
        // Set report_data[0..32] = SHA256(public_key)
        let pk_hash = sp_core::hashing::sha2_256(&[2u8; 32]);
        quote[TDX_REPORTDATA_OFFSET..TDX_REPORTDATA_OFFSET + 32].copy_from_slice(&pk_hash);
        // Set report_data[32..64] = nonce
        let nonce = [0xCCu8; 32];
        quote[TDX_REPORTDATA_OFFSET + 32..TDX_REPORTDATA_OFFSET + 64].copy_from_slice(&nonce);

        // Seed nonce and MRTD whitelist
        let now = frame_system::Pallet::<T>::block_number();
        AttestationNonces::<T>::insert(&bid, (nonce, now));
        ApprovedMrtd::<T>::insert(&mrtd, 1u32);

        let tdx_quote_raw: BoundedVec<u8, T::MaxQuoteLen> = quote.try_into().expect("fits");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, tdx_quote_raw, None, None);

        assert!(Attestations::<T>::contains_key(&bid));
    }

    // ========================================================================
    // request_attestation_nonce (call_index 11)
    // ========================================================================
    #[benchmark]
    fn request_attestation_nonce() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid);

        assert!(AttestationNonces::<T>::contains_key(&bid));
    }

    // ========================================================================
    // approve_api_server_mrtd (call_index 12)
    // ========================================================================
    #[benchmark]
    fn approve_api_server_mrtd() {
        let mrtd = [0xDDu8; 48];

        #[extrinsic_call]
        _(RawOrigin::Root, mrtd, 1u32);

        assert!(ApprovedApiServerMrtd::<T>::contains_key(&mrtd));
    }

    // ========================================================================
    // register_pck_key (call_index 13)
    // ========================================================================
    #[benchmark]
    fn register_pck_key() {
        let platform_id = [0xCCu8; 32];
        let pck_key = [0xDDu8; 64];

        #[extrinsic_call]
        _(RawOrigin::Root, platform_id, pck_key);

        assert!(RegisteredPckKeys::<T>::contains_key(&platform_id));
    }

    // ========================================================================
    // submit_dcap_attestation (call_index 14)
    //   Requires DCAP verification (crypto). For benchmarks, we measure the
    //   pre-verify overhead. The actual crypto cost is calibrated in weights.rs.
    // ========================================================================
    #[benchmark]
    fn submit_dcap_attestation() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        AttestationNonces::<T>::insert(&bid, ([0xCCu8; 32], now));

        let quote = alloc::vec![0u8; 700];
        let tdx_quote_raw: BoundedVec<u8, T::MaxQuoteLen> = quote.try_into().expect("fits");

        #[block]
        {
            let _result = GroupRobotRegistry::<T>::submit_dcap_attestation(
                RawOrigin::Signed(caller).into(),
                bid,
                tdx_quote_raw,
                None,
                None,
            );
        }
        Ok(())
    }

    // ========================================================================
    // submit_dcap_dual_attestation (call_index 15)
    // ========================================================================
    #[benchmark]
    fn submit_dcap_dual_attestation() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        AttestationNonces::<T>::insert(&bid, ([0xCCu8; 32], now));

        let bot_quote = alloc::vec![0u8; 700];
        let api_quote = alloc::vec![0u8; 700];
        let bot_quote_raw: BoundedVec<u8, T::MaxQuoteLen> = bot_quote.try_into().expect("fits");
        let api_quote_raw: BoundedVec<u8, T::MaxQuoteLen> = api_quote.try_into().expect("fits");

        #[block]
        {
            let _result = GroupRobotRegistry::<T>::submit_dcap_dual_attestation(
                RawOrigin::Signed(caller).into(),
                bid,
                bot_quote_raw,
                api_quote_raw,
                None,
                None,
            );
        }
        Ok(())
    }

    // ========================================================================
    // submit_dcap_full_attestation (call_index 16)
    // ========================================================================
    #[benchmark]
    fn submit_dcap_full_attestation() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        AttestationNonces::<T>::insert(&bid, ([0xCCu8; 32], now));

        let quote = alloc::vec![0u8; 700];
        let pck_cert = alloc::vec![0u8; 100];
        let inter_cert = alloc::vec![0u8; 100];
        let tdx_quote_raw: BoundedVec<u8, T::MaxQuoteLen> = quote.try_into().expect("fits");
        let pck_cert_der: BoundedVec<u8, T::MaxQuoteLen> = pck_cert.try_into().expect("fits");
        let inter_cert_der: BoundedVec<u8, T::MaxQuoteLen> = inter_cert.try_into().expect("fits");

        #[block]
        {
            let _result = GroupRobotRegistry::<T>::submit_dcap_full_attestation(
                RawOrigin::Signed(caller).into(),
                bid,
                tdx_quote_raw,
                pck_cert_der,
                inter_cert_der,
                None,
            );
        }
        Ok(())
    }

    // ========================================================================
    // register_peer (call_index 17)
    // ========================================================================
    #[benchmark]
    fn register_peer() {
        let caller: T::AccountId = whitelisted_caller();
        // bot_hash[0]==1 => MockSubscription returns Basic tier (paid)
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let peer_pk = [0x11u8; 32];
        let endpoint: BoundedVec<u8, T::MaxEndpointLen> =
            b"https://peer:8443".to_vec().try_into().expect("fits");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, peer_pk, endpoint);

        assert_eq!(PeerRegistry::<T>::get(&bid).len(), 1);
    }

    // ========================================================================
    // deregister_peer (call_index 18)
    // ========================================================================
    #[benchmark]
    fn deregister_peer() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let peer_pk = [0x11u8; 32];
        seed_peer::<T>(&bid, peer_pk);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, peer_pk);

        assert_eq!(PeerRegistry::<T>::get(&bid).len(), 0);
    }

    // ========================================================================
    // heartbeat_peer (call_index 19)
    // ========================================================================
    #[benchmark]
    fn heartbeat_peer() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let peer_pk = [0x11u8; 32];
        seed_peer::<T>(&bid, peer_pk);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, peer_pk);

        // Heartbeat count should be incremented
        assert!(PeerHeartbeatCount::<T>::get(&bid, &peer_pk) >= 1);
    }

    // ========================================================================
    // submit_sgx_attestation (call_index 20)
    // ========================================================================
    #[benchmark]
    fn submit_sgx_attestation() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        AttestationNonces::<T>::insert(&bid, ([0xCCu8; 32], now));

        let quote = alloc::vec![0u8; 700];
        let sgx_quote_raw: BoundedVec<u8, T::MaxQuoteLen> = quote.try_into().expect("fits");

        #[block]
        {
            let _result = GroupRobotRegistry::<T>::submit_sgx_attestation(
                RawOrigin::Signed(caller).into(),
                bid,
                sgx_quote_raw,
                None,
                None,
                None,
            );
        }
        Ok(())
    }

    // ========================================================================
    // submit_tee_attestation (call_index 21)
    // ========================================================================
    #[benchmark]
    fn submit_tee_attestation() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        AttestationNonces::<T>::insert(&bid, ([0xCCu8; 32], now));

        let quote = alloc::vec![0u8; 700];
        let quote_raw: BoundedVec<u8, T::MaxQuoteLen> = quote.try_into().expect("fits");

        #[block]
        {
            let _result = GroupRobotRegistry::<T>::submit_tee_attestation(
                RawOrigin::Signed(caller).into(),
                bid,
                quote_raw,
                None,
                None,
                None,
            );
        }
        Ok(())
    }

    // ========================================================================
    // report_stale_peer (call_index 22)
    // ========================================================================
    #[benchmark]
    fn report_stale_peer() {
        let caller: T::AccountId = whitelisted_caller();
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &owner);
        let peer_pk = [0x11u8; 32];
        seed_peer::<T>(&bid, peer_pk);

        // Set peer's last_seen far in the past so it's stale
        PeerRegistry::<T>::mutate(&bid, |peers| {
            if let Some(peer) = peers.iter_mut().find(|p| p.public_key == peer_pk) {
                peer.last_seen = 0u32.into();
            }
        });
        // Advance block past heartbeat timeout
        let timeout: u32 = T::PeerHeartbeatTimeout::get().unique_saturated_into();
        frame_system::Pallet::<T>::set_block_number((timeout + 10).into());

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, peer_pk);

        assert_eq!(PeerRegistry::<T>::get(&bid).len(), 0);
    }

    // ========================================================================
    // register_operator (call_index 23)
    // ========================================================================
    #[benchmark]
    fn register_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let name: BoundedVec<u8, T::MaxOperatorNameLen> =
            b"MyBot".to_vec().try_into().expect("fits");
        let contact: BoundedVec<u8, T::MaxOperatorContactLen> =
            b"@mybot".to_vec().try_into().expect("fits");
        let app_hash = [0x22u8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), Platform::Telegram, app_hash, name, contact);

        assert!(Operators::<T>::contains_key(&caller, Platform::Telegram));
    }

    // ========================================================================
    // update_operator (call_index 24)
    // ========================================================================
    #[benchmark]
    fn update_operator() {
        let caller: T::AccountId = whitelisted_caller();
        seed_operator::<T>(&caller, Platform::Telegram);
        let new_name: BoundedVec<u8, T::MaxOperatorNameLen> =
            b"Updated".to_vec().try_into().expect("fits");
        let new_contact: BoundedVec<u8, T::MaxOperatorContactLen> =
            b"@updated".to_vec().try_into().expect("fits");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), Platform::Telegram, new_name, new_contact);

        let op = Operators::<T>::get(&caller, Platform::Telegram).unwrap();
        assert_eq!(op.name.as_slice(), b"Updated");
    }

    // ========================================================================
    // deregister_operator (call_index 25)
    // ========================================================================
    #[benchmark]
    fn deregister_operator() {
        let caller: T::AccountId = whitelisted_caller();
        seed_operator::<T>(&caller, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), Platform::Telegram);

        assert!(!Operators::<T>::contains_key(&caller, Platform::Telegram));
    }

    // ========================================================================
    // set_operator_sla (call_index 26)
    // ========================================================================
    #[benchmark]
    fn set_operator_sla() {
        let operator: T::AccountId = account("operator", 0, 0);
        seed_operator::<T>(&operator, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Root, operator.clone(), Platform::Telegram, 2u8);

        let op = Operators::<T>::get(&operator, Platform::Telegram).unwrap();
        assert_eq!(op.sla_level, 2);
    }

    // ========================================================================
    // assign_bot_to_operator (call_index 27)
    // ========================================================================
    #[benchmark]
    fn assign_bot_to_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        seed_operator::<T>(&caller, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, Platform::Telegram);

        assert!(BotOperator::<T>::contains_key(&bid));
    }

    // ========================================================================
    // unassign_bot_from_operator (call_index 28)
    // ========================================================================
    #[benchmark]
    fn unassign_bot_from_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        seed_operator::<T>(&caller, Platform::Telegram);
        seed_bot_operator_assignment::<T>(&bid, &caller, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid);

        assert!(!BotOperator::<T>::contains_key(&bid));
    }

    // ========================================================================
    // revoke_mrtd (call_index 29)
    // ========================================================================
    #[benchmark]
    fn revoke_mrtd() {
        let mrtd = [0xFFu8; 48];
        ApprovedMrtd::<T>::insert(&mrtd, 1u32);

        #[extrinsic_call]
        _(RawOrigin::Root, mrtd);

        assert!(!ApprovedMrtd::<T>::contains_key(&mrtd));
    }

    // ========================================================================
    // revoke_mrenclave (call_index 30)
    // ========================================================================
    #[benchmark]
    fn revoke_mrenclave() {
        let mrenclave = [0xEEu8; 32];
        ApprovedMrenclave::<T>::insert(&mrenclave, 1u32);

        #[extrinsic_call]
        _(RawOrigin::Root, mrenclave);

        assert!(!ApprovedMrenclave::<T>::contains_key(&mrenclave));
    }

    // ========================================================================
    // suspend_bot (call_index 31)
    // ========================================================================
    #[benchmark]
    fn suspend_bot() {
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &owner);

        #[extrinsic_call]
        _(RawOrigin::Root, bid);

        assert_eq!(Bots::<T>::get(&bid).unwrap().status, BotStatus::Suspended);
    }

    // ========================================================================
    // reactivate_bot (call_index 32)
    // ========================================================================
    #[benchmark]
    fn reactivate_bot() {
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_suspended_bot::<T>(&bid, &owner);

        #[extrinsic_call]
        _(RawOrigin::Root, bid);

        assert_eq!(Bots::<T>::get(&bid).unwrap().status, BotStatus::Active);
    }

    // ========================================================================
    // unbind_user_platform (call_index 33)
    // ========================================================================
    #[benchmark]
    fn unbind_user_platform() {
        let caller: T::AccountId = whitelisted_caller();
        seed_user_platform_binding::<T>(&caller, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), Platform::Telegram);

        assert!(!UserPlatformBindings::<T>::contains_key(&caller, Platform::Telegram));
    }

    // ========================================================================
    // transfer_bot_ownership (call_index 34)
    // ========================================================================
    #[benchmark]
    fn transfer_bot_ownership() {
        let caller: T::AccountId = whitelisted_caller();
        let new_owner: T::AccountId = account("new_owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, new_owner.clone());

        assert_eq!(Bots::<T>::get(&bid).unwrap().owner, new_owner);
    }

    // ========================================================================
    // revoke_api_server_mrtd (call_index 35)
    // ========================================================================
    #[benchmark]
    fn revoke_api_server_mrtd() {
        let mrtd = [0xDDu8; 48];
        ApprovedApiServerMrtd::<T>::insert(&mrtd, 1u32);

        #[extrinsic_call]
        _(RawOrigin::Root, mrtd);

        assert!(!ApprovedApiServerMrtd::<T>::contains_key(&mrtd));
    }

    // ========================================================================
    // revoke_pck_key (call_index 36)
    // ========================================================================
    #[benchmark]
    fn revoke_pck_key() {
        let platform_id = [0xCCu8; 32];
        let now = frame_system::Pallet::<T>::block_number();
        RegisteredPckKeys::<T>::insert(&platform_id, ([0xDDu8; 64], now));

        #[extrinsic_call]
        _(RawOrigin::Root, platform_id);

        assert!(!RegisteredPckKeys::<T>::contains_key(&platform_id));
    }

    // ========================================================================
    // force_deactivate_bot (call_index 37)
    // ========================================================================
    #[benchmark]
    fn force_deactivate_bot() {
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &owner);

        #[extrinsic_call]
        _(RawOrigin::Root, bid);

        assert_eq!(Bots::<T>::get(&bid).unwrap().status, BotStatus::Deactivated);
    }

    // ========================================================================
    // suspend_operator (call_index 38)
    // ========================================================================
    #[benchmark]
    fn suspend_operator() {
        let operator: T::AccountId = account("operator", 0, 0);
        seed_operator::<T>(&operator, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Root, operator.clone(), Platform::Telegram);

        let op = Operators::<T>::get(&operator, Platform::Telegram).unwrap();
        assert_eq!(op.status, OperatorStatus::Suspended);
    }

    // ========================================================================
    // unsuspend_operator (call_index 39)
    // ========================================================================
    #[benchmark]
    fn unsuspend_operator() {
        let operator: T::AccountId = account("operator", 0, 0);
        seed_suspended_operator::<T>(&operator, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Root, operator.clone(), Platform::Telegram);

        let op = Operators::<T>::get(&operator, Platform::Telegram).unwrap();
        assert_eq!(op.status, OperatorStatus::Active);
    }

    // ========================================================================
    // update_peer_endpoint (call_index 40)
    // ========================================================================
    #[benchmark]
    fn update_peer_endpoint() {
        let caller: T::AccountId = whitelisted_caller();
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &caller);
        let peer_pk = [0x11u8; 32];
        seed_peer::<T>(&bid, peer_pk);
        let new_endpoint: BoundedVec<u8, T::MaxEndpointLen> =
            b"https://new-endpoint:9443".to_vec().try_into().expect("fits");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid, peer_pk, new_endpoint);

        let peers = PeerRegistry::<T>::get(&bid);
        assert_eq!(peers[0].endpoint.as_slice(), b"https://new-endpoint:9443");
    }

    // ========================================================================
    // cleanup_deactivated_bot (call_index 41)
    // ========================================================================
    #[benchmark]
    fn cleanup_deactivated_bot() {
        let caller: T::AccountId = whitelisted_caller();
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_deactivated_bot::<T>(&bid, &owner);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid);

        assert!(!Bots::<T>::contains_key(&bid));
    }

    // ========================================================================
    // operator_unassign_bot (call_index 42)
    // ========================================================================
    #[benchmark]
    fn operator_unassign_bot() {
        let caller: T::AccountId = whitelisted_caller();
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &owner);
        seed_operator::<T>(&caller, Platform::Telegram);
        seed_bot_operator_assignment::<T>(&bid, &caller, Platform::Telegram);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bid);

        assert!(!BotOperator::<T>::contains_key(&bid));
    }

    // ========================================================================
    // force_expire_attestation (call_index 43)
    // ========================================================================
    #[benchmark]
    fn force_expire_attestation() {
        let owner: T::AccountId = account("owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &owner);
        seed_attestation::<T>(&bid, &owner);

        #[extrinsic_call]
        _(RawOrigin::Root, bid);

        assert!(!Attestations::<T>::contains_key(&bid));
    }

    // ========================================================================
    // force_transfer_bot_ownership (call_index 44)
    // ========================================================================
    #[benchmark]
    fn force_transfer_bot_ownership() {
        let owner: T::AccountId = account("owner", 0, 0);
        let new_owner: T::AccountId = account("new_owner", 0, 0);
        let bid = bot_hash(1);
        seed_bot::<T>(&bid, &owner);

        #[extrinsic_call]
        _(RawOrigin::Root, bid, new_owner.clone());

        assert_eq!(Bots::<T>::get(&bid).unwrap().owner, new_owner);
    }

    impl_benchmark_test_suite!(
        GroupRobotRegistry,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
