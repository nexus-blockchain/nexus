use crate::pallet::*;
use crate::types::{ArchivedComplaint, ComplaintStatus};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::{Saturating, SaturatedConversion};

impl<T: Config> Pallet<T> {
    pub fn expire_old_complaints(max_count: u32) -> u32 {
        let now = frame_system::Pallet::<T>::block_number();
        let mut expired_count = 0u32;
        let mut cursor = ComplaintExpiryCursor::<T>::get();
        let max_id = NextComplaintId::<T>::get();

        while expired_count < max_count && cursor < max_id {
            if let Some(mut complaint) = Complaints::<T>::get(cursor) {
                let should_expire = if complaint.status == ComplaintStatus::Submitted {
                    now > complaint.response_deadline
                } else if complaint.status.is_active() {
                    now.saturating_sub(complaint.created_at) >= T::ComplaintMaxLifetimeBlocks::get()
                } else {
                    false
                };
                if should_expire {
                    complaint.status = ComplaintStatus::Expired;
                    complaint.updated_at = now;

                    Self::refund_complaint_deposit(cursor, &complaint.complainant);

                    Complaints::<T>::insert(cursor, &complaint);

                    DomainStats::<T>::mutate(complaint.domain, |stats| {
                        stats.resolved_count = stats.resolved_count.saturating_add(1);
                        stats.expired_count = stats.expired_count.saturating_add(1);
                    });

                    Self::decrement_active_count(&complaint.complainant);

                    PendingArbitrationComplaints::<T>::remove(cursor);

                    Self::deposit_event(Event::ComplaintExpired { complaint_id: cursor });
                    expired_count = expired_count.saturating_add(1);
                }
            }
            cursor = cursor.saturating_add(1);
        }

        let final_cursor = if cursor >= max_id && max_id > 0 { 0 } else { cursor };
        ComplaintExpiryCursor::<T>::put(final_cursor);
        expired_count
    }

    pub fn auto_escalate_stale_complaints(max_count: u32) -> u32 {
        let now = frame_system::Pallet::<T>::block_number();
        let auto_escalate = T::AutoEscalateBlocks::get();
        let mut escalated_count = 0u32;
        let mut cursor = AutoEscalateCursor::<T>::get();
        let max_id = NextComplaintId::<T>::get();

        while escalated_count < max_count && cursor < max_id {
            if let Some(mut complaint) = Complaints::<T>::get(cursor) {
                if complaint.status == ComplaintStatus::Responded
                    && now.saturating_sub(complaint.updated_at) >= auto_escalate
                {
                    complaint.status = ComplaintStatus::Arbitrating;
                    complaint.updated_at = now;
                    Complaints::<T>::insert(cursor, &complaint);
                    PendingArbitrationComplaints::<T>::insert(cursor, ());

                    Self::deposit_event(Event::ComplaintAutoEscalated { complaint_id: cursor });
                    escalated_count = escalated_count.saturating_add(1);
                }
            }
            cursor = cursor.saturating_add(1);
        }

        let final_cursor = if cursor >= max_id && max_id > 0 { 0 } else { cursor };
        AutoEscalateCursor::<T>::put(final_cursor);
        escalated_count
    }

    pub fn archive_old_complaints(max_count: u32) -> u32 {
        let now = frame_system::Pallet::<T>::block_number();
        let archive_delay: BlockNumberFor<T> = T::ComplaintArchiveDelayBlocks::get();
        let appeal_window: BlockNumberFor<T> = T::AppealWindowBlocks::get();
        let mut archived_count = 0u32;
        let mut cursor = ComplaintArchiveCursor::<T>::get();
        let max_id = NextComplaintId::<T>::get();

        while archived_count < max_count && cursor < max_id {
            if let Some(complaint) = Complaints::<T>::get(cursor) {
                let time_since_update = now.saturating_sub(complaint.updated_at);
                let can_archive = complaint.status.is_resolved()
                    && time_since_update >= archive_delay
                    && time_since_update >= appeal_window;

                if can_archive {
                    let decision = match complaint.status {
                        ComplaintStatus::ResolvedComplainantWin => 0,
                        ComplaintStatus::ResolvedRespondentWin => 1,
                        ComplaintStatus::ResolvedSettlement => 2,
                        ComplaintStatus::Withdrawn => 3,
                        ComplaintStatus::Expired => 4,
                        _ => 2,
                    };

                    let current_block: u64 = now.saturated_into();
                    let archived = ArchivedComplaint {
                        id: cursor,
                        domain: complaint.domain,
                        object_id: complaint.object_id,
                        decision,
                        resolved_at: current_block,
                        year_month: pallet_storage_lifecycle::block_to_year_month(current_block.saturated_into(), 14400),
                    };

                    ArchivedComplaints::<T>::insert(cursor, archived);
                    Complaints::<T>::remove(cursor);

                    // Clean up associated storage to prevent state bloat
                    ComplaintEvidenceCids::<T>::remove(cursor);
                    ComplaintCooldown::<T>::remove(complaint.domain, complaint.object_id);

                    archived_count = archived_count.saturating_add(1);
                    Self::deposit_event(Event::ComplaintArchived { complaint_id: cursor });
                }
            }
            cursor = cursor.saturating_add(1);
        }

        let final_cursor = if cursor >= max_id && max_id > 0 { 0 } else { cursor };
        ComplaintArchiveCursor::<T>::put(final_cursor);
        archived_count
    }

    pub(crate) fn cleanup_old_archived_disputes(current_block: u64, max_per_call: u32) -> u32 {
        let ttl = T::ArchiveTtlBlocks::get();
        if ttl == 0 { return 0; }
        let ttl_u64 = ttl as u64;
        let mut cursor = ArchiveDisputeCleanupCursor::<T>::get();
        let max_id = NextArchivedId::<T>::get();
        let mut cleaned = 0u32;

        while cursor < max_id && cleaned < max_per_call {
            if let Some(archived) = ArchivedDisputes::<T>::get(cursor) {
                if current_block.saturating_sub(archived.completed_at) > ttl_u64 {
                    ArchivedDisputes::<T>::remove(cursor);
                    cleaned += 1;
                }
            }
            cursor = cursor.saturating_add(1);
        }

        let final_cursor = if cursor >= max_id && max_id > 0 { 0 } else { cursor };
        ArchiveDisputeCleanupCursor::<T>::put(final_cursor);
        cleaned
    }

    pub(crate) fn cleanup_old_archived_complaints(current_block: u64, max_per_call: u32) -> u32 {
        let ttl = T::ArchiveTtlBlocks::get();
        if ttl == 0 { return 0; }
        let ttl_u64 = ttl as u64;
        let mut cursor = ArchiveComplaintCleanupCursor::<T>::get();
        let next_complaint = NextComplaintId::<T>::get();
        let mut cleaned = 0u32;

        while cursor < next_complaint && cleaned < max_per_call {
            if let Some(archived) = ArchivedComplaints::<T>::get(cursor) {
                if current_block.saturating_sub(archived.resolved_at) > ttl_u64 {
                    ArchivedComplaints::<T>::remove(cursor);
                    cleaned += 1;
                }
            }
            cursor = cursor.saturating_add(1);
        }

        let final_cursor = if cursor >= next_complaint && next_complaint > 0 { 0 } else { cursor };
        ArchiveComplaintCleanupCursor::<T>::put(final_cursor);
        cleaned
    }
}
