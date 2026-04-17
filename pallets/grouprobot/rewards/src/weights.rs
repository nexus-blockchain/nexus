//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换默认值。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_grouprobot_rewards \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/grouprobot/rewards/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn claim_rewards() -> Weight;
    fn rescue_stranded_rewards() -> Weight;
    fn batch_claim_rewards(n: u32) -> Weight;
    fn set_reward_recipient() -> Weight;
    fn force_slash_pending_rewards() -> Weight;
    fn set_reward_split() -> Weight;
    fn claim_owner_rewards() -> Weight;
    fn pause_distribution() -> Weight;
    fn resume_distribution() -> Weight;
    fn force_set_pending_rewards() -> Weight;
    fn force_prune_era_rewards(n: u32) -> Weight;
}

/// 默认权重实现（保守估计，生产前需 benchmark 替换）
pub struct SubstrateWeight;
impl WeightInfo for SubstrateWeight {
    fn claim_rewards() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn rescue_stranded_rewards() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn batch_claim_rewards(n: u32) -> Weight {
        Weight::from_parts(
            40_000_000u64.saturating_mul(n as u64),
            6_000u64.saturating_mul(n as u64),
        )
    }
    fn set_reward_recipient() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn force_slash_pending_rewards() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn set_reward_split() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn claim_owner_rewards() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn pause_distribution() -> Weight {
        Weight::from_parts(10_000_000, 2_000)
    }
    fn resume_distribution() -> Weight {
        Weight::from_parts(10_000_000, 2_000)
    }
    fn force_set_pending_rewards() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn force_prune_era_rewards(n: u32) -> Weight {
        Weight::from_parts(
            10_000_000u64.saturating_mul(n as u64),
            3_000u64.saturating_mul(n as u64),
        )
    }
}

/// 单元测试用零权重实现
impl WeightInfo for () {
    fn claim_rewards() -> Weight {
        Weight::zero()
    }
    fn rescue_stranded_rewards() -> Weight {
        Weight::zero()
    }
    fn batch_claim_rewards(_n: u32) -> Weight {
        Weight::zero()
    }
    fn set_reward_recipient() -> Weight {
        Weight::zero()
    }
    fn force_slash_pending_rewards() -> Weight {
        Weight::zero()
    }
    fn set_reward_split() -> Weight {
        Weight::zero()
    }
    fn claim_owner_rewards() -> Weight {
        Weight::zero()
    }
    fn pause_distribution() -> Weight {
        Weight::zero()
    }
    fn resume_distribution() -> Weight {
        Weight::zero()
    }
    fn force_set_pending_rewards() -> Weight {
        Weight::zero()
    }
    fn force_prune_era_rewards(_n: u32) -> Weight {
        Weight::zero()
    }
}
