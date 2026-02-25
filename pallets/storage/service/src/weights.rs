//! # Storage Service Pallet Weights
//!
//! 存储服务模块权重定义

use frame_support::{traits::Get, weights::Weight};

/// 权重信息 Trait
pub trait WeightInfo {
    fn request_pin() -> Weight;
    fn mark_pinned() -> Weight;
    fn mark_pin_failed() -> Weight;
    fn charge_due(n: u32) -> Weight;
    fn set_billing_params() -> Weight;
    // H6修复：新增权重函数
    fn join_operator() -> Weight;
    fn update_operator() -> Weight;
    fn leave_operator() -> Weight;
    fn set_operator_status() -> Weight;
    fn report_probe() -> Weight;
    fn slash_operator() -> Weight;
    fn fund_subject_account() -> Weight;
    fn fund_user_account() -> Weight;
    fn set_replicas_config() -> Weight;
    fn distribute_to_operators() -> Weight;
    fn set_storage_layer_config() -> Weight;
    fn set_operator_layer() -> Weight;
    fn pause_operator() -> Weight;
    fn resume_operator() -> Weight;
    fn update_tier_config() -> Weight;
    fn operator_claim_rewards() -> Weight;
    fn emergency_pause_billing() -> Weight;
    fn resume_billing() -> Weight;
    fn register_domain() -> Weight;
    fn update_domain_config() -> Weight;
    fn request_unpin() -> Weight;
    fn set_domain_priority() -> Weight;
}

/// Substrate 权重实现
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn request_pin() -> Weight {
        Weight::from_parts(80_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(8))
            .saturating_add(T::DbWeight::get().writes(10))
    }
    fn mark_pinned() -> Weight {
        Weight::from_parts(40_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn mark_pin_failed() -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn charge_due(n: u32) -> Weight {
        Weight::from_parts(50_000_000, 2_000)
            .saturating_add(Weight::from_parts(30_000_000, 500).saturating_mul(n as u64))
            .saturating_add(T::DbWeight::get().reads(2 + n as u64 * 4))
            .saturating_add(T::DbWeight::get().writes(n as u64 * 3))
    }
    fn set_billing_params() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(6))
    }
    fn join_operator() -> Weight {
        Weight::from_parts(50_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn update_operator() -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn leave_operator() -> Weight {
        Weight::from_parts(40_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn set_operator_status() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn report_probe() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn slash_operator() -> Weight {
        Weight::from_parts(35_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn fund_subject_account() -> Weight {
        Weight::from_parts(40_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn fund_user_account() -> Weight {
        Weight::from_parts(40_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn set_replicas_config() -> Weight {
        Weight::from_parts(20_000_000, 500)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn distribute_to_operators() -> Weight {
        Weight::from_parts(100_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(20))
            .saturating_add(T::DbWeight::get().writes(20))
    }
    fn set_storage_layer_config() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn set_operator_layer() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn pause_operator() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn resume_operator() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn update_tier_config() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn operator_claim_rewards() -> Weight {
        Weight::from_parts(40_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn emergency_pause_billing() -> Weight {
        Weight::from_parts(15_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn resume_billing() -> Weight {
        Weight::from_parts(15_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn register_domain() -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn update_domain_config() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn request_unpin() -> Weight {
        Weight::from_parts(35_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn set_domain_priority() -> Weight {
        Weight::from_parts(15_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// 默认权重实现（用于测试）
impl WeightInfo for () {
    fn request_pin() -> Weight { Weight::from_parts(80_000_000, 3_500) }
    fn mark_pinned() -> Weight { Weight::from_parts(40_000_000, 2_000) }
    fn mark_pin_failed() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn charge_due(n: u32) -> Weight { Weight::from_parts(50_000_000 + 30_000_000 * n as u64, 2_000) }
    fn set_billing_params() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn join_operator() -> Weight { Weight::from_parts(50_000_000, 2_500) }
    fn update_operator() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn leave_operator() -> Weight { Weight::from_parts(40_000_000, 2_000) }
    fn set_operator_status() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn report_probe() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn slash_operator() -> Weight { Weight::from_parts(35_000_000, 1_500) }
    fn fund_subject_account() -> Weight { Weight::from_parts(40_000_000, 1_500) }
    fn fund_user_account() -> Weight { Weight::from_parts(40_000_000, 1_500) }
    fn set_replicas_config() -> Weight { Weight::from_parts(20_000_000, 500) }
    fn distribute_to_operators() -> Weight { Weight::from_parts(100_000_000, 5_000) }
    fn set_storage_layer_config() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn set_operator_layer() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn pause_operator() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn resume_operator() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn update_tier_config() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn operator_claim_rewards() -> Weight { Weight::from_parts(40_000_000, 1_500) }
    fn emergency_pause_billing() -> Weight { Weight::from_parts(15_000_000, 500) }
    fn resume_billing() -> Weight { Weight::from_parts(15_000_000, 500) }
    fn register_domain() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn update_domain_config() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn request_unpin() -> Weight { Weight::from_parts(35_000_000, 1_500) }
    fn set_domain_priority() -> Weight { Weight::from_parts(15_000_000, 500) }
}
