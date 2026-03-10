use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU32, ConstU64, ConstU128},
};
use pallet_grouprobot_primitives::*;
use sp_runtime::{BuildStorage, DispatchError};

#[allow(unused_imports)]
use codec::{Decode, Encode};
#[allow(unused_imports)]
use scale_info::TypeInfo;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		AdsEntity: pallet_ads_entity,
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

// ============================================================================
// Constants
// ============================================================================

pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const CHARLIE: u64 = 3;
pub const TEE_OPERATOR: u64 = 30;
pub const TREASURY: u64 = 999;
pub const REWARD_POOL: u64 = 998;

// ============================================================================
// Mock EntityProvider
// ============================================================================

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
	fn entity_exists(entity_id: u64) -> bool { entity_id == 1 }
	fn is_entity_active(entity_id: u64) -> bool { entity_id == 1 }
	fn entity_status(entity_id: u64) -> Option<pallet_entity_common::EntityStatus> {
		if entity_id == 1 { Some(pallet_entity_common::EntityStatus::Active) } else { None }
	}
	fn entity_owner(entity_id: u64) -> Option<u64> {
		if entity_id == 1 { Some(ALICE) } else { None }
	}
	fn entity_account(entity_id: u64) -> u64 { 1000 + entity_id }
	fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
	fn is_entity_admin(entity_id: u64, account: &u64, _perm: u32) -> bool {
		entity_id == 1 && *account == BOB
	}
}

// ============================================================================
// Mock ShopProvider
// ============================================================================

pub struct MockShopProvider;

impl pallet_entity_common::ShopProvider<u64> for MockShopProvider {
	fn shop_exists(shop_id: u64) -> bool { shop_id == 10 }
	fn is_shop_active(shop_id: u64) -> bool { shop_id == 10 }
	fn shop_entity_id(shop_id: u64) -> Option<u64> {
		if shop_id == 10 { Some(1) } else { None }
	}
	fn shop_owner(shop_id: u64) -> Option<u64> {
		if shop_id == 10 { Some(ALICE) } else { None }
	}
	fn shop_account(shop_id: u64) -> u64 { 2000 + shop_id }
	fn shop_type(_: u64) -> Option<pallet_entity_common::ShopType> {
		Some(pallet_entity_common::ShopType::OnlineStore)
	}
	fn is_shop_manager(shop_id: u64, account: &u64) -> bool {
		shop_id == 10 && *account == CHARLIE
	}
	fn update_shop_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
	fn update_shop_rating(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
	fn deduct_operating_fund(_: u64, _: u128) -> Result<(), DispatchError> { Ok(()) }
	fn operating_balance(_: u64) -> u128 { 0 }
}

// ============================================================================
// Mock GroupRobot Providers
// ============================================================================

pub struct MockNodeConsensus;

impl NodeConsensusProvider<u64> for MockNodeConsensus {
	fn is_node_active(node_id: &NodeId) -> bool { node_id[0] != 0 }
	fn node_operator(node_id: &NodeId) -> Option<u64> {
		if node_id[0] == 0 { None } else { Some(node_id[0] as u64) }
	}
	fn is_tee_node_by_operator(operator: &u64) -> bool { *operator == TEE_OPERATOR }
}

pub struct MockSubscription;

impl SubscriptionProvider for MockSubscription {
	fn effective_tier(_: &BotIdHash) -> SubscriptionTier { SubscriptionTier::Pro }
	fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
		Self::effective_tier(bot_id_hash).feature_gate()
	}
	fn is_subscription_active(_: &BotIdHash) -> bool { true }
	fn subscription_status(_: &BotIdHash) -> Option<SubscriptionStatus> {
		Some(SubscriptionStatus::Active)
	}
}

pub struct MockRewardPool;

impl RewardAccruer for MockRewardPool {
	fn accrue_node_reward(_: &NodeId, _: u128) {}
}

pub struct MockBotRegistry;

impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(_: &BotIdHash) -> bool { true }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		if bot_id_hash[0] != 0 { Some(ALICE) } else { None }
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		if bot_id_hash[0] != 0 { Some(ALICE) } else { None }
	}
	fn bot_status(_: &BotIdHash) -> Option<BotStatus> { Some(BotStatus::Active) }
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

// ============================================================================
// Pallet Configs
// ============================================================================

parameter_types! {
	pub const TreasuryAccount: u64 = TREASURY;
	pub const RewardPoolAccount: u64 = REWARD_POOL;
	pub const UnbondingPeriodBlocks: u64 = 10;
}

impl pallet_ads_entity::Config for Test {
	type WeightInfo = ();
	type Currency = Balances;
	type EntityProvider = MockEntityProvider;
	type ShopProvider = MockShopProvider;
	type TreasuryAccount = TreasuryAccount;
	type PlatformAdShareBps = ConstU16<2000>;
	type AdPlacementDeposit = ConstU128<100>;
	type AdPlacementDepositUsd = ConstU64<1_000_000>;
	type DepositCalculator = ();
	type MaxPlacementsPerEntity = ConstU32<10>;
	type DefaultDailyImpressionCap = ConstU32<1000>;
	type BlocksPerDay = ConstU32<14400>;
}

impl pallet_ads_grouprobot::Config for Test {
	type Currency = Balances;
	type NodeConsensus = MockNodeConsensus;
	type Subscription = MockSubscription;
	type RewardPool = MockRewardPool;
	type BotRegistry = MockBotRegistry;
	type TreasuryAccount = TreasuryAccount;
	type RewardPoolAccount = RewardPoolAccount;
	type AudienceSurgeThresholdPct = ConstU32<100>;
	type NodeDeviationThresholdPct = ConstU32<20>;
	type AdSlashPercentage = ConstU32<30>;
	type UnbondingPeriod = UnbondingPeriodBlocks;
	type StakerRewardPct = ConstU32<10>;
	type MaxStakersPerCommunity = ConstU32<50>;
	type WeightInfo = ();
}

// ============================================================================
// Helpers
// ============================================================================

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default()
		.build_storage()
		.unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(ALICE, 1_000_000_000_000_000),
			(BOB, 1_000_000_000_000_000),
			(CHARLIE, 1_000_000_000_000_000),
			(TEE_OPERATOR, 1_000_000_000_000_000),
			(TREASURY, 10_000_000_000_000_000),
			(REWARD_POOL, 10_000_000_000_000_000),
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

/// Entity 级 PlacementId (与 pallet-ads-entity 一致)
pub fn entity_placement_id(entity_id: u64) -> [u8; 32] {
	pallet_ads_entity::entity_placement_id(entity_id)
}

/// GroupRobot 级 PlacementId (社区 hash)
pub fn community_placement_id(n: u8) -> [u8; 32] {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}
