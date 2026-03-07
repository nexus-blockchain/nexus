//! Benchmarking for pallet-entity-registry.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! Governance extrinsics (via GovernanceOrigin = EnsureRoot) are fully benchmarked.
//! Signed extrinsics that depend on mock providers (PricingProvider, ShopProvider)
//! use seed helpers to pre-populate storage directly, bypassing `create_entity`.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode};

fn seed_entity<T: Config>(entity_id: u64) {
    use frame_support::BoundedVec;
    let name: BoundedVec<u8, T::MaxEntityNameLength> =
        b"bench-entity".to_vec().try_into().expect("name fits");
    let entity = Entity {
        id: entity_id,
        owner: frame_benchmarking::account::<T::AccountId>("owner", 0, 0),
        name,
        logo_cid: None,
        description_cid: None,
        status: EntityStatus::Active,
        created_at: frame_system::Pallet::<T>::block_number(),
        entity_type: EntityType::Merchant,
        admins: BoundedVec::default(),
        governance_mode: GovernanceMode::None,
        verified: false,
        metadata_uri: None,
        contact_cid: None,
        primary_shop_id: 0,
    };
    Entities::<T>::insert(entity_id, entity);
    NextEntityId::<T>::put(entity_id.saturating_add(1));
}

fn seed_entity_with_status<T: Config>(entity_id: u64, status: EntityStatus) {
    seed_entity::<T>(entity_id);
    Entities::<T>::mutate(entity_id, |e| {
        if let Some(e) = e {
            e.status = status;
        }
    });
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn approve_entity() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::Pending);
        let treasury = Pallet::<T>::entity_treasury_account(entity_id);
        let min_bal = T::MinOperatingBalance::get();
        let _ = T::Currency::deposit_creating(&treasury, min_bal.saturating_mul(10u32.into()));

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
    }

    #[benchmark]
    fn approve_close_entity() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::PendingClose);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Closed);
    }

    #[benchmark]
    fn suspend_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, None);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Suspended);
    }

    #[benchmark]
    fn resume_entity() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::Suspended);
        let treasury = Pallet::<T>::entity_treasury_account(entity_id);
        let min_bal = T::MinOperatingBalance::get();
        let _ = T::Currency::deposit_creating(&treasury, min_bal.saturating_mul(10u32.into()));

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
    }

    #[benchmark]
    fn ban_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, false, None);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Banned);
    }

    #[benchmark]
    fn verify_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(e.verified);
    }

    #[benchmark]
    fn unban_entity() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::Banned);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Pending);
    }

    #[benchmark]
    fn unverify_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);
        Entities::<T>::mutate(entity_id, |e| {
            if let Some(e) = e { e.verified = true; }
        });

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(!e.verified);
    }

    #[benchmark]
    fn force_transfer_ownership() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);
        let new_owner: T::AccountId = frame_benchmarking::account("new_owner", 0, 0);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, new_owner.clone());

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.owner, new_owner);
    }

    #[benchmark]
    fn reject_close_request() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::PendingClose);
        let treasury = Pallet::<T>::entity_treasury_account(entity_id);
        let min_bal = T::MinOperatingBalance::get();
        let _ = T::Currency::deposit_creating(&treasury, min_bal.saturating_mul(10u32.into()));

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
    }

    #[benchmark]
    fn upgrade_entity_type() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, EntityType::DAO, GovernanceMode::DualTrack);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.entity_type, EntityType::DAO);
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
