use crate as pallet_grouprobot_community;
use frame_support::derive_impl;
use pallet_grouprobot_primitives::*;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		GroupRobotCommunity: pallet_grouprobot_community,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}

// Mock SubscriptionProvider: community_hash(1) => Basic (paid), others => Free
pub struct MockSubscription;
impl SubscriptionProvider for MockSubscription {
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
		if bot_id_hash[0] == 1 { SubscriptionTier::Basic } else { SubscriptionTier::Free }
	}
	fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
		MockSubscription::effective_tier(bot_id_hash).feature_gate()
	}
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
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		if bot_id_hash[0] == 1 { Some(OWNER) } else { None }
	}
}

impl pallet_grouprobot_community::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxLogsPerCommunity = frame_support::traits::ConstU32<10>;
	type ReputationCooldown = frame_support::traits::ConstU64<5>;
	type MaxReputationDelta = frame_support::traits::ConstU32<100>;
	type BotRegistry = MockBotRegistry;
	type Subscription = MockSubscription;
}

pub const OWNER: u64 = 1;
pub const OTHER: u64 = 2;

pub fn community_hash(n: u8) -> CommunityIdHash {
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
