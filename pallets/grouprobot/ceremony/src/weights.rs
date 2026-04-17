//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换默认值。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_grouprobot_ceremony \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/grouprobot/ceremony/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn record_ceremony(p: u32) -> Weight;
    fn revoke_ceremony() -> Weight;
    fn approve_ceremony_enclave() -> Weight;
    fn remove_ceremony_enclave() -> Weight;
    fn force_re_ceremony() -> Weight;
    fn cleanup_ceremony() -> Weight;
    fn owner_revoke_ceremony() -> Weight;
    fn revoke_by_mrenclave() -> Weight;
    fn trigger_expiry() -> Weight;
    fn batch_cleanup_ceremonies(n: u32) -> Weight;
    fn renew_ceremony() -> Weight;
    fn on_initialize() -> Weight;
}

/// 默认权重实现（保守估计，生产前需 benchmark 替换）
pub struct SubstrateWeight;
impl WeightInfo for SubstrateWeight {
    fn record_ceremony(p: u32) -> Weight {
        Weight::from_parts(
            50_000_000u64.saturating_add(5_000_000u64.saturating_mul(p as u64)),
            10_000u64.saturating_add(1_000u64.saturating_mul(p as u64)),
        )
    }
    fn revoke_ceremony() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn approve_ceremony_enclave() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
    fn remove_ceremony_enclave() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
    }
    fn force_re_ceremony() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn cleanup_ceremony() -> Weight {
        Weight::from_parts(30_000_000, 6_000)
    }
    fn owner_revoke_ceremony() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn revoke_by_mrenclave() -> Weight {
        Weight::from_parts(100_000_000, 20_000)
    }
    fn trigger_expiry() -> Weight {
        Weight::from_parts(30_000_000, 6_000)
    }
    fn batch_cleanup_ceremonies(n: u32) -> Weight {
        Weight::from_parts(
            20_000_000u64.saturating_add(10_000_000u64.saturating_mul(n as u64)),
            4_000u64.saturating_add(2_000u64.saturating_mul(n as u64)),
        )
    }
    fn renew_ceremony() -> Weight {
        Weight::from_parts(40_000_000, 8_000)
    }
    fn on_initialize() -> Weight {
        Weight::from_parts(50_000_000, 10_000)
    }
}

/// 单元测试用零权重实现
impl WeightInfo for () {
    fn record_ceremony(_p: u32) -> Weight {
        Weight::zero()
    }
    fn revoke_ceremony() -> Weight {
        Weight::zero()
    }
    fn approve_ceremony_enclave() -> Weight {
        Weight::zero()
    }
    fn remove_ceremony_enclave() -> Weight {
        Weight::zero()
    }
    fn force_re_ceremony() -> Weight {
        Weight::zero()
    }
    fn cleanup_ceremony() -> Weight {
        Weight::zero()
    }
    fn owner_revoke_ceremony() -> Weight {
        Weight::zero()
    }
    fn revoke_by_mrenclave() -> Weight {
        Weight::zero()
    }
    fn trigger_expiry() -> Weight {
        Weight::zero()
    }
    fn batch_cleanup_ceremonies(_n: u32) -> Weight {
        Weight::zero()
    }
    fn renew_ceremony() -> Weight {
        Weight::zero()
    }
    fn on_initialize() -> Weight {
        Weight::zero()
    }
}
