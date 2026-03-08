#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Ads GroupRobot — GroupRobot 广告适配层
//!
//! ## 功能
//! - TEE 节点投放验证 (实现 `DeliveryVerifier`)
//! - 社区管理员映射 (实现 `PlacementAdminProvider`)
//! - 四方收入分配: 社区/质押者/国库/节点 (实现 `RevenueDistributor`)
//! - 社区质押 → audience_cap
//! - 反作弊: audience 突增检测 + 多节点交叉验证
//! - TEE/社区分成百分比治理
//!
//! ## 设计
//! 本 pallet 不包含 Campaign CRUD 等核心广告逻辑 (由 `pallet-ads-core` 提供)。
//! 仅实现 GroupRobot 领域特定的适配 trait，并提供额外的 GroupRobot 专属 extrinsic
//! (质押/取消质押, 分成百分比调整, 反作弊等)。

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_ads_primitives::*;
use pallet_grouprobot_primitives::{
	NodeId, CommunityIdHash,
	NodeConsensusProvider, SubscriptionProvider, RewardAccruer, BotRegistryProvider,
};
use sp_runtime::traits::{Saturating, Zero};
use frame_support::traits::ExistenceRequirement;

/// Balance 类型别名
type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// 最大并行解锁请求数
const MAX_UNBONDING_REQUESTS: u32 = 8;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::{Currency, ReservableCurrency};

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		type Currency: ReservableCurrency<Self::AccountId>;

		/// 节点共识查询 (用于验证节点状态 + TEE 状态)
		type NodeConsensus: NodeConsensusProvider<Self::AccountId>;

		/// 订阅层级查询 (用于检查 Bot 订阅层级 + 功能限制)
		type Subscription: SubscriptionProvider;

		/// 统一奖励写入 (ads 节点奖励写入同一奖励池)
		type RewardPool: RewardAccruer;

		/// Bot 注册查询 (用于绑定 CommunityAdmin 到 Bot Owner)
		type BotRegistry: BotRegistryProvider<Self::AccountId>;

		/// 国库账户
		type TreasuryAccount: Get<Self::AccountId>;

		/// 奖励池账户 (广告节点份额转入)
		type RewardPoolAccount: Get<Self::AccountId>;

		/// L3 突增阈值百分比 (e.g. 100 = 允许 100% 增长)
		#[pallet::constant]
		type AudienceSurgeThresholdPct: Get<u32>;

		/// L5 多节点偏差阈值百分比 (e.g. 20 = 20%)
		#[pallet::constant]
		type NodeDeviationThresholdPct: Get<u32>;

		/// Slash 百分比 (e.g. 30 = 30%)
		#[pallet::constant]
		type AdSlashPercentage: Get<u32>;

		/// 质押解锁冷却期 (区块数, 0=即时解锁)
		#[pallet::constant]
		type UnbondingPeriod: Get<BlockNumberFor<Self>>;

		/// 质押者从社区广告份额中的分成百分比 (e.g. 10 = 社区份额中 10% 归质押者池, 必须 ≤ 100)
		#[pallet::constant]
		type StakerRewardPct: Get<u32>;

		/// 每个社区最大质押者数量 (用于限制 O(n) 遍历的上界)
		#[pallet::constant]
		type MaxStakersPerCommunity: Get<u32>;

		/// Benchmark weight info
		type WeightInfo: WeightInfo;
	}

	// ========================================================================
	// Storage — GroupRobot 专属
	// ========================================================================

	/// 社区广告质押
	#[pallet::storage]
	pub type CommunityAdStake<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, BalanceOf<T>, ValueQuery>;

	/// 社区 audience 上限 (由质押决定)
	#[pallet::storage]
	pub type CommunityAudienceCap<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 每个质押者在每个社区的质押额
	#[pallet::storage]
	pub type CommunityStakers<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, CommunityIdHash,
		Blake2_128Concat, T::AccountId,
		BalanceOf<T>,
		ValueQuery,
	>;

	/// 社区当前质押者计数
	#[pallet::storage]
	pub type CommunityStakerCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 社区管理员 (首个质押者自动成为管理员)
	#[pallet::storage]
	pub type CommunityAdmin<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, T::AccountId>;

	/// TEE 节点广告分成百分比 (None=默认15%, Some(x)=x%)
	#[pallet::storage]
	pub type TeeNodeAdPct<T: Config> = StorageValue<_, u32>;

	/// 社区广告分成百分比 (None=默认80%, Some(x)=x%)
	#[pallet::storage]
	pub type CommunityAdPct<T: Config> = StorageValue<_, u32>;

	// ---- 反作弊 ----

	/// 上一 Era 社区活跃人数 (用于 L3 突增检测)
	#[pallet::storage]
	pub type PreviousEraAudience<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 社区因 audience 突增被暂停广告 (Era 数, 0=未暂停)
	#[pallet::storage]
	pub type AudienceSurgePaused<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// 多节点 audience 上报 (用于 L5 交叉验证)
	#[pallet::storage]
	pub type NodeAudienceReports<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdHash,
		BoundedVec<(u32, u32), ConstU32<10>>,  // (node_id_prefix, audience_size)
		ValueQuery,
	>;

	/// 解锁请求队列: (社区, 质押者) → BoundedVec<(金额, 可提取区块)>
	#[pallet::storage]
	pub type UnbondingRequests<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, CommunityIdHash,
		Blake2_128Concat, T::AccountId,
		BoundedVec<(BalanceOf<T>, BlockNumberFor<T>), ConstU32<MAX_UNBONDING_REQUESTS>>,
		ValueQuery,
	>;

	/// 社区管理员主动暂停广告
	#[pallet::storage]
	pub type AdminPausedAds<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, bool, ValueQuery>;

	/// 可配置质押阶梯 [(threshold_u128, audience_cap)], 按 threshold 降序
	#[pallet::storage]
	pub type StakeTiers<T: Config> =
		StorageValue<_, BoundedVec<(u128, u32), ConstU32<10>>>;

	/// 全局广告暂停开关
	#[pallet::storage]
	pub type GlobalAdsPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// 社区被 Slash 累计次数
	#[pallet::storage]
	pub type CommunitySlashCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, u32, ValueQuery>;

	/// Bot Owner 主动禁用广告
	#[pallet::storage]
	pub type BotAdsDisabled<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdHash, bool, ValueQuery>;

	/// 质押者可提取的广告分成
	#[pallet::storage]
	pub type StakerClaimable<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, CommunityIdHash,
		Blake2_128Concat, T::AccountId,
		BalanceOf<T>,
		ValueQuery,
	>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
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
		/// TEE 节点广告分成百分比已更新
		TeeAdPercentUpdated {
			tee_pct: u32,
		},
		/// 社区广告分成百分比已更新
		CommunityAdPercentUpdated {
			community_pct: u32,
		},
		/// 社区管理员已变更
		CommunityAdminUpdated {
			community_id_hash: CommunityIdHash,
			new_admin: T::AccountId,
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
		/// Slash 转账失败 (资金已回退 reserve)
		SlashTransferFailed {
			community_id_hash: CommunityIdHash,
			staker: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// 解锁请求已创建
		UnbondingStarted {
			community_id_hash: CommunityIdHash,
			who: T::AccountId,
			amount: BalanceOf<T>,
			unlock_at: BlockNumberFor<T>,
		},
		/// 解锁资金已提取
		UnbondedWithdrawn {
			community_id_hash: CommunityIdHash,
			who: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// 社区管理员暂停广告
		AdsPausedByAdmin {
			community_id_hash: CommunityIdHash,
		},
		/// 社区管理员恢复广告
		AdsResumedByAdmin {
			community_id_hash: CommunityIdHash,
		},
		/// 质押阶梯已更新
		StakeTiersUpdated {
			tier_count: u32,
		},
		/// 社区管理员已辞职
		CommunityAdminResigned {
			community_id_hash: CommunityIdHash,
			resigned: T::AccountId,
		},
		/// 全局广告暂停状态变更
		GlobalAdsPauseToggled {
			paused: bool,
		},
		/// Bot 广告开关变更
		BotAdsToggled {
			community_id_hash: CommunityIdHash,
			disabled: bool,
		},
		/// 质押者提取广告分成
		StakerRewardClaimed {
			community_id_hash: CommunityIdHash,
			who: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Root 强制设置管理员
		ForceAdminSet {
			community_id_hash: CommunityIdHash,
			new_admin: T::AccountId,
		},
		/// Root 强制解除全部质押
		ForceUnstaked {
			community_id_hash: CommunityIdHash,
			total_amount: BalanceOf<T>,
			staker_count: u32,
		},
		/// 节点 audience 上报成功
		NodeAudienceReported {
			community_id_hash: CommunityIdHash,
			node_id: NodeId,
			audience_size: u32,
		},
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// 质押金额为零
		ZeroStakeAmount,
		/// 质押不足 (unstake 超出已质押)
		InsufficientStake,
		/// 非社区管理员
		NotCommunityAdmin,
		/// 无效的百分比值
		InvalidPercentage,
		/// 节点不存在或未激活
		NodeNotActive,
		/// 节点非 TEE
		NodeNotTee,
		/// 社区因突增被暂停广告
		CommunityAdsPaused,
		/// 节点 audience 报告已满
		NodeReportsFull,
		/// Bot 订阅层级不允许禁用广告
		AdsDisabledByTier,
		/// Bot 订阅层级不支持 TEE 功能
		TeeNotAvailableForTier,
		/// 调用者不是该 node_id 的运营者
		NodeOperatorMismatch,
		/// 社区未被暂停
		CommunityNotPaused,
		/// 无解锁请求
		NothingToWithdraw,
		/// 解锁冷却期未到
		UnbondingNotReady,
		/// 社区广告已被管理员暂停
		AdsPausedByAdmin,
		/// 社区广告未被管理员暂停
		AdsNotPausedByAdmin,
		/// 阶梯必须非空且按阈值降序
		InvalidStakeTiers,
		/// 全局广告已暂停
		GlobalAdsPausedErr,
		/// Bot 广告已禁用
		BotAdsDisabledErr,
		/// 无可提取奖励
		NoClaimableReward,
		/// 社区无质押
		NoStakeInCommunity,
		/// 解锁请求队列已满
		UnbondingQueueFull,
		/// 不是 Bot Owner
		NotBotOwner,
		/// 社区质押者数量已达上限
		MaxStakersReached,
	}

	// ========================================================================
	// Extrinsics — GroupRobot 专属
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// 为社区质押以接入广告
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::stake_for_ads())]
		pub fn stake_for_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroStakeAmount);

			let existing = CommunityStakers::<T>::get(&community_id_hash, &who);
			if existing.is_zero() {
				let count = CommunityStakerCount::<T>::get(&community_id_hash);
				ensure!(count < T::MaxStakersPerCommunity::get(), Error::<T>::MaxStakersReached);
				CommunityStakerCount::<T>::insert(&community_id_hash, count.saturating_add(1));
			}

			T::Currency::reserve(&who, amount)?;

			CommunityAdStake::<T>::mutate(&community_id_hash, |s| {
				*s = s.saturating_add(amount);
			});
			CommunityStakers::<T>::mutate(&community_id_hash, &who, |s| {
				*s = s.saturating_add(amount);
			});

			if !CommunityAdmin::<T>::contains_key(&community_id_hash) {
				CommunityAdmin::<T>::insert(&community_id_hash, who.clone());
				Self::deposit_event(Event::CommunityAdminUpdated {
					community_id_hash,
					new_admin: who.clone(),
				});
			}

			let total_stake = CommunityAdStake::<T>::get(&community_id_hash);
			let cap = Self::compute_audience_cap(total_stake);
			CommunityAudienceCap::<T>::insert(&community_id_hash, cap);

			Self::deposit_event(Event::AdStaked {
				community_id_hash,
				amount,
				audience_cap: cap,
			});
			Ok(())
		}

		/// 取消社区质押 (若 UnbondingPeriod > 0 则进入冷却期, 支持多个并行解锁请求)
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::unstake_for_ads())]
		pub fn unstake_for_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroStakeAmount);

			let staked = CommunityStakers::<T>::get(&community_id_hash, &who);
			ensure!(staked >= amount, Error::<T>::InsufficientStake);

			let new_staker_balance = staked.saturating_sub(amount);
			if new_staker_balance.is_zero() {
				CommunityStakers::<T>::remove(&community_id_hash, &who);
				CommunityStakerCount::<T>::mutate(&community_id_hash, |c| {
					*c = c.saturating_sub(1);
				});
			} else {
				CommunityStakers::<T>::insert(&community_id_hash, &who, new_staker_balance);
			}

			CommunityAdStake::<T>::mutate(&community_id_hash, |s| {
				*s = s.saturating_sub(amount);
			});

			let total_stake = CommunityAdStake::<T>::get(&community_id_hash);
			let cap = Self::compute_audience_cap(total_stake);
			CommunityAudienceCap::<T>::insert(&community_id_hash, cap);

			if total_stake.is_zero() {
				CommunityAdmin::<T>::remove(&community_id_hash);
			}

			let unbonding_period = T::UnbondingPeriod::get();
			if unbonding_period.is_zero() {
				T::Currency::unreserve(&who, amount);
			} else {
				let now = <frame_system::Pallet<T>>::block_number();
				let unlock_at = now.saturating_add(unbonding_period);
				UnbondingRequests::<T>::try_mutate(&community_id_hash, &who, |queue| {
					queue.try_push((amount, unlock_at)).map_err(|_| Error::<T>::UnbondingQueueFull)
				})?;
				Self::deposit_event(Event::UnbondingStarted {
					community_id_hash,
					who: who.clone(),
					amount,
					unlock_at,
				});
			}

			Self::deposit_event(Event::AdUnstaked {
				community_id_hash,
				amount,
			});
			Ok(())
		}

		/// 设置 TEE 节点广告分成百分比 (Root)
		/// None = 恢复默认 15%, Some(x) = 设为 x% (允许 0)
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_tee_ad_pct())]
		pub fn set_tee_ad_pct(
			origin: OriginFor<T>,
			tee_pct: Option<u32>,
		) -> DispatchResult {
			ensure_root(origin)?;
			let effective = tee_pct.unwrap_or(15);
			ensure!(effective <= 100, Error::<T>::InvalidPercentage);
			ensure!(
				effective.saturating_add(Self::effective_community_pct()) <= 100,
				Error::<T>::InvalidPercentage
			);
			match tee_pct {
				Some(v) => TeeNodeAdPct::<T>::put(v),
				None => TeeNodeAdPct::<T>::kill(),
			}
			Self::deposit_event(Event::TeeAdPercentUpdated { tee_pct: Self::effective_tee_pct() });
			Ok(())
		}

		/// 设置社区广告分成百分比 (Root)
		/// None = 恢复默认 80%, Some(x) = 设为 x% (允许 0)
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::set_community_ad_pct())]
		pub fn set_community_ad_pct(
			origin: OriginFor<T>,
			community_pct: Option<u32>,
		) -> DispatchResult {
			ensure_root(origin)?;
			let effective = community_pct.unwrap_or(80);
			ensure!(effective <= 100, Error::<T>::InvalidPercentage);
			ensure!(
				effective.saturating_add(Self::effective_tee_pct()) <= 100,
				Error::<T>::InvalidPercentage
			);
			match community_pct {
				Some(v) => CommunityAdPct::<T>::put(v),
				None => CommunityAdPct::<T>::kill(),
			}
			Self::deposit_event(Event::CommunityAdPercentUpdated { community_pct: Self::effective_community_pct() });
			Ok(())
		}

		/// 设置社区管理员 (仅当前管理员 / Bot Owner)
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_community_admin())]
		pub fn set_community_admin(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			new_admin: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let current_admin = CommunityAdmin::<T>::get(&community_id_hash);
			let bot_owner = T::BotRegistry::bot_owner(&community_id_hash);

			ensure!(
				current_admin.as_ref() == Some(&who) || bot_owner.as_ref() == Some(&who),
				Error::<T>::NotCommunityAdmin
			);

			CommunityAdmin::<T>::insert(&community_id_hash, new_admin.clone());
			Self::deposit_event(Event::CommunityAdminUpdated {
				community_id_hash,
				new_admin,
			});
			Ok(())
		}

		/// 上报节点 audience (L5 交叉验证)
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::report_node_audience())]
		pub fn report_node_audience(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			node_id: NodeId,
			audience_size: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(T::NodeConsensus::is_node_active(&node_id), Error::<T>::NodeNotActive);
			ensure!(
				T::NodeConsensus::node_operator(&node_id) == Some(who),
				Error::<T>::NodeOperatorMismatch
			);

			NodeAudienceReports::<T>::try_mutate(&community_id_hash, |reports| {
				let prefix = u32::from_le_bytes([node_id[0], node_id[1], node_id[2], node_id[3]]);
				if let Some(existing) = reports.iter_mut().find(|(p, _)| *p == prefix) {
					existing.1 = audience_size;
				} else {
					reports.try_push((prefix, audience_size)).map_err(|_| Error::<T>::NodeReportsFull)?;
				}
				Ok::<(), Error<T>>(())
			})?;

			Self::deposit_event(Event::NodeAudienceReported {
				community_id_hash,
				node_id,
				audience_size,
			});

			Ok(())
		}

		/// 检测 audience 突增 (L3, 仅 Root/DAO 可触发)
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::check_audience_surge())]
		pub fn check_audience_surge(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			current_audience: u32,
		) -> DispatchResult {
			ensure_root(origin)?;

			let previous = PreviousEraAudience::<T>::get(&community_id_hash);
			let threshold_pct = T::AudienceSurgeThresholdPct::get();

			if previous > 0 {
				let increase = current_audience.saturating_sub(previous);
				let threshold = previous.saturating_mul(threshold_pct) / 100;
				if increase > threshold {
					AudienceSurgePaused::<T>::insert(&community_id_hash, 1u32);
					Self::deposit_event(Event::AudienceSurgePausedEvent {
						community_id_hash,
						previous,
						current: current_audience,
					});
				}
			}

			PreviousEraAudience::<T>::insert(&community_id_hash, current_audience);
			Ok(())
		}

		/// 恢复因 audience 突增被暂停的社区广告投放 (Root)
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::resume_audience_surge())]
		pub fn resume_audience_surge(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				AudienceSurgePaused::<T>::get(&community_id_hash) != 0,
				Error::<T>::CommunityNotPaused
			);
			AudienceSurgePaused::<T>::remove(&community_id_hash);
			Self::deposit_event(Event::AudienceSurgeResumed { community_id_hash });
			Ok(())
		}

		/// L5 多节点交叉验证 (Root)
		/// 偏差过大时发射 NodeDeviationRejected 事件。始终返回 Ok。
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::cross_validate_nodes())]
		pub fn cross_validate_nodes(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;

			if let Err((min_audience, max_audience)) = Self::validate_node_reports(&community_id_hash) {
				Self::deposit_event(Event::NodeDeviationRejected {
					community_id_hash,
					min_audience,
					max_audience,
				});
			}

			NodeAudienceReports::<T>::remove(&community_id_hash);
			Ok(())
		}

		/// Slash 社区质押 (Root)
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::slash_community(T::MaxStakersPerCommunity::get()))]
		pub fn slash_community(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;

			let total_stake = CommunityAdStake::<T>::get(&community_id_hash);
			ensure!(!total_stake.is_zero(), Error::<T>::InsufficientStake);

			let slash_pct = T::AdSlashPercentage::get();
			let slash_amount = Self::percent_of(total_stake, slash_pct);

			if slash_amount.is_zero() {
				return Ok(());
			}

			let mut total_slashed: BalanceOf<T> = Zero::zero();
			let mut slash_count: u32 = 0;

			let stakers: alloc::vec::Vec<_> = CommunityStakers::<T>::iter_prefix(&community_id_hash)
				.collect();

			for (staker, staker_balance) in stakers {
				if staker_balance.is_zero() {
					continue;
				}
				let staker_slash = Self::percent_of(staker_balance, slash_pct);
				if staker_slash.is_zero() {
					continue;
				}

				let remaining = T::Currency::unreserve(&staker, staker_slash);
				let actual = staker_slash.saturating_sub(remaining);

				if !actual.is_zero() {
					let treasury = T::TreasuryAccount::get();
					if T::Currency::transfer(
						&staker,
						&treasury,
						actual,
						ExistenceRequirement::AllowDeath,
					).is_ok() {
						let new_balance = staker_balance.saturating_sub(actual);
						if new_balance.is_zero() {
							CommunityStakers::<T>::remove(&community_id_hash, &staker);
							CommunityStakerCount::<T>::mutate(&community_id_hash, |c| {
								*c = c.saturating_sub(1);
							});
						} else {
							CommunityStakers::<T>::insert(&community_id_hash, &staker, new_balance);
						}
						total_slashed = total_slashed.saturating_add(actual);
						slash_count = slash_count.saturating_add(1);
					} else {
						let _ = T::Currency::reserve(&staker, actual);
						Self::deposit_event(Event::SlashTransferFailed {
							community_id_hash,
							staker: staker.clone(),
							amount: actual,
						});
					}
				}
			}

			if !total_slashed.is_zero() {
				CommunityAdStake::<T>::mutate(&community_id_hash, |s| {
					*s = s.saturating_sub(total_slashed);
				});
				let new_total = CommunityAdStake::<T>::get(&community_id_hash);
				let cap = Self::compute_audience_cap(new_total);
				CommunityAudienceCap::<T>::insert(&community_id_hash, cap);

				if new_total.is_zero() {
					CommunityAdmin::<T>::remove(&community_id_hash);
				}

				CommunitySlashCount::<T>::mutate(&community_id_hash, |c| {
					*c = c.saturating_add(1);
				});

				Self::deposit_event(Event::CommunitySlashed {
					community_id_hash,
					slashed_amount: total_slashed,
					slash_count,
				});
			}

			Ok(())
		}

		// ====================================================================
		// Extrinsics (call_index 10-19)
		// ====================================================================

		/// 社区管理员暂停广告
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::admin_pause_ads())]
		pub fn admin_pause_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_admin_or_bot_owner(&who, &community_id_hash)?;
			ensure!(!AdminPausedAds::<T>::get(&community_id_hash), Error::<T>::AdsPausedByAdmin);
			AdminPausedAds::<T>::insert(&community_id_hash, true);
			Self::deposit_event(Event::AdsPausedByAdmin { community_id_hash });
			Ok(())
		}

		/// 社区管理员恢复广告
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::admin_resume_ads())]
		pub fn admin_resume_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_admin_or_bot_owner(&who, &community_id_hash)?;
			ensure!(AdminPausedAds::<T>::get(&community_id_hash), Error::<T>::AdsNotPausedByAdmin);
			AdminPausedAds::<T>::remove(&community_id_hash);
			Self::deposit_event(Event::AdsResumedByAdmin { community_id_hash });
			Ok(())
		}

		/// 社区管理员辞职 (回退到 Bot Owner, 若无则清空)
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::resign_community_admin())]
		pub fn resign_community_admin(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let current = CommunityAdmin::<T>::get(&community_id_hash);
			ensure!(current.as_ref() == Some(&who), Error::<T>::NotCommunityAdmin);

			Self::deposit_event(Event::CommunityAdminResigned {
				community_id_hash,
				resigned: who,
			});

			if let Some(bot_owner) = T::BotRegistry::bot_owner(&community_id_hash) {
				CommunityAdmin::<T>::insert(&community_id_hash, bot_owner.clone());
				Self::deposit_event(Event::CommunityAdminUpdated {
					community_id_hash,
					new_admin: bot_owner,
				});
			} else {
				CommunityAdmin::<T>::remove(&community_id_hash);
			}

			Ok(())
		}

		/// 提取所有到期的解锁质押
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::withdraw_unbonded())]
		pub fn withdraw_unbonded(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut queue = UnbondingRequests::<T>::get(&community_id_hash, &who);
			ensure!(!queue.is_empty(), Error::<T>::NothingToWithdraw);

			let now = <frame_system::Pallet<T>>::block_number();
			let mut total_withdrawn: BalanceOf<T> = Zero::zero();

			queue.retain(|&(amount, unlock_at)| {
				if now >= unlock_at {
					T::Currency::unreserve(&who, amount);
					total_withdrawn = total_withdrawn.saturating_add(amount);
					false
				} else {
					true
				}
			});

			ensure!(!total_withdrawn.is_zero(), Error::<T>::UnbondingNotReady);

			if queue.is_empty() {
				UnbondingRequests::<T>::remove(&community_id_hash, &who);
			} else {
				UnbondingRequests::<T>::insert(&community_id_hash, &who, queue);
			}

			Self::deposit_event(Event::UnbondedWithdrawn {
				community_id_hash,
				who,
				amount: total_withdrawn,
			});
			Ok(())
		}

		/// 设置质押阶梯 (Root), 按 threshold 降序
		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::set_stake_tiers())]
		pub fn set_stake_tiers(
			origin: OriginFor<T>,
			tiers: BoundedVec<(u128, u32), ConstU32<10>>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(!tiers.is_empty(), Error::<T>::InvalidStakeTiers);
			for i in 1..tiers.len() {
				ensure!(tiers[i].0 < tiers[i - 1].0, Error::<T>::InvalidStakeTiers);
			}
			let count = tiers.len() as u32;
			StakeTiers::<T>::put(tiers);
			Self::deposit_event(Event::StakeTiersUpdated { tier_count: count });
			Ok(())
		}

		/// Root 强制设置社区管理员
		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::force_set_community_admin())]
		pub fn force_set_community_admin(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			new_admin: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			CommunityAdmin::<T>::insert(&community_id_hash, new_admin.clone());
			Self::deposit_event(Event::ForceAdminSet { community_id_hash, new_admin });
			Ok(())
		}

		/// 全局广告暂停开关 (Root)
		#[pallet::call_index(16)]
		#[pallet::weight(T::WeightInfo::set_global_ads_pause())]
		pub fn set_global_ads_pause(
			origin: OriginFor<T>,
			paused: bool,
		) -> DispatchResult {
			ensure_root(origin)?;
			GlobalAdsPaused::<T>::put(paused);
			Self::deposit_event(Event::GlobalAdsPauseToggled { paused });
			Ok(())
		}

		/// Bot Owner 禁用/启用广告
		#[pallet::call_index(17)]
		#[pallet::weight(T::WeightInfo::set_bot_ads_enabled())]
		pub fn set_bot_ads_enabled(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			disabled: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::BotRegistry::bot_owner(&community_id_hash) == Some(who),
				Error::<T>::NotBotOwner
			);
			if disabled {
				BotAdsDisabled::<T>::insert(&community_id_hash, true);
			} else {
				BotAdsDisabled::<T>::remove(&community_id_hash);
			}
			Self::deposit_event(Event::BotAdsToggled { community_id_hash, disabled });
			Ok(())
		}

		/// 质押者提取广告分成
		#[pallet::call_index(18)]
		#[pallet::weight(T::WeightInfo::claim_staker_reward())]
		pub fn claim_staker_reward(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let claimable = StakerClaimable::<T>::get(&community_id_hash, &who);
			ensure!(!claimable.is_zero(), Error::<T>::NoClaimableReward);

			let treasury = T::TreasuryAccount::get();
			T::Currency::transfer(
				&treasury,
				&who,
				claimable,
				ExistenceRequirement::AllowDeath,
			)?;
			StakerClaimable::<T>::remove(&community_id_hash, &who);

			Self::deposit_event(Event::StakerRewardClaimed {
				community_id_hash,
				who,
				amount: claimable,
			});
			Ok(())
		}

		/// Root 强制解除社区全部质押
		#[pallet::call_index(19)]
		#[pallet::weight(T::WeightInfo::force_unstake(T::MaxStakersPerCommunity::get()))]
		pub fn force_unstake(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;
			let total = CommunityAdStake::<T>::get(&community_id_hash);
			ensure!(!total.is_zero(), Error::<T>::NoStakeInCommunity);

			let stakers: alloc::vec::Vec<_> =
				CommunityStakers::<T>::iter_prefix(&community_id_hash).collect();
			let mut count: u32 = 0;
			let mut total_unstaked: BalanceOf<T> = Zero::zero();

			for (staker, balance) in stakers {
				if !balance.is_zero() {
					T::Currency::unreserve(&staker, balance);
					total_unstaked = total_unstaked.saturating_add(balance);
					count = count.saturating_add(1);
				}
				CommunityStakers::<T>::remove(&community_id_hash, &staker);
			}

			CommunityAdStake::<T>::remove(&community_id_hash);
			CommunityAudienceCap::<T>::remove(&community_id_hash);
			CommunityAdmin::<T>::remove(&community_id_hash);
			CommunityStakerCount::<T>::remove(&community_id_hash);
			let _ = UnbondingRequests::<T>::clear_prefix(&community_id_hash, u32::MAX, None);

			Self::deposit_event(Event::ForceUnstaked {
				community_id_hash,
				total_amount: total_unstaked,
				staker_count: count,
			});
			Ok(())
		}
	}

	// ========================================================================
	// Helper functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		pub fn compute_audience_cap(stake: BalanceOf<T>) -> u32 {
			let stake_u128: u128 = stake.try_into().unwrap_or(0u128);

			if let Some(tiers) = StakeTiers::<T>::get() {
				for &(threshold, cap) in tiers.iter() {
					if stake_u128 >= threshold {
						return cap;
					}
				}
				return 0;
			}

			let unit: u128 = 1_000_000_000_000; // 1 UNIT
			if stake_u128 >= 1000 * unit {
				10_000
			} else if stake_u128 >= 100 * unit {
				5_000
			} else if stake_u128 >= 10 * unit {
				1_000
			} else if stake_u128 >= unit {
				200
			} else {
				0
			}
		}

		pub fn percent_of(amount: BalanceOf<T>, pct: u32) -> BalanceOf<T> {
			let pct_balance: BalanceOf<T> = pct.into();
			let hundred: BalanceOf<T> = 100u32.into();
			amount.saturating_mul(pct_balance) / hundred
		}

		pub fn validate_node_reports(community_id_hash: &CommunityIdHash) -> Result<(), (u32, u32)> {
			let reports = NodeAudienceReports::<T>::get(community_id_hash);
			if reports.len() < 2 {
				return Ok(());
			}

			let min_a = reports.iter().map(|(_, a)| *a).min().unwrap_or(0);
			let max_a = reports.iter().map(|(_, a)| *a).max().unwrap_or(0);

			if min_a == 0 {
				return Ok(());
			}

			let deviation_pct = (max_a.saturating_sub(min_a)).saturating_mul(100) / min_a;
			if deviation_pct > T::NodeDeviationThresholdPct::get() {
				Err((min_a, max_a))
			} else {
				Ok(())
			}
		}

		pub fn effective_community_pct() -> u32 {
			CommunityAdPct::<T>::get().unwrap_or(80)
		}

		pub fn effective_tee_pct() -> u32 {
			TeeNodeAdPct::<T>::get().unwrap_or(15)
		}

		pub fn ensure_admin_or_bot_owner(
			who: &T::AccountId,
			community_id_hash: &CommunityIdHash,
		) -> DispatchResult {
			let current_admin = CommunityAdmin::<T>::get(community_id_hash);
			let bot_owner = T::BotRegistry::bot_owner(community_id_hash);
			ensure!(
				current_admin.as_ref() == Some(who) || bot_owner.as_ref() == Some(who),
				Error::<T>::NotCommunityAdmin
			);
			Ok(())
		}
	}

	// ========================================================================
	// DeliveryVerifier 实现
	// ========================================================================

	impl<T: Config> DeliveryVerifier<T::AccountId> for Pallet<T> {
		fn verify_and_cap_audience(
			who: &T::AccountId,
			placement_id: &PlacementId,
			audience_size: u32,
			_node_id: Option<[u8; 32]>,
		) -> Result<u32, sp_runtime::DispatchError> {
			if GlobalAdsPaused::<T>::get() {
				return Err(Error::<T>::GlobalAdsPausedErr.into());
			}

			if BotAdsDisabled::<T>::get(placement_id) {
				return Err(Error::<T>::BotAdsDisabledErr.into());
			}

			if AdminPausedAds::<T>::get(placement_id) {
				return Err(Error::<T>::AdsPausedByAdmin.into());
			}

			let gate = T::Subscription::effective_feature_gate(placement_id);
			if !gate.tee_access {
				return Err(Error::<T>::TeeNotAvailableForTier.into());
			}

			if gate.can_disable_ads {
				let stake = CommunityAdStake::<T>::get(placement_id);
				if stake.is_zero() {
					return Err(Error::<T>::AdsDisabledByTier.into());
				}
			}

			if !T::NodeConsensus::is_tee_node_by_operator(who) {
				return Err(Error::<T>::NodeNotTee.into());
			}

			if AudienceSurgePaused::<T>::get(placement_id) != 0 {
				return Err(Error::<T>::CommunityAdsPaused.into());
			}

			let cap = CommunityAudienceCap::<T>::get(placement_id);
			let effective = if cap > 0 {
				core::cmp::min(audience_size, cap)
			} else {
				audience_size
			};

			Ok(effective)
		}
	}

	// ========================================================================
	// PlacementAdminProvider 实现
	// ========================================================================

	impl<T: Config> PlacementAdminProvider<T::AccountId> for Pallet<T> {
		fn placement_admin(placement_id: &PlacementId) -> Option<T::AccountId> {
			CommunityAdmin::<T>::get(placement_id)
				.or_else(|| T::BotRegistry::bot_owner(placement_id))
		}

		fn is_placement_banned(_placement_id: &PlacementId) -> bool {
			false
		}

		fn placement_status(placement_id: &PlacementId) -> PlacementStatus {
			if GlobalAdsPaused::<T>::get() {
				return PlacementStatus::Paused;
			}
			if BotAdsDisabled::<T>::get(placement_id) {
				return PlacementStatus::Paused;
			}
			if AdminPausedAds::<T>::get(placement_id) {
				return PlacementStatus::Paused;
			}
			if AudienceSurgePaused::<T>::get(placement_id) != 0 {
				return PlacementStatus::Paused;
			}
			if Self::placement_admin(placement_id).is_some() {
				PlacementStatus::Active
			} else {
				PlacementStatus::Unknown
			}
		}
	}

	// ========================================================================
	// RevenueDistributor 实现 — 四方分成
	// ========================================================================

	impl<T: Config> RevenueDistributor<T::AccountId, BalanceOf<T>> for Pallet<T> {
		fn distribute(
			placement_id: &PlacementId,
			total_cost: BalanceOf<T>,
			_advertiser: &T::AccountId,
		) -> Result<RevenueBreakdown<BalanceOf<T>>, sp_runtime::DispatchError> {
			frame_support::ensure!(
				!GlobalAdsPaused::<T>::get(),
				Error::<T>::GlobalAdsPausedErr
			);

			frame_support::ensure!(
				!BotAdsDisabled::<T>::get(placement_id),
				Error::<T>::BotAdsDisabledErr
			);

			frame_support::ensure!(
				!AdminPausedAds::<T>::get(placement_id),
				Error::<T>::AdsPausedByAdmin
			);

			frame_support::ensure!(
				AudienceSurgePaused::<T>::get(placement_id) == 0,
				Error::<T>::CommunityAdsPaused
			);

			let community_pct = Self::effective_community_pct();
			let tee_pct = Self::effective_tee_pct();
			let community_share = Self::percent_of(total_cost, community_pct);
			let node_share = Self::percent_of(total_cost, tee_pct);

			// 节点份额: 国库 → RewardPoolAccount + accrue
			// node_id 使用 placement_id (CommunityIdHash) 作为奖励池的分配键
			if !node_share.is_zero() {
				let treasury = T::TreasuryAccount::get();
				let reward_pool = T::RewardPoolAccount::get();
				if T::Currency::transfer(
					&treasury,
					&reward_pool,
					node_share,
					ExistenceRequirement::AllowDeath,
				).is_ok() {
					let reward_key: pallet_grouprobot_primitives::NodeId = *placement_id;
					let node_share_u128: u128 = node_share.try_into().unwrap_or(0u128);
					T::RewardPool::accrue_node_reward(&reward_key, node_share_u128);

					Self::deposit_event(Event::NodeAdRewardAccrued {
						node_id: reward_key,
						amount: node_share,
					});
				} else {
					log::warn!(
						"[ads-grouprobot] distribute: node share transfer failed for placement {:?}",
						placement_id,
					);
				}
			}

			let staker_pct = T::StakerRewardPct::get().min(100);
			let staker_pool = Self::percent_of(community_share, staker_pct);
			if !staker_pool.is_zero() {
				let total_stake = CommunityAdStake::<T>::get(placement_id);
				if !total_stake.is_zero() {
					let stakers: alloc::vec::Vec<_> =
						CommunityStakers::<T>::iter_prefix(placement_id).collect();
					for (staker, staker_balance) in stakers {
						if staker_balance.is_zero() {
							continue;
						}
						let staker_reward = staker_pool
							.saturating_mul(staker_balance) / total_stake;
						if !staker_reward.is_zero() {
							StakerClaimable::<T>::mutate(placement_id, &staker, |c| {
								*c = c.saturating_add(staker_reward);
							});
						}
					}
				}
			}

			let net_community_share = community_share.saturating_sub(staker_pool);
			let platform_share = total_cost
				.saturating_sub(community_share)
				.saturating_sub(node_share);
			Ok(RevenueBreakdown {
				placement_share: net_community_share,
				node_share,
				platform_share,
			})
		}
	}
}
