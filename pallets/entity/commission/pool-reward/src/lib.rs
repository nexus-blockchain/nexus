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
    use pallet_entity_common::EntityProvider;
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;

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
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

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
        /// 会员未激活
        MemberNotActivated,
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
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置沉淀池奖励配置（Root / Governance）
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_pool_reward_config())]
        pub fn set_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(round_duration > BlockNumberFor::<T>::zero(), Error::<T>::InvalidRoundDuration);
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

            // 1. 资格检查
            ensure!(T::MemberProvider::is_member(entity_id, &who), Error::<T>::NotMember);
            ensure!(T::MemberProvider::is_activated(entity_id, &who), Error::<T>::MemberNotActivated);

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

        /// 强制开启新轮次（Root）
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::force_new_round())]
        pub fn force_new_round(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;

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

        /// 启用/禁用 Entity Token 池分配（Root）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_token_pool_enabled())]
        pub fn set_token_pool_enabled(
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
            // M1-R3 审计修复: token 启用/禁用立即生效，使当前轮次失效
            // L1-R4 审计修复: 仅在值实际变更时才失效轮次，避免幂等调用浪费快照
            if changed {
                Self::invalidate_current_round(entity_id);
            }
            Self::deposit_event(Event::TokenPoolEnabledUpdated { entity_id, enabled });
            Ok(())
        }
    }

    // ========================================================================
    // Internal logic
    // ========================================================================

    impl<T: Config> Pallet<T> {
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
            let old_round_id = CurrentRound::<T>::get(entity_id)
                .map(|r| r.round_id)
                .unwrap_or_else(|| LastRoundId::<T>::get(entity_id));

            // M1 审计修复: 防止 round_id 在 u64::MAX 时 saturating_add(1) 不变导致重复 ID
            frame_support::ensure!(old_round_id < u64::MAX, Error::<T>::RoundIdOverflow);

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

            Ok(new_round)
        }
    }
}

// ============================================================================
// PoolRewardPlanWriter implementation (v2)
// ============================================================================

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
        // L1-R3 审计修复: emit 事件
        Self::deposit_event(pallet::Event::PoolRewardConfigUpdated { entity_id });
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
