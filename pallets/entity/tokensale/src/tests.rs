//! 代币发售模块测试

use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;

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
        assert_eq!(round.payment_options.len(), 1);
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
            vec![BUYER, BUYER2],
        ));
        assert!(RoundWhitelist::<Test>::get(round_id, BUYER));
        assert!(RoundWhitelist::<Test>::get(round_id, BUYER2));
        assert_eq!(WhitelistCount::<Test>::get(round_id), 2);

        // 重复添加不增加计数
        assert_ok!(EntityTokenSale::add_to_whitelist(
            RuntimeOrigin::signed(CREATOR), round_id, vec![BUYER],
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
                RuntimeOrigin::signed(CREATOR), round_id, vec![BUYER],
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
            RuntimeOrigin::signed(CREATOR), 0, vec![BUYER],
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
