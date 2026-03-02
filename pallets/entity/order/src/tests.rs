use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_noop, assert_ok};
use pallet_entity_common::OrderStatus;

// ==================== place_order ====================

#[test]
fn place_order_physical_works() {
    new_test_ext().execute_with(|| {
        // Product 1 = Physical, shop 1, price 100
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER),
            1, // product_id
            2, // quantity
            Some(b"addr_cid".to_vec()),
            None,
            None,
            None,
        ));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.buyer, BUYER);
        assert_eq!(order.seller, SELLER);
        assert_eq!(order.quantity, 2);
        assert_eq!(order.unit_price, 100);
        assert_eq!(order.total_amount, 200); // 100 * 2
        assert_eq!(order.status, OrderStatus::Paid);
        assert!(order.requires_shipping);
        assert_eq!(order.escrow_id, 1);
    });
}

#[test]
fn place_order_digital_auto_completes() {
    new_test_ext().execute_with(|| {
        // Product 2 = Digital, shop 1, price 50
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER),
            2, // digital product
            1,
            None,
            None,
            None,
            None,
        ));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Completed);
        assert!(order.completed_at.is_some());
        assert!(!order.requires_shipping);
    });
}

#[test]
fn place_order_service_works() {
    new_test_ext().execute_with(|| {
        // Product 3 = Service, shop 1, price 200
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER),
            3,
            1,
            None,
            None,
            None,
            None,
        ));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Paid);
        assert!(!order.requires_shipping);
        assert_eq!(order.product_category, pallet_entity_common::ProductCategory::Service);
    });
}

#[test]
fn place_order_fails_zero_quantity() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 0, None, None, None, None),
            Error::<Test>::InvalidQuantity
        );
    });
}

#[test]
fn place_order_fails_product_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Transaction::place_order(RuntimeOrigin::signed(BUYER), 99, 1, None, None, None, None),
            Error::<Test>::ProductNotFound
        );
    });
}

#[test]
fn place_order_fails_buy_own_product() {
    new_test_ext().execute_with(|| {
        // SELLER owns shop 1, product 1 belongs to shop 1
        assert_noop!(
            Transaction::place_order(RuntimeOrigin::signed(SELLER), 1, 1, None, None, None, None),
            Error::<Test>::CannotBuyOwnProduct
        );
    });
}

// ==================== cancel_order ====================

#[test]
fn cancel_order_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_ok!(Transaction::cancel_order(RuntimeOrigin::signed(BUYER), 1));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Cancelled);

        // C3: on_order_cancelled was called
        assert_eq!(get_cancelled_orders(), vec![1]);
    });
}

#[test]
fn cancel_order_fails_digital() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 2, 1, None, None, None, None));

        assert_noop!(
            Transaction::cancel_order(RuntimeOrigin::signed(BUYER), 1),
            Error::<Test>::DigitalProductCannotCancel
        );
    });
}

#[test]
fn cancel_order_fails_not_buyer() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_noop!(
            Transaction::cancel_order(RuntimeOrigin::signed(BUYER2), 1),
            Error::<Test>::NotOrderBuyer
        );
    });
}

#[test]
fn cancel_order_fails_after_shipped() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track123".to_vec()));

        assert_noop!(
            Transaction::cancel_order(RuntimeOrigin::signed(BUYER), 1),
            Error::<Test>::CannotCancelOrder
        );
    });
}

// ==================== ship_order ====================

#[test]
fn ship_order_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track_cid".to_vec()));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Shipped);
        assert!(order.shipped_at.is_some());
        assert!(order.tracking_cid.is_some());
    });
}

#[test]
fn ship_order_fails_not_seller() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_noop!(
            Transaction::ship_order(RuntimeOrigin::signed(BUYER), 1, b"track".to_vec()),
            Error::<Test>::NotOrderSeller
        );
    });
}

// ==================== confirm_receipt ====================

#[test]
fn confirm_receipt_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track".to_vec()));
        assert_ok!(Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Completed);
        assert!(order.completed_at.is_some());

        // Check stats
        let stats = Transaction::order_stats();
        assert_eq!(stats.completed_orders, 1);
    });
}

#[test]
fn confirm_receipt_fails_not_shipped() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_noop!(
            Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1),
            Error::<Test>::InvalidOrderStatus
        );
    });
}

// ==================== request_refund / approve_refund ====================

#[test]
fn request_refund_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_ok!(Transaction::request_refund(
            RuntimeOrigin::signed(BUYER), 1, b"reason".to_vec()
        ));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Disputed);
    });
}

#[test]
fn request_refund_fails_digital() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 2, 1, None, None, None, None));

        assert_noop!(
            Transaction::request_refund(RuntimeOrigin::signed(BUYER), 1, b"reason".to_vec()),
            Error::<Test>::DigitalProductCannotRefund
        );
    });
}

#[test]
fn approve_refund_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::request_refund(RuntimeOrigin::signed(BUYER), 1, b"reason".to_vec()));
        assert_ok!(Transaction::approve_refund(RuntimeOrigin::signed(SELLER), 1));

        let order = Transaction::orders(1).expect("order should exist");
        assert_eq!(order.status, OrderStatus::Refunded);

        // C3: on_order_cancelled was called
        assert_eq!(get_cancelled_orders(), vec![1]);
    });
}

#[test]
fn approve_refund_fails_not_seller() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::request_refund(RuntimeOrigin::signed(BUYER), 1, b"reason".to_vec()));

        assert_noop!(
            Transaction::approve_refund(RuntimeOrigin::signed(BUYER), 1),
            Error::<Test>::NotOrderSeller
        );
    });
}

// ==================== Service order flow ====================

#[test]
fn service_order_full_flow() {
    new_test_ext().execute_with(|| {
        // Product 3 = Service
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Paid);

        // Start service
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Shipped);
        assert!(order.service_started_at.is_some());

        // Complete service
        assert_ok!(Transaction::complete_service(RuntimeOrigin::signed(SELLER), 1));
        let order = Transaction::orders(1).unwrap();
        assert!(order.service_completed_at.is_some());

        // Confirm service
        assert_ok!(Transaction::confirm_service(RuntimeOrigin::signed(BUYER), 1));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);
    });
}

#[test]
fn start_service_fails_not_service_order() {
    new_test_ext().execute_with(|| {
        // Product 1 = Physical
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_noop!(
            Transaction::start_service(RuntimeOrigin::signed(SELLER), 1),
            Error::<Test>::NotServiceOrder
        );
    });
}

#[test]
fn confirm_service_fails_without_completion() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));

        // service_completed_at is None → should fail
        assert_noop!(
            Transaction::confirm_service(RuntimeOrigin::signed(BUYER), 1),
            Error::<Test>::InvalidOrderStatus
        );
    });
}

// ==================== Timeout processing ====================

#[test]
fn ship_timeout_auto_refunds() {
    new_test_ext().execute_with(|| {
        // Physical product, ShipTimeout = 100 blocks
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_eq!(Transaction::orders(1).unwrap().status, OrderStatus::Paid);

        // Advance to block 1 + 100 = 101
        run_to_block(101);

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Refunded);

        // C3: on_order_cancelled was called by timeout handler
        assert!(get_cancelled_orders().contains(&1));
    });
}

#[test]
fn confirm_timeout_auto_completes() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track".to_vec()));

        // shipped_at = 1, ConfirmTimeout = 200, expiry = 1 + 200 = 201
        run_to_block(201);

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);
    });
}

#[test]
fn service_timeout_auto_refunds() {
    new_test_ext().execute_with(|| {
        // Service product, ShipTimeout = 100
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        // Don't start service, wait for timeout
        run_to_block(101);

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Refunded);
    });
}

// ==================== OrderProvider trait ====================

#[test]
fn order_provider_trait_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::OrderProvider;

        assert!(!<Transaction as OrderProvider<u64, u64>>::order_exists(1));

        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert!(<Transaction as OrderProvider<u64, u64>>::order_exists(1));
        assert_eq!(<Transaction as OrderProvider<u64, u64>>::order_buyer(1), Some(BUYER));
        assert_eq!(<Transaction as OrderProvider<u64, u64>>::order_shop_id(1), Some(SHOP_1));
        assert!(!<Transaction as OrderProvider<u64, u64>>::is_order_completed(1));

        // Ship and confirm
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"t".to_vec()));
        assert_ok!(Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1));
        assert!(<Transaction as OrderProvider<u64, u64>>::is_order_completed(1));
    });
}

// ==================== Statistics ====================

#[test]
fn order_stats_tracking() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 2, Some(b"addr".to_vec()), None, None, None));

        let stats = Transaction::order_stats();
        assert_eq!(stats.total_orders, 2);
        assert_eq!(stats.completed_orders, 0);

        // Complete first order
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"t".to_vec()));
        assert_ok!(Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1));

        let stats = Transaction::order_stats();
        assert_eq!(stats.completed_orders, 1);
    });
}

// ==================== Platform fee ====================

#[test]
fn platform_fee_calculated_correctly() {
    new_test_ext().execute_with(|| {
        // Price 100 * qty 1 = 100, platform_fee = 100 * 200 / 10000 = 2
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.total_amount, 100);
        assert_eq!(order.platform_fee, 2);
    });
}

// ==================== H5: Service order expiry timing ====================

#[test]
fn service_in_progress_not_auto_completed_by_ship_timeout() {
    new_test_ext().execute_with(|| {
        // H5: Service order started but ShipTimeout fires → should NOT auto-complete
        // because service_completed_at is still None (service not done yet)
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        // Seller starts service at block 50 (before ShipTimeout at 101)
        System::set_block_number(50);
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Shipped);
        assert!(order.service_completed_at.is_none());

        // ShipTimeout = 100, original expiry at block 1+100 = 101
        // Run to block 101 — the ShipTimeout fires but service_completed_at is None
        run_to_block(101);

        // H5: Order should still be Shipped (not auto-completed)
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Shipped);
    });
}

#[test]
fn service_completed_auto_confirms_after_service_confirm_timeout() {
    new_test_ext().execute_with(|| {
        // Service order: start → complete → auto-confirm after ServiceConfirmTimeout
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        System::set_block_number(10);
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));

        System::set_block_number(20);
        assert_ok!(Transaction::complete_service(RuntimeOrigin::signed(SELLER), 1));
        // complete_service enqueues expiry at 20 + 150 = 170

        // At block 101, the ShipTimeout entry fires. service_completed_at IS set,
        // so the H5 check allows auto-complete. This is correct behavior:
        // the service is done, buyer hasn't confirmed, so auto-complete is fine.
        run_to_block(101);
        assert_eq!(Transaction::orders(1).unwrap().status, OrderStatus::Completed);
    });
}

// ==================== Audit fix regression tests ====================

// C2: Service order cannot use ship_order
#[test]
fn c2_ship_order_rejects_service_order() {
    new_test_ext().execute_with(|| {
        // Product 3 = Service
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        assert_noop!(
            Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track".to_vec()),
            Error::<Test>::ServiceOrderCannotShip
        );
    });
}

// C2: Service order cannot use confirm_receipt
#[test]
fn c2_confirm_receipt_rejects_service_order() {
    new_test_ext().execute_with(|| {
        // Product 3 = Service, use start_service to get to Shipped
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));

        assert_noop!(
            Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1),
            Error::<Test>::ServiceOrderCannotShip
        );
    });
}

// M5: request_refund rejects empty reason_cid
#[test]
fn m5_request_refund_rejects_empty_reason() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        assert_noop!(
            Transaction::request_refund(RuntimeOrigin::signed(BUYER), 1, b"".to_vec()),
            Error::<Test>::EmptyReasonCid
        );
    });
}

// M5: request_refund rejects too-long reason_cid
#[test]
fn m5_request_refund_rejects_long_reason() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));

        let long_cid = vec![b'x'; 65]; // MaxCidLength = 64
        assert_noop!(
            Transaction::request_refund(RuntimeOrigin::signed(BUYER), 1, long_cid),
            Error::<Test>::CidTooLong
        );
    });
}

// M6: Physical order requires shipping_cid
#[test]
fn m6_physical_order_requires_shipping_cid() {
    new_test_ext().execute_with(|| {
        // Product 1 = Physical, no shipping_cid → should fail
        assert_noop!(
            Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, None, None, None, None),
            Error::<Test>::ShippingCidRequired
        );

        // With shipping_cid → should succeed
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None
        ));
    });
}

// M6: Service order does NOT require shipping_cid
#[test]
fn m6_service_order_no_shipping_cid_ok() {
    new_test_ext().execute_with(|| {
        // Product 3 = Service, no shipping_cid → should succeed
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));
    });
}

// C1: ExpiryQueue partial processing preserves unprocessed entries
#[test]
fn c1_expiry_queue_preserves_unprocessed() {
    new_test_ext().execute_with(|| {
        // Place 3 physical orders — all expire at block 1 + 100 = 101
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"a".to_vec()), None, None, None));
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"b".to_vec()), None, None, None));
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"c".to_vec()), None, None, None));

        // Verify all 3 are in the expiry queue at block 101
        let queue = crate::ExpiryQueue::<Test>::get(101u64);
        assert_eq!(queue.len(), 3);

        // Process with very limited weight — only 1 order can be processed
        System::set_block_number(101);
        <Transaction as frame_support::traits::Hooks<u64>>::on_idle(
            101,
            frame_support::weights::Weight::from_parts(250_000_000, 100_000), // ~1 order
        );

        // Order 1 should be refunded, orders 2 and 3 should remain
        assert_eq!(Transaction::orders(1).unwrap().status, OrderStatus::Refunded);

        // Remaining orders should still be in the queue
        let queue_after = crate::ExpiryQueue::<Test>::get(101u64);
        assert!(queue_after.len() >= 1, "unprocessed orders should remain in queue");

        // Process remaining with enough weight
        <Transaction as frame_support::traits::Hooks<u64>>::on_idle(
            101,
            frame_support::weights::Weight::from_parts(10_000_000_000, 1_000_000),
        );

        // All should now be refunded
        assert_eq!(Transaction::orders(2).unwrap().status, OrderStatus::Refunded);
        assert_eq!(Transaction::orders(3).unwrap().status, OrderStatus::Refunded);

        // Queue should be empty now
        let queue_final = crate::ExpiryQueue::<Test>::get(101u64);
        assert!(queue_final.is_empty());
    });
}

// ============================================================================
// MemberHandler 集成测试
// ============================================================================

#[test]
fn order_complete_triggers_member_register_and_update_spent() {
    new_test_ext().execute_with(|| {
        // Digital product auto-completes → triggers do_complete_order
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER),
            2, // Digital, price 50
            1,
            None, None, None,
            None,
        ));

        // auto_register should have been called with (shop_id=1, buyer=1)
        let registered = get_member_registered();
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0], (SHOP_1, BUYER));

        // update_spent should have been called with (shop_id=1, buyer=1, amount=50, amount_usdt)
        let spent = get_member_spent();
        assert_eq!(spent.len(), 1);
        assert_eq!(spent[0].0, SHOP_1);
        assert_eq!(spent[0].1, BUYER);
        assert_eq!(spent[0].2, 50); // amount NEX
        // MockPricingProvider: 1 USDT/NEX → 50 NEX * 1_000_000 / 10^12
        // 注意: mock 中 Balance=u64, 50 * 1_000_000 / 10^12 = 0（精度截断）
        // 这在测试环境中是预期的，因为 mock Balance 不是 12 位精度
    });
}

#[test]
fn confirm_order_triggers_member_handler() {
    new_test_ext().execute_with(|| {
        // Physical order: place → ship → confirm → do_complete_order
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER),
            1, // Physical, price 100
            1,
            Some(b"addr".to_vec()), None, None,
            None,
        ));

        // place_order 不触发 do_complete_order
        assert!(get_member_registered().is_empty());
        assert!(get_member_spent().is_empty());

        // ship
        assert_ok!(Transaction::ship_order(
            RuntimeOrigin::signed(SELLER),
            1,
            b"tracking".to_vec(),
        ));

        // confirm → triggers do_complete_order
        assert_ok!(Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1));

        let registered = get_member_registered();
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0], (SHOP_1, BUYER));

        let spent = get_member_spent();
        assert_eq!(spent.len(), 1);
        assert_eq!(spent[0].0, SHOP_1);
        assert_eq!(spent[0].1, BUYER);
        assert_eq!(spent[0].2, 100); // amount NEX = price 100 * qty 1
    });
}

#[test]
fn auto_complete_timeout_triggers_member_handler() {
    new_test_ext().execute_with(|| {
        // Physical: place → ship → timeout auto-complete
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER),
            1, 1, Some(b"addr".to_vec()), None, None,
            None,
        ));
        assert_ok!(Transaction::ship_order(
            RuntimeOrigin::signed(SELLER),
            1,
            b"tracking".to_vec(),
        ));

        // 发货后不确认，等待 ConfirmTimeout(200) 自动完成
        run_to_block(202);

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);

        let registered = get_member_registered();
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0], (SHOP_1, BUYER));

        let spent = get_member_spent();
        assert_eq!(spent.len(), 1);
        assert_eq!(spent[0].2, 100);
    });
}

// ==================== Token payment tests ====================

use pallet_entity_common::PaymentAsset;

const ENTITY_1: u64 = 1; // shop_entity_id(SHOP_1) returns 1

#[test]
fn token_place_order_physical_works() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None,
            Some(PaymentAsset::EntityToken),
        ));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.payment_asset, PaymentAsset::EntityToken);
        assert_eq!(order.token_payment_amount, 100); // price 100 * qty 1
        assert_eq!(order.platform_fee, 0); // no platform fee for Token orders
        assert_eq!(order.status, OrderStatus::Paid);

        // Token reserved from buyer
        assert_eq!(get_token_balance(ENTITY_1, BUYER), 10_000 - 100);
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 100);
    });
}

#[test]
fn token_place_order_digital_auto_completes() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 2, 1, // Digital, price 50
            None, None, None,
            Some(PaymentAsset::EntityToken),
        ));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.payment_asset, PaymentAsset::EntityToken);
        assert_eq!(order.token_payment_amount, 50);
        assert_eq!(order.status, OrderStatus::Completed);

        // Token transferred from buyer's reserve to seller
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 0);
        assert_eq!(get_token_balance(ENTITY_1, SELLER), 50);

        // TokenCommissionHandler::on_token_order_completed was called
        let completed = get_token_completed_orders();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], (ENTITY_1, SHOP_1, 1, BUYER, 50, 0));
    });
}

#[test]
fn token_cancel_order_unreserves_tokens() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None,
            Some(PaymentAsset::EntityToken),
        ));
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 100);

        assert_ok!(Transaction::cancel_order(RuntimeOrigin::signed(BUYER), 1));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Cancelled);

        // Tokens unreserved back to buyer
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 0);
        assert_eq!(get_token_balance(ENTITY_1, BUYER), 10_000);

        // TokenCommissionHandler::on_token_order_cancelled was called
        assert_eq!(get_token_cancelled_orders(), vec![1]);
    });
}

#[test]
fn token_confirm_receipt_transfers_to_seller() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None,
            Some(PaymentAsset::EntityToken),
        ));
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track".to_vec()));
        assert_ok!(Transaction::confirm_receipt(RuntimeOrigin::signed(BUYER), 1));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);

        // Tokens moved from buyer reserved → seller free
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 0);
        assert_eq!(get_token_balance(ENTITY_1, SELLER), 100);

        // TokenCommissionHandler called
        let completed = get_token_completed_orders();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], (ENTITY_1, SHOP_1, 1, BUYER, 100, 0));
    });
}

#[test]
fn token_approve_refund_unreserves_tokens() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None,
            Some(PaymentAsset::EntityToken),
        ));
        assert_ok!(Transaction::request_refund(RuntimeOrigin::signed(BUYER), 1, b"reason".to_vec()));
        assert_ok!(Transaction::approve_refund(RuntimeOrigin::signed(SELLER), 1));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Refunded);

        // Tokens returned to buyer
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 0);
        assert_eq!(get_token_balance(ENTITY_1, BUYER), 10_000);

        // TokenCommissionHandler::on_token_order_cancelled was called
        assert_eq!(get_token_cancelled_orders(), vec![1]);
    });
}

#[test]
fn token_ship_timeout_auto_refunds() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None,
            Some(PaymentAsset::EntityToken),
        ));

        run_to_block(101); // ShipTimeout = 100

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Refunded);
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 0);
        assert_eq!(get_token_balance(ENTITY_1, BUYER), 10_000);
        assert!(get_token_cancelled_orders().contains(&1));
    });
}

#[test]
fn token_confirm_timeout_auto_completes() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None,
            Some(PaymentAsset::EntityToken),
        ));
        assert_ok!(Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"track".to_vec()));

        run_to_block(201); // ConfirmTimeout = 200

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);
        assert_eq!(get_token_reserved(ENTITY_1, BUYER), 0);
        assert_eq!(get_token_balance(ENTITY_1, SELLER), 100);
    });
}

#[test]
fn token_place_order_fails_token_not_enabled() {
    new_test_ext().execute_with(|| {
        // Token NOT enabled for entity
        set_token_balance(ENTITY_1, BUYER, 10_000);

        assert_noop!(
            Transaction::place_order(
                RuntimeOrigin::signed(BUYER), 1, 1,
                Some(b"addr".to_vec()), None, None,
                Some(PaymentAsset::EntityToken),
            ),
            Error::<Test>::EntityTokenNotEnabled
        );
    });
}

#[test]
fn token_place_order_fails_insufficient_balance() {
    new_test_ext().execute_with(|| {
        set_token_enabled(ENTITY_1, true);
        set_token_balance(ENTITY_1, BUYER, 50); // price is 100, insufficient

        assert_noop!(
            Transaction::place_order(
                RuntimeOrigin::signed(BUYER), 1, 1,
                Some(b"addr".to_vec()), None, None,
                Some(PaymentAsset::EntityToken),
            ),
            Error::<Test>::InsufficientTokenBalance
        );
    });
}

#[test]
fn token_place_order_none_defaults_to_native() {
    new_test_ext().execute_with(|| {
        // payment_asset = None should default to Native
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 1, 1,
            Some(b"addr".to_vec()), None, None, None,
        ));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.payment_asset, PaymentAsset::Native);
        assert_eq!(order.token_payment_amount, 0);
        assert_eq!(order.platform_fee, 2); // 100 * 200 / 10000 = 2
    });
}

// ==================== Audit regression tests ====================

// H2: stock=0 应拒绝购买（不再当作"无限库存"）
#[test]
fn h2_zero_stock_rejects_purchase() {
    new_test_ext().execute_with(|| {
        set_product_stock(1, 0); // Physical product, stock=0
        assert_noop!(
            Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None),
            Error::<Test>::InsufficientStock
        );
    });
}

// H3: ship_order 拒绝空 tracking_cid
#[test]
fn h3_ship_order_rejects_empty_tracking_cid() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 1, 1, Some(b"addr".to_vec()), None, None, None));
        assert_noop!(
            Transaction::ship_order(RuntimeOrigin::signed(SELLER), 1, b"".to_vec()),
            Error::<Test>::EmptyTrackingCid
        );
    });
}

// H4: start_service 写入 ExpiryQueue，服务超时后自动退款
#[test]
fn h4_service_start_timeout_auto_refunds() {
    new_test_ext().execute_with(|| {
        // Product 3 = Service, ShipTimeout=100, ServiceConfirmTimeout=150
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        // Start service at block 10
        System::set_block_number(10);
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Shipped);
        assert!(order.service_started_at.is_some());

        // ServiceConfirmTimeout entry at block 10+150=160
        let queue = crate::ExpiryQueue::<Test>::get(160u64);
        assert!(queue.contains(&1), "start_service should enqueue to ExpiryQueue");

        // At block 101 (ShipTimeout from place_order), service is still in progress
        // and 101 < 10+150=160, so should NOT be refunded
        run_to_block(101);
        assert_eq!(Transaction::orders(1).unwrap().status, OrderStatus::Shipped);

        // At block 160 (ServiceConfirmTimeout from start_service), auto-refund
        run_to_block(160);
        assert_eq!(Transaction::orders(1).unwrap().status, OrderStatus::Refunded);
    });
}

// H4: 服务正常完成后确认不受影响
#[test]
fn h4_service_normal_flow_unaffected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Transaction::place_order(RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None));

        System::set_block_number(10);
        assert_ok!(Transaction::start_service(RuntimeOrigin::signed(SELLER), 1));

        System::set_block_number(20);
        assert_ok!(Transaction::complete_service(RuntimeOrigin::signed(SELLER), 1));

        System::set_block_number(25);
        assert_ok!(Transaction::confirm_service(RuntimeOrigin::signed(BUYER), 1));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);
    });
}

// M3: do_complete_order 的 reward_on_purchase 使用正确的 entity_id
#[test]
fn m3_reward_uses_resolved_entity_id() {
    new_test_ext().execute_with(|| {
        // Digital product auto-completes → do_complete_order → reward_on_purchase
        // This test verifies it doesn't panic and completes successfully
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 2, 1, None, None, None, None,
        ));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Completed);
    });
}

// ==================== Audit Round 3 回归测试 ====================

// C1: EntityToken 订单可以成功申请退款（修复前会因 Escrow::set_disputed 失败）
#[test]
fn c1_token_order_request_refund_works() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_token_enabled(entity_id, true);
        set_token_balance(entity_id, BUYER, 10_000);

        // 用 EntityToken 下单（服务类产品不需要 shipping_cid）
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 3, 1,
            None, None, None,
            Some(PaymentAsset::EntityToken),
        ));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.payment_asset, PaymentAsset::EntityToken);
        assert_eq!(order.status, OrderStatus::Paid);

        // EntityToken 订单申请退款（修复前此处会因 NoLock 失败）
        assert_ok!(Transaction::request_refund(
            RuntimeOrigin::signed(BUYER), 1, b"reason_cid".to_vec(),
        ));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Disputed);
    });
}

// C1: Native 订单退款仍通过 Escrow::set_disputed（行为不变）
#[test]
fn c1_native_order_request_refund_still_uses_escrow() {
    new_test_ext().execute_with(|| {
        // 服务类产品不需要 shipping_cid
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 3, 1, None, None, None, None,
        ));
        assert_ok!(Transaction::request_refund(
            RuntimeOrigin::signed(BUYER), 1, b"reason_cid".to_vec(),
        ));
        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Disputed);
    });
}

// C1: EntityToken 订单退款后卖家可同意退款，Token 被 unreserve
#[test]
fn c1_token_order_approve_refund_unreserves_tokens() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        set_token_enabled(entity_id, true);
        set_token_balance(entity_id, BUYER, 10_000);

        // 服务类产品不需要 shipping_cid
        assert_ok!(Transaction::place_order(
            RuntimeOrigin::signed(BUYER), 3, 1,
            None, None, None,
            Some(PaymentAsset::EntityToken),
        ));

        // 买家申请退款
        assert_ok!(Transaction::request_refund(
            RuntimeOrigin::signed(BUYER), 1, b"reason".to_vec(),
        ));

        // 卖家同意退款
        assert_ok!(Transaction::approve_refund(
            RuntimeOrigin::signed(SELLER), 1,
        ));

        let order = Transaction::orders(1).unwrap();
        assert_eq!(order.status, OrderStatus::Refunded);

        // Token 已退回买家（unreserved）
        let reserved = get_token_reserved(entity_id, BUYER);
        assert_eq!(reserved, 0);
    });
}
