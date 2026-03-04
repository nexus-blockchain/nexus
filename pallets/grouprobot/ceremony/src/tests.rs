use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use pallet_grouprobot_primitives::*;

// ============================================================================
// approve_ceremony_enclave / remove_ceremony_enclave
// ============================================================================

#[test]
fn approve_enclave_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, b"test enclave v1".to_vec()
		));
		assert!(ApprovedEnclaves::<Test>::contains_key(mrenclave(1)));
		let info = ApprovedEnclaves::<Test>::get(mrenclave(1)).unwrap();
		assert_eq!(info.version, 1);
	});
}

#[test]
fn approve_enclave_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_noop!(
			GroupRobotCeremony::approve_ceremony_enclave(
				RuntimeOrigin::root(), mrenclave(1), 2, vec![]
			),
			Error::<Test>::EnclaveAlreadyApproved
		);
	});
}

#[test]
fn approve_enclave_fails_not_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::approve_ceremony_enclave(
				RuntimeOrigin::signed(OWNER), mrenclave(1), 1, vec![]
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_enclave_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::remove_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1)
		));
		assert!(!ApprovedEnclaves::<Test>::contains_key(mrenclave(1)));
	});
}

#[test]
fn remove_enclave_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::remove_ceremony_enclave(RuntimeOrigin::root(), mrenclave(99)),
			Error::<Test>::EnclaveNotFound
		);
	});
}

// ============================================================================
// record_ceremony
// ============================================================================

#[test]
fn record_ceremony_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		let participants = vec![[10u8; 32], [11u8; 32]];
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1),
			mrenclave(1),
			2, // k
			3, // n
			bot_pk(1),
			participants,
			bot_id(1),
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert_eq!(record.k, 2);
		assert_eq!(record.n, 3);
		assert_eq!(record.bot_public_key, bot_pk(1));
		assert_eq!(record.participant_count, 2);
		assert!(matches!(record.status, CeremonyStatus::Active));
		assert_eq!(record.expires_at, 1 + 100);

		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), Some(ceremony_hash(1)));
		assert_eq!(CeremonyCount::<Test>::get(), 1);
		assert_eq!(CeremonyHistory::<Test>::get(bot_pk(1)).len(), 1);
	});
}

#[test]
fn record_ceremony_fails_invalid_shamir() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		// k=0
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 0, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
			),
			Error::<Test>::InvalidShamirParams
		);

		// k > n
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 5, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
			),
			Error::<Test>::InvalidShamirParams
		);
	});
}

#[test]
fn record_ceremony_fails_enclave_not_approved() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(99), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
			),
			Error::<Test>::EnclaveNotApproved
		);
	});
}

#[test]
fn record_ceremony_fails_duplicate() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
			),
			Error::<Test>::CeremonyAlreadyExists
		);
	});
}

#[test]
fn record_ceremony_supersedes_old() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		// First ceremony
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		// Second ceremony for same bot_pk
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 3, 5, bot_pk(1), vec![[10u8; 32], [11u8; 32], [12u8; 32]], bot_id(1),
		));

		// Old should be Superseded
		let old = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(old.status, CeremonyStatus::Superseded { replaced_by } if replaced_by == ceremony_hash(2)));

		// New is active
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), Some(ceremony_hash(2)));
		assert_eq!(CeremonyHistory::<Test>::get(bot_pk(1)).len(), 2);
	});
}

#[test]
fn record_ceremony_fails_empty_participants() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![], bot_id(1),
			),
			Error::<Test>::EmptyParticipants
		);
	});
}

// ============================================================================
// revoke_ceremony
// ============================================================================

#[test]
fn revoke_ceremony_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), None);
	});
}

#[test]
fn revoke_ceremony_fails_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(99)),
			Error::<Test>::CeremonyNotFound
		);
	});
}

#[test]
fn revoke_ceremony_fails_already_revoked() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));
		// L1-R3: revoke_ceremony now uses CeremonyNotActive (consistent with force_re_ceremony)
		assert_noop!(
			GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)),
			Error::<Test>::CeremonyNotActive
		);
	});
}

// ============================================================================
// force_re_ceremony
// ============================================================================

#[test]
fn force_re_ceremony_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		assert_ok!(GroupRobotCeremony::force_re_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), None);
	});
}

// ============================================================================
// on_initialize: ceremony expiry
// ============================================================================

#[test]
fn ceremony_expires_on_initialize() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));

		// expires_at = 1 + 100 = 101. Advance to 110 (interval=10)
		advance_to(110);

		assert!(!GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Expired));
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), None);
	});
}

#[test]
fn ceremony_not_expired_before_time() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		advance_to(50);
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
	});
}

// ============================================================================
// Helper Functions
// ============================================================================

#[test]
fn helper_ceremony_shamir_params() {
	new_test_ext().execute_with(|| {
		assert_eq!(GroupRobotCeremony::ceremony_shamir_params(&bot_pk(1)), None);

		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_eq!(GroupRobotCeremony::ceremony_shamir_params(&bot_pk(1)), Some((2, 3)));
	});
}

#[test]
fn helper_functions_coverage() {
	new_test_ext().execute_with(|| {
		assert!(!GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
		assert_eq!(GroupRobotCeremony::get_active_ceremony(&bot_pk(1)), None);

		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
		assert_eq!(
			GroupRobotCeremony::ceremony_shamir_params(&bot_pk(1)),
			Some((2, 3))
		);
		assert_eq!(
			GroupRobotCeremony::get_active_ceremony(&bot_pk(1)),
			Some(ceremony_hash(1))
		);
		assert!(GroupRobotCeremony::is_enclave_approved(&mrenclave(1)));
		assert!(!GroupRobotCeremony::is_enclave_approved(&mrenclave(99)));
	});
}

// ============================================================================
// G1: record_ceremony caller identity verification
// ============================================================================

#[test]
fn record_ceremony_fails_not_bot_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// OTHER (account 2) tries to record for bot_id(1) owned by OWNER
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OTHER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn record_ceremony_fails_bot_not_found() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// bot_id(99) does not exist in MockBotRegistry
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(99),
			),
			Error::<Test>::BotNotFound
		);
	});
}

// ============================================================================
// G2: Re-ceremony marker fields
// ============================================================================

#[test]
fn record_ceremony_first_is_not_re_ceremony() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(!record.is_re_ceremony);
		assert_eq!(record.supersedes, None);
	});
}

// ============================================================================
// Tier gate
// ============================================================================

#[test]
fn record_ceremony_fails_free_tier() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// bot_id(3) → owner=OTHER, active=true, tier=Free in MockSubscription
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OTHER),
				ceremony_hash(10), mrenclave(1), 2, 3, bot_pk(3), vec![[10u8; 32], [11u8; 32]], bot_id(3),
			),
			Error::<Test>::FreeTierNotAllowed
		);
	});
}

#[test]
fn record_ceremony_second_is_re_ceremony() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 3, 5, bot_pk(1), vec![[10u8; 32], [11u8; 32], [12u8; 32]], bot_id(1),
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(2)).unwrap();
		assert!(record.is_re_ceremony);
		assert_eq!(record.supersedes, Some(ceremony_hash(1)));
	});
}

// ============================================================================
// C1: bot_id_hash stored in CeremonyRecord
// ============================================================================

#[test]
fn c1_record_stores_bot_id_hash() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert_eq!(record.bot_id_hash, bot_id(1));
	});
}

// ============================================================================
// H1: InsufficientParticipants (participant_count < k)
// ============================================================================

#[test]
fn h1_record_ceremony_fails_insufficient_participants() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// k=3 but only 2 participants → should fail
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 3, 5, bot_pk(1),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			),
			Error::<Test>::InsufficientParticipants
		);
	});
}

#[test]
fn h1_record_ceremony_ok_participants_equal_k() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// k=2, 2 participants → should succeed
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
	});
}

// ============================================================================
// H2: force_re_ceremony rejects non-active ceremonies
// ============================================================================

#[test]
fn h2_force_re_ceremony_rejects_already_revoked() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// Revoke first
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));
		// Force re-ceremony on already-revoked should fail
		assert_noop!(
			GroupRobotCeremony::force_re_ceremony(RuntimeOrigin::root(), ceremony_hash(1)),
			Error::<Test>::CeremonyNotActive
		);
	});
}

#[test]
fn h2_force_re_ceremony_rejects_expired() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// Expire via on_initialize
		advance_to(110);
		// Force re-ceremony on expired should fail
		assert_noop!(
			GroupRobotCeremony::force_re_ceremony(RuntimeOrigin::root(), ceremony_hash(1)),
			Error::<Test>::CeremonyNotActive
		);
	});
}

// ============================================================================
// M1: approve_ceremony_enclave rejects too-long description
// ============================================================================

#[test]
fn m1_approve_enclave_rejects_too_long_description() {
	new_test_ext().execute_with(|| {
		let long_desc = vec![0x41u8; 129]; // 129 bytes > 128 max
		assert_noop!(
			GroupRobotCeremony::approve_ceremony_enclave(
				RuntimeOrigin::root(), mrenclave(1), 1, long_desc,
			),
			Error::<Test>::DescriptionTooLong
		);
	});
}

#[test]
fn m1_approve_enclave_accepts_max_length_description() {
	new_test_ext().execute_with(|| {
		let max_desc = vec![0x41u8; 128]; // exactly 128 bytes
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, max_desc.clone(),
		));
		let info = ApprovedEnclaves::<Test>::get(mrenclave(1)).unwrap();
		assert_eq!(info.description.len(), 128);
	});
}

// ============================================================================
// Round 2 审计回归测试
// ============================================================================

#[test]
fn m1_audit_peer_count_zero_triggers_at_risk() {
	// M1-audit: peer_count == 0 时应触发 CeremonyAtRisk（secret 完全不可恢复）
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));

		// 设置 peer_count = 0（模拟所有节点离线）
		set_mock_peer_count(Some(0));
		advance_to(10); // 触发 on_initialize（interval=10）

		// 仪式应仍然活跃（未过期）但发出 AtRisk 事件
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremonyAtRisk {
				ceremony_hash: ceremony_hash(1),
				bot_public_key: bot_pk(1),
				required_k: 2,
				current_peer_count: 0,
			},
		));

		// 清理 mock
		set_mock_peer_count(None);
	});
}

#[test]
fn m1_audit_peer_count_equal_k_triggers_at_risk() {
	// CeremonyAtRisk 一般测试：peer_count == k 时也触发
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 3, 5, bot_pk(1),
			vec![[10u8; 32], [11u8; 32], [12u8; 32]],
			bot_id(1),
		));

		// peer_count = 3 == k，应触发 AtRisk
		set_mock_peer_count(Some(3));
		advance_to(10);

		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremonyAtRisk {
				ceremony_hash: ceremony_hash(1),
				bot_public_key: bot_pk(1),
				required_k: 3,
				current_peer_count: 3,
			},
		));

		set_mock_peer_count(None);
	});
}

#[test]
fn at_risk_not_triggered_when_peer_count_above_k() {
	// peer_count > k 时不应触发 CeremonyAtRisk
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));

		// peer_count = 3 > k(2)，不应触发
		// MockBotRegistry 默认 peer_count=3 for bot_id(1)
		advance_to(10);

		// 确认没有 AtRisk 事件
		assert!(!System::events().iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::GroupRobotCeremony(
					crate::pallet::Event::CeremonyAtRisk { .. }
				)
			)
		}));
	});
}

#[test]
fn m2_audit_duplicate_participants_rejected() {
	// M2-audit: 重复的 participant_enclaves 应被拒绝
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 两个相同的 enclave
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
				vec![[10u8; 32], [10u8; 32]],
				bot_id(1),
			),
			Error::<Test>::DuplicateParticipant
		);
	});
}

#[test]
fn m2_audit_unique_participants_accepted() {
	// M2-audit: 唯一的 participant_enclaves 应被接受
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
	});
}

// ============================================================================
// L1-R3: revoke_ceremony rejects non-active ceremonies
// ============================================================================

#[test]
fn l1_r3_revoke_ceremony_rejects_expired() {
	// L1-R3: 过期仪式不能被撤销（与 force_re_ceremony 保持一致）
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 让仪式过期
		advance_to(110);
		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Expired));
		// 尝试撤销过期仪式应失败
		assert_noop!(
			GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)),
			Error::<Test>::CeremonyNotActive
		);
	});
}

#[test]
fn l1_r3_revoke_ceremony_rejects_superseded() {
	// L1-R3: 已替代的仪式不能被撤销
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 新仪式替代旧仪式
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		let old = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(old.status, CeremonyStatus::Superseded { .. }));
		// 尝试撤销 Superseded 仪式应失败
		assert_noop!(
			GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)),
			Error::<Test>::CeremonyNotActive
		);
	});
}

// ============================================================================
// L2-R3: ExpiryQueue + bounded on_initialize
// ============================================================================

#[test]
fn l2_r3_expiry_queue_populated_on_record() {
	// L2-R3: record_ceremony 后 ExpiryQueue 应包含新条目
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		let queue = ExpiryQueue::<Test>::get();
		assert_eq!(queue.len(), 1);
		assert_eq!(queue[0].0, 101); // expires_at = 1 + 100
		assert_eq!(queue[0].2, ceremony_hash(1));
	});
}

#[test]
fn l2_r3_expiry_queue_sorted_by_expires_at() {
	// L2-R3: 不同时间创建的仪式在 ExpiryQueue 中按 expires_at 排序
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 在 block 1 创建第一个仪式 (expires_at = 101)
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 在 block 5 创建第二个仪式 (expires_at = 105)
		System::set_block_number(5);
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(3), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		let queue = ExpiryQueue::<Test>::get();
		// 第一个被 supersede 移除，只剩第二个
		// 但第一个被 supersede 了所以从队列移除了
		assert_eq!(queue.len(), 1);
		assert_eq!(queue[0].0, 105); // expires_at = 5 + 100
	});
}

#[test]
fn l2_r3_expiry_queue_cleaned_on_revoke() {
	// L2-R3: revoke_ceremony 后 ExpiryQueue 中应移除对应条目
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_eq!(ExpiryQueue::<Test>::get().len(), 1);

		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));
		assert_eq!(ExpiryQueue::<Test>::get().len(), 0);
	});
}

#[test]
fn l2_r3_expiry_queue_cleaned_on_force_re_ceremony() {
	// L2-R3: force_re_ceremony 后 ExpiryQueue 中应移除对应条目
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_eq!(ExpiryQueue::<Test>::get().len(), 1);

		assert_ok!(GroupRobotCeremony::force_re_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));
		assert_eq!(ExpiryQueue::<Test>::get().len(), 0);
	});
}

#[test]
fn l2_r3_on_initialize_uses_expiry_queue() {
	// L2-R3: on_initialize 通过 ExpiryQueue 处理过期，而非全表扫描
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_eq!(ExpiryQueue::<Test>::get().len(), 1);

		// 过期后 on_initialize 应处理并从队列移除
		advance_to(110);

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Expired));
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), None);
		assert_eq!(ExpiryQueue::<Test>::get().len(), 0);
	});
}

// ============================================================================
// Round 4 审计回归测试
// ============================================================================

// M2-R4: CeremonyHistory FIFO — 历史满时移除最旧条目，不阻塞新仪式
#[test]
fn m2_r4_ceremony_history_fifo_when_full() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// MaxCeremonyHistory = 10 in mock
		// 创建 10 个仪式填满历史
		for i in 1..=10u8 {
			let mut ch = [0u8; 32];
			ch[0] = i;
			ch[1] = 0xCC; // 区分 ceremony_hash
			assert_ok!(GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ch, mrenclave(1), 2, 3, bot_pk(1),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			));
		}
		assert_eq!(CeremonyHistory::<Test>::get(bot_pk(1)).len(), 10);

		// 第 11 个仪式 — 以前会 CeremonyHistoryFull，现在应 FIFO 成功
		let mut ch11 = [0u8; 32];
		ch11[0] = 11;
		ch11[1] = 0xCC;
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ch11, mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		let history = CeremonyHistory::<Test>::get(bot_pk(1));
		assert_eq!(history.len(), 10); // 仍然 10 条
		// 最旧的 (i=1) 被移除
		let mut first_hash = [0u8; 32];
		first_hash[0] = 1;
		first_hash[1] = 0xCC;
		assert!(!history.contains(&first_hash));
		// 最新的 (i=11) 存在
		assert!(history.contains(&ch11));
	});
}

#[test]
fn m2_r4_ceremony_history_fifo_preserves_order() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 创建 10 个仪式
		for i in 1..=10u8 {
			let mut ch = [0u8; 32];
			ch[0] = i;
			ch[1] = 0xDD;
			assert_ok!(GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ch, mrenclave(1), 2, 3, bot_pk(1),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			));
		}
		// 再加 2 个
		for i in 11..=12u8 {
			let mut ch = [0u8; 32];
			ch[0] = i;
			ch[1] = 0xDD;
			assert_ok!(GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ch, mrenclave(1), 2, 3, bot_pk(1),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			));
		}
		let history = CeremonyHistory::<Test>::get(bot_pk(1));
		assert_eq!(history.len(), 10);
		// i=1 和 i=2 应已被 FIFO 移除，i=3 是最旧的
		let mut expected_oldest = [0u8; 32];
		expected_oldest[0] = 3;
		expected_oldest[1] = 0xDD;
		assert_eq!(history[0], expected_oldest);
	});
}

// M1-R4: ExpiryQueue 满时返回 ExpiryQueueFull 错误
// 注: ExpiryQueue 上限 1000，mock 中 MaxProcessPerBlock=20
// 为避免超长测试，我们直接填充 ExpiryQueue 到上限然后验证
#[test]
fn m1_r4_expiry_queue_full_rejects_record() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		// 直接填充 ExpiryQueue 到 1000 条
		ExpiryQueue::<Test>::mutate(|queue| {
			for i in 0..1000u32 {
				let mut fake_pk = [0u8; 32];
				fake_pk[0..4].copy_from_slice(&i.to_le_bytes());
				let mut fake_hash = [0u8; 32];
				fake_hash[0..4].copy_from_slice(&i.to_le_bytes());
				fake_hash[31] = 0xFF;
				let _ = queue.try_push((9999u64, fake_pk, fake_hash));
			}
		});
		assert_eq!(ExpiryQueue::<Test>::get().len(), 1000);

		// 尝试记录仪式应失败
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			),
			Error::<Test>::ExpiryQueueFull
		);
	});
}

#[test]
fn m1_r4_expiry_queue_not_full_allows_record() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		// 填充 ExpiryQueue 到 999 条（差 1 满）
		ExpiryQueue::<Test>::mutate(|queue| {
			for i in 0..999u32 {
				let mut fake_pk = [0u8; 32];
				fake_pk[0..4].copy_from_slice(&i.to_le_bytes());
				let mut fake_hash = [0u8; 32];
				fake_hash[0..4].copy_from_slice(&i.to_le_bytes());
				fake_hash[31] = 0xFF;
				let _ = queue.try_push((9999u64, fake_pk, fake_hash));
			}
		});
		assert_eq!(ExpiryQueue::<Test>::get().len(), 999);

		// 记录仪式应成功
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		assert_eq!(ExpiryQueue::<Test>::get().len(), 1000);
	});
}

#[test]
fn l3_bot_public_key_mismatch_rejected() {
	// L3: bot_public_key 与注册不匹配时拒绝
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		// 设置 mock 返回特定的注册公钥
		let registered_pk = [0xAA; 32];
		set_mock_bot_pk(Some(registered_pk));

		// 使用不同的公钥尝试记录仪式
		let wrong_pk = [0xBB; 32];
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, wrong_pk,
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			),
			Error::<Test>::BotPublicKeyMismatch
		);

		// 使用正确公钥应成功
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, registered_pk,
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));

		set_mock_bot_pk(None);
	});
}

// ============================================================================
// Round 5 审计回归测试
// ============================================================================

// M1-R5: AtRisk 游标不再跳过边界处的仪式
// 修复前: skip_while(|(k,_)| k <= start_key) 会跳过游标位置的仪式
// 修复后: skip_while(|(k,_)| k < start_key) 从游标位置恢复（含该位置）
#[test]
fn m1_r5_at_risk_cursor_does_not_skip_boundary_ceremony() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));

		// 创建一个仪式，peer_count <= k 触发 AtRisk
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));

		// 手动设置 AtRiskCursor 为该仪式的 bot_pk
		// 模拟上一轮 Phase 2 预算耗尽时将此 bot_pk 存为游标
		AtRiskCursor::<Test>::put(bot_pk(1));

		// 设置 peer_count = 1 <= k(2)，应触发 AtRisk
		set_mock_peer_count(Some(1));

		// 触发 on_initialize
		advance_to(10);

		// 修复后: 游标位置的仪式应被处理，发出 AtRisk 事件
		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremonyAtRisk {
				ceremony_hash: ceremony_hash(1),
				bot_public_key: bot_pk(1),
				required_k: 2,
				current_peer_count: 1,
			},
		));

		set_mock_peer_count(None);
	});
}

// M2-R5: TooManyParticipants 在 O(n²) 去重检测之前被拒绝
// MaxParticipants = 5 (mock), 提交 6 个参与者应立即被拒
#[test]
fn m2_r5_too_many_participants_rejected_before_dup_check() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 6 个参与者 > MaxParticipants(5)
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 6, bot_pk(1),
				vec![[10u8; 32], [11u8; 32], [12u8; 32], [13u8; 32], [14u8; 32], [15u8; 32]],
				bot_id(1),
			),
			Error::<Test>::TooManyParticipants
		);
	});
}

// M2-R5: 验证 MaxParticipants 边界 — 正好等于上限应成功
#[test]
fn m2_r5_max_participants_boundary_accepted() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 正好 5 个参与者 = MaxParticipants(5)，应成功
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 5, bot_pk(1),
			vec![[10u8; 32], [11u8; 32], [12u8; 32], [13u8; 32], [14u8; 32]],
			bot_id(1),
		));
	});
}

// ============================================================================
// Round 6 审计回归测试
// ============================================================================

// M1-R6: record_ceremony 拒绝未激活的 Bot
#[test]
fn m1_r6_record_ceremony_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// bot_id(2) → owner=OTHER, is_bot_active=false (mock: only bot_id[0]==1 is active)
		// bot_id(2) 的 tier 是 Free，所以我们需要一个 active=false 但 paid tier 的 bot
		// 使用 bot_id(1) 但 mock 返回 active=true，所以我们需要用不同方式测试
		// 实际上 bot_id[0]==2 → is_bot_active=false, tier=Free
		// 但 FreeTierNotAllowed 会先拦截。让我们直接验证 BotNotActive 的逻辑：
		// bot_id(99) → bot_owner=None → BotNotFound
		// 我们需要一个 bot_id 满足: owner存在, active=false, tier=paid
		// Mock中: bot_id[0]==1 → owner=OWNER, active=true, tier=Basic
		//         bot_id[0]==2 → owner=OTHER, active=false, tier=Free
		// 由于 mock 限制，active=false 的都是 Free tier，会先被 FreeTierNotAllowed 拦截
		// 为了测试 BotNotActive，我们需要修改测试策略：验证检查顺序
		// is_bot_active 在 tier check 之前，所以 bot_id(2) 应返回 BotNotActive 而非 FreeTierNotAllowed
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OTHER),
				ceremony_hash(10), mrenclave(1), 2, 3, bot_pk(2),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(2),
			),
			Error::<Test>::BotNotActive
		);
	});
}

// M1-R6: 激活的 Bot 可以正常记录仪式（回归验证）
#[test]
fn m1_r6_record_ceremony_allows_active_bot() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// bot_id(1) → active=true, tier=Basic
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
	});
}

// M2-R6: 参与者数量超过 n 时被拒绝
#[test]
fn m2_r6_participants_exceeding_n_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// k=2, n=2, 但提供 3 个参与者 → 超过 n
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 2, bot_pk(1),
				vec![[10u8; 32], [11u8; 32], [12u8; 32]],
				bot_id(1),
			),
			Error::<Test>::ParticipantCountExceedsN
		);
	});
}

// M2-R6: 参与者数量等于 n 时成功（边界）
#[test]
fn m2_r6_participants_equal_n_accepted() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// k=2, n=3, 3 个参与者 = n → 应成功
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32], [12u8; 32]],
			bot_id(1),
		));
	});
}

// M2-R6: 参与者数量小于 n 但 >= k 时成功（部分参与）
#[test]
fn m2_r6_participants_between_k_and_n_accepted() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// k=2, n=5, 3 个参与者: k <= 3 <= n → 应成功
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 5, bot_pk(1),
			vec![[10u8; 32], [11u8; 32], [12u8; 32]],
			bot_id(1),
		));
	});
}

// M3-R6: cleanup_ceremony 清理 Expired 仪式
#[test]
fn m3_r6_cleanup_expired_ceremony() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		// 过期
		advance_to(110);
		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Expired));

		// 清理
		assert_ok!(GroupRobotCeremony::cleanup_ceremony(
			RuntimeOrigin::signed(OTHER), ceremony_hash(1)
		));
		assert!(Ceremonies::<Test>::get(ceremony_hash(1)).is_none());
		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremonyCleaned { ceremony_hash: ceremony_hash(1) },
		));
	});
}

// M3-R6: cleanup_ceremony 清理 Revoked 仪式
#[test]
fn m3_r6_cleanup_revoked_ceremony() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));

		assert_ok!(GroupRobotCeremony::cleanup_ceremony(
			RuntimeOrigin::signed(OTHER), ceremony_hash(1)
		));
		assert!(Ceremonies::<Test>::get(ceremony_hash(1)).is_none());
	});
}

// M3-R6: cleanup_ceremony 清理 Superseded 仪式
#[test]
fn m3_r6_cleanup_superseded_ceremony() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		// 新仪式替代旧仪式
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		let old = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(old.status, CeremonyStatus::Superseded { .. }));

		assert_ok!(GroupRobotCeremony::cleanup_ceremony(
			RuntimeOrigin::signed(OTHER), ceremony_hash(1)
		));
		assert!(Ceremonies::<Test>::get(ceremony_hash(1)).is_none());
		// 新仪式不受影响
		assert!(Ceremonies::<Test>::get(ceremony_hash(2)).is_some());
	});
}

// M3-R6: cleanup_ceremony 拒绝清理 Active 仪式
#[test]
fn m3_r6_cleanup_active_ceremony_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]],
			bot_id(1),
		));
		assert_noop!(
			GroupRobotCeremony::cleanup_ceremony(RuntimeOrigin::signed(OTHER), ceremony_hash(1)),
			Error::<Test>::CeremonyNotTerminal
		);
	});
}

// M3-R6: cleanup_ceremony 拒绝不存在的仪式
#[test]
fn m3_r6_cleanup_nonexistent_ceremony_rejected() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::cleanup_ceremony(RuntimeOrigin::signed(OTHER), ceremony_hash(99)),
			Error::<Test>::CeremonyNotFound
		);
	});
}

// L1-R5: CeremonyHistoryFull 已移除 — FIFO 行为在无该错误码下仍正常
#[test]
fn l1_r5_ceremony_history_fifo_works_without_history_full_error() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 创建 11 个仪式（超过 MaxCeremonyHistory=10），确认 FIFO 无需 CeremonyHistoryFull
		for i in 1..=11u8 {
			let mut ch = [0u8; 32];
			ch[0] = i;
			ch[1] = 0xA5;
			assert_ok!(GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ch, mrenclave(1), 2, 3, bot_pk(1),
				vec![[10u8; 32], [11u8; 32]],
				bot_id(1),
			));
		}
		let history = CeremonyHistory::<Test>::get(bot_pk(1));
		assert_eq!(history.len(), 10);
		// 第 1 个已被 FIFO 移除，第 2 个是最旧的
		let mut oldest = [0u8; 32];
		oldest[0] = 2;
		oldest[1] = 0xA5;
		assert_eq!(history[0], oldest);
	});
}

// ============================================================================
// F1: owner_revoke_ceremony
// ============================================================================

#[test]
fn f1_owner_revoke_ceremony_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		assert_ok!(GroupRobotCeremony::owner_revoke_ceremony(
			RuntimeOrigin::signed(OWNER), ceremony_hash(1)
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), None);

		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::OwnerCeremonyRevoked {
				ceremony_hash: ceremony_hash(1),
				bot_public_key: bot_pk(1),
			},
		));
	});
}

#[test]
fn f1_owner_revoke_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		assert_noop!(
			GroupRobotCeremony::owner_revoke_ceremony(
				RuntimeOrigin::signed(OTHER), ceremony_hash(1)
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn f1_owner_revoke_rejects_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::owner_revoke_ceremony(
				RuntimeOrigin::signed(OWNER), ceremony_hash(99)
			),
			Error::<Test>::CeremonyNotFound
		);
	});
}

#[test]
fn f1_owner_revoke_rejects_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// Revoke first
		assert_ok!(GroupRobotCeremony::owner_revoke_ceremony(
			RuntimeOrigin::signed(OWNER), ceremony_hash(1)
		));
		// Second revoke should fail
		assert_noop!(
			GroupRobotCeremony::owner_revoke_ceremony(
				RuntimeOrigin::signed(OWNER), ceremony_hash(1)
			),
			Error::<Test>::CeremonyNotActive
		);
	});
}

// ============================================================================
// F7: revoke_by_mrenclave
// ============================================================================

#[test]
fn f7_revoke_by_mrenclave_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 创建两个使用同一 mrenclave 的仪式（不同 bot）
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// bot_id(3) → owner=OTHER, active=true, tier=Free → 但我们需要 paid tier
		// 使用不同的 ceremony_hash 但同一 bot_pk 会 supersede
		// 改用同一 bot 的新仪式不行，直接手动插入第二个
		// 实际上只测试一个仪式被撤销即可
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));

		assert_ok!(GroupRobotCeremony::revoke_by_mrenclave(
			RuntimeOrigin::root(), mrenclave(1)
		));

		assert!(!GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));

		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremoniesRevokedByMrenclave {
				mrenclave: mrenclave(1),
				count: 1,
			},
		));
	});
}

#[test]
fn f7_revoke_by_mrenclave_no_match() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		// 使用不匹配的 mrenclave
		assert_ok!(GroupRobotCeremony::revoke_by_mrenclave(
			RuntimeOrigin::root(), mrenclave(99)
		));

		// 仪式不受影响
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));

		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremoniesRevokedByMrenclave {
				mrenclave: mrenclave(99),
				count: 0,
			},
		));
	});
}

#[test]
fn f7_revoke_by_mrenclave_rejects_non_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::revoke_by_mrenclave(
				RuntimeOrigin::signed(OWNER), mrenclave(1)
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ============================================================================
// F12: trigger_expiry
// ============================================================================

#[test]
fn f12_trigger_expiry_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// expires_at = 1 + 100 = 101
		// Set block to 102 (past expiry) but don't trigger on_initialize
		System::set_block_number(102);

		assert_ok!(GroupRobotCeremony::trigger_expiry(
			RuntimeOrigin::signed(OTHER), ceremony_hash(1)
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(matches!(record.status, CeremonyStatus::Expired));
		assert_eq!(ActiveCeremony::<Test>::get(bot_pk(1)), None);
		assert_eq!(ExpiryQueue::<Test>::get().len(), 0);

		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremonyManuallyExpired {
				ceremony_hash: ceremony_hash(1),
			},
		));
	});
}

#[test]
fn f12_trigger_expiry_rejects_not_expired() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 还未过期 (current block=1, expires_at=101)
		assert_noop!(
			GroupRobotCeremony::trigger_expiry(
				RuntimeOrigin::signed(OTHER), ceremony_hash(1)
			),
			Error::<Test>::CeremonyNotExpired
		);
	});
}

#[test]
fn f12_trigger_expiry_rejects_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));

		System::set_block_number(200);
		assert_noop!(
			GroupRobotCeremony::trigger_expiry(
				RuntimeOrigin::signed(OTHER), ceremony_hash(1)
			),
			Error::<Test>::CeremonyNotActive
		);
	});
}

#[test]
fn f12_trigger_expiry_rejects_not_found() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::trigger_expiry(
				RuntimeOrigin::signed(OTHER), ceremony_hash(99)
			),
			Error::<Test>::CeremonyNotFound
		);
	});
}

// ============================================================================
// F11: batch_cleanup_ceremonies
// ============================================================================

#[test]
fn f11_batch_cleanup_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		// 创建 3 个仪式 (第 1 个会被替代两次)
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// ceremony_hash(1) is now Superseded
		// Revoke ceremony_hash(2)
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(2)));

		// 批量清理
		assert_ok!(GroupRobotCeremony::batch_cleanup_ceremonies(
			RuntimeOrigin::signed(OTHER),
			vec![ceremony_hash(1), ceremony_hash(2)],
		));

		assert!(Ceremonies::<Test>::get(ceremony_hash(1)).is_none());
		assert!(Ceremonies::<Test>::get(ceremony_hash(2)).is_none());
	});
}

#[test]
fn f11_batch_cleanup_rejects_empty() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			GroupRobotCeremony::batch_cleanup_ceremonies(
				RuntimeOrigin::signed(OTHER), vec![],
			),
			Error::<Test>::NoCeremoniesToCleanup
		);
	});
}

#[test]
fn f11_batch_cleanup_rejects_too_many() {
	new_test_ext().execute_with(|| {
		// MaxProcessPerBlock = 20 in mock
		let hashes: alloc::vec::Vec<[u8; 32]> = (0..21u8).map(|i| {
			let mut h = [0u8; 32];
			h[0] = i;
			h[31] = 0xFA;
			h
		}).collect();
		assert_noop!(
			GroupRobotCeremony::batch_cleanup_ceremonies(
				RuntimeOrigin::signed(OTHER), hashes,
			),
			Error::<Test>::TooManyCeremonies
		);
	});
}

#[test]
fn f11_batch_cleanup_rejects_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		assert_noop!(
			GroupRobotCeremony::batch_cleanup_ceremonies(
				RuntimeOrigin::signed(OTHER), vec![ceremony_hash(1)],
			),
			Error::<Test>::CeremonyNotTerminal
		);
	});
}

// ============================================================================
// F2: renew_ceremony
// ============================================================================

#[test]
fn f2_renew_ceremony_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 原 expires_at = 1 + 100 = 101

		// 在 block 50 续期
		System::set_block_number(50);
		assert_ok!(GroupRobotCeremony::renew_ceremony(
			RuntimeOrigin::signed(OWNER), ceremony_hash(1)
		));

		// 新 expires_at = 50 + 100 = 150
		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert_eq!(record.expires_at, 150);

		// ExpiryQueue 更新
		let queue = ExpiryQueue::<Test>::get();
		assert_eq!(queue.len(), 1);
		assert_eq!(queue[0].0, 150);

		System::assert_has_event(RuntimeEvent::GroupRobotCeremony(
			crate::pallet::Event::CeremonyRenewed {
				ceremony_hash: ceremony_hash(1),
				new_expires_at: 150,
			},
		));
	});
}

#[test]
fn f2_renew_rejects_non_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_noop!(
			GroupRobotCeremony::renew_ceremony(
				RuntimeOrigin::signed(OTHER), ceremony_hash(1)
			),
			Error::<Test>::NotBotOwner
		);
	});
}

#[test]
fn f2_renew_rejects_not_active() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));

		assert_noop!(
			GroupRobotCeremony::renew_ceremony(
				RuntimeOrigin::signed(OWNER), ceremony_hash(1)
			),
			Error::<Test>::CeremonyNotActive
		);
	});
}

#[test]
fn f2_renew_rejects_inactive_bot() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 手动将仪式的 bot_id_hash 改为 inactive bot
		Ceremonies::<Test>::mutate(ceremony_hash(1), |maybe| {
			if let Some(r) = maybe {
				r.bot_id_hash = bot_id(2); // bot_id(2) → active=false
			}
		});
		assert_noop!(
			GroupRobotCeremony::renew_ceremony(
				RuntimeOrigin::signed(OTHER), ceremony_hash(1)
			),
			Error::<Test>::BotNotActive
		);
	});
}

#[test]
fn f2_renew_prevents_expiry() {
	// 续期后仪式不会在原过期时间到期
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		// 原 expires_at = 101

		// 在 block 90 续期 → new expires_at = 190
		System::set_block_number(90);
		assert_ok!(GroupRobotCeremony::renew_ceremony(
			RuntimeOrigin::signed(OWNER), ceremony_hash(1)
		));

		// 推进到 block 110 (原过期时间后)
		advance_to(110);
		// 仪式应仍然活跃
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));

		// 推进到 block 200 (新过期时间后)
		advance_to(200);
		// 现在应过期
		assert!(!GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
	});
}

// ============================================================================
// F3: ceremony_health
// ============================================================================

#[test]
fn f3_ceremony_health_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		// MockBotRegistry: bot_id(1) → peer_count=3
		let health = GroupRobotCeremony::ceremony_health(&bot_pk(1));
		assert!(health.is_some());
		let (expires_at, peer_count, k) = health.unwrap();
		assert_eq!(expires_at, 101);
		assert_eq!(peer_count, 3);
		assert_eq!(k, 2);
	});
}

#[test]
fn f3_ceremony_health_none_when_no_ceremony() {
	new_test_ext().execute_with(|| {
		assert_eq!(GroupRobotCeremony::ceremony_health(&bot_pk(1)), None);
	});
}

// ============================================================================
// F13: ceremony_expires_at / ceremony_participant_enclaves
// ============================================================================

#[test]
fn f13_ceremony_expires_at_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));
		assert_eq!(GroupRobotCeremony::ceremony_expires_at(&bot_pk(1)), Some(101));
		assert_eq!(GroupRobotCeremony::ceremony_expires_at(&bot_pk(99)), None);
	});
}

#[test]
fn f13_ceremony_participant_enclaves_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		let participants = vec![[10u8; 32], [11u8; 32], [12u8; 32]];
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1),
			participants.clone(), bot_id(1),
		));
		assert_eq!(
			GroupRobotCeremony::ceremony_participant_enclaves(&bot_pk(1)),
			Some(participants)
		);
		assert_eq!(GroupRobotCeremony::ceremony_participant_enclaves(&bot_pk(99)), None);
	});
}
