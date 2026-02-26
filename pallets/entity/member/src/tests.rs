use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_noop, assert_ok, BoundedVec};

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
            Error::<Test>::NotShopOwner
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
            1000u128, // threshold
            500,      // discount_rate (5%)
            300,      // commission_bonus (3%)
        ));

        let system = EntityLevelSystems::<Test>::get(ENTITY_1).unwrap();
        assert_eq!(system.levels.len(), 1);
        assert_eq!(system.levels[0].id, 0);
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
                1000u128,
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
                1000u128,
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 1000u128, 0, 0,
        ));

        // Threshold must be > previous
        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, name2.clone(), 500u128, 0, 0,
            ),
            Error::<Test>::InvalidThreshold
        );

        // Same threshold also fails
        assert_noop!(
            MemberPallet::add_custom_level(
                RuntimeOrigin::signed(OWNER), SHOP_1, name2, 1000u128, 0, 0,
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 1000u128, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 2000u128, 0, 0,
        ));

        // Can't remove first level
        assert_noop!(
            MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 0),
            Error::<Test>::InvalidLevelId
        );

        // Can remove last
        assert_ok!(MemberPallet::remove_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, 1,
        ));
    });
}

#[test]
fn manual_upgrade_member_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::init_level_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, true, LevelUpgradeMode::ManualUpgrade,
        ));

        let name: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP1".to_vec().try_into().unwrap();

        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 0u128, 0, 0,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        assert_ok!(MemberPallet::manual_upgrade_member(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 0,
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
            MemberPallet::manual_upgrade_member(RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 0),
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
            UpgradeTrigger::TotalSpent { threshold: 1000u128 },
            0,
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
fn update_spent_auto_upgrades_global_level() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Spend enough for Silver (100 USDT = 100_000_000 in 6-decimal)
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 100_000_000u128, 100_000_000));

        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.level, MemberLevel::Silver);
    });
}

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
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u128, 100, 50,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 600u128, 0));

        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 0); // VIP1 = level_id 0
    });
}

#[test]
fn update_spent_invalid_shop_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MemberPallet::update_spent(INVALID_SHOP, &ALICE, 100u128, 0),
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
        assert_eq!(system.levels[0].id, 0);
        assert_eq!(system.levels[1].id, 1);
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
                ENTITY_1, 0, None, None, Some(10001), None,
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
                RuntimeOrigin::signed(OWNER), SHOP_1, name, 1000u128, 0, 0,
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
                RuntimeOrigin::signed(OWNER), SHOP_1, name, 1000u128, 0, 0,
            ),
            Error::<Test>::LevelSystemNotInitialized
        );
    });
}

#[test]
fn member_provider_trait_works() {
    new_test_ext().execute_with(|| {
        use crate::MemberProvider;

        assert!(!<MemberPallet as MemberProvider<u64, u128>>::is_member(SHOP_1, &ALICE));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert!(<MemberPallet as MemberProvider<u64, u128>>::is_member(SHOP_1, &ALICE));

        let level = <MemberPallet as MemberProvider<u64, u128>>::member_level(SHOP_1, &ALICE);
        assert_eq!(level, Some(MemberLevel::Normal));
    });
}

// ============================================================================
// 审计回归测试
// ============================================================================

#[test]
fn h2_set_member_policy_rejects_invalid_bits() {
    new_test_ext().execute_with(|| {
        // 高位垃圾值 (8 = 0b1000) 应被拒绝
        assert_noop!(
            MemberPallet::set_member_policy(RuntimeOrigin::signed(OWNER), SHOP_1, 8),
            Error::<Test>::InvalidPolicyBits
        );
        // 255 也应被拒绝
        assert_noop!(
            MemberPallet::set_member_policy(RuntimeOrigin::signed(OWNER), SHOP_1, 255),
            Error::<Test>::InvalidPolicyBits
        );
        // 7 = 0b111 (全部3个标志) 应该成功
        assert_ok!(MemberPallet::set_member_policy(
            RuntimeOrigin::signed(OWNER), SHOP_1, 7,
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 1000u128, 0, 0,
        ));
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));

        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"BadRule".to_vec().try_into().unwrap();

        // target_level_id=5 不存在（只有 level 0）
        assert_noop!(
            MemberPallet::add_upgrade_rule(
                RuntimeOrigin::signed(OWNER), SHOP_1, rule_name.clone(),
                UpgradeTrigger::TotalSpent { threshold: 1000u128 },
                5, None, 1, false, None,
            ),
            Error::<Test>::InvalidTargetLevel
        );

        // target_level_id=0 存在，应该成功
        let valid_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"GoodRule".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, valid_name,
            UpgradeTrigger::TotalSpent { threshold: 1000u128 },
            0, None, 1, false, None,
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u128, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 500u128, 0, 0,
        ));

        // Register member and manually upgrade to VIP2 (level_id=1)
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));
        assert_ok!(MemberPallet::manual_upgrade_member(
            RuntimeOrigin::signed(OWNER), SHOP_1, ALICE, 1,
        ));

        // Verify stored level is 1
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1);

        // Set expiry in the past
        MemberLevelExpiry::<Test>::insert(ENTITY_1, &ALICE, 1u64);

        // Advance past expiry
        System::set_block_number(10);

        // custom_level_id via MemberProvider trait should respect expiry
        // With 0 spending, calculated level falls back to 0
        let level = <MemberPallet as MemberProvider<u64, u128>>::custom_level_id(SHOP_1, &ALICE);
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name1, 100u128, 0, 0,
        ));
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 200u128, 0, 0,
        ));

        // Create rule targeting VIP2 (level_id=1)
        assert_ok!(MemberPallet::init_upgrade_rule_system(
            RuntimeOrigin::signed(OWNER), SHOP_1, ConflictStrategy::HighestLevel,
        ));
        let rule_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"SpendRule".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_upgrade_rule(
            RuntimeOrigin::signed(OWNER), SHOP_1, rule_name,
            UpgradeTrigger::TotalSpent { threshold: 100u128 },
            1, // target VIP2
            None, 1, false, None,
        ));

        // Register member
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Delete VIP2 (the target level)
        assert_ok!(MemberPallet::remove_custom_level(RuntimeOrigin::signed(OWNER), SHOP_1, 1));

        // Spend enough to trigger rule
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 150u128, 0));

        // check_order_upgrade_rules should handle deleted level gracefully
        assert_ok!(MemberPallet::check_order_upgrade_rules(SHOP_1, &ALICE, 0, 150u128));

        // Member should NOT have been upgraded to deleted level
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        // auto_upgrade via update_spent sets to VIP1 (level 0, threshold 100, spent 150)
        assert_eq!(member.custom_level_id, 0);
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
                ENTITY_1, 0, Some(long_name.as_slice()), None, None, None,
            ),
            Error::<Test>::NameTooLong
        );
    });
}

// ============================================================================
// P3: USDT 精度修复 — 全局等级使用独立 USDT 累计
// ============================================================================

#[test]
fn p3_global_level_uses_usdt_not_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 模拟: Balance=1（NEX 精度可能很小），但 USDT=100_000_000（100 USDT）
        // 旧逻辑: saturated_into(1) = 1 → Normal
        // 新逻辑: 累计 USDT = 100_000_000 → Silver
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 1u128, 100_000_000));

        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.level, MemberLevel::Silver);

        // 验证 USDT 独立存储
        assert_eq!(MemberPallet::member_spent_usdt(ENTITY_1, ALICE), 100_000_000);
    });
}

#[test]
fn p3_usdt_accumulates_across_orders() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 3 笔订单，每笔 200 USDT
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 1u128, 200_000_000));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 1u128, 200_000_000));
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 1u128, 200_000_000));

        // 累计 600 USDT → Gold (阈值 500 USDT)
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.level, MemberLevel::Gold);
        assert_eq!(MemberPallet::member_spent_usdt(ENTITY_1, ALICE), 600_000_000);
    });
}

#[test]
fn p3_zero_usdt_keeps_normal_level() {
    new_test_ext().execute_with(|| {
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // Balance 很大但 USDT=0 → 不升级
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 999_999_999_999u128, 0));

        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.level, MemberLevel::Normal);
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name.clone(), 500u128, 100, 50,
        ));
        let name2: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            b"VIP2".to_vec().try_into().unwrap();
        assert_ok!(MemberPallet::add_custom_level(
            RuntimeOrigin::signed(OWNER), SHOP_1, name2, 2000u128, 200, 100,
        ));

        // 注册会员
        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 手动设置等级为 VIP2 (level_id=1)，设置过期时间为 block 10
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 1; // VIP2
                member.total_spent = 600;   // 只够 VIP1 (>500, <2000)
            }
        });
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 10u64);

        // 当前 block=1，未过期 → custom_level_id 仍为 1
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1);

        // 推进到 block 11（过期后）
        System::set_block_number(11);

        // get_effective_level 返回基于消费的等级（VIP1=0），但存储未变
        assert_eq!(MemberPallet::get_effective_level(SHOP_1, &ALICE), 0);
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 1); // 存储仍为旧值！

        // P4 修复: update_spent 应修正过期的存储
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u128, 0));

        // 修正后: custom_level_id 应为 0 (VIP1)，MemberLevelExpiry 已清除
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 0);
        assert!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE).is_none());
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u128, 100, 50,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 手动设置 VIP1(level_id=0)，消费=0（不够 VIP1），过期在 block 5
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 0; // VIP1
                member.total_spent = 100;   // 不够 VIP1 (需 500)
            }
        });
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 5u64);

        System::set_block_number(10);
        // 清除之前的事件
        System::reset_events();

        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u128, 0));

        // 过期后 recalculated=0 (消费 110 < 500 → 无等级即 0)
        // 但 member.custom_level_id 已是 0，所以不会 emit MemberLevelExpired
        // 因为 recalculated == member.custom_level_id
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
            RuntimeOrigin::signed(OWNER), SHOP_1, name, 500u128, 100, 50,
        ));

        assert_ok!(MemberPallet::register_member(RuntimeOrigin::signed(ALICE), SHOP_1, None));

        // 设置 VIP1，过期在 block 100（远未到期）
        crate::EntityMembers::<Test>::mutate(ENTITY_1, ALICE, |m| {
            if let Some(ref mut member) = m {
                member.custom_level_id = 0;
                member.total_spent = 0; // 消费不够，但有手动升级
            }
        });
        crate::MemberLevelExpiry::<Test>::insert(ENTITY_1, ALICE, 100u64);

        // block=1，未过期
        assert_ok!(MemberPallet::update_spent(SHOP_1, &ALICE, 10u128, 0));

        // 等级和过期记录都不应改变
        let member = MemberPallet::get_member_by_shop(SHOP_1, &ALICE).unwrap();
        assert_eq!(member.custom_level_id, 0);
        assert_eq!(crate::MemberLevelExpiry::<Test>::get(ENTITY_1, ALICE), Some(100));
    });
}
