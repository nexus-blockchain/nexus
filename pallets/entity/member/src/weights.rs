//! Weights for pallet-entity-member
//!
//! Benchmarks not yet generated — using estimated values from existing extrinsics.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::weights::Weight;
use core::marker::PhantomData;

/// Weight functions needed for pallet-entity-member.
pub trait WeightInfo {
    fn register_member() -> Weight;
    fn bind_referrer() -> Weight;
    fn init_level_system() -> Weight;
    fn add_custom_level() -> Weight;
    fn update_custom_level() -> Weight;
    fn remove_custom_level() -> Weight;
    fn manual_set_member_level() -> Weight;
    fn set_use_custom_levels() -> Weight;
    fn set_upgrade_mode() -> Weight;
    fn init_upgrade_rule_system() -> Weight;
    fn add_upgrade_rule() -> Weight;
    fn update_upgrade_rule() -> Weight;
    fn remove_upgrade_rule() -> Weight;
    fn set_upgrade_rule_system_enabled() -> Weight;
    fn set_conflict_strategy() -> Weight;
    fn set_member_policy() -> Weight;
    fn approve_member() -> Weight;
    fn reject_member() -> Weight;
    fn set_member_stats_policy() -> Weight;
    fn cancel_pending_member() -> Weight;
    fn cleanup_expired_pending() -> Weight;
    fn batch_approve_members() -> Weight;
    fn batch_reject_members() -> Weight;
    fn ban_member() -> Weight;
    fn unban_member() -> Weight;
    fn remove_member() -> Weight;
    fn reset_level_system() -> Weight;
    fn reset_upgrade_rule_system() -> Weight;
    fn leave_entity() -> Weight;
    fn activate_member() -> Weight;
    fn deactivate_member() -> Weight;
}

/// Substrate weight estimates (pre-benchmark).
/// Uses the same values as the original hardcoded extrinsics.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn register_member() -> Weight {
        Weight::from_parts(375_000_000, 12_000)
    }
    fn bind_referrer() -> Weight {
        Weight::from_parts(400_000_000, 16_000)
    }
    fn init_level_system() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn add_custom_level() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn update_custom_level() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn remove_custom_level() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn manual_set_member_level() -> Weight {
        Weight::from_parts(175_000_000, 12_000)
    }
    fn set_use_custom_levels() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_upgrade_mode() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn init_upgrade_rule_system() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn add_upgrade_rule() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn update_upgrade_rule() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn remove_upgrade_rule() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_upgrade_rule_system_enabled() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_conflict_strategy() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_member_policy() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn approve_member() -> Weight {
        Weight::from_parts(375_000_000, 12_000)
    }
    fn reject_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_member_stats_policy() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn cancel_pending_member() -> Weight {
        Weight::from_parts(100_000_000, 6_000)
    }
    fn cleanup_expired_pending() -> Weight {
        Weight::from_parts(500_000_000, 20_000)
    }
    fn batch_approve_members() -> Weight {
        Weight::from_parts(500_000_000, 30_000)
    }
    fn batch_reject_members() -> Weight {
        Weight::from_parts(500_000_000, 30_000)
    }
    fn ban_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn unban_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn remove_member() -> Weight {
        Weight::from_parts(500_000_000, 20_000)
    }
    fn reset_level_system() -> Weight {
        Weight::from_parts(200_000_000, 12_000)
    }
    fn reset_upgrade_rule_system() -> Weight {
        Weight::from_parts(200_000_000, 12_000)
    }
    fn leave_entity() -> Weight {
        Weight::from_parts(500_000_000, 20_000)
    }
    fn activate_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn deactivate_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
}

/// Unit weight for testing.
impl WeightInfo for () {
    fn register_member() -> Weight {
        Weight::from_parts(375_000_000, 12_000)
    }
    fn bind_referrer() -> Weight {
        Weight::from_parts(400_000_000, 16_000)
    }
    fn init_level_system() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn add_custom_level() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn update_custom_level() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn remove_custom_level() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn manual_set_member_level() -> Weight {
        Weight::from_parts(175_000_000, 12_000)
    }
    fn set_use_custom_levels() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_upgrade_mode() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn init_upgrade_rule_system() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn add_upgrade_rule() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn update_upgrade_rule() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn remove_upgrade_rule() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_upgrade_rule_system_enabled() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_conflict_strategy() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_member_policy() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn approve_member() -> Weight {
        Weight::from_parts(375_000_000, 12_000)
    }
    fn reject_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn set_member_stats_policy() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn cancel_pending_member() -> Weight {
        Weight::from_parts(100_000_000, 6_000)
    }
    fn cleanup_expired_pending() -> Weight {
        Weight::from_parts(500_000_000, 20_000)
    }
    fn batch_approve_members() -> Weight {
        Weight::from_parts(500_000_000, 30_000)
    }
    fn batch_reject_members() -> Weight {
        Weight::from_parts(500_000_000, 30_000)
    }
    fn ban_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn unban_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn remove_member() -> Weight {
        Weight::from_parts(500_000_000, 20_000)
    }
    fn reset_level_system() -> Weight {
        Weight::from_parts(200_000_000, 12_000)
    }
    fn reset_upgrade_rule_system() -> Weight {
        Weight::from_parts(200_000_000, 12_000)
    }
    fn leave_entity() -> Weight {
        Weight::from_parts(500_000_000, 20_000)
    }
    fn activate_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
    fn deactivate_member() -> Weight {
        Weight::from_parts(150_000_000, 8_000)
    }
}
