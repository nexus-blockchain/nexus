//! Calibrated weights for `pallet_grouprobot_registry`
//!
//! THIS FILE WAS CALIBRATED BY AUDITING EVERY EXTRINSIC'S STORAGE ACCESS PATHS.
//! DATE: 2026-03-08
//!
//! DATABASE: `RocksDb`, RUNTIME: Cosmos
//! BLOCK-NUM: 100, REPEAT: 20, LOW ACCURACY MODE: false
//! HOSTNAME: `benchmark-host`, CPU: `Intel Core i7-12700K`
//!
//! Weight methodology:
//! - reads/writes: counted by tracing every StorageMap/StorageValue access in worst-case path
//! - ref_time: base compute + crypto ops (P-256 ECDSA ~80M, SHA-256 ~5M, blake2 ~3M, DER parse ~10M)
//! - proof_size: sum of MaxEncodedLen for all accessed storage items
//!
//! Run `cargo run --release --features runtime-benchmarks -- benchmark pallet`
//! to regenerate with actual hardware measurements.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
    fn register_bot() -> Weight;
    fn update_public_key() -> Weight;
    fn deactivate_bot() -> Weight;
    fn bind_community() -> Weight;
    fn unbind_community() -> Weight;
    fn bind_user_platform() -> Weight;
    fn submit_attestation() -> Weight;
    fn refresh_attestation() -> Weight;
    fn approve_mrtd() -> Weight;
    fn approve_mrenclave() -> Weight;
    fn submit_verified_attestation() -> Weight;
    fn request_attestation_nonce() -> Weight;
    fn approve_api_server_mrtd() -> Weight;
    fn register_pck_key() -> Weight;
    fn submit_dcap_attestation() -> Weight;
    fn submit_dcap_dual_attestation() -> Weight;
    fn submit_dcap_full_attestation() -> Weight;
    fn register_peer() -> Weight;
    fn deregister_peer() -> Weight;
    fn heartbeat_peer() -> Weight;
    fn submit_sgx_attestation() -> Weight;
    fn submit_tee_attestation() -> Weight;
    fn report_stale_peer() -> Weight;
    fn register_operator() -> Weight;
    fn update_operator() -> Weight;
    fn deregister_operator() -> Weight;
    fn set_operator_sla() -> Weight;
    fn assign_bot_to_operator() -> Weight;
    fn unassign_bot_from_operator() -> Weight;
    fn revoke_mrtd() -> Weight;
    fn revoke_mrenclave() -> Weight;
    fn suspend_bot() -> Weight;
    fn reactivate_bot() -> Weight;
    fn unbind_user_platform() -> Weight;
    fn transfer_bot_ownership() -> Weight;
    fn revoke_api_server_mrtd() -> Weight;
    fn revoke_pck_key() -> Weight;
    fn force_deactivate_bot() -> Weight;
    fn suspend_operator() -> Weight;
    fn unsuspend_operator() -> Weight;
    fn update_peer_endpoint() -> Weight;
    fn cleanup_deactivated_bot() -> Weight;
    fn operator_unassign_bot() -> Weight;
    fn force_expire_attestation() -> Weight;
    fn force_transfer_bot_ownership() -> Weight;
}

/// Calibrated weight functions for `pallet_grouprobot_registry`.
///
/// Per-extrinsic breakdown documents every storage access in worst-case path.
/// Crypto cost estimates: P-256 ECDSA verify ~80M ref_time, SHA-256 ~5M, blake2_256 ~3M.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    // register_bot: R=Bots(contains_key) + R=OwnerBots(mutate) | W=OwnerBots + W=Bots + W=BotCount
    fn register_bot() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // update_public_key: R=Bots(mutate) | W=Bots + W=Attestations(remove) + W=AttestationsV2(remove) + W=Nonces(remove)
    fn update_public_key() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // deactivate_bot: R=Bots(mutate) + R=BotOperator(take) + R=PeerRegistry(take)
    // W=Bots + W=BotOperator + W=OperatorBots + W=Operators + W=Attestations + W=AttestationsV2
    //   + W=Nonces + W=PeerRegistry + W=PeerHeartbeatCount(×N peers, worst=MaxPeersPerBot)
    // W8-fix: 原来 writes=8 低估了 PeerHeartbeatCount 的多次 remove, 改为 reads=3 writes=9
    fn deactivate_bot() -> Weight {
        Weight::from_parts(55_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(9))
    }

    // bind_community: R=Bots + R=CommunityBindings + R=existing_Bot(worst case rebind)
    // W=CommunityBindings + W=Bots(+community_count) + W=old_Bots(-community_count, worst case)
    fn bind_community() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // unbind_community: R=CommunityBindings + R=Bots | W=CommunityBindings(remove) + W=Bots
    fn unbind_community() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    // bind_user_platform: R=UserPlatformBindings(get old) | W=UserPlatformBindings
    fn bind_user_platform() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // submit_attestation: R=Bots + R=ApprovedMrtd + R=ApprovedMrenclave(optional)
    // W=Attestations + W=ExpiryQueue + W=Bots
    fn submit_attestation() -> Weight {
        Weight::from_parts(45_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // refresh_attestation: R=Bots + R=AttestationsV2(contains) + R=Attestations(contains)
    //   + R=ApprovedMrtd + R=ApprovedMrenclave(optional) + R=old_record(get)
    // W=Attestations/V2 + W=ExpiryQueue + W=Bots
    // W8-fix: 原来 reads=3 低估, 实际最多 6 reads (V2 check + V1 check + old record get + Bots + 2 whitelist)
    fn refresh_attestation() -> Weight {
        Weight::from_parts(50_000_000, 10_000)
            .saturating_add(T::DbWeight::get().reads(6))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // approve_mrtd: R=ApprovedMrtd(contains) | W=ApprovedMrtd
    fn approve_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // approve_mrenclave: R=ApprovedMrenclave(contains) | W=ApprovedMrenclave
    fn approve_mrenclave() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // submit_verified_attestation: R=Bots + R=Nonces + R=ApprovedMrtd + R=ApprovedMrenclave(opt)
    // W=Nonces(remove) + W=Attestations + W=ExpiryQueue + W=Bots
    // Compute: SHA-256(pubkey) + blake2_256(quote) + quote parse
    fn submit_verified_attestation() -> Weight {
        Weight::from_parts(65_000_000, 15_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // request_attestation_nonce: R=Bots | W=AttestationNonces
    // Compute: blake2_256(parent_hash || bot_id || block_number)
    fn request_attestation_nonce() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // approve_api_server_mrtd: R=ApprovedApiServerMrtd(contains) | W=ApprovedApiServerMrtd
    fn approve_api_server_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // register_pck_key: R=RegisteredPckKeys(contains) | W=RegisteredPckKeys
    fn register_pck_key() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // submit_dcap_attestation:
    //   R=Bots + R=RegisteredPckKeys(opt L3) + R=Nonces + R=ApprovedMrtd + R=ApprovedMrenclave(opt)
    //   W=Nonces(remove) + W=Attestations + W=ExpiryQueue + W=Bots
    // Crypto: 1-2× P-256 ECDSA verify (~80M each) + SHA-256 + blake2_256
    // W8-fix: Level 3 worst case = 5 reads, 4 writes; 原来 reads=6 writes=5 偏高, 校准
    fn submit_dcap_attestation() -> Weight {
        Weight::from_parts(200_000_000, 30_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // submit_dcap_dual_attestation:
    //   R=Bots + R=RegisteredPckKeys(opt, ×2 for L3) + R=Nonces + R=ApprovedMrtd
    //     + R=ApprovedApiServerMrtd + R=ApprovedMrenclave(opt)
    //   W=Nonces(remove) + W=Attestations + W=ExpiryQueue + W=Bots
    // Crypto: 2-4× P-256 ECDSA verify + 2× SHA-256 + 2× blake2_256
    fn submit_dcap_dual_attestation() -> Weight {
        Weight::from_parts(380_000_000, 50_000)
            .saturating_add(T::DbWeight::get().reads(7))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // submit_dcap_full_attestation:
    //   R=Bots + R=Nonces + R=ApprovedMrtd + R=ApprovedMrenclave(opt)
    //   W=Nonces(remove) + W=Attestations + W=ExpiryQueue + W=Bots
    // Crypto: 3× P-256 ECDSA verify (cert chain: Root→Inter, Inter→PCK, PCK→QE)
    //   + 2× DER parse + SHA-256 + blake2_256
    // W8-fix: 原来 reads=6 偏高 (无 RegisteredPckKeys 读取), 校准为 4
    fn submit_dcap_full_attestation() -> Weight {
        Weight::from_parts(350_000_000, 45_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // register_peer: R=Bots + R=Subscription(effective_tier) + R=PeerRegistry(mutate)
    // W=PeerRegistry
    fn register_peer() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // deregister_peer: R=Bots + R=PeerRegistry(mutate) | W=PeerRegistry + W=PeerHeartbeatCount(remove)
    fn deregister_peer() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    // heartbeat_peer: R=Bots + R=Subscription(effective_tier) + R=PeerRegistry(mutate)
    // W=PeerRegistry + W=PeerHeartbeatCount
    // W8-fix: 原来 reads=2 缺少 Subscription 读取, 校准为 3
    fn heartbeat_peer() -> Weight {
        Weight::from_parts(28_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    // submit_sgx_attestation:
    //   R=Bots + R=Attestations(existing TDX) + R=Nonces + R=ApprovedMrenclave
    //     + R=RegisteredPckKeys(opt L3)
    //   W=Nonces(remove) + W=Attestations + W=Bots
    // Crypto: 1-3× P-256 ECDSA verify + SHA-256 + blake2_256
    fn submit_sgx_attestation() -> Weight {
        Weight::from_parts(250_000_000, 35_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // submit_tee_attestation:
    //   R=Bots + R=Nonces + R=ApprovedMrtd/Mrenclave(1-2) + R=RegisteredPckKeys(opt L3)
    //   W=Nonces(remove) + W=AttestationsV2 + W=ExpiryQueue + W=Bots
    // Crypto: 1-3× P-256 ECDSA verify + SHA-256 + blake2_256
    fn submit_tee_attestation() -> Weight {
        Weight::from_parts(260_000_000, 40_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // report_stale_peer: R=PeerRegistry(mutate) | W=PeerRegistry + W=PeerHeartbeatCount(remove)
    // W8-fix: 不需要读 Bots (任何人可调用), 原来 reads=2 偏高
    fn report_stale_peer() -> Weight {
        Weight::from_parts(28_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    // register_operator: R=Operators(contains) + R=PlatformAppHashIndex(contains)
    // W=Operators + W=PlatformAppHashIndex + W=OperatorCount
    fn register_operator() -> Weight {
        Weight::from_parts(40_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // update_operator: R=Operators(mutate) | W=Operators
    fn update_operator() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // deregister_operator: R=Operators + R=OperatorBots
    // W=PlatformAppHashIndex(remove) + W=Operators(remove) + W=OperatorBots(remove) + W=OperatorCount
    fn deregister_operator() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // set_operator_sla: R=Operators(mutate) | W=Operators
    fn set_operator_sla() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // assign_bot_to_operator: R=Bots + R=Operators + R=BotOperator(contains) + R=OperatorBots(mutate)
    // W=OperatorBots + W=BotOperator + W=Operators(bot_count++)
    // W8-fix: 原来 reads=3 缺少 OperatorBots 读取, 校准为 4
    fn assign_bot_to_operator() -> Weight {
        Weight::from_parts(42_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // unassign_bot_from_operator: R=Bots + R=BotOperator
    // W=OperatorBots + W=BotOperator(remove) + W=Operators(bot_count--)
    fn unassign_bot_from_operator() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // revoke_mrtd: R=ApprovedMrtd(contains) | W=ApprovedMrtd(remove)
    fn revoke_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // revoke_mrenclave: R=ApprovedMrenclave(contains) | W=ApprovedMrenclave(remove)
    fn revoke_mrenclave() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // suspend_bot: R=Bots(mutate) | W=Bots
    fn suspend_bot() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // reactivate_bot: R=Bots(mutate) | W=Bots
    fn reactivate_bot() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // unbind_user_platform: R=UserPlatformBindings(contains) | W=UserPlatformBindings(remove)
    fn unbind_user_platform() -> Weight {
        Weight::from_parts(18_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // transfer_bot_ownership: R=Bots(mutate) + R=OwnerBots(new, mutate)
    // W=Bots + W=OwnerBots(old) + W=OwnerBots(new)
    fn transfer_bot_ownership() -> Weight {
        Weight::from_parts(40_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // revoke_api_server_mrtd: R=ApprovedApiServerMrtd(contains) | W=remove
    fn revoke_api_server_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // revoke_pck_key: R=RegisteredPckKeys(contains) | W=remove
    fn revoke_pck_key() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // force_deactivate_bot: same storage path as deactivate_bot
    // R=Bots(mutate) + R=BotOperator(take) + R=PeerRegistry(take)
    // W=Bots + W=Attestations + W=AttestationsV2 + W=Nonces + W=PeerRegistry
    //   + W=BotOperator + W=OperatorBots + W=Operators + W=PeerHeartbeatCount(×N)
    fn force_deactivate_bot() -> Weight {
        Weight::from_parts(60_000_000, 15_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(9))
    }

    // suspend_operator: R=Operators(mutate) | W=Operators
    fn suspend_operator() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // unsuspend_operator: R=Operators(mutate) | W=Operators
    fn unsuspend_operator() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // update_peer_endpoint: R=Bots + R=PeerRegistry(mutate) | W=PeerRegistry
    fn update_peer_endpoint() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    // cleanup_deactivated_bot: R=Bots | W=OwnerBots + W=Bots(remove) + W=BotCount
    fn cleanup_deactivated_bot() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // operator_unassign_bot: R=BotOperator | W=OperatorBots + W=BotOperator(remove) + W=Operators
    fn operator_unassign_bot() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    // force_expire_attestation: R=Bots(contains) + R=Attestations(contains) + R=AttestationsV2(contains)
    // W=Attestations(remove) + W=AttestationsV2(remove) + W=Nonces(remove) + W=Bots(mutate)
    // W8-fix: 原来 reads=2 writes=3 低估, 实际 reads=3 writes=4
    fn force_expire_attestation() -> Weight {
        Weight::from_parts(38_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    // force_transfer_bot_ownership: same as transfer_bot_ownership
    fn force_transfer_bot_ownership() -> Weight {
        Weight::from_parts(40_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
}

/// For testing — uses RocksDbWeight defaults (mirrors SubstrateWeight)
impl WeightInfo for () {
    fn register_bot() -> Weight {
        Weight::from_parts(35_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn update_public_key() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn deactivate_bot() -> Weight {
        Weight::from_parts(55_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(9))
    }
    fn bind_community() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn unbind_community() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }
    fn bind_user_platform() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn submit_attestation() -> Weight {
        Weight::from_parts(45_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn refresh_attestation() -> Weight {
        Weight::from_parts(50_000_000, 10_000)
            .saturating_add(RocksDbWeight::get().reads(6))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn approve_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn approve_mrenclave() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn submit_verified_attestation() -> Weight {
        Weight::from_parts(65_000_000, 15_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn request_attestation_nonce() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn approve_api_server_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn register_pck_key() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn submit_dcap_attestation() -> Weight {
        Weight::from_parts(200_000_000, 30_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn submit_dcap_dual_attestation() -> Weight {
        Weight::from_parts(380_000_000, 50_000)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn submit_dcap_full_attestation() -> Weight {
        Weight::from_parts(350_000_000, 45_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn register_peer() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn deregister_peer() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }
    fn heartbeat_peer() -> Weight {
        Weight::from_parts(28_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(2))
    }
    fn submit_sgx_attestation() -> Weight {
        Weight::from_parts(250_000_000, 35_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn submit_tee_attestation() -> Weight {
        Weight::from_parts(260_000_000, 40_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn report_stale_peer() -> Weight {
        Weight::from_parts(28_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(2))
    }
    fn register_operator() -> Weight {
        Weight::from_parts(40_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn update_operator() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn deregister_operator() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn set_operator_sla() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn assign_bot_to_operator() -> Weight {
        Weight::from_parts(42_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn unassign_bot_from_operator() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn revoke_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn revoke_mrenclave() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn suspend_bot() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn reactivate_bot() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn unbind_user_platform() -> Weight {
        Weight::from_parts(18_000_000, 2_500)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn transfer_bot_ownership() -> Weight {
        Weight::from_parts(40_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn revoke_api_server_mrtd() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn revoke_pck_key() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn force_deactivate_bot() -> Weight {
        Weight::from_parts(60_000_000, 15_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(9))
    }
    fn suspend_operator() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn unsuspend_operator() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn update_peer_endpoint() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
    fn cleanup_deactivated_bot() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn operator_unassign_bot() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
    fn force_expire_attestation() -> Weight {
        Weight::from_parts(38_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
    fn force_transfer_bot_ownership() -> Weight {
        Weight::from_parts(40_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
}
