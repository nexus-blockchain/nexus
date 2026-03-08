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

// Mock SubscriptionProvider: bot_id[0]==1 => Basic (paid), bot_id[0]==2 => Free
pub struct MockSubscription;
impl SubscriptionProvider for MockSubscription {
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
		if bot_id_hash[0] == 1 { SubscriptionTier::Basic } else { SubscriptionTier::Free }
	}
	fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
		MockSubscription::effective_tier(bot_id_hash).feature_gate()
	}
	fn is_subscription_active(bot_id_hash: &BotIdHash) -> bool {
		MockSubscription::effective_tier(bot_id_hash).is_paid()
	}
	fn subscription_status(bot_id_hash: &BotIdHash) -> Option<SubscriptionStatus> {
		if MockSubscription::effective_tier(bot_id_hash).is_paid() { Some(SubscriptionStatus::Active) } else { None }
	}
}

use core::cell::RefCell;

thread_local! {
	/// 允许测试覆盖 peer_count 返回值 (None = 使用默认逻辑)
	static MOCK_PEER_COUNT: RefCell<Option<u32>> = RefCell::new(None);
	/// 允许测试设置 bot_public_key 返回值 (None = 默认返回 None)
	static MOCK_BOT_PK: RefCell<Option<[u8; 32]>> = RefCell::new(None);
}

pub fn set_mock_peer_count(count: Option<u32>) {
	MOCK_PEER_COUNT.with(|c| *c.borrow_mut() = count);
}

pub fn set_mock_bot_pk(pk: Option<[u8; 32]>) {
	MOCK_BOT_PK.with(|c| *c.borrow_mut() = pk);
}

pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool { matches!(bot_id_hash[0], 1 | 3) }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OWNER),
			2 => Some(OTHER), // inactive + Free tier bot, owned by OTHER
			3 => Some(OTHER), // active + Free tier bot, owned by OTHER
			_ => None,
		}
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> {
		MOCK_BOT_PK.with(|c| *c.borrow())
	}
	fn peer_count(bot_id_hash: &BotIdHash) -> u32 {
		MOCK_PEER_COUNT.with(|c| {
			if let Some(count) = *c.borrow() {
				return count;
			}
			if bot_id_hash[0] == 1 { 3 } else { 0 }
		})
	}
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OWNER),
			2 | 3 => Some(OTHER),
			_ => None,
		}
	}
	fn bot_status(bot_id_hash: &BotIdHash) -> Option<BotStatus> {
		if matches!(bot_id_hash[0], 1 | 3) { Some(BotStatus::Active) } else { None }
	}
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

parameter_types! {
	pub const CeremonyValidityBlocks: u64 = 100;
	pub const CeremonyCheckInterval: u64 = 10;
	pub const MaxProcessPerBlock: u32 = 20;
}

impl pallet_grouprobot_ceremony::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxParticipants = frame_support::traits::ConstU32<5>;
	type MaxCeremonyHistory = frame_support::traits::ConstU32<10>;
	type CeremonyValidityBlocks = CeremonyValidityBlocks;
	type CeremonyCheckInterval = CeremonyCheckInterval;
	type MaxProcessPerBlock = MaxProcessPerBlock;
	type BotRegistry = MockBotRegistry;
	type Subscription = MockSubscription;
	type WeightInfo = ();
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
