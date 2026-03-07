//! Benchmarking for pallet-entity-disclosure
//!
//! 全部 39 个 extrinsics 均有 benchmark。
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::Saturating;

const ENTITY_1: u64 = 1;

// ==================== Helper 函数 ====================

/// 在 test 环境下设置 mock 状态
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
        crate::mock::set_entity_owner(_eid, id);
    }
}

/// 插入一条 Published 披露记录
fn seed_published_disclosure<T: Config>(entity_id: u64, who: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let id = NextDisclosureId::<T>::get();
    let cid: BoundedVec<u8, T::MaxCidLength> = b"QmBenchCid".to_vec().try_into().unwrap();
    Disclosures::<T>::insert(id, DisclosureRecord {
        id,
        entity_id,
        disclosure_type: DisclosureType::AnnualReport,
        content_cid: cid,
        summary_cid: None,
        discloser: who.clone(),
        disclosed_at: now,
        status: DisclosureStatus::Published,
        previous_id: None,
    });
    NextDisclosureId::<T>::put(id.saturating_add(1));
    EntityDisclosures::<T>::mutate(entity_id, |h| { let _ = h.try_push(id); });
    id
}

/// 插入一条 Draft 披露记录
fn seed_draft_disclosure<T: Config>(entity_id: u64, who: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let id = NextDisclosureId::<T>::get();
    let cid: BoundedVec<u8, T::MaxCidLength> = b"QmDraftCid".to_vec().try_into().unwrap();
    Disclosures::<T>::insert(id, DisclosureRecord {
        id,
        entity_id,
        disclosure_type: DisclosureType::AnnualReport,
        content_cid: cid,
        summary_cid: None,
        discloser: who.clone(),
        disclosed_at: now,
        status: DisclosureStatus::Draft,
        previous_id: None,
    });
    NextDisclosureId::<T>::put(id.saturating_add(1));
    id
}

/// 插入一条 Active 公告记录
fn seed_announcement<T: Config>(entity_id: u64, who: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let id = NextAnnouncementId::<T>::get();
    let title: BoundedVec<u8, T::MaxTitleLength> = b"Bench Title".to_vec().try_into().unwrap();
    let cid: BoundedVec<u8, T::MaxCidLength> = b"QmAnnCid".to_vec().try_into().unwrap();
    Announcements::<T>::insert(id, AnnouncementRecord {
        id,
        entity_id,
        category: AnnouncementCategory::General,
        title,
        content_cid: cid,
        publisher: who.clone(),
        published_at: now,
        expires_at: None,
        status: AnnouncementStatus::Active,
        is_pinned: false,
    });
    NextAnnouncementId::<T>::put(id.saturating_add(1));
    EntityAnnouncements::<T>::mutate(entity_id, |h| { let _ = h.try_push(id); });
    id
}

/// 插入一条带过期时间的 Active 公告
fn seed_expiring_announcement<T: Config>(entity_id: u64, who: &T::AccountId, expires_at: BlockNumberFor<T>) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let id = NextAnnouncementId::<T>::get();
    let title: BoundedVec<u8, T::MaxTitleLength> = b"Expiring".to_vec().try_into().unwrap();
    let cid: BoundedVec<u8, T::MaxCidLength> = b"QmExpCid".to_vec().try_into().unwrap();
    Announcements::<T>::insert(id, AnnouncementRecord {
        id,
        entity_id,
        category: AnnouncementCategory::Promotion,
        title,
        content_cid: cid,
        publisher: who.clone(),
        published_at: now,
        expires_at: Some(expires_at),
        status: AnnouncementStatus::Active,
        is_pinned: false,
    });
    NextAnnouncementId::<T>::put(id.saturating_add(1));
    EntityAnnouncements::<T>::mutate(entity_id, |h| { let _ = h.try_push(id); });
    id
}

/// 插入披露配置
fn seed_disclosure_config<T: Config>(entity_id: u64, level: DisclosureLevel) {
    let now = frame_system::Pallet::<T>::block_number();
    DisclosureConfigs::<T>::insert(entity_id, DisclosureConfig {
        level,
        insider_trading_control: true,
        blackout_period_after: 50u32.into(),
        next_required_disclosure: now.saturating_add(500u32.into()),
        last_disclosure: now,
        violation_count: 0,
    });
}

/// 添加内幕人员
fn seed_insider<T: Config>(entity_id: u64, account: &T::AccountId, role: InsiderRole) {
    let now = frame_system::Pallet::<T>::block_number();
    Insiders::<T>::mutate(entity_id, |insiders| {
        let _ = insiders.try_push(InsiderRecord {
            account: account.clone(),
            role,
            added_at: now,
        });
    });
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ==================== call_index(0): configure_disclosure ====================
    #[benchmark]
    fn configure_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, DisclosureLevel::Standard, true, 50u32.into());
    }

    // ==================== call_index(1): publish_disclosure ====================
    #[benchmark]
    fn publish_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, DisclosureType::QuarterlyReport,
          b"QmBenchContent".to_vec(), Some(b"QmSummary".to_vec()));
    }

    // ==================== call_index(2): withdraw_disclosure ====================
    #[benchmark]
    fn withdraw_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let did = seed_published_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did);
    }

    // ==================== call_index(3): correct_disclosure ====================
    #[benchmark]
    fn correct_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        let did = seed_published_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did, b"QmCorrected".to_vec(), None);
    }

    // ==================== call_index(4): add_insider ====================
    #[benchmark]
    fn add_insider() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let insider: T::AccountId = account("insider", 0, 0);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, insider, InsiderRole::Admin);
    }

    // ==================== call_index(5): remove_insider ====================
    #[benchmark]
    fn remove_insider() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let insider: T::AccountId = account("insider", 0, 0);
        seed_insider::<T>(ENTITY_1, &insider, InsiderRole::Admin);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, insider);
    }

    // ==================== call_index(6): start_blackout ====================
    #[benchmark]
    fn start_blackout() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 100u32.into());
    }

    // ==================== call_index(7): end_blackout ====================
    #[benchmark]
    fn end_blackout() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        BlackoutPeriods::<T>::insert(ENTITY_1, (now, now.saturating_add(100u32.into())));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(8): publish_announcement ====================
    #[benchmark]
    fn publish_announcement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, AnnouncementCategory::General,
          b"Bench Title".to_vec(), b"QmBenchCid".to_vec(), None);
    }

    // ==================== call_index(9): update_announcement ====================
    #[benchmark]
    fn update_announcement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let aid = seed_announcement::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), aid,
          Some(b"New Title".to_vec()), Some(b"QmNewCid".to_vec()),
          Some(AnnouncementCategory::SystemUpdate), None);
    }

    // ==================== call_index(10): withdraw_announcement ====================
    #[benchmark]
    fn withdraw_announcement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let aid = seed_announcement::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), aid);
    }

    // ==================== call_index(11): pin_announcement ====================
    #[benchmark]
    fn pin_announcement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let aid = seed_announcement::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, aid);
    }

    // ==================== call_index(12): expire_announcement ====================
    #[benchmark]
    fn expire_announcement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        // 创建一个已过期的公告（expires_at = 1，当前 block > 1）
        let now = frame_system::Pallet::<T>::block_number();
        let expires = now.saturating_add(1u32.into());
        let aid = seed_expiring_announcement::<T>(ENTITY_1, &caller, expires);
        // 推进区块使其过期
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(10u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), aid);
    }

    // ==================== call_index(13): cleanup_disclosure_history ====================
    #[benchmark]
    fn cleanup_disclosure_history() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let did = seed_published_disclosure::<T>(ENTITY_1, &caller);
        // 标记为 Withdrawn 使其可清理
        Disclosures::<T>::mutate(did, |r| { if let Some(rec) = r { rec.status = DisclosureStatus::Withdrawn; } });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, did);
    }

    // ==================== call_index(14): cleanup_announcement_history ====================
    #[benchmark]
    fn cleanup_announcement_history() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let aid = seed_announcement::<T>(ENTITY_1, &caller);
        // 标记为 Withdrawn 使其可清理
        Announcements::<T>::mutate(aid, |r| { if let Some(rec) = r { rec.status = AnnouncementStatus::Withdrawn; } });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, aid);
    }

    // ==================== call_index(15): report_disclosure_violation ====================
    #[benchmark]
    fn report_disclosure_violation() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        // 配置披露并使其逾期
        let now = frame_system::Pallet::<T>::block_number();
        DisclosureConfigs::<T>::insert(ENTITY_1, DisclosureConfig {
            level: DisclosureLevel::Standard,
            insider_trading_control: false,
            blackout_period_after: BlockNumberFor::<T>::from(0u32),
            next_required_disclosure: now, // 已逾期
            last_disclosure: now,
            violation_count: 0,
        });
        // 推进使其逾期
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(10u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, ViolationType::LateDisclosure);
    }

    // ==================== call_index(16): force_configure_disclosure ====================
    #[benchmark]
    fn force_configure_disclosure() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, DisclosureLevel::Full, true, 100u32.into());
    }

    // ==================== call_index(17): cleanup_entity_disclosure ====================
    #[benchmark]
    fn cleanup_entity_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        #[cfg(test)]
        {
            crate::mock::set_entity_status(ENTITY_1, pallet_entity_common::EntityStatus::Closed);
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(18): create_draft_disclosure ====================
    #[benchmark]
    fn create_draft_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, DisclosureType::QuarterlyReport,
          b"QmDraftContent".to_vec(), None);
    }

    // ==================== call_index(19): update_draft ====================
    #[benchmark]
    fn update_draft() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let did = seed_draft_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did, b"QmUpdatedDraft".to_vec(), None);
    }

    // ==================== call_index(20): delete_draft ====================
    #[benchmark]
    fn delete_draft() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let did = seed_draft_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did);
    }

    // ==================== call_index(21): publish_draft ====================
    #[benchmark]
    fn publish_draft() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        let did = seed_draft_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did);
    }

    // ==================== call_index(22): update_insider_role ====================
    #[benchmark]
    fn update_insider_role() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let insider: T::AccountId = account("insider", 0, 0);
        seed_insider::<T>(ENTITY_1, &insider, InsiderRole::Admin);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, insider, InsiderRole::Auditor);
    }

    // ==================== call_index(23): unpin_announcement ====================
    #[benchmark]
    fn unpin_announcement() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let aid = seed_announcement::<T>(ENTITY_1, &caller);
        // 先置顶
        PinnedAnnouncements::<T>::mutate(ENTITY_1, |p| { let _ = p.try_push(aid); });
        Announcements::<T>::mutate(aid, |r| { if let Some(rec) = r { rec.is_pinned = true; } });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, aid);
    }

    // ==================== call_index(24): batch_add_insiders ====================
    #[benchmark]
    fn batch_add_insiders() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let insider1: T::AccountId = account("insider", 0, 0);
        let insider2: T::AccountId = account("insider", 1, 0);
        let list: BoundedVec<(T::AccountId, InsiderRole), T::MaxInsiders> =
            vec![(insider1, InsiderRole::Admin), (insider2, InsiderRole::Auditor)]
            .try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, list);
    }

    // ==================== call_index(25): batch_remove_insiders ====================
    #[benchmark]
    fn batch_remove_insiders() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let insider1: T::AccountId = account("insider", 0, 0);
        let insider2: T::AccountId = account("insider", 1, 0);
        seed_insider::<T>(ENTITY_1, &insider1, InsiderRole::Admin);
        seed_insider::<T>(ENTITY_1, &insider2, InsiderRole::Auditor);
        let list: BoundedVec<T::AccountId, T::MaxInsiders> =
            vec![insider1, insider2].try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, list);
    }

    // ==================== call_index(26): reset_violation_count ====================
    #[benchmark]
    fn reset_violation_count() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        HighRiskEntities::<T>::insert(ENTITY_1, true);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(27): expire_blackout ====================
    #[benchmark]
    fn expire_blackout() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        // 设置一个已过期的黑窗口期
        BlackoutPeriods::<T>::insert(ENTITY_1, (now, now.saturating_add(1u32.into())));
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(10u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(28): configure_approval_requirements ====================
    #[benchmark]
    fn configure_approval_requirements() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 2u32, 0x06u8); // Admin + Auditor
    }

    // ==================== call_index(29): approve_disclosure ====================
    #[benchmark]
    fn approve_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        // 添加 caller 为内幕人员（Auditor）
        seed_insider::<T>(ENTITY_1, &caller, InsiderRole::Auditor);
        // 配置审批要求
        ApprovalConfigs::<T>::insert(ENTITY_1, ApprovalConfig {
            required_approvals: 1,
            allowed_roles: 0x04, // Auditor
        });
        let did = seed_draft_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did);
    }

    // ==================== call_index(30): reject_disclosure ====================
    #[benchmark]
    fn reject_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_insider::<T>(ENTITY_1, &caller, InsiderRole::Auditor);
        ApprovalConfigs::<T>::insert(ENTITY_1, ApprovalConfig {
            required_approvals: 1,
            allowed_roles: 0x04,
        });
        let did = seed_draft_disclosure::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did);
    }

    // ==================== call_index(31): publish_emergency_disclosure ====================
    #[benchmark]
    fn publish_emergency_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Full);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, DisclosureType::MaterialEvent,
          b"QmEmergency".to_vec(), None);
    }

    // ==================== call_index(32): report_insider_transaction ====================
    #[benchmark]
    fn report_insider_transaction() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_insider::<T>(ENTITY_1, &caller, InsiderRole::Admin);
        let now = frame_system::Pallet::<T>::block_number();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, InsiderTransactionType::Buy, 10_000u128, now);
    }

    // ==================== call_index(33): configure_fiscal_year ====================
    #[benchmark]
    fn configure_fiscal_year() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 1u32.into(), 1000u32.into());
    }

    // ==================== call_index(34): escalate_penalty ====================
    #[benchmark]
    fn escalate_penalty() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, PenaltyLevel::Warning);
    }

    // ==================== call_index(35): reset_penalty ====================
    #[benchmark]
    fn reset_penalty() {
        setup_entity_for::<T>(ENTITY_1, &whitelisted_caller::<T::AccountId>());
        EntityPenalties::<T>::insert(ENTITY_1, PenaltyLevel::Restricted);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(36): cleanup_expired_cooldowns ====================
    #[benchmark]
    fn cleanup_expired_cooldowns() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        // 插入一些已过期的冷静期记录
        let now = frame_system::Pallet::<T>::block_number();
        for i in 0..5u32 {
            let acct: T::AccountId = account("removed", i, 0);
            RemovedInsiders::<T>::insert(ENTITY_1, &acct, now);
        }
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(10u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 10u32);
    }

    // ==================== call_index(37): set_disclosure_metadata ====================
    #[benchmark]
    fn set_disclosure_metadata() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        seed_disclosure_config::<T>(ENTITY_1, DisclosureLevel::Standard);
        let did = seed_published_disclosure::<T>(ENTITY_1, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did,
          Some(now), Some(now.saturating_add(100u32.into())), true);
    }

    // ==================== call_index(38): audit_disclosure ====================
    #[benchmark]
    fn audit_disclosure() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        // caller 必须是 Auditor 角色的内幕人员
        seed_insider::<T>(ENTITY_1, &caller, InsiderRole::Auditor);
        let did = seed_published_disclosure::<T>(ENTITY_1, &caller);
        // 设置元数据为 Pending 审计状态
        DisclosureMetadataStore::<T>::insert(did, DisclosureMetadata {
            period_start: None,
            period_end: None,
            audit_status: AuditStatus::Pending,
            is_emergency: false,
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), did, true);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
