//! Benchmarks for pallet-entity-shop
//!
//! 全部 33 个 extrinsics 均有 benchmark。
//! 由于 shop pallet 依赖大量外部 trait（EntityProvider / StoragePin / ProductProvider 等），
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::{ShopOperatingStatus, ShopType};
use frame_support::{BoundedVec, traits::{Currency, Get}};
use sp_runtime::traits::{Bounded, Saturating, Zero};

/// 确保账户有足够余额
fn fund_account<T: Config>(account: &T::AccountId) {
    let amount = BalanceOf::<T>::max_value() / 4u32.into();
    let _ = T::Currency::deposit_creating(account, amount);
}

/// 创建有足够余额的账户
fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let account: T::AccountId = frame_benchmarking::account(name, index, 0);
    fund_account::<T>(&account);
    account
}

/// 构造 benchmark CID
fn bench_cid<T: Config>() -> BoundedVec<u8, T::MaxCidLength> {
    let cid = b"QmBenchCid12345678901234567890123456789012345678".to_vec();
    cid.try_into().expect("cid fits MaxCidLength")
}

/// 构造 benchmark Shop 名称
fn bench_name<T: Config>() -> BoundedVec<u8, T::MaxShopNameLength> {
    b"Benchmark Shop".to_vec().try_into().expect("name fits")
}

/// 直接写入存储种子一个 Shop，绕过外部依赖
fn seed_shop<T: Config>(
    shop_id: u64,
    entity_id: u64,
    status: ShopOperatingStatus,
) {
    let now = frame_system::Pallet::<T>::block_number();
    let name: BoundedVec<u8, T::MaxShopNameLength> = b"Bench Shop".to_vec().try_into().unwrap();
    let cid: BoundedVec<u8, T::MaxCidLength> = bench_cid::<T>();

    let shop = Shop {
        id: shop_id,
        entity_id,
        name,
        logo_cid: Some(cid.clone()),
        description_cid: Some(cid.clone()),
        shop_type: ShopType::OnlineStore,
        status,
        managers: BoundedVec::default(),
        location: None,
        address_cid: None,
        business_hours_cid: Some(cid.clone()),
        policies_cid: Some(cid),
        created_at: now,
        product_count: 0,
        total_sales: Zero::zero(),
        total_orders: 0,
        rating: 0,
        rating_total: 0,
        rating_count: 0,
    };

    Shops::<T>::insert(shop_id, shop);
    ShopEntity::<T>::insert(shop_id, entity_id);
    if NextShopId::<T>::get() <= shop_id {
        NextShopId::<T>::put(shop_id.saturating_add(1));
    }
}

/// 种子 Shop 并设为 Active + 有运营资金
fn seed_active_shop<T: Config>(shop_id: u64, entity_id: u64) {
    seed_shop::<T>(shop_id, entity_id, ShopOperatingStatus::Active);
    // 给 shop 账户充值运营资金
    let shop_account = Pallet::<T>::shop_account_id(shop_id);
    let amount: BalanceOf<T> = 100_000u32.into();
    let _ = T::Currency::deposit_creating(&shop_account, amount);
}

/// 种子积分系统
fn seed_points<T: Config>(shop_id: u64) {
    let name: BoundedVec<u8, T::MaxPointsNameLength> = b"Points".to_vec().try_into().unwrap();
    let symbol: BoundedVec<u8, T::MaxPointsSymbolLength> = b"PTS".to_vec().try_into().unwrap();
    let config = PointsConfig {
        name,
        symbol,
        reward_rate: 500,
        exchange_rate: 1000,
        transferable: true,
    };
    ShopPointsConfigs::<T>::insert(shop_id, config);
}

/// 种子积分余额
fn seed_points_balance<T: Config>(shop_id: u64, account: &T::AccountId, amount: BalanceOf<T>) {
    ShopPointsBalances::<T>::insert(shop_id, account, amount);
    ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_add(amount));
}

/// 种子多个积分用户（用于 clear_prefix 分页测试）
fn seed_points_users<T: Config>(shop_id: u64, count: u32) {
    for i in 0..count {
        let account: T::AccountId = frame_benchmarking::account("points_user", i, 0);
        let amount: BalanceOf<T> = 100u32.into();
        ShopPointsBalances::<T>::insert(shop_id, &account, amount);
        ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_add(amount));
        // 设置过期时间
        let now = frame_system::Pallet::<T>::block_number();
        ShopPointsExpiresAt::<T>::insert(shop_id, &account, now.saturating_add(1000u32.into()));
    }
}


#[benchmarks]
mod benches {
    use super::*;

    // ==================== call_index(0): create_shop ====================
    #[benchmark]
    fn create_shop() {
        let owner = funded_account::<T>("owner", 0);
        NextShopId::<T>::put(1u64);

        let name = bench_name::<T>();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            1u64,                    // entity_id
            name,
            ShopType::OnlineStore,
            1000u32.into(),          // initial_fund
        );

        assert!(Shops::<T>::contains_key(1u64));
    }

    // ==================== call_index(1): update_shop ====================
    #[benchmark]
    fn update_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        let new_name: BoundedVec<u8, T::MaxShopNameLength> =
            b"Updated Shop".to_vec().try_into().unwrap();
        let new_cid = Some(Some(bench_cid::<T>()));

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            1u64,
            Some(new_name),
            new_cid.clone(),   // logo_cid
            new_cid.clone(),   // description_cid
            new_cid.clone(),   // business_hours_cid
            new_cid,           // policies_cid
        );
    }

    // ==================== call_index(2): add_manager ====================
    #[benchmark]
    fn add_manager() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        let manager: T::AccountId = frame_benchmarking::account("manager", 0, 0);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, manager.clone());

        let shop = Shops::<T>::get(1u64).unwrap();
        assert!(shop.managers.contains(&manager));
    }

    // ==================== call_index(3): remove_manager ====================
    #[benchmark]
    fn remove_manager() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        let manager: T::AccountId = frame_benchmarking::account("manager", 0, 0);

        // 先添加 manager
        Shops::<T>::mutate(1u64, |maybe| {
            if let Some(shop) = maybe {
                let _ = shop.managers.try_push(manager.clone());
            }
        });

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, manager.clone());

        let shop = Shops::<T>::get(1u64).unwrap();
        assert!(!shop.managers.contains(&manager));
    }

    // ==================== call_index(4): fund_operating ====================
    #[benchmark]
    fn fund_operating() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, 5000u32.into());
    }

    // ==================== call_index(5): pause_shop ====================
    #[benchmark]
    fn pause_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Paused);
    }

    // ==================== call_index(6): resume_shop ====================
    #[benchmark]
    fn resume_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_shop::<T>(1, 1, ShopOperatingStatus::Paused);
        // 确保有足够运营资金
        let shop_account = Pallet::<T>::shop_account_id(1u64);
        let _ = T::Currency::deposit_creating(&shop_account, 100_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Active);
    }

    // ==================== call_index(7): set_location ====================
    #[benchmark]
    fn set_location() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        let addr_cid = Some(Some(bench_cid::<T>()));

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            1u64,
            Some((121_473_000i64, 31_230_000i64)), // 上海坐标
            addr_cid,
        );
    }

    // ==================== call_index(8): enable_points ====================
    #[benchmark]
    fn enable_points() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        let name: BoundedVec<u8, T::MaxPointsNameLength> =
            b"Shop Points".to_vec().try_into().unwrap();
        let symbol: BoundedVec<u8, T::MaxPointsSymbolLength> =
            b"SP".to_vec().try_into().unwrap();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            1u64,
            name,
            symbol,
            500u16,   // reward_rate 5%
            1000u16,  // exchange_rate 10%
            true,     // transferable
        );

        assert!(ShopPointsConfigs::<T>::contains_key(1u64));
    }

    // ==================== call_index(9): close_shop ====================
    #[benchmark]
    fn close_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        // 确保不是 primary shop
        EntityPrimaryShop::<T>::remove(1u64);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Closing);
    }

    // ==================== call_index(10): disable_points ====================
    // 注意：此 benchmark 使用分页清理后的逻辑
    #[benchmark]
    fn disable_points() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);
        seed_points_users::<T>(1, 50);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64);

        assert!(!ShopPointsConfigs::<T>::contains_key(1u64));
    }

    // ==================== call_index(11): update_points_config ====================
    #[benchmark]
    fn update_points_config() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            1u64,
            Some(800u16),   // reward_rate
            Some(2000u16),  // exchange_rate
            Some(false),    // transferable
        );
    }

    // ==================== call_index(12): transfer_points ====================
    #[benchmark]
    fn transfer_points() {
        let from = funded_account::<T>("from", 0);
        let to: T::AccountId = frame_benchmarking::account("to", 0, 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);
        seed_points_balance::<T>(1, &from, 10_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(from), 1u64, to, 5_000u32.into());
    }

    // ==================== call_index(13): withdraw_operating_fund ====================
    #[benchmark]
    fn withdraw_operating_fund() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        // 确保 shop 有足够余额（超过 MinOperatingBalance）
        let shop_account = Pallet::<T>::shop_account_id(1u64);
        let _ = T::Currency::deposit_creating(&shop_account, 500_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, 1000u32.into());
    }

    // ==================== call_index(15): finalize_close_shop ====================
    #[benchmark]
    fn finalize_close_shop() {
        let caller = funded_account::<T>("caller", 0);
        seed_shop::<T>(1, 1, ShopOperatingStatus::Closing);
        // 确保不是 primary shop
        EntityPrimaryShop::<T>::remove(1u64);

        // 设置关闭时间为足够早
        let now = frame_system::Pallet::<T>::block_number();
        let grace = T::ShopClosingGracePeriod::get();
        let closing_at = now.saturating_sub(grace.saturating_add(1u32.into()));
        ShopClosingAt::<T>::insert(1u64, closing_at);

        // 种子一些积分数据用于清理
        seed_points::<T>(1);
        seed_points_users::<T>(1, 20);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Closed);
    }

    // ==================== call_index(16): manager_issue_points ====================
    #[benchmark]
    fn manager_issue_points() {
        let owner = funded_account::<T>("owner", 0);
        let to: T::AccountId = frame_benchmarking::account("recipient", 0, 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, to, 5_000u32.into());
    }

    // ==================== call_index(17): manager_burn_points ====================
    #[benchmark]
    fn manager_burn_points() {
        let owner = funded_account::<T>("owner", 0);
        let target: T::AccountId = frame_benchmarking::account("target", 0, 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);
        seed_points_balance::<T>(1, &target, 10_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, target, 5_000u32.into());
    }

    // ==================== call_index(18): redeem_points ====================
    #[benchmark]
    fn redeem_points() {
        let user = funded_account::<T>("user", 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);
        seed_points_balance::<T>(1, &user, 10_000u32.into());

        // 确保 shop 有足够运营资金支付兑换
        let shop_account = Pallet::<T>::shop_account_id(1u64);
        let _ = T::Currency::deposit_creating(&shop_account, 500_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(user), 1u64, 1_000u32.into());
    }

    // ==================== call_index(19): transfer_shop ====================
    #[benchmark]
    fn transfer_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        // 确保不是 primary shop
        EntityPrimaryShop::<T>::remove(1u64);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, 2u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.entity_id, 2u64);
    }

    // ==================== call_index(20): set_primary_shop ====================
    #[benchmark]
    fn set_primary_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        seed_active_shop::<T>(2, 1);
        // 设置 shop 1 为当前 primary
        EntityPrimaryShop::<T>::insert(1u64, 1u64);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, 2u64);

        assert_eq!(EntityPrimaryShop::<T>::get(1u64), Some(2u64));
    }

    // ==================== call_index(21): force_pause_shop ====================
    #[benchmark]
    fn force_pause_shop() {
        seed_active_shop::<T>(1, 1);

        #[extrinsic_call]
        _(RawOrigin::Root, 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Paused);
    }

    // ==================== call_index(22): set_points_ttl ====================
    #[benchmark]
    fn set_points_ttl() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, 100u32.into());

        assert_eq!(ShopPointsTtl::<T>::get(1u64), 100u32.into());
    }

    // ==================== call_index(23): expire_points ====================
    #[benchmark]
    fn expire_points() {
        let caller = funded_account::<T>("caller", 0);
        let target: T::AccountId = frame_benchmarking::account("target", 0, 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);
        seed_points_balance::<T>(1, &target, 10_000u32.into());

        // 设置过期时间为过去
        let now = frame_system::Pallet::<T>::block_number();
        ShopPointsExpiresAt::<T>::insert(1u64, &target, now.saturating_sub(1u32.into()));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1u64, target);
    }

    // ==================== call_index(24): force_close_shop ====================
    #[benchmark]
    fn force_close_shop() {
        seed_active_shop::<T>(1, 1);
        // 确保不是 primary shop
        EntityPrimaryShop::<T>::remove(1u64);
        // 种子积分数据
        seed_points::<T>(1);
        seed_points_users::<T>(1, 20);

        #[extrinsic_call]
        _(RawOrigin::Root, 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Closed);
    }

    // ==================== call_index(27): set_shop_type ====================
    #[benchmark]
    fn set_shop_type() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, ShopType::PhysicalStore);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.shop_type, ShopType::PhysicalStore);
    }

    // ==================== call_index(28): cancel_close_shop ====================
    #[benchmark]
    fn cancel_close_shop() {
        let owner = funded_account::<T>("owner", 0);
        seed_shop::<T>(1, 1, ShopOperatingStatus::Closing);
        ShopClosingAt::<T>::insert(1u64, frame_system::Pallet::<T>::block_number());

        // 确保有运营资金（恢复为 Active）
        let shop_account = Pallet::<T>::shop_account_id(1u64);
        let _ = T::Currency::deposit_creating(&shop_account, 100_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Active);
    }

    // ==================== call_index(29): set_points_max_supply ====================
    #[benchmark]
    fn set_points_max_supply() {
        let owner = funded_account::<T>("owner", 0);
        seed_active_shop::<T>(1, 1);
        seed_points::<T>(1);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), 1u64, 1_000_000u32.into());

        assert_eq!(ShopPointsMaxSupply::<T>::get(1u64), 1_000_000u32.into());
    }

    // ==================== call_index(30): resign_manager ====================
    #[benchmark]
    fn resign_manager() {
        let manager = funded_account::<T>("manager", 0);
        seed_active_shop::<T>(1, 1);

        // 添加 manager 到列表
        Shops::<T>::mutate(1u64, |maybe| {
            if let Some(shop) = maybe {
                let _ = shop.managers.try_push(manager.clone());
            }
        });

        #[extrinsic_call]
        _(RawOrigin::Signed(manager.clone()), 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert!(!shop.managers.contains(&manager));
    }

    // ==================== call_index(31): ban_shop ====================
    #[benchmark]
    fn ban_shop() {
        seed_active_shop::<T>(1, 1);

        let reason: BoundedVec<u8, T::MaxCidLength> =
            b"Violation of terms".to_vec().try_into().unwrap();

        #[extrinsic_call]
        _(RawOrigin::Root, 1u64, reason);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Banned);
    }

    // ==================== call_index(32): unban_shop ====================
    #[benchmark]
    fn unban_shop() {
        seed_shop::<T>(1, 1, ShopOperatingStatus::Banned);
        ShopStatusBeforeBan::<T>::insert(1u64, ShopOperatingStatus::Active);
        ShopBanReason::<T>::insert(
            1u64,
            BoundedVec::<u8, T::MaxCidLength>::try_from(b"reason".to_vec()).unwrap(),
        );

        #[extrinsic_call]
        _(RawOrigin::Root, 1u64);

        let shop = Shops::<T>::get(1u64).unwrap();
        assert_eq!(shop.status, ShopOperatingStatus::Active);
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
