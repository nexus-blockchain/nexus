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

        // 新版 withdraw_token_commission 先做偿付能力检查，5000 < 10000 → InsufficientTokenCommission
        assert_noop!(
            CommissionCore::withdraw_token_commission(
                RuntimeOrigin::signed(REFERRER),
                ENTITY_ID,
                None,
                None,
                None,
            ),
            Error::<Test>::InsufficientTokenCommission
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
            ENTITY_ID, SHOP_ID, 1001, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 1002, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 1003, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 1004, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 1005, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 1006, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 2001, &BUYER, 20_000, 0,
        ));
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 2002, &BUYER, 30_000, 0,
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
fn e2e_token_commission_not_configured_rejected() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);
        // 不配置 CommissionConfigs

        assert_noop!(
            CommissionCore::process_token_commission(
                ENTITY_ID, SHOP_ID, 4001, &BUYER, 20_000, 0,
            ),
            Error::<Test>::CommissionNotConfigured
        );
    });
}

#[test]
fn e2e_token_commission_disabled_rejected() {
    new_test_ext().execute_with(|| {
        let ea = entity_account(ENTITY_ID);
        set_token_balance(ENTITY_ID, ea, 100_000);

        CommissionConfigs::<Test>::insert(ENTITY_ID, CoreCommissionConfig {
            enabled_modes: CommissionModes(CommissionModes::DIRECT_REWARD),
            max_commission_rate: 5000,
            enabled: false, // 已禁用
            withdrawal_cooldown: 0,
        });

        assert_noop!(
            CommissionCore::process_token_commission(
                ENTITY_ID, SHOP_ID, 4002, &BUYER, 20_000, 0,
            ),
            Error::<Test>::CommissionNotConfigured
        );
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
            ENTITY_ID, SHOP_ID, 5001, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 5002, &BUYER, 20_000, 0,
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
            Error::<Test>::NotEntityOwner
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
            ENTITY_ID, SHOP_ID, 6001, &BUYER, 20_000, 1_000,
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
            ENTITY_ID, SHOP_ID, 6002, &BUYER, 20_000, 1_000,
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
            ENTITY_ID, SHOP_ID, 6003, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 7001, &BUYER, 20_000, 0,
        ));
        // 插件为空 → 10000 全部入沉淀池
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // Order 2: amount=20000, max_commission=10000
        // H1 修复后: available = 15000 - 10000(committed) = 5000 → remaining = min(10000, 5000) = 5000
        // 修复前: available = 15000 → remaining = min(10000, 15000) = 10000 (超额承诺!)
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7002, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 7003, &BUYER, 20_000, 0,
        ));
        assert_eq!(UnallocatedTokenPool::<Test>::get(ENTITY_ID), 10_000);

        // Order 2: available = 10000 - 10000 = 0 → remaining = 0
        assert_ok!(CommissionCore::process_token_commission(
            ENTITY_ID, SHOP_ID, 7004, &BUYER, 20_000, 0,
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
            ENTITY_ID, SHOP_ID, 7005, &BUYER, 40_000, 0,
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

