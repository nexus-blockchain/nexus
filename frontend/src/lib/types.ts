// ============================================================================
// Entity System Types
// ============================================================================

export interface EntityData {
  id: number;
  owner: string;
  name: string;
  logoCid: string | null;
  descriptionCid: string | null;
  metadataUri: string | null;
  status: string;
  entityType: string;
  governanceMode: string;
  verified: boolean;
  governanceLocked: boolean;
  admins: Array<{ address: string; permissions: number }>;
  shopIds: number[];
  primaryShopId: number;
  totalSales: bigint;
  totalOrders: number;
  fundBalance: bigint;
  createdAt: number;
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
  category: string;
  visibility: string;
  status: string;
  salesCount: number;
  minOrderQty: number;
  maxOrderQty: number;
  createdAt: number;
}

export interface OrderData {
  id: number;
  shopId: number;
  entityId: number;
  productId: number;
  buyer: string;
  seller: string;
  quantity: number;
  totalPrice: bigint;
  paymentAsset: string;
  tokenDiscount: bigint;
  shoppingBalanceDiscount: bigint;
  status: string;
  trackingCid: string | null;
  reasonCid: string | null;
  createdAt: number;
  paidAt: number | null;
  shippedAt: number | null;
  completedAt: number | null;
}

export interface TokenConfig {
  enabled: boolean;
  name: string;
  symbol: string;
  decimals: number;
  tokenType: string;
  rewardRate: number;
  exchangeRate: number;
  minRedeem: bigint;
  maxRedeemPerOrder: bigint;
  transferable: boolean;
  transferRestriction: string;
  maxSupply: bigint;
  totalSupply: bigint;
  holderCount: number;
  dividendConfig: DividendConfig;
  minReceiverKyc: number;
  createdAt: number;
}

export interface DividendConfig {
  enabled: boolean;
  minPeriod: number;
  lastDistribution: number;
  accumulated: bigint;
}

export interface VestingSchedule {
  total: bigint;
  released: bigint;
  startBlock: number;
  cliffBlocks: number;
  vestingBlocks: number;
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
  referrer: string | null;
  introducedBy: string | null;
  status: string;
  customLevelId: number;
  totalSpentUsdt: number;
  orderCount: number;
  directReferrals: number;
  teamSize: number;
  joinedAt: number;
  lastActiveAt: number;
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
  entityId: number;
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
  proposalType: string | Record<string, unknown>;
  title: string;
  descriptionCid: string | null;
  status: string;
  createdAt: number;
  votingStart: number;
  votingEnd: number;
  executionTime: number | null;
  yesVotes: bigint;
  noVotes: bigint;
  abstainVotes: bigint;
  snapshotQuorum: number;
  snapshotPass: number;
  snapshotExecutionDelay: number;
  snapshotTotalSupply: bigint;
}

export interface GovernanceConfigData {
  mode: string;
  votingPeriod: number;
  executionDelay: number;
  quorumThreshold: number;
  passThreshold: number;
  proposalThreshold: number;
  adminVetoEnabled: boolean;
}

export interface VoteRecordData {
  voter: string;
  vote: string;
  weight: bigint;
  votedAt: number;
}

export interface DisclosureData {
  id: number;
  entityId: number;
  disclosureType: string;
  contentCid: string;
  summaryCid: string | null;
  discloser: string;
  disclosedAt: number;
  status: string;
  previousId: number | null;
}

export interface DisclosureConfigData {
  level: string;
  insiderTradingControl: boolean;
  blackoutPeriodAfter: number;
  nextRequiredDisclosure: number;
  lastDisclosure: number;
  violationCount: number;
}

export interface InsiderRecord {
  account: string;
  role: string;
  addedAt: number;
}

export interface AnnouncementData {
  id: number;
  entityId: number;
  category: string;
  title: string;
  contentCid: string;
  publisher: string;
  publishedAt: number;
  expiresAt: number | null;
  status: string;
  isPinned: boolean;
}

export interface KycRecord {
  account: string;
  level: string;
  status: string;
  provider: string | null;
  dataCid: string | null;
  submittedAt: number;
  verifiedAt: number | null;
  expiresAt: number | null;
  rejectionReason: string | null;
  rejectionDetailsCid: string | null;
  countryCode: string;
  riskScore: number;
}

export interface KycProviderData {
  account: string;
  name: string;
  providerType: string;
  maxLevel: string;
  verificationsCount: number;
  suspended: boolean;
}

export interface EntityKycRequirement {
  minLevel: string;
  mandatory: boolean;
  gracePeriod: number;
  allowHighRiskCountries: boolean;
  maxRiskScore: number;
}

export interface SaleRound {
  id: number;
  entityId: number;
  mode: string;
  status: string;
  totalSupply: bigint;
  soldAmount: bigint;
  remainingAmount: bigint;
  participantsCount: number;
  paymentOptionsCount: number;
  vestingConfig: TokenSaleVestingConfig | null;
  kycRequired: boolean;
  minKycLevel: number;
  startBlock: number;
  endBlock: number;
  dutchStartPrice: bigint | null;
  dutchEndPrice: bigint | null;
  creator: string;
  createdAt: number;
  fundsWithdrawn: boolean;
  cancelledAt: number | null;
  totalRefundedTokens: bigint;
  totalRefundedNex: bigint;
  softCap: bigint;
}

export interface TokenSaleVestingConfig {
  vestingType: string;
  initialUnlockBps: number;
  cliffDuration: number;
  totalDuration: number;
  unlockInterval: number;
}

export interface PaymentOptionConfig {
  assetId: number | null;
  price: bigint;
  minPurchase: bigint;
  maxPurchasePerAccount: bigint;
  enabled: boolean;
}

export interface SubscriptionData {
  subscriber: string;
  roundId: number;
  amount: bigint;
  paymentAsset: number | null;
  paymentAmount: bigint;
  subscribedAt: number;
  claimed: boolean;
  unlockedAmount: bigint;
  refunded: boolean;
}

export interface VestingConfig {
  cliffBlocks: number;
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

export interface CommissionConfig {
  modes: number;
  baseRate: number;
  enabled: boolean;
}

// ============================================================================
// NEX P2P Market Types (pallet-nex-market)
// ============================================================================

export interface NexOrder {
  id: number;
  maker: string;
  side: "Sell" | "Buy";
  nexAmount: bigint;
  filledAmount: bigint;
  usdtPrice: number;
  tronAddress: string | null;
  status: string;
  createdAt: number;
  expiresAt: number | null;
  buyerDeposit: bigint;
}

export interface UsdtTrade {
  id: number;
  orderId: number;
  buyer: string;
  seller: string;
  nexAmount: bigint;
  usdtAmount: number;
  tronAddress: string;
  txHash: string | null;
  status: string;
  depositStatus: string;
  createdAt: number;
  completedAt: number | null;
  underpaidDeadline: number | null;
}

export interface NexMarketStats {
  totalVolume: bigint;
  tradeCount: number;
  lastPrice: number;
}

export interface PriceProtectionConfig {
  maxDeviationBps: number;
  circuitBreakerThreshold: number;
  circuitBreakerDuration: number;
}

export interface TradeDispute {
  tradeId: number;
  initiator: string;
  status: string;
  createdAt: number;
  evidenceCid: string;
}

// ============================================================================
// IPFS Storage Types (pallet-storage-service)
// ============================================================================

export interface StorageOperator {
  account: string;
  peerId: string;
  capacityGib: number;
  endpointHash: string;
  status: number;
  registeredAt: number;
  layer: string;
  priority: number;
  bond: bigint;
  usedBytes: bigint;
  pinCount: number;
  rewards: bigint;
}

export interface PinInfo {
  cidHash: string;
  cid: string;
  state: string;
  tier: string;
  sizeBytes: number;
  replicas: number;
  createdAt: number;
  lastActivity: number;
  owner: string;
  subjectId: number | null;
}

export interface DomainConfig {
  autoPinEnabled: boolean;
  defaultTier: string;
  subjectTypeId: number;
  createdAt: number;
}

// ============================================================================
// GroupRobot Types
// ============================================================================

export interface BotInfo {
  botIdHash: string;
  owner: string;
  publicKey: string;
  status: string;
  registeredAt: number;
  nodeType: string;
  communityCount: number;
}

export interface CommunityBinding {
  communityIdHash: string;
  platform: string;
  botIdHash: string;
  boundBy: string;
  boundAt: number;
}

export interface AttestationRecord {
  botIdHash: string;
  teeType: string;
  primaryMeasurement: string;
  attester: string;
  attestedAt: number;
  expiresAt: number;
  dcapLevel: number;
  quoteVerified: boolean;
}

export interface PeerEndpoint {
  publicKey: string;
  endpoint: string;
  registeredAt: number;
  lastSeen: number;
}

export interface OperatorInfo {
  owner: string;
  platform: string;
  platformAppHash: string;
  name: string;
  contact: string;
  status: string;
  registeredAt: number;
  botCount: number;
  slaLevel: number;
  reputationScore: number;
}

export interface CommunityConfig {
  nodeRequirement: string;
  antiFloodEnabled: boolean;
  floodLimit: number;
  warnLimit: number;
  warnAction: string;
  welcomeEnabled: boolean;
  adsEnabled: boolean;
  activeMembers: number;
  language: string;
  status: string;
}

export interface ReputationRecord {
  score: number;
  awards: number;
  deductions: number;
  lastUpdated: number;
}

export interface ConsensusNode {
  operator: string;
  nodeId: string;
  status: string;
  stake: bigint;
  registeredAt: number;
  isTeeNode: boolean;
}

export interface SubscriptionRecord {
  owner: string;
  botIdHash: string;
  tier: string;
  feePerEra: bigint;
  startedAt: number;
  status: string;
}

export interface EraRewardInfo {
  subscriptionIncome: bigint;
  adsIncome: bigint;
  inflationMint: bigint;
  totalDistributed: bigint;
  treasuryShare: bigint;
  nodeCount: number;
}

// ============================================================================
// Dispute Types (pallet-dispute-*)
// ============================================================================

export interface ComplaintData {
  id: number;
  complainant: string;
  respondent: string;
  complaintType: string;
  status: string;
  evidenceIds: number[];
  objectId: number | null;
  createdAt: number;
  deposit: bigint;
}

export interface EscrowData {
  from: string;
  to: string;
  amount: bigint;
  state: number;
  disputedAt: number | null;
}

export interface EvidenceData {
  id: number;
  submitter: string;
  target: string;
  namespace: string;
  contentCid: string;
  imagesCids: string[];
  videosCids: string[];
  docsCids: string[];
  memo: string;
  contentType: string;
  status: string;
  sealed: boolean;
  createdAt: number;
  parentId: number | null;
}

// ============================================================================
// Ads Types (pallet-ads-*)
// ============================================================================

export interface AdCampaign {
  id: number;
  advertiser: string;
  text: string;
  url: string;
  bidPerMille: bigint;
  bidPerClick: bigint;
  campaignType: string;
  dailyBudget: bigint;
  totalBudget: bigint;
  spent: bigint;
  status: string;
  reviewStatus: string;
  totalDeliveries: number;
  totalClicks: number;
  createdAt: number;
  expiresAt: number;
}

export interface AdPlacement {
  placementId: string;
  entityId: number | null;
  shopId: number | null;
  level: string;
  dailyImpressionCap: number;
  dailyClickCap: number;
  registeredBy: string;
  registeredAt: number;
  active: boolean;
}

export interface CommunityAdStake {
  communityIdHash: string;
  stakerCount: number;
  totalStake: bigint;
  audienceCap: number;
  adminPaused: boolean;
}

// ============================================================================
// Shared / Utility
// ============================================================================

export interface PageRequest {
  offset: number;
  limit: number;
}

export interface PageResponse<T> {
  items: T[];
  total: number;
  hasMore: boolean;
}

export interface EntityStatistics {
  totalEntities: number;
  activeEntities: number;
}
