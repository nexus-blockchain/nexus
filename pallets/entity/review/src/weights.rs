// Weights for pallet-entity-review
// Benchmarks not yet generated — using estimated values

use frame_support::{weights::Weight, pallet_prelude::Get};

pub trait WeightInfo {
    fn submit_review() -> Weight;
    fn set_review_enabled() -> Weight;
    fn remove_review() -> Weight;
}

/// Substrate weight estimates (pre-benchmark).
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// submit_review:
    /// reads: order_buyer, is_order_completed, is_order_disputed, order_completed_at,
    ///        Reviews(contains_key), order_shop_id, shop_entity_id, EntityReviewDisabled,
    ///        UserReviews(try_mutate), ReviewCount(try_mutate), update_shop_rating = ~10 worst-case
    /// writes: UserReviews, Reviews, ReviewCount, ShopReviewCount,
    ///         update_shop_rating = ~5 worst-case
    fn submit_review() -> Weight {
        Weight::from_parts(55_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(10))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// set_review_enabled: 4 reads (entity_exists, is_entity_active, is_entity_admin,
    ///                              EntityReviewDisabled::contains_key)
    /// + 1 write (EntityReviewDisabled)
    fn set_review_enabled() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// remove_review: 2 reads (Reviews::take, order_shop_id)
    /// + 4 writes (Reviews, ReviewCount, ShopReviewCount, UserReviews)
    fn remove_review() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(4))
    }
}

/// Unit weight for testing.
impl WeightInfo for () {
    fn submit_review() -> Weight {
        Weight::from_parts(55_000_000, 8_000)
    }

    fn set_review_enabled() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }

    fn remove_review() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
}
