#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Ads Router — 广告适配层路由
//!
//! ## 功能
//! 根据 PlacementId 自动分发到 Entity 或 GroupRobot 适配器。
//! 实现 `DeliveryVerifier`、`PlacementAdminProvider`、`RevenueDistributor` 三大 trait。
//!
//! ## 路由规则
//! - 如果 PlacementId 已在 `pallet-ads-entity::RegisteredPlacements` 中注册 → Entity 路径
//! - 否则 → GroupRobot 路径 (默认回退)
//!
//! ## 设计
//! 纯路由层，不包含任何 Storage 或 Extrinsic。
//! Entity 和 GroupRobot 两个适配层各自保持独立运营。

use frame_support::traits::Currency;
use pallet_ads_primitives::*;
use sp_runtime::DispatchError;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarking;

/// Entity 适配层的 Balance 类型
type BalanceOf<T> = <<T as pallet_ads_entity::Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

/// 路由适配器 — 泛型参数 T 同时满足 Entity 和 GroupRobot Config
pub struct AdsRouter<T>(core::marker::PhantomData<T>);

/// 检查 PlacementId 是否属于 Entity 路径
fn is_entity_placement<T: pallet_ads_entity::Config>(placement_id: &PlacementId) -> bool {
	pallet_ads_entity::RegisteredPlacements::<T>::contains_key(placement_id)
}

// ============================================================================
// DeliveryVerifier 路由
// ============================================================================

impl<T> DeliveryVerifier<T::AccountId> for AdsRouter<T>
where
	T: pallet_ads_entity::Config + pallet_ads_grouprobot::Config,
{
	fn verify_and_cap_audience(
		who: &T::AccountId,
		placement_id: &PlacementId,
		audience_size: u32,
		node_id: Option<[u8; 32]>,
	) -> Result<u32, DispatchError> {
		if is_entity_placement::<T>(placement_id) {
			<pallet_ads_entity::Pallet<T> as DeliveryVerifier<T::AccountId>>::verify_and_cap_audience(
				who, placement_id, audience_size, node_id,
			)
		} else {
			<pallet_ads_grouprobot::Pallet<T> as DeliveryVerifier<T::AccountId>>::verify_and_cap_audience(
				who, placement_id, audience_size, node_id,
			)
		}
	}
}

// ============================================================================
// ClickVerifier 路由
// ============================================================================

impl<T> ClickVerifier<T::AccountId> for AdsRouter<T>
where
	T: pallet_ads_entity::Config + pallet_ads_grouprobot::Config,
{
	fn verify_and_cap_clicks(
		who: &T::AccountId,
		placement_id: &PlacementId,
		click_count: u32,
		verified_clicks: u32,
	) -> Result<u32, DispatchError> {
		if is_entity_placement::<T>(placement_id) {
			<pallet_ads_entity::Pallet<T> as ClickVerifier<T::AccountId>>::verify_and_cap_clicks(
				who, placement_id, click_count, verified_clicks,
			)
		} else {
			Err(AdsRouterError::CpcNotSupportedForPath.into())
		}
	}
}

// ============================================================================
// PlacementAdminProvider 路由
// ============================================================================

impl<T> PlacementAdminProvider<T::AccountId> for AdsRouter<T>
where
	T: pallet_ads_entity::Config + pallet_ads_grouprobot::Config,
{
	fn placement_admin(placement_id: &PlacementId) -> Option<T::AccountId> {
		if is_entity_placement::<T>(placement_id) {
			<pallet_ads_entity::Pallet<T> as PlacementAdminProvider<T::AccountId>>::placement_admin(placement_id)
		} else {
			<pallet_ads_grouprobot::Pallet<T> as PlacementAdminProvider<T::AccountId>>::placement_admin(placement_id)
		}
	}

	fn is_placement_banned(placement_id: &PlacementId) -> bool {
		if is_entity_placement::<T>(placement_id) {
			<pallet_ads_entity::Pallet<T> as PlacementAdminProvider<T::AccountId>>::is_placement_banned(placement_id)
		} else {
			<pallet_ads_grouprobot::Pallet<T> as PlacementAdminProvider<T::AccountId>>::is_placement_banned(placement_id)
		}
	}

	fn placement_status(placement_id: &PlacementId) -> PlacementStatus {
		if is_entity_placement::<T>(placement_id) {
			<pallet_ads_entity::Pallet<T> as PlacementAdminProvider<T::AccountId>>::placement_status(placement_id)
		} else {
			<pallet_ads_grouprobot::Pallet<T> as PlacementAdminProvider<T::AccountId>>::placement_status(placement_id)
		}
	}
}

// ============================================================================
// RevenueDistributor 路由
// ============================================================================

impl<T> RevenueDistributor<T::AccountId, BalanceOf<T>> for AdsRouter<T>
where
	T: pallet_ads_entity::Config
		+ pallet_ads_grouprobot::Config<
			Currency = <T as pallet_ads_entity::Config>::Currency,
		>,
	BalanceOf<T>: Default,
{
	fn distribute(
		placement_id: &PlacementId,
		total_cost: BalanceOf<T>,
		advertiser: &T::AccountId,
	) -> Result<RevenueBreakdown<BalanceOf<T>>, DispatchError> {
		if is_entity_placement::<T>(placement_id) {
			<pallet_ads_entity::Pallet<T> as RevenueDistributor<T::AccountId, BalanceOf<T>>>::distribute(
				placement_id, total_cost, advertiser,
			)
		} else {
			<pallet_ads_grouprobot::Pallet<T> as RevenueDistributor<T::AccountId, BalanceOf<T>>>::distribute(
				placement_id, total_cost, advertiser,
			)
		}
	}
}
