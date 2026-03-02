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

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
	/// 受众规模 (已裁切)
	pub audience_size: u32,
	/// CPM 倍率 (基点)
	pub cpm_multiplier_bps: u32,
	pub delivered_at: BlockNumberFor<T>,
	/// 是否已结算
	pub settled: bool,
	/// 提交者
	pub submitter: T::AccountId,
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

		/// 广告位管理员 (适配层实现)
		type PlacementAdmin: PlacementAdminProvider<Self::AccountId>;

		/// 收入分配策略 (适配层实现)
		type RevenueDistributor: RevenueDistributor<Self::AccountId, BalanceOf<Self>>;

		/// 私有广告注册费用
		#[pallet::constant]
		type PrivateAdRegistrationFee: Get<BalanceOf<Self>>;
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
		/// 已审核通过的 Campaign 不可重复审核
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
		/// 结算转账失败
		SettlementTransferFailed,
		/// 私有广告注册次数必须 > 0
		ZeroPrivateAdCount,
		/// Campaign ID 溢出
		CampaignIdOverflow,
		/// 过期时间无效 (必须晚于当前区块)
		InvalidExpiry,
		/// 质押不足
		InsufficientStake,
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
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!text.is_empty(), Error::<T>::EmptyAdText);
			ensure!(delivery_types > 0 && delivery_types <= 0b111, Error::<T>::InvalidDeliveryTypes);
			ensure!(bid_per_mille >= T::MinBidPerMille::get(), Error::<T>::BidTooLow);
			ensure!(!total_budget.is_zero(), Error::<T>::ZeroBudget);

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
				daily_budget,
				total_budget,
				spent: Zero::zero(),
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
				ensure!(
					c.status == CampaignStatus::Active
					|| c.status == CampaignStatus::Paused
					|| c.status == CampaignStatus::Exhausted,
					Error::<T>::CampaignInactive
				);

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
			cpm_multiplier_bps: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let campaign = Campaigns::<T>::get(campaign_id)
				.ok_or(Error::<T>::CampaignNotFound)?;
			ensure!(campaign.status == CampaignStatus::Active, Error::<T>::CampaignNotActive);
			ensure!(campaign.review_status == AdReviewStatus::Approved, Error::<T>::CampaignNotApproved);

			let now = frame_system::Pallet::<T>::block_number();
			ensure!(now <= campaign.expires_at, Error::<T>::CampaignExpired);

			ensure!(!BannedPlacements::<T>::get(&placement_id), Error::<T>::PlacementBanned);

			// 委托适配层验证 + 裁切 audience
			let effective_audience = T::DeliveryVerifier::verify_and_cap_audience(
				&who,
				&placement_id,
				audience_size,
			).map_err(|_| Error::<T>::DeliveryVerificationFailed)?;

			ensure!(
				effective_audience >= T::MinAudienceSize::get(),
				Error::<T>::AudienceBelowMinimum
			);

			let receipt = DeliveryReceipt::<T> {
				campaign_id,
				placement_id,
				audience_size: effective_audience,
				cpm_multiplier_bps,
				delivered_at: now,
				settled: false,
				submitter: who,
			};

			DeliveryReceipts::<T>::try_mutate(&placement_id, |receipts| {
				receipts.try_push(receipt).map_err(|_| Error::<T>::ReceiptsFull)
			})?;

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
			let _who = ensure_signed(origin)?;

			let treasury = T::TreasuryAccount::get();
			let mut total_cost = BalanceOf::<T>::zero();
			let mut placement_share_total = BalanceOf::<T>::zero();

			// 收集待结算收据数据
			let receipts_snapshot = DeliveryReceipts::<T>::get(&placement_id);
			let mut settle_items: alloc::vec::Vec<(u64, BalanceOf<T>, T::AccountId)> = alloc::vec::Vec::new();

			for receipt in receipts_snapshot.iter() {
				if receipt.settled {
					continue;
				}
				if let Some(campaign) = Campaigns::<T>::get(receipt.campaign_id) {
					let cost = Self::compute_cpm_cost(
						campaign.bid_per_mille,
						receipt.audience_size,
						receipt.cpm_multiplier_bps,
					);
					let escrow = CampaignEscrow::<T>::get(receipt.campaign_id);
					let actual_cost = core::cmp::min(cost, escrow);
					if !actual_cost.is_zero() {
						settle_items.push((
							receipt.campaign_id,
							actual_cost,
							campaign.advertiser.clone(),
						));
					}
				}
			}

			// 执行转账
			for (campaign_id, _snapshot_cost, advertiser) in settle_items.iter() {
				let current_escrow = CampaignEscrow::<T>::get(campaign_id);
				let adjusted_cost = core::cmp::min(*_snapshot_cost, current_escrow);
				if adjusted_cost.is_zero() {
					continue;
				}

				// 解锁 advertiser 的 reserve
				T::Currency::unreserve(advertiser, adjusted_cost);

				// 全额转入国库
				T::Currency::transfer(
					advertiser,
					&treasury,
					adjusted_cost,
					ExistenceRequirement::AllowDeath,
				).map_err(|_| Error::<T>::SettlementTransferFailed)?;

				// 委托适配层分配收入 (从国库分配给各方)
				let placement_share = T::RevenueDistributor::distribute(
					&placement_id,
					adjusted_cost,
					advertiser,
				).unwrap_or(Zero::zero());

				// 广告位可提取份额记账
				PlacementClaimable::<T>::mutate(&placement_id, |c| {
					*c = c.saturating_add(placement_share);
				});

				// 更新 escrow
				CampaignEscrow::<T>::mutate(campaign_id, |e| {
					*e = e.saturating_sub(adjusted_cost);
				});

				// 更新 campaign spent
				Campaigns::<T>::mutate(campaign_id, |maybe| {
					if let Some(c) = maybe {
						c.spent = c.spent.saturating_add(adjusted_cost);
						if CampaignEscrow::<T>::get(*campaign_id).is_zero() {
							c.status = CampaignStatus::Exhausted;
						}
					}
				});

				total_cost = total_cost.saturating_add(adjusted_cost);
				placement_share_total = placement_share_total.saturating_add(placement_share);
			}

			// 清空本 Era 收据
			DeliveryReceipts::<T>::remove(&placement_id);

			// 更新收入统计
			if !total_cost.is_zero() {
				EraAdRevenue::<T>::mutate(&placement_id, |r| *r = r.saturating_add(total_cost));
				PlacementTotalRevenue::<T>::mutate(&placement_id, |r| *r = r.saturating_add(total_cost));

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
				ensure!(
					c.review_status == AdReviewStatus::Approved || c.review_status == AdReviewStatus::Pending,
					Error::<T>::AlreadyReviewed
				);
				c.review_status = AdReviewStatus::Flagged;
				Ok::<(), DispatchError>(())
			})?;

			Self::deposit_event(Event::CampaignFlagged { campaign_id, reporter: who });
			Ok(())
		}

		/// 广告位管理员提取广告收入
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(40_000_000, 6_000))]
		pub fn claim_ad_revenue(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let admin = T::PlacementAdmin::placement_admin(&placement_id)
				.ok_or(Error::<T>::NotPlacementAdmin)?;
			ensure!(admin == who, Error::<T>::NotPlacementAdmin);

			let amount = PlacementClaimable::<T>::get(&placement_id);
			ensure!(!amount.is_zero(), Error::<T>::NothingToClaim);

			let treasury = T::TreasuryAccount::get();
			T::Currency::transfer(
				&treasury,
				&who,
				amount,
				ExistenceRequirement::AllowDeath,
			)?;

			PlacementClaimable::<T>::remove(&placement_id);

			Self::deposit_event(Event::AdRevenueClaimed {
				placement_id,
				amount,
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

			let count = PlacementFlagCount::<T>::get(&placement_id).saturating_add(1);
			PlacementFlagCount::<T>::insert(&placement_id, count);

			Self::deposit_event(Event::PlacementFlagged {
				placement_id,
				reporter: who,
				flag_count: count,
			});
			Ok(())
		}

		/// Slash 广告位 (Root/DAO)
		#[pallet::call_index(18)]
		#[pallet::weight(Weight::from_parts(60_000_000, 8_000))]
		pub fn slash_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			reporter: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;

			// 递增 Slash 次数
			let count = SlashCount::<T>::get(&placement_id).saturating_add(1);
			SlashCount::<T>::insert(&placement_id, count);

			Self::deposit_event(Event::PlacementSlashed {
				placement_id,
				slashed_amount: Zero::zero(),
				slash_count: count,
			});

			// 连续 3 次 → 永久禁止
			if count >= 3 {
				BannedPlacements::<T>::insert(&placement_id, true);
				Self::deposit_event(Event::PlacementBannedFromAds { placement_id });
			}

			let _ = reporter;
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
	}

	// ========================================================================
	// AdScheduleProvider 实现
	// ========================================================================

	impl<T: Config> AdScheduleProvider for Pallet<T> {
		fn is_ads_enabled(placement_id: &PlacementId) -> bool {
			PlacementTotalRevenue::<T>::contains_key(placement_id) ||
			PlacementEraDeliveries::<T>::get(placement_id) > 0
		}

		fn community_ad_revenue(placement_id: &PlacementId) -> u128 {
			let revenue = PlacementTotalRevenue::<T>::get(placement_id);
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
