use super::*;

// ============================================================================
// EntityType tests
// ============================================================================

#[test]
fn entity_type_default_governance() {
    assert_eq!(EntityType::Merchant.default_governance(), GovernanceMode::None);
    assert_eq!(EntityType::Enterprise.default_governance(), GovernanceMode::FullDAO);
    assert_eq!(EntityType::DAO.default_governance(), GovernanceMode::FullDAO);
    assert_eq!(EntityType::Community.default_governance(), GovernanceMode::None);
    assert_eq!(EntityType::Project.default_governance(), GovernanceMode::FullDAO);
    assert_eq!(EntityType::Fund.default_governance(), GovernanceMode::FullDAO);
    assert_eq!(EntityType::ServiceProvider.default_governance(), GovernanceMode::None);
    assert_eq!(EntityType::Custom(99).default_governance(), GovernanceMode::None);
}

#[test]
fn entity_type_default_token_type() {
    assert_eq!(EntityType::Merchant.default_token_type(), TokenType::Points);
    assert_eq!(EntityType::DAO.default_token_type(), TokenType::Governance);
    assert_eq!(EntityType::Enterprise.default_token_type(), TokenType::Equity);
    assert_eq!(EntityType::Community.default_token_type(), TokenType::Membership);
    assert_eq!(EntityType::Fund.default_token_type(), TokenType::Share);
}

#[test]
fn entity_type_requires_kyc() {
    assert!(EntityType::Enterprise.requires_kyc_by_default());
    assert!(EntityType::Fund.requires_kyc_by_default());
    assert!(EntityType::Project.requires_kyc_by_default());
    assert!(!EntityType::Merchant.requires_kyc_by_default());
    assert!(!EntityType::DAO.requires_kyc_by_default());
    assert!(!EntityType::Community.requires_kyc_by_default());
}

#[test]
fn entity_type_suggests_token_type() {
    // Merchant should not suggest Equity
    assert!(!EntityType::Merchant.suggests_token_type(&TokenType::Equity));
    assert!(!EntityType::Merchant.suggests_token_type(&TokenType::Bond));
    // DAO should not suggest Points
    assert!(!EntityType::DAO.suggests_token_type(&TokenType::Points));
    // Fund should not suggest Points
    assert!(!EntityType::Fund.suggests_token_type(&TokenType::Points));
    // Normal combos
    assert!(EntityType::Enterprise.suggests_token_type(&TokenType::Equity));
    assert!(EntityType::DAO.suggests_token_type(&TokenType::Governance));
}

#[test]
fn entity_type_suggests_governance() {
    assert!(!EntityType::DAO.suggests_governance(&GovernanceMode::None));
    assert!(!EntityType::Fund.suggests_governance(&GovernanceMode::FullDAO));
    assert!(EntityType::Enterprise.suggests_governance(&GovernanceMode::FullDAO));
}

#[test]
fn entity_type_default_transfer_restriction() {
    assert_eq!(EntityType::Merchant.default_transfer_restriction(), TransferRestrictionMode::None);
    assert_eq!(EntityType::Enterprise.default_transfer_restriction(), TransferRestrictionMode::Whitelist);
    assert_eq!(EntityType::DAO.default_transfer_restriction(), TransferRestrictionMode::None);
    assert_eq!(EntityType::Project.default_transfer_restriction(), TransferRestrictionMode::KycRequired);
}

// ============================================================================
// EffectiveShopStatus::compute tests
// ============================================================================

#[test]
fn compute_entity_banned_forces_closed() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Banned, &ShopOperatingStatus::Active),
        EffectiveShopStatus::ClosedByEntity
    );
}

#[test]
fn compute_entity_closed_forces_closed() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Closed, &ShopOperatingStatus::Active),
        EffectiveShopStatus::ClosedByEntity
    );
}

#[test]
fn compute_entity_suspended_pauses_shop() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Suspended, &ShopOperatingStatus::Active),
        EffectiveShopStatus::PausedByEntity
    );
}

#[test]
fn compute_entity_suspended_but_shop_closed_shows_closed() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Suspended, &ShopOperatingStatus::Closed),
        EffectiveShopStatus::Closed
    );
}

#[test]
fn compute_entity_active_shop_active() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Active, &ShopOperatingStatus::Active),
        EffectiveShopStatus::Active
    );
}

#[test]
fn compute_entity_active_shop_paused() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Active, &ShopOperatingStatus::Paused),
        EffectiveShopStatus::PausedBySelf
    );
}

#[test]
fn compute_entity_active_shop_fund_depleted() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Active, &ShopOperatingStatus::FundDepleted),
        EffectiveShopStatus::FundDepleted
    );
}

#[test]
fn compute_entity_active_shop_closing() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Active, &ShopOperatingStatus::Closing),
        EffectiveShopStatus::Closing
    );
}

#[test]
fn compute_entity_pending_pauses_shop() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Pending, &ShopOperatingStatus::Active),
        EffectiveShopStatus::PausedByEntity
    );
}

#[test]
fn effective_shop_status_is_operational() {
    assert!(EffectiveShopStatus::Active.is_operational());
    assert!(!EffectiveShopStatus::PausedBySelf.is_operational());
    assert!(!EffectiveShopStatus::ClosedByEntity.is_operational());
}

#[test]
fn effective_shop_status_is_entity_caused() {
    assert!(EffectiveShopStatus::PausedByEntity.is_entity_caused());
    assert!(EffectiveShopStatus::ClosedByEntity.is_entity_caused());
    assert!(!EffectiveShopStatus::PausedBySelf.is_entity_caused());
    assert!(!EffectiveShopStatus::Active.is_entity_caused());
}

// ============================================================================
// MemberRegistrationPolicy tests
// ============================================================================

#[test]
fn member_policy_open_by_default() {
    let policy = MemberRegistrationPolicy::default();
    assert!(policy.is_open());
    assert!(!policy.requires_purchase());
    assert!(!policy.requires_referral());
    assert!(!policy.requires_approval());
}

#[test]
fn member_policy_flags() {
    let policy = MemberRegistrationPolicy(
        MemberRegistrationPolicy::PURCHASE_REQUIRED | MemberRegistrationPolicy::REFERRAL_REQUIRED,
    );
    assert!(!policy.is_open());
    assert!(policy.requires_purchase());
    assert!(policy.requires_referral());
    assert!(!policy.requires_approval());
}

#[test]
fn member_policy_all_flags() {
    let policy = MemberRegistrationPolicy(
        MemberRegistrationPolicy::PURCHASE_REQUIRED
            | MemberRegistrationPolicy::REFERRAL_REQUIRED
            | MemberRegistrationPolicy::APPROVAL_REQUIRED,
    );
    assert!(policy.requires_purchase());
    assert!(policy.requires_referral());
    assert!(policy.requires_approval());
}

// ============================================================================
// TokenType tests
// ============================================================================

#[test]
fn token_type_voting_power() {
    assert!(TokenType::Governance.has_voting_power());
    assert!(TokenType::Equity.has_voting_power());
    assert!(TokenType::Hybrid(0).has_voting_power());
    assert!(!TokenType::Points.has_voting_power());
    assert!(!TokenType::Membership.has_voting_power());
}

#[test]
fn token_type_dividend_rights() {
    assert!(TokenType::Equity.has_dividend_rights());
    assert!(TokenType::Share.has_dividend_rights());
    assert!(TokenType::Hybrid(0).has_dividend_rights());
    assert!(!TokenType::Points.has_dividend_rights());
    assert!(!TokenType::Governance.has_dividend_rights());
}

#[test]
fn token_type_transferable() {
    assert!(!TokenType::Membership.is_transferable_by_default());
    assert!(TokenType::Points.is_transferable_by_default());
    assert!(TokenType::Equity.is_transferable_by_default());
}

#[test]
fn token_type_kyc_levels() {
    assert_eq!(TokenType::Points.required_kyc_level(), (0, 0));
    assert_eq!(TokenType::Membership.required_kyc_level(), (1, 1));
    assert_eq!(TokenType::Equity.required_kyc_level(), (3, 3));
}

#[test]
fn token_type_is_security() {
    assert!(TokenType::Equity.is_security());
    assert!(TokenType::Share.is_security());
    assert!(TokenType::Bond.is_security());
    assert!(!TokenType::Points.is_security());
    assert!(!TokenType::Governance.is_security());
}

#[test]
fn token_type_default_transfer_restriction_returns_enum() {
    assert_eq!(TokenType::Points.default_transfer_restriction(), TransferRestrictionMode::None);
    assert_eq!(TokenType::Membership.default_transfer_restriction(), TransferRestrictionMode::MembersOnly);
    assert_eq!(TokenType::Governance.default_transfer_restriction(), TransferRestrictionMode::KycRequired);
    assert_eq!(TokenType::Equity.default_transfer_restriction(), TransferRestrictionMode::Whitelist);
    assert_eq!(TokenType::Hybrid(0).default_transfer_restriction(), TransferRestrictionMode::None);
}

// ============================================================================
// TransferRestrictionMode tests
// ============================================================================

#[test]
#[allow(deprecated)]
fn transfer_restriction_from_u8() {
    assert_eq!(TransferRestrictionMode::from_u8(0), TransferRestrictionMode::None);
    assert_eq!(TransferRestrictionMode::from_u8(1), TransferRestrictionMode::Whitelist);
    assert_eq!(TransferRestrictionMode::from_u8(2), TransferRestrictionMode::Blacklist);
    assert_eq!(TransferRestrictionMode::from_u8(3), TransferRestrictionMode::KycRequired);
    assert_eq!(TransferRestrictionMode::from_u8(4), TransferRestrictionMode::MembersOnly);
    assert_eq!(TransferRestrictionMode::from_u8(255), TransferRestrictionMode::None);
}

// ============================================================================
// ShopType tests
// ============================================================================

#[test]
fn shop_type_requires_location() {
    assert!(ShopType::PhysicalStore.requires_location());
    assert!(ShopType::Warehouse.requires_location());
    assert!(!ShopType::OnlineStore.requires_location());
    assert!(!ShopType::Virtual.requires_location());
}

#[test]
fn shop_type_supports_physical() {
    assert!(ShopType::OnlineStore.supports_physical_products());
    assert!(ShopType::PhysicalStore.supports_physical_products());
    assert!(!ShopType::Virtual.supports_physical_products());
}

#[test]
fn shop_type_supports_services() {
    assert!(ShopType::ServicePoint.supports_services());
    assert!(ShopType::Virtual.supports_services());
    assert!(!ShopType::Warehouse.supports_services());
}

// ============================================================================
// ShopOperatingStatus tests
// ============================================================================

#[test]
fn shop_operating_status_operational() {
    assert!(ShopOperatingStatus::Active.is_operational());
    assert!(!ShopOperatingStatus::Paused.is_operational());
    assert!(!ShopOperatingStatus::Closed.is_operational());
}

#[test]
fn shop_operating_status_can_resume() {
    assert!(ShopOperatingStatus::Paused.can_resume());
    assert!(ShopOperatingStatus::FundDepleted.can_resume());
    assert!(!ShopOperatingStatus::Active.can_resume());
    assert!(!ShopOperatingStatus::Closed.can_resume());
}

// ============================================================================
// NullPricingProvider test
// ============================================================================

#[test]
fn null_pricing_provider_returns_one() {
    assert_eq!(NullPricingProvider::get_nex_usdt_price(), 1);
}

// ============================================================================
// M1: EffectiveShopStatus::compute — Closing treated consistently
// ============================================================================

#[test]
fn m1_compute_entity_suspended_shop_closing_shows_closing() {
    // Before fix: Closing + Suspended → PausedByEntity (wrong)
    // After fix:  Closing + Suspended → Closing (closing is irreversible, preserves semantic)
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Suspended, &ShopOperatingStatus::Closing),
        EffectiveShopStatus::Closing
    );
}

#[test]
fn m1_compute_entity_pending_close_shop_closing_shows_closing() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::PendingClose, &ShopOperatingStatus::Closing),
        EffectiveShopStatus::Closing
    );
}

#[test]
fn m1_compute_entity_pending_shop_closing_shows_closing() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Pending, &ShopOperatingStatus::Closing),
        EffectiveShopStatus::Closing
    );
}

// ============================================================================
// M2: AdminPermission ALL_DEFINED + is_valid
// ============================================================================

#[test]
fn m2_admin_permission_all_defined_covers_all_bits() {
    assert_eq!(AdminPermission::ALL_DEFINED, 0b0000_0111_1111_1111);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::SHOP_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::MEMBER_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::TOKEN_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::ADS_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::REVIEW_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::DISCLOSURE_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::ENTITY_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::KYC_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::GOVERNANCE_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::ORDER_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::COMMISSION_MANAGE != 0);
}

#[test]
fn m2_admin_permission_is_valid_accepts_defined_bits() {
    assert!(AdminPermission::is_valid(AdminPermission::SHOP_MANAGE));
    assert!(AdminPermission::is_valid(AdminPermission::ALL_DEFINED));
    assert!(AdminPermission::is_valid(
        AdminPermission::SHOP_MANAGE | AdminPermission::MEMBER_MANAGE
    ));
}

#[test]
fn m2_admin_permission_is_valid_rejects_undefined_bits() {
    assert!(!AdminPermission::is_valid(0b1000_0000_0000)); // bit 11 undefined
    assert!(!AdminPermission::is_valid(0xFFFF_FFFF)); // ALL includes undefined
    assert!(!AdminPermission::is_valid(AdminPermission::SHOP_MANAGE | 0x8000_0000));
}

#[test]
fn m2_admin_permission_zero_is_valid() {
    // 0 contains no undefined bits (but is meaningless — registry checks != 0 separately)
    assert!(AdminPermission::is_valid(0));
}

// ============================================================================
// M3: MemberRegistrationPolicy + MemberStatsPolicy is_valid
// ============================================================================

#[test]
fn m3_registration_policy_is_valid_accepts_defined() {
    assert!(MemberRegistrationPolicy(0).is_valid()); // OPEN
    assert!(MemberRegistrationPolicy(0b0000_0111).is_valid()); // all 3 flags
    assert!(MemberRegistrationPolicy(MemberRegistrationPolicy::PURCHASE_REQUIRED).is_valid());
}

#[test]
fn m3_registration_policy_is_valid_rejects_undefined() {
    assert!(!MemberRegistrationPolicy(0b0010_0000).is_valid()); // bit 5 undefined
    assert!(!MemberRegistrationPolicy(0xFF).is_valid());
}

#[test]
fn m3_registration_policy_all_valid_matches_defined_bits() {
    assert_eq!(MemberRegistrationPolicy::ALL_VALID, 0b0001_1111);
}

#[test]
fn m3_stats_policy_is_valid_accepts_defined() {
    assert!(MemberStatsPolicy(0).is_valid()); // default
    assert!(MemberStatsPolicy(0b0000_0011).is_valid()); // both flags
    assert!(MemberStatsPolicy(MemberStatsPolicy::INCLUDE_REPURCHASE_DIRECT).is_valid());
}

#[test]
fn m3_stats_policy_is_valid_rejects_undefined() {
    assert!(!MemberStatsPolicy(0b0000_0100).is_valid()); // bit 2 undefined
    assert!(!MemberStatsPolicy(0xFF).is_valid());
}

#[test]
fn m3_stats_policy_all_valid_matches_defined_bits() {
    assert_eq!(MemberStatsPolicy::ALL_VALID, 0b0000_0011);
}

// ============================================================================
// L2: TransferRestrictionMode::try_from_u8
// ============================================================================

#[test]
fn l2_try_from_u8_valid_values() {
    assert_eq!(TransferRestrictionMode::try_from_u8(0), Some(TransferRestrictionMode::None));
    assert_eq!(TransferRestrictionMode::try_from_u8(1), Some(TransferRestrictionMode::Whitelist));
    assert_eq!(TransferRestrictionMode::try_from_u8(2), Some(TransferRestrictionMode::Blacklist));
    assert_eq!(TransferRestrictionMode::try_from_u8(3), Some(TransferRestrictionMode::KycRequired));
    assert_eq!(TransferRestrictionMode::try_from_u8(4), Some(TransferRestrictionMode::MembersOnly));
}

#[test]
fn l2_try_from_u8_invalid_returns_none() {
    assert_eq!(TransferRestrictionMode::try_from_u8(5), None);
    assert_eq!(TransferRestrictionMode::try_from_u8(255), None);
}

// ============================================================================
// EntityStatus helper methods
// ============================================================================

#[test]
fn entity_status_is_active() {
    assert!(EntityStatus::Active.is_active());
    assert!(!EntityStatus::Pending.is_active());
    assert!(!EntityStatus::Suspended.is_active());
    assert!(!EntityStatus::Banned.is_active());
    assert!(!EntityStatus::Closed.is_active());
    assert!(!EntityStatus::PendingClose.is_active());
}

#[test]
fn entity_status_is_terminal() {
    assert!(EntityStatus::Banned.is_terminal());
    assert!(EntityStatus::Closed.is_terminal());
    assert!(!EntityStatus::Active.is_terminal());
    assert!(!EntityStatus::Pending.is_terminal());
    assert!(!EntityStatus::Suspended.is_terminal());
    assert!(!EntityStatus::PendingClose.is_terminal());
}

#[test]
fn entity_status_can_operate() {
    assert!(EntityStatus::Active.can_operate());
    assert!(!EntityStatus::Pending.can_operate());
    assert!(!EntityStatus::Banned.can_operate());
}

#[test]
fn entity_status_is_pending() {
    assert!(EntityStatus::Pending.is_pending());
    assert!(EntityStatus::PendingClose.is_pending());
    assert!(!EntityStatus::Active.is_pending());
    assert!(!EntityStatus::Banned.is_pending());
}

// ============================================================================
// AdminPermission new bits
// ============================================================================

#[test]
fn admin_permission_new_bits_are_valid() {
    assert!(AdminPermission::is_valid(AdminPermission::GOVERNANCE_MANAGE));
    assert!(AdminPermission::is_valid(AdminPermission::ORDER_MANAGE));
    assert!(AdminPermission::is_valid(AdminPermission::COMMISSION_MANAGE));
    assert!(AdminPermission::is_valid(
        AdminPermission::GOVERNANCE_MANAGE | AdminPermission::ORDER_MANAGE | AdminPermission::COMMISSION_MANAGE
    ));
}

#[test]
fn admin_permission_new_bits_values() {
    assert_eq!(AdminPermission::GOVERNANCE_MANAGE, 0b0001_0000_0000);
    assert_eq!(AdminPermission::ORDER_MANAGE,      0b0010_0000_0000);
    assert_eq!(AdminPermission::COMMISSION_MANAGE,  0b0100_0000_0000);
}
