#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Ads GroupRobot — GroupRobot 广告适配层
//!
//! ## 功能
//! - TEE 节点投放验证 (实现 `DeliveryVerifier`)
//! - 社区管理员映射 (实现 `PlacementAdminProvider`)
//! - 三方收入分配: 社区/国库/节点 (实现 `RevenueDistributor`)
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

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::{Currency, ReservableCurrency};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
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

		/// 质押者从社区广告份额中的分成百分比 (e.g. 10 = 社区份额中 10% 归质押者池)
		#[pallet::constant]
		type StakerRewardPct: Get<u32>;
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

	/// 解锁请求: (社区, 质押者) → (金额, 可提取区块)
	#[pallet::storage]
	pub type UnbondingRequests<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, CommunityIdHash,
		Blake2_128Concat, T::AccountId,
		(BalanceOf<T>, BlockNumberFor<T>),
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
		/// 已有解锁请求待处理
		UnbondingAlreadyPending,
		/// 不是 Bot Owner
		NotBotOwner,
	}

	// ========================================================================
	// Extrinsics — GroupRobot 专属
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// 为社区质押以接入广告
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn stake_for_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroStakeAmount);

			T::Currency::reserve(&who, amount)?;

			CommunityAdStake::<T>::mutate(&community_id_hash, |s| {
				*s = s.saturating_add(amount);
			});
			CommunityStakers::<T>::mutate(&community_id_hash, &who, |s| {
				*s = s.saturating_add(amount);
			});

			// 首个质押者自动成为管理员
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

		/// 取消社区质押 (若 UnbondingPeriod > 0 则进入冷却期)
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn unstake_for_ads(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroStakeAmount);

			let staked = CommunityStakers::<T>::get(&community_id_hash, &who);
			ensure!(staked >= amount, Error::<T>::InsufficientStake);
			ensure!(
				!UnbondingRequests::<T>::contains_key(&community_id_hash, &who),
				Error::<T>::UnbondingAlreadyPending
			);

			// 更新质押者余额 + 总质押 + cap (立即生效)
			let new_staker_balance = staked.saturating_sub(amount);
			if new_staker_balance.is_zero() {
				CommunityStakers::<T>::remove(&community_id_hash, &who);
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

			// 冷却期: 资金保持 reserved 直到 withdraw_unbonded
			let unbonding_period = T::UnbondingPeriod::get();
			if unbonding_period.is_zero() {
				T::Currency::unreserve(&who, amount);
			} else {
				let now = <frame_system::Pallet<T>>::block_number();
				let unlock_at = now.saturating_add(unbonding_period);
				UnbondingRequests::<T>::insert(&community_id_hash, &who, (amount, unlock_at));
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
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
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
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
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

			// M2 审计修复: 同一 node prefix 更新而非追加,避免单节点消耗全部槽位
			// L-GR4: 使用前 4 字节而非仅首字节,降低碰撞概率
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
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
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

		/// H3 审计修复: 恢复因 audience 突增被暂停的社区广告投放 (Root)
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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

		/// M3 审计修复: L5 多节点交叉验证 (Root)
		/// 调用 validate_node_reports,偏差过大时发射 NodeDeviationRejected 事件。
		/// 注意: 始终返回 Ok,因为 Substrate 的事务层会在 Err 时回滚所有存储变更(含事件),
		/// 使 NodeDeviationRejected 无法上链。调用者通过事件判断偏差检测结果。
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
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

			// 无论结果如何都清理报告
			NodeAudienceReports::<T>::remove(&community_id_hash);
			Ok(())
		}

		/// M3+L2 审计修复: Slash 社区质押 (Root)
		/// 使用 AdSlashPercentage 计算 slash 金额,从管理员 reserved 中释放并转入国库。
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
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

			// 按比例从各质押者扣除
			let mut total_slashed: BalanceOf<T> = Zero::zero();
			let mut slash_count: u32 = 0;

			let stakers: alloc::vec::Vec<_> = CommunityStakers::<T>::iter_prefix(&community_id_hash)
				.collect();

			for (staker, staker_balance) in stakers {
				if staker_balance.is_zero() {
					continue;
				}
				// 按比例: staker_slash = slash_amount * staker_balance / total_stake
				let staker_slash = Self::percent_of(staker_balance, slash_pct);
				if staker_slash.is_zero() {
					continue;
				}

				// unreserve 并计算实际释放
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
						} else {
							CommunityStakers::<T>::insert(&community_id_hash, &staker, new_balance);
						}
						total_slashed = total_slashed.saturating_add(actual);
						slash_count = slash_count.saturating_add(1);
					} else {
						// 转账失败: 重新 reserve 回退释放的资金
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
				// 更新总质押 + audience_cap
				CommunityAdStake::<T>::mutate(&community_id_hash, |s| {
					*s = s.saturating_sub(total_slashed);
				});
				let new_total = CommunityAdStake::<T>::get(&community_id_hash);
				let cap = Self::compute_audience_cap(new_total);
				CommunityAudienceCap::<T>::insert(&community_id_hash, cap);

				if new_total.is_zero() {
					CommunityAdmin::<T>::remove(&community_id_hash);
				}

				// 累计 slash 次数
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
		// 新增 Extrinsics (call_index 10-19)
		// ====================================================================

		/// 社区管理员暂停广告
		#[pallet::call_index(10)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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

		/// 提取解锁到期的质押
		#[pallet::call_index(13)]
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
		pub fn withdraw_unbonded(
			origin: OriginFor<T>,
			community_id_hash: CommunityIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let (amount, unlock_at) = UnbondingRequests::<T>::get(&community_id_hash, &who)
				.ok_or(Error::<T>::NothingToWithdraw)?;
			let now = <frame_system::Pallet<T>>::block_number();
			ensure!(now >= unlock_at, Error::<T>::UnbondingNotReady);

			T::Currency::unreserve(&who, amount);
			UnbondingRequests::<T>::remove(&community_id_hash, &who);

			Self::deposit_event(Event::UnbondedWithdrawn {
				community_id_hash,
				who,
				amount,
			});
			Ok(())
		}

		/// 设置质押阶梯 (Root), 按 threshold 降序
		#[pallet::call_index(14)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(10_000_000, 2_000))]
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
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
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
		#[pallet::weight(Weight::from_parts(30_000_000, 5_000))]
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
		#[pallet::weight(Weight::from_parts(100_000_000, 20_000))]
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
		/// 根据质押额计算 audience_cap (阶梯函数, 优先使用可配置 StakeTiers)
		pub fn compute_audience_cap(stake: BalanceOf<T>) -> u32 {
			let stake_u128: u128 = stake.try_into().unwrap_or(0u128);

			// 若有自定义阶梯, 使用自定义; 否则使用硬编码默认
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

		/// 百分比计算
		pub fn percent_of(amount: BalanceOf<T>, pct: u32) -> BalanceOf<T> {
			let pct_balance: BalanceOf<T> = pct.into();
			let hundred: BalanceOf<T> = 100u32.into();
			amount.saturating_mul(pct_balance) / hundred
		}

		/// 验证节点报告偏差 (L5)
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

		/// 获取 community_pct (None=默认80)
		pub fn effective_community_pct() -> u32 {
			CommunityAdPct::<T>::get().unwrap_or(80)
		}

		/// 获取 tee_pct (None=默认15)
		pub fn effective_tee_pct() -> u32 {
			TeeNodeAdPct::<T>::get().unwrap_or(15)
		}

		/// 确认调用者是社区管理员或 Bot Owner
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
	// DeliveryVerifier 实现 — TEE 节点验证 + 订阅门控 + audience_cap 裁切
	// ========================================================================

	impl<T: Config> DeliveryVerifier<T::AccountId> for Pallet<T> {
		fn verify_and_cap_audience(
			who: &T::AccountId,
			placement_id: &PlacementId,
			audience_size: u32,
			_node_id: Option<[u8; 32]>,
		) -> Result<u32, sp_runtime::DispatchError> {
			// 0. 全局暂停检查
			if GlobalAdsPaused::<T>::get() {
				return Err(Error::<T>::GlobalAdsPausedErr.into());
			}

			// 0b. Bot Owner 禁用检查
			if BotAdsDisabled::<T>::get(placement_id) {
				return Err(Error::<T>::BotAdsDisabledErr.into());
			}

			// 0c. 管理员暂停检查
			if AdminPausedAds::<T>::get(placement_id) {
				return Err(Error::<T>::AdsPausedByAdmin.into());
			}

			// 1. 订阅层级检查
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

			// 2. TEE 节点验证 — 调用者必须是 TEE 节点运营者
			if !T::NodeConsensus::is_tee_node_by_operator(who) {
				return Err(Error::<T>::NodeNotTee.into());
			}

			// 3. 突增暂停检查
			if AudienceSurgePaused::<T>::get(placement_id) != 0 {
				return Err(Error::<T>::CommunityAdsPaused.into());
			}

			// 4. audience_cap 裁切
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
			// 委托给 ads-core 的 BannedPlacements (通过 runtime 配置)
			// 此处也可检查 GroupRobot 特有的 ban 逻辑
			false
		}

		fn placement_status(placement_id: &PlacementId) -> PlacementStatus {
			if GlobalAdsPaused::<T>::get() {
				return PlacementStatus::Paused;
			}
			if BotAdsDisabled::<T>::get(placement_id) {
				return PlacementStatus::Paused;
			}
			// 有管理员或 bot owner → 存在
			if Self::placement_admin(placement_id).is_some() {
				PlacementStatus::Active
			} else {
				PlacementStatus::Unknown
			}
		}
	}

	// ========================================================================
	// RevenueDistributor 实现 — 三方分成
	// ========================================================================

	/// H1 审计修复: distribute 实际执行三方分成 — 社区/国库/节点
	/// ads-core settle_era_ads 已将资金从广告主转入国库。
	/// 此处从国库转出节点份额到 RewardPoolAccount 并 accrue_node_reward。
	/// 社区份额通过返回值记入 PlacementClaimable (由 ads-core 管理)。
	/// 国库保留剩余 (100% - community% - tee%)。
	/// 质押者从社区份额中按 StakerRewardPct 分成。
	impl<T: Config> RevenueDistributor<T::AccountId, BalanceOf<T>> for Pallet<T> {
		fn distribute(
			placement_id: &PlacementId,
			total_cost: BalanceOf<T>,
			_advertiser: &T::AccountId,
		) -> Result<RevenueBreakdown<BalanceOf<T>>, sp_runtime::DispatchError> {
			// 全局暂停检查
			frame_support::ensure!(
				!GlobalAdsPaused::<T>::get(),
				Error::<T>::GlobalAdsPausedErr
			);

			// Bot Owner 禁用检查
			frame_support::ensure!(
				!BotAdsDisabled::<T>::get(placement_id),
				Error::<T>::BotAdsDisabledErr
			);

			// 管理员暂停检查
			frame_support::ensure!(
				!AdminPausedAds::<T>::get(placement_id),
				Error::<T>::AdsPausedByAdmin
			);

			// M4 审计修复: 被暂停的社区不应获得收入分配
			frame_support::ensure!(
				AudienceSurgePaused::<T>::get(placement_id) == 0,
				Error::<T>::CommunityAdsPaused
			);

			let community_pct = Self::effective_community_pct();
			let tee_pct = Self::effective_tee_pct();
			let community_share = Self::percent_of(total_cost, community_pct);
			let node_share = Self::percent_of(total_cost, tee_pct);

			// 节点份额: 国库 → RewardPoolAccount + accrue
			if !node_share.is_zero() {
				let treasury = T::TreasuryAccount::get();
				let reward_pool = T::RewardPoolAccount::get();
				if T::Currency::transfer(
					&treasury,
					&reward_pool,
					node_share,
					ExistenceRequirement::AllowDeath,
				).is_ok() {
					let node_id: pallet_grouprobot_primitives::NodeId = *placement_id;
					let node_share_u128: u128 = node_share.try_into().unwrap_or(0u128);
					T::RewardPool::accrue_node_reward(&node_id, node_share_u128);

					Self::deposit_event(Event::NodeAdRewardAccrued {
						node_id,
						amount: node_share,
					});
				} else {
					log::warn!(
						"[ads-grouprobot] distribute: node share transfer failed for placement {:?}",
						placement_id,
					);
				}
			}

			// 质押者分成: 从社区份额中按 StakerRewardPct 比例分配给各质押者
			let staker_pct = T::StakerRewardPct::get();
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
						// staker_reward = staker_pool * staker_balance / total_stake
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

			// 社区份额返回给 ads-core 记入 PlacementClaimable (扣除质押者份额)
			// 国库保留剩余 (total_cost - community_share - node_share)
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
