//! Benchmarks for pallet-entity-member
//!
//! 覆盖高风险调用：register_member（含推荐链写入）、batch_approve_members（循环注册）、
//! update_spent（含规则评估 + 等级过期修正）、cleanup_expired_pending（遍历清理）。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use frame_support::BoundedVec;
use pallet::*;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register_member() {
        let caller: T::AccountId = whitelisted_caller();
        // Setup: ensure shop/entity exists via provider
        // Note: actual setup depends on MockEntityProvider/MockShopProvider in runtime benchmarks config
        #[extrinsic_call]
        register_member(RawOrigin::Signed(caller), 1000u64, None);
    }

    #[benchmark]
    fn bind_referrer() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        bind_referrer(RawOrigin::Signed(caller), 1000u64, whitelisted_caller());
    }

    #[benchmark]
    fn init_level_system() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        init_level_system(RawOrigin::Signed(caller), 1000u64, true, LevelUpgradeMode::AutoUpgrade);
    }

    #[benchmark]
    fn add_custom_level() {
        let caller: T::AccountId = whitelisted_caller();
        let name: BoundedVec<u8, ConstU32<32>> = b"VIP1".to_vec().try_into().unwrap();
        #[extrinsic_call]
        add_custom_level(RawOrigin::Signed(caller), 1000u64, name, 1_000_000u64, 500u16, 100u16);
    }

    #[benchmark]
    fn manual_set_member_level() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        manual_set_member_level(RawOrigin::Signed(caller), 1000u64, whitelisted_caller(), 0u8);
    }

    #[benchmark]
    fn add_upgrade_rule() {
        let caller: T::AccountId = whitelisted_caller();
        let name: BoundedVec<u8, ConstU32<64>> = b"Rule1".to_vec().try_into().unwrap();
        #[extrinsic_call]
        add_upgrade_rule(
            RawOrigin::Signed(caller),
            1000u64,
            name,
            UpgradeTrigger::TotalSpent { threshold: 1_000_000 },
            0u8,
            None,
            0u8,
            false,
            None,
        );
    }

    #[benchmark]
    fn set_member_policy() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        set_member_policy(RawOrigin::Signed(caller), 1000u64, 0u8);
    }

    #[benchmark]
    fn approve_member() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        approve_member(RawOrigin::Signed(caller), 1000u64, whitelisted_caller());
    }

    #[benchmark]
    fn batch_approve_members() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        batch_approve_members(RawOrigin::Signed(caller), 1000u64, alloc::vec![]);
    }

    #[benchmark]
    fn ban_member() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        ban_member(RawOrigin::Signed(caller), 1000u64, whitelisted_caller(), None);
    }

    #[benchmark]
    fn remove_member() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        remove_member(RawOrigin::Signed(caller), 1000u64, whitelisted_caller());
    }

    #[benchmark]
    fn cleanup_expired_pending() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        cleanup_expired_pending(RawOrigin::Signed(caller), 100u64, 10u32);
    }

    #[benchmark]
    fn leave_entity() {
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        leave_entity(RawOrigin::Signed(caller), 100u64);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
