#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet GroupRobot Subscription — 订阅管理 + Escrow + 层级费用结算
//!
//! 从 consensus pallet 拆分而来, 负责:
//! - 订阅 CRUD (subscribe/deposit/cancel/change_tier)
//! - Escrow 预存余额管理
//! - Era 订阅费结算 (游标分页)
//! - 实现 SubscriptionProvider + SubscriptionSettler trait

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
	pub paid_until_era: u64,
	pub status: SubscriptionStatus,
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
		/// 国库账户 (订阅费转入)
		type TreasuryAccount: Get<Self::AccountId>;
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

			let fee = Self::tier_fee(&tier);
			ensure!(deposit >= fee, Error::<T>::InsufficientDeposit);

			T::Currency::reserve(&who, deposit)?;
			SubscriptionEscrow::<T>::insert(&bot_id_hash, deposit);

			let now = frame_system::Pallet::<T>::block_number();
			let current_era = T::CurrentEraProvider::get();
			let sub = SubscriptionRecord::<T> {
				owner: who.clone(),
				bot_id_hash,
				tier,
				fee_per_era: fee,
				started_at: now,
				paid_until_era: current_era.saturating_add(1),
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
			let sub = Subscriptions::<T>::get(&bot_id_hash).ok_or(Error::<T>::SubscriptionNotFound)?;
			ensure!(sub.status != SubscriptionStatus::Cancelled, Error::<T>::SubscriptionAlreadyCancelled);
			ensure!(sub.owner == who, Error::<T>::NotSubscriptionOwner);

			T::Currency::reserve(&who, amount)?;
			SubscriptionEscrow::<T>::mutate(&bot_id_hash, |escrow| {
				*escrow = escrow.saturating_add(amount);
			});

			// 如果是 PastDue/Suspended，重新激活
			if sub.status == SubscriptionStatus::PastDue || sub.status == SubscriptionStatus::Suspended {
				Subscriptions::<T>::mutate(&bot_id_hash, |maybe_sub| {
					if let Some(s) = maybe_sub {
						s.status = SubscriptionStatus::Active;
					}
				});
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
				let _ = T::Currency::transfer(
					&who,
					&treasury,
					prorated_fee,
					ExistenceRequirement::AllowDeath,
				);
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
				ensure!(sub.tier != new_tier, Error::<T>::SameTier);

				let old_tier = sub.tier;
				sub.tier = new_tier;
				sub.fee_per_era = Self::tier_fee(&new_tier);

				Self::deposit_event(Event::TierChanged { bot_id_hash, old_tier, new_tier });
				Ok(())
			})
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

		/// 查询 Bot 的有效层级
		pub fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
			match Subscriptions::<T>::get(bot_id_hash) {
				Some(sub) => match sub.status {
					SubscriptionStatus::Active => sub.tier,
					SubscriptionStatus::PastDue => sub.tier,
					_ => SubscriptionTier::Free,
				},
				None => SubscriptionTier::Free,
			}
		}

		/// 查询 Bot 的功能限制
		pub fn effective_feature_gate(bot_id_hash: &BotIdHash) -> TierFeatureGate {
			Self::effective_tier(bot_id_hash).feature_gate()
		}

		/// Era 订阅费结算 (游标分页)
		///
		/// 返回本次结算收取的总收入
		pub fn settle_era_subscriptions() -> BalanceOf<T> {
			let treasury = T::TreasuryAccount::get();
			let max_settle = T::MaxSubscriptionSettlePerEra::get();
			let mut subscription_income = BalanceOf::<T>::zero();
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
					last_key = Some(bot_hash);
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
					SubscriptionEscrow::<T>::mutate(&sub.bot_id_hash, |e| {
						*e = e.saturating_sub(sub.fee_per_era);
					});
					T::Currency::unreserve(&sub.owner, sub.fee_per_era);
					let _ = T::Currency::transfer(
						&sub.owner,
						&treasury,
						sub.fee_per_era,
						ExistenceRequirement::AllowDeath,
					);
					subscription_income = subscription_income.saturating_add(sub.fee_per_era);

					Self::deposit_event(Event::SubscriptionFeeCollected {
						bot_id_hash: sub.bot_id_hash,
						amount: sub.fee_per_era,
					});
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

			subscription_income
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
		fn settle_era() -> u128 {
			let income = Self::settle_era_subscriptions();
			income.unique_saturated_into()
		}
	}
}
