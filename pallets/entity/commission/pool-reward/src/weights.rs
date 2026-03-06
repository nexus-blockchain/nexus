// Weights for pallet-commission-pool-reward
//
// 基于 DB read/write 分析的合理估算，后续可通过 benchmark 框架替换为实测值。
//
// M1-R4 审计修复: 更新 DB 计数以反映 M1-R3/M2-R3 新增的 invalidate_current_round 操作
//
// | Extrinsic             | DB Reads | DB Writes | ref_time   | proof_size |
// |-----------------------|----------|-----------|------------|------------|
// | set_pool_reward_config  | 4 R      | 3 W       | 50M        | 6K         |
// | claim_pool_reward       | 10 R     | 7 W       | 150M       | 15K        |
// | force_new_round         | 7+N R   | 1 W        | 110M       | 11K        |
// | set_token_pool_enabled  | 4 R      | 3 W       | 45M        | 5K         |
// | clear_pool_reward_config| 4 R      | 4 W       | 40M        | 5K         |
// | pause_pool_reward       | 4 R      | 1 W       | 30M        | 4K         |
// | resume_pool_reward      | 4 R      | 1 W       | 30M        | 4K         |
// | set_global_paused       | 1 R      | 1 W       | 15M        | 2K         |

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn set_pool_reward_config() -> Weight;
    fn claim_pool_reward() -> Weight;
    fn force_new_round() -> Weight;
    fn set_token_pool_enabled() -> Weight;
    fn clear_pool_reward_config() -> Weight;
    fn pause_pool_reward() -> Weight;
    fn resume_pool_reward() -> Weight;
    fn set_global_pool_reward_paused() -> Weight;
}

/// 基于 DB 读写分析的估算权重
pub struct SubstrateWeight;

impl WeightInfo for SubstrateWeight {
    /// set_pool_reward_config:
    ///   reads: entity_active(1) + entity_owner(1) + existing_config(1) + CurrentRound in invalidate(1) = 4
    ///   writes: config_insert(1) + LastRoundId(1) + CurrentRound::remove(1) = 3
    fn set_pool_reward_config() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(3))
    }

    /// claim_pool_reward:
    ///   reads: entity_active(1) + is_member(1) + participation(1) +
    ///          config(1) + current_round(1) + pool_balance(1) + last_claimed(1) +
    ///          Currency::from_bal(1) + Currency::to_bal(1) = 9
    ///   writes: deduct_pool(1) + Currency::from_bal(1) + Currency::to_bal(1) +
    ///           current_round(1) + last_claimed(1) + claim_records(1) + token(0-1) = 7
    fn claim_pool_reward() -> Weight {
        Weight::from_parts(150_000_000, 15_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(10))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(7))
    }

    /// force_new_round:
    ///   reads: entity_active(1) + entity_owner(1) + config(1) + CurrentRound(1) + pool_balance(1)
    ///          + member_count_by_level(N, up to 10) + token_pool_balance(0-1) ≈ 7 base + N
    ///   writes: CurrentRound(1)
    fn force_new_round() -> Weight {
        Weight::from_parts(110_000_000, 11_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(7))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    /// set_token_pool_enabled:
    ///   reads: entity_active(1) + entity_owner(1) + config_mutate(1) + CurrentRound in invalidate(1) = 4
    ///   writes: config(1) + LastRoundId(1) + CurrentRound::remove(1) = 3
    fn set_token_pool_enabled() -> Weight {
        Weight::from_parts(45_000_000, 5_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(3))
    }

    /// clear_pool_reward_config:
    ///   reads: entity_active(1) + entity_owner(1) + config_exists(1) + CurrentRound in invalidate(1) = 4
    ///   writes: config_remove(1) + PoolRewardPaused::remove(1) + LastRoundId(1) + CurrentRound::remove(1) = 4
    fn clear_pool_reward_config() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(4))
    }

    /// pause_pool_reward:
    ///   reads: entity_active(1) + entity_owner(1) + config_exists(1) + paused_check(1) = 4
    ///   writes: PoolRewardPaused::insert(1) = 1
    fn pause_pool_reward() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    /// resume_pool_reward:
    ///   reads: entity_active(1) + entity_owner(1) + config_exists(1) + paused_check(1) = 4
    ///   writes: PoolRewardPaused::remove(1) = 1
    fn resume_pool_reward() -> Weight {
        Weight::from_parts(30_000_000, 4_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(4))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }

    /// set_global_pool_reward_paused:
    ///   reads: GlobalPoolRewardPaused(1) = 1
    ///   writes: GlobalPoolRewardPaused::put/kill(1) = 1
    fn set_global_pool_reward_paused() -> Weight {
        Weight::from_parts(15_000_000, 2_000)
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().reads(1))
            .saturating_add(frame_support::weights::constants::RocksDbWeight::get().writes(1))
    }
}

/// 测试用：统一返回零权重
impl WeightInfo for () {
    fn set_pool_reward_config() -> Weight { Weight::zero() }
    fn claim_pool_reward() -> Weight { Weight::zero() }
    fn force_new_round() -> Weight { Weight::zero() }
    fn set_token_pool_enabled() -> Weight { Weight::zero() }
    fn clear_pool_reward_config() -> Weight { Weight::zero() }
    fn pause_pool_reward() -> Weight { Weight::zero() }
    fn resume_pool_reward() -> Weight { Weight::zero() }
    fn set_global_pool_reward_paused() -> Weight { Weight::zero() }
}
