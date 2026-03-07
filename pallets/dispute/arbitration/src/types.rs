use codec::{Encode, Decode};
use frame_support::pallet_prelude::*;

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum Decision {
    Release,
    Refund,
    Partial(u16),
}

pub mod domains {
    pub const ENTITY_ORDER: [u8; 8] = *b"entorder";
    pub const OTC_ORDER: [u8; 8] = *b"otc_ord_";
    pub const MAKER: [u8; 8] = *b"maker___";
    pub const NFT_TRADE: [u8; 8] = *b"nft_trd_";
    pub const SWAP: [u8; 8] = *b"swap____";
    pub const MEMBER: [u8; 8] = *b"member__";
    pub const ADS: [u8; 8] = *b"ads_____";
    pub const TOKNSALE: [u8; 8] = *b"toknsale";
    pub const OTHER: [u8; 8] = *b"other___";
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum ComplaintType {
    // entity/order — 商城订单争议
    EntityOrderNotDeliver,
    EntityOrderFalseClaim,
    EntityOrderFraud,
    EntityOrderQuality,

    // trading/nex-market — OTC 法币交易
    OtcSellerNotDeliver,
    OtcBuyerFalseClaim,
    OtcTradeFraud,
    OtcPriceDispute,

    // trading/nex-market — 做市商
    MakerCreditDefault,
    MakerMaliciousOperation,
    MakerFalseQuote,

    // trading/nex-market — NFT 交易（预留）
    NftSellerNotDeliver,
    NftCounterfeit,
    NftTradeFraud,
    NftAuctionDispute,

    // trading/nex-market — Swap 交换（预留）
    SwapMakerNotComplete,
    SwapVerificationTimeout,
    SwapFraud,

    // entity/member — 会员
    MemberBenefitNotProvided,
    MemberServiceQuality,

    // ads/core — 广告
    AdsReceiptDispute,
    AdsFraudClick,
    AdsSettlementDispute,

    // entity/tokensale — 代币发售
    TokenSaleNotDeliver,
    TokenSaleTermsViolation,

    Other,
}

impl ComplaintType {
    pub fn domain(&self) -> [u8; 8] {
        match self {
            Self::EntityOrderNotDeliver | Self::EntityOrderFalseClaim |
            Self::EntityOrderFraud | Self::EntityOrderQuality => domains::ENTITY_ORDER,

            Self::OtcSellerNotDeliver | Self::OtcBuyerFalseClaim |
            Self::OtcTradeFraud | Self::OtcPriceDispute => domains::OTC_ORDER,

            Self::MakerCreditDefault | Self::MakerMaliciousOperation |
            Self::MakerFalseQuote => domains::MAKER,

            Self::NftSellerNotDeliver | Self::NftCounterfeit |
            Self::NftTradeFraud | Self::NftAuctionDispute => domains::NFT_TRADE,

            Self::SwapMakerNotComplete | Self::SwapVerificationTimeout |
            Self::SwapFraud => domains::SWAP,

            Self::MemberBenefitNotProvided | Self::MemberServiceQuality => domains::MEMBER,

            Self::AdsReceiptDispute | Self::AdsFraudClick |
            Self::AdsSettlementDispute => domains::ADS,

            Self::TokenSaleNotDeliver | Self::TokenSaleTermsViolation => domains::TOKNSALE,

            Self::Other => domains::OTHER,
        }
    }

    pub fn penalty_rate(&self) -> u16 {
        match self {
            Self::OtcTradeFraud | Self::EntityOrderFraud => 8000,
            Self::MakerMaliciousOperation | Self::AdsFraudClick => 5000,
            _ => 3000,
        }
    }

    pub fn triggers_permanent_ban(&self) -> bool {
        matches!(self, Self::OtcTradeFraud | Self::EntityOrderFraud)
    }
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ComplaintStatus {
    #[default]
    Submitted,
    Responded,
    Mediating,
    Arbitrating,
    ResolvedComplainantWin,
    ResolvedRespondentWin,
    ResolvedSettlement,
    Withdrawn,
    Expired,
    Appealed,
}

impl ComplaintStatus {
    pub fn is_resolved(&self) -> bool {
        matches!(self,
            Self::ResolvedComplainantWin |
            Self::ResolvedRespondentWin |
            Self::ResolvedSettlement |
            Self::Withdrawn |
            Self::Expired
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self,
            Self::Submitted |
            Self::Responded |
            Self::Mediating |
            Self::Arbitrating |
            Self::Appealed
        )
    }
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct ArchivedComplaint {
    pub id: u64,
    pub domain: [u8; 8],
    pub object_id: u64,
    /// 0=complainant win, 1=respondent win, 2=settlement, 3=withdrawn, 4=expired
    pub decision: u8,
    pub resolved_at: u64,
    pub year_month: u16,
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct DomainStatistics {
    pub total_complaints: u64,
    pub resolved_count: u64,
    pub complainant_wins: u64,
    pub respondent_wins: u64,
    pub settlements: u64,
    pub expired_count: u64,
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct ArchivedDispute {
    pub domain: [u8; 8],
    pub object_id: u64,
    /// 0=Release, 1=Refund, 2=Partial
    pub decision: u8,
    pub partial_bps: u16,
    pub completed_at: u64,
    pub year_month: u16,
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct ArbitrationPermanentStats {
    pub total_disputes: u64,
    pub release_count: u64,
    pub refund_count: u64,
    pub partial_count: u64,
}

#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct TwoWayDepositRecord<AccountId, Balance, BlockNumber> {
    pub initiator: AccountId,
    pub initiator_deposit: Balance,
    pub respondent: AccountId,
    pub respondent_deposit: Option<Balance>,
    pub response_deadline: BlockNumber,
    pub has_responded: bool,
}
