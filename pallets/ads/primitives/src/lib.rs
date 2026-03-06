#![cfg_attr(not(feature = "std"), no_std)]

//! # Ads Primitives — 通用广告系统共享类型 + Trait 接口
//!
//! 所有 ads 子 pallet (core, grouprobot, entity 等) 依赖此 crate。
//! 无 Storage、无 Extrinsic，纯类型 + Trait 定义。
//!
//! ## 设计目标
//! 将广告系统的通用概念 (Campaign 状态、审核状态、偏好) 与领域特定概念
//! (GroupRobot 的 TEE 节点、Entity 的 Shop) 分离，使核心广告引擎可复用。

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;

// ============================================================================
// Type Aliases
// ============================================================================

/// 广告位 ID (32 bytes) — 泛化标识，对应 GroupRobot 的 CommunityIdHash 或 Entity 的 EntityId
pub type PlacementId = [u8; 32];

// ============================================================================
// Enums — 通用广告状态
// ============================================================================

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
	/// 治理暂停 (可恢复)
	Suspended,
	/// 审核中 (新建或修改后待审核)
	UnderReview,
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
	/// 举报
	Flagged,
}

impl Default for AdReviewStatus {
	fn default() -> Self {
		Self::Pending
	}
}

/// 广告活动类型
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum CampaignType {
	/// CPM — 按展示量计费
	Cpm,
	/// CPC — 按点击计费
	Cpc,
	/// 固定费用 (包时段/包位)
	Fixed,
	/// 私有广告 (仅指定广告位可见)
	Private,
}

impl Default for CampaignType {
	fn default() -> Self {
		Self::Cpm
	}
}

/// 广告位状态 (由适配层报告)
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub enum PlacementStatus {
	/// 正常接收广告
	Active,
	/// 管理员/Owner 主动暂停
	Paused,
	/// 被永久禁止
	Banned,
	/// 未注册或不存在
	Unknown,
}

impl Default for PlacementStatus {
	fn default() -> Self {
		Self::Unknown
	}
}

// ============================================================================
// Click Attestation — C2b Proxy Account 点击证明
// ============================================================================

/// 点击证明 — 由用户的 Proxy Account 签名的点击事件
///
/// C2b 方案: 用户主账户通过 `proxy.addProxy` 委托有限签名权给 DApp 管理的 proxy 账户。
/// 用户点击广告时, DApp 自动使用 proxy 账户签名生成 ClickAttestation。
/// Entity 将批量 attestation 聚合后提交上链。
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq,
	TypeInfo, MaxEncodedLen,
)]
pub struct ClickAttestation<AccountId> {
	/// 点击者 (用户主账户)
	pub clicker: AccountId,
	/// 签名者 (proxy 账户, 已被 clicker 授权)
	pub proxy: AccountId,
	/// 点击的 Campaign ID
	pub campaign_id: u64,
	/// 广告位 ID
	pub placement_id: PlacementId,
	/// 点击时间戳 (区块号)
	pub clicked_at: u64,
}

// ============================================================================
// Trait Interfaces — 适配层需实现
// ============================================================================

/// 投放收据验证 — 各适配层实现投放真实性验证
///
/// GroupRobot: TEE 节点签名验证 + 订阅层级门控
/// Entity: Entity 活跃状态检查 + 展示量验证
pub trait DeliveryVerifier<AccountId> {
	/// 验证投放收据的合法性
	///
	/// - `who`: 提交者
	/// - `placement_id`: 广告位
	/// - `audience_size`: 受众规模
	/// - `node_id`: 投放节点 ID (GroupRobot: TEE 节点; Entity: None)
	///
	/// 返回: 验证通过后的有效受众数 (可能被裁切)
	fn verify_and_cap_audience(
		who: &AccountId,
		placement_id: &PlacementId,
		audience_size: u32,
		node_id: Option<[u8; 32]>,
	) -> Result<u32, sp_runtime::DispatchError>;
}

/// DeliveryVerifier 空实现 (直通, 用于测试)
/// 返回原始 audience_size 不做裁切。
impl<AccountId> DeliveryVerifier<AccountId> for () {
	fn verify_and_cap_audience(
		_: &AccountId,
		_: &PlacementId,
		audience_size: u32,
		_node_id: Option<[u8; 32]>,
	) -> Result<u32, sp_runtime::DispatchError> {
		Ok(audience_size)
	}
}

/// 点击收据验证 — 各适配层实现点击真实性验证与每日上限
///
/// Entity: Entity 活跃状态检查 + 每日点击量上限 + 权限验证
pub trait ClickVerifier<AccountId> {
	/// 验证点击收据的合法性
	///
	/// - `who`: 提交者 (Entity owner/admin/shop manager)
	/// - `placement_id`: 广告位
	/// - `click_count`: 本次提交的点击数
	/// - `verified_clicks`: 经 proxy 签名验证的点击数 (C2b)
	///
	/// 返回: 验证通过后的有效点击数 (可能被每日上限裁切)
	fn verify_and_cap_clicks(
		who: &AccountId,
		placement_id: &PlacementId,
		click_count: u32,
		verified_clicks: u32,
	) -> Result<u32, sp_runtime::DispatchError>;
}

/// ClickVerifier 空实现 (直通, 用于测试)
impl<AccountId> ClickVerifier<AccountId> for () {
	fn verify_and_cap_clicks(
		_: &AccountId,
		_: &PlacementId,
		click_count: u32,
		_verified_clicks: u32,
	) -> Result<u32, sp_runtime::DispatchError> {
		Ok(click_count)
	}
}

/// 广告位管理员解析 — 各适配层映射广告位到管理员
///
/// GroupRobot: CommunityAdmin (首个质押者 / Bot Owner)
/// Entity: Entity Owner / Shop Admin
pub trait PlacementAdminProvider<AccountId> {
	/// 查询广告位管理员
	fn placement_admin(placement_id: &PlacementId) -> Option<AccountId>;
	/// 广告位是否被永久禁止
	fn is_placement_banned(placement_id: &PlacementId) -> bool;
	/// 查询广告位当前状态 (Active/Paused/Banned/Unknown)
	fn placement_status(placement_id: &PlacementId) -> PlacementStatus;
}

impl<AccountId> PlacementAdminProvider<AccountId> for () {
	fn placement_admin(_: &PlacementId) -> Option<AccountId> { None }
	fn is_placement_banned(_: &PlacementId) -> bool { false }
	fn placement_status(_: &PlacementId) -> PlacementStatus { PlacementStatus::Unknown }
}

/// 收入分配明细
#[derive(RuntimeDebug, Clone, PartialEq, Eq)]
pub struct RevenueBreakdown<Balance> {
	/// 广告位方可提取份额 (社区/Entity Owner)
	pub placement_share: Balance,
	/// 节点份额 (GroupRobot: TEE 节点; Entity: 0)
	pub node_share: Balance,
	/// 平台/国库份额
	pub platform_share: Balance,
}

/// 结算后的收入分配策略 — 各适配层定义分成比例和分配逻辑
///
/// GroupRobot: 三方分成 (社区/国库/节点)
/// Entity: 二方分成 (Entity Owner/平台)
pub trait RevenueDistributor<AccountId, Balance> {
	/// 分配广告收入
	///
	/// - `placement_id`: 广告位
	/// - `total_cost`: 本次总费用
	/// - `advertiser`: 广告主账户
	///
	/// 返回: 收入分配明细 (RevenueBreakdown)
	fn distribute(
		placement_id: &PlacementId,
		total_cost: Balance,
		advertiser: &AccountId,
	) -> Result<RevenueBreakdown<Balance>, sp_runtime::DispatchError>;
}

/// `()` 空实现: 广告位方份额为零 (全部收入归国库)。
/// 生产环境必须提供领域适配层实现。
impl<AccountId, Balance: Default> RevenueDistributor<AccountId, Balance> for () {
	fn distribute(
		_: &PlacementId,
		total_cost: Balance,
		_: &AccountId,
	) -> Result<RevenueBreakdown<Balance>, sp_runtime::DispatchError> {
		Ok(RevenueBreakdown {
			placement_share: Balance::default(),
			node_share: Balance::default(),
			platform_share: total_cost,
		})
	}
}

/// 广告排期查询 (其他 pallet 查询广告状态)
pub trait AdScheduleProvider {
	/// 广告位是否启用广告
	fn is_ads_enabled(placement_id: &PlacementId) -> bool;
	/// 广告位累计广告收入 (Balance 用 u128 表示)
	fn placement_ad_revenue(placement_id: &PlacementId) -> u128;
	/// 广告位当前 Era 广告收入 (Balance 用 u128 表示)
	fn placement_era_revenue(placement_id: &PlacementId) -> u128;
}

impl AdScheduleProvider for () {
	fn is_ads_enabled(_: &PlacementId) -> bool { false }
	fn placement_ad_revenue(_: &PlacementId) -> u128 { 0 }
	fn placement_era_revenue(_: &PlacementId) -> u128 { 0 }
}

/// 广告投放计数查询 (外部 pallet 查询投放达标情况)
pub trait AdDeliveryCountProvider {
	/// 查询广告位在当前 Era 的广告投放次数
	fn era_delivery_count(placement_id: &PlacementId) -> u32;
	/// 重置广告位的 Era 投放计数 (Era 结算后调用)
	fn reset_era_deliveries(placement_id: &PlacementId);
}

impl AdDeliveryCountProvider for () {
	fn era_delivery_count(_: &PlacementId) -> u32 { 0 }
	fn reset_era_deliveries(_: &PlacementId) {}
}

/// 广告策略查询 — 治理层或适配层提供的广告投放策略参数
///
/// 用于限制广告位的投放行为 (每广告位最大活动数、最低预算、审核策略等)。
pub trait AdPolicyProvider {
	/// 广告位允许的最大并发活动数 (0 = 无限制)
	fn max_campaigns_per_placement(placement_id: &PlacementId) -> u32;
	/// 创建活动的最低预算 (u128, 0 = 无门槛)
	fn min_campaign_budget(placement_id: &PlacementId) -> u128;
	/// 新活动是否需要审核
	fn requires_review(placement_id: &PlacementId) -> bool;
}

impl AdPolicyProvider for () {
	fn max_campaigns_per_placement(_: &PlacementId) -> u32 { 0 }
	fn min_campaign_budget(_: &PlacementId) -> u128 { 0 }
	fn requires_review(_: &PlacementId) -> bool { false }
}

/// 广告位配置查询 — 适配层提供的广告位级别参数
///
/// GroupRobot: 质押量 → 受众上限映射
/// Entity: 广告位级别 → 每日展示量上限
pub trait PlacementConfigProvider {
	/// 广告位每日展示量上限 (0 = 无限制)
	fn daily_impression_cap(placement_id: &PlacementId) -> u32;
	/// 广告位收入分成比例 (基点, 10000 = 100%)
	fn revenue_share_bps(placement_id: &PlacementId) -> u32;
	/// 广告位是否支持私有广告
	fn supports_private_ads(placement_id: &PlacementId) -> bool;
}

impl PlacementConfigProvider for () {
	fn daily_impression_cap(_: &PlacementId) -> u32 { 0 }
	fn revenue_share_bps(_: &PlacementId) -> u32 { 0 }
	fn supports_private_ads(_: &PlacementId) -> bool { false }
}
