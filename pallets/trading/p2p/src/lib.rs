#![cfg_attr(not(feature = "std"), no_std)]

//! # P2P Trading Pallet (P2P 交易模块)
//!
//! ## 概述
//! 统一 Buy（USDT→NEX）和 Sell（NEX→USDT）两方向的 P2P 交易。
//! 合并原 `pallet-trading-otc` 和 `pallet-trading-swap` 的功能。
//!
//! ## Buy 流程（原 OTC）
//! 1. 买家 create_buy_order → 做市商 NEX 锁定到托管
//! 2. 买家链下转 USDT → mark_paid
//! 3. 做市商确认收款 → release_nex
//!
//! ## Sell 流程（原 Swap）
//! 1. 用户 create_sell_order → 用户 NEX 锁定到托管
//! 2. 做市商链下转 USDT → mark_sell_complete（提交 TRC20 tx hash）
//! 3. OCW 验证 TRC20 → confirm_verification → NEX 释放给做市商
//!
//! ## 版本历史
//! - v0.1.0 (2026-02-08): 初始骨架
//! - v1.0.0 (2026-02-08): Phase 1 完整实现

extern crate alloc;

pub use pallet::*;

pub mod ocw;
pub mod kyc;
pub mod types;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use frame_support::{
        traits::{Currency, Get, UnixTime},
        BoundedVec,
        PalletId,
        sp_runtime::{SaturatedConversion, traits::{Saturating, AccountIdConversion}},
    };
    use sp_core::H256;
    use pallet_trading_common::{
        TronAddress,
        MomentOf,
        Cid,
        PricingProvider,
        MakerInterface,
        MakerCreditInterface,
        MakerValidationError,
    };
    use pallet_escrow::Escrow as EscrowTrait;
    use crate::types::*;

    /// Pallet ID（用于生成内部账户）
    const PALLET_ID: PalletId = PalletId(*b"p2p/trad");

    /// Balance 类型别名
    pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    // ========================================================================
    // Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {

        /// 货币操作
        type Currency: Currency<Self::AccountId>;

        /// 时间戳（Buy 侧用 UnixTime 计算超时）
        type Timestamp: UnixTime;

        /// 托管服务接口
        type Escrow: pallet_escrow::Escrow<Self::AccountId, BalanceOf<Self>>;

        /// 买家信用记录接口（Buy 侧）
        type BuyerCredit: pallet_trading_credit::BuyerCreditInterface<Self::AccountId>
            + pallet_trading_credit::quota::BuyerQuotaInterface<Self::AccountId>;

        /// 做市商信用记录接口
        type MakerCredit: MakerCreditInterface;

        /// 定价服务接口
        type Pricing: PricingProvider<BalanceOf<Self>>;

        /// 做市商接口
        type MakerPallet: MakerInterface<Self::AccountId, BalanceOf<Self>>;

        /// 委员会起源（KYC 配置管理）
        type CommitteeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Identity Provider（KYC 验证，Buy 侧）
        type IdentityProvider: IdentityVerificationProvider<Self::AccountId>;

        /// 验证权限（OCW 或委员会，Sell 侧）
        type VerificationOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// 仲裁员起源（争议判定）
        type ArbitratorOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// CID 锁定管理器（争议期间锁定证据）
        type CidLockManager: pallet_storage_service::CidLockManager<Self::Hash, BlockNumberFor<Self>>;

        /// 权重信息
        type WeightInfo: WeightInfo;

        // ===== Buy-side 常量 =====

        /// Buy 订单超时时间（毫秒）
        #[pallet::constant]
        type BuyOrderTimeout: Get<u64>;

        /// 证据窗口时间（毫秒）
        #[pallet::constant]
        type EvidenceWindow: Get<u64>;

        /// 首购订单 USD 固定价值（精度 10^6）
        #[pallet::constant]
        type FirstPurchaseUsdValue: Get<u128>;

        /// 首购订单最小 NEX 数量
        #[pallet::constant]
        type MinFirstPurchaseCosAmount: Get<BalanceOf<Self>>;

        /// 首购订单最大 NEX 数量
        #[pallet::constant]
        type MaxFirstPurchaseCosAmount: Get<BalanceOf<Self>>;

        /// Buy 订单最大 USD 金额（精度 10^6）
        #[pallet::constant]
        type MaxOrderUsdAmount: Get<u64>;

        /// Buy 订单最小 USD 金额（精度 10^6，首购除外）
        #[pallet::constant]
        type MinOrderUsdAmount: Get<u64>;

        /// 首购订单固定 USD 金额（精度 10^6）
        #[pallet::constant]
        type FirstPurchaseUsdAmount: Get<u64>;

        /// 金额验证容差（基点）
        #[pallet::constant]
        type AmountValidationTolerance: Get<u16>;

        /// 每个做市商最多同时接收的首购订单数量
        #[pallet::constant]
        type MaxFirstPurchaseOrdersPerMaker: Get<u32>;

        /// 最小押金金额
        #[pallet::constant]
        type MinDeposit: Get<BalanceOf<Self>>;

        /// 统一押金比例（bps）
        #[pallet::constant]
        type DepositRate: Get<u16>;

        /// 取消订单押金扣除比例（bps）
        #[pallet::constant]
        type CancelPenaltyRate: Get<u16>;

        /// 做市商最低押金 USD 价值（精度 10^6）
        #[pallet::constant]
        type MinMakerDepositUsd: Get<u64>;

        /// 争议响应超时时间（秒）
        #[pallet::constant]
        type DisputeResponseTimeout: Get<u64>;

        /// 争议仲裁超时时间（秒）
        #[pallet::constant]
        type DisputeArbitrationTimeout: Get<u64>;

        // ===== Sell-side 常量 =====

        /// Sell 做市商兑换超时时间（区块数）
        #[pallet::constant]
        type SellTimeoutBlocks: Get<BlockNumberFor<Self>>;

        /// TRC20 验证超时时间（区块数）
        #[pallet::constant]
        type VerificationTimeoutBlocks: Get<BlockNumberFor<Self>>;

        /// Sell 最小兑换金额
        #[pallet::constant]
        type MinSellAmount: Get<BalanceOf<Self>>;

        /// TRON 交易哈希 TTL（区块数）
        #[pallet::constant]
        type TxHashTtlBlocks: Get<BlockNumberFor<Self>>;

        /// 验证确认奖励
        #[pallet::constant]
        type VerificationReward: Get<BalanceOf<Self>>;

        /// Sell 手续费率（基点）
        #[pallet::constant]
        type SellFeeRateBps: Get<u32>;

        /// 最低 Sell 手续费
        #[pallet::constant]
        type MinSellFee: Get<BalanceOf<Self>>;
    }

    /// Identity 验证 Provider trait（从 OTC 迁移）
    pub trait IdentityVerificationProvider<AccountId> {
        /// 获取账户的最高身份认证等级
        /// 返回 None 表示未设置身份信息
        /// 返回值：0=Unknown, 1=FeePaid, 2=Reasonable, 3=KnownGood
        fn get_highest_judgement_priority(who: &AccountId) -> Option<u8>;

        /// 检查账户的身份认证是否有问题
        fn has_problematic_judgement(who: &AccountId) -> bool;
    }

    /// 空实现（KYC 未启用时使用）
    impl<AccountId> IdentityVerificationProvider<AccountId> for () {
        fn get_highest_judgement_priority(_who: &AccountId) -> Option<u8> { None }
        fn has_problematic_judgement(_who: &AccountId) -> bool { false }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Hooks
    // ========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Buy 侧：每 100 个区块检查过期订单
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let check_interval: u32 = 100;
            let now_u32: u32 = now.saturated_into();
            if now_u32 % check_interval != 0 {
                return Weight::zero();
            }
            Self::process_expired_buy_orders()
        }

        /// 空闲时归档已完成订单 + 清理过期 TxHash
        fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let base_weight = Weight::from_parts(20_000, 0);
            if remaining_weight.ref_time() < base_weight.ref_time() * 10 {
                return Weight::zero();
            }
            // Buy 侧归档
            let w1 = Self::archive_completed_buy_orders(5);
            // Sell 侧归档
            let w2 = Self::archive_completed_sell_orders(5);
            // Buy 侧 TxHash TTL 清理
            let w3 = Self::cleanup_expired_buy_tx_hashes(now, 10);
            w1.saturating_add(w2).saturating_add(w3)
        }
    }

    // ========================================================================
    // Storage — Buy-side（原 OTC）
    // ========================================================================

    /// 下一个 Buy 订单 ID
    #[pallet::storage]
    #[pallet::getter(fn next_buy_order_id)]
    pub type NextBuyOrderId<T> = StorageValue<_, u64, ValueQuery>;

    /// Buy 订单记录
    #[pallet::storage]
    #[pallet::getter(fn buy_orders)]
    pub type BuyOrders<T: Config> = StorageMap<_, Blake2_128Concat, u64, BuyOrder<T>>;

    /// 买家订单列表
    #[pallet::storage]
    #[pallet::getter(fn buyer_order_list)]
    pub type BuyerOrderList<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId,
        BoundedVec<u64, ConstU32<100>>, ValueQuery,
    >;

    /// 做市商 Buy 订单列表
    #[pallet::storage]
    #[pallet::getter(fn maker_buy_order_list)]
    pub type MakerBuyOrderList<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        BoundedVec<u64, ConstU32<200>>, ValueQuery,
    >;

    /// 买家是否已首购
    #[pallet::storage]
    #[pallet::getter(fn has_first_purchased)]
    pub type HasFirstPurchased<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId, bool, ValueQuery,
    >;

    /// 做市商首购订单计数
    #[pallet::storage]
    #[pallet::getter(fn maker_first_purchase_count)]
    pub type MakerFirstPurchaseCount<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, u32, ValueQuery,
    >;

    /// 做市商首购订单列表
    #[pallet::storage]
    #[pallet::getter(fn maker_first_purchase_orders)]
    pub type MakerFirstPurchaseOrders<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        BoundedVec<u64, ConstU32<10>>, ValueQuery,
    >;

    /// Buy 侧 TRON 交易哈希使用记录（防重放）
    #[pallet::storage]
    #[pallet::getter(fn buy_tron_tx_used)]
    pub type BuyTronTxUsed<T: Config> = StorageMap<
        _, Blake2_128Concat, H256, BlockNumberFor<T>,
    >;

    /// Buy 争议记录
    #[pallet::storage]
    #[pallet::getter(fn buy_disputes)]
    pub type BuyDisputes<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, BuyDispute<T>, OptionQuery,
    >;

    /// 买家已完成订单计数
    #[pallet::storage]
    #[pallet::getter(fn buyer_completed_order_count)]
    pub type BuyerCompletedOrderCount<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId, u32, ValueQuery,
    >;

    /// 押金池总余额
    #[pallet::storage]
    #[pallet::getter(fn total_deposit_pool_balance)]
    pub type TotalDepositPoolBalance<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Buy 归档订单 L1
    #[pallet::storage]
    #[pallet::getter(fn archived_buy_orders)]
    pub type ArchivedBuyOrders<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, ArchivedBuyOrder<T>, OptionQuery,
    >;

    /// Buy 归档游标
    #[pallet::storage]
    pub type BuyArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Buy L1→L2 归档游标
    #[pallet::storage]
    pub type BuyL1ArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Buy 过期检查游标（持久化，避免只扫描最近 N 个订单）
    #[pallet::storage]
    pub type BuyExpiryCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Buy 侧 TxHash TTL 清理游标（BlockNumber）
    #[pallet::storage]
    pub type BuyTxHashCleanupCursor<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    // ========================================================================
    // Storage — Sell-side（原 Swap）
    // ========================================================================

    /// 下一个 Sell 订单 ID
    #[pallet::storage]
    #[pallet::getter(fn next_sell_order_id)]
    pub type NextSellOrderId<T> = StorageValue<_, u64, ValueQuery>;

    /// Sell 订单记录
    #[pallet::storage]
    #[pallet::getter(fn sell_orders)]
    pub type SellOrders<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, SellOrder<T>,
    >;

    /// 用户 Sell 订单列表
    #[pallet::storage]
    #[pallet::getter(fn user_sell_list)]
    pub type UserSellList<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId,
        BoundedVec<u64, ConstU32<100>>, ValueQuery,
    >;

    /// 做市商 Sell 订单列表
    #[pallet::storage]
    #[pallet::getter(fn maker_sell_list)]
    pub type MakerSellList<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        BoundedVec<u64, ConstU32<200>>, ValueQuery,
    >;

    /// Sell 侧已使用 TRON 交易哈希（防重放）
    #[pallet::storage]
    #[pallet::getter(fn sell_used_tx_hashes)]
    pub type SellUsedTxHashes<T: Config> = StorageMap<
        _, Blake2_128Concat, BoundedVec<u8, ConstU32<128>>,
        BlockNumberFor<T>, OptionQuery,
    >;

    /// Sell TTL 清理游标
    #[pallet::storage]
    pub type SellTxHashCleanupCursor<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    /// Sell 待验证队列
    #[pallet::storage]
    #[pallet::getter(fn sell_pending_verifications)]
    pub type SellPendingVerifications<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, SellVerificationRequest<T>, OptionQuery,
    >;

    /// Sell 验证游标
    #[pallet::storage]
    pub type SellVerificationCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Sell OCW 验证结果
    #[pallet::storage]
    #[pallet::getter(fn sell_ocw_verification_results)]
    pub type SellOcwVerificationResults<T: Config> = StorageMap<
        _, Blake2_128Concat, u64,
        (bool, Option<BoundedVec<u8, ConstU32<128>>>), OptionQuery,
    >;

    /// Sell 少付证据
    #[pallet::storage]
    #[pallet::getter(fn sell_underpaid_evidences)]
    pub type SellUnderpaidEvidences<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, SellUnderpaidEvidence<T>, OptionQuery,
    >;

    /// Sell 归档 L1
    #[pallet::storage]
    #[pallet::getter(fn archived_sell_orders)]
    pub type ArchivedSellOrders<T: Config> = StorageMap<
        _, Blake2_128Concat, u64, ArchivedSellOrder<T>, OptionQuery,
    >;

    /// Sell 归档游标
    #[pallet::storage]
    pub type SellArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Sell L1→L2 归档游标
    #[pallet::storage]
    pub type SellL1ArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    // ========================================================================
    // Storage — 共享
    // ========================================================================

    /// L2 归档（Buy + Sell 共享）
    #[pallet::storage]
    #[pallet::getter(fn archived_orders_l2)]
    pub type ArchivedOrdersL2Store<T: Config> = StorageMap<
        _, Blake2_128Concat, (u8, u64), ArchivedOrderL2, OptionQuery,
    >;

    /// P2P 永久统计
    #[pallet::storage]
    #[pallet::getter(fn p2p_stats)]
    pub type P2pStats<T: Config> = StorageValue<_, P2pPermanentStats, ValueQuery>;

    /// KYC 配置
    #[pallet::storage]
    #[pallet::getter(fn kyc_config)]
    pub type KycConfigStore<T: Config> = StorageValue<
        _, KycConfig<BlockNumberFor<T>>, ValueQuery,
    >;

    /// KYC 豁免账户
    #[pallet::storage]
    #[pallet::getter(fn kyc_exempt_accounts)]
    pub type KycExemptAccounts<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId, (), OptionQuery,
    >;

    // ========================================================================
    // Order Structs（定义在 pallet 内部，因为依赖 Config）
    // ========================================================================

    /// Buy 订单结构（原 OTC Order）
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct BuyOrder<T: Config> {
        /// 做市商 ID
        pub maker_id: u64,
        /// 做市商账户
        pub maker: T::AccountId,
        /// 买家账户
        pub taker: T::AccountId,
        /// 单价（USDT/NEX，精度 10^6）
        pub price: BalanceOf<T>,
        /// NEX 数量
        pub qty: BalanceOf<T>,
        /// USDT 总金额（精度 10^6，如 10_000_000 = 10 USD）
        pub amount: BalanceOf<T>,
        /// 创建时间（Unix 秒）
        pub created_at: MomentOf,
        /// 超时时间（Unix 秒）
        pub expire_at: MomentOf,
        /// 证据窗口截止（Unix 秒）
        pub evidence_until: MomentOf,
        /// 做市商 TRON 收款地址
        pub maker_tron_address: TronAddress,
        /// 支付承诺哈希
        pub payment_commit: H256,
        /// 联系方式承诺哈希
        pub contact_commit: H256,
        /// 订单状态
        pub state: BuyOrderState,
        /// 完成时间
        pub completed_at: Option<MomentOf>,
        /// 是否为首购
        pub is_first_purchase: bool,
        /// 买家押金金额
        pub buyer_deposit: BalanceOf<T>,
        /// 押金状态
        pub deposit_status: DepositStatus,
    }

    /// Buy 争议记录
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct BuyDispute<T: Config> {
        pub order_id: u64,
        pub initiator: T::AccountId,
        pub respondent: T::AccountId,
        pub created_at: MomentOf,
        pub response_deadline: MomentOf,
        pub arbitration_deadline: MomentOf,
        pub status: BuyDisputeStatus,
        pub buyer_evidence: Option<Cid>,
        pub maker_evidence: Option<Cid>,
    }

    /// Buy 归档 L1
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct ArchivedBuyOrder<T: Config> {
        pub maker_id: u64,
        pub taker: T::AccountId,
        pub qty: u64,
        pub amount: u64,
        pub state: BuyOrderState,
        pub completed_at: u64,
    }

    /// Sell 订单结构（原 MakerSwapRecord）
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct SellOrder<T: Config> {
        /// 订单 ID
        pub sell_id: u64,
        /// 做市商 ID
        pub maker_id: u64,
        /// 做市商账户
        pub maker: T::AccountId,
        /// 用户账户
        pub user: T::AccountId,
        /// NEX 数量
        pub nex_amount: BalanceOf<T>,
        /// USDT 金额（精度 10^6）
        pub usdt_amount: u64,
        /// USDT 接收地址
        pub usdt_address: TronAddress,
        /// 创建区块
        pub created_at: BlockNumberFor<T>,
        /// 超时区块
        pub timeout_at: BlockNumberFor<T>,
        /// TRC20 交易哈希
        pub trc20_tx_hash: Option<BoundedVec<u8, ConstU32<128>>>,
        /// 完成区块
        pub completed_at: Option<BlockNumberFor<T>>,
        /// 证据 CID
        pub evidence_cid: Option<BoundedVec<u8, ConstU32<256>>>,
        /// 订单状态
        pub status: SellOrderStatus,
        /// 价格（USDT，精度 10^6）
        pub price_usdt: u64,
        /// 仲裁押金
        pub dispute_deposit: Option<BalanceOf<T>>,
    }

    /// Sell TRC20 验证请求
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct SellVerificationRequest<T: Config> {
        pub sell_id: u64,
        pub tx_hash: BoundedVec<u8, ConstU32<128>>,
        pub expected_to: TronAddress,
        pub expected_amount: u64,
        pub submitted_at: BlockNumberFor<T>,
        pub verification_timeout_at: BlockNumberFor<T>,
        pub retry_count: u8,
    }

    /// Sell 少付证据
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct SellUnderpaidEvidence<T: Config> {
        pub sell_id: u64,
        pub tx_hash: BoundedVec<u8, ConstU32<128>>,
        pub expected_amount: u64,
        pub actual_amount: u64,
        pub shortage_percent: u8,
        pub verified_at: BlockNumberFor<T>,
    }

    /// Sell 归档 L1
    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct ArchivedSellOrder<T: Config> {
        pub sell_id: u64,
        pub maker_id: u64,
        pub user: T::AccountId,
        pub nex_amount: u64,
        pub usdt_amount: u64,
        pub status: SellOrderStatus,
        pub completed_at: u32,
    }

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // ===== Buy-side 事件 =====

        /// Buy 订单已创建
        BuyOrderCreated {
            order_id: u64,
            maker_id: u64,
            buyer: T::AccountId,
            nex_amount: BalanceOf<T>,
            is_first_purchase: bool,
        },
        /// Buy 订单状态变更
        BuyOrderStateChanged {
            order_id: u64,
            old_state: u8,
            new_state: u8,
            actor: Option<T::AccountId>,
        },
        /// 首购订单已创建
        FirstPurchaseCreated {
            order_id: u64,
            buyer: T::AccountId,
            maker_id: u64,
            usd_value: u128,
            nex_amount: BalanceOf<T>,
        },
        /// Buy 订单已自动过期
        BuyOrderAutoExpired {
            order_id: u64,
            buyer: T::AccountId,
            maker_id: u64,
            nex_amount: BalanceOf<T>,
        },
        /// 买家押金已锁定
        BuyerDepositLocked {
            order_id: u64,
            buyer: T::AccountId,
            deposit_amount: BalanceOf<T>,
        },
        /// 买家押金已释放
        BuyerDepositReleased {
            order_id: u64,
            buyer: T::AccountId,
            refund_amount: BalanceOf<T>,
        },
        /// 买家押金已没收
        BuyerDepositForfeited {
            order_id: u64,
            buyer: T::AccountId,
            maker_id: u64,
            forfeited_amount: BalanceOf<T>,
        },
        /// Buy 争议已发起
        BuyDisputeInitiated { order_id: u64, buyer: T::AccountId },
        /// Buy 争议已判定
        BuyDisputeResolved { order_id: u64, buyer_wins: bool },

        // ===== Sell-side 事件 =====

        /// Sell 订单已创建
        SellOrderCreated {
            sell_id: u64,
            maker_id: u64,
            user: T::AccountId,
            nex_amount: BalanceOf<T>,
        },
        /// 做市商已标记 Sell 完成
        SellMarkedComplete {
            sell_id: u64,
            maker_id: u64,
            trc20_tx_hash: BoundedVec<u8, ConstU32<128>>,
        },
        /// TRC20 验证已提交
        SellVerificationSubmitted {
            sell_id: u64,
            tx_hash: BoundedVec<u8, ConstU32<128>>,
        },
        /// TRC20 验证成功
        SellVerificationConfirmed {
            sell_id: u64,
            maker: T::AccountId,
        },
        /// TRC20 验证失败
        SellVerificationFailed {
            sell_id: u64,
            reason: BoundedVec<u8, ConstU32<128>>,
        },
        /// Sell 超时退款
        SellTimeout {
            sell_id: u64,
            user: T::AccountId,
            maker_id: u64,
        },
        /// Sell 手续费已收取
        SellFeeCollected {
            sell_id: u64,
            maker: T::AccountId,
            fee: BalanceOf<T>,
            net_amount: BalanceOf<T>,
        },
        /// 验证奖励已领取
        VerificationRewardClaimed {
            sell_id: u64,
            claimer: T::AccountId,
            reward: BalanceOf<T>,
        },
        /// Sell 严重少付
        SellSeverelyUnderpaid {
            sell_id: u64,
            expected_amount: u64,
            actual_amount: u64,
            shortage_percent: u8,
        },
        /// 用户接受部分 USDT
        UserAcceptedPartialUsdt {
            sell_id: u64,
            user_nex: BalanceOf<T>,
            maker_nex: BalanceOf<T>,
        },
        /// 用户要求退还 USDT
        UserRequestedUsdtRefund { sell_id: u64 },
        /// 做市商确认退还 USDT
        MakerUsdtRefundConfirmed {
            sell_id: u64,
            refund_tx_hash: BoundedVec<u8, ConstU32<128>>,
        },
        /// 做市商保证金被罚没
        MakerDepositSlashed {
            sell_id: u64,
            maker_id: u64,
            penalty_id: u64,
        },
        /// Sell 仲裁已发起
        SellDisputeFiled {
            sell_id: u64,
            user: T::AccountId,
            deposit: BalanceOf<T>,
            evidence_cid: BoundedVec<u8, ConstU32<128>>,
        },

        // ===== KYC 事件 =====

        KycEnabled { min_judgment_priority: u8 },
        KycDisabled,
        KycLevelUpdated { new_priority: u8 },
        AccountExemptedFromKyc { account: T::AccountId },
        AccountRemovedFromKycExemption { account: T::AccountId },
        KycVerificationFailed { account: T::AccountId, reason_code: u8 },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        // ===== Buy-side 错误 =====

        /// Buy 订单不存在
        BuyOrderNotFound,
        /// 做市商不存在
        MakerNotFound,
        /// 做市商未激活
        MakerNotActive,
        /// Buy 订单状态不正确
        InvalidBuyOrderStatus,
        /// 未授权
        NotAuthorized,
        /// 编码错误
        EncodingError,
        /// 存储限制已达到
        StorageLimitReached,
        /// 订单太多
        TooManyOrders,
        /// 已经首购过
        AlreadyFirstPurchased,
        /// 首购配额已用完
        FirstPurchaseQuotaExhausted,
        /// 做市商余额不足
        MakerInsufficientBalance,
        /// 做市商押金不足
        MakerDepositInsufficient,
        /// 定价不可用
        PricingUnavailable,
        /// 价格无效
        InvalidPrice,
        /// 计算溢出
        CalculationOverflow,
        /// TRON 交易哈希已使用
        TronTxHashAlreadyUsed,
        /// 订单金额超过限制
        OrderAmountExceedsLimit,
        /// 订单金额太小
        OrderAmountTooSmall,
        /// 买家押金余额不足
        InsufficientDepositBalance,
        /// 争议不存在
        DisputeNotFound,
        /// 争议状态不正确
        InvalidDisputeStatus,
        /// 不是订单买家
        NotOrderBuyer,

        // ===== Sell-side 错误 =====

        /// Sell 订单不存在
        SellOrderNotFound,
        /// Sell 状态无效
        InvalidSellStatus,
        /// 不是做市商
        NotMaker,
        /// 兑换金额太低
        SellAmountTooLow,
        /// 无效 TRON 地址
        InvalidTronAddress,
        /// 交易哈希无效
        InvalidTxHash,
        /// 不是订单用户
        NotSellUser,
        /// 金额溢出
        AmountOverflow,
        /// USDT 金额太小
        UsdtAmountTooSmall,
        /// 尚未超时
        NotYetTimeout,
        /// 验证请求不存在
        VerificationNotFound,
        /// 验证尚未超时
        VerificationNotYetTimeout,
        /// 无法发起仲裁
        CannotDispute,
        /// 托管余额不足以扣除押金
        InsufficientEscrowForDeposit,
        /// 少付证据不存在
        EvidenceNotFound,

        // ===== KYC 错误 =====

        /// 未设置身份信息
        IdentityNotSet,
        /// 没有有效的身份判断
        NoValidJudgement,
        /// KYC 认证等级不足
        InsufficientKycLevel,
        /// 身份认证质量问题
        IdentityQualityIssue,
        /// 账户已在豁免列表中
        AccountAlreadyExempted,
        /// 账户不在豁免列表中
        AccountNotExempted,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ===== Buy-side Extrinsics（原 OTC）=====

        /// 创建 Buy 订单（USDT→NEX）
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_buy_order())]
        pub fn create_buy_order(
            origin: OriginFor<T>,
            maker_id: u64,
            nex_amount: BalanceOf<T>,
            payment_commit: H256,
            contact_commit: H256,
        ) -> DispatchResult {
            let buyer = ensure_signed(origin)?;
            let _order_id = Self::do_create_buy_order(
                &buyer, maker_id, nex_amount, payment_commit, contact_commit,
            )?;
            Ok(())
        }

        /// 创建首购订单
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::create_buy_order())]
        pub fn create_first_purchase(
            origin: OriginFor<T>,
            maker_id: u64,
            payment_commit: H256,
            contact_commit: H256,
        ) -> DispatchResult {
            let buyer = ensure_signed(origin)?;
            let _order_id = Self::do_create_first_purchase(
                &buyer, maker_id, payment_commit, contact_commit,
            )?;
            Ok(())
        }

        /// 买家标记已付款
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::mark_paid())]
        pub fn mark_paid(
            origin: OriginFor<T>,
            order_id: u64,
            tron_tx_hash: Option<sp_std::vec::Vec<u8>>,
        ) -> DispatchResult {
            let buyer = ensure_signed(origin)?;
            Self::do_mark_paid(&buyer, order_id, tron_tx_hash)
        }

        /// 做市商释放 NEX 给买家
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::release_nex())]
        pub fn release_nex(
            origin: OriginFor<T>,
            order_id: u64,
        ) -> DispatchResult {
            let maker = ensure_signed(origin)?;
            Self::do_release_nex(&maker, order_id)
        }

        /// 取消 Buy 订单
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::cancel_buy_order())]
        pub fn cancel_buy_order(
            origin: OriginFor<T>,
            order_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_cancel_buy_order(&who, order_id)
        }

        /// 发起 Buy 争议
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::dispute_buy_order())]
        pub fn dispute_buy_order(
            origin: OriginFor<T>,
            order_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_dispute_buy_order(&who, order_id)
        }

        // ===== Sell-side Extrinsics（原 Swap）=====

        /// 创建 Sell 订单（NEX→USDT）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::create_sell_order())]
        pub fn create_sell_order(
            origin: OriginFor<T>,
            maker_id: u64,
            nex_amount: BalanceOf<T>,
            usdt_address: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;
            let _sell_id = Self::do_create_sell_order(
                &user, maker_id, nex_amount, usdt_address,
            )?;
            Ok(())
        }

        /// 做市商标记 Sell 已完成（提交 TRC20 tx hash）
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::mark_sell_complete())]
        pub fn mark_sell_complete(
            origin: OriginFor<T>,
            sell_id: u64,
            trc20_tx_hash: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let maker = ensure_signed(origin)?;
            Self::do_mark_sell_complete(&maker, sell_id, trc20_tx_hash)
        }

        /// 用户举报 Sell 订单
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::report_sell())]
        pub fn report_sell(
            origin: OriginFor<T>,
            sell_id: u64,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;
            Self::do_report_sell(&user, sell_id)
        }

        /// 确认 TRC20 验证结果（VerificationOrigin）
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::confirm_sell_verification())]
        pub fn confirm_sell_verification(
            origin: OriginFor<T>,
            sell_id: u64,
            verified: bool,
            reason: Option<sp_std::vec::Vec<u8>>,
        ) -> DispatchResult {
            T::VerificationOrigin::ensure_origin(origin)?;
            Self::do_confirm_sell_verification(sell_id, verified, reason)
        }

        /// 处理 Sell 验证超时
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::report_sell())]
        pub fn handle_sell_verification_timeout(
            origin: OriginFor<T>,
            sell_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            Self::do_handle_sell_verification_timeout(sell_id)
        }

        /// OCW 提交 Sell 验证结果（无签名交易）
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::mark_sell_complete())]
        pub fn ocw_submit_sell_verification(
            origin: OriginFor<T>,
            sell_id: u64,
            verified: bool,
            reason: Option<sp_std::vec::Vec<u8>>,
        ) -> DispatchResult {
            ensure_none(origin)?;
            let reason_bounded: Option<BoundedVec<u8, ConstU32<128>>> = reason
                .map(|r| r.try_into().unwrap_or_default());
            SellOcwVerificationResults::<T>::insert(sell_id, (verified, reason_bounded));
            Ok(())
        }

        /// 用户发起 Sell 仲裁
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::report_sell())]
        pub fn file_sell_dispute(
            origin: OriginFor<T>,
            sell_id: u64,
            evidence_cid: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;
            Self::do_file_sell_dispute(&user, sell_id, evidence_cid)
        }

        /// 任何人可调用确认验证（激励机制）
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::mark_sell_complete())]
        pub fn claim_sell_verification_reward(
            origin: OriginFor<T>,
            sell_id: u64,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            Self::do_claim_sell_verification_reward(&caller, sell_id)
        }

        /// 用户接受部分 USDT（SeverelyDisputed）
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::mark_sell_complete())]
        pub fn user_accept_partial_usdt(
            origin: OriginFor<T>,
            sell_id: u64,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;
            Self::do_user_accept_partial_usdt(&user, sell_id)
        }

        /// 用户要求做市商退还 USDT
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::mark_sell_complete())]
        pub fn user_request_usdt_refund(
            origin: OriginFor<T>,
            sell_id: u64,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;
            Self::do_user_request_usdt_refund(&user, sell_id)
        }

        /// 做市商确认已退还 USDT
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::mark_sell_complete())]
        pub fn maker_confirm_usdt_refund(
            origin: OriginFor<T>,
            sell_id: u64,
            refund_tx_hash: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let maker = ensure_signed(origin)?;
            Self::do_maker_confirm_usdt_refund(&maker, sell_id, refund_tx_hash)
        }

        // ===== KYC 管理 Extrinsics =====

        /// 启用 KYC 要求
        #[pallet::call_index(30)]
        #[pallet::weight(T::WeightInfo::enable_kyc())]
        pub fn enable_kyc_requirement(
            origin: OriginFor<T>,
            min_judgment_priority: u8,
        ) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();
            let config = KycConfig {
                enabled: true,
                min_judgment_priority,
                effective_block: current_block,
                updated_at: current_block,
            };
            KycConfigStore::<T>::put(config);
            Self::deposit_event(Event::KycEnabled { min_judgment_priority });
            Ok(())
        }

        /// 禁用 KYC 要求
        #[pallet::call_index(31)]
        #[pallet::weight(T::WeightInfo::disable_kyc())]
        pub fn disable_kyc_requirement(origin: OriginFor<T>) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();
            KycConfigStore::<T>::mutate(|config| {
                config.enabled = false;
                config.effective_block = current_block;
                config.updated_at = current_block;
            });
            Self::deposit_event(Event::KycDisabled);
            Ok(())
        }

        /// 更新最低认证等级
        #[pallet::call_index(32)]
        #[pallet::weight(T::WeightInfo::enable_kyc())]
        pub fn update_min_judgment_level(
            origin: OriginFor<T>,
            new_priority: u8,
        ) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();
            KycConfigStore::<T>::mutate(|config| {
                config.min_judgment_priority = new_priority;
                config.effective_block = current_block;
                config.updated_at = current_block;
            });
            Self::deposit_event(Event::KycLevelUpdated { new_priority });
            Ok(())
        }

        /// 添加 KYC 豁免账户
        #[pallet::call_index(33)]
        #[pallet::weight(T::WeightInfo::enable_kyc())]
        pub fn exempt_account_from_kyc(
            origin: OriginFor<T>,
            account: T::AccountId,
        ) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            ensure!(
                !KycExemptAccounts::<T>::contains_key(&account),
                Error::<T>::AccountAlreadyExempted
            );
            KycExemptAccounts::<T>::insert(&account, ());
            Self::deposit_event(Event::AccountExemptedFromKyc { account });
            Ok(())
        }

        /// 移除 KYC 豁免账户
        #[pallet::call_index(34)]
        #[pallet::weight(T::WeightInfo::disable_kyc())]
        pub fn remove_kyc_exemption(
            origin: OriginFor<T>,
            account: T::AccountId,
        ) -> DispatchResult {
            T::CommitteeOrigin::ensure_origin(origin)?;
            ensure!(
                KycExemptAccounts::<T>::contains_key(&account),
                Error::<T>::AccountNotExempted
            );
            KycExemptAccounts::<T>::remove(&account);
            Self::deposit_event(Event::AccountRemovedFromKycExemption { account });
            Ok(())
        }
    }

    // ========================================================================
    // OCW 无签名交易验证（Sell 侧）
    // ========================================================================

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::ocw_submit_sell_verification { sell_id, verified: _, reason } => {
                    match source {
                        TransactionSource::Local | TransactionSource::InBlock => {},
                        TransactionSource::External => {
                            log::warn!(target: "p2p-ocw", "External unsigned tx for sell {}", sell_id);
                        }
                    }

                    let record = match SellOrders::<T>::get(sell_id) {
                        Some(r) => r,
                        None => return InvalidTransaction::Custom(1).into(),
                    };
                    if record.status != SellOrderStatus::AwaitingVerification {
                        return InvalidTransaction::Custom(2).into();
                    }
                    if !SellPendingVerifications::<T>::contains_key(sell_id) {
                        return InvalidTransaction::Custom(3).into();
                    }
                    if let Some(ref r) = reason {
                        if r.len() > 256 {
                            return InvalidTransaction::Custom(4).into();
                        }
                    }

                    let priority = match source {
                        TransactionSource::Local => 100,
                        TransactionSource::InBlock => 80,
                        TransactionSource::External => 50,
                    };

                    ValidTransaction::with_tag_prefix("P2pTRC20Verify")
                        .priority(priority)
                        .longevity(10)
                        .and_provides([&(b"p2p_verify", sell_id)])
                        .propagate(true)
                        .build()
                },
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    // ========================================================================
    // Internal — Helpers
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// Pallet 内部账户
        pub fn pallet_account_id() -> T::AccountId {
            PALLET_ID.into_account_truncating()
        }

        /// KYC 验证
        pub fn enforce_kyc_requirement(who: &T::AccountId) -> DispatchResult {
            let config = KycConfigStore::<T>::get();
            if !config.enabled {
                return Ok(());
            }
            if KycExemptAccounts::<T>::contains_key(who) {
                return Ok(());
            }
            let priority = T::IdentityProvider::get_highest_judgement_priority(who)
                .ok_or(Error::<T>::IdentityNotSet)?;
            if T::IdentityProvider::has_problematic_judgement(who) {
                Self::deposit_event(Event::KycVerificationFailed {
                    account: who.clone(),
                    reason_code: KycFailureReason::QualityIssue.to_code(),
                });
                return Err(Error::<T>::IdentityQualityIssue.into());
            }
            if priority < config.min_judgment_priority {
                Self::deposit_event(Event::KycVerificationFailed {
                    account: who.clone(),
                    reason_code: KycFailureReason::InsufficientLevel.to_code(),
                });
                return Err(Error::<T>::InsufficientKycLevel.into());
            }
            Ok(())
        }

        /// Buy 侧: 计算 USD 金额
        pub fn calculate_usd_amount_from_nex(
            nex_amount: BalanceOf<T>,
            price: BalanceOf<T>,
        ) -> Result<u64, DispatchError> {
            let nex_u128: u128 = nex_amount.saturated_into();
            let price_u128: u128 = price.saturated_into();
            let usd = nex_u128
                .checked_mul(price_u128)
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_div(1_000_000_000_000u128) // NEX 精度
                .ok_or(Error::<T>::CalculationOverflow)?;
            Ok(usd as u64)
        }

        /// Buy 侧: 计算买家押金
        pub fn calculate_buyer_deposit(
            buyer: &T::AccountId,
            nex_amount: BalanceOf<T>,
        ) -> BalanceOf<T> {
            // 首购免押金
            if !HasFirstPurchased::<T>::get(buyer) {
                return BalanceOf::<T>::from(0u32);
            }
            // 信用用户减免
            let completed = BuyerCompletedOrderCount::<T>::get(buyer);
            if completed >= 10 {
                return BalanceOf::<T>::from(0u32);
            }
            // 按比例计算
            let rate = T::DepositRate::get() as u128;
            let amount: u128 = nex_amount.saturated_into();
            let deposit = amount * rate / 10000;
            let deposit_balance: BalanceOf<T> = deposit.saturated_into();
            let min = T::MinDeposit::get();
            if deposit_balance < min { min } else { deposit_balance }
        }

        /// Buy 侧: 锁定买家押金
        pub fn lock_buyer_deposit(
            buyer: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            // 从买家转入 Pallet 账户
            let pallet_account = Self::pallet_account_id();
            <T::Currency as Currency<T::AccountId>>::transfer(
                buyer,
                &pallet_account,
                amount,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;
            TotalDepositPoolBalance::<T>::mutate(|total| {
                *total = total.saturating_add(amount);
            });
            Ok(())
        }

        /// Buy 侧: 验证订单金额
        pub fn validate_buy_order_amount(
            nex_amount: BalanceOf<T>,
            is_first_purchase: bool,
        ) -> Result<u64, DispatchError> {
            let price = T::Pricing::get_cos_to_usd_rate()
                .ok_or(Error::<T>::PricingUnavailable)?;
            let usd_amount = Self::calculate_usd_amount_from_nex(nex_amount, price)?;

            if is_first_purchase {
                // 首购使用固定金额
            } else {
                ensure!(
                    usd_amount >= T::MinOrderUsdAmount::get(),
                    Error::<T>::OrderAmountTooSmall
                );
                ensure!(
                    usd_amount <= T::MaxOrderUsdAmount::get(),
                    Error::<T>::OrderAmountExceedsLimit
                );
            }
            Ok(usd_amount)
        }

        // ===== Placeholder 内部实现（下一步填充）=====

        pub fn do_create_buy_order(
            buyer: &T::AccountId,
            maker_id: u64,
            nex_amount: BalanceOf<T>,
            payment_commit: H256,
            contact_commit: H256,
        ) -> Result<u64, DispatchError> {
            use pallet_trading_credit::quota::BuyerQuotaInterface;

            // KYC 验证
            Self::enforce_kyc_requirement(buyer)?;

            // 验证金额
            let _usd_amount = Self::validate_buy_order_amount(nex_amount, false)?;

            // 验证做市商
            let maker_app = T::MakerPallet::validate_maker(maker_id)
                .map_err(|e| match e {
                    MakerValidationError::NotFound => Error::<T>::MakerNotFound,
                    MakerValidationError::NotActive => Error::<T>::MakerNotActive,
                })?;

            // 验证做市商押金
            let min_deposit_usd = T::MinMakerDepositUsd::get();
            let maker_deposit_usd = T::MakerPallet::get_deposit_usd_value(maker_id).unwrap_or(0);
            ensure!(maker_deposit_usd >= min_deposit_usd, Error::<T>::MakerDepositInsufficient);

            // 获取价格
            let price = T::Pricing::get_cos_to_usd_rate()
                .ok_or(Error::<T>::PricingUnavailable)?;

            // 计算 USD 金额（精度 10^6）
            let amount_usd = Self::calculate_usd_amount_from_nex(nex_amount, price)?;
            let amount: BalanceOf<T> = (amount_usd as u128).saturated_into();

            // 额度检查和占用
            T::BuyerCredit::occupy_quota(buyer, amount_usd)?;

            // TRON 地址
            let maker_tron_address = maker_app.tron_address
                .try_into().map_err(|_| Error::<T>::EncodingError)?;

            let order_id = NextBuyOrderId::<T>::get();

            // 锁定做市商 NEX 到托管
            T::Escrow::lock_from(&maker_app.account, order_id, nex_amount)?;

            // 计算买家押金
            let buyer_deposit = Self::calculate_buyer_deposit(buyer, nex_amount);
            let deposit_status = if buyer_deposit.is_zero() {
                DepositStatus::None
            } else {
                Self::lock_buyer_deposit(buyer, buyer_deposit)?;
                DepositStatus::Locked
            };

            // 时间
            let now = T::Timestamp::now().as_secs().saturated_into::<u64>();
            let expire_at = now.checked_add(T::BuyOrderTimeout::get())
                .ok_or(Error::<T>::CalculationOverflow)?;
            let evidence_until = now.checked_add(T::EvidenceWindow::get())
                .ok_or(Error::<T>::CalculationOverflow)?;

            let order = BuyOrder {
                maker_id,
                maker: maker_app.account.clone(),
                taker: buyer.clone(),
                price,
                qty: nex_amount,
                amount,
                created_at: now,
                expire_at,
                evidence_until,
                maker_tron_address,
                payment_commit,
                contact_commit,
                state: BuyOrderState::Created,
                completed_at: None,
                is_first_purchase: false,
                buyer_deposit,
                deposit_status,
            };

            BuyOrders::<T>::insert(order_id, order);
            NextBuyOrderId::<T>::put(order_id + 1);

            BuyerOrderList::<T>::try_mutate(buyer, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::TooManyOrders)
            })?;
            MakerBuyOrderList::<T>::try_mutate(maker_id, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::TooManyOrders)
            })?;

            Self::deposit_event(Event::BuyOrderCreated {
                order_id,
                maker_id,
                buyer: buyer.clone(),
                nex_amount,
                is_first_purchase: false,
            });

            Ok(order_id)
        }

        pub fn do_create_first_purchase(
            buyer: &T::AccountId,
            maker_id: u64,
            payment_commit: H256,
            contact_commit: H256,
        ) -> Result<u64, DispatchError> {
            // KYC 验证
            Self::enforce_kyc_requirement(buyer)?;

            // 检查买家是否已首购
            ensure!(
                !HasFirstPurchased::<T>::get(buyer),
                Error::<T>::AlreadyFirstPurchased
            );

            // 验证做市商
            let maker_app = T::MakerPallet::validate_maker(maker_id)
                .map_err(|e| match e {
                    MakerValidationError::NotFound => Error::<T>::MakerNotFound,
                    MakerValidationError::NotActive => Error::<T>::MakerNotActive,
                })?;

            // 检查做市商首购配额
            let current_count = MakerFirstPurchaseCount::<T>::get(maker_id);
            ensure!(
                current_count < T::MaxFirstPurchaseOrdersPerMaker::get(),
                Error::<T>::FirstPurchaseQuotaExhausted
            );

            // 获取价格并计算 NEX 数量
            let price = T::Pricing::get_cos_to_usd_rate()
                .ok_or(Error::<T>::PricingUnavailable)?;
            let usd_value = T::FirstPurchaseUsdValue::get();
            let price_u128: u128 = price.saturated_into();
            ensure!(price_u128 > 0, Error::<T>::InvalidPrice);

            let nex_amount_u128 = usd_value
                .checked_mul(1_000_000_000_000u128)
                .and_then(|v| v.checked_div(price_u128))
                .ok_or(Error::<T>::CalculationOverflow)?;
            let nex_amount: BalanceOf<T> = nex_amount_u128.saturated_into();

            ensure!(
                nex_amount >= T::MinFirstPurchaseCosAmount::get(),
                Error::<T>::InvalidPrice
            );
            ensure!(
                nex_amount <= T::MaxFirstPurchaseCosAmount::get(),
                Error::<T>::InvalidPrice
            );

            // 验证做市商余额
            let maker_balance = T::Currency::free_balance(&maker_app.account);
            ensure!(maker_balance >= nex_amount, Error::<T>::MakerInsufficientBalance);

            // TRON 地址
            let maker_tron_address: TronAddress = maker_app.tron_address
                .try_into().map_err(|_| Error::<T>::EncodingError)?;

            let order_id = NextBuyOrderId::<T>::get();

            // 锁定做市商 NEX 到托管
            T::Escrow::lock_from(&maker_app.account, order_id, nex_amount)?;

            // 时间
            let now = T::Timestamp::now().as_secs().saturated_into::<u64>();
            let expire_at = now.checked_add(T::BuyOrderTimeout::get())
                .ok_or(Error::<T>::CalculationOverflow)?;
            let evidence_until = now.checked_add(T::EvidenceWindow::get())
                .ok_or(Error::<T>::CalculationOverflow)?;

            // 首购 USD 金额（usd_value 精度 10^6，如 10_000_000 = 10 USD）
            let amount: BalanceOf<T> = usd_value.saturated_into();

            let order = BuyOrder {
                maker_id,
                maker: maker_app.account.clone(),
                taker: buyer.clone(),
                price,
                qty: nex_amount,
                amount,
                created_at: now,
                expire_at,
                evidence_until,
                maker_tron_address,
                payment_commit,
                contact_commit,
                state: BuyOrderState::Created,
                completed_at: None,
                is_first_purchase: true,
                buyer_deposit: BalanceOf::<T>::from(0u32),
                deposit_status: DepositStatus::None,
            };

            BuyOrders::<T>::insert(order_id, order);
            NextBuyOrderId::<T>::put(order_id + 1);

            BuyerOrderList::<T>::try_mutate(buyer, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::TooManyOrders)
            })?;
            MakerBuyOrderList::<T>::try_mutate(maker_id, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::TooManyOrders)
            })?;

            // 更新做市商首购计数和列表
            MakerFirstPurchaseCount::<T>::mutate(maker_id, |count| {
                *count = count.saturating_add(1);
            });
            MakerFirstPurchaseOrders::<T>::try_mutate(maker_id, |orders| {
                orders.try_push(order_id).map_err(|_| Error::<T>::StorageLimitReached)
            })?;

            Self::deposit_event(Event::FirstPurchaseCreated {
                order_id,
                buyer: buyer.clone(),
                maker_id,
                usd_value,
                nex_amount,
            });

            Ok(order_id)
        }

        pub fn do_mark_paid(
            buyer: &T::AccountId,
            order_id: u64,
            tron_tx_hash: Option<sp_std::vec::Vec<u8>>,
        ) -> DispatchResult {
            let mut order = BuyOrders::<T>::get(order_id)
                .ok_or(Error::<T>::BuyOrderNotFound)?;

            ensure!(
                order.state == BuyOrderState::Created,
                Error::<T>::InvalidBuyOrderStatus
            );
            ensure!(order.taker == *buyer, Error::<T>::NotAuthorized);

            // 如提供 TRON 交易哈希，验证并记录
            if let Some(tx_hash_vec) = tron_tx_hash {
                ensure!(tx_hash_vec.len() == 32, Error::<T>::EncodingError);
                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&tx_hash_vec);
                let tx_hash = H256::from(hash_bytes);

                ensure!(
                    !BuyTronTxUsed::<T>::contains_key(tx_hash),
                    Error::<T>::TronTxHashAlreadyUsed
                );
                let current_block = frame_system::Pallet::<T>::block_number();
                BuyTronTxUsed::<T>::insert(tx_hash, current_block);
            }

            let old_state = Self::buy_state_to_u8(&order.state);
            order.state = BuyOrderState::PaidOrCommitted;
            BuyOrders::<T>::insert(order_id, order);

            Self::deposit_event(Event::BuyOrderStateChanged {
                order_id,
                old_state,
                new_state: Self::buy_state_to_u8(&BuyOrderState::PaidOrCommitted),
                actor: Some(buyer.clone()),
            });

            Ok(())
        }

        pub fn do_release_nex(
            maker: &T::AccountId,
            order_id: u64,
        ) -> DispatchResult {
            use pallet_trading_credit::quota::BuyerQuotaInterface;

            let mut order = BuyOrders::<T>::get(order_id)
                .ok_or(Error::<T>::BuyOrderNotFound)?;

            ensure!(
                order.state == BuyOrderState::PaidOrCommitted,
                Error::<T>::InvalidBuyOrderStatus
            );
            ensure!(order.maker == *maker, Error::<T>::NotAuthorized);

            // 从托管释放 NEX 到买家
            T::Escrow::release_all(order_id, &order.taker)?;

            let old_state = Self::buy_state_to_u8(&order.state);
            order.state = BuyOrderState::Released;
            let now = T::Timestamp::now().as_secs().saturated_into::<u64>();
            order.completed_at = Some(now);
            BuyOrders::<T>::insert(order_id, order.clone());

            // 记录做市商信用
            let response_time = now.saturating_sub(order.created_at) as u32;
            let _ = T::MakerCredit::record_maker_order_completed(
                order.maker_id, order_id, response_time,
            );

            // 释放买家额度
            let amount_usd = Self::calculate_usd_amount_from_nex(order.qty, order.price)?;
            let _ = T::BuyerCredit::release_quota(&order.taker, amount_usd);
            let _ = T::BuyerCredit::record_order_completed(&order.taker, order_id);

            // 如是首购，更新首购状态
            if order.is_first_purchase {
                HasFirstPurchased::<T>::insert(&order.taker, true);
                MakerFirstPurchaseCount::<T>::mutate(order.maker_id, |count| {
                    *count = count.saturating_sub(1);
                });
            }

            // 退还买家押金
            if !order.buyer_deposit.is_zero() {
                let _ = Self::release_buyer_deposit(&order.taker, order.buyer_deposit);
                BuyOrders::<T>::mutate(order_id, |o| {
                    if let Some(ord) = o { ord.deposit_status = DepositStatus::Released; }
                });
                Self::deposit_event(Event::BuyerDepositReleased {
                    order_id,
                    buyer: order.taker.clone(),
                    refund_amount: order.buyer_deposit,
                });
            }

            // 更新完成计数
            BuyerCompletedOrderCount::<T>::mutate(&order.taker, |count| {
                *count = count.saturating_add(1);
            });

            Self::deposit_event(Event::BuyOrderStateChanged {
                order_id,
                old_state,
                new_state: Self::buy_state_to_u8(&BuyOrderState::Released),
                actor: Some(maker.clone()),
            });

            Ok(())
        }

        pub fn do_cancel_buy_order(
            who: &T::AccountId,
            order_id: u64,
        ) -> DispatchResult {
            use pallet_trading_credit::quota::BuyerQuotaInterface;

            let mut order = BuyOrders::<T>::get(order_id)
                .ok_or(Error::<T>::BuyOrderNotFound)?;

            ensure!(
                order.taker == *who || order.maker == *who,
                Error::<T>::NotAuthorized
            );
            ensure!(
                order.state == BuyOrderState::Created || order.state == BuyOrderState::Expired,
                Error::<T>::InvalidBuyOrderStatus
            );

            // 退还托管 NEX 给做市商
            T::Escrow::refund_all(order_id, &order.maker)?;

            let old_state = Self::buy_state_to_u8(&order.state);
            order.state = BuyOrderState::Canceled;
            let now = T::Timestamp::now().as_secs().saturated_into::<u64>();
            order.completed_at = Some(now);
            BuyOrders::<T>::insert(order_id, order.clone());

            // 释放买家额度
            let amount_usd = Self::calculate_usd_amount_from_nex(order.qty, order.price)?;
            let _ = T::BuyerCredit::release_quota(&order.taker, amount_usd);
            let _ = T::BuyerCredit::record_order_cancelled(&order.taker, order_id);

            // 如是首购，减少计数
            if order.is_first_purchase {
                MakerFirstPurchaseCount::<T>::mutate(order.maker_id, |count| {
                    *count = count.saturating_sub(1);
                });
            }

            // 处理买家押金
            if !order.buyer_deposit.is_zero() {
                let is_buyer_cancel = order.taker == *who;
                if is_buyer_cancel {
                    // 买家取消：部分没收
                    let penalty_rate: BalanceOf<T> = T::CancelPenaltyRate::get().into();
                    let divisor: BalanceOf<T> = 10000u32.into();
                    let penalty = order.buyer_deposit * penalty_rate / divisor;
                    let refund = order.buyer_deposit.saturating_sub(penalty);

                    if !penalty.is_zero() {
                        let _ = Self::forfeit_buyer_deposit(&order.maker, penalty);
                    }
                    if !refund.is_zero() {
                        let _ = Self::release_buyer_deposit(&order.taker, refund);
                    }
                    BuyOrders::<T>::mutate(order_id, |o| {
                        if let Some(ord) = o { ord.deposit_status = DepositStatus::PartiallyForfeited; }
                    });
                    Self::deposit_event(Event::BuyerDepositForfeited {
                        order_id,
                        buyer: order.taker.clone(),
                        maker_id: order.maker_id,
                        forfeited_amount: penalty,
                    });
                } else {
                    // 做市商取消：100% 退还
                    let _ = Self::release_buyer_deposit(&order.taker, order.buyer_deposit);
                    BuyOrders::<T>::mutate(order_id, |o| {
                        if let Some(ord) = o { ord.deposit_status = DepositStatus::Released; }
                    });
                    Self::deposit_event(Event::BuyerDepositReleased {
                        order_id,
                        buyer: order.taker.clone(),
                        refund_amount: order.buyer_deposit,
                    });
                }
            }

            Self::deposit_event(Event::BuyOrderStateChanged {
                order_id,
                old_state,
                new_state: Self::buy_state_to_u8(&BuyOrderState::Canceled),
                actor: Some(who.clone()),
            });

            Ok(())
        }

        pub fn do_dispute_buy_order(
            who: &T::AccountId,
            order_id: u64,
        ) -> DispatchResult {
            let mut order = BuyOrders::<T>::get(order_id)
                .ok_or(Error::<T>::BuyOrderNotFound)?;

            ensure!(
                order.taker == *who || order.maker == *who,
                Error::<T>::NotAuthorized
            );
            ensure!(
                order.state == BuyOrderState::PaidOrCommitted,
                Error::<T>::InvalidBuyOrderStatus
            );
            ensure!(
                !BuyDisputes::<T>::contains_key(order_id),
                Error::<T>::InvalidDisputeStatus
            );

            let now = T::Timestamp::now().as_secs().saturated_into::<u64>();
            let response_deadline = now + T::DisputeResponseTimeout::get();
            let arbitration_deadline = now + T::DisputeArbitrationTimeout::get();

            let (initiator, respondent) = if order.taker == *who {
                (order.taker.clone(), order.maker.clone())
            } else {
                (order.maker.clone(), order.taker.clone())
            };

            let dispute = BuyDispute {
                order_id,
                initiator: initiator.clone(),
                respondent,
                created_at: now,
                response_deadline,
                arbitration_deadline,
                status: BuyDisputeStatus::WaitingMakerResponse,
                buyer_evidence: None,
                maker_evidence: None,
            };
            BuyDisputes::<T>::insert(order_id, dispute);

            let old_state = Self::buy_state_to_u8(&order.state);
            order.state = BuyOrderState::Disputed;
            BuyOrders::<T>::insert(order_id, order.clone());

            Self::deposit_event(Event::BuyOrderStateChanged {
                order_id,
                old_state,
                new_state: Self::buy_state_to_u8(&BuyOrderState::Disputed),
                actor: Some(who.clone()),
            });
            Self::deposit_event(Event::BuyDisputeInitiated {
                order_id,
                buyer: order.taker,
            });

            Ok(())
        }

        pub fn do_create_sell_order(
            user: &T::AccountId,
            maker_id: u64,
            nex_amount: BalanceOf<T>,
            usdt_address: sp_std::vec::Vec<u8>,
        ) -> Result<u64, DispatchError> {
            // 验证最小金额
            ensure!(nex_amount >= T::MinSellAmount::get(), Error::<T>::SellAmountTooLow);

            // 验证做市商
            let maker_app = T::MakerPallet::validate_maker(maker_id)
                .map_err(|e| match e {
                    MakerValidationError::NotFound => Error::<T>::MakerNotFound,
                    MakerValidationError::NotActive => Error::<T>::MakerNotActive,
                })?;

            // 验证 USDT 地址
            let usdt_addr: TronAddress = usdt_address
                .try_into().map_err(|_| Error::<T>::InvalidTronAddress)?;

            // 获取价格并计算 USDT 金额
            let price_balance = T::Pricing::get_cos_to_usd_rate()
                .ok_or(Error::<T>::PricingUnavailable)?;
            let price_usdt: u64 = price_balance.saturated_into();

            let nex_u128: u128 = nex_amount.saturated_into();
            let usdt_amount_u128 = nex_u128
                .checked_mul(price_usdt as u128)
                .ok_or(Error::<T>::AmountOverflow)?
                .checked_div(1_000_000_000_000u128)
                .ok_or(Error::<T>::AmountOverflow)?;
            ensure!(usdt_amount_u128 >= 1_000_000, Error::<T>::UsdtAmountTooSmall);
            let usdt_amount = usdt_amount_u128 as u64;

            let sell_id = NextSellOrderId::<T>::get();

            // 锁定用户 NEX 到托管
            T::Escrow::lock_from(user, sell_id, nex_amount)?;

            let current_block = frame_system::Pallet::<T>::block_number();
            let timeout_at = current_block + T::SellTimeoutBlocks::get();

            let record = SellOrder {
                sell_id,
                maker_id,
                maker: maker_app.account,
                user: user.clone(),
                nex_amount,
                usdt_amount,
                usdt_address: usdt_addr,
                created_at: current_block,
                timeout_at,
                trc20_tx_hash: None,
                completed_at: None,
                evidence_cid: None,
                status: SellOrderStatus::Pending,
                price_usdt,
                dispute_deposit: None,
            };

            SellOrders::<T>::insert(sell_id, record);
            NextSellOrderId::<T>::put(sell_id + 1);

            UserSellList::<T>::try_mutate(user, |list| {
                list.try_push(sell_id).map_err(|_| Error::<T>::TooManyOrders)
            })?;
            MakerSellList::<T>::try_mutate(maker_id, |list| {
                list.try_push(sell_id).map_err(|_| Error::<T>::TooManyOrders)
            })?;

            Self::deposit_event(Event::SellOrderCreated {
                sell_id,
                user: user.clone(),
                maker_id,
                nex_amount,
            });

            Ok(sell_id)
        }

        pub fn do_mark_sell_complete(
            maker: &T::AccountId,
            sell_id: u64,
            trc20_tx_hash: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;
            ensure!(record.maker == *maker, Error::<T>::NotMaker);
            ensure!(record.status == SellOrderStatus::Pending, Error::<T>::InvalidSellStatus);

            let tx_hash: BoundedVec<u8, ConstU32<128>> = trc20_tx_hash
                .try_into().map_err(|_| Error::<T>::InvalidTxHash)?;
            ensure!(
                !SellUsedTxHashes::<T>::contains_key(&tx_hash),
                Error::<T>::TronTxHashAlreadyUsed
            );

            let current_block = frame_system::Pallet::<T>::block_number();
            SellUsedTxHashes::<T>::insert(&tx_hash, current_block);

            record.trc20_tx_hash = Some(tx_hash.clone());
            record.status = SellOrderStatus::AwaitingVerification;
            SellOrders::<T>::insert(sell_id, record.clone());

            let verification_timeout_at = current_block + T::VerificationTimeoutBlocks::get();
            let verification_request = SellVerificationRequest {
                sell_id,
                tx_hash: tx_hash.clone(),
                expected_to: record.usdt_address.clone(),
                expected_amount: record.usdt_amount,
                submitted_at: current_block,
                verification_timeout_at,
                retry_count: 0,
            };
            SellPendingVerifications::<T>::insert(sell_id, verification_request);

            Self::deposit_event(Event::SellVerificationSubmitted { sell_id, tx_hash });
            Ok(())
        }

        pub fn do_report_sell(
            user: &T::AccountId,
            sell_id: u64,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;

            ensure!(record.user == *user, Error::<T>::NotSellUser);
            // 只允许在 Pending 状态举报（Completed 是终态，不可回退）
            ensure!(
                record.status == SellOrderStatus::Pending,
                Error::<T>::InvalidSellStatus
            );

            record.status = SellOrderStatus::UserReported;
            SellOrders::<T>::insert(sell_id, record);

            Ok(())
        }

        pub fn do_confirm_sell_verification(
            sell_id: u64,
            verified: bool,
            reason: Option<sp_std::vec::Vec<u8>>,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;
            ensure!(
                record.status == SellOrderStatus::AwaitingVerification,
                Error::<T>::InvalidSellStatus
            );

            SellPendingVerifications::<T>::remove(sell_id);
            let current_block = frame_system::Pallet::<T>::block_number();

            if verified {
                // 计算手续费
                let fee_by_rate = record.nex_amount
                    .saturating_mul(T::SellFeeRateBps::get().into()) / 10000u32.into();
                let min_fee = T::MinSellFee::get();
                let fee = if fee_by_rate > min_fee { fee_by_rate } else { min_fee };
                let fee = if fee > record.nex_amount { record.nex_amount } else { fee };
                let net_amount = record.nex_amount.saturating_sub(fee);

                if net_amount > BalanceOf::<T>::from(0u32) {
                    T::Escrow::transfer_from_escrow(sell_id, &record.maker, net_amount)?;
                }
                if fee > BalanceOf::<T>::from(0u32) {
                    let pallet_account = Self::pallet_account_id();
                    T::Escrow::transfer_from_escrow(sell_id, &pallet_account, fee)?;
                }

                record.status = SellOrderStatus::Completed;
                record.completed_at = Some(current_block);
                SellOrders::<T>::insert(sell_id, record.clone());

                // 记录信用
                let block_duration = current_block.saturating_sub(record.created_at);
                let response_time = (block_duration.saturated_into::<u64>() * 6) as u32;
                let _ = T::MakerCredit::record_maker_order_completed(
                    record.maker_id, sell_id, response_time,
                );

                // 上报交易数据
                let timestamp = current_block.saturated_into::<u64>() * 6000;
                let nex_qty: u128 = record.nex_amount.saturated_into();
                let _ = T::Pricing::report_p2p_trade(timestamp, record.price_usdt, nex_qty);

                Self::deposit_event(Event::SellFeeCollected {
                    sell_id,
                    maker: record.maker.clone(),
                    fee,
                    net_amount,
                });
                Self::deposit_event(Event::SellVerificationConfirmed {
                    sell_id,
                    maker: record.maker,
                });
            } else {
                // 验证失败
                record.status = SellOrderStatus::VerificationFailed;
                SellOrders::<T>::insert(sell_id, record);

                let reason_bounded: BoundedVec<u8, ConstU32<128>> = reason
                    .unwrap_or_else(|| b"Unknown verification failure".to_vec())
                    .try_into()
                    .unwrap_or_else(|_| BoundedVec::default());
                Self::deposit_event(Event::SellVerificationFailed {
                    sell_id,
                    reason: reason_bounded,
                });
            }
            Ok(())
        }

        pub fn do_handle_sell_verification_timeout(sell_id: u64) -> DispatchResult {
            let request = SellPendingVerifications::<T>::get(sell_id)
                .ok_or(Error::<T>::VerificationNotFound)?;

            let current_block = frame_system::Pallet::<T>::block_number();
            ensure!(
                current_block >= request.verification_timeout_at,
                Error::<T>::VerificationNotYetTimeout
            );

            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;
            ensure!(
                record.status == SellOrderStatus::AwaitingVerification,
                Error::<T>::InvalidSellStatus
            );

            SellPendingVerifications::<T>::remove(sell_id);

            // 超时自动退款给用户
            let refund_result = T::Escrow::refund_all(sell_id, &record.user);
            if refund_result.is_ok() {
                record.status = SellOrderStatus::Refunded;
                let _ = T::MakerCredit::record_maker_order_timeout(record.maker_id, sell_id);
            } else {
                record.status = SellOrderStatus::Arbitrating;
            }
            record.completed_at = Some(current_block);
            SellOrders::<T>::insert(sell_id, record.clone());

            Self::deposit_event(Event::SellTimeout {
                sell_id,
                user: record.user,
                maker_id: record.maker_id,
            });

            Ok(())
        }

        pub fn do_file_sell_dispute(
            user: &T::AccountId,
            sell_id: u64,
            evidence_cid: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;

            ensure!(record.user == *user, Error::<T>::NotSellUser);
            ensure!(
                record.status == SellOrderStatus::VerificationFailed
                    || record.status == SellOrderStatus::AwaitingVerification,
                Error::<T>::CannotDispute
            );

            // 如果是 AwaitingVerification，检查是否已超时
            if record.status == SellOrderStatus::AwaitingVerification {
                if let Some(request) = SellPendingVerifications::<T>::get(sell_id) {
                    let current_block = frame_system::Pallet::<T>::block_number();
                    ensure!(
                        current_block >= request.verification_timeout_at,
                        Error::<T>::VerificationNotYetTimeout
                    );
                }
            }

            // 计算押金（托管的 1%，最低 1 NEX）
            let escrow_balance = T::Escrow::amount_of(sell_id);
            let one_percent = escrow_balance / 100u32.into();
            let min_deposit: BalanceOf<T> = 1_000_000_000_000u128.saturated_into();
            let deposit_amount = if one_percent > min_deposit { one_percent } else { min_deposit };
            ensure!(escrow_balance > deposit_amount, Error::<T>::InsufficientEscrowForDeposit);

            // 从托管扣除押金到 Pallet 账户
            let pallet_account = Self::pallet_account_id();
            T::Escrow::transfer_from_escrow(sell_id, &pallet_account, deposit_amount)
                .map_err(|_| Error::<T>::InsufficientEscrowForDeposit)?;

            record.status = SellOrderStatus::Arbitrating;
            record.dispute_deposit = Some(deposit_amount);
            SellOrders::<T>::insert(sell_id, record);

            SellPendingVerifications::<T>::remove(sell_id);

            Self::deposit_event(Event::SellDisputeFiled {
                sell_id,
                user: user.clone(),
                deposit: deposit_amount,
                evidence_cid: evidence_cid.try_into().unwrap_or_default(),
            });

            Ok(())
        }

        pub fn do_claim_sell_verification_reward(
            caller: &T::AccountId,
            sell_id: u64,
        ) -> DispatchResult {
            let record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;
            ensure!(
                record.status == SellOrderStatus::AwaitingVerification,
                Error::<T>::InvalidSellStatus
            );

            // 从链上存储读取 OCW 验证结果
            let (verified, reason_bounded) = SellOcwVerificationResults::<T>::get(sell_id)
                .ok_or(Error::<T>::VerificationNotFound)?;
            let reason = reason_bounded.map(|r| r.to_vec());

            // 执行验证确认
            Self::do_confirm_sell_verification(sell_id, verified, reason)?;

            // 清理 OCW 验证结果
            SellOcwVerificationResults::<T>::remove(sell_id);

            // 支付奖励
            let reward = T::VerificationReward::get();
            if reward > BalanceOf::<T>::from(0u32) {
                let pallet_account = Self::pallet_account_id();
                let _ = T::Currency::transfer(
                    &pallet_account,
                    caller,
                    reward,
                    frame_support::traits::ExistenceRequirement::KeepAlive,
                );
            }

            Self::deposit_event(Event::VerificationRewardClaimed {
                sell_id,
                claimer: caller.clone(),
                reward,
            });

            Ok(())
        }

        pub fn do_user_accept_partial_usdt(
            user: &T::AccountId,
            sell_id: u64,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;

            ensure!(&record.user == user, Error::<T>::NotSellUser);
            ensure!(
                record.status == SellOrderStatus::SeverelyDisputed,
                Error::<T>::InvalidSellStatus
            );

            let evidence = SellUnderpaidEvidences::<T>::get(sell_id)
                .ok_or(Error::<T>::EvidenceNotFound)?;

            // 按比例分配 NEX
            let maker_ratio = if evidence.expected_amount > 0 {
                evidence.actual_amount * 10000 / evidence.expected_amount
            } else {
                0
            };
            let maker_nex = record.nex_amount
                .saturating_mul(BalanceOf::<T>::from(maker_ratio as u32))
                / BalanceOf::<T>::from(10000u32);
            let user_nex = record.nex_amount.saturating_sub(maker_nex);

            if maker_nex > BalanceOf::<T>::from(0u32) {
                T::Escrow::transfer_from_escrow(sell_id, &record.maker, maker_nex)?;
            }
            if user_nex > BalanceOf::<T>::from(0u32) {
                T::Escrow::transfer_from_escrow(sell_id, user, user_nex)?;
            }

            // 罚没做市商保证金
            let penalty_result = T::MakerPallet::slash_deposit_for_severely_underpaid(
                record.maker_id, sell_id,
                evidence.expected_amount, evidence.actual_amount,
                1000, // 10% = 1000 bps
            );
            let penalty_id = penalty_result.ok();
            let maker_id = record.maker_id;

            record.status = SellOrderStatus::Completed;
            record.completed_at = Some(frame_system::Pallet::<T>::block_number());
            SellOrders::<T>::insert(sell_id, record);
            SellUnderpaidEvidences::<T>::remove(sell_id);

            Self::deposit_event(Event::UserAcceptedPartialUsdt {
                sell_id,
                user_nex,
                maker_nex,
            });
            if let Some(pid) = penalty_id {
                Self::deposit_event(Event::MakerDepositSlashed {
                    sell_id, maker_id, penalty_id: pid,
                });
            }

            Ok(())
        }

        pub fn do_user_request_usdt_refund(
            user: &T::AccountId,
            sell_id: u64,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;

            ensure!(&record.user == user, Error::<T>::NotSellUser);
            ensure!(
                record.status == SellOrderStatus::SeverelyDisputed,
                Error::<T>::InvalidSellStatus
            );

            // 标记为等待退款（Arbitrating 状态）
            record.status = SellOrderStatus::Arbitrating;
            SellOrders::<T>::insert(sell_id, record);

            Self::deposit_event(Event::UserRequestedUsdtRefund { sell_id });

            Ok(())
        }

        pub fn do_maker_confirm_usdt_refund(
            maker: &T::AccountId,
            sell_id: u64,
            refund_tx_hash: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let mut record = SellOrders::<T>::get(sell_id)
                .ok_or(Error::<T>::SellOrderNotFound)?;

            ensure!(&record.maker == maker, Error::<T>::NotMaker);
            ensure!(
                record.status == SellOrderStatus::Arbitrating,
                Error::<T>::InvalidSellStatus
            );

            let refund_hash: BoundedVec<u8, ConstU32<128>> = refund_tx_hash
                .try_into().map_err(|_| Error::<T>::InvalidTxHash)?;

            // NEX 全部退还用户
            T::Escrow::release_all(sell_id, &record.user)?;

            record.status = SellOrderStatus::Refunded;
            SellOrders::<T>::insert(sell_id, record);
            SellUnderpaidEvidences::<T>::remove(sell_id);

            Self::deposit_event(Event::MakerUsdtRefundConfirmed {
                sell_id,
                refund_tx_hash: refund_hash,
            });

            Ok(())
        }

        // ===== 辅助函数 =====

        /// 释放买家押金（退还给买家）
        fn release_buyer_deposit(
            buyer: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            if amount.is_zero() { return Ok(()); }
            let pallet_account = Self::pallet_account_id();
            <T::Currency as Currency<T::AccountId>>::transfer(
                &pallet_account, buyer, amount,
                frame_support::traits::ExistenceRequirement::AllowDeath,
            )?;
            TotalDepositPoolBalance::<T>::mutate(|total| {
                *total = total.saturating_sub(amount);
            });
            Ok(())
        }

        /// 没收买家押金（转给做市商）
        fn forfeit_buyer_deposit(
            maker: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            if amount.is_zero() { return Ok(()); }
            let pallet_account = Self::pallet_account_id();
            <T::Currency as Currency<T::AccountId>>::transfer(
                &pallet_account, maker, amount,
                frame_support::traits::ExistenceRequirement::AllowDeath,
            )?;
            TotalDepositPoolBalance::<T>::mutate(|total| {
                *total = total.saturating_sub(amount);
            });
            Ok(())
        }

        /// Buy 状态转 u8
        fn buy_state_to_u8(state: &BuyOrderState) -> u8 {
            match state {
                BuyOrderState::Created => 0,
                BuyOrderState::PaidOrCommitted => 1,
                BuyOrderState::Released => 2,
                BuyOrderState::Refunded => 3,
                BuyOrderState::Canceled => 4,
                BuyOrderState::Disputed => 5,
                BuyOrderState::Closed => 6,
                BuyOrderState::Expired => 7,
            }
        }

        // ===== Hook 实现 =====

        /// 处理过期 Buy 订单（使用持久化游标，确保所有订单都能被扫描到）
        pub fn process_expired_buy_orders() -> Weight {
            let mut processed = 0u32;
            let max_per_block = 10u32;
            let max_scan = 50u32;
            let mut scanned = 0u32;

            let mut cursor = BuyExpiryCursor::<T>::get();
            let next_id = NextBuyOrderId::<T>::get();
            let now_secs = T::Timestamp::now().as_secs().saturated_into::<u64>();

            while cursor < next_id && processed < max_per_block && scanned < max_scan {
                scanned += 1;
                if let Some(order) = BuyOrders::<T>::get(cursor) {
                    if order.state == BuyOrderState::Created && now_secs > order.expire_at {
                        if Self::do_expire_buy_order(cursor, &order).is_ok() {
                            processed += 1;
                        }
                    }
                }
                cursor += 1;
            }
            BuyExpiryCursor::<T>::put(cursor);

            Weight::from_parts((processed as u64) * 100_000 + 10_000, 0)
        }

        /// 执行单个 Buy 订单过期
        fn do_expire_buy_order(order_id: u64, order: &BuyOrder<T>) -> DispatchResult {
            use pallet_trading_credit::quota::BuyerQuotaInterface;

            // 更新状态
            BuyOrders::<T>::mutate(order_id, |maybe| {
                if let Some(o) = maybe { o.state = BuyOrderState::Expired; }
            });

            // 退还托管给做市商
            let _ = T::Escrow::refund_all(order_id, &order.maker);

            // 释放买家额度（amount 已是 USD 精度 10^6）
            let amount_u128: u128 = order.amount.saturated_into();
            let _ = T::BuyerCredit::release_quota(&order.taker, amount_u128 as u64);

            // 首购处理
            if order.is_first_purchase {
                MakerFirstPurchaseCount::<T>::mutate(order.maker_id, |count| {
                    *count = count.saturating_sub(1);
                });
            }

            // 超时没收买家押金
            if !order.buyer_deposit.is_zero() {
                let _ = Self::forfeit_buyer_deposit(&order.maker, order.buyer_deposit);
                BuyOrders::<T>::mutate(order_id, |o| {
                    if let Some(ord) = o { ord.deposit_status = DepositStatus::Forfeited; }
                });
                Self::deposit_event(Event::BuyerDepositForfeited {
                    order_id,
                    buyer: order.taker.clone(),
                    maker_id: order.maker_id,
                    forfeited_amount: order.buyer_deposit,
                });
            }

            Self::deposit_event(Event::BuyOrderAutoExpired {
                order_id,
                buyer: order.taker.clone(),
                maker_id: order.maker_id,
                nex_amount: order.qty,
            });

            Ok(())
        }

        /// 仲裁裁决执行（Buy 侧）
        ///
        /// 由 ArbitrationBridge 调用，将仲裁结果应用到 Buy 订单
        pub fn apply_arbitration_decision(
            order_id: u64,
            decision: pallet_arbitration::pallet::Decision,
        ) -> DispatchResult {
            let mut order = BuyOrders::<T>::get(order_id)
                .ok_or(Error::<T>::BuyOrderNotFound)?;

            ensure!(
                order.state == BuyOrderState::Disputed,
                Error::<T>::InvalidBuyOrderStatus
            );

            use pallet_arbitration::pallet::Decision;
            // Buy 场景：托管的是做市商 NEX
            // Release = 买家胜 → NEX 给买家(taker)
            // Refund  = 做市商胜 → NEX 退回做市商(maker)
            let maker_win = match decision {
                Decision::Release => {
                    T::Escrow::release_all(order_id, &order.taker)?;
                    order.state = BuyOrderState::Released;
                    false
                },
                Decision::Refund => {
                    T::Escrow::refund_all(order_id, &order.maker)?;
                    order.state = BuyOrderState::Refunded;
                    true
                },
                Decision::Partial(bps) => {
                    // bps = 买家获得的比例，剩余给做市商
                    T::Escrow::split_partial(order_id, &order.taker, &order.maker, bps)?;
                    order.state = BuyOrderState::Released;
                    bps < 5000
                },
            };

            let _ = T::MakerCredit::record_maker_dispute_result(
                order.maker_id, order_id, maker_win,
            );

            order.completed_at = Some(T::Timestamp::now().as_secs());
            BuyOrders::<T>::insert(order_id, order);

            Ok(())
        }

        /// Buy 归档（L1）
        pub fn archive_completed_buy_orders(max: u32) -> Weight {
            let mut archived = 0u32;
            let mut cursor = BuyArchiveCursor::<T>::get();
            let next_id = NextBuyOrderId::<T>::get();

            while cursor < next_id && archived < max {
                match BuyOrders::<T>::get(cursor) {
                    Some(order) => {
                        let is_terminal = matches!(
                            order.state,
                            BuyOrderState::Released | BuyOrderState::Canceled
                            | BuyOrderState::Closed | BuyOrderState::Expired
                        );
                        if is_terminal {
                            let archived_order = ArchivedBuyOrder {
                                maker_id: order.maker_id,
                                taker: order.taker,
                                qty: order.qty.saturated_into(),
                                amount: order.amount.saturated_into(),
                                state: order.state,
                                completed_at: order.completed_at.unwrap_or(0),
                            };
                            ArchivedBuyOrders::<T>::insert(cursor, archived_order);
                            BuyOrders::<T>::remove(cursor);
                            archived += 1;
                            cursor += 1;
                        } else {
                            // 非终态订单，停止推进游标，下次再试
                            break;
                        }
                    },
                    None => {
                        // 订单已被删除或不存在，跳过
                        cursor += 1;
                    }
                }
            }
            BuyArchiveCursor::<T>::put(cursor);

            Weight::from_parts((archived as u64) * 50_000, 0)
        }

        /// Sell 归档（L1）
        pub fn archive_completed_sell_orders(max: u32) -> Weight {
            let mut archived = 0u32;
            let mut cursor = SellArchiveCursor::<T>::get();
            let next_id = NextSellOrderId::<T>::get();

            while cursor < next_id && archived < max {
                match SellOrders::<T>::get(cursor) {
                    Some(record) => {
                        let is_terminal = matches!(
                            record.status,
                            SellOrderStatus::Completed | SellOrderStatus::Refunded
                            | SellOrderStatus::ArbitrationApproved | SellOrderStatus::ArbitrationRejected
                        );
                        if is_terminal {
                            let archived_sell = ArchivedSellOrder {
                                sell_id: cursor,
                                maker_id: record.maker_id,
                                user: record.user,
                                nex_amount: record.nex_amount.saturated_into(),
                                usdt_amount: record.usdt_amount,
                                status: record.status,
                                completed_at: record.completed_at
                                    .map(|b| b.saturated_into::<u32>()).unwrap_or(0),
                            };
                            ArchivedSellOrders::<T>::insert(cursor, archived_sell);
                            SellOrders::<T>::remove(cursor);
                            archived += 1;
                            cursor += 1;
                        } else {
                            // 非终态订单，停止推进游标
                            break;
                        }
                    },
                    None => {
                        // 订单已被删除或不存在，跳过
                        cursor += 1;
                    }
                }
            }
            SellArchiveCursor::<T>::put(cursor);

            Weight::from_parts((archived as u64) * 50_000, 0)
        }

        /// Buy 侧 TxHash TTL 清理（复用 Sell 侧的 TxHashTtlBlocks）
        fn cleanup_expired_buy_tx_hashes(now: BlockNumberFor<T>, max: u32) -> Weight {
            let ttl = T::TxHashTtlBlocks::get();
            let mut cleaned = 0u32;

            // 遍历 BuyTronTxUsed，删除超过 TTL 的条目
            let mut to_remove = sp_std::vec::Vec::new();
            for (hash, recorded_block) in BuyTronTxUsed::<T>::iter() {
                if cleaned >= max { break; }
                if now > recorded_block + ttl {
                    to_remove.push(hash);
                    cleaned += 1;
                }
            }
            for hash in to_remove {
                BuyTronTxUsed::<T>::remove(hash);
            }

            Weight::from_parts((cleaned as u64) * 30_000, 0)
        }
    }
}
