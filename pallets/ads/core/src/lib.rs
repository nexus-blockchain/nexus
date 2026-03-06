#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Ads Core — 通用广告引擎
//!
//! ## 功能
//! - Campaign CRUD (创建/追加预算/暂停/取消/审核/举报)
//! - Escrow 预算托管
//! - 投放收据提交 + Era 结算
//! - 双向偏好: 广告主 ⇄ 广告位 黑名单/白名单
//! - 广告位收入提取
//! - Slash/惩罚机制
//! - 私域广告登记
//!
//! ## 设计
//! 核心引擎不包含任何领域特定逻辑 (TEE 节点、Bot 订阅等)。
//! 通过 Config trait 中的适配层 trait 注入领域行为:
//! - `DeliveryVerifier`: 投放验证 (TEE 签名 / Entity 活跃检查)
//! - `PlacementAdminProvider`: 广告位管理员 (Bot Owner / Entity Owner)
//! - `RevenueDistributor`: 收入分配策略 (三方分成 / 二方分成)

pub use pallet::*;
pub mod runtime_api;
pub use runtime_api::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

extern crate alloc;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_ads_primitives::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{Saturating, Zero};

/// Balance 类型别名
type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

// ============================================================================
// 核心数据结构
// ============================================================================

/// 广告活动
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AdCampaign<T: Config> {
	pub advertiser: T::AccountId,
	/// 广告文本 (≤280 字节)
	pub text: BoundedVec<u8, T::MaxAdTextLength>,
	/// 链接 URL (≤256 字节)
	pub url: BoundedVec<u8, T::MaxAdUrlLength>,
	/// 每千人触达出价 (CPM)
	pub bid_per_mille: BalanceOf<T>,
	/// 每次点击出价 (CPC) — 仅 CampaignType::Cpc 使用
	pub bid_per_click: BalanceOf<T>,
	/// 活动类型 (CPM / CPC / Fixed / Private)
	pub campaign_type: CampaignType,
	/// 每日预算上限
	pub daily_budget: BalanceOf<T>,
	/// 总预算
	pub total_budget: BalanceOf<T>,
	/// 已花费
	pub spent: BalanceOf<T>,
	/// 投放类型 bitmask
	pub delivery_types: u8,
	/// 活动状态
	pub status: CampaignStatus,
	/// 审核状态
	pub review_status: AdReviewStatus,
	/// 累计投放次数
	pub total_deliveries: u64,
	/// 累计点击次数 (CPC)
	pub total_clicks: u64,
	/// 创建区块
	pub created_at: BlockNumberFor<T>,
	/// 过期区块
	pub expires_at: BlockNumberFor<T>,
}

/// 投放收据
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct DeliveryReceipt<T: Config> {
	pub campaign_id: u64,
	pub placement_id: PlacementId,
	/// 受众规模 (已裁切) — CPM 模式使用
	pub audience_size: u32,
	/// 点击数 (已裁切) — CPC 模式使用
	pub click_count: u32,
	/// 经 proxy 签名验证的点击数 — CPC-Verified 模式
	pub verified_clicks: u32,
	/// CPM 倍率 (基点)
	pub cpm_multiplier_bps: u32,
	pub delivered_at: BlockNumberFor<T>,
	/// 是否已结算
	pub settled: bool,
	/// 提交者
	pub submitter: T::AccountId,
}

/// 收据确认状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum ReceiptStatus {
	/// 等待广告主确认 (确认窗口内)
	Pending,
	/// 广告主已确认
	Confirmed,
	/// 广告主发起争议
	Disputed,
	/// 确认窗口已过期, 自动确认
	AutoConfirmed,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::{Currency, ReservableCurrency, ExistenceRequirement};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: ReservableCurrency<Self::AccountId>;

		/// 广告文本最大长度
		#[pallet::constant]
		type MaxAdTextLength: Get<u32>;

		/// 广告 URL 最大长度
		#[pallet::constant]
		type MaxAdUrlLength: Get<u32>;

		/// 每广告位最大收据数
		#[pallet::constant]
		type MaxReceiptsPerPlacement: Get<u32>;

		/// 广告主黑名单上限
		#[pallet::constant]
		type MaxAdvertiserBlacklist: Get<u32>;

		/// 广告主白名单上限
		#[pallet::constant]
		type MaxAdvertiserWhitelist: Get<u32>;

		/// 广告位黑名单上限
		#[pallet::constant]
		type MaxPlacementBlacklist: Get<u32>;

		/// 广告位白名单上限
		#[pallet::constant]
		type MaxPlacementWhitelist: Get<u32>;

		/// 最低 CPM 出价
		#[pallet::constant]
		type MinBidPerMille: Get<BalanceOf<Self>>;

		/// 接入广告的最低受众人数
		#[pallet::constant]
		type MinAudienceSize: Get<u32>;

		/// Slash 百分比 (e.g. 30 = 30%)
		#[pallet::constant]
		type AdSlashPercentage: Get<u32>;

		/// 国库账户
		type TreasuryAccount: Get<Self::AccountId>;

		/// 投放验证器 (适配层实现)
		type DeliveryVerifier: DeliveryVerifier<Self::AccountId>;

		/// 点击验证器 (适配层实现, C2b Proxy Account)
		type ClickVerifier: ClickVerifier<Self::AccountId>;

		/// 最低 CPC 出价
		#[pallet::constant]
		type MinBidPerClick: Get<BalanceOf<Self>>;

		/// 广告位管理员 (适配层实现)
		type PlacementAdmin: PlacementAdminProvider<Self::AccountId>;

		/// 收入分配策略 (适配层实现)
		type RevenueDistributor: RevenueDistributor<Self::AccountId, BalanceOf<Self>>;

		/// 私有广告注册费用
		#[pallet::constant]
		type PrivateAdRegistrationFee: Get<BalanceOf<Self>>;

		/// 结算激励 (基点, e.g. 10 = 0.1%)
		#[pallet::constant]
		type SettlementIncentiveBps: Get<u32>;

		/// 每广告主最大 Campaign 数
		#[pallet::constant]
		type MaxCampaignsPerAdvertiser: Get<u32>;

		/// 每 Campaign 最大定向广告位数 (0 = 不限制)
		#[pallet::constant]
		type MaxTargetsPerCampaign: Get<u32>;

		/// 收据确认窗口 (区块数, 建议 7200 ≈ 12h)
		#[pallet::constant]
		type ReceiptConfirmationWindow: Get<BlockNumberFor<Self>>;

		/// 广告主推荐佣金比例 (基点, 500 = 5%, 从平台份额中扣)
		#[pallet::constant]
		type AdvertiserReferralRate: Get<u32>;

		/// 每人最多推荐广告主数
		#[pallet::constant]
		type MaxReferredAdvertisers: Get<u32>;
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// 下一个 Campaign ID
	#[pallet::storage]
	pub type NextCampaignId<T> = StorageValue<_, u64, ValueQuery>;

	/// 广告活动
	#[pallet::storage]
	pub type Campaigns<T: Config> = StorageMap<_, Blake2_128Concat, u64, AdCampaign<T>>;

	/// Campaign 锁定预算 (escrow)
	#[pallet::storage]
	pub type CampaignEscrow<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

	/// 投放收据 (per placement, 每 Era 结算后清空)
	#[pallet::storage]
	pub type DeliveryReceipts<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PlacementId,
		BoundedVec<DeliveryReceipt<T>, T::MaxReceiptsPerPlacement>,
		ValueQuery,
	>;

	/// 每 Era 广告位广告收入
	#[pallet::storage]
	pub type EraAdRevenue<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, BalanceOf<T>, ValueQuery>;

	/// 广告位累计广告总收入
	#[pallet::storage]
	pub type PlacementTotalRevenue<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, BalanceOf<T>, ValueQuery>;

	/// 广告位待提取收入 (claimable)
	#[pallet::storage]
	pub type PlacementClaimable<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, BalanceOf<T>, ValueQuery>;

	/// 广告位本 Era 投放次数 (供外部 pallet 检查)
	#[pallet::storage]
	pub type PlacementEraDeliveries<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32, ValueQuery>;

	/// 私有广告注册次数
	#[pallet::storage]
	pub type PrivateAdCount<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32, ValueQuery>;

	// ---- 双向偏好 ----

	/// 广告主拉黑的广告位列表
	#[pallet::storage]
	pub type AdvertiserBlacklist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<PlacementId, T::MaxAdvertiserBlacklist>,
		ValueQuery,
	>;

	/// 广告主指定(白名单)的广告位列表
	#[pallet::storage]
	pub type AdvertiserWhitelist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<PlacementId, T::MaxAdvertiserWhitelist>,
		ValueQuery,
	>;

	/// 广告位拉黑的广告主列表
	#[pallet::storage]
	pub type PlacementBlacklist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PlacementId,
		BoundedVec<T::AccountId, T::MaxPlacementBlacklist>,
		ValueQuery,
	>;

	/// 广告位指定(白名单)的广告主列表
	#[pallet::storage]
	pub type PlacementWhitelist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PlacementId,
		BoundedVec<T::AccountId, T::MaxPlacementWhitelist>,
		ValueQuery,
	>;

	/// Slash 记录 (placement → 连续 Slash 次数)
	#[pallet::storage]
	pub type SlashCount<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32, ValueQuery>;

	/// 被永久禁止广告的广告位
	#[pallet::storage]
	pub type BannedPlacements<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, bool, ValueQuery>;

	/// 广告位被举报次数
	#[pallet::storage]
	pub type PlacementFlagCount<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32, ValueQuery>;

	/// 广告位举报去重 (placement_id, reporter) → bool
	#[pallet::storage]
	pub type PlacementFlaggedBy<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, PlacementId,
		Blake2_128Concat, T::AccountId,
		bool,
		ValueQuery,
	>;

	/// Campaign 每日已花费 (campaign_id, day_index) → amount
	#[pallet::storage]
	pub type CampaignDailySpent<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, u64,
		Blake2_128Concat, u32,
		BalanceOf<T>,
		ValueQuery,
	>;

	/// 广告主拥有的 Campaign 列表
	#[pallet::storage]
	pub type CampaignsByAdvertiser<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<u64, T::MaxCampaignsPerAdvertiser>,
		ValueQuery,
	>;

	/// 广告位接受的投放类型 bitmask (0 = 接受所有)
	#[pallet::storage]
	pub type PlacementDeliveryTypes<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u8, ValueQuery>;

	/// Campaign 定向广告位列表 (空 = 全网投放)
	#[pallet::storage]
	pub type CampaignTargets<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		u64, // campaign_id
		BoundedVec<PlacementId, T::MaxTargetsPerCampaign>,
	>;

	/// Campaign 级 CPM 倍率 (广告主设置, 基点 100 = 1x)
	#[pallet::storage]
	pub type CampaignMultiplier<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, u32>;

	/// 广告位级 CPM 倍率 (Entity Owner 设置, 基点 100 = 1x)
	#[pallet::storage]
	pub type PlacementMultiplier<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32>;

	/// 广告位是否要求 Campaign 级审核 (false = 开放投放)
	#[pallet::storage]
	pub type PlacementRequiresApproval<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, bool, ValueQuery>;

	/// 广告位对 Campaign 的审核状态 (true = 已批准)
	#[pallet::storage]
	pub type PlacementCampaignApproval<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, PlacementId,
		Blake2_128Concat, u64, // campaign_id
		bool,
		ValueQuery,
	>;

	/// 发现索引: Active + Approved 的 Campaign ID 列表
	#[pallet::storage]
	pub type ActiveApprovedCampaigns<T: Config> = StorageValue<
		_, BoundedVec<u64, ConstU32<10000>>, ValueQuery,
	>;

	/// 发现索引: 按投放类型 → Campaign ID 列表
	#[pallet::storage]
	pub type CampaignsByDeliveryType<T: Config> = StorageMap<
		_, Blake2_128Concat, u8,
		BoundedVec<u64, ConstU32<5000>>, ValueQuery,
	>;

	/// 发现索引: 广告位 → 定向到该广告位的 Campaign ID 列表
	#[pallet::storage]
	pub type CampaignsForPlacement<T: Config> = StorageMap<
		_, Blake2_128Concat, PlacementId,
		BoundedVec<u64, ConstU32<1000>>, ValueQuery,
	>;

	/// 收据确认状态 (campaign_id, placement_id, receipt_index) → ReceiptStatus
	#[pallet::storage]
	pub type ReceiptConfirmation<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, u64>,         // campaign_id
			NMapKey<Blake2_128Concat, PlacementId>,  // placement_id
			NMapKey<Blake2_128Concat, u32>,          // receipt_index
		),
		ReceiptStatus,
	>;

	/// 收据提交区块 (campaign_id, placement_id, receipt_index) → BlockNumber
	#[pallet::storage]
	pub type ReceiptSubmittedAt<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, u64>,
			NMapKey<Blake2_128Concat, PlacementId>,
			NMapKey<Blake2_128Concat, u32>,
		),
		BlockNumberFor<T>,
	>;

	/// 已审核通过 Campaign 的举报次数
	#[pallet::storage]
	pub type CampaignReportCount<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;

	/// 已审核通过 Campaign 的举报去重
	#[pallet::storage]
	pub type CampaignReportedBy<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, u64,
		Blake2_128Concat, T::AccountId,
		bool,
		ValueQuery,
	>;

	// ---- Phase 6: 广告主推荐 ----

	/// 广告主的推荐人 (advertiser → referrer)
	#[pallet::storage]
	pub type AdvertiserReferrer<T: Config> = StorageMap<
		_, Blake2_128Concat, T::AccountId, T::AccountId,
	>;

	/// 广告主注册区块
	#[pallet::storage]
	pub type AdvertiserRegisteredAt<T: Config> = StorageMap<
		_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>,
	>;

	/// 推荐人可提取佣金
	#[pallet::storage]
	pub type ReferrerClaimable<T: Config> = StorageMap<
		_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery,
	>;

	/// 推荐人累计佣金收入 (统计用)
	#[pallet::storage]
	pub type ReferrerTotalEarnings<T: Config> = StorageMap<
		_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery,
	>;

	/// 推荐人名下广告主列表
	#[pallet::storage]
	pub type ReferrerAdvertisers<T: Config> = StorageMap<
		_, Blake2_128Concat, T::AccountId,
		BoundedVec<T::AccountId, T::MaxReferredAdvertisers>, ValueQuery,
	>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// 广告活动已创建
		CampaignCreated {
			campaign_id: u64,
			advertiser: T::AccountId,
			total_budget: BalanceOf<T>,
			bid_per_mille: BalanceOf<T>,
			campaign_type: CampaignType,
			bid_per_click: BalanceOf<T>,
		},
		/// 追加预算
		CampaignFunded {
			campaign_id: u64,
			amount: BalanceOf<T>,
		},
		/// 暂停
		CampaignPaused { campaign_id: u64 },
		/// 取消 (退款)
		CampaignCancelled {
			campaign_id: u64,
			refunded: BalanceOf<T>,
		},
		/// 审核结果
		CampaignReviewed {
			campaign_id: u64,
			approved: bool,
		},
		/// 投放收据已提交
		DeliveryReceiptSubmitted {
			campaign_id: u64,
			placement_id: PlacementId,
			audience_size: u32,
		},
		/// Era 结算完成
		EraAdsSettled {
			placement_id: PlacementId,
			total_cost: BalanceOf<T>,
			placement_share: BalanceOf<T>,
		},
		/// 广告举报
		CampaignFlagged {
			campaign_id: u64,
			reporter: T::AccountId,
		},
		/// 广告收入已提取
		AdRevenueClaimed {
			placement_id: PlacementId,
			amount: BalanceOf<T>,
			claimer: T::AccountId,
		},
		/// 广告位被 Slash
		PlacementSlashed {
			placement_id: PlacementId,
			slashed_amount: BalanceOf<T>,
			slash_count: u32,
		},
		/// 广告位被永久禁止
		PlacementBannedFromAds {
			placement_id: PlacementId,
		},
		/// 广告主拉黑广告位
		AdvertiserBlockedPlacement {
			advertiser: T::AccountId,
			placement_id: PlacementId,
		},
		/// 广告主取消拉黑
		AdvertiserUnblockedPlacement {
			advertiser: T::AccountId,
			placement_id: PlacementId,
		},
		/// 广告主指定广告位
		AdvertiserPreferredPlacement {
			advertiser: T::AccountId,
			placement_id: PlacementId,
		},
		/// 广告主取消指定
		AdvertiserUnpreferredPlacement {
			advertiser: T::AccountId,
			placement_id: PlacementId,
		},
		/// 广告位拉黑广告主
		PlacementBlockedAdvertiser {
			placement_id: PlacementId,
			advertiser: T::AccountId,
		},
		/// 广告位取消拉黑
		PlacementUnblockedAdvertiser {
			placement_id: PlacementId,
			advertiser: T::AccountId,
		},
		/// 广告位指定广告主
		PlacementPreferredAdvertiser {
			placement_id: PlacementId,
			advertiser: T::AccountId,
		},
		/// 广告位取消指定
		PlacementUnpreferredAdvertiser {
			placement_id: PlacementId,
			advertiser: T::AccountId,
		},
		/// 广告位被举报
		PlacementFlagged {
			placement_id: PlacementId,
			reporter: T::AccountId,
			flag_count: u32,
		},
		/// 私有广告已注册
		PrivateAdRegistered {
			placement_id: PlacementId,
			registrar: T::AccountId,
			count: u32,
		},
		/// 广告活动已恢复
		CampaignResumed { campaign_id: u64 },
		/// 广告活动已过期 (剩余预算已退还)
		CampaignMarkedExpired { campaign_id: u64, refunded: BalanceOf<T> },
		/// 广告活动已更新
		CampaignUpdated { campaign_id: u64 },
		/// 广告活动过期时间已延长
		CampaignExpiryExtended { campaign_id: u64, new_expires_at: BlockNumberFor<T> },
		/// Root 强制取消 Campaign
		CampaignForceCancelled { campaign_id: u64, refunded: BalanceOf<T> },
		/// 广告位已解除封禁
		PlacementUnbanned { placement_id: PlacementId },
		/// 广告位 Slash 计数已重置
		PlacementSlashCountReset { placement_id: PlacementId },
		/// 广告位举报已清除
		PlacementFlagsCleared { placement_id: PlacementId },
		/// Campaign 被治理暂停
		CampaignSuspended { campaign_id: u64 },
		/// Campaign 被治理解除暂停
		CampaignUnsuspended { campaign_id: u64 },
		/// 已审核通过的 Campaign 被举报
		CampaignReported { campaign_id: u64, reporter: T::AccountId, report_count: u32 },
		/// Campaign 重新提交审核
		CampaignResubmitted { campaign_id: u64 },
		/// 广告位投放类型已设置
		PlacementDeliveryTypesSet { placement_id: PlacementId, delivery_types: u8 },
		/// 私域广告已注销
		PrivateAdUnregistered { placement_id: PlacementId, count: u32 },
		/// 已终结 Campaign 存储已清理
		CampaignCleaned { campaign_id: u64 },
		/// 结算激励已发放
		SettlementIncentivePaid { settler: T::AccountId, amount: BalanceOf<T> },
		/// Campaign 定向广告位已设置
		CampaignTargetsSet { campaign_id: u64, count: u32 },
		/// Campaign 定向已清除 (恢复全网投放)
		CampaignTargetsCleared { campaign_id: u64 },
		/// Campaign CPM 倍率已设置
		CampaignMultiplierSet { campaign_id: u64, multiplier_bps: u32 },
		/// 广告位 CPM 倍率已设置
		PlacementMultiplierSet { placement_id: PlacementId, multiplier_bps: u32 },
		/// 广告位审核要求已设置
		PlacementApprovalRequirementSet { placement_id: PlacementId, required: bool },
		/// Campaign 已被广告位批准
		CampaignApprovedForPlacement { placement_id: PlacementId, campaign_id: u64 },
		/// Campaign 已被广告位拒绝
		CampaignRejectedForPlacement { placement_id: PlacementId, campaign_id: u64 },
		/// 收据已被广告主确认
		ReceiptConfirmed { campaign_id: u64, placement_id: PlacementId, receipt_index: u32 },
		/// 收据被广告主争议
		ReceiptDisputed { campaign_id: u64, placement_id: PlacementId, receipt_index: u32 },
		/// 收据已自动确认 (确认窗口超时)
		ReceiptAutoConfirmed { campaign_id: u64, placement_id: PlacementId, receipt_index: u32 },
		/// 广告主已注册 (通过推荐)
		AdvertiserRegistered { advertiser: T::AccountId, referrer: T::AccountId },
		/// 种子广告主已注册 (Root)
		SeedAdvertiserRegistered { advertiser: T::AccountId },
		/// 推荐佣金已记账
		ReferralCommissionCredited { referrer: T::AccountId, advertiser: T::AccountId, amount: BalanceOf<T> },
		/// 推荐佣金已提取
		ReferralEarningsClaimed { referrer: T::AccountId, amount: BalanceOf<T> },
		/// 点击收据已提交 (CPC)
		ClickReceiptSubmitted {
			campaign_id: u64,
			placement_id: PlacementId,
			click_count: u32,
			verified_clicks: u32,
		},
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// Campaign 不存在
		CampaignNotFound,
		/// 非广告主本人
		NotCampaignOwner,
		/// Campaign 非 Active 状态
		CampaignNotActive,
		/// 出价低于最低 CPM
		BidTooLow,
		/// 预算为零
		ZeroBudget,
		/// 广告文本为空
		EmptyAdText,
		/// 投放类型无效
		InvalidDeliveryTypes,
		/// 收据已满
		ReceiptsFull,
		/// 黑名单已满
		BlacklistFull,
		/// 白名单已满
		WhitelistFull,
		/// 已在黑名单中
		AlreadyBlacklisted,
		/// 不在黑名单中
		NotBlacklisted,
		/// 已在白名单中
		AlreadyWhitelisted,
		/// 不在白名单中
		NotWhitelisted,
		/// 广告位已被永久禁止
		PlacementBanned,
		/// 无可提取收入
		NothingToClaim,
		/// Campaign 已取消/过期
		CampaignInactive,
		/// 已拒绝的 Campaign 不可重复审核
		AlreadyReviewed,
		/// Campaign 需先通过审核才能投放
		CampaignNotApproved,
		/// 活跃人数低于门槛
		AudienceBelowMinimum,
		/// 投放验证失败
		DeliveryVerificationFailed,
		/// 非广告位管理员
		NotPlacementAdmin,
		/// Campaign 已过期
		CampaignExpired,
		/// 私有广告注册次数必须 > 0
		ZeroPrivateAdCount,
		/// Campaign ID 溢出
		CampaignIdOverflow,
		/// 过期时间无效 (必须晚于当前区块)
		InvalidExpiry,
		/// 已举报过该广告位
		AlreadyFlaggedPlacement,
		/// Campaign 非 Paused 状态
		CampaignNotPaused,
		/// 广告主已拉黑该广告位
		AdvertiserBlacklistedPlacement,
		/// 广告位已拉黑该广告主
		PlacementBlacklistedAdvertiser,
		/// Campaign 尚未过期 (当前区块 <= expires_at)
		CampaignNotExpired,
		/// 每日预算已耗尽
		DailyBudgetExhausted,
		/// Campaign 列表已满
		CampaignListFull,
		/// 广告位未被封禁
		PlacementNotBanned,
		/// Campaign 非 Suspended 状态
		CampaignNotSuspended,
		/// 广告位白名单不匹配
		NotInAdvertiserWhitelist,
		/// 已举报过该 Campaign
		AlreadyReportedCampaign,
		/// Campaign 非 Rejected 状态
		CampaignNotRejected,
		/// 投放类型不匹配
		DeliveryTypeMismatch,
		/// Campaign 非终结状态 (不可清理)
		CampaignNotTerminated,
		/// 提取金额超过可提取余额
		ClaimAmountTooLarge,
		/// 新过期时间必须晚于当前过期时间
		ExpiryNotExtended,
		/// Campaign 定向不包含该广告位
		PlacementNotTargeted,
		/// 定向列表不能为空
		EmptyTargetsList,
		/// CPM 倍率无效 (需 10..=10000)
		InvalidMultiplier,
		/// 广告位要求审核但 Campaign 未被批准
		CampaignNotApprovedForPlacement,
		/// 收据不存在
		ReceiptNotFound,
		/// 收据不在 Pending 状态
		ReceiptNotPending,
		/// 确认窗口尚未过期 (不可自动确认)
		ConfirmationWindowNotExpired,
		/// 调用者已是注册广告主
		AlreadyRegisteredAdvertiser,
		/// 推荐人不是注册广告主
		ReferrerNotAdvertiser,
		/// 不能推荐自己
		SelfReferral,
		/// 推荐人名下广告主数已满
		ReferralListFull,
		/// 调用者不是注册广告主 (需先注册)
		NotRegisteredAdvertiser,
		/// 无可提取推荐佣金
		NoReferralEarnings,
		/// CPC 出价低于最低值
		ClickBidTooLow,
		/// 点击数为零
		ZeroClickCount,
		/// 点击验证失败
		ClickVerificationFailed,
		/// Campaign 类型不匹配 (e.g. 向 CPM campaign 提交点击收据)
		CampaignTypeMismatch,
		/// 经验证点击数不能超过总点击数
		VerifiedExceedsTotal,
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// 创建广告活动, 锁定预算
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(60_000_000, 8_000))]
		pub fn create_campaign(
			origin: OriginFor<T>,
			text: BoundedVec<u8, T::MaxAdTextLength>,
			url: BoundedVec<u8, T::MaxAdUrlLength>,
			bid_per_mille: BalanceOf<T>,
			daily_budget: BalanceOf<T>,
			total_budget: BalanceOf<T>,
			delivery_types: u8,
			expires_at: BlockNumberFor<T>,
			targets: Option<BoundedVec<PlacementId, T::MaxTargetsPerCampaign>>,
			campaign_type: CampaignType,
			bid_per_click: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Phase 6: 必须是注册广告主
			ensure!(AdvertiserRegisteredAt::<T>::contains_key(&who), Error::<T>::NotRegisteredAdvertiser);

			ensure!(!text.is_empty(), Error::<T>::EmptyAdText);
			ensure!(delivery_types > 0 && delivery_types <= 0b111, Error::<T>::InvalidDeliveryTypes);
			ensure!(!total_budget.is_zero(), Error::<T>::ZeroBudget);

			// 根据 campaign_type 验证出价
			match campaign_type {
				CampaignType::Cpc => {
					ensure!(bid_per_click >= T::MinBidPerClick::get(), Error::<T>::ClickBidTooLow);
				},
				_ => {
					ensure!(bid_per_mille >= T::MinBidPerMille::get(), Error::<T>::BidTooLow);
				},
			}

			let now = frame_system::Pallet::<T>::block_number();
			ensure!(expires_at > now, Error::<T>::InvalidExpiry);

			let id = NextCampaignId::<T>::get();
			ensure!(id < u64::MAX, Error::<T>::CampaignIdOverflow);

			T::Currency::reserve(&who, total_budget)?;

			NextCampaignId::<T>::put(id.saturating_add(1));

			let campaign = AdCampaign::<T> {
				advertiser: who.clone(),
				text,
				url,
				bid_per_mille,
				bid_per_click,
				campaign_type,
				daily_budget,
				total_budget,
				spent: Zero::zero(),
				delivery_types,
				status: CampaignStatus::Active,
				review_status: AdReviewStatus::Pending,
				total_deliveries: 0,
				total_clicks: 0,
				created_at: now,
				expires_at,
			};

			Campaigns::<T>::insert(id, campaign);
			CampaignEscrow::<T>::insert(id, total_budget);

			if let Some(ref t) = targets {
				ensure!(!t.is_empty(), Error::<T>::EmptyTargetsList);
				Self::index_update_placement_targets(id, None, Some(t));
				CampaignTargets::<T>::insert(id, t);
			}

			CampaignsByAdvertiser::<T>::try_mutate(&who, |list| {
				list.try_push(id).map_err(|_| Error::<T>::CampaignListFull)
			})?;

			Self::deposit_event(Event::CampaignCreated {
				campaign_id: id,
				advertiser: who,
				total_budget,
				bid_per_mille,
				campaign_type,
				bid_per_click,
			});
			Ok(())
		}

		/// 追加预算
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn fund_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroBudget);

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(
					c.status == CampaignStatus::Active
					|| c.status == CampaignStatus::Paused
					|| c.status == CampaignStatus::Exhausted,
					Error::<T>::CampaignInactive
				);

				// H1: 阻止对已过期 Campaign 追加预算 — 资金无法用于投放
				let now = frame_system::Pallet::<T>::block_number();
				ensure!(now <= c.expires_at, Error::<T>::CampaignExpired);

				T::Currency::reserve(&who, amount)?;
				c.total_budget = c.total_budget.saturating_add(amount);
				CampaignEscrow::<T>::mutate(campaign_id, |e| *e = e.saturating_add(amount));

				if c.status == CampaignStatus::Exhausted {
					c.status = CampaignStatus::Active;
				}

				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignFunded { campaign_id, amount });
			Ok(())
		}

		/// 暂停广告活动
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn pause_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(c.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
				c.status = CampaignStatus::Paused;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignPaused { campaign_id });
			Ok(())
		}

		/// 取消广告活动, 退还剩余预算
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(50_000_000, 6_000))]
		pub fn cancel_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (refunded, delivery_types) = Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(
					c.status == CampaignStatus::Active || c.status == CampaignStatus::Paused,
					Error::<T>::CampaignInactive
				);

				let remaining = CampaignEscrow::<T>::get(campaign_id);
				if !remaining.is_zero() {
					T::Currency::unreserve(&who, remaining);
				}
				CampaignEscrow::<T>::remove(campaign_id);
				c.status = CampaignStatus::Cancelled;
				Ok::<(BalanceOf<T>, u8), DispatchError>((remaining, c.delivery_types))
			})?;

			Self::index_remove_active(campaign_id, delivery_types);
			Self::deposit_event(Event::CampaignCancelled { campaign_id, refunded });
			Ok(())
		}

		/// 审核广告内容 (Root/DAO)
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn review_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
			approved: bool,
		) -> DispatchResult {
			ensure_root(origin)?;

			let delivery_types = Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				c.review_status = if approved {
					AdReviewStatus::Approved
				} else {
					// 拒绝时自动退款
					if matches!(c.status, CampaignStatus::Active | CampaignStatus::Paused) {
						let remaining = CampaignEscrow::<T>::take(campaign_id);
						if !remaining.is_zero() {
							T::Currency::unreserve(&c.advertiser, remaining);
						}
						c.status = CampaignStatus::Cancelled;
					}
					AdReviewStatus::Rejected
				};
				Ok::<u8, DispatchError>(c.delivery_types)
			})?;

			// 维护发现索引
			if approved {
				Self::index_add_active(campaign_id, delivery_types);
			} else {
				Self::index_remove_active(campaign_id, delivery_types);
			}

			Self::deposit_event(Event::CampaignReviewed { campaign_id, approved });
			Ok(())
		}

		/// 提交投放收据
		///
		/// 核心引擎负责:
		/// 1. Campaign 状态检查 (Active, Approved, 未过期)
		/// 2. 广告位 ban 检查
		/// 3. 委托 DeliveryVerifier 做领域验证 + audience 裁切
		/// 4. 存储收据
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn submit_delivery_receipt(
			origin: OriginFor<T>,
			campaign_id: u64,
			placement_id: PlacementId,
			audience_size: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
			ensure!(campaign.review_status == AdReviewStatus::Approved, Error::<T>::CampaignNotApproved);

			// CPM 收据仅允许 CPM/Fixed/Private 类型 Campaign
			ensure!(campaign.campaign_type != CampaignType::Cpc, Error::<T>::CampaignTypeMismatch);

			let now = frame_system::Pallet::<T>::block_number();
			ensure!(now <= campaign.expires_at, Error::<T>::CampaignExpired);

			ensure!(!BannedPlacements::<T>::get(&placement_id), Error::<T>::PlacementBanned);

			// Campaign 定向检查: 如果 Campaign 指定了目标广告位, 仅目标可投放
			if let Some(targets) = CampaignTargets::<T>::get(campaign_id) {
				ensure!(targets.contains(&placement_id), Error::<T>::PlacementNotTargeted);
			}

			// 广告位级审核: 如果广告位要求审核, 检查 Campaign 是否已被批准
			if PlacementRequiresApproval::<T>::get(&placement_id) {
				ensure!(
					PlacementCampaignApproval::<T>::get(&placement_id, campaign_id),
					Error::<T>::CampaignNotApprovedForPlacement
				);
			}

			// M2-R2: 执行双向黑名单检查
			let advertiser_bl = AdvertiserBlacklist::<T>::get(&campaign.advertiser);
			ensure!(
				!advertiser_bl.contains(&placement_id),
				Error::<T>::AdvertiserBlacklistedPlacement
			);
			let placement_bl = PlacementBlacklist::<T>::get(&placement_id);
			ensure!(
				!placement_bl.contains(&campaign.advertiser),
				Error::<T>::PlacementBlacklistedAdvertiser
			);

			// 白名单执行: 若广告主设置了白名单, 仅白名单内的广告位可投放
			let advertiser_wl = AdvertiserWhitelist::<T>::get(&campaign.advertiser);
			if !advertiser_wl.is_empty() {
				ensure!(
					advertiser_wl.contains(&placement_id),
					Error::<T>::NotInAdvertiserWhitelist
				);
			}

			// 投放类型匹配: 若广告位设置了接受类型, 检查交集
			let placement_dt = PlacementDeliveryTypes::<T>::get(&placement_id);
			if placement_dt > 0 {
				ensure!(
					campaign.delivery_types & placement_dt > 0,
					Error::<T>::DeliveryTypeMismatch
				);
			}

			// 委托适配层验证 + 裁切 audience
			let effective_audience = T::DeliveryVerifier::verify_and_cap_audience(
				&who,
				&placement_id,
				audience_size,
				None, // node_id: 由适配层从 `who` 解析, 或未来由 extrinsic 提供
			).map_err(|e| {
				log::warn!("[ads-core] delivery verification failed: {:?}", e);
				Error::<T>::DeliveryVerificationFailed
			})?;

			ensure!(
				effective_audience >= T::MinAudienceSize::get(),
				Error::<T>::AudienceBelowMinimum
			);

			// 解析 CPM 倍率: Campaign 级 > 广告位级 > 默认 1x (100)
			let cpm_multiplier_bps = CampaignMultiplier::<T>::get(campaign_id)
				.or_else(|| PlacementMultiplier::<T>::get(&placement_id))
				.unwrap_or(100u32);

			let receipt = DeliveryReceipt::<T> {
				campaign_id,
				placement_id,
				audience_size: effective_audience,
				click_count: 0,
				verified_clicks: 0,
				cpm_multiplier_bps,
				delivered_at: now,
				settled: false,
				submitter: who,
			};

			let receipt_index = DeliveryReceipts::<T>::get(&placement_id).len() as u32;
			DeliveryReceipts::<T>::try_mutate(&placement_id, |receipts| {
				receipts.try_push(receipt).map_err(|_| Error::<T>::ReceiptsFull)
			})?;

			// Phase 5: 记录收据确认状态为 Pending
			ReceiptConfirmation::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				ReceiptStatus::Pending,
			);
			ReceiptSubmittedAt::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				now,
			);

			PlacementEraDeliveries::<T>::mutate(&placement_id, |c| *c = c.saturating_add(1));

			Campaigns::<T>::mutate(campaign_id, |maybe| {
				if let Some(c) = maybe {
					c.total_deliveries = c.total_deliveries.saturating_add(1);
				}
			});

			Self::deposit_event(Event::DeliveryReceiptSubmitted {
				campaign_id,
				placement_id,
				audience_size: effective_audience,
			});
			Ok(())
		}

		/// 提交点击收据 (CPC Campaign)
		///
		/// C2b Proxy Account 方案:
		/// 1. Entity 聚合用户点击事件
		/// 2. verified_clicks = 经 proxy 签名验证的点击数
		/// 3. 委托 ClickVerifier 做领域验证 + 每日上限裁切
		/// 4. 存储收据 (与 CPM 收据共用 DeliveryReceipts 存储)
		#[pallet::call_index(49)]
		#[pallet::weight(Weight::from_parts(60_000_000, 10_000))]
		pub fn submit_click_receipt(
			origin: OriginFor<T>,
			campaign_id: u64,
			placement_id: PlacementId,
			click_count: u32,
			verified_clicks: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(click_count > 0, Error::<T>::ZeroClickCount);
			ensure!(verified_clicks <= click_count, Error::<T>::VerifiedExceedsTotal);

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
			ensure!(campaign.review_status == AdReviewStatus::Approved, Error::<T>::CampaignNotApproved);
			ensure!(campaign.campaign_type == CampaignType::Cpc, Error::<T>::CampaignTypeMismatch);

			let now = frame_system::Pallet::<T>::block_number();
			ensure!(now <= campaign.expires_at, Error::<T>::CampaignExpired);

			ensure!(!BannedPlacements::<T>::get(&placement_id), Error::<T>::PlacementBanned);

			// Campaign 定向检查
			if let Some(targets) = CampaignTargets::<T>::get(campaign_id) {
				ensure!(targets.contains(&placement_id), Error::<T>::PlacementNotTargeted);
			}

			// 广告位级审核
			if PlacementRequiresApproval::<T>::get(&placement_id) {
				ensure!(
					PlacementCampaignApproval::<T>::get(&placement_id, campaign_id),
					Error::<T>::CampaignNotApprovedForPlacement
				);
			}

			// 双向黑名单检查
			let advertiser_bl = AdvertiserBlacklist::<T>::get(&campaign.advertiser);
			ensure!(
				!advertiser_bl.contains(&placement_id),
				Error::<T>::AdvertiserBlacklistedPlacement
			);
			let placement_bl = PlacementBlacklist::<T>::get(&placement_id);
			ensure!(
				!placement_bl.contains(&campaign.advertiser),
				Error::<T>::PlacementBlacklistedAdvertiser
			);

			// 白名单执行
			let advertiser_wl = AdvertiserWhitelist::<T>::get(&campaign.advertiser);
			if !advertiser_wl.is_empty() {
				ensure!(
					advertiser_wl.contains(&placement_id),
					Error::<T>::NotInAdvertiserWhitelist
				);
			}

			// 投放类型匹配
			let placement_dt = PlacementDeliveryTypes::<T>::get(&placement_id);
			if placement_dt > 0 {
				ensure!(
					campaign.delivery_types & placement_dt > 0,
					Error::<T>::DeliveryTypeMismatch
				);
			}

			// 委托适配层验证 + 裁切 click_count
			let effective_clicks = T::ClickVerifier::verify_and_cap_clicks(
				&who,
				&placement_id,
				click_count,
				verified_clicks,
			)?;

			// CPM 倍率 (CPC 模式下也可能有加价/折扣)
			let cpm_multiplier_bps = CampaignMultiplier::<T>::get(campaign_id)
				.or_else(|| PlacementMultiplier::<T>::get(&placement_id))
				.unwrap_or(100u32);

			// verified_clicks 按同比例裁切
			let effective_verified = if click_count > 0 && effective_clicks < click_count {
				(verified_clicks as u64)
					.saturating_mul(effective_clicks as u64)
					/ (click_count as u64)
			} else {
				verified_clicks as u64
			} as u32;

			let receipt = DeliveryReceipt::<T> {
				campaign_id,
				placement_id,
				audience_size: 0,
				click_count: effective_clicks,
				verified_clicks: effective_verified,
				cpm_multiplier_bps,
				delivered_at: now,
				settled: false,
				submitter: who,
			};

			let receipt_index = DeliveryReceipts::<T>::get(&placement_id).len() as u32;
			DeliveryReceipts::<T>::try_mutate(&placement_id, |receipts| {
				receipts.try_push(receipt).map_err(|_| Error::<T>::ReceiptsFull)
			})?;

			ReceiptConfirmation::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				ReceiptStatus::Pending,
			);
			ReceiptSubmittedAt::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				now,
			);

			PlacementEraDeliveries::<T>::mutate(&placement_id, |c| *c = c.saturating_add(1));

			Campaigns::<T>::mutate(campaign_id, |maybe| {
				if let Some(c) = maybe {
					c.total_deliveries = c.total_deliveries.saturating_add(1);
					c.total_clicks = c.total_clicks.saturating_add(effective_clicks as u64);
				}
			});

			Self::deposit_event(Event::ClickReceiptSubmitted {
				campaign_id,
				placement_id,
				click_count: effective_clicks,
				verified_clicks: effective_verified,
			});
			Ok(())
		}

		/// 结算某广告位的 Era 广告 (任何人可触发)
		///
		/// 核心引擎负责:
		/// 1. 遍历收据计算 CPM 费用
		/// 2. unreserve + 转入国库
		/// 3. 委托 RevenueDistributor 分配收入
		/// 4. 记录可提取份额
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::from_parts(100_000_000, 15_000))]
		pub fn settle_era_ads(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let settler = ensure_signed(origin)?;

			let treasury = T::TreasuryAccount::get();
			let mut total_cost = BalanceOf::<T>::zero();
			let mut placement_share_total = BalanceOf::<T>::zero();

			// 收集待结算收据数据
			let receipts_snapshot = DeliveryReceipts::<T>::get(&placement_id);
			let mut settle_items: sp_runtime::Vec<(u64, BalanceOf<T>, T::AccountId)> = sp_runtime::Vec::new();

			for (idx, receipt) in receipts_snapshot.iter().enumerate() {
				if receipt.settled {
					continue;
				}
				// Phase 5: 仅结算 Confirmed / AutoConfirmed 的收据
				let confirmation = ReceiptConfirmation::<T>::get((
					receipt.campaign_id, receipt.placement_id, idx as u32,
				));
				match confirmation {
					Some(ReceiptStatus::Confirmed) | Some(ReceiptStatus::AutoConfirmed) => {},
					_ => continue, // Pending / Disputed / None → 跳过
				}
				// M3: 对无法结算的收据记录警告，避免静默丢弃
				if let Some(campaign) = Campaigns::<T>::get(receipt.campaign_id) {
					// 根据 campaign_type 计算费用
					let cost = match campaign.campaign_type {
						CampaignType::Cpc => Self::compute_cpc_cost(
							campaign.bid_per_click,
							receipt.click_count,
							receipt.cpm_multiplier_bps,
						),
						_ => Self::compute_cpm_cost(
							campaign.bid_per_mille,
							receipt.audience_size,
							receipt.cpm_multiplier_bps,
						),
					};
					let escrow = CampaignEscrow::<T>::get(receipt.campaign_id);
					// daily_budget 执行: 计算当日剩余预算
					let daily_remaining = if !campaign.daily_budget.is_zero() {
						let now = frame_system::Pallet::<T>::block_number();
						let day_index: u32 = Self::block_to_day_index(now, campaign.created_at);
						let day_spent = CampaignDailySpent::<T>::get(receipt.campaign_id, day_index);
						campaign.daily_budget.saturating_sub(day_spent)
					} else {
						escrow // 无每日限制
					};
					let actual_cost = core::cmp::min(core::cmp::min(cost, escrow), daily_remaining);
					if !actual_cost.is_zero() {
						settle_items.push((
							receipt.campaign_id,
							actual_cost,
							campaign.advertiser.clone(),
						));
					} else {
						log::warn!(
							"[ads-core] settle: campaign {} escrow exhausted, receipt skipped",
							receipt.campaign_id,
						);
					}
				} else {
					log::warn!(
						"[ads-core] settle: campaign {} not found, receipt skipped",
						receipt.campaign_id,
					);
				}
			}

			// 执行转账
			for (campaign_id, _snapshot_cost, advertiser) in settle_items.iter() {
				let current_escrow = CampaignEscrow::<T>::get(campaign_id);
				let adjusted_cost = core::cmp::min(*_snapshot_cost, current_escrow);
				if adjusted_cost.is_zero() {
					continue;
				}

				// C1 审计修复: 检查 unreserve 返回值，仅转移实际解锁的金额
				let deficit = T::Currency::unreserve(advertiser, adjusted_cost);
				let actually_unreserved = adjusted_cost.saturating_sub(deficit);
				if actually_unreserved.is_zero() {
					log::warn!(
						"[ads-core] settle: campaign {} unreserve returned zero, skipping",
						campaign_id,
					);
					continue;
				}

				// H1 审计修复: transfer 失败 skip 而非 abort，避免单个 campaign 阻塞整体结算
				if T::Currency::transfer(
					advertiser,
					&treasury,
					actually_unreserved,
					ExistenceRequirement::AllowDeath,
				).is_err() {
					log::warn!(
						"[ads-core] settle: campaign {} transfer failed, skipping",
						campaign_id,
					);
					// 重新 reserve 已解锁的金额，避免资金泄露
					let _ = T::Currency::reserve(advertiser, actually_unreserved);
					continue;
				}
				let adjusted_cost = actually_unreserved;

				// 委托适配层分配收入 (从国库分配给各方, 使用实际转入金额)
				let breakdown = T::RevenueDistributor::distribute(
					&placement_id,
					adjusted_cost,
					advertiser,
				).unwrap_or(RevenueBreakdown {
					placement_share: Zero::zero(),
					node_share: Zero::zero(),
					platform_share: adjusted_cost,
				});

				// 广告位可提取份额记账
				PlacementClaimable::<T>::mutate(&placement_id, |c| {
					*c = c.saturating_add(breakdown.placement_share);
				});

				// Phase 6: 推荐佣金 — 从平台份额中抽取
				let referral_rate = T::AdvertiserReferralRate::get();
				if referral_rate > 0 {
					if let Some(ref referrer) = AdvertiserReferrer::<T>::get(advertiser) {
						let bps_divisor: BalanceOf<T> = 10_000u32.into();
						let rate_bal: BalanceOf<T> = referral_rate.into();
						let referral_commission = breakdown.platform_share
							.saturating_mul(rate_bal) / bps_divisor;
						if !referral_commission.is_zero() {
							ReferrerClaimable::<T>::mutate(referrer, |c| {
								*c = c.saturating_add(referral_commission);
							});
							ReferrerTotalEarnings::<T>::mutate(referrer, |e| {
								*e = e.saturating_add(referral_commission);
							});
							Self::deposit_event(Event::ReferralCommissionCredited {
								referrer: referrer.clone(),
								advertiser: advertiser.clone(),
								amount: referral_commission,
							});
						}
					}
				}

				// 更新 escrow
				CampaignEscrow::<T>::mutate(campaign_id, |e| {
					*e = e.saturating_sub(adjusted_cost);
				});

				// 更新 campaign spent + daily spent
				Campaigns::<T>::mutate(campaign_id, |maybe| {
					if let Some(c) = maybe {
						c.spent = c.spent.saturating_add(adjusted_cost);
						if CampaignEscrow::<T>::get(*campaign_id).is_zero() {
							c.status = CampaignStatus::Exhausted;
						}
						// daily_budget 记账
						if !c.daily_budget.is_zero() {
							let now = frame_system::Pallet::<T>::block_number();
							let day_index = Self::block_to_day_index(now, c.created_at);
							CampaignDailySpent::<T>::mutate(campaign_id, day_index, |d| {
								*d = d.saturating_add(adjusted_cost);
							});
						}
					}
				});

				total_cost = total_cost.saturating_add(adjusted_cost);
				placement_share_total = placement_share_total.saturating_add(breakdown.placement_share);
			}

			// 清空本 Era 收据
			DeliveryReceipts::<T>::remove(&placement_id);

			// 更新收入统计
			if !total_cost.is_zero() {
				EraAdRevenue::<T>::insert(&placement_id, total_cost);
				PlacementTotalRevenue::<T>::mutate(&placement_id, |r| *r = r.saturating_add(total_cost));

				// 结算激励: 从国库分配小比例给触发者
				let incentive_bps = T::SettlementIncentiveBps::get();
				if incentive_bps > 0 {
					let bps_divisor: BalanceOf<T> = 10_000u32.into();
					let incentive_bps_bal: BalanceOf<T> = incentive_bps.into();
					let incentive = total_cost.saturating_mul(incentive_bps_bal) / bps_divisor;
					if !incentive.is_zero() {
						let _ = T::Currency::transfer(
							&treasury,
							&settler,
							incentive,
							ExistenceRequirement::AllowDeath,
						);
						Self::deposit_event(Event::SettlementIncentivePaid {
							settler: settler.clone(),
							amount: incentive,
						});
					}
				}

				Self::deposit_event(Event::EraAdsSettled {
					placement_id,
					total_cost,
					placement_share: placement_share_total,
				});
			}

			Ok(())
		}

		/// 举报广告
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn flag_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
				// H2 审计修复: 仅允许 flag Pending 的 Campaign，
				// 已 Approved 的 Campaign 不能被单个用户 griefing 停止投放
				ensure!(
					c.review_status == AdReviewStatus::Pending,
					Error::<T>::AlreadyReviewed
				);
				c.review_status = AdReviewStatus::Flagged;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignFlagged { campaign_id, reporter: who });
			Ok(())
		}

		/// 广告位管理员提取广告收入 (支持部分提取, amount=0 表示全部提取)
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_ad_revenue(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			let claimable = PlacementClaimable::<T>::get(&placement_id);
			ensure!(!claimable.is_zero(), Error::<T>::NothingToClaim);

			let claim_amount = if amount.is_zero() { claimable } else { amount };
			ensure!(claim_amount <= claimable, Error::<T>::ClaimAmountTooLarge);

			let treasury = T::TreasuryAccount::get();
			T::Currency::transfer(
				&treasury,
				&who,
				claim_amount,
				ExistenceRequirement::AllowDeath,
			)?;

			let remaining = claimable.saturating_sub(claim_amount);
			if remaining.is_zero() {
				PlacementClaimable::<T>::remove(&placement_id);
			} else {
				PlacementClaimable::<T>::insert(&placement_id, remaining);
			}

			Self::deposit_event(Event::AdRevenueClaimed {
				placement_id,
				amount: claim_amount,
				claimer: who,
			});
			Ok(())
		}

		// ---- 双向偏好 ----

		/// 广告主拉黑广告位
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_block_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserBlacklist::<T>::try_mutate(&who, |list| {
				ensure!(!list.contains(&placement_id), Error::<T>::AlreadyBlacklisted);
				list.try_push(placement_id).map_err(|_| Error::<T>::BlacklistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserBlockedPlacement {
				advertiser: who,
				placement_id,
			});
			Ok(())
		}

		/// 广告主取消拉黑广告位
		#[pallet::call_index(10)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_unblock_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserBlacklist::<T>::try_mutate(&who, |list| {
				let pos = list.iter().position(|h| h == &placement_id)
					.ok_or(Error::<T>::NotBlacklisted)?;
				list.swap_remove(pos);
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserUnblockedPlacement {
				advertiser: who,
				placement_id,
			});
			Ok(())
		}

		/// 广告主指定广告位 (白名单)
		#[pallet::call_index(11)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_prefer_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserWhitelist::<T>::try_mutate(&who, |list| {
				ensure!(!list.contains(&placement_id), Error::<T>::AlreadyWhitelisted);
				list.try_push(placement_id).map_err(|_| Error::<T>::WhitelistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserPreferredPlacement {
				advertiser: who,
				placement_id,
			});
			Ok(())
		}

		/// 广告主取消指定广告位 (移除白名单)
		#[pallet::call_index(12)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_unprefer_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserWhitelist::<T>::try_mutate(&who, |list| {
				let pos = list.iter().position(|h| h == &placement_id)
					.ok_or(Error::<T>::NotWhitelisted)?;
				list.swap_remove(pos);
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserUnpreferredPlacement {
				advertiser: who,
				placement_id,
			});
			Ok(())
		}

		/// 广告位拉黑广告主 (仅管理员)
		#[pallet::call_index(13)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn placement_block_advertiser(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			PlacementBlacklist::<T>::try_mutate(&placement_id, |list| {
				ensure!(!list.contains(&advertiser), Error::<T>::AlreadyBlacklisted);
				list.try_push(advertiser.clone()).map_err(|_| Error::<T>::BlacklistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::PlacementBlockedAdvertiser {
				placement_id,
				advertiser,
			});
			Ok(())
		}

		/// 广告位取消拉黑广告主 (仅管理员)
		#[pallet::call_index(14)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn placement_unblock_advertiser(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			PlacementBlacklist::<T>::try_mutate(&placement_id, |list| {
				let pos = list.iter().position(|a| a == &advertiser)
					.ok_or(Error::<T>::NotBlacklisted)?;
				list.swap_remove(pos);
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::PlacementUnblockedAdvertiser {
				placement_id,
				advertiser,
			});
			Ok(())
		}

		/// 广告位指定广告主 (白名单, 仅管理员)
		#[pallet::call_index(15)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn placement_prefer_advertiser(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			PlacementWhitelist::<T>::try_mutate(&placement_id, |list| {
				ensure!(!list.contains(&advertiser), Error::<T>::AlreadyWhitelisted);
				list.try_push(advertiser.clone()).map_err(|_| Error::<T>::WhitelistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::PlacementPreferredAdvertiser {
				placement_id,
				advertiser,
			});
			Ok(())
		}

		/// 广告位取消指定广告主 (移除白名单, 仅管理员)
		#[pallet::call_index(16)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn placement_unprefer_advertiser(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			PlacementWhitelist::<T>::try_mutate(&placement_id, |list| {
				let pos = list.iter().position(|a| a == &advertiser)
					.ok_or(Error::<T>::NotWhitelisted)?;
				list.swap_remove(pos);
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::PlacementUnpreferredAdvertiser {
				placement_id,
				advertiser,
			});
			Ok(())
		}

		/// 举报广告位 (任何人)
		#[pallet::call_index(17)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn flag_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// M1: 同一用户不可重复举报同一广告位
			ensure!(
				!PlacementFlaggedBy::<T>::get(&placement_id, &who),
				Error::<T>::AlreadyFlaggedPlacement
			);

			PlacementFlaggedBy::<T>::insert(&placement_id, &who, true);
			let count = PlacementFlagCount::<T>::get(&placement_id).saturating_add(1);
			PlacementFlagCount::<T>::insert(&placement_id, count);

			Self::deposit_event(Event::PlacementFlagged {
				placement_id,
				reporter: who,
				flag_count: count,
			});
			Ok(())
		}

		/// Slash 广告位 (Root/DAO) — 扣除 PlacementClaimable 的 AdSlashPercentage%
		#[pallet::call_index(18)]
		#[pallet::weight(Weight::from_parts(60_000_000, 8_000))]
		pub fn slash_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			_reporter: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;

			// 实际 Slash: 扣除 PlacementClaimable 的百分比
			let claimable = PlacementClaimable::<T>::get(&placement_id);
			let slash_pct: BalanceOf<T> = T::AdSlashPercentage::get().into();
			let hundred: BalanceOf<T> = 100u32.into();
			let slashed_amount = claimable.saturating_mul(slash_pct) / hundred;
			if !slashed_amount.is_zero() {
				PlacementClaimable::<T>::mutate(&placement_id, |c| {
					*c = c.saturating_sub(slashed_amount);
				});
			}

			// 递增 Slash 次数
			let count = SlashCount::<T>::get(&placement_id).saturating_add(1);
			SlashCount::<T>::insert(&placement_id, count);

			Self::deposit_event(Event::PlacementSlashed {
				placement_id,
				slashed_amount,
				slash_count: count,
			});

			// 连续 3 次 → 永久禁止
			if count >= 3 {
				BannedPlacements::<T>::insert(&placement_id, true);
				Self::deposit_event(Event::PlacementBannedFromAds { placement_id });
			}

			Ok(())
		}

		/// M1-R2: 恢复已暂停的广告活动
		#[pallet::call_index(20)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn resume_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(c.status == CampaignStatus::Paused, Error::<T>::CampaignNotPaused);

				// 阻止恢复已过期的 Campaign
				let now = frame_system::Pallet::<T>::block_number();
				ensure!(now <= c.expires_at, Error::<T>::CampaignExpired);

				c.status = CampaignStatus::Active;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignResumed { campaign_id });
			Ok(())
		}

		/// L-CORE4: 标记已过期的广告活动, 退还剩余预算 (任何人可调用)
		#[pallet::call_index(21)]
		#[pallet::weight(Weight::from_parts(50_000_000, 6_000))]
		pub fn expire_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let mut campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;

			// 仅 Active / Paused / Exhausted / Suspended 状态可过期
			ensure!(
				matches!(campaign.status, CampaignStatus::Active | CampaignStatus::Paused | CampaignStatus::Exhausted | CampaignStatus::Suspended),
				Error::<T>::CampaignInactive
			);

			// 必须已过期
			let now = frame_system::Pallet::<T>::block_number();
			ensure!(now > campaign.expires_at, Error::<T>::CampaignNotExpired);

			// 退还剩余预算 (使用 CampaignEscrow 作为真实源)
			let remaining = CampaignEscrow::<T>::take(campaign_id);
			let mut refunded: BalanceOf<T> = Zero::zero();
			if !remaining.is_zero() {
				let deficit = T::Currency::unreserve(&campaign.advertiser, remaining);
				refunded = remaining.saturating_sub(deficit);
			}

			let dt = campaign.delivery_types;
			campaign.status = CampaignStatus::Expired;
			Campaigns::<T>::insert(campaign_id, campaign);

			Self::index_remove_active(campaign_id, dt);
			Self::deposit_event(Event::CampaignMarkedExpired { campaign_id, refunded });
			Ok(())
		}

		/// 私域广告自助登记
		#[pallet::call_index(19)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn register_private_ad(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			count: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(count > 0, Error::<T>::ZeroPrivateAdCount);

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			let fee = T::PrivateAdRegistrationFee::get()
				.saturating_mul(count.into());
			if !fee.is_zero() {
				let treasury = T::TreasuryAccount::get();
				T::Currency::transfer(
					&who,
					&treasury,
					fee,
					ExistenceRequirement::AllowDeath,
				)?;
			}

			PrivateAdCount::<T>::mutate(&placement_id, |c| *c = c.saturating_add(count));
			PlacementEraDeliveries::<T>::mutate(&placement_id, |c| *c = c.saturating_add(count));

			Self::deposit_event(Event::PrivateAdRegistered {
				placement_id,
				registrar: who,
				count,
			});
			Ok(())
		}

		// ================================================================
		// 新增 Extrinsics (call_index 22+)
		// ================================================================

		/// 广告主更新 Campaign (Active/Paused 状态, 重置审核为 Pending)
		#[pallet::call_index(22)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn update_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
			text: Option<BoundedVec<u8, T::MaxAdTextLength>>,
			url: Option<BoundedVec<u8, T::MaxAdUrlLength>>,
			bid_per_mille: Option<BalanceOf<T>>,
			daily_budget: Option<BalanceOf<T>>,
			delivery_types: Option<u8>,
			bid_per_click: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(
					c.status == CampaignStatus::Active || c.status == CampaignStatus::Paused,
					Error::<T>::CampaignInactive
				);

				if let Some(t) = text {
					ensure!(!t.is_empty(), Error::<T>::EmptyAdText);
					c.text = t;
				}
				if let Some(u) = url {
					c.url = u;
				}
				// 根据 campaign_type 分派出价验证
				if let Some(b) = bid_per_mille {
					ensure!(c.campaign_type != CampaignType::Cpc, Error::<T>::CampaignTypeMismatch);
					ensure!(b >= T::MinBidPerMille::get(), Error::<T>::BidTooLow);
					c.bid_per_mille = b;
				}
				if let Some(b) = bid_per_click {
					ensure!(c.campaign_type == CampaignType::Cpc, Error::<T>::CampaignTypeMismatch);
					ensure!(b >= T::MinBidPerClick::get(), Error::<T>::ClickBidTooLow);
					c.bid_per_click = b;
				}
				if let Some(d) = daily_budget {
					c.daily_budget = d;
				}
				if let Some(dt) = delivery_types {
					ensure!(dt > 0 && dt <= 0b111, Error::<T>::InvalidDeliveryTypes);
					c.delivery_types = dt;
				}

				c.review_status = AdReviewStatus::Pending;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignUpdated { campaign_id });
			Ok(())
		}

		/// 广告主延长 Campaign 过期时间
		#[pallet::call_index(23)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn extend_campaign_expiry(
			origin: OriginFor<T>,
			campaign_id: u64,
			new_expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(
					matches!(c.status, CampaignStatus::Active | CampaignStatus::Paused),
					Error::<T>::CampaignInactive
				);
				ensure!(new_expires_at > c.expires_at, Error::<T>::ExpiryNotExtended);

				let now = frame_system::Pallet::<T>::block_number();
				ensure!(new_expires_at > now, Error::<T>::InvalidExpiry);

				c.expires_at = new_expires_at;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignExpiryExtended { campaign_id, new_expires_at });
			Ok(())
		}

		/// Root 强制取消 Campaign (退还剩余预算给广告主)
		#[pallet::call_index(24)]
		#[pallet::weight(Weight::from_parts(60_000_000, 8_000))]
		pub fn force_cancel_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			ensure_root(origin)?;

			let mut campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(
				!matches!(campaign.status, CampaignStatus::Cancelled | CampaignStatus::Expired),
				Error::<T>::CampaignInactive
			);

			let remaining = CampaignEscrow::<T>::take(campaign_id);
			let mut refunded: BalanceOf<T> = Zero::zero();
			if !remaining.is_zero() {
				let deficit = T::Currency::unreserve(&campaign.advertiser, remaining);
				refunded = remaining.saturating_sub(deficit);
			}

			let dt = campaign.delivery_types;
			campaign.status = CampaignStatus::Cancelled;
			Campaigns::<T>::insert(campaign_id, campaign);

			Self::index_remove_active(campaign_id, dt);
			Self::deposit_event(Event::CampaignForceCancelled { campaign_id, refunded });
			Ok(())
		}

		/// Root 解除广告位封禁
		#[pallet::call_index(25)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn unban_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(BannedPlacements::<T>::get(&placement_id), Error::<T>::PlacementNotBanned);

			BannedPlacements::<T>::remove(&placement_id);
			SlashCount::<T>::remove(&placement_id);

			Self::deposit_event(Event::PlacementUnbanned { placement_id });
			Ok(())
		}

		/// Root 重置广告位 Slash 计数
		#[pallet::call_index(26)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn reset_slash_count(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			ensure_root(origin)?;
			SlashCount::<T>::remove(&placement_id);
			Self::deposit_event(Event::PlacementSlashCountReset { placement_id });
			Ok(())
		}

		/// Root 清除广告位举报记录
		#[pallet::call_index(27)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn clear_placement_flags(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			ensure_root(origin)?;
			PlacementFlagCount::<T>::remove(&placement_id);
			let _ = PlacementFlaggedBy::<T>::clear_prefix(&placement_id, u32::MAX, None);
			Self::deposit_event(Event::PlacementFlagsCleared { placement_id });
			Ok(())
		}

		/// Root 暂停 Campaign (可恢复)
		#[pallet::call_index(28)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn suspend_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			ensure_root(origin)?;

			let delivery_types = Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
				c.status = CampaignStatus::Suspended;
				Ok::<u8, DispatchError>(c.delivery_types)
			})?;

			Self::index_remove_active(campaign_id, delivery_types);
			Self::deposit_event(Event::CampaignSuspended { campaign_id });
			Ok(())
		}

		/// Root 解除 Campaign 暂停
		#[pallet::call_index(29)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn unsuspend_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			ensure_root(origin)?;

			let delivery_types = Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.status == CampaignStatus::Suspended, Error::<T>::CampaignNotSuspended);
				c.status = CampaignStatus::Active;
				Ok::<u8, DispatchError>(c.delivery_types)
			})?;

			if Campaigns::<T>::get(campaign_id).map_or(false, |c| c.review_status == AdReviewStatus::Approved) {
				Self::index_add_active(campaign_id, delivery_types);
			}
			Self::deposit_event(Event::CampaignUnsuspended { campaign_id });
			Ok(())
		}

		/// 举报已审核通过的 Campaign (独立于 flag_campaign)
		#[pallet::call_index(30)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn report_approved_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.review_status == AdReviewStatus::Approved, Error::<T>::CampaignNotApproved);

			ensure!(
				!CampaignReportedBy::<T>::get(campaign_id, &who),
				Error::<T>::AlreadyReportedCampaign
			);

			CampaignReportedBy::<T>::insert(campaign_id, &who, true);
			let count = CampaignReportCount::<T>::get(campaign_id).saturating_add(1);
			CampaignReportCount::<T>::insert(campaign_id, count);

			Self::deposit_event(Event::CampaignReported {
				campaign_id,
				reporter: who,
				report_count: count,
			});
			Ok(())
		}

		/// 广告主重新提交被拒绝的 Campaign (修改内容 + 重置审核状态)
		#[pallet::call_index(31)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn resubmit_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
			text: BoundedVec<u8, T::MaxAdTextLength>,
			url: BoundedVec<u8, T::MaxAdUrlLength>,
			total_budget: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!text.is_empty(), Error::<T>::EmptyAdText);
			ensure!(!total_budget.is_zero(), Error::<T>::ZeroBudget);

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(c.advertiser == who, Error::<T>::NotCampaignOwner);
				ensure!(c.review_status == AdReviewStatus::Rejected, Error::<T>::CampaignNotRejected);

				// H1-R2: 先验证过期, 再执行 reserve 等副作用
				let now = frame_system::Pallet::<T>::block_number();
				ensure!(c.expires_at > now, Error::<T>::CampaignExpired);

				// 重新 reserve 预算 (rejected 时已退款)
				T::Currency::reserve(&who, total_budget)?;

				c.text = text;
				c.url = url;
				c.total_budget = total_budget;
				c.spent = Zero::zero();
				// M3-R2: 重置累计投放/点击计数
				c.total_deliveries = 0;
				c.total_clicks = 0;
				c.status = CampaignStatus::Active;
				c.review_status = AdReviewStatus::Pending;

				CampaignEscrow::<T>::insert(campaign_id, total_budget);

				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignResubmitted { campaign_id });
			Ok(())
		}

		/// 广告位管理员设置接受的投放类型 (0 = 接受所有)
		#[pallet::call_index(32)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn set_placement_delivery_types(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			delivery_types: u8,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);
			ensure!(delivery_types <= 0b111, Error::<T>::InvalidDeliveryTypes);

			if delivery_types == 0 {
				PlacementDeliveryTypes::<T>::remove(&placement_id);
			} else {
				PlacementDeliveryTypes::<T>::insert(&placement_id, delivery_types);
			}

			Self::deposit_event(Event::PlacementDeliveryTypesSet { placement_id, delivery_types });
			Ok(())
		}

		/// 私域广告注销
		#[pallet::call_index(33)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn unregister_private_ad(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			count: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(count > 0, Error::<T>::ZeroPrivateAdCount);

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			PrivateAdCount::<T>::mutate(&placement_id, |c| *c = c.saturating_sub(count));

			Self::deposit_event(Event::PrivateAdUnregistered { placement_id, count });
			Ok(())
		}

		/// 清理已终结 Campaign 存储 (Cancelled/Expired, 任何人可调用)
		#[pallet::call_index(34)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn cleanup_campaign(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(
				matches!(campaign.status, CampaignStatus::Cancelled | CampaignStatus::Expired),
				Error::<T>::CampaignNotTerminated
			);

			// 清理发现索引
			Self::index_remove_active(campaign_id, campaign.delivery_types);
			if let Some(targets) = CampaignTargets::<T>::get(campaign_id) {
				Self::index_update_placement_targets(campaign_id, Some(&targets), None);
			}

			Campaigns::<T>::remove(campaign_id);
			CampaignEscrow::<T>::remove(campaign_id);
			CampaignTargets::<T>::remove(campaign_id);
			CampaignMultiplier::<T>::remove(campaign_id);

			// 清理 CampaignsByAdvertiser 索引
			CampaignsByAdvertiser::<T>::mutate(&campaign.advertiser, |list| {
				if let Some(pos) = list.iter().position(|id| *id == campaign_id) {
					list.swap_remove(pos);
				}
			});

			// 清理 daily spent
			let _ = CampaignDailySpent::<T>::clear_prefix(campaign_id, u32::MAX, None);

			// 清理 report 记录
			CampaignReportCount::<T>::remove(campaign_id);
			let _ = CampaignReportedBy::<T>::clear_prefix(campaign_id, u32::MAX, None);

			Self::deposit_event(Event::CampaignCleaned { campaign_id });
			Ok(())
		}

		/// Root 强制结算 Era 广告
		#[pallet::call_index(35)]
		#[pallet::weight(Weight::from_parts(100_000_000, 15_000))]
		pub fn force_settle_era_ads(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			ensure_root(origin)?;
			// 复用结算逻辑 — 创建一个合成的 signed origin 效果
			// 直接内联结算核心逻辑
			let treasury = T::TreasuryAccount::get();
			let mut total_cost = BalanceOf::<T>::zero();
			let mut placement_share_total = BalanceOf::<T>::zero();

			let receipts_snapshot = DeliveryReceipts::<T>::get(&placement_id);
			let mut settle_items: sp_runtime::Vec<(u64, BalanceOf<T>, T::AccountId)> = sp_runtime::Vec::new();

			for (idx, receipt) in receipts_snapshot.iter().enumerate() {
				if receipt.settled { continue; }
				// Phase 5: 仅结算 Confirmed / AutoConfirmed 的收据
				let confirmation = ReceiptConfirmation::<T>::get((
					receipt.campaign_id, receipt.placement_id, idx as u32,
				));
				match confirmation {
					Some(ReceiptStatus::Confirmed) | Some(ReceiptStatus::AutoConfirmed) => {},
					_ => continue,
				}
				if let Some(campaign) = Campaigns::<T>::get(receipt.campaign_id) {
					// 根据 campaign_type 计算费用
					let cost = match campaign.campaign_type {
						CampaignType::Cpc => Self::compute_cpc_cost(
							campaign.bid_per_click,
							receipt.click_count,
							receipt.cpm_multiplier_bps,
						),
						_ => Self::compute_cpm_cost(
							campaign.bid_per_mille,
							receipt.audience_size,
							receipt.cpm_multiplier_bps,
						),
					};
					let escrow = CampaignEscrow::<T>::get(receipt.campaign_id);
					let daily_remaining = if !campaign.daily_budget.is_zero() {
						let now = frame_system::Pallet::<T>::block_number();
						let day_index = Self::block_to_day_index(now, campaign.created_at);
						let day_spent = CampaignDailySpent::<T>::get(receipt.campaign_id, day_index);
						campaign.daily_budget.saturating_sub(day_spent)
					} else {
						escrow
					};
					let actual_cost = core::cmp::min(core::cmp::min(cost, escrow), daily_remaining);
					if !actual_cost.is_zero() {
						settle_items.push((receipt.campaign_id, actual_cost, campaign.advertiser.clone()));
					}
				}
			}

			for (campaign_id, snapshot_cost, advertiser) in settle_items.iter() {
				let current_escrow = CampaignEscrow::<T>::get(campaign_id);
				let adjusted_cost = core::cmp::min(*snapshot_cost, current_escrow);
				if adjusted_cost.is_zero() { continue; }

				let deficit = T::Currency::unreserve(advertiser, adjusted_cost);
				let actually_unreserved = adjusted_cost.saturating_sub(deficit);
				if actually_unreserved.is_zero() { continue; }

				if T::Currency::transfer(
					advertiser, &treasury, actually_unreserved, ExistenceRequirement::AllowDeath,
				).is_err() {
					let _ = T::Currency::reserve(advertiser, actually_unreserved);
					continue;
				}
				let adjusted_cost = actually_unreserved;

				let breakdown = T::RevenueDistributor::distribute(
					&placement_id, adjusted_cost, advertiser,
				).unwrap_or(RevenueBreakdown {
					placement_share: Zero::zero(),
					node_share: Zero::zero(),
					platform_share: adjusted_cost,
				});

				PlacementClaimable::<T>::mutate(&placement_id, |c| *c = c.saturating_add(breakdown.placement_share));

				// Phase 6: 推荐佣金 — 从平台份额中抽取
				let referral_rate = T::AdvertiserReferralRate::get();
				if referral_rate > 0 {
					if let Some(ref referrer) = AdvertiserReferrer::<T>::get(advertiser) {
						let bps_divisor: BalanceOf<T> = 10_000u32.into();
						let rate_bal: BalanceOf<T> = referral_rate.into();
						let referral_commission = breakdown.platform_share
							.saturating_mul(rate_bal) / bps_divisor;
						if !referral_commission.is_zero() {
							ReferrerClaimable::<T>::mutate(referrer, |c| {
								*c = c.saturating_add(referral_commission);
							});
							ReferrerTotalEarnings::<T>::mutate(referrer, |e| {
								*e = e.saturating_add(referral_commission);
							});
							Self::deposit_event(Event::ReferralCommissionCredited {
								referrer: referrer.clone(),
								advertiser: advertiser.clone(),
								amount: referral_commission,
							});
						}
					}
				}

				CampaignEscrow::<T>::mutate(campaign_id, |e| *e = e.saturating_sub(adjusted_cost));

				Campaigns::<T>::mutate(campaign_id, |maybe| {
					if let Some(c) = maybe {
						c.spent = c.spent.saturating_add(adjusted_cost);
						if CampaignEscrow::<T>::get(*campaign_id).is_zero() {
							c.status = CampaignStatus::Exhausted;
						}
						if !c.daily_budget.is_zero() {
							let now = frame_system::Pallet::<T>::block_number();
							let day_index = Self::block_to_day_index(now, c.created_at);
							CampaignDailySpent::<T>::mutate(campaign_id, day_index, |d| {
								*d = d.saturating_add(adjusted_cost);
							});
						}
					}
				});

				total_cost = total_cost.saturating_add(adjusted_cost);
				placement_share_total = placement_share_total.saturating_add(breakdown.placement_share);
			}

			DeliveryReceipts::<T>::remove(&placement_id);

			if !total_cost.is_zero() {
				EraAdRevenue::<T>::insert(&placement_id, total_cost);
				PlacementTotalRevenue::<T>::mutate(&placement_id, |r| *r = r.saturating_add(total_cost));
				Self::deposit_event(Event::EraAdsSettled {
					placement_id, total_cost, placement_share: placement_share_total,
				});
			}

			Ok(())
		}

		/// 广告主设置 Campaign 定向广告位列表
		#[pallet::call_index(36)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn set_campaign_targets(
			origin: OriginFor<T>,
			campaign_id: u64,
			targets: BoundedVec<PlacementId, T::MaxTargetsPerCampaign>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!targets.is_empty(), Error::<T>::EmptyTargetsList);

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.advertiser == who, Error::<T>::NotCampaignOwner);
			ensure!(
				matches!(campaign.status, CampaignStatus::Active | CampaignStatus::Paused),
				Error::<T>::CampaignInactive
			);

			let old_targets = CampaignTargets::<T>::get(campaign_id);
			let count = targets.len() as u32;
			Self::index_update_placement_targets(
				campaign_id,
				old_targets.as_ref().map(|v| v.as_slice()),
				Some(&targets),
			);
			CampaignTargets::<T>::insert(campaign_id, targets);

			Self::deposit_event(Event::CampaignTargetsSet { campaign_id, count });
			Ok(())
		}

		/// 广告主清除 Campaign 定向 (恢复全网投放)
		#[pallet::call_index(37)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn clear_campaign_targets(
			origin: OriginFor<T>,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.advertiser == who, Error::<T>::NotCampaignOwner);
			ensure!(
				matches!(campaign.status, CampaignStatus::Active | CampaignStatus::Paused),
				Error::<T>::CampaignInactive
			);

			let old_targets = CampaignTargets::<T>::take(campaign_id);
			Self::index_update_placement_targets(
				campaign_id,
				old_targets.as_ref().map(|v| v.as_slice()),
				None,
			);

			Self::deposit_event(Event::CampaignTargetsCleared { campaign_id });
			Ok(())
		}

		/// 广告主设置 Campaign 级 CPM 倍率 (基点, 100 = 1x; 0 = 清除)
		#[pallet::call_index(38)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn set_campaign_multiplier(
			origin: OriginFor<T>,
			campaign_id: u64,
			multiplier_bps: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.advertiser == who, Error::<T>::NotCampaignOwner);

			if multiplier_bps == 0 {
				CampaignMultiplier::<T>::remove(campaign_id);
			} else {
				ensure!(multiplier_bps >= 10 && multiplier_bps <= 10_000, Error::<T>::InvalidMultiplier);
				CampaignMultiplier::<T>::insert(campaign_id, multiplier_bps);
			}

			Self::deposit_event(Event::CampaignMultiplierSet { campaign_id, multiplier_bps });
			Ok(())
		}

		/// 广告位管理员设置广告位级 CPM 倍率 (基点, 100 = 1x; 0 = 清除)
		#[pallet::call_index(39)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn set_placement_multiplier(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			multiplier_bps: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(who == admin, Error::<T>::NotPlacementAdmin);

			if multiplier_bps == 0 {
				PlacementMultiplier::<T>::remove(&placement_id);
			} else {
				ensure!(multiplier_bps >= 10 && multiplier_bps <= 10_000, Error::<T>::InvalidMultiplier);
				PlacementMultiplier::<T>::insert(&placement_id, multiplier_bps);
			}

			Self::deposit_event(Event::PlacementMultiplierSet { placement_id, multiplier_bps });
			Ok(())
		}

		/// 广告位管理员设置是否要求 Campaign 级审核
		#[pallet::call_index(40)]
		#[pallet::weight(Weight::from_parts(25_000_000, 5_000))]
		pub fn set_placement_approval_required(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			required: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(who == admin, Error::<T>::NotPlacementAdmin);

			PlacementRequiresApproval::<T>::insert(&placement_id, required);

			Self::deposit_event(Event::PlacementApprovalRequirementSet { placement_id, required });
			Ok(())
		}

		/// 广告位管理员批准 Campaign 在该广告位投放
		#[pallet::call_index(41)]
		#[pallet::weight(Weight::from_parts(25_000_000, 5_000))]
		pub fn approve_campaign_for_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(who == admin, Error::<T>::NotPlacementAdmin);

			ensure!(Campaigns::<T>::contains_key(campaign_id), Error::<T>::CampaignNotFound);

			PlacementCampaignApproval::<T>::insert(&placement_id, campaign_id, true);

			Self::deposit_event(Event::CampaignApprovedForPlacement { placement_id, campaign_id });
			Ok(())
		}

		/// 广告位管理员拒绝/撤销 Campaign 在该广告位的投放权限
		#[pallet::call_index(42)]
		#[pallet::weight(Weight::from_parts(25_000_000, 5_000))]
		pub fn reject_campaign_for_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			campaign_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(who == admin, Error::<T>::NotPlacementAdmin);

			PlacementCampaignApproval::<T>::remove(&placement_id, campaign_id);

			Self::deposit_event(Event::CampaignRejectedForPlacement { placement_id, campaign_id });
			Ok(())
		}

		/// 广告主确认收据 (确认窗口内)
		#[pallet::call_index(43)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn confirm_receipt(
			origin: OriginFor<T>,
			campaign_id: u64,
			placement_id: PlacementId,
			receipt_index: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.advertiser == who, Error::<T>::NotCampaignOwner);

			let status = ReceiptConfirmation::<T>::get((campaign_id, placement_id, receipt_index))
				.ok_or(Error::<T>::ReceiptNotFound)?;
			ensure!(status == ReceiptStatus::Pending, Error::<T>::ReceiptNotPending);

			ReceiptConfirmation::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				ReceiptStatus::Confirmed,
			);

			Self::deposit_event(Event::ReceiptConfirmed { campaign_id, placement_id, receipt_index });
			Ok(())
		}

		/// 广告主争议收据 (确认窗口内)
		#[pallet::call_index(44)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn dispute_receipt(
			origin: OriginFor<T>,
			campaign_id: u64,
			placement_id: PlacementId,
			receipt_index: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.advertiser == who, Error::<T>::NotCampaignOwner);

			let status = ReceiptConfirmation::<T>::get((campaign_id, placement_id, receipt_index))
				.ok_or(Error::<T>::ReceiptNotFound)?;
			ensure!(status == ReceiptStatus::Pending, Error::<T>::ReceiptNotPending);

			ReceiptConfirmation::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				ReceiptStatus::Disputed,
			);

			Self::deposit_event(Event::ReceiptDisputed { campaign_id, placement_id, receipt_index });
			Ok(())
		}

		/// 自动确认超时收据 (任何人可调用)
		#[pallet::call_index(45)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn auto_confirm_receipt(
			origin: OriginFor<T>,
			campaign_id: u64,
			placement_id: PlacementId,
			receipt_index: u32,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let status = ReceiptConfirmation::<T>::get((campaign_id, placement_id, receipt_index))
				.ok_or(Error::<T>::ReceiptNotFound)?;
			ensure!(status == ReceiptStatus::Pending, Error::<T>::ReceiptNotPending);

			let submitted_at = ReceiptSubmittedAt::<T>::get((campaign_id, placement_id, receipt_index))
				.ok_or(Error::<T>::ReceiptNotFound)?;
			let now = frame_system::Pallet::<T>::block_number();
			let window = T::ReceiptConfirmationWindow::get();
			ensure!(now > submitted_at.saturating_add(window), Error::<T>::ConfirmationWindowNotExpired);

			ReceiptConfirmation::<T>::insert(
				(campaign_id, placement_id, receipt_index),
				ReceiptStatus::AutoConfirmed,
			);

			Self::deposit_event(Event::ReceiptAutoConfirmed { campaign_id, placement_id, receipt_index });
			Ok(())
		}

		// ================================================================
		// Phase 6: 广告主推荐
		// ================================================================

		/// 注册成为广告主 (必须有推荐人, 推荐人必须是已注册广告主)
		#[pallet::call_index(46)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn register_advertiser(
			origin: OriginFor<T>,
			referrer: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!AdvertiserRegisteredAt::<T>::contains_key(&who), Error::<T>::AlreadyRegisteredAdvertiser);
			ensure!(who != referrer, Error::<T>::SelfReferral);
			ensure!(AdvertiserRegisteredAt::<T>::contains_key(&referrer), Error::<T>::ReferrerNotAdvertiser);

			ReferrerAdvertisers::<T>::try_mutate(&referrer, |list| {
				list.try_push(who.clone()).map_err(|_| Error::<T>::ReferralListFull)
			})?;

			let now = frame_system::Pallet::<T>::block_number();
			AdvertiserRegisteredAt::<T>::insert(&who, now);
			AdvertiserReferrer::<T>::insert(&who, referrer.clone());

			Self::deposit_event(Event::AdvertiserRegistered { advertiser: who, referrer });
			Ok(())
		}

		/// Root 注册种子广告主 (无需推荐人, 冷启动用)
		#[pallet::call_index(47)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn force_register_advertiser(
			origin: OriginFor<T>,
			advertiser: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(!AdvertiserRegisteredAt::<T>::contains_key(&advertiser), Error::<T>::AlreadyRegisteredAdvertiser);

			let now = frame_system::Pallet::<T>::block_number();
			AdvertiserRegisteredAt::<T>::insert(&advertiser, now);

			Self::deposit_event(Event::SeedAdvertiserRegistered { advertiser });
			Ok(())
		}

		/// 推荐人提取累计推荐佣金
		#[pallet::call_index(48)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_referral_earnings(
			origin: OriginFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let claimable = ReferrerClaimable::<T>::get(&who);
			ensure!(!claimable.is_zero(), Error::<T>::NoReferralEarnings);

			let treasury = T::TreasuryAccount::get();
			T::Currency::transfer(
				&treasury,
				&who,
				claimable,
				ExistenceRequirement::AllowDeath,
			)?;

			ReferrerClaimable::<T>::remove(&who);

			Self::deposit_event(Event::ReferralEarningsClaimed { referrer: who, amount: claimable });
			Ok(())
		}
	}

	// ========================================================================
	// Helper Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 计算 CPM 费用: bid_per_mille * audience / 1000 * multiplier / 100
		pub fn compute_cpm_cost(bid_per_mille: BalanceOf<T>, audience: u32, multiplier_bps: u32) -> BalanceOf<T> {
			let audience_balance: BalanceOf<T> = audience.into();
			let multiplier: BalanceOf<T> = multiplier_bps.into();
			let divisor: BalanceOf<T> = 100_000u32.into(); // 1000 * 100
			bid_per_mille.saturating_mul(audience_balance)
				.saturating_mul(multiplier) / divisor
		}

		/// 计算 CPC 费用: bid_per_click * click_count * multiplier / 100
		pub fn compute_cpc_cost(bid_per_click: BalanceOf<T>, click_count: u32, multiplier_bps: u32) -> BalanceOf<T> {
			let clicks: BalanceOf<T> = click_count.into();
			let multiplier: BalanceOf<T> = multiplier_bps.into();
			let divisor: BalanceOf<T> = 100u32.into();
			bid_per_click.saturating_mul(clicks)
				.saturating_mul(multiplier) / divisor
		}

		/// 将区块号转换为相对于 Campaign 创建时间的天索引
		/// 假设每天 14400 个区块 (6 秒/区块)
		pub fn block_to_day_index(now: BlockNumberFor<T>, created_at: BlockNumberFor<T>) -> u32 {
			let blocks_per_day: BlockNumberFor<T> = 14400u32.into();
			let elapsed = now.saturating_sub(created_at);
			let day: BlockNumberFor<T> = elapsed / blocks_per_day;
			day.try_into().unwrap_or(u32::MAX)
		}

		/// 将 Campaign 加入发现索引 (ActiveApprovedCampaigns + CampaignsByDeliveryType)
		fn index_add_active(campaign_id: u64, delivery_types: u8) {
			ActiveApprovedCampaigns::<T>::mutate(|list| {
				if !list.contains(&campaign_id) {
					let _ = list.try_push(campaign_id);
				}
			});
			CampaignsByDeliveryType::<T>::mutate(delivery_types, |list| {
				if !list.contains(&campaign_id) {
					let _ = list.try_push(campaign_id);
				}
			});
		}

		/// 将 Campaign 从发现索引中移除
		fn index_remove_active(campaign_id: u64, delivery_types: u8) {
			ActiveApprovedCampaigns::<T>::mutate(|list| {
				list.retain(|id| *id != campaign_id);
			});
			CampaignsByDeliveryType::<T>::mutate(delivery_types, |list| {
				list.retain(|id| *id != campaign_id);
			});
		}

		/// Runtime API: 查询指定广告位可投放的 Campaign 列表
		pub fn available_campaigns_for_placement(
			placement_id: &PlacementId,
			max_results: u32,
		) -> alloc::vec::Vec<runtime_api::CampaignSummary<T::AccountId, BalanceOf<T>>> {
			let now = frame_system::Pallet::<T>::block_number();
			let mut results: alloc::vec::Vec<runtime_api::CampaignSummary<T::AccountId, BalanceOf<T>>> = alloc::vec::Vec::new();
			let max = max_results.min(200) as usize;

			// 如果广告位有定向索引, 优先使用; 否则扫描全部 Active+Approved
			let candidate_ids = {
				let targeted = CampaignsForPlacement::<T>::get(placement_id);
				if !targeted.is_empty() {
					targeted.into_inner()
				} else {
					ActiveApprovedCampaigns::<T>::get().into_inner()
				}
			};

			for cid in candidate_ids {
				if let Some(c) = Campaigns::<T>::get(cid) {
					if c.status != CampaignStatus::Active
						|| c.review_status != AdReviewStatus::Approved
						|| now > c.expires_at
					{
						continue;
					}
					// 定向检查
					if let Some(targets) = CampaignTargets::<T>::get(cid) {
						if !targets.contains(placement_id) {
							continue;
						}
					}
					// 审核检查
					if PlacementRequiresApproval::<T>::get(placement_id)
						&& !PlacementCampaignApproval::<T>::get(placement_id, cid)
					{
						continue;
					}
					// ban 检查
					if BannedPlacements::<T>::get(placement_id) {
						continue;
					}

					let multiplier_bps = CampaignMultiplier::<T>::get(cid)
						.or_else(|| PlacementMultiplier::<T>::get(placement_id))
						.unwrap_or(100u32);

					// 计算有效出价: 基础出价 × multiplier / 100
					let base_bid = if c.campaign_type == CampaignType::Cpc {
						c.bid_per_click
					} else {
						c.bid_per_mille
					};
					let effective_bid = base_bid
						.saturating_mul(multiplier_bps.into()) / 100u32.into();

					results.push(runtime_api::CampaignSummary {
						campaign_id: cid,
						advertiser: c.advertiser,
						bid_per_mille: c.bid_per_mille,
						bid_per_click: c.bid_per_click,
						campaign_type: c.campaign_type as u8,
						daily_budget: c.daily_budget,
						total_budget: c.total_budget,
						spent: c.spent,
						delivery_types: c.delivery_types,
						multiplier_bps,
						effective_bid,
					});
				}
			}
			// 按有效出价降序排列 — 高出价广告优先展示
			results.sort_by(|a, b| b.effective_bid.cmp(&a.effective_bid));
			results.truncate(max);
			results
		}

		/// Runtime API: 查询 Campaign 详情
		pub fn campaign_details(
			campaign_id: u64,
		) -> Option<runtime_api::CampaignDetail<T::AccountId, BalanceOf<T>>> {
			let c = Campaigns::<T>::get(campaign_id)?;
			let multiplier_bps = CampaignMultiplier::<T>::get(campaign_id).unwrap_or(100u32);
			let targets = CampaignTargets::<T>::get(campaign_id)
				.map(|t| t.into_inner());

			Some(runtime_api::CampaignDetail {
				campaign_id,
				advertiser: c.advertiser,
				text: c.text.into_inner(),
				url: c.url.into_inner(),
				bid_per_mille: c.bid_per_mille,
				bid_per_click: c.bid_per_click,
				campaign_type: c.campaign_type as u8,
				daily_budget: c.daily_budget,
				total_budget: c.total_budget,
				spent: c.spent,
				delivery_types: c.delivery_types,
				status: c.status as u8,
				review_status: c.review_status as u8,
				// M2-R2: 安全截断, 与 total_clicks 修复一致
				total_deliveries: c.total_deliveries.min(u32::MAX as u64) as u32,
				total_clicks: c.total_clicks.min(u32::MAX as u64) as u32,
				created_at: c.created_at.try_into().unwrap_or(0u64),
				expires_at: c.expires_at.try_into().unwrap_or(0u64),
				multiplier_bps,
				targets,
			})
		}

		/// 更新 CampaignsForPlacement 反向索引 (先清除旧的, 再写入新的)
		fn index_update_placement_targets(
			campaign_id: u64,
			old_targets: Option<&[PlacementId]>,
			new_targets: Option<&[PlacementId]>,
		) {
			// 清除旧索引
			if let Some(old) = old_targets {
				for pid in old {
					CampaignsForPlacement::<T>::mutate(pid, |list| {
						list.retain(|id| *id != campaign_id);
					});
				}
			}
			// 写入新索引
			if let Some(new) = new_targets {
				for pid in new {
					CampaignsForPlacement::<T>::mutate(pid, |list| {
						if !list.contains(&campaign_id) {
							let _ = list.try_push(campaign_id);
						}
					});
				}
			}
		}
	}

	// ========================================================================
	// Hooks — integrity_test
	// ========================================================================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		#[cfg(feature = "std")]
		fn integrity_test() {
			assert!(
				T::MaxReceiptsPerPlacement::get() > 0,
				"MaxReceiptsPerPlacement must be > 0"
			);
			assert!(
				T::AdSlashPercentage::get() <= 100,
				"AdSlashPercentage must be <= 100"
			);
			assert!(
				T::SettlementIncentiveBps::get() <= 10_000,
				"SettlementIncentiveBps must be <= 10000"
			);
			assert!(
				!T::MinBidPerClick::get().is_zero(),
				"MinBidPerClick must be > 0"
			);
			// M1-R2: 防止免费 CPM campaign
			assert!(
				!T::MinBidPerMille::get().is_zero(),
				"MinBidPerMille must be > 0"
			);
			assert!(
				T::AdvertiserReferralRate::get() <= 10_000,
				"AdvertiserReferralRate must be <= 10000 bps"
			);
		}
	}

	// ========================================================================
	// AdScheduleProvider 实现
	// ========================================================================

	impl<T: Config> AdScheduleProvider for Pallet<T> {
		fn is_ads_enabled(placement_id: &PlacementId) -> bool {
			PlacementTotalRevenue::<T>::contains_key(placement_id) ||
			PlacementEraDeliveries::<T>::get(placement_id) > 0
		}

		fn placement_ad_revenue(placement_id: &PlacementId) -> u128 {
			let revenue = PlacementTotalRevenue::<T>::get(placement_id);
			revenue.try_into().unwrap_or(0u128)
		}

		fn placement_era_revenue(placement_id: &PlacementId) -> u128 {
			let revenue = EraAdRevenue::<T>::get(placement_id);
			revenue.try_into().unwrap_or(0u128)
		}
	}

	// ========================================================================
	// AdDeliveryCountProvider 实现
	// ========================================================================

	impl<T: Config> AdDeliveryCountProvider for Pallet<T> {
		fn era_delivery_count(placement_id: &PlacementId) -> u32 {
			PlacementEraDeliveries::<T>::get(placement_id)
		}

		fn reset_era_deliveries(placement_id: &PlacementId) {
			PlacementEraDeliveries::<T>::remove(placement_id);
		}
	}
}
