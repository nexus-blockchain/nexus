//! Pallet arbitration 权重接口与手写估算实现（上线前用 benchmark 生成替换）。
use core::marker::PhantomData;
use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};

pub trait WeightInfo {
    /// dispute / dispute_with_evidence_id / append_evidence_id / dispute_with_two_way_deposit / respond_to_dispute
    fn dispute(evidence_count: u32) -> Weight;
    /// arbitrate / resolve_complaint
    fn arbitrate() -> Weight;
    /// file_complaint (含押金锁定)
    fn file_complaint() -> Weight;
    /// respond_to_complaint
    fn respond_to_complaint() -> Weight;
    /// withdraw_complaint (含押金退还)
    fn withdraw_complaint() -> Weight;
    /// settle_complaint (含押金退还)
    fn settle_complaint() -> Weight;
    /// escalate_to_arbitration
    fn escalate_to_arbitration() -> Weight;
    /// 🆕 F1: request_default_judgment
    fn request_default_judgment() -> Weight;
    /// 🆕 F2/F5: supplement_complaint_evidence / supplement_response_evidence
    fn supplement_evidence() -> Weight;
    /// 🆕 F3: settle_dispute
    fn settle_dispute() -> Weight;
    /// 🆕 F8: start_mediation
    fn start_mediation() -> Weight;
    /// 🆕 F10: dismiss_dispute
    fn dismiss_dispute() -> Weight;
    /// 🆕 F10: dismiss_complaint
    fn dismiss_complaint() -> Weight;
    /// 🆕 F12: force_close_dispute
    fn force_close_dispute() -> Weight;
    /// 🆕 F12: force_close_complaint
    fn force_close_complaint() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn dispute(evidence_count: u32) -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(Weight::from_parts(5_000_000, 0).saturating_mul(evidence_count as u64))
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn arbitrate() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(5_u64))
    }
    fn file_complaint() -> Weight {
        // +F4 RespondentActiveComplaints(w), +F6 ObjectComplaints(w) = 8 writes
        Weight::from_parts(75_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(8_u64))
    }
    fn respond_to_complaint() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn withdraw_complaint() -> Weight {
        // +L4: UserActiveComplaints(w), RespondentActiveComplaints(w), ObjectComplaints(w)
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }
    fn settle_complaint() -> Weight {
        // +L4: 3 index cleanup writes
        Weight::from_parts(65_000_000, 5_500)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(7_u64))
    }
    fn escalate_to_arbitration() -> Weight {
        // +F7 PendingArbitrationComplaints(w) = 2 writes
        Weight::from_parts(45_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn request_default_judgment() -> Weight {
        // TwoWayDeposits(r), Router(r+w), Fungible(r+w), CidLock(r+w), Archive(w)
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(5_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }
    fn supplement_evidence() -> Weight {
        // Complaints(r) + Paused(r) = 2
        Weight::from_parts(35_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(0_u64))
    }
    fn settle_dispute() -> Weight {
        // Disputed(r), TwoWayDeposits(r), Fungible release x2
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }
    fn start_mediation() -> Weight {
        // Complaints(r+w)
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn dismiss_dispute() -> Weight {
        // Same as arbitrate
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(5_u64))
    }
    fn dismiss_complaint() -> Weight {
        // Complaints(r+w), ComplaintDeposits(r+w), DomainPenaltyRates(r), DomainStats(w), Fungible
        // +L4: 3 index cleanup writes
        Weight::from_parts(75_000_000, 6_500)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(7_u64))
    }
    fn force_close_dispute() -> Weight {
        // Same as arbitrate
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(5_u64))
    }
    fn force_close_complaint() -> Weight {
        // +H1-R2: 3 index cleanup writes
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }
}

impl WeightInfo for () {
    fn dispute(evidence_count: u32) -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(Weight::from_parts(5_000_000, 0).saturating_mul(evidence_count as u64))
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
    fn arbitrate() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(5_u64))
    }
    fn file_complaint() -> Weight {
        Weight::from_parts(75_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(8_u64))
    }
    fn respond_to_complaint() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn withdraw_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn settle_complaint() -> Weight {
        Weight::from_parts(65_000_000, 5_500)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(7_u64))
    }
    fn escalate_to_arbitration() -> Weight {
        Weight::from_parts(45_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn request_default_judgment() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(5_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn supplement_evidence() -> Weight {
        Weight::from_parts(35_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(0_u64))
    }
    fn settle_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn start_mediation() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn dismiss_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(5_u64))
    }
    fn dismiss_complaint() -> Weight {
        Weight::from_parts(75_000_000, 6_500)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(7_u64))
    }
    fn force_close_dispute() -> Weight {
        Weight::from_parts(85_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(5_u64))
    }
    fn force_close_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_500)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
}
