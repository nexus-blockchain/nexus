//! # Commission Pool Reward Plugin (pallet-commission-pool-reward) — v2
//!
//! 沉淀池奖励插件：周期性等额分配模型（Periodic Equal-Share Claim）。
//!
//! ## 核心逻辑
//!
//! 当 `POOL_REWARD` 模式启用后，未分配佣金自动沉淀入 Entity 级沉淀池（Phase 1.5，由 core 管理）。
//! 每隔 `round_duration` 区块为一轮。首个 claim 触发新轮快照，记录池余额和各等级会员数。
//! 用户在轮次窗口内签名调用 `claim_pool_reward` 领取属于自己等级的份额。
//! 未领取的金额留在池中，下一轮自然包含。
//!
//! ## Entity Owner 不可提取
//!
//! 沉淀池资金完全由算法驱动分配，Entity Owner 无法直接提取。

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get},
    };
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        MemberProvider, PoolBalanceProvider, TokenPoolBalanceProvider,
        TokenTransferProvider as TokenTransferProviderT,
        ParticipationGuard,
    };
    use pallet_entity_common::{EntityProvider, AdminPermission};
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;

    /// F12: 池奖励领取回调（将 claim 记录写入 commission-core 的统一佣金体系）
    pub trait PoolRewardClaimCallback<AccountId, Balance, TokenBalance> {
        fn on_pool_reward_claimed(
            entity_id: u64,
            account: &AccountId,
            nex_amount: Balance,
            token_amount: TokenBalance,
            round_id: u64,
            level_id: u8,
        );
    }

    /// 空回调实现
    impl<AccountId, Balance, TokenBalance> PoolRewardClaimCallback<AccountId, Balance, TokenBalance> for () {
        fn on_pool_reward_claimed(_: u64, _: &AccountId, _: Balance, _: TokenBalance, _: u64, _: u8) {}
    }

    // ========================================================================
    // Data structs
    // ========================================================================

    /// 沉淀池奖励配置（per-entity, v2）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct PoolRewardConfig<MaxLevels: Get<u32>, BlockNumber> {
        /// 各等级分配比率（基点），sum 必须等于 10000
        /// (level_id, ratio_bps)
        pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
        /// 轮次持续时间（区块数）
        pub round_duration: BlockNumber,
        /// 是否启用 Entity Token 池分配（默认 false）
        pub token_pool_enabled: bool,
    }

    pub type PoolRewardConfigOf<T> = PoolRewardConfig<
        <T as Config>::MaxPoolRewardLevels,
        BlockNumberFor<T>,
    >;

    /// 等级快照
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct LevelSnapshot<Balance> {
        pub level_id: u8,
        /// 快照时该等级会员数量
        pub member_count: u32,
        /// 每人可领取数量
        pub per_member_reward: Balance,
        /// 已领取人数
        pub claimed_count: u32,
    }

    /// 轮次快照数据（per-entity）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct RoundInfo<MaxLevels: Get<u32>, Balance, TokenBalance, BlockNumber> {
        /// 轮次 ID（单调递增）
        pub round_id: u64,
        /// 轮次开始区块
        pub start_block: BlockNumber,
        /// 快照时池余额
        pub pool_snapshot: Balance,
        /// 各等级快照
        pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
        /// Token 池快照（None = 未启用 Token 池）
        pub token_pool_snapshot: Option<TokenBalance>,
        /// Token 各等级快照
        pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
    }

    pub type RoundInfoOf<T> = RoundInfo<
        <T as Config>::MaxPoolRewardLevels,
        BalanceOf<T>,
        TokenBalanceOf<T>,
        BlockNumberFor<T>,
    >;

    /// 领取记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ClaimRecord<Balance, TokenBalance, BlockNumber> {
        /// 领取的轮次 ID
        pub round_id: u64,
        /// 领取数量
        pub amount: Balance,
        /// 领取时的会员等级
        pub level_id: u8,
        /// 领取时区块高度
        pub claimed_at: BlockNumber,
        /// Token 领取数量（0 = 无 Token 奖励）
        pub token_amount: TokenBalance,
    }

    pub type ClaimRecordOf<T> = ClaimRecord<BalanceOf<T>, TokenBalanceOf<T>, BlockNumberFor<T>>;

    /// F10: 已完成轮次摘要（写入 RoundHistory）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct CompletedRoundSummary<MaxLevels: Get<u32>, Balance, TokenBalance, BlockNumber> {
        pub round_id: u64,
        pub start_block: BlockNumber,
        pub end_block: BlockNumber,
        pub pool_snapshot: Balance,
        pub token_pool_snapshot: Option<TokenBalance>,
        /// 各等级快照（含 claimed_count 终态）
        pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
        pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
    }

    pub type CompletedRoundSummaryOf<T> = CompletedRoundSummary<
        <T as Config>::MaxPoolRewardLevels,
        BalanceOf<T>,
        TokenBalanceOf<T>,
        BlockNumberFor<T>,
    >;

    /// F9: 累计分配统计（per-entity）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DistributionStats<Balance: Default, TokenBalance: Default> {
        /// 累计分配 NEX 总量
        pub total_nex_distributed: Balance,
        /// 累计分配 Token 总量
        pub total_token_distributed: TokenBalance,
        /// 累计完成轮次数
        pub total_rounds_completed: u64,
        /// 累计领取次数
        pub total_claims: u64,
    }

    pub type DistributionStatsOf<T> = DistributionStats<BalanceOf<T>, TokenBalanceOf<T>>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// Entity 查询接口（获取 entity_account 用于转账）
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 池余额读写接口（访问 commission-core 的 UnallocatedPool）
        type PoolBalanceProvider: PoolBalanceProvider<BalanceOf<Self>>;

        /// 最大等级配置数
        #[pallet::constant]
        type MaxPoolRewardLevels: Get<u32>;

        /// 每用户最大领取历史记录数
        #[pallet::constant]
        type MaxClaimHistory: Get<u32>;

        // ====================================================================
        // Token 多资产扩展
        // ====================================================================

        /// Entity Token 余额类型
        type TokenBalance: codec::FullCodec
            + codec::MaxEncodedLen
            + TypeInfo
            + Copy
            + Default
            + core::fmt::Debug
            + sp_runtime::traits::AtLeast32BitUnsigned
            + From<u32>
            + Into<u128>;

        /// Token 沉淀池余额读写（访问 commission-core 的 UnallocatedTokenPool）
        type TokenPoolBalanceProvider: TokenPoolBalanceProvider<TokenBalanceOf<Self>>;

        /// Token 转账接口（entity_id 级）
        type TokenTransferProvider: TokenTransferProviderT<Self::AccountId, TokenBalanceOf<Self>>;

        /// Entity 参与权守卫（KYC / 合规检查）
        /// 默认使用 `()` 允许所有操作（无 KYC 要求）
        type ParticipationGuard: ParticipationGuard<Self::AccountId>;

        /// 权重信息（L1 审计修复: 替换硬编码 Weight）
        type WeightInfo: WeightInfo;

        /// F4: 最小轮次持续时间（区块数）
        #[pallet::constant]
        type MinRoundDuration: Get<BlockNumberFor<Self>>;

        /// F10: 每个 Entity 最大轮次历史记录数
        #[pallet::constant]
        type MaxRoundHistory: Get<u32>;

        /// F12: 池奖励领取回调（将 claim 记录写入 commission-core 统一体系）
        type ClaimCallback: PoolRewardClaimCallback<Self::AccountId, BalanceOf<Self>, TokenBalanceOf<Self>>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// F7: 运行时配置校验
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(
                T::MaxPoolRewardLevels::get() >= 1,
                "MaxPoolRewardLevels must be >= 1"
            );
            assert!(
                T::MaxClaimHistory::get() >= 1,
                "MaxClaimHistory must be >= 1"
            );
            assert!(
                T::MinRoundDuration::get() > BlockNumberFor::<T>::zero(),
                "MinRoundDuration must be > 0"
            );
            assert!(
                T::MaxRoundHistory::get() >= 1,
                "MaxRoundHistory must be >= 1"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// 沉淀池奖励配置 entity_id -> PoolRewardConfig
    #[pallet::storage]
    #[pallet::getter(fn pool_reward_config)]
    pub type PoolRewardConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        PoolRewardConfigOf<T>,
    >;

    /// 当前轮次快照数据 entity_id -> RoundInfo
    #[pallet::storage]
    #[pallet::getter(fn current_round)]
    pub type CurrentRound<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        RoundInfoOf<T>,
    >;

    /// 上一轮次 ID（配置变更后保留，用于保持 round_id 单调递增）
    /// M2-R3 审计修复: 消除 set_pool_reward_config 中的 clear_prefix(LastClaimedRound)
    #[pallet::storage]
    pub type LastRoundId<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, ValueQuery>;

    /// 用户最后领取的轮次 ID（防双领）
    #[pallet::storage]
    #[pallet::getter(fn last_claimed_round)]
    pub type LastClaimedRound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        u64,
        ValueQuery,
    >;

    /// 用户领取历史
    #[pallet::storage]
    #[pallet::getter(fn claim_records)]
    pub type ClaimRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BoundedVec<ClaimRecordOf<T>, T::MaxClaimHistory>,
        ValueQuery,
    >;

    /// F3: 沉淀池奖励暂停状态（per-entity）
    #[pallet::storage]
    #[pallet::getter(fn pool_reward_paused)]
    pub type PoolRewardPaused<T: Config> = StorageMap<_, Blake2_128Concat, u64, bool, ValueQuery>;

    /// F8: 全局紧急暂停
    #[pallet::storage]
    #[pallet::getter(fn global_pool_reward_paused)]
    pub type GlobalPoolRewardPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// F10: 轮次历史存储 (entity_id -> BoundedVec<CompletedRoundSummary>)
    #[pallet::storage]
    #[pallet::getter(fn round_history)]
    pub type RoundHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BoundedVec<CompletedRoundSummaryOf<T>, T::MaxRoundHistory>,
        ValueQuery,
    >;

    /// F9: 累计分配统计（per-entity）
    #[pallet::storage]
    #[pallet::getter(fn distribution_stats)]
    pub type DistributionStatistics<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        DistributionStatsOf<T>,
        ValueQuery,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 配置更新
        PoolRewardConfigUpdated { entity_id: u64 },
        /// 新轮次开始，快照已创建
        NewRoundStarted {
            entity_id: u64,
            round_id: u64,
            pool_snapshot: BalanceOf<T>,
            token_pool_snapshot: Option<TokenBalanceOf<T>>,
        },
        /// 用户成功领取池奖励
        PoolRewardClaimed {
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
            token_amount: TokenBalanceOf<T>,
            round_id: u64,
            level_id: u8,
        },
        /// Token 池启用/禁用
        TokenPoolEnabledUpdated { entity_id: u64, enabled: bool },
        /// 管理员强制开启新轮次
        RoundForced { entity_id: u64, round_id: u64 },
        /// Token 回滚转账失败（可能导致 token 泄漏，需人工干预）
        TokenTransferRollbackFailed {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
        /// M3-R5: 沉淀池奖励配置已清除（区别于更新）
        PoolRewardConfigCleared { entity_id: u64 },
        /// F3: 沉淀池奖励暂停
        PoolRewardPausedEvent { entity_id: u64 },
        /// F3: 沉淀池奖励恢复
        PoolRewardResumedEvent { entity_id: u64 },
        /// F8: 全局暂停
        GlobalPoolRewardPausedEvent,
        /// F8: 全局恢复
        GlobalPoolRewardResumedEvent,
        /// F10: 轮次历史已归档
        RoundArchived { entity_id: u64, round_id: u64 },
        /// F11: 新轮次快照详情（含各等级 per_member_reward / member_count）
        NewRoundDetails {
            entity_id: u64,
            round_id: u64,
            pool_snapshot: BalanceOf<T>,
            token_pool_snapshot: Option<TokenBalanceOf<T>>,
            level_snapshots: alloc::vec::Vec<(u8, u32, BalanceOf<T>)>,
            token_level_snapshots: Option<alloc::vec::Vec<(u8, u32, TokenBalanceOf<T>)>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 单个比率超出 (0, 10000] 范围
        InvalidRatio,
        /// 所有等级比率之和不等于 10000
        RatioSumMismatch,
        /// 配置中存在重复的 level_id
        DuplicateLevelId,
        /// round_duration 为 0
        InvalidRoundDuration,
        /// 调用者不是该 Entity 的会员
        NotMember,
        /// 用户等级未在配置中或比率为 0
        LevelNotConfigured,
        /// 本轮已领取过
        AlreadyClaimed,
        /// 该等级本轮领取名额已满
        LevelQuotaExhausted,
        /// 可领取金额为 0
        NothingToClaim,
        /// 沉淀池余额不足
        InsufficientPool,
        /// Entity 未配置沉淀池奖励
        ConfigNotFound,
        /// Entity 不存在或未激活
        EntityNotActive,
        /// 等级未在快照中
        LevelNotInSnapshot,
        /// round_id 已达 u64::MAX，无法创建新轮次
        RoundIdOverflow,
        /// 账户未满足 Entity 参与要求（如 KYC）
        ParticipationRequirementNotMet,
        /// 调用者不是 Entity Owner 或授权管理员
        NotAuthorized,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// F3: 沉淀池奖励已暂停
        PoolRewardIsPaused,
        /// F3: 沉淀池奖励未暂停
        PoolRewardNotPaused,
        /// F4: 轮次持续时间低于最小值
        RoundDurationTooShort,
        /// F8: 全局暂停中
        GlobalPaused,
        /// F8: 全局未暂停
        GlobalNotPaused,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置沉淀池奖励配置（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_pool_reward_config())]
        pub fn set_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            ensure!(round_duration > BlockNumberFor::<T>::zero(), Error::<T>::InvalidRoundDuration);
            // F4: 最小轮次间隔校验
            ensure!(round_duration >= T::MinRoundDuration::get(), Error::<T>::RoundDurationTooShort);
            Self::validate_level_ratios(&level_ratios)?;

            // H2: preserve existing token_pool_enabled
            let token_pool_enabled = PoolRewardConfigs::<T>::get(entity_id)
                .map(|c| c.token_pool_enabled)
                .unwrap_or(false);

            PoolRewardConfigs::<T>::insert(entity_id, PoolRewardConfig {
                level_ratios,
                round_duration,
                token_pool_enabled,
            });

            // H2 审计修复: 配置变更时使当前轮次失效，强制下次 claim 创建新快照
            // M2-R3 审计修复: 用 invalidate_current_round 保持 round_id 单调递增，
            //   消除 clear_prefix(LastClaimedRound) 的 O(n) 无界写入
            Self::invalidate_current_round(entity_id);

            Self::deposit_event(Event::PoolRewardConfigUpdated { entity_id });
            Ok(())
        }

        /// 用户领取沉淀池奖励（NEX + Token 双池统一入口）
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::claim_pool_reward())]
        pub fn claim_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // M2 审计修复: Entity 激活状态检查（Banned/Closed Entity 不应分配池奖励）
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            // F8: 全局暂停检查
            ensure!(!GlobalPoolRewardPaused::<T>::get(), Error::<T>::GlobalPaused);
            // F3: per-entity 暂停检查
            ensure!(!PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardIsPaused);

            // 1. 资格检查
            ensure!(T::MemberProvider::is_member(entity_id, &who), Error::<T>::NotMember);
            // M1-R8: 封禁或冻结/暂停的会员不可领取池奖励
            ensure!(!T::MemberProvider::is_banned(entity_id, &who), Error::<T>::NotMember);
            ensure!(T::MemberProvider::is_member_active(entity_id, &who), Error::<T>::NotMember);

            // PR-H1 审计修复: 池奖励领取需检查参与权（与 withdraw_commission 一致）
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, &who),
                Error::<T>::ParticipationRequirementNotMet
            );

            let config = PoolRewardConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            let user_level = T::MemberProvider::custom_level_id(entity_id, &who);

            // 检查用户等级是否在配置中
            ensure!(
                config.level_ratios.iter().any(|(id, _)| *id == user_level),
                Error::<T>::LevelNotConfigured
            );

            // 2. 轮次检查 / 创建
            let now = <frame_system::Pallet<T>>::block_number();
            let mut round = Self::ensure_current_round(entity_id, &config, now)?;

            // 3. 防双领
            let last_round = LastClaimedRound::<T>::get(entity_id, &who);
            ensure!(last_round < round.round_id, Error::<T>::AlreadyClaimed);

            // 4. 查找 NEX 等级快照（先获取索引，避免借用冲突）
            let nex_snap_idx = round.level_snapshots.iter()
                .position(|s| s.level_id == user_level)
                .ok_or(Error::<T>::LevelNotInSnapshot)?;

            // 5. NEX 配额 + 金额检查
            let reward = {
                let snapshot = &round.level_snapshots[nex_snap_idx];
                ensure!(snapshot.claimed_count < snapshot.member_count, Error::<T>::LevelQuotaExhausted);
                let r = snapshot.per_member_reward;
                ensure!(!r.is_zero(), Error::<T>::NothingToClaim);
                r
            };

            // 6. NEX 池偿付：先扣记账、后转实物
            let pool = T::PoolBalanceProvider::pool_balance(entity_id);
            ensure!(pool >= reward, Error::<T>::InsufficientPool);
            // M3-R2 审计修复: deduct_pool 在 transfer 之前，符合「先扣记账、后转实物」最佳实践
            T::PoolBalanceProvider::deduct_pool(entity_id, reward)?;
            let entity_account = T::EntityProvider::entity_account(entity_id);
            T::Currency::transfer(&entity_account, &who, reward, ExistenceRequirement::KeepAlive)?;

            // 7. Token 部分（best-effort：失败不影响 NEX 领取）
            // 注：Token 采用 transfer-first 顺序，因为 best-effort 路径不走 `?`
            //     不会触发 Substrate 事务回滚，且无 "add_back" 接口回滚记账扣减
            let mut token_reward = TokenBalanceOf::<T>::default();
            if let Some(ref mut token_snapshots) = round.token_level_snapshots {
                if let Some(token_snap) = token_snapshots.iter_mut().find(|s| s.level_id == user_level) {
                    if token_snap.claimed_count < token_snap.member_count
                        && !token_snap.per_member_reward.is_zero()
                    {
                        let tr = token_snap.per_member_reward;
                        let token_pool = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                        if token_pool >= tr {
                            if T::TokenTransferProvider::token_transfer(
                                entity_id, &entity_account, &who, tr,
                            ).is_ok() {
                                if T::TokenPoolBalanceProvider::deduct_token_pool(entity_id, tr).is_ok() {
                                    token_snap.claimed_count += 1;
                                    token_reward = tr;
                                } else {
                                    // 池扣减失败，回滚转账
                                    if T::TokenTransferProvider::token_transfer(
                                        entity_id, &who, &entity_account, tr,
                                    ).is_err() {
                                        Self::deposit_event(Event::TokenTransferRollbackFailed {
                                            entity_id,
                                            account: who.clone(),
                                            amount: tr,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 8. 状态更新
            let round_id = round.round_id;
            round.level_snapshots[nex_snap_idx].claimed_count += 1;
            CurrentRound::<T>::insert(entity_id, round);
            LastClaimedRound::<T>::insert(entity_id, &who, round_id);

            // 9. 写入领取历史
            ClaimRecords::<T>::mutate(entity_id, &who, |history| {
                let record = ClaimRecord {
                    round_id,
                    amount: reward,
                    level_id: user_level,
                    claimed_at: now,
                    token_amount: token_reward,
                };
                if history.is_full() {
                    history.remove(0);
                }
                let _ = history.try_push(record);
            });

            // F9: 累计分配统计更新
            DistributionStatistics::<T>::mutate(entity_id, |stats| {
                stats.total_nex_distributed = stats.total_nex_distributed.saturating_add(reward);
                stats.total_token_distributed = stats.total_token_distributed.saturating_add(token_reward);
                stats.total_claims = stats.total_claims.saturating_add(1);
            });

            // F12: 池奖励领取回调（写入 commission-core 统一记录）
            T::ClaimCallback::on_pool_reward_claimed(
                entity_id, &who, reward, token_reward, round_id, user_level,
            );

            Self::deposit_event(Event::PoolRewardClaimed {
                entity_id,
                account: who,
                amount: reward,
                token_amount: token_reward,
                round_id,
                level_id: user_level,
            });

            Ok(())
        }

        /// 开启新轮次（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::force_new_round())]
        pub fn force_new_round(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let config = PoolRewardConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let round = Self::create_new_round(entity_id, &config, now)?;

            Self::deposit_event(Event::RoundForced {
                entity_id,
                round_id: round.round_id,
            });

            Ok(())
        }

        /// 启用/禁用 Entity Token 池分配（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_token_pool_enabled())]
        pub fn set_token_pool_enabled(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            let mut changed = false;
            PoolRewardConfigs::<T>::try_mutate(entity_id, |maybe| -> DispatchResult {
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                if config.token_pool_enabled != enabled {
                    config.token_pool_enabled = enabled;
                    changed = true;
                }
                Ok(())
            })?;
            // M1-R3 审计修复: token 启用/禁用立即生效，使当前轮次失效
            // L1-R4 审计修复: 仅在值实际变更时才失效轮次，避免幂等调用浪费快照
            if changed {
                Self::invalidate_current_round(entity_id);
            }
            Self::deposit_event(Event::TokenPoolEnabledUpdated { entity_id, enabled });
            Ok(())
        }

        // ===== Root force_* 紧急覆写 extrinsics =====

        /// [Root] 强制设置沉淀池奖励配置（绕过 Owner/Admin 权限和 EntityLocked 检查）
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_pool_reward_config())]
        pub fn force_set_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(round_duration > BlockNumberFor::<T>::zero(), Error::<T>::InvalidRoundDuration);
            // F4: 最小轮次间隔校验
            ensure!(round_duration >= T::MinRoundDuration::get(), Error::<T>::RoundDurationTooShort);
            Self::validate_level_ratios(&level_ratios)?;

            let token_pool_enabled = PoolRewardConfigs::<T>::get(entity_id)
                .map(|c| c.token_pool_enabled)
                .unwrap_or(false);

            PoolRewardConfigs::<T>::insert(entity_id, PoolRewardConfig {
                level_ratios,
                round_duration,
                token_pool_enabled,
            });

            Self::invalidate_current_round(entity_id);
            Self::deposit_event(Event::PoolRewardConfigUpdated { entity_id });
            Ok(())
        }

        /// [Root] 强制启用/禁用 Token 池分配（绕过 Owner/Admin 权限和 EntityLocked 检查）
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::set_token_pool_enabled())]
        pub fn force_set_token_pool_enabled(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let mut changed = false;
            PoolRewardConfigs::<T>::try_mutate(entity_id, |maybe| -> DispatchResult {
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                if config.token_pool_enabled != enabled {
                    config.token_pool_enabled = enabled;
                    changed = true;
                }
                Ok(())
            })?;
            if changed {
                Self::invalidate_current_round(entity_id);
            }
            Self::deposit_event(Event::TokenPoolEnabledUpdated { entity_id, enabled });
            Ok(())
        }

        /// 清除沉淀池奖励配置（Entity Owner / Admin(COMMISSION_MANAGE)）
        ///
        /// 仅移除配置并使当前轮次失效，不清理历史领取记录。
        /// 完整清理（含 LastClaimedRound / ClaimRecords）请使用 PoolRewardPlanWriter::clear_config。
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::clear_pool_reward_config())]
        pub fn clear_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            PoolRewardConfigs::<T>::remove(entity_id);
            // M1-R7: 同步清除暂停状态，防止 re-create 后残留
            PoolRewardPaused::<T>::remove(entity_id);
            Self::invalidate_current_round(entity_id);
            Self::deposit_event(Event::PoolRewardConfigCleared { entity_id });
            Ok(())
        }

        /// [Root] 强制清除沉淀池奖励配置（绕过 Owner/Admin 权限和 EntityLocked 检查）
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::clear_pool_reward_config())]
        pub fn force_clear_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // X2: 仅配置存在时才 remove + emit，防止幻影事件
            if PoolRewardConfigs::<T>::contains_key(entity_id) {
                PoolRewardConfigs::<T>::remove(entity_id);
                // M1-R7: 同步清除暂停状态，防止 re-create 后残留
                PoolRewardPaused::<T>::remove(entity_id);
                Self::invalidate_current_round(entity_id);
                Self::deposit_event(Event::PoolRewardConfigCleared { entity_id });
            }
            Ok(())
        }

        /// [Root] 强制开启新轮次（绕过 Owner/Admin 权限和 EntityLocked 检查）
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::force_new_round())]
        pub fn force_start_new_round(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);

            let config = PoolRewardConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let round = Self::create_new_round(entity_id, &config, now)?;

            Self::deposit_event(Event::RoundForced {
                entity_id,
                round_id: round.round_id,
            });

            Ok(())
        }

        /// F3: 暂停沉淀池奖励分配（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::pause_pool_reward())]
        pub fn pause_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(!PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardIsPaused);

            PoolRewardPaused::<T>::insert(entity_id, true);
            Self::deposit_event(Event::PoolRewardPausedEvent { entity_id });
            Ok(())
        }

        /// F3: 恢复沉淀池奖励分配（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::resume_pool_reward())]
        pub fn resume_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardNotPaused);

            PoolRewardPaused::<T>::remove(entity_id);
            Self::deposit_event(Event::PoolRewardResumedEvent { entity_id });
            Ok(())
        }

        /// F8: 全局紧急暂停/恢复（Root only）
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::set_global_pool_reward_paused())]
        pub fn set_global_pool_reward_paused(
            origin: OriginFor<T>,
            paused: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if paused {
                ensure!(!GlobalPoolRewardPaused::<T>::get(), Error::<T>::GlobalPaused);
                GlobalPoolRewardPaused::<T>::put(true);
                Self::deposit_event(Event::GlobalPoolRewardPausedEvent);
            } else {
                ensure!(GlobalPoolRewardPaused::<T>::get(), Error::<T>::GlobalNotPaused);
                GlobalPoolRewardPaused::<T>::kill();
                Self::deposit_event(Event::GlobalPoolRewardResumedEvent);
            }
            Ok(())
        }
    }

    // ========================================================================
    // Internal logic
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 确保调用者是 Entity Owner 或拥有 COMMISSION_MANAGE 权限的管理员
        fn ensure_owner_or_admin(who: &T::AccountId, entity_id: u64) -> DispatchResult {
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotActive)?;
            ensure!(
                *who == owner || T::EntityProvider::is_entity_admin(
                    entity_id, who, AdminPermission::COMMISSION_MANAGE
                ),
                Error::<T>::NotAuthorized
            );
            Ok(())
        }

        /// M2-R3 审计修复: 使当前轮次失效，保留 round_id 到 LastRoundId 保持单调递增
        pub(crate) fn invalidate_current_round(entity_id: u64) {
            if let Some(round) = CurrentRound::<T>::get(entity_id) {
                LastRoundId::<T>::insert(entity_id, round.round_id);
            }
            CurrentRound::<T>::remove(entity_id);
        }

        /// 校验等级比率配置：无重复 level_id、每个 ratio ∈ (0, 10000]、总和 = 10000
        pub(crate) fn validate_level_ratios(ratios: &[(u8, u16)]) -> DispatchResult {
            // M1 审计修复: O(n log n) BTreeSet 替代 O(n²) 嵌套循环
            {
                let mut seen_ids = alloc::collections::BTreeSet::new();
                for (level_id, _) in ratios.iter() {
                    ensure!(seen_ids.insert(*level_id), Error::<T>::DuplicateLevelId);
                }
            }
            let mut sum: u16 = 0;
            for (_, ratio) in ratios.iter() {
                ensure!(*ratio > 0 && *ratio <= 10000, Error::<T>::InvalidRatio);
                sum = sum.saturating_add(*ratio);
            }
            ensure!(sum == 10000, Error::<T>::RatioSumMismatch);
            Ok(())
        }

        /// 根据池余额和等级计数构建各等级快照（NEX / Token 通用）
        fn build_level_snapshots<B>(
            pool_balance: B,
            level_counts: &[(u8, u16, u32)],
        ) -> BoundedVec<LevelSnapshot<B>, T::MaxPoolRewardLevels>
        where
            B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
        {
            let mut snapshots = BoundedVec::default();
            for &(level_id, ratio, count) in level_counts.iter() {
                let per_member = if count > 0 && !pool_balance.is_zero() {
                    let ratio_balance: B = (ratio as u32).into();
                    let count_balance: B = count.into();
                    pool_balance.saturating_mul(ratio_balance) / 10000u32.into() / count_balance
                } else {
                    Zero::zero()
                };
                // M2 审计修复: try_push 失败时触发 defensive（不应发生，边界与 config 一致）
                if snapshots.try_push(LevelSnapshot {
                    level_id,
                    member_count: count,
                    per_member_reward: per_member,
                    claimed_count: 0,
                }).is_err() {
                    frame_support::defensive!("pool-reward: snapshot overflow in build_level_snapshots");
                }
            }
            snapshots
        }

        /// 确保当前轮次有效；若已过期或不存在则创建新轮
        fn ensure_current_round(
            entity_id: u64,
            config: &PoolRewardConfigOf<T>,
            now: BlockNumberFor<T>,
        ) -> Result<RoundInfoOf<T>, DispatchError> {
            if let Some(round) = CurrentRound::<T>::get(entity_id) {
                let end_block = round.start_block.saturating_add(config.round_duration);
                if now < end_block {
                    return Ok(round);
                }
            }
            Self::create_new_round(entity_id, config, now)
        }

        /// 创建新轮次快照
        fn create_new_round(
            entity_id: u64,
            config: &PoolRewardConfigOf<T>,
            now: BlockNumberFor<T>,
        ) -> Result<RoundInfoOf<T>, DispatchError> {
            let pool_balance = T::PoolBalanceProvider::pool_balance(entity_id);
            let old_round = CurrentRound::<T>::get(entity_id);
            let old_round_id = old_round.as_ref()
                .map(|r| r.round_id)
                .unwrap_or_else(|| LastRoundId::<T>::get(entity_id));

            // M1 审计修复: 防止 round_id 在 u64::MAX 时 saturating_add(1) 不变导致重复 ID
            frame_support::ensure!(old_round_id < u64::MAX, Error::<T>::RoundIdOverflow);

            // F10: 归档旧轮次到 RoundHistory
            if let Some(ref old) = old_round {
                let summary = CompletedRoundSummary {
                    round_id: old.round_id,
                    start_block: old.start_block,
                    end_block: now,
                    pool_snapshot: old.pool_snapshot,
                    token_pool_snapshot: old.token_pool_snapshot,
                    level_snapshots: old.level_snapshots.clone(),
                    token_level_snapshots: old.token_level_snapshots.clone(),
                };
                RoundHistory::<T>::mutate(entity_id, |history| {
                    if history.is_full() {
                        history.remove(0);
                    }
                    let _ = history.try_push(summary);
                });
                Self::deposit_event(Event::RoundArchived {
                    entity_id,
                    round_id: old.round_id,
                });
                // F9: 轮次完成统计
                DistributionStatistics::<T>::mutate(entity_id, |stats| {
                    stats.total_rounds_completed = stats.total_rounds_completed.saturating_add(1);
                });
            }

            // 缓存各等级会员数，避免 Token 快照重复读取 storage
            let level_counts: alloc::vec::Vec<(u8, u16, u32)> = config.level_ratios.iter()
                .map(|(level_id, ratio)| {
                    let count = T::MemberProvider::member_count_by_level(entity_id, *level_id);
                    (*level_id, *ratio, count)
                })
                .collect();

            let level_snapshots = Self::build_level_snapshots(pool_balance, &level_counts);

            // Token 快照（仅当 token_pool_enabled = true）
            let (token_pool_snapshot, token_level_snapshots) = if config.token_pool_enabled {
                let token_balance = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                (Some(token_balance), Some(Self::build_level_snapshots(token_balance, &level_counts)))
            } else {
                (None, None)
            };

            let new_round = RoundInfo {
                round_id: old_round_id.saturating_add(1),
                start_block: now,
                pool_snapshot: pool_balance,
                level_snapshots,
                token_pool_snapshot,
                token_level_snapshots,
            };

            CurrentRound::<T>::insert(entity_id, &new_round);

            Self::deposit_event(Event::NewRoundStarted {
                entity_id,
                round_id: new_round.round_id,
                pool_snapshot: pool_balance,
                token_pool_snapshot,
            });

            // F11: 发出详细轮次快照事件
            let level_details: alloc::vec::Vec<(u8, u32, BalanceOf<T>)> = new_round.level_snapshots.iter()
                .map(|s| (s.level_id, s.member_count, s.per_member_reward))
                .collect();
            let token_level_details = new_round.token_level_snapshots.as_ref().map(|snaps| {
                snaps.iter().map(|s| (s.level_id, s.member_count, s.per_member_reward)).collect()
            });
            Self::deposit_event(Event::NewRoundDetails {
                entity_id,
                round_id: new_round.round_id,
                pool_snapshot: pool_balance,
                token_pool_snapshot,
                level_snapshots: level_details,
                token_level_snapshots: token_level_details,
            });

            Ok(new_round)
        }

        // F1: 可领取金额预查询
        /// 返回 (nex_claimable, token_claimable)，如果不可领取返回 (0, 0)
        pub fn get_claimable(
            entity_id: u64,
            who: &T::AccountId,
        ) -> (BalanceOf<T>, TokenBalanceOf<T>) {
            let zero_nex = BalanceOf::<T>::zero();
            let zero_token = TokenBalanceOf::<T>::default();

            // 检查暂停状态
            if GlobalPoolRewardPaused::<T>::get() || PoolRewardPaused::<T>::get(entity_id) {
                return (zero_nex, zero_token);
            }

            // 检查 Entity 激活
            if !T::EntityProvider::is_entity_active(entity_id) {
                return (zero_nex, zero_token);
            }

            // 检查会员资格
            if !T::MemberProvider::is_member(entity_id, who) {
                return (zero_nex, zero_token);
            }
            // M1-R8: 封禁或冻结/暂停的会员不可领取
            if T::MemberProvider::is_banned(entity_id, who)
                || !T::MemberProvider::is_member_active(entity_id, who)
            {
                return (zero_nex, zero_token);
            }

            // 检查参与权
            if !T::ParticipationGuard::can_participate(entity_id, who) {
                return (zero_nex, zero_token);
            }

            let config = match PoolRewardConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return (zero_nex, zero_token),
            };

            let user_level = T::MemberProvider::custom_level_id(entity_id, who);
            if !config.level_ratios.iter().any(|(id, _)| *id == user_level) {
                return (zero_nex, zero_token);
            }

            // 获取或预测轮次
            let now = <frame_system::Pallet<T>>::block_number();
            let round = if let Some(r) = CurrentRound::<T>::get(entity_id) {
                let end_block = r.start_block.saturating_add(config.round_duration);
                if now < end_block {
                    r
                } else {
                    // 轮次已过期，需创建新轮次才能 claim
                    // 模拟新快照
                    return Self::simulate_claimable(entity_id, &config, user_level);
                }
            } else {
                return Self::simulate_claimable(entity_id, &config, user_level);
            };

            // 防双领检查
            let last_round = LastClaimedRound::<T>::get(entity_id, who);
            if last_round >= round.round_id {
                return (zero_nex, zero_token);
            }

            // NEX 快照查找
            let nex_reward = round.level_snapshots.iter()
                .find(|s| s.level_id == user_level)
                .and_then(|s| {
                    if s.claimed_count < s.member_count && !s.per_member_reward.is_zero() {
                        let pool = T::PoolBalanceProvider::pool_balance(entity_id);
                        if pool >= s.per_member_reward { Some(s.per_member_reward) } else { None }
                    } else {
                        None
                    }
                })
                .unwrap_or(zero_nex);

            // Token 快照查找
            let token_reward = round.token_level_snapshots.as_ref()
                .and_then(|snaps| snaps.iter().find(|s| s.level_id == user_level))
                .and_then(|s| {
                    if s.claimed_count < s.member_count && !s.per_member_reward.is_zero() {
                        let token_pool = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                        if token_pool >= s.per_member_reward { Some(s.per_member_reward) } else { None }
                    } else {
                        None
                    }
                })
                .unwrap_or(zero_token);

            (nex_reward, token_reward)
        }

        /// F1 辅助: 模拟新轮次的可领取金额（不写入 storage）
        fn simulate_claimable(
            entity_id: u64,
            config: &PoolRewardConfigOf<T>,
            user_level: u8,
        ) -> (BalanceOf<T>, TokenBalanceOf<T>) {
            let zero_nex = BalanceOf::<T>::zero();
            let zero_token = TokenBalanceOf::<T>::default();

            let pool_balance = T::PoolBalanceProvider::pool_balance(entity_id);
            if pool_balance.is_zero() {
                return (zero_nex, zero_token);
            }

            let level_counts: alloc::vec::Vec<(u8, u16, u32)> = config.level_ratios.iter()
                .map(|(level_id, ratio)| {
                    let count = T::MemberProvider::member_count_by_level(entity_id, *level_id);
                    (*level_id, *ratio, count)
                })
                .collect();

            let nex_reward = level_counts.iter()
                .find(|(id, _, _)| *id == user_level)
                .map(|&(_, ratio, count)| {
                    if count > 0 {
                        let ratio_b: BalanceOf<T> = (ratio as u32).into();
                        let count_b: BalanceOf<T> = count.into();
                        pool_balance.saturating_mul(ratio_b) / 10000u32.into() / count_b
                    } else {
                        zero_nex
                    }
                })
                .unwrap_or(zero_nex);

            let token_reward = if config.token_pool_enabled {
                let token_balance = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                if token_balance.is_zero() {
                    zero_token
                } else {
                    level_counts.iter()
                        .find(|(id, _, _)| *id == user_level)
                        .map(|&(_, ratio, count)| {
                            if count > 0 {
                                let ratio_b: TokenBalanceOf<T> = (ratio as u32).into();
                                let count_b: TokenBalanceOf<T> = count.into();
                                token_balance.saturating_mul(ratio_b) / 10000u32.into() / count_b
                            } else {
                                zero_token
                            }
                        })
                        .unwrap_or(zero_token)
                }
            } else {
                zero_token
            };

            (nex_reward, token_reward)
        }

        // F5: 轮次领取进度查询
        /// 返回各等级 (level_id, member_count, claimed_count, per_member_reward)
        pub fn get_round_statistics(entity_id: u64) -> Option<alloc::vec::Vec<(u8, u32, u32, BalanceOf<T>)>> {
            CurrentRound::<T>::get(entity_id).map(|round| {
                round.level_snapshots.iter()
                    .map(|s| (s.level_id, s.member_count, s.claimed_count, s.per_member_reward))
                    .collect()
            })
        }
    }
}

// ============================================================================
// PoolRewardPlanWriter implementation (v2)
// ============================================================================

use frame_support::traits::Get as _;

impl<T: pallet::Config> pallet_commission_common::PoolRewardPlanWriter for pallet::Pallet<T> {
    fn set_pool_reward_config(
        entity_id: u64,
        level_ratios: alloc::vec::Vec<(u8, u16)>,
        round_duration: u32,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(
            round_duration > 0,
            sp_runtime::DispatchError::Other("InvalidRoundDuration")
        );

        let bounded: frame_support::BoundedVec<(u8, u16), T::MaxPoolRewardLevels> = level_ratios
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;

        pallet::Pallet::<T>::validate_level_ratios(&bounded)?;

        let rd: frame_system::pallet_prelude::BlockNumberFor<T> = round_duration.into();

        // F4: 最小轮次间隔校验
        frame_support::ensure!(
            rd >= T::MinRoundDuration::get(),
            sp_runtime::DispatchError::Other("RoundDurationTooShort")
        );

        // H2: preserve existing token_pool_enabled
        let token_pool_enabled = pallet::PoolRewardConfigs::<T>::get(entity_id)
            .map(|c| c.token_pool_enabled)
            .unwrap_or(false);

        pallet::PoolRewardConfigs::<T>::insert(entity_id, pallet::PoolRewardConfig {
            level_ratios: bounded,
            round_duration: rd,
            token_pool_enabled,
        });

        // M2-R3 审计修复: 用 invalidate_current_round 保持 round_id 单调递增
        pallet::Pallet::<T>::invalidate_current_round(entity_id);

        // L1-R3 审计修复: PlanWriter 路径也需 emit 事件，供 off-chain indexer 感知
        Self::deposit_event(pallet::Event::PoolRewardConfigUpdated { entity_id });

        Ok(())
    }

    /// L2-R4 注意: clear_prefix(u32::MAX) 写入量 O(n)，n = entity 下会员数。
    /// 调用方（如 governance pallet）需在自身 weight 中计入此开销。
    /// 此操作仅在 Entity 完全删除/停用时调用，频率极低，故保留。
    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::PoolRewardConfigs::<T>::remove(entity_id);
        pallet::CurrentRound::<T>::remove(entity_id);
        pallet::LastRoundId::<T>::remove(entity_id);
        // H3: clear LastClaimedRound and ClaimRecords for this entity
        let _ = pallet::LastClaimedRound::<T>::clear_prefix(entity_id, u32::MAX, None);
        let _ = pallet::ClaimRecords::<T>::clear_prefix(entity_id, u32::MAX, None);
        // F3/F9/F10: 清除新增存储
        pallet::PoolRewardPaused::<T>::remove(entity_id);
        pallet::RoundHistory::<T>::remove(entity_id);
        pallet::DistributionStatistics::<T>::remove(entity_id);
        // M3-R5: 使用专用 Cleared 事件，区别于 Updated
        Self::deposit_event(pallet::Event::PoolRewardConfigCleared { entity_id });
        Ok(())
    }

    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> Result<(), sp_runtime::DispatchError> {
        let mut changed = false;
        pallet::PoolRewardConfigs::<T>::try_mutate(entity_id, |maybe| -> Result<(), sp_runtime::DispatchError> {
            let config = maybe.as_mut().ok_or(sp_runtime::DispatchError::Other("ConfigNotFound"))?;
            if config.token_pool_enabled != enabled {
                config.token_pool_enabled = enabled;
                changed = true;
            }
            Ok(())
        })?;
        // M1-R3 审计修复: token 启用/禁用立即生效
        // L1-R4 审计修复: 仅在值实际变更时才失效轮次
        if changed {
            pallet::Pallet::<T>::invalidate_current_round(entity_id);
        }
        // L1-R3 审计修复: emit 事件
        Self::deposit_event(pallet::Event::TokenPoolEnabledUpdated { entity_id, enabled });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
