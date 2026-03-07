//! Runtime API for pallet-dispute-arbitration
//!
//! Provides:
//! - `get_complaints_by_status`: paginated query by status
//! - `get_user_complaints`: active complaint IDs for a user
//! - `get_complaint_detail`: aggregated complaint + evidence + deposit

use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use crate::types::{ComplaintStatus, ComplaintType};

extern crate alloc;
use alloc::vec::Vec;

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct ComplaintSummary<AccountId, Balance> {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    pub complaint_type: ComplaintType,
    pub complainant: AccountId,
    pub respondent: AccountId,
    pub amount: Option<Balance>,
    pub status: ComplaintStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct ComplaintDetail<AccountId, Balance> {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    pub complaint_type: ComplaintType,
    pub complainant: AccountId,
    pub respondent: AccountId,
    pub details_cid: Vec<u8>,
    pub response_cid: Option<Vec<u8>>,
    pub amount: Option<Balance>,
    pub status: ComplaintStatus,
    pub created_at: u64,
    pub response_deadline: u64,
    pub settlement_cid: Option<Vec<u8>>,
    pub resolution_cid: Option<Vec<u8>>,
    pub appeal_cid: Option<Vec<u8>>,
    pub appellant: Option<AccountId>,
    pub updated_at: u64,
    pub deposit: Option<Balance>,
    pub evidence_cids: Vec<Vec<u8>>,
}

sp_api::decl_runtime_apis! {
    pub trait ArbitrationApi<AccountId, Balance>
    where
        AccountId: Codec,
        Balance: Codec,
    {
        /// Paginated complaints filtered by status
        fn get_complaints_by_status(status: ComplaintStatus, offset: u32, limit: u32) -> Vec<ComplaintSummary<AccountId, Balance>>;

        /// Active complaint IDs for a given user (as complainant or respondent)
        fn get_user_complaints(account: AccountId) -> Vec<u64>;

        /// Full complaint detail: complaint + deposit + evidence CIDs in one call
        fn get_complaint_detail(complaint_id: u64) -> Option<ComplaintDetail<AccountId, Balance>>;
    }
}
