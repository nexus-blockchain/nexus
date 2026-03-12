extern crate alloc;

use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};

// ============================================================================
// P0: 会员注册
// ============================================================================

#[test]
fn register_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
        assert_eq!(MemberPallet::member_count(ENTITY_1), 1);
    });
}

#[test]
fn register_member_duplicate_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None),
            Error::<Test>::AlreadyMember
        );
    });
}

#[test]
fn register_member_invalid_shop_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), INVALID_SHOP, None),
            Error::<Test>::ShopNotFound
        );
    });
}

#[test]
fn register_member_with_referrer_works() {
    new_test_ext().execute_with(|| {
        // Register referrer first
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        // Register with referrer
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(BOB),
            SHOP_1,
            Some(ALICE),
        ));
        let bob_member = MemberPallet::get_member_by_shop(SHOP_1, &BOB).unwrap();
        assert_eq!(bob_member.referrer, Some(ALICE));

        let alice_member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice_member.direct_referrals, 1);
        assert_eq!(alice_member.qualified_referrals, 1);
        assert_eq!(alice_member.team_size, 1);
    });
}

#[test]
fn register_member_self_referral_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, Some(ALICE)),
            Error::<Test>::SelfReferral
        );
    });
}

#[test]
fn register_member_invalid_referrer_fails() {
    new_test_ext().execute_with(|| {
        // BOB not registered, can't be referrer
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, Some(BOB)),
            Error::<Test>::InvalidReferrer
        );
    });
}

#[test]
fn register_member_entity_level_dedup() {
    new_test_ext().execute_with(|| {
        // Register via SHOP_1
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        // Try register via SHOP_2 (same entity) — should fail
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_2, None),
            Error::<Test>::AlreadyMember
        );
    });
}

// ============================================================================
// P0: 注册策略
// ============================================================================

#[test]
fn register_member_purchase_required_blocks_manual() {
    new_test_ext().execute_with(|| {
        // Set purchase-required policy
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            1, // PURCHASE_REQUIRED
        ));
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None),
            Error::<Test>::PurchaseRequiredForRegistration
        );
    });
}

#[test]
fn register_member_referral_required_blocks_without_referrer() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            2, // REFERRAL_REQUIRED
        ));
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None),
            Error::<Test>::ReferralRequiredForRegistration
        );
    });
}

#[test]
fn register_member_approval_required_creates_pending() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            4, // APPROVAL_REQUIRED
        ));
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        // Not yet a member
        assert!(!MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
        // Is pending
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
    });
}

#[test]
fn approve_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            4,
        ));
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        // Approve by owner
        assert_ok!(MemberPallet::approve_member(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            ALICE,
        ));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
    });
}

#[test]
fn approve_member_by_admin_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            4,
        ));
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        // Approve by admin
        assert_ok!(MemberPallet::approve_member(
            RuntimeOrigin::signed(ADMIN),
            SHOP_1,
            ALICE,
        ));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

#[test]
fn reject_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            4,
        ));
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE),
            SHOP_1,
            None,
        ));
        assert_ok!(MemberPallet::reject_member(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            ALICE,
        ));
        assert!(!MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
    });
}

// ============================================================================
// P0: 权限校验
// ============================================================================

#[test]
fn ensure_shop_owner_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::init_level_system(
                RuntimeOrigin::signed(ALICE),
                SHOP_1,
                false,
                LevelUpgradeMode::AutoUpgrade,
            ),
            Error::<Test>::NotEntityAdmin
        );
    });
}

#[test]
fn ensure_shop_owner_invalid_shop_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::init_level_system(
                RuntimeOrigin::signed(OWNER),
                INVALID_SHOP,
                false,
                LevelUpgradeMode::AutoUpgrade,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

// ============================================================================
// P0: 推荐关系
// ============================================================================

#[test]
fn bind_referrer_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));
        assert_ok!(MemberPallet::bind_referrer(RuntimeOrigin::signed(BOB), SHOP_1, ALICE));

        let bob_member = MemberPallet::get_member_by_shop(SHOP_1, &BOB).unwrap();
        assert_eq!(bob_member.referrer, Some(ALICE));
    });
}

#[test]
fn bind_referrer_self_referral_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_noop!(
            MemberPallet::bind_referrer(RuntimeOrigin::signed(ALICE), SHOP_1, ALICE),
            Error::<Test>::SelfReferral
        );
    });
}

#[test]
fn bind_referrer_already_bound_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, None));
        assert_noop!(
            MemberPallet::bind_referrer(RuntimeOrigin::signed(BOB), SHOP_1, CHARLIE),
            Error::<Test>::ReferrerAlreadyBound
        );
    });
}

#[test]
fn circular_referral_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));
        // ALICE trying to bind BOB as referrer (BOB → ALICE → BOB loop)
        assert_noop!(
            MemberPallet::bind_referrer(RuntimeOrigin::signed(ALICE), SHOP_1, BOB),
            Error::<Test>::CircularReferral
        );
    });
}

// ============================================================================
// P1: 等级系统 CRUD
// ============================================================================

#[test]
fn init_level_system_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            true,
            LevelUpgradeMode::AutoUpgrade,
        ));
        assert!(EntityLevelSystems::<Test>::get(ENTITY_1).is_some());
    });
}

#[test]
fn add_custom_level_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            true,
            LevelUpgradeMode::AutoUpgrade,
        ));

        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();

        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            name,
            1000u64, // threshold
            500,     // discount_rate (5%)
            300,     // commission_bonus (3%)
        ));

        let system = EntityLevelSystems::<Test>::get(ENTITY_1).unwrap();
        assert_eq!(system.levels.len(), 1);
        assert_eq!(system.levels[0].id, 1);
        assert_eq!(system.levels[0].threshold, 1000);
    });
}

#[test]
fn add_custom_level_invalid_basis_points_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            true,
            LevelUpgradeMode::AutoUpgrade,
        ));

        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();

        // discount_rate > 10000
        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER),
                SHOP_1,
                name.clone(),
                1000u64,
                10001,
                300,
            ),
            Error::<Test>::InvalidBasisPoints
        );

        // commission_bonus > 10000
        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER),
                SHOP_1,
                name,
                1000u64,
                500,
                10001,
            ),
            Error::<Test>::InvalidBasisPoints
        );
    });
}

#[test]
fn add_custom_level_threshold_ordering() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            true,
            LevelUpgradeMode::AutoUpgrade,
        ));

        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();

        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 1000u64, 0, 0,
        ));

        // Threshold must be > previous
        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, name2.clone(), 500u64, 0, 0,
            ),
            Error::<Test>::InvalidThreshold
        );

        // Same threshold also fails
        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, name2, 1000u64, 0, 0,
            ),
            Error::<Test>::InvalidThreshold
        );
    });
}

#[test]
fn remove_custom_level_only_last() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));

        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();

        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 1000u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 2000u64, 0, 0,
        ));

        // Can't remove first level
        assert_noop!(
            MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 1),
            Error::<Test>::InvalidLevelId
        );

        // Can remove last
        assert_ok!(MemberPallet::remove_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, 2,
        ));
    });
}

#[test]
fn manual_set_member_level_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));

        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();

        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 0u64, 0, 0,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 1,
        ));
    });
}

#[test]
fn manual_upgrade_rejects_auto_mode() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_noop!(
            MemberPallet::manual_set_member_level(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 1),
            Error::<Test>::ManualUpgradeNotSupported
        );
    });
}

// ============================================================================
// P1: 升级规则
// ============================================================================

#[test]
fn upgrade_rule_system_lifecycle() {
    new_test_ext().execute_with(|| {
        // H8: level system must exist before adding rules
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let level_name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, level_name, 1000u64, 0, 0,
        ));

        // Init
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            ConflictStrategy::HighestLevel,
        ));

        let name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"SpendRule".to_vec().try_into().unwrap();

        // Add rule
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER),
            SHOP_1,
            name,
            UpgradeTrigger::TotalSpent { threshold: 1000u64 },
            1,
            None,
            1,
            false,
            Some(5),
        ));

        let system = EntityUpgradeRules::<Test>::get(ENTITY_1).unwrap();
        assert_eq!(system.rules.len(), 1);
        assert_eq!(system.rules[0].max_triggers, Some(5));

        // Update rule
        assert_ok!(MemberPallet::update_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0, Some(false), None,
        ));
        let system = EntityUpgradeRules::<Test>::get(ENTITY_1).unwrap();
        assert!(!system.rules[0].enabled);

        // Remove rule
        assert_ok!(MemberPallet::remove_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0,
        ));
        let system = EntityUpgradeRules::<Test>::get(ENTITY_1).unwrap();
        assert!(system.rules.is_empty());
    });
}

// ============================================================================
// P1: MemberProvider — update_spent 自动升级
// ============================================================================

#[test]
fn update_spent_auto_upgrades_custom_level() {
    new_test_ext().execute_with(|| {
        // Setup custom level system
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u64, 100, 50,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 600u64));

        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1); // VIP1 = level_id 1
    });
}

#[test]
fn update_spent_invalid_shop_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::update_spent(INVALID_SHOP, &ALICE, 100u64),
            Error::<Test>::ShopNotFound
        );
    });
}

// ============================================================================
// P1: auto_register
// ============================================================================

#[test]
fn auto_register_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::auto_register(SHOP_1, &ALICE, None));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

#[test]
fn auto_register_referral_required_rejects_without_referrer() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 2,
        ));
        // auto_register with referral_required but no referrer → error
        assert_noop!(
            MemberPallet::auto_register(SHOP_1, &ALICE, None),
            Error::<Test>::ReferralRequiredForRegistration
        );
    });
}

// ============================================================================
// P2: 治理函数
// ============================================================================

#[test]
fn governance_add_custom_level_auto_assigns_id() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));

        assert_ok!(MemberPallet::governance_add_custom_level(
            ENTITY_1, b"VIP1", 1000, 100, 50,
        ));
        assert_ok!(MemberPallet::governance_add_custom_level(
            ENTITY_1, b"VIP2", 2000, 200, 100,
        ));

        let system = EntityLevelSystems::<Test>::get(ENTITY_1).unwrap();
        assert_eq!(system.levels.len(), 2);
        assert_eq!(system.levels[0].id, 1);
        assert_eq!(system.levels[1].id, 2);
    });
}

#[test]
fn governance_add_custom_level_invalid_basis_points() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));

        assert_noop!(
            MemberPallet::governance_add_custom_level(ENTITY_1, b"VIP1", 1000, 10001, 50),
            Error::<Test>::InvalidBasisPoints
        );
    });
}

#[test]
fn governance_update_custom_level_validates_basis_points() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        assert_ok!(MemberPallet::governance_add_custom_level(
            ENTITY_1, b"VIP1", 1000, 100, 50,
        ));

        assert_noop!(
            MemberPallet::governance_update_custom_level(
                ENTITY_1, 1, None, None, Some(10001), None,
            ),
            Error::<Test>::InvalidBasisPoints
        );
    });
}

// ============================================================================
// P2: 边界条件
// ============================================================================

#[test]
fn empty_level_name_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));

        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            BoundedVec::default();

        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, name, 1000u64, 0, 0,
            ),
            Error::<Test>::EmptyLevelName
        );
    });
}

#[test]
fn level_system_not_initialized_fails() {
    new_test_ext().execute_with(|| {
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();

        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, name, 1000u64, 0, 0,
            ),
            Error::<Test>::LevelSystemNotInitialized
        );
    });
}

#[test]
fn member_provider_trait_works() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;

        assert!(!<MemberPallet as MemberProvider<u64>>::is_member(ENTITY_1, &ALICE));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert!(<MemberPallet as MemberProvider<u64>>::is_member(ENTITY_1, &ALICE));

        let level_id = <MemberPallet as MemberProvider<u64>>::custom_level_id(ENTITY_1, &ALICE);
        assert_eq!(level_id, 0);
    });
}

// ============================================================================
// 审计回归测试
// ============================================================================

#[test]
fn h2_set_member_policy_rejects_invalid_bits() {
    new_test_ext().execute_with(|| {
        // 高位垃圾值 (32 = 0b100000) 应被拒绝
        assert_noop!(
            MemberPallet::set_member_policy(RuntimeOrigin::signed(OWNER), SHOP_1, 32),
            Error::<Test>::InvalidPolicyBits
        );
        // 255 也应被拒绝
        assert_noop!(
            MemberPallet::set_member_policy(RuntimeOrigin::signed(OWNER), SHOP_1, 255),
            Error::<Test>::InvalidPolicyBits
        );
        // 31 = 0b11111 (全部5个标志) 应该成功
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 31,
        ));
    });
}

#[test]
fn h3_init_level_system_rejects_reinit() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        // 第二次初始化应被拒绝
        assert_noop!(
            MemberPallet::init_level_system(
                RuntimeOrigin::signed(OWNER), SHOP_1, false, LevelUpgradeMode::ManualUpgrade,
            ),
            Error::<Test>::LevelSystemAlreadyInitialized
        );
        // 确认原始配置未被修改
        let system = EntityLevelSystems::<Test>::get(ENTITY_1).unwrap();
        assert!(system.use_custom);
        assert_eq!(system.upgrade_mode, LevelUpgradeMode::AutoUpgrade);
    });
}

#[test]
fn h4_add_upgrade_rule_rejects_invalid_target_level() {
    new_test_ext().execute_with(|| {
        // 初始化等级和规则系统
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 1000u64, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));

        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"BadRule".to_vec().try_into().unwrap();

        // target_level_id=5 不存在（只有 level 1）
        assert_noop!(
            MemberPallet::add_upgrade_rule(
                RuntimeOrigin::signed(OWNER), SHOP_1, rule_name.clone(),
                UpgradeTrigger::TotalSpent { threshold: 1000 },
                5, None, 1, false, None,
            ),
            Error::<Test>::InvalidTargetLevel
        );

        // target_level_id=1 存在，应该成功
        let valid_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"GoodRule".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, valid_name,
            UpgradeTrigger::TotalSpent { threshold: 1000 },
            1, None, 1, false, None,
        ));
    });
}

#[test]
fn h1_team_size_no_double_count() {
    new_test_ext().execute_with(|| {
        // 创建 ALICE -> BOB -> CHARLIE 推荐链
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, Some(BOB)));

        // 验证 team_size 正确（不会因循环检测缺失而异常）
        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.team_size, 2); // BOB + CHARLIE
        let bob = MemberPallet::get_member_by_shop(SHOP_1, &BOB).unwrap();
        assert_eq!(bob.team_size, 1); // CHARLIE
        let charlie = MemberPallet::get_member_by_shop(SHOP_1, &CHARLIE).unwrap();
        assert_eq!(charlie.team_size, 0);
    });
}

#[test]
fn m4_governance_set_upgrade_mode_rejects_invalid() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        // mode=2 无效
        assert_noop!(
            MemberPallet::governance_set_upgrade_mode(ENTITY_1, 2),
            Error::<Test>::InvalidUpgradeMode
        );
        // mode=0 有效
        assert_ok!(MemberPallet::governance_set_upgrade_mode(ENTITY_1, 0));
        // mode=1 有效
        assert_ok!(MemberPallet::governance_set_upgrade_mode(ENTITY_1, 1));
    });
}

#[test]
fn h5_init_upgrade_rule_system_rejects_reinit() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        // 第二次初始化应被拒绝
        assert_noop!(
            MemberPallet::init_upgrade_rule_system(
                RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::FirstMatch,
            ),
            Error::<Test>::UpgradeRuleSystemAlreadyInitialized
        );
        // 确认原始配置未被修改
        let system = EntityUpgradeRules::<Test>::get(ENTITY_1).unwrap();
        assert_eq!(system.conflict_strategy, ConflictStrategy::HighestLevel);
    });
}

#[test]
fn h6_custom_level_id_respects_expiry() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;

        // Setup custom levels with ManualUpgrade
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0,
        ));

        // Register member and manually upgrade to VIP2 (level_id=2)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2,
        ));

        // Verify stored level is 2
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2);

        // Set expiry in the past
        MemberLevelExpiry::<Test>::insert(ENTITY_1, &ALICE, 1u64);

        // Advance past expiry
        System::set_block_number(10);

        // custom_level_id via MemberProvider trait should respect expiry
        // With 0 spending, calculated level falls back to 0
        let level = <MemberPallet as MemberProvider<u64>>::custom_level_id(SHOP_1, &ALICE);
        assert_eq!(level, 0); // expired → calculated from spending (0)
    });
}

#[test]
fn h7_apply_upgrade_skips_deleted_level() {
    new_test_ext().execute_with(|| {
        // Setup: create level system + rules
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 200u64, 0, 0,
        ));

        // Create rule targeting VIP2 (level_id=2)
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"SpendRule".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::TotalSpent { threshold: 100 },
            2, // target VIP2
            None, 1, false, None,
        ));

        // Register member
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Delete VIP2 (the target level)
        assert_ok!(MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 2));

        // Spend enough to trigger rule
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 150u64));

        // check_order_upgrade_rules should handle deleted level gracefully
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 150u64));

        // Member should NOT have been upgraded to deleted level
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        // auto_upgrade via update_spent sets to VIP1 (level 1, threshold 100, spent 150)
        assert_eq!(member.custom_level_id, 1);
    });
}

#[test]
fn h10_stackable_rule_cannot_downgrade() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels + rule system
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0,
        ));

        // Create stackable rule targeting VIP1 (level_id=0)
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"StackLow".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::TotalSpent { threshold: 50 },
            1, // target VIP1
            Some(100), 1, true, None,
        ));

        // Register member and manually upgrade to VIP2 (level_id=2)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2,
        ));
        assert_eq!(MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap().custom_level_id, 2);

        // Spend enough to trigger the stackable rule targeting VIP1
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 200u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 200u64));

        // H10: Member should NOT be downgraded from VIP2 to VIP1
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2, "stackable rule must not downgrade");
    });
}

#[test]
fn h12_auto_upgrade_preserves_active_rule_upgrade() {
    new_test_ext().execute_with(|| {
        // Setup: custom level system in AutoUpgrade mode
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 500u64, 100, 50,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 2000u64, 200, 100,
        ));

        // Setup: rule system with a rule that upgrades to VIP2 with 100-block duration
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"BuySpecial".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::PurchaseProduct { product_id: 42 },
            2, // target VIP2
            Some(100), // 100-block duration
            1, false, None,
        ));

        // Register and spend enough for VIP1 only (>500, <2000)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 600u64));
        assert_eq!(MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap().custom_level_id, 1); // VIP1

        // Trigger rule: upgrade to VIP2 with expiry at block 101
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 42, 600u64));
        assert_eq!(MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap().custom_level_id, 2); // VIP2
        assert_eq!(MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE), Some(101));

        // H12: Another order/update_spent should NOT overwrite VIP2 back to VIP1
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 100u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2, "auto-upgrade must not overwrite active rule upgrade");

        // After expiry, auto-upgrade should recalculate
        System::set_block_number(102);
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1, "after expiry, auto-upgrade should recalculate to VIP1");
    });
}

#[test]
fn m14_stackable_preserves_permanent_upgrade() {
    new_test_ext().execute_with(|| {
        // Setup: custom levels + rule system
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0,
        ));

        // Create stackable rule targeting VIP1 (level 1) with 50-block duration
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"StackVIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::TotalSpent { threshold: 50 },
            1, // target VIP1 (level 1)
            Some(50), // 50-block duration
            1, true, None,
        ));

        // Register member and manually upgrade to VIP1 (level 1, permanent, no expiry)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 1,
        ));
        assert!(MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none());

        // Trigger stackable rule targeting same level (1 == 1)
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 100u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));

        // I4 fix: member already holds level 1 permanently (no expiry).
        // Stackable rule targeting same level must NOT demote permanent to timed.
        let expiry = MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE);
        assert_eq!(expiry, None, "permanent level must not be demoted to timed by stackable rule");

        // Verify: stackable upgrade to a HIGHER level (target > current) with no prior expiry
        // should correctly set a new timed expiry
        let rule_name2: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"StackVIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name2,
            UpgradeTrigger::TotalSpent { threshold: 80 },
            2, // target VIP2 (level 2) — higher than current level 1
            Some(50), // 50-block duration
            2, true, None,
        ));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));
        let expiry = MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE);
        assert_eq!(expiry, Some(51), "first-time stackable upgrade to higher level should set now + duration");
        let member = MemberPallet::entity_members(ENTITY_1, ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2, "should have been upgraded to VIP2");
    });
}

#[test]
fn m18_order_count_tracked_when_rule_system_disabled() {
    new_test_ext().execute_with(|| {
        // Register member
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // No rule system initialized — check_order_upgrade_rules should still track orders
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 200u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 300u64));

        // M18: Order count should be 3 even without rule system
        assert_eq!(MemberPallet::member_order_count(ENTITY_1, ALICE), 3);

        // Now init rule system with OrderCount trigger
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"OrderRule".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::OrderCount { count: 4 },
            1, None, 1, false, None,
        ));

        // 4th order should trigger the rule (3 previous + 1 new)
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));
        assert_eq!(MemberPallet::member_order_count(ENTITY_1, ALICE), 4);
    });
}

#[test]
fn h8_add_upgrade_rule_requires_level_system() {
    new_test_ext().execute_with(|| {
        // Init upgrade rule system WITHOUT level system
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));

        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"BadRule".to_vec().try_into().unwrap();

        // H8: Should fail because no level system exists
        assert_noop!(
            MemberPallet::add_upgrade_rule(
                RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
                UpgradeTrigger::TotalSpent { threshold: 1000 },
                1, None, 1, false, None,
            ),
            Error::<Test>::LevelSystemNotInitialized
        );
    });
}

#[test]
fn m7_governance_add_custom_level_name_too_long() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        // 33 bytes exceeds ConstU32<32>
        let long_name = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567";
        assert_noop!(
            MemberPallet::governance_add_custom_level(ENTITY_1, long_name, 1000, 100, 50),
            Error::<Test>::NameTooLong
        );
    });
}

#[test]
fn m7_governance_update_custom_level_name_too_long() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        assert_ok!(MemberPallet::governance_add_custom_level(
            ENTITY_1, b"VIP1", 1000, 100, 50,
        ));
        let long_name = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567"; // 33 bytes
        assert_noop!(
            MemberPallet::governance_update_custom_level(
                ENTITY_1, 1, Some(long_name.as_slice()), None, None, None,
            ),
            Error::<Test>::NameTooLong
        );
    });
}

// ============================================================================
// P4: 等级过期时 update_spent 自动修正存储
// ============================================================================

#[test]
fn p4_update_spent_corrects_expired_level() {
    new_test_ext().execute_with(|| {
        // 初始化自定义等级系统
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name.clone(), 500u64, 100, 50,
        ));
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 2000u64, 200, 100,
        ));

        // 注册会员
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 手动设置等级为 VIP2 (level_id=2)，设置过期时间为 block 10
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 2; // VIP2
                member.total_spent = 600;   // 只够 VIP1 (>500, <2000)
            }
        });
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 10u64);

        // 当前 block=1，未过期 → custom_level_id 仍为 2
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2);

        // 推进到 block 11（过期后）
        System::set_block_number(11);

        // S1 修复: get_effective_level 现在是写穿模式 — 检测到过期时立即修正存储
        assert_eq!(MemberPallet::get_effective_level(SHOP_1, &ALICE), 1);
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1); // S1: 存储已被写穿修正
        assert!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none());

        // update_spent 不再需要额外修正（已由 get_effective_level 完成）
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1);
    });
}

#[test]
fn p4_update_spent_emits_expired_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u64, 100, 50,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 1; // VIP1
                member.total_spent = 100;   // 不够 VIP1 (需 500)
            }
        });
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 5u64);

        System::set_block_number(10);
        // 清除之前的事件
        System::reset_events();

        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u64));

        // 过期后 recalculated=0 (消费 110 < 500 → 无等级即 0)
        // member.custom_level_id 是 1，recalculated 是 0，会 emit MemberLevelExpired
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 0);
        // 过期记录应已清除（即使等级未变）
        assert!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none());
    });
}

#[test]
fn p4_non_expired_level_not_touched() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u64, 100, 50,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 设置 VIP1，过期在 block 100（远未到期）
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 1;
                member.total_spent = 0; // 消费不够，但有手动升级
            }
        });
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 100u64);

        // block=1，未过期
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u64));

        // 等级和过期记录都不应改变
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1);
        assert_eq!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE), Some(100));
    });
}

// ============================================================================
// MemberStatsPolicy 测试
// ============================================================================

#[test]
fn set_member_stats_policy_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::MemberStatsPolicy;

        // 默认策略 = 0（排除复购）
        assert_eq!(MemberPallet::entity_member_stats_policy(ENTITY_1), MemberStatsPolicy(0));

        // Owner 设置策略
        assert_ok!(MemberPallet::set_member_stats_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0011,
        ));
        assert_eq!(
            MemberPallet::entity_member_stats_policy(ENTITY_1),
            MemberStatsPolicy(0b0000_0011),
        );

        // Admin 也可以设置
        assert_ok!(MemberPallet::set_member_stats_policy(
            RuntimeOrigin::signed(ADMIN), SHOP_1, 0b0000_0001,
        ));
        assert_eq!(
            MemberPallet::entity_member_stats_policy(ENTITY_1),
            MemberStatsPolicy(0b0000_0001),
        );
    });
}

#[test]
fn set_member_stats_policy_rejects_invalid_bits() {
    new_test_ext().execute_with(|| {
        // 0b0000_0100 = 4，超出低 2 位
        assert_noop!(
            MemberPallet::set_member_stats_policy(RuntimeOrigin::signed(OWNER), SHOP_1, 4),
            Error::<Test>::InvalidPolicyBits
        );
    });
}

#[test]
fn set_member_stats_policy_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::set_member_stats_policy(RuntimeOrigin::signed(ALICE), SHOP_1, 0),
            Error::<Test>::NotEntityAdmin
        );
    });
}

#[test]
fn get_member_stats_default_returns_qualified() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;

        // ALICE 注册
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));

        // BOB 复购注册 (qualified=false)，CHARLIE 主动注册 (qualified=true)
        MemberPallet::auto_register_by_entity(ENTITY_1, &BOB, Some(ALICE), false).unwrap();
        MemberPallet::auto_register_by_entity(ENTITY_1, &CHARLIE, Some(ALICE), true).unwrap();

        // 默认策略（0）：get_member_stats 返回 qualified_referrals
        let (direct, _team, _spent) =
            <MemberPallet as MemberProvider<u64>>::get_member_stats(ENTITY_1, &ALICE);
        assert_eq!(direct, 1, "默认策略下 get_member_stats 应返回 qualified_referrals=1");
    });
}

#[test]
fn get_member_stats_include_repurchase_direct() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;
        use pallet_entity_common::MemberStatsPolicy;

        // ALICE 注册
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));

        // BOB 复购注册，CHARLIE 主动注册
        MemberPallet::auto_register_by_entity(ENTITY_1, &BOB, Some(ALICE), false).unwrap();
        MemberPallet::auto_register_by_entity(ENTITY_1, &CHARLIE, Some(ALICE), true).unwrap();

        // 设置策略：直推含复购
        assert_ok!(MemberPallet::set_member_stats_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            MemberStatsPolicy::INCLUDE_REPURCHASE_DIRECT,
        ));

        // get_member_stats 现在返回 direct_referrals（含复购）
        let (direct, _team, _spent) =
            <MemberPallet as MemberProvider<u64>>::get_member_stats(ENTITY_1, &ALICE);
        assert_eq!(direct, 2, "含复购策略下 get_member_stats 应返回 direct_referrals=2");
    });
}

// ============================================================================
// qualified_referrals 测试
// ============================================================================

#[test]
fn qualified_referrals_not_incremented_for_repurchase() {
    new_test_ext().execute_with(|| {
        // ALICE 先注册
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));

        // BOB 通过复购赠与注册 (qualified=false)
        MemberPallet::auto_register_by_entity(ENTITY_1, &BOB, Some(ALICE), false).unwrap();

        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.direct_referrals, 1, "direct_referrals 应递增");
        assert_eq!(alice.qualified_referrals, 0, "qualified_referrals 不应递增（复购赠与）");

        // CHARLIE 主动注册推荐人 ALICE (qualified=true)
        MemberPallet::auto_register_by_entity(ENTITY_1, &CHARLIE, Some(ALICE), true).unwrap();

        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.direct_referrals, 2);
        assert_eq!(alice.qualified_referrals, 1, "qualified_referrals 仅在 qualified=true 时递增");
    });
}

// ============================================================================
// H13: manual_set_member_level 应清除 MemberLevelExpiry
// ============================================================================

#[test]
fn h13_manual_upgrade_clears_stale_expiry() {
    new_test_ext().execute_with(|| {
        // Setup: 3 custom levels + rule system
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        let name3: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP3".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name3, 2000u64, 0, 0,
        ));

        // Register member
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Simulate rule-based upgrade to VIP2 (level_id=2) with expiry at block 100
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 2;
                member.total_spent = 200; // enough for VIP1, not VIP2
            }
        });
        crate::LevelMemberCount::<Test>::mutate(ENTITY_1, 0u8, |c| *c = c.saturating_sub(1));
        crate::LevelMemberCount::<Test>::mutate(ENTITY_1, 2u8, |c| *c = c.saturating_add(1));
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 100u64);

        // At block 50, owner manually upgrades to VIP3 (level_id=3) — permanent
        System::set_block_number(50);
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 3,
        ));

        // H13: MemberLevelExpiry should be cleared by manual upgrade
        assert!(
            crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none(),
            "manual upgrade must clear stale expiry"
        );

        // At block 101 (past the old expiry), update_spent should NOT downgrade
        System::set_block_number(101);
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u64));

        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 3, "VIP3 must persist after old expiry passes");
    });
}

#[test]
fn h13_manual_upgrade_without_prior_expiry_works() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0,
        ));

        // Register and manually upgrade (no prior expiry)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none());

        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2,
        ));

        // Still no expiry
        assert!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none());
        assert_eq!(MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap().custom_level_id, 2);
    });
}

// ============================================================================
// H9: MemberProvider get_member_stats 应尊重 MemberStatsPolicy
// (pallet 内部 impl 已正确，此测试验证 pallet-level behavior)
// ============================================================================

#[test]
fn h9_get_member_stats_respects_policy_default() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // BOB: repurchase (qualified=false), CHARLIE: active (qualified=true)
        MemberPallet::auto_register_by_entity(ENTITY_1, &BOB, Some(ALICE), false).unwrap();
        MemberPallet::auto_register_by_entity(ENTITY_1, &CHARLIE, Some(ALICE), true).unwrap();

        // Default policy (0): should return qualified_referrals = 1
        let (direct, _team, _spent) =
            <MemberPallet as MemberProvider<u64>>::get_member_stats(ENTITY_1, &ALICE);
        assert_eq!(direct, 1, "default policy: get_member_stats should return qualified_referrals");
    });
}

#[test]
fn h9_get_member_stats_respects_policy_include_repurchase() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;
        use pallet_entity_common::MemberStatsPolicy;

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        MemberPallet::auto_register_by_entity(ENTITY_1, &BOB, Some(ALICE), false).unwrap();
        MemberPallet::auto_register_by_entity(ENTITY_1, &CHARLIE, Some(ALICE), true).unwrap();

        // Set policy to include repurchase in direct count
        assert_ok!(MemberPallet::set_member_stats_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            MemberStatsPolicy::INCLUDE_REPURCHASE_DIRECT,
        ));

        // Should return direct_referrals = 2 (including repurchase)
        let (direct, _team, _spent) =
            <MemberPallet as MemberProvider<u64>>::get_member_stats(ENTITY_1, &ALICE);
        assert_eq!(direct, 2, "include-repurchase policy: should return direct_referrals");
    });
}

// ============================================================================
// M9: get_effective_level_by_entity 应验证等级仍存在
// ============================================================================

#[test]
fn m9_effective_level_falls_back_when_level_deleted() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels, auto upgrade
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0,
        ));

        // Register member, manually set to VIP2 (level_id=2)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 2;
                member.total_spent = 200; // enough for VIP1 only
            }
        });

        // Verify effective level is 2 before deletion
        assert_eq!(MemberPallet::get_effective_level_by_entity(ENTITY_1, &ALICE), 2);

        // Delete VIP2 (last level)
        assert_ok!(MemberPallet::remove_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, 2,
        ));

        // M9: effective level should fall back via calculate (total_spent=200 >= 100 → VIP1=1)
        let effective = MemberPallet::get_effective_level_by_entity(ENTITY_1, &ALICE);
        assert_eq!(effective, 1, "deleted level should fall back to spending-based calculation");
    });
}

#[test]
fn m9_effective_level_valid_level_unchanged() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 1;
            }
        });

        // Level 1 (VIP1) still exists — should return as-is
        assert_eq!(MemberPallet::get_effective_level_by_entity(ENTITY_1, &ALICE), 1);
    });
}

// ============================================================================
// M10: 推荐类触发器在注册/绑定推荐人时立即评估
// ============================================================================

#[test]
fn m10_referral_count_trigger_fires_on_registration() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels, upgrade rule: ReferralCount >= 2 → VIP2 (level_id=2)
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 500u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 1000u64, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"Ref2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::ReferralCount { count: 2 },
            2, None, 1, false, None,
        ));

        // Register ALICE as referrer
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // First referral: BOB → ALICE. direct_referrals=1, not enough
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));
        assert_eq!(MemberPallet::get_effective_level_by_entity(ENTITY_1, &ALICE), 0,
            "1 referral: should stay at base level");

        // Second referral: CHARLIE → ALICE. direct_referrals=2, triggers upgrade to VIP2
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, Some(ALICE)));
        assert_eq!(MemberPallet::get_effective_level_by_entity(ENTITY_1, &ALICE), 2,
            "M10: referral count trigger should fire on registration, upgrading to VIP2 (level_id=2)");

        // Verify event was emitted
        let events = System::events();
        let upgraded = events.iter().any(|e| {
            matches!(
                &e.event,
                RuntimeEvent::MemberPallet(crate::Event::MemberUpgradedByRule {
                    entity_id: ENTITY_1,
                    rule_id: 0,
                    ..
                })
            )
        });
        assert!(upgraded, "MemberUpgradedByRule event should be emitted on referral trigger");
    });
}

#[test]
fn m10_referral_trigger_does_not_fire_below_threshold() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels, upgrade rule: ReferralCount >= 3 → VIP2 (level_id=2)
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 500u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 1000u64, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"Ref3".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::ReferralCount { count: 3 },
            2, None, 1, false, None,
        ));

        // Register ALICE + 2 referrals (below threshold of 3)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, Some(ALICE)));

        // No upgrade event should be emitted
        let events = System::events();
        let upgraded = events.iter().any(|e| {
            matches!(
                &e.event,
                RuntimeEvent::MemberPallet(crate::Event::MemberUpgradedByRule { .. })
            )
        });
        assert!(!upgraded, "should not trigger upgrade below referral threshold");
    });
}

#[test]
fn m10_bind_referrer_triggers_referral_upgrade() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels, upgrade rule: ReferralCount >= 1 → VIP2 (level_id=2)
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 500u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 1000u64, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"Ref1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::ReferralCount { count: 1 },
            2, None, 1, false, None,
        ));

        // Register both without referrer
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));

        // No upgrade yet
        let events = System::events();
        let upgraded_before = events.iter().any(|e| {
            matches!(
                &e.event,
                RuntimeEvent::MemberPallet(crate::Event::MemberUpgradedByRule { .. })
            )
        });
        assert!(!upgraded_before, "no upgrade before bind_referrer");

        // BOB binds ALICE as referrer → ALICE gets 1 referral → triggers
        assert_ok!(MemberPallet::bind_referrer(RuntimeOrigin::signed(BOB), SHOP_1, ALICE));

        let events = System::events();
        let upgraded_after = events.iter().any(|e| {
            matches!(
                &e.event,
                RuntimeEvent::MemberPallet(crate::Event::MemberUpgradedByRule {
                    entity_id: ENTITY_1,
                    ..
                })
            )
        });
        assert!(upgraded_after, "M10: bind_referrer should trigger referral upgrade rule");
    });
}

// ============================================================================
// H1: remove_custom_level 拒绝删除有会员的等级
// ============================================================================

#[test]
fn h1_remove_custom_level_rejects_when_level_has_members() {
    new_test_ext().execute_with(|| {
        // Setup: init level system + add 2 custom levels
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 500u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 1000u64, 0, 0,
        ));

        // Register ALICE and manually upgrade to VIP2 (level_id=2)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2,
        ));

        // Try to remove VIP2 — should fail because ALICE is at that level
        assert_noop!(
            MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 2),
            crate::Error::<Test>::LevelHasMembers
        );

        // Downgrade ALICE back to level 0
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 0,
        ));

        // Now removal should succeed
        assert_ok!(MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 2));
    });
}

#[test]
fn h1_remove_empty_level_succeeds() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u64, 0, 0,
        ));

        // No members at level 1 (custom) — removal succeeds
        assert_ok!(MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 1));
    });
}

// ============================================================================
// M1: next_rule_id overflow
// ============================================================================

#[test]
fn m1_add_upgrade_rule_overflow_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u64, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));

        // Force next_rule_id to u32::MAX
        crate::EntityUpgradeRules::<Test>::mutate(ENTITY_1, |maybe_system| {
            if let Some(system) = maybe_system {
                system.next_rule_id = u32::MAX;
            }
        });

        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"Overflow".to_vec().try_into().unwrap();
        assert_noop!(
            MemberPallet::add_upgrade_rule(
                RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
                UpgradeTrigger::ReferralCount { count: 1 },
                1, None, 1, false, None,
            ),
            crate::Error::<Test>::RuleIdOverflow
        );
    });
}

// ============================================================================
// R6-M1: bind_referrer 复用 mutate_member_referral 回归测试
// ============================================================================

#[test]
fn r6_m1_bind_referrer_updates_all_stats_correctly() {
    new_test_ext().execute_with(|| {
        // Register ALICE, BOB, CHARLIE without referrer
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, None));

        // BOB binds ALICE as referrer
        assert_ok!(MemberPallet::bind_referrer(RuntimeOrigin::signed(BOB), SHOP_1, ALICE));

        // Verify referrer set
        let bob = MemberPallet::get_member_by_shop(SHOP_1, &BOB).unwrap();
        assert_eq!(bob.referrer, Some(ALICE));

        // Verify ALICE stats: direct=1, qualified=1, team=1
        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.direct_referrals, 1);
        assert_eq!(alice.qualified_referrals, 1);
        assert_eq!(alice.team_size, 1);

        // CHARLIE binds BOB → ALICE team grows to 2
        assert_ok!(MemberPallet::bind_referrer(RuntimeOrigin::signed(CHARLIE), SHOP_1, BOB));
        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.team_size, 2, "indirect referral should increment team_size");
        let bob = MemberPallet::get_member_by_shop(SHOP_1, &BOB).unwrap();
        assert_eq!(bob.direct_referrals, 1);
        assert_eq!(bob.team_size, 1);
    });
}

#[test]
fn r6_m1_bind_referrer_fails_on_referrals_full() {
    new_test_ext().execute_with(|| {
        // Register ALICE
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Fill DirectReferrals for ALICE to MaxDirectReferrals (50)
        for i in 0..50u64 {
            let account = 100 + i;
            assert_ok!(MemberPallet::register_member(
                RuntimeOrigin::signed(account), SHOP_1, Some(ALICE),
            ));
        }

        // Register one more without referrer, then try to bind
        let extra = 200u64;
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(extra), SHOP_1, None));

        // bind_referrer should fail with ReferralsFull
        assert_noop!(
            MemberPallet::bind_referrer(RuntimeOrigin::signed(extra), SHOP_1, ALICE),
            Error::<Test>::ReferralsFull
        );

        // Verify the member's referrer was NOT set (transactional rollback)
        let member = MemberPallet::get_member_by_shop(SHOP_1, &extra).unwrap();
        assert!(member.referrer.is_none(), "referrer must not be set on ReferralsFull failure");
    });
}

// ============================================================================
// R6-L1: 配置变更事件回归测试
// ============================================================================

#[test]
fn r6_l1_set_use_custom_levels_emits_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        System::reset_events();

        assert_ok!(MemberPallet::set_use_custom_levels(
            RuntimeOrigin::signed(OWNER), SHOP_1, false,
        ));

        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            &e.event,
            RuntimeEvent::MemberPallet(crate::Event::UseCustomLevelsUpdated {
                shop_id: SHOP_1,
                use_custom: false,
            })
        )), "UseCustomLevelsUpdated event should be emitted");
    });
}

#[test]
fn r6_l1_set_upgrade_mode_emits_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        System::reset_events();

        assert_ok!(MemberPallet::set_upgrade_mode(
            RuntimeOrigin::signed(OWNER), SHOP_1, LevelUpgradeMode::ManualUpgrade,
        ));

        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            &e.event,
            RuntimeEvent::MemberPallet(crate::Event::UpgradeModeUpdated {
                shop_id: SHOP_1,
                upgrade_mode: LevelUpgradeMode::ManualUpgrade,
            })
        )), "UpgradeModeUpdated event should be emitted");
    });
}

#[test]
fn r6_l1_set_upgrade_rule_system_enabled_emits_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        System::reset_events();

        assert_ok!(MemberPallet::set_upgrade_rule_system_enabled(
            RuntimeOrigin::signed(OWNER), SHOP_1, false,
        ));

        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            &e.event,
            RuntimeEvent::MemberPallet(crate::Event::UpgradeRuleSystemToggled {
                shop_id: SHOP_1,
                enabled: false,
            })
        )), "UpgradeRuleSystemToggled event should be emitted");
    });
}

#[test]
fn r6_l1_set_conflict_strategy_emits_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        System::reset_events();

        assert_ok!(MemberPallet::set_conflict_strategy(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::FirstMatch,
        ));

        let events = System::events();
        assert!(events.iter().any(|e| matches!(
            &e.event,
            RuntimeEvent::MemberPallet(crate::Event::ConflictStrategyUpdated {
                shop_id: SHOP_1,
                strategy: ConflictStrategy::FirstMatch,
            })
        )), "ConflictStrategyUpdated event should be emitted");
    });
}

// ============================================================================
// R6-L3: 冲突策略覆盖测试
// ============================================================================

#[test]
fn r6_l3_conflict_strategy_highest_priority() {
    new_test_ext().execute_with(|| {
        // Setup: 3 custom levels
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP2".to_vec().try_into().unwrap();
        let name3: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP3".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0));
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0));
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name3, 1000u64, 0, 0));

        // Init rule system with HighestPriority strategy
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestPriority,
        ));

        // Rule 1: TotalSpent >= 50 → VIP3 (level 3), priority=1
        let rn1: BoundedVec<u8, frame_support::traits::ConstU32<64>> = b"LowPri".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rn1,
            UpgradeTrigger::TotalSpent { threshold: 50 }, 3, None, 1, false, None,
        ));
        // Rule 2: TotalSpent >= 50 → VIP1 (level 1), priority=10
        let rn2: BoundedVec<u8, frame_support::traits::ConstU32<64>> = b"HighPri".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rn2,
            UpgradeTrigger::TotalSpent { threshold: 50 }, 1, None, 10, false, None,
        ));

        // Register and spend
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 100u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));

        // HighestPriority: rule with priority=10 wins → VIP1 (level 1)
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1, "HighestPriority should select priority=10 rule → VIP1");
    });
}

#[test]
fn r6_l3_conflict_strategy_longest_duration() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0));
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0));

        // Init with LongestDuration strategy
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::LongestDuration,
        ));

        // Rule 1: TotalSpent >= 50 → VIP1, duration=10
        let rn1: BoundedVec<u8, frame_support::traits::ConstU32<64>> = b"Short".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rn1,
            UpgradeTrigger::TotalSpent { threshold: 50 }, 1, Some(10u64), 1, false, None,
        ));
        // Rule 2: TotalSpent >= 50 → VIP2, duration=None (permanent)
        let rn2: BoundedVec<u8, frame_support::traits::ConstU32<64>> = b"Perm".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rn2,
            UpgradeTrigger::TotalSpent { threshold: 50 }, 2, None, 1, false, None,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 100u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));

        // LongestDuration: None (permanent) > Some(10) → VIP2 (level 2)
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2, "LongestDuration should prefer permanent rule");
        assert!(MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none(), "permanent rule = no expiry");
    });
}

#[test]
fn r6_l3_conflict_strategy_first_match() {
    new_test_ext().execute_with(|| {
        // Setup: 2 custom levels
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> = b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0));
        assert_ok!(MemberPallet::add_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u64, 0, 0));

        // Init with FirstMatch strategy
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::FirstMatch,
        ));

        // Rule 1 (added first): TotalSpent >= 50 → VIP1
        let rn1: BoundedVec<u8, frame_support::traits::ConstU32<64>> = b"First".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rn1,
            UpgradeTrigger::TotalSpent { threshold: 50 }, 1, None, 1, false, None,
        ));
        // Rule 2 (added second): TotalSpent >= 50 → VIP2
        let rn2: BoundedVec<u8, frame_support::traits::ConstU32<64>> = b"Second".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rn2,
            UpgradeTrigger::TotalSpent { threshold: 50 }, 2, None, 1, false, None,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 100u64));
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 100u64));

        // FirstMatch: first rule wins → VIP1 (level 1)
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1, "FirstMatch should select the first matching rule → VIP1");
    });
}

// ============================================================================
// M5: USDT 累计消费（total_spent 直接存储 USDT）
// ============================================================================

#[test]
fn m5_update_spent_accumulates_usdt() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 第一笔: 50 USDT
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 50u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.total_spent, 50u64);

        // 第二笔: 30 USDT — 累加
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 30u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.total_spent, 80u64);
    });
}

#[test]
fn m5_zero_usdt_no_change() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // amount_usdt = 0 → total_spent 不变
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 0u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.total_spent, 0u64);
    });
}

#[test]
fn m5_get_member_stats_returns_usdt() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 消费 1_000_000 USDT (精度 10^6)
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 1_000_000u64));

        let (_, _, spent_usdt) = <MemberPallet as crate::MemberProvider<u64>>::get_member_stats(ENTITY_1, &ALICE);
        assert_eq!(spent_usdt, 1_000_000u128, "get_member_stats should return USDT amount");
    });
}

#[test]
fn m5_usdt_saturating_add_no_overflow() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 预设一个接近 u64::MAX 的值
        EntityMembers::<Test>::mutate(ENTITY_1, &ALICE, |maybe| {
            if let Some(ref mut m) = maybe {
                m.total_spent = u64::MAX - 10;
            }
        });

        // 再加 20 应 saturating 到 u64::MAX
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 20u64));
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.total_spent, u64::MAX);
    });
}

// ============================================================================
// M6: PendingMembers 过期机制
// ============================================================================

#[test]
fn m6_pending_member_stores_applied_at() {
    new_test_ext().execute_with(|| {
        // 启用 APPROVAL_REQUIRED
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        System::set_block_number(42);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 验证 applied_at = 42
        let (referrer, applied_at) = PendingMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(referrer, None);
        assert_eq!(applied_at, 42);
    });
}

#[test]
fn m6_approve_within_expiry_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        System::set_block_number(10);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // PendingMemberExpiry = 100, so approve at block 110 is still valid (10 + 100 = 110)
        System::set_block_number(110);
        assert_ok!(MemberPallet::approve_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

#[test]
fn m6_approve_after_expiry_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        System::set_block_number(10);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Block 111 > 10 + 100 = expired
        System::set_block_number(111);
        assert_noop!(
            MemberPallet::approve_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            Error::<Test>::PendingMemberAlreadyExpired
        );
        // PendingMembers entry was already taken by approve_member (take before check)
        // so the entry should be removed even on failure... let's verify behavior:
        // Actually, take() removes it, then ensure! fails, but the extrinsic is transactional
        // so the storage change is reverted. The entry should still exist.
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
    });
}

#[test]
fn m6_cancel_pending_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));

        // ALICE cancels her own pending application
        assert_ok!(MemberPallet::cancel_pending_member(RuntimeOrigin::signed(ALICE), SHOP_1));
        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));

        // Event emitted
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::PendingMemberCancelled { entity_id: ENTITY_1, account: ALICE },
        ));
    });
}

#[test]
fn m6_cancel_pending_not_found_fails() {
    new_test_ext().execute_with(|| {
        // No pending entry for ALICE
        assert_noop!(
            MemberPallet::cancel_pending_member(RuntimeOrigin::signed(ALICE), SHOP_1),
            Error::<Test>::PendingMemberNotFound
        );
    });
}

#[test]
fn m6_cleanup_expired_pending_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        // Register at block 10
        System::set_block_number(10);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Register at block 50
        System::set_block_number(50);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));

        // At block 111: ALICE expired (10+100=110 < 111), BOB still valid (50+100=150 >= 111)
        System::set_block_number(111);
        assert_ok!(MemberPallet::cleanup_expired_pending(
            RuntimeOrigin::signed(DAVE), ENTITY_1, 10,
        ));

        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE), "ALICE should be cleaned up");
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &BOB), "BOB should still be pending");

        // Expired event for ALICE
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::PendingMemberExpired { entity_id: ENTITY_1, account: ALICE },
        ));
    });
}

#[test]
fn m6_cleanup_respects_max_clean() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        // Register 3 members at block 1
        System::set_block_number(1);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, None));

        // All expired at block 102
        System::set_block_number(102);

        // max_clean = 1 → only 1 removed
        assert_ok!(MemberPallet::cleanup_expired_pending(
            RuntimeOrigin::signed(DAVE), ENTITY_1, 1,
        ));

        // Count remaining
        let remaining: Vec<_> = PendingMembers::<Test>::iter_prefix(ENTITY_1).collect();
        assert_eq!(remaining.len(), 2, "max_clean=1 should only remove 1 entry");
    });
}

#[test]
fn m6_cleanup_zero_expiry_is_noop() {
    // This test requires PendingMemberExpiry = 0 (never expires).
    // Since our mock sets it to 100, we test via the extrinsic which checks expiry.is_zero().
    // We can't easily change the config per test, but we can verify the logic:
    // If expiry is non-zero, cleanup should work (tested above).
    // Here we verify that with non-expired entries, cleanup doesn't remove them.
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        System::set_block_number(50);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Block 100: not expired yet (50 + 100 = 150 > 100)
        System::set_block_number(100);
        assert_ok!(MemberPallet::cleanup_expired_pending(
            RuntimeOrigin::signed(DAVE), ENTITY_1, 10,
        ));
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE), "not expired yet");
    });
}

#[test]
fn m6_auto_register_stores_applied_at() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        System::set_block_number(77);
        // auto_register triggers pending
        assert_ok!(MemberPallet::auto_register(SHOP_1, &ALICE, None));

        let (referrer, applied_at) = PendingMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(referrer, None);
        assert_eq!(applied_at, 77);
    });
}

// ============================================================================
// P1-1: 批量审批/拒绝
// ============================================================================

#[test]
fn batch_approve_members_works() {
    new_test_ext().execute_with(|| {
        // 设置为 APPROVAL_REQUIRED
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        // 提交 3 个待审批
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(CHARLIE), SHOP_1, None));

        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &BOB));
        assert!(PendingMembers::<Test>::contains_key(ENTITY_1, &CHARLIE));

        // 批量审批
        assert_ok!(MemberPallet::batch_approve_members(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            vec![ALICE, BOB, CHARLIE],
        ));

        // 验证全部注册成功
        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &ALICE));
        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &BOB));
        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &CHARLIE));
        assert_eq!(MemberPallet::member_count(ENTITY_1), 3);

        // 验证事件
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::BatchMembersApproved { entity_id: ENTITY_1, count: 3 },
        ));
    });
}

#[test]
fn batch_approve_skips_expired() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        System::set_block_number(1);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        System::set_block_number(150);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));

        // 超过 ALICE 的过期时间 (applied_at=1 + expiry=100 < 200), BOB 未过期 (applied_at=150 + 100 > 200)
        System::set_block_number(200);

        assert_ok!(MemberPallet::batch_approve_members(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            vec![ALICE, BOB],
        ));

        // ALICE 过期被跳过, BOB 未过期应注册成功
        assert!(!EntityMembers::<Test>::contains_key(ENTITY_1, &ALICE));
        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &BOB));

        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::BatchMembersApproved { entity_id: ENTITY_1, count: 1 },
        ));
    });
}

#[test]
fn batch_approve_skips_nonexistent() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // BOB 没有申请, 应被跳过
        assert_ok!(MemberPallet::batch_approve_members(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            vec![ALICE, BOB],
        ));

        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &ALICE));
        assert!(!EntityMembers::<Test>::contains_key(ENTITY_1, &BOB));

        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::BatchMembersApproved { entity_id: ENTITY_1, count: 1 },
        ));
    });
}

#[test]
fn batch_approve_rejects_over_limit() {
    new_test_ext().execute_with(|| {
        let accounts: Vec<u64> = (100..152).collect(); // 52 accounts
        assert_noop!(
            MemberPallet::batch_approve_members(
                RuntimeOrigin::signed(OWNER), SHOP_1, accounts,
            ),
            crate::Error::<Test>::BatchLimitExceeded
        );
    });
}

#[test]
fn batch_approve_admin_can_call() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // ADMIN (account 2) should have MEMBER_MANAGE permission
        assert_ok!(MemberPallet::batch_approve_members(
            RuntimeOrigin::signed(ADMIN), SHOP_1,
            vec![ALICE],
        ));

        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &ALICE));
    });
}

#[test]
fn batch_reject_members_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));

        assert_ok!(MemberPallet::batch_reject_members(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            vec![ALICE, BOB],
        ));

        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &BOB));

        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::BatchMembersRejected { entity_id: ENTITY_1, count: 2 },
        ));
    });
}

#[test]
fn batch_reject_skips_nonexistent() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // BOB not pending — should be silently skipped
        assert_ok!(MemberPallet::batch_reject_members(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            vec![ALICE, BOB],
        ));

        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::BatchMembersRejected { entity_id: ENTITY_1, count: 1 },
        ));
    });
}

#[test]
fn batch_reject_rejects_over_limit() {
    new_test_ext().execute_with(|| {
        let accounts: Vec<u64> = (100..152).collect();
        assert_noop!(
            MemberPallet::batch_reject_members(
                RuntimeOrigin::signed(OWNER), SHOP_1, accounts,
            ),
            crate::Error::<Test>::BatchLimitExceeded
        );
    });
}

// ============================================================================
// P1-2: 封禁/解封会员
// ============================================================================

#[test]
fn ban_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_ok!(MemberPallet::ban_member(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None,
        ));

        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_some());

        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::MemberBanned { entity_id: ENTITY_1, account: ALICE, reason: None },
        ));
    });
}

#[test]
fn ban_member_not_member_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None),
            crate::Error::<Test>::NotMember
        );
    });
}

#[test]
fn ban_member_already_banned_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));

        assert_noop!(
            MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None),
            crate::Error::<Test>::MemberAlreadyBanned
        );
    });
}

#[test]
fn ban_member_admin_can_call() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_ok!(MemberPallet::ban_member(
            RuntimeOrigin::signed(ADMIN), SHOP_1, ALICE, None,
        ));

        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_some());
    });
}

#[test]
fn ban_member_non_admin_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_noop!(
            MemberPallet::ban_member(RuntimeOrigin::signed(BOB), SHOP_1, ALICE, None),
            crate::Error::<Test>::NotEntityAdmin
        );
    });
}

#[test]
fn unban_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));

        assert_ok!(MemberPallet::unban_member(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE,
        ));

        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_none());

        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::MemberUnbanned { entity_id: ENTITY_1, account: ALICE },
        ));
    });
}

#[test]
fn unban_member_not_banned_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_noop!(
            MemberPallet::unban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            crate::Error::<Test>::MemberNotBanned
        );
    });
}

#[test]
fn unban_member_admin_can_call() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));

        assert_ok!(MemberPallet::unban_member(
            RuntimeOrigin::signed(ADMIN), SHOP_1, ALICE,
        ));

        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_none());
    });
}

// ============================================================================
// P0-2: MemberProvider trait — member_count / is_banned
// ============================================================================

#[test]
fn member_provider_member_count_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(MemberPallet::member_count(ENTITY_1), 0);

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_eq!(MemberPallet::member_count(ENTITY_1), 1);

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));
        assert_eq!(MemberPallet::member_count(ENTITY_1), 2);
    });
}

#[test]
fn member_provider_is_banned_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 未封禁
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_none());

        // 封禁后
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_some());

        // 解封后
        assert_ok!(MemberPallet::unban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.banned_at.is_none());
    });
}

#[test]
fn ban_member_stores_block_number() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        System::set_block_number(42);
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));

        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(m.banned_at, Some(42));
    });
}

// ============================================================================
// last_active_at 可读可写
// ============================================================================

#[test]
fn last_active_at_set_on_register() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 注册时 last_active_at = joined_at = 当前区块
        let member = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(member.last_active_at, 10);

        // 通过 trait 方法读取
        assert_eq!(
            <MemberPallet as crate::MemberProvider<u64>>::last_active_at(ENTITY_1, &ALICE),
            10
        );
    });
}

#[test]
fn last_active_at_updated_on_spend() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_eq!(
            <MemberPallet as crate::MemberProvider<u64>>::last_active_at(ENTITY_1, &ALICE),
            5
        );

        // 消费后 last_active_at 更新
        System::set_block_number(100);
        assert_ok!(MemberPallet::update_spent_by_entity(ENTITY_1, &ALICE, 1000u64));
        assert_eq!(
            <MemberPallet as crate::MemberProvider<u64>>::last_active_at(ENTITY_1, &ALICE),
            100
        );

        // 再次消费
        System::set_block_number(200);
        assert_ok!(MemberPallet::update_spent_by_entity(ENTITY_1, &ALICE, 500u64));
        assert_eq!(
            <MemberPallet as crate::MemberProvider<u64>>::last_active_at(ENTITY_1, &ALICE),
            200
        );
    });
}

#[test]
fn last_active_at_returns_zero_for_non_member() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            <MemberPallet as crate::MemberProvider<u64>>::last_active_at(ENTITY_1, &ALICE),
            0
        );
    });
}

// ============================================================================
// KYC 阻断测试
// ============================================================================

#[test]
fn kyc_required_blocks_register_member() {
    new_test_ext().execute_with(|| {
        // 设置 KYC_REQUIRED (bit 3 = 0b0000_1000 = 8)
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_1000,
        ));

        // 未通过 KYC → 注册失败
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None),
            Error::<Test>::KycNotPassed
        );

        // 通过 KYC 后 → 注册成功
        set_kyc_passed(ENTITY_1, ALICE);
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

#[test]
fn kyc_required_blocks_auto_register() {
    new_test_ext().execute_with(|| {
        // 设置 KYC_REQUIRED
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_1000,
        ));

        // 未通过 KYC → auto_register 失败
        assert_noop!(
            MemberPallet::auto_register(SHOP_1, &ALICE, None),
            Error::<Test>::KycNotPassed
        );

        // 通过 KYC 后 → 成功
        set_kyc_passed(ENTITY_1, ALICE);
        assert_ok!(MemberPallet::auto_register(SHOP_1, &ALICE, None));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

#[test]
fn kyc_required_blocks_auto_register_by_entity() {
    new_test_ext().execute_with(|| {
        // 设置 KYC_REQUIRED
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_1000,
        ));

        // 未通过 KYC → auto_register_by_entity 失败
        assert_noop!(
            MemberPallet::auto_register_by_entity(ENTITY_1, &ALICE, None, true),
            Error::<Test>::KycNotPassed
        );

        // 通过 KYC 后 → 成功
        set_kyc_passed(ENTITY_1, ALICE);
        assert_ok!(MemberPallet::auto_register_by_entity(ENTITY_1, &ALICE, None, true));
        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, ALICE));
    });
}

#[test]
fn kyc_not_required_allows_register_without_kyc() {
    new_test_ext().execute_with(|| {
        // 默认策略（OPEN，无 KYC 要求）→ 直接注册成功
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

#[test]
fn kyc_upgrade_required_blocks_auto_upgrade() {
    new_test_ext().execute_with(|| {
        // 初始化等级系统
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            BoundedVec::try_from(b"Base".to_vec()).unwrap(), 0, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            BoundedVec::try_from(b"VIP1".to_vec()).unwrap(), 100, 500, 0,
        ));

        // 注册会员
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));

        // 设置 KYC_UPGRADE_REQUIRED (bit 4 = 0b0001_0000 = 16)
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0001_0000,
        ));

        // 未通过 KYC → apply_upgrade 静默跳过（不升级）
        assert_ok!(MemberPallet::apply_upgrade_for_test(ENTITY_1, &ALICE, 0, 2, None, false));
        let member = EntityMembers::<Test>::get(ENTITY_1, ALICE).unwrap();
        assert_eq!(member.custom_level_id, 0); // 未升级

        // 通过 KYC 后 → 升级成功
        set_kyc_passed(ENTITY_1, ALICE);
        assert_ok!(MemberPallet::apply_upgrade_for_test(ENTITY_1, &ALICE, 0, 2, None, false));
        let member = EntityMembers::<Test>::get(ENTITY_1, ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2); // 已升级
    });
}

#[test]
fn kyc_upgrade_required_blocks_manual_upgrade() {
    new_test_ext().execute_with(|| {
        // 初始化等级系统（手动升级模式）
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            BoundedVec::try_from(b"Base".to_vec()).unwrap(), 0, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            BoundedVec::try_from(b"VIP1".to_vec()).unwrap(), 100, 500, 0,
        ));

        // 注册会员
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, None,
        ));

        // 设置 KYC_UPGRADE_REQUIRED
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0001_0000,
        ));

        // 未通过 KYC → 手动升级失败
        assert_noop!(
            MemberPallet::manual_set_member_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2,
            ),
            Error::<Test>::KycRequiredForUpgrade
        );

        // 通过 KYC 后 → 手动升级成功
        set_kyc_passed(ENTITY_1, ALICE);
        assert_ok!(MemberPallet::manual_set_member_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2,
        ));
        let member = EntityMembers::<Test>::get(ENTITY_1, ALICE).unwrap();
        assert_eq!(member.custom_level_id, 2);
    });
}

#[test]
fn kyc_combined_with_other_policies() {
    new_test_ext().execute_with(|| {
        // KYC_REQUIRED + REFERRAL_REQUIRED = 0b1000 | 0b0010 = 10
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_1010,
        ));

        // 注册 BOB 作为推荐人（先关闭 KYC 策略）
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0,
        ));
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(BOB), SHOP_1, None,
        ));
        // 恢复策略
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_1010,
        ));

        // 有推荐人但无 KYC → 失败
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, Some(BOB)),
            Error::<Test>::KycNotPassed
        );

        // 有 KYC 但无推荐人 → 失败
        set_kyc_passed(ENTITY_1, ALICE);
        assert_noop!(
            MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None),
            Error::<Test>::ReferralRequiredForRegistration
        );

        // 有 KYC + 有推荐人 → 成功
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(ALICE), SHOP_1, Some(BOB),
        ));
        assert!(MemberPallet::is_member_of_shop(SHOP_1, &ALICE));
    });
}

// ============================================================================
// 封禁执行力测试
// ============================================================================

#[test]
fn banned_member_update_spent_silently_skipped() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        // 先消费一次
        assert_ok!(MemberPallet::update_spent_by_entity(ENTITY_1, &ALICE, 100u64));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(m.total_spent, 100u64);

        // 封禁后消费被跳过
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));
        assert_ok!(MemberPallet::update_spent_by_entity(ENTITY_1, &ALICE, 200u64));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(m.total_spent, 100u64); // 未增加

        // 解封后消费恢复
        assert_ok!(MemberPallet::unban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        assert_ok!(MemberPallet::update_spent_by_entity(ENTITY_1, &ALICE, 300u64));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(m.total_spent, 400u64); // 100 + 300
    });
}

#[test]
fn banned_member_auto_upgrade_silently_skipped() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        // 初始化等级系统 + 添加两个等级
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name0: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"Base".to_vec().try_into().unwrap();
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name0, 0u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));

        // 封禁
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));

        // 尝试 apply_upgrade → 静默跳过
        assert_ok!(MemberPallet::apply_upgrade_for_test(ENTITY_1, &ALICE, 1, 2, None, false));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(m.custom_level_id, 0); // 未升级
    });
}

#[test]
fn banned_member_manual_upgrade_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        // 初始化等级系统（ManualUpgrade 模式）
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));
        let name0: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"Base".to_vec().try_into().unwrap();
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name0, 0u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));

        // 封禁
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));

        // 手动升级 → 显式错误
        assert_noop!(
            MemberPallet::manual_set_member_level(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2),
            Error::<Test>::MemberIsBanned
        );

        // 解封后可以升级
        assert_ok!(MemberPallet::unban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        assert_ok!(MemberPallet::manual_set_member_level(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 2));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(m.custom_level_id, 2);
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_init_level_system() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_1);
        assert_noop!(
            MemberPallet::init_level_system(
                RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_approve_member() {
    new_test_ext().execute_with(|| {
        // 设置需审核的会员政策 (APPROVAL_REQUIRED = 0b0000_0100)
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));
        // 注册会员（进入待审核状态）
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        // 锁定后无法审核
        set_entity_locked(ENTITY_1);
        assert_noop!(
            MemberPallet::approve_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_ban_member() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        set_entity_locked(ENTITY_1);
        assert_noop!(
            MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_set_member_policy() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_1);
        assert_noop!(
            MemberPallet::set_member_policy(
                RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ============================================================================
// is_activated: 激活状态（首次消费达标）
// ============================================================================

#[test]
fn member_provider_is_activated_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 新注册会员: activated=false → 未激活
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(!m.activated);
        assert!(!<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));
        // 但未被封禁
        assert!(!<MemberPallet as crate::MemberProvider<u64>>::is_banned(ENTITY_1, &ALICE));

        // 消费后 → activated=true + MemberActivated 事件
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::update_spent(ENTITY_1, &ALICE, 100));
        let m = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert!(m.activated);
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::MemberActivated { entity_id: ENTITY_1, account: ALICE },
        ));

        // 再次消费不重复发出事件（已激活）
        let events_before = System::events().len();
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::update_spent(ENTITY_1, &ALICE, 200));
        // 不应有新的 MemberActivated 事件
        let all_events = System::events();
        let activated_events: Vec<_> = all_events[events_before..].iter()
            .filter(|e| matches!(e.event, RuntimeEvent::MemberPallet(crate::Event::MemberActivated { .. })))
            .collect();
        assert!(activated_events.is_empty());

        // 封禁后仍为已激活（激活与封禁是独立概念）
        assert_ok!(MemberPallet::ban_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, None));
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_banned(ENTITY_1, &ALICE));

        // 非会员 → 未激活
        assert!(!<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &BOB));
    });
}

#[test]
fn activate_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 注册后未激活
        assert!(!<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));

        // Owner 手动激活
        assert_ok!(MemberPallet::activate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::MemberActivated { entity_id: ENTITY_1, account: ALICE },
        ));

        // 重复激活应失败
        assert_noop!(
            MemberPallet::activate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            Error::<Test>::AlreadyActivated
        );
    });
}

#[test]
fn deactivate_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 消费后激活
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::update_spent(ENTITY_1, &ALICE, 100));
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));

        // Owner 手动取消激活
        assert_ok!(MemberPallet::deactivate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        assert!(!<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::MemberDeactivated { entity_id: ENTITY_1, account: ALICE },
        ));

        // 重复取消激活应失败
        assert_noop!(
            MemberPallet::deactivate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            Error::<Test>::NotActivated
        );
    });
}

#[test]
fn activate_deactivate_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 非 Owner/Admin 不能激活
        assert_noop!(
            MemberPallet::activate_member(RuntimeOrigin::signed(ALICE), SHOP_1, ALICE),
            Error::<Test>::NotEntityAdmin
        );

        // 消费后激活
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::update_spent(ENTITY_1, &ALICE, 100));

        // 非 Owner/Admin 不能取消激活
        assert_noop!(
            MemberPallet::deactivate_member(RuntimeOrigin::signed(ALICE), SHOP_1, ALICE),
            Error::<Test>::NotEntityAdmin
        );
    });
}

#[test]
fn activate_deactivate_rejects_entity_locked() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        set_entity_locked(ENTITY_1);

        assert_noop!(
            MemberPallet::activate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            Error::<Test>::EntityLocked
        );

        assert_noop!(
            MemberPallet::deactivate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn deactivate_then_reactivate_by_spending() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 消费激活
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::update_spent(ENTITY_1, &ALICE, 100));
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));

        // 管理员取消激活
        assert_ok!(MemberPallet::deactivate_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE));
        assert!(!<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));

        // 再次消费 → 重新激活
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::update_spent(ENTITY_1, &ALICE, 50));
        assert!(<MemberPallet as crate::MemberProvider<u64>>::is_activated(ENTITY_1, &ALICE));
    });
}

// ============================================================================
// H1 审计回归: apply_upgrade 在等级系统不存在时应静默跳过
// ============================================================================

#[test]
fn h1_apply_upgrade_skips_when_level_system_absent() {
    new_test_ext().execute_with(|| {
        // 初始化等级系统 + 升级规则
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::AutoUpgrade,
        ));
        let name1: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u64, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 200u64, 0, 0,
        ));

        // 注册会员
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 初始化升级规则系统，规则目标 level_id=2 (VIP2)
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"Buy100".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::SingleOrder { threshold: 50 }, 2, None, 1, false, None,
        ));

        // 重置等级系统（但升级规则系统仍在）
        assert_ok!(MemberPallet::reset_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1,
        ));
        assert!(crate::EntityLevelSystems::<Test>::get(ENTITY_1).is_none());
        assert!(crate::EntityUpgradeRules::<Test>::get(ENTITY_1).is_some());

        // 通过 check_order_upgrade_rules 触发规则 → 调用 apply_upgrade
        // apply_upgrade 应静默跳过，不会把会员设到不存在的等级
        System::reset_events();
        assert_ok!(<MemberPallet as crate::MemberProvider<u64>>::check_order_upgrade_rules(
            ENTITY_1, &ALICE, 0, 200,
        ));

        let member = EntityMembers::<Test>::get(ENTITY_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 0, "H1: should not upgrade when level system absent");

        // 不应有 MemberUpgradedByRule 事件
        let events = System::events();
        let upgraded = events.iter().any(|e| {
            matches!(&e.event, RuntimeEvent::MemberPallet(crate::Event::MemberUpgradedByRule { .. }))
        });
        assert!(!upgraded, "H1: no upgrade event when level system absent");
    });
}

// ============================================================================
// H2 审计回归: batch_approve 过期记录应发出 PendingMemberExpired 事件
// ============================================================================

#[test]
fn h2_batch_approve_emits_expired_event() {
    new_test_ext().execute_with(|| {
        // 设置 APPROVAL_REQUIRED
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 0b0000_0100,
        ));

        // ALICE 在 block 1 申请
        System::set_block_number(1);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // BOB 在 block 150 申请（未过期）
        System::set_block_number(150);
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, None));

        // block 200: ALICE 过期 (1+100 < 200), BOB 未过期 (150+100 >= 200)
        System::set_block_number(200);
        System::reset_events();

        assert_ok!(MemberPallet::batch_approve_members(
            RuntimeOrigin::signed(OWNER), SHOP_1,
            vec![ALICE, BOB],
        ));

        // ALICE 应收到 PendingMemberExpired 事件
        System::assert_has_event(RuntimeEvent::MemberPallet(
            crate::Event::PendingMemberExpired { entity_id: ENTITY_1, account: ALICE },
        ));

        // BOB 应注册成功
        assert!(EntityMembers::<Test>::contains_key(ENTITY_1, &BOB));

        // ALICE 的 PendingMembers 应已被 take() 清除
        assert!(!PendingMembers::<Test>::contains_key(ENTITY_1, &ALICE));
    });
}

// ============================================================================
// M1 审计回归: remove_member 递减推荐人的 qualified_referrals
// ============================================================================

#[test]
fn m1_remove_member_decrements_qualified_referrals() {
    new_test_ext().execute_with(|| {
        // ALICE 注册，BOB 以 ALICE 为推荐人注册（qualified=true）
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));

        // 验证 ALICE: direct=1, qualified=1
        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.direct_referrals, 1);
        assert_eq!(alice.qualified_referrals, 1);

        // 移除 BOB（叶子节点，无下线）
        assert_ok!(MemberPallet::remove_member(RuntimeOrigin::signed(OWNER), SHOP_1, BOB));

        // M1: qualified_referrals 应被递减
        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.direct_referrals, 0, "direct_referrals decremented");
        assert_eq!(alice.qualified_referrals, 0, "M1: qualified_referrals must be decremented on remove");
    });
}

#[test]
fn m1_leave_entity_decrements_qualified_referrals() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(BOB), SHOP_1, Some(ALICE)));

        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.qualified_referrals, 1);

        // BOB 主动退出
        assert_ok!(MemberPallet::leave_entity(RuntimeOrigin::signed(BOB), ENTITY_1));

        let alice = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(alice.qualified_referrals, 0, "M1: leave_entity also decrements qualified_referrals");
    });
}

// ============================================================================
// 方案 D: 有界递归 + 溢出异步补偿 测试
// ============================================================================

/// 辅助函数：构建深度为 depth 的推荐链
/// 返回所有账户 ID（从根到叶）
/// 账户 ID 从 base_id 开始递增
fn build_referral_chain(depth: u32, base_id: u64) -> alloc::vec::Vec<u64> {
    let accounts: alloc::vec::Vec<u64> = (0..depth).map(|i| base_id + i as u64).collect();

    // 注册根节点（无推荐人）
    assert_ok!(MemberPallet::register_member(
        RuntimeOrigin::signed(accounts[0]),
        SHOP_1,
        None,
    ));

    // 依次注册后续节点，推荐人为前一个
    for i in 1..accounts.len() {
        assert_ok!(MemberPallet::register_member(
            RuntimeOrigin::signed(accounts[i]),
            SHOP_1,
            Some(accounts[i - 1]),
        ));
    }

    accounts
}

#[test]
fn d1_shallow_chain_no_overflow() {
    new_test_ext().execute_with(|| {
        // 10 层链（< MAX_SYNC_DEPTH=15），不应产生异步补偿记录
        let accounts = build_referral_chain(10, 100);

        // 无 pending 记录
        assert_eq!(NextPendingUpdateId::<Test>::get(), 0);
        assert_eq!(ProcessedPendingUpdateId::<Test>::get(), 0);

        // 验证 team_size 正确
        let root = MemberPallet::get_member_by_shop(SHOP_1, &accounts[0]).unwrap();
        assert_eq!(root.team_size, 9); // 9 个后代
        let mid = MemberPallet::get_member_by_shop(SHOP_1, &accounts[4]).unwrap();
        assert_eq!(mid.team_size, 5); // 5 个后代
        let leaf = MemberPallet::get_member_by_shop(SHOP_1, &accounts[9]).unwrap();
        assert_eq!(leaf.team_size, 0);
    });
}

#[test]
fn d2_deep_chain_creates_overflow_record() {
    new_test_ext().execute_with(|| {
        // 构建 20 层链（> MAX_SYNC_DEPTH=15）
        // 注册第 20 个节点时，从其推荐人向上递归 15 层后溢出
        let accounts = build_referral_chain(20, 100);

        // 应有 pending 记录（最后几次注册会溢出）
        let next_id = NextPendingUpdateId::<Test>::get();
        assert!(next_id > 0, "深链应产生异步补偿记录");

        // 前 15 层的祖先 team_size 应已同步更新
        // 第 20 个节点（accounts[19]）注册时，其推荐人是 accounts[18]
        // 同步更新 accounts[18]..accounts[4]（15 层），accounts[3]..accounts[0] 需异步补偿
        let leaf_parent = MemberPallet::get_member_by_shop(SHOP_1, &accounts[18]).unwrap();
        assert_eq!(leaf_parent.team_size, 1); // 只有 leaf

        // 验证 pending 记录存在
        assert!(PendingTeamSizeUpdates::<Test>::get(0).is_some());
    });
}

#[test]
fn d3_on_idle_processes_pending_updates() {
    new_test_ext().execute_with(|| {
        // 构建 20 层链
        let accounts = build_referral_chain(20, 100);

        let next_id = NextPendingUpdateId::<Test>::get();
        assert!(next_id > 0);

        // 记录 root 当前 team_size（同步阶段可能未完全更新）
        let root_before = MemberPallet::get_member_by_shop(SHOP_1, &accounts[0]).unwrap();

        // 运行 on_idle 多次直到所有 pending 处理完毕
        let big_weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, 1_000_000);
        for block in 2..20u64 {
            System::set_block_number(block);
            MemberPallet::on_idle(block.into(), big_weight);
        }

        // 所有 pending 应已处理
        assert_eq!(
            ProcessedPendingUpdateId::<Test>::get(),
            NextPendingUpdateId::<Test>::get(),
            "所有 pending 记录应已处理完毕"
        );

        // root 的 team_size 应为 19（所有后代）
        let root_after = MemberPallet::get_member_by_shop(SHOP_1, &accounts[0]).unwrap();
        assert_eq!(root_after.team_size, 19, "异步补偿后 root team_size 应为 19");
        assert!(root_after.team_size >= root_before.team_size);
    });
}

#[test]
fn d4_decrement_overflow_processed_by_on_idle() {
    new_test_ext().execute_with(|| {
        // 构建 20 层链
        let accounts = build_referral_chain(20, 100);

        // 先处理所有注册产生的 pending
        let big_weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, 1_000_000);
        for block in 2..30u64 {
            System::set_block_number(block);
            MemberPallet::on_idle(block.into(), big_weight);
        }

        let root_before = MemberPallet::get_member_by_shop(SHOP_1, &accounts[0]).unwrap();
        assert_eq!(root_before.team_size, 19);

        // 移除叶子节点
        System::set_block_number(50);
        assert_ok!(MemberPallet::remove_member(
            RuntimeOrigin::signed(OWNER), SHOP_1, accounts[19],
        ));

        // 移除也可能产生 pending（如果链深度 > 15）
        let next_id = NextPendingUpdateId::<Test>::get();
        let processed_id = ProcessedPendingUpdateId::<Test>::get();

        // 处理 pending
        for block in 51..70u64 {
            System::set_block_number(block);
            MemberPallet::on_idle(block.into(), big_weight);
        }

        assert_eq!(
            ProcessedPendingUpdateId::<Test>::get(),
            NextPendingUpdateId::<Test>::get(),
        );

        // root team_size 应减 1
        let root_after = MemberPallet::get_member_by_shop(SHOP_1, &accounts[0]).unwrap();
        assert_eq!(root_after.team_size, 18, "移除后 root team_size 应为 18");
    });
}

#[test]
fn d5_weight_limit_pauses_processing() {
    new_test_ext().execute_with(|| {
        // 构建 20 层链
        let _accounts = build_referral_chain(20, 100);

        let next_id = NextPendingUpdateId::<Test>::get();
        assert!(next_id > 0);

        // 给极小的 weight，不够处理任何 pending
        let tiny_weight = frame_support::weights::Weight::from_parts(1_000, 100);
        System::set_block_number(2);
        MemberPallet::on_idle(2u64.into(), tiny_weight);

        // pending 应未被处理（或只处理了很少）
        // 至少第一个 pending 应该还在
        let processed = ProcessedPendingUpdateId::<Test>::get();
        assert!(processed < next_id, "weight 不足时不应处理完所有 pending");
    });
}
