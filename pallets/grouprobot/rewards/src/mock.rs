use crate as pallet_grouprobot_rewards;
use frame_support::{derive_impl, parameter_types};
use pallet_grouprobot_primitives::*;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Rewards: pallet_grouprobot_rewards,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = u128;
}

pub struct MockNodeConsensus;
impl NodeConsensusProvider<u64> for MockNodeConsensus {
	fn is_node_active(node_id: &NodeId) -> bool {
		matches!(node_id[0], 1 | 2)
	}
	fn node_operator(node_id: &NodeId) -> Option<u64> {
		match node_id[0] {
			1 => Some(OPERATOR),
			2 => Some(OPERATOR2),
			_ => None,
		}
	}
	fn is_tee_node_by_operator(_: &u64) -> bool { false }
}

parameter_types! {
	pub const MaxEraHist: u64 = 10;
}

impl pallet_grouprobot_rewards::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type NodeConsensus = MockNodeConsensus;
	type MaxEraHistory = MaxEraHist;
}

pub const OPERATOR: u64 = 10;
pub const OPERATOR2: u64 = 11;
pub const OTHER: u64 = 99;

pub fn node_id(n: u8) -> NodeId {
	let mut id = [0u8; 32];
	id[0] = n;
	id
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(OPERATOR, 100_000),
			(OPERATOR2, 100_000),
			(OTHER, 100_000),
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
