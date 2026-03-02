//! # 实体代币发售模块 (pallet-entity-tokensale)
//!
//! ## 概述
//!
//! 本模块实现实体代币公开发售功能：
//! - 多种发售模式（固定价格、荷兰拍卖、白名单分配）
//! - 多轮发售支持
//! - 代币锁仓和线性解锁
//! - 多资产支付支持（当前仅支持原生 NEX）
//! - KYC 集成
//! - 实际资金转账（认购扣款、代币发放、退款）
//!
//! ## 资金流
//!
//! - subscribe: 认购者 NEX → Pallet 托管账户
//! - start_sale: Entity 代币 reserve (锁定)
//! - claim_tokens / unlock_tokens: Entity 代币 → 认购者
//! - end_sale: 释放未售代币
//! - cancel_sale + claim_refund: NEX 退还认购者 + 释放代币
//! - withdraw_funds: NEX → Entity 派生账户
//!
//! ## 版本历史
//!
//! - v0.1.0 (2026-02-03): Phase 8 初始版本
//! - v0.2.0 (2026-02-09): 深度审计修复：EntityProvider/Currency/TokenProvider/KYC 集成，
//!   白名单独立存储，输入校验，实际转账，新增 claim_refund/withdraw_funds

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// KYC 检查接口
pub trait KycChecker<AccountId> {
    /// 获取账户 KYC 级别 (0=None, 1=Basic, 2=Enhanced, 3=Premium, 4=Institutional)
    fn kyc_level(account: &AccountId) -> u8;
}

/// 空 KYC 检查器（不检查 KYC，所有人返回 0）
pub struct NullKycChecker;
impl<AccountId> KycChecker<AccountId> for NullKycChecker {
    fn kyc_level(_: &AccountId) -> u8 { 0 }
}

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
    use pallet_entity_common::{EntityProvider, EntityTokenProvider};
    use sp_runtime::{
        traits::{AccountIdConversion, Saturating, Zero},
        SaturatedConversion,
    };

    /// 发售托管 PalletId
    const SALE_PALLET_ID: PalletId = PalletId(*b"et/sale/");

    /// NEX 余额类型别名（从 Currency 派生）
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
    /// 资产 ID 类型别名
    pub type AssetIdOf<T> = <T as Config>::AssetId;

    // ==================== 类型定义 ====================

    /// 发售模式
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum SaleMode {
        /// 固定价格
        #[default]
        FixedPrice,
        /// 荷兰拍卖（价格递减）
        DutchAuction,
        /// 白名单分配
        WhitelistAllocation,
        /// 先到先得
        FCFS,
        /// 抽签发售
        Lottery,
    }

    /// 发售轮次状态
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum RoundStatus {
        /// 未开始
        #[default]
        NotStarted,
        /// 白名单注册中
        WhitelistOpen,
        /// 发售进行中
        Active,
        /// 已售罄
        SoldOut,
        /// 已结束
        Ended,
        /// 已取消
        Cancelled,
        /// 结算中
        Settling,
        /// 已完成
        Completed,
    }

    /// 锁仓类型
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum VestingType {
        /// 无锁仓
        #[default]
        None,
        /// 线性解锁
        Linear,
        /// 阶梯解锁
        Cliff,
        /// 自定义解锁
        Custom,
    }

    /// 锁仓配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct VestingConfig<BlockNumber> {
        /// 锁仓类型
        pub vesting_type: VestingType,
        /// 初始解锁比例（基点，如 1000 = 10%）
        pub initial_unlock_bps: u16,
        /// 悬崖期（区块数）
        pub cliff_duration: BlockNumber,
        /// 总解锁期（区块数）
        pub total_duration: BlockNumber,
        /// 解锁间隔（区块数，用于阶梯解锁）
        pub unlock_interval: BlockNumber,
    }

    /// 支付配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct PaymentConfig<AssetId, Balance> {
        /// 支付资产 ID（None = 原生代币 NEX）
        pub asset_id: Option<AssetId>,
        /// 单价（NEX 计价）
        pub price: Balance,
        /// 最小购买量
        pub min_purchase: Balance,
        /// 最大购买量（每人）
        pub max_purchase_per_account: Balance,
        /// 是否启用
        pub enabled: bool,
    }

    /// 发售轮次（支付选项和白名单已拆分为独立存储）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct SaleRound<AccountId, Balance, BlockNumber> {
        /// 轮次 ID
        pub id: u64,
        /// 实体 ID
        pub entity_id: u64,
        /// 发售模式
        pub mode: SaleMode,
        /// 状态
        pub status: RoundStatus,
        /// 代币总量
        pub total_supply: Balance,
        /// 已售数量
        pub sold_amount: Balance,
        /// 剩余数量
        pub remaining_amount: Balance,
        /// 参与人数
        pub participants_count: u32,
        /// 支付选项数（实际数据在 RoundPaymentOptions 独立存储）
        pub payment_options_count: u32,
        /// 锁仓配置
        pub vesting_config: VestingConfig<BlockNumber>,
        /// 是否需要 KYC
        pub kyc_required: bool,
        /// 最低 KYC 级别（0-4）
        pub min_kyc_level: u8,
        /// 开始时间
        pub start_block: BlockNumber,
        /// 结束时间
        pub end_block: BlockNumber,
        /// 荷兰拍卖起始价格
        pub dutch_start_price: Option<Balance>,
        /// 荷兰拍卖结束价格
        pub dutch_end_price: Option<Balance>,
        /// 创建者
        pub creator: AccountId,
        /// 创建时间
        pub created_at: BlockNumber,
        /// 募集资金是否已提取
        pub funds_withdrawn: bool,
        /// 取消时间（用于退款宽限期计算）
        pub cancelled_at: Option<BlockNumber>,
        /// 累计已退回的 Entity 代币数
        pub total_refunded_tokens: Balance,
        /// 累计已退回的 NEX 数
        pub total_refunded_nex: Balance,
    }

    /// 发售轮次类型别名
    pub type SaleRoundOf<T> = SaleRound<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
    >;

    /// 认购记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct Subscription<AccountId, Balance, BlockNumber, AssetId> {
        /// 认购者
        pub subscriber: AccountId,
        /// 轮次 ID
        pub round_id: u64,
        /// 认购数量（Entity 代币）
        pub amount: Balance,
        /// 支付资产
        pub payment_asset: Option<AssetId>,
        /// 支付金额（NEX）
        pub payment_amount: Balance,
        /// 认购时间
        pub subscribed_at: BlockNumber,
        /// 是否已领取初始解锁
        pub claimed: bool,
        /// 已解锁数量
        pub unlocked_amount: Balance,
        /// 上次解锁时间
        pub last_unlock_at: BlockNumber,
        /// 是否已退款（取消发售后）
        pub refunded: bool,
    }

    /// 认购记录类型别名
    pub type SubscriptionOf<T> = Subscription<
        <T as frame_system::Config>::AccountId,
        BalanceOf<T>,
        BlockNumberFor<T>,
        AssetIdOf<T>,
    >;

    // ==================== 配置 ====================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// NEX 货币类型（用于认购支付和退款）
        type Currency: Currency<Self::AccountId>;

        /// 资产 ID 类型（多资产支付预留）
        type AssetId: Member
            + Parameter
            + Default
            + Copy
            + MaxEncodedLen
            + From<u64>
            + Into<u64>;

        /// Entity 查询接口（验证存在性、权限）
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Entity 代币接口（锁定、转移代币）
        type TokenProvider: EntityTokenProvider<Self::AccountId, BalanceOf<Self>>;

        /// KYC 检查接口
        type KycChecker: KycChecker<Self::AccountId>;

        /// 最大支付选项数
        #[pallet::constant]
        type MaxPaymentOptions: Get<u32>;

        /// 最大白名单大小（每轮次）
        #[pallet::constant]
        type MaxWhitelistSize: Get<u32>;

        /// 最大历史轮次数（每 Entity）
        #[pallet::constant]
        type MaxRoundsHistory: Get<u32>;

        /// 最大认购人数（每轮次）
        #[pallet::constant]
        type MaxSubscriptionsPerRound: Get<u32>;

        /// 最大同时活跃轮次数（on_initialize 扫描上限）
        #[pallet::constant]
        type MaxActiveRounds: Get<u32>;

        /// 退款宽限期（取消后多少区块内可退款，之后创建者可回收未领代币）
        #[pallet::constant]
        type RefundGracePeriod: Get<BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ==================== 存储项 ====================

    /// 下一个轮次 ID
    #[pallet::storage]
    #[pallet::getter(fn next_round_id)]
    pub type NextRoundId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 发售轮次存储
    #[pallet::storage]
    #[pallet::getter(fn sale_rounds)]
    pub type SaleRounds<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        SaleRoundOf<T>,
    >;

    /// 实体发售轮次索引
    #[pallet::storage]
    #[pallet::getter(fn entity_rounds)]
    pub type EntityRounds<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // entity_id
        BoundedVec<u64, T::MaxRoundsHistory>,
        ValueQuery,
    >;

    /// 认购记录 (round_id, subscriber) -> Subscription
    #[pallet::storage]
    #[pallet::getter(fn subscriptions)]
    pub type Subscriptions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        Blake2_128Concat,
        T::AccountId,
        SubscriptionOf<T>,
    >;

    /// 轮次参与者列表
    #[pallet::storage]
    #[pallet::getter(fn round_participants)]
    pub type RoundParticipants<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        BoundedVec<T::AccountId, T::MaxSubscriptionsPerRound>,
        ValueQuery,
    >;

    /// 已募集资金 (round_id, asset_id) -> amount
    #[pallet::storage]
    #[pallet::getter(fn raised_funds)]
    pub type RaisedFunds<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        Blake2_128Concat,
        Option<AssetIdOf<T>>,  // None = native NEX
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 支付选项存储（从 SaleRound 拆出，减少频繁读写开销）
    #[pallet::storage]
    pub type RoundPaymentOptions<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        BoundedVec<PaymentConfig<AssetIdOf<T>, BalanceOf<T>>, T::MaxPaymentOptions>,
        ValueQuery,
    >;

    /// 当前活跃轮次 ID 列表（on_initialize 自动结束扫描用）
    #[pallet::storage]
    pub type ActiveRounds<T: Config> = StorageValue<
        _,
        BoundedVec<u64, T::MaxActiveRounds>,
        ValueQuery,
    >;

    /// 白名单存储（独立于 SaleRound，避免大 struct 读取）
    #[pallet::storage]
    pub type RoundWhitelist<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        Blake2_128Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;

    /// 白名单计数
    #[pallet::storage]
    pub type WhitelistCount<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // round_id
        u32,
        ValueQuery,
    >;

    // ==================== 事件 ====================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 发售轮次已创建
        SaleRoundCreated {
            round_id: u64,
            entity_id: u64,
            mode: SaleMode,
            total_supply: BalanceOf<T>,
        },
        /// 支付选项已添加
        PaymentOptionAdded {
            round_id: u64,
            asset_id: Option<AssetIdOf<T>>,
        },
        /// 锁仓配置已设置
        VestingConfigSet { round_id: u64 },
        /// 荷兰拍卖已配置
        DutchAuctionConfigured { round_id: u64 },
        /// 发售轮次已开始（Entity 代币已锁定）
        SaleRoundStarted {
            round_id: u64,
        },
        /// 发售轮次已结束
        SaleRoundEnded {
            round_id: u64,
            sold_amount: BalanceOf<T>,
            participants_count: u32,
        },
        /// 发售轮次已取消
        SaleRoundCancelled {
            round_id: u64,
        },
        /// 用户已认购（NEX 已转入托管）
        Subscribed {
            round_id: u64,
            subscriber: T::AccountId,
            amount: BalanceOf<T>,
            payment_amount: BalanceOf<T>,
        },
        /// 代币已领取（初始解锁，Entity 代币已转给用户）
        TokensClaimed {
            round_id: u64,
            subscriber: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 代币已解锁（Entity 代币已转给用户）
        TokensUnlocked {
            round_id: u64,
            subscriber: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 白名单已更新
        WhitelistUpdated {
            round_id: u64,
            count: u32,
        },
        /// 募集资金已提取（NEX → Entity 账户）
        FundsWithdrawn {
            round_id: u64,
            recipient: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 退款已领取（NEX → 认购者）
        RefundClaimed {
            round_id: u64,
            subscriber: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// 发售自动结束（on_initialize 触发）
        SaleAutoEnded {
            round_id: u64,
            sold_amount: BalanceOf<T>,
            participants_count: u32,
        },
        /// 过期未领退款已回收（创建者触发）
        ExpiredRefundsReclaimed {
            round_id: u64,
            tokens_reclaimed: BalanceOf<T>,
            nex_reclaimed: BalanceOf<T>,
        },
    }

    // ==================== 错误 ====================

    #[pallet::error]
    pub enum Error<T> {
        /// 轮次不存在
        RoundNotFound,
        /// 轮次未开始
        RoundNotStarted,
        /// 轮次已结束
        RoundEnded,
        /// 轮次已取消
        RoundCancelled,
        /// 轮次已售罄
        SoldOut,
        /// 无效的轮次状态
        InvalidRoundStatus,
        /// 余额不足
        InsufficientBalance,
        /// 超过购买限额
        ExceedsPurchaseLimit,
        /// 低于最小购买量
        BelowMinPurchase,
        /// 不在白名单中
        NotInWhitelist,
        /// KYC 级别不足
        InsufficientKycLevel,
        /// 无效的支付资产（当前仅支持原生 NEX）
        InvalidPaymentAsset,
        /// 已认购
        AlreadySubscribed,
        /// 未认购
        NotSubscribed,
        /// 代币已领取
        AlreadyClaimed,
        /// 无可解锁代币
        NoTokensToUnlock,
        /// 悬崖期未到
        CliffNotReached,
        /// 无权限（非 Entity owner/admin）
        Unauthorized,
        /// 白名单已满
        WhitelistFull,
        /// 轮次历史已满
        RoundsHistoryFull,
        /// 参与者已满
        ParticipantsFull,
        /// 支付选项已满
        PaymentOptionsFull,
        /// 荷兰拍卖配置无效
        InvalidDutchAuctionConfig,
        /// 锁仓配置无效
        InvalidVestingConfig,
        /// Entity 不存在
        EntityNotFound,
        /// Entity 未激活
        EntityNotActive,
        /// 总供应量无效（必须 > 0）
        InvalidTotalSupply,
        /// 时间窗口无效（end_block 须 > start_block）
        InvalidTimeWindow,
        /// 商品价格无效（必须 > 0）
        InvalidPrice,
        /// 锁仓期配置无效（total_duration 须 >= cliff_duration）
        InvalidVestingDuration,
        /// 发售不在时间窗口内
        SaleNotInTimeWindow,
        /// 算术溢出
        ArithmeticOverflow,
        /// 无支付选项（启动前需至少添加一个）
        NoPaymentOptions,
        /// KYC 级别超出范围（0-4）
        InvalidKycLevel,
        /// Entity 代币余额不足
        InsufficientTokenSupply,
        /// 资金已提取
        FundsAlreadyWithdrawn,
        /// 发售未取消（退款需要取消状态）
        SaleNotCancelled,
        /// 已退款
        AlreadyRefunded,
        /// 购买量限额配置无效（max 须 >= min）
        InvalidPurchaseLimits,
        /// 退款宽限期未到期
        RefundPeriodNotExpired,
        /// 荷兰拍卖未配置价格曲线
        DutchAuctionNotConfigured,
        /// 活跃轮次已满
        ActiveRoundsFull,
        /// 轮次 ID 溢出
        RoundIdOverflow,
        /// 开始时间不能在过去
        StartBlockInPast,
        /// Entity 代币 unreserve 不完整
        IncompleteUnreserve,
    }

    // ==================== Hooks ====================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 每个区块自动检查并结束过期发售
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut weight = T::DbWeight::get().reads(1); // read ActiveRounds
            let active = ActiveRounds::<T>::get();
            if active.is_empty() {
                return weight;
            }

            let mut remaining: BoundedVec<u64, T::MaxActiveRounds> = BoundedVec::default();

            for &round_id in active.iter() {
                weight = weight.saturating_add(T::DbWeight::get().reads(1));
                let should_end = SaleRounds::<T>::get(round_id)
                    .map(|r| r.status == RoundStatus::Active && now > r.end_block)
                    .unwrap_or(false);

                if should_end {
                    Self::do_auto_end_sale(round_id);
                    // M1-audit: do_auto_end_sale 包含 SaleRounds::mutate(r+w) + unreserve(r+w) + event(w)
                    weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 3));
                } else {
                    let _ = remaining.try_push(round_id);
                }
            }

            ActiveRounds::<T>::put(remaining);
            weight.saturating_add(T::DbWeight::get().writes(1))
        }
    }

    // ==================== Extrinsics ====================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 创建发售轮次（需 Entity owner/admin 权限）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn create_sale_round(
            origin: OriginFor<T>,
            entity_id: u64,
            mode: SaleMode,
            total_supply: BalanceOf<T>,
            start_block: BlockNumberFor<T>,
            end_block: BlockNumberFor<T>,
            kyc_required: bool,
            min_kyc_level: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H3: Entity 存在性 + 权限验证
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            let owner = T::EntityProvider::entity_owner(entity_id).ok_or(Error::<T>::EntityNotFound)?;
            ensure!(owner == who || T::EntityProvider::is_entity_admin(entity_id, &who), Error::<T>::Unauthorized);

            // H1: total_supply > 0
            ensure!(!total_supply.is_zero(), Error::<T>::InvalidTotalSupply);
            // H2: end_block > start_block
            ensure!(end_block > start_block, Error::<T>::InvalidTimeWindow);
            // L2: min_kyc_level <= 4
            ensure!(min_kyc_level <= 4, Error::<T>::InvalidKycLevel);

            let now = <frame_system::Pallet<T>>::block_number();
            // L3-audit: start_block 不能在过去
            ensure!(start_block >= now, Error::<T>::StartBlockInPast);
            let round_id = NextRoundId::<T>::get();

            let round = SaleRound {
                id: round_id,
                entity_id,
                mode,
                status: RoundStatus::NotStarted,
                total_supply,
                sold_amount: Zero::zero(),
                remaining_amount: total_supply,
                participants_count: 0,
                payment_options_count: 0,
                vesting_config: VestingConfig::default(),
                kyc_required,
                min_kyc_level,
                start_block,
                end_block,
                dutch_start_price: None,
                dutch_end_price: None,
                creator: who.clone(),
                created_at: now,
                funds_withdrawn: false,
                cancelled_at: None,
                total_refunded_tokens: Zero::zero(),
                total_refunded_nex: Zero::zero(),
            };

            SaleRounds::<T>::insert(round_id, round);
            // L2-audit: checked_add 防止 u64 溢出导致 ID 覆盖
            let next_id = round_id.checked_add(1).ok_or(Error::<T>::RoundIdOverflow)?;

            NextRoundId::<T>::put(next_id);

            EntityRounds::<T>::try_mutate(entity_id, |rounds| -> DispatchResult {
                rounds.try_push(round_id).map_err(|_| Error::<T>::RoundsHistoryFull)?;
                Ok(())
            })?;

            Self::deposit_event(Event::SaleRoundCreated {
                round_id,
                entity_id,
                mode,
                total_supply,
            });
            Ok(())
        }

        /// 添加支付选项（仅 NotStarted 状态）
        ///
        /// DutchAuction 模式下 price 可为 0（实际价格由荷兰公式决定）。
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn add_payment_option(
            origin: OriginFor<T>,
            round_id: u64,
            asset_id: Option<AssetIdOf<T>>,
            price: BalanceOf<T>,
            min_purchase: BalanceOf<T>,
            max_purchase_per_account: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // H3-fix: 当前仅支持原生 NEX（asset_id = None），阻止多资产键不一致
            ensure!(asset_id.is_none(), Error::<T>::InvalidPaymentAsset);
            ensure!(!min_purchase.is_zero(), Error::<T>::InvalidPrice);
            ensure!(max_purchase_per_account >= min_purchase, Error::<T>::InvalidPurchaseLimits);

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(round.status == RoundStatus::NotStarted, Error::<T>::InvalidRoundStatus);

                // L4: DutchAuction 模式下 price 由荷兰公式决定，允许为 0
                if round.mode != SaleMode::DutchAuction {
                    ensure!(!price.is_zero(), Error::<T>::InvalidPrice);
                }

                let option = PaymentConfig {
                    asset_id,
                    price,
                    min_purchase,
                    max_purchase_per_account,
                    enabled: true,
                };

                // L2: 写入独立存储
                RoundPaymentOptions::<T>::try_mutate(round_id, |options| -> DispatchResult {
                    options.try_push(option).map_err(|_| Error::<T>::PaymentOptionsFull)?;
                    Ok(())
                })?;
                round.payment_options_count = round.payment_options_count.saturating_add(1);
                Ok(())
            })?;

            Self::deposit_event(Event::PaymentOptionAdded { round_id, asset_id });
            Ok(())
        }

        /// 设置锁仓配置（仅 NotStarted 状态）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn set_vesting_config(
            origin: OriginFor<T>,
            round_id: u64,
            vesting_type: VestingType,
            initial_unlock_bps: u16,
            cliff_duration: BlockNumberFor<T>,
            total_duration: BlockNumberFor<T>,
            unlock_interval: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(round.status == RoundStatus::NotStarted, Error::<T>::InvalidRoundStatus);

                ensure!(initial_unlock_bps <= 10000, Error::<T>::InvalidVestingConfig);
                // H7: total_duration >= cliff_duration
                ensure!(total_duration >= cliff_duration, Error::<T>::InvalidVestingDuration);

                round.vesting_config = VestingConfig {
                    vesting_type,
                    initial_unlock_bps,
                    cliff_duration,
                    total_duration,
                    unlock_interval,
                };
                Ok(())
            })?;

            Self::deposit_event(Event::VestingConfigSet { round_id });
            Ok(())
        }

        /// 配置荷兰拍卖（仅 NotStarted 状态 + DutchAuction 模式）
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(120_000_000, 6_000))]
        pub fn configure_dutch_auction(
            origin: OriginFor<T>,
            round_id: u64,
            start_price: BalanceOf<T>,
            end_price: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(round.mode == SaleMode::DutchAuction, Error::<T>::InvalidRoundStatus);
                // M3: 必须在 NotStarted 状态
                ensure!(round.status == RoundStatus::NotStarted, Error::<T>::InvalidRoundStatus);
                ensure!(start_price > end_price, Error::<T>::InvalidDutchAuctionConfig);
                // L5-audit: end_price 必须 > 0，防止拍卖末期免费代币
                ensure!(!end_price.is_zero(), Error::<T>::InvalidDutchAuctionConfig);

                round.dutch_start_price = Some(start_price);
                round.dutch_end_price = Some(end_price);
                Ok(())
            })?;

            Self::deposit_event(Event::DutchAuctionConfigured { round_id });
            Ok(())
        }

        /// 添加白名单（仅 NotStarted 状态，使用独立存储）
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
        pub fn add_to_whitelist(
            origin: OriginFor<T>,
            round_id: u64,
            accounts: BoundedVec<T::AccountId, T::MaxWhitelistSize>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let round = SaleRounds::<T>::get(round_id).ok_or(Error::<T>::RoundNotFound)?;
            ensure!(round.creator == who, Error::<T>::Unauthorized);
            // M5: 只能在 NotStarted 状态添加白名单
            ensure!(round.status == RoundStatus::NotStarted, Error::<T>::InvalidRoundStatus);

            let max_size = T::MaxWhitelistSize::get();
            let mut current_count = WhitelistCount::<T>::get(round_id);

            for account in accounts {
                if !RoundWhitelist::<T>::get(round_id, &account) {
                    ensure!(current_count < max_size, Error::<T>::WhitelistFull);
                    RoundWhitelist::<T>::insert(round_id, &account, true);
                    current_count = current_count.saturating_add(1);
                }
            }

            WhitelistCount::<T>::insert(round_id, current_count);
            Self::deposit_event(Event::WhitelistUpdated { round_id, count: current_count });
            Ok(())
        }

        /// 开始发售（锁定 Entity 代币，需至少一个支付选项）
        ///
        /// DutchAuction 模式还需提前调用 configure_dutch_auction 设置价格曲线。
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn start_sale(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(round.status == RoundStatus::NotStarted, Error::<T>::InvalidRoundStatus);
                // L2: 检查独立存储中的支付选项数
                ensure!(round.payment_options_count > 0, Error::<T>::NoPaymentOptions);

                // L4: DutchAuction 必须已配置价格曲线
                if round.mode == SaleMode::DutchAuction {
                    ensure!(
                        round.dutch_start_price.is_some() && round.dutch_end_price.is_some(),
                        Error::<T>::DutchAuctionNotConfigured
                    );
                }

                // 锁定 Entity 代币
                let entity_account = T::EntityProvider::entity_account(round.entity_id);
                T::TokenProvider::reserve(round.entity_id, &entity_account, round.total_supply)
                    .map_err(|_| Error::<T>::InsufficientTokenSupply)?;

                round.status = RoundStatus::Active;

                // L1: 注册到活跃轮次列表（on_initialize 自动结束用）
                ActiveRounds::<T>::try_mutate(|active| -> DispatchResult {
                    active.try_push(round_id).map_err(|_| Error::<T>::ActiveRoundsFull)?;
                    Ok(())
                })?;

                Self::deposit_event(Event::SaleRoundStarted { round_id });
                Ok(())
            })
        }

        /// 认购（扣除 NEX 到托管账户）
        #[pallet::call_index(6)]
        #[pallet::weight(Weight::from_parts(300_000_000, 14_000))]
        pub fn subscribe(
            origin: OriginFor<T>,
            round_id: u64,
            amount: BalanceOf<T>,
            payment_asset: Option<AssetIdOf<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let round = SaleRounds::<T>::get(round_id).ok_or(Error::<T>::RoundNotFound)?;

            // 验证状态
            ensure!(round.status == RoundStatus::Active, Error::<T>::InvalidRoundStatus);
            ensure!(round.remaining_amount >= amount, Error::<T>::SoldOut);

            // H9: 时间窗口校验
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now >= round.start_block && now <= round.end_block, Error::<T>::SaleNotInTimeWindow);

            // 验证未重复认购
            ensure!(!Subscriptions::<T>::contains_key(round_id, &who), Error::<T>::AlreadySubscribed);

            // H8: KYC 校验
            if round.kyc_required {
                let level = T::KycChecker::kyc_level(&who);
                ensure!(level >= round.min_kyc_level, Error::<T>::InsufficientKycLevel);
            }

            // 白名单校验（使用独立存储）
            if round.mode == SaleMode::WhitelistAllocation {
                ensure!(RoundWhitelist::<T>::get(round_id, &who), Error::<T>::NotInWhitelist);
            }

            // H1-audit: 预检参与者容量，避免转账后才发现已满
            {
                let participants = RoundParticipants::<T>::get(round_id);
                ensure!(
                    (participants.len() as u32) < T::MaxSubscriptionsPerRound::get(),
                    Error::<T>::ParticipantsFull
                );
            }

            // L2: 从独立存储读取支付选项
            let payment_options = RoundPaymentOptions::<T>::get(round_id);
            let payment_option = payment_options.iter()
                .find(|o| o.asset_id == payment_asset && o.enabled)
                .ok_or(Error::<T>::InvalidPaymentAsset)?;

            // 验证购买限额
            ensure!(amount >= payment_option.min_purchase, Error::<T>::BelowMinPurchase);
            ensure!(amount <= payment_option.max_purchase_per_account, Error::<T>::ExceedsPurchaseLimit);

            // H10: checked_mul 替代 saturating_mul
            let payment_amount = Self::calculate_payment_amount(&round, amount, payment_option)?;

            // H4: 实际 NEX 转账到 Pallet 托管账户
            let pallet_account = Self::pallet_account();
            T::Currency::transfer(
                &who,
                &pallet_account,
                payment_amount,
                ExistenceRequirement::KeepAlive,
            )?;

            // 创建认购记录
            let subscription = Subscription {
                subscriber: who.clone(),
                round_id,
                amount,
                payment_asset,
                payment_amount,
                subscribed_at: now,
                claimed: false,
                unlocked_amount: Zero::zero(),
                last_unlock_at: now,
                refunded: false,
            };

            Subscriptions::<T>::insert(round_id, &who, subscription);

            // 更新轮次数据
            SaleRounds::<T>::mutate(round_id, |maybe_round| {
                if let Some(round) = maybe_round {
                    round.sold_amount = round.sold_amount.saturating_add(amount);
                    round.remaining_amount = round.remaining_amount.saturating_sub(amount);
                    round.participants_count = round.participants_count.saturating_add(1);
                }
            });

            // 添加到参与者列表
            RoundParticipants::<T>::try_mutate(round_id, |participants| -> DispatchResult {
                participants.try_push(who.clone()).map_err(|_| Error::<T>::ParticipantsFull)?;
                Ok(())
            })?;

            // 更新募集资金统计
            RaisedFunds::<T>::mutate(round_id, payment_asset, |funds| {
                *funds = funds.saturating_add(payment_amount);
            });

            Self::deposit_event(Event::Subscribed {
                round_id,
                subscriber: who,
                amount,
                payment_amount,
            });
            Ok(())
        }

        /// 结束发售（释放未售 Entity 代币）
        ///
        /// 要求 `now >= end_block` 或已售罄（remaining == 0），防止创建者提前截止。
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn end_sale(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(round.status == RoundStatus::Active, Error::<T>::InvalidRoundStatus);

                // H2-audit: 必须超过结束时间 或 已售罄，才能结束
                let now = <frame_system::Pallet<T>>::block_number();
                ensure!(
                    now >= round.end_block || round.remaining_amount.is_zero(),
                    Error::<T>::SaleNotInTimeWindow
                );

                // 释放未售 Entity 代币
                if !round.remaining_amount.is_zero() {
                    let entity_account = T::EntityProvider::entity_account(round.entity_id);
                    T::TokenProvider::unreserve(round.entity_id, &entity_account, round.remaining_amount);
                    // M1-fix: 清零 remaining_amount（与 do_auto_end_sale 一致）
                    round.remaining_amount = Zero::zero();
                }

                round.status = RoundStatus::Ended;

                // L1: 从活跃列表移除
                ActiveRounds::<T>::mutate(|active| {
                    active.retain(|&id| id != round_id);
                });

                Self::deposit_event(Event::SaleRoundEnded {
                    round_id,
                    sold_amount: round.sold_amount,
                    participants_count: round.participants_count,
                });
                Ok(())
            })
        }

        /// 领取代币（初始解锁部分，Entity 代币从 reserve 转给用户）
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn claim_tokens(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let round = SaleRounds::<T>::get(round_id).ok_or(Error::<T>::RoundNotFound)?;
            // H2-fix: 仅允许 Ended 状态（Completed 来自 cancel→reclaim 路径，代币已被 unreserve）
            ensure!(
                round.status == RoundStatus::Ended,
                Error::<T>::InvalidRoundStatus
            );

            Subscriptions::<T>::try_mutate(round_id, &who, |maybe_sub| -> DispatchResult {
                let sub = maybe_sub.as_mut().ok_or(Error::<T>::NotSubscribed)?;
                ensure!(!sub.claimed, Error::<T>::AlreadyClaimed);

                // 计算初始解锁量
                let initial_unlock = Self::calculate_initial_unlock(&round.vesting_config, sub.amount);

                // H5: 实际转移 Entity 代币
                if !initial_unlock.is_zero() {
                    let entity_account = T::EntityProvider::entity_account(round.entity_id);
                    T::TokenProvider::repatriate_reserved(
                        round.entity_id,
                        &entity_account,
                        &who,
                        initial_unlock,
                    )?;
                }

                sub.claimed = true;
                sub.unlocked_amount = initial_unlock;

                Self::deposit_event(Event::TokensClaimed {
                    round_id,
                    subscriber: who.clone(),
                    amount: initial_unlock,
                });
                Ok(())
            })
        }

        /// 解锁代币（锁仓期后，Entity 代币从 reserve 转给用户）
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn unlock_tokens(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let round = SaleRounds::<T>::get(round_id).ok_or(Error::<T>::RoundNotFound)?;
            // M2-audit: 仅 Ended 状态允许解锁，防止 Cancelled 等状态下误操作
            ensure!(round.status == RoundStatus::Ended, Error::<T>::InvalidRoundStatus);

            Subscriptions::<T>::try_mutate(round_id, &who, |maybe_sub| -> DispatchResult {
                let sub = maybe_sub.as_mut().ok_or(Error::<T>::NotSubscribed)?;
                ensure!(sub.claimed, Error::<T>::NotSubscribed);

                let now = <frame_system::Pallet<T>>::block_number();

                // 计算可解锁量
                let unlockable = Self::calculate_unlockable(
                    &round.vesting_config,
                    sub.amount,
                    sub.unlocked_amount,
                    sub.subscribed_at,
                    now,
                )?;

                ensure!(!unlockable.is_zero(), Error::<T>::NoTokensToUnlock);

                // H5: 实际转移 Entity 代币
                let entity_account = T::EntityProvider::entity_account(round.entity_id);
                T::TokenProvider::repatriate_reserved(
                    round.entity_id,
                    &entity_account,
                    &who,
                    unlockable,
                )?;

                sub.unlocked_amount = sub.unlocked_amount.saturating_add(unlockable);
                sub.last_unlock_at = now;

                Self::deposit_event(Event::TokensUnlocked {
                    round_id,
                    subscriber: who.clone(),
                    amount: unlockable,
                });
                Ok(())
            })
        }

        /// 取消发售（释放未售 Entity 代币，已认购者需调用 claim_refund 退款）
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn cancel_sale(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(
                    round.status == RoundStatus::NotStarted || round.status == RoundStatus::Active,
                    Error::<T>::InvalidRoundStatus
                );

                // 如果已启动（Active），释放未售 Entity 代币
                if round.status == RoundStatus::Active && !round.remaining_amount.is_zero() {
                    let entity_account = T::EntityProvider::entity_account(round.entity_id);
                    T::TokenProvider::unreserve(round.entity_id, &entity_account, round.remaining_amount);
                }

                round.status = RoundStatus::Cancelled;
                round.cancelled_at = Some(<frame_system::Pallet<T>>::block_number());

                // L1: 从活跃列表移除（如果是 Active 状态被取消）
                ActiveRounds::<T>::mutate(|active| {
                    active.retain(|&id| id != round_id);
                });

                Self::deposit_event(Event::SaleRoundCancelled { round_id });
                Ok(())
            })
        }

        /// 认购者领取退款（仅 Cancelled 状态，释放对应 Entity 代币 + 退还 NEX）
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(250_000_000, 12_000))]
        pub fn claim_refund(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let round = SaleRounds::<T>::get(round_id).ok_or(Error::<T>::RoundNotFound)?;
            ensure!(round.status == RoundStatus::Cancelled, Error::<T>::SaleNotCancelled);

            Subscriptions::<T>::try_mutate(round_id, &who, |maybe_sub| -> DispatchResult {
                let sub = maybe_sub.as_mut().ok_or(Error::<T>::NotSubscribed)?;
                ensure!(!sub.refunded, Error::<T>::AlreadyRefunded);

                // M5-audit: 释放认购者对应的 Entity 代币锁定，检查返回值
                let entity_account = T::EntityProvider::entity_account(round.entity_id);
                let remaining = T::TokenProvider::unreserve(round.entity_id, &entity_account, sub.amount);
                ensure!(remaining.is_zero(), Error::<T>::IncompleteUnreserve);

                // 退还 NEX
                let pallet_account = Self::pallet_account();
                T::Currency::transfer(
                    &pallet_account,
                    &who,
                    sub.payment_amount,
                    ExistenceRequirement::AllowDeath,
                )?;

                sub.refunded = true;

                // L3: 更新退款计数器
                SaleRounds::<T>::mutate(round_id, |maybe_round| {
                    if let Some(r) = maybe_round {
                        r.total_refunded_tokens = r.total_refunded_tokens.saturating_add(sub.amount);
                        r.total_refunded_nex = r.total_refunded_nex.saturating_add(sub.payment_amount);
                    }
                });

                Self::deposit_event(Event::RefundClaimed {
                    round_id,
                    subscriber: who.clone(),
                    amount: sub.payment_amount,
                });
                Ok(())
            })
        }

        /// 回收过期未领退款（宽限期后创建者可回收未领的 Entity 代币和 NEX）
        ///
        /// 防止未领取退款导致 Entity 代币永久锁定。
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn reclaim_unclaimed_tokens(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(round.status == RoundStatus::Cancelled, Error::<T>::SaleNotCancelled);

                // 检查宽限期是否已过
                let cancelled_at = round.cancelled_at.ok_or(Error::<T>::SaleNotCancelled)?;
                let now = <frame_system::Pallet<T>>::block_number();
                let deadline = cancelled_at.saturating_add(T::RefundGracePeriod::get());
                ensure!(now >= deadline, Error::<T>::RefundPeriodNotExpired);

                // 计算未领取的代币和 NEX
                let unclaimed_tokens = round.sold_amount.saturating_sub(round.total_refunded_tokens);
                let total_raised = RaisedFunds::<T>::get(round_id, Option::<AssetIdOf<T>>::None);
                let unclaimed_nex = total_raised.saturating_sub(round.total_refunded_nex);

                let entity_account = T::EntityProvider::entity_account(round.entity_id);

                // 释放未领取的 Entity 代币
                if !unclaimed_tokens.is_zero() {
                    T::TokenProvider::unreserve(round.entity_id, &entity_account, unclaimed_tokens);
                }

                // 将未领取的 NEX 转给 Entity
                if !unclaimed_nex.is_zero() {
                    let pallet_account = Self::pallet_account();
                    T::Currency::transfer(
                        &pallet_account,
                        &entity_account,
                        unclaimed_nex,
                        ExistenceRequirement::AllowDeath,
                    )?;
                }

                // C1-fix: 标记资金已提取，防止 withdraw_funds 双重提取
                round.funds_withdrawn = true;
                round.status = RoundStatus::Completed;

                Self::deposit_event(Event::ExpiredRefundsReclaimed {
                    round_id,
                    tokens_reclaimed: unclaimed_tokens,
                    nex_reclaimed: unclaimed_nex,
                });
                Ok(())
            })
        }

        /// 提取募集资金（NEX → Entity 派生账户，仅 Ended/Completed）
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(200_000_000, 10_000))]
        pub fn withdraw_funds(
            origin: OriginFor<T>,
            round_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            SaleRounds::<T>::try_mutate(round_id, |maybe_round| -> DispatchResult {
                let round = maybe_round.as_mut().ok_or(Error::<T>::RoundNotFound)?;
                ensure!(round.creator == who, Error::<T>::Unauthorized);
                ensure!(
                    round.status == RoundStatus::Ended || round.status == RoundStatus::Completed,
                    Error::<T>::InvalidRoundStatus
                );
                ensure!(!round.funds_withdrawn, Error::<T>::FundsAlreadyWithdrawn);

                // 计算 NEX 总募集额（仅原生 NEX）
                let total_raised = RaisedFunds::<T>::get(round_id, Option::<AssetIdOf<T>>::None);

                if !total_raised.is_zero() {
                    let pallet_account = Self::pallet_account();
                    let entity_account = T::EntityProvider::entity_account(round.entity_id);
                    T::Currency::transfer(
                        &pallet_account,
                        &entity_account,
                        total_raised,
                        ExistenceRequirement::AllowDeath,
                    )?;
                }

                round.funds_withdrawn = true;

                Self::deposit_event(Event::FundsWithdrawn {
                    round_id,
                    recipient: T::EntityProvider::entity_account(round.entity_id),
                    amount: total_raised,
                });
                Ok(())
            })
        }
    }

    // ==================== 辅助函数 ====================

    impl<T: Config> Pallet<T> {
        /// 获取 Pallet 托管账户
        pub fn pallet_account() -> T::AccountId {
            SALE_PALLET_ID.into_account_truncating()
        }

        /// 计算支付金额（H10: 使用 checked_mul 防止溢出）
        fn calculate_payment_amount(
            round: &SaleRoundOf<T>,
            amount: BalanceOf<T>,
            payment_option: &PaymentConfig<AssetIdOf<T>, BalanceOf<T>>,
        ) -> Result<BalanceOf<T>, DispatchError> {
            let price = if round.mode == SaleMode::DutchAuction {
                Self::calculate_dutch_price(round)?
            } else {
                payment_option.price
            };

            // H10: checked_mul 替代 saturating_mul
            let amount_u128: u128 = amount.saturated_into();
            let price_u128: u128 = price.saturated_into();
            let result = amount_u128.checked_mul(price_u128).ok_or(Error::<T>::ArithmeticOverflow)?;
            Ok(result.saturated_into())
        }

        /// 计算荷兰拍卖当前价格
        fn calculate_dutch_price(round: &SaleRoundOf<T>) -> Result<BalanceOf<T>, DispatchError> {
            let start_price = round.dutch_start_price.ok_or(Error::<T>::InvalidDutchAuctionConfig)?;
            let end_price = round.dutch_end_price.ok_or(Error::<T>::InvalidDutchAuctionConfig)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let start = round.start_block;
            let end = round.end_block;

            if now <= start {
                return Ok(start_price);
            }
            if now >= end {
                return Ok(end_price);
            }

            // 线性递减
            let total_duration: u128 = Self::block_to_u128(end.saturating_sub(start));
            let elapsed: u128 = Self::block_to_u128(now.saturating_sub(start));
            let start_u128: u128 = start_price.saturated_into();
            let end_u128: u128 = end_price.saturated_into();
            let price_range: u128 = start_u128.saturating_sub(end_u128);

            let price_drop = price_range.saturating_mul(elapsed) / total_duration;
            let current_price: u128 = start_u128.saturating_sub(price_drop);

            Ok(current_price.saturated_into())
        }

        /// 计算初始解锁量
        pub(crate) fn calculate_initial_unlock(
            vesting: &VestingConfig<BlockNumberFor<T>>,
            total: BalanceOf<T>,
        ) -> BalanceOf<T> {
            if vesting.vesting_type == VestingType::None {
                return total;
            }

            let initial_bps: u128 = vesting.initial_unlock_bps.into();
            let total_u128: u128 = total.saturated_into();
            let initial: u128 = total_u128.saturating_mul(initial_bps) / 10000;

            initial.saturated_into()
        }

        /// 计算可解锁量
        pub(crate) fn calculate_unlockable(
            vesting: &VestingConfig<BlockNumberFor<T>>,
            total: BalanceOf<T>,
            already_unlocked: BalanceOf<T>,
            start: BlockNumberFor<T>,
            now: BlockNumberFor<T>,
        ) -> Result<BalanceOf<T>, DispatchError> {
            if vesting.vesting_type == VestingType::None {
                return Ok(total.saturating_sub(already_unlocked));
            }

            // 检查悬崖期
            let cliff_end = start.saturating_add(vesting.cliff_duration);
            if now < cliff_end {
                return Err(Error::<T>::CliffNotReached.into());
            }

            let total_end = start.saturating_add(vesting.total_duration);

            if now >= total_end {
                // 全部解锁
                return Ok(total.saturating_sub(already_unlocked));
            }

            // 计算线性/阶梯解锁
            let vesting_duration: u128 = Self::block_to_u128(vesting.total_duration.saturating_sub(vesting.cliff_duration));
            let elapsed: u128 = Self::block_to_u128(now.saturating_sub(cliff_end));

            let initial_bps: u128 = vesting.initial_unlock_bps.into();
            let vesting_bps: u128 = 10000u128.saturating_sub(initial_bps);

            let total_u128: u128 = total.saturated_into();
            let vesting_amount = total_u128.saturating_mul(vesting_bps) / 10000;

            // M1-audit: Cliff 类型使用 unlock_interval 做阶梯解锁
            let effective_elapsed = if vesting.vesting_type == VestingType::Cliff {
                let interval: u128 = Self::block_to_u128(vesting.unlock_interval);
                if interval > 0 {
                    // 按 interval 取整（阶梯）
                    (elapsed / interval).saturating_mul(interval)
                } else {
                    elapsed
                }
            } else {
                // Linear / Custom：连续线性
                elapsed
            };

            let unlocked_vesting = if vesting_duration > 0 {
                vesting_amount.saturating_mul(effective_elapsed) / vesting_duration
            } else {
                vesting_amount
            };

            let initial_amount = total_u128.saturating_mul(initial_bps) / 10000;
            let total_unlockable: u128 = initial_amount.saturating_add(unlocked_vesting);

            let total_unlockable_bal: BalanceOf<T> = total_unlockable.saturated_into();
            let unlockable = total_unlockable_bal.saturating_sub(already_unlocked);
            Ok(unlockable)
        }

        /// 获取轮次当前价格
        pub fn get_current_price(round_id: u64, asset_id: Option<AssetIdOf<T>>) -> Option<BalanceOf<T>> {
            let round = SaleRounds::<T>::get(round_id)?;

            if round.mode == SaleMode::DutchAuction {
                Self::calculate_dutch_price(&round).ok()
            } else {
                // L2: 从独立存储读取
                let options = RoundPaymentOptions::<T>::get(round_id);
                options.iter()
                    .find(|o| o.asset_id == asset_id && o.enabled)
                    .map(|o| o.price)
            }
        }

        /// L1: 自动结束过期发售（on_initialize 调用）
        fn do_auto_end_sale(round_id: u64) {
            SaleRounds::<T>::mutate(round_id, |maybe_round| {
                if let Some(round) = maybe_round.as_mut() {
                    // 释放未售 Entity 代币
                    if !round.remaining_amount.is_zero() {
                        let entity_account = T::EntityProvider::entity_account(round.entity_id);
                        T::TokenProvider::unreserve(round.entity_id, &entity_account, round.remaining_amount);
                        // M1-fix: 清零 remaining_amount，防止查询返回过时数据
                        round.remaining_amount = Zero::zero();
                    }
                    let sold = round.sold_amount;
                    let count = round.participants_count;
                    round.status = RoundStatus::Ended;

                    Self::deposit_event(Event::SaleAutoEnded {
                        round_id,
                        sold_amount: sold,
                        participants_count: count,
                    });
                }
            });
        }

        /// 获取用户认购信息
        pub fn get_subscription(round_id: u64, account: &T::AccountId) -> Option<SubscriptionOf<T>> {
            Subscriptions::<T>::get(round_id, account)
        }

        /// BlockNumber 转 u128
        fn block_to_u128(block: BlockNumberFor<T>) -> u128 {
            use sp_runtime::traits::UniqueSaturatedInto;
            block.unique_saturated_into()
        }

        /// 获取用户可解锁代币数量
        pub fn get_unlockable_amount(round_id: u64, account: &T::AccountId) -> BalanceOf<T> {
            let round = match SaleRounds::<T>::get(round_id) {
                Some(r) => r,
                None => return Zero::zero(),
            };

            let sub = match Subscriptions::<T>::get(round_id, account) {
                Some(s) => s,
                None => return Zero::zero(),
            };

            if !sub.claimed {
                return Zero::zero();
            }

            let now = <frame_system::Pallet<T>>::block_number();
            Self::calculate_unlockable(
                &round.vesting_config,
                sub.amount,
                sub.unlocked_amount,
                sub.subscribed_at,
                now,
            ).unwrap_or(Zero::zero())
        }
    }
}
