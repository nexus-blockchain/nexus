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
//!   --pallet pallet_entity_order \
//!   --extrinsic '*' \
//!   --steps 50 \
//!   --repeat 20 \
//!   --output pallets/entity/order/src/weights.rs \
//!   --template .maintain/frame-weight-template.hbs
//! ```

use frame_support::{weights::Weight, pallet_prelude::Get};

/// WeightInfo trait — 每个 extrinsic 对应一个权重函数
pub trait WeightInfo {
    fn place_order() -> Weight;
    fn cancel_order() -> Weight;
    fn ship_order() -> Weight;
    fn confirm_receipt() -> Weight;
    fn request_refund() -> Weight;
    fn approve_refund() -> Weight;
    fn start_service() -> Weight;
    fn complete_service() -> Weight;
    fn confirm_service() -> Weight;
    fn set_platform_fee_rate() -> Weight;
    fn cleanup_buyer_orders() -> Weight;
    fn reject_refund() -> Weight;
    fn seller_cancel_order() -> Weight;
    fn force_refund() -> Weight;
    fn force_complete() -> Weight;
    fn update_shipping_address() -> Weight;
    fn extend_confirm_timeout() -> Weight;
    fn cleanup_shop_orders() -> Weight;
    fn update_tracking() -> Weight;
    fn seller_refund_order() -> Weight;
    fn force_partial_refund() -> Weight;
    fn withdraw_dispute() -> Weight;
    fn force_process_expirations() -> Weight;
    fn place_order_for() -> Weight;
    fn cleanup_payer_orders() -> Weight;
}

/// 基于 DB read/write 分析的权重估算（pre-benchmark）。
///
/// 估算方法：
/// - ref_time 基础值：每次 DB read ≈ 25M，每次 DB write ≈ 100M，加上计算开销
/// - proof_size：每个 storage item ≈ 500 bytes，基础 ≈ 3500
/// - 使用 `T::DbWeight` 精确反映 DB 操作成本
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// place_order:
    /// reads: ProductProvider::get_product_info, ShopProvider(×4), MemberProvider(×3),
    ///        Currency::free_balance, NextOrderId, ExpiryQueue, Escrow::lock_from = ~14
    /// writes: Escrow::lock_from, Orders, BuyerOrders, ShopOrders, NextOrderId,
    ///         ExpiryQueue, OrderStats, OrderReferrer, ProductProvider::deduct_stock = ~9
    fn place_order() -> Weight {
        Weight::from_parts(120_000_000, 16_000)
            .saturating_add(T::DbWeight::get().reads(14))
            .saturating_add(T::DbWeight::get().writes(9))
    }

    /// cancel_order:
    /// reads: Orders, Escrow::refund_all, ProductProvider::restore_stock = ~3
    /// writes: Orders, BuyerOrders(retain?), Escrow, ProductProvider, CommissionHandler = ~5
    fn cancel_order() -> Weight {
        Weight::from_parts(80_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// ship_order:
    /// reads: Orders(try_mutate), ExpiryQueue(old + new) = ~3
    /// writes: Orders, ExpiryQueue(old mutate + new try_mutate) = ~3
    fn ship_order() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// confirm_receipt:
    /// reads: Orders, Escrow(transfer×2), ShopProvider::update_shop_stats,
    ///        MemberHandler(×3), CommissionHandler, PricingProvider = ~10
    /// writes: Orders, Escrow(×2), OrderStats, OrderReferrer = ~5
    fn confirm_receipt() -> Weight {
        Weight::from_parts(100_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(10))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// request_refund:
    /// reads: Orders(try_mutate), ExpiryQueue, Escrow::set_disputed = ~3
    /// writes: Orders, ExpiryQueue, Escrow = ~3
    fn request_refund() -> Weight {
        Weight::from_parts(50_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// approve_refund:
    /// reads: Orders, Escrow(set_resolved + refund_all), ProductProvider::restore_stock = ~4
    /// writes: Orders, Escrow(×2), ProductProvider, CommissionHandler, OrderReferrer = ~6
    fn approve_refund() -> Weight {
        Weight::from_parts(80_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(6))
    }

    /// start_service:
    /// reads: Orders(try_mutate), ExpiryQueue(old + new) = ~3
    /// writes: Orders, ExpiryQueue(old + new) = ~3
    fn start_service() -> Weight {
        Weight::from_parts(50_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// complete_service:
    /// reads: Orders(try_mutate), ExpiryQueue(old + new) = ~3
    /// writes: Orders, ExpiryQueue(old + new) = ~3
    fn complete_service() -> Weight {
        Weight::from_parts(55_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// confirm_service:
    /// reads: Orders, Escrow(transfer×2), ShopProvider, MemberHandler(×3),
    ///        CommissionHandler, PricingProvider = ~10
    /// writes: Orders, Escrow(×2), OrderStats, OrderReferrer = ~5
    fn confirm_service() -> Weight {
        Weight::from_parts(100_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(10))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// set_platform_fee_rate:
    /// reads: PlatformFeeRate = ~1
    /// writes: PlatformFeeRate = ~1
    fn set_platform_fee_rate() -> Weight {
        Weight::from_parts(10_000_000, 2_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// cleanup_buyer_orders:
    /// reads: BuyerOrders, Orders(×N for status check) = ~1 + N
    /// writes: BuyerOrders = ~1
    /// 最坏情况 N=1000
    fn cleanup_buyer_orders() -> Weight {
        Weight::from_parts(80_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(101)) // 1 + 100 worst-case sample
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// reject_refund:
    /// reads: Orders, ExpiryQueue(old + new) = ~3
    /// writes: Orders, ExpiryQueue(old + new) = ~3
    fn reject_refund() -> Weight {
        Weight::from_parts(50_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// seller_cancel_order:
    /// reads: Orders, Escrow::refund_all, ProductProvider::restore_stock = ~3
    /// writes: Orders, Escrow, ProductProvider, CommissionHandler, OrderReferrer = ~5
    fn seller_cancel_order() -> Weight {
        Weight::from_parts(80_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// force_refund:
    /// reads: Orders, Escrow(set_resolved + refund_all), ProductProvider = ~4
    /// writes: Orders, Escrow(×2), ProductProvider, CommissionHandler, OrderReferrer = ~6
    fn force_refund() -> Weight {
        Weight::from_parts(100_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(6))
    }

    /// force_complete:
    /// reads: Orders, Escrow(set_resolved + transfer×2), ShopProvider,
    ///        MemberHandler(×3), CommissionHandler, PricingProvider = ~11
    /// writes: Orders, Escrow(×3), OrderStats, OrderReferrer = ~6
    fn force_complete() -> Weight {
        Weight::from_parts(120_000_000, 16_000)
            .saturating_add(T::DbWeight::get().reads(11))
            .saturating_add(T::DbWeight::get().writes(6))
    }

    /// update_shipping_address:
    /// reads: Orders(try_mutate) = ~1
    /// writes: Orders = ~1
    fn update_shipping_address() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// extend_confirm_timeout:
    /// reads: Orders(try_mutate), ExpiryQueue(old + new) = ~3
    /// writes: Orders, ExpiryQueue(old + new) = ~3
    fn extend_confirm_timeout() -> Weight {
        Weight::from_parts(50_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }

    /// cleanup_shop_orders:
    /// reads: ShopProvider::shop_owner, ShopOrders, Orders(×N) = ~2 + N
    /// writes: ShopOrders = ~1
    fn cleanup_shop_orders() -> Weight {
        Weight::from_parts(100_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(102))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// update_tracking:
    /// reads: Orders(try_mutate) = ~1
    /// writes: Orders = ~1
    fn update_tracking() -> Weight {
        Weight::from_parts(25_000_000, 4_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }

    /// seller_refund_order:
    /// reads: Orders, Escrow::refund_all, ProductProvider::restore_stock = ~3
    /// writes: Orders, Escrow, ProductProvider, CommissionHandler, OrderReferrer = ~5
    fn seller_refund_order() -> Weight {
        Weight::from_parts(80_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// force_partial_refund:
    /// reads: Orders, Escrow(set_resolved + split_partial) = ~3
    /// writes: Orders, Escrow(×2), CommissionHandler, OrderReferrer = ~5
    fn force_partial_refund() -> Weight {
        Weight::from_parts(100_000_000, 12_000)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(5))
    }

    /// withdraw_dispute:
    /// reads: Orders, ExpiryQueue(old + new), Escrow::set_resolved = ~4
    /// writes: Orders, ExpiryQueue(old + new), Escrow = ~4
    fn withdraw_dispute() -> Weight {
        Weight::from_parts(60_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(4))
    }

    /// force_process_expirations:
    /// reads: ExpiryQueue, Orders(×N), Escrow(×N) = ~1 + 2N
    /// writes: ExpiryQueue, Orders(×N), Escrow(×N) = ~1 + 2N
    /// 最坏情况 N=500
    fn force_process_expirations() -> Weight {
        Weight::from_parts(200_000_000, 20_000)
            .saturating_add(T::DbWeight::get().reads(501))
            .saturating_add(T::DbWeight::get().writes(501))
    }

    /// place_order_for: same as place_order + PayerOrders write
    fn place_order_for() -> Weight {
        Weight::from_parts(130_000_000, 18_000)
            .saturating_add(T::DbWeight::get().reads(14))
            .saturating_add(T::DbWeight::get().writes(10))
    }

    /// cleanup_payer_orders: same pattern as cleanup_buyer_orders
    fn cleanup_payer_orders() -> Weight {
        Weight::from_parts(80_000_000, 8_000)
            .saturating_add(T::DbWeight::get().reads(101))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// 单元测试用零权重实现
impl WeightInfo for () {
    fn place_order() -> Weight { Weight::zero() }
    fn cancel_order() -> Weight { Weight::zero() }
    fn ship_order() -> Weight { Weight::zero() }
    fn confirm_receipt() -> Weight { Weight::zero() }
    fn request_refund() -> Weight { Weight::zero() }
    fn approve_refund() -> Weight { Weight::zero() }
    fn start_service() -> Weight { Weight::zero() }
    fn complete_service() -> Weight { Weight::zero() }
    fn confirm_service() -> Weight { Weight::zero() }
    fn set_platform_fee_rate() -> Weight { Weight::zero() }
    fn cleanup_buyer_orders() -> Weight { Weight::zero() }
    fn reject_refund() -> Weight { Weight::zero() }
    fn seller_cancel_order() -> Weight { Weight::zero() }
    fn force_refund() -> Weight { Weight::zero() }
    fn force_complete() -> Weight { Weight::zero() }
    fn update_shipping_address() -> Weight { Weight::zero() }
    fn extend_confirm_timeout() -> Weight { Weight::zero() }
    fn cleanup_shop_orders() -> Weight { Weight::zero() }
    fn update_tracking() -> Weight { Weight::zero() }
    fn seller_refund_order() -> Weight { Weight::zero() }
    fn force_partial_refund() -> Weight { Weight::zero() }
    fn withdraw_dispute() -> Weight { Weight::zero() }
    fn force_process_expirations() -> Weight { Weight::zero() }
    fn place_order_for() -> Weight { Weight::zero() }
    fn cleanup_payer_orders() -> Weight { Weight::zero() }
}
