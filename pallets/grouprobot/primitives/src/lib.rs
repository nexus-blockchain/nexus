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

/// 节点类型 (标准 vs TEE)
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo,
	MaxEncodedLen,
)]
pub enum NodeType {
	/// 普通节点 (无 TEE 证明)
	StandardNode,
	/// TEE 节点 (TDX + 可选 SGX)
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
	Basic,
	Pro,
	Enterprise,
}

impl Default for SubscriptionTier {
	fn default() -> Self {
		Self::Basic
	}
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
		Self::Any
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
}

/// BotRegistryProvider 空实现 (用于不依赖 registry 的测试)
impl<AccountId> BotRegistryProvider<AccountId> for () {
	fn is_bot_active(_: &BotIdHash) -> bool { false }
	fn is_tee_node(_: &BotIdHash) -> bool { false }
	fn has_dual_attestation(_: &BotIdHash) -> bool { false }
	fn is_attestation_fresh(_: &BotIdHash) -> bool { false }
	fn bot_owner(_: &BotIdHash) -> Option<AccountId> { None }
	fn bot_public_key(_: &BotIdHash) -> Option<[u8; 32]> { None }
}

/// 社区管理查询 (consensus 依赖 community)
pub trait CommunityProvider<AccountId> {
	fn get_node_requirement(community_id_hash: &CommunityIdHash) -> NodeRequirement;
	fn is_community_bound(community_id_hash: &CommunityIdHash) -> bool;
}

/// CommunityProvider 空实现
impl<AccountId> CommunityProvider<AccountId> for () {
	fn get_node_requirement(_: &CommunityIdHash) -> NodeRequirement {
		NodeRequirement::Any
	}
	fn is_community_bound(_: &CommunityIdHash) -> bool { false }
}

/// 仪式查询 (registry 可选依赖 ceremony)
pub trait CeremonyProvider {
	fn is_ceremony_active(bot_public_key: &[u8; 32]) -> bool;
	fn ceremony_shamir_params(bot_public_key: &[u8; 32]) -> Option<(u8, u8)>;
}

impl CeremonyProvider for () {
	fn is_ceremony_active(_: &[u8; 32]) -> bool { false }
	fn ceremony_shamir_params(_: &[u8; 32]) -> Option<(u8, u8)> { None }
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
