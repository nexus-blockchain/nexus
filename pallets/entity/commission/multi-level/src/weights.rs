use frame_support::weights::Weight;

/// Weight functions for pallet-commission-multi-level
pub trait WeightInfo {
    fn set_multi_level_config(levels: u32) -> Weight;
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
}

impl WeightInfo for () {
    fn set_multi_level_config(levels: u32) -> Weight {
        Weight::from_parts(
            35_000_000u64.saturating_add(2_000_000u64.saturating_mul(levels as u64)),
            3_000u64,
        )
    }
}
