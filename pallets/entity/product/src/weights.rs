//! 权重定义模块
//!
//! 定义 `WeightInfo` trait 和默认 `SubstrateWeight` 实现。
//! 当前为基于 DB read/write 分析的手工估算值。
//! 生产环境应通过 `frame_benchmarking` 生成真实权重替换。
//!
//! 运行 benchmark 命令：
//! ```bash
//! cargo run --release --features runtime-benchmarks -- benchmark pallet \
//!   --chain dev \
//!   --pallet pallet_entity_product \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/entity/product/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::{weights::Weight, pallet_prelude::Get};

/// WeightInfo trait — 每个 extrinsic 对应一个权重函数
pub trait WeightInfo {
    fn create_product() -> Weight;
    fn update_product() -> Weight;
    fn publish_product() -> Weight;
    fn unpublish_product() -> Weight;
    fn delete_product() -> Weight;
    fn force_unpublish_product() -> Weight;
    fn batch_publish_products(n: u32) -> Weight;
    fn batch_unpublish_products(n: u32) -> Weight;
    fn batch_delete_products(n: u32) -> Weight;
    fn force_delete_product() -> Weight;
}

/// 基于 DB read/write 分析的权重估算（pre-benchmark）。
///
/// 估算方法：
/// - ref_time 基础值：每次 DB read ≈ 25M，每次 DB write ≈ 100M，加上计算开销
/// - proof_size：每个 storage item ≈ 500 bytes，基础 ≈ 3500
/// - 使用 `T::DbWeight` 精确反映 DB 操作成本
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// create_product:
    /// reads: ShopProvider::shop_exists, ShopProvider::is_shop_active,
    ///        ShopProvider::shop_entity_id, EntityProvider::is_entity_locked,
    ///        ShopProvider::shop_owner, ShopProducts(get), PricingProvider::get_nex_usdt_price,
    ///        PricingProvider::is_price_stale, Currency::free_balance,
    ///        NextProductId, ShopProvider::shop_account = ~11
    /// writes: Currency::transfer(2 accounts), Products, ShopProducts(try_mutate),
    ///         NextProductId, ProductDeposits, ProductStats,
    ///         ShopProvider::increment_product_count,
    ///         StoragePin::pin(×3~5) = ~10
    fn create_product() -> Weight {
        Weight::from_parts(80_000_000, 10_000)
            .saturating_add(T::DbWeight::get().reads(11))
            .saturating_add(T::DbWeight::get().writes(10))
    }

    /// update_product:
    /// reads: Products(try_mutate), ShopProvider::shop_entity_id,
    ///        EntityProvider::is_entity_locked, ShopProvider::shop_owner,
    ///        ShopProvider::is_shop_active, ProductStats(conditional) = ~6
    /// writes: Products(mutate), ProductStats(conditional),
    ///         StoragePin::unpin(×0~5), StoragePin::pin(×0~5) = ~6 worst-case
    fn update_product() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(6))
            .saturating_add(T::DbWeight::get().writes(6))
    }

    /// publish_product:
    /// reads: Products(try_mutate), ShopProvider::shop_entity_id,
    ///        EntityProvider::is_entity_locked, ShopProvider::shop_owner,
    ///        ShopProvider::is_shop_active = ~5
    /// writes: Products(mutate), ProductStats = ~2
    fn publish_product() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// unpublish_product:
    /// reads: Products(try_mutate), ShopProvider::shop_entity_id,
    ///        EntityProvider::is_entity_locked, ShopProvider::shop_owner = ~4
    /// writes: Products(mutate), ProductStats(conditional) = ~2
    fn unpublish_product() -> Weight {
        Weight::from_parts(30_000_000, 5_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// delete_product:
    /// reads: Products(get), ShopProvider::shop_entity_id,
    ///        EntityProvider::is_entity_locked, ShopProvider::shop_owner,
    ///        ProductDeposits(take), Currency::free_balance = ~6
    /// writes: Currency::transfer(2 accounts), Products(remove),
    ///         ShopProducts(mutate), ProductDeposits(take), ProductStats,
    ///         ShopProvider::decrement_product_count,
    ///         StoragePin::unpin(×3~5) = ~10
    fn delete_product() -> Weight {
        Weight::from_parts(70_000_000, 9_000)
            .saturating_add(T::DbWeight::get().reads(6))
            .saturating_add(T::DbWeight::get().writes(10))
    }

    /// force_unpublish_product:
    /// reads: Products(try_mutate) = ~1
    /// writes: Products(mutate), ProductStats(conditional) = ~2
    fn force_unpublish_product() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    /// batch_publish_products: per-item = publish_product cost
    /// reads: n × 5, writes: n × 2
    fn batch_publish_products(n: u32) -> Weight {
        Weight::from_parts(30_000_000u64.saturating_mul(n as u64), 5_000u64.saturating_mul(n as u64))
            .saturating_add(T::DbWeight::get().reads(5u64.saturating_mul(n as u64)))
            .saturating_add(T::DbWeight::get().writes(2u64.saturating_mul(n as u64)))
    }

    /// batch_unpublish_products: per-item = unpublish_product cost
    /// reads: n × 4, writes: n × 2
    fn batch_unpublish_products(n: u32) -> Weight {
        Weight::from_parts(30_000_000u64.saturating_mul(n as u64), 5_000u64.saturating_mul(n as u64))
            .saturating_add(T::DbWeight::get().reads(4u64.saturating_mul(n as u64)))
            .saturating_add(T::DbWeight::get().writes(2u64.saturating_mul(n as u64)))
    }

    /// batch_delete_products: per-item = delete_product cost
    /// reads: n × 6, writes: n × 10
    fn batch_delete_products(n: u32) -> Weight {
        Weight::from_parts(70_000_000u64.saturating_mul(n as u64), 9_000u64.saturating_mul(n as u64))
            .saturating_add(T::DbWeight::get().reads(6u64.saturating_mul(n as u64)))
            .saturating_add(T::DbWeight::get().writes(10u64.saturating_mul(n as u64)))
    }

    /// force_delete_product:
    /// reads: Products(get), ProductDeposits(take), Currency::free_balance = ~3
    /// writes: Currency::transfer(2), Products(remove), ShopProducts(mutate),
    ///         ProductDeposits(take), ProductStats,
    ///         ShopProvider::decrement_product_count,
    ///         StoragePin::unpin(×3~5) = ~10
    fn force_delete_product() -> Weight {
        Weight::from_parts(70_000_000, 10_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(10))
    }
}

/// 单元测试用零权重实现
impl WeightInfo for () {
    fn create_product() -> Weight { Weight::zero() }
    fn update_product() -> Weight { Weight::zero() }
    fn publish_product() -> Weight { Weight::zero() }
    fn unpublish_product() -> Weight { Weight::zero() }
    fn delete_product() -> Weight { Weight::zero() }
    fn force_unpublish_product() -> Weight { Weight::zero() }
    fn batch_publish_products(_n: u32) -> Weight { Weight::zero() }
    fn batch_unpublish_products(_n: u32) -> Weight { Weight::zero() }
    fn batch_delete_products(_n: u32) -> Weight { Weight::zero() }
    fn force_delete_product() -> Weight { Weight::zero() }
}
