export const AdminPermission = {
  SHOP_MANAGE: 0x01,
  PRODUCT_MANAGE: 0x02,
  ORDER_MANAGE: 0x04,
  MEMBER_MANAGE: 0x08,
  TOKEN_MANAGE: 0x10,
  GOVERNANCE_MANAGE: 0x20,
  FINANCE_MANAGE: 0x40,
  DISCLOSURE_MANAGE: 0x80,
} as const;

export const PERMISSION_LABELS: Record<number, string> = {
  [AdminPermission.SHOP_MANAGE]: "Shop",
  [AdminPermission.PRODUCT_MANAGE]: "Product",
  [AdminPermission.ORDER_MANAGE]: "Order",
  [AdminPermission.MEMBER_MANAGE]: "Member",
  [AdminPermission.TOKEN_MANAGE]: "Token",
  [AdminPermission.GOVERNANCE_MANAGE]: "Governance",
  [AdminPermission.FINANCE_MANAGE]: "Finance",
  [AdminPermission.DISCLOSURE_MANAGE]: "Disclosure",
};

export const ALL_PERMISSIONS = Object.values(AdminPermission);

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
] as const;

export type EntityStatus = (typeof ENTITY_STATUS)[number];

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

export type TransferRestrictionMode = (typeof TRANSFER_RESTRICTION_MODES)[number];

export const GOVERNANCE_MODES = ["None", "FullDAO"] as const;
export type GovernanceMode = (typeof GOVERNANCE_MODES)[number];

export const SHOP_TYPES = [
  "Online",
  "Physical",
  "Hybrid",
  "Service",
  "Digital",
] as const;

export const KYC_LEVELS = ["None", "Basic", "Standard", "Enhanced"] as const;

export const FUND_HEALTH = ["Healthy", "Warning", "Critical", "Depleted"] as const;
export type FundHealth = (typeof FUND_HEALTH)[number];

export const ORDER_STATUS = [
  "Pending",
  "Paid",
  "Shipped",
  "Delivered",
  "Completed",
  "Cancelled",
  "RefundRequested",
  "Refunded",
  "ServiceStarted",
  "ServiceCompleted",
] as const;

export const PROPOSAL_STATUS = [
  "Active",
  "Passed",
  "Failed",
  "Executed",
  "Cancelled",
  "Expired",
] as const;

export const DISCLOSURE_TYPES = [
  "AnnualReport",
  "QuarterlyReport",
  "MaterialEvent",
  "InsiderTransaction",
  "RegulatoryFiling",
] as const;

export const STATUS_COLORS: Record<string, string> = {
  Active: "bg-green-100 text-green-800",
  Pending: "bg-yellow-100 text-yellow-800",
  Suspended: "bg-orange-100 text-orange-800",
  Banned: "bg-red-100 text-red-800",
  Closed: "bg-gray-100 text-gray-800",
  Healthy: "bg-green-100 text-green-800",
  Warning: "bg-yellow-100 text-yellow-800",
  Critical: "bg-red-100 text-red-800",
  Depleted: "bg-gray-100 text-gray-800",
  Passed: "bg-green-100 text-green-800",
  Failed: "bg-red-100 text-red-800",
  Executed: "bg-blue-100 text-blue-800",
  Cancelled: "bg-gray-100 text-gray-800",
  Expired: "bg-gray-100 text-gray-800",
};
