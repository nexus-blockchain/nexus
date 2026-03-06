use frame_support::weights::Weight;

pub trait WeightInfo {
    fn set_single_line_config() -> Weight;
    fn clear_single_line_config() -> Weight;
    fn update_single_line_params() -> Weight;
    fn set_level_based_levels() -> Weight;
    fn remove_level_based_levels() -> Weight;
    fn force_set_single_line_config() -> Weight;
    fn force_clear_single_line_config() -> Weight;
    fn force_reset_single_line(limit: u32) -> Weight;
    fn pause_single_line() -> Weight;
    fn resume_single_line() -> Weight;
    fn schedule_config_change() -> Weight;
    fn apply_pending_config() -> Weight;
    fn cancel_pending_config() -> Weight;
    fn force_remove_from_single_line() -> Weight;
}

/// Hand-estimated weights based on DB read/write analysis.
/// For production, replace with benchmarked values via `frame_benchmarking`.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn set_single_line_config() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn clear_single_line_config() -> Weight {
        Weight::from_parts(35_000_000, 4_000)
    }

    fn update_single_line_params() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn set_level_based_levels() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
    }

    fn remove_level_based_levels() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }

    fn force_set_single_line_config() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn force_clear_single_line_config() -> Weight {
        Weight::from_parts(35_000_000, 4_000)
    }

    fn force_reset_single_line(limit: u32) -> Weight {
        Weight::from_parts(
            50_000_000u64.saturating_add(10_000_000u64.saturating_mul(limit as u64)),
            5_000u64.saturating_add(1_000u64.saturating_mul(limit as u64)),
        )
    }

    fn pause_single_line() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
    }

    fn resume_single_line() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
    }

    fn schedule_config_change() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn apply_pending_config() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
    }

    fn cancel_pending_config() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
    }

    fn force_remove_from_single_line() -> Weight {
        Weight::from_parts(25_000_000, 3_000)
    }
}

impl WeightInfo for () {
    fn set_single_line_config() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn clear_single_line_config() -> Weight {
        Weight::from_parts(35_000_000, 4_000)
    }

    fn update_single_line_params() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn set_level_based_levels() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
    }

    fn remove_level_based_levels() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }

    fn force_set_single_line_config() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn force_clear_single_line_config() -> Weight {
        Weight::from_parts(35_000_000, 4_000)
    }

    fn force_reset_single_line(limit: u32) -> Weight {
        Weight::from_parts(
            50_000_000u64.saturating_add(10_000_000u64.saturating_mul(limit as u64)),
            5_000u64.saturating_add(1_000u64.saturating_mul(limit as u64)),
        )
    }

    fn pause_single_line() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
    }

    fn resume_single_line() -> Weight {
        Weight::from_parts(20_000_000, 3_000)
    }

    fn schedule_config_change() -> Weight {
        Weight::from_parts(40_000_000, 4_000)
    }

    fn apply_pending_config() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
    }

    fn cancel_pending_config() -> Weight {
        Weight::from_parts(30_000_000, 3_000)
    }

    fn force_remove_from_single_line() -> Weight {
        Weight::from_parts(25_000_000, 3_000)
    }
}
