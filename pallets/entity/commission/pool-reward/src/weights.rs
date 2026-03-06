use frame_support::weights::Weight;

pub trait WeightInfo {
    fn set_pool_reward_config() -> Weight;
    fn claim_pool_reward() -> Weight;
    fn start_new_round() -> Weight;
    fn set_token_pool_enabled() -> Weight;
    fn clear_pool_reward_config() -> Weight;
    fn force_clear_pool_reward_config() -> Weight;
    fn pause_pool_reward() -> Weight;
    fn resume_pool_reward() -> Weight;
    fn set_global_pool_reward_paused() -> Weight;
    fn force_pause_pool_reward() -> Weight;
    fn force_resume_pool_reward() -> Weight;
    fn schedule_pool_reward_config_change() -> Weight;
    fn apply_pending_pool_reward_config() -> Weight;
    fn cancel_pending_pool_reward_config() -> Weight;
}

pub struct SubstrateWeight;

impl WeightInfo for SubstrateWeight {
    /// entity_exists(1) + auth(3) + is_locked(1) + config_read(1) + current_round(1) = 7R
    /// config_write(1) + current_round_remove(1) + last_round_id(1) + pending_config_remove(1) = 4W
    fn set_pool_reward_config() -> Weight {
        Weight::from_parts(55_000_000, 7_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(7))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(4))
    }

    /// worst case: base 10R/7W + round creation ~8R/4W + N × member_count_by_level
    /// + TokenPoolDeficit write (rollback failure path) + P2-13 remove(0)
    fn claim_pool_reward() -> Weight {
        Weight::from_parts(250_000_000, 25_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(20))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(13))
    }

    /// P2-4 修复: create_new_round 包含 CurrentRound(1) + RoundHistory(1) + DistributionStats(1) + LastRoundId(1)
    fn start_new_round() -> Weight {
        Weight::from_parts(110_000_000, 11_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(8))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(4))
    }

    fn set_token_pool_enabled() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(3))
    }

    /// P2-6 修复: 增加 RoundHistory + DistributionStats 写入
    fn clear_pool_reward_config() -> Weight {
        Weight::from_parts(45_000_000, 6_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(7))
    }

    /// 2x clear_prefix(u32::MAX) 按最多 500 用户估权 + TokenPoolDeficit
    /// Root 操作，后续可通过 benchmark 替换
    fn force_clear_pool_reward_config() -> Weight {
        Weight::from_parts(500_000_000, 100_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(2))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1010))
    }

    fn pause_pool_reward() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    fn resume_pool_reward() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    fn set_global_pool_reward_paused() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(1))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    fn force_pause_pool_reward() -> Weight {
        Weight::from_parts(25_000_000, 3_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(2))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    fn force_resume_pool_reward() -> Weight {
        Weight::from_parts(25_000_000, 3_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(2))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    fn schedule_pool_reward_config_change() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    /// P2-7: 现在需要 owner/admin 检查，增加 reads
    fn apply_pending_pool_reward_config() -> Weight {
        Weight::from_parts(55_000_000, 7_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(6))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(4))
    }

    fn cancel_pending_pool_reward_config() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(3))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }
}

impl WeightInfo for () {
    fn set_pool_reward_config() -> Weight { Weight::zero() }
    fn claim_pool_reward() -> Weight { Weight::zero() }
    fn start_new_round() -> Weight { Weight::zero() }
    fn set_token_pool_enabled() -> Weight { Weight::zero() }
    fn clear_pool_reward_config() -> Weight { Weight::zero() }
    fn force_clear_pool_reward_config() -> Weight { Weight::zero() }
    fn pause_pool_reward() -> Weight { Weight::zero() }
    fn resume_pool_reward() -> Weight { Weight::zero() }
    fn set_global_pool_reward_paused() -> Weight { Weight::zero() }
    fn force_pause_pool_reward() -> Weight { Weight::zero() }
    fn force_resume_pool_reward() -> Weight { Weight::zero() }
    fn schedule_pool_reward_config_change() -> Weight { Weight::zero() }
    fn apply_pending_pool_reward_config() -> Weight { Weight::zero() }
    fn cancel_pending_pool_reward_config() -> Weight { Weight::zero() }
}
