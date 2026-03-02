use crate as pallet_grouprobot_ads;
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
		GroupRobotAds: pallet_grouprobot_ads,
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

/// Mock 节点共识: node_id[0] 决定运营者, node_id[1]==1 表示 TEE 节点
pub struct MockNodeConsensus;

impl NodeConsensusProvider<u64> for MockNodeConsensus {
	fn is_node_active(node_id: &NodeId) -> bool {
		// node_id[0] != 0 视为活跃
		node_id[0] != 0
	}

	fn node_operator(node_id: &NodeId) -> Option<u64> {
		if node_id[0] == 0 {
			None
		} else {
			// node_id[0] 作为运营者 account id
			Some(node_id[0] as u64)
		}
	}

	fn is_tee_node_by_operator(operator: &u64) -> bool {
		// operator == TEE_NODE_OPERATOR 视为 TEE 节点
		*operator == TEE_NODE_OPERATOR
	}
}

// ============================================================================
// Mock SubscriptionProvider
// ============================================================================

pub struct MockSubscription;

impl SubscriptionProvider for MockSubscription {
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
		// community_hash(1) = Pro, community_hash(2) = Free, others = Basic
		match bot_id_hash[0] {
			1 => SubscriptionTier::Pro,
			2 => SubscriptionTier::Free,
			_ => SubscriptionTier::Basic,
		}
	}
	fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
		MockSubscription::effective_tier(bot_id_hash).feature_gate()
	}
}

// ============================================================================
// Mock RewardAccruer
// ============================================================================

pub struct MockRewardPool;

impl RewardAccruer for MockRewardPool {
	fn accrue_node_reward(_node_id: &NodeId, _amount: u128) {
		// no-op in tests; rewards pallet handles unified claim
	}
}

// ============================================================================
// Mock BotRegistryProvider (10.9: CommunityAdmin 绑定 Bot Owner)
// ============================================================================

pub struct MockBotRegistry;

impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(_: &BotIdHash) -> bool { true }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		// community_hash(1) 的 owner 是 BOT_OWNER (40)
		// community_hash(2) 的 owner 是 BOT_OWNER2 (41)
		// 其他没有 owner
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
}

impl pallet_grouprobot_ads::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MaxAdTextLength = ConstU32<280>;
	type MaxAdUrlLength = ConstU32<256>;
	type MaxReceiptsPerCommunity = ConstU32<50>;
	type MaxAdvertiserBlacklist = ConstU32<50>;
	type MaxAdvertiserWhitelist = ConstU32<20>;
	type MaxCommunityBlacklist = ConstU32<50>;
	type MaxCommunityWhitelist = ConstU32<10>;
	type MinBidPerMille = ConstU128<100_000_000_000>; // 0.1 UNIT
	type MinAudienceSize = ConstU32<20>;
	type AudienceSurgeThresholdPct = ConstU32<100>; // 100% 增长触发暂停
	type NodeDeviationThresholdPct = ConstU32<20>;  // 20% 偏差拒结
	type AdSlashPercentage = ConstU32<30>;
	type TreasuryAccount = TreasuryAccountId;
	type RewardPoolAccount = RewardPoolAccountId;
	type NodeConsensus = MockNodeConsensus;
	type Subscription = MockSubscription;
	type RewardPool = MockRewardPool;
	type BotRegistry = MockBotRegistry;
	type PrivateAdRegistrationFee = ConstU128<1_000_000_000_000>; // 1 UNIT per private ad
}

pub const ADVERTISER: u64 = 1;
pub const ADVERTISER2: u64 = 2;
pub const COMMUNITY_OWNER: u64 = 10;
pub const TREASURY: u64 = 999;
pub const REPORTER: u64 = 50;
pub const BOT_OWNER: u64 = 40;
pub const BOT_OWNER2: u64 = 41;
pub const NODE_OPERATOR: u64 = 20;
pub const TEE_NODE_OPERATOR: u64 = 30;

pub fn community_hash(n: u8) -> [u8; 32] {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

/// 生成 node_id: [0]=operator account, [1]=is_tee (1/0)
pub fn node_id(operator: u8, is_tee: bool) -> NodeId {
	let mut id = [0u8; 32];
	id[0] = operator;
	id[1] = if is_tee { 1 } else { 0 };
	id
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(ADVERTISER, 1_000_000_000_000_000),       // 1000 UNIT
			(ADVERTISER2, 500_000_000_000_000),         // 500 UNIT
			(COMMUNITY_OWNER, 500_000_000_000_000),     // 500 UNIT
			(TREASURY, 1_000_000_000_000_000),          // 1000 UNIT
			(998u64, 1_000_000_000_000_000),             // RewardPool 1000 UNIT
			(REPORTER, 100_000_000_000_000),            // 100 UNIT
			(NODE_OPERATOR, 100_000_000_000_000),       // 100 UNIT
			(TEE_NODE_OPERATOR, 100_000_000_000_000),   // 100 UNIT
			(BOT_OWNER, 500_000_000_000_000),           // 500 UNIT
			(BOT_OWNER2, 500_000_000_000_000),          // 500 UNIT
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		// 🆕 10.6: Pro 社区默认有质押, 以通过 AdsDisabledByTier 检查
		crate::CommunityAdStake::<Test>::insert(community_hash(1), 100_000_000_000_000u128);
	});
	ext
}

