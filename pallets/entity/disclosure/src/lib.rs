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

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
    use pallet_entity_common::{AdminPermission, DisclosureProvider, EntityProvider, EntityStatus};
    use sp_runtime::traits::{Saturating, Zero};

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
            let base_weight = Weight::from_parts(5_000_000, 1_000);

            if remaining_weight.ref_time() < base_weight.ref_time() {
                return Weight::zero();
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let mut scanned = 0u32;
            let mut used_weight = base_weight;

            for (entity_id, config) in DisclosureConfigs::<T>::iter() {
                if scanned >= MAX_SCAN {
                    break;
                }
                let needed = used_weight.saturating_add(per_entity_weight);
                if needed.ref_time() > remaining_weight.ref_time() {
                    break;
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

                        Self::deposit_event(Event::AutoViolationDetected {
                            entity_id,
                            violation_count: new_count,
                        });
                    }
                }

                scanned += 1;
                used_weight = needed;
            }

            used_weight
        }
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 配置实体披露设置
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
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
        #[pallet::weight(Weight::from_parts(50_000_000, 6_000))]
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
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn create_draft_disclosure(
            origin: OriginFor<T>,
            entity_id: u64,
            disclosure_type: DisclosureType,
            content_cid: Vec<u8>,
            summary_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                Error::<T>::NotAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn update_draft(
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
        #[pallet::weight(Weight::from_parts(15_000_000, 2_000))]
        pub fn delete_draft(
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let record = Disclosures::<T>::get(disclosure_id)
                .ok_or(Error::<T>::DisclosureNotFound)?;
            ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

            T::EntityProvider::entity_owner(record.entity_id).ok_or(Error::<T>::EntityNotFound)?;
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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn publish_draft(
            origin: OriginFor<T>,
            disclosure_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = Disclosures::<T>::try_mutate(disclosure_id, |maybe_record| -> Result<u64, DispatchError> {
                let record = maybe_record.as_mut().ok_or(Error::<T>::DisclosureNotFound)?;
                ensure!(record.status == DisclosureStatus::Draft, Error::<T>::DisclosureNotDraft);

                T::EntityProvider::entity_owner(record.entity_id).ok_or(Error::<T>::EntityNotFound)?;
                ensure!(
                    T::EntityProvider::is_entity_admin(record.entity_id, &who, AdminPermission::DISCLOSURE_MANAGE),
                    Error::<T>::NotAdmin
                );
                ensure!(!T::EntityProvider::is_entity_locked(record.entity_id), Error::<T>::EntityLocked);

                let now = <frame_system::Pallet<T>>::block_number();
                record.status = DisclosureStatus::Published;
                record.disclosed_at = now;

                Ok(record.entity_id)
            })?;

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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn withdraw_disclosure(
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
        #[pallet::weight(Weight::from_parts(50_000_000, 6_000))]
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
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn add_insider(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            role: InsiderRole,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
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
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn update_insider_role(
            origin: OriginFor<T>,
            entity_id: u64,
            account: T::AccountId,
            new_role: InsiderRole,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
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
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn remove_insider(
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

            Self::deposit_event(Event::InsiderRemoved {
                entity_id,
                account,
            });
            Ok(())
        }

        /// 手动开始黑窗口期
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(20_000_000, 2_000))]
        pub fn start_blackout(
            origin: OriginFor<T>,
            entity_id: u64,
            duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2: 验证实体存在 + 管理员权限
            T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
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
        #[pallet::weight(Weight::from_parts(20_000_000, 2_000))]
        pub fn end_blackout(
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
        #[pallet::weight(Weight::from_parts(40_000_000, 5_000))]
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
        #[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn withdraw_announcement(
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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn pin_announcement(
            origin: OriginFor<T>,
            entity_id: u64,
            announcement_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
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
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn unpin_announcement(
            origin: OriginFor<T>,
            entity_id: u64,
            announcement_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn expire_announcement(
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
        #[pallet::weight(Weight::from_parts(25_000_000, 3_000))]
        pub fn force_configure_disclosure(
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
        #[pallet::weight(Weight::from_parts(20_000_000, 3_000))]
        pub fn report_disclosure_violation(
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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn cleanup_disclosure_history(
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
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn cleanup_announcement_history(
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
        #[pallet::weight(Weight::from_parts(50_000_000, 10_000))]
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

            Self::deposit_event(Event::EntityDisclosureCleaned { entity_id });
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
            // 如果不是内幕人员，允许交易
            if !Self::is_insider(entity_id, account) {
                return true;
            }

            // 检查是否启用了内幕交易控制
            if let Some(config) = DisclosureConfigs::<T>::get(entity_id) {
                if !config.insider_trading_control {
                    return true;
                }
            } else {
                return true;
            }

            // 检查是否在黑窗口期内
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
    }
}
