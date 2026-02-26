use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_ok, assert_noop};
use pallet_commission_common::{CommissionModes, CommissionType};

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
            SHOP_ID,
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
                SHOP_ID,
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
                SHOP_ID,
                5000,
            ),
            Error::<Test>::NotEntityOwner
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
            SHOP_ID, 1, &BUYER, 100_000, 100_000, platform_fee,
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
            SHOP_ID, 2, &BUYER, 100_000, 100_000, platform_fee,
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
            SHOP_ID, 3, &BUYER, 100_000, 100_000, 10_000,
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
            SHOP_ID, 5, &BUYER, 100_000, 100_000, 0,
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
            SHOP_ID, 6, &BUYER, 100_000, 100_000, platform_fee,
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
            SHOP_ID, 7, &BUYER, 100_000, 100_000, 10_000,
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
            SHOP_ID, 20, &BUYER, 100_000, 100_000, platform_fee,
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
            SHOP_ID, 21, &BUYER, 100_000, 100_000, 10_000,
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
            SHOP_ID, 22, &BUYER, 100_000, 100_000, 0,
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
            SHOP_ID, 23, &BUYER, 100_000, 100_000, platform_fee,
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
            SHOP_ID, 8, &BUYER, 100_000, 100_000, 10_000,
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
            SHOP_ID, 30, &BUYER, 100_000, 100_000, 10_000,
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
// init_commission_plan
// ============================================================================

#[test]
fn init_commission_plan_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionCore::init_commission_plan(
            RuntimeOrigin::signed(SELLER),
            SHOP_ID,
            pallet_commission_common::CommissionPlan::DirectOnly { rate: 500 },
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.max_commission_rate, 500);
        assert_eq!(config.enabled, true);
    });
}
