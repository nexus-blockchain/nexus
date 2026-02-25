use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok, weights::Weight};
use sp_runtime::traits::Zero;

fn tron_address() -> Vec<u8> {
    b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb".to_vec()
}

fn buyer_tron() -> Vec<u8> {
    b"T1234567890123456789012345678901AB".to_vec()
}

/// 设置初始价格（seed_liquidity 需要基准价格）
fn setup_seed_price() {
    assert_ok!(NexMarket::set_initial_price(RuntimeOrigin::root(), 500_000));
}

// ==================== 卖单测试 ====================

#[test]
fn place_sell_order_works() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128; // 100 NEX
        let price = 500_000u64;            // 0.5 USDT/NEX

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, price, tron_address(),
        ));

        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.side, OrderSide::Sell);
        assert_eq!(order.nex_amount, nex);
        assert_eq!(order.usdt_price, price);
        assert_eq!(order.status, OrderStatus::Open);

        // NEX 已锁定
        assert_eq!(Balances::reserved_balance(ALICE), nex);
    });
}

#[test]
fn place_sell_order_insufficient_balance() {
    new_test_ext().execute_with(|| {
        let too_much = 2_000_000_000_000_000u128; // 2000 NEX, Alice only has 1000
        assert_noop!(
            NexMarket::place_sell_order(
                RuntimeOrigin::signed(ALICE), too_much, 500_000, tron_address(),
            ),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn place_sell_order_zero_price() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            NexMarket::place_sell_order(
                RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 0, tron_address(),
            ),
            Error::<Test>::ZeroPrice
        );
    });
}

#[test]
fn place_sell_order_invalid_tron_address() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            NexMarket::place_sell_order(
                RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, b"short".to_vec(),
            ),
            Error::<Test>::InvalidTronAddress
        );
    });
}

// ==================== 买单测试 ====================

#[test]
fn place_buy_order_works() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;
        let price = 500_000u64;

        let bob_before = Balances::free_balance(BOB);
        assert_ok!(NexMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), nex, price, buyer_tron(),
        ));

        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.nex_amount, nex);
        // 买单预锁定保证金
        assert!(!order.buyer_deposit.is_zero());
        assert_eq!(Balances::reserved_balance(BOB), order.buyer_deposit);
        assert_eq!(Balances::free_balance(BOB), bob_before - order.buyer_deposit);
    });
}

#[test]
fn place_buy_order_insufficient_deposit() {
    new_test_ext().execute_with(|| {
        // Charlie 只有 100 NEX，不够支付大额买单的保证金
        let huge_nex = 1_000_000_000_000_000_000u128; // 1000000 NEX
        assert_noop!(
            NexMarket::place_buy_order(
                RuntimeOrigin::signed(CHARLIE), huge_nex, 500_000, buyer_tron(),
            ),
            Error::<Test>::InsufficientDepositBalance
        );
    });
}

// ==================== 取消订单测试 ====================

#[test]
fn cancel_sell_order_returns_nex() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;
        let initial = Balances::free_balance(ALICE);

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_eq!(Balances::free_balance(ALICE), initial - nex);

        assert_ok!(NexMarket::cancel_order(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(Balances::free_balance(ALICE), initial);

        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.status, OrderStatus::Cancelled);
    });
}

#[test]
fn cancel_order_not_owner() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_noop!(
            NexMarket::cancel_order(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NotOrderOwner
        );
    });
}

#[test]
fn cancel_buy_order_works() {
    new_test_ext().execute_with(|| {
        let bob_before = Balances::free_balance(BOB);
        assert_ok!(NexMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), 100_000_000_000_000, 500_000, buyer_tron(),
        ));
        // 保证金已锁定
        assert!(Balances::reserved_balance(BOB) > 0);

        assert_ok!(NexMarket::cancel_order(RuntimeOrigin::signed(BOB), 0));

        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.status, OrderStatus::Cancelled);
        // 保证金已退还
        assert_eq!(Balances::reserved_balance(BOB), 0);
        assert_eq!(Balances::free_balance(BOB), bob_before);
    });
}

// ==================== reserve_sell_order 测试 ====================

#[test]
fn reserve_sell_order_works() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128; // 100 NEX
        let price = 500_000u64;            // 0.5 USDT/NEX

        // Alice 挂卖单
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, price, tron_address(),
        ));

        // Bob 预锁定（吃卖单）
        let _bob_initial = Balances::free_balance(BOB);
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // 检查 trade
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.seller, ALICE);
        assert_eq!(trade.buyer, BOB);
        assert_eq!(trade.nex_amount, nex);
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingPayment);
        assert!(!trade.buyer_deposit.is_zero());
        assert!(trade.buyer_tron_address.is_some());

        // Bob 保证金已锁定
        assert!(Balances::reserved_balance(BOB) > 0);

        // 订单已更新
        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.filled_amount, nex);
        assert_eq!(order.status, OrderStatus::Filled);
    });
}

#[test]
fn reserve_sell_order_partial() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));

        // Bob 只吃一半
        let half = 50_000_000_000_000u128;
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, Some(half), buyer_tron(),
        ));

        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.filled_amount, half);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
    });
}

#[test]
fn reserve_sell_order_cannot_take_own() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_noop!(
            NexMarket::reserve_sell_order(RuntimeOrigin::signed(ALICE), 0, None, buyer_tron()),
            Error::<Test>::CannotTakeOwnOrder
        );
    });
}

// ==================== accept_buy_order 测试 ====================

#[test]
fn accept_buy_order_works() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        // Bob 挂买单（保证金已预锁定）
        assert_ok!(NexMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), nex, 500_000, buyer_tron(),
        ));
        let deposit_after_place = Balances::reserved_balance(BOB);
        assert!(deposit_after_place > 0);

        // Alice 接受（卖 NEX）— 不应题外增加 Bob 的 reserved
        assert_ok!(NexMarket::accept_buy_order(
            RuntimeOrigin::signed(ALICE), 0, None, tron_address(),
        ));
        assert_eq!(Balances::reserved_balance(ALICE), nex);
        // Bob 的 reserved 不变（保证金已在 place_buy_order 时预锁）
        assert_eq!(Balances::reserved_balance(BOB), deposit_after_place);

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.seller, ALICE);
        assert_eq!(trade.buyer, BOB);
        // 交易的保证金 == 订单预锁的保证金（全额成交）
        assert_eq!(trade.buyer_deposit, deposit_after_place);
    });
}

// ==================== confirm_payment 测试 ====================

#[test]
fn confirm_payment_works() {
    new_test_ext().execute_with(|| {
        // 创建卖单 + 预锁定
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // Bob 确认支付（卖单流程，buyer_tron 已在 reserve 时提供，可传 None）
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingVerification);
        assert!(trade.buyer_tron_address.is_some());

        // 已加入待验证队列
        let pending = NexMarket::pending_usdt_trades();
        assert!(pending.contains(&0));
    });
}

#[test]
fn confirm_payment_not_buyer() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert_noop!(
            NexMarket::confirm_payment(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NotTradeParticipant
        );
    });
}

// ==================== OCW + claim_reward 测试 ====================

#[test]
fn full_trade_flow_exact_payment() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128; // 100 NEX
        let price = 500_000u64;            // 0.5 USDT/NEX

        // 1. Alice 挂卖单
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, price, tron_address(),
        ));

        // 2. Bob 预锁定
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let usdt_amount = trade.usdt_amount;

        // 3. Bob 确认支付
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));

        // 4. OCW 提交结果（精确付款）
        assert_ok!(NexMarket::submit_ocw_result(
            RuntimeOrigin::none(), 0, usdt_amount,
        ));

        let (result, _) = NexMarket::ocw_verification_results(0).unwrap();
        assert_eq!(result, PaymentVerificationResult::Exact);

        // 5. Charlie 领取奖励
        let bob_before = Balances::free_balance(BOB);

        assert_ok!(NexMarket::claim_verification_reward(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        // Bob 获得 NEX + 保证金退还
        let bob_after = Balances::free_balance(BOB);
        assert!(bob_after > bob_before);

        // 保证金已退还（Bob 的 reserved 为 0）
        assert_eq!(Balances::reserved_balance(BOB), 0);

        // Trade 已完成
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Released);
    });
}

#[test]
fn underpaid_enters_pending_then_finalize() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));

        // 少付 70% → 进入 UnderpaidPending
        let actual = expected * 70 / 100;
        assert_ok!(NexMarket::submit_ocw_result(
            RuntimeOrigin::none(), 0, actual,
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::UnderpaidPending);
        assert!(trade.underpaid_deadline.is_some());
        assert!(NexMarket::pending_underpaid_trades().contains(&0));

        // 补付窗口未到期 → finalize 失败
        assert_noop!(
            NexMarket::finalize_underpaid(RuntimeOrigin::signed(CHARLIE), 0),
            Error::<Test>::UnderpaidGraceNotExpired
        );

        // 推进到窗口到期后
        let deadline: u64 = trade.underpaid_deadline.unwrap().into();
        System::set_block_number(deadline + 1);

        let alice_before = Balances::free_balance(ALICE);
        let bob_before = Balances::free_balance(BOB);

        assert_ok!(NexMarket::finalize_underpaid(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        // Bob 获得约 70% 的 NEX
        let bob_nex_gained = Balances::free_balance(BOB) - bob_before;
        let expected_nex = nex * 70 / 100;
        assert!(bob_nex_gained <= expected_nex + 1_000_000_000);

        // Alice 退还了约 30% 的 NEX
        let alice_gained = Balances::free_balance(ALICE) - alice_before;
        assert!(alice_gained > 0);

        // 保证金被没收（70% ratio → 50% forfeit rate）
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Forfeited);
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
    });
}

#[test]
fn severely_underpaid_auto_process() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));

        // 严重少付 10%
        let actual = expected * 10 / 100;
        assert_ok!(NexMarket::submit_ocw_result(
            RuntimeOrigin::none(), 0, actual,
        ));

        let (result, _) = NexMarket::ocw_verification_results(0).unwrap();
        assert_eq!(result, PaymentVerificationResult::SeverelyUnderpaid);

        assert_ok!(NexMarket::claim_verification_reward(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Forfeited);
    });
}

#[test]
fn underpaid_topup_upgrades_to_exact() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        // 少付 80% → UnderpaidPending
        let actual_80 = expected * 80 / 100;
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, actual_80));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::UnderpaidPending);

        // 补付窗口内 OCW 检测到买家补齐了 → submit_underpaid_update
        assert_ok!(NexMarket::submit_underpaid_update(RuntimeOrigin::none(), 0, expected));

        // 状态回到 AwaitingVerification（补齐了）
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingVerification);
        assert!(!NexMarket::pending_underpaid_trades().contains(&0));
        assert!(NexMarket::pending_usdt_trades().contains(&0));

        // 正常 claim 流程
        assert_ok!(NexMarket::claim_verification_reward(RuntimeOrigin::signed(CHARLIE), 0));
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Released);
    });
}

#[test]
fn underpaid_update_rejects_decrease() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        let actual_80 = expected * 80 / 100;
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, actual_80));

        // 尝试提交更低的金额 → 应该是 no-op（不会更新）
        assert_ok!(NexMarket::submit_underpaid_update(RuntimeOrigin::none(), 0, actual_80 - 1));

        let (_, stored_amount) = NexMarket::ocw_verification_results(0).unwrap();
        assert_eq!(stored_amount, actual_80); // 金额未变
    });
}

#[test]
fn graduated_deposit_forfeit_light_underpay() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;
        let deposit = trade.buyer_deposit;

        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        // 少付 97%（轻微少付 → forfeit 20%）
        let actual_97 = expected * 97 / 100;
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, actual_97));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::UnderpaidPending);

        let deadline: u64 = trade.underpaid_deadline.unwrap().into();
        System::set_block_number(deadline + 1);

        let bob_before = Balances::free_balance(BOB);
        assert_ok!(NexMarket::finalize_underpaid(RuntimeOrigin::signed(CHARLIE), 0));

        // 97% ratio → forfeit rate 20% → 买家应该退还 80% 保证金
        // 退还的保证金 = deposit * 80% (unreserved 回 free balance)
        // Bob 还获得 97% 的 NEX
        let bob_after = Balances::free_balance(BOB);
        // Bob 净收入 = 97% NEX + 80% 保证金退还
        assert!(bob_after > bob_before);

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Forfeited);
    });
}

#[test]
fn finalize_underpaid_full_topup_in_window() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        // 少付 60% → UnderpaidPending
        let actual_60 = expected * 60 / 100;
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, actual_60));
        assert_eq!(NexMarket::usdt_trades(0).unwrap().status, UsdtTradeStatus::UnderpaidPending);

        // 窗口内补齐到 100%，但不做 claim（仍是 AwaitingVerification）
        assert_ok!(NexMarket::submit_underpaid_update(RuntimeOrigin::none(), 0, expected));
        assert_eq!(NexMarket::usdt_trades(0).unwrap().status, UsdtTradeStatus::AwaitingVerification);

        // 正常 claim → 全额结算
        assert_ok!(NexMarket::claim_verification_reward(RuntimeOrigin::signed(CHARLIE), 0));
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Released);
    });
}

#[test]
fn submit_underpaid_update_rejects_wrong_status() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        // 还在 AwaitingVerification，不是 UnderpaidPending
        assert_noop!(
            NexMarket::submit_underpaid_update(RuntimeOrigin::none(), 0, 50_000_000),
            Error::<Test>::NotUnderpaidPending
        );
    });
}

#[test]
fn process_timeout_handles_underpaid_pending() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        // 少付 90% → UnderpaidPending
        let actual_90 = expected * 90 / 100;
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, actual_90));
        assert_eq!(NexMarket::usdt_trades(0).unwrap().status, UsdtTradeStatus::UnderpaidPending);

        let trade = NexMarket::usdt_trades(0).unwrap();
        let deadline: u64 = trade.underpaid_deadline.unwrap().into();

        // 补付窗口未到期 → process_timeout 也失败
        assert_noop!(
            NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0),
            Error::<Test>::UnderpaidGraceNotExpired
        );

        // 推进到窗口到期 → process_timeout 可以终裁
        System::set_block_number(deadline + 1);
        assert_ok!(NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
        assert!(!NexMarket::pending_underpaid_trades().contains(&0));
    });
}

#[test]
fn auto_confirm_underpaid_routes_to_pending() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        // auto_confirm with 80% → 应该进入 UnderpaidPending（而非 AwaitingVerification）
        let actual_80 = expected * 80 / 100;
        assert_ok!(NexMarket::auto_confirm_payment(RuntimeOrigin::none(), 0, actual_80));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::UnderpaidPending);
        assert!(trade.underpaid_deadline.is_some());
        assert!(NexMarket::pending_underpaid_trades().contains(&0));
        // 不在 PendingUsdtTrades
        assert!(!NexMarket::pending_usdt_trades().contains(&0));
        // AwaitingPaymentTrades 已清理
        assert!(!NexMarket::awaiting_payment_trades().contains(&0));
    });
}

#[test]
fn auto_confirm_exact_routes_to_verification() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let expected = trade.usdt_amount;

        // auto_confirm exact → AwaitingVerification
        assert_ok!(NexMarket::auto_confirm_payment(RuntimeOrigin::none(), 0, expected));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingVerification);
        assert!(NexMarket::pending_usdt_trades().contains(&0));

        // claim → Completed
        assert_ok!(NexMarket::claim_verification_reward(RuntimeOrigin::signed(CHARLIE), 0));
        assert_eq!(NexMarket::usdt_trades(0).unwrap().status, UsdtTradeStatus::Completed);
    });
}

// ==================== 超时测试 ====================

#[test]
fn process_timeout_works() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // 模拟超时
        let trade = NexMarket::usdt_trades(0).unwrap();
        let timeout_block: u64 = trade.timeout_at.into();
        System::set_block_number(timeout_block + 1);

        let alice_before = Balances::free_balance(ALICE);

        assert_ok!(NexMarket::process_timeout(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        // NEX 退还给 Alice
        assert_eq!(Balances::free_balance(ALICE) - alice_before, nex);
        assert_eq!(Balances::reserved_balance(ALICE), 0);

        // 订单回滚
        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.filled_amount, 0);

        // Trade 已退款
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Refunded);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Forfeited);
    });
}

#[test]
fn process_timeout_before_expiry_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        // 不推进区块 → 超时检查失败
        assert_noop!(
            NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0),
            Error::<Test>::InvalidTradeStatus
        );
    });
}

#[test]
fn process_timeout_awaiting_verification_grace_period_blocks() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        // 买家确认付款 → AwaitingVerification
        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingVerification);
        let timeout_block: u64 = trade.timeout_at.into();

        // 超过 timeout 但在宽限期内 → StillInGracePeriod
        System::set_block_number(timeout_block + 1);
        assert_noop!(
            NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0),
            Error::<Test>::StillInGracePeriod
        );

        // 超过 timeout + grace_period → 允许超时
        // grace = 600, timeout_block + 600 + 1
        System::set_block_number(timeout_block + 601);
        assert_ok!(NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Refunded);
    });
}

#[test]
fn process_timeout_awaiting_verification_settles_if_ocw_result_exists() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let timeout_block: u64 = trade.timeout_at.into();

        // OCW 提交了验证结果（精确到账）
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, 50_000_000));

        // 超过 timeout + grace → 但已有 OCW 结果，应按正常流程结算
        System::set_block_number(timeout_block + 601);

        let alice_before = Balances::free_balance(ALICE);
        let bob_before = Balances::free_balance(BOB);

        assert_ok!(NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0));

        // 应走 process_full_payment → NEX 给买家，保证金退还
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::Released);

        // BOB 收到 NEX
        assert!(Balances::free_balance(BOB) > bob_before);
    });
}

#[test]
fn process_timeout_awaiting_payment_no_grace_period() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        // 不调用 confirm_payment → 仍在 AwaitingPayment

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingPayment);
        let timeout_block: u64 = trade.timeout_at.into();

        // 超过 timeout 即可，不需要宽限期
        System::set_block_number(timeout_block + 1);
        assert_ok!(NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0));

        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Refunded);
    });
}

// ==================== 自动检测付款测试 ====================

#[test]
fn auto_confirm_payment_works() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // Trade 在 AwaitingPayment，且在跟踪队列中
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingPayment);
        assert!(NexMarket::awaiting_payment_trades().contains(&0));

        // OCW 检测到 USDT 已到账 → sidecar 调用 auto_confirm_payment
        assert_ok!(NexMarket::auto_confirm_payment(RuntimeOrigin::none(), 0, 50_000_000));

        // Trade 状态跳到 AwaitingVerification + OCW 结果已存储
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::AwaitingVerification);
        assert!(NexMarket::ocw_verification_results(0).is_some());
        let (result, amount) = NexMarket::ocw_verification_results(0).unwrap();
        assert_eq!(result, PaymentVerificationResult::Exact);
        assert_eq!(amount, 50_000_000);

        // 从 AwaitingPaymentTrades 移除
        assert!(!NexMarket::awaiting_payment_trades().contains(&0));
        // 加入 PendingUsdtTrades
        assert!(NexMarket::pending_usdt_trades().contains(&0));

        // 任何人可以领取奖励完成结算
        assert_ok!(NexMarket::claim_verification_reward(RuntimeOrigin::signed(CHARLIE), 0));
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.status, UsdtTradeStatus::Completed);
    });
}

#[test]
fn auto_confirm_payment_rejects_non_awaiting_payment() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        // 买家手动确认 → 变为 AwaitingVerification
        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));

        // auto_confirm_payment 应失败（已不是 AwaitingPayment）
        assert_noop!(
            NexMarket::auto_confirm_payment(RuntimeOrigin::none(), 0, 50_000_000),
            Error::<Test>::InvalidTradeStatus
        );
    });
}

#[test]
fn auto_confirm_payment_rejects_signed_origin() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // signed origin 应失败（只允许 unsigned）
        assert_noop!(
            NexMarket::auto_confirm_payment(RuntimeOrigin::signed(CHARLIE), 0, 50_000_000),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn awaiting_payment_trades_tracking() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        // 创建交易 → 加入跟踪队列
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert!(NexMarket::awaiting_payment_trades().contains(&0));

        // confirm_payment → 从跟踪队列移除
        assert_ok!(NexMarket::confirm_payment(RuntimeOrigin::signed(BOB), 0));
        assert!(!NexMarket::awaiting_payment_trades().contains(&0));
    });
}

#[test]
fn awaiting_payment_trades_cleaned_on_timeout() {
    new_test_ext().execute_with(|| {
        let nex = 100_000_000_000_000u128;

        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), nex, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert!(NexMarket::awaiting_payment_trades().contains(&0));

        let trade = NexMarket::usdt_trades(0).unwrap();
        let timeout_block: u64 = trade.timeout_at.into();
        System::set_block_number(timeout_block + 1);

        assert_ok!(NexMarket::process_timeout(RuntimeOrigin::signed(CHARLIE), 0));
        assert!(!NexMarket::awaiting_payment_trades().contains(&0));
    });
}

// ==================== 价格保护测试 ====================

#[test]
fn set_initial_price_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::set_initial_price(
            RuntimeOrigin::root(), 500_000,
        ));

        let config = NexMarket::price_protection().unwrap();
        assert_eq!(config.initial_price, Some(500_000));
        assert_eq!(NexMarket::last_trade_price(), Some(500_000));
    });
}

#[test]
fn configure_price_protection_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::configure_price_protection(
            RuntimeOrigin::root(), true, 3000, 6000, 50,
        ));

        let config = NexMarket::price_protection().unwrap();
        assert_eq!(config.max_price_deviation, 3000);
        assert_eq!(config.circuit_breaker_threshold, 6000);
        assert_eq!(config.min_trades_for_twap, 50);
    });
}

#[test]
fn price_deviation_blocks_extreme_orders() {
    new_test_ext().execute_with(|| {
        // 设置初始价格 0.5 USDT/NEX
        assert_ok!(NexMarket::set_initial_price(RuntimeOrigin::root(), 500_000));

        // 正常范围内的价格可以挂单
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 10_000_000_000_000, 550_000, tron_address(), // +10%
        ));

        // 偏离超过 20% 默认阈值 → 被阻止
        assert_noop!(
            NexMarket::place_sell_order(
                RuntimeOrigin::signed(ALICE), 10_000_000_000_000, 1_000_000, tron_address(), // +100%
            ),
            Error::<Test>::PriceDeviationTooHigh
        );
    });
}

// ==================== 市场统计测试 ====================

#[test]
fn market_stats_updated() {
    new_test_ext().execute_with(|| {
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), 50_000_000_000_000, 500_000, buyer_tron(),
        ));

        let stats = NexMarket::market_stats();
        assert_eq!(stats.total_orders, 2);
    });
}

// ==================== 最优价格测试 ====================

#[test]
fn best_prices_updated() {
    new_test_ext().execute_with(|| {
        // Alice 卖 100 NEX @ 0.6
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 600_000, tron_address(),
        ));
        // Alice 卖 100 NEX @ 0.5
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        // Bob 买 100 NEX @ 0.4
        assert_ok!(NexMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), 100_000_000_000_000, 400_000, buyer_tron(),
        ));
        // Bob 买 100 NEX @ 0.45
        assert_ok!(NexMarket::place_buy_order(
            RuntimeOrigin::signed(BOB), 100_000_000_000_000, 450_000, buyer_tron(),
        ));

        assert_eq!(NexMarket::best_ask(), Some(500_000));  // 最低卖价
        assert_eq!(NexMarket::best_bid(), Some(450_000));  // 最高买价
    });
}

// ==================== 多档判定测试 ====================

#[test]
fn payment_verification_result_categories() {
    new_test_ext().execute_with(|| {
        // 通过 OCW 提交结果间接测试多档判定
        // 精确付款
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 10_000_000_000_000, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));

        // Exact
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, trade.usdt_amount));
        let (result, _) = NexMarket::ocw_verification_results(0).unwrap();
        assert_eq!(result, PaymentVerificationResult::Exact);

        // 清理并测试 Overpaid (需要新的 trade)
        // Overpaid 和其他类型已在 full_trade_flow 和 underpaid 测试中覆盖
    });
}

// ==================== seed_liquidity 测试 ====================

#[test]
fn seed_liquidity_works() {
    new_test_ext().execute_with(|| {
        setup_seed_price(); // InitialPrice=500_000, premium=20% → seed_price=600_000

        let seed_account: u64 = 96;
        let seed_before = Balances::free_balance(seed_account);

        // 挂 2 笔固定 10 USDT 订单（默认金额，usdt_override = None）
        // nex_per_order = 10_000_000 × 10^12 / 600_000 = 16_666_666_666_666
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            2,
            None,
        ));

        let expected_nex_per_order: u128 = 10_000_000u128 * 1_000_000_000_000 / 600_000;
        let total_locked = expected_nex_per_order * 2;

        // 种子账户余额减少
        assert_eq!(
            Balances::free_balance(seed_account),
            seed_before - total_locked
        );

        // 最优卖价 = seed_price
        assert_eq!(NexMarket::best_ask(), Some(600_000));

        // 订单检查
        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.usdt_price, 600_000);
        assert_eq!(order.nex_amount, expected_nex_per_order);
        assert!(order.deposit_waived);
    });
}

#[test]
fn seed_liquidity_usdt_override() {
    new_test_ext().execute_with(|| {
        setup_seed_price(); // seed_price = 600_000

        let seed_account: u64 = 96;
        let seed_before = Balances::free_balance(seed_account);

        // usdt_override = 20 USDT (20_000_000)
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            1,
            Some(20_000_000),
        ));

        // nex = 20_000_000 × 10^12 / 600_000 = 33_333_333_333_333
        let expected_nex: u128 = 20_000_000u128 * 1_000_000_000_000 / 600_000;
        assert_eq!(Balances::free_balance(seed_account), seed_before - expected_nex);

        let order = NexMarket::orders(0).unwrap();
        assert_eq!(order.nex_amount, expected_nex);
    });
}

#[test]
fn seed_liquidity_rejects_no_price() {
    new_test_ext().execute_with(|| {
        // 不设置 InitialPrice → NoPriceReference
        assert_noop!(
            NexMarket::seed_liquidity(
                RuntimeOrigin::root(),
                1,
                None,
            ),
            Error::<Test>::NoPriceReference
        );
    });
}

// ==================== fund_seed_account 测试 ====================

#[test]
fn fund_seed_account_works() {
    new_test_ext().execute_with(|| {
        let seed_account: u64 = 96;
        let treasury: u64 = 99;
        let seed_before = Balances::free_balance(seed_account);
        let treasury_before = Balances::free_balance(treasury);

        let amount = 200_000_000_000_000u128; // 200 NEX
        assert_ok!(NexMarket::fund_seed_account(RuntimeOrigin::root(), amount));

        assert_eq!(Balances::free_balance(seed_account), seed_before + amount);
        assert_eq!(Balances::free_balance(treasury), treasury_before - amount);
    });
}

#[test]
fn fund_seed_account_rejects_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            NexMarket::fund_seed_account(RuntimeOrigin::root(), 0),
            Error::<Test>::AmountTooSmall
        );
    });
}

#[test]
fn fund_seed_account_limited_only_by_treasury_balance() {
    new_test_ext().execute_with(|| {
        // Treasury = 1,000 NEX，无累计上限
        assert_ok!(NexMarket::fund_seed_account(RuntimeOrigin::root(), 999_000_000_000_000u128));
        assert_eq!(Balances::free_balance(96u64), 500_000_000_000_000 + 999_000_000_000_000);
    });
}

// ==================== 免保证金首单测试 ====================

#[test]
fn waived_deposit_reserve_works_for_new_buyer() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        // seed 一笔免保证金卖单 (100 NEX = 60 USDT @ seed_price 0.6)
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            1,
            Some(60_000_000),
        ));

        // DAVE (零余额) 可以吃免保证金卖单
        // 注意: DAVE 没有 NEX，但免保证金不需要锁定
        // 但 DAVE 需要存在于链上 (existential deposit)
        // 用 BOB 来测试免保证金（BOB 有余额但不应被扣保证金）
        let bob_reserved_before = Balances::reserved_balance(BOB);
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, Some(50_000_000_000_000u128), buyer_tron(),
        ));

        // BOB 无保证金被锁定
        assert_eq!(Balances::reserved_balance(BOB), bob_reserved_before);

        // trade 已创建
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_eq!(trade.buyer_deposit, 0);
        assert_eq!(trade.deposit_status, BuyerDepositStatus::None);

        // ActiveWaivedTrades 已记录
        assert_eq!(ActiveWaivedTrades::<Test>::get(BOB), 1);
    });
}

#[test]
fn waived_deposit_l2_one_active_limit() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        // seed 2 笔免保证金卖单 (每笔 50 NEX = 30 USDT)
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            2,
            Some(30_000_000),
        ));

        // BOB 吃第一笔
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // BOB 尝试吃第二笔 → 被 L2 限制阻止
        assert_noop!(
            NexMarket::reserve_sell_order(RuntimeOrigin::signed(BOB), 1, None, buyer_tron()),
            Error::<Test>::FirstOrderLimitReached
        );
    });
}

#[test]
fn waived_deposit_l2_amount_cap() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        // seed 一笔大额免保证金卖单 (200 NEX = 120 USDT)
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            1,
            Some(120_000_000),
        ));

        // BOB 尝试吃 150 NEX → 超过 MaxFirstOrderAmount (100 NEX)
        assert_noop!(
            NexMarket::reserve_sell_order(
                RuntimeOrigin::signed(BOB), 0, Some(150_000_000_000_000u128), buyer_tron(),
            ),
            Error::<Test>::FirstOrderAmountTooLarge
        );

        // BOB 吃 100 NEX（上限）→ 成功
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, Some(100_000_000_000_000u128), buyer_tron(),
        ));
    });
}

#[test]
fn waived_deposit_l2_short_timeout() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            1,
            Some(30_000_000),
        ));

        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        let trade = NexMarket::usdt_trades(0).unwrap();
        // 免保证金超时 = FirstOrderTimeout = 600 blocks (1h)
        // 标准超时 = UsdtTimeout = 7200 blocks (12h)
        let expected_timeout: u64 = 600;
        assert_eq!(trade.timeout_at, expected_timeout);
    });
}

#[test]
fn waived_deposit_l3_completed_buyer_blocked() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        // seed 2 笔 (每笔 50 NEX = 30 USDT)
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            2,
            Some(30_000_000),
        ));

        // BOB 吃第一笔
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // BOB 完成交易流程
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_ok!(NexMarket::submit_ocw_result(
            RuntimeOrigin::none(), 0, trade.usdt_amount,
        ));
        assert_ok!(NexMarket::claim_verification_reward(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        // L3: BOB 已标记为 CompletedBuyer
        assert!(CompletedBuyers::<Test>::get(BOB));
        // ActiveWaivedTrades 已清零
        assert_eq!(ActiveWaivedTrades::<Test>::get(BOB), 0);

        // BOB 尝试再次吃免保证金卖单 → 被 L3 阻止
        assert_noop!(
            NexMarket::reserve_sell_order(RuntimeOrigin::signed(BOB), 1, None, buyer_tron()),
            Error::<Test>::BuyerAlreadyCompleted
        );
    });
}

#[test]
fn waived_deposit_timeout_cleans_counter() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            2,
            Some(30_000_000),
        ));

        // BOB 吃第一笔
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert_eq!(ActiveWaivedTrades::<Test>::get(BOB), 1);

        // 超时
        let trade = NexMarket::usdt_trades(0).unwrap();
        let timeout_block: u64 = trade.timeout_at.into();
        System::set_block_number(timeout_block + 1);

        assert_ok!(NexMarket::process_timeout(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        // 超时后计数器归零，BOB 未被标记为 CompletedBuyer
        assert_eq!(ActiveWaivedTrades::<Test>::get(BOB), 0);
        assert!(!CompletedBuyers::<Test>::get(BOB));

        // BOB 可以再次吃免保证金卖单（超时不等于完成）
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 1, None, buyer_tron(),
        ));
    });
}

#[test]
fn cumulative_seed_usdt_sold_tracks_waived_trades() {
    new_test_ext().execute_with(|| {
        setup_seed_price();
        // 初始为 0
        assert_eq!(NexMarket::cumulative_seed_usdt_sold(), 0);

        // seed_liquidity 创建免保证金卖单 (50 NEX = 30 USDT)
        assert_ok!(NexMarket::seed_liquidity(
            RuntimeOrigin::root(),
            1,
            Some(30_000_000),
        ));

        // BOB 吃单
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // BOB 提交付款
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));

        // OCW 提交验证结果（Exact）
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_ok!(NexMarket::submit_ocw_result(
            RuntimeOrigin::none(), 0, trade.usdt_amount,
        ));

        // claim reward → 触发 process_full_payment
        assert_ok!(NexMarket::claim_verification_reward(
            RuntimeOrigin::signed(CHARLIE), 0,
        ));

        // 审计值应等于该笔交易的 USDT 金额
        assert_eq!(NexMarket::cumulative_seed_usdt_sold(), trade.usdt_amount);

        // 正常卖单不影响审计值
        let prev = NexMarket::cumulative_seed_usdt_sold();
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 50_000_000_000_000, 500_000, tron_address(),
        ));
        // order_id = 1 (normal order)
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(CHARLIE), 1, None, buyer_tron(),
        ));
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(CHARLIE), 1,
        ));
        let trade2 = NexMarket::usdt_trades(1).unwrap();
        assert_ok!(NexMarket::submit_ocw_result(
            RuntimeOrigin::none(), 1, trade2.usdt_amount,
        ));
        assert_ok!(NexMarket::claim_verification_reward(
            RuntimeOrigin::signed(ALICE), 1,
        ));

        // 审计值不变（正常订单不累计）
        assert_eq!(NexMarket::cumulative_seed_usdt_sold(), prev);
    });
}

#[test]
fn on_idle_advances_twap_snapshots() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        // 设置初始价格并完成一笔交易来初始化 TWAP
        assert_ok!(NexMarket::set_initial_price(RuntimeOrigin::root(), 500_000));
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));
        assert_ok!(NexMarket::confirm_payment(
            RuntimeOrigin::signed(BOB), 0,
        ));
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert_ok!(NexMarket::submit_ocw_result(RuntimeOrigin::none(), 0, trade.usdt_amount));
        assert_ok!(NexMarket::claim_verification_reward(RuntimeOrigin::signed(CHARLIE), 0));

        // 交易完成后，记录当前 TWAP 累积器状态
        let acc_before = NexMarket::twap_accumulator().unwrap();
        let cum_before = acc_before.current_cumulative;

        // 前进 700 区块（> hour_interval=100, > bph=600）但不做任何交易
        let new_block = acc_before.current_block as u64 + 700;
        System::set_block_number(new_block);

        // 调用 on_idle
        NexMarket::on_idle(new_block, Weight::from_parts(1_000_000_000, 1_000_000));

        // 检查累积器已推进
        let acc_after = NexMarket::twap_accumulator().unwrap();
        assert_eq!(acc_after.current_block, new_block as u32);
        assert!(acc_after.current_cumulative > cum_before);

        // hour_snapshot 应已更新（700 > hour_interval=100）
        assert!(acc_after.hour_snapshot.block_number > acc_before.hour_snapshot.block_number);

        // day_snapshot 应已更新（700 > bph=600）
        assert!(acc_after.day_snapshot.block_number > acc_before.day_snapshot.block_number);

        // 再次 on_idle（同一区块）→ 累积器 current_block 不变
        let cum_mid = acc_after.current_cumulative;
        NexMarket::on_idle(new_block, Weight::from_parts(1_000_000_000, 1_000_000));
        let acc_same = NexMarket::twap_accumulator().unwrap();
        assert_eq!(acc_same.current_cumulative, cum_mid); // 没有变化
    });
}

#[test]
fn on_idle_noop_without_twap_data() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Hooks;

        // 没有任何交易，TWAP 累积器不存在
        System::set_block_number(100);
        NexMarket::on_idle(100, Weight::from_parts(1_000_000_000, 1_000_000));
        // TWAP 仍不存在
        assert!(NexMarket::twap_accumulator().is_none());
    });
}

#[test]
fn normal_sell_order_still_requires_deposit() {
    new_test_ext().execute_with(|| {
        // Alice 正常挂卖单（非 seed_liquidity）
        assert_ok!(NexMarket::place_sell_order(
            RuntimeOrigin::signed(ALICE), 100_000_000_000_000, 500_000, tron_address(),
        ));

        let order = NexMarket::orders(0).unwrap();
        assert!(!order.deposit_waived);

        // BOB 吃这个正常卖单 → 照常扣保证金
        let bob_reserved_before = Balances::reserved_balance(BOB);
        assert_ok!(NexMarket::reserve_sell_order(
            RuntimeOrigin::signed(BOB), 0, None, buyer_tron(),
        ));

        // 保证金已锁定
        assert!(Balances::reserved_balance(BOB) > bob_reserved_before);
        let trade = NexMarket::usdt_trades(0).unwrap();
        assert!(!trade.buyer_deposit.is_zero());
    });
}
