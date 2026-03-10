#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Ads Entity — Entity DApp 广告适配层
//!
//! ## 功能
//! - Entity 展示量验证 (实现 `DeliveryVerifier`)
//! - Entity/Shop 管理员映射 (实现 `PlacementAdminProvider`)
//! - 二方收入分配: Entity Owner / 平台 (实现 `RevenueDistributor`)
//! - Entity 广告注册 (opt-in) + Shop 级广告位管理
//! - 每日展示量上限 (按注册的广告计划配额)
//! - Entity 广告分成百分比治理
//!
//! ## 设计
//! 本 pallet 不包含 Campaign CRUD 等核心广告逻辑 (由 `pallet-ads-core` 提供)。
//! 仅实现 Entity DApp 领域特定的适配 trait，并提供额外的 Entity 专属 extrinsic
//! (注册广告位, 设置分成, 设置展示量上限, Shop 级广告位管理等)。
//!
//! ## PlacementId 映射
//! Entity 使用 `blake2_256(b"entity-ad:" ++ entity_id.to_le_bytes())` 作为 PlacementId。
//! Shop 级使用 `blake2_256(b"shop-ad:" ++ shop_id.to_le_bytes())` 作为 PlacementId。

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_ads_primitives::*;
use pallet_entity_common::{EntityProvider, ShopProvider};
use sp_runtime::traits::{Saturating, Zero};

/// Balance 类型别名
type BalanceOf<T> =
	<<T as Config>::Currency as frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;

// ============================================================================
// PlacementId 编码辅助
// ============================================================================

/// Entity 级 PlacementId: blake2_256(b"entity-ad:" ++ entity_id.to_le_bytes())
pub fn entity_placement_id(entity_id: u64) -> PlacementId {
	let mut buf = [0u8; 18]; // "entity-ad:" = 10 bytes + u64 = 8 bytes
	buf[..10].copy_from_slice(b"entity-ad:");
	buf[10..].copy_from_slice(&entity_id.to_le_bytes());
	sp_core::hashing::blake2_256(&buf)
}

/// Shop 级 PlacementId: blake2_256(b"shop-ad:" ++ shop_id.to_le_bytes())
pub fn shop_placement_id(shop_id: u64) -> PlacementId {
	let mut buf = [0u8; 16]; // "shop-ad:" = 8 bytes + u64 = 8 bytes
	buf[..8].copy_from_slice(b"shop-ad:");
	buf[8..].copy_from_slice(&shop_id.to_le_bytes());
	sp_core::hashing::blake2_256(&buf)
}

/// 广告位级别
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, RuntimeDebug, PartialEq, Eq,
	scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum PlacementLevel {
	/// Entity 级 (整个 Entity 的所有 Shop 共享)
	Entity,
	/// Shop 级 (仅限单个 Shop)
	Shop,
}

impl Default for PlacementLevel {
	fn default() -> Self {
		Self::Entity
	}
}

/// 广告位注册信息
#[derive(
	Encode, Decode, codec::DecodeWithMemTracking, Clone, RuntimeDebug, PartialEq, Eq,
	scale_info::TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct AdPlacementInfo<T: Config> {
	/// 广告位所属 entity_id
	pub entity_id: u64,
	/// 广告位所属 shop_id (0 = Entity 级)
	pub shop_id: u64,
	/// 广告位级别
	pub level: PlacementLevel,
	/// 每日展示量上限 (0 = 无限制)
	pub daily_impression_cap: u32,
	/// 每日点击量上限 (0 = 无限制)
	pub daily_click_cap: u32,
	/// 已注册者 (Entity owner 或 Shop manager)
	pub registered_by: T::AccountId,
	/// 注册区块号
	pub registered_at: BlockNumberFor<T>,
	/// 是否活跃
	pub active: bool,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use pallet_trading_common::DepositCalculator;
	use frame_support::traits::ReservableCurrency;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		type WeightInfo: WeightInfo;
		type Currency: ReservableCurrency<Self::AccountId>;

		/// Entity 查询 (活跃状态 + 所有者 + 管理员)
		type EntityProvider: EntityProvider<Self::AccountId>;

		/// Shop 查询 (活跃状态 + 所属 Entity + 管理员)
		type ShopProvider: ShopProvider<Self::AccountId>;

		/// 平台国库账户
		type TreasuryAccount: Get<Self::AccountId>;

		/// Benchmark helper for setting up external state.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self::AccountId>;

		/// 平台广告分成 (基点, 默认 2000 = 20%)
		#[pallet::constant]
		type PlatformAdShareBps: Get<u16>;

		/// 注册广告位所需最低保证金（NEX 兜底值）
		#[pallet::constant]
		type AdPlacementDeposit: Get<BalanceOf<Self>>;

		/// 广告位押金 USDT 目标值（精度 10^6，如 1_000_000 = 1 USDT）
		#[pallet::constant]
		type AdPlacementDepositUsd: Get<u64>;

		/// 动态 USDT→NEX 换算器
		type DepositCalculator: pallet_trading_common::DepositCalculator<BalanceOf<Self>>;

		/// 每个 Entity 最大广告位数
		#[pallet::constant]
		type MaxPlacementsPerEntity: Get<u32>;

		/// 默认每日展示量上限
		#[pallet::constant]
		type DefaultDailyImpressionCap: Get<u32>;

		/// 每日区块数 (用于展示量重置周期, 默认 14400 ≈ 24h @ 6s/block)
		#[pallet::constant]
		type BlocksPerDay: Get<u32>;
	}

	// ========================================================================
	// Storage — Entity 广告专属
	// ========================================================================

	/// 已注册的广告位 (PlacementId → Info)
	#[pallet::storage]
	pub type RegisteredPlacements<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, AdPlacementInfo<T>>;

	/// Entity 下注册的广告位 ID 列表
	#[pallet::storage]
	pub type EntityPlacementIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		u64,  // entity_id
		BoundedVec<PlacementId, T::MaxPlacementsPerEntity>,
		ValueQuery,
	>;

	/// 广告位今日展示量计数
	#[pallet::storage]
	pub type DailyImpressions<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32, ValueQuery>;

	/// 广告位累计展示量
	#[pallet::storage]
	pub type TotalImpressions<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u64, ValueQuery>;

	/// 广告位保证金存入额
	#[pallet::storage]
	pub type PlacementDeposits<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, BalanceOf<T>, ValueQuery>;

	/// Entity 自定义分成比例 (基点, 0 = 使用默认)
	/// Entity Owner 拿 (10000 - PlatformAdShareBps - entity_custom)
	/// 默认: Entity Owner 拿 10000 - PlatformAdShareBps
	#[pallet::storage]
	pub type EntityAdShareBps<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, u16, ValueQuery>;

	/// 被禁止投放广告的 Entity
	#[pallet::storage]
	pub type BannedEntities<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, bool, ValueQuery>;

	/// 展示量计数器最后重置区块 (用于每日重置)
	#[pallet::storage]
	pub type ImpressionResetBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, BlockNumberFor<T>, ValueQuery>;

	/// 广告位今日点击量计数 (CPC)
	#[pallet::storage]
	pub type DailyClicks<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u32, ValueQuery>;

	/// 广告位累计点击量 (CPC)
	#[pallet::storage]
	pub type TotalClicks<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, u64, ValueQuery>;

	/// 点击量计数器最后重置区块 (用于每日重置)
	#[pallet::storage]
	pub type ClickResetBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, PlacementId, BlockNumberFor<T>, ValueQuery>;

	// ========================================================================
	// Events
	// ========================================================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// 广告位已注册
		PlacementRegistered {
			entity_id: u64,
			shop_id: u64,
			placement_id: PlacementId,
			level: PlacementLevel,
			deposit: BalanceOf<T>,
		},
		/// 广告位已注销
		PlacementDeregistered {
			placement_id: PlacementId,
			deposit_returned: BalanceOf<T>,
		},
		/// 广告位已激活/禁用
		PlacementStatusUpdated {
			placement_id: PlacementId,
			active: bool,
		},
		/// 每日展示量上限已更新
		ImpressionCapUpdated {
			placement_id: PlacementId,
			daily_cap: u32,
		},
		/// Entity 广告分成比例已更新
		EntityShareUpdated {
			entity_id: u64,
			share_bps: u16,
		},
		/// Entity 被禁止广告
		EntityBanned {
			entity_id: u64,
		},
		/// Entity 禁令解除
		EntityUnbanned {
			entity_id: u64,
		},
		/// 每日点击量上限已更新
		ClickCapUpdated {
			placement_id: PlacementId,
			daily_cap: u32,
		},
	}

	// ========================================================================
	// Errors
	// ========================================================================

	#[pallet::error]
	pub enum Error<T> {
		/// Entity 不存在
		EntityNotFound,
		/// Entity 未激活
		EntityNotActive,
		/// Shop 不存在
		ShopNotFound,
		/// Shop 未激活
		ShopNotActive,
		/// Shop 不属于该 Entity
		ShopEntityMismatch,
		/// 调用者不是 Entity Owner 或管理员
		NotEntityAdmin,
		/// 调用者不是 Shop 管理员
		NotShopManager,
		/// 广告位已注册
		PlacementAlreadyRegistered,
		/// 广告位未注册
		PlacementNotRegistered,
		/// 广告位未激活
		PlacementNotActive,
		/// Entity 广告位数量达上限
		MaxPlacementsReached,
		/// 分成百分比无效 (超过 10000 基点)
		InvalidShareBps,
		/// Entity 已被禁止
		EntityBanned,
		/// 每日展示量已达上限
		DailyImpressionCapReached,
		/// Entity 已被禁止 (重复禁止)
		EntityAlreadyBanned,
		/// Entity 未被禁止 (无需解禁)
		EntityNotBanned,
		/// 广告位激活状态未变更
		PlacementStatusUnchanged,
		/// 每日展示量上限未变更
		ImpressionCapUnchanged,
		/// 每日点击量已达上限
		DailyClickCapReached,
		/// 每日点击量上限未变更
		ClickCapUnchanged,
		/// 实体已被全局锁定
		EntityLocked,
	}

	// ========================================================================
	// Extrinsics — Entity 广告专属
	// ========================================================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// 注册 Entity 级广告位 (Entity Owner / Admin)
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::register_entity_placement())]
		pub fn register_entity_placement(
			origin: OriginFor<T>,
			entity_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// 验证 Entity 状态
			ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
			ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
			ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
			ensure!(
				T::EntityProvider::entity_owner(entity_id) == Some(who.clone()) ||
				T::EntityProvider::is_entity_admin(entity_id, &who, pallet_entity_common::AdminPermission::ADS_MANAGE),
				Error::<T>::NotEntityAdmin
			);
			ensure!(!BannedEntities::<T>::get(entity_id), Error::<T>::EntityBanned);

			let placement_id = entity_placement_id(entity_id);
			ensure!(
				!RegisteredPlacements::<T>::contains_key(&placement_id),
				Error::<T>::PlacementAlreadyRegistered
			);

			// 广告位数量限制
			let mut ids = EntityPlacementIds::<T>::get(entity_id);
			ensure!(
				(ids.len() as u32) < T::MaxPlacementsPerEntity::get(),
				Error::<T>::MaxPlacementsReached
			);

			// 保证金
			let deposit = Self::calculate_placement_deposit();
			if !deposit.is_zero() {
				T::Currency::reserve(&who, deposit)?;
			}

			let info = AdPlacementInfo {
				entity_id,
				shop_id: 0,
				level: PlacementLevel::Entity,
				daily_impression_cap: T::DefaultDailyImpressionCap::get(),
				daily_click_cap: 0,
				registered_by: who,
				registered_at: frame_system::Pallet::<T>::block_number(),
				active: true,
			};

			RegisteredPlacements::<T>::insert(&placement_id, info);
			PlacementDeposits::<T>::insert(&placement_id, deposit);
			ids.try_push(placement_id).map_err(|_| Error::<T>::MaxPlacementsReached)?;
			EntityPlacementIds::<T>::insert(entity_id, ids);

			Self::deposit_event(Event::PlacementRegistered {
				entity_id,
				shop_id: 0,
				placement_id,
				level: PlacementLevel::Entity,
				deposit,
			});
			Ok(())
		}

		/// 注册 Shop 级广告位 (Entity Owner / Shop Manager)
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::register_shop_placement())]
		pub fn register_shop_placement(
			origin: OriginFor<T>,
			entity_id: u64,
			shop_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// 验证 Entity + Shop 状态
			ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
			ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
			ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
			ensure!(T::ShopProvider::shop_exists(shop_id), Error::<T>::ShopNotFound);
			ensure!(T::ShopProvider::is_shop_active(shop_id), Error::<T>::ShopNotActive);
			ensure!(
				T::ShopProvider::shop_entity_id(shop_id) == Some(entity_id),
				Error::<T>::ShopEntityMismatch
			);
			ensure!(
				T::EntityProvider::entity_owner(entity_id) == Some(who.clone()) ||
				T::EntityProvider::is_entity_admin(entity_id, &who, pallet_entity_common::AdminPermission::ADS_MANAGE) ||
				T::ShopProvider::is_shop_manager(shop_id, &who),
				Error::<T>::NotShopManager
			);
			ensure!(!BannedEntities::<T>::get(entity_id), Error::<T>::EntityBanned);

			let placement_id = shop_placement_id(shop_id);
			ensure!(
				!RegisteredPlacements::<T>::contains_key(&placement_id),
				Error::<T>::PlacementAlreadyRegistered
			);

			let mut ids = EntityPlacementIds::<T>::get(entity_id);
			ensure!(
				(ids.len() as u32) < T::MaxPlacementsPerEntity::get(),
				Error::<T>::MaxPlacementsReached
			);

			let deposit = Self::calculate_placement_deposit();
			if !deposit.is_zero() {
				T::Currency::reserve(&who, deposit)?;
			}

			let info = AdPlacementInfo {
				entity_id,
				shop_id,
				level: PlacementLevel::Shop,
				daily_impression_cap: T::DefaultDailyImpressionCap::get(),
				daily_click_cap: 0,
				registered_by: who,
				registered_at: frame_system::Pallet::<T>::block_number(),
				active: true,
			};

			RegisteredPlacements::<T>::insert(&placement_id, info);
			PlacementDeposits::<T>::insert(&placement_id, deposit);
			ids.try_push(placement_id).map_err(|_| Error::<T>::MaxPlacementsReached)?;
			EntityPlacementIds::<T>::insert(entity_id, ids);

			Self::deposit_event(Event::PlacementRegistered {
				entity_id,
				shop_id,
				placement_id,
				level: PlacementLevel::Shop,
				deposit,
			});
			Ok(())
		}

		/// 注销广告位并退还保证金
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::deregister_placement())]
		pub fn deregister_placement(
			origin: OriginFor<T>,
			placement_id: PlacementId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let info = RegisteredPlacements::<T>::get(&placement_id)
				.ok_or(Error::<T>::PlacementNotRegistered)?;

			// 权限: Entity owner/admin 或注册者
			ensure!(!T::EntityProvider::is_entity_locked(info.entity_id), Error::<T>::EntityLocked);
			ensure!(
				T::EntityProvider::entity_owner(info.entity_id) == Some(who.clone()) ||
				T::EntityProvider::is_entity_admin(info.entity_id, &who, pallet_entity_common::AdminPermission::ADS_MANAGE) ||
				info.registered_by == who,
				Error::<T>::NotEntityAdmin
			);

			// 退还保证金
			let deposit = PlacementDeposits::<T>::take(&placement_id);
			if !deposit.is_zero() {
				T::Currency::unreserve(&info.registered_by, deposit);
			}

			// 清理存储
			RegisteredPlacements::<T>::remove(&placement_id);
			DailyImpressions::<T>::remove(&placement_id);
			TotalImpressions::<T>::remove(&placement_id);
			ImpressionResetBlock::<T>::remove(&placement_id);
			DailyClicks::<T>::remove(&placement_id);
			TotalClicks::<T>::remove(&placement_id);
			ClickResetBlock::<T>::remove(&placement_id);

			EntityPlacementIds::<T>::mutate(info.entity_id, |ids| {
				ids.retain(|id| id != &placement_id);
			});

			Self::deposit_event(Event::PlacementDeregistered {
				placement_id,
				deposit_returned: deposit,
			});
			Ok(())
		}

		/// 激活/禁用广告位
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::set_placement_active())]
		pub fn set_placement_active(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			active: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			RegisteredPlacements::<T>::try_mutate(&placement_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::PlacementNotRegistered)?;
				ensure!(!T::EntityProvider::is_entity_locked(info.entity_id), Error::<T>::EntityLocked);
				ensure!(
					T::EntityProvider::entity_owner(info.entity_id) == Some(who.clone()) ||
					T::EntityProvider::is_entity_admin(info.entity_id, &who, pallet_entity_common::AdminPermission::ADS_MANAGE) ||
					info.registered_by == who,
					Error::<T>::NotEntityAdmin
				);

				// L3: 状态未变更时跳过，避免冗余事件
				ensure!(info.active != active, Error::<T>::PlacementStatusUnchanged);

				info.active = active;

				Self::deposit_event(Event::PlacementStatusUpdated {
					placement_id,
					active,
				});
				Ok(())
			})
		}

		/// 设置广告位每日展示量上限
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_impression_cap())]
		pub fn set_impression_cap(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			daily_cap: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			RegisteredPlacements::<T>::try_mutate(&placement_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::PlacementNotRegistered)?;
				ensure!(!T::EntityProvider::is_entity_locked(info.entity_id), Error::<T>::EntityLocked);
				ensure!(
					T::EntityProvider::entity_owner(info.entity_id) == Some(who.clone()) ||
					T::EntityProvider::is_entity_admin(info.entity_id, &who, pallet_entity_common::AdminPermission::ADS_MANAGE) ||
					info.registered_by == who,
					Error::<T>::NotEntityAdmin
				);

				// L1-R2: 值未变更时跳过，避免冗余事件
				ensure!(info.daily_impression_cap != daily_cap, Error::<T>::ImpressionCapUnchanged);

				info.daily_impression_cap = daily_cap;

				Self::deposit_event(Event::ImpressionCapUpdated {
					placement_id,
					daily_cap,
				});
				Ok(())
			})
		}

		/// 设置 Entity 自定义广告分成比例 (Entity Owner, 基点)
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::set_entity_ad_share())]
		pub fn set_entity_ad_share(
			origin: OriginFor<T>,
			entity_id: u64,
			share_bps: u16,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
			ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
			ensure!(
				T::EntityProvider::entity_owner(entity_id) == Some(who),
				Error::<T>::NotEntityAdmin
			);
			ensure!(
				share_bps <= 10_000u16.saturating_sub(T::PlatformAdShareBps::get()),
				Error::<T>::InvalidShareBps
			);

			EntityAdShareBps::<T>::insert(entity_id, share_bps);

			Self::deposit_event(Event::EntityShareUpdated {
				entity_id,
				share_bps,
			});
			Ok(())
		}

		/// 禁止 Entity 参与广告 (Root)
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::ban_entity())]
		pub fn ban_entity(
			origin: OriginFor<T>,
			entity_id: u64,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
			ensure!(!BannedEntities::<T>::get(entity_id), Error::<T>::EntityAlreadyBanned);
			BannedEntities::<T>::insert(entity_id, true);
			Self::deposit_event(Event::EntityBanned { entity_id });
			Ok(())
		}

		/// 解除 Entity 广告禁令 (Root)
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::unban_entity())]
		pub fn unban_entity(
			origin: OriginFor<T>,
			entity_id: u64,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
			ensure!(BannedEntities::<T>::get(entity_id), Error::<T>::EntityNotBanned);
			BannedEntities::<T>::remove(entity_id);
			Self::deposit_event(Event::EntityUnbanned { entity_id });
			Ok(())
		}

		/// 设置广告位每日点击量上限 (CPC)
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::set_click_cap())]
		pub fn set_click_cap(
			origin: OriginFor<T>,
			placement_id: PlacementId,
			daily_cap: u32,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			RegisteredPlacements::<T>::try_mutate(&placement_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::PlacementNotRegistered)?;
				ensure!(!T::EntityProvider::is_entity_locked(info.entity_id), Error::<T>::EntityLocked);
				ensure!(
					T::EntityProvider::entity_owner(info.entity_id) == Some(who.clone()) ||
					T::EntityProvider::is_entity_admin(info.entity_id, &who, pallet_entity_common::AdminPermission::ADS_MANAGE) ||
					info.registered_by == who,
					Error::<T>::NotEntityAdmin
				);

				ensure!(info.daily_click_cap != daily_cap, Error::<T>::ClickCapUnchanged);

				info.daily_click_cap = daily_cap;

				Self::deposit_event(Event::ClickCapUpdated {
					placement_id,
					daily_cap,
				});
				Ok(())
			})
		}
	}

	// ========================================================================
	// Helper functions
	// ========================================================================

	/// 动态押金计算
	impl<T: Config> Pallet<T> {
		/// 计算广告位押金（USDT 目标 → NEX 换算，无价格时回退兜底值）
		fn calculate_placement_deposit() -> BalanceOf<T> {
			T::DepositCalculator::calculate_deposit(
				T::AdPlacementDepositUsd::get(),
				T::AdPlacementDeposit::get(),
			)
		}
	}

	impl<T: Config> Pallet<T> {
		/// 百分比计算 (基点)
		pub fn bps_of(amount: BalanceOf<T>, bps: u16) -> BalanceOf<T> {
			let bps_balance: BalanceOf<T> = (bps as u32).into();
			let ten_k: BalanceOf<T> = 10_000u32.into();
			amount.saturating_mul(bps_balance) / ten_k
		}

		/// Entity 有效分成比例: Entity Owner 拿 (10000 - PlatformAdShareBps) 基点
		/// 如果 Entity 自定义了 EntityAdShareBps，则 Entity Owner 拿 custom 基点
		pub fn effective_entity_share_bps(entity_id: u64) -> u16 {
			let custom = EntityAdShareBps::<T>::get(entity_id);
			if custom > 0 {
				custom
			} else {
				10_000u16.saturating_sub(T::PlatformAdShareBps::get())
			}
		}

		/// 检查并重置每日展示量计数器 (周期由 Config::BlocksPerDay 定义)
		pub fn check_and_reset_daily(placement_id: &PlacementId) {
			let now = frame_system::Pallet::<T>::block_number();
			let last_reset = ImpressionResetBlock::<T>::get(placement_id);
			let day_blocks: BlockNumberFor<T> = T::BlocksPerDay::get().into();

			if now.saturating_sub(last_reset) >= day_blocks {
				DailyImpressions::<T>::insert(placement_id, 0u32);
				ImpressionResetBlock::<T>::insert(placement_id, now);
			}
		}

		/// 查找 PlacementId 对应的 entity_id
		pub fn placement_entity_id(placement_id: &PlacementId) -> Option<u64> {
			RegisteredPlacements::<T>::get(placement_id).map(|info| info.entity_id)
		}

		/// 检查并重置每日点击量计数器 (周期由 Config::BlocksPerDay 定义)
		pub fn check_and_reset_daily_clicks(placement_id: &PlacementId) {
			let now = frame_system::Pallet::<T>::block_number();
			let last_reset = ClickResetBlock::<T>::get(placement_id);
			let day_blocks: BlockNumberFor<T> = T::BlocksPerDay::get().into();

			if now.saturating_sub(last_reset) >= day_blocks {
				DailyClicks::<T>::insert(placement_id, 0u32);
				ClickResetBlock::<T>::insert(placement_id, now);
			}
		}
	}

	// ========================================================================
	// DeliveryVerifier 实现 — Entity 展示量验证
	// ========================================================================

	impl<T: Config> DeliveryVerifier<T::AccountId> for Pallet<T> {
		fn verify_and_cap_audience(
			who: &T::AccountId,
			placement_id: &PlacementId,
			audience_size: u32,
			_node_id: Option<[u8; 32]>,
		) -> Result<u32, sp_runtime::DispatchError> {
			// 1. 广告位必须已注册且激活
			let info = RegisteredPlacements::<T>::get(placement_id)
				.ok_or(Error::<T>::PlacementNotRegistered)?;
			ensure!(info.active, Error::<T>::PlacementNotActive);

			// 2. Entity 必须活跃且未被禁止
			ensure!(
				T::EntityProvider::is_entity_active(info.entity_id),
				Error::<T>::EntityNotActive
			);
			ensure!(!BannedEntities::<T>::get(info.entity_id), Error::<T>::EntityBanned);

			// 3. 调用者权限: Entity owner/admin 或 shop manager
			let is_authorized = T::EntityProvider::entity_owner(info.entity_id) == Some(who.clone())
				|| T::EntityProvider::is_entity_admin(info.entity_id, who, pallet_entity_common::AdminPermission::ADS_MANAGE)
				|| (info.shop_id > 0 && T::ShopProvider::is_shop_manager(info.shop_id, who));
			ensure!(is_authorized, Error::<T>::NotEntityAdmin);

			// 4. 每日展示量上限检查与重置
			Self::check_and_reset_daily(placement_id);

			let current = DailyImpressions::<T>::get(placement_id);
			let cap = info.daily_impression_cap;
			let effective = if cap > 0 {
				let remaining = cap.saturating_sub(current);
				if remaining == 0 {
					return Err(Error::<T>::DailyImpressionCapReached.into());
				}
				core::cmp::min(audience_size, remaining)
			} else {
				audience_size
			};

			// 5. 更新展示量计数
			DailyImpressions::<T>::mutate(placement_id, |c| {
				*c = c.saturating_add(effective);
			});
			TotalImpressions::<T>::mutate(placement_id, |c| {
				*c = c.saturating_add(effective as u64);
			});

			Ok(effective)
		}
	}

	// ========================================================================
	// ClickVerifier 实现 — Entity 点击量验证 (CPC)
	// ========================================================================

	impl<T: Config> ClickVerifier<T::AccountId> for Pallet<T> {
		fn verify_and_cap_clicks(
			who: &T::AccountId,
			placement_id: &PlacementId,
			click_count: u32,
			_verified_clicks: u32,
		) -> Result<u32, sp_runtime::DispatchError> {
			// 1. 广告位必须已注册且激活
			let info = RegisteredPlacements::<T>::get(placement_id)
				.ok_or(Error::<T>::PlacementNotRegistered)?;
			ensure!(info.active, Error::<T>::PlacementNotActive);

			// 2. Entity 必须活跃且未被禁止
			ensure!(
				T::EntityProvider::is_entity_active(info.entity_id),
				Error::<T>::EntityNotActive
			);
			ensure!(!BannedEntities::<T>::get(info.entity_id), Error::<T>::EntityBanned);

			// 3. 调用者权限: Entity owner/admin 或 shop manager
			let is_authorized = T::EntityProvider::entity_owner(info.entity_id) == Some(who.clone())
				|| T::EntityProvider::is_entity_admin(info.entity_id, who, pallet_entity_common::AdminPermission::ADS_MANAGE)
				|| (info.shop_id > 0 && T::ShopProvider::is_shop_manager(info.shop_id, who));
			ensure!(is_authorized, Error::<T>::NotEntityAdmin);

			// 4. 每日点击量上限检查与重置
			Self::check_and_reset_daily_clicks(placement_id);

			let current = DailyClicks::<T>::get(placement_id);
			let cap = info.daily_click_cap;
			let effective = if cap > 0 {
				let remaining = cap.saturating_sub(current);
				if remaining == 0 {
					return Err(Error::<T>::DailyClickCapReached.into());
				}
				core::cmp::min(click_count, remaining)
			} else {
				click_count
			};

			// 5. 更新点击量计数
			DailyClicks::<T>::mutate(placement_id, |c| {
				*c = c.saturating_add(effective);
			});
			TotalClicks::<T>::mutate(placement_id, |c| {
				*c = c.saturating_add(effective as u64);
			});

			Ok(effective)
		}
	}

	// ========================================================================
	// PlacementAdminProvider 实现
	// ========================================================================

	impl<T: Config> PlacementAdminProvider<T::AccountId> for Pallet<T> {
		fn placement_admin(placement_id: &PlacementId) -> Option<T::AccountId> {
			RegisteredPlacements::<T>::get(placement_id).and_then(|info| {
				if info.shop_id > 0 {
					// Shop 级: 优先 shop owner, 回退到 entity owner
					T::ShopProvider::shop_owner(info.shop_id)
						.or_else(|| T::EntityProvider::entity_owner(info.entity_id))
				} else {
					T::EntityProvider::entity_owner(info.entity_id)
				}
			})
		}

		fn is_placement_banned(placement_id: &PlacementId) -> bool {
			RegisteredPlacements::<T>::get(placement_id)
				.map(|info| BannedEntities::<T>::get(info.entity_id))
				.unwrap_or(false)
		}

		fn placement_status(placement_id: &PlacementId) -> PlacementStatus {
			match RegisteredPlacements::<T>::get(placement_id) {
				None => PlacementStatus::Unknown,
				Some(info) => {
					if BannedEntities::<T>::get(info.entity_id) {
						PlacementStatus::Banned
					} else if !info.active {
						PlacementStatus::Paused
					} else {
						PlacementStatus::Active
					}
				}
			}
		}
	}

	// ========================================================================
	// RevenueDistributor 实现 — 二方分成
	// ========================================================================

	impl<T: Config> RevenueDistributor<T::AccountId, BalanceOf<T>> for Pallet<T> {
		fn distribute(
			placement_id: &PlacementId,
			total_cost: BalanceOf<T>,
			_advertiser: &T::AccountId,
		) -> Result<RevenueBreakdown<BalanceOf<T>>, sp_runtime::DispatchError> {
			let entity_id = Self::placement_entity_id(placement_id)
				.ok_or(Error::<T>::PlacementNotRegistered)?;
			let entity_share_bps = Self::effective_entity_share_bps(entity_id);
			let entity_share = Self::bps_of(total_cost, entity_share_bps);

			// 平台份额 = total_cost - entity_share (自动包含余数)
			// 转入国库由 ads-core 的 settle_era_ads 处理
			let platform_share = total_cost.saturating_sub(entity_share);

			Ok(RevenueBreakdown {
				placement_share: entity_share,
				node_share: Zero::zero(),
				platform_share,
			})
		}
	}
}
