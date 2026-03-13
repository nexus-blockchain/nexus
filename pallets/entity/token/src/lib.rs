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

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

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
    use pallet_entity_common::{AdminPermission, DividendConfig, DisclosureProvider, EntityProvider, TokenType, TransferRestrictionMode};
    use sp_runtime::traits::{AtLeast32BitUnsigned, Saturating, Zero};

    pub use crate::weights::WeightInfo;

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

    /// 配置类型别名
    pub type EntityTokenConfigOf<T> = EntityTokenConfig<
        <T as Config>::AssetBalance,
        BlockNumberFor<T>,
    >;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
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

        /// 披露查询接口（黑窗口期内幕人员交易限制）
        type DisclosureProvider: DisclosureProvider<Self::AccountId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// KYC 级别查询 Trait（per-entity: 用户在指定 Entity 下的 KYC 级别）
    pub trait KycLevelProvider<AccountId> {
        /// 获取用户在指定 Entity 下的 KYC 级别 (0-4)
        fn get_kyc_level(entity_id: u64, account: &AccountId) -> u8;
        /// 检查用户在指定 Entity 下是否满足 KYC 要求
        fn meets_kyc_requirement(entity_id: u64, account: &AccountId, min_level: u8) -> bool;
    }

    /// 实体成员查询 Trait
    pub trait EntityMemberProvider<AccountId> {
        /// 检查是否为实体成员
        fn is_member(entity_id: u64, account: &AccountId) -> bool;
    }

    /// 空 KYC 提供者（默认实现）
    pub struct NullKycProvider;
    impl<AccountId> KycLevelProvider<AccountId> for NullKycProvider {
        fn get_kyc_level(_entity_id: u64, _account: &AccountId) -> u8 { 0 }
        fn meets_kyc_requirement(_entity_id: u64, _account: &AccountId, min_level: u8) -> bool { min_level == 0 }
    }

    /// 空成员提供者（默认实现）
    pub struct NullMemberProvider;
    impl<AccountId> EntityMemberProvider<AccountId> for NullMemberProvider {
        fn is_member(_entity_id: u64, _account: &AccountId) -> bool { true }
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 实体代币配置存储（Entity 级统一代币）
    #[pallet::storage]
    #[pallet::getter(fn entity_token_configs)]
    pub type EntityTokenConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        EntityTokenConfigOf<T>,
    >;

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

    /// 统计：已创建的实体代币数量
    #[pallet::storage]
    #[pallet::getter(fn total_entity_tokens)]
    pub type TotalEntityTokens<T: Config> = StorageValue<_, u64, ValueQuery>;

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

    /// M1-R4: 实体待领取分红总额（已承诺但未铸造）
    /// distribute_dividend 时递增，claim_dividend 时递减
    #[pallet::storage]
    #[pallet::getter(fn total_pending_dividends)]
    pub type TotalPendingDividends<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        T::AssetBalance,
        ValueQuery,
    >;

    // ========== Phase 8 新增存储项：转账限制 ==========

    /// 转账白名单 (entity_id, account) -> ()  [O(1) 查询]
    #[pallet::storage]
    pub type TransferWhitelist<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        (),
        OptionQuery,
    >;

    /// 转账黑名单 (entity_id, account) -> ()  [O(1) 查询]
    #[pallet::storage]
    pub type TransferBlacklist<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        Blake2_128Concat,
        T::AccountId,
        (),
        OptionQuery,
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

    // ========== P1/P3 紧急管控存储项 ==========

    /// P1: 实体转账冻结标记 (entity_id) -> ()
    /// 存在即表示该实体的代币转账已被冻结，分红领取不受影响
    #[pallet::storage]
    #[pallet::getter(fn transfers_frozen)]
    pub type TransfersFrozen<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        (),
        OptionQuery,
    >;

    /// P3: 全平台代币紧急暂停开关
    /// true = 所有涉及 pallet-assets 的操作被暂停
    #[pallet::storage]
    #[pallet::getter(fn global_token_paused)]
    pub type GlobalTokenPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

    // ========== R4 审计新增存储项 ==========

    /// 代币授权额度 (entity_id, owner, spender) -> amount
    #[pallet::storage]
    #[pallet::getter(fn token_approvals)]
    pub type TokenApprovals<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, u64>,           // entity_id
            NMapKey<Blake2_128Concat, T::AccountId>,  // owner
            NMapKey<Blake2_128Concat, T::AccountId>,  // spender
        ),
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
        // ========== P1/P2/P3 紧急管控事件 ==========
        /// P1: 代币已被平台强制禁用
        TokenForceDisabled { entity_id: u64 },
        /// P1: 代币转账已被冻结（分红领取不受影响）
        TransfersFrozenEvent { entity_id: u64 },
        /// P1: 代币转账冻结已解除
        TransfersUnfrozen { entity_id: u64 },
        /// P2: 代币已被平台强制销毁
        TokensForceBurned {
            entity_id: u64,
            from: T::AccountId,
            amount: T::AssetBalance,
        },
        /// P3: 全平台代币暂停状态变更
        GlobalTokenPauseSet { paused: bool },
        /// H4: 治理提案销毁代币
        TokensGovernanceBurned {
            entity_id: u64,
            from: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币已被持有人或所有者销毁
        TokensBurned {
            entity_id: u64,
            holder: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币元数据已更新
        TokenMetadataUpdated {
            entity_id: u64,
            name: Vec<u8>,
            symbol: Vec<u8>,
        },
        /// 合规强制转账
        TokensForceTransferred {
            entity_id: u64,
            from: T::AccountId,
            to: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币已被平台强制重新启用
        TokenForceEnabled { entity_id: u64 },
        // ========== R4 审计新增事件 ==========
        /// 待领取分红已被强制取消
        PendingDividendsCancelled {
            entity_id: u64,
            total_cancelled: T::AssetBalance,
            accounts_affected: u32,
        },
        /// 代币授权额度已设置
        TokenApprovalSet {
            entity_id: u64,
            owner: T::AccountId,
            spender: T::AccountId,
            amount: T::AssetBalance,
        },
        /// 代币已通过授权转账
        TokensTransferredFrom {
            entity_id: u64,
            owner: T::AccountId,
            spender: T::AccountId,
            to: T::AccountId,
            amount: T::AssetBalance,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 实体不存在
        EntityNotFound,
        // L1-R3: 移除死代码 NotEntityOwner（已被 NotAuthorized 取代）
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
        /// 变更为相同通证类型
        SameTokenType,
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
        /// 实体未激活
        EntityNotActive,
        /// 数量为零
        ZeroAmount,
        /// 锁仓时长为零
        InvalidLockDuration,
        /// 分红接收人过多
        TooManyRecipients,
        /// 分红总额为零
        ZeroDividendAmount,
        /// 分红总额不匹配
        DividendAmountMismatch,
        /// 兑换限额设置无效（min > max）
        InvalidRedeemLimits,
        /// 名称为空
        EmptyName,
        /// 符号为空
        EmptySymbol,
        /// 代币总数计数溢出
        TokenCountOverflow,
        /// 内幕人员黑窗口期内禁止交易
        InsiderTradingRestricted,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// 调用者既非实体所有者，也非拥有 TOKEN_MANAGE 权限的管理员
        NotAuthorized,
        // ========== P1/P2/P3 紧急管控错误 ==========
        /// P1: 该实体的代币转账已被冻结
        TokenTransfersFrozen,
        /// P3: 全平台代币操作已暂停
        GlobalPaused,
        /// P1: 代币已处于禁用状态
        TokenAlreadyDisabled,
        /// P1: 转账未被冻结，无需解冻
        TransfersNotFrozen,
        /// P1: 转账已被冻结，无需重复冻结
        TransfersAlreadyFrozen,
        // ========== 发送方限制错误 ==========
        /// 发送方不在白名单
        SenderNotInWhitelist,
        /// 发送方在黑名单
        SenderInBlacklist,
        /// 发送方 KYC 级别不足
        SenderKycInsufficient,
        /// 发送方不是实体成员
        SenderNotMember,
        /// 代币已处于启用状态
        TokenAlreadyEnabled,
        // ========== R4 审计新增错误 ==========
        /// 自转账（from == to）
        SelfTransfer,
        /// 资产 ID 溢出（ShopTokenOffset + entity_id > u64::MAX）
        AssetIdOverflow,
        /// 无待取消的分红
        NoPendingDividendsToCancel,
        /// 授权额度不足
        InsufficientAllowance,
        /// D-1: 实体处于披露处罚状态（Restricted 及以上），交易受限
        DisclosurePenaltyRestricted,
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
        #[pallet::weight(T::WeightInfo::create_shop_token())]
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

            // P3: 全平台暂停检查
            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);

            // P0: 验证实体存在且调用者是所有者或 TOKEN_MANAGE 管理员
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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

            // R4: 资产 ID 溢出保护
            T::ShopTokenOffset::get().checked_add(entity_id).ok_or(Error::<T>::AssetIdOverflow)?;
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
                transfer_restriction: token_type.default_transfer_restriction(),
                min_receiver_kyc: token_type.required_kyc_level().1,
            };
            EntityTokenConfigs::<T>::insert(entity_id, config);
            EntityTokenMetadata::<T>::insert(entity_id, (name_bounded, symbol_bounded, decimals));
            TotalEntityTokens::<T>::try_mutate(|n| -> Result<(), DispatchError> {
                *n = n.checked_add(1).ok_or(Error::<T>::TokenCountOverflow)?;
                Ok(())
            })?;

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
        #[pallet::weight(T::WeightInfo::update_token_config())]
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

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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
        #[pallet::weight(T::WeightInfo::mint_tokens())]
        pub fn mint_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            to: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P3: 全平台暂停检查
            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 检查代币是否启用
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            // M3: 铸造需要 Entity 处于活跃状态
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
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
        #[pallet::weight(T::WeightInfo::transfer_tokens())]
        pub fn transfer_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            to: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P3: 全平台暂停检查
            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);
            // P1: 实体转账冻结检查
            ensure!(!TransfersFrozen::<T>::contains_key(entity_id), Error::<T>::TokenTransfersFrozen);

            // 检查代币配置
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            ensure!(config.transferable, Error::<T>::TransferNotAllowed);
            // L1-R3: 被封禁/暂停的 Entity 代币不允许转账
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            // M1: 禁止零数量转账
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            // R4: 禁止自转账（浪费 gas 的无效操作）
            ensure!(who != to, Error::<T>::SelfTransfer);

            // Phase 8: 检查转账限制（双向）
            Self::check_transfer_restriction(entity_id, &config, &who, &to)?;

            // P0-a6: 内幕人员黑窗口期限制
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &who),
                Error::<T>::InsiderTradingRestricted
            );
            // D-1: 披露处罚限制
            ensure!(
                !T::DisclosureProvider::is_penalty_active(entity_id),
                Error::<T>::DisclosurePenaltyRestricted
            );

            // H4: 检查可用余额（扣除锁仓和预留）
            Self::ensure_available_balance(entity_id, &who, amount)?;

            // 转账
            let asset_id = Self::entity_to_asset_id(entity_id);
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
        #[pallet::weight(T::WeightInfo::configure_dividend())]
        pub fn configure_dividend(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
            min_period: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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
        #[pallet::weight(T::WeightInfo::distribute_dividend(recipients.len() as u32))]
        pub fn distribute_dividend(
            origin: OriginFor<T>,
            entity_id: u64,
            total_amount: T::AssetBalance,
            recipients: BoundedVec<(T::AccountId, T::AssetBalance), T::MaxDividendRecipients>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            // M1-R3: 分发分红创建铸造义务，需 Entity 处于活跃状态
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            // 检查分红配置
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            // H1: 通证必须启用
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            ensure!(config.dividend_config.enabled, Error::<T>::DividendNotEnabled);
            // M6: 检查通证类型是否支持分红
            ensure!(config.token_type.has_dividend_rights(), Error::<T>::TokenTypeNotSupported);
            // M2: 禁止零总额分红（防止滥用重置冷却计时器）
            ensure!(!total_amount.is_zero(), Error::<T>::ZeroDividendAmount);

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

            // H2-R3 + M1-R4: 预检查 max_supply 确保分红总额有铸造空间
            // 必须包含已承诺但未领取的分红（TotalPendingDividends），
            // 否则多次 distribute 可承诺超过 max_supply 的分红总额。
            if !config.max_supply.is_zero() {
                let current_supply = Self::get_total_supply(entity_id);
                let existing_pending = TotalPendingDividends::<T>::get(entity_id);
                ensure!(
                    current_supply.saturating_add(existing_pending).saturating_add(total_amount) <= config.max_supply,
                    Error::<T>::ExceedsMaxSupply
                );
            }

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

            // M1-R4: 递增实体待领取分红总额
            TotalPendingDividends::<T>::mutate(entity_id, |p| {
                *p = p.saturating_add(total_amount);
            });

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
        #[pallet::weight(T::WeightInfo::claim_dividend())]
        pub fn claim_dividend(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P3: 全平台暂停检查（claim 会铸造代币，需要 pallet-assets）
            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);

            // R4: 代币必须处于启用状态，force_disable 后阻止铸造新代币
            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);

            let pending = PendingDividends::<T>::get(entity_id, &who);
            ensure!(!pending.is_zero(), Error::<T>::NoDividendToClaim);

            // H2-R3: 移除 claim 时的 max_supply 检查。
            // 分红在 distribute_dividend 时已预检查 max_supply，承诺的分红应始终可领取。
            // 旧逻辑允许 owner 在 distribute 后操控 max_supply（铸币填满或降低上限），
            // 导致 PendingDividends 永久无法领取且无清理机制。

            // 清空待领取
            PendingDividends::<T>::remove(entity_id, &who);

            // M1-R4: 递减实体待领取分红总额
            TotalPendingDividends::<T>::mutate(entity_id, |p| {
                *p = p.saturating_sub(pending);
            });

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
        #[pallet::weight(T::WeightInfo::lock_tokens())]
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
            // L1-R3: 被封禁/暂停的 Entity 代币不允许新增锁仓
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            // M3: amount > 0
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            // M4: duration > 0
            ensure!(!lock_duration.is_zero(), Error::<T>::InvalidLockDuration);

            // 检查可用余额（扣除锁仓和预留）
            Self::ensure_available_balance(entity_id, &who, amount)?;

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
        #[pallet::weight(T::WeightInfo::unlock_tokens())]
        pub fn unlock_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let mut unlocked_total = T::AssetBalance::zero();

            // L1-R4: 合并为单次 try_mutate，减少 3 次读取到 1 次
            LockedTokens::<T>::try_mutate_exists(entity_id, &who, |maybe_entries| -> DispatchResult {
                let entries = maybe_entries.as_mut()
                    .filter(|e| !e.is_empty())
                    .ok_or(Error::<T>::NoLockedTokens)?;

                let mut i = 0;
                while i < entries.len() {
                    if now >= entries[i].unlock_at {
                        unlocked_total = unlocked_total.saturating_add(entries[i].amount);
                        entries.swap_remove(i);
                    } else {
                        i += 1;
                    }
                }

                ensure!(!unlocked_total.is_zero(), Error::<T>::UnlockTimeNotReached);

                // M4: 清理空存储条目
                if entries.is_empty() {
                    *maybe_entries = None;
                }
                Ok(())
            })?;

            Self::deposit_event(Event::TokensUnlocked {
                entity_id,
                holder: who,
                amount: unlocked_total,
            });
            Ok(())
        }

        /// 变更通证类型（需所有者操作）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::change_token_type())]
        pub fn change_token_type(
            origin: OriginFor<T>,
            entity_id: u64,
            new_type: TokenType,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let old_type = EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> Result<TokenType, DispatchError> {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                let old = config.token_type;
                // H2: 禁止变更为相同类型（防止意外重置自定义转账限制配置）
                ensure!(old != new_type, Error::<T>::SameTokenType);
                config.token_type = new_type;
                
                // 根据新类型更新可转让性
                config.transferable = new_type.is_transferable_by_default();
                // M5: 联动更新转账限制和 KYC 要求
                config.transfer_restriction = new_type.default_transfer_restriction();
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
        #[pallet::weight(T::WeightInfo::set_max_supply())]
        pub fn set_max_supply(
            origin: OriginFor<T>,
            entity_id: u64,
            max_supply: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                
                // M1-R4: 检查当前供应量 + 已承诺分红是否超过新的最大值
                // 不能将 max_supply 降低到无法兑现已承诺分红的水平
                if !max_supply.is_zero() {
                    let current_supply = Self::get_total_supply(entity_id);
                    let pending = TotalPendingDividends::<T>::get(entity_id);
                    ensure!(
                        current_supply.saturating_add(pending) <= max_supply,
                        Error::<T>::ExceedsMaxSupply
                    );
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
        #[pallet::weight(T::WeightInfo::set_transfer_restriction())]
        pub fn set_transfer_restriction(
            origin: OriginFor<T>,
            entity_id: u64,
            mode: TransferRestrictionMode,
            min_receiver_kyc: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

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
        #[pallet::weight(T::WeightInfo::add_to_whitelist(accounts.len() as u32))]
        pub fn add_to_whitelist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let mut added = 0u32;
            for account in accounts.iter() {
                if !TransferWhitelist::<T>::contains_key(entity_id, account) {
                    TransferWhitelist::<T>::insert(entity_id, account, ());
                    added = added.saturating_add(1);
                }
            }

            Self::deposit_event(Event::WhitelistUpdated {
                entity_id,
                added,
                removed: 0,
            });
            Ok(())
        }

        /// 移除白名单地址
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::remove_from_whitelist(accounts.len() as u32))]
        pub fn remove_from_whitelist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let mut removed = 0u32;
            for account in accounts.iter() {
                if TransferWhitelist::<T>::contains_key(entity_id, account) {
                    TransferWhitelist::<T>::remove(entity_id, account);
                    removed = removed.saturating_add(1);
                }
            }

            Self::deposit_event(Event::WhitelistUpdated {
                entity_id,
                added: 0,
                removed,
            });
            Ok(())
        }

        /// 添加黑名单地址
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::add_to_blacklist(accounts.len() as u32))]
        pub fn add_to_blacklist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let mut added = 0u32;
            for account in accounts.iter() {
                if !TransferBlacklist::<T>::contains_key(entity_id, account) {
                    TransferBlacklist::<T>::insert(entity_id, account, ());
                    added = added.saturating_add(1);
                }
            }

            Self::deposit_event(Event::BlacklistUpdated {
                entity_id,
                added,
                removed: 0,
            });
            Ok(())
        }

        /// 移除黑名单地址
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::remove_from_blacklist(accounts.len() as u32))]
        pub fn remove_from_blacklist(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M7: 限制输入列表长度
            ensure!(accounts.len() <= T::MaxTransferListSize::get() as usize, Error::<T>::TransferListFull);

            // P0: 验证调用者是所有者或 TOKEN_MANAGE 管理员
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let mut removed = 0u32;
            for account in accounts.iter() {
                if TransferBlacklist::<T>::contains_key(entity_id, account) {
                    TransferBlacklist::<T>::remove(entity_id, account);
                    removed = removed.saturating_add(1);
                }
            }

            Self::deposit_event(Event::BlacklistUpdated {
                entity_id,
                added: 0,
                removed,
            });
            Ok(())
        }

        // ==================== P1/P2/P3 紧急管控 Extrinsics ====================

        /// P1: 平台强制禁用某实体的代币（Root-only）
        /// 紧急情况下（如代币被用于欺诈）强制设置 enabled=false
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::force_disable_token())]
        pub fn force_disable_token(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                ensure!(config.enabled, Error::<T>::TokenAlreadyDisabled);
                config.enabled = false;
                Ok(())
            })?;

            Self::deposit_event(Event::TokenForceDisabled { entity_id });
            Ok(())
        }

        /// P1: 合规冻结 — 暂停某实体代币的所有转账，但不影响分红领取（Root-only）
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::force_freeze_transfers())]
        pub fn force_freeze_transfers(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // 代币必须存在
            ensure!(EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenNotEnabled);
            // 幂等性检查
            ensure!(!TransfersFrozen::<T>::contains_key(entity_id), Error::<T>::TransfersAlreadyFrozen);

            TransfersFrozen::<T>::insert(entity_id, ());

            Self::deposit_event(Event::TransfersFrozenEvent { entity_id });
            Ok(())
        }

        /// P1: 解除转账冻结（Root-only）
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::force_unfreeze_transfers())]
        pub fn force_unfreeze_transfers(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(TransfersFrozen::<T>::contains_key(entity_id), Error::<T>::TransfersNotFrozen);

            TransfersFrozen::<T>::remove(entity_id);

            Self::deposit_event(Event::TransfersUnfrozen { entity_id });
            Ok(())
        }

        /// P2: 法律合规强制销毁代币（Root-only）
        /// 如法院命令冻结并销毁涉案资产
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::force_burn())]
        pub fn force_burn(
            origin: OriginFor<T>,
            entity_id: u64,
            from: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            // 代币必须存在
            ensure!(EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenNotEnabled);

            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::burn_from(
                asset_id,
                &from,
                amount,
                frame_support::traits::tokens::Preservation::Expendable,
                frame_support::traits::tokens::Precision::Exact,
                frame_support::traits::tokens::Fortitude::Force,
            )?;

            // 清理关联存储：如果余额已为 0，清除锁仓、预留、待领取分红
            let remaining = T::Assets::balance(asset_id, &from);
            if remaining.is_zero() {
                LockedTokens::<T>::remove(entity_id, &from);
                let reserved = ReservedTokens::<T>::take(entity_id, &from);
                let pending = PendingDividends::<T>::take(entity_id, &from);
                // 递减全局待领取分红计数
                if !pending.is_zero() {
                    TotalPendingDividends::<T>::mutate(entity_id, |p| {
                        *p = p.saturating_sub(pending);
                    });
                }
                let _ = reserved; // reserved 已通过 take 清除
            }

            Self::deposit_event(Event::TokensForceBurned {
                entity_id,
                from,
                amount,
            });
            Ok(())
        }

        /// P3: 全平台代币紧急暂停开关（Root-only）
        /// 发现底层 pallet-assets 漏洞时立即暂停所有代币操作
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::set_global_token_pause())]
        pub fn set_global_token_pause(
            origin: OriginFor<T>,
            paused: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            GlobalTokenPaused::<T>::put(paused);

            Self::deposit_event(Event::GlobalTokenPauseSet { paused });
            Ok(())
        }

        // ==================== 新增 Extrinsics ====================

        /// 持有人销毁自己的代币
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::burn_tokens())]
        pub fn burn_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            // M3: 不再要求 Entity 活跃。用户销毁自己的代币属于资产处置权，
            // 与 unlock_tokens/claim_dividend 一样应在 Entity 不活跃时仍可执行。

            // 检查可用余额（扣除锁仓和预留）
            Self::ensure_available_balance(entity_id, &who, amount)?;

            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::burn_from(
                asset_id,
                &who,
                amount,
                frame_support::traits::tokens::Preservation::Preserve,
                frame_support::traits::tokens::Precision::Exact,
                frame_support::traits::tokens::Fortitude::Polite,
            )?;

            Self::deposit_event(Event::TokensBurned {
                entity_id,
                holder: who,
                amount,
            });
            Ok(())
        }

        /// 更新代币元数据（名称、符号）
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::update_token_metadata())]
        pub fn update_token_metadata(
            origin: OriginFor<T>,
            entity_id: u64,
            name: Vec<u8>,
            symbol: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenNotEnabled);

            ensure!(!name.is_empty(), Error::<T>::EmptyName);
            ensure!(!symbol.is_empty(), Error::<T>::EmptySymbol);

            let name_bounded: BoundedVec<u8, T::MaxTokenNameLength> =
                name.clone().try_into().map_err(|_| Error::<T>::NameTooLong)?;
            let symbol_bounded: BoundedVec<u8, T::MaxTokenSymbolLength> =
                symbol.clone().try_into().map_err(|_| Error::<T>::SymbolTooLong)?;

            // 获取现有 decimals
            let decimals = EntityTokenMetadata::<T>::get(entity_id)
                .map(|(_, _, d)| d)
                .unwrap_or(18);

            // H3: 使用 Entity Owner 调用 Assets::set，因为 pallet-assets 要求资产管理员
            // （即 create_shop_token 时的调用者）。当 TOKEN_MANAGE 管理员调用时，
            // who 不是资产管理员，直接传 who 会导致 Assets::set 失败。
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::set(asset_id, &owner, name.clone(), symbol.clone(), decimals)
                .map_err(|_| Error::<T>::AssetCreationFailed)?;

            // 更新本模块存储
            EntityTokenMetadata::<T>::insert(entity_id, (name_bounded, symbol_bounded, decimals));

            Self::deposit_event(Event::TokenMetadataUpdated {
                entity_id,
                name,
                symbol,
            });
            Ok(())
        }

        /// 合规强制转账（Root-only）
        /// 绕过所有转账限制，用于法律/合规要求
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::force_transfer())]
        pub fn force_transfer(
            origin: OriginFor<T>,
            entity_id: u64,
            from: T::AccountId,
            to: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            ensure!(EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenNotEnabled);

            let asset_id = Self::entity_to_asset_id(entity_id);
            // M1: Root 合规操作使用 Expendable，允许完全转出（如法院扣押全部资产）
            T::Assets::transfer(
                asset_id,
                &from,
                &to,
                amount,
                frame_support::traits::tokens::Preservation::Expendable,
            )?;

            // M1: 清理关联存储：如果剩余余额为 0，清除锁仓、预留、待领取分红
            // force_transfer 绕过锁仓/预留检查，可能导致记账不一致
            let remaining = T::Assets::balance(asset_id, &from);
            if remaining.is_zero() {
                LockedTokens::<T>::remove(entity_id, &from);
                let pending = PendingDividends::<T>::take(entity_id, &from);
                ReservedTokens::<T>::remove(entity_id, &from);
                if !pending.is_zero() {
                    TotalPendingDividends::<T>::mutate(entity_id, |p| {
                        *p = p.saturating_sub(pending);
                    });
                }
            }

            Self::deposit_event(Event::TokensForceTransferred {
                entity_id,
                from,
                to,
                amount,
            });
            Ok(())
        }

        /// 平台强制重新启用代币（Root-only）
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::force_enable_token())]
        pub fn force_enable_token(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
                ensure!(!config.enabled, Error::<T>::TokenAlreadyEnabled);
                config.enabled = true;
                Ok(())
            })?;

            Self::deposit_event(Event::TokenForceEnabled { entity_id });
            Ok(())
        }

        // ==================== R4 审计新增 Extrinsics ====================

        /// 强制取消待领取分红（Root-only）
        /// 用于取消欺诈性分红分发
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::force_cancel_pending_dividends(accounts.len() as u32))]
        pub fn force_cancel_pending_dividends(
            origin: OriginFor<T>,
            entity_id: u64,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenNotEnabled);

            let mut total_cancelled = T::AssetBalance::zero();
            let mut count = 0u32;

            for account in accounts.iter() {
                let pending = PendingDividends::<T>::take(entity_id, account);
                if !pending.is_zero() {
                    total_cancelled = total_cancelled.saturating_add(pending);
                    count = count.saturating_add(1);
                }
            }

            ensure!(!total_cancelled.is_zero(), Error::<T>::NoPendingDividendsToCancel);

            TotalPendingDividends::<T>::mutate(entity_id, |p| {
                *p = p.saturating_sub(total_cancelled);
            });

            Self::deposit_event(Event::PendingDividendsCancelled {
                entity_id,
                total_cancelled,
                accounts_affected: count,
            });
            Ok(())
        }

        /// 授权第三方使用代币额度
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::approve_tokens())]
        pub fn approve_tokens(
            origin: OriginFor<T>,
            entity_id: u64,
            spender: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(EntityTokenConfigs::<T>::contains_key(entity_id), Error::<T>::TokenNotEnabled);
            ensure!(who != spender, Error::<T>::SelfTransfer);

            TokenApprovals::<T>::insert((entity_id, &who, &spender), amount);

            Self::deposit_event(Event::TokenApprovalSet {
                entity_id,
                owner: who,
                spender,
                amount,
            });
            Ok(())
        }

        /// 授权转账：spender 将 owner 代币转给 to
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::transfer_from())]
        pub fn transfer_from(
            origin: OriginFor<T>,
            entity_id: u64,
            owner: T::AccountId,
            to: T::AccountId,
            amount: T::AssetBalance,
        ) -> DispatchResult {
            let spender = ensure_signed(origin)?;

            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);
            ensure!(!TransfersFrozen::<T>::contains_key(entity_id), Error::<T>::TokenTransfersFrozen);

            let config = EntityTokenConfigs::<T>::get(entity_id).ok_or(Error::<T>::TokenNotEnabled)?;
            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            ensure!(config.transferable, Error::<T>::TransferNotAllowed);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            ensure!(owner != to, Error::<T>::SelfTransfer);

            // 检查并扣减授权额度
            TokenApprovals::<T>::try_mutate((entity_id, &owner, &spender), |allowance| -> DispatchResult {
                ensure!(*allowance >= amount, Error::<T>::InsufficientAllowance);
                *allowance = allowance.saturating_sub(amount);
                Ok(())
            })?;

            // 转账限制检查
            Self::check_transfer_restriction(entity_id, &config, &owner, &to)?;

            // 内幕交易检查（检查 owner，因为资产从 owner 账户流出）
            ensure!(
                T::DisclosureProvider::can_insider_trade(entity_id, &owner),
                Error::<T>::InsiderTradingRestricted
            );
            // D-1: 披露处罚限制
            ensure!(
                !T::DisclosureProvider::is_penalty_active(entity_id),
                Error::<T>::DisclosurePenaltyRestricted
            );

            // 检查可用余额
            Self::ensure_available_balance(entity_id, &owner, amount)?;

            let asset_id = Self::entity_to_asset_id(entity_id);
            T::Assets::transfer(asset_id, &owner, &to, amount, frame_support::traits::tokens::Preservation::Preserve)?;

            Self::deposit_event(Event::TokensTransferredFrom {
                entity_id,
                owner,
                spender,
                to,
                amount,
            });
            Ok(())
        }
    }

    // ==================== 完整性检查 ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(
                T::ShopTokenOffset::get() > 0,
                "ShopTokenOffset must be > 0 to avoid asset ID collision"
            );
            assert!(
                T::MaxTokenNameLength::get() >= 1,
                "MaxTokenNameLength must be >= 1"
            );
            assert!(
                T::MaxTokenSymbolLength::get() >= 1,
                "MaxTokenSymbolLength must be >= 1"
            );
            assert!(
                T::MaxTransferListSize::get() >= 1,
                "MaxTransferListSize must be >= 1"
            );
            assert!(
                T::MaxDividendRecipients::get() >= 1,
                "MaxDividendRecipients must be >= 1"
            );
        }
    }

    // ==================== 内部函数 ====================

    impl<T: Config> Pallet<T> {
        /// P0: 确保调用者是 Entity Owner 或拥有 TOKEN_MANAGE 权限的管理员
        fn ensure_owner_or_admin(who: &T::AccountId, entity_id: u64) -> DispatchResult {
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                *who == owner || T::EntityProvider::is_entity_admin(
                    entity_id, who, AdminPermission::TOKEN_MANAGE
                ),
                Error::<T>::NotAuthorized
            );
            Ok(())
        }

        /// Entity ID 转资产 ID
        pub fn entity_to_asset_id(entity_id: u64) -> T::AssetId {
            (T::ShopTokenOffset::get() + entity_id).into()
        }

        /// T-M1: 计算用户所有未过期锁仓总额
        pub fn total_locked_amount(entity_id: u64, who: &T::AccountId) -> T::AssetBalance {
            let now = <frame_system::Pallet<T>>::block_number();
            LockedTokens::<T>::get(entity_id, who)
                .iter()
                .filter(|e| now < e.unlock_at)
                .fold(T::AssetBalance::zero(), |acc, e| acc.saturating_add(e.amount))
        }

        /// R4: 统一可用余额检查（balance - locked - reserved >= required）
        pub(crate) fn ensure_available_balance(entity_id: u64, who: &T::AccountId, required: T::AssetBalance) -> DispatchResult {
            let asset_id = Self::entity_to_asset_id(entity_id);
            let balance = T::Assets::balance(asset_id, who);
            let locked = Self::total_locked_amount(entity_id, who);
            let reserved = ReservedTokens::<T>::get(entity_id, who);
            let available = balance.saturating_sub(locked).saturating_sub(reserved);
            ensure!(available >= required, Error::<T>::InsufficientBalance);
            Ok(())
        }

        /// Phase 8: 检查转账限制（双向：发送方 + 接收方）
        pub(crate) fn check_transfer_restriction(
            entity_id: u64,
            config: &EntityTokenConfigOf<T>,
            from: &T::AccountId,
            to: &T::AccountId,
        ) -> DispatchResult {
            match config.transfer_restriction {
                TransferRestrictionMode::None => Ok(()),
                
                TransferRestrictionMode::Whitelist => {
                    ensure!(TransferWhitelist::<T>::contains_key(entity_id, from), Error::<T>::SenderNotInWhitelist);
                    ensure!(TransferWhitelist::<T>::contains_key(entity_id, to), Error::<T>::ReceiverNotInWhitelist);
                    Ok(())
                }
                
                TransferRestrictionMode::Blacklist => {
                    ensure!(!TransferBlacklist::<T>::contains_key(entity_id, from), Error::<T>::SenderInBlacklist);
                    ensure!(!TransferBlacklist::<T>::contains_key(entity_id, to), Error::<T>::ReceiverInBlacklist);
                    Ok(())
                }
                
                TransferRestrictionMode::KycRequired => {
                    ensure!(
                        T::KycProvider::meets_kyc_requirement(entity_id, from, config.min_receiver_kyc),
                        Error::<T>::SenderKycInsufficient
                    );
                    ensure!(
                        T::KycProvider::meets_kyc_requirement(entity_id, to, config.min_receiver_kyc),
                        Error::<T>::ReceiverKycInsufficient
                    );
                    Ok(())
                }
                
                TransferRestrictionMode::MembersOnly => {
                    ensure!(
                        T::MemberProvider::is_member(entity_id, from),
                        Error::<T>::SenderNotMember
                    );
                    ensure!(
                        T::MemberProvider::is_member(entity_id, to),
                        Error::<T>::ReceiverNotMember
                    );
                    Ok(())
                }
            }
        }

        /// 检查铸造是否在 max_supply 范围内
        /// M1-R6: 必须包含 TotalPendingDividends，否则 mint 会侵占已承诺分红的容量
        fn ensure_within_max_supply(
            entity_id: u64,
            config: &EntityTokenConfigOf<T>,
            mint_amount: T::AssetBalance,
        ) -> DispatchResult {
            if !config.max_supply.is_zero() {
                let current_supply = Self::get_total_supply(entity_id);
                let pending = TotalPendingDividends::<T>::get(entity_id);
                ensure!(
                    current_supply.saturating_add(pending).saturating_add(mint_amount) <= config.max_supply,
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
            // P3: 全平台暂停时静默跳过奖励
            if GlobalTokenPaused::<T>::get() {
                return Ok(Zero::zero());
            }

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

            // H2 + M1-R6: 检查 max_supply（含已承诺分红），超过上限时静默跳过
            if !config.max_supply.is_zero() {
                let current_supply = Self::get_total_supply(entity_id);
                let pending = TotalPendingDividends::<T>::get(entity_id);
                if current_supply.saturating_add(pending).saturating_add(reward) > config.max_supply {
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
            // P3: 全平台暂停时拒绝兑换
            ensure!(!GlobalTokenPaused::<T>::get(), Error::<T>::GlobalPaused);

            let config = EntityTokenConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::TokenNotEnabled)?;

            ensure!(config.enabled, Error::<T>::TokenNotEnabled);
            ensure!(tokens_to_use >= config.min_redeem, Error::<T>::BelowMinRedeem);
            ensure!(
                config.max_redeem_per_order.is_zero() || tokens_to_use <= config.max_redeem_per_order,
                Error::<T>::ExceedsMaxRedeem
            );

            // T-H1 审计修复: 检查可用余额（扣除锁仓和预留）
            Self::ensure_available_balance(entity_id, buyer, tokens_to_use)?;

            // 计算折扣：tokens * exchange_rate / 10000
            let discount = tokens_to_use
                .saturating_mul(config.exchange_rate.into())
                / 10000u32.into();

            // 销毁积分
            let asset_id = Self::entity_to_asset_id(entity_id);
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

    /// 查询用户在某实体的完整代币状态
    /// 返回 (balance, locked, reserved, pending_dividends, available)
    pub fn get_account_token_info(
        entity_id: u64,
        holder: &T::AccountId,
    ) -> (T::AssetBalance, T::AssetBalance, T::AssetBalance, T::AssetBalance, T::AssetBalance) {
        let balance = Self::token_balance(entity_id, holder);
        let locked = Self::total_locked_amount(entity_id, holder);
        let reserved = pallet::ReservedTokens::<T>::get(entity_id, holder);
        let pending = pallet::PendingDividends::<T>::get(entity_id, holder);
        let available = balance.saturating_sub(locked).saturating_sub(reserved);
        (balance, locked, reserved, pending, available)
    }

    /// 查询用户在某实体的锁仓条目列表
    pub fn get_lock_entries(
        entity_id: u64,
        holder: &T::AccountId,
    ) -> alloc::vec::Vec<pallet::LockEntry<T::AssetBalance, frame_system::pallet_prelude::BlockNumberFor<T>>> {
        pallet::LockedTokens::<T>::get(entity_id, holder).into_inner()
    }

    /// 获取可用余额（总余额 - 锁仓 - 预留）
    pub fn get_available_balance(entity_id: u64, holder: &T::AccountId) -> T::AssetBalance {
        let balance = Self::token_balance(entity_id, holder);
        let locked = Self::total_locked_amount(entity_id, holder);
        let reserved = pallet::ReservedTokens::<T>::get(entity_id, holder);
        balance.saturating_sub(locked).saturating_sub(reserved)
    }

    /// 检查账户是否在白名单中
    pub fn is_whitelisted(entity_id: u64, account: &T::AccountId) -> bool {
        pallet::TransferWhitelist::<T>::contains_key(entity_id, account)
    }

    /// 检查账户是否在黑名单中
    pub fn is_blacklisted(entity_id: u64, account: &T::AccountId) -> bool {
        pallet::TransferBlacklist::<T>::contains_key(entity_id, account)
    }

    /// 获取授权额度
    pub fn get_allowance(entity_id: u64, owner: &T::AccountId, spender: &T::AccountId) -> T::AssetBalance {
        pallet::TokenApprovals::<T>::get((entity_id, owner, spender))
    }
}

// ==================== EntityTokenProvider 实现 ====================

use pallet_entity_common::{EntityProvider, EntityTokenProvider, TokenType};
use sp_runtime::traits::{Zero as _Zero, Saturating as _Saturating};

impl<T: Config> EntityTokenProvider<T::AccountId, T::AssetBalance> for Pallet<T> {
    fn is_token_enabled(entity_id: u64) -> bool {
        Pallet::<T>::is_token_enabled(entity_id)
    }

    fn token_balance(entity_id: u64, holder: &T::AccountId) -> T::AssetBalance {
        Pallet::<T>::token_balance(entity_id, holder)
    }

    fn available_balance(entity_id: u64, holder: &T::AccountId) -> T::AssetBalance {
        Pallet::<T>::get_available_balance(entity_id, holder)
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

        // L2-R3: 与 transfer_tokens extrinsic 一致，拒绝零数量转账
        if amount.is_zero() {
            return Err(sp_runtime::DispatchError::Other("ZeroAmount"));
        }

        // 检查全局暂停、实体冻结、代币状态
        if pallet::GlobalTokenPaused::<T>::get() {
            return Err(sp_runtime::DispatchError::Other("GlobalPaused"));
        }
        if pallet::TransfersFrozen::<T>::contains_key(entity_id) {
            return Err(sp_runtime::DispatchError::Other("TokenTransfersFrozen"));
        }
        // M2: 与 transfer_tokens extrinsic 保持一致，检查 Entity 是否活跃
        if !T::EntityProvider::is_entity_active(entity_id) {
            return Err(sp_runtime::DispatchError::Other("EntityNotActive"));
        }
        if let Some(config) = pallet::EntityTokenConfigs::<T>::get(entity_id) {
            if !config.enabled {
                return Err(sp_runtime::DispatchError::Other("TokenNotEnabled"));
            }
            if !config.transferable {
                return Err(sp_runtime::DispatchError::Other("TransferNotAllowed"));
            }
            // 双向转账限制检查
            Pallet::<T>::check_transfer_restriction(entity_id, &config, from, to)?;
        }

        // 检查可用余额
        Pallet::<T>::ensure_available_balance(entity_id, from, amount)
            .map_err(|_| sp_runtime::DispatchError::Other("InsufficientBalance"))?;

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
        // L3-R3: 拒绝零数量预留
        if amount.is_zero() {
            return Err(sp_runtime::DispatchError::Other("ZeroAmount"));
        }
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
        use frame_support::traits::fungibles::Mutate;
        let reserved = pallet::ReservedTokens::<T>::get(entity_id, from);
        let actual = amount.min(reserved);
        if actual.is_zero() {
            return Ok(actual);
        }
        // H1-R2: 直接调用 Assets::transfer，绕过所有策略检查。
        // 预留代币是已承诺资金（如佣金托管、订单押金），释放时不应受
        // GlobalPaused / TransfersFrozen / EntityNotActive / 转账限制 的阻拦。
        // 旧代码调用 Self::transfer()（含全部策略检查 + 循环可用余额扣减），
        // 导致 Entity 不活跃或冻结时，佣金退款和订单结算永久卡死。
        let asset_id = Pallet::<T>::entity_to_asset_id(entity_id);
        // Expendable: 预留可能等于全部余额，释放时必须允许账户清零
        T::Assets::transfer(
            asset_id,
            from,
            to,
            actual,
            frame_support::traits::tokens::Preservation::Expendable,
        )?;
        pallet::ReservedTokens::<T>::mutate(entity_id, from, |r| {
            *r = r.saturating_sub(actual);
        });
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

    fn governance_burn(entity_id: u64, amount: T::AssetBalance) -> Result<(), sp_runtime::DispatchError> {
        use frame_support::traits::fungibles::Mutate;
        if amount.is_zero() {
            return Err(sp_runtime::DispatchError::Other("ZeroAmount"));
        }
        // L1: 仅验证代币存在，无需绑定 config
        if !pallet::EntityTokenConfigs::<T>::contains_key(entity_id) {
            return Err(sp_runtime::DispatchError::Other("TokenNotEnabled"));
        }
        let entity_account = T::EntityProvider::entity_account(entity_id);
        let asset_id = Pallet::<T>::entity_to_asset_id(entity_id);
        T::Assets::burn_from(
            asset_id,
            &entity_account,
            amount,
            frame_support::traits::tokens::Preservation::Preserve,
            frame_support::traits::tokens::Precision::Exact,
            frame_support::traits::tokens::Fortitude::Polite,
        )?;
        Pallet::<T>::deposit_event(pallet::Event::TokensGovernanceBurned {
            entity_id,
            from: entity_account,
            amount,
        });
        Ok(())
    }

    fn governance_set_max_supply(entity_id: u64, new_max_supply: T::AssetBalance) -> Result<(), sp_runtime::DispatchError> {
        EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> Result<(), sp_runtime::DispatchError> {
            let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
            if !new_max_supply.is_zero() {
                let current_supply = Self::get_total_supply(entity_id);
                let pending = TotalPendingDividends::<T>::get(entity_id);
                frame_support::ensure!(
                    current_supply.saturating_add(pending) <= new_max_supply,
                    Error::<T>::ExceedsMaxSupply
                );
            }
            config.max_supply = new_max_supply;
            Ok(())
        })?;
        Self::deposit_event(pallet::Event::TokenConfigUpdated { entity_id });
        Ok(())
    }

    fn governance_set_token_type(entity_id: u64, new_type: TokenType) -> Result<(), sp_runtime::DispatchError> {
        let old_type = EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> Result<TokenType, sp_runtime::DispatchError> {
            let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
            let old = config.token_type;
            frame_support::ensure!(old != new_type, Error::<T>::SameTokenType);
            config.token_type = new_type;
            config.transferable = new_type.is_transferable_by_default();
            config.transfer_restriction = new_type.default_transfer_restriction();
            config.min_receiver_kyc = new_type.required_kyc_level().1;
            Ok(old)
        })?;
        Self::deposit_event(pallet::Event::TokenTypeChanged {
            entity_id,
            old_type,
            new_type,
        });
        Ok(())
    }

    fn governance_set_transfer_restriction(entity_id: u64, restriction: u8, min_receiver_kyc: u8) -> Result<(), sp_runtime::DispatchError> {
        use pallet_entity_common::TransferRestrictionMode;
        let mode = TransferRestrictionMode::try_from_u8(restriction)
            .ok_or(sp_runtime::DispatchError::Other("InvalidRestrictionMode"))?;
        let clamped_kyc = min_receiver_kyc.min(4);
        EntityTokenConfigs::<T>::try_mutate(entity_id, |maybe_config| -> Result<(), sp_runtime::DispatchError> {
            let config = maybe_config.as_mut().ok_or(Error::<T>::TokenNotEnabled)?;
            config.transfer_restriction = mode;
            config.min_receiver_kyc = clamped_kyc;
            Ok(())
        })?;
        Self::deposit_event(pallet::Event::TransferRestrictionSet {
            entity_id,
            mode,
            min_receiver_kyc: clamped_kyc,
        });
        Ok(())
    }
}

impl<T: Config> pallet_entity_common::TokenGovernancePort<T::AccountId> for Pallet<T> {
    fn governance_manage_blacklist(
        entity_id: u64,
        _account_cid: &[u8],
        _add: bool,
    ) -> Result<(), sp_runtime::DispatchError> {
        // account_cid 是 IPFS CID，指向链下的账户列表数据
        // 链上无法解析 CID 为具体 AccountId，此操作需 off-chain 执行
        // 治理提案通过即表示 DAO 批准了黑名单变更，实际执行由管理员完成
        frame_support::ensure!(
            pallet::EntityTokenConfigs::<T>::contains_key(entity_id),
            sp_runtime::DispatchError::Other("TokenNotEnabled")
        );
        Ok(())
    }
}
