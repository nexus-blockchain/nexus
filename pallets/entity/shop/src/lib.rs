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

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
        BoundedVec, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{
        CommissionFundGuard, EffectiveShopStatus, EntityProvider, EntityStatus,
        ShopOperatingStatus, ShopProvider, ShopType,
    };
    use sp_runtime::{
        traits::{AccountIdConversion, Saturating, Zero},
        DispatchError, SaturatedConversion,
    };
    use frame_support::weights::Weight;

    /// Shop 派生账户 PalletId
    const SHOP_PALLET_ID: PalletId = PalletId(*b"et/shop_");

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Shop 积分配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxNameLen, MaxSymbolLen))]
    pub struct PointsConfig<MaxNameLen: Get<u32>, MaxSymbolLen: Get<u32>> {
        /// 积分名称
        pub name: BoundedVec<u8, MaxNameLen>,
        /// 积分符号
        pub symbol: BoundedVec<u8, MaxSymbolLen>,
        /// 购物返积分比例（基点，500 = 5%）
        pub reward_rate: u16,
        /// 积分兑换比例（基点，1000 = 10%）
        pub exchange_rate: u16,
        /// 积分是否可转让
        pub transferable: bool,
    }

    /// 积分配置类型别名
    pub type PointsConfigOf<T> = PointsConfig<
        <T as Config>::MaxPointsNameLength,
        <T as Config>::MaxPointsSymbolLength,
    >;

    /// Shop 结构体
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxNameLen, MaxCidLen, MaxManagers))]
    pub struct Shop<AccountId, Balance, BlockNumber, MaxNameLen: Get<u32>, MaxCidLen: Get<u32>, MaxManagers: Get<u32>> {
        /// Shop ID（全局唯一）
        pub id: u64,
        /// 所属 Entity ID
        pub entity_id: u64,
        /// 是否为主 Shop（每个 Entity 有且仅有一个，不可关闭）
        pub is_primary: bool,
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
        /// 客服账户
        pub customer_service: Option<AccountId>,
        /// 初始运营资金
        pub initial_fund: Balance,
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

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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

        /// 积分名称最大长度
        #[pallet::constant]
        type MaxPointsNameLength: Get<u32>;

        /// 积分符号最大长度
        #[pallet::constant]
        type MaxPointsSymbolLength: Get<u32>;

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

    /// Shop 积分配置 shop_id -> PointsConfig
    #[pallet::storage]
    #[pallet::getter(fn shop_points_config)]
    pub type ShopPointsConfigs<T: Config> = StorageMap<_, Blake2_128Concat, u64, PointsConfigOf<T>>;

    /// Shop 积分余额 (shop_id, account) -> balance
    #[pallet::storage]
    #[pallet::getter(fn shop_points_balance)]
    pub type ShopPointsBalances<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// Shop 积分总供应量 shop_id -> total_supply
    #[pallet::storage]
    #[pallet::getter(fn shop_points_total_supply)]
    pub type ShopPointsTotalSupply<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

    /// Shop 积分有效期（区块数，0=永不过期）shop_id -> ttl_blocks
    #[pallet::storage]
    #[pallet::getter(fn shop_points_ttl)]
    pub type ShopPointsTtl<T: Config> = StorageMap<_, Blake2_128Concat, u64, BlockNumberFor<T>, ValueQuery>;

    /// 用户积分到期时间 (shop_id, account) -> expires_at_block
    #[pallet::storage]
    #[pallet::getter(fn shop_points_expires_at)]
    pub type ShopPointsExpiresAt<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BlockNumberFor<T>,
    >;

    /// Shop 积分总量上限 shop_id -> max_supply（0=无上限）
    #[pallet::storage]
    #[pallet::getter(fn shop_points_max_supply)]
    pub type ShopPointsMaxSupply<T: Config> = StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

    /// Entity 主 Shop 索引 entity_id -> primary_shop_id
    #[pallet::storage]
    #[pallet::getter(fn entity_primary_shop)]
    pub type EntityPrimaryShop<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64>;

    /// 封禁前的 Shop 状态（用于解封时恢复）shop_id -> ShopOperatingStatus
    #[pallet::storage]
    pub type ShopStatusBeforeBan<T: Config> = StorageMap<_, Blake2_128Concat, u64, ShopOperatingStatus>;

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
        /// Shop 积分启用
        ShopPointsEnabled { shop_id: u64, name: BoundedVec<u8, T::MaxPointsNameLength> },
        /// Shop 积分禁用
        ShopPointsDisabled { shop_id: u64 },
        /// Shop 积分发放
        PointsIssued { shop_id: u64, to: T::AccountId, amount: BalanceOf<T> },
        /// Shop 积分销毁
        PointsBurned { shop_id: u64, from: T::AccountId, amount: BalanceOf<T> },
        /// Shop 积分转移
        PointsTransferred { shop_id: u64, from: T::AccountId, to: T::AccountId, amount: BalanceOf<T> },
        /// 资金预警
        FundWarning { shop_id: u64, balance: BalanceOf<T> },
        /// 资金耗尽
        FundDepleted { shop_id: u64 },
        /// Shop 运营资金提取
        OperatingFundWithdrawn { shop_id: u64, to: T::AccountId, amount: BalanceOf<T>, new_balance: BalanceOf<T> },
        /// Shop 关闭时资金退还
        ShopClosedFundRefunded { shop_id: u64, to: T::AccountId, amount: BalanceOf<T> },
        /// Shop 客服账户更新
        CustomerServiceUpdated { shop_id: u64, customer_service: Option<T::AccountId> },
        /// Shop 进入关闭宽限期
        ShopClosing { shop_id: u64, grace_until: BlockNumberFor<T> },
        /// Shop 关闭完成（宽限期满）
        ShopCloseFinalized { shop_id: u64 },
        /// 积分兑换
        PointsRedeemed { shop_id: u64, who: T::AccountId, points_burned: BalanceOf<T>, payout: BalanceOf<T> },
        /// Shop 转让
        ShopTransferred { shop_id: u64, from_entity_id: u64, to_entity_id: u64 },
        /// 主 Shop 变更
        PrimaryShopChanged { entity_id: u64, old_shop_id: u64, new_shop_id: u64 },
        /// Shop 被强制暂停（Root）
        ShopForcePaused { shop_id: u64 },
        /// 积分有效期设置
        PointsTtlSet { shop_id: u64, ttl_blocks: BlockNumberFor<T> },
        /// 积分过期清除
        PointsExpired { shop_id: u64, account: T::AccountId, amount: BalanceOf<T> },
        /// Shop 被 Root 强制关闭
        ShopForceClosedByRoot { shop_id: u64 },
        /// Shop 营业时间更新
        ShopBusinessHoursUpdated { shop_id: u64 },
        /// Shop 政策更新
        ShopPoliciesUpdated { shop_id: u64 },
        /// Shop 类型变更
        ShopTypeChanged { shop_id: u64, old_type: ShopType, new_type: ShopType },
        /// Shop 关闭撤回
        ShopClosingCancelled { shop_id: u64 },
        /// 积分总量上限设置
        PointsMaxSupplySet { shop_id: u64, max_supply: BalanceOf<T> },
        /// Manager 自我辞职
        ManagerResigned { shop_id: u64, manager: T::AccountId },
        /// Shop 被封禁（Root）
        ShopBannedByRoot { shop_id: u64 },
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
        /// 积分未启用
        PointsNotEnabled,
        /// 积分已启用
        PointsAlreadyEnabled,
        /// 积分不可转让
        PointsNotTransferable,
        /// 积分余额不足
        InsufficientPointsBalance,
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
        /// 积分名称不能为空
        PointsNameEmpty,
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
        /// 积分未过期（无法清除）
        PointsNotExpired,
        /// 兑换金额为零（积分数量太小）
        RedeemPayoutZero,
        /// Shop 类型未变更
        ShopTypeSame,
        /// 积分总量超过上限
        PointsMaxSupplyExceeded,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// Entity 的 Shop 数量已达上限
        ShopLimitReached,
        /// Shop 已被封禁
        ShopBanned,
        /// Shop 未被封禁
        ShopNotBanned,
        /// 调用者不是该 Shop 的管理员
        NotManager,
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
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
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
            
            // 先创建 Shop（获取分配的 shop_id）
            let shop_id = Self::do_create_shop(entity_id, name, shop_type, initial_fund)?;
            
            // 再转移初始资金（Shop 已创建，转账失败会回滚整个 extrinsic）
            if !initial_fund.is_zero() {
                let shop_account = Self::shop_account_id(shop_id);
                T::Currency::transfer(&who, &shop_account, initial_fund, ExistenceRequirement::KeepAlive)?;
            }
            
            Ok(())
        }

        /// 更新 Shop 信息
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn update_shop(
            origin: OriginFor<T>,
            shop_id: u64,
            name: Option<BoundedVec<u8, T::MaxShopNameLength>>,
            logo_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
            description_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // L1-R3: 至少需要修改一个字段
            ensure!(
                name.is_some() || logo_cid.is_some() || description_cid.is_some(),
                Error::<T>::InvalidConfig
            );

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                
                // H3: 已关闭/关闭中/封禁的 Shop 不可修改
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

                if let Some(n) = name {
                    // H2: 名称不能为空
                    ensure!(!n.is_empty(), Error::<T>::ShopNameEmpty);
                    shop.name = n;
                }
                // Option<Option<CID>>: None=不修改, Some(None)=清除, Some(Some(cid))=设新值
                match logo_cid {
                    Some(Some(cid)) => {
                        ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                        shop.logo_cid = Some(cid);
                    }
                    Some(None) => { shop.logo_cid = None; }
                    None => {}
                }
                match description_cid {
                    Some(Some(cid)) => {
                        ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                        shop.description_cid = Some(cid);
                    }
                    Some(None) => { shop.description_cid = None; }
                    None => {}
                }
                
                Self::deposit_event(Event::ShopUpdated { shop_id });
                Ok(())
            })
        }

        /// 添加 Shop 管理员
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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

                // H3: 已关闭/关闭中的 Shop 不可操作管理员
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                
                // 检查是否已存在
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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

                // H3: 已关闭/关闭中的 Shop 不可操作管理员
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                
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
        #[pallet::weight(Weight::from_parts(200_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(175_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn set_location(
            origin: OriginFor<T>,
            shop_id: u64,
            location: Option<(i64, i64)>,
            address_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
            business_hours_cid: Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
                
                // 验证位置（经度 -180~180，纬度 -90~90，精度 10^6）
                if let Some((lng, lat)) = location {
                    ensure!(lng >= -180_000_000 && lng <= 180_000_000, Error::<T>::InvalidLocation);
                    ensure!(lat >= -90_000_000 && lat <= 90_000_000, Error::<T>::InvalidLocation);
                }
                
                shop.location = location;
                // Option<Option<CID>>: None=不修改, Some(None)=清除, Some(Some(cid))=设新值
                match address_cid {
                    Some(Some(cid)) => {
                        ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                        shop.address_cid = Some(cid);
                    }
                    Some(None) => { shop.address_cid = None; }
                    None => {}
                }
                match business_hours_cid {
                    Some(Some(cid)) => {
                        ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                        shop.business_hours_cid = Some(cid);
                    }
                    Some(None) => { shop.business_hours_cid = None; }
                    None => {}
                }
                
                Self::deposit_event(Event::ShopLocationUpdated { shop_id, location });
                Ok(())
            })
        }

        /// 启用 Shop 积分
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn enable_points(
            origin: OriginFor<T>,
            shop_id: u64,
            name: BoundedVec<u8, T::MaxPointsNameLength>,
            symbol: BoundedVec<u8, T::MaxPointsSymbolLength>,
            reward_rate: u16,
            exchange_rate: u16,
            transferable: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            
            // 检查权限
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            
            // 检查是否已启用
            ensure!(!ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsAlreadyEnabled);
            
            // L2: 积分名称和符号不能为空（使用专用错误码）
            ensure!(!name.is_empty(), Error::<T>::PointsNameEmpty);
            ensure!(!symbol.is_empty(), Error::<T>::InvalidConfig);

            // 验证配置
            ensure!(reward_rate <= 10000 && exchange_rate <= 10000, Error::<T>::InvalidConfig);
            
            let config = PointsConfig {
                name: name.clone(),
                symbol,
                reward_rate,
                exchange_rate,
                transferable,
            };
            
            ShopPointsConfigs::<T>::insert(shop_id, config);
            
            Self::deposit_event(Event::ShopPointsEnabled { shop_id, name });
            Ok(())
        }

        /// 关闭 Shop（进入宽限期）
        ///
        /// 将 Shop 状态设为 Closing，开始宽限期倒计时。
        /// 宽限期内：不可接新单/上新品，已有订单可完成，积分可转移。
        /// 宽限期满后调用 `finalize_close_shop` 完成清理。
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
                
                // 主 Shop 不可关闭
                ensure!(!shop.is_primary, Error::<T>::CannotClosePrimaryShop);
                
                // 检查状态
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                ensure!(shop.status != ShopOperatingStatus::Closing, Error::<T>::ShopAlreadyClosing);
                
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
        #[pallet::weight(Weight::from_parts(200_000_000, 12_000))]
        pub fn finalize_close_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                ensure!(shop.status == ShopOperatingStatus::Closing, Error::<T>::ShopNotClosing);

                // 检查宽限期
                let closing_at = ShopClosingAt::<T>::get(shop_id)
                    .ok_or(Error::<T>::ShopNotFound)?;
                let now = <frame_system::Pallet<T>>::block_number();
                let grace = T::ShopClosingGracePeriod::get();
                ensure!(now >= closing_at.saturating_add(grace), Error::<T>::ClosingGracePeriodNotElapsed);

                shop.status = ShopOperatingStatus::Closed;

                // 清理 closing 计时器
                ShopClosingAt::<T>::remove(shop_id);

                // 注销 Entity-Shop 关联
                T::EntityProvider::unregister_shop(shop.entity_id, shop_id)?;

                // 清理 ShopEntity 反向索引
                ShopEntity::<T>::remove(shop_id);

                // 清理积分数据
                ShopPointsConfigs::<T>::remove(shop_id);
                let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, u32::MAX, None);
                ShopPointsTotalSupply::<T>::remove(shop_id);
                ShopPointsTtl::<T>::remove(shop_id);
                let _ = ShopPointsExpiresAt::<T>::clear_prefix(shop_id, u32::MAX, None);
                ShopPointsMaxSupply::<T>::remove(shop_id);

                // 退还 shop_account 余额给 entity_owner
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

                Self::deposit_event(Event::ShopCloseFinalized { shop_id });
                Self::deposit_event(Event::ShopClosed { shop_id });
                Ok(())
            })
        }

        /// 禁用 Shop 积分
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn disable_points(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            // M3: 关闭中/封禁的 Shop 不可禁用积分
            ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            ensure!(ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsNotEnabled);

            ShopPointsConfigs::<T>::remove(shop_id);
            // H2: 清理积分余额和总供应量，避免残留数据
            let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, u32::MAX, None);
            ShopPointsTotalSupply::<T>::remove(shop_id);
            // M1: 清理 TTL 相关存储
            ShopPointsTtl::<T>::remove(shop_id);
            let _ = ShopPointsExpiresAt::<T>::clear_prefix(shop_id, u32::MAX, None);
            // 清理积分总量上限
            ShopPointsMaxSupply::<T>::remove(shop_id);

            Self::deposit_event(Event::ShopPointsDisabled { shop_id });
            Ok(())
        }

        /// 更新积分配置
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn update_points_config(
            origin: OriginFor<T>,
            shop_id: u64,
            reward_rate: Option<u16>,
            exchange_rate: Option<u16>,
            transferable: Option<bool>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // L2-R3: 至少需要修改一个字段
            ensure!(
                reward_rate.is_some() || exchange_rate.is_some() || transferable.is_some(),
                Error::<T>::InvalidConfig
            );

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

            ShopPointsConfigs::<T>::try_mutate(shop_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::PointsNotEnabled)?;

                if let Some(rate) = reward_rate {
                    ensure!(rate <= 10000, Error::<T>::InvalidConfig);
                    config.reward_rate = rate;
                }
                if let Some(rate) = exchange_rate {
                    ensure!(rate <= 10000, Error::<T>::InvalidConfig);
                    config.exchange_rate = rate;
                }
                if let Some(t) = transferable {
                    config.transferable = t;
                }

                Self::deposit_event(Event::ShopUpdated { shop_id });
                Ok(())
            })
        }

        /// 转移 Shop 积分（用户之间）
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(150_000_000, 12_000))]
        pub fn transfer_points(
            origin: OriginFor<T>,
            shop_id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);
            ensure!(who != to, Error::<T>::InvalidConfig);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);

            let config = ShopPointsConfigs::<T>::get(shop_id)
                .ok_or(Error::<T>::PointsNotEnabled)?;
            ensure!(config.transferable, Error::<T>::PointsNotTransferable);

            // 懒过期检查
            Self::check_points_expiry(shop_id, &who);

            let from_balance = ShopPointsBalances::<T>::get(shop_id, &who);
            ensure!(from_balance >= amount, Error::<T>::InsufficientPointsBalance);

            ShopPointsBalances::<T>::mutate(shop_id, &who, |b| *b = b.saturating_sub(amount));
            ShopPointsBalances::<T>::mutate(shop_id, &to, |b| *b = b.saturating_add(amount));

            Self::deposit_event(Event::PointsTransferred {
                shop_id,
                from: who,
                to,
                amount,
            });
            Ok(())
        }

        /// 提取运营资金
        ///
        /// 仅 Entity owner 可调用，将 shop_account 中的运营资金提取到个人账户。
        /// - 活跃 Shop: 提取后余额不得低于 MinOperatingBalance
        /// - 已关闭 Shop: 无最低余额限制，可全额提取
        /// - 佣金保护: 不得侵占已承诺的佣金资金 (pending + shopping)
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(200_000_000, 8_000))]
        pub fn withdraw_operating_fund(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::ZeroWithdrawAmount);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;

            // 仅 Entity owner 可提取（不允许 manager）
            let owner = T::EntityProvider::entity_owner(shop.entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(who == owner, Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);

            let shop_account = Self::shop_account_id(shop_id);
            let balance = T::Currency::free_balance(&shop_account);

            // 佣金保护：不得侵占已承诺的佣金资金
            let protected: BalanceOf<T> = T::CommissionFundGuard::protected_funds(shop.entity_id).saturated_into();
            let available = balance.saturating_sub(protected);
            ensure!(available >= amount, Error::<T>::InsufficientOperatingFund);

            // 活跃 Shop 检查最低余额
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

        /// 设置/清除 Shop 客服账户
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
        pub fn set_customer_service(
            origin: OriginFor<T>,
            shop_id: u64,
            customer_service: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

                shop.customer_service = customer_service.clone();

                Self::deposit_event(Event::CustomerServiceUpdated { shop_id, customer_service });
                Ok(())
            })
        }

        /// Manager 直接发放积分
        #[pallet::call_index(16)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn manager_issue_points(
            origin: OriginFor<T>,
            shop_id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            ensure!(ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsNotEnabled);

            // 积分总量上限检查
            Self::check_points_max_supply(shop_id, amount)?;

            ShopPointsBalances::<T>::mutate(shop_id, &to, |b| *b = b.saturating_add(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_add(amount));

            Self::maybe_extend_points_expiry(shop_id, &to);

            Self::deposit_event(Event::PointsIssued { shop_id, to, amount });
            Ok(())
        }

        /// Manager 直接销毁积分
        #[pallet::call_index(17)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn manager_burn_points(
            origin: OriginFor<T>,
            shop_id: u64,
            from: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
            ensure!(ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsNotEnabled);

            // 懒过期检查
            Self::check_points_expiry(shop_id, &from);

            let balance = ShopPointsBalances::<T>::get(shop_id, &from);
            ensure!(balance >= amount, Error::<T>::InsufficientPointsBalance);

            ShopPointsBalances::<T>::mutate(shop_id, &from, |b| *b = b.saturating_sub(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_sub(amount));

            Self::deposit_event(Event::PointsBurned { shop_id, from, amount });
            Ok(())
        }

        /// 用户兑换积分为货币
        ///
        /// 按 exchange_rate（基点）计算：payout = amount * exchange_rate / 10000
        /// 货币从 Shop 运营资金账户支出。
        #[pallet::call_index(18)]
        #[pallet::weight(Weight::from_parts(200_000_000, 12_000))]
        pub fn redeem_points(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);

            let config = ShopPointsConfigs::<T>::get(shop_id)
                .ok_or(Error::<T>::PointsNotEnabled)?;
            ensure!(config.exchange_rate > 0, Error::<T>::InvalidConfig);

            // 懒过期检查
            Self::check_points_expiry(shop_id, &who);

            let balance = ShopPointsBalances::<T>::get(shop_id, &who);
            ensure!(balance >= amount, Error::<T>::InsufficientPointsBalance);

            // 计算兑换金额
            let rate: BalanceOf<T> = (config.exchange_rate as u128).saturated_into();
            let divisor: BalanceOf<T> = 10000u128.saturated_into();
            let payout = amount.saturating_mul(rate) / divisor;
            ensure!(!payout.is_zero(), Error::<T>::RedeemPayoutZero);

            // 从 Shop 运营账户转给用户
            let shop_account = Self::shop_account_id(shop_id);
            T::Currency::transfer(&shop_account, &who, payout, ExistenceRequirement::AllowDeath)?;

            // 销毁积分
            ShopPointsBalances::<T>::mutate(shop_id, &who, |b| *b = b.saturating_sub(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_sub(amount));

            Self::deposit_event(Event::PointsRedeemed { shop_id, who, points_burned: amount, payout });
            Ok(())
        }

        /// 转让 Shop 到另一个 Entity
        ///
        /// 仅 Entity owner 可调用。主 Shop 不可转让。
        /// Shop 运营资金随 Shop 账户自动转移。
        #[pallet::call_index(19)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn transfer_shop(
            origin: OriginFor<T>,
            shop_id: u64,
            to_entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                let from_entity_id = shop.entity_id;

                // 仅 Entity owner 可转让
                let owner = T::EntityProvider::entity_owner(from_entity_id)
                    .ok_or(Error::<T>::EntityNotFound)?;
                ensure!(who == owner, Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(from_entity_id), Error::<T>::EntityLocked);

                ensure!(!shop.is_primary, Error::<T>::CannotTransferPrimaryShop);
                ensure!(from_entity_id != to_entity_id, Error::<T>::SameEntity);
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);

                // 目标 Entity 必须存在且激活
                ensure!(T::EntityProvider::entity_exists(to_entity_id), Error::<T>::EntityNotFound);
                ensure!(T::EntityProvider::is_entity_active(to_entity_id), Error::<T>::EntityNotActive);

                // 注销原 Entity 关联，注册到新 Entity
                T::EntityProvider::unregister_shop(from_entity_id, shop_id)?;
                T::EntityProvider::register_shop(to_entity_id, shop_id)?;

                shop.entity_id = to_entity_id;
                ShopEntity::<T>::insert(shop_id, to_entity_id);

                Self::deposit_event(Event::ShopTransferred { shop_id, from_entity_id, to_entity_id });
                Ok(())
            })
        }

        /// 更换 Entity 的主 Shop
        ///
        /// 仅 Entity owner 可调用。新主 Shop 必须属于同一 Entity 且处于活跃状态。
        #[pallet::call_index(20)]
        #[pallet::weight(Weight::from_parts(200_000_000, 12_000))]
        pub fn set_primary_shop(
            origin: OriginFor<T>,
            entity_id: u64,
            new_primary_shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(who == owner, Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let new_shop = Shops::<T>::get(new_primary_shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(new_shop.entity_id == entity_id, Error::<T>::NotAuthorized);
            ensure!(!new_shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
            ensure!(!new_shop.is_primary, Error::<T>::InvalidConfig);

            // 通过 EntityPrimaryShop 索引 O(1) 查找当前主 Shop
            let old_primary_id = EntityPrimaryShop::<T>::get(entity_id).unwrap_or(0);

            if old_primary_id > 0 {
                Shops::<T>::mutate(old_primary_id, |maybe| {
                    if let Some(s) = maybe.as_mut() {
                        s.is_primary = false;
                    }
                });
            }

            // 设置新主 Shop
            Shops::<T>::mutate(new_primary_shop_id, |maybe| {
                if let Some(s) = maybe.as_mut() {
                    s.is_primary = true;
                }
            });
            EntityPrimaryShop::<T>::insert(entity_id, new_primary_shop_id);

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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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

        /// 设置 Shop 积分有效期（TTL）
        ///
        /// ttl_blocks = 0 表示永不过期（移除 TTL 限制）。
        /// 新 TTL 仅影响后续发放的积分。
        #[pallet::call_index(22)]
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
        pub fn set_points_ttl(
            origin: OriginFor<T>,
            shop_id: u64,
            ttl_blocks: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            ensure!(ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsNotEnabled);

            if ttl_blocks.is_zero() {
                ShopPointsTtl::<T>::remove(shop_id);
            } else {
                ShopPointsTtl::<T>::insert(shop_id, ttl_blocks);
            }

            Self::deposit_event(Event::PointsTtlSet { shop_id, ttl_blocks });
            Ok(())
        }

        /// 清除过期积分
        ///
        /// 任何人可调用。仅当积分确实已过期时才执行清除。
        #[pallet::call_index(23)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn expire_points(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            ensure!(Shops::<T>::contains_key(shop_id), Error::<T>::ShopNotFound);

            let expiry = ShopPointsExpiresAt::<T>::get(shop_id, &account)
                .ok_or(Error::<T>::PointsNotExpired)?;

            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now > expiry, Error::<T>::PointsNotExpired);

            let expired_amount = ShopPointsBalances::<T>::take(shop_id, &account);
            if !expired_amount.is_zero() {
                ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_sub(expired_amount));
                Self::deposit_event(Event::PointsExpired {
                    shop_id,
                    account: account.clone(),
                    amount: expired_amount,
                });
            }
            ShopPointsExpiresAt::<T>::remove(shop_id, &account);

            Ok(())
        }

        /// Root 强制关闭 Shop
        ///
        /// 跳过宽限期，立即关闭并清理。委托给 ShopProvider::force_close_shop。
        #[pallet::call_index(24)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
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

        /// 设置 Shop 营业时间（独立于位置信息）
        #[pallet::call_index(25)]
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
        pub fn set_business_hours(
            origin: OriginFor<T>,
            shop_id: u64,
            business_hours_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

                match business_hours_cid {
                    Some(cid) => {
                        ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                        shop.business_hours_cid = Some(cid);
                    }
                    None => { shop.business_hours_cid = None; }
                }

                Self::deposit_event(Event::ShopBusinessHoursUpdated { shop_id });
                Ok(())
            })
        }

        /// 设置/清除 Shop 政策（退换货/运费/服务条款）
        #[pallet::call_index(26)]
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
        pub fn set_shop_policies(
            origin: OriginFor<T>,
            shop_id: u64,
            policies_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;

                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
                ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
                ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);

                match policies_cid {
                    Some(cid) => {
                        ensure!(!cid.is_empty(), Error::<T>::EmptyCid);
                        shop.policies_cid = Some(cid);
                    }
                    None => { shop.policies_cid = None; }
                }

                Self::deposit_event(Event::ShopPoliciesUpdated { shop_id });
                Ok(())
            })
        }

        /// 变更 Shop 类型
        ///
        /// 仅 Entity owner 可调用。已关闭/关闭中的 Shop 不可变更。
        #[pallet::call_index(27)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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

        /// 设置积分总量上限
        ///
        /// max_supply = 0 表示无上限。已有供应量超过新上限时拒绝。
        #[pallet::call_index(29)]
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
        pub fn set_points_max_supply(
            origin: OriginFor<T>,
            shop_id: u64,
            max_supply: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(shop.entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(shop.entity_id), Error::<T>::EntityNotActive);
            ensure!(!shop.status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            ensure!(ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsNotEnabled);

            // 如果设置上限，当前供应量不得超过
            if !max_supply.is_zero() {
                let current_supply = ShopPointsTotalSupply::<T>::get(shop_id);
                ensure!(current_supply <= max_supply, Error::<T>::PointsMaxSupplyExceeded);
            }

            if max_supply.is_zero() {
                ShopPointsMaxSupply::<T>::remove(shop_id);
            } else {
                ShopPointsMaxSupply::<T>::insert(shop_id, max_supply);
            }

            Self::deposit_event(Event::PointsMaxSupplySet { shop_id, max_supply });
            Ok(())
        }

        /// Manager 自我辞职
        ///
        /// 允许 Shop Manager 主动从管理员列表中移除自己。
        /// Entity Owner 和 Entity Admin 不受此影响（他们的权限来自 Entity 层）。
        #[pallet::call_index(30)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn ban_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
                ensure!(!shop.status.is_banned(), Error::<T>::ShopBanned);

                // 记录封禁前状态，解封时恢复
                ShopStatusBeforeBan::<T>::insert(shop_id, shop.status);
                shop.status = ShopOperatingStatus::Banned;

                Self::deposit_event(Event::ShopBannedByRoot { shop_id });
                Ok(())
            })
        }

        /// 解封 Shop（Root/治理层调用）
        ///
        /// 恢复为封禁前的状态（Active/Paused/FundDepleted）。
        #[pallet::call_index(32)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn unban_shop(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                ensure!(shop.status.is_banned(), Error::<T>::ShopNotBanned);

                // 恢复封禁前状态，默认 Paused（安全）
                let restored = ShopStatusBeforeBan::<T>::take(shop_id)
                    .unwrap_or(ShopOperatingStatus::Paused);
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

        /// 检查并清除过期积分（懒过期）
        fn check_points_expiry(shop_id: u64, account: &T::AccountId) {
            if let Some(expiry) = ShopPointsExpiresAt::<T>::get(shop_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                if now > expiry {
                    let expired = ShopPointsBalances::<T>::take(shop_id, account);
                    if !expired.is_zero() {
                        ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_sub(expired));
                        Self::deposit_event(Event::PointsExpired {
                            shop_id,
                            account: account.clone(),
                            amount: expired,
                        });
                    }
                    ShopPointsExpiresAt::<T>::remove(shop_id, account);
                }
            }
        }

        /// 检查积分发行是否超过总量上限
        fn check_points_max_supply(shop_id: u64, amount: BalanceOf<T>) -> DispatchResult {
            let max_supply = ShopPointsMaxSupply::<T>::get(shop_id);
            if !max_supply.is_zero() {
                let current = ShopPointsTotalSupply::<T>::get(shop_id);
                ensure!(current.saturating_add(amount) <= max_supply, Error::<T>::PointsMaxSupplyExceeded);
            }
            Ok(())
        }

        /// 延长积分有效期（发放积分时调用）
        fn maybe_extend_points_expiry(shop_id: u64, account: &T::AccountId) {
            let ttl = ShopPointsTtl::<T>::get(shop_id);
            if !ttl.is_zero() {
                let now = <frame_system::Pallet<T>>::block_number();
                let new_expiry = now.saturating_add(ttl);
                // 取当前到期时间和新到期时间的较大值（滑动窗口延长）
                let final_expiry = match ShopPointsExpiresAt::<T>::get(shop_id, account) {
                    Some(current) if current > new_expiry => current,
                    _ => new_expiry,
                };
                ShopPointsExpiresAt::<T>::insert(shop_id, account, final_expiry);
            }
        }

        /// 发放积分（供外部模块调用，如订单完成后返积分）
        pub fn issue_points(
            shop_id: u64,
            to: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);
            // M5: 已关闭/关闭中的 Shop 不可发放积分
            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(!shop.status.is_closed_or_closing(), Error::<T>::ShopAlreadyClosed);
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

            // 积分总量上限检查
            Self::check_points_max_supply(shop_id, amount)?;

            ShopPointsBalances::<T>::mutate(shop_id, to, |b| *b = b.saturating_add(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_add(amount));

            Self::maybe_extend_points_expiry(shop_id, to);

            Self::deposit_event(Event::PointsIssued {
                shop_id,
                to: to.clone(),
                amount,
            });
            Ok(())
        }

        /// 销毁积分（供外部模块调用，如积分兑换消费）
        pub fn burn_points(
            shop_id: u64,
            from: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);
            // M5: 已关闭的 Shop 不可销毁积分
            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

            // 懒过期检查
            Self::check_points_expiry(shop_id, from);

            let balance = ShopPointsBalances::<T>::get(shop_id, from);
            ensure!(balance >= amount, Error::<T>::InsufficientPointsBalance);

            ShopPointsBalances::<T>::mutate(shop_id, from, |b| *b = b.saturating_sub(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_sub(amount));

            Self::deposit_event(Event::PointsBurned {
                shop_id,
                from: from.clone(),
                amount,
            });
            Ok(())
        }

        /// 创建 Shop 内部实现（公共核心）
        fn do_create_shop_inner(
            entity_id: u64,
            name: BoundedVec<u8, T::MaxShopNameLength>,
            shop_type: ShopType,
            initial_fund: BalanceOf<T>,
            is_primary: bool,
        ) -> Result<u64, DispatchError> {
            let shop_id = NextShopId::<T>::get();
            ensure!(shop_id < u64::MAX, Error::<T>::ShopIdOverflow);
            let now = <frame_system::Pallet<T>>::block_number();

            let shop = Shop {
                id: shop_id,
                entity_id,
                is_primary,
                name: name.clone(),
                logo_cid: None,
                description_cid: None,
                shop_type,
                status: ShopOperatingStatus::Active,
                managers: BoundedVec::default(),
                customer_service: None,
                initial_fund,
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
            if is_primary {
                EntityPrimaryShop::<T>::insert(entity_id, shop_id);
            }
            NextShopId::<T>::put(shop_id + 1);

            // 回写 Entity 的 shop_ids（维护双向一致性）
            T::EntityProvider::register_shop(entity_id, shop_id)?;

            Self::deposit_event(Event::ShopCreated {
                shop_id,
                entity_id,
                name,
                shop_type,
            });

            Ok(shop_id)
        }

        /// 创建普通 Shop（extrinsic 调用入口）
        pub fn do_create_shop(
            entity_id: u64,
            name: BoundedVec<u8, T::MaxShopNameLength>,
            shop_type: ShopType,
            initial_fund: BalanceOf<T>,
        ) -> Result<u64, DispatchError> {
            Self::do_create_shop_inner(entity_id, name, shop_type, initial_fund, false)
        }

        /// 获取 Shop 运营资金余额
        pub fn get_operating_balance(shop_id: u64) -> BalanceOf<T> {
            let shop_account = Self::shop_account_id(shop_id);
            T::Currency::free_balance(&shop_account)
        }

        // ================================================================
        // 积分查询 Runtime API 辅助方法
        // ================================================================

        /// 查询用户在指定 Shop 的积分余额
        pub fn get_points_balance(shop_id: u64, account: &T::AccountId) -> BalanceOf<T> {
            ShopPointsBalances::<T>::get(shop_id, account)
        }

        /// 查询 Shop 积分总供应量
        pub fn get_points_total_supply(shop_id: u64) -> BalanceOf<T> {
            ShopPointsTotalSupply::<T>::get(shop_id)
        }

        /// 查询 Shop 积分配置
        pub fn get_points_config(shop_id: u64) -> Option<PointsConfigOf<T>> {
            ShopPointsConfigs::<T>::get(shop_id)
        }

        /// 查询用户积分到期区块
        pub fn get_points_expiry(shop_id: u64, account: &T::AccountId) -> Option<BlockNumberFor<T>> {
            ShopPointsExpiresAt::<T>::get(shop_id, account)
        }

        /// 查询 Shop 积分总量上限
        pub fn get_points_max_supply(shop_id: u64) -> BalanceOf<T> {
            ShopPointsMaxSupply::<T>::get(shop_id)
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
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                // 输入校验：rating 范围 0-5，精度 *100 存入 rating_total
                let clamped_rating = rating.min(5) as u64;
                shop.rating_total = shop.rating_total.saturating_add(clamped_rating.saturating_mul(100));
                shop.rating_count = shop.rating_count.saturating_add(1);
                // rating = rating_total / rating_count，无精度损失累积
                if shop.rating_count > 0 {
                    shop.rating = (shop.rating_total / shop.rating_count as u64).min(500) as u16;
                }
                Ok(())
            })
        }

        fn deduct_operating_fund(shop_id: u64, amount: u128) -> Result<(), DispatchError> {
            // H4: 已关闭的 Shop 不可扣减运营资金
            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);

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

            Self::do_create_shop_inner(entity_id, bounded_name, shop_type, Zero::zero(), true)
        }

        fn is_primary_shop(shop_id: u64) -> bool {
            Shops::<T>::get(shop_id)
                .map(|s| s.is_primary)
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
                ensure!(shop.status.can_resume(), Error::<T>::ShopNotPaused);
                // M2-fix: trait 版 resume_shop 也检查运营资金（与 extrinsic 一致）
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
                // M1-R3: 防止重复关闭发射重复事件
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                shop.status = ShopOperatingStatus::Closed;

                // 清理 Closing 计时器（如果正在 Closing）
                ShopClosingAt::<T>::remove(shop_id);

                // H4: 注销 Entity-Shop 关联（与 close_shop extrinsic 保持一致）
                let _ = T::EntityProvider::unregister_shop(shop.entity_id, shop_id);

                // M1-fix: 清理 ShopEntity 反向索引
                ShopEntity::<T>::remove(shop_id);

                // M3: 清理积分数据
                ShopPointsConfigs::<T>::remove(shop_id);
                let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, u32::MAX, None);
                ShopPointsTotalSupply::<T>::remove(shop_id);
                ShopPointsTtl::<T>::remove(shop_id);
                let _ = ShopPointsExpiresAt::<T>::clear_prefix(shop_id, u32::MAX, None);
                ShopPointsMaxSupply::<T>::remove(shop_id);

                // M4: 清理 EntityPrimaryShop 索引（主 Shop 被强制关闭时）
                if shop.is_primary {
                    EntityPrimaryShop::<T>::remove(shop.entity_id);
                }

                // 退还 shop_account 余额给 entity_owner（best-effort）
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
                Ok(())
            })
        }
    }
}
