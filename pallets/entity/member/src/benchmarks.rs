//! Benchmarks for pallet-entity-member
//!
//! 高风险调用真实 benchmark：register_member（含推荐链写入）、
//! remove_member（含推荐链递减 + 异步补偿）、batch_approve_members（循环注册）。
//!
//! 其余调用提供正确 setup 的骨架 benchmark。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use frame_support::{BoundedVec, traits::ConstU32};

/// Mock 环境常量（与 mock.rs 一致）
const BENCH_OWNER: u64 = 1;
const BENCH_SHOP: u64 = 1000;
const BENCH_ENTITY: u64 = 100;

/// 辅助：直接写入存储注册一个会员（绕过 extrinsic 权限检查）
fn seed_member<T: Config>(entity_id: u64, account: &T::AccountId, referrer: Option<T::AccountId>) {
    let now = frame_system::Pallet::<T>::block_number();
    let member = EntityMember {
        referrer: referrer.clone(),
        direct_referrals: 0,
        qualified_referrals: 0,
        indirect_referrals: 0,
        qualified_indirect_referrals: 0,
        team_size: 0,
        total_spent: 0u64,
        custom_level_id: 0,
        joined_at: now,
        last_active_at: now,
        activated: false,
        is_qualified_referral: true,
        banned_at: None,
        ban_reason: None,
    };
    EntityMembers::<T>::insert(entity_id, account, member);
    MemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));
    LevelMemberCount::<T>::mutate(entity_id, 0u8, |c| *c = c.saturating_add(1));

    if let Some(ref ref_account) = referrer {
        EntityMembers::<T>::mutate(entity_id, ref_account, |maybe| {
            if let Some(ref mut m) = maybe {
                m.direct_referrals = m.direct_referrals.saturating_add(1);
                m.qualified_referrals = m.qualified_referrals.saturating_add(1);
                m.team_size = m.team_size.saturating_add(1);
            }
        });
        let _ = DirectReferrals::<T>::try_mutate(entity_id, ref_account, |referrals| {
            referrals.try_push(account.clone()).map_err(|_| ())
        });
    }
}

/// 辅助：构造 AccountId
fn account_id<T: Config>(seed: u32) -> T::AccountId {
    account("member", seed, 0)
}

#[benchmarks(
    where T::AccountId: From<u64>,
)]
mod bench {
    use super::*;

    // ========================================================================
    // 高风险调用：register_member
    // 最坏情况：有推荐人 + 推荐链深度 MAX_SYNC_DEPTH（15 层同步更新）
    // ========================================================================

    #[benchmark]
    fn register_member() {
        let owner: T::AccountId = BENCH_OWNER.into();
        seed_member::<T>(BENCH_ENTITY, &owner, None);

        // 构建 13 层中间节点（owner 是第 1 层）
        let mut prev = owner;
        for i in 0..13u32 {
            let acc = account_id::<T>(i + 100);
            seed_member::<T>(BENCH_ENTITY, &acc, Some(prev.clone()));
            prev = acc;
        }

        // caller 将注册为第 15 层
        let caller = account_id::<T>(999);

        #[extrinsic_call]
        register_member(RawOrigin::Signed(caller), BENCH_SHOP, Some(prev));
    }

    // ========================================================================
    // 高风险调用：remove_member
    // 最坏情况：被移除会员有推荐人 + 推荐链深度 MAX_SYNC_DEPTH
    // ========================================================================

    #[benchmark]
    fn remove_member() {
        let owner: T::AccountId = BENCH_OWNER.into();
        seed_member::<T>(BENCH_ENTITY, &owner, None);

        let mut prev = owner.clone();
        for i in 0..13u32 {
            let acc = account_id::<T>(i + 200);
            seed_member::<T>(BENCH_ENTITY, &acc, Some(prev.clone()));
            prev = acc;
        }

        // target: 叶子节点（无下线，可被移除）
        let target = account_id::<T>(299);
        seed_member::<T>(BENCH_ENTITY, &target, Some(prev));

        #[extrinsic_call]
        remove_member(RawOrigin::Signed(owner), BENCH_SHOP, target);
    }

    // ========================================================================
    // 高风险调用：batch_approve_members
    // 最坏情况：50 个待审批会员全部通过
    // ========================================================================

    #[benchmark]
    fn batch_approve_members() {
        let owner: T::AccountId = BENCH_OWNER.into();

        EntityMemberPolicy::<T>::insert(
            BENCH_ENTITY,
            pallet_entity_common::MemberRegistrationPolicy(0b0000_0100),
        );

        let now = frame_system::Pallet::<T>::block_number();
        let mut accounts = alloc::vec::Vec::new();
        for i in 0..50u32 {
            let acc = account_id::<T>(i + 300);
            PendingMembers::<T>::insert(BENCH_ENTITY, &acc, (None::<T::AccountId>, now));
            accounts.push(acc);
        }

        #[extrinsic_call]
        batch_approve_members(RawOrigin::Signed(owner), BENCH_SHOP, accounts);
    }

    // ========================================================================
    // 其余调用
    // ========================================================================

    #[benchmark]
    fn bind_referrer() {
        let owner: T::AccountId = BENCH_OWNER.into();
        let caller = account_id::<T>(400);

        seed_member::<T>(BENCH_ENTITY, &owner, None);
        seed_member::<T>(BENCH_ENTITY, &caller, None);

        #[extrinsic_call]
        bind_referrer(RawOrigin::Signed(caller), BENCH_SHOP, owner);
    }

    #[benchmark]
    fn init_level_system() {
        let owner: T::AccountId = BENCH_OWNER.into();

        #[extrinsic_call]
        init_level_system(RawOrigin::Signed(owner), BENCH_SHOP, true, LevelUpgradeMode::AutoUpgrade);
    }

    #[benchmark]
    fn add_custom_level() {
        let owner: T::AccountId = BENCH_OWNER.into();

        EntityLevelSystems::<T>::insert(BENCH_ENTITY, EntityLevelSystem::<<T as Config>::MaxCustomLevels> {
            use_custom: true,
            upgrade_mode: LevelUpgradeMode::AutoUpgrade,
            levels: BoundedVec::default(),
        });

        let name: BoundedVec<u8, ConstU32<32>> = b"VIP1".to_vec().try_into().unwrap();

        #[extrinsic_call]
        add_custom_level(RawOrigin::Signed(owner), BENCH_SHOP, name, 1_000_000u64, 500u16, 100u16);
    }

    #[benchmark]
    fn manual_set_member_level() {
        let owner: T::AccountId = BENCH_OWNER.into();
        let target = account_id::<T>(401);

        let mut levels: BoundedVec<CustomLevel, <T as Config>::MaxCustomLevels> = BoundedVec::default();
        let _ = levels.try_push(CustomLevel {
            id: 0,
            name: b"VIP1".to_vec().try_into().unwrap_or_default(),
            threshold: 100u64,
            discount_rate: 0,
            commission_bonus: 0,
        });
        EntityLevelSystems::<T>::insert(BENCH_ENTITY, EntityLevelSystem {
            use_custom: true,
            upgrade_mode: LevelUpgradeMode::ManualUpgrade,
            levels,
        });
        seed_member::<T>(BENCH_ENTITY, &target, None);

        #[extrinsic_call]
        manual_set_member_level(RawOrigin::Signed(owner), BENCH_SHOP, target, 0u8);
    }

    #[benchmark]
    fn add_upgrade_rule() {
        let owner: T::AccountId = BENCH_OWNER.into();

        let mut levels: BoundedVec<CustomLevel, <T as Config>::MaxCustomLevels> = BoundedVec::default();
        let _ = levels.try_push(CustomLevel {
            id: 0,
            name: b"VIP1".to_vec().try_into().unwrap_or_default(),
            threshold: 100u64,
            discount_rate: 0,
            commission_bonus: 0,
        });
        EntityLevelSystems::<T>::insert(BENCH_ENTITY, EntityLevelSystem {
            use_custom: true,
            upgrade_mode: LevelUpgradeMode::AutoUpgrade,
            levels,
        });
        EntityUpgradeRules::<T>::insert(BENCH_ENTITY, EntityUpgradeRuleSystem::<frame_system::pallet_prelude::BlockNumberFor<T>, <T as Config>::MaxUpgradeRules> {
            enabled: true,
            conflict_strategy: ConflictStrategy::HighestLevel,
            rules: BoundedVec::default(),
            next_rule_id: 0,
        });

        let name: BoundedVec<u8, ConstU32<64>> = b"Rule1".to_vec().try_into().unwrap();

        #[extrinsic_call]
        add_upgrade_rule(
            RawOrigin::Signed(owner),
            BENCH_SHOP,
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
        let owner: T::AccountId = BENCH_OWNER.into();

        #[extrinsic_call]
        set_member_policy(RawOrigin::Signed(owner), BENCH_SHOP, 0u8);
    }

    #[benchmark]
    fn approve_member() {
        let owner: T::AccountId = BENCH_OWNER.into();
        let pending = account_id::<T>(402);

        let now = frame_system::Pallet::<T>::block_number();
        PendingMembers::<T>::insert(BENCH_ENTITY, &pending, (None::<T::AccountId>, now));

        #[extrinsic_call]
        approve_member(RawOrigin::Signed(owner), BENCH_SHOP, pending);
    }

    #[benchmark]
    fn ban_member() {
        let owner: T::AccountId = BENCH_OWNER.into();
        let target = account_id::<T>(403);

        seed_member::<T>(BENCH_ENTITY, &target, None);

        #[extrinsic_call]
        ban_member(RawOrigin::Signed(owner), BENCH_SHOP, target, None);
    }

    #[benchmark]
    fn cleanup_expired_pending() {
        let caller = account_id::<T>(404);

        let expired_block: frame_system::pallet_prelude::BlockNumberFor<T> = 1u32.into();
        for i in 0..10u32 {
            let acc = account_id::<T>(i + 500);
            PendingMembers::<T>::insert(BENCH_ENTITY, &acc, (None::<T::AccountId>, expired_block));
        }
        frame_system::Pallet::<T>::set_block_number(200u32.into());

        #[extrinsic_call]
        cleanup_expired_pending(RawOrigin::Signed(caller), BENCH_ENTITY, 10u32);
    }

    #[benchmark]
    fn leave_entity() {
        let caller = account_id::<T>(405);

        seed_member::<T>(BENCH_ENTITY, &caller, None);

        #[extrinsic_call]
        leave_entity(RawOrigin::Signed(caller), BENCH_ENTITY);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
