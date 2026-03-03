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

/// 双向偏好控制 (广告主 ⇄ 广告位)
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

// ============================================================================
// Trait Interfaces — 适配层需实现
// ============================================================================

/// 投放方式 trait — 各适配层自定义投放类型
///
/// GroupRobot: ScheduledPost / ReplyFooter / WelcomeEmbed
/// Entity: BannerAd / ProductPlacement / SponsoredListing
pub trait DeliveryMethod:
	Encode + Decode + Clone + MaxEncodedLen + TypeInfo + core::fmt::Debug + PartialEq + Eq
{
	/// CPM 定价系数 (百分比整数, 100 = 1.0x, 200 = 2.0x)
	///
	/// 注意: 此处并非金融基点 (1/10000)，而是百分比整数。
	/// ads-core 计算公式: bid * audience * multiplier / 100_000
	/// 其中 /1000 为 CPM 标准 (每千人)，/100 为此系数归一化。
	fn cpm_multiplier_bps(&self) -> u32;
}

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
	///
	/// 返回: 验证通过后的有效受众数 (可能被裁切)
	fn verify_and_cap_audience(
		who: &AccountId,
		placement_id: &PlacementId,
		audience_size: u32,
	) -> Result<u32, sp_runtime::DispatchError>;
}

/// DeliveryVerifier 空实现 (直通, 用于测试)
/// 返回原始 audience_size 不做裁切。
impl<AccountId> DeliveryVerifier<AccountId> for () {
	fn verify_and_cap_audience(
		_: &AccountId,
		_: &PlacementId,
		audience_size: u32,
	) -> Result<u32, sp_runtime::DispatchError> {
		Ok(audience_size)
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
}

impl<AccountId> PlacementAdminProvider<AccountId> for () {
	fn placement_admin(_: &PlacementId) -> Option<AccountId> { None }
	fn is_placement_banned(_: &PlacementId) -> bool { false }
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
	/// 返回: 广告位方可提取的份额
	fn distribute(
		placement_id: &PlacementId,
		total_cost: Balance,
		advertiser: &AccountId,
	) -> Result<Balance, sp_runtime::DispatchError>;
}

/// `()` 空实现: 广告位方份额为零 (全部收入归国库)。
/// 生产环境必须提供领域适配层实现。
impl<AccountId, Balance: Default> RevenueDistributor<AccountId, Balance> for () {
	fn distribute(
		_: &PlacementId,
		_: Balance,
		_: &AccountId,
	) -> Result<Balance, sp_runtime::DispatchError> {
		Ok(Balance::default())
	}
}

/// 广告位质押管理 — 可选，广告位可通过质押获取受众上限
///
/// GroupRobot: 质押 → audience_cap 阶梯函数
/// Entity: 可能不需要 (网页展示量由流量决定)
pub trait PlacementStakeProvider<Balance> {
	/// 查询广告位的受众上限
	fn audience_cap(placement_id: &PlacementId) -> u32;
	/// 查询广告位的质押额
	fn stake_amount(placement_id: &PlacementId) -> Balance;
}

/// `()` 空实现: 无质押 → audience_cap = 0 (安全方向: 禁止无质押广告投放)。
/// 生产环境由适配层根据质押额动态计算 audience_cap。
impl<Balance: Default> PlacementStakeProvider<Balance> for () {
	fn audience_cap(_: &PlacementId) -> u32 { 0 }
	fn stake_amount(_: &PlacementId) -> Balance { Balance::default() }
}

/// 广告排期查询 (其他 pallet 查询广告状态)
pub trait AdScheduleProvider {
	/// 广告位是否启用广告
	fn is_ads_enabled(placement_id: &PlacementId) -> bool;
	/// 广告位累计广告收入 (Balance 用 u128 表示)
	fn placement_ad_revenue(placement_id: &PlacementId) -> u128;
}

impl AdScheduleProvider for () {
	fn is_ads_enabled(_: &PlacementId) -> bool { false }
	fn placement_ad_revenue(_: &PlacementId) -> u128 { 0 }
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
