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
  orderId: number;
  maker: string;
  side: string;
  nexAmount: bigint;
  filledAmount: bigint;
  usdtPrice: number;
  tronAddress: string | null;
  status: string;
  createdAt: number;
  expiresAt: number;
  buyerDeposit: bigint;
  depositWaived: boolean;
}

export interface UsdtTrade {
  tradeId: number;
  orderId: number;
  seller: string;
  buyer: string;
  nexAmount: bigint;
  usdtAmount: number;
  sellerTronAddress: string;
  buyerTronAddress: string | null;
  status: string;
  createdAt: number;
  timeoutAt: number;
  buyerDeposit: bigint;
  depositStatus: string;
  firstVerifiedAt: number | null;
  firstActualAmount: number | null;
  underpaidDeadline: number | null;
}

export interface NexMarketStats {
  totalOrders: number;
  totalTrades: number;
  totalVolumeUsdt: number;
}

export interface TwapData {
  currentCumulative: bigint;
  currentBlock: number;
  lastPrice: number;
  tradeCount: number;
  hourSnapshot: { cumulativePrice: bigint; blockNumber: number };
  daySnapshot: { cumulativePrice: bigint; blockNumber: number };
  weekSnapshot: { cumulativePrice: bigint; blockNumber: number };
}

export interface PriceProtectionConfig {
  enabled: boolean;
  maxPriceDeviation: number;
  circuitBreakerThreshold: number;
  minTradesForTwap: number;
  circuitBreakerActive: boolean;
  circuitBreakerUntil: number;
  initialPrice: number | null;
}

export interface TradeDispute {
  tradeId: number;
  initiator: string;
  status: string;
  createdAt: number;
  evidenceCid: string;
}

export interface MarketSummary {
  bestAsk: number | null;
  bestBid: number | null;
  lastTradePrice: number | null;
  isPaused: boolean;
  tradingFeeBps: number;
  pendingTradesCount: number;
}

// ============================================================================
// IPFS Storage Types (pallet-storage-service)
// ============================================================================

export interface StorageOperator {
  account: string;
  peerId: string;
  capacityGib: number;
  endpointHash: string;
  certFingerprint: string | null;
  status: number;
  registeredAt: number;
  layer: string;
  priority: number;
  bond: bigint;
  usedBytes: number;
  pinCount: number;
  rewards: bigint;
  healthScore: number;
}

export interface OperatorSlaData {
  pinnedBytes: number;
  probeOk: number;
  probeFail: number;
  degraded: number;
  lastUpdate: number;
}

export interface PinInfo {
  cidHash: string;
  cid: string;
  state: string;
  tier: string;
  size: number;
  replicas: number;
  createdAt: number;
  lastActivity: number;
  owner: string;
  subjectId: number | null;
}

export interface BillingParams {
  pricePerGibWeek: bigint;
  periodBlocks: number;
  graceBlocks: number;
  maxChargePerBlock: number;
  subjectMinReserve: bigint;
  paused: boolean;
}

export interface GlobalHealthStats {
  totalPins: number;
  totalSizeBytes: number;
  healthyCount: number;
  degradedCount: number;
  criticalCount: number;
  lastFullScan: number;
  totalRepairs: number;
}

export interface DomainConfig {
  autoPinEnabled: boolean;
  defaultTier: string;
  subjectTypeId: number;
  ownerPallet: string;
  createdAt: number;
}

export interface TierConfig {
  replicas: number;
  healthCheckInterval: number;
  feeMultiplier: number;
  gracePeriodBlocks: number;
  enabled: boolean;
}

export interface ArchiveConfig {
  l1Delay: number;
  l2Delay: number;
  purgeDelay: number;
  purgeEnabled: boolean;
  maxBatchSize: number;
}

export interface ArchiveStats {
  totalL1Archived: number;
  totalL2Archived: number;
  totalPurged: number;
  totalBytesSaved: number;
  lastArchiveAt: number;
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
  version: number;
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

export interface AdCommitmentRecord {
  owner: string;
  botIdHash: string;
  communityIdHash: string;
  committedAdsPerEra: number;
  effectiveTier: string;
  underdeliveryEras: number;
  status: string;
  startedAt: number;
}

export interface CeremonyRecord {
  ceremonyMrenclave: string;
  k: number;
  n: number;
  botPublicKey: string;
  participantCount: number;
  participantEnclaves: string[];
  initiator: string;
  createdAt: number;
  status: string;
  expiresAt: number;
  isReCeremony: boolean;
  supersedes: string | null;
  botIdHash: string;
}

export interface TierFeatureGate {
  maxRules: number;
  logRetentionDays: number;
  forcedAdsPerDay: number;
  canDisableAds: boolean;
  teeAccess: boolean;
}

export interface EraRewardInfo {
  subscriptionIncome: bigint;
  adsIncome: bigint;
  inflationMint: bigint;
  totalDistributed: bigint;
  treasuryShare: bigint;
  nodeCount: number;
}

export interface NodeRewardSummary {
  pending: bigint;
  totalEarned: bigint;
}

// ============================================================================
// Dispute Types (pallet-dispute-*)
// ============================================================================

export interface ComplaintData {
  id: number;
  domain: string;
  objectId: number;
  complaintType: string;
  complainant: string;
  respondent: string;
  detailsCid: string;
  responseCid: string | null;
  amount: bigint | null;
  status: string;
  createdAt: number;
  responseDeadline: number;
  settlementCid: string | null;
  resolutionCid: string | null;
  updatedAt: number;
}

export interface EscrowData {
  id: number;
  amount: bigint;
  state: number;
  nonce: number;
  expiresAt: number | null;
  disputedAt: number | null;
}

export interface EvidenceData {
  id: number;
  domain: number;
  targetId: number;
  owner: string;
  contentCid: string;
  contentType: string;
  createdAt: number;
  isEncrypted: boolean;
  commit: string | null;
  ns: string | null;
  status: string;
}

export interface ArbitrationStats {
  totalDisputes: number;
  releaseCount: number;
  refundCount: number;
  partialCount: number;
}

export interface DomainStatistics {
  totalComplaints: number;
  resolvedCount: number;
  complainantWins: number;
  respondentWins: number;
  settlements: number;
  expiredCount: number;
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
  deliveryTypes: number;
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
