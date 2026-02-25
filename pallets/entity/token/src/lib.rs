//! # 实体通证模块 (pallet-entity-token)
//!
//! ## 概述
//!
//! 本模块作为 pallet-assets 的桥接层，为每个实体提供通证功能：
//! - 实体通证创建和配置
//! - 多种通证类型（积分、治理、股权、会员等）
//! - 购物/参与奖励
//! - 积分/通证兑换
//! - 通证转让
//! - 分红功能
//!
//! ## 架构
//!
//! ```text
//! pallet-entity-token (桥接层)
//!         │
//!         │ fungibles::* traits
//!         ▼
//! pallet-assets (底层资产模块)
//! ```
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-01-31): 初始版本
//! - v0.2.0 (2026-02-03): Phase 2 扩展，支持多种通证类型和分红

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
        traits::fungibles::{Create, Inspect, Mutate, metadata::Mutate as MetadataMutate},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{DividendConfig, EntityProvider, ShopProvider, TokenType, TransferRestrictionMode};
    use sp_runtime::traits::{AtLeast32BitUnsigned, Saturating, Zero};

    /// T-M1 审计修复: 独立锁仓条目，避免合并时意外延长解锁时间
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct LockEntry<Balance, BlockNumber> {
        pub amount: Balance,
        pub unlock_at: BlockNumber,
    }

    /// 实体通证配置（原 ShopTokenConfig，Phase 2 扩展）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct EntityTokenConfig<Balance, BlockNumber> {
        /// 是否已启用通证
        pub enabled: bool,
        /// 购物/参与返积分比例（基点，500 = 5%）
        pub reward_rate: u16,
        /// 积分兑换比例（基点，1000 = 10%，即 10 积分 = 1 元折扣）
        pub exchange_rate: u16,
        /// 最低兑换门槛
        pub min_redeem: Balance,
        /// 单笔最大兑换（0 = 无限制）
        pub max_redeem_per_order: Balance,
        /// 是否允许用户间转让
        pub transferable: bool,
        /// 创建时间
        pub created_at: BlockNumber,
        // ========== Phase 2 新增字段 ==========
        /// 通证类型（默认 Points）
        pub token_type: TokenType,
        /// 最大供应量（0 = 无限制）
        pub max_supply: Balance,
        /// 分红配置
        pub dividend_config: DividendConfig<Balance, BlockNumber>,
        // ========== Phase 8 新增字段：转账限制 ==========
        /// 转账限制模式
        pub transfer_restriction: TransferRestrictionMode,
        /// 接收方最低 KYC 级别 (0-4)
        pub min_receiver_kyc: u8,
    }

    /// 向后兼容：ShopTokenConfig 类型别名
    pub type ShopTokenConfig<Balance, BlockNumber> = EntityTokenConfig<Balance, BlockNumber>;

    /// 配置类型别名
    pub type ShopTokenConfigOf<T> = ShopTokenConfig<
        <T as Config>::AssetBalance,
        BlockNumberFor<T>,
    >;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 资产 ID 类型
        type AssetId: Member + Parameter + Copy + MaxEncodedLen + From<u64> + Into<u64>;

        /// 资产余额类型
        type AssetBalance: Member
            + Parameter
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaxEncodedLen
            + From<u128>
            + Into<u128>;

        /// 资产创建接口
        type Assets: Create<Self::AccountId, AssetId = Self::AssetId, Balance = Self::AssetBalance>
            + Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Self::AssetBalance>
            + Mutate<Self::AccountId, AssetId = Self::AssetId, Balance = Self::AssetBalance>
            + MetadataMutate<Self::AccountId, AssetId = Self::AssetId>;

        /// 实体查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Shop 查询接口（Entity-Shop 分离架构）
        type ShopProvider: ShopProvider<Self::AccountId>;

        /// 店铺代币 ID 偏移量（避免与其他资产冲突）
        #[pallet::constant]
        type ShopTokenOffset: Get<u64>;

        /// 代币名称最大长度
        #[pallet::constant]
        type MaxTokenNameLength: Get<u32>;

        /// 代币符号最大长度
        #[pallet::constant]
        type MaxTokenSymbolLength: Get<u32>;

        /// 白名单/黑名单最大地址数
        #[pallet::constant]
        type MaxTransferListSize: Get<u32>;

        /// 分红单次最大接收人数
        #[pallet::constant]
        type MaxDividendRecipients: Get<u32>;

        /// KYC 查询接口（可选）
        type KycProvider: KycLevelProvider<Self::AccountId>;

        /// 成员查询接口（可选）
        type MemberProvider: EntityMemberProvider<Self::AccountId>;
    }

    /// KYC 级别查询 Trait
    pub trait KycLevelProvider<AccountId> {
        /// 获取用户 KYC 级别 (0-4)
        fn get_kyc_level(account: &AccountId) -> u8;
        /// 检查是否满足 KYC 要求
        fn meets_kyc_requirement(account: &AccountId, min_level: u8) -> bool;
    }

    /// 实体成员查询 Trait
    pub trait EntityMemberProvider<AccountId> {
        /// 检查是否为实体成员
        fn is_member(entity_id: u64, account: &AccountId) -> bool;
    }

    /// 空 KYC 提供者（默认实现）
    pub struct NullKycProvider;
    impl<AccountId> KycLevelProvider<AccountId> for NullKycProvider {
        fn get_kyc_level(_account: &AccountId) -> u8 { 0 }
        fn meets_kyc_requirement(_account: &AccountId, min_level: u8) -> bool { min_level == 0 }
    }

    /// 空成员提供者（默认实现）
    pub struct NullMemberProvider;
    impl<AccountId> EntityMemberProvider<AccountId> for NullMemberProvider {
        fn is_member(_entity_id: u64, _account: &AccountId) -> bool { true }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 实体代币配置存储（Entity 级统一代币）
    #[pallet::storage]
    #[pallet::getter(fn entity_token_configs)]
    pub type EntityTokenConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        ShopTokenConfigOf<T>,
    >;

    /// 向后兼容别名
    pub type ShopTokenConfigs<T> = EntityTokenConfigs<T>;

    /// 实体代币元数据（名称、符号）
    #[pallet::storage]
    #[pallet::getter(fn entity_token_metadata)]
    pub type EntityTokenMetadata<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        (
            BoundedVec<u8, T::MaxTokenNameLength>,   // name
            BoundedVec<u8, T::MaxTokenSymbolLength>, // symbol
            u8,                                      // decimals
        ),
    >;

    /// 向后兼容别名
    pub type ShopTokenMetadata<T> = EntityTokenMetadata<T>;

    /// 统计：已创建的实体代币数量
    #[pallet::storage]
    #[pallet::getter(fn total_entity_tokens)]
    pub type TotalEntityTokens<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 向后兼容别名
    pub type TotalShopTokens<T> = TotalEntityTokens<T>;

    // ========== Phase 4 新增存储项 ==========

    /// T-M1: 锁仓记录 (entity_id, holder) -> Vec<LockEntry>
    /// 每个用户最多 10 条独立锁仓，各自独立到期
    #[pallet::storage]
    #[pallet::getter(fn locked_tokens)]
    pub type LockedTokens<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<LockEntry<T::AssetBalance, BlockNumberFor<T>>, ConstU32<10>>,
        ValueQuery,
    >;

    /// 待领取分红 (entity_id, holder) -> amount
    #[pallet::storage]
    #[pallet::getter(fn pending_dividends)]
    pub type PendingDividends<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        T::AssetBalance,
        ValueQuery,
    >;

    /// 已领取分红总额 (entity_id, holder) -> total_claimed
    #[pallet::storage]
    #[pallet::getter(fn claimed_dividends)]
    pub type ClaimedDividends<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,
        Blake2_128Concat,
        T::AccountId,
        T::AssetBalance,
        ValueQuery,
    >;

    // ========== Phase 8 新增存储项：转账限制 ==========

    /// 转账白名单 entity_id -> Vec<AccountId>
    #[pallet::storage]
    #[pallet::getter(fn transfer_whitelist)]
    pub type TransferWhitelist<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<T::AccountId, T::MaxTransferListSize>,
        ValueQuery,
    >;

    /// 转账黑名单 entity_id -> Vec<AccountId>
    #[pallet::storage]
    #[pallet::getter(fn transfer_blacklist)]
    pub type TransferBlacklist<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<T::AccountId, T::MaxTransferListSize>,
        ValueQuery,
    >;

    /// 预留代币 (entity_id, holder) -> reserved_amount
    #[pallet::storage]
    #[pallet::getter(fn reserved_tokens)]
    pub type ReservedTokens<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        T::AssetBalance,
        ValueQuery,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 实体代币已创建
        EntityTokenCreated {
            entity_id: u64,
            asset_id: T::AssetId,
            name: Vec<u8>,
            symbol: Vec<u8>,
        },
        /// 代币配置已更新
        TokenConfigUpdated { entity_id: u64 },
        /// 购物奖励已发放
        RewardIssued {
            entity_id: u64,
            buyer: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 积分已兑换
        TokensRedeemed {
            entity_id: u64,
            buyer: T::AccountId,
            tokens: T::AssetBalance,
            discount: T::AssetBalance,
        },
        /// 积分已转让
        TokensTransferred {
            entity_id: u64,
            from: T::AccountId,
            to: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币已铸造
        TokensMinted {
            entity_id: u64,
            to: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币已销毁
        TokensBurned {
            entity_id: u64,
            from: T::AccountId,
            amount: T::AssetBalance,
        },
        // ========== Phase 4 新增事件 ==========
        /// 分红已配置
        DividendConfigured {
            entity_id: u64,
            enabled: bool,
            min_period: BlockNumberFor<T>,
        },
        /// 分红已分发
        DividendDistributed {
            entity_id: u64,
            total_amount: T::AssetBalance,
            recipients_count: u32,
        },
        /// 分红已领取
        DividendClaimed {
            entity_id: u64,
            holder: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币已锁仓
        TokensLocked {
            entity_id: u64,
            holder: T::AccountId,
            amount: T::AssetBalance,
            unlock_at: BlockNumberFor<T>,
        },
        /// 代币已解锁
        TokensUnlocked {
            entity_id: u64,
            holder: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 通证类型已变更
        TokenTypeChanged {
            entity_id: u64,
            old_type: TokenType,
            new_type: TokenType,
        },
        // ========== Phase 8 新增事件：转账限制 ==========
        /// 转账限制模式已设置
        TransferRestrictionSet {
            entity_id: u64,
            mode: TransferRestrictionMode,
            min_receiver_kyc: u8,
        },
        /// 白名单已更新
        WhitelistUpdated {
            entity_id: u64,
            added: u32,
            removed: u32,
        },
        /// 黑名单已更新
        BlacklistUpdated {
            entity_id: u64,
            added: u32,
            removed: u32,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 店铺不存在
        ShopNotFound,
        /// 不是店主
        NotShopOwner,
        /// 店铺代币未启用
        TokenNotEnabled,
        /// 代币已存在
        TokenAlreadyExists,
        /// 余额不足
        InsufficientBalance,
        /// 低于最低兑换门槛
        BelowMinRedeem,
        /// 超过单笔最大兑换
        ExceedsMaxRedeem,
        /// 不允许转让
        TransferNotAllowed,
        /// 名称过长
        NameTooLong,
        /// 符号过长
        SymbolTooLong,
        /// 资产创建失败
        AssetCreationFailed,
        /// 无效的奖励率
        InvalidRewardRate,
        /// 无效的兑换率
        InvalidExchangeRate,
        // ========== Phase 4 新增错误 ==========
        /// 分红未启用
        DividendNotEnabled,
        /// 分红周期未到
        DividendPeriodNotReached,
        /// 无可领取分红
        NoDividendToClaim,
        /// 代币已锁仓
        TokensAreLocked,
        /// 无锁仓代币
        NoLockedTokens,
        /// 解锁时间未到
        UnlockTimeNotReached,
        /// 锁仓条目已满（最多 10 条）
        LocksFull,
        /// 超过最大供应量
        ExceedsMaxSupply,
        /// 通证类型不支持此操作
        TokenTypeNotSupported,
        /// 不允许该通证类型
        TokenTypeNotAllowed,
        // ========== Phase 8 新增错误：转账限制 ==========
        /// 接收方不在白名单
        ReceiverNotInWhitelist,
        /// 接收方在黑名单
        ReceiverInBlacklist,
        /// 接收方 KYC 级别不足
        ReceiverKycInsufficient,
        /// 接收方不是实体成员
        ReceiverNotMember,
        /// 白名单/黑名单已满
        TransferListFull,
        /// 地址已在列表中
        AddressAlreadyInList,
        /// 地址不在列表中
        AddressNotInList,
        /// 店铺未激活
        ShopNotActive,
        /// 数量为零
        ZeroAmount,
        /// 锁仓时长为零
        InvalidLockDuration,
        /// 分红接收人过多
        TooManyRecipients,
        /// 分红总额不匹配
        DividendAmountMismatch,
        /// 兑换限额设置无效（min > max）
        InvalidRedeemLimits,
        /// 名称为空
        EmptyName,
        /// 符号为空
        EmptySymbol,
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 为实体创建代币（Entity 级统一代币）
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        /// - `name`: 代币名称
        /// - `symbol`: 代币符号
        /// - `decimals`: 小数位数
        /// - `reward_rate`: 购物返积分比例（基点）
        /// - `exchange_rate`: 积分兑换比例（基点）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(200_000_000, 5_000))]
        pub fn create_shop_token(
            origin: OriginFor<T>,
            entity_id: u64,
            name: Vec<u8>,
            symbol: Vec<u8>,
            decimals: u8,
            reward_rate: u16,
            exchange_rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体存在且调用者是所有者
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::ShopNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::ShopNotActive);
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // 检查代币是否已存在
            ensure!(!EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenAlreadyExists);

            // 验证参数
            ensure!(reward_rate <= 10000, Error::<T>::InvalidRewardRate);
            ensure!(exchange_rate <= 10000, Error::<T>::InvalidExchangeRate);
            // L1: 名称和符号不能为空
            ensure!(!name.is_empty(), Error::<T>::EmptyName);
            ensure!(!symbol.is_empty(), Error::<T>::EmptySymbol);

            // 转换名称和符号
            let name_bounded: BoundedVec<u8, T::MaxTokenNameLength> =
                name.clone().try_into().map_err(|_| Error::<T>::NameTooLong)?;
            let symbol_bounded: BoundedVec<u8, T::MaxTokenSymbolLength> =
                symbol.clone().try_into().map_err(|_| Error::<T>::SymbolTooLong)?;

            // 计算资产 ID
            let asset_id = Self::entity_to_asset_id(entity_id);

            // 通过 pallet-assets 创建资产
            T::Assets::create(asset_id, who.clone(), true, 1u32.into())
                .map_err(|_| Error::<T>::AssetCreationFailed)?;

            // 设置元数据
            T::Assets::set(asset_id, &who, name.clone(), symbol.clone(), decimals)
                .map_err(|_| Error::<T>::AssetCreationFailed)?;

            // 保存配置
            let now = <frame_system::Pallet<T>>::block_number();
            let token_type = TokenType::Points;
            let config = EntityTokenConfig {
                enabled: true,
                reward_rate,
                exchange_rate,
                min_redeem: Zero::zero(),
                max_redeem_per_order: Zero::zero(),
                transferable: true,
                created_at: now,
                token_type,
                max_supply: Zero::zero(),
                dividend_config: DividendConfig {
                    enabled: false,
                    min_period: Zero::zero(),
                    last_distribution: Zero::zero(),
                    accumulated: Zero::zero(),
                },
                transfer_restriction: TransferRestrictionMode::from_u8(token_type.default_transfer_restriction()),
                min_receiver_kyc: token_type.required_kyc_level().1,
            };
            EntityTokenConfigs::<T>::insert(entity_id, config);
            EntityTokenMetadata::<T>::insert(entity_id, (name_bounded, symbol_bounded, decimals));
            TotalEntityTokens::<T>::mutate(|n| *n = n.saturating_add(1));

            Self::deposit_event(Event::EntityTokenCreated {
                entity_id,
                asset_id,
                name,
                symbol,
            });

            Ok(())
        }

        /// 更新代币配置
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn update_token_config(
            origin: OriginFor<T>,
            entity_id: u64,
            reward_rate: Option<u16>,
            exchange_rate: Option<u16>,
            min_redeem: Option<T::AssetBalance>,
            max_redeem_per_order: Option<T::AssetBalance>,
            transferable: Option<bool>,
            enabled: Option<bool>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;

                if let Some(rate) = reward_rate {
                    ensure!(rate <= 10000, Error::<T>::InvalidRewardRate);
                    config.reward_rate = rate;
                }
                if let Some(rate) = exchange_rate {
                    ensure!(rate <= 10000, Error::<T>::InvalidExchangeRate);
                    config.exchange_rate = rate;
                }
                if let Some(min) = min_redeem {
                    config.min_redeem = min;
                }
                if let Some(max) = max_redeem_per_order {
                    config.max_redeem_per_order = max;
                }
                if let Some(t) = transferable {
                    config.transferable = t;
                }
                if let Some(e) = enabled {
                    config.enabled = e;
                }

                // L6: 验证 min_redeem <= max_redeem_per_order
                if !config.max_redeem_per_order.is_zero() && config.min_redeem > config.max_redeem_per_order {
                    return Err(Error::<T>::InvalidRedeemLimits.into());
                }

                Ok(())
            })?;

            Self::deposit_event(Event::TokenConfigUpdated { entity_id });
            Ok(())
        }

        /// 实体所有者铸造代币（用于活动奖励等）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(150_000_000, 4_000))]
        pub fn mint_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            to: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证实体所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // 检查代币是否启用
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            // T-L1 审计修复: 禁止铸造零数量
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

            // H1: 检查 max_supply
            Self::ensure_within_max_supply(entity_id, &config, amount)?;

            // 铸造代币
            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::mint_into(asset_id, &to, amount)?;

            Self::deposit_event(Event::TokensMinted {
                entity_id,
                to,
                amount,
            });

            Ok(())
        }

        /// 用户转让积分
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(150_000_000, 5_000))]
        pub fn transfer_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            to: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 检查代币配置
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            ensure!(config.transferable, Error::<T>::TransferNotAllowed);

            // Phase 8: 检查转账限制
            Self::check_transfer_restriction(entity_id, &config, &to)?;

            // H4: 检查可用余额（扣除锁仓和预留）
            let asset_id = Self::entity_to_asset_id(entity_id);
            let balance = T::Assets::balance(asset_id, &who);
            let locked = Self::total_locked_amount(entity_id, &who);
            let reserved = ReservedTokens::<T>::get(entity_id, &who);
            let available = balance.saturating_sub(locked).saturating_sub(reserved);
            ensure!(available >= amount, Error::<T>::InsufficientBalance);

            // 转账
            T::Assets::transfer(asset_id, &who, &to, amount, frame_support::traits::tokens::Preservation::Preserve)?;

            Self::deposit_event(Event::TokensTransferred {
                entity_id,
                from: who,
                to,
                amount,
            });

            Ok(())
        }

        // ==================== Phase 4 新增 Extrinsics ====================

        /// 配置分红
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn configure_dividend(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
            min_period: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // 检查代币是否存在
            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                
                // 检查通证类型是否支持分红
                ensure!(config.token_type.has_dividend_rights(), Error::<T>::TokenTypeNotSupported);
                
                config.dividend_config.enabled = enabled;
                config.dividend_config.min_period = min_period;
                
                Ok(())
            })?;

            Self::deposit_event(Event::DividendConfigured {
                entity_id,
                enabled,
                min_period,
            });
            Ok(())
        }

        /// 分发分红（按持有比例分配）
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(300_000_000, 10_000))]
        pub fn distribute_dividend(
            origin: OriginFor<T>,
            entity_id: u64,
            total_amount: T::AssetBalance,
            recipients: Vec<(T::AccountId, T::AssetBalance)>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            // H5: 限制接收人数量
            ensure!(
                recipients.len() <= T::MaxDividendRecipients::get() as usize,
                Error::<T>::TooManyRecipients
            );

            // 检查分红配置
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.dividend_config.enabled, Error::<T>::DividendNotEnabled);
            // M6: 检查通证类型是否支持分红
            ensure!(config.token_type.has_dividend_rights(), Error::<T>::TokenTypeNotSupported);

            let now = <frame_system::Pallet<T>>::block_number();
            let last = config.dividend_config.last_distribution;
            let min_period = config.dividend_config.min_period;
            
            // 检查分红周期（首次分红跳过检查）
            if !last.is_zero() {
                ensure!(now >= last + min_period, Error::<T>::DividendPeriodNotReached);
            }

            // H6: 校验 total_amount == sum(recipients)
            let mut sum = T::AssetBalance::zero();
            for (_, amount) in recipients.iter() {
                sum = sum.saturating_add(*amount);
            }
            ensure!(sum == total_amount, Error::<T>::DividendAmountMismatch);

            // 分配分红到待领取
            let mut count = 0u32;
            for (holder, amount) in recipients.iter() {
                if !amount.is_zero() {
                    PendingDividends::<T>::mutate(entity_id, holder, |pending| {
                        *pending = pending.saturating_add(*amount);
                    });
                    count = count.saturating_add(1);
                }
            }

            // 更新上次分红时间和累计金额
            EntityTokenConfigs::<T>::mutate(entity_id, |maybe_config| {
                if let Some(config) = maybe_config {
                    config.dividend_config.last_distribution = now;
                    // L5: 更新累计分红金额
                    config.dividend_config.accumulated = config.dividend_config.accumulated.saturating_add(total_amount);
                }
            });

            Self::deposit_event(Event::DividendDistributed {
                entity_id,
                total_amount,
                recipients_count: count,
            });
            Ok(())
        }

        /// 领取分红
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(150_000_000, 4_000))]
        pub fn claim_dividend(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let pending = PendingDividends::<T>::get(entity_id, &who);
            ensure!(!pending.is_zero(), Error::<T>::NoDividendToClaim);

            // H3: 检查 max_supply
            if let Some(config) = EntityTokenConfigs::<T>::get(entity_id) {
                Self::ensure_within_max_supply(entity_id, &config, pending)?;
            }

            // 清空待领取
            PendingDividends::<T>::remove(entity_id, &who);

            // 更新已领取总额
            ClaimedDividends::<T>::mutate(entity_id, &who, |claimed| {
                *claimed = claimed.saturating_add(pending);
            });

            // 铸造分红代币给持有人（或从国库转出，这里简化为铸造）
            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::mint_into(asset_id, &who, pending)?;

            Self::deposit_event(Event::DividendClaimed {
                entity_id,
                holder: who,
                amount: pending,
            });
            Ok(())
        }

        /// 锁仓代币
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(150_000_000, 4_000))]
        pub fn lock_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: T::AssetBalance,
            lock_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M2: 检查通证是否启用
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            // M3: amount > 0
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            // M4: duration > 0
            ensure!(!lock_duration.is_zero(), Error::<T>::InvalidLockDuration);

            // 检查余额
            let asset_id = Self::entity_to_asset_id(entity_id);
            let balance = T::Assets::balance(asset_id, &who);

            // T-M1: 计算所有未过期锁仓总额
            let existing_locked = Self::total_locked_amount(entity_id, &who);

            let reserved = ReservedTokens::<T>::get(entity_id, &who);
            let available = balance.saturating_sub(existing_locked).saturating_sub(reserved);
            ensure!(available >= amount, Error::<T>::InsufficientBalance);

            let now = <frame_system::Pallet<T>>::block_number();
            let unlock_at = now.saturating_add(lock_duration);

            // T-M1: 添加独立锁仓条目（不合并，各自独立到期）
            LockedTokens::<T>::try_mutate(entity_id, &who, |entries| -> DispatchResult {
                entries.try_push(LockEntry { amount, unlock_at })
                    .map_err(|_| Error::<T>::LocksFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::TokensLocked {
                entity_id,
                holder: who,
                amount,
                unlock_at,
            });
            Ok(())
        }

        /// 解锁代币（T-M1: 仅移除已到期的锁仓条目，保留未到期的）
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn unlock_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let mut unlocked_total = T::AssetBalance::zero();

            LockedTokens::<T>::mutate(entity_id, &who, |entries| {
                let mut i = 0;
                while i < entries.len() {
                    if now >= entries[i].unlock_at {
                        unlocked_total = unlocked_total.saturating_add(entries[i].amount);
                        entries.swap_remove(i);
                    } else {
                        i += 1;
                    }
                }
            });

            ensure!(!unlocked_total.is_zero(), Error::<T>::NoLockedTokens);

            Self::deposit_event(Event::TokensUnlocked {
                entity_id,
                holder: who,
                amount: unlocked_total,
            });
            Ok(())
        }

        /// 变更通证类型（需所有者操作）
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn change_token_type(
            origin: OriginFor<T>,
            entity_id: u64,
            new_type: TokenType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let old_type = EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> Result<TokenType, DispatchError> {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                let old = config.token_type;
                config.token_type = new_type;
                
                // 根据新类型更新可转让性
                config.transferable = new_type.is_transferable_by_default();
                // M5: 联动更新转账限制和 KYC 要求
                config.transfer_restriction = TransferRestrictionMode::from_u8(new_type.default_transfer_restriction());
                config.min_receiver_kyc = new_type.required_kyc_level().1;
                
                Ok(old)
            })?;

            Self::deposit_event(Event::TokenTypeChanged {
                entity_id,
                old_type,
                new_type,
            });
            Ok(())
        }

        /// 设置最大供应量
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn set_max_supply(
            origin: OriginFor<T>,
            entity_id: u64,
            max_supply: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                
                // 检查当前供应量是否超过新的最大值
                let current_supply = Self::get_total_supply(entity_id);
                if !max_supply.is_zero() {
                    ensure!(current_supply <= max_supply, Error::<T>::ExceedsMaxSupply);
                }
                
                config.max_supply = max_supply;
                Ok(())
            })?;

            Self::deposit_event(Event::TokenConfigUpdated { entity_id });
            Ok(())
        }

        // ==================== Phase 8 新增 Extrinsics：转账限制 ====================

        /// 设置转账限制模式
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(100_000_000, 4_000))]
        pub fn set_transfer_restriction(
            origin: OriginFor<T>,
            entity_id: u64,
            mode: TransferRestrictionMode,
            min_receiver_kyc: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let clamped_kyc = min_receiver_kyc.min(4);
            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                config.transfer_restriction = mode;
                config.min_receiver_kyc = clamped_kyc;
                Ok(())
            })?;

            // L3: 事件使用 clamped 后的值
            Self::deposit_event(Event::TransferRestrictionSet {
                entity_id,
                mode,
                min_receiver_kyc: clamped_kyc,
            });
            Ok(())
        }

        /// 添加白名单地址
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(150_000_000, 5_000))]
        pub fn add_to_whitelist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let mut added = 0u32;
            TransferWhitelist::<T>::try_mutate(entity_id, |list| -> DispatchResult {
                for account in accounts.iter() {
                    if !list.contains(account) {
                        list.try_push(account.clone()).map_err(|_| Error::<T>::TransferListFull)?;
                        added = added.saturating_add(1);
                    }
                }
                Ok(())
            })?;

            Self::deposit_event(Event::WhitelistUpdated {
                entity_id,
                added,
                removed: 0,
            });
            Ok(())
        }

        /// 移除白名单地址
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(150_000_000, 5_000))]
        pub fn remove_from_whitelist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let mut removed = 0u32;
            TransferWhitelist::<T>::mutate(entity_id, |list| {
                for account in accounts.iter() {
                    if let Some(pos) = list.iter().position(|x| x == account) {
                        list.swap_remove(pos);
                        removed = removed.saturating_add(1);
                    }
                }
            });

            Self::deposit_event(Event::WhitelistUpdated {
                entity_id,
                added: 0,
                removed,
            });
            Ok(())
        }

        /// 添加黑名单地址
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(150_000_000, 5_000))]
        pub fn add_to_blacklist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let mut added = 0u32;
            TransferBlacklist::<T>::try_mutate(entity_id, |list| -> DispatchResult {
                for account in accounts.iter() {
                    if !list.contains(account) {
                        list.try_push(account.clone()).map_err(|_| Error::<T>::TransferListFull)?;
                        added = added.saturating_add(1);
                    }
                }
                Ok(())
            })?;

            Self::deposit_event(Event::BlacklistUpdated {
                entity_id,
                added,
                removed: 0,
            });
            Ok(())
        }

        /// 移除黑名单地址
        #[pallet::call_index(15)]
        #[pallet::weight(Weight::from_parts(150_000_000, 5_000))]
        pub fn remove_from_blacklist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // 验证所有者
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::ShopNotFound)?;
            ensure!(owner == who, Error::<T>::NotShopOwner);

            let mut removed = 0u32;
            TransferBlacklist::<T>::mutate(entity_id, |list| {
                for account in accounts.iter() {
                    if let Some(pos) = list.iter().position(|x| x == account) {
                        list.swap_remove(pos);
                        removed = removed.saturating_add(1);
                    }
                }
            });

            Self::deposit_event(Event::BlacklistUpdated {
                entity_id,
                added: 0,
                removed,
            });
            Ok(())
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// Entity ID 转资产 ID
        pub fn entity_to_asset_id(entity_id: u64) -> T::AssetId {
            (T::ShopTokenOffset::get() + entity_id).into()
        }

        /// 向后兼容别名
        pub fn shop_to_asset_id(shop_id: u64) -> T::AssetId {
            Self::entity_to_asset_id(shop_id)
        }

        /// T-M1: 计算用户所有未过期锁仓总额
        pub fn total_locked_amount(entity_id: u64, who: &T::AccountId) -> T::AssetBalance {
            let now = <frame_system::Pallet<T>>::block_number();
            LockedTokens::<T>::get(entity_id, who)
                .iter()
                .filter(|e| now < e.unlock_at)
                .fold(T::AssetBalance::zero(), |acc, e| acc.saturating_add(e.amount))
        }

        /// Phase 8: 检查转账限制
        fn check_transfer_restriction(
            entity_id: u64,
            config: &ShopTokenConfigOf<T>,
            to: &T::AccountId,
        ) -> DispatchResult {
            match config.transfer_restriction {
                TransferRestrictionMode::None => Ok(()),
                
                TransferRestrictionMode::Whitelist => {
                    let whitelist = TransferWhitelist::<T>::get(entity_id);
                    ensure!(whitelist.contains(to), Error::<T>::ReceiverNotInWhitelist);
                    Ok(())
                }
                
                TransferRestrictionMode::Blacklist => {
                    let blacklist = TransferBlacklist::<T>::get(entity_id);
                    ensure!(!blacklist.contains(to), Error::<T>::ReceiverInBlacklist);
                    Ok(())
                }
                
                TransferRestrictionMode::KycRequired => {
                    ensure!(
                        T::KycProvider::meets_kyc_requirement(to, config.min_receiver_kyc),
                        Error::<T>::ReceiverKycInsufficient
                    );
                    Ok(())
                }
                
                TransferRestrictionMode::MembersOnly => {
                    ensure!(
                        T::MemberProvider::is_member(entity_id, to),
                        Error::<T>::ReceiverNotMember
                    );
                    Ok(())
                }
            }
        }

        /// 检查铸造是否在 max_supply 范围内
        fn ensure_within_max_supply(
            entity_id: u64,
            config: &ShopTokenConfigOf<T>,
            mint_amount: T::AssetBalance,
        ) -> DispatchResult {
            if !config.max_supply.is_zero() {
                let current_supply = Self::get_total_supply(entity_id);
                ensure!(
                    current_supply.saturating_add(mint_amount) <= config.max_supply,
                    Error::<T>::ExceedsMaxSupply
                );
            }
            Ok(())
        }

        /// 资产 ID 转 Entity ID
        pub fn asset_to_entity_id(asset_id: T::AssetId) -> Option<u64> {
            let id: u64 = asset_id.into();
            let offset = T::ShopTokenOffset::get();
            if id >= offset {
                Some(id - offset)
            } else {
                None
            }
        }

        /// 向后兼容别名
        pub fn asset_to_shop_id(asset_id: T::AssetId) -> Option<u64> {
            Self::asset_to_entity_id(asset_id)
        }

        /// 获取用户在某实体的代币余额
        pub fn token_balance(entity_id: u64, holder: &T::AccountId) -> T::AssetBalance {
            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::balance(asset_id, holder)
        }

        /// 购物奖励（由 order 模块调用，传入 entity_id）
        pub fn reward_on_purchase(
            entity_id: u64,
            buyer: &T::AccountId,
            purchase_amount: T::AssetBalance,
        ) -> Result<T::AssetBalance, DispatchError> {
            let config = match EntityTokenConfigs::<T>::get(entity_id) {
                Some(c) if c.enabled && c.reward_rate > 0 => c,
                _ => return Ok(Zero::zero()),
            };

            // 计算奖励：purchase_amount * reward_rate / 10000
            let reward = purchase_amount
                .saturating_mul(config.reward_rate.into())
                / 10000u32.into();

            if reward.is_zero() {
                return Ok(Zero::zero());
            }

            // H2: 检查 max_supply，超过上限时静默跳过（不报错，避免阻塞订单流程）
            if !config.max_supply.is_zero() {
                let current_supply = Self::get_total_supply(entity_id);
                if current_supply.saturating_add(reward) > config.max_supply {
                    return Ok(Zero::zero());
                }
            }

            // 铸造代币给买家
            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::mint_into(asset_id, buyer, reward)?;

            Self::deposit_event(Event::RewardIssued {
                entity_id,
                buyer: buyer.clone(),
                amount: reward,
            });

            Ok(reward)
        }

        /// 积分兑换折扣（由 order 模块调用，传入 entity_id）
        pub fn redeem_for_discount(
            entity_id: u64,
            buyer: &T::AccountId,
            tokens_to_use: T::AssetBalance,
        ) -> Result<T::AssetBalance, DispatchError> {
            let config = EntityTokenConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::TokenNotEnabled)?;

            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            ensure!(tokens_to_use >= config.min_redeem, Error::<T>::BelowMinRedeem);
            ensure!(
                config.max_redeem_per_order.is_zero() || tokens_to_use <= config.max_redeem_per_order,
                Error::<T>::ExceedsMaxRedeem
            );

            // T-H1 审计修复: 检查可用余额（扣除锁仓和预留）
            let asset_id = Self::entity_to_asset_id(entity_id);
            let balance = T::Assets::balance(asset_id, buyer);
            let locked = Self::total_locked_amount(entity_id, buyer);
            let reserved = ReservedTokens::<T>::get(entity_id, buyer);
            let available = balance.saturating_sub(locked).saturating_sub(reserved);
            ensure!(available >= tokens_to_use, Error::<T>::InsufficientBalance);

            // 计算折扣：tokens * exchange_rate / 10000
            let discount = tokens_to_use
                .saturating_mul(config.exchange_rate.into())
                / 10000u32.into();

            // 销毁积分
            T::Assets::burn_from(
                asset_id,
                buyer,
                tokens_to_use,
                frame_support::traits::tokens::Preservation::Expendable,
                frame_support::traits::tokens::Precision::Exact,
                frame_support::traits::tokens::Fortitude::Polite,
            )?;

            Self::deposit_event(Event::TokensRedeemed {
                entity_id,
                buyer: buyer.clone(),
                tokens: tokens_to_use,
                discount,
            });

            Ok(discount)
        }
    }
}

// ==================== 公共查询函数 ====================

impl<T: Config> Pallet<T> {
    /// 获取用户在实体的代币余额（公共接口）
    pub fn get_balance(entity_id: u64, holder: &T::AccountId) -> T::AssetBalance {
        Self::token_balance(entity_id, holder)
    }

    /// 获取实体代币总供应量
    pub fn get_total_supply(entity_id: u64) -> T::AssetBalance {
        use frame_support::traits::fungibles::Inspect;
        let asset_id = Self::entity_to_asset_id(entity_id);
        T::Assets::total_issuance(asset_id)
    }

    /// 检查实体代币是否启用
    pub fn is_token_enabled(entity_id: u64) -> bool {
        EntityTokenConfigs::<T>::get(entity_id)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }
}

// ==================== EntityTokenProvider 实现 ====================

use pallet_entity_common::{EntityTokenProvider, TokenType};
use sp_runtime::traits::{Zero as _Zero, Saturating as _Saturating};

impl<T: Config> EntityTokenProvider<T::AccountId, T::AssetBalance> for Pallet<T> {
    fn is_token_enabled(entity_id: u64) -> bool {
        Pallet::<T>::is_token_enabled(entity_id)
    }

    fn token_balance(entity_id: u64, holder: &T::AccountId) -> T::AssetBalance {
        Pallet::<T>::get_balance(entity_id, holder)
    }

    fn reward_on_purchase(
        entity_id: u64,
        buyer: &T::AccountId,
        purchase_amount: T::AssetBalance,
    ) -> Result<T::AssetBalance, sp_runtime::DispatchError> {
        Pallet::<T>::reward_on_purchase(entity_id, buyer, purchase_amount)
    }

    fn redeem_for_discount(
        entity_id: u64,
        buyer: &T::AccountId,
        tokens: T::AssetBalance,
    ) -> Result<T::AssetBalance, sp_runtime::DispatchError> {
        Pallet::<T>::redeem_for_discount(entity_id, buyer, tokens)
    }

    fn transfer(
        entity_id: u64,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: T::AssetBalance,
    ) -> Result<(), sp_runtime::DispatchError> {
        use frame_support::traits::fungibles::Mutate;
        let asset_id = Pallet::<T>::entity_to_asset_id(entity_id);
        T::Assets::transfer(
            asset_id,
            from,
            to,
            amount,
            frame_support::traits::tokens::Preservation::Preserve,
        )?;
        Ok(())
    }

    fn reserve(
        entity_id: u64,
        who: &T::AccountId,
        amount: T::AssetBalance,
    ) -> Result<(), sp_runtime::DispatchError> {
        use frame_support::traits::fungibles::Inspect;
        let asset_id = Pallet::<T>::entity_to_asset_id(entity_id);
        let balance = T::Assets::balance(asset_id, who);
        let locked = Pallet::<T>::total_locked_amount(entity_id, who);
        let already_reserved = pallet::ReservedTokens::<T>::get(entity_id, who);
        let available = balance
            .saturating_sub(locked)
            .saturating_sub(already_reserved);
        if available < amount {
            return Err(sp_runtime::DispatchError::Other("InsufficientBalance"));
        }
        pallet::ReservedTokens::<T>::mutate(entity_id, who, |r| {
            *r = r.saturating_add(amount);
        });
        Ok(())
    }

    fn unreserve(
        entity_id: u64,
        who: &T::AccountId,
        amount: T::AssetBalance,
    ) -> T::AssetBalance {
        let current = pallet::ReservedTokens::<T>::get(entity_id, who);
        let actual = amount.min(current);
        if actual.is_zero() {
            return actual;
        }
        pallet::ReservedTokens::<T>::mutate(entity_id, who, |r| {
            *r = r.saturating_sub(actual);
        });
        actual
    }

    fn repatriate_reserved(
        entity_id: u64,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: T::AssetBalance,
    ) -> Result<T::AssetBalance, sp_runtime::DispatchError> {
        let reserved = pallet::ReservedTokens::<T>::get(entity_id, from);
        let actual = amount.min(reserved);
        if actual.is_zero() {
            return Ok(actual);
        }
        // 减少 from 的预留
        pallet::ReservedTokens::<T>::mutate(entity_id, from, |r| {
            *r = r.saturating_sub(actual);
        });
        // 实际转账
        Self::transfer(entity_id, from, to, actual)?;
        Ok(actual)
    }

    fn get_token_type(entity_id: u64) -> TokenType {
        EntityTokenConfigs::<T>::get(entity_id)
            .map(|c| c.token_type)
            .unwrap_or_default()
    }

    fn total_supply(entity_id: u64) -> T::AssetBalance {
        Pallet::<T>::get_total_supply(entity_id)
    }
}
