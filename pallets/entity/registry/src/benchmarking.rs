//! Benchmarking for pallet-entity-registry.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! All 23 active extrinsics are benchmarked.
//!
//! Signed extrinsics that depend on mock providers (PricingProvider, ShopProvider)
//! use seed helpers to pre-populate storage directly, bypassing `create_entity`.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::{AdminPermission, EntityStatus, EntityType, GovernanceMode};
use frame_support::traits::{Currency, Get};
use sp_runtime::traits::Bounded;
use sp_runtime::Saturating;

/// 创建一个有足够余额的账户
fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let account: T::AccountId = frame_benchmarking::account(name, index, 0);
    let amount = BalanceOf::<T>::max_value() / 2u32.into();
    let _ = T::Currency::deposit_creating(&account, amount);
    account
}

/// 种子实体（直接写入存储，绕过 create_entity 的外部依赖）
fn seed_entity<T: Config>(entity_id: u64) -> T::AccountId {
    let owner = funded_account::<T>("owner", 0);
    let name_bytes = alloc::format!("bench-entity-{}", entity_id).into_bytes();
    let name: BoundedVec<u8, T::MaxEntityNameLength> =
        name_bytes.try_into().expect("name fits");

    // 写入名称索引
    if let Ok(normalized) = Pallet::<T>::normalize_entity_name(&name) {
        EntityNameIndex::<T>::insert(&normalized, entity_id);
    }

    let entity = Entity {
        id: entity_id,
        owner: owner.clone(),
        name,
        logo_cid: None,
        description_cid: None,
        status: EntityStatus::Active,
        created_at: frame_system::Pallet::<T>::block_number(),
        entity_type: EntityType::Merchant,
        admins: BoundedVec::default(),
        governance_mode: GovernanceMode::None,
        verified: false,
        metadata_uri: None,
        contact_cid: None,
        primary_shop_id: 0,
    };
    Entities::<T>::insert(entity_id, entity);
    NextEntityId::<T>::put(entity_id.saturating_add(1));

    // 写入 UserEntity 索引
    let _ = UserEntity::<T>::try_mutate(&owner, |entities| {
        entities.try_push(entity_id)
    });

    // 更新统计
    EntityStats::<T>::mutate(|stats| {
        stats.total_entities = stats.total_entities.saturating_add(1);
        stats.active_entities = stats.active_entities.saturating_add(1);
    });

    // 向金库充值
    let treasury = Pallet::<T>::entity_treasury_account(entity_id);
    let fund = T::FundWarningThreshold::get().saturating_mul(10u32.into());
    let _ = T::Currency::deposit_creating(&treasury, fund);

    owner
}

/// 种子实体并设置指定状态
fn seed_entity_with_status<T: Config>(entity_id: u64, status: EntityStatus) -> T::AccountId {
    let owner = seed_entity::<T>(entity_id);
    Entities::<T>::mutate(entity_id, |e| {
        if let Some(e) = e {
            e.status = status;
        }
    });
    if status != EntityStatus::Active {
        EntityStats::<T>::mutate(|stats| {
            stats.active_entities = stats.active_entities.saturating_sub(1);
        });
    }
    owner
}

/// 种子实体并填充最大管理员
fn seed_entity_with_max_admins<T: Config>(entity_id: u64) -> T::AccountId {
    let owner = seed_entity::<T>(entity_id);
    Entities::<T>::mutate(entity_id, |e| {
        if let Some(e) = e {
            let max = T::MaxAdmins::get();
            for i in 0..max {
                let admin: T::AccountId = frame_benchmarking::account("admin", i, 0);
                let _ = e.admins.try_push((admin, AdminPermission::ALL_DEFINED));
            }
        }
    });
    owner
}

/// 种子实体并添加一个管理员
fn seed_entity_with_one_admin<T: Config>(entity_id: u64) -> (T::AccountId, T::AccountId) {
    let owner = seed_entity::<T>(entity_id);
    let admin: T::AccountId = funded_account::<T>("admin", 0);
    Entities::<T>::mutate(entity_id, |e| {
        if let Some(e) = e {
            let _ = e.admins.try_push((admin.clone(), AdminPermission::ALL_DEFINED));
        }
    });
    (owner, admin)
}

/// 种子实体并注册 shop
fn seed_entity_with_shops<T: Config>(entity_id: u64, shop_count: u32) -> T::AccountId {
    let owner = seed_entity::<T>(entity_id);
    EntityShops::<T>::mutate(entity_id, |shops| {
        for i in 1..=shop_count {
            let _ = shops.try_push(i as u64);
        }
    });
    if shop_count > 0 {
        Entities::<T>::mutate(entity_id, |e| {
            if let Some(e) = e {
                e.primary_shop_id = 1;
            }
        });
    }
    owner
}

#[benchmarks]
mod benches {
    use super::*;
    use frame_support::traits::Get;

    // ==================== call_index(0): create_entity ====================
    #[benchmark]
    fn create_entity() {
        let caller = funded_account::<T>("caller", 0);
        let name = b"Benchmark Entity".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), name, None, None, None);

        let entity_id = NextEntityId::<T>::get() - 1;
        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.owner, caller);
        assert_eq!(e.status, EntityStatus::Active);
    }

    // ==================== call_index(1): update_entity ====================
    #[benchmark]
    fn update_entity() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);
        let new_name = b"Updated Name".to_vec();
        let logo = b"QmBenchLogo123456789012345678901234567890123456".to_vec();
        let desc = b"QmBenchDesc123456789012345678901234567890123456".to_vec();
        let meta = b"ipfs://QmBenchMeta12345678901234567890123456789012".to_vec();
        let contact = b"QmBenchContact234567890123456789012345678901234".to_vec();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            entity_id,
            Some(new_name),
            Some(logo),
            Some(desc),
            Some(meta),
            Some(contact),
        );

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.name.to_vec(), b"Updated Name".to_vec());
        assert!(e.logo_cid.is_some());
        assert!(e.description_cid.is_some());
        assert!(e.metadata_uri.is_some());
        assert!(e.contact_cid.is_some());
    }

    // ==================== call_index(2): request_close_entity ====================
    #[benchmark]
    fn request_close_entity() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::PendingClose);
        assert!(EntityCloseRequests::<T>::contains_key(entity_id));
    }

    // ==================== call_index(3): top_up_fund ====================
    #[benchmark]
    fn top_up_fund() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);
        let amount: BalanceOf<T> = T::MinOperatingBalance::get().saturating_mul(10u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id, amount);

        // 验证余额增加
        let balance = Pallet::<T>::get_entity_fund_balance(entity_id);
        assert!(balance > T::FundWarningThreshold::get());
    }

    // ==================== call_index(6): suspend_entity ====================
    #[benchmark]
    fn suspend_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);
        let reason = b"Suspicious activity detected during routine audit".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, Some(reason));

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Suspended);
        assert!(GovernanceSuspended::<T>::get(entity_id));
        assert!(SuspensionReasons::<T>::contains_key(entity_id));
    }

    // ==================== call_index(7): resume_entity ====================
    #[benchmark]
    fn resume_entity() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::Suspended);
        GovernanceSuspended::<T>::insert(entity_id, true);
        SuspensionReasons::<T>::insert(
            entity_id,
            BoundedVec::<u8, sp_runtime::traits::ConstU32<256>>::try_from(b"test".to_vec()).unwrap(),
        );

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
        assert!(!GovernanceSuspended::<T>::get(entity_id));
    }

    // ==================== call_index(8): ban_entity ====================
    #[benchmark]
    fn ban_entity() {
        let entity_id: u64 = 9999;
        let max_shops = T::MaxShopsPerEntity::get();
        seed_entity_with_shops::<T>(entity_id, max_shops);
        let reason = b"Fraud detected".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, true, Some(reason));

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Banned);
    }

    // ==================== call_index(9): add_admin ====================
    #[benchmark]
    fn add_admin() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);
        let new_admin: T::AccountId = frame_benchmarking::account("new_admin", 0, 0);

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            entity_id,
            new_admin.clone(),
            AdminPermission::ALL_DEFINED,
        );

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(e.admins.iter().any(|(a, _)| a == &new_admin));
    }

    // ==================== call_index(10): remove_admin ====================
    #[benchmark]
    fn remove_admin() {
        let entity_id: u64 = 9999;
        // 填充最大管理员数，worst case
        let owner = seed_entity_with_max_admins::<T>(entity_id);
        let admin_to_remove: T::AccountId = frame_benchmarking::account("admin", 0, 0);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id, admin_to_remove.clone());

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(!e.admins.iter().any(|(a, _)| a == &admin_to_remove));
    }

    // ==================== call_index(11): transfer_ownership ====================
    #[benchmark]
    fn transfer_ownership() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);
        let new_owner = funded_account::<T>("new_owner", 0);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id, new_owner.clone());

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.owner, new_owner);
    }

    // ==================== call_index(12): upgrade_entity_type ====================
    #[benchmark]
    fn upgrade_entity_type() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(
            RawOrigin::Root,
            entity_id,
            EntityType::DAO,
            GovernanceMode::FullDAO,
        );

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.entity_type, EntityType::DAO);
        assert_eq!(e.governance_mode, GovernanceMode::FullDAO);
    }

    // ==================== call_index(14): verify_entity ====================
    #[benchmark]
    fn verify_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(e.verified);
    }

    // ==================== call_index(15): reopen_entity ====================
    #[benchmark]
    fn reopen_entity() {
        let entity_id: u64 = 9999;
        let owner = seed_entity_with_status::<T>(entity_id, EntityStatus::Closed);
        // Closed 时 UserEntity 已清理，名称索引已清理
        UserEntity::<T>::mutate(&owner, |entities| {
            entities.retain(|&id| id != entity_id);
        });
        // 清理名称索引（模拟 close 后的状态）
        let entity = Entities::<T>::get(entity_id).unwrap();
        if let Ok(normalized) = Pallet::<T>::normalize_entity_name(&entity.name) {
            EntityNameIndex::<T>::remove(&normalized);
        }

        #[extrinsic_call]
        _(RawOrigin::Signed(owner.clone()), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
        assert!(UserEntity::<T>::get(&owner).contains(&entity_id));
    }

    // ==================== call_index(16): bind_entity_referrer ====================
    #[benchmark]
    fn bind_entity_referrer() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);
        // 创建推荐人（需拥有非终态 Entity，且不能是同一个账户）
        let referrer_entity_id: u64 = 8888;
        let referrer = funded_account::<T>("referrer", 1);
        // 手动为 referrer 创建一个 entity
        let ref_name: BoundedVec<u8, T::MaxEntityNameLength> =
            b"referrer-entity".to_vec().try_into().expect("name fits");
        let ref_entity = Entity {
            id: referrer_entity_id,
            owner: referrer.clone(),
            name: ref_name,
            logo_cid: None,
            description_cid: None,
            status: EntityStatus::Active,
            created_at: frame_system::Pallet::<T>::block_number(),
            entity_type: EntityType::Merchant,
            admins: BoundedVec::default(),
            governance_mode: GovernanceMode::None,
            verified: false,
            metadata_uri: None,
            contact_cid: None,
            primary_shop_id: 0,
        };
        Entities::<T>::insert(referrer_entity_id, ref_entity);
        let _ = UserEntity::<T>::try_mutate(&referrer, |entities| {
            entities.try_push(referrer_entity_id)
        });

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id, referrer.clone());

        assert_eq!(EntityReferrer::<T>::get(entity_id), Some(referrer.clone()));
        assert!(ReferrerEntities::<T>::get(&referrer).contains(&entity_id));
    }

    // ==================== call_index(17): update_admin_permissions ====================
    #[benchmark]
    fn update_admin_permissions() {
        let entity_id: u64 = 9999;
        let (owner, admin) = seed_entity_with_one_admin::<T>(entity_id);

        #[extrinsic_call]
        _(
            RawOrigin::Signed(owner),
            entity_id,
            admin.clone(),
            AdminPermission::SHOP_MANAGE,
        );

        let e = Entities::<T>::get(entity_id).unwrap();
        let (_, perm) = e.admins.iter().find(|(a, _)| a == &admin).unwrap();
        assert_eq!(*perm, AdminPermission::SHOP_MANAGE);
    }

    // ==================== call_index(18): unban_entity ====================
    #[benchmark]
    fn unban_entity() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::Banned);
        // 清理名称索引（ban 时会清理）
        let entity = Entities::<T>::get(entity_id).unwrap();
        if let Ok(normalized) = Pallet::<T>::normalize_entity_name(&entity.name) {
            EntityNameIndex::<T>::remove(&normalized);
        }

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
    }

    // ==================== call_index(19): unverify_entity ====================
    #[benchmark]
    fn unverify_entity() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);
        Entities::<T>::mutate(entity_id, |e| {
            if let Some(e) = e {
                e.verified = true;
            }
        });

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(!e.verified);
    }

    // ==================== call_index(20): cancel_close_request ====================
    #[benchmark]
    fn cancel_close_request() {
        let entity_id: u64 = 9999;
        let owner = seed_entity_with_status::<T>(entity_id, EntityStatus::PendingClose);
        EntityCloseRequests::<T>::insert(entity_id, frame_system::Pallet::<T>::block_number());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
        assert!(!EntityCloseRequests::<T>::contains_key(entity_id));
    }

    // ==================== call_index(21): resign_admin ====================
    #[benchmark]
    fn resign_admin() {
        let entity_id: u64 = 9999;
        let (_owner, admin) = seed_entity_with_one_admin::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(admin.clone()), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert!(!e.admins.iter().any(|(a, _)| a == &admin));
    }

    // call_index(22) 已移除: set_primary_shop（统一由 shop pallet 管理）

    // ==================== call_index(23): self_pause_entity ====================
    #[benchmark]
    fn self_pause_entity() {
        let entity_id: u64 = 9999;
        let owner = seed_entity::<T>(entity_id);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Suspended);
        assert!(OwnerPaused::<T>::get(entity_id));
    }

    // ==================== call_index(24): self_resume_entity ====================
    #[benchmark]
    fn self_resume_entity() {
        let entity_id: u64 = 9999;
        let owner = seed_entity_with_status::<T>(entity_id, EntityStatus::Suspended);
        OwnerPaused::<T>::insert(entity_id, true);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
        assert!(!OwnerPaused::<T>::get(entity_id));
    }

    // ==================== call_index(25): force_transfer_ownership ====================
    #[benchmark]
    fn force_transfer_ownership() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);
        let new_owner: T::AccountId = frame_benchmarking::account("new_owner", 0, 0);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, new_owner.clone());

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.owner, new_owner);
    }

    // ==================== call_index(26): reject_close_request ====================
    #[benchmark]
    fn reject_close_request() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::PendingClose);
        EntityCloseRequests::<T>::insert(entity_id, frame_system::Pallet::<T>::block_number());

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Active);
    }

    // ==================== call_index(27): execute_close_timeout ====================
    #[benchmark]
    fn execute_close_timeout() {
        let entity_id: u64 = 9999;
        seed_entity_with_status::<T>(entity_id, EntityStatus::PendingClose);
        // 设置关闭申请时间为区块 1
        EntityCloseRequests::<T>::insert(entity_id, frame_system::Pallet::<T>::block_number());
        // 推进区块号超过 timeout（使用足够大的值确保超时）
        frame_system::Pallet::<T>::set_block_number(1_000_000u32.into());

        let caller = funded_account::<T>("caller", 99);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), entity_id);

        let e = Entities::<T>::get(entity_id).unwrap();
        assert_eq!(e.status, EntityStatus::Closed);
    }

    // ==================== call_index(28): force_rebind_referrer ====================
    #[benchmark]
    fn force_rebind_referrer() {
        let entity_id: u64 = 9999;
        seed_entity::<T>(entity_id);
        // 设置初始推荐人
        let old_referrer = funded_account::<T>("old_referrer", 1);
        EntityReferrer::<T>::insert(entity_id, &old_referrer);
        let _ = ReferrerEntities::<T>::try_mutate(&old_referrer, |entities| {
            entities.try_push(entity_id)
        });
        // 创建新推荐人
        let new_referrer = funded_account::<T>("new_referrer", 2);

        #[extrinsic_call]
        _(RawOrigin::Root, entity_id, new_referrer.clone());

        assert_eq!(EntityReferrer::<T>::get(entity_id), Some(new_referrer.clone()));
        assert!(ReferrerEntities::<T>::get(&new_referrer).contains(&entity_id));
        assert!(!ReferrerEntities::<T>::get(&old_referrer).contains(&entity_id));
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
