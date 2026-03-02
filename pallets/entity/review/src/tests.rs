use crate::{mock::*, Error, Event, Reviews, ReviewCount, ShopReviewCount, UserReviews, EntityReviewDisabled};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

// ==================== 基础成功路径 ====================

#[test]
fn submit_review_works() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1,
            5,
            None,
        ));

        // 验证存储
        let review = Reviews::<Test>::get(1).unwrap();
        assert_eq!(review.order_id, 1);
        assert_eq!(review.reviewer, BUYER);
        assert_eq!(review.rating, 5);
        assert_eq!(review.content_cid, None);
        assert_eq!(review.created_at, 1);

        // 验证计数
        assert_eq!(ReviewCount::<Test>::get(), 1);

        // 验证店铺评分更新
        let (sum, count) = get_shop_rating(100).unwrap();
        assert_eq!(sum, 5);
        assert_eq!(count, 1);

        // 验证事件
        System::assert_last_event(
            Event::ReviewSubmitted {
                order_id: 1,
                reviewer: BUYER,
                shop_id: Some(100),
                rating: 5,
            }
            .into(),
        );
    });
}

#[test]
fn submit_review_with_cid_works() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        let cid = b"QmTest1234567890".to_vec();
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1,
            4,
            Some(cid.clone()),
        ));

        let review = Reviews::<Test>::get(1).unwrap();
        assert_eq!(review.content_cid.unwrap().to_vec(), cid);
        assert_eq!(review.rating, 4);
    });
}

#[test]
fn submit_review_rating_boundary_1_works() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1,
            1,
            None,
        ));

        assert_eq!(Reviews::<Test>::get(1).unwrap().rating, 1);
    });
}

#[test]
fn submit_review_rating_boundary_5_works() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1,
            5,
            None,
        ));

        assert_eq!(Reviews::<Test>::get(1).unwrap().rating, 5);
    });
}

#[test]
fn multiple_reviews_different_orders() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        add_order(2, BUYER, 100, true);
        add_order(3, BUYER2, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            2, 3, None,
        ));
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER2).into(),
            3, 4, None,
        ));

        assert_eq!(ReviewCount::<Test>::get(), 3);
        assert!(Reviews::<Test>::get(1).is_some());
        assert!(Reviews::<Test>::get(2).is_some());
        assert!(Reviews::<Test>::get(3).is_some());

        // 店铺评分累计
        let (sum, count) = get_shop_rating(100).unwrap();
        assert_eq!(sum, 12); // 5+3+4
        assert_eq!(count, 3);
    });
}

// ==================== 评分验证 ====================

#[test]
fn submit_review_fails_rating_zero() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 0, None,
            ),
            Error::<Test>::InvalidRating
        );
    });
}

#[test]
fn submit_review_fails_rating_six() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 6, None,
            ),
            Error::<Test>::InvalidRating
        );
    });
}

#[test]
fn submit_review_fails_rating_255() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 255, None,
            ),
            Error::<Test>::InvalidRating
        );
    });
}

// ==================== 订单验证 ====================

#[test]
fn submit_review_fails_order_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                999, 5, None,
            ),
            Error::<Test>::OrderNotFound
        );
    });
}

#[test]
fn submit_review_fails_not_buyer() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(OTHER).into(),
                1, 5, None,
            ),
            Error::<Test>::NotOrderBuyer
        );
    });
}

#[test]
fn submit_review_fails_order_not_completed() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, false); // not completed

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 5, None,
            ),
            Error::<Test>::OrderNotCompleted
        );
    });
}

#[test]
fn submit_review_fails_already_reviewed() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 3, None,
            ),
            Error::<Test>::AlreadyReviewed
        );

        // 计数仍为 1
        assert_eq!(ReviewCount::<Test>::get(), 1);
    });
}

// ==================== CID 验证 ====================

#[test]
fn submit_review_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        let cid = vec![0u8; 65]; // MaxCidLength = 64

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 5, Some(cid),
            ),
            Error::<Test>::CidTooLong
        );
    });
}

#[test]
fn submit_review_cid_at_max_length() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        let cid = vec![0u8; 64]; // 刚好 MaxCidLength

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, Some(cid.clone()),
        ));

        let review = Reviews::<Test>::get(1).unwrap();
        assert_eq!(review.content_cid.unwrap().len(), 64);
    });
}

#[test]
fn submit_review_empty_cid_is_none() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));

        let review = Reviews::<Test>::get(1).unwrap();
        assert!(review.content_cid.is_none());
    });
}

// ==================== 店铺评分更新 ====================

#[test]
fn submit_review_shop_rating_fail_still_stores_review() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        set_shop_rating_fail(true);

        // H1: update_shop_rating 失败不回滚评价，best-effort
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));

        // 评价应已存储
        assert!(Reviews::<Test>::get(1).is_some());
        assert_eq!(ReviewCount::<Test>::get(), 1);
        assert_eq!(Reviews::<Test>::get(1).unwrap().rating, 5);

        // 店铺评分未更新（失败）
        let (sum, count) = get_shop_rating(100).unwrap();
        assert_eq!(sum, 0);
        assert_eq!(count, 0);

        // ShopRatingUpdateFailed 事件应被发出
        System::assert_has_event(
            Event::ShopRatingUpdateFailed {
                order_id: 1,
                shop_id: 100,
            }
            .into(),
        );
    });
}

#[test]
fn shop_rating_accumulates_correctly() {
    new_test_ext().execute_with(|| {
        add_shop(200);
        add_order(10, BUYER, 200, true);
        add_order(11, BUYER2, 200, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            10, 2, None,
        ));
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER2).into(),
            11, 4, None,
        ));

        let (sum, count) = get_shop_rating(200).unwrap();
        assert_eq!(sum, 6);
        assert_eq!(count, 2);
    });
}

// ==================== 权限验证 ====================

#[test]
fn submit_review_fails_unsigned() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::None.into(),
                1, 5, None,
            ),
            frame_support::error::BadOrigin
        );
    });
}

// ==================== 事件验证 ====================

#[test]
fn submit_review_emits_correct_event_with_shop_id() {
    new_test_ext().execute_with(|| {
        add_shop(42);
        add_order(7, BUYER, 42, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            7, 3, None,
        ));

        System::assert_last_event(
            Event::ReviewSubmitted {
                order_id: 7,
                reviewer: BUYER,
                shop_id: Some(42),
                rating: 3,
            }
            .into(),
        );
    });
}

// ==================== ReviewCount 溢出安全 ====================

#[test]
fn review_count_saturating_add() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        // 预设计数为 u64::MAX - 1
        ReviewCount::<Test>::put(u64::MAX - 1);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));

        // saturating_add 应止于 MAX
        assert_eq!(ReviewCount::<Test>::get(), u64::MAX);
    });
}

// ==================== 数据结构快照 ====================

#[test]
fn mall_review_struct_fields() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        System::set_block_number(42);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 3, Some(b"Qm123".to_vec()),
        ));

        let review = Reviews::<Test>::get(1).unwrap();
        assert_eq!(review.order_id, 1);
        assert_eq!(review.reviewer, BUYER);
        assert_eq!(review.rating, 3);
        assert_eq!(review.content_cid.unwrap().to_vec(), b"Qm123".to_vec());
        assert_eq!(review.created_at, 42);
    });
}

// ==================== 不同店铺评分隔离 ====================

#[test]
fn different_shops_rating_isolated() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_shop(200);
        add_order(1, BUYER, 100, true);
        add_order(2, BUYER2, 200, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER2).into(),
            2, 1, None,
        ));

        let (sum1, count1) = get_shop_rating(100).unwrap();
        assert_eq!(sum1, 5);
        assert_eq!(count1, 1);

        let (sum2, count2) = get_shop_rating(200).unwrap();
        assert_eq!(sum2, 1);
        assert_eq!(count2, 1);
    });
}

// ==================== H1: 空 CID 验证 ====================

#[test]
fn h1_submit_review_rejects_empty_cid() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 5, Some(vec![]),
            ),
            Error::<Test>::EmptyCid
        );

        // 评价不应存储
        assert!(Reviews::<Test>::get(1).is_none());
        assert_eq!(ReviewCount::<Test>::get(), 0);
    });
}

// ==================== H2: 店铺评价计数 ====================

#[test]
fn h2_shop_review_count_increments() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        add_order(2, BUYER2, 100, true);

        assert_eq!(ShopReviewCount::<Test>::get(100), 0);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert_eq!(ShopReviewCount::<Test>::get(100), 1);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER2).into(),
            2, 3, None,
        ));
        assert_eq!(ShopReviewCount::<Test>::get(100), 2);
    });
}

#[test]
fn h2_shop_review_count_isolated_per_shop() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_shop(200);
        add_order(1, BUYER, 100, true);
        add_order(2, BUYER2, 200, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 4, None,
        ));
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER2).into(),
            2, 2, None,
        ));

        assert_eq!(ShopReviewCount::<Test>::get(100), 1);
        assert_eq!(ShopReviewCount::<Test>::get(200), 1);
    });
}

#[test]
fn h2_shop_review_count_not_incremented_when_rating_fails() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        set_shop_rating_fail(true);

        // H1: best-effort — 评价存储但 ShopReviewCount 不递增
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));

        // 评价已存储
        assert!(Reviews::<Test>::get(1).is_some());
        // 店铺评价计数不应递增（update_shop_rating 失败）
        assert_eq!(ShopReviewCount::<Test>::get(100), 0);
    });
}

#[test]
fn h1_submit_review_accepts_non_empty_cid() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);

        // 单字节 CID 应被接受
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 4, Some(vec![0x42]),
        ));

        let review = Reviews::<Test>::get(1).unwrap();
        assert_eq!(review.content_cid.unwrap().to_vec(), vec![0x42]);
    });
}

// ==================== H2(v0.4): 用户评价索引 ====================

#[test]
fn h2v4_user_reviews_index_populated() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        add_order(2, BUYER, 100, true);

        assert_eq!(UserReviews::<Test>::get(BUYER).len(), 0);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert_eq!(UserReviews::<Test>::get(BUYER).to_vec(), vec![1]);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            2, 3, None,
        ));
        assert_eq!(UserReviews::<Test>::get(BUYER).to_vec(), vec![1, 2]);
    });
}

#[test]
fn h2v4_user_reviews_isolated_per_user() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        add_order(2, BUYER2, 100, true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER2).into(),
            2, 3, None,
        ));

        assert_eq!(UserReviews::<Test>::get(BUYER).to_vec(), vec![1]);
        assert_eq!(UserReviews::<Test>::get(BUYER2).to_vec(), vec![2]);
    });
}

#[test]
fn h2v4_user_reviews_limit_reached() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        // MaxReviewsPerUser = 100，创建 101 个订单
        for i in 0..101u64 {
            add_order(i, BUYER, 100, true);
        }

        // 前 100 个评价应成功
        for i in 0..100u64 {
            assert_ok!(EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                i, 4, None,
            ));
        }
        assert_eq!(UserReviews::<Test>::get(BUYER).len(), 100);

        // 第 101 个评价应因达到上限而失败
        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                100, 4, None,
            ),
            Error::<Test>::UserReviewLimitReached
        );
    });
}

// ==================== H1(v0.4): shop rating best-effort ====================

#[test]
fn h1v4_shop_rating_fail_emits_event_and_stores_review() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        add_order(1, BUYER, 100, true);
        set_shop_rating_fail(true);

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 4, None,
        ));

        // 评价已存储
        let review = Reviews::<Test>::get(1).unwrap();
        assert_eq!(review.rating, 4);
        assert_eq!(ReviewCount::<Test>::get(), 1);

        // 用户索引已更新
        assert_eq!(UserReviews::<Test>::get(BUYER).to_vec(), vec![1]);

        // 店铺评分未更新
        assert_eq!(ShopReviewCount::<Test>::get(100), 0);

        // 最后事件是 ReviewSubmitted（ShopRatingUpdateFailed 在前）
        System::assert_last_event(
            Event::ReviewSubmitted {
                order_id: 1,
                reviewer: BUYER,
                shop_id: Some(100),
                rating: 4,
            }
            .into(),
        );
        System::assert_has_event(
            Event::ShopRatingUpdateFailed {
                order_id: 1,
                shop_id: 100,
            }
            .into(),
        );
    });
}

// ==================== Entity 评价开关 ====================

#[test]
fn set_review_enabled_disable_then_submit_fails() {
    new_test_ext().execute_with(|| {
        add_entity(ENTITY_1, ENTITY_OWNER, true, vec![ENTITY_ADMIN]);
        add_shop(100);
        set_shop_entity(100, ENTITY_1);
        add_order(1, BUYER, 100, true);

        // Entity owner 关闭评价
        assert_ok!(EntityReview::set_review_enabled(
            RawOrigin::Signed(ENTITY_OWNER).into(),
            ENTITY_1,
            false,
        ));

        // 存储已写入
        assert!(EntityReviewDisabled::<Test>::contains_key(ENTITY_1));

        // 事件
        System::assert_last_event(
            Event::ReviewConfigUpdated {
                entity_id: ENTITY_1,
                enabled: false,
            }
            .into(),
        );

        // 提交评价应失败
        assert_noop!(
            EntityReview::submit_review(
                RawOrigin::Signed(BUYER).into(),
                1, 5, None,
            ),
            Error::<Test>::ReviewsDisabledForEntity
        );
    });
}

#[test]
fn set_review_enabled_reopen_works() {
    new_test_ext().execute_with(|| {
        add_entity(ENTITY_1, ENTITY_OWNER, true, vec![]);
        add_shop(100);
        set_shop_entity(100, ENTITY_1);
        add_order(1, BUYER, 100, true);

        // 关闭
        assert_ok!(EntityReview::set_review_enabled(
            RawOrigin::Signed(ENTITY_OWNER).into(),
            ENTITY_1,
            false,
        ));
        assert!(EntityReviewDisabled::<Test>::contains_key(ENTITY_1));

        // 重新开启
        assert_ok!(EntityReview::set_review_enabled(
            RawOrigin::Signed(ENTITY_OWNER).into(),
            ENTITY_1,
            true,
        ));
        assert!(!EntityReviewDisabled::<Test>::contains_key(ENTITY_1));

        // 提交评价应成功
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert!(Reviews::<Test>::get(1).is_some());
    });
}

#[test]
fn set_review_enabled_admin_can_toggle() {
    new_test_ext().execute_with(|| {
        add_entity(ENTITY_1, ENTITY_OWNER, true, vec![ENTITY_ADMIN]);

        // Admin 可以设置
        assert_ok!(EntityReview::set_review_enabled(
            RawOrigin::Signed(ENTITY_ADMIN).into(),
            ENTITY_1,
            false,
        ));
        assert!(EntityReviewDisabled::<Test>::contains_key(ENTITY_1));
    });
}

#[test]
fn set_review_enabled_fails_not_admin() {
    new_test_ext().execute_with(|| {
        add_entity(ENTITY_1, ENTITY_OWNER, true, vec![]);

        assert_noop!(
            EntityReview::set_review_enabled(
                RawOrigin::Signed(OTHER).into(),
                ENTITY_1,
                false,
            ),
            Error::<Test>::NotEntityAdmin
        );
    });
}

#[test]
fn set_review_enabled_fails_entity_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityReview::set_review_enabled(
                RawOrigin::Signed(ENTITY_OWNER).into(),
                9999,
                false,
            ),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn set_review_enabled_fails_entity_not_active() {
    new_test_ext().execute_with(|| {
        add_entity(ENTITY_1, ENTITY_OWNER, false, vec![]); // not active

        assert_noop!(
            EntityReview::set_review_enabled(
                RawOrigin::Signed(ENTITY_OWNER).into(),
                ENTITY_1,
                false,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn submit_review_default_enabled_no_entity() {
    new_test_ext().execute_with(|| {
        add_shop(100);
        // 不设置 shop_entity 映射 → shop_entity_id 返回 None
        add_order(1, BUYER, 100, true);

        // 无 entity 关联时默认放行
        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 4, None,
        ));
        assert!(Reviews::<Test>::get(1).is_some());
    });
}

#[test]
fn submit_review_entity_enabled_by_default() {
    new_test_ext().execute_with(|| {
        add_entity(ENTITY_1, ENTITY_OWNER, true, vec![]);
        add_shop(100);
        set_shop_entity(100, ENTITY_1);
        add_order(1, BUYER, 100, true);

        // 从未调用 set_review_enabled → 默认开启
        assert!(!EntityReviewDisabled::<Test>::contains_key(ENTITY_1));

        assert_ok!(EntityReview::submit_review(
            RawOrigin::Signed(BUYER).into(),
            1, 5, None,
        ));
        assert!(Reviews::<Test>::get(1).is_some());
    });
}
