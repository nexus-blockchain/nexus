use crate as pallet_grouprobot_consensus;
use frame_support::{derive_impl, parameter_types, traits::Hooks};
use pallet_grouprobot_primitives::*;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		GroupRobotConsensus: pallet_grouprobot_consensus,
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

// Mock BotRegistryProvider
pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool {
		matches!(bot_id_hash[0], 1 | 2 | 10 | 11)
	}
	fn is_tee_node(bot_id_hash: &BotIdHash) -> bool {
		// bot_hash(10) and bot_hash(11) have valid TEE attestation
		matches!(bot_id_hash[0], 10 | 11)
	}
	fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool {
		// bot_hash(10) has SGX dual attestation, bot_hash(11) is TDX-only
		bot_id_hash[0] == 10
	}
	fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool {
		matches!(bot_id_hash[0], 10 | 11)
	}
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OWNER),
			2 => Some(OWNER2),
			10 => Some(OPERATOR),
			11 => Some(OPERATOR2),
			_ => None,
		}
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
}

parameter_types! {
	pub const MinStake: u128 = 100;
	pub const ExitCooldown: u64 = 10;
	pub const EraLength: u64 = 50;
	pub const InflationPerEra: u128 = 1000;
	pub const SlashPct: u32 = 10;
	pub const BasicFee: u128 = 10;
	pub const ProFee: u128 = 30;
	pub const EnterpriseFee: u128 = 100;
	pub const SequenceTtl: u64 = 100;
	pub const MaxSeqCleanup: u32 = 10;
	pub const MaxEraHist: u64 = 10;
}

impl pallet_grouprobot_consensus::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MaxActiveNodes = frame_support::traits::ConstU32<10>;
	type MinStake = MinStake;
	type ExitCooldownPeriod = ExitCooldown;
	type EraLength = EraLength;
	type InflationPerEra = InflationPerEra;
	type SlashPercentage = SlashPct;
	type BotRegistry = MockBotRegistry;
	type BasicFeePerEra = BasicFee;
	type ProFeePerEra = ProFee;
	type EnterpriseFeePerEra = EnterpriseFee;
	type SequenceTtlBlocks = SequenceTtl;
	type MaxSequenceCleanupPerBlock = MaxSeqCleanup;
	type MaxEraHistory = MaxEraHist;
}

pub const OWNER: u64 = 1;
pub const OWNER2: u64 = 2;
pub const OPERATOR: u64 = 10;
pub const OPERATOR2: u64 = 11;
pub const OTHER: u64 = 99;

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
			(OWNER, 100_000),
			(OWNER2, 100_000),
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

pub fn advance_to(n: u64) {
	while System::block_number() < n {
		let next = System::block_number() + 1;
		System::set_block_number(next);
		GroupRobotConsensus::on_initialize(next);
	}
}
