#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Subscription — 订阅管理 + Escrow + 层级费用结算
//!
//! 从 consensus pallet 拆分而来, 负责:
//! - 订阅 CRUD (subscribe/deposit/cancel/change_tier)
//! - Escrow 预存余额管理
//! - Era 订阅费结算 (游标分页)
//! - 实现 SubscriptionProvider + SubscriptionSettler trait

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
use sp_runtime::traits::{Saturating, UniqueSaturatedInto, Zero};

/// 订阅信息
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct SubscriptionRecord<T: Config> {
	pub owner: T::AccountId,
	pub bot_id_hash: BotIdHash,
	pub tier: SubscriptionTier,
	pub fee_per_era: BalanceOf<T>,
	pub started_at: BlockNumberFor<T>,
	pub status: SubscriptionStatus,
}

/// 广告承诺订阅记录
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AdCommitmentRecord<T: Config> {
	pub owner: T::AccountId,
	pub bot_id_hash: BotIdHash,
	pub community_id_hash: CommunityIdHash,
	/// 每 Era 承诺接受的广告数
	pub committed_ads_per_era: u32,
	/// 对应的权益层级
	pub effective_tier: SubscriptionTier,
	/// 连续未达标 Era 数
	pub underdelivery_eras: u8,
	pub status: AdCommitmentStatus,
	pub started_at: BlockNumberFor<T>,
}

type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
		/// Bot 注册查询
		type BotRegistry: BotRegistryProvider<Self::AccountId>;
		/// Basic 层级每 Era 费用
		#[pallet::constant]
		type BasicFeePerEra: Get<BalanceOf<Self>>;
		/// Pro 层级每 Era 费用
		#[pallet::constant]
		type ProFeePerEra: Get<BalanceOf<Self>>;
		/// Enterprise 层级每 Era 费用
		#[pallet::constant]
		type EnterpriseFeePerEra: Get<BalanceOf<Self>>;
		/// 国库账户 (订阅费 10% 转入)
		type TreasuryAccount: Get<Self::AccountId>;
		/// 奖励池账户 (订阅费 80% 节点份额转入)
		type RewardPoolAccount: Get<Self::AccountId>;
		/// 每次 Era 结算最多处理的订阅数 (游标分页)
		#[pallet::constant]
		type MaxSubscriptionSettlePerEra: Get<u32>;
		/// Era 长度 (区块数, 用于按比例计算取消费用)
		#[pallet::constant]
		type EraLength: Get<BlockNumberFor<Self>>;
		/// Era 起始区块查询 (从 consensus 读取)
		type EraStartBlockProvider: Get<BlockNumberFor<Self>>;
		/// 当前 Era 编号查询
		type CurrentEraProvider: Get<u64>;

		/// 广告投放计数查询 (从 ads pallet 读取社区投放次数)
		type AdDelivery: AdDeliveryProvider;

		/// 广告承诺阈值: Basic 层级最低广告数/Era
		#[pallet::constant]
		type AdBasicThreshold: Get<u32>;
		/// 广告承诺阈值: Pro 层级最低广告数/Era
		#[pallet::constant]
		type AdProThreshold: Get<u32>;
		/// 广告承诺阈值: Enterprise 层级最低广告数/Era
		#[pallet::constant]
		type AdEnterpriseThreshold: Get<u32>;
		/// 连续未达标最大 Era 数 (超过则降级为 Free)
		#[pallet::constant]
		type MaxUnderdeliveryEras: Get<u8>;
	}

	// ========================================================================
	// Storage
	// ========================================================================

	/// 订阅表: bot_id_hash → Subscription
	#[pallet::storage]
	pub type Subscriptions<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, SubscriptionRecord<T>>;

	/// 订阅 Escrow: bot_id_hash → 预存余额
	#[pallet::storage]
	pub type SubscriptionEscrow<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, BalanceOf<T>, ValueQuery>;

	/// 订阅结算游标 (上次结算到的 bot_id_hash, None=从头开始)
	#[pallet::storage]
	pub type SubscriptionSettleCursor<T: Config> = StorageValue<_, BotIdHash>;

	/// 当前 Era 是否还有待结算的订阅 (true=上一轮未结算完, 需继续)
	#[pallet::storage]
	pub type SubscriptionSettlePending<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// 广告承诺订阅表: bot_id_hash → AdCommitmentRecord
	#[pallet::storage]
	pub type AdCommitments<T: Config> =
		StorageMap<_, Blake2_128Concat, BotIdHash, AdCommitmentRecord<T>>;

	/// M5: 广告承诺结算游标
	#[pallet::storage]
	pub type AdCommitmentSettleCursor<T: Config> = StorageValue<_, BotIdHash>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Subscribed { bot_id_hash: BotIdHash, tier: SubscriptionTier, owner: T::AccountId },
		/// 订阅到期/暂停 → 降级为 Free
		FreeTierFallback { bot_id_hash: BotIdHash },
		SubscriptionDeposited { bot_id_hash: BotIdHash, amount: BalanceOf<T> },
		SubscriptionCancelled { bot_id_hash: BotIdHash },
		TierChanged { bot_id_hash: BotIdHash, old_tier: SubscriptionTier, new_tier: SubscriptionTier },
		/// 订阅费已转入国库
		SubscriptionFeeCollected { bot_id_hash: BotIdHash, amount: BalanceOf<T> },
		/// 取消订阅时按比例扣除当期费用
		SubscriptionCancelledWithProration { bot_id_hash: BotIdHash, prorated_fee: BalanceOf<T>, refunded: BalanceOf<T> },
		/// 广告承诺订阅已创建
		AdCommitted { bot_id_hash: BotIdHash, community_id_hash: CommunityIdHash, committed_ads_per_era: u32, tier: SubscriptionTier },
		/// 广告承诺订阅已取消
		AdCommitmentCancelled { bot_id_hash: BotIdHash },
		/// 广告承诺达标检查通过
		AdCommitmentFulfilled { bot_id_hash: BotIdHash, delivered: u32, committed: u32 },
		/// 广告承诺未达标
		AdCommitmentUnderdelivered { bot_id_hash: BotIdHash, delivered: u32, committed: u32, consecutive: u8 },
		/// 广告承诺因连续未达标而降级
		AdCommitmentDowngraded { bot_id_hash: BotIdHash },
		/// 已取消的付费订阅记录已清理
		SubscriptionCleaned { bot_id_hash: BotIdHash },
		/// 已取消的广告承诺记录已清理
		AdCommitmentCleaned { bot_id_hash: BotIdHash },
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// Bot 未注册
		BotNotRegistered,
		/// 不是 Bot 所有者
		NotBotOwner,
		/// 订阅已存在
		SubscriptionAlreadyExists,
		/// 订阅不存在
		SubscriptionNotFound,
		/// 订阅已取消
		SubscriptionAlreadyCancelled,
		/// 层级相同
		SameTier,
		/// 预存不足
		InsufficientDeposit,
		/// Free 层级无需订阅
		CannotSubscribeFree,
		/// 订阅费转账到国库失败
		SubscriptionFeeTransferFailed,
		/// 仅订阅 Owner 可充值
		NotSubscriptionOwner,
		/// Bot 未分配运营者 (订阅前必须 assign_bot_to_operator)
		BotHasNoOperator,
		/// 广告承诺已存在
		AdCommitmentAlreadyExists,
		/// 广告承诺不存在
		AdCommitmentNotFound,
		/// 广告承诺已取消
		AdCommitmentAlreadyCancelled,
		/// 承诺广告数不足 (未达 Basic 阈值)
		CommitmentBelowMinimum,
		/// 充值金额不能为零
		ZeroDepositAmount,
		/// 订阅未处于活跃状态 (已取消或已暂停)
		SubscriptionNotActive,
		/// 订阅未处于终态 (仅 Cancelled 可清理)
		SubscriptionNotTerminal,
		/// 广告承诺未处于终态 (仅 Cancelled 可清理)
		AdCommitmentNotTerminal,
	}

	// ========================================================================
	// Extrinsics
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// 订阅 Bot 服务
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn subscribe(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			tier: SubscriptionTier,
			deposit: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(tier.is_paid(), Error::<T>::CannotSubscribeFree);
			ensure!(T::BotRegistry::is_bot_active(&bot_id_hash), Error::<T>::BotNotRegistered);
			ensure!(
				T::BotRegistry::bot_owner(&bot_id_hash) == Some(who.clone()),
				Error::<T>::NotBotOwner
			);
			ensure!(!Subscriptions::<T>::contains_key(&bot_id_hash), Error::<T>::SubscriptionAlreadyExists);
			ensure!(T::BotRegistry::bot_operator(&bot_id_hash).is_some(), Error::<T>::BotHasNoOperator);

			let fee = Self::tier_fee(&tier);
			ensure!(deposit >= fee, Error::<T>::InsufficientDeposit);

			T::Currency::reserve(&who, deposit)?;
			SubscriptionEscrow::<T>::insert(&bot_id_hash, deposit);

			let now = frame_system::Pallet::<T>::block_number();
			let sub = SubscriptionRecord::<T> {
				owner: who.clone(),
				bot_id_hash,
				tier,
				fee_per_era: fee,
				started_at: now,
				status: SubscriptionStatus::Active,
			};
			Subscriptions::<T>::insert(&bot_id_hash, sub);

			Self::deposit_event(Event::Subscribed { bot_id_hash, tier, owner: who });
			Ok(())
		}

		/// 充值订阅
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn deposit_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroDepositAmount);
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
			ensure!(sub.owner == who, Error::<T>::NotSubscriptionOwner);

			T::Currency::reserve(&who, amount)?;
			SubscriptionEscrow::<T>::mutate(&bot_id_hash, |escrow| {
				*escrow = escrow.saturating_add(amount);
			});

			// M1-fix: 仅在充值后余额覆盖至少一个 Era 费用时才重新激活
			if sub.status == SubscriptionStatus::PastDue || sub.status == SubscriptionStatus::Suspended {
				let new_escrow = SubscriptionEscrow::<T>::get(&bot_id_hash);
				if new_escrow >= sub.fee_per_era {
					Subscriptions::<T>::mutate(&bot_id_hash, |maybe_sub| {
						if let Some(s) = maybe_sub {
							s.status = SubscriptionStatus::Active;
						}
					});
				}
			}

			Self::deposit_event(Event::SubscriptionDeposited { bot_id_hash, amount });
			Ok(())
		}

		/// 取消订阅
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn cancel_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let fee_per_era = Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe_sub| -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
				let sub = maybe_sub.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.owner == who, Error::<T>::NotBotOwner);
				ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
				let fee = sub.fee_per_era;
				sub.status = SubscriptionStatus::Cancelled;
				Ok(fee)
			})?;

			// 按比例扣除当期已使用的费用
			let escrow = SubscriptionEscrow::<T>::take(&bot_id_hash);
			let now = frame_system::Pallet::<T>::block_number();
			let era_start = T::EraStartBlockProvider::get();
			let era_length = T::EraLength::get();
			let blocks_used = now.saturating_sub(era_start);
			let prorated_fee = if era_length > BlockNumberFor::<T>::default() {
				let fee_u128: u128 = fee_per_era.unique_saturated_into();
				let used_u128: u128 = blocks_used.unique_saturated_into();
				let len_u128: u128 = era_length.unique_saturated_into();
				let prorated_u128 = fee_u128.saturating_mul(used_u128) / len_u128;
				let prorated: BalanceOf<T> = prorated_u128.unique_saturated_into();
				core::cmp::min(prorated, escrow)
			} else {
				BalanceOf::<T>::zero()
			};
			let refundable = escrow.saturating_sub(prorated_fee);

			if !escrow.is_zero() {
				T::Currency::unreserve(&who, escrow);
			}
			if !prorated_fee.is_zero() {
				let treasury = T::TreasuryAccount::get();
				T::Currency::transfer(
					&who,
					&treasury,
					prorated_fee,
					ExistenceRequirement::AllowDeath,
				).map_err(|_| Error::<T>::SubscriptionFeeTransferFailed)?;
			}

			Self::deposit_event(Event::SubscriptionCancelledWithProration {
				bot_id_hash,
				prorated_fee,
				refunded: refundable,
			});
			Ok(())
		}

		/// 变更订阅层级
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn change_tier(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			new_tier: SubscriptionTier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(new_tier.is_paid(), Error::<T>::CannotSubscribeFree);
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe_sub| -> DispatchResult {
				let sub = maybe_sub.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.owner == who, Error::<T>::NotBotOwner);
				ensure!(
					matches!(sub.status, SubscriptionStatus::Active | SubscriptionStatus::PastDue),
					Error::<T>::SubscriptionNotActive
				);
				ensure!(sub.tier != new_tier, Error::<T>::SameTier);

				// M1-R4: 升级到更贵层级时验证 escrow 充足性
				let new_fee = Self::tier_fee(&new_tier);
				if new_fee > sub.fee_per_era {
					let escrow = SubscriptionEscrow::<T>::get(&bot_id_hash);
					ensure!(escrow >= new_fee, Error::<T>::InsufficientDeposit);
				}

				let old_tier = sub.tier;
				sub.tier = new_tier;
				sub.fee_per_era = Self::tier_fee(&new_tier);

				Self::deposit_event(Event::TierChanged { bot_id_hash, old_tier, new_tier });
				Ok(())
			})
		}

		/// 广告承诺订阅: 群主承诺社区接受 N 条广告/Era, 换取对应层级权益
		///
		/// ## 双重收益政策 (G3)
		///
		/// 广告承诺路径同时产生两项收益:
		/// 1. **订阅层级权益** — 由承诺数量决定 (ads_to_tier 映射)
		/// 2. **广告分成收入** — 由实际投放的 CPM 广告产生 (CommunityClaimable)
		///
		/// 这是正确的激励设计: 群主用用户注意力换取服务, 广告收入是对用户体验
		/// 成本的补偿。阈值参数 (AdBasicThreshold/AdProThreshold/AdEnterpriseThreshold)
		/// 由治理设定, 需确保承诺门槛高于纯收益换算值, 以维持付费订阅的吸引力。
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn commit_ads(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			community_id_hash: CommunityIdHash,
			committed_ads_per_era: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(T::BotRegistry::is_bot_active(&bot_id_hash), Error::<T>::BotNotRegistered);
			ensure!(
				T::BotRegistry::bot_owner(&bot_id_hash) == Some(who.clone()),
				Error::<T>::NotBotOwner
			);
			ensure!(!AdCommitments::<T>::contains_key(&bot_id_hash), Error::<T>::AdCommitmentAlreadyExists);
			// L1-fix: 与 subscribe 一致, 要求 Bot 已分配运营者
			ensure!(T::BotRegistry::bot_operator(&bot_id_hash).is_some(), Error::<T>::BotHasNoOperator);

			let tier = Self::ads_to_tier(committed_ads_per_era);
			ensure!(tier.is_paid(), Error::<T>::CommitmentBelowMinimum);

			let now = frame_system::Pallet::<T>::block_number();
			let record = AdCommitmentRecord::<T> {
				owner: who,
				bot_id_hash,
				community_id_hash,
				committed_ads_per_era,
				effective_tier: tier,
				underdelivery_eras: 0,
				status: AdCommitmentStatus::Active,
				started_at: now,
			};
			AdCommitments::<T>::insert(&bot_id_hash, record);

			Self::deposit_event(Event::AdCommitted {
				bot_id_hash,
				community_id_hash,
				committed_ads_per_era,
				tier,
			});
			Ok(())
		}

		/// 取消广告承诺订阅
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn cancel_ad_commitment(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			AdCommitments::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::AdCommitmentNotFound)?;
				ensure!(record.owner == who, Error::<T>::NotBotOwner);
				ensure!(record.status != AdCommitmentStatus::Cancelled, Error::<T>::AdCommitmentAlreadyCancelled);
				record.status = AdCommitmentStatus::Cancelled;
				Ok(())
			})?;
			Self::deposit_event(Event::AdCommitmentCancelled { bot_id_hash });
			Ok(())
		}

		/// M2-R3: 清理已取消的付费订阅记录 (任何人可调用)
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn cleanup_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			ensure_signed(origin)?;
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.status == SubscriptionStatus::Cancelled, Error::<T>::SubscriptionNotTerminal);
			Subscriptions::<T>::remove(&bot_id_hash);
			// L2-R4: 防御性清理 escrow 存储残留
			SubscriptionEscrow::<T>::remove(&bot_id_hash);
			Self::deposit_event(Event::SubscriptionCleaned { bot_id_hash });
			Ok(())
		}

		/// M2-R3: 清理已取消的广告承诺记录 (任何人可调用)
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn cleanup_ad_commitment(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			ensure_signed(origin)?;
			let record = AdCommitments::<T>::get(&bot_id_hash).ok_or(Error::<T>::AdCommitmentNotFound)?;
			ensure!(record.status == AdCommitmentStatus::Cancelled, Error::<T>::AdCommitmentNotTerminal);
			AdCommitments::<T>::remove(&bot_id_hash);
			Self::deposit_event(Event::AdCommitmentCleaned { bot_id_hash });
			Ok(())
		}
	}

	// ========================================================================
	// Internal Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 获取层级费用
		pub fn tier_fee(tier: &SubscriptionTier) -> BalanceOf<T> {
			match tier {
				SubscriptionTier::Free => BalanceOf::<T>::zero(),
				SubscriptionTier::Basic => T::BasicFeePerEra::get(),
				SubscriptionTier::Pro => T::ProFeePerEra::get(),
				SubscriptionTier::Enterprise => T::EnterpriseFeePerEra::get(),
			}
		}

		/// 广告承诺数量 → 对应层级
		pub fn ads_to_tier(ads_per_era: u32) -> SubscriptionTier {
			let enterprise = T::AdEnterpriseThreshold::get();
			let pro = T::AdProThreshold::get();
			let basic = T::AdBasicThreshold::get();
			if ads_per_era >= enterprise {
				SubscriptionTier::Enterprise
			} else if ads_per_era >= pro {
				SubscriptionTier::Pro
			} else if ads_per_era >= basic {
				SubscriptionTier::Basic
			} else {
				SubscriptionTier::Free
			}
		}

		/// 层级排序值 (用于取 max)
		fn tier_rank(tier: &SubscriptionTier) -> u8 {
			match tier {
				SubscriptionTier::Free => 0,
				SubscriptionTier::Basic => 1,
				SubscriptionTier::Pro => 2,
				SubscriptionTier::Enterprise => 3,
			}
		}

		/// 查询 Bot 的有效层级 (综合付费订阅 + 广告承诺, 取较高者)
		pub fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
			let paid_tier = match Subscriptions::<T>::get(bot_id_hash) {
				Some(sub) => match sub.status {
					SubscriptionStatus::Active | SubscriptionStatus::PastDue => sub.tier,
					_ => SubscriptionTier::Free,
				},
				None => SubscriptionTier::Free,
			};

			let ad_tier = match AdCommitments::<T>::get(bot_id_hash) {
				Some(c) if matches!(c.status, AdCommitmentStatus::Active | AdCommitmentStatus::Underdelivery) => c.effective_tier,
				_ => SubscriptionTier::Free,
			};

			if Self::tier_rank(&ad_tier) > Self::tier_rank(&paid_tier) {
				ad_tier
			} else {
				paid_tier
			}
		}

		/// 查询 Bot 的功能限制
		pub fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
			Self::effective_tier(bot_id_hash).feature_gate()
		}

		/// Era 订阅费结算 (游标分页)
		///
		/// 返回 (total_income, treasury_share): 本次结算收取的总收入和实际转入国库的金额
		/// (90% node_share 已直接转给 Bot 运营者, 10% treasury_share 转入国库)
		pub fn settle_era_subscriptions() -> (BalanceOf<T>, BalanceOf<T>) {
			let treasury = T::TreasuryAccount::get();
			let max_settle = T::MaxSubscriptionSettlePerEra::get();
			let mut subscription_income = BalanceOf::<T>::zero();
			let mut total_treasury = BalanceOf::<T>::zero();
			let mut settled = 0u32;

			let iter = match SubscriptionSettleCursor::<T>::get() {
				Some(cursor) => Subscriptions::<T>::iter_from(
					Subscriptions::<T>::hashed_key_for(&cursor),
				),
				None => Subscriptions::<T>::iter(),
			};

			let mut last_key: Option<BotIdHash> = None;
			for (bot_hash, sub) in iter {
				if settled >= max_settle {
					// C1-fix: 不更新 last_key — cursor 应保持为最后已处理的 key
					// iter_from 从给定 key 之后开始, 若设为未处理 key 则该条目被永久跳过
					SubscriptionSettlePending::<T>::put(true);
					break;
				}
				settled += 1;
				last_key = Some(bot_hash);

				if sub.status == SubscriptionStatus::Cancelled {
					continue;
				}
				let escrow = SubscriptionEscrow::<T>::get(&sub.bot_id_hash);
				if escrow >= sub.fee_per_era {
					// M1-R3: 先 unreserve 以允许 transfer, escrow 扣减移至转账成功后
					T::Currency::unreserve(&sub.owner, sub.fee_per_era);

					// 90/10 拆分 (运营者 90%, 国库 10%)
					let node_share = sub.fee_per_era * 90u32.into() / 100u32.into();
					let treasury_share = sub.fee_per_era.saturating_sub(node_share);

					let reward_pool = T::RewardPoolAccount::get();

					let node_recipient = T::BotRegistry::bot_operator(&sub.bot_id_hash)
						.unwrap_or_else(|| reward_pool.clone());

					let node_ok = T::Currency::transfer(
						&sub.owner,
						&node_recipient,
						node_share,
						ExistenceRequirement::AllowDeath,
					).is_ok();
					let treasury_ok = T::Currency::transfer(
						&sub.owner,
						&treasury,
						treasury_share,
						ExistenceRequirement::AllowDeath,
					).is_ok();
					if node_ok && treasury_ok {
						// M1-R3: 全部转账成功后才扣减 escrow
						SubscriptionEscrow::<T>::mutate(&sub.bot_id_hash, |e| {
							*e = e.saturating_sub(sub.fee_per_era);
						});
						subscription_income = subscription_income.saturating_add(sub.fee_per_era);
						total_treasury = total_treasury.saturating_add(treasury_share);
						Self::deposit_event(Event::SubscriptionFeeCollected {
							bot_id_hash: sub.bot_id_hash,
							amount: sub.fee_per_era,
						});
					} else {
						// M1-R3: 部分失败 — 回收未转出金额, 仅扣减实际转出额
						let paid = (if node_ok { node_share } else { BalanceOf::<T>::zero() })
							.saturating_add(if treasury_ok { treasury_share } else { BalanceOf::<T>::zero() });
						let to_re_reserve = sub.fee_per_era.saturating_sub(paid);
						if !to_re_reserve.is_zero() {
							let _ = T::Currency::reserve(&sub.owner, to_re_reserve);
						}
						if !paid.is_zero() {
							SubscriptionEscrow::<T>::mutate(&sub.bot_id_hash, |e| {
								*e = e.saturating_sub(paid);
							});
						}
						log::warn!(
							"⚠️ Subscription fee transfer failed for bot {:?}: node_ok={}, treasury_ok={}",
							sub.bot_id_hash, node_ok, treasury_ok,
						);
					}
				} else {
					let bot_hash_inner = sub.bot_id_hash;
					Subscriptions::<T>::mutate(&bot_hash_inner, |maybe_sub| {
						if let Some(s) = maybe_sub {
							if s.status == SubscriptionStatus::Active {
								s.status = SubscriptionStatus::PastDue;
							} else if s.status == SubscriptionStatus::PastDue {
								s.status = SubscriptionStatus::Suspended;
								Self::deposit_event(Event::FreeTierFallback { bot_id_hash: bot_hash_inner });
							}
						}
					});
				}
			}

			if settled < max_settle {
				SubscriptionSettleCursor::<T>::kill();
				SubscriptionSettlePending::<T>::put(false);
			} else if let Some(key) = last_key {
				SubscriptionSettleCursor::<T>::put(key);
			}

			(subscription_income, total_treasury)
		}

		/// Era 广告承诺达标检查
		///
		/// 遍历所有 AdCommitments, 检查实际投放是否达标:
		/// - 达标: 重置 underdelivery_eras
		/// - 未达标: underdelivery_eras++, 超过阈值则降级为 Cancelled
		pub fn settle_ad_commitments() {
			let max_underdelivery = T::MaxUnderdeliveryEras::get();
			// M5-fix: 游标分页, 复用 MaxSubscriptionSettlePerEra 限制
			let max_settle = T::MaxSubscriptionSettlePerEra::get();
			let mut settled = 0u32;

			let iter = match AdCommitmentSettleCursor::<T>::get() {
				Some(cursor) => AdCommitments::<T>::iter_from(
					AdCommitments::<T>::hashed_key_for(&cursor),
				),
				None => AdCommitments::<T>::iter(),
			};

			let mut last_key: Option<BotIdHash> = None;
			for (bot_hash, record) in iter {
				if settled >= max_settle {
					// C1-fix: 不更新 last_key, 避免跳过未处理条目
					break;
				}
				settled += 1;
				last_key = Some(bot_hash);

				if record.status == AdCommitmentStatus::Cancelled {
					continue;
				}

				let delivered = T::AdDelivery::era_delivery_count(&record.community_id_hash);
				let committed = record.committed_ads_per_era;

				if delivered >= committed {
					// 达标: 重置计数
					AdCommitments::<T>::mutate(&bot_hash, |maybe| {
						if let Some(r) = maybe {
							r.underdelivery_eras = 0;
							r.status = AdCommitmentStatus::Active;
						}
					});
					Self::deposit_event(Event::AdCommitmentFulfilled {
						bot_id_hash: bot_hash,
						delivered,
						committed,
					});
				} else {
					// 未达标
					let new_count = record.underdelivery_eras.saturating_add(1);
					if new_count >= max_underdelivery {
						// 连续未达标超限: 降级
						AdCommitments::<T>::mutate(&bot_hash, |maybe| {
							if let Some(r) = maybe {
								r.status = AdCommitmentStatus::Cancelled;
								r.underdelivery_eras = new_count;
							}
						});
						Self::deposit_event(Event::AdCommitmentDowngraded {
							bot_id_hash: bot_hash,
						});
					} else {
						AdCommitments::<T>::mutate(&bot_hash, |maybe| {
							if let Some(r) = maybe {
								r.underdelivery_eras = new_count;
								r.status = AdCommitmentStatus::Underdelivery;
							}
						});
						Self::deposit_event(Event::AdCommitmentUnderdelivered {
							bot_id_hash: bot_hash,
							delivered,
							committed,
							consecutive: new_count,
						});
					}
				}

				// 重置社区投放计数
				T::AdDelivery::reset_era_deliveries(&record.community_id_hash);
			}

			// M5-fix: 更新游标
			if settled < max_settle {
				AdCommitmentSettleCursor::<T>::kill();
			} else if let Some(key) = last_key {
				AdCommitmentSettleCursor::<T>::put(key);
			}
		}
	}

	// ========================================================================
	// SubscriptionProvider 实现
	// ========================================================================

	impl<T: Config> SubscriptionProvider for Pallet<T> {
		fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
			Pallet::<T>::effective_tier(bot_id_hash)
		}
		fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
			Pallet::<T>::effective_feature_gate(bot_id_hash)
		}
	}

	// ========================================================================
	// SubscriptionSettler 实现
	// ========================================================================

	impl<T: Config> SubscriptionSettler for Pallet<T> {
		fn settle_era() -> EraSettlementResult {
			let (income, treasury) = Self::settle_era_subscriptions();
			Self::settle_ad_commitments();
			EraSettlementResult {
				total_income: income.unique_saturated_into(),
				treasury_share: treasury.unique_saturated_into(),
			}
		}
	}
}
