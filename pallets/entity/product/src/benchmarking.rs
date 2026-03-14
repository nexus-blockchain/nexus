//! Benchmarking for pallet-entity-product.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! All 10 extrinsics are benchmarked.
//!
//! Signed extrinsics that depend on mock providers (ShopProvider, EntityProvider, PricingProvider)
//! use seed helpers to pre-populate storage directly.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use sp_std::vec::Vec;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::{ProductCategory, ProductStatus, ProductVisibility, ShopProvider};
use frame_support::{traits::Currency, BoundedVec, PalletId};
use sp_runtime::traits::{AccountIdConversion, Bounded};

/// 创建一个有足够余额的账户
fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let account: T::AccountId = frame_benchmarking::account(name, index, 0);
    let amount = BalanceOf::<T>::max_value() / 4u32.into();
    let _ = T::Currency::deposit_creating(&account, amount);
    account
}

/// 种子商品：直接写入存储，绕过 create_product 的外部依赖
/// 返回 (owner, shop_account, product_id)
fn seed_product<T: Config>(
    product_id: u64,
    shop_id: u64,
    status: ProductStatus,
) -> T::AccountId {
    let owner = funded_account::<T>("owner", 0);

    // 确保 shop_account 有足够余额
    let shop_account = T::ShopProvider::shop_account(shop_id);
    let amount = BalanceOf::<T>::max_value() / 4u32.into();
    let _ = T::Currency::deposit_creating(&shop_account, amount);

    // 确保 pallet 账户有余额（用于退还押金）
    let pallet_account: T::AccountId =
        frame_support::PalletId(*b"et/prod/").into_account_truncating();
    let _ = T::Currency::deposit_creating(&pallet_account, amount);

    let cid = b"QmBenchCid12345678901234567890123456789012345678".to_vec();
    let bounded_cid: BoundedVec<u8, T::MaxCidLength> =
        cid.try_into().expect("cid fits");

    let now = frame_system::Pallet::<T>::block_number();
    let product = Product {
        id: product_id,
        shop_id,
        name_cid: bounded_cid.clone(),
        images_cid: bounded_cid.clone(),
        detail_cid: bounded_cid.clone(),
        usdt_price: 1_000_000,
        stock: 100,
        sold_count: 0,
        status,
        category: ProductCategory::Physical,
        sort_weight: 0,
        tags_cid: BoundedVec::default(),
        sku_cid: BoundedVec::default(),
        min_order_quantity: 1,
        max_order_quantity: 0,
        visibility: ProductVisibility::Public,
        created_at: now,
        updated_at: now,
    };

    Products::<T>::insert(product_id, product);
    let _ = ShopProducts::<T>::try_mutate(shop_id, |ids| ids.try_push(product_id));

    // 确保 NextProductId 大于当前 product_id
    let next = product_id.saturating_add(1);
    if NextProductId::<T>::get() <= product_id {
        NextProductId::<T>::put(next);
    }

    // 记录押金信息
    let deposit: BalanceOf<T> = 1_000_000_000_000u64.try_into().unwrap_or(1u32.into());
    ProductDeposits::<T>::insert(product_id, ProductDepositInfo {
        shop_id,
        amount: deposit,
        source_account: shop_account,
    });

    // 更新统计
    ProductStats::<T>::mutate(|stats| {
        stats.total_products = stats.total_products.saturating_add(1);
        if status == ProductStatus::OnSale {
            stats.on_sale_products = stats.on_sale_products.saturating_add(1);
        }
    });

    owner
}

/// 种子 N 个商品用于批量操作 benchmark
fn seed_products_batch<T: Config>(
    shop_id: u64,
    count: u32,
    status: ProductStatus,
) -> T::AccountId {
    let owner = funded_account::<T>("owner", 0);

    let shop_account = T::ShopProvider::shop_account(shop_id);
    let amount = BalanceOf::<T>::max_value() / 4u32.into();
    let _ = T::Currency::deposit_creating(&shop_account, amount);

    let pallet_account: T::AccountId =
        frame_support::PalletId(*b"et/prod/").into_account_truncating();
    let _ = T::Currency::deposit_creating(&pallet_account, amount);

    for i in 0..count {
        let pid = i as u64;
        let cid = b"QmBenchCid12345678901234567890123456789012345678".to_vec();
        let bounded_cid: BoundedVec<u8, T::MaxCidLength> =
            cid.try_into().expect("cid fits");

        let now = frame_system::Pallet::<T>::block_number();
        let product = Product {
            id: pid,
            shop_id,
            name_cid: bounded_cid.clone(),
            images_cid: bounded_cid.clone(),
            detail_cid: bounded_cid.clone(),
            usdt_price: 1_000_000,
            stock: 100,
            sold_count: 0,
            status,
            category: ProductCategory::Physical,
            sort_weight: 0,
            tags_cid: BoundedVec::default(),
            sku_cid: BoundedVec::default(),
            min_order_quantity: 1,
            max_order_quantity: 0,
            visibility: ProductVisibility::Public,
            created_at: now,
            updated_at: now,
        };

        Products::<T>::insert(pid, product);
        let _ = ShopProducts::<T>::try_mutate(shop_id, |ids| ids.try_push(pid));

        let deposit: BalanceOf<T> = 1_000_000_000_000u64.try_into().unwrap_or(1u32.into());
        ProductDeposits::<T>::insert(pid, ProductDepositInfo {
            shop_id,
            amount: deposit,
            source_account: shop_account.clone(),
        });

        ProductStats::<T>::mutate(|stats| {
            stats.total_products = stats.total_products.saturating_add(1);
            if status == ProductStatus::OnSale {
                stats.on_sale_products = stats.on_sale_products.saturating_add(1);
            }
        });
    }

    NextProductId::<T>::put(count as u64);
    owner
}


#[benchmarks]
mod benches {
    use super::*;

    // ==================== call_index(0): create_product ====================
    #[benchmark]
    fn create_product() {
        let caller = funded_account::<T>("caller", 0);
        let shop_id: u64 = 1;

        // 确保 shop_account 有足够余额
        let shop_account = T::ShopProvider::shop_account(shop_id);
        let amount = BalanceOf::<T>::max_value() / 4u32.into();
        let _ = T::Currency::deposit_creating(&shop_account, amount);

        let cid = b"QmBenchCid12345678901234567890123456789012345678".to_vec();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            shop_id,
            cid.clone(),       // name_cid
            cid.clone(),       // images_cid
            cid.clone(),       // detail_cid
            1_000_000u64,      // usdt_price
            100u32,            // stock
            ProductCategory::Physical,
            0u32,              // sort_weight
            cid.clone(),       // tags_cid
            cid.clone(),       // sku_cid
            1u32,              // min_order_quantity
            0u32,              // max_order_quantity
            ProductVisibility::Public,
        );

        let product_id = NextProductId::<T>::get() - 1;
        assert!(Products::<T>::contains_key(product_id));
    }

    // ==================== call_index(1): update_product ====================
    #[benchmark]
    fn update_product() {
        let shop_id: u64 = 1;
        let product_id: u64 = 9999;
        let owner = seed_product::<T>(product_id, shop_id, ProductStatus::Draft);

        let new_cid = b"QmUpdatedCid234567890123456789012345678901234567".to_vec();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            product_id,
            Some(new_cid.clone()),  // name_cid
            Some(new_cid.clone()),  // images_cid
            Some(new_cid.clone()),  // detail_cid
            Some(2_000_000u64),     // usdt_price
            Some(200u32),           // stock
            Some(ProductCategory::Physical),
            Some(1u32),             // sort_weight
            Some(new_cid.clone()),  // tags_cid
            Some(new_cid.clone()),  // sku_cid
            Some(1u32),             // min_order_quantity
            Some(0u32),             // max_order_quantity
            Some(ProductVisibility::Public),
        );

        let product = Products::<T>::get(product_id).unwrap();
        assert_eq!(product.stock, 200);
    }

    // ==================== call_index(2): publish_product ====================
    #[benchmark]
    fn publish_product() {
        let shop_id: u64 = 1;
        let product_id: u64 = 9999;
        let owner = seed_product::<T>(product_id, shop_id, ProductStatus::Draft);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), product_id);

        let product = Products::<T>::get(product_id).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);
    }

    // ==================== call_index(3): unpublish_product ====================
    #[benchmark]
    fn unpublish_product() {
        let shop_id: u64 = 1;
        let product_id: u64 = 9999;
        let owner = seed_product::<T>(product_id, shop_id, ProductStatus::OnSale);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), product_id);

        let product = Products::<T>::get(product_id).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);
    }

    // ==================== call_index(4): delete_product ====================
    #[benchmark]
    fn delete_product() {
        let shop_id: u64 = 1;
        let product_id: u64 = 9999;
        let owner = seed_product::<T>(product_id, shop_id, ProductStatus::Draft);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), product_id);

        assert!(!Products::<T>::contains_key(product_id));
    }

    // ==================== call_index(5): force_unpublish_product ====================
    #[benchmark]
    fn force_unpublish_product() {
        let shop_id: u64 = 1;
        let product_id: u64 = 9999;
        let _owner = seed_product::<T>(product_id, shop_id, ProductStatus::OnSale);

        let reason = b"Benchmark force unpublish reason for testing".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Root, product_id, Some(reason));

        let product = Products::<T>::get(product_id).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);
    }

    // ==================== call_index(6): batch_publish_products ====================
    #[benchmark]
    fn batch_publish_products(n: Linear<1, 20>) {
        let shop_id: u64 = 1;
        let owner = seed_products_batch::<T>(shop_id, n, ProductStatus::Draft);

        let product_ids: Vec<u64> = (0..n as u64).collect();

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), product_ids);

        // 验证第一个商品已上架
        let product = Products::<T>::get(0u64).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);
    }

    // ==================== call_index(7): batch_unpublish_products ====================
    #[benchmark]
    fn batch_unpublish_products(n: Linear<1, 20>) {
        let shop_id: u64 = 1;
        let owner = seed_products_batch::<T>(shop_id, n, ProductStatus::OnSale);

        let product_ids: Vec<u64> = (0..n as u64).collect();

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), product_ids);

        let product = Products::<T>::get(0u64).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);
    }

    // ==================== call_index(8): batch_delete_products ====================
    #[benchmark]
    fn batch_delete_products(n: Linear<1, 20>) {
        let shop_id: u64 = 1;
        let owner = seed_products_batch::<T>(shop_id, n, ProductStatus::Draft);

        let product_ids: Vec<u64> = (0..n as u64).collect();

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), product_ids);

        assert!(!Products::<T>::contains_key(0u64));
    }

    // ==================== call_index(9): force_delete_product ====================
    #[benchmark]
    fn force_delete_product() {
        let shop_id: u64 = 1;
        let product_id: u64 = 9999;
        let _owner = seed_product::<T>(product_id, shop_id, ProductStatus::OnSale);

        let reason = b"Benchmark force delete reason for testing purposes".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Root, product_id, Some(reason));

        assert!(!Products::<T>::contains_key(product_id));
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
