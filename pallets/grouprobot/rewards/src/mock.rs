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

pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(_: &BotIdHash) -> bool { true }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			10 => Some(BOT_OWNER),
			_ => None,
		}
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(_: &BotIdHash) -> Option<u64> { None }
	fn bot_status(_: &BotIdHash) -> Option<BotStatus> { None }
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

parameter_types! {
	pub const MaxEraHist: u64 = 10;
	pub const RewardPoolAcct: u64 = 100;
	pub const MaxBatch: u32 = 10;
}

impl pallet_grouprobot_rewards::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type NodeConsensus = MockNodeConsensus;
	type BotRegistry = MockBotRegistry;
	type RewardPoolAccount = RewardPoolAcct;
	type MaxEraHistory = MaxEraHist;
	type MaxBatchClaim = MaxBatch;
}

pub const OPERATOR: u64 = 10;
pub const OPERATOR2: u64 = 11;
pub const OTHER: u64 = 99;
pub const BOT_OWNER: u64 = 50;
pub const REWARD_POOL: u64 = 100;

pub fn node_id(n: u8) -> NodeId {
	let mut id = [0u8; 32];
	id[0] = n;
	id
}

pub fn bot_hash(n: u8) -> BotIdHash {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(OPERATOR, 100_000),
			(OPERATOR2, 100_000),
			(OTHER, 100_000),
			(BOT_OWNER, 100_000),
			(REWARD_POOL, 1_000_000),
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
