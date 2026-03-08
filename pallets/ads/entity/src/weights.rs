//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换默认值。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_ads_entity \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/ads/entity/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn register_entity_placement() -> Weight;
	fn register_shop_placement() -> Weight;
	fn deregister_placement() -> Weight;
	fn set_placement_active() -> Weight;
	fn set_impression_cap() -> Weight;
	fn set_entity_ad_share() -> Weight;
	fn ban_entity() -> Weight;
	fn unban_entity() -> Weight;
	fn set_click_cap() -> Weight;
}

/// 默认权重实现（保守估计，生产前需 benchmark 替换）
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T> WeightInfo for SubstrateWeight<T> {
	fn register_entity_placement() -> Weight {
		Weight::from_parts(50_000_000, 8_000)
	}
	fn register_shop_placement() -> Weight {
		Weight::from_parts(50_000_000, 8_000)
	}
	fn deregister_placement() -> Weight {
		Weight::from_parts(50_000_000, 8_000)
	}
	fn set_placement_active() -> Weight {
		Weight::from_parts(30_000_000, 5_000)
	}
	fn set_impression_cap() -> Weight {
		Weight::from_parts(30_000_000, 5_000)
	}
	fn set_entity_ad_share() -> Weight {
		Weight::from_parts(20_000_000, 4_000)
	}
	fn ban_entity() -> Weight {
		Weight::from_parts(20_000_000, 4_000)
	}
	fn unban_entity() -> Weight {
		Weight::from_parts(20_000_000, 4_000)
	}
	fn set_click_cap() -> Weight {
		Weight::from_parts(30_000_000, 5_000)
	}
}

impl WeightInfo for () {
	fn register_entity_placement() -> Weight {
		Weight::from_parts(50_000_000, 8_000)
	}
	fn register_shop_placement() -> Weight {
		Weight::from_parts(50_000_000, 8_000)
	}
	fn deregister_placement() -> Weight {
		Weight::from_parts(50_000_000, 8_000)
	}
	fn set_placement_active() -> Weight {
		Weight::from_parts(30_000_000, 5_000)
	}
	fn set_impression_cap() -> Weight {
		Weight::from_parts(30_000_000, 5_000)
	}
	fn set_entity_ad_share() -> Weight {
		Weight::from_parts(20_000_000, 4_000)
	}
	fn ban_entity() -> Weight {
		Weight::from_parts(20_000_000, 4_000)
	}
	fn unban_entity() -> Weight {
		Weight::from_parts(20_000_000, 4_000)
	}
	fn set_click_cap() -> Weight {
		Weight::from_parts(30_000_000, 5_000)
	}
}
