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
    /// 🆕 F6: 管理员应急强制释放
    fn force_release() -> Weight;
    /// 🆕 F6: 管理员应急强制退款
    fn force_refund() -> Weight;
    /// 🆕 F1: 部分退款
    fn refund_partial() -> Weight;
    /// 🆕 F3: 部分释放
    fn release_partial() -> Weight;
    /// 🆕 F8: 清理已关闭托管
    fn cleanup_closed() -> Weight;
    /// 🆕 F9: Token 托管锁定
    fn token_lock() -> Weight;
    /// 🆕 F9: Token 托管释放
    fn token_release() -> Weight;
    /// 🆕 F9: Token 托管退款
    fn token_refund() -> Weight;
}

/// Substrate 权重实现
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn lock() -> Weight {
        // L4-R2修复: reads 3→4 (Paused + LockStateOf×2 + Locked)
        Weight::from_parts(50_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn release() -> Weight {
        // reads: Paused + LockStateOf(extrinsic) + LockStateOf(trait) + Locked + Currency
        Weight::from_parts(60_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn refund() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn lock_with_nonce() -> Weight {
        // reads: Paused + LockNonces + LockStateOf(extrinsic) + LockStateOf(trait) + Locked
        Weight::from_parts(55_000_000, 4_500)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn release_split() -> Weight {
        Weight::from_parts(100_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(10))
    }
    fn dispute() -> Weight {
        // L3-R2修复: writes 1→3 (LockStateOf + DisputedAt + Event)
        Weight::from_parts(40_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn apply_decision_release() -> Weight {
        // M4-R2: +1 write for DisputedAt::remove
        Weight::from_parts(70_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn apply_decision_refund() -> Weight {
        Weight::from_parts(70_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn apply_decision_partial() -> Weight {
        Weight::from_parts(90_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(6))
    }
    fn set_pause() -> Weight {
        Weight::from_parts(10_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn schedule_expiry() -> Weight {
        Weight::from_parts(50_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn cancel_expiry() -> Weight {
        Weight::from_parts(40_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn force_release() -> Weight {
        Weight::from_parts(70_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn force_refund() -> Weight {
        Weight::from_parts(70_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn refund_partial() -> Weight {
        // reads: Paused + LockStateOf(extrinsic) + LockStateOf(trait) + Locked + Currency
        Weight::from_parts(60_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn release_partial() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn cleanup_closed() -> Weight {
        // M2-R2: +reads/writes for ExpiringAt cleanup
        Weight::from_parts(50_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(20))
            .saturating_add(T::DbWeight::get().writes(60))
    }
    fn token_lock() -> Weight {
        Weight::from_parts(50_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn token_release() -> Weight {
        Weight::from_parts(60_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn token_refund() -> Weight {
        Weight::from_parts(60_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
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
    fn force_release() -> Weight { Weight::from_parts(70_000_000, 0) }
    fn force_refund() -> Weight { Weight::from_parts(70_000_000, 0) }
    fn refund_partial() -> Weight { Weight::from_parts(60_000_000, 0) }
    fn release_partial() -> Weight { Weight::from_parts(60_000_000, 0) }
    fn cleanup_closed() -> Weight { Weight::from_parts(50_000_000, 0) }
    fn token_lock() -> Weight { Weight::from_parts(50_000_000, 0) }
    fn token_release() -> Weight { Weight::from_parts(60_000_000, 0) }
    fn token_refund() -> Weight { Weight::from_parts(60_000_000, 0) }
}
