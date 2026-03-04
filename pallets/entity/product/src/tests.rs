use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok};
use pallet_entity_common::{AdminPermission, ProductCategory, ProductStatus, ProductVisibility};

// ==================== 辅助宏 ====================

fn create_default_product() {
    assert_ok!(EntityProduct::create_product(
        RuntimeOrigin::signed(1),
        1,
        b"QmName".to_vec(),
        b"QmImages".to_vec(),
        b"QmDetail".to_vec(),
        1_000_000_000_000u128,
        0,        // usdt_price
        100,
        ProductCategory::Physical,
        0,        // sort_weight
        vec![],   // tags_cid
        vec![],   // sku_cid
        1,        // min_order_quantity
        0,        // max_order_quantity (unlimited)
        ProductVisibility::Public,
    ));
}

// ==================== create_product 测试 ====================

#[test]
fn create_product_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1),
            1,
            b"QmName".to_vec(),
            b"QmImages".to_vec(),
            b"QmDetail".to_vec(),
            1_000_000_000_000u128,
            500_000,     // usdt_price
            100,
            ProductCategory::Physical,
            10,          // sort_weight
            b"QmTags".to_vec(),
            b"QmSku".to_vec(),
            2,           // min_order_quantity
            10,          // max_order_quantity
            ProductVisibility::Public,
        ));

        let product = Products::<Test>::get(0).expect("product should exist");
        assert_eq!(product.id, 0);
        assert_eq!(product.shop_id, 1);
        assert_eq!(product.price, 1_000_000_000_000u128);
        assert_eq!(product.usdt_price, 500_000);
        assert_eq!(product.stock, 100);
        assert_eq!(product.status, ProductStatus::Draft);
        assert_eq!(product.category, ProductCategory::Physical);
        assert_eq!(product.sort_weight, 10);
        assert_eq!(product.tags_cid.to_vec(), b"QmTags".to_vec());
        assert_eq!(product.sku_cid.to_vec(), b"QmSku".to_vec());
        assert_eq!(product.min_order_quantity, 2);
        assert_eq!(product.max_order_quantity, 10);
        assert_eq!(product.visibility, ProductVisibility::Public);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.total_products, 1);
        assert_eq!(stats.on_sale_products, 0);

        assert_eq!(NextProductId::<Test>::get(), 1);

        let deposit_info = ProductDeposits::<Test>::get(0).expect("deposit should exist");
        assert_eq!(deposit_info.shop_id, 1);
        assert!(deposit_info.amount > 0);
        assert_eq!(deposit_info.source_account, 110);
    });
}

#[test]
fn create_product_fails_zero_price() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                0u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::InvalidPrice
        );
    });
}

#[test]
fn create_product_fails_shop_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 999,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

#[test]
fn create_product_fails_shop_not_active() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(2), 2,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::ShopNotActive
        );
    });
}

#[test]
fn create_product_fails_not_authorized() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(5), 1,  // 账户 5 无任何权限
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn create_product_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        let long_cid = vec![0u8; 65];
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                long_cid, b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::CidTooLong
        );
    });
}

#[test]
fn create_product_fails_max_products_reached() {
    new_test_ext().execute_with(|| {
        for _i in 0..10 {
            assert_ok!(EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ));
        }
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::MaxProductsReached
        );
    });
}

#[test]
fn create_product_infinite_stock() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 0, ProductCategory::Digital, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0);
        assert_eq!(product.category, ProductCategory::Digital);
    });
}

#[test]
fn update_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1),
            0,
            Some(b"QmNewName".to_vec()),
            None,
            None,
            Some(2_000_000_000_000u128),
            Some(500_000u64),
            Some(200),
            Some(ProductCategory::Digital),
            Some(5),
            Some(b"QmTags".to_vec()),
            Some(b"QmSku".to_vec()),
            None, None, None,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.name_cid.to_vec(), b"QmNewName".to_vec());
        assert_eq!(product.price, 2_000_000_000_000u128);
        assert_eq!(product.usdt_price, 500_000);
        assert_eq!(product.stock, 200);
        assert_eq!(product.category, ProductCategory::Digital);
        assert_eq!(product.sort_weight, 5);
        assert_eq!(product.tags_cid.to_vec(), b"QmTags".to_vec());
        assert_eq!(product.sku_cid.to_vec(), b"QmSku".to_vec());
        // images_cid 未变
        assert_eq!(product.images_cid.to_vec(), b"QmImages".to_vec());
    });
}

#[test]
fn update_product_fails_not_authorized() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(5), // 无权限
                0,
                Some(b"QmNew".to_vec()),
                None, None, None, None, None, None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn update_product_soldout_to_onsale_on_restock() {
    new_test_ext().execute_with(|| {
        create_default_product();

        // 上架
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 通过 ProductProvider 扣减全部库存 → SoldOut
        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::SoldOut);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 0); // M4: SoldOut 减了统计

        // 补货
        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1),
            0,
            None, None, None, None, None,
            Some(50),                       // 补货 50
            None, None, None, None,
            None, None, None,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);
        assert_eq!(product.stock, 50);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 1); // M3: 补货恢复了统计
    });
}

// ==================== publish_product 测试 ====================

#[test]
fn publish_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 1);
    });
}

#[test]
fn publish_product_fails_already_on_sale() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // C3: 重复上架应失败
        assert_noop!(
            EntityProduct::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn publish_product_fails_sold_out_cannot_publish() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 扣完库存 → SoldOut
        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::SoldOut);

        // C3: SoldOut 不能直接 publish
        assert_noop!(
            EntityProduct::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn publish_product_from_offshelf_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);

        // 从 OffShelf 重新上架
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 1);
    });
}

#[test]
fn publish_product_fails_shop_not_active() {
    new_test_ext().execute_with(|| {
        create_default_product();
        set_shop_active(1, false);

        assert_noop!(
            EntityProduct::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::ShopNotActive
        );
    });
}

// ==================== unpublish_product 测试 ====================

#[test]
fn unpublish_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 0);
    });
}

#[test]
fn unpublish_product_fails_draft_status() {
    new_test_ext().execute_with(|| {
        create_default_product();

        // C4: Draft 状态不能下架
        assert_noop!(
            EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn unpublish_product_fails_already_offshelf() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));

        // C4: OffShelf 不能再次下架
        assert_noop!(
            EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn unpublish_soldout_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);

        // C4: SoldOut 可以下架，但不减 on_sale_products（已被 deduct_stock 减过）
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 0);
    });
}

// ==================== delete_product 测试 ====================

#[test]
fn delete_product_draft_works() {
    new_test_ext().execute_with(|| {
        create_default_product();

        let shop_balance_before = pallet_balances::Pallet::<Test>::free_balance(110);

        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));

        assert!(Products::<Test>::get(0).is_none());
        assert!(ProductDeposits::<Test>::get(0).is_none());

        let shop_balance_after = pallet_balances::Pallet::<Test>::free_balance(110);
        assert!(shop_balance_after > shop_balance_before); // 押金退还

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.total_products, 0);
    });
}

#[test]
fn delete_product_offshelf_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));
        assert!(Products::<Test>::get(0).is_none());
    });
}

#[test]
fn delete_product_fails_on_sale() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_noop!(
            EntityProduct::delete_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn delete_product_fails_sold_out() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        assert_noop!(
            EntityProduct::delete_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn delete_product_fails_not_authorized() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityProduct::delete_product(RuntimeOrigin::signed(5), 0),
            Error::<Test>::NotAuthorized
        );
    });
}

// ==================== ProductProvider trait 测试 ====================

#[test]
fn product_provider_basic_queries() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert!(<EntityProduct as ProductProvider<u64, u128>>::product_exists(0));
        assert!(!<EntityProduct as ProductProvider<u64, u128>>::product_exists(999));

        assert!(<EntityProduct as ProductProvider<u64, u128>>::is_product_on_sale(0));

        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_shop_id(0),
            Some(1)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_price(0),
            Some(1_000_000_000_000u128)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_stock(0),
            Some(100)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_category(0),
            Some(ProductCategory::Physical)
        );
    });
}

#[test]
fn deduct_stock_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 30));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 70);
        assert_eq!(product.status, ProductStatus::OnSale);
    });
}

#[test]
fn deduct_stock_to_zero_becomes_sold_out() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0);
        assert_eq!(product.status, ProductStatus::SoldOut);

        // M4: 统计应已更新
        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 0);
    });
}

#[test]
fn deduct_stock_fails_insufficient() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 101),
            Error::<Test>::InsufficientStock
        );
    });
}

#[test]
fn deduct_stock_infinite_stock_no_change() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        // 创建无限库存商品
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 0, ProductCategory::Digital, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        // 上架后测试
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // deduct_stock 对无限库存不起作用
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 999));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0); // 仍然是 0
        assert_eq!(product.status, ProductStatus::OnSale);
    });
}

#[test]
fn restore_stock_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        // SoldOut 后恢复库存
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 50));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 50);
        assert_eq!(product.status, ProductStatus::OnSale);

        // M4: 恢复库存后统计更新
        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 1);
    });
}

#[test]
fn restore_stock_infinite_no_change() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 0, ProductCategory::Digital, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        // 上架后测试
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 无限库存 restore 不起作用
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 50));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0); // 仍然是 0（无限库存）
    });
}

#[test]
fn add_sold_count_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();

        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::add_sold_count(0, 5));
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::add_sold_count(0, 3));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.sold_count, 8);
    });
}

// ==================== 押金机制测试 ====================

#[test]
fn deposit_calculation_respects_pricing() {
    new_test_ext().execute_with(|| {
        // 默认价格 1 USDT/NEX => 1 USDT = 1 NEX = 1_000_000_000_000
        let deposit = EntityProduct::calculate_product_deposit().unwrap();
        // 1_000_000 * 10^12 / 1_000_000 = 1_000_000_000_000
        assert_eq!(deposit, 1_000_000_000_000u128);
    });
}

#[test]
fn deposit_respects_min_bound() {
    new_test_ext().execute_with(|| {
        // 设置极高价格 => 押金极小 => 应被 min 限制
        // nex = 1_000_000 * 10^12 / price, 需要 price 极大使 nex < 100
        // nex < 100 => price > 1_000_000 * 10^12 / 100 = 10_000_000_000_000_000
        set_pricing(100_000_000_000_000_000);
        let deposit = EntityProduct::calculate_product_deposit().unwrap();
        // min bound = 100
        assert_eq!(deposit, 100u128);
    });
}

#[test]
fn deposit_respects_max_bound() {
    new_test_ext().execute_with(|| {
        set_pricing(1); // 极低价格 => 极大押金
        let deposit = EntityProduct::calculate_product_deposit().unwrap();
        // max bound = 10_000_000_000_000
        assert_eq!(deposit, 10_000_000_000_000u128);
    });
}

#[test]
fn deposit_fails_when_price_zero() {
    new_test_ext().execute_with(|| {
        set_pricing(0);
        assert_noop!(
            EntityProduct::calculate_product_deposit(),
            Error::<Test>::PriceUnavailable
        );
    });
}

#[test]
fn delete_product_refunds_deposit() {
    new_test_ext().execute_with(|| {
        let shop_account = 110u64; // shop_account(1)
        let balance_before = pallet_balances::Pallet::<Test>::free_balance(shop_account);

        create_default_product();

        let balance_after_create = pallet_balances::Pallet::<Test>::free_balance(shop_account);
        let deposit = balance_before - balance_after_create;
        assert!(deposit > 0);

        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));

        let balance_after_delete = pallet_balances::Pallet::<Test>::free_balance(shop_account);
        assert_eq!(balance_after_delete, balance_before);
    });
}

// ==================== on_sale_products 统计一致性测试 ====================

#[test]
fn on_sale_products_stats_consistency() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        // 创建 3 个商品
        for _ in 0..3 {
            create_default_product();
        }

        // 全部上架
        for i in 0..3 {
            assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), i));
        }
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 3);

        // 商品 0: 售罄 (M4: on_sale -1)
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 2);

        // 商品 0: 恢复库存 (M4: on_sale +1)
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 10));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 3);

        // 商品 1: 下架 (on_sale -1)
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 1));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 2);

        // 商品 2: 扣光 → SoldOut → 下架 (on_sale 不变，因为 deduct 已减过)
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(2, 100));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 1);
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 2));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 1); // SoldOut 下架不减

        // 商品 1: 重新上架 (on_sale +1)
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 1));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 2);

        // 删除商品 0（OnSale → 删除需先下架）
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 1);
        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));
        assert_eq!(ProductStats::<Test>::get().total_products, 2);
    });
}

// ==================== M4: update_product zero price ====================

#[test]
fn update_product_fails_zero_price() {
    new_test_ext().execute_with(|| {
        create_default_product();

        // M4: 更新价格为 0 应失败
        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1),
                0,
                None, None, None,
                Some(0u128),  // price = 0
                None, None, None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::InvalidPrice
        );
    });
}

// ==================== ShopProducts 索引测试 ====================

#[test]
fn shop_products_index_correct() {
    new_test_ext().execute_with(|| {
        create_default_product(); // product 0
        create_default_product(); // product 1

        let shop_products = ShopProducts::<Test>::get(1);
        assert_eq!(shop_products.to_vec(), vec![0, 1]);

        // 删除 product 0
        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));
        let shop_products = ShopProducts::<Test>::get(1);
        assert_eq!(shop_products.to_vec(), vec![1]);
    });
}

// ==================== H1: 补货检查 Shop 激活状态 ====================

#[test]
fn h1_update_product_restock_fails_shop_not_active() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 扣光库存 → SoldOut
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);

        // Shop 变为不激活
        set_shop_active(1, false);

        // H1: 补货应失败，因为 Shop 未激活
        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1),
                0,
                None, None, None, None, None,
                Some(50), // 补货
                None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::ShopNotActive
        );

        // 商品应仍为 SoldOut
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);
    });
}

#[test]
fn h1_update_product_restock_works_when_shop_active() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        // Shop 激活状态下补货应成功
        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1),
            0,
            None, None, None, None, None,
            Some(50),
            None, None, None, None,
            None, None, None,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);
        assert_eq!(product.stock, 50);
    });
}

// ==================== H2: 空 CID 检查 ====================

#[test]
fn h2_create_product_rejects_empty_name_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                vec![],  // 空 name_cid
                b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn h2_update_product_rejects_empty_name_cid() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1),
                0,
                Some(vec![]),  // 空 name_cid
                None, None, None, None, None, None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::EmptyCid
        );

        // 确认 name_cid 未被修改
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.name_cid.to_vec(), b"QmName".to_vec());
    });
}

// ==================== M2: StockUpdated 事件 ====================

#[test]
fn m2_deduct_stock_emits_stock_updated_event() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        System::reset_events();
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 30));

        let events = System::events();
        let found = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityProduct(Event::StockUpdated { product_id: 0, new_stock: 70 })
            )
        });
        assert!(found, "StockUpdated event should be emitted by deduct_stock");
    });
}

#[test]
fn m2_restore_stock_emits_stock_updated_event() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        System::reset_events();
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 25));

        let events = System::events();
        let found = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityProduct(Event::StockUpdated { product_id: 0, new_stock: 25 })
            )
        });
        assert!(found, "StockUpdated event should be emitted by restore_stock");
    });
}

// ==================== 审计修复回归测试 ====================

#[test]
fn deduct_stock_rejects_draft_product() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product(); // status = Draft
        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 10),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn deduct_stock_rejects_offshelf_product() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));
        // status = OffShelf
        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 10),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn deduct_stock_rejects_soldout_product() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        // 扣完库存使其 SoldOut
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::SoldOut);
        // 再次扣减应失败
        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 1),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn restore_stock_rejects_draft_product() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product(); // status = Draft
        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 10),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn restore_stock_works_for_offshelf_product() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 50));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));
        // OffShelf 状态下恢复库存应成功，但不改变状态
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 50));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);
        assert_eq!(product.stock, 100);
    });
}

#[test]
fn restore_stock_soldout_to_onsale() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::SoldOut);
        // 恢复库存应将 SoldOut -> OnSale
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 25));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OnSale);
        assert_eq!(product.stock, 25);
    });
}

#[test]
fn h1_delete_product_succeeds_without_deposit_record() {
    new_test_ext().execute_with(|| {
        create_default_product();
        // 手动移除押金记录
        ProductDeposits::<Test>::remove(0);
        // H1: 押金记录缺失不阻断删除（best-effort, deposit_refunded=0）
        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));
        // 商品应已删除
        assert!(Products::<Test>::get(0).is_none());
        assert_eq!(ProductStats::<Test>::get().total_products, 0);
    });
}

// ==================== H1: restore_stock OffShelf+stock=0 ====================

#[test]
fn h1_restore_stock_works_for_offshelf_soldout_product() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product(); // stock=100
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 全部售出 → SoldOut
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);

        // 店主下架 → OffShelf (stock=0)
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);
        assert_eq!(product.stock, 0);

        // H1: 订单取消恢复库存 — 之前此处会静默丢弃
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 30));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 30);
        // 状态保持 OffShelf（店主手动下架的，不应自动上架）
        assert_eq!(product.status, ProductStatus::OffShelf);
    });
}

// ==================== H2: SoldCountUpdated 事件 ====================

#[test]
fn h2_add_sold_count_emits_sold_count_updated_event() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();

        System::reset_events();
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::add_sold_count(0, 5));

        let events = System::events();
        // 应发出 SoldCountUpdated 而非 StockUpdated
        let found_sold = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityProduct(Event::SoldCountUpdated { product_id: 0, sold_count: 5 })
            )
        });
        assert!(found_sold, "SoldCountUpdated event should be emitted");

        let found_stock = events.iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::EntityProduct(Event::StockUpdated { .. })
            )
        });
        assert!(!found_stock, "StockUpdated should NOT be emitted by add_sold_count");
    });
}

// ==================== M1: OnSale stock=0 防护 ====================

#[test]
fn m1_update_product_rejects_zero_stock_while_on_sale() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 在售商品不可将 stock 设为 0（避免隐式转无限库存）
        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1),
                0,
                None, None, None, None, None,
                Some(0), // stock=0 on OnSale
                None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::CannotClearStockWhileOnSale
        );

        // stock 应未变
        assert_eq!(Products::<Test>::get(0).unwrap().stock, 100);
    });
}

#[test]
fn m1_update_product_allows_zero_stock_on_draft() {
    new_test_ext().execute_with(|| {
        create_default_product(); // Draft

        // Draft 状态可以设置 stock=0（表示无限库存）
        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1),
            0,
            None, None, None, None, None,
            Some(0),
            None, None, None, None,
            None, None, None,
        ));
        assert_eq!(Products::<Test>::get(0).unwrap().stock, 0);
    });
}

// ==================== M2: 空 CID 校验 ====================

#[test]
fn m2_create_product_rejects_empty_images_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(),
                vec![],  // 空 images_cid
                b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn m2_create_product_rejects_empty_detail_cid() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(), b"QmImages".to_vec(),
                vec![],  // 空 detail_cid
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn m2_update_product_rejects_empty_images_cid() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1),
                0,
                None,
                Some(vec![]),  // 空 images_cid
                None, None, None, None, None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

#[test]
fn m2_update_product_rejects_empty_detail_cid() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1),
                0,
                None, None,
                Some(vec![]),  // 空 detail_cid
                None, None, None, None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::EmptyCid
        );
    });
}

// ==================== IPFS Pin 集成测试 ====================

#[test]
fn ipfs_create_product_pins_three_cids() {
    new_test_ext().execute_with(|| {
        clear_pin_tracking();

        create_default_product();

        let pinned = get_pinned_cids();
        assert_eq!(pinned.len(), 3, "create_product should pin 3 CIDs");
        // 所有 CID 关联到 product_id=0
        assert!(pinned.iter().all(|(id, _)| *id == 0));
        assert_eq!(pinned[0].1, b"QmName".to_vec());
        assert_eq!(pinned[1].1, b"QmImages".to_vec());
        assert_eq!(pinned[2].1, b"QmDetail".to_vec());
        assert!(get_unpinned_cids().is_empty());
    });
}

#[test]
fn ipfs_update_product_unpins_old_pins_new() {
    new_test_ext().execute_with(|| {
        create_default_product();
        clear_pin_tracking();

        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1),
            0,
            Some(b"QmNewName".to_vec()),
            None,
            Some(b"QmNewDetail".to_vec()),
            None, None, None, None, None, None, None,
            None, None, None,
        ));

        let pinned = get_pinned_cids();
        let unpinned = get_unpinned_cids();

        // 2 CIDs changed → 2 unpins + 2 pins
        assert_eq!(unpinned.len(), 2);
        assert_eq!(pinned.len(), 2);
        // 旧 CID 被 unpin
        assert!(unpinned.contains(&b"QmName".to_vec()));
        assert!(unpinned.contains(&b"QmDetail".to_vec()));
        // 新 CID 被 pin
        assert!(pinned.iter().any(|(_, c)| c == &b"QmNewName".to_vec()));
        assert!(pinned.iter().any(|(_, c)| c == &b"QmNewDetail".to_vec()));
    });
}

#[test]
fn ipfs_update_product_unchanged_fields_no_pin() {
    new_test_ext().execute_with(|| {
        create_default_product();
        clear_pin_tracking();

        // 仅更新价格和库存，CID 不变 → 不应有 pin/unpin
        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1),
            0,
            None, None, None,
            Some(2_000u128),
            None,
            Some(50),
            None, None, None, None,
            None, None, None,
        ));

        assert!(get_pinned_cids().is_empty(), "no CID changed, no pin expected");
        assert!(get_unpinned_cids().is_empty(), "no CID changed, no unpin expected");
    });
}

#[test]
fn ipfs_delete_product_unpins_all_cids() {
    new_test_ext().execute_with(|| {
        create_default_product();
        clear_pin_tracking();

        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));

        let unpinned = get_unpinned_cids();
        assert_eq!(unpinned.len(), 3, "delete_product should unpin 3 CIDs");
        assert!(unpinned.contains(&b"QmName".to_vec()));
        assert!(unpinned.contains(&b"QmImages".to_vec()));
        assert!(unpinned.contains(&b"QmDetail".to_vec()));
    });
}

#[test]
fn ipfs_pin_failure_does_not_block_create() {
    new_test_ext().execute_with(|| {
        set_pin_should_fail(true);

        // pin 失败不应阻断商品创建
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000_000_000_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        // 商品应正常存在
        assert!(Products::<Test>::get(0).is_some());
        assert_eq!(NextProductId::<Test>::get(), 1);

        set_pin_should_fail(false);
    });
}

// ==================== M3: stale price 路径 ====================

#[test]
fn m3_stale_price_returns_min_deposit() {
    new_test_ext().execute_with(|| {
        // 正常价格 => 正常押金
        let normal_deposit = EntityProduct::calculate_product_deposit().unwrap();
        assert_eq!(normal_deposit, 1_000_000_000_000u128); // 1 UNIT

        // 标记价格过时 => 返回 min_deposit
        set_price_stale(true);
        let stale_deposit = EntityProduct::calculate_product_deposit().unwrap();
        assert_eq!(stale_deposit, 100u128); // MinProductDepositCos

        // 恢复
        set_price_stale(false);
        let restored = EntityProduct::calculate_product_deposit().unwrap();
        assert_eq!(restored, normal_deposit);
    });
}

// ==================== 审计 Round 2 回归测试 ====================

#[test]
fn h1_delete_product_succeeds_when_pallet_insolvent() {
    new_test_ext().execute_with(|| {
        create_default_product();
        // 手动清空 Pallet 账户余额，模拟偿付能力不足
        let pallet_account: u64 = sp_runtime::traits::AccountIdConversion::<u64>::into_account_truncating(
            &frame_support::PalletId(*b"et/prod/")
        );
        let pallet_balance = Balances::free_balance(pallet_account);
        if pallet_balance > 0 {
            let _ = <Balances as frame_support::traits::Currency<u64>>::slash(&pallet_account, pallet_balance);
        }
        // H1: Pallet 偿付能力不足不阻断删除
        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(1), 0));
        assert!(Products::<Test>::get(0).is_none());
        assert_eq!(ProductStats::<Test>::get().total_products, 0);
    });
}

#[test]
fn h2_restore_stock_soldout_inactive_shop_stays_soldout() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        // 创建有限库存商品并上架
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"Qm1".to_vec(), b"Qm2".to_vec(), b"Qm3".to_vec(),
            100_000_000_000_000u128, 0, 10, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 扣减全部库存 → SoldOut
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 10));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);

        // 关闭 Shop
        set_shop_active(1, false);

        // H2: restore_stock 应增加库存但不恢复 OnSale（Shop 未激活）
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 5));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 5);
        assert_eq!(product.status, ProductStatus::SoldOut); // 仍为 SoldOut

        // 恢复 Shop → 再次 restore_stock，SoldOut + Shop active → OnSale
        set_shop_active(1, true);
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, 3));
        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 8);
        assert_eq!(product.status, ProductStatus::OnSale); // Shop 激活时恢复 OnSale
    });
}

#[test]
fn m2_create_product_fails_insufficient_balance_with_ed() {
    new_test_ext().execute_with(|| {
        // 押金 = 1_000_000_000_000 (1 UNIT), ED = 1
        // 给 shop 账户恰好 deposit + ED - 1 的余额（不够 KeepAlive）
        let shop_account: u64 = 1 * 100 + 10; // mock shop_account derivation
        let deposit = EntityProduct::calculate_product_deposit().unwrap();
        let ed = <Balances as frame_support::traits::Currency<u64>>::minimum_balance();
        // 清空 shop 账户后设置精确余额
        let current = Balances::free_balance(shop_account);
        if current > 0 {
            let _ = <Balances as frame_support::traits::Currency<u64>>::slash(&shop_account, current);
        }
        let _ = <Balances as frame_support::traits::Currency<u64>>::deposit_creating(&shop_account, deposit + ed - 1);

        // M2: 应返回 InsufficientShopFund（而非 transfer 内部错误）
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"Qm1".to_vec(), b"Qm2".to_vec(), b"Qm3".to_vec(),
                100_000_000_000_000u128, 0, 10, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::InsufficientShopFund
        );
    });
}

#[test]
fn m3_restore_stock_overflow_rejected() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        // 创建有限库存商品并上架
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"Qm1".to_vec(), b"Qm2".to_vec(), b"Qm3".to_vec(),
            100_000_000_000_000u128, 0, 10, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // M3: restore_stock u32::MAX 溢出应报错
        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::restore_stock(0, u32::MAX),
            Error::<Test>::StockOverflow
        );
        // 库存应未变
        assert_eq!(Products::<Test>::get(0).unwrap().stock, 10);
    });
}

// ==================== Admin 权限测试 ====================

#[test]
fn admin_can_create_product() {
    new_test_ext().execute_with(|| {
        // 账户 3 是 entity 1 的 Admin (SHOP_MANAGE)
        add_entity_admin(1, 3, AdminPermission::SHOP_MANAGE);

        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(3), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.shop_id, 1);
    });
}

#[test]
fn admin_can_update_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        add_entity_admin(1, 3, AdminPermission::SHOP_MANAGE);

        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(3), 0,
            Some(b"QmAdminName".to_vec()),
            None, None, None, None, None, None, None, None, None,
            None, None, None,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.name_cid.to_vec(), b"QmAdminName".to_vec());
    });
}

#[test]
fn admin_can_publish_unpublish_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        add_entity_admin(1, 3, AdminPermission::SHOP_MANAGE);

        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(3), 0));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::OnSale);

        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(3), 0));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::OffShelf);
    });
}

#[test]
fn admin_can_delete_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        add_entity_admin(1, 3, AdminPermission::SHOP_MANAGE);

        assert_ok!(EntityProduct::delete_product(RuntimeOrigin::signed(3), 0));
        assert!(Products::<Test>::get(0).is_none());
    });
}

#[test]
fn admin_without_shop_manage_cannot_create() {
    new_test_ext().execute_with(|| {
        // Admin 有 MEMBER_MANAGE 但无 SHOP_MANAGE
        add_entity_admin(1, 3, AdminPermission::MEMBER_MANAGE);

        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(3), 1,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

// ==================== Manager 权限测试 ====================

#[test]
fn manager_can_create_product() {
    new_test_ext().execute_with(|| {
        add_shop_manager(1, 4);

        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(4), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 50, ProductCategory::Digital, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        assert!(Products::<Test>::get(0).is_some());
    });
}

#[test]
fn manager_can_publish_unpublish() {
    new_test_ext().execute_with(|| {
        create_default_product();
        add_shop_manager(1, 4);

        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(4), 0));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(4), 0));
    });
}

#[test]
fn manager_cannot_delete_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        add_shop_manager(1, 4);

        // delete_product 仅 Owner/Admin，不允许 Manager
        assert_noop!(
            EntityProduct::delete_product(RuntimeOrigin::signed(4), 0),
            Error::<Test>::NotAuthorized
        );
    });
}

// ==================== force_unpublish_product 测试 ====================

#[test]
fn force_unpublish_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(EntityProduct::force_unpublish_product(
            RuntimeOrigin::root(), 0, Some(b"violation".to_vec()),
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);
    });
}

#[test]
fn force_unpublish_product_fails_not_root() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        assert_noop!(
            EntityProduct::force_unpublish_product(
                RuntimeOrigin::signed(1), 0, Some(b"reason".to_vec()),
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_unpublish_product_fails_not_on_sale() {
    new_test_ext().execute_with(|| {
        create_default_product(); // Draft

        assert_noop!(
            EntityProduct::force_unpublish_product(
                RuntimeOrigin::root(), 0, Some(b"reason".to_vec()),
            ),
            Error::<Test>::InvalidProductStatus
        );
    });
}

// ==================== batch 操作测试 ====================

#[test]
fn batch_publish_products_works() {
    new_test_ext().execute_with(|| {
        for _ in 0..3 {
            create_default_product();
        }

        assert_ok!(EntityProduct::batch_publish_products(
            RuntimeOrigin::signed(1), vec![0, 1, 2],
        ));

        for i in 0..3 {
            assert_eq!(Products::<Test>::get(i).unwrap().status, ProductStatus::OnSale);
        }
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 3);
    });
}

#[test]
fn batch_unpublish_products_works() {
    new_test_ext().execute_with(|| {
        for _ in 0..3 {
            create_default_product();
        }
        assert_ok!(EntityProduct::batch_publish_products(
            RuntimeOrigin::signed(1), vec![0, 1, 2],
        ));

        assert_ok!(EntityProduct::batch_unpublish_products(
            RuntimeOrigin::signed(1), vec![0, 1, 2],
        ));

        for i in 0..3 {
            assert_eq!(Products::<Test>::get(i).unwrap().status, ProductStatus::OffShelf);
        }
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 0);
    });
}

#[test]
fn batch_delete_products_works() {
    new_test_ext().execute_with(|| {
        for _ in 0..3 {
            create_default_product();
        }

        assert_ok!(EntityProduct::batch_delete_products(
            RuntimeOrigin::signed(1), vec![0, 1, 2],
        ));

        for i in 0..3u64 {
            assert!(Products::<Test>::get(i).is_none());
        }
        assert_eq!(ProductStats::<Test>::get().total_products, 0);
    });
}

#[test]
fn batch_publish_fails_too_large() {
    new_test_ext().execute_with(|| {
        // MaxBatchSize = 20, 尝试 21 个
        let ids: Vec<u64> = (0..21).collect();
        assert_noop!(
            EntityProduct::batch_publish_products(
                RuntimeOrigin::signed(1), ids,
            ),
            Error::<Test>::BatchTooLarge
        );
    });
}

// ==================== ProductProvider 扩展方法测试 ====================

#[test]
fn product_provider_status_query() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_status(0),
            Some(ProductStatus::Draft)
        );

        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_status(0),
            Some(ProductStatus::OnSale)
        );

        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_status(999),
            None
        );
    });
}

#[test]
fn product_provider_usdt_price_query() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"Qm1".to_vec(), b"Qm2".to_vec(), b"Qm3".to_vec(),
            1_000u128, 500_000, 10, ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
        ));

        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_usdt_price(0),
            Some(500_000)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_usdt_price(999),
            None
        );
    });
}

#[test]
fn product_provider_owner_query() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        // shop 1 的 owner 是账户 1
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_owner(0),
            Some(1)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_owner(999),
            None
        );
    });
}

#[test]
fn product_provider_shop_product_ids_query() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product(); // product 0
        create_default_product(); // product 1

        let ids = <EntityProduct as ProductProvider<u64, u128>>::shop_product_ids(1);
        assert_eq!(ids, vec![0, 1]);

        let empty = <EntityProduct as ProductProvider<u64, u128>>::shop_product_ids(999);
        assert!(empty.is_empty());
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_create_product() {
    new_test_ext().execute_with(|| {
        set_entity_locked(1);
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(), b"QmImg".to_vec(), b"QmDet".to_vec(),
                1_000_000_000_000u128, 0, 100,
                ProductCategory::Physical, 0, vec![], vec![], 1, 0, ProductVisibility::Public,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_update_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        set_entity_locked(1);
        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1), 0,
                Some(b"QmNew".to_vec()), None, None, None, None, None, None, None, None, None,
                None, None, None,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_publish_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        set_entity_locked(1);
        assert_noop!(
            EntityProduct::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_unpublish_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        set_entity_locked(1);
        assert_noop!(
            EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_delete_product() {
    new_test_ext().execute_with(|| {
        create_default_product();
        set_entity_locked(1);
        assert_noop!(
            EntityProduct::delete_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== 购买数量限制测试 ====================

#[test]
fn create_product_with_order_quantity_limits() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
            5,   // min_order_quantity
            20,  // max_order_quantity
            ProductVisibility::Public,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.min_order_quantity, 5);
        assert_eq!(product.max_order_quantity, 20);
    });
}

#[test]
fn create_product_fails_invalid_order_quantity() {
    new_test_ext().execute_with(|| {
        // max < min 且两者都 > 0 应失败
        assert_noop!(
            EntityProduct::create_product(
                RuntimeOrigin::signed(1), 1,
                b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
                1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
                10,  // min_order_quantity
                5,   // max_order_quantity < min
                ProductVisibility::Public,
            ),
            Error::<Test>::InvalidOrderQuantity
        );
    });
}

#[test]
fn create_product_allows_zero_max_order_quantity() {
    new_test_ext().execute_with(|| {
        // max=0 表示不限，应通过
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
            10,  // min_order_quantity
            0,   // max_order_quantity = unlimited
            ProductVisibility::Public,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.min_order_quantity, 10);
        assert_eq!(product.max_order_quantity, 0);
    });
}

#[test]
fn update_product_order_quantity_limits() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1), 0,
            None, None, None, None, None, None, None, None, None, None,
            Some(3),   // min_order_quantity
            Some(50),  // max_order_quantity
            None,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.min_order_quantity, 3);
        assert_eq!(product.max_order_quantity, 50);
    });
}

#[test]
fn update_product_fails_invalid_order_quantity() {
    new_test_ext().execute_with(|| {
        create_default_product();

        // 设置 max < min 应失败
        assert_noop!(
            EntityProduct::update_product(
                RuntimeOrigin::signed(1), 0,
                None, None, None, None, None, None, None, None, None, None,
                Some(20),  // min
                Some(5),   // max < min
                None,
            ),
            Error::<Test>::InvalidOrderQuantity
        );
    });
}

// ==================== 可见性测试 ====================

#[test]
fn create_product_with_visibility() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
            1, 0,
            ProductVisibility::MembersOnly,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.visibility, ProductVisibility::MembersOnly);
    });
}

#[test]
fn create_product_with_level_gated_visibility() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
            1, 0,
            ProductVisibility::LevelGated(3),
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.visibility, ProductVisibility::LevelGated(3));
    });
}

#[test]
fn update_product_visibility() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_ok!(EntityProduct::update_product(
            RuntimeOrigin::signed(1), 0,
            None, None, None, None, None, None, None, None, None, None,
            None, None,
            Some(ProductVisibility::MembersOnly),
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.visibility, ProductVisibility::MembersOnly);
    });
}

// ==================== ProductProvider 查询测试 ====================

#[test]
fn product_provider_visibility_query() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
            1, 0,
            ProductVisibility::LevelGated(5),
        ));

        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_visibility(0),
            Some(ProductVisibility::LevelGated(5))
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_visibility(999),
            None
        );
    });
}

#[test]
fn product_provider_order_quantity_queries() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        assert_ok!(EntityProduct::create_product(
            RuntimeOrigin::signed(1), 1,
            b"QmName".to_vec(), b"QmImages".to_vec(), b"QmDetail".to_vec(),
            1_000u128, 0, 100, ProductCategory::Physical, 0, vec![], vec![],
            2, 50,
            ProductVisibility::Public,
        ));

        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_min_order_quantity(0),
            Some(2)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_max_order_quantity(0),
            Some(50)
        );
        assert_eq!(
            <EntityProduct as ProductProvider<u64, u128>>::product_min_order_quantity(999),
            None
        );
    });
}

// ==================== 治理接口测试 ====================

#[test]
fn governance_update_price_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();

        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::update_price(0, 5_000u128));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.price, 5_000u128);
    });
}

#[test]
fn governance_update_price_rejects_zero() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();

        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::update_price(0, 0u128),
            Error::<Test>::InvalidPrice
        );
    });
}

#[test]
fn governance_update_price_rejects_nonexistent() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        assert_noop!(
            <EntityProduct as ProductProvider<u64, u128>>::update_price(999, 100u128),
            Error::<Test>::ProductNotFound
        );
    });
}

#[test]
fn governance_set_inventory_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 设置库存为 200
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::set_inventory(0, 200));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 200);
        assert_eq!(product.status, ProductStatus::OnSale);
    });
}

#[test]
fn governance_set_inventory_zero_causes_soldout() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // set_inventory(0) 应触发 SoldOut
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::set_inventory(0, 0));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0);
        assert_eq!(product.status, ProductStatus::SoldOut);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 0);
    });
}

#[test]
fn governance_set_inventory_restores_soldout_to_onsale() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 扣光库存 → SoldOut
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);

        // set_inventory 恢复库存 → OnSale
        assert_ok!(<EntityProduct as ProductProvider<u64, u128>>::set_inventory(0, 50));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 50);
        assert_eq!(product.status, ProductStatus::OnSale);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 1);
    });
}

// ==================== 批量操作 best-effort 测试 ====================

#[test]
fn batch_publish_best_effort_partial_success() {
    new_test_ext().execute_with(|| {
        // 创建 2 个商品
        create_default_product(); // product 0 (Draft)
        create_default_product(); // product 1 (Draft)

        // product 0 上架后下架（变为 OffShelf）
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityProduct::unpublish_product(RuntimeOrigin::signed(1), 0));

        // 批量上架 [0, 1, 999]，999 不存在应跳过
        assert_ok!(EntityProduct::batch_publish_products(
            RuntimeOrigin::signed(1),
            vec![0, 1, 999],
        ));

        // 0 和 1 应成功上架
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::OnSale);
        assert_eq!(Products::<Test>::get(1).unwrap().status, ProductStatus::OnSale);

        // 应有 BatchCompleted 事件，succeeded=2, failed=1
        let events = frame_system::Pallet::<Test>::events();
        let batch_event = events.iter().rev().find(|e| {
            matches!(
                e.event,
                RuntimeEvent::EntityProduct(Event::BatchCompleted { .. })
            )
        });
        assert!(batch_event.is_some());
    });
}

#[test]
fn batch_unpublish_best_effort_partial_success() {
    new_test_ext().execute_with(|| {
        create_default_product(); // product 0
        create_default_product(); // product 1

        // 仅上架 product 0
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 0));

        // 批量下架 [0, 1]，product 1 是 Draft 应跳过
        assert_ok!(EntityProduct::batch_unpublish_products(
            RuntimeOrigin::signed(1),
            vec![0, 1],
        ));

        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::OffShelf);
        assert_eq!(Products::<Test>::get(1).unwrap().status, ProductStatus::Draft); // 未变
    });
}

#[test]
fn batch_delete_best_effort_partial_success() {
    new_test_ext().execute_with(|| {
        create_default_product(); // product 0 (Draft)
        create_default_product(); // product 1 (Draft)

        // 上架 product 1（不可删除 OnSale 状态）
        assert_ok!(EntityProduct::publish_product(RuntimeOrigin::signed(1), 1));

        // 批量删除 [0, 1, 999]
        assert_ok!(EntityProduct::batch_delete_products(
            RuntimeOrigin::signed(1),
            vec![0, 1, 999],
        ));

        // product 0 应被删除
        assert!(Products::<Test>::get(0).is_none());
        // product 1 应仍存在（OnSale 不可删）
        assert!(Products::<Test>::get(1).is_some());
    });
}
