export interface EntityData {
  id: number;
  owner: string;
  name: string;
  logoCid: string | null;
  descriptionCid: string | null;
  status: string;
  createdAt: number;
  entityType: string;
  admins: Array<{ address: string; permissions: number }>;
  governanceMode: string;
  verified: boolean;
  metadataUri: string | null;
  primaryShopId: number;
  totalSales: bigint;
  totalOrders: number;
}

export interface ShopData {
  id: number;
  entityId: number;
  isPrimary: boolean;
  name: string;
  logoCid: string | null;
  descriptionCid: string | null;
  shopType: string;
  status: string;
  managers: string[];
  customerService: string | null;
  initialFund: bigint;
  location: { lat: number; lng: number } | null;
  addressCid: string | null;
  businessHoursCid: string | null;
  createdAt: number;
  productCount: number;
  totalSales: bigint;
  totalOrders: number;
  rating: number;
  ratingTotal: number;
  ratingCount: number;
}

export interface ProductData {
  id: number;
  shopId: number;
  nameCid: string;
  imagesCid: string;
  detailCid: string;
  price: bigint;
  usdtPrice: number;
  stock: number;
  isDigital: boolean;
  status: string;
  salesCount: number;
  createdAt: number;
}

export interface OrderData {
  id: number;
  shopId: number;
  productId: number;
  buyer: string;
  quantity: number;
  totalPrice: bigint;
  tokenDiscount: bigint;
  shoppingBalanceDiscount: bigint;
  status: string;
  trackingCid: string | null;
  reasonCid: string | null;
  createdAt: number;
}

export interface TokenConfig {
  enabled: boolean;
  rewardRate: number;
  exchangeRate: number;
  minRedeem: bigint;
  maxRedeemPerOrder: bigint;
  transferable: boolean;
  createdAt: number;
  tokenType: string;
  maxSupply: bigint;
  dividendConfig: DividendConfig;
  transferRestriction: string;
  minReceiverKyc: number;
}

export interface DividendConfig {
  enabled: boolean;
  interval: number;
  minAmount: bigint;
  lastDistributed: number;
}

export interface MarketOrder {
  id: number;
  entityId: number;
  maker: string;
  side: "Buy" | "Sell";
  tokenAmount: bigint;
  price: bigint;
  filled: bigint;
  status: string;
  createdAt: number;
}

export interface MemberData {
  entityId: number;
  account: string;
  shopId: number;
  referrer: string | null;
  totalSpent: bigint;
  orderCount: number;
  customLevelId: number;
  joinedAt: number;
}

export interface LevelData {
  id: number;
  name: string;
  threshold: number;
  discountRate: number;
  commissionBonus: number;
}

export interface UpgradeRule {
  id: number;
  shopId: number;
  trigger: string;
  targetLevelId: number;
  threshold: bigint;
  priority: number;
  stackable: boolean;
  maxTriggers: number;
  triggerCount: number;
  enabled: boolean;
}

export interface ProposalData {
  id: number;
  entityId: number;
  proposer: string;
  proposalType: string;
  title: string;
  descriptionCid: string | null;
  status: string;
  yesVotes: bigint;
  noVotes: bigint;
  abstainVotes: bigint;
  votingEnd: number;
  executionTime: number | null;
  createdAt: number;
}

export interface DisclosureData {
  id: number;
  entityId: number;
  disclosureType: string;
  contentCid: string;
  status: string;
  materiality: string;
  publishedAt: number;
  correctedBy: number | null;
}

export interface AnnouncementData {
  id: number;
  entityId: number;
  title: string;
  contentCid: string;
  category: string;
  status: string;
  isPinned: boolean;
  expiresAt: number | null;
  createdAt: number;
}

export interface KycRecord {
  account: string;
  level: string;
  status: string;
  riskScore: number;
  countryCode: string;
  provider: string | null;
  expiresAt: number | null;
  createdAt: number;
}

export interface SaleRound {
  id: number;
  entityId: number;
  mode: string;
  totalAmount: bigint;
  remaining: bigint;
  price: bigint;
  startBlock: number;
  endBlock: number;
  status: string;
  totalRaised: bigint;
  subscriberCount: number;
  vestingConfig: VestingConfig | null;
}

export interface VestingConfig {
  initialUnlockPct: number;
  vestingBlocks: number;
}

export interface PointsConfig {
  name: string;
  symbol: string;
  rewardRate: number;
  exchangeRate: number;
  transferable: boolean;
  enabled: boolean;
}

export interface EntityStatistics {
  totalEntities: number;
  activeEntities: number;
}
