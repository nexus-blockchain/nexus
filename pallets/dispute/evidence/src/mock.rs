use crate as pallet_evidence;
use frame_support::{
    derive_impl, parameter_types,
    traits::ConstU32,
    PalletId,
};
use sp_runtime::{
    traits::IdentityLookup,
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        EvidencePallet: pallet_evidence,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<u64>;
}

// -- Mock EvidenceAuthorizer --

use core::cell::RefCell;

thread_local! {
    static AUTHORIZED: RefCell<bool> = RefCell::new(true);
}

pub fn set_authorized(val: bool) {
    AUTHORIZED.with(|v| *v.borrow_mut() = val);
}

pub struct MockAuthorizer;
impl pallet_evidence::pallet::EvidenceAuthorizer<u64> for MockAuthorizer {
    fn is_authorized(_ns: [u8; 8], _who: &u64) -> bool {
        AUTHORIZED.with(|v| *v.borrow())
    }
}

// -- Mock IpfsPinner --

pub struct MockIpfsPinner;
impl pallet_storage_service::IpfsPinner<u64, u128> for MockIpfsPinner {
    fn pin_cid_for_subject(
        _caller: u64,
        _subject_type: pallet_storage_service::SubjectType,
        _subject_id: u64,
        _cid: Vec<u8>,
        _tier: Option<pallet_storage_service::PinTier>,
    ) -> sp_runtime::DispatchResult {
        Ok(())
    }
    fn unpin_cid(_caller: u64, _cid: Vec<u8>) -> sp_runtime::DispatchResult {
        Ok(())
    }
}

// -- Evidence Config --

parameter_types! {
    pub const EvidenceNsBytes: [u8; 8] = *b"evidence";
    pub const EnableGlobalCidDedup: bool = true;
    pub const DefaultStoragePrice: u128 = 100;
    pub const WindowBlocks: u64 = 100;
    pub const EvidenceEditWindow: u64 = 200;
}

impl pallet_evidence::pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxContentCidLen = ConstU32<64>;
    type MaxSchemeLen = ConstU32<32>;
    type MaxCidLen = ConstU32<64>;
    type MaxImg = ConstU32<10>;
    type MaxVid = ConstU32<5>;
    type MaxDoc = ConstU32<5>;
    type MaxMemoLen = ConstU32<256>;
    type MaxAuthorizedUsers = ConstU32<20>;
    type MaxKeyLen = ConstU32<128>;
    type EvidenceNsBytes = EvidenceNsBytes;
    type Authorizer = MockAuthorizer;
    type MaxPerSubjectTarget = ConstU32<100>;
    type MaxPerSubjectNs = ConstU32<100>;
    type WindowBlocks = WindowBlocks;
    type MaxPerWindow = ConstU32<50>;
    type EnableGlobalCidDedup = EnableGlobalCidDedup;
    type MaxListLen = ConstU32<50>;
    type WeightInfo = ();
    type IpfsPinner = MockIpfsPinner;
    type Balance = u128;
    type DefaultStoragePrice = DefaultStoragePrice;
    type EvidenceEditWindow = EvidenceEditWindow;
    type ArchiveTtlBlocks = ConstU32<1000>;
    type ArchiveDelayBlocks = ConstU32<50>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    AUTHORIZED.with(|v| *v.borrow_mut() = true);

    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
