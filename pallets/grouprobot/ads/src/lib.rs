#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Ads — 群组广告竞价 + CPM 结算 + 质押反作弊 + 双向偏好控制
//!
//! ## 功能
//! - Campaign CRUD (创建/追加预算/暂停/取消/审核/举报)
//! - CPM 竞价 + Vickrey 第二价格拍卖
//! - Bot 投放收据上报 + Era 结算 (60/25/15 或 70/20/10 分成)
//! - 质押锚定 audience_cap + Slash 机制
//! - 双向偏好: 广告主 ⇄ 群组 黑名单/白名单
//! - 群主提取广告收入

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_grouprobot_primitives::*;
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
	/// 每日预算上限
	pub daily_budget: BalanceOf<T>,
	/// 总预算
	pub total_budget: BalanceOf<T>,
	/// 已花费
	pub spent: BalanceOf<T>,
	/// 目标标签
	pub target: AdTargetTag,
	/// 投放类型 bitmask (bit0=ScheduledPost, bit1=ReplyFooter, bit2=WelcomeEmbed)
	pub delivery_types: u8,
	/// 活动状态
	pub status: CampaignStatus,
	/// 审核状态
	pub review_status: AdReviewStatus,
	/// 累计投放次数
	pub total_deliveries: u64,
	/// 创建区块
	pub created_at: BlockNumberFor<T>,
	/// 过期区块
	pub expires_at: BlockNumberFor<T>,
}

/// 社区广告排期 (每 Era 更新)
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct CommunityAdSchedule {
	pub community_id_hash: CommunityIdHash,
	/// 本 Era 排期中的 Campaign ID 列表
	pub scheduled_campaigns: BoundedVec<u64, ConstU32<10>>,
	/// 每日投放上限
	pub daily_limit: u8,
	/// 本 Era 已投放次数
	pub delivered_this_era: u32,
}

/// Bot 投放收据
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct DeliveryReceipt<T: Config> {
	pub campaign_id: u64,
	pub community_id_hash: CommunityIdHash,
	pub delivery_type: AdDeliveryType,
	/// Bot 统计的活跃成员数
	pub audience_size: u32,
	/// 投放节点 ID (仅 TEE 节点可提交)
	pub node_id: NodeId,
	/// 节点签名
	pub node_signature: [u8; 64],
	pub delivered_at: BlockNumberFor<T>,
	/// 是否已结算
	pub settled: bool,
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

		/// 每社区最大收据数
		#[pallet::constant]
		type MaxReceiptsPerCommunity: Get<u32>;

		/// 广告主黑名单上限
		#[pallet::constant]
		type MaxAdvertiserBlacklist: Get<u32>;

		/// 广告主白名单上限
		#[pallet::constant]
		type MaxAdvertiserWhitelist: Get<u32>;

		/// 社区黑名单上限
		#[pallet::constant]
		type MaxCommunityBlacklist: Get<u32>;

		/// 社区白名单上限
		#[pallet::constant]
		type MaxCommunityWhitelist: Get<u32>;

		/// 最低 CPM 出价
		#[pallet::constant]
		type MinBidPerMille: Get<BalanceOf<Self>>;

		/// 接入广告的最低活跃人数
		#[pallet::constant]
		type MinAudienceSize: Get<u32>;

		/// L3 突增阈值百分比 (e.g. 100 = 允许 100% 增长)
		#[pallet::constant]
		type AudienceSurgeThresholdPct: Get<u32>;

		/// L5 多节点偏差阈值百分比 (e.g. 20 = 20%)
		#[pallet::constant]
		type NodeDeviationThresholdPct: Get<u32>;

		/// Slash 百分比 (e.g. 30 = 30%)
		#[pallet::constant]
		type AdSlashPercentage: Get<u32>;

		/// 国库账户
		type TreasuryAccount: Get<Self::AccountId>;

		/// 节点共识查询 (用于验证节点状态 + TEE 状态)
		type NodeConsensus: NodeConsensusProvider<Self::AccountId>;
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

	/// 社区广告排期
	#[pallet::storage]
	pub type CommunitySchedules<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, CommunityAdSchedule>;

	/// 投放收据 (per community, 每 Era 结算后清空)
	#[pallet::storage]
	pub type DeliveryReceipts<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdHash,
		BoundedVec<DeliveryReceipt<T>, T::MaxReceiptsPerCommunity>,
		ValueQuery,
	>;

	/// 每 Era 社区广告收入
	#[pallet::storage]
	pub type EraAdRevenue<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, BalanceOf<T>, ValueQuery>;

	/// 社区累计广告总收入
	#[pallet::storage]
	pub type CommunityTotalRevenue<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, BalanceOf<T>, ValueQuery>;

	/// 社区待提取收入 (claimable)
	#[pallet::storage]
	pub type CommunityClaimable<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, BalanceOf<T>, ValueQuery>;

	/// 社区广告质押
	#[pallet::storage]
	pub type CommunityAdStake<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, BalanceOf<T>, ValueQuery>;

	/// 社区 audience 上限 (由质押决定)
	#[pallet::storage]
	pub type CommunityAudienceCap<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	// ---- 双向偏好 ----

	/// 广告主拉黑的社区列表
	#[pallet::storage]
	pub type AdvertiserBlacklist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<CommunityIdHash, T::MaxAdvertiserBlacklist>,
		ValueQuery,
	>;

	/// 广告主指定(白名单)的社区列表
	#[pallet::storage]
	pub type AdvertiserWhitelist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<CommunityIdHash, T::MaxAdvertiserWhitelist>,
		ValueQuery,
	>;

	/// 社区拉黑的广告主列表
	#[pallet::storage]
	pub type CommunityBlacklist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdHash,
		BoundedVec<T::AccountId, T::MaxCommunityBlacklist>,
		ValueQuery,
	>;

	/// 社区指定(白名单)的广告主列表
	#[pallet::storage]
	pub type CommunityWhitelist<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdHash,
		BoundedVec<T::AccountId, T::MaxCommunityWhitelist>,
		ValueQuery,
	>;

	/// Slash 记录 (community_hash → 连续 Slash 次数)
	#[pallet::storage]
	pub type SlashCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 被永久禁止广告的社区
	#[pallet::storage]
	pub type BannedCommunities<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, bool, ValueQuery>;

	// ---- TEE 节点广告收入 ----

	/// 节点广告待领取奖励
	#[pallet::storage]
	pub type NodeAdPendingRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

	/// 节点广告累计已领取
	#[pallet::storage]
	pub type NodeAdTotalEarned<T: Config> =
		StorageMap<_, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

	/// TEE 节点广告分成百分比 (默认 32, 即 32%)
	#[pallet::storage]
	pub type TeeNodeAdPct<T: Config> = StorageValue<_, u32, ValueQuery>;

	// ---- Phase 5: 反作弊 ----

	/// 上一 Era 社区活跃人数 (用于 L3 突增检测)
	#[pallet::storage]
	pub type PreviousEraAudience<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 社区因 audience 突增被暂停广告 (Era 数, 0=未暂停)
	#[pallet::storage]
	pub type AudienceSurgePaused<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 社区被举报次数 (community-level, 非 campaign)
	#[pallet::storage]
	pub type CommunityFlagCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 多节点 audience 上报 (community_hash → Vec<(node_signature_prefix, audience)>)
	/// 用于 L5 交叉验证
	#[pallet::storage]
	pub type NodeAudienceReports<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdHash,
		BoundedVec<(u32, u32), ConstU32<10>>,  // (node_id_prefix, audience_size)
		ValueQuery,
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
			community_id_hash: CommunityIdHash,
			audience_size: u32,
		},
		/// Era 结算完成
		EraAdsSettled {
			community_id_hash: CommunityIdHash,
			total_cost: BalanceOf<T>,
			community_share: BalanceOf<T>,
		},
		/// 广告举报
		CampaignFlagged {
			campaign_id: u64,
			reporter: T::AccountId,
		},
		/// 广告收入已提取
		AdRevenueClaimed {
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
			claimer: T::AccountId,
		},
		/// 质押
		AdStaked {
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
			audience_cap: u32,
		},
		/// 取消质押
		AdUnstaked {
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		},
		/// 社区被 Slash
		CommunitySlashed {
			community_id_hash: CommunityIdHash,
			slashed_amount: BalanceOf<T>,
			slash_count: u32,
		},
		/// 社区被永久禁止广告
		CommunityBannedFromAds {
			community_id_hash: CommunityIdHash,
		},
		/// 广告主拉黑社区
		AdvertiserBlockedCommunity {
			advertiser: T::AccountId,
			community_id_hash: CommunityIdHash,
		},
		/// 广告主取消拉黑
		AdvertiserUnblockedCommunity {
			advertiser: T::AccountId,
			community_id_hash: CommunityIdHash,
		},
		/// 广告主指定社区
		AdvertiserPreferredCommunity {
			advertiser: T::AccountId,
			community_id_hash: CommunityIdHash,
		},
		/// 社区拉黑广告主
		CommunityBlockedAdvertiser {
			community_id_hash: CommunityIdHash,
			advertiser: T::AccountId,
		},
		/// 社区取消拉黑
		CommunityUnblockedAdvertiser {
			community_id_hash: CommunityIdHash,
			advertiser: T::AccountId,
		},
		/// 社区指定广告主
		CommunityPreferredAdvertiser {
			community_id_hash: CommunityIdHash,
			advertiser: T::AccountId,
		},
		/// 社区被举报 (community-level)
		CommunityFlagged {
			community_id_hash: CommunityIdHash,
			reporter: T::AccountId,
			flag_count: u32,
		},
		/// L3: audience 突增自动暂停
		AudienceSurgePausedEvent {
			community_id_hash: CommunityIdHash,
			previous: u32,
			current: u32,
		},
		/// audience 突增暂停已恢复
		AudienceSurgeResumed {
			community_id_hash: CommunityIdHash,
		},
		/// L5: 多节点偏差过大, 拒结
		NodeDeviationRejected {
			community_id_hash: CommunityIdHash,
			min_audience: u32,
			max_audience: u32,
		},
		/// 节点广告奖励分配
		NodeAdRewardAccrued {
			node_id: NodeId,
			amount: BalanceOf<T>,
		},
		/// 节点广告收入已提取
		NodeAdRevenueClaimed {
			node_id: NodeId,
			amount: BalanceOf<T>,
			claimer: T::AccountId,
		},
		/// TEE 节点广告分成百分比已更新
		TeeAdPercentUpdated {
			tee_pct: u32,
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
		/// 社区已被永久禁止
		CommunityBanned,
		/// 无可提取收入
		NothingToClaim,
		/// Campaign 已取消/过期
		CampaignInactive,
		/// 质押金额为零
		ZeroStakeAmount,
		/// 质押不足 (unstake 超出已质押)
		InsufficientStake,
		/// 已审核通过的 Campaign 不可重复审核
		AlreadyReviewed,
		/// Campaign 需先通过审核才能投放
		CampaignNotApproved,
		/// 活跃人数低于门槛
		AudienceBelowMinimum,
		/// 社区因突增被暂停广告
		CommunityAdsPaused,
		/// L5 多节点偏差过大
		NodeDeviationTooHigh,
		/// 节点 audience 报告已满
		NodeReportsFull,
		/// 节点不存在或未激活
		NodeNotActive,
		/// 节点无待领取广告收入
		NoNodeAdReward,
		/// 非节点运营者
		NotNodeOperator,
		/// 节点非 TEE, 不允许提交广告投放收据
		NodeNotTee,
		/// 无效的百分比值
		InvalidPercentage,
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
			target: AdTargetTag,
			delivery_types: u8,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!text.is_empty(), Error::<T>::EmptyAdText);
			ensure!(delivery_types > 0 && delivery_types <= 0b111, Error::<T>::InvalidDeliveryTypes);
			ensure!(bid_per_mille >= T::MinBidPerMille::get(), Error::<T>::BidTooLow);
			ensure!(!total_budget.is_zero(), Error::<T>::ZeroBudget);

			// 锁定预算
			T::Currency::reserve(&who, total_budget)?;

			let id = NextCampaignId::<T>::get();
			NextCampaignId::<T>::put(id.saturating_add(1));

			let now = frame_system::Pallet::<T>::block_number();

			let campaign = AdCampaign::<T> {
				advertiser: who.clone(),
				text,
				url,
				bid_per_mille,
				daily_budget,
				total_budget,
				spent: Zero::zero(),
				target,
				delivery_types,
				status: CampaignStatus::Active,
				review_status: AdReviewStatus::Pending,
				total_deliveries: 0,
				created_at: now,
				expires_at,
			};

			Campaigns::<T>::insert(id, campaign);
			CampaignEscrow::<T>::insert(id, total_budget);

			Self::deposit_event(Event::CampaignCreated {
				campaign_id: id,
				advertiser: who,
				total_budget,
				bid_per_mille,
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
				ensure!(c.status == CampaignStatus::Active || c.status == CampaignStatus::Paused,
					Error::<T>::CampaignInactive);

				T::Currency::reserve(&who, amount)?;
				c.total_budget = c.total_budget.saturating_add(amount);
				CampaignEscrow::<T>::mutate(campaign_id, |e| *e = e.saturating_add(amount));

				// 如果之前因预算耗尽而暂停，恢复
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

			let refunded = Campaigns::<T>::try_mutate(campaign_id, |maybe| {
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
				Ok::<BalanceOf<T>, DispatchError>(remaining)
			})?;

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

			Campaigns::<T>::try_mutate(campaign_id, |maybe| {
				let c = maybe.as_mut().ok_or(Error::<T>::CampaignNotFound)?;
				ensure!(
					c.review_status == AdReviewStatus::Pending || c.review_status == AdReviewStatus::Flagged,
					Error::<T>::AlreadyReviewed
				);
				c.review_status = if approved {
					AdReviewStatus::Approved
				} else {
					AdReviewStatus::Rejected
				};
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignReviewed { campaign_id, approved });
			Ok(())
		}

		/// Bot 上报投放收据
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn submit_delivery_receipt(
			origin: OriginFor<T>,
			campaign_id: u64,
			community_id_hash: CommunityIdHash,
			delivery_type: AdDeliveryType,
			audience_size: u32,
			node_id: NodeId,
			node_signature: [u8; 64],
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// 验证 Campaign 存在且已审核通过
			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
			ensure!(campaign.review_status == AdReviewStatus::Approved, Error::<T>::CampaignNotApproved);

			// 验证节点存在且活跃
			ensure!(T::NodeConsensus::is_node_active(&node_id), Error::<T>::NodeNotActive);

			// 强制要求 TEE 节点: 非 TEE 节点不应持有 BOT_TOKEN, 无法投放广告
			ensure!(T::NodeConsensus::is_tee_node_by_operator(&_who), Error::<T>::NodeNotTee);

			// 社区未被禁止
			ensure!(!BannedCommunities::<T>::get(&community_id_hash), Error::<T>::CommunityBanned);

			// P5 L3: 社区未因突增被暂停
			ensure!(
				AudienceSurgePaused::<T>::get(&community_id_hash) == 0,
				Error::<T>::CommunityAdsPaused
			);

			let now = frame_system::Pallet::<T>::block_number();

			let receipt = DeliveryReceipt::<T> {
				campaign_id,
				community_id_hash,
				delivery_type,
				audience_size,
				node_id,
				node_signature,
				delivered_at: now,
				settled: false,
			};

			DeliveryReceipts::<T>::try_mutate(&community_id_hash, |receipts| {
				receipts.try_push(receipt).map_err(|_| Error::<T>::ReceiptsFull)
			})?;

			// 更新 campaign delivery count
			Campaigns::<T>::mutate(campaign_id, |maybe| {
				if let Some(c) = maybe {
					c.total_deliveries = c.total_deliveries.saturating_add(1);
				}
			});

			Self::deposit_event(Event::DeliveryReceiptSubmitted {
				campaign_id,
				community_id_hash,
				audience_size,
			});
			Ok(())
		}

		/// 结算某社区的 Era 广告 (任何人可触发)
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::from_parts(100_000_000, 15_000))]
		pub fn settle_era_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// P5 L3: 社区未因突增被暂停
			ensure!(
				AudienceSurgePaused::<T>::get(&community_id_hash) == 0,
				Error::<T>::CommunityAdsPaused
			);

			// P5 L5: 多节点交叉验证
			if let Err((min_a, max_a)) = Self::validate_node_reports(&community_id_hash) {
				Self::deposit_event(Event::NodeDeviationRejected {
					community_id_hash,
					min_audience: min_a,
					max_audience: max_a,
				});
				// 清除本 Era 报告
				NodeAudienceReports::<T>::remove(&community_id_hash);
				return Err(Error::<T>::NodeDeviationTooHigh.into());
			}

			// 清除本 Era 节点报告
			NodeAudienceReports::<T>::remove(&community_id_hash);

			let treasury = T::TreasuryAccount::get();
			let mut total_cost = BalanceOf::<T>::zero();
			let mut community_share_total = BalanceOf::<T>::zero();

			DeliveryReceipts::<T>::mutate(&community_id_hash, |receipts| {
				for receipt in receipts.iter_mut() {
					if receipt.settled {
						continue;
					}

					// 计算有效 audience
					let cap = CommunityAudienceCap::<T>::get(&community_id_hash);
					let effective = if cap > 0 {
						core::cmp::min(receipt.audience_size, cap)
					} else {
						receipt.audience_size
					};

					if let Some(campaign) = Campaigns::<T>::get(receipt.campaign_id) {
						// CPM 计费: cost = bid_per_mille * effective / 1000
						let cost = Self::compute_cpm_cost(campaign.bid_per_mille, effective);

						// 从 escrow 扣除
						let escrow = CampaignEscrow::<T>::get(receipt.campaign_id);
						let actual_cost = core::cmp::min(cost, escrow);

						if !actual_cost.is_zero() {
							// 解锁 advertiser 的 reserve
							T::Currency::unreserve(&campaign.advertiser, actual_cost);

							// 二方分成: 社区 60%, TEE 节点 (治理可调, 默认 32%), 余额→国库
							let community_pct = 60u32;
							let tee_pct = {
								let v = TeeNodeAdPct::<T>::get();
								if v == 0 { 32u32 } else { v }
							};

							let community_share = Self::percent_of(actual_cost, community_pct);
							let node_share = Self::percent_of(actual_cost, tee_pct);

							// 国库 = 剩余部分
							let final_treasury_share = actual_cost
								.saturating_sub(community_share)
								.saturating_sub(node_share);

							// 转给国库
							if !final_treasury_share.is_zero() {
								let _ = T::Currency::transfer(
									&campaign.advertiser,
									&treasury,
									final_treasury_share,
									ExistenceRequirement::AllowDeath,
								);
							}

							// 节点收入存入 NodeAdPendingRewards
							if !node_share.is_zero() {
								// 节点广告收入从 advertiser 转入国库代管
								let _ = T::Currency::transfer(
									&campaign.advertiser,
									&treasury,
									node_share,
									ExistenceRequirement::AllowDeath,
								);
								NodeAdPendingRewards::<T>::mutate(&receipt.node_id, |pending| {
									*pending = pending.saturating_add(node_share);
								});

								Self::deposit_event(Event::NodeAdRewardAccrued {
									node_id: receipt.node_id,
									amount: node_share,
								});
							}

							// 群主收入存入 claimable
							CommunityClaimable::<T>::mutate(&community_id_hash, |c| {
								*c = c.saturating_add(community_share);
							});

							// 更新 escrow
							CampaignEscrow::<T>::mutate(receipt.campaign_id, |e| {
								*e = e.saturating_sub(actual_cost);
							});

							// 更新 campaign spent
							Campaigns::<T>::mutate(receipt.campaign_id, |maybe| {
								if let Some(c) = maybe {
									c.spent = c.spent.saturating_add(actual_cost);
									// 检查预算耗尽
									if CampaignEscrow::<T>::get(receipt.campaign_id).is_zero() {
										c.status = CampaignStatus::Exhausted;
									}
								}
							});

							total_cost = total_cost.saturating_add(actual_cost);
							community_share_total = community_share_total.saturating_add(community_share);
						}
					}

					receipt.settled = true;
				}
			});

			// 更新收入统计
			if !total_cost.is_zero() {
				EraAdRevenue::<T>::mutate(&community_id_hash, |r| *r = r.saturating_add(total_cost));
				CommunityTotalRevenue::<T>::mutate(&community_id_hash, |r| *r = r.saturating_add(total_cost));

				Self::deposit_event(Event::EraAdsSettled {
					community_id_hash,
					total_cost,
					community_share: community_share_total,
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
				c.review_status = AdReviewStatus::Flagged;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignFlagged { campaign_id, reporter: who });
			Ok(())
		}

		/// 群主提取广告收入
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_ad_revenue(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let amount = CommunityClaimable::<T>::get(&community_id_hash);
			ensure!(!amount.is_zero(), Error::<T>::NothingToClaim);

			// 从国库转给群主 (广告收入在结算时已从 advertiser 转出)
			let treasury = T::TreasuryAccount::get();
			T::Currency::transfer(
				&treasury,
				&who,
				amount,
				ExistenceRequirement::AllowDeath,
			)?;

			CommunityClaimable::<T>::remove(&community_id_hash);

			Self::deposit_event(Event::AdRevenueClaimed {
				community_id_hash,
				amount,
				claimer: who,
			});
			Ok(())
		}

		/// 质押以获取 audience_cap
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn stake_for_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroStakeAmount);
			ensure!(!BannedCommunities::<T>::get(&community_id_hash), Error::<T>::CommunityBanned);

			T::Currency::reserve(&who, amount)?;

			let new_stake = CommunityAdStake::<T>::get(&community_id_hash).saturating_add(amount);
			CommunityAdStake::<T>::insert(&community_id_hash, new_stake);

			let cap = Self::compute_audience_cap(new_stake);
			CommunityAudienceCap::<T>::insert(&community_id_hash, cap);

			Self::deposit_event(Event::AdStaked {
				community_id_hash,
				amount,
				audience_cap: cap,
			});
			Ok(())
		}

		/// 取消质押
		#[pallet::call_index(10)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn unstake_from_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroStakeAmount);

			let current = CommunityAdStake::<T>::get(&community_id_hash);
			ensure!(current >= amount, Error::<T>::InsufficientStake);

			T::Currency::unreserve(&who, amount);

			let new_stake = current.saturating_sub(amount);
			CommunityAdStake::<T>::insert(&community_id_hash, new_stake);

			let cap = Self::compute_audience_cap(new_stake);
			CommunityAudienceCap::<T>::insert(&community_id_hash, cap);

			Self::deposit_event(Event::AdUnstaked { community_id_hash, amount });
			Ok(())
		}

		// ---- 双向偏好 ----

		/// 广告主拉黑社区
		#[pallet::call_index(11)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_block_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserBlacklist::<T>::try_mutate(&who, |list| {
				ensure!(!list.contains(&community_id_hash), Error::<T>::AlreadyBlacklisted);
				list.try_push(community_id_hash).map_err(|_| Error::<T>::BlacklistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserBlockedCommunity {
				advertiser: who,
				community_id_hash,
			});
			Ok(())
		}

		/// 广告主取消拉黑社区
		#[pallet::call_index(12)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_unblock_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserBlacklist::<T>::try_mutate(&who, |list| {
				let pos = list.iter().position(|h| h == &community_id_hash)
					.ok_or(Error::<T>::NotBlacklisted)?;
				list.swap_remove(pos);
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserUnblockedCommunity {
				advertiser: who,
				community_id_hash,
			});
			Ok(())
		}

		/// 广告主指定社区 (白名单)
		#[pallet::call_index(13)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn advertiser_prefer_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			AdvertiserWhitelist::<T>::try_mutate(&who, |list| {
				ensure!(!list.contains(&community_id_hash), Error::<T>::AlreadyWhitelisted);
				list.try_push(community_id_hash).map_err(|_| Error::<T>::WhitelistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::AdvertiserPreferredCommunity {
				advertiser: who,
				community_id_hash,
			});
			Ok(())
		}

		/// 社区拉黑广告主
		#[pallet::call_index(14)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn community_block_advertiser(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			CommunityBlacklist::<T>::try_mutate(&community_id_hash, |list| {
				ensure!(!list.contains(&advertiser), Error::<T>::AlreadyBlacklisted);
				list.try_push(advertiser.clone()).map_err(|_| Error::<T>::BlacklistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CommunityBlockedAdvertiser {
				community_id_hash,
				advertiser,
			});
			Ok(())
		}

		/// 社区取消拉黑广告主
		#[pallet::call_index(15)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn community_unblock_advertiser(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			CommunityBlacklist::<T>::try_mutate(&community_id_hash, |list| {
				let pos = list.iter().position(|a| a == &advertiser)
					.ok_or(Error::<T>::NotBlacklisted)?;
				list.swap_remove(pos);
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CommunityUnblockedAdvertiser {
				community_id_hash,
				advertiser,
			});
			Ok(())
		}

		/// 社区指定广告主 (白名单)
		#[pallet::call_index(16)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn community_prefer_advertiser(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			advertiser: T::AccountId,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			CommunityWhitelist::<T>::try_mutate(&community_id_hash, |list| {
				ensure!(!list.contains(&advertiser), Error::<T>::AlreadyWhitelisted);
				list.try_push(advertiser.clone()).map_err(|_| Error::<T>::WhitelistFull)?;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CommunityPreferredAdvertiser {
				community_id_hash,
				advertiser,
			});
			Ok(())
		}

		/// Slash 社区 (Root/DAO 裁决后调用)
		#[pallet::call_index(17)]
		#[pallet::weight(Weight::from_parts(60_000_000, 8_000))]
		pub fn slash_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			reporter: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;

			let stake = CommunityAdStake::<T>::get(&community_id_hash);
			ensure!(!stake.is_zero(), Error::<T>::InsufficientStake);

			let slash_pct = T::AdSlashPercentage::get();
			let slash_amount = Self::percent_of(stake, slash_pct);

			if !slash_amount.is_zero() {
				// 50% 给举报者, 50% 入国库
				let reporter_share = Self::percent_of(slash_amount, 50u32);
				let treasury_share = slash_amount.saturating_sub(reporter_share);

				// slash = unreserve from stake
				// (质押者未知, 简化: 直接从 reserve 中减少质押并铸币给举报者/国库)
				// 实际实现中应记录 staker accountId, 此处简化
				let treasury = T::TreasuryAccount::get();

				if !reporter_share.is_zero() {
					let _ = T::Currency::deposit_into_existing(&reporter, reporter_share);
				}
				if !treasury_share.is_zero() {
					let _ = T::Currency::deposit_into_existing(&treasury, treasury_share);
				}
			}

			// 减少质押
			let new_stake = stake.saturating_sub(slash_amount);
			CommunityAdStake::<T>::insert(&community_id_hash, new_stake);

			// audience_cap 砍半
			let current_cap = CommunityAudienceCap::<T>::get(&community_id_hash);
			CommunityAudienceCap::<T>::insert(&community_id_hash, current_cap / 2);

			// 递增 Slash 次数
			let count = SlashCount::<T>::get(&community_id_hash).saturating_add(1);
			SlashCount::<T>::insert(&community_id_hash, count);

			Self::deposit_event(Event::CommunitySlashed {
				community_id_hash,
				slashed_amount: slash_amount,
				slash_count: count,
			});

			// 连续 3 次 → 永久禁止
			if count >= 3 {
				BannedCommunities::<T>::insert(&community_id_hash, true);
				Self::deposit_event(Event::CommunityBannedFromAds { community_id_hash });
			}

			Ok(())
		}

		// ---- Phase 5: 反作弊 extrinsics ----

		/// 举报社区 (community-level 作弊举报)
		#[pallet::call_index(18)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn flag_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let count = CommunityFlagCount::<T>::get(&community_id_hash).saturating_add(1);
			CommunityFlagCount::<T>::insert(&community_id_hash, count);

			Self::deposit_event(Event::CommunityFlagged {
				community_id_hash,
				reporter: who,
				flag_count: count,
			});
			Ok(())
		}

		/// 节点上报 audience 统计 (L5 多节点交叉验证)
		/// 每个节点独立上报, 结算时比较偏差
		#[pallet::call_index(19)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn report_node_audience(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			audience_size: u32,
			node_id_prefix: u32,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			ensure!(!BannedCommunities::<T>::get(&community_id_hash), Error::<T>::CommunityBanned);

			NodeAudienceReports::<T>::try_mutate(&community_id_hash, |reports| {
				reports.try_push((node_id_prefix, audience_size))
					.map_err(|_| Error::<T>::NodeReportsFull)
			})?;

			Ok(())
		}

		/// 检查 audience 突增并自动暂停 (L3, 任何人可触发)
		#[pallet::call_index(20)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn check_audience_surge(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			current_audience: u32,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let previous = PreviousEraAudience::<T>::get(&community_id_hash);
			let threshold_pct = T::AudienceSurgeThresholdPct::get();

			// 首次上报或 previous=0 → 记录并通过
			if previous == 0 {
				PreviousEraAudience::<T>::insert(&community_id_hash, current_audience);
				return Ok(());
			}

			// 突增检测: current > previous * (1 + threshold/100)
			let allowed_max = previous.saturating_add(
				previous.saturating_mul(threshold_pct) / 100
			);

			if current_audience > allowed_max {
				// 暂停广告 2 个 Era
				AudienceSurgePaused::<T>::insert(&community_id_hash, 2u32);

				Self::deposit_event(Event::AudienceSurgePausedEvent {
					community_id_hash,
					previous,
					current: current_audience,
				});
			} else {
				// 正常: 如果之前被暂停, 递减暂停计数
				let paused = AudienceSurgePaused::<T>::get(&community_id_hash);
				if paused > 0 {
					let new_paused = paused.saturating_sub(1);
					if new_paused == 0 {
						AudienceSurgePaused::<T>::remove(&community_id_hash);
						Self::deposit_event(Event::AudienceSurgeResumed { community_id_hash });
					} else {
						AudienceSurgePaused::<T>::insert(&community_id_hash, new_paused);
					}
				}
			}

			// 更新历史
			PreviousEraAudience::<T>::insert(&community_id_hash, current_audience);

			Ok(())
		}

		/// 节点运营者提取广告收入
		#[pallet::call_index(21)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_node_ad_revenue(
			origin: OriginFor<T>,
			node_id: NodeId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// 验证调用者是节点运营者
			let operator = T::NodeConsensus::node_operator(&node_id)
				.ok_or(Error::<T>::NodeNotActive)?;
			ensure!(operator == who, Error::<T>::NotNodeOperator);

			let amount = NodeAdPendingRewards::<T>::get(&node_id);
			ensure!(!amount.is_zero(), Error::<T>::NoNodeAdReward);

			// 从国库转给节点运营者
			let treasury = T::TreasuryAccount::get();
			T::Currency::transfer(
				&treasury,
				&who,
				amount,
				ExistenceRequirement::AllowDeath,
			)?;

			NodeAdPendingRewards::<T>::remove(&node_id);
			NodeAdTotalEarned::<T>::mutate(&node_id, |earned| {
				*earned = earned.saturating_add(amount);
			});

			Self::deposit_event(Event::NodeAdRevenueClaimed {
				node_id,
				amount,
				claimer: who,
			});
			Ok(())
		}

		/// 设置 TEE 节点广告分成百分比 (Root/DAO)
		/// 约束: community(60%) + tee_pct <= 100, 即 tee_pct <= 40
		#[pallet::call_index(22)]
		#[pallet::weight(Weight::from_parts(10_000_000, 3_000))]
		pub fn set_tee_ad_percentage(
			origin: OriginFor<T>,
			tee_pct: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			// 社区固定 60%, TEE 节点 + 国库 ≤ 40%
			ensure!(tee_pct <= 40, Error::<T>::InvalidPercentage);

			TeeNodeAdPct::<T>::put(tee_pct);

			Self::deposit_event(Event::TeeAdPercentUpdated {
				tee_pct,
			});
			Ok(())
		}
	}

	// ========================================================================
	// Helper Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 计算 CPM 费用: bid_per_mille * audience / 1000
		fn compute_cpm_cost(bid_per_mille: BalanceOf<T>, audience: u32) -> BalanceOf<T> {
			let audience_balance: BalanceOf<T> = audience.into();
			let thousand: BalanceOf<T> = 1000u32.into();
			// bid * audience / 1000
			bid_per_mille.saturating_mul(audience_balance) / thousand
		}

		/// 百分比计算: value * pct / 100
		fn percent_of(value: BalanceOf<T>, pct: u32) -> BalanceOf<T> {
			let pct_balance: BalanceOf<T> = pct.into();
			let hundred: BalanceOf<T> = 100u32.into();
			value.saturating_mul(pct_balance) / hundred
		}

		/// 质押 → audience_cap 阶梯函数
		/// 0-10 UNIT: 20 人/UNIT → 200
		/// 10-50 UNIT: 20 人/UNIT → 1000
		/// 50-200 UNIT: ~27 人/UNIT → 5000
		/// 200+ UNIT: 递减
		///
		/// 简化实现: cap = stake_units * 20, 最大 10000

		/// L5: 多节点交叉验证 — 检查偏差是否在阈值内
		/// 返回 Ok(median) 或 Err 如果偏差过大
		pub fn validate_node_reports(community_id_hash: &CommunityIdHash) -> Result<Option<u32>, (u32, u32)> {
			let reports = NodeAudienceReports::<T>::get(community_id_hash);
			if reports.len() < 2 {
				// 不足 2 个节点, 跳过交叉验证
				return Ok(reports.first().map(|(_, a)| *a));
			}

			let audiences: alloc::vec::Vec<u32> = reports.iter().map(|(_, a)| *a).collect();
			let min_a = *audiences.iter().min().unwrap_or(&0);
			let max_a = *audiences.iter().max().unwrap_or(&0);

			if min_a == 0 {
				return Ok(Some(max_a));
			}

			// 偏差 = (max - min) / min * 100
			let deviation_pct = ((max_a - min_a) as u64 * 100) / (min_a as u64);
			let threshold = T::NodeDeviationThresholdPct::get() as u64;

			if deviation_pct > threshold {
				Err((min_a, max_a))
			} else {
				// 取中位数
				let mut sorted = audiences;
				sorted.sort();
				let median = sorted[sorted.len() / 2];
				Ok(Some(median))
			}
		}

		pub fn compute_audience_cap(stake: BalanceOf<T>) -> u32 {
			// 将 Balance 转为 u128 来做计算
			let stake_u128: u128 = stake.try_into().unwrap_or(0u128);
			// 假设 1 UNIT = 10^12 (标准 substrate)
			let unit: u128 = 1_000_000_000_000;
			let stake_units = stake_u128 / unit;

			let cap = if stake_units <= 50 {
				// 0-50: 20 人/UNIT
				stake_units.saturating_mul(20)
			} else if stake_units <= 200 {
				// 50-200: 1000 + (stake-50)*~27
				let extra = stake_units.saturating_sub(50);
				1000u128.saturating_add(extra.saturating_mul(4000) / 150)
			} else {
				// 200+: 5000 + (stake-200)*~17, 最大 10000
				let extra = stake_units.saturating_sub(200);
				5000u128.saturating_add(extra.saturating_mul(5000) / 300)
			};

			core::cmp::min(cap, 10_000) as u32
		}
	}

	// ========================================================================
	// AdScheduleProvider 实现
	// ========================================================================

	impl<T: Config> AdScheduleProvider for Pallet<T> {
		fn is_ads_enabled(community_id_hash: &CommunityIdHash) -> bool {
			CommunitySchedules::<T>::contains_key(community_id_hash)
		}

		fn community_ad_revenue(community_id_hash: &CommunityIdHash) -> u128 {
			let revenue = CommunityTotalRevenue::<T>::get(community_id_hash);
			revenue.try_into().unwrap_or(0u128)
		}
	}
}
