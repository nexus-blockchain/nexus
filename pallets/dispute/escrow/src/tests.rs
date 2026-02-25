use crate::mock::*;
use crate::pallet::{self, Error, ExpiringAt, ExpiryOf, Locked, LockStateOf};
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};

type EscrowPallet = crate::pallet::Pallet<Test>;
type EscrowTrait = crate::pallet::Pallet<Test>;

// ============================================================================
// 基础锁定/释放/退款
// ============================================================================

#[test]
fn lock_and_release_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_eq!(Locked::<Test>::get(100), 500);
        assert_eq!(LockStateOf::<Test>::get(100), 0u8);

        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));
        assert_eq!(Locked::<Test>::get(100), 0);
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);
    });
}

#[test]
fn lock_and_refund_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 200, 1, 300));
        assert_ok!(EscrowPallet::refund(RuntimeOrigin::signed(1), 200, 1));
        assert_eq!(Locked::<Test>::get(200), 0);
        assert_eq!(LockStateOf::<Test>::get(200), 3u8);
    });
}

#[test]
fn release_fails_no_lock() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EscrowPallet::release(RuntimeOrigin::signed(1), 999, 2),
            Error::<Test>::NoLock
        );
    });
}

// ============================================================================
// C1: transfer_from_escrow 状态检查
// ============================================================================

#[test]
fn transfer_from_escrow_blocked_during_dispute() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));
        assert_eq!(LockStateOf::<Test>::get(100), 1u8);

        assert_noop!(
            <EscrowTrait as pallet::Escrow<u64, u128>>::transfer_from_escrow(100, &2, 100),
            Error::<Test>::DisputeActive
        );
    });
}

#[test]
fn transfer_from_escrow_blocked_after_close() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);

        assert_noop!(
            <EscrowTrait as pallet::Escrow<u64, u128>>::transfer_from_escrow(100, &2, 100),
            Error::<Test>::AlreadyClosed
        );
    });
}

// ============================================================================
// C2: lock_from 拒绝已关闭 id
// ============================================================================

#[test]
fn lock_from_blocked_after_close() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);

        assert_noop!(
            <EscrowTrait as pallet::Escrow<u64, u128>>::lock_from(&1, 100, 200),
            Error::<Test>::AlreadyClosed
        );
    });
}

// ============================================================================
// H1: AllowDeath 测试（小额释放不应卡住）
// ============================================================================

#[test]
fn release_all_allows_small_amounts() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 2));
        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);
    });
}

// ============================================================================
// H2: 仲裁决议状态正确性
// ============================================================================

#[test]
fn apply_decision_release_sets_closed() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));
        assert_eq!(LockStateOf::<Test>::get(100), 1u8);

        assert_ok!(EscrowPallet::apply_decision_release_all(
            RuntimeOrigin::signed(1), 100, 2
        ));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);
    });
}

#[test]
fn apply_decision_refund_sets_closed() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        assert_ok!(EscrowPallet::apply_decision_refund_all(
            RuntimeOrigin::signed(1), 100, 1
        ));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);
    });
}

#[test]
fn apply_decision_partial_bps_sets_closed() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 1000));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        assert_ok!(EscrowPallet::apply_decision_partial_bps(
            RuntimeOrigin::signed(1), 100, 2, 3, 5000
        ));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);
        assert_eq!(Locked::<Test>::get(100), 0);
    });
}

// ============================================================================
// M1: GloballyPaused 错误
// ============================================================================

#[test]
fn paused_blocks_operations() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::set_pause(RuntimeOrigin::root(), true));

        assert_noop!(
            EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500),
            Error::<Test>::GloballyPaused
        );

        assert_ok!(EscrowPallet::set_pause(RuntimeOrigin::root(), false));
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
    });
}

// ============================================================================
// M4: BoundedVec release_split
// ============================================================================

#[test]
fn release_split_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 1000));

        let entries: BoundedVec<(u64, u128), <Test as crate::Config>::MaxSplitEntries> =
            BoundedVec::try_from(vec![(2, 400), (3, 600)]).unwrap();

        assert_ok!(EscrowPallet::release_split(RuntimeOrigin::signed(1), 100, entries));
        assert_eq!(Locked::<Test>::get(100), 0);
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);
    });
}

#[test]
fn release_split_fails_exceeding_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));

        let entries: BoundedVec<(u64, u128), <Test as crate::Config>::MaxSplitEntries> =
            BoundedVec::try_from(vec![(2, 400), (3, 200)]).unwrap();

        assert_noop!(
            EscrowPallet::release_split(RuntimeOrigin::signed(1), 100, entries),
            Error::<Test>::Insufficient
        );
    });
}

// ============================================================================
// 幂等 nonce 锁定
// ============================================================================

#[test]
fn lock_with_nonce_idempotent() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock_with_nonce(RuntimeOrigin::signed(1), 100, 1, 500, 1));
        assert_eq!(Locked::<Test>::get(100), 500);

        // 重复 nonce 应被忽略
        assert_ok!(EscrowPallet::lock_with_nonce(RuntimeOrigin::signed(1), 100, 1, 500, 1));
        assert_eq!(Locked::<Test>::get(100), 500);

        // 递增 nonce 成功
        assert_ok!(EscrowPallet::lock_with_nonce(RuntimeOrigin::signed(1), 100, 1, 200, 2));
        assert_eq!(Locked::<Test>::get(100), 700);
    });
}

// ============================================================================
// 争议流程
// ============================================================================

#[test]
fn dispute_blocks_release_and_refund() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        // 🆕 EM2修复: 争议中应返回 DisputeActive 而非 NoLock
        assert_noop!(
            EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2),
            Error::<Test>::DisputeActive
        );
        assert_noop!(
            EscrowPallet::refund(RuntimeOrigin::signed(1), 100, 1),
            Error::<Test>::DisputeActive
        );
    });
}

// ============================================================================
// 到期调度
// ============================================================================

#[test]
fn schedule_and_cancel_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::schedule_expiry(RuntimeOrigin::signed(1), 100, 10));

        assert_eq!(ExpiryOf::<Test>::get(100), Some(10));
        assert_eq!(ExpiringAt::<Test>::get(10).len(), 1);

        assert_ok!(EscrowPallet::cancel_expiry(RuntimeOrigin::signed(1), 100));
        assert_eq!(ExpiryOf::<Test>::get(100), None);
        assert_eq!(ExpiringAt::<Test>::get(10).len(), 0);
    });
}

// ============================================================================
// H3: on_initialize 到期处理
// ============================================================================

#[test]
fn on_initialize_processes_expiry() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::schedule_expiry(RuntimeOrigin::signed(1), 100, 5));

        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 101, 2, 300));
        assert_ok!(EscrowPallet::schedule_expiry(RuntimeOrigin::signed(1), 101, 5));

        System::set_block_number(5);
        EscrowPallet::on_initialize(5);

        assert_eq!(Locked::<Test>::get(100), 0);
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);

        assert_eq!(Locked::<Test>::get(101), 0);
        assert_eq!(LockStateOf::<Test>::get(101), 3u8);
    });
}

#[test]
fn on_initialize_skips_disputed() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::schedule_expiry(RuntimeOrigin::signed(1), 100, 5));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        System::set_block_number(5);
        EscrowPallet::on_initialize(5);

        assert_eq!(Locked::<Test>::get(100), 500);
        assert_eq!(LockStateOf::<Test>::get(100), 1u8);
    });
}

// ============================================================================
// 重复操作防护
// ============================================================================

#[test]
fn double_release_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));

        assert_noop!(
            EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2),
            Error::<Test>::AlreadyClosed
        );
    });
}

// ============================================================================
// 🆕 EH3: lock 不能重新打开已关闭的托管
// ============================================================================

#[test]
fn lock_rejects_closed_id() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);

        // 尝试重新 lock 已关闭的 id 应失败
        assert_noop!(
            EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 200),
            Error::<Test>::AlreadyClosed
        );
    });
}

#[test]
fn lock_with_nonce_rejects_closed_id() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2));
        assert_eq!(LockStateOf::<Test>::get(100), 3u8);

        // 尝试通过 nonce 重新打开已关闭的 id 应失败
        assert_noop!(
            EscrowPallet::lock_with_nonce(RuntimeOrigin::signed(1), 100, 1, 200, 99),
            Error::<Test>::AlreadyClosed
        );
    });
}

// ============================================================================
// 🆕 EM2: release/refund 在争议中返回 DisputeActive
// ============================================================================

#[test]
fn release_returns_dispute_active_not_no_lock() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        assert_noop!(
            EscrowPallet::release(RuntimeOrigin::signed(1), 100, 2),
            Error::<Test>::DisputeActive
        );
    });
}

#[test]
fn refund_returns_dispute_active_not_no_lock() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        assert_noop!(
            EscrowPallet::refund(RuntimeOrigin::signed(1), 100, 1),
            Error::<Test>::DisputeActive
        );
    });
}

// 🆕 E4修复测试: 争议中禁止通过 lock 追加锁定
#[test]
fn lock_blocked_during_dispute() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));
        assert_eq!(LockStateOf::<Test>::get(100), 1u8);

        // lock 应该失败
        assert_noop!(
            EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 2, 200),
            Error::<Test>::DisputeActive
        );
    });
}

// 🆕 E4修复测试: 争议中禁止通过 lock_with_nonce 追加锁定
#[test]
fn lock_with_nonce_blocked_during_dispute() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 500));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        assert_noop!(
            EscrowPallet::lock_with_nonce(RuntimeOrigin::signed(1), 100, 2, 200, 1),
            Error::<Test>::DisputeActive
        );
    });
}

#[test]
fn release_split_returns_dispute_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(EscrowPallet::lock(RuntimeOrigin::signed(1), 100, 1, 1000));
        assert_ok!(EscrowPallet::dispute(RuntimeOrigin::signed(1), 100, 1));

        let entries: BoundedVec<(u64, u128), <Test as crate::Config>::MaxSplitEntries> =
            BoundedVec::try_from(vec![(2, 500), (3, 500)]).unwrap();

        assert_noop!(
            EscrowPallet::release_split(RuntimeOrigin::signed(1), 100, entries),
            Error::<Test>::DisputeActive
        );
    });
}
