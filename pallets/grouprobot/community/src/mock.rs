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

// Mock SubscriptionProvider: community_hash(1) => Basic, community_hash(2) => Pro, community_hash(5) => Enterprise, others => Free
pub struct MockSubscription;
impl SubscriptionProvider for MockSubscription {
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
		match bot_id_hash[0] {
			1 => SubscriptionTier::Basic,
			2 => SubscriptionTier::Pro,
			5 => SubscriptionTier::Enterprise,
			_ => SubscriptionTier::Free,
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

pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool { bot_id_hash[0] <= 2 || bot_id_hash[0] == 4 }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		// community_hash(1,2) = active, community_hash(3) = inactive, community_hash(4) = active+Free
		if bot_id_hash[0] <= 4 { Some(OWNER) } else { None }
	}
	fn bot_public_key(bot_id_hash: &BotIdHash) -> Option<[u8; 32]> {
		if bot_id_hash[0] <= 4 { Some(test_public_key()) } else { None }
	}
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		if bot_id_hash[0] <= 4 { Some(OWNER) } else { None }
	}
	fn bot_status(bot_id_hash: &BotIdHash) -> Option<BotStatus> {
		if bot_id_hash[0] <= 2 || bot_id_hash[0] == 4 { Some(BotStatus::Active) } else { None }
	}
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

impl pallet_grouprobot_community::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxLogsPerCommunity = frame_support::traits::ConstU32<10>;
	type ReputationCooldown = frame_support::traits::ConstU64<5>;
	type MaxReputationDelta = frame_support::traits::ConstU32<100>;
	type MaxBatchSize = frame_support::traits::ConstU32<50>;
	type BlocksPerDay = frame_support::traits::ConstU32<14_400>;
	type BotRegistry = MockBotRegistry;
	type Subscription = MockSubscription;
}

pub const OWNER: u64 = 1;
pub const OTHER: u64 = 2;

/// 测试用 Ed25519 密钥对 (确定性种子)
use sp_core::ed25519;
use sp_core::Pair;

pub fn test_keypair() -> ed25519::Pair {
	ed25519::Pair::from_seed(&[1u8; 32])
}

pub fn test_public_key() -> [u8; 32] {
	test_keypair().public().0
}

/// 生成测试用 Ed25519 签名
pub fn test_sign(
	community_id_hash: &[u8; 32],
	action_type: &pallet_grouprobot_primitives::ActionType,
	target_hash: &[u8; 32],
	sequence: u64,
	message_hash: &[u8; 32],
) -> [u8; 64] {
	use codec::Encode;
	let mut msg = Vec::with_capacity(32 + 4 + 32 + 8 + 32);
	msg.extend_from_slice(community_id_hash);
	msg.extend_from_slice(&action_type.encode());
	msg.extend_from_slice(target_hash);
	msg.extend_from_slice(&sequence.to_le_bytes());
	msg.extend_from_slice(message_hash);
	test_keypair().sign(&msg).0
}

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
