use frame_support::{
    pallet_prelude::*,
    traits::{OnRuntimeUpgrade, StorageVersion},
    weights::Weight,
};

use super::*;

pub mod v1 {
    use super::*;

    pub struct MigrateToV1<T>(core::marker::PhantomData<T>);

    impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
        fn on_runtime_upgrade() -> Weight {
            let on_chain = StorageVersion::get::<Pallet<T>>();
            if on_chain >= 1 {
                log::info!(target: "storage-service", "skipping v1 migration (on-chain={:?})", on_chain);
                return Weight::zero();
            }

            log::info!(target: "storage-service", "running v1 migration: cleanup dead storage");

            let mut reads: u64 = 1;
            let mut writes: u64 = 1;

            // Populate RegisteredDomainList from RegisteredDomains iterator (one-time)
            let mut list: Vec<BoundedVec<u8, ConstU32<32>>> = Vec::new();
            for (domain, _) in pallet::RegisteredDomains::<T>::iter() {
                list.push(domain);
                reads += 1;
            }
            if let Ok(bounded) = BoundedVec::try_from(list) {
                pallet::RegisteredDomainList::<T>::put(bounded);
                writes += 1;
            }

            StorageVersion::new(1).put::<Pallet<T>>();

            T::DbWeight::get().reads_writes(reads, writes)
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
            let on_chain = StorageVersion::get::<Pallet<T>>();
            log::info!(target: "storage-service", "pre_upgrade: on-chain version = {:?}", on_chain);
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
            let on_chain = StorageVersion::get::<Pallet<T>>();
            assert_eq!(on_chain, 1, "expected v1 after migration");
            log::info!(target: "storage-service", "post_upgrade: version is now {:?}", on_chain);
            Ok(())
        }
    }
}
