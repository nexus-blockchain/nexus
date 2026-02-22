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
		// C1-fix: 无 ECDSA 签名验证 → quote_verified=false, dcap_level=1
		assert!(!record.quote_verified);
		assert_eq!(record.dcap_level, 1);
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
// DCAP 验证: submit_dcap_attestation (Level 2)
// ============================================================================

/// 构建 DCAP 验证用的 TDX Quote (带真实 P-256 签名)
/// report_data[0..32] = SHA256(bot_public_key), report_data[32..64] = nonce
fn build_dcap_quote(
	mrtd_val: &[u8; 48],
	public_key: &[u8; 32],
	nonce: &[u8; 32],
) -> (Vec<u8>, [u8; 64]) {
	use crate::dcap::test_utils::TestQuoteBuilder;

	let mut rd = [0u8; 64];
	let pk_hash = sp_core::hashing::sha2_256(public_key);
	rd[..32].copy_from_slice(&pk_hash);
	rd[32..64].copy_from_slice(nonce);

	let builder = TestQuoteBuilder::new(1)
		.with_mrtd(*mrtd_val)
		.with_report_data(rd);

	let pck_key = builder.pck_public_key();
	let quote = builder.build();
	(quote, pck_key)
}

#[test]
fn dcap_level2_attestation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_dcap_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			bounded,
			None,
			None, // Level 2 (no platform_id)
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		// X1-fix: Level 2 攻击者控制 AK → quote_verified=false
		assert!(!record.quote_verified);
		assert_eq!(record.dcap_level, 2);
		assert_eq!(record.mrtd, mrtd(1));
		assert!(record.api_server_mrtd.is_none());
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		// nonce 已消费
		assert!(AttestationNonces::<Test>::get(bot_hash(1)).is_none());
	});
}

#[test]
fn dcap_level3_attestation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, pck_key) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);

		// 注册 PCK 公钥
		let platform_id = [0x01u8; 32];
		assert_ok!(GroupRobotRegistry::register_pck_key(
			RuntimeOrigin::root(), platform_id, pck_key,
		));

		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_dcap_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			bounded,
			None,
			Some(platform_id), // Level 3
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.dcap_level, 3);
	});
}

#[test]
fn dcap_rejects_tampered_mrtd() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (mut quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);

		// 篡改 MRTD (在签名之后)
		quote[crate::dcap::MRTD_OFFSET] = 0xFF;

		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_dcap_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::DcapBodySignatureInvalid
		);
	});
}

#[test]
fn dcap_rejects_tampered_report_data() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (mut quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);

		// 篡改 report_data (在签名之后)
		quote[crate::dcap::REPORTDATA_OFFSET] = 0xFF;

		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_dcap_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::DcapBodySignatureInvalid
		);
	});
}

#[test]
fn dcap_rejects_handcrafted_quote() {
	new_test_ext().execute_with(|| {
		// 模拟攻击: 手工构造 Quote (正确偏移量放入合法 MRTD)，但无有效签名
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		// 使用旧的 build_fake_tdx_quote (无 ECDSA 签名，只是字节拼接)
		let fake_quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &nonce);
		// 需要满足 DCAP 最小长度 (1214)
		let mut padded = fake_quote;
		padded.resize(crate::dcap::MIN_DCAP_QUOTE_LEN, 0);

		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			padded.try_into().unwrap();

		// 应被 DCAP 验证拒绝 (Quote 结构无效: header 字段不匹配)
		assert_noop!(
			GroupRobotRegistry::submit_dcap_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::DcapQuoteInvalid
		);
	});
}

#[test]
fn dcap_rejects_wrong_pck_key() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _real_pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);

		// 注册错误的 PCK 公钥
		let platform_id = [0x01u8; 32];
		let wrong_pck = {
			use p256::ecdsa::{SigningKey, VerifyingKey};
			let mut seed = [0u8; 32];
			seed[0] = 0xCC;
			seed[1] = 0xDD;
			let sk = SigningKey::from_slice(&seed).unwrap();
			let vk = VerifyingKey::from(&sk);
			let pt = vk.to_encoded_point(false);
			let mut k = [0u8; 64];
			k.copy_from_slice(&pt.as_bytes()[1..65]);
			k
		};
		assert_ok!(GroupRobotRegistry::register_pck_key(
			RuntimeOrigin::root(), platform_id, wrong_pck,
		));

		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_dcap_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, Some(platform_id),
			),
			Error::<Test>::DcapQeSignatureInvalid
		);
	});
}

#[test]
fn dcap_nonce_consumed_after_use() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.clone().try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_dcap_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded.clone(), None, None,
		));

		// 重放同一 Quote → NonceMissing
		assert_noop!(
			GroupRobotRegistry::submit_dcap_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::NonceMissing
		);
	});
}

// ============================================================================
// DCAP 双 Quote 证明: submit_dcap_dual_attestation
// ============================================================================

/// 构建双 Quote (Bot + API Server)
fn build_dcap_dual_quotes(
	bot_mrtd: &[u8; 48],
	api_mrtd: &[u8; 48],
	public_key: &[u8; 32],
	nonce: &[u8; 32],
) -> (Vec<u8>, Vec<u8>, [u8; 64]) {
	use crate::dcap::test_utils::TestQuoteBuilder;

	let pk_hash = sp_core::hashing::sha2_256(public_key);

	// Bot Quote: report_data = SHA256(pk) || nonce
	let mut bot_rd = [0u8; 64];
	bot_rd[..32].copy_from_slice(&pk_hash);
	bot_rd[32..64].copy_from_slice(nonce);

	let bot_builder = TestQuoteBuilder::new(1)
		.with_mrtd(*bot_mrtd)
		.with_report_data(bot_rd);
	let pck_key = bot_builder.pck_public_key();
	let bot_quote = bot_builder.build();

	// API Server Quote: report_data = SHA256(pk) || zeros (不需要 nonce)
	let mut api_rd = [0u8; 64];
	api_rd[..32].copy_from_slice(&pk_hash);

	let api_builder = TestQuoteBuilder::new(2)
		.with_mrtd(*api_mrtd)
		.with_report_data(api_rd);
	let api_quote = api_builder.build();

	(bot_quote, api_quote, pck_key)
}

#[test]
fn dcap_dual_attestation_works() {
	new_test_ext().execute_with(|| {
		let bot_m = mrtd(1);
		let api_m = mrtd(2);
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), bot_m, 1));
		assert_ok!(GroupRobotRegistry::approve_api_server_mrtd(RuntimeOrigin::root(), api_m, 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (bot_quote, api_quote, _pck) = build_dcap_dual_quotes(&bot_m, &api_m, &pk(1), &nonce);

		let bot_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			bot_quote.try_into().unwrap();
		let api_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			api_quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_dcap_dual_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			bot_bounded,
			api_bounded,
			None,
			None,
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		// X1-fix: Level 2 dual → quote_verified=false
		assert!(!record.quote_verified);
		assert_eq!(record.dcap_level, 2);
		assert!(record.is_dual_attestation);
		assert_eq!(record.mrtd, bot_m);
		assert_eq!(record.api_server_mrtd, Some(api_m));
		assert!(record.api_server_quote_hash.is_some());
	});
}

#[test]
fn dcap_dual_fails_api_server_mrtd_not_approved() {
	new_test_ext().execute_with(|| {
		let bot_m = mrtd(1);
		let api_m = mrtd(2);
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), bot_m, 1));
		// 不审批 API Server MRTD
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (bot_quote, api_quote, _pck) = build_dcap_dual_quotes(&bot_m, &api_m, &pk(1), &nonce);

		let bot_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			bot_quote.try_into().unwrap();
		let api_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			api_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_dcap_dual_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bot_bounded, api_bounded, None, None,
			),
			Error::<Test>::ApiServerMrtdNotApproved
		);
	});
}

#[test]
fn dcap_dual_fails_api_server_report_data_mismatch() {
	new_test_ext().execute_with(|| {
		use crate::dcap::test_utils::TestQuoteBuilder;

		let bot_m = mrtd(1);
		let api_m = mrtd(2);
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), bot_m, 1));
		assert_ok!(GroupRobotRegistry::approve_api_server_mrtd(RuntimeOrigin::root(), api_m, 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));

		// Bot Quote: 正确绑定
		let pk_hash = sp_core::hashing::sha2_256(&pk(1));
		let mut bot_rd = [0u8; 64];
		bot_rd[..32].copy_from_slice(&pk_hash);
		bot_rd[32..64].copy_from_slice(&nonce);
		let bot_builder = TestQuoteBuilder::new(1).with_mrtd(bot_m).with_report_data(bot_rd);
		let bot_quote = bot_builder.build();

		// API Server Quote: 绑定到不同公钥
		let wrong_pk_hash = sp_core::hashing::sha2_256(&pk(99));
		let mut api_rd = [0u8; 64];
		api_rd[..32].copy_from_slice(&wrong_pk_hash);
		let api_builder = TestQuoteBuilder::new(2).with_mrtd(api_m).with_report_data(api_rd);
		let api_quote = api_builder.build();

		let bot_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			bot_quote.try_into().unwrap();
		let api_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			api_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_dcap_dual_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bot_bounded, api_bounded, None, None,
			),
			Error::<Test>::ApiServerReportDataMismatch
		);
	});
}

// ============================================================================
// approve_api_server_mrtd
// ============================================================================

#[test]
fn approve_api_server_mrtd_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_api_server_mrtd(
			RuntimeOrigin::root(), mrtd(10), 1,
		));
		assert_eq!(ApprovedApiServerMrtd::<Test>::get(mrtd(10)), Some(1));
	});
}

#[test]
fn approve_api_server_mrtd_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_api_server_mrtd(
			RuntimeOrigin::root(), mrtd(10), 1,
		));
		assert_noop!(
			GroupRobotRegistry::approve_api_server_mrtd(RuntimeOrigin::root(), mrtd(10), 2),
			Error::<Test>::ApiServerMrtdAlreadyApproved
		);
	});
}

// ============================================================================
// register_pck_key
// ============================================================================

#[test]
fn register_pck_key_works() {
	new_test_ext().execute_with(|| {
		let pid = [0x01u8; 32];
		let pck = [0xAAu8; 64];
		assert_ok!(GroupRobotRegistry::register_pck_key(
			RuntimeOrigin::root(), pid, pck,
		));
		let (stored_key, _) = RegisteredPckKeys::<Test>::get(pid).unwrap();
		assert_eq!(stored_key, pck);
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

// ============================================================================
// Audit Regression Tests
// ============================================================================

/// H1-fix: 公钥轮换必须使已有 Attestation 失效
#[test]
fn key_rotation_invalidates_attestation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 完成证明
		let nonce = request_nonce(OWNER, bot_hash(1));
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_verified_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
		));
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		assert!(Attestations::<Test>::get(bot_hash(1)).is_some());

		// 轮换公钥
		assert_ok!(GroupRobotRegistry::update_public_key(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(2),
		));

		// Attestation 必须被清除, NodeType 重置
		assert!(!GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		assert!(Attestations::<Test>::get(bot_hash(1)).is_none());
		assert!(AttestationNonces::<Test>::get(bot_hash(1)).is_none());
	});
}

/// M1-fix: 停用的 Bot 不能提交 Attestation
#[test]
fn deactivated_bot_cannot_attest() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 停用 Bot
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));

		// 尝试请求 nonce → 失败
		assert_noop!(
			GroupRobotRegistry::request_attestation_nonce(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			Error::<Test>::BotNotActive
		);
	});
}

/// M1-fix: 停用的 Bot 不能提交 DCAP Attestation
#[test]
fn deactivated_bot_cannot_dcap_attest() {
	new_test_ext().execute_with(|| {
		use crate::dcap::test_utils::TestQuoteBuilder;

		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 停用 Bot
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));

		// 构建 Quote
		let mut rd = [0u8; 64];
		let pk_hash = sp_core::hashing::sha2_256(&pk(1));
		rd[..32].copy_from_slice(&pk_hash);
		let builder = TestQuoteBuilder::new(1).with_mrtd(mrtd(1)).with_report_data(rd);
		let quote = builder.build();
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		// 尝试提交 DCAP Attestation → 失败
		assert_noop!(
			GroupRobotRegistry::submit_dcap_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
			),
			Error::<Test>::BotNotActive
		);
	});
}

/// H2-fix: DER 解析器对恶意输入不 panic, 而是返回错误
#[test]
fn der_parser_handles_malicious_lengths() {
	use crate::dcap::*;

	// 构造一个 DER 证书，TBS 长度字段声称 65535 但实际很短
	let malicious_cert: Vec<u8> = vec![
		0x30, 0x82, 0x00, 0x20, // outer SEQUENCE, len=32
		0x30, 0x82, 0xFF, 0xFF, // TBS SEQUENCE, len=65535 (恶意!)
		0x00, 0x00, 0x00, 0x00,
		0x00, 0x00, 0x00, 0x00,
		0x00, 0x00, 0x00, 0x00,
		0x00, 0x00, 0x00, 0x00,
		0x00, 0x00, 0x00, 0x00,
		0x00, 0x00, 0x00, 0x00,
		0x00, 0x00, 0x00, 0x00,
	];

	// 不应 panic, 应返回 CertParsingFailed
	assert_eq!(extract_tbs_from_cert(&malicious_cert).unwrap_err(), DcapError::CertParsingFailed);
	assert_eq!(extract_ecdsa_sig_from_cert(&malicious_cert).unwrap_err(), DcapError::CertParsingFailed);
}

/// C1-fix: submit_verified_attestation 的 dcap_level 应为 1 (非密码学验证)
#[test]
fn submit_verified_attestation_level_is_1_not_0() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let quote = build_fake_tdx_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_verified_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert!(!record.quote_verified, "C1: no ECDSA → quote_verified must be false");
		assert_eq!(record.dcap_level, 1, "C1: structure-only parse → level 1");
	});
}
