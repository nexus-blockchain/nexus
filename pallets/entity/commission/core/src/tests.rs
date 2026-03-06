use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_ok, assert_noop};
use frame_support::BoundedVec;
use frame_support::traits::ConstU32;
use pallet_commission_common::{CommissionModes, CommissionStatus, CommissionType, MemberProvider, WithdrawalMode, WithdrawalTierConfig};

// ============================================================================
// Helpers
// ============================================================================

/// 配置会员返佣（招商奖金比例由全局常量 ReferrerShareBps=5000 控制）
fn setup_config(max_commission_rate: u16) {
    CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
        enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
        max_commission_rate,
        enabled: true,
        withdrawal_cooldown: 0,
        creator_reward_rate: 0,
        token_withdrawal_cooldown: 0,
    });
}

// ============================================================================
// set_commission_rate — Entity Owner 设置会员返佣上限
// ============================================================================

#[test]
fn set_commission_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            5000,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.max_commission_rate, 5000);
    });
}

#[test]
fn set_commission_rate_rejects_invalid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
                10001,
            ),
            Error::<Test>::InvalidCommissionRate
        );
    });
}

#[test]
fn set_commission_rate_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(BUYER),
                ENTITY_ID,
                5000,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ============================================================================
// process_commission — 平台费固定分配：50% 招商奖金 + 50% 国库
// ============================================================================

#[test]
fn referrer_gets_half_platform_fee() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        fund(SELLER, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        let platform_fee: Balance = 10_000;
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 1, &BUYER, 100_000, 100_000, platform_fee,
        ));

        // ReferrerShareBps=5000 → referrer = 10000 * 50% = 5000
        let records = OrderCommissionRecords::<Test>::get(1u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].beneficiary, REFERRER);
        assert_eq!(records[0].amount, 5000);
        assert_eq!(records[0].commission_type, CommissionType::EntityReferral);

        // Entity 账户收到佣金
        let ea = entity_account(ENTITY_ID);
        let entity_balance = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&ea);
        assert_eq!(entity_balance, 5000);

        // 国库收到另一半
        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 5000);
    });
}

#[test]
fn dual_source_both_pools_work() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        fund(SELLER, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(5000); // 50% seller rate

        let platform_fee: Balance = 10_000;
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 2, &BUYER, 100_000, 100_000, platform_fee,
        ));

        // 池A: referrer = 5000, treasury = 5000
        // 池B: max_commission = 100000 * 5000 / 10000 = 50000 (插件为空, 不分配)
        let records = OrderCommissionRecords::<Test>::get(2u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].amount, 5000);

        let (total, orders) = ShopCommissionTotals::<Test>::get(ENTITY_ID);
        assert_eq!(total, 5000);
        assert_eq!(orders, 1);
    });
}

#[test]
fn referrer_skipped_when_no_referrer() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 3, &BUYER, 100_000, 100_000, 10_000,
        ));

        // 无推荐人 → 无佣金记录，全部进国库
        let records = OrderCommissionRecords::<Test>::get(3u64);
        assert_eq!(records.len(), 0);
        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 10_000);
    });
}

#[test]
fn referrer_skipped_when_platform_fee_zero() {
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 5, &BUYER, 100_000, 100_000, 0,
        ));

        let records = OrderCommissionRecords::<Test>::get(5u64);
        assert_eq!(records.len(), 0);
    });
}

#[test]
fn referrer_amount_capped_by_platform_balance() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 501); // 501, transferable = 500
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        let platform_fee: Balance = 10_000;
        // referrer_quota = 10000 * 5000 / 10000 = 5000
        // treasury_portion = 5000, treasury_cap = 500 - 5000 = 0 (预留推荐人)
        // 国库 = 0，referrer transferable = 501 - 1 = 500
        // referrer_amount = min(5000, 500) = 500
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 6, &BUYER, 100_000, 100_000, platform_fee,
        ));

        let records = OrderCommissionRecords::<Test>::get(6u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].amount, 500);

        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 0);
    });
}

#[test]
fn referrer_stats_tracked_correctly() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7, &BUYER, 100_000, 100_000, 10_000,
        ));

        // referrer = 50% of 10000 = 5000
        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.total_earned, 5000);
        assert_eq!(stats.pending, 5000);

        let pending = ShopPendingTotal::<Test>::get(ENTITY_ID);
        assert_eq!(pending, 5000);
    });
}

// ============================================================================
// process_commission — 平台费剩余转国库
// ============================================================================

#[test]
fn platform_fee_50_50_split() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        let platform_fee: Balance = 10_000;
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 20, &BUYER, 100_000, 100_000, platform_fee,
        ));

        // 50/50: referrer=5000, treasury=5000
        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 5000);
        assert_eq!(OrderTreasuryTransfer::<Test>::get(20u64), 5000);
    });
}

#[test]
fn full_platform_fee_to_treasury_when_no_referrer() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 21, &BUYER, 100_000, 100_000, 10_000,
        ));

        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 10_000);
        assert_eq!(OrderTreasuryTransfer::<Test>::get(21u64), 10_000);
    });
}

#[test]
fn no_treasury_transfer_when_platform_fee_zero() {
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 22, &BUYER, 100_000, 100_000, 0,
        ));

        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 0);
        assert_eq!(OrderTreasuryTransfer::<Test>::get(22u64), 0);
    });
}

#[test]
fn treasury_transfer_capped_by_platform_balance() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 501); // 501, transferable = 500
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        let platform_fee: Balance = 10_000;
        // referrer_quota = 5000, treasury_portion = 5000
        // platform_transferable = 500, treasury_cap = 500 - 5000 = 0
        // actual_treasury = 0
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 23, &BUYER, 100_000, 100_000, platform_fee,
        ));

        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 0);
        assert_eq!(OrderTreasuryTransfer::<Test>::get(23u64), 0);
    });
}

// ============================================================================
// cancel_commission — 双来源退款 + 国库退款
// ============================================================================

#[test]
fn cancel_commission_refunds_all() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        let ea = entity_account(ENTITY_ID);
        fund(ea, 1); // 底仓

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 8, &BUYER, 100_000, 100_000, 10_000,
        ));

        // referrer=5000, treasury=5000
        let before = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&ea);
        assert_eq!(before, 5001); // 1 底仓 + 5000 佣金

        let treasury_before = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_before, 5000);

        let platform_before = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&PLATFORM);

        assert_ok!(CommissionCore::cancel_commission(8));

        // Entity 账户退回底仓
        let after = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&ea);
        assert_eq!(after, 1);

        // 国库全部退回
        let treasury_after = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_after, 0);

        // 平台收回佣金+国库
        let platform_after = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&PLATFORM);
        assert_eq!(platform_after, platform_before + 5000 + 5000);

        assert_eq!(OrderTreasuryTransfer::<Test>::get(8u64), 0);

        let records = OrderCommissionRecords::<Test>::get(8u64);
        assert_eq!(records[0].status, pallet_commission_common::CommissionStatus::Cancelled);

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.total_earned, 0);
    });
}

// ============================================================================
// process_commission — 未配置佣金时平台费仍入国库
// ============================================================================

#[test]
fn treasury_receives_platform_fee_even_without_commission_config() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 30, &BUYER, 100_000, 100_000, 10_000,
        ));

        // 无推荐人 → 全部 10000 进国库
        let treasury_bal = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&TREASURY);
        assert_eq!(treasury_bal, 10_000);
        assert_eq!(OrderTreasuryTransfer::<Test>::get(30u64), 10_000);

        let records = OrderCommissionRecords::<Test>::get(30u64);
        assert_eq!(records.len(), 0);
    });
}

// ============================================================================
// set_withdrawal_config — M1 审计修复: level_id 唯一性校验
// ============================================================================

#[test]
fn m1_set_withdrawal_config_rejects_duplicate_level_id() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::{WithdrawalMode, WithdrawalTierConfig};
        use frame_support::BoundedVec;
        use frame_support::traits::ConstU32;

        let tier_a = WithdrawalTierConfig { withdrawal_rate: 8000, repurchase_rate: 2000 };
        let tier_b = WithdrawalTierConfig { withdrawal_rate: 7000, repurchase_rate: 3000 };
        let default_tier = WithdrawalTierConfig { withdrawal_rate: 6000, repurchase_rate: 4000 };

        // 重复 level_id = 1
        let overrides: BoundedVec<(u8, WithdrawalTierConfig), ConstU32<10>> =
            vec![(1, tier_a.clone()), (1, tier_b.clone())].try_into().unwrap();

        assert_noop!(
            CommissionCore::set_withdrawal_config(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
                WithdrawalMode::LevelBased,
                default_tier.clone(),
                overrides,
                0,
                true,
            ),
            Error::<Test>::DuplicateLevelId
        );

        // 不重复时正常通过
        let overrides_ok: BoundedVec<(u8, WithdrawalTierConfig), ConstU32<10>> =
            vec![(0, tier_a), (1, tier_b)].try_into().unwrap();

        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::LevelBased,
            default_tier,
            overrides_ok,
            0,
            true,
        ));
    });
}

// ============================================================================
// init_commission_plan
// ============================================================================

#[test]
fn init_commission_plan_is_disabled() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::init_commission_plan(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
            ),
            crate::Error::<Test>::CommissionPlanDisabled
        );
    });
}

// ============================================================================
// H1: withdraw_commission 在 WithdrawalConfig disabled 时应拒绝
// ============================================================================

#[test]
fn h1_withdraw_blocked_when_config_disabled() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::{WithdrawalMode, WithdrawalTierConfig};
        use frame_support::BoundedVec;

        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 100_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        // 产生佣金
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 100, &BUYER, 100_000, 100_000, 10_000,
        ));
        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert!(stats.pending > 0);

        // 设置 WithdrawalConfig 但 enabled=false
        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 3000 },
            WithdrawalTierConfig { withdrawal_rate: 7000, repurchase_rate: 3000 },
            BoundedVec::default(),
            0,
            false, // disabled
        ));

        // 提现应失败
        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::WithdrawalConfigNotEnabled
        );
    });
}

#[test]
fn h1_withdraw_allowed_when_no_config() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 100_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 101, &BUYER, 100_000, 100_000, 10_000,
        ));

        // 无 WithdrawalConfig → 允许全额提现
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.withdrawn, 5000); // 50% of 10000 platform fee
    });
}

// ============================================================================
// H3: stats.repurchased 包含 bonus
// ============================================================================

#[test]
fn h3_repurchased_includes_bonus() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::{WithdrawalMode, WithdrawalTierConfig};
        use frame_support::BoundedVec;

        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 500_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 200, &BUYER, 100_000, 100_000, 10_000,
        ));

        // MemberChoice 模式: min=2000(20%), bonus_rate=1000(10%)
        // 会员请求 5000(50%) 复购
        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::MemberChoice { min_repurchase_rate: 2000 },
            WithdrawalTierConfig { withdrawal_rate: 8000, repurchase_rate: 2000 },
            BoundedVec::default(),
            1000, // 10% voluntary bonus
            true,
        ));

        let pending = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER).pending;
        assert_eq!(pending, 5000);

        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            Some(5000), // 请求 50% 复购
            None,
        ));

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        // repurchase = 5000 * 5000 / 10000 = 2500
        // mandatory_repurchase = 5000 * 2000 / 10000 = 1000
        // voluntary_extra = 2500 - 1000 = 1500
        // bonus = 1500 * 1000 / 10000 = 150
        // repurchased should include both repurchase(2500) + bonus(150)
        assert_eq!(stats.repurchased, 2500 + 150);
        assert_eq!(stats.withdrawn, 2500); // 5000 - 2500
        assert_eq!(stats.pending, 0);

        // 购物余额 = repurchase + bonus
        let shopping = MemberShoppingBalance::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(shopping, 2500 + 150);
    });
}

// ============================================================================
// M3: TieredWithdrawal 事件包含 repurchase_target
// ============================================================================

#[test]
fn m3_event_includes_repurchase_target() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::{WithdrawalMode, WithdrawalTierConfig};
        use frame_support::BoundedVec;

        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 500_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 300, &BUYER, 100_000, 100_000, 10_000,
        ));

        // FixedRate 30% 复购
        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 3000 },
            WithdrawalTierConfig { withdrawal_rate: 7000, repurchase_rate: 3000 },
            BoundedVec::default(),
            0,
            true,
        ));

        // 设置推荐关系: REFERRER 是 BUYER 在 entity 级的推荐人
        set_referrer(ENTITY_ID, BUYER, REFERRER);

        // REFERRER 提现，target=BUYER
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            Some(BUYER),
        ));

        // 验证事件包含 repurchase_target
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TieredWithdrawal {
                entity_id: ENTITY_ID,
                account: REFERRER,
                repurchase_target: BUYER,
                withdrawn_amount: 3500, // 5000 * 70%
                repurchase_amount: 1500, // 5000 * 30%
                bonus_amount: 0,
            },
        ));

        // 购物余额记入 BUYER
        let buyer_shopping = MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER);
        assert_eq!(buyer_shopping, 1500);
    });
}

// ============================================================================
// 基础复购场景: FixedRate 模式
// ============================================================================

#[test]
fn fixed_rate_withdrawal_split_works() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::{WithdrawalMode, WithdrawalTierConfig};
        use frame_support::BoundedVec;

        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 500_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 400, &BUYER, 100_000, 100_000, 20_000,
        ));
        // referrer gets 50% of 20000 = 10000

        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 4000 },
            WithdrawalTierConfig { withdrawal_rate: 6000, repurchase_rate: 4000 },
            BoundedVec::default(),
            0,
            true,
        ));

        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 6000); // 10000 * 60%
        assert_eq!(stats.repurchased, 4000); // 10000 * 40%
        assert_eq!(stats.pending, 0);

        let shopping = MemberShoppingBalance::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(shopping, 4000);

        let shop_shopping = ShopShoppingTotal::<Test>::get(ENTITY_ID);
        assert_eq!(shop_shopping, 4000);
    });
}

// ============================================================================
// Governance 底线 + FullWithdrawal 模式
// ============================================================================

#[test]
fn governance_floor_enforced_in_full_withdrawal_mode() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::{WithdrawalMode, WithdrawalTierConfig};
        use frame_support::BoundedVec;

        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 500_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 500, &BUYER, 100_000, 100_000, 20_000,
        ));

        // FullWithdrawal + governance floor at 30%
        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FullWithdrawal,
            WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            BoundedVec::default(),
            0,
            true,
        ));
        GlobalMinRepurchaseRate::<Test>::insert(ENTITY_ID, 3000u16);

        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        // Governance floor 30%: withdrawal = 70%, repurchase = 30%
        assert_eq!(stats.withdrawn, 7000); // 10000 * 70%
        assert_eq!(stats.repurchased, 3000); // 10000 * 30%

        let shopping = MemberShoppingBalance::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(shopping, 3000);
    });
}

// ============================================================================
// H3: ParticipationGuard (KYC) tests
// ============================================================================

#[test]
fn h3_withdraw_blocked_when_target_participation_denied() {
    new_test_ext().execute_with(|| {
        let target: u64 = 200;
        fund(entity_account(ENTITY_ID), 100_000);
        // 给 REFERRER 佣金
        MemberCommissionStats::<Test>::mutate(ENTITY_ID, &REFERRER, |s| {
            s.pending = 10_000;
        });
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 10_000u128);
        // 设置推荐关系 (target 的推荐人是 REFERRER)
        set_referrer(ENTITY_ID, target, REFERRER);

        // 标记 target 不满足参与要求（模拟 mandatory KYC 拒绝）
        block_participation(ENTITY_ID, target);

        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                Some(5_000),
                None,
                Some(target),
            ),
            Error::<Test>::TargetParticipationDenied
        );

        // 解除限制后应成功
        unblock_participation(ENTITY_ID, target);
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            Some(5_000),
            None,
            Some(target),
        ));
    });
}

#[test]
fn h3_consume_shopping_balance_blocked_when_participation_denied() {
    new_test_ext().execute_with(|| {
        fund(entity_account(ENTITY_ID), 100_000);
        // 给 REFERRER 购物余额
        MemberShoppingBalance::<Test>::insert(ENTITY_ID, REFERRER, 5_000u128);
        ShopShoppingTotal::<Test>::insert(ENTITY_ID, 5_000u128);

        // 标记 REFERRER 不满足参与要求
        block_participation(ENTITY_ID, REFERRER);

        assert_noop!(
            CommissionCore::do_consume_shopping_balance(ENTITY_ID, &REFERRER, 1_000),
            Error::<Test>::ParticipationRequirementNotMet
        );

        // 解除限制后应成功
        unblock_participation(ENTITY_ID, REFERRER);
        assert_ok!(CommissionCore::do_consume_shopping_balance(ENTITY_ID, &REFERRER, 1_000));
    });
}

#[test]
fn h1_self_withdraw_blocked_by_participation_guard() {
    // H1 审计修复: target=self 也经过 ParticipationGuard 检查（与 Token 提现一致）
    new_test_ext().execute_with(|| {
        fund(entity_account(ENTITY_ID), 100_000);
        MemberCommissionStats::<Test>::mutate(ENTITY_ID, &REFERRER, |s| {
            s.pending = 10_000;
        });
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 10_000u128);

        // 标记 REFERRER 自己不满足参与要求
        block_participation(ENTITY_ID, REFERRER);

        // H1: target=self 也被阻止
        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                Some(5_000),
                None,
                None, // target = self
            ),
            Error::<Test>::ParticipationRequirementNotMet
        );
    });
}

// ============================================================================
// 购物余额仅可用于购物，不可直接提取
// ============================================================================

#[test]
fn use_shopping_balance_extrinsic_always_rejected() {
    new_test_ext().execute_with(|| {
        fund(entity_account(ENTITY_ID), 100_000);
        MemberShoppingBalance::<Test>::insert(ENTITY_ID, REFERRER, 5_000u128);

        assert_noop!(
            CommissionCore::use_shopping_balance(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                1_000,
            ),
            Error::<Test>::ShoppingBalanceWithdrawalDisabled
        );

        // 余额不变
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, REFERRER), 5_000);
    });
}

// ============================================================================
// Phase 7a: withdraw_token_commission — Token 佣金提现测试
// ============================================================================

/// 辅助：手动注入 Token 佣金 pending 状态（模拟 credit_token_commission 的结果）
fn inject_token_pending(entity_id: u64, beneficiary: u64, amount: u128) {
    MemberTokenCommissionStats::<Test>::mutate(entity_id, &beneficiary, |stats| {
        stats.total_earned = stats.total_earned.saturating_add(amount);
        stats.pending = stats.pending.saturating_add(amount);
    });
    TokenPendingTotal::<Test>::mutate(entity_id, |total| {
        *total = total.saturating_add(amount);
    });
}

#[test]
fn token_withdraw_full_pending_works() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        // entity_account 需要有足够 Token 余额
        set_token_balance(ENTITY_ID, ea, 50_000);

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None, // 全额提现
            None,
            None,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.withdrawn, 10_000);
        assert_eq!(stats.total_earned, 10_000);
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);

        // Token 余额验证
        assert_eq!(get_token_balance(ENTITY_ID, ea), 40_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 10_000);

        // 事件验证（新版 TokenTieredWithdrawal 替代旧 TokenCommissionWithdrawn）
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenTieredWithdrawal {
                entity_id: ENTITY_ID,
                account: REFERRER,
                repurchase_target: REFERRER,
                withdrawn_amount: 10_000,
                repurchase_amount: 0,
                bonus_amount: 0,
            },
        ));
    });
}

#[test]
fn token_withdraw_partial_amount_works() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            Some(3_000),
            None,
            None,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 7_000);
        assert_eq!(stats.withdrawn, 3_000);
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 7_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 3_000);
    });
}

#[test]
fn token_withdraw_rejects_zero_amount() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        // pending = 0，全额提现 → ZeroWithdrawalAmount

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::ZeroWithdrawalAmount
        );
    });
}

#[test]
fn token_withdraw_rejects_insufficient_pending() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 5_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                Some(10_000), // 超过 pending
                None,
                None,
            ),
            Error::<Test>::InsufficientTokenCommission
        );
    });
}

#[test]
fn token_withdraw_rejects_when_entity_balance_insufficient() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 5_000); // 不足

        // M2 审计修复: 偿付能力不足使用专用错误码 InsufficientEntityTokenFunds
        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );

        // stats 不应变化（try_mutate 回滚）
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 10_000);
        assert_eq!(stats.withdrawn, 0);
    });
}

#[test]
fn token_withdraw_cooldown_enforced() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 设置带冻结期的佣金配置
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 100, // 100 blocks 冻结期
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        // P3 审计修复: Token 冻结期使用独立的 MemberTokenLastCredited
        // MemberTokenLastCredited 模拟为 block 1（当前 block=1），cooldown=100
        // now(1) < last(1) + cooldown(100) = 101 → 应被拒绝
        crate::pallet::MemberTokenLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 1u64);

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::WithdrawalCooldownNotMet
        );

        // 推进到 block 101 → 应通过
        System::set_block_number(101);
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.withdrawn, 10_000);
    });
}

#[test]
fn token_withdraw_multiple_partial_draws() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 第一次提 3000
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            Some(3_000),
            None,
            None,
        ));
        // 第二次提 4000
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            Some(4_000),
            None,
            None,
        ));
        // 第三次提剩余 3000
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.withdrawn, 10_000);
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 10_000);
        assert_eq!(get_token_balance(ENTITY_ID, ea), 40_000);
    });
}

// ============================================================================
// Phase 7b: 端到端集成测试 — Token 下单 → 佣金分配 → 提现/取消
// ============================================================================

/// 辅助：设置 Token 佣金配置（含 POOL_REWARD）
fn setup_token_config(max_commission_rate: u16, pool_reward: bool) {
    let mut modes = CommissionModes(CommissionModes::DIRECT_REWARD);
    if pool_reward {
        modes = CommissionModes(modes.0 | CommissionModes::POOL_REWARD);
    }
    CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
        enabled_modes: modes,
        max_commission_rate,
        enabled: true,
        withdrawal_cooldown: 0,
        creator_reward_rate: 0,
        token_withdrawal_cooldown: 0,
    });
}

#[test]
fn e2e_token_commission_all_to_pool_when_no_plugins() {
    // 插件都是 ()，所有 Token 佣金 → UnallocatedTokenPool
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, true); // 50% max, pool_reward=true

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 1001, &BUYER, 20_000, 20_000, 0,
        ));

        // max_commission = 20000 * 5000 / 10000 = 10000
        // entity_balance = 100000 → remaining = min(10000, 100000) = 10000
        // 插件全部为空 → remaining = 10000 → 全部入沉淀池
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);
        let (eid, sid, amt) = OrderTokenUnallocated::<Test>::get(1001u64);
        assert_eq!((eid, sid, amt), (ENTITY_ID, SHOP_ID, 10_000));

        // 无佣金记录（插件为空）
        assert_eq!(OrderTokenCommissionRecords::<Test>::get(1001u64).len(), 0);

        // buyer 订单数递增
        let buyer_stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, BUYER);
        assert_eq!(buyer_stats.order_count, 1);

        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenUnallocatedPooled {
                entity_id: ENTITY_ID,
                order_id: 1001,
                amount: 10_000,
            },
        ));
    });
}

#[test]
fn e2e_token_commission_capped_by_entity_balance() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 3_000); // 余额不足
        setup_token_config(5000, true);

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 1002, &BUYER, 20_000, 20_000, 0,
        ));

        // max_commission = 10000, entity_balance = 3000 → remaining = 3000
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 3_000);
    });
}

#[test]
fn e2e_token_commission_no_pool_without_pool_reward_mode() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, false); // pool_reward=false

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 1003, &BUYER, 20_000, 20_000, 0,
        ));

        // 无 POOL_REWARD 模式 → 剩余不入池
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
    });
}

#[test]
fn e2e_token_cancel_refunds_pool_and_records() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, true);

        // 产生 Token 佣金（全部入池）
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 1004, &BUYER, 20_000, 20_000, 0,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // 手动注入一条 Token 佣金记录（模拟插件分配）
        inject_token_pending(ENTITY_ID, REFERRER, 2_000);
        let record = pallet_commission_common::TokenCommissionRecord {
            entity_id: ENTITY_ID,
            order_id: 1004,
            buyer: BUYER,
            beneficiary: REFERRER,
            amount: 2_000u128,
            commission_type: CommissionType::DirectReward,
            level: 0,
            status: pallet_commission_common::CommissionStatus::Pending,
            created_at: 1u64,
        };
        OrderTokenCommissionRecords::<Test>::mutate(1004u64, |records| {
            let _ = records.try_push(record);
        });

        // 取消 Token 佣金
        assert_ok!(CommissionCore::do_cancel_token_commission(1004));

        // Token 记录已取消
        let records = OrderTokenCommissionRecords::<Test>::get(1004u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, pallet_commission_common::CommissionStatus::Cancelled);

        // 统计已回退
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.total_earned, 0);
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);

        // 沉淀池已退还（Token 转给 seller）
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
        assert_eq!(OrderTokenUnallocated::<Test>::get(1004u64), (0, 0, 0));
        // seller = SHOP_OWNERS[SHOP_ID] = SELLER
        assert_eq!(get_token_balance(ENTITY_ID, SELLER), 10_000);

        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenCommissionCancelled {
                order_id: 1004,
                cancelled_count: 1,
            },
        ));
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenUnallocatedPoolRefunded {
                entity_id: ENTITY_ID,
                order_id: 1004,
                amount: 10_000,
            },
        ));
    });
}

#[test]
fn e2e_token_process_then_withdraw_full_flow() {
    // 完整流程: process_token_commission → inject pending (模拟插件分配) → withdraw
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, true);

        // Step 1: 产生 Token 佣金
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 1005, &BUYER, 20_000, 20_000, 0,
        ));

        // Step 2: 模拟插件分配了 5000 给 REFERRER
        inject_token_pending(ENTITY_ID, REFERRER, 5_000);

        // Step 3: REFERRER 提现
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.withdrawn, 5_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 5_000);
        assert_eq!(get_token_balance(ENTITY_ID, ea), 95_000); // 100000 - 5000
    });
}

#[test]
fn e2e_token_partial_withdraw_then_cancel_remaining() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, true);

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 1006, &BUYER, 20_000, 20_000, 0,
        ));

        // 注入 8000 佣金给 REFERRER，并添加记录
        inject_token_pending(ENTITY_ID, REFERRER, 8_000);
        let record = pallet_commission_common::TokenCommissionRecord {
            entity_id: ENTITY_ID,
            order_id: 1006,
            buyer: BUYER,
            beneficiary: REFERRER,
            amount: 8_000u128,
            commission_type: CommissionType::DirectReward,
            level: 0,
            status: pallet_commission_common::CommissionStatus::Pending,
            created_at: 1u64,
        };
        OrderTokenCommissionRecords::<Test>::mutate(1006u64, |records| {
            let _ = records.try_push(record);
        });

        // Step 1: 部分提现 3000
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            Some(3_000),
            None,
            None,
        ));
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 3_000);

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 5_000);
        assert_eq!(stats.withdrawn, 3_000);

        // Step 2: 取消订单 → 剩余 pending 被取消
        assert_ok!(CommissionCore::do_cancel_token_commission(1006));

        let stats_after = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats_after.pending, 0);
        assert_eq!(stats_after.total_earned, 0); // total_earned 也被回退
        assert_eq!(stats_after.withdrawn, 3_000); // withdrawn 不变

        // 记录状态
        let records = OrderTokenCommissionRecords::<Test>::get(1006u64);
        assert_eq!(records[0].status, pallet_commission_common::CommissionStatus::Cancelled);
    });
}

#[test]
fn e2e_token_multiple_orders_accumulate() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 200_000);
        setup_token_config(5000, true);

        // 两笔订单
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 2001, &BUYER, 20_000, 20_000, 0,
        ));
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 2002, &BUYER, 30_000, 30_000, 0,
        ));

        // 第一笔: max = 10000, 第二笔: max = 15000 → 全部入池
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 25_000);

        // buyer 订单数 = 2
        let buyer_stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, BUYER);
        assert_eq!(buyer_stats.order_count, 2);

        // 取消第一笔
        assert_ok!(CommissionCore::do_cancel_token_commission(2001));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 15_000);
        assert_eq!(get_token_balance(ENTITY_ID, SELLER), 10_000);

        // 第二笔仍在
        let (eid, _, amt) = OrderTokenUnallocated::<Test>::get(2002u64);
        assert_eq!(eid, ENTITY_ID);
        assert_eq!(amt, 15_000);
    });
}

#[test]
fn e2e_cancel_nex_does_not_affect_token_records() {
    // 验证 NEX cancel_commission 也会处理 Token 记录（因为 cancel_commission 包含第六步）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 1);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);
        set_token_balance(ENTITY_ID, ea, 100_000);

        // 产生 NEX 佣金
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 3001, &BUYER, 100_000, 100_000, 10_000,
        ));

        // 同时注入 Token 佣金记录到同一 order_id
        inject_token_pending(ENTITY_ID, REFERRER, 5_000);
        let record = pallet_commission_common::TokenCommissionRecord {
            entity_id: ENTITY_ID,
            order_id: 3001,
            buyer: BUYER,
            beneficiary: REFERRER,
            amount: 5_000u128,
            commission_type: CommissionType::DirectReward,
            level: 0,
            status: pallet_commission_common::CommissionStatus::Pending,
            created_at: 1u64,
        };
        OrderTokenCommissionRecords::<Test>::mutate(3001u64, |records| {
            let _ = records.try_push(record);
        });

        // cancel_commission（NEX）也会取消 Token 记录（第六步）
        assert_ok!(CommissionCore::cancel_commission(3001));

        // NEX 佣金已取消
        let nex_records = OrderCommissionRecords::<Test>::get(3001u64);
        assert_eq!(nex_records[0].status, pallet_commission_common::CommissionStatus::Cancelled);

        // Token 记录也已取消
        let token_records = OrderTokenCommissionRecords::<Test>::get(3001u64);
        assert_eq!(token_records[0].status, pallet_commission_common::CommissionStatus::Cancelled);

        // Token pending 已回退
        let token_stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(token_stats.pending, 0);
    });
}

#[test]
fn e2e_token_commission_not_configured_returns_ok() {
    // M2-R5: 未配置时优雅返回 Ok（与 NEX 版对称）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        // 不配置 CommissionConfigs

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 4001, &BUYER, 20_000, 20_000, 0,
        ));
        // 未配置时不产生佣金记录
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);
    });
}

#[test]
fn e2e_token_commission_disabled_returns_ok() {
    // M2-R5: 配置存在但禁用时优雅返回 Ok（与 NEX 版对称）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 5000,
            enabled: false, // 已禁用
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 4002, &BUYER, 20_000, 20_000, 0,
        ));
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);
    });
}

// ============================================================================
// Phase 7c: 审计回归测试
// ============================================================================

#[test]
fn h1_token_withdraw_blocked_when_participation_denied() {
    // H1: withdraw_token_commission 需检查 ParticipationGuard
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 将 REFERRER 加入 KYC 黑名单
        block_participation(ENTITY_ID, REFERRER);

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::ParticipationRequirementNotMet
        );

        // 余额不应变化
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 10_000);
        assert_eq!(stats.withdrawn, 0);
    });
}

#[test]
fn h2_cancel_token_pool_refund_skipped_on_transfer_failure() {
    // H2: do_cancel_token_commission 转账失败时不应扣减池余额
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, true);

        // 产生 Token 佣金（10000 入池）
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 5001, &BUYER, 20_000, 20_000, 0,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // 将 entity_account 的 Token 余额清零，模拟偿付能力不足
        set_token_balance(ENTITY_ID, ea, 0);

        // 取消 Token 佣金
        assert_ok!(CommissionCore::do_cancel_token_commission(5001));

        // H2 修复: 转账失败 → 池余额不应被扣减，记录不应被移除
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);
        let (eid, _, amt) = OrderTokenUnallocated::<Test>::get(5001u64);
        assert_eq!(eid, ENTITY_ID);
        assert_eq!(amt, 10_000);
    });
}

#[test]
fn h2_cancel_token_pool_refund_succeeds_normally() {
    // H2 正常路径: 转账成功时池余额正常扣减
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        setup_token_config(5000, true);

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 5002, &BUYER, 20_000, 20_000, 0,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // entity_account 有足够余额 → 正常退还
        assert_ok!(CommissionCore::do_cancel_token_commission(5002));

        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
        assert_eq!(OrderTokenUnallocated::<Test>::get(5002u64), (0, 0, 0));
        assert_eq!(get_token_balance(ENTITY_ID, SELLER), 10_000);
    });
}

// ============================================================================
// Step G: 新功能测试 — extrinsics + Pool A + 复购分流 + 赠与提现 + 治理底线
// ============================================================================

// --- G3: set_token_withdrawal_config ---

#[test]
fn g3_set_token_withdrawal_config_fixed_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 3000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            500, // voluntary_bonus_rate = 5%
            true,
        ));
        let wc = TokenWithdrawalConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(wc.mode, WithdrawalMode::FixedRate { repurchase_rate: 3000 });
        assert_eq!(wc.voluntary_bonus_rate, 500);
        assert!(wc.enabled);
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenWithdrawalConfigUpdated { entity_id: ENTITY_ID },
        ));
    });
}

#[test]
fn g3_set_token_withdrawal_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_token_withdrawal_config(
                RuntimeOrigin::signed(BUYER),
                ENTITY_ID,
                WithdrawalMode::FullWithdrawal,
                WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
                Default::default(),
                0,
                true,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn g3_set_token_withdrawal_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_token_withdrawal_config(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
                WithdrawalMode::FixedRate { repurchase_rate: 10001 },
                WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
                Default::default(),
                0,
                true,
            ),
            Error::<Test>::InvalidWithdrawalConfig
        );
    });
}

// --- G4: set_global_min_token_repurchase_rate ---

#[test]
fn g4_set_global_min_token_repurchase_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_global_min_token_repurchase_rate(
            RuntimeOrigin::root(), ENTITY_ID, 2000,
        ));
        assert_eq!(GlobalMinTokenRepurchaseRate::<Test>::get(ENTITY_ID), 2000);
    });
}

#[test]
fn g4_set_global_min_token_repurchase_rate_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_min_token_repurchase_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 2000,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn g4_set_global_min_token_repurchase_rate_rejects_over_10000() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_min_token_repurchase_rate(
                RuntimeOrigin::root(), ENTITY_ID, 10001,
            ),
            Error::<Test>::InvalidCommissionRate
        );
    });
}

// --- G5: process_token_commission with Pool A (token_platform_fee > 0) ---

#[test]
fn g5_pool_a_referrer_gets_share_of_token_platform_fee() {
    // 设置 entity_referrer → 推荐人应从 token_platform_fee 获得分成
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 200_000);
        setup_token_config(5000, true);
        // 设置 Entity 招商推荐人
        set_entity_referrer(ENTITY_ID, REFERRER);

        // token_platform_fee = 1000, ReferrerShareBps = 5000 (50%)
        // → referrer 应得 500
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 6001, &BUYER, 20_000, 19_000, 1_000,
        ));

        // 推荐人应获得 500 Token 佣金（EntityReferral 类型）
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.total_earned, 500);
        assert_eq!(stats.pending, 500);

        // 验证 TokenCommissionDistributed 事件
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenCommissionDistributed {
                entity_id: ENTITY_ID,
                order_id: 6001,
                beneficiary: REFERRER,
                amount: 500,
                commission_type: CommissionType::EntityReferral,
                level: 0,
            },
        ));
    });
}

#[test]
fn g5_pool_a_no_referrer_means_all_stays_in_entity() {
    // 无 entity_referrer → token_platform_fee 全部留在 entity_account
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 200_000);
        setup_token_config(5000, true);
        // 不设置 entity_referrer

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 6002, &BUYER, 20_000, 19_000, 1_000,
        ));

        // 无推荐人 → 无 EntityReferral 佣金记录
        let records = OrderTokenCommissionRecords::<Test>::get(6002u64);
        assert!(records.iter().all(|r| r.commission_type != CommissionType::EntityReferral));
    });
}

#[test]
fn g5_pool_a_zero_fee_skips_referrer() {
    // token_platform_fee = 0 时不应有 Pool A 分配（即使有推荐人）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 200_000);
        setup_token_config(5000, true);
        set_entity_referrer(ENTITY_ID, REFERRER);

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 6003, &BUYER, 20_000, 20_000, 0,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.total_earned, 0);
    });
}

// --- G6: withdraw_token_commission with repurchase split ---

#[test]
fn g6_token_withdraw_with_fixed_rate_repurchase() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 设置 Token 提现配置: FixedRate 30% 复购
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 3000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            0, // 无自愿奖励
            true,
        ));

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        // 10000 × 70% = 7000 提现, 10000 × 30% = 3000 复购
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 7_000);
        assert_eq!(stats.repurchased, 3_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 7_000);
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, REFERRER), 3_000);
        assert_eq!(TokenShoppingTotal::<Test>::get(ENTITY_ID), 3_000);

        // 事件
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenTieredWithdrawal {
                entity_id: ENTITY_ID,
                account: REFERRER,
                repurchase_target: REFERRER,
                withdrawn_amount: 7_000,
                repurchase_amount: 3_000,
                bonus_amount: 0,
            },
        ));
    });
}

#[test]
fn g6_token_withdraw_member_choice_with_voluntary_bonus() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // MemberChoice: min 20%, voluntary_bonus_rate = 1000 (10%)
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::MemberChoice { min_repurchase_rate: 2000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            1000, // 10% 自愿奖励
            true,
        ));

        // 会员请求 50% 复购（超出最低 20%）
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            Some(5000), // 请求 50% 复购
            None,
        ));

        // 10000 × 50% = 5000 提现, 10000 × 50% = 5000 复购
        // mandatory_min = 20%, 强制复购 = 2000
        // voluntary_extra = 5000 - 2000 = 3000
        // bonus = 3000 × 10% = 300
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 5_000);
        assert_eq!(stats.repurchased, 5_300); // 5000 repurchase + 300 bonus
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 5_000);
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, REFERRER), 5_300);
    });
}

// --- G7: gifted withdrawal (repurchase_target != who) ---

#[test]
fn g7_gifted_withdrawal_to_existing_member_referral_ok() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // GIFT_TARGET 是会员，且推荐人是 REFERRER
        set_referrer(ENTITY_ID, GIFT_TARGET, REFERRER);

        // FixedRate 40% 复购
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 4000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            0,
            true,
        ));

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            Some(GIFT_TARGET), // 赠与目标
        ));

        // 6000 提现给 REFERRER, 4000 复购给 GIFT_TARGET
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 6_000);
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, GIFT_TARGET), 4_000);
        // REFERRER 的购物余额应为 0（复购目标是 GIFT_TARGET）
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, REFERRER), 0);

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 6_000);
        assert_eq!(stats.repurchased, 4_000);
    });
}

#[test]
fn g7_gifted_withdrawal_rejects_wrong_referrer() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // GIFT_TARGET 是会员但推荐人不是 REFERRER
        set_referrer(ENTITY_ID, GIFT_TARGET, BUYER); // 推荐人是 BUYER，不是 REFERRER

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                Some(GIFT_TARGET),
            ),
            Error::<Test>::NotDirectReferral
        );
    });
}

#[test]
fn g7_gifted_withdrawal_auto_registers_non_member() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // GIFT_TARGET 不是会员
        set_non_member(ENTITY_ID, GIFT_TARGET);

        // FixedRate 30% 复购
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 3000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            0,
            true,
        ));

        // 赠与给非会员 → 应触发自动注册
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            Some(GIFT_TARGET),
        ));

        // 注册后 GIFT_TARGET 成为会员
        assert!(MockMemberProvider::is_member(ENTITY_ID, &GIFT_TARGET));
        // 复购余额给 GIFT_TARGET
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, GIFT_TARGET), 3_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 7_000);
    });
}

// --- G8: governance floor enforcement ---

#[test]
fn g8_governance_floor_overrides_entity_full_withdrawal() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // Entity 设置 FullWithdrawal（无复购）
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FullWithdrawal,
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            0,
            true,
        ));

        // Governance 设置全局最低复购 20%
        assert_ok!(CommissionCore::set_global_min_token_repurchase_rate(
            RuntimeOrigin::root(), ENTITY_ID, 2000,
        ));

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        // FullWithdrawal 但 governance 强制 20% 复购
        // 10000 × 80% = 8000 提现, 10000 × 20% = 2000 复购
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 8_000);
        assert_eq!(stats.repurchased, 2_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 8_000);
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, REFERRER), 2_000);
    });
}

#[test]
fn g8_governance_floor_does_not_lower_entity_rate() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // Entity 设置 FixedRate 50% 复购
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 5000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            0,
            true,
        ));

        // Governance 底线 20%（低于 Entity 的 50%）
        assert_ok!(CommissionCore::set_global_min_token_repurchase_rate(
            RuntimeOrigin::root(), ENTITY_ID, 2000,
        ));

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        // Entity 50% > governance 20% → 最终 50% 复购
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 5_000);
        assert_eq!(stats.repurchased, 5_000);
    });
}

#[test]
fn g8_governance_floor_applies_when_no_token_withdrawal_config() {
    // 无 TokenWithdrawalConfig 时，calc_token_withdrawal_split 返回 (0,0,0) 模式
    // governance 底线仍生效
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 不设置任何 Token 提现配置

        // Governance 底线 30%
        assert_ok!(CommissionCore::set_global_min_token_repurchase_rate(
            RuntimeOrigin::root(), ENTITY_ID, 3000,
        ));

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        // 无 config → mode (0,0,0), governance 30% 兜底
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.withdrawn, 7_000);
        assert_eq!(stats.repurchased, 3_000);
    });
}

#[test]
fn g8_disabled_token_withdrawal_config_is_rejected() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // Token 提现配置存在但 enabled=false
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 8000 },
            WithdrawalTierConfig { repurchase_rate: 0, withdrawal_rate: 10000 },
            Default::default(),
            0,
            false,
        ));

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::WithdrawalConfigNotEnabled
        );
    });
}

// ============================================================================
// P1: withdraw_entity_funds / withdraw_entity_token_funds
// ============================================================================

#[test]
fn p1_withdraw_entity_funds_works() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        // 无任何锁定，全部可提（减去 minimum_balance=1）
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 99_999,
        ));
        let seller_bal = Balances::free_balance(SELLER);
        assert_eq!(seller_bal, 99_999);
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::EntityFundsWithdrawn { entity_id: ENTITY_ID, to: SELLER, amount: 99_999 },
        ));
    });
}

#[test]
fn p1_withdraw_entity_funds_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        assert_noop!(
            CommissionCore::withdraw_entity_funds(
                RuntimeOrigin::signed(BUYER), ENTITY_ID, 1_000,
            ),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn p1_withdraw_entity_funds_respects_locked_pools() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        // 开启 POOL_REWARD 使沉淀池锁定
        setup_token_config(5000, true);
        // 模拟锁定: pending=30000, shopping=20000, unallocated=10000 = 60000 locked
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 30_000u128);
        ShopShoppingTotal::<Test>::insert(ENTITY_ID, 20_000u128);
        UnallocatedPool::<Test>::insert(ENTITY_ID, 10_000u128);
        // available = 100000 - 60000 - 1(min_balance) = 39999
        assert_noop!(
            CommissionCore::withdraw_entity_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 40_000,
            ),
            Error::<Test>::InsufficientEntityFunds
        );
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 39_999,
        ));
    });
}

#[test]
fn p1_withdraw_entity_funds_rejects_zero() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        assert_noop!(
            CommissionCore::withdraw_entity_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 0,
            ),
            Error::<Test>::ZeroWithdrawalAmount
        );
    });
}

#[test]
fn p1_withdraw_entity_token_funds_pre_existing_balance_withdrawable() {
    // 首次访问: 已有余额视为合法运营资金，可提取
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 50_000,
        ));
        assert_eq!(get_token_balance(ENTITY_ID, ea), 0);
        assert_eq!(get_token_balance(ENTITY_ID, SELLER), 50_000);
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
    });
}

#[test]
fn p1_withdraw_entity_token_funds_external_transfer_swept() {
    // 已初始化 EntityTokenAccountedBalance 后，外部转入被 sweep 归入沉淀池
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        // 开启 POOL_REWARD 使沉淀池锁定
        setup_token_config(5000, true);
        // 第一次提取: 初始化 accounted = 50,000，提取 10,000
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 10_000,
        ));
        // accounted = 50,000 - 10,000 = 40,000, actual = 40,000
        assert_eq!(get_token_balance(ENTITY_ID, ea), 40_000);

        // 外部转入 5,000
        set_token_balance(ENTITY_ID, ea, 45_000);

        // 第二次提取: sweep 检测 external = 45,000 - 40,000 = 5,000 → UnallocatedTokenPool
        // available = 45,000 - (0 + 0 + 5,000) = 40,000
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 40_000,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 5_000);
        assert_eq!(get_token_balance(ENTITY_ID, ea), 5_000); // 外部转入留在沉淀池
        // 尝试提取沉淀池中的 5,000 → 失败（POOL_REWARD 开启，锁定）
        assert_noop!(
            CommissionCore::withdraw_entity_token_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 1,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );
    });
}

#[test]
fn p1_withdraw_entity_token_funds_respects_locked_pools() {
    // 池记账部分不可提取，仅合法 free 部分可提（需开启 POOL_REWARD）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        // 开启 POOL_REWARD 使沉淀池锁定
        setup_token_config(5000, true);
        TokenPendingTotal::<Test>::insert(ENTITY_ID, 20_000u128);
        TokenShoppingTotal::<Test>::insert(ENTITY_ID, 10_000u128);
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 5_000u128);
        // available = 50000 - 35000 = 15000（首次访问，全部视为合法）
        assert_noop!(
            CommissionCore::withdraw_entity_token_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 15_001,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 15_000,
        ));
    });
}

// ============================================================================
// P10: 沉淀池条件解锁 — POOL_REWARD 关闭 + cooldown 后可提取
// ============================================================================

#[test]
fn p10_token_pool_locked_when_pool_reward_enabled() {
    // POOL_REWARD 开启时，沉淀池资金不可提取
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        setup_token_config(5000, true);
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 20_000u128);
        // available = 50000 - 20000(locked pool) = 30000
        assert_noop!(
            CommissionCore::withdraw_entity_token_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 30_001,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 30_000,
        ));
        // 沉淀池不变
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 20_000);
    });
}

#[test]
fn p10_token_pool_cooldown_blocks_early_withdrawal() {
    // POOL_REWARD 关闭后 cooldown 期内不可提取池资金
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        // 先开启 POOL_REWARD
        setup_token_config(5000, true);
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 20_000u128);

        // 在 block 10 关闭 POOL_REWARD
        System::set_block_number(10);
        CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ).unwrap();
        assert_eq!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID), Some(10));

        // block 50: cooldown=100, 还没到 10+100=110
        System::set_block_number(50);
        // 提取 free 部分 (30000) 没问题
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 30_000,
        ));
        // 但尝试动用池资金 → cooldown 期内池锁定，余额不足
        assert_noop!(
            CommissionCore::withdraw_entity_token_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 1,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );
    });
}

#[test]
fn p10_token_pool_unlocked_after_cooldown() {
    // cooldown 到期后，池资金可提取并同步扣减
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        setup_token_config(5000, true);
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 20_000u128);

        // block 10: 关闭 POOL_REWARD
        System::set_block_number(10);
        CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ).unwrap();

        // block 110: cooldown(100) 到期
        System::set_block_number(110);
        // 全部 50000 可提取（池 20000 已解锁）
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 50_000,
        ));
        // 池被同步扣减至 0
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
        assert_eq!(get_token_balance(ENTITY_ID, ea), 0);
    });
}

#[test]
fn p10_token_pool_partial_withdrawal_decreases_pool() {
    // 部分提取动用池资金时，池余额同步递减
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        setup_token_config(5000, true);
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 20_000u128);

        // 关闭 POOL_REWARD 并跳过 cooldown
        System::set_block_number(10);
        CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ).unwrap();
        System::set_block_number(200);

        // free = 30000, pool = 20000, total available = 50000
        // 提 40000: free 用完, 从池中扣 10000
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 40_000,
        ));
        // actual = 10000, non_pool = 0, max_pool = 10000
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);
        assert_eq!(get_token_balance(ENTITY_ID, ea), 10_000);
    });
}

#[test]
fn p10_nex_pool_unlocked_after_cooldown() {
    // NEX 沉淀池同样在 POOL_REWARD 关闭 + cooldown 后可提取
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        setup_token_config(5000, true);
        UnallocatedPool::<Test>::insert(ENTITY_ID, 30_000u128);

        // POOL_REWARD 开启 → 池锁定
        // available = 100000 - 30000 - 1(min) = 69999
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 69_999,
        ));
        assert_eq!(UnallocatedPool::<Test>::get(ENTITY_ID), 30_000);

        // 关闭 POOL_REWARD
        System::set_block_number(10);
        CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ).unwrap();

        // cooldown 未满 → 池锁定，余额不足
        System::set_block_number(50);
        assert_noop!(
            CommissionCore::withdraw_entity_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 1,
            ),
            Error::<Test>::InsufficientEntityFunds
        );

        // cooldown 到期
        System::set_block_number(110);
        let remaining = Balances::free_balance(ea);
        let min_bal = 1u128; // ExistentialDeposit = 1 in test mock
        let pool_val = UnallocatedPool::<Test>::get(ENTITY_ID);
        let withdrawable = remaining.saturating_sub(min_bal);
        assert!(withdrawable >= pool_val);
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, withdrawable,
        ));
        // 池同步扣减
        assert_eq!(UnallocatedPool::<Test>::get(ENTITY_ID), 0);
    });
}

#[test]
fn p10_reopen_pool_reward_clears_cooldown() {
    // 重新开启 POOL_REWARD 清除 cooldown 记录，池重新锁定
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        setup_token_config(5000, true);
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 20_000u128);

        // 关闭 POOL_REWARD
        System::set_block_number(10);
        CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ).unwrap();
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_some());

        // 重新开启 POOL_REWARD → cooldown 清除
        System::set_block_number(20);
        CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
        ).unwrap();
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_none());

        // 池重新锁定，即使跳过足够多的区块也不可提取池资金
        System::set_block_number(500);
        // available = 50000 - 20000(locked) = 30000
        assert_noop!(
            CommissionCore::withdraw_entity_token_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 30_001,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );
    });
}

#[test]
fn p10_no_pool_reward_history_pool_unlocked() {
    // 从未开启过 POOL_REWARD → 池资金可直接提取（无 cooldown）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        // 配置不含 POOL_REWARD
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 20_000u128);
        // 池不锁定，全部 50000 可提取
        assert_ok!(CommissionCore::withdraw_entity_token_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 50_000,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
    });
}

// ============================================================================
// P2: do_consume_token_shopping_balance
// ============================================================================

#[test]
fn p2_consume_token_shopping_balance_works() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        // 注入 Token 购物余额
        MemberTokenShoppingBalance::<Test>::insert(ENTITY_ID, REFERRER, 10_000u128);
        TokenShoppingTotal::<Test>::insert(ENTITY_ID, 10_000u128);

        assert_ok!(CommissionCore::do_consume_token_shopping_balance(
            ENTITY_ID, &REFERRER, 6_000,
        ));
        assert_eq!(MemberTokenShoppingBalance::<Test>::get(ENTITY_ID, REFERRER), 4_000);
        assert_eq!(TokenShoppingTotal::<Test>::get(ENTITY_ID), 4_000);
        assert_eq!(get_token_balance(ENTITY_ID, REFERRER), 6_000);
        assert_eq!(get_token_balance(ENTITY_ID, ea), 44_000);
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenShoppingBalanceUsed { entity_id: ENTITY_ID, account: REFERRER, amount: 6_000 },
        ));
    });
}

#[test]
fn p2_consume_token_shopping_balance_rejects_insufficient() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        MemberTokenShoppingBalance::<Test>::insert(ENTITY_ID, REFERRER, 5_000u128);
        TokenShoppingTotal::<Test>::insert(ENTITY_ID, 5_000u128);

        assert_noop!(
            CommissionCore::do_consume_token_shopping_balance(ENTITY_ID, &REFERRER, 5_001),
            Error::<Test>::InsufficientShoppingBalance
        );
    });
}

#[test]
fn p2_consume_token_shopping_balance_rejects_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::do_consume_token_shopping_balance(ENTITY_ID, &REFERRER, 0),
            Error::<Test>::ZeroWithdrawalAmount
        );
    });
}

#[test]
fn p2_consume_token_shopping_balance_blocked_by_kyc() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);
        MemberTokenShoppingBalance::<Test>::insert(ENTITY_ID, REFERRER, 5_000u128);
        TokenShoppingTotal::<Test>::insert(ENTITY_ID, 5_000u128);

        block_participation(ENTITY_ID, REFERRER);
        assert_noop!(
            CommissionCore::do_consume_token_shopping_balance(ENTITY_ID, &REFERRER, 1_000),
            Error::<Test>::ParticipationRequirementNotMet
        );
    });
}

// ============================================================================
// 审计回归测试
// ============================================================================

#[test]
fn h2_set_token_withdrawal_config_rejects_invalid_tier_sum() {
    // H2: default_tier.withdrawal_rate + repurchase_rate 必须等于 10000
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_token_withdrawal_config(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
                WithdrawalMode::FixedRate { repurchase_rate: 3000 },
                WithdrawalTierConfig { repurchase_rate: 5000, withdrawal_rate: 4000 }, // sum=9000
                Default::default(),
                0,
                true,
            ),
            Error::<Test>::InvalidWithdrawalConfig
        );

        // 正确的 tier sum 应成功
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FixedRate { repurchase_rate: 3000 },
            WithdrawalTierConfig { repurchase_rate: 3000, withdrawal_rate: 7000 }, // sum=10000
            Default::default(),
            0,
            true,
        ));
    });
}

#[test]
fn h2_set_token_withdrawal_config_rejects_invalid_override_tier_sum() {
    // H2: level_overrides tier sum 也必须等于 10000
    new_test_ext().execute_with(|| {
        let bad_overrides: BoundedVec<(u8, WithdrawalTierConfig), ConstU32<10>> =
            BoundedVec::try_from(vec![
                (1, WithdrawalTierConfig { repurchase_rate: 6000, withdrawal_rate: 6000 }), // sum=12000
            ]).unwrap();
        assert_noop!(
            CommissionCore::set_token_withdrawal_config(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
                WithdrawalMode::LevelBased,
                WithdrawalTierConfig { repurchase_rate: 3000, withdrawal_rate: 7000 },
                bad_overrides,
                0,
                true,
            ),
            Error::<Test>::InvalidWithdrawalConfig
        );
    });
}

// ============================================================================
// H1 审计修复 (Round 4): Token 佣金预算扣除已承诺额度
// ============================================================================

#[test]
fn h1_token_budget_deducts_committed_amounts() {
    // H1: process_token_commission 第二笔订单的预算应扣除第一笔的已承诺额度
    // 防止跨订单重复承诺同一批 Token
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 15_000); // 余额有限
        setup_token_config(5000, true); // 50% max, pool_reward=true

        // Order 1: amount=20000, max_commission=10000, available=15000 → remaining=10000
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7001, &BUYER, 20_000, 20_000, 0,
        ));
        // 插件为空 → 10000 全部入沉淀池
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // Order 2: amount=20000, max_commission=10000
        // H1 修复后: available = 15000 - 10000(committed) = 5000 → remaining = min(10000, 5000) = 5000
        // 修复前: available = 15000 → remaining = min(10000, 15000) = 10000 (超额承诺!)
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7002, &BUYER, 20_000, 20_000, 0,
        ));
        // 第二笔只应承诺 5000（而非 10000）
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 15_000);

        // 总承诺 = 15000 = entity_balance，不超额
    });
}

#[test]
fn h1_token_budget_zero_when_fully_committed() {
    // 当已承诺额度 ≥ entity_balance 时，新订单应零分配
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 10_000);
        setup_token_config(5000, true);

        // Order 1: 全部可用余额被承诺
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7003, &BUYER, 20_000, 20_000, 0,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // Order 2: available = 10000 - 10000 = 0 → remaining = 0
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7004, &BUYER, 20_000, 20_000, 0,
        ));
        // 沉淀池不应增长
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // 第二笔订单的 unallocated 应为 0
        let (_, _, amt) = OrderTokenUnallocated::<Test>::get(7004u64);
        assert_eq!(amt, 0);
    });
}

#[test]
fn h1_token_budget_accounts_for_pending_and_shopping() {
    // 验证 committed 包含 TokenPendingTotal + TokenShoppingTotal + UnallocatedTokenPool
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 30_000);
        setup_token_config(5000, true);

        // 手动注入各类已承诺额度
        inject_token_pending(ENTITY_ID, REFERRER, 8_000);   // TokenPendingTotal = 8000
        TokenShoppingTotal::<Test>::insert(ENTITY_ID, 7_000u128); // TokenShoppingTotal = 7000
        UnallocatedTokenPool::<Test>::insert(ENTITY_ID, 5_000u128); // UnallocatedTokenPool = 5000
        // committed = 8000 + 7000 + 5000 = 20000
        // available = 30000 - 20000 = 10000

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7005, &BUYER, 40_000, 40_000, 0,
        ));
        // max_commission = 40000 * 5000 / 10000 = 20000
        // remaining = min(20000, 10000) = 10000
        // 插件为空 → 10000 入沉淀池
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 5_000 + 10_000);
    });
}

#[test]
fn h3_cancel_commission_refunds_pool_b_via_shop_id() {
    // H3: credit_commission 现在存储真实 shop_id，cancel_commission 可正确退款
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 1);
        fund(PLATFORM, 1_000_000);
        fund(SELLER, 200_000);
        setup_config(5000); // max_commission_rate = 50%

        // 处理佣金（Pool B 需要 seller 有余额）
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 999, &BUYER, 100_000u128, 100_000u128, 10_000u128,
        ));

        // 验证记录中的 shop_id 不为 0
        let records = OrderCommissionRecords::<Test>::get(999);
        for record in records.iter() {
            if record.commission_type != CommissionType::EntityReferral {
                assert_eq!(record.shop_id, SHOP_ID, "Pool B records should have real shop_id");
            }
        }

        // 取消佣金 — 应成功退款
        assert_ok!(CommissionCore::cancel_commission(999));

        // 验证 Pool B 记录被取消
        let records_after = OrderCommissionRecords::<Test>::get(999);
        for record in records_after.iter() {
            assert_eq!(record.status, CommissionStatus::Cancelled);
        }
    });
}

#[test]
fn h4_trait_set_commission_modes_tracks_pool_reward() {
    // H4: CommissionProvider::set_commission_modes 现在跟踪 POOL_REWARD 变化
    use pallet_commission_common::CommissionProvider;
    new_test_ext().execute_with(|| {
        // M1-R6: 必须先设置 enabled=true，否则重新开启 POOL_REWARD 不会清除 cooldown
        CommissionConfigs::<Test>::mutate(ENTITY_ID, |maybe| {
            let config = maybe.get_or_insert_with(CoreCommissionConfig::default);
            config.enabled = true;
        });

        // 开启 POOL_REWARD
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_commission_modes(
            ENTITY_ID,
            CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD,
        ));
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_none());

        // 关闭 POOL_REWARD → 应记录关闭时间
        System::set_block_number(42);
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_commission_modes(
            ENTITY_ID,
            CommissionModes::DIRECT_REWARD,
        ));
        assert_eq!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID), Some(42));

        // 重新开启 POOL_REWARD → 应清除关闭记录
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_commission_modes(
            ENTITY_ID,
            CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD,
        ));
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_none());
    });
}

// ============================================================================
// P3 审计修复: Token 冻结期与 NEX 完全解耦回归测试
// ============================================================================

#[test]
fn p3_nex_credit_does_not_freeze_token_withdrawal() {
    // NEX 入账更新 MemberLastCredited，不应阻止 Token 提现
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 设置 100 blocks 冻结期
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 100,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        // 模拟 NEX 入账 → 更新 MemberLastCredited
        System::set_block_number(50);
        crate::pallet::MemberLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 50u64);
        // MemberTokenLastCredited 未被更新（默认 0）

        // Token 提现不应被 NEX 冻结期阻止
        // now(50) >= token_last(0) + cooldown(100) = 100 → false, 但 0 + 100 = 100 > 50
        // 需要推进到 block 100
        System::set_block_number(100);
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.withdrawn, 10_000);
    });
}

#[test]
fn p3_token_credit_does_not_freeze_nex_withdrawal() {
    // Token 入账更新 MemberTokenLastCredited，不应阻止 NEX 提现
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 500_000);
        // 注入 NEX pending 佣金
        MemberCommissionStats::<Test>::mutate(ENTITY_ID, &REFERRER, |stats| {
            stats.total_earned = 10_000;
            stats.pending = 10_000;
        });
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 10_000u128);

        // 设置 100 blocks 冻结期
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 100,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        // 模拟 Token 入账 → 更新 MemberTokenLastCredited（block 50）
        System::set_block_number(50);
        crate::pallet::MemberTokenLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 50u64);
        // MemberLastCredited 未被更新（默认 0）

        // NEX 提现不应被 Token 冻结期影响
        // now(50) >= nex_last(0) + cooldown(100) = 100 → false
        System::set_block_number(100);
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(stats.pending, 0);
    });
}

#[test]
fn p3_token_cooldown_independent_from_nex_cooldown() {
    // Token 和 NEX 各自入账、各自冻结，互不干扰
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 100,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        // Token 入账在 block 200
        System::set_block_number(200);
        crate::pallet::MemberTokenLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 200u64);
        // NEX 入账在 block 10（很早）
        crate::pallet::MemberLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 10u64);

        // 在 block 250: NEX 冻结期早已过（10+100=110 < 250），Token 冻结期未过（200+100=300 > 250）
        System::set_block_number(250);

        // Token 应被拒绝
        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::WithdrawalCooldownNotMet
        );

        // 推进到 block 300: Token 冻结期也过了
        System::set_block_number(300);
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            None,
            None,
            None,
        ));
    });
}

#[test]
fn p3_credit_token_commission_writes_token_last_credited() {
    // credit_token_commission 应写入 MemberTokenLastCredited，不写 MemberLastCredited
    new_test_ext().execute_with(|| {
        System::set_block_number(42);

        assert_ok!(CommissionCore::credit_token_commission(
            ENTITY_ID, 1, &BUYER, &REFERRER,
            5_000u128, CommissionType::DirectReward, 1, 42u64,
        ));

        // MemberTokenLastCredited 应被更新
        assert_eq!(
            crate::pallet::MemberTokenLastCredited::<Test>::get(ENTITY_ID, REFERRER),
            42u64
        );

        // MemberLastCredited 不应被更新（保持默认值 0）
        assert_eq!(
            crate::pallet::MemberLastCredited::<Test>::get(ENTITY_ID, REFERRER),
            0u64
        );
    });
}

// ============================================================================
// Round 4 审计回归测试
// ============================================================================

#[test]
fn m1_set_global_min_token_repurchase_rate_emits_event() {
    // M1: set_global_min_token_repurchase_rate 应发射 GlobalMinTokenRepurchaseRateSet 事件
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_global_min_token_repurchase_rate(
            RuntimeOrigin::root(), ENTITY_ID, 2500,
        ));
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::GlobalMinTokenRepurchaseRateSet {
                entity_id: ENTITY_ID,
                rate: 2500,
            },
        ));
    });
}

#[test]
fn l5_set_min_repurchase_rate_via_trait_emits_event() {
    // L5: CommissionProvider::set_min_repurchase_rate 应发射 GlobalMinRepurchaseRateSet 事件
    use pallet_commission_common::CommissionProvider;
    new_test_ext().execute_with(|| {
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_min_repurchase_rate(
            ENTITY_ID, 3000,
        ));
        assert_eq!(GlobalMinRepurchaseRate::<Test>::get(ENTITY_ID), 3000);
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::GlobalMinRepurchaseRateSet {
                entity_id: ENTITY_ID,
                rate: 3000,
            },
        ));
    });
}

#[test]
fn m2_withdraw_commission_solvency_uses_entity_funds_error() {
    // M2: Entity 偿付能力不足应返回 InsufficientEntityFunds（而非 InsufficientCommission）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        // entity_account 余额极低，不足以覆盖提现 + 剩余承诺
        fund(ea, 100);
        // 注入大量 pending 佣金
        MemberCommissionStats::<Test>::mutate(ENTITY_ID, &REFERRER, |stats| {
            stats.total_earned = 50_000;
            stats.pending = 50_000;
        });
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 50_000u128);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        // pending 足够（50000 >= 1000），但 entity 余额不足
        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                Some(1_000),
                None,
                None,
            ),
            Error::<Test>::InsufficientEntityFunds
        );
    });
}

#[test]
fn m2_withdraw_token_commission_solvency_uses_entity_token_funds_error() {
    // M2: Token Entity 偿付能力不足应返回 InsufficientEntityTokenFunds
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        // entity_account Token 余额极低
        set_token_balance(ENTITY_ID, ea, 10);
        inject_token_pending(ENTITY_ID, REFERRER, 50_000);

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                Some(1_000),
                None,
                None,
            ),
            Error::<Test>::InsufficientEntityTokenFunds
        );
    });
}

#[test]
fn m3_cancel_commission_still_cancels_token_records() {
    // M3: 重构后 cancel_commission 仍能正确取消 Token 记录（通过 do_cancel_token_commission）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 1);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);
        set_token_balance(ENTITY_ID, ea, 100_000);

        // 产生 NEX 佣金
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 8001, &BUYER, 100_000, 100_000, 10_000,
        ));

        // 注入 Token 佣金记录到同一 order_id
        inject_token_pending(ENTITY_ID, REFERRER, 5_000);
        let record = pallet_commission_common::TokenCommissionRecord {
            entity_id: ENTITY_ID,
            order_id: 8001,
            buyer: BUYER,
            beneficiary: REFERRER,
            amount: 5_000u128,
            commission_type: CommissionType::DirectReward,
            level: 0,
            status: pallet_commission_common::CommissionStatus::Pending,
            created_at: 1u64,
        };
        OrderTokenCommissionRecords::<Test>::mutate(8001u64, |records| {
            let _ = records.try_push(record);
        });

        assert_ok!(CommissionCore::cancel_commission(8001));

        // Token 记录应被取消
        let token_records = OrderTokenCommissionRecords::<Test>::get(8001u64);
        assert_eq!(token_records[0].status, pallet_commission_common::CommissionStatus::Cancelled);
        let token_stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(token_stats.pending, 0);

        // TokenCommissionCancelled 事件应被发射
        System::assert_has_event(RuntimeEvent::CommissionCore(
            crate::pallet::Event::TokenCommissionCancelled {
                order_id: 8001,
                cancelled_count: 1,
            },
        ));
    });
}

#[test]
fn m4_trait_set_commission_modes_uses_is_valid() {
    // M4: CommissionProvider::set_commission_modes 应拒绝含未知位的模式
    use pallet_commission_common::CommissionProvider;
    new_test_ext().execute_with(|| {
        // 有效模式: DIRECT_REWARD | POOL_REWARD = 0b10_0000_0001 = 513
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_commission_modes(
            ENTITY_ID, 513,
        ));

        // CREATOR_REWARD = 0b100_0000_0000 = 1024, 也是有效的
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_commission_modes(
            ENTITY_ID, CommissionModes::CREATOR_REWARD,
        ));

        // 无效模式: 设置一个超出 ALL_VALID 的高位 (bit 12 = 4096)
        assert_noop!(
            <CommissionCore as CommissionProvider<u64, u128>>::set_commission_modes(
                ENTITY_ID, 4096,
            ),
            sp_runtime::DispatchError::Other("InvalidModes")
        );
    });
}

// ============================================================================
// Creator Reward: set_creator_reward_rate extrinsic
// ============================================================================

#[test]
fn set_creator_reward_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            3000,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.creator_reward_rate, 3000);
    });
}

#[test]
fn set_creator_reward_rate_rejects_over_5000() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(SELLER),
                ENTITY_ID,
                5001,
            ),
            Error::<Test>::InvalidCommissionRate
        );
    });
}

#[test]
fn set_creator_reward_rate_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(BUYER),
                ENTITY_ID,
                3000,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

// ============================================================================
// R8: creator_reward_rate — None+Locked 单调递减豁免
// ============================================================================

#[test]
fn r8_locked_none_allows_decrease() {
    new_test_ext().execute_with(|| {
        // Set initial rate to 3000
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
        ));

        // Lock in None mode
        set_entity_locked(ENTITY_ID);
        set_governance_mode(ENTITY_ID, 0); // None

        // Decrease to 2000 → allowed
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 2000,
        ));
        assert_eq!(CommissionConfigs::<Test>::get(ENTITY_ID).unwrap().creator_reward_rate, 2000);

        // Decrease to 0 → allowed
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 0,
        ));
        assert_eq!(CommissionConfigs::<Test>::get(ENTITY_ID).unwrap().creator_reward_rate, 0);
    });
}

#[test]
fn r8_locked_none_blocks_increase() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 2000,
        ));
        set_entity_locked(ENTITY_ID);
        set_governance_mode(ENTITY_ID, 0);

        // Increase from 2000 → 3000 → blocked
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
            ),
            Error::<Test>::LockedOnlyDecreaseAllowed
        );
    });
}

#[test]
fn r8_locked_none_blocks_same_value() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 2000,
        ));
        set_entity_locked(ENTITY_ID);
        set_governance_mode(ENTITY_ID, 0);

        // Same value (2000 → 2000) → blocked (must be strictly less)
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 2000,
            ),
            Error::<Test>::LockedOnlyDecreaseAllowed
        );
    });
}

#[test]
fn r8_locked_fulldao_blocks_all() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
        ));
        set_entity_locked(ENTITY_ID);
        set_governance_mode(ENTITY_ID, 1); // FullDAO

        // FullDAO locked → EntityLocked (even decrease)
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn r8_unlocked_allows_any_value() {
    new_test_ext().execute_with(|| {
        // Not locked → free to set any value
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
        ));
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 5000,
        ));
        assert_ok!(CommissionCore::set_creator_reward_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
        ));
        assert_eq!(CommissionConfigs::<Test>::get(ENTITY_ID).unwrap().creator_reward_rate, 1000);
    });
}

#[test]
fn r8_locked_none_no_config_blocks_increase() {
    new_test_ext().execute_with(|| {
        // No config exists → current = 0
        set_entity_locked(ENTITY_ID);
        set_governance_mode(ENTITY_ID, 0);

        // Trying to set to any positive value from 0 → increase → blocked
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
            ),
            Error::<Test>::LockedOnlyDecreaseAllowed
        );
    });
}

// ============================================================================
// Creator Reward: NEX process_commission
// ============================================================================

/// 辅助：配置带创建人收益的佣金
fn setup_creator_config(max_commission_rate: u16, creator_reward_rate: u16) {
    CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
        enabled_modes: CommissionModes(
            CommissionModes::DIRECT_REWARD | CommissionModes::CREATOR_REWARD
        ),
        max_commission_rate,
        enabled: true,
        withdrawal_cooldown: 0,
        creator_reward_rate,
        token_withdrawal_cooldown: 0,
    });
}

#[test]
fn creator_reward_nex_basic() {
    // 创建人收益从 Pool B 预算中优先扣除
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        fund(SELLER, 1_000_000);
        setup_creator_config(5000, 2000); // max 50%, creator 20% of pool B

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9001, &BUYER, 100_000, 100_000, 0,
        ));

        // Pool B = 100_000 * 5000 / 10000 = 50_000
        // creator_amount = 50_000 * 2000 / 10000 = 10_000
        let records = OrderCommissionRecords::<Test>::get(9001u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].beneficiary, SELLER); // entity_owner = SELLER
        assert_eq!(records[0].amount, 10_000);
        assert_eq!(records[0].commission_type, CommissionType::CreatorReward);

        // Creator stats tracked
        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, SELLER);
        assert_eq!(stats.total_earned, 10_000);
        assert_eq!(stats.pending, 10_000);
    });
}

#[test]
fn creator_reward_nex_with_referrer() {
    // 创建人收益 + 招商推荐人奖金共存
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        fund(SELLER, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_creator_config(5000, 3000); // max 50%, creator 30% of pool B

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9002, &BUYER, 100_000, 100_000, 10_000,
        ));

        // Pool A: referrer = 10_000 * 50% = 5_000, treasury = 5_000
        // Pool B = 100_000 * 5000 / 10000 = 50_000
        // creator_amount = 50_000 * 3000 / 10000 = 15_000
        let records = OrderCommissionRecords::<Test>::get(9002u64);
        assert_eq!(records.len(), 2);

        // 第一条: EntityReferral
        assert_eq!(records[0].commission_type, CommissionType::EntityReferral);
        assert_eq!(records[0].beneficiary, REFERRER);
        assert_eq!(records[0].amount, 5_000);

        // 第二条: CreatorReward
        assert_eq!(records[1].commission_type, CommissionType::CreatorReward);
        assert_eq!(records[1].beneficiary, SELLER);
        assert_eq!(records[1].amount, 15_000);
    });
}

#[test]
fn creator_reward_nex_disabled_when_mode_off() {
    // CREATOR_REWARD 模式位未启用时不分配
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD), // 无 CREATOR_REWARD
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 3000, // 已设置但模式位未开
            token_withdrawal_cooldown: 0,
        });

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9003, &BUYER, 100_000, 100_000, 0,
        ));

        // 无创建人记录
        let records = OrderCommissionRecords::<Test>::get(9003u64);
        assert_eq!(records.len(), 0);
    });
}

#[test]
fn creator_reward_nex_zero_rate_no_record() {
    // creator_reward_rate = 0 时不分配
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        setup_creator_config(5000, 0); // rate = 0

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9004, &BUYER, 100_000, 100_000, 0,
        ));

        let records = OrderCommissionRecords::<Test>::get(9004u64);
        assert_eq!(records.len(), 0);
    });
}

#[test]
fn creator_reward_nex_reduces_remaining_for_plugins() {
    // 创建人收益减少了剩余给插件的预算
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        setup_creator_config(10000, 5000); // max 100%, creator 50% of pool B

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9005, &BUYER, 100_000, 100_000, 0,
        ));

        // Pool B = 100_000 * 10000 / 10000 = 100_000
        // creator_amount = 100_000 * 5000 / 10000 = 50_000
        // remaining for plugins = 50_000 (but all plugins are (), so no further distribution)
        let records = OrderCommissionRecords::<Test>::get(9005u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].amount, 50_000);
        assert_eq!(records[0].commission_type, CommissionType::CreatorReward);

        // Entity funds transfer: only creator commission = 50_000
        let ea = entity_account(ENTITY_ID);
        let entity_balance = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&ea);
        assert_eq!(entity_balance, 50_000);
    });
}

// ============================================================================
// Creator Reward: Token process_token_commission
// ============================================================================

#[test]
fn creator_reward_token_basic() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(
                CommissionModes::DIRECT_REWARD | CommissionModes::CREATOR_REWARD
            ),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 2000,
            token_withdrawal_cooldown: 0,
        });

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 9010, &BUYER, 80_000, 80_000, 0,
        ));

        // max_commission = 80_000 * 5000 / 10000 = 40_000
        // available_token = 100_000 - 0 = 100_000, remaining = min(40_000, 100_000) = 40_000
        // creator_amount = 40_000 * 2000 / 10000 = 8_000
        let records = OrderTokenCommissionRecords::<Test>::get(9010u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].beneficiary, SELLER);
        assert_eq!(records[0].amount, 8_000);
        assert_eq!(records[0].commission_type, CommissionType::CreatorReward);

        // Token stats tracked
        let stats = MemberTokenCommissionStats::<Test>::get(ENTITY_ID, SELLER);
        assert_eq!(stats.total_earned, 8_000);
        assert_eq!(stats.pending, 8_000);
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 8_000);
    });
}

#[test]
fn creator_reward_token_disabled_when_mode_off() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD), // 无 CREATOR_REWARD
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 3000,
            token_withdrawal_cooldown: 0,
        });

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 9011, &BUYER, 80_000, 80_000, 0,
        ));

        // 无 Token 创建人记录
        let records = OrderTokenCommissionRecords::<Test>::get(9011u64);
        assert_eq!(records.len(), 0);
    });
}

// ============================================================================
// Creator Reward: cancel_commission routing
// ============================================================================

#[test]
fn cancel_commission_refunds_creator_reward_to_seller() {
    // CreatorReward 退款路由: entity_account → seller（与其他会员佣金相同）
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        let ea = entity_account(ENTITY_ID);
        fund(ea, 1); // 底仓
        setup_creator_config(5000, 2000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9020, &BUYER, 100_000, 100_000, 0,
        ));

        // creator = 10_000, entity got 10_000
        let entity_before = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&ea);
        assert_eq!(entity_before, 10_001); // 1 底仓 + 10_000 佣金
        let seller_before = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&SELLER);

        assert_ok!(CommissionCore::cancel_commission(9020));

        // Entity 退回底仓
        let entity_after = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&ea);
        assert_eq!(entity_after, 1);

        // Seller 拿回佣金
        let seller_after = <pallet_balances::Pallet<Test> as frame_support::traits::Currency<u64>>::free_balance(&SELLER);
        assert_eq!(seller_after, seller_before + 10_000);

        // 记录已取消
        let records = OrderCommissionRecords::<Test>::get(9020u64);
        assert_eq!(records[0].status, CommissionStatus::Cancelled);

        // Stats cleared
        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, SELLER);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.total_earned, 0);
    });
}

// ============================================================================
// Creator Reward: CommissionProvider trait method
// ============================================================================

#[test]
fn trait_set_creator_reward_rate_works() {
    use pallet_commission_common::CommissionProvider;
    new_test_ext().execute_with(|| {
        assert_ok!(<CommissionCore as CommissionProvider<u64, u128>>::set_creator_reward_rate(
            ENTITY_ID, 2500,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.creator_reward_rate, 2500);

        // 超出上限 5000 应拒绝
        assert_noop!(
            <CommissionCore as CommissionProvider<u64, u128>>::set_creator_reward_rate(
                ENTITY_ID, 5001,
            ),
            sp_runtime::DispatchError::Other("InvalidRate")
        );
    });
}

// ============================================================================
// Token Platform Fee Rate Governance Tests
// ============================================================================

#[test]
fn set_token_platform_fee_rate_works() {
    new_test_ext().execute_with(|| {
        // 默认值 = 100 bps (1%)
        assert_eq!(TokenPlatformFeeRate::<Test>::get(), 100);

        // Root 设置为 200 bps (2%)
        assert_ok!(CommissionCore::set_token_platform_fee_rate(RuntimeOrigin::root(), 200));
        assert_eq!(TokenPlatformFeeRate::<Test>::get(), 200);

        // 验证事件
        System::assert_last_event(RuntimeEvent::CommissionCore(
            crate::Event::TokenPlatformFeeRateUpdated { old_rate: 100, new_rate: 200 },
        ));
    });
}

#[test]
fn set_token_platform_fee_rate_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_token_platform_fee_rate(RuntimeOrigin::signed(BUYER), 200),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn set_token_platform_fee_rate_rejects_too_high() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_token_platform_fee_rate(RuntimeOrigin::root(), 1001),
            crate::Error::<Test>::TokenPlatformFeeRateTooHigh
        );
        // 边界值 1000 应通过
        assert_ok!(CommissionCore::set_token_platform_fee_rate(RuntimeOrigin::root(), 1000));
        assert_eq!(TokenPlatformFeeRate::<Test>::get(), 1000);
    });
}

#[test]
fn set_token_platform_fee_rate_zero_disables() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_token_platform_fee_rate(RuntimeOrigin::root(), 0));
        assert_eq!(TokenPlatformFeeRate::<Test>::get(), 0);
    });
}

// ============================================================================
// F1: Admin 权限支持 — ensure_owner_or_admin
// ============================================================================

#[test]
fn f1_admin_can_set_commission_modes() {
    new_test_ext().execute_with(|| {
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(ADMIN),
            ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(config.enabled_modes.contains(CommissionModes::DIRECT_REWARD));
    });
}

#[test]
fn f1_admin_can_set_commission_rate() {
    new_test_ext().execute_with(|| {
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID, 5000,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.max_commission_rate, 5000);
    });
}

#[test]
fn f1_admin_can_enable_commission() {
    new_test_ext().execute_with(|| {
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID, true,
        ));
    });
}

#[test]
fn f1_admin_without_permission_rejected() {
    new_test_ext().execute_with(|| {
        // Admin has OTHER permission, not COMMISSION_MANAGE
        set_entity_admin(ENTITY_ID, ADMIN, 0x01);
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(ADMIN), ENTITY_ID, 5000,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn f1_non_owner_non_admin_rejected() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(BUYER), ENTITY_ID, 5000,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn f1_owner_still_works() {
    new_test_ext().execute_with(|| {
        // Owner (SELLER) should still work without being admin
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 8000,
        ));
    });
}

#[test]
fn f1_admin_cannot_withdraw_entity_funds() {
    // Fund withdrawal is Owner-only (not admin)
    new_test_ext().execute_with(|| {
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        fund(entity_account(ENTITY_ID), 100_000);

        assert_noop!(
            CommissionCore::withdraw_entity_funds(
                RuntimeOrigin::signed(ADMIN), ENTITY_ID, 1_000,
            ),
            Error::<Test>::NotEntityOwner
        );
    });
}

// ============================================================================
// F13: NEX 全局最低复购比例 — set_global_min_repurchase_rate
// ============================================================================

#[test]
fn f13_set_global_min_repurchase_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_global_min_repurchase_rate(
            RuntimeOrigin::root(), ENTITY_ID, 3000,
        ));
        assert_eq!(GlobalMinRepurchaseRate::<Test>::get(ENTITY_ID), 3000);
        System::assert_last_event(RuntimeEvent::CommissionCore(
            crate::Event::GlobalMinRepurchaseRateSet { entity_id: ENTITY_ID, rate: 3000 },
        ));
    });
}

#[test]
fn f13_set_global_min_repurchase_rate_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_min_repurchase_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn f13_set_global_min_repurchase_rate_rejects_over_10000() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_min_repurchase_rate(
                RuntimeOrigin::root(), ENTITY_ID, 10001,
            ),
            Error::<Test>::InvalidCommissionRate
        );
    });
}

// ============================================================================
// F2: set_withdrawal_cooldown 独立 extrinsic
// ============================================================================

#[test]
fn f2_set_withdrawal_cooldown_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_withdrawal_cooldown(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 200, 300,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.withdrawal_cooldown, 200);
        assert_eq!(config.token_withdrawal_cooldown, 300);
        System::assert_last_event(RuntimeEvent::CommissionCore(
            crate::Event::WithdrawalCooldownUpdated {
                entity_id: ENTITY_ID,
                nex_cooldown: 200,
                token_cooldown: 300,
            },
        ));
    });
}

#[test]
fn f2_set_withdrawal_cooldown_admin_works() {
    new_test_ext().execute_with(|| {
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        assert_ok!(CommissionCore::set_withdrawal_cooldown(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID, 50, 100,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.withdrawal_cooldown, 50);
        assert_eq!(config.token_withdrawal_cooldown, 100);
    });
}

// ============================================================================
// F3: Token 独立冻结期 — token_withdrawal_cooldown
// ============================================================================

#[test]
fn f3_token_uses_independent_cooldown() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // NEX cooldown = 100, Token cooldown = 50
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 100,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 50,
        });

        // Token 入账在 block 10
        System::set_block_number(10);
        crate::pallet::MemberTokenLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 10u64);

        // block 55: token cooldown 已过 (10 + 50 = 60 > 55)
        System::set_block_number(55);
        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER), ENTITY_ID, Some(1_000), None, None,
            ),
            Error::<Test>::WithdrawalCooldownNotMet
        );

        // block 60: token cooldown 刚好满足
        System::set_block_number(60);
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER), ENTITY_ID, Some(1_000), None, None,
        ));
    });
}

#[test]
fn f3_token_fallback_to_nex_cooldown_when_zero() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // token_withdrawal_cooldown = 0 → 回退到 withdrawal_cooldown = 100
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 10000,
            enabled: true,
            withdrawal_cooldown: 100,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        System::set_block_number(10);
        crate::pallet::MemberTokenLastCredited::<Test>::insert(ENTITY_ID, REFERRER, 10u64);

        // block 100: should still be blocked (10 + 100 = 110 > 100)
        System::set_block_number(100);
        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER), ENTITY_ID, Some(1_000), None, None,
            ),
            Error::<Test>::WithdrawalCooldownNotMet
        );

        // block 110: OK
        System::set_block_number(110);
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER), ENTITY_ID, Some(1_000), None, None,
        ));
    });
}

// ============================================================================
// F14: force_disable_entity_commission — Root 紧急暂停
// ============================================================================

#[test]
fn f14_force_disable_works() {
    new_test_ext().execute_with(|| {
        setup_config(5000);
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(config.enabled);

        assert_ok!(CommissionCore::force_disable_entity_commission(
            RuntimeOrigin::root(), ENTITY_ID,
        ));

        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(!config.enabled);
        System::assert_last_event(RuntimeEvent::CommissionCore(
            crate::Event::CommissionForceDisabled { entity_id: ENTITY_ID },
        ));
    });
}

#[test]
fn f14_force_disable_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::force_disable_entity_commission(
                RuntimeOrigin::signed(SELLER), ENTITY_ID,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn f14_force_disable_creates_config_if_absent() {
    new_test_ext().execute_with(|| {
        // No config exists
        assert!(CommissionConfigs::<Test>::get(ENTITY_ID).is_none());

        assert_ok!(CommissionCore::force_disable_entity_commission(
            RuntimeOrigin::root(), ENTITY_ID,
        ));

        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(!config.enabled);
    });
}

// ============================================================================
// F15: 全局佣金率上限 — GlobalMaxCommissionRate
// ============================================================================

#[test]
fn f15_set_global_max_commission_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_global_max_commission_rate(
            RuntimeOrigin::root(), ENTITY_ID, 5000,
        ));
        assert_eq!(GlobalMaxCommissionRate::<Test>::get(ENTITY_ID), 5000);
        System::assert_last_event(RuntimeEvent::CommissionCore(
            crate::Event::GlobalMaxCommissionRateSet { entity_id: ENTITY_ID, rate: 5000 },
        ));
    });
}

#[test]
fn f15_set_commission_rate_blocked_by_global_max() {
    new_test_ext().execute_with(|| {
        // Root sets global max to 5000
        GlobalMaxCommissionRate::<Test>::insert(ENTITY_ID, 5000u16);

        // Owner tries to set 6000 → blocked
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 6000,
            ),
            Error::<Test>::CommissionRateExceedsGlobalMax
        );

        // 5000 should work
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 5000,
        ));
    });
}

#[test]
fn f15_zero_global_max_means_no_limit() {
    new_test_ext().execute_with(|| {
        // Default is 0 = no limit
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 10000,
        ));
    });
}

#[test]
fn f15_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_max_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 5000,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ============================================================================
// F4: clear_commission_config / clear_withdrawal_config
// ============================================================================

#[test]
fn f4_clear_commission_config_works() {
    new_test_ext().execute_with(|| {
        setup_config(5000);
        assert!(CommissionConfigs::<Test>::get(ENTITY_ID).is_some());

        assert_ok!(CommissionCore::clear_commission_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
        ));
        assert!(CommissionConfigs::<Test>::get(ENTITY_ID).is_none());
        System::assert_last_event(RuntimeEvent::CommissionCore(
            crate::Event::CommissionConfigCleared { entity_id: ENTITY_ID },
        ));
    });
}

#[test]
fn f4_clear_commission_config_rejects_absent() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::clear_commission_config(
                RuntimeOrigin::signed(SELLER), ENTITY_ID,
            ),
            Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn f4_clear_withdrawal_config_works() {
    new_test_ext().execute_with(|| {
        use frame_support::BoundedVec;
        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FullWithdrawal,
            WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            BoundedVec::default(),
            0,
            true,
        ));
        assert!(WithdrawalConfigs::<Test>::get(ENTITY_ID).is_some());

        assert_ok!(CommissionCore::clear_withdrawal_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
        ));
        assert!(WithdrawalConfigs::<Test>::get(ENTITY_ID).is_none());
    });
}

#[test]
fn f4_clear_token_withdrawal_config_works() {
    new_test_ext().execute_with(|| {
        use frame_support::BoundedVec;
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER),
            ENTITY_ID,
            WithdrawalMode::FullWithdrawal,
            WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            BoundedVec::default(),
            0,
            true,
        ));
        assert!(TokenWithdrawalConfigs::<Test>::get(ENTITY_ID).is_some());

        assert_ok!(CommissionCore::clear_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
        ));
        assert!(TokenWithdrawalConfigs::<Test>::get(ENTITY_ID).is_none());
    });
}

#[test]
fn f4_clear_config_admin_works() {
    new_test_ext().execute_with(|| {
        setup_config(5000);
        set_entity_admin(ENTITY_ID, ADMIN, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);

        assert_ok!(CommissionCore::clear_commission_config(
            RuntimeOrigin::signed(ADMIN), ENTITY_ID,
        ));
        assert!(CommissionConfigs::<Test>::get(ENTITY_ID).is_none());
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_set_commission_rate() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 5000,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_clear_commission_config() {
    new_test_ext().execute_with(|| {
        setup_config(5000);
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            CommissionCore::clear_commission_config(
                RuntimeOrigin::signed(SELLER), ENTITY_ID,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== #4 Entity 活跃状态校验 ====================

#[test]
fn f4_entity_inactive_rejects_set_commission_rate() {
    new_test_ext().execute_with(|| {
        set_entity_inactive(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 5000,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f4_entity_inactive_rejects_set_commission_modes() {
    new_test_ext().execute_with(|| {
        setup_config(5000);
        set_entity_inactive(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_commission_modes(
                RuntimeOrigin::signed(SELLER), ENTITY_ID,
                CommissionModes(CommissionModes::DIRECT_REWARD),
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f4_entity_inactive_rejects_enable_commission() {
    new_test_ext().execute_with(|| {
        setup_config(5000);
        set_entity_inactive(ENTITY_ID);
        assert_noop!(
            CommissionCore::enable_commission(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, false,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f4_entity_inactive_rejects_set_withdrawal_config() {
    new_test_ext().execute_with(|| {
        set_entity_inactive(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_withdrawal_config(
                RuntimeOrigin::signed(SELLER), ENTITY_ID,
                WithdrawalMode::FullWithdrawal,
                WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
                BoundedVec::<_, ConstU32<10>>::default(),
                0,
                true,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f4_entity_inactive_rejects_set_creator_reward_rate() {
    new_test_ext().execute_with(|| {
        set_entity_inactive(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_creator_reward_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f4_entity_inactive_rejects_set_withdrawal_cooldown() {
    new_test_ext().execute_with(|| {
        set_entity_inactive(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_withdrawal_cooldown(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 100, 200,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== #8 全局紧急暂停 ====================

#[test]
fn f8_force_global_pause_works() {
    new_test_ext().execute_with(|| {
        assert!(!GlobalCommissionPaused::<Test>::get());
        assert_ok!(CommissionCore::force_global_pause(RuntimeOrigin::root(), true));
        assert!(GlobalCommissionPaused::<Test>::get());

        // 非 Root 无法调用
        assert_noop!(
            CommissionCore::force_global_pause(RuntimeOrigin::signed(SELLER), false),
            sp_runtime::DispatchError::BadOrigin
        );

        // Root 恢复
        assert_ok!(CommissionCore::force_global_pause(RuntimeOrigin::root(), false));
        assert!(!GlobalCommissionPaused::<Test>::get());
    });
}

#[test]
fn f8_global_pause_blocks_process_commission() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        setup_config(10000);

        GlobalCommissionPaused::<Test>::put(true);

        assert_noop!(
            CommissionCore::process_commission(
                ENTITY_ID, SHOP_ID, 9999, &BUYER, 100_000, 100_000, 10_000,
            ),
            Error::<Test>::GlobalCommissionPaused
        );
    });
}

#[test]
fn f8_global_pause_blocks_withdraw_commission() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        // 先产生佣金
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9998, &BUYER, 100_000, 100_000, 10_000,
        ));

        // 暂停后提现失败
        GlobalCommissionPaused::<Test>::put(true);

        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(REFERRER), ENTITY_ID, None, None, None,
            ),
            Error::<Test>::GlobalCommissionPaused
        );
    });
}

// ==================== #7 全局 Token 最大佣金率 ====================

#[test]
fn f7_set_global_max_token_commission_rate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::set_global_max_token_commission_rate(
            RuntimeOrigin::root(), ENTITY_ID, 5000,
        ));
        assert_eq!(GlobalMaxTokenCommissionRate::<Test>::get(ENTITY_ID), 5000);
    });
}

#[test]
fn f7_global_max_token_rate_rejects_invalid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_max_token_commission_rate(
                RuntimeOrigin::root(), ENTITY_ID, 10001,
            ),
            Error::<Test>::InvalidCommissionRate
        );
    });
}

#[test]
fn f7_global_max_token_rate_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_global_max_token_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 5000,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== #5 提现暂停开关 ====================

#[test]
fn f5_pause_withdrawals_works() {
    new_test_ext().execute_with(|| {
        assert!(!WithdrawalPaused::<Test>::get(ENTITY_ID));

        assert_ok!(CommissionCore::pause_withdrawals(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        assert!(WithdrawalPaused::<Test>::get(ENTITY_ID));

        assert_ok!(CommissionCore::pause_withdrawals(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, false,
        ));
        assert!(!WithdrawalPaused::<Test>::get(ENTITY_ID));
    });
}

#[test]
fn f5_pause_withdrawals_blocks_nex_withdraw() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 9997, &BUYER, 100_000, 100_000, 10_000,
        ));

        WithdrawalPaused::<Test>::insert(ENTITY_ID, true);

        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(REFERRER), ENTITY_ID, None, None, None,
            ),
            Error::<Test>::WithdrawalPausedByOwner
        );
    });
}

#[test]
fn f5_pause_withdrawals_blocks_token_withdraw() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        WithdrawalPaused::<Test>::insert(ENTITY_ID, true);

        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER), ENTITY_ID, None, None, None,
            ),
            Error::<Test>::WithdrawalPausedByOwner
        );
    });
}

#[test]
fn f5_pause_withdrawals_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            CommissionCore::pause_withdrawals(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== #2 会员级佣金记录索引 ====================

#[test]
fn f2_member_commission_order_ids_indexed() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        // 产生多个订单的佣金
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7001, &BUYER, 100_000, 100_000, 10_000,
        ));
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7002, &BUYER, 100_000, 100_000, 10_000,
        ));
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7003, &BUYER, 100_000, 100_000, 10_000,
        ));

        let ids = MemberCommissionOrderIds::<Test>::get(ENTITY_ID, REFERRER);
        assert!(ids.contains(&7001));
        assert!(ids.contains(&7002));
        assert!(ids.contains(&7003));
        assert_eq!(ids.len(), 3);
    });
}

#[test]
fn f2_member_commission_order_ids_deduplicates() {
    new_test_ext().execute_with(|| {
        fund(PLATFORM, 1_000_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        // 同一订单多次佣金不重复
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7010, &BUYER, 100_000, 100_000, 10_000,
        ));
        // 再来一个不同的订单
        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7010, &BUYER, 100_000, 100_000, 10_000,
        ));

        let ids = MemberCommissionOrderIds::<Test>::get(ENTITY_ID, REFERRER);
        // order 7010 应只出现一次
        assert_eq!(ids.iter().filter(|&&id| id == 7010).count(), 1);
    });
}

// ==================== #3 提现记录存储 ====================

#[test]
fn f3_nex_withdrawal_history_recorded() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(PLATFORM, 1_000_000);
        fund(ea, 100_000);
        set_entity_referrer(ENTITY_ID, REFERRER);
        setup_config(10000);

        assert_ok!(CommissionCore::process_commission(
            ENTITY_ID, SHOP_ID, 7020, &BUYER, 100_000, 100_000, 10_000,
        ));

        let stats = MemberCommissionStats::<Test>::get(ENTITY_ID, REFERRER);
        assert!(stats.pending > 0, "referrer should have pending commission");

        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER), ENTITY_ID, None, None, None,
        ));

        let history = MemberWithdrawalHistory::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(history.len(), 1);
        assert!(history[0].total_amount > 0);
        assert_eq!(history[0].block_number, 1u64);
    });
}

#[test]
fn f3_token_withdrawal_history_recorded() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        inject_token_pending(ENTITY_ID, REFERRER, 10_000);
        set_token_balance(ENTITY_ID, ea, 50_000);

        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(REFERRER), ENTITY_ID, None, None, None,
        ));

        let history = MemberTokenWithdrawalHistory::<Test>::get(ENTITY_ID, REFERRER);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].total_amount, 10_000);
        assert_eq!(history[0].withdrawn, 10_000);
    });
}

// ==================== #10 历史记录清理机制 ====================

#[test]
fn f10_archive_order_records_works() {
    new_test_ext().execute_with(|| {
        // 手动插入已完结的 NEX 佣金记录
        OrderCommissionRecords::<Test>::mutate(5001u64, |records| {
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 5001,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 1000,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Withdrawn,
                created_at: 1u64,
            });
        });
        OrderTreasuryTransfer::<Test>::insert(5001u64, 500u128);

        assert_ok!(CommissionCore::archive_order_records(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 5001,
        ));

        assert!(OrderCommissionRecords::<Test>::get(5001u64).is_empty());
        assert_eq!(OrderTreasuryTransfer::<Test>::get(5001u64), 0);
    });
}

#[test]
fn f10_archive_rejects_non_finalized_records() {
    new_test_ext().execute_with(|| {
        OrderCommissionRecords::<Test>::mutate(5002u64, |records| {
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 5002,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 1000,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Pending, // 未完结
                created_at: 1u64,
            });
        });

        assert_noop!(
            CommissionCore::archive_order_records(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 5002,
            ),
            Error::<Test>::OrderRecordsNotFinalized
        );
    });
}

#[test]
fn f10_archive_rejects_empty_order() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::archive_order_records(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 9999,
            ),
            Error::<Test>::OrderRecordsNotFound
        );
    });
}

// ==================== #11 Storage 版本与迁移钩子 ====================

#[test]
fn f11_on_runtime_upgrade_bumps_version() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;
        use frame_support::traits::StorageVersion;

        // 模拟旧版本
        StorageVersion::new(0).put::<CommissionCore>();
        let weight = CommissionCore::on_runtime_upgrade();
        assert!(weight.ref_time() > 0);

        // 验证版本已升到 1
        let on_chain = StorageVersion::get::<CommissionCore>();
        assert_eq!(on_chain, StorageVersion::new(1));

        // 再次升级应无操作
        let weight2 = CommissionCore::on_runtime_upgrade();
        assert_eq!(weight2.ref_time(), 0);
    });
}

// ============================================================================
// R5 审计回归测试
// ============================================================================

// ---------- H1-R5: enable_commission(false) 必须触发 POOL_REWARD cooldown ----------

#[test]
fn h1r5_enable_commission_false_sets_pool_reward_disabled_at() {
    new_test_ext().execute_with(|| {
        // 开启 POOL_REWARD + enabled=true
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });
        UnallocatedPool::<Test>::insert(ENTITY_ID, 50_000u128);
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);

        // disable commission at block 10
        System::set_block_number(10);
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, false,
        ));

        // PoolRewardDisabledAt 必须被设置
        assert_eq!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID), Some(10));

        // cooldown 期内不可提取池资金
        System::set_block_number(50);
        // available = 100000 - 50000(pool locked) - 1(min) = 49999
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 49_999,
        ));
        // 池资金仍锁定
        assert_eq!(UnallocatedPool::<Test>::get(ENTITY_ID), 50_000);
    });
}

#[test]
fn h1r5_enable_commission_true_clears_pool_reward_cooldown() {
    new_test_ext().execute_with(|| {
        // 先开启 POOL_REWARD 然后 disable
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        System::set_block_number(10);
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, false,
        ));
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_some());

        // 重新启用 → cooldown 应被清除（因 POOL_REWARD mode 仍存在）
        System::set_block_number(20);
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_none());
    });
}

// ---------- H1-R5: clear_commission_config 必须触发 POOL_REWARD cooldown ----------

#[test]
fn h1r5_clear_commission_config_sets_pool_reward_disabled_at() {
    new_test_ext().execute_with(|| {
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });
        UnallocatedPool::<Test>::insert(ENTITY_ID, 30_000u128);
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);

        System::set_block_number(10);
        assert_ok!(CommissionCore::clear_commission_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
        ));

        // 配置已清除
        assert!(CommissionConfigs::<Test>::get(ENTITY_ID).is_none());
        // cooldown 必须被设置
        assert_eq!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID), Some(10));

        // cooldown 期内池仍锁定
        System::set_block_number(50);
        let balance = Balances::free_balance(ea);
        let available = balance.saturating_sub(30_000).saturating_sub(1); // pool locked + ED
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, available,
        ));
        assert_eq!(UnallocatedPool::<Test>::get(ENTITY_ID), 30_000);
    });
}

#[test]
fn h1r5_clear_config_without_pool_reward_no_cooldown() {
    // 没有 POOL_REWARD 的配置被清除时不应设置 cooldown
    new_test_ext().execute_with(|| {
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        assert_ok!(CommissionCore::clear_commission_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
        ));
        assert!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID).is_none());
    });
}

// ---------- H1-R5: force_disable_entity_commission 必须触发 POOL_REWARD cooldown ----------

#[test]
fn h1r5_force_disable_sets_pool_reward_disabled_at() {
    new_test_ext().execute_with(|| {
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });
        UnallocatedPool::<Test>::insert(ENTITY_ID, 20_000u128);
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);

        System::set_block_number(10);
        assert_ok!(CommissionCore::force_disable_entity_commission(
            RuntimeOrigin::root(), ENTITY_ID,
        ));

        // cooldown 被设置
        assert_eq!(PoolRewardDisabledAt::<Test>::get(ENTITY_ID), Some(10));

        // cooldown 期内池锁定
        System::set_block_number(50);
        let balance = Balances::free_balance(ea);
        let available = balance.saturating_sub(20_000).saturating_sub(1);
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, available,
        ));
        assert_eq!(UnallocatedPool::<Test>::get(ENTITY_ID), 20_000);

        // cooldown 过后池解锁
        System::set_block_number(111); // 10 + 100 + 1
        let balance2 = Balances::free_balance(ea);
        let pool_left = UnallocatedPool::<Test>::get(ENTITY_ID);
        // 池不再锁定，可全部提取
        let max_withdraw = balance2.saturating_sub(1); // only ED
        assert_ok!(CommissionCore::withdraw_entity_funds(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, max_withdraw.min(pool_left),
        ));
    });
}

// ---------- M1-R5: archive_order_records 跨实体越权归档 ----------

#[test]
fn m1r5_archive_order_rejects_cross_entity() {
    new_test_ext().execute_with(|| {
        let other_entity_id = 2u64;
        // 设置 entity 2 的 owner
        set_entity_owner(other_entity_id, 77);

        // 在 entity 1 下创建已完结的订单记录
        let record = pallet_commission_common::CommissionRecord {
            entity_id: ENTITY_ID,
            shop_id: SHOP_ID,
            order_id: 999,
            buyer: BUYER,
            beneficiary: REFERRER,
            amount: 100u128,
            commission_type: CommissionType::DirectReward,
            level: 1,
            status: CommissionStatus::Cancelled,
            created_at: 1u64,
        };
        OrderCommissionRecords::<Test>::mutate(999u64, |records| {
            let _ = records.try_push(record);
        });

        // entity 2 的 owner 尝试归档 entity 1 的订单 → 应失败
        assert_noop!(
            CommissionCore::archive_order_records(
                RuntimeOrigin::signed(77), other_entity_id, 999,
            ),
            Error::<Test>::OrderRecordsNotFound
        );

        // entity 1 的 owner 可以归档
        assert_ok!(CommissionCore::archive_order_records(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 999,
        ));
        assert!(OrderCommissionRecords::<Test>::get(999u64).is_empty());
    });
}

// ---------- M2-R5: process_token_commission 未配置时优雅返回 ----------

#[test]
fn m2r5_process_token_commission_ok_when_not_configured() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 不设置任何 CommissionConfig → 应成功返回而非报错
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 2001, &BUYER, 10_000, 10_000, 500,
        ));

        // token_platform_fee (500) 通过 sweep 被计入 accounted balance
        // 但不会进入 pending/pool/shopping（因无配置）
        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 0);
    });
}

#[test]
fn m2r5_process_token_commission_ok_when_disabled() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 50_000);

        // 配置存在但 enabled=false
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 5000,
            enabled: false,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 2002, &BUYER, 10_000, 10_000, 500,
        ));

        assert_eq!(TokenPendingTotal::<Test>::get(ENTITY_ID), 0);
    });
}

// ============================================================================
// M1-R6: set_commission_modes POOL_REWARD cooldown bypass 回归测试
// ============================================================================

#[test]
fn m1r6_mode_toggle_while_disabled_preserves_cooldown() {
    // 攻击路径: enable_commission(false) → remove POOL_REWARD → add POOL_REWARD back
    // 预期: cooldown 不被清除（pool 仍锁定）
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        fund(PLATFORM, 100_000);
        fund(SELLER, 100_000);

        // 1. 启用佣金 + POOL_REWARD，让池有资金
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });
        UnallocatedPool::<Test>::insert(ENTITY_ID, 10_000u128);

        // 2. 禁用佣金 → cooldown 开始
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, false,
        ));
        assert!(PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID));

        // 3. 移除 POOL_REWARD 模式位 → cooldown 重写（合法）
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ));
        // cooldown 仍存在（被重写为 now）
        assert!(PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID));

        // 4. 在 enabled=false 的状态下重新添加 POOL_REWARD
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
        ));

        // M1-R6 修复: cooldown 不应被清除（enabled 仍为 false）
        assert!(PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID),
            "cooldown should NOT be cleared when adding POOL_REWARD while enabled=false");
    });
}

#[test]
fn m1r6_mode_toggle_while_enabled_clears_cooldown() {
    // 对照测试: enabled=true 时添加 POOL_REWARD 应正常清除 cooldown
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        fund(SELLER, 100_000);

        // 1. 有 POOL_REWARD 且 enabled
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        // 2. 移除 POOL_REWARD → cooldown 设置
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ));
        assert!(PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID));

        // 3. 重新添加 POOL_REWARD（enabled=true）→ cooldown 应清除
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
        ));
        assert!(!PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID),
            "cooldown SHOULD be cleared when adding POOL_REWARD while enabled=true");
    });
}

#[test]
fn m1r6_full_attack_path_pool_stays_locked() {
    // 完整攻击路径验证: 尝试通过 mode toggle 提取 pool 资金应失败
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        fund(ea, 100_000);
        fund(SELLER, 100_000);

        // 1. 设置有 POOL_REWARD 的 Entity，池有资金
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });
        UnallocatedPool::<Test>::insert(ENTITY_ID, 50_000u128);

        // 2. 禁用佣金
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, false,
        ));

        // 3. 尝试 mode toggle bypass
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ));
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
        ));

        // 4. 尝试提取 pool 资金 — 应失败（pool 仍锁定，cooldown 未过期）
        assert_noop!(
            CommissionCore::withdraw_entity_funds(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 50_000,
            ),
            Error::<Test>::InsufficientEntityFunds
        );
    });
}

// ============================================================================
// M2-R6: Token cancel 回退 Pool A 留存 回归测试
// ============================================================================

#[test]
fn m2r6_token_cancel_reverses_pool_a_retention() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);

        // 配置启用佣金 + POOL_REWARD
        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        let order_id = 3001u64;
        let token_platform_fee = 1000u128;

        // process_token_commission: platform_fee (1000) 无 referrer → 全部进 pool_a_retention
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, order_id, &BUYER, 10_000, 10_000, token_platform_fee,
        ));

        // pool_a_retention = 1000 (全部 platform_fee，无 referrer)
        // Pool B remaining (max_commission=5000, no plugins → all 5000 to pool)
        // Total pool = 1000 (Pool A) + 5000 (Pool B) = 6000
        let pool_before = UnallocatedTokenPool::<Test>::get(ENTITY_ID);
        assert_eq!(pool_before, 6000, "pool = pool_a_retention(1000) + pool_b_remaining(5000)");

        // 验证 OrderTokenPlatformRetention 被记录
        let (ret_eid, ret_amount) = OrderTokenPlatformRetention::<Test>::get(order_id);
        assert_eq!(ret_eid, ENTITY_ID);
        assert_eq!(ret_amount, token_platform_fee);

        // cancel → Pool B refund (-5000) + Pool A retention refund (-1000) = 0
        assert_ok!(CommissionCore::do_cancel_token_commission(order_id));

        let pool_after = UnallocatedTokenPool::<Test>::get(ENTITY_ID);
        assert_eq!(pool_after, 0,
            "pool should be 0 after both Pool B and Pool A retention are reversed");

        // OrderTokenPlatformRetention 应被清除
        assert!(!OrderTokenPlatformRetention::<Test>::contains_key(order_id));
    });
}

#[test]
fn m2r6_token_cancel_with_referrer_partial_retention() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);

        // 设置 entity referrer → pool_a 会分出 referrer 份额
        set_entity_referrer(ENTITY_ID, REFERRER);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        let order_id = 3002u64;
        let token_platform_fee = 1000u128;

        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, order_id, &BUYER, 10_000, 10_000, token_platform_fee,
        ));

        // ReferrerShareBps = 5000 (50%), so referrer gets 500, retention = 500
        let (ret_eid, ret_amount) = OrderTokenPlatformRetention::<Test>::get(order_id);
        assert_eq!(ret_eid, ENTITY_ID);
        assert_eq!(ret_amount, 500); // 1000 - 500 referrer

        // Pool = 500 (Pool A retention) + Pool B remaining
        let pool_before = UnallocatedTokenPool::<Test>::get(ENTITY_ID);
        assert!(pool_before >= 500, "pool should include at least pool_a_retention");

        // cancel reverses both Pool B and Pool A retention
        assert_ok!(CommissionCore::do_cancel_token_commission(order_id));

        let pool_after = UnallocatedTokenPool::<Test>::get(ENTITY_ID);
        assert_eq!(pool_after, 0, "pool should be fully reversed after cancel");
        assert!(!OrderTokenPlatformRetention::<Test>::contains_key(order_id));
    });
}

#[test]
fn m2r6_archive_cleans_order_token_platform_retention() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        fund(ea, 100_000);
        fund(PLATFORM, 100_000);
        fund(SELLER, 100_000);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD | CommissionModes::POOL_REWARD),
            max_commission_rate: 5000,
            enabled: true,
            withdrawal_cooldown: 0,
            creator_reward_rate: 0,
            token_withdrawal_cooldown: 0,
        });

        let order_id = 3003u64;

        // Process token commission to populate OrderTokenPlatformRetention
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, order_id, &BUYER, 10_000, 10_000, 1000,
        ));
        assert!(OrderTokenPlatformRetention::<Test>::contains_key(order_id));

        // Cancel to make records finalized (Cancelled)
        assert_ok!(CommissionCore::do_cancel_token_commission(order_id));

        // Insert cancelled NEX + Token records so archive passes validation
        use pallet_commission_common::CommissionRecord;
        OrderCommissionRecords::<Test>::mutate(order_id, |records| {
            let _ = records.try_push(CommissionRecord {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id,
                buyer: BUYER,
                beneficiary: BUYER,
                amount: 100u128,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Cancelled,
                created_at: 1u64,
            });
        });

        use pallet_commission_common::TokenCommissionRecord;
        OrderTokenCommissionRecords::<Test>::mutate(order_id, |records| {
            records.clear();
            let _ = records.try_push(TokenCommissionRecord {
                entity_id: ENTITY_ID,
                order_id,
                buyer: BUYER,
                beneficiary: BUYER,
                amount: 100u128,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Cancelled,
                created_at: 1u64,
            });
        });

        assert_ok!(CommissionCore::archive_order_records(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, order_id,
        ));

        // OrderTokenPlatformRetention should be cleaned
        assert!(!OrderTokenPlatformRetention::<Test>::contains_key(order_id));
    });
}

// ==================== BUG-1: settle_order_commission ====================

#[test]
fn bug1_settle_order_records_transitions_pending_to_withdrawn() {
    new_test_ext().execute_with(|| {
        OrderCommissionRecords::<Test>::mutate(7001u64, |records| {
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 7001,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 1000,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Pending,
                created_at: 1u64,
            });
        });

        assert_ok!(CommissionCore::do_settle_order_records(7001));

        let records = OrderCommissionRecords::<Test>::get(7001u64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, CommissionStatus::Withdrawn);
    });
}

#[test]
fn bug1_settle_preserves_cancelled_records() {
    new_test_ext().execute_with(|| {
        OrderCommissionRecords::<Test>::mutate(7002u64, |records| {
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 7002,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 500,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Cancelled,
                created_at: 1u64,
            });
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 7002,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 300,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Pending,
                created_at: 1u64,
            });
        });

        assert_ok!(CommissionCore::do_settle_order_records(7002));

        let records = OrderCommissionRecords::<Test>::get(7002u64);
        assert_eq!(records[0].status, CommissionStatus::Cancelled);
        assert_eq!(records[1].status, CommissionStatus::Withdrawn);
    });
}

#[test]
fn bug1_settle_then_archive_succeeds() {
    new_test_ext().execute_with(|| {
        OrderCommissionRecords::<Test>::mutate(7003u64, |records| {
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 7003,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 1000,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Pending,
                created_at: 1u64,
            });
        });

        // settle first
        assert_ok!(CommissionCore::do_settle_order_records(7003));
        // then archive should succeed
        assert_ok!(CommissionCore::archive_order_records(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 7003,
        ));
        assert!(OrderCommissionRecords::<Test>::get(7003u64).is_empty());
    });
}

#[test]
fn bug1_settle_token_records_works() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::TokenCommissionRecord;
        OrderTokenCommissionRecords::<Test>::mutate(7004u64, |records| {
            let _ = records.try_push(TokenCommissionRecord {
                entity_id: ENTITY_ID,
                order_id: 7004,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 500u128,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Pending,
                created_at: 1u64,
            });
        });

        assert_ok!(CommissionCore::do_settle_order_records(7004));

        let records = OrderTokenCommissionRecords::<Test>::get(7004u64);
        assert_eq!(records[0].status, CommissionStatus::Withdrawn);
    });
}

#[test]
fn bug1_trait_settle_order_commission_works() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::CommissionProvider;

        OrderCommissionRecords::<Test>::mutate(7005u64, |records| {
            let _ = records.try_push(crate::pallet::CommissionRecordOf::<Test> {
                entity_id: ENTITY_ID,
                shop_id: SHOP_ID,
                order_id: 7005,
                buyer: BUYER,
                beneficiary: REFERRER,
                amount: 1000,
                commission_type: CommissionType::DirectReward,
                level: 0,
                status: CommissionStatus::Pending,
                created_at: 1u64,
            });
        });

        assert_ok!(<CommissionCore as pallet_commission_common::CommissionProvider<u64, u128>>::settle_order_commission(7005));

        let records = OrderCommissionRecords::<Test>::get(7005u64);
        assert_eq!(records[0].status, CommissionStatus::Withdrawn);
    });
}

// ==================== BUG-2: GlobalMaxTokenCommissionRate enforcement ====================

#[test]
fn bug2_set_commission_rate_respects_token_global_max() {
    new_test_ext().execute_with(|| {
        // 设置 Token 全局上限 = 3000
        assert_ok!(CommissionCore::set_global_max_token_commission_rate(
            RuntimeOrigin::root(), ENTITY_ID, 3000,
        ));

        // 设置 rate = 3000 → 成功
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
        ));

        // 设置 rate = 3001 → 失败
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 3001,
            ),
            Error::<Test>::TokenCommissionRateExceedsGlobalMax
        );
    });
}

#[test]
fn bug2_both_global_caps_enforced() {
    new_test_ext().execute_with(|| {
        // NEX 上限 5000, Token 上限 3000 → 实际上限取两者中较小的
        assert_ok!(CommissionCore::set_global_max_commission_rate(
            RuntimeOrigin::root(), ENTITY_ID, 5000,
        ));
        assert_ok!(CommissionCore::set_global_max_token_commission_rate(
            RuntimeOrigin::root(), ENTITY_ID, 3000,
        ));

        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 3000,
        ));
        assert_noop!(
            CommissionCore::set_commission_rate(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 4000,
            ),
            Error::<Test>::TokenCommissionRateExceedsGlobalMax
        );
    });
}

// ==================== BUG-3: force_enable_entity_commission ====================

#[test]
fn bug3_force_enable_after_force_disable() {
    new_test_ext().execute_with(|| {
        // 先配置并启用
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(config.enabled);

        // Root 强制禁用
        assert_ok!(CommissionCore::force_disable_entity_commission(
            RuntimeOrigin::root(), ENTITY_ID,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(!config.enabled);

        // Root 重新启用
        assert_ok!(CommissionCore::force_enable_entity_commission(
            RuntimeOrigin::root(), ENTITY_ID,
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert!(config.enabled);
    });
}

#[test]
fn bug3_force_enable_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::force_enable_entity_commission(
                RuntimeOrigin::signed(SELLER), ENTITY_ID,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn bug3_force_enable_rejects_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::force_enable_entity_commission(
                RuntimeOrigin::root(), 999,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

// ==================== MISSING-2: retry_cancel_commission ====================

#[test]
fn missing2_retry_cancel_commission_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::retry_cancel_commission(
                RuntimeOrigin::signed(SELLER), 1,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn missing2_retry_cancel_is_idempotent() {
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        fund(PLATFORM, 1_000_000);

        // 配置佣金
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
        ));
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ));

        // 取消（无 pending records → 无操作）
        assert_ok!(CommissionCore::retry_cancel_commission(
            RuntimeOrigin::root(), 9999,
        ));
    });
}

// ==================== MISSING-3: min_withdrawal_interval ====================

#[test]
fn missing3_set_min_withdrawal_interval_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(MinWithdrawalInterval::<Test>::get(ENTITY_ID), 0);

        assert_ok!(CommissionCore::set_min_withdrawal_interval(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 50,
        ));

        assert_eq!(MinWithdrawalInterval::<Test>::get(ENTITY_ID), 50);
    });
}

#[test]
fn missing3_set_interval_requires_owner_or_admin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionCore::set_min_withdrawal_interval(
                RuntimeOrigin::signed(BUYER), ENTITY_ID, 50,
            ),
            Error::<Test>::NotEntityOwnerOrAdmin
        );
    });
}

#[test]
fn missing3_set_interval_checks_entity_locked() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            CommissionCore::set_min_withdrawal_interval(
                RuntimeOrigin::signed(SELLER), ENTITY_ID, 50,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn missing3_withdrawal_interval_enforced() {
    new_test_ext().execute_with(|| {
        fund(SELLER, 1_000_000);
        fund(PLATFORM, 1_000_000);
        let entity_acct = entity_account(ENTITY_ID);
        fund(entity_acct, 1_000_000);

        // 配置佣金
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
        ));
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        assert_ok!(CommissionCore::set_withdrawal_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            WithdrawalMode::FullWithdrawal,
            WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            BoundedVec::default(), 0, true,
        ));

        // 给会员记入佣金
        MemberCommissionStats::<Test>::mutate(ENTITY_ID, &BUYER, |stats| {
            stats.pending = 5000;
            stats.total_earned = 5000;
        });
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 5000u128);

        // 设置最小间隔 = 10 区块
        assert_ok!(CommissionCore::set_min_withdrawal_interval(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 10,
        ));

        // 第一次提现 → 成功（无历史提现记录，last_withdrawn = 0）
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(BUYER), ENTITY_ID, Some(1000u128), None, None,
        ));

        // 立即第二次提现 → 失败（间隔不足）
        assert_noop!(
            CommissionCore::withdraw_commission(
                RuntimeOrigin::signed(BUYER), ENTITY_ID, Some(1000u128), None, None,
            ),
            Error::<Test>::WithdrawalIntervalNotMet
        );

        // 推进区块到间隔之后
        System::set_block_number(11);

        // 第三次提现 → 成功
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(BUYER), ENTITY_ID, Some(1000u128), None, None,
        ));
    });
}

#[test]
fn missing3_token_withdrawal_interval_enforced() {
    new_test_ext().execute_with(|| {
        let entity_acct = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, entity_acct, 1_000_000);

        // 配置佣金
        assert_ok!(CommissionCore::set_commission_rate(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 1000,
        ));
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        assert_ok!(CommissionCore::set_token_withdrawal_config(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            WithdrawalMode::FullWithdrawal,
            WithdrawalTierConfig { withdrawal_rate: 10000, repurchase_rate: 0 },
            BoundedVec::default(), 0, true,
        ));

        // 给会员记入 Token 佣金
        MemberTokenCommissionStats::<Test>::mutate(ENTITY_ID, &BUYER, |stats| {
            stats.pending = 5000u128;
            stats.total_earned = 5000u128;
        });
        TokenPendingTotal::<Test>::insert(ENTITY_ID, 5000u128);

        // 设置最小间隔 = 10 区块
        assert_ok!(CommissionCore::set_min_withdrawal_interval(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, 10,
        ));

        // 第一次提现 → 成功
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(BUYER), ENTITY_ID, Some(1000u128), None, None,
        ));

        // 立即第二次提现 → 失败
        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(BUYER), ENTITY_ID, Some(1000u128), None, None,
            ),
            Error::<Test>::WithdrawalIntervalNotMet
        );

        // 推进区块到间隔之后
        System::set_block_number(11);

        // 第三次提现 → 成功
        assert_ok!(CommissionCore::withdraw_token_commission(
            RuntimeOrigin::signed(BUYER), ENTITY_ID, Some(1000u128), None, None,
        ));
    });
}

// ==================== R-11: Double storage read eliminated ====================

#[test]
fn r11_set_commission_modes_pool_reward_tracking_still_works() {
    new_test_ext().execute_with(|| {
        // 启用佣金 + POOL_REWARD
        assert_ok!(CommissionCore::enable_commission(
            RuntimeOrigin::signed(SELLER), ENTITY_ID, true,
        ));
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::POOL_REWARD | CommissionModes::DIRECT_REWARD),
        ));
        assert!(!PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID));

        // 移除 POOL_REWARD → 应记录 disabled_at
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::DIRECT_REWARD),
        ));
        assert!(PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID));

        // 重新添加 POOL_REWARD（commission 已 enabled）→ 应清除 disabled_at
        assert_ok!(CommissionCore::set_commission_modes(
            RuntimeOrigin::signed(SELLER), ENTITY_ID,
            CommissionModes(CommissionModes::POOL_REWARD | CommissionModes::DIRECT_REWARD),
        ));
        assert!(!PoolRewardDisabledAt::<Test>::contains_key(ENTITY_ID));
    });
}
