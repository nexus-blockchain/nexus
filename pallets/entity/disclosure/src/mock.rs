//! 财务披露模块测试 mock

use crate as pallet_entity_disclosure;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU16, ConstU32},
};
use pallet_entity_common::{EntityProvider, EntityStatus};
use sp_runtime::{BuildStorage, DispatchError};

type Block = frame_system::mocking::MockBlock<Test>;

pub const OWNER: u64 = 1;
pub const OWNER_2: u64 = 2;
pub const ALICE: u64 = 10;
pub const BOB: u64 = 11;
pub const ENTITY_ID: u64 = 1;
pub const ENTITY_ID_2: u64 = 2;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        EntityDisclosure: pallet_entity_disclosure,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const BasicDisclosureInterval: u64 = 1000;
    pub const StandardDisclosureInterval: u64 = 500;
    pub const EnhancedDisclosureInterval: u64 = 100;
}

/// Mock EntityProvider — 正确实现 EntityProvider trait
pub struct MockEntityProvider;
impl EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        entity_id == ENTITY_ID || entity_id == ENTITY_ID_2
    }
    fn is_entity_active(entity_id: u64) -> bool {
        entity_id == ENTITY_ID || entity_id == ENTITY_ID_2
    }
    fn entity_status(entity_id: u64) -> Option<EntityStatus> {
        if entity_id <= 2 { Some(EntityStatus::Active) } else { None }
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        match entity_id {
            ENTITY_ID => Some(OWNER),
            ENTITY_ID_2 => Some(OWNER_2),
            _ => None,
        }
    }
    fn entity_account(entity_id: u64) -> u64 {
        100 + entity_id
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn update_entity_rating(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
}

parameter_types! {
    pub const MaxBlackoutDuration: u64 = 200; // 测试用较小值
}

impl pallet_entity_disclosure::Config for Test {
    type EntityProvider = MockEntityProvider;
    type MaxCidLength = ConstU32<64>;
    type MaxInsiders = ConstU32<5>;
    type MaxDisclosureHistory = ConstU32<10>;
    type BasicDisclosureInterval = BasicDisclosureInterval;
    type StandardDisclosureInterval = StandardDisclosureInterval;
    type EnhancedDisclosureInterval = EnhancedDisclosureInterval;
    type MajorHolderThreshold = ConstU16<500>;
    type MaxBlackoutDuration = MaxBlackoutDuration;
    type MaxAnnouncementHistory = ConstU32<10>;
    type MaxTitleLength = ConstU32<128>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn advance_blocks(n: u64) {
    let current = System::block_number();
    System::set_block_number(current + n);
}
