#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Ads GroupRobot — GroupRobot 广告适配层
//!
//! ## 功能
//! - TEE 节点投放验证 (实现 `DeliveryVerifier`)
//! - 社区管理员映射 (实现 `PlacementAdminProvider`)
//! - 三方收入分配: 社区/国库/节点 (实现 `RevenueDistributor`)
//! - 社区质押 → audience_cap (实现 `PlacementStakeProvider`)
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

	/// TEE 节点广告分成百分比 (默认 15, 即 15%)
	#[pallet::storage]
	pub type TeeNodeAdPct<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// 社区广告分成百分比 (默认 80, 即 80%)
	#[pallet::storage]
	pub type CommunityAdPct<T: Config> = StorageValue<_, u32, ValueQuery>;

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
		/// L5 多节点偏差过大
		NodeDeviationTooHigh,
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

		/// 取消社区质押
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

			T::Currency::unreserve(&who, amount);

			// M1 审计修复: 清理零余额质押者 + 零总质押管理员
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

			// M1: 总质押为零时清理管理员
			if total_stake.is_zero() {
				CommunityAdmin::<T>::remove(&community_id_hash);
			}

			Self::deposit_event(Event::AdUnstaked {
				community_id_hash,
				amount,
			});
			Ok(())
		}

		/// 设置 TEE 节点广告分成百分比 (Root)
		/// H2 审计修复: 直接校验传入值,不再使用 "0=默认" 语义做校验
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
		pub fn set_tee_ad_pct(
			origin: OriginFor<T>,
			tee_pct: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(tee_pct <= 100, Error::<T>::InvalidPercentage);
			ensure!(
				tee_pct.saturating_add(Self::effective_community_pct()) <= 100,
				Error::<T>::InvalidPercentage
			);
			TeeNodeAdPct::<T>::put(tee_pct);
			Self::deposit_event(Event::TeeAdPercentUpdated { tee_pct });
			Ok(())
		}

		/// 设置社区广告分成百分比 (Root)
		/// H2 审计修复: 直接校验传入值,不再使用 "0=默认" 语义做校验
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(20_000_000, 4_000))]
		pub fn set_community_ad_pct(
			origin: OriginFor<T>,
			community_pct: u32,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(community_pct <= 100, Error::<T>::InvalidPercentage);
			ensure!(
				community_pct.saturating_add(Self::effective_tee_pct()) <= 100,
				Error::<T>::InvalidPercentage
			);
			CommunityAdPct::<T>::put(community_pct);
			Self::deposit_event(Event::CommunityAdPercentUpdated { community_pct });
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

				Self::deposit_event(Event::CommunitySlashed {
					community_id_hash,
					slashed_amount: total_slashed,
					slash_count,
				});
			}

			Ok(())
		}
	}

	// ========================================================================
	// Helper functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 根据质押额计算 audience_cap (阶梯函数)
		pub fn compute_audience_cap(stake: BalanceOf<T>) -> u32 {
			let stake_u128: u128 = stake.try_into().unwrap_or(0u128);
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

		/// 获取 community_pct (默认 80)
		pub fn effective_community_pct() -> u32 {
			let v = CommunityAdPct::<T>::get();
			if v == 0 { 80u32 } else { v }
		}

		/// 获取 tee_pct (默认 15)
		pub fn effective_tee_pct() -> u32 {
			let v = TeeNodeAdPct::<T>::get();
			if v == 0 { 15u32 } else { v }
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
		) -> Result<u32, sp_runtime::DispatchError> {
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
	}

	// ========================================================================
	// RevenueDistributor 实现 — 三方分成
	// ========================================================================

	/// H1 审计修复: distribute 实际执行三方分成 — 社区/国库/节点
	/// ads-core settle_era_ads 已将资金从广告主转入国库。
	/// 此处从国库转出节点份额到 RewardPoolAccount 并 accrue_node_reward。
	/// 社区份额通过返回值记入 PlacementClaimable (由 ads-core 管理)。
	/// 国库保留剩余 (100% - community% - tee%)。
	impl<T: Config> RevenueDistributor<T::AccountId, BalanceOf<T>> for Pallet<T> {
		fn distribute(
			placement_id: &PlacementId,
			total_cost: BalanceOf<T>,
			_advertiser: &T::AccountId,
		) -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
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
					// 将奖励写入统一奖励池 (按 placement_id 作为 node_id 的代理)
					// 实际 node_id 需在更高层关联，此处使用 placement_id 前 32 字节
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

			// 社区份额返回给 ads-core 记入 PlacementClaimable
			// 国库保留剩余 (total_cost - community_share - node_share)
			Ok(community_share)
		}
	}
}
