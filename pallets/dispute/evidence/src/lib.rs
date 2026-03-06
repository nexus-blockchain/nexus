#![cfg_attr(not(feature = "std"), no_std)]
// 函数级中文注释：允许未使用的导入（trait方法调用）
#![allow(unused_imports)]

extern crate alloc;

pub use pallet::*;
use sp_core::Get;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// 函数级中文注释：导入log用于记录自动pin失败的警告
extern crate log;
// 函数级中文注释：导入pallet_memo_ipfs用于StoragePin trait
extern crate pallet_storage_service;
use pallet_storage_service::StoragePin;
extern crate pallet_crypto_common;

// 函数级中文注释：权重模块导入，提供 WeightInfo 接口用于基于输入规模计算交易权重。
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod private_content;
pub mod weights;
// L-4修复：CID加密验证模块
pub mod cid_validator;

#[allow(deprecated)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::{
        private_content::{EncryptedKeyBundles, UserPublicKey},
        weights::WeightInfo,
    };
    use alloc::collections::BTreeSet;
    use alloc::vec::Vec;
    use frame_support::{pallet_prelude::*, BoundedVec};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_core::blake2_256;
    use sp_core::H256;
    use sp_runtime::traits::{Saturating, AtLeast32BitUnsigned, SaturatedConversion};
    use frame_support::weights::Weight;
    // 导入共享媒体工具库
    use media_utils::{
        HashHelper, IpfsHelper, MediaError
    };
    // 导入CID验证器trait
    use crate::cid_validator::{CidValidator, DefaultCidValidator};

    /// Phase 1.5优化：证据内容类型枚举
    /// 
    /// 函数级中文注释：标识证据的内容类型
    /// - 用于前端渲染和验证
    /// - 支持单一类型和混合类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
    pub enum ContentType {
        /// 图片证据（单张或多张）
        Image,
        /// 视频证据（单个或多个）
        Video,
        /// 文档证据（单个或多个）
        Document,
        /// 混合类型（图片+视频+文档）
        Mixed,
        /// 纯文本描述
        Text,
    }

    /// P1-1: 证据状态枚举
    ///
    /// 支持证据生命周期管理：活跃、已撤回、已密封（仲裁冻结）、已移除（强制）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug, Default)]
    pub enum EvidenceStatus {
        /// 活跃状态（正常可用）
        #[default]
        Active,
        /// 已撤回（提交者主动撤回，不可再用于仲裁）
        Withdrawn,
        /// 已密封（仲裁期间冻结，不可修改/撤回/取消链接）
        Sealed,
        /// 已移除（Root强制移除违规内容）
        Removed,
    }

    /// 存储膨胀防护：归档证据摘要（精简版，~50字节）
    /// 
    /// 函数级详细中文注释：
    /// - 原始 Evidence 结构约 200+ 字节
    /// - 归档后仅保留关键摘要信息
    /// - 存储降低约 75%
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug, Default)]
    pub struct ArchivedEvidence {
        /// 证据ID
        pub id: u64,
        /// 所属域
        pub domain: u8,
        /// 目标ID
        pub target_id: u64,
        /// 内容哈希摘要（blake2_256(content_cid)）
        pub content_hash: H256,
        /// 内容类型
        pub content_type: u8,
        /// 创建时间（区块号，u32足够）
        pub created_at: u32,
        /// 归档时间（区块号）
        pub archived_at: u32,
        /// 年月（YYMM格式，便于按月统计）
        pub year_month: u16,
    }

    /// Phase 1.5优化：共享证据记录结构（CID化版本）
    /// 
    /// 函数级详细中文注释：
    /// **核心优化**：
    /// - 旧版：链上存储所有CID数组（imgs, vids, docs）
    /// - 新版：链上只存储单一content_cid，实际内容存IPFS
    /// 
    /// **存储成本对比**：
    /// - 旧版：840字节（10张图片）
    /// - 新版：214字节（仅元数据+CID引用）
    /// - **降低74.5%** ⭐
    /// 
    /// **IPFS内容格式**（JSON）：
    /// ```json
    /// {
    ///   "version": "1.0",
    ///   "evidence_id": 123,
    ///   "domain": 2,
    ///   "target_id": 456,
    ///   "content": {
    ///     "images": ["QmXxx1", "QmXxx2", ...],
    ///     "videos": ["QmYyy1", ...],
    ///     "documents": ["QmZzz1", ...],
    ///     "memo": "可选文字说明"
    ///   },
    ///   "metadata": {
    ///     "created_at": 1234567890,
    ///     "owner": "5GrwvaEF...",
    ///     "encryption": {
    ///       "enabled": true,
    ///       "scheme": "aes256-gcm",
    ///       "key_bundles": {...}
    ///     }
    ///   }
    /// }
    /// ```
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(MaxContentCidLen, MaxSchemeLen))]
    pub struct Evidence<
        AccountId,
        BlockNumber,
        MaxContentCidLen: Get<u32>,
        MaxSchemeLen: Get<u32>,
    > {
        /// 证据唯一ID
        pub id: u64,
        /// 所属域（1=Evidence, 2=OtcOrder, 3=General, etc.）
        pub domain: u8,
        /// 目标ID（如subject_id）
        pub target_id: u64,
        /// 证据所有者
        pub owner: AccountId,
        
        /// Phase 1.5优化：核心字段 - IPFS内容CID
        /// - 指向IPFS上的JSON文件
        /// - 包含所有图片/视频/文档的CID数组
        /// - 链上只存64字节CID引用
        pub content_cid: BoundedVec<u8, MaxContentCidLen>,
        
        /// Phase 1.5优化：内容类型标识
        /// - 便于前端快速识别和渲染
        /// - 无需下载IPFS内容即可知道类型
        pub content_type: ContentType,
        
        /// 创建时间（区块号）
        pub created_at: BlockNumber,
        
        /// Phase 1.5优化：加密标识
        /// - true: content_cid指向的内容已加密
        /// - false: 公开内容
        pub is_encrypted: bool,
        
        /// Phase 1.5优化：加密方案描述（可选）
        /// - 例如："aes256-gcm", "xchacha20-poly1305"
        /// - 用于解密时选择正确的算法
        pub encryption_scheme: Option<BoundedVec<u8, MaxSchemeLen>>,
        
        /// 证据承诺（commit），例如 H(ns || subject_id || cid_enc || salt || ver)
        pub commit: Option<H256>,
        
        /// 命名空间（8字节），用于授权与分域检索
        pub ns: Option<[u8; 8]>,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config + TypeInfo + core::fmt::Debug {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        // Phase 1.5优化：新的泛型参数（CID化版本）
        /// 内容CID最大长度（IPFS CID，建议64字节）
        #[pallet::constant]
        type MaxContentCidLen: Get<u32>;
        /// 加密方案描述最大长度（建议32字节）
        #[pallet::constant]
        type MaxSchemeLen: Get<u32>;
        
        // 旧版泛型参数（保留以向后兼容旧API）
        #[pallet::constant]
        type MaxCidLen: Get<u32>;
        #[pallet::constant]
        type MaxImg: Get<u32>;
        #[pallet::constant]
        type MaxVid: Get<u32>;
        #[pallet::constant]
        type MaxDoc: Get<u32>;
        #[pallet::constant]
        type MaxMemoLen: Get<u32>;
        #[pallet::constant]
        type MaxAuthorizedUsers: Get<u32>;
        #[pallet::constant]
        type MaxKeyLen: Get<u32>;
        #[pallet::constant]
        type EvidenceNsBytes: Get<[u8; 8]>;
        type Authorizer: EvidenceAuthorizer<Self::AccountId>;
        #[pallet::constant]
        type MaxPerSubjectTarget: Get<u32>;
        #[pallet::constant]
        type MaxPerSubjectNs: Get<u32>;
        #[pallet::constant]
        type WindowBlocks: Get<BlockNumberFor<Self>>;
        #[pallet::constant]
        type MaxPerWindow: Get<u32>;
        #[pallet::constant]
        type EnableGlobalCidDedup: Get<bool>;
        #[pallet::constant]
        type MaxListLen: Get<u32>;
        type WeightInfo: WeightInfo;
        
        // ============= IPFS自动Pin相关配置 =============
        /// 函数级详细中文注释：IPFS自动pin提供者，供证据CID自动固定
        /// 
        /// 集成目标：
        /// - imgs: 图片证据CID列表自动pin
        /// - vids: 视频证据CID列表自动pin
        /// - docs: 文档证据CID列表自动pin
        /// 
        /// 使用场景：
        /// - commit: 提交证据时自动pin所有CID
        /// 
        /// 注意：
        /// - 证据通常关联到target_id（如subject_id）
        /// - 由Runtime注入实现（pallet_storage_service::Pallet<Runtime>）
        type StoragePin: pallet_storage_service::StoragePin<Self::AccountId>;
        
        /// 函数级中文注释：余额类型（用于IPFS存储费用支付）
        type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
        
        /// 函数级中文注释：默认IPFS存储单价（每副本每月）
        #[pallet::constant]
        type DefaultStoragePrice: Get<Self::Balance>;

        /// 🆕 证据修改窗口（区块数，28800 ≈ 2天，按6秒/块计算）
        #[pallet::constant]
        type EvidenceEditWindow: Get<BlockNumberFor<Self>>;

        /// 🆕 防膨胀: 归档记录 TTL（区块数，超过此值的归档记录将被清理）
        /// 默认 2_592_000 ≈ 180天 (6s/block)
        #[pallet::constant]
        type ArchiveTtlBlocks: Get<u32>;

        /// 🆕 M-NEW-5修复: 归档延迟（区块数），证据创建后多久可归档
        /// 默认 1_296_000 ≈ 90天 (6s/block)
        #[pallet::constant]
        type ArchiveDelayBlocks: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type NextEvidenceId<T: Config> = StorageValue<_, u64, ValueQuery>;
    #[pallet::storage]
    pub type Evidences<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        Evidence<T::AccountId, BlockNumberFor<T>, T::MaxContentCidLen, T::MaxSchemeLen>,
        OptionQuery,
    >;
    #[pallet::storage]
    pub type EvidenceByTarget<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, (u8, u64), Blake2_128Concat, u64, (), OptionQuery>;

    /// 新增：按命名空间+主体键值引用证据 id（便于按 ns/subject_id 聚合）
    #[pallet::storage]
    pub type EvidenceByNs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ([u8; 8], u64),
        Blake2_128Concat,
        u64,
        (),
        OptionQuery,
    >;

    /// 新增：承诺哈希到 EvidenceId 的唯一索引，防止重复提交
    #[pallet::storage]
    pub type CommitIndex<T: Config> = StorageMap<_, Blake2_128Concat, H256, u64, OptionQuery>;

    /// 函数级中文注释：Plain 模式全局 CID 去重索引（可选）。
    /// - key 为 blake2_256(cid)；value 为 EvidenceId（首次出现的记录）。
    #[pallet::storage]
    pub type CidHashIndex<T: Config> = StorageMap<_, Blake2_128Concat, H256, u64, OptionQuery>;

    /// 函数级中文注释：每主体（domain,target）下的证据提交计数（链接操作不计数）。
    #[pallet::storage]
    pub type EvidenceCountByTarget<T: Config> =
        StorageMap<_, Blake2_128Concat, (u8, u64), u32, ValueQuery>;

    /// 函数级中文注释：每主体（ns,subject_id）下的证据提交计数（commit_hash 路径）。
    #[pallet::storage]
    pub type EvidenceCountByNs<T: Config> =
        StorageMap<_, Blake2_128Concat, ([u8; 8], u64), u32, ValueQuery>;

    /// 函数级中文注释：账户限频窗口存储（窗口起点与计数）。
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Default)]
    pub struct WindowInfo<BlockNumber> {
        pub window_start: BlockNumber,
        pub count: u32,
    }
    #[pallet::storage]
    pub type AccountWindows<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, WindowInfo<BlockNumberFor<T>>, ValueQuery>;

    // === 私密内容存储 ===

    /// 私密内容序列号
    #[pallet::storage]
    pub type NextPrivateContentId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 私密内容主存储
    #[pallet::storage]
    pub type PrivateContents<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, private_content::PrivateContent<T>, OptionQuery>;

    /// 按CID索引私密内容（支持去重和快速查找）
    #[pallet::storage]
    pub type PrivateContentByCid<T: Config> =
        StorageMap<_, Blake2_128Concat, BoundedVec<u8, T::MaxCidLen>, u64, OptionQuery>;

    /// 按主体索引私密内容
    #[pallet::storage]
    pub type PrivateContentBySubject<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ([u8; 8], u64), // (ns, subject_id)
        Blake2_128Concat,
        u64, // content_id
        (),
        OptionQuery,
    >;

    /// 用户公钥存储
    #[pallet::storage]
    pub type UserPublicKeys<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, UserPublicKey<T>, OptionQuery>;

    /// 密钥轮换历史
    #[pallet::storage]
    pub type KeyRotationHistory<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64, // content_id
        Blake2_128Concat,
        u32, // rotation_round
        private_content::KeyRotationRecord<T>,
        OptionQuery,
    >;

    /// 🆕 VM1: 密钥轮换计数器（避免 iter_prefix O(N) 扫描）
    #[pallet::storage]
    pub type KeyRotationCounter<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;

    /// 访问请求存储：(content_id, requester) → 请求区块号
    /// 用户请求访问加密内容后，创建者可通过 grant_access 批准
    #[pallet::storage]
    pub type AccessRequests<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64, // content_id
        Blake2_128Concat,
        T::AccountId, // requester
        BlockNumberFor<T>, // requested_at
        OptionQuery,
    >;

    // ==================== 存储膨胀防护：归档机制 ====================

    /// 归档证据存储（精简摘要，~50字节/条）
    #[pallet::storage]
    pub type ArchivedEvidences<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, ArchivedEvidence, OptionQuery>;

    /// 归档游标：记录已扫描到的证据ID
    #[pallet::storage]
    pub type EvidenceArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 归档统计
    #[pallet::storage]
    pub type ArchiveStats<T: Config> = StorageValue<_, ArchiveStatistics, ValueQuery>;

    /// 🆕 防膨胀: 归档清理游标（记录已清理到的归档ID）
    #[pallet::storage]
    pub type ArchiveCleanupCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    // ==================== P0-3: 证据密封（仲裁冻结）====================

    /// 已密封的证据集合：evidence_id → 密封发起者
    /// 密封后证据不可修改、撤回、取消链接
    #[pallet::storage]
    pub type SealedEvidences<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, T::AccountId, OptionQuery>;

    // ==================== P1-1: 证据状态 ====================

    /// 证据状态存储（默认 Active）
    #[pallet::storage]
    pub type EvidenceStatuses<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, EvidenceStatus, ValueQuery>;

    // ==================== P1-5: PendingManifest 过期清理游标 ====================

    /// 过期 PendingManifest 清理游标
    #[pallet::storage]
    pub type PendingManifestCleanupCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    // ==================== 证据追加链 ====================

    /// 证据追加关系：子证据 → 父证据
    /// 用于追溯证据链，支持补充证据功能
    #[pallet::storage]
    pub type EvidenceParent<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, OptionQuery>;

    /// 证据子项列表：父证据 → 子证据列表
    /// 用于查询某证据的所有补充证据
    #[pallet::storage]
    pub type EvidenceChildren<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        BoundedVec<u64, ConstU32<100>>, // 最多100个补充证据
        ValueQuery,
    >;

    /// 归档统计结构
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug, Default)]
    pub struct ArchiveStatistics {
        /// 已归档证据总数
        pub total_archived: u64,
        /// 释放的存储字节数（估算）
        pub bytes_saved: u64,
        /// 最后归档时间
        pub last_archive_block: u32,
    }

    // ==================== 🆕 待处理清单（2天修改窗口）====================

    /// 待处理清单状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug, Default)]
    pub enum ManifestStatus {
        /// 待处理（可修改）
        #[default]
        Pending,
        /// 处理中（OCW 已获取）
        Processing,
        /// 已确认
        Confirmed,
        /// 处理失败
        Failed,
    }

    /// 待处理清单结构
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
    #[scale_info(skip_type_params(MaxCidLen, MaxMediaCount, MaxMemoLen))]
    pub struct PendingManifest<AccountId, BlockNumber, MaxCidLen: Get<u32>, MaxMediaCount: Get<u32>, MaxMemoLen: Get<u32>> {
        /// 证据ID
        pub evidence_id: u64,
        /// 图片 CID 列表
        pub imgs: BoundedVec<BoundedVec<u8, MaxCidLen>, MaxMediaCount>,
        /// 视频 CID 列表
        pub vids: BoundedVec<BoundedVec<u8, MaxCidLen>, MaxMediaCount>,
        /// 文档 CID 列表
        pub docs: BoundedVec<BoundedVec<u8, MaxCidLen>, MaxMediaCount>,
        /// 备注
        pub memo: Option<BoundedVec<u8, MaxMemoLen>>,
        /// 提交者
        pub owner: AccountId,
        /// 创建区块
        pub created_at: BlockNumber,
        /// 处理状态
        pub status: ManifestStatus,
    }

    /// 待处理清单存储
    #[pallet::storage]
    pub type PendingManifests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // evidence_id
        PendingManifest<T::AccountId, BlockNumberFor<T>, T::MaxCidLen, T::MaxImg, T::MaxMemoLen>,
        OptionQuery,
    >;

    /// 待处理队列
    #[pallet::storage]
    pub type PendingManifestQueue<T: Config> = StorageValue<
        _,
        BoundedVec<u64, T::MaxListLen>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        EvidenceCommitted {
            id: u64,
            domain: u8,
            target_id: u64,
            owner: T::AccountId,
        },
        EvidenceLinked {
            domain: u8,
            target_id: u64,
            id: u64,
        },
        EvidenceUnlinked {
            domain: u8,
            target_id: u64,
            id: u64,
        },
        /// 新增：V2 事件，按命名空间与主体提交/链接
        EvidenceCommittedV2 {
            id: u64,
            ns: [u8; 8],
            subject_id: u64,
            owner: T::AccountId,
        },
        EvidenceLinkedV2 {
            ns: [u8; 8],
            subject_id: u64,
            id: u64,
        },
        EvidenceUnlinkedV2 {
            ns: [u8; 8],
            subject_id: u64,
            id: u64,
        },
        /// 函数级中文注释：因限频或配额被限制（便于前端提示）。
        EvidenceThrottled(T::AccountId, u8 /*reason_code: 1=RateLimited,2=Quota*/),
        /// 函数级中文注释：达到主体配额上限。
        EvidenceQuotaReached(
            u8,  /*0=target,1=ns*/
            u64, /*subject_id or target_id*/
        ),

        // === 私密内容事件 ===
        /// 私密内容已存储
        PrivateContentStored {
            content_id: u64,
            ns: [u8; 8],
            subject_id: u64,
            cid: BoundedVec<u8, T::MaxCidLen>,
            creator: T::AccountId,
        },

        /// 访问权限已授予
        AccessGranted {
            content_id: u64,
            user: T::AccountId,
            granted_by: T::AccountId,
        },

        /// 访问权限已撤销
        AccessRevoked {
            content_id: u64,
            user: T::AccountId,
            revoked_by: T::AccountId,
        },

        /// 密钥已轮换
        KeysRotated {
            content_id: u64,
            rotation_round: u32,
            rotated_by: T::AccountId,
        },

        /// 用户公钥已注册
        PublicKeyRegistered {
            user: T::AccountId,
            key_type: pallet_crypto_common::KeyType,
        },

        // === 归档事件 ===
        /// 证据已归档
        EvidenceArchived {
            id: u64,
            domain: u8,
            target_id: u64,
        },

        /// 补充证据已追加
        EvidenceAppended {
            /// 新证据ID
            id: u64,
            /// 父证据ID（被补充的原始证据）
            parent_id: u64,
            /// 所属域
            domain: u8,
            /// 目标ID
            target_id: u64,
            /// 提交者
            owner: T::AccountId,
        },

        // ==================== 🆕 待处理清单事件 ====================

        /// 证据清单已更新（在修改窗口内）
        EvidenceManifestUpdated {
            evidence_id: u64,
            owner: T::AccountId,
        },

        /// 证据清单已确认（OCW 处理完成）
        EvidenceManifestConfirmed {
            evidence_id: u64,
            manifest_cid: BoundedVec<u8, T::MaxContentCidLen>,
        },

        // === 解密流程事件 ===

        /// 用户请求访问加密内容（等待创建者批准）
        AccessRequested {
            content_id: u64,
            requester: T::AccountId,
        },

        /// 访问策略已更新
        AccessPolicyUpdated {
            content_id: u64,
            updated_by: T::AccountId,
        },

        // ==================== 新增事件 ====================

        /// P0-2: 承诺已揭示（Commit-Reveal 的 Reveal 阶段）
        CommitmentRevealed {
            id: u64,
            ns: [u8; 8],
            subject_id: u64,
            content_cid: BoundedVec<u8, T::MaxContentCidLen>,
            owner: T::AccountId,
        },

        /// P0-3: 证据已密封（仲裁冻结）
        EvidenceSealed {
            id: u64,
            sealed_by: T::AccountId,
        },

        /// P0-3: 证据密封已解除
        EvidenceUnsealed {
            id: u64,
            unsealed_by: T::AccountId,
        },

        /// P0-4: 证据已被强制移除（Root操作）
        EvidenceForceRemoved {
            id: u64,
            domain: u8,
            target_id: u64,
        },

        /// P1-1: 证据已撤回
        EvidenceWithdrawn {
            id: u64,
            owner: T::AccountId,
        },

        /// P1-3: 私密内容已删除
        PrivateContentDeleted {
            content_id: u64,
            deleted_by: T::AccountId,
        },

        /// P1-6: 证据已被强制归档（管理员操作）
        EvidenceForceArchived {
            id: u64,
            domain: u8,
            target_id: u64,
        },

        /// P1-6: 用户公钥已撤销
        PublicKeyRevoked {
            user: T::AccountId,
        },

        /// P2: 访问请求已取消
        AccessRequestCancelled {
            content_id: u64,
            requester: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 权限不足（命名空间或账户不被授权）
        NotAuthorized,
        /// 未找到目标对象
        NotFound,

        // === 私密内容错误 ===
        /// 私密内容未找到
        PrivateContentNotFound,
        /// 用户公钥未注册
        PublicKeyNotRegistered,
        /// 无权访问此内容
        AccessDenied,
        /// CID已存在（去重检查）
        CidAlreadyExists,
        /// 授权用户数量过多
        TooManyAuthorizedUsers,
        /// 无效的加密密钥格式
        InvalidEncryptedKey,
        /// 密钥类型不支持
        UnsupportedKeyType,
        /// 图片数量超过上限
        TooManyImages,
        /// 视频数量超过上限
        TooManyVideos,
        /// 文档数量超过上限
        TooManyDocs,
        /// CID 长度或格式非法（非可见 ASCII 或为空）
        InvalidCidFormat,
        /// 发现重复的 CID 输入
        DuplicateCid,
        /// 提交的承诺已存在（防重）
        CommitAlreadyExists,
        /// 证据命名空间与当前操作命名空间不匹配
        NamespaceMismatch,
        /// 账号在窗口内达到提交上限
        RateLimited,
        /// 该主体已达到最大证据条数
        TooManyForSubject,
        /// 全局 CID 去重命中（Plain 模式）
        DuplicateCidGlobal,
        /// 父证据不存在
        ParentEvidenceNotFound,
        /// 补充证据数量超过上限
        TooManySupplements,
        /// 不能追加到已归档的证据
        CannotAppendToArchived,

        // ==================== 🆕 待处理清单错误 ====================

        /// 待处理清单不存在
        PendingManifestNotFound,
        /// 修改窗口已过期
        EditWindowExpired,
        /// 待处理队列已满
        PendingQueueFull,

        // === 解密流程错误 ===

        /// 用户已提交过访问请求
        AlreadyRequested,
        /// 用户已被授权访问（无需重复请求）
        AlreadyAuthorized,
        /// 不能向自己发送访问请求
        SelfAccessRequest,

        // ==================== 新增错误 ====================

        /// P0-2: 承诺哈希不存在（reveal 时找不到对应的 commit_hash 证据）
        CommitNotFound,
        /// P0-2: 揭示数据与承诺哈希不匹配
        CommitMismatch,
        /// P0-2: 证据已经揭示过（不可重复揭示）
        AlreadyRevealed,
        /// P0-3: 证据已被密封，不可修改/撤回/取消链接
        EvidenceSealed,
        /// P0-3: 证据未被密封（解封时使用）
        EvidenceNotSealed,
        /// P1-1: 证据已被撤回
        EvidenceWithdrawn,
        /// P1-1: 证据状态不允许此操作
        InvalidEvidenceStatus,
        /// P1-3: 私密内容仍有活跃访问者（删除前需先撤销所有访问权限）
        ContentHasActiveUsers,
        /// P2: 访问请求不存在
        AccessRequestNotFound,
    }

    #[allow(deprecated)]
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 函数级中文注释：提交证据，生成 EvidenceId 并落库；仅授权账户可提交。
        #[pallet::call_index(0)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::commit(imgs.len() as u32, vids.len() as u32, docs.len() as u32))]
        pub fn commit(
            origin: OriginFor<T>,
            domain: u8,
            target_id: u64,
            imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg>,
            vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid>,
            docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc>,
            _memo: Option<BoundedVec<u8, T::MaxMemoLen>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Authorizer 鉴权（通过适配器，解耦到 runtime）
            let ns = T::EvidenceNsBytes::get();
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );
            // 限频与配额
            let now = <frame_system::Pallet<T>>::block_number();
            Self::touch_window(&who, now)?;
            let cnt = EvidenceCountByTarget::<T>::get((domain, target_id));
            ensure!(
                cnt < T::MaxPerSubjectTarget::get(),
                Error::<T>::TooManyForSubject
            );
            // 🆕 V3: BoundedVec 已在 SCALE 解码层拦截超大载荷（MaxImg/MaxVid/MaxDoc）
            // 校验 CID（长度/格式/重复）与数量上限
            Self::validate_cid_vec(&imgs)?;
            Self::validate_cid_vec(&vids)?;
            Self::validate_cid_vec(&docs)?;
            // 可选全局去重
            Self::ensure_global_cid_unique([imgs.as_slice(), vids.as_slice(), docs.as_slice()])?;
            
            let id = NextEvidenceId::<T>::mutate(|n| {
                let id = *n;
                *n = n.saturating_add(1);
                id
            });
            
            // 🆕 M-NEW-4修复: 拒绝所有媒体列表为空的证据提交（不再使用占位符CID）
            ensure!(
                !imgs.is_empty() || !vids.is_empty() || !docs.is_empty(),
                Error::<T>::InvalidCidFormat
            );
            // 🔮 Phase 1.5 计划：将 imgs/vids/docs 打包为JSON上传IPFS，返回content_cid
            // 当前临时方案：使用第一个媒体的CID作为content_cid
            let temp_vec: Vec<u8> = if !imgs.is_empty() {
                imgs[0].clone().into_inner()
            } else if !vids.is_empty() {
                vids[0].clone().into_inner()
            } else {
                docs[0].clone().into_inner()
            };
            let content_cid: BoundedVec<u8, T::MaxContentCidLen> = temp_vec.try_into()
                .map_err(|_| Error::<T>::InvalidCidFormat)?;
            
            let ev = Evidence {
                id,
                domain,
                target_id,
                owner: who.clone(),
                content_cid,
                content_type: ContentType::Mixed, // 临时使用Mixed类型
                created_at: now,
                is_encrypted: false, // 临时假设不加密
                encryption_scheme: None,
                commit: None,
                ns: Some(ns),
            };
            Evidences::<T>::insert(id, &ev);
            EvidenceByTarget::<T>::insert((domain, target_id), id, ());
            // 计数 + 去重索引落库
            EvidenceCountByTarget::<T>::insert((domain, target_id), cnt.saturating_add(1));
            
            // 🔮 Phase 1.5 计划：从 content_cid 指向的JSON解析出所有CID进行去重和pin
            // 当前临时方案：对当前的content_cid进行去重和pin
            if T::EnableGlobalCidDedup::get() {
                let h = H256::from(blake2_256(&ev.content_cid.clone().into_inner()));
                if CidHashIndex::<T>::get(h).is_none() {
                    CidHashIndex::<T>::insert(h, id);
                }
            }

            // 函数级详细中文注释：自动pin证据CID到IPFS（P1重构：使用证据专用接口）
            // 🔮 Phase 1.5 计划：pin content_cid及其包含的所有媒体CID
            // 当前方案：使用 pin（Evidence 类型，Critical 级别）
            let cid_vec: Vec<u8> = ev.content_cid.clone().into_inner();
            if let Err(e) = T::StoragePin::pin(
                who.clone(),
                b"evidence",
                id,
                None,
                cid_vec,
                pallet_storage_service::PinTier::Critical,
            ) {
                log::warn!(
                    target: "evidence",
                    "Auto-pin content cid failed for evidence {:?}: {:?}",
                    id,
                    e
                );
            }
            
            // 只读方法移至模块外部以避免 non_local_definitions 警告在 -D warnings 下被提升为错误。
            Self::deposit_event(Event::EvidenceCommitted {
                id,
                domain,
                target_id,
                owner: who,
            });
            Ok(())
        }

        /// 函数级中文注释（V2）：仅登记承诺哈希（不在链上存储任何明文/可逆 CID）。
        /// - ns：8 字节命名空间（如 b"kyc_____"、b"otc_ord_"）。
        /// - subject_id：业务主体 id（如订单号、账户短码等）。
        /// - commit：承诺哈希（例如 blake2b256(ns||subject_id||cid_enc||salt||ver)）。
        #[pallet::call_index(1)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::commit_hash())]
        pub fn commit_hash(
            origin: OriginFor<T>,
            ns: [u8; 8],
            subject_id: u64,
            commit: H256,
            memo: Option<BoundedVec<u8, T::MaxMemoLen>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );
            // 防重：承诺哈希唯一
            ensure!(
                CommitIndex::<T>::get(commit).is_none(),
                Error::<T>::CommitAlreadyExists
            );
            // 限频与配额
            let now = <frame_system::Pallet<T>>::block_number();
            Self::touch_window(&who, now)?;
            let cnt = EvidenceCountByNs::<T>::get((ns, subject_id));
            ensure!(
                cnt < T::MaxPerSubjectNs::get(),
                Error::<T>::TooManyForSubject
            );
            let id = NextEvidenceId::<T>::mutate(|n| {
                let id = *n;
                *n = n.saturating_add(1);
                id
            });
            // 🆕 M-NEW-4修复: commit_hash 要求 memo 非空（不再使用占位符CID）
            let memo_ref = memo.as_ref().ok_or(Error::<T>::InvalidCidFormat)?;
            ensure!(!memo_ref.is_empty(), Error::<T>::InvalidCidFormat);
            // TODO: Phase 1.5 完整实施 - 从memo或其他来源获取content_cid
            // 临时方案：转换memo为content_cid类型
            let temp_vec2: Vec<u8> = memo_ref.clone().into_inner();
            let content_cid: BoundedVec<u8, T::MaxContentCidLen> = temp_vec2.try_into()
                .map_err(|_| Error::<T>::InvalidCidFormat)?;
            
            let ev = Evidence {
                id,
                domain: 0,
                target_id: subject_id,
                owner: who.clone(),
                content_cid,
                content_type: ContentType::Mixed,
                created_at: now,
                is_encrypted: false,
                encryption_scheme: None,
                commit: Some(commit),
                ns: Some(ns),
            };
            Evidences::<T>::insert(id, &ev);
            EvidenceByNs::<T>::insert((ns, subject_id), id, ());
            CommitIndex::<T>::insert(commit, id);
            EvidenceCountByNs::<T>::insert((ns, subject_id), cnt.saturating_add(1));
            Self::deposit_event(Event::EvidenceCommittedV2 {
                id,
                ns,
                subject_id,
                owner: who,
            });
            Ok(())
        }

        /// 函数级中文注释：为目标链接已存在的证据（允许复用）；仅授权账户可调用。
        #[pallet::call_index(2)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::link())]
        pub fn link(origin: OriginFor<T>, domain: u8, target_id: u64, id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let ev = Evidences::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            let ev_ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ev_ns, &who),
                Error::<T>::NotAuthorized
            );
            // 🆕 M2-R3修复: 密封和状态检查（与 unlink 对称）
            ensure!(!SealedEvidences::<T>::contains_key(id), Error::<T>::EvidenceSealed);
            ensure!(EvidenceStatuses::<T>::get(id) == EvidenceStatus::Active, Error::<T>::InvalidEvidenceStatus);
            EvidenceByTarget::<T>::insert((domain, target_id), id, ());
            Self::deposit_event(Event::EvidenceLinked {
                domain,
                target_id,
                id,
            });
            Ok(())
        }

        /// 函数级中文注释（V2）：按命名空间与主体链接既有证据 id（仅保存引用，不触碰明文）。
        #[pallet::call_index(3)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::link_by_ns())]
        pub fn link_by_ns(
            origin: OriginFor<T>,
            ns: [u8; 8],
            subject_id: u64,
            id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );
            let ev = Evidences::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            let ev_ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            ensure!(ev_ns == ns, Error::<T>::NamespaceMismatch);
            // 🆕 M2-R3修复: 密封和状态检查（与 unlink_by_ns 对称）
            ensure!(!SealedEvidences::<T>::contains_key(id), Error::<T>::EvidenceSealed);
            ensure!(EvidenceStatuses::<T>::get(id) == EvidenceStatus::Active, Error::<T>::InvalidEvidenceStatus);
            EvidenceByNs::<T>::insert((ns, subject_id), id, ());
            Self::deposit_event(Event::EvidenceLinkedV2 { ns, subject_id, id });
            Ok(())
        }

        /// 函数级中文注释：取消目标与证据的链接；仅授权账户可调用。
        #[pallet::call_index(4)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::unlink())]
        pub fn unlink(origin: OriginFor<T>, domain: u8, target_id: u64, id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let ev = Evidences::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            let ev_ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ev_ns, &who),
                Error::<T>::NotAuthorized
            );
            // P0-3: 密封检查
            ensure!(!SealedEvidences::<T>::contains_key(id), Error::<T>::EvidenceSealed);
            EvidenceByTarget::<T>::remove((domain, target_id), id);
            Self::deposit_event(Event::EvidenceUnlinked {
                domain,
                target_id,
                id,
            });
            Ok(())
        }

        /// 函数级中文注释（V2）：按命名空间与主体取消链接。
        #[pallet::call_index(5)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::unlink_by_ns())]
        pub fn unlink_by_ns(
            origin: OriginFor<T>,
            ns: [u8; 8],
            subject_id: u64,
            id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );
            let ev = Evidences::<T>::get(id).ok_or(Error::<T>::NotFound)?;
            let ev_ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            ensure!(ev_ns == ns, Error::<T>::NamespaceMismatch);
            // P0-3: 密封检查
            ensure!(!SealedEvidences::<T>::contains_key(id), Error::<T>::EvidenceSealed);
            EvidenceByNs::<T>::remove((ns, subject_id), id);
            Self::deposit_event(Event::EvidenceUnlinkedV2 { ns, subject_id, id });
            Ok(())
        }

        // ===== 私密内容管理 Extrinsics =====

        /// 注册用户公钥（用于加密密钥包）
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::register_public_key())]
        pub fn register_public_key(
            origin: OriginFor<T>,
            key_data: BoundedVec<u8, T::MaxKeyLen>,
            key_type: pallet_crypto_common::KeyType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证密钥长度（使用 KeyType 枚举的类型安全验证）
            ensure!(
                key_type.validate_key_len(key_data.len()),
                Error::<T>::InvalidEncryptedKey
            );

            let now = <frame_system::Pallet<T>>::block_number();

            let public_key = UserPublicKey::<T> {
                key_data,
                key_type,
                registered_at: now,
            };

            UserPublicKeys::<T>::insert(&who, &public_key);

            Self::deposit_event(Event::PublicKeyRegistered {
                user: who,
                key_type,
            });

            Ok(())
        }

        /// 存储私密内容
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::store_private_content())]
        pub fn store_private_content(
            origin: OriginFor<T>,
            ns: [u8; 8],
            subject_id: u64,
            cid: BoundedVec<u8, T::MaxCidLen>,
            content_hash: H256,
            encryption_method: pallet_crypto_common::EncryptionMethod,
            access_policy: private_content::AccessPolicy<T>,
            encrypted_keys: EncryptedKeyBundles<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 权限检查
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );

            // 私密内容必须使用加密CID验证
            let cid_bytes: &[u8] = cid.as_slice();
            ensure!(
                crate::cid_validator::DefaultCidValidator::is_encrypted(cid_bytes),
                Error::<T>::InvalidCidFormat
            );

            // P0-1修复: 加密CID的格式验证 — 剥离加密前缀后验证底层IPFS CID
            // 旧代码直接调用 IpfsHelper::validate_cid(full_cid)，但加密前缀(enc-/sealed-等)
            // 与 IPFS CID 格式要求(首字符 b/z/f/m/Q)互斥，导致所有加密CID验证失败。
            let inner_cid_bytes = crate::cid_validator::strip_encrypted_prefix(cid_bytes);
            ensure!(!inner_cid_bytes.is_empty(), Error::<T>::InvalidCidFormat);
            let inner_cid_str = core::str::from_utf8(inner_cid_bytes)
                .map_err(|_| Error::<T>::InvalidCidFormat)?;
            IpfsHelper::validate_cid(inner_cid_str)
                .map_err(|_| Error::<T>::InvalidCidFormat)?;

            // CID 去重检查
            ensure!(
                PrivateContentByCid::<T>::get(&cid).is_none(),
                Error::<T>::CidAlreadyExists
            );

            // 验证创建者是否有加密密钥
            ensure!(
                encrypted_keys.iter().any(|(user, _)| user == &who),
                Error::<T>::InvalidEncryptedKey
            );

            // 验证所有用户都已注册公钥
            for (user, _) in encrypted_keys.iter() {
                ensure!(
                    UserPublicKeys::<T>::contains_key(user),
                    Error::<T>::PublicKeyNotRegistered
                );
            }

            let content_id = NextPrivateContentId::<T>::mutate(|id| {
                let current = *id;
                *id = id.saturating_add(1);
                current
            });

            let now = <frame_system::Pallet<T>>::block_number();

            let content = pallet_crypto_common::PrivateContent::<
                T::AccountId,
                BlockNumberFor<T>,
                T::MaxCidLen,
                T::MaxAuthorizedUsers,
                T::MaxKeyLen,
            > {
                id: content_id,
                ns,
                subject_id,
                cid: cid.clone(),
                content_hash,
                encryption_method,
                creator: who.clone(),
                access_policy,
                encrypted_keys,
                status: pallet_crypto_common::ContentStatus::Active,
                created_at: now,
                updated_at: now,
            };

            // 存储
            PrivateContents::<T>::insert(content_id, &content);
            PrivateContentByCid::<T>::insert(&cid, content_id);
            PrivateContentBySubject::<T>::insert((ns, subject_id), content_id, ());

            Self::deposit_event(Event::PrivateContentStored {
                content_id,
                ns,
                subject_id,
                cid,
                creator: who,
            });

            Ok(())
        }

        /// 授予用户访问权限
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::grant_access())]
        pub fn grant_access(
            origin: OriginFor<T>,
            content_id: u64,
            user: T::AccountId,
            encrypted_key: BoundedVec<u8, ConstU32<512>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证用户已注册公钥
            ensure!(
                UserPublicKeys::<T>::contains_key(&user),
                Error::<T>::PublicKeyNotRegistered
            );

            PrivateContents::<T>::try_mutate(content_id, |maybe_content| -> DispatchResult {
                let content = maybe_content
                    .as_mut()
                    .ok_or(Error::<T>::PrivateContentNotFound)?;

                // 权限检查：仅创建者可授予权限
                ensure!(content.creator == who, Error::<T>::AccessDenied);

                // 检查是否已授权
                let key_vec: Vec<u8> = encrypted_key.into();
                let bounded_key = BoundedVec::try_from(key_vec.clone())
                    .map_err(|_| Error::<T>::InvalidEncryptedKey)?;
                let mut found = false;
                for (existing_user, existing_key) in content.encrypted_keys.iter_mut() {
                    if existing_user == &user {
                        *existing_key = bounded_key.clone();
                        found = true;
                        break;
                    }
                }

                if !found {
                    content
                        .encrypted_keys
                        .try_push((user.clone(), bounded_key))
                        .map_err(|_| Error::<T>::TooManyAuthorizedUsers)?;
                }

                content.updated_at = <frame_system::Pallet<T>>::block_number();

                // 自动清除该用户的待处理访问请求（如果有）
                AccessRequests::<T>::remove(content_id, &user);

                Self::deposit_event(Event::AccessGranted {
                    content_id,
                    user,
                    granted_by: who,
                });

                Ok(())
            })
        }

        /// 撤销用户访问权限
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::revoke_access())]
        pub fn revoke_access(
            origin: OriginFor<T>,
            content_id: u64,
            user: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            PrivateContents::<T>::try_mutate(content_id, |maybe_content| -> DispatchResult {
                let content = maybe_content
                    .as_mut()
                    .ok_or(Error::<T>::PrivateContentNotFound)?;

                // 权限检查
                ensure!(content.creator == who, Error::<T>::AccessDenied);
                ensure!(user != who, Error::<T>::AccessDenied); // 不能撤销自己的权限

                // 🆕 M1修复: 验证用户确实在授权列表中，避免静默无操作
                let before_len = content.encrypted_keys.len();
                content
                    .encrypted_keys
                    .retain(|(existing_user, _)| existing_user != &user);
                ensure!(content.encrypted_keys.len() < before_len, Error::<T>::NotFound);
                content.updated_at = <frame_system::Pallet<T>>::block_number();

                Self::deposit_event(Event::AccessRevoked {
                    content_id,
                    user,
                    revoked_by: who,
                });

                Ok(())
            })
        }

        /// 轮换内容加密密钥
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::rotate_content_keys())]
        pub fn rotate_content_keys(
            origin: OriginFor<T>,
            content_id: u64,
            new_content_hash: H256, // 重新加密后的内容哈希
            new_encrypted_keys: BoundedVec<
                (T::AccountId, BoundedVec<u8, ConstU32<512>>),
                T::MaxAuthorizedUsers,
            >,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            PrivateContents::<T>::try_mutate(content_id, |maybe_content| -> DispatchResult {
                let content = maybe_content
                    .as_mut()
                    .ok_or(Error::<T>::PrivateContentNotFound)?;

                // 权限检查
                ensure!(content.creator == who, Error::<T>::AccessDenied);

                // 验证所有用户都已注册公钥
                for (user, _) in new_encrypted_keys.iter() {
                    ensure!(
                        UserPublicKeys::<T>::contains_key(user),
                        Error::<T>::PublicKeyNotRegistered
                    );
                }

                // 更新内容
                content.content_hash = new_content_hash;
                let converted = new_encrypted_keys
                    .into_iter()
                    .map(|(u, k)| {
                        let key_vec: Vec<u8> = k.into();
                        BoundedVec::try_from(key_vec)
                            .map(|bk| (u, bk))
                            .map_err(|_| Error::<T>::InvalidEncryptedKey)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let bounded_converted = BoundedVec::try_from(converted)
                    .map_err(|_| Error::<T>::TooManyAuthorizedUsers)?;
                content.encrypted_keys = bounded_converted;
                content.updated_at = <frame_system::Pallet<T>>::block_number();

                // 🆕 VM1修复: 使用计数器代替 O(N) iter_prefix 扫描
                let rotation_round = KeyRotationCounter::<T>::mutate(content_id, |c| {
                    *c = c.saturating_add(1);
                    *c
                });

                let rotation_record = pallet_crypto_common::KeyRotationRecord::<
                    T::AccountId,
                    BlockNumberFor<T>,
                > {
                    content_id,
                    rotation_round,
                    rotated_at: content.updated_at,
                    rotated_by: who.clone(),
                };

                KeyRotationHistory::<T>::insert(content_id, rotation_round, &rotation_record);

                Self::deposit_event(Event::KeysRotated {
                    content_id,
                    rotation_round,
                    rotated_by: who,
                });

                Ok(())
            })
        }

        /// 追加补充证据
        /// 
        /// 函数级中文注释：为已存在的证据追加补充材料
        /// - 原证据保持不可变
        /// - 新证据与原证据形成父子关系
        /// - 可追溯完整证据链
        /// 
        /// # 参数
        /// - `parent_id`: 父证据ID（被补充的原始证据）
        /// - `imgs`: 补充图片CID列表
        /// - `vids`: 补充视频CID列表
        /// - `docs`: 补充文档CID列表
        /// - `memo`: 补充说明（可选）
        #[pallet::call_index(11)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::commit(imgs.len() as u32, vids.len() as u32, docs.len() as u32))]
        pub fn append_evidence(
            origin: OriginFor<T>,
            parent_id: u64,
            imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg>,
            vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid>,
            docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc>,
            _memo: Option<BoundedVec<u8, T::MaxMemoLen>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // 1. 验证父证据存在
            let parent = Evidences::<T>::get(parent_id)
                .ok_or(Error::<T>::ParentEvidenceNotFound)?;
            
            // 2. 验证父证据未被归档
            ensure!(
                !ArchivedEvidences::<T>::contains_key(parent_id),
                Error::<T>::CannotAppendToArchived
            );
            
            // 🆕 H2修复: 验证父证据未被密封且状态为 Active
            ensure!(
                !SealedEvidences::<T>::contains_key(parent_id),
                Error::<T>::EvidenceSealed
            );
            let parent_status = EvidenceStatuses::<T>::get(parent_id);
            ensure!(
                parent_status == EvidenceStatus::Active,
                Error::<T>::InvalidEvidenceStatus
            );
            
            // 3. 验证权限（同命名空间）
            let ns = T::EvidenceNsBytes::get();
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );

            // 🆕 V3: BoundedVec 已在 SCALE 解码层拦截超大载荷（MaxImg/MaxVid/MaxDoc）
            
            // 4. 验证补充数量限制
            let children = EvidenceChildren::<T>::get(parent_id);
            ensure!(
                children.len() < 100,
                Error::<T>::TooManySupplements
            );
            
            // 🆕 M3-R4修复: 检查 per-subject 配额（与 commit 一致）
            let target_count = EvidenceCountByTarget::<T>::get((parent.domain, parent.target_id));
            ensure!(
                target_count < T::MaxPerSubjectTarget::get(),
                Error::<T>::TooManyForSubject
            );

            // 5. 限频检查
            let now = <frame_system::Pallet<T>>::block_number();
            Self::touch_window(&who, now)?;
            
            // 6. 校验 CID
            Self::validate_cid_vec(&imgs)?;
            Self::validate_cid_vec(&vids)?;
            Self::validate_cid_vec(&docs)?;
            // 🆕 M1-R3修复: 全局 CID 去重检查（与 commit 保持一致）
            Self::ensure_global_cid_unique([imgs.as_slice(), vids.as_slice(), docs.as_slice()])?;
            
            // 7. 生成新证据ID
            let id = NextEvidenceId::<T>::mutate(|n| {
                let id = *n;
                *n = n.saturating_add(1);
                id
            });
            
            // 🆕 M-NEW-4修复: 拒绝所有媒体列表为空的补充证据
            ensure!(
                !imgs.is_empty() || !vids.is_empty() || !docs.is_empty(),
                Error::<T>::InvalidCidFormat
            );
            // 8. 构建补充证据（继承父证据的domain和target_id）
            let temp_vec: Vec<u8> = if !imgs.is_empty() {
                imgs[0].clone().into_inner()
            } else if !vids.is_empty() {
                vids[0].clone().into_inner()
            } else {
                docs[0].clone().into_inner()
            };
            let content_cid: BoundedVec<u8, T::MaxContentCidLen> = temp_vec.try_into()
                .map_err(|_| Error::<T>::InvalidCidFormat)?;
            
            let ev = Evidence {
                id,
                domain: parent.domain,
                target_id: parent.target_id,
                owner: who.clone(),
                content_cid,
                content_type: ContentType::Mixed,
                created_at: now,
                is_encrypted: false,
                encryption_scheme: None,
                commit: None,
                ns: Some(ns),
            };
            
            // 9. 存储证据
            Evidences::<T>::insert(id, &ev);
            EvidenceByTarget::<T>::insert((parent.domain, parent.target_id), id, ());
            // 🆕 M-NEW-2修复: 同步递增 EvidenceCountByTarget（与 commit 保持一致）
            EvidenceCountByTarget::<T>::mutate(
                (parent.domain, parent.target_id), |c| {
                    *c = c.saturating_add(1);
                },
            );
            // 🆕 M1-R3修复: 注册子证据 content_cid 到 CidHashIndex（与 commit 保持一致）
            if T::EnableGlobalCidDedup::get() {
                let h = H256::from(blake2_256(&ev.content_cid.clone().into_inner()));
                if CidHashIndex::<T>::get(h).is_none() {
                    CidHashIndex::<T>::insert(h, id);
                }
            }
            
            // 10. 建立父子关系
            EvidenceParent::<T>::insert(id, parent_id);
            EvidenceChildren::<T>::mutate(parent_id, |children| {
                let _ = children.try_push(id);
            });
            
            // 11. 自动pin证据CID
            let cid_vec: Vec<u8> = ev.content_cid.clone().into_inner();
            if let Err(e) = T::StoragePin::pin(
                who.clone(),
                b"evidence",
                id,
                None,
                cid_vec,
                pallet_storage_service::PinTier::Critical,
            ) {
                log::warn!(
                    target: "evidence",
                    "Auto-pin content cid failed for appended evidence {:?}: {:?}",
                    id,
                    e
                );
            }
            
            // 12. 发送事件
            Self::deposit_event(Event::EvidenceAppended {
                id,
                parent_id,
                domain: parent.domain,
                target_id: parent.target_id,
                owner: who,
            });
            
            Ok(())
        }

        // ==================== 🆕 2天修改窗口 Extrinsics ====================

        /// 修改待处理证据清单（在修改窗口内可任意修改）
        /// 
        /// 允许证据提交者在 EvidenceEditWindow 内修改已提交的清单内容
        #[pallet::call_index(12)]
        #[allow(deprecated)]
        #[pallet::weight(T::WeightInfo::commit(imgs.len() as u32, vids.len() as u32, docs.len() as u32))]
        pub fn update_evidence_manifest(
            origin: OriginFor<T>,
            evidence_id: u64,
            imgs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg>,
            vids: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxVid>,
            docs: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxDoc>,
            memo: Option<BoundedVec<u8, T::MaxMemoLen>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // 1. 获取待处理清单
            let pending = PendingManifests::<T>::get(evidence_id)
                .ok_or(Error::<T>::PendingManifestNotFound)?;
            
            // P0-3: 密封检查
            ensure!(!SealedEvidences::<T>::contains_key(evidence_id), Error::<T>::EvidenceSealed);

            // 2. 验证修改窗口
            let now = <frame_system::Pallet<T>>::block_number();
            let edit_window = T::EvidenceEditWindow::get();
            ensure!(
                now <= pending.created_at.saturating_add(edit_window),
                Error::<T>::EditWindowExpired
            );
            
            // 3. 验证权限
            ensure!(pending.owner == who, Error::<T>::NotAuthorized);
            
            // 4. 校验 CID
            Self::validate_cid_vec(&imgs)?;
            Self::validate_cid_vec(&vids)?;
            Self::validate_cid_vec(&docs)?;
            
            // 5. 🆕 V3: BoundedVec 已在 SCALE 解码层拦截超大载荷，仅需转换类型以匹配 PendingManifest 存储
            let imgs_bounded = imgs;
            let vids_bounded: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg> = 
                BoundedVec::try_from(vids.into_inner()).map_err(|_| Error::<T>::TooManyVideos)?;
            let docs_bounded: BoundedVec<BoundedVec<u8, T::MaxCidLen>, T::MaxImg> = 
                BoundedVec::try_from(docs.into_inner()).map_err(|_| Error::<T>::TooManyDocs)?;
            
            // 6. 更新清单（保持原创建时间，不重置窗口）
            let updated = PendingManifest {
                evidence_id,
                imgs: imgs_bounded,
                vids: vids_bounded,
                docs: docs_bounded,
                memo,
                owner: who.clone(),
                created_at: pending.created_at, // 保持原时间，不重置窗口
                status: ManifestStatus::Pending,
            };
            
            PendingManifests::<T>::insert(evidence_id, updated);
            
            // 7. 发送事件
            Self::deposit_event(Event::EvidenceManifestUpdated {
                evidence_id,
                owner: who,
            });
            
            Ok(())
        }

        // === 解密流程 Extrinsics ===

        /// 请求访问加密内容
        ///
        /// 用户在已注册公钥的前提下，向创建者发送访问请求。
        /// 创建者通过 `grant_access` 批准后，用户即可获取解密密钥包。
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::request_access())]
        pub fn request_access(
            origin: OriginFor<T>,
            content_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证用户已注册公钥
            ensure!(
                UserPublicKeys::<T>::contains_key(&who),
                Error::<T>::PublicKeyNotRegistered
            );

            // 验证内容存在
            let content = PrivateContents::<T>::get(content_id)
                .ok_or(Error::<T>::PrivateContentNotFound)?;

            // 不能向自己请求访问
            ensure!(content.creator != who, Error::<T>::SelfAccessRequest);

            // 检查是否已被授权
            ensure!(
                !Self::can_access_private_content(content_id, &who),
                Error::<T>::AlreadyAuthorized
            );

            // 检查是否已请求过
            ensure!(
                !AccessRequests::<T>::contains_key(content_id, &who),
                Error::<T>::AlreadyRequested
            );

            let now = <frame_system::Pallet<T>>::block_number();
            AccessRequests::<T>::insert(content_id, &who, now);

            Self::deposit_event(Event::AccessRequested {
                content_id,
                requester: who,
            });

            Ok(())
        }

        /// 更新加密内容的访问策略
        ///
        /// 仅创建者可更改访问策略。注意：更改策略不会自动添加/移除密钥包，
        /// 创建者需配合 `grant_access` / `revoke_access` / `rotate_content_keys` 使用。
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::update_access_policy())]
        pub fn update_access_policy(
            origin: OriginFor<T>,
            content_id: u64,
            new_policy: private_content::AccessPolicy<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            PrivateContents::<T>::try_mutate(content_id, |maybe_content| -> DispatchResult {
                let content = maybe_content
                    .as_mut()
                    .ok_or(Error::<T>::PrivateContentNotFound)?;

                // 权限检查：仅创建者可更新策略
                ensure!(content.creator == who, Error::<T>::AccessDenied);

                content.access_policy = new_policy;
                content.updated_at = <frame_system::Pallet<T>>::block_number();

                Self::deposit_event(Event::AccessPolicyUpdated {
                    content_id,
                    updated_by: who,
                });

                Ok(())
            })
        }

        // ==================== P0-2: 承诺揭示 (Commit-Reveal 的 Reveal 阶段) ====================

        /// 揭示承诺哈希对应的真实 CID
        ///
        /// 提交者在 commit_hash 阶段提交了 H(ns || subject_id || cid || salt || ver)，
        /// 现在揭示原始参数，链上验证哈希匹配后将 content_cid 写入证据。
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::reveal_commitment())]
        pub fn reveal_commitment(
            origin: OriginFor<T>,
            evidence_id: u64,
            cid: BoundedVec<u8, T::MaxCidLen>,
            salt: BoundedVec<u8, T::MaxMemoLen>,
            version: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 1. 获取证据并验证
            let mut ev = Evidences::<T>::get(evidence_id).ok_or(Error::<T>::NotFound)?;
            ensure!(ev.owner == who, Error::<T>::NotAuthorized);

            // 2. 检查状态
            let status = EvidenceStatuses::<T>::get(evidence_id);
            ensure!(status == EvidenceStatus::Active, Error::<T>::InvalidEvidenceStatus);
            ensure!(!SealedEvidences::<T>::contains_key(evidence_id), Error::<T>::EvidenceSealed);

            // 3. 必须是承诺模式的证据
            let commit_hash = ev.commit.ok_or(Error::<T>::CommitNotFound)?;

            // 4. 验证尚未揭示（content_cid 仍为 memo 占位值，commit 字段非 None）
            // 揭示后 commit 字段会被清除
            // 若已揭示过，commit 已为 None
            // （此处 commit 存在说明还未揭示）

            // 5. 验证承诺哈希
            let ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            let computed = Self::compute_evidence_commitment(
                &ns,
                ev.target_id,
                &cid,
                &salt,
                version,
            );
            ensure!(computed == commit_hash, Error::<T>::CommitMismatch);

            // 6. CID 格式验证
            let cid_str = core::str::from_utf8(cid.as_slice())
                .map_err(|_| Error::<T>::InvalidCidFormat)?;
            IpfsHelper::validate_cid(cid_str)
                .map_err(|_| Error::<T>::InvalidCidFormat)?;

            // 7. 更新证据：写入真实 content_cid，清除 commit
            let cid_vec: Vec<u8> = cid.clone().into_inner();
            let content_cid: BoundedVec<u8, T::MaxContentCidLen> = cid_vec.clone().try_into()
                .map_err(|_| Error::<T>::InvalidCidFormat)?;
            ev.content_cid = content_cid.clone();
            // 🆕 H1-R3修复: 清理 CommitIndex（reveal 后 commit hash 不再需要）
            CommitIndex::<T>::remove(commit_hash);
            ev.commit = None; // 标记已揭示
            ev.is_encrypted = false;
            Evidences::<T>::insert(evidence_id, &ev);

            // 8. 自动 pin CID
            if let Err(e) = T::StoragePin::pin(
                who.clone(),
                b"evidence",
                evidence_id,
                None,
                cid_vec,
                pallet_storage_service::PinTier::Critical,
            ) {
                log::warn!(
                    target: "evidence",
                    "Auto-pin CID failed for revealed evidence {:?}: {:?}",
                    evidence_id, e
                );
            }

            // 9. 🆕 M2修复: 全局去重索引 — 揭示时也执行去重检查（与 commit 一致）
            if T::EnableGlobalCidDedup::get() {
                let h = H256::from(blake2_256(&content_cid.clone().into_inner()));
                ensure!(
                    CidHashIndex::<T>::get(h).is_none(),
                    Error::<T>::DuplicateCidGlobal
                );
                CidHashIndex::<T>::insert(h, evidence_id);
            }

            Self::deposit_event(Event::CommitmentRevealed {
                id: evidence_id,
                ns,
                subject_id: ev.target_id,
                content_cid,
                owner: who,
            });
            Ok(())
        }

        // ==================== P0-3: 证据密封/解封 ====================

        /// 密封证据（仲裁冻结）
        ///
        /// 被密封的证据不可修改、撤回、取消链接。
        /// 仅授权账户（如仲裁 pallet 通过 Authorizer）可调用。
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::seal_evidence())]
        pub fn seal_evidence(
            origin: OriginFor<T>,
            evidence_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证证据存在
            let ev = Evidences::<T>::get(evidence_id).ok_or(Error::<T>::NotFound)?;
            let ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );

            // 检查状态
            let status = EvidenceStatuses::<T>::get(evidence_id);
            ensure!(status == EvidenceStatus::Active, Error::<T>::InvalidEvidenceStatus);
            ensure!(!SealedEvidences::<T>::contains_key(evidence_id), Error::<T>::EvidenceSealed);

            // 密封
            SealedEvidences::<T>::insert(evidence_id, &who);
            EvidenceStatuses::<T>::insert(evidence_id, EvidenceStatus::Sealed);

            Self::deposit_event(Event::EvidenceSealed {
                id: evidence_id,
                sealed_by: who,
            });
            Ok(())
        }

        /// 解封证据
        ///
        /// 仅授权账户可调用，解封后证据恢复为 Active 状态。
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::unseal_evidence())]
        pub fn unseal_evidence(
            origin: OriginFor<T>,
            evidence_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证证据存在
            let ev = Evidences::<T>::get(evidence_id).ok_or(Error::<T>::NotFound)?;
            let ns = ev.ns.ok_or(Error::<T>::NamespaceMismatch)?;
            ensure!(
                <T as Config>::Authorizer::is_authorized(ns, &who),
                Error::<T>::NotAuthorized
            );

            // 检查已密封
            ensure!(SealedEvidences::<T>::contains_key(evidence_id), Error::<T>::EvidenceNotSealed);

            // 解封
            SealedEvidences::<T>::remove(evidence_id);
            EvidenceStatuses::<T>::insert(evidence_id, EvidenceStatus::Active);

            Self::deposit_event(Event::EvidenceUnsealed {
                id: evidence_id,
                unsealed_by: who,
            });
            Ok(())
        }

        // ==================== P0-4: 强制移除违规证据 ====================

        /// 强制移除违规证据（Root 级操作）
        ///
        /// 用于法律合规：移除违法内容（儿童色情、恐怖主义、法院命令等）。
        /// 清理所有索引、密封状态、父子关系。
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::force_remove_evidence())]
        pub fn force_remove_evidence(
            origin: OriginFor<T>,
            evidence_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let ev = Evidences::<T>::get(evidence_id).ok_or(Error::<T>::NotFound)?;

            // 清理所有索引
            EvidenceByTarget::<T>::remove((ev.domain, ev.target_id), evidence_id);
            if let Some(ns) = &ev.ns {
                EvidenceByNs::<T>::remove((*ns, ev.target_id), evidence_id);
                EvidenceCountByNs::<T>::mutate((*ns, ev.target_id), |c| {
                    *c = c.saturating_sub(1);
                });
            }
            EvidenceCountByTarget::<T>::mutate((ev.domain, ev.target_id), |c| {
                *c = c.saturating_sub(1);
            });

            // 清理 CID 索引
            let content_hash = H256::from(blake2_256(&ev.content_cid.clone().into_inner()));
            CidHashIndex::<T>::remove(content_hash);

            // 清理承诺索引
            if let Some(commit) = ev.commit {
                CommitIndex::<T>::remove(commit);
            }

            // 清理密封状态
            SealedEvidences::<T>::remove(evidence_id);

            // 清理父子关系
            if let Some(parent_id) = EvidenceParent::<T>::get(evidence_id) {
                EvidenceChildren::<T>::mutate(parent_id, |children| {
                    children.retain(|&id| id != evidence_id);
                });
                EvidenceParent::<T>::remove(evidence_id);
            }
            // 将子证据的父引用也清除
            let children = EvidenceChildren::<T>::get(evidence_id);
            for child_id in children.iter() {
                EvidenceParent::<T>::remove(child_id);
            }
            EvidenceChildren::<T>::remove(evidence_id);

            // 清理 PendingManifest
            PendingManifests::<T>::remove(evidence_id);

            // 移除证据记录
            Evidences::<T>::remove(evidence_id);
            EvidenceStatuses::<T>::insert(evidence_id, EvidenceStatus::Removed);

            Self::deposit_event(Event::EvidenceForceRemoved {
                id: evidence_id,
                domain: ev.domain,
                target_id: ev.target_id,
            });
            Ok(())
        }

        // ==================== P1-1: 证据撤回 ====================

        /// 撤回证据
        ///
        /// 仅证据所有者可撤回。已密封的证据不可撤回。
        /// 撤回后证据保留在链上但标记为 Withdrawn，不可再用于仲裁。
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::withdraw_evidence())]
        pub fn withdraw_evidence(
            origin: OriginFor<T>,
            evidence_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let ev = Evidences::<T>::get(evidence_id).ok_or(Error::<T>::NotFound)?;
            ensure!(ev.owner == who, Error::<T>::NotAuthorized);

            // 检查状态
            let status = EvidenceStatuses::<T>::get(evidence_id);
            ensure!(status == EvidenceStatus::Active, Error::<T>::InvalidEvidenceStatus);
            ensure!(!SealedEvidences::<T>::contains_key(evidence_id), Error::<T>::EvidenceSealed);

            // 标记为已撤回
            EvidenceStatuses::<T>::insert(evidence_id, EvidenceStatus::Withdrawn);

            Self::deposit_event(Event::EvidenceWithdrawn {
                id: evidence_id,
                owner: who,
            });
            Ok(())
        }

        // ==================== P1-3: 删除私密内容 (GDPR 合规) ====================

        /// 删除私密内容
        ///
        /// 仅创建者可删除。清理 PrivateContents + PrivateContentByCid + PrivateContentBySubject + AccessRequests。
        /// 删除前要求无除创建者外的活跃访问者（需先 revoke_access 所有用户）。
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::delete_private_content())]
        pub fn delete_private_content(
            origin: OriginFor<T>,
            content_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let content = PrivateContents::<T>::get(content_id)
                .ok_or(Error::<T>::PrivateContentNotFound)?;

            // 权限检查：仅创建者
            ensure!(content.creator == who, Error::<T>::AccessDenied);

            // 检查无除创建者外的活跃访问者
            let other_users = content.encrypted_keys.iter()
                .filter(|(u, _)| u != &who)
                .count();
            ensure!(other_users == 0, Error::<T>::ContentHasActiveUsers);

            // 清理索引
            PrivateContentByCid::<T>::remove(&content.cid);
            PrivateContentBySubject::<T>::remove((content.ns, content.subject_id), content_id);

            // 清理所有访问请求
            let _ = AccessRequests::<T>::clear_prefix(content_id, u32::MAX, None);

            // 清理密钥轮换历史
            let rotation_count = KeyRotationCounter::<T>::get(content_id);
            for round in 1..=rotation_count {
                KeyRotationHistory::<T>::remove(content_id, round);
            }
            KeyRotationCounter::<T>::remove(content_id);

            // 移除主记录
            PrivateContents::<T>::remove(content_id);

            Self::deposit_event(Event::PrivateContentDeleted {
                content_id,
                deleted_by: who,
            });
            Ok(())
        }

        // ==================== P1-6: 强制归档 ====================

        /// 强制归档证据（Root 操作）
        ///
        /// 管理员立即归档指定证据（不等 90 天），用于内容下架但保留审计线索。
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::force_archive_evidence())]
        pub fn force_archive_evidence(
            origin: OriginFor<T>,
            evidence_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let ev = Evidences::<T>::get(evidence_id).ok_or(Error::<T>::NotFound)?;
            let now: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();

            // 创建归档记录
            let content_hash = H256::from(blake2_256(&ev.content_cid.clone().into_inner()));
            let archived = ArchivedEvidence {
                id: evidence_id,
                domain: ev.domain,
                target_id: ev.target_id,
                content_hash,
                content_type: match ev.content_type {
                    ContentType::Image => 0,
                    ContentType::Video => 1,
                    ContentType::Document => 2,
                    ContentType::Mixed => 3,
                    ContentType::Text => 4,
                },
                created_at: ev.created_at.saturated_into(),
                archived_at: now,
                year_month: pallet_storage_lifecycle::block_to_year_month(now, 14400),
            };

            ArchivedEvidences::<T>::insert(evidence_id, archived);

            // 清理二级索引
            EvidenceByTarget::<T>::remove((ev.domain, ev.target_id), evidence_id);
            if let Some(ns) = &ev.ns {
                EvidenceByNs::<T>::remove((*ns, ev.target_id), evidence_id);
                EvidenceCountByNs::<T>::mutate((*ns, ev.target_id), |c| {
                    *c = c.saturating_sub(1);
                });
            }
            EvidenceCountByTarget::<T>::mutate((ev.domain, ev.target_id), |c| {
                *c = c.saturating_sub(1);
            });
            CidHashIndex::<T>::remove(content_hash);
            // 🆕 H1-R3修复: 清理 CommitIndex
            if let Some(commit) = &ev.commit {
                CommitIndex::<T>::remove(*commit);
            }

            // P1-5: 清理父子关系
            if let Some(parent_id) = EvidenceParent::<T>::get(evidence_id) {
                EvidenceChildren::<T>::mutate(parent_id, |children| {
                    children.retain(|&id| id != evidence_id);
                });
                EvidenceParent::<T>::remove(evidence_id);
            }
            // 🆕 H1-R4修复: 清理子证据的父引用 + 移除自身 children 列表
            // （与 archive_old_evidences / force_remove_evidence 对齐）
            let children_list = EvidenceChildren::<T>::get(evidence_id);
            for child_id in children_list.iter() {
                EvidenceParent::<T>::remove(child_id);
            }
            EvidenceChildren::<T>::remove(evidence_id);

            // 清理密封、状态、PendingManifest
            SealedEvidences::<T>::remove(evidence_id);
            EvidenceStatuses::<T>::remove(evidence_id);
            PendingManifests::<T>::remove(evidence_id);

            // 移除原始记录
            Evidences::<T>::remove(evidence_id);

            ArchiveStats::<T>::mutate(|stats| {
                stats.total_archived = stats.total_archived.saturating_add(1);
                stats.bytes_saved = stats.bytes_saved.saturating_add(150);
                stats.last_archive_block = now;
            });

            Self::deposit_event(Event::EvidenceForceArchived {
                id: evidence_id,
                domain: ev.domain,
                target_id: ev.target_id,
            });
            Ok(())
        }

        // ==================== P1-6: 撤销公钥 ====================

        /// 撤销用户公钥
        ///
        /// 密钥泄露时用户可作废旧公钥。撤销后需重新注册才能接收新的加密内容。
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::register_public_key())]
        pub fn revoke_public_key(
            origin: OriginFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                UserPublicKeys::<T>::contains_key(&who),
                Error::<T>::PublicKeyNotRegistered
            );

            UserPublicKeys::<T>::remove(&who);

            Self::deposit_event(Event::PublicKeyRevoked { user: who });
            Ok(())
        }

        // ==================== P2: 取消访问请求 ====================

        /// 取消访问请求
        ///
        /// 用户取消自己对某加密内容的待处理访问请求。
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::request_access())]
        pub fn cancel_access_request(
            origin: OriginFor<T>,
            content_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                AccessRequests::<T>::contains_key(content_id, &who),
                Error::<T>::AccessRequestNotFound
            );

            AccessRequests::<T>::remove(content_id, &who);

            Self::deposit_event(Event::AccessRequestCancelled {
                content_id,
                requester: who,
            });
            Ok(())
        }

        // 只读接口应放置在 inherent impl 中，而非 extrinsics 块。
    }

    /// 承诺哈希和验证工具函数
    impl<T: Config> Pallet<T> {
        /// 计算 Evidence 承诺哈希
        ///
        /// 使用 nexus-media-common 的 HashHelper 计算标准格式的承诺哈希:
        /// H(ns || subject_id || cid || salt || version)
        ///
        /// # 参数
        /// - `ns`: 8字节命名空间
        /// - `subject_id`: 主体ID
        /// - `cid`: IPFS CID数据
        /// - `salt`: 盐值
        /// - `version`: 版本号（通常为1）
        ///
        /// # 返回
        /// - 计算得到的 H256 承诺哈希
        pub fn compute_evidence_commitment(
            ns: &[u8; 8],
            subject_id: u64,
            cid: &[u8],
            salt: &[u8],
            version: u32,
        ) -> H256 {
            HashHelper::evidence_commitment(ns, subject_id, cid, salt, version)
        }

        /// 验证承诺哈希是否正确
        ///
        /// # 参数
        /// - `ns`: 8字节命名空间
        /// - `subject_id`: 主体ID
        /// - `cid`: IPFS CID数据
        /// - `salt`: 盐值
        /// - `version`: 版本号
        /// - `expected_commit`: 期望的承诺哈希
        ///
        /// # 返回
        /// - `true`: 验证通过
        /// - `false`: 验证失败
        pub fn verify_evidence_commitment(
            ns: &[u8; 8],
            subject_id: u64,
            cid: &[u8],
            salt: &[u8],
            version: u32,
            expected_commit: &H256,
        ) -> bool {
            let computed = Self::compute_evidence_commitment(ns, subject_id, cid, salt, version);
            &computed == expected_commit
        }

        /// 验证 CID 格式（单个）
        ///
        /// 使用 nexus-media-common 的 IpfsHelper 验证单个 CID 格式。
        pub fn validate_single_cid(cid: &[u8]) -> Result<(), Error<T>> {
            let cid_str = core::str::from_utf8(cid)
                .map_err(|_| Error::<T>::InvalidCidFormat)?;

            IpfsHelper::validate_cid(cid_str)
                .map_err(|_| Error::<T>::InvalidCidFormat)
        }

        /// 验证内容完整性
        ///
        /// 使用 nexus-media-common 的 IpfsHelper 验证内容与 CID 的对应关系。
        /// 注意：此函数仅在有实际内容数据时使用。
        pub fn verify_content_integrity(content_data: &[u8], cid: &str) -> bool {
            IpfsHelper::verify_content(content_data, cid)
        }
    }

    /// 授权适配接口：由 runtime 实现并桥接到 `pallet-authorizer`，以保持低耦合。
    pub trait EvidenceAuthorizer<AccountId> {
        /// 校验某账户是否在给定命名空间下被授权提交/链接证据
        fn is_authorized(ns: [u8; 8], who: &AccountId) -> bool;
    }

    /// P1-2: 证据摘要信息（跨 pallet 查询用，轻量级）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Debug)]
    pub struct EvidenceInfo<AccountId> {
        pub id: u64,
        pub owner: AccountId,
        pub domain: u8,
        pub target_id: u64,
        pub is_encrypted: bool,
        pub status: EvidenceStatus,
    }

    /// P1-2: 只读查询 trait：供其他 pallet 低耦合读取证据信息。
    pub trait EvidenceProvider<AccountId> {
        /// 返回指定 ID 的证据摘要信息
        fn get(id: u64) -> Option<EvidenceInfo<AccountId>>;

        /// 检查证据是否存在
        fn exists(id: u64) -> bool;

        /// 获取证据状态
        fn get_status(id: u64) -> Option<EvidenceStatus>;

        /// 检查证据是否处于活跃状态
        fn is_active(id: u64) -> bool {
            matches!(Self::get_status(id), Some(EvidenceStatus::Active))
        }
    }

    /// P1-2: Pallet 实现 EvidenceProvider
    impl<T: Config> EvidenceProvider<T::AccountId> for Pallet<T> {
        fn get(id: u64) -> Option<EvidenceInfo<T::AccountId>> {
            Evidences::<T>::get(id).map(|ev| EvidenceInfo {
                id: ev.id,
                owner: ev.owner,
                domain: ev.domain,
                target_id: ev.target_id,
                is_encrypted: ev.is_encrypted,
                status: EvidenceStatuses::<T>::get(id),
            })
        }

        fn exists(id: u64) -> bool {
            Evidences::<T>::contains_key(id)
        }

        fn get_status(id: u64) -> Option<EvidenceStatus> {
            if Evidences::<T>::contains_key(id) {
                Some(EvidenceStatuses::<T>::get(id))
            } else {
                None
            }
        }
    }

    /// pallet_crypto_common::PrivateContentProvider 实现：
    /// 供其他 pallet 低耦合查询加密内容权限和密钥
    impl<T: Config> pallet_crypto_common::PrivateContentProvider<T::AccountId> for Pallet<T> {
        fn can_access(content_id: u64, user: &T::AccountId) -> bool {
            Self::can_access_private_content(content_id, user)
        }

        fn get_encrypted_key(content_id: u64, user: &T::AccountId) -> Option<Vec<u8>> {
            Self::get_encrypted_key_for_user(content_id, user)
                .map(|k| k.into_inner())
        }

        fn get_decryption_info(
            content_id: u64,
            user: &T::AccountId,
        ) -> Option<(Vec<u8>, sp_core::H256, pallet_crypto_common::EncryptionMethod, Vec<u8>)> {
            Pallet::<T>::get_decryption_info(content_id, user)
        }

        fn get_content_status(content_id: u64) -> Option<pallet_crypto_common::ContentStatus> {
            PrivateContents::<T>::get(content_id).map(|c| c.status)
        }

        fn get_content_creator(content_id: u64) -> Option<T::AccountId> {
            PrivateContents::<T>::get(content_id).map(|c| c.creator)
        }
    }

    impl<T: Config> Pallet<T> {
        // ===== 私密内容查询方法 =====

        /// 检查用户是否有访问特定私密内容的权限
        ///
        /// 使用 AccessPolicy::is_authorized() 统一判断，避免重复 match 逻辑
        pub fn can_access_private_content(content_id: u64, user: &T::AccountId) -> bool {
            if let Some(content) = PrivateContents::<T>::get(content_id) {
                // 已清除的内容不可访问
                if !content.status.is_readable() {
                    return false;
                }

                // 检查是否持有加密密钥包（由创建者通过 grant_access 显式授予）
                if content.encrypted_keys.iter().any(|(u, _)| u == user) {
                    return true;
                }

                // 使用 AccessPolicy 的统一授权检查
                let now = <frame_system::Pallet<T>>::block_number();
                if content.access_policy.is_authorized(user, &content.creator, &now) {
                    return true;
                }

                // P1-4: GovernanceControlled 额外检查 — 委托给 Authorizer trait
                // pallet-crypto-common 对 GovernanceControlled 返回 false（需外部逻辑），
                // 此处补充：在治理模式下，任何被命名空间授权的账户均可访问
                if matches!(content.access_policy, pallet_crypto_common::AccessPolicy::GovernanceControlled) {
                    let ns = T::EvidenceNsBytes::get();
                    return <T as Config>::Authorizer::is_authorized(ns, user);
                }

                false
            } else {
                false
            }
        }

        /// 获取用户的加密密钥包
        pub fn get_encrypted_key_for_user(
            content_id: u64,
            user: &T::AccountId,
        ) -> Option<BoundedVec<u8, T::MaxKeyLen>> {
            if let Some(content) = PrivateContents::<T>::get(content_id) {
                if Self::can_access_private_content(content_id, user) {
                    content
                        .encrypted_keys
                        .iter()
                        .find(|(u, _)| u == user)
                        .map(|(_, key)| key.clone())
                } else {
                    None
                }
            } else {
                None
            }
        }

        /// 通过CID查找私密内容
        pub fn get_private_content_by_cid(
            cid: &BoundedVec<u8, T::MaxCidLen>,
        ) -> Option<private_content::PrivateContent<T>> {
            if let Some(content_id) = PrivateContentByCid::<T>::get(cid) {
                PrivateContents::<T>::get(content_id)
            } else {
                None
            }
        }

        /// 获取主体下的所有私密内容ID
        pub fn get_private_content_ids_by_subject(ns: [u8; 8], subject_id: u64) -> Vec<u64> {
            PrivateContentBySubject::<T>::iter_prefix((ns, subject_id))
                .map(|(content_id, _)| content_id)
                .collect()
        }

        /// 获取解密所需的全部信息（供客户端调用）
        ///
        /// 返回：(cid, content_hash, encryption_method, encrypted_key)
        /// 如果用户无权访问，返回 None
        pub fn get_decryption_info(
            content_id: u64,
            user: &T::AccountId,
        ) -> Option<(Vec<u8>, sp_core::H256, pallet_crypto_common::EncryptionMethod, Vec<u8>)> {
            let content = PrivateContents::<T>::get(content_id)?;
            if !Self::can_access_private_content(content_id, user) {
                return None;
            }
            let encrypted_key = content
                .encrypted_keys
                .iter()
                .find(|(u, _)| u == user)
                .map(|(_, k)| k.clone().into_inner())?;
            Some((
                content.cid.into_inner(),
                content.content_hash,
                content.encryption_method,
                encrypted_key,
            ))
        }

        /// 获取加密内容的公开元数据（不含密钥包）
        ///
        /// 任何人可查询：id, ns, subject_id, cid, encryption_method, creator, access_policy 类型
        pub fn get_content_metadata(
            content_id: u64,
        ) -> Option<(u64, [u8; 8], u64, Vec<u8>, pallet_crypto_common::EncryptionMethod, T::AccountId, u8)> {
            let c = PrivateContents::<T>::get(content_id)?;
            let policy_tag: u8 = match &c.access_policy {
                pallet_crypto_common::AccessPolicy::OwnerOnly => 0,
                pallet_crypto_common::AccessPolicy::SharedWith(_) => 1,
                pallet_crypto_common::AccessPolicy::TimeboxedAccess { .. } => 2,
                pallet_crypto_common::AccessPolicy::GovernanceControlled => 3,
                pallet_crypto_common::AccessPolicy::RoleBased(_) => 4,
            };
            Some((c.id, c.ns, c.subject_id, c.cid.into_inner(), c.encryption_method, c.creator, policy_tag))
        }

        /// 列出某加密内容的所有待处理访问请求
        pub fn list_access_requests(content_id: u64) -> Vec<(T::AccountId, BlockNumberFor<T>)> {
            AccessRequests::<T>::iter_prefix(content_id).collect()
        }

        /// 获取用户的注册公钥
        pub fn get_user_public_key(user: &T::AccountId) -> Option<UserPublicKey<T>> {
            UserPublicKeys::<T>::get(user)
        }

        /// 函数级中文注释：限频检查并计数。
        /// - 进入窗口：超过 WindowBlocks 自动滚动窗口并清零计数；严格小于最大次数方可提交。
        fn touch_window(who: &T::AccountId, now: BlockNumberFor<T>) -> Result<(), Error<T>> {
            AccountWindows::<T>::mutate(who, |w| {
                let wb = T::WindowBlocks::get();
                if now.saturating_sub(w.window_start) >= wb {
                    w.window_start = now;
                    w.count = 0;
                }
            });
            let info = AccountWindows::<T>::get(who);
            ensure!(info.count < T::MaxPerWindow::get(), Error::<T>::RateLimited);
            AccountWindows::<T>::mutate(who, |w| {
                w.count = w.count.saturating_add(1);
            });
            Ok(())
        }

        /// 函数级中文注释：校验一组 CID 的格式与去重要求。
        /// 规则：每个 CID 必须非空、符合IPFS格式规范；组内不得重复。
        /// 使用 nexus-media-common 的 IpfsHelper 进行规范验证。
        fn validate_cid_vec(list: &[BoundedVec<u8, T::MaxCidLen>]) -> Result<(), Error<T>> {
            let mut set: BTreeSet<Vec<u8>> = BTreeSet::new();
            for cid in list.iter() {
                if cid.is_empty() {
                    return Err(Error::<T>::InvalidCidFormat);
                }

                // 转换为字符串进行IPFS规范验证
                let cid_str = core::str::from_utf8(cid.as_slice())
                    .map_err(|_| Error::<T>::InvalidCidFormat)?;

                // 使用 nexus-media-common 的 IpfsHelper 进行规范验证
                IpfsHelper::validate_cid(cid_str)
                    .map_err(|_| Error::<T>::InvalidCidFormat)?;

                // 检查重复
                let v: Vec<u8> = cid.clone().into_inner();
                if !set.insert(v) {
                    return Err(Error::<T>::DuplicateCid);
                }
            }
            Ok(())
        }

        /// 函数级中文注释：可选的全局 CID 去重检查（Plain 模式）。
        /// - EnableGlobalCidDedup=true 时，逐个 CID 计算 blake2_256 并查重；首次出现时在提交成功后写入索引。
        fn ensure_global_cid_unique(
            list_groups: [&[BoundedVec<u8, T::MaxCidLen>]; 3],
        ) -> Result<(), Error<T>> {
            if !T::EnableGlobalCidDedup::get() {
                return Ok(());
            }
            for list in list_groups.into_iter() {
                for cid in list.iter() {
                    let h = H256::from(blake2_256(&cid.clone().into_inner()));
                    if CidHashIndex::<T>::get(h).is_some() {
                        return Err(Error::<T>::DuplicateCidGlobal);
                    }
                }
            }
            Ok(())
        }

        // ==================== 存储膨胀防护：归档辅助函数 ====================

        /// 函数级中文注释：归档旧证据
        /// 
        /// 参数：
        /// - max_count: 每次最多处理的证据数量
        /// 
        /// 返回：已归档的证据数量
        /// 
        /// 归档条件：证据创建时间超过 90 天（1_296_000 区块）
        pub fn archive_old_evidences(max_count: u32) -> u32 {
            let now: u32 = frame_system::Pallet::<T>::block_number().saturated_into();
            // 🆕 M-NEW-5修复: 使用 Config 常量替代硬编码归档延迟
            let archive_delay: u32 = T::ArchiveDelayBlocks::get();
            let mut archived_count = 0u32;
            let mut cursor = EvidenceArchiveCursor::<T>::get();
            let max_id = NextEvidenceId::<T>::get();

            while archived_count < max_count && cursor < max_id {
                if let Some(evidence) = Evidences::<T>::get(cursor) {
                    // 🆕 H1修复: 跳过已密封的证据（仲裁冻结中，不可归档）
                    if SealedEvidences::<T>::contains_key(cursor) {
                        cursor = cursor.saturating_add(1);
                        continue;
                    }
                    let created_at: u32 = evidence.created_at.saturated_into();
                    
                    // 检查是否可归档：创建时间 + 归档延迟 <= 当前时间
                    if now.saturating_sub(created_at) >= archive_delay {
                        // 计算内容哈希
                        let content_hash = H256::from(blake2_256(&evidence.content_cid.clone().into_inner()));
                        
                        // 创建归档记录
                        let archived = ArchivedEvidence {
                            id: cursor,
                            domain: evidence.domain,
                            target_id: evidence.target_id,
                            content_hash,
                            content_type: match evidence.content_type {
                                ContentType::Image => 0,
                                ContentType::Video => 1,
                                ContentType::Document => 2,
                                ContentType::Mixed => 3,
                                ContentType::Text => 4,
                            },
                            created_at,
                            archived_at: now,
                            year_month: pallet_storage_lifecycle::block_to_year_month(now, 14400),
                        };

                        // 存储归档记录
                        ArchivedEvidences::<T>::insert(cursor, archived);

                        // 🆕 VM2修复: 清理二级索引（防止存储泄漏）
                        EvidenceByTarget::<T>::remove((evidence.domain, evidence.target_id), cursor);
                        if let Some(ns) = &evidence.ns {
                            EvidenceByNs::<T>::remove((*ns, evidence.target_id), cursor);
                            EvidenceCountByNs::<T>::mutate((*ns, evidence.target_id), |c| {
                                *c = c.saturating_sub(1);
                            });
                        }
                        EvidenceCountByTarget::<T>::mutate(
                            (evidence.domain, evidence.target_id), |c| {
                                *c = c.saturating_sub(1);
                            },
                        );
                        // 清理 CID 哈希索引
                        CidHashIndex::<T>::remove(content_hash);
                        // 🆕 H1-R3修复: 清理 CommitIndex（无论是否已揭示）
                        if let Some(commit) = &evidence.commit {
                            CommitIndex::<T>::remove(*commit);
                        }

                        // P1-5: 归档时清理父子关系
                        if let Some(parent_id) = EvidenceParent::<T>::get(cursor) {
                            EvidenceChildren::<T>::mutate(parent_id, |children| {
                                children.retain(|&id| id != cursor);
                            });
                            EvidenceParent::<T>::remove(cursor);
                        }
                        // 🆕 M3-R3修复: 清理子证据的父引用 + 移除自身 children 列表
                        // （与 force_remove_evidence 对齐，防止孤立存储泄漏）
                        let children_list = EvidenceChildren::<T>::get(cursor);
                        for child_id in children_list.iter() {
                            EvidenceParent::<T>::remove(child_id);
                        }
                        EvidenceChildren::<T>::remove(cursor);

                        // 清理密封、状态、PendingManifest
                        SealedEvidences::<T>::remove(cursor);
                        EvidenceStatuses::<T>::remove(cursor);
                        PendingManifests::<T>::remove(cursor);

                        // 移除原始证据记录（释放存储）
                        Evidences::<T>::remove(cursor);

                        // 更新统计（估算每条证据节省约 150 字节）
                        ArchiveStats::<T>::mutate(|stats| {
                            stats.total_archived = stats.total_archived.saturating_add(1);
                            stats.bytes_saved = stats.bytes_saved.saturating_add(150);
                            stats.last_archive_block = now;
                        });

                        Self::deposit_event(Event::EvidenceArchived {
                            id: cursor,
                            domain: evidence.domain,
                            target_id: evidence.target_id,
                        });

                        archived_count = archived_count.saturating_add(1);
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            EvidenceArchiveCursor::<T>::put(cursor);
            archived_count
        }

        /// P1-5: 清理过期的 PendingManifest（编辑窗口已过期）
        ///
        /// 游标式：从 PendingManifestCleanupCursor 开始扫描，删除已过期的 PendingManifest。
        fn cleanup_expired_pending_manifests(now: BlockNumberFor<T>, max_per_call: u32) -> u32 {
            let edit_window = T::EvidenceEditWindow::get();
            let mut cursor = PendingManifestCleanupCursor::<T>::get();
            let max_id = NextEvidenceId::<T>::get();
            let mut cleaned = 0u32;

            while cursor < max_id && cleaned < max_per_call {
                if let Some(pending) = PendingManifests::<T>::get(cursor) {
                    if now > pending.created_at.saturating_add(edit_window) {
                        PendingManifests::<T>::remove(cursor);
                        cleaned += 1;
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            PendingManifestCleanupCursor::<T>::put(cursor);
            cleaned
        }

        /// 🆕 防膨胀: 清理过期归档记录
        ///
        /// 游标式: 从 ArchiveCleanupCursor 开始扫描 ArchivedEvidences,
        /// 删除 archived_at + ArchiveTtlBlocks < current_block 的记录。
        /// 每次最多清理 max_per_call 条。
        fn cleanup_old_archives(current_block: u32, max_per_call: u32) -> u32 {
            let ttl = T::ArchiveTtlBlocks::get();
            if ttl == 0 {
                return 0; // TTL=0 表示不清理
            }
            let mut cursor = ArchiveCleanupCursor::<T>::get();
            let max_id = NextEvidenceId::<T>::get();
            let mut cleaned = 0u32;

            while cursor < max_id && cleaned < max_per_call {
                if let Some(archived) = ArchivedEvidences::<T>::get(cursor) {
                    if current_block.saturating_sub(archived.archived_at) > ttl {
                        ArchivedEvidences::<T>::remove(cursor);
                        cleaned += 1;
                    } else {
                        // 归档记录按时间单调递增, 遇到未过期的可以停止
                        break;
                    }
                }
                cursor = cursor.saturating_add(1);
            }

            ArchiveCleanupCursor::<T>::put(cursor);
            cleaned
        }
    }

    // ==================== 存储膨胀防护：Hooks 实现 ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 函数级中文注释：空闲时间归档旧证据 + 清理过期归档
        fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let mut weight_used = Weight::zero();
            // 🆕 V4修复: 每项归档涉及 Evidences(r) + ArchivedEvidences(w) + 二级索引清理(rw) ≈ 30M/项
            let base_weight = Weight::from_parts(30_000_000, 2_500);
            // 🆕 M4-R3修复: 扫描开销 — 每次迭代读 Evidences + SealedEvidences，无论是否归档
            let per_scan_weight = Weight::from_parts(10_000_000, 1_500);
            let max_archive = 10u32;

            // 确保有足够权重处理扫描 + 至少 1 条归档
            let scan_overhead = per_scan_weight.saturating_mul(max_archive as u64);
            if remaining_weight.ref_time() > scan_overhead.ref_time().saturating_add(base_weight.ref_time()) {
                let archived = Self::archive_old_evidences(max_archive);
                // 始终计入扫描开销（最多 max_archive 次迭代）+ 实际归档开销
                weight_used = weight_used
                    .saturating_add(scan_overhead)
                    .saturating_add(base_weight.saturating_mul(archived as u64));
            }

            // 🆕 防膨胀: 清理过期归档记录
            let remaining = remaining_weight.saturating_sub(weight_used);
            if remaining.ref_time() > base_weight.ref_time() * 5 {
                let current_block: u32 = now.saturated_into();
                let cleaned = Self::cleanup_old_archives(current_block, 5);
                weight_used = weight_used.saturating_add(base_weight.saturating_mul(cleaned as u64));
            }

            // P1-5: 清理过期 PendingManifest
            let remaining2 = remaining_weight.saturating_sub(weight_used);
            if remaining2.ref_time() > base_weight.ref_time() * 5 {
                let cleaned = Self::cleanup_expired_pending_manifests(now, 5);
                weight_used = weight_used.saturating_add(base_weight.saturating_mul(cleaned as u64));
            }

            weight_used
        }
    }
}

// ===== 只读方法（模块外部，避免 non_local_definitions）=====
impl<T: pallet::Config> Pallet<T> {
    /// 函数级中文注释：只读-按 (domain,target) 分页列出 evidence id（从 start_id 起，最多 MaxListLen 条）。
    pub fn list_ids_by_target(
        domain: u8,
        target_id: u64,
        start_id: u64,
        limit: u32,
    ) -> alloc::vec::Vec<u64> {
        let mut out: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
        let mut cnt: u32 = 0;
        let cap = core::cmp::min(limit, T::MaxListLen::get());
        for id in pallet::EvidenceByTarget::<T>::iter_key_prefix((domain, target_id)) {
            if id < start_id {
                continue;
            }
            out.push(id);
            cnt = cnt.saturating_add(1);
            if cnt >= cap {
                break;
            }
        }
        out
    }

    /// 函数级中文注释：只读-按 (ns,subject_id) 分页列出 evidence id（从 start_id 起，最多 MaxListLen 条）。
    pub fn list_ids_by_ns(
        ns: [u8; 8],
        subject_id: u64,
        start_id: u64,
        limit: u32,
    ) -> alloc::vec::Vec<u64> {
        let mut out: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
        let mut cnt: u32 = 0;
        let cap = core::cmp::min(limit, T::MaxListLen::get());
        for id in pallet::EvidenceByNs::<T>::iter_key_prefix((ns, subject_id)) {
            if id < start_id {
                continue;
            }
            out.push(id);
            cnt = cnt.saturating_add(1);
            if cnt >= cap {
                break;
            }
        }
        out
    }

    /// 函数级中文注释：只读-获取主体证据数量。
    pub fn count_by_target(domain: u8, target_id: u64) -> u32 {
        pallet::EvidenceCountByTarget::<T>::get((domain, target_id))
    }
    pub fn count_by_ns(ns: [u8; 8], subject_id: u64) -> u32 {
        pallet::EvidenceCountByNs::<T>::get((ns, subject_id))
    }

}
