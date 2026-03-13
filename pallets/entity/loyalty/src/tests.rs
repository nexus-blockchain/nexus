//! Tests for pallet-entity-loyalty — 回滚 + 游标清理

use crate::{mock::*, pallet::*, CleanupPhase};
use frame_support::{assert_noop, assert_ok};
use pallet_entity_common::LoyaltyWritePort;

// ============================================================================
// 回滚购物余额
// ============================================================================

#[test]
fn rollback_shopping_balance_restores_balance_and_nex() {
    new_test_ext().execute_with(|| {
        // 1. 先 credit 100 给 BUYER（模拟 commission 结算）
        assert_ok!(Loyalty::do_credit_shopping_balance(ENTITY_ID, &BUYER, 100));
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 100);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 100);

        // 2. consume 60（NEX 从 entity → buyer）
        assert_ok!(Loyalty::do_consume_shopping_balance(ENTITY_ID, &BUYER, 60));
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 40);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 40);

        let buyer_bal_after_consume = balance_of(BUYER);
        let entity_bal_after_consume = balance_of(ENTITY_ACCOUNT);

        // 3. rollback 60（NEX 从 buyer → entity）
        assert_ok!(Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 60));

        // 验证记账恢复
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 100);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 100);

        // 验证 NEX 转回
        assert_eq!(balance_of(BUYER), buyer_bal_after_consume - 60);
        assert_eq!(balance_of(ENTITY_ACCOUNT), entity_bal_after_consume + 60);

        // 验证事件
        System::assert_has_event(
            Event::<Test>::ShoppingBalanceRolledBack {
                entity_id: ENTITY_ID,
                account: BUYER,
                amount: 60,
            }
            .into(),
        );
    });
}

#[test]
fn rollback_shopping_balance_zero_is_noop() {
    new_test_ext().execute_with(|| {
        let buyer_bal = balance_of(BUYER);
        assert_ok!(Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 0));
        // 余额不变、无事件
        assert_eq!(balance_of(BUYER), buyer_bal);
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 0);
        assert!(System::events().is_empty() || !System::events().iter().any(|e| {
            matches!(e.event, RuntimeEvent::Loyalty(Event::ShoppingBalanceRolledBack { .. }))
        }));
    });
}

#[test]
fn rollback_shopping_balance_fails_insufficient_buyer_funds() {
    new_test_ext().execute_with(|| {
        // BUYER 只有 100_000，试图 rollback 200_000
        assert_noop!(
            Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 200_000),
            sp_runtime::TokenError::FundsUnavailable
        );
    });
}

#[test]
fn rollback_multiple_times_accumulates() {
    new_test_ext().execute_with(|| {
        // credit 200
        assert_ok!(Loyalty::do_credit_shopping_balance(ENTITY_ID, &BUYER, 200));
        // consume 200
        assert_ok!(Loyalty::do_consume_shopping_balance(ENTITY_ID, &BUYER, 200));
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 0);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 0);

        // rollback 80
        assert_ok!(Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 80));
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 80);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 80);

        // rollback 120
        assert_ok!(Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 120));
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 200);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 200);
    });
}

// ============================================================================
// LoyaltyWritePort rollback 委托
// ============================================================================

#[test]
fn loyalty_write_port_rollback_shopping_balance_delegates() {
    new_test_ext().execute_with(|| {
        assert_ok!(Loyalty::do_credit_shopping_balance(ENTITY_ID, &BUYER, 500));
        assert_ok!(Loyalty::do_consume_shopping_balance(ENTITY_ID, &BUYER, 300));

        // 通过 trait 接口调用
        assert_ok!(
            <Loyalty as LoyaltyWritePort<u64, u128>>::rollback_shopping_balance(
                ENTITY_ID, &BUYER, 300,
            )
        );
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 500);
    });
}

#[test]
fn loyalty_write_port_rollback_token_discount_delegates_to_token_provider() {
    new_test_ext().execute_with(|| {
        // 通过 trait 接口调用
        assert_ok!(
            <Loyalty as LoyaltyWritePort<u64, u128>>::rollback_token_discount(
                ENTITY_ID, &BUYER, 1000,
            )
        );
        // 验证 MockTokenProvider::refund_discount_tokens 被调用
        let calls = get_refund_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (ENTITY_ID, BUYER, 1000));
    });
}

// ============================================================================
// 游标式积分清理
// ============================================================================

#[test]
fn disable_points_sets_cleanup_cursor() {
    new_test_ext().execute_with(|| {
        // 启用积分
        assert_ok!(Loyalty::enable_points(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            b"TestPts".to_vec().try_into().unwrap(),
            b"TP".to_vec().try_into().unwrap(),
            500, 1000, false,
        ));
        assert!(ShopPointsConfigs::<Test>::contains_key(SHOP_ID));

        // 禁用 — config 应立即删除
        assert_ok!(Loyalty::disable_points(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert!(!ShopPointsConfigs::<Test>::contains_key(SHOP_ID));

        // 少量数据 → 一次清完，游标应已移除
        assert!(!PointsCleanupCursor::<Test>::contains_key(SHOP_ID));
    });
}

#[test]
fn continue_cleanup_fails_when_no_cursor() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Loyalty::continue_cleanup(RuntimeOrigin::signed(BUYER), SHOP_ID),
            Error::<Test>::NoCleanupInProgress
        );
    });
}

#[test]
fn continue_cleanup_works_when_cursor_exists() {
    new_test_ext().execute_with(|| {
        // 手动设置游标模拟未完成的清理
        PointsCleanupCursor::<Test>::insert(SHOP_ID, CleanupPhase::ClearingBalances);

        // 无实际数据 → 一次完成
        assert_ok!(Loyalty::continue_cleanup(RuntimeOrigin::signed(BUYER), SHOP_ID));
        assert!(!PointsCleanupCursor::<Test>::contains_key(SHOP_ID));
    });
}

#[test]
fn cleanup_shop_points_uses_cursor() {
    new_test_ext().execute_with(|| {
        // 启用并发放积分
        assert_ok!(Loyalty::enable_points(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            b"TestPts".to_vec().try_into().unwrap(),
            b"TP".to_vec().try_into().unwrap(),
            500, 1000, false,
        ));
        // 发放一些积分
        assert_ok!(Loyalty::issue_points(SHOP_ID, &BUYER, 100));
        assert_eq!(ShopPointsBalances::<Test>::get(SHOP_ID, BUYER), 100);

        // cleanup（供 shop 关闭时调用）
        Loyalty::cleanup_shop_points(SHOP_ID);

        // config 已删除
        assert!(!ShopPointsConfigs::<Test>::contains_key(SHOP_ID));
        // 小数据量一次清完
        assert!(!PointsCleanupCursor::<Test>::contains_key(SHOP_ID));
        // 余额已清零
        assert_eq!(ShopPointsBalances::<Test>::get(SHOP_ID, BUYER), 0);
        // metadata 已清
        assert_eq!(ShopPointsTotalSupply::<Test>::get(SHOP_ID), 0);
    });
}

#[test]
fn disable_points_clears_metadata_after_all_data_cleaned() {
    new_test_ext().execute_with(|| {
        // 启用 + 设置 TTL + max_supply + 发放积分
        assert_ok!(Loyalty::enable_points(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            b"LP".to_vec().try_into().unwrap(),
            b"L".to_vec().try_into().unwrap(),
            500, 1000, true,
        ));
        assert_ok!(Loyalty::set_points_ttl(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            100,
        ));
        assert_ok!(Loyalty::set_points_max_supply(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            10_000,
        ));
        assert_ok!(Loyalty::issue_points(SHOP_ID, &BUYER, 50));

        // 禁用
        assert_ok!(Loyalty::disable_points(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // 验证所有 metadata 都被清理
        assert_eq!(ShopPointsTotalSupply::<Test>::get(SHOP_ID), 0);
        assert_eq!(ShopPointsTtl::<Test>::get(SHOP_ID), 0);
        assert_eq!(ShopPointsMaxSupply::<Test>::get(SHOP_ID), 0);
        assert_eq!(ShopPointsBalances::<Test>::get(SHOP_ID, BUYER), 0);
    });
}

// ============================================================================
// 回滚 + consume 对称性
// ============================================================================

#[test]
fn consume_then_rollback_is_identity() {
    new_test_ext().execute_with(|| {
        // credit 500
        assert_ok!(Loyalty::do_credit_shopping_balance(ENTITY_ID, &BUYER, 500));

        let buyer_bal_before = balance_of(BUYER);
        let entity_bal_before = balance_of(ENTITY_ACCOUNT);
        let member_bal_before = MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER);
        let total_before = ShopShoppingTotal::<Test>::get(ENTITY_ID);

        // consume 300
        assert_ok!(Loyalty::do_consume_shopping_balance(ENTITY_ID, &BUYER, 300));

        // rollback 300
        assert_ok!(Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 300));

        // 所有状态恢复
        assert_eq!(balance_of(BUYER), buyer_bal_before);
        assert_eq!(balance_of(ENTITY_ACCOUNT), entity_bal_before);
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), member_bal_before);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), total_before);
    });
}

#[test]
fn rollback_without_prior_consume_still_adds_balance() {
    new_test_ext().execute_with(|| {
        // 直接 rollback（无 consume 历史）— 仍然应该增加记账余额
        // 这在边界情况下是安全的（如手动修复场景）
        assert_ok!(Loyalty::do_rollback_shopping_balance(ENTITY_ID, &BUYER, 50));
        assert_eq!(MemberShoppingBalance::<Test>::get(ENTITY_ID, BUYER), 50);
        assert_eq!(ShopShoppingTotal::<Test>::get(ENTITY_ID), 50);
    });
}
