use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

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

fn mrtd(n: u8) -> [u8; 48] {
	let mut m = [0u8; 48];
	m[0] = n;
	m
}

fn mrenclave(n: u8) -> [u8; 32] {
	let mut m = [0u8; 32];
	m[0] = n;
	m
}

fn pk(n: u8) -> [u8; 32] {
	let mut k = [0u8; 32];
	k[0] = n;
	k
}

// ============================================================================
// register_bot
// ============================================================================

#[test]
fn register_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(bot.owner, OWNER);
		assert_eq!(bot.public_key, pk(1));
		assert_eq!(bot.status, BotStatus::Active);
		assert!(matches!(bot.node_type, NodeType::StandardNode));
		assert_eq!(BotCount::<Test>::get(), 1);
		assert_eq!(OwnerBots::<Test>::get(OWNER).len(), 1);
	});
}

#[test]
fn register_bot_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(2)),
			Error::<Test>::BotAlreadyRegistered
		);
	});
}

#[test]
fn register_bot_fails_max_reached() {
	new_test_ext().execute_with(|| {
		for i in 0..5 {
			assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(i), pk(i)));
		}
		assert_noop!(
			GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(10), pk(10)),
			Error::<Test>::MaxBotsReached
		);
	});
}

// ============================================================================
// update_public_key
// ============================================================================

#[test]
fn update_public_key_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::update_public_key(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(2)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().public_key, pk(2));
	});
}

#[test]
fn update_public_key_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::update_public_key(RuntimeOrigin::signed(OTHER), bot_hash(1), pk(2)),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn update_public_key_fails_same_key() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::update_public_key(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)),
			Error::<Test>::SamePublicKey
		);
	});
}

// ============================================================================
// deactivate_bot
// ============================================================================

#[test]
fn deactivate_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().status, BotStatus::Deactivated);
	});
}

#[test]
fn deactivate_bot_fails_already() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_noop!(
			GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			Error::<Test>::BotAlreadyDeactivated
		);
	});
}

// ============================================================================
// bind_community / unbind_community
// ============================================================================

#[test]
fn bind_community_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::bind_community(
			RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Telegram
		));
		assert!(CommunityBindings::<Test>::contains_key(community_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().community_count, 1);
	});
}

#[test]
fn bind_community_fails_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_noop!(
			GroupRobotRegistry::bind_community(
				RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Telegram
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn bind_community_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::bind_community(
			RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Telegram
		));
		assert_noop!(
			GroupRobotRegistry::bind_community(
				RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Discord
			),
			Error::<Test>::CommunityAlreadyBound
		);
	});
}

#[test]
fn unbind_community_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::bind_community(
			RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Telegram
		));
		assert_ok!(GroupRobotRegistry::unbind_community(RuntimeOrigin::signed(OWNER), community_hash(1)));
		assert!(!CommunityBindings::<Test>::contains_key(community_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().community_count, 0);
	});
}

#[test]
fn unbind_community_fails_not_bound() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::unbind_community(RuntimeOrigin::signed(OWNER), community_hash(99)),
			Error::<Test>::CommunityNotBound
		);
	});
}

// ============================================================================
// bind_user_platform
// ============================================================================

#[test]
fn bind_user_platform_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::bind_user_platform(
			RuntimeOrigin::signed(OWNER), Platform::Discord, pk(42)
		));
		assert_eq!(UserPlatformBindings::<Test>::get(OWNER, Platform::Discord), Some(pk(42)));
	});
}

// ============================================================================
// submit_attestation / refresh_attestation
// ============================================================================

#[test]
fn submit_attestation_works() {
	new_test_ext().execute_with(|| {
		// Setup whitelist
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));

		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			[1u8; 32], // tdx_quote_hash
			Some([2u8; 32]), // sgx_quote_hash
			mrtd(1),
			Some(mrenclave(1)),
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert!(record.is_dual_attestation);
		assert_eq!(record.expires_at, 1 + 100); // block 1 + validity 100

		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert!(matches!(bot.node_type, NodeType::TeeNode { .. }));
	});
}

#[test]
fn submit_attestation_fails_mrtd_not_approved() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::submit_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				[1u8; 32], None, mrtd(1), None,
			),
			Error::<Test>::MrtdNotApproved
		);
	});
}

#[test]
fn submit_attestation_fails_mrenclave_not_approved() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::submit_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				[1u8; 32], Some([2u8; 32]), mrtd(1), Some(mrenclave(99)),
			),
			Error::<Test>::MrenclaveNotApproved
		);
	});
}

#[test]
fn refresh_attestation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		// Advance and refresh
		System::set_block_number(50);
		assert_ok!(GroupRobotRegistry::refresh_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[3u8; 32], None, mrtd(1), None,
		));
		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.attested_at, 50);
		assert_eq!(record.expires_at, 150);
	});
}

#[test]
fn refresh_attestation_fails_no_existing() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::refresh_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				[1u8; 32], None, mrtd(1), None,
			),
			Error::<Test>::AttestationNotFound
		);
	});
}

// ============================================================================
// submit_verified_attestation (P0a: 防 MRTD 伪造)
// ============================================================================

/// 构造模拟 TDX Quote: 在正确偏移量填入 mrtd 和 report_data
/// report_data[0..32] = SHA256(public_key), report_data[32..64] = nonce
fn build_fake_tdx_quote(mrtd_val: &[u8; 48], public_key: &[u8; 32], nonce: &[u8; 32]) -> Vec<u8> {
	use sp_core::hashing::sha2_256;
	let mut quote = vec![0u8; crate::TDX_MIN_QUOTE_LEN + 64];
	quote[crate::TDX_MRTD_OFFSET..crate::TDX_MRTD_OFFSET + 48].copy_from_slice(mrtd_val);
	let hash = sha2_256(public_key);
	quote[crate::TDX_REPORTDATA_OFFSET..crate::TDX_REPORTDATA_OFFSET + 32].copy_from_slice(&hash);
	quote[crate::TDX_REPORTDATA_OFFSET + 32..crate::TDX_REPORTDATA_OFFSET + 64].copy_from_slice(nonce);
	quote
}

/// 请求 nonce 并返回其值
fn request_nonce(owner: u64, bot: BotIdHash) -> [u8; 32] {
	assert_ok!(GroupRobotRegistry::request_attestation_nonce(RuntimeOrigin::signed(owner), bot));
	let (nonce, _) = AttestationNonces::<Test>::get(bot).unwrap();
	nonce
}

#[test]
fn submit_verified_attestation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_verified_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			bounded,
			None,
			None,
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert!(record.quote_verified);
		assert_eq!(record.mrtd, mrtd(1));
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		// nonce 应被消费
		assert!(AttestationNonces::<Test>::get(bot_hash(1)).is_none());
	});
}

#[test]
fn submit_verified_attestation_fails_spoofed_mrtd() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let quote = build_fake_tdx_quote(&mrtd(99), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::MrtdNotApproved
		);
	});
}

#[test]
fn submit_verified_attestation_fails_report_data_mismatch() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(99), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::QuoteReportDataMismatch
		);
	});
}

#[test]
fn submit_verified_attestation_fails_quote_too_short() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		let _nonce = request_nonce(OWNER, bot_hash(1));

		let short_quote = vec![0u8; 100];
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			short_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::QuoteTooShort
		);
	});
}

#[test]
fn submit_verified_fails_nonce_missing() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 未请求 nonce 就提交
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &[0u8; 32]);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::NonceMissing
		);
	});
}

#[test]
fn submit_verified_fails_nonce_mismatch_replay() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let _nonce = request_nonce(OWNER, bot_hash(1));
		// 提交带错误 nonce 的 quote (模拟重放旧 quote)
		let wrong_nonce = [0xFFu8; 32];
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &wrong_nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::NonceMismatch
		);
	});
}

#[test]
fn submit_verified_fails_nonce_expired() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		// 前进超过 AttestationValidityBlocks (100)
		System::set_block_number(200);

		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::NonceExpired
		);
	});
}

#[test]
fn nonce_consumed_after_use() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_verified_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded.clone(), None, None,
		));

		// 重放同一 quote → NonceMissing (已被消费)
		assert_noop!(
			GroupRobotRegistry::submit_verified_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::NonceMissing
		);
	});
}

#[test]
fn old_submit_attestation_sets_quote_verified_false() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert!(!record.quote_verified, "old extrinsic must set quote_verified=false");
	});
}

// ============================================================================
// approve_mrtd / approve_mrenclave
// ============================================================================

#[test]
fn approve_mrtd_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_eq!(ApprovedMrtd::<Test>::get(mrtd(1)), Some(1));
	});
}

#[test]
fn approve_mrtd_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_noop!(
			GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 2),
			Error::<Test>::MrtdAlreadyApproved
		);
	});
}

#[test]
fn approve_mrtd_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::approve_mrtd(RuntimeOrigin::signed(OWNER), mrtd(1), 1),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn approve_mrenclave_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_eq!(ApprovedMrenclave::<Test>::get(mrenclave(1)), Some(1));
	});
}

// ============================================================================
// Helper functions
// ============================================================================

#[test]
fn helper_is_bot_active() {
	new_test_ext().execute_with(|| {
		assert!(!GroupRobotRegistry::is_bot_active(&bot_hash(1)));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert!(GroupRobotRegistry::is_bot_active(&bot_hash(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert!(!GroupRobotRegistry::is_bot_active(&bot_hash(1)));
	});
}

#[test]
fn helper_is_tee_node() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert!(!GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));
	});
}

#[test]
fn helper_has_dual_attestation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// Single attestation
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert!(!GroupRobotRegistry::has_dual_attestation(&bot_hash(1)));

		// Dual attestation
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], Some([2u8; 32]), mrtd(1), Some(mrenclave(1)),
		));
		assert!(GroupRobotRegistry::has_dual_attestation(&bot_hash(1)));
	});
}

#[test]
fn helper_is_attestation_fresh() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert!(GroupRobotRegistry::is_attestation_fresh(&bot_hash(1)));

		// Advance past expiry
		System::set_block_number(200);
		assert!(!GroupRobotRegistry::is_attestation_fresh(&bot_hash(1)));
	});
}

// ============================================================================
// on_initialize: attestation expiry
// ============================================================================

#[test]
fn attestation_expires_on_initialize() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));

		// expires_at = 1 + 100 = 101. Advance to block 110 (interval=10, so check at 110)
		advance_to(110);

		// Should be expired and downgraded
		assert!(!Attestations::<Test>::contains_key(bot_hash(1)));
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert!(matches!(bot.node_type, NodeType::StandardNode));
	});
}

#[test]
fn attestation_not_expired_before_time() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		// Advance to block 50 (interval=10 checks at 10,20,30,40,50)
		advance_to(50);
		assert!(Attestations::<Test>::contains_key(bot_hash(1)));
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));
	});
}

// ============================================================================
// bot_owner / bot_public_key
// ============================================================================

#[test]
fn bot_owner_and_public_key() {
	new_test_ext().execute_with(|| {
		assert_eq!(GroupRobotRegistry::bot_owner(&bot_hash(1)), None);
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(GroupRobotRegistry::bot_owner(&bot_hash(1)), Some(OWNER));
		assert_eq!(GroupRobotRegistry::bot_public_key(&bot_hash(1)), Some(pk(1)));
	});
}

// ============================================================================
// BotRegistryProvider trait impl
// ============================================================================

#[test]
fn bot_registry_provider_trait() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::BotRegistryProvider;
		assert!(!<GroupRobotRegistry as BotRegistryProvider<u64>>::is_bot_active(&bot_hash(1)));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert!(<GroupRobotRegistry as BotRegistryProvider<u64>>::is_bot_active(&bot_hash(1)));
		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::bot_owner(&bot_hash(1)),
			Some(OWNER)
		);
	});
}
