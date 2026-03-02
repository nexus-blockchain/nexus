use crate as pallet_commission_single_line;
use frame_support::{derive_impl, parameter_types};
use pallet_commission_common::MemberCommissionStatsData;
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeMap;

type Balance = u128;

// ============================================================================
// Thread-local mock state
// ============================================================================

thread_local! {
    pub static MEMBER_STATS: RefCell<BTreeMap<(u64, u64), MemberCommissionStatsData<Balance>>> =
        RefCell::new(BTreeMap::new());
    /// 自定义等级 (entity_id, account) -> custom_level_id
    pub static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> =
        RefCell::new(BTreeMap::new());
}

pub fn clear_mocks() {
    MEMBER_STATS.with(|m| m.borrow_mut().clear());
    CUSTOM_LEVEL_IDS.with(|m| m.borrow_mut().clear());
}

pub fn set_member_stats(entity_id: u64, account: u64, total_earned: Balance) {
    MEMBER_STATS.with(|m| {
        m.borrow_mut().insert(
            (entity_id, account),
            MemberCommissionStatsData {
                total_earned,
                ..Default::default()
            },
        );
    });
}

/// 设置自定义等级 ID
pub fn set_custom_level(entity_id: u64, account: u64, level_id: u8) {
    CUSTOM_LEVEL_IDS.with(|m| {
        m.borrow_mut().insert((entity_id, account), level_id);
    });
}

// ============================================================================
// MockStatsProvider + MockMemberLevelProvider
// ============================================================================

pub struct MockStatsProvider;

impl crate::pallet::SingleLineStatsProvider<u64, Balance> for MockStatsProvider {
    fn get_member_stats(entity_id: u64, account: &u64) -> MemberCommissionStatsData<Balance> {
        MEMBER_STATS.with(|m| {
            m.borrow()
                .get(&(entity_id, *account))
                .cloned()
                .unwrap_or_default()
        })
    }
}

pub struct MockMemberLevelProvider;

impl crate::pallet::SingleLineMemberLevelProvider<u64> for MockMemberLevelProvider {
    fn custom_level_id(entity_id: u64, account: &u64) -> u8 {
        CUSTOM_LEVEL_IDS.with(|m| m.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
    }
}

// ============================================================================
// Mock Runtime
// ============================================================================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        CommissionSingleLine: pallet_commission_single_line,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = frame_system::mocking::MockBlock<Test>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
}

parameter_types! {
    pub const MaxSingleLineLength: u32 = 100;
}

impl pallet_commission_single_line::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type StatsProvider = MockStatsProvider;
    type MemberLevelProvider = MockMemberLevelProvider;
    type MaxSingleLineLength = MaxSingleLineLength;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    clear_mocks();
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
