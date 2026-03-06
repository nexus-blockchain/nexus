#![cfg_attr(not(feature = "std"), no_std)]
//! 说明：临时全局允许 `deprecated`（RuntimeEvent/常量权重），后续移除
#![allow(deprecated)]

extern crate alloc;

pub use pallet::*;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::weights::WeightInfo;
    use frame_support::traits::{EnsureOrigin, fungible::{Inspect as FungibleInspect, Mutate as FungibleMutate, MutateHold as FungibleMutateHold}};
    use frame_support::{pallet_prelude::*, BoundedVec};
    use frame_system::pallet_prelude::*;
    use pallet_escrow::pallet::Escrow as EscrowTrait;
    use pallet_storage_service::CidLockManager;
    use pallet_trading_common::PricingProvider;
    use sp_runtime::{Saturating, SaturatedConversion};
    use pallet_storage_lifecycle::block_to_year_month;
    // 基准模块在 pallet 外部声明；此处不在 proc-macro 输入中声明子模块，避免 E0658

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    pub enum Decision {
        Release,
        Refund,
        Partial(u16),
    } // bps

    // ============================================================================
    // 🆕 Phase 1-4: 统一投诉系统类型定义
    // ============================================================================

    /// 业务域常量 (12个域)
    pub mod domains {
        /// OTC 交易投诉域
        pub const OTC_ORDER: [u8; 8] = *b"otc_ord_";
        /// 直播投诉域
        pub const LIVESTREAM: [u8; 8] = *b"livstrm_";
        /// 做市商投诉域
        pub const MAKER: [u8; 8] = *b"maker___";
        /// NFT 交易投诉域
        pub const NFT_TRADE: [u8; 8] = *b"nft_trd_";
        /// Swap 交换投诉域
        pub const SWAP: [u8; 8] = *b"swap____";
        /// 会员投诉域
        pub const MEMBER: [u8; 8] = *b"member__";
        /// 信用系统申诉域
        pub const CREDIT: [u8; 8] = *b"credit__";
        /// 其他
        pub const OTHER: [u8; 8] = *b"other___";
    }

    /// 统一投诉类型枚举 (56种类型，覆盖12个业务域)
    /// 
    /// 设计原则：
    /// - 按业务域分组，保持原有语义
    /// - 每个类型与原模块类型一一对应
    /// - 前端可根据域筛选显示
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ComplaintType {
        // ========== OTC 交易投诉 (域: otc_ord_) ==========
        /// 卖家未放币
        OtcSellerNotDeliver,
        /// 买家虚假付款声明
        OtcBuyerFalseClaim,
        /// OTC 交易欺诈
        OtcTradeFraud,
        /// OTC 价格争议
        OtcPriceDispute,
        
        // ========== 直播投诉 (域: livstrm_) ==========
        /// 直播违规内容
        LiveIllegalContent,
        /// 直播虚假宣传
        LiveFalseAdvertising,
        /// 直播骚扰观众
        LiveHarassment,
        /// 直播诈骗
        LiveFraud,
        /// 礼物退款请求
        LiveGiftRefund,
        /// 直播其他违规
        LiveOther,
        
        // ========== 做市商投诉 (域: maker___) ==========
        /// 做市商信用违约
        MakerCreditDefault,
        /// 做市商恶意操作
        MakerMaliciousOperation,
        /// 做市商虚假报价
        MakerFalseQuote,
        
        // ========== NFT 交易投诉 (域: nft_trd_) ==========
        /// NFT 卖家未交付
        NftSellerNotDeliver,
        /// NFT 假冒/盗版
        NftCounterfeit,
        /// NFT 交易欺诈
        NftTradeFraud,
        /// NFT 拍卖/出价争议
        NftAuctionDispute,
        
        // ========== Swap 交换投诉 (域: swap____) ==========
        /// Swap 做市商未完成交换
        SwapMakerNotComplete,
        /// Swap 验证超时
        SwapVerificationTimeout,
        /// Swap 交换欺诈
        SwapFraud,
        
        // ========== 会员投诉 (域: member__) ==========
        /// 会员权益未兑现
        MemberBenefitNotProvided,
        /// 会员服务质量问题
        MemberServiceQuality,
        
        // ========== 信用系统申诉 (域: credit__) ==========
        /// 信用评分争议
        CreditScoreDispute,
        /// 被错误惩罚申诉
        CreditPenaltyAppeal,
        
        // ========== 其他 ==========
        /// 其他投诉
        Other,
    }

    impl ComplaintType {
        /// 获取所属业务域
        pub fn domain(&self) -> [u8; 8] {
            match self {
                // OTC 交易
                Self::OtcSellerNotDeliver | Self::OtcBuyerFalseClaim | 
                Self::OtcTradeFraud | Self::OtcPriceDispute => domains::OTC_ORDER,
                
                // 直播
                Self::LiveIllegalContent | Self::LiveFalseAdvertising | 
                Self::LiveHarassment | Self::LiveFraud | 
                Self::LiveGiftRefund | Self::LiveOther => domains::LIVESTREAM,
                
                // 做市商
                Self::MakerCreditDefault | Self::MakerMaliciousOperation |
                Self::MakerFalseQuote => domains::MAKER,
                
                // NFT 交易
                Self::NftSellerNotDeliver | Self::NftCounterfeit |
                Self::NftTradeFraud | Self::NftAuctionDispute => domains::NFT_TRADE,
                
                // Swap 交换
                Self::SwapMakerNotComplete | Self::SwapVerificationTimeout |
                Self::SwapFraud => domains::SWAP,
                
                // 会员
                Self::MemberBenefitNotProvided | Self::MemberServiceQuality => domains::MEMBER,
                
                // 信用系统
                Self::CreditScoreDispute | Self::CreditPenaltyAppeal => domains::CREDIT,
                
                // 其他
                Self::Other => domains::OTHER,
            }
        }
        
        /// 获取惩罚比例（基点，10000 = 100%）
        pub fn penalty_rate(&self) -> u16 {
            match self {
                // 重度违规，80%罚没
                Self::OtcTradeFraud => 8000,
                // 中度违规，50%罚没
                Self::LiveIllegalContent |
                Self::MakerMaliciousOperation => 5000,
                // 轻度违规，30%罚没
                _ => 3000,
            }
        }
        
        /// 是否触发永久封禁
        pub fn triggers_permanent_ban(&self) -> bool {
            matches!(self, Self::OtcTradeFraud)
        }
    }

    /// 投诉状态枚举（精简版）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum ComplaintStatus {
        /// 已提交，等待响应
        #[default]
        Submitted,
        /// 已响应/申诉
        Responded,
        /// 调解中
        Mediating,
        /// 仲裁中
        Arbitrating,
        /// 已解决 - 投诉方胜诉
        ResolvedComplainantWin,
        /// 已解决 - 被投诉方胜诉
        ResolvedRespondentWin,
        /// 已解决 - 和解
        ResolvedSettlement,
        /// 已撤销
        Withdrawn,
        /// 已过期
        Expired,
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
    }

    /// 投诉记录（精简版，链上存储优化）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct Complaint<T: Config> {
        /// 投诉唯一ID
        pub id: u64,
        /// 业务域标识
        pub domain: [u8; 8],
        /// 业务对象ID
        pub object_id: u64,
        /// 投诉类型
        pub complaint_type: ComplaintType,
        /// 投诉发起人
        pub complainant: T::AccountId,
        /// 被投诉人
        pub respondent: T::AccountId,
        /// 详情CID（指向IPFS完整内容）
        pub details_cid: BoundedVec<u8, T::MaxCidLen>,
        /// 🆕 A2修复: 被投诉方响应CID（独立存储，不覆盖原始投诉详情）
        pub response_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
        /// 涉及金额
        pub amount: Option<BalanceOf<T>>,
        /// 当前状态
        pub status: ComplaintStatus,
        /// 创建时间
        pub created_at: BlockNumberFor<T>,
        /// 响应截止时间
        pub response_deadline: BlockNumberFor<T>,
        /// 🆕 M-NEW-3修复: 和解详情CID（独立存储，不覆盖原始投诉详情）
        pub settlement_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
        /// 🆕 M-NEW-3修复: 仲裁裁决CID（独立存储，不覆盖原始投诉详情）
        pub resolution_cid: Option<BoundedVec<u8, T::MaxCidLen>>,
        /// 最后更新时间
        pub updated_at: BlockNumberFor<T>,
    }

    /// 归档投诉摘要（超精简，~38字节）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct ArchivedComplaint {
        /// 投诉ID
        pub id: u64,
        /// 业务域
        pub domain: [u8; 8],
        /// 业务对象ID
        pub object_id: u64,
        /// 裁决结果 (0=投诉方胜, 1=被投诉方胜, 2=和解, 3=撤销, 4=过期)
        pub decision: u8,
        /// 解决时间（区块号）
        pub resolved_at: u32,
        /// 年月（YYMM格式）
        pub year_month: u16,
    }

    /// 域统计信息
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DomainStatistics {
        /// 总投诉数
        pub total_complaints: u64,
        /// 已解决数
        pub resolved_count: u64,
        /// 投诉方胜诉数
        pub complainant_wins: u64,
        /// 被投诉方胜诉数
        pub respondent_wins: u64,
        /// 和解数
        pub settlements: u64,
        /// 过期数
        pub expired_count: u64,
    }

    // ============================================================================

    /// 仲裁域路由接口：由 runtime 实现，根据域将仲裁请求路由到对应业务 pallet
    ///
    /// 设计目的：
    /// - 以 [u8;8] 域常量（通常与 PalletId 字节对齐）标识业务域
    /// - can_dispute：校验发起人是否有权对 (domain, id) 发起争议
    /// - apply_decision：按裁决对 (domain, id) 应用资金与状态变更（由各业务 pallet 内部完成）
    /// - get_counterparty：获取纠纷对方账户（用于双向押金）
    /// - get_order_amount：获取订单/交易金额（用于计算押金比例）
    pub trait ArbitrationRouter<AccountId, Balance> {
        /// 校验是否允许发起争议
        fn can_dispute(domain: [u8; 8], who: &AccountId, id: u64) -> bool;
        /// 应用裁决（放款/退款/部分放款）
        fn apply_decision(domain: [u8; 8], id: u64, decision: Decision) -> DispatchResult;
        /// 获取纠纷对方账户（发起方是买家，返回卖家；反之亦然）
        fn get_counterparty(domain: [u8; 8], initiator: &AccountId, id: u64) -> Result<AccountId, DispatchError>;
        /// 🆕 获取订单/交易金额（用于计算押金）
        fn get_order_amount(domain: [u8; 8], id: u64) -> Result<Balance, DispatchError>;
        /// 🆕 获取做市商ID（用于信用分更新，仅OTC域有效）
        fn get_maker_id(_domain: [u8; 8], _id: u64) -> Option<u64> { None }
        /// 🆕 F9: 执行永久封禁（由投诉裁决触发）
        fn ban_account(_domain: [u8; 8], _who: &AccountId, _id: u64) -> DispatchResult { Ok(()) }
    }

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_escrow::pallet::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type MaxEvidence: Get<u32>;
        type MaxCidLen: Get<u32>;
        /// 托管接口（调用释放/退款/部分分账）
        type Escrow: EscrowTrait<Self::AccountId, BalanceOf<Self>>;
        /// 权重信息
        type WeightInfo: weights::WeightInfo;
        /// 域路由：把仲裁请求路由到对应业务 pallet 的仲裁钩子
        type Router: ArbitrationRouter<Self::AccountId, BalanceOf<Self>>;
        /// 函数级中文注释：仲裁决策起源（治理）。
        /// - 由 runtime 绑定为 Root 或 内容委员会 阈值（例如 2/3 通过）。
        /// - 用于 `arbitrate` 裁决入口的权限校验，替代任意签名账户。
        type DecisionOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 🆕 双向押金相关配置
        /// Fungible 接口：用于锁定和释放押金
        type Fungible: FungibleInspect<Self::AccountId, Balance = BalanceOf<Self>>
            + FungibleMutate<Self::AccountId>
            + FungibleMutateHold<Self::AccountId, Reason = Self::RuntimeHoldReason>;
        /// RuntimeHoldReason：押金锁定原因标识
        type RuntimeHoldReason: From<HoldReason>;
        /// 🆕 押金比例（基点，1500 = 15%）
        type DepositRatioBps: Get<u16>;
        /// 应诉期限（区块数，默认 7 天）
        type ResponseDeadline: Get<BlockNumberFor<Self>>;
        /// 驳回罚没比例（基点，3000 = 30%）
        type RejectedSlashBps: Get<u16>;
        /// 部分胜诉罚没比例（基点，5000 = 50%）
        type PartialSlashBps: Get<u16>;
        /// 投诉押金兜底金额（COS数量，pricing不可用时使用）
        #[pallet::constant]
        type ComplaintDeposit: Get<BalanceOf<Self>>;
        /// 投诉押金USD价值（精度10^6，1_000_000 = 1 USDT）
        #[pallet::constant]
        type ComplaintDepositUsd: Get<u64>;
        /// 定价接口（用于换算投诉押金）
        type Pricing: pallet_trading_common::PricingProvider<BalanceOf<Self>>;
        /// 投诉败诉罚没比例（基点，5000 = 50%）
        #[pallet::constant]
        type ComplaintSlashBps: Get<u16>;
        /// 国库账户
        type TreasuryAccount: Get<Self::AccountId>;
        
        /// 🆕 P2: CID 锁定管理器（仲裁期间锁定证据 CID）
        /// 
        /// 功能：
        /// - 发起仲裁时自动锁定相关证据 CID
        /// - 仲裁完成后自动解锁
        /// - 防止仲裁期间证据被删除
        type CidLockManager: pallet_storage_service::CidLockManager<Self::Hash, BlockNumberFor<Self>>;
        
        /// 🆕 信用分更新器（仲裁结果反馈到信用系统）
        /// 
        /// 功能：
        /// - 做市商败诉时扣除信用分
        /// - 做市商胜诉时可选加分
        type CreditUpdater: CreditUpdater;

        /// 🆕 防膨胀: 归档记录 TTL（区块数，超过此值的归档记录将被清理）
        /// 默认 2_592_000 ≈ 180天 (6s/block)。设为 0 禁用清理。
        #[pallet::constant]
        type ArchiveTtlBlocks: Get<u32>;

        /// 🆕 M-NEW-5修复: 投诉归档延迟（区块数），投诉解决后多久可归档
        /// 默认 432_000 ≈ 30天 (6s/block)
        #[pallet::constant]
        type ComplaintArchiveDelayBlocks: Get<BlockNumberFor<Self>>;

        /// 🆕 M-NEW-6修复: 证据存在性检查器
        /// 验证 evidence_id 在 pallet-evidence 中是否实际存在
        type EvidenceExists: EvidenceExistenceChecker;
    }

    /// 🆕 M-NEW-6修复: 证据存在性检查接口
    pub trait EvidenceExistenceChecker {
        /// 返回指定 evidence_id 是否存在
        fn evidence_exists(id: u64) -> bool;
    }
    
    /// 信用分更新接口
    pub trait CreditUpdater {
        /// 记录做市商争议结果
        /// - maker_id: 做市商ID
        /// - order_id: 订单ID
        /// - maker_win: 做市商是否胜诉
        fn record_maker_dispute_result(maker_id: u64, order_id: u64, maker_win: bool) -> DispatchResult;
    }
    
    /// 空实现（用于不需要信用集成的场景）
    impl CreditUpdater for () {
        fn record_maker_dispute_result(_: u64, _: u64, _: bool) -> DispatchResult { Ok(()) }
    }

    pub type BalanceOf<T> =
        <<T as pallet_escrow::pallet::Config>::Currency as frame_support::traits::Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance;

    /// 🆕 押金锁定原因枚举
    #[pallet::composite_enum]
    pub enum HoldReason {
        /// 纠纷发起方押金
        DisputeInitiator,
        /// 应诉方押金
        DisputeRespondent,
        /// 投诉押金（防止恶意投诉）
        ComplaintDeposit,
    }

    /// 🆕 存储膨胀防护：归档仲裁记录（精简版）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct ArchivedDispute {
        /// 域（业务类型）
        pub domain: [u8; 8],
        /// 对象ID
        pub object_id: u64,
        /// 裁决结果 (0=Release, 1=Refund, 2=Partial)
        pub decision: u8,
        /// 部分裁决比例（基点）
        pub partial_bps: u16,
        /// 完成区块
        pub completed_at: u32,
        /// 年月 (YYMM格式)
        pub year_month: u16,
    }

    /// 🆕 存储膨胀防护：仲裁永久统计
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct ArbitrationPermanentStats {
        /// 总仲裁数
        pub total_disputes: u64,
        /// Release裁决数
        pub release_count: u64,
        /// Refund裁决数
        pub refund_count: u64,
        /// Partial裁决数
        pub partial_count: u64,
    }

    /// 🆕 双向押金记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
    pub struct TwoWayDepositRecord<AccountId, Balance, BlockNumber> {
        /// 发起方账户
        pub initiator: AccountId,
        /// 发起方押金金额
        pub initiator_deposit: Balance,
        /// 应诉方账户
        pub respondent: AccountId,
        /// 应诉方押金金额（可选，未应诉时为 None）
        pub respondent_deposit: Option<Balance>,
        /// 应诉截止区块
        pub response_deadline: BlockNumber,
        /// 是否已应诉
        pub has_responded: bool,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// 争议登记：(domain, object_id) => ()
    #[pallet::storage]
    pub type Disputed<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, [u8; 8], Blake2_128Concat, u64, (), OptionQuery>;

    /// 函数级中文注释：每个仲裁案件引用的 evidence_id 列表（证据本体由 pallet-evidence 存储）。
    #[pallet::storage]
    pub type EvidenceIds<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 8],
        Blake2_128Concat,
        u64,
        BoundedVec<u64, T::MaxEvidence>,
        ValueQuery,
    >;

    /// 🆕 P2: 仲裁案件关联的 CID 哈希列表（用于锁定/解锁）
    /// 
    /// 存储结构：(domain, object_id) => Vec<CidHash>
    /// 由 dispute 时传入或从 Evidence 模块解析
    #[pallet::storage]
    pub type LockedCidHashes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 8],
        Blake2_128Concat,
        u64,
        BoundedVec<T::Hash, T::MaxEvidence>,
        ValueQuery,
    >;

    /// 🆕 双向押金记录存储：(domain, object_id) => TwoWayDepositRecord
    #[pallet::storage]
    pub type TwoWayDeposits<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 8],
        Blake2_128Concat,
        u64,
        TwoWayDepositRecord<T::AccountId, BalanceOf<T>, BlockNumberFor<T>>,
        OptionQuery,
    >;

    // ==================== 🆕 存储膨胀防护：归档存储 ====================

    /// 下一个归档ID
    #[pallet::storage]
    pub type NextArchivedId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 归档仲裁记录
    #[pallet::storage]
    #[pallet::getter(fn archived_disputes)]
    pub type ArchivedDisputes<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // archived_id
        ArchivedDispute,
        OptionQuery,
    >;

    /// 仲裁永久统计
    #[pallet::storage]
    #[pallet::getter(fn arbitration_stats)]
    pub type ArbitrationStats<T: Config> = StorageValue<_, ArbitrationPermanentStats, ValueQuery>;

    // ==================== 🆕 Phase 1-4: 统一投诉系统存储 ====================

    /// 投诉ID计数器
    #[pallet::storage]
    pub type NextComplaintId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 活跃投诉主存储
    #[pallet::storage]
    #[pallet::getter(fn complaints)]
    pub type Complaints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // complaint_id
        Complaint<T>,
        OptionQuery,
    >;

    /// 归档投诉存储
    #[pallet::storage]
    #[pallet::getter(fn archived_complaints)]
    pub type ArchivedComplaints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // complaint_id
        ArchivedComplaint,
        OptionQuery,
    >;

    /// 🆕 防膨胀: 归档仲裁清理游标
    #[pallet::storage]
    pub type ArchiveDisputeCleanupCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 🆕 防膨胀: 归档投诉清理游标
    #[pallet::storage]
    pub type ArchiveComplaintCleanupCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 用户活跃投诉索引（作为投诉人）
    #[pallet::storage]
    pub type UserActiveComplaints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<50>>, // 每用户最多50个活跃投诉
        ValueQuery,
    >;

    /// 投诉押金记录（complaint_id -> 押金金额）
    #[pallet::storage]
    pub type ComplaintDeposits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // complaint_id
        BalanceOf<T>,
        OptionQuery,
    >;

    /// 域统计信息
    #[pallet::storage]
    #[pallet::getter(fn domain_stats)]
    pub type DomainStats<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 8], // domain
        DomainStatistics,
        ValueQuery,
    >;

    /// 投诉归档游标
    #[pallet::storage]
    pub type ComplaintArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 🆕 AH4: 投诉过期扫描游标（避免全表扫描）
    #[pallet::storage]
    pub type ComplaintExpiryCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 🆕 F4: 被投诉人活跃投诉索引（作为被投诉方）
    #[pallet::storage]
    pub type RespondentActiveComplaints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, ConstU32<50>>,
        ValueQuery,
    >;

    /// 🆕 F6: 按业务对象查询投诉索引 (domain, object_id) => Vec<complaint_id>
    #[pallet::storage]
    pub type ObjectComplaints<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 8],
        Blake2_128Concat,
        u64,
        BoundedVec<u64, ConstU32<50>>,
        ValueQuery,
    >;

    /// 🆕 F7: 待裁决仲裁纠纷队列 (domain, object_id)
    #[pallet::storage]
    pub type PendingArbitrationDisputes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 8],
        Blake2_128Concat,
        u64,
        (),
        OptionQuery,
    >;

    /// 🆕 F7: 待裁决投诉队列
    #[pallet::storage]
    pub type PendingArbitrationComplaints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // complaint_id
        (),
        OptionQuery,
    >;

    /// 🆕 F11: 全局暂停开关
    #[pallet::storage]
    pub type Paused<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// 🆕 F13: 域惩罚比例动态配置 (domain => penalty_rate_bps)
    /// 治理可调，没有配置时回退到 ComplaintType::penalty_rate()
    #[pallet::storage]
    pub type DomainPenaltyRates<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 8],
        u16, // bps
        OptionQuery,
    >;

    // ============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 发起争议事件（含域）
        Disputed { domain: [u8; 8], id: u64 },
        /// 完成裁决事件（含域）
        Arbitrated {
            domain: [u8; 8],
            id: u64,
            decision: u8,
            bps: Option<u16>,
        },
        /// 🆕 发起纠纷并锁定押金
        DisputeWithDepositInitiated {
            domain: [u8; 8],
            id: u64,
            initiator: T::AccountId,
            respondent: T::AccountId,
            deposit: BalanceOf<T>,
            deadline: BlockNumberFor<T>,
        },
        /// 🆕 应诉方锁定押金
        RespondentDepositLocked {
            domain: [u8; 8],
            id: u64,
            respondent: T::AccountId,
            deposit: BalanceOf<T>,
        },
        /// 🆕 押金已处理（罚没或释放）
        DepositProcessed {
            domain: [u8; 8],
            id: u64,
            account: T::AccountId,
            released: BalanceOf<T>,
            slashed: BalanceOf<T>,
        },
        
        // ==================== 🆕 Phase 1-4: 统一投诉系统事件 ====================
        
        /// 投诉已提交
        ComplaintFiled {
            complaint_id: u64,
            domain: [u8; 8],
            object_id: u64,
            complainant: T::AccountId,
            respondent: T::AccountId,
            complaint_type: ComplaintType,
        },
        /// 投诉已响应/申诉
        ComplaintResponded {
            complaint_id: u64,
            respondent: T::AccountId,
        },
        /// 投诉已撤销
        ComplaintWithdrawn {
            complaint_id: u64,
        },
        /// 投诉已和解
        ComplaintSettled {
            complaint_id: u64,
        },
        /// 投诉已升级到仲裁
        ComplaintEscalated {
            complaint_id: u64,
        },
        /// 投诉已裁决
        ComplaintResolved {
            complaint_id: u64,
            decision: u8,
        },
        /// 投诉已过期
        ComplaintExpired {
            complaint_id: u64,
        },
        /// 投诉已归档
        ComplaintArchived {
            complaint_id: u64,
        },

        // ==================== 🆕 F1-F13: 新功能事件 ====================

        /// 🆕 F1: 缺席裁决
        DefaultJudgment {
            domain: [u8; 8],
            id: u64,
            initiator: T::AccountId,
        },
        /// 🆕 F2/F5: 投诉补充证据
        ComplaintEvidenceSupplemented {
            complaint_id: u64,
            who: T::AccountId,
            evidence_cid: BoundedVec<u8, T::MaxCidLen>,
        },
        /// 🆕 F3: 仲裁纠纷双方和解
        DisputeSettled {
            domain: [u8; 8],
            id: u64,
        },
        /// 🆕 F8: 投诉进入调解阶段
        ComplaintMediationStarted {
            complaint_id: u64,
        },
        /// 🆕 F9: 永久封禁执行
        AccountBanned {
            domain: [u8; 8],
            object_id: u64,
            account: T::AccountId,
        },
        /// 🆕 F10: 案件被驳回
        DisputeDismissed {
            domain: [u8; 8],
            id: u64,
        },
        /// 🆕 F10: 投诉被驳回
        ComplaintDismissed {
            complaint_id: u64,
        },
        /// 🆕 F11: 模块暂停/恢复
        PausedStateChanged {
            paused: bool,
        },
        /// 🆕 F12: 强制关闭纠纷
        DisputeForceClosed {
            domain: [u8; 8],
            id: u64,
        },
        /// 🆕 F12: 强制关闭投诉
        ComplaintForceClosed {
            complaint_id: u64,
        },
        /// 🆕 F13: 域惩罚比例已更新
        DomainPenaltyRateUpdated {
            domain: [u8; 8],
            rate_bps: Option<u16>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        AlreadyDisputed,
        NotDisputed,
        /// 🆕 押金不足
        InsufficientDeposit,
        /// 🆕 已经应诉
        AlreadyResponded,
        /// 🆕 应诉期已过
        ResponseDeadlinePassed,
        /// 🆕 无法获取对方账户
        CounterpartyNotFound,
        
        // ==================== 🆕 Phase 1-4: 统一投诉系统错误 ====================
        
        /// 投诉不存在
        ComplaintNotFound,
        /// 无权操作
        NotAuthorized,
        /// 无效的投诉类型（与域不匹配）
        InvalidComplaintType,
        /// 无效的状态转换
        InvalidState,
        /// 该对象投诉数量过多
        TooManyComplaints,
        /// 用户活跃投诉数量已达上限
        TooManyActiveComplaints,
        /// 🆕 M-NEW-6修复: 引用的证据不存在
        EvidenceNotFound,
        /// 🆕 F1: 应诉期未到，不能申请缺席裁决
        ResponseDeadlineNotReached,
        /// 🆕 F3: 双方和解需要对方确认
        SettlementNotConfirmed,
        /// 🆕 F11: 模块已暂停
        ModulePaused,
        /// 🆕 F13: 无效的惩罚比例
        InvalidPenaltyRate,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 发起仲裁：记录争议，证据 CID 存链（仅登记摘要/CID，不碰业务存储）
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::dispute(_evidence.len() as u32))]
        pub fn dispute(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
            _evidence: alloc::vec::Vec<BoundedVec<u8, T::MaxCidLen>>,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let _who = ensure_signed(origin)?;
            // 鉴权：由 Router 依据业务 pallet 规则判断是否允许发起（基准模式下跳过，便于构造场景）
            #[cfg(not(feature = "runtime-benchmarks"))]
            {
                ensure!(
                    T::Router::can_dispute(domain, &_who, id),
                    Error::<T>::NotDisputed
                );
            }
            ensure!(
                Disputed::<T>::get(domain, id).is_none(),
                Error::<T>::AlreadyDisputed
            );
            Disputed::<T>::insert(domain, id, ());
            // 证据仅留 CID；如需可扩展附加存储（MVP 省略内容）
            Self::deposit_event(Event::Disputed { domain, id });
            Ok(())
        }
        /// 仲裁者裁决（治理起源：Root/委员会）。
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::arbitrate())]
        pub fn arbitrate(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
            decision_code: u8,
            bps: Option<u16>,
        ) -> DispatchResult {
            // 函数级详细中文注释：仲裁裁决入口
            // - 安全：仅允许由治理起源触发（Root 或 内容委员会阈值），避免任意账户执行清算。
            // - 通过 runtime 注入的 DecisionOrigin 校验 origin。
            T::DecisionOrigin::ensure_origin(origin)?;
            ensure!(
                Disputed::<T>::get(domain, id).is_some(),
                Error::<T>::NotDisputed
            );
            // 通过 Router 将裁决应用到对应域的业务 pallet
            let decision = match (decision_code, bps) {
                (0, _) => Decision::Release,
                (1, _) => Decision::Refund,
                (2, Some(p)) => Decision::Partial(p),
                _ => Decision::Refund,
            };
            T::Router::apply_decision(domain, id, decision.clone())?;

            // 🆕 处理双向押金
            Self::handle_deposits_on_arbitration(domain, id, &decision)?;

            // 🆕 P2: 解锁仲裁期间锁定的证据 CID
            Self::unlock_all_evidence_cids(domain, id)?;

            // 🆕 信用分集成：根据裁决结果更新做市商信用分
            // - Release（做市商胜诉）：maker_win = true
            // - Refund/Partial（做市商败诉）：maker_win = false，扣除信用分
            if let Some(maker_id) = T::Router::get_maker_id(domain, id) {
                let maker_win = matches!(decision, Decision::Release);
                // 忽略错误，信用更新失败不影响主流程
                let _ = T::CreditUpdater::record_maker_dispute_result(maker_id, id, maker_win);
            }
            
            let out = match decision {
                Decision::Release => (0, None),
                Decision::Refund => (1, None),
                Decision::Partial(p) => (2, Some(p)),
            };

            // 🆕 F7: 从待裁决队列移除
            PendingArbitrationDisputes::<T>::remove(domain, id);

            // 🆕 归档已完成的仲裁并清理存储
            Self::archive_and_cleanup(domain, id, out.0, out.1.unwrap_or(0));

            Self::deposit_event(Event::Arbitrated {
                domain,
                id,
                decision: out.0,
                bps: out.1,
            });
            Ok(())
        }

        /// 函数级中文注释：以 evidence_id 的方式发起仲裁登记。
        /// - 适用场景：前端/当事人先调用 `pallet-evidence::commit` 获得 `evidence_id`，再把该 id 带入此函数，
        ///   从而实现“证据统一在 evidence 中存储与复用”，仲裁侧仅保存引用。
        /// - 行为：
        ///   1) 校验可发起（通过 Router.can_dispute）；2) 确保未被登记；3) 登记 Disputed；
        ///   4) 将 evidence_id 追加到本案的证据引用列表；5) 触发 Disputed 事件。
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::dispute(1))]
        pub fn dispute_with_evidence_id(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
            evidence_id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let _who = ensure_signed(origin)?;
            #[cfg(not(feature = "runtime-benchmarks"))]
            {
                ensure!(
                    T::Router::can_dispute(domain, &_who, id),
                    Error::<T>::NotDisputed
                );
            }
            ensure!(
                Disputed::<T>::get(domain, id).is_none(),
                Error::<T>::AlreadyDisputed
            );
            // 🆕 M-NEW-6修复: 验证 evidence_id 存在性
            ensure!(T::EvidenceExists::evidence_exists(evidence_id), Error::<T>::EvidenceNotFound);
            Disputed::<T>::insert(domain, id, ());
            EvidenceIds::<T>::try_mutate(domain, id, |v| -> Result<(), Error<T>> {
                v.try_push(evidence_id)
                    .map_err(|_| Error::<T>::AlreadyDisputed)?;
                Ok(())
            })?;
            Self::deposit_event(Event::Disputed { domain, id });
            Ok(())
        }

        /// 函数级中文注释：为已登记的仲裁案件追加一个 evidence_id 引用。
        /// - 适用场景：补充证据；证据本体由 `pallet-evidence` 统一存储。
        /// - 行为：
        ///   1) 确认本案已登记；2) 追加 evidence_id 到引用列表。
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::dispute(1))]
        pub fn append_evidence_id(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
            evidence_id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let _who = ensure_signed(origin)?;
            ensure!(
                Disputed::<T>::get(domain, id).is_some(),
                Error::<T>::NotDisputed
            );
            // 🆕 AH1修复: 校验调用者是否有权操作此案件
            #[cfg(not(feature = "runtime-benchmarks"))]
            {
                ensure!(
                    T::Router::can_dispute(domain, &_who, id),
                    Error::<T>::NotAuthorized
                );
            }
            // 🆕 M-NEW-6修复: 验证 evidence_id 存在性
            ensure!(T::EvidenceExists::evidence_exists(evidence_id), Error::<T>::EvidenceNotFound);
            EvidenceIds::<T>::try_mutate(domain, id, |v| -> Result<(), Error<T>> {
                v.try_push(evidence_id)
                    .map_err(|_| Error::<T>::AlreadyDisputed)?;
                Ok(())
            })?;
            Ok(())
        }

        /// 🆕 函数级中文注释：以双向押金方式发起纠纷
        /// - 从托管账户扣除押金（订单金额的15%）
        /// - 获取应诉方（卖家）信息
        /// - 设置应诉截止期限
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::dispute(1))]
        pub fn dispute_with_two_way_deposit(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
            evidence_id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let initiator = ensure_signed(origin)?;

            // 1. 权限校验
            #[cfg(not(feature = "runtime-benchmarks"))]
            {
                ensure!(
                    T::Router::can_dispute(domain, &initiator, id),
                    Error::<T>::NotDisputed
                );
            }

            // 2. 确保未被登记
            ensure!(
                Disputed::<T>::get(domain, id).is_none(),
                Error::<T>::AlreadyDisputed
            );

            // 2.5 验证 evidence_id 存在性（在任何状态变更之前）
            ensure!(T::EvidenceExists::evidence_exists(evidence_id), Error::<T>::EvidenceNotFound);

            // 3. 获取订单金额
            let order_amount = T::Router::get_order_amount(domain, id)
                .map_err(|_| Error::<T>::CounterpartyNotFound)?;

            // 4. 计算押金金额（订单金额的15%）
            // 修复 C-4: 使用 Permill 而非 Perbill，确保 bps * 100 = 百万分比
            // 例如: 1500 bps = 15%, Permill::from_parts(150000) = 15%
            let deposit_ratio_bps = T::DepositRatioBps::get();
            let deposit_amount = sp_runtime::Permill::from_parts((deposit_ratio_bps as u32) * 100)
                .mul_floor(order_amount);

            // 5. 检查托管余额是否足够
            let escrow_balance = T::Escrow::amount_of(id);
            ensure!(
                escrow_balance >= deposit_amount,
                Error::<T>::InsufficientDeposit
            );

            // 6. 获取托管账户并从托管账户锁定押金
            let escrow_account = Self::get_escrow_account();
            T::Fungible::hold(
                &T::RuntimeHoldReason::from(HoldReason::DisputeInitiator),
                &escrow_account,
                deposit_amount,
            )
            .map_err(|_| Error::<T>::InsufficientDeposit)?;

            // 7. 获取对方账户
            let respondent = T::Router::get_counterparty(domain, &initiator, id)
                .map_err(|_| Error::<T>::CounterpartyNotFound)?;

            // 8. 计算应诉截止期限
            let current_block = frame_system::Pallet::<T>::block_number();
            let deadline = current_block + T::ResponseDeadline::get();

            // 9. 登记纠纷和双向押金记录
            Disputed::<T>::insert(domain, id, ());
            TwoWayDeposits::<T>::insert(
                domain,
                id,
                TwoWayDepositRecord {
                    initiator: initiator.clone(),
                    initiator_deposit: deposit_amount,
                    respondent: respondent.clone(),
                    respondent_deposit: None,
                    response_deadline: deadline,
                    has_responded: false,
                },
            );

            // 10. 添加证据引用
            EvidenceIds::<T>::try_mutate(domain, id, |v| -> Result<(), Error<T>> {
                v.try_push(evidence_id)
                    .map_err(|_| Error::<T>::AlreadyDisputed)?;
                Ok(())
            })?;

            // 🆕 F7: 加入待裁决纠纷队列
            PendingArbitrationDisputes::<T>::insert(domain, id, ());

            // 11. 触发事件
            Self::deposit_event(Event::DisputeWithDepositInitiated {
                domain,
                id,
                initiator,
                respondent,
                deposit: deposit_amount,
                deadline,
            });

            Ok(())
        }

        /// 🆕 函数级中文注释：应诉方从托管锁定押金并提交反驳证据
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::dispute(1))]
        pub fn respond_to_dispute(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
            counter_evidence_id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let respondent = ensure_signed(origin)?;

            // 1. 获取押金记录
            let mut deposit_record = TwoWayDeposits::<T>::get(domain, id)
                .ok_or(Error::<T>::NotDisputed)?;

            // 2. 验证是应诉方
            ensure!(
                deposit_record.respondent == respondent,
                Error::<T>::NotDisputed
            );

            // 3. 确保未应诉
            ensure!(!deposit_record.has_responded, Error::<T>::AlreadyResponded);

            // 4. 检查是否超时
            let current_block = frame_system::Pallet::<T>::block_number();
            ensure!(
                current_block <= deposit_record.response_deadline,
                Error::<T>::ResponseDeadlinePassed
            );

            // 4.5 验证 counter_evidence_id 存在性（在任何状态变更之前）
            ensure!(T::EvidenceExists::evidence_exists(counter_evidence_id), Error::<T>::EvidenceNotFound);

            // 5. 计算押金金额（与发起方相同）
            let deposit_amount = deposit_record.initiator_deposit;

            // 6. 检查托管余额是否足够（应诉方也从托管扣押金）
            let escrow_balance = T::Escrow::amount_of(id);
            ensure!(
                escrow_balance >= deposit_amount,
                Error::<T>::InsufficientDeposit
            );

            // 7. 从托管账户锁定应诉方押金
            let escrow_account = Self::get_escrow_account();
            T::Fungible::hold(
                &T::RuntimeHoldReason::from(HoldReason::DisputeRespondent),
                &escrow_account,
                deposit_amount,
            )
            .map_err(|_| Error::<T>::InsufficientDeposit)?;

            // 8. 更新押金记录
            deposit_record.respondent_deposit = Some(deposit_amount);
            deposit_record.has_responded = true;
            TwoWayDeposits::<T>::insert(domain, id, deposit_record);

            // 9. 添加反驳证据
            EvidenceIds::<T>::try_mutate(domain, id, |v| -> Result<(), Error<T>> {
                v.try_push(counter_evidence_id)
                    .map_err(|_| Error::<T>::AlreadyDisputed)?;
                Ok(())
            })?;

            // 10. 触发事件
            Self::deposit_event(Event::RespondentDepositLocked {
                domain,
                id,
                respondent,
                deposit: deposit_amount,
            });

            Ok(())
        }

        // ==================== 🆕 Phase 1-4: 统一投诉系统 Extrinsics ====================

        /// 发起投诉（需缴纳押金防止恶意投诉）
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::file_complaint())]
        pub fn file_complaint(
            origin: OriginFor<T>,
            domain: [u8; 8],
            object_id: u64,
            complaint_type: ComplaintType,
            details_cid: BoundedVec<u8, T::MaxCidLen>,
            amount: Option<BalanceOf<T>>,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let complainant = ensure_signed(origin)?;

            // 1. 验证投诉权限
            #[cfg(not(feature = "runtime-benchmarks"))]
            ensure!(
                T::Router::can_dispute(domain, &complainant, object_id),
                Error::<T>::NotAuthorized
            );

            // 2. 获取被投诉人
            let respondent = T::Router::get_counterparty(domain, &complainant, object_id)
                .map_err(|_| Error::<T>::CounterpartyNotFound)?;

            // 3. 验证投诉类型与域匹配
            ensure!(
                complaint_type.domain() == domain || matches!(complaint_type, ComplaintType::Other),
                Error::<T>::InvalidComplaintType
            );

            // 3.5 锁定投诉押金（使用pricing换算1 USDT价值的COS）
            let min_deposit = T::ComplaintDeposit::get();
            let deposit_usd = T::ComplaintDepositUsd::get(); // 1_000_000 (1 USDT)
            
            let deposit_amount = if let Some(price) = T::Pricing::get_nex_to_usd_rate() {
                let price_u128: u128 = price.saturated_into();
                if price_u128 > 0u128 {
                    // COS数量 = USD金额 * 精度 / 价格
                    let required_u128 = (deposit_usd as u128).saturating_mul(1_000_000u128) / price_u128;
                    let required: BalanceOf<T> = required_u128.saturated_into();
                    // 取换算值和兜底值中的较大者
                    if required > min_deposit { required } else { min_deposit }
                } else {
                    min_deposit
                }
            } else {
                min_deposit
            };
            // 注：价格陈旧性由上游 TradingPricingProvider（TWAP 优先）保障，
            // 且 min_deposit 兜底确保即使价格异常也不会产生零押金。
            
            T::Fungible::hold(
                &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                &complainant,
                deposit_amount,
            ).map_err(|_| Error::<T>::InsufficientDeposit)?;

            // 4. 生成投诉ID
            let complaint_id = NextComplaintId::<T>::mutate(|id| {
                let current = *id;
                *id = id.saturating_add(1);
                current
            });

            // 4.5 记录押金
            ComplaintDeposits::<T>::insert(complaint_id, deposit_amount);

            // 5. 计算响应截止时间
            let now = frame_system::Pallet::<T>::block_number();
            let deadline = now + T::ResponseDeadline::get();

            // 6. 创建投诉记录
            let complaint = Complaint {
                id: complaint_id,
                domain,
                object_id,
                complaint_type: complaint_type.clone(),
                complainant: complainant.clone(),
                respondent: respondent.clone(),
                details_cid,
                response_cid: None,
                amount,
                status: ComplaintStatus::Submitted,
                created_at: now,
                response_deadline: deadline,
                settlement_cid: None,
                resolution_cid: None,
                updated_at: now,
            };

            // 7. 存储
            Complaints::<T>::insert(complaint_id, &complaint);

            // 8. 更新用户索引
            UserActiveComplaints::<T>::try_mutate(&complainant, |list| {
                list.try_push(complaint_id)
            }).map_err(|_| Error::<T>::TooManyActiveComplaints)?;

            // 🆕 F4: 更新被投诉人索引
            RespondentActiveComplaints::<T>::try_mutate(&respondent, |list| {
                list.try_push(complaint_id)
            }).map_err(|_| Error::<T>::TooManyActiveComplaints)?;

            // 🆕 F6: 更新对象投诉索引
            ObjectComplaints::<T>::try_mutate(domain, object_id, |list| {
                list.try_push(complaint_id)
            }).map_err(|_| Error::<T>::TooManyComplaints)?;

            // 9. 更新域统计
            DomainStats::<T>::mutate(domain, |stats| {
                stats.total_complaints = stats.total_complaints.saturating_add(1);
            });

            // 10. 触发事件
            Self::deposit_event(Event::ComplaintFiled {
                complaint_id,
                domain,
                object_id,
                complainant,
                respondent,
                complaint_type,
            });

            Ok(())
        }

        /// 响应/申诉投诉
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::respond_to_complaint())]
        pub fn respond_to_complaint(
            origin: OriginFor<T>,
            complaint_id: u64,
            response_cid: BoundedVec<u8, T::MaxCidLen>,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let respondent = ensure_signed(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                // 验证身份
                ensure!(complaint.respondent == respondent, Error::<T>::NotAuthorized);

                // 验证状态
                ensure!(
                    complaint.status == ComplaintStatus::Submitted,
                    Error::<T>::InvalidState
                );

                // 验证未过期
                let now = frame_system::Pallet::<T>::block_number();
                ensure!(
                    now <= complaint.response_deadline,
                    Error::<T>::ResponseDeadlinePassed
                );

                // 🆕 A2修复: 使用独立的 response_cid 字段，保留原始投诉详情
                complaint.response_cid = Some(response_cid);
                complaint.status = ComplaintStatus::Responded;
                complaint.updated_at = now;

                Self::deposit_event(Event::ComplaintResponded {
                    complaint_id,
                    respondent,
                });

                Ok(())
            })
        }

        /// 撤销投诉
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_complaint())]
        pub fn withdraw_complaint(
            origin: OriginFor<T>,
            complaint_id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                ensure!(complaint.complainant == who, Error::<T>::NotAuthorized);

                ensure!(
                    matches!(complaint.status, ComplaintStatus::Submitted | ComplaintStatus::Responded),
                    Error::<T>::InvalidState
                );

                let now = frame_system::Pallet::<T>::block_number();
                complaint.status = ComplaintStatus::Withdrawn;
                complaint.updated_at = now;

                // 🆕 AH5修复: 撤诉时退还投诉押金
                if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
                    let _ = T::Fungible::release(
                        &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                        &complaint.complainant,
                        deposit_amount,
                        frame_support::traits::tokens::Precision::BestEffort,
                    );
                }

                // 立即清理用户索引（不等归档）
                Self::remove_from_user_complaint_index(&complaint.complainant, complaint_id);
                Self::remove_from_respondent_complaint_index(&complaint.respondent, complaint_id);
                Self::remove_from_object_complaint_index(complaint.domain, complaint.object_id, complaint_id);

                Self::deposit_event(Event::ComplaintWithdrawn { complaint_id });

                Ok(())
            })
        }

        /// 达成和解
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::settle_complaint())]
        pub fn settle_complaint(
            origin: OriginFor<T>,
            complaint_id: u64,
            settlement_cid: BoundedVec<u8, T::MaxCidLen>,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                // 验证是当事人
                ensure!(
                    complaint.complainant == who || complaint.respondent == who,
                    Error::<T>::NotAuthorized
                );

                // 验证状态
                ensure!(
                    matches!(complaint.status, ComplaintStatus::Responded | ComplaintStatus::Mediating),
                    Error::<T>::InvalidState
                );

                // 🆕 H2-R2修复: 调解中仅投诉方可和解（防止被投诉方单方面关闭调解绕过仲裁）
                if complaint.status == ComplaintStatus::Mediating {
                    ensure!(complaint.complainant == who, Error::<T>::NotAuthorized);
                }

                // 🆕 M-NEW-3修复: 使用独立字段存储和解详情，保留原始 details_cid
                let now = frame_system::Pallet::<T>::block_number();
                complaint.settlement_cid = Some(settlement_cid);
                complaint.status = ComplaintStatus::ResolvedSettlement;
                complaint.updated_at = now;

                // 🆕 AH6修复: 和解时退还投诉押金
                if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
                    let _ = T::Fungible::release(
                        &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                        &complaint.complainant,
                        deposit_amount,
                        frame_support::traits::tokens::Precision::BestEffort,
                    );
                }

                // 更新统计
                DomainStats::<T>::mutate(complaint.domain, |stats| {
                    stats.resolved_count = stats.resolved_count.saturating_add(1);
                    stats.settlements = stats.settlements.saturating_add(1);
                });

                // 立即清理用户索引（不等归档）
                Self::remove_from_user_complaint_index(&complaint.complainant, complaint_id);
                Self::remove_from_respondent_complaint_index(&complaint.respondent, complaint_id);
                Self::remove_from_object_complaint_index(complaint.domain, complaint.object_id, complaint_id);

                Self::deposit_event(Event::ComplaintSettled { complaint_id });

                Ok(())
            })
        }

        /// 提交仲裁（升级到仲裁委员会）
        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::escalate_to_arbitration())]
        pub fn escalate_to_arbitration(
            origin: OriginFor<T>,
            complaint_id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                ensure!(
                    complaint.complainant == who || complaint.respondent == who,
                    Error::<T>::NotAuthorized
                );

                ensure!(
                    matches!(complaint.status, ComplaintStatus::Responded | ComplaintStatus::Mediating),
                    Error::<T>::InvalidState
                );

                let now = frame_system::Pallet::<T>::block_number();
                complaint.status = ComplaintStatus::Arbitrating;
                complaint.updated_at = now;

                // 🆕 F7: 加入待裁决投诉队列
                PendingArbitrationComplaints::<T>::insert(complaint_id, ());

                Self::deposit_event(Event::ComplaintEscalated { complaint_id });

                Ok(())
            })
        }

        /// 仲裁裁决（仅仲裁委员会/Root）
        #[pallet::call_index(15)]
        #[pallet::weight(<T as Config>::WeightInfo::arbitrate())]
        pub fn resolve_complaint(
            origin: OriginFor<T>,
            complaint_id: u64,
            decision: u8, // 0=投诉方胜, 1=被投诉方胜, 2=部分裁决
            reason_cid: BoundedVec<u8, T::MaxCidLen>,
            // 🆕 L-5修复: 可选自定义分账比例（基点，仅 decision==2 时生效）
            partial_bps: Option<u16>,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                ensure!(
                    complaint.status == ComplaintStatus::Arbitrating,
                    Error::<T>::InvalidState
                );

                // 🆕 L-5修复: 支持自定义分账比例，默认使用 PartialSlashBps 配置值
                let router_decision = match decision {
                    0 => Decision::Refund,      // 投诉方胜诉 = 退款
                    1 => Decision::Release,     // 被投诉方胜诉 = 释放
                    _ => {
                        let bps = partial_bps.unwrap_or(T::PartialSlashBps::get());
                        Decision::Partial(bps.min(10_000))
                    },
                };
                T::Router::apply_decision(complaint.domain, complaint.object_id, router_decision)?;

                // 🆕 M-NEW-3修复: 使用独立字段存储裁决详情，保留原始 details_cid
                let now = frame_system::Pallet::<T>::block_number();
                complaint.resolution_cid = Some(reason_cid);
                complaint.status = match decision {
                    0 => ComplaintStatus::ResolvedComplainantWin,
                    1 => ComplaintStatus::ResolvedRespondentWin,
                    _ => ComplaintStatus::ResolvedSettlement,
                };
                complaint.updated_at = now;

                // 处理投诉押金
                match decision {
                    1 => {
                        // 被投诉方胜诉：罚没部分押金给被投诉方
                        Self::slash_complaint_deposit(
                            complaint_id,
                            &complaint.complainant,
                            &complaint.respondent,
                            complaint.domain,
                            &complaint.complaint_type,
                        );
                    },
                    _ => {
                        // 投诉方胜诉 / 和解：全额退还押金
                        if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
                            let _ = T::Fungible::release(
                                &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                                &complaint.complainant,
                                deposit_amount,
                                frame_support::traits::tokens::Precision::BestEffort,
                            );
                        }
                    }
                }

                // 🆕 F9: 投诉方胜诉且投诉类型触发永久封禁
                if decision == 0 && complaint.complaint_type.triggers_permanent_ban() {
                    let _ = T::Router::ban_account(
                        complaint.domain,
                        &complaint.respondent,
                        complaint.object_id,
                    );
                    Self::deposit_event(Event::AccountBanned {
                        domain: complaint.domain,
                        object_id: complaint.object_id,
                        account: complaint.respondent.clone(),
                    });
                }

                // 🆕 F7: 从待裁决队列移除
                PendingArbitrationComplaints::<T>::remove(complaint_id);

                // 更新统计
                // M-2修复: 部分裁决(decision>=2)计入 complainant_wins（偏向投诉方），
                // settlements 仅统计双方自愿和解
                DomainStats::<T>::mutate(complaint.domain, |stats| {
                    stats.resolved_count = stats.resolved_count.saturating_add(1);
                    match decision {
                        0 | 2.. => stats.complainant_wins = stats.complainant_wins.saturating_add(1),
                        1 => stats.respondent_wins = stats.respondent_wins.saturating_add(1),
                    }
                });

                // 立即清理用户索引（不等归档）
                Self::remove_from_user_complaint_index(&complaint.complainant, complaint_id);
                Self::remove_from_respondent_complaint_index(&complaint.respondent, complaint_id);
                Self::remove_from_object_complaint_index(complaint.domain, complaint.object_id, complaint_id);

                Self::deposit_event(Event::ComplaintResolved {
                    complaint_id,
                    decision,
                });

                Ok(())
            })
        }

        // ==================== 🆕 F1-F13: 新功能 Extrinsics ====================

        /// 🆕 F1: 缺席裁决请求 — 应诉方超时未应诉时，发起方可请求缺席裁决（自动 Refund）
        #[pallet::call_index(20)]
        #[pallet::weight(<T as Config>::WeightInfo::request_default_judgment())]
        pub fn request_default_judgment(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            let deposit_record = TwoWayDeposits::<T>::get(domain, id)
                .ok_or(Error::<T>::NotDisputed)?;

            // 只有发起方可以请求缺席裁决
            ensure!(deposit_record.initiator == who, Error::<T>::NotAuthorized);

            // 确保应诉方未应诉
            ensure!(!deposit_record.has_responded, Error::<T>::AlreadyResponded);

            // 确保已过应诉截止期限
            let current_block = frame_system::Pallet::<T>::block_number();
            ensure!(
                current_block > deposit_record.response_deadline,
                Error::<T>::ResponseDeadlineNotReached
            );

            // 自动执行 Refund（买家胜诉）
            let decision = Decision::Refund;
            T::Router::apply_decision(domain, id, decision.clone())?;

            // 处理押金：发起方押金全额返还，应诉方无押金（未应诉）
            Self::handle_deposits_on_arbitration(domain, id, &decision)?;

            // 解锁证据 CID
            Self::unlock_all_evidence_cids(domain, id)?;

            // 信用分更新
            if let Some(maker_id) = T::Router::get_maker_id(domain, id) {
                let _ = T::CreditUpdater::record_maker_dispute_result(maker_id, id, false);
            }

            // 从待裁决队列移除
            PendingArbitrationDisputes::<T>::remove(domain, id);

            // 归档
            Self::archive_and_cleanup(domain, id, 1, 0); // 1 = Refund

            Self::deposit_event(Event::DefaultJudgment {
                domain,
                id,
                initiator: who,
            });
            Ok(())
        }

        /// 🆕 F2: 投诉人补充证据
        #[pallet::call_index(21)]
        #[pallet::weight(<T as Config>::WeightInfo::supplement_evidence())]
        pub fn supplement_complaint_evidence(
            origin: OriginFor<T>,
            complaint_id: u64,
            evidence_cid: BoundedVec<u8, T::MaxCidLen>,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            let complaint = Complaints::<T>::get(complaint_id)
                .ok_or(Error::<T>::ComplaintNotFound)?;

            // 投诉人才能补充投诉证据
            ensure!(complaint.complainant == who, Error::<T>::NotAuthorized);

            // 仅在活跃状态下可补充
            ensure!(
                matches!(complaint.status,
                    ComplaintStatus::Submitted | ComplaintStatus::Responded |
                    ComplaintStatus::Mediating | ComplaintStatus::Arbitrating
                ),
                Error::<T>::InvalidState
            );

            Self::deposit_event(Event::ComplaintEvidenceSupplemented {
                complaint_id,
                who,
                evidence_cid,
            });

            Ok(())
        }

        /// 🆕 F5: 应诉方补充证据
        #[pallet::call_index(22)]
        #[pallet::weight(<T as Config>::WeightInfo::supplement_evidence())]
        pub fn supplement_response_evidence(
            origin: OriginFor<T>,
            complaint_id: u64,
            evidence_cid: BoundedVec<u8, T::MaxCidLen>,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            let complaint = Complaints::<T>::get(complaint_id)
                .ok_or(Error::<T>::ComplaintNotFound)?;

            // 被投诉方才能补充应诉证据
            ensure!(complaint.respondent == who, Error::<T>::NotAuthorized);

            // 仅在活跃状态下可补充
            ensure!(
                matches!(complaint.status,
                    ComplaintStatus::Responded | ComplaintStatus::Mediating |
                    ComplaintStatus::Arbitrating
                ),
                Error::<T>::InvalidState
            );

            Self::deposit_event(Event::ComplaintEvidenceSupplemented {
                complaint_id,
                who,
                evidence_cid,
            });

            Ok(())
        }

        /// 🆕 F3: 仲裁纠纷双方和解 — 发起方请求和解，释放双方押金
        #[pallet::call_index(23)]
        #[pallet::weight(<T as Config>::WeightInfo::settle_dispute())]
        pub fn settle_dispute(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
        ) -> DispatchResult {
            ensure!(!Paused::<T>::get(), Error::<T>::ModulePaused);
            let who = ensure_signed(origin)?;

            // 确保纠纷存在
            ensure!(
                Disputed::<T>::get(domain, id).is_some(),
                Error::<T>::NotDisputed
            );

            let deposit_record = TwoWayDeposits::<T>::get(domain, id)
                .ok_or(Error::<T>::NotDisputed)?;

            // 必须是当事人
            ensure!(
                deposit_record.initiator == who || deposit_record.respondent == who,
                Error::<T>::NotAuthorized
            );

            // 双方都已参与（应诉方必须已锁定押金才能和解）
            ensure!(deposit_record.has_responded, Error::<T>::SettlementNotConfirmed);

            // 全额释放双方押金（和解无罚没）
            let escrow_account = Self::get_escrow_account();
            Self::release_deposit(
                &escrow_account,
                deposit_record.initiator_deposit,
                &HoldReason::DisputeInitiator,
                domain, id,
            )?;
            if let Some(respondent_deposit) = deposit_record.respondent_deposit {
                Self::release_deposit(
                    &escrow_account,
                    respondent_deposit,
                    &HoldReason::DisputeRespondent,
                    domain, id,
                )?;
            }

            // 解锁证据 CID
            Self::unlock_all_evidence_cids(domain, id)?;

            // 从待裁决队列移除
            PendingArbitrationDisputes::<T>::remove(domain, id);

            // 清理存储（不归档统计，和解不算正式裁决）
            Disputed::<T>::remove(domain, id);
            EvidenceIds::<T>::remove(domain, id);
            TwoWayDeposits::<T>::remove(domain, id);

            Self::deposit_event(Event::DisputeSettled { domain, id });
            Ok(())
        }

        /// 🆕 F8: 启动调解 — 仲裁委员会将投诉转入调解阶段
        #[pallet::call_index(24)]
        #[pallet::weight(<T as Config>::WeightInfo::start_mediation())]
        pub fn start_mediation(
            origin: OriginFor<T>,
            complaint_id: u64,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                ensure!(
                    complaint.status == ComplaintStatus::Responded,
                    Error::<T>::InvalidState
                );

                let now = frame_system::Pallet::<T>::block_number();
                complaint.status = ComplaintStatus::Mediating;
                complaint.updated_at = now;

                Self::deposit_event(Event::ComplaintMediationStarted { complaint_id });

                Ok(())
            })
        }

        /// 🆕 F10: 驳回无效纠纷 — 仲裁委员会驳回并罚没发起方押金
        #[pallet::call_index(25)]
        #[pallet::weight(<T as Config>::WeightInfo::dismiss_dispute())]
        pub fn dismiss_dispute(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            ensure!(
                Disputed::<T>::get(domain, id).is_some(),
                Error::<T>::NotDisputed
            );

            // 驳回 = 发起方败诉（Release），罚没发起方押金
            let decision = Decision::Release;
            T::Router::apply_decision(domain, id, decision.clone())?;
            Self::handle_deposits_on_arbitration(domain, id, &decision)?;
            Self::unlock_all_evidence_cids(domain, id)?;

            // 从待裁决队列移除
            PendingArbitrationDisputes::<T>::remove(domain, id);

            // 归档
            Self::archive_and_cleanup(domain, id, 0, 0); // 0 = Release

            Self::deposit_event(Event::DisputeDismissed { domain, id });
            Ok(())
        }

        /// 🆕 F10: 驳回无效投诉 — 仲裁委员会驳回并罚没投诉人押金
        #[pallet::call_index(26)]
        #[pallet::weight(<T as Config>::WeightInfo::dismiss_complaint())]
        pub fn dismiss_complaint(
            origin: OriginFor<T>,
            complaint_id: u64,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                // 只能驳回活跃状态的投诉
                ensure!(
                    !complaint.status.is_resolved(),
                    Error::<T>::InvalidState
                );

                let now = frame_system::Pallet::<T>::block_number();
                complaint.status = ComplaintStatus::ResolvedRespondentWin;
                complaint.updated_at = now;

                // 罚没投诉人押金（与被投诉方胜诉相同逻辑）
                Self::slash_complaint_deposit(
                    complaint_id,
                    &complaint.complainant,
                    &complaint.respondent,
                    complaint.domain,
                    &complaint.complaint_type,
                );

                // 从待裁决队列移除
                PendingArbitrationComplaints::<T>::remove(complaint_id);

                // 更新统计
                DomainStats::<T>::mutate(complaint.domain, |stats| {
                    stats.resolved_count = stats.resolved_count.saturating_add(1);
                    stats.respondent_wins = stats.respondent_wins.saturating_add(1);
                });

                // 立即清理用户索引（不等归档）
                Self::remove_from_user_complaint_index(&complaint.complainant, complaint_id);
                Self::remove_from_respondent_complaint_index(&complaint.respondent, complaint_id);
                Self::remove_from_object_complaint_index(complaint.domain, complaint.object_id, complaint_id);

                Self::deposit_event(Event::ComplaintDismissed { complaint_id });

                Ok(())
            })
        }

        /// 🆕 F11: 紧急暂停/恢复模块（仅 Root/治理）
        #[pallet::call_index(27)]
        #[pallet::weight(Weight::from_parts(10_000_000, 1_000))]
        pub fn set_paused(
            origin: OriginFor<T>,
            paused: bool,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;
            Paused::<T>::put(paused);
            Self::deposit_event(Event::PausedStateChanged { paused });
            Ok(())
        }

        /// 🆕 F12: 强制关闭卡住的纠纷（仅 Root/治理）— 释放所有押金
        #[pallet::call_index(28)]
        #[pallet::weight(<T as Config>::WeightInfo::force_close_dispute())]
        pub fn force_close_dispute(
            origin: OriginFor<T>,
            domain: [u8; 8],
            id: u64,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            ensure!(
                Disputed::<T>::get(domain, id).is_some(),
                Error::<T>::NotDisputed
            );

            // 释放所有押金（无罚没）
            if let Some(deposit_record) = TwoWayDeposits::<T>::take(domain, id) {
                let escrow_account = Self::get_escrow_account();
                let _ = Self::release_deposit(
                    &escrow_account,
                    deposit_record.initiator_deposit,
                    &HoldReason::DisputeInitiator,
                    domain, id,
                );
                if let Some(respondent_deposit) = deposit_record.respondent_deposit {
                    let _ = Self::release_deposit(
                        &escrow_account,
                        respondent_deposit,
                        &HoldReason::DisputeRespondent,
                        domain, id,
                    );
                }
            }

            // 解锁证据 CID
            let _ = Self::unlock_all_evidence_cids(domain, id);

            // 从待裁决队列移除
            PendingArbitrationDisputes::<T>::remove(domain, id);

            // 清理存储
            Disputed::<T>::remove(domain, id);
            EvidenceIds::<T>::remove(domain, id);

            Self::deposit_event(Event::DisputeForceClosed { domain, id });
            Ok(())
        }

        /// 🆕 F12: 强制关闭卡住的投诉（仅 Root/治理）— 退还投诉押金
        #[pallet::call_index(29)]
        #[pallet::weight(<T as Config>::WeightInfo::force_close_complaint())]
        pub fn force_close_complaint(
            origin: OriginFor<T>,
            complaint_id: u64,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            Complaints::<T>::try_mutate(complaint_id, |maybe_complaint| -> DispatchResult {
                let complaint = maybe_complaint.as_mut().ok_or(Error::<T>::ComplaintNotFound)?;

                // 只能关闭未解决的投诉
                ensure!(
                    !complaint.status.is_resolved(),
                    Error::<T>::InvalidState
                );

                let now = frame_system::Pallet::<T>::block_number();
                complaint.status = ComplaintStatus::Withdrawn;
                complaint.updated_at = now;

                // 退还押金
                if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
                    let _ = T::Fungible::release(
                        &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                        &complaint.complainant,
                        deposit_amount,
                        frame_support::traits::tokens::Precision::BestEffort,
                    );
                }

                // 从待裁决队列移除
                PendingArbitrationComplaints::<T>::remove(complaint_id);

                // 🆕 H1-R2修复: 立即清理用户索引（与其他解决路径一致）
                Self::remove_from_user_complaint_index(&complaint.complainant, complaint_id);
                Self::remove_from_respondent_complaint_index(&complaint.respondent, complaint_id);
                Self::remove_from_object_complaint_index(complaint.domain, complaint.object_id, complaint_id);

                Self::deposit_event(Event::ComplaintForceClosed { complaint_id });

                Ok(())
            })
        }

        /// 🆕 F13: 动态设置域惩罚比例（仅 Root/治理）
        #[pallet::call_index(30)]
        #[pallet::weight(Weight::from_parts(10_000_000, 1_000))]
        pub fn set_domain_penalty_rate(
            origin: OriginFor<T>,
            domain: [u8; 8],
            rate_bps: Option<u16>,
        ) -> DispatchResult {
            T::DecisionOrigin::ensure_origin(origin)?;

            if let Some(rate) = rate_bps {
                ensure!(rate <= 10_000, Error::<T>::InvalidPenaltyRate);
                DomainPenaltyRates::<T>::insert(domain, rate);
            } else {
                DomainPenaltyRates::<T>::remove(domain);
            }

            Self::deposit_event(Event::DomainPenaltyRateUpdated { domain, rate_bps });
            Ok(())
        }
    }

    /// 🆕 辅助函数实现
    impl<T: Config> Pallet<T> {
        /// 函数级中文注释：获取托管账户
        /// - 通过 Escrow trait 获取，解耦对 pallet_escrow::Config 的依赖
        fn get_escrow_account() -> T::AccountId {
            T::Escrow::escrow_account()
        }
        /// 函数级中文注释：仲裁时处理双向押金
        /// - Release: 买家败诉，罚没买家押金30%，卖家押金全额返还到托管
        /// - Refund: 卖家败诉，罚没卖家押金30%，买家押金全额返还到托管
        /// - Partial: 双方都有责任，各罚没50%
        ///
        /// 注意：所有押金操作都在托管账户上进行
        fn handle_deposits_on_arbitration(
            domain: [u8; 8],
            id: u64,
            decision: &Decision,
        ) -> DispatchResult {
            if let Some(deposit_record) = TwoWayDeposits::<T>::take(domain, id) {
                let treasury = T::TreasuryAccount::get();
                let escrow_account = Self::get_escrow_account();

                match decision {
                    Decision::Release => {
                        // 卖家胜诉：买家押金罚没30%，卖家押金全额返还到托管
                        Self::slash_and_release(
                            &escrow_account,
                            deposit_record.initiator_deposit,
                            T::RejectedSlashBps::get(),
                            &HoldReason::DisputeInitiator,
                            &treasury,
                            domain, id,
                        )?;

                        if let Some(respondent_deposit) = deposit_record.respondent_deposit {
                            Self::release_deposit(
                                &escrow_account,
                                respondent_deposit,
                                &HoldReason::DisputeRespondent,
                                domain, id,
                            )?;
                        }
                    }
                    Decision::Refund => {
                        // 买家胜诉：买家押金全额返还到托管，卖家押金罚没30%
                        Self::release_deposit(
                            &escrow_account,
                            deposit_record.initiator_deposit,
                            &HoldReason::DisputeInitiator,
                            domain, id,
                        )?;

                        if let Some(respondent_deposit) = deposit_record.respondent_deposit {
                            Self::slash_and_release(
                                &escrow_account,
                                respondent_deposit,
                                T::RejectedSlashBps::get(),
                                &HoldReason::DisputeRespondent,
                                &treasury,
                                domain, id,
                            )?;
                        }
                    }
                    Decision::Partial(_) => {
                        // 部分胜诉：双方各罚没50%
                        Self::slash_and_release(
                            &escrow_account,
                            deposit_record.initiator_deposit,
                            T::PartialSlashBps::get(),
                            &HoldReason::DisputeInitiator,
                            &treasury,
                            domain, id,
                        )?;

                        if let Some(respondent_deposit) = deposit_record.respondent_deposit {
                            Self::slash_and_release(
                                &escrow_account,
                                respondent_deposit,
                                T::PartialSlashBps::get(),
                                &HoldReason::DisputeRespondent,
                                &treasury,
                                domain, id,
                            )?;
                        }
                    }
                }
            }
            Ok(())
        }

        /// 函数级中文注释：罚没并释放押金
        /// - slash_bps: 罚没比例（基点，如 3000 = 30%）
        /// 🆕 AH3修复: 传入 domain/id 以准确记录事件
        fn slash_and_release(
            account: &T::AccountId,
            amount: BalanceOf<T>,
            slash_bps: u16,
            hold_reason: &HoldReason,
            treasury: &T::AccountId,
            domain: [u8; 8],
            object_id: u64,
        ) -> DispatchResult {
            use sp_runtime::traits::Zero;

            let slash_amount = sp_runtime::Permill::from_parts((slash_bps as u32) * 100)
                .mul_floor(amount);
            let release_amount = amount.saturating_sub(slash_amount);

            // 罚没部分转入国库
            if !slash_amount.is_zero() {
                T::Fungible::transfer_on_hold(
                    &T::RuntimeHoldReason::from(hold_reason.clone()),
                    account,
                    treasury,
                    slash_amount,
                    frame_support::traits::tokens::Precision::BestEffort,
                    frame_support::traits::tokens::Restriction::Free,
                    frame_support::traits::tokens::Fortitude::Force,
                )?;
            }

            // 释放剩余部分
            if !release_amount.is_zero() {
                T::Fungible::release(
                    &T::RuntimeHoldReason::from(hold_reason.clone()),
                    account,
                    release_amount,
                    frame_support::traits::tokens::Precision::Exact,
                )?;
            }

            Self::deposit_event(Event::DepositProcessed {
                domain,
                id: object_id,
                account: account.clone(),
                released: release_amount,
                slashed: slash_amount,
            });

            Ok(())
        }

        /// 函数级中文注释：全额释放押金（无罚没）
        fn release_deposit(
            account: &T::AccountId,
            amount: BalanceOf<T>,
            hold_reason: &HoldReason,
            domain: [u8; 8],
            object_id: u64,
        ) -> DispatchResult {
            use sp_runtime::traits::Zero;

            T::Fungible::release(
                &T::RuntimeHoldReason::from(hold_reason.clone()),
                account,
                amount,
                frame_support::traits::tokens::Precision::Exact,
            )?;

            Self::deposit_event(Event::DepositProcessed {
                domain,
                id: object_id,
                account: account.clone(),
                released: amount,
                slashed: BalanceOf::<T>::zero(),
            });

            Ok(())
        }

        /// 投诉押金罚没：将 slash_bps 部分转给被投诉方，退还剩余
        /// 罚没比例优先级: DomainPenaltyRates > ComplaintType::penalty_rate() > ComplaintSlashBps
        fn slash_complaint_deposit(
            complaint_id: u64,
            complainant: &T::AccountId,
            respondent: &T::AccountId,
            domain: [u8; 8],
            complaint_type: &ComplaintType,
        ) {
            if let Some(deposit_amount) = ComplaintDeposits::<T>::take(complaint_id) {
                let slash_bps = DomainPenaltyRates::<T>::get(domain)
                    .unwrap_or_else(|| complaint_type.penalty_rate());
                let slash_amount = sp_runtime::Permill::from_parts((slash_bps as u32) * 100)
                    .mul_floor(deposit_amount);
                let return_amount = deposit_amount.saturating_sub(slash_amount);

                if !slash_amount.is_zero() {
                    let _ = T::Fungible::transfer_on_hold(
                        &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                        complainant,
                        respondent,
                        slash_amount,
                        frame_support::traits::tokens::Precision::BestEffort,
                        frame_support::traits::tokens::Restriction::Free,
                        frame_support::traits::tokens::Fortitude::Polite,
                    );
                }
                if !return_amount.is_zero() {
                    let _ = T::Fungible::release(
                        &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                        complainant,
                        return_amount,
                        frame_support::traits::tokens::Precision::BestEffort,
                    );
                }
            }
        }

        // ============================================================================
        // 🆕 P2: CID 锁定管理辅助函数
        // ============================================================================

        /// 函数级中文注释：锁定仲裁案件相关的 CID
        /// 
        /// 参数：
        /// - domain: 业务域
        /// - id: 案件ID
        /// - cid_hash: 要锁定的 CID 哈希
        /// 
        /// 说明：
        /// - 锁定原因格式为 "arb:{domain_hex}:{id}"
        /// - 锁定时间为永久（直到仲裁完成）
        pub fn lock_evidence_cid(
            domain: [u8; 8],
            id: u64,
            cid_hash: T::Hash,
        ) -> DispatchResult {
            // 构建锁定原因
            let reason = Self::build_lock_reason(domain, id);
            
            // 调用 CidLockManager 锁定
            T::CidLockManager::lock_cid(cid_hash, reason, None)?;
            
            // 记录到本地存储
            // L-2修复: 使用语义正确的错误（BoundedVec已满 ≠ 已登记纠纷）
            LockedCidHashes::<T>::try_mutate(domain, id, |hashes| -> Result<(), DispatchError> {
                hashes.try_push(cid_hash)
                    .map_err(|_| Error::<T>::TooManyComplaints)?;
                Ok(())
            })?;
            
            Ok(())
        }

        /// 函数级中文注释：解锁仲裁案件相关的所有 CID
        /// 
        /// 参数：
        /// - domain: 业务域
        /// - id: 案件ID
        /// 
        /// 说明：
        /// - 仲裁完成后自动调用
        /// - 解锁所有在 LockedCidHashes 中记录的 CID
        pub fn unlock_all_evidence_cids(domain: [u8; 8], id: u64) -> DispatchResult {
            let reason = Self::build_lock_reason(domain, id);
            let locked_hashes = LockedCidHashes::<T>::take(domain, id);
            
            for cid_hash in locked_hashes.iter() {
                // 忽略解锁失败（可能已被其他原因解锁或不存在）
                let _ = T::CidLockManager::unlock_cid(*cid_hash, reason.clone());
            }
            
            Ok(())
        }

        /// 函数级中文注释：构建锁定原因字符串
        fn build_lock_reason(domain: [u8; 8], id: u64) -> alloc::vec::Vec<u8> {
            // 格式: "arb:{domain_hex}:{id}"
            let mut reason = b"arb:".to_vec();
            reason.extend_from_slice(&domain);
            reason.push(b':');
            reason.extend_from_slice(&id.to_le_bytes());
            reason
        }

        /// 🆕 存储膨胀防护：归档仲裁并清理存储
        fn archive_and_cleanup(domain: [u8; 8], id: u64, decision: u8, partial_bps: u16) {
            let current_block: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
            
            // 创建归档记录
            let archived = ArchivedDispute {
                domain,
                object_id: id,
                decision,
                partial_bps,
                completed_at: current_block,
                year_month: block_to_year_month(current_block, 14400),
            };

            // 保存归档记录
            let archived_id = NextArchivedId::<T>::get();
            ArchivedDisputes::<T>::insert(archived_id, archived);
            NextArchivedId::<T>::put(archived_id.saturating_add(1));

            // 更新统计
            ArbitrationStats::<T>::mutate(|stats| {
                stats.total_disputes = stats.total_disputes.saturating_add(1);
                match decision {
                    0 => stats.release_count = stats.release_count.saturating_add(1),
                    1 => stats.refund_count = stats.refund_count.saturating_add(1),
                    _ => stats.partial_count = stats.partial_count.saturating_add(1),
                }
            });

            // 清理原始存储
            Disputed::<T>::remove(domain, id);
            EvidenceIds::<T>::remove(domain, id);
            TwoWayDeposits::<T>::remove(domain, id);
        }

        // ==================== 🆕 Phase 4: 投诉归档辅助函数 ====================

        /// 归档已解决的投诉
        /// 在 on_idle 中调用，每次最多处理 max_count 个
        pub fn archive_old_complaints(max_count: u32) -> u32 {
            let now = frame_system::Pallet::<T>::block_number();
            // 🆕 M-NEW-5修复: 使用 Config 常量替代硬编码归档延迟
            let archive_delay: BlockNumberFor<T> = T::ComplaintArchiveDelayBlocks::get();
            let mut archived_count = 0u32;
            let mut cursor = ComplaintArchiveCursor::<T>::get();
            let max_id = NextComplaintId::<T>::get();

            while archived_count < max_count && cursor < max_id {
                if let Some(complaint) = Complaints::<T>::get(cursor) {
                    // 检查是否可归档
                    let can_archive = complaint.status.is_resolved() 
                        && now.saturating_sub(complaint.updated_at) >= archive_delay;

                    if can_archive {
                        // 创建归档记录
                        let decision = match complaint.status {
                            ComplaintStatus::ResolvedComplainantWin => 0,
                            ComplaintStatus::ResolvedRespondentWin => 1,
                            ComplaintStatus::ResolvedSettlement => 2,
                            ComplaintStatus::Withdrawn => 3,
                            ComplaintStatus::Expired => 4,
                            _ => 2,
                        };

                        let current_block: u32 = now.saturated_into();
                        let archived = ArchivedComplaint {
                            id: cursor,
                            domain: complaint.domain,
                            object_id: complaint.object_id,
                            decision,
                            resolved_at: current_block,
                            year_month: block_to_year_month(current_block, 14400),
                        };

                        // 存储归档记录
                        ArchivedComplaints::<T>::insert(cursor, archived);

                        // 移除活跃记录
                        Complaints::<T>::remove(cursor);

                        // L-1修复: 用户/被投诉人/对象索引已在解决时立即清理，
                        // 归档阶段无需重复清理

                        archived_count = archived_count.saturating_add(1);

                        Self::deposit_event(Event::ComplaintArchived { complaint_id: cursor });
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            ComplaintArchiveCursor::<T>::put(cursor);
            archived_count
        }

        /// 从用户投诉索引中移除
        fn remove_from_user_complaint_index(user: &T::AccountId, complaint_id: u64) {
            UserActiveComplaints::<T>::mutate(user, |list| {
                list.retain(|&id| id != complaint_id);
            });
        }

        /// 🆕 F4: 从被投诉人索引中移除
        fn remove_from_respondent_complaint_index(respondent: &T::AccountId, complaint_id: u64) {
            RespondentActiveComplaints::<T>::mutate(respondent, |list| {
                list.retain(|&id| id != complaint_id);
            });
        }

        /// 🆕 F6: 从对象投诉索引中移除
        fn remove_from_object_complaint_index(domain: [u8; 8], object_id: u64, complaint_id: u64) {
            ObjectComplaints::<T>::mutate(domain, object_id, |list| {
                list.retain(|&id| id != complaint_id);
            });
        }

        /// 🆕 AH4修复: 使用游标代替全表扫描，处理过期投诉
        pub fn expire_old_complaints(max_count: u32) -> u32 {
            let now = frame_system::Pallet::<T>::block_number();
            let mut expired_count = 0u32;
            let mut cursor = ComplaintExpiryCursor::<T>::get();
            let max_id = NextComplaintId::<T>::get();

            while expired_count < max_count && cursor < max_id {
                if let Some(mut complaint) = Complaints::<T>::get(cursor) {
                    // 🆕 M2-R2修复: 移除 early break 假设（ResponseDeadline 可能被 runtime 升级修改，
                    // 导致后续投诉 deadline 更早）。cursor + max_count 已限制每次处理量。
                    if complaint.status == ComplaintStatus::Submitted && now > complaint.response_deadline {
                        complaint.status = ComplaintStatus::Expired;
                        complaint.updated_at = now;

                        // AH7: 过期投诉退还押金
                        if let Some(deposit_amount) = ComplaintDeposits::<T>::take(cursor) {
                            let _ = T::Fungible::release(
                                &T::RuntimeHoldReason::from(HoldReason::ComplaintDeposit),
                                &complaint.complainant,
                                deposit_amount,
                                frame_support::traits::tokens::Precision::BestEffort,
                            );
                        }

                        Complaints::<T>::insert(cursor, &complaint);

                        // 更新统计
                        DomainStats::<T>::mutate(complaint.domain, |stats| {
                            stats.resolved_count = stats.resolved_count.saturating_add(1);
                            stats.expired_count = stats.expired_count.saturating_add(1);
                        });

                        // 立即清理用户索引（不等归档）
                        Self::remove_from_user_complaint_index(&complaint.complainant, cursor);
                        Self::remove_from_respondent_complaint_index(&complaint.respondent, cursor);
                        Self::remove_from_object_complaint_index(complaint.domain, complaint.object_id, cursor);

                        Self::deposit_event(Event::ComplaintExpired { complaint_id: cursor });
                        expired_count = expired_count.saturating_add(1);
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            ComplaintExpiryCursor::<T>::put(cursor);
            expired_count
        }

        /// 🆕 防膨胀: 清理过期归档仲裁记录
        fn cleanup_old_archived_disputes(current_block: u32, max_per_call: u32) -> u32 {
            let ttl = T::ArchiveTtlBlocks::get();
            if ttl == 0 { return 0; }
            let mut cursor = ArchiveDisputeCleanupCursor::<T>::get();
            let max_id = NextArchivedId::<T>::get();
            let mut cleaned = 0u32;

            while cursor < max_id && cleaned < max_per_call {
                if let Some(archived) = ArchivedDisputes::<T>::get(cursor) {
                    if current_block.saturating_sub(archived.completed_at) > ttl {
                        ArchivedDisputes::<T>::remove(cursor);
                        cleaned += 1;
                    } else {
                        break;
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            ArchiveDisputeCleanupCursor::<T>::put(cursor);
            cleaned
        }

        /// 🆕 防膨胀: 清理过期归档投诉记录
        fn cleanup_old_archived_complaints(current_block: u32, max_per_call: u32) -> u32 {
            let ttl = T::ArchiveTtlBlocks::get();
            if ttl == 0 { return 0; }
            let mut cursor = ArchiveComplaintCleanupCursor::<T>::get();
            let next_complaint = NextComplaintId::<T>::get();
            let mut cleaned = 0u32;

            while cursor < next_complaint && cleaned < max_per_call {
                if let Some(archived) = ArchivedComplaints::<T>::get(cursor) {
                    if current_block.saturating_sub(archived.resolved_at) > ttl {
                        ArchivedComplaints::<T>::remove(cursor);
                        cleaned += 1;
                    } else {
                        break;
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            ArchiveComplaintCleanupCursor::<T>::put(cursor);
            cleaned
        }
    }

    // ==================== 🆕 Phase 4: Hooks 实现 ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let mut weight_used = Weight::zero();
            // 🆕 A7修复: 每项处理涉及 DB 读写，权重从 10K 提升到 25M
            let base_weight = Weight::from_parts(25_000_000, 2_000);
            // 每个阶段至少读取 cursor + max_id（2次 DB read）
            let read_overhead = Weight::from_parts(10_000_000, 1_000);

            // 阶段1：处理过期投诉（每次最多5个）
            if remaining_weight.ref_time() > base_weight.ref_time() * 5 {
                let expired = Self::expire_old_complaints(5);
                weight_used = weight_used.saturating_add(
                    read_overhead.saturating_add(base_weight.saturating_mul(expired as u64))
                );
            }

            // 阶段2：归档已解决投诉（每次最多10个）
            let remaining = remaining_weight.saturating_sub(weight_used);
            if remaining.ref_time() > base_weight.ref_time() * 10 {
                let archived = Self::archive_old_complaints(10);
                weight_used = weight_used.saturating_add(
                    read_overhead.saturating_add(base_weight.saturating_mul(archived as u64))
                );
            }

            // 🆕 阶段3：清理过期归档记录
            let current_block: u32 = now.saturated_into();
            let remaining2 = remaining_weight.saturating_sub(weight_used);
            if remaining2.ref_time() > base_weight.ref_time() * 5 {
                let cleaned = Self::cleanup_old_archived_disputes(current_block, 5);
                weight_used = weight_used.saturating_add(
                    read_overhead.saturating_add(base_weight.saturating_mul(cleaned as u64))
                );
            }
            let remaining3 = remaining_weight.saturating_sub(weight_used);
            if remaining3.ref_time() > base_weight.ref_time() * 5 {
                let cleaned = Self::cleanup_old_archived_complaints(current_block, 5);
                weight_used = weight_used.saturating_add(
                    read_overhead.saturating_add(base_weight.saturating_mul(cleaned as u64))
                );
            }

            weight_used
        }
    }
}
