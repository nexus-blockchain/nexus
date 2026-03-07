//! Hand-estimated weights for pallet-dispute-arbitration. Replace with benchmark-generated values before mainnet.
use core::marker::PhantomData;
use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};

pub trait WeightInfo {
    fn dispute(evidence_count: u32) -> Weight;
    fn arbitrate() -> Weight;
    fn file_complaint() -> Weight;
    fn respond_to_complaint() -> Weight;
    fn withdraw_complaint() -> Weight;
    fn settle_complaint() -> Weight;
    fn escalate_to_arbitration() -> Weight;
    fn request_default_judgment() -> Weight;
    fn supplement_evidence() -> Weight;
    fn settle_dispute() -> Weight;
    fn start_mediation() -> Weight;
    fn dismiss_dispute() -> Weight;
    fn dismiss_complaint() -> Weight;
    fn force_close_dispute() -> Weight;
    fn force_close_complaint() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    // Reads: Paused, Router::can_dispute, Disputed, EvidenceExists, Router::get_order_amount,
    //        Fungible::balance, Router::get_counterparty, TwoWayDeposits
    // Writes: Disputed, TwoWayDeposits, EvidenceIds, PendingArbitrationDisputes, Fungible::hold
    fn dispute(evidence_count: u32) -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(Weight::from_parts(5_000_000, 0).saturating_mul(evidence_count as u64))
            .saturating_add(T::DbWeight::get().reads(5_u64))
            .saturating_add(T::DbWeight::get().writes(4_u64))
    }

    // Reads: Disputed, TwoWayDeposits, Fungible, LockedCidHashes, ArbitrationStats, NextArchivedId
    // Writes: Router::apply_decision, Fungible::transfer/release, ArchivedDisputes,
    //         ArbitrationStats, Disputed, EvidenceIds, TwoWayDeposits, PendingArbitrationDisputes
    fn arbitrate() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(6_u64))
            .saturating_add(T::DbWeight::get().writes(8_u64))
    }

    // Reads: Paused, Router::can_dispute, Router::get_counterparty, ComplaintCooldown,
    //        ActiveComplaintCount, Pricing, Fungible::balance, NextComplaintId, DomainStats
    // Writes: Fungible::hold, ComplaintDeposits, Complaints, ActiveComplaintCount,
    //         DomainStats, NextComplaintId
    fn file_complaint() -> Weight {
        Weight::from_parts(75_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(6_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }

    // Reads: Paused, Complaints
    // Writes: Complaints
    fn respond_to_complaint() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }

    // Reads: Paused, Complaints, ComplaintDeposits
    // Writes: Complaints, ComplaintDeposits, Fungible::release, ActiveComplaintCount
    fn withdraw_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(4_u64))
    }

    // Reads: Paused, Complaints, ComplaintDeposits
    // Writes: Complaints, ComplaintDeposits, Fungible::release, ActiveComplaintCount,
    //         DomainStats, ComplaintCooldown
    fn settle_complaint() -> Weight {
        Weight::from_parts(65_000_000, 5_500)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }

    // Reads: Paused, Complaints
    // Writes: Complaints, PendingArbitrationComplaints
    fn escalate_to_arbitration() -> Weight {
        Weight::from_parts(45_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }

    // Reads: Paused, TwoWayDeposits, Fungible, LockedCidHashes, ArbitrationStats, NextArchivedId
    // Writes: Router::apply_decision, Fungible, ArchivedDisputes, ArbitrationStats,
    //         Disputed, EvidenceIds, TwoWayDeposits, PendingArbitrationDisputes
    fn request_default_judgment() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(6_u64))
            .saturating_add(T::DbWeight::get().writes(8_u64))
    }

    // Reads: Paused, Complaints, ComplaintEvidenceCids
    // Writes: ComplaintEvidenceCids
    fn supplement_evidence() -> Weight {
        Weight::from_parts(40_000_000, 4_500)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }

    // Reads: Paused, Disputed, TwoWayDeposits, Fungible
    // Writes: Fungible::release x2, LockedCidHashes, PendingArbitrationDisputes,
    //         Disputed, EvidenceIds, TwoWayDeposits
    fn settle_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(7_u64))
    }

    // Reads: Complaints
    // Writes: Complaints
    fn start_mediation() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }

    // Same as arbitrate (governance dispute resolution)
    fn dismiss_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(6_u64))
            .saturating_add(T::DbWeight::get().writes(8_u64))
    }

    // Reads: Complaints, ComplaintDeposits, DomainPenaltyRates, DomainStats
    // Writes: Complaints, ComplaintDeposits, Fungible::transfer+release,
    //         PendingArbitrationComplaints, ActiveComplaintCount, DomainStats
    fn dismiss_complaint() -> Weight {
        Weight::from_parts(75_000_000, 6_500)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }

    // Reads: Disputed, TwoWayDeposits, Fungible, LockedCidHashes
    // Writes: TwoWayDeposits, Fungible::release, LockedCidHashes,
    //         PendingArbitrationDisputes, Disputed, EvidenceIds
    fn force_close_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }

    // Reads: Complaints, ComplaintDeposits
    // Writes: Complaints, ComplaintDeposits, Fungible::release,
    //         PendingArbitrationComplaints, ActiveComplaintCount
    fn force_close_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(5_u64))
    }
}

impl WeightInfo for () {
    fn dispute(evidence_count: u32) -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(Weight::from_parts(5_000_000, 0).saturating_mul(evidence_count as u64))
            .saturating_add(RocksDbWeight::get().reads(5_u64))
            .saturating_add(RocksDbWeight::get().writes(4_u64))
    }
    fn arbitrate() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(6_u64))
            .saturating_add(RocksDbWeight::get().writes(8_u64))
    }
    fn file_complaint() -> Weight {
        Weight::from_parts(75_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(6_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn respond_to_complaint() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn withdraw_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(4_u64))
    }
    fn settle_complaint() -> Weight {
        Weight::from_parts(65_000_000, 5_500)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn escalate_to_arbitration() -> Weight {
        Weight::from_parts(45_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn request_default_judgment() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(6_u64))
            .saturating_add(RocksDbWeight::get().writes(8_u64))
    }
    fn supplement_evidence() -> Weight {
        Weight::from_parts(40_000_000, 4_500)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn settle_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(7_u64))
    }
    fn start_mediation() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn dismiss_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(6_u64))
            .saturating_add(RocksDbWeight::get().writes(8_u64))
    }
    fn dismiss_complaint() -> Weight {
        Weight::from_parts(75_000_000, 6_500)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn force_close_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn force_close_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(5_u64))
    }
}
