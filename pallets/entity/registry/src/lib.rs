//! # 实体注册管理模块 (pallet-entity-registry)
//!
//! ## 概述
//!
//! 本模块负责实体的生命周期管理，包括：
//! - 实体创建（转入运营资金到派生账户）
//! - 实体信息更新
//! - 运营资金管理（充值、消费、健康监控）
//! - 实体状态管理（暂停、恢复、申请关闭）
//! - 治理审核（批准、封禁、审批关闭）
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
        traits::{Currency, ExistenceRequirement, Get},
        BoundedVec, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode, PricingProvider, EntityProvider, ShopProvider, ShopType, MemberMode};
    use sp_runtime::{
        traits::{AccountIdConversion, Saturating, Zero},
        SaturatedConversion,
    };

    /// 实体金库派生账户 PalletId
    const ENTITY_PALLET_ID: PalletId = PalletId(*b"et/enty/");

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
    pub struct Entity<AccountId, Balance, BlockNumber, MaxNameLen: Get<u32>, MaxCidLen: Get<u32>, MaxAdmins: Get<u32>> {
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
        /// 管理员列表（所有者之外的管理员）
        pub admins: BoundedVec<AccountId, MaxAdmins>,
        /// 治理模式（默认 None）
        pub governance_mode: GovernanceMode,
        /// 是否已验证（官方认证）
        pub verified: bool,
        /// 元数据 URI（链下扩展信息）
        pub metadata_uri: Option<BoundedVec<u8, MaxCidLen>>,
        // ========== Entity-Shop 关联（1:N 多店铺） ==========
        /// Primary Shop ID（0 表示未创建）
        pub primary_shop_id: u64,
        // ========== 汇总统计 ===========
        /// 累计销售额
        pub total_sales: Balance,
        /// 累计订单数（所有 Shop 汇总）
        pub total_orders: u64,
    }

    /// 实体类型别名
    pub type EntityOf<T> = Entity<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
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
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// Entity ID 起始值（从 1 开始，避免 0 与 primary_shop_id 哨兵值冲突）
    #[pallet::type_value]
    pub fn DefaultNextEntityId() -> u64 { 1 }

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

    /// 账户招商统计 (account → 推荐的 entity 数量)
    #[pallet::storage]
    #[pallet::getter(fn entity_referral_count)]
    pub type EntityReferralCount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

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
        },
        /// 管理员已移除
        AdminRemoved {
            entity_id: u64,
            admin: T::AccountId,
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
        /// 实体重新开业申请（Closed → Pending，等待治理审批）
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
        /// 治理模式未变化
        SameGovernanceMode,
        /// 推荐人已绑定（不可更改）
        ReferrerAlreadyBound,
        /// 无效推荐人（推荐人未拥有 Active Entity）
        InvalidReferrer,
        /// 不能推荐自己
        SelfReferral,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建 Entity（组织身份）
        /// 
        /// # 激活模型：付费即激活（Pay-to-Activate）
        /// 
        /// 新建 Entity 直接进入 `Active` 状态，不经过 `Pending` 审批。
        /// 信任锚点是 50 USDT 等值 NEX 押金，构成经济层面的 Sybil 防护。
        /// 
        /// `approve_entity`（call_index 4）**仅用于** `reopen_entity` 后的治理审批，
        /// 因为重开的 Entity 有历史关闭/封禁记录，需要额外的治理信任层。
        /// 
        /// 若未来需要新建也走审批流程，需修改：
        /// 1. 初始状态改为 `EntityStatus::Pending`
        /// 2. `active_entities` 统计延后到 `approve_entity` 中递增
        /// 3. Primary Shop 创建时机延后到 `approve_entity`（避免 Pending 期间产生可运营的 Shop）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn create_entity(
            origin: OriginFor<T>,
            name: Vec<u8>,
            logo_cid: Option<Vec<u8>>,
            description_cid: Option<Vec<u8>>,
            referrer: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 检查用户实体数量是否达到上限
            let user_entities = UserEntity::<T>::get(&who);
            ensure!(
                (user_entities.len() as u32) < T::MaxEntitiesPerUser::get(),
                Error::<T>::MaxEntitiesReached
            );

            ensure!(!name.is_empty(), Error::<T>::NameEmpty);
            // H2: 名称内容校验（UTF-8 + 无控制字符 + 非全空白）
            let name_str = core::str::from_utf8(&name).map_err(|_| Error::<T>::InvalidName)?;
            ensure!(!name_str.trim().is_empty(), Error::<T>::NameEmpty);
            ensure!(!name_str.chars().any(|c| c.is_control()), Error::<T>::InvalidName);
            let name: BoundedVec<u8, T::MaxEntityNameLength> =
                name.try_into().map_err(|_| Error::<T>::NameTooLong)?;

            // H1: 空 CID 转为 None
            let logo_cid: Option<BoundedVec<u8, T::MaxCidLength>> = logo_cid
                .filter(|c| !c.is_empty())
                .map(|c| c.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;
            let description_cid: Option<BoundedVec<u8, T::MaxCidLength>> = description_cid
                .filter(|c| !c.is_empty())
                .map(|c| c.try_into().map_err(|_| Error::<T>::CidTooLong))
                .transpose()?;

            // 验证推荐人
            if let Some(ref ref_account) = referrer {
                ensure!(ref_account != &who, Error::<T>::SelfReferral);
                ensure!(Self::has_active_entity(ref_account), Error::<T>::InvalidReferrer);
            }

            // 计算初始金库资金（50 USDT 等值 NEX）
            let initial_fund = Self::calculate_initial_fund()?;
            
            // 生成 Entity ID 和金库账户
            let entity_id = NextEntityId::<T>::get();
            let treasury_account = Self::entity_treasury_account(entity_id);
            
            // 转入金库账户
            T::Currency::transfer(
                &who,
                &treasury_account,
                initial_fund,
                ExistenceRequirement::KeepAlive,
            ).map_err(|_| Error::<T>::InsufficientBalanceForInitialFund)?;

            let now = <frame_system::Pallet<T>>::block_number();

            // 创建 Entity（Shop 列表初始为空，用户按需创建）
            let entity = Entity {
                id: entity_id,
                owner: who.clone(),
                name,
                logo_cid,
                description_cid,
                status: EntityStatus::Active,
                created_at: now,
                entity_type: EntityType::Merchant,
                admins: BoundedVec::default(),
                governance_mode: GovernanceMode::None,
                verified: false,
                metadata_uri: None,
                primary_shop_id: 0,
                total_sales: Zero::zero(),
                total_orders: 0,
            };

            Entities::<T>::insert(entity_id, &entity);
            UserEntity::<T>::try_mutate(&who, |entities| {
                entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)
            })?;
            NextEntityId::<T>::put(entity_id.saturating_add(1));

            // 自动创建 Primary Shop（继承 Entity 名称，默认线上商城 + Inherit 模式）
            let _primary_shop_id = T::ShopProvider::create_primary_shop(
                entity_id,
                entity.name.to_vec(),
                ShopType::OnlineStore,
                MemberMode::Inherit,
            )?;

            // 更新统计（付费即激活，无需审批）
            EntityStats::<T>::mutate(|stats| {
                stats.total_entities = stats.total_entities.saturating_add(1);
                stats.active_entities = stats.active_entities.saturating_add(1);
            });

            // 记录推荐人（如果有）
            if let Some(ref ref_account) = referrer {
                EntityReferrer::<T>::insert(entity_id, ref_account);
                EntityReferralCount::<T>::mutate(ref_account, |count| {
                    *count = count.saturating_add(1);
                });
                Self::deposit_event(Event::EntityReferrerBound {
                    entity_id,
                    referrer: ref_account.clone(),
                });
            }

            Self::deposit_event(Event::EntityCreated {
                entity_id,
                owner: who,
                treasury_account,
                initial_fund,
            });

            Ok(())
        }

        /// 更新实体信息
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
        pub fn update_entity(
            origin: OriginFor<T>,
            entity_id: u64,
            name: Option<Vec<u8>>,
            logo_cid: Option<Vec<u8>>,
            description_cid: Option<Vec<u8>>,
            metadata_uri: Option<Vec<u8>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Entities::<T>::try_mutate(entity_id, |maybe_entity| -> DispatchResult {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::InvalidEntityStatus
                );

                // H2: 名称更新时校验内容
                if let Some(n) = name {
                    ensure!(!n.is_empty(), Error::<T>::NameEmpty);
                    let n_str = core::str::from_utf8(&n).map_err(|_| Error::<T>::InvalidName)?;
                    ensure!(!n_str.trim().is_empty(), Error::<T>::NameEmpty);
                    ensure!(!n_str.chars().any(|c| c.is_control()), Error::<T>::InvalidName);
                    entity.name = n.try_into().map_err(|_| Error::<T>::NameTooLong)?;
                }
                // H1: 空 CID 转为 None
                if let Some(c) = logo_cid {
                    entity.logo_cid = if c.is_empty() { None } else { Some(c.try_into().map_err(|_| Error::<T>::CidTooLong)?) };
                }
                if let Some(c) = description_cid {
                    entity.description_cid = if c.is_empty() { None } else { Some(c.try_into().map_err(|_| Error::<T>::CidTooLong)?) };
                }
                // M2: 支持更新 metadata_uri
                if let Some(uri) = metadata_uri {
                    entity.metadata_uri = Some(uri.try_into().map_err(|_| Error::<T>::CidTooLong)?);
                }

                Ok(())
            })?;

            Self::deposit_event(Event::EntityUpdated { entity_id });
            Ok(())
        }

        /// 申请关闭实体（需治理审批）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
        pub fn request_close_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let was_active = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<bool, sp_runtime::DispatchError> {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
                ensure!(
                    entity.status == EntityStatus::Active || entity.status == EntityStatus::Suspended,
                    Error::<T>::InvalidEntityStatus
                );

                let was_active = entity.status == EntityStatus::Active;
                entity.status = EntityStatus::PendingClose;
                Ok(was_active)
            })?;

            // 记录申请时间
            let now = <frame_system::Pallet<T>>::block_number();
            EntityCloseRequests::<T>::insert(entity_id, now);

            // 修正统计：Active → PendingClose 时递减
            if was_active {
                EntityStats::<T>::mutate(|stats| {
                    stats.active_entities = stats.active_entities.saturating_sub(1);
                });
            }

            Self::deposit_event(Event::EntityCloseRequested { entity_id });
            Ok(())
        }

        /// 充值金库资金
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(100_000_000, 5_000))]
        pub fn top_up_fund(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(
                entity.status != EntityStatus::Closed && entity.status != EntityStatus::Banned,
                Error::<T>::InvalidEntityStatus
            );
            // M1: 充值金额不能为零
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

            let treasury_account = Self::entity_treasury_account(entity_id);

            T::Currency::transfer(
                &who,
                &treasury_account,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            let new_balance = T::Currency::free_balance(&treasury_account);
            let min_balance = T::MinOperatingBalance::get();

            // 仅资金不足暂停可自动恢复；治理暂停需要 resume_entity 显式恢复
            if entity.status == EntityStatus::Suspended
                && new_balance >= min_balance
                && !GovernanceSuspended::<T>::get(entity_id)
            {
                Entities::<T>::mutate(entity_id, |s| {
                    if let Some(e) = s {
                        e.status = EntityStatus::Active;
                    }
                });
                EntityStats::<T>::mutate(|stats| {
                    stats.active_entities = stats.active_entities.saturating_add(1);
                });
                Self::deposit_event(Event::EntityResumedAfterFunding { entity_id });
            }

            Self::deposit_event(Event::FundToppedUp {
                entity_id,
                amount,
                new_balance,
            });

            Ok(())
        }

        /// 审核通过实体（治理：激活 Pending 状态的实体）
        /// 
        /// # 唯一触发路径
        /// 
        /// 当前仅 `reopen_entity`（call_index 15）会产生 `Pending` 状态。
        /// `create_entity` 付费即激活，直接进入 `Active`，不经过本函数。
        /// 
        /// 本函数同时恢复 Shop：因 `reopen_entity` 之前的 close/ban 已
        /// 通过 `force_close_shop` 物理写入终态，需显式 `resume_shop` 恢复。
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
        pub fn approve_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::Pending, Error::<T>::InvalidEntityStatus);

            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Active;
                }
            });

            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_add(1);
            });

            // reopen_entity 恢复路径：Shop 之前被 force_close（终态写入），
            // 需要显式恢复为 Active（遍历所有关联 Shop）
            for sid in EntityShops::<T>::get(entity_id).iter() {
                if T::ShopProvider::resume_shop(*sid).is_err() {
                    Self::deposit_event(Event::ShopCascadeFailed { entity_id, shop_id: *sid });
                }
            }

            Self::deposit_event(Event::EntityStatusChanged {
                entity_id,
                status: EntityStatus::Active,
            });
            Ok(())
        }

        /// 审批关闭实体（治理，退还全部余额）
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn approve_close_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::PendingClose, Error::<T>::InvalidEntityStatus);

            let treasury_account = Self::entity_treasury_account(entity_id);
            let balance = T::Currency::free_balance(&treasury_account);

            // H3 修复: 退还余额给所有者，失败不阻止关闭流程
            let fund_refunded = if !balance.is_zero() {
                match T::Currency::transfer(
                    &treasury_account,
                    &entity.owner,
                    balance,
                    ExistenceRequirement::AllowDeath,
                ) {
                    Ok(_) => balance,
                    Err(_) => Zero::zero(),
                }
            } else {
                Zero::zero()
            };

            // 更新状态
            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Closed;
                }
            });

            EntityCloseRequests::<T>::remove(entity_id);

            // 清理用户实体索引
            UserEntity::<T>::mutate(&entity.owner, |entities| {
                entities.retain(|&id| id != entity_id);
            });

            // 注：active_entities 已在 request_close_entity 中递减，此处无需重复

            // 级联关闭所有 Shop（绕过 is_primary 保护）
            for sid in EntityShops::<T>::get(entity_id).iter() {
                if T::ShopProvider::force_close_shop(*sid).is_err() {
                    Self::deposit_event(Event::ShopCascadeFailed { entity_id, shop_id: *sid });
                }
            }

            Self::deposit_event(Event::EntityClosed {
                entity_id,
                fund_refunded,
            });
            Ok(())
        }

        /// 暂停实体（治理）
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
        pub fn suspend_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::Active, Error::<T>::InvalidEntityStatus);

            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Suspended;
                }
            });

            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_sub(1);
            });

            // 标记为治理暂停（防止 top_up_fund 绕过治理自动恢复）
            GovernanceSuspended::<T>::insert(entity_id, true);

            // 不再级联写入 Shop — is_shop_active() 会自动检查 Entity 状态

            Self::deposit_event(Event::EntityStatusChanged {
                entity_id,
                status: EntityStatus::Suspended,
            });
            Ok(())
        }

        /// 恢复实体（治理，需资金充足）
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
        pub fn resume_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            let treasury_account = Self::entity_treasury_account(entity_id);
            let balance = T::Currency::free_balance(&treasury_account);
            let min_balance = T::MinOperatingBalance::get();
            ensure!(balance >= min_balance, Error::<T>::InsufficientOperatingFund);

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::Suspended, Error::<T>::InvalidEntityStatus);

            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Active;
                }
            });

            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_add(1);
            });

            // 清除治理暂停标记
            GovernanceSuspended::<T>::remove(entity_id);

            // 不再级联写入 Shop — Entity Active 后，Shop 恢复其原有独立状态

            Self::deposit_event(Event::EntityStatusChanged {
                entity_id,
                status: EntityStatus::Active,
            });
            Ok(())
        }

        /// 封禁实体（治理，可选没收资金）
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn ban_entity(
            origin: OriginFor<T>,
            entity_id: u64,
            confiscate_fund: bool,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            // 仅允许 ban Active/Suspended/PendingClose 实体
            ensure!(
                matches!(entity.status, EntityStatus::Active | EntityStatus::Suspended | EntityStatus::PendingClose),
                Error::<T>::InvalidEntityStatus
            );

            let treasury_account = Self::entity_treasury_account(entity_id);
            let balance = T::Currency::free_balance(&treasury_account);

            if !balance.is_zero() {
                if confiscate_fund {
                    // H1: 没收资金转入平台账户，仅成功时发射事件
                    if T::Currency::transfer(
                        &treasury_account,
                        &T::PlatformAccount::get(),
                        balance,
                        ExistenceRequirement::AllowDeath,
                    ).is_ok() {
                        Self::deposit_event(Event::FundConfiscated { entity_id, amount: balance });
                    }
                } else {
                    // H2 修复: 退还失败不应阻止封禁（owner 账户可能已被 reaped）
                    let _ = T::Currency::transfer(
                        &treasury_account,
                        &entity.owner,
                        balance,
                        ExistenceRequirement::AllowDeath,
                    );
                }
            }

            Entities::<T>::mutate(entity_id, |maybe| {
                if let Some(e) = maybe {
                    e.status = EntityStatus::Banned;
                }
            });

            // 清理用户实体索引
            UserEntity::<T>::mutate(&entity.owner, |entities| {
                entities.retain(|&id| id != entity_id);
            });

            // 清理 PendingClose 关闭申请（若有）
            EntityCloseRequests::<T>::remove(entity_id);

            // H3 修复: 清除治理暂停标记（防止残留影响后续 reopen 流程）
            GovernanceSuspended::<T>::remove(entity_id);

            if entity.status == EntityStatus::Active {
                EntityStats::<T>::mutate(|stats| {
                    stats.active_entities = stats.active_entities.saturating_sub(1);
                });
            }

            // 级联关闭所有 Shop（绕过 is_primary 保护）
            for sid in EntityShops::<T>::get(entity_id).iter() {
                if T::ShopProvider::force_close_shop(*sid).is_err() {
                    Self::deposit_event(Event::ShopCascadeFailed { entity_id, shop_id: *sid });
                }
            }

            Self::deposit_event(Event::EntityBanned {
                entity_id,
                fund_confiscated: confiscate_fund,
            });
            Ok(())
        }

        // ==================== Phase 3 新增 Extrinsics ====================

        /// 添加管理员
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(60_000_000, 4_000))]
        pub fn add_admin(
            origin: OriginFor<T>,
            entity_id: u64,
            new_admin: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Entities::<T>::try_mutate(entity_id, |maybe_entity| -> DispatchResult {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                
                // 只有所有者可以添加管理员
                ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
                // H2: 不允许对 Banned/Closed 实体操作
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::InvalidEntityStatus
                );
                
                // 检查是否已是管理员
                ensure!(!entity.admins.contains(&new_admin), Error::<T>::AdminAlreadyExists);
                ensure!(new_admin != entity.owner, Error::<T>::AdminAlreadyExists);
                
                // 添加管理员
                entity.admins.try_push(new_admin.clone())
                    .map_err(|_| Error::<T>::MaxAdminsReached)?;
                
                Ok(())
            })?;

            Self::deposit_event(Event::AdminAdded {
                entity_id,
                admin: new_admin,
            });
            Ok(())
        }

        /// 移除管理员
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(60_000_000, 4_000))]
        pub fn remove_admin(
            origin: OriginFor<T>,
            entity_id: u64,
            admin: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Entities::<T>::try_mutate(entity_id, |maybe_entity| -> DispatchResult {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                
                // 只有所有者可以移除管理员
                ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
                // H2: 不允许对 Banned/Closed 实体操作
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::InvalidEntityStatus
                );
                
                // 不能移除所有者
                ensure!(admin != entity.owner, Error::<T>::CannotRemoveOwner);
                
                // 查找并移除
                let pos = entity.admins.iter().position(|a| a == &admin)
                    .ok_or(Error::<T>::AdminNotFound)?;
                entity.admins.remove(pos);
                
                Ok(())
            })?;

            Self::deposit_event(Event::AdminRemoved {
                entity_id,
                admin,
            });
            Ok(())
        }

        /// 转移所有权
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
        pub fn transfer_ownership(
            origin: OriginFor<T>,
            entity_id: u64,
            new_owner: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H2 修复: 先验证新 owner 容量，避免 Entity 变更后 UserEntity 写入失败导致不一致
            let new_owner_entities = UserEntity::<T>::get(&new_owner);
            ensure!(
                (new_owner_entities.len() as u32) < T::MaxEntitiesPerUser::get(),
                Error::<T>::MaxEntitiesReached
            );
            
            let old_owner = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<T::AccountId, DispatchError> {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                
                // 只有所有者可以转移所有权
                ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
                // H3: 不允许对 Banned/Closed 实体转移所有权
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::InvalidEntityStatus
                );
                
                let old = entity.owner.clone();
                entity.owner = new_owner.clone();
                
                // 如果新所有者在管理员列表中，移除
                if let Some(pos) = entity.admins.iter().position(|a| a == &new_owner) {
                    entity.admins.remove(pos);
                }
                
                Ok(old)
            })?;

            // 更新用户实体索引（容量已预先验证，try_push 不会失败）
            UserEntity::<T>::mutate(&old_owner, |entities| {
                entities.retain(|&id| id != entity_id);
            });
            UserEntity::<T>::try_mutate(&new_owner, |entities| {
                entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)
            })?;

            Self::deposit_event(Event::OwnershipTransferred {
                entity_id,
                old_owner,
                new_owner,
            });
            Ok(())
        }

        /// 升级实体类型（需治理批准或满足条件）
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
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
            
            let (old_type, old_mode) = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<(EntityType, GovernanceMode), DispatchError> {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::InvalidEntityStatus
                );
                
                // M2 修复: 同类型且同治理模式不需要升级
                ensure!(
                    entity.entity_type != new_type || entity.governance_mode != new_governance,
                    Error::<T>::SameEntityType
                );

                // 非治理操作需要是所有者，且受升级路径限制
                if let Some(ref caller) = who {
                    ensure!(entity.owner == *caller, Error::<T>::NotEntityOwner);
                    Self::validate_entity_type_upgrade(&entity.entity_type, &new_type)?;
                }
                // 治理 origin 可任意升级类型，不受路径限制
                
                // DAO 类型需要治理模式
                if new_type == EntityType::DAO {
                    ensure!(new_governance != GovernanceMode::None, Error::<T>::DAORequiresGovernance);
                }
                
                let old_t = entity.entity_type;
                let old_g = entity.governance_mode;
                entity.entity_type = new_type;
                entity.governance_mode = new_governance;
                
                Ok((old_t, old_g))
            })?;

            Self::deposit_event(Event::EntityTypeUpgraded {
                entity_id,
                old_type,
                new_type,
            });
            
            if old_mode != new_governance {
                Self::deposit_event(Event::GovernanceModeChanged {
                    entity_id,
                    old_mode,
                    new_mode: new_governance,
                });
            }
            
            Ok(())
        }

        // call_index(13) 已移除: change_governance_mode 死代码
        // 治理模式变更统一由 pallet-entity-governance::configure_governance 管理

        /// 验证实体（治理）
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(60_000_000, 4_000))]
        pub fn verify_entity(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            Entities::<T>::try_mutate(entity_id, |maybe_entity| -> DispatchResult {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::InvalidEntityStatus
                );
                // L3: 幂等检查 — 已验证时直接返回，避免无效写入
                ensure!(!entity.verified, Error::<T>::AlreadyVerified);
                entity.verified = true;
                Ok(())
            })?;

            Self::deposit_event(Event::EntityVerified { entity_id });
            Ok(())
        }

        /// 重新开业（owner 申请，Closed → Pending，需重新缴纳押金，等待治理审批）
        ///
        /// **注意**: 仅允许 `Closed` 状态的实体重开。`Banned` 状态的实体不可重开，
        /// 需治理通过其他流程（如链上提案）解除封禁后才能操作。
        #[pallet::call_index(15)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn reopen_entity(origin: OriginFor<T>, entity_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体存在且处于 Closed 状态
            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(entity.status == EntityStatus::Closed, Error::<T>::InvalidEntityStatus);

            // 重新计算并缴纳押金（50 USDT 等值 NEX）
            let initial_fund_amount = Self::calculate_initial_fund()?;

            let treasury_account = Self::entity_treasury_account(entity_id);
            T::Currency::transfer(
                &who,
                &treasury_account,
                initial_fund_amount,
                ExistenceRequirement::KeepAlive,
            )?;

            // 更新状态为 Pending，等待治理审批
            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Pending;
                }
            });

            // 恢复 UserEntity 索引（防御性去重：避免异常路径导致重复索引）
            UserEntity::<T>::try_mutate(&who, |entities| -> DispatchResult {
                if !entities.contains(&entity_id) {
                    entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)?;
                }
                Ok(())
            })?;

            Self::deposit_event(Event::EntityReopened {
                entity_id,
                owner: who,
                initial_fund: initial_fund_amount,
            });

            Ok(())
        }

        /// 补绑 Entity 推荐人（仅限创建时未填的，一次性操作）
        ///
        /// # 规则
        /// - 仅 Entity owner 可操作
        /// - Entity 必须 Active
        /// - 尚未绑定推荐人
        /// - 推荐人必须拥有 Active Entity
        /// - 不能推荐自己
        #[pallet::call_index(16)]
        #[pallet::weight(Weight::from_parts(80_000_000, 5_000))]
        pub fn bind_entity_referrer(
            origin: OriginFor<T>,
            entity_id: u64,
            referrer: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(entity.status == EntityStatus::Active, Error::<T>::EntityNotActive);

            // 不能已有推荐人
            ensure!(!EntityReferrer::<T>::contains_key(entity_id), Error::<T>::ReferrerAlreadyBound);

            // 验证推荐人
            ensure!(referrer != who, Error::<T>::SelfReferral);
            ensure!(Self::has_active_entity(&referrer), Error::<T>::InvalidReferrer);

            // 写入
            EntityReferrer::<T>::insert(entity_id, &referrer);
            EntityReferralCount::<T>::mutate(&referrer, |count| {
                *count = count.saturating_add(1);
            });

            Self::deposit_event(Event::EntityReferrerBound {
                entity_id,
                referrer,
            });

            Ok(())
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        /// 获取 Entity 金库派生账户
        pub fn entity_treasury_account(entity_id: u64) -> T::AccountId {
            ENTITY_PALLET_ID.into_sub_account_truncating(entity_id)
        }

        /// 计算初始运营资金（USDT 等值 NEX）
        /// 
        /// # 算法
        /// 1. 获取 NEX/USDT 价格（精度 10^6）
        /// 2. 计算所需 NEX 数量 = USDT 金额 * 10^12 / 价格
        /// 3. 限制在 [MinInitialFundCos, MaxInitialFundCos] 范围内
        pub fn calculate_initial_fund() -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
            let price = T::PricingProvider::get_nex_usdt_price();
            ensure!(price > 0, Error::<T>::PriceUnavailable);

            let min_fund = T::MinInitialFundCos::get();
            let max_fund = T::MaxInitialFundCos::get();

            // 价格过时时使用保守兜底值，避免基于过期数据计算押金
            if T::PricingProvider::is_price_stale() {
                return Ok(min_fund);
            }

            let usdt_amount = T::InitialFundUsdt::get();

            // nex_amount = usdt_amount * 10^12 / price
            let nex_amount_u128 = (usdt_amount as u128)
                .checked_mul(1_000_000_000_000u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(price as u128)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            let nex_amount: BalanceOf<T> = nex_amount_u128.saturated_into();

            let final_fund = nex_amount.max(min_fund).min(max_fund);

            Ok(final_fund)
        }

        /// 获取资金健康状态
        pub fn get_fund_health(balance: BalanceOf<T>) -> FundHealth {
            let min_balance = T::MinOperatingBalance::get();
            let warning_threshold = T::FundWarningThreshold::get();

            if balance.is_zero() {
                FundHealth::Depleted
            } else if balance <= min_balance {
                FundHealth::Critical
            } else if balance <= warning_threshold {
                FundHealth::Warning
            } else {
                FundHealth::Healthy
            }
        }

        /// 获取实体金库资金余额
        pub fn get_entity_fund_balance(entity_id: u64) -> BalanceOf<T> {
            let treasury_account = Self::entity_treasury_account(entity_id);
            T::Currency::free_balance(&treasury_account)
        }

        /// 扣除运营费用（供其他模块调用）
        pub fn deduct_operating_fee(
            entity_id: u64,
            fee: BalanceOf<T>,
            fee_type: FeeType,
        ) -> sp_runtime::DispatchResult {
            // H4+M4 修复: 检查 Entity 状态，仅 Active/Suspended 允许扣费
            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                matches!(entity.status, EntityStatus::Active | EntityStatus::Suspended),
                Error::<T>::EntityNotActive
            );

            let treasury_account = Self::entity_treasury_account(entity_id);
            let balance = T::Currency::free_balance(&treasury_account);

            ensure!(balance >= fee, Error::<T>::InsufficientOperatingFund);

            // 运营费用转入平台账户
            T::Currency::transfer(
                &treasury_account,
                &T::PlatformAccount::get(),
                fee,
                ExistenceRequirement::AllowDeath,
            )?;

            let new_balance = T::Currency::free_balance(&treasury_account);
            let min_balance = T::MinOperatingBalance::get();
            let warning_threshold = T::FundWarningThreshold::get();

            // 检查资金健康状态
            if new_balance <= min_balance {
                // 低于最低余额，暂停实体
                Entities::<T>::mutate(entity_id, |s| {
                    if let Some(entity) = s {
                        if entity.status == EntityStatus::Active {
                            entity.status = EntityStatus::Suspended;
                            EntityStats::<T>::mutate(|stats| {
                                stats.active_entities = stats.active_entities.saturating_sub(1);
                            });
                        }
                    }
                });
                Self::deposit_event(Event::EntitySuspendedLowFund {
                    entity_id,
                    current_balance: new_balance,
                    minimum_balance: min_balance,
                });
            } else if new_balance <= warning_threshold {
                // 发出预警
                Self::deposit_event(Event::FundWarning {
                    entity_id,
                    current_balance: new_balance,
                    warning_threshold,
                });
            }

            Self::deposit_event(Event::OperatingFeeDeducted {
                entity_id,
                fee,
                fee_type,
                remaining_balance: new_balance,
            });

            Ok(())
        }

        /// 获取当前初始资金金额（供前端查询）
        pub fn get_current_initial_fund() -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
            Self::calculate_initial_fund()
        }

        /// 获取初始资金计算详情（供前端查询）
        pub fn get_initial_fund_details() -> (u64, u64, u128) {
            let price = T::PricingProvider::get_nex_usdt_price();
            let usdt_amount = T::InitialFundUsdt::get();

            let nex_amount = if price > 0 {
                (usdt_amount as u128)
                    .saturating_mul(1_000_000_000_000u128)
                    .checked_div(price as u128)
                    .unwrap_or(0)
            } else {
                0
            };

            (usdt_amount, price, nex_amount)
        }

        // ==================== Phase 3 新增辅助函数 ====================

        /// 检查是否是管理员（所有者或管理员列表中）
        pub fn is_admin(entity_id: u64, who: &T::AccountId) -> bool {
            Entities::<T>::get(entity_id)
                .map(|entity| {
                    entity.owner == *who || entity.admins.contains(who)
                })
                .unwrap_or(false)
        }

        /// 确保调用者是管理员
        pub fn ensure_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult {
            ensure!(Self::is_admin(entity_id, who), Error::<T>::NotAdmin);
            Ok(())
        }

        /// 验证 owner 发起的升级路径（治理 origin 绕过此函数，可任意升级）
        /// - Merchant → 任何类型
        /// - Community → DAO
        /// - Project → DAO / Enterprise
        /// - 其他类型 → 仅治理可操作
        pub fn validate_entity_type_upgrade(
            current: &EntityType,
            new: &EntityType,
        ) -> DispatchResult {
            // 相同类型不需要升级
            if current == new {
                return Ok(());
            }

            // 允许的升级路径
            let allowed = match current {
                EntityType::Merchant => true, // 商户可升级为任何类型
                EntityType::Community => matches!(new, EntityType::DAO), // 社区可升级为 DAO
                EntityType::Project => matches!(new, EntityType::DAO | EntityType::Enterprise), // 项目可升级为 DAO 或企业
                _ => false, // 其他类型需要治理特殊批准
            };

            ensure!(allowed, Error::<T>::InvalidEntityTypeUpgrade);
            Ok(())
        }

        /// 获取实体类型
        pub fn get_entity_type(entity_id: u64) -> Option<EntityType> {
            Entities::<T>::get(entity_id).map(|e| e.entity_type)
        }

        /// 获取治理模式
        pub fn get_governance_mode(entity_id: u64) -> Option<GovernanceMode> {
            Entities::<T>::get(entity_id).map(|e| e.governance_mode)
        }

        /// 检查实体是否已验证
        pub fn is_verified(entity_id: u64) -> bool {
            Entities::<T>::get(entity_id)
                .map(|e| e.verified)
                .unwrap_or(false)
        }

        /// 获取管理员列表
        pub fn get_admins(entity_id: u64) -> Vec<T::AccountId> {
            Entities::<T>::get(entity_id)
                .map(|e| e.admins.into_inner())
                .unwrap_or_default()
        }

        /// 检查账户是否拥有至少一个 Active Entity
        pub fn has_active_entity(account: &T::AccountId) -> bool {
            UserEntity::<T>::get(account)
                .iter()
                .any(|&eid| {
                    Entities::<T>::get(eid)
                        .map(|e| e.status == EntityStatus::Active)
                        .unwrap_or(false)
                })
        }
    }

    // ==================== EntityProvider 实现 ====================

    impl<T: Config> EntityProvider<T::AccountId> for Pallet<T> {
        fn entity_exists(entity_id: u64) -> bool {
            Entities::<T>::contains_key(entity_id)
        }

        fn is_entity_active(entity_id: u64) -> bool {
            Entities::<T>::get(entity_id)
                .map(|s| s.status == EntityStatus::Active)
                .unwrap_or(false)
        }

        fn entity_status(entity_id: u64) -> Option<EntityStatus> {
            Entities::<T>::get(entity_id).map(|s| s.status)
        }

        fn entity_owner(entity_id: u64) -> Option<T::AccountId> {
            Entities::<T>::get(entity_id).map(|s| s.owner)
        }

        fn entity_account(entity_id: u64) -> T::AccountId {
            Self::entity_treasury_account(entity_id)
        }

        fn update_entity_stats(entity_id: u64, sales_amount: u128, order_count: u32) -> Result<(), sp_runtime::DispatchError> {
            Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<(), sp_runtime::DispatchError> {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                // L2 修复: 不允许对 Banned/Closed Entity 更新统计
                ensure!(
                    !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                    Error::<T>::EntityNotActive
                );
                entity.total_sales = entity.total_sales.saturating_add(sales_amount.saturated_into());
                entity.total_orders = entity.total_orders.saturating_add(order_count as u64);
                Ok(())
            })
        }

        fn update_entity_rating(_entity_id: u64, _rating: u8) -> Result<(), sp_runtime::DispatchError> {
            // Note: 评分功能已移至 Shop 层级，Entity 层级不再存储评分
            // 此方法保留以保持 API 兼容性
            Ok(())
        }

        fn register_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
            // 验证 Entity 存在
            ensure!(Entities::<T>::contains_key(entity_id), Error::<T>::EntityNotFound);

            // 添加到 EntityShops 列表
            EntityShops::<T>::try_mutate(entity_id, |shops| -> Result<(), sp_runtime::DispatchError> {
                // H5 修复: 区分重复注册和容量已满两种错误
                ensure!(!shops.contains(&shop_id), Error::<T>::ShopAlreadyRegistered);
                shops.try_push(shop_id).map_err(|_| Error::<T>::ShopLimitReached)?;
                Ok(())
            })?;

            // 如果是第一个 Shop，设为 primary
            Entities::<T>::mutate(entity_id, |maybe_entity| {
                if let Some(entity) = maybe_entity {
                    if entity.primary_shop_id == 0 {
                        entity.primary_shop_id = shop_id;
                    }
                }
            });

            Self::deposit_event(Event::ShopAddedToEntity { entity_id, shop_id });
            Ok(())
        }

        fn unregister_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
            EntityShops::<T>::try_mutate(entity_id, |shops| -> Result<(), sp_runtime::DispatchError> {
                let pos = shops.iter().position(|&id| id == shop_id)
                    .ok_or(Error::<T>::ShopNotRegistered)?;
                shops.remove(pos);
                Ok(())
            })?;

            // 如果移除的是 primary，重新指定（取列表第一个，或清零）
            Entities::<T>::mutate(entity_id, |maybe_entity| {
                if let Some(entity) = maybe_entity {
                    if entity.primary_shop_id == shop_id {
                        entity.primary_shop_id = EntityShops::<T>::get(entity_id)
                            .first()
                            .copied()
                            .unwrap_or(0);
                    }
                }
            });
            Ok(())
        }

        fn is_entity_admin(entity_id: u64, account: &T::AccountId) -> bool {
            Self::is_admin(entity_id, account)
        }

        fn entity_shops(entity_id: u64) -> sp_std::vec::Vec<u64> {
            EntityShops::<T>::get(entity_id).into_inner()
        }

        // H4 修复: 实现 pause_entity/resume_entity（供治理模块调用）
        fn pause_entity(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::Active, Error::<T>::InvalidEntityStatus);

            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Suspended;
                }
            });
            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_sub(1);
            });
            // 标记为治理暂停（防止 top_up_fund 绕过治理自动恢复）
            GovernanceSuspended::<T>::insert(entity_id, true);

            Self::deposit_event(Event::EntityStatusChanged {
                entity_id,
                status: EntityStatus::Suspended,
            });
            Ok(())
        }

        fn resume_entity(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
            let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::Suspended, Error::<T>::InvalidEntityStatus);

            let treasury_account = Self::entity_treasury_account(entity_id);
            let balance = T::Currency::free_balance(&treasury_account);
            let min_balance = T::MinOperatingBalance::get();
            ensure!(balance >= min_balance, Error::<T>::InsufficientOperatingFund);

            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Active;
                }
            });
            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_add(1);
            });
            GovernanceSuspended::<T>::remove(entity_id);

            Self::deposit_event(Event::EntityStatusChanged {
                entity_id,
                status: EntityStatus::Active,
            });
            Ok(())
        }

        // C2: 治理 pallet 同步调用 — 更新 Entity.governance_mode 字段
        fn set_governance_mode(entity_id: u64, mode: GovernanceMode) -> Result<(), sp_runtime::DispatchError> {
            Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<(), sp_runtime::DispatchError> {
                let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
                entity.governance_mode = mode;
                Ok(())
            })
        }
    }
}
