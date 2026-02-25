use crate as pallet_grouprobot_ceremony;
use frame_support::{derive_impl, parameter_types, traits::Hooks};
use pallet_grouprobot_primitives::*;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		GroupRobotCeremony: pallet_grouprobot_ceremony,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}

pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool { bot_id_hash[0] == 1 }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		if bot_id_hash[0] == 1 { Some(OWNER) } else { None }
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(bot_id_hash: &BotIdHash) -> u32 {
		if bot_id_hash[0] == 1 { 3 } else { 0 }
	}
}

parameter_types! {
	pub const CeremonyValidityBlocks: u64 = 100;
	pub const CeremonyCheckInterval: u64 = 10;
}

impl pallet_grouprobot_ceremony::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxParticipants = frame_support::traits::ConstU32<5>;
	type MaxCeremonyHistory = frame_support::traits::ConstU32<10>;
	type CeremonyValidityBlocks = CeremonyValidityBlocks;
	type CeremonyCheckInterval = CeremonyCheckInterval;
	type BotRegistry = MockBotRegistry;
}

pub const OWNER: u64 = 1;
pub const OTHER: u64 = 2;

pub fn ceremony_hash(n: u8) -> [u8; 32] {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

pub fn mrenclave(n: u8) -> [u8; 32] {
	let mut m = [0u8; 32];
	m[0] = n;
	m[31] = 0xEE; // distinguish from ceremony_hash
	m
}

pub fn bot_pk(n: u8) -> [u8; 32] {
	let mut p = [0u8; 32];
	p[0] = n;
	p[31] = 0xBB;
	p
}

pub fn bot_id(n: u8) -> [u8; 32] {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

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
		GroupRobotCeremony::on_initialize(next);
	}
}
