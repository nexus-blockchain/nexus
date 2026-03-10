use crate as pallet_grouprobot_consensus;
use frame_support::{derive_impl, parameter_types, traits::Hooks};
use pallet_grouprobot_primitives::*;
use sp_core::{ed25519, Pair as PairT};
use sp_runtime::BuildStorage;
use core::cell::RefCell;

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
// Benchmark 模式下支持通过 thread_local 动态注册 bot
#[cfg(feature = "runtime-benchmarks")]
thread_local! {
	static BENCH_BOT_OWNERS: RefCell<alloc::collections::BTreeMap<u8, u64>> = RefCell::new(alloc::collections::BTreeMap::new());
	static BENCH_PAID_BOTS: RefCell<alloc::collections::BTreeSet<u8>> = RefCell::new(alloc::collections::BTreeSet::new());
}

pub struct MockBotRegistry;
impl BotRegistryProvider<u64> for MockBotRegistry {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool {
		if matches!(bot_id_hash[0], 1 | 2 | 10 | 11) { return true; }
		#[cfg(feature = "runtime-benchmarks")]
		{ return BENCH_BOT_OWNERS.with(|m| m.borrow().contains_key(&bot_id_hash[0])); }
		#[cfg(not(feature = "runtime-benchmarks"))]
		false
	}
	fn is_tee_node(bot_id_hash: &BotIdHash) -> bool {
		if matches!(bot_id_hash[0], 10 | 11) { return true; }
		#[cfg(feature = "runtime-benchmarks")]
		{ return BENCH_BOT_OWNERS.with(|m| m.borrow().contains_key(&bot_id_hash[0])); }
		#[cfg(not(feature = "runtime-benchmarks"))]
		false
	}
	fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool {
		bot_id_hash[0] == 10
	}
	fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool {
		if matches!(bot_id_hash[0], 10 | 11) { return true; }
		#[cfg(feature = "runtime-benchmarks")]
		{ return BENCH_BOT_OWNERS.with(|m| m.borrow().contains_key(&bot_id_hash[0])); }
		#[cfg(not(feature = "runtime-benchmarks"))]
		false
	}
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OWNER),
			2 => Some(OWNER2),
			10 => Some(OPERATOR),
			11 => Some(OPERATOR2),
			_ => {
				#[cfg(feature = "runtime-benchmarks")]
				{ return BENCH_BOT_OWNERS.with(|m| m.borrow().get(&bot_id_hash[0]).copied()); }
				#[cfg(not(feature = "runtime-benchmarks"))]
				None
			}
		}
	}
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
	fn bot_operator(bot_id_hash: &BotIdHash) -> Option<u64> {
		match bot_id_hash[0] {
			1 => Some(OPERATOR),
			2 => Some(OPERATOR2),
			10 => Some(OPERATOR),
			11 => Some(OPERATOR2),
			_ => {
				#[cfg(feature = "runtime-benchmarks")]
				{ return BENCH_BOT_OWNERS.with(|m| m.borrow().get(&bot_id_hash[0]).copied()); }
				#[cfg(not(feature = "runtime-benchmarks"))]
				None
			}
		}
	}
	fn bot_status(bot_id_hash: &BotIdHash) -> Option<BotStatus> {
		if matches!(bot_id_hash[0], 1 | 2 | 10 | 11) { return Some(BotStatus::Active); }
		#[cfg(feature = "runtime-benchmarks")]
		{
			if BENCH_BOT_OWNERS.with(|m| m.borrow().contains_key(&bot_id_hash[0])) {
				return Some(BotStatus::Active);
			}
		}
		None
	}
	fn attestation_level(_: &BotIdHash) -> u8 { 0 }
	fn tee_type(_: &BotIdHash) -> Option<TeeType> { None }
}

// Mock SubscriptionProvider: bot_hash(1)=Basic, bot_hash(2)=Basic, bot_hash(10/11)=Pro, others=Free
// Benchmark 模式下支持通过 BENCH_PAID_BOTS thread_local 动态注册 paid bot
pub struct MockSubscription;
impl SubscriptionProvider for MockSubscription {
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
		match bot_id_hash[0] {
			1 | 2 => return SubscriptionTier::Basic,
			10 | 11 => return SubscriptionTier::Pro,
			_ => {}
		}
		#[cfg(feature = "runtime-benchmarks")]
		{
			if BENCH_PAID_BOTS.with(|s| s.borrow().contains(&bot_id_hash[0])) {
				return SubscriptionTier::Basic;
			}
		}
		SubscriptionTier::Free
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

// Mock SubscriptionSettler
thread_local! {
	static SETTLE_INCOME: RefCell<u128> = RefCell::new(0);
	static SETTLE_TREASURY: RefCell<u128> = RefCell::new(0);
	static DISTRIBUTED_REWARDS: RefCell<Vec<(NodeId, u128)>> = RefCell::new(Vec::new());
	static PRUNED_ERA: RefCell<Option<u64>> = RefCell::new(None);
}

pub struct MockSubscriptionSettler;
impl SubscriptionSettler for MockSubscriptionSettler {
	fn settle_era() -> EraSettlementResult {
		let income = SETTLE_INCOME.with(|v| *v.borrow());
		let treasury = SETTLE_TREASURY.with(|v| *v.borrow());
		EraSettlementResult {
			total_income: income,
			node_share: income.saturating_sub(treasury),
			treasury_share: treasury,
		}
	}
}

pub fn set_mock_settle_income(income: u128) {
	SETTLE_INCOME.with(|v| *v.borrow_mut() = income);
	// L4-R2: 模拟 10% 国库分成
	SETTLE_TREASURY.with(|v| *v.borrow_mut() = income / 10);
}

// Mock EraRewardDistributor
pub struct MockRewardDistributor;
impl EraRewardDistributor for MockRewardDistributor {
	fn distribute_and_record(
		_era: u64,
		total_pool: u128,
		_subscription_income: u128,
		_ads_income: u128,
		_inflation: u128,
		_treasury_share: u128,
		node_weights: &[(NodeId, u128)],
		_node_count: u32,
	) -> u128 {
		let mut total_weight: u128 = 0;
		for (_, w) in node_weights.iter() {
			total_weight = total_weight.saturating_add(*w);
		}
		let mut distributed = 0u128;
		if total_weight > 0 {
			for (node_id, w) in node_weights.iter() {
				let reward = total_pool.saturating_mul(*w) / total_weight;
				if reward > 0 {
					DISTRIBUTED_REWARDS.with(|v| v.borrow_mut().push((*node_id, reward)));
					distributed = distributed.saturating_add(reward);
				}
			}
		}
		distributed
	}
	fn prune_old_eras(current_era: u64) {
		PRUNED_ERA.with(|v| *v.borrow_mut() = Some(current_era));
	}
}

pub fn get_distributed_rewards() -> Vec<(NodeId, u128)> {
	DISTRIBUTED_REWARDS.with(|v| v.borrow().clone())
}

pub fn clear_distributed_rewards() {
	DISTRIBUTED_REWARDS.with(|v| v.borrow_mut().clear());
}

// Mock OrphanRewardClaimer (H3-fix)
thread_local! {
	static ORPHAN_CLAIMS: RefCell<Vec<(NodeId, u64)>> = RefCell::new(Vec::new());
}

pub struct MockOrphanRewardClaimer;
impl OrphanRewardClaimer<u64> for MockOrphanRewardClaimer {
	fn try_claim_orphan_rewards(node_id: &NodeId, operator: &u64) {
		ORPHAN_CLAIMS.with(|v| v.borrow_mut().push((*node_id, *operator)));
	}
}

pub fn get_orphan_claims() -> Vec<(NodeId, u64)> {
	ORPHAN_CLAIMS.with(|v| v.borrow().clone())
}

// Mock PeerUptimeRecorder
thread_local! {
	static RECORDED_UPTIME_ERAS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
}

pub struct MockPeerUptimeRecorder;
impl PeerUptimeRecorder for MockPeerUptimeRecorder {
	fn record_era_uptime(era: u64) {
		RECORDED_UPTIME_ERAS.with(|v| v.borrow_mut().push(era));
	}
}

pub fn get_recorded_uptime_eras() -> Vec<u64> {
	RECORDED_UPTIME_ERAS.with(|v| v.borrow().clone())
}

parameter_types! {
	pub const MinStake: u128 = 100;
	pub const ExitCooldown: u64 = 10;
	pub const EraLength: u64 = 50;
	pub const InflationPerEra: u128 = 1000;
	pub const SlashPct: u32 = 10;
	pub const SequenceTtl: u64 = 100;
	pub const MaxSeqCleanup: u32 = 10;
}

impl pallet_grouprobot_consensus::Config for Test {
	type Currency = Balances;
	type WeightInfo = ();
	type MaxActiveNodes = frame_support::traits::ConstU32<10>;
	type MinStake = MinStake;
	type MinStakeUsd = frame_support::traits::ConstU64<50_000_000>;
	type DepositCalculator = ();
	type ExitCooldownPeriod = ExitCooldown;
	type EraLength = EraLength;
	type InflationPerEra = InflationPerEra;
	type SlashPercentage = SlashPct;
	type BotRegistry = MockBotRegistry;
	type SequenceTtlBlocks = SequenceTtl;
	type MaxSequenceCleanupPerBlock = MaxSeqCleanup;
	type SubscriptionSettler = MockSubscriptionSettler;
	type RewardDistributor = MockRewardDistributor;
	type Subscription = MockSubscription;
	type PeerUptimeRecorder = MockPeerUptimeRecorder;
	type OrphanRewardClaimer = MockOrphanRewardClaimer;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

pub const OWNER: u64 = 1;
pub const OWNER2: u64 = 2;
pub const OPERATOR: u64 = 10;
pub const OPERATOR2: u64 = 11;
pub const OTHER: u64 = 99;
pub const TREASURY: u64 = 200;

/// P14: 从确定性种子生成 ed25519 密钥对, 返回 (NodeId=公钥, Pair)
pub fn ed25519_pair(n: u8) -> (NodeId, ed25519::Pair) {
	let seed = format!("//Node{}", n);
	let pair = ed25519::Pair::from_string(&seed, None).unwrap();
	let public: [u8; 32] = pair.public().0;
	(public, pair)
}

/// 返回 NodeId (公钥), 现在基于真实 ed25519 密钥对
pub fn node_id(n: u8) -> NodeId {
	ed25519_pair(n).0
}

/// P14: 用指定节点的密钥对消息签名, 返回 (msg_hash, signature_bytes)
pub fn sign_msg(n: u8, msg: &[u8]) -> ([u8; 32], [u8; 64]) {
	let (_, pair) = ed25519_pair(n);
	// 使用 msg 的 blake2_256 哈希作为 msg_hash (但签名是对 msg_hash 签)
	let msg_hash = sp_core::blake2_256(msg);
	let sig = pair.sign(&msg_hash);
	(msg_hash, sig.0)
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
			(TREASURY, 100_000),
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

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<Test> for () {
	fn fund_account(who: &u64, amount: u128) {
		use frame_support::traits::Currency;
		let _ = <Balances as Currency<u64>>::make_free_balance_be(who, amount);
	}
	fn setup_tee_bot(bot_id_hash: &pallet_grouprobot_primitives::BotIdHash, owner: &u64) {
		BENCH_BOT_OWNERS.with(|m| m.borrow_mut().insert(bot_id_hash[0], *owner));
	}
	fn setup_active_bot(bot_id_hash: &pallet_grouprobot_primitives::BotIdHash, owner: &u64) {
		BENCH_BOT_OWNERS.with(|m| m.borrow_mut().insert(bot_id_hash[0], *owner));
	}
	fn setup_paid_subscription(bot_id_hash: &pallet_grouprobot_primitives::BotIdHash) {
		BENCH_PAID_BOTS.with(|s| s.borrow_mut().insert(bot_id_hash[0]));
	}
	fn node_id(seed: u32) -> pallet_grouprobot_primitives::NodeId {
		node_id(seed as u8)
	}
	fn bot_id_hash(seed: u32) -> pallet_grouprobot_primitives::BotIdHash {
		bot_hash(seed as u8)
	}
	fn sign_message(node_seed: u32, msg: &[u8]) -> ([u8; 32], [u8; 64]) {
		sign_msg(node_seed as u8, msg)
	}
}
