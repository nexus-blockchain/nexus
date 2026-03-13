use super::*;

// ============================================================================
// EntityType tests
// ============================================================================

#[test]
#[allow(deprecated)]
fn entity_type_default_governance() {
    assert_eq!(EntityType::Merchant.default_governance(), GovernanceMode::None);
    assert_eq!(EntityType::Enterprise.default_governance(), GovernanceMode::MultiSig);
    assert_eq!(EntityType::DAO.default_governance(), GovernanceMode::FullDAO);
    assert_eq!(EntityType::Community.default_governance(), GovernanceMode::Council);
    assert_eq!(EntityType::Project.default_governance(), GovernanceMode::FullDAO);
    assert_eq!(EntityType::Fund.default_governance(), GovernanceMode::Council);
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
    assert!(!EntityType::Enterprise.suggests_governance(&GovernanceMode::FullDAO));
    assert!(EntityType::Enterprise.suggests_governance(&GovernanceMode::MultiSig));
    assert!(EntityType::Community.suggests_governance(&GovernanceMode::Council));
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
fn member_policy_default_purchase_and_referral() {
    let policy = MemberRegistrationPolicy::default();
    assert!(!policy.is_open());
    assert!(policy.requires_purchase());
    assert!(policy.requires_referral());
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
    assert!(TokenType::Hybrid.has_voting_power());
    assert!(!TokenType::Points.has_voting_power());
    assert!(!TokenType::Membership.has_voting_power());
}

#[test]
fn token_type_dividend_rights() {
    assert!(TokenType::Equity.has_dividend_rights());
    assert!(TokenType::Share.has_dividend_rights());
    assert!(TokenType::Hybrid.has_dividend_rights());
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
    assert_eq!(TokenType::Hybrid.default_transfer_restriction(), TransferRestrictionMode::None);
}

// ============================================================================
// TransferRestrictionMode tests
// ============================================================================


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
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::PRODUCT_MANAGE != 0);
    assert!(AdminPermission::ALL_DEFINED & AdminPermission::MARKET_MANAGE != 0);
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
fn entity_status_is_active_covers_can_operate() {
    assert!(EntityStatus::Active.is_active());
    assert!(!EntityStatus::Pending.is_active());
    assert!(!EntityStatus::Banned.is_active());
    assert!(!EntityStatus::Suspended.is_active());
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
    assert_eq!(AdminPermission::PRODUCT_MANAGE,     0b1000_0000_0000);
    assert_eq!(AdminPermission::MARKET_MANAGE,      0b0001_0000_0000_0000);
}

#[test]
fn admin_permission_product_market_are_valid() {
    assert!(AdminPermission::is_valid(AdminPermission::PRODUCT_MANAGE));
    assert!(AdminPermission::is_valid(AdminPermission::MARKET_MANAGE));
    assert!(AdminPermission::is_valid(
        AdminPermission::PRODUCT_MANAGE | AdminPermission::MARKET_MANAGE | AdminPermission::SHOP_MANAGE
    ));
}

// ============================================================================
// DisputeStatus tests
// ============================================================================

#[test]
fn dispute_status_default_is_none() {
    assert_eq!(DisputeStatus::default(), DisputeStatus::None);
}

#[test]
fn dispute_status_is_active() {
    assert!(DisputeStatus::Submitted.is_active());
    assert!(DisputeStatus::Responded.is_active());
    assert!(DisputeStatus::Mediating.is_active());
    assert!(DisputeStatus::Arbitrating.is_active());
    assert!(!DisputeStatus::None.is_active());
    assert!(!DisputeStatus::Resolved.is_active());
    assert!(!DisputeStatus::Withdrawn.is_active());
    assert!(!DisputeStatus::Expired.is_active());
}

#[test]
fn dispute_status_is_terminal() {
    assert!(DisputeStatus::Resolved.is_terminal());
    assert!(DisputeStatus::Withdrawn.is_terminal());
    assert!(DisputeStatus::Expired.is_terminal());
    assert!(!DisputeStatus::None.is_terminal());
    assert!(!DisputeStatus::Submitted.is_terminal());
    assert!(!DisputeStatus::Arbitrating.is_terminal());
}

// ============================================================================
// TokenSaleStatus tests
// ============================================================================

#[test]
fn token_sale_status_default_is_not_started() {
    assert_eq!(TokenSaleStatus::default(), TokenSaleStatus::NotStarted);
}

#[test]
fn token_sale_status_is_purchasable() {
    assert!(TokenSaleStatus::Active.is_purchasable());
    assert!(!TokenSaleStatus::NotStarted.is_purchasable());
    assert!(!TokenSaleStatus::Paused.is_purchasable());
    assert!(!TokenSaleStatus::Ended.is_purchasable());
    assert!(!TokenSaleStatus::Cancelled.is_purchasable());
    assert!(!TokenSaleStatus::Completed.is_purchasable());
}


// ============================================================================
// M1: EffectiveShopStatus::compute preserves Banned when Entity non-Active
// ============================================================================

#[test]
fn m1_compute_entity_suspended_shop_banned_shows_banned() {
    // Before fix: Banned + Suspended → PausedByEntity (masked governance ban)
    // After fix:  Banned + Suspended → Banned (governance ban preserved)
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Suspended, &ShopOperatingStatus::Banned),
        EffectiveShopStatus::Banned
    );
}

#[test]
fn m1_compute_entity_pending_shop_banned_shows_banned() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Pending, &ShopOperatingStatus::Banned),
        EffectiveShopStatus::Banned
    );
}

#[test]
fn m1_compute_entity_pending_close_shop_banned_shows_banned() {
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::PendingClose, &ShopOperatingStatus::Banned),
        EffectiveShopStatus::Banned
    );
}

#[test]
fn m1_compute_entity_active_shop_banned_shows_banned() {
    // Entity Active + Shop Banned → Banned (unchanged, was already correct)
    assert_eq!(
        EffectiveShopStatus::compute(&EntityStatus::Active, &ShopOperatingStatus::Banned),
        EffectiveShopStatus::Banned
    );
}

// ============================================================================
// M3: VestingSchedule::releasable_at tests
// ============================================================================

#[test]
fn vesting_before_cliff_returns_zero() {
    let schedule = VestingSchedule {
        total: 1_000_000,
        released: 0,
        start_block: 100,
        cliff_blocks: 50,
        vesting_blocks: 200,
    };
    assert_eq!(schedule.releasable_at(100), 0); // at start
    assert_eq!(schedule.releasable_at(149), 0); // just before cliff end
}

#[test]
fn vesting_at_cliff_end_returns_zero_vested() {
    let schedule = VestingSchedule {
        total: 1_000_000,
        released: 0,
        start_block: 100,
        cliff_blocks: 50,
        vesting_blocks: 200,
    };
    // cliff_end = 150, elapsed = 0 → total * 0 / 200 = 0
    assert_eq!(schedule.releasable_at(150), 0);
}

#[test]
fn vesting_partial_release() {
    let schedule = VestingSchedule {
        total: 1_000_000,
        released: 0,
        start_block: 100,
        cliff_blocks: 50,
        vesting_blocks: 200,
    };
    // cliff_end = 150, at block 250: elapsed = 100 → total * 100 / 200 = 500_000
    assert_eq!(schedule.releasable_at(250), 500_000);
}

#[test]
fn vesting_partial_with_prior_release() {
    let schedule = VestingSchedule {
        total: 1_000_000,
        released: 300_000,
        start_block: 100,
        cliff_blocks: 50,
        vesting_blocks: 200,
    };
    // cliff_end = 150, at block 250: total_vested = 500_000, releasable = 500_000 - 300_000
    assert_eq!(schedule.releasable_at(250), 200_000);
}

#[test]
fn vesting_fully_elapsed() {
    let schedule = VestingSchedule {
        total: 1_000_000,
        released: 0,
        start_block: 100,
        cliff_blocks: 50,
        vesting_blocks: 200,
    };
    // cliff_end = 150, at block 350: elapsed = 200 >= vesting_blocks → total
    assert_eq!(schedule.releasable_at(350), 1_000_000);
    // well past end
    assert_eq!(schedule.releasable_at(999), 1_000_000);
}

#[test]
fn vesting_zero_vesting_blocks_releases_all_at_cliff() {
    let schedule = VestingSchedule {
        total: 1_000_000,
        released: 0,
        start_block: 100,
        cliff_blocks: 50,
        vesting_blocks: 0,
    };
    // vesting_blocks == 0 → all released at cliff end
    assert_eq!(schedule.releasable_at(149), 0);
    assert_eq!(schedule.releasable_at(150), 1_000_000);
}

#[test]
fn vesting_is_fully_released() {
    let not_released = VestingSchedule {
        total: 1_000_000,
        released: 999_999,
        start_block: 0,
        cliff_blocks: 0,
        vesting_blocks: 100,
    };
    assert!(!not_released.is_fully_released());

    let fully_released = VestingSchedule {
        total: 1_000_000,
        released: 1_000_000,
        start_block: 0,
        cliff_blocks: 0,
        vesting_blocks: 100,
    };
    assert!(fully_released.is_fully_released());

    let over_released = VestingSchedule {
        total: 1_000_000,
        released: 1_000_001,
        start_block: 0,
        cliff_blocks: 0,
        vesting_blocks: 100,
    };
    assert!(over_released.is_fully_released());
}

// ============================================================================
// L2: PageResponse tests
// ============================================================================

#[test]
fn page_response_empty() {
    let resp = PageResponse::<u32>::empty();
    assert!(resp.items.is_empty());
    assert_eq!(resp.total, 0);
    assert!(!resp.has_more);
}

#[test]
fn page_response_from_slice_first_page() {
    let items: sp_std::vec::Vec<u32> = (0..10).collect();
    let page = PageRequest::new(0, 3);
    let resp = PageResponse::from_slice(items, &page);
    assert_eq!(resp.items, vec![0, 1, 2]);
    assert_eq!(resp.total, 10);
    assert!(resp.has_more);
}

#[test]
fn page_response_from_slice_last_page() {
    let items: sp_std::vec::Vec<u32> = (0..10).collect();
    let page = PageRequest::new(8, 5);
    let resp = PageResponse::from_slice(items, &page);
    assert_eq!(resp.items, vec![8, 9]);
    assert_eq!(resp.total, 10);
    assert!(!resp.has_more);
}

#[test]
fn page_response_from_slice_offset_beyond_end() {
    let items: sp_std::vec::Vec<u32> = (0..5).collect();
    let page = PageRequest::new(100, 10);
    let resp = PageResponse::from_slice(items, &page);
    assert!(resp.items.is_empty());
    assert_eq!(resp.total, 5);
    assert!(!resp.has_more);
}

#[test]
fn page_response_from_slice_zero_limit() {
    let items: sp_std::vec::Vec<u32> = (0..5).collect();
    let page = PageRequest::new(0, 0);
    let resp = PageResponse::from_slice(items, &page);
    assert!(resp.items.is_empty());
    assert_eq!(resp.total, 5);
    assert!(resp.has_more);
}

#[test]
fn page_request_capped() {
    let page = PageRequest::new(0, 100).capped(20);
    assert_eq!(page.limit, 20);
    let page2 = PageRequest::new(0, 5).capped(20);
    assert_eq!(page2.limit, 5);
}

// ============================================================================
// R2-M3: ShopOperatingStatus new helpers tests
// ============================================================================

#[test]
fn shop_operating_status_is_closed_or_closing() {
    assert!(ShopOperatingStatus::Closed.is_closed_or_closing());
    assert!(ShopOperatingStatus::Closing.is_closed_or_closing());
    assert!(!ShopOperatingStatus::Active.is_closed_or_closing());
    assert!(!ShopOperatingStatus::Paused.is_closed_or_closing());
    assert!(!ShopOperatingStatus::FundDepleted.is_closed_or_closing());
    assert!(!ShopOperatingStatus::Banned.is_closed_or_closing());
}

#[test]
fn shop_operating_status_is_banned() {
    assert!(ShopOperatingStatus::Banned.is_banned());
    assert!(!ShopOperatingStatus::Active.is_banned());
    assert!(!ShopOperatingStatus::Closed.is_banned());
    assert!(!ShopOperatingStatus::Paused.is_banned());
}

#[test]
fn shop_operating_status_is_terminal_or_banned() {
    assert!(ShopOperatingStatus::Closed.is_terminal_or_banned());
    assert!(ShopOperatingStatus::Closing.is_terminal_or_banned());
    assert!(ShopOperatingStatus::Banned.is_terminal_or_banned());
    assert!(!ShopOperatingStatus::Active.is_terminal_or_banned());
    assert!(!ShopOperatingStatus::Paused.is_terminal_or_banned());
    assert!(!ShopOperatingStatus::FundDepleted.is_terminal_or_banned());
}

// ============================================================================
// R2-L1: MemberStatus helper tests
// ============================================================================

#[test]
fn member_status_is_active() {
    assert!(MemberStatus::Active.is_active());
    assert!(!MemberStatus::Pending.is_active());
    assert!(!MemberStatus::Frozen.is_active());
    assert!(!MemberStatus::Banned.is_active());
    assert!(!MemberStatus::Expired.is_active());
}

#[test]
fn member_status_is_restricted() {
    assert!(MemberStatus::Frozen.is_restricted());
    assert!(MemberStatus::Banned.is_restricted());
    assert!(MemberStatus::Expired.is_restricted());
    assert!(!MemberStatus::Active.is_restricted());
    assert!(!MemberStatus::Pending.is_restricted());
}

#[test]
fn member_status_can_reactivate() {
    assert!(MemberStatus::Frozen.can_reactivate());
    assert!(MemberStatus::Expired.can_reactivate());
    assert!(!MemberStatus::Banned.can_reactivate());
    assert!(!MemberStatus::Active.can_reactivate());
    assert!(!MemberStatus::Pending.can_reactivate());
}

#[test]
fn member_status_is_pending() {
    assert!(MemberStatus::Pending.is_pending());
    assert!(!MemberStatus::Active.is_pending());
    assert!(!MemberStatus::Frozen.is_pending());
    assert!(!MemberStatus::Banned.is_pending());
}

// ============================================================================
// R2-M2: MemberRegistrationPolicy KYC flags tests
// ============================================================================

#[test]
fn member_policy_kyc_flags() {
    let policy = MemberRegistrationPolicy(
        MemberRegistrationPolicy::KYC_REQUIRED | MemberRegistrationPolicy::KYC_UPGRADE_REQUIRED,
    );
    assert!(policy.requires_kyc());
    assert!(policy.requires_kyc_for_upgrade());
    assert!(!policy.requires_purchase());
    assert!(!policy.requires_referral());
    assert!(!policy.requires_approval());
    assert!(policy.is_valid());
}

#[test]
fn member_policy_all_five_flags() {
    let policy = MemberRegistrationPolicy(MemberRegistrationPolicy::ALL_VALID);
    assert!(policy.requires_purchase());
    assert!(policy.requires_referral());
    assert!(policy.requires_approval());
    assert!(policy.requires_kyc());
    assert!(policy.requires_kyc_for_upgrade());
    assert!(policy.is_valid());
    assert!(!policy.is_open());
}

// ============================================================================
// R2-L2: DisclosureLevel ordering tests
// ============================================================================

#[test]
fn disclosure_level_ordering() {
    assert!(DisclosureLevel::Basic < DisclosureLevel::Standard);
    assert!(DisclosureLevel::Standard < DisclosureLevel::Enhanced);
    assert!(DisclosureLevel::Enhanced < DisclosureLevel::Full);
    assert_eq!(DisclosureLevel::default(), DisclosureLevel::Basic);
}

// ============================================================================
// R2-L3: NullEntityProvider tests
// ============================================================================

#[test]
fn null_entity_provider_returns_defaults() {
    assert!(!<NullEntityProvider as EntityProvider<u64>>::entity_exists(1));
    assert!(!<NullEntityProvider as EntityProvider<u64>>::is_entity_active(1));
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::entity_status(1), None);
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::entity_owner(1), None);
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::entity_account(1), 0u64);
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::update_entity_stats(1, 100, 1), Ok(()));
    // default trait methods
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::entity_type(1), None);
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::register_shop(1, 1), Ok(()));
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::unregister_shop(1, 1), Ok(()));
    assert!(!<NullEntityProvider as EntityProvider<u64>>::is_entity_admin(1, &1u64, 0xFF));
    assert!(<NullEntityProvider as EntityProvider<u64>>::entity_shops(1).is_empty());
    assert!(!<NullEntityProvider as EntityProvider<u64>>::is_entity_locked(1));
    assert!(<NullEntityProvider as EntityProvider<u64>>::entity_name(1).is_empty());
    assert_eq!(<NullEntityProvider as EntityProvider<u64>>::entity_metadata_cid(1), None);
    assert!(<NullEntityProvider as EntityProvider<u64>>::entity_description(1).is_empty());
}

// ============================================================================
// R2-L3: NullFeeConfigProvider tests
// ============================================================================

#[test]
fn null_fee_config_provider_returns_defaults() {
    assert_eq!(NullFeeConfigProvider::platform_fee_rate(), 100);
    assert_eq!(NullFeeConfigProvider::entity_fee_override(1), None);
    assert_eq!(NullFeeConfigProvider::token_fee_rate(1), 0);
    // effective_fee_rate should fall back to platform_fee_rate when no override
    assert_eq!(NullFeeConfigProvider::effective_fee_rate(1), 100);
}

// ============================================================================
// R2-L3: NullVestingProvider tests
// ============================================================================

#[test]
fn null_vesting_provider_returns_defaults() {
    assert_eq!(<NullVestingProvider as VestingProvider<u64>>::vesting_balance(1, &1u64), 0);
    assert_eq!(<NullVestingProvider as VestingProvider<u64>>::releasable_balance(1, &1u64), 0);
    assert_eq!(<NullVestingProvider as VestingProvider<u64>>::release(1, &1u64), Ok(0));
    assert_eq!(<NullVestingProvider as VestingProvider<u64>>::vesting_schedule(1, &1u64), None);
    assert!(!<NullVestingProvider as VestingProvider<u64>>::has_vesting(1, &1u64));
}

// ============================================================================
// R2-L3: NullDividendProvider tests
// ============================================================================

#[test]
fn null_dividend_provider_returns_defaults() {
    assert_eq!(<NullDividendProvider as DividendProvider<u64, u128>>::pending_dividend(1, &1u64), 0);
    assert_eq!(<NullDividendProvider as DividendProvider<u64, u128>>::claim_dividend(1, &1u64), Ok(0));
    assert!(!<NullDividendProvider as DividendProvider<u64, u128>>::is_dividend_active(1));
    assert_eq!(<NullDividendProvider as DividendProvider<u64, u128>>::next_distribution_at(1), None);
    assert_eq!(<NullDividendProvider as DividendProvider<u64, u128>>::total_distributed(1), 0);
}

// ============================================================================
// R2-L3: NullEmergencyProvider tests
// ============================================================================

#[test]
fn null_emergency_provider_returns_defaults() {
    assert!(!NullEmergencyProvider::is_emergency_paused());
    assert!(!NullEmergencyProvider::is_module_paused(0));
    assert!(!NullEmergencyProvider::is_module_paused(255));
    // pause_system / resume_system use trait defaults → Err
    assert!(NullEmergencyProvider::pause_system().is_err());
    assert!(NullEmergencyProvider::resume_system().is_err());
}

// ============================================================================
// R2-L3: NullPricingProvider is_price_stale test
// ============================================================================

#[test]
fn null_pricing_provider_is_not_stale() {
    assert!(!NullPricingProvider::is_price_stale());
}

// ============================================================================
// R2-L3: EntityTokenPriceProvider for () tests
// ============================================================================

#[test]
fn unit_entity_token_price_provider_returns_defaults() {
    assert_eq!(<() as EntityTokenPriceProvider>::get_token_price(1), None);
    assert_eq!(<() as EntityTokenPriceProvider>::get_token_price_usdt(1), None);
    assert_eq!(<() as EntityTokenPriceProvider>::token_price_confidence(1), 0);
    assert!(<() as EntityTokenPriceProvider>::is_token_price_stale(1, 100));
    assert!(!<() as EntityTokenPriceProvider>::is_token_price_reliable(1));
}

// ============================================================================
// R2-L3: NullDisclosureProvider tests
// ============================================================================

#[test]
fn null_disclosure_provider_returns_defaults() {
    assert!(!<NullDisclosureProvider as DisclosureProvider<u64>>::is_in_blackout(1));
    assert!(!<NullDisclosureProvider as DisclosureProvider<u64>>::is_insider(1, &1u64));
    assert!(<NullDisclosureProvider as DisclosureProvider<u64>>::can_insider_trade(1, &1u64));
    assert_eq!(
        <NullDisclosureProvider as DisclosureProvider<u64>>::get_disclosure_level(1),
        DisclosureLevel::Basic
    );
    assert!(!<NullDisclosureProvider as DisclosureProvider<u64>>::is_disclosure_overdue(1));
    assert_eq!(<NullDisclosureProvider as DisclosureProvider<u64>>::get_violation_count(1), 0);
    assert_eq!(<NullDisclosureProvider as DisclosureProvider<u64>>::get_insider_role(1, &1u64), None);
    assert!(!<NullDisclosureProvider as DisclosureProvider<u64>>::is_disclosure_configured(1));
    assert!(!<NullDisclosureProvider as DisclosureProvider<u64>>::is_high_risk(1));
}

// ============================================================================
// R2-L3: NullProductProvider tests
// ============================================================================

#[test]
fn null_product_provider_returns_defaults() {
    assert!(!<NullProductProvider as ProductProvider<u64, u128>>::product_exists(1));
    assert!(!<NullProductProvider as ProductProvider<u64, u128>>::is_product_on_sale(1));
    assert_eq!(<NullProductProvider as ProductProvider<u64, u128>>::product_shop_id(1), None);
    assert_eq!(<NullProductProvider as ProductProvider<u64, u128>>::product_price(1), None);
    assert_eq!(<NullProductProvider as ProductProvider<u64, u128>>::product_stock(1), None);
    assert_eq!(<NullProductProvider as ProductProvider<u64, u128>>::product_category(1), None);
    // trait default methods
    assert_eq!(<NullProductProvider as ProductProvider<u64, u128>>::product_status(1), None);
    assert_eq!(<NullProductProvider as ProductProvider<u64, u128>>::product_owner(1), None);
    assert!(<NullProductProvider as ProductProvider<u64, u128>>::shop_product_ids(1).is_empty());
}

// ============================================================================
// #1 NullOrderProvider 扩展方法测试
// ============================================================================

#[test]
fn null_order_provider_extended_methods() {
    assert_eq!(<NullOrderProvider as OrderProvider<u64, u128>>::order_status(1), None::<OrderStatus>);
    assert_eq!(<NullOrderProvider as OrderProvider<u64, u128>>::order_entity_id(1), None::<u64>);
    assert_eq!(<NullOrderProvider as OrderProvider<u64, u128>>::order_product_id(1), None::<u64>);
    assert_eq!(<NullOrderProvider as OrderProvider<u64, u128>>::order_quantity(1), None::<u32>);
}

// ============================================================================
// #2 NullDisputeQueryProvider 测试
// ============================================================================

#[test]
fn null_dispute_query_provider_returns_defaults() {
    assert_eq!(
        <NullDisputeQueryProvider as DisputeQueryProvider<u64>>::order_dispute_status(1),
        DisputeStatus::None
    );
    assert_eq!(
        <NullDisputeQueryProvider as DisputeQueryProvider<u64>>::dispute_resolution(1),
        None
    );
    assert_eq!(
        <NullDisputeQueryProvider as DisputeQueryProvider<u64>>::active_dispute_count(0, &1u64),
        0
    );
    assert!(!<NullDisputeQueryProvider as DisputeQueryProvider<u64>>::has_active_dispute(1));
    assert_eq!(
        <NullDisputeQueryProvider as DisputeQueryProvider<u64>>::dispute_id_by_order(1),
        None
    );
    assert_eq!(
        <NullDisputeQueryProvider as DisputeQueryProvider<u64>>::dispute_amount(1),
        None
    );
}

// ============================================================================
// #7 NullShopProvider ban/unban 测试
// ============================================================================

#[test]
fn null_shop_provider_ban_unban() {
    assert_eq!(<NullShopProvider as ShopProvider<u64>>::ban_shop(1), Ok(()));
    assert_eq!(<NullShopProvider as ShopProvider<u64>>::unban_shop(1), Ok(()));
}

// ============================================================================
// #8 NullTokenSaleProvider 测试
// ============================================================================

#[test]
fn null_token_sale_provider_returns_defaults() {
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::active_sale_round(1), None);
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::sale_round_status(1), None);
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::sold_amount(1), None);
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::remaining_amount(1), None);
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::participants_count(1), None);
    assert!(!<NullTokenSaleProvider as TokenSaleProvider<u128>>::has_active_sale(1));
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::sale_total_supply(1), None);
    assert_eq!(<NullTokenSaleProvider as TokenSaleProvider<u128>>::sale_entity_id(1), None);
}

// ============================================================================
// #9 NullKycProvider 扩展方法测试
// ============================================================================

#[test]
fn null_kyc_provider_extended_methods() {
    assert!(!NullKycProvider::is_kyc_expired(1, &1u64));
    assert!(!NullKycProvider::can_participate(1, &1u64));
    assert_eq!(NullKycProvider::kyc_expires_at(1, &1u64), 0);
}

// ============================================================================
// #10 NullGovernanceProvider 扩展方法测试
// ============================================================================

#[test]
fn null_governance_provider_extended_methods() {
    assert_eq!(NullGovernanceProvider::active_proposal_count(1), 0);
    assert!(!NullGovernanceProvider::is_governance_initialized(1));
    assert_eq!(NullGovernanceProvider::execution_delay(1), 0);
    assert_eq!(NullGovernanceProvider::pass_threshold(1), 0);
}

// ============================================================================
// #5/#6 NullMemberProvider 扩展方法测试
// ============================================================================

#[test]
fn null_member_provider_introduced_by() {
    assert_eq!(
        <NullMemberProvider as MemberProvider<u64>>::get_introduced_by(1, &1u64),
        None
    );
}

#[test]
fn null_member_provider_ban_unban_remove() {
    assert_eq!(
        <NullMemberProvider as MemberProvider<u64>>::ban_member(1, &1u64),
        Ok(())
    );
    assert_eq!(
        <NullMemberProvider as MemberProvider<u64>>::unban_member(1, &1u64),
        Ok(())
    );
    assert_eq!(
        <NullMemberProvider as MemberProvider<u64>>::remove_member(1, &1u64),
        Ok(())
    );
}

// ============================================================================
// #11 NullEntityTokenProvider 扩展方法测试
// ============================================================================

#[test]
fn null_entity_token_provider_metadata() {
    assert!(
        <NullEntityTokenProvider as EntityTokenProvider<u64, u128>>::token_name(1).is_empty()
    );
    assert!(
        <NullEntityTokenProvider as EntityTokenProvider<u64, u128>>::token_symbol(1).is_empty()
    );
    assert_eq!(
        <NullEntityTokenProvider as EntityTokenProvider<u64, u128>>::token_decimals(1),
        0
    );
    assert!(
        !<NullEntityTokenProvider as EntityTokenProvider<u64, u128>>::is_token_transferable(1)
    );
    assert_eq!(
        <NullEntityTokenProvider as EntityTokenProvider<u64, u128>>::token_holder_count(1),
        0
    );
}

// ============================================================================
// v0.9.0: GovernanceMode new variants
// ============================================================================

#[test]
fn governance_mode_new_variants_exist() {
    let ms = GovernanceMode::MultiSig;
    let co = GovernanceMode::Council;
    assert_ne!(ms, GovernanceMode::None);
    assert_ne!(co, GovernanceMode::FullDAO);
    assert_ne!(ms, co);
}

#[test]
fn governance_mode_default_is_none() {
    assert_eq!(GovernanceMode::default(), GovernanceMode::None);
}

// ============================================================================
// v0.9.0: OrderStatus new variants
// ============================================================================

#[test]
fn order_status_new_variants() {
    let p = OrderStatus::Processing;
    let ac = OrderStatus::AwaitingConfirmation;
    let pr = OrderStatus::PartiallyRefunded;
    assert_ne!(p, OrderStatus::Paid);
    assert_ne!(ac, OrderStatus::Shipped);
    assert_ne!(pr, OrderStatus::Refunded);
}

// ============================================================================
// v0.9.0: DisputeResolution::PartialSettlement
// ============================================================================

#[test]
fn dispute_resolution_partial_settlement() {
    let ps = DisputeResolution::PartialSettlement { complainant_share_bps: 6000 };
    assert_ne!(ps, DisputeResolution::Settlement);
    if let DisputeResolution::PartialSettlement { complainant_share_bps } = ps {
        assert_eq!(complainant_share_bps, 6000);
    } else {
        panic!("expected PartialSettlement");
    }
}

#[test]
fn dispute_resolution_partial_settlement_full_range() {
    let zero = DisputeResolution::PartialSettlement { complainant_share_bps: 0 };
    let full = DisputeResolution::PartialSettlement { complainant_share_bps: 10000 };
    assert_ne!(zero, full);
}

// ============================================================================
// v0.9.0: DisputeResolution validation methods
// ============================================================================

#[test]
fn dispute_resolution_is_valid() {
    assert!(DisputeResolution::ComplainantWin.is_valid());
    assert!(DisputeResolution::RespondentWin.is_valid());
    assert!(DisputeResolution::Settlement.is_valid());
    assert!(DisputeResolution::PartialSettlement { complainant_share_bps: 0 }.is_valid());
    assert!(DisputeResolution::PartialSettlement { complainant_share_bps: 5000 }.is_valid());
    assert!(DisputeResolution::PartialSettlement { complainant_share_bps: 10000 }.is_valid());
    assert!(!DisputeResolution::PartialSettlement { complainant_share_bps: 10001 }.is_valid());
    assert!(!DisputeResolution::PartialSettlement { complainant_share_bps: u16::MAX }.is_valid());
}

#[test]
fn dispute_resolution_complainant_share_bps() {
    assert_eq!(DisputeResolution::ComplainantWin.complainant_share_bps(), 10000);
    assert_eq!(DisputeResolution::RespondentWin.complainant_share_bps(), 0);
    assert_eq!(DisputeResolution::Settlement.complainant_share_bps(), 5000);
    assert_eq!(
        DisputeResolution::PartialSettlement { complainant_share_bps: 7500 }.complainant_share_bps(),
        7500
    );
}

// ============================================================================
// v0.9.0: PriceReliability
// ============================================================================

#[test]
fn price_reliability_enum_values() {
    assert_ne!(PriceReliability::Reliable, PriceReliability::Low);
    assert_ne!(PriceReliability::Low, PriceReliability::Unavailable);
    assert_ne!(PriceReliability::Reliable, PriceReliability::Unavailable);
}

// ============================================================================
// v0.9.0: DividendState
// ============================================================================

#[test]
fn dividend_state_default() {
    let state = DividendState::<u128, u64>::default();
    assert_eq!(state.last_distribution, 0);
    assert_eq!(state.accumulated, 0);
    assert_eq!(state.total_distributed, 0);
    assert_eq!(state.round_count, 0);
}

// ============================================================================
// v0.9.0: NullReviewProvider
// ============================================================================

#[test]
fn null_review_provider_returns_defaults() {
    assert_eq!(<NullReviewProvider as ReviewProvider<u64>>::shop_average_rating(1), 0);
    assert_eq!(<NullReviewProvider as ReviewProvider<u64>>::shop_review_count(1), 0);
    assert_eq!(<NullReviewProvider as ReviewProvider<u64>>::product_average_rating(1), 0);
    assert_eq!(<NullReviewProvider as ReviewProvider<u64>>::product_review_count(1), 0);
    assert!(!<NullReviewProvider as ReviewProvider<u64>>::has_reviewed_order(1, &1u64));
    assert!(<NullReviewProvider as ReviewProvider<u64>>::is_review_enabled(1));
    assert_eq!(<NullReviewProvider as ReviewProvider<u64>>::user_review_count(1, &1u64), 0);
}

// ============================================================================
// v0.9.0: NullMarketProvider
// ============================================================================

#[test]
fn null_market_provider_returns_defaults() {
    assert!(!<NullMarketProvider as MarketProvider<u64, u128>>::has_active_market(1));
    assert_eq!(<NullMarketProvider as MarketProvider<u64, u128>>::trading_volume_24h(1), 0);
    assert_eq!(<NullMarketProvider as MarketProvider<u64, u128>>::best_bid(1), None);
    assert_eq!(<NullMarketProvider as MarketProvider<u64, u128>>::best_ask(1), None);
    assert_eq!(<NullMarketProvider as MarketProvider<u64, u128>>::user_active_order_count(1, &1u64), 0);
    assert!(!<NullMarketProvider as MarketProvider<u64, u128>>::is_market_paused(1));
}

// ============================================================================
// v0.9.0: NullDisclosureReadProvider / NullDisclosureWriteProvider
// ============================================================================

#[test]
fn null_disclosure_read_provider_returns_defaults() {
    assert!(!<NullDisclosureReadProvider as DisclosureReadProvider<u64>>::is_in_blackout(1));
    assert!(!<NullDisclosureReadProvider as DisclosureReadProvider<u64>>::is_insider(1, &1u64));
    assert!(<NullDisclosureReadProvider as DisclosureReadProvider<u64>>::can_insider_trade(1, &1u64));
    assert_eq!(
        <NullDisclosureReadProvider as DisclosureReadProvider<u64>>::get_disclosure_level(1),
        DisclosureLevel::Basic
    );
    assert!(!<NullDisclosureReadProvider as DisclosureReadProvider<u64>>::is_disclosure_overdue(1));
}

#[test]
fn null_disclosure_write_provider_defaults() {
    assert!(
        <NullDisclosureWriteProvider as DisclosureWriteProvider<u64>>::governance_configure_disclosure(1, DisclosureLevel::Standard, true, 100).is_err()
    );
    assert!(
        <NullDisclosureWriteProvider as DisclosureWriteProvider<u64>>::governance_reset_violations(1).is_err()
    );
    assert_eq!(
        <NullDisclosureWriteProvider as DisclosureWriteProvider<u64>>::register_major_holder(1, &1u64),
        Ok(())
    );
    assert_eq!(
        <NullDisclosureWriteProvider as DisclosureWriteProvider<u64>>::governance_set_penalty_level(1, 2),
        Ok(())
    );
}

// ============================================================================
// v0.9.0: DisclosureProvider → DisclosureReadProvider blanket impl
// ============================================================================

#[test]
fn disclosure_provider_bridges_to_read_provider() {
    assert!(!<NullDisclosureProvider as DisclosureReadProvider<u64>>::is_in_blackout(1));
    assert!(!<NullDisclosureProvider as DisclosureReadProvider<u64>>::is_insider(1, &1u64));
    assert!(<NullDisclosureProvider as DisclosureReadProvider<u64>>::can_insider_trade(1, &1u64));
    assert_eq!(
        <NullDisclosureProvider as DisclosureReadProvider<u64>>::get_disclosure_level(1),
        DisclosureLevel::Basic
    );
    assert!(!<NullDisclosureProvider as DisclosureReadProvider<u64>>::is_disclosure_overdue(1));
    assert_eq!(<NullDisclosureProvider as DisclosureReadProvider<u64>>::get_violation_count(1), 0);
    assert!(!<NullDisclosureProvider as DisclosureReadProvider<u64>>::is_high_risk(1));
}

#[test]
fn disclosure_provider_bridges_to_write_provider() {
    assert_eq!(
        <NullDisclosureProvider as DisclosureWriteProvider<u64>>::register_major_holder(1, &1u64),
        Ok(())
    );
    assert_eq!(
        <NullDisclosureProvider as DisclosureWriteProvider<u64>>::deregister_major_holder(1, &1u64),
        Ok(())
    );
    assert_eq!(
        <NullDisclosureProvider as DisclosureWriteProvider<u64>>::governance_set_penalty_level(1, 0),
        Ok(())
    );
}

// ============================================================================
// v0.9.0: MemberQueryProvider / MemberWriteProvider split
// ============================================================================

#[test]
fn null_member_query_provider_returns_defaults() {
    assert!(!<NullMemberQueryProvider as MemberQueryProvider<u64>>::is_member(1, &1u64));
    assert_eq!(<NullMemberQueryProvider as MemberQueryProvider<u64>>::get_referrer(1, &1u64), None);
    assert_eq!(<NullMemberQueryProvider as MemberQueryProvider<u64>>::custom_level_id(1, &1u64), 0);
    assert_eq!(<NullMemberQueryProvider as MemberQueryProvider<u64>>::get_level_commission_bonus(1, 0), 0);
    assert!(!<NullMemberQueryProvider as MemberQueryProvider<u64>>::uses_custom_levels(1));
    assert_eq!(<NullMemberQueryProvider as MemberQueryProvider<u64>>::get_member_stats(1, &1u64), (0, 0, 0));
}

#[test]
fn null_member_write_provider_returns_ok() {
    assert_eq!(
        <NullMemberWriteProvider as MemberWriteProvider<u64>>::auto_register(1, &1u64, None),
        Ok(())
    );
}

#[test]
fn member_provider_bridges_to_query_provider() {
    assert!(!<NullMemberProvider as MemberQueryProvider<u64>>::is_member(1, &1u64));
    assert_eq!(<NullMemberProvider as MemberQueryProvider<u64>>::get_referrer(1, &1u64), None);
    assert_eq!(<NullMemberProvider as MemberQueryProvider<u64>>::member_count(1), 0);
    assert!(!<NullMemberProvider as MemberQueryProvider<u64>>::is_banned(1, &1u64));
    assert!(<NullMemberProvider as MemberQueryProvider<u64>>::is_member_active(1, &1u64));
}

#[test]
fn member_provider_bridges_to_write_provider() {
    assert_eq!(
        <NullMemberProvider as MemberWriteProvider<u64>>::auto_register(1, &1u64, None),
        Ok(())
    );
    assert_eq!(
        <NullMemberProvider as MemberWriteProvider<u64>>::update_spent(1, &1u64, 100),
        Ok(())
    );
    assert_eq!(
        <NullMemberProvider as MemberWriteProvider<u64>>::ban_member(1, &1u64),
        Ok(())
    );
}

// ============================================================================
// v0.9.0: EntityProvider ownership transfer defaults
// ============================================================================

#[test]
fn null_entity_provider_ownership_transfer_defaults() {
    assert_eq!(
        <NullEntityProvider as EntityProvider<u64>>::pending_ownership_transfer(1),
        None
    );
    assert!(
        <NullEntityProvider as EntityProvider<u64>>::initiate_ownership_transfer(1, &2u64).is_err()
    );
    assert!(
        <NullEntityProvider as EntityProvider<u64>>::accept_ownership_transfer(1, &2u64).is_err()
    );
    assert!(
        <NullEntityProvider as EntityProvider<u64>>::cancel_ownership_transfer(1).is_err()
    );
}

// ============================================================================
// v0.9.0: CommonError constants smoke test
// ============================================================================

#[test]
fn common_error_constants_are_non_empty() {
    assert!(!CommonError::ENTITY_NOT_FOUND.is_empty());
    assert!(!CommonError::SHOP_NOT_FOUND.is_empty());
    assert!(!CommonError::ORDER_NOT_FOUND.is_empty());
    assert!(!CommonError::INSUFFICIENT_PERMISSION.is_empty());
    assert!(!CommonError::EMERGENCY_PAUSED.is_empty());
    assert!(!CommonError::PRICE_UNAVAILABLE.is_empty());
}

