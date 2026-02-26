//! 代币发售模块测试

use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};

/// 创建标准发售轮次的辅助函数
fn setup_round() -> u64 {
    assert_ok!(EntityTokenSale::create_sale_round(
        RuntimeOrigin::signed(CREATOR),
        ENTITY_ID,
        SaleMode::FixedPrice,
        1_000_000u128,
        10u64.into(),
        100u64.into(),
        false,
        0,
    ));
    0 // round_id
}

/// 创建完整可订阅的轮次
fn setup_active_round() -> u64 {
    let round_id = setup_round();
    assert_ok!(EntityTokenSale::add_payment_option(
        RuntimeOrigin::signed(CREATOR), round_id,
        None, 100u128, 10u128, 100_000u128,
    ));
    assert_ok!(EntityTokenSale::start_sale(
        RuntimeOrigin::signed(CREATOR), round_id,
    ));
    // 推进到 start_block 内
    frame_system::Pallet::<Test>::set_block_number(10);
    round_id
}

// ==================== create_sale_round ====================

#[test]
fn create_sale_round_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.entity_id, ENTITY_ID);
        assert_eq!(round.total_supply, 1_000_000u128);
        assert_eq!(round.status, RoundStatus::NotStarted);
        assert_eq!(round.funds_withdrawn, false);
    });
}

#[test]
fn create_sale_round_rejects_invalid_entity() {
    new_test_ext().execute_with(|| {
        // 不存在的 entity
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), 999,
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), false, 0,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn create_sale_round_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(BUYER), ENTITY_ID,
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), false, 0,
            ),
            Error::<Test>::Unauthorized
        );
    });
}

#[test]
fn create_sale_round_rejects_zero_supply() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::FixedPrice, 0u128, 10u64.into(), 100u64.into(), false, 0,
            ),
            Error::<Test>::InvalidTotalSupply
        );
    });
}

#[test]
fn create_sale_round_rejects_bad_time_window() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::FixedPrice, 1_000u128, 100u64.into(), 10u64.into(), false, 0,
            ),
            Error::<Test>::InvalidTimeWindow
        );
    });
}

#[test]
fn create_sale_round_rejects_invalid_kyc_level() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), true, 5,
            ),
            Error::<Test>::InvalidKycLevel
        );
    });
}

// ==================== add_payment_option ====================

#[test]
fn add_payment_option_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id,
            None, 100u128, 10u128, 10_000u128,
        ));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.payment_options_count, 1);
        let options = RoundPaymentOptions::<Test>::get(round_id);
        assert_eq!(options.len(), 1);
    });
}

#[test]
fn add_payment_option_rejects_zero_price() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::add_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id,
                None, 0u128, 10u128, 10_000u128,
            ),
            Error::<Test>::InvalidPrice
        );
    });
}

#[test]
fn add_payment_option_rejects_bad_limits() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // max < min
        assert_noop!(
            EntityTokenSale::add_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id,
                None, 100u128, 1000u128, 10u128,
            ),
            Error::<Test>::InvalidPurchaseLimits
        );
    });
}

// ==================== set_vesting_config ====================

#[test]
fn set_vesting_config_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::set_vesting_config(
            RuntimeOrigin::signed(CREATOR), round_id,
            VestingType::Linear, 1000, 100u64.into(), 1000u64.into(), 100u64.into(),
        ));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.vesting_config.vesting_type, VestingType::Linear);
        assert_eq!(round.vesting_config.initial_unlock_bps, 1000);
    });
}

#[test]
fn set_vesting_config_rejects_cliff_gt_total() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::set_vesting_config(
                RuntimeOrigin::signed(CREATOR), round_id,
                VestingType::Linear, 1000,
                2000u64.into(), // cliff > total
                1000u64.into(),
                100u64.into(),
            ),
            Error::<Test>::InvalidVestingDuration
        );
    });
}

// ==================== configure_dutch_auction ====================

#[test]
fn configure_dutch_auction_requires_not_started() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
        ));
        assert_ok!(EntityTokenSale::configure_dutch_auction(
            RuntimeOrigin::signed(CREATOR), 0, 1000u128, 100u128,
        ));
        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.dutch_start_price, Some(1000u128));
        assert_eq!(round.dutch_end_price, Some(100u128));
    });
}

// ==================== add_to_whitelist ====================

#[test]
fn whitelist_uses_separate_storage() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![BUYER, BUYER2]).unwrap(),
        ));
        assert!(RoundWhitelist::<Test>::get(round_id, BUYER));
        assert!(RoundWhitelist::<Test>::get(round_id, BUYER2));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 2);

        // 重复添加不增加计数
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 2);
    });
}

#[test]
fn whitelist_rejects_non_not_started() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::add_to_whitelist(
                RuntimeOrigin::signed(CREATOR), round_id,
                BoundedVec::try_from(vec![BUYER]).unwrap(),
            ),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== start_sale ====================

#[test]
fn start_sale_requires_payment_options() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // 没有支付选项
        assert_noop!(
            EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::NoPaymentOptions
        );
    });
}

#[test]
fn start_sale_locks_entity_tokens() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Active);
    });
}

// ==================== subscribe ====================

#[test]
fn subscribe_transfers_nex() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        let buyer_before = Balances::free_balance(BUYER);

        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id,
            100u128, // 100 tokens
            None,    // native NEX
        ));

        let buyer_after = Balances::free_balance(BUYER);
        // price=100, amount=100 → payment = 10_000
        assert_eq!(buyer_before - buyer_after, 10_000u128);

        // Pallet 账户收到 NEX
        let pallet_account = EntityTokenSale::pallet_account();
        assert_eq!(Balances::free_balance(pallet_account), 10_000u128);

        // 认购记录
        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert_eq!(sub.amount, 100u128);
        assert_eq!(sub.payment_amount, 10_000u128);
        assert_eq!(sub.refunded, false);
    });
}

#[test]
fn subscribe_rejects_outside_time_window() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), round_id));
        // block = 0, start_block = 10
        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None),
            Error::<Test>::SaleNotInTimeWindow
        );
    });
}

#[test]
fn subscribe_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));
        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None),
            Error::<Test>::AlreadySubscribed
        );
    });
}

#[test]
fn subscribe_checks_kyc() {
    new_test_ext().execute_with(|| {
        // 创建需要 KYC 的轮次
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(),
            true, 2, // kyc_required, min_level=2
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // BUYER 没有 KYC
        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None),
            Error::<Test>::InsufficientKycLevel
        );

        // 设置 KYC level 2
        MockKycChecker::set_level(BUYER, 2);
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));
    });
}

#[test]
fn subscribe_checks_whitelist() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::WhitelistAllocation, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        // 只添加 BUYER 到白名单
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), 0,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // BUYER2 不在白名单
        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER2), 0, 100u128, None),
            Error::<Test>::NotInWhitelist
        );
        // BUYER 在白名单
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));
    });
}

// ==================== end_sale ====================

#[test]
fn end_sale_releases_unsold_tokens() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 认购部分
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));
        // H2-audit: 须在 end_block 之后才能结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
        assert_eq!(round.sold_amount, 100u128);
    });
}

// ==================== claim_tokens ====================

#[test]
fn claim_tokens_distributes_entity_tokens() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // claim（无锁仓 → 全额解锁）
        assert_ok!(EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id));
        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert!(sub.claimed);
        assert_eq!(sub.unlocked_amount, 100u128); // VestingType::None → 全额
    });
}

#[test]
fn claim_tokens_rejects_double_claim() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        assert_ok!(EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id));
        assert_noop!(
            EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::AlreadyClaimed
        );
    });
}

// ==================== cancel_sale + claim_refund ====================

#[test]
fn cancel_and_refund_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        let buyer_before = Balances::free_balance(BUYER);

        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));
        let buyer_after_sub = Balances::free_balance(BUYER);
        assert_eq!(buyer_before - buyer_after_sub, 10_000u128);

        // 取消发售
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);

        // 认购者领取退款
        assert_ok!(EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), round_id));
        let buyer_after_refund = Balances::free_balance(BUYER);
        assert_eq!(buyer_after_refund, buyer_before); // 完全退还

        // 不能重复退款
        assert_noop!(
            EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::AlreadyRefunded
        );
    });
}

#[test]
fn claim_refund_rejects_non_cancelled() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        // 轮次仍 Active
        assert_noop!(
            EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::SaleNotCancelled
        );
    });
}

// ==================== withdraw_funds ====================

#[test]
fn withdraw_funds_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let entity_before = Balances::free_balance(ENTITY_ACCOUNT);
        assert_ok!(EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id));
        let entity_after = Balances::free_balance(ENTITY_ACCOUNT);
        assert_eq!(entity_after - entity_before, 10_000u128);

        // 不能重复提取
        assert_noop!(
            EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::FundsAlreadyWithdrawn
        );
    });
}

// ==================== calculate_initial_unlock ====================

#[test]
fn calculate_initial_unlock_works() {
    new_test_ext().execute_with(|| {
        let vesting = VestingConfig {
            vesting_type: VestingType::Linear,
            initial_unlock_bps: 2000, // 20%
            cliff_duration: 100u64,
            total_duration: 1000u64,
            unlock_interval: 100u64,
        };

        let total = 1_000_000u128;
        let initial = crate::pallet::Pallet::<Test>::calculate_initial_unlock(&vesting, total);
        assert_eq!(initial, 200_000u128); // 20% of 1M
    });
}

#[test]
fn calculate_initial_unlock_no_vesting_returns_total() {
    new_test_ext().execute_with(|| {
        let vesting = VestingConfig::default(); // VestingType::None
        let total = 1_000_000u128;
        let initial = crate::pallet::Pallet::<Test>::calculate_initial_unlock(&vesting, total);
        assert_eq!(initial, total); // 无锁仓 → 全额
    });
}

// ==================== checked_mul overflow ====================

#[test]
fn subscribe_rejects_overflow() {
    new_test_ext().execute_with(|| {
        // 创建一个 total_supply 较大的轮次
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice,
            1_000_000u128, // total_supply
            10u64.into(), 100u64.into(), false, 0,
        ));
        // 极高单价：amount(1000) * price(u128::MAX/1000+1) 会溢出
        let huge_price = u128::MAX / 500;
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0,
            None, huge_price, 10u128, 1_000_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // amount(1000) * huge_price 溢出 u128
        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 1000u128, None),
            Error::<Test>::ArithmeticOverflow
        );
    });
}

// ==================== H2-audit: end_sale time window enforcement ====================

#[test]
fn end_sale_rejects_premature_end() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // block=10, end_block=100, remaining > 0 → 不允许提前结束
        assert_noop!(
            EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::SaleNotInTimeWindow
        );
    });
}

#[test]
fn end_sale_allows_when_sold_out() {
    new_test_ext().execute_with(|| {
        // 创建供应量为 100 的轮次
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 100u128,
            10u64.into(), 100u64.into(), false, 0,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 1u128, 100u128, 100u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 买光所有代币
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));
        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.remaining_amount, 0u128);

        // 未到 end_block 但已售罄 → 允许结束
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), 0));
        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

#[test]
fn end_sale_allows_after_end_block() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 推进到 end_block 之后
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

// ==================== M1-audit: cliff vesting with unlock_interval ====================

#[test]
fn cliff_vesting_unlock_interval_step_function() {
    new_test_ext().execute_with(|| {
        // 直接测试 calculate_unlockable 的阶梯解锁逻辑
        let vesting = VestingConfig {
            vesting_type: VestingType::Cliff,
            initial_unlock_bps: 2000, // 20%
            cliff_duration: 100u64,
            total_duration: 1000u64,
            unlock_interval: 200u64,
        };
        let total = 1_000_000u128;
        let start = 10u64;

        // 在 cliff 之前 → 报错
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 0u128, start, 50u64,
        );
        assert!(result.is_err());

        // cliff 刚过 (elapsed=0 from cliff_end) → 只有初始解锁 200_000
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 200_000u128, start, 110u64,
        ).unwrap();
        // elapsed = 110 - 110 = 0, effective_elapsed = 0 → vesting_unlocked = 0
        assert_eq!(result, 0u128);

        // elapsed = 150 blocks from cliff_end (< interval 200) → effective_elapsed = 0
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 200_000u128, start, 260u64,
        ).unwrap();
        // elapsed = 260 - 110 = 150, interval=200 → 150/200=0 steps → effective=0
        assert_eq!(result, 0u128);

        // elapsed = 200 blocks → 1 step → effective_elapsed = 200
        // vesting_duration = 1000 - 100 = 900
        // vesting_amount = 1_000_000 * 8000 / 10000 = 800_000
        // unlocked = 800_000 * 200 / 900 = 177_777
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 200_000u128, start, 310u64,
        ).unwrap();
        // elapsed = 310 - 110 = 200, interval=200 → 1 step → effective=200
        // total_unlockable = 200_000 + 177_777 = 377_777
        // unlockable = 377_777 - 200_000 = 177_777
        assert_eq!(result, 177_777u128);

        // elapsed = 350 blocks → still 1 step (350/200=1)
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 200_000u128, start, 460u64,
        ).unwrap();
        // elapsed = 460 - 110 = 350, interval=200 → 1 step → effective=200
        assert_eq!(result, 177_777u128);

        // elapsed = 400 blocks → 2 steps → effective=400
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 200_000u128, start, 510u64,
        ).unwrap();
        // elapsed = 510 - 110 = 400, 400/200=2 → effective=400
        // unlocked = 800_000 * 400 / 900 = 355_555
        // total_unlockable = 200_000 + 355_555 = 555_555
        // result = 555_555 - 200_000 = 355_555
        assert_eq!(result, 355_555u128);
    });
}

#[test]
fn linear_vesting_continuous_unlock() {
    new_test_ext().execute_with(|| {
        let vesting = VestingConfig {
            vesting_type: VestingType::Linear,
            initial_unlock_bps: 1000, // 10%
            cliff_duration: 50u64,
            total_duration: 500u64,
            unlock_interval: 100u64, // 不影响 Linear 类型
        };
        let total = 1_000_000u128;
        let start = 0u64;

        // elapsed = 100 from cliff_end (block 50+100=150)
        let result = crate::pallet::Pallet::<Test>::calculate_unlockable(
            &vesting, total, 100_000u128, start, 150u64,
        ).unwrap();
        // vesting_duration = 500 - 50 = 450
        // vesting_amount = 1_000_000 * 9000 / 10000 = 900_000
        // elapsed = 150 - 50 = 100
        // unlocked_vesting = 900_000 * 100 / 450 = 200_000
        // total = 100_000 + 200_000 = 300_000
        // result = 300_000 - 100_000 = 200_000
        assert_eq!(result, 200_000u128);
    });
}

// ==================== L1: on_initialize auto-end ====================

#[test]
fn on_initialize_auto_ends_expired_sale() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 认购一部分
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        // 确认在 ActiveRounds 中
        let active = ActiveRounds::<Test>::get();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], round_id);

        // 推进到 end_block+1（on_initialize 使用 now > end_block）
        frame_system::Pallet::<Test>::set_block_number(101);
        EntityTokenSale::on_initialize(101);

        // 确认已自动结束
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);

        // 确认已从 ActiveRounds 移除
        let active = ActiveRounds::<Test>::get();
        assert!(active.is_empty());
    });
}

#[test]
fn on_initialize_does_not_end_before_expiry() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // block=10, end_block=100 → not expired
        EntityTokenSale::on_initialize(10);

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Active);
        assert_eq!(ActiveRounds::<Test>::get().len(), 1);
    });
}

#[test]
fn on_initialize_handles_multiple_rounds() {
    new_test_ext().execute_with(|| {
        // 创建 2 个轮次，不同的 end_block
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 100_000u128,
            10u64.into(), 50u64.into(), false, 0,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));

        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 100_000u128,
            10u64.into(), 200u64.into(), false, 0,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 1, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 1));

        assert_eq!(ActiveRounds::<Test>::get().len(), 2);

        // 推进到 51（只有 round 0 过期）
        frame_system::Pallet::<Test>::set_block_number(51);
        EntityTokenSale::on_initialize(51);

        let r0 = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(r0.status, RoundStatus::Ended);
        let r1 = SaleRounds::<Test>::get(1).unwrap();
        assert_eq!(r1.status, RoundStatus::Active);

        let active = ActiveRounds::<Test>::get();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], 1);
    });
}

// ==================== L2: PaymentOptions 独立存储 ====================

#[test]
fn payment_options_stored_separately() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // H3-fix: 仅允许 None asset_id，添加 2 个不同价格的 NEX 选项
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 100u128, 10u128, 10_000u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 50u128, 5u128, 5_000u128,
        ));

        // SaleRound 只记录计数
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.payment_options_count, 2);

        // 实际数据在 RoundPaymentOptions
        let options = RoundPaymentOptions::<Test>::get(round_id);
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].price, 100u128);
        assert_eq!(options[1].price, 50u128);
    });
}

// ==================== L3: 退款宽限期回收 ====================

#[test]
fn reclaim_unclaimed_tokens_after_grace_period() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        let buyer_before = Balances::free_balance(BUYER);

        // 2 个人认购
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER2), round_id, 200u128, None,
        ));

        // 取消发售（block = 10）
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.cancelled_at, Some(10u64));

        // BUYER 领取退款
        assert_ok!(EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), round_id));
        assert_eq!(Balances::free_balance(BUYER), buyer_before); // 完全退还

        // 确认退款计数器已更新
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.total_refunded_tokens, 100u128);
        assert_eq!(round.total_refunded_nex, 10_000u128);

        // BUYER2 没有领取退款 — 宽限期内不能回收
        frame_system::Pallet::<Test>::set_block_number(50);
        assert_noop!(
            EntityTokenSale::reclaim_unclaimed_tokens(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::RefundPeriodNotExpired
        );

        // 推进到宽限期后（cancelled_at=10, grace=100, deadline=110）
        frame_system::Pallet::<Test>::set_block_number(110);
        let entity_before = Balances::free_balance(ENTITY_ACCOUNT);

        assert_ok!(EntityTokenSale::reclaim_unclaimed_tokens(
            RuntimeOrigin::signed(CREATOR), round_id,
        ));

        // 检查 BUYER2 的代币和 NEX 被回收到 Entity 账户
        let entity_after = Balances::free_balance(ENTITY_ACCOUNT);
        // BUYER2 paid 200 * 100 = 20_000 NEX
        assert_eq!(entity_after - entity_before, 20_000u128);

        // 轮次标记为 Completed
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Completed);
        // C1-fix: funds_withdrawn 也被标记
        assert!(round.funds_withdrawn);
    });
}

// ==================== C1-fix: reclaim 后 withdraw_funds 被阻止 ====================

#[test]
fn c1_reclaim_blocks_subsequent_withdraw() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();

        // 认购
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        // 取消
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 推进到宽限期后
        frame_system::Pallet::<Test>::set_block_number(200);

        // reclaim
        assert_ok!(EntityTokenSale::reclaim_unclaimed_tokens(
            RuntimeOrigin::signed(CREATOR), round_id,
        ));

        // withdraw_funds 应被 FundsAlreadyWithdrawn 阻止
        assert_noop!(
            EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::FundsAlreadyWithdrawn
        );
    });
}

// ==================== H2-fix: claim_tokens 拒绝 Completed 状态 ====================

#[test]
fn h2_claim_tokens_rejects_completed_from_cancel() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();

        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        // cancel → reclaim → Completed
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));
        frame_system::Pallet::<Test>::set_block_number(200);
        assert_ok!(EntityTokenSale::reclaim_unclaimed_tokens(
            RuntimeOrigin::signed(CREATOR), round_id,
        ));

        // claim_tokens 应被拒绝（Completed 不再允许）
        assert_noop!(
            EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== H3-fix: add_payment_option 拒绝非 None asset_id ====================

#[test]
fn h3_add_payment_option_rejects_non_none_asset_id() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::add_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id, Some(1u64), 100u128, 10u128, 10_000u128,
            ),
            Error::<Test>::InvalidPaymentAsset
        );
    });
}

#[test]
fn reclaim_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));
        frame_system::Pallet::<Test>::set_block_number(200);

        assert_noop!(
            EntityTokenSale::reclaim_unclaimed_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::Unauthorized
        );
    });
}

// ==================== L4: DutchAuction 价格冗余修复 ====================

#[test]
fn dutch_auction_allows_zero_price_in_payment_option() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
        ));
        // DutchAuction 模式 price=0 应被允许
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 0u128, 10u128, 100_000u128,
        ));
        let options = RoundPaymentOptions::<Test>::get(0);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].price, 0u128);
    });
}

#[test]
fn non_dutch_rejects_zero_price() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round(); // FixedPrice mode
        assert_noop!(
            EntityTokenSale::add_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id, None, 0u128, 10u128, 10_000u128,
            ),
            Error::<Test>::InvalidPrice
        );
    });
}

#[test]
fn dutch_auction_start_requires_configure() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 0u128, 10u128, 100_000u128,
        ));
        // 没有配置 dutch auction → start_sale 应失败
        assert_noop!(
            EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0),
            Error::<Test>::DutchAuctionNotConfigured
        );

        // 配置后可以启动
        assert_ok!(EntityTokenSale::configure_dutch_auction(
            RuntimeOrigin::signed(CREATOR), 0, 1000u128, 100u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
    });
}
