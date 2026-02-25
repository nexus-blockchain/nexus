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
				ceremony_hash(1), mrenclave(1), 0, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
			),
			Error::<Test>::InvalidShamirParams
		);

		// k > n
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 5, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
				ceremony_hash(1), mrenclave(99), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));
		assert_noop!(
			GroupRobotCeremony::record_ceremony(
				RuntimeOrigin::signed(OWNER),
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));

		// Second ceremony for same bot_pk
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 3, 5, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)));
		assert_noop!(
			GroupRobotCeremony::revoke_ceremony(RuntimeOrigin::root(), ceremony_hash(1)),
			Error::<Test>::CeremonyAlreadyRevoked
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));

		advance_to(50);
		assert!(GroupRobotCeremony::is_ceremony_active(&bot_pk(1)));
	});
}

// ============================================================================
// Helpers / CeremonyProvider
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));
		assert_eq!(GroupRobotCeremony::ceremony_shamir_params(&bot_pk(1)), Some((2, 3)));
	});
}

#[test]
fn ceremony_provider_trait() {
	new_test_ext().execute_with(|| {
		use pallet_grouprobot_primitives::CeremonyProvider;

		assert!(!<GroupRobotCeremony as CeremonyProvider>::is_ceremony_active(&bot_pk(1)));
		assert_eq!(<GroupRobotCeremony as CeremonyProvider>::active_ceremony_hash(&bot_pk(1)), None);
		assert_eq!(<GroupRobotCeremony as CeremonyProvider>::ceremony_participant_count(&bot_pk(1)), None);

		assert_ok!(GroupRobotCeremony::approve_ceremony_enclave(
			RuntimeOrigin::root(), mrenclave(1), 1, vec![]
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));

		assert!(<GroupRobotCeremony as CeremonyProvider>::is_ceremony_active(&bot_pk(1)));
		assert_eq!(
			<GroupRobotCeremony as CeremonyProvider>::ceremony_shamir_params(&bot_pk(1)),
			Some((2, 3))
		);
		assert_eq!(
			<GroupRobotCeremony as CeremonyProvider>::active_ceremony_hash(&bot_pk(1)),
			Some(ceremony_hash(1))
		);
		assert_eq!(
			<GroupRobotCeremony as CeremonyProvider>::ceremony_participant_count(&bot_pk(1)),
			Some(1)
		);
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
				ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(1)).unwrap();
		assert!(!record.is_re_ceremony);
		assert_eq!(record.supersedes, None);
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
			ceremony_hash(1), mrenclave(1), 2, 3, bot_pk(1), vec![[10u8; 32]], bot_id(1),
		));
		assert_ok!(GroupRobotCeremony::record_ceremony(
			RuntimeOrigin::signed(OWNER),
			ceremony_hash(2), mrenclave(1), 3, 5, bot_pk(1), vec![[10u8; 32], [11u8; 32]], bot_id(1),
		));

		let record = Ceremonies::<Test>::get(ceremony_hash(2)).unwrap();
		assert!(record.is_re_ceremony);
		assert_eq!(record.supersedes, Some(ceremony_hash(1)));
	});
}
