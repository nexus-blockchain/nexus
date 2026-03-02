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
    /// submit_review: 3 reads (order_buyer, is_order_completed, Reviews)
    /// + 1 read+write (UserReviews) + 2 writes (Reviews, ReviewCount)
    /// + conditional 1 read + 1 write (ShopReviewCount, update_shop_rating)
    fn submit_review() -> Weight {
        Weight::from_parts(45_000_000, 7_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(5))
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
        Weight::from_parts(45_000_000, 7_000)
    }

    fn set_review_enabled() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
    }
}
