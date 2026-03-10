#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;
use types::*;

fn seed_complaint<T: Config>(id: u64, status: ComplaintStatus) {
    let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
    let respondent: T::AccountId = frame_benchmarking::account("respondent", 0, 1);
    let now = frame_system::Pallet::<T>::block_number();
    let deadline = now + T::ResponseDeadline::get();
    let complaint = Complaint::<T> {
        id,
        domain: *b"otc_ord_",
        object_id: 1,
        complaint_type: ComplaintType::OtcSellerNotDeliver,
        complainant: complainant.clone(),
        respondent,
        details_cid: BoundedVec::truncate_from(b"cid".to_vec()),
        response_cid: None,
        amount: None,
        status,
        created_at: now,
        response_deadline: deadline,
        settlement_cid: None,
        resolution_cid: None,
        appeal_cid: None,
        appellant: None,
        updated_at: now,
    };
    Complaints::<T>::insert(id, complaint);
    NextComplaintId::<T>::put(id.saturating_add(1));
    let deposit = T::ComplaintDeposit::get();
    ComplaintDeposits::<T>::insert(id, deposit);
    ActiveComplaintCount::<T>::mutate(&complainant, |c| *c = c.saturating_add(1));
}

fn seed_dispute<T: Config>(domain: [u8; 8], id: u64) {
    Disputed::<T>::insert(domain, id, ());
    PendingArbitrationDisputes::<T>::insert(domain, id, ());
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn dispute(n: Linear<0, 5>) {
        let caller: T::AccountId = frame_benchmarking::whitelisted_caller();
        let domain: [u8; 8] = *b"otc_ord_";
        #[extrinsic_call]
        dispute(RawOrigin::Signed(caller), domain, 1, alloc::vec::Vec::new());
    }

    #[benchmark]
    fn arbitrate() {
        let domain: [u8; 8] = *b"otc_ord_";
        seed_dispute::<T>(domain, 1);
        #[extrinsic_call]
        _(RawOrigin::Root, domain, 1, 1u8, None);
    }

    #[benchmark]
    fn file_complaint() {
        let caller: T::AccountId = frame_benchmarking::whitelisted_caller();
        let cid = BoundedVec::truncate_from(b"details".to_vec());
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), *b"otc_ord_", 1, ComplaintType::OtcSellerNotDeliver, cid, None);
    }

    #[benchmark]
    fn respond_to_complaint() {
        seed_complaint::<T>(0, ComplaintStatus::Submitted);
        let respondent: T::AccountId = frame_benchmarking::account("respondent", 0, 1);
        let cid = BoundedVec::truncate_from(b"response".to_vec());
        #[extrinsic_call]
        _(RawOrigin::Signed(respondent), 0, cid);
    }

    #[benchmark]
    fn withdraw_complaint() {
        seed_complaint::<T>(0, ComplaintStatus::Submitted);
        let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
        #[extrinsic_call]
        _(RawOrigin::Signed(complainant), 0);
    }

    #[benchmark]
    fn settle_complaint() {
        seed_complaint::<T>(0, ComplaintStatus::Responded);
        let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
        let cid = BoundedVec::truncate_from(b"settlement".to_vec());
        #[extrinsic_call]
        _(RawOrigin::Signed(complainant), 0, cid);
    }

    #[benchmark]
    fn escalate_to_arbitration() {
        seed_complaint::<T>(0, ComplaintStatus::Responded);
        let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
        #[extrinsic_call]
        _(RawOrigin::Signed(complainant), 0);
    }

    #[benchmark]
    fn request_default_judgment() {
        let domain: [u8; 8] = *b"otc_ord_";
        let initiator: T::AccountId = frame_benchmarking::whitelisted_caller();
        let respondent: T::AccountId = frame_benchmarking::account("respondent", 0, 1);
        seed_dispute::<T>(domain, 1);
        let now = frame_system::Pallet::<T>::block_number();
        TwoWayDeposits::<T>::insert(domain, 1, TwoWayDepositRecord {
            initiator: initiator.clone(),
            initiator_deposit: BalanceOf::<T>::from(0u32),
            respondent,
            respondent_deposit: None,
            response_deadline: now,
            has_responded: false,
        });
        frame_system::Pallet::<T>::set_block_number(now + T::ResponseDeadline::get() + 1u32.into());
        #[extrinsic_call]
        _(RawOrigin::Signed(initiator), domain, 1);
    }

    #[benchmark]
    fn supplement_evidence() {
        seed_complaint::<T>(0, ComplaintStatus::Submitted);
        let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
        let cid = BoundedVec::truncate_from(b"evidence".to_vec());
        #[extrinsic_call]
        supplement_complaint_evidence(RawOrigin::Signed(complainant), 0, cid);
    }

    #[benchmark]
    fn settle_dispute() {
        let domain: [u8; 8] = *b"otc_ord_";
        let initiator: T::AccountId = frame_benchmarking::whitelisted_caller();
        let respondent: T::AccountId = frame_benchmarking::account("respondent", 0, 1);
        seed_dispute::<T>(domain, 1);
        TwoWayDeposits::<T>::insert(domain, 1, TwoWayDepositRecord {
            initiator: initiator.clone(),
            initiator_deposit: BalanceOf::<T>::from(0u32),
            respondent: respondent.clone(),
            respondent_deposit: Some(BalanceOf::<T>::from(0u32)),
            response_deadline: frame_system::Pallet::<T>::block_number(),
            has_responded: true,
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(initiator), domain, 1);
    }

    #[benchmark]
    fn start_mediation() {
        seed_complaint::<T>(0, ComplaintStatus::Responded);
        #[extrinsic_call]
        _(RawOrigin::Root, 0);
    }

    #[benchmark]
    fn dismiss_dispute() {
        let domain: [u8; 8] = *b"otc_ord_";
        seed_dispute::<T>(domain, 1);
        #[extrinsic_call]
        _(RawOrigin::Root, domain, 1);
    }

    #[benchmark]
    fn dismiss_complaint() {
        seed_complaint::<T>(0, ComplaintStatus::Arbitrating);
        #[extrinsic_call]
        _(RawOrigin::Root, 0);
    }

    #[benchmark]
    fn force_close_dispute() {
        let domain: [u8; 8] = *b"otc_ord_";
        seed_dispute::<T>(domain, 1);
        #[extrinsic_call]
        _(RawOrigin::Root, domain, 1);
    }

    #[benchmark]
    fn force_close_complaint() {
        seed_complaint::<T>(0, ComplaintStatus::Arbitrating);
        #[extrinsic_call]
        _(RawOrigin::Root, 0);
    }

    #[benchmark]
    fn appeal() {
        // Seed a resolved complaint (RespondentWin so complainant can appeal)
        seed_complaint::<T>(0, ComplaintStatus::ResolvedRespondentWin);
        // Remove the deposit record so appeal guard passes
        ComplaintDeposits::<T>::remove(0);
        let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
        let cid = BoundedVec::truncate_from(b"appeal_reason".to_vec());
        #[extrinsic_call]
        _(RawOrigin::Signed(complainant), 0, cid);
    }

    #[benchmark]
    fn resolve_appeal() {
        seed_complaint::<T>(0, ComplaintStatus::Appealed);
        let cid = BoundedVec::truncate_from(b"appeal_resolution".to_vec());
        // Set appellant
        Complaints::<T>::mutate(0, |maybe| {
            if let Some(c) = maybe.as_mut() {
                let complainant: T::AccountId = frame_benchmarking::account("complainant", 0, 0);
                c.appellant = Some(complainant);
            }
        });
        #[extrinsic_call]
        _(RawOrigin::Root, 0, 0u8, cid);
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
