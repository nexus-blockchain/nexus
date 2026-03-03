// Weights for pallet-entity-review
// Benchmarks not yet generated — using estimated values

use frame_support::{weights::Weight, pallet_prelude::Get};

pub trait WeightInfo {
    fn submit_review() -> Weight;
    fn set_review_enabled() -> Weight;
}

/// Substrate weight estimates (pre-benchmark).
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// submit_review:
    /// reads: order_buyer, is_order_completed, is_order_disputed, order_completed_at,
    ///        Reviews, order_shop_id, shop_entity_id, EntityReviewDisabled,
    ///        UserReviews, update_shop_rating, update_entity_rating = ~8 worst-case
    /// writes: UserReviews, Reviews, ReviewCount, ShopReviewCount,
    ///         update_shop_rating, update_entity_rating = ~6 worst-case
    fn submit_review() -> Weight {
        Weight::from_parts(55_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(8))
            .saturating_add(T::DbWeight::get().writes(6))
    }

    /// set_review_enabled: 3 reads (entity_exists, is_entity_active, is_entity_admin)
    /// + 1 write (EntityReviewDisabled)
    fn set_review_enabled() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(1))
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
}
