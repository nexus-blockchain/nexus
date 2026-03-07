//! Benchmarking for pallet-grouprobot-registry
//!
//! 全部 44 个 extrinsics 均有 benchmark。
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_grouprobot_primitives::*;
use sp_runtime::traits::Saturating;

// ==================== Helper 函数 ====================

fn bot_hash(n: u8) -> BotIdHash {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

fn community_hash(n: u8) -> CommunityIdHash {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

fn mrtd_val(n: u8) -> [u8; 48] {
    let mut m = [0u8; 48];
    m[0] = n;
    m
}

fn mrenclave_val(n: u8) -> [u8; 32] {
    let mut m = [0u8; 32];
    m[0] = n;
    m
}

fn pk(n: u8) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[0] = n;
    k
}

/// 注册一个 Bot 并返回 bot_id_hash
fn seed_bot<T: Config>(owner: &T::AccountId, n: u8) -> BotIdHash {
    let bh = bot_hash(n);
    let now = frame_system::Pallet::<T>::block_number();
    let bot = BotInfo::<T> {
        owner: owner.clone(),
        bot_id_hash: bh,
        public_key: pk(n),
        status: BotStatus::Active,
        registered_at: now,
        node_type: NodeType::StandardNode,
        community_count: 0,
    };
    Bots::<T>::insert(&bh, bot);
    OwnerBots::<T>::mutate(owner, |bots| { let _ = bots.try_push(bh); });
    BotCount::<T>::mutate(|c| *c = c.saturating_add(1));
    bh
}

/// 注册一个 TeeNode Bot (有 V1 证明)
fn seed_tee_bot<T: Config>(owner: &T::AccountId, n: u8) -> BotIdHash {
    let bh = seed_bot::<T>(owner, n);
    let now = frame_system::Pallet::<T>::block_number();
    let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());
    let m = mrtd_val(n);
    ApprovedMrtd::<T>::insert(&m, 1u32);

    let record = AttestationRecord::<T> {
        bot_id_hash: bh,
        tdx_quote_hash: [n; 32],
        sgx_quote_hash: None,
        mrtd: m,
        mrenclave: None,
        attester: owner.clone(),
        attested_at: now,
        expires_at,
        is_dual_attestation: false,
        quote_verified: false,
        dcap_level: 0,
        api_server_mrtd: None,
        api_server_quote_hash: None,
    };
    Attestations::<T>::insert(&bh, record);

    let now_u64: u64 = now.unique_saturated_into();
    let expires_u64: u64 = expires_at.unique_saturated_into();
    Bots::<T>::mutate(&bh, |maybe_bot| {
        if let Some(bot) = maybe_bot {
            bot.node_type = NodeType::TeeNode {
                mrtd: m,
                mrenclave: None,
                tdx_attested_at: now_u64,
                sgx_attested_at: None,
                expires_at: expires_u64,
            };
        }
    });
    bh
}

/// 注册一个 Peer
fn seed_peer<T: Config>(bot_id_hash: &BotIdHash, peer_pk: [u8; 32]) {
    let now = frame_system::Pallet::<T>::block_number();
    let endpoint: BoundedVec<u8, T::MaxEndpointLen> =
        b"https://node:8443".to_vec().try_into().unwrap();
    let peer = PeerEndpoint::<T> {
        public_key: peer_pk,
        endpoint,
        registered_at: now,
        last_seen: now,
    };
    PeerRegistry::<T>::mutate(bot_id_hash, |peers| { let _ = peers.try_push(peer); });
}

/// 注册一个运营商
fn seed_operator<T: Config>(owner: &T::AccountId, platform: Platform) {
    let now = frame_system::Pallet::<T>::block_number();
    let name: BoundedVec<u8, T::MaxOperatorNameLen> =
        b"BenchOp".to_vec().try_into().unwrap();
    let contact: BoundedVec<u8, T::MaxOperatorContactLen> =
        b"@bench".to_vec().try_into().unwrap();
    let app_hash = [1u8; 32];
    let info = OperatorInfo::<T> {
        owner: owner.clone(),
        platform,
        platform_app_hash: app_hash,
        name,
        contact,
        status: OperatorStatus::Active,
        registered_at: now,
        bot_count: 0,
        sla_level: 0,
        reputation_score: 100,
    };
    Operators::<T>::insert(owner, platform, info);
    PlatformAppHashIndex::<T>::insert(platform, app_hash, owner);
    OperatorCount::<T>::mutate(|c| *c = c.saturating_add(1));
}

/// 绑定社区
fn seed_community_binding<T: Config>(bot_id_hash: BotIdHash, community_n: u8, owner: &T::AccountId) {
    let now = frame_system::Pallet::<T>::block_number();
    let ch = community_hash(community_n);
    let binding = CommunityBinding::<T> {
        community_id_hash: ch,
        platform: Platform::Telegram,
        bot_id_hash,
        bound_by: owner.clone(),
        bound_at: now,
    };
    CommunityBindings::<T>::insert(&ch, binding);
    Bots::<T>::mutate(&bot_id_hash, |maybe_bot| {
        if let Some(bot) = maybe_bot {
            bot.community_count = bot.community_count.saturating_add(1);
        }
    });
}
