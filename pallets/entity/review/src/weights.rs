// Weights for pallet-entity-review
// Benchmarks not yet generated — using estimated values

use frame_support::{weights::Weight, pallet_prelude::Get};

pub trait WeightInfo {
    fn submit_review() -> Weight;
}

/// Substrate weight estimates (pre-benchmark).
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// submit_review: 3 reads (order_buyer, is_order_completed, Reviews) + 2 writes (Reviews, ReviewCount)
    /// + conditional 1 read + 1 write (ShopReviewCount, update_shop_rating)
    fn submit_review() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(4))
    }
}

/// Unit weight for testing.
impl WeightInfo for () {
    fn submit_review() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
}
