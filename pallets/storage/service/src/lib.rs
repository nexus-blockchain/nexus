#![cfg_attr(not(feature = "std"), no_std)]
//! 说明：临时全局允许 `deprecated`（RuntimeEvent/常量权重），后续基准权重接入后移除
#![allow(deprecated)]

extern crate alloc;

use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
    BoundedVec,
};
use frame_system::pallet_prelude::*;
// （已下线）移除对 memo-endowment 的接口依赖
use alloc::string::String;
use codec::Encode;
use serde_json::Value as JsonValue;
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
    offchain::{http, StorageKind},
    traits::AtLeast32BitUnsigned,
};
use sp_std::vec::Vec;

/// 函数级详细中文注释：优化后的类型定义模块
/// 
/// 包含以下核心类型：
/// - 分层配置（PinTier, TierConfig）
/// - Subject管理（SubjectType, SubjectInfo）
/// - 健康巡检（HealthCheckTask, HealthStatus, GlobalHealthStats）
/// - 周期扣费（BillingTask, ChargeLayer, GraceStatus）
pub mod types;
pub mod runtime_api;

// 导出 runtime API
pub use runtime_api::*;

// 导出常用类型，方便其他模块使用
pub use types::{
    BillingTask, ChargeLayer, ChargeResult, DomainStats, GraceStatus, GlobalHealthStats, HealthCheckTask,
    HealthStatus, LayeredOperatorSelection, LayeredPinAssignment, OperatorLayer,
    OperatorMetrics, OperatorPinHealth, PinTier, SimpleNodeStats, SimplePinStatus,
    StorageLayerConfig, SubjectInfo, SubjectType, TierConfig, UnpinReason,
};

/// 函数级详细中文注释：Subject所有者只读提供者（低耦合）
/// 
/// ### 功能
/// - 从业务pallet读取owner字段（当前所有者）
/// - 用于权限检查
/// 
/// ### 设计理念
/// - **owner可转让**：支持所有权转移
/// - **权限控制**：用于检查操作权限
/// - **低耦合设计**：通过trait解耦，不直接依赖业务pallet
/// 
/// ### 使用场景
/// - 权限检查：request_pin_for_subject等操作
/// - subject存在性检查
pub trait SubjectOwnerProvider<AccountId> {
    /// 返回subject的owner（当前所有者）
    /// 
    /// ### 参数
    /// - `subject_id`: Subject ID
    /// 
    /// ### 返回
    /// - `Some(owner)`: subject存在，返回当前所有者账户
    /// - `None`: subject不存在
    fn owner_of(subject_id: u64) -> Option<AccountId>;
}

/// 专用 Offchain 签名 KeyType。注意：需要在节点端注册对应密钥。
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ipfs");

/// 函数级详细中文注释：OCW 专用签名算法类型
/// - 使用 sr25519 作为默认曲线；
/// - 节点 keystore 中通过 `--key` 或 RPC 注入该类型的密钥；
pub mod sr25519_app {
    use super::KEY_TYPE;
    use sp_application_crypto::{app_crypto, sr25519};
    app_crypto!(sr25519, KEY_TYPE);
}

pub type AuthorityId = sr25519_app::Public;

/// 函数级详细中文注释：IPFS自动pin接口，供其他pallet调用实现内容自动固定
/// 
/// 设计目标：
/// - 为各业务pallet（evidence、otc等）提供统一的pin接口；
/// - 自动使用四层扣费机制（IpfsPool配额 → SubjectFunding → IpfsPool兜底 → GracePeriod）；
/// - 支持Subject维度的CID固定。
/// 
/// 使用方式：
/// ```rust
/// // 在业务pallet的Config中添加：
/// type IpfsPinner: IpfsPinner<Self::AccountId, Self::Balance>;
/// 
/// // 在extrinsic中调用：
/// T::IpfsPinner::pin_cid_for_subject(
///     who.clone(),
///     SubjectType::Evidence,
///     subject_id,
///     cid,
///     Some(PinTier::Critical),
/// )?;
/// ```
pub trait IpfsPinner<AccountId, Balance> {
    /// 函数级详细中文注释：为Subject关联的CID发起pin请求
    /// 
    /// 参数：
    /// - `caller`: 发起调用的账户
    /// - `subject_type`: Subject类型（Evidence/OtcOrder/General等）
    /// - `subject_id`: Subject ID（用于派生SubjectFunding账户）
    /// - `cid`: IPFS CID（Vec<u8>格式）
    /// - `tier`: 分层等级（None则使用默认Standard）
    /// 
    /// 返回：
    /// - `Ok(())`: pin请求成功提交，费用扣取成功
    /// - `Err(...)`: 失败原因（余额不足、CID格式错误、系统错误等）
    /// 
    /// 扣费机制（四层回退）：
    /// 1. 优先从 `IpfsPoolAccount` 扣取（配额优先）
    /// 2. 如配额不足，从 `SubjectFunding(subject_id)` 扣取
    /// 3. 如SubjectFunding不足，从 `IpfsPoolAccount` 兜底
    /// 4. 如仍失败，进入宽限期（GracePeriod）
    /// 
    /// 分层配置：
    /// - Critical：5副本，6小时巡检，1.5x费率
    /// - Standard：3副本，24小时巡检，1.0x费率（默认）
    /// - Temporary：1副本，7天巡检，0.5x费率
    fn pin_cid_for_subject(
        caller: AccountId,
        subject_type: SubjectType,
        subject_id: u64,
        cid: Vec<u8>,
        tier: Option<PinTier>,
    ) -> DispatchResult;

    /// 函数级详细中文注释：取消固定CID
    /// 
    /// 参数：
    /// - `caller`: 发起调用的账户（必须是原Pin请求者）
    /// - `cid`: 要取消固定的IPFS CID
    /// 
    /// 返回：
    /// - `Ok(())`: unpin请求成功提交
    /// - `Err(...)`: 失败原因（CID不存在、无权限等）
    /// 
    /// 行为：
    /// - 标记CID为待删除状态
    /// - OCW将在后续区块执行物理删除
    /// - 停止后续扣费
    fn unpin_cid(caller: AccountId, cid: Vec<u8>) -> DispatchResult;
}

/// 函数级详细中文注释：内容注册接口 - 新pallet域自动PIN机制
/// 
/// 设计目标：
/// - 为新业务pallet提供统一的内容注册接口
/// - 自动处理域注册、CID固定、费用扣除
/// - 无需了解IPFS内部实现细节
/// - 支持任意自定义域扩展
/// 
/// 使用方式：
/// ```rust
/// // 在新业务pallet的Config中添加：
/// type ContentRegistry: ContentRegistry;
/// 
/// // 在extrinsic中一行代码完成注册：
/// T::ContentRegistry::register_content(
///     b"my-pallet".to_vec(),  // 域名
///     subject_id,             // 主体ID
///     cid,                    // 内容CID
///     PinTier::Standard,      // Pin等级
/// )?;
/// ```
/// 
/// 优势：
/// - ✅ 简单易用：一行代码完成所有操作
/// - ✅ 自动化：自动创建SubjectType、注册域、执行PIN
/// - ✅ 低耦合：业务逻辑与存储逻辑完全解耦
/// - ✅ 可扩展：支持未来任意新业务pallet
pub trait ContentRegistry {
    /// 函数级详细中文注释：注册内容到IPFS（自动域管理）
    /// 
    /// 功能：
    /// 1. 自动创建或使用现有域
    /// 2. 派生SubjectFunding账户
    /// 3. 执行PIN操作
    /// 4. 自动扣费（三层机制）
    /// 
    /// 参数：
    /// - `domain`: 域名（如 b"evidence", b"nft-metadata"）
    /// - `subject_id`: 主体ID（与域组合唯一标识）
    /// - `cid`: IPFS内容标识符
    /// - `tier`: Pin等级（Critical/Standard/Temporary）
    /// 
    /// 返回：
    /// - `Ok(())`: 注册成功，内容已PIN
    /// - `Err(...)`: 失败原因
    fn register_content(
        domain: Vec<u8>,
        subject_id: u64,
        cid: Vec<u8>,
        tier: PinTier,
    ) -> DispatchResult;
    
    /// 函数级详细中文注释：查询域是否已注册
    /// 
    /// 用途：检查域是否已在系统中注册
    fn is_domain_registered(domain: &[u8]) -> bool;
    
    /// 函数级详细中文注释：获取域的SubjectType映射
    /// 
    /// 用途：查询域对应的SubjectType
    fn get_domain_subject_type(domain: &[u8]) -> Option<SubjectType>;

    /// 函数级详细中文注释：取消注册内容（Unpin）
    /// 
    /// 功能：
    /// 1. 标记 CID 为待删除状态
    /// 2. OCW 将在后续区块执行物理删除
    /// 3. 停止后续扣费
    /// 
    /// 参数：
    /// - `domain`: 域名
    /// - `cid`: IPFS 内容标识符
    /// 
    /// 返回：
    /// - `Ok(())`: 取消注册成功
    /// - `Err(...)`: 失败原因
    fn unregister_content(
        domain: Vec<u8>,
        cid: Vec<u8>,
    ) -> DispatchResult;
}

/// 函数级详细中文注释：CID 锁定管理器接口
/// 
/// 设计目标：
/// - 支持仲裁期间锁定证据 CID，防止被删除
/// - 仲裁完成后自动解锁
/// - 支持过期时间设置
pub trait CidLockManager<Hash, BlockNumber> {
    /// 锁定 CID（防止删除）
    /// 
    /// 参数：
    /// - `cid_hash`: CID 的哈希值
    /// - `reason`: 锁定原因（如 "arbitration:otc:123"）
    /// - `until`: 可选的锁定到期区块号
    fn lock_cid(cid_hash: Hash, reason: Vec<u8>, until: Option<BlockNumber>) -> DispatchResult;
    
    /// 解锁 CID
    /// 
    /// 参数：
    /// - `cid_hash`: CID 的哈希值
    /// - `reason`: 锁定原因（必须匹配）
    fn unlock_cid(cid_hash: Hash, reason: Vec<u8>) -> DispatchResult;
    
    /// 检查 CID 是否被锁定
    fn is_locked(cid_hash: &Hash) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec;  // 添加vec宏导入（no_std环境）
    use frame_support::traits::ConstU32;
    use frame_support::traits::StorageVersion;
    use sp_runtime::traits::Saturating;
    use sp_runtime::SaturatedConversion;
    // 已移除签名交易上报，避免对 CreateSignedTransaction 约束
    use alloc::string::ToString;
    use frame_support::traits::tokens::Imbalance;
    use frame_support::PalletId;
    use sp_runtime::traits::AccountIdConversion;

    /// 余额别名
    pub type BalanceOf<T> = <T as Config>::Balance;

    /// 函数级中文注释：Pin元信息结构体
    /// - replicas: 副本数
    /// - size: 文件大小（字节）
    /// - created_at: 创建时间（区块号）
    /// - last_activity: 最后活动时间（区块号）
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(BlockNumber))]
    pub struct PinMetadata<BlockNumber> {
        pub replicas: u32,
        pub size: u64,
        pub created_at: BlockNumber,
        pub last_activity: BlockNumber,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// 货币接口（用于预留押金或扣费）
        type Currency: Currency<Self::AccountId, Balance = Self::Balance>
            + ReservableCurrency<Self::AccountId>;
        /// 余额类型
        type Balance: Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;

        /// 资金接收账户解析器（例如 Treasury 或平台账户），用于收取一次性费用与周期扣费。
        type FeeCollector: sp_core::Get<Self::AccountId>;

        /// 治理 Origin（用于参数/黑名单/配额）
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        // 已移除：OCW 签名标识（当前版本不从 OCW 发送签名交易）

        /// 最大支持的 `cid_hash` 长度（字节）
        #[pallet::constant]
        type MaxCidHashLen: Get<u32>;

        /// 最大支持的 PeerId 字节长度（Base58 文本或多地址指纹摘要）
        #[pallet::constant]
        type MaxPeerIdLen: Get<u32>;

        /// 最小运营者保证金兜底值（NEX 最小单位，pricing不可用时使用）
        #[pallet::constant]
        type MinOperatorBond: Get<Self::Balance>;

        /// 最小运营者保证金USD价值（精度10^6，100_000_000 = 100 USDT）
        #[pallet::constant]
        type MinOperatorBondUsd: Get<u64>;

        /// 保证金计算器（统一的 USD 价值动态计算）
        type DepositCalculator: pallet_trading_common::DepositCalculator<Self::Balance>;

        /// 最小可宣告容量（GiB）
        #[pallet::constant]
        type MinCapacityGiB: Get<u32>;

        /// 权重信息占位
        type WeightInfo: WeightInfo;

        /// 函数级中文注释：派生“主题资金账户”的 PalletId（creator+subject_id 派生稳定地址）
        #[pallet::constant]
        type SubjectPalletId: Get<PalletId>;

    /// 函数级中文注释：IPFS 池账户（公共费用来源）
    /// 
    /// 说明：
    /// - 由 pallet-storage-treasury 定期补充（供奉路由 2% × 50%）
    /// - 用于为 subject 提供免费配额
    type IpfsPoolAccount: Get<Self::AccountId>;
    
    /// 函数级中文注释：运营者托管账户（服务费接收方）
    /// 
    /// 说明：
    /// - 接收所有 pin 服务费用
    /// - 待运营者完成任务后基于 SLA 分配
    type OperatorEscrowAccount: Get<Self::AccountId>;
    
    /// 函数级中文注释：每月公共费用配额
    /// 
    /// 说明：
    /// - 每个 subject 每月可使用的免费额度
    /// - 默认：100 NEX（可治理调整）
    #[pallet::constant]
    type MonthlyPublicFeeQuota: Get<BalanceOf<Self>>;
    
    /// 函数级中文注释：配额重置周期（区块数）
    /// 
    /// 说明：
    /// - 默认：100,800 × 4 = 403,200 区块 ≈ 28 天
    #[pallet::constant]
    type QuotaResetPeriod: Get<BlockNumberFor<Self>>;
    
    /// 函数级详细中文注释：默认扣费周期（区块数）
    /// 
    /// 说明：
    /// - 周期性扣费的间隔时间
    /// - 默认：100,800 区块 ≈ 7天（假设3秒/块）
    /// - 可通过治理调整
    #[pallet::constant]
    type DefaultBillingPeriod: Get<u32>;
}

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);
    
    /// 函数级详细中文注释：Genesis配置（初始化分层配置默认值）
    /// 
    /// 目的：
    /// - 在创世块时设置三层Pin配置的合理默认值
    /// - 可通过runtime中的GenesisConfig自定义初始值
    /// - 提供开箱即用的生产级配置
    // 注释：由于substrate新版本对GenesisConfig的Serialize/Deserialize要求较严格，
    // 暂时使用默认配置。链启动后可通过治理接口update_tier_config动态调整。
    // 
    // 默认配置已在TierConfig::default()、TierConfig::critical_default()、
    // TierConfig::temporary_default()中定义，get_tier_config会自动应用。

    /// 定价参数原始字节（骨架）
    #[pallet::storage]
    /// 函数级中文注释：定价参数原始字节（使用 BoundedVec 以满足 MaxEncodedLen 要求）
    pub type PricingParams<T: Config> = StorageValue<_, BoundedVec<u8, ConstU32<8192>>, ValueQuery>;

    /// 函数级中文注释：Pin 订单存储
    /// 
    /// Key: cid_hash
    /// Value: (payer, replicas, subject_id, size_bytes, deposit)
    #[pallet::storage]
    pub type PendingPins<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Hash, (T::AccountId, u32, u64, u64, T::Balance), OptionQuery>;

    /// Pin 元信息（副本数、大小、创建时间、最后巡检）
    #[pallet::storage]
    pub type PinMeta<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash,
        PinMetadata<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Pin 状态机：0=Requested,1=Pinning,2=Pinned,3=Degraded,4=Failed
    #[pallet::storage]
    pub type PinStateOf<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, u8, ValueQuery>;

    /// 副本分配：为每个 cid_hash 挑选的运营者账户
    #[pallet::storage]
    pub type PinAssignments<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash,
        BoundedVec<T::AccountId, frame_support::traits::ConstU32<16>>,
        OptionQuery,
    >;

    /// 分配内的成功标记：(cid_hash, operator) -> 成功与否
    #[pallet::storage]
    pub type PinSuccess<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::Hash,
        Blake2_128Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;

    /// 函数级详细中文注释：运营者信息结构体（优化版）
    /// 
    /// ### 字段说明
    /// - peer_id: IPFS节点的PeerID
    /// - capacity_gib: 声明的存储容量（GiB）
    /// - endpoint_hash: IPFS Cluster API端点的哈希
    /// - cert_fingerprint: TLS证书指纹（可选）
    /// - status: 运营者状态（0=Active, 1=Suspended, 2=Banned）
    /// - registered_at: 注册时间戳（区块高度）✅ P1新增
    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct OperatorInfo<T: Config> {
        pub peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
        pub capacity_gib: u32,
        pub endpoint_hash: T::Hash,
        pub cert_fingerprint: Option<T::Hash>,
        pub status: u8, // 0=Active,1=Suspended,2=Banned
        pub registered_at: BlockNumberFor<T>, // ✅ P1新增：注册时间戳
        pub layer: OperatorLayer, // ✅ Layer分层：Core/Community/External
        pub priority: u8, // ✅ 优先级：0-255（越小越优先，Core通常0-50，Community通常51-200）
    }

    /// 运营者注册表与保证金
    #[pallet::storage]
    pub type Operators<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, OperatorInfo<T>, OptionQuery>;

    #[pallet::storage]
    pub type OperatorBond<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 函数级详细中文注释：待注销运营者列表（宽限期机制）✅ P0-3新增
    /// 
    /// ### 用途
    /// - 记录已提交unregister但仍有Pin的运营者
    /// - Value: 宽限期到期时间（区块高度）
    /// - 宽限期内OCW自动迁移Pin到其他运营者
    /// - 宽限期结束后检查Pin数量，无Pin则返还保证金并移除记录
    /// 
    /// ### 宽限期设计
    /// - 默认7天（100,800块，假设6秒/块）
    /// - 可通过治理调整
    #[pallet::storage]
    pub type PendingUnregistrations<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>, OptionQuery>;

    /// 运营者 SLA 统计
    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct SlaStats<T: Config> {
        pub pinned_bytes: u64,
        pub probe_ok: u32,
        pub probe_fail: u32,
        pub degraded: u32,
        pub last_update: BlockNumberFor<T>,
    }

    impl<T: Config> Default for SlaStats<T> {
        /// 函数级中文注释：为 SlaStats<T> 提供显式的 Default 实现，避免对 T 施加 Default 约束
        /// - 将计数置 0，last_update 使用 BlockNumber 的默认值
        fn default() -> Self {
            Self {
                pinned_bytes: 0,
                probe_ok: 0,
                probe_fail: 0,
                degraded: 0,
                last_update: Default::default(),
            }
        }
    }

    #[pallet::storage]
    pub type OperatorSla<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, SlaStats<T>, ValueQuery>;

    // ====== 双重扣款配额管理 ======
    
    /// 函数级中文注释：公共费用配额使用记录
    /// 
    /// 说明：
    /// - 记录每个 subject 的月度配额使用情况
    /// - 超过配额自动切换到 SubjectFunding
    /// 
    /// Key: subject_id
    /// Value: (已使用金额, 配额重置区块号)
    #[pallet::storage]
    pub type PublicFeeQuotaUsage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // subject_id
        (BalanceOf<T>, BlockNumberFor<T>), // (used_amount, reset_block)
        ValueQuery,
    >;

    /// 函数级中文注释：累计从 IPFS 池扣款统计
    #[pallet::storage]
    pub type TotalChargedFromPool<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// 函数级中文注释：累计从 SubjectFunding 扣款统计
    #[pallet::storage]
    pub type TotalChargedFromSubject<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// 函数级中文注释：用户级存储资金账户余额追踪
    /// 
    /// 混合方案：每个用户一个派生账户，替代每个 Subject 一个账户
    /// Key: 用户账户
    /// Value: 累计充值金额（用于统计）
    #[pallet::storage]
    #[pallet::getter(fn user_funding_balance)]
    pub type UserFundingBalance<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 函数级中文注释：Subject 级费用使用追踪（不派生账户，仅记账）
    /// 
    /// 混合方案：记录每个 Subject 的存储费用消耗，便于审计
    /// Key: (用户账户, SubjectType, subject_id)
    /// Value: 累计消耗金额
    #[pallet::storage]
    #[pallet::getter(fn subject_usage)]
    pub type SubjectUsage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (T::AccountId, u8, u64),  // (user, subject_type_domain, subject_id)
        BalanceOf<T>,
        ValueQuery,
    >;

    // ====== 动态副本数配置 ======
    
    /// 函数级中文注释：推荐副本数配置（按重要性等级）
    /// 
    /// 说明：
    /// - 允许治理设置不同重要性等级的推荐副本数
    /// - Level 0: 临时文件（默认 2）
    /// - Level 1: 一般文件（默认 3）✅ 推荐
    /// - Level 2: 重要文件（默认 5）
    /// - Level 3: 关键文件（默认 7）
    #[pallet::type_value]
    pub fn DefaultReplicasForLevel0<T: Config>() -> u32 { 2 }
    #[pallet::type_value]
    pub fn DefaultReplicasForLevel1<T: Config>() -> u32 { 3 }
    #[pallet::type_value]
    pub fn DefaultReplicasForLevel2<T: Config>() -> u32 { 5 }
    #[pallet::type_value]
    pub fn DefaultReplicasForLevel3<T: Config>() -> u32 { 7 }
    
    #[pallet::storage]
    pub type ReplicasForLevel0<T: Config> = 
        StorageValue<_, u32, ValueQuery, DefaultReplicasForLevel0<T>>;
    
    #[pallet::storage]
    pub type ReplicasForLevel1<T: Config> = 
        StorageValue<_, u32, ValueQuery, DefaultReplicasForLevel1<T>>;
    
    #[pallet::storage]
    pub type ReplicasForLevel2<T: Config> = 
        StorageValue<_, u32, ValueQuery, DefaultReplicasForLevel2<T>>;
    
    #[pallet::storage]
    pub type ReplicasForLevel3<T: Config> = 
        StorageValue<_, u32, ValueQuery, DefaultReplicasForLevel3<T>>;
    
    /// 函数级中文注释：最小副本数阈值
    /// 
    /// 说明：
    /// - 当副本数低于此阈值时，OCW 自动补充
    /// - 默认：2（至少保证 2 个副本）
    #[pallet::type_value]
    pub fn DefaultMinReplicasThreshold<T: Config>() -> u32 { 2 }
    
    #[pallet::storage]
    pub type MinReplicasThreshold<T: Config> = 
        StorageValue<_, u32, ValueQuery, DefaultMinReplicasThreshold<T>>;

    // ====== 计费与生命周期（最小增量）======
    /// 函数级中文注释：每 GiB·周 单价（治理可调）。单位使用链上最小余额单位的整数，建议采用按字节的定点基数以避免小数。
    #[pallet::type_value]
    pub fn DefaultPricePerGiBWeek<T: Config>() -> u128 {
        1_000_000_000
    }
    #[pallet::storage]
    pub type PricePerGiBWeek<T: Config> =
        StorageValue<_, u128, ValueQuery, DefaultPricePerGiBWeek<T>>;

    /// 函数级中文注释：计费周期（块），默认一周（6s/块 × 60 × 60 × 24 × 7 = 100_800）。
    #[pallet::type_value]
    pub fn DefaultBillingPeriodBlocks<T: Config>() -> u32 {
        100_800
    }
    #[pallet::storage]
    pub type BillingPeriodBlocks<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultBillingPeriodBlocks<T>>;

    /// 函数级中文注释：宽限期（块）。在余额不足时进入 Grace，超过宽限仍不足则过期。
    /// 
    /// 默认值：5,184,000 块 ≈ 360天（按6秒/块计算）
    /// 计算：360天 × 24小时 × 60分钟 × 10块/分钟 = 5,184,000
    /// 
    /// 设计理由：
    /// - 长宽限期体现平台对用户数据的重视
    /// - 给用户充足时间处理账户问题
    /// - 可通过治理调整（set_billing_params）
    #[pallet::type_value]
    pub fn DefaultGraceBlocks<T: Config>() -> u32 {
        5_184_000 // 360天
    }
    #[pallet::storage]
    pub type GraceBlocks<T: Config> = StorageValue<_, u32, ValueQuery, DefaultGraceBlocks<T>>;

    /// 函数级中文注释：每块处理的最大扣费数，用于限流保护。
    #[pallet::type_value]
    pub fn DefaultMaxChargePerBlock<T: Config>() -> u32 {
        50
    }
    #[pallet::storage]
    pub type MaxChargePerBlock<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultMaxChargePerBlock<T>>;

    /// 函数级中文注释：主体资金账户最低保留（KeepAlive 余量），扣费需确保余额-金额≥该值。
    #[pallet::type_value]
    pub fn DefaultSubjectMinReserve<T: Config>() -> BalanceOf<T> {
        BalanceOf::<T>::default()
    }
    #[pallet::storage]
    pub type SubjectMinReserve<T: Config> =
        StorageValue<_, BalanceOf<T>, ValueQuery, DefaultSubjectMinReserve<T>>;

    /// 函数级中文注释：计费暂停总开关（治理控制）。
    #[pallet::type_value]
    pub fn DefaultBillingPaused<T: Config>() -> bool {
        false
    }
    #[pallet::storage]
    pub type BillingPaused<T: Config> = StorageValue<_, bool, ValueQuery, DefaultBillingPaused<T>>;

    // ⭐ P2优化：已删除 AllowDirectPin 存储（10行）
    // 原因：旧版 request_pin() extrinsic 已删除，此配置无用
    // 删除日期：2025-10-26

    /// 函数级中文注释：到期队列容量上限（每个区块键对应的最大 CID 数）。
    #[pallet::type_value]
    pub fn DefaultDueListCap<T: Config>() -> u32 {
        1024
    }
    #[pallet::storage]
    pub type DueQueue<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        BoundedVec<T::Hash, ConstU32<1024>>,
        ValueQuery,
    >;

    /// 函数级中文注释：入队扩散窗口（块）。将到期项在 `base..base+spread` 内寻找首个未满的队列入队，平滑负载。
    #[pallet::type_value]
    pub fn DefaultDueEnqueueSpread<T: Config>() -> u32 {
        10
    }
    #[pallet::storage]
    pub type DueEnqueueSpread<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultDueEnqueueSpread<T>>;

    /// 函数级中文注释：每个 CID 的计费状态：下一次扣费块高、单价快照、状态（0=Active,1=Grace,2=Expired）。
    #[pallet::storage]
    pub type PinBilling<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Hash, (BlockNumberFor<T>, u128, u8), OptionQuery>;

    /// 函数级中文注释：记录 CID 的 funding 来源（owner, subject_id），用于从派生账户自动扣款。
    #[pallet::storage]
    pub type PinSubjectOf<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Hash, (T::AccountId, u64), OptionQuery>;

    // ============================================================================
    // 新增存储：域索引、分层配置、健康巡检、周期扣费（优化改造）
    // ============================================================================

    /// 函数级详细中文注释：域维度Pin索引，O(1)查找某域下的所有CID
    /// 
    /// 设计目标：
    /// - 替代全局扫描 PendingPins::iter()
    /// - 支持域级别的优先级调度（Subject优先于OTC）
    /// - 便于域级别的批量操作（如暂停某域的扣费）
    /// 
    /// 存储结构：
    /// - Key1: domain（如 b"subject", b"evidence", b"otc"）
    /// - Key2: cid_hash
    /// - Value: ()（标记存在即可）
    /// 
    /// 使用场景：
    /// - OCW巡检时，按域顺序扫描：Evidence → OtcOrder → General...
    /// - 统计各域的Pin数量和存储容量
    /// - 实现域级别的优先级队列
    #[pallet::storage]
    #[pallet::getter(fn domain_pins)]
    pub type DomainPins<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>,  // domain
        Blake2_128Concat,
        T::Hash,                        // cid_hash
        (),                             // 标记存在
        OptionQuery,
    >;

    /// 函数级详细中文注释：域注册表 - 新pallet域自动PIN机制的核心存储
    /// 
    /// 功能：
    /// - 记录已注册的域及其配置
    /// - 支持域的自动发现和管理
    /// - 映射域名到SubjectType
    /// 
    /// 存储结构：
    /// - Key: 域名（如 b"subject-video", b"nft-metadata"）
    /// - Value: DomainConfig（域配置信息）
    #[pallet::storage]
    pub type RegisteredDomains<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>,  // domain name
        types::DomainConfig,            // 域配置
        OptionQuery,
    >;

    /// 函数级详细中文注释：CID到Subject的反向映射，用于扣费时查找资金账户
    /// 
    /// 设计目标：
    /// - 周期扣费时，根据 cid_hash 查找对应的 SubjectFunding 账户
    /// - 支持一个CID属于多个Subject的场景（如共享媒体文件）
    /// 
    /// 存储结构：
    /// - Key: cid_hash
    /// - Value: BoundedVec<SubjectInfo>（主Subject + 可选共享Subject列表）
    /// 
    /// SubjectInfo 包含：
    /// - subject_type: SubjectType (Subject/General/OtcOrder/...)
    /// - subject_id: u64
    /// - funding_share: u8（该Subject承担的费用比例，0-100）
    #[pallet::storage]
    #[pallet::getter(fn cid_to_subject)]
    pub type CidToSubject<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash,                                    // cid_hash
        BoundedVec<SubjectInfo, ConstU32<8>>,      // 最多8个Subject共享
        OptionQuery,
    >;

    /// 函数级详细中文注释：分层Pin策略配置
    /// 
    /// 设计目标：
    /// - 根据内容重要性，配置不同的副本数和巡检周期
    /// - 平衡存储成本和可靠性
    /// - 支持运行时动态调整（治理提案）
    /// 
    /// 存储结构：
    /// - Key: PinTier（Critical/Standard/Temporary）
    /// - Value: TierConfig（副本数、巡检周期、费率系数等）
    /// 
    /// 默认值：
    /// - Critical：5副本，7200块（6小时），1.5x费率
    /// - Standard：3副本，28800块（24小时），1.0x费率
    /// - Temporary：1副本，604800块（7天），0.5x费率
    #[pallet::storage]
    #[pallet::getter(fn pin_tier_config)]
    pub type PinTierConfig<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        PinTier,
        TierConfig,
        ValueQuery,
    >;

    /// 函数级详细中文注释：CID分层映射，记录每个CID的优先级等级
    /// 
    /// 存储结构：
    /// - Key: cid_hash
    /// - Value: PinTier
    /// 
    /// 默认规则：
    /// - 未显式设置：Standard（标准级）
    /// - 业务pallet调用时指定（如 pin_cid_for_subject 传递 tier 参数）
    #[pallet::type_value]
    pub fn DefaultPinTier() -> PinTier {
        PinTier::Standard
    }
    #[pallet::storage]
    #[pallet::getter(fn cid_tier)]
    pub type CidTier<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash,
        PinTier,
        ValueQuery,
        DefaultPinTier,
    >;

    /// 函数级详细中文注释：健康巡检队列，按优先级和到期时间排序
    /// 
    /// 设计目标：
    /// - 替代全局扫描，提供高效的巡检调度
    /// - 支持优先级队列（Critical优先）
    /// - 自动去重和过期清理
    /// 
    /// 存储结构：
    /// - Key1: next_check_block（下次巡检时间）
    /// - Key2: cid_hash
    /// - Value: HealthCheckTask
    /// 
    /// 调度逻辑：
    /// - on_finalize 时，扫描 next_check_block <= current_block 的任务
    /// - 执行巡检后，重新插入队列（next_check_block + interval）
    #[pallet::storage]
    #[pallet::getter(fn health_check_queue)]
    pub type HealthCheckQueue<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,  // next_check_block
        Blake2_128Concat,
        T::Hash,            // cid_hash
        HealthCheckTask<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// 函数级详细中文注释：巡检统计数据，用于链上仪表板展示
    /// 
    /// 存储内容：
    /// - 总Pin数、总存储量
    /// - 健康/降级/危险CID数量
    /// - 上次巡检时间
    /// - 累计修复次数
    #[pallet::storage]
    #[pallet::getter(fn health_check_stats)]
    pub type HealthCheckStats<T: Config> = StorageValue<
        _,
        GlobalHealthStats<BlockNumberFor<T>>,
        ValueQuery,
    >;

    /// 函数级详细中文注释：域级健康统计
    /// 
    /// 记录每个域的Pin数量、存储容量、健康状态等统计信息
    /// 
    /// 存储结构：
    /// - Key: domain（如 b"subject", b"evidence", b"otc"）
    /// - Value: DomainStats
    /// 
    /// 使用场景：
    /// - Dashboard按域展示统计数据
    /// - 域级别的监控告警
    /// - 优先级调度决策参考
    #[pallet::storage]
    #[pallet::getter(fn domain_health_stats)]
    pub type DomainHealthStats<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>,  // domain
        DomainStats,
        OptionQuery,
    >;

    /// 函数级详细中文注释：域优先级配置
    /// 
    /// 定义各域的巡检优先级，数值越小优先级越高
    /// 
    /// 存储结构：
    /// - Key: domain
    /// - Value: priority（0-255，0为最高优先级）
    /// 
    /// 默认优先级：
    /// - evidence: 0（最高优先级）
    /// - otc: 10
    /// - general: 20
    /// - custom: 100
    /// 
    /// 治理可调：通过 Root 权限调整优先级
    #[pallet::storage]
    #[pallet::getter(fn domain_priority)]
    pub type DomainPriority<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<32>>,  // domain
        u8,                             // priority
        ValueQuery,                     // 默认返回255（最低优先级）
    >;

    /// 函数级详细中文注释：周期扣费队列，替代手动 charge_due 调用
    /// 
    /// 设计目标：
    /// - on_finalize 自动扫描到期的扣费任务
    /// - 支持四层回退充电机制
    /// - 自动进入宽限期/Unpin流程
    /// 
    /// 存储结构：
    /// - Key1: due_block（下次扣费时间）
    /// - Key2: cid_hash
    /// - Value: BillingTask
    /// 
    /// 调度逻辑：
    /// - on_finalize 时，批量处理 due_block <= current_block 的任务
    /// - 扣费成功：更新 due_block += billing_period
    /// - 扣费失败：进入宽限期或标记Unpin
    #[pallet::storage]
    #[pallet::getter(fn billing_queue)]
    pub type BillingQueue<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,  // due_block
        Blake2_128Concat,
        T::Hash,            // cid_hash
        BillingTask<BlockNumberFor<T>, BalanceOf<T>>,
        OptionQuery,
    >;

    /// 函数级详细中文注释：运营者奖励账户，累计待提取的奖励
    /// 
    /// 存储结构：
    /// - Key: operator_account
    /// - Value: 累计奖励金额
    /// 
    /// 使用场景：
    /// - 周期扣费成功后，自动累加到运营者奖励
    /// - 运营者调用 operator_claim_rewards 提取
    #[pallet::storage]
    #[pallet::getter(fn operator_rewards)]
    pub type OperatorRewards<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 函数级详细中文注释：运营者Pin健康统计（阶段1：链上基础监控）
    /// 
    /// 记录每个运营者的Pin管理健康状况，用于：
    /// 1. 实时监控运营者服务质量
    /// 2. 计算运营者健康度得分（0-100）
    /// 3. 容量预警与负载均衡
    /// 4. 运营者排行榜与信誉评分
    /// 
    /// 存储结构：
    /// - Key: operator_account
    /// - Value: OperatorPinHealth（total_pins, healthy_pins, failed_pins, last_check, health_score）
    /// 
    /// 更新时机：
    /// - Pin分配时（request_pin_for_subject）：total_pins +1
    /// - Pin成功时（OCW回调）：healthy_pins +1
    /// - Pin失败时（OCW回调）：failed_pins +1
    /// - 健康检查时（on_finalize / OCW）：重新计算health_score
    /// 
    /// 使用场景：
    /// - 运营者Dashboard展示（RPC查询）
    /// - 容量告警（容量使用率超过80%）
    /// - 健康度下降告警（得分下降超过10分）
    /// - 运营者排行榜（按健康度排序）
    #[pallet::storage]
    #[pallet::getter(fn operator_pin_stats)]
    pub type OperatorPinStats<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        OperatorPinHealth<BlockNumberFor<T>>,
        ValueQuery, // 默认值为全0，健康度100分
    >;

    /// 函数级详细中文注释：分层存储策略配置存储
    /// 
    /// 按（数据类型 × Pin层级）存储分层策略配置，用于：
    /// 1. 动态配置不同数据类型的存储策略
    /// 2. 治理提案调整分层参数
    /// 3. 运营者选择时的依据
    /// 
    /// Key: (SubjectType, PinTier)
    /// Value: StorageLayerConfig
    /// 
    /// 示例：
    /// - (SubjectType::Evidence, PinTier::Critical) → {core: 5, community: 0, ...}
    /// - (SubjectType::General, PinTier::Critical) → {core: 3, community: 2, ...}
    /// - (SubjectType::OtcOrder, PinTier::Standard) → {core: 1, community: 2, ...}
    /// 
    /// 默认值：使用 StorageLayerConfig::default()
    #[pallet::storage]
    #[pallet::getter(fn storage_layer_config)]
    pub type StorageLayerConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (SubjectType, PinTier), // (数据类型, Pin层级)
        StorageLayerConfig,
        ValueQuery, // 使用默认配置
    >;

    /// 函数级详细中文注释：CID的分层Pin分配记录存储
    /// 
    /// 记录每个CID被分配到哪些Layer 1和Layer 2运营者，用于：
    /// 1. 审计和追溯
    /// 2. 费用分配（按层级分配收益）
    /// 3. 健康检查（分层验证副本数）
    /// 4. 数据迁移（Layer之间迁移）
    /// 
    /// Key: CID Hash
    /// Value: LayeredPinAssignment<AccountId>
    /// 
    /// 包含信息：
    /// - core_operators: Layer 1运营者列表
    /// - community_operators: Layer 2运营者列表
    /// - external_used: 是否使用Layer 3
    /// - external_network: 外部网络类型
    #[pallet::storage]
    #[pallet::getter(fn layered_pin_assignments)]
    pub type LayeredPinAssignments<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash, // CID Hash
        LayeredPinAssignment<T::AccountId>,
        OptionQuery,
    >;

    // ============================================================================
    // 公共IPFS网络简化管理存储（无隐私约束版本）
    // ============================================================================

    /// 函数级详细中文注释：简化的节点统计信息（用于公共IPFS网络）
    /// 
    /// 记录每个Substrate节点的PIN统计和健康状态，用于智能PIN分配：
    /// - total_pins：该节点当前Pin的CID总数
    /// - capacity_gib：节点存储容量（GB）
    /// - health_score：健康评分（0-100）
    /// - last_check：最后一次健康检查的区块号
    #[pallet::storage]
    #[pallet::getter(fn simple_node_stats)]
    pub type SimpleNodeStatsMap<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        SimpleNodeStats<BlockNumberFor<T>>,
        ValueQuery,
    >;

    /// 函数级详细中文注释：简化的PIN分配记录（公共IPFS网络）
    /// 
    /// 记录每个CID分配到哪些Substrate节点：
    /// - Critical数据：3副本（3个节点）
    /// - Standard数据：2副本（2个节点）
    /// - Temporary数据：1副本（1个节点）
    #[pallet::storage]
    #[pallet::getter(fn simple_pin_assignments)]
    pub type SimplePinAssignments<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash, // CID Hash
        BoundedVec<T::AccountId, ConstU32<8>>, // 最多8个节点
        OptionQuery,
    >;

    /// 函数级详细中文注释：CID注册表（plaintext CID映射到hash）
    /// 
    /// 存储CID的plaintext形式，用于OCW调用本地IPFS API
    #[pallet::storage]
    #[pallet::getter(fn cid_registry)]
    pub type CidRegistry<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::Hash, // CID Hash
        BoundedVec<u8, ConstU32<128>>, // Plaintext CID
        OptionQuery,
    >;

    /// 事件
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 请求已受理（cid_hash, payer, replicas, size, price）
        PinRequested(T::Hash, T::AccountId, u32, u64, T::Balance),
        /// 已提交到 ipfs-cluster（cid_hash）
        PinSubmitted(T::Hash),
        /// 标记已 Pin 成功（cid_hash, replicas）
        PinMarkedPinned(T::Hash, u32),
        /// 标记 Pin 失败（cid_hash, code）
        PinMarkedFailed(T::Hash, u16),
        /// 运营者相关事件
        OperatorJoined(T::AccountId),
        OperatorUpdated(T::AccountId),
        OperatorLeft(T::AccountId),
        OperatorStatusChanged(T::AccountId, u8),
        /// 函数级详细中文注释：运营者暂停（运营者自主操作）✅ P0-1新增
        OperatorPaused { operator: T::AccountId },
        /// 函数级详细中文注释：运营者恢复（运营者自主操作）✅ P0-2新增
        OperatorResumed { operator: T::AccountId },
        /// 函数级详细中文注释：运营者注销进入宽限期（有未完成Pin）✅ P0-3新增
        OperatorUnregistrationPending {
            operator: T::AccountId,
            remaining_pins: u32,
            expires_at: BlockNumberFor<T>,
        },
        /// 函数级详细中文注释：运营者注销完成（保证金已返还）✅ P0-3新增
        OperatorUnregistered { operator: T::AccountId },
        /// 运营者探测结果（ok=true 表示在线且集群识别到该 Peer）
        OperatorProbed(T::AccountId, bool),
        /// 创建了副本分配（cid_hash, count）
        AssignmentCreated(T::Hash, u32),
        /// 状态迁移（cid_hash, state）
        PinStateChanged(T::Hash, u8),
        /// 副本降级与修复（cid_hash, operator）
        ReplicaDegraded(T::Hash, T::AccountId),
        ReplicaRepaired(T::Hash, T::AccountId),
        /// 降级累计达到告警阈值（operator, degraded_count）
        OperatorDegradationAlert(T::AccountId, u32),
        /// 主题账户已充值（subject_id, from, to, amount）- 已弃用
        SubjectFunded(u64, T::AccountId, T::AccountId, BalanceOf<T>),
        /// 用户存储账户已充值（混合方案）
        UserFunded {
            user: T::AccountId,
            funder: T::AccountId,
            funding_account: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 函数级中文注释：已完成一次周期扣费（cid_hash, amount, period_blocks, next_charge_at）。
        PinCharged(T::Hash, BalanceOf<T>, u32, BlockNumberFor<T>),
        /// 函数级中文注释：余额不足进入宽限期（cid_hash）。
        PinGrace(T::Hash),
        /// 函数级中文注释：超出宽限期仍欠费，标记过期（cid_hash）。
        PinExpired(T::Hash),
        /// 函数级中文注释：到期队列出入队统计（block, enqueued, dequeued, remaining）。
        DueQueueStats(BlockNumberFor<T>, u32, u32, u32),
        /// 函数级中文注释：OCW 巡检上报 Pin 状态汇总（样本数、pinning、pinned、missing）。
        PinProbe(u32, u32, u32, u32),
        /// 函数级中文注释：从 IPFS 池扣款成功（subject_id, amount, remaining_quota）
        ChargedFromIpfsPool {
            subject_id: u64,
            amount: BalanceOf<T>,
            remaining_quota: BalanceOf<T>,
        },
        /// 函数级中文注释：从 SubjectFunding 账户扣款成功（subject_id, amount）
        ChargedFromSubjectFunding {
            subject_id: u64,
            amount: BalanceOf<T>,
        },
        /// 函数级中文注释：配额已重置（subject_id, new_reset_block）
        QuotaReset {
            subject_id: u64,
            new_reset_block: BlockNumberFor<T>,
        },
        /// 函数级中文注释：IPFS 池余额不足警告（current_balance, threshold）
        IpfsPoolLowBalance {
            current_balance: BalanceOf<T>,
            threshold: BalanceOf<T>,
        },
        /// 函数级中文注释：从调用者账户扣款成功（fallback，自费模式）
        ChargedFromCaller {
            caller: T::AccountId,
            subject_id: u64,
            amount: BalanceOf<T>,
        },
        /// 函数级中文注释：运营者获得奖励分配（运营者账户、金额、权重、总权重）
        OperatorRewarded {
            operator: T::AccountId,
            amount: BalanceOf<T>,
            weight: u128,
            total_weight: u128,
        },
        /// 函数级中文注释：完成一轮奖励分配（总金额、运营者数量、平均权重）
        RewardDistributed {
            total_amount: BalanceOf<T>,
            operator_count: u32,
            average_weight: u128,
        },
        
        // ============================================================================
        // 新增事件：分层配置、健康巡检、自动化扣费（优化改造）
        // ============================================================================
        
        /// 函数级详细中文注释：分层配置已更新（治理调整）
        TierConfigUpdated {
            tier: PinTier,
            config: TierConfig,
        },
        
        /// 函数级详细中文注释：健康巡检结果（CID健康状态变化）
        HealthCheckCompleted {
            cid_hash: T::Hash,
            status: HealthStatus,
            next_check: BlockNumberFor<T>,
        },
        
        /// 函数级详细中文注释：健康状态降级（副本数不足）
        HealthDegraded {
            cid_hash: T::Hash,
            current_replicas: u32,
            target: u32,
        },
        
        /// 函数级详细中文注释：健康状态危险（副本数 < 2）
        HealthCritical {
            cid_hash: T::Hash,
            current_replicas: u32,
        },
        
        /// 函数级详细中文注释：健康巡检失败（网络错误等）
        HealthCheckFailed {
            cid_hash: T::Hash,
            failures: u8,
        },
        
        /// 函数级详细中文注释：自动修复触发（副本数降级）
        AutoRepairTriggered {
            cid_hash: T::Hash,
            current_replicas: u32,
            target: u32,
        },
        
        /// 函数级详细中文注释：自动修复完成
        AutoRepairCompleted {
            cid_hash: T::Hash,
            new_replicas: u32,
        },
        
        /// 函数级详细中文注释：宽限期开始（费用不足）
        GracePeriodStarted {
            cid_hash: T::Hash,
            expires_at: BlockNumberFor<T>,
        },
        
        /// 函数级详细中文注释：宽限期过期，标记Unpin
        GracePeriodExpired {
            cid_hash: T::Hash,
        },
        
        /// 函数级详细中文注释：CID已标记为待Unpin
        MarkedForUnpin {
            cid_hash: T::Hash,
            reason: UnpinReason,
        },
        
        /// 函数级详细中文注释：CID已从IPFS物理删除（OCW执行unpin成功）
        /// 
        /// 触发时机：
        /// - OCW扫描到过期CID（PinBilling.state=2）
        /// - 调用submit_delete_pin成功
        /// - 清理所有链上存储完成
        PinRemoved {
            cid_hash: T::Hash,
            reason: UnpinReason,
        },
        
        /// 函数级详细中文注释：IPFS公共池余额不足警告（需要补充）
        IpfsPoolLowBalanceWarning {
            current: BalanceOf<T>,
        },
        
        /// 函数级详细中文注释：运营者提取奖励
        RewardsClaimed {
            operator: T::AccountId,
            amount: BalanceOf<T>,
        },
        
        /// 函数级详细中文注释：扣费已暂停（紧急开关）
        BillingPausedByGovernance {
            by: T::AccountId,
        },
        
        /// 函数级详细中文注释：扣费已恢复
        BillingResumedByGovernance {
            by: T::AccountId,
        },
        
        // ============================================================================
        // 运营者监控相关Events（阶段1：链上基础监控）
        // ============================================================================
        
        /// 函数级详细中文注释：运营者容量告警（使用率超过80%）
        /// 
        /// 触发时机：
        /// - Pin分配时检查容量
        /// - 健康检查时定期检查
        /// 
        /// 使用场景：
        /// - 运营者Dashboard展示告警
        /// - 提醒运营者扩容
        /// - Pin分配算法避开高负载运营者
        OperatorCapacityWarning {
            operator: T::AccountId,
            used_capacity_gib: u32,
            total_capacity_gib: u32,
            usage_percent: u8,
        },
        
        /// 函数级详细中文注释：运营者健康度下降（得分下降超过10分）
        /// 
        /// 触发时机：
        /// - Pin失败时
        /// - 健康检查发现副本数不足时
        /// 
        /// 使用场景：
        /// - 运营者Dashboard展示告警
        /// - 评估运营者服务质量
        /// - 治理决策参考（淘汰低质量运营者）
        OperatorHealthDegraded {
            operator: T::AccountId,
            old_score: u8,
            new_score: u8,
            total_pins: u32,
            failed_pins: u32,
        },
        
        /// 函数级详细中文注释：Pin已分配给运营者
        /// 
        /// 触发时机：
        /// - request_pin_for_subject 成功分配Pin给运营者
        /// 
        /// 使用场景：
        /// - 运营者Dashboard展示新任务
        /// - 追踪Pin分配历史
        /// - 负载均衡分析
        PinAssignedToOperator {
            operator: T::AccountId,
            cid_hash: T::Hash,
            current_pins: u32,
            capacity_usage_percent: u8,
        },
        
        /// 函数级详细中文注释：运营者Pin成功
        /// 
        /// 触发时机：
        /// - OCW确认Pin成功且副本数达标
        /// 
        /// 使用场景：
        /// - 运营者Dashboard展示成功率
        /// - 计算运营者信誉评分
        OperatorPinSuccess {
            operator: T::AccountId,
            cid_hash: T::Hash,
            replicas_confirmed: u32,
        },
        
        /// 函数级详细中文注释：运营者Pin失败
        /// 
        /// 触发时机：
        /// - OCW检测到Pin失败
        /// - IPFS Cluster API返回错误
        /// 
        /// 使用场景：
        /// - 运营者Dashboard展示失败原因
        /// - 自动触发重新分配
        /// - 降低运营者健康度得分
        OperatorPinFailed {
            operator: T::AccountId,
            cid_hash: T::Hash,
            reason: BoundedVec<u8, ConstU32<128>>,
        },

        /// 函数级详细中文注释：核心运营者不足告警
        /// 
        /// 触发时机：
        /// - select_operators_by_layer 时Layer 1运营者数量不足
        /// 
        /// 告警级别：高（可能影响服务）
        /// 
        /// 建议措施：
        /// - 立即增加Layer 1运营者
        /// - 或调整存储策略配置
        CoreOperatorShortage {
            required: u32,
            available: u32,
        },

        /// 函数级详细中文注释：社区运营者不足告警
        /// 
        /// 触发时机：
        /// - select_operators_by_layer 时Layer 2运营者数量不足
        /// 
        /// 告警级别：中（不影响核心服务）
        /// 
        /// 建议措施：
        /// - 增加社区运营者激励
        /// - 调整Layer 2副本配置
        CommunityOperatorShortage {
            required: u32,
            available: u32,
        },

        /// 函数级详细中文注释：分层Pin分配完成
        /// 
        /// 触发时机：
        /// - request_pin_for_subject 成功分配Pin
        /// 
        /// 包含信息：
        /// - Layer 1运营者列表
        /// - Layer 2运营者列表
        /// - 是否使用Layer 3
        LayeredPinAssigned {
            cid_hash: T::Hash,
            core_operators: BoundedVec<T::AccountId, ConstU32<8>>,
            community_operators: BoundedVec<T::AccountId, ConstU32<8>>,
            external_used: bool,
        },

        /// 函数级详细中文注释：分层策略配置更新
        /// 
        /// 触发时机：
        /// - 治理调用 set_storage_layer_config
        /// 
        /// 使用场景：
        /// - 审计配置变更历史
        /// - 前端Dashboard展示
        StorageLayerConfigUpdated {
            subject_type: SubjectType,
            tier: PinTier,
            core_replicas: u32,
            community_replicas: u32,
        },

        /// 函数级详细中文注释：运营者层级更新
        /// 
        /// 触发时机：
        /// - 治理调用 set_operator_layer
        /// 
        /// 使用场景：
        /// - 审计运营者层级变更
        /// - 追踪Layer 1/2迁移
        OperatorLayerUpdated {
            operator: T::AccountId,
            layer: OperatorLayer,
            priority: u8,
        },

        // ============================================================================
        // 公共IPFS网络简化事件（无隐私约束版本）
        // ============================================================================

        /// 函数级详细中文注释：简化PIN分配完成事件
        /// 
        /// 当CID成功分配到指定节点时触发，记录：
        /// - cid_hash：CID的hash值
        /// - tier：PIN层级（Critical/Standard/Temporary）
        /// - nodes：分配到的节点列表
        /// - replicas：实际分配的副本数
        SimplePinAllocated {
            cid_hash: T::Hash,
            tier: PinTier,
            nodes: BoundedVec<T::AccountId, ConstU32<8>>,
            replicas: u32,
        },

        /// 函数级详细中文注释：PIN状态报告事件（OCW上报）
        /// 
        /// OCW健康检查后上报PIN状态：
        /// - cid_hash：CID的hash值
        /// - node：上报的节点
        /// - status：PIN状态（Pinned/Failed/Restored）
        SimplePinStatusReported {
            cid_hash: T::Hash,
            node: T::AccountId,
            status: SimplePinStatus,
        },

        /// 函数级详细中文注释：节点负载警告事件
        /// 
        /// 当节点容量使用率超过阈值时触发：
        /// - node：节点账户
        /// - capacity_usage：容量使用率（0-100）
        /// - current_pins：当前PIN数量
        SimpleNodeLoadWarning {
            node: T::AccountId,
            capacity_usage: u8,
            current_pins: u32,
        },
        
        // ============================================================================
        // 新pallet域自动PIN机制相关Events
        // ============================================================================
        
        /// 函数级详细中文注释：域已注册
        /// 
        /// 触发时机：
        /// - 新业务pallet首次调用register_content时自动注册
        /// - 治理手动注册域时
        /// 
        /// 使用场景：
        /// - 追踪系统中的所有业务域
        /// - 域管理Dashboard展示
        DomainRegistered {
            domain: BoundedVec<u8, ConstU32<32>>,
            subject_type_id: u8,
        },
        
        /// 函数级详细中文注释：内容已通过域注册
        /// 
        /// 触发时机：
        /// - 业务pallet调用ContentRegistry::register_content成功
        /// 
        /// 使用场景：
        /// - 追踪域级别的内容注册
        /// - 统计各域的存储使用量
        ContentRegisteredViaDomain {
            domain: BoundedVec<u8, ConstU32<32>>,
            subject_id: u64,
            cid_hash: T::Hash,
            tier: PinTier,
        },
        
        /// 函数级详细中文注释：域配置已更新
        /// 
        /// 触发时机：
        /// - 治理更新域配置（启用/禁用自动PIN，修改默认tier等）
        /// 
        /// 使用场景：
        /// - 域管理日志
        DomainConfigUpdated {
            domain: BoundedVec<u8, ConstU32<32>>,
            auto_pin_enabled: bool,
        },
        
        /// 函数级详细中文注释：域统计已更新
        /// 
        /// OCW按域扫描统计后触发，包含该域的完整统计信息
        /// 
        /// 字段说明：
        /// - domain：域名
        /// - total_pins：该域的总Pin数量
        /// - total_size_bytes：该域的总存储容量（字节）
        /// - healthy_count：健康CID数量
        /// - degraded_count：降级CID数量
        /// - critical_count：危险CID数量
        /// 
        /// 使用场景：
        /// - Dashboard实时更新域级统计
        /// - 监控系统告警
        /// - 统计报表生成
        DomainStatsUpdated {
            domain: BoundedVec<u8, ConstU32<32>>,
            total_pins: u64,
            total_size_bytes: u64,
            healthy_count: u64,
            degraded_count: u64,
            critical_count: u64,
        },
        
        /// 函数级详细中文注释：域优先级已设置
        /// 
        /// 治理调用 set_domain_priority 后触发
        /// 
        /// 字段说明：
        /// - domain：域名
        /// - priority：新的优先级（0-255，0为最高）
        /// 
        /// 使用场景：
        /// - 治理日志
        /// - 优先级调整追踪
        DomainPrioritySet {
            domain: BoundedVec<u8, ConstU32<32>>,
            priority: u8,
        },
        
    }

    #[pallet::error]
    #[derive(PartialEq)]
    pub enum Error<T> {
        /// 参数非法
        BadParams,
        /// 订单不存在
        OrderNotFound,
        /// 运营者不存在
        OperatorNotFound,
        /// 运营者已存在
        OperatorExists,
        /// 运营者已被禁用
        OperatorBanned,
        /// 函数级详细中文注释：运营者已暂停（无法再次暂停）✅ P0-1新增
        AlreadyPaused,
        /// 函数级详细中文注释：运营者未暂停（无法恢复）✅ P0-2新增
        NotPaused,
        /// 保证金不足
        InsufficientBond,
        /// 容量不足
        InsufficientCapacity,
        /// 无效状态
        BadStatus,
        /// 分配不存在
        AssignmentNotFound,
        /// 仍存在未完成的副本分配，禁止退出
        HasActiveAssignments,
        /// 调用方未被指派到该内容的副本分配中
        OperatorNotAssigned,
        // ⭐ P2优化：已删除 DirectPinDisabled 错误（旧版 request_pin() 已删除）
        /// 函数级中文注释：两个账户余额都不足（IPFS池和SubjectFunding都无法支付）
        BothAccountsInsufficientBalance,
        /// 函数级中文注释：IPFS 池余额不足
        IpfsPoolInsufficientBalance,
        /// 函数级中文注释：SubjectFunding 账户余额不足
        SubjectFundingInsufficientBalance,
        /// 函数级中文注释：CID已经被pin，禁止重复pin
        CidAlreadyPinned,
        /// 函数级中文注释：三个账户余额都不足（IpfsPool、SubjectFunding、Caller都无法支付）
        AllThreeAccountsInsufficientBalance,
        /// 函数级中文注释：没有活跃的运营者（无法进行奖励分配）
        NoActiveOperators,
        /// 函数级中文注释：运营者托管账户余额不足
        InsufficientEscrowBalance,
        /// 函数级中文注释：计算权重时发生溢出
        WeightOverflow,
        
        // ============================================================================
        // 新增错误：分层配置、健康巡检、自动化扣费（优化改造）
        // ============================================================================
        
        /// 函数级详细中文注释：域名太长（超过32字节）
        DomainTooLong,
        /// 函数级详细中文注释：Subject未找到（CID无归属）
        SubjectNotFound,
        /// 函数级详细中文注释：非所有者（无权限操作）
        NotOwner,
        /// 函数级详细中文注释：已经Pin过（避免重复）
        AlreadyPinned,
        /// 函数级详细中文注释：副本数无效（必须1-10）
        InvalidReplicas,
        /// 函数级详细中文注释：巡检间隔太短（必须≥600块，约30分钟）
        IntervalTooShort,
        /// 函数级详细中文注释：费率系数无效（必须0.1x-10x）
        InvalidMultiplier,
        /// 函数级详细中文注释：宽限期已过（无法再扣费）
        GraceExpired,
        /// 函数级详细中文注释：没有分配运营者（Pin未成功）
        NoOperatorsAssigned,
        /// 函数级详细中文注释：没有可用奖励（余额为零）
        NoRewardsAvailable,
        /// 函数级详细中文注释：分层配置未找到
        TierConfigNotFound,
        /// 函数级详细中文注释：健康巡检任务未找到
        HealthCheckTaskNotFound,
        /// 函数级详细中文注释：扣费任务未找到
        BillingTaskNotFound,
        /// 函数级详细中文注释：可用运营者不足（P0-1新增）
        /// 
        /// 触发场景：
        /// - 活跃运营者数量 < 需要的副本数
        /// - 容量充足的运营者数量 < 需要的副本数
        /// 
        /// 解决方案：
        /// - 增加运营者注册
        /// - 运营者扩容
        /// - 降低副本数需求
        NotEnoughOperators,
        
        // ============================================================================
        // 公共IPFS网络简化错误（无隐私约束版本）
        // ============================================================================
        
        /// 函数级详细中文注释：没有可用的IPFS运营者
        /// 
        /// 触发场景：
        /// - 所有运营者都是Suspended或Banned状态
        /// - 没有注册任何运营者
        NoAvailableOperators,
        
        /// 函数级详细中文注释：节点数量不足
        /// 
        /// 触发场景：
        /// - 活跃节点数量 < 需要的副本数
        /// - 容量充足的节点 < 需要的副本数
        InsufficientNodes,
        
        /// 函数级详细中文注释：节点数量过多
        /// 
        /// 触发场景：
        /// - 尝试分配超过BoundedVec限制的节点数
        TooManyNodes,
        
        // ============================================================================
        // 新pallet域自动PIN机制相关Errors
        // ============================================================================
        
        /// 函数级详细中文注释：无效的域名（长度超过限制或包含非法字符）
        InvalidDomain,
        /// 函数级详细中文注释：域的自动PIN已禁用
        DomainPinDisabled,
        /// 函数级详细中文注释：域不存在
        DomainNotFound,
        /// 函数级详细中文注释：域已存在（尝试重复注册）
        DomainAlreadyExists,
        
    }

    impl<T: Config> Pallet<T> {
        // ⭐ P0优化：已删除 select_operators_by_weight() 函数（82行）
        // 原因：所有引用已迁移到 select_operators_by_layer()
        // 迁移完成位置：
        // - offchain_worker 初始分配 (行4176)
        // - offchain_worker 补充副本 (行4382)
        // 删除日期：2025-10-26
        
        /// 函数级详细中文注释：根据 (domain, subject_id) 计算派生子账户（稳定派生，与创建者/拥有者解耦）
        /// - 使用 `SubjectPalletId.into_sub_account_truncating((domain:u8, subject_id:u64))` 派生稳定地址
        /// - 该账户无私钥，不可外发，仅用于托管与扣费
        /// 
        /// ⚠️ 已弃用：请使用 derive_user_funding_account() 替代
        /// 保留此函数仅为向后兼容
        #[inline]
        #[deprecated(note = "请使用 derive_user_funding_account() 替代，混合方案每用户一个账户")]
        pub fn subject_account_for(domain: u8, subject_id: u64) -> T::AccountId {
            T::SubjectPalletId::get().into_sub_account_truncating((domain, subject_id))
        }

        /// 函数级详细中文注释：根据用户账户派生存储资金账户（混合方案）
        /// 
        /// 设计理念：
        /// - 每个用户一个派生账户，替代每个 Subject 一个账户
        /// - 大幅减少派生账户数量（从 N×M 降到 N）
        /// - 用户只需充值一个账户，管理简单
        /// 
        /// 派生公式：
        /// - SubjectPalletId.into_sub_account_truncating(("user", user_account))
        /// 
        /// 特点：
        /// - 确定性：相同用户 → 永远相同地址
        /// - 无私钥：链上逻辑控制
        /// - 可验证：任何人可根据用户地址计算出派生地址
        #[inline]
        pub fn derive_user_funding_account(user: &T::AccountId) -> T::AccountId {
            T::SubjectPalletId::get().into_sub_account_truncating((b"user", user))
        }

        /// 函数级中文注释：将 SubjectType 转换为 domain 编号（用于 SubjectUsage 记账）
        #[inline]
        pub fn subject_type_to_domain(subject_type: &SubjectType) -> u8 {
            match subject_type {
                SubjectType::Evidence => 0,
                SubjectType::OtcOrder => 1,
                SubjectType::Chat => 5,
                SubjectType::Livestream => 6,
                SubjectType::Swap => 7,
                SubjectType::Arbitration => 8,
                SubjectType::UserProfile => 9,
                SubjectType::General => 98,
                SubjectType::Custom(_) => 99,
            }
        }
        /// 函数级详细中文注释：获取推荐副本数（根据重要性等级）
        /// 
        /// 参数：
        /// - `level`: 重要性等级（0-3）
        /// 
        /// 返回：
        /// - 推荐的副本数
        pub fn get_recommended_replicas(level: u8) -> u32 {
            match level {
                0 => ReplicasForLevel0::<T>::get(),
                1 => ReplicasForLevel1::<T>::get(),
                2 => ReplicasForLevel2::<T>::get(),
                3 => ReplicasForLevel3::<T>::get(),
                _ => ReplicasForLevel1::<T>::get(), // 默认返回 Level 1
            }
        }
        
        /// 函数级详细中文注释：CID 解密/映射内部工具函数（非外部可调用）
        /// - 从 offchain local storage 读取 `/memo/ipfs/cid/<hash_hex>` 对应的明文 CID；
        /// - 若不存在，返回占位 `"<redacted>"`，用于上层降级处理。
        #[inline]
        fn resolve_cid(cid_hash: &T::Hash) -> alloc::string::String {
            let mut key = b"/memo/ipfs/cid/".to_vec();
            let hex = hex::encode(cid_hash.as_ref());
            key.extend_from_slice(hex.as_bytes());
            if let Some(bytes) = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, &key) {
                if let Ok(s) = core::str::from_utf8(&bytes) {
                    return s.into();
                }
            }
            "<redacted>".into()
        }

        // ⭐ P1优化：已删除 derive_subject_funding_account() 函数（39行）
        // 原因：所有引用已迁移到 derive_subject_funding_account_v2()
        // 迁移完成位置：fund_subject_account() extrinsic (行2491)
        // 删除日期：2025-10-26
        // 
        // 新函数优势：
        // - 支持多种SubjectType（Subject/General/OtcOrder/OtcOrder/Evidence/Custom）
        // - 统一的派生逻辑
        // - 向后兼容（Subject使用相同的domain）

        // ⭐ P0优化：已删除 dual_charge_storage_fee() 函数（131行）
        // 原因：所有引用已迁移到 four_layer_charge()
        // 迁移完成位置：charge_due() extrinsic (行3270)
        // 删除日期：2025-10-26

        // ⭐ P1优化：已删除 triple_charge_storage_fee() 函数（160行）
        // 原因：所有引用已迁移到 four_layer_charge()
        // 旧版调用位置：old_pin_cid_for_subject()（已同时删除）
        // 删除日期：2025-10-26
        
        // ============================================================================
        // 新增辅助函数：分层配置、健康巡检、自动化扣费（优化改造）
        // ============================================================================
        
        /// 函数级详细中文注释：获取分层配置（带默认值）
        /// 
        /// 如果链上没有配置，返回默认值：
        /// - Critical: 5副本，7200块（6小时），1.5x费率
        /// - Standard: 3副本，28800块（24小时），1.0x费率
        /// - Temporary: 1副本，604800块（7天），0.5x费率
        pub fn get_tier_config(tier: &PinTier) -> Result<TierConfig, Error<T>> {
            let config = PinTierConfig::<T>::get(tier);
            
            // 如果没有配置，使用默认值
            if config.enabled {
                Ok(config)
            } else {
                // 返回默认配置
                Ok(match tier {
                    PinTier::Critical => TierConfig::critical_default(),
                    PinTier::Standard => TierConfig::default(),
                    PinTier::Temporary => TierConfig::temporary_default(),
                })
            }
        }
        
        /// 函数级详细中文注释：根据SubjectType派生资金账户
        /// 
        /// 派生规则：
        /// - Evidence: (domain=0, subject_id)
        /// - OtcOrder: (domain=1, subject_id)
        /// - General: (domain=2, subject_id)
        /// - Custom: (domain=99, subject_id)
        /// 函数级详细中文注释：统计运营者的Pin数量 ✅ P0-3新增
        /// 
        /// ### 功能
        /// - 遍历PinAssignments，统计该运营者被分配的Pin数量
        /// - 用于leave_operator判断是否需要进入宽限期
        /// 
        /// ### 复杂度
        /// - O(n)，n为所有Pin总数
        /// - MVP实现，生产环境建议增加索引优化
        pub fn count_operator_pins(operator: &T::AccountId) -> u32 {
            let mut count = 0u32;
            for (_cid, operators) in PinAssignments::<T>::iter() {
                if operators.iter().any(|o| o == operator) {
                    count = count.saturating_add(1);
                }
            }
            count
        }

        /// 函数级详细中文注释：完成运营者注销（内部函数）✅ P0-3新增
        /// 
        /// ### 功能
        /// - 返还保证金
        /// - 移除运营者记录
        /// - 移除宽限期记录（如有）
        /// - 发送OperatorUnregistered事件
        /// 
        /// ### 调用场景
        /// 1. leave_operator：无Pin时立即调用
        /// 2. on_finalize：宽限期到期且无Pin时调用
        pub fn finalize_operator_unregistration(operator: &T::AccountId) -> DispatchResult {
            // 返还保证金
            let bond = OperatorBond::<T>::take(operator);
            if !bond.is_zero() {
                let _ = <T as Config>::Currency::unreserve(operator, bond);
            }

            // 移除运营者记录
            Operators::<T>::remove(operator);
            
            // 移除宽限期记录（如果存在）
            PendingUnregistrations::<T>::remove(operator);

            // 发送事件
            Self::deposit_event(Event::OperatorUnregistered {
                operator: operator.clone(),
            });

            Ok(())
        }

        // ============================================================================
        // 运营者监控相关辅助函数（阶段1：链上基础监控）
        // ============================================================================

        /// 函数级详细中文注释：更新运营者Pin统计（核心监控函数）
        /// 
        /// ### 功能
        /// - 更新运营者的Pin统计数据（total_pins, healthy_pins, failed_pins）
        /// - 重新计算健康度得分
        /// - 健康度下降超过10分时自动发送告警Event
        /// 
        /// ### 参数
        /// - `operator`: 运营者账户
        /// - `delta_total`: Pin总数变化（+1分配，-1移除）
        /// - `delta_failed`: 失败Pin数变化（+1失败）
        /// 
        /// ### 调用场景
        /// 1. Pin分配时：delta_total=+1, delta_failed=0
        /// 2. Pin失败时：delta_total=0, delta_failed=+1
        /// 3. Pin移除时：delta_total=-1, delta_failed=0
        /// 
        /// ### 注意
        /// - 健康度得分会在每次调用时重新计算
        /// - 下降超过10分会触发OperatorHealthDegraded事件
        pub fn update_operator_pin_stats(
            operator: &T::AccountId,
            delta_total: i32,
            delta_failed: i32,
        ) -> DispatchResult {
            OperatorPinStats::<T>::try_mutate(operator, |stats| -> DispatchResult {
                // 更新Pin总数
                if delta_total > 0 {
                    stats.total_pins = stats.total_pins.saturating_add(delta_total as u32);
                } else if delta_total < 0 {
                    stats.total_pins = stats.total_pins.saturating_sub((-delta_total) as u32);
                }
                
                // 更新失败数
                if delta_failed > 0 {
                    stats.failed_pins = stats.failed_pins.saturating_add(delta_failed as u32);
                }
                
                // 重新计算健康度得分
                let old_score = stats.health_score;
                stats.health_score = Self::calculate_health_score(operator);
                stats.last_check = <frame_system::Pallet<T>>::block_number();
                
                // 如果健康度下降超过10分，发射Event
                if old_score.saturating_sub(stats.health_score) >= 10 {
                    Self::deposit_event(Event::OperatorHealthDegraded {
                        operator: operator.clone(),
                        old_score,
                        new_score: stats.health_score,
                        total_pins: stats.total_pins,
                        failed_pins: stats.failed_pins,
                    });
                }
                
                Ok(())
            })
        }
        
        /// 函数级详细中文注释：计算运营者健康度得分（智能评分算法）
        /// 
        /// ### 评分算法
        /// 1. 基础分：60分
        /// 2. 健康Pin比例奖励：(healthy_pins / total_pins) * 40，最多+40分
        /// 3. 失败率惩罚：(failed_pins / total_pins) * 100 * 2，每1%失败率扣2分，最多扣60分
        /// 4. 最终得分：max(0, min(100, 60 + 健康奖励 - 失败惩罚))
        /// 
        /// ### 示例
        /// - 无Pin：100分（初始满分）
        /// - 100个Pin，100个健康，0个失败：100分（60 + 40 - 0）
        /// - 100个Pin，90个健康，10个失败：78分（60 + 36 - 20）
        /// - 100个Pin，50个健康，50个失败：0分（60 + 20 - 100，取0）
        /// 
        /// ### 参数
        /// - `operator`: 运营者账户
        /// 
        /// ### 返回
        /// - u8: 健康度得分（0-100）
        pub fn calculate_health_score(operator: &T::AccountId) -> u8 {
            let stats = OperatorPinStats::<T>::get(operator);
            
            if stats.total_pins == 0 {
                return 100; // 无Pin时默认满分
            }
            
            // 失败率惩罚：每1%失败率扣2分
            let failure_rate = stats.failed_pins.saturating_mul(100) / stats.total_pins;
            let failure_penalty = failure_rate.saturating_mul(2).min(60); // 最多扣60分
            
            // 健康Pin比例奖励：健康Pin占比越高，得分越高
            let health_ratio = stats.healthy_pins.saturating_mul(100) / stats.total_pins;
            let health_bonus = health_ratio.min(40); // 最多加40分
            
            // 基础分60 + 健康奖励 - 失败惩罚
            60u8.saturating_add(health_bonus as u8)
                .saturating_sub(failure_penalty as u8)
                .max(0)
                .min(100)
        }
        
        /// 函数级详细中文注释：检查运营者容量并发出告警
        /// 
        /// ### 功能
        /// - 计算运营者容量使用率（0-100）
        /// - 估算每个Pin平均2MB
        /// 
        /// ### 参数
        /// - `operator`: 运营者账户
        /// 
        /// ### 返回
        /// - `u8`: 容量使用率百分比（0-100），如果运营者不存在或容量为0，返回100
        pub fn calculate_capacity_usage(operator: &T::AccountId) -> u8 {
            let Some(info) = Operators::<T>::get(operator) else {
                return 100; // 运营者不存在，视为满载
            };
            
            if info.capacity_gib == 0 {
                return 100; // 容量为0，视为满载
            }
            
            let current_pins = Self::count_operator_pins(operator);
            
            // 估算使用容量（每个Pin平均2MB）
            let avg_size_mb: u64 = 2;
            let used_capacity_gib = (current_pins as u64 * avg_size_mb) / 1024;
            let total_capacity_gib = info.capacity_gib as u64;
            
            ((used_capacity_gib * 100) / total_capacity_gib) as u8
        }

        /// 函数级详细中文注释：检查运营者容量告警并自动发出事件
        /// 
        /// ### 功能
        /// - 估算运营者当前使用的存储容量
        /// - 计算容量使用率
        /// - 使用率超过80%时自动发送OperatorCapacityWarning事件
        /// 
        /// ### 算法
        /// - 假设每个Pin平均大小为2MB
        /// - used_capacity_gib = (current_pins * 2MB) / 1024
        /// - usage_percent = (used_capacity_gib / total_capacity_gib) * 100
        /// 
        /// ### 参数
        /// - `operator`: 运营者账户
        /// 
        /// ### 返回
        /// - `true`: 已发出容量告警
        /// - `false`: 容量正常
        /// 
        /// ### 触发时机
        /// - Pin分配后
        /// - 健康检查时（on_finalize / OCW）
        pub fn check_operator_capacity_warning(operator: &T::AccountId) -> bool {
            let Some(info) = Operators::<T>::get(operator) else {
                return false;
            };
            
            let current_pins = Self::count_operator_pins(operator);
            
            // 估算使用容量（每个Pin平均2MB）
            let avg_size_mb: u64 = 2;
            let used_capacity_gib = (current_pins as u64 * avg_size_mb) / 1024;
            let total_capacity_gib = info.capacity_gib as u64;
            
            if total_capacity_gib == 0 {
                return false;
            }
            
            let usage_percent = ((used_capacity_gib * 100) / total_capacity_gib) as u8;
            
            // 如果使用率超过80%，发出告警
            if usage_percent >= 80 {
                Self::deposit_event(Event::OperatorCapacityWarning {
                    operator: operator.clone(),
                    used_capacity_gib: used_capacity_gib as u32,
                    total_capacity_gib: total_capacity_gib as u32,
                    usage_percent,
                });
                return true;
            }
            
            false
        }
        
        /// 函数级详细中文注释：获取运营者综合指标（供RPC调用）
        /// 
        /// ### 功能
        /// - 聚合运营者的多维度数据（基础信息、Pin统计、容量使用、收益）
        /// - 返回完整的OperatorMetrics结构体
        /// 
        /// ### 返回数据
        /// - status: 运营者状态（0=Active, 1=Suspended）
        /// - capacity_gib: 声明的存储容量
        /// - registered_at: 注册时间
        /// - total_pins: 当前管理的Pin总数
        /// - healthy_pins: 健康Pin数
        /// - failed_pins: 累计失败Pin数
        /// - health_score: 健康度得分（0-100）
        /// - used_capacity_gib: 已使用容量（估算）
        /// - capacity_usage_percent: 容量使用率（0-100）
        /// - pending_rewards: 待领取收益
        /// 
        /// ### 使用场景
        /// - RPC方法 `memoIpfs_getOperatorMetrics`
        /// - 前端运营者Dashboard
        /// - 运营者排行榜
        /// 
        /// ### 参数
        /// - `operator`: 运营者账户
        /// 
        /// ### 返回
        /// - `Some(OperatorMetrics)`: 运营者存在
        /// - `None`: 运营者不存在
        pub fn get_operator_metrics(
            operator: &T::AccountId,
        ) -> Option<OperatorMetrics<BalanceOf<T>, BlockNumberFor<T>>> {
            let info = Operators::<T>::get(operator)?;
            let stats = OperatorPinStats::<T>::get(operator);
            let pending_rewards = OperatorRewards::<T>::get(operator);
            
            let current_pins = Self::count_operator_pins(operator);
            let avg_size_mb: u64 = 2;
            let used_capacity_gib = (current_pins as u64 * avg_size_mb) / 1024;
            let capacity_usage_percent = if info.capacity_gib > 0 {
                ((used_capacity_gib * 100) / (info.capacity_gib as u64)) as u8
            } else {
                0
            };
            
            Some(OperatorMetrics {
                // 基础信息
                status: info.status,
                capacity_gib: info.capacity_gib,
                registered_at: info.registered_at,
                
                // Pin统计
                total_pins: stats.total_pins,
                healthy_pins: stats.healthy_pins,
                failed_pins: stats.failed_pins,
                health_score: stats.health_score,
                
                // 容量使用
                used_capacity_gib: used_capacity_gib as u32,
                capacity_usage_percent,
                
                // 收益
                pending_rewards,
            })
        }

        // ⭐ P1优化：已删除 select_operators_for_pin() 函数（98行）
        // 原因：所有引用已迁移到 select_operators_by_layer()
        // 迁移完成位置：
        // - request_pin_for_subject() extrinsic (已使用select_operators_by_layer)
        // 删除日期：2025-10-26
        // 
        // 新函数优势：
        // - 支持分层选择（Layer 1 Core + Layer 2 Community）
        // - 更智能的评分算法（健康度+容量+优先级）
        // - 详细的审计追溯（LayeredPinAssignments）

        /// 函数级详细中文注释：根据分层策略智能选择运营者（Layer 1/Layer 2）
        /// 
        /// ### 功能
        /// 根据数据类型和Pin优先级，从Layer 1（核心）和Layer 2（社区）运营者池中智能选择，实现：
        /// 1. 核心数据优先分配到Layer 1（项目方运营者）
        /// 2. 社区运营者增强冗余
        /// 3. 自动降级机制（运营者不足时）
        /// 4. 健康度和容量优先排序
        /// 
        /// ### 参数
        /// - `subject_type`: 数据类型（Subject/General/Evidence等）
        /// - `tier`: Pin优先级（Critical/Standard/Temporary）
        /// 
        /// ### 返回
        /// - `Ok(LayeredOperatorSelection)`: 分层运营者列表（Layer 1 + Layer 2）
        /// - `Err(Error::InsufficientOperators)`: 总副本数不足最低要求
        /// 
        /// ### 分层策略
        /// 1. 从 `StorageLayerConfigs` 获取该数据类型的配置
        /// 2. 筛选Layer 1运营者：Active + 容量<80% + 非待注销
        /// 3. 按健康度和优先级排序Layer 1运营者
        /// 4. 选择Top N个Layer 1运营者
        /// 5. 筛选Layer 2运营者（同样条件）
        /// 6. 选择Top M个Layer 2运营者
        /// 7. 检查总副本数是否满足最低要求
        /// 8. 发出告警事件（如果某层不足）
        /// 
        /// ### 降级策略
        /// - Layer 1不足时，发出 `CoreOperatorShortage` 事件（高优先级告警）
        /// - Layer 2不足时，发出 `CommunityOperatorShortage` 事件（中优先级告警）
        /// - 总副本数不足时，返回错误拒绝Pin请求
        /// 
        /// ### 使用场景
        /// - `request_pin_for_subject` 初始Pin分配
        /// - OCW自动修复时重新分配
        pub fn select_operators_by_layer(
            subject_type: SubjectType,
            tier: PinTier,
        ) -> Result<LayeredOperatorSelection<T::AccountId>, Error<T>> {
            // 1. 获取分层策略配置
            let config = StorageLayerConfigs::<T>::get((subject_type.clone(), tier.clone()));
            
            // 2. 收集所有Layer 1（核心）运营者候选
            let mut core_candidates: Vec<(T::AccountId, u8, u8, u8)> = Vec::new(); 
            // (account, health_score, capacity_usage, priority)
            
            for (operator, info) in Operators::<T>::iter() {
                // 筛选条件1：必须是Core层
                if info.layer != OperatorLayer::Core {
                    continue;
                }
                
                // 筛选条件2：只选Active状态
                if info.status != 0 {
                    continue;
                }
                
                // 筛选条件3：不在待注销列表
                if PendingUnregistrations::<T>::contains_key(&operator) {
                    continue;
                }
                
                // 计算容量使用率
                let capacity_usage_percent = Self::calculate_capacity_usage(&operator);
                
                // 筛选条件4：容量使用率 < 80%
                if capacity_usage_percent >= 80 {
                    continue;
                }
                
                // 获取健康度得分
                let health_score = Self::calculate_health_score(&operator);
                
                core_candidates.push((operator, health_score, capacity_usage_percent, info.priority));
            }
            
            // 3. 排序Layer 1运营者：健康度优先、优先级次要、容量第三
            core_candidates.sort_by(|a, b| {
                // 第一排序：健康度降序（高优先）
                match b.1.cmp(&a.1) {
                    core::cmp::Ordering::Equal => {
                        // 第二排序：优先级升序（低值优先）
                        match a.3.cmp(&b.3) {
                            core::cmp::Ordering::Equal => {
                                // 第三排序：容量使用率升序（低优先）
                                a.2.cmp(&b.2)
                            },
                            other => other,
                        }
                    },
                    other => other,
                }
            });
            
            // 4. 选择Top N个Layer 1运营者
            let selected_core: Vec<T::AccountId> = core_candidates
                .into_iter()
                .take(config.core_replicas as usize)
                .map(|(account, _, _, _)| account)
                .collect();
            
            // 5. 如果Layer 1运营者不足，发出告警
            if (selected_core.len() as u32) < config.core_replicas {
                Self::deposit_event(Event::CoreOperatorShortage {
                    required: config.core_replicas,
                    available: selected_core.len() as u32,
                });
            }
            
            // 6. 收集所有Layer 2（社区）运营者候选
            let mut community_candidates: Vec<(T::AccountId, u8, u8)> = Vec::new(); 
            // (account, health_score, capacity_usage)
            
            for (operator, info) in Operators::<T>::iter() {
                // 筛选条件1：必须是Community层
                if info.layer != OperatorLayer::Community {
                    continue;
                }
                
                // 筛选条件2：只选Active状态
                if info.status != 0 {
                    continue;
                }
                
                // 筛选条件3：不在待注销列表
                if PendingUnregistrations::<T>::contains_key(&operator) {
                    continue;
                }
                
                // 计算容量使用率
                let capacity_usage_percent = Self::calculate_capacity_usage(&operator);
                
                // 筛选条件4：容量使用率 < 80%
                if capacity_usage_percent >= 80 {
                    continue;
                }
                
                // 获取健康度得分
                let health_score = Self::calculate_health_score(&operator);
                
                community_candidates.push((operator, health_score, capacity_usage_percent));
            }
            
            // 7. 排序Layer 2运营者：健康度优先、容量次要
            community_candidates.sort_by(|a, b| {
                // 第一排序：健康度降序（高优先）
                match b.1.cmp(&a.1) {
                    core::cmp::Ordering::Equal => {
                        // 第二排序：容量使用率升序（低优先）
                        a.2.cmp(&b.2)
                    },
                    other => other,
                }
            });
            
            // 8. 选择Top M个Layer 2运营者
            let selected_community: Vec<T::AccountId> = community_candidates
                .into_iter()
                .take(config.community_replicas as usize)
                .map(|(account, _, _)| account)
                .collect();
            
            // 9. 如果Layer 2运营者不足，发出告警（但不影响系统运行）
            if (selected_community.len() as u32) < config.community_replicas {
                Self::deposit_event(Event::CommunityOperatorShortage {
                    required: config.community_replicas,
                    available: selected_community.len() as u32,
                });
            }
            
            // 10. 检查总副本数是否满足最低要求
            let total_selected = selected_core.len() + selected_community.len();
            ensure!(
                total_selected >= config.min_total_replicas as usize,
                Error::<T>::NotEnoughOperators
            );
            
            // 11. 转换为BoundedVec并返回
            Ok(LayeredOperatorSelection {
                core_operators: BoundedVec::try_from(selected_core)
                    .map_err(|_| Error::<T>::BadParams)?,
                community_operators: BoundedVec::try_from(selected_community)
                    .map_err(|_| Error::<T>::BadParams)?,
            })
        }

        /// 函数级详细中文注释：派生SubjectFunding账户（v2版本，支持多种类型）
        /// 
        /// 根据SubjectType和subject_id派生唯一的资金账户地址：
        /// - Evidence：domain=0（证据类数据）
        /// - OtcOrder：domain=1（OTC订单）
        /// - Chat：domain=5（聊天消息）
        /// - Livestream：domain=6（直播间）
        /// - Swap：domain=7（Swap兑换）
        /// - Arbitration：domain=8（仲裁证据）
        /// - UserProfile：domain=9（用户档案）
        /// - General：domain=98（通用存储）
        /// - Custom：domain=99（自定义域）
        pub fn derive_subject_funding_account_v2(
            subject_type: SubjectType,
            subject_id: u64,
        ) -> T::AccountId {
            let domain: u8 = match subject_type {
                SubjectType::Evidence => 0,           // 证据类数据
                SubjectType::OtcOrder => 1,           // OTC订单
                SubjectType::Chat => 5,               // 聊天消息
                SubjectType::Livestream => 6,         // 直播间
                SubjectType::Swap => 7,               // Swap兑换
                SubjectType::Arbitration => 8,        // 仲裁证据
                SubjectType::UserProfile => 9,        // 用户档案
                SubjectType::General => 98,           // 通用存储
                SubjectType::Custom(_) => 99,         // 自定义域统一使用99
            };
            
            Self::subject_account_for(domain, subject_id)
        }
        
        /// 函数级详细中文注释：统一四层扣费机制（配额优先）
        /// 
        /// 扣费顺序（所有类型统一）：
        /// 1. 配额优先（从 IpfsPool 扣费，计入月度配额）
        /// 2. SubjectFunding（用户充值账户）
        /// 3. IpfsPool 兜底（公共池补贴）
        /// 4. GracePeriod（宽限期）
        /// 
        /// 返回：
        /// - Ok(ChargeResult::Success)：扣费成功，记录使用的层级
        /// - Ok(ChargeResult::EnterGrace)：进入宽限期
        /// - Err(Error::GraceExpired)：宽限期已过
        pub fn four_layer_charge(
            cid_hash: &T::Hash,
            task: &mut BillingTask<BlockNumberFor<T>, BalanceOf<T>>,
        ) -> Result<ChargeResult<BlockNumberFor<T>>, Error<T>> {
            let amount = task.amount_per_period;
            let current_block = <frame_system::Pallet<T>>::block_number();
            let pool_account = T::IpfsPoolAccount::get();
            
            // 获取Subject信息
            let subjects = CidToSubject::<T>::get(cid_hash)
                .ok_or(Error::<T>::SubjectNotFound)?;
            
            // 获取第一个 subject_id（用于配额追踪）
            let subject_id = subjects.first()
                .map(|s| s.subject_id)
                .unwrap_or(0);
            
            // ===== 第1层：配额优先（从 IpfsPool 扣费，计入配额）=====
            if Self::check_and_use_quota(subject_id, amount, current_block) {
                let pool_balance = T::Currency::free_balance(&pool_account);
                
                if pool_balance >= amount {
                    let _ = T::Currency::withdraw(
                        &pool_account,
                        amount,
                        frame_support::traits::WithdrawReasons::TRANSFER,
                        ExistenceRequirement::KeepAlive,
                    ).map_err(|_| Error::<T>::IpfsPoolInsufficientBalance)?;
                    
                    let _ = Self::distribute_to_pin_operators(cid_hash, amount);
                    TotalChargedFromPool::<T>::mutate(|total| *total = total.saturating_add(amount));
                    
                    // 发送配额使用事件
                    Self::deposit_event(Event::ChargedFromIpfsPool {
                        subject_id: subject_id,
                        amount,
                        remaining_quota: Self::get_remaining_quota(subject_id, current_block),
                    });
                    
                    return Ok(ChargeResult::Success {
                        layer: ChargeLayer::IpfsPool,
                    });
                }
                // IpfsPool 余额不足，回滚配额使用
                Self::rollback_quota_usage(subject_id, amount);
            }
            
            // ===== 第2层：UserFunding（用户级充值账户，混合方案）=====
            // 从 PinSubjectOf 获取 CID 的 owner
            if let Some((owner, _)) = PinSubjectOf::<T>::get(cid_hash) {
                let user_funding_account = Self::derive_user_funding_account(&owner);
                let funding_balance = T::Currency::free_balance(&user_funding_account);
                
                if funding_balance >= amount {
                    T::Currency::transfer(
                        &user_funding_account,
                        &pool_account,
                        amount,
                        ExistenceRequirement::KeepAlive,
                    ).map_err(|_| Error::<T>::SubjectFundingInsufficientBalance)?;
                    
                    let _ = Self::distribute_to_pin_operators(cid_hash, amount);
                    TotalChargedFromSubject::<T>::mutate(|total| *total = total.saturating_add(amount));
                    
                    // 记录 Subject 级费用使用（用于审计）
                    for subject_info in subjects.iter() {
                        let domain = Self::subject_type_to_domain(&subject_info.subject_type);
                        let share_amount = if subject_info.funding_share > 0 {
                            amount.saturating_mul(subject_info.funding_share.into()) / 100u32.into()
                        } else {
                            amount
                        };
                        SubjectUsage::<T>::mutate(
                            (owner.clone(), domain, subject_info.subject_id),
                            |usage| *usage = usage.saturating_add(share_amount)
                        );
                    }
                    
                    Self::deposit_event(Event::ChargedFromSubjectFunding {
                        subject_id: subject_id,
                        amount,
                    });
                    
                    return Ok(ChargeResult::Success {
                        layer: ChargeLayer::SubjectFunding,
                    });
                }
            }
            
            // ===== 第3层：IpfsPool 兜底（公共池补贴）=====
            let pool_balance = T::Currency::free_balance(&pool_account);
            if pool_balance >= amount {
                let _ = T::Currency::withdraw(
                    &pool_account,
                    amount,
                    frame_support::traits::WithdrawReasons::TRANSFER,
                    ExistenceRequirement::KeepAlive,
                ).map_err(|_| Error::<T>::IpfsPoolInsufficientBalance)?;
                
                let _ = Self::distribute_to_pin_operators(cid_hash, amount);
                TotalChargedFromPool::<T>::mutate(|total| *total = total.saturating_add(amount));
                
                Self::deposit_event(Event::IpfsPoolLowBalanceWarning {
                    current: T::Currency::free_balance(&pool_account),
                });
                
                return Ok(ChargeResult::Success {
                    layer: ChargeLayer::IpfsPool,
                });
            }
            
            // ===== 第4层：GracePeriod（宽限期）=====
            match &task.grace_status {
                GraceStatus::Normal => {
                    let tier = CidTier::<T>::get(cid_hash);
                    let tier_config = Self::get_tier_config(&tier).unwrap_or_default();
                    let expires_at = current_block + tier_config.grace_period_blocks.into();
                    Ok(ChargeResult::EnterGrace { expires_at })
                },
                GraceStatus::InGrace { expires_at, .. } => {
                    if current_block > *expires_at {
                        Err(Error::<T>::GraceExpired)
                    } else {
                        Ok(ChargeResult::EnterGrace { expires_at: *expires_at })
                    }
                },
                GraceStatus::Expired => {
                    Err(Error::<T>::GraceExpired)
                },
            }
        }
        
        /// 检查并使用免费配额
        fn check_and_use_quota(
            subject_id: u64,
            amount: BalanceOf<T>,
            current_block: BlockNumberFor<T>,
        ) -> bool {
            let (used, reset_block) = PublicFeeQuotaUsage::<T>::get(subject_id);
            let quota_limit = T::MonthlyPublicFeeQuota::get();
            
            // 检查是否需要重置配额
            let (current_used, new_reset_block) = if current_block >= reset_block {
                let new_reset = current_block + T::QuotaResetPeriod::get();
                (BalanceOf::<T>::zero(), new_reset)
            } else {
                (used, reset_block)
            };
            
            // 检查配额是否充足
            let remaining = quota_limit.saturating_sub(current_used);
            if remaining >= amount {
                let new_used = current_used.saturating_add(amount);
                PublicFeeQuotaUsage::<T>::insert(subject_id, (new_used, new_reset_block));
                true
            } else {
                false
            }
        }
        
        /// 获取剩余配额
        fn get_remaining_quota(
            subject_id: u64,
            current_block: BlockNumberFor<T>,
        ) -> BalanceOf<T> {
            let (used, reset_block) = PublicFeeQuotaUsage::<T>::get(subject_id);
            let quota_limit = T::MonthlyPublicFeeQuota::get();
            
            if current_block >= reset_block {
                quota_limit
            } else {
                quota_limit.saturating_sub(used)
            }
        }
        
        /// 回滚配额使用
        fn rollback_quota_usage(subject_id: u64, amount: BalanceOf<T>) {
            PublicFeeQuotaUsage::<T>::mutate(subject_id, |(used, _)| {
                *used = used.saturating_sub(amount);
            });
        }
        
        /// 函数级详细中文注释：自动分配存储费给运营者
        /// 
        /// 分配逻辑：
        /// 1. 查询哪些运营者存储了该CID（从PinAssignments读取）
        /// 2. 平均分配费用给所有运营者
        /// 3. 累计到运营者奖励账户（OperatorRewards）
        /// 
        /// 参数：
        /// - cid_hash: CID哈希
        /// - total_amount: 总费用
        /// 
        /// 防作弊：
        /// - 运营者必须在PinAssignments中有该CID的记录
        /// - 定期健康巡检验证运营者确实存储了数据
        /// - 虚假报告会被申诉系统惩罚
        pub fn distribute_to_pin_operators(
            cid_hash: &T::Hash,
            total_amount: BalanceOf<T>,
        ) -> DispatchResult {
            // 1. 从PinAssignments获取运营者列表（链上记录）
            let operators = PinAssignments::<T>::get(cid_hash)
                .ok_or(Error::<T>::NoOperatorsAssigned)?;
            
            if operators.is_empty() {
                return Err(Error::<T>::NoOperatorsAssigned.into());
            }
            
            // 2. 计算每个运营者的奖励
            let per_operator = total_amount / (operators.len() as u32).into();
            
            // 3. 累计到运营者奖励账户
            for operator in operators.iter() {
                OperatorRewards::<T>::mutate(operator, |balance| {
                    *balance = balance.saturating_add(per_operator);
                });
                
                // 发送事件（简化版，不包含权重信息）
                Self::deposit_event(Event::OperatorRewarded {
                    operator: operator.clone(),
                    amount: per_operator,
                    weight: 1000, // 平均分配，权重相同
                    total_weight: (operators.len() as u128) * 1000,
                });
            }
            
            // 4. 发送分配完成事件
            Self::deposit_event(Event::RewardDistributed {
                total_amount,
                operator_count: operators.len() as u32,
                average_weight: 1000,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：获取Pin的运营者列表
        /// 
        /// 从PinAssignments存储中读取分配给该CID的运营者列表
        pub fn get_pin_operators(cid_hash: &T::Hash) -> Result<BoundedVec<T::AccountId, ConstU32<16>>, Error<T>> {
            PinAssignments::<T>::get(cid_hash).ok_or(Error::<T>::NoOperatorsAssigned)
        }
        
        /// 函数级详细中文注释：健康巡检（检查Pin状态）
        /// 
        /// 功能：
        /// 1. 调用IPFS Cluster status API查询副本状态
        /// 2. 比对目标副本数与当前副本数
        /// 3. 返回健康状态（Healthy/Degraded/Critical/Unknown）
        /// 
        /// 参数：
        /// - cid_hash: CID哈希
        /// 
        /// 返回：
        /// - HealthStatus枚举值
        /// 
        /// 注意：此函数应在OCW中调用，不应在链上执行
        pub fn check_pin_health(_cid_hash: &T::Hash) -> HealthStatus {
            // TODO: 实际实现需要在OCW中调用IPFS Cluster API
            // 这里返回默认值，后续在on_finalize中实现
            HealthStatus::Unknown
        }
        
        /// 函数级详细中文注释：计算初始Pin费用（一次性预扣30天费用）
        /// 
        /// 参数：
        /// - size_bytes: CID大小（字节）
        /// - replicas: 副本数
        /// 
        /// 返回：
        /// - Ok(Balance): 计算出的初始费用
        /// - Err: 计算失败
        /// 
        /// 计算公式：
        /// 费用 = (size_bytes / 1GiB) × replicas × base_rate × 4周
        pub fn calculate_initial_pin_fee(
            size_bytes: u64,
            replicas: u32,
        ) -> Result<BalanceOf<T>, Error<T>> {
            let gib: u128 = 1_073_741_824u128; // 1 GiB
            let size_u128 = size_bytes as u128;
            let units = (size_u128 + gib - 1) / gib; // ceil
            
            let base_rate = PricePerGiBWeek::<T>::get();
            let weeks_count = 4u128; // 30天 ≈ 4周
            
            let total = units
                .saturating_mul(replicas as u128)
                .saturating_mul(base_rate)
                .saturating_mul(weeks_count);
            
            Ok(total.saturated_into())
        }
        
        /// 函数级详细中文注释：计算周期费用（每个billing_period的费用）
        /// 
        /// 参数：
        /// - size_bytes: CID大小（字节）
        /// - replicas: 副本数
        /// 
        /// 返回：
        /// - Ok(Balance): 计算出的周期费用
        /// - Err: 计算失败
        /// 
        /// 计算公式：
        /// 费用 = (size_bytes / 1GiB) × replicas × base_rate × 1周
        pub fn calculate_period_fee(
            size_bytes: u64,
            replicas: u32,
        ) -> Result<BalanceOf<T>, Error<T>> {
            let gib: u128 = 1_073_741_824u128;
            let size_u128 = size_bytes as u128;
            let units = (size_u128 + gib - 1) / gib;
            
            let base_rate = PricePerGiBWeek::<T>::get();
            
            let total = units
                .saturating_mul(replicas as u128)
                .saturating_mul(base_rate);
            
            Ok(total.saturated_into())
        }
        
        /// 函数级详细中文注释：获取治理账户（辅助函数）
        /// 
        /// 返回一个固定的治理账户地址（用于日志记录）
        pub fn governance_account() -> T::AccountId {
            // 使用SubjectPalletId作为治理账户的默认值
            T::SubjectPalletId::get().into_account_truncating()
        }
    }

    // 说明：临时允许 warnings 以通过工作区 -D warnings；后续将以 WeightInfo 基准权重替换常量权重
    #[allow(warnings)]
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 函数级详细中文注释：为用户存储账户充值（混合方案，推荐）
        /// 
        /// ### 混合方案
        /// - 每个用户一个派生账户，替代每个 Subject 一个账户
        /// - 大幅减少派生账户数量
        /// - 用户只需充值一个账户，管理简单
        /// 
        /// ### 权限
        /// - **任何账户都可以充值**（开放性）
        /// - 无需owner权限
        /// 
        /// ### 资金流向
        /// - caller → UserFunding(target_user)
        /// 
        /// ### 使用场景
        /// - 用户自己充值（常规）
        /// - 家人朋友赞助（情感）
        /// - 社区众筹（公益）
        /// 
        /// ### 参数
        /// - `target_user`: 目标用户账户（可以是自己或他人）
        /// - `amount`: 充值金额（必须>0）
        /// 
        /// ### 事件
        /// - UserFunded(target_user, who, to, amount)
        #[pallet::call_index(21)]
        #[pallet::weight(10_000)]
        pub fn fund_user_account(
            origin: OriginFor<T>,
            target_user: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(amount != BalanceOf::<T>::default(), Error::<T>::BadParams);
            
            // ✅ 派生用户级存储账户
            let to = Self::derive_user_funding_account(&target_user);
            
            // ✅ 转账（任何人都可以充值）
            <T as Config>::Currency::transfer(
                &who,
                &to,
                amount,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;
            
            // ✅ 更新用户充值统计
            UserFundingBalance::<T>::mutate(&target_user, |balance| {
                *balance = balance.saturating_add(amount);
            });
            
            // ✅ 发送事件
            Self::deposit_event(Event::UserFunded {
                user: target_user,
                funder: who,
                funding_account: to,
                amount,
            });
            Ok(())
        }

        /// 函数级详细中文注释：为SubjectFunding账户充值（已弃用，保留向后兼容）
        /// 
        /// ⚠️ 已弃用：请使用 fund_user_account() 替代
        /// 
        /// ### 权限
        /// - **任何账户都可以充值**（开放性）
        /// - 无需owner权限
        /// - 只需要subject存在
        /// 
        /// ### 资金流向
        /// - caller → SubjectFunding(subject_id)
        /// - SubjectFunding地址基于creator派生（稳定地址）
        /// 
        /// ### 使用场景
        /// - owner自己充值（常规）
        /// - 家人朋友赞助（情感）
        /// - 社区众筹（公益）
        /// - 服务商预付费（商业）
        /// - 慈善捐赠（慈善）
        /// 
        /// ### 设计理念
        /// - **开放性**：任何人都可以资助
        /// - **灵活性**：支持多种场景
        /// - **安全性**：资金只能用于IPFS pin
        /// - **简单性**：不需要权限检查
        /// 
        /// ### 参数
        /// - `subject_id`: 主体ID
        /// - `amount`: 充值金额（必须>0）
        /// 
        /// ### 事件
        /// - SubjectFunded(subject_id, who, to, amount)
        #[pallet::call_index(9)]
        #[pallet::weight(10_000)]
        #[deprecated(note = "请使用 fund_user_account() 替代，混合方案每用户一个账户")]
        pub fn fund_subject_account(
            origin: OriginFor<T>,
            subject_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(amount != BalanceOf::<T>::default(), Error::<T>::BadParams);
            
            // ✅ 派生SubjectFunding地址（使用统一的v2版本）
            #[allow(deprecated)]
            let to = Self::derive_subject_funding_account_v2(SubjectType::General, subject_id);
            
            // ✅ 转账（任何人都可以充值）
            <T as Config>::Currency::transfer(
                &who,
                &to,
                amount,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;
            
            // ✅ 发送事件
            Self::deposit_event(Event::SubjectFunded(subject_id, who, to, amount));
            Ok(())
        }
        // ⭐ P2优化：已删除 request_pin() extrinsic（46行）
        // 原因：已被破坏式改造的 request_pin_for_subject 替代
        // 删除日期：2025-10-26
        // 
        // 旧版问题：
        // - 使用cid_hash而非明文CID（不利于IPFS集成）
        // - 直接扣费模式（无四层回退保障）
        // - 不支持分层运营者选择
        // - 不支持自动化计费和宽限期
        // 
        // 新版优势（request_pin_for_subject）：
        // - 使用明文CID（Vec<u8>），便于IPFS API调用
        // - 三层扣费逻辑（IpfsPool → SubjectFunding → Grace）
        // - 分层运营者选择（Layer 1 Core + Layer 2 Community）
        // - 支持PinTier（Critical/Standard/Temporary）
        // - 自动化周期计费和宽限期管理

        /// 函数级详细中文注释：为主体发起 Pin（四层扣款逻辑）
        /// 
        /// 授权：caller 必须为该 subject 的 owner
        /// 
        /// 扣款优先级（四层扣款）：
        /// 1. IpfsPoolAccount（配额内优先，公共福利）
        /// 2. SubjectFunding（主体专属资金，推荐）
        /// 3. IpfsPool 兜底（公共池补贴）
        /// 4. GracePeriod（宽限期）
        /// 
        /// 优点：
        /// - 用户体验最好：一次交易完成
        /// - 新用户友好：无需预充值
        /// - 向后兼容：保留双重扣款逻辑
        /// - 仍鼓励使用 SubjectFunding（第二优先级）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::request_pin())]
        pub fn request_pin_for_subject(
            origin: OriginFor<T>,
            subject_id: u64,
            cid: Vec<u8>,
            tier: Option<PinTier>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            
            // 1. 计算CID哈希
            use sp_runtime::traits::Hash;
            let cid_hash = T::Hashing::hash(&cid[..]);
            
            // 3. 防重复Pin
            ensure!(!PinMeta::<T>::contains_key(&cid_hash), Error::<T>::AlreadyPinned);
            
            // 4. 获取分层配置
            let tier = tier.unwrap_or(PinTier::Standard);
            let tier_config = Self::get_tier_config(&tier)?;
            
            // 5. 估算CID大小（简化处理，实际可从OCW获取）
            let size_bytes = cid.len() as u64 * 1024; // 假设平均1KB/字符
            
            // 6. 计算初始Pin费用（根据tier的fee_multiplier调整）
            let base_fee = Self::calculate_initial_pin_fee(size_bytes, tier_config.replicas)?;
            let adjusted_fee = base_fee.saturating_mul(tier_config.fee_multiplier.into()) / 10000u32.into();
            
            // 7. 执行初始扣费（使用四层回退机制）
            // 创建临时的BillingTask用于扣费
            let current_block = <frame_system::Pallet<T>>::block_number();
            let mut temp_task = BillingTask {
                billing_period: T::DefaultBillingPeriod::get(),
                amount_per_period: adjusted_fee,
                last_charge: current_block,
                grace_status: GraceStatus::Normal,
                charge_layer: ChargeLayer::IpfsPool,
            };
            
            // 先注册CidToSubject（four_layer_charge需要）
            let subject_info = SubjectInfo {
                subject_type: SubjectType::General,
                subject_id,
                funding_share: 100,  // 独占100%费用
            };
            let subject_vec = BoundedVec::try_from(vec![subject_info])
                .map_err(|_| Error::<T>::BadParams)?;
            CidToSubject::<T>::insert(&cid_hash, subject_vec);
            
            // ⭐ 公共IPFS网络简化PIN分配（无隐私约束）
            // 优先使用简化算法：更快、更高效
            let simple_nodes = Self::optimized_pin_allocation(cid_hash, tier.clone(), size_bytes)?;
            
            // 同时保留完整的Layer 1/Layer 2逻辑（向后兼容）
            let selection = Self::select_operators_by_layer(SubjectType::General, tier.clone())?;
            
            // 合并Layer 1和Layer 2运营者为完整列表
            let mut all_operators = selection.core_operators.to_vec();
            all_operators.extend(selection.community_operators.to_vec());
            
            // 更新每个运营者的统计信息（分别处理Layer 1和Layer 2）
            // Layer 1运营者
            for operator in selection.core_operators.iter() {
                Self::update_operator_pin_stats(operator, 1, 0)?;
                Self::check_operator_capacity_warning(operator);
                
                let current_pins = Self::count_operator_pins(operator);
                let capacity_usage_percent = Self::calculate_capacity_usage(operator);
                
                Self::deposit_event(Event::PinAssignedToOperator {
                    operator: operator.clone(),
                    cid_hash,
                    current_pins,
                    capacity_usage_percent,
                });
            }
            
            // Layer 2运营者
            for operator in selection.community_operators.iter() {
                Self::update_operator_pin_stats(operator, 1, 0)?;
                Self::check_operator_capacity_warning(operator);
                
                let current_pins = Self::count_operator_pins(operator);
                let capacity_usage_percent = Self::calculate_capacity_usage(operator);
                
                Self::deposit_event(Event::PinAssignedToOperator {
                    operator: operator.clone(),
                    cid_hash,
                    current_pins,
                    capacity_usage_percent,
                });
            }
            
            // ⭐ 记录分层Pin分配（用于审计和追溯）
            // 注意：使用truncate_from转换不同bound的BoundedVec
            let core_ops_for_storage = BoundedVec::truncate_from(selection.core_operators.to_vec());
            let community_ops_for_storage = BoundedVec::truncate_from(selection.community_operators.to_vec());
            
            LayeredPinAssignments::<T>::insert(
                &cid_hash,
                LayeredPinAssignment {
                    core_operators: core_ops_for_storage.clone(),
                    community_operators: community_ops_for_storage.clone(),
                    external_used: false,  // 暂不支持Layer 3
                    external_network: None,
                },
            );
            
            // 发送分层Pin分配完成事件
            Self::deposit_event(Event::LayeredPinAssigned {
                cid_hash,
                core_operators: core_ops_for_storage,
                community_operators: community_ops_for_storage,
                external_used: false,
            });
            
            // 注册到PinAssignments（向后兼容）
            let operators_bounded = BoundedVec::try_from(all_operators)
                .map_err(|_| Error::<T>::BadParams)?;
            PinAssignments::<T>::insert(&cid_hash, operators_bounded);
            
            // 执行扣费
            match Self::four_layer_charge(&cid_hash, &mut temp_task) {
                Ok(ChargeResult::Success { layer }) => {
                    // 扣费成功
                },
                Ok(ChargeResult::EnterGrace { .. }) => {
                    // 进入宽限期也允许Pin，但发出警告
                    Self::deposit_event(Event::IpfsPoolLowBalanceWarning {
                        current: T::Currency::free_balance(&T::IpfsPoolAccount::get()),
                    });
                },
                Err(e) => return Err(e.into()),
            }
            
            // 8. 注册CID到CidRegistry（用于OCW调用IPFS API）
            let cid_bounded = BoundedVec::try_from(cid.clone())
                .map_err(|_| Error::<T>::BadParams)?;
            CidRegistry::<T>::insert(&cid_hash, cid_bounded);
            
            // 9. 注册到域索引
            let domain = BoundedVec::try_from(b"subject".to_vec())
                .map_err(|_| Error::<T>::DomainTooLong)?;
            DomainPins::<T>::insert(&domain, &cid_hash, ());
            
            // 9. 记录分层等级
            CidTier::<T>::insert(&cid_hash, tier.clone());
            
            // 10. 注册到健康巡检队列
            let next_check = current_block + tier_config.health_check_interval.into();
            let check_task = HealthCheckTask {
                tier: tier.clone(),
                last_check: current_block,
                last_status: HealthStatus::Unknown,  // 初始状态未知
                consecutive_failures: 0,
            };
            HealthCheckQueue::<T>::insert(next_check, &cid_hash, check_task);
            
            // 11. 注册到周期扣费队列
            let period_fee = Self::calculate_period_fee(size_bytes, tier_config.replicas)?;
            let period_fee_adjusted = period_fee.saturating_mul(tier_config.fee_multiplier.into()) / 10000u32.into();
            let billing_period = T::DefaultBillingPeriod::get();
            let next_billing = current_block + billing_period.into();
            let billing_task = BillingTask {
                billing_period,
                amount_per_period: period_fee_adjusted,
                last_charge: current_block,
                grace_status: GraceStatus::Normal,
                charge_layer: ChargeLayer::IpfsPool,
            };
            BillingQueue::<T>::insert(next_billing, &cid_hash, billing_task);
            
            // 12. 存储Pin元信息
            let meta = PinMetadata {
                replicas: tier_config.replicas,
                size: size_bytes,
                created_at: current_block,
                last_activity: current_block,
            };
            PinMeta::<T>::insert(&cid_hash, meta);
            
            // 13. 保留旧的存储项（兼容OCW）
            PendingPins::<T>::insert(&cid_hash, (caller.clone(), tier_config.replicas, subject_id, size_bytes, adjusted_fee));
            PinStateOf::<T>::insert(&cid_hash, 0u8);  // 0=Pending
            PinSubjectOf::<T>::insert(&cid_hash, (caller.clone(), subject_id));
            
            // 14. 发送事件
            Self::deposit_event(Event::PinRequested(
                cid_hash,
                caller,
                tier_config.replicas,
                size_bytes,
                adjusted_fee,
            ));
            
            Ok(())
        }

        /// 函数级详细中文注释：【治理/服务商】处理到期扣费项（limit 条）
        /// - Origin：GovernanceOrigin（可扩展加入白名单服务商 Origin）
        /// - 行为：从到期队列中取出 ≤limit 个，到期的 CID 进行扣费；成功则推进下一次扣费并重新入队；余额不足则进入宽限或过期。
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::charge_due(*limit))]
        pub fn charge_due(origin: OriginFor<T>, limit: u32) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(!BillingPaused::<T>::get(), Error::<T>::BadStatus);
            let now = <frame_system::Pallet<T>>::block_number();
            let mut left = core::cmp::min(limit, MaxChargePerBlock::<T>::get());
            if left == 0 {
                return Ok(());
            }
            // 取出本块到期列表
            let mut list = DueQueue::<T>::take(now);
            let original_len = list.len() as u32;
            while left > 0 {
                let Some(cid) = list.pop() else { break };
                left = left.saturating_sub(1);
                // 读取计费与来源
                if let Some((_, unit_price, state)) = PinBilling::<T>::get(&cid) {
                    if let Some((owner, subject_id)) = PinSubjectOf::<T>::get(&cid) {
                        // 仅处理 Active/Grace，已过期则跳过
                        if state <= 1u8 {
                            // 计算应收：ceil(size/GiB) * replicas * unit_price
                            if let Some(meta) = PinMeta::<T>::get(&cid) {
                                let gib: u128 = 1_073_741_824u128; // 1024^3
                                let sz = meta.size as u128;
                                let replicas = meta.replicas;
                                let units = (sz + gib - 1) / gib; // ceil
                                let due_u128 = units
                                    .saturating_mul(replicas as u128)
                                    .saturating_mul(unit_price);
                                let due_bal: BalanceOf<T> = due_u128.saturated_into();
                                
                                // ⭐ P0优化：使用四层扣费逻辑替代双重扣费
                                // 创建临时的BillingTask用于扣费
                                let mut temp_task = BillingTask {
                                    billing_period: BillingPeriodBlocks::<T>::get().into(),
                                    amount_per_period: due_bal,
                                    last_charge: now,
                                    grace_status: match state {
                                        0 => GraceStatus::Normal,
                                        1 => GraceStatus::InGrace { 
                                            entered_at: now.saturating_sub(GraceBlocks::<T>::get().into()),
                                            expires_at: now
                                        },
                                        _ => GraceStatus::Normal, // 不应该到这里
                                    },
                                    charge_layer: ChargeLayer::IpfsPool,
                                };
                                
                                match Self::four_layer_charge(&cid, &mut temp_task) {
                                    Ok(ChargeResult::Success { layer }) => {
                                        // 扣费成功：推进下一期并重新入队
                                        let period = BillingPeriodBlocks::<T>::get();
                                        let next = now.saturating_add(period.into());
                                        PinBilling::<T>::insert(&cid, (next, unit_price, 0u8));
                                        Self::enqueue_due(cid, next);
                                        Self::deposit_event(Event::PinCharged(
                                            cid, due_bal, period, next,
                                        ));
                                        
                                        // 分配收益给运营者
                                        let _ = Self::distribute_to_pin_operators(&cid, due_bal);
                                    },
                                    Ok(ChargeResult::EnterGrace { expires_at }) => {
                                        // 进入宽限期：更新状态并重新入队
                                        PinBilling::<T>::insert(&cid, (expires_at, unit_price, 1u8));
                                        Self::enqueue_due(cid, expires_at);
                                        Self::deposit_event(Event::PinGrace(cid));
                                    },
                                    Err(_) => {
                                        // 扣费失败（宽限期已过）：标记过期
                                        PinBilling::<T>::insert(&cid, (now, unit_price, 2u8));
                                        Self::deposit_event(Event::PinExpired(cid));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // 剩余未处理的放回队列
            if !list.is_empty() {
                DueQueue::<T>::insert(now, list.clone());
            }
            let remaining = list.len() as u32;
            let dequeued = original_len.saturating_sub(remaining);
            Self::deposit_event(Event::DueQueueStats(now, original_len, dequeued, remaining));
            Ok(())
        }

        /// 函数级详细中文注释：治理设置/暂停计费参数。
        /// - 任何入参为 None 表示保持不变；部分更新。
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::set_billing_params())]
        pub fn set_billing_params(
            origin: OriginFor<T>,
            price_per_gib_week: Option<u128>,
            period_blocks: Option<u32>,
            grace_blocks: Option<u32>,
            max_charge_per_block: Option<u32>,
            subject_min_reserve: Option<BalanceOf<T>>,
            paused: Option<bool>,
            // ⭐ P2优化：已删除 allow_direct_pin 参数（旧版 request_pin() 已删除）
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            // 参数防呆校验：确保关键参数为正，避免导致停摆或无限宽限
            if let Some(v) = price_per_gib_week {
                ensure!(v > 0, Error::<T>::BadParams);
                PricePerGiBWeek::<T>::put(v);
            }
            if let Some(v) = period_blocks {
                ensure!(v > 0, Error::<T>::BadParams);
                BillingPeriodBlocks::<T>::put(v);
            }
            if let Some(v) = grace_blocks {
                ensure!(v > 0, Error::<T>::BadParams);
                GraceBlocks::<T>::put(v);
            }
            if let Some(v) = max_charge_per_block {
                ensure!(v > 0, Error::<T>::BadParams);
                MaxChargePerBlock::<T>::put(v);
            }
            if let Some(v) = subject_min_reserve {
                SubjectMinReserve::<T>::put(v);
            }
            if let Some(v) = paused {
                BillingPaused::<T>::put(v);
            }
            // ⭐ P2优化：已删除 allow_direct_pin 参数处理（旧版 request_pin() 已删除）
            Ok(())
        }

        /// 函数级详细中文注释：设置推荐副本数配置
        /// 
        /// 权限：治理 Origin
        /// 
        /// 参数：
        /// - `level0_replicas`: Level 0（临时文件）的推荐副本数
        /// - `level1_replicas`: Level 1（一般文件）的推荐副本数
        /// - `level2_replicas`: Level 2（重要文件）的推荐副本数
        /// - `level3_replicas`: Level 3（关键文件）的推荐副本数
        /// - `min_threshold`: 最小副本数阈值（触发自动补充）
        /// 
        /// 使用 Option 支持部分更新
        #[pallet::call_index(14)]
        #[pallet::weight(10_000)]
        pub fn set_replicas_config(
            origin: OriginFor<T>,
            level0_replicas: Option<u32>,
            level1_replicas: Option<u32>,
            level2_replicas: Option<u32>,
            level3_replicas: Option<u32>,
            min_threshold: Option<u32>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            if let Some(v) = level0_replicas {
                ensure!(v >= 1 && v <= 10, Error::<T>::BadParams);
                ReplicasForLevel0::<T>::put(v);
            }
            if let Some(v) = level1_replicas {
                ensure!(v >= 1 && v <= 10, Error::<T>::BadParams);
                ReplicasForLevel1::<T>::put(v);
            }
            if let Some(v) = level2_replicas {
                ensure!(v >= 1 && v <= 10, Error::<T>::BadParams);
                ReplicasForLevel2::<T>::put(v);
            }
            if let Some(v) = level3_replicas {
                ensure!(v >= 1 && v <= 10, Error::<T>::BadParams);
                ReplicasForLevel3::<T>::put(v);
            }
            if let Some(v) = min_threshold {
                ensure!(v >= 1 && v <= 10, Error::<T>::BadParams);
                MinReplicasThreshold::<T>::put(v);
            }
            
            Ok(())
        }

        /// 函数级详细中文注释：将 OperatorEscrowAccount 中的资金按权重分配给活跃运营者
        /// 
        /// 权重计算公式：
        /// - weight = pinned_bytes × reliability_factor
        /// - reliability_factor = probe_ok / (probe_ok + probe_fail)
        /// - 如果 probe_ok + probe_fail = 0，则使用默认值 50%
        /// 
        /// 分配规则：
        /// - 仅分配给状态为 Active(0) 的运营者
        /// - 按权重比例分配：运营者收益 = 总金额 × (运营者权重 / 所有运营者权重之和)
        /// - 忽略权重为 0 的运营者（pinned_bytes = 0）
        /// 
        /// 权限：
        /// - 治理 Origin（Root 或技术委员会）
        /// - 建议定期（如每周）执行一次
        /// 
        /// 参数：
        /// - `max_amount`: 本次分配的最大金额（0 表示分配托管账户的全部余额）
        #[pallet::call_index(13)]
        #[pallet::weight(10_000)]
        pub fn distribute_to_operators(
            origin: OriginFor<T>,
            max_amount: BalanceOf<T>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            let escrow_account = T::OperatorEscrowAccount::get();
            
            // 1. 确定要分配的总金额
            let escrow_balance = <T as Config>::Currency::free_balance(&escrow_account);
            let total_amount = if max_amount.is_zero() {
                escrow_balance
            } else {
                max_amount.min(escrow_balance)
            };
            
            ensure!(!total_amount.is_zero(), Error::<T>::InsufficientEscrowBalance);
            
            // 2. 收集所有活跃运营者的权重
            let mut weights: alloc::vec::Vec<(T::AccountId, u128)> = alloc::vec::Vec::new();
            let mut total_weight: u128 = 0;
            
            for (op, sla) in OperatorSla::<T>::iter() {
                if let Some(info) = Operators::<T>::get(&op) {
                    if info.status == 0 {  // Active
                        // 计算可靠性因子（千分比，避免除零）
                        let total_probes = sla.probe_ok.saturating_add(sla.probe_fail);
                        let reliability = if total_probes > 0 {
                            // 计算 probe_ok / total_probes，结果乘以 1000 得到千分比
                            (sla.probe_ok as u128)
                                .saturating_mul(1000)
                                .saturating_div(total_probes as u128)
                        } else {
                            // 默认 50% 可靠性
                            500
                        };
                        
                        // 综合权重 = 存储量 × 可靠性 / 1000
                        let weight = (sla.pinned_bytes as u128)
                            .saturating_mul(reliability)
                            .checked_div(1000)
                            .ok_or(Error::<T>::WeightOverflow)?;
                        
                        // 只记录权重大于 0 的运营者
                        if weight > 0 {
                            weights.push((op.clone(), weight));
                            total_weight = total_weight
                                .checked_add(weight)
                                .ok_or(Error::<T>::WeightOverflow)?;
                        }
                    }
                }
            }
            
            ensure!(!weights.is_empty(), Error::<T>::NoActiveOperators);
            ensure!(total_weight > 0, Error::<T>::NoActiveOperators);
            
            // 3. 按权重比例分配
            let mut distributed_amount = BalanceOf::<T>::zero();
            let operator_count = weights.len() as u32;
            
            for (op, weight) in weights.iter() {
                // 计算该运营者应得的份额
                // share = total_amount × weight / total_weight
                let share_u128 = (total_amount.saturated_into::<u128>())
                    .saturating_mul(*weight)
                    .saturating_div(total_weight);
                
                let share: BalanceOf<T> = share_u128.saturated_into();
                
                if share > BalanceOf::<T>::zero() {
                    // 转账给运营者
                    <T as Config>::Currency::transfer(
                        &escrow_account,
                        op,
                        share,
                        ExistenceRequirement::KeepAlive,
                    )?;
                    
                    distributed_amount = distributed_amount.saturating_add(share);
                    
                    Self::deposit_event(Event::OperatorRewarded {
                        operator: op.clone(),
                        amount: share,
                        weight: *weight,
                        total_weight,
                    });
                }
            }
            
            // 4. 发出汇总事件
            let average_weight = if operator_count > 0 {
                total_weight / (operator_count as u128)
            } else {
                0
            };
            
            Self::deposit_event(Event::RewardDistributed {
                total_amount: distributed_amount,
                operator_count,
                average_weight,
            });
            
            Ok(())
        }

        /// 函数级详细中文注释：OCW 上报标记已 Pin 成功
        /// - 需要节点 keystore 的专用 key 签名；
        /// - 仅更新状态并发出事件（骨架）。
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::mark_pinned())]
        pub fn mark_pinned(
            origin: OriginFor<T>,
            cid_hash: T::Hash,
            replicas: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // 仅允许活跃运营者上报
            let op = Operators::<T>::get(&who).ok_or(Error::<T>::OperatorNotFound)?;
            ensure!(op.status == 0, Error::<T>::OperatorBanned);
            ensure!(
                PendingPins::<T>::contains_key(&cid_hash),
                Error::<T>::OrderNotFound
            );
            // 必须是该 cid 的指派运营者之一
            if let Some(assign) = PinAssignments::<T>::get(&cid_hash) {
                ensure!(
                    assign.iter().any(|a| a == &who),
                    Error::<T>::OperatorNotAssigned
                );
            } else {
                return Err(Error::<T>::AssignmentNotFound.into());
            }
            // 标记该运营者完成
            PinSuccess::<T>::insert(&cid_hash, &who, true);
            // 达到副本数则完成
            if let Some(meta) = PinMeta::<T>::get(&cid_hash) {
                let expect = meta.replicas;
                let mut ok_count: u32 = 0;
                if let Some(ops) = PinAssignments::<T>::get(&cid_hash) {
                    for o in ops.iter() {
                        if PinSuccess::<T>::get(&cid_hash, o) {
                            ok_count = ok_count.saturating_add(1);
                        }
                    }
                }
                if ok_count >= expect {
                    // 清理 pending，设置状态
                    PendingPins::<T>::remove(&cid_hash);
                    PinStateOf::<T>::insert(&cid_hash, 2u8); // Pinned
                    Self::deposit_event(Event::PinStateChanged(cid_hash, 2));
                } else {
                    PinStateOf::<T>::insert(&cid_hash, 1u8); // Pinning
                    Self::deposit_event(Event::PinStateChanged(cid_hash, 1));
                }
            }
            Self::deposit_event(Event::PinMarkedPinned(cid_hash, replicas));
            Ok(())
        }

        /// 函数级详细中文注释：OCW 上报标记 Pin 失败
        /// - 记录错误码，便于外部审计。
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::mark_pin_failed())]
        pub fn mark_pin_failed(
            origin: OriginFor<T>,
            cid_hash: T::Hash,
            code: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let op = Operators::<T>::get(&who).ok_or(Error::<T>::OperatorNotFound)?;
            ensure!(op.status == 0, Error::<T>::OperatorBanned);
            ensure!(
                PendingPins::<T>::contains_key(&cid_hash),
                Error::<T>::OrderNotFound
            );
            if let Some(assign) = PinAssignments::<T>::get(&cid_hash) {
                ensure!(
                    assign.iter().any(|a| a == &who),
                    Error::<T>::OperatorNotAssigned
                );
            } else {
                return Err(Error::<T>::AssignmentNotFound.into());
            }
            // 标记失败并置为 Pinning/Failed
            PinSuccess::<T>::insert(&cid_hash, &who, false);
            PinStateOf::<T>::insert(&cid_hash, 1u8);
            Self::deposit_event(Event::PinStateChanged(cid_hash, 1));
            Self::deposit_event(Event::PinMarkedFailed(cid_hash, code));
            Ok(())
        }

        /// 函数级详细中文注释：申请成为运营者并存入保证金
        /// - 要求容量 >= MinCapacityGiB，保证金 >= MinOperatorBond；
        /// - 保证金使用可保留余额（reserve），离开时解保留。
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn join_operator(
            origin: OriginFor<T>,
            peer_id: BoundedVec<u8, T::MaxPeerIdLen>,
            capacity_gib: u32,
            endpoint_hash: T::Hash,
            cert_fingerprint: Option<T::Hash>,
            bond: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                !Operators::<T>::contains_key(&who),
                Error::<T>::OperatorExists
            );
            ensure!(
                capacity_gib >= T::MinCapacityGiB::get(),
                Error::<T>::InsufficientCapacity
            );
            // 计算最小保证金（100 USDT 等值的 NEX）
            let min_bond = Self::calculate_operator_bond();
            ensure!(
                bond >= min_bond,
                Error::<T>::InsufficientBond
            );
            // 保证金保留
            <T as Config>::Currency::reserve(&who, bond)?;
            OperatorBond::<T>::insert(&who, bond);
            
            // ✅ P1-1：获取当前区块高度作为注册时间
            let current_block = <frame_system::Pallet<T>>::block_number();
            
            let info = OperatorInfo::<T> {
                peer_id,
                capacity_gib,
                endpoint_hash,
                cert_fingerprint,
                status: 0,
                registered_at: current_block,  // ✅ P1-1：记录注册时间
                layer: OperatorLayer::Community,  // ✅ Layer分层：新运营者默认分配到Layer 2（社区）
                priority: 128,  // ✅ 默认优先级：中等（0-255，Community通常51-200）
            };
            Operators::<T>::insert(&who, info);
            Self::deposit_event(Event::OperatorJoined(who));
            Ok(())
        }

        /// 函数级详细中文注释：更新运营者元信息（不影响保证金）
        #[pallet::call_index(4)]
        #[pallet::weight(10_000)]
        pub fn update_operator(
            origin: OriginFor<T>,
            peer_id: Option<BoundedVec<u8, T::MaxPeerIdLen>>,
            capacity_gib: Option<u32>,
            endpoint_hash: Option<T::Hash>,
            cert_fingerprint: Option<Option<T::Hash>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Operators::<T>::try_mutate(&who, |maybe| -> DispatchResult {
                let op = maybe.as_mut().ok_or(Error::<T>::OperatorNotFound)?;
                if let Some(p) = peer_id {
                    op.peer_id = p;
                }
                if let Some(c) = capacity_gib {
                    ensure!(
                        c >= T::MinCapacityGiB::get(),
                        Error::<T>::InsufficientCapacity
                    );
                    op.capacity_gib = c;
                }
                if let Some(h) = endpoint_hash {
                    op.endpoint_hash = h;
                }
                if let Some(cf) = cert_fingerprint {
                    op.cert_fingerprint = cf;
                }
                Ok(())
            })?;
            Self::deposit_event(Event::OperatorUpdated(who));
            Ok(())
        }

        /// 函数级详细中文注释：退出运营者并解保留保证金（需无未完成订单，MVP 略过校验）
        #[pallet::call_index(5)]
        #[pallet::weight(10_000)]
        /// 函数级详细中文注释：运营者注销（永久退出）✅ P0-3优化版
        /// 
        /// ### 功能说明
        /// - 运营者自己调用，永久退出运营者身份
        /// - 如果有未完成的Pin，进入7天宽限期
        /// - 宽限期内OCW自动迁移Pin到其他运营者
        /// - 宽限期结束后，如无Pin则返还保证金并移除记录
        /// - 如果没有Pin，立即退出并返还保证金
        /// 
        /// ### 宽限期机制（✅ P0-3新增）
        /// - 默认7天（100,800块）
        /// - 宽限期内运营者状态标记为Suspended（停止新Pin）
        /// - OCW负责迁移Pin到其他运营者
        /// - 宽限期到期由on_finalize检查并处理
        /// 
        /// ### 流程
        /// 1. 检查是否是运营者
        /// 2. 统计当前Pin数量
        /// 3. 如有Pin → 进入宽限期（7天）
        /// 4. 如无Pin → 立即退出
        /// 
        /// ### 权限
        /// - 签名账户必须是已注册的运营者
        pub fn leave_operator(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // 检查是否是运营者
            ensure!(
                Operators::<T>::contains_key(&who),
                Error::<T>::OperatorNotFound
            );

            // ✅ P0-3：统计运营者的Pin数量
            let assigned_pins = Self::count_operator_pins(&who);
            
            if assigned_pins > 0 {
                // ✅ P0-3：进入宽限期
                let grace_period_blocks = 100_800u32.into();  // 7天（100,800块）
                let current_block = <frame_system::Pallet<T>>::block_number();
                let expires_at = current_block.saturating_add(grace_period_blocks);
                
                // 记录到宽限期队列
                PendingUnregistrations::<T>::insert(&who, expires_at);
                
                // 立即停止新Pin分配（标记为Suspended）
                Operators::<T>::try_mutate(&who, |maybe| -> DispatchResult {
                    let op = maybe.as_mut().ok_or(Error::<T>::OperatorNotFound)?;
                    op.status = 1;  // 1 = Suspended
                    Ok(())
                })?;
                
                // 发送进入宽限期事件
                Self::deposit_event(Event::OperatorUnregistrationPending {
                    operator: who,
                    remaining_pins: assigned_pins,
                    expires_at,
                });
            } else {
                // ✅ 无Pin，立即退出
                Self::finalize_operator_unregistration(&who)?;
            }

            Ok(())
        }

        /// 函数级详细中文注释：治理设置运营者状态（0=Active,1=Suspended,2=Banned）
        #[pallet::call_index(6)]
        #[pallet::weight(10_000)]
        pub fn set_operator_status(
            origin: OriginFor<T>,
            who: T::AccountId,
            status: u8,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Operators::<T>::try_mutate(&who, |maybe| -> DispatchResult {
                let op = maybe.as_mut().ok_or(Error::<T>::OperatorNotFound)?;
                op.status = status;
                Ok(())
            })?;
            Self::deposit_event(Event::OperatorStatusChanged(who, status));
            Ok(())
        }

        /// 函数级详细中文注释：治理更新分层存储策略配置
        /// 
        /// ### 功能说明
        /// - 治理Root调用，动态调整不同数据类型的分层存储策略
        /// - 支持按（数据类型 × Pin层级）细粒度配置
        /// - 配置项包括：Layer 1副本数、Layer 2副本数、是否允许Layer 3、最低副本数
        /// 
        /// ### 参数
        /// - `origin`: 必须是Root
        /// - `subject_type`: 数据类型（Subject/General/Evidence等）
        /// - `tier`: Pin优先级（Critical/Standard/Temporary）
        /// - `config`: 分层配置参数
        /// 
        /// ### 使用场景
        /// - 调整证据数据的高安全策略（仅Layer 1，5副本）
        /// - 调整供奉品的低成本策略（允许Layer 3）
        /// - 应对运营者数量变化，动态调整副本分配
        #[pallet::call_index(19)]
        #[pallet::weight(10_000)]
        pub fn set_storage_layer_config(
            origin: OriginFor<T>,
            subject_type: SubjectType,
            tier: PinTier,
            config: StorageLayerConfig,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            // 验证配置合理性
            ensure!(
                config.min_total_replicas > 0,
                Error::<T>::BadParams
            );
            ensure!(
                config.core_replicas + config.community_replicas >= config.min_total_replicas,
                Error::<T>::BadParams
            );
            
            // 更新配置
            StorageLayerConfigs::<T>::insert((subject_type.clone(), tier.clone()), config.clone());
            
            // 发送事件
            Self::deposit_event(Event::StorageLayerConfigUpdated {
                subject_type,
                tier,
                core_replicas: config.core_replicas,
                community_replicas: config.community_replicas,
            });
            
            Ok(())
        }

        /// 函数级详细中文注释：治理设置运营者层级
        /// 
        /// ### 功能说明
        /// - 治理Root调用，手动调整运营者的层级和优先级
        /// - 支持Layer 1/2之间的迁移
        /// - 用于项目方将自己的节点设置为Layer 1（核心）
        /// 
        /// ### 参数
        /// - `origin`: 必须是Root
        /// - `operator`: 运营者账户
        /// - `layer`: 新的层级（Core/Community）
        /// - `priority`: 优先级（0-255，越小越优先）
        /// 
        /// ### 使用场景
        /// - 项目方初始运营者设置为Layer 1（Core）
        /// - 优秀社区运营者升级到Layer 1
        /// - 降级不活跃的Layer 1运营者到Layer 2
        #[pallet::call_index(20)]
        #[pallet::weight(10_000)]
        pub fn set_operator_layer(
            origin: OriginFor<T>,
            operator: T::AccountId,
            layer: OperatorLayer,
            priority: u8,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            // 检查运营者是否存在
            Operators::<T>::try_mutate(&operator, |info_opt| -> DispatchResult {
                let info = info_opt.as_mut().ok_or(Error::<T>::OperatorNotFound)?;
                
                // 更新层级和优先级
                info.layer = layer.clone();
                info.priority = priority;
                
                // 发送事件
                Self::deposit_event(Event::OperatorLayerUpdated {
                    operator: operator.clone(),
                    layer,
                    priority,
                });
                
                Ok(())
            })
        }

        /// 函数级详细中文注释：运营者自主暂停（临时停止接收新Pin）✅ P0-1实现
        /// 
        /// ### 功能说明
        /// - 运营者自己调用，无需治理介入
        /// - 将status从0(Active)改为1(Suspended)
        /// - 停止分配新Pin，但已有Pin仍需维护
        /// - 保留运营者身份和保证金
        /// - 可随时调用resume_operator()恢复
        /// 
        /// ### 适用场景
        /// - 短期维护（硬件升级、网络故障修复）
        /// - 临时离线（1-7天）
        /// - 容量不足需要扩容
        /// 
        /// ### 权限
        /// - 签名账户必须是已注册的运营者
        /// - 当前状态必须是Active（status=0）
        #[pallet::call_index(22)]
        #[pallet::weight(10_000)]
        pub fn pause_operator(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 检查是否是运营者
            let mut info = Operators::<T>::get(&who)
                .ok_or(Error::<T>::OperatorNotFound)?;

            // 检查是否已暂停
            ensure!(info.status == 0, Error::<T>::AlreadyPaused);

            // 标记为暂停
            info.status = 1;  // 1 = Suspended
            Operators::<T>::insert(&who, info);

            // 发送事件
            Self::deposit_event(Event::OperatorPaused { operator: who });

            Ok(())
        }

        /// 函数级详细中文注释：运营者自主恢复（从暂停状态恢复）✅ P0-2实现
        /// 
        /// ### 功能说明
        /// - 运营者自己调用，无需治理介入
        /// - 将status从1(Suspended)改为0(Active)
        /// - 恢复接收新Pin分配
        /// - 保证金和运营者信息不变
        /// 
        /// ### 适用场景
        /// - 维护完成后恢复服务
        /// - 硬件扩容完成
        /// - 网络问题修复
        /// 
        /// ### 权限
        /// - 签名账户必须是已注册的运营者
        /// - 当前状态必须是Suspended（status=1）
        #[pallet::call_index(23)]
        #[pallet::weight(10_000)]
        pub fn resume_operator(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 检查是否是运营者
            let mut info = Operators::<T>::get(&who)
                .ok_or(Error::<T>::OperatorNotFound)?;

            // 检查是否已暂停
            ensure!(info.status == 1, Error::<T>::NotPaused);

            // 恢复激活
            info.status = 0;  // 0 = Active
            Operators::<T>::insert(&who, info);

            // 发送事件
            Self::deposit_event(Event::OperatorResumed { operator: who });

            Ok(())
        }

        /// 函数级详细中文注释：运营者自证在线（由运行其节点的 OCW 定期上报）
        /// - 探测逻辑在 OCW：若 /peers 含有自身 peer_id → ok=true，否则 false。
        #[pallet::call_index(7)]
        #[pallet::weight(10_000)]
        pub fn report_probe(origin: OriginFor<T>, ok: bool) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let op = Operators::<T>::get(&who).ok_or(Error::<T>::OperatorNotFound)?;
            ensure!(op.status == 0, Error::<T>::BadStatus);
            OperatorSla::<T>::mutate(&who, |s| {
                if ok {
                    s.probe_ok = s.probe_ok.saturating_add(1);
                } else {
                    s.probe_fail = s.probe_fail.saturating_add(1);
                }
                s.last_update = <frame_system::Pallet<T>>::block_number();
            });
            Self::deposit_event(Event::OperatorProbed(who, ok));
            Ok(())
        }

        /// 函数级详细中文注释：治理扣罚运营者的保证金（阶梯惩罚使用）。
        #[pallet::call_index(8)]
        #[pallet::weight(10_000)]
        pub fn slash_operator(
            origin: OriginFor<T>,
            who: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(
                Operators::<T>::contains_key(&who),
                Error::<T>::OperatorNotFound
            );
            let (slashed, _remaining) = <T as Config>::Currency::slash_reserved(&who, amount);
            // 记录剩余 bond（slash_reserved 返回负不平衡，使用 peek 获取相应余额值再进行安全减法）
            let old = OperatorBond::<T>::get(&who);
            let slashed_amount = slashed.peek();
            let new = old.saturating_sub(slashed_amount);
            OperatorBond::<T>::insert(&who, new);
            Ok(())
        }
        
        // ============================================================================
        // 新增治理接口：分层配置、扣费控制、运营者奖励（优化改造）
        // ============================================================================
        
        /// 函数级详细中文注释：治理更新分层配置
        /// 
        /// 功能：
        /// - 允许治理提案动态调整分层配置参数
        /// - 支持调整副本数、巡检周期、费率系数、宽限期
        /// 
        /// 参数：
        /// - tier: 分层等级（Critical/Standard/Temporary）
        /// - config: 新的配置参数
        /// 
        /// 权限：
        /// - 治理Origin（Root或技术委员会）
        /// 
        /// 验证：
        /// - 副本数：1-10
        /// - 巡检间隔：≥600块（约30分钟）
        /// - 费率系数：1000-100000（0.1x-10x）
        #[pallet::call_index(15)]
        #[pallet::weight(10_000)]
        pub fn update_tier_config(
            origin: OriginFor<T>,
            tier: PinTier,
            config: TierConfig,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            // 验证配置合理性
            ensure!(
                config.replicas > 0 && config.replicas <= 10,
                Error::<T>::InvalidReplicas
            );
            ensure!(
                config.health_check_interval >= 600,  // 至少30分钟
                Error::<T>::IntervalTooShort
            );
            ensure!(
                config.fee_multiplier >= 1000 && config.fee_multiplier <= 100000,  // 0.1x ~ 10x
                Error::<T>::InvalidMultiplier
            );
            
            // 更新配置
            PinTierConfig::<T>::insert(&tier, config.clone());
            
            // 发送事件
            Self::deposit_event(Event::TierConfigUpdated { tier, config });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：运营者提取奖励
        /// 
        /// 功能：
        /// - 运营者提取累计的存储费奖励
        /// - 从IpfsPoolAccount转账到运营者账户
        /// - 清零奖励记录
        /// 
        /// 权限：
        /// - 签名账户（运营者本人）
        /// 
        /// 检查：
        /// - 必须有可用奖励（余额 > 0）
        /// - IpfsPoolAccount余额充足
        #[pallet::call_index(16)]
        #[pallet::weight(100_000)]
        pub fn operator_claim_rewards(origin: OriginFor<T>) -> DispatchResult {
            let operator = ensure_signed(origin)?;
            
            // 1. 获取奖励余额
            let reward = OperatorRewards::<T>::get(&operator);
            ensure!(!reward.is_zero(), Error::<T>::NoRewardsAvailable);
            
            // 2. 转账
            let pool_account = T::IpfsPoolAccount::get();
            T::Currency::transfer(
                &pool_account,
                &operator,
                reward,
                ExistenceRequirement::KeepAlive,
            )?;
            
            // 3. 清零奖励
            OperatorRewards::<T>::remove(&operator);
            
            // 4. 发送事件
            Self::deposit_event(Event::RewardsClaimed {
                operator,
                amount: reward,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：紧急暂停自动扣费（应急开关）
        /// 
        /// 使用场景：
        /// - 发现扣费逻辑漏洞，需要暂停保护用户资金
        /// - IPFS集群故障，暂停扣费直到恢复
        /// - 链上治理投票期间，暂停重大变更
        /// 
        /// 功能：
        /// - 设置BillingPaused标志为true
        /// - on_finalize将跳过扣费逻辑
        /// 
        /// 权限：
        /// - 治理Origin（Root或技术委员会）
        #[pallet::call_index(17)]
        #[pallet::weight(30_000)]
        pub fn emergency_pause_billing(origin: OriginFor<T>) -> DispatchResult {
            let who = T::GovernanceOrigin::ensure_origin(origin)?;
            
            BillingPaused::<T>::put(true);
            
            Self::deposit_event(Event::BillingPausedByGovernance {
                by: Self::governance_account(),
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：恢复自动扣费
        /// 
        /// 功能：
        /// - 设置BillingPaused标志为false
        /// - on_finalize恢复扣费逻辑
        /// 
        /// 权限：
        /// - 治理Origin（Root或技术委员会）
        #[pallet::call_index(18)]
        #[pallet::weight(30_000)]
        pub fn resume_billing(origin: OriginFor<T>) -> DispatchResult {
            let who = T::GovernanceOrigin::ensure_origin(origin)?;
            
            BillingPaused::<T>::put(false);
            
            Self::deposit_event(Event::BillingResumedByGovernance {
                by: Self::governance_account(),
            });
            
            Ok(())
        }
        
        // ============================================================================
        // 新pallet域自动PIN机制相关Extrinsics
        // ============================================================================
        
        /// 函数级详细中文注释：治理手动注册域
        /// 
        /// 功能：
        /// - 手动注册新业务域
        /// - 配置域的SubjectType映射
        /// - 设置域的默认Pin等级
        /// 
        /// 权限：
        /// - 治理Origin（Root或技术委员会）
        /// 
        /// 使用场景：
        /// - 预注册域，避免自动创建时的不确定性
        /// - 修改域的默认配置
        /// - 禁用某些域的自动PIN
        #[pallet::call_index(25)]
        #[pallet::weight(50_000)]
        pub fn register_domain(
            origin: OriginFor<T>,
            domain: Vec<u8>,
            subject_type_id: u8,
            default_tier: types::PinTier,
            auto_pin_enabled: bool,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            // 1. 转换域名为BoundedVec
            let bounded_domain: BoundedVec<u8, ConstU32<32>> = domain
                .try_into()
                .map_err(|_| Error::<T>::InvalidDomain)?;
            
            // 2. 检查域是否已存在
            ensure!(
                !RegisteredDomains::<T>::contains_key(&bounded_domain),
                Error::<T>::DomainAlreadyExists
            );
            
            // 3. 创建域配置
            let config = types::DomainConfig {
                auto_pin_enabled,
                default_tier,
                subject_type_id,
                owner_pallet: bounded_domain.clone(),
                created_at: {
                    use sp_runtime::SaturatedConversion;
                    frame_system::Pallet::<T>::block_number().saturated_into()
                },
            };
            
            // 4. 保存配置
            RegisteredDomains::<T>::insert(&bounded_domain, &config);
            
            // 5. 发送事件
            Self::deposit_event(Event::DomainRegistered {
                domain: bounded_domain,
                subject_type_id,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：治理更新域配置
        /// 
        /// 功能：
        /// - 修改域的自动PIN开关
        /// - 修改域的默认等级
        /// - 重新映射SubjectType
        /// 
        /// 权限：
        /// - 治理Origin（Root或技术委员会）
        #[pallet::call_index(26)]
        #[pallet::weight(40_000)]
        pub fn update_domain_config(
            origin: OriginFor<T>,
            domain: Vec<u8>,
            auto_pin_enabled: Option<bool>,
            default_tier: Option<types::PinTier>,
            subject_type_id: Option<u8>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            // 1. 转换域名为BoundedVec
            let bounded_domain: BoundedVec<u8, ConstU32<32>> = domain
                .try_into()
                .map_err(|_| Error::<T>::InvalidDomain)?;
            
            // 2. 更新配置
            RegisteredDomains::<T>::try_mutate(&bounded_domain, |maybe_config| {
                let config = maybe_config.as_mut().ok_or(Error::<T>::DomainNotFound)?;
                
                if let Some(enabled) = auto_pin_enabled {
                    config.auto_pin_enabled = enabled;
                }
                if let Some(tier) = default_tier {
                    config.default_tier = tier;
                }
                if let Some(type_id) = subject_type_id {
                    config.subject_type_id = type_id;
                }
                
                Ok::<(), Error<T>>(())
            })?;
            
            // 3. 发送事件
            Self::deposit_event(Event::DomainConfigUpdated {
                domain: bounded_domain,
                auto_pin_enabled: auto_pin_enabled.unwrap_or(true),
            });
            
            Ok(())
        }

        /// 函数级详细中文注释：用户请求取消固定CID
        /// 
        /// ### 功能
        /// - 用户主动取消固定自己的CID
        /// - 标记CID为待删除状态
        /// - OCW将在后续区块执行物理删除
        /// - 停止后续扣费
        /// 
        /// ### 参数
        /// - `cid`：要取消固定的IPFS CID（明文）
        /// 
        /// ### 权限
        /// - 签名账户必须是CID的所有者（Pin时的caller）
        /// 
        /// ### 行为
        /// 1. 计算CID哈希
        /// 2. 验证调用者是CID所有者
        /// 3. 标记为待删除状态（state=2）
        /// 4. 发送 MarkedForUnpin 事件
        /// 5. OCW后续执行物理删除
        #[pallet::call_index(32)]
        #[pallet::weight(50_000)]
        pub fn request_unpin(
            origin: OriginFor<T>,
            cid: Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            
            use sp_runtime::traits::Hash;
            
            // 1. 计算CID哈希
            let cid_hash = T::Hashing::hash(&cid[..]);
            
            // 2. 检查CID是否存在
            ensure!(PinMeta::<T>::contains_key(&cid_hash), Error::<T>::OrderNotFound);
            
            // 3. 权限验证：检查调用者是否为CID所有者
            let (owner, _subject_id) = PinSubjectOf::<T>::get(&cid_hash)
                .ok_or(Error::<T>::NotOwner)?;
            ensure!(caller == owner, Error::<T>::NotOwner);
            
            // 4. 标记为待删除状态
            let current_block = <frame_system::Pallet<T>>::block_number();
            if let Some((_, unit_price, _)) = PinBilling::<T>::get(&cid_hash) {
                PinBilling::<T>::insert(&cid_hash, (current_block, unit_price, 2u8));
            }
            
            // 5. 发送事件
            Self::deposit_event(Event::MarkedForUnpin {
                cid_hash,
                reason: UnpinReason::ManualRequest,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：设置域优先级
        /// 
        /// ### 功能
        /// - 设置域的巡检优先级
        /// - 优先级范围：0-255（0为最高优先级）
        /// - 影响OCW按域扫描的顺序
        /// 
        /// ### 参数
        /// - `domain`：域名（如 b"evidence", b"otc", b"general"）
        /// - `priority`：优先级（0-255）
        /// 
        /// ### 默认优先级
        /// - evidence: 0（最高）
        /// - otc: 10
        /// - general: 20
        /// - custom: 100
        /// - 其他：255（默认）
        /// 
        /// ### 权限
        /// - Root权限
        /// 
        /// ### 使用场景
        /// - 调整域的巡检优先级
        /// - 确保关键域优先处理
        #[pallet::call_index(27)]
        #[pallet::weight(10_000)]
        pub fn set_domain_priority(
            origin: OriginFor<T>,
            domain: Vec<u8>,
            priority: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            
            // 1. 转换域名为BoundedVec
            let bounded_domain: BoundedVec<u8, ConstU32<32>> = domain
                .try_into()
                .map_err(|_| Error::<T>::InvalidDomain)?;
            
            // 2. 设置优先级
            DomainPriority::<T>::insert(&bounded_domain, priority);
            
            // 3. 发送事件
            Self::deposit_event(Event::DomainPrioritySet {
                domain: bounded_domain,
                priority,
            });
            
            Ok(())
        }
        
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 函数级详细中文注释：Offchain Worker 入口
        /// - 周期性扫描 `PendingPins`，对每个 `cid_hash` 调用 ipfs-cluster API 进行 Pin；
        /// - 成功则提交 `mark_pinned`，失败则提交 `mark_pin_failed`；
        /// - HTTP 令牌与集群端点从本地 offchain storage 读取，避免上链泄露。
        fn offchain_worker(_n: BlockNumberFor<T>) {
            // 读取本地配置（示例键）："/memo/ipfs/cluster_endpoint" 与 "/memo/ipfs/token"
            let endpoint: alloc::string::String = sp_io::offchain::local_storage_get(
                StorageKind::PERSISTENT,
                b"/memo/ipfs/cluster_endpoint",
            )
            .and_then(|v| core::str::from_utf8(&v).ok().map(|s| s.to_string()))
            .unwrap_or_else(|| alloc::string::String::from("http://127.0.0.1:9094"));
            let token: Option<alloc::string::String> =
                sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, b"/memo/ipfs/token")
                    .and_then(|v| core::str::from_utf8(&v).ok().map(|s| s.to_string()));

            // 分配与 Pin：遍历 PendingPins，若无分配则创建；否则尝试 POST /pins 携带 allocations
            if let Some((cid_hash, (_payer, _replicas, _subject_id, _size, _price))) =
                <PendingPins<T>>::iter().next()
            {
                // ⭐ P0优化：使用分层选择算法替代旧版权重选择
                if LayeredPinAssignments::<T>::get(&cid_hash).is_none() {
                    // 获取CID的Tier（默认Standard，ValueQuery自动返回默认值）和SubjectType（默认Custom）
                    let tier = CidTier::<T>::get(&cid_hash);
                    let subject_type = SubjectType::Custom(Default::default());
                    
                    // 使用分层选择算法：Layer 1 + Layer 2
                    match Self::select_operators_by_layer(subject_type, tier.clone()) {
                        Ok(selection) => {
                            // 合并Layer 1和Layer 2运营者
                            let mut all_operators = selection.core_operators.to_vec();
                            all_operators.extend(selection.community_operators.to_vec());
                            
                            // 记录分层Pin分配
                            let core_ops = BoundedVec::truncate_from(selection.core_operators.to_vec());
                            let community_ops = BoundedVec::truncate_from(selection.community_operators.to_vec());
                            
                            LayeredPinAssignments::<T>::insert(
                                &cid_hash,
                                LayeredPinAssignment {
                                    core_operators: core_ops.clone(),
                                    community_operators: community_ops.clone(),
                                    external_used: false,
                                    external_network: None,
                                },
                            );
                            
                            // 向后兼容：同时更新旧版PinAssignments
                            if let Ok(operators_bounded) = BoundedVec::try_from(all_operators) {
                                PinAssignments::<T>::insert(&cid_hash, operators_bounded);
                            }
                            
                            // 发送分层Pin分配完成事件
                            Self::deposit_event(Event::LayeredPinAssigned {
                                cid_hash,
                                core_operators: core_ops,
                                community_operators: community_ops,
                                external_used: false,
                            });
                        },
                        Err(_) => {
                            // 选择失败，跳过（运营者不足等）
                        }
                    }
                }
                // 发起 Pin 请求（MVP 不在 body 中传 allocations，真实集群应携带）
                let _ = Self::submit_pin_request(&endpoint, &token, cid_hash);
                PinStateOf::<T>::insert(&cid_hash, 1u8);
                Self::deposit_event(Event::PinStateChanged(cid_hash, 1));
            }

            // 探测自身是否在线（运营者必须运行集群节点）：读取 /peers 并查找自身 peer_id
            // 探测自身是否在线：简化为本地统计，避免依赖 CreateSignedTransaction
            let _ = Self::http_get_bytes(&endpoint, &token, "/peers");

            // 巡检：针对已 Pinned/Pinning 的对象，GET /pins/{cid} 矫正副本；若缺少则再 Pin；并统计上报
            // 注意：演示中未持有明文 CID，这里仅示意调用；生产需有 CID 解密/映射。
            // 逻辑：遍历 PinStateOf in {1=Pinning,2=Pinned}，若 assignments 存在，检查成功标记数；不足副本则再次发起 submit_pin_request。
            let mut sample: u32 = 0;
            let mut pinning: u32 = 0;
            let mut pinned: u32 = 0;
            let mut missing: u32 = 0;
            for (cid_hash, state) in PinStateOf::<T>::iter() {
                if state == 1u8 || state == 2u8 {
                    sample = sample.saturating_add(1);
                    if let Some(assign) = PinAssignments::<T>::get(&cid_hash) {
                        let expect = PinMeta::<T>::get(&cid_hash)
                            .map(|m| m.replicas)
                            .unwrap_or(assign.len() as u32);
                        let mut ok_count: u32 = 0;
                        for o in assign.iter() {
                            if PinSuccess::<T>::get(&cid_hash, o) {
                                ok_count = ok_count.saturating_add(1);
                            }
                        }
                        if ok_count < expect {
                            // 副本不足，需要补充
                            let shortage = expect.saturating_sub(ok_count);
                            
                            // 解析 /pins/{cid}，对比分配并触发降级/修复事件
                            let cid_str = Self::resolve_cid(&cid_hash);
                            // 直接 GET /pins/{cid} 获取状态（Plan B 替换 submit_get_pin_status_collect）
                            if let Some(body) = Self::http_get_bytes(
                                &endpoint,
                                &token,
                                &alloc::format!("/pins/{}", cid_str),
                            ) {
                                let mut online_peers: Vec<Vec<u8>> = Vec::new();
                                if let Ok(json) = serde_json::from_slice::<JsonValue>(&body) {
                                    // 兼容两类结构：{peer_map:{"peerid":{status:"pinned"|...}}} 或 {allocations:["peerid",...]}
                                    if let Some(map) =
                                        json.get("peer_map").and_then(|v| v.as_object())
                                    {
                                        for (pid, st) in map.iter() {
                                            if st.get("status").and_then(|s| s.as_str())
                                                == Some("pinned")
                                            {
                                                online_peers.push(pid.as_bytes().to_vec());
                                            }
                                        }
                                    } else if let Some(arr) =
                                        json.get("allocations").and_then(|v| v.as_array())
                                    {
                                        for v in arr.iter() {
                                            if let Some(s) = v.as_str() {
                                                online_peers.push(s.as_bytes().to_vec());
                                            }
                                        }
                                    }
                                }
                                // 标记降级与修复：对比本地分配和在线列表
                                for op_acc in assign.iter() {
                                    if let Some(info) = Operators::<T>::get(op_acc) {
                                        let present = online_peers
                                            .iter()
                                            .any(|p| p.as_slice() == info.peer_id.as_slice());
                                        let success = PinSuccess::<T>::get(&cid_hash, op_acc);
                                        if present && !success {
                                            PinSuccess::<T>::insert(&cid_hash, op_acc, true);
                                            
                                            // P0-4：自动更新healthy_pins统计
                                            // 注意：这里不修改total_pins（已在分配时+1）
                                            // 仅需要在Pinned时更新healthy_pins
                                            let _ = OperatorPinStats::<T>::try_mutate(op_acc, |stats| {
                                                stats.healthy_pins = stats.healthy_pins.saturating_add(1);
                                                stats.last_check = _n;
                                                // 重新计算健康度得分
                                                stats.health_score = Self::calculate_health_score(op_acc);
                                                Ok::<(), ()>(())
                                            });
                                            
                                            Self::deposit_event(Event::ReplicaRepaired(
                                                cid_hash,
                                                op_acc.clone(),
                                            ));
                                            
                                            // 发送运营者Pin成功事件
                                            if let Some(meta) = PinMeta::<T>::get(&cid_hash) {
                                                Self::deposit_event(Event::OperatorPinSuccess {
                                                    operator: op_acc.clone(),
                                                    cid_hash,
                                                    replicas_confirmed: meta.replicas,
                                                });
                                            }
                                        }
                                        if !present && success {
                                            PinSuccess::<T>::insert(&cid_hash, op_acc, false);
                                            
                                            // P0-4：自动更新监控统计 - Pin失败
                                            // healthy_pins -1, failed_pins +1
                                            let _ = OperatorPinStats::<T>::try_mutate(op_acc, |stats| {
                                                stats.healthy_pins = stats.healthy_pins.saturating_sub(1);
                                                stats.failed_pins = stats.failed_pins.saturating_add(1);
                                                stats.last_check = _n;
                                                
                                                // 重新计算健康度得分
                                                let old_score = stats.health_score;
                                                stats.health_score = Self::calculate_health_score(op_acc);
                                                
                                                // 如果健康度下降超过10分，发送告警
                                                if old_score.saturating_sub(stats.health_score) >= 10 {
                                                    Self::deposit_event(Event::OperatorHealthDegraded {
                                                        operator: op_acc.clone(),
                                                        old_score,
                                                        new_score: stats.health_score,
                                                        total_pins: stats.total_pins,
                                                        failed_pins: stats.failed_pins,
                                                    });
                                                }
                                                
                                                Ok::<(), ()>(())
                                            });
                                            
                                            // 发送运营者Pin失败事件
                                            let reason = alloc::format!("Pin degraded - operator offline or unreachable")
                                                .as_bytes()
                                                .to_vec();
                                            if let Ok(bounded_reason) = BoundedVec::try_from(reason) {
                                                Self::deposit_event(Event::OperatorPinFailed {
                                                    operator: op_acc.clone(),
                                                    cid_hash,
                                                    reason: bounded_reason,
                                                });
                                            }
                                            
                                            // 统计降级次数并触发告警建议
                                            OperatorSla::<T>::mutate(op_acc, |s| {
                                                s.degraded = s.degraded.saturating_add(1);
                                                if s.degraded % 10 == 0 {
                                                    // 简单阈值：每 10 次降级告警
                                                    Self::deposit_event(
                                                        Event::OperatorDegradationAlert(
                                                            op_acc.clone(),
                                                            s.degraded,
                                                        ),
                                                    );
                                                }
                                            });
                                            Self::deposit_event(Event::ReplicaDegraded(
                                                cid_hash,
                                                op_acc.clone(),
                                            ));
                                        }
                                    }
                                }
                                
                                // ⭐ P0优化：自动补充副本使用分层选择算法
                                if shortage > 0 {
                                    // 获取CID的Tier（ValueQuery自动返回默认值）和SubjectType
                                    let tier = CidTier::<T>::get(&cid_hash);
                                    let subject_type = SubjectType::Custom(Default::default());
                                    
                                    // 使用分层选择算法获取新运营者
                                    if let Ok(selection) = Self::select_operators_by_layer(subject_type, tier) {
                                        // 合并Layer 1和Layer 2运营者
                                        let mut new_candidates = selection.core_operators.to_vec();
                                        new_candidates.extend(selection.community_operators.to_vec());
                                        
                                        // 获取当前已分配的运营者（用于去重）
                                        let current_operators: alloc::vec::Vec<T::AccountId> = 
                                            assign.iter().cloned().collect();
                                        
                                        // 过滤出新的运营者（未在当前分配列表中）
                                        let mut updated_assign = assign.clone();
                                        let mut added_count = 0u32;
                                        
                                        for new_op in new_candidates.iter() {
                                            if !current_operators.contains(new_op) && added_count < shortage {
                                                if updated_assign.try_push(new_op.clone()).is_ok() {
                                                    added_count += 1;
                                                }
                                            }
                                        }
                                        
                                        if added_count > 0 {
                                            PinAssignments::<T>::insert(&cid_hash, &updated_assign);
                                            
                                            // 触发事件：已添加新运营者补充副本
                                            Self::deposit_event(Event::AssignmentCreated(
                                                cid_hash,
                                                added_count,
                                            ));
                                        }
                                    }
                                }
                            }
                            // 再 Pin（带退避）
                            let _ = Self::submit_pin_request(&endpoint, &token, cid_hash);
                            PinStateOf::<T>::insert(&cid_hash, 1u8);
                            Self::deposit_event(Event::PinStateChanged(cid_hash, 1));
                            pinning = pinning.saturating_add(1);
                        } else {
                            pinned = pinned.saturating_add(1);
                        }
                    } else {
                        // 无分配但状态为 pinning/pinned，视作缺失
                        missing = missing.saturating_add(1);
                    }
                }
            }
            // 事件上报（轻量只读）：不改变状态，仅供监控
            if sample > 0 {
                Self::deposit_event(Event::PinProbe(sample, pinning, pinned, missing));
            }
            
            // ============================================================================
            // 过期CID物理删除（OCW调用IPFS unpin）
            // ============================================================================
            // 
            // 扫描已过期的CID（PinBilling.state=2），调用IPFS Cluster API执行物理删除
            // 删除成功后清理链上存储
            
            let mut unpinned_count = 0u32;
            const MAX_UNPIN_PER_BLOCK: u32 = 5; // 每块最多删除5个，避免阻塞
            
            for (cid_hash, (_, _, state)) in PinBilling::<T>::iter() {
                if state == 2u8 && unpinned_count < MAX_UNPIN_PER_BLOCK {
                    // 获取明文CID
                    let cid_str = Self::resolve_cid(&cid_hash);
                    
                    // 调用IPFS Cluster API执行物理删除
                    if Self::submit_delete_pin(&endpoint, &token, &cid_str).is_ok() {
                        // 删除成功：清理链上存储
                        PinBilling::<T>::remove(&cid_hash);
                        PinMeta::<T>::remove(&cid_hash);
                        PinStateOf::<T>::remove(&cid_hash);
                        PinSubjectOf::<T>::remove(&cid_hash);
                        PinAssignments::<T>::remove(&cid_hash);
                        CidToSubject::<T>::remove(&cid_hash);
                        CidTier::<T>::remove(&cid_hash);
                        CidRegistry::<T>::remove(&cid_hash);
                        LayeredPinAssignments::<T>::remove(&cid_hash);
                        SimplePinAssignments::<T>::remove(&cid_hash);
                        
                        // 从域索引中移除
                        for (domain, hash, _) in DomainPins::<T>::iter() {
                            if hash == cid_hash {
                                DomainPins::<T>::remove(&domain, &cid_hash);
                                break;
                            }
                        }
                        
                        // 清理健康检查队列
                        for (block, hash, _) in HealthCheckQueue::<T>::iter() {
                            if hash == cid_hash {
                                HealthCheckQueue::<T>::remove(block, &cid_hash);
                                break;
                            }
                        }
                        
                        // 发送删除成功事件
                        Self::deposit_event(Event::PinRemoved {
                            cid_hash,
                            reason: UnpinReason::InsufficientFunds,
                        });
                        
                        unpinned_count += 1;
                    }
                    // 删除失败：保留记录，下次重试
                }
            }
            
            // ============================================================================
            // 公共IPFS网络简化健康检查（无隐私约束版本）
            // ============================================================================
            
            // 从本地存储读取节点账户（用于识别本节点）
            // TODO: 实现从OCW本地存储读取节点账户的逻辑
            // 暂时跳过简化健康检查（需要节点账户配置）
            let local_node_account_bytes = sp_io::offchain::local_storage_get(
                StorageKind::PERSISTENT,
                b"/memo/ipfs/node_account",
            );
            
            // 节点账户未配置，跳过简化健康检查
            let local_node_account_bytes = match local_node_account_bytes {
                Some(bytes) => bytes,
                None => return,
            };
            
            let local_node_account = match T::AccountId::decode(&mut &local_node_account_bytes[..]) {
                Ok(account) => account,
                Err(_) => return, // 解码失败，跳过
            };
            
            // 获取分配给本节点的CID列表（限制每次检查10个，避免阻塞）
            let my_cids = Self::get_my_assigned_cids(&local_node_account, 10);
            
            // 遍历检查每个CID的PIN状态
            for (cid_hash, plaintext_cid) in my_cids.iter() {
                // 调用本地IPFS HTTP API检查Pin状态
                match Self::check_ipfs_pin(plaintext_cid) {
                    Ok(true) => {
                        // Pin存在且健康，上报成功状态
                        Self::deposit_event(Event::SimplePinStatusReported {
                            cid_hash: *cid_hash,
                            node: local_node_account.clone(),
                            status: SimplePinStatus::Pinned,
                        });
                    },
                    Ok(false) => {
                        // Pin不存在，尝试重新Pin
                        if let Ok(()) = Self::pin_to_local_ipfs(plaintext_cid) {
                            // 重新Pin成功
                            Self::deposit_event(Event::SimplePinStatusReported {
                                cid_hash: *cid_hash,
                                node: local_node_account.clone(),
                                status: SimplePinStatus::Restored,
                            });
                        } else {
                            // 重新Pin失败
                            Self::deposit_event(Event::SimplePinStatusReported {
                                cid_hash: *cid_hash,
                                node: local_node_account.clone(),
                                status: SimplePinStatus::Failed,
                            });
                        }
                    },
                    Err(_) => {
                        // HTTP请求失败，上报失败状态
                        Self::deposit_event(Event::SimplePinStatusReported {
                            cid_hash: *cid_hash,
                            node: local_node_account.clone(),
                            status: SimplePinStatus::Failed,
                        });
                    },
                }
                
                // 检查节点负载，发出告警
                let capacity_usage = Self::calculate_simple_capacity_usage(&local_node_account);
                if capacity_usage > 80 {
                    let stats = SimpleNodeStatsMap::<T>::get(&local_node_account);
                    Self::deposit_event(Event::SimpleNodeLoadWarning {
                        node: local_node_account.clone(),
                        capacity_usage: capacity_usage as u8,
                        current_pins: stats.total_pins,
                    });
                }
            }
        }
        
        /// 函数级详细中文注释：区块结束时的自动化任务（优化改造）
        /// 
        /// 执行顺序（优先级从高到低）：
        /// 1. 周期扣费（确保资金流转）
        /// 2. 健康巡检（确保数据安全）
        /// 3. 统计更新（链上仪表板）
        /// 
        /// 限流保护：
        /// - 每块最多处理20个扣费任务
        /// - 每块最多处理10个巡检任务
        /// - 防止区块拥堵
        fn on_finalize(n: BlockNumberFor<T>) {
            let current_block = n;
            
            // 检查是否暂停扣费
            if BillingPaused::<T>::get() {
                return;
            }
            
            // ======== 任务1：自动周期扣费 ========
            let max_charges_per_block = 20u32;
            let mut charged = 0u32;
            
            // 遍历到期的扣费任务（due_block <= current_block）
            let mut tasks_to_process: alloc::vec::Vec<(BlockNumberFor<T>, T::Hash, BillingTask<BlockNumberFor<T>, BalanceOf<T>>)> 
                = alloc::vec::Vec::new();
            
            // 收集到期任务（限制数量）
            for (due_block, cid_hash, task) in BillingQueue::<T>::iter() {
                if due_block <= current_block && charged < max_charges_per_block {
                    tasks_to_process.push((due_block, cid_hash, task));
                    charged += 1;
                }
            }
            
            // 处理收集到的任务
            for (due_block, cid_hash, mut task) in tasks_to_process {
                // 执行四层回退扣费
                match Self::four_layer_charge(&cid_hash, &mut task) {
                    Ok(ChargeResult::Success { layer }) => {
                        // 扣费成功：更新下次扣费时间
                        let next_billing = current_block + task.billing_period.into();
                        task.last_charge = current_block;
                        task.charge_layer = layer;
                        task.grace_status = GraceStatus::Normal;
                        BillingQueue::<T>::insert(next_billing, &cid_hash, task);
                        
                        // 移除旧的队列项
                        BillingQueue::<T>::remove(due_block, &cid_hash);
                    },
                    Ok(ChargeResult::EnterGrace { expires_at }) => {
                        // 进入宽限期：发送通知
                        task.grace_status = GraceStatus::InGrace {
                            entered_at: current_block,
                            expires_at,
                        };
                        // 1小时后再试
                        let next_billing = current_block + 1200u32.into();
                        BillingQueue::<T>::insert(next_billing, &cid_hash, task);
                        
                        Self::deposit_event(Event::GracePeriodStarted {
                            cid_hash: cid_hash.clone(),
                            expires_at,
                        });
                        
                        // 移除旧的队列项
                        BillingQueue::<T>::remove(due_block, &cid_hash);
                    },
                    Err(_) => {
                        // 宽限期已过，标记Unpin
                        task.grace_status = GraceStatus::Expired;
                        
                        // 从所有队列中移除
                        BillingQueue::<T>::remove(due_block, &cid_hash);
                        
                        // 标记为过期（保留兼容旧逻辑）
                        if let Some((_, unit_price, _)) = PinBilling::<T>::get(&cid_hash) {
                            PinBilling::<T>::insert(&cid_hash, (current_block, unit_price, 2u8)); // 2=Expired
                        }
                        
                        Self::deposit_event(Event::MarkedForUnpin {
                            cid_hash: cid_hash.clone(),
                            reason: UnpinReason::InsufficientFunds,
                        });
                    },
                }
            }
            
            // ======== 任务2：自动健康巡检 ========
            let max_checks_per_block = 10u32;
            let mut checked = 0u32;
            
            // 收集到期的巡检任务
            let mut checks_to_process: alloc::vec::Vec<(BlockNumberFor<T>, T::Hash, HealthCheckTask<BlockNumberFor<T>>)> 
                = alloc::vec::Vec::new();
            
            for (check_block, cid_hash, task) in HealthCheckQueue::<T>::iter() {
                if check_block <= current_block && checked < max_checks_per_block {
                    checks_to_process.push((check_block, cid_hash, task));
                    checked += 1;
                }
            }
            
            // 处理巡检任务
            for (check_block, cid_hash, mut task) in checks_to_process {
                // 执行巡检（简化版，实际应在OCW中调用IPFS Cluster API）
                let status = Self::check_pin_health(&cid_hash);
                
                // 获取分层配置
                let tier_config = Self::get_tier_config(&task.tier).unwrap_or_default();
                
                // 根据状态决定下一步
                match status {
                    HealthStatus::Healthy { .. } => {
                        // 健康：重新入队，正常间隔
                        let next_check = current_block + tier_config.health_check_interval.into();
                        task.last_check = current_block;
                        task.last_status = status.clone();
                        task.consecutive_failures = 0;
                        HealthCheckQueue::<T>::insert(next_check, &cid_hash, task);
                    },
                    HealthStatus::Degraded { current_replicas, target } => {
                        // 降级：缩短巡检间隔（降级期间更频繁检查）
                        let urgent_interval = tier_config.health_check_interval / 4;
                        let next_check = current_block + urgent_interval.into();
                        task.last_check = current_block;
                        task.last_status = status.clone();
                        task.consecutive_failures = task.consecutive_failures.saturating_add(1);
                        HealthCheckQueue::<T>::insert(next_check, &cid_hash, task);
                        
                        // 发送告警事件
                        Self::deposit_event(Event::HealthDegraded {
                            cid_hash: cid_hash.clone(),
                            current_replicas,
                            target,
                        });
                    },
                    HealthStatus::Critical { current_replicas } => {
                        // 危险：极短巡检间隔（每小时检查一次）
                        let critical_interval = 1200u32; // ~1小时
                        let next_check = current_block + critical_interval.into();
                        task.last_check = current_block;
                        task.last_status = status.clone();
                        task.consecutive_failures = task.consecutive_failures.saturating_add(1);
                        HealthCheckQueue::<T>::insert(next_check, &cid_hash, task);
                        
                        // 发送紧急告警
                        Self::deposit_event(Event::HealthCritical {
                            cid_hash: cid_hash.clone(),
                            current_replicas,
                        });
                    },
                    HealthStatus::Unknown => {
                        // 未知：可能是网络问题，稍后重试
                        let retry_interval = 600u32; // ~30分钟后重试
                        let next_check = current_block + retry_interval.into();
                        task.last_check = current_block;
                        task.last_status = status.clone();
                        task.consecutive_failures = task.consecutive_failures.saturating_add(1);
                        
                        // 连续失败5次，发送告警
                        if task.consecutive_failures >= 5 {
                            Self::deposit_event(Event::HealthCheckFailed {
                                cid_hash: cid_hash.clone(),
                                failures: task.consecutive_failures,
                            });
                        }
                        
                        HealthCheckQueue::<T>::insert(next_check, &cid_hash, task);
                    },
                }
                
                // 移除旧的队列项
                HealthCheckQueue::<T>::remove(check_block, &cid_hash);
            }
            
            // ======== 任务3：域统计更新（每24小时一次）========
            // ⭐ 使用域级统计替代全局统计，自动汇总全局数据
            if current_block % 7200u32.into() == Zero::zero() {
                Self::update_domain_health_stats_impl();
            }
        }
    }
    
    /// 函数级详细中文注释：域级健康统计（替代全局统计）
    /// 
    /// ⭐ 优化说明：
    /// - 旧版本：update_global_health_stats_impl() 全量扫描所有Pin
    /// - 新版本：update_domain_health_stats_impl() 按域扫描并自动汇总
    /// 
    /// 优势：
    /// 1. 性能优化：使用 iter_prefix 减少扫描范围
    /// 2. 可观测性：提供域级别的细粒度统计
    /// 3. 优先级调度：按域优先级顺序处理
    /// 4. 自动汇总：域统计完成后自动更新全局统计
    impl<T: Config> Pallet<T> {
        
        /// 函数级详细中文注释：按域统计Pin健康状态（OCW调用）
        /// 
        /// ### 功能
        /// - 按优先级顺序遍历各域
        /// - 统计每个域的Pin数量、存储容量、健康状态
        /// - 更新域级统计数据
        /// - 发送域统计更新事件
        /// 
        /// ### 性能优化
        /// - 使用 iter_prefix 只遍历特定域的CID
        /// - 批量限制：每域最多处理1000个CID
        /// - 自动跳过空域
        /// 
        /// ### 调用时机
        /// - OCW中每24小时执行一次（与全局统计同步）
        fn update_domain_health_stats_impl() {
            use sp_std::collections::btree_set::BTreeSet;
            
            let current_block = <frame_system::Pallet<T>>::block_number();
            
            // 1. 获取所有已注册的域
            let mut domains_with_priority: Vec<(BoundedVec<u8, ConstU32<32>>, u8)> = Vec::new();
            
            // 遍历 DomainPins 获取所有域名
            let mut seen_domains = BTreeSet::new();
            for (domain, _, _) in DomainPins::<T>::iter() {
                if seen_domains.insert(domain.clone()) {
                    let priority = DomainPriority::<T>::get(&domain);
                    domains_with_priority.push((domain, priority));
                }
            }
            
            // 2. 按优先级排序（数值越小优先级越高）
            domains_with_priority.sort_by_key(|(_domain, priority)| *priority);
            
            // 3. 按域顺序统计
            for (domain, _priority) in domains_with_priority.iter() {
                let mut domain_stats = DomainStats {
                    domain: domain.clone(),
                    total_pins: 0,
                    total_size_bytes: 0,
                    healthy_count: 0,
                    degraded_count: 0,
                    critical_count: 0,
                };
                
                let mut cid_count = 0u32;
                const MAX_CIDS: u32 = 1000;  // 批量限制
                
                // ⭐ 使用前缀迭代器高效遍历该域的CID
                for (cid_hash, _) in DomainPins::<T>::iter_prefix(domain) {
                    if cid_count >= MAX_CIDS {
                        break;  // 限制处理数量
                    }
                    
                    domain_stats.total_pins += 1;
                    
                    // 获取Pin元信息
                    if let Some(meta) = PinMeta::<T>::get(&cid_hash) {
                        domain_stats.total_size_bytes += meta.size;
                    }
                    
                    // 检查健康状态
                    let mut found_health = false;
                    for (_, hash, task) in HealthCheckQueue::<T>::iter() {
                        if hash == cid_hash {
                            match task.last_status {
                                HealthStatus::Healthy { .. } => {
                                    domain_stats.healthy_count += 1;
                                },
                                HealthStatus::Degraded { .. } => {
                                    domain_stats.degraded_count += 1;
                                },
                                HealthStatus::Critical { .. } => {
                                    domain_stats.critical_count += 1;
                                },
                                _ => {},
                            }
                            found_health = true;
                            break;
                        }
                    }
                    
                    // 未找到健康检查记录，默认为健康
                    if !found_health {
                        domain_stats.healthy_count += 1;
                    }
                    
                    cid_count += 1;
                }
                
                // 4. 存储统计结果
                DomainHealthStats::<T>::insert(domain, domain_stats.clone());
                
                // 5. 发送事件
                Self::deposit_event(Event::DomainStatsUpdated {
                    domain: domain.clone(),
                    total_pins: domain_stats.total_pins,
                    total_size_bytes: domain_stats.total_size_bytes,
                    healthy_count: domain_stats.healthy_count,
                    degraded_count: domain_stats.degraded_count,
                    critical_count: domain_stats.critical_count,
                });
            }
            
            // 6. 更新全局统计（汇总所有域）
            let mut global_stats = GlobalHealthStats::<BlockNumberFor<T>>::default();
            for (_domain, stats) in DomainHealthStats::<T>::iter() {
                global_stats.total_pins += stats.total_pins;
                global_stats.total_size_bytes += stats.total_size_bytes;
                global_stats.healthy_count += stats.healthy_count;
                global_stats.degraded_count += stats.degraded_count;
                global_stats.critical_count += stats.critical_count;
            }
            global_stats.last_full_scan = current_block;
            HealthCheckStats::<T>::put(global_stats);
        }
        
        /// 函数级详细中文注释：查询域统计（RPC接口）
        /// 
        /// ### 功能
        /// - 查询指定域的统计信息
        /// - 返回Pin数量、存储容量、健康状态
        /// 
        /// ### 参数
        /// - `domain`：域名（如 b"subject"）
        /// 
        /// ### 返回
        /// - `Option<DomainStats>`：域统计信息，如果域不存在返回None
        /// 
        /// ### 使用场景
        /// - Dashboard查询域统计
        /// - 监控系统获取域状态
        pub fn get_domain_stats(domain: Vec<u8>) -> Option<DomainStats> {
            if let Ok(bounded_domain) = BoundedVec::try_from(domain) {
                DomainHealthStats::<T>::get(&bounded_domain)
            } else {
                None
            }
        }
        
        /// 函数级详细中文注释：查询所有域统计（RPC接口）
        /// 
        /// ### 功能
        /// - 查询所有已注册域的统计信息
        /// - 按优先级排序返回
        /// 
        /// ### 返回
        /// - `Vec<(Vec<u8>, DomainStats, u8)>`：域列表
        ///   - 域名
        ///   - 域统计
        ///   - 优先级
        /// 
        /// ### 使用场景
        /// - Dashboard展示所有域的统计
        /// - 监控系统全局视图
        pub fn get_all_domain_stats() -> Vec<(Vec<u8>, DomainStats, u8)> {
            let mut result = Vec::new();
            
            for (domain, stats) in DomainHealthStats::<T>::iter() {
                let priority = DomainPriority::<T>::get(&domain);
                result.push((domain.to_vec(), stats, priority));
            }
            
            // 按优先级排序（优先级越小越靠前）
            result.sort_by_key(|(_, _, priority)| *priority);
            
            result
        }
        
        /// 函数级详细中文注释：查询域的CID列表（RPC接口，分页）
        /// 
        /// ### 功能
        /// - 查询指定域的CID列表（分页）
        /// - 返回CID及其元数据
        /// 
        /// ### 参数
        /// - `domain`：域名
        /// - `offset`：分页偏移量
        /// - `limit`：每页数量（最大100）
        /// 
        /// ### 返回
        /// - `Vec<(T::Hash, PinMetadata<BlockNumberFor<T>>)>`：CID列表
        ///   - CID的hash
        ///   - Pin元数据
        /// 
        /// ### 使用场景
        /// - Dashboard查看域的详细CID列表
        /// - 调试和诊断
        pub fn get_domain_cids(
            domain: Vec<u8>,
            offset: u32,
            limit: u32,
        ) -> Vec<(T::Hash, PinMetadata<BlockNumberFor<T>>)> {
            let limit = limit.min(100);  // 限制最大100条
            let mut result = Vec::new();
            
            if let Ok(bounded_domain) = BoundedVec::try_from(domain) {
                let mut count = 0u32;
                let mut skipped = 0u32;
                
                for (cid_hash, _) in DomainPins::<T>::iter_prefix(&bounded_domain) {
                    // 跳过offset之前的记录
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    
                    // 达到limit后停止
                    if count >= limit {
                        break;
                    }
                    
                    // 获取元数据
                    if let Some(meta) = PinMeta::<T>::get(&cid_hash) {
                        result.push((cid_hash, meta));
                        count += 1;
                    }
                }
            }
            
            result
        }
    }

    impl<T: Config> Pallet<T> {
        /// 函数级中文注释：只读统计 - 读取某块到期列表的长度（便于前端/索引层分页）。
        pub fn due_at_count(block: BlockNumberFor<T>) -> u32 {
            DueQueue::<T>::get(block).len() as u32
        }
        /// 函数级中文注释：只读 - 在闭区间 [from, to] 返回非空到期列表的块号与长度元组（最多 512 条）。
        pub fn due_between(
            from: BlockNumberFor<T>,
            to: BlockNumberFor<T>,
        ) -> BoundedVec<(BlockNumberFor<T>, u32), ConstU32<512>> {
            let mut out: BoundedVec<(BlockNumberFor<T>, u32), ConstU32<512>> = Default::default();
            let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
            let mut n = lo;
            while n <= hi {
                let c = DueQueue::<T>::get(n).len() as u32;
                if c > 0 {
                    let _ = out.try_push((n, c));
                }
                if out.len() as u32 >= 512 {
                    break;
                }
                n = n.saturating_add(1u32.into());
            }
            out
        }
        /// 函数级详细中文注释：扩散入队工具函数
        /// - 在 base..base+spread 范围内寻找首个未满的队列入队；全部满则放弃（避免单点拥塞）。
        #[inline]
        fn enqueue_due(cid: T::Hash, base_next: BlockNumberFor<T>) {
            let spread: u32 = DueEnqueueSpread::<T>::get();
            let mut inserted = false;
            for off in 0..=spread {
                let key = base_next.saturating_add(off.into());
                let mut v = DueQueue::<T>::get(key);
                if v.try_push(cid).is_ok() {
                    DueQueue::<T>::insert(key, v);
                    inserted = true;
                    break;
                }
            }
            if !inserted { /* 放弃，治理可通过扫描修复 */ }
        }
        /// 函数级详细中文注释：GET 请求帮助函数，返回主体字节（2xx 才返回）
        fn http_get_bytes(endpoint: &str, token: &Option<String>, path: &str) -> Option<Vec<u8>> {
            let url = alloc::format!("{}{}", endpoint, path);
            let mut req = http::Request::get(&url);
            if let Some(t) = token.as_ref() {
                req = req.add_header("Authorization", &alloc::format!("Bearer {}", t));
            }
            let timeout = sp_io::offchain::timestamp()
                .add(sp_runtime::offchain::Duration::from_millis(3_000));
            let pending = req.deadline(timeout).send().ok()?;
            // try_wait 返回 Result<Option<Response>, _> → ok()?.ok()? 解包为 Response
            let resp = pending.try_wait(timeout).ok()?.ok()?;
            let code: u16 = resp.code;
            if (200..300).contains(&code) {
                Some(resp.body().collect::<Vec<u8>>())
            } else {
                None
            }
        }

        /// 函数级详细中文注释：通过 OCW 发送 HTTP POST /pins 请求到 ipfs-cluster
        /// - 仅示例：构造最小 JSON 体，包含 `cid` 字段（此处我们只有 `cid_hash`，生产应由 OCW 从密文解出 CID）。
        /// - 返回：若 HTTP 状态为 2xx 则认为提交成功，随后发起 `mark_pinned` 外部交易。
        fn submit_pin_request(
            endpoint: &str,
            token: &Option<String>,
            cid_hash: T::Hash,
        ) -> Result<(), ()> {
            let cid_hex = hex::encode(cid_hash.as_ref());
            // 构造最小 JSON（根据你的 API 需要调整）
            let body_json = alloc::format!(r#"{{"cid":"{}"}}"#, cid_hex);
            let body_vec: Vec<u8> = body_json.into_bytes();
            let url = alloc::format!("{}/pins", endpoint);
            // 不用切片：使用 Vec<Vec<u8>> 作为 POST body，以满足 add_header/deadline 的 T: Default 约束
            let chunks: Vec<Vec<u8>> = alloc::vec![body_vec];
            let mut req = http::Request::post(&url, chunks);
            if let Some(t) = token.as_ref() {
                req = req
                    .add_header("Authorization", &alloc::format!("Bearer {}", t))
                    .add_header("Content-Type", "application/json");
            }
            let timeout = sp_io::offchain::timestamp()
                .add(sp_runtime::offchain::Duration::from_millis(5_000));
            let pending = req.deadline(timeout).send().map_err(|_| ())?;
            let resp = pending.try_wait(timeout).map_err(|_| ())?.map_err(|_| ())?;
            let code: u16 = resp.code;
            if (200..300).contains(&code) {
                Ok(())
            } else {
                Err(())
            }
        }

        /// 函数级详细中文注释：通过 OCW 发送 HTTP DELETE /pins/{cid}
        /// 
        /// 功能：
        /// - 调用IPFS Cluster API执行物理unpin操作
        /// - 某些环境下使用 `X-HTTP-Method-Override: DELETE` 搭配 POST 以规避代理限制
        /// 
        /// 调用时机：
        /// - OCW扫描到过期CID（PinBilling.state=2）时自动调用
        /// 
        /// 返回：
        /// - Ok(())：删除成功（HTTP 2xx）
        /// - Err(())：删除失败（网络错误或HTTP非2xx）
        fn submit_delete_pin(
            endpoint: &str,
            token: &Option<String>,
            cid_str: &str,
        ) -> Result<(), ()> {
            let url = alloc::format!("{}/pins/{}", endpoint, cid_str);
            // 不用切片：空体使用 Vec<Vec<u8>>
            let chunks: Vec<Vec<u8>> = alloc::vec![Vec::new()];
            let mut req =
                http::Request::post(&url, chunks).add_header("X-HTTP-Method-Override", "DELETE");
            if let Some(t) = token.as_ref() {
                req = req.add_header("Authorization", &alloc::format!("Bearer {}", t));
            }
            let timeout = sp_io::offchain::timestamp()
                .add(sp_runtime::offchain::Duration::from_millis(5_000));
            let pending = req.deadline(timeout).send().map_err(|_| ())?;
            let resp = pending.try_wait(timeout).map_err(|_| ())?.map_err(|_| ())?;
            let code: u16 = resp.code;
            if (200..300).contains(&code) {
                Ok(())
            } else {
                Err(())
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// 函数级中文注释：只读接口——根据运营者账户派生对应的押金保留账户地址。
        pub fn operator_bond_account(operator: &T::AccountId) -> T::AccountId {
            T::SubjectPalletId::get()
                .try_into_sub_account((b"bond", operator))
                .expect("pallet sub-account derivation should not fail")
        }

        /// 函数级中文注释：只读接口——根据 subject_id 派生其资金账户地址。
        pub fn subject_account(subject_id: u64) -> T::AccountId {
            // 使用 General 类型的域编码（2）作为默认
            Self::subject_account_for(2u8, subject_id)
        }
    }

    /// 权重占位：后续通过 benchmarking 填充
    pub trait WeightInfo {
        fn request_pin() -> Weight;
        fn mark_pinned() -> Weight;
        fn mark_pin_failed() -> Weight;
        /// 函数级中文注释：到期扣费，按 limit 线性增长（读写多项状态）。
        fn charge_due(limit: u32) -> Weight;
        /// 函数级中文注释：设置计费参数，常量级权重（少量读写）。
        fn set_billing_params() -> Weight;
    }
    impl WeightInfo for () {
        fn request_pin() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn mark_pinned() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn mark_pin_failed() -> Weight {
            Weight::from_parts(10_000, 0)
        }
        fn charge_due(limit: u32) -> Weight {
            // 简化：基准前权重估算（常数项 + 每件线性项）
            Weight::from_parts(20_000, 0)
                .saturating_add(Weight::from_parts(5_000, 0).saturating_mul(limit.into()))
        }
        fn set_billing_params() -> Weight {
            Weight::from_parts(20_000, 0)
        }
    }
}

// 函数级中文注释：将 pallet 模块内导出的类型（如 Pallet、Call、Event 等）在 crate 根进行再导出
// 作用：
// 1) 让 runtime 集成宏（#[frame_support::runtime]）能够找到 `tt_default_parts_v2` 等默认部件；
// 2) 便于上层以 `pallet_storage_service::Call` 等简洁路径引用类型，降低路径耦合。
pub use pallet::*;

pub mod weights;
// 注意：WeightInfo trait 定义在 pallet 模块内，但实现已移至 weights.rs

/// 函数级详细中文注释：为 Pallet<T> 实现 IpfsPinner trait，供其他pallet调用
/// 
/// 实现说明（⭐ P1优化后）：
/// - 直接调用破坏式改造后的 `request_pin_for_subject` extrinsic；
/// - 使用 `four_layer_charge` 三层扣费机制（IpfsPool → SubjectFunding → Grace）；
/// - 支持分层运营者选择（Layer 1 Core + Layer 2 Community）；
/// - 自动分配收益给运营者。
impl<T: Config> IpfsPinner<<T as frame_system::Config>::AccountId, BalanceOf<T>> for Pallet<T> {
    /// 函数级详细中文注释：为主体关联的CID发起pin请求
    /// 
    /// 内部实现：
    /// 1. 将Vec<u8> CID转换为BoundedVec
    /// 2. 调用破坏式改造后的 `request_pin_for_subject` extrinsic
    /// 3. 使用 `four_layer_charge` 三层扣费逻辑（IpfsPool → SubjectFunding → Grace）
    /// 4. 支持分层运营者选择（Layer 1 Core + Layer 2 Community）
    fn pin_cid_for_subject(
        caller: <T as frame_system::Config>::AccountId,
        _subject_type: SubjectType,  // 暂未使用，保留用于未来扩展
        subject_id: u64,
        cid: Vec<u8>,
        tier: Option<PinTier>,
    ) -> DispatchResult {
        // 直接调用破坏式重写的request_pin_for_subject
        Self::request_pin_for_subject(
            OriginFor::<T>::from(Some(caller).into()),
            subject_id,
            cid,
            tier,
        )
    }

    /// 函数级详细中文注释：取消固定CID
    /// 
    /// 内部实现：
    /// 1. 计算CID哈希
    /// 2. 验证调用者是CID所有者
    /// 3. 标记为待删除状态（state=2）
    /// 4. OCW将在后续区块执行物理删除
    fn unpin_cid(
        caller: <T as frame_system::Config>::AccountId,
        cid: Vec<u8>,
    ) -> DispatchResult {
        use sp_runtime::traits::Hash;
        
        // 1. 计算CID哈希
        let cid_hash = T::Hashing::hash(&cid[..]);
        
        // 2. 检查CID是否存在
        if !PinMeta::<T>::contains_key(&cid_hash) {
            // CID不存在，直接返回成功（幂等操作）
            return Ok(());
        }
        
        // 3. 权限验证：检查调用者是否为CID所有者
        if let Some((owner, _subject_id)) = PinSubjectOf::<T>::get(&cid_hash) {
            // 只有所有者才能取消固定
            ensure!(caller == owner, Error::<T>::NotOwner);
        } else {
            // 没有所有者记录，拒绝操作（安全起见）
            return Err(Error::<T>::NotOwner.into());
        }
        
        // 4. 标记为待删除状态
        // 更新 PinBilling 状态为 2（Expired/待删除）
        let current_block = <frame_system::Pallet<T>>::block_number();
        if let Some((_, unit_price, _)) = PinBilling::<T>::get(&cid_hash) {
            PinBilling::<T>::insert(&cid_hash, (current_block, unit_price, 2u8));
        }
        
        // 5. 发送事件
        Self::deposit_event(Event::MarkedForUnpin {
            cid_hash,
            reason: UnpinReason::ManualRequest,
        });
        
        Ok(())
    }
}

/// 函数级详细中文注释：ContentRegistry trait实现 - 新pallet域自动PIN机制
/// 
/// 提供统一的内容注册接口，让新业务pallet无需了解IPFS细节即可实现内容固定。
impl<T: Config> ContentRegistry for Pallet<T> {
    /// 函数级详细中文注释：注册内容到IPFS（核心实现）
    /// 
    /// 功能流程：
    /// 1. 检查域是否已注册，未注册则自动创建
    /// 2. 根据域配置派生SubjectType
    /// 3. 调用内部PIN逻辑
    /// 4. 自动扣费（三层机制）
    fn register_content(
        domain: Vec<u8>,
        subject_id: u64,
        cid: Vec<u8>,
        tier: PinTier,
    ) -> DispatchResult {
        // 1. 转换域名为BoundedVec
        let bounded_domain: BoundedVec<u8, ConstU32<32>> = domain
            .try_into()
            .map_err(|_| Error::<T>::InvalidDomain)?;
        
        // 2. 检查或创建域
        let domain_config = RegisteredDomains::<T>::get(&bounded_domain)
            .unwrap_or_else(|| {
                // 自动创建默认域配置
                let config = types::DomainConfig {
                    auto_pin_enabled: true,
                    default_tier: tier.clone(),
                    subject_type_id: 99, // 默认自定义类型
                    owner_pallet: bounded_domain.clone(),
                    created_at: {
                        use sp_runtime::SaturatedConversion;
                        frame_system::Pallet::<T>::block_number().saturated_into()
                    },
                };
                RegisteredDomains::<T>::insert(&bounded_domain, &config);
                
                // 发送域注册事件
                Self::deposit_event(Event::DomainRegistered {
                    domain: bounded_domain.clone(),
                    subject_type_id: config.subject_type_id,
                });
                
                config
            });
        
        // 3. 检查域是否启用自动PIN
        ensure!(domain_config.auto_pin_enabled, Error::<T>::DomainPinDisabled);
        
        // 4. 创建临时caller（使用IpfsPoolAccount）
        let caller = T::IpfsPoolAccount::get();
        
        // 5. 调用PIN逻辑（使用subject逻辑，因为它支持SubjectFunding）
        Self::request_pin_for_subject(
            OriginFor::<T>::from(frame_system::RawOrigin::Signed(caller)),
            subject_id,
            cid.clone(),
            Some(tier.clone()),
        )?;
        
        // 6. 更新域索引
        let cid_hash = <T::Hashing as sp_runtime::traits::Hash>::hash(&cid);
        DomainPins::<T>::insert(&bounded_domain, &cid_hash, ());
        
        // 7. 发送成功事件
        Self::deposit_event(Event::ContentRegisteredViaDomain {
            domain: bounded_domain,
            subject_id,
            cid_hash,
            tier,
        });
        
        Ok(())
    }
    
    /// 函数级详细中文注释：查询域是否已注册
    fn is_domain_registered(domain: &[u8]) -> bool {
        if let Ok(bounded_domain) = BoundedVec::<u8, ConstU32<32>>::try_from(domain.to_vec()) {
            RegisteredDomains::<T>::contains_key(&bounded_domain)
        } else {
            false
        }
    }
    
    /// 函数级详细中文注释：获取域的SubjectType映射
    fn get_domain_subject_type(domain: &[u8]) -> Option<SubjectType> {
        if let Ok(bounded_domain) = BoundedVec::<u8, ConstU32<32>>::try_from(domain.to_vec()) {
            RegisteredDomains::<T>::get(&bounded_domain).map(|config| {
                // 根据subject_type_id映射到SubjectType
                match config.subject_type_id {
                    0 => SubjectType::Evidence,
                    1 => SubjectType::OtcOrder,
                    5 => SubjectType::Chat,
                    6 => SubjectType::Livestream,
                    7 => SubjectType::Swap,
                    8 => SubjectType::Arbitration,
                    9 => SubjectType::UserProfile,
                    98 => SubjectType::General,
                    _ => SubjectType::Custom(bounded_domain),
                }
            })
        } else {
            None
        }
    }

    /// 函数级详细中文注释：取消注册内容（Unpin）
    /// 
    /// 功能流程：
    /// 1. 计算 CID 哈希
    /// 2. 标记为待删除状态（state=2）
    /// 3. OCW 将在后续区块执行物理删除
    fn unregister_content(
        _domain: Vec<u8>,
        cid: Vec<u8>,
    ) -> DispatchResult {
        // 1. 计算 CID 哈希
        let cid_hash = <T::Hashing as sp_runtime::traits::Hash>::hash(&cid);
        
        // 2. 检查 CID 是否存在
        if !PinBilling::<T>::contains_key(&cid_hash) {
            // CID 不存在，直接返回成功（幂等操作）
            return Ok(());
        }
        
        // 3. 标记为待删除状态
        let current_block = <frame_system::Pallet<T>>::block_number();
        if let Some((_, unit_price, _)) = PinBilling::<T>::get(&cid_hash) {
            PinBilling::<T>::insert(&cid_hash, (current_block, unit_price, 2u8)); // 2=Expired/待删除
        }
        
        // 4. 发送事件
        Self::deposit_event(Event::MarkedForUnpin {
            cid_hash,
            reason: UnpinReason::ManualRequest,
        });
        
        Ok(())
    }
}

// ⭐ P1优化：已删除 old_pin_cid_for_subject() 函数（68行）
// 原因：已被 request_pin_for_subject() extrinsic的破坏式改造替代
// 该函数使用了已删除的 triple_charge_storage_fee()
// 删除日期：2025-10-26

/// CidLockManager trait 实现 - 证据锁定机制
impl<T: Config> CidLockManager<T::Hash, BlockNumberFor<T>> for Pallet<T> {
    fn lock_cid(_cid_hash: T::Hash, _reason: Vec<u8>, _until: Option<BlockNumberFor<T>>) -> DispatchResult {
        // TODO: 实现 CID 锁定存储逻辑
        // 当前为 stub 实现，允许编译通过
        Ok(())
    }
    
    fn unlock_cid(_cid_hash: T::Hash, _reason: Vec<u8>) -> DispatchResult {
        // TODO: 实现 CID 解锁逻辑
        Ok(())
    }
    
    fn is_locked(_cid_hash: &T::Hash) -> bool {
        // TODO: 实现锁定状态查询
        false
    }
}

#[cfg(test)]
mod tests;

// ============================================================================
// 公共IPFS网络简化PIN管理实现（无隐私约束版本）
// ============================================================================

impl<T: Config> Pallet<T> {
    /// 函数级详细中文注释：简化的智能PIN分配算法（公共IPFS网络）
    /// 
    /// 根据PinTier确定副本数，然后选择最优节点：
    /// - Critical数据：3副本（3个节点）
    /// - Standard数据：2副本（2个节点）
    /// - Temporary数据：1副本（1个节点）
    /// 
    /// 节点选择策略（简化评分算法）：
    /// score = capacity_usage(50%) + (100 - health_score)(50%)
    /// 评分越低，节点越优先被选择
    /// 
    /// 参数：
    /// - cid_hash: CID的hash值
    /// - tier: PIN层级
    /// - estimated_size: 估算的文件大小（字节）
    /// 
    /// 返回：
    /// - 选中的节点列表
    pub fn optimized_pin_allocation(
        cid_hash: T::Hash,
        tier: PinTier,
        estimated_size: u64,
    ) -> Result<BoundedVec<T::AccountId, ConstU32<8>>, Error<T>> {
        // 1. 获取所有活跃节点（从Operators或者一个简化的节点列表）
        let all_nodes = Self::get_active_ipfs_nodes()?;
        
        // 2. 确定副本数（简化策略）
        let replica_count = match tier {
            PinTier::Critical => 3,  // 3副本（充分冗余）
            PinTier::Standard => 2,  // 2副本（平衡）
            PinTier::Temporary => 1, // 1副本（最小化）
        };
        
        // 3. 智能选择节点（基于负载和健康度）
        let selected_nodes = Self::select_best_ipfs_nodes(
            &all_nodes,
            replica_count,
            estimated_size,
        )?;
        
        // 4. 记录分配
        SimplePinAssignments::<T>::insert(&cid_hash, selected_nodes.clone());
        
        // 5. 更新节点统计（增加PIN计数）
        for node in selected_nodes.iter() {
            SimpleNodeStatsMap::<T>::mutate(node, |stats| {
                stats.total_pins = stats.total_pins.saturating_add(1);
            });
        }
        
        // 6. 发送事件
        Self::deposit_event(Event::SimplePinAllocated {
            cid_hash,
            tier,
            nodes: selected_nodes.clone(),
            replicas: replica_count,
        });
        
        Ok(selected_nodes)
    }
    
    /// 函数级详细中文注释：获取所有活跃的IPFS节点
    /// 
    /// 从Operators存储中筛选出状态为Active(0)的节点
    /// 
    /// 返回：活跃节点账户列表
    fn get_active_ipfs_nodes() -> Result<alloc::vec::Vec<T::AccountId>, Error<T>> {
        let mut active_nodes = alloc::vec::Vec::new();
        
        for (node, info) in Operators::<T>::iter() {
            // 只选择Active状态的运营者
            if info.status == 0 {
                active_nodes.push(node);
            }
        }
        
        ensure!(!active_nodes.is_empty(), Error::<T>::NoAvailableOperators);
        
        Ok(active_nodes)
    }
    
    /// 函数级详细中文注释：选择最优IPFS节点（简化评分）
    /// 
    /// 评分算法：
    /// score = capacity_usage(50%) + (100 - health_score)(50%)
    /// 
    /// 容量检查：
    /// - 容量使用率 > 90% 的节点自动跳过
    /// 
    /// 参数：
    /// - nodes: 候选节点列表
    /// - count: 需要选择的节点数
    /// - estimated_size: 估算的文件大小
    /// 
    /// 返回：
    /// - 选中的节点列表（BoundedVec）
    fn select_best_ipfs_nodes(
        nodes: &alloc::vec::Vec<T::AccountId>,
        count: u32,
        _estimated_size: u64,
    ) -> Result<BoundedVec<T::AccountId, ConstU32<8>>, Error<T>> {
        let mut node_scores: alloc::vec::Vec<(T::AccountId, u32)> = alloc::vec::Vec::new();
        
        for node in nodes {
            let stats = SimpleNodeStatsMap::<T>::get(node);
            
            // 计算容量使用率（简化估算）
            let capacity_usage = Self::calculate_simple_capacity_usage(node);
            
            // 容量检查（使用率 > 90%则跳过）
            if capacity_usage > 90 {
                continue;
            }
            
            // 简化评分公式
            // score = capacity_usage(50%) + (100 - health_score)(50%)
            let health_penalty = 100u8.saturating_sub(stats.health_score);
            let score = (capacity_usage / 2) + (health_penalty as u32 / 2);
            
            node_scores.push((node.clone(), score));
        }
        
        // 按评分排序（升序，评分越低越优先）
        node_scores.sort_by(|a, b| a.1.cmp(&b.1));
        
        // 选择前N个节点
        let selected: alloc::vec::Vec<T::AccountId> = node_scores
            .iter()
            .take(count as usize)
            .map(|(node, _)| node.clone())
            .collect();
        
        // 确保有足够节点
        ensure!(
            selected.len() >= count as usize,
            Error::<T>::InsufficientNodes
        );
        
        BoundedVec::try_from(selected)
            .map_err(|_| Error::<T>::TooManyNodes)
    }
    
    /// 函数级详细中文注释：计算节点容量使用率（简化估算）
    /// 
    /// 估算方法：
    /// - 每个PIN平均2MB
    /// - used_capacity_gib = (total_pins × 2MB) / 1024
    /// - capacity_usage = (used_capacity_gib / total_capacity_gib) × 100
    /// 
    /// 参数：
    /// - node: 节点账户
    /// 
    /// 返回：
    /// - 容量使用率（0-100）
    pub fn calculate_simple_capacity_usage(node: &T::AccountId) -> u32 {
        let Some(info) = Operators::<T>::get(node) else {
            return 100; // 节点不存在，视为满载
        };
        
        if info.capacity_gib == 0 {
            return 100; // 容量为0，视为满载
        }
        
        let stats = SimpleNodeStatsMap::<T>::get(node);
        
        // 估算使用容量（每个PIN平均2MB）
        let avg_size_mb: u64 = 2;
        let used_capacity_gib = (stats.total_pins as u64 * avg_size_mb) / 1024;
        let total_capacity_gib = info.capacity_gib as u64;
        
        ((used_capacity_gib * 100) / total_capacity_gib) as u32
    }
    
    /// 函数级详细中文注释：获取分配给本节点的CID列表（OCW使用）
    /// 
    /// OCW在健康检查时调用，获取需要检查的CID列表
    /// 
    /// 参数：
    /// - node: 本节点账户
    /// - limit: 最多返回多少个CID
    /// 
    /// 返回：
    /// - (cid_hash, plaintext_cid) 列表
    pub fn get_my_assigned_cids(
        node: &T::AccountId,
        limit: u32,
    ) -> alloc::vec::Vec<(T::Hash, alloc::vec::Vec<u8>)> {
        let mut result = alloc::vec::Vec::new();
        
        for (cid_hash, assigned_nodes) in SimplePinAssignments::<T>::iter() {
            if assigned_nodes.contains(node) {
                if let Some(cid) = CidRegistry::<T>::get(&cid_hash) {
                    result.push((cid_hash, cid.to_vec()));
                    
                    if result.len() >= limit as usize {
                        break;
                    }
                }
            }
        }
        
        result
    }
    
    /// 函数级详细中文注释：检查IPFS Pin状态（OCW调用）
    /// 
    /// 调用本地IPFS HTTP API检查Pin是否存在：
    /// GET http://127.0.0.1:5001/api/v0/pin/ls?arg=<CID>
    /// 
    /// 参数：
    /// - cid: Plaintext CID
    /// 
    /// 返回：
    /// - Ok(true): Pin存在且健康
    /// - Ok(false): Pin不存在
    /// - Err: HTTP请求失败
    pub fn check_ipfs_pin(cid: &[u8]) -> Result<bool, &'static str> {
        let cid_str = alloc::string::String::from_utf8_lossy(cid);
        let url = alloc::format!("http://127.0.0.1:5001/api/v0/pin/ls?arg={}", cid_str);
        
        let request = http::Request::get(&url);
        let pending = request
            .deadline(sp_io::offchain::timestamp().add(sp_runtime::offchain::Duration::from_millis(5_000)))
            .send()
            .map_err(|_| "HTTP request failed")?;
        
        let response = pending
            .try_wait(sp_io::offchain::timestamp().add(sp_runtime::offchain::Duration::from_millis(5_000)))
            .map_err(|_| "HTTP timeout")?
            .map_err(|_| "HTTP error")?;
        
        Ok(response.code == 200)
    }
    
    /// 函数级详细中文注释：Pin到本地IPFS（OCW调用）
    /// 
    /// 调用本地IPFS HTTP API执行Pin：
    /// POST http://127.0.0.1:5001/api/v0/pin/add?arg=<CID>
    /// 
    /// 参数：
    /// - cid: Plaintext CID
    /// 
    /// 返回：
    /// - Ok(()): Pin成功
    /// - Err: Pin失败
    pub fn pin_to_local_ipfs(cid: &[u8]) -> Result<(), &'static str> {
        let cid_str = alloc::string::String::from_utf8_lossy(cid);
        let url = alloc::format!("http://127.0.0.1:5001/api/v0/pin/add?arg={}", cid_str);
        
        // 使用Vec<Vec<u8>>作为body，满足http::Request的要求
        let chunks: Vec<Vec<u8>> = Vec::new();
        let timeout = sp_io::offchain::timestamp()
            .add(sp_runtime::offchain::Duration::from_millis(30_000));
        
        let request = http::Request::post(&url, chunks);
        let pending = request
            .deadline(timeout)
            .send()
            .map_err(|_| "HTTP request failed")?;
        
        let response = pending
            .try_wait(timeout)
            .map_err(|_| "HTTP timeout")?
            .map_err(|_| "HTTP error")?;
        
        let code: u16 = response.code;
        if (200..300).contains(&code) {
            Ok(())
        } else {
            Err("Pin failed")
        }
    }
    
    /// 函数级详细中文注释：获取本地节点账户（OCW使用）
    /// 
    /// 从OCW本地存储中读取节点账户ID
    /// 
    /// 返回：
    /// - Some(本节点账户) 如果配置存在
    /// - None 如果未配置
    pub fn get_local_node_account() -> Option<T::AccountId> {
        // 从本地存储读取节点账户
        let account_bytes = sp_io::offchain::local_storage_get(
            StorageKind::PERSISTENT,
            b"/memo/ipfs/node_account",
        )?;
        
        T::AccountId::decode(&mut &account_bytes[..]).ok()
    }

    /// 计算运营者保证金金额（100 USDT 等值的 NEX）
    /// 
    /// 使用统一的 DepositCalculator trait 计算
    pub fn calculate_operator_bond() -> BalanceOf<T> {
        use pallet_trading_common::DepositCalculator;
        T::DepositCalculator::calculate_deposit(
            T::MinOperatorBondUsd::get(),
            T::MinOperatorBond::get(),
        )
    }
}
