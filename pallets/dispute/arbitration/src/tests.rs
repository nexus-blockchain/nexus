use crate::{mock::*, pallet::*};
use frame_support::{assert_ok, assert_noop, BoundedVec};
use frame_system::RawOrigin;

const DOMAIN: [u8; 8] = *b"otc_ord_";

fn cid(s: &[u8]) -> BoundedVec<u8, <Test as Config>::MaxCidLen> {
    BoundedVec::truncate_from(s.to_vec())
}

// ==================== dispute (call_index 0) ====================

#[test]
fn dispute_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1),
            DOMAIN,
            1,
            vec![cid(b"evidence_cid_1")],
        ));
        assert!(Disputed::<Test>::get(DOMAIN, 1).is_some());
    });
}

#[test]
fn dispute_already_disputed() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        assert_noop!(
            Arbitration::dispute(RuntimeOrigin::signed(1), DOMAIN, 1, vec![]),
            Error::<Test>::AlreadyDisputed
        );
    });
}

// ==================== arbitrate (call_index 1) ====================

#[test]
fn arbitrate_release_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        assert_ok!(Arbitration::arbitrate(
            RawOrigin::Root.into(), DOMAIN, 1, 0, None,
        ));
        // Dispute should be cleaned up (archived)
        assert!(Disputed::<Test>::get(DOMAIN, 1).is_none());
        // Stats should be updated
        let stats = ArbitrationStats::<Test>::get();
        assert_eq!(stats.total_disputes, 1);
        assert_eq!(stats.release_count, 1);
    });
}

#[test]
fn arbitrate_refund_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        assert_ok!(Arbitration::arbitrate(
            RawOrigin::Root.into(), DOMAIN, 1, 1, None,
        ));
        let stats = ArbitrationStats::<Test>::get();
        assert_eq!(stats.refund_count, 1);
    });
}

#[test]
fn arbitrate_partial_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        assert_ok!(Arbitration::arbitrate(
            RawOrigin::Root.into(), DOMAIN, 1, 2, Some(5000),
        ));
        let stats = ArbitrationStats::<Test>::get();
        assert_eq!(stats.partial_count, 1);
    });
}

#[test]
fn arbitrate_not_disputed() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Arbitration::arbitrate(RawOrigin::Root.into(), DOMAIN, 1, 0, None),
            Error::<Test>::NotDisputed
        );
    });
}

#[test]
fn arbitrate_requires_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        assert!(Arbitration::arbitrate(
            RuntimeOrigin::signed(1), DOMAIN, 1, 0, None,
        ).is_err());
    });
}

// ==================== dispute_with_evidence_id (call_index 2) ====================

#[test]
fn dispute_with_evidence_id_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute_with_evidence_id(
            RuntimeOrigin::signed(1), DOMAIN, 1, 42,
        ));
        assert!(Disputed::<Test>::get(DOMAIN, 1).is_some());
        let evidence_ids = EvidenceIds::<Test>::get(DOMAIN, 1);
        assert_eq!(evidence_ids.to_vec(), vec![42]);
    });
}

#[test]
fn dispute_with_evidence_id_rejects_nonexistent_evidence() {
    new_test_ext().execute_with(|| {
        set_evidence_exists(false);
        assert_noop!(
            Arbitration::dispute_with_evidence_id(
                RuntimeOrigin::signed(1), DOMAIN, 1, 42,
            ),
            Error::<Test>::EvidenceNotFound
        );
    });
}

// ==================== append_evidence_id (call_index 3) ====================

#[test]
fn append_evidence_id_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        assert_ok!(Arbitration::append_evidence_id(
            RuntimeOrigin::signed(1), DOMAIN, 1, 100,
        ));
        let evidence_ids = EvidenceIds::<Test>::get(DOMAIN, 1);
        assert_eq!(evidence_ids.to_vec(), vec![100]);
    });
}

#[test]
fn append_evidence_id_not_disputed() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Arbitration::append_evidence_id(
                RuntimeOrigin::signed(1), DOMAIN, 1, 100,
            ),
            Error::<Test>::NotDisputed
        );
    });
}

// ==================== file_complaint (call_index 10) ====================

#[test]
fn file_complaint_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1),
            DOMAIN,
            1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details_cid"),
            Some(500),
        ));
        let complaint = Complaints::<Test>::get(0).expect("complaint should exist");
        assert_eq!(complaint.complainant, 1);
        assert_eq!(complaint.respondent, 2);
        assert_eq!(complaint.status, ComplaintStatus::Submitted);
        assert_eq!(complaint.domain, DOMAIN);

        // Deposit should be recorded
        assert!(ComplaintDeposits::<Test>::get(0).is_some());

        // Domain stats updated
        let stats = DomainStats::<Test>::get(DOMAIN);
        assert_eq!(stats.total_complaints, 1);
    });
}

#[test]
fn file_complaint_invalid_type_for_domain() {
    new_test_ext().execute_with(|| {
        // LiveIllegalContent belongs to livstrm_ domain, not otc_ord_
        assert_noop!(
            Arbitration::file_complaint(
                RuntimeOrigin::signed(1),
                DOMAIN,
                1,
                ComplaintType::LiveIllegalContent,
                cid(b"details_cid"),
                None,
            ),
            Error::<Test>::InvalidComplaintType
        );
    });
}

// ==================== respond_to_complaint (call_index 11) ====================

#[test]
fn respond_to_complaint_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Responded);
        assert!(complaint.response_cid.is_some());
    });
}

#[test]
fn respond_to_complaint_not_respondent() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_noop!(
            Arbitration::respond_to_complaint(
                RuntimeOrigin::signed(3), 0, cid(b"response"),
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn respond_to_complaint_past_deadline() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        // Advance past deadline (ResponseDeadline = 100)
        System::set_block_number(200);
        assert_noop!(
            Arbitration::respond_to_complaint(
                RuntimeOrigin::signed(2), 0, cid(b"response"),
            ),
            Error::<Test>::ResponseDeadlinePassed
        );
    });
}

// ==================== withdraw_complaint (call_index 12) ====================

#[test]
fn withdraw_complaint_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::withdraw_complaint(
            RuntimeOrigin::signed(1), 0,
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Withdrawn);
        // Deposit should be returned
        assert!(ComplaintDeposits::<Test>::get(0).is_none());
    });
}

#[test]
fn withdraw_complaint_not_complainant() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_noop!(
            Arbitration::withdraw_complaint(RuntimeOrigin::signed(2), 0),
            Error::<Test>::NotAuthorized
        );
    });
}

// ==================== settle_complaint (call_index 13) ====================

#[test]
fn settle_complaint_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::settle_complaint(
            RuntimeOrigin::signed(1), 0, cid(b"settlement"),
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::ResolvedSettlement);
        assert!(complaint.settlement_cid.is_some());
    });
}

// ==================== escalate_to_arbitration (call_index 14) ====================

#[test]
fn escalate_to_arbitration_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Arbitrating);
    });
}

#[test]
fn escalate_wrong_status() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        // Can't escalate from Submitted state
        assert_noop!(
            Arbitration::escalate_to_arbitration(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidState
        );
    });
}

// ==================== resolve_complaint (call_index 15) ====================

#[test]
fn resolve_complaint_complainant_wins() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));
        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 0, cid(b"reason"), None,
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::ResolvedComplainantWin);
        // Deposit returned to complainant
        assert!(ComplaintDeposits::<Test>::get(0).is_none());
    });
}

#[test]
fn resolve_complaint_respondent_wins_slashes_deposit() {
    new_test_ext().execute_with(|| {
        let balance_before = Balances::free_balance(1);
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        let balance_after_filing = Balances::free_balance(1);
        let deposit = balance_before - balance_after_filing;
        assert!(deposit > 0);

        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));
        // decision=1 means respondent wins, slash complainant deposit
        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 1, cid(b"reason"), None,
        ));
        // Deposit partially slashed (50% to respondent per ComplaintSlashBps=5000)
        let balance_final = Balances::free_balance(1);
        // balance_final should be less than balance_before (some deposit slashed)
        assert!(balance_final < balance_before);
    });
}

// ==================== expire_old_complaints ====================

#[test]
fn expire_old_complaints_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        // Advance past deadline
        System::set_block_number(200);
        let expired = Arbitration::expire_old_complaints(10);
        assert_eq!(expired, 1);
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Expired);
    });
}

// ==================== archive_old_complaints ====================

#[test]
fn archive_old_complaints_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::withdraw_complaint(
            RuntimeOrigin::signed(1), 0,
        ));
        // Advance past archive delay (ComplaintArchiveDelayBlocks=50)
        System::set_block_number(100);
        let archived = Arbitration::archive_old_complaints(10);
        assert_eq!(archived, 1);
        // Active complaint removed
        assert!(Complaints::<Test>::get(0).is_none());
        // Archived complaint exists
        assert!(ArchivedComplaints::<Test>::get(0).is_some());
    });
}

// ==================== ComplaintType domain matching ====================

#[test]
fn complaint_type_domain_mapping() {
    assert_eq!(ComplaintType::OtcSellerNotDeliver.domain(), *b"otc_ord_");
    assert_eq!(ComplaintType::LiveIllegalContent.domain(), *b"livstrm_");
    assert_eq!(ComplaintType::MakerCreditDefault.domain(), *b"maker___");
    assert_eq!(ComplaintType::Other.domain(), *b"other___");
}

#[test]
fn complaint_status_is_resolved() {
    assert!(ComplaintStatus::ResolvedComplainantWin.is_resolved());
    assert!(ComplaintStatus::ResolvedRespondentWin.is_resolved());
    assert!(ComplaintStatus::ResolvedSettlement.is_resolved());
    assert!(ComplaintStatus::Withdrawn.is_resolved());
    assert!(ComplaintStatus::Expired.is_resolved());
    assert!(!ComplaintStatus::Submitted.is_resolved());
    assert!(!ComplaintStatus::Responded.is_resolved());
    assert!(!ComplaintStatus::Arbitrating.is_resolved());
}

// ==================== Regression: H1 — evidence validation before state mutations ====================

#[test]
fn h1_dispute_with_two_way_deposit_rejects_bad_evidence_no_state_change() {
    new_test_ext().execute_with(|| {
        set_evidence_exists(false);
        let balance_before = Balances::free_balance(1);
        assert_noop!(
            Arbitration::dispute_with_two_way_deposit(
                RuntimeOrigin::signed(1), DOMAIN, 1, 999,
            ),
            Error::<Test>::EvidenceNotFound
        );
        // No state mutation: no dispute recorded, balance unchanged
        assert!(Disputed::<Test>::get(DOMAIN, 1).is_none());
        assert_eq!(Balances::free_balance(1), balance_before);
    });
}

// ==================== Regression: H2 — counter_evidence validation before state mutations ====================

#[test]
fn h2_respond_to_dispute_rejects_bad_evidence_no_state_change() {
    new_test_ext().execute_with(|| {
        // First create a valid dispute
        assert_ok!(Arbitration::dispute_with_two_way_deposit(
            RuntimeOrigin::signed(1), DOMAIN, 1, 42,
        ));
        // Now set evidence to not exist for the response
        set_evidence_exists(false);
        assert_noop!(
            Arbitration::respond_to_dispute(
                RuntimeOrigin::signed(2), DOMAIN, 1, 100,
            ),
            Error::<Test>::EvidenceNotFound
        );
        // TwoWayDeposit should still not have responded
        let deposit = TwoWayDeposits::<Test>::get(DOMAIN, 1).unwrap();
        assert!(!deposit.has_responded);
    });
}

// ==================== Regression: M2 — append_evidence_id blocked when paused ====================

#[test]
fn m2_append_evidence_id_blocked_when_paused() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::dispute(
            RuntimeOrigin::signed(1), DOMAIN, 1, vec![],
        ));
        // Pause the module
        assert_ok!(Arbitration::set_paused(RawOrigin::Root.into(), true));
        assert_noop!(
            Arbitration::append_evidence_id(
                RuntimeOrigin::signed(1), DOMAIN, 1, 100,
            ),
            Error::<Test>::ModulePaused
        );
    });
}

// ==================== Regression: L1 — DomainPenaltyRates overrides ComplaintSlashBps ====================

#[test]
fn l1_domain_penalty_rate_overrides_complaint_slash() {
    new_test_ext().execute_with(|| {
        // Set domain-specific penalty rate to 8000 bps (80%) instead of default 5000 (50%)
        assert_ok!(Arbitration::set_domain_penalty_rate(
            RawOrigin::Root.into(), DOMAIN, Some(8000),
        ));

        let balance_before = Balances::free_balance(1);
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        let balance_after_filing = Balances::free_balance(1);
        let deposit = balance_before - balance_after_filing;
        assert!(deposit > 0);

        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));

        let respondent_before = Balances::free_balance(2);

        // decision=1 → respondent wins → slash complainant deposit at domain rate (80%)
        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 1, cid(b"reason"), None,
        ));

        let respondent_after = Balances::free_balance(2);
        let slashed_to_respondent = respondent_after - respondent_before;

        // 80% of deposit should go to respondent (DomainPenaltyRate=8000)
        let expected_slash = sp_runtime::Permill::from_parts(8000u32 * 100).mul_floor(deposit);
        assert_eq!(slashed_to_respondent, expected_slash);
    });
}

// ==================== Regression: L4 — immediate user index cleanup ====================

#[test]
fn l4_withdraw_complaint_cleans_user_indexes_immediately() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        // Verify indexes exist
        assert!(!UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(!RespondentActiveComplaints::<Test>::get(2).is_empty());

        assert_ok!(Arbitration::withdraw_complaint(
            RuntimeOrigin::signed(1), 0,
        ));

        // Indexes should be cleaned immediately, not waiting for archive
        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());
    });
}

#[test]
fn l4_settle_complaint_cleans_user_indexes_immediately() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));

        assert!(!UserActiveComplaints::<Test>::get(1).is_empty());

        assert_ok!(Arbitration::settle_complaint(
            RuntimeOrigin::signed(1), 0, cid(b"settlement"),
        ));

        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());
    });
}

#[test]
fn l4_resolve_complaint_cleans_user_indexes_immediately() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));

        assert!(!UserActiveComplaints::<Test>::get(1).is_empty());

        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 0, cid(b"reason"), None,
        ));

        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());
    });
}

#[test]
fn l4_dismiss_complaint_cleans_user_indexes_immediately() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));

        assert!(!UserActiveComplaints::<Test>::get(1).is_empty());

        assert_ok!(Arbitration::dismiss_complaint(
            RawOrigin::Root.into(), 0,
        ));

        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());
    });
}

#[test]
fn l4_expire_complaint_cleans_user_indexes_immediately() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));

        assert!(!UserActiveComplaints::<Test>::get(1).is_empty());

        // Advance past deadline
        System::set_block_number(200);
        let expired = Arbitration::expire_old_complaints(10);
        assert_eq!(expired, 1);

        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());
    });
}

// ==================== Regression: H1-R2 — force_close_complaint cleans user indexes ====================

#[test]
fn h1r2_force_close_complaint_cleans_user_indexes() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));

        // Verify indexes populated
        assert!(!UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(!RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(!ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());

        assert_ok!(Arbitration::force_close_complaint(
            RawOrigin::Root.into(), 0,
        ));

        // Indexes must be cleaned immediately
        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
        assert!(ObjectComplaints::<Test>::get(DOMAIN, 1).is_empty());
    });
}

// ==================== Regression: H2-R2 — respondent cannot settle Mediating complaint ====================

#[test]
fn h2r2_respondent_cannot_settle_mediating_complaint() {
    new_test_ext().execute_with(|| {
        // File → Respond → Mediate
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::start_mediation(
            RawOrigin::Root.into(), 0,
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Mediating);

        // Respondent (account 2) tries to settle — should fail
        assert_noop!(
            Arbitration::settle_complaint(
                RuntimeOrigin::signed(2), 0, cid(b"settlement"),
            ),
            Error::<Test>::NotAuthorized
        );

        // Complainant (account 1) CAN still settle
        assert_ok!(Arbitration::settle_complaint(
            RuntimeOrigin::signed(1), 0, cid(b"settlement"),
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::ResolvedSettlement);
    });
}

#[test]
fn h2r2_respondent_can_still_settle_responded_complaint() {
    new_test_ext().execute_with(|| {
        // File → Respond (no mediation)
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));

        // Respondent CAN settle from Responded state (unchanged behavior)
        assert_ok!(Arbitration::settle_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"settlement"),
        ));
        let complaint = Complaints::<Test>::get(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::ResolvedSettlement);
    });
}

// ==================== Regression: M2-R2 — expire skips non-monotonic deadlines ====================

#[test]
fn m2r2_expire_does_not_skip_later_expired_complaint() {
    new_test_ext().execute_with(|| {
        // File complaint 0 at block 1 → deadline = 1 + 100 = 101
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));

        // Respond to complaint 0 so it's no longer Submitted (won't expire)
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));

        // File complaint 1 at block 1 → deadline = 1 + 100 = 101
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 2,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details2"), None,
        ));

        // Advance past deadline
        System::set_block_number(200);

        // Complaint 0 is Responded (skipped), complaint 1 is Submitted+expired
        // Without the M2-R2 fix, the old break logic would stop at complaint 0
        // (which is not Submitted) and never check complaint 1
        let expired = Arbitration::expire_old_complaints(10);
        assert_eq!(expired, 1);

        // Complaint 1 should be expired
        let c1 = Complaints::<Test>::get(1).unwrap();
        assert_eq!(c1.status, ComplaintStatus::Expired);
    });
}

// ==================== Regression: M-1 — per-type penalty_rate used when no domain override ====================

#[test]
fn m1_per_type_penalty_rate_used_without_domain_override() {
    new_test_ext().execute_with(|| {
        // OtcTradeFraud has penalty_rate() = 8000 (80%)
        // No DomainPenaltyRate set, so per-type rate should be used
        let balance_before = Balances::free_balance(1);
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcTradeFraud,
            cid(b"details"), None,
        ));
        let balance_after_filing = Balances::free_balance(1);
        let deposit = balance_before - balance_after_filing;
        assert!(deposit > 0);

        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));

        let respondent_before = Balances::free_balance(2);

        // decision=1 → respondent wins → slash at OtcTradeFraud penalty_rate (80%)
        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 1, cid(b"reason"), None,
        ));

        let respondent_after = Balances::free_balance(2);
        let slashed_to_respondent = respondent_after - respondent_before;

        // 80% of deposit should go to respondent (OtcTradeFraud penalty_rate=8000)
        let expected_slash = sp_runtime::Permill::from_parts(8000u32 * 100).mul_floor(deposit);
        assert_eq!(slashed_to_respondent, expected_slash);
    });
}

#[test]
fn m1_domain_override_takes_precedence_over_type_rate() {
    new_test_ext().execute_with(|| {
        // OtcTradeFraud has penalty_rate() = 8000 (80%)
        // But set domain rate to 2000 (20%) — should override
        assert_ok!(Arbitration::set_domain_penalty_rate(
            RawOrigin::Root.into(), DOMAIN, Some(2000),
        ));

        let balance_before = Balances::free_balance(1);
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcTradeFraud,
            cid(b"details"), None,
        ));
        let balance_after_filing = Balances::free_balance(1);
        let deposit = balance_before - balance_after_filing;

        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));

        let respondent_before = Balances::free_balance(2);

        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 1, cid(b"reason"), None,
        ));

        let respondent_after = Balances::free_balance(2);
        let slashed_to_respondent = respondent_after - respondent_before;

        // Domain rate 2000 (20%) should override type rate 8000 (80%)
        let expected_slash = sp_runtime::Permill::from_parts(2000u32 * 100).mul_floor(deposit);
        assert_eq!(slashed_to_respondent, expected_slash);
    });
}

#[test]
fn m1_default_type_penalty_rate_for_non_fraud() {
    new_test_ext().execute_with(|| {
        // OtcSellerNotDeliver has penalty_rate() = 3000 (30%, default)
        let balance_before = Balances::free_balance(1);
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        let balance_after_filing = Balances::free_balance(1);
        let deposit = balance_before - balance_after_filing;

        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));

        let respondent_before = Balances::free_balance(2);

        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 1, cid(b"reason"), None,
        ));

        let respondent_after = Balances::free_balance(2);
        let slashed_to_respondent = respondent_after - respondent_before;

        // Default penalty_rate = 3000 (30%) for OtcSellerNotDeliver
        let expected_slash = sp_runtime::Permill::from_parts(3000u32 * 100).mul_floor(deposit);
        assert_eq!(slashed_to_respondent, expected_slash);
    });
}

// ==================== Regression: M-2 — partial ruling stats ====================

#[test]
fn m2_partial_ruling_counts_as_complainant_win_not_settlement() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));
        assert_ok!(Arbitration::respond_to_complaint(
            RuntimeOrigin::signed(2), 0, cid(b"response"),
        ));
        assert_ok!(Arbitration::escalate_to_arbitration(
            RuntimeOrigin::signed(1), 0,
        ));

        // decision=2 → partial ruling
        assert_ok!(Arbitration::resolve_complaint(
            RawOrigin::Root.into(), 0, 2, cid(b"reason"), Some(5000),
        ));

        let stats = DomainStats::<Test>::get(DOMAIN);
        assert_eq!(stats.resolved_count, 1);
        // Partial ruling should count as complainant_win, not settlement
        assert_eq!(stats.complainant_wins, 1);
        assert_eq!(stats.settlements, 0);
        assert_eq!(stats.respondent_wins, 0);
    });
}

// ==================== Regression: L-1 — archive does not redundantly clean indexes ====================

#[test]
fn l1_archive_works_after_indexes_already_cleaned() {
    new_test_ext().execute_with(|| {
        assert_ok!(Arbitration::file_complaint(
            RuntimeOrigin::signed(1), DOMAIN, 1,
            ComplaintType::OtcSellerNotDeliver,
            cid(b"details"), None,
        ));

        // Withdraw cleans indexes immediately
        assert_ok!(Arbitration::withdraw_complaint(
            RuntimeOrigin::signed(1), 0,
        ));
        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());

        // Advance past archive delay
        System::set_block_number(100);
        let archived = Arbitration::archive_old_complaints(10);
        assert_eq!(archived, 1);

        // Complaint archived successfully even though indexes were already clean
        assert!(Complaints::<Test>::get(0).is_none());
        assert!(ArchivedComplaints::<Test>::get(0).is_some());
        // Indexes still clean (no panic from double-remove)
        assert!(UserActiveComplaints::<Test>::get(1).is_empty());
        assert!(RespondentActiveComplaints::<Test>::get(2).is_empty());
    });
}
