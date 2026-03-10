//! Benchmarking for pallet-entity-kyc
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::vec::Vec;
use frame_benchmarking::v2::*;
use frame_support::{pallet_prelude::ConstU32, traits::Get, BoundedVec};
use frame_system::RawOrigin;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::Saturating;

const ENTITY_1: u64 = 1;

/// 在 test 环境下注册 Entity 并设置 owner
fn setup_entity_for<T: Config>(_eid: u64, _owner: &T::AccountId) {
    #[cfg(test)]
    {
        use codec::Encode;
        let bytes = _owner.encode();
        let id = if bytes.len() >= 8 {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            0u64
        };
        crate::mock::MockEntityProvider::set_entity_owner(_eid, id);
    }
}

fn do_register_provider<T: Config>(provider: &T::AccountId) {
    if Providers::<T>::contains_key(provider) {
        return;
    }
    let name: BoundedVec<u8, T::MaxProviderNameLength> =
        b"Bench Provider".to_vec().try_into().unwrap();
    Providers::<T>::insert(
        provider,
        KycProvider { name, max_level: KycLevel::Institutional, suspended: false },
    );
    ProviderCount::<T>::mutate(|c| *c = c.saturating_add(1));
}

fn do_authorize<T: Config>(provider: &T::AccountId, entity_id: u64) {
    do_register_provider::<T>(provider);
    EntityAuthorizedProviders::<T>::insert(entity_id, provider, ());
    ProviderAuthorizedEntities::<T>::mutate(provider, |v| {
        let _ = v.try_push(entity_id);
    });
}

fn insert_approved<T: Config>(
    entity_id: u64,
    user: &T::AccountId,
    provider: &T::AccountId,
    level: KycLevel,
) {
    let now = frame_system::Pallet::<T>::block_number();
    let validity = Pallet::<T>::get_validity_period(level);
    let record = KycRecord {
        level,
        status: KycStatus::Approved,
        provider: Some(provider.clone()),
        data_cid: Some(b"QmBenchApproved".to_vec().try_into().unwrap()),
        submitted_at: Some(now),
        verified_at: Some(now),
        expires_at: Some(now.saturating_add(validity)),
        rejection_reason: None,
        rejection_details_cid: None,
        country_code: Some(*b"US"),
        risk_score: 10,
    };
    KycRecords::<T>::insert(entity_id, user, record);
    ApprovedKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));
}

fn insert_pending<T: Config>(entity_id: u64, user: &T::AccountId) {
    let now = frame_system::Pallet::<T>::block_number();
    let record = KycRecord {
        level: KycLevel::Basic,
        status: KycStatus::Pending,
        provider: None,
        data_cid: Some(b"QmBenchPending".to_vec().try_into().unwrap()),
        submitted_at: Some(now),
        verified_at: None,
        expires_at: None,
        rejection_reason: None,
        rejection_details_cid: None,
        country_code: Some(*b"US"),
        risk_score: 0,
    };
    KycRecords::<T>::insert(entity_id, user, record);
    PendingKycCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));
}

fn insert_expired<T: Config>(entity_id: u64, user: &T::AccountId, provider: &T::AccountId) {
    let record = KycRecord {
        level: KycLevel::Basic,
        status: KycStatus::Expired,
        provider: Some(provider.clone()),
        data_cid: Some(b"QmBenchExpired".to_vec().try_into().unwrap()),
        submitted_at: Some(BlockNumberFor::<T>::from(1u32)),
        verified_at: Some(BlockNumberFor::<T>::from(2u32)),
        expires_at: Some(BlockNumberFor::<T>::from(3u32)),
        rejection_reason: None,
        rejection_details_cid: None,
        country_code: Some(*b"US"),
        risk_score: 5,
    };
    KycRecords::<T>::insert(entity_id, user, record);
}

fn insert_rejected<T: Config>(entity_id: u64, user: &T::AccountId) {
    let record = KycRecord {
        level: KycLevel::Basic,
        status: KycStatus::Rejected,
        provider: None,
        data_cid: Some(b"QmBenchRejected".to_vec().try_into().unwrap()),
        submitted_at: Some(BlockNumberFor::<T>::from(1u32)),
        verified_at: None,
        expires_at: None,
        rejection_reason: Some(RejectionReason::ExpiredDocument),
        rejection_details_cid: None,
        country_code: Some(*b"US"),
        risk_score: 0,
    };
    KycRecords::<T>::insert(entity_id, user, record);
}

/// 生成一个与 caller 不同的 AccountId 用于 approve/reject/renew 等需要 who != account 的场景
fn make_user<T: Config>(seed: u32) -> T::AccountId {
    account("user", seed, 0)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // call_index(0): submit_kyc(origin, entity_id, level, data_cid, country_code)
    #[benchmark]
    fn submit_kyc() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, KycLevel::Basic, b"QmBenchData123".to_vec(), *b"US");
    }

    // call_index(1): approve_kyc(origin, entity_id, account, risk_score)
    #[benchmark]
    fn approve_kyc() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let user = make_user::<T>(99);
        insert_pending::<T>(ENTITY_1, &user);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user, 10u8);
    }

    // call_index(2): reject_kyc(origin, entity_id, account, reason, details_cid)
    #[benchmark]
    fn reject_kyc() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let user = make_user::<T>(99);
        insert_pending::<T>(ENTITY_1, &user);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user, RejectionReason::ExpiredDocument, None);
    }

    // call_index(3): revoke_kyc(origin, entity_id, account, reason) — AdminOrigin
    #[benchmark]
    fn revoke_kyc() {
        let provider: T::AccountId = whitelisted_caller();
        let user = make_user::<T>(99);
        setup_entity_for::<T>(ENTITY_1, &provider);
        insert_approved::<T>(ENTITY_1, &user, &provider, KycLevel::Basic);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, user, RejectionReason::SuspiciousActivity);
    }

    // call_index(4): register_provider(origin, provider_account, name, max_level) — AdminOrigin
    #[benchmark]
    fn register_provider() {
        let provider = make_user::<T>(88);
        #[extrinsic_call]
        _(RawOrigin::Root, provider, b"New Provider".to_vec(), KycLevel::Enhanced);
    }

    // call_index(5): remove_provider(origin, provider_account) — AdminOrigin
    // n = number of authorized entities to clean up
    #[benchmark]
    fn remove_provider(n: Linear<0, 50>) {
        let provider = make_user::<T>(88);
        do_register_provider::<T>(&provider);
        for i in 0..n {
            let eid = 100u64 + i as u64;
            EntityAuthorizedProviders::<T>::insert(eid, &provider, ());
            ProviderAuthorizedEntities::<T>::mutate(&provider, |v| {
                let _ = v.try_push(eid);
            });
        }
        #[extrinsic_call]
        _(RawOrigin::Root, provider);
    }

    // call_index(6): set_entity_requirement(origin, entity_id, min_level, mandatory, grace_period, allow_high_risk, max_risk_score)
    #[benchmark]
    fn set_entity_requirement() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, KycLevel::Basic, true, 100u32, false, 80u8);
    }

    // call_index(7): update_high_risk_countries(origin, countries) — AdminOrigin
    #[benchmark]
    fn update_high_risk_countries(n: Linear<1, 50>) {
        let mut countries = Vec::new();
        // 生成 n 个不同的国家代码
        for i in 0..n {
            let a = b'A' + (i % 26) as u8;
            let b = b'A' + (i / 26) as u8;
            countries.push([a, b]);
        }
        let bounded: BoundedVec<[u8; 2], ConstU32<50>> = countries.try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Root, bounded);
    }

    // call_index(8): expire_kyc(origin, entity_id, account)
    #[benchmark]
    fn expire_kyc() {
        let c: T::AccountId = whitelisted_caller();
        let user = make_user::<T>(99);
        setup_entity_for::<T>(ENTITY_1, &c);
        // 插入一个已过期的 Approved 记录
        let record = KycRecord {
            level: KycLevel::Basic,
            status: KycStatus::Approved,
            provider: Some(c.clone()),
            data_cid: Some(b"QmBench".to_vec().try_into().unwrap()),
            submitted_at: Some(BlockNumberFor::<T>::from(1u32)),
            verified_at: Some(BlockNumberFor::<T>::from(1u32)),
            expires_at: Some(BlockNumberFor::<T>::from(1u32)), // 已过期
            rejection_reason: None,
            rejection_details_cid: None,
            country_code: Some(*b"US"),
            risk_score: 0,
        };
        KycRecords::<T>::insert(ENTITY_1, &user, record);
        ApprovedKycCount::<T>::mutate(ENTITY_1, |c| *c = c.saturating_add(1));
        // 推进区块号使其过期
        frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(100u32));
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user);
    }

    // call_index(9): cancel_kyc(origin, entity_id)
    #[benchmark]
    fn cancel_kyc() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        insert_pending::<T>(ENTITY_1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1);
    }

    // call_index(10): force_set_entity_requirement(origin, entity_id, min_level, mandatory, grace_period, allow_high_risk, max_risk_score) — AdminOrigin
    #[benchmark]
    fn force_set_entity_requirement() {
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, KycLevel::Standard, true, 200u32, true, 90u8);
    }

    // call_index(11): update_risk_score(origin, entity_id, account, new_score)
    #[benchmark]
    fn update_risk_score() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let user = make_user::<T>(99);
        insert_approved::<T>(ENTITY_1, &user, &c, KycLevel::Basic);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user, 50u8);
    }

    // call_index(12): update_provider(origin, provider_account, name, max_level) — AdminOrigin
    #[benchmark]
    fn update_provider() {
        let provider = make_user::<T>(88);
        do_register_provider::<T>(&provider);
        #[extrinsic_call]
        _(RawOrigin::Root, provider, Some(b"Updated Name".to_vec()), Some(KycLevel::Institutional));
    }

    // call_index(13): suspend_provider(origin, provider_account) — AdminOrigin
    #[benchmark]
    fn suspend_provider() {
        let provider = make_user::<T>(88);
        do_register_provider::<T>(&provider);
        #[extrinsic_call]
        _(RawOrigin::Root, provider);
    }

    // call_index(14): resume_provider(origin, provider_account) — AdminOrigin
    #[benchmark]
    fn resume_provider() {
        let provider = make_user::<T>(88);
        do_register_provider::<T>(&provider);
        Providers::<T>::mutate(&provider, |p| {
            if let Some(ref mut prov) = p {
                prov.suspended = true;
            }
        });
        #[extrinsic_call]
        _(RawOrigin::Root, provider);
    }

    // call_index(15): force_approve_kyc(origin, entity_id, account, level, risk_score, country_code) — AdminOrigin
    #[benchmark]
    fn force_approve_kyc() {
        let user = make_user::<T>(99);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, user, KycLevel::Enhanced, 20u8, *b"US");
    }

    // call_index(16): renew_kyc(origin, entity_id, account)
    #[benchmark]
    fn renew_kyc() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let user = make_user::<T>(99);
        insert_expired::<T>(ENTITY_1, &user, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user);
    }

    // call_index(17): update_kyc_data(origin, entity_id, new_data_cid)
    #[benchmark]
    fn update_kyc_data() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        insert_pending::<T>(ENTITY_1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, b"QmUpdatedData456".to_vec());
    }

    // call_index(18): purge_kyc_data(origin, entity_id)
    #[benchmark]
    fn purge_kyc_data() {
        let c: T::AccountId = whitelisted_caller();
        insert_rejected::<T>(ENTITY_1, &c);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1);
    }

    // call_index(19): remove_entity_requirement(origin, entity_id)
    #[benchmark]
    fn remove_entity_requirement() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        EntityRequirements::<T>::insert(ENTITY_1, EntityKycRequirement {
            min_level: KycLevel::Basic,
            mandatory: true,
            grace_period: 100,
            allow_high_risk_countries: false,
            max_risk_score: 80,
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1);
    }

    // call_index(20): timeout_pending_kyc(origin, entity_id, account)
    #[benchmark]
    fn timeout_pending_kyc() {
        let c: T::AccountId = whitelisted_caller();
        let user = make_user::<T>(99);
        setup_entity_for::<T>(ENTITY_1, &c);
        // 在 block 1 插入 pending 记录
        frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(1u32));
        insert_pending::<T>(ENTITY_1, &user);
        // 推进到超时之后
        let timeout = T::PendingKycTimeout::get();
        let past_timeout = BlockNumberFor::<T>::from(1u32).saturating_add(timeout).saturating_add(BlockNumberFor::<T>::from(1u32));
        frame_system::Pallet::<T>::set_block_number(past_timeout);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user);
    }

    // call_index(21): batch_revoke_by_provider(origin, entity_id, provider_account, accounts, reason) — AdminOrigin
    #[benchmark]
    fn batch_revoke_by_provider(n: Linear<1, 100>) {
        let provider = make_user::<T>(88);
        do_register_provider::<T>(&provider);
        let mut accts = Vec::new();
        for i in 0..n {
            let user: T::AccountId = account("batch_user", i, 0);
            insert_approved::<T>(ENTITY_1, &user, &provider, KycLevel::Basic);
            accts.push(user);
        }
        let bounded: BoundedVec<T::AccountId, ConstU32<100>> = accts.try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, provider, bounded, RejectionReason::SuspiciousActivity);
    }

    // call_index(23): authorize_provider(origin, entity_id, provider_account)
    #[benchmark]
    fn authorize_provider() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let provider = make_user::<T>(88);
        do_register_provider::<T>(&provider);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, provider);
    }

    // call_index(24): deauthorize_provider(origin, entity_id, provider_account)
    #[benchmark]
    fn deauthorize_provider() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let provider = make_user::<T>(88);
        do_authorize::<T>(&provider, ENTITY_1);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, provider);
    }

    // call_index(25): entity_revoke_kyc(origin, entity_id, account, reason)
    #[benchmark]
    fn entity_revoke_kyc() {
        let c: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &c);
        let user = make_user::<T>(99);
        insert_approved::<T>(ENTITY_1, &user, &c, KycLevel::Basic);
        #[extrinsic_call]
        _(RawOrigin::Signed(c), ENTITY_1, user, RejectionReason::SuspiciousActivity);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
