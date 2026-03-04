use crate as pallet_grouprobot_registry;
use frame_support::{derive_impl, parameter_types, traits::Hooks};
use pallet_grouprobot_primitives::*;
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
	pub const PeerHeartbeatTimeout: u64 = 50;
}

// Mock SubscriptionProvider: bot_id[0]==1 => Basic (paid), others => Free
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

impl pallet_grouprobot_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxBotsPerOwner = frame_support::traits::ConstU32<5>;
	type MaxPlatformsPerCommunity = frame_support::traits::ConstU32<3>;
	type MaxPlatformBindingsPerUser = frame_support::traits::ConstU32<5>;
	type AttestationValidityBlocks = AttestationValidityBlocks;
	type AttestationCheckInterval = AttestationCheckInterval;
	type MaxQuoteLen = frame_support::traits::ConstU32<8192>;
	type MaxPeersPerBot = frame_support::traits::ConstU32<10>;
	type MaxEndpointLen = frame_support::traits::ConstU32<256>;
	type PeerHeartbeatTimeout = PeerHeartbeatTimeout;
	type Subscription = MockSubscription;
	type MaxOperatorNameLen = frame_support::traits::ConstU32<64>;
	type MaxOperatorContactLen = frame_support::traits::ConstU32<128>;
	type MaxBotsPerOperator = frame_support::traits::ConstU32<10>;
	type MaxUptimeEraHistory = frame_support::traits::ConstU32<10>;
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
