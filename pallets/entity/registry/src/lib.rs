//! # 实体注册管理模块 (pallet-entity-registry)
//!
//! ## 概述
//!
//! 本模块负责实体的生命周期管理，包括：
//! - 实体创建（转入运营资金到派生账户，付费即激活）
//! - 实体信息更新
//! - 运营资金管理（充值、消费、健康监控）
//! - 实体状态管理（暂停、恢复、申请关闭）
//! - 治理操作（封禁、解禁、暂停、恢复、认证）
//!
//! ## 运营资金机制
//!
//! - 创建实体时转入 50 USDT 等值 NEX 到派生账户
//! - 资金可用于支付 IPFS Pin、存储租金等运营费用
//! - 资金不可提取，仅治理关闭后退还
//! - 低于最低余额时实体自动暂停
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-01-31): 从 pallet-mall 拆分
//! - v0.2.0 (2026-02-01): 实现运营资金派生账户机制
//! - v0.3.0 (2026-02-03): 重构为 Entity，支持多种实体类型和治理模式

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode};
pub use weights::{WeightInfo, SubstrateWeight};

pub mod runtime_api;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

mod helpers;
mod traits;
mod lifecycle;
mod governance;
mod admin;
mod owner_ops;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get},
        BoundedVec, PalletId,
    };
    use sp_runtime::traits::ConstU32;
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode, PricingProvider};
    use pallet_storage_service::StoragePin;

    /// 实体金库派生账户 PalletId
    pub(crate) const ENTITY_PALLET_ID: PalletId = PalletId(*b"et/enty/");

    /// 资金健康状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum FundHealth {
        /// 健康（余额 > 预警阈值）
        Healthy,
        /// 预警（最低余额 < 余额 ≤ 预警阈值）
        Warning,
        /// 危险（余额 ≤ 最低余额，实体暂停）
        Critical,
        /// 耗尽（余额 = 0）
        Depleted,
    }

    /// 运营费用类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum FeeType {
        /// IPFS Pin 费用
        IpfsPin,
        /// 链上存储租金
        StorageRent,
        /// 交易手续费
        TransactionFee,
        /// 推广费用
        Promotion,
        /// 其他费用
        Other,
    }

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// 实体信息（组织层，Entity-Shop 分离架构）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxNameLen, MaxCidLen, MaxAdmins))]
    pub struct Entity<AccountId, BlockNumber, MaxNameLen: Get<u32>, MaxCidLen: Get<u32>, MaxAdmins: Get<u32>> {
        /// 实体 ID
        pub id: u64,
        /// 创建者/所有者账户
        pub owner: AccountId,
        /// 实体名称
        pub name: BoundedVec<u8, MaxNameLen>,
        /// 实体 Logo IPFS CID
        pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 实体描述 IPFS CID
        pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 实体状态
        pub status: EntityStatus,
        /// 创建时间
        pub created_at: BlockNumber,
        // ========== 组织层字段 ==========
        /// 实体类型（默认 Merchant）
        pub entity_type: EntityType,
        /// 管理员列表（所有者之外的管理员，每个管理员绑定权限位掩码）
        pub admins: BoundedVec<(AccountId, u32), MaxAdmins>,
        /// 治理模式（默认 None）
        pub governance_mode: GovernanceMode,
        /// 是否已验证（官方认证）
        pub verified: bool,
        /// 元数据 URI（链下扩展信息）
        pub metadata_uri: Option<BoundedVec<u8, MaxCidLen>>,
        /// 联系方式 IPFS CID（邮箱/电话/社交等结构化数据）
        pub contact_cid: Option<BoundedVec<u8, MaxCidLen>>,
        // ========== Entity-Shop 关联（1:N 多店铺） ==========
        /// Primary Shop ID（0 表示未创建）
        pub primary_shop_id: u64,
    }

    /// 实体类型别名
    pub type EntityOf<T> = Entity<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
        <T as Config>::MaxEntityNameLength,
        <T as Config>::MaxCidLength,
        <T as Config>::MaxAdmins,
    >;

    /// 实体统计
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntityStatistics {
        /// 累计创建实体数（生命周期计数器，只增不减；关闭/封禁不递减）
        pub total_entities: u64,
        /// 活跃实体数
        pub active_entities: u64,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 货币类型
        type Currency: Currency<Self::AccountId>;

        /// 实体名称最大长度
        #[pallet::constant]
        type MaxEntityNameLength: Get<u32>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 治理 Origin
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 定价提供者（用于计算 USDT 等值 NEX 押金）
        type PricingProvider: PricingProvider;

        /// 初始运营资金 USDT 金额（精度 10^6，即 50_000_000 = 50 USDT）
        #[pallet::constant]
        type InitialFundUsdt: Get<u64>;

        /// 最小初始资金 NEX（防止价格过高时资金过低）
        #[pallet::constant]
        type MinInitialFundCos: Get<BalanceOf<Self>>;

        /// 最大初始资金 NEX（防止价格过低时资金过高）
        #[pallet::constant]
        type MaxInitialFundCos: Get<BalanceOf<Self>>;

        /// 最低运营余额（低于此值实体暂停）
        #[pallet::constant]
        type MinOperatingBalance: Get<BalanceOf<Self>>;

        /// 资金预警阈值（低于此值发出预警）
        #[pallet::constant]
        type FundWarningThreshold: Get<BalanceOf<Self>>;

        // ========== Phase 2 新增配置 ==========
        
        /// 最大管理员数量
        #[pallet::constant]
        type MaxAdmins: Get<u32>;

        /// 每个用户最大 Entity 数量
        #[pallet::constant]
        type MaxEntitiesPerUser: Get<u32>;

        // ========== Entity-Shop 分离架构配置 ==========

        /// Shop 模块（用于创建 Primary Shop）
        type ShopProvider: pallet_entity_common::ShopProvider<Self::AccountId>;

        /// 每个 Entity 最大 Shop 数量
        #[pallet::constant]
        type MaxShopsPerEntity: Get<u32>;

        /// 平台账户（没收资金、运营费用的接收方）
        #[pallet::constant]
        type PlatformAccount: Get<Self::AccountId>;

        /// 治理查询提供者（用于全局实体锁定检查）
        type GovernanceProvider: pallet_entity_common::GovernanceProvider;

        /// 关闭申请超时区块数（超时后任何人可触发自动关闭）
        #[pallet::constant]
        type CloseRequestTimeout: Get<BlockNumberFor<Self>>;

        /// 每个推荐人最大推荐实体数量
        #[pallet::constant]
        type MaxReferralsPerReferrer: Get<u32>;

        /// IPFS Pin 管理接口（用于实体元数据 CID 持久化）
        type StoragePin: StoragePin<Self::AccountId>;

        /// Entity 状态变更级联通知（暂停/封禁/关闭/恢复时通知下游模块）
        type OnEntityStatusChange: pallet_entity_common::OnEntityStatusChange;

        /// 订单查询提供者（用于关闭前置检查）
        type OrderProvider: pallet_entity_common::OrderProvider<Self::AccountId, BalanceOf<Self>>;

        /// 代币发售查询提供者（用于关闭前置检查）
        type TokenSaleProvider: pallet_entity_common::TokenSaleProvider<BalanceOf<Self>>;

        /// 争议查询提供者（用于关闭前置检查）
        type DisputeQueryProvider: pallet_entity_common::DisputeQueryProvider<Self::AccountId>;

        /// 市场交易查询提供者（用于关闭前置检查）
        type MarketProvider: pallet_entity_common::MarketProvider<Self::AccountId, BalanceOf<Self>>;

        /// 权重信息（由 benchmark 生成，或使用默认占位值）
        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn integrity_test() {
            assert!(T::MaxEntityNameLength::get() > 0, "MaxEntityNameLength must be > 0");
            assert!(T::MaxCidLength::get() > 0, "MaxCidLength must be > 0");
            assert!(T::MaxAdmins::get() > 0, "MaxAdmins must be > 0");
            assert!(T::MaxEntitiesPerUser::get() > 0, "MaxEntitiesPerUser must be > 0");
            assert!(T::MaxShopsPerEntity::get() > 0, "MaxShopsPerEntity must be > 0");
            assert!(T::MaxReferralsPerReferrer::get() > 0, "MaxReferralsPerReferrer must be > 0");
            assert!(
                T::MinOperatingBalance::get() <= T::FundWarningThreshold::get(),
                "MinOperatingBalance must be <= FundWarningThreshold"
            );
            assert!(
                T::MinInitialFundCos::get() <= T::MaxInitialFundCos::get(),
                "MinInitialFundCos must be <= MaxInitialFundCos"
            );
            // L1 审计修复: 确保 reopen 路径资金校验不会因配置错误而死锁
            assert!(
                T::MinOperatingBalance::get() <= T::MinInitialFundCos::get(),
                "MinOperatingBalance must be <= MinInitialFundCos"
            );
            // L1-R13: 超时为 0 时 execute_close_timeout 可绕过治理审批
            assert!(
                !T::CloseRequestTimeout::get().is_zero(),
                "CloseRequestTimeout must be > 0"
            );
        }

        fn on_runtime_upgrade() -> Weight {
            use frame_support::traits::GetStorageVersion;
            let on_chain = <Pallet<T> as GetStorageVersion>::on_chain_storage_version();

            if on_chain < StorageVersion::new(1) {
                log::info!(
                    "🔄 pallet-entity-registry: migrating from v{:?} to v1",
                    on_chain
                );

                // v0 → v1: 构建 EntityNameIndex（新增 contact_cid 字段默认 None，无需迁移数据）
                let mut count = 0u64;
                for (_, entity) in Entities::<T>::iter() {
                    if !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed) {
                        if let Ok(normalized) = Self::normalize_entity_name(&entity.name) {
                            EntityNameIndex::<T>::insert(&normalized, entity.id);
                            count += 1;
                        }
                    }
                }
                log::info!("🔄 pallet-entity-registry: indexed {} entity names", count);

                StorageVersion::new(1).put::<Pallet<T>>();
                Weight::from_parts(
                    count.saturating_mul(50_000_000).saturating_add(10_000_000),
                    count.saturating_mul(5_000).saturating_add(1_000),
                )
            } else {
                Weight::zero()
            }
        }
    }

    // ==================== 存储项 ====================

    /// Entity ID 起始值（从 1 开始，避免 0 与 primary_shop_id 哨兵值冲突）
    #[pallet::type_value]
    pub fn DefaultNextEntityId() -> u64 { 100_000 }

    /// 下一个 Entity ID
    #[pallet::storage]
    #[pallet::getter(fn next_entity_id)]
    pub type NextEntityId<T> = StorageValue<_, u64, ValueQuery, DefaultNextEntityId>;

    /// Entity 存储 entity_id -> Entity
    #[pallet::storage]
    #[pallet::getter(fn entities)]
    pub type Entities<T: Config> = StorageMap<_, Blake2_128Concat, u64, EntityOf<T>>;

    /// 用户 Entity 索引（支持多实体）
    #[pallet::storage]
    #[pallet::getter(fn user_entities)]
    pub type UserEntity<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, T::MaxEntitiesPerUser>,
        ValueQuery,
    >;

    /// Entity 统计
    #[pallet::storage]
    #[pallet::getter(fn entity_stats)]
    pub type EntityStats<T: Config> = StorageValue<_, EntityStatistics, ValueQuery>;

    /// Entity 关闭申请时间
    #[pallet::storage]
    #[pallet::getter(fn entity_close_requests)]
    pub type EntityCloseRequests<T: Config> = StorageMap<_, Blake2_128Concat, u64, BlockNumberFor<T>>;

    /// 治理暂停标记（区分治理暂停 vs 资金不足暂停，防止 top_up_fund 绕过治理）
    #[pallet::storage]
    pub type GovernanceSuspended<T: Config> = StorageMap<_, Blake2_128Concat, u64, bool, ValueQuery>;

    /// Entity 推荐人 (entity_id → referrer_account)
    #[pallet::storage]
    #[pallet::getter(fn entity_referrer)]
    pub type EntityReferrer<T: Config> = StorageMap<_, Blake2_128Concat, u64, T::AccountId>;

    /// Owner 主动暂停标记（区分 owner 自主暂停 vs 治理暂停 vs 资金不足暂停）
    #[pallet::storage]
    pub type OwnerPaused<T: Config> = StorageMap<_, Blake2_128Concat, u64, bool, ValueQuery>;

    /// 推荐人反向索引（referrer_account → entity_ids）
    #[pallet::storage]
    #[pallet::getter(fn referrer_entities)]
    pub type ReferrerEntities<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, T::MaxReferralsPerReferrer>,
        ValueQuery,
    >;

    /// Entity 销售数据（独立存储，避免每次订单都读写整个 Entity struct）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct EntitySalesData<Balance: Default> {
        /// 累计销售额
        pub total_sales: Balance,
        /// 累计订单数（所有 Shop 汇总）
        pub total_orders: u64,
    }

    /// Entity 销售统计（独立 StorageMap，O(1) 更新无需读写整个 Entity）
    #[pallet::storage]
    #[pallet::getter(fn entity_sales_data)]
    pub type EntitySales<T: Config> = StorageMap<_, Blake2_128Concat, u64, EntitySalesData<BalanceOf<T>>, ValueQuery>;

    /// Entity 关联的所有 Shop IDs（1:N 多店铺）
    #[pallet::storage]
    #[pallet::getter(fn entity_shop_ids)]
    pub type EntityShops<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<u64, T::MaxShopsPerEntity>,
        ValueQuery,
    >;

    /// Entity 名称唯一性索引（normalized_name -> entity_id）
    #[pallet::storage]
    pub type EntityNameIndex<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxEntityNameLength>,
        u64,
    >;

    /// 治理暂停原因（entity_id -> reason）
    #[pallet::storage]
    #[pallet::getter(fn suspension_reason)]
    pub type SuspensionReasons<T: Config> = StorageMap<_, Blake2_128Concat, u64, BoundedVec<u8, ConstU32<256>>>;


    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Entity 已创建
        EntityCreated {
            entity_id: u64,
            owner: T::AccountId,
            treasury_account: T::AccountId,
            initial_fund: BalanceOf<T>,
        },
        /// Shop 已添加到 Entity
        ShopAddedToEntity {
            entity_id: u64,
            shop_id: u64,
        },
        /// 实体已更新
        EntityUpdated { entity_id: u64 },
        /// 实体状态已变更
        EntityStatusChanged { entity_id: u64, status: EntityStatus },
        /// 运营资金已充值
        FundToppedUp {
            entity_id: u64,
            amount: BalanceOf<T>,
            new_balance: BalanceOf<T>,
        },
        /// 运营费用已扣除
        OperatingFeeDeducted {
            entity_id: u64,
            fee: BalanceOf<T>,
            fee_type: FeeType,
            remaining_balance: BalanceOf<T>,
        },
        /// 资金预警
        FundWarning {
            entity_id: u64,
            current_balance: BalanceOf<T>,
            warning_threshold: BalanceOf<T>,
        },
        /// 实体因资金不足暂停
        EntitySuspendedLowFund {
            entity_id: u64,
            current_balance: BalanceOf<T>,
            minimum_balance: BalanceOf<T>,
        },
        /// 充值后实体恢复
        EntityResumedAfterFunding { entity_id: u64 },
        /// 所有者申请关闭实体
        EntityCloseRequested { entity_id: u64 },
        /// 实体已关闭（资金已退还）
        EntityClosed {
            entity_id: u64,
            fund_refunded: BalanceOf<T>,
        },
        /// 实体被封禁
        EntityBanned {
            entity_id: u64,
            fund_confiscated: bool,
            reason: Option<BoundedVec<u8, ConstU32<256>>>,
        },
        /// 资金被没收
        FundConfiscated {
            entity_id: u64,
            amount: BalanceOf<T>,
        },
        // ========== Phase 3 新增事件 ==========
        /// 管理员已添加
        AdminAdded {
            entity_id: u64,
            admin: T::AccountId,
            permissions: u32,
        },
        /// 管理员已移除
        AdminRemoved {
            entity_id: u64,
            admin: T::AccountId,
        },
        /// 管理员权限已更新
        AdminPermissionsUpdated {
            entity_id: u64,
            admin: T::AccountId,
            old_permissions: u32,
            new_permissions: u32,
        },
        /// 实体类型已升级
        EntityTypeUpgraded {
            entity_id: u64,
            old_type: EntityType,
            new_type: EntityType,
        },
        /// 治理模式已变更
        GovernanceModeChanged {
            entity_id: u64,
            old_mode: GovernanceMode,
            new_mode: GovernanceMode,
        },
        /// 实体已验证
        EntityVerified {
            entity_id: u64,
        },
        /// 实体重新开业（Closed → Active，付费即激活）
        EntityReopened {
            entity_id: u64,
            owner: T::AccountId,
            initial_fund: BalanceOf<T>,
        },
        /// 所有权已转移
        OwnershipTransferred {
            entity_id: u64,
            old_owner: T::AccountId,
            new_owner: T::AccountId,
        },
        /// Shop 级联操作失败（需人工干预）
        ShopCascadeFailed {
            entity_id: u64,
            shop_id: u64,
        },
        /// Entity 推荐人已绑定
        EntityReferrerBound {
            entity_id: u64,
            referrer: T::AccountId,
        },
        /// 封禁时资金退还失败（资金滞留在 treasury 账户，需人工干预）
        FundRefundFailed {
            entity_id: u64,
            amount: BalanceOf<T>,
        },
        /// Shop 已从 Entity 移除
        ShopRemovedFromEntity {
            entity_id: u64,
            shop_id: u64,
        },
        /// 实体已解除封禁（Banned → Active，直接激活）
        EntityUnbanned {
            entity_id: u64,
        },
        /// 实体认证已撤销
        EntityUnverified {
            entity_id: u64,
        },
        /// 关闭申请已撤销（PendingClose → Active）
        CloseRequestCancelled {
            entity_id: u64,
        },
        /// 管理员主动辞职
        AdminResigned {
            entity_id: u64,
            admin: T::AccountId,
        },
        // ========== Phase 5 新增事件 ==========
        /// Primary Shop 已变更
        PrimaryShopChanged {
            entity_id: u64,
            old_shop_id: u64,
            new_shop_id: u64,
        },
        /// Owner 主动暂停实体
        EntityOwnerPaused {
            entity_id: u64,
        },
        /// Owner 主动恢复实体
        EntityOwnerResumed {
            entity_id: u64,
        },
        /// 治理强制转移所有权
        OwnershipForceTransferred {
            entity_id: u64,
            old_owner: T::AccountId,
            new_owner: T::AccountId,
        },
        /// 治理拒绝关闭申请
        CloseRequestRejected {
            entity_id: u64,
        },
        // ========== Phase 6 新增事件 ==========
        /// 治理暂停实体（附原因）
        EntitySuspendedWithReason {
            entity_id: u64,
            reason: BoundedVec<u8, ConstU32<256>>,
        },
        /// 关闭申请超时自动执行
        CloseRequestAutoExecuted {
            entity_id: u64,
            fund_refunded: BalanceOf<T>,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 实体不存在
        EntityNotFound,
        /// 用户实体数量已达上限
        MaxEntitiesReached,
        /// 不是实体所有者
        NotEntityOwner,
        /// 运营资金不足
        InsufficientOperatingFund,
        /// 无效的实体状态
        InvalidEntityStatus,
        /// 名称为空
        NameEmpty,
        /// 名称过长
        NameTooLong,
        /// 名称内容无效（非 UTF-8 或含控制字符）
        InvalidName,
        /// CID 过长
        CidTooLong,
        /// 价格不可用
        PriceUnavailable,
        /// 算术溢出
        ArithmeticOverflow,
        /// 余额不足以支付初始资金
        InsufficientBalanceForInitialFund,
        // ========== Phase 3 新增错误 ==========
        /// 不是管理员
        NotAdmin,
        /// 管理员已存在
        AdminAlreadyExists,
        /// 管理员不存在
        AdminNotFound,
        /// 管理员数量已达上限
        MaxAdminsReached,
        /// 不能移除所有者
        CannotRemoveOwner,
        /// DAO 类型需要治理模式
        DAORequiresGovernance,
        /// 无效的实体类型升级
        InvalidEntityTypeUpgrade,
        // ========== Entity-Shop 分离架构错误 ==========
        /// Entity Shop 数量已达上限
        ShopLimitReached,
        /// Shop 未注册在此 Entity
        ShopNotRegistered,
        /// 充值金额为零
        ZeroAmount,
        /// 实体已验证
        AlreadyVerified,
        /// Shop 已注册在此 Entity
        ShopAlreadyRegistered,
        /// 实体状态不允许此操作（已关闭或已封禁）
        EntityNotActive,
        /// 类型未变化
        SameEntityType,
        /// 推荐人已绑定（不可更改）
        ReferrerAlreadyBound,
        /// 无效推荐人（推荐人未拥有 Active Entity）
        InvalidReferrer,
        /// 不能推荐自己
        SelfReferral,
        /// 不能转移给自己
        SameOwner,
        /// 无效权限值（不可为 0）
        InvalidPermissions,
        /// 实体未认证（无法撤销）
        NotVerified,
        /// 调用者不是此实体的管理员
        NotAdminCaller,
        // ========== Phase 5 新增错误 ==========
        /// Shop 不属于此 Entity
        ShopNotInEntity,
        /// 已是当前 Primary Shop
        AlreadyPrimaryShop,
        /// Entity 已被 Owner 暂停
        AlreadyOwnerPaused,
        /// Entity 未被 Owner 暂停
        NotOwnerPaused,
        /// 推荐人反向索引已满
        ReferrerIndexFull,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        // ========== Phase 6 新增错误 ==========
        /// 名称已被其他实体使用
        NameAlreadyTaken,
        /// 关闭申请尚未超时
        CloseRequestNotExpired,
        /// 存在活跃治理提案，不允许关闭
        HasActiveProposals,
        /// 存在未完成订单，不允许关闭
        HasActiveOrders,
        /// 存在活跃争议，不允许关闭
        HasActiveDisputes,
        /// 存在活跃代币发售，不允许关闭
        HasActiveTokenSale,
        /// 存在活跃市场交易，不允许关闭
        HasActiveMarket,
    }

    // ==================== Extrinsics ====================
    // 逻辑委托到 lifecycle.rs, governance.rs, admin.rs, owner_ops.rs

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建 Entity（组织身份）— 付费即激活
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_entity())]
        pub fn create_entity(
            origin: OriginFor<T>,
            name: Vec<u8>,
            logo_cid: Option<Vec<u8>>,
            description_cid: Option<Vec<u8>>,
            referrer: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_create_entity(who, name, logo_cid, description_cid, referrer)
        }

        /// 更新实体信息
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_entity())]
        pub fn update_entity(
            origin: OriginFor<T>,
            entity_id: u64,
            name: Option<Vec<u8>>,
            logo_cid: Option<Vec<u8>>,
            description_cid: Option<Vec<u8>>,
            metadata_uri: Option<Vec<u8>>,
            contact_cid: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_update_entity(who, entity_id, name, logo_cid, description_cid, metadata_uri, contact_cid)
        }

        /// 申请关闭实体（超时后自动关闭）
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::request_close_entity())]
        pub fn request_close_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_request_close_entity(who, entity_id)
        }

        /// 充值金库资金
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::top_up_fund())]
        pub fn top_up_fund(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_top_up_fund(who, entity_id, amount)
        }

        // call_index(4) 已移除: approve_entity（付费即激活，reopen/unban 直接 Active）
        // call_index(5) 已移除: approve_close_entity（关闭统一走超时机制 execute_close_timeout）

        /// 暂停实体（治理，可附原因）
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::suspend_entity())]
        pub fn suspend_entity(
            origin: OriginFor<T>,
            entity_id: u64,
            reason: Option<Vec<u8>>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_suspend_entity(entity_id, reason)
        }

        /// 恢复实体（治理，需资金充足）
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::resume_entity())]
        pub fn resume_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_resume_entity(entity_id)
        }

        /// 封禁实体（治理，可选没收资金，可附原因）
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::ban_entity())]
        pub fn ban_entity(
            origin: OriginFor<T>,
            entity_id: u64,
            confiscate_fund: bool,
            reason: Option<Vec<u8>>,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_ban_entity(entity_id, confiscate_fund, reason)
        }

        /// 添加管理员（指定权限位掩码）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::add_admin())]
        pub fn add_admin(
            origin: OriginFor<T>,
            entity_id: u64,
            new_admin: T::AccountId,
            permissions: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_add_admin(who, entity_id, new_admin, permissions)
        }

        /// 移除管理员
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::remove_admin())]
        pub fn remove_admin(
            origin: OriginFor<T>,
            entity_id: u64,
            admin: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_remove_admin(who, entity_id, admin)
        }

        /// 转移所有权
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::transfer_ownership())]
        pub fn transfer_ownership(
            origin: OriginFor<T>,
            entity_id: u64,
            new_owner: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_transfer_ownership(who, entity_id, new_owner)
        }

        /// 升级实体类型（需治理批准或满足条件）
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::upgrade_entity_type())]
        pub fn upgrade_entity_type(
            origin: OriginFor<T>,
            entity_id: u64,
            new_type: EntityType,
            new_governance: GovernanceMode,
        ) -> DispatchResult {
            // 治理或所有者可以升级
            let is_governance = T::GovernanceOrigin::ensure_origin(origin.clone()).is_ok();
            let who = if !is_governance {
                Some(ensure_signed(origin)?)
            } else {
                None
            };
            Self::do_upgrade_entity_type(who, entity_id, new_type, new_governance)
        }

        // call_index(13) 已移除: change_governance_mode 死代码
        // 治理模式变更统一由 pallet-entity-governance::configure_governance 管理

        /// 验证实体（治理）
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::verify_entity())]
        pub fn verify_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_verify_entity(entity_id)
        }

        /// 重新开业（owner 申请，Closed → Active，需重新缴纳押金，付费即激活）
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::reopen_entity())]
        pub fn reopen_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_reopen_entity(who, entity_id)
        }

        /// 补绑 Entity 推荐人（仅限创建时未填的，一次性操作）
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::bind_entity_referrer())]
        pub fn bind_entity_referrer(
            origin: OriginFor<T>,
            entity_id: u64,
            referrer: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_bind_entity_referrer(who, entity_id, referrer)
        }

        /// 更新管理员权限
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::update_admin_permissions())]
        pub fn update_admin_permissions(
            origin: OriginFor<T>,
            entity_id: u64,
            admin: T::AccountId,
            new_permissions: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_update_admin_permissions(who, entity_id, admin, new_permissions)
        }

        /// 解除封禁（治理，Banned → Active，需资金充足）
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::unban_entity())]
        pub fn unban_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_unban_entity(entity_id)
        }

        /// 撤销认证（治理）
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::unverify_entity())]
        pub fn unverify_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_unverify_entity(entity_id)
        }

        /// 撤销关闭申请（Owner，PendingClose → Active/Suspended）
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::cancel_close_request())]
        pub fn cancel_close_request(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_cancel_close_request(who, entity_id)
        }

        /// 管理员主动辞职
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::resign_admin())]
        pub fn resign_admin(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_resign_admin(who, entity_id)
        }

        /// 设置 Primary Shop（owner 或 ENTITY_MANAGE admin）
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::set_primary_shop())]
        pub fn set_primary_shop(
            origin: OriginFor<T>,
            entity_id: u64,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_set_primary_shop(who, entity_id, shop_id)
        }

        /// Owner 主动暂停实体
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::self_pause_entity())]
        pub fn self_pause_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_self_pause_entity(who, entity_id)
        }

        /// Owner 恢复主动暂停的实体
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::self_resume_entity())]
        pub fn self_resume_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_self_resume_entity(who, entity_id)
        }

        /// 治理强制转移所有权
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::force_transfer_ownership())]
        pub fn force_transfer_ownership(
            origin: OriginFor<T>,
            entity_id: u64,
            new_owner: T::AccountId,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_force_transfer_ownership(entity_id, new_owner)
        }

        /// 治理拒绝关闭申请（PendingClose → Active/Suspended）
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::reject_close_request())]
        pub fn reject_close_request(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_reject_close_request(entity_id)
        }

        /// 执行超时关闭申请（任何人可调用）
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::execute_close_timeout())]
        pub fn execute_close_timeout(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            ensure_signed(origin)?;
            Self::do_execute_close_timeout(entity_id)
        }
    }
}

