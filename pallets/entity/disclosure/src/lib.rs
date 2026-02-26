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
    use pallet_entity_common::EntityProvider;
    use sp_runtime::traits::{Saturating, Zero};

    // ==================== 类型定义 ====================

    /// 披露级别
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum DisclosureLevel {
        /// 基础披露（年度简报）
        #[default]
        Basic,
        /// 标准披露（季度报告）
        Standard,
        /// 增强披露（月度报告 + 重大事件）
        Enhanced,
        /// 完全披露（实时 + 详细财务）
        Full,
    }

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
        /// 待审核
        #[default]
        Pending,
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
        /// 验证者 — 预留字段，待后续版本实现验证流程
        pub verifier: Option<AccountId>,
        /// 验证时间 — 预留字段
        pub verified_at: Option<BlockNumber>,
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
        /// 披露前黑窗口期（区块数）— 预留，当前仅 blackout_period_after 生效
        pub blackout_period_before: BlockNumber,
        /// 披露后黑窗口期（区块数）
        pub blackout_period_after: BlockNumber,
        /// 下次必须披露时间
        pub next_required_disclosure: BlockNumber,
        /// 上次披露时间
        pub last_disclosure: BlockNumber,
        /// 连续违规次数 — 预留，当前未自动递增（需外部 hook 或 off-chain 触发）
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
        /// 是否活跃
        pub active: bool,
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

        /// 大股东阈值（基点，如 500 = 5%）— 预留，供外部模块判断 MajorHolder 角色
        #[pallet::constant]
        type MajorHolderThreshold: Get<u16>;

        /// 黑窗口期最大时长（区块数）
        #[pallet::constant]
        type MaxBlackoutDuration: Get<BlockNumberFor<Self>>;

        /// 每个实体最大公告数
        #[pallet::constant]
        type MaxAnnouncementHistory: Get<u32>;

        /// 公告标题最大长度（字节）
        #[pallet::constant]
        type MaxTitleLength: Get<u32>;
    }

    #[pallet::pallet]
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

    /// 实体置顶公告 (每个实体最多一个置顶)
    #[pallet::storage]
    #[pallet::getter(fn pinned_announcement)]
    pub type PinnedAnnouncement<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        u64,  // announcement_id
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
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
        /// 披露违规 — 预留事件，当前未自动发出（需 off-chain worker 或外部 hook 检测违规后调用）
        DisclosureViolation {
            entity_id: u64,
            violation_type: ViolationType,
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
            blackout_before: BlockNumberFor<T>,
            blackout_after: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证管理员权限
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            // H1: 验证 blackout_after 不超过 MaxBlackoutDuration
            let max_blackout = T::MaxBlackoutDuration::get();
            ensure!(blackout_after <= max_blackout, Error::<T>::BlackoutExceedsMax);

            let now = <frame_system::Pallet<T>>::block_number();
            let next_required = Self::calculate_next_disclosure(level, now);

            // D-L1 审计修复: 保留已有 violation_count 和 last_disclosure，防止通过重新配置清除
            let existing = DisclosureConfigs::<T>::get(entity_id);
            let existing_violations = existing.as_ref().map(|c| c.violation_count).unwrap_or(0);
            // H5: 保留已有 last_disclosure，不伪造为 now
            let existing_last = existing.as_ref().map(|c| c.last_disclosure).unwrap_or_else(Zero::zero);

            DisclosureConfigs::<T>::insert(entity_id, DisclosureConfig {
                level,
                insider_trading_control,
                blackout_period_before: blackout_before,
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

            // 验证管理员权限
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            // H2: 内容 CID 不能为空
            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);

            let content_bounded: BoundedVec<u8, T::MaxCidLength> = 
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
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
                verifier: None,
                verified_at: None,
            };

            // 保存记录
            Disclosures::<T>::insert(disclosure_id, record);
            NextDisclosureId::<T>::put(disclosure_id.saturating_add(1));

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
                        BlackoutPeriods::<T>::insert(entity_id, (now, end_block));
                        Self::deposit_event(Event::BlackoutStarted {
                            entity_id,
                            start_block: now,
                            end_block,
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
                
                // 验证权限
                let owner = T::EntityProvider::entity_owner(record.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(owner == who || record.discloser == who, Error::<T>::NotAdmin);
                
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

            // 验证权限
            let owner = T::EntityProvider::entity_owner(old_record.entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            // H2/H3: 内容 CID 不能为空
            ensure!(!content_cid.is_empty(), Error::<T>::EmptyCid);

            let content_bounded: BoundedVec<u8, T::MaxCidLength> = 
                content_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;
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
                verifier: None,
                verified_at: None,
            };

            // 更新旧记录状态
            Disclosures::<T>::mutate(old_disclosure_id, |maybe_record| {
                if let Some(record) = maybe_record {
                    record.status = DisclosureStatus::Corrected;
                }
            });

            // 保存新记录
            Disclosures::<T>::insert(new_disclosure_id, new_record);
            NextDisclosureId::<T>::put(new_disclosure_id.saturating_add(1));

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
                        BlackoutPeriods::<T>::insert(entity_id, (now, end_block));
                        Self::deposit_event(Event::BlackoutStarted {
                            entity_id,
                            start_block: now,
                            end_block,
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

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            let now = <frame_system::Pallet<T>>::block_number();

            Insiders::<T>::try_mutate(entity_id, |insiders| -> DispatchResult {
                // 检查是否已活跃
                ensure!(
                    !insiders.iter().any(|i| i.account == account && i.active),
                    Error::<T>::InsiderExists
                );

                // H3: 尝试重新激活已存在的非活跃记录
                if let Some(existing) = insiders.iter_mut().find(|i| i.account == account && !i.active) {
                    existing.role = role;
                    existing.added_at = now;
                    existing.active = true;
                } else {
                    let record = InsiderRecord {
                        account: account.clone(),
                        role,
                        added_at: now,
                        active: true,
                    };
                    insiders.try_push(record).map_err(|_| Error::<T>::InsidersFull)?;
                }
                Ok(())
            })?;

            Self::deposit_event(Event::InsiderAdded {
                entity_id,
                account,
                role,
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

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            Insiders::<T>::try_mutate(entity_id, |insiders| -> DispatchResult {
                let pos = insiders.iter().position(|i| i.account == account && i.active)
                    .ok_or(Error::<T>::InsiderNotFound)?;
                insiders[pos].active = false;
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

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            // M3: 使用 Config 常量替代硬编码
            let max_blackout = T::MaxBlackoutDuration::get();
            ensure!(!duration.is_zero() && duration <= max_blackout, Error::<T>::InvalidBlackoutDuration);

            let now = <frame_system::Pallet<T>>::block_number();
            let end_block = now.saturating_add(duration);

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

            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

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

            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

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
            NextAnnouncementId::<T>::put(announcement_id.saturating_add(1));

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

            Announcements::<T>::try_mutate(announcement_id, |maybe_record| -> DispatchResult {
                let record = maybe_record.as_mut().ok_or(Error::<T>::AnnouncementNotFound)?;

                let owner = T::EntityProvider::entity_owner(record.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(owner == who, Error::<T>::NotAdmin);

                ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

                let now = <frame_system::Pallet<T>>::block_number();

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

                let owner = T::EntityProvider::entity_owner(record.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(owner == who || record.publisher == who, Error::<T>::NotAdmin);

                ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

                record.status = AnnouncementStatus::Withdrawn;
                record.is_pinned = false;

                let entity_id = record.entity_id;

                // 如果是置顶公告，清除置顶
                if PinnedAnnouncement::<T>::get(entity_id) == Some(announcement_id) {
                    PinnedAnnouncement::<T>::remove(entity_id);
                }

                Self::deposit_event(Event::AnnouncementWithdrawn {
                    announcement_id,
                    entity_id,
                });
                Ok(())
            })
        }

        /// 置顶/取消置顶公告
        ///
        /// - `announcement_id = Some(id)`: 置顶指定公告（自动替换旧置顶）
        /// - `announcement_id = None`: 取消当前置顶
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(25_000_000, 4_000))]
        pub fn pin_announcement(
            origin: OriginFor<T>,
            entity_id: u64,
            announcement_id: Option<u64>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who, Error::<T>::NotAdmin);

            match announcement_id {
                Some(id) => {
                    // 验证公告存在且属于该实体
                    let mut record = Announcements::<T>::get(id)
                        .ok_or(Error::<T>::AnnouncementNotFound)?;
                    ensure!(record.entity_id == entity_id, Error::<T>::AnnouncementNotFound);
                    ensure!(record.status == AnnouncementStatus::Active, Error::<T>::AnnouncementNotActive);

                    // 取消旧置顶
                    if let Some(old_pinned_id) = PinnedAnnouncement::<T>::get(entity_id) {
                        if old_pinned_id != id {
                            Announcements::<T>::mutate(old_pinned_id, |maybe| {
                                if let Some(r) = maybe {
                                    r.is_pinned = false;
                                }
                            });
                        }
                    }

                    // 设置新置顶
                    record.is_pinned = true;
                    Announcements::<T>::insert(id, record);
                    PinnedAnnouncement::<T>::insert(entity_id, id);

                    Self::deposit_event(Event::AnnouncementPinned {
                        entity_id,
                        announcement_id: id,
                    });
                },
                None => {
                    // 取消置顶
                    if let Some(old_pinned_id) = PinnedAnnouncement::<T>::take(entity_id) {
                        Announcements::<T>::mutate(old_pinned_id, |maybe| {
                            if let Some(r) = maybe {
                                r.is_pinned = false;
                            }
                        });
                    }

                    Self::deposit_event(Event::AnnouncementUnpinned { entity_id });
                },
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
            Insiders::<T>::get(entity_id)
                .iter()
                .any(|i| &i.account == account && i.active)
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

        /// 获取实体的置顶公告 ID
        pub fn get_pinned_announcement(entity_id: u64) -> Option<u64> {
            PinnedAnnouncement::<T>::get(entity_id)
        }
    }
}
