//! # Escrow Pallet Weights
//!
//! 托管模块权重定义（M2审计修复：覆盖全部 12 个 extrinsic）

use frame_support::{traits::Get, weights::Weight};

/// 权重信息 Trait
pub trait WeightInfo {
    fn lock() -> Weight;
    fn release() -> Weight;
    fn refund() -> Weight;
    fn lock_with_nonce() -> Weight;
    fn release_split() -> Weight;
    fn dispute() -> Weight;
    fn apply_decision_release() -> Weight;
    fn apply_decision_refund() -> Weight;
    fn apply_decision_partial() -> Weight;
    fn set_pause() -> Weight;
    fn schedule_expiry() -> Weight;
    fn cancel_expiry() -> Weight;
}

/// Substrate 权重实现
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn lock() -> Weight {
        Weight::from_parts(50_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn release() -> Weight {
        Weight::from_parts(60_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn refund() -> Weight {
        Weight::from_parts(60_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn lock_with_nonce() -> Weight {
        Weight::from_parts(55_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn release_split() -> Weight {
        Weight::from_parts(100_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(10))
    }
    fn dispute() -> Weight {
        Weight::from_parts(40_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn apply_decision_release() -> Weight {
        Weight::from_parts(70_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn apply_decision_refund() -> Weight {
        Weight::from_parts(70_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn apply_decision_partial() -> Weight {
        Weight::from_parts(90_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn set_pause() -> Weight {
        Weight::from_parts(10_000_000, 0)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn schedule_expiry() -> Weight {
        Weight::from_parts(50_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn cancel_expiry() -> Weight {
        Weight::from_parts(40_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
}

/// 默认权重实现（用于测试）
impl WeightInfo for () {
    fn lock() -> Weight { Weight::from_parts(50_000_000, 0) }
    fn release() -> Weight { Weight::from_parts(60_000_000, 0) }
    fn refund() -> Weight { Weight::from_parts(60_000_000, 0) }
    fn lock_with_nonce() -> Weight { Weight::from_parts(55_000_000, 0) }
    fn release_split() -> Weight { Weight::from_parts(100_000_000, 0) }
    fn dispute() -> Weight { Weight::from_parts(40_000_000, 0) }
    fn apply_decision_release() -> Weight { Weight::from_parts(70_000_000, 0) }
    fn apply_decision_refund() -> Weight { Weight::from_parts(70_000_000, 0) }
    fn apply_decision_partial() -> Weight { Weight::from_parts(90_000_000, 0) }
    fn set_pause() -> Weight { Weight::from_parts(10_000_000, 0) }
    fn schedule_expiry() -> Weight { Weight::from_parts(50_000_000, 0) }
    fn cancel_expiry() -> Weight { Weight::from_parts(40_000_000, 0) }
}
