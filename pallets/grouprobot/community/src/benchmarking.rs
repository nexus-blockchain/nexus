//! Benchmarking setup for pallet-grouprobot-community
//!
//! Every benchmark seeds only this pallet's own storage items.
//! External trait dependencies (`BotRegistry`, `Subscription`) are satisfied by
//! the runtime-level benchmark configuration (or mock implementations in tests).
//!
//! For extrinsics that require Ed25519 signature verification (`submit_action_log`,
//! `batch_submit_logs`), the `BenchmarkHelper` trait must be implemented by the
//! runtime to provide valid signatures matching the `BotRegistry::bot_public_key`.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as GroupRobotCommunity;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet_grouprobot_primitives::*;

/// Helper: create a community_id_hash where `hash[0] = tag`.
/// The mock routes `hash[0]` to decide tier/owner/active status.
fn community_hash(tag: u8) -> CommunityIdHash {
    let mut h = [0u8; 32];
    h[0] = tag;
    h
}

/// Helper: insert a default active CommunityConfig into storage.
fn seed_community_config<T: Config>(community_id_hash: &CommunityIdHash) {
    CommunityConfigs::<T>::insert(
        community_id_hash,
        CommunityConfig {
            node_requirement: NodeRequirement::TeeOnly,
            anti_flood_enabled: false,
            flood_limit: 10,
            warn_limit: 3,
            warn_action: WarnAction::Kick,
            welcome_enabled: false,
            ads_enabled: false,
            active_members: 0,
            language: *b"en",
            version: 0,
            status: CommunityStatus::Active,
        },
    );
}

/// Helper: insert a banned CommunityConfig.
fn seed_banned_community_config<T: Config>(community_id_hash: &CommunityIdHash) {
    CommunityConfigs::<T>::insert(
        community_id_hash,
        CommunityConfig {
            node_requirement: NodeRequirement::TeeOnly,
            anti_flood_enabled: false,
            flood_limit: 10,
            warn_limit: 3,
            warn_action: WarnAction::Kick,
            welcome_enabled: false,
            ads_enabled: false,
            active_members: 0,
            language: *b"en",
            version: 0,
            status: CommunityStatus::Banned,
        },
    );
}

/// Helper: seed a single ActionLog directly (bypass signature verification).
fn seed_action_log<T: Config>(
    community_id_hash: &CommunityIdHash,
    operator: &T::AccountId,
    sequence: u64,
    block_number: BlockNumberFor<T>,
) {
    ActionLogs::<T>::mutate(community_id_hash, |logs| {
        let log = ActionLog::<T> {
            community_id_hash: *community_id_hash,
            action_type: ActionType::Kick,
            operator: operator.clone(),
            target_hash: [0u8; 32],
            sequence,
            message_hash: [0u8; 32],
            signature: [0u8; 64],
            block_number,
        };
        let _ = logs.try_push(log);
    });
    LastSequence::<T>::insert(community_id_hash, sequence);
}

/// Helper: seed a reputation cooldown entry.
fn seed_cooldown<T: Config>(
    operator: &T::AccountId,
    community_id_hash: &CommunityIdHash,
    user_hash: &[u8; 32],
    block: BlockNumberFor<T>,
) {
    ReputationCooldowns::<T>::insert((operator, community_id_hash, user_hash), block);
}

/// Helper: seed a MemberReputation entry.
fn seed_reputation<T: Config>(
    community_id_hash: &CommunityIdHash,
    user_hash: &[u8; 32],
    score: i64,
) {
    MemberReputation::<T>::insert(
        community_id_hash,
        user_hash,
        ReputationRecord::<T> {
            score,
            awards: 1,
            deductions: 0,
            last_updated: frame_system::Pallet::<T>::block_number(),
        },
    );
}

/// Trait that the runtime must implement for benchmarks requiring Ed25519 signatures
/// or external-pallet state setup.
///
/// This allows the runtime to provide valid Ed25519 signatures that match the
/// `BotRegistry::bot_public_key` for a given community, and to set up any
/// external pallet state (bot registration, subscription, etc.) needed for
/// the benchmark extrinsics to pass their guard checks.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
    /// Return a valid Ed25519 signature for the given action-log message components,
    /// signed with the private key whose public key is returned by
    /// `BotRegistry::bot_public_key(community_id_hash)`.
    fn sign_action_log(
        community_id_hash: &CommunityIdHash,
        action_type: &ActionType,
        target_hash: &[u8; 32],
        sequence: u64,
        message_hash: &[u8; 32],
    ) -> [u8; 64];

    /// Set up all external-pallet state so that:
    /// - `caller` is the bot owner for `community_id_hash`
    /// - The bot is active
    /// - The subscription tier for `community_id_hash` is paid (Basic or above)
    ///
    /// This seeds BotRegistry + Subscription pallets; the community pallet's own
    /// storage is seeded separately by each benchmark.
    fn setup_active_paid_community(caller: &AccountId, community_id_hash: &CommunityIdHash);

    /// Same as `setup_active_paid_community` but only requires bot-owner status
    /// (no active-bot or paid-tier requirement). Used for `update_community_config`
    /// and `delete_community_config`.
    fn setup_bot_owner(caller: &AccountId, community_id_hash: &CommunityIdHash);
}

/// Fallback implementation for any runtime (uses `sp_io` host functions).
#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
    fn sign_action_log(
        community_id_hash: &CommunityIdHash,
        action_type: &ActionType,
        target_hash: &[u8; 32],
        sequence: u64,
        message_hash: &[u8; 32],
    ) -> [u8; 64] {
        use codec::Encode;
        use sp_core::ed25519;

        let seed = alloc::vec![1u8; 32];
        let mut msg = alloc::vec::Vec::with_capacity(32 + 4 + 32 + 8 + 32);
        msg.extend_from_slice(community_id_hash);
        msg.extend_from_slice(&action_type.encode());
        msg.extend_from_slice(target_hash);
        msg.extend_from_slice(&sequence.to_le_bytes());
        msg.extend_from_slice(message_hash);

        let raw_pub = sp_io::crypto::ed25519_generate(sp_core::testing::ED25519, Some(seed));
        let public = ed25519::Public::from_raw(raw_pub.0);
        let sig = sp_io::crypto::ed25519_sign(sp_core::testing::ED25519, &public, &msg)
            .expect("ed25519_sign should succeed in benchmark");
        sig.0
    }

    fn setup_active_paid_community(_caller: &AccountId, _community_id_hash: &CommunityIdHash) {
        // Default no-op; override in runtime-specific impls if needed.
    }

    fn setup_bot_owner(_caller: &AccountId, _community_id_hash: &CommunityIdHash) {
        // Default no-op; override in runtime-specific impls if needed.
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ========================================================================
    // submit_action_log (call_index 0)
    //   Requires: active bot owner, not banned, paid tier, monotonic sequence,
    //             valid Ed25519 signature.
    // ========================================================================
    #[benchmark]
    fn submit_action_log() {
        // community_hash(1) => Basic tier, active bot, owner = whitelisted_caller (in mock: OWNER=1)
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);

        let action_type = ActionType::Kick;
        let target_hash = [0xABu8; 32];
        let sequence = 1u64;
        let message_hash = [0xCDu8; 32];
        let signature = T::BenchmarkHelper::sign_action_log(
            &cid,
            &action_type,
            &target_hash,
            sequence,
            &message_hash,
        );

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            cid,
            action_type,
            target_hash,
            sequence,
            message_hash,
            signature,
        );

        assert_eq!(ActionLogs::<T>::get(&cid).len(), 1);
        assert_eq!(LastSequence::<T>::get(&cid), Some(1));
    }

    // ========================================================================
    // set_node_requirement (call_index 1)
    //   Requires: active bot owner, not banned, community config exists,
    //             new requirement differs from current.
    // ========================================================================
    #[benchmark]
    fn set_node_requirement() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);
        // Default config has TeeOnly; set to Any.

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, NodeRequirement::Any);

        assert_eq!(
            CommunityConfigs::<T>::get(&cid).unwrap().node_requirement,
            NodeRequirement::Any
        );
    }

    // ========================================================================
    // update_community_config (call_index 2)
    //   Requires: bot owner, valid language, CAS version match.
    //   Note: uses ensure_bot_owner (not ensure_active_bot_owner).
    // ========================================================================
    #[benchmark]
    fn update_community_config() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_bot_owner(&caller, &cid);
        seed_community_config::<T>(&cid);
        // Seeded config has version=0.

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            cid,
            0u32,  // expected_version
            true,  // anti_flood_enabled
            20u16, // flood_limit
            5u8,   // warn_limit
            WarnAction::Ban,
            true,   // welcome_enabled
            true,   // ads_enabled
            *b"zh", // language
        );

        let config = CommunityConfigs::<T>::get(&cid).unwrap();
        assert_eq!(config.version, 1);
        assert!(config.anti_flood_enabled);
    }

    // ========================================================================
    // batch_submit_logs (call_index 3)
    //   Requires: active bot owner, not banned, paid tier, non-empty batch,
    //             monotonic sequences, valid Ed25519 signatures.
    //   Weight is linear in `n`.
    // ========================================================================
    #[benchmark]
    fn batch_submit_logs(n: Linear<1, 10>) {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);

        let target_hash = [0xABu8; 32];
        let message_hash = [0xCDu8; 32];
        let action_type = ActionType::Kick;

        let mut logs_vec = alloc::vec::Vec::new();
        for i in 1..=n {
            let seq = i as u64;
            let sig = T::BenchmarkHelper::sign_action_log(
                &cid,
                &action_type,
                &target_hash,
                seq,
                &message_hash,
            );
            logs_vec.push((action_type.clone(), target_hash, seq, message_hash, sig));
        }
        let logs: BoundedVec<_, T::MaxBatchSize> = logs_vec.try_into().expect("n <= MaxBatchSize");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, logs);

        assert_eq!(ActionLogs::<T>::get(&cid).len(), n as usize);
    }

    // ========================================================================
    // clear_expired_logs (call_index 4)
    //   Requires: signed, max_age > 0, tier gate retention check, logs older
    //             than max_age exist.
    //   We use community_hash(1) = Basic tier (log_retention_days=30).
    // ========================================================================
    #[benchmark]
    fn clear_expired_logs() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);

        // Seed a log at block 1 (old).
        seed_action_log::<T>(&cid, &caller, 1, 1u32.into());

        // Advance block number far enough to exceed Basic tier retention
        // (30 days * BlocksPerDay blocks) + some margin.
        let blocks_per_day: u32 = T::BlocksPerDay::get();
        let retention_blocks: u32 = 30u32.saturating_mul(blocks_per_day);
        let future_block: u32 = retention_blocks.saturating_add(100);
        frame_system::Pallet::<T>::set_block_number(future_block.into());

        let max_age: BlockNumberFor<T> = retention_blocks.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, max_age);

        assert_eq!(ActionLogs::<T>::get(&cid).len(), 0);
    }

    // ========================================================================
    // award_reputation (call_index 5)
    //   Requires: active bot owner, not banned, paid tier, delta > 0,
    //             delta <= MaxReputationDelta, cooldown passed.
    // ========================================================================
    #[benchmark]
    fn award_reputation() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);
        let user_hash = [0xFFu8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, user_hash, 10u32);

        assert_eq!(MemberReputation::<T>::get(&cid, &user_hash).score, 10);
    }

    // ========================================================================
    // deduct_reputation (call_index 6)
    //   Same guards as award_reputation.
    // ========================================================================
    #[benchmark]
    fn deduct_reputation() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);
        let user_hash = [0xFFu8; 32];
        // Seed initial reputation so deduction result is clear.
        seed_reputation::<T>(&cid, &user_hash, 50);

        // Advance past cooldown so we can call from same operator.
        let cooldown_blocks = T::ReputationCooldown::get();
        let now = frame_system::Pallet::<T>::block_number();
        frame_system::Pallet::<T>::set_block_number(now + cooldown_blocks + 1u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, user_hash, 5u32);

        assert_eq!(MemberReputation::<T>::get(&cid, &user_hash).score, 45);
    }

    // ========================================================================
    // reset_reputation (call_index 7)
    //   Requires: active bot owner.
    // ========================================================================
    #[benchmark]
    fn reset_reputation() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);
        let user_hash = [0xFFu8; 32];
        seed_reputation::<T>(&cid, &user_hash, 100);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, user_hash);

        assert_eq!(MemberReputation::<T>::get(&cid, &user_hash).score, 0);
    }

    // ========================================================================
    // update_active_members (call_index 8)
    //   Requires: active bot owner, community config exists.
    // ========================================================================
    #[benchmark]
    fn update_active_members() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_active_paid_community(&caller, &cid);
        seed_community_config::<T>(&cid);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid, 42u32);

        assert_eq!(CommunityConfigs::<T>::get(&cid).unwrap().active_members, 42);
    }

    // ========================================================================
    // cleanup_expired_cooldowns (call_index 9)
    //   Requires: signed, cooldown entry exists and is expired.
    // ========================================================================
    #[benchmark]
    fn cleanup_expired_cooldowns() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        let operator: T::AccountId = account("operator", 0, 0);
        let user_hash = [0xFFu8; 32];

        // Seed a cooldown at block 1.
        seed_cooldown::<T>(&operator, &cid, &user_hash, 1u32.into());

        // Advance block past cooldown period.
        let cooldown_blocks = T::ReputationCooldown::get();
        frame_system::Pallet::<T>::set_block_number(cooldown_blocks + 2u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), operator, cid, user_hash);

        assert_eq!(
            ReputationCooldowns::<T>::get((
                &account::<T::AccountId>("operator", 0, 0),
                &cid,
                &user_hash
            )),
            BlockNumberFor::<T>::default()
        );
    }

    // ========================================================================
    // delete_community_config (call_index 10)
    //   Requires: bot owner, config exists.
    // ========================================================================
    #[benchmark]
    fn delete_community_config() {
        let cid = community_hash(1);
        let caller: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::setup_bot_owner(&caller, &cid);
        seed_community_config::<T>(&cid);

        // Also seed some logs and reputation to test cleanup path.
        seed_action_log::<T>(&cid, &caller, 1, 1u32.into());
        seed_reputation::<T>(&cid, &[0xFFu8; 32], 10);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), cid);

        assert!(!CommunityConfigs::<T>::contains_key(&cid));
        assert_eq!(ActionLogs::<T>::get(&cid).len(), 0);
    }

    // ========================================================================
    // force_remove_community (call_index 11)
    //   Requires: Root.
    // ========================================================================
    #[benchmark]
    fn force_remove_community() {
        let cid = community_hash(1);
        seed_community_config::<T>(&cid);
        let dummy: T::AccountId = account("dummy", 0, 0);
        seed_action_log::<T>(&cid, &dummy, 1, 1u32.into());
        seed_reputation::<T>(&cid, &[0xFFu8; 32], 10);

        #[extrinsic_call]
        _(RawOrigin::Root, cid);

        assert!(!CommunityConfigs::<T>::contains_key(&cid));
        assert_eq!(ActionLogs::<T>::get(&cid).len(), 0);
    }

    // ========================================================================
    // ban_community (call_index 12)
    //   Requires: Root, config exists, not already banned.
    // ========================================================================
    #[benchmark]
    fn ban_community() {
        let cid = community_hash(1);
        seed_community_config::<T>(&cid); // status = Active

        #[extrinsic_call]
        _(RawOrigin::Root, cid);

        assert_eq!(
            CommunityConfigs::<T>::get(&cid).unwrap().status,
            CommunityStatus::Banned
        );
    }

    // ========================================================================
    // unban_community (call_index 13)
    //   Requires: Root, config exists, currently banned.
    // ========================================================================
    #[benchmark]
    fn unban_community() {
        let cid = community_hash(1);
        seed_banned_community_config::<T>(&cid);

        #[extrinsic_call]
        _(RawOrigin::Root, cid);

        assert_eq!(
            CommunityConfigs::<T>::get(&cid).unwrap().status,
            CommunityStatus::Active
        );
    }

    // ========================================================================
    // force_update_community_config (call_index 14)
    //   Requires: Root, valid language.
    // ========================================================================
    #[benchmark]
    fn force_update_community_config() {
        let cid = community_hash(1);
        seed_community_config::<T>(&cid);

        #[extrinsic_call]
        _(
            RawOrigin::Root,
            cid,
            true,  // anti_flood_enabled
            50u16, // flood_limit
            10u8,  // warn_limit
            WarnAction::Mute,
            true,   // welcome_enabled
            false,  // ads_enabled
            *b"de", // language
        );

        let config = CommunityConfigs::<T>::get(&cid).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.language, *b"de");
    }

    // ========================================================================
    // force_reset_community_reputation (call_index 15)
    //   Requires: Root, config exists.
    // ========================================================================
    #[benchmark]
    fn force_reset_community_reputation() {
        let cid = community_hash(1);
        seed_community_config::<T>(&cid);
        // Seed several reputation entries.
        for i in 0u8..5 {
            let mut uh = [0u8; 32];
            uh[0] = i;
            seed_reputation::<T>(&cid, &uh, (i as i64) * 10);
        }

        #[extrinsic_call]
        _(RawOrigin::Root, cid);

        // After force reset, all reputation should be cleared.
        for i in 0u8..5 {
            let mut uh = [0u8; 32];
            uh[0] = i;
            assert_eq!(MemberReputation::<T>::get(&cid, &uh).score, 0);
        }
    }

    impl_benchmark_test_suite!(
        GroupRobotCommunity,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
