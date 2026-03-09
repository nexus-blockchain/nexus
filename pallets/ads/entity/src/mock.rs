use crate as pallet_ads_entity;
use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU32, ConstU64},
};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError,
};
use std::cell::RefCell;

thread_local! {
	static ENTITY_LOCKED: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
}

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		AdsEntity: pallet_ads_entity,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = u128;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ConstU32<0>;
}

use frame_support::traits::ConstU128;

// ============================================================================
// Mock Entity Provider
// ============================================================================

/// entity_id=1: 存在、激活、owner=ALICE(1), admin=BOB(2)
/// entity_id=2: 存在、未激活
/// entity_id=99: 不存在
pub struct MockEntityProvider;

pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const CHARLIE: u64 = 3;
pub const TREASURY: u64 = 100;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
	fn entity_exists(entity_id: u64) -> bool {
		entity_id == 1 || entity_id == 2
	}
	fn is_entity_active(entity_id: u64) -> bool {
		entity_id == 1
	}
	fn entity_status(entity_id: u64) -> Option<pallet_entity_common::EntityStatus> {
		if entity_id == 1 {
			Some(pallet_entity_common::EntityStatus::Active)
		} else if entity_id == 2 {
			Some(pallet_entity_common::EntityStatus::Suspended)
		} else {
			None
		}
	}
	fn entity_owner(entity_id: u64) -> Option<u64> {
		if entity_id == 1 || entity_id == 2 {
			Some(ALICE)
		} else {
			None
		}
	}
	fn entity_account(entity_id: u64) -> u64 {
		1000 + entity_id
	}
	fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
	fn is_entity_admin(entity_id: u64, account: &u64, _required_permission: u32) -> bool {
		entity_id == 1 && *account == BOB
	}

	fn is_entity_locked(entity_id: u64) -> bool {
		ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
	}
}

// ============================================================================
// Mock Shop Provider
// ============================================================================

/// shop_id=10: 存在、激活、属于 entity_id=1, manager=CHARLIE(3)
/// shop_id=20: 存在、未激活
/// shop_id=99: 不存在
pub struct MockShopProvider;

impl pallet_entity_common::ShopProvider<u64> for MockShopProvider {
	fn shop_exists(shop_id: u64) -> bool {
		shop_id == 10 || shop_id == 20
	}
	fn is_shop_active(shop_id: u64) -> bool {
		shop_id == 10
	}
	fn shop_entity_id(shop_id: u64) -> Option<u64> {
		if shop_id == 10 || shop_id == 20 {
			Some(1)
		} else {
			None
		}
	}
	fn shop_owner(shop_id: u64) -> Option<u64> {
		if shop_id == 10 || shop_id == 20 {
			Some(ALICE)
		} else {
			None
		}
	}
	fn shop_account(shop_id: u64) -> u64 {
		2000 + shop_id
	}
	fn shop_type(_: u64) -> Option<pallet_entity_common::ShopType> {
		Some(pallet_entity_common::ShopType::OnlineStore)
	}
	fn is_shop_manager(shop_id: u64, account: &u64) -> bool {
		shop_id == 10 && *account == CHARLIE
	}
	fn update_shop_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
	fn update_shop_rating(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
	fn deduct_operating_fund(_: u64, _: u128) -> Result<(), DispatchError> { Ok(()) }
	fn operating_balance(_: u64) -> u128 { 0 }
}

// ============================================================================
// Pallet Config
// ============================================================================

parameter_types! {
	pub const TreasuryAccount: u64 = TREASURY;
}

impl pallet_ads_entity::Config for Test {
	type WeightInfo = ();
	type Currency = Balances;
	type EntityProvider = MockEntityProvider;
	type ShopProvider = MockShopProvider;
	type TreasuryAccount = TreasuryAccount;
	type PlatformAdShareBps = ConstU16<2000>;       // 20%
	type AdPlacementDeposit = ConstU128<100>;        // 100 units
	type MaxPlacementsPerEntity = ConstU32<10>;
	type DefaultDailyImpressionCap = ConstU32<1000>;
	type BlocksPerDay = ConstU32<14400>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

// ============================================================================
// Test Helpers
// ============================================================================

pub fn set_entity_locked(entity_id: u64) {
	ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default()
		.build_storage()
		.unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(ALICE, 100_000),
			(BOB, 100_000),
			(CHARLIE, 100_000),
			(TREASURY, 100_000),
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
