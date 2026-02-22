//! # P2P Pallet 单元测试
//!
//! 覆盖 Buy-side、Sell-side、KYC、Hooks 核心流程。

use crate::mock::*;
use crate::pallet::*;
use crate::types::*;
use frame_support::{assert_ok, assert_noop};
use sp_core::H256;

// ==================== 1. Mock 环境验证 ====================

#[test]
fn mock_environment_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(System::block_number(), 0);
        assert_eq!(Balances::free_balance(BUYER), 1_000_000_000_000_000);
        assert_eq!(Balances::free_balance(BUYER2), 1_000_000_000_000_000);
        assert_eq!(Balances::free_balance(MAKER_ACCOUNT), 10_000_000_000_000_000);
    });
}

// ==================== 2. 存储初始状态 ====================

#[test]
fn initial_storage_state() {
    new_test_ext().execute_with(|| {
        assert_eq!(P2pTrading::next_buy_order_id(), 0);
        assert_eq!(P2pTrading::next_sell_order_id(), 0);
        assert!(P2pTrading::buy_orders(0).is_none());
        assert!(P2pTrading::sell_orders(0).is_none());
        assert!(P2pTrading::buyer_order_list(BUYER).is_empty());
        assert!(!P2pTrading::has_first_purchased(BUYER));
        assert_eq!(P2pTrading::total_deposit_pool_balance(), 0u128);

        let kyc_config = KycConfigStore::<Test>::get();
        assert!(!kyc_config.enabled);
    });
}

// ==================== 3. 数据类型快照 ====================

#[test]
fn buy_order_state_variants() {
    let states = vec![
        BuyOrderState::Created,
        BuyOrderState::PaidOrCommitted,
        BuyOrderState::Released,
        BuyOrderState::Refunded,
        BuyOrderState::Canceled,
        BuyOrderState::Disputed,
        BuyOrderState::Closed,
        BuyOrderState::Expired,
    ];
    assert_eq!(states.len(), 8);
}

#[test]
fn sell_order_status_variants() {
    let statuses = vec![
        SellOrderStatus::Pending,
        SellOrderStatus::AwaitingVerification,
        SellOrderStatus::Completed,
        SellOrderStatus::VerificationFailed,
        SellOrderStatus::UserReported,
        SellOrderStatus::Arbitrating,
        SellOrderStatus::ArbitrationApproved,
        SellOrderStatus::ArbitrationRejected,
        SellOrderStatus::Refunded,
        SellOrderStatus::SeverelyDisputed,
    ];
    assert_eq!(statuses.len(), 10);
}

#[test]
fn deposit_status_variants() {
    let statuses = vec![
        DepositStatus::None,
        DepositStatus::Locked,
        DepositStatus::Released,
        DepositStatus::Forfeited,
        DepositStatus::PartiallyForfeited,
    ];
    assert_eq!(statuses.len(), 5);
}

#[test]
fn kyc_types_snapshot() {
    let results = vec![
        KycVerificationResult::Passed,
        KycVerificationResult::Failed(KycFailureReason::IdentityNotSet),
        KycVerificationResult::Exempted,
        KycVerificationResult::Skipped,
    ];
    assert_eq!(results.len(), 4);

    assert_eq!(KycFailureReason::IdentityNotSet.to_code(), 0);
    assert_eq!(KycFailureReason::NoValidJudgement.to_code(), 1);
    assert_eq!(KycFailureReason::InsufficientLevel.to_code(), 2);
    assert_eq!(KycFailureReason::QualityIssue.to_code(), 3);
}

#[test]
fn p2p_permanent_stats_default() {
    let stats = P2pPermanentStats::default();
    assert_eq!(stats.total_buy_orders, 0);
    assert_eq!(stats.completed_buy_orders, 0);
    assert_eq!(stats.cancelled_buy_orders, 0);
    assert_eq!(stats.buy_volume, 0);
    assert_eq!(stats.total_sell_orders, 0);
    assert_eq!(stats.completed_sell_orders, 0);
    assert_eq!(stats.refunded_sell_orders, 0);
    assert_eq!(stats.sell_volume, 0);
}

// ==================== 4. Buy-side 流程测试 ====================

/// 辅助：创建首购订单
fn create_first_purchase_order() -> u64 {
    assert_ok!(P2pTrading::create_first_purchase(
        RuntimeOrigin::signed(BUYER),
        MAKER_ID,
        H256::repeat_byte(0x01),
        H256::repeat_byte(0x02),
    ));
    0 // first order id
}

#[test]
fn create_first_purchase_works() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();

        // 验证订单已创建
        let order = P2pTrading::buy_orders(order_id).expect("order should exist");
        assert_eq!(order.maker_id, MAKER_ID);
        assert_eq!(order.taker, BUYER);
        assert_eq!(order.state, BuyOrderState::Created);
        assert!(order.is_first_purchase);
        assert_eq!(order.deposit_status, DepositStatus::None);

        // NextBuyOrderId 已递增
        assert_eq!(P2pTrading::next_buy_order_id(), 1);

        // 买家订单列表中有记录
        assert_eq!(P2pTrading::buyer_order_list(BUYER).len(), 1);

        // 做市商首购计数增加
        assert_eq!(P2pTrading::maker_first_purchase_count(MAKER_ID), 1);
    });
}

#[test]
fn create_first_purchase_twice_fails() {
    new_test_ext().execute_with(|| {
        // 先标记为已首购
        HasFirstPurchased::<Test>::insert(BUYER, true);

        assert_noop!(
            P2pTrading::create_first_purchase(
                RuntimeOrigin::signed(BUYER),
                MAKER_ID,
                H256::repeat_byte(0x01),
                H256::repeat_byte(0x02),
            ),
            Error::<Test>::AlreadyFirstPurchased
        );
    });
}

#[test]
fn create_first_purchase_invalid_maker_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            P2pTrading::create_first_purchase(
                RuntimeOrigin::signed(BUYER),
                99, // non-existent maker
                H256::repeat_byte(0x01),
                H256::repeat_byte(0x02),
            ),
            Error::<Test>::MakerNotFound
        );
    });
}

#[test]
fn buy_flow_first_purchase_mark_paid_release() {
    new_test_ext().execute_with(|| {
        // 1. 创建首购
        let order_id = create_first_purchase_order();

        // 2. 买家标记已付款
        assert_ok!(P2pTrading::mark_paid(
            RuntimeOrigin::signed(BUYER),
            order_id,
            None,
        ));
        let order = P2pTrading::buy_orders(order_id).unwrap();
        assert_eq!(order.state, BuyOrderState::PaidOrCommitted);

        // 3. 做市商释放 NEX
        assert_ok!(P2pTrading::release_nex(
            RuntimeOrigin::signed(MAKER_ACCOUNT),
            order_id,
        ));
        let order = P2pTrading::buy_orders(order_id).unwrap();
        assert_eq!(order.state, BuyOrderState::Released);
        assert!(order.completed_at.is_some());

        // 买家完成计数
        assert_eq!(P2pTrading::buyer_completed_order_count(BUYER), 1);

        // 首购状态已更新
        assert!(P2pTrading::has_first_purchased(BUYER));
    });
}

#[test]
fn mark_paid_wrong_buyer_fails() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();

        assert_noop!(
            P2pTrading::mark_paid(
                RuntimeOrigin::signed(BUYER2), // wrong buyer
                order_id,
                None,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn release_nex_wrong_maker_fails() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();
        assert_ok!(P2pTrading::mark_paid(
            RuntimeOrigin::signed(BUYER),
            order_id,
            None,
        ));

        assert_noop!(
            P2pTrading::release_nex(
                RuntimeOrigin::signed(BUYER), // not maker
                order_id,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn release_nex_wrong_state_fails() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();
        // Still in Created state, not PaidOrCommitted
        assert_noop!(
            P2pTrading::release_nex(
                RuntimeOrigin::signed(MAKER_ACCOUNT),
                order_id,
            ),
            Error::<Test>::InvalidBuyOrderStatus
        );
    });
}

#[test]
fn cancel_buy_order_by_buyer() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();

        assert_ok!(P2pTrading::cancel_buy_order(
            RuntimeOrigin::signed(BUYER),
            order_id,
        ));
        let order = P2pTrading::buy_orders(order_id).unwrap();
        assert_eq!(order.state, BuyOrderState::Canceled);
        assert!(order.completed_at.is_some());
    });
}

#[test]
fn cancel_buy_order_by_maker() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();

        assert_ok!(P2pTrading::cancel_buy_order(
            RuntimeOrigin::signed(MAKER_ACCOUNT),
            order_id,
        ));
        let order = P2pTrading::buy_orders(order_id).unwrap();
        assert_eq!(order.state, BuyOrderState::Canceled);
    });
}

#[test]
fn cancel_buy_order_wrong_state_fails() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();
        // Move to PaidOrCommitted
        assert_ok!(P2pTrading::mark_paid(
            RuntimeOrigin::signed(BUYER),
            order_id,
            None,
        ));

        // Cannot cancel in PaidOrCommitted state
        assert_noop!(
            P2pTrading::cancel_buy_order(
                RuntimeOrigin::signed(BUYER),
                order_id,
            ),
            Error::<Test>::InvalidBuyOrderStatus
        );
    });
}

#[test]
fn dispute_buy_order_works() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();
        assert_ok!(P2pTrading::mark_paid(
            RuntimeOrigin::signed(BUYER),
            order_id,
            None,
        ));

        // 买家发起争议
        assert_ok!(P2pTrading::dispute_buy_order(
            RuntimeOrigin::signed(BUYER),
            order_id,
        ));
        let order = P2pTrading::buy_orders(order_id).unwrap();
        assert_eq!(order.state, BuyOrderState::Disputed);

        // 争议记录已创建
        let dispute = P2pTrading::buy_disputes(order_id).expect("dispute should exist");
        assert_eq!(dispute.order_id, order_id);
        assert_eq!(dispute.status, BuyDisputeStatus::WaitingMakerResponse);
    });
}

#[test]
fn dispute_buy_order_wrong_state_fails() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();
        // Still Created, not PaidOrCommitted
        assert_noop!(
            P2pTrading::dispute_buy_order(
                RuntimeOrigin::signed(BUYER),
                order_id,
            ),
            Error::<Test>::InvalidBuyOrderStatus
        );
    });
}

#[test]
fn mark_paid_with_tron_tx_hash() {
    new_test_ext().execute_with(|| {
        let order_id = create_first_purchase_order();
        let tx_hash = vec![0xABu8; 32];

        assert_ok!(P2pTrading::mark_paid(
            RuntimeOrigin::signed(BUYER),
            order_id,
            Some(tx_hash.clone()),
        ));

        // TRON tx hash 已记录
        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(&tx_hash);
        let h256 = H256::from(hash_bytes);
        assert!(BuyTronTxUsed::<Test>::contains_key(h256));

        // 重复使用同一 tx hash 应失败
        // 先创建另一个订单
        assert_ok!(P2pTrading::create_first_purchase(
            RuntimeOrigin::signed(BUYER2),
            MAKER_ID,
            H256::repeat_byte(0x03),
            H256::repeat_byte(0x04),
        ));
        assert_noop!(
            P2pTrading::mark_paid(
                RuntimeOrigin::signed(BUYER2),
                1, // second order
                Some(tx_hash),
            ),
            Error::<Test>::TronTxHashAlreadyUsed
        );
    });
}

// ==================== 5. Sell-side 流程测试 ====================

/// 辅助：创建 Sell 订单
fn create_sell_order() -> u64 {
    let nex_amount: u128 = 10_000_000_000_000; // 10 NEX
    let usdt_address = vec![0x41u8; 34]; // mock TRON address
    assert_ok!(P2pTrading::create_sell_order(
        RuntimeOrigin::signed(BUYER),
        MAKER_ID,
        nex_amount,
        usdt_address,
    ));
    0 // first sell_id
}

#[test]
fn create_sell_order_works() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();

        let record = P2pTrading::sell_orders(sell_id).expect("sell order should exist");
        assert_eq!(record.sell_id, 0);
        assert_eq!(record.maker_id, MAKER_ID);
        assert_eq!(record.user, BUYER);
        assert_eq!(record.status, SellOrderStatus::Pending);
        assert_eq!(record.nex_amount, 10_000_000_000_000u128);

        assert_eq!(P2pTrading::next_sell_order_id(), 1);
        assert_eq!(P2pTrading::user_sell_list(BUYER).len(), 1);
        assert_eq!(P2pTrading::maker_sell_list(MAKER_ID).len(), 1);
    });
}

#[test]
fn create_sell_order_below_min_fails() {
    new_test_ext().execute_with(|| {
        let small_amount: u128 = 100; // way below MinSellAmount
        assert_noop!(
            P2pTrading::create_sell_order(
                RuntimeOrigin::signed(BUYER),
                MAKER_ID,
                small_amount,
                vec![0x41u8; 34],
            ),
            Error::<Test>::SellAmountTooLow
        );
    });
}

#[test]
fn create_sell_order_invalid_maker_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            P2pTrading::create_sell_order(
                RuntimeOrigin::signed(BUYER),
                99, // non-existent
                10_000_000_000_000u128,
                vec![0x41u8; 34],
            ),
            Error::<Test>::MakerNotFound
        );
    });
}

#[test]
fn mark_sell_complete_works() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();
        let tx_hash = vec![0xFFu8; 64]; // mock TRC20 tx hash

        assert_ok!(P2pTrading::mark_sell_complete(
            RuntimeOrigin::signed(MAKER_ACCOUNT),
            sell_id,
            tx_hash,
        ));

        let record = P2pTrading::sell_orders(sell_id).unwrap();
        assert_eq!(record.status, SellOrderStatus::AwaitingVerification);
        assert!(record.trc20_tx_hash.is_some());

        // 验证请求已创建
        let vr = P2pTrading::sell_pending_verifications(sell_id)
            .expect("verification request should exist");
        assert_eq!(vr.sell_id, sell_id);
    });
}

#[test]
fn mark_sell_complete_not_maker_fails() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();

        assert_noop!(
            P2pTrading::mark_sell_complete(
                RuntimeOrigin::signed(BUYER), // not the maker
                sell_id,
                vec![0xFFu8; 64],
            ),
            Error::<Test>::NotMaker
        );
    });
}

#[test]
fn confirm_sell_verification_success() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();
        assert_ok!(P2pTrading::mark_sell_complete(
            RuntimeOrigin::signed(MAKER_ACCOUNT),
            sell_id,
            vec![0xFFu8; 64],
        ));

        // VerificationOrigin (Root) confirms
        assert_ok!(P2pTrading::confirm_sell_verification(
            RuntimeOrigin::root(),
            sell_id,
            true,
            None,
        ));

        let record = P2pTrading::sell_orders(sell_id).unwrap();
        assert_eq!(record.status, SellOrderStatus::Completed);
        assert!(record.completed_at.is_some());

        // 验证请求已清除
        assert!(P2pTrading::sell_pending_verifications(sell_id).is_none());
    });
}

#[test]
fn confirm_sell_verification_failure() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();
        assert_ok!(P2pTrading::mark_sell_complete(
            RuntimeOrigin::signed(MAKER_ACCOUNT),
            sell_id,
            vec![0xFFu8; 64],
        ));

        assert_ok!(P2pTrading::confirm_sell_verification(
            RuntimeOrigin::root(),
            sell_id,
            false,
            Some(b"amount mismatch".to_vec()),
        ));

        let record = P2pTrading::sell_orders(sell_id).unwrap();
        // 验证失败后状态变为 Pending（做市商需重新提交）或 SeverelyDisputed
        // 根据实现可能有不同处理
        assert_ne!(record.status, SellOrderStatus::Completed);
    });
}

#[test]
fn report_sell_works() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();

        assert_ok!(P2pTrading::report_sell(
            RuntimeOrigin::signed(BUYER),
            sell_id,
        ));

        let record = P2pTrading::sell_orders(sell_id).unwrap();
        assert_eq!(record.status, SellOrderStatus::UserReported);
    });
}

#[test]
fn report_sell_not_user_fails() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();

        assert_noop!(
            P2pTrading::report_sell(
                RuntimeOrigin::signed(BUYER2), // not the user
                sell_id,
            ),
            Error::<Test>::NotSellUser
        );
    });
}

#[test]
fn report_sell_wrong_status_fails() {
    new_test_ext().execute_with(|| {
        let sell_id = create_sell_order();
        // Move to AwaitingVerification
        assert_ok!(P2pTrading::mark_sell_complete(
            RuntimeOrigin::signed(MAKER_ACCOUNT),
            sell_id,
            vec![0xFFu8; 64],
        ));

        assert_noop!(
            P2pTrading::report_sell(
                RuntimeOrigin::signed(BUYER),
                sell_id,
            ),
            Error::<Test>::InvalidSellStatus
        );
    });
}

// ==================== 6. KYC 管理测试 ====================

#[test]
fn enable_kyc_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(P2pTrading::enable_kyc_requirement(
            RuntimeOrigin::root(),
            2, // min_judgment_priority
        ));

        let config = KycConfigStore::<Test>::get();
        assert!(config.enabled);
        assert_eq!(config.min_judgment_priority, 2);
    });
}

#[test]
fn disable_kyc_works() {
    new_test_ext().execute_with(|| {
        // Enable first
        assert_ok!(P2pTrading::enable_kyc_requirement(RuntimeOrigin::root(), 2));
        // Then disable
        assert_ok!(P2pTrading::disable_kyc_requirement(RuntimeOrigin::root()));

        let config = KycConfigStore::<Test>::get();
        assert!(!config.enabled);
    });
}

#[test]
fn kyc_non_root_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            P2pTrading::enable_kyc_requirement(
                RuntimeOrigin::signed(BUYER),
                2,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn update_min_judgment_level_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(P2pTrading::enable_kyc_requirement(RuntimeOrigin::root(), 1));
        assert_ok!(P2pTrading::update_min_judgment_level(RuntimeOrigin::root(), 3));

        let config = KycConfigStore::<Test>::get();
        assert_eq!(config.min_judgment_priority, 3);
    });
}

// ==================== 7. KYC verify_kyc 查询测试 ====================

#[test]
fn verify_kyc_skipped_when_disabled() {
    new_test_ext().execute_with(|| {
        // KYC 默认禁用
        let result = P2pTrading::verify_kyc(&BUYER);
        assert_eq!(result, KycVerificationResult::Skipped);
    });
}

#[test]
fn verify_kyc_exempted() {
    new_test_ext().execute_with(|| {
        assert_ok!(P2pTrading::enable_kyc_requirement(RuntimeOrigin::root(), 2));
        // 添加豁免
        KycExemptAccounts::<Test>::insert(BUYER, ());

        let result = P2pTrading::verify_kyc(&BUYER);
        assert_eq!(result, KycVerificationResult::Exempted);

        // is_kyc_exempt helper
        assert!(P2pTrading::is_kyc_exempt(&BUYER));
        assert!(!P2pTrading::is_kyc_exempt(&BUYER2));
    });
}

// ==================== 8. 配置常量快照 ====================

#[test]
fn config_constants_snapshot() {
    new_test_ext().execute_with(|| {
        use frame_support::traits::Get;

        // Buy-side
        assert_eq!(<Test as crate::pallet::Config>::BuyOrderTimeout::get(), 3_600_000);
        assert_eq!(<Test as crate::pallet::Config>::EvidenceWindow::get(), 86_400_000);
        assert_eq!(<Test as crate::pallet::Config>::MaxOrderUsdAmount::get(), 200_000_000);
        assert_eq!(<Test as crate::pallet::Config>::MinOrderUsdAmount::get(), 20_000_000);
        assert_eq!(<Test as crate::pallet::Config>::FirstPurchaseUsdAmount::get(), 10_000_000);
        assert_eq!(<Test as crate::pallet::Config>::DepositRate::get(), 1000);
        assert_eq!(<Test as crate::pallet::Config>::CancelPenaltyRate::get(), 3000);

        // Sell-side
        assert_eq!(<Test as crate::pallet::Config>::SellTimeoutBlocks::get(), 100);
        assert_eq!(<Test as crate::pallet::Config>::VerificationTimeoutBlocks::get(), 50);
        assert_eq!(<Test as crate::pallet::Config>::SellFeeRateBps::get(), 30);
    });
}

// ==================== 9. L2 归档结构快照 ====================

#[test]
fn archived_order_l2_default() {
    let l2 = ArchivedOrderL2::default();
    assert_eq!(l2.id, 0);
    assert_eq!(l2.status, 0);
    assert_eq!(l2.year_month, 0);
    assert_eq!(l2.amount_tier, 0);
    assert_eq!(l2.flags, 0);
}
