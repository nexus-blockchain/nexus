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
        /// 是否启用
        pub enabled: bool,
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
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

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
        /// Shop 未激活
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
        /// 余额不足
        InsufficientBalance,
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
            logo_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
            description_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                
                // H3: 已关闭的 Shop 不可修改
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);

                if let Some(n) = name {
                    // H2: 名称不能为空
                    ensure!(!n.is_empty(), Error::<T>::ShopNameEmpty);
                    shop.name = n;
                }
                if let Some(cid) = logo_cid {
                    shop.logo_cid = Some(cid);
                }
                if let Some(cid) = description_cid {
                    shop.description_cid = Some(cid);
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

                // H3: 已关闭的 Shop 不可操作管理员
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                
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

                // H3: 已关闭的 Shop 不可操作管理员
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                
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
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                // H2: 关闭的店铺不允许充值
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                
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
                
                // 检查状态
                ensure!(shop.status == ShopOperatingStatus::Active, Error::<T>::ShopAlreadyPaused);
                
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
            address_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
            business_hours_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> DispatchResult {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                
                // 检查权限
                ensure!(Self::can_manage_shop(shop, &who), Error::<T>::NotAuthorized);
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                
                // 验证位置（经度 -180~180，纬度 -90~90，精度 10^6）
                if let Some((lng, lat)) = location {
                    ensure!(lng >= -180_000_000 && lng <= 180_000_000, Error::<T>::InvalidLocation);
                    ensure!(lat >= -90_000_000 && lat <= 90_000_000, Error::<T>::InvalidLocation);
                }
                
                shop.location = location;
                if let Some(cid) = address_cid {
                    shop.address_cid = Some(cid);
                }
                if let Some(cid) = business_hours_cid {
                    shop.business_hours_cid = Some(cid);
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
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
            
            // 检查是否已启用
            ensure!(!ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsAlreadyEnabled);
            
            // M1: 积分名称和符号不能为空
            ensure!(!name.is_empty(), Error::<T>::ShopNameEmpty);
            ensure!(!symbol.is_empty(), Error::<T>::InvalidConfig);

            // 验证配置
            ensure!(reward_rate <= 10000 && exchange_rate <= 10000, Error::<T>::InvalidConfig);
            
            let config = PointsConfig {
                name: name.clone(),
                symbol,
                reward_rate,
                exchange_rate,
                transferable,
                enabled: true,
            };
            
            ShopPointsConfigs::<T>::insert(shop_id, config);
            
            Self::deposit_event(Event::ShopPointsEnabled { shop_id, name });
            Ok(())
        }

        /// 关闭 Shop
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
                
                // 主 Shop 不可关闭
                ensure!(!shop.is_primary, Error::<T>::CannotClosePrimaryShop);
                
                // 检查状态
                ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
                
                shop.status = ShopOperatingStatus::Closed;
                
                // M4: 注销 Entity-Shop 关联
                T::EntityProvider::unregister_shop(shop.entity_id, shop_id)?;

                // M3: 清理积分数据
                ShopPointsConfigs::<T>::remove(shop_id);
                let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, u32::MAX, None);
                ShopPointsTotalSupply::<T>::remove(shop_id);

                // 退还 shop_account 余额给 entity_owner
                let shop_account = Self::shop_account_id(shop_id);
                let remaining = T::Currency::free_balance(&shop_account);
                if !remaining.is_zero() {
                    if T::Currency::transfer(
                        &shop_account,
                        &who,
                        remaining,
                        ExistenceRequirement::AllowDeath,
                    ).is_ok() {
                        Self::deposit_event(Event::ShopClosedFundRefunded {
                            shop_id,
                            to: who,
                            amount: remaining,
                        });
                    }
                }
                
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
            ensure!(ShopPointsConfigs::<T>::contains_key(shop_id), Error::<T>::PointsNotEnabled);

            ShopPointsConfigs::<T>::remove(shop_id);
            // H2: 清理积分余额和总供应量，避免残留数据
            let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, u32::MAX, None);
            ShopPointsTotalSupply::<T>::remove(shop_id);

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

            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(Self::can_manage_shop(&shop, &who), Error::<T>::NotAuthorized);

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

            let config = ShopPointsConfigs::<T>::get(shop_id)
                .ok_or(Error::<T>::PointsNotEnabled)?;
            ensure!(config.transferable, Error::<T>::PointsNotTransferable);

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

            let shop_account = Self::shop_account_id(shop_id);
            let balance = T::Currency::free_balance(&shop_account);

            // 佣金保护：不得侵占已承诺的佣金资金
            let protected: BalanceOf<T> = T::CommissionFundGuard::protected_funds(shop_id).saturated_into();
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
            // Entity admin 继承 Shop 管理权限
            if T::EntityProvider::is_entity_admin(shop.entity_id, account) {
                return true;
            }
            // Shop manager 有权限
            shop.managers.contains(account)
        }

        /// 发放积分（供外部模块调用，如订单完成后返积分）
        pub fn issue_points(
            shop_id: u64,
            to: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);
            // M5: 已关闭的 Shop 不可发放积分
            let shop = Shops::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(shop.status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
            let config = ShopPointsConfigs::<T>::get(shop_id)
                .ok_or(Error::<T>::PointsNotEnabled)?;
            ensure!(config.enabled, Error::<T>::PointsNotEnabled);

            ShopPointsBalances::<T>::mutate(shop_id, to, |b| *b = b.saturating_add(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_add(amount));

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
            NextShopId::<T>::put(shop_id.saturating_add(1));

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

        fn update_shop_stats(shop_id: u64, sales_amount: u128, order_count: u32) -> Result<(), DispatchError> {
            let entity_id = ShopEntity::<T>::get(shop_id).ok_or(Error::<T>::ShopNotFound)?;

            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                shop.total_sales = shop.total_sales.saturating_add(
                    sales_amount.saturated_into()
                );
                shop.total_orders = shop.total_orders.saturating_add(order_count);
                Ok(())
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
                shop.rating = (shop.rating_total / shop.rating_count as u64).min(500) as u16;
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
            let protected: BalanceOf<T> = T::CommissionFundGuard::protected_funds(shop_id).saturated_into();
            let available = balance.saturating_sub(protected);
            ensure!(available >= amount_balance, Error::<T>::InsufficientOperatingFund);
            
            // 扣减运营资金（销毁，费用已由 Entity 层 deduct_operating_fee 处理）
            let _imbalance = T::Currency::withdraw(
                &shop_account,
                amount_balance,
                frame_support::traits::WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
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
                shop.status = ShopOperatingStatus::Paused;
                Self::deposit_event(Event::ShopPaused { shop_id });
                Ok(())
            })
        }

        fn resume_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                shop.status = ShopOperatingStatus::Active;
                Self::deposit_event(Event::ShopResumed { shop_id });
                Ok(())
            })
        }

        fn force_close_shop(shop_id: u64) -> Result<(), DispatchError> {
            Shops::<T>::try_mutate(shop_id, |maybe_shop| -> Result<(), DispatchError> {
                let shop = maybe_shop.as_mut().ok_or(Error::<T>::ShopNotFound)?;
                shop.status = ShopOperatingStatus::Closed;

                // M3: 清理积分数据
                ShopPointsConfigs::<T>::remove(shop_id);
                let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, u32::MAX, None);
                ShopPointsTotalSupply::<T>::remove(shop_id);

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
