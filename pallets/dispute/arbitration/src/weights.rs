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
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn dispute(evidence_count: u32) -> Weight {
        // 读：Disputed(r), Router::can_dispute(r) = 2; 写：Disputed(w), EvidenceIds(w) = 2
        // 对于 two_way_deposit: 额外 Pricing(r), TwoWayDeposits(w), Fungible::hold(r+w)
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(Weight::from_parts(5_000_000, 0).saturating_mul(evidence_count as u64))
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn arbitrate() -> Weight {
        // 读：Disputed(r), TwoWayDeposits(r), Escrow::locked(r) = 4
        // 写：Disputed(w), TwoWayDeposits(w), Escrow ops(w), Router::apply_decision(r+w) = 5
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(5_u64))
    }
    fn file_complaint() -> Weight {
        // 读：Pricing(r), UserActiveComplaints(r) = 2; 写：Complaints(w), UserActiveComplaints(w), ComplaintDeposits(w), NextComplaintId(w), DomainStats(w), Fungible::hold = 6
        Weight::from_parts(70_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(6_u64))
    }
    fn respond_to_complaint() -> Weight {
        // 读：Complaints(r) = 1; 写：Complaints(w) = 1
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn withdraw_complaint() -> Weight {
        // 读：Complaints(r), ComplaintDeposits(r) = 2; 写：Complaints(w), ComplaintDeposits(w), Fungible::release = 3
        Weight::from_parts(55_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn settle_complaint() -> Weight {
        // 读：Complaints(r), ComplaintDeposits(r) = 2; 写：Complaints(w), ComplaintDeposits(w), DomainStats(w), Fungible::release = 4
        Weight::from_parts(60_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(4_u64))
    }
    fn escalate_to_arbitration() -> Weight {
        // 读：Complaints(r) = 1; 写：Complaints(w) = 1
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
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
        Weight::from_parts(70_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(6_u64))
    }
    fn respond_to_complaint() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn withdraw_complaint() -> Weight {
        Weight::from_parts(55_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
    fn settle_complaint() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(4_u64))
    }
    fn escalate_to_arbitration() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
}
