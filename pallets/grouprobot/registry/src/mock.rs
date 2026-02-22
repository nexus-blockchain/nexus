use crate as pallet_grouprobot_registry;
use frame_support::{derive_impl, parameter_types, traits::Hooks};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		GroupRobotRegistry: pallet_grouprobot_registry,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}

parameter_types! {
	pub const AttestationValidityBlocks: u64 = 100;
	pub const AttestationCheckInterval: u64 = 10;
}

impl pallet_grouprobot_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxBotsPerOwner = frame_support::traits::ConstU32<5>;
	type MaxPlatformsPerCommunity = frame_support::traits::ConstU32<3>;
	type MaxPlatformBindingsPerUser = frame_support::traits::ConstU32<5>;
	type AttestationValidityBlocks = AttestationValidityBlocks;
	type AttestationCheckInterval = AttestationCheckInterval;
	type MaxQuoteLen = frame_support::traits::ConstU32<8192>;
}

pub const OWNER: u64 = 1;
pub const OWNER2: u64 = 2;
pub const OTHER: u64 = 3;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn advance_to(n: u64) {
	while System::block_number() < n {
		let next = System::block_number() + 1;
		System::set_block_number(next);
		GroupRobotRegistry::on_initialize(next);
	}
}
