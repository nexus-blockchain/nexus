//! # Commission Pool Reward Plugin (pallet-commission-pool-reward)
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

pub mod runtime_api;
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
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

    impl<AccountId, Balance, TokenBalance> PoolRewardClaimCallback<AccountId, Balance, TokenBalance> for () {
        fn on_pool_reward_claimed(_: u64, _: &AccountId, _: Balance, _: TokenBalance, _: u64, _: u8) {}
    }

    // ========================================================================
    // Data structs
    // ========================================================================

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct PoolRewardConfig<MaxLevels: Get<u32>, BlockNumber> {
        pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
        pub round_duration: BlockNumber,
        pub token_pool_enabled: bool,
    }

    pub type PoolRewardConfigOf<T> = PoolRewardConfig<
        <T as Config>::MaxPoolRewardLevels,
        BlockNumberFor<T>,
    >;

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct LevelSnapshot<Balance> {
        pub level_id: u8,
        pub member_count: u32,
        pub per_member_reward: Balance,
        pub claimed_count: u32,
    }

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct RoundInfo<MaxLevels: Get<u32>, Balance, TokenBalance, BlockNumber> {
        pub round_id: u64,
        pub start_block: BlockNumber,
        pub pool_snapshot: Balance,
        pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
        pub token_pool_snapshot: Option<TokenBalance>,
        pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
    }

    pub type RoundInfoOf<T> = RoundInfo<
        <T as Config>::MaxPoolRewardLevels,
        BalanceOf<T>,
        TokenBalanceOf<T>,
        BlockNumberFor<T>,
    >;

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct ClaimRecord<Balance, TokenBalance, BlockNumber> {
        pub round_id: u64,
        pub amount: Balance,
        pub level_id: u8,
        pub claimed_at: BlockNumber,
        pub token_amount: TokenBalance,
    }

    pub type ClaimRecordOf<T> = ClaimRecord<BalanceOf<T>, TokenBalanceOf<T>, BlockNumberFor<T>>;

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct CompletedRoundSummary<MaxLevels: Get<u32>, Balance, TokenBalance, BlockNumber> {
        pub round_id: u64,
        pub start_block: BlockNumber,
        pub end_block: BlockNumber,
        pub pool_snapshot: Balance,
        pub token_pool_snapshot: Option<TokenBalance>,
        pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,
        pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
    }

    pub type CompletedRoundSummaryOf<T> = CompletedRoundSummary<
        <T as Config>::MaxPoolRewardLevels,
        BalanceOf<T>,
        TokenBalanceOf<T>,
        BlockNumberFor<T>,
    >;

    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct DistributionStats<Balance: Default, TokenBalance: Default> {
        pub total_nex_distributed: Balance,
        pub total_token_distributed: TokenBalance,
        pub total_rounds_completed: u64,
        pub total_claims: u64,
    }

    pub type DistributionStatsOf<T> = DistributionStats<BalanceOf<T>, TokenBalanceOf<T>>;

    /// 待生效的配置变更
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct PendingConfigChange<MaxLevels: Get<u32>, BlockNumber> {
        pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
        pub round_duration: BlockNumber,
        pub apply_after: BlockNumber,
    }

    pub type PendingConfigChangeOf<T> = PendingConfigChange<
        <T as Config>::MaxPoolRewardLevels,
        BlockNumberFor<T>,
    >;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;
        type EntityProvider: EntityProvider<Self::AccountId>;
        type PoolBalanceProvider: PoolBalanceProvider<BalanceOf<Self>>;

        #[pallet::constant]
        type MaxPoolRewardLevels: Get<u32>;

        #[pallet::constant]
        type MaxClaimHistory: Get<u32>;

        type TokenBalance: codec::FullCodec
            + codec::MaxEncodedLen
            + TypeInfo
            + Copy
            + Default
            + core::fmt::Debug
            + sp_runtime::traits::AtLeast32BitUnsigned
            + From<u32>
            + Into<u128>;

        type TokenPoolBalanceProvider: TokenPoolBalanceProvider<TokenBalanceOf<Self>>;
        type TokenTransferProvider: TokenTransferProviderT<Self::AccountId, TokenBalanceOf<Self>>;
        type ParticipationGuard: ParticipationGuard<Self::AccountId>;
        type WeightInfo: WeightInfo;

        #[pallet::constant]
        type MinRoundDuration: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type MaxRoundHistory: Get<u32>;

        type ClaimCallback: PoolRewardClaimCallback<Self::AccountId, BalanceOf<Self>, TokenBalanceOf<Self>>;

        /// 配置变更延迟（区块数）— 计划配置变更生效前的最小等待时间
        #[pallet::constant]
        type ConfigChangeDelay: Get<BlockNumberFor<Self>>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let base_weight = T::DbWeight::get().reads_writes(1, 1);
            let mut consumed = Weight::zero();
            let mut processed = 0u32;
            const MAX_PER_BLOCK: u32 = 5;

            let mut iter = TokenPoolDeficit::<T>::iter();
            while processed < MAX_PER_BLOCK {
                if consumed.saturating_add(base_weight).any_gt(remaining_weight) {
                    break;
                }
                let Some((entity_id, deficit)) = iter.next() else { break };
                if deficit.is_zero() {
                    TokenPoolDeficit::<T>::remove(entity_id);
                    consumed = consumed.saturating_add(base_weight);
                    processed += 1;
                    continue;
                }
                if T::TokenPoolBalanceProvider::deduct_token_pool(entity_id, deficit).is_ok() {
                    TokenPoolDeficit::<T>::remove(entity_id);
                    Self::deposit_event(Event::TokenPoolDeficitCorrected {
                        entity_id,
                        amount: deficit,
                    });
                }
                consumed = consumed.saturating_add(base_weight);
                processed += 1;
            }
            consumed
        }

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
            assert!(
                T::ConfigChangeDelay::get() > BlockNumberFor::<T>::zero(),
                "ConfigChangeDelay must be > 0"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    #[pallet::storage]
    #[pallet::getter(fn pool_reward_config)]
    pub type PoolRewardConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        PoolRewardConfigOf<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn current_round)]
    pub type CurrentRound<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        RoundInfoOf<T>,
    >;

    #[pallet::storage]
    pub type LastRoundId<T: Config> = StorageMap<_, Blake2_128Concat, u64, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn last_claimed_round)]
    pub type LastClaimedRound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        u64,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn claim_records)]
    pub type ClaimRecords<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BoundedVec<ClaimRecordOf<T>, T::MaxClaimHistory>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pool_reward_paused)]
    pub type PoolRewardPaused<T: Config> = StorageMap<_, Blake2_128Concat, u64, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn global_pool_reward_paused)]
    pub type GlobalPoolRewardPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn round_history)]
    pub type RoundHistory<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        BoundedVec<CompletedRoundSummaryOf<T>, T::MaxRoundHistory>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn distribution_stats)]
    pub type DistributionStatistics<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        DistributionStatsOf<T>,
        ValueQuery,
    >;

    /// 待生效的配置变更
    #[pallet::storage]
    #[pallet::getter(fn pending_pool_reward_config)]
    pub type PendingPoolRewardConfig<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        PendingConfigChangeOf<T>,
    >;

    /// Token 池账本差额：回滚失败时累计的已转出但未扣减的 token 数量
    #[pallet::storage]
    #[pallet::getter(fn token_pool_deficit)]
    pub type TokenPoolDeficit<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TokenBalanceOf<T>,
        ValueQuery,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PoolRewardConfigUpdated { entity_id: u64 },
        /// R1: 合并原 NewRoundStarted + NewRoundDetails 为单一事件
        NewRoundStarted {
            entity_id: u64,
            round_id: u64,
            pool_snapshot: BalanceOf<T>,
            token_pool_snapshot: Option<TokenBalanceOf<T>>,
            level_snapshots: alloc::vec::Vec<(u8, u32, BalanceOf<T>)>,
            token_level_snapshots: Option<alloc::vec::Vec<(u8, u32, TokenBalanceOf<T>)>>,
        },
        PoolRewardClaimed {
            entity_id: u64,
            account: T::AccountId,
            amount: BalanceOf<T>,
            token_amount: TokenBalanceOf<T>,
            round_id: u64,
            level_id: u8,
        },
        TokenPoolEnabledUpdated { entity_id: u64, enabled: bool },
        RoundForced { entity_id: u64, round_id: u64 },
        TokenTransferRollbackFailed {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
        /// P2-12: Token 初始转账失败（区分于回滚失败）
        TokenClaimTransferFailed {
            entity_id: u64,
            account: T::AccountId,
            amount: TokenBalanceOf<T>,
        },
        PoolRewardConfigCleared { entity_id: u64 },
        /// R3: 去除 Event 后缀
        PoolRewardPaused { entity_id: u64 },
        PoolRewardResumed { entity_id: u64 },
        GlobalPoolRewardPaused,
        GlobalPoolRewardResumed,
        RoundArchived { entity_id: u64, round_id: u64 },
        /// P0: 配置变更已计划
        PoolRewardConfigScheduled { entity_id: u64, apply_after: BlockNumberFor<T> },
        PendingPoolRewardConfigApplied { entity_id: u64 },
        PendingPoolRewardConfigCancelled { entity_id: u64 },
        /// Token 池差额已被 Root 修正
        TokenPoolDeficitCorrected { entity_id: u64, amount: TokenBalanceOf<T> },
        /// force_clear 用户记录未完全清理，需再次调用
        ClearIncomplete { entity_id: u64, remaining: u32 },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidRatio,
        RatioSumMismatch,
        DuplicateLevelId,
        InvalidRoundDuration,
        NotMember,
        LevelNotConfigured,
        AlreadyClaimed,
        LevelQuotaExhausted,
        NothingToClaim,
        InsufficientPool,
        ConfigNotFound,
        EntityNotActive,
        LevelNotInSnapshot,
        RoundIdOverflow,
        ParticipationRequirementNotMet,
        NotAuthorized,
        EntityLocked,
        PoolRewardIsPaused,
        PoolRewardNotPaused,
        RoundDurationTooShort,
        GlobalPaused,
        GlobalNotPaused,
        /// 已存在待生效的配置变更
        PendingConfigExists,
        /// 无待生效的配置变更
        NoPendingConfig,
        /// 配置变更延迟未到期
        ConfigChangeDelayNotMet,
        /// P2-10: 当前轮次尚未过期，不可创建新轮
        RoundNotExpired,
        /// Token 池无差额可修正
        NoDeficit,
    }

    // ========================================================================
    // Extrinsics (17 个)
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
            Self::do_set_pool_reward_config(entity_id, level_ratios, round_duration)
        }

        /// 用户领取沉淀池奖励（NEX + Token 双池统一入口）
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::claim_pool_reward())]
        pub fn claim_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!GlobalPoolRewardPaused::<T>::get(), Error::<T>::GlobalPaused);
            ensure!(!PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardIsPaused);

            ensure!(T::MemberProvider::is_member(entity_id, &who), Error::<T>::NotMember);
            ensure!(!T::MemberProvider::is_banned(entity_id, &who), Error::<T>::NotMember);
            ensure!(T::MemberProvider::is_member_active(entity_id, &who), Error::<T>::NotMember);
            ensure!(
                T::ParticipationGuard::can_participate(entity_id, &who),
                Error::<T>::ParticipationRequirementNotMet
            );

            let config = PoolRewardConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            let user_level = T::MemberProvider::custom_level_id(entity_id, &who);
            let effective_level = Self::resolve_effective_level(&config, user_level)
                .ok_or(Error::<T>::LevelNotConfigured)?;
            // P1-1 修复: 精确匹配检查配额，回退用户仅靠池余额保护
            let is_fallback = user_level != effective_level;

            let now = <frame_system::Pallet<T>>::block_number();
            let mut round = Self::ensure_current_round(entity_id, &config, now)?;

            let last_round = LastClaimedRound::<T>::get(entity_id, &who);
            ensure!(last_round < round.round_id, Error::<T>::AlreadyClaimed);

            let nex_snap_idx = round.level_snapshots.iter()
                .position(|s| s.level_id == effective_level)
                .ok_or(Error::<T>::LevelNotInSnapshot)?;

            let reward = {
                let snapshot = &round.level_snapshots[nex_snap_idx];
                if !is_fallback {
                    ensure!(snapshot.claimed_count < snapshot.member_count, Error::<T>::LevelQuotaExhausted);
                }
                let r = snapshot.per_member_reward;
                ensure!(!r.is_zero(), Error::<T>::NothingToClaim);
                r
            };

            let pool = T::PoolBalanceProvider::pool_balance(entity_id);
            ensure!(pool >= reward, Error::<T>::InsufficientPool);
            T::PoolBalanceProvider::deduct_pool(entity_id, reward)?;
            let entity_account = T::EntityProvider::entity_account(entity_id);
            T::Currency::transfer(&entity_account, &who, reward, ExistenceRequirement::KeepAlive)?;

            // Token 采用 transfer-first 顺序：best-effort 路径不走 `?`，
            // 不会触发 Substrate 事务回滚，且无 add_back 接口回滚记账扣减
            let mut token_reward = TokenBalanceOf::<T>::default();
            if let Some(ref mut token_snapshots) = round.token_level_snapshots {
                if let Some(token_snap) = token_snapshots.iter_mut().find(|s| s.level_id == effective_level) {
                    // P1-1: 回退用户跳过 Token 配额检查
                    let token_quota_ok = is_fallback || token_snap.claimed_count < token_snap.member_count;
                    if token_quota_ok && !token_snap.per_member_reward.is_zero() {
                        let tr = token_snap.per_member_reward;
                        let token_pool = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                        if token_pool >= tr {
                            match T::TokenTransferProvider::token_transfer(
                                entity_id, &entity_account, &who, tr,
                            ) {
                            Ok(()) => {
                                if T::TokenPoolBalanceProvider::deduct_token_pool(entity_id, tr).is_ok() {
                                    token_snap.claimed_count += 1;
                                    token_reward = tr;
                                } else {
                                    // P1-2 修复: 扣减失败 → 尝试回滚转账
                                    if T::TokenTransferProvider::token_transfer(
                                        entity_id, &who, &entity_account, tr,
                                    ).is_err() {
                                        // 回滚也失败 → Token 已转出但账本未扣减
                                        // 必须记录分配以保持 claimed_count 一致
                                        token_snap.claimed_count += 1;
                                        token_reward = tr;
                                        TokenPoolDeficit::<T>::mutate(entity_id, |d| {
                                            *d = d.saturating_add(tr);
                                        });
                                        Self::deposit_event(Event::TokenTransferRollbackFailed {
                                            entity_id,
                                            account: who.clone(),
                                            amount: tr,
                                        });
                                    }
                                }
                            },
                            Err(_) => {
                                // P2-12: Token 转账失败事件（区分于回滚失败）
                                Self::deposit_event(Event::TokenClaimTransferFailed {
                                    entity_id,
                                    account: who.clone(),
                                    amount: tr,
                                });
                            },
                            }
                        }
                    }
                }
            }

            let round_id = round.round_id;
            round.level_snapshots[nex_snap_idx].claimed_count += 1;
            CurrentRound::<T>::insert(entity_id, round);
            LastClaimedRound::<T>::insert(entity_id, &who, round_id);

            ClaimRecords::<T>::mutate(entity_id, &who, |history| {
                let record = ClaimRecord {
                    round_id,
                    amount: reward,
                    level_id: effective_level,
                    claimed_at: now,
                    token_amount: token_reward,
                };
                if history.is_full() {
                    history.remove(0);
                }
                let _ = history.try_push(record);
            });

            DistributionStatistics::<T>::mutate(entity_id, |stats| {
                stats.total_nex_distributed = stats.total_nex_distributed.saturating_add(reward);
                stats.total_token_distributed = stats.total_token_distributed.saturating_add(token_reward);
                stats.total_claims = stats.total_claims.saturating_add(1);
            });

            T::ClaimCallback::on_pool_reward_claimed(
                entity_id, &who, reward, token_reward, round_id, effective_level,
            );

            Self::deposit_event(Event::PoolRewardClaimed {
                entity_id,
                account: who,
                amount: reward,
                token_amount: token_reward,
                round_id,
                level_id: effective_level,
            });

            Ok(())
        }

        /// R4: 开启新轮次（Entity Owner / Admin）— 原 force_new_round，去除 force_ 前缀
        /// P1-4: 检查暂停状态
        /// P2-10: 最小轮龄保护
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::start_new_round())]
        pub fn start_new_round(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(!GlobalPoolRewardPaused::<T>::get(), Error::<T>::GlobalPaused);
            ensure!(!PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardIsPaused);

            let config = PoolRewardConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            // P2-10 修复: 当前轮次未过期时拒绝（防止频繁空转 + round_id 浪费）
            let now = <frame_system::Pallet<T>>::block_number();
            if let Some(ref current) = CurrentRound::<T>::get(entity_id) {
                let end_block = current.start_block.saturating_add(config.round_duration);
                ensure!(now >= end_block, Error::<T>::RoundNotExpired);
            }

            let round = Self::create_new_round(entity_id, &config, now)?;

            Self::deposit_event(Event::RoundForced {
                entity_id,
                round_id: round.round_id,
            });

            Ok(())
        }

        /// 启用/禁用 Entity Token 池分配（Entity Owner / Admin）
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
            Self::do_set_token_pool_enabled(entity_id, enabled)
        }

        /// [Root] 强制设置沉淀池奖励配置
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_pool_reward_config())]
        pub fn force_set_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_set_pool_reward_config(entity_id, level_ratios, round_duration)
        }

        /// [Root] 强制启用/禁用 Token 池分配
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::set_token_pool_enabled())]
        pub fn force_set_token_pool_enabled(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_set_token_pool_enabled(entity_id, enabled)
        }

        /// [Root] 强制开启新轮次（不检查暂停状态）
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::start_new_round())]
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

        /// 清除沉淀池奖励配置（Owner/Admin，部分清理）
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
            Self::do_clear_pool_reward_config(entity_id);
            Ok(())
        }

        /// [Root] 强制清除 — 完整清理全部存储（含用户记录）
        /// `max_users`: 每次最多清理的用户记录数（控制单次权重）。
        /// 如果用户记录未完全清理，发出 `ClearIncomplete` 事件，需再次调用。
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::force_clear_pool_reward_config())]
        pub fn force_clear_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            max_users: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            Self::do_full_clear_pool_reward(entity_id, max_users);
            Ok(())
        }

        /// 暂停沉淀池奖励分配（Entity Owner / Admin）
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
            Self::deposit_event(Event::PoolRewardPaused { entity_id });
            Ok(())
        }

        /// 恢复沉淀池奖励分配（Entity Owner / Admin）
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
            Self::deposit_event(Event::PoolRewardResumed { entity_id });
            Ok(())
        }

        /// 全局紧急暂停/恢复（Root only）
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
                Self::deposit_event(Event::GlobalPoolRewardPaused);
            } else {
                ensure!(GlobalPoolRewardPaused::<T>::get(), Error::<T>::GlobalNotPaused);
                GlobalPoolRewardPaused::<T>::kill();
                Self::deposit_event(Event::GlobalPoolRewardResumed);
            }
            Ok(())
        }

        /// P0-2: [Root] 强制暂停 per-entity 池奖励（绕过 EntityLocked）
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::force_pause_pool_reward())]
        pub fn force_pause_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(!PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardIsPaused);

            PoolRewardPaused::<T>::insert(entity_id, true);
            Self::deposit_event(Event::PoolRewardPaused { entity_id });
            Ok(())
        }

        /// P0-2: [Root] 强制恢复 per-entity 池奖励（绕过 EntityLocked）
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::force_resume_pool_reward())]
        pub fn force_resume_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(PoolRewardPaused::<T>::get(entity_id), Error::<T>::PoolRewardNotPaused);

            PoolRewardPaused::<T>::remove(entity_id);
            Self::deposit_event(Event::PoolRewardResumed { entity_id });
            Ok(())
        }

        /// P0-1: 计划配置变更（Owner/Admin）— 延迟 ConfigChangeDelay 区块后生效
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::schedule_pool_reward_config_change())]
        pub fn schedule_pool_reward_config_change(
            origin: OriginFor<T>,
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(PoolRewardConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(!PendingPoolRewardConfig::<T>::contains_key(entity_id), Error::<T>::PendingConfigExists);

            ensure!(round_duration > BlockNumberFor::<T>::zero(), Error::<T>::InvalidRoundDuration);
            ensure!(round_duration >= T::MinRoundDuration::get(), Error::<T>::RoundDurationTooShort);
            Self::validate_level_ratios(&level_ratios)?;

            let now = <frame_system::Pallet<T>>::block_number();
            let apply_after = now.saturating_add(T::ConfigChangeDelay::get());

            PendingPoolRewardConfig::<T>::insert(entity_id, PendingConfigChange {
                level_ratios,
                round_duration,
                apply_after,
            });

            Self::deposit_event(Event::PoolRewardConfigScheduled { entity_id, apply_after });
            Ok(())
        }

        /// P0-1: 应用待生效的配置变更
        /// P2-7 修复: 限制为 Owner/Admin（防止任意用户提前失效活跃轮次）
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::apply_pending_pool_reward_config())]
        pub fn apply_pending_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let pending = PendingPoolRewardConfig::<T>::get(entity_id)
                .ok_or(Error::<T>::NoPendingConfig)?;

            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(now >= pending.apply_after, Error::<T>::ConfigChangeDelayNotMet);

            Self::do_set_pool_reward_config(entity_id, pending.level_ratios, pending.round_duration)?;
            Self::deposit_event(Event::PendingPoolRewardConfigApplied { entity_id });
            Ok(())
        }

        /// P0-1: 取消待生效的配置变更（Owner/Admin）
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::cancel_pending_pool_reward_config())]
        pub fn cancel_pending_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(&who, entity_id)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(PendingPoolRewardConfig::<T>::contains_key(entity_id), Error::<T>::NoPendingConfig);

            PendingPoolRewardConfig::<T>::remove(entity_id);
            Self::deposit_event(Event::PendingPoolRewardConfigCancelled { entity_id });
            Ok(())
        }

        /// [Root] 修正 Token 池账本差额（回滚失败导致的已转出未扣减部分）
        /// 同时从 Token 池余额中扣减对应金额，使链上余额与实际一致。
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::correct_token_pool_deficit())]
        pub fn correct_token_pool_deficit(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let deficit = TokenPoolDeficit::<T>::take(entity_id);
            ensure!(!deficit.is_zero(), Error::<T>::NoDeficit);
            if let Err(e) = T::TokenPoolBalanceProvider::deduct_token_pool(entity_id, deficit) {
                frame_support::defensive!("pool-reward: deduct_token_pool failed during deficit correction");
                TokenPoolDeficit::<T>::insert(entity_id, deficit);
                return Err(e);
            }
            Self::deposit_event(Event::TokenPoolDeficitCorrected { entity_id, amount: deficit });
            Ok(())
        }
    }

    // ========================================================================
    // Internal logic
    // ========================================================================

    impl<T: Config> Pallet<T> {
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

        pub(crate) fn invalidate_current_round(entity_id: u64) {
            if let Some(round) = CurrentRound::<T>::get(entity_id) {
                LastRoundId::<T>::insert(entity_id, round.round_id);
            }
            CurrentRound::<T>::remove(entity_id);
        }

        pub(crate) fn validate_level_ratios(ratios: &[(u8, u16)]) -> DispatchResult {
            ensure!(!ratios.is_empty(), Error::<T>::InvalidRatio);
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

        /// P1-5: 解析有效等级 — 当前等级在配置中则返回，否则回退到最近的低等级
        fn resolve_effective_level(config: &PoolRewardConfigOf<T>, user_level: u8) -> Option<u8> {
            if config.level_ratios.iter().any(|(id, _)| *id == user_level) {
                return Some(user_level);
            }
            config.level_ratios.iter()
                .filter(|(id, _)| *id <= user_level)
                .max_by_key(|(id, _)| *id)
                .map(|(id, _)| *id)
        }

        // ================================================================
        // R2: do_* 共享内部逻辑，消除 normal/force 代码重复
        // ================================================================

        /// 设置配置的共享逻辑（校验 + 写入 + 失效轮次 + 清除待生效变更）
        pub(crate) fn do_set_pool_reward_config(
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotActive);
            ensure!(round_duration > BlockNumberFor::<T>::zero(), Error::<T>::InvalidRoundDuration);
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
            PendingPoolRewardConfig::<T>::remove(entity_id);
            Self::deposit_event(Event::PoolRewardConfigUpdated { entity_id });
            Ok(())
        }

        /// Token 开关共享逻辑
        pub(crate) fn do_set_token_pool_enabled(entity_id: u64, enabled: bool) -> DispatchResult {
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
                Self::deposit_event(Event::TokenPoolEnabledUpdated { entity_id, enabled });
            }
            Ok(())
        }

        /// 部分清理（Owner 级别：不清理用户级记录）
        /// P2-6 修复: 同时清理 RoundHistory 和 DistributionStatistics（避免存储泄漏）
        pub(crate) fn do_clear_pool_reward_config(entity_id: u64) {
            PoolRewardConfigs::<T>::remove(entity_id);
            PoolRewardPaused::<T>::remove(entity_id);
            PendingPoolRewardConfig::<T>::remove(entity_id);
            Self::invalidate_current_round(entity_id);
            RoundHistory::<T>::remove(entity_id);
            DistributionStatistics::<T>::remove(entity_id);
            Self::deposit_event(Event::PoolRewardConfigCleared { entity_id });
        }

        /// 完整清理（Root / PlanWriter 级别：含全部用户记录）
        /// `max_users`: 每次 clear_prefix 的上限。如果未完全清理，发出 ClearIncomplete。
        pub(crate) fn do_full_clear_pool_reward(entity_id: u64, max_users: u32) {
            PoolRewardConfigs::<T>::remove(entity_id);
            CurrentRound::<T>::remove(entity_id);
            LastRoundId::<T>::remove(entity_id);

            let limit = max_users.max(1);
            let r1 = LastClaimedRound::<T>::clear_prefix(entity_id, limit, None);
            let r2 = ClaimRecords::<T>::clear_prefix(entity_id, limit, None);

            let has_remaining = r1.maybe_cursor.is_some() || r2.maybe_cursor.is_some();

            PoolRewardPaused::<T>::remove(entity_id);
            PendingPoolRewardConfig::<T>::remove(entity_id);
            RoundHistory::<T>::remove(entity_id);
            DistributionStatistics::<T>::remove(entity_id);
            TokenPoolDeficit::<T>::remove(entity_id);

            if has_remaining {
                let remaining = r1.loops.saturating_add(r2.loops);
                Self::deposit_event(Event::ClearIncomplete { entity_id, remaining });
            }
            Self::deposit_event(Event::PoolRewardConfigCleared { entity_id });
        }

        fn build_level_snapshots<B>(
            pool_balance: B,
            level_counts: &[(u8, u16, u32)],
        ) -> BoundedVec<LevelSnapshot<B>, T::MaxPoolRewardLevels>
        where
            B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
        {
            let mut snapshots = BoundedVec::default();
            for &(level_id, ratio, count) in level_counts.iter() {
                let per_member = Self::safe_per_member_reward(pool_balance, ratio, count);
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

            frame_support::ensure!(old_round_id < u64::MAX, Error::<T>::RoundIdOverflow);

            if let Some(ref old) = old_round {
                let computed_end = old.start_block.saturating_add(config.round_duration);
                let summary = CompletedRoundSummary {
                    round_id: old.round_id,
                    start_block: old.start_block,
                    end_block: computed_end,
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
                DistributionStatistics::<T>::mutate(entity_id, |stats| {
                    stats.total_rounds_completed = stats.total_rounds_completed.saturating_add(1);
                });
            }

            let level_counts: alloc::vec::Vec<(u8, u16, u32)> = config.level_ratios.iter()
                .map(|(level_id, ratio)| {
                    let count = T::MemberProvider::member_count_by_level(entity_id, *level_id);
                    (*level_id, *ratio, count)
                })
                .collect();

            let level_snapshots = Self::build_level_snapshots(pool_balance, &level_counts);

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

            // R1: 单一合并事件（原 NewRoundStarted + NewRoundDetails）
            let level_details: alloc::vec::Vec<(u8, u32, BalanceOf<T>)> = new_round.level_snapshots.iter()
                .map(|s| (s.level_id, s.member_count, s.per_member_reward))
                .collect();
            let token_level_details = new_round.token_level_snapshots.as_ref().map(|snaps| {
                snaps.iter().map(|s| (s.level_id, s.member_count, s.per_member_reward)).collect()
            });
            Self::deposit_event(Event::NewRoundStarted {
                entity_id,
                round_id: new_round.round_id,
                pool_snapshot: pool_balance,
                token_pool_snapshot,
                level_snapshots: level_details,
                token_level_snapshots: token_level_details,
            });

            Ok(new_round)
        }

        /// 可领取金额预查询（只读）
        pub fn get_claimable(
            entity_id: u64,
            who: &T::AccountId,
        ) -> (BalanceOf<T>, TokenBalanceOf<T>) {
            let zero_nex = BalanceOf::<T>::zero();
            let zero_token = TokenBalanceOf::<T>::default();

            if GlobalPoolRewardPaused::<T>::get() || PoolRewardPaused::<T>::get(entity_id) {
                return (zero_nex, zero_token);
            }
            if !T::EntityProvider::is_entity_active(entity_id) {
                return (zero_nex, zero_token);
            }
            if !T::MemberProvider::is_member(entity_id, who) {
                return (zero_nex, zero_token);
            }
            if T::MemberProvider::is_banned(entity_id, who)
                || !T::MemberProvider::is_member_active(entity_id, who)
            {
                return (zero_nex, zero_token);
            }
            if !T::ParticipationGuard::can_participate(entity_id, who) {
                return (zero_nex, zero_token);
            }

            let config = match PoolRewardConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return (zero_nex, zero_token),
            };

            let user_level = T::MemberProvider::custom_level_id(entity_id, who);
            let effective_level = match Self::resolve_effective_level(&config, user_level) {
                Some(l) => l,
                None => return (zero_nex, zero_token),
            };
            let is_fallback = user_level != effective_level;

            let now = <frame_system::Pallet<T>>::block_number();
            let round = if let Some(r) = CurrentRound::<T>::get(entity_id) {
                let end_block = r.start_block.saturating_add(config.round_duration);
                if now < end_block {
                    r
                } else {
                    return Self::simulate_claimable(entity_id, &config, effective_level);
                }
            } else {
                return Self::simulate_claimable(entity_id, &config, effective_level);
            };

            let last_round = LastClaimedRound::<T>::get(entity_id, who);
            if last_round >= round.round_id {
                return (zero_nex, zero_token);
            }

            let nex_reward = round.level_snapshots.iter()
                .find(|s| s.level_id == effective_level)
                .and_then(|s| {
                    let quota_ok = is_fallback || s.claimed_count < s.member_count;
                    if quota_ok && !s.per_member_reward.is_zero() {
                        let pool = T::PoolBalanceProvider::pool_balance(entity_id);
                        if pool >= s.per_member_reward { Some(s.per_member_reward) } else { None }
                    } else {
                        None
                    }
                })
                .unwrap_or(zero_nex);

            let token_reward = round.token_level_snapshots.as_ref()
                .and_then(|snaps| snaps.iter().find(|s| s.level_id == effective_level))
                .and_then(|s| {
                    let quota_ok = is_fallback || s.claimed_count < s.member_count;
                    if quota_ok && !s.per_member_reward.is_zero() {
                        let token_pool = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                        if token_pool >= s.per_member_reward { Some(s.per_member_reward) } else { None }
                    } else {
                        None
                    }
                })
                .unwrap_or(zero_token);

            (nex_reward, token_reward)
        }

        /// Safe per-member reward calculation with overflow protection.
        /// Shared by `build_level_snapshots` and `simulate_claimable`.
        fn safe_per_member_reward<B>(pool_balance: B, ratio: u16, count: u32) -> B
        where
            B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
        {
            if count == 0 || pool_balance.is_zero() {
                return B::zero();
            }
            let ratio_b: B = (ratio as u32).into();
            let count_b: B = count.into();
            let divisor: B = B::from(10000u32).saturating_mul(count_b);
            let product = match pool_balance.checked_mul(&ratio_b) {
                Some(p) => p,
                None => {
                    frame_support::defensive!("pool-reward: pool_balance * ratio overflow in per_member_reward");
                    pool_balance.saturating_mul(ratio_b)
                }
            };
            product / divisor
        }

        fn simulate_claimable(
            entity_id: u64,
            config: &PoolRewardConfigOf<T>,
            effective_level: u8,
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
                .find(|(id, _, _)| *id == effective_level)
                .map(|&(_, ratio, count)| Self::safe_per_member_reward(pool_balance, ratio, count))
                .unwrap_or(zero_nex);

            let token_reward = if config.token_pool_enabled {
                let token_balance = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                if token_balance.is_zero() {
                    zero_token
                } else {
                    level_counts.iter()
                        .find(|(id, _, _)| *id == effective_level)
                        .map(|&(_, ratio, count)| Self::safe_per_member_reward(token_balance, ratio, count))
                        .unwrap_or(zero_token)
                }
            } else {
                zero_token
            };

            (nex_reward, token_reward)
        }

        /// 轮次领取进度查询
        pub fn get_round_statistics(entity_id: u64) -> Option<alloc::vec::Vec<(u8, u32, u32, BalanceOf<T>)>> {
            CurrentRound::<T>::get(entity_id).map(|round| {
                round.level_snapshots.iter()
                    .map(|s| (s.level_id, s.member_count, s.claimed_count, s.per_member_reward))
                    .collect()
            })
        }

        // ================================================================
        // Runtime API helpers
        // ================================================================

        fn block_to_u64(b: BlockNumberFor<T>) -> u64 {
            sp_runtime::traits::UniqueSaturatedInto::<u64>::unique_saturated_into(b)
        }

        fn build_level_progress<B: Copy>(
            snapshots: &[LevelSnapshot<B>],
            config: &PoolRewardConfigOf<T>,
        ) -> alloc::vec::Vec<crate::runtime_api::LevelProgressInfo<B>> {
            snapshots.iter().map(|s| {
                let ratio_bps = config.level_ratios.iter()
                    .find(|(id, _)| *id == s.level_id)
                    .map(|(_, r)| *r)
                    .unwrap_or(0);
                crate::runtime_api::LevelProgressInfo {
                    level_id: s.level_id,
                    ratio_bps,
                    member_count: s.member_count,
                    claimed_count: s.claimed_count,
                    per_member_reward: s.per_member_reward,
                }
            }).collect()
        }

        fn build_round_detail(
            round: &RoundInfoOf<T>,
            config: &PoolRewardConfigOf<T>,
        ) -> crate::runtime_api::RoundDetailInfo<BalanceOf<T>, TokenBalanceOf<T>> {
            let end_block = round.start_block.saturating_add(config.round_duration);
            crate::runtime_api::RoundDetailInfo {
                round_id: round.round_id,
                start_block: Self::block_to_u64(round.start_block),
                end_block: Self::block_to_u64(end_block),
                pool_snapshot: round.pool_snapshot,
                token_pool_snapshot: round.token_pool_snapshot,
                level_snapshots: Self::build_level_progress(&round.level_snapshots, config),
                token_level_snapshots: round.token_level_snapshots.as_ref().map(|ts| {
                    Self::build_level_progress(ts, config)
                }),
            }
        }

        /// Runtime API: 会员沉淀池详情
        pub fn get_pool_reward_member_view(
            entity_id: u64,
            who: &T::AccountId,
        ) -> Option<crate::runtime_api::PoolRewardMemberView<BalanceOf<T>, TokenBalanceOf<T>>> {
            let config = PoolRewardConfigs::<T>::get(entity_id)?;

            if !T::MemberProvider::is_member(entity_id, who) {
                return None;
            }

            let user_level = T::MemberProvider::custom_level_id(entity_id, who);
            let effective_level = Self::resolve_effective_level(&config, user_level)
                .unwrap_or(user_level);

            let (claimable_nex, claimable_token) = Self::get_claimable(entity_id, who);

            let is_paused = GlobalPoolRewardPaused::<T>::get()
                || PoolRewardPaused::<T>::get(entity_id);

            let last_claimed = LastClaimedRound::<T>::get(entity_id, who);

            let now = <frame_system::Pallet<T>>::block_number();
            let round = CurrentRound::<T>::get(entity_id);
            let current_round_id = round.as_ref()
                .map(|r| r.round_id)
                .unwrap_or_else(|| LastRoundId::<T>::get(entity_id));

            let round_expired = round.as_ref()
                .map(|r| now >= r.start_block.saturating_add(config.round_duration))
                .unwrap_or(true);

            let already_claimed = if round_expired {
                false
            } else {
                last_claimed >= current_round_id && current_round_id > 0
            };

            let (round_start_block, round_end_block, pool_snapshot, token_pool_snapshot,
                 level_progress, token_level_progress) = match round {
                Some(ref r) => {
                    let end = r.start_block.saturating_add(config.round_duration);
                    (
                        Self::block_to_u64(r.start_block),
                        Self::block_to_u64(end),
                        r.pool_snapshot,
                        r.token_pool_snapshot,
                        Self::build_level_progress(&r.level_snapshots, &config),
                        r.token_level_snapshots.as_ref().map(|ts| {
                            Self::build_level_progress(ts, &config)
                        }),
                    )
                },
                None => (
                    0u64,
                    0u64,
                    BalanceOf::<T>::zero(),
                    None,
                    alloc::vec::Vec::new(),
                    None,
                ),
            };

            let claim_history: alloc::vec::Vec<crate::runtime_api::ClaimRecordInfo<BalanceOf<T>, TokenBalanceOf<T>>> =
                ClaimRecords::<T>::get(entity_id, who).iter().map(|r| {
                    crate::runtime_api::ClaimRecordInfo {
                        round_id: r.round_id,
                        amount: r.amount,
                        token_amount: r.token_amount,
                        level_id: r.level_id,
                        claimed_at: Self::block_to_u64(r.claimed_at),
                    }
                }).collect();

            Some(crate::runtime_api::PoolRewardMemberView {
                round_duration: Self::block_to_u64(config.round_duration),
                token_pool_enabled: config.token_pool_enabled,
                level_ratios: config.level_ratios.into_inner(),
                current_round_id,
                round_start_block,
                round_end_block,
                pool_snapshot,
                token_pool_snapshot,
                effective_level,
                claimable_nex,
                claimable_token,
                already_claimed,
                round_expired,
                last_claimed_round: last_claimed,
                level_progress,
                token_level_progress,
                claim_history,
                is_paused,
                has_pending_config: PendingPoolRewardConfig::<T>::contains_key(entity_id),
            })
        }

        /// Runtime API: 管理者沉淀池总览
        pub fn get_pool_reward_admin_view(
            entity_id: u64,
        ) -> Option<crate::runtime_api::PoolRewardAdminView<BalanceOf<T>, TokenBalanceOf<T>>> {
            let config = PoolRewardConfigs::<T>::get(entity_id)?;

            let current_round = CurrentRound::<T>::get(entity_id)
                .map(|r| Self::build_round_detail(&r, &config));

            let stats = DistributionStatistics::<T>::get(entity_id);

            let round_history: alloc::vec::Vec<crate::runtime_api::CompletedRoundInfo<BalanceOf<T>, TokenBalanceOf<T>>> =
                RoundHistory::<T>::get(entity_id).iter().map(|r| {
                    crate::runtime_api::CompletedRoundInfo {
                        round_id: r.round_id,
                        start_block: Self::block_to_u64(r.start_block),
                        end_block: Self::block_to_u64(r.end_block),
                        pool_snapshot: r.pool_snapshot,
                        token_pool_snapshot: r.token_pool_snapshot,
                        level_snapshots: Self::build_level_progress(&r.level_snapshots, &config),
                        token_level_snapshots: r.token_level_snapshots.as_ref().map(|ts| {
                            Self::build_level_progress(ts, &config)
                        }),
                    }
                }).collect();

            let pending_config = PendingPoolRewardConfig::<T>::get(entity_id)
                .map(|p| crate::runtime_api::PendingConfigInfo {
                    level_ratios: p.level_ratios.into_inner(),
                    round_duration: Self::block_to_u64(p.round_duration),
                    apply_after: Self::block_to_u64(p.apply_after),
                });

            let current_pool_balance = T::PoolBalanceProvider::pool_balance(entity_id);
            let current_token_pool_balance = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);

            Some(crate::runtime_api::PoolRewardAdminView {
                level_ratios: config.level_ratios.into_inner(),
                round_duration: Self::block_to_u64(config.round_duration),
                token_pool_enabled: config.token_pool_enabled,
                current_round,
                total_nex_distributed: stats.total_nex_distributed,
                total_token_distributed: stats.total_token_distributed,
                total_rounds_completed: stats.total_rounds_completed,
                total_claims: stats.total_claims,
                round_history,
                pending_config,
                is_paused: PoolRewardPaused::<T>::get(entity_id),
                is_global_paused: GlobalPoolRewardPaused::<T>::get(),
                current_pool_balance,
                current_token_pool_balance,
                token_pool_deficit: TokenPoolDeficit::<T>::get(entity_id),
            })
        }
    }
}

// ============================================================================
// PoolRewardPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::PoolRewardPlanWriter for pallet::Pallet<T> {
    fn set_pool_reward_config(
        entity_id: u64,
        level_ratios: alloc::vec::Vec<(u8, u16)>,
        round_duration: u32,
    ) -> Result<(), sp_runtime::DispatchError> {
        let bounded: frame_support::BoundedVec<(u8, u16), T::MaxPoolRewardLevels> = level_ratios
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;

        let rd: frame_system::pallet_prelude::BlockNumberFor<T> = round_duration.into();
        pallet::Pallet::<T>::do_set_pool_reward_config(entity_id, bounded, rd)
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::Pallet::<T>::do_full_clear_pool_reward(entity_id, u32::MAX);
        Ok(())
    }

    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> Result<(), sp_runtime::DispatchError> {
        pallet::Pallet::<T>::do_set_token_pool_enabled(entity_id, enabled)
    }
}

// ============================================================================
// PoolRewardQueryProvider 实现
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::PoolRewardQueryProvider<
    T::AccountId,
    BalanceOf<T>,
    TokenBalanceOf<T>,
> for pallet::Pallet<T> {
    fn claimable(entity_id: u64, account: &T::AccountId) -> (BalanceOf<T>, TokenBalanceOf<T>) {
        Self::get_claimable(entity_id, account)
    }

    fn is_paused(entity_id: u64) -> bool {
        pallet::GlobalPoolRewardPaused::<T>::get() || pallet::PoolRewardPaused::<T>::get(entity_id)
    }

    fn current_round_id(entity_id: u64) -> u64 {
        pallet::CurrentRound::<T>::get(entity_id)
            .map(|r| r.round_id)
            .unwrap_or_else(|| pallet::LastRoundId::<T>::get(entity_id))
    }
}

// ============================================================================
// P2-14: OnMemberRemoved 回调实现
// ============================================================================

impl<T: pallet::Config> pallet_entity_common::OnMemberRemoved<T::AccountId> for pallet::Pallet<T> {
    fn on_member_removed(entity_id: u64, account: &T::AccountId) {
        pallet::LastClaimedRound::<T>::remove(entity_id, account);
        pallet::ClaimRecords::<T>::remove(entity_id, account);
    }
}

#[cfg(test)]
mod tests;
