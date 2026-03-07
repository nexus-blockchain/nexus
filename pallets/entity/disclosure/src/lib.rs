//! # 实体财务披露模块 (pallet-entity-disclosure)
//!
//! ## 概述
//!
//! 本模块实现实体财务信息披露功能：
//! - 多级别披露要求（基础、标准、增强、完全）
//! - 多种披露类型（财务报告、重大事件、关联交易等）
//! - 内幕交易控制（黑窗口期、交易限制）
//! - 披露验证和审计追踪
//!
//! ## 披露级别
//!
//! - **Basic**: 基础披露（年度简报）
//! - **Standard**: 标准披露（季度报告）
//! - **Enhanced**: 增强披露（月度报告 + 重大事件）
//! - **Full**: 完全披露（实时 + 详细财务）
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-02-03): Phase 6 初始版本

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{
        AdminPermission, DisclosureProvider, EntityProvider, EntityStatus,
        OnDisclosureViolation, OnEntityStatusChange,
    };
    use sp_runtime::traits::{Saturating, Zero};
    use sp_runtime::SaturatedConversion;

    // Re-export DisclosureLevel from pallet-entity-common
    pub use pallet_entity_common::DisclosureLevel;

    // ==================== 类型定义 ====================

    /// 披露类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum DisclosureType {
        /// 年度财务报告
        #[default]
        AnnualReport,
        /// 季度财务报告
        QuarterlyReport,
        /// 月度财务报告
        MonthlyReport,
        /// 重大事件公告
        MaterialEvent,
        /// 关联交易披露
        RelatedPartyTransaction,
        /// 股权/代币变动
        OwnershipChange,
        /// 管理层变动
        ManagementChange,
        /// 业务变更
        BusinessChange,
        /// 风险提示
        RiskWarning,
        /// 分红公告
        DividendAnnouncement,
        /// 代币发行公告
        TokenIssuance,
        /// 回购公告
        Buyback,
        /// 其他
        Other,
    }

    /// 披露状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum DisclosureStatus {
        /// 草稿（未发布，可编辑/删除）
        #[default]
        Draft,
        /// 已发布
        Published,
        /// 已撤回
        Withdrawn,
        /// 已更正
        Corrected,
    }

    /// 披露记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen))]
    pub struct DisclosureRecord<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
        /// 披露 ID
        pub id: u64,
        /// 实体 ID
        pub entity_id: u64,
        /// 披露类型
        pub disclosure_type: DisclosureType,
        /// 披露内容 CID (IPFS)
        pub content_cid: BoundedVec<u8, MaxCidLen>,
        /// 披露摘要 CID
        pub summary_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 披露者
        pub discloser: AccountId,
        /// 披露时间
        pub disclosed_at: BlockNumber,
        /// 状态
        pub status: DisclosureStatus,
        /// 关联的前一个披露 ID（用于更正）
        pub previous_id: Option<u64>,
    }

    /// 披露记录类型别名
    pub type DisclosureRecordOf<T> = DisclosureRecord<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
    >;

    /// 实体披露配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DisclosureConfig<BlockNumber> {
        /// 披露级别
        pub level: DisclosureLevel,
        /// 是否启用内幕交易控制
        pub insider_trading_control: bool,
        /// 披露后黑窗口期（区块数）
        pub blackout_period_after: BlockNumber,
        /// 下次必须披露时间
        pub next_required_disclosure: BlockNumber,
        /// 上次披露时间
        pub last_disclosure: BlockNumber,
        /// 连续违规次数（通过 report_disclosure_violation 递增）
        pub violation_count: u32,
    }

    /// 披露配置类型别名
    pub type DisclosureConfigOf<T> = DisclosureConfig<BlockNumberFor<T>>;

    /// 内幕人员记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct InsiderRecord<AccountId, BlockNumber> {
        /// 内幕人员账户
        pub account: AccountId,
        /// 角色（如 owner, admin, auditor）
        pub role: InsiderRole,
        /// 添加时间
        pub added_at: BlockNumber,
    }

    /// P2-a23: 内幕人员角色变更记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct InsiderRoleChangeRecord<BlockNumber> {
        /// 旧角色（None 表示初始添加）
        pub old_role: Option<InsiderRole>,
        /// 新角色
        pub new_role: InsiderRole,
        /// 变更时间
        pub changed_at: BlockNumber,
    }

    // ==================== 公告类型定义 ====================

    /// 公告分类
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum AnnouncementCategory {
        /// 一般公告
        #[default]
        General,
        /// 促销活动
        Promotion,
        /// 系统更新
        SystemUpdate,
        /// 活动通知
        Event,
        /// 政策变更
        Policy,
        /// 合作公告
        Partnership,
        /// 产品公告
        Product,
        /// 其他
        Other,
    }

    /// 公告状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum AnnouncementStatus {
        /// 发布中
        #[default]
        Active,
        /// 已撤回
        Withdrawn,
        /// 已过期
        Expired,
    }

    /// 公告记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxCidLen, MaxTitleLen))]
    pub struct AnnouncementRecord<AccountId, BlockNumber, MaxCidLen: Get<u32>, MaxTitleLen: Get<u32>> {
        /// 公告 ID
        pub id: u64,
        /// 实体 ID
        pub entity_id: u64,
        /// 公告分类
        pub category: AnnouncementCategory,
        /// 标题
        pub title: BoundedVec<u8, MaxTitleLen>,
        /// 内容 CID (IPFS)
        pub content_cid: BoundedVec<u8, MaxCidLen>,
        /// 发布者
        pub publisher: AccountId,
        /// 发布时间
        pub published_at: BlockNumber,
        /// 过期时间（可选，None 表示永不过期）
        pub expires_at: Option<BlockNumber>,
        /// 状态
        pub status: AnnouncementStatus,
        /// 是否置顶
        pub is_pinned: bool,
    }

    /// 公告记录类型别名
    pub type AnnouncementRecordOf<T> = AnnouncementRecord<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxCidLength,
        <T as Config>::MaxTitleLength,
    >;

    /// 内幕人员角色
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum InsiderRole {
        /// 所有者
        #[default]
        Owner,
        /// 管理员
        Admin,
        /// 审计员
        Auditor,
        /// 顾问
        Advisor,
        /// 大股东（持有超过阈值）
        MajorHolder,
    }

    // ==================== v0.6 新增类型 ====================

    /// 审计状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum AuditStatus {
        #[default]
        NotRequired,
        Pending,
        Approved,
        Rejected,
    }

    /// 处罚级别（渐进式）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum PenaltyLevel {
        #[default]
        None,
        /// 口头警告（仅记录）
        Warning,
        /// 限制交易（内幕交易控制强制开启）
        Restricted,
        /// 暂停运营（所有披露写操作冻结）
        Suspended,
        /// 退市（最高级别处罚）
        Delisted,
    }

    impl PenaltyLevel {
        pub fn as_u8(&self) -> u8 {
            match self {
                PenaltyLevel::None => 0,
                PenaltyLevel::Warning => 1,
                PenaltyLevel::Restricted => 2,
                PenaltyLevel::Suspended => 3,
                PenaltyLevel::Delisted => 4,
            }
        }

        pub fn from_u8(v: u8) -> Self {
            match v {
                1 => PenaltyLevel::Warning,
                2 => PenaltyLevel::Restricted,
                3 => PenaltyLevel::Suspended,
                4 => PenaltyLevel::Delisted,
                _ => PenaltyLevel::None,
            }
        }

        pub fn next(&self) -> Self {
            match self {
                PenaltyLevel::None => PenaltyLevel::Warning,
                PenaltyLevel::Warning => PenaltyLevel::Restricted,
                PenaltyLevel::Restricted => PenaltyLevel::Suspended,
                PenaltyLevel::Suspended => PenaltyLevel::Delisted,
                PenaltyLevel::Delisted => PenaltyLevel::Delisted,
            }
        }
    }

    /// 内幕人员交易类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum InsiderTransactionType {
        #[default]
        Buy,
        Sell,
        Transfer,
        Pledge,
        Gift,
    }

    /// 内幕人员交易申报记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct InsiderTransactionReport<AccountId, BlockNumber> {
        pub account: AccountId,
        pub transaction_type: InsiderTransactionType,
        pub token_amount: u128,
        pub reported_at: BlockNumber,
        pub transaction_block: BlockNumber,
    }

    pub type InsiderTransactionReportOf<T> = InsiderTransactionReport<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
    >;

    /// 审批配置（多方签核要求）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ApprovalConfig {
        /// 发布前所需审批人数
        pub required_approvals: u32,
        /// 允许审批的角色位掩码: Owner=0x01, Admin=0x02, Auditor=0x04, Advisor=0x08, MajorHolder=0x10
        pub allowed_roles: u8,
    }

    impl Default for ApprovalConfig {
        fn default() -> Self {
            Self { required_approvals: 0, allowed_roles: 0 }
        }
    }

    impl ApprovalConfig {
        pub fn role_allowed(&self, role: InsiderRole) -> bool {
            let bit = match role {
                InsiderRole::Owner => 0x01,
                InsiderRole::Admin => 0x02,
                InsiderRole::Auditor => 0x04,
                InsiderRole::Advisor => 0x08,
                InsiderRole::MajorHolder => 0x10,
            };
            self.allowed_roles & bit != 0
        }
    }

    /// 披露扩展元数据（与 DisclosureRecord 分离存储，避免迁移）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DisclosureMetadata<BlockNumber: Default> {
        pub period_start: Option<BlockNumber>,
        pub period_end: Option<BlockNumber>,
        pub audit_status: AuditStatus,
        pub is_emergency: bool,
    }

    pub type DisclosureMetadataOf<T> = DisclosureMetadata<BlockNumberFor<T>>;

    /// 财务年度配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct FiscalYearConfig<BlockNumber> {
        /// 财务年度起始区块
        pub year_start_block: BlockNumber,
        /// 财务年度周期长度（区块数）
        pub year_length: BlockNumber,
    }

    pub type FiscalYearConfigOf<T> = FiscalYearConfig<BlockNumberFor<T>>;

    /// 内幕人员记录类型别名
    pub type InsiderRecordOf<T> = InsiderRecord<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
    >;

    // ==================== 配置 ====================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 实体查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 每个实体最大内幕人员数
        #[pallet::constant]
        type MaxInsiders: Get<u32>;

        /// 每个实体最大披露记录数（历史）
        #[pallet::constant]
        type MaxDisclosureHistory: Get<u32>;

        /// 基础披露间隔（区块数，如年度 = 365 * 24 * 600）
        #[pallet::constant]
        type BasicDisclosureInterval: Get<BlockNumberFor<Self>>;

        /// 标准披露间隔（区块数，如季度）
        #[pallet::constant]
        type StandardDisclosureInterval: Get<BlockNumberFor<Self>>;

        /// 增强披露间隔（区块数，如月度）
        #[pallet::constant]
        type EnhancedDisclosureInterval: Get<BlockNumberFor<Self>>;

        /// 黑窗口期最大时长（区块数）
        #[pallet::constant]
        type MaxBlackoutDuration: Get<BlockNumberFor<Self>>;

        /// 每个实体最大公告数
        #[pallet::constant]
        type MaxAnnouncementHistory: Get<u32>;

        /// 公告标题最大长度（字节）
        #[pallet::constant]
        type MaxTitleLength: Get<u32>;

        /// P2-a14: 每个实体最大置顶公告数
        #[pallet::constant]
        type MaxPinnedAnnouncements: Get<u32>;

        /// P2-a23: 每个内幕人员最大角色变更历史记录数
        #[pallet::constant]
        type MaxInsiderRoleHistory: Get<u32>;

        /// F4: 内幕人员移除后冷静期（区块数，期间仍受黑窗口限制）
        #[pallet::constant]
        type InsiderCooldownPeriod: Get<BlockNumberFor<Self>>;

        /// F5: 大股东认定阈值（basis points，如 500 = 5%）
        #[pallet::constant]
        type MajorHolderThreshold: Get<u32>;

        /// F6: 违规次数阈值（达到后标记为高风险）
        #[pallet::constant]
        type ViolationThreshold: Get<u32>;

        /// v0.6: 每个披露最大审批人数
        #[pallet::constant]
        type MaxApprovers: Get<u32>;

        /// v0.6: 每个内幕人员最大交易申报记录数
        #[pallet::constant]
        type MaxInsiderTransactionHistory: Get<u32>;

        /// v0.6: 紧急披露黑窗口期倍数（正常黑窗口期 × 此倍数）
        #[pallet::constant]
        type EmergencyBlackoutMultiplier: Get<u32>;

        /// v0.6: 披露违规回调（通知下游模块处罚升级）
        type OnDisclosureViolation: OnDisclosureViolation;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: crate::weights::WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 下一个披露 ID
    #[pallet::storage]
    #[pallet::getter(fn next_disclosure_id)]
    pub type NextDisclosureId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 披露记录存储
    #[pallet::storage]
    #[pallet::getter(fn disclosures)]
    pub type Disclosures<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // disclosure_id
        DisclosureRecordOf<T>,
    >;

    /// 实体披露配置
    #[pallet::storage]
    #[pallet::getter(fn disclosure_configs)]
    pub type DisclosureConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        DisclosureConfigOf<T>,
    >;

    /// 实体披露历史索引
    #[pallet::storage]
    #[pallet::getter(fn entity_disclosures)]
    pub type EntityDisclosures<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<u64, T::MaxDisclosureHistory>,
        ValueQuery,
    >;

    /// 内幕人员列表
    #[pallet::storage]
    #[pallet::getter(fn insiders)]
    pub type Insiders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<InsiderRecordOf<T>, T::MaxInsiders>,
        ValueQuery,
    >;

    /// 黑窗口期状态 (entity_id) -> (start_block, end_block)
    #[pallet::storage]
    #[pallet::getter(fn blackout_periods)]
    pub type BlackoutPeriods<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        (BlockNumberFor<T>, BlockNumberFor<T>),
    >;

    /// 下一个公告 ID
    #[pallet::storage]
    #[pallet::getter(fn next_announcement_id)]
    pub type NextAnnouncementId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 公告记录存储
    #[pallet::storage]
    #[pallet::getter(fn announcements)]
    pub type Announcements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // announcement_id
        AnnouncementRecordOf<T>,
    >;

    /// 实体公告历史索引
    #[pallet::storage]
    #[pallet::getter(fn entity_announcements)]
    pub type EntityAnnouncements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<u64, T::MaxAnnouncementHistory>,
        ValueQuery,
    >;

    /// 违规记录（防止同一周期重复举报）
    /// key = (entity_id, next_required_disclosure 快照), value = true
    #[pallet::storage]
    pub type ViolationRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        BlockNumberFor<T>,  // next_required_disclosure snapshot
        bool,
        ValueQuery,
    >;

    /// P2-a14: 实体置顶公告（支持多个置顶）
    #[pallet::storage]
    #[pallet::getter(fn pinned_announcements)]
    pub type PinnedAnnouncements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<u64, T::MaxPinnedAnnouncements>,
        ValueQuery,
    >;

    /// P2-a23: 内幕人员角色变更历史
    #[pallet::storage]
    pub type InsiderRoleHistory<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<InsiderRoleChangeRecord<BlockNumberFor<T>>, T::MaxInsiderRoleHistory>,
        ValueQuery,
    >;

    /// F4: 已移除内幕人员冷静期记录 (entity_id, account) -> cooldown_until
    #[pallet::storage]
    pub type RemovedInsiders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BlockNumberFor<T>,
    >;

    /// H1-R2: on_idle 自动违规检测游标（跳过计数，避免重复扫描同一批实体）
    #[pallet::storage]
    pub type AutoViolationCursor<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// F6: 高风险实体标记（违规次数达到阈值）
    #[pallet::storage]
    pub type HighRiskEntities<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        bool,
        ValueQuery,
    >;

    // ==================== v0.6 新增存储 ====================

    /// 审批配置（实体级别的多方签核要求）
    #[pallet::storage]
    pub type ApprovalConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        ApprovalConfig,
    >;

    /// 披露审批记录 (disclosure_id, approver_account) → approved
    #[pallet::storage]
    pub type DisclosureApprovals<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // disclosure_id
        Blake2_128Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;

    /// 披露已获审批计数 disclosure_id → count
    #[pallet::storage]
    pub type DisclosureApprovalCounts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // disclosure_id
        u32,
        ValueQuery,
    >;

    /// 内幕人员交易申报记录
    #[pallet::storage]
    pub type InsiderTransactionReports<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<InsiderTransactionReportOf<T>, T::MaxInsiderTransactionHistory>,
        ValueQuery,
    >;

    /// 实体处罚级别（渐进式处罚）
    #[pallet::storage]
    #[pallet::getter(fn entity_penalties)]
    pub type EntityPenalties<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        PenaltyLevel,
        ValueQuery,
    >;

    /// 财务年度配置
    #[pallet::storage]
    pub type FiscalYearConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        FiscalYearConfigOf<T>,
    >;

    /// 暂停的披露截止时间 entity_id → (paused_at, remaining_blocks_to_deadline)
    #[pallet::storage]
    pub type PausedDeadlines<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        (BlockNumberFor<T>, BlockNumberFor<T>),
    >;

    /// 披露扩展元数据（报告期间、审计状态等）
    #[pallet::storage]
    pub type DisclosureMetadataStore<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // disclosure_id
        DisclosureMetadataOf<T>,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// P2-a2: 草稿已创建
        DraftCreated {
            disclosure_id: u64,
            entity_id: u64,
            disclosure_type: DisclosureType,
        },
        /// P2-a2: 草稿已更新
        DraftUpdated {
            disclosure_id: u64,
        },
        /// P2-a2: 草稿已删除
        DraftDeleted {
            disclosure_id: u64,
            entity_id: u64,
        },
        /// 披露已发布
        DisclosurePublished {
            disclosure_id: u64,
            entity_id: u64,
            disclosure_type: DisclosureType,
            discloser: T::AccountId,
        },
        /// 披露已撤回
        DisclosureWithdrawn {
            disclosure_id: u64,
            entity_id: u64,
        },
        /// 披露已更正
        DisclosureCorrected {
            old_disclosure_id: u64,
            new_disclosure_id: u64,
            entity_id: u64,
        },
        /// 披露配置已更新
        DisclosureConfigUpdated {
            entity_id: u64,
            level: DisclosureLevel,
        },
        /// 内幕人员已添加
        InsiderAdded {
            entity_id: u64,
            account: T::AccountId,
            role: InsiderRole,
        },
        /// 内幕人员已移除
        InsiderRemoved {
            entity_id: u64,
            account: T::AccountId,
        },
        /// P2-a3: 内幕人员角色已更新
        InsiderRoleUpdated {
            entity_id: u64,
            account: T::AccountId,
            old_role: InsiderRole,
            new_role: InsiderRole,
        },
        /// 黑窗口期已开始
        BlackoutStarted {
            entity_id: u64,
            start_block: BlockNumberFor<T>,
            end_block: BlockNumberFor<T>,
        },
        /// 黑窗口期已结束
        BlackoutEnded {
            entity_id: u64,
        },
        /// 披露违规（通过 report_disclosure_violation 发出）
        DisclosureViolation {
            entity_id: u64,
            violation_type: ViolationType,
            violation_count: u32,
        },
        /// P1-a15: Root 强制配置披露
        DisclosureForceConfigured {
            entity_id: u64,
            level: DisclosureLevel,
        },
        /// P1-a20: 实体披露存储已清理
        EntityDisclosureCleaned {
            entity_id: u64,
        },
        /// P1-a16: on_idle 自动检测到披露违规
        AutoViolationDetected {
            entity_id: u64,
            violation_count: u32,
        },
        /// 公告已发布
        AnnouncementPublished {
            announcement_id: u64,
            entity_id: u64,
            category: AnnouncementCategory,
            publisher: T::AccountId,
        },
        /// 公告已更新
        AnnouncementUpdated {
            announcement_id: u64,
            entity_id: u64,
        },
        /// 公告已撤回
        AnnouncementWithdrawn {
            announcement_id: u64,
            entity_id: u64,
        },
        /// 公告已置顶
        AnnouncementPinned {
            entity_id: u64,
            announcement_id: u64,
        },
        /// 公告已取消置顶
        AnnouncementUnpinned {
            entity_id: u64,
        },
        /// 公告已标记为过期
        AnnouncementMarkedExpired {
            announcement_id: u64,
            entity_id: u64,
        },
        /// M1-R3: 披露历史索引已清理
        DisclosureHistoryCleaned {
            entity_id: u64,
            disclosure_id: u64,
        },
        /// M1-R3: 公告历史索引已清理
        AnnouncementHistoryCleaned {
            entity_id: u64,
            announcement_id: u64,
        },
        /// F1: 批量添加内幕人员
        InsidersBatchAdded {
            entity_id: u64,
            count: u32,
        },
        /// F1: 批量移除内幕人员
        InsidersBatchRemoved {
            entity_id: u64,
            count: u32,
        },
        /// F4: 内幕人员冷静期开始
        InsiderCooldownStarted {
            entity_id: u64,
            account: T::AccountId,
            until: BlockNumberFor<T>,
        },
        /// F6: 实体被标记为高风险
        EntityMarkedHighRisk {
            entity_id: u64,
            violation_count: u32,
        },
        /// F8: 违规次数已重置
        ViolationCountReset {
            entity_id: u64,
        },
        /// F9: 黑窗口期已过期清理
        BlackoutExpired {
            entity_id: u64,
        },

        // ==================== v0.6 新增事件 ====================

        /// 披露已获审批
        DisclosureApproved {
            disclosure_id: u64,
            approver: T::AccountId,
            approval_count: u32,
            required: u32,
        },
        /// 披露审批被拒绝
        DisclosureRejected {
            disclosure_id: u64,
            rejector: T::AccountId,
        },
        /// 审批要求已配置
        ApprovalRequirementConfigured {
            entity_id: u64,
            required_approvals: u32,
        },
        /// 紧急披露已发布
        EmergencyDisclosurePublished {
            disclosure_id: u64,
            entity_id: u64,
            discloser: T::AccountId,
        },
        /// 内幕人员交易已申报
        InsiderTransactionReported {
            entity_id: u64,
            account: T::AccountId,
            transaction_type: InsiderTransactionType,
            token_amount: u128,
        },
        /// 处罚已升级
        PenaltyEscalated {
            entity_id: u64,
            old_level: PenaltyLevel,
            new_level: PenaltyLevel,
        },
        /// 处罚已重置
        PenaltyReset {
            entity_id: u64,
        },
        /// 财务年度已配置
        FiscalYearConfigured {
            entity_id: u64,
        },
        /// 过期冷静期已清理
        CooldownsCleaned {
            entity_id: u64,
            count: u32,
        },
        /// 披露截止时间已暂停（实体被暂停/封禁时）
        DeadlinePaused {
            entity_id: u64,
        },
        /// 披露截止时间已恢复（实体恢复运营时）
        DeadlineResumed {
            entity_id: u64,
            new_deadline: BlockNumberFor<T>,
        },
        /// 大股东已自动注册为内幕人员
        MajorHolderRegistered {
            entity_id: u64,
            account: T::AccountId,
        },
        /// 大股东已自动注销内幕人员身份
        MajorHolderDeregistered {
            entity_id: u64,
            account: T::AccountId,
        },
    }

    /// 违规类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum ViolationType {
        /// 逾期披露
        LateDisclosure,
        /// 黑窗口期交易
        BlackoutTrading,
        /// 未披露重大事件
        UndisclosedMaterialEvent,
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 实体不存在
        EntityNotFound,
        /// 不是管理员
        NotAdmin,
        /// 披露不存在
        DisclosureNotFound,
        /// CID 过长
        CidTooLong,
        /// 历史记录已满
        HistoryFull,
        /// 内幕人员已存在
        InsiderExists,
        /// 内幕人员不存在
        InsiderNotFound,
        /// 内幕人员列表已满
        InsidersFull,
        /// 黑窗口期内禁止交易 — 预留，供外部模块调用 can_insider_trade 后自行使用
        InBlackoutPeriod,
        /// 无效的披露状态
        InvalidDisclosureStatus,
        /// 披露级别不满足要求 — 预留，供外部模块按级别限制操作
        InsufficientDisclosureLevel,
        /// 披露间隔未到 — 预留，供外部模块检查披露频率
        DisclosureIntervalNotReached,
        /// 黑窗口期时长为零或超过上限
        InvalidBlackoutDuration,
        /// 内容 CID 不能为空
        EmptyCid,
        /// 黑窗口期不存在
        BlackoutNotFound,
        /// 黑窗口期配置超限：blackout_after 超过 MaxBlackoutDuration
        BlackoutExceedsMax,
        /// 公告不存在
        AnnouncementNotFound,
        /// 公告历史记录已满
        AnnouncementHistoryFull,
        /// 公告状态不是 Active
        AnnouncementNotActive,
        /// 公告标题不能为空
        EmptyTitle,
        /// 公告标题超过最大长度
        TitleTooLong,
        /// 过期时间无效（早于当前区块）
        InvalidExpiry,
        /// ID 溢出（u64::MAX）
        IdOverflow,
        /// 更新公告时未提供任何修改项
        NoUpdateProvided,
        /// 公告已过期
        AnnouncementExpired,
        /// 已存在活跃黑窗口期，不能缩短（请先 end_blackout）
        BlackoutStillActive,
        /// 公告尚未过期（无 expires_at 或未到过期时间）
        AnnouncementNotExpired,
        /// 披露未处于终态（仅 Withdrawn/Corrected 可清理）
        DisclosureNotTerminal,
        /// 公告未处于终态（仅 Withdrawn/Expired 可清理）
        AnnouncementNotTerminal,
        /// 披露未逾期，无法举报违规
        DisclosureNotOverdue,
        /// 该周期违规已记录，不可重复举报
        ViolationAlreadyRecorded,
        /// 未配置披露（无 DisclosureConfig）
        DisclosureNotConfigured,
        /// P1-a5: 管理员不能降低披露级别（仅 Root 可降级）
        DisclosureLevelDowngrade,
        /// P1-a20: 实体未关闭，不可清理披露存储
        EntityNotClosed,
        /// P2-a2: 披露不是草稿状态
        DisclosureNotDraft,
        /// P2-a14: 置顶公告已满
        PinnedAnnouncementsFull,
        /// P2-a14: 公告未置顶
        AnnouncementNotPinned,
        /// P2-a23: 角色变更历史已满
        RoleHistoryFull,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// F1: 批量操作列表为空
        EmptyBatch,
        /// F2: 实体不是 Active 状态，写操作被拒绝
        EntityNotActive,
        /// F3: 披露类型不允许在当前披露级别下使用
        DisclosureTypeNotAllowed,
        /// F4: 内幕人员处于冷静期，交易受限
        InsiderInCooldown,
        /// F9: 黑窗口期尚未过期
        BlackoutNotExpired,

        // ==================== v0.6 新增错误 ====================

        /// 已经审批过此披露
        AlreadyApproved,
        /// 不具备审批资格（角色不在允许列表中）
        NotApprover,
        /// 审批数量不足，不可发布
        InsufficientApprovals,
        /// 未配置审批要求
        ApprovalNotConfigured,
        /// 交易申报记录已满
        TransactionHistoryFull,
        /// 处罚级别无效
        InvalidPenaltyLevel,
        /// 处罚已达到或超过指定级别
        PenaltyAlreadyAtLevel,
        /// 截止时间未暂停
        DeadlineNotPaused,
        /// 截止时间已暂停
        DeadlineAlreadyPaused,
        /// 不是内幕人员（交易申报需要内幕人员身份）
        NotInsider,
        /// 报告期间无效（start >= end）
        InvalidReportingPeriod,
        /// 无效的审批角色配置（allowed_roles 为 0）
        InvalidApprovalRoles,
        /// 审批要求数量为零（required_approvals > 0）
        ZeroApprovalCount,
        /// 实体当前受到处罚限制，写操作被拒绝
        PenaltyRestricted,
        /// 财务年度周期长度不能为零
        ZeroFiscalYearLength,
        /// 大股东已在内幕人员列表中
        MajorHolderAlreadyRegistered,
    }

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// P1-a16: on_idle 扫描逾期披露，自动递增 violation_count
        ///
        /// 每个区块最多扫描 MAX_SCAN 个实体。通过 ViolationRecords 天然去重，
        /// 同一逾期周期不会重复递增 violation_count。
        fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            const MAX_SCAN: u32 = 10;
            let per_entity_weight = Weight::from_parts(15_000_000, 2_000);
            // M1-R2: 跳过的实体仍消耗存储读取，需计入权重（读开销较小）
            let per_skip_weight = Weight::from_parts(5_000_000, 1_000);
            let base_weight = Weight::from_parts(5_000_000, 1_000);

            // M2: 同时检查 ref_time 和 proof_size
            if remaining_weight.ref_time() < base_weight.ref_time()
                || remaining_weight.proof_size() < base_weight.proof_size()
            {
                return Weight::zero();
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let mut scanned = 0u32;
            let mut skipped = 0u32;
            let mut used_weight = base_weight;

            // H1-R2: 使用跳过计数（而非 entity_id 比较），因为 StorageMap::iter()
            // 按哈希顺序迭代，不按 key 数值排序
            let skip_count = AutoViolationCursor::<T>::get();

            for (entity_id, config) in DisclosureConfigs::<T>::iter() {
                // 跳过前 skip_count 个实体
                if skipped < skip_count {
                    skipped += 1;
                    // M1-R2: 计入跳过的权重
                    used_weight = used_weight.saturating_add(per_skip_weight);
                    if used_weight.ref_time() > remaining_weight.ref_time()
                        || used_weight.proof_size() > remaining_weight.proof_size()
                    {
                        break;
                    }
                    continue;
                }

                if scanned >= MAX_SCAN {
                    break;
                }
                let needed = used_weight.saturating_add(per_entity_weight);
                // M2: 同时检查 ref_time 和 proof_size
                if needed.ref_time() > remaining_weight.ref_time()
                    || needed.proof_size() > remaining_weight.proof_size()
                {
                    break;
                }

                // v0.6: 跳过已暂停截止时间的实体（实体被暂停/封禁时不计违规）
                if PausedDeadlines::<T>::contains_key(entity_id) {
                    scanned += 1;
                    used_weight = needed;
                    continue;
                }

                // v0.6: 跳过非 Active 状态实体
                if !T::EntityProvider::is_entity_active(entity_id) {
                    scanned += 1;
                    used_weight = needed;
                    continue;
                }

                // 检查是否逾期
                if now > config.next_required_disclosure {
                    let snapshot = config.next_required_disclosure;
                    // ViolationRecords 天然去重：同一周期只记录一次
                    if !ViolationRecords::<T>::get(entity_id, snapshot) {
                        DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                            if let Some(c) = maybe_config {
                                c.violation_count = c.violation_count.saturating_add(1);
                            }
                        });
                        ViolationRecords::<T>::insert(entity_id, snapshot, true);

                        let new_count = DisclosureConfigs::<T>::get(entity_id)
                            .map(|c| c.violation_count)
                            .unwrap_or(0);

                        // F6: 检查是否达到高风险阈值
                        Self::check_violation_threshold(entity_id, new_count);

                        Self::deposit_event(Event::AutoViolationDetected {
                            entity_id,
                            violation_count: new_count,
                        });
                    }
                }

                scanned += 1;
                used_weight = needed;
            }

            // H1-R2: 更新游标
            if scanned == 0 {
                // 没有扫描到任何实体（已到末尾或权重耗尽在跳过阶段），归零重新开始
                AutoViolationCursor::<T>::put(0u32);
            } else {
                // 下次从 skip_count + scanned 开始
                AutoViolationCursor::<T>::put(skip_count.saturating_add(scanned));
            }

            used_weight
        }
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 配置实体披露设置
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::configure_disclosure())]
        pub fn configure_disclosure(
            origin: OriginFor<T>,
            entity_id: u64,
            level: DisclosureLevel,
            insider_trading_control: bool,
            blackout_after: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // H1: 验证 blackout_after 不超过 MaxBlackoutDuration
            let max_blackout = T::MaxBlackoutDuration::get();
            ensure!(blackout_after <= max_blackout, Error::<T>::BlackoutExceedsMax);

            // P1-a5: 管理员不能降低披露级别（仅 Root 可降级）
            let existing = DisclosureConfigs::<T>::get(entity_id);
            if let Some(ref config) = existing {
                ensure!(
                    level as u8 >= config.level as u8,
                    Error::<T>::DisclosureLevelDowngrade
                );
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let next_required = Self::calculate_next_disclosure(level, now);

            // D-L1 审计修复: 保留已有 violation_count 和 last_disclosure，防止通过重新配置清除
            let existing_violations = existing.as_ref().map(|c| c.violation_count).unwrap_or(0);
            // H5: 保留已有 last_disclosure，不伪造为 now
            let existing_last = existing.as_ref().map(|c| c.last_disclosure).unwrap_or_else(Zero::zero);

            DisclosureConfigs::<T>::insert(entity_id, DisclosureConfig {
                level,
                insider_trading_control,
                blackout_period_after: blackout_after,
                next_required_disclosure: next_required,
                last_disclosure: existing_last,
                violation_count: existing_violations,
            });

            Self::deposit_event(Event::DisclosureConfigUpdated {
                entity_id,
                level,
            });
            Ok(())
        }

        /// 发布披露
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::publish_disclosure())]
        pub fn publish_disclosure(
            origin: OriginFor<T>,
            entity_id: u64,
            disclosure_type: DisclosureType,
            content_cid: Vec<u8>,
            summary_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // F3: 验证披露类型是否允许在当前级别下使用
            Self::validate_disclosure_type(entity_id, disclosure_type)?;

            // H2: 内容 CID 不能为空
            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);

            let content_bounded: BoundedVec<u8, T::MaxCidLength> = 
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            // M3: summary_cid 不能为空字符串
            if let Some(ref s) = summary_cid {
                ensure!(!s.is_empty(), Error::<T>::EmptyCid);
            }
            let summary_bounded = summary_cid
                .map(|s| s.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            let now = <frame_system::Pallet<T>>::block_number();
            let disclosure_id = NextDisclosureId::<T>::get();

            let record = DisclosureRecord {
                id: disclosure_id,
                entity_id,
                disclosure_type,
                content_cid: content_bounded,
                summary_cid: summary_bounded,
                discloser: who.clone(),
                disclosed_at: now,
                status: DisclosureStatus::Published,
                previous_id: None,
            };

            // 保存记录
            Disclosures::<T>::insert(disclosure_id, record);
            // H1: checked_add 防止 ID 溢出
            NextDisclosureId::<T>::put(
                disclosure_id.checked_add(1).ok_or(Error::<T>::IdOverflow)?
            );

            // 更新实体披露历史
            EntityDisclosures::<T>::try_mutate(entity_id, |history| -> DispatchResult {
                history.try_push(disclosure_id).map_err(|_| Error::<T>::HistoryFull)?;
                Ok(())
            })?;

            // H4: 更新配置并检查黑窗口期（合并为单次读写）
            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.last_disclosure = now;
                    config.next_required_disclosure = Self::calculate_next_disclosure(config.level, now);

                    // 如果启用内幕交易控制，开始黑窗口期
                    if config.insider_trading_control && !config.blackout_period_after.is_zero() {
                        let end_block = now.saturating_add(config.blackout_period_after);
                        // M1-deep: 不缩短已有的黑窗口期
                        let actual_end = Self::set_or_extend_blackout(entity_id, now, end_block);
                        Self::deposit_event(Event::BlackoutStarted {
                            entity_id,
                            start_block: now,
                            end_block: actual_end,
                        });
                    }
                }
            });

            Self::deposit_event(Event::DisclosurePublished {
                disclosure_id,
                entity_id,
                disclosure_type,
                discloser: who,
            });
            Ok(())
        }

        /// P2-a2: 创建草稿披露（不触发黑窗口期，不更新配置）
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::create_draft_disclosure())]
        pub fn create_draft_disclosure(
            origin: OriginFor<T>,
            entity_id: u64,
            disclosure_type: DisclosureType,
            content_cid: Vec<u8>,
            summary_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // F3: 验证披露类型是否允许在当前级别下使用
            Self::validate_disclosure_type(entity_id, disclosure_type)?;

            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);
            let content_bounded: BoundedVec<u8, T::MaxCidLength> =
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            if let Some(ref s) = summary_cid {
                ensure!(!s.is_empty(), Error::<T>::EmptyCid);
            }
            let summary_bounded = summary_cid
                .map(|s| s.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            let now = <frame_system::Pallet<T>>::block_number();
            let disclosure_id = NextDisclosureId::<T>::get();

            let record = DisclosureRecord {
                id: disclosure_id,
                entity_id,
                disclosure_type,
                content_cid: content_bounded,
                summary_cid: summary_bounded,
                discloser: who,
                disclosed_at: now,
                status: DisclosureStatus::Draft,
                previous_id: None,
            };

            Disclosures::<T>::insert(disclosure_id, record);
            NextDisclosureId::<T>::put(
                disclosure_id.checked_add(1).ok_or(Error::<T>::IdOverflow)?
            );

            Self::deposit_event(Event::DraftCreated {
                disclosure_id,
                entity_id,
                disclosure_type,
            });
            Ok(())
        }

        /// P2-a2: 更新草稿披露内容
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::update_draft())]
            origin: OriginFor<T>,
            disclosure_id: u64,
            content_cid: Vec<u8>,
            summary_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Disclosures::<T>::try_mutate(disclosure_id, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::DisclosureNotFound)?;
                ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

                T::EntityProvider::entity_owner(record.entity_id).ok_or(Error::<T>::EntityNotFound)?;
                ensure!(T::EntityProvider::is_entity_active(record.entity_id), Error::<T>::EntityNotActive);
                ensure!(
                    T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                    Error::<T>::NotAdmin
                );
                ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);

                ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);
                record.content_cid = content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                if let Some(ref s) = summary_cid {
                    ensure!(!s.is_empty(), Error::<T>::EmptyCid);
                }
                record.summary_cid = summary_cid
                    .map(|s| s.try_into().map_err(|_| Error::<T>::CidTooLong))
                    .transpose()?;

                Ok(())
            })?;

            Self::deposit_event(Event::DraftUpdated { disclosure_id });
            Ok(())
        }

        /// P2-a2: 删除草稿披露
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::delete_draft())]
        pub fn delete_draft(
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;
            ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

            T::EntityProvider::entity_owner(record.entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(record.entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);

            let entity_id = record.entity_id;
            Disclosures::<T>::remove(disclosure_id);

            Self::deposit_event(Event::DraftDeleted { disclosure_id, entity_id });
            Ok(())
        }

        /// P2-a2: 发布草稿（Draft → Published，触发黑窗口期 + 配置更新）
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::publish_draft())]
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = Disclosures::<T>::try_mutate(disclosure_id, |maybe_record| -> Result<u64, DispatchError> {
                let record = maybe_record.as_mut().ok_or(Error::<T>::DisclosureNotFound)?;
                ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

                T::EntityProvider::entity_owner(record.entity_id).ok_or(Error::<T>::EntityNotFound)?;
                ensure!(T::EntityProvider::is_entity_active(record.entity_id), Error::<T>::EntityNotActive);
                ensure!(
                    T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                    Error::<T>::NotAdmin
                );
                ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);

                // v0.6: 检查审批要求
                if let Some(approval_config) = ApprovalConfigs::<T>::get(record.entity_id) {
                    if approval_config.required_approvals > 0 {
                        let count = DisclosureApprovalCounts::<T>::get(disclosure_id);
                        ensure!(count >= approval_config.required_approvals, Error::<T>::InsufficientApprovals);
                    }
                }

                let now = <frame_system::Pallet<T>>::block_number();
                record.status = DisclosureStatus::Published;
                record.disclosed_at = now;

                Ok(record.entity_id)
            })?;

            // v0.6: 清理审批记录
            let _ = DisclosureApprovals::<T>::clear_prefix(disclosure_id, u32::MAX, None);
            DisclosureApprovalCounts::<T>::remove(disclosure_id);

            let now = <frame_system::Pallet<T>>::block_number();

            // 添加到实体披露历史
            EntityDisclosures::<T>::try_mutate(entity_id, |history| -> DispatchResult {
                history.try_push(disclosure_id).map_err(|_| Error::<T>::HistoryFull)?;
                Ok(())
            })?;

            // 更新配置 + 黑窗口期（与 publish_disclosure 一致）
            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.last_disclosure = now;
                    config.next_required_disclosure = Self::calculate_next_disclosure(config.level, now);

                    if config.insider_trading_control && !config.blackout_period_after.is_zero() {
                        let end_block = now.saturating_add(config.blackout_period_after);
                        let actual_end = Self::set_or_extend_blackout(entity_id, now, end_block);
                        Self::deposit_event(Event::BlackoutStarted {
                            entity_id,
                            start_block: now,
                            end_block: actual_end,
                        });
                    }
                }
            });

            let disclosure_type = Disclosures::<T>::get(disclosure_id)
                .map(|r| r.disclosure_type)
                .unwrap_or_default();

            Self::deposit_event(Event::DisclosurePublished {
                disclosure_id,
                entity_id,
                disclosure_type,
                discloser: who,
            });
            Ok(())
        }

        /// 撤回披露
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::withdraw_disclosure())]
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Disclosures::<T>::try_mutate(disclosure_id, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::DisclosureNotFound)?;
                
                // H2: 验证权限（管理员或原始披露者）
                T::EntityProvider::entity_owner(record.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(
                    T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE)
                        || record.discloser == who,
                    Error::<T>::NotAdmin
                );
                ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);
                
                // 验证状态
                ensure!(record.status == DisclosureStatus::Published, Error::<T>::InvalidDisclosureStatus);
                
                record.status = DisclosureStatus::Withdrawn;
                
                Self::deposit_event(Event::DisclosureWithdrawn {
                    disclosure_id,
                    entity_id: record.entity_id,
                });
                Ok(())
            })
        }

        /// 更正披露（发布新版本）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::correct_disclosure())]
        pub fn correct_disclosure(
            origin: OriginFor<T>,
            old_disclosure_id: u64,
            content_cid: Vec<u8>,
            summary_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let old_record = Disclosures::<T>::get(old_disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;
            
            // H2: 只能更正 Published 状态的记录
            ensure!(old_record.status == DisclosureStatus::Published, Error::<T>::InvalidDisclosureStatus);

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(old_record.entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(old_record.entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(old_record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(old_record.entity_id), Error::<T>::EntityLocked);

            // H2/H3: 内容 CID 不能为空
            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);

            let content_bounded: BoundedVec<u8, T::MaxCidLength> = 
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            // M3: summary_cid 不能为空字符串
            if let Some(ref s) = summary_cid {
                ensure!(!s.is_empty(), Error::<T>::EmptyCid);
            }
            let summary_bounded = summary_cid
                .map(|s| s.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            let now = <frame_system::Pallet<T>>::block_number();
            let new_disclosure_id = NextDisclosureId::<T>::get();

            // 创建新记录
            let new_record = DisclosureRecord {
                id: new_disclosure_id,
                entity_id: old_record.entity_id,
                disclosure_type: old_record.disclosure_type,
                content_cid: content_bounded,
                summary_cid: summary_bounded,
                discloser: who.clone(),
                disclosed_at: now,
                status: DisclosureStatus::Published,
                previous_id: Some(old_disclosure_id),
            };

            // 更新旧记录状态
            Disclosures::<T>::mutate(old_disclosure_id, |maybe_record| {
                if let Some(record) = maybe_record {
                    record.status = DisclosureStatus::Corrected;
                }
            });

            // 保存新记录
            Disclosures::<T>::insert(new_disclosure_id, new_record);
            // H1: checked_add 防止 ID 溢出
            NextDisclosureId::<T>::put(
                new_disclosure_id.checked_add(1).ok_or(Error::<T>::IdOverflow)?
            );

            // 更新历史
            EntityDisclosures::<T>::try_mutate(old_record.entity_id, |history| -> DispatchResult {
                history.try_push(new_disclosure_id).map_err(|_| Error::<T>::HistoryFull)?;
                Ok(())
            })?;

            // H3: 更正披露也应触发黑窗口期
            let entity_id = old_record.entity_id;
            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.last_disclosure = now;
                    config.next_required_disclosure = Self::calculate_next_disclosure(config.level, now);

                    if config.insider_trading_control && !config.blackout_period_after.is_zero() {
                        let end_block = now.saturating_add(config.blackout_period_after);
                        // M1-deep: 不缩短已有的黑窗口期
                        let actual_end = Self::set_or_extend_blackout(entity_id, now, end_block);
                        Self::deposit_event(Event::BlackoutStarted {
                            entity_id,
                            start_block: now,
                            end_block: actual_end,
                        });
                    }
                }
            });

            Self::deposit_event(Event::DisclosureCorrected {
                old_disclosure_id,
                new_disclosure_id,
                entity_id,
            });
            Ok(())
        }

        /// 添加内幕人员
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::add_insider())]
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            role: InsiderRole,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let now = <frame_system::Pallet<T>>::block_number();

            Insiders::<T>::try_mutate(entity_id, |insiders| -> DispatchResult {
                // M3: 硬删除后不存在非活跃记录，直接检查是否已存在
                ensure!(
                    !insiders.iter().any(|i| i.account == account),
                    Error::<T>::InsiderExists
                );

                let record = InsiderRecord {
                    account: account.clone(),
                    role,
                    added_at: now,
                };
                insiders.try_push(record).map_err(|_| Error::<T>::InsidersFull)?;
                Ok(())
            })?;

            // P2-a23: 记录初始角色历史
            InsiderRoleHistory::<T>::try_mutate(entity_id, &account, |history| -> DispatchResult {
                let record = InsiderRoleChangeRecord {
                    old_role: None,
                    new_role: role,
                    changed_at: now,
                };
                history.try_push(record).map_err(|_| Error::<T>::RoleHistoryFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::InsiderAdded {
                entity_id,
                account,
                role,
            });
            Ok(())
        }

        /// P2-a3: 更新内幕人员角色
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::update_insider_role())]
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            new_role: InsiderRole,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let old_role = Insiders::<T>::try_mutate(entity_id, |insiders| -> Result<InsiderRole, DispatchError> {
                let insider = insiders.iter_mut()
                    .find(|i| i.account == account)
                    .ok_or(Error::<T>::InsiderNotFound)?;
                let old = insider.role;
                insider.role = new_role;
                Ok(old)
            })?;

            // P2-a23: 记录角色变更历史
            let now = <frame_system::Pallet<T>>::block_number();
            InsiderRoleHistory::<T>::try_mutate(entity_id, &account, |history| -> DispatchResult {
                let record = InsiderRoleChangeRecord {
                    old_role: Some(old_role),
                    new_role,
                    changed_at: now,
                };
                history.try_push(record).map_err(|_| Error::<T>::RoleHistoryFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::InsiderRoleUpdated {
                entity_id,
                account,
                old_role,
                new_role,
            });
            Ok(())
        }

        /// 移除内幕人员
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::remove_insider())]
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            Insiders::<T>::try_mutate(entity_id, |insiders| -> DispatchResult {
                // M3: 硬删除 — swap_remove 释放 BoundedVec 容量
                let pos = insiders.iter().position(|i| i.account == account)
                    .ok_or(Error::<T>::InsiderNotFound)?;
                insiders.swap_remove(pos);
                Ok(())
            })?;

            // F4: 记录冷静期
            let cooldown = T::InsiderCooldownPeriod::get();
            if !cooldown.is_zero() {
                let now = <frame_system::Pallet<T>>::block_number();
                let until = now.saturating_add(cooldown);
                RemovedInsiders::<T>::insert(entity_id, &account, until);
                Self::deposit_event(Event::InsiderCooldownStarted {
                    entity_id,
                    account: account.clone(),
                    until,
                });
            }

            Self::deposit_event(Event::InsiderRemoved {
                entity_id,
                account,
            });
            Ok(())
        }

        /// 手动开始黑窗口期
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::start_blackout())]
            origin: OriginFor<T>,
            entity_id: u64,
            duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // M3: 使用 Config 常量替代硬编码
            let max_blackout = T::MaxBlackoutDuration::get();
            ensure!(!duration.is_zero() && duration <= max_blackout, Error::<T>::InvalidBlackoutDuration);

            let now = <frame_system::Pallet<T>>::block_number();
            let end_block = now.saturating_add(duration);

            // M1-R2: 不允许缩短活跃黑窗口期（防止绕过 auto-blackout 内幕交易控制）
            if let Some((_existing_start, existing_end)) = BlackoutPeriods::<T>::get(entity_id) {
                if existing_end > now && end_block < existing_end {
                    return Err(Error::<T>::BlackoutStillActive.into());
                }
            }

            BlackoutPeriods::<T>::insert(entity_id, (now, end_block));

            Self::deposit_event(Event::BlackoutStarted {
                entity_id,
                start_block: now,
                end_block,
            });
            Ok(())
        }

        /// 结束黑窗口期
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::end_blackout())]
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // L1: 检查黑窗口期是否存在
            ensure!(BlackoutPeriods::<T>::contains_key(entity_id), Error::<T>::BlackoutNotFound);

            BlackoutPeriods::<T>::remove(entity_id);

            Self::deposit_event(Event::BlackoutEnded { entity_id });
            Ok(())
        }

        // ==================== 公告 Extrinsics ====================

        /// 发布公告
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::publish_announcement())]
        pub fn publish_announcement(
            origin: OriginFor<T>,
            entity_id: u64,
            category: AnnouncementCategory,
            title: Vec<u8>,
            content_cid: Vec<u8>,
            expires_at: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 验证标题
            ensure!(!title.is_empty(), Error::<T>::EmptyTitle);
            let title_bounded: BoundedVec<u8, T::MaxTitleLength> =
                title.try_into().map_err(|_| Error::<T>::TitleTooLong)?;

            // 验证内容 CID
            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);
            let content_bounded: BoundedVec<u8, T::MaxCidLength> =
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let now = <frame_system::Pallet<T>>::block_number();

            // 验证过期时间
            if let Some(exp) = expires_at {
                ensure!(exp > now, Error::<T>::InvalidExpiry);
            }

            let announcement_id = NextAnnouncementId::<T>::get();

            let record = AnnouncementRecord {
                id: announcement_id,
                entity_id,
                category,
                title: title_bounded,
                content_cid: content_bounded,
                publisher: who.clone(),
                published_at: now,
                expires_at,
                status: AnnouncementStatus::Active,
                is_pinned: false,
            };

            Announcements::<T>::insert(announcement_id, record);
            // H1: checked_add 防止 ID 溢出
            NextAnnouncementId::<T>::put(
                announcement_id.checked_add(1).ok_or(Error::<T>::IdOverflow)?
            );

            EntityAnnouncements::<T>::try_mutate(entity_id, |history| -> DispatchResult {
                history.try_push(announcement_id)
                    .map_err(|_| Error::<T>::AnnouncementHistoryFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::AnnouncementPublished {
                announcement_id,
                entity_id,
                category,
                publisher: who,
            });
            Ok(())
        }

        /// 更新公告（标题、内容、分类、过期时间）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::update_announcement())]
        pub fn update_announcement(
            origin: OriginFor<T>,
            announcement_id: u64,
            title: Option<Vec<u8>>,
            content_cid: Option<Vec<u8>>,
            category: Option<AnnouncementCategory>,
            expires_at: Option<Option<BlockNumberFor<T>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M2: 至少需要更新一项
            ensure!(
                title.is_some() || content_cid.is_some() || category.is_some() || expires_at.is_some(),
                Error::<T>::NoUpdateProvided
            );

            Announcements::<T>::try_mutate(announcement_id, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::AnnouncementNotFound)?;

                // H2: 验证实体存在 + 管理员权限
                T::EntityProvider::entity_owner(record.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(T::EntityProvider::is_entity_active(record.entity_id), Error::<T>::EntityNotActive);
                ensure!(
                    T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                    Error::<T>::NotAdmin
                );
                ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);

                ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

                let now = <frame_system::Pallet<T>>::block_number();

                // M2-R2: 不允许更新已逾期的公告
                if let Some(exp) = record.expires_at {
                    ensure!(now <= exp, Error::<T>::AnnouncementExpired);
                }

                if let Some(new_title) = title {
                    ensure!(!new_title.is_empty(), Error::<T>::EmptyTitle);
                    record.title = new_title.try_into().map_err(|_| Error::<T>::TitleTooLong)?;
                }

                if let Some(new_cid) = content_cid {
                    ensure!(!new_cid.is_empty(), Error::<T>::EmptyCid);
                    record.content_cid = new_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
                }

                if let Some(new_category) = category {
                    record.category = new_category;
                }

                if let Some(new_expires) = expires_at {
                    if let Some(exp) = new_expires {
                        ensure!(exp > now, Error::<T>::InvalidExpiry);
                    }
                    record.expires_at = new_expires;
                }

                let entity_id = record.entity_id;
                Self::deposit_event(Event::AnnouncementUpdated {
                    announcement_id,
                    entity_id,
                });
                Ok(())
            })
        }

        /// 撤回公告
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::withdraw_announcement())]
            origin: OriginFor<T>,
            announcement_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Announcements::<T>::try_mutate(announcement_id, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::AnnouncementNotFound)?;

                // H2: 验证权限（管理员或原始发布者）
                T::EntityProvider::entity_owner(record.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(
                    T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE)
                        || record.publisher == who,
                    Error::<T>::NotAdmin
                );
                ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);

                ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

                record.status = AnnouncementStatus::Withdrawn;
                record.is_pinned = false;

                let entity_id = record.entity_id;

                // 如果是置顶公告，从置顶列表移除
                PinnedAnnouncements::<T>::mutate(entity_id, |pinned| {
                    if let Some(pos) = pinned.iter().position(|&id| id == announcement_id) {
                        pinned.swap_remove(pos);
                    }
                });

                Self::deposit_event(Event::AnnouncementWithdrawn {
                    announcement_id,
                    entity_id,
                });
                Ok(())
            })
        }

        /// P2-a14: 置顶公告（支持多个置顶）
        ///
        /// 将指定公告添加到置顶列表。如果已置顶则忽略。
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::pin_announcement())]
            origin: OriginFor<T>,
            entity_id: u64,
            announcement_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let mut record = Announcements::<T>::get(announcement_id)
                .ok_or(Error::<T>::AnnouncementNotFound)?;
            ensure!(record.entity_id == entity_id, Error::<T>::AnnouncementNotFound);
            ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

            if let Some(exp) = record.expires_at {
                let now = <frame_system::Pallet<T>>::block_number();
                ensure!(now <= exp, Error::<T>::AnnouncementExpired);
            }

            PinnedAnnouncements::<T>::try_mutate(entity_id, |pinned| -> DispatchResult {
                if !pinned.contains(&announcement_id) {
                    pinned.try_push(announcement_id).map_err(|_| Error::<T>::PinnedAnnouncementsFull)?;
                }
                Ok(())
            })?;

            record.is_pinned = true;
            Announcements::<T>::insert(announcement_id, record);

            Self::deposit_event(Event::AnnouncementPinned {
                entity_id,
                announcement_id,
            });
            Ok(())
        }

        /// P2-a14: 取消置顶公告
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::unpin_announcement())]
            origin: OriginFor<T>,
            entity_id: u64,
            announcement_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            PinnedAnnouncements::<T>::try_mutate(entity_id, |pinned| -> DispatchResult {
                let pos = pinned.iter().position(|&id| id == announcement_id)
                    .ok_or(Error::<T>::AnnouncementNotPinned)?;
                pinned.swap_remove(pos);
                Ok(())
            })?;

            Announcements::<T>::mutate(announcement_id, |maybe| {
                if let Some(r) = maybe {
                    r.is_pinned = false;
                }
            });

            Self::deposit_event(Event::AnnouncementUnpinned { entity_id });
            Ok(())
        }

        /// 标记已过期公告（任何人可调用）
        ///
        /// 将 `expires_at <= now` 且状态为 Active 的公告标记为 `Expired`。
        /// 如果该公告是置顶公告，自动清除置顶。
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::expire_announcement())]
            origin: OriginFor<T>,
            announcement_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            Announcements::<T>::try_mutate(announcement_id, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::AnnouncementNotFound)?;

                ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

                let now = <frame_system::Pallet<T>>::block_number();
                let exp = record.expires_at.ok_or(Error::<T>::AnnouncementNotExpired)?;
                ensure!(now > exp, Error::<T>::AnnouncementNotExpired);

                record.status = AnnouncementStatus::Expired;
                record.is_pinned = false;

                let entity_id = record.entity_id;

                // 如果是置顶公告，从置顶列表移除
                PinnedAnnouncements::<T>::mutate(entity_id, |pinned| {
                    if let Some(pos) = pinned.iter().position(|&id| id == announcement_id) {
                        pinned.swap_remove(pos);
                    }
                });

                Self::deposit_event(Event::AnnouncementMarkedExpired {
                    announcement_id,
                    entity_id,
                });
                Ok(())
            })
        }

        /// P1-a15: Root 强制配置实体披露设置
        ///
        /// 仅 Root 可调用。可降级披露级别、强制开启内幕交易控制等。
        /// 保留已有 violation_count 和 last_disclosure。
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::force_configure_disclosure())]
            origin: OriginFor<T>,
            entity_id: u64,
            level: DisclosureLevel,
            insider_trading_control: bool,
            blackout_after: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // 验证实体存在
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;

            // H1: 验证 blackout_after 不超过 MaxBlackoutDuration
            let max_blackout = T::MaxBlackoutDuration::get();
            ensure!(blackout_after <= max_blackout, Error::<T>::BlackoutExceedsMax);

            let now = <frame_system::Pallet<T>>::block_number();
            let next_required = Self::calculate_next_disclosure(level, now);

            let existing = DisclosureConfigs::<T>::get(entity_id);
            let existing_violations = existing.as_ref().map(|c| c.violation_count).unwrap_or(0);
            let existing_last = existing.as_ref().map(|c| c.last_disclosure).unwrap_or_else(Zero::zero);

            DisclosureConfigs::<T>::insert(entity_id, DisclosureConfig {
                level,
                insider_trading_control,
                blackout_period_after: blackout_after,
                next_required_disclosure: next_required,
                last_disclosure: existing_last,
                violation_count: existing_violations,
            });

            Self::deposit_event(Event::DisclosureForceConfigured {
                entity_id,
                level,
            });
            Ok(())
        }

        /// P0-a7: 举报披露违规（任何人可调用）
        ///
        /// 检查实体披露是否逾期，若逾期则递增 violation_count 并发出事件。
        /// 同一逾期周期内不可重复举报（通过 ViolationRecords 去重）。
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::report_disclosure_violation())]
            origin: OriginFor<T>,
            entity_id: u64,
            violation_type: ViolationType,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            // 验证实体存在
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;

            // 读取配置
            let config = DisclosureConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::DisclosureNotConfigured)?;

            // 根据违规类型检查条件
            match violation_type {
                ViolationType::LateDisclosure => {
                    let now = <frame_system::Pallet<T>>::block_number();
                    ensure!(now > config.next_required_disclosure, Error::<T>::DisclosureNotOverdue);
                },
                ViolationType::BlackoutTrading => {
                    // 黑窗口期交易违规需要链下证据，此处仅验证当前确实在黑窗口期内
                    ensure!(Self::is_in_blackout(entity_id), Error::<T>::BlackoutNotFound);
                },
                ViolationType::UndisclosedMaterialEvent => {
                    // 未披露重大事件需要链下证据，此处仅验证配置存在
                    // 实际验证由治理流程或链下 worker 补充
                },
            }

            // 防止同一周期重复举报
            let snapshot = config.next_required_disclosure;
            ensure!(
                !ViolationRecords::<T>::get(entity_id, snapshot),
                Error::<T>::ViolationAlreadyRecorded
            );

            // 递增 violation_count
            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(c) = maybe_config {
                    c.violation_count = c.violation_count.saturating_add(1);
                }
            });

            // 标记本周期已举报
            ViolationRecords::<T>::insert(entity_id, snapshot, true);

            let new_count = DisclosureConfigs::<T>::get(entity_id)
                .map(|c| c.violation_count)
                .unwrap_or(0);

            // F6: 检查是否达到高风险阈值
            Self::check_violation_threshold(entity_id, new_count);

            Self::deposit_event(Event::DisclosureViolation {
                entity_id,
                violation_type,
                violation_count: new_count,
            });
            Ok(())
        }

        /// M1-R3: 清理已终结的披露历史索引（任何人可调用）
        ///
        /// 从 `EntityDisclosures` 移除 Withdrawn 或 Corrected 状态的披露 ID，
        /// 释放 BoundedVec 容量供新披露使用。披露记录本身保留供审计。
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::cleanup_disclosure_history())]
            origin: OriginFor<T>,
            entity_id: u64,
            disclosure_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;
            ensure!(record.entity_id == entity_id, Error::<T>::DisclosureNotFound);
            ensure!(
                record.status == DisclosureStatus::Withdrawn || record.status == DisclosureStatus::Corrected,
                Error::<T>::DisclosureNotTerminal
            );

            EntityDisclosures::<T>::mutate(entity_id, |history| {
                if let Some(pos) = history.iter().position(|&id| id == disclosure_id) {
                    history.swap_remove(pos);
                }
            });

            Self::deposit_event(Event::DisclosureHistoryCleaned {
                entity_id,
                disclosure_id,
            });
            Ok(())
        }

        /// M1-R3: 清理已终结的公告历史索引（任何人可调用）
        ///
        /// 从 `EntityAnnouncements` 移除 Withdrawn 或 Expired 状态的公告 ID，
        /// 释放 BoundedVec 容量供新公告使用。公告记录本身保留供审计。
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::cleanup_announcement_history())]
            origin: OriginFor<T>,
            entity_id: u64,
            announcement_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let record = Announcements::<T>::get(announcement_id)
                .ok_or(Error::<T>::AnnouncementNotFound)?;
            ensure!(record.entity_id == entity_id, Error::<T>::AnnouncementNotFound);
            ensure!(
                record.status == AnnouncementStatus::Withdrawn || record.status == AnnouncementStatus::Expired,
                Error::<T>::AnnouncementNotTerminal
            );

            EntityAnnouncements::<T>::mutate(entity_id, |history| {
                if let Some(pos) = history.iter().position(|&id| id == announcement_id) {
                    history.swap_remove(pos);
                }
            });

            Self::deposit_event(Event::AnnouncementHistoryCleaned {
                entity_id,
                announcement_id,
            });
            Ok(())
        }
        /// P1-a20: 清理已关闭实体的披露存储（任何人可调用）
        ///
        /// 仅当实体状态为 Closed/Banned 或实体不存在时允许调用。
        /// 清理: DisclosureConfigs, EntityDisclosures, Insiders, BlackoutPeriods,
        /// EntityAnnouncements, PinnedAnnouncements, ViolationRecords, InsiderRoleHistory。
        /// 披露/公告记录本身保留供审计。
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::cleanup_entity_disclosure())]
        pub fn cleanup_entity_disclosure(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            // 仅允许对已关闭/已封禁/不存在的实体清理
            match T::EntityProvider::entity_status(entity_id) {
                Some(EntityStatus::Closed) | Some(EntityStatus::Banned) | None => {},
                _ => return Err(Error::<T>::EntityNotClosed.into()),
            }

            // 清理实体级存储
            DisclosureConfigs::<T>::remove(entity_id);
            EntityDisclosures::<T>::remove(entity_id);
            Insiders::<T>::remove(entity_id);
            BlackoutPeriods::<T>::remove(entity_id);
            EntityAnnouncements::<T>::remove(entity_id);
            PinnedAnnouncements::<T>::remove(entity_id);
            // ViolationRecords: 移除该实体的所有违规记录
            let _ = ViolationRecords::<T>::clear_prefix(entity_id, u32::MAX, None);
            // P2-a23: 清理内幕人员角色变更历史
            let _ = InsiderRoleHistory::<T>::clear_prefix(entity_id, u32::MAX, None);
            // F4: 清理冷静期记录
            let _ = RemovedInsiders::<T>::clear_prefix(entity_id, u32::MAX, None);
            // F6: 清理高风险标记
            HighRiskEntities::<T>::remove(entity_id);
            // v0.6: 清理新增存储
            ApprovalConfigs::<T>::remove(entity_id);
            FiscalYearConfigs::<T>::remove(entity_id);
            EntityPenalties::<T>::remove(entity_id);
            PausedDeadlines::<T>::remove(entity_id);
            let _ = InsiderTransactionReports::<T>::clear_prefix(entity_id, u32::MAX, None);

            Self::deposit_event(Event::EntityDisclosureCleaned { entity_id });
            Ok(())
        }

        /// F1: 批量添加内幕人员
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::batch_add_insiders(insiders_list.len() as u32))]
        pub fn batch_add_insiders(
            origin: OriginFor<T>,
            entity_id: u64,
            insiders_list: BoundedVec<(T::AccountId, InsiderRole), T::MaxInsiders>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!insiders_list.is_empty(), Error::<T>::EmptyBatch);

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let now = <frame_system::Pallet<T>>::block_number();
            let mut added: u32 = 0;

            Insiders::<T>::try_mutate(entity_id, |insiders| -> DispatchResult {
                for (account, role) in insiders_list.iter() {
                    ensure!(
                        !insiders.iter().any(|i| &i.account == account),
                        Error::<T>::InsiderExists
                    );
                    let record = InsiderRecord {
                        account: account.clone(),
                        role: *role,
                        added_at: now,
                    };
                    insiders.try_push(record).map_err(|_| Error::<T>::InsidersFull)?;
                    added += 1;
                }
                Ok(())
            })?;

            // 记录角色历史
            for (account, role) in insiders_list.iter() {
                InsiderRoleHistory::<T>::try_mutate(entity_id, account, |history| -> DispatchResult {
                    let record = InsiderRoleChangeRecord {
                        old_role: None,
                        new_role: *role,
                        changed_at: now,
                    };
                    history.try_push(record).map_err(|_| Error::<T>::RoleHistoryFull)?;
                    Ok(())
                })?;
            }

            Self::deposit_event(Event::InsidersBatchAdded { entity_id, count: added });
            Ok(())
        }

        /// F1: 批量移除内幕人员
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::batch_remove_insiders(accounts.len() as u32))]
        pub fn batch_remove_insiders(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: BoundedVec<T::AccountId, T::MaxInsiders>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!accounts.is_empty(), Error::<T>::EmptyBatch);

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let removed_count = accounts.len() as u32;

            Insiders::<T>::try_mutate(entity_id, |insiders| -> DispatchResult {
                for account in accounts.iter() {
                    let pos = insiders.iter().position(|i| &i.account == account)
                        .ok_or(Error::<T>::InsiderNotFound)?;
                    insiders.swap_remove(pos);
                }
                Ok(())
            })?;

            // F4: 记录冷静期
            let cooldown = T::InsiderCooldownPeriod::get();
            if !cooldown.is_zero() {
                let now = <frame_system::Pallet<T>>::block_number();
                let until = now.saturating_add(cooldown);
                for account in accounts.iter() {
                    RemovedInsiders::<T>::insert(entity_id, account, until);
                }
            }

            Self::deposit_event(Event::InsidersBatchRemoved { entity_id, count: removed_count });
            Ok(())
        }

        /// F8: 重置违规次数（仅 Root 可调用）
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::reset_violation_count())]
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;

            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.violation_count = 0;
                }
            });

            // F6: 同时清除高风险标记
            HighRiskEntities::<T>::remove(entity_id);

            Self::deposit_event(Event::ViolationCountReset { entity_id });
            Ok(())
        }

        /// F9: 清理已过期的黑窗口期存储（任何人可调用）
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::expire_blackout())]
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let (_, end) = BlackoutPeriods::<T>::get(entity_id)
                .ok_or(Error::<T>::BlackoutNotFound)?;

            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now > end, Error::<T>::BlackoutNotExpired);

            BlackoutPeriods::<T>::remove(entity_id);

            Self::deposit_event(Event::BlackoutExpired { entity_id });
            Ok(())
        }

        // ==================== v0.6 新增 Extrinsics ====================

        /// v0.6: 配置审批要求（多方签核才能发布草稿）
        #[pallet::call_index(28)]
        #[pallet::weight(T::WeightInfo::configure_approval_requirements())]
            origin: OriginFor<T>,
            entity_id: u64,
            required_approvals: u32,
            allowed_roles: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            if required_approvals > 0 {
                ensure!(allowed_roles != 0, Error::<T>::InvalidApprovalRoles);
            }

            if required_approvals == 0 {
                ApprovalConfigs::<T>::remove(entity_id);
            } else {
                ApprovalConfigs::<T>::insert(entity_id, ApprovalConfig {
                    required_approvals,
                    allowed_roles,
                });
            }

            Self::deposit_event(Event::ApprovalRequirementConfigured {
                entity_id,
                required_approvals,
            });
            Ok(())
        }

        /// v0.6: 审批披露草稿（多方签核）
        #[pallet::call_index(29)]
        #[pallet::weight(T::WeightInfo::approve_disclosure())]
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;
            ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

            let entity_id = record.entity_id;

            let approval_config = ApprovalConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ApprovalNotConfigured)?;

            // 验证审批人是内幕人员且角色在允许列表中
            let insider_role = Insiders::<T>::get(entity_id)
                .iter()
                .find(|i| i.account == who)
                .map(|i| i.role)
                .ok_or(Error::<T>::NotApprover)?;

            ensure!(approval_config.role_allowed(insider_role), Error::<T>::NotApprover);

            // 检查是否已审批过
            ensure!(
                !DisclosureApprovals::<T>::get(disclosure_id, &who),
                Error::<T>::AlreadyApproved
            );

            DisclosureApprovals::<T>::insert(disclosure_id, &who, true);
            let new_count = DisclosureApprovalCounts::<T>::mutate(disclosure_id, |c| {
                *c = c.saturating_add(1);
                *c
            });

            Self::deposit_event(Event::DisclosureApproved {
                disclosure_id,
                approver: who,
                approval_count: new_count,
                required: approval_config.required_approvals,
            });
            Ok(())
        }

        /// v0.6: 拒绝披露草稿审批
        #[pallet::call_index(30)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn reject_disclosure(
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;
            ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

            let entity_id = record.entity_id;

            let approval_config = ApprovalConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ApprovalNotConfigured)?;

            let insider_role = Insiders::<T>::get(entity_id)
                .iter()
                .find(|i| i.account == who)
                .map(|i| i.role)
                .ok_or(Error::<T>::NotApprover)?;

            ensure!(approval_config.role_allowed(insider_role), Error::<T>::NotApprover);

            // 拒绝时重置审批计数，要求重新审批
            let _ = DisclosureApprovals::<T>::clear_prefix(disclosure_id, u32::MAX, None);
            DisclosureApprovalCounts::<T>::remove(disclosure_id);

            Self::deposit_event(Event::DisclosureRejected {
                disclosure_id,
                rejector: who,
            });
            Ok(())
        }

        /// v0.6: 发布紧急披露（跳过审批，触发加倍黑窗口期）
        #[pallet::call_index(31)]
        #[pallet::weight(Weight::from_parts(50_000_000, 6_000))]
        pub fn publish_emergency_disclosure(
            origin: OriginFor<T>,
            entity_id: u64,
            disclosure_type: DisclosureType,
            content_cid: Vec<u8>,
            summary_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            Self::validate_disclosure_type(entity_id, disclosure_type)?;

            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);
            let content_bounded: BoundedVec<u8, T::MaxCidLength> =
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
            if let Some(ref s) = summary_cid {
                ensure!(!s.is_empty(), Error::<T>::EmptyCid);
            }
            let summary_bounded = summary_cid
                .map(|s| s.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            let now = <frame_system::Pallet<T>>::block_number();
            let disclosure_id = NextDisclosureId::<T>::get();

            let record = DisclosureRecord {
                id: disclosure_id,
                entity_id,
                disclosure_type,
                content_cid: content_bounded,
                summary_cid: summary_bounded,
                discloser: who.clone(),
                disclosed_at: now,
                status: DisclosureStatus::Published,
                previous_id: None,
            };

            Disclosures::<T>::insert(disclosure_id, record);
            NextDisclosureId::<T>::put(
                disclosure_id.checked_add(1).ok_or(Error::<T>::IdOverflow)?
            );

            EntityDisclosures::<T>::try_mutate(entity_id, |history| -> DispatchResult {
                history.try_push(disclosure_id).map_err(|_| Error::<T>::HistoryFull)?;
                Ok(())
            })?;

            // 存储紧急元数据
            DisclosureMetadataStore::<T>::insert(disclosure_id, DisclosureMetadata {
                period_start: None,
                period_end: None,
                audit_status: AuditStatus::NotRequired,
                is_emergency: true,
            });

            // 更新配置 + 触发加倍黑窗口期
            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.last_disclosure = now;
                    config.next_required_disclosure = Self::calculate_next_disclosure(config.level, now);

                    if !config.blackout_period_after.is_zero() {
                        let multiplier: BlockNumberFor<T> = T::EmergencyBlackoutMultiplier::get().into();
                        let extended = config.blackout_period_after.saturating_mul(multiplier);
                        let max_blackout = T::MaxBlackoutDuration::get();
                        let capped = extended.min(max_blackout);
                        let end_block = now.saturating_add(capped);
                        let actual_end = Self::set_or_extend_blackout(entity_id, now, end_block);
                        Self::deposit_event(Event::BlackoutStarted {
                            entity_id,
                            start_block: now,
                            end_block: actual_end,
                        });
                    }
                }
            });

            Self::deposit_event(Event::EmergencyDisclosurePublished {
                disclosure_id,
                entity_id,
                discloser: who,
            });
            Ok(())
        }

        /// v0.6: 内幕人员交易申报
        #[pallet::call_index(32)]
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn report_insider_transaction(
            origin: OriginFor<T>,
            entity_id: u64,
            transaction_type: InsiderTransactionType,
            token_amount: u128,
            transaction_block: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;

            // 必须是内幕人员或在冷静期内
            let is_current = Self::is_insider(entity_id, &who);
            let in_cooldown = RemovedInsiders::<T>::contains_key(entity_id, &who);
            ensure!(is_current || in_cooldown, Error::<T>::NotInsider);

            let now = <frame_system::Pallet<T>>::block_number();

            let report = InsiderTransactionReport {
                account: who.clone(),
                transaction_type,
                token_amount,
                reported_at: now,
                transaction_block,
            };

            InsiderTransactionReports::<T>::try_mutate(entity_id, &who, |reports| -> DispatchResult {
                reports.try_push(report).map_err(|_| Error::<T>::TransactionHistoryFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::InsiderTransactionReported {
                entity_id,
                account: who,
                transaction_type,
                token_amount,
            });
            Ok(())
        }

        /// v0.6: 配置财务年度
        #[pallet::call_index(33)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn configure_fiscal_year(
            origin: OriginFor<T>,
            entity_id: u64,
            year_start_block: BlockNumberFor<T>,
            year_length: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(!year_length.is_zero(), Error::<T>::ZeroFiscalYearLength);

            FiscalYearConfigs::<T>::insert(entity_id, FiscalYearConfig {
                year_start_block,
                year_length,
            });

            Self::deposit_event(Event::FiscalYearConfigured { entity_id });
            Ok(())
        }

        /// v0.6: 手动升级处罚级别（仅 Root）
        #[pallet::call_index(34)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn escalate_penalty(
            origin: OriginFor<T>,
            entity_id: u64,
            new_level: PenaltyLevel,
        ) -> DispatchResult {
            ensure_root(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;

            let old_level = EntityPenalties::<T>::get(entity_id);
            ensure!(new_level > old_level, Error::<T>::PenaltyAlreadyAtLevel);

            EntityPenalties::<T>::insert(entity_id, new_level);

            T::OnDisclosureViolation::on_violation_threshold_reached(
                entity_id,
                DisclosureConfigs::<T>::get(entity_id)
                    .map(|c| c.violation_count)
                    .unwrap_or(0),
                new_level.as_u8(),
            );

            Self::deposit_event(Event::PenaltyEscalated {
                entity_id,
                old_level,
                new_level,
            });
            Ok(())
        }

        /// v0.6: 重置处罚级别（仅 Root）
        #[pallet::call_index(35)]
        #[pallet::weight(Weight::from_parts(15_000_000, 2_000))]
        pub fn reset_penalty(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;

            EntityPenalties::<T>::remove(entity_id);

            Self::deposit_event(Event::PenaltyReset { entity_id });
            Ok(())
        }

        /// v0.6: 清理已过期的冷静期记录（任何人可调用）
        ///
        /// `max_count` 限制每次最多清理的记录数，防止无界迭代。
        #[pallet::call_index(36)]
        #[pallet::weight(Weight::from_parts(15_000_000u64.saturating_add(
            5_000_000u64.saturating_mul(*max_count as u64)), 3_000))]
        pub fn cleanup_expired_cooldowns(
            origin: OriginFor<T>,
            entity_id: u64,
            max_count: u32,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let mut cleaned = 0u32;

            let mut to_remove = Vec::new();
            for (account, until) in RemovedInsiders::<T>::iter_prefix(entity_id) {
                if cleaned >= max_count {
                    break;
                }
                if now > until {
                    to_remove.push(account);
                    cleaned += 1;
                }
            }

            for account in to_remove.iter() {
                RemovedInsiders::<T>::remove(entity_id, account);
            }

            Self::deposit_event(Event::CooldownsCleaned {
                entity_id,
                count: cleaned,
            });
            Ok(())
        }

        /// v0.6: 设置披露扩展元数据（报告期间、审计要求）
        #[pallet::call_index(37)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn set_disclosure_metadata(
            origin: OriginFor<T>,
            disclosure_id: u64,
            period_start: Option<BlockNumberFor<T>>,
            period_end: Option<BlockNumberFor<T>>,
            requires_audit: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;

            T::EntityProvider::entity_owner(record.entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );

            if let (Some(start), Some(end)) = (period_start, period_end) {
                ensure!(start < end, Error::<T>::InvalidReportingPeriod);
            }

            let audit_status = if requires_audit { AuditStatus::Pending } else { AuditStatus::NotRequired };

            DisclosureMetadataStore::<T>::insert(disclosure_id, DisclosureMetadata {
                period_start,
                period_end,
                audit_status,
                is_emergency: false,
            });

            Ok(())
        }

        /// v0.6: 审计员签核披露（更新审计状态）
        #[pallet::call_index(38)]
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn audit_disclosure(
            origin: OriginFor<T>,
            disclosure_id: u64,
            approved: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;

            let entity_id = record.entity_id;

            // 审计员必须是内幕人员且角色为 Auditor
            let insider_role = Insiders::<T>::get(entity_id)
                .iter()
                .find(|i| i.account == who)
                .map(|i| i.role)
                .ok_or(Error::<T>::NotApprover)?;
            ensure!(insider_role == InsiderRole::Auditor, Error::<T>::NotApprover);

            DisclosureMetadataStore::<T>::try_mutate(disclosure_id, |maybe_meta| -> DispatchResult {
                let meta = maybe_meta.as_mut().ok_or(Error::<T>::DisclosureNotFound)?;
                ensure!(meta.audit_status == AuditStatus::Pending, Error::<T>::InvalidDisclosureStatus);

                meta.audit_status = if approved { AuditStatus::Approved } else { AuditStatus::Rejected };
                Ok(())
            })?;

            if approved {
                Self::deposit_event(Event::DisclosureApproved {
                    disclosure_id,
                    approver: who,
                    approval_count: 1,
                    required: 1,
                });
            } else {
                Self::deposit_event(Event::DisclosureRejected {
                    disclosure_id,
                    rejector: who,
                });
            }
            Ok(())
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        /// 计算下次必须披露时间
        pub fn calculate_next_disclosure(level: DisclosureLevel, now: BlockNumberFor<T>) -> BlockNumberFor<T> {
            let interval = match level {
                DisclosureLevel::Basic => T::BasicDisclosureInterval::get(),
                DisclosureLevel::Standard => T::StandardDisclosureInterval::get(),
                DisclosureLevel::Enhanced => T::EnhancedDisclosureInterval::get(),
                DisclosureLevel::Full => BlockNumberFor::<T>::zero(), // 实时，无固定间隔
            };
            now.saturating_add(interval)
        }

        /// M1-deep: 设置或延长黑窗口期（不缩短已有的结束时间）
        ///
        /// 返回实际写入的 end_block（可能大于传入值）。
        fn set_or_extend_blackout(entity_id: u64, start: BlockNumberFor<T>, end: BlockNumberFor<T>) -> BlockNumberFor<T> {
            let final_end = BlackoutPeriods::<T>::get(entity_id)
                .map(|(_, existing_end)| end.max(existing_end))
                .unwrap_or(end);
            BlackoutPeriods::<T>::insert(entity_id, (start, final_end));
            final_end
        }

        /// 检查是否在黑窗口期内
        pub fn is_in_blackout(entity_id: u64) -> bool {
            if let Some((start, end)) = BlackoutPeriods::<T>::get(entity_id) {
                let now = <frame_system::Pallet<T>>::block_number();
                now >= start && now <= end
            } else {
                false
            }
        }

        /// 检查账户是否是内幕人员
        pub fn is_insider(entity_id: u64, account: &T::AccountId) -> bool {
            // M2-deep: remove_insider 已改为 swap_remove，所有条目均为活跃
            Insiders::<T>::get(entity_id)
                .iter()
                .any(|i| &i.account == account)
        }

        /// 检查内幕人员是否可以交易
        pub fn can_insider_trade(entity_id: u64, account: &T::AccountId) -> bool {
            // 检查是否启用了内幕交易控制
            let config = match DisclosureConfigs::<T>::get(entity_id) {
                Some(c) if c.insider_trading_control => c,
                _ => return true,
            };
            let _ = config; // 确认已启用内幕交易控制

            let is_current_insider = Self::is_insider(entity_id, account);

            // F4: 检查是否在冷静期内（已移除但未过冷静期）
            let in_cooldown = if let Some(until) = RemovedInsiders::<T>::get(entity_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                now <= until
            } else {
                false
            };

            // 非内幕人员且不在冷静期，允许交易
            if !is_current_insider && !in_cooldown {
                return true;
            }

            // 内幕人员或冷静期内，检查是否在黑窗口期内
            !Self::is_in_blackout(entity_id)
        }

        /// 获取实体的披露级别
        pub fn get_disclosure_level(entity_id: u64) -> DisclosureLevel {
            DisclosureConfigs::<T>::get(entity_id)
                .map(|c| c.level)
                .unwrap_or_default()
        }

        /// 检查披露是否逾期
        pub fn is_disclosure_overdue(entity_id: u64) -> bool {
            if let Some(config) = DisclosureConfigs::<T>::get(entity_id) {
                let now = <frame_system::Pallet<T>>::block_number();
                now > config.next_required_disclosure
            } else {
                false
            }
        }

        /// 检查公告是否已过期
        pub fn is_announcement_expired(announcement_id: u64) -> bool {
            if let Some(record) = Announcements::<T>::get(announcement_id) {
                if record.status != AnnouncementStatus::Active {
                    return false;
                }
                if let Some(exp) = record.expires_at {
                    let now = <frame_system::Pallet<T>>::block_number();
                    return now > exp;
                }
            }
            false
        }

        /// F3: 验证披露类型是否允许在当前披露级别下使用
        fn validate_disclosure_type(entity_id: u64, disclosure_type: DisclosureType) -> DispatchResult {
            let level = Self::get_disclosure_level(entity_id);
            let allowed = match level {
                DisclosureLevel::Basic => matches!(disclosure_type,
                    DisclosureType::AnnualReport | DisclosureType::Other),
                DisclosureLevel::Standard => matches!(disclosure_type,
                    DisclosureType::AnnualReport | DisclosureType::QuarterlyReport |
                    DisclosureType::MaterialEvent | DisclosureType::RiskWarning |
                    DisclosureType::Other),
                DisclosureLevel::Enhanced => !matches!(disclosure_type,
                    DisclosureType::TokenIssuance | DisclosureType::Buyback),
                DisclosureLevel::Full => true,
            };
            ensure!(allowed, Error::<T>::DisclosureTypeNotAllowed);
            Ok(())
        }

        /// F5: 获取大股东认定阈值（basis points）
        pub fn get_major_holder_threshold() -> u32 {
            T::MajorHolderThreshold::get()
        }

        /// F6: 检查并标记高风险实体 + v0.6 渐进式处罚自动升级
        fn check_violation_threshold(entity_id: u64, violation_count: u32) {
            let threshold = T::ViolationThreshold::get();
            if threshold > 0 && violation_count >= threshold && !HighRiskEntities::<T>::get(entity_id) {
                HighRiskEntities::<T>::insert(entity_id, true);
                Self::deposit_event(Event::EntityMarkedHighRisk {
                    entity_id,
                    violation_count,
                });
            }

            // v0.6: 渐进式处罚自动升级
            if threshold > 0 {
                let current_penalty = EntityPenalties::<T>::get(entity_id);
                let new_penalty = if violation_count >= threshold.saturating_mul(3) {
                    PenaltyLevel::Delisted
                } else if violation_count >= threshold.saturating_mul(2) {
                    PenaltyLevel::Suspended
                } else if violation_count >= threshold {
                    PenaltyLevel::Restricted
                } else if violation_count >= threshold / 2 {
                    PenaltyLevel::Warning
                } else {
                    PenaltyLevel::None
                };

                if new_penalty > current_penalty {
                    EntityPenalties::<T>::insert(entity_id, new_penalty);
                    Self::deposit_event(Event::PenaltyEscalated {
                        entity_id,
                        old_level: current_penalty,
                        new_level: new_penalty,
                    });

                    T::OnDisclosureViolation::on_violation_threshold_reached(
                        entity_id,
                        violation_count,
                        new_penalty.as_u8(),
                    );
                }
            }
        }

        // ==================== v0.6 新增辅助函数 ====================

        /// v0.6: 暂停披露截止时间（保存剩余区块数）
        fn pause_deadline(entity_id: u64) {
            if let Some(config) = DisclosureConfigs::<T>::get(entity_id) {
                if PausedDeadlines::<T>::contains_key(entity_id) {
                    return;
                }
                let now = <frame_system::Pallet<T>>::block_number();
                let remaining = if config.next_required_disclosure > now {
                    config.next_required_disclosure.saturating_sub(now)
                } else {
                    Zero::zero()
                };
                PausedDeadlines::<T>::insert(entity_id, (now, remaining));
                Self::deposit_event(Event::DeadlinePaused { entity_id });
            }
        }

        /// v0.6: 恢复披露截止时间（基于暂停时保存的剩余区块数）
        fn resume_deadline(entity_id: u64) {
            if let Some((_, remaining)) = PausedDeadlines::<T>::take(entity_id) {
                let now = <frame_system::Pallet<T>>::block_number();
                let new_deadline = now.saturating_add(remaining);
                DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                    if let Some(config) = maybe_config {
                        config.next_required_disclosure = new_deadline;
                    }
                });
                Self::deposit_event(Event::DeadlineResumed {
                    entity_id,
                    new_deadline,
                });
            }
        }

        /// v0.6: 将账户注册为大股东内幕人员
        fn do_register_major_holder(entity_id: u64, account: &T::AccountId) -> sp_runtime::DispatchResult {
            Insiders::<T>::try_mutate(entity_id, |insiders| -> sp_runtime::DispatchResult {
                if insiders.iter().any(|i| &i.account == account) {
                    return Ok(());
                }

                let now = <frame_system::Pallet<T>>::block_number();
                let record = InsiderRecord {
                    account: account.clone(),
                    role: InsiderRole::MajorHolder,
                    added_at: now,
                };
                insiders.try_push(record).map_err(|_| Error::<T>::InsidersFull)?;

                InsiderRoleHistory::<T>::try_mutate(entity_id, account, |history| -> sp_runtime::DispatchResult {
                    let change = InsiderRoleChangeRecord {
                        old_role: None,
                        new_role: InsiderRole::MajorHolder,
                        changed_at: now,
                    };
                    history.try_push(change).map_err(|_| Error::<T>::RoleHistoryFull)?;
                    Ok(())
                })?;

                Self::deposit_event(Event::MajorHolderRegistered {
                    entity_id,
                    account: account.clone(),
                });
                Ok(())
            })
        }

        /// v0.6: 注销大股东内幕人员身份
        fn do_deregister_major_holder(entity_id: u64, account: &T::AccountId) -> sp_runtime::DispatchResult {
            Insiders::<T>::try_mutate(entity_id, |insiders| -> sp_runtime::DispatchResult {
                let pos = insiders.iter().position(|i| &i.account == account && i.role == InsiderRole::MajorHolder);
                if let Some(idx) = pos {
                    insiders.swap_remove(idx);

                    let cooldown = T::InsiderCooldownPeriod::get();
                    if !cooldown.is_zero() {
                        let now = <frame_system::Pallet<T>>::block_number();
                        let until = now.saturating_add(cooldown);
                        RemovedInsiders::<T>::insert(entity_id, account, until);
                    }

                    Self::deposit_event(Event::MajorHolderDeregistered {
                        entity_id,
                        account: account.clone(),
                    });
                }
                Ok(())
            })
        }

        /// P2-a14: 获取所有有效的置顶公告 ID
        pub fn get_active_pinned_announcements(entity_id: u64) -> Vec<u64> {
            let now = <frame_system::Pallet<T>>::block_number();
            PinnedAnnouncements::<T>::get(entity_id)
                .into_iter()
                .filter(|&id| {
                    if let Some(record) = Announcements::<T>::get(id) {
                        if record.status != AnnouncementStatus::Active {
                            return false;
                        }
                        if let Some(exp) = record.expires_at {
                            if now > exp {
                                return false;
                            }
                        }
                        true
                    } else {
                        false
                    }
                })
                .collect()
        }
    }

    // ==================== DisclosureProvider 实现 ====================

    impl<T: Config> DisclosureProvider<T::AccountId> for Pallet<T> {
        fn is_in_blackout(entity_id: u64) -> bool {
            Pallet::<T>::is_in_blackout(entity_id)
        }

        fn is_insider(entity_id: u64, account: &T::AccountId) -> bool {
            Pallet::<T>::is_insider(entity_id, account)
        }

        fn can_insider_trade(entity_id: u64, account: &T::AccountId) -> bool {
            Pallet::<T>::can_insider_trade(entity_id, account)
        }

        fn get_disclosure_level(entity_id: u64) -> DisclosureLevel {
            Pallet::<T>::get_disclosure_level(entity_id)
        }

        fn is_disclosure_overdue(entity_id: u64) -> bool {
            Pallet::<T>::is_disclosure_overdue(entity_id)
        }

        fn get_violation_count(entity_id: u64) -> u32 {
            DisclosureConfigs::<T>::get(entity_id)
                .map(|c| c.violation_count)
                .unwrap_or(0)
        }

        fn get_insider_role(entity_id: u64, account: &T::AccountId) -> Option<u8> {
            Insiders::<T>::get(entity_id)
                .iter()
                .find(|i| &i.account == account)
                .map(|i| i.role as u8)
        }

        fn is_disclosure_configured(entity_id: u64) -> bool {
            DisclosureConfigs::<T>::contains_key(entity_id)
        }

        fn is_high_risk(entity_id: u64) -> bool {
            HighRiskEntities::<T>::get(entity_id)
        }

        fn governance_configure_disclosure(
            entity_id: u64,
            level: DisclosureLevel,
            insider_trading_control: bool,
            blackout_period_after: u64,
        ) -> sp_runtime::DispatchResult {
            let blackout_after: BlockNumberFor<T> = blackout_period_after.saturated_into();
            // M1-audit: 验证 blackout_period_after 不超过上限
            let max_blackout: BlockNumberFor<T> = T::MaxBlackoutDuration::get();
            frame_support::ensure!(
                blackout_after <= max_blackout,
                Error::<T>::BlackoutExceedsMax
            );
            let now = <frame_system::Pallet<T>>::block_number();
            let next_required = Pallet::<T>::calculate_next_disclosure(level, now);

            let existing = DisclosureConfigs::<T>::get(entity_id);
            let existing_violations = existing.as_ref().map(|c| c.violation_count).unwrap_or(0);
            let existing_last = existing.as_ref().map(|c| c.last_disclosure).unwrap_or_else(Zero::zero);

            DisclosureConfigs::<T>::insert(entity_id, DisclosureConfig {
                level,
                insider_trading_control,
                blackout_period_after: blackout_after,
                next_required_disclosure: next_required,
                last_disclosure: existing_last,
                violation_count: existing_violations,
            });

            Pallet::<T>::deposit_event(Event::DisclosureConfigUpdated {
                entity_id,
                level,
            });
            Ok(())
        }

        fn governance_reset_violations(entity_id: u64) -> sp_runtime::DispatchResult {
            DisclosureConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.violation_count = 0;
                }
            });
            HighRiskEntities::<T>::remove(entity_id);

            Pallet::<T>::deposit_event(Event::ViolationCountReset { entity_id });
            Ok(())
        }

        // ==================== v0.6: 大股东自动注册 ====================

        fn register_major_holder(entity_id: u64, account: &T::AccountId) -> sp_runtime::DispatchResult {
            if !DisclosureConfigs::<T>::contains_key(entity_id) {
                return Ok(());
            }
            Pallet::<T>::do_register_major_holder(entity_id, account)
        }

        fn deregister_major_holder(entity_id: u64, account: &T::AccountId) -> sp_runtime::DispatchResult {
            Pallet::<T>::do_deregister_major_holder(entity_id, account)
        }

        // ==================== v0.6: 渐进式处罚 ====================

        fn get_penalty_level(entity_id: u64) -> u8 {
            EntityPenalties::<T>::get(entity_id).as_u8()
        }

        fn is_penalty_active(entity_id: u64) -> bool {
            EntityPenalties::<T>::get(entity_id) >= PenaltyLevel::Restricted
        }
    }

    // ==================== v0.6: OnEntityStatusChange 实现 ====================

    impl<T: Config> OnEntityStatusChange for Pallet<T> {
        fn on_entity_suspended(entity_id: u64) {
            Self::pause_deadline(entity_id);
        }

        fn on_entity_banned(entity_id: u64) {
            Self::pause_deadline(entity_id);
        }

        fn on_entity_resumed(entity_id: u64) {
            Self::resume_deadline(entity_id);
        }

        fn on_entity_closed(entity_id: u64) {
            PausedDeadlines::<T>::remove(entity_id);
        }
    }
}
