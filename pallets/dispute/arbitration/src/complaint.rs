use crate::pallet::*;
use frame_support::traits::{
    Get,
    tokens::Precision,
    fungible::MutateHold as FungibleMutateHold,
};
use sp_runtime::{traits::Zero, Saturating};

impl<T: Config> Pallet<T> {
    pub(crate) fn slash_complaint_deposit(
        complaint_id: u64,
        complainant: &T::AccountId,
        respondent: &T::AccountId,
        domain: [u8; 8],
        complaint_type: &crate::types::ComplaintType,
    ) {
        if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
            let slash_bps = DomainPenaltyRates::<T>::get(domain)
                .unwrap_or_else(|| {
                    let type_rate = complaint_type.penalty_rate();
                    if type_rate > 0 { type_rate } else { T::ComplaintSlashBps::get() }
                });
            let slash_amount = sp_runtime::Permill::from_parts((slash_bps as u32) * 100)
                .mul_floor(deposit_amount);
            let return_amount = deposit_amount.saturating_sub(slash_amount);

            if !slash_amount.is_zero() {
                let _ = T::Fungible::transfer_on_hold(
                    &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                    complainant,
                    respondent,
                    slash_amount,
                    Precision::BestEffort,
                    frame_support::traits::tokens::Restriction::Free,
                    frame_support::traits::tokens::Fortitude::Polite,
                );
            }
            if !return_amount.is_zero() {
                let _ = T::Fungible::release(
                    &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                    complainant,
                    return_amount,
                    Precision::BestEffort,
                );
            }
        }
    }

    pub(crate) fn refund_complaint_deposit(complaint_id: u64, complainant: &T::AccountId) {
        if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
            let _ = T::Fungible::release(
                &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                complainant,
                deposit_amount,
                Precision::BestEffort,
            );
        }
    }

    pub(crate) fn decrement_active_count(who: &T::AccountId) {
        ActiveComplaintCount::<T>::mutate(who, |c| *c = c.saturating_sub(1));
    }
}
