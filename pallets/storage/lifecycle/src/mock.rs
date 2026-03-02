use crate as pallet_storage_lifecycle;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        StorageLifecycle: pallet_storage_lifecycle,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const L1ArchiveDelay: u32 = 100;
    pub const L2ArchiveDelay: u32 = 200;
    pub const PurgeDelay: u32 = 300;
    pub const EnablePurge: bool = true;
    pub const MaxBatchSize: u32 = 10;
}

use std::cell::RefCell;

thread_local! {
    static ARCHIVABLE_IDS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
    static ARCHIVED_IDS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
}

pub fn set_archivable_ids(ids: Vec<u64>) {
    ARCHIVABLE_IDS.with(|a| *a.borrow_mut() = ids);
}

pub fn get_archived_ids() -> Vec<u64> {
    ARCHIVED_IDS.with(|a| a.borrow().clone())
}

pub struct MockStorageArchiver;

impl pallet_storage_lifecycle::StorageArchiver for MockStorageArchiver {
    fn scan_archivable(_delay: u64, max_count: u32) -> Vec<u64> {
        ARCHIVABLE_IDS.with(|a| {
            let ids = a.borrow();
            ids.iter().take(max_count as usize).copied().collect()
        })
    }

    fn archive_records(ids: &[u64]) {
        ARCHIVED_IDS.with(|a| a.borrow_mut().extend_from_slice(ids));
        ARCHIVABLE_IDS.with(|a| {
            let mut v = a.borrow_mut();
            v.retain(|id| !ids.contains(id));
        });
    }
}

impl pallet_storage_lifecycle::pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type L1ArchiveDelay = L1ArchiveDelay;
    type L2ArchiveDelay = L2ArchiveDelay;
    type PurgeDelay = PurgeDelay;
    type EnablePurge = EnablePurge;
    type MaxBatchSize = MaxBatchSize;
    type StorageArchiver = MockStorageArchiver;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    ARCHIVABLE_IDS.with(|a| a.borrow_mut().clear());
    ARCHIVED_IDS.with(|a| a.borrow_mut().clear());
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
