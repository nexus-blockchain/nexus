//! # Storage Service Pallet Weights
//!
//! 存储服务模块权重定义
//! 注意：当前为手写估算值，需通过 frame-benchmarking-cli 生成真实权重替换。

use frame_support::{traits::Get, weights::Weight};

/// 权重信息 Trait — 覆盖所有 dispatchable extrinsics
pub trait WeightInfo {
    // 用户接口
    fn request_pin() -> Weight;
    fn fund_user_account() -> Weight;
    fn withdraw_user_funding() -> Weight;
    fn request_unpin() -> Weight;
    fn batch_unpin() -> Weight;
    fn renew_pin() -> Weight;
    fn upgrade_pin_tier() -> Weight;
    fn downgrade_pin_tier() -> Weight;
    fn fund_ipfs_pool() -> Weight;
    // 运营者接口
    fn join_operator() -> Weight;
    fn update_operator() -> Weight;
    fn leave_operator() -> Weight;
    fn pause_operator() -> Weight;
    fn resume_operator() -> Weight;
    fn report_probe() -> Weight;
    fn operator_claim_rewards() -> Weight;
    fn top_up_bond() -> Weight;
    fn reduce_bond() -> Weight;
    fn dispute_slash() -> Weight;
    fn mark_pinned() -> Weight;
    fn mark_pin_failed() -> Weight;
    // OCW unsigned
    fn ocw_mark_pinned() -> Weight;
    fn ocw_mark_pin_failed() -> Weight;
    fn ocw_submit_assignments() -> Weight;
    fn ocw_report_health() -> Weight;
    // 公共清理
    fn cleanup_expired_cids(n: u32) -> Weight;
    fn cleanup_expired_locks() -> Weight;
    // 治理接口
    fn set_operator_status() -> Weight;
    fn slash_operator() -> Weight;
    fn set_billing_params() -> Weight;
    fn distribute_to_operators() -> Weight;
    fn update_tier_config() -> Weight;
    fn emergency_pause_billing() -> Weight;
    fn resume_billing() -> Weight;
    fn set_storage_layer_config() -> Weight;
    fn set_operator_layer() -> Weight;
    fn register_domain() -> Weight;
    fn update_domain_config() -> Weight;
    fn set_domain_priority() -> Weight;
    fn governance_force_unpin() -> Weight;
    fn migrate_operator_pins() -> Weight;
    // 已废弃（保留权重以兼容 call_index）
    fn fund_subject_account() -> Weight;
}

/// Substrate 权重实现（手写估算，待 benchmark 替换）
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    // ---- 用户接口 ----
    fn request_pin() -> Weight {
        Weight::from_parts(80_000_000, 3_500)
            .saturating_add(T::DbWeight::get().reads(8))
            .saturating_add(T::DbWeight::get().writes(10))
    }
    fn fund_user_account() -> Weight {
        Weight::from_parts(40_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn withdraw_user_funding() -> Weight {
        Weight::from_parts(40_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn request_unpin() -> Weight {
        Weight::from_parts(50_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn batch_unpin() -> Weight {
        Weight::from_parts(200_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(40))
            .saturating_add(T::DbWeight::get().writes(40))
    }
    fn renew_pin() -> Weight {
        Weight::from_parts(60_000_000, 2_500)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn upgrade_pin_tier() -> Weight {
        Weight::from_parts(70_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(6))
            .saturating_add(T::DbWeight::get().writes(5))
    }
    fn downgrade_pin_tier() -> Weight {
        Weight::from_parts(50_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    fn fund_ipfs_pool() -> Weight {
        Weight::from_parts(35_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    // ---- 运营者接口 ----
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
    fn report_probe() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn operator_claim_rewards() -> Weight {
        Weight::from_parts(40_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn top_up_bond() -> Weight {
        Weight::from_parts(35_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn reduce_bond() -> Weight {
        Weight::from_parts(35_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn dispute_slash() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(0))
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
    // ---- OCW unsigned ----
    fn ocw_mark_pinned() -> Weight {
        Weight::from_parts(40_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn ocw_mark_pin_failed() -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn ocw_submit_assignments() -> Weight {
        Weight::from_parts(60_000_000, 3_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(6))
    }
    fn ocw_report_health() -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    // ---- 公共清理 ----
    fn cleanup_expired_cids(n: u32) -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(Weight::from_parts(20_000_000, 500).saturating_mul(n as u64))
            .saturating_add(T::DbWeight::get().reads(1 + n as u64 * 3))
            .saturating_add(T::DbWeight::get().writes(n as u64 * 10))
    }
    fn cleanup_expired_locks() -> Weight {
        Weight::from_parts(50_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(10))
            .saturating_add(T::DbWeight::get().writes(10))
    }
    // ---- 治理接口 ----
    fn set_operator_status() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn slash_operator() -> Weight {
        Weight::from_parts(35_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn set_billing_params() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(6))
    }
    fn distribute_to_operators() -> Weight {
        Weight::from_parts(100_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(20))
            .saturating_add(T::DbWeight::get().writes(20))
    }
    fn update_tier_config() -> Weight {
        Weight::from_parts(20_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn emergency_pause_billing() -> Weight {
        Weight::from_parts(15_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn resume_billing() -> Weight {
        Weight::from_parts(15_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
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
    fn register_domain() -> Weight {
        Weight::from_parts(30_000_000, 1_500)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn update_domain_config() -> Weight {
        Weight::from_parts(25_000_000, 1_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn set_domain_priority() -> Weight {
        Weight::from_parts(15_000_000, 500)
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn governance_force_unpin() -> Weight {
        Weight::from_parts(40_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    fn migrate_operator_pins() -> Weight {
        Weight::from_parts(100_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(20))
            .saturating_add(T::DbWeight::get().writes(20))
    }
    fn fund_subject_account() -> Weight {
        Weight::from_parts(10_000_000, 500)
    }
}

/// 默认权重实现（用于测试）
impl WeightInfo for () {
    fn request_pin() -> Weight { Weight::from_parts(80_000_000, 3_500) }
    fn fund_user_account() -> Weight { Weight::from_parts(40_000_000, 1_500) }
    fn withdraw_user_funding() -> Weight { Weight::from_parts(40_000_000, 1_500) }
    fn request_unpin() -> Weight { Weight::from_parts(50_000_000, 2_000) }
    fn batch_unpin() -> Weight { Weight::from_parts(200_000_000, 8_000) }
    fn renew_pin() -> Weight { Weight::from_parts(60_000_000, 2_500) }
    fn upgrade_pin_tier() -> Weight { Weight::from_parts(70_000_000, 3_000) }
    fn downgrade_pin_tier() -> Weight { Weight::from_parts(50_000_000, 2_000) }
    fn fund_ipfs_pool() -> Weight { Weight::from_parts(35_000_000, 1_500) }
    fn join_operator() -> Weight { Weight::from_parts(50_000_000, 2_500) }
    fn update_operator() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn leave_operator() -> Weight { Weight::from_parts(40_000_000, 2_000) }
    fn pause_operator() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn resume_operator() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn report_probe() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn operator_claim_rewards() -> Weight { Weight::from_parts(40_000_000, 1_500) }
    fn top_up_bond() -> Weight { Weight::from_parts(35_000_000, 1_500) }
    fn reduce_bond() -> Weight { Weight::from_parts(35_000_000, 1_500) }
    fn dispute_slash() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn mark_pinned() -> Weight { Weight::from_parts(40_000_000, 2_000) }
    fn mark_pin_failed() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn ocw_mark_pinned() -> Weight { Weight::from_parts(40_000_000, 2_000) }
    fn ocw_mark_pin_failed() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn ocw_submit_assignments() -> Weight { Weight::from_parts(60_000_000, 3_000) }
    fn ocw_report_health() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn cleanup_expired_cids(n: u32) -> Weight { Weight::from_parts(30_000_000 + 20_000_000 * n as u64, 1_500) }
    fn cleanup_expired_locks() -> Weight { Weight::from_parts(50_000_000, 2_000) }
    fn set_operator_status() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn slash_operator() -> Weight { Weight::from_parts(35_000_000, 1_500) }
    fn set_billing_params() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn distribute_to_operators() -> Weight { Weight::from_parts(100_000_000, 5_000) }
    fn update_tier_config() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn emergency_pause_billing() -> Weight { Weight::from_parts(15_000_000, 500) }
    fn resume_billing() -> Weight { Weight::from_parts(15_000_000, 500) }
    fn set_storage_layer_config() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn set_operator_layer() -> Weight { Weight::from_parts(20_000_000, 1_000) }
    fn register_domain() -> Weight { Weight::from_parts(30_000_000, 1_500) }
    fn update_domain_config() -> Weight { Weight::from_parts(25_000_000, 1_000) }
    fn set_domain_priority() -> Weight { Weight::from_parts(15_000_000, 500) }
    fn governance_force_unpin() -> Weight { Weight::from_parts(40_000_000, 2_000) }
    fn migrate_operator_pins() -> Weight { Weight::from_parts(100_000_000, 5_000) }
    fn fund_subject_account() -> Weight { Weight::from_parts(10_000_000, 500) }
}
