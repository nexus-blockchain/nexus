use crate as pallet_grouprobot_subscription;
use frame_support::{derive_impl, parameter_types};
use pallet_grouprobot_primitives::*;
use sp_runtime::BuildStorage;
use core::cell::RefCell;

thread_local! {
	static DELIVERY_COUNTS: RefCell<std::collections::HashMap<CommunityIdHash, u32>> = RefCell::new(std::collections::HashMap::new());
}

pub struct MockAdDelivery;
impl AdDeliveryProvider for MockAdDelivery {
	fn era_delivery_count(community_id_hash: &CommunityIdHash) -> u32 {
		DELIVERY_COUNTS.with(|m| *m.borrow().get(community_id_hash).unwrap_or(&0))
	}
	fn reset_era_deliveries(community_id_hash: &CommunityIdHash) {
		DELIVERY_COUNTS.with(|m| { m.borrow_mut().remove(community_id_hash); });
	}
}

pub fn set_delivery_count(community_id_hash: &CommunityIdHash, count: u32) {
	DELIVERY_COUNTS.with(|m| { m.borrow_mut().insert(*community_id_hash, count); });
}

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Subscription: pallet_grouprobot_subscription,
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

pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool {
		matches!(bot_id_hash[0], 1 | 2)
	}
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OWNER),
			2 => Some(OWNER2),
			_ => None,
		}
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OPERATOR),  // bot 1 有运营者
			2 => None,            // bot 2 无运营者 (fallback 到 RewardPool)
			_ => None,
		}
	}
	fn bot_status(bot_id_hash: &BotIdHash) -> Option<BotStatus> {
		if matches!(bot_id_hash[0], 1 | 2) { Some(BotStatus::Active) } else { None }
	}
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

parameter_types! {
	pub const BasicFee: u128 = 10;
	pub const ProFee: u128 = 30;
	pub const EnterpriseFee: u128 = 100;
	pub const TreasuryAcct: u64 = 200;
	pub const RewardPoolAcct: u64 = 201;
	pub const MaxSubSettle: u32 = 50;
	pub const EraLength: u64 = 50;
	pub const EraStartBlockVal: u64 = 1;
	pub const CurrentEraVal: u64 = 0;
	pub const AdBasicThreshold: u32 = 3;
	pub const AdProThreshold: u32 = 6;
	pub const AdEnterpriseThreshold: u32 = 11;
	pub const MaxUnderdeliveryEras: u8 = 3;
}

impl pallet_grouprobot_subscription::Config for Test {
	type Currency = Balances;
	type WeightInfo = ();
	type BotRegistry = MockBotRegistry;
	type BasicFeePerEra = BasicFee;
	type BasicFeePerEraUsd = frame_support::traits::ConstU64<33_333>;
	type ProFeePerEra = ProFee;
	type ProFeePerEraUsd = frame_support::traits::ConstU64<66_667>;
	type EnterpriseFeePerEra = EnterpriseFee;
	type EnterpriseFeePerEraUsd = frame_support::traits::ConstU64<166_667>;
	type DepositCalculator = ();
	type TreasuryAccount = TreasuryAcct;
	type RewardPoolAccount = RewardPoolAcct;
	type MaxSubscriptionSettlePerEra = MaxSubSettle;
	type EraLength = EraLength;
	type EraStartBlockProvider = EraStartBlockVal;
	type CurrentEraProvider = CurrentEraVal;
	type AdDelivery = MockAdDelivery;
	type AdBasicThreshold = AdBasicThreshold;
	type AdProThreshold = AdProThreshold;
	type AdEnterpriseThreshold = AdEnterpriseThreshold;
	type MaxUnderdeliveryEras = MaxUnderdeliveryEras;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

pub const OWNER: u64 = 1;
pub const OWNER2: u64 = 2;
pub const OTHER: u64 = 99;
pub const TREASURY: u64 = 200;
pub const REWARD_POOL: u64 = 201;
pub const OPERATOR: u64 = 300;

pub fn bot_hash(n: u8) -> BotIdHash {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

pub fn community_hash(n: u8) -> CommunityIdHash {
	let mut h = [0u8; 32];
	h[0] = n;
	h[1] = 0xCC; // 区分 bot_hash
	h
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(OWNER, 100_000),
			(OWNER2, 100_000),
			(OTHER, 100_000),
			(TREASURY, 100_000),
			(REWARD_POOL, 100_000),
			(OPERATOR, 100_000),
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
