//! # Storage Service Pallet Benchmarking
//!
//! 存储服务模块基准测试 — 覆盖所有 dispatchable extrinsics

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;
use frame_support::BoundedVec;
use sp_runtime::traits::Hash;

/// 辅助：注册一个 Active 运营者并返回其 AccountId
fn setup_operator<T: Config>(seed: u32) -> T::AccountId {
    let operator: T::AccountId = account("operator", seed, 0);
    let info = pallet::OperatorInfo::<T> {
        peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
        capacity_gib: 1000,
        endpoint_hash: T::Hashing::hash(&[seed as u8]),
        cert_fingerprint: None,
        status: 0,
        registered_at: frame_system::Pallet::<T>::block_number(),
        layer: OperatorLayer::Core,
        priority: 100,
    };
    pallet::Operators::<T>::insert(&operator, info);
    pallet::OperatorBond::<T>::insert(&operator, T::MinOperatorBond::get());
    // 加入活跃索引
    pallet::ActiveOperatorIndex::<T>::mutate(|idx| {
        let _ = idx.try_push(operator.clone());
    });
    operator
}

/// 辅助：创建一个已 Pin 的 CID 并返回 (cid_hash, owner)
fn setup_pinned_cid<T: Config>(seed: u8) -> (T::Hash, T::AccountId) {
    let owner: T::AccountId = account("owner", seed as u32, 0);
    let cid = alloc::vec![b'Q', b'm', seed, seed, seed];
    let cid_hash = T::Hashing::hash(&cid);
    pallet::PinMeta::<T>::insert(cid_hash, pallet::PinMetadata {
        replicas: 2,
        size: 1024,
        created_at: frame_system::Pallet::<T>::block_number(),
        last_activity: frame_system::Pallet::<T>::block_number(),
    });
    pallet::PinSubjectOf::<T>::insert(cid_hash, (owner.clone(), 1u64));
    pallet::CidTier::<T>::insert(cid_hash, PinTier::Standard);
    let subject_info = SubjectInfo {
        subject_type: SubjectType::General,
        subject_id: 1,
    };
    let subjects = BoundedVec::try_from(alloc::vec![subject_info]).unwrap();
    pallet::CidToSubject::<T>::insert(cid_hash, subjects);
    let cid_bounded: BoundedVec<u8, frame_support::traits::ConstU32<128>> =
        BoundedVec::try_from(cid).unwrap();
    pallet::CidRegistry::<T>::insert(cid_hash, cid_bounded);
    (cid_hash, owner)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ========== 用户接口 ==========

    #[benchmark]
    fn request_pin_for_subject() {
        let caller: T::AccountId = whitelisted_caller();
        let _op = setup_operator::<T>(1);
        let cid: Vec<u8> = b"QmBenchmarkCID12345678901234567890123456".to_vec();
        let size_bytes: u64 = 1024;

        // 初始化 tier 配置
        pallet::PinTierConfig::<T>::insert(PinTier::Standard, TierConfig::default());
        let layer_config = StorageLayerConfig { core_replicas: 1, community_replicas: 0, min_total_replicas: 1 };
        pallet::StorageLayerConfigs::<T>::insert((SubjectType::General, PinTier::Standard), layer_config);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1u64, cid, size_bytes, None);
    }

    #[benchmark]
    fn fund_user_account() {
        let caller: T::AccountId = whitelisted_caller();
        let target: T::AccountId = account("target", 1, 0);
        let amount: BalanceOf<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), target, amount);
    }

    #[benchmark]
    fn withdraw_user_funding() {
        let caller: T::AccountId = whitelisted_caller();
        let amount: BalanceOf<T> = 100u32.into();
        // 预充值
        let funding = Pallet::<T>::derive_user_funding_account(&caller);
        let _ = T::Currency::deposit_creating(&funding, 1000u32.into());
        pallet::UserFundingBalance::<T>::insert(&caller, BalanceOf::<T>::from(1000u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), amount);
    }

    #[benchmark]
    fn request_unpin() {
        let (cid_hash, owner) = setup_pinned_cid::<T>(1);
        let cid = pallet::CidRegistry::<T>::get(cid_hash)
            .map(|b| b.to_vec())
            .unwrap_or_else(|| alloc::vec![b'Q', b'm', 1, 1, 1]);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), cid);
    }

    #[benchmark]
    fn batch_unpin() {
        let owner: T::AccountId = whitelisted_caller();
        let mut cids: Vec<Vec<u8>> = Vec::new();
        for i in 0u8..5 {
            let cid = alloc::vec![b'Q', b'm', 50 + i, 50 + i, 50 + i];
            let cid_hash = T::Hashing::hash(&cid);
            pallet::PinMeta::<T>::insert(cid_hash, pallet::PinMetadata {
                replicas: 1, size: 512,
                created_at: frame_system::Pallet::<T>::block_number(),
                last_activity: frame_system::Pallet::<T>::block_number(),
            });
            pallet::PinSubjectOf::<T>::insert(cid_hash, (owner.clone(), 1u64));
            cids.push(cid);
        }

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), cids);
    }

    #[benchmark]
    fn renew_pin() {
        let (cid_hash, owner) = setup_pinned_cid::<T>(2);
        pallet::PinTierConfig::<T>::insert(PinTier::Standard, TierConfig::default());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), cid_hash, 1u32);
    }

    #[benchmark]
    fn upgrade_pin_tier() {
        let (cid_hash, owner) = setup_pinned_cid::<T>(3);
        pallet::PinTierConfig::<T>::insert(PinTier::Standard, TierConfig::default());
        pallet::PinTierConfig::<T>::insert(PinTier::Critical, TierConfig::critical_default());
        let _op = setup_operator::<T>(10);

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), cid_hash, PinTier::Critical);
    }

    #[benchmark]
    fn downgrade_pin_tier() {
        let (cid_hash, owner) = setup_pinned_cid::<T>(4);
        let cid = pallet::CidRegistry::<T>::get(cid_hash)
            .map(|b| b.to_vec())
            .unwrap_or_else(|| alloc::vec![b'Q', b'm', 4, 4, 4]);
        pallet::PinTierConfig::<T>::insert(PinTier::Standard, TierConfig::default());
        pallet::PinTierConfig::<T>::insert(PinTier::Temporary, TierConfig::temporary_default());

        #[extrinsic_call]
        _(RawOrigin::Signed(owner), cid, PinTier::Temporary);
    }

    #[benchmark]
    fn fund_ipfs_pool() {
        let caller: T::AccountId = whitelisted_caller();
        let amount: BalanceOf<T> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), amount);
    }

    // ========== 运营者接口 ==========

    #[benchmark]
    fn join_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let peer_id: BoundedVec<u8, T::MaxPeerIdLen> =
            BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap();
        let capacity: u32 = 1000;
        let endpoint_hash = T::Hashing::hash(b"https://ipfs.example.com");
        let bond: BalanceOf<T> = T::MinOperatorBond::get();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), peer_id, capacity, endpoint_hash, None, bond);
    }

    #[benchmark]
    fn update_operator() {
        let caller: T::AccountId = whitelisted_caller();
        // 预注册
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), None, None, None, None);
    }

    #[benchmark]
    fn leave_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);
        pallet::OperatorBond::<T>::insert(&caller, BalanceOf::<T>::from(0u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn pause_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn resume_operator() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 1, // Suspended
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn report_probe() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), true);
    }

    #[benchmark]
    fn operator_claim_rewards() {
        let caller: T::AccountId = whitelisted_caller();
        pallet::OperatorRewards::<T>::insert(&caller, BalanceOf::<T>::from(0u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn top_up_bond() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);
        pallet::OperatorBond::<T>::insert(&caller, BalanceOf::<T>::from(100u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), BalanceOf::<T>::from(50u32));
    }

    #[benchmark]
    fn reduce_bond() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);
        pallet::OperatorBond::<T>::insert(&caller, BalanceOf::<T>::from(10000u32));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), BalanceOf::<T>::from(50u32));
    }

    #[benchmark]
    fn dispute_slash() {
        let caller: T::AccountId = whitelisted_caller();
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&caller, info);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), BalanceOf::<T>::from(100u32), b"unfair slash".to_vec());
    }

    #[benchmark]
    fn mark_pinned() {
        let cid_hash = T::Hashing::hash(b"QmCID");

        #[extrinsic_call]
        _(RawOrigin::Root, cid_hash, 1u32);
    }

    #[benchmark]
    fn mark_pin_failed() {
        let cid_hash = T::Hashing::hash(b"QmCID");
        let error_code: u16 = 1;

        #[extrinsic_call]
        _(RawOrigin::Root, cid_hash, error_code);
    }

    // ========== 治理接口 ==========

    #[benchmark]
    fn set_operator_status() {
        let operator: T::AccountId = account("operator", 1, 0);
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&operator, info);

        #[extrinsic_call]
        _(RawOrigin::Root, operator, 1u8);
    }

    #[benchmark]
    fn slash_operator() {
        let operator: T::AccountId = account("operator", 1, 0);
        let info = pallet::OperatorInfo::<T> {
            peer_id: BoundedVec::try_from(alloc::vec![1u8; 16]).unwrap(),
            capacity_gib: 100, endpoint_hash: T::Hashing::hash(&[0u8]),
            cert_fingerprint: None, status: 0,
            registered_at: frame_system::Pallet::<T>::block_number(),
            layer: OperatorLayer::Core, priority: 100,
        };
        pallet::Operators::<T>::insert(&operator, info);
        pallet::OperatorBond::<T>::insert(&operator, BalanceOf::<T>::from(10000u32));

        #[extrinsic_call]
        _(RawOrigin::Root, operator, BalanceOf::<T>::from(100u32));
    }

    #[benchmark]
    fn set_billing_params() {
        #[extrinsic_call]
        _(RawOrigin::Root, None, None, None, None, None, None);
    }

    #[benchmark]
    fn distribute_to_operators() {
        let max_amount: BalanceOf<T> = 10000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, max_amount);
    }

    #[benchmark]
    fn update_tier_config() {
        let config = TierConfig::default();

        #[extrinsic_call]
        _(RawOrigin::Root, PinTier::Standard, config);
    }

    #[benchmark]
    fn emergency_pause_billing() {
        #[extrinsic_call]
        _(RawOrigin::Root);
    }

    #[benchmark]
    fn resume_billing() {
        pallet::BillingPaused::<T>::put(true);

        #[extrinsic_call]
        _(RawOrigin::Root);
    }

    #[benchmark]
    fn set_storage_layer_config() {
        let config = StorageLayerConfig::default();

        #[extrinsic_call]
        _(RawOrigin::Root, SubjectType::General, PinTier::Standard, config);
    }

    #[benchmark]
    fn set_operator_layer() {
        let operator = setup_operator::<T>(1);

        #[extrinsic_call]
        _(RawOrigin::Root, operator, OperatorLayer::Community, 150u8);
    }

    #[benchmark]
    fn register_domain() {
        let domain = b"benchmark-domain".to_vec();

        #[extrinsic_call]
        _(RawOrigin::Root, domain, 0u8, PinTier::Standard, true);
    }

    #[benchmark]
    fn update_domain_config() {
        let domain = b"bench-update".to_vec();
        let domain_bounded: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            BoundedVec::try_from(domain.clone()).unwrap();
        pallet::RegisteredDomains::<T>::insert(&domain_bounded, types::DomainConfig::default());

        #[extrinsic_call]
        _(RawOrigin::Root, domain, Some(false), Some(PinTier::Standard), Some(1u8));
    }

    #[benchmark]
    fn set_domain_priority() {
        let domain = b"bench-prio".to_vec();
        let domain_bounded: BoundedVec<u8, frame_support::traits::ConstU32<32>> =
            BoundedVec::try_from(domain.clone()).unwrap();
        pallet::RegisteredDomains::<T>::insert(&domain_bounded, types::DomainConfig::default());

        #[extrinsic_call]
        _(RawOrigin::Root, domain, 5u8);
    }

    #[benchmark]
    fn governance_force_unpin() {
        let cid = b"QmForceUnpinBench".to_vec();
        let cid_hash = T::Hashing::hash(&cid);
        pallet::PinMeta::<T>::insert(cid_hash, pallet::PinMetadata {
            replicas: 1, size: 512,
            created_at: frame_system::Pallet::<T>::block_number(),
            last_activity: frame_system::Pallet::<T>::block_number(),
        });

        #[extrinsic_call]
        _(RawOrigin::Root, cid, b"violation".to_vec());
    }

    #[benchmark]
    fn migrate_operator_pins() {
        let from = setup_operator::<T>(1);
        let to = setup_operator::<T>(2);

        #[extrinsic_call]
        _(RawOrigin::Root, from, to, 10u32);
    }

    // ========== 公共清理接口 ==========

    #[benchmark]
    fn cleanup_expired_cids(n: Linear<1, 20>) {
        // 插入 n 个过期 CID
        for i in 0..n {
            let cid_hash = T::Hashing::hash(&[i as u8, 0xEE]);
            pallet::PinBilling::<T>::insert(cid_hash, (
                frame_system::Pallet::<T>::block_number(),
                100u128,
                2u8,
            ));
            pallet::ExpiredCidQueue::<T>::mutate(|q| { let _ = q.try_push(cid_hash); });
        }
        pallet::ExpiredCidPending::<T>::put(true);

        #[extrinsic_call]
        _(RawOrigin::Signed(whitelisted_caller()), n);
    }

    #[benchmark]
    fn cleanup_expired_locks() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 10u32);
    }

    // ========== OCW Unsigned 接口 ==========

    #[benchmark]
    fn ocw_mark_pinned() {
        let operator = setup_operator::<T>(1);
        let cid_hash = T::Hashing::hash(&[0xAA, 0xBB]);
        pallet::PendingPins::<T>::insert(cid_hash, (operator.clone(), 1u32, 0u64, 1024u64, BalanceOf::<T>::from(0u32)));
        let ops: BoundedVec<T::AccountId, frame_support::traits::ConstU32<16>> =
            BoundedVec::try_from(alloc::vec![operator.clone()]).unwrap();
        pallet::PinAssignments::<T>::insert(cid_hash, ops);
        pallet::PinStateOf::<T>::insert(cid_hash, 1u8);

        #[extrinsic_call]
        _(RawOrigin::None, operator, cid_hash, 1u32);
    }

    #[benchmark]
    fn ocw_mark_pin_failed() {
        let operator = setup_operator::<T>(1);
        let cid_hash = T::Hashing::hash(&[0xCC, 0xDD]);
        pallet::PendingPins::<T>::insert(cid_hash, (operator.clone(), 1u32, 0u64, 1024u64, BalanceOf::<T>::from(0u32)));
        let ops: BoundedVec<T::AccountId, frame_support::traits::ConstU32<16>> =
            BoundedVec::try_from(alloc::vec![operator.clone()]).unwrap();
        pallet::PinAssignments::<T>::insert(cid_hash, ops);

        #[extrinsic_call]
        _(RawOrigin::None, operator, cid_hash, 500u16);
    }

    #[benchmark]
    fn ocw_submit_assignments() {
        let operator = setup_operator::<T>(1);
        let cid_hash = T::Hashing::hash(&[0xEE, 0xFF]);
        pallet::PendingPins::<T>::insert(cid_hash, (operator.clone(), 1u32, 0u64, 1024u64, BalanceOf::<T>::from(0u32)));
        let core_ops: Vec<T::AccountId> = alloc::vec![operator.clone()];
        let community_ops: Vec<T::AccountId> = alloc::vec![];

        #[extrinsic_call]
        _(RawOrigin::None, cid_hash, core_ops, community_ops);
    }

    #[benchmark]
    fn ocw_report_health() {
        let operator = setup_operator::<T>(1);
        let cid_hash = T::Hashing::hash(&[0x11, 0x22]);
        let ops: BoundedVec<T::AccountId, frame_support::traits::ConstU32<16>> =
            BoundedVec::try_from(alloc::vec![operator.clone()]).unwrap();
        pallet::PinAssignments::<T>::insert(cid_hash, ops);

        #[extrinsic_call]
        _(RawOrigin::None, cid_hash, operator, true);
    }

    // ========== 已废弃（保留 benchmark 以生成权重） ==========

    #[benchmark]
    fn fund_subject_account() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), 1u64, BalanceOf::<T>::from(100u32));
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
