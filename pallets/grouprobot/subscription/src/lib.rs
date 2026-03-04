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

	/// 治理可配置的层级功能限制 (覆盖 SubscriptionTier::feature_gate() 硬编码默认值)
	#[pallet::storage]
	pub type TierFeatureGateOverrides<T: Config> =
		StorageMap<_, Blake2_128Concat, SubscriptionTier, TierFeatureGate>;

	/// 治理动态费率覆盖: tier → fee_per_era (None = 使用 Config 常量)
	#[pallet::storage]
	pub type TierFeeOverrides<T: Config> =
		StorageMap<_, Blake2_128Concat, SubscriptionTier, BalanceOf<T>>;

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
		/// 治理更新层级功能限制
		TierFeatureGateUpdated { tier: SubscriptionTier, gate: TierFeatureGate },
		/// Bot Owner 部分提取 Escrow
		EscrowWithdrawn { bot_id_hash: BotIdHash, amount: BalanceOf<T>, remaining: BalanceOf<T> },
		/// 广告承诺已更新 (原子性修改)
		AdCommitmentUpdated { bot_id_hash: BotIdHash, old_ads: u32, new_ads: u32, old_tier: SubscriptionTier, new_tier: SubscriptionTier },
		/// Root 强制取消订阅
		SubscriptionForceCancelled { bot_id_hash: BotIdHash, escrow_slashed: BalanceOf<T> },
		/// Root 强制暂停订阅
		SubscriptionForceSuspended { bot_id_hash: BotIdHash },
		/// Root 强制变更层级
		TierForceChanged { bot_id_hash: BotIdHash, old_tier: SubscriptionTier, new_tier: SubscriptionTier },
		/// Root 恢复层级功能限制为默认值
		TierFeatureGateReset { tier: SubscriptionTier },
		/// Root 动态调整层级费率
		TierFeeUpdated { tier: SubscriptionTier, new_fee: BalanceOf<T> },
		/// Operator 为 Bot 充值 Escrow
		OperatorDeposited { bot_id_hash: BotIdHash, operator: T::AccountId, amount: BalanceOf<T> },
		/// Owner 主动暂停订阅
		SubscriptionPaused { bot_id_hash: BotIdHash },
		/// Owner 恢复已暂停的订阅
		SubscriptionResumed { bot_id_hash: BotIdHash },
		/// 批量清理已取消记录
		BatchCleaned { subscriptions_cleaned: u32, ad_commitments_cleaned: u32 },
		/// Root 强制取消广告承诺
		AdCommitmentForceCancelled { bot_id_hash: BotIdHash },
		/// Escrow 余额不足 2 Era 费用预警
		EscrowLow { bot_id_hash: BotIdHash, remaining: BalanceOf<T>, fee_per_era: BalanceOf<T> },
		/// settle 跳过非活跃 Bot
		SettleSkippedInactiveBot { bot_id_hash: BotIdHash },
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
		/// 提取后 Escrow 不足一个 Era 费用
		WithdrawWouldUnderfund,
		/// 订阅未处于暂停状态
		SubscriptionNotPaused,
		/// 订阅已处于暂停状态
		SubscriptionAlreadyPaused,
		/// 承诺广告数未变更
		SameCommitment,
		/// 批量清理列表为空
		EmptyBatch,
		/// 不是 Bot Operator
		NotBotOperator,
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
		/// 治理更新层级功能限制 (root)
		///
		/// 覆盖 SubscriptionTier::feature_gate() 硬编码默认值,
		/// 允许治理调整各层级的 max_rules / log_retention_days /
		/// forced_ads_per_day / can_disable_ads / tee_access 参数.
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn update_tier_feature_gate(
			origin: OriginFor<T>,
			tier: SubscriptionTier,
			gate: TierFeatureGate,
		) -> DispatchResult {
			ensure_root(origin)?;
			TierFeatureGateOverrides::<T>::insert(&tier, &gate);
			Self::deposit_event(Event::TierFeatureGateUpdated { tier, gate });
			Ok(())
		}

		/// Root 强制取消订阅 (治理应急)
		///
		/// Escrow 没收至国库, 订阅状态设为 Cancelled.
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn force_cancel_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);

			// 没收 escrow 至国库
			let escrow = SubscriptionEscrow::<T>::take(&bot_id_hash);
			if !escrow.is_zero() {
				T::Currency::unreserve(&sub.owner, escrow);
				let treasury = T::TreasuryAccount::get();
				let _ = T::Currency::transfer(
					&sub.owner,
					&treasury,
					escrow,
					ExistenceRequirement::AllowDeath,
				);
			}

			Subscriptions::<T>::mutate(&bot_id_hash, |maybe| {
				if let Some(s) = maybe {
					s.status = SubscriptionStatus::Cancelled;
				}
			});
			Self::deposit_event(Event::SubscriptionForceCancelled { bot_id_hash, escrow_slashed: escrow });
			Ok(())
		}

		/// Bot Owner 部分提取 Escrow
		///
		/// 提取后 Escrow 余额必须 >= fee_per_era (至少覆盖 1 Era).
		#[pallet::call_index(10)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn withdraw_escrow(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroDepositAmount);
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.owner == who, Error::<T>::NotBotOwner);
			ensure!(
				matches!(sub.status, SubscriptionStatus::Active | SubscriptionStatus::Paused),
				Error::<T>::SubscriptionNotActive
			);

			let escrow = SubscriptionEscrow::<T>::get(&bot_id_hash);
			let remaining = escrow.saturating_sub(amount);
			ensure!(remaining >= sub.fee_per_era, Error::<T>::WithdrawWouldUnderfund);

			T::Currency::unreserve(&who, amount);
			SubscriptionEscrow::<T>::insert(&bot_id_hash, remaining);
			Self::deposit_event(Event::EscrowWithdrawn { bot_id_hash, amount, remaining });
			Ok(())
		}

		/// 原子性修改广告承诺 (避免 cancel + commit 的降级窗口)
		#[pallet::call_index(11)]
		#[pallet::weight(Weight::from_parts(50_000_000, 8_000))]
		pub fn update_ad_commitment(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			new_committed_ads_per_era: u32,
			new_community_id_hash: Option<CommunityIdHash>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			AdCommitments::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::AdCommitmentNotFound)?;
				ensure!(record.owner == who, Error::<T>::NotBotOwner);
				ensure!(
					matches!(record.status, AdCommitmentStatus::Active | AdCommitmentStatus::Underdelivery),
					Error::<T>::AdCommitmentAlreadyCancelled
				);

				let new_tier = Self::ads_to_tier(new_committed_ads_per_era);
				ensure!(new_tier.is_paid(), Error::<T>::CommitmentBelowMinimum);
				// 至少 ads 数量或社区有变更
				let community_changed = new_community_id_hash.map_or(false, |c| c != record.community_id_hash);
				ensure!(
					record.committed_ads_per_era != new_committed_ads_per_era || community_changed,
					Error::<T>::SameCommitment
				);

				let old_ads = record.committed_ads_per_era;
				let old_tier = record.effective_tier;
				record.committed_ads_per_era = new_committed_ads_per_era;
				record.effective_tier = new_tier;
				if let Some(c) = new_community_id_hash {
					record.community_id_hash = c;
				}
				// 变更后重置未达标计数
				record.underdelivery_eras = 0;
				record.status = AdCommitmentStatus::Active;

				Self::deposit_event(Event::AdCommitmentUpdated {
					bot_id_hash,
					old_ads,
					new_ads: new_committed_ads_per_era,
					old_tier,
					new_tier,
				});
				Ok(())
			})
		}

		/// Root 强制暂停订阅 (调查期)
		#[pallet::call_index(12)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn force_suspend_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let sub = maybe.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
				sub.status = SubscriptionStatus::Suspended;
				Ok(())
			})?;
			Self::deposit_event(Event::SubscriptionForceSuspended { bot_id_hash });
			Ok(())
		}

		/// Operator 为 Bot 充值 Escrow (激励对齐: Operator 拿 90% 分成)
		#[pallet::call_index(13)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn operator_deposit_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroDepositAmount);
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
			ensure!(
				T::BotRegistry::bot_operator(&bot_id_hash) == Some(who.clone()),
				Error::<T>::NotBotOperator
			);

			T::Currency::reserve(&who, amount)?;
			SubscriptionEscrow::<T>::mutate(&bot_id_hash, |escrow| {
				*escrow = escrow.saturating_add(amount);
			});

			// 与 deposit_subscription 相同的重新激活逻辑
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

			Self::deposit_event(Event::OperatorDeposited { bot_id_hash, operator: who, amount });
			Ok(())
		}

		/// Root 恢复层级功能限制为硬编码默认值
		#[pallet::call_index(14)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn reset_tier_feature_gate(
			origin: OriginFor<T>,
			tier: SubscriptionTier,
		) -> DispatchResult {
			ensure_root(origin)?;
			TierFeatureGateOverrides::<T>::remove(&tier);
			Self::deposit_event(Event::TierFeatureGateReset { tier });
			Ok(())
		}

		/// Root 强制变更 Bot 订阅层级
		#[pallet::call_index(15)]
		#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
		pub fn force_change_tier(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
			new_tier: SubscriptionTier,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(new_tier.is_paid(), Error::<T>::CannotSubscribeFree);
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let sub = maybe.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
				let old_tier = sub.tier;
				sub.tier = new_tier;
				sub.fee_per_era = Self::tier_fee(&new_tier);
				Self::deposit_event(Event::TierForceChanged { bot_id_hash, old_tier, new_tier });
				Ok(())
			})
		}

		/// Bot Owner 主动暂停订阅 (不扣费, 不享受层级权益)
		#[pallet::call_index(16)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn pause_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let sub = maybe.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.owner == who, Error::<T>::NotBotOwner);
				ensure!(sub.status == SubscriptionStatus::Active, Error::<T>::SubscriptionNotActive);
				sub.status = SubscriptionStatus::Paused;
				Ok(())
			})?;
			Self::deposit_event(Event::SubscriptionPaused { bot_id_hash });
			Ok(())
		}

		/// Bot Owner 恢复已暂停的订阅
		#[pallet::call_index(17)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn resume_subscription(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Subscriptions::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let sub = maybe.as_mut().ok_or(Error::<T>::SubscriptionNotFound)?;
				ensure!(sub.owner == who, Error::<T>::NotBotOwner);
				ensure!(sub.status == SubscriptionStatus::Paused, Error::<T>::SubscriptionNotPaused);
				let escrow = SubscriptionEscrow::<T>::get(&bot_id_hash);
				if escrow >= sub.fee_per_era {
					sub.status = SubscriptionStatus::Active;
				} else {
					sub.status = SubscriptionStatus::PastDue;
				}
				Ok(())
			})?;
			Self::deposit_event(Event::SubscriptionResumed { bot_id_hash });
			Ok(())
		}

		/// 批量清理已取消的订阅和广告承诺记录
		#[pallet::call_index(18)]
		#[pallet::weight(Weight::from_parts(50_000_000, 10_000))]
		pub fn batch_cleanup(
			origin: OriginFor<T>,
			subscription_ids: sp_runtime::BoundedVec<BotIdHash, frame_support::traits::ConstU32<50>>,
			ad_commitment_ids: sp_runtime::BoundedVec<BotIdHash, frame_support::traits::ConstU32<50>>,
		) -> DispatchResult {
			ensure_signed(origin)?;
			ensure!(
				!subscription_ids.is_empty() || !ad_commitment_ids.is_empty(),
				Error::<T>::EmptyBatch
			);

			let mut subs_cleaned = 0u32;
			for bot_id_hash in subscription_ids.iter() {
				if let Some(sub) = Subscriptions::<T>::get(bot_id_hash) {
					if sub.status == SubscriptionStatus::Cancelled {
						Subscriptions::<T>::remove(bot_id_hash);
						SubscriptionEscrow::<T>::remove(bot_id_hash);
						subs_cleaned += 1;
					}
				}
			}

			let mut ads_cleaned = 0u32;
			for bot_id_hash in ad_commitment_ids.iter() {
				if let Some(record) = AdCommitments::<T>::get(bot_id_hash) {
					if record.status == AdCommitmentStatus::Cancelled {
						AdCommitments::<T>::remove(bot_id_hash);
						ads_cleaned += 1;
					}
				}
			}

			Self::deposit_event(Event::BatchCleaned {
				subscriptions_cleaned: subs_cleaned,
				ad_commitments_cleaned: ads_cleaned,
			});
			Ok(())
		}

		/// Root 动态调整层级费率
		///
		/// 新费率在下一次 Era 结算时生效.
		/// 已有订阅的 fee_per_era 不自动更新 (需 Owner change_tier 或下次新建时生效).
		#[pallet::call_index(19)]
		#[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
		pub fn update_tier_fee(
			origin: OriginFor<T>,
			tier: SubscriptionTier,
			new_fee: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(tier.is_paid(), Error::<T>::CannotSubscribeFree);
			TierFeeOverrides::<T>::insert(&tier, new_fee);
			Self::deposit_event(Event::TierFeeUpdated { tier, new_fee });
			Ok(())
		}

		/// Root 强制取消广告承诺
		#[pallet::call_index(20)]
		#[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
		pub fn force_cancel_ad_commitment(
			origin: OriginFor<T>,
			bot_id_hash: BotIdHash,
		) -> DispatchResult {
			ensure_root(origin)?;
			AdCommitments::<T>::try_mutate(&bot_id_hash, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::AdCommitmentNotFound)?;
				ensure!(record.status != AdCommitmentStatus::Cancelled, Error::<T>::AdCommitmentAlreadyCancelled);
				record.status = AdCommitmentStatus::Cancelled;
				Ok(())
			})?;
			Self::deposit_event(Event::AdCommitmentForceCancelled { bot_id_hash });
			Ok(())
		}
	}

	// ========================================================================
	// Internal Functions
	// ========================================================================

	impl<T: Config> Pallet<T> {
		/// 获取层级费用 (优先使用治理覆盖值, 否则回退 Config 常量)
		pub fn tier_fee(tier: &SubscriptionTier) -> BalanceOf<T> {
			if let Some(override_fee) = TierFeeOverrides::<T>::get(tier) {
				return override_fee;
			}
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
		/// Paused 状态不享受层级权益
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

		/// 查询 Bot 的功能限制 (优先使用治理覆盖值, 否则回退硬编码默认)
		pub fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
			let tier = Self::effective_tier(bot_id_hash);
			TierFeatureGateOverrides::<T>::get(&tier).unwrap_or_else(|| tier.feature_gate())
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

				if sub.status == SubscriptionStatus::Cancelled || sub.status == SubscriptionStatus::Paused {
					continue;
				}
				// Bot 封禁联动: 非活跃 Bot 跳过扣费
				if !T::BotRegistry::is_bot_active(&sub.bot_id_hash) {
					Self::deposit_event(Event::SettleSkippedInactiveBot { bot_id_hash: sub.bot_id_hash });
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
						// EscrowLow 预警: 剩余 < 2 Era 费用
						let remaining = SubscriptionEscrow::<T>::get(&sub.bot_id_hash);
						let two_eras = sub.fee_per_era.saturating_mul(2u32.into());
						if remaining < two_eras {
							Self::deposit_event(Event::EscrowLow {
								bot_id_hash: sub.bot_id_hash,
								remaining,
								fee_per_era: sub.fee_per_era,
							});
						}
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
		fn is_subscription_active(bot_id_hash: &BotIdHash) -> bool {
			Subscriptions::<T>::get(bot_id_hash).map_or(false, |sub| {
				matches!(sub.status, SubscriptionStatus::Active | SubscriptionStatus::PastDue | SubscriptionStatus::Paused)
			})
		}
		fn subscription_status(bot_id_hash: &BotIdHash) -> Option<SubscriptionStatus> {
			Subscriptions::<T>::get(bot_id_hash).map(|sub| sub.status)
		}
	}

	// ========================================================================
	// SubscriptionSettler 实现
	// ========================================================================

	impl<T: Config> SubscriptionSettler for Pallet<T> {
		fn settle_era() -> EraSettlementResult {
			let (income, treasury) = Self::settle_era_subscriptions();
			Self::settle_ad_commitments();
			let income_u128: u128 = income.unique_saturated_into();
			let treasury_u128: u128 = treasury.unique_saturated_into();
			EraSettlementResult {
				total_income: income_u128,
				node_share: income_u128.saturating_sub(treasury_u128),
				treasury_share: treasury_u128,
			}
		}
	}
}
