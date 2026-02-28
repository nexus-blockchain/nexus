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
                false,
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
            false,
        ));
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
            ENTITY_ID,
            pallet_commission_common::CommissionPlan::DirectOnly { rate: 500 },
        ));
        let config = CommissionConfigs::<Test>::get(ENTITY_ID).unwrap();
        assert_eq!(config.max_commission_rate, 500);
        assert_eq!(config.enabled, true);
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
            SHOP_ID, 100, &BUYER, 100_000, 100_000, 10_000,
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
            false,
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
            SHOP_ID, 101, &BUYER, 100_000, 100_000, 10_000,
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
            SHOP_ID, 200, &BUYER, 100_000, 100_000, 10_000,
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
            false,
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
            SHOP_ID, 300, &BUYER, 100_000, 100_000, 10_000,
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
            false,
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
            SHOP_ID, 400, &BUYER, 100_000, 100_000, 20_000,
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
            false,
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
            SHOP_ID, 500, &BUYER, 100_000, 100_000, 20_000,
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
            false,
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
fn h3_self_withdraw_not_checked_by_participation_guard() {
    // target=self 时不经过 ParticipationGuard 检查（只在 target != who 分支中检查）
    new_test_ext().execute_with(|| {
        fund(entity_account(ENTITY_ID), 100_000);
        MemberCommissionStats::<Test>::mutate(ENTITY_ID, &REFERRER, |s| {
            s.pending = 10_000;
        });
        ShopPendingTotal::<Test>::insert(ENTITY_ID, 10_000u128);

        // 标记 REFERRER 自己不满足参与要求
        block_participation(ENTITY_ID, REFERRER);

        // target=self（None）→ 不进入 target != who 分支 → 不检查 ParticipationGuard
        assert_ok!(CommissionCore::withdraw_commission(
            RuntimeOrigin::signed(REFERRER),
            ENTITY_ID,
            Some(5_000),
            None,
            None, // target = self
        ));
    });
}
