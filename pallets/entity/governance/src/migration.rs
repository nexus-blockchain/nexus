//! Storage migrations for governance pallet
//!
//! v1 adds `LastProposalCreatedAt` storage item (no data migration needed,
//! new DoubleMap starts empty) and updates STORAGE_VERSION from 0 to 1.
//! v2 updates the stored `ProposalType` schema for KYC provider governance
//! proposals from `u64 provider_id` to `AccountId` payloads, and bumps
//! STORAGE_VERSION from 1 to 2. This migration performs only the version bump;
//! operator is responsible for ensuring no incompatible legacy stored proposals
//! remain before upgrade.

use frame_support::{
    pallet_prelude::*,
    traits::{OnRuntimeUpgrade, StorageVersion},
    weights::Weight,
};

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

use crate::{Config, Pallet};

pub struct MigrateV0ToV1<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateV0ToV1<T> {
    fn on_runtime_upgrade() -> Weight {
        let on_chain = StorageVersion::get::<Pallet<T>>();
        if on_chain == 0 {
            StorageVersion::new(1).put::<Pallet<T>>();
            T::DbWeight::get().reads_writes(1, 1)
        } else {
            T::DbWeight::get().reads(1)
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, TryRuntimeError> {
        let _on_chain = StorageVersion::get::<Pallet<T>>();
        Ok(sp_std::vec::Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), TryRuntimeError> {
        let on_chain = StorageVersion::get::<Pallet<T>>();
        assert_eq!(
            on_chain,
            StorageVersion::new(1),
            "post_upgrade: STORAGE_VERSION should be 1"
        );
        Ok(())
    }
}

pub struct MigrateV1ToV2<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateV1ToV2<T> {
    fn on_runtime_upgrade() -> Weight {
        let on_chain = StorageVersion::get::<Pallet<T>>();
        if on_chain == 1 {
            StorageVersion::new(2).put::<Pallet<T>>();
            T::DbWeight::get().reads_writes(1, 1)
        } else {
            T::DbWeight::get().reads(1)
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, TryRuntimeError> {
        let on_chain = StorageVersion::get::<Pallet<T>>();
        ensure!(
            on_chain == 1,
            "pre_upgrade: expected STORAGE_VERSION 1 before v2 migration"
        );
        Ok(sp_std::vec::Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), TryRuntimeError> {
        let on_chain = StorageVersion::get::<Pallet<T>>();
        assert_eq!(
            on_chain,
            StorageVersion::new(2),
            "post_upgrade: STORAGE_VERSION should be 2"
        );
        Ok(())
    }
}
