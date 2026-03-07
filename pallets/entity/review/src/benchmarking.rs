//! Benchmarking for pallet-entity-review
//!
//! 全部 5 个 extrinsics 均有 benchmark。
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

const ORDER_1: u64 = 1;
const ORDER_2: u64 = 2;
const SHOP_1: u64 = 10;
const ENTITY_1: u64 = 100;
const PRODUCT_1: u64 = 1000;

// ==================== Helper 函数 ====================

/// 在 test 环境下设置 mock 状态
fn setup_entity_for<T: Config>(_eid: u64, _owner: &T::AccountId) {
    #[cfg(test)]
    {
        use codec::Encode;
        let bytes = _owner.encode();
        let id = if bytes.len() >= 8 {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            0u64
        };
        crate::mock::add_entity(_eid, id, true, vec![id]);
        crate::mock::set_entity_admin(_eid, id, 0xFFFFFFFF);
    }
}

fn setup_order_for<T: Config>(_order_id: u64, _buyer: &T::AccountId, _shop_id: u64) {
    #[cfg(test)]
    {
        use codec::Encode;
        let bytes = _buyer.encode();
        let id = if bytes.len() >= 8 {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            0u64
        };
        crate::mock::add_order(_order_id, id, _shop_id, true);
        crate::mock::set_order_completed_at(_order_id, 1);
        crate::mock::set_order_product_id(_order_id, PRODUCT_1);
    }
}

fn setup_shop_for<T: Config>(_shop_id: u64, _entity_id: u64) {
    #[cfg(test)]
    {
        crate::mock::add_shop(_shop_id);
        crate::mock::set_shop_entity(_shop_id, _entity_id);
    }
}

/// 直接写入一条评价记录
fn seed_review<T: Config>(order_id: u64, reviewer: &T::AccountId, rating: u8, product_id: Option<u64>) {
    let now = frame_system::Pallet::<T>::block_number();
    let cid: BoundedVec<u8, T::MaxCidLength> = b"QmBenchCid".to_vec().try_into().unwrap();
    let review = pallet::MallReview {
        order_id,
        reviewer: reviewer.clone(),
        rating,
        content_cid: Some(cid),
        created_at: now,
        product_id,
        edited: false,
    };
    pallet::Reviews::<T>::insert(order_id, review);
    pallet::ReviewCount::<T>::mutate(|c| *c = c.saturating_add(1));
    pallet::UserReviews::<T>::mutate(reviewer, |reviews| {
        let _ = reviews.try_push(order_id);
    });
    if let Some(pid) = product_id {
        pallet::ProductReviews::<T>::mutate(pid, |reviews| {
            let _ = reviews.try_push(order_id);
        });
        pallet::ProductReviewCount::<T>::mutate(pid, |c| *c = c.saturating_add(1));
        pallet::ProductRatingSum::<T>::mutate(pid, |s| *s = s.saturating_add(rating as u64));
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ==================== call_index(0): submit_review ====================
    #[benchmark]
    fn submit_review() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        setup_shop_for::<T>(SHOP_1, ENTITY_1);
        setup_order_for::<T>(ORDER_1, &caller, SHOP_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ORDER_1, 5u8, Some(b"QmBenchContent".to_vec()));
    }

    // ==================== call_index(1): set_review_enabled ====================
    #[benchmark]
    fn set_review_enabled() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, false);
    }

    // ==================== call_index(2): remove_review ====================
    #[benchmark]
    fn remove_review() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        setup_shop_for::<T>(SHOP_1, ENTITY_1);
        setup_order_for::<T>(ORDER_1, &caller, SHOP_1);
        seed_review::<T>(ORDER_1, &caller, 4, Some(PRODUCT_1));
        pallet::ShopReviewCount::<T>::insert(SHOP_1, 1u64);
        #[extrinsic_call]
        _(RawOrigin::Root, ORDER_1);
    }

    // ==================== call_index(3): reply_to_review ====================
    #[benchmark]
    fn reply_to_review() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        setup_shop_for::<T>(SHOP_1, ENTITY_1);
        setup_order_for::<T>(ORDER_1, &caller, SHOP_1);
        seed_review::<T>(ORDER_1, &caller, 3, None);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ORDER_1, b"QmReplyContent".to_vec());
    }

    // ==================== call_index(4): edit_review ====================
    #[benchmark]
    fn edit_review() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        setup_shop_for::<T>(SHOP_1, ENTITY_1);
        setup_order_for::<T>(ORDER_1, &caller, SHOP_1);
        seed_review::<T>(ORDER_1, &caller, 3, Some(PRODUCT_1));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ORDER_1, 5u8, Some(b"QmEditedContent".to_vec()));
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
