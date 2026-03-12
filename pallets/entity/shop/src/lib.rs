//! # Shop 模块 (pallet-entity-shop)
//!
//! ## 概述
//!
//! 本模块负责 Shop（业务层店铺）的生命周期管理，包括：
//! - Shop 创建（归属于 Entity）
//! - Shop 信息更新
//! - Shop 管理员管理
//! - 运营资金管理
//! - Shop 状态管理（暂停、恢复、关闭）
//! - Shop 积分系统（可选）
//!
//! ## Entity-Shop 关系
//!
//! - 一个 Entity 可以拥有多个 Shop
//! - 每个 Entity 有且仅有一个 Primary Shop（创建 Entity 时自动创建）
//! - Shop 继承 Entity 的代币和治理，但有独立的运营资金和业务数据
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-02-05): 初始版本

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use pallet_entity_common::{ShopOperatingStatus, ShopType};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
        BoundedVec, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{
        CommissionFundGuard, EffectiveShopStatus, EntityProvider, EntityStatus,
        OrderProvider, PointsCleanup, ProductProvider, ShopOperatingStatus, ShopProvider, ShopType,
    };
    use pallet_storage_service::{StoragePin, PinTier};
    use sp_runtime::{
        traits::{AccountIdConversion, Saturating, Zero},
        DispatchError, SaturatedConversion,
    };

    use crate::WeightInfo;

    /// Shop 派生账户 PalletId
    const SHOP_PALLET_ID: PalletId = PalletId(*b"et/shop_");

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Shop 结构体
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxNameLen, MaxCidLen, MaxManagers))]
    pub struct Shop<AccountId, Balance, BlockNumber, MaxNameLen: Get<u32>, MaxCidLen: Get<u32>, MaxManagers: Get<u32>> {
        /// Shop ID（全局唯一）
        pub id: u64,
        /// 所属 Entity ID
        pub entity_id: u64,
        /// Shop 名称
        pub name: BoundedVec<u8, MaxNameLen>,
        /// Logo IPFS CID
        pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 描述 IPFS CID
        pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// Shop 类型
        pub shop_type: ShopType,
        /// Shop 状态
        pub status: ShopOperatingStatus,
        /// Shop 管理员列表
        pub managers: BoundedVec<AccountId, MaxManagers>,
        /// 地理位置（经度, 纬度）* 10^6
        pub location: Option<(i64, i64)>,
        /// 地址信息 CID
        pub address_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 营业时间 CID
        pub business_hours_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 店铺政策 CID（退换货/运费/服务条款）
        pub policies_cid: Option<BoundedVec<u8, MaxCidLen>>,
        /// 创建时间
        pub created_at: BlockNumber,
        /// 商品/服务数量
        pub product_count: u32,
        /// 累计销售额
        pub total_sales: Balance,
        /// 累计订单数
        pub total_orders: u32,
        /// 评分（0-500 = 0.0-5.0）
        pub rating: u16,
        /// 评分累计总和（精度 *100，用于精确计算平均值）
        pub rating_total: u64,
        /// 评价数量
        pub rating_count: u32,
    }

    /// Shop 类型别名
    pub type ShopOf<T> = Shop<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
        <T as Config>::MaxShopNameLength,
        <T as Config>::MaxCidLength,
        <T as Config>::MaxManagers,
    >;

    /// 转让请求（双向确认模式）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct PendingShopTransfer<BlockNumber> {
        /// 源 Entity ID
        pub from_entity_id: u64,
        /// 目标 Entity ID
        pub to_entity_id: u64,
        /// 是否继承原有 managers（目标方可在 accept 时覆盖）
        pub keep_managers: bool,
        /// 请求创建区块
        pub requested_at: BlockNumber,
    }

    /// PendingShopTransfer 类型别名
    pub type PendingShopTransferOf<T> = PendingShopTransfer<BlockNumberFor<T>>;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 货币类型
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Entity 提供者
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Shop 名称最大长度
        #[pallet::constant]
        type MaxShopNameLength: Get<u32>;

        /// CID 最大长度
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// 最大管理员数量
        #[pallet::constant]
        type MaxManagers: Get<u32>;

        /// 最低运营余额（低于此值 Shop 暂停）
        #[pallet::constant]
        type MinOperatingBalance: Get<BalanceOf<Self>>;

        /// 资金预警阈值
        #[pallet::constant]
        type WarningThreshold: Get<BalanceOf<Self>>;

        /// 佣金资金保护（查询已承诺的 pending + shopping 总额）
        type CommissionFundGuard: pallet_entity_common::CommissionFundGuard;

        /// Shop 关闭宽限期（区块数）
        #[pallet::constant]
        type ShopClosingGracePeriod: Get<BlockNumberFor<Self>>;

        /// 每个 Entity 最大 Shop 数量
        #[pallet::constant]
        type MaxShopsPerEntity: Get<u32>;

        /// IPFS Pin 管理接口（用于店铺元数据 CID 持久化）
        type StoragePin: StoragePin<Self::AccountId>;

        /// Product 提供者（用于 Shop 关闭时级联 unpin Product CID）
        type ProductProvider: ProductProvider<Self::AccountId, BalanceOf<Self>>;

        /// 积分清理接口（Shop 关闭时委托 loyalty 模块清理）
        type PointsCleanup: pallet_entity_common::PointsCleanup;

        /// 订单查询接口（用于 close/transfer 前检查活跃订单）
        type OrderProvider: OrderProvider<Self::AccountId, BalanceOf<Self>>;

        /// Weight 信息（由 benchmark 生成）
        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(
                T::MaxManagers::get() >= 1,
                "MaxManagers must be >= 1"
            );
            assert!(
                T::MaxShopNameLength::get() >= 1,
                "MaxShopNameLength must be >= 1"
            );
            assert!(
                T::MaxCidLength::get() >= 1,
                "MaxCidLength must be >= 1"
            );
            assert!(
                T::MaxShopsPerEntity::get() >= 1,
                "MaxShopsPerEntity must be >= 1"
            );
            assert!(
                T::WarningThreshold::get() >= T::MinOperatingBalance::get(),
                "WarningThreshold must be >= MinOperatingBalance"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// Shop 存储 shop_id -> Shop
    #[pallet::storage]
    #[pallet::getter(fn shops)]
    pub type Shops<T: Config> = StorageMap<_, Blake2_128Concat, u64, ShopOf<T>>;

    /// Shop -> Entity 反向索引 shop_id -> entity_id
    #[pallet::storage]
    #[pallet::getter(fn shop_entity)]
    pub type ShopEntity<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64>;

    /// Shop ID 起始值（从 1 开始，避免 0 与 primary_shop_id 哨兵值冲突）
    #[pallet::type_value]
    pub fn DefaultNextShopId() -> u64 { 1 }

    /// 下一个 Shop ID
    #[pallet::storage]
    #[pallet::getter(fn next_shop_id)]
    pub type NextShopId<T: Config> = StorageValue<_, u64, ValueQuery, DefaultNextShopId>;

    /// Shop 关闭倒计时 shop_id -> closing_initiated_at_block
    #[pallet::storage]
    #[pallet::getter(fn shop_closing_at)]
    pub type ShopClosingAt<T: Config> = StorageMap<_, Blake2_128Concat, u64, BlockNumberFor<T>>;

    // EntityPrimaryShop 已移除：主店由 registry Entity.primary_shop_id 唯一管理
    // 通过 T::EntityProvider::get_primary_shop_id(entity_id) 查询

    /// 封禁前的 Shop 状态（用于解封时恢复）shop_id -> ShopOperatingStatus
    #[pallet::storage]
    pub type ShopStatusBeforeBan<T: Config> = StorageMap<_, Blake2_128Concat, u64, ShopOperatingStatus>;

    /// 封禁原因 shop_id -> reason
    #[pallet::storage]
    pub type ShopBanReason<T: Config> = StorageMap<_, Blake2_128Concat, u64, BoundedVec<u8, T::MaxCidLength>>;

    /// 待确认的转让请求 shop_id -> PendingShopTransfer
    #[pallet::storage]
    pub type PendingTransfers<T: Config> = StorageMap<_, Blake2_128Concat, u64, PendingShopTransferOf<T>>;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Shop 创建
        ShopCreated {
            shop_id: u64,
            entity_id: u64,
            name: BoundedVec<u8, T::MaxShopNameLength>,
            shop_type: ShopType,
        },
        /// Shop 信息更新
        ShopUpdated { shop_id: u64 },
        /// Shop 管理员添加
        ManagerAdded { shop_id: u64, manager: T::AccountId },
        /// Shop 管理员移除
        ManagerRemoved { shop_id: u64, manager: T::AccountId },
        /// Shop 运营资金充值
        OperatingFundDeposited { shop_id: u64, amount: BalanceOf<T>, new_balance: BalanceOf<T> },
        /// Shop 运营资金扣减
        OperatingFundDeducted { shop_id: u64, amount: BalanceOf<T>, new_balance: BalanceOf<T> },
        /// Shop 暂停
        ShopPaused { shop_id: u64 },
        /// Shop 恢复
        ShopResumed { shop_id: u64 },
        /// Shop 关闭
        ShopClosed { shop_id: u64 },
        /// Shop 位置更新
        ShopLocationUpdated { shop_id: u64, location: Option<(i64, i64)> },
        /// 资金预警
        FundWarning { shop_id: u64, balance: BalanceOf<T> },
        /// 资金耗尽
        FundDepleted { shop_id: u64 },
        /// Shop 运营资金提取
        OperatingFundWithdrawn { shop_id: u64, to: T::AccountId, amount: BalanceOf<T>, new_balance: BalanceOf<T> },
        /// Shop 关闭时资金退还
        ShopClosedFundRefunded { shop_id: u64, to: T::AccountId, amount: BalanceOf<T> },
        /// Shop 进入关闭宽限期
        ShopClosing { shop_id: u64, grace_until: BlockNumberFor<T> },
        /// Shop 关闭完成（宽限期满）
        ShopCloseFinalized { shop_id: u64 },
        /// Shop 转让
        ShopTransferred { shop_id: u64, from_entity_id: u64, to_entity_id: u64 },
        /// Shop 转让请求发起
        ShopTransferRequested { shop_id: u64, from_entity_id: u64, to_entity_id: u64 },
        /// Shop 转让请求被取消
        ShopTransferCancelled { shop_id: u64 },
        /// 主 Shop 变更
        PrimaryShopChanged { entity_id: u64, old_shop_id: u64, new_shop_id: u64 },
        /// Shop 被强制暂停（Root）
        ShopForcePaused { shop_id: u64 },
        /// Shop 被 Root 强制关闭
        ShopForceClosedByRoot { shop_id: u64 },
        /// Shop 类型变更
        ShopTypeChanged { shop_id: u64, old_type: ShopType, new_type: ShopType },
        /// Shop 关闭撤回
        ShopClosingCancelled { shop_id: u64 },
        /// Manager 自我辞职
        ManagerResigned { shop_id: u64, manager: T::AccountId },
        /// Shop 被封禁（Root）
        ShopBannedByRoot { shop_id: u64, reason: BoundedVec<u8, T::MaxCidLength> },
        /// Shop 被解封（Root）
        ShopUnbannedByRoot { shop_id: u64, restored_status: ShopOperatingStatus },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Entity 不存在
        EntityNotFound,
        /// Entity 未激活
        EntityNotActive,
        /// Shop 不存在
        ShopNotFound,
        /// Shop 未处于 Active 状态（无法执行此操作）
        ShopNotActive,
        /// 无权限操作
        NotAuthorized,
        /// Shop 名称过长
        NameTooLong,
        /// Shop 名称不能为空
        ShopNameEmpty,
        /// 管理员已存在
        ManagerAlreadyExists,
        /// 管理员不存在
        ManagerNotFound,
        /// 管理员数量已满
        TooManyManagers,
        /// 运营资金不足
        InsufficientOperatingFund,
        /// Shop 已暂停
        ShopAlreadyPaused,
        /// Shop 未暂停
        ShopNotPaused,
        /// Shop 已关闭
        ShopAlreadyClosed,
        /// 无效的位置信息
        InvalidLocation,
        /// 无效的配置
        InvalidConfig,
        /// 不能关闭主 Shop
        CannotClosePrimaryShop,
        /// 提取后余额低于最低运营要求
        WithdrawBelowMinimum,
        /// 提取金额为零
        ZeroWithdrawAmount,
        /// Shop ID 溢出
        ShopIdOverflow,
        /// CID 不能为空
        EmptyCid,
        /// 充值金额为零
        ZeroFundAmount,
        /// Shop 已在关闭中（宽限期内）
        ShopAlreadyClosing,
        /// 关闭宽限期未满
        ClosingGracePeriodNotElapsed,
        /// Shop 不在关闭宽限期中
        ShopNotClosing,
        /// 不可转让主 Shop
        CannotTransferPrimaryShop,
        /// 不能转让给同一 Entity
        SameEntity,
        /// Shop 类型未变更
        ShopTypeSame,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// Entity 的 Shop 数量已达上限
        ShopLimitReached,
        /// 评分超出范围（须 1-5）
        InvalidRating,
        /// Shop 已被封禁
        ShopBanned,
        /// Shop 未被封禁
        ShopNotBanned,
        /// 调用者不是该 Shop 的管理员
        NotManager,
        /// Shop 存在活跃订单（关闭/转让前须结清）
        HasActiveOrders,
        /// 已有待确认的转让请求
        TransferAlreadyPending,
        /// 无待确认的转让请求
        NoTransferPending,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建 Shop
        /// 
        /// 参数:
        /// - `entity_id`: 所属 Entity ID
        /// - `name`: Shop 名称
        /// - `shop_type`: Shop 类型
        /// - `initial_fund`: 初始运营资金
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_shop())]
        pub fn create_shop(
            origin: OriginFor<T>,
            entity_id: u64,
            name: BoundedVec<u8, T::MaxShopNameLength>,
            shop_type: ShopType,
            initial_fund: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // 校验参数
            ensure!(!name.is_empty(), Error::<T>::ShopNameEmpty);

            // 检查 Entity 存在且激活
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            
            // 检查权限（仅 Entity owner）
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(who == owner, Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // Shop 数量预检
            let current_shops = T::EntityProvider::entity_shops(entity_id);
            ensure!(
                (current_shops.len() as u32) < T::MaxShopsPerEntity::get(),
                Error::<T>::ShopLimitReached
            );
            
            let shop_id = Self::do_create_shop(entity_id, name, shop_type)?;
            
            // 再转移初始资金（Shop 已创建，转账失败会回滚整个 extrinsic）
            if !initial_fund.is_zero() {
                let shop_account = Self::shop_account_id(shop_id);
                T::Currency::transfer(&who, &shop_account, initial_fund, ExistenceRequirement::KeepAlive)?;
            }
            
            Ok(())
        }

        /// 更新 Shop 信息
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_shop())]
        pub fn update_shop(
            origin: OriginFor<T>,
            shop_id: u64,
            name: Option<BoundedVec<u8, T::MaxShopNameLength>>,
            logo_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
            description_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
            business_hours_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
            policies_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            ensure!(
                name.is_some() || logo_cid.is_some() || description_cid.is_some()
                    || business_hours_cid.is_some() || policies_cid.is_some(),
                Error::<T>::InvalidConfig
            );

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .unwrap_or_else(|| who.clone());

                let old_logo = shop.logo_cid.clone();
                let old_desc = shop.description_cid.clone();
                let old_hours = shop.business_hours_cid.clone();
                let old_policies = shop.policies_cid.clone();

                if let Some(n) = name {
                    ensure!(!n.is_empty(), Error::<T>::ShopNameEmpty);
                    shop.name = n;
                }
                Self::apply_triple_option_cid(&mut shop.logo_cid, &logo_cid)?;
                Self::apply_triple_option_cid(&mut shop.description_cid, &description_cid)?;
                Self::apply_triple_option_cid(&mut shop.business_hours_cid, &business_hours_cid)?;
                Self::apply_triple_option_cid(&mut shop.policies_cid, &policies_cid)?;

                Self::ipfs_handle_triple_option(&owner, shop_id, &old_logo, &logo_cid);
                Self::ipfs_handle_triple_option(&owner, shop_id, &old_desc, &description_cid);
                Self::ipfs_handle_triple_option(&owner, shop_id, &old_hours, &business_hours_cid);
                Self::ipfs_handle_triple_option(&owner, shop_id, &old_policies, &policies_cid);
                
                Self::deposit_event(Event::ShopUpdated { shop_id });
                Ok(())
            })
        }

        /// 添加 Shop 管理员
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::add_manager())]
        pub fn add_manager(
            origin: OriginFor<T>,
            shop_id: u64,
            manager: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 只有 Entity owner 可以添加管理员
                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(who == owner, Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);

                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                
                ensure!(!shop.managers.contains(&manager), Error::<T>::ManagerAlreadyExists);
                
                // 添加管理员
                shop.managers.try_push(manager.clone())
                    .map_err(|_| Error::<T>::TooManyManagers)?;
                
                Self::deposit_event(Event::ManagerAdded { shop_id, manager });
                Ok(())
            })
        }

        /// 移除 Shop 管理员
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::remove_manager())]
        pub fn remove_manager(
            origin: OriginFor<T>,
            shop_id: u64,
            manager: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 只有 Entity owner 可以移除管理员
                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(who == owner, Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);

                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
                
                // 查找并移除
                let pos = shop.managers.iter().position(|m| m == &manager)
                    .ok_or(Error::<T>::ManagerNotFound)?;
                shop.managers.remove(pos);
                
                Self::deposit_event(Event::ManagerRemoved { shop_id, manager });
                Ok(())
            })
        }

        /// 充值运营资金
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::fund_operating())]
        pub fn fund_operating(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            ensure!(!amount.is_zero(), Error::<T>::ZeroFundAmount);

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                // H2: 已关闭/封禁的店铺不允许充值（Closing 允许，以覆盖宽限期内义务）
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
                
                // 转移资金
                let shop_account = Self::shop_account_id(shop_id);
                T::Currency::transfer(&who, &shop_account, amount, ExistenceRequirement::KeepAlive)?;
                
                let new_balance = T::Currency::free_balance(&shop_account);
                
                // 如果之前是 FundDepleted 状态，恢复为 Active
                if shop.status == ShopOperatingStatus::FundDepleted && new_balance >= T::MinOperatingBalance::get() {
                    shop.status = ShopOperatingStatus::Active;
                    Self::deposit_event(Event::ShopResumed { shop_id });
                }
                
                Self::deposit_event(Event::OperatingFundDeposited { shop_id, amount, new_balance });
                Ok(())
            })
        }

        /// 暂停 Shop
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::pause_shop())]
        pub fn pause_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                
                // 检查状态（仅 Active 可暂停；FundDepleted/Closing/Closed 等不可暂停）
                ensure!(shop.status == ShopOperatingStatus::Active, Error::<T>::ShopNotActive);
                
                shop.status = ShopOperatingStatus::Paused;
                
                Self::deposit_event(Event::ShopPaused { shop_id });
                Ok(())
            })
        }

        /// 恢复 Shop
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::resume_shop())]
        pub fn resume_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                
                // 封禁状态仅 Root 可解封，owner/manager 不可恢复
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
                // 检查状态
                ensure!(shop.status.can_resume(), Error::<T>::ShopNotPaused);
                
                // 检查运营资金
                let shop_account = Self::shop_account_id(shop_id);
                let balance = T::Currency::free_balance(&shop_account);
                ensure!(balance >= T::MinOperatingBalance::get(), Error::<T>::InsufficientOperatingFund);
                
                shop.status = ShopOperatingStatus::Active;
                
                Self::deposit_event(Event::ShopResumed { shop_id });
                Ok(())
            })
        }

        /// 设置 Shop 位置信息
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::set_location())]
        pub fn set_location(
            origin: OriginFor<T>,
            shop_id: u64,
            location: Option<(i64, i64)>,
            address_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .unwrap_or_else(|| who.clone());

                let old_addr = shop.address_cid.clone();
                
                if let Some((lng, lat)) = location {
                    ensure!(lng >= -180_000_000 && lng <= 180_000_000, Error::<T>::InvalidLocation);
                    ensure!(lat >= -90_000_000 && lat <= 90_000_000, Error::<T>::InvalidLocation);
                }
                
                shop.location = location;
                Self::apply_triple_option_cid(&mut shop.address_cid, &address_cid)?;

                Self::ipfs_handle_triple_option(&owner, shop_id, &old_addr, &address_cid);
                
                Self::deposit_event(Event::ShopLocationUpdated { shop_id, location });
                Ok(())
            })
        }

        /// 关闭 Shop（进入宽限期）
        ///
        /// 将 Shop 状态设为 Closing，开始宽限期倒计时。
        /// 宽限期内：不可接新单/上新品，已有订单可完成，积分可转移。
        /// 宽限期满后调用 `finalize_close_shop` 完成清理。
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::close_shop())]
        pub fn close_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 只有 Entity owner 可以关闭 Shop
                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(who == owner, Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                
                ensure!(
                    T::EntityProvider::get_primary_shop_id(shop.entity_id) != shop_id,
                    Error::<T>::CannotClosePrimaryShop
                );
                
                // 检查状态
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                ensure!(shop.status != ShopOperatingStatus::Closing, Error::<T>::ShopAlreadyClosing);
                // M1: 封禁状态仅 Root 可通过 unban_shop 解封，owner 不可通过 close 绕过封禁
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);

                // 活跃订单检查：存在未终结订单时不允许进入关闭流程
                ensure!(
                    !T::OrderProvider::has_active_orders_for_shop(shop_id),
                    Error::<T>::HasActiveOrders
                );
                
                shop.status = ShopOperatingStatus::Closing;
                
                let now = <frame_system::Pallet<T>>::block_number();
                let grace = T::ShopClosingGracePeriod::get();
                let grace_until = now.saturating_add(grace);
                ShopClosingAt::<T>::insert(shop_id, now);
                
                Self::deposit_event(Event::ShopClosing { shop_id, grace_until });
                Ok(())
            })
        }

        /// 完成 Shop 关闭（宽限期满后调用）
        ///
        /// 任何人可调用。检查宽限期已过，然后执行清理：
        /// 注销 Entity 关联、清积分数据、退还剩余资金。
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::finalize_close_shop())]
        pub fn finalize_close_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                ensure!(shop.status == ShopOperatingStatus::Closing, Error::<T>::ShopNotClosing);

                let closing_at = ShopClosingAt::<T>::get(shop_id)
                    .ok_or(Error::<T>::ShopNotFound)?;
                let now = <frame_system::Pallet<T>>::block_number();
                let grace = T::ShopClosingGracePeriod::get();
                ensure!(now >= closing_at.saturating_add(grace), Error::<T>::ClosingGracePeriodNotElapsed);

                // 二次检查：宽限期内可能有已存在的订单仍在进行
                ensure!(
                    !T::OrderProvider::has_active_orders_for_shop(shop_id),
                    Error::<T>::HasActiveOrders
                );

                Self::do_close_shop_cleanup(shop, shop_id);

                Self::deposit_event(Event::ShopCloseFinalized { shop_id });
                Ok(())
            })
        }

        /// 提取运营资金
        ///
        /// 仅 Entity owner 可调用，将 shop_account 中的运营资金提取到个人账户。
        /// - 活跃 Shop: 提取后余额不得低于 MinOperatingBalance
        /// - 已关闭 Shop: 无最低余额限制，可全额提取
        /// - 佣金保护: 不得侵占已承诺的佣金资金 (pending + shopping)
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::withdraw_operating_fund())]
        pub fn withdraw_operating_fund(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawAmount);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;

            let owner = T::EntityProvider::entity_owner(shop.entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(who == owner, Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);

            // P0-H1: 封禁/关闭中的 Shop 不允许提取运营资金
            ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
            ensure!(shop.status != ShopOperatingStatus::Closing, Error::<T>::ShopAlreadyClosing);

            let shop_account = Self::shop_account_id(shop_id);
            let balance = T::Currency::free_balance(&shop_account);

            let protected: BalanceOf<T> = T::CommissionFundGuard::protected_funds(shop.entity_id).saturated_into();
            let available = balance.saturating_sub(protected);
            ensure!(available >= amount, Error::<T>::InsufficientOperatingFund);

            if shop.status != ShopOperatingStatus::Closed {
                let after_withdraw = balance.saturating_sub(amount);
                ensure!(
                    after_withdraw >= T::MinOperatingBalance::get(),
                    Error::<T>::WithdrawBelowMinimum
                );
            }

            // 转账 shop_account → entity_owner
            T::Currency::transfer(
                &shop_account,
                &who,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            let new_balance = T::Currency::free_balance(&shop_account);

            Self::deposit_event(Event::OperatingFundWithdrawn {
                shop_id,
                to: who,
                amount,
                new_balance,
            });
            Ok(())
        }

        /// 发起 Shop 转让请求（源 Entity owner 调用）
        ///
        /// 创建一个待确认的转让请求。目标 Entity owner 需调用 accept_transfer_shop 接收。
        /// 转让前检查活跃订单。主 Shop 不可转让。
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::transfer_shop())]
        pub fn request_transfer_shop(
            origin: OriginFor<T>,
            shop_id: u64,
            to_entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 不得有未处理的转让请求
            ensure!(!PendingTransfers::<T>::contains_key(shop_id), Error::<T>::TransferAlreadyPending);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            let from_entity_id = shop.entity_id;

            // 仅 Entity owner 可发起转让
            let owner = T::EntityProvider::entity_owner(from_entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(who == owner, Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(from_entity_id), Error::<T>::EntityLocked);

            ensure!(
                T::EntityProvider::get_primary_shop_id(from_entity_id) != shop_id,
                Error::<T>::CannotTransferPrimaryShop
            );
            ensure!(from_entity_id != to_entity_id, Error::<T>::SameEntity);
            ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
            ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);

            // 活跃订单检查
            ensure!(
                !T::OrderProvider::has_active_orders_for_shop(shop_id),
                Error::<T>::HasActiveOrders
            );

            // 目标 Entity 必须存在且激活
            ensure!(T::EntityProvider::entity_exists(to_entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(to_entity_id), Error::<T>::EntityNotActive);

            // 检查目标 Entity 的 Shop 数量上限
            let target_shops = T::EntityProvider::entity_shops(to_entity_id);
            ensure!(
                (target_shops.len() as u32) < T::MaxShopsPerEntity::get(),
                Error::<T>::ShopLimitReached
            );

            let now = <frame_system::Pallet<T>>::block_number();
            PendingTransfers::<T>::insert(shop_id, PendingShopTransfer {
                from_entity_id,
                to_entity_id,
                keep_managers: true,
                requested_at: now,
            });

            Self::deposit_event(Event::ShopTransferRequested {
                shop_id,
                from_entity_id,
                to_entity_id,
            });
            Ok(())
        }

        /// 接受 Shop 转让（目标 Entity owner 调用）
        ///
        /// 目标方可选择是否保留原有 managers。
        #[pallet::call_index(33)]
        #[pallet::weight(T::WeightInfo::transfer_shop())]
        pub fn accept_transfer_shop(
            origin: OriginFor<T>,
            shop_id: u64,
            keep_managers: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let pending = PendingTransfers::<T>::get(shop_id)
                .ok_or(Error::<T>::NoTransferPending)?;

            // 仅目标 Entity owner 可接受
            let target_owner = T::EntityProvider::entity_owner(pending.to_entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(who == target_owner, Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(pending.to_entity_id), Error::<T>::EntityLocked);

            // 重新验证目标方条件（可能在请求后变化）
            ensure!(T::EntityProvider::is_entity_active(pending.to_entity_id), Error::<T>::EntityNotActive);
            let target_shops = T::EntityProvider::entity_shops(pending.to_entity_id);
            ensure!(
                (target_shops.len() as u32) < T::MaxShopsPerEntity::get(),
                Error::<T>::ShopLimitReached
            );

            // 重新验证源方条件
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(shop.entity_id == pending.from_entity_id, Error::<T>::InvalidConfig);
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);

                // 再次检查活跃订单
                ensure!(
                    !T::OrderProvider::has_active_orders_for_shop(shop_id),
                    Error::<T>::HasActiveOrders
                );

                // 执行转让
                T::EntityProvider::unregister_shop(pending.from_entity_id, shop_id)?;
                T::EntityProvider::register_shop(pending.to_entity_id, shop_id)?;

                shop.entity_id = pending.to_entity_id;
                ShopEntity::<T>::insert(shop_id, pending.to_entity_id);

                // 目标方选择是否保留 managers
                if !keep_managers {
                    shop.managers = BoundedVec::default();
                }

                PendingTransfers::<T>::remove(shop_id);

                Self::deposit_event(Event::ShopTransferred {
                    shop_id,
                    from_entity_id: pending.from_entity_id,
                    to_entity_id: pending.to_entity_id,
                });
                Ok(())
            })
        }

        /// 取消 Shop 转让请求（源 Entity owner 或目标 Entity owner 均可调用）
        #[pallet::call_index(34)]
        #[pallet::weight(T::WeightInfo::cancel_close_shop())]
        pub fn cancel_transfer_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let pending = PendingTransfers::<T>::get(shop_id)
                .ok_or(Error::<T>::NoTransferPending)?;

            // 源方或目标方 owner 均可取消
            let from_owner = T::EntityProvider::entity_owner(pending.from_entity_id);
            let to_owner = T::EntityProvider::entity_owner(pending.to_entity_id);
            ensure!(
                from_owner.as_ref() == Some(&who) || to_owner.as_ref() == Some(&who),
                Error::<T>::NotAuthorized
            );

            PendingTransfers::<T>::remove(shop_id);

            Self::deposit_event(Event::ShopTransferCancelled { shop_id });
            Ok(())
        }

        /// 更换 Entity 的主 Shop
        ///
        /// Entity owner 或拥有 ENTITY_MANAGE 权限的 admin 可调用。
        /// 新主 Shop 必须属于同一 Entity 且处于活跃状态。
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::set_primary_shop())]
        pub fn set_primary_shop(
            origin: OriginFor<T>,
            entity_id: u64,
            new_primary_shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                who == owner || T::EntityProvider::is_entity_admin(
                    entity_id, &who, pallet_entity_common::AdminPermission::ENTITY_MANAGE,
                ),
                Error::<T>::NotAuthorized
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let new_shop = Shops::<T>::get(new_primary_shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(new_shop.entity_id == entity_id, Error::<T>::NotAuthorized);
            ensure!(!new_shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            ensure!(
                T::EntityProvider::get_primary_shop_id(entity_id) != new_primary_shop_id,
                Error::<T>::InvalidConfig
            );

            let old_primary_id = T::EntityProvider::get_primary_shop_id(entity_id);

            // Registry 是主店 source of truth
            T::EntityProvider::set_primary_shop_id(entity_id, new_primary_shop_id);

            Self::deposit_event(Event::PrimaryShopChanged {
                entity_id,
                old_shop_id: old_primary_id,
                new_shop_id: new_primary_shop_id,
            });
            Ok(())
        }

        /// 强制暂停 Shop（Root/治理层调用）
        ///
        /// 可被 owner/manager 通过 resume_shop 恢复。
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::force_pause_shop())]
        pub fn force_pause_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
                ensure!(shop.status != ShopOperatingStatus::Paused, Error::<T>::ShopAlreadyPaused);

                shop.status = ShopOperatingStatus::Paused;

                Self::deposit_event(Event::ShopForcePaused { shop_id });
                Ok(())
            })
        }

        /// Root 强制关闭 Shop
        ///
        /// 跳过宽限期，立即关闭并清理。委托给 ShopProvider::force_close_shop。
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::force_close_shop())]
        pub fn force_close_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // 委托给 trait 实现（已包含完整清理逻辑）
            <Self as ShopProvider<T::AccountId>>::force_close_shop(shop_id)?;

            Self::deposit_event(Event::ShopForceClosedByRoot { shop_id });
            Ok(())
        }

        /// 变更 Shop 类型
        ///
        /// 仅 Entity owner 可调用。已关闭/关闭中的 Shop 不可变更。
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::set_shop_type())]
        pub fn set_shop_type(
            origin: OriginFor<T>,
            shop_id: u64,
            new_shop_type: ShopType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                // 仅 Entity owner 可变更类型
                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(who == owner, Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
                ensure!(shop.shop_type != new_shop_type, Error::<T>::ShopTypeSame);

                let old_type = shop.shop_type;
                shop.shop_type = new_shop_type;

                Self::deposit_event(Event::ShopTypeChanged { shop_id, old_type, new_type: new_shop_type });
                Ok(())
            })
        }

        /// 撤回 Shop 关闭（取消宽限期）
        ///
        /// 仅 Entity owner 可调用。Shop 必须处于 Closing 状态。
        /// 撤回后恢复为 Active（如果运营资金充足）或 FundDepleted。
        #[pallet::call_index(28)]
        #[pallet::weight(T::WeightInfo::cancel_close_shop())]
        pub fn cancel_close_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                // 仅 Entity owner 可撤回
                let owner = T::EntityProvider::entity_owner(shop.entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(who == owner, Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(shop.status == ShopOperatingStatus::Closing, Error::<T>::ShopNotClosing);

                // 清理关闭计时器
                ShopClosingAt::<T>::remove(shop_id);

                // 恢复状态：检查运营资金决定恢复为 Active 还是 FundDepleted
                let shop_account = Self::shop_account_id(shop_id);
                let balance = T::Currency::free_balance(&shop_account);
                if balance >= T::MinOperatingBalance::get() {
                    shop.status = ShopOperatingStatus::Active;
                } else {
                    shop.status = ShopOperatingStatus::FundDepleted;
                }

                Self::deposit_event(Event::ShopClosingCancelled { shop_id });
                Ok(())
            })
        }

        /// Manager 自我辞职
        ///
        /// 允许 Shop Manager 主动从管理员列表中移除自己。
        /// Entity Owner 和 Entity Admin 不受此影响（他们的权限来自 Entity 层）。
        #[pallet::call_index(30)]
        #[pallet::weight(T::WeightInfo::resign_manager())]
        pub fn resign_manager(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                // 查找调用者在 managers 列表中的位置
                let pos = shop.managers.iter().position(|m| m == &who)
                    .ok_or(Error::<T>::NotManager)?;
                shop.managers.remove(pos);

                Self::deposit_event(Event::ManagerResigned { shop_id, manager: who });
                Ok(())
            })
        }

        /// 封禁 Shop（Root/治理层调用）
        ///
        /// 将 Shop 设为 Banned 状态，owner/manager 无法 resume。
        /// 仅 Root 可通过 unban_shop 解封。
        #[pallet::call_index(31)]
        #[pallet::weight(T::WeightInfo::ban_shop())]
        pub fn ban_shop(
            origin: OriginFor<T>,
            shop_id: u64,
            reason: BoundedVec<u8, T::MaxCidLength>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(!reason.is_empty(), Error::<T>::EmptyCid);

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);

                ShopStatusBeforeBan::<T>::insert(shop_id, shop.status);
                ShopBanReason::<T>::insert(shop_id, reason.clone());
                shop.status = ShopOperatingStatus::Banned;

                // v1.2-C2: 封禁时下架全部在售商品，防止封禁期间商品仍可被购买
                if let Err(e) = T::ProductProvider::force_delist_all_shop_products(shop_id) {
                    log::error!(
                        target: "entity-shop",
                        "force_delist_all_shop_products failed for shop {}: {:?}",
                        shop_id, e
                    );
                }

                Self::deposit_event(Event::ShopBannedByRoot { shop_id, reason });
                Ok(())
            })
        }

        /// 解封 Shop（Root/治理层调用）
        ///
        /// 恢复为封禁前的状态（Active/Paused/FundDepleted）。
        #[pallet::call_index(32)]
        #[pallet::weight(T::WeightInfo::unban_shop())]
        pub fn unban_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(shop.status.is_banned(), Error::<T>::ShopNotBanned);

                let restored = ShopStatusBeforeBan::<T>::take(shop_id)
                    .unwrap_or(ShopOperatingStatus::Paused);
                ShopBanReason::<T>::remove(shop_id);
                shop.status = restored;

                Self::deposit_event(Event::ShopUnbannedByRoot { shop_id, restored_status: restored });
                Ok(())
            })
        }
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 获取 Shop 派生账户
        pub fn shop_account_id(shop_id: u64) -> T::AccountId {
            SHOP_PALLET_ID.into_sub_account_truncating(shop_id)
        }

        /// 检查是否有权管理 Shop
        pub fn can_manage_shop(shop: &ShopOf<T>, account: &T::AccountId) -> bool {
            // Entity owner 有权限
            if let Some(owner) = T::EntityProvider::entity_owner(shop.entity_id) {
                if &owner == account {
                    return true;
                }
            }
            // Entity admin 继承 Shop 管理权限（需 SHOP_MANAGE 权限）
            if T::EntityProvider::is_entity_admin(shop.entity_id, account, pallet_entity_common::AdminPermission::SHOP_MANAGE) {
                return true;
            }
            // Shop manager 有权限
            shop.managers.contains(account)
        }

        /// 应用 Option<Option<CID>> 三态语义到字段
        fn apply_triple_option_cid(
            field: &mut Option<BoundedVec<u8, T::MaxCidLength>>,
            input: &Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
        ) -> DispatchResult {
            match input {
                Some(Some(ref cid)) => {
                    ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                    *field = Some(cid.clone());
                }
                Some(None) => { *field = None; }
                None => {}
            }
            Ok(())
        }

        /// 关闭 Shop 的统一清理逻辑（finalize_close_shop / force_close_shop 共用）
        fn do_close_shop_cleanup(shop: &mut ShopOf<T>, shop_id: u64) {
            if let Some(owner) = T::EntityProvider::entity_owner(shop.entity_id) {
                Self::ipfs_unpin_all_shop_cids(shop, &owner);
            }
            // v1.2-C1: 店铺关闭时移除全部商品（退还押金 + unpin CID + 清理存储 + 更新统计）
            if let Err(e) = T::ProductProvider::force_remove_all_shop_products(shop_id) {
                log::error!(
                    target: "entity-shop",
                    "force_remove_all_shop_products failed for shop {}: {:?}",
                    shop_id, e
                );
            }

            shop.status = ShopOperatingStatus::Closed;

            ShopClosingAt::<T>::remove(shop_id);
            if let Err(e) = T::EntityProvider::unregister_shop(shop.entity_id, shop_id) {
                log::error!(
                    target: "entity-shop",
                    "unregister_shop failed for entity {} shop {}: {:?}",
                    shop.entity_id, shop_id, e
                );
            }
            ShopEntity::<T>::remove(shop_id);

            // 主店由 registry unregister_shop 自动重新指定，无需 shop 侧清理

            ShopStatusBeforeBan::<T>::remove(shop_id);
            ShopBanReason::<T>::remove(shop_id);
            PendingTransfers::<T>::remove(shop_id);

            // 委托 loyalty 模块清理积分数据
            T::PointsCleanup::cleanup_shop_points(shop_id);

            let shop_account = Self::shop_account_id(shop_id);
            let remaining = T::Currency::free_balance(&shop_account);
            if !remaining.is_zero() {
                if let Some(owner) = T::EntityProvider::entity_owner(shop.entity_id) {
                    if T::Currency::transfer(
                        &shop_account,
                        &owner,
                        remaining,
                        ExistenceRequirement::AllowDeath,
                    ).is_ok() {
                        Self::deposit_event(Event::ShopClosedFundRefunded {
                            shop_id,
                            to: owner,
                            amount: remaining,
                        });
                    }
                }
            }

            Self::deposit_event(Event::ShopClosed { shop_id });
        }

        // ==================== IPFS Pin/Unpin 辅助函数 ====================

        fn ipfs_pin_cid(
            caller: &T::AccountId,
            shop_id: u64,
            cid: &BoundedVec<u8, T::MaxCidLength>,
        ) {
            if cid.is_empty() { return; }
            let entity_id = ShopEntity::<T>::get(shop_id);
            if let Err(e) = T::StoragePin::pin(caller.clone(), b"shop", shop_id, entity_id, cid.to_vec(), cid.len() as u64, PinTier::Standard) {
                log::warn!(
                    target: "entity-shop",
                    "IPFS Pin failed for shop {}: {:?}", shop_id, e
                );
            }
        }

        fn ipfs_unpin_cid(caller: &T::AccountId, cid: &BoundedVec<u8, T::MaxCidLength>) {
            if cid.is_empty() { return; }
            if let Err(e) = T::StoragePin::unpin(caller.clone(), cid.to_vec()) {
                log::warn!(
                    target: "entity-shop",
                    "IPFS Unpin failed: {:?}", e
                );
            }
        }

        fn ipfs_unpin_optional(caller: &T::AccountId, cid: &Option<BoundedVec<u8, T::MaxCidLength>>) {
            if let Some(c) = cid {
                Self::ipfs_unpin_cid(caller, c);
            }
        }

        /// 处理 Option<Option<CID>> 三态语义的 ΔPin
        /// None = 不修改, Some(None) = 清除, Some(Some(cid)) = 设新值
        fn ipfs_handle_triple_option(
            caller: &T::AccountId,
            shop_id: u64,
            current: &Option<BoundedVec<u8, T::MaxCidLength>>,
            input: &Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
        ) {
            match input {
                Some(Some(new_cid)) => {
                    Self::ipfs_unpin_optional(caller, current);
                    Self::ipfs_pin_cid(caller, shop_id, new_cid);
                }
                Some(None) => {
                    Self::ipfs_unpin_optional(caller, current);
                }
                None => {}
            }
        }

        /// Unpin Shop 的所有 CID（关闭/封禁时调用）
        fn ipfs_unpin_all_shop_cids(shop: &ShopOf<T>, caller: &T::AccountId) {
            Self::ipfs_unpin_optional(caller, &shop.logo_cid);
            Self::ipfs_unpin_optional(caller, &shop.description_cid);
            Self::ipfs_unpin_optional(caller, &shop.address_cid);
            Self::ipfs_unpin_optional(caller, &shop.business_hours_cid);
            Self::ipfs_unpin_optional(caller, &shop.policies_cid);
        }

        fn do_create_shop_inner(
            entity_id: u64,
            name: BoundedVec<u8, T::MaxShopNameLength>,
            shop_type: ShopType,
            is_primary: bool,
        ) -> Result<u64, DispatchError> {
            let shop_id = NextShopId::<T>::get();
            ensure!(shop_id < u64::MAX, Error::<T>::ShopIdOverflow);
            let now = <frame_system::Pallet<T>>::block_number();

            let shop = Shop {
                id: shop_id,
                entity_id,
                name: name.clone(),
                logo_cid: None,
                description_cid: None,
                shop_type,
                status: ShopOperatingStatus::Active,
                managers: BoundedVec::default(),
                location: None,
                address_cid: None,
                business_hours_cid: None,
                policies_cid: None,
                created_at: now,
                product_count: 0,
                total_sales: Zero::zero(),
                total_orders: 0,
                rating: 0,
                rating_total: 0,
                rating_count: 0,
            };

            Shops::<T>::insert(shop_id, shop);
            ShopEntity::<T>::insert(shop_id, entity_id);
            NextShopId::<T>::put(shop_id.checked_add(1).ok_or(Error::<T>::ShopIdOverflow)?);

            // 回写 Entity 的 shop_ids（维护双向一致性）
            T::EntityProvider::register_shop(entity_id, shop_id)?;

            // 主店由 registry register_shop 自动处理（第一个 shop 自动成为 primary）
            // 若 is_primary=true 且不是第一个 shop，显式设置
            if is_primary {
                T::EntityProvider::set_primary_shop_id(entity_id, shop_id);
            }

            Self::deposit_event(Event::ShopCreated {
                shop_id,
                entity_id,
                name,
                shop_type,
            });

            Ok(shop_id)
        }

        pub fn do_create_shop(
            entity_id: u64,
            name: BoundedVec<u8, T::MaxShopNameLength>,
            shop_type: ShopType,
        ) -> Result<u64, DispatchError> {
            Self::do_create_shop_inner(entity_id, name, shop_type, false)
        }

        /// 获取 Shop 运营资金余额
        pub fn get_operating_balance(shop_id: u64) -> BalanceOf<T> {
            let shop_account = Self::shop_account_id(shop_id);
            T::Currency::free_balance(&shop_account)
        }
    }

    // ========================================================================
    // ShopProvider Trait 实现
    // ========================================================================

    impl<T: Config> ShopProvider<T::AccountId> for Pallet<T> {
        fn shop_exists(shop_id: u64) -> bool {
            Shops::<T>::contains_key(shop_id)
        }

        fn is_shop_active(shop_id: u64) -> bool {
            Shops::<T>::get(shop_id)
                .map(|s| {
                    s.status.is_operational()
                    && ShopEntity::<T>::get(shop_id)
                        .map(|eid| T::EntityProvider::is_entity_active(eid))
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        }

        fn shop_entity_id(shop_id: u64) -> Option<u64> {
            ShopEntity::<T>::get(shop_id)
        }

        fn shop_owner(shop_id: u64) -> Option<T::AccountId> {
            // 通过 Entity 查询 Shop 所有者
            ShopEntity::<T>::get(shop_id)
                .and_then(|entity_id| T::EntityProvider::entity_owner(entity_id))
        }

        fn shop_account(shop_id: u64) -> T::AccountId {
            Self::shop_account_id(shop_id)
        }

        fn shop_type(shop_id: u64) -> Option<ShopType> {
            Shops::<T>::get(shop_id).map(|s| s.shop_type)
        }

        fn is_shop_manager(shop_id: u64, account: &T::AccountId) -> bool {
            Shops::<T>::get(shop_id)
                .map(|s| Self::can_manage_shop(&s, account))
                .unwrap_or(false)
        }

        fn shop_own_status(shop_id: u64) -> Option<ShopOperatingStatus> {
            Shops::<T>::get(shop_id).map(|s| s.status)
        }

        fn effective_status(shop_id: u64) -> Option<EffectiveShopStatus> {
            let shop = Shops::<T>::get(shop_id)?;
            let entity_id = ShopEntity::<T>::get(shop_id)?;
            let entity_status = T::EntityProvider::entity_status(entity_id)
                .unwrap_or(EntityStatus::Pending);
            Some(EffectiveShopStatus::compute(&entity_status, &shop.status))
        }

        fn increment_product_count(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                shop.product_count = shop.product_count.saturating_add(1);
                Ok(())
            })
        }

        fn decrement_product_count(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                shop.product_count = shop.product_count.saturating_sub(1);
                Ok(())
            })
        }

        fn update_shop_stats(shop_id: u64, sales_amount: u128, order_count: u32) -> Result<(), DispatchError> {
            // M2-R3: 从 shop struct 获取 entity_id，避免冗余 ShopEntity 存储读
            let entity_id = Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<u64, DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                shop.total_sales = shop.total_sales.saturating_add(
                    sales_amount.saturated_into()
                );
                shop.total_orders = shop.total_orders.saturating_add(order_count);
                Ok(shop.entity_id)
            })?;

            // 级联更新 Entity 统计
            T::EntityProvider::update_entity_stats(entity_id, sales_amount, order_count)?;

            Ok(())
        }

        fn update_shop_rating(shop_id: u64, rating: u8) -> Result<(), DispatchError> {
            ensure!(rating >= 1 && rating <= 5, Error::<T>::InvalidRating);
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                let r = rating as u64;
                shop.rating_total = shop.rating_total.saturating_add(r.saturating_mul(100));
                shop.rating_count = shop.rating_count.saturating_add(1);
                shop.rating = (shop.rating_total / shop.rating_count as u64).min(500) as u16;
                Ok(())
            })
        }

        fn revert_shop_rating(shop_id: u64, old_rating: u8, new_rating: Option<u8>) -> Result<(), DispatchError> {
            ensure!(old_rating >= 1 && old_rating <= 5, Error::<T>::InvalidRating);
            if let Some(nr) = new_rating {
                ensure!(nr >= 1 && nr <= 5, Error::<T>::InvalidRating);
            }
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                let old_r = old_rating as u64;
                // 减去旧评分
                shop.rating_total = shop.rating_total.saturating_sub(old_r.saturating_mul(100));
                match new_rating {
                    Some(nr) => {
                        // 修改评价：替换评分，count 不变
                        let new_r = nr as u64;
                        shop.rating_total = shop.rating_total.saturating_add(new_r.saturating_mul(100));
                    },
                    None => {
                        // 删除评价：减少 count
                        shop.rating_count = shop.rating_count.saturating_sub(1);
                    },
                }
                // 重新计算平均分
                if shop.rating_count > 0 {
                    shop.rating = (shop.rating_total / shop.rating_count as u64).min(500) as u16;
                } else {
                    shop.rating = 0;
                    shop.rating_total = 0;
                }
                Ok(())
            })
        }

        fn deduct_operating_fund(shop_id: u64, amount: u128) -> Result<(), DispatchError> {
            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
            ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);

            let shop_account = Self::shop_account_id(shop_id);
            let balance = T::Currency::free_balance(&shop_account);
            let amount_balance: BalanceOf<T> = amount.saturated_into();

            // 偿付安全：扣费不得侵占已承诺的佣金资金（pending + shopping）
            let protected: BalanceOf<T> = T::CommissionFundGuard::protected_funds(shop.entity_id).saturated_into();
            let available = balance.saturating_sub(protected);
            ensure!(available >= amount_balance, Error::<T>::InsufficientOperatingFund);
            
            // H3-fix: 扣减运营资金 → 转给 Entity 账户
            // 使用 AllowDeath 以允许完全耗尽（KeepAlive 会阻止余额低于 ED，与 FundDepleted 逻辑冲突）
            let entity_account = T::EntityProvider::entity_account(shop.entity_id);
            T::Currency::transfer(
                &shop_account,
                &entity_account,
                amount_balance,
                ExistenceRequirement::AllowDeath,
            )?;
            
            let new_balance = T::Currency::free_balance(&shop_account);
            
            Self::deposit_event(Event::OperatingFundDeducted {
                shop_id,
                amount: amount_balance,
                new_balance,
            });
            
            // 检查是否低于阈值
            if new_balance < T::WarningThreshold::get() {
                Self::deposit_event(Event::FundWarning { shop_id, balance: new_balance });
            }
            
            // 检查是否耗尽
            if new_balance < T::MinOperatingBalance::get() {
                Shops::<T>::mutate(shop_id, |maybe_shop| {
                    if let Some(s) = maybe_shop {
                        s.status = ShopOperatingStatus::FundDepleted;
                    }
                });
                Self::deposit_event(Event::FundDepleted { shop_id });
            }
            
            Ok(())
        }

        fn operating_balance(shop_id: u64) -> u128 {
            Self::get_operating_balance(shop_id).saturated_into()
        }

        fn create_primary_shop(
            entity_id: u64,
            name: sp_std::vec::Vec<u8>,
            shop_type: ShopType,
        ) -> Result<u64, DispatchError> {
            let bounded_name: BoundedVec<u8, T::MaxShopNameLength> = name
                .try_into()
                .map_err(|_| Error::<T>::NameTooLong)?;

            Self::do_create_shop_inner(entity_id, bounded_name, shop_type, true)
        }

        fn is_primary_shop(shop_id: u64) -> bool {
            ShopEntity::<T>::get(shop_id)
                .map(|entity_id| T::EntityProvider::get_primary_shop_id(entity_id) == shop_id)
                .unwrap_or(false)
        }

        fn pause_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(shop.status == ShopOperatingStatus::Active, Error::<T>::ShopNotActive);
                shop.status = ShopOperatingStatus::Paused;
                Self::deposit_event(Event::ShopPaused { shop_id });
                Ok(())
            })
        }

        fn resume_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
                ensure!(shop.status.can_resume(), Error::<T>::ShopNotPaused);
                let shop_account = Self::shop_account_id(shop_id);
                let balance = T::Currency::free_balance(&shop_account);
                ensure!(balance >= T::MinOperatingBalance::get(), Error::<T>::InsufficientOperatingFund);
                shop.status = ShopOperatingStatus::Active;
                Self::deposit_event(Event::ShopResumed { shop_id });
                Ok(())
            })
        }

        fn force_pause_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
                ensure!(shop.status != ShopOperatingStatus::Paused, Error::<T>::ShopAlreadyPaused);
                shop.status = ShopOperatingStatus::Paused;
                Self::deposit_event(Event::ShopForcePaused { shop_id });
                Ok(())
            })
        }

        fn force_close_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);

                Self::do_close_shop_cleanup(shop, shop_id);
                Ok(())
            })
        }

        fn governance_close_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);

                Self::do_close_shop_cleanup(shop, shop_id);
                Ok(())
            })
        }

        fn governance_set_shop_type(shop_id: u64, new_type: ShopType) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);
                ensure!(shop.shop_type != new_type, Error::<T>::ShopTypeSame);

                let old_type = shop.shop_type;
                shop.shop_type = new_type;

                Self::deposit_event(Event::ShopTypeChanged { shop_id, old_type, new_type });
                Ok(())
            })
        }
    }
}
