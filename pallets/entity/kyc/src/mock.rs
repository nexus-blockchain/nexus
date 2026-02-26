//! KYC 模块测试 mock

use crate as pallet_entity_kyc;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use frame_system::EnsureRoot;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

pub const ADMIN: u64 = 1;
pub const PROVIDER: u64 = 2;
pub const USER: u64 = 3;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        EntityKyc: pallet_entity_kyc,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const BasicKycValidity: u64 = 1000;
    pub const StandardKycValidity: u64 = 500;
    pub const EnhancedKycValidity: u64 = 2000;
    pub const InstitutionalKycValidity: u64 = 3000;
}

impl pallet_entity_kyc::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxCidLength = ConstU32<64>;
    type MaxProviderNameLength = ConstU32<64>;
    type MaxProviders = ConstU32<20>;
    type BasicKycValidity = BasicKycValidity;
    type StandardKycValidity = StandardKycValidity;
    type EnhancedKycValidity = EnhancedKycValidity;
    type InstitutionalKycValidity = InstitutionalKycValidity;
    type AdminOrigin = EnsureRoot<u64>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    t.into()
}
