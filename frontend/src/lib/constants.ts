// ============================================================================
// Admin Permission Bitmask (matches pallets/entity/common AdminPermission)
// ============================================================================

export const AdminPermission = {
  SHOP_MANAGE: 0x001,
  MEMBER_MANAGE: 0x002,
  TOKEN_MANAGE: 0x004,
  ADS_MANAGE: 0x008,
  REVIEW_MANAGE: 0x010,
  DISCLOSURE_MANAGE: 0x020,
  ENTITY_MANAGE: 0x040,
  KYC_MANAGE: 0x080,
  GOVERNANCE_MANAGE: 0x100,
  ORDER_MANAGE: 0x200,
  COMMISSION_MANAGE: 0x400,
  ALL: 0xffffffff,
} as const;

export const PERMISSION_LABELS: Record<number, string> = {
  [AdminPermission.SHOP_MANAGE]: "Shop",
  [AdminPermission.MEMBER_MANAGE]: "Member",
  [AdminPermission.TOKEN_MANAGE]: "Token",
  [AdminPermission.ADS_MANAGE]: "Ads",
  [AdminPermission.REVIEW_MANAGE]: "Review",
  [AdminPermission.DISCLOSURE_MANAGE]: "Disclosure",
  [AdminPermission.ENTITY_MANAGE]: "Entity",
  [AdminPermission.KYC_MANAGE]: "KYC",
  [AdminPermission.GOVERNANCE_MANAGE]: "Governance",
  [AdminPermission.ORDER_MANAGE]: "Order",
  [AdminPermission.COMMISSION_MANAGE]: "Commission",
};

export const ALL_PERMISSIONS = Object.values(AdminPermission).filter(
  (v) => typeof v === "number" && v !== AdminPermission.ALL
) as number[];

// ============================================================================
// Member Registration Policy (bitmask, composable)
// ============================================================================

export const MemberRegistrationPolicy = {
  OPEN: 0,
  PURCHASE_REQUIRED: 0b00001,
  REFERRAL_REQUIRED: 0b00010,
  APPROVAL_REQUIRED: 0b00100,
  KYC_REQUIRED: 0b01000,
  KYC_UPGRADE_REQUIRED: 0b10000,
} as const;

// ============================================================================
// Entity Types
// ============================================================================

export const ENTITY_TYPES = [
  "Merchant",
  "Enterprise",
  "DAO",
  "Community",
  "Project",
  "ServiceProvider",
  "Fund",
] as const;
export type EntityType = (typeof ENTITY_TYPES)[number] | { Custom: number };

export const ENTITY_STATUS = [
  "Pending",
  "Active",
  "Suspended",
  "Banned",
  "Closed",
  "PendingClose",
] as const;
export type EntityStatus = (typeof ENTITY_STATUS)[number];

// ============================================================================
// Governance
// ============================================================================

export const GOVERNANCE_MODES = ["None", "FullDAO"] as const;
export type GovernanceMode = (typeof GOVERNANCE_MODES)[number];

// ============================================================================
// Token Types
// ============================================================================

export const TOKEN_TYPES = [
  "Points",
  "Governance",
  "Equity",
  "Membership",
  "Share",
  "Bond",
] as const;
export type TokenType = (typeof TOKEN_TYPES)[number] | { Hybrid: number };

export const TRANSFER_RESTRICTION_MODES = [
  "None",
  "Whitelist",
  "Blacklist",
  "KycRequired",
  "MembersOnly",
] as const;
export type TransferRestrictionMode =
  (typeof TRANSFER_RESTRICTION_MODES)[number];

// ============================================================================
// Shop
// ============================================================================

export const SHOP_TYPES = [
  "OnlineStore",
  "PhysicalStore",
  "ServicePoint",
  "Warehouse",
  "Franchise",
  "Popup",
  "Virtual",
] as const;
export type ShopType = (typeof SHOP_TYPES)[number];

export const SHOP_OPERATING_STATUS = [
  "Active",
  "Paused",
  "FundDepleted",
  "Closed",
  "Closing",
  "Banned",
] as const;
export type ShopOperatingStatus = (typeof SHOP_OPERATING_STATUS)[number];

// ============================================================================
// Product
// ============================================================================

export const PRODUCT_CATEGORIES = [
  "Digital",
  "Physical",
  "Service",
  "Subscription",
  "Bundle",
  "Other",
] as const;
export type ProductCategory = (typeof PRODUCT_CATEGORIES)[number];

export const PRODUCT_STATUS = [
  "Draft",
  "OnSale",
  "SoldOut",
  "OffShelf",
] as const;
export type ProductStatus = (typeof PRODUCT_STATUS)[number];

// ============================================================================
// Order
// ============================================================================

export const ORDER_STATUS = [
  "Created",
  "Paid",
  "Shipped",
  "Completed",
  "Cancelled",
  "Disputed",
  "Refunded",
  "Expired",
] as const;
export type OrderStatus = (typeof ORDER_STATUS)[number];

export const PAYMENT_ASSETS = ["Native", "EntityToken"] as const;
export type PaymentAsset = (typeof PAYMENT_ASSETS)[number];

// ============================================================================
// Member
// ============================================================================

export const MEMBER_STATUS = [
  "Active",
  "Pending",
  "Frozen",
  "Banned",
  "Expired",
] as const;
export type MemberStatus = (typeof MEMBER_STATUS)[number];

// ============================================================================
// KYC
// ============================================================================

export const KYC_LEVELS = [
  "None",
  "Basic",
  "Standard",
  "Enhanced",
  "Full",
] as const;
export type KycLevel = (typeof KYC_LEVELS)[number];

export const KYC_STATUS = [
  "NotSubmitted",
  "Pending",
  "Approved",
  "Rejected",
  "Expired",
  "Revoked",
] as const;
export type KycStatus = (typeof KYC_STATUS)[number];

// ============================================================================
// Governance Proposals
// ============================================================================

export const PROPOSAL_STATUS = [
  "Voting",
  "Passed",
  "Failed",
  "Executed",
  "Cancelled",
  "Expired",
] as const;
export type ProposalStatus = (typeof PROPOSAL_STATUS)[number];

export const VOTE_TYPES = ["Yes", "No", "Abstain"] as const;
export type VoteType = (typeof VOTE_TYPES)[number];

export const PROPOSAL_TYPE_CATEGORIES = {
  "Product & Shop": [
    "PriceChange", "ProductListing", "ProductDelisting", "InventoryAdjustment",
    "Promotion", "ShopNameChange", "ShopDescriptionChange", "ShopPause", "ShopResume",
  ],
  "Token": [
    "TokenConfigChange", "TokenMint", "TokenBurn", "AirdropDistribution", "Dividend",
  ],
  "Treasury & Finance": [
    "TreasurySpend", "FeeAdjustment", "RevenueShare", "RefundPolicy",
  ],
  "Governance Rules": [
    "VotingPeriodChange", "QuorumChange", "ProposalThresholdChange",
    "ExecutionDelayChange", "PassThresholdChange", "AdminVetoToggle",
  ],
  "Commission": [
    "CommissionModesChange", "DirectRewardChange", "MultiLevelChange",
    "LevelDiffChange", "FixedAmountChange", "FirstOrderChange",
    "RepeatPurchaseChange", "SingleLineChange", "WithdrawalConfigChange",
    "MinRepurchaseRateChange",
  ],
  "Member Levels": [
    "AddCustomLevel", "UpdateCustomLevel", "RemoveCustomLevel",
    "SetUpgradeMode", "EnableCustomLevels", "AddUpgradeRule", "RemoveUpgradeRule",
  ],
  "Team Performance": [
    "TeamPerformanceChange", "TeamPerformancePause", "TeamPerformanceResume",
  ],
  "Compliance": [
    "DisclosureLevelChange", "DisclosureResetViolations",
  ],
  "Community": [
    "CommunityEvent", "RuleSuggestion", "General",
  ],
} as const;

// ============================================================================
// Disclosure
// ============================================================================

export const DISCLOSURE_LEVELS = [
  "Basic",
  "Standard",
  "Enhanced",
  "Full",
] as const;
export type DisclosureLevel = (typeof DISCLOSURE_LEVELS)[number];

export const DISCLOSURE_TYPES = [
  "AnnualReport", "QuarterlyReport", "MonthlyReport", "MaterialEvent",
  "RelatedPartyTransaction", "OwnershipChange", "ManagementChange",
  "BusinessChange", "RiskWarning", "DividendAnnouncement",
  "TokenIssuance", "Buyback", "Other",
] as const;
export type DisclosureType = (typeof DISCLOSURE_TYPES)[number];

export const DISCLOSURE_STATUS = [
  "Draft", "Published", "Withdrawn", "Corrected",
] as const;

export const ANNOUNCEMENT_CATEGORIES = [
  "General", "Promotion", "SystemUpdate", "Event",
  "Policy", "Partnership", "Product", "Other",
] as const;
export type AnnouncementCategory = (typeof ANNOUNCEMENT_CATEGORIES)[number];

export const ANNOUNCEMENT_STATUS = [
  "Active", "Withdrawn", "Expired",
] as const;

export const INSIDER_ROLES = [
  "Owner", "Admin", "Auditor", "Advisor", "MajorHolder",
] as const;
export type InsiderRole = (typeof INSIDER_ROLES)[number];

export const VIOLATION_TYPES = [
  "LateDisclosure", "BlackoutTrading", "UndisclosedMaterialEvent",
] as const;

// ============================================================================
// KYC (extended)
// ============================================================================

export const REJECTION_REASONS = [
  "UnclearDocument", "ExpiredDocument", "InformationMismatch",
  "SuspiciousActivity", "SanctionedEntity", "HighRiskCountry",
  "ForgedDocument", "Other",
] as const;
export type RejectionReason = (typeof REJECTION_REASONS)[number];

export const PROVIDER_TYPES = [
  "Internal", "ThirdParty", "Government", "Financial",
] as const;
export type ProviderType = (typeof PROVIDER_TYPES)[number];

// ============================================================================
// Token Sale
// ============================================================================

export const SALE_MODES = [
  "FixedPrice", "DutchAuction", "WhitelistAllocation", "FCFS", "Lottery",
] as const;
export type SaleMode = (typeof SALE_MODES)[number];

export const ROUND_STATUS = [
  "NotStarted", "Active", "Ended", "Cancelled", "Completed", "Paused",
] as const;
export type RoundStatus = (typeof ROUND_STATUS)[number];

export const VESTING_TYPES = [
  "None", "Linear", "Cliff", "Custom",
] as const;
export type VestingType = (typeof VESTING_TYPES)[number];

// ============================================================================
// Fund Health
// ============================================================================

export const FUND_HEALTH = [
  "Healthy",
  "Warning",
  "Critical",
  "Depleted",
] as const;
export type FundHealth = (typeof FUND_HEALTH)[number];

// ============================================================================
// NEX P2P Market (pallet-nex-market)
// ============================================================================

export const NEX_ORDER_SIDE = ["Sell", "Buy"] as const;
export type NexOrderSide = (typeof NEX_ORDER_SIDE)[number];

export const NEX_ORDER_STATUS = [
  "Open",
  "PartiallyFilled",
  "Filled",
  "Cancelled",
  "Expired",
] as const;
export type NexOrderStatus = (typeof NEX_ORDER_STATUS)[number];

export const USDT_TRADE_STATUS = [
  "AwaitingPayment",
  "AwaitingVerification",
  "Completed",
  "Disputed",
  "Cancelled",
  "Refunded",
  "UnderpaidPending",
] as const;
export type UsdtTradeStatus = (typeof USDT_TRADE_STATUS)[number];

export const DISPUTE_RESOLUTION = [
  "ReleaseToBuyer",
  "RefundToSeller",
] as const;
export type DisputeResolution = (typeof DISPUTE_RESOLUTION)[number];

// ============================================================================
// IPFS Storage (pallet-storage-service)
// ============================================================================

export const PIN_TIERS = ["Critical", "Standard", "Temporary"] as const;
export type PinTier = (typeof PIN_TIERS)[number];

export const PIN_STATES = [
  "Requested",
  "Pinning",
  "Pinned",
  "Degraded",
  "Failed",
] as const;
export type PinState = (typeof PIN_STATES)[number];

export const OPERATOR_LAYERS = ["Core", "Community", "External"] as const;
export type OperatorLayer = (typeof OPERATOR_LAYERS)[number];

// ============================================================================
// GroupRobot
// ============================================================================

export const PLATFORMS = [
  "Telegram",
  "Discord",
  "Slack",
  "Matrix",
  "Farcaster",
] as const;
export type Platform = (typeof PLATFORMS)[number];

export const BOT_STATUS = ["Active", "Suspended", "Deactivated"] as const;
export type BotStatus = (typeof BOT_STATUS)[number];

export const TEE_TYPES = ["Tdx", "Sgx", "TdxPlusSgx"] as const;
export type TeeType = (typeof TEE_TYPES)[number];

export const SUBSCRIPTION_TIERS = [
  "Free",
  "Basic",
  "Pro",
  "Enterprise",
] as const;
export type SubscriptionTier = (typeof SUBSCRIPTION_TIERS)[number];

export const SUBSCRIPTION_STATUS = [
  "Active",
  "PastDue",
  "Suspended",
  "Cancelled",
  "Paused",
] as const;
export type SubscriptionStatus = (typeof SUBSCRIPTION_STATUS)[number];

// ============================================================================
// Dispute (pallet-dispute-arbitration)
// ============================================================================

export const COMPLAINT_TYPES = [
  "OtcFraud",
  "OtcQuality",
  "OtcDelivery",
  "LivestreamContent",
  "LivestreamPayment",
  "MakerQuality",
  "MakerDescription",
  "NftCopyright",
  "NftAuthenticity",
  "SwapSettlement",
  "MemberReputation",
  "CreditFraud",
  "Other",
] as const;
export type ComplaintType = (typeof COMPLAINT_TYPES)[number];

export const COMPLAINT_STATUS = [
  "Submitted",
  "Responded",
  "Mediating",
  "Arbitrating",
  "ResolvedComplainantWin",
  "ResolvedRespondentWin",
  "ResolvedSettlement",
  "Withdrawn",
  "Expired",
] as const;
export type ComplaintStatus = (typeof COMPLAINT_STATUS)[number];

// ============================================================================
// Ads (pallet-ads-core)
// ============================================================================

export const CAMPAIGN_STATUS = [
  "Active",
  "Paused",
  "Exhausted",
  "Expired",
  "Cancelled",
  "Suspended",
  "UnderReview",
] as const;
export type CampaignStatus = (typeof CAMPAIGN_STATUS)[number];

export const CAMPAIGN_TYPES = ["Cpm", "Cpc", "Fixed", "Private"] as const;
export type CampaignType = (typeof CAMPAIGN_TYPES)[number];

// ============================================================================
// Status Colors (shared across UI)
// ============================================================================

export const STATUS_COLORS: Record<string, string> = {
  Active: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  Pending: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400",
  Suspended: "bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-400",
  Banned: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  Closed: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  PendingClose: "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400",
  Healthy: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  Warning: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400",
  Critical: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  Depleted: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  Passed: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  Failed: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  Executed: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  Cancelled: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  Expired: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  Open: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  PartiallyFilled: "bg-cyan-100 text-cyan-800 dark:bg-cyan-900/30 dark:text-cyan-400",
  Filled: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  AwaitingPayment: "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400",
  AwaitingVerification: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  Completed: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  Disputed: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  Refunded: "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400",
  UnderpaidPending: "bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-400",
  Pinned: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  Degraded: "bg-orange-100 text-orange-800 dark:bg-orange-900/30 dark:text-orange-400",
  Requested: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  Submitted: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  Responded: "bg-cyan-100 text-cyan-800 dark:bg-cyan-900/30 dark:text-cyan-400",
  Mediating: "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400",
  Arbitrating: "bg-indigo-100 text-indigo-800 dark:bg-indigo-900/30 dark:text-indigo-400",
  Paused: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400",
  Exhausted: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  UnderReview: "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400",
  Deactivated: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  Draft: "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400",
  OnSale: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  SoldOut: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  OffShelf: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  Frozen: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  Created: "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  Paid: "bg-cyan-100 text-cyan-800 dark:bg-cyan-900/30 dark:text-cyan-400",
  Shipped: "bg-indigo-100 text-indigo-800 dark:bg-indigo-900/30 dark:text-indigo-400",
  Voting: "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400",
  Published: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  Withdrawn: "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400",
  Corrected: "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400",
  NotStarted: "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400",
  Revoked: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  Rejected: "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  Approved: "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  NotSubmitted: "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400",
};
