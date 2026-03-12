//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换默认值。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_entity_registry \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/entity/registry/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::weights::Weight;

/// WeightInfo trait — 每个 extrinsic 对应一个权重函数
pub trait WeightInfo {
    fn create_entity() -> Weight;
    fn update_entity() -> Weight;
    fn request_close_entity() -> Weight;
    fn top_up_fund() -> Weight;
    fn suspend_entity() -> Weight;
    fn resume_entity() -> Weight;
    fn ban_entity() -> Weight;
    fn add_admin() -> Weight;
    fn remove_admin() -> Weight;
    fn transfer_ownership() -> Weight;
    fn upgrade_entity_type() -> Weight;
    fn verify_entity() -> Weight;
    fn reopen_entity() -> Weight;
    fn bind_entity_referrer() -> Weight;
    fn update_admin_permissions() -> Weight;
    fn unban_entity() -> Weight;
    fn unverify_entity() -> Weight;
    fn cancel_close_request() -> Weight;
    fn resign_admin() -> Weight;
    fn self_pause_entity() -> Weight;
    fn self_resume_entity() -> Weight;
    fn force_transfer_ownership() -> Weight;
    fn reject_close_request() -> Weight;
    fn execute_close_timeout() -> Weight;
    fn force_rebind_referrer() -> Weight;
}

/// 默认权重实现（保守估计，生产前需 benchmark 替换）
pub struct SubstrateWeight;
impl WeightInfo for SubstrateWeight {
    fn create_entity() -> Weight {
        Weight::from_parts(200_000_000, 10_000)
    }
    fn update_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn request_close_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn top_up_fund() -> Weight {
        Weight::from_parts(100_000_000, 5_000)
    }
    fn suspend_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn resume_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn ban_entity() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn add_admin() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
    }
    fn remove_admin() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
    }
    fn transfer_ownership() -> Weight {
        Weight::from_parts(100_000_000, 6_000)
    }
    fn upgrade_entity_type() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn verify_entity() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
    }
    fn reopen_entity() -> Weight {
        Weight::from_parts(200_000_000, 10_000)
    }
    fn bind_entity_referrer() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn update_admin_permissions() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
    }
    fn unban_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn unverify_entity() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
    }
    fn cancel_close_request() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn resign_admin() -> Weight {
        Weight::from_parts(60_000_000, 4_000)
    }
    fn self_pause_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn self_resume_entity() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn force_transfer_ownership() -> Weight {
        Weight::from_parts(100_000_000, 6_000)
    }
    fn reject_close_request() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
    fn execute_close_timeout() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn force_rebind_referrer() -> Weight {
        Weight::from_parts(80_000_000, 5_000)
    }
}
impl WeightInfo for () {
    fn create_entity() -> Weight { Weight::zero() }
    fn update_entity() -> Weight { Weight::zero() }
    fn request_close_entity() -> Weight { Weight::zero() }
    fn top_up_fund() -> Weight { Weight::zero() }
    fn suspend_entity() -> Weight { Weight::zero() }
    fn resume_entity() -> Weight { Weight::zero() }
    fn ban_entity() -> Weight { Weight::zero() }
    fn add_admin() -> Weight { Weight::zero() }
    fn remove_admin() -> Weight { Weight::zero() }
    fn transfer_ownership() -> Weight { Weight::zero() }
    fn upgrade_entity_type() -> Weight { Weight::zero() }
    fn verify_entity() -> Weight { Weight::zero() }
    fn reopen_entity() -> Weight { Weight::zero() }
    fn bind_entity_referrer() -> Weight { Weight::zero() }
    fn update_admin_permissions() -> Weight { Weight::zero() }
    fn unban_entity() -> Weight { Weight::zero() }
    fn unverify_entity() -> Weight { Weight::zero() }
    fn cancel_close_request() -> Weight { Weight::zero() }
    fn resign_admin() -> Weight { Weight::zero() }
    fn self_pause_entity() -> Weight { Weight::zero() }
    fn self_resume_entity() -> Weight { Weight::zero() }
    fn force_transfer_ownership() -> Weight { Weight::zero() }
    fn reject_close_request() -> Weight { Weight::zero() }
    fn execute_close_timeout() -> Weight { Weight::zero() }
    fn force_rebind_referrer() -> Weight { Weight::zero() }
}
