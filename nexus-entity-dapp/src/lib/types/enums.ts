// Entity 类型 - 7 种
export enum EntityType {
  Merchant = 'Merchant',
  Enterprise = 'Enterprise',
  DAO = 'DAO',
  Community = 'Community',
  Project = 'Project',
  ServiceProvider = 'ServiceProvider',
  Fund = 'Fund',
}

// Entity 状态 - 6 种
export enum EntityStatus {
  Active = 'Active',
  Suspended = 'Suspended',
  Banned = 'Banned',
  PendingClose = 'PendingClose',
  Closed = 'Closed',
  PendingApproval = 'PendingApproval',
}

// 治理模式
export enum GovernanceMode {
  None = 'None',
  FullDAO = 'FullDAO',
}

// 代币类型 - 7 种
export enum TokenType {
  Points = 'Points',
  Governance = 'Governance',
  Equity = 'Equity',
  Membership = 'Membership',
  Share = 'Share',
  Bond = 'Bond',
  Hybrid = 'Hybrid',
}

// 转账限制模式 - 5 种
export enum TransferRestrictionMode {
  None = 'None',
  Whitelist = 'Whitelist',
  Blacklist = 'Blacklist',
  KycRequired = 'KycRequired',
  MembersOnly = 'MembersOnly',
}

// Shop 类型 - 7 种
export enum ShopType {
  OnlineStore = 'OnlineStore',
  PhysicalStore = 'PhysicalStore',
  ServicePoint = 'ServicePoint',
  Warehouse = 'Warehouse',
  Franchise = 'Franchise',
  Popup = 'Popup',
  Virtual = 'Virtual',
}

// Shop 有效状态 - 8 种
export enum EffectiveShopStatus {
  Active = 'Active',
  PausedBySelf = 'PausedBySelf',
  PausedByEntity = 'PausedByEntity',
  FundDepleted = 'FundDepleted',
  Closed = 'Closed',
  ClosedByEntity = 'ClosedByEntity',
  Closing = 'Closing',
  Banned = 'Banned',
}

// Shop 运营状态 (用于计算 EffectiveShopStatus)
export enum ShopOperatingStatus {
  Active = 'Active',
  Paused = 'Paused',
  Closing = 'Closing',
  Closed = 'Closed',
}

// 商品类别 - 6 种
export enum ProductCategory {
  Digital = 'Digital',
  Physical = 'Physical',
  Service = 'Service',
  Subscription = 'Subscription',
  Bundle = 'Bundle',
  Other = 'Other',
}

// 商品状态
export enum ProductStatus {
  Draft = 'Draft',
  OnSale = 'OnSale',
  SoldOut = 'SoldOut',
  OffShelf = 'OffShelf',
}

// 商品可见性
export enum ProductVisibility {
  Public = 'Public',
  MembersOnly = 'MembersOnly',
  LevelGated = 'LevelGated',
}

// 订单状态 - 12 种
export enum OrderStatus {
  Created = 'Created',
  Paid = 'Paid',
  Shipped = 'Shipped',
  ServiceStarted = 'ServiceStarted',
  ServiceCompleted = 'ServiceCompleted',
  Confirmed = 'Confirmed',
  Completed = 'Completed',
  RefundRequested = 'RefundRequested',
  Refunded = 'Refunded',
  Disputed = 'Disputed',
  Cancelled = 'Cancelled',
  Expired = 'Expired',
}

// 支付资产
export enum PaymentAsset {
  Native = 'Native',
  EntityToken = 'EntityToken',
}

// 会员状态 - 5 种
export enum MemberStatus {
  Active = 'Active',
  Pending = 'Pending',
  Frozen = 'Frozen',
  Banned = 'Banned',
  Expired = 'Expired',
}

// 注册策略位标记
export enum RegistrationPolicy {
  OPEN = 0b00000,
  PURCHASE_REQUIRED = 0b00001,
  REFERRAL_REQUIRED = 0b00010,
  APPROVAL_REQUIRED = 0b00100,
  KYC_REQUIRED = 0b01000,
  KYC_UPGRADE_REQUIRED = 0b10000,
}

// KYC 等级 - 5 级
export enum KycLevel {
  None = 0,
  Basic = 1,
  Standard = 2,
  Enhanced = 3,
  Full = 4,
}

// KYC 状态
export enum KycStatus {
  NotSubmitted = 'NotSubmitted',
  Pending = 'Pending',
  Approved = 'Approved',
  Rejected = 'Rejected',
  Expired = 'Expired',
  Revoked = 'Revoked',
}

// 披露等级 - 4 级
export enum DisclosureLevel {
  Basic = 'Basic',
  Standard = 'Standard',
  Enhanced = 'Enhanced',
  Full = 'Full',
}

// 披露状态
export enum DisclosureStatus {
  Draft = 'Draft',
  Published = 'Published',
  Withdrawn = 'Withdrawn',
  Corrected = 'Corrected',
}

// 内幕人员角色 - 5 种
export enum InsiderRole {
  Owner = 'Owner',
  Admin = 'Admin',
  Auditor = 'Auditor',
  Advisor = 'Advisor',
  MajorHolder = 'MajorHolder',
}

// 佣金插件模式 - 6 种
export enum CommissionPlugin {
  Referral = 'Referral',
  MultiLevel = 'MultiLevel',
  LevelDiff = 'LevelDiff',
  SingleLine = 'SingleLine',
  Team = 'Team',
  PoolReward = 'PoolReward',
}

// 升级触发规则 - 9 种
export enum UpgradeTrigger {
  PurchaseProduct = 'PurchaseProduct',
  SingleOrder = 'SingleOrder',
  TotalSpent = 'TotalSpent',
  OrderCount = 'OrderCount',
  TotalSpentUsdt = 'TotalSpentUsdt',
  SingleOrderUsdt = 'SingleOrderUsdt',
  ReferralCount = 'ReferralCount',
  TeamSize = 'TeamSize',
  ReferralLevelCount = 'ReferralLevelCount',
}

// 提案类型分类 - 8 种
export enum ProposalCategory {
  EntityManagement = 'EntityManagement',
  ShopManagement = 'ShopManagement',
  TokenManagement = 'TokenManagement',
  MarketManagement = 'MarketManagement',
  MemberManagement = 'MemberManagement',
  CommissionManagement = 'CommissionManagement',
  DisclosureManagement = 'DisclosureManagement',
  GovernanceManagement = 'GovernanceManagement',
}

// 投票选项
export enum VoteOption {
  Approve = 'Approve',
  Reject = 'Reject',
  Abstain = 'Abstain',
}

// 交易状态
export type TxStatus = 'idle' | 'signing' | 'broadcasting' | 'inBlock' | 'finalized' | 'error';

// 渲染模式 (前端专用)
export type RenderMode = 'normal' | 'readonly' | 'restricted' | 'not_found';

// 发售轮次状态
export enum SaleRoundStatus {
  Created = 'Created',
  Started = 'Started',
  Subscribing = 'Subscribing',
  Ended = 'Ended',
  Claiming = 'Claiming',
}

// 会员管理操作
export enum MemberAction {
  Freeze = 'Freeze',
  Unfreeze = 'Unfreeze',
  Ban = 'Ban',
  Unban = 'Unban',
  Remove = 'Remove',
}
