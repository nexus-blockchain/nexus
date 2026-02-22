#![cfg_attr(not(feature = "std"), no_std)]

//! # Maker Pallet (做市商管理模块)
//!
//! ## 概述
//!
//! 本模块负责做市商的完整生命周期管理，包括：
//! - 做市商申请与审核
//! - 押金管理（锁定/解锁）
//! - 提现管理（冷却期）
//! - 溢价配置
//! - 服务暂停/恢复
//!
//! ## 版本历史
//!
//! - v0.1.0 (2025-11-03): 从 pallet-trading 拆分而来

pub use pallet::*;

// TODO: 测试文件待完善 mock 配置（需要多个依赖 pallet）
// #[cfg(test)]
// mod mock;

// #[cfg(test)]
// mod tests;

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
        traits::{Currency, ReservableCurrency, Get, ExistenceRequirement, UnixTime},
        BoundedVec,
        weights::Weight,
    };
    use sp_runtime::traits::{Saturating, SaturatedConversion};
    
    // 🆕 v0.4.0: 从 pallet-trading-common 导入公共类型和 Trait
    use pallet_trading_common::{
        TronAddress,
        Cid,
        PricingProvider,
    };
    
    /// 函数级详细中文注释：Balance 类型别名
    pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    // ===== 押金扣除相关数据结构 =====

    /// 函数级详细中文注释：押金扣除类型
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum PenaltyType {
        /// OTC订单超时
        OtcTimeout {
            order_id: u64,
            timeout_hours: u32,
        },
        /// Bridge兑换超时
        BridgeTimeout {
            swap_id: u64,
            timeout_hours: u32,
        },
        /// 争议败诉
        ArbitrationLoss {
            case_id: u64,
            loss_amount: u64, // USD amount
        },
        /// 信用分过低
        LowCreditScore {
            current_score: u32,
            days_below_threshold: u32,
        },
        /// 恶意行为
        MaliciousBehavior {
            behavior_type: u8,
            evidence_cid: BoundedVec<u8, ConstU32<64>>,
        },
        /// 🆕 Swap 严重少付（做市商少付 USDT 给用户）
        SwapSeverelyUnderpaid {
            swap_id: u64,
            expected_usdt: u64,   // 预期 USDT（精度 10^6）
            actual_usdt: u64,     // 实际 USDT（精度 10^6）
        },
    }

    /// 函数级详细中文注释：押金扣除记录
    #[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct PenaltyRecord<T: Config> {
        /// 做市商ID
        pub maker_id: u64,
        /// 扣除类型
        pub penalty_type: PenaltyType,
        /// 扣除的NEX数量
        pub deducted_amount: BalanceOf<T>,
        /// 扣除时的USD价值
        pub usd_value: u64,
        /// 受益人账户（如果有）
        pub beneficiary: Option<T::AccountId>,
        /// 扣除时间
        pub deducted_at: BlockNumberFor<T>,
        /// 是否已申诉
        pub appealed: bool,
        /// 申诉结果
        pub appeal_result: Option<bool>,
    }

    /// 🆕 归档惩罚记录（L2精简版，~24字节）
    /// 用于长期存储历史惩罚记录，减少链上存储占用
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
    pub struct ArchivedPenaltyL2 {
        /// 惩罚记录ID
        pub penalty_id: u64,
        /// 做市商ID
        pub maker_id: u64,
        /// 扣除的USD价值
        pub usd_value: u64,
        /// 惩罚类型代码 (0=OtcTimeout, 1=BridgeTimeout, 2=ArbitrationLoss, 3=LowCredit, 4=Malicious)
        pub penalty_type_code: u8,
        /// 申诉结果 (0=未申诉, 1=申诉成功, 2=申诉失败)
        pub appeal_status: u8,
    }

    impl ArchivedPenaltyL2 {
        /// 从完整记录创建归档版本
        pub fn from_full<T: Config>(penalty_id: u64, record: &PenaltyRecord<T>) -> Self {
            let penalty_type_code = match &record.penalty_type {
                PenaltyType::OtcTimeout { .. } => 0,
                PenaltyType::BridgeTimeout { .. } => 1,
                PenaltyType::ArbitrationLoss { .. } => 2,
                PenaltyType::LowCreditScore { .. } => 3,
                PenaltyType::MaliciousBehavior { .. } => 4,
                PenaltyType::SwapSeverelyUnderpaid { .. } => 5,
            };
            let appeal_status = match (record.appealed, record.appeal_result) {
                (false, _) => 0,
                (true, Some(true)) => 1,
                (true, _) => 2,
            };
            Self {
                penalty_id,
                maker_id: record.maker_id,
                usd_value: record.usd_value,
                penalty_type_code,
                appeal_status,
            }
        }
    }
    
    // ===== 数据结构 =====
    
    /// 函数级详细中文注释：做市商申请状态
    #[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum ApplicationStatus {
        /// 押金已锁定，等待提交资料
        DepositLocked,
        /// 资料已提交，等待审核
        PendingReview,
        /// 审核通过，做市商已激活
        Active,
        /// 审核驳回
        Rejected,
        /// 申请已取消
        Cancelled,
        /// 申请已超时
        Expired,
    }
    
    /// 函数级详细中文注释：做市商业务方向
    #[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum Direction {
        /// 仅买入（仅Bridge）- 做市商购买COS，支付USDT
        Buy = 0,
        /// 仅卖出（仅OTC）- 做市商出售COS，收取USDT
        Sell = 1,
        /// 双向（OTC + Bridge）- 既可以买入也可以卖出
        BuyAndSell = 2,
    }
    
    impl Direction {
        /// 从 u8 转换为 Direction
        pub fn from_u8(value: u8) -> Option<Self> {
            match value {
                0 => Some(Direction::Buy),
                1 => Some(Direction::Sell),
                2 => Some(Direction::BuyAndSell),
                _ => None,
            }
        }
    }
    
    impl Default for Direction {
        fn default() -> Self {
            Self::BuyAndSell
        }
    }
    
    /// 函数级详细中文注释：提现请求状态
    #[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum WithdrawalStatus {
        /// 待执行（冷却期中）
        Pending,
        /// 已执行
        Executed,
        /// 已取消
        Cancelled,
    }
    
    /// 函数级详细中文注释：做市商申请记录
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct MakerApplication<T: Config> {
        /// 所有者账户
        pub owner: T::AccountId,
        /// 押金金额
        pub deposit: BalanceOf<T>,
        /// 申请状态
        pub status: ApplicationStatus,
        /// 业务方向
        pub direction: Direction,
        /// TRON地址（统一用于OTC收款和Bridge发款）
        pub tron_address: TronAddress,
        /// 公开资料CID（IPFS，加密）
        pub public_cid: Cid,
        /// 私密资料CID（IPFS，加密）
        pub private_cid: Cid,
        /// Buy溢价（基点，-500 ~ 500）
        pub buy_premium_bps: i16,
        /// Sell溢价（基点，-500 ~ 500）
        pub sell_premium_bps: i16,
        /// 最小交易金额
        pub min_amount: BalanceOf<T>,
        /// 创建时间（Unix时间戳，秒）
        pub created_at: u32,
        /// 资料提交截止时间（Unix时间戳，秒）
        pub info_deadline: u32,
        /// 审核截止时间（Unix时间戳，秒）
        pub review_deadline: u32,
        /// 服务暂停状态
        pub service_paused: bool,
        /// 已服务用户数量
        pub users_served: u32,
        /// 脱敏姓名（显示给用户）
        pub masked_full_name: BoundedVec<u8, ConstU32<64>>,
        /// 脱敏身份证号
        pub masked_id_card: BoundedVec<u8, ConstU32<32>>,
        /// 脱敏生日
        pub masked_birthday: BoundedVec<u8, ConstU32<16>>,
        /// 脱敏收款方式信息（JSON格式）
        pub masked_payment_info: BoundedVec<u8, ConstU32<512>>,
        /// 微信号（显示给用户）
        pub wechat_id: BoundedVec<u8, ConstU32<64>>,
        /// 押金目标USD价值（固定1000 USDT，精度10^6）
        pub target_deposit_usd: u64,
        /// 上次价格检查时间
        pub last_price_check: BlockNumberFor<T>,
        /// 押金不足警告状态
        pub deposit_warning: bool,
    }
    
    /// 函数级详细中文注释：提现请求记录
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct WithdrawalRequest<Balance> {
        /// 提现金额
        pub amount: Balance,
        /// 申请时间（Unix时间戳，秒）
        pub requested_at: u32,
        /// 可执行时间（Unix时间戳，秒）
        pub executable_at: u32,
        /// 请求状态
        pub status: WithdrawalStatus,
    }
    
    #[pallet::pallet]
    pub struct Pallet<T>(_);
    
    /// 函数级详细中文注释：做市商模块配置 trait
    #[pallet::config]
    /// 函数级中文注释：Maker Pallet 配置 trait
    /// - 🔴 stable2506 API 变更：RuntimeEvent 自动继承，无需显式声明
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        
        /// 货币类型（用于押金锁定）
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
        
        /// 信用记录接口
        /// 🆕 2026-01-18: 统一使用 pallet_trading_common::MakerCreditInterface
        type MakerCredit: pallet_trading_common::MakerCreditInterface;
        
        /// 治理权限（用于审批做市商）
        /// 注意：移除 Success = AccountId 约束，以兼容委员会集体 Origin
        /// 委员会提案执行时使用 Collective Origin，其 Success 类型为 ()
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        
        /// Timestamp（用于获取当前时间）
        type Timestamp: UnixTime;
        
        /// 做市商押金金额
        #[pallet::constant]
        type MakerDepositAmount: Get<BalanceOf<Self>>;

        /// 做市商押金目标USD价值（1000 USD，精度10^6）
        #[pallet::constant]
        type TargetDepositUsd: Get<u64>;

        /// 押金补充触发阈值（950 USD，精度10^6）
        #[pallet::constant]
        type DepositReplenishThreshold: Get<u64>;

        /// 押金补充目标（1050 USD，精度10^6）
        #[pallet::constant]
        type DepositReplenishTarget: Get<u64>;

        /// 价格检查间隔（区块数，每小时检查一次）
        #[pallet::constant]
        type PriceCheckInterval: Get<BlockNumberFor<Self>>;

        /// 申诉时限（区块数，7天）
        #[pallet::constant]
        type AppealDeadline: Get<BlockNumberFor<Self>>;

        /// 定价服务接口
        type Pricing: PricingProvider<BalanceOf<Self>>;

        /// 申请超时时间（区块数）
        #[pallet::constant]
        type MakerApplicationTimeout: Get<BlockNumberFor<Self>>;
        
        /// 提现冷却期（区块数，默认 7 天）
        #[pallet::constant]
        type WithdrawalCooldown: Get<BlockNumberFor<Self>>;
        
        /// 权重信息
        type WeightInfo: WeightInfo;

        /// 🆕 P3: IPFS 内容注册接口（用于自动 Pin 做市商资料）
        /// 
        /// 集成 pallet-nexus-ipfs 的 ContentRegistry trait，
        /// 在做市商注册/更新资料时自动 Pin 内容到 IPFS。
        /// 
        /// Pin 策略：
        /// - 做市商公开资料：Standard 层级
        /// - 做市商私密资料：Standard 层级
        /// - 申诉证据：Standard 层级
        type ContentRegistry: pallet_storage_service::ContentRegistry;

        /// 🆕 国库账户（用于接收无受益人时的扣款）
        /// 
        /// 当做市商押金扣除但无指定受益人时，扣除的金额将转入国库账户
        type TreasuryAccount: Get<Self::AccountId>;
    }
    
    // ===== 存储 =====
    
    /// 函数级详细中文注释：下一个做市商 ID
    #[pallet::storage]
    #[pallet::getter(fn next_maker_id)]
    pub type NextMakerId<T> = StorageValue<_, u64, ValueQuery>;
    
    /// 函数级详细中文注释：做市商申请记录
    #[pallet::storage]
    #[pallet::getter(fn maker_applications)]
    pub type MakerApplications<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        MakerApplication<T>,
    >;
    
    /// 函数级详细中文注释：账户到做市商 ID 的映射
    #[pallet::storage]
    #[pallet::getter(fn account_to_maker)]
    pub type AccountToMaker<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u64,
    >;
    
    /// 函数级详细中文注释：提现请求记录
    #[pallet::storage]
    #[pallet::getter(fn withdrawal_requests)]
    pub type WithdrawalRequests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,  // maker_id
        WithdrawalRequest<BalanceOf<T>>,
    >;

    /// 函数级详细中文注释：下一个惩罚记录ID
    #[pallet::storage]
    #[pallet::getter(fn next_penalty_id)]
    pub type NextPenaltyId<T> = StorageValue<_, u64, ValueQuery>;

    /// 函数级详细中文注释：惩罚记录
    #[pallet::storage]
    #[pallet::getter(fn penalty_records)]
    pub type PenaltyRecords<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // penalty_id
        PenaltyRecord<T>,
    >;

    /// 函数级详细中文注释：做市商的惩罚记录列表
    #[pallet::storage]
    #[pallet::getter(fn maker_penalties)]
    pub type MakerPenalties<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // maker_id
        BoundedVec<u64, ConstU32<100>>, // penalty_ids
        ValueQuery,
    >;

    /// 🆕 押金自动补充检查游标
    /// 用于 on_idle 中追踪上次检查到哪个 maker_id
    #[pallet::storage]
    pub type DepositCheckCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 🆕 惩罚记录归档游标
    /// 用于 on_idle 中追踪上次归档到哪个 penalty_id
    #[pallet::storage]
    pub type PenaltyArchiveCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 🆕 已归档的惩罚记录（L2精简版，按年月索引）
    /// 保留最少信息用于历史查询
    #[pallet::storage]
    pub type ArchivedPenalties<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32, // year_month (YYMM格式)
        BoundedVec<ArchivedPenaltyL2, ConstU32<1000>>,
        ValueQuery,
    >;
    
    // ===== Hooks =====

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// 🆕 空闲时自动检查并补充做市商押金 + 归档旧惩罚记录
        ///
        /// 每次最多检查 max_count 个做市商，避免阻塞区块
        fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let base_weight = Weight::from_parts(25_000, 0);
            
            // 确保有足够权重执行检查
            if remaining_weight.ref_time() < base_weight.ref_time() * 10 {
                return Weight::zero();
            }
            
            let mut consumed = Weight::zero();
            
            // 1. 押金自动补充检查（5个做市商）
            consumed = consumed.saturating_add(Self::auto_check_and_replenish_deposits(5));
            
            // 2. 惩罚记录归档（3条记录，30天以上的记录）
            let archive_weight = Self::archive_old_penalty_records(now, 3, 30 * 14400);
            consumed = consumed.saturating_add(archive_weight);
            
            consumed
        }
    }
    
    // ===== 事件 =====
    
    /// 函数级详细中文注释：做市商模块事件
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 押金已锁定
        MakerDepositLocked { maker_id: u64, who: T::AccountId, amount: BalanceOf<T> },
        /// 资料已提交
        MakerInfoSubmitted { maker_id: u64, who: T::AccountId },
        /// 做市商已批准
        /// 注意：由于委员会集体 Origin 不返回具体账户，approved_by 为 None 表示通过委员会投票批准
        MakerApproved { maker_id: u64, approved_by: Option<T::AccountId> },
        /// 做市商已驳回
        /// 注意：由于委员会集体 Origin 不返回具体账户，rejected_by 为 None 表示通过委员会投票驳回
        MakerRejected { maker_id: u64, rejected_by: Option<T::AccountId> },
        /// 做市商申请已取消
        MakerCancelled { maker_id: u64, who: T::AccountId },
        /// 提现已申请
        WithdrawalRequested { maker_id: u64, amount: BalanceOf<T> },
        /// 提现已执行
        WithdrawalExecuted { maker_id: u64, amount: BalanceOf<T> },
        /// 提现已取消
        WithdrawalCancelled { maker_id: u64 },
        /// 紧急提现已执行
        EmergencyWithdrawalExecuted { maker_id: u64, to: T::AccountId, amount: BalanceOf<T> },

        /// 押金已补充
        DepositReplenished {
            maker_id: u64,
            amount: BalanceOf<T>,
            total_deposit: BalanceOf<T>,
        },

        /// 押金不足警告
        DepositInsufficient {
            maker_id: u64,
            current_usd_value: u64,
        },

        /// 押金检查完成
        DepositCheckCompleted {
            checked_count: u32,
            insufficient_count: u32,
        },

        /// 押金已扣除
        DepositDeducted {
            maker_id: u64,
            penalty_id: u64,
            deducted_amount: BalanceOf<T>,
            usd_value: u64,
            reason: BoundedVec<u8, ConstU32<64>>,
            beneficiary: Option<T::AccountId>,
        },

        /// 需要补充押金
        DepositReplenishmentRequired {
            maker_id: u64,
            current_usd_value: u64,
            required_usd_value: u64,
        },

        /// 押金扣除申诉
        PenaltyAppealed {
            maker_id: u64,
            penalty_id: u64,
            appeal_case_id: u64,
        },

        /// 申诉结果处理
        AppealResultProcessed {
            penalty_id: u64,
            maker_id: u64,
            appeal_granted: bool,
        },

        /// 押金已退还
        PenaltyRefunded {
            penalty_id: u64,
            maker_id: u64,
            refunded_amount: BalanceOf<T>,
        },

        /// 🆕 惩罚记录已归档
        PenaltyArchived {
            penalty_id: u64,
            maker_id: u64,
            year_month: u32,
        },
    }
    
    // ===== 错误 =====
    
    /// 函数级详细中文注释：做市商模块错误
    #[pallet::error]
    pub enum Error<T> {
        /// 已经申请过做市商
        MakerAlreadyExists,
        /// 做市商不存在
        MakerNotFound,
        /// 状态不正确
        InvalidMakerStatus,
        /// 押金不足
        InsufficientDeposit,
        /// 做市商未激活
        MakerNotActive,
        /// 余额不足
        InsufficientBalance,
        /// 无效的 TRON 地址
        InvalidTronAddress,
        /// 编码错误
        EncodingError,
        /// 提现请求不存在
        WithdrawalRequestNotFound,
        /// 提现冷却期未满足
        WithdrawalCooldownNotMet,
        /// 未授权
        NotAuthorized,

        /// 价格不可用
        PriceNotAvailable,
        /// 押金计算溢出
        DepositCalculationOverflow,
        /// 押金不足且无法补充
        CannotReplenishDeposit,
        /// 惩罚记录不存在
        PenaltyRecordNotFound,
        /// 已经申诉过
        AlreadyAppealed,
        /// 申诉期限已过
        AppealDeadlineExpired,
        /// 证据太长
        EvidenceTooLong,
        /// 订单不存在
        OrderNotFound,
        /// 兑换不存在
        SwapNotFound,
        /// 计算溢出
        CalculationOverflow,
    }
    
    // ===== Extrinsics =====
    
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 🆕 治理强制补充做市商押金
        ///
        /// 当做市商未主动补充且押金严重不足时，治理可强制触发
        ///
        /// # 参数
        /// - `origin`: 治理权限
        /// - `maker_id`: 做市商ID
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn force_replenish_deposit(
            origin: OriginFor<T>,
            maker_id: u64,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            
            // 检查做市商是否需要补充
            ensure!(
                Self::needs_deposit_replenishment(maker_id)?,
                Error::<T>::InsufficientDeposit
            );
            
            // 执行补充
            let _amount = Self::replenish_maker_deposit(maker_id)?;
            
            Ok(())
        }

        /// 函数级详细中文注释：锁定做市商押金
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是签名账户）
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn lock_deposit(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_lock_deposit(&who)
        }
        
        /// 函数级详细中文注释：提交做市商资料
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是签名账户）
        /// - `real_name`: 真实姓名
        /// - `id_card_number`: 身份证号
        /// - `birthday`: 生日（YYYY-MM-DD）
        /// - `tron_address`: TRON 地址
        /// - `wechat_id`: 微信号
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn submit_info(
            origin: OriginFor<T>,
            real_name: sp_std::vec::Vec<u8>,
            id_card_number: sp_std::vec::Vec<u8>,
            birthday: sp_std::vec::Vec<u8>,
            tron_address: sp_std::vec::Vec<u8>,
            wechat_id: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_submit_info(
                &who,
                real_name,
                id_card_number,
                birthday,
                tron_address,
                wechat_id,
            )
        }
        
        /// 函数级详细中文注释：审批做市商申请
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是治理权限：Root 或委员会 2/3 多数）
        /// - `maker_id`: 做市商 ID
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        ///
        /// # 说明
        /// - 支持 Root 直接调用
        /// - 支持委员会提案投票通过后执行
        /// - 委员会集体 Origin 不返回具体账户，事件中 approved_by 为 None
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn approve_maker(origin: OriginFor<T>, maker_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_approve_maker(maker_id)
        }
        
        /// 函数级详细中文注释：驳回做市商申请
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是治理权限：Root 或委员会 2/3 多数）
        /// - `maker_id`: 做市商 ID
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        ///
        /// # 说明
        /// - 支持 Root 直接调用
        /// - 支持委员会提案投票通过后执行
        /// - 驳回后将解锁申请人的押金
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn reject_maker(origin: OriginFor<T>, maker_id: u64) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_reject_maker(maker_id)
        }
        
        /// 函数级详细中文注释：取消做市商申请
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是签名账户）
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn cancel_maker(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_cancel_maker(&who)
        }
        
        /// 函数级详细中文注释：申请提现押金
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是签名账户）
        /// - `amount`: 提现金额
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn request_withdrawal(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_request_withdrawal(&who, amount)
        }
        
        /// 函数级详细中文注释：执行提现
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是签名账户）
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn execute_withdrawal(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_execute_withdrawal(&who)
        }
        
        /// 函数级详细中文注释：取消提现请求
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是签名账户）
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn cancel_withdrawal(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_cancel_withdrawal(&who)
        }
        
        /// 函数级详细中文注释：紧急提现（治理功能）
        ///
        /// # 参数
        /// - `origin`: 调用者（必须是治理权限）
        /// - `maker_id`: 做市商 ID
        /// - `to`: 接收账户
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn emergency_withdrawal(
            origin: OriginFor<T>,
            maker_id: u64,
            to: T::AccountId,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            Self::do_emergency_withdrawal(maker_id, &to)
        }

        /// 函数级详细中文注释：做市商主动补充押金
        ///
        /// # 参数
        /// - `origin`: 调用者（做市商，必须是签名账户）
        ///
        /// # 返回
        /// - `DispatchResult`: 成功或错误
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn replenish_deposit(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取做市商ID
            let maker_id = Self::account_to_maker(&who)
                .ok_or(Error::<T>::MakerNotFound)?;

            // 执行补充
            let _amount = Self::replenish_maker_deposit(maker_id)?;

            Ok(())
        }

        /// 函数级详细中文注释：申诉押金扣除
        ///
        /// # 参数
        /// - `origin`: 做市商账户
        /// - `penalty_id`: 扣除记录ID
        /// - `evidence_cid`: 申诉证据IPFS CID
        ///
        /// # 返回
        /// - `DispatchResult`: 申诉结果
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::lock_deposit())]
        pub fn appeal_penalty(
            origin: OriginFor<T>,
            penalty_id: u64,
            _evidence_cid: sp_std::vec::Vec<u8>,  // 添加下划线前缀忽略警告
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 获取做市商ID
            let maker_id = Self::account_to_maker(&who)
                .ok_or(Error::<T>::MakerNotFound)?;

            // 获取扣除记录
            let mut record = PenaltyRecords::<T>::get(penalty_id)
                .ok_or(Error::<T>::PenaltyRecordNotFound)?;

            // 验证申诉权限
            ensure!(record.maker_id == maker_id, Error::<T>::NotAuthorized);
            ensure!(!record.appealed, Error::<T>::AlreadyAppealed);

            // 验证申诉时限（扣除后7天内）
            let current_block = frame_system::Pallet::<T>::block_number();
            let deadline = record.deducted_at + T::AppealDeadline::get();
            ensure!(current_block <= deadline, Error::<T>::AppealDeadlineExpired);

            // 标记为已申诉
            record.appealed = true;
            PenaltyRecords::<T>::insert(penalty_id, record);

            // 发出申诉事件（简化版，假设仲裁case_id为penalty_id）
            Self::deposit_event(Event::PenaltyAppealed {
                maker_id,
                penalty_id,
                appeal_case_id: penalty_id, // 简化处理
            });

            Ok(())
        }
    }
    
    // ===== 内部实现 =====
    
    impl<T: Config> Pallet<T> {
        /// 函数级详细中文注释：锁定做市商押金
        /// 
        /// # 参数
        /// - who: 申请人账户
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_lock_deposit(who: &T::AccountId) -> DispatchResult {
            // 检查是否已申请
            ensure!(
                !AccountToMaker::<T>::contains_key(who),
                Error::<T>::MakerAlreadyExists
            );
            
            let deposit = T::MakerDepositAmount::get();
            
            // 锁定押金
            T::Currency::reserve(who, deposit)
                .map_err(|_| Error::<T>::InsufficientBalance)?;
            
            // 获取新的做市商ID
            let maker_id = NextMakerId::<T>::get();
            NextMakerId::<T>::put(maker_id.saturating_add(1));
            
            // 获取当前时间
            let now = T::Timestamp::now().as_secs().saturated_into::<u32>();
            
            // 创建申请记录
            let application = MakerApplication::<T> {
                owner: who.clone(),
                deposit,
                status: ApplicationStatus::DepositLocked,
                direction: Direction::default(),
                tron_address: BoundedVec::default(),
                public_cid: BoundedVec::default(),
                private_cid: BoundedVec::default(),
                buy_premium_bps: 0,
                sell_premium_bps: 0,
                min_amount: BalanceOf::<T>::default(),
                created_at: now,
                info_deadline: now + 3600, // 1小时提交资料窗口
                review_deadline: now + 86400, // 24小时审核窗口
                service_paused: false,
                users_served: 0,
                masked_full_name: BoundedVec::default(),
                masked_id_card: BoundedVec::default(),
                masked_birthday: BoundedVec::default(),
                masked_payment_info: BoundedVec::default(),
                wechat_id: BoundedVec::default(),
                target_deposit_usd: T::TargetDepositUsd::get(), // 新增：目标USD价值
                last_price_check: frame_system::Pallet::<T>::block_number(), // 新增：价格检查时间
                deposit_warning: false, // 新增：警告状态
            };
            
            // 存储申请记录
            MakerApplications::<T>::insert(maker_id, application);
            AccountToMaker::<T>::insert(who, maker_id);
            
            // 触发事件
            Self::deposit_event(Event::MakerDepositLocked {
                maker_id,
                who: who.clone(),
                amount: deposit,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：提交做市商资料
        /// 
        /// # 参数
        /// - who: 申请人账户
        /// - real_name: 真实姓名
        /// - id_card_number: 身份证号
        /// - birthday: 生日（格式：YYYY-MM-DD）
        /// - tron_address: TRON地址
        /// - wechat_id: 微信号
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_submit_info(
            who: &T::AccountId,
            real_name: sp_std::vec::Vec<u8>,
            id_card_number: sp_std::vec::Vec<u8>,
            birthday: sp_std::vec::Vec<u8>,
            tron_address: sp_std::vec::Vec<u8>,
            wechat_id: sp_std::vec::Vec<u8>,
        ) -> DispatchResult {
            use pallet_trading_common::is_valid_tron_address;
            use pallet_trading_common::{mask_name, mask_id_card, mask_birthday};
            
            // 获取做市商ID
            let maker_id = AccountToMaker::<T>::get(who)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            // 获取申请记录
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> DispatchResult {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;
                
                // 检查状态
                ensure!(
                    app.status == ApplicationStatus::DepositLocked,
                    Error::<T>::InvalidMakerStatus
                );
                
                // 验证 TRON 地址
                ensure!(
                    is_valid_tron_address(&tron_address),
                    Error::<T>::InvalidTronAddress
                );
                
                // 脱敏处理
                let real_name_str = core::str::from_utf8(&real_name)
                    .map_err(|_| Error::<T>::EncodingError)?;
                let id_card_str = core::str::from_utf8(&id_card_number)
                    .map_err(|_| Error::<T>::EncodingError)?;
                let birthday_str = core::str::from_utf8(&birthday)
                    .map_err(|_| Error::<T>::EncodingError)?;
                
                let masked_name = mask_name(real_name_str);
                let masked_id = mask_id_card(id_card_str);
                let masked_birth = mask_birthday(birthday_str);
                
                // 更新申请记录
                app.status = ApplicationStatus::PendingReview;
                app.tron_address = TronAddress::try_from(tron_address)
                    .map_err(|_| Error::<T>::EncodingError)?;
                app.masked_full_name = BoundedVec::try_from(masked_name)
                    .map_err(|_| Error::<T>::EncodingError)?;
                app.masked_id_card = BoundedVec::try_from(masked_id)
                    .map_err(|_| Error::<T>::EncodingError)?;
                app.masked_birthday = BoundedVec::try_from(masked_birth)
                    .map_err(|_| Error::<T>::EncodingError)?;
                app.wechat_id = BoundedVec::try_from(wechat_id)
                    .map_err(|_| Error::<T>::EncodingError)?;
                
                // 🆕 P3: 自动 Pin 做市商资料到 IPFS（Standard 层级）
                // 公开资料和私密资料都需要长期保存
                if !app.public_cid.is_empty() {
                    let _ = <T::ContentRegistry as pallet_storage_service::ContentRegistry>::register_content(
                        b"trading-maker".to_vec(),
                        maker_id,
                        app.public_cid.to_vec(),
                        pallet_storage_service::PinTier::Standard,
                    );
                }
                if !app.private_cid.is_empty() {
                    let _ = <T::ContentRegistry as pallet_storage_service::ContentRegistry>::register_content(
                        b"trading-maker".to_vec(),
                        maker_id.saturating_add(1000000), // 私密资料使用偏移ID
                        app.private_cid.to_vec(),
                        pallet_storage_service::PinTier::Standard,
                    );
                }
                
                Ok(())
            })?;
            
            // 触发事件
            Self::deposit_event(Event::MakerInfoSubmitted {
                maker_id,
                who: who.clone(),
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：审批做市商申请
        ///
        /// # 参数
        /// - maker_id: 做市商ID
        ///
        /// # 返回
        /// - DispatchResult
        ///
        /// # 说明
        /// - 由于委员会集体 Origin 不返回具体账户，事件中 approved_by 设为 None
        /// - 审批记录可通过链上事件追溯（提案发起者、投票者等）
        pub fn do_approve_maker(maker_id: u64) -> DispatchResult {
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> DispatchResult {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;

                // 检查状态
                ensure!(
                    app.status == ApplicationStatus::PendingReview,
                    Error::<T>::InvalidMakerStatus
                );

                // 更新状态
                app.status = ApplicationStatus::Active;

                Ok(())
            })?;

            // 触发事件（approved_by 为 None，表示通过治理流程批准）
            Self::deposit_event(Event::MakerApproved {
                maker_id,
                approved_by: None,
            });

            Ok(())
        }
        
        /// 函数级详细中文注释：驳回做市商申请
        ///
        /// # 参数
        /// - maker_id: 做市商ID
        ///
        /// # 返回
        /// - DispatchResult
        ///
        /// # 说明
        /// - 由于委员会集体 Origin 不返回具体账户，事件中 rejected_by 设为 None
        /// - 驳回后将解锁申请人的押金
        pub fn do_reject_maker(maker_id: u64) -> DispatchResult {
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> DispatchResult {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;

                // 检查状态
                ensure!(
                    app.status == ApplicationStatus::PendingReview,
                    Error::<T>::InvalidMakerStatus
                );

                // 更新状态
                app.status = ApplicationStatus::Rejected;

                // 解锁押金
                T::Currency::unreserve(&app.owner, app.deposit);

                Ok(())
            })?;

            // 触发事件（rejected_by 为 None，表示通过治理流程驳回）
            Self::deposit_event(Event::MakerRejected {
                maker_id,
                rejected_by: None,
            });

            Ok(())
        }
        
        /// 函数级详细中文注释：取消做市商申请
        /// 
        /// # 参数
        /// - who: 申请人账户
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_cancel_maker(who: &T::AccountId) -> DispatchResult {
            // 获取做市商ID
            let maker_id = AccountToMaker::<T>::get(who)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> DispatchResult {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;
                
                // 检查状态（只能在 DepositLocked 或 PendingReview 状态下取消）
                ensure!(
                    app.status == ApplicationStatus::DepositLocked 
                    || app.status == ApplicationStatus::PendingReview,
                    Error::<T>::InvalidMakerStatus
                );
                
                // 更新状态
                app.status = ApplicationStatus::Cancelled;
                
                // 解锁押金
                T::Currency::unreserve(&app.owner, app.deposit);
                
                Ok(())
            })?;
            
            // 触发事件
            Self::deposit_event(Event::MakerCancelled {
                maker_id,
                who: who.clone(),
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：申请提现押金
        /// 
        /// # 参数
        /// - who: 做市商账户
        /// - amount: 提现金额
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_request_withdrawal(who: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            // 获取做市商ID
            let maker_id = AccountToMaker::<T>::get(who)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            // 检查做市商状态
            let app = MakerApplications::<T>::get(maker_id)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            ensure!(
                app.status == ApplicationStatus::Active,
                Error::<T>::MakerNotActive
            );
            
            // 检查押金是否足够
            ensure!(
                app.deposit >= amount,
                Error::<T>::InsufficientDeposit
            );
            
            // 检查是否已有待处理的提现请求
            ensure!(
                !WithdrawalRequests::<T>::contains_key(maker_id),
                Error::<T>::NotAuthorized
            );
            
            // 获取当前时间
            let now = T::Timestamp::now().as_secs().saturated_into::<u32>();
            let cooldown = T::WithdrawalCooldown::get().saturated_into::<u32>();
            
            // 创建提现请求
            let request = WithdrawalRequest {
                amount,
                requested_at: now,
                executable_at: now.saturating_add(cooldown),
                status: WithdrawalStatus::Pending,
            };
            
            WithdrawalRequests::<T>::insert(maker_id, request);
            
            // 触发事件
            Self::deposit_event(Event::WithdrawalRequested {
                maker_id,
                amount,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：执行提现
        /// 
        /// # 参数
        /// - who: 做市商账户
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_execute_withdrawal(who: &T::AccountId) -> DispatchResult {
            // 获取做市商ID
            let maker_id = AccountToMaker::<T>::get(who)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            // 获取提现请求
            let request = WithdrawalRequests::<T>::get(maker_id)
                .ok_or(Error::<T>::WithdrawalRequestNotFound)?;
            
            // 检查状态
            ensure!(
                request.status == WithdrawalStatus::Pending,
                Error::<T>::InvalidMakerStatus
            );
            
            // 检查冷却期
            let now = T::Timestamp::now().as_secs().saturated_into::<u32>();
            ensure!(
                now >= request.executable_at,
                Error::<T>::WithdrawalCooldownNotMet
            );
            
            // 解锁押金
            T::Currency::unreserve(who, request.amount);
            
            // 更新申请记录中的押金金额
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> DispatchResult {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;
                app.deposit = app.deposit.saturating_sub(request.amount);
                Ok(())
            })?;
            
            // 更新提现请求状态
            WithdrawalRequests::<T>::mutate(maker_id, |maybe_req| {
                if let Some(req) = maybe_req {
                    req.status = WithdrawalStatus::Executed;
                }
            });
            
            // 触发事件
            Self::deposit_event(Event::WithdrawalExecuted {
                maker_id,
                amount: request.amount,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：取消提现请求
        /// 
        /// # 参数
        /// - who: 做市商账户
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_cancel_withdrawal(who: &T::AccountId) -> DispatchResult {
            // 获取做市商ID
            let maker_id = AccountToMaker::<T>::get(who)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            // 获取提现请求
            let request = WithdrawalRequests::<T>::get(maker_id)
                .ok_or(Error::<T>::WithdrawalRequestNotFound)?;
            
            // 检查状态
            ensure!(
                request.status == WithdrawalStatus::Pending,
                Error::<T>::InvalidMakerStatus
            );
            
            // 更新提现请求状态
            WithdrawalRequests::<T>::mutate(maker_id, |maybe_req| {
                if let Some(req) = maybe_req {
                    req.status = WithdrawalStatus::Cancelled;
                }
            });
            
            // 触发事件
            Self::deposit_event(Event::WithdrawalCancelled {
                maker_id,
            });
            
            Ok(())
        }
        
        /// 函数级详细中文注释：紧急提现（治理功能）
        /// 
        /// # 参数
        /// - maker_id: 做市商ID
        /// - to: 接收账户
        /// 
        /// # 返回
        /// - DispatchResult
        pub fn do_emergency_withdrawal(maker_id: u64, to: &T::AccountId) -> DispatchResult {
            // 获取申请记录
            let app = MakerApplications::<T>::get(maker_id)
                .ok_or(Error::<T>::MakerNotFound)?;
            
            // 解锁全部押金并转给指定账户
            T::Currency::unreserve(&app.owner, app.deposit);
            T::Currency::transfer(
                &app.owner,
                to,
                app.deposit,
                ExistenceRequirement::AllowDeath
            )?;
            
            // 更新申请记录中的押金金额
            MakerApplications::<T>::mutate(maker_id, |maybe_app| {
                if let Some(app) = maybe_app {
                    app.deposit = BalanceOf::<T>::default();
                }
            });
            
            // 触发事件
            Self::deposit_event(Event::EmergencyWithdrawalExecuted {
                maker_id,
                to: to.clone(),
                amount: app.deposit,
            });
            
            Ok(())
        }
    }

    // ===== 新增：动态押金管理和扣除机制 =====

    impl<T: Config> Pallet<T> {
        /// 函数级详细中文注释：计算指定USD价值对应的NEX数量
        pub fn calculate_nex_amount_for_usd(usd_value: u64) -> Result<BalanceOf<T>, DispatchError> {
            // 获取当前NEX/USD价格
            let cos_to_usd_rate = T::Pricing::get_cos_to_usd_rate()
                .ok_or(Error::<T>::PriceNotAvailable)?;

            // 计算所需NEX数量
            // NEX数量 = USD价值 / (NEX/USD价格)
            Self::calculate_cos_from_usd_rate(usd_value, cos_to_usd_rate)
        }

        /// 函数级详细中文注释：根据USD价值和汇率计算NEX数量
        fn calculate_cos_from_usd_rate(
            usd_value: u64,
            cos_to_usd_rate: BalanceOf<T>
        ) -> Result<BalanceOf<T>, DispatchError> {
            // 转换为u128进行高精度计算
            let usd_u128 = usd_value as u128;
            let rate_u128: u128 = cos_to_usd_rate.saturated_into();

            // 计算NEX数量 = USD价值 × NEX精度 ÷ NEX/USD汇率
            let cos_u128 = usd_u128
                .checked_mul(1_000_000_000_000u128) // NEX精度10^12
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_div(rate_u128)
                .ok_or(Error::<T>::CalculationOverflow)?;

            // 转换为BalanceOf<T>
            let nex_amount: BalanceOf<T> = cos_u128
                .try_into()
                .map_err(|_| Error::<T>::CalculationOverflow)?;

            Ok(nex_amount)
        }

        /// 函数级详细中文注释：计算COS押金的USD价值
        pub fn calculate_usd_value_of_deposit(deposit: BalanceOf<T>) -> Result<u64, DispatchError> {
            let cos_to_usd_rate = T::Pricing::get_cos_to_usd_rate()
                .ok_or(Error::<T>::PriceNotAvailable)?;

            // 转换为u128进行高精度计算
            let deposit_u128: u128 = deposit.saturated_into();
            let rate_u128: u128 = cos_to_usd_rate.saturated_into();

            // 计算USD价值 = NEX数量 × NEX/USD汇率 ÷ NEX精度
            let usd_u128 = deposit_u128
                .checked_mul(rate_u128)
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_div(1_000_000_000_000u128) // 除以NEX精度10^12
                .ok_or(Error::<T>::CalculationOverflow)?;

            // 转换为u64
            let usd_value: u64 = usd_u128
                .try_into()
                .map_err(|_| Error::<T>::CalculationOverflow)?;

            Ok(usd_value)
        }

        /// 函数级详细中文注释：检查做市商押金是否充足
        pub fn check_deposit_sufficiency(maker_id: u64) -> Result<bool, DispatchError> {
            let app = Self::maker_applications(maker_id)
                .ok_or(Error::<T>::MakerNotFound)?;

            // 计算当前押金的USD价值
            let current_usd_value = Self::calculate_usd_value_of_deposit(app.deposit)?;

            // 检查是否低于补充阈值
            Ok(current_usd_value >= T::DepositReplenishThreshold::get())
        }

        /// 函数级详细中文注释：补充做市商押金
        pub fn replenish_maker_deposit(maker_id: u64) -> Result<BalanceOf<T>, DispatchError> {
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> Result<BalanceOf<T>, DispatchError> {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;

                // 确保做市商已激活
                ensure!(
                    app.status == ApplicationStatus::Active,
                    Error::<T>::MakerNotActive
                );

                // 计算补充目标数量
                let target_nex_amount = Self::calculate_nex_amount_for_usd(
                    T::DepositReplenishTarget::get()
                )?;

                // 计算需要补充的金额
                let replenish_amount = target_nex_amount
                    .saturating_sub(app.deposit);

                if replenish_amount.is_zero() {
                    return Ok(replenish_amount);
                }

                // 锁定补充金额
                T::Currency::reserve(&app.owner, replenish_amount)
                    .map_err(|_| Error::<T>::InsufficientBalance)?;

                // 更新押金金额
                app.deposit = app.deposit.saturating_add(replenish_amount);
                app.deposit_warning = false;
                app.last_price_check = frame_system::Pallet::<T>::block_number();

                // 发出补充事件
                Self::deposit_event(Event::DepositReplenished {
                    maker_id,
                    amount: replenish_amount,
                    total_deposit: app.deposit,
                });

                Ok(replenish_amount)
            })
        }

        /// 函数级详细中文注释：执行押金扣除
        pub fn deduct_maker_deposit(
            maker_id: u64,
            penalty_type: PenaltyType,
            beneficiary: Option<T::AccountId>,
        ) -> Result<u64, DispatchError> {
            // 1. 验证做市商存在且处于活跃状态
            let mut app = Self::maker_applications(maker_id)
                .ok_or(Error::<T>::MakerNotFound)?;

            ensure!(
                app.status == ApplicationStatus::Active,
                Error::<T>::MakerNotActive
            );

            // 2. 计算扣除金额
            let (deduct_usd, reason) = Self::calculate_penalty_amount(&penalty_type)?;
            let deduct_cos = Self::calculate_nex_amount_for_usd(deduct_usd)?;

            // 3. 验证押金是否充足
            ensure!(
                app.deposit >= deduct_cos,
                Error::<T>::InsufficientDeposit
            );

            // 4. 执行扣除
            let penalty_id = Self::next_penalty_id();
            app.deposit = app.deposit.saturating_sub(deduct_cos);

            // 5. 处理扣除的资金
            match beneficiary.as_ref() {
                Some(beneficiary_account) => {
                    // 转给受益人
                    T::Currency::unreserve(&app.owner, deduct_cos);
                    T::Currency::transfer(
                        &app.owner,
                        beneficiary_account,
                        deduct_cos,
                        ExistenceRequirement::KeepAlive,
                    )?;
                },
                None => {
                    // 转入国库账户
                    T::Currency::unreserve(&app.owner, deduct_cos);
                    let treasury = T::TreasuryAccount::get();
                    T::Currency::transfer(
                        &app.owner,
                        &treasury,
                        deduct_cos,
                        ExistenceRequirement::AllowDeath,
                    )?;
                }
            }

            // 6. 记录扣除操作
            let record = PenaltyRecord {
                maker_id,
                penalty_type: penalty_type.clone(),
                deducted_amount: deduct_cos,
                usd_value: deduct_usd,
                beneficiary: beneficiary.clone(),
                deducted_at: frame_system::Pallet::<T>::block_number(),
                appealed: false,
                appeal_result: None,
            };

            PenaltyRecords::<T>::insert(penalty_id, record);
            MakerApplications::<T>::insert(maker_id, app.clone());
            NextPenaltyId::<T>::put(penalty_id + 1);

            // 7. 更新做市商惩罚记录列表
            MakerPenalties::<T>::try_mutate(maker_id, |penalties| {
                penalties.try_push(penalty_id)
                    .map_err(|_| Error::<T>::EncodingError)
            })?;

            // 8. 检查是否需要补充押金
            if Self::needs_deposit_replenishment_after_deduction(maker_id)? {
                Self::trigger_deposit_replenishment_warning(maker_id)?;
            }

            // 9. 发出事件
            Self::deposit_event(Event::DepositDeducted {
                maker_id,
                penalty_id,
                deducted_amount: deduct_cos,
                usd_value: deduct_usd,
                reason: BoundedVec::try_from(reason.as_bytes().to_vec()).unwrap_or_default(),
                beneficiary,
            });

            Ok(penalty_id)
        }

        /// 函数级详细中文注释：计算惩罚金额
        fn calculate_penalty_amount(
            penalty_type: &PenaltyType,
        ) -> Result<(u64, &'static str), DispatchError> {
            let (base_usd, reason) = match penalty_type {
                PenaltyType::OtcTimeout { order_id: _, timeout_hours: _ } => {
                    // OTC超时：固定50 USD（简化版）
                    (50_000_000u64, "OTC订单超时违约")
                },
                PenaltyType::BridgeTimeout { swap_id: _, timeout_hours: _ } => {
                    // Bridge超时：固定30 USD（简化版）
                    (30_000_000u64, "Bridge兑换超时")
                },
                PenaltyType::ArbitrationLoss { case_id: _, loss_amount } => {
                    // 争议败诉：损失金额的10%
                    let penalty_usd = (loss_amount * 10) / 100;
                    (penalty_usd + 20_000_000, "争议仲裁败诉") // +20 USD仲裁费
                },
                PenaltyType::LowCreditScore { current_score: _, days_below_threshold } => {
                    // 信用分过低：每日1 USD
                    (*days_below_threshold as u64 * 1_000_000, "信用分过低")
                },
                PenaltyType::MaliciousBehavior { behavior_type, evidence_cid: _ } => {
                    // 恶意行为：根据严重程度
                    let penalty_usd = match behavior_type {
                        1 => 50_000_000,   // 轻微：50 USD
                        2 => 100_000_000,  // 中等：100 USD
                        3 => 200_000_000,  // 严重：200 USD
                        _ => 50_000_000,   // 默认：50 USD
                    };
                    (penalty_usd, "恶意行为违规")
                },
                PenaltyType::SwapSeverelyUnderpaid { swap_id: _, expected_usdt, actual_usdt } => {
                    // 🆕 Swap 严重少付：动态最低罚金
                    // 罚金 = max(差额 × 10%, 预期金额 × 5%)
                    // 这样小额交易罚金比例合理，大额交易保持威慑力
                    let shortage = expected_usdt.saturating_sub(*actual_usdt);
                    let base_penalty = shortage / 10;  // 差额的 10%
                    let min_penalty = expected_usdt / 20;  // 预期金额的 5%
                    let penalty_usd = core::cmp::max(base_penalty, min_penalty);
                    (penalty_usd, "Swap严重少付违约")
                },
            };

            Ok((base_usd, reason))
        }

        /// 函数级详细中文注释：检查扣除后是否需要补充押金
        fn needs_deposit_replenishment_after_deduction(
            maker_id: u64,
        ) -> Result<bool, DispatchError> {
            let app = Self::maker_applications(maker_id)
                .ok_or(Error::<T>::MakerNotFound)?;

            // 计算当前押金的USD价值
            let current_usd_value = Self::calculate_usd_value_of_deposit(app.deposit)?;

            // 检查是否低于补充阈值
            Ok(current_usd_value < T::DepositReplenishThreshold::get())
        }

        /// 函数级详细中文注释：触发押金补充警告
        fn trigger_deposit_replenishment_warning(maker_id: u64) -> Result<(), DispatchError> {
            // 设置警告状态
            MakerApplications::<T>::try_mutate(maker_id, |maybe_app| -> DispatchResult {
                let app = maybe_app.as_mut().ok_or(Error::<T>::MakerNotFound)?;
                app.deposit_warning = true;
                Ok(())
            })?;

            // 发出警告事件
            Self::deposit_event(Event::DepositReplenishmentRequired {
                maker_id,
                current_usd_value: Self::get_deposit_usd_value(maker_id)?,
                required_usd_value: T::TargetDepositUsd::get(),
            });

            Ok(())
        }

        /// 函数级详细中文注释：查询做市商押金的USD价值
        pub fn get_deposit_usd_value(maker_id: u64) -> Result<u64, DispatchError> {
            let app = Self::maker_applications(maker_id)
                .ok_or(Error::<T>::MakerNotFound)?;

            Self::calculate_usd_value_of_deposit(app.deposit)
        }

        /// 函数级详细中文注释：查询做市商是否需要补充押金
        pub fn needs_deposit_replenishment(maker_id: u64) -> Result<bool, DispatchError> {
            Self::check_deposit_sufficiency(maker_id)
                .map(|sufficient| !sufficient)
        }

        /// 🆕 自动检查并补充做市商押金
        ///
        /// 从游标位置开始，检查 max_count 个活跃做市商的押金状态
        /// 如果押金不足且做市商账户余额充足，自动触发补充
        ///
        /// # 参数
        /// - `max_count`: 每次最多检查的做市商数量
        ///
        /// # 返回
        /// - 消耗的权重
        fn auto_check_and_replenish_deposits(max_count: u32) -> Weight {
            let next_id = NextMakerId::<T>::get();
            if next_id == 0 {
                return Weight::from_parts(5_000, 0);
            }

            let mut cursor = DepositCheckCursor::<T>::get();
            let mut checked_count = 0u32;
            let mut replenished_count = 0u32;
            let mut warning_count = 0u32;

            // 从游标位置开始循环检查
            for _ in 0..max_count {
                // 跳过 maker_id = 0（无效）
                if cursor == 0 {
                    cursor = 1;
                }

                // 循环回到起点
                if cursor >= next_id {
                    cursor = 1;
                }

                // 获取做市商信息
                if let Some(app) = MakerApplications::<T>::get(cursor) {
                    // 只检查活跃的做市商
                    if app.status == ApplicationStatus::Active {
                        checked_count = checked_count.saturating_add(1);

                        // 检查是否需要补充押金
                        if let Ok(true) = Self::needs_deposit_replenishment(cursor) {
                            // 尝试自动补充
                            match Self::replenish_maker_deposit(cursor) {
                                Ok(amount) if !amount.is_zero() => {
                                    replenished_count = replenished_count.saturating_add(1);
                                },
                                _ => {
                                    // 补充失败，发出警告
                                    let _ = Self::trigger_deposit_replenishment_warning(cursor);
                                    warning_count = warning_count.saturating_add(1);
                                }
                            }
                        }
                    }
                }

                cursor = cursor.saturating_add(1);
            }

            // 更新游标
            DepositCheckCursor::<T>::put(cursor);

            // 发出检查完成事件
            if checked_count > 0 {
                Self::deposit_event(Event::DepositCheckCompleted {
                    checked_count,
                    insufficient_count: warning_count,
                });
            }

            // 返回消耗的权重
            Weight::from_parts(
                (checked_count as u64) * 50_000 + (replenished_count as u64) * 100_000 + 10_000,
                0
            )
        }

        /// 🆕 归档旧惩罚记录
        ///
        /// 将超过 age_threshold 区块的惩罚记录从完整存储迁移到归档存储
        ///
        /// # 参数
        /// - `now`: 当前区块号
        /// - `max_count`: 每次最多归档的记录数
        /// - `age_threshold`: 归档阈值（区块数，超过此时间的记录将被归档）
        ///
        /// # 返回
        /// - 消耗的权重
        fn archive_old_penalty_records(
            now: BlockNumberFor<T>,
            max_count: u32,
            age_threshold: u32,
        ) -> Weight {
            let next_id = NextPenaltyId::<T>::get();
            if next_id == 0 {
                return Weight::from_parts(5_000, 0);
            }

            let mut cursor = PenaltyArchiveCursor::<T>::get();
            let mut archived_count = 0u32;
            let threshold_block = now.saturating_sub(age_threshold.into());

            // 从游标位置开始检查
            for _ in 0..max_count {
                if cursor >= next_id {
                    // 所有记录都已检查，重置游标
                    cursor = 0;
                    break;
                }

                // 获取惩罚记录
                if let Some(record) = PenaltyRecords::<T>::get(cursor) {
                    // 检查是否超过归档阈值
                    if record.deducted_at < threshold_block {
                        // 创建归档版本
                        let archived = ArchivedPenaltyL2::from_full::<T>(cursor, &record);
                        
                        // 计算年月（简化：使用区块号除以每月区块数）
                        let block_num: u32 = record.deducted_at.saturated_into();
                        let year_month = block_num / (30 * 14400); // 约30天
                        
                        // 添加到归档存储
                        ArchivedPenalties::<T>::mutate(year_month, |list| {
                            let _ = list.try_push(archived);
                        });
                        
                        // 删除完整记录
                        PenaltyRecords::<T>::remove(cursor);
                        
                        // 从做市商的惩罚列表中移除
                        MakerPenalties::<T>::mutate(record.maker_id, |ids| {
                            ids.retain(|&id| id != cursor);
                        });
                        
                        archived_count = archived_count.saturating_add(1);
                        
                        // 发出归档事件
                        Self::deposit_event(Event::PenaltyArchived {
                            penalty_id: cursor,
                            maker_id: record.maker_id,
                            year_month,
                        });
                    }
                }

                cursor = cursor.saturating_add(1);
            }

            // 更新游标
            PenaltyArchiveCursor::<T>::put(cursor);

            // 返回消耗的权重
            Weight::from_parts(
                (archived_count as u64) * 80_000 + 10_000,
                0
            )
        }
    }

    // ===== 公共查询接口 =====
    
    impl<T: Config> Pallet<T> {
        /// 函数级详细中文注释：检查账户是否是做市商
        pub fn is_maker(who: &T::AccountId) -> bool {
            AccountToMaker::<T>::contains_key(who)
        }
        
        /// 函数级详细中文注释：检查做市商是否活跃
        pub fn is_maker_active(maker_id: u64) -> bool {
            if let Some(app) = MakerApplications::<T>::get(maker_id) {
                app.status == ApplicationStatus::Active && !app.service_paused
            } else {
                false
            }
        }
        
        /// 函数级详细中文注释：获取做市商ID（通过账户）
        pub fn get_maker_id(who: &T::AccountId) -> Option<u64> {
            AccountToMaker::<T>::get(who)
        }
    }
}
