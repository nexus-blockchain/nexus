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

pub use pallet::*;

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

        /// 默认轮次持续区块数（可被 per-entity 配置覆盖）
        #[pallet::constant]
        type DefaultRoundDuration: Get<BlockNumberFor<Self>>;

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
        /// 等级未在快照中
        LevelNotInSnapshot,
        /// Token 沉淀池余额不足
        InsufficientTokenPool,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置沉淀池奖励配置（Root / Governance）
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn set_pool_reward_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_ratios: BoundedVec<(u8, u16), T::MaxPoolRewardLevels>,
            round_duration: BlockNumberFor<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // 校验 round_duration > 0
            ensure!(round_duration > BlockNumberFor::<T>::zero(), Error::<T>::InvalidRoundDuration);

            // 校验无重复 level_id
            for i in 0..level_ratios.len() {
                for j in (i + 1)..level_ratios.len() {
                    ensure!(level_ratios[i].0 != level_ratios[j].0, Error::<T>::DuplicateLevelId);
                }
            }

            // 校验每个 ratio 在 (0, 10000]，且总和 = 10000
            let mut sum: u16 = 0;
            for (_, ratio) in level_ratios.iter() {
                ensure!(*ratio > 0 && *ratio <= 10000, Error::<T>::InvalidRatio);
                sum = sum.saturating_add(*ratio);
            }
            ensure!(sum == 10000, Error::<T>::RatioSumMismatch);

            PoolRewardConfigs::<T>::insert(entity_id, PoolRewardConfig {
                level_ratios,
                round_duration,
                token_pool_enabled: false,
            });

            Self::deposit_event(Event::PoolRewardConfigUpdated { entity_id });
            Ok(())
        }

        /// 用户领取沉淀池奖励（NEX + Token 双池统一入口）
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(150_000_000, 12_000))]
        pub fn claim_pool_reward(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // 1. 资格检查
            ensure!(T::MemberProvider::is_member(entity_id, &who), Error::<T>::NotMember);
            ensure!(T::MemberProvider::is_activated(entity_id, &who), Error::<T>::MemberNotActivated);

            let config = PoolRewardConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            let user_level = T::MemberProvider::custom_level_id(entity_id, &who);

            // 检查用户等级是否在配置中且比率 > 0
            let _user_ratio = config.level_ratios.iter()
                .find(|(id, _)| *id == user_level)
                .map(|(_, r)| *r)
                .ok_or(Error::<T>::LevelNotConfigured)?;

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

            // 6. NEX 池偿付检查 + 转账
            let pool = T::PoolBalanceProvider::pool_balance(entity_id);
            ensure!(pool >= reward, Error::<T>::InsufficientPool);
            let entity_account = T::EntityProvider::entity_account(entity_id);
            T::Currency::transfer(&entity_account, &who, reward, ExistenceRequirement::KeepAlive)?;

            // 7. Token 部分（best-effort：失败不影响 NEX 领取）
            // M3 审计修复: deduct_token_pool 失败时回滚转账，保持池余额一致性
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
                                    let _ = T::TokenTransferProvider::token_transfer(
                                        entity_id, &who, &entity_account, tr,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // 8. 状态更新
            T::PoolBalanceProvider::deduct_pool(entity_id, reward)?;
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
        #[pallet::weight(Weight::from_parts(100_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(30_000_000, 3_000))]
        pub fn set_token_pool_enabled(
            origin: OriginFor<T>,
            entity_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;
            PoolRewardConfigs::<T>::try_mutate(entity_id, |maybe| -> DispatchResult {
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                config.token_pool_enabled = enabled;
                Ok(())
            })?;
            Self::deposit_event(Event::TokenPoolEnabledUpdated { entity_id, enabled });
            Ok(())
        }
    }

    // ========================================================================
    // Internal logic
    // ========================================================================

    impl<T: Config> Pallet<T> {
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
                .unwrap_or(0);

            // NEX 快照
            let mut level_snapshots: BoundedVec<LevelSnapshot<BalanceOf<T>>, T::MaxPoolRewardLevels> =
                BoundedVec::default();

            for (level_id, ratio) in config.level_ratios.iter() {
                let count = T::MemberProvider::member_count_by_level(entity_id, *level_id);
                let per_member = if count > 0 && !pool_balance.is_zero() {
                    let ratio_balance: BalanceOf<T> = (*ratio as u32).into();
                    let count_balance: BalanceOf<T> = count.into();
                    pool_balance
                        .saturating_mul(ratio_balance)
                        / 10000u32.into()
                        / count_balance
                } else {
                    Zero::zero()
                };
                let _ = level_snapshots.try_push(LevelSnapshot {
                    level_id: *level_id,
                    member_count: count,
                    per_member_reward: per_member,
                    claimed_count: 0,
                });
            }

            // Token 快照（仅当 token_pool_enabled = true）
            let (token_pool_snapshot, token_level_snapshots) = if config.token_pool_enabled {
                let token_balance = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                let mut token_snaps: BoundedVec<LevelSnapshot<TokenBalanceOf<T>>, T::MaxPoolRewardLevels> =
                    BoundedVec::default();
                for (level_id, ratio) in config.level_ratios.iter() {
                    let count = T::MemberProvider::member_count_by_level(entity_id, *level_id);
                    let per_member = if count > 0 && !token_balance.is_zero() {
                        let ratio_balance: TokenBalanceOf<T> = (*ratio as u32).into();
                        let count_balance: TokenBalanceOf<T> = count.into();
                        token_balance.saturating_mul(ratio_balance) / 10000u32.into() / count_balance
                    } else {
                        Zero::zero()
                    };
                    let _ = token_snaps.try_push(LevelSnapshot {
                        level_id: *level_id,
                        member_count: count,
                        per_member_reward: per_member,
                        claimed_count: 0,
                    });
                }
                (Some(token_balance), Some(token_snaps))
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
        let bounded: frame_support::BoundedVec<(u8, u16), T::MaxPoolRewardLevels> = level_ratios
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;

        let rd: frame_system::pallet_prelude::BlockNumberFor<T> = round_duration.into();

        pallet::PoolRewardConfigs::<T>::insert(entity_id, pallet::PoolRewardConfig {
            level_ratios: bounded,
            round_duration: rd,
            token_pool_enabled: false,
        });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::PoolRewardConfigs::<T>::remove(entity_id);
        pallet::CurrentRound::<T>::remove(entity_id);
        Ok(())
    }

    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> Result<(), sp_runtime::DispatchError> {
        pallet::PoolRewardConfigs::<T>::try_mutate(entity_id, |maybe| -> Result<(), sp_runtime::DispatchError> {
            let config = maybe.as_mut().ok_or(sp_runtime::DispatchError::Other("ConfigNotFound"))?;
            config.token_pool_enabled = enabled;
            Ok(())
        })
    }
}

// ============================================================================
// Tests (v2)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{
        assert_ok, assert_noop,
        traits::ConstU32,
        derive_impl,
    };
    use sp_runtime::BuildStorage;

    type Balance = u128;

    // -- Mock thread-local state --
    use core::cell::RefCell;
    use alloc::collections::BTreeMap;

    thread_local! {
        static MEMBERS: RefCell<BTreeMap<(u64, u64), bool>> = RefCell::new(BTreeMap::new());
        static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> = RefCell::new(BTreeMap::new());
        static LEVEL_MEMBER_COUNTS: RefCell<BTreeMap<(u64, u8), u32>> = RefCell::new(BTreeMap::new());
        static POOL_BALANCES: RefCell<BTreeMap<u64, Balance>> = RefCell::new(BTreeMap::new());
        static TOKEN_POOL_BALANCES: RefCell<BTreeMap<u64, Balance>> = RefCell::new(BTreeMap::new());
        static TOKEN_BALANCES: RefCell<BTreeMap<(u64, u64), Balance>> = RefCell::new(BTreeMap::new());
    }

    fn clear_mocks() {
        MEMBERS.with(|m| m.borrow_mut().clear());
        CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().clear());
        LEVEL_MEMBER_COUNTS.with(|l| l.borrow_mut().clear());
        POOL_BALANCES.with(|p| p.borrow_mut().clear());
        TOKEN_POOL_BALANCES.with(|p| p.borrow_mut().clear());
        TOKEN_BALANCES.with(|m| m.borrow_mut().clear());
    }

    fn set_member(entity_id: u64, account: u64, level: u8) {
        MEMBERS.with(|m| m.borrow_mut().insert((entity_id, account), true));
        CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((entity_id, account), level));
    }

    fn set_level_count(entity_id: u64, level_id: u8, count: u32) {
        LEVEL_MEMBER_COUNTS.with(|l| l.borrow_mut().insert((entity_id, level_id), count));
    }

    fn set_pool_balance(entity_id: u64, balance: Balance) {
        POOL_BALANCES.with(|p| p.borrow_mut().insert(entity_id, balance));
    }

    fn get_pool_balance(entity_id: u64) -> Balance {
        POOL_BALANCES.with(|p| p.borrow().get(&entity_id).copied().unwrap_or(0))
    }

    fn set_token_pool_balance(entity_id: u64, balance: Balance) {
        TOKEN_POOL_BALANCES.with(|p| p.borrow_mut().insert(entity_id, balance));
    }

    fn set_token_balance(entity_id: u64, account: u64, balance: Balance) {
        TOKEN_BALANCES.with(|m| m.borrow_mut().insert((entity_id, account), balance));
    }

    #[allow(dead_code)]
    fn get_token_pool_balance(entity_id: u64) -> Balance {
        TOKEN_POOL_BALANCES.with(|p| p.borrow().get(&entity_id).copied().unwrap_or(0))
    }

    #[allow(dead_code)]
    fn get_token_balance(entity_id: u64, account: u64) -> Balance {
        TOKEN_BALANCES.with(|m| m.borrow().get(&(entity_id, account)).copied().unwrap_or(0))
    }

    // -- Mock TokenPoolBalanceProvider --
    pub struct MockTokenPoolBalanceProvider;

    impl pallet_commission_common::TokenPoolBalanceProvider<Balance> for MockTokenPoolBalanceProvider {
        fn token_pool_balance(entity_id: u64) -> Balance {
            TOKEN_POOL_BALANCES.with(|p| p.borrow().get(&entity_id).copied().unwrap_or(0))
        }
        fn deduct_token_pool(entity_id: u64, amount: Balance) -> Result<(), sp_runtime::DispatchError> {
            TOKEN_POOL_BALANCES.with(|p| {
                let mut map = p.borrow_mut();
                let bal = map.get(&entity_id).copied().unwrap_or(0);
                if bal < amount {
                    return Err(sp_runtime::DispatchError::Other("InsufficientTokenPool"));
                }
                map.insert(entity_id, bal - amount);
                Ok(())
            })
        }
    }

    // -- Mock TokenTransferProvider --
    pub struct MockTokenTransferProvider;

    impl pallet_commission_common::TokenTransferProvider<u64, Balance> for MockTokenTransferProvider {
        fn token_balance_of(entity_id: u64, who: &u64) -> Balance {
            TOKEN_BALANCES.with(|m| m.borrow().get(&(entity_id, *who)).copied().unwrap_or(0))
        }
        fn token_transfer(
            entity_id: u64, from: &u64, to: &u64, amount: Balance,
        ) -> Result<(), sp_runtime::DispatchError> {
            TOKEN_BALANCES.with(|m| {
                let mut map = m.borrow_mut();
                let from_bal = map.get(&(entity_id, *from)).copied().unwrap_or(0);
                if from_bal < amount {
                    return Err(sp_runtime::DispatchError::Other("InsufficientTokenBalance"));
                }
                map.insert((entity_id, *from), from_bal - amount);
                let to_bal = map.get(&(entity_id, *to)).copied().unwrap_or(0);
                map.insert((entity_id, *to), to_bal + amount);
                Ok(())
            })
        }
    }

    // -- Mock MemberProvider --
    pub struct MockMemberProvider;

    impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
        fn is_member(entity_id: u64, account: &u64) -> bool {
            MEMBERS.with(|m| m.borrow().contains_key(&(entity_id, *account)))
        }
        fn get_referrer(_: u64, _: &u64) -> Option<u64> { None }
        fn member_level(_: u64, _: &u64) -> Option<pallet_entity_common::MemberLevel> { None }
        fn get_member_stats(_: u64, _: &u64) -> (u32, u32, u128) { (0, 0, 0) }
        fn uses_custom_levels(_: u64) -> bool { true }
        fn custom_level_id(entity_id: u64, account: &u64) -> u8 {
            CUSTOM_LEVEL_IDS.with(|c| c.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
        }
        fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
        fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn custom_level_count(_: u64) -> u8 { 0 }
        fn member_count_by_level(entity_id: u64, level_id: u8) -> u32 {
            LEVEL_MEMBER_COUNTS.with(|l| l.borrow().get(&(entity_id, level_id)).copied().unwrap_or(0))
        }
    }

    // -- Mock EntityProvider --
    pub struct MockEntityProvider;

    impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
        fn entity_exists(_: u64) -> bool { true }
        fn is_entity_active(_: u64) -> bool { true }
        fn entity_status(_: u64) -> Option<pallet_entity_common::EntityStatus> { None }
        fn entity_owner(_: u64) -> Option<u64> { Some(999) }
        fn entity_account(entity_id: u64) -> u64 { entity_id + 9000 }
        fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_entity_rating(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    }

    // -- Mock PoolBalanceProvider --
    pub struct MockPoolBalanceProvider;

    impl pallet_commission_common::PoolBalanceProvider<Balance> for MockPoolBalanceProvider {
        fn pool_balance(entity_id: u64) -> Balance {
            get_pool_balance(entity_id)
        }
        fn deduct_pool(entity_id: u64, amount: Balance) -> Result<(), sp_runtime::DispatchError> {
            POOL_BALANCES.with(|p| {
                let mut map = p.borrow_mut();
                let bal = map.get(&entity_id).copied().unwrap_or(0);
                if bal < amount {
                    return Err(sp_runtime::DispatchError::Other("InsufficientPool"));
                }
                map.insert(entity_id, bal - amount);
                Ok(())
            })
        }
    }

    // -- Mock Runtime --
    frame_support::construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            CommissionPoolReward: pallet,
        }
    );

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
    impl frame_system::Config for Test {
        type Block = frame_system::mocking::MockBlock<Test>;
        type AccountData = pallet_balances::AccountData<Balance>;
    }

    #[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
    impl pallet_balances::Config for Test {
        type AccountStore = System;
        type Balance = Balance;
    }

    frame_support::parameter_types! {
        pub const DefaultRoundDuration: u64 = 100;
    }

    impl pallet::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type MemberProvider = MockMemberProvider;
        type EntityProvider = MockEntityProvider;
        type PoolBalanceProvider = MockPoolBalanceProvider;
        type MaxPoolRewardLevels = ConstU32<10>;
        type DefaultRoundDuration = DefaultRoundDuration;
        type MaxClaimHistory = ConstU32<5>;
        type TokenBalance = u128;
        type TokenPoolBalanceProvider = MockTokenPoolBalanceProvider;
        type TokenTransferProvider = MockTokenTransferProvider;
    }

    /// Entity account = entity_id + 9000
    const ENTITY_ACCOUNT: u64 = 9001; // entity_id=1

    fn new_test_ext() -> sp_io::TestExternalities {
        clear_mocks();
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
            // Fund entity account so transfers work
            let _ = pallet_balances::Pallet::<Test>::force_set_balance(
                RuntimeOrigin::root(), ENTITY_ACCOUNT, 1_000_000,
            );
        });
        ext
    }

    fn setup_config(entity_id: u64) {
        // level_1=5000bps(50%), level_2=5000bps(50%), sum=10000
        let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
            vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::root(), entity_id, ratios, 100,
        ));
    }

    // ====================================================================
    // Config tests
    // ====================================================================

    #[test]
    fn set_config_works() {
        new_test_ext().execute_with(|| {
            let ratios = vec![(1u8, 3000u16), (2, 7000)];
            assert_ok!(CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 200,
            ));
            let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.level_ratios.len(), 2);
            assert_eq!(config.round_duration, 200);
        });
    }

    #[test]
    fn set_config_rejects_ratio_sum_mismatch() {
        new_test_ext().execute_with(|| {
            let ratios = vec![(1u8, 3000u16), (2, 3000)]; // sum=6000
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 100,
                ),
                Error::<Test>::RatioSumMismatch
            );
        });
    }

    #[test]
    fn set_config_rejects_zero_ratio() {
        new_test_ext().execute_with(|| {
            let ratios = vec![(1u8, 0u16), (2, 10000)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 100,
                ),
                Error::<Test>::InvalidRatio
            );
        });
    }

    #[test]
    fn set_config_rejects_duplicate_level() {
        new_test_ext().execute_with(|| {
            let ratios = vec![(1u8, 5000u16), (1, 5000)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 100,
                ),
                Error::<Test>::DuplicateLevelId
            );
        });
    }

    #[test]
    fn set_config_rejects_zero_duration() {
        new_test_ext().execute_with(|| {
            let ratios = vec![(1u8, 10000u16)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 0,
                ),
                Error::<Test>::InvalidRoundDuration
            );
        });
    }

    #[test]
    fn set_config_requires_root() {
        new_test_ext().execute_with(|| {
            let ratios = vec![(1u8, 10000u16)];
            assert_noop!(
                CommissionPoolReward::set_pool_reward_config(
                    RuntimeOrigin::signed(1), 1, ratios.try_into().unwrap(), 100,
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    // ====================================================================
    // Round tests
    // ====================================================================

    #[test]
    fn first_claim_creates_round() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 2);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            assert!(pallet::CurrentRound::<Test>::get(entity_id).is_none());

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round.round_id, 1);
            assert_eq!(round.start_block, 1);
            assert_eq!(round.pool_snapshot, 10_000);
            assert_eq!(round.level_snapshots.len(), 2);
        });
    }

    #[test]
    fn round_persists_within_duration() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_member(entity_id, 20, 1);
            set_level_count(entity_id, 1, 2);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            // First claim at block 1 creates round 1
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));
            let round1 = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round1.round_id, 1);

            // Second claim at block 50 (within round_duration=100)
            System::set_block_number(50);
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(20), entity_id,
            ));
            let round_still = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round_still.round_id, 1); // same round
        });
    }

    #[test]
    fn round_rolls_over_after_expiry() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 2);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            // Claim at block 1 → round 1
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));
            assert_eq!(pallet::CurrentRound::<Test>::get(entity_id).unwrap().round_id, 1);

            // Advance past round_duration=100 → block 101
            System::set_block_number(101);
            // Claim triggers new round
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));
            assert_eq!(pallet::CurrentRound::<Test>::get(entity_id).unwrap().round_id, 2);
        });
    }

    #[test]
    fn force_new_round_works() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 5_000);

            assert_ok!(CommissionPoolReward::force_new_round(
                RuntimeOrigin::root(), entity_id,
            ));
            let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round.round_id, 1);

            // Force again creates round 2
            assert_ok!(CommissionPoolReward::force_new_round(
                RuntimeOrigin::root(), entity_id,
            ));
            let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round.round_id, 2);
        });
    }

    // ====================================================================
    // Claim tests
    // ====================================================================

    #[test]
    fn basic_claim_works() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 2);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            let balance_before = pallet_balances::Pallet::<Test>::free_balance(10);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // level_1: 10000 * 5000 / 10000 / 2 = 2500
            let expected_reward: Balance = 2500;
            let balance_after = pallet_balances::Pallet::<Test>::free_balance(10);
            assert_eq!(balance_after - balance_before, expected_reward);

            // Pool deducted
            assert_eq!(get_pool_balance(entity_id), 10_000 - expected_reward);

            // Last claimed round updated
            assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 1);
        });
    }

    #[test]
    fn claim_correct_amount_per_level() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1); // level 1
            set_member(entity_id, 20, 2); // level 2
            set_level_count(entity_id, 1, 5);  // 5 members in level 1
            set_level_count(entity_id, 2, 2);  // 2 members in level 2
            set_pool_balance(entity_id, 10_000);

            // level_1: 10000 * 5000/10000 / 5 = 1000
            // level_2: 10000 * 5000/10000 / 2 = 2500

            let bal_10_before = pallet_balances::Pallet::<Test>::free_balance(10);
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));
            assert_eq!(pallet_balances::Pallet::<Test>::free_balance(10) - bal_10_before, 1000);

            let bal_20_before = pallet_balances::Pallet::<Test>::free_balance(20);
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(20), entity_id,
            ));
            assert_eq!(pallet_balances::Pallet::<Test>::free_balance(20) - bal_20_before, 2500);
        });
    }

    #[test]
    fn claim_rejects_non_member() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_pool_balance(entity_id, 10_000);
            // account 10 is NOT a member
            assert_noop!(
                CommissionPoolReward::claim_pool_reward(
                    RuntimeOrigin::signed(10), entity_id,
                ),
                Error::<Test>::NotMember
            );
        });
    }

    #[test]
    fn claim_rejects_unconfigured_level() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id); // only level 1 & 2 configured
            set_member(entity_id, 10, 0); // level 0: not in config
            set_pool_balance(entity_id, 10_000);
            assert_noop!(
                CommissionPoolReward::claim_pool_reward(
                    RuntimeOrigin::signed(10), entity_id,
                ),
                Error::<Test>::LevelNotConfigured
            );
        });
    }

    #[test]
    fn double_claim_rejected() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));
            assert_noop!(
                CommissionPoolReward::claim_pool_reward(
                    RuntimeOrigin::signed(10), entity_id,
                ),
                Error::<Test>::AlreadyClaimed
            );
        });
    }

    #[test]
    fn level_quota_exhausted() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            // Snapshot: level_1 has 1 member
            set_member(entity_id, 10, 1);
            set_member(entity_id, 20, 1); // will try to claim same level
            set_level_count(entity_id, 1, 1); // snapshot count = 1
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            // First claim by 10 succeeds (claimed_count=1, member_count=1)
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // Second claim by 20 (same level) fails: quota exhausted
            assert_noop!(
                CommissionPoolReward::claim_pool_reward(
                    RuntimeOrigin::signed(20), entity_id,
                ),
                Error::<Test>::LevelQuotaExhausted
            );
        });
    }

    #[test]
    fn claim_deducts_pool_balance() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // level_1: 10000 * 5000/10000 / 1 = 5000
            assert_eq!(get_pool_balance(entity_id), 5_000);
        });
    }

    #[test]
    fn zero_member_level_no_reward() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 0); // 0 members in level 2
            set_pool_balance(entity_id, 10_000);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // level_1: 10000 * 5000/10000 / 1 = 5000
            // level_2: per_member=0 (0 members), 5000 allocation stays in pool
            assert_eq!(get_pool_balance(entity_id), 5_000);
        });
    }

    #[test]
    fn config_not_found_error() {
        new_test_ext().execute_with(|| {
            set_member(1, 10, 1);
            assert_noop!(
                CommissionPoolReward::claim_pool_reward(
                    RuntimeOrigin::signed(10), 1,
                ),
                Error::<Test>::ConfigNotFound
            );
        });
    }

    // ====================================================================
    // Claim history tests
    // ====================================================================

    #[test]
    fn claim_history_recorded() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 2);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            let records = pallet::ClaimRecords::<Test>::get(entity_id, 10);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].round_id, 1);
            assert_eq!(records[0].amount, 2500); // 10000*5000/10000/2
            assert_eq!(records[0].level_id, 1);
            assert_eq!(records[0].claimed_at, 1);
        });
    }

    #[test]
    fn claim_history_multi_rounds() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);

            // Round 1
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // Advance to round 2
            System::set_block_number(101);
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            let records = pallet::ClaimRecords::<Test>::get(entity_id, 10);
            assert_eq!(records.len(), 2);
            assert_eq!(records[0].round_id, 1);
            assert_eq!(records[1].round_id, 2);
        });
    }

    #[test]
    fn claim_history_evicts_oldest() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 1_000_000);

            // MaxClaimHistory = 5, so claim 6 rounds to trigger eviction
            for i in 0..6u64 {
                System::set_block_number(1 + i * 101);
                assert_ok!(CommissionPoolReward::claim_pool_reward(
                    RuntimeOrigin::signed(10), entity_id,
                ));
            }

            let records = pallet::ClaimRecords::<Test>::get(entity_id, 10);
            assert_eq!(records.len(), 5); // MaxClaimHistory
            assert_eq!(records[0].round_id, 2); // round 1 evicted
            assert_eq!(records[4].round_id, 6);
        });
    }

    // ====================================================================
    // PlanWriter tests
    // ====================================================================

    #[test]
    fn plan_writer_set_config() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::PoolRewardPlanWriter;
            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
                1,
                vec![(1, 3000), (2, 7000)],
                43200,
            ));
            let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.level_ratios.len(), 2);
            assert_eq!(config.level_ratios[0], (1, 3000));
            assert_eq!(config.round_duration, 43200);
        });
    }

    #[test]
    fn plan_writer_clear_config() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::PoolRewardPlanWriter;
            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
                1, vec![(1, 10000)], 100,
            ));
            assert!(pallet::PoolRewardConfigs::<Test>::get(1).is_some());

            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::clear_config(1));
            assert!(pallet::PoolRewardConfigs::<Test>::get(1).is_none());
            assert!(pallet::CurrentRound::<Test>::get(1).is_none());
        });
    }

    // ====================================================================
    // Token dual-pool tests
    // ====================================================================

    /// 辅助：创建启用 Token 池的配置
    fn setup_config_with_token(entity_id: u64) {
        setup_config(entity_id);
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::root(), entity_id, true,
        ));
    }

    #[test]
    fn set_token_pool_enabled_works() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id);
            let config = pallet::PoolRewardConfigs::<Test>::get(entity_id).unwrap();
            assert!(!config.token_pool_enabled);

            assert_ok!(CommissionPoolReward::set_token_pool_enabled(
                RuntimeOrigin::root(), entity_id, true,
            ));
            let config = pallet::PoolRewardConfigs::<Test>::get(entity_id).unwrap();
            assert!(config.token_pool_enabled);
        });
    }

    #[test]
    fn set_token_pool_enabled_requires_config() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                CommissionPoolReward::set_token_pool_enabled(
                    RuntimeOrigin::root(), 999, true,
                ),
                Error::<Test>::ConfigNotFound
            );
        });
    }

    #[test]
    fn round_includes_token_snapshot_when_enabled() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config_with_token(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 2);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);
            set_token_pool_balance(entity_id, 5_000);

            // Fund entity account for NEX transfer
            assert_ok!(CommissionPoolReward::force_new_round(
                RuntimeOrigin::root(), entity_id,
            ));

            let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round.token_pool_snapshot, Some(5_000));
            assert!(round.token_level_snapshots.is_some());
            let token_snaps = round.token_level_snapshots.unwrap();
            assert_eq!(token_snaps.len(), 2);
            // level_1: 5000 * 5000/10000 / 2 = 1250
            assert_eq!(token_snaps[0].per_member_reward, 1250);
            // level_2: 5000 * 5000/10000 / 1 = 2500
            assert_eq!(token_snaps[1].per_member_reward, 2500);
        });
    }

    #[test]
    fn round_no_token_snapshot_when_disabled() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config(entity_id); // token_pool_enabled = false
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);
            set_token_pool_balance(entity_id, 5_000);

            assert_ok!(CommissionPoolReward::force_new_round(
                RuntimeOrigin::root(), entity_id,
            ));

            let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
            assert_eq!(round.token_pool_snapshot, None);
            assert!(round.token_level_snapshots.is_none());
        });
    }

    #[test]
    fn claim_dual_pool_nex_and_token() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config_with_token(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);
            set_token_pool_balance(entity_id, 6_000);
            // Fund entity account with tokens for transfer
            set_token_balance(entity_id, ENTITY_ACCOUNT, 6_000);

            let nex_before = pallet_balances::Pallet::<Test>::free_balance(10);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // NEX: level_1 = 10000 * 5000/10000 / 1 = 5000
            let nex_after = pallet_balances::Pallet::<Test>::free_balance(10);
            assert_eq!(nex_after - nex_before, 5000);
            assert_eq!(get_pool_balance(entity_id), 5_000);

            // Token: level_1 = 6000 * 5000/10000 / 1 = 3000
            assert_eq!(get_token_balance(entity_id, 10), 3000);
            assert_eq!(get_token_pool_balance(entity_id), 3_000);

            // Claim record includes token_amount
            let records = pallet::ClaimRecords::<Test>::get(entity_id, 10);
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].amount, 5000);
            assert_eq!(records[0].token_amount, 3000);
        });
    }

    #[test]
    fn claim_token_best_effort_nex_still_works() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;
            setup_config_with_token(entity_id);
            set_member(entity_id, 10, 1);
            set_level_count(entity_id, 1, 1);
            set_level_count(entity_id, 2, 1);
            set_pool_balance(entity_id, 10_000);
            set_token_pool_balance(entity_id, 6_000);
            // Entity account has NO token balance → token transfer will fail
            // but NEX claim should still succeed

            let nex_before = pallet_balances::Pallet::<Test>::free_balance(10);

            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));

            // NEX claim succeeded
            let nex_after = pallet_balances::Pallet::<Test>::free_balance(10);
            assert_eq!(nex_after - nex_before, 5000);

            // Token claim was skipped (best-effort)
            assert_eq!(get_token_balance(entity_id, 10), 0);
            // Token pool NOT deducted
            assert_eq!(get_token_pool_balance(entity_id), 6_000);

            // Claim record has token_amount = 0
            let records = pallet::ClaimRecords::<Test>::get(entity_id, 10);
            assert_eq!(records[0].token_amount, 0);
        });
    }

    #[test]
    fn plan_writer_set_token_pool_enabled() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::PoolRewardPlanWriter;
            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
                1, vec![(1, 10000)], 100,
            ));
            let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
            assert!(!config.token_pool_enabled);

            assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_token_pool_enabled(1, true));
            let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
            assert!(config.token_pool_enabled);
        });
    }
}
