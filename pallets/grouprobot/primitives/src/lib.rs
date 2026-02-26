#![cfg_attr(not(feature = "std"), no_std)]

//! # GroupRobot Primitives — 共享类型 + Trait 接口
//!
//! 所有 grouprobot 子 pallet 依赖此 crate。
//! 无 Storage、无 Extrinsic，纯类型 + Trait 定义。

extern crate alloc;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;

// ============================================================================
// Type Aliases
// ============================================================================

/// 节点 ID (32 bytes)
pub type NodeId = [u8; 32];

/// Bot ID Hash (32 bytes)
pub type BotIdHash = [u8; 32];

/// 社区 ID Hash (32 bytes)
pub type CommunityIdHash = [u8; 32];

// ============================================================================
// Enums
// ============================================================================

/// 社交平台枚举
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum Platform {
	Telegram,
	Discord,
	Slack,
	Matrix,
	Farcaster,
}

impl Default for Platform {
	fn default() -> Self {
		Self::Telegram
	}
}

/// Bot 状态
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum BotStatus {
	Active,
	Suspended,
	Deactivated,
}

impl Default for BotStatus {
	fn default() -> Self {
		Self::Active
	}
}

/// TEE 硬件类型
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum TeeType {
	/// TDX-Only: primary_measurement = MRTD (48 bytes)
	Tdx,
	/// SGX-Only: primary_measurement = MRENCLAVE (32 bytes, padded to 48)
	Sgx,
	/// TDX + SGX 双证明: primary_measurement = MRTD, mrenclave = Some(...)
	TdxPlusSgx,
}

impl Default for TeeType {
	fn default() -> Self {
		Self::Tdx
	}
}

/// 节点类型 (标准 vs TEE)
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum NodeType {
	/// 普通节点 (无 TEE 证明)
	StandardNode,
	/// V1: TEE 节点 (TDX + 可选 SGX, 向后兼容)
	TeeNode {
		/// TDX Trust Domain 度量值 (48 bytes)
		mrtd: [u8; 48],
		/// SGX Enclave 度量值 (32 bytes, 可选)
		mrenclave: Option<[u8; 32]>,
		/// TDX Quote 提交区块
		tdx_attested_at: u64,
		/// SGX Quote 提交区块
		sgx_attested_at: Option<u64>,
		/// 证明过期区块
		expires_at: u64,
	},
	/// V2: 三模式统一 TEE 节点 (SGX-Only / TDX-Only / TDX+SGX)
	TeeNodeV2 {
		/// 统一度量值: MRTD(48B) 或 MRENCLAVE(32B + 16B zero-pad)
		primary_measurement: [u8; 48],
		/// TEE 类型
		tee_type: TeeType,
		/// SGX MRENCLAVE (原始 32B; TDX+SGX 双证明时有值, SGX-Only 时同 primary[..32])
		mrenclave: Option<[u8; 32]>,
		/// 主证明提交区块
		attested_at: u64,
		/// 补充 SGX 证明提交区块 (仅 TDX+SGX)
		sgx_attested_at: Option<u64>,
		/// 证明过期区块
		expires_at: u64,
	},
}

impl Default for NodeType {
	fn default() -> Self {
		Self::StandardNode
	}
}

/// 节点状态
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum NodeStatus {
	Active,
	Probation,
	Suspended,
	Exiting,
}

impl Default for NodeStatus {
	fn default() -> Self {
		Self::Active
	}
}

/// 暂停原因
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum SuspendReason {
	LowReputation,
	Equivocation,
	Offline,
	Manual,
}

/// 订阅层级
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum SubscriptionTier {
	/// 免费层级 (默认, 无链上订阅记录)
	Free,
	Basic,
	Pro,
	Enterprise,
}

impl Default for SubscriptionTier {
	fn default() -> Self {
		Self::Free
	}
}

impl SubscriptionTier {
	/// 该层级是否需要付费订阅
	pub fn is_paid(&self) -> bool {
		!matches!(self, Self::Free)
	}

	/// 获取该层级的功能限制
	pub fn feature_gate(&self) -> TierFeatureGate {
		match self {
			Self::Free => TierFeatureGate {
				max_rules: 3,
				log_retention_days: 7,
				forced_ads_per_day: 2,
				can_disable_ads: false,
				tee_access: false,
				ad_revenue_community_pct: 60,
				ad_revenue_treasury_pct: 25,
				ad_revenue_node_pct: 15,
			},
			Self::Basic => TierFeatureGate {
				max_rules: 10,
				log_retention_days: 30,
				forced_ads_per_day: 0,
				can_disable_ads: true,
				tee_access: false,
				ad_revenue_community_pct: 70,
				ad_revenue_treasury_pct: 20,
				ad_revenue_node_pct: 10,
			},
			Self::Pro => TierFeatureGate {
				max_rules: 50,
				log_retention_days: 90,
				forced_ads_per_day: 0,
				can_disable_ads: true,
				tee_access: true,
				ad_revenue_community_pct: 0,
				ad_revenue_treasury_pct: 0,
				ad_revenue_node_pct: 0,
			},
			Self::Enterprise => TierFeatureGate {
				max_rules: u16::MAX,
				log_retention_days: 0, // 0 = 永久
				forced_ads_per_day: 0,
				can_disable_ads: true,
				tee_access: true,
				ad_revenue_community_pct: 0,
				ad_revenue_treasury_pct: 0,
				ad_revenue_node_pct: 0,
			},
		}
	}
}

/// 层级功能限制
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub struct TierFeatureGate {
	/// 最大可启用规则数
	pub max_rules: u16,
	/// 日志保留天数 (0 = 永久)
	pub log_retention_days: u16,
	/// 每日强制广告数 (0 = 无强制)
	pub forced_ads_per_day: u8,
	/// 是否可关闭广告
	pub can_disable_ads: bool,
	/// 是否可使用 TEE 节点
	pub tee_access: bool,
	/// 广告收入分成: 社区 % (deprecated: 实际由 ads pallet 治理配置)
	pub ad_revenue_community_pct: u8,
	/// 广告收入分成: 国库 % (deprecated: 实际由 ads pallet 治理配置)
	pub ad_revenue_treasury_pct: u8,
	/// 广告收入分成: 节点 % (deprecated: 实际由 ads pallet 治理配置)
	pub ad_revenue_node_pct: u8,
}

/// 订阅状态
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum SubscriptionStatus {
	Active,
	PastDue,
	Suspended,
	Cancelled,
}

impl Default for SubscriptionStatus {
	fn default() -> Self {
		Self::Active
	}
}

/// 动作类型
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum ActionType {
	Kick,
	Ban,
	Mute,
	Warn,
	Unmute,
	Unban,
	Promote,
	Demote,
	Welcome,
	ConfigUpdate(ConfigUpdateAction),
}

/// 配置更新动作
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum ConfigUpdateAction {
	AddBlacklistWord,
	RemoveBlacklistWord,
	LockType,
	UnlockType,
	SetWelcome,
	SetFloodLimit,
	SetWarnLimit,
	SetWarnAction,
}

/// 节点准入策略
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum NodeRequirement {
	/// 任意节点
	Any,
	/// 仅 TEE 节点
	TeeOnly,
	/// TEE 优先 (有 TEE 时优先调度)
	TeePreferred,
	/// 最低 TEE 节点数
	MinTee(u32),
}

impl Default for NodeRequirement {
	fn default() -> Self {
		Self::TeeOnly
	}
}

/// 警告达限后动作
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum WarnAction {
	Kick,
	Ban,
	Mute,
}

impl Default for WarnAction {
	fn default() -> Self {
		Self::Kick
	}
}

// ============================================================================
// Ad System Types (群组广告)
// ============================================================================

/// 广告投放类型
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum AdDeliveryType {
	/// 定时推送到群组
	ScheduledPost,
	/// Bot 回复底部附带广告
	ReplyFooter,
	/// 嵌入欢迎消息
	WelcomeEmbed,
}

impl Default for AdDeliveryType {
	fn default() -> Self {
		Self::ScheduledPost
	}
}

/// 广告目标标签 (用于匹配社区)
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum AdTargetTag {
	/// 按平台
	TargetPlatform(Platform),
	/// 按社区最低活跃成员数
	MinMembers(u32),
	/// 按语言/地区 (ISO 639-1)
	Language([u8; 2]),
	/// 全部社区
	All,
}

impl Default for AdTargetTag {
	fn default() -> Self {
		Self::All
	}
}

/// 双向偏好控制 (广告主 ⇄ 群组)
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum AdPreference {
	/// 默认: 允许
	Allow,
	/// 拉黑
	Blocked,
	/// 指定/白名单 (优先匹配)
	Preferred,
}

impl Default for AdPreference {
	fn default() -> Self {
		Self::Allow
	}
}

/// 广告活动状态
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum CampaignStatus {
	Active,
	Paused,
	/// 预算耗尽
	Exhausted,
	Expired,
	Cancelled,
}

impl Default for CampaignStatus {
	fn default() -> Self {
		Self::Active
	}
}

/// 广告审核状态
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum AdReviewStatus {
	Pending,
	Approved,
	Rejected,
	/// 社区举报
	Flagged,
}

impl Default for AdReviewStatus {
	fn default() -> Self {
		Self::Pending
	}
}

// ============================================================================
// Ceremony Types
// ============================================================================

/// 仪式状态
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum CeremonyStatus {
	Active,
	Superseded { replaced_by: [u8; 32] },
	Revoked { revoked_at: u64 },
	Expired,
}

impl Default for CeremonyStatus {
	fn default() -> Self {
		Self::Active
	}
}

// ============================================================================
// Trait Interfaces
// ============================================================================

/// Bot 注册查询 (consensus/ceremony 依赖 registry)
pub trait BotRegistryProvider<AccountId> {
	fn is_bot_active(bot_id_hash: &BotIdHash) -> bool;
	fn is_tee_node(bot_id_hash: &BotIdHash) -> bool;
	fn has_dual_attestation(bot_id_hash: &BotIdHash) -> bool;
	fn is_attestation_fresh(bot_id_hash: &BotIdHash) -> bool;
	fn bot_owner(bot_id_hash: &BotIdHash) -> Option<AccountId>;
	fn bot_public_key(bot_id_hash: &BotIdHash) -> Option<[u8; 32]>;
	/// 获取 Bot 的存活 Peer 数量
	fn peer_count(bot_id_hash: &BotIdHash) -> u32;
}

/// BotRegistryProvider 空实现 (用于不依赖 registry 的测试)
impl<AccountId> BotRegistryProvider<AccountId> for () {
	fn is_bot_active(_: &BotIdHash) -> bool { false }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(_: &BotIdHash) -> Option<AccountId> { None }
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
	fn peer_count(_: &BotIdHash) -> u32 { 0 }
}

/// 社区管理查询 (consensus 依赖 community)
pub trait CommunityProvider<AccountId> {
	fn get_node_requirement(community_id_hash: &CommunityIdHash) -> NodeRequirement;
	fn is_community_bound(community_id_hash: &CommunityIdHash) -> bool;
}

/// CommunityProvider 空实现
impl<AccountId> CommunityProvider<AccountId> for () {
	fn get_node_requirement(_: &CommunityIdHash) -> NodeRequirement {
		NodeRequirement::TeeOnly
	}
	fn is_community_bound(_: &CommunityIdHash) -> bool { false }
}

/// 仪式查询 (registry 可选依赖 ceremony)
pub trait CeremonyProvider {
	fn is_ceremony_active(bot_public_key: &[u8; 32]) -> bool;
	fn ceremony_shamir_params(bot_public_key: &[u8; 32]) -> Option<(u8, u8)>;
	/// 获取活跃仪式哈希
	fn active_ceremony_hash(bot_public_key: &[u8; 32]) -> Option<[u8; 32]>;
	/// 获取活跃仪式参与者数量
	fn ceremony_participant_count(bot_public_key: &[u8; 32]) -> Option<u8>;
}

impl CeremonyProvider for () {
	fn is_ceremony_active(_: &[u8; 32]) -> bool { false }
	fn ceremony_shamir_params(_: &[u8; 32]) -> Option<(u8, u8)> { None }
	fn active_ceremony_hash(_: &[u8; 32]) -> Option<[u8; 32]> { None }
	fn ceremony_participant_count(_: &[u8; 32]) -> Option<u8> { None }
}

/// 声誉查询 (其他 pallet 可查询用户声誉)
pub trait ReputationProvider {
	/// 获取用户在社区的本地声誉
	fn get_reputation(community_id_hash: &CommunityIdHash, user_hash: &[u8; 32]) -> i64;
	/// 获取用户全局声誉
	fn get_global_reputation(user_hash: &[u8; 32]) -> i64;
}

impl ReputationProvider for () {
	fn get_reputation(_: &CommunityIdHash, _: &[u8; 32]) -> i64 { 0 }
	fn get_global_reputation(_: &[u8; 32]) -> i64 { 0 }
}

/// 广告排期查询 (Bot 侧 / 其他 pallet 查询广告投放信息)
pub trait AdScheduleProvider {
	/// 社区是否启用广告
	fn is_ads_enabled(community_id_hash: &CommunityIdHash) -> bool;
	/// 社区累计广告收入 (Balance 用 u128 表示)
	fn community_ad_revenue(community_id_hash: &CommunityIdHash) -> u128;
}

impl AdScheduleProvider for () {
	fn is_ads_enabled(_: &CommunityIdHash) -> bool { false }
	fn community_ad_revenue(_: &CommunityIdHash) -> u128 { 0 }
}

/// 节点共识查询 (community 可选依赖 consensus)
pub trait NodeConsensusProvider<AccountId> {
	fn is_node_active(node_id: &NodeId) -> bool;
	fn node_operator(node_id: &NodeId) -> Option<AccountId>;
	fn is_tee_node_by_operator(operator: &AccountId) -> bool;
}

impl<AccountId> NodeConsensusProvider<AccountId> for () {
	fn is_node_active(_: &NodeId) -> bool { false }
	fn node_operator(_: &NodeId) -> Option<AccountId> { None }
	fn is_tee_node_by_operator(_: &AccountId) -> bool { false }
}

/// 🆕 10.2/10.4: 订阅查询 trait (为未来 subscription pallet 拆分准备)
///
/// 允许 ads 等 pallet 查询 Bot 的有效订阅层级, 无需直接依赖 consensus pallet.
pub trait SubscriptionProvider {
	/// 查询 Bot 的有效订阅层级 (无订阅 = Free)
	fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier;
	/// 查询 Bot 的功能限制
	fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate;
}

impl SubscriptionProvider for () {
	fn effective_tier(_: &BotIdHash) -> SubscriptionTier { SubscriptionTier::Free }
	fn effective_feature_gate(_: &BotIdHash) -> TierFeatureGate { SubscriptionTier::Free.feature_gate() }
}

/// 🆕 10.4: 统一奖励写入 trait (为未来 rewards pallet 拆分准备)
///
/// ads 和 subscription 都通过此 trait 向同一奖励池写入节点奖励,
/// 节点只需调用一次 claim 即可领取全部来源的奖励.
pub trait RewardAccruer {
	/// 向节点累加待领取奖励
	fn accrue_node_reward(node_id: &NodeId, amount: u128);
}

impl RewardAccruer for () {
	fn accrue_node_reward(_: &NodeId, _: u128) {}
}

/// 订阅结算 trait (consensus on_era_end 调用 subscription pallet)
pub trait SubscriptionSettler {
	/// 结算当前 Era 的订阅费, 返回本次收取的总收入 (u128)
	fn settle_era() -> u128;
}

impl SubscriptionSettler for () {
	fn settle_era() -> u128 { 0 }
}

/// Era 奖励分配 trait (consensus on_era_end 调用 rewards pallet)
pub trait EraRewardDistributor {
	/// 向节点分配奖励并记录 Era 信息
	///
	/// - `era`: 当前 Era 编号
	/// - `total_pool`: 可分配总额 (subscription node_share + inflation)
	/// - `subscription_income`: 本期订阅收入
	/// - `inflation`: 本期通胀铸币
	/// - `treasury_share`: 国库分成
	/// - `node_weights`: 各节点权重 (node_id, weight)
	/// - `node_count`: 活跃节点数
	///
	/// 返回实际分配的总额
	fn distribute_and_record(
		era: u64,
		total_pool: u128,
		subscription_income: u128,
		inflation: u128,
		treasury_share: u128,
		node_weights: &[(NodeId, u128)],
		node_count: u32,
	) -> u128;

	/// 清理过期 Era 奖励记录
	fn prune_old_eras(current_era: u64);
}

impl EraRewardDistributor for () {
	fn distribute_and_record(
		_: u64, _: u128, _: u128, _: u128, _: u128, _: &[(NodeId, u128)], _: u32,
	) -> u128 { 0 }
	fn prune_old_eras(_: u64) {}
}
