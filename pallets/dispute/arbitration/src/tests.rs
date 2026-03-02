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
