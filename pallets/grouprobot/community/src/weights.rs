//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换默认值。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_grouprobot_community \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/grouprobot/community/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use core::marker::PhantomData;
use frame_support::weights::Weight;

pub trait WeightInfo {
    fn submit_action_log() -> Weight;
    fn set_node_requirement() -> Weight;
    fn update_community_config() -> Weight;
    fn batch_submit_logs(len: u32) -> Weight;
    fn clear_expired_logs() -> Weight;
    fn award_reputation() -> Weight;
    fn deduct_reputation() -> Weight;
    fn reset_reputation() -> Weight;
    fn update_active_members() -> Weight;
    fn cleanup_expired_cooldowns() -> Weight;
    fn delete_community_config() -> Weight;
    fn force_remove_community() -> Weight;
    fn ban_community() -> Weight;
    fn unban_community() -> Weight;
    fn force_update_community_config() -> Weight;
    fn force_reset_community_reputation() -> Weight;
}

/// 默认权重实现（保守估计，生产前需 benchmark 替换）
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn submit_action_log() -> Weight {
        Weight::from_parts(55_000_000, 8_000)
    }
    fn set_node_requirement() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn update_community_config() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn batch_submit_logs(len: u32) -> Weight {
        Weight::from_parts(
            30_000_000u64.saturating_add(55_000_000u64.saturating_mul(len as u64)),
            5_000u64.saturating_add(500u64.saturating_mul(len as u64)),
        )
    }
    fn clear_expired_logs() -> Weight {
        Weight::from_parts(50_000_000, 10_000)
    }
    fn award_reputation() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
    }
    fn deduct_reputation() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
    }
    fn reset_reputation() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn update_active_members() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn cleanup_expired_cooldowns() -> Weight {
        Weight::from_parts(15_000_000, 4_000)
    }
    fn delete_community_config() -> Weight {
        Weight::from_parts(60_000_000, 12_000)
    }
    fn force_remove_community() -> Weight {
        Weight::from_parts(80_000_000, 15_000)
    }
    fn ban_community() -> Weight {
        Weight::from_parts(25_000_000, 5_000)
    }
    fn unban_community() -> Weight {
        Weight::from_parts(25_000_000, 5_000)
    }
    fn force_update_community_config() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn force_reset_community_reputation() -> Weight {
        Weight::from_parts(100_000_000, 20_000)
    }
}

/// 单元测试用 fallback 实现（与 SubstrateWeight 相同权重）
impl WeightInfo for () {
    fn submit_action_log() -> Weight {
        Weight::from_parts(55_000_000, 8_000)
    }
    fn set_node_requirement() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn update_community_config() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn batch_submit_logs(len: u32) -> Weight {
        Weight::from_parts(
            30_000_000u64.saturating_add(55_000_000u64.saturating_mul(len as u64)),
            5_000u64.saturating_add(500u64.saturating_mul(len as u64)),
        )
    }
    fn clear_expired_logs() -> Weight {
        Weight::from_parts(50_000_000, 10_000)
    }
    fn award_reputation() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
    }
    fn deduct_reputation() -> Weight {
        Weight::from_parts(35_000_000, 6_000)
    }
    fn reset_reputation() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
    }
    fn update_active_members() -> Weight {
        Weight::from_parts(20_000_000, 4_000)
    }
    fn cleanup_expired_cooldowns() -> Weight {
        Weight::from_parts(15_000_000, 4_000)
    }
    fn delete_community_config() -> Weight {
        Weight::from_parts(60_000_000, 12_000)
    }
    fn force_remove_community() -> Weight {
        Weight::from_parts(80_000_000, 15_000)
    }
    fn ban_community() -> Weight {
        Weight::from_parts(25_000_000, 5_000)
    }
    fn unban_community() -> Weight {
        Weight::from_parts(25_000_000, 5_000)
    }
    fn force_update_community_config() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
    fn force_reset_community_reputation() -> Weight {
        Weight::from_parts(100_000_000, 20_000)
    }
}
