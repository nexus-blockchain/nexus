import {
  EntityType,
  EntityStatus,
  GovernanceMode,
  TokenType,
  TransferRestrictionMode,
  ShopType,
  EffectiveShopStatus,
  ProductCategory,
  ProductStatus,
  ProductVisibility,
  OrderStatus,
  PaymentAsset,
  MemberStatus,
  KycLevel,
  KycStatus,
  CommissionPlugin,
  TxStatus,
  RenderMode,
} from './enums';

// AdminPermission 常量对象（11 个权限位）
export const AdminPermission = {
  SHOP_MANAGE:       0x001,
  MEMBER_MANAGE:     0x002,
  TOKEN_MANAGE:      0x004,
  ADS_MANAGE:        0x008,
  REVIEW_MANAGE:     0x010,
  DISCLOSURE_MANAGE: 0x020,
  ENTITY_MANAGE:     0x040,
  KYC_MANAGE:        0x080,
  GOVERNANCE_MANAGE: 0x100,
  ORDER_MANAGE:      0x200,
  COMMISSION_MANAGE: 0x400,
  ALL:               0xFFFFFFFF,
} as const;

export type AdminPermissionValue = (typeof AdminPermission)[keyof typeof AdminPermission];

// Entity 基本信息
export interface EntityData {
  id: number;
  owner: string;
  name: string;
  logoCid: string | null;
  descriptionCid: string | null;
  metadataUri: string | null;
  status: EntityStatus;
  entityType: EntityType;
  governanceMode: GovernanceMode;
  verified: boolean;
  governanceLocked: boolean;
  fundBalance: bigint;
  createdAt: number;
}

// Entity Context (used by EntityProvider)
export interface EntityContext {
  entityId: number;
  entity: EntityData | null;
  isLoading: boolean;
  error: string | null;
  permissions: number;
  isOwner: boolean;
  isReadOnly: boolean;
  isSuspended: boolean;
  entityType: EntityType;
  governanceMode: GovernanceMode;
}

// Shop 数据
export interface PointsConfig {
  rewardRateBps: number;
  exchangeRateBps: number;
  transferable: boolean;
}

export interface ShopData {
  id: number;
  entityId: number;
  name: string;
  shopType: ShopType;
  operatingStatus: string;
  effectiveStatus: EffectiveShopStatus;
  fundBalance: bigint;
  pointsConfig: PointsConfig | null;
}

// 商品数据
export interface ProductData {
  id: number;
  shopId: number;
  nameCid: string;
  imageCid: string;
  detailCid: string;
  priceNex: bigint;
  priceUsdt: number;
  stock: number;
  category: ProductCategory;
  visibility: ProductVisibility;
  levelGate: number | null;
  status: ProductStatus;
  minQuantity: number;
  maxQuantity: number;
}

// 订单数据
export interface OrderData {
  id: number;
  shopId: number;
  productId: number;
  buyer: string;
  quantity: number;
  paymentAsset: PaymentAsset;
  totalAmount: bigint;
  status: OrderStatus;
  escrowId: number | null;
  createdAt: number;
  updatedAt: number;
  /** 商品类别，用于确定订单流程分化。链上查询时从关联商品获取 */
  productCategory?: ProductCategory;
}

// 代币配置
export interface TokenConfig {
  entityId: number;
  name: string;
  symbol: string;
  decimals: number;
  tokenType: TokenType;
  totalSupply: bigint;
  maxSupply: bigint;
  transferRestriction: TransferRestrictionMode;
  holderCount: number;
}

// 价格保护配置
export interface PriceProtectionConfig {
  maxDeviationBps: number;
  circuitBreakerThreshold: number;
  circuitBreakerDuration: number;
}

// 市场订单
export interface MarketOrder {
  id: number;
  entityId: number;
  trader: string;
  side: 'Buy' | 'Sell';
  price: bigint;
  amount: bigint;
  filled: bigint;
  createdAt: number;
}

// 市场统计
export interface MarketStats {
  twapPrice: bigint;
  lastPrice: bigint;
  volume24h: bigint;
  circuitBreakerActive: boolean;
  priceProtection: PriceProtectionConfig;
}

// 会员数据
export interface MemberData {
  entityId: number;
  account: string;
  status: MemberStatus;
  level: number;
  referrer: string | null;
  joinedAt: number;
  totalSpent: bigint;
  orderCount: number;
}

// 自定义等级
export interface CustomLevel {
  name: string;
  threshold: bigint;
  discountRate: number;
  commissionBonus: number;
}

// 提案数据
export interface ProposalData {
  id: number;
  entityId: number;
  proposer: string;
  proposalType: string;
  title: string;
  description: string;
  votesApprove: bigint;
  votesReject: bigint;
  votesAbstain: bigint;
  quorumPct: number;
  passThreshold: number;
  endBlock: number;
  status: string;
  executed: boolean;
}

// 佣金配置
export interface WithdrawalConfig {
  minAmount: bigint;
  feeRate: number;
  cooldown: number;
}

export interface CommissionConfig {
  entityId: number;
  enabled: boolean;
  baseRate: number;
  enabledModes: number;
  withdrawalConfig: WithdrawalConfig;
  withdrawalPaused: boolean;
}

// KYC 记录
export interface KycRecord {
  entityId: number;
  account: string;
  level: KycLevel;
  status: KycStatus;
  dataCid: string;
  countryCode: string | null;
  riskScore: number;
  submittedAt: number;
  expiresAt: number | null;
}

// Vesting 配置
export interface VestingConfig {
  cliffBlocks: number;
  vestingBlocks: number;
}

// 荷兰拍配置
export interface DutchAuctionConfig {
  startPrice: bigint;
  endPrice: bigint;
  decayBlocks: number;
}

// 发售轮次
export interface SaleRound {
  id: number;
  entityId: number;
  name: string;
  totalSupply: bigint;
  price: bigint;
  startBlock: number;
  endBlock: number;
  minPurchase: bigint;
  maxPurchase: bigint;
  softCap: bigint;
  hardCap: bigint;
  totalRaised: bigint;
  participantCount: number;
  vestingConfig: VestingConfig | null;
  dutchAuction: DutchAuctionConfig | null;
  status: string;
}

// 链上错误
export interface ChainError {
  module: string;
  name: string;
  message: string;
}

// 交易确认对话框配置
export interface ConfirmDialogConfig {
  title: string;
  description: string;
  severity: 'info' | 'warning' | 'danger';
  requireInput?: string;
}

// 通知
export interface Notification {
  id: string;
  pallet: string;
  event: string;
  summary: string;
  timestamp: number;
  read: boolean;
}

// 交易状态
export interface TxState {
  status: TxStatus;
  hash: string | null;
  error: string | null;
  blockNumber: number | null;
}
