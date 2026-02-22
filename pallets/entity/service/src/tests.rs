use crate::{mock::*, pallet::*};
use frame_support::{assert_noop, assert_ok};
use pallet_entity_common::{ProductCategory, ProductStatus};

// ==================== create_product 测试 ====================

#[test]
fn create_product_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityService::create_product(
            RuntimeOrigin::signed(1),
            1,                              // shop_id
            b"QmName".to_vec(),
            b"QmImages".to_vec(),
            b"QmDetail".to_vec(),
            1_000_000_000_000u128,          // price: 1 UNIT
            100,                            // stock
            ProductCategory::Physical,
        ));

        let product = Products::<Test>::get(0).expect("product should exist");
        assert_eq!(product.id, 0);
        assert_eq!(product.shop_id, 1);
        assert_eq!(product.price, 1_000_000_000_000u128);
        assert_eq!(product.stock, 100);
        assert_eq!(product.status, ProductStatus::Draft);
        assert_eq!(product.category, ProductCategory::Physical);

        // 检查统计
        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.total_products, 1);
        assert_eq!(stats.on_sale_products, 0);

        // 检查 NextProductId 递增
        assert_eq!(NextProductId::<Test>::get(), 1);

        // 检查押金记录
        let deposit_info = ProductDeposits::<Test>::get(0).expect("deposit should exist");
        assert_eq!(deposit_info.shop_id, 1);
        assert!(deposit_info.amount > 0);
        assert_eq!(deposit_info.source_account, 110); // shop_account(1)
    });
}

#[test]
fn create_product_fails_zero_price() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityService::create_product(
                RuntimeOrigin::signed(1),
                1,
                b"QmName".to_vec(),
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                0u128,                      // price = 0
                100,
                ProductCategory::Physical,
            ),
            Error::<Test>::InvalidPrice
        );
    });
}

#[test]
fn create_product_fails_shop_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityService::create_product(
                RuntimeOrigin::signed(1),
                999,                        // 不存在的 shop
                b"QmName".to_vec(),
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                1_000u128,
                100,
                ProductCategory::Physical,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

#[test]
fn create_product_fails_shop_not_active() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityService::create_product(
                RuntimeOrigin::signed(2),
                2,                          // shop 2 存在但不激活
                b"QmName".to_vec(),
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                1_000u128,
                100,
                ProductCategory::Physical,
            ),
            Error::<Test>::ShopNotActive
        );
    });
}

#[test]
fn create_product_fails_not_shop_owner() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityService::create_product(
                RuntimeOrigin::signed(2),   // 非 shop 1 的 owner
                1,
                b"QmName".to_vec(),
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                1_000u128,
                100,
                ProductCategory::Physical,
            ),
            Error::<Test>::NotShopOwner
        );
    });
}

#[test]
fn create_product_fails_cid_too_long() {
    new_test_ext().execute_with(|| {
        let long_cid = vec![0u8; 65]; // 超过 MaxCidLength=64
        assert_noop!(
            EntityService::create_product(
                RuntimeOrigin::signed(1),
                1,
                long_cid,
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                1_000u128,
                100,
                ProductCategory::Physical,
            ),
            Error::<Test>::CidTooLong
        );
    });
}

#[test]
fn create_product_fails_max_products_reached() {
    new_test_ext().execute_with(|| {
        // MaxProductsPerShop = 10
        for _i in 0..10 {
            assert_ok!(EntityService::create_product(
                RuntimeOrigin::signed(1),
                1,
                b"QmName".to_vec(),
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                1_000u128,
                100,
                ProductCategory::Physical,
            ));
        }
        // 第 11 个应该失败
        assert_noop!(
            EntityService::create_product(
                RuntimeOrigin::signed(1),
                1,
                b"QmName".to_vec(),
                b"QmImages".to_vec(),
                b"QmDetail".to_vec(),
                1_000u128,
                100,
                ProductCategory::Physical,
            ),
            Error::<Test>::MaxProductsReached
        );
    });
}

#[test]
fn create_product_infinite_stock() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityService::create_product(
            RuntimeOrigin::signed(1),
            1,
            b"QmName".to_vec(),
            b"QmImages".to_vec(),
            b"QmDetail".to_vec(),
            1_000u128,
            0,                              // stock = 0 = 无限库存
            ProductCategory::Digital,
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0);
        assert_eq!(product.category, ProductCategory::Digital);
    });
}

// ==================== update_product 测试 ====================

fn create_default_product() {
    assert_ok!(EntityService::create_product(
        RuntimeOrigin::signed(1),
        1,
        b"QmName".to_vec(),
        b"QmImages".to_vec(),
        b"QmDetail".to_vec(),
        1_000_000_000_000u128,
        100,
        ProductCategory::Physical,
    ));
}

#[test]
fn update_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_ok!(EntityService::update_product(
            RuntimeOrigin::signed(1),
            0,
            Some(b"QmNewName".to_vec()),
            None,
            None,
            Some(2_000_000_000_000u128),
            Some(200),
            Some(ProductCategory::Digital),
        ));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.name_cid.to_vec(), b"QmNewName".to_vec());
        assert_eq!(product.price, 2_000_000_000_000u128);
        assert_eq!(product.stock, 200);
        assert_eq!(product.category, ProductCategory::Digital);
        // images_cid 未变
        assert_eq!(product.images_cid.to_vec(), b"QmImages".to_vec());
    });
}

#[test]
fn update_product_fails_not_owner() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityService::update_product(
                RuntimeOrigin::signed(2), // 非 owner
                0,
                Some(b"QmNew".to_vec()),
                None, None, None, None, None,
            ),
            Error::<Test>::NotShopOwner
        );
    });
}

#[test]
fn update_product_soldout_to_onsale_on_restock() {
    new_test_ext().execute_with(|| {
        create_default_product();

        // 上架
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        // 通过 ProductProvider 扣减全部库存 → SoldOut
        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::SoldOut);

        let stats = ProductStats::<Test>::get();
        assert_eq!(stats.on_sale_products, 0); // M4: SoldOut 减了统计

        // 补货
        assert_ok!(EntityService::update_product(
            RuntimeOrigin::signed(1),
            0,
            None, None, None, None,
            Some(50),                       // 补货 50
            None,
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

        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

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
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        // C3: 重复上架应失败
        assert_noop!(
            EntityService::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn publish_product_fails_sold_out_cannot_publish() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        // 扣完库存 → SoldOut
        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::SoldOut);

        // C3: SoldOut 不能直接 publish
        assert_noop!(
            EntityService::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn publish_product_from_offshelf_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 0));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.status, ProductStatus::OffShelf);

        // 从 OffShelf 重新上架
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));
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
            EntityService::publish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::ShopNotActive
        );
    });
}

// ==================== unpublish_product 测试 ====================

#[test]
fn unpublish_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 0));

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
            EntityService::unpublish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn unpublish_product_fails_already_offshelf() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 0));

        // C4: OffShelf 不能再次下架
        assert_noop!(
            EntityService::unpublish_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn unpublish_soldout_product_works() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(Products::<Test>::get(0).unwrap().status, ProductStatus::SoldOut);

        // C4: SoldOut 可以下架，但不减 on_sale_products（已被 deduct_stock 减过）
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 0));

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

        assert_ok!(EntityService::delete_product(RuntimeOrigin::signed(1), 0));

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
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(EntityService::delete_product(RuntimeOrigin::signed(1), 0));
        assert!(Products::<Test>::get(0).is_none());
    });
}

#[test]
fn delete_product_fails_on_sale() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        assert_noop!(
            EntityService::delete_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn delete_product_fails_sold_out() {
    new_test_ext().execute_with(|| {
        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        use pallet_entity_common::ProductProvider;
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        assert_noop!(
            EntityService::delete_product(RuntimeOrigin::signed(1), 0),
            Error::<Test>::InvalidProductStatus
        );
    });
}

#[test]
fn delete_product_fails_not_owner() {
    new_test_ext().execute_with(|| {
        create_default_product();

        assert_noop!(
            EntityService::delete_product(RuntimeOrigin::signed(2), 0),
            Error::<Test>::NotShopOwner
        );
    });
}

// ==================== ProductProvider trait 测试 ====================

#[test]
fn product_provider_basic_queries() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        assert!(<EntityService as ProductProvider<u64, u128>>::product_exists(0));
        assert!(!<EntityService as ProductProvider<u64, u128>>::product_exists(999));

        assert!(<EntityService as ProductProvider<u64, u128>>::is_product_on_sale(0));

        assert_eq!(
            <EntityService as ProductProvider<u64, u128>>::product_shop_id(0),
            Some(1)
        );
        assert_eq!(
            <EntityService as ProductProvider<u64, u128>>::product_price(0),
            Some(1_000_000_000_000u128)
        );
        assert_eq!(
            <EntityService as ProductProvider<u64, u128>>::product_stock(0),
            Some(100)
        );
        assert_eq!(
            <EntityService as ProductProvider<u64, u128>>::product_category(0),
            Some(ProductCategory::Physical)
        );
    });
}

#[test]
fn deduct_stock_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 30));

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
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));

        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));

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

        assert_noop!(
            <EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 101),
            Error::<Test>::InsufficientStock
        );
    });
}

#[test]
fn deduct_stock_infinite_stock_no_change() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        // 创建无限库存商品
        assert_ok!(EntityService::create_product(
            RuntimeOrigin::signed(1),
            1,
            b"QmName".to_vec(),
            b"QmImages".to_vec(),
            b"QmDetail".to_vec(),
            1_000u128,
            0,                              // 无限库存
            ProductCategory::Digital,
        ));

        // deduct_stock 对无限库存不起作用
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 999));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0); // 仍然是 0
        assert_eq!(product.status, ProductStatus::Draft);
    });
}

#[test]
fn restore_stock_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 0));
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));

        // SoldOut 后恢复库存
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::restore_stock(0, 50));

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

        assert_ok!(EntityService::create_product(
            RuntimeOrigin::signed(1),
            1,
            b"QmName".to_vec(),
            b"QmImages".to_vec(),
            b"QmDetail".to_vec(),
            1_000u128,
            0,                              // 无限库存
            ProductCategory::Digital,
        ));

        // 无限库存 restore 不起作用
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::restore_stock(0, 50));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.stock, 0); // 仍然是 0（无限库存）
    });
}

#[test]
fn add_sold_count_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::ProductProvider;

        create_default_product();

        assert_ok!(<EntityService as ProductProvider<u64, u128>>::add_sold_count(0, 5));
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::add_sold_count(0, 3));

        let product = Products::<Test>::get(0).unwrap();
        assert_eq!(product.sold_count, 8);
    });
}

// ==================== 押金机制测试 ====================

#[test]
fn deposit_calculation_respects_pricing() {
    new_test_ext().execute_with(|| {
        // 默认价格 1 USDT/NEX => 1 USDT = 1 NEX = 1_000_000_000_000
        let deposit = EntityService::calculate_product_deposit().unwrap();
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
        let deposit = EntityService::calculate_product_deposit().unwrap();
        // min bound = 100
        assert_eq!(deposit, 100u128);
    });
}

#[test]
fn deposit_respects_max_bound() {
    new_test_ext().execute_with(|| {
        set_pricing(1); // 极低价格 => 极大押金
        let deposit = EntityService::calculate_product_deposit().unwrap();
        // max bound = 10_000_000_000_000
        assert_eq!(deposit, 10_000_000_000_000u128);
    });
}

#[test]
fn deposit_fails_when_price_zero() {
    new_test_ext().execute_with(|| {
        set_pricing(0);
        assert_noop!(
            EntityService::calculate_product_deposit(),
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

        assert_ok!(EntityService::delete_product(RuntimeOrigin::signed(1), 0));

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
            assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), i));
        }
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 3);

        // 商品 0: 售罄 (M4: on_sale -1)
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(0, 100));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 2);

        // 商品 0: 恢复库存 (M4: on_sale +1)
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::restore_stock(0, 10));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 3);

        // 商品 1: 下架 (on_sale -1)
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 1));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 2);

        // 商品 2: 扣光 → SoldOut → 下架 (on_sale 不变，因为 deduct 已减过)
        assert_ok!(<EntityService as ProductProvider<u64, u128>>::deduct_stock(2, 100));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 1);
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 2));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 1); // SoldOut 下架不减

        // 商品 1: 重新上架 (on_sale +1)
        assert_ok!(EntityService::publish_product(RuntimeOrigin::signed(1), 1));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 2);

        // 删除商品 0（OnSale → 删除需先下架）
        assert_ok!(EntityService::unpublish_product(RuntimeOrigin::signed(1), 0));
        assert_eq!(ProductStats::<Test>::get().on_sale_products, 1);
        assert_ok!(EntityService::delete_product(RuntimeOrigin::signed(1), 0));
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
            EntityService::update_product(
                RuntimeOrigin::signed(1),
                0,
                None, None, None,
                Some(0u128),  // price = 0
                None, None,
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
        assert_ok!(EntityService::delete_product(RuntimeOrigin::signed(1), 0));
        let shop_products = ShopProducts::<Test>::get(1);
        assert_eq!(shop_products.to_vec(), vec![1]);
    });
}
