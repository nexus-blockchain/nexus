//! # Loyalty 模块 (pallet-entity-loyalty)
//!
//! ## 概述
//!
//! 统一管理 Entity 会员忠诚度系统：
//! - **Shop 积分系统**（从 shop 模块搬入）：积分发放、销毁、转移、兑换、过期
//! - **NEX 购物余额**（从 commission 模块搬入）：佣金复购/奖励产生的消费额度
//! - **Token 购物余额**（从 commission/core 搬入）：Token 返佣复购产生的 Token 消费额度
//!
//! ## 设计原则
//!
//! - 积分系统 100% 搬入，loyalty 拥有全部 storage + 逻辑
//! - Token 操作通过 Config::TokenProvider 委托，不搬 token 内部 storage
//! - NEX 购物余额的 storage 搬入 loyalty，commission 通过 LoyaltyWritePort 交互
//! - Token 购物余额的 storage 从 commission/core 搬入 loyalty，commission 通过 LoyaltyTokenWritePort 交互

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{
        CommissionFundGuard, EntityProvider, EntityTokenProvider, LoyaltyReadPort,
        LoyaltyWritePort, LoyaltyTokenReadPort, LoyaltyTokenWritePort, ShopProvider,
    };
    use pallet_commission_common::{ParticipationGuard, TokenTransferProvider};
    use sp_runtime::{
        traits::{Saturating, Zero},
        DispatchError, SaturatedConversion,
    };

    use crate::WeightInfo;

    /// 单次 clear_prefix 最大清理条目数，防止超出区块权重
    const POINTS_CLEANUP_LIMIT: u32 = 500;

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Token 余额类型别名
    pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;

    /// Shop 积分配置
    #[derive(
        Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo,
        MaxEncodedLen, RuntimeDebug,
    )]
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

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 货币类型
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Shop 查询接口（积分权限校验）
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// Entity 查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Token 操作委托（reward_on_purchase / redeem_for_discount）
        type TokenProvider: EntityTokenProvider<Self::AccountId, BalanceOf<Self>>;

        /// 佣金资金保护（redeem_points 检查已承诺资金）
        type CommissionFundGuard: CommissionFundGuard;

        /// KYC 参与检查（购物余额消费时）
        type ParticipationGuard: pallet_commission_common::ParticipationGuard<Self::AccountId>;

        /// Entity Token 余额类型（与 commission-core 的 TokenBalance 一致）
        type TokenBalance: codec::FullCodec
            + codec::MaxEncodedLen
            + TypeInfo
            + Copy
            + Default
            + core::fmt::Debug
            + sp_runtime::traits::AtLeast32BitUnsigned
            + From<u32>
            + Into<u128>;

        /// Token 转账接口（entity_id 级），用于 Token 购物余额消费时转账
        type TokenTransferProvider: TokenTransferProvider<Self::AccountId, TokenBalanceOf<Self>>;

        /// 积分名称最大长度
        #[pallet::constant]
        type MaxPointsNameLength: Get<u32>;

        /// 积分符号最大长度
        #[pallet::constant]
        type MaxPointsSymbolLength: Get<u32>;

        /// Weight 信息
        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // ========================================================================
    // Storage — 积分系统（从 shop 搬入）
    // ========================================================================

    /// Shop 积分配置 shop_id -> PointsConfig
    #[pallet::storage]
    #[pallet::getter(fn shop_points_config)]
    pub type ShopPointsConfigs<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, PointsConfigOf<T>>;

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
    pub type ShopPointsTotalSupply<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

    /// Shop 积分有效期（区块数，0=永不过期）shop_id -> ttl_blocks
    #[pallet::storage]
    #[pallet::getter(fn shop_points_ttl)]
    pub type ShopPointsTtl<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BlockNumberFor<T>, ValueQuery>;

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
    pub type ShopPointsMaxSupply<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, BalanceOf<T>, ValueQuery>;

    // ========================================================================
    // Storage — NEX 购物余额（从 commission 搬入）
    // ========================================================================

    /// Entity 购物余额总额 entity_id -> Balance（资金锁定）
    #[pallet::storage]
    #[pallet::getter(fn entity_shopping_total)]
    pub type ShopShoppingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 会员购物余额 (entity_id, account) -> Balance
    #[pallet::storage]
    #[pallet::getter(fn member_shopping_balance)]
    pub type MemberShoppingBalance<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    // ========================================================================
    // Storage — Token 购物余额（从 commission/core 迁入）
    // ========================================================================

    /// Token 购物余额 (entity_id, account) → TokenBalance
    #[pallet::storage]
    #[pallet::getter(fn member_token_shopping_balance)]
    pub type MemberTokenShoppingBalance<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    /// Token 购物余额总额 entity_id → TokenBalance（资金锁定）
    #[pallet::storage]
    #[pallet::getter(fn token_shopping_total)]
    pub type TokenShoppingTotal<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Shop 积分启用
        ShopPointsEnabled { shop_id: u64, name: BoundedVec<u8, T::MaxPointsNameLength> },
        /// Shop 积分禁用
        ShopPointsDisabled { shop_id: u64 },
        /// Shop 积分发放
        PointsIssued { shop_id: u64, to: T::AccountId, amount: BalanceOf<T> },
        /// Shop 积分销毁
        PointsBurned { shop_id: u64, from: T::AccountId, amount: BalanceOf<T> },
        /// Shop 积分转移
        PointsTransferred {
            shop_id: u64,
            from: T::AccountId,
            to: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 积分配置更新
        PointsConfigUpdated { shop_id: u64 },
        /// 积分兑换
        PointsRedeemed {
            shop_id: u64,
            who: T::AccountId,
            points_burned: BalanceOf<T>,
            payout: BalanceOf<T>,
        },
        /// 积分有效期设置
        PointsTtlSet { shop_id: u64, ttl_blocks: BlockNumberFor<T> },
        /// 积分过期清除
        PointsExpired { shop_id: u64, account: T::AccountId, amount: BalanceOf<T> },
        /// 积分总量上限设置
        PointsMaxSupplySet { shop_id: u64, max_supply: BalanceOf<T> },
        /// 购物余额使用
        ShoppingBalanceUsed {
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 购物余额写入
        ShoppingBalanceCredited {
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// Token 购物余额使用
        TokenShoppingBalanceUsed {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
        /// Token 购物余额写入
        TokenShoppingBalanceCredited {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 积分未启用
        PointsNotEnabled,
        /// 积分已启用
        PointsAlreadyEnabled,
        /// 积分不可转让
        PointsNotTransferable,
        /// 积分余额不足
        InsufficientPointsBalance,
        /// 积分名称不能为空
        PointsNameEmpty,
        /// 积分未过期（无法清除）
        PointsNotExpired,
        /// 兑换金额为零（积分数量太小）
        RedeemPayoutZero,
        /// 积分总量超过上限
        PointsMaxSupplyExceeded,
        /// 无效的配置
        InvalidConfig,
        /// Shop 不存在
        ShopNotFound,
        /// 无权限操作
        NotAuthorized,
        /// Entity 未激活
        EntityNotActive,
        /// 实体已被全局锁定
        EntityLocked,
        /// Shop 已关闭
        ShopAlreadyClosed,
        /// Shop 已被封禁
        ShopBanned,
        /// 不能转给自己
        SameAccount,
        /// 运营资金不足
        InsufficientOperatingFund,
        /// 购物余额不足
        InsufficientShoppingBalance,
        /// 金额为零
        ZeroAmount,
        /// 不满足参与要求（KYC）
        ParticipationRequirementNotMet,
    }

    // ========================================================================
    // Extrinsics — 积分系统
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 启用 Shop 积分
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::enable_points())]
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

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 检查权限
            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            // 检查 Shop 状态
            Self::ensure_shop_not_terminal_or_banned(shop_id)?;

            // 检查是否已启用
            ensure!(
                !ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsAlreadyEnabled
            );

            // 积分名称和符号不能为空
            ensure!(!name.is_empty(), Error::<T>::PointsNameEmpty);
            ensure!(!symbol.is_empty(), Error::<T>::InvalidConfig);

            // 验证配置
            ensure!(
                reward_rate <= 10000 && exchange_rate <= 10000,
                Error::<T>::InvalidConfig
            );

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

        /// 禁用 Shop 积分
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::disable_points())]
        pub fn disable_points(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Self::ensure_shop_not_terminal_or_banned(shop_id)?;
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

            ShopPointsConfigs::<T>::remove(shop_id);
            let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, POINTS_CLEANUP_LIMIT, None);
            ShopPointsTotalSupply::<T>::remove(shop_id);
            ShopPointsTtl::<T>::remove(shop_id);
            let _ = ShopPointsExpiresAt::<T>::clear_prefix(shop_id, POINTS_CLEANUP_LIMIT, None);
            ShopPointsMaxSupply::<T>::remove(shop_id);

            Self::deposit_event(Event::ShopPointsDisabled { shop_id });
            Ok(())
        }

        /// 更新积分配置
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::update_points_config())]
        pub fn update_points_config(
            origin: OriginFor<T>,
            shop_id: u64,
            reward_rate: Option<u16>,
            exchange_rate: Option<u16>,
            transferable: Option<bool>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 至少需要修改一个字段
            ensure!(
                reward_rate.is_some() || exchange_rate.is_some() || transferable.is_some(),
                Error::<T>::InvalidConfig
            );

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Self::ensure_shop_not_terminal_or_banned(shop_id)?;

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

                Self::deposit_event(Event::PointsConfigUpdated { shop_id });
                Ok(())
            })
        }

        /// 转移 Shop 积分（用户之间）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::transfer_points())]
        pub fn transfer_points(
            origin: OriginFor<T>,
            shop_id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);
            ensure!(who != to, Error::<T>::SameAccount);

            ensure!(
                T::ShopProvider::shop_exists(shop_id),
                Error::<T>::ShopNotFound
            );
            Self::ensure_shop_not_closed(shop_id)?;
            Self::ensure_shop_not_banned(shop_id)?;

            let config = ShopPointsConfigs::<T>::get(shop_id)
                .ok_or(Error::<T>::PointsNotEnabled)?;
            ensure!(config.transferable, Error::<T>::PointsNotTransferable);

            // 懒过期检查
            Self::check_points_expiry(shop_id, &who);

            let from_balance = ShopPointsBalances::<T>::get(shop_id, &who);
            ensure!(from_balance >= amount, Error::<T>::InsufficientPointsBalance);

            ShopPointsBalances::<T>::mutate(shop_id, &who, |b| *b = b.saturating_sub(amount));
            ShopPointsBalances::<T>::mutate(shop_id, &to, |b| *b = b.saturating_add(amount));

            // 延长接收方积分有效期
            Self::maybe_extend_points_expiry(shop_id, &to);

            Self::deposit_event(Event::PointsTransferred {
                shop_id,
                from: who,
                to,
                amount,
            });
            Ok(())
        }

        /// Manager 直接发放积分
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::manager_issue_points())]
        pub fn manager_issue_points(
            origin: OriginFor<T>,
            shop_id: u64,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Self::ensure_shop_not_terminal_or_banned(shop_id)?;
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

            Self::check_points_max_supply(shop_id, amount)?;

            ShopPointsBalances::<T>::mutate(shop_id, &to, |b| *b = b.saturating_add(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_add(amount));

            Self::maybe_extend_points_expiry(shop_id, &to);

            Self::deposit_event(Event::PointsIssued { shop_id, to, amount });
            Ok(())
        }

        /// Manager 直接销毁积分
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::manager_burn_points())]
        pub fn manager_burn_points(
            origin: OriginFor<T>,
            shop_id: u64,
            from: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            // 已关闭不可销毁（Closing 宽限期内允许），封禁不可销毁
            Self::ensure_shop_not_closed(shop_id)?;
            Self::ensure_shop_not_banned(shop_id)?;
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

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
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::redeem_points())]
        pub fn redeem_points(
            origin: OriginFor<T>,
            shop_id: u64,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            ensure!(
                T::ShopProvider::shop_exists(shop_id),
                Error::<T>::ShopNotFound
            );
            Self::ensure_shop_not_closed(shop_id)?;
            Self::ensure_shop_not_banned(shop_id)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

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

            // 佣金保护 — 不得侵占已承诺的佣金资金
            let shop_account = T::ShopProvider::shop_account(shop_id);
            let shop_balance = T::Currency::free_balance(&shop_account);
            let protected: BalanceOf<T> =
                T::CommissionFundGuard::protected_funds(entity_id).saturated_into();
            let available = shop_balance.saturating_sub(protected);
            ensure!(available >= payout, Error::<T>::InsufficientOperatingFund);

            // 从 Shop 运营账户转给用户
            T::Currency::transfer(
                &shop_account,
                &who,
                payout,
                ExistenceRequirement::AllowDeath,
            )?;

            // 销毁积分
            ShopPointsBalances::<T>::mutate(shop_id, &who, |b| *b = b.saturating_sub(amount));
            ShopPointsTotalSupply::<T>::mutate(shop_id, |s| *s = s.saturating_sub(amount));

            Self::deposit_event(Event::PointsRedeemed {
                shop_id,
                who,
                points_burned: amount,
                payout,
            });
            Ok(())
        }

        /// 设置 Shop 积分有效期（TTL）
        ///
        /// ttl_blocks = 0 表示永不过期（移除 TTL 限制）。
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::set_points_ttl())]
        pub fn set_points_ttl(
            origin: OriginFor<T>,
            shop_id: u64,
            ttl_blocks: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Self::ensure_shop_not_terminal_or_banned(shop_id)?;
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

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
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::expire_points())]
        pub fn expire_points(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            ensure!(
                T::ShopProvider::shop_exists(shop_id),
                Error::<T>::ShopNotFound
            );

            let expiry = ShopPointsExpiresAt::<T>::get(shop_id, &account)
                .ok_or(Error::<T>::PointsNotExpired)?;

            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now > expiry, Error::<T>::PointsNotExpired);

            let expired_amount = ShopPointsBalances::<T>::take(shop_id, &account);
            if !expired_amount.is_zero() {
                ShopPointsTotalSupply::<T>::mutate(shop_id, |s| {
                    *s = s.saturating_sub(expired_amount)
                });
                Self::deposit_event(Event::PointsExpired {
                    shop_id,
                    account: account.clone(),
                    amount: expired_amount,
                });
            }
            ShopPointsExpiresAt::<T>::remove(shop_id, &account);

            Ok(())
        }

        /// 设置积分总量上限
        ///
        /// max_supply = 0 表示无上限。已有供应量超过新上限时拒绝。
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::set_points_max_supply())]
        pub fn set_points_max_supply(
            origin: OriginFor<T>,
            shop_id: u64,
            max_supply: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(T::ShopProvider::is_shop_manager(shop_id, &who), Error::<T>::NotAuthorized);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Self::ensure_shop_not_terminal_or_banned(shop_id)?;
            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

            // 如果设置上限，当前供应量不得超过
            if !max_supply.is_zero() {
                let current_supply = ShopPointsTotalSupply::<T>::get(shop_id);
                ensure!(
                    current_supply <= max_supply,
                    Error::<T>::PointsMaxSupplyExceeded
                );
            }

            if max_supply.is_zero() {
                ShopPointsMaxSupply::<T>::remove(shop_id);
            } else {
                ShopPointsMaxSupply::<T>::insert(shop_id, max_supply);
            }

            Self::deposit_event(Event::PointsMaxSupplySet { shop_id, max_supply });
            Ok(())
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    impl<T: Config> Pallet<T> {
        // ---- Shop 状态检查 helpers ----

        fn ensure_shop_not_terminal_or_banned(shop_id: u64) -> DispatchResult {
            if let Some(status) = T::ShopProvider::shop_own_status(shop_id) {
                ensure!(!status.is_terminal_or_banned(), Error::<T>::ShopAlreadyClosed);
            }
            Ok(())
        }

        fn ensure_shop_not_closed(shop_id: u64) -> DispatchResult {
            use pallet_entity_common::ShopOperatingStatus;
            if let Some(status) = T::ShopProvider::shop_own_status(shop_id) {
                ensure!(status != ShopOperatingStatus::Closed, Error::<T>::ShopAlreadyClosed);
            }
            Ok(())
        }

        fn ensure_shop_not_banned(shop_id: u64) -> DispatchResult {
            if let Some(status) = T::ShopProvider::shop_own_status(shop_id) {
                ensure!(!status.is_banned(), Error::<T>::ShopBanned);
            }
            Ok(())
        }

        // ---- 积分 helpers ----

        /// 检查并清除过期积分（懒过期）
        fn check_points_expiry(shop_id: u64, account: &T::AccountId) {
            if let Some(expiry) = ShopPointsExpiresAt::<T>::get(shop_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                if now > expiry {
                    let expired = ShopPointsBalances::<T>::take(shop_id, account);
                    if !expired.is_zero() {
                        ShopPointsTotalSupply::<T>::mutate(shop_id, |s| {
                            *s = s.saturating_sub(expired)
                        });
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
                ensure!(
                    current.saturating_add(amount) <= max_supply,
                    Error::<T>::PointsMaxSupplyExceeded
                );
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

            ensure!(
                T::ShopProvider::shop_exists(shop_id),
                Error::<T>::ShopNotFound
            );
            // 已关闭/关闭中的 Shop 不可发放积分
            Self::ensure_shop_not_closed(shop_id)?;
            // 封禁状态不可发放积分
            Self::ensure_shop_not_banned(shop_id)?;

            // Closing 状态也不发积分（与 shop 原逻辑一致：is_closed_or_closing）
            use pallet_entity_common::ShopOperatingStatus;
            if let Some(status) = T::ShopProvider::shop_own_status(shop_id) {
                ensure!(status != ShopOperatingStatus::Closing, Error::<T>::ShopAlreadyClosed);
            }

            ensure!(
                ShopPointsConfigs::<T>::contains_key(shop_id),
                Error::<T>::PointsNotEnabled
            );

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

        /// 销毁积分（供外部模块调用）
        pub fn burn_points(
            shop_id: u64,
            from: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::InsufficientPointsBalance);

            ensure!(
                T::ShopProvider::shop_exists(shop_id),
                Error::<T>::ShopNotFound
            );
            Self::ensure_shop_not_closed(shop_id)?;
            Self::ensure_shop_not_banned(shop_id)?;
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

        /// 清理某个 Shop 的全部积分数据（Shop 关闭时由 shop 模块调用）
        pub fn cleanup_shop_points(shop_id: u64) {
            ShopPointsConfigs::<T>::remove(shop_id);
            let _ = ShopPointsBalances::<T>::clear_prefix(shop_id, POINTS_CLEANUP_LIMIT, None);
            ShopPointsTotalSupply::<T>::remove(shop_id);
            ShopPointsTtl::<T>::remove(shop_id);
            let _ = ShopPointsExpiresAt::<T>::clear_prefix(shop_id, POINTS_CLEANUP_LIMIT, None);
            ShopPointsMaxSupply::<T>::remove(shop_id);
        }

        // ---- 积分查询 Runtime API 辅助方法 ----

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
        pub fn get_points_expiry(
            shop_id: u64,
            account: &T::AccountId,
        ) -> Option<BlockNumberFor<T>> {
            ShopPointsExpiresAt::<T>::get(shop_id, account)
        }

        /// 查询 Shop 积分总量上限
        pub fn get_points_max_supply(shop_id: u64) -> BalanceOf<T> {
            ShopPointsMaxSupply::<T>::get(shop_id)
        }

        // ---- NEX 购物余额 helpers ----

        /// 扣减购物余额（纯记账，不转 NEX）— 供 order 下单时使用
        pub fn do_use_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            MemberShoppingBalance::<T>::try_mutate(
                entity_id,
                account,
                |balance| -> DispatchResult {
                    ensure!(*balance >= amount, Error::<T>::InsufficientShoppingBalance);
                    *balance = balance.saturating_sub(amount);

                    ShopShoppingTotal::<T>::mutate(entity_id, |total| {
                        *total = total.saturating_sub(amount);
                    });

                    Self::deposit_event(Event::ShoppingBalanceUsed {
                        entity_id,
                        account: account.clone(),
                        amount,
                    });

                    Ok(())
                },
            )
        }

        /// 消费购物余额（记账 + NEX 从 Entity 账户转入会员钱包）
        pub fn do_consume_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

            // KYC 检查
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, account),
                Error::<T>::ParticipationRequirementNotMet
            );

            MemberShoppingBalance::<T>::try_mutate(
                entity_id,
                account,
                |balance| -> DispatchResult {
                    ensure!(*balance >= amount, Error::<T>::InsufficientShoppingBalance);
                    *balance = balance.saturating_sub(amount);

                    ShopShoppingTotal::<T>::mutate(entity_id, |total| {
                        *total = total.saturating_sub(amount);
                    });

                    // 将 NEX 从 Entity 账户转入会员钱包
                    let entity_account = T::EntityProvider::entity_account(entity_id);
                    T::Currency::transfer(
                        &entity_account,
                        account,
                        amount,
                        ExistenceRequirement::KeepAlive,
                    )?;

                    Self::deposit_event(Event::ShoppingBalanceUsed {
                        entity_id,
                        account: account.clone(),
                        amount,
                    });

                    Ok(())
                },
            )
        }

        /// 写入购物余额（commission 结算后调用）
        pub fn do_credit_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }

            MemberShoppingBalance::<T>::mutate(entity_id, account, |balance| {
                *balance = balance.saturating_add(amount);
            });
            ShopShoppingTotal::<T>::mutate(entity_id, |total| {
                *total = total.saturating_add(amount);
            });

            Self::deposit_event(Event::ShoppingBalanceCredited {
                entity_id,
                account: account.clone(),
                amount,
            });

            Ok(())
        }

        /// 查询 Entity 级购物余额总额（供 commission solvency check 使用）
        pub fn shopping_total(entity_id: u64) -> BalanceOf<T> {
            ShopShoppingTotal::<T>::get(entity_id)
        }

        // ---- Token 购物余额 helpers ----

        /// 写入 Token 购物余额（commission 结算后调用）
        pub fn do_credit_token_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: TokenBalanceOf<T>,
        ) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }

            MemberTokenShoppingBalance::<T>::mutate(entity_id, account, |balance| {
                *balance = balance.saturating_add(amount);
            });
            TokenShoppingTotal::<T>::mutate(entity_id, |total| {
                *total = total.saturating_add(amount);
            });

            Self::deposit_event(Event::TokenShoppingBalanceCredited {
                entity_id,
                account: account.clone(),
                amount,
            });

            Ok(())
        }

        /// 消费 Token 购物余额（记账 + Token 从 Entity 账户转入会员钱包）
        pub fn do_consume_token_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: TokenBalanceOf<T>,
        ) -> DispatchResult {
            use pallet_entity_common::EntityProvider as _;

            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

            // KYC 参与检查
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, account),
                Error::<T>::ParticipationRequirementNotMet
            );

            MemberTokenShoppingBalance::<T>::try_mutate(
                entity_id,
                account,
                |balance| -> DispatchResult {
                    ensure!(*balance >= amount, Error::<T>::InsufficientShoppingBalance);
                    *balance = balance.saturating_sub(amount);

                    TokenShoppingTotal::<T>::mutate(entity_id, |total| {
                        *total = total.saturating_sub(amount);
                    });

                    // 将 Token 从 Entity 账户转入会员钱包
                    let entity_account = T::EntityProvider::entity_account(entity_id);
                    T::TokenTransferProvider::token_transfer(
                        entity_id, &entity_account, account, amount,
                    )?;

                    Self::deposit_event(Event::TokenShoppingBalanceUsed {
                        entity_id,
                        account: account.clone(),
                        amount,
                    });

                    Ok(())
                },
            )
        }

        /// 查询 Entity 级 Token 购物余额总额
        pub fn token_shopping_total_of(entity_id: u64) -> TokenBalanceOf<T> {
            TokenShoppingTotal::<T>::get(entity_id)
        }
    }

    // ========================================================================
    // LoyaltyReadPort + LoyaltyWritePort 实现
    // ========================================================================

    impl<T: Config> LoyaltyReadPort<T::AccountId, BalanceOf<T>> for Pallet<T> {
        fn is_token_enabled(entity_id: u64) -> bool {
            T::TokenProvider::is_token_enabled(entity_id)
        }

        fn token_discount_balance(entity_id: u64, who: &T::AccountId) -> BalanceOf<T> {
            T::TokenProvider::token_balance(entity_id, who)
        }

        fn shopping_balance(entity_id: u64, account: &T::AccountId) -> BalanceOf<T> {
            MemberShoppingBalance::<T>::get(entity_id, account)
        }

        fn shopping_total(entity_id: u64) -> BalanceOf<T> {
            ShopShoppingTotal::<T>::get(entity_id)
        }
    }

    impl<T: Config> LoyaltyWritePort<T::AccountId, BalanceOf<T>> for Pallet<T> {
        fn redeem_for_discount(
            entity_id: u64,
            who: &T::AccountId,
            tokens: BalanceOf<T>,
        ) -> Result<BalanceOf<T>, DispatchError> {
            T::TokenProvider::redeem_for_discount(entity_id, who, tokens)
        }

        fn consume_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> Result<(), DispatchError> {
            Self::do_consume_shopping_balance(entity_id, account, amount)
        }

        fn reward_on_purchase(
            entity_id: u64,
            who: &T::AccountId,
            purchase_amount: BalanceOf<T>,
        ) -> Result<BalanceOf<T>, DispatchError> {
            T::TokenProvider::reward_on_purchase(entity_id, who, purchase_amount)
        }

        fn credit_shopping_balance(
            entity_id: u64,
            who: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> Result<(), DispatchError> {
            Self::do_credit_shopping_balance(entity_id, who, amount)
        }
    }

    // ========================================================================
    // LoyaltyTokenReadPort + LoyaltyTokenWritePort 实现（Token 购物余额）
    // ========================================================================

    impl<T: Config> LoyaltyTokenReadPort<T::AccountId, TokenBalanceOf<T>> for Pallet<T> {
        fn token_shopping_balance(entity_id: u64, who: &T::AccountId) -> TokenBalanceOf<T> {
            MemberTokenShoppingBalance::<T>::get(entity_id, who)
        }

        fn token_shopping_total(entity_id: u64) -> TokenBalanceOf<T> {
            TokenShoppingTotal::<T>::get(entity_id)
        }
    }

    impl<T: Config> LoyaltyTokenWritePort<T::AccountId, TokenBalanceOf<T>> for Pallet<T> {
        fn credit_token_shopping_balance(
            entity_id: u64,
            who: &T::AccountId,
            amount: TokenBalanceOf<T>,
        ) -> Result<(), DispatchError> {
            Self::do_credit_token_shopping_balance(entity_id, who, amount)
        }

        fn consume_token_shopping_balance(
            entity_id: u64,
            account: &T::AccountId,
            amount: TokenBalanceOf<T>,
        ) -> Result<(), DispatchError> {
            Self::do_consume_token_shopping_balance(entity_id, account, amount)
        }
    }

    // ========================================================================
    // PointsCleanup 实现（供 shop 关闭时调用）
    // ========================================================================

    impl<T: Config> pallet_entity_common::PointsCleanup for Pallet<T> {
        fn cleanup_shop_points(shop_id: u64) {
            Self::cleanup_shop_points(shop_id);
        }
    }

    // ========================================================================
    // ShopGovernancePort 实现（积分治理操作）
    // ========================================================================

    impl<T: Config> pallet_entity_common::ShopGovernancePort for Pallet<T> {
        fn governance_set_points_config(
            entity_id: u64,
            reward_rate: u16,
            exchange_rate: u16,
            transferable: bool,
        ) -> Result<(), sp_runtime::DispatchError> {
            // 验证参数
            frame_support::ensure!(
                reward_rate <= 10000 && exchange_rate <= 10000,
                sp_runtime::DispatchError::Other("InvalidConfig")
            );

            // 更新 entity 下所有 shop 的积分配置
            let shops = T::EntityProvider::entity_shops(entity_id);
            let mut updated = 0u32;
            for shop_id in shops {
                if ShopPointsConfigs::<T>::contains_key(shop_id) {
                    ShopPointsConfigs::<T>::mutate(shop_id, |maybe_config| {
                        if let Some(config) = maybe_config.as_mut() {
                            config.reward_rate = reward_rate;
                            config.exchange_rate = exchange_rate;
                            config.transferable = transferable;
                            updated += 1;
                        }
                    });
                    Self::deposit_event(Event::PointsConfigUpdated { shop_id });
                }
            }

            frame_support::ensure!(updated > 0, sp_runtime::DispatchError::Other("NoPointsConfigFound"));
            Ok(())
        }

        fn governance_toggle_points(
            entity_id: u64,
            enabled: bool,
        ) -> Result<(), sp_runtime::DispatchError> {
            if !enabled {
                // 禁用积分：清理 entity 下所有 shop 的积分
                let shops = T::EntityProvider::entity_shops(entity_id);
                for shop_id in shops {
                    if ShopPointsConfigs::<T>::contains_key(shop_id) {
                        Self::cleanup_shop_points(shop_id);
                        Self::deposit_event(Event::ShopPointsDisabled { shop_id });
                    }
                }
            }
            // 启用积分需要名称/符号等初始化参数，治理端无法提供，返回 Ok
            // 实际启用需通过 enable_points extrinsic
            Ok(())
        }

        fn governance_set_shop_policies(
            _entity_id: u64,
            _policies_cid: &[u8],
        ) -> Result<(), sp_runtime::DispatchError> {
            // Shop policies_cid 存储在 shop pallet 的 Shop 结构体中
            // 治理提案通过后需要 off-chain 执行（CID 指向链下策略文档）
            Ok(())
        }
    }
}
