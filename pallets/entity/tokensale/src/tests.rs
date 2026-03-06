//! 代币发售模块测试

use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok, traits::{Currency, Hooks}, BoundedVec};

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
        0u128, // soft_cap
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
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), false, 0, 0u128,
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
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), false, 0, 0u128,
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
                SaleMode::FixedPrice, 0u128, 10u64.into(), 100u64.into(), false, 0, 0u128,
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
                SaleMode::FixedPrice, 1_000u128, 100u64.into(), 10u64.into(), false, 0, 0u128,
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
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), true, 5, 0u128,
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
            10u64.into(), 100u64.into(), false, 0, 0u128,
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
            BoundedVec::try_from(vec![(BUYER, None), (BUYER2, None)]).unwrap(),
        ));
        assert!(RoundWhitelist::<Test>::contains_key(round_id, BUYER));
        assert!(RoundWhitelist::<Test>::contains_key(round_id, BUYER2));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 2);

        // 重复添加不增加计数
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![(BUYER, None)]).unwrap(),
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
                BoundedVec::try_from(vec![(BUYER, None)]).unwrap(),
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
            true, 2, 0u128, // kyc_required, min_level=2, soft_cap
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
            10u64.into(), 100u64.into(), false, 0, 0u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        // 只添加 BUYER 到白名单
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), 0,
            BoundedVec::try_from(vec![(BUYER, None)]).unwrap(),
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
            10u64.into(), 100u64.into(), false, 0, 0u128,
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
            10u64.into(), 100u64.into(), false, 0, 0u128,
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
            10u64.into(), 50u64.into(), false, 0, 0u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));

        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 100_000u128,
            10u64.into(), 200u64.into(), false, 0, 0u128,
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
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 100u128, 10u128, 10_000u128,
        ));

        // SaleRound 只记录计数
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.payment_options_count, 1);

        // 实际数据在 RoundPaymentOptions
        let options = RoundPaymentOptions::<Test>::get(round_id);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].price, 100u128);
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
            10u64.into(), 100u64.into(), false, 0, 0u128,
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
            10u64.into(), 100u64.into(), false, 0, 0u128,
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

// ==================== M2-audit: unlock_tokens 拒绝非 Ended 状态 ====================

#[test]
fn m2_unlock_tokens_rejects_non_ended_status() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();

        // 认购
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        // 推进到结束后
        frame_system::Pallet::<Test>::set_block_number(101);
        // 结束发售
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // claim_tokens（Ended 状态）
        assert_ok!(EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id));

        // unlock 在 Ended 状态下应成功（设置无锁仓，全部可解锁）
        // 由于默认 VestingType::None, claim_tokens 已解锁全部，所以 unlock 会返回 NoTokensToUnlock
        assert_noop!(
            EntityTokenSale::unlock_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::NoTokensToUnlock
        );

        // 验证状态为 Ended（不是其他状态）
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

#[test]
fn m2_unlock_tokens_rejects_cancelled_status() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();

        // 认购
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        // 取消发售
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // unlock_tokens 在 Cancelled 状态下应被拒绝
        assert_noop!(
            EntityTokenSale::unlock_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== L3-audit: create_sale_round 拒绝过去的 start_block ====================

#[test]
fn l3_create_sale_round_rejects_start_block_in_past() {
    new_test_ext().execute_with(|| {
        // 推进到 block 50
        frame_system::Pallet::<Test>::set_block_number(50);

        // start_block=10 < now=50 应被拒绝
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::FixedPrice, 1_000_000u128,
                10u64.into(), 100u64.into(), false, 0, 0u128,
            ),
            Error::<Test>::StartBlockInPast
        );

        // start_block=50 == now=50 应通过
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            50u64.into(), 100u64.into(), false, 0, 0u128,
        ));
    });
}

// ==================== L5-audit: configure_dutch_auction 拒绝 end_price=0 ====================

#[test]
fn l5_dutch_auction_rejects_zero_end_price() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0, 0u128,
        ));

        // end_price=0 应被拒绝
        assert_noop!(
            EntityTokenSale::configure_dutch_auction(
                RuntimeOrigin::signed(CREATOR), 0, 1000u128, 0u128,
            ),
            Error::<Test>::InvalidDutchAuctionConfig
        );

        // end_price=1 应通过
        assert_ok!(EntityTokenSale::configure_dutch_auction(
            RuntimeOrigin::signed(CREATOR), 0, 1000u128, 1u128,
        ));
    });
}

// ==================== Audit Round 3 回归测试 ====================

// M1: end_sale 后 remaining_amount 应归零
#[test]
fn m1_end_sale_zeros_remaining_amount() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 认购部分代币
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        // 推进到 end_block 之后并手动结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
        // M1-fix: remaining_amount 应为 0（修复前保留原始值 999_900）
        assert_eq!(round.remaining_amount, 0u128);
        assert_eq!(round.sold_amount, 100u128);
    });
}

// M1: on_initialize 自动结束后 remaining_amount 应归零
#[test]
fn m1_auto_end_sale_zeros_remaining_amount() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 认购部分代币
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 500u128, None));

        // 推进到 end_block+1，触发 on_initialize 自动结束
        frame_system::Pallet::<Test>::set_block_number(101);
        <EntityTokenSale as Hooks<u64>>::on_initialize(101);

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
        // M1-fix: remaining_amount 应为 0
        assert_eq!(round.remaining_amount, 0u128);
        assert_eq!(round.sold_amount, 500u128);
    });
}

// ==================== L2-audit: NextRoundId overflow ====================

#[test]
fn l2_next_round_id_overflow_detected() {
    new_test_ext().execute_with(|| {
        // 手动将 NextRoundId 设置为 u64::MAX
        NextRoundId::<Test>::put(u64::MAX);

        // 创建轮次时应检测到溢出
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::FixedPrice, 1_000_000u128,
                10u64.into(), 100u64.into(), false, 0, 0u128,
            ),
            Error::<Test>::RoundIdOverflow
        );
    });
}

// ==================== Deep Audit Round 4 回归测试 ====================

// H1-deep: claim_tokens 检测 repatriate_reserved 不完整转移
#[test]
fn h1_deep_claim_tokens_rejects_partial_repatriation() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();

        // 认购 100 个代币
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        // 推进到 end_block 之后并结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 篡改 reserved：将 entity_account 的 reserved 降低到只剩 50（正常应有 100）
        MockTokenProvider::set_reserved(ENTITY_ID, ENTITY_ACCOUNT, 50);

        // claim_tokens 应失败：repatriate_reserved 只能转移 50，但请求 100
        assert_noop!(
            EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::IncompleteUnreserve
        );
    });
}

// H1-deep: unlock_tokens 检测 repatriate_reserved 不完整转移
#[test]
fn h1_deep_unlock_tokens_rejects_partial_repatriation() {
    new_test_ext().execute_with(|| {
        // 创建带锁仓的轮次
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id,
            None, 100u128, 10u128, 100_000u128,
        ));

        // 设置锁仓：50% 初始解锁，50% 线性解锁，总时长 100 块
        assert_ok!(EntityTokenSale::set_vesting_config(
            RuntimeOrigin::signed(CREATOR), round_id,
            VestingType::Linear, 5000u16, 0u64.into(), 100u64.into(), 0u64.into(),
        ));

        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), round_id));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 认购 1000 个代币
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 1000u128, None,
        ));

        // 推进到 end_block 之后并结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // claim_tokens 正常（初始解锁 50% = 500）
        assert_ok!(EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id));

        // 推进到锁仓期结束（subscribed_at=10, total_duration=100 → 需要 block 110+）
        frame_system::Pallet::<Test>::set_block_number(111);

        // 篡改 reserved：将 entity_account 的 reserved 降低到 0（正常应有剩余 500）
        MockTokenProvider::set_reserved(ENTITY_ID, ENTITY_ACCOUNT, 0);

        // unlock_tokens 应失败：repatriate_reserved 返回 0，但请求 500
        assert_noop!(
            EntityTokenSale::unlock_tokens(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::IncompleteUnreserve
        );
    });
}

// H1-deep: 正常情况下 claim_tokens + unlock_tokens 全额转移应成功
#[test]
fn h1_deep_claim_and_unlock_full_amount_succeeds() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id,
            None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::set_vesting_config(
            RuntimeOrigin::signed(CREATOR), round_id,
            VestingType::Linear, 5000u16, 0u64.into(), 100u64.into(), 0u64.into(),
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), round_id));
        frame_system::Pallet::<Test>::set_block_number(10);

        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 1000u128, None,
        ));

        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // claim 正常
        assert_ok!(EntityTokenSale::claim_tokens(RuntimeOrigin::signed(BUYER), round_id));
        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert_eq!(sub.unlocked_amount, 500); // 50% initial unlock

        // unlock 全部剩余
        frame_system::Pallet::<Test>::set_block_number(111);
        assert_ok!(EntityTokenSale::unlock_tokens(RuntimeOrigin::signed(BUYER), round_id));
        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert_eq!(sub.unlocked_amount, 1000); // 全部解锁
    });
}

// M1-deep: cancel_sale 后 remaining_amount 归零
#[test]
fn m1_deep_cancel_sale_zeros_remaining_amount() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();

        // 认购部分代币
        assert_ok!(EntityTokenSale::subscribe(
            RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
        ));

        let round_before = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round_before.remaining_amount, 999_900u128); // 1_000_000 - 100

        // 取消发售
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let round_after = SaleRounds::<Test>::get(round_id).unwrap();
        // M1-deep: remaining_amount 应归零（修复前保留 999_900）
        assert_eq!(round_after.remaining_amount, 0u128);
        assert_eq!(round_after.status, RoundStatus::Cancelled);
    });
}

// M1-deep: cancel_sale NotStarted 状态不修改 remaining_amount（未 reserve）
#[test]
fn m1_deep_cancel_not_started_keeps_remaining() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // NotStarted 状态取消
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        // NotStarted 不触发 unreserve，remaining_amount 保持原值
        assert_eq!(round.remaining_amount, 1_000_000u128);
        assert_eq!(round.status, RoundStatus::Cancelled);
    });
}

// M2-deep: DutchAuction 价格不低于 end_price（极端 price_range 溢出场景）
#[test]
fn m2_deep_dutch_price_clamped_to_end_price() {
    new_test_ext().execute_with(|| {
        // 创建 DutchAuction 轮次，使用极端价格
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0, 0u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 0u128, 10u128, 100_000u128,
        ));
        // 使用极大 start_price 使 price_range * elapsed 溢出 u128
        let start_price: u128 = u128::MAX / 2;
        let end_price: u128 = 1000;
        assert_ok!(EntityTokenSale::configure_dutch_auction(
            RuntimeOrigin::signed(CREATOR), 0, start_price, end_price,
        ));

        // 查询中间时刻的价格
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(55); // 中间时刻

        let price = EntityTokenSale::get_current_price(0, None).unwrap();
        // M2-deep: 价格应 >= end_price（修复前 saturating_mul 溢出可能导致 price < end_price 甚至 0）
        assert!(price >= end_price, "Dutch price {} should be >= end_price {}", price, end_price);
    });
}

// ==================== M1-R5: add_payment_option 拒绝重复 asset_id ====================

#[test]
fn m1_r5_add_payment_option_rejects_duplicate_asset_id() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // 第一次添加 None 选项应成功
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 100u128, 10u128, 10_000u128,
        ));
        // 第二次添加相同 asset_id (None) 应被拒绝
        assert_noop!(
            EntityTokenSale::add_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id, None, 50u128, 5u128, 5_000u128,
            ),
            Error::<Test>::DuplicatePaymentOption
        );
        // 确认只有 1 个选项
        let options = RoundPaymentOptions::<Test>::get(round_id);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].price, 100u128);
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.payment_options_count, 1);
    });
}

// M2-deep: 正常 DutchAuction 价格递减到 end_price
#[test]
fn m2_deep_dutch_price_reaches_end_price_at_end() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0, 0u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 0u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::configure_dutch_auction(
            RuntimeOrigin::signed(CREATOR), 0, 10000u128, 1000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));

        // 结束时刻价格应等于 end_price
        frame_system::Pallet::<Test>::set_block_number(100);
        let price = EntityTokenSale::get_current_price(0, None).unwrap();
        assert_eq!(price, 1000u128);

        // 超过结束时刻也应等于 end_price
        frame_system::Pallet::<Test>::set_block_number(200);
        let price = EntityTokenSale::get_current_price(0, None).unwrap();
        assert_eq!(price, 1000u128);
    });
}

// ==================== P0: force_cancel_sale ====================

#[test]
fn p0_force_cancel_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        assert_ok!(EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);
        assert_eq!(round.remaining_amount, 0u128);
        assert!(round.cancelled_at.is_some());
        assert!(ActiveRounds::<Test>::get().is_empty());
    });
}

#[test]
fn p0_force_cancel_sale_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::force_cancel_sale(RuntimeOrigin::signed(CREATOR), round_id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p0_force_cancel_sale_rejects_ended() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_noop!(
            EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

#[test]
fn p0_force_cancel_not_started_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);
    });
}

// ==================== P0: force_end_sale ====================

#[test]
fn p0_force_end_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 200u128, None));

        // Root 可在任意时间强制结束（无需等 end_block）
        assert_ok!(EntityTokenSale::force_end_sale(RuntimeOrigin::root(), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
        assert_eq!(round.sold_amount, 200u128);
        assert_eq!(round.remaining_amount, 0u128);
        assert!(ActiveRounds::<Test>::get().is_empty());
    });
}

#[test]
fn p0_force_end_sale_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::force_end_sale(RuntimeOrigin::signed(CREATOR), round_id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p0_force_end_sale_rejects_not_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round(); // NotStarted
        assert_noop!(
            EntityTokenSale::force_end_sale(RuntimeOrigin::root(), round_id),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== P1: force_refund ====================

#[test]
fn p1_force_refund_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        let buyer_before = Balances::free_balance(BUYER);
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        // force_cancel + force_refund
        assert_ok!(EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id));
        assert_ok!(EntityTokenSale::force_refund(RuntimeOrigin::root(), round_id, BUYER));

        let buyer_after = Balances::free_balance(BUYER);
        assert_eq!(buyer_after, buyer_before);

        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert!(sub.refunded);

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.total_refunded_nex, 10_000u128);
    });
}

#[test]
fn p1_force_refund_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        assert_ok!(EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id));

        assert_noop!(
            EntityTokenSale::force_refund(RuntimeOrigin::signed(CREATOR), round_id, BUYER),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p1_force_refund_rejects_not_cancelled() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        assert_noop!(
            EntityTokenSale::force_refund(RuntimeOrigin::root(), round_id, BUYER),
            Error::<Test>::SaleNotCancelled
        );
    });
}

#[test]
fn p1_force_refund_rejects_already_refunded() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        assert_ok!(EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id));
        assert_ok!(EntityTokenSale::force_refund(RuntimeOrigin::root(), round_id, BUYER));

        assert_noop!(
            EntityTokenSale::force_refund(RuntimeOrigin::root(), round_id, BUYER),
            Error::<Test>::AlreadyRefunded
        );
    });
}

// ==================== P1: force_withdraw_funds ====================

#[test]
fn p1_force_withdraw_funds_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let entity_before = Balances::free_balance(ENTITY_ACCOUNT);
        assert_ok!(EntityTokenSale::force_withdraw_funds(RuntimeOrigin::root(), round_id));
        let entity_after = Balances::free_balance(ENTITY_ACCOUNT);
        assert_eq!(entity_after - entity_before, 10_000u128);

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert!(round.funds_withdrawn);
    });
}

#[test]
fn p1_force_withdraw_funds_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_noop!(
            EntityTokenSale::force_withdraw_funds(RuntimeOrigin::signed(BUYER), round_id),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p1_force_withdraw_funds_rejects_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::force_withdraw_funds(RuntimeOrigin::root(), round_id),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== P1: update_sale_round ====================

#[test]
fn p1_update_sale_round_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::update_sale_round(
            RuntimeOrigin::signed(CREATOR), round_id,
            Some(2_000_000u128), Some(20u64.into()), Some(200u64.into()),
            Some(true), Some(3),
        ));

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.total_supply, 2_000_000u128);
        assert_eq!(round.remaining_amount, 2_000_000u128);
        assert_eq!(round.start_block, 20u64);
        assert_eq!(round.end_block, 200u64);
        assert!(round.kyc_required);
        assert_eq!(round.min_kyc_level, 3);
    });
}

#[test]
fn p1_update_sale_round_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::update_sale_round(
                RuntimeOrigin::signed(BUYER), round_id,
                Some(2_000_000u128), None, None, None, None,
            ),
            Error::<Test>::Unauthorized
        );
    });
}

#[test]
fn p1_update_sale_round_rejects_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::update_sale_round(
                RuntimeOrigin::signed(CREATOR), round_id,
                Some(2_000_000u128), None, None, None, None,
            ),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

#[test]
fn p1_update_sale_round_rejects_no_update() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::update_sale_round(
                RuntimeOrigin::signed(CREATOR), round_id,
                None, None, None, None, None,
            ),
            Error::<Test>::NoUpdateProvided
        );
    });
}

#[test]
fn p1_update_sale_round_validates_time_window() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // end_block < start_block after update
        assert_noop!(
            EntityTokenSale::update_sale_round(
                RuntimeOrigin::signed(CREATOR), round_id,
                None, Some(200u64.into()), Some(50u64.into()), None, None,
            ),
            Error::<Test>::InvalidTimeWindow
        );
    });
}

#[test]
fn p1_update_sale_round_rejects_zero_supply() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::update_sale_round(
                RuntimeOrigin::signed(CREATOR), round_id,
                Some(0u128), None, None, None, None,
            ),
            Error::<Test>::InvalidTotalSupply
        );
    });
}

// ==================== P1: increase_subscription ====================

#[test]
fn p1_increase_subscription_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 初次认购 100
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        let buyer_mid = Balances::free_balance(BUYER);

        // 追加 200
        assert_ok!(EntityTokenSale::increase_subscription(
            RuntimeOrigin::signed(BUYER), round_id, 200u128, None,
        ));

        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert_eq!(sub.amount, 300u128); // 100 + 200
        assert_eq!(sub.payment_amount, 30_000u128); // 300 * 100

        let buyer_after = Balances::free_balance(BUYER);
        assert_eq!(buyer_mid - buyer_after, 20_000u128); // additional: 200 * 100

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.sold_amount, 300u128);
    });
}

#[test]
fn p1_increase_subscription_rejects_not_subscribed() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::increase_subscription(
                RuntimeOrigin::signed(BUYER), round_id, 100u128, None,
            ),
            Error::<Test>::NotSubscribed
        );
    });
}

#[test]
fn p1_increase_subscription_rejects_exceeds_limit() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // max_purchase_per_account = 100_000
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 50_000u128, None));

        // 追加超过限额
        assert_noop!(
            EntityTokenSale::increase_subscription(
                RuntimeOrigin::signed(BUYER), round_id, 60_000u128, None,
            ),
            Error::<Test>::ExceedsPurchaseLimit
        );
    });
}

#[test]
fn p1_increase_subscription_rejects_sold_out() {
    new_test_ext().execute_with(|| {
        // 创建供应量只有 200 的轮次
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 200u128, 10u64.into(), 100u64.into(), false, 0, 0u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 200u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 150u128, None));

        // 追加超过剩余
        assert_noop!(
            EntityTokenSale::increase_subscription(RuntimeOrigin::signed(BUYER), 0, 100u128, None),
            Error::<Test>::SoldOut
        );
    });
}

// ==================== P1: remove_from_whitelist ====================

#[test]
fn p1_remove_from_whitelist_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        // 先添加白名单
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![(BUYER, None), (BUYER2, None)]).unwrap(),
        ));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 2);

        // 移除 BUYER
        assert_ok!(EntityTokenSale::remove_from_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));
        assert!(!RoundWhitelist::<Test>::contains_key(round_id, BUYER));
        assert!(RoundWhitelist::<Test>::contains_key(round_id, BUYER2));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 1);
    });
}

#[test]
fn p1_remove_from_whitelist_ignores_nonexistent() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![(BUYER, None)]).unwrap(),
        ));
        // 移除不在白名单中的地址 — 不报错，removed_count=0
        assert_ok!(EntityTokenSale::remove_from_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id,
            BoundedVec::try_from(vec![BUYER2]).unwrap(),
        ));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 1);
    });
}

#[test]
fn p1_remove_from_whitelist_rejects_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::remove_from_whitelist(
                RuntimeOrigin::signed(CREATOR), round_id,
                BoundedVec::try_from(vec![BUYER]).unwrap(),
            ),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== P2: remove_payment_option ====================

#[test]
fn p2_remove_payment_option_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, None, 100u128, 10u128, 10_000u128,
        ));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.payment_options_count, 1);

        assert_ok!(EntityTokenSale::remove_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id, 0,
        ));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.payment_options_count, 0);
        assert!(RoundPaymentOptions::<Test>::get(round_id).is_empty());
    });
}

#[test]
fn p2_remove_payment_option_rejects_invalid_index() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round();
        assert_noop!(
            EntityTokenSale::remove_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id, 0,
            ),
            Error::<Test>::PaymentOptionNotFound
        );
    });
}

#[test]
fn p2_remove_payment_option_rejects_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::remove_payment_option(
                RuntimeOrigin::signed(CREATOR), round_id, 0,
            ),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== P2: extend_sale ====================

#[test]
fn p2_extend_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::extend_sale(
            RuntimeOrigin::signed(CREATOR), round_id, 200u64.into(),
        ));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.end_block, 200u64);
    });
}

#[test]
fn p2_extend_sale_rejects_shorter() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round(); // end_block = 100
        assert_noop!(
            EntityTokenSale::extend_sale(
                RuntimeOrigin::signed(CREATOR), round_id, 50u64.into(),
            ),
            Error::<Test>::InvalidExtension
        );
    });
}

#[test]
fn p2_extend_sale_rejects_not_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round(); // NotStarted
        assert_noop!(
            EntityTokenSale::extend_sale(
                RuntimeOrigin::signed(CREATOR), round_id, 200u64.into(),
            ),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

// ==================== P3: pause_sale / resume_sale ====================

#[test]
fn p3_pause_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Paused);
    });
}

#[test]
fn p3_pause_sale_rejects_not_active() {
    new_test_ext().execute_with(|| {
        let round_id = setup_round(); // NotStarted
        assert_noop!(
            EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

#[test]
fn p3_resume_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));
        assert_ok!(EntityTokenSale::resume_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Active);
    });
}

#[test]
fn p3_resume_sale_rejects_not_paused() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::resume_sale(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::SaleNotPaused
        );
    });
}

#[test]
fn p3_subscribe_rejects_paused() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None),
            Error::<Test>::InvalidRoundStatus
        );
    });
}

#[test]
fn p3_cancel_paused_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);
        assert_eq!(round.remaining_amount, 0u128);
    });
}

#[test]
fn p3_end_paused_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // Paused 状态也可在 end_block 后结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

#[test]
fn p3_force_end_paused_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_ok!(EntityTokenSale::force_end_sale(RuntimeOrigin::root(), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

#[test]
fn p3_force_cancel_paused_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_ok!(EntityTokenSale::force_cancel_sale(RuntimeOrigin::root(), round_id));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);
    });
}

#[test]
fn p3_on_initialize_skips_paused_round() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 推进到 end_block 之后
        frame_system::Pallet::<Test>::set_block_number(101);
        EntityTokenSale::on_initialize(101);

        // Paused 轮次不应被自动结束
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Paused);
    });
}

#[test]
fn p2_extend_paused_sale_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // Paused 状态也可延长
        assert_ok!(EntityTokenSale::extend_sale(
            RuntimeOrigin::signed(CREATOR), round_id, 500u64.into(),
        ));
        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.end_block, 500u64);
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_create_sale_round() {
    new_test_ext().execute_with(|| {
        set_entity_locked(ENTITY_ID);
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::FixedPrice, 1_000u128, 10u64.into(), 100u64.into(), false, 0, 0u128,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== F2: Soft Cap 保护 ====================

#[test]
fn f2_soft_cap_met_ends_normally() {
    new_test_ext().execute_with(|| {
        // 创建 soft_cap = 500 的轮次
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
            500u128, // soft_cap
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 购买 10 个代币 → 支付 1000 NEX (10 * 100)
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 10u128, None));

        // 推进到结束并结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), 0));

        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

#[test]
fn f2_soft_cap_not_met_auto_cancels() {
    new_test_ext().execute_with(|| {
        // 创建 soft_cap = 50000 的轮次
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
            50000u128, // soft_cap 很高
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 只购买少量（不够 soft_cap）
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 10u128, None));

        // 推进到结束
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), 0));

        let round = SaleRounds::<Test>::get(0).unwrap();
        // 未达 soft cap → 应被取消
        assert_eq!(round.status, RoundStatus::Cancelled);
    });
}

#[test]
fn f2_soft_cap_auto_end_cancels() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
            999_999_999u128, // 超高 soft_cap
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));

        // on_initialize 自动结束 → 应因 soft cap 未达标而自动取消
        frame_system::Pallet::<Test>::set_block_number(101);
        <EntityTokenSale as Hooks<u64>>::on_initialize(101);

        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);
    });
}

#[test]
fn f2_zero_soft_cap_ends_normally() {
    new_test_ext().execute_with(|| {
        // soft_cap = 0 意味着无最低要求
        let round_id = setup_active_round();
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.status, RoundStatus::Ended);
    });
}

// ==================== F4: Insider 交易防护 ====================

#[test]
fn f4_insider_blocked_from_subscribe() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        MockDisclosureProvider::block_insider(ENTITY_ID, BUYER);

        assert_noop!(
            EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None),
            Error::<Test>::InsiderTradingBlocked
        );
    });
}

#[test]
fn f4_non_insider_can_subscribe() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // BUYER 不是 insider，可以正常认购
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
    });
}

#[test]
fn f4_insider_blocked_from_increase_subscription() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        // 先正常认购
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        // 然后标记为 insider
        MockDisclosureProvider::block_insider(ENTITY_ID, BUYER);
        // 追加认购被拒
        assert_noop!(
            EntityTokenSale::increase_subscription(RuntimeOrigin::signed(BUYER), round_id, 50u128, None),
            Error::<Test>::InsiderTradingBlocked
        );
    });
}

// ==================== F5: 白名单个人额度 ====================

#[test]
fn f5_whitelist_individual_allocation_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::WhitelistAllocation, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0, 0u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        // BUYER 获得 50 的个人配额
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), 0,
            BoundedVec::try_from(vec![(BUYER, Some(50u128))]).unwrap(),
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 认购 50 应成功
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 50u128, None));

        // 认购超过配额应失败（通过 increase_subscription）
        assert_noop!(
            EntityTokenSale::increase_subscription(RuntimeOrigin::signed(BUYER), 0, 10u128, None),
            Error::<Test>::ExceedsPurchaseLimit
        );
    });
}

#[test]
fn f5_whitelist_none_allocation_uses_default() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::WhitelistAllocation, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0, 0u128,
        ));
        // max_purchase_per_account = 100_000
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        // BUYER 获得 None 配额 → 使用默认限额
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), 0,
            BoundedVec::try_from(vec![(BUYER, None)]).unwrap(),
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 认购 100 应成功（在 100_000 限额内）
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));
    });
}

// ==================== F6: 发售统计查询 ====================

#[test]
fn f6_sale_statistics_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 200u128, None));

        let stats = EntityTokenSale::get_sale_statistics(round_id).unwrap();
        assert_eq!(stats.0, 1_000_000u128); // total_supply
        assert_eq!(stats.1, 200u128);        // sold_amount
        assert_eq!(stats.2, 999_800u128);    // remaining_amount
        assert_eq!(stats.3, 1);              // participants_count
    });
}

#[test]
fn f6_sale_statistics_none_for_missing() {
    new_test_ext().execute_with(|| {
        assert!(EntityTokenSale::get_sale_statistics(999).is_none());
    });
}

// ==================== F7: TokenSaleProvider trait ====================

#[test]
fn f7_token_sale_provider_active_round() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::TokenSaleProvider;

        // 无活跃轮次
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::active_sale_round(ENTITY_ID), None);

        let round_id = setup_active_round();
        // 有活跃轮次
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::active_sale_round(ENTITY_ID), Some(round_id));

        // 状态
        let status = <EntityTokenSale as TokenSaleProvider<u128>>::sale_round_status(round_id);
        assert_eq!(status, Some(pallet_entity_common::TokenSaleStatus::Active));

        // 数据
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::sold_amount(round_id), Some(0u128));
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::remaining_amount(round_id), Some(1_000_000u128));
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::participants_count(round_id), Some(0));
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::sale_entity_id(round_id), Some(ENTITY_ID));
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::sale_total_supply(round_id), Some(1_000_000u128));
    });
}

#[test]
fn f7_token_sale_provider_missing_round() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::TokenSaleProvider;
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::sale_round_status(999), None);
        assert_eq!(<EntityTokenSale as TokenSaleProvider<u128>>::sold_amount(999), None);
    });
}

// ==================== F8: 存储清理机制 ====================

#[test]
fn f8_cleanup_round_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        // 结束轮次
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 提取资金
        assert_ok!(EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id));

        // 现在清理
        assert_ok!(EntityTokenSale::cleanup_round(RuntimeOrigin::signed(CREATOR), round_id));

        // 验证存储已清理
        assert!(!Subscriptions::<Test>::contains_key(round_id, BUYER));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 0);
        assert!(RoundPaymentOptions::<Test>::get(round_id).is_empty());
    });
}

#[test]
fn f8_cleanup_rejects_active_round() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::cleanup_round(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::RoundNotCleanable
        );
    });
}

#[test]
fn f8_cleanup_rejects_non_creator() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        assert_ok!(EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id));

        assert_noop!(
            EntityTokenSale::cleanup_round(RuntimeOrigin::signed(BUYER), round_id),
            Error::<Test>::Unauthorized
        );
    });
}

#[test]
fn f8_cleanup_rejects_funds_not_withdrawn() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 未提取资金
        assert_noop!(
            EntityTokenSale::cleanup_round(RuntimeOrigin::signed(CREATOR), round_id),
            Error::<Test>::RoundNotCleanable
        );
    });
}

// ==================== F9: 批量强制退款 ====================

#[test]
fn f9_force_batch_refund_works() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        let buyer_balance_before = pallet_balances::Pallet::<Test>::free_balance(BUYER);

        // 取消轮次
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 批量退款
        assert_ok!(EntityTokenSale::force_batch_refund(
            RuntimeOrigin::root(), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));

        // 验证退款
        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert!(sub.refunded);
        let buyer_balance_after = pallet_balances::Pallet::<Test>::free_balance(BUYER);
        assert!(buyer_balance_after > buyer_balance_before);
    });
}

#[test]
fn f9_force_batch_refund_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_noop!(
            EntityTokenSale::force_batch_refund(
                RuntimeOrigin::signed(CREATOR), round_id,
                BoundedVec::try_from(vec![BUYER]).unwrap(),
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn f9_force_batch_refund_rejects_non_cancelled() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_noop!(
            EntityTokenSale::force_batch_refund(
                RuntimeOrigin::root(), round_id,
                BoundedVec::try_from(vec![BUYER]).unwrap(),
            ),
            Error::<Test>::SaleNotCancelled
        );
    });
}

#[test]
fn f9_force_batch_refund_rejects_empty_batch() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_noop!(
            EntityTokenSale::force_batch_refund(
                RuntimeOrigin::root(), round_id,
                BoundedVec::try_from(vec![]).unwrap(),
            ),
            Error::<Test>::EmptyBatch
        );
    });
}

#[test]
fn f9_force_batch_refund_skips_already_refunded() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 先单独退款
        assert_ok!(EntityTokenSale::force_refund(RuntimeOrigin::root(), round_id, BUYER));

        // 再批量退款 — 不应重复退
        assert_ok!(EntityTokenSale::force_batch_refund(
            RuntimeOrigin::root(), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));
        // 事件中 refunded_count 应为 0
    });
}

// ==================== F11: Lottery 模式阻断 ====================

#[test]
fn f11_lottery_mode_rejected() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityTokenSale::create_sale_round(
                RuntimeOrigin::signed(CREATOR), ENTITY_ID,
                SaleMode::Lottery, 1_000u128, 10u64.into(), 100u64.into(), false, 0, 0u128,
            ),
            Error::<Test>::LotteryNotImplemented
        );
    });
}

// ==================== F12: 发售期间代币转让限制 ====================

#[test]
fn f12_has_active_sale_returns_true_when_active() {
    new_test_ext().execute_with(|| {
        let _round_id = setup_active_round();
        assert!(EntityTokenSale::has_active_sale(ENTITY_ID));
    });
}

#[test]
fn f12_has_active_sale_returns_false_when_none() {
    new_test_ext().execute_with(|| {
        assert!(!EntityTokenSale::has_active_sale(ENTITY_ID));
    });
}

#[test]
fn f12_has_active_sale_returns_false_after_end() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        assert!(!EntityTokenSale::has_active_sale(ENTITY_ID));
    });
}

#[test]
fn f12_has_active_sale_true_when_paused() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::pause_sale(RuntimeOrigin::signed(CREATOR), round_id));
        // Paused 仍算活跃
        assert!(EntityTokenSale::has_active_sale(ENTITY_ID));
    });
}

// ==================== 审计回归测试 ====================

#[test]
fn h1h2_force_batch_refund_unreserves_entity_tokens() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        let reserved_before = MockTokenProvider::get_reserved(ENTITY_ID, ENTITY_ACCOUNT);
        assert!(reserved_before >= 100, "tokens should be reserved after subscribe");

        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 批量退款应释放 Entity 代币
        assert_ok!(EntityTokenSale::force_batch_refund(
            RuntimeOrigin::root(), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));

        let reserved_after = MockTokenProvider::get_reserved(ENTITY_ID, ENTITY_ACCOUNT);
        // cancel_sale 释放 remaining_amount，batch_refund 应释放 sold_amount (100)
        assert_eq!(reserved_after, 0, "all entity tokens should be unreserved after batch refund");
    });
}

#[test]
fn m1_force_batch_refund_updates_total_refunded_tokens() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER2), round_id, 200u128, None));

        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        assert_ok!(EntityTokenSale::force_batch_refund(
            RuntimeOrigin::root(), round_id,
            BoundedVec::try_from(vec![BUYER, BUYER2]).unwrap(),
        ));

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        assert_eq!(round.total_refunded_tokens, 300u128, "total_refunded_tokens should include both subscribers");
        assert!(round.total_refunded_nex > 0, "total_refunded_nex should be updated");
    });
}

#[test]
fn h1_force_batch_refund_skips_on_transfer_failure() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 清空 pallet 账户余额使 NEX 转账失败
        let pallet_account = EntityTokenSale::pallet_account();
        let pallet_bal = <pallet_balances::Pallet<Test> as Currency<u64>>::free_balance(&pallet_account);
        if pallet_bal > 1 {
            // 转走 pallet 账户的钱使其不足以退款
            let _ = <pallet_balances::Pallet<Test> as Currency<u64>>::transfer(
                &pallet_account, &99u64, pallet_bal - 1,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            );
        }

        // 批量退款应成功（不 panic），但 BUYER 不会被标记为 refunded
        assert_ok!(EntityTokenSale::force_batch_refund(
            RuntimeOrigin::root(), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));

        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert!(!sub.refunded, "subscriber should NOT be marked refunded when NEX transfer fails");
    });
}

// ==================== H1-R6: soft cap failure 后 claim_refund 可正常退款 ====================

#[test]
/// H1-R6: end_sale soft cap 未达标后，claim_refund 应能正常退还 NEX
fn h1r6_claim_refund_works_after_end_sale_soft_cap_failure() {
    new_test_ext().execute_with(|| {
        // 创建 soft_cap = 999_999 的轮次（几乎不可能达标）
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
            999_999u128, // 极高 soft_cap
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        let buyer_before = Balances::free_balance(BUYER);

        // 认购 100 tokens, 支付 10_000 NEX
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));

        // 推进到 end_block 之后，结束发售 — soft cap 未达标
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), 0));

        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);

        // H1-R6: claim_refund 应成功（修复前因 IncompleteUnreserve 失败）
        assert_ok!(EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), 0));

        let buyer_after = Balances::free_balance(BUYER);
        assert_eq!(buyer_after, buyer_before, "NEX should be fully refunded");

        // 验证 entity tokens 全部释放
        let reserved = MockTokenProvider::get_reserved(ENTITY_ID, ENTITY_ACCOUNT);
        assert_eq!(reserved, 0, "all entity tokens should be unreserved after refund");
    });
}

#[test]
/// H1-R6: on_initialize auto-cancel (soft cap) 后 claim_refund 应正常工作
fn h1r6_claim_refund_works_after_auto_end_soft_cap_failure() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
            999_999u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        let buyer_before = Balances::free_balance(BUYER);

        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));

        // on_initialize 自动结束 → soft cap 未达标 → 自动取消
        frame_system::Pallet::<Test>::set_block_number(101);
        <EntityTokenSale as Hooks<u64>>::on_initialize(101);

        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.status, RoundStatus::Cancelled);

        // claim_refund 应成功
        assert_ok!(EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), 0));

        let buyer_after = Balances::free_balance(BUYER);
        assert_eq!(buyer_after, buyer_before, "NEX should be fully refunded after auto-cancel");
    });
}

#[test]
/// H1-R6: soft cap 取消后 reclaim_unclaimed_tokens 也应正常工作
fn h1r6_reclaim_works_after_soft_cap_cancel() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 1_000_000u128,
            10u64.into(), 100u64.into(), false, 0,
            999_999u128,
        ));
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), 0, None, 100u128, 10u128, 100_000u128,
        ));
        assert_ok!(EntityTokenSale::start_sale(RuntimeOrigin::signed(CREATOR), 0));
        frame_system::Pallet::<Test>::set_block_number(10);

        // 2 个用户认购
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), 0, 100u128, None));
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER2), 0, 200u128, None));

        // end_sale → soft cap 未达标 → 取消
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), 0));

        // BUYER 领取退款
        assert_ok!(EntityTokenSale::claim_refund(RuntimeOrigin::signed(BUYER), 0));

        // BUYER2 不领取退款 — 宽限期后 reclaim
        frame_system::Pallet::<Test>::set_block_number(210); // cancelled_at=101, grace=100, deadline=201
        let entity_before = Balances::free_balance(ENTITY_ACCOUNT);
        assert_ok!(EntityTokenSale::reclaim_unclaimed_tokens(RuntimeOrigin::signed(CREATOR), 0));

        // BUYER2 的 20_000 NEX 应转给 entity
        let entity_after = Balances::free_balance(ENTITY_ACCOUNT);
        assert_eq!(entity_after - entity_before, 20_000u128);

        let round = SaleRounds::<Test>::get(0).unwrap();
        assert_eq!(round.status, RoundStatus::Completed);
    });
}

#[test]
fn m2_dutch_price_no_overflow_with_large_values() {
    new_test_ext().execute_with(|| {
        // 创建荷兰拍卖轮次，使用接近 u128 上限的价格
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::DutchAuction, 1_000u128, 10u64.into(), 110u64.into(), false, 0, 0u128,
        ));
        let round_id = 0u64;
        assert_ok!(EntityTokenSale::add_payment_option(
            RuntimeOrigin::signed(CREATOR), round_id,
            None, 1u128, 1u128, 100_000u128,
        ));

        // 使用非常大的价格范围
        let large_start: u128 = u128::MAX / 2;
        let large_end: u128 = 1_000_000;
        assert_ok!(EntityTokenSale::configure_dutch_auction(
            RuntimeOrigin::signed(CREATOR), round_id,
            large_start, large_end,
        ));

        assert_ok!(EntityTokenSale::start_sale(
            RuntimeOrigin::signed(CREATOR), round_id,
        ));

        // 设置在中间时刻 (start=10, end=110, midpoint=60)
        frame_system::Pallet::<Test>::set_block_number(60);

        let round = SaleRounds::<Test>::get(round_id).unwrap();
        let price = EntityTokenSale::calculate_dutch_price(&round).unwrap();
        let price_u128: u128 = price;

        // 价格应在 start 和 end 之间
        assert!(price_u128 >= large_end, "price should be >= end_price, got {}", price_u128);
        assert!(price_u128 <= large_start, "price should be <= start_price, got {}", price_u128);
        // 中间点价格应大致在中间
        let expected_mid = large_start / 2;
        let tolerance = large_start / 10;
        assert!(
            price_u128 > expected_mid.saturating_sub(tolerance) && price_u128 < expected_mid.saturating_add(tolerance),
            "price at midpoint should be roughly half of start_price, got {} vs expected ~{}", price_u128, expected_mid
        );
    });
}

// ==================== M1-R7: cleanup_round 释放 EntityRounds 槽位 ====================

#[test]
fn m1r7_cleanup_round_frees_entity_rounds_slot() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        // 结束 + 提取资金
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        assert_ok!(EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id));

        // 确认 EntityRounds 包含该轮次
        let rounds_before = EntityRounds::<Test>::get(ENTITY_ID);
        assert!(rounds_before.contains(&round_id));

        // cleanup
        assert_ok!(EntityTokenSale::cleanup_round(RuntimeOrigin::signed(CREATOR), round_id));

        // M1-R7: EntityRounds 应不再包含该轮次
        let rounds_after = EntityRounds::<Test>::get(ENTITY_ID);
        assert!(!rounds_after.contains(&round_id), "round should be removed from EntityRounds after cleanup");
    });
}

#[test]
fn m1r7_cleanup_allows_new_round_after_slot_freed() {
    new_test_ext().execute_with(|| {
        // 创建多个轮次，清理后应能继续创建
        let round_id = setup_active_round();

        // 结束 + 提取 + 清理
        frame_system::Pallet::<Test>::set_block_number(101);
        assert_ok!(EntityTokenSale::end_sale(RuntimeOrigin::signed(CREATOR), round_id));
        assert_ok!(EntityTokenSale::withdraw_funds(RuntimeOrigin::signed(CREATOR), round_id));
        assert_ok!(EntityTokenSale::cleanup_round(RuntimeOrigin::signed(CREATOR), round_id));

        // 应能创建新轮次（EntityRounds 槽位已释放）
        assert_ok!(EntityTokenSale::create_sale_round(
            RuntimeOrigin::signed(CREATOR), ENTITY_ID,
            SaleMode::FixedPrice, 500_000u128,
            200u64.into(), 300u64.into(), false, 0, 0u128,
        ));

        let rounds = EntityRounds::<Test>::get(ENTITY_ID);
        assert_eq!(rounds.len(), 1, "only the new round should be in EntityRounds");
        assert_eq!(rounds[0], 1u64, "new round id should be 1");
    });
}

// ==================== L1-R7: force_batch_refund 部分 unreserve 回滚 ====================

#[test]
fn l1r7_force_batch_refund_rereserves_on_partial_unreserve() {
    new_test_ext().execute_with(|| {
        let round_id = setup_active_round();
        assert_ok!(EntityTokenSale::subscribe(RuntimeOrigin::signed(BUYER), round_id, 100u128, None));

        assert_ok!(EntityTokenSale::cancel_sale(RuntimeOrigin::signed(CREATOR), round_id));

        // 篡改 reserved：降低到只剩 50（正常应有 100 给 BUYER）
        // cancel_sale 已释放 remaining_amount，只剩 sold_amount (100) 在 reserved
        // 手动降低 reserved 模拟异常状态
        MockTokenProvider::set_reserved(ENTITY_ID, ENTITY_ACCOUNT, 50);

        let reserved_before = MockTokenProvider::get_reserved(ENTITY_ID, ENTITY_ACCOUNT);
        assert_eq!(reserved_before, 50);

        // 批量退款 — unreserve(100) 只能释放 50，deficit=50
        // L1-R7: 应回滚已释放的 50，reserved 恢复为 50
        assert_ok!(EntityTokenSale::force_batch_refund(
            RuntimeOrigin::root(), round_id,
            BoundedVec::try_from(vec![BUYER]).unwrap(),
        ));

        // BUYER 不应被标记 refunded（因为 unreserve 不完整）
        let sub = Subscriptions::<Test>::get(round_id, BUYER).unwrap();
        assert!(!sub.refunded, "subscriber should NOT be refunded when unreserve has deficit");

        // L1-R7: reserved 应恢复（re-reserve 了 freed 的 50）
        let reserved_after = MockTokenProvider::get_reserved(ENTITY_ID, ENTITY_ACCOUNT);
        assert_eq!(reserved_after, 50, "reserved should be restored after re-reserve");
    });
}
