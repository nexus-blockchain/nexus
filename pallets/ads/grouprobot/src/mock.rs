use crate as pallet_ads_grouprobot;
use frame_support::{
	derive_impl,
	parameter_types,
	traits::ConstU32,
};
use pallet_grouprobot_primitives::*;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		AdsGroupRobot: pallet_ads_grouprobot,
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
	type ExistentialDeposit = ConstU128<1>;
}

use frame_support::traits::ConstU128;

parameter_types! {
	pub const TreasuryAccountId: u64 = 999;
	pub const RewardPoolAccountId: u64 = 998;
}

// ============================================================================
// Mock NodeConsensusProvider
// ============================================================================

pub struct MockNodeConsensus;

impl NodeConsensusProvider<u64> for MockNodeConsensus {
	fn is_node_active(node_id: &NodeId) -> bool {
		node_id[0] != 0
	}

	fn node_operator(node_id: &NodeId) -> Option<u64> {
		if node_id[0] == 0 {
			None
		} else {
			Some(node_id[0] as u64)
		}
	}

	fn is_tee_node_by_operator(operator: &u64) -> bool {
		*operator == TEE_NODE_OPERATOR
	}
}

// ============================================================================
// Mock SubscriptionProvider
// ============================================================================

pub struct MockSubscription;

impl SubscriptionProvider for MockSubscription {
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
		match bot_id_hash[0] {
			1 => SubscriptionTier::Pro,
			2 => SubscriptionTier::Free,
			_ => SubscriptionTier::Basic,
		}
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

// ============================================================================
// Mock RewardAccruer
// ============================================================================

pub struct MockRewardPool;

impl RewardAccruer for MockRewardPool {
	fn accrue_node_reward(_node_id: &NodeId, _amount: u128) {}
}

// ============================================================================
// Mock BotRegistryProvider
// ============================================================================

pub struct MockBotRegistry;

impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(_: &BotIdHash) -> bool { true }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(BOT_OWNER),
			2 => Some(BOT_OWNER2),
			_ => None,
		}
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(BOT_OWNER),
			2 => Some(BOT_OWNER2),
			_ => None,
		}
	}
	fn bot_status(_: &BotIdHash) -> Option<BotStatus> {
		Some(BotStatus::Active)
	}
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

parameter_types! {
	pub const UnbondingPeriodBlocks: u64 = 10;
}

impl pallet_ads_grouprobot::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type NodeConsensus = MockNodeConsensus;
	type Subscription = MockSubscription;
	type RewardPool = MockRewardPool;
	type BotRegistry = MockBotRegistry;
	type TreasuryAccount = TreasuryAccountId;
	type RewardPoolAccount = RewardPoolAccountId;
	type AudienceSurgeThresholdPct = ConstU32<100>;
	type NodeDeviationThresholdPct = ConstU32<20>;
	type AdSlashPercentage = ConstU32<30>;
	type UnbondingPeriod = UnbondingPeriodBlocks;
	type StakerRewardPct = ConstU32<10>;
}

pub const STAKER: u64 = 1;
pub const STAKER2: u64 = 2;
pub const BOT_OWNER: u64 = 40;
pub const BOT_OWNER2: u64 = 41;
pub const TEE_NODE_OPERATOR: u64 = 30;
pub const NODE_OPERATOR: u64 = 20;

pub fn community_hash(n: u8) -> CommunityIdHash {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

pub fn node_id(operator: u8, _is_tee: bool) -> NodeId {
	let mut id = [0u8; 32];
	id[0] = operator;
	id
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(STAKER, 1_000_000_000_000_000),
			(STAKER2, 500_000_000_000_000),
			(BOT_OWNER, 500_000_000_000_000),
			(BOT_OWNER2, 500_000_000_000_000),
			(TEE_NODE_OPERATOR, 100_000_000_000_000),
			(NODE_OPERATOR, 100_000_000_000_000),
			(999u64, 1_000_000_000_000_000),
			(998u64, 1_000_000_000_000_000),
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext
}
