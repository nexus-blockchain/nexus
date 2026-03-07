use core::marker::PhantomData;
use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};

pub trait WeightInfo {
    fn commit(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight;
    fn commit_hash() -> Weight;
    fn commit_v2() -> Weight;
    fn link() -> Weight;
    fn link_by_ns() -> Weight;
    fn unlink() -> Weight;
    fn unlink_by_ns() -> Weight;
    fn seal_evidence(n_children: u32) -> Weight;
    fn unseal_evidence(n_children: u32) -> Weight;
    fn withdraw_evidence() -> Weight;
    fn reveal_commitment() -> Weight;
    fn register_public_key() -> Weight;
    fn store_private_content() -> Weight;
    fn grant_access() -> Weight;
    fn revoke_access() -> Weight;
    fn rotate_content_keys() -> Weight;
    fn request_access() -> Weight;
    fn update_access_policy() -> Weight;
    fn force_remove_evidence() -> Weight;
    fn force_archive_evidence() -> Weight;
    fn delete_private_content() -> Weight;
    fn revoke_public_key() -> Weight;
    fn cancel_access_request() -> Weight;
    fn append_evidence(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn commit(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight {
        let per_cid_cost = 2_000_000u64;
        let n_total = n_imgs.saturating_add(n_vids).saturating_add(n_docs);
        Weight::from_parts(10_000_000, 4_000)
            .saturating_add(Weight::from_parts(per_cid_cost, 0).saturating_mul(n_total as u64))
            .saturating_add(T::DbWeight::get().reads(3_u64.saturating_add(n_total as u64)))
            .saturating_add(T::DbWeight::get().writes(7_u64.saturating_add(n_total as u64)))
    }
    fn commit_hash() -> Weight {
        Weight::from_parts(8_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(8_u64))
    }
    fn commit_v2() -> Weight {
        Weight::from_parts(10_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(5_u64))
            .saturating_add(T::DbWeight::get().writes(9_u64))
    }
    fn link() -> Weight {
        Weight::from_parts(5_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn link_by_ns() -> Weight {
        Weight::from_parts(5_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn unlink() -> Weight {
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn unlink_by_ns() -> Weight {
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn seal_evidence(n_children: u32) -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
            .saturating_add(T::DbWeight::get().reads(n_children as u64))
            .saturating_add(T::DbWeight::get().writes(n_children as u64))
    }
    fn unseal_evidence(n_children: u32) -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
            .saturating_add(T::DbWeight::get().reads(n_children as u64))
            .saturating_add(T::DbWeight::get().writes(n_children as u64))
    }
    fn withdraw_evidence() -> Weight {
        // reads: Evidences, EvidenceStatuses; writes: indexes cleanup + status + deposit
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(7_u64))
    }
    fn reveal_commitment() -> Weight {
        Weight::from_parts(50_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn register_public_key() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn store_private_content() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(5_u64))
            .saturating_add(T::DbWeight::get().writes(4_u64))
    }
    fn grant_access() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
    fn revoke_access() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn rotate_content_keys() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn request_access() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn update_access_policy() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn force_remove_evidence() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(14_u64))
    }
    fn force_archive_evidence() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(15_u64))
    }
    fn delete_private_content() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(2_u64))
            .saturating_add(T::DbWeight::get().writes(5_u64))
    }
    fn revoke_public_key() -> Weight {
        Weight::from_parts(35_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
    fn cancel_access_request() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }
    fn append_evidence(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight {
        let per_cid_cost = 2_000_000u64;
        let n_total = n_imgs.saturating_add(n_vids).saturating_add(n_docs);
        Weight::from_parts(15_000_000, 5_000)
            .saturating_add(Weight::from_parts(per_cid_cost, 0).saturating_mul(n_total as u64))
            .saturating_add(T::DbWeight::get().reads(7_u64.saturating_add(n_total as u64)))
            .saturating_add(T::DbWeight::get().writes(11_u64.saturating_add(n_total as u64)))
    }
}

impl WeightInfo for () {
    fn commit(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight {
        let per_cid_cost = 2_000_000u64;
        let n_total = n_imgs as u64 + n_vids as u64 + n_docs as u64;
        Weight::from_parts(10_000_000, 4_000)
            .saturating_add(Weight::from_parts(per_cid_cost, 0).saturating_mul(n_total))
            .saturating_add(RocksDbWeight::get().reads(3_u64.saturating_add(n_total)))
            .saturating_add(RocksDbWeight::get().writes(7_u64.saturating_add(n_total)))
    }
    fn commit_hash() -> Weight {
        Weight::from_parts(8_000_000, 3_500)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(8_u64))
    }
    fn commit_v2() -> Weight {
        Weight::from_parts(10_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(5_u64))
            .saturating_add(RocksDbWeight::get().writes(9_u64))
    }
    fn link() -> Weight {
        Weight::from_parts(5_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn link_by_ns() -> Weight {
        Weight::from_parts(5_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn unlink() -> Weight {
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn unlink_by_ns() -> Weight {
        Weight::from_parts(4_000_000, 2_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn seal_evidence(n_children: u32) -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
            .saturating_add(RocksDbWeight::get().reads(n_children as u64))
            .saturating_add(RocksDbWeight::get().writes(n_children as u64))
    }
    fn unseal_evidence(n_children: u32) -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
            .saturating_add(RocksDbWeight::get().reads(n_children as u64))
            .saturating_add(RocksDbWeight::get().writes(n_children as u64))
    }
    fn withdraw_evidence() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(7_u64))
    }
    fn reveal_commitment() -> Weight {
        Weight::from_parts(50_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
    fn register_public_key() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn store_private_content() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(5_u64))
            .saturating_add(RocksDbWeight::get().writes(4_u64))
    }
    fn grant_access() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
    fn revoke_access() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn rotate_content_keys() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn request_access() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(4_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn update_access_policy() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn force_remove_evidence() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(14_u64))
    }
    fn force_archive_evidence() -> Weight {
        Weight::from_parts(90_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(15_u64))
    }
    fn delete_private_content() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2_u64))
            .saturating_add(RocksDbWeight::get().writes(5_u64))
    }
    fn revoke_public_key() -> Weight {
        Weight::from_parts(35_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
    fn cancel_access_request() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }
    fn append_evidence(n_imgs: u32, n_vids: u32, n_docs: u32) -> Weight {
        let per_cid_cost = 2_000_000u64;
        let n_total = n_imgs as u64 + n_vids as u64 + n_docs as u64;
        Weight::from_parts(15_000_000, 5_000)
            .saturating_add(Weight::from_parts(per_cid_cost, 0).saturating_mul(n_total))
            .saturating_add(RocksDbWeight::get().reads(7_u64.saturating_add(n_total)))
            .saturating_add(RocksDbWeight::get().writes(11_u64.saturating_add(n_total)))
    }
}
