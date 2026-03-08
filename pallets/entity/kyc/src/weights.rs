//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 当前为基于 DB read/write 分析的手工估算值。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_entity_kyc \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/entity/kyc/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::weights::Weight;

/// WeightInfo trait — 每个 extrinsic 对应一个权重函数
pub trait WeightInfo {
    fn submit_kyc() -> Weight;
    fn approve_kyc() -> Weight;
    fn reject_kyc() -> Weight;
    fn revoke_kyc() -> Weight;
    fn register_provider() -> Weight;
    fn remove_provider() -> Weight;
    fn set_entity_requirement() -> Weight;
    fn update_high_risk_countries() -> Weight;
    fn expire_kyc() -> Weight;
    fn cancel_kyc() -> Weight;
    fn force_set_entity_requirement() -> Weight;
    fn update_risk_score() -> Weight;
    fn update_provider() -> Weight;
    fn suspend_provider() -> Weight;
    fn resume_provider() -> Weight;
    fn force_approve_kyc() -> Weight;
    fn renew_kyc() -> Weight;
    fn update_kyc_data() -> Weight;
    fn purge_kyc_data() -> Weight;
    fn remove_entity_requirement() -> Weight;
    fn timeout_pending_kyc() -> Weight;
    fn batch_revoke_by_provider(accounts_len: u32) -> Weight;
    fn authorize_provider() -> Weight;
    fn deauthorize_provider() -> Weight;
    fn entity_revoke_kyc() -> Weight;
}

/// 基于 DB read/write 分析的权重估算（pre-benchmark）。
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn submit_kyc() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn approve_kyc() -> Weight {
        Weight::from_parts(80_000_000, 6_000)
    }

    fn reject_kyc() -> Weight {
        Weight::from_parts(60_000_000, 6_000)
    }

    fn revoke_kyc() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn register_provider() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
    }

    fn remove_provider() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn set_entity_requirement() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn update_high_risk_countries() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn expire_kyc() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn cancel_kyc() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn force_set_entity_requirement() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn update_risk_score() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn update_provider() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn suspend_provider() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn resume_provider() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn force_approve_kyc() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
    }

    fn renew_kyc() -> Weight {
        Weight::from_parts(60_000_000, 5_000)
    }

    fn update_kyc_data() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn purge_kyc_data() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn remove_entity_requirement() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn timeout_pending_kyc() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }

    fn batch_revoke_by_provider(accounts_len: u32) -> Weight {
        Weight::from_parts(
            50_000_000u64.saturating_add(50_000_000u64.saturating_mul(accounts_len as u64)),
            5_000u64.saturating_add(5_000u64.saturating_mul(accounts_len as u64)),
        )
    }

    fn authorize_provider() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn deauthorize_provider() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn entity_revoke_kyc() -> Weight {
        Weight::from_parts(50_000_000, 5_000)
    }
}

/// 单元测试用零权重实现
impl WeightInfo for () {
    fn submit_kyc() -> Weight {
        Weight::zero()
    }
    fn approve_kyc() -> Weight {
        Weight::zero()
    }
    fn reject_kyc() -> Weight {
        Weight::zero()
    }
    fn revoke_kyc() -> Weight {
        Weight::zero()
    }
    fn register_provider() -> Weight {
        Weight::zero()
    }
    fn remove_provider() -> Weight {
        Weight::zero()
    }
    fn set_entity_requirement() -> Weight {
        Weight::zero()
    }
    fn update_high_risk_countries() -> Weight {
        Weight::zero()
    }
    fn expire_kyc() -> Weight {
        Weight::zero()
    }
    fn cancel_kyc() -> Weight {
        Weight::zero()
    }
    fn force_set_entity_requirement() -> Weight {
        Weight::zero()
    }
    fn update_risk_score() -> Weight {
        Weight::zero()
    }
    fn update_provider() -> Weight {
        Weight::zero()
    }
    fn suspend_provider() -> Weight {
        Weight::zero()
    }
    fn resume_provider() -> Weight {
        Weight::zero()
    }
    fn force_approve_kyc() -> Weight {
        Weight::zero()
    }
    fn renew_kyc() -> Weight {
        Weight::zero()
    }
    fn update_kyc_data() -> Weight {
        Weight::zero()
    }
    fn purge_kyc_data() -> Weight {
        Weight::zero()
    }
    fn remove_entity_requirement() -> Weight {
        Weight::zero()
    }
    fn timeout_pending_kyc() -> Weight {
        Weight::zero()
    }
    fn batch_revoke_by_provider(_accounts_len: u32) -> Weight {
        Weight::zero()
    }
    fn authorize_provider() -> Weight {
        Weight::zero()
    }
    fn deauthorize_provider() -> Weight {
        Weight::zero()
    }
    fn entity_revoke_kyc() -> Weight {
        Weight::zero()
    }
}
