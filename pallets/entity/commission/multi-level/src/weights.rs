use frame_support::weights::Weight;

/// Weight functions for pallet-commission-multi-level
pub trait WeightInfo {
    fn set_multi_level_config(levels: u32) -> Weight;
    fn clear_multi_level_config() -> Weight;
    /// F3: 部分更新 — 1 read + 1 write (try_mutate)
    fn update_multi_level_params() -> Weight;
    /// F4: 插入层级 — 1 read + 1 write + Vec insert
    fn add_tier() -> Weight;
    /// F4: 移除层级 — 1 read + 1 write + Vec remove
    fn remove_tier() -> Weight;
}

/// Substrate weight estimates based on DB read/write analysis.
///
/// set_multi_level_config: 1 write (MultiLevelConfigs), bounded iteration for validation.
/// Base: 35M ref_time + 3K proof_size, +2M per level for validation loop.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn set_multi_level_config(levels: u32) -> Weight {
        Weight::from_parts(
            35_000_000u64.saturating_add(2_000_000u64.saturating_mul(levels as u64)),
            3_000u64,
        )
    }

    fn clear_multi_level_config() -> Weight {
        Weight::from_parts(35_000_000, 3_000)
    }

    fn update_multi_level_params() -> Weight {
        // 1 read (MultiLevelConfigs) + 1 write (try_mutate) + permission checks
        Weight::from_parts(40_000_000, 4_000)
    }

    fn add_tier() -> Weight {
        // 1 read + 1 write + Vec realloc/insert
        Weight::from_parts(40_000_000, 4_000)
    }

    fn remove_tier() -> Weight {
        // 1 read + 1 write + Vec remove
        Weight::from_parts(38_000_000, 4_000)
    }
}

impl WeightInfo for () {
    fn set_multi_level_config(levels: u32) -> Weight {
        Weight::from_parts(
            35_000_000u64.saturating_add(2_000_000u64.saturating_mul(levels as u64)),
            3_000u64,
        )
    }

    fn clear_multi_level_config() -> Weight {
        Weight::from_parts(35_000_000, 3_000)
    }

    fn update_multi_level_params() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn add_tier() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn remove_tier() -> Weight {
        Weight::from_parts(38_000_000, 4_000)
    }
}
