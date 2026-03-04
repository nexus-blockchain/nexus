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

/// P6-L1-fix: 首次绑定发射 UserPlatformBound 事件
#[test]
fn p6_l1_bind_user_platform_emits_bound_event() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::bind_user_platform(
			RuntimeOrigin::signed(OWNER), Platform::Discord, pk(42),
		));
		assert_eq!(UserPlatformBindings::<Test>::get(OWNER, Platform::Discord), Some(pk(42)));
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::UserPlatformBound { account: OWNER, platform: Platform::Discord },
		));
	});
}

/// P6-L1-fix: 覆盖绑定发射 UserPlatformBindingUpdated 事件 (含 old_hash)
#[test]
fn p6_l1_bind_user_platform_overwrite_emits_updated_event() {
	new_test_ext().execute_with(|| {
		// 首次绑定
		assert_ok!(GroupRobotRegistry::bind_user_platform(
			RuntimeOrigin::signed(OWNER), Platform::Discord, pk(42),
		));
		// 覆盖绑定
		assert_ok!(GroupRobotRegistry::bind_user_platform(
			RuntimeOrigin::signed(OWNER), Platform::Discord, pk(99),
		));
		assert_eq!(UserPlatformBindings::<Test>::get(OWNER, Platform::Discord), Some(pk(99)));
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::UserPlatformBindingUpdated {
				account: OWNER,
				platform: Platform::Discord,
				old_hash: pk(42),
			},
		));
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

// ============================================================================
// PeerRegistry: register_peer / deregister_peer / heartbeat_peer
// ============================================================================

fn endpoint(s: &str) -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<256>> {
	s.as_bytes().to_vec().try_into().unwrap()
}

#[test]
fn register_peer_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://node-a:8443"),
		));
		let peers = PeerRegistry::<Test>::get(bot_hash(1));
		assert_eq!(peers.len(), 1);
		assert_eq!(peers[0].public_key, pk(10));
		assert_eq!(GroupRobotRegistry::peer_count(&bot_hash(1)), 1);
	});
}

#[test]
fn register_peer_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://node-a:8443"),
		));
		assert_noop!(
			GroupRobotRegistry::register_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://node-b:8443"),
			),
			Error::<Test>::PeerAlreadyRegistered
		);
	});
}

#[test]
fn register_peer_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::register_peer(
				RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10), endpoint("https://node-a:8443"),
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn register_peer_fails_endpoint_empty() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::register_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint(""),
			),
			Error::<Test>::EndpointEmpty
		);
	});
}

#[test]
fn register_peer_fails_max_peers() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		// MaxPeersPerBot = 10 in mock
		for i in 0..10u8 {
			assert_ok!(GroupRobotRegistry::register_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(100 + i),
				endpoint("https://node:8443"),
			));
		}
		assert_noop!(
			GroupRobotRegistry::register_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(200),
				endpoint("https://node:8443"),
			),
			Error::<Test>::MaxPeersReached
		);
	});
}

#[test]
fn deregister_peer_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://node-a:8443"),
		));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(11), endpoint("https://node-b:8443"),
		));
		assert_eq!(GroupRobotRegistry::peer_count(&bot_hash(1)), 2);

		assert_ok!(GroupRobotRegistry::deregister_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
		));
		assert_eq!(GroupRobotRegistry::peer_count(&bot_hash(1)), 1);
		let peers = PeerRegistry::<Test>::get(bot_hash(1));
		assert_eq!(peers[0].public_key, pk(11));
	});
}

#[test]
fn deregister_peer_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::deregister_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			),
			Error::<Test>::PeerNotFound
		);
	});
}

#[test]
fn heartbeat_peer_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://node-a:8443"),
		));
		let peers = PeerRegistry::<Test>::get(bot_hash(1));
		assert_eq!(peers[0].last_seen, 1); // block 1

		System::set_block_number(50);
		assert_ok!(GroupRobotRegistry::heartbeat_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
		));
		let peers = PeerRegistry::<Test>::get(bot_hash(1));
		assert_eq!(peers[0].last_seen, 50);
	});
}

#[test]
fn heartbeat_peer_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			),
			Error::<Test>::PeerNotFound
		);
	});
}

// ============================================================================
// G4: Peer heartbeat expiry (on_initialize)
// ============================================================================

#[test]
fn peer_expires_after_heartbeat_timeout() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		// Register peer at block 1
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://a:8443"),
		));
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 1);

		// P3-2: Peer cleanup is now passive via report_stale_peer
		// Peer registered at block 1, last_seen=1. At block 60: 60-1=59 > 50 → stale
		System::set_block_number(60);

		// Anyone can report
		assert_ok!(GroupRobotRegistry::report_stale_peer(
			RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10),
		));
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 0);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::StalePeerReported { bot_id_hash: bot_hash(1), public_key: pk(10), reporter: OTHER, peer_count: 0 },
		));
	});
}

#[test]
fn peer_survives_if_heartbeat_fresh() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://a:8443"),
		));

		// Heartbeat at block 40 → last_seen=40
		System::set_block_number(40);
		assert_ok!(GroupRobotRegistry::heartbeat_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
		));

		// At block 60: 60-40=20 < 50 → NOT stale, report should fail
		System::set_block_number(60);
		assert_noop!(
			GroupRobotRegistry::report_stale_peer(RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10)),
			Error::<Test>::PeerNotStale
		);
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 1);

		// At block 100: 100-40=60 > 50 → stale, report succeeds
		System::set_block_number(100);
		assert_ok!(GroupRobotRegistry::report_stale_peer(
			RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10),
		));
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 0);
	});
}

#[test]
fn peer_expiry_partial_removes_only_stale() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		// Peer A at block 1
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://a:8443"),
		));
		// Peer B at block 30
		System::set_block_number(30);
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(11), endpoint("https://b:8443"),
		));
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 2);

		// At block 60: Peer A (last_seen=1, 60-1=59>50) stale, Peer B (last_seen=30, 60-30=30<50) fresh
		System::set_block_number(60);

		// Report Peer A → succeeds
		assert_ok!(GroupRobotRegistry::report_stale_peer(
			RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10),
		));
		// Report Peer B → fails (not stale)
		assert_noop!(
			GroupRobotRegistry::report_stale_peer(RuntimeOrigin::signed(OTHER), bot_hash(1), pk(11)),
			Error::<Test>::PeerNotStale
		);
		let peers = PeerRegistry::<Test>::get(bot_hash(1));
		assert_eq!(peers.len(), 1);
		assert_eq!(peers[0].public_key, pk(11));
	});
}

#[test]
fn register_peer_fails_deactivated_bot() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_noop!(
			GroupRobotRegistry::register_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://node:8443"),
			),
			Error::<Test>::BotNotActive
		);
	});
}

// ============================================================================
// SGX DCAP 验证: submit_sgx_attestation
// ============================================================================

/// 构建 SGX Quote v3 (带真实 P-256 签名)
/// report_data[0..32] = SHA256(bot_public_key)
fn build_sgx_quote(
	mrenclave_val: &[u8; 32],
	public_key: &[u8; 32],
) -> (Vec<u8>, [u8; 64]) {
	use crate::dcap::test_utils::TestSgxQuoteBuilder;

	let mut rd = [0u8; 64];
	let pk_hash = sp_core::hashing::sha2_256(public_key);
	rd[..32].copy_from_slice(&pk_hash);

	let builder = TestSgxQuoteBuilder::new(1)
		.with_mrenclave(*mrenclave_val)
		.with_report_data(rd);

	let pck_key = builder.pck_public_key();
	let quote = builder.build();
	(quote, pck_key)
}

#[test]
fn sgx_attestation_works() {
	new_test_ext().execute_with(|| {
		// Setup: approve MRTD + MRENCLAVE, register bot, submit TDX attestation
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		// Verify: no dual attestation yet
		assert!(!GroupRobotRegistry::has_dual_attestation(&bot_hash(1)));

		// M1-fix: request nonce before SGX attestation
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (sgx_quote, _pck) = build_sgx_quote_with_nonce(&mrenclave(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_sgx_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			bounded,
			None, None, None,
		));

		// Verify: now has dual attestation
		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert!(record.is_dual_attestation);
		assert_eq!(record.mrenclave, Some(mrenclave(1)));
		assert!(record.sgx_quote_hash.is_some());
		assert!(GroupRobotRegistry::has_dual_attestation(&bot_hash(1)));

		// Verify: BotInfo updated
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		if let NodeType::TeeNode { mrenclave: sgx_mre, sgx_attested_at, .. } = bot.node_type {
			assert_eq!(sgx_mre, Some(mrenclave(1)));
			assert!(sgx_attested_at.is_some());
		} else {
			panic!("expected TeeNode");
		}
	});
}

#[test]
fn sgx_attestation_fails_without_tdx() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// No TDX attestation submitted → AttestationNotFound
		let (sgx_quote, _pck) = build_sgx_quote(&mrenclave(1), &pk(1));
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_sgx_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::AttestationNotFound
		);
	});
}

#[test]
fn sgx_attestation_fails_mrenclave_not_approved() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		// M1-fix: request nonce so we reach MRENCLAVE check
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (sgx_quote, _pck) = build_sgx_quote_with_nonce(&mrenclave(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_sgx_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::MrenclaveNotApproved
		);
	});
}

#[test]
fn sgx_attestation_fails_report_data_mismatch() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		// Build SGX quote with WRONG public key → report_data mismatch
		let (sgx_quote, _pck) = build_sgx_quote(&mrenclave(1), &pk(99));
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_sgx_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::QuoteReportDataMismatch
		);
	});
}

#[test]
fn sgx_level3_attestation_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		// M1-fix: request nonce before SGX attestation
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (sgx_quote, pck_key) = build_sgx_quote_with_nonce(&mrenclave(1), &pk(1), &nonce);

		// Register PCK key for Level 3
		let platform_id = [0x02u8; 32];
		assert_ok!(GroupRobotRegistry::register_pck_key(
			RuntimeOrigin::root(), platform_id, pck_key,
		));

		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_sgx_attestation(
			RuntimeOrigin::signed(OWNER),
			bot_hash(1),
			bounded,
			Some(platform_id),
			None, None,
		));

		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert!(record.is_dual_attestation);
		assert_eq!(record.mrenclave, Some(mrenclave(1)));
	});
}

#[test]
fn sgx_attestation_tampered_quote_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		let (mut sgx_quote, _pck) = build_sgx_quote(&mrenclave(1), &pk(1));
		// Tamper with MRENCLAVE after signing → ECDSA sig invalid
		sgx_quote[crate::dcap::SGX_MRENCLAVE_OFFSET] ^= 0xFF;
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_sgx_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::DcapBodySignatureInvalid
		);
	});
}

// ============================================================================
// submit_tee_attestation (三模式统一入口, call_index 21)
// ============================================================================

/// 构建 SGX Quote v3 (带 nonce, 用于 submit_tee_attestation)
fn build_sgx_quote_with_nonce(
	mrenclave_val: &[u8; 32],
	public_key: &[u8; 32],
	nonce: &[u8; 32],
) -> (Vec<u8>, [u8; 64]) {
	use crate::dcap::test_utils::TestSgxQuoteBuilder;

	let mut rd = [0u8; 64];
	let pk_hash = sp_core::hashing::sha2_256(public_key);
	rd[..32].copy_from_slice(&pk_hash);
	rd[32..64].copy_from_slice(nonce);

	let builder = TestSgxQuoteBuilder::new(1)
		.with_mrenclave(*mrenclave_val)
		.with_report_data(rd);

	let pck_key = builder.pck_public_key();
	let quote = builder.build();
	(quote, pck_key)
}

#[test]
fn tee_attestation_tdx_only_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));

		// Verify AttestationRecordV2
		let record = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.tee_type, TeeType::Tdx);
		assert_eq!(record.primary_measurement, mrtd(1));
		assert!(record.mrenclave.is_none());
		assert!(!record.is_dual_attestation);
		assert_eq!(record.dcap_level, 2);

		// Verify NodeType is TeeNodeV2
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		match bot.node_type {
			NodeType::TeeNodeV2 { tee_type, primary_measurement, mrenclave, .. } => {
				assert_eq!(tee_type, TeeType::Tdx);
				assert_eq!(primary_measurement, mrtd(1));
				assert!(mrenclave.is_none());
			}
			_ => panic!("expected TeeNodeV2"),
		}

		// Event emitted
		System::assert_has_event(Event::TeeAttestationSubmitted {
			bot_id_hash: bot_hash(1),
			tee_type: TeeType::Tdx,
			dcap_level: 2,
		}.into());
	});
}

#[test]
fn tee_attestation_sgx_only_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_sgx_quote_with_nonce(&mrenclave(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));

		// Verify AttestationRecordV2
		let record = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.tee_type, TeeType::Sgx);
		assert_eq!(record.mrenclave, Some(mrenclave(1)));
		// primary_measurement = MRENCLAVE padded to 48B
		let mut expected_primary = [0u8; 48];
		expected_primary[..32].copy_from_slice(&mrenclave(1));
		assert_eq!(record.primary_measurement, expected_primary);
		assert!(!record.is_dual_attestation);
		assert_eq!(record.dcap_level, 2);

		// Verify NodeType is TeeNodeV2 with Sgx
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		match bot.node_type {
			NodeType::TeeNodeV2 { tee_type, mrenclave: mr_val, .. } => {
				assert_eq!(tee_type, TeeType::Sgx);
				assert_eq!(mr_val, Some(mrenclave(1)));
			}
			_ => panic!("expected TeeNodeV2"),
		}

		System::assert_has_event(Event::TeeAttestationSubmitted {
			bot_id_hash: bot_hash(1),
			tee_type: TeeType::Sgx,
			dcap_level: 2,
		}.into());
	});
}

#[test]
fn tee_attestation_rejects_invalid_version() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		let _nonce = request_nonce(OWNER, bot_hash(1));

		// Quote with version=5 (unsupported)
		let mut fake = vec![0u8; 700];
		fake[0] = 5; // version LE low byte = 5
		fake[1] = 0;
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			fake.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_tee_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::DcapQuoteInvalid
		);
	});
}

#[test]
fn tee_attestation_sgx_fails_mrenclave_not_approved() {
	new_test_ext().execute_with(|| {
		// No mrenclave approved
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_sgx_quote_with_nonce(&mrenclave(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_tee_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::MrenclaveNotApproved
		);
	});
}

#[test]
fn tee_attestation_tdx_fails_mrtd_not_approved() {
	new_test_ext().execute_with(|| {
		// No mrtd approved
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_tee_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::MrtdNotApproved
		);
	});
}

#[test]
fn tee_attestation_v2_expires_on_initialize() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));

		// V2 record exists
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_some());
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));

		// Advance past expiry (AttestationValidityBlocks = 100, check interval = 10)
		advance_to(110);

		// V2 record should be cleared, NodeType reset
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_none());
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert!(matches!(bot.node_type, NodeType::StandardNode));
	});
}

#[test]
fn tee_attestation_mode_switch_tdx_to_sgx() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(2), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// First: TDX attestation
		let nonce1 = request_nonce(OWNER, bot_hash(1));
		let (tdx_quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce1);
		let bounded1: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			tdx_quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded1, None, None, None,
		));

		let record1 = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record1.tee_type, TeeType::Tdx);

		// Second: Switch to SGX attestation (simulating hardware migration)
		let nonce2 = request_nonce(OWNER, bot_hash(1));
		let (sgx_quote, _pck) = build_sgx_quote_with_nonce(&mrenclave(2), &pk(1), &nonce2);
		let bounded2: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded2, None, None, None,
		));

		// V2 record now reflects SGX
		let record2 = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record2.tee_type, TeeType::Sgx);
		assert_eq!(record2.mrenclave, Some(mrenclave(2)));

		// NodeType also switched
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		match bot.node_type {
			NodeType::TeeNodeV2 { tee_type, .. } => assert_eq!(tee_type, TeeType::Sgx),
			_ => panic!("expected TeeNodeV2"),
		}
	});
}

#[test]
fn tee_attestation_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_tee_attestation(
				RuntimeOrigin::signed(OTHER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

// ============================================================================
// Operator Tests (Multi-Platform)
// ============================================================================

fn op_name(s: &[u8]) -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> {
	s.to_vec().try_into().unwrap()
}

fn op_contact(s: &[u8]) -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<128>> {
	s.to_vec().try_into().unwrap()
}

fn app_hash(n: u8) -> [u8; 32] {
	let mut h = [0u8; 32];
	h[0] = n;
	h[31] = 0xAA; // distinguish from bot_hash
	h
}

#[test]
fn register_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER),
			Platform::Telegram,
			app_hash(1),
			op_name(b"Operator Alpha"),
			op_contact(b"@alpha_op"),
		));
		let op = Operators::<Test>::get(OWNER, Platform::Telegram).unwrap();
		assert_eq!(op.owner, OWNER);
		assert_eq!(op.platform, Platform::Telegram);
		assert_eq!(op.platform_app_hash, app_hash(1));
		assert_eq!(op.status, OperatorStatus::Active);
		assert_eq!(op.bot_count, 0);
		assert_eq!(op.sla_level, 0);
		assert_eq!(op.reputation_score, 100);
		assert_eq!(OperatorCount::<Test>::get(), 1);
		assert_eq!(PlatformAppHashIndex::<Test>::get(Platform::Telegram, app_hash(1)), Some(OWNER));
	});
}

#[test]
fn register_operator_rejects_duplicate_same_platform() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op1"), op_contact(b"c1"),
		));
		assert_noop!(
			GroupRobotRegistry::register_operator(
				RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(2), op_name(b"Op2"), op_contact(b"c2"),
			),
			Error::<Test>::OperatorAlreadyRegistered
		);
	});
}

#[test]
fn register_operator_allows_different_platforms() {
	new_test_ext().execute_with(|| {
		// 同一账户可在 Telegram 和 Discord 上分别注册
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"TG Op"), op_contact(b"@tg"),
		));
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Discord, app_hash(2), op_name(b"DC Op"), op_contact(b"@dc"),
		));
		assert_eq!(OperatorCount::<Test>::get(), 2);
		assert!(Operators::<Test>::get(OWNER, Platform::Telegram).is_some());
		assert!(Operators::<Test>::get(OWNER, Platform::Discord).is_some());
	});
}

#[test]
fn register_operator_rejects_duplicate_app_hash_same_platform() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op1"), op_contact(b"c1"),
		));
		assert_noop!(
			GroupRobotRegistry::register_operator(
				RuntimeOrigin::signed(OWNER2), Platform::Telegram, app_hash(1), op_name(b"Op2"), op_contact(b"c2"),
			),
			Error::<Test>::ApiIdHashAlreadyUsed
		);
	});
}

#[test]
fn register_operator_allows_same_app_hash_different_platforms() {
	new_test_ext().execute_with(|| {
		// 同一 app_hash 在不同平台上是允许的 (不同平台的 ID 命名空间独立)
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"TG"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER2), Platform::Discord, app_hash(1), op_name(b"DC"), op_contact(b"c"),
		));
		assert_eq!(OperatorCount::<Test>::get(), 2);
	});
}

#[test]
fn register_operator_rejects_empty_name() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::register_operator(
				RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b""), op_contact(b"c1"),
			),
			Error::<Test>::OperatorNameEmpty
		);
	});
}

#[test]
fn update_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Old"), op_contact(b"old"),
		));
		assert_ok!(GroupRobotRegistry::update_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, op_name(b"New"), op_contact(b"new"),
		));
		let op = Operators::<Test>::get(OWNER, Platform::Telegram).unwrap();
		assert_eq!(op.name.as_slice(), b"New");
		assert_eq!(op.contact.as_slice(), b"new");
	});
}

#[test]
fn update_operator_fails_not_registered() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::update_operator(
				RuntimeOrigin::signed(OWNER), Platform::Telegram, op_name(b"X"), op_contact(b"y"),
			),
			Error::<Test>::OperatorNotFound
		);
	});
}

#[test]
fn deregister_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_eq!(OperatorCount::<Test>::get(), 1);

		assert_ok!(GroupRobotRegistry::deregister_operator(RuntimeOrigin::signed(OWNER), Platform::Telegram));
		assert!(Operators::<Test>::get(OWNER, Platform::Telegram).is_none());
		assert!(PlatformAppHashIndex::<Test>::get(Platform::Telegram, app_hash(1)).is_none());
		assert_eq!(OperatorCount::<Test>::get(), 0);
	});
}

#[test]
fn deregister_operator_fails_with_active_bots() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));

		assert_noop!(
			GroupRobotRegistry::deregister_operator(RuntimeOrigin::signed(OWNER), Platform::Telegram),
			Error::<Test>::OperatorHasActiveBots
		);
	});
}

#[test]
fn set_operator_sla_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::set_operator_sla(RuntimeOrigin::root(), OWNER, Platform::Telegram, 2));
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().sla_level, 2);
	});
}

#[test]
fn set_operator_sla_rejects_invalid_level() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_noop!(
			GroupRobotRegistry::set_operator_sla(RuntimeOrigin::root(), OWNER, Platform::Telegram, 4),
			Error::<Test>::InvalidSlaLevel
		);
	});
}

#[test]
fn set_operator_sla_rejects_non_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_noop!(
			GroupRobotRegistry::set_operator_sla(RuntimeOrigin::signed(OWNER), OWNER, Platform::Telegram, 1),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn assign_bot_to_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));

		assert_eq!(BotOperator::<Test>::get(bot_hash(1)), Some((OWNER, Platform::Telegram)));
		assert_eq!(OperatorBots::<Test>::get(OWNER, Platform::Telegram).len(), 1);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 1);
	});
}

#[test]
fn assign_bot_rejects_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER2), Platform::Telegram, app_hash(2), op_name(b"Op2"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::assign_bot_to_operator(
				RuntimeOrigin::signed(OWNER2), bot_hash(1), Platform::Telegram,
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn assign_bot_rejects_not_operator() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::assign_bot_to_operator(
				RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
			),
			Error::<Test>::OperatorNotFound
		);
	});
}

#[test]
fn assign_bot_rejects_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));
		assert_noop!(
			GroupRobotRegistry::assign_bot_to_operator(
				RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
			),
			Error::<Test>::BotAlreadyAssigned
		);
	});
}

#[test]
fn unassign_bot_from_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 1);

		assert_ok!(GroupRobotRegistry::unassign_bot_from_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
		));
		assert!(BotOperator::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(OperatorBots::<Test>::get(OWNER, Platform::Telegram).len(), 0);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 0);
	});
}

#[test]
fn unassign_bot_fails_not_assigned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::unassign_bot_from_operator(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
			),
			Error::<Test>::BotNotAssigned
		);
	});
}

#[test]
fn operator_multi_bot_multi_peer_integration() {
	new_test_ext().execute_with(|| {
		let tg = Platform::Telegram;
		// 注册 Telegram 运营商
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), tg, app_hash(1), op_name(b"MultiBot Op"), op_contact(b"@multi"),
		));

		// 注册 2 个 Bot (bot_hash(1) is paid tier in mock)
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(2), pk(2)));

		// 将两个 Bot 都分配给 Telegram 运营商
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(RuntimeOrigin::signed(OWNER), bot_hash(1), tg));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(RuntimeOrigin::signed(OWNER), bot_hash(2), tg));
		assert_eq!(Operators::<Test>::get(OWNER, tg).unwrap().bot_count, 2);
		assert_eq!(OperatorBots::<Test>::get(OWNER, tg).len(), 2);

		// Bot 1 (paid) 注册 2 个 Peer (运营商内部多节点)
		let ep1: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<256>> =
			b"https://node1.example.com".to_vec().try_into().unwrap();
		let ep2: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<256>> =
			b"https://node2.example.com".to_vec().try_into().unwrap();

		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), ep1,
		));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(11), ep2,
		));
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 2);

		// 验证 helper functions
		assert_eq!(GroupRobotRegistry::bot_operator_account(&bot_hash(1)), Some(OWNER));
		assert_eq!(GroupRobotRegistry::bot_operator_full(&bot_hash(1)), Some((OWNER, tg)));
		assert_eq!(GroupRobotRegistry::operator_bots(&OWNER, tg).len(), 2);

		// 取消分配 Bot 2
		assert_ok!(GroupRobotRegistry::unassign_bot_from_operator(RuntimeOrigin::signed(OWNER), bot_hash(2)));
		assert_eq!(Operators::<Test>::get(OWNER, tg).unwrap().bot_count, 1);
		assert!(BotOperator::<Test>::get(bot_hash(2)).is_none());

		// 现在可以取消 Bot 1 后注销运营商
		assert_ok!(GroupRobotRegistry::unassign_bot_from_operator(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_ok!(GroupRobotRegistry::deregister_operator(RuntimeOrigin::signed(OWNER), tg));
		assert_eq!(OperatorCount::<Test>::get(), 0);
	});
}

#[test]
fn two_operators_independent_bots() {
	new_test_ext().execute_with(|| {
		let tg = Platform::Telegram;
		// 两个独立运营商 (同一平台)
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), tg, app_hash(1), op_name(b"Op A"), op_contact(b"@a"),
		));
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER2), tg, app_hash(2), op_name(b"Op B"), op_contact(b"@b"),
		));
		assert_eq!(OperatorCount::<Test>::get(), 2);

		// 各自注册自己的 Bot
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER2), bot_hash(2), pk(2)));

		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(RuntimeOrigin::signed(OWNER), bot_hash(1), tg));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(RuntimeOrigin::signed(OWNER2), bot_hash(2), tg));

		// 互不影响
		assert_eq!(GroupRobotRegistry::bot_operator_account(&bot_hash(1)), Some(OWNER));
		assert_eq!(GroupRobotRegistry::bot_operator_account(&bot_hash(2)), Some(OWNER2));
		assert_eq!(GroupRobotRegistry::operator_bots(&OWNER, tg).len(), 1);
		assert_eq!(GroupRobotRegistry::operator_bots(&OWNER2, tg).len(), 1);
	});
}

#[test]
fn cross_platform_operator_isolation() {
	new_test_ext().execute_with(|| {
		// 同一账户在 Telegram 和 Discord 上各注册运营商
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1),
			op_name(b"TG Operator"), op_contact(b"@tg_op"),
		));
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Discord, app_hash(2),
			op_name(b"DC Operator"), op_contact(b"@dc_op"),
		));
		assert_eq!(OperatorCount::<Test>::get(), 2);

		// 注册 2 个 Bot
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(2), pk(2)));

		// Bot 1 分配给 Telegram 运营商, Bot 2 分配给 Discord 运营商
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(2), Platform::Discord,
		));

		// 验证隔离
		assert_eq!(GroupRobotRegistry::bot_operator_full(&bot_hash(1)), Some((OWNER, Platform::Telegram)));
		assert_eq!(GroupRobotRegistry::bot_operator_full(&bot_hash(2)), Some((OWNER, Platform::Discord)));
		assert_eq!(GroupRobotRegistry::operator_bots(&OWNER, Platform::Telegram).len(), 1);
		assert_eq!(GroupRobotRegistry::operator_bots(&OWNER, Platform::Discord).len(), 1);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 1);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Discord).unwrap().bot_count, 1);

		// 注销 Telegram 运营商 (需先取消 Bot)
		assert_ok!(GroupRobotRegistry::unassign_bot_from_operator(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_ok!(GroupRobotRegistry::deregister_operator(RuntimeOrigin::signed(OWNER), Platform::Telegram));

		// Discord 运营商不受影响
		assert_eq!(OperatorCount::<Test>::get(), 1);
		assert!(Operators::<Test>::get(OWNER, Platform::Discord).is_some());
		assert_eq!(GroupRobotRegistry::bot_operator_full(&bot_hash(2)), Some((OWNER, Platform::Discord)));
	});
}

// ============================================================================
// Peer Uptime Tracking Tests
// ============================================================================

#[test]
fn heartbeat_increments_uptime_counter() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) => paid tier (MockSubscription)
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("n1"),
		));

		// 初始计数为 0
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 0);

		// 发送 3 次心跳
		for _ in 0..3 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			));
		}

		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 3);
	});
}

#[test]
fn record_era_uptime_snapshots_and_resets() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("n1"),
		));

		// 5 次心跳
		for _ in 0..5 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			));
		}
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 5);

		// 模拟 Era 0 结算
		<GroupRobotRegistry as PeerUptimeRecorder>::record_era_uptime(0);

		// 计数器已重置
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 0);

		// 历史快照已记录
		let history = GroupRobotRegistry::peer_era_uptime(&bot_hash(1), &pk(10));
		assert_eq!(history.len(), 1);
		assert_eq!(history[0], (0, 5));
	});
}

#[test]
fn uptime_rolling_window_evicts_oldest() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("n1"),
		));

		// MaxUptimeEraHistory = 10 in mock
		// 填满 10 个 Era + 额外 2 个
		for era in 0..12u64 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			));
			<GroupRobotRegistry as PeerUptimeRecorder>::record_era_uptime(era);
		}

		let history = GroupRobotRegistry::peer_era_uptime(&bot_hash(1), &pk(10));
		// 滚动窗口: 只保留最近 10 个
		assert_eq!(history.len(), 10);
		// 最老的应该是 era 2 (era 0, 1 被淘汰)
		assert_eq!(history[0].0, 2);
		assert_eq!(history[9].0, 11);
	});
}

#[test]
fn uptime_multiple_peers_tracked_independently() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("n1"),
		));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(11), endpoint("n2"),
		));

		// Peer A: 3 次心跳, Peer B: 7 次心跳
		for _ in 0..3 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			));
		}
		for _ in 0..7 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(11),
			));
		}

		<GroupRobotRegistry as PeerUptimeRecorder>::record_era_uptime(0);

		let history_a = GroupRobotRegistry::peer_era_uptime(&bot_hash(1), &pk(10));
		let history_b = GroupRobotRegistry::peer_era_uptime(&bot_hash(1), &pk(11));
		assert_eq!(history_a[0], (0, 3));
		assert_eq!(history_b[0], (0, 7));
	});
}

#[test]
fn no_heartbeat_no_uptime_record() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("n1"),
		));

		// 无心跳, 直接结算
		<GroupRobotRegistry as PeerUptimeRecorder>::record_era_uptime(0);

		let history = GroupRobotRegistry::peer_era_uptime(&bot_hash(1), &pk(10));
		assert_eq!(history.len(), 0);
	});
}

// ============================================================================
// Audit Regression Tests (Phase 4)
// ============================================================================

/// C1-fix: 公钥轮换必须同时清除 AttestationsV2 记录
#[test]
fn c1_key_rotation_clears_attestations_v2() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 通过 submit_tee_attestation 写入 AttestationsV2
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));

		// 确认 V2 记录存在
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_some());
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));

		// 轮换公钥
		assert_ok!(GroupRobotRegistry::update_public_key(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(2),
		));

		// C1-fix: AttestationsV2 必须被清除
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_none());
		assert!(!GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert!(matches!(bot.node_type, NodeType::StandardNode));
	});
}

/// M5-fix: deregister_peer 必须清理 PeerHeartbeatCount
#[test]
fn m5_deregister_peer_clears_heartbeat_count() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) => paid tier
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://n1:8443"),
		));

		// 发送心跳, 累积计数
		for _ in 0..3 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			));
		}
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 3);

		// 注销 Peer
		assert_ok!(GroupRobotRegistry::deregister_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
		));

		// M5-fix: 心跳计数必须被清除
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 0);
	});
}

/// L1-fix: revoke_mrtd 从白名单移除 MRTD
#[test]
fn l1_revoke_mrtd_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert!(ApprovedMrtd::<Test>::contains_key(mrtd(1)));

		assert_ok!(GroupRobotRegistry::revoke_mrtd(RuntimeOrigin::root(), mrtd(1)));
		assert!(!ApprovedMrtd::<Test>::contains_key(mrtd(1)));
	});
}

/// L1-fix: revoke_mrtd 不存在时报错
#[test]
fn l1_revoke_mrtd_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::revoke_mrtd(RuntimeOrigin::root(), mrtd(99)),
			Error::<Test>::MrtdNotFound
		);
	});
}

/// L1-fix: revoke_mrenclave 从白名单移除 MRENCLAVE
#[test]
fn l1_revoke_mrenclave_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert!(ApprovedMrenclave::<Test>::contains_key(mrenclave(1)));

		assert_ok!(GroupRobotRegistry::revoke_mrenclave(RuntimeOrigin::root(), mrenclave(1)));
		assert!(!ApprovedMrenclave::<Test>::contains_key(mrenclave(1)));
	});
}

/// L1-fix: revoke_mrenclave 不存在时报错
#[test]
fn l1_revoke_mrenclave_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::revoke_mrenclave(RuntimeOrigin::root(), mrenclave(99)),
			Error::<Test>::MrenclaveNotFound
		);
	});
}

/// L2-fix: register_pck_key 拒绝覆盖已注册的 key
#[test]
fn l2_register_pck_key_rejects_overwrite() {
	new_test_ext().execute_with(|| {
		let platform_id = [0x01u8; 32];
		let pck1 = [1u8; 64];
		let pck2 = [2u8; 64];

		assert_ok!(GroupRobotRegistry::register_pck_key(RuntimeOrigin::root(), platform_id, pck1));
		assert_noop!(
			GroupRobotRegistry::register_pck_key(RuntimeOrigin::root(), platform_id, pck2),
			Error::<Test>::PckKeyAlreadyRegistered
		);
	});
}

/// L3-fix: deactivate_bot 清理 BotOperator 和 OperatorBots
#[test]
fn l3_deactivate_bot_clears_operator_assignments() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));

		// 确认分配存在
		assert!(BotOperator::<Test>::get(bot_hash(1)).is_some());
		assert_eq!(OperatorBots::<Test>::get(OWNER, Platform::Telegram).len(), 1);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 1);

		// 停用 Bot
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));

		// L3-fix: 分配关系必须清除
		assert!(BotOperator::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(OperatorBots::<Test>::get(OWNER, Platform::Telegram).len(), 0);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 0);
	});
}

/// M1-fix: submit_sgx_attestation 无 nonce 时拒绝
#[test]
fn m1_sgx_attestation_fails_without_nonce() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));

		// 不请求 nonce, 直接提交 → NonceMissing
		let (sgx_quote, _pck) = build_sgx_quote(&mrenclave(1), &pk(1));
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			sgx_quote.try_into().unwrap();

		assert_noop!(
			GroupRobotRegistry::submit_sgx_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				bounded, None, None, None,
			),
			Error::<Test>::NonceMissing
		);
	});
}

/// M3-fix: report_stale_peer 必须清理 PeerHeartbeatCount
#[test]
fn m3_report_stale_peer_clears_heartbeat_count() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) => paid tier
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://n1:8443"),
		));

		// 发送心跳, 累积计数
		for _ in 0..5 {
			assert_ok!(GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			));
		}
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 5);

		// 前进超过心跳超时 (PeerHeartbeatTimeout = 50)
		System::set_block_number(60);

		// 举报过期 Peer
		assert_ok!(GroupRobotRegistry::report_stale_peer(
			RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10),
		));

		// M3-fix: 心跳计数必须被清除
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 0);
	});
}

// ============================================================================
// Phase 6 回归测试
// ============================================================================

/// P6-H1: refresh_attestation 支持 V2 存储 (submit_tee_attestation 路径)
#[test]
fn p6_h1_refresh_attestation_works_with_v2() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 通过 submit_tee_attestation 写入 V2
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_some());
		assert!(Attestations::<Test>::get(bot_hash(1)).is_none());

		// refresh_attestation 应该成功刷新 V2
		assert_ok!(GroupRobotRegistry::refresh_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[0xAA; 32], None, mrtd(1), None,
		));

		// V2 记录已更新
		let v2 = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(v2.primary_quote_hash, [0xAA; 32]);
		// V1 仍不存在
		assert!(Attestations::<Test>::get(bot_hash(1)).is_none());
	});
}

/// P6-H1: refresh_attestation 仍然支持 V1 存储
#[test]
fn p6_h1_refresh_attestation_still_works_with_v1() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 通过 submit_attestation 写入 V1
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[0x11; 32], None, mrtd(1), None,
		));
		assert!(Attestations::<Test>::get(bot_hash(1)).is_some());

		// refresh 应该成功
		assert_ok!(GroupRobotRegistry::refresh_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[0x22; 32], None, mrtd(1), None,
		));
		let v1 = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(v1.tdx_quote_hash, [0x22; 32]);
	});
}

/// P6-H1: refresh_attestation 无证明时仍失败
#[test]
fn p6_h1_refresh_attestation_fails_no_attestation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		assert_noop!(
			GroupRobotRegistry::refresh_attestation(
				RuntimeOrigin::signed(OWNER), bot_hash(1),
				[0x11; 32], None, mrtd(1), None,
			),
			Error::<Test>::AttestationNotFound
		);
	});
}

/// P6-H2: heartbeat_peer 拒绝已停用 Bot
#[test]
fn p6_h2_heartbeat_peer_rejects_deactivated_bot() {
	new_test_ext().execute_with(|| {
		// bot_hash(1) => paid tier
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://n1:8443"),
		));

		// 心跳正常工作
		assert_ok!(GroupRobotRegistry::heartbeat_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
		));

		// 手动设置 Bot 为 Deactivated (绕过 deactivate_bot 清理 peer 的逻辑, 保留 peer)
		Bots::<Test>::mutate(bot_hash(1), |maybe_bot| {
			if let Some(bot) = maybe_bot {
				bot.status = BotStatus::Deactivated;
			}
		});

		// P6-H2: 心跳应失败
		assert_noop!(
			GroupRobotRegistry::heartbeat_peer(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
			),
			Error::<Test>::BotNotActive
		);
	});
}

/// P6-H3: deactivate_bot 清理证明、Nonce、Peer 存储
#[test]
fn p6_h3_deactivate_bot_clears_attestations_and_peers() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 提交 V1 证明
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[0x11; 32], None, mrtd(1), None,
		));
		assert!(Attestations::<Test>::get(bot_hash(1)).is_some());

		// 请求 nonce
		assert_ok!(GroupRobotRegistry::request_attestation_nonce(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
		));
		assert!(AttestationNonces::<Test>::get(bot_hash(1)).is_some());

		// 注册 Peer + 心跳
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://n1:8443"),
		));
		assert_ok!(GroupRobotRegistry::heartbeat_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10),
		));
		assert_eq!(GroupRobotRegistry::peer_count(&bot_hash(1)), 1);
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 1);

		// 停用
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));

		// P6-H3: 所有相关存储必须已清理
		assert!(Attestations::<Test>::get(bot_hash(1)).is_none());
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_none());
		assert!(AttestationNonces::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(GroupRobotRegistry::peer_count(&bot_hash(1)), 0);
		assert_eq!(GroupRobotRegistry::peer_heartbeat_count(&bot_hash(1), &pk(10)), 0);
	});
}

/// P6-H3: deactivate_bot 清理 V2 证明
#[test]
fn p6_h3_deactivate_bot_clears_v2_attestation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// 通过 submit_tee_attestation 写入 V2
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_some());

		// 停用
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));

		// V2 必须已清理
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_none());
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(bot.status, BotStatus::Deactivated);
	});
}

/// P6-M1: bind_community 可从已停用 Bot 重新绑定
#[test]
fn p6_m1_bind_community_rebinds_from_deactivated_bot() {
	new_test_ext().execute_with(|| {
		// Bot 1 绑定社区
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::bind_community(
			RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Telegram,
		));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().community_count, 1);

		// 停用 Bot 1
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));

		// Bot 2 尝试绑定同一社区 — P6-M1 修复后应成功
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(2), pk(2)));
		assert_ok!(GroupRobotRegistry::bind_community(
			RuntimeOrigin::signed(OWNER), bot_hash(2), community_hash(1), Platform::Telegram,
		));

		// 新绑定生效
		let binding = CommunityBindings::<Test>::get(community_hash(1)).unwrap();
		assert_eq!(binding.bot_id_hash, bot_hash(2));
		assert_eq!(Bots::<Test>::get(bot_hash(2)).unwrap().community_count, 1);
		// 旧 Bot 的 community_count 已递减
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().community_count, 0);
	});
}

/// P6-M1: bind_community 对活跃 Bot 仍拒绝重复绑定
#[test]
fn p6_m1_bind_community_rejects_active_bot_rebind() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(2), pk(2)));

		assert_ok!(GroupRobotRegistry::bind_community(
			RuntimeOrigin::signed(OWNER), bot_hash(1), community_hash(1), Platform::Telegram,
		));

		// Bot 1 仍活跃, Bot 2 不应抢占
		assert_noop!(
			GroupRobotRegistry::bind_community(
				RuntimeOrigin::signed(OWNER), bot_hash(2), community_hash(1), Platform::Telegram,
			),
			Error::<Test>::CommunityAlreadyBound
		);
	});
}

// ============================================================================
// New Extrinsics: suspend_bot / reactivate_bot (call_index 31-32)
// ============================================================================

#[test]
fn suspend_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().status, BotStatus::Suspended);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::BotSuspended { bot_id_hash: bot_hash(1) },
		));
	});
}

#[test]
fn suspend_bot_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::suspend_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn suspend_bot_fails_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_noop!(
			GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(1)),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn suspend_bot_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(99)),
			Error::<Test>::BotNotFound
		);
	});
}

#[test]
fn reactivate_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_ok!(GroupRobotRegistry::reactivate_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().status, BotStatus::Active);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::BotReactivated { bot_id_hash: bot_hash(1) },
		));
	});
}

#[test]
fn reactivate_bot_fails_not_suspended() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::reactivate_bot(RuntimeOrigin::root(), bot_hash(1)),
			Error::<Test>::BotNotSuspended
		);
	});
}

#[test]
fn reactivate_bot_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_noop!(
			GroupRobotRegistry::reactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// New Extrinsic: unbind_user_platform (call_index 33)
// ============================================================================

#[test]
fn unbind_user_platform_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::bind_user_platform(
			RuntimeOrigin::signed(OWNER), Platform::Discord, pk(42),
		));
		assert!(UserPlatformBindings::<Test>::contains_key(OWNER, Platform::Discord));

		assert_ok!(GroupRobotRegistry::unbind_user_platform(
			RuntimeOrigin::signed(OWNER), Platform::Discord,
		));
		assert!(!UserPlatformBindings::<Test>::contains_key(OWNER, Platform::Discord));
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::UserPlatformUnbound { account: OWNER, platform: Platform::Discord },
		));
	});
}

#[test]
fn unbind_user_platform_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::unbind_user_platform(RuntimeOrigin::signed(OWNER), Platform::Discord),
			Error::<Test>::PlatformBindingNotFound
		);
	});
}

// ============================================================================
// New Extrinsic: transfer_bot_ownership (call_index 34)
// ============================================================================

#[test]
fn transfer_bot_ownership_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::transfer_bot_ownership(
			RuntimeOrigin::signed(OWNER), bot_hash(1), OWNER2,
		));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().owner, OWNER2);
		assert_eq!(OwnerBots::<Test>::get(OWNER).len(), 0);
		assert_eq!(OwnerBots::<Test>::get(OWNER2).len(), 1);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::BotOwnershipTransferred {
				bot_id_hash: bot_hash(1),
				old_owner: OWNER,
				new_owner: OWNER2,
			},
		));
	});
}

#[test]
fn transfer_bot_ownership_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::transfer_bot_ownership(RuntimeOrigin::signed(OTHER), bot_hash(1), OWNER2),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn transfer_bot_ownership_fails_same_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::transfer_bot_ownership(RuntimeOrigin::signed(OWNER), bot_hash(1), OWNER),
			Error::<Test>::SameOwner
		);
	});
}

#[test]
fn transfer_bot_ownership_fails_new_owner_max_bots() {
	new_test_ext().execute_with(|| {
		// OWNER2 fills up their bot slots (MaxBotsPerOwner = 5)
		for i in 0..5u8 {
			assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER2), bot_hash(10 + i), pk(10 + i)));
		}
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::transfer_bot_ownership(RuntimeOrigin::signed(OWNER), bot_hash(1), OWNER2),
			Error::<Test>::MaxBotsReached
		);
	});
}

// ============================================================================
// New Extrinsic: revoke_api_server_mrtd (call_index 35)
// ============================================================================

#[test]
fn revoke_api_server_mrtd_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_api_server_mrtd(RuntimeOrigin::root(), mrtd(10), 1));
		assert!(ApprovedApiServerMrtd::<Test>::contains_key(mrtd(10)));

		assert_ok!(GroupRobotRegistry::revoke_api_server_mrtd(RuntimeOrigin::root(), mrtd(10)));
		assert!(!ApprovedApiServerMrtd::<Test>::contains_key(mrtd(10)));
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::ApiServerMrtdRevoked { mrtd: mrtd(10) },
		));
	});
}

#[test]
fn revoke_api_server_mrtd_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::revoke_api_server_mrtd(RuntimeOrigin::root(), mrtd(99)),
			Error::<Test>::ApiServerMrtdNotFound
		);
	});
}

#[test]
fn revoke_api_server_mrtd_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_api_server_mrtd(RuntimeOrigin::root(), mrtd(10), 1));
		assert_noop!(
			GroupRobotRegistry::revoke_api_server_mrtd(RuntimeOrigin::signed(OWNER), mrtd(10)),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// New Extrinsic: revoke_pck_key (call_index 36)
// ============================================================================

#[test]
fn revoke_pck_key_works() {
	new_test_ext().execute_with(|| {
		let pid = [0x01u8; 32];
		let pck = [0xAAu8; 64];
		assert_ok!(GroupRobotRegistry::register_pck_key(RuntimeOrigin::root(), pid, pck));
		assert!(RegisteredPckKeys::<Test>::contains_key(pid));

		assert_ok!(GroupRobotRegistry::revoke_pck_key(RuntimeOrigin::root(), pid));
		assert!(!RegisteredPckKeys::<Test>::contains_key(pid));
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::PckKeyRevoked { platform_id: pid },
		));
	});
}

#[test]
fn revoke_pck_key_fails_not_registered() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::revoke_pck_key(RuntimeOrigin::root(), [0x99u8; 32]),
			Error::<Test>::PckKeyNotRegistered
		);
	});
}

// ============================================================================
// New Extrinsic: force_deactivate_bot (call_index 37)
// ============================================================================

#[test]
fn force_deactivate_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::force_deactivate_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().status, BotStatus::Deactivated);
	});
}

#[test]
fn force_deactivate_bot_works_from_suspended() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_ok!(GroupRobotRegistry::force_deactivate_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().status, BotStatus::Deactivated);
	});
}

#[test]
fn force_deactivate_bot_clears_all_state() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// Setup attestation + peer + operator
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://n1:8443"),
		));
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));

		assert_ok!(GroupRobotRegistry::force_deactivate_bot(RuntimeOrigin::root(), bot_hash(1)));

		// All cleaned
		assert!(Attestations::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(PeerRegistry::<Test>::get(bot_hash(1)).len(), 0);
		assert!(BotOperator::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 0);
	});
}

#[test]
fn force_deactivate_bot_fails_already_deactivated() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert_noop!(
			GroupRobotRegistry::force_deactivate_bot(RuntimeOrigin::root(), bot_hash(1)),
			Error::<Test>::BotAlreadyDeactivated
		);
	});
}

#[test]
fn force_deactivate_bot_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::force_deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// New Extrinsics: suspend_operator / unsuspend_operator (call_index 38-39)
// ============================================================================

#[test]
fn suspend_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::suspend_operator(RuntimeOrigin::root(), OWNER, Platform::Telegram));
		assert_eq!(
			Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().status,
			OperatorStatus::Suspended
		);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::OperatorSuspended { operator: OWNER, platform: Platform::Telegram },
		));
	});
}

#[test]
fn suspend_operator_fails_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::suspend_operator(RuntimeOrigin::root(), OWNER, Platform::Telegram));
		assert_noop!(
			GroupRobotRegistry::suspend_operator(RuntimeOrigin::root(), OWNER, Platform::Telegram),
			Error::<Test>::OperatorNotActive
		);
	});
}

#[test]
fn suspend_operator_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_noop!(
			GroupRobotRegistry::suspend_operator(RuntimeOrigin::signed(OWNER), OWNER, Platform::Telegram),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn unsuspend_operator_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::suspend_operator(RuntimeOrigin::root(), OWNER, Platform::Telegram));
		assert_ok!(GroupRobotRegistry::unsuspend_operator(RuntimeOrigin::root(), OWNER, Platform::Telegram));
		assert_eq!(
			Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().status,
			OperatorStatus::Active
		);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::OperatorUnsuspended { operator: OWNER, platform: Platform::Telegram },
		));
	});
}

#[test]
fn unsuspend_operator_fails_not_suspended() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_noop!(
			GroupRobotRegistry::unsuspend_operator(RuntimeOrigin::root(), OWNER, Platform::Telegram),
			Error::<Test>::OperatorNotSuspended
		);
	});
}

// ============================================================================
// New Extrinsic: update_peer_endpoint (call_index 40)
// ============================================================================

#[test]
fn update_peer_endpoint_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://old:8443"),
		));

		assert_ok!(GroupRobotRegistry::update_peer_endpoint(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://new:9443"),
		));
		let peers = PeerRegistry::<Test>::get(bot_hash(1));
		assert_eq!(peers[0].endpoint.as_slice(), b"https://new:9443");
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::PeerEndpointUpdated { bot_id_hash: bot_hash(1), public_key: pk(10) },
		));
	});
}

#[test]
fn update_peer_endpoint_fails_not_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://old:8443"),
		));
		assert_noop!(
			GroupRobotRegistry::update_peer_endpoint(
				RuntimeOrigin::signed(OTHER), bot_hash(1), pk(10), endpoint("https://new:9443"),
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn update_peer_endpoint_fails_peer_not_found() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::update_peer_endpoint(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(99), endpoint("https://new:9443"),
			),
			Error::<Test>::PeerNotFound
		);
	});
}

#[test]
fn update_peer_endpoint_fails_empty() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::register_peer(
			RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint("https://old:8443"),
		));
		assert_noop!(
			GroupRobotRegistry::update_peer_endpoint(
				RuntimeOrigin::signed(OWNER), bot_hash(1), pk(10), endpoint(""),
			),
			Error::<Test>::EndpointEmpty
		);
	});
}

// ============================================================================
// New Extrinsic: cleanup_deactivated_bot (call_index 41)
// ============================================================================

#[test]
fn cleanup_deactivated_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(BotCount::<Test>::get(), 1);
		assert_eq!(OwnerBots::<Test>::get(OWNER).len(), 1);

		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		// Bot still exists in storage
		assert!(Bots::<Test>::contains_key(bot_hash(1)));

		// Anyone can clean up
		assert_ok!(GroupRobotRegistry::cleanup_deactivated_bot(RuntimeOrigin::signed(OTHER), bot_hash(1)));
		assert!(!Bots::<Test>::contains_key(bot_hash(1)));
		assert_eq!(BotCount::<Test>::get(), 0);
		assert_eq!(OwnerBots::<Test>::get(OWNER).len(), 0);
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::BotCleaned { bot_id_hash: bot_hash(1) },
		));
	});
}

#[test]
fn cleanup_deactivated_bot_fails_not_deactivated() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::cleanup_deactivated_bot(RuntimeOrigin::signed(OTHER), bot_hash(1)),
			Error::<Test>::BotNotDeactivated
		);
	});
}

#[test]
fn cleanup_deactivated_bot_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotRegistry::cleanup_deactivated_bot(RuntimeOrigin::signed(OTHER), bot_hash(99)),
			Error::<Test>::BotNotFound
		);
	});
}

#[test]
fn cleanup_frees_owner_bot_slot() {
	new_test_ext().execute_with(|| {
		// Fill all 5 slots
		for i in 0..5u8 {
			assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(i), pk(i)));
		}
		assert_noop!(
			GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(10), pk(10)),
			Error::<Test>::MaxBotsReached
		);

		// Deactivate + cleanup one
		assert_ok!(GroupRobotRegistry::deactivate_bot(RuntimeOrigin::signed(OWNER), bot_hash(0)));
		assert_ok!(GroupRobotRegistry::cleanup_deactivated_bot(RuntimeOrigin::signed(OTHER), bot_hash(0)));

		// Now can register a new one
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(10), pk(10)));
	});
}

// ============================================================================
// New Extrinsic: operator_unassign_bot (call_index 42)
// ============================================================================

#[test]
fn operator_unassign_bot_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));

		// Operator unassigns
		assert_ok!(GroupRobotRegistry::operator_unassign_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)));
		assert!(BotOperator::<Test>::get(bot_hash(1)).is_none());
		assert_eq!(OperatorBots::<Test>::get(OWNER, Platform::Telegram).len(), 0);
		assert_eq!(Operators::<Test>::get(OWNER, Platform::Telegram).unwrap().bot_count, 0);
	});
}

#[test]
fn operator_unassign_bot_fails_not_operator() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_operator(
			RuntimeOrigin::signed(OWNER), Platform::Telegram, app_hash(1), op_name(b"Op"), op_contact(b"c"),
		));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::assign_bot_to_operator(
			RuntimeOrigin::signed(OWNER), bot_hash(1), Platform::Telegram,
		));

		// OTHER is not the operator
		assert_noop!(
			GroupRobotRegistry::operator_unassign_bot(RuntimeOrigin::signed(OTHER), bot_hash(1)),
			Error::<Test>::NotOperator
		);
	});
}

#[test]
fn operator_unassign_bot_fails_not_assigned() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::operator_unassign_bot(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			Error::<Test>::BotNotAssigned
		);
	});
}

// ============================================================================
// New Extrinsic: force_expire_attestation (call_index 43)
// ============================================================================

#[test]
fn force_expire_attestation_v1_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert!(GroupRobotRegistry::is_tee_node(&bot_hash(1)));

		assert_ok!(GroupRobotRegistry::force_expire_attestation(RuntimeOrigin::root(), bot_hash(1)));
		assert!(!GroupRobotRegistry::is_tee_node(&bot_hash(1)));
		assert!(Attestations::<Test>::get(bot_hash(1)).is_none());
		let bot = Bots::<Test>::get(bot_hash(1)).unwrap();
		assert!(matches!(bot.node_type, NodeType::StandardNode));
		System::assert_has_event(RuntimeEvent::GroupRobotRegistry(
			crate::Event::AttestationExpired { bot_id_hash: bot_hash(1) },
		));
	});
}

#[test]
fn force_expire_attestation_v2_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			bounded, None, None, None,
		));
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_some());

		assert_ok!(GroupRobotRegistry::force_expire_attestation(RuntimeOrigin::root(), bot_hash(1)));
		assert!(AttestationsV2::<Test>::get(bot_hash(1)).is_none());
		assert!(!GroupRobotRegistry::is_tee_node(&bot_hash(1)));
	});
}

#[test]
fn force_expire_attestation_fails_no_attestation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::force_expire_attestation(RuntimeOrigin::root(), bot_hash(1)),
			Error::<Test>::AttestationNotFound
		);
	});
}

#[test]
fn force_expire_attestation_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		assert_noop!(
			GroupRobotRegistry::force_expire_attestation(RuntimeOrigin::signed(OWNER), bot_hash(1)),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// New Extrinsic: force_transfer_bot_ownership (call_index 44)
// ============================================================================

#[test]
fn force_transfer_bot_ownership_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::force_transfer_bot_ownership(
			RuntimeOrigin::root(), bot_hash(1), OWNER2,
		));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().owner, OWNER2);
		assert_eq!(OwnerBots::<Test>::get(OWNER).len(), 0);
		assert_eq!(OwnerBots::<Test>::get(OWNER2).len(), 1);
	});
}

#[test]
fn force_transfer_bot_ownership_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_noop!(
			GroupRobotRegistry::force_transfer_bot_ownership(RuntimeOrigin::signed(OWNER), bot_hash(1), OWNER2),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn force_transfer_allows_same_owner() {
	new_test_ext().execute_with(|| {
		// force_transfer does NOT check SameOwner (unlike user transfer)
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::force_transfer_bot_ownership(
			RuntimeOrigin::root(), bot_hash(1), OWNER,
		));
		assert_eq!(Bots::<Test>::get(bot_hash(1)).unwrap().owner, OWNER);
	});
}

// ============================================================================
// R1-fix: refresh_attestation 维持 dcap_level/quote_verified
// ============================================================================

#[test]
fn r1_refresh_preserves_dcap_level_v1() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// DCAP Level 2 attestation
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_dcap_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
		));
		let record = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(record.dcap_level, 2);

		// Refresh - should preserve dcap_level=2
		assert_ok!(GroupRobotRegistry::refresh_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[0xBB; 32], None, mrtd(1), None,
		));
		let refreshed = Attestations::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(refreshed.dcap_level, 2, "R1-fix: dcap_level must be preserved");
		assert_eq!(refreshed.quote_verified, record.quote_verified, "R1-fix: quote_verified must be preserved");
	});
}

#[test]
fn r1_refresh_preserves_dcap_level_v2() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		// V2 attestation via submit_tee_attestation
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None, None,
		));
		let v2 = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(v2.dcap_level, 2);

		// Refresh V2 - should preserve dcap_level
		assert_ok!(GroupRobotRegistry::refresh_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[0xCC; 32], None, mrtd(1), None,
		));
		let refreshed = AttestationsV2::<Test>::get(bot_hash(1)).unwrap();
		assert_eq!(refreshed.dcap_level, 2, "R1-fix: V2 dcap_level must be preserved");
		assert_eq!(refreshed.tee_type, v2.tee_type, "R1-fix: V2 tee_type must be preserved");
	});
}

// ============================================================================
// New Query Helpers: attestation_level, get_tee_type, attestation_info
// ============================================================================

#[test]
fn query_attestation_level_default_zero() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(GroupRobotRegistry::attestation_level(&bot_hash(1)), 0);
	});
}

#[test]
fn query_attestation_level_v1() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_dcap_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None,
		));
		assert_eq!(GroupRobotRegistry::attestation_level(&bot_hash(1)), 2);
	});
}

#[test]
fn query_attestation_level_v2() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None, None,
		));
		assert_eq!(GroupRobotRegistry::attestation_level(&bot_hash(1)), 2);
	});
}

#[test]
fn query_tee_type_standard_node() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(GroupRobotRegistry::get_tee_type(&bot_hash(1)), None);
	});
}

#[test]
fn query_tee_type_v1_tee_node() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_ok!(GroupRobotRegistry::submit_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1),
			[1u8; 32], None, mrtd(1), None,
		));
		// V1 TeeNode maps to TeeType::Tdx
		assert_eq!(GroupRobotRegistry::get_tee_type(&bot_hash(1)), Some(TeeType::Tdx));
	});
}

#[test]
fn query_tee_type_v2_sgx() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrenclave(RuntimeOrigin::root(), mrenclave(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_sgx_quote_with_nonce(&mrenclave(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None, None,
		));
		assert_eq!(GroupRobotRegistry::get_tee_type(&bot_hash(1)), Some(TeeType::Sgx));
	});
}

#[test]
fn query_attestation_info_returns_full_tuple() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));

		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None, None,
		));

		let info = GroupRobotRegistry::attestation_info(&bot_hash(1));
		assert!(info.is_some());
		let (level, _verified, expires, tee) = info.unwrap();
		assert_eq!(level, 2);
		assert!(expires > 1);
		assert_eq!(tee, Some(TeeType::Tdx));
	});
}

#[test]
fn query_attestation_info_none_without_attestation() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert!(GroupRobotRegistry::attestation_info(&bot_hash(1)).is_none());
	});
}

// ============================================================================
// BotRegistryProvider: new trait methods (attestation_level, tee_type)
// ============================================================================

#[test]
fn bot_registry_provider_attestation_level() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::BotRegistryProvider;

		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::attestation_level(&bot_hash(1)),
			0
		);

		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None, None,
		));

		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::attestation_level(&bot_hash(1)),
			2
		);
	});
}

#[test]
fn bot_registry_provider_tee_type() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::BotRegistryProvider;

		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::tee_type(&bot_hash(1)),
			None
		);

		assert_ok!(GroupRobotRegistry::approve_mrtd(RuntimeOrigin::root(), mrtd(1), 1));
		let nonce = request_nonce(OWNER, bot_hash(1));
		let (quote, _pck) = build_dcap_quote(&mrtd(1), &pk(1), &nonce);
		let bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<8192>> =
			quote.try_into().unwrap();
		assert_ok!(GroupRobotRegistry::submit_tee_attestation(
			RuntimeOrigin::signed(OWNER), bot_hash(1), bounded, None, None, None,
		));

		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::tee_type(&bot_hash(1)),
			Some(TeeType::Tdx)
		);
	});
}

#[test]
fn bot_registry_provider_bot_status() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::BotRegistryProvider;

		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::bot_status(&bot_hash(1)),
			None
		);
		assert_ok!(GroupRobotRegistry::register_bot(RuntimeOrigin::signed(OWNER), bot_hash(1), pk(1)));
		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::bot_status(&bot_hash(1)),
			Some(BotStatus::Active)
		);
		assert_ok!(GroupRobotRegistry::suspend_bot(RuntimeOrigin::root(), bot_hash(1)));
		assert_eq!(
			<GroupRobotRegistry as BotRegistryProvider<u64>>::bot_status(&bot_hash(1)),
			Some(BotStatus::Suspended)
		);
	});
}
