use crate::types::ComplaintStatus;
use crate::types::ComplaintStatus::*;

pub fn can_transition(from: &ComplaintStatus, to: &ComplaintStatus) -> bool {
    matches!(
        (from, to),
        // Submitted ->
        (Submitted, Responded)
        | (Submitted, Withdrawn)
        | (Submitted, Expired)
        // Responded -> (includes Withdrawn for voluntary withdrawal)
        | (Responded, Withdrawn)
        // Responded ->
        | (Responded, Mediating)
        | (Responded, Arbitrating)
        | (Responded, ResolvedSettlement)
        | (Responded, Expired)
        // Mediating ->
        | (Mediating, Arbitrating)
        | (Mediating, ResolvedSettlement)
        | (Mediating, Expired)
        // Arbitrating ->
        | (Arbitrating, ResolvedComplainantWin)
        | (Arbitrating, ResolvedRespondentWin)
        | (Arbitrating, Expired)
        // Appeal window (resolved -> appealed)
        | (ResolvedComplainantWin, Appealed)
        | (ResolvedRespondentWin, Appealed)
        // Appeal resolution (final)
        | (Appealed, ResolvedComplainantWin)
        | (Appealed, ResolvedRespondentWin)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions_accepted() {
        assert!(can_transition(&Submitted, &Responded));
        assert!(can_transition(&Submitted, &Withdrawn));
        assert!(can_transition(&Submitted, &Expired));
        assert!(can_transition(&Responded, &Mediating));
        assert!(can_transition(&Responded, &Arbitrating));
        assert!(can_transition(&Responded, &ResolvedSettlement));
        assert!(can_transition(&Mediating, &Arbitrating));
        assert!(can_transition(&Arbitrating, &ResolvedComplainantWin));
        assert!(can_transition(&Arbitrating, &ResolvedRespondentWin));
        assert!(can_transition(&ResolvedComplainantWin, &Appealed));
        assert!(can_transition(&ResolvedRespondentWin, &Appealed));
        assert!(can_transition(&Appealed, &ResolvedComplainantWin));
        assert!(can_transition(&Appealed, &ResolvedRespondentWin));
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert!(!can_transition(&Submitted, &Arbitrating));
        assert!(!can_transition(&Submitted, &ResolvedComplainantWin));
        assert!(can_transition(&Responded, &Withdrawn));
        assert!(!can_transition(&Arbitrating, &Responded));
        assert!(!can_transition(&Expired, &Submitted));
        assert!(!can_transition(&Withdrawn, &Submitted));
        assert!(!can_transition(&ResolvedSettlement, &Appealed));
        assert!(!can_transition(&Appealed, &ResolvedSettlement));
    }

    #[test]
    fn status_predicates() {
        assert!(ResolvedComplainantWin.is_resolved());
        assert!(ResolvedRespondentWin.is_resolved());
        assert!(ResolvedSettlement.is_resolved());
        assert!(Withdrawn.is_resolved());
        assert!(Expired.is_resolved());
        assert!(!Submitted.is_resolved());
        assert!(!Responded.is_resolved());
        assert!(!Arbitrating.is_resolved());
        assert!(!Appealed.is_resolved());

        assert!(Submitted.is_active());
        assert!(Responded.is_active());
        assert!(Mediating.is_active());
        assert!(Arbitrating.is_active());
        assert!(Appealed.is_active());
        assert!(!ResolvedComplainantWin.is_active());
        assert!(!Withdrawn.is_active());
    }
}
