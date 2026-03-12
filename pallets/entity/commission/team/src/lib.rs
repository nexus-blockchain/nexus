//! # Commission Team Performance Plugin (pallet-commission-team)
//!
//! 团队业绩返佣插件：基于推荐链上级的团队累计销售额，按阶梯比例发放奖金。
//!
//! ## 核心逻辑
//!
//! 当买家下单时，沿推荐链向上遍历（最多 `max_depth` 层），
//! 对每个上级查询其团队统计（team_size, total_spent），
//! 匹配最高达标的阶梯档位，按该档位比例对当前订单金额计算奖金。
//!
//! ## 与其他模式的区别
//!
//! - `LEVEL_DIFF`：按等级差价，每笔订单只取差额
//! - `TEAM_PERFORMANCE`：按团队累计业绩阶梯，每笔订单按档位比例发放

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::collections::BTreeSet;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Get},
    };
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        CommissionOutput, CommissionType, MemberProvider,
    };
    use pallet_entity_common::{EntityProvider, AdminPermission};
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    /// 团队业绩阶梯档位
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct TeamPerformanceTier<Balance> {
        /// 团队累计销售额门槛
        pub sales_threshold: Balance,
        /// 团队最小人数门槛（0 = 不限制）
        pub min_team_size: u32,
        /// 奖金比例（基点，500 = 5%）
        pub rate: u16,
    }

    /// 团队业绩门槛数据源模式
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum SalesThresholdMode {
        /// 使用 get_member_stats 返回的 total_spent（u128，来自 MemberProvider）
        Nex = 0,
        /// 使用 get_member_spent_usdt 返回的 USDT 累计（精度 10^6）
        Usdt = 1,
    }

    impl Default for SalesThresholdMode {
        fn default() -> Self { Self::Nex }
    }

    /// 团队业绩返佣配置（per-entity）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxTiers))]
    pub struct TeamPerformanceConfig<Balance, MaxTiers: Get<u32>> {
        /// 阶梯档位列表（按 sales_threshold 升序排列）
        pub tiers: BoundedVec<TeamPerformanceTier<Balance>, MaxTiers>,
        /// 沿推荐链向上最大遍历深度
        pub max_depth: u8,
        /// 是否允许多层叠加（false = 仅最近一个达标上级获得奖金）
        pub allow_stacking: bool,
        /// 门槛数据源模式（Nex=使用 NEX 累计, Usdt=使用 USDT 累计消费）
        pub threshold_mode: SalesThresholdMode,
    }

    impl<Balance: Default, MaxTiers: Get<u32>> Default for TeamPerformanceConfig<Balance, MaxTiers> {
        fn default() -> Self {
            Self {
                tiers: BoundedVec::default(),
                max_depth: 5,
                allow_stacking: false,
                threshold_mode: SalesThresholdMode::Nex,
            }
        }
    }

    pub type TeamPerformanceConfigOf<T> =
        TeamPerformanceConfig<BalanceOf<T>, <T as Config>::MaxTeamTiers>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 最大阶梯档位数
        #[pallet::constant]
        type MaxTeamTiers: Get<u32>;

        /// 权重
        type WeightInfo: WeightInfo;

        /// Benchmark helper for setting up external state.
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self::AccountId>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // F6: integrity_test 运行时完整性校验
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(
                T::MaxTeamTiers::get() > 0,
                "MaxTeamTiers must be > 0"
            );
            // L3-R6: match_tier_with_index 返回 u8 索引，MaxTeamTiers 超过 255 会导致截断
            assert!(
                T::MaxTeamTiers::get() <= 255,
                "MaxTeamTiers must be <= 255 (tier index stored as u8)"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// 团队业绩配置 entity_id -> TeamPerformanceConfig
    #[pallet::storage]
    #[pallet::getter(fn team_performance_config)]
    pub type TeamPerformanceConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        TeamPerformanceConfigOf<T>,
    >;

    /// F2: 团队业绩返佣启用状态（配置存在时默认 true）
    #[pallet::storage]
    pub type TeamPerformanceEnabled<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        bool,
        ValueQuery,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// F7: 配置更新（增强信息）
        TeamPerformanceConfigUpdated {
            entity_id: u64,
            tier_count: u32,
            max_depth: u8,
            allow_stacking: bool,
            threshold_mode: SalesThresholdMode,
        },
        TeamPerformanceConfigCleared { entity_id: u64 },
        /// F2: 团队业绩返佣已暂停
        TeamPerformancePaused { entity_id: u64 },
        /// F2: 团队业绩返佣已恢复
        TeamPerformanceResumed { entity_id: u64 },
        /// F3: 档位已添加
        TeamTierAdded { entity_id: u64, tier_index: u32 },
        /// F3: 档位已更新
        TeamTierUpdated { entity_id: u64, tier_index: u32 },
        /// F3: 档位已移除
        TeamTierRemoved { entity_id: u64, tier_index: u32 },
        /// F7: 团队业绩佣金发放事件（NEX 路径）
        TeamCommissionAwarded {
            entity_id: u64,
            beneficiary: T::AccountId,
            tier_index: u8,
            rate: u16,
            amount: BalanceOf<T>,
            depth: u8,
        },
        /// M1-R6: Token 路径团队阶梯匹配事件
        ///
        /// Token 佣金金额由 core 的 TokenCommissionDistributed 事件记录，
        /// 此事件补充 team 插件特有的 tier_index/rate 信息。
        TokenTeamTierMatched {
            entity_id: u64,
            beneficiary: T::AccountId,
            tier_index: u8,
            rate: u16,
            depth: u8,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// 费率无效（超过 10000 基点）
        InvalidRate,
        /// 档位数为 0
        EmptyTiers,
        /// 遍历深度无效
        InvalidMaxDepth,
        /// 阶梯门槛未严格递增
        TiersNotAscending,
        /// 实体不存在
        EntityNotFound,
        /// 非实体所有者或无 COMMISSION_MANAGE 权限
        NotEntityOwnerOrAdmin,
        /// 配置不存在（清除/更新时）
        ConfigNotFound,
        /// 更新参数全部为 None（无操作）
        NothingToUpdate,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// F1: 实体未激活（暂停/封禁）
        EntityNotActive,
        /// F2: 团队业绩返佣已暂停
        TeamPerformanceIsPaused,
        /// F2: 团队业绩返佣未暂停（恢复时检查）
        TeamPerformanceNotPaused,
        /// F3: 档位索引越界
        TierIndexOutOfBounds,
        /// F3: 档位数已达上限
        TierLimitReached,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置团队业绩返佣配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_team_performance_config(tiers.len() as u32))]
        pub fn set_team_performance_config(
            origin: OriginFor<T>,
            entity_id: u64,
            tiers: BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
            max_depth: u8,
            allow_stacking: bool,
            threshold_mode: SalesThresholdMode,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            Self::validate_tiers(&tiers, max_depth)?;

            let tier_count = tiers.len() as u32;
            TeamPerformanceConfigs::<T>::insert(entity_id, TeamPerformanceConfig {
                tiers,
                max_depth,
                allow_stacking,
                threshold_mode,
            });
            // F2: 新配置默认启用
            TeamPerformanceEnabled::<T>::insert(entity_id, true);

            Self::deposit_event(Event::TeamPerformanceConfigUpdated {
                entity_id, tier_count, max_depth, allow_stacking, threshold_mode,
            });
            Ok(())
        }

        /// 清除团队业绩返佣配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_team_performance_config())]
        pub fn clear_team_performance_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(TeamPerformanceConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            TeamPerformanceConfigs::<T>::remove(entity_id);
            TeamPerformanceEnabled::<T>::remove(entity_id);
            Self::deposit_event(Event::TeamPerformanceConfigCleared { entity_id });
            Ok(())
        }

        /// 部分更新团队业绩参数（不重提 tiers）
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::update_team_performance_params())]
        pub fn update_team_performance_params(
            origin: OriginFor<T>,
            entity_id: u64,
            max_depth: Option<u8>,
            allow_stacking: Option<bool>,
            threshold_mode: Option<SalesThresholdMode>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(
                max_depth.is_some() || allow_stacking.is_some() || threshold_mode.is_some(),
                Error::<T>::NothingToUpdate
            );

            // M2 审计修复: 从 try_mutate 闭包返回事件数据，避免冗余 storage read
            let (tier_count, depth_val, stacking_val, mode_val) =
                TeamPerformanceConfigs::<T>::try_mutate(entity_id, |maybe| -> Result<(u32, u8, bool, SalesThresholdMode), DispatchError> {
                    let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                    if let Some(d) = max_depth {
                        ensure!(d > 0 && d <= 30, Error::<T>::InvalidMaxDepth);
                        config.max_depth = d;
                    }
                    if let Some(s) = allow_stacking {
                        config.allow_stacking = s;
                    }
                    if let Some(m) = threshold_mode {
                        config.threshold_mode = m;
                    }
                    Ok((config.tiers.len() as u32, config.max_depth, config.allow_stacking, config.threshold_mode))
                })?;

            Self::deposit_event(Event::TeamPerformanceConfigUpdated {
                entity_id,
                tier_count,
                max_depth: depth_val,
                allow_stacking: stacking_val,
                threshold_mode: mode_val,
            });
            Ok(())
        }

        /// [Root] 强制设置团队业绩返佣配置
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::force_set_team_performance_config(tiers.len() as u32))]
        pub fn force_set_team_performance_config(
            origin: OriginFor<T>,
            entity_id: u64,
            tiers: BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
            max_depth: u8,
            allow_stacking: bool,
            threshold_mode: SalesThresholdMode,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::validate_tiers(&tiers, max_depth)?;

            let tier_count = tiers.len() as u32;
            TeamPerformanceConfigs::<T>::insert(entity_id, TeamPerformanceConfig {
                tiers,
                max_depth,
                allow_stacking,
                threshold_mode,
            });
            TeamPerformanceEnabled::<T>::insert(entity_id, true);

            Self::deposit_event(Event::TeamPerformanceConfigUpdated {
                entity_id, tier_count, max_depth, allow_stacking, threshold_mode,
            });
            Ok(())
        }

        /// [Root] 强制清除团队业绩返佣配置
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::force_clear_team_performance_config())]
        pub fn force_clear_team_performance_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if TeamPerformanceConfigs::<T>::contains_key(entity_id) {
                TeamPerformanceConfigs::<T>::remove(entity_id);
                TeamPerformanceEnabled::<T>::remove(entity_id);
                Self::deposit_event(Event::TeamPerformanceConfigCleared { entity_id });
            }
            Ok(())
        }

        // ====================================================================
        // F2: 暂停/恢复团队业绩返佣
        // ====================================================================

        /// F2: 暂停团队业绩返佣（保留配置不删除）
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::pause_team_performance())]
        pub fn pause_team_performance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(TeamPerformanceConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(TeamPerformanceEnabled::<T>::get(entity_id), Error::<T>::TeamPerformanceIsPaused);

            TeamPerformanceEnabled::<T>::insert(entity_id, false);
            Self::deposit_event(Event::TeamPerformancePaused { entity_id });
            Ok(())
        }

        /// F2: 恢复团队业绩返佣
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::resume_team_performance())]
        pub fn resume_team_performance(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(TeamPerformanceConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);
            ensure!(!TeamPerformanceEnabled::<T>::get(entity_id), Error::<T>::TeamPerformanceNotPaused);

            TeamPerformanceEnabled::<T>::insert(entity_id, true);
            Self::deposit_event(Event::TeamPerformanceResumed { entity_id });
            Ok(())
        }

        // ====================================================================
        // F3: 单个档位 CRUD
        // ====================================================================

        /// F3: 添加新档位（插入到正确位置以保持 sales_threshold 升序）
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::add_tier())]
        pub fn add_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            tier: TeamPerformanceTier<BalanceOf<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);

            TeamPerformanceConfigs::<T>::try_mutate(entity_id, |maybe| -> DispatchResult {
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                // 找到插入位置（保持 sales_threshold 严格升序）
                let insert_pos = config.tiers.iter().position(|t| t.sales_threshold >= tier.sales_threshold);
                if let Some(pos) = insert_pos {
                    ensure!(config.tiers[pos].sales_threshold != tier.sales_threshold, Error::<T>::TiersNotAscending);
                    config.tiers.try_insert(pos, tier).map_err(|_| Error::<T>::TierLimitReached)?;
                    Self::deposit_event(Event::TeamTierAdded { entity_id, tier_index: pos as u32 });
                } else {
                    config.tiers.try_push(tier).map_err(|_| Error::<T>::TierLimitReached)?;
                    Self::deposit_event(Event::TeamTierAdded { entity_id, tier_index: (config.tiers.len() - 1) as u32 });
                }
                Ok(())
            })?;
            Ok(())
        }

        /// F3: 更新指定索引的档位
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::update_tier())]
        pub fn update_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            tier_index: u32,
            sales_threshold: Option<BalanceOf<T>>,
            min_team_size: Option<u32>,
            rate: Option<u16>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(
                sales_threshold.is_some() || min_team_size.is_some() || rate.is_some(),
                Error::<T>::NothingToUpdate
            );

            TeamPerformanceConfigs::<T>::try_mutate(entity_id, |maybe| -> DispatchResult {
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = tier_index as usize;
                ensure!(idx < config.tiers.len(), Error::<T>::TierIndexOutOfBounds);

                if let Some(r) = rate {
                    ensure!(r <= 10000, Error::<T>::InvalidRate);
                    config.tiers[idx].rate = r;
                }
                if let Some(mts) = min_team_size {
                    config.tiers[idx].min_team_size = mts;
                }
                if let Some(st) = sales_threshold {
                    config.tiers[idx].sales_threshold = st;
                    // 重新验证升序约束
                    for window in config.tiers.windows(2) {
                        ensure!(
                            window[1].sales_threshold > window[0].sales_threshold,
                            Error::<T>::TiersNotAscending
                        );
                    }
                }
                Ok(())
            })?;

            Self::deposit_event(Event::TeamTierUpdated { entity_id, tier_index });
            Ok(())
        }

        /// F3: 移除指定索引的档位
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::remove_tier())]
        pub fn remove_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            tier_index: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            TeamPerformanceConfigs::<T>::try_mutate(entity_id, |maybe| -> DispatchResult {
                let config = maybe.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = tier_index as usize;
                ensure!(idx < config.tiers.len(), Error::<T>::TierIndexOutOfBounds);
                // 不允许删除最后一个档位（请使用 clear_team_performance_config）
                ensure!(config.tiers.len() > 1, Error::<T>::EmptyTiers);
                config.tiers.remove(idx);
                Ok(())
            })?;

            Self::deposit_event(Event::TeamTierRemoved { entity_id, tier_index });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation
    // ========================================================================

    impl<T: Config> Pallet<T> {
        /// 验证 Entity Owner 或 Admin(COMMISSION_MANAGE) 权限
        fn ensure_owner_or_admin(entity_id: u64, who: &T::AccountId) -> DispatchResult {
            let owner = T::EntityProvider::entity_owner(entity_id)
                .ok_or(Error::<T>::EntityNotFound)?;
            if *who == owner {
                return Ok(());
            }
            ensure!(
                T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::COMMISSION_MANAGE),
                Error::<T>::NotEntityOwnerOrAdmin
            );
            Ok(())
        }

        /// 校验 tiers 参数合法性
        fn validate_tiers(
            tiers: &BoundedVec<TeamPerformanceTier<BalanceOf<T>>, T::MaxTeamTiers>,
            max_depth: u8,
        ) -> DispatchResult {
            ensure!(!tiers.is_empty(), Error::<T>::EmptyTiers);
            ensure!(max_depth > 0 && max_depth <= 30, Error::<T>::InvalidMaxDepth);
            for tier in tiers.iter() {
                ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            }
            for window in tiers.windows(2) {
                ensure!(
                    window[1].sales_threshold > window[0].sales_threshold,
                    Error::<T>::TiersNotAscending
                );
            }
            Ok(())
        }

        /// 匹配最高达标的阶梯档位（返回索引和费率）
        pub(crate) fn match_tier_with_index(
            tiers: &[TeamPerformanceTier<BalanceOf<T>>],
            team_size: u32,
            total_spent: u128,
        ) -> Option<(u8, u16)> {
            let mut matched: Option<(u8, u16)> = None;

            for (i, tier) in tiers.iter().enumerate() {
                let threshold_u128: u128 =
                    sp_runtime::SaturatedConversion::saturated_into(tier.sales_threshold);

                // TM-M1 审计修复: 仅在 sales_threshold 不满足时 break（阶梯升序保证）。
                // min_team_size 不要求单调递增，跳过但不 break，以免遗漏更高档位。
                if total_spent < threshold_u128 {
                    break;
                }
                if tier.min_team_size == 0 || team_size >= tier.min_team_size {
                    matched = Some((i as u8, tier.rate));
                }
            }

            matched
        }

        // ====================================================================
        // F4: 阶梯匹配查询（前端展示）
        // ====================================================================

        /// F4: 查询指定会员当前匹配的阶梯档位
        ///
        /// 返回 (tier_index, rate, next_tier_threshold, next_tier_min_team_size)
        /// next_tier_* 为 None 表示已达最高档位
        pub fn get_matched_tier_for_account(
            entity_id: u64,
            account: &T::AccountId,
        ) -> Option<(u8, u16, Option<BalanceOf<T>>, Option<u32>)> {
            let config = TeamPerformanceConfigs::<T>::get(entity_id)?;
            let (_direct, team_size, nex_spent) =
                T::MemberProvider::get_member_stats(entity_id, account);
            let total_spent = match config.threshold_mode {
                SalesThresholdMode::Nex => nex_spent,
                SalesThresholdMode::Usdt => {
                    T::MemberProvider::get_member_spent_usdt(entity_id, account) as u128
                }
            };

            let (tier_idx, rate) = Self::match_tier_with_index(&config.tiers, team_size, total_spent)?;
            let next_idx = (tier_idx as usize) + 1;
            let (next_threshold, next_team_size) = if next_idx < config.tiers.len() {
                (Some(config.tiers[next_idx].sales_threshold), Some(config.tiers[next_idx].min_team_size))
            } else {
                (None, None)
            };
            Some((tier_idx, rate, next_threshold, next_team_size))
        }

        /// F4: 查询团队业绩返佣状态
        ///
        /// 返回 (config_exists, is_enabled)
        pub fn get_team_performance_status(entity_id: u64) -> (bool, bool) {
            let exists = TeamPerformanceConfigs::<T>::contains_key(entity_id);
            let enabled = TeamPerformanceEnabled::<T>::get(entity_id);
            (exists, enabled)
        }

        /// 处理团队业绩返佣
        pub fn process_team_performance(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            config: &TeamPerformanceConfigOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) where T::AccountId: Ord {
            if config.tiers.is_empty() { return; }

            let mut current = T::MemberProvider::get_referrer(entity_id, buyer);
            let mut depth: u8 = 0;
            // H1 审计修复: 循环检测，防止推荐链有环时重复发放佣金
            let mut visited = BTreeSet::new();

            while let Some(ref ancestor) = current {
                // H1: 检测循环
                if !visited.insert(ancestor.clone()) { break; }
                depth += 1;
                if depth > config.max_depth { break; }
                if remaining.is_zero() { break; }

                // M1 审计修复: 跳过非会员推荐人（会员被移除但推荐链未清理的边缘情况）
                if !T::MemberProvider::is_member(entity_id, ancestor) {
                    current = T::MemberProvider::get_referrer(entity_id, ancestor);
                    continue;
                }

                // P1: 跳过被封禁或未激活会员
                // M1-R5: 补充 is_member_active 检查（与 referral/multi-level 一致）
                if T::MemberProvider::is_banned(entity_id, ancestor)
                    || !T::MemberProvider::is_activated(entity_id, ancestor)
                    || !T::MemberProvider::is_member_active(entity_id, ancestor)
                {
                    current = T::MemberProvider::get_referrer(entity_id, ancestor);
                    continue;
                }

                // 查询团队统计：(direct_referrals, team_size, total_spent)
                let (_direct, team_size, nex_spent) =
                    T::MemberProvider::get_member_stats(entity_id, ancestor);
                let total_spent = match config.threshold_mode {
                    SalesThresholdMode::Nex => nex_spent,
                    SalesThresholdMode::Usdt => {
                        T::MemberProvider::get_member_spent_usdt(entity_id, ancestor) as u128
                    }
                };

                if let Some((tier_idx, rate)) = Self::match_tier_with_index(&config.tiers, team_size, total_spent) {
                    if rate > 0 {
                        let commission = order_amount
                            .saturating_mul(rate.into())
                            / 10000u32.into();
                        let actual = commission.min(*remaining);

                        if !actual.is_zero() {
                            *remaining = remaining.saturating_sub(actual);
                            outputs.push(CommissionOutput {
                                beneficiary: ancestor.clone(),
                                amount: actual,
                                commission_type: CommissionType::TeamPerformance,
                                level: depth,
                            });
                            // F7: 发射佣金发放事件
                            Self::deposit_event(Event::TeamCommissionAwarded {
                                entity_id,
                                beneficiary: ancestor.clone(),
                                tier_index: tier_idx,
                                rate,
                                amount: actual,
                                depth,
                            });
                        }

                        // 非叠加模式：仅奖励最近一个达标上级
                        if !config.allow_stacking {
                            break;
                        }
                    }
                }

                current = T::MemberProvider::get_referrer(entity_id, ancestor);
            }
        }
    }
}

// ============================================================================
// CommissionPlugin implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::CommissionPlugin<T::AccountId, pallet::BalanceOf<T>>
    for pallet::Pallet<T>
{
    fn calculate(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (
        alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>,
        pallet::BalanceOf<T>,
    ) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::TEAM_PERFORMANCE) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F2: 暂停时跳过佣金计算
        if !pallet::TeamPerformanceEnabled::<T>::get(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::TeamPerformanceConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_team_performance(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// Token 多资产 — TokenCommissionPlugin implementation
// ============================================================================

use pallet_commission_common::MemberProvider as _;

impl<T: pallet::Config> pallet::Pallet<T> {
    /// Token 版团队业绩计算（泛型，rate-based）
    ///
    /// 阶梯匹配逻辑与 NEX 版完全一致（基于 MemberProvider 的 team_size / total_spent）。
    /// 仅佣金金额计算使用泛型 TB。
    fn process_team_performance_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        config: &pallet::TeamPerformanceConfigOf<T>,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
        T::AccountId: Ord,
    {
        if config.tiers.is_empty() { return; }

        let mut current = T::MemberProvider::get_referrer(entity_id, buyer);
        let mut depth: u8 = 0;
        // H1 审计修复: 循环检测
        let mut visited = alloc::collections::BTreeSet::new();

        while let Some(ref ancestor) = current {
            // H1: 检测循环
            if !visited.insert(ancestor.clone()) { break; }
            depth += 1;
            if depth > config.max_depth { break; }
            if remaining.is_zero() { break; }

            // M1 审计修复: 跳过非会员推荐人
            if !T::MemberProvider::is_member(entity_id, ancestor) {
                current = T::MemberProvider::get_referrer(entity_id, ancestor);
                continue;
            }

            // P1: 跳过被封禁或未激活会员
            // M1-R5: 补充 is_member_active 检查（与 referral/multi-level 一致）
            if T::MemberProvider::is_banned(entity_id, ancestor)
                || !T::MemberProvider::is_activated(entity_id, ancestor)
                || !T::MemberProvider::is_member_active(entity_id, ancestor)
            {
                current = T::MemberProvider::get_referrer(entity_id, ancestor);
                continue;
            }

            let (_direct, team_size, nex_spent) =
                T::MemberProvider::get_member_stats(entity_id, ancestor);
            let total_spent = match config.threshold_mode {
                pallet::SalesThresholdMode::Nex => nex_spent,
                pallet::SalesThresholdMode::Usdt => {
                    T::MemberProvider::get_member_spent_usdt(entity_id, ancestor) as u128
                }
            };

            // M3 审计修复: 使用 match_tier_with_index 与 NEX 路径一致
            if let Some((tier_idx, rate)) = Self::match_tier_with_index(&config.tiers, team_size, total_spent) {
                if rate > 0 {
                    let commission = order_amount
                        .saturating_mul(TB::from(rate as u32))
                        / TB::from(10000u32);
                    let actual = commission.min(*remaining);

                    if !actual.is_zero() {
                        *remaining = remaining.saturating_sub(actual);
                        outputs.push(pallet_commission_common::CommissionOutput {
                            beneficiary: ancestor.clone(),
                            amount: actual,
                            commission_type: pallet_commission_common::CommissionType::TeamPerformance,
                            level: depth,
                        });
                        // M1-R6: Token 路径也发射阶梯匹配事件（金额由 core 的 TokenCommissionDistributed 记录）
                        Self::deposit_event(pallet::Event::TokenTeamTierMatched {
                            entity_id,
                            beneficiary: ancestor.clone(),
                            tier_index: tier_idx,
                            rate,
                            depth,
                        });
                    }

                    if !config.allow_stacking {
                        break;
                    }
                }
            }

            current = T::MemberProvider::get_referrer(entity_id, ancestor);
        }
    }
}

impl<T: pallet::Config, TB> pallet_commission_common::TokenCommissionPlugin<T::AccountId, TB>
    for pallet::Pallet<T>
where
    TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
{
    fn calculate_token(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: TB,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (
        alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
        TB,
    ) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::TEAM_PERFORMANCE) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F2: 暂停时跳过佣金计算
        if !pallet::TeamPerformanceEnabled::<T>::get(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::TeamPerformanceConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_team_performance_token::<TB>(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// TeamPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::TeamPlanWriter<pallet::BalanceOf<T>>
    for pallet::Pallet<T>
{
    fn set_team_config(
        entity_id: u64,
        tiers: alloc::vec::Vec<(u128, u32, u16)>,
        max_depth: u8,
        allow_stacking: bool,
        threshold_mode: u8,
    ) -> Result<(), sp_runtime::DispatchError> {
        // TM-M2 审计修复: PlanWriter 路径与 extrinsic 一致的参数校验
        frame_support::ensure!(!tiers.is_empty(), sp_runtime::DispatchError::Other("EmptyTiers"));
        frame_support::ensure!(
            max_depth > 0 && max_depth <= 30,
            sp_runtime::DispatchError::Other("InvalidMaxDepth")
        );
        for &(_, _, rate) in tiers.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        // 校验阶梯门槛严格递增
        for window in tiers.windows(2) {
            frame_support::ensure!(
                window[1].0 > window[0].0,
                sp_runtime::DispatchError::Other("TiersNotAscending")
            );
        }

        let bounded: frame_support::BoundedVec<
            pallet::TeamPerformanceTier<pallet::BalanceOf<T>>,
            T::MaxTeamTiers,
        > = tiers
            .into_iter()
            .map(|(threshold, min_team_size, rate)| pallet::TeamPerformanceTier {
                sales_threshold: sp_runtime::SaturatedConversion::saturated_into(threshold),
                min_team_size,
                rate,
            })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyTiers"))?;

        let mode = match threshold_mode {
            0 => pallet::SalesThresholdMode::Nex,
            1 => pallet::SalesThresholdMode::Usdt,
            _ => return Err(sp_runtime::DispatchError::Other("InvalidThresholdMode")),
        };

        let tier_count = bounded.len() as u32;
        pallet::TeamPerformanceConfigs::<T>::insert(
            entity_id,
            pallet::TeamPerformanceConfig {
                tiers: bounded,
                max_depth,
                allow_stacking,
                threshold_mode: mode,
            },
        );
        // F2: PlanWriter 路径也设置启用状态
        pallet::TeamPerformanceEnabled::<T>::insert(entity_id, true);
        // M1 审计修复: PlanWriter 路径也发出事件
        pallet::Pallet::<T>::deposit_event(pallet::Event::TeamPerformanceConfigUpdated {
            entity_id, tier_count, max_depth, allow_stacking, threshold_mode: mode,
        });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        if pallet::TeamPerformanceConfigs::<T>::contains_key(entity_id) {
            pallet::TeamPerformanceConfigs::<T>::remove(entity_id);
            pallet::TeamPerformanceEnabled::<T>::remove(entity_id);
            pallet::Pallet::<T>::deposit_event(pallet::Event::TeamPerformanceConfigCleared { entity_id });
        }
        Ok(())
    }

    fn governance_pause(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        if !pallet::TeamPerformanceEnabled::<T>::get(entity_id) {
            return Ok(()); // 已暂停，幂等操作
        }
        pallet::TeamPerformanceEnabled::<T>::insert(entity_id, false);
        pallet::Pallet::<T>::deposit_event(pallet::Event::TeamPerformancePaused { entity_id });
        Ok(())
    }

    fn governance_resume(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        // 仅在配置存在时才允许恢复
        frame_support::ensure!(
            pallet::TeamPerformanceConfigs::<T>::contains_key(entity_id),
            sp_runtime::DispatchError::Other("TeamConfigNotFound")
        );
        if pallet::TeamPerformanceEnabled::<T>::get(entity_id) {
            return Ok(()); // 已启用，幂等操作
        }
        pallet::TeamPerformanceEnabled::<T>::insert(entity_id, true);
        pallet::Pallet::<T>::deposit_event(pallet::Event::TeamPerformanceResumed { entity_id });
        Ok(())
    }
}

// ============================================================================
// TeamQueryProvider 实现
// ============================================================================

impl<T: Config> pallet_commission_common::TeamQueryProvider<T::AccountId, BalanceOf<T>> for Pallet<T>
where
    BalanceOf<T>: Into<u128>,
{
    fn matched_tier(entity_id: u64, account: &T::AccountId) -> Option<pallet_commission_common::TeamTierInfo<BalanceOf<T>>> {
        let (tier_index, rate, next_threshold, next_min_team_size) =
            Self::get_matched_tier_for_account(entity_id, account)?;
        Some(pallet_commission_common::TeamTierInfo {
            tier_index,
            rate,
            next_threshold,
            next_min_team_size,
        })
    }

    fn status(entity_id: u64) -> (bool, bool) {
        Self::get_team_performance_status(entity_id)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{
        assert_ok, assert_noop,
        traits::ConstU32,
        derive_impl,
    };
    use pallet_commission_common::CommissionModes;
    use sp_runtime::BuildStorage;

    type Balance = u128;

    // -- Mock MemberProvider --
    use core::cell::RefCell;
    use alloc::collections::{BTreeMap, BTreeSet};

    thread_local! {
        static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
        static MEMBER_STATS: RefCell<BTreeMap<(u64, u64), (u32, u32, u128)>> = RefCell::new(BTreeMap::new());
        static MEMBER_SPENT_USDT: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
        static ENTITY_OWNERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
        static ENTITY_ADMINS: RefCell<BTreeMap<(u64, u64), u32>> = RefCell::new(BTreeMap::new());
        static BANNED_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
        static UNACTIVATED_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
        static ENTITY_LOCKED: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
        static ENTITY_INACTIVE: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
        // M1 审计修复: 支持 is_member 测试
        static NON_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
        // M1-R5: 支持 is_member_active 测试
        static FROZEN_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
    }

    pub struct MockMemberProvider;

    impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
        fn is_member(entity_id: u64, account: &u64) -> bool {
            !NON_MEMBERS.with(|n| n.borrow().contains(&(entity_id, *account)))
        }
        fn get_referrer(entity_id: u64, account: &u64) -> Option<u64> {
            REFERRERS.with(|r| r.borrow().get(&(entity_id, *account)).copied())
        }
        fn get_member_stats(entity_id: u64, account: &u64) -> (u32, u32, u128) {
            MEMBER_STATS.with(|s| s.borrow().get(&(entity_id, *account)).copied().unwrap_or((0, 0, 0)))
        }
        fn uses_custom_levels(_: u64) -> bool { false }
        fn custom_level_id(_: u64, _: &u64) -> u8 { 0 }
        fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
        fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn custom_level_count(_: u64) -> u8 { 0 }
        fn get_member_spent_usdt(entity_id: u64, account: &u64) -> u64 {
            MEMBER_SPENT_USDT.with(|s| s.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
        }
        fn is_banned(entity_id: u64, account: &u64) -> bool {
            BANNED_MEMBERS.with(|b| b.borrow().contains(&(entity_id, *account)))
        }
        fn is_activated(entity_id: u64, account: &u64) -> bool {
            !UNACTIVATED_MEMBERS.with(|u| u.borrow().contains(&(entity_id, *account)))
        }
        fn is_member_active(entity_id: u64, account: &u64) -> bool {
            !FROZEN_MEMBERS.with(|f| f.borrow().contains(&(entity_id, *account)))
        }
        fn get_effective_level(_: u64, _: &u64) -> u8 { 0 }
        fn get_level_discount(_: u64, _: u8) -> u16 { 0 }
        fn update_spent(_: u64, _: &u64, _: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn check_order_upgrade_rules(_: u64, _: &u64, _: u64, _: u64) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    }

    // -- Mock EntityProvider --
    pub struct MockEntityProvider;

    impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
        fn entity_exists(entity_id: u64) -> bool {
            ENTITY_OWNERS.with(|o| o.borrow().contains_key(&entity_id))
        }
        fn is_entity_active(entity_id: u64) -> bool {
            ENTITY_INACTIVE.with(|s| !s.borrow().contains(&entity_id))
        }
        fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> { None }
        fn entity_owner(entity_id: u64) -> Option<u64> {
            ENTITY_OWNERS.with(|o| o.borrow().get(&entity_id).copied())
        }
        fn entity_account(_entity_id: u64) -> u64 { 0 }
        fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn is_entity_admin(entity_id: u64, account: &u64, required_permission: u32) -> bool {
            ENTITY_ADMINS.with(|a| {
                a.borrow().get(&(entity_id, *account))
                    .map(|perms| perms & required_permission == required_permission)
                    .unwrap_or(false)
            })
        }
        fn is_entity_locked(entity_id: u64) -> bool {
            ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
        }
    }

    // -- Mock Runtime --
    frame_support::construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            CommissionTeam: pallet,
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

    impl pallet::Config for Test {
        type Currency = Balances;
        type MemberProvider = MockMemberProvider;
        type EntityProvider = MockEntityProvider;
        type MaxTeamTiers = ConstU32<10>;
        type WeightInfo = ();
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        // L2 审计修复: 每个测试开始前清理 thread-local 状态，防止测试间泄漏
        clear_thread_locals();
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
        });
        ext
    }

    fn setup_chain(entity_id: u64) {
        // 推荐链: 10 → 20 → 30 → 40 → 50 (buyer)
        REFERRERS.with(|r| {
            let mut m = r.borrow_mut();
            m.insert((entity_id, 50), 40);
            m.insert((entity_id, 40), 30);
            m.insert((entity_id, 30), 20);
            m.insert((entity_id, 20), 10);
        });
    }

    fn set_stats(entity_id: u64, account: u64, direct: u32, team_size: u32, total_spent: u128) {
        MEMBER_STATS.with(|s| {
            s.borrow_mut().insert((entity_id, account), (direct, team_size, total_spent));
        });
    }

    fn set_spent_usdt(entity_id: u64, account: u64, usdt: u64) {
        MEMBER_SPENT_USDT.with(|s| {
            s.borrow_mut().insert((entity_id, account), usdt);
        });
    }

    fn clear_thread_locals() {
        REFERRERS.with(|r| r.borrow_mut().clear());
        MEMBER_STATS.with(|s| s.borrow_mut().clear());
        MEMBER_SPENT_USDT.with(|s| s.borrow_mut().clear());
        ENTITY_OWNERS.with(|o| o.borrow_mut().clear());
        ENTITY_ADMINS.with(|a| a.borrow_mut().clear());
        BANNED_MEMBERS.with(|b| b.borrow_mut().clear());
        UNACTIVATED_MEMBERS.with(|u| u.borrow_mut().clear());
        ENTITY_LOCKED.with(|l| l.borrow_mut().clear());
        ENTITY_INACTIVE.with(|s| s.borrow_mut().clear());
        NON_MEMBERS.with(|n| n.borrow_mut().clear());
        FROZEN_MEMBERS.with(|f| f.borrow_mut().clear());
    }

    fn set_entity_owner(entity_id: u64, owner: u64) {
        ENTITY_OWNERS.with(|o| o.borrow_mut().insert(entity_id, owner));
    }

    fn set_entity_admin(entity_id: u64, admin: u64, permissions: u32) {
        ENTITY_ADMINS.with(|a| a.borrow_mut().insert((entity_id, admin), permissions));
    }

    fn set_banned(entity_id: u64, account: u64) {
        BANNED_MEMBERS.with(|b| b.borrow_mut().insert((entity_id, account)));
    }

    fn set_unactivated(entity_id: u64, account: u64) {
        UNACTIVATED_MEMBERS.with(|u| u.borrow_mut().insert((entity_id, account)));
    }

    fn set_non_member(entity_id: u64, account: u64) {
        NON_MEMBERS.with(|n| n.borrow_mut().insert((entity_id, account)));
    }

    fn set_entity_locked(entity_id: u64) {
        ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
    }

    fn set_entity_inactive(entity_id: u64) {
        ENTITY_INACTIVE.with(|s| s.borrow_mut().insert(entity_id));
    }

    fn set_member_frozen(entity_id: u64, account: u64) {
        FROZEN_MEMBERS.with(|f| f.borrow_mut().insert((entity_id, account)));
    }

    // ====================================================================
    // Extrinsic tests
    // ====================================================================

    #[test]
    fn set_config_works_by_owner() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 20, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100),
                1,
                tiers.try_into().unwrap(),
                10,
                false,
                pallet::SalesThresholdMode::Nex,
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 2);
            assert_eq!(config.max_depth, 10);
            assert!(!config.allow_stacking);
            assert_eq!(config.threshold_mode, pallet::SalesThresholdMode::Nex);
        });
    }

    #[test]
    fn set_config_rejects_empty_tiers() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers: Vec<pallet::TeamPerformanceTier<Balance>> = vec![];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::EmptyTiers
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_rate() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 10001 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::InvalidRate
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_depth_zero() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 0, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::InvalidMaxDepth
            );
        });
    }

    #[test]
    fn set_config_rejects_invalid_depth_over_30() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 31, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::InvalidMaxDepth
            );
        });
    }

    #[test]
    fn set_config_rejects_non_ascending_thresholds() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 0, rate: 300 },
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::TiersNotAscending
            );
        });
    }

    #[test]
    fn set_config_rejects_non_owner() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            // account 999 is not owner nor admin
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(999), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::NotEntityOwnerOrAdmin
            );
        });
    }

    #[test]
    fn set_config_rejects_entity_not_found() {
        new_test_ext().execute_with(|| {
            // entity 99 does not exist
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(1), 99, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::EntityNotFound
            );
        });
    }

    // ====================================================================
    // CommissionPlugin calculation tests
    // ====================================================================

    #[test]
    fn no_config_returns_empty() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn mode_not_enabled_returns_empty() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            // 配置存在但模式位未启用
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 100, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::DIRECT_REWARD); // 不含 TEAM_PERFORMANCE
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn single_tier_non_stacking() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: team_size=10, total_spent=5000
            set_stats(1, 40, 3, 10, 5000);
            // account 30: team_size=50, total_spent=20000
            set_stats(1, 30, 8, 50, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 5, rate: 200 },
                pallet::TeamPerformanceTier { sales_threshold: 10000, min_team_size: 20, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            // buyer=50, order=10000
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // non-stacking: 最近达标上级 = account 40 (tier1: 3000/5 → rate 200)
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
            // 10000 * 200 / 10000 = 200
            assert_eq!(outputs[0].amount, 200);
            assert_eq!(outputs[0].level, 1);
            assert_eq!(remaining, 9800);
        });
    }

    #[test]
    fn stacking_mode_rewards_multiple() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            set_stats(1, 40, 3, 10, 5000);  // 达标 tier1
            set_stats(1, 30, 8, 50, 20000); // 达标 tier2

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 5, rate: 200 },
                pallet::TeamPerformanceTier { sales_threshold: 10000, min_team_size: 20, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex, // allow_stacking
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // stacking: account 40 gets tier1 rate=200, account 30 gets tier2 rate=500
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 200); // 10000 * 200 / 10000
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 500); // 10000 * 500 / 10000 (but capped by remaining=9800, 500 < 9800)
            assert_eq!(remaining, 10000 - 200 - 500);
        });
    }

    #[test]
    fn team_size_threshold_filters() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: 足够的销售额但团队太小
            set_stats(1, 40, 1, 3, 5000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 5, rate: 200 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // account 40 团队人数 3 < 5，不达标
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn remaining_caps_commission() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 5000 }, // 50%
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            // remaining 仅 100
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 100, modes, false, 0,
            );

            // 计算 10000*5000/10000=5000 但 capped by remaining=100
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 100);
            assert_eq!(remaining, 0);
        });
    }

    #[test]
    fn max_depth_limits_traversal() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // 只有 account 10 达标（深度4）
            set_stats(1, 10, 10, 100, 50000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 300 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 2, false, pallet::SalesThresholdMode::Nex, // max_depth=2
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // max_depth=2 只遍历到 account 30 (深度2)，account 10 在深度4，被截断
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn plan_writer_works() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1,
                vec![(1000, 5, 200), (5000, 20, 500)],
                8,
                true,
                0, // Nex mode
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 2);
            assert_eq!(config.tiers[0].rate, 200);
            assert_eq!(config.tiers[1].sales_threshold, 5000);
            assert_eq!(config.max_depth, 8);
            assert!(config.allow_stacking);
            assert_eq!(config.threshold_mode, pallet::SalesThresholdMode::Nex);

            // Usdt mode via PlanWriter
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                2,
                vec![(50_000_000, 5, 300)],
                5,
                false,
                1, // Usdt mode
            ));
            let config2 = pallet::TeamPerformanceConfigs::<Test>::get(2).unwrap();
            assert_eq!(config2.threshold_mode, pallet::SalesThresholdMode::Usdt);

            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::clear_config(1));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_none());
        });
    }

    #[test]
    fn usdt_mode_uses_member_spent_usdt() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: NEX spent=100 (low), but USDT spent=5_000_000 (5 USDT, 10^6 precision)
            set_stats(1, 40, 3, 10, 100);
            set_spent_usdt(1, 40, 5_000_000);

            // Threshold in USDT: 3_000_000 = 3 USDT
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 3_000_000, min_team_size: 5, rate: 200 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Usdt,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // USDT mode: 5_000_000 >= 3_000_000 threshold, team_size 10 >= 5 → matched
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 200); // 10000 * 200 / 10000
            assert_eq!(remaining, 9800);
        });
    }

    #[test]
    fn usdt_mode_nex_spent_ignored() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: high NEX spent but low USDT spent
            set_stats(1, 40, 3, 10, 999_999_999);
            set_spent_usdt(1, 40, 1_000_000); // only 1 USDT

            // Threshold 2 USDT
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 2_000_000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Usdt,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // USDT 1_000_000 < 2_000_000 threshold → no match despite huge NEX spent
            assert!(outputs.is_empty());
        });
    }

    // ====================================================================
    // TM-M1: match_tier non-monotonic min_team_size
    // ====================================================================

    #[test]
    fn tm_m1_non_monotonic_team_size_matches_higher_tier() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40: high spent, moderate team_size
            // Fails tier0 (min_team_size=50) but matches tier1 (min_team_size=5)
            set_stats(1, 40, 3, 10, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 50, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 5, rate: 300 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // Before fix: would break at tier0 (team_size fail), never check tier1
            // After fix: skips tier0, matches tier1 (rate=300)
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300); // 10000 * 300 / 10000
            assert_eq!(remaining, 9700);
        });
    }

    // ====================================================================
    // TM-M2: PlanWriter validation
    // ====================================================================

    #[test]
    fn tm_m2_plan_writer_rejects_empty_tiers() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![], 5, false, 0,
            ).is_err());
        });
    }

    #[test]
    fn tm_m2_plan_writer_rejects_invalid_rate() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 10001)], 5, false, 0,
            ).is_err());
        });
    }

    #[test]
    fn tm_m2_plan_writer_rejects_invalid_depth() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            // depth=0
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 0, false, 0,
            ).is_err());
            // depth=31
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 31, false, 0,
            ).is_err());
        });
    }

    #[test]
    fn tm_m2_plan_writer_rejects_non_ascending_thresholds() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(5000, 0, 200), (1000, 0, 100)], 5, false, 0,
            ).is_err());
        });
    }

    // ====================================================================
    // H1-deep: cycle detection
    // ====================================================================

    #[test]
    fn h1_deep_cycle_prevents_duplicate_commission() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            // 构造循环推荐链: 50 → 40 → 30 → 40 (cycle)
            REFERRERS.with(|r| {
                let mut m = r.borrow_mut();
                m.insert((1, 50), 40);
                m.insert((1, 40), 30);
                m.insert((1, 30), 40); // cycle back to 40
            });
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 5, 20, 10000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // Without cycle detection: 40 and 30 would alternate, paying each multiple times
            // With cycle detection: 40 (depth 1) + 30 (depth 2) then cycle detected → break
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[1].beneficiary, 30);
            // 10000 * 500 / 10000 = 500 each
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(outputs[1].amount, 500);
            assert_eq!(remaining, 10000 - 500 - 500);
        });
    }

    #[test]
    fn h1_deep_self_referral_cycle_breaks_immediately() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            // 自引用: 50 → 40 → 40 (self-cycle)
            REFERRERS.with(|r| {
                let mut m = r.borrow_mut();
                m.insert((1, 50), 40);
                m.insert((1, 40), 40); // self-referral
            });
            set_stats(1, 40, 5, 20, 10000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // 40 paid once, then self-cycle detected → break
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 40);
        });
    }

    // ====================================================================
    // M1-deep: PlanWriter emits events
    // ====================================================================

    #[test]
    fn m1_deep_plan_writer_emits_events() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;

            // set_team_config should emit TeamPerformanceConfigUpdated
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 5, false, 0,
            ));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigUpdated {
                    entity_id: 1, tier_count: 1, max_depth: 5,
                    allow_stacking: false, threshold_mode: pallet::SalesThresholdMode::Nex,
                },
            ));

            // clear_config should emit TeamPerformanceConfigCleared
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::clear_config(1));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigCleared { entity_id: 1 },
            ));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_none());
        });
    }

    // ====================================================================
    // P0: Admin with COMMISSION_MANAGE permission
    // ====================================================================

    #[test]
    fn set_config_works_by_admin_with_commission_manage() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            set_entity_admin(1, 200, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(200), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_some());
        });
    }

    #[test]
    fn set_config_rejects_admin_without_commission_manage() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            // Admin has SHOP_MANAGE only, not COMMISSION_MANAGE
            set_entity_admin(1, 200, pallet_entity_common::AdminPermission::SHOP_MANAGE);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(200), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::NotEntityOwnerOrAdmin
            );
        });
    }

    // ====================================================================
    // P1: clear_team_performance_config
    // ====================================================================

    #[test]
    fn clear_config_works_by_owner() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_ok!(CommissionTeam::clear_team_performance_config(
                RuntimeOrigin::signed(100), 1,
            ));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_none());
        });
    }

    #[test]
    fn clear_config_rejects_when_not_found() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            assert_noop!(
                CommissionTeam::clear_team_performance_config(RuntimeOrigin::signed(100), 1),
                Error::<Test>::ConfigNotFound
            );
        });
    }

    // ====================================================================
    // P1: force_set / force_clear (Root)
    // ====================================================================

    #[test]
    fn force_set_config_works() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_some());
        });
    }

    #[test]
    fn force_set_config_rejects_non_root() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::force_set_team_performance_config(
                    RuntimeOrigin::signed(1), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn force_clear_config_works() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            // force_clear even without checking existence
            assert_ok!(CommissionTeam::force_clear_team_performance_config(RuntimeOrigin::root(), 1));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(1).is_none());
        });
    }

    #[test]
    fn force_clear_config_rejects_non_root() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                CommissionTeam::force_clear_team_performance_config(RuntimeOrigin::signed(1), 1),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    // ====================================================================
    // P2: update_team_performance_params
    // ====================================================================

    #[test]
    fn update_params_works() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            // Update max_depth only
            assert_ok!(CommissionTeam::update_team_performance_params(
                RuntimeOrigin::signed(100), 1, Some(15), None, None,
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.max_depth, 15);
            assert!(!config.allow_stacking);
            assert_eq!(config.threshold_mode, pallet::SalesThresholdMode::Nex);

            // Update allow_stacking and threshold_mode
            assert_ok!(CommissionTeam::update_team_performance_params(
                RuntimeOrigin::signed(100), 1, None, Some(true), Some(pallet::SalesThresholdMode::Usdt),
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.max_depth, 15);
            assert!(config.allow_stacking);
            assert_eq!(config.threshold_mode, pallet::SalesThresholdMode::Usdt);
        });
    }

    #[test]
    fn update_params_rejects_all_none() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            assert_noop!(
                CommissionTeam::update_team_performance_params(
                    RuntimeOrigin::signed(100), 1, None, None, None,
                ),
                Error::<Test>::NothingToUpdate
            );
        });
    }

    #[test]
    fn update_params_rejects_config_not_found() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            assert_noop!(
                CommissionTeam::update_team_performance_params(
                    RuntimeOrigin::signed(100), 1, Some(10), None, None,
                ),
                Error::<Test>::ConfigNotFound
            );
        });
    }

    #[test]
    fn update_params_rejects_invalid_depth() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_noop!(
                CommissionTeam::update_team_performance_params(
                    RuntimeOrigin::signed(100), 1, Some(0), None, None,
                ),
                Error::<Test>::InvalidMaxDepth
            );
            assert_noop!(
                CommissionTeam::update_team_performance_params(
                    RuntimeOrigin::signed(100), 1, Some(31), None, None,
                ),
                Error::<Test>::InvalidMaxDepth
            );
        });
    }

    // ====================================================================
    // P1: banned member skipped in commission calculation
    // ====================================================================

    #[test]
    fn banned_ancestor_skipped_in_non_stacking() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            // account 40 is banned, account 30 is eligible
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);
            set_banned(1, 40);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // non-stacking: account 40 banned → skipped, account 30 is first eligible → paid
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 30);
            assert_eq!(outputs[0].amount, 500); // 10000 * 500 / 10000
            assert_eq!(remaining, 9500);
        });
    }

    #[test]
    fn banned_ancestor_skipped_in_stacking() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);
            set_stats(1, 20, 3, 10, 5000);
            set_banned(1, 30); // ban account 30

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // stacking: 40 paid, 30 banned→skipped, 20 paid
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[1].beneficiary, 20);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(outputs[1].amount, 500);
            assert_eq!(remaining, 10000 - 500 - 500);
        });
    }

    // ====================================================================
    // 审计 Round 2: M1 — force_clear 无幻影事件
    // ====================================================================

    #[test]
    fn m1_force_clear_no_phantom_event_when_config_absent() {
        new_test_ext().execute_with(|| {
            // entity 99 has no config
            assert_ok!(CommissionTeam::force_clear_team_performance_config(RuntimeOrigin::root(), 99));
            // Should NOT emit TeamPerformanceConfigCleared since nothing was cleared
            assert_eq!(
                System::events()
                    .iter()
                    .filter(|e| matches!(
                        e.event,
                        RuntimeEvent::CommissionTeam(pallet::Event::TeamPerformanceConfigCleared { .. })
                    ))
                    .count(),
                0
            );
        });
    }

    #[test]
    fn m1_force_clear_emits_event_when_config_exists() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_ok!(CommissionTeam::force_clear_team_performance_config(RuntimeOrigin::root(), 1));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigCleared { entity_id: 1 },
            ));
        });
    }

    #[test]
    fn m1_plan_writer_clear_no_phantom_event() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            // Clear non-existent config via PlanWriter
            assert_ok!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::clear_config(999));
            assert_eq!(
                System::events()
                    .iter()
                    .filter(|e| matches!(
                        e.event,
                        RuntimeEvent::CommissionTeam(pallet::Event::TeamPerformanceConfigCleared { .. })
                    ))
                    .count(),
                0
            );
        });
    }

    // ====================================================================
    // 审计 Round 2: L2 — PlanWriter rejects invalid threshold_mode
    // ====================================================================

    #[test]
    fn l2_plan_writer_rejects_invalid_threshold_mode() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::TeamPlanWriter;
            // threshold_mode = 2 should be rejected
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 5, false, 2,
            ).is_err());
            // threshold_mode = 255 should be rejected
            assert!(<pallet::Pallet<Test> as TeamPlanWriter<Balance>>::set_team_config(
                1, vec![(1000, 5, 200)], 5, false, 255,
            ).is_err());
        });
    }

    // ====================================================================
    // 审计 Round 2: L4 — Extrinsic event emission verification
    // ====================================================================

    #[test]
    fn set_config_emits_updated_event() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigUpdated {
                    entity_id: 1, tier_count: 1, max_depth: 5,
                    allow_stacking: false, threshold_mode: pallet::SalesThresholdMode::Nex,
                },
            ));
        });
    }

    #[test]
    fn clear_config_emits_cleared_event() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_ok!(CommissionTeam::clear_team_performance_config(RuntimeOrigin::signed(100), 1));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigCleared { entity_id: 1 },
            ));
        });
    }

    #[test]
    fn update_params_emits_updated_event() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_ok!(CommissionTeam::update_team_performance_params(
                RuntimeOrigin::signed(100), 1, Some(20), None, None,
            ));
            // Should have TWO Updated events (set + update)
            assert_eq!(
                System::events()
                    .iter()
                    .filter(|e| matches!(
                        &e.event,
                        RuntimeEvent::CommissionTeam(pallet::Event::TeamPerformanceConfigUpdated { entity_id, .. }) if *entity_id == 1
                    ))
                    .count(),
                2
            );
        });
    }

    #[test]
    fn force_set_emits_updated_event() {
        new_test_ext().execute_with(|| {
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigUpdated {
                    entity_id: 1, tier_count: 1, max_depth: 5,
                    allow_stacking: false, threshold_mode: pallet::SalesThresholdMode::Nex,
                },
            ));
        });
    }

    // ====================================================================
    // 审计 Round 2: duplicate threshold values
    // ====================================================================

    #[test]
    fn set_config_rejects_equal_thresholds() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 200 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::TiersNotAscending
            );
        });
    }

    // ====================================================================
    // EntityLocked 回归测试
    // ====================================================================

    #[test]
    fn entity_locked_rejects_set_config() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            set_entity_locked(1);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::EntityLocked
            );
        });
    }

    #[test]
    fn entity_locked_rejects_clear_config() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            set_entity_locked(1);
            assert_noop!(
                CommissionTeam::clear_team_performance_config(RuntimeOrigin::signed(100), 1),
                Error::<Test>::EntityLocked
            );
        });
    }

    #[test]
    fn entity_locked_rejects_update_params() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            set_entity_locked(1);
            assert_noop!(
                CommissionTeam::update_team_performance_params(
                    RuntimeOrigin::signed(100), 1, Some(15), None, None,
                ),
                Error::<Test>::EntityLocked
            );
        });
    }

    // ====================================================================
    // F1: Entity Active 状态守卫
    // ====================================================================

    #[test]
    fn f1_inactive_entity_rejects_set_config() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            set_entity_inactive(1);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_noop!(
                CommissionTeam::set_team_performance_config(
                    RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
                ),
                Error::<Test>::EntityNotActive
            );
        });
    }

    #[test]
    fn f1_inactive_entity_rejects_clear_config() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            set_entity_inactive(1);
            assert_noop!(
                CommissionTeam::clear_team_performance_config(RuntimeOrigin::signed(100), 1),
                Error::<Test>::EntityNotActive
            );
        });
    }

    #[test]
    fn f1_inactive_entity_rejects_update_params() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            set_entity_inactive(1);
            assert_noop!(
                CommissionTeam::update_team_performance_params(
                    RuntimeOrigin::signed(100), 1, Some(15), None, None,
                ),
                Error::<Test>::EntityNotActive
            );
        });
    }

    // ====================================================================
    // F2: 暂停/恢复机制
    // ====================================================================

    #[test]
    fn f2_pause_and_resume_works() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            // 默认启用
            assert!(pallet::TeamPerformanceEnabled::<Test>::get(1));

            // 暂停
            assert_ok!(CommissionTeam::pause_team_performance(RuntimeOrigin::signed(100), 1));
            assert!(!pallet::TeamPerformanceEnabled::<Test>::get(1));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformancePaused { entity_id: 1 },
            ));

            // 重复暂停失败
            assert_noop!(
                CommissionTeam::pause_team_performance(RuntimeOrigin::signed(100), 1),
                Error::<Test>::TeamPerformanceIsPaused
            );

            // 恢复
            assert_ok!(CommissionTeam::resume_team_performance(RuntimeOrigin::signed(100), 1));
            assert!(pallet::TeamPerformanceEnabled::<Test>::get(1));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceResumed { entity_id: 1 },
            ));

            // 重复恢复失败
            assert_noop!(
                CommissionTeam::resume_team_performance(RuntimeOrigin::signed(100), 1),
                Error::<Test>::TeamPerformanceNotPaused
            );
        });
    }

    #[test]
    fn f2_pause_skips_commission_calculation() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            set_entity_owner(1, 100);
            setup_chain(1);
            set_stats(1, 40, 3, 10, 5000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            // 正常计算有佣金
            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _rem) = <CommissionTeam as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 1,
            );
            assert!(!outputs.is_empty());

            // 暂停后计算无佣金
            assert_ok!(CommissionTeam::pause_team_performance(RuntimeOrigin::signed(100), 1));
            let (outputs2, rem2) = <CommissionTeam as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 1,
            );
            assert!(outputs2.is_empty());
            assert_eq!(rem2, 10000);

            // 恢复后计算有佣金
            assert_ok!(CommissionTeam::resume_team_performance(RuntimeOrigin::signed(100), 1));
            let (outputs3, _rem3) = <CommissionTeam as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 1,
            );
            assert!(!outputs3.is_empty());
        });
    }

    #[test]
    fn f2_clear_config_removes_enabled_state() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert!(pallet::TeamPerformanceEnabled::<Test>::get(1));

            assert_ok!(CommissionTeam::clear_team_performance_config(RuntimeOrigin::signed(100), 1));
            // ValueQuery 默认值为 false
            assert!(!pallet::TeamPerformanceEnabled::<Test>::get(1));
        });
    }

    // ====================================================================
    // F3: 单个档位 CRUD
    // ====================================================================

    #[test]
    fn f3_add_tier_works() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 20, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            // 插入中间档位
            let new_tier = pallet::TeamPerformanceTier { sales_threshold: 3000, min_team_size: 10, rate: 200 };
            assert_ok!(CommissionTeam::add_tier(RuntimeOrigin::signed(100), 1, new_tier));

            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 3);
            assert_eq!(config.tiers[0].sales_threshold, 1000);
            assert_eq!(config.tiers[1].sales_threshold, 3000);
            assert_eq!(config.tiers[2].sales_threshold, 5000);

            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamTierAdded { entity_id: 1, tier_index: 1 },
            ));
        });
    }

    #[test]
    fn f3_add_tier_rejects_duplicate_threshold() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            let dup_tier = pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 10, rate: 200 };
            assert_noop!(
                CommissionTeam::add_tier(RuntimeOrigin::signed(100), 1, dup_tier),
                Error::<Test>::TiersNotAscending
            );
        });
    }

    #[test]
    fn f3_add_tier_appends_at_end() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            let new_tier = pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 10, rate: 300 };
            assert_ok!(CommissionTeam::add_tier(RuntimeOrigin::signed(100), 1, new_tier));

            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 2);
            assert_eq!(config.tiers[1].sales_threshold, 5000);
        });
    }

    #[test]
    fn f3_update_tier_works() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 20, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            // 更新费率
            assert_ok!(CommissionTeam::update_tier(
                RuntimeOrigin::signed(100), 1, 0, None, None, Some(200),
            ));
            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers[0].rate, 200);

            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamTierUpdated { entity_id: 1, tier_index: 0 },
            ));
        });
    }

    #[test]
    fn f3_update_tier_rejects_out_of_bounds() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_noop!(
                CommissionTeam::update_tier(RuntimeOrigin::signed(100), 1, 5, None, None, Some(200)),
                Error::<Test>::TierIndexOutOfBounds
            );
        });
    }

    #[test]
    fn f3_update_tier_rejects_ascending_violation() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 0, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            // 将第二个档位的 threshold 改为低于第一个
            assert_noop!(
                CommissionTeam::update_tier(RuntimeOrigin::signed(100), 1, 1, Some(500), None, None),
                Error::<Test>::TiersNotAscending
            );
        });
    }

    #[test]
    fn f3_remove_tier_works() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 0, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_ok!(CommissionTeam::remove_tier(RuntimeOrigin::signed(100), 1, 0));

            let config = pallet::TeamPerformanceConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.tiers.len(), 1);
            assert_eq!(config.tiers[0].sales_threshold, 5000);

            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamTierRemoved { entity_id: 1, tier_index: 0 },
            ));
        });
    }

    #[test]
    fn f3_remove_last_tier_rejected() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert_noop!(
                CommissionTeam::remove_tier(RuntimeOrigin::signed(100), 1, 0),
                Error::<Test>::EmptyTiers
            );
        });
    }

    // ====================================================================
    // F4: 阶梯匹配查询
    // ====================================================================

    #[test]
    fn f4_get_matched_tier_for_account_works() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            set_entity_owner(1, 100);
            set_stats(1, 40, 3, 10, 3000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 15, rate: 300 },
                pallet::TeamPerformanceTier { sales_threshold: 10000, min_team_size: 30, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            // 账户 40: total_spent=3000, team_size=10 → 匹配 tier 0 (threshold=1000, team>=5)
            let result = CommissionTeam::get_matched_tier_for_account(1, &40);
            assert!(result.is_some());
            let (tier_idx, rate, next_threshold, next_team_size) = result.unwrap();
            assert_eq!(tier_idx, 0);
            assert_eq!(rate, 100);
            assert_eq!(next_threshold, Some(5000));
            assert_eq!(next_team_size, Some(15));
        });
    }

    #[test]
    fn f4_get_matched_tier_highest_returns_none_for_next() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            set_entity_owner(1, 100);
            set_stats(1, 40, 3, 50, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 10, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            let result = CommissionTeam::get_matched_tier_for_account(1, &40);
            let (tier_idx, rate, next_threshold, next_team_size) = result.unwrap();
            assert_eq!(tier_idx, 1);
            assert_eq!(rate, 300);
            assert_eq!(next_threshold, None);
            assert_eq!(next_team_size, None);
        });
    }

    #[test]
    fn f4_get_team_performance_status_works() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);

            // 无配置
            let (exists, enabled) = CommissionTeam::get_team_performance_status(1);
            assert!(!exists);
            assert!(!enabled);

            // 设置配置
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            let (exists, enabled) = CommissionTeam::get_team_performance_status(1);
            assert!(exists);
            assert!(enabled);

            // 暂停后
            assert_ok!(CommissionTeam::pause_team_performance(RuntimeOrigin::signed(100), 1));
            let (exists, enabled) = CommissionTeam::get_team_performance_status(1);
            assert!(exists);
            assert!(!enabled);
        });
    }

    // ====================================================================
    // F7: 事件信息增强
    // ====================================================================

    #[test]
    fn f7_set_config_emits_enhanced_event() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 10, rate: 300 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, true, pallet::SalesThresholdMode::Usdt,
            ));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamPerformanceConfigUpdated {
                    entity_id: 1,
                    tier_count: 2,
                    max_depth: 5,
                    allow_stacking: true,
                    threshold_mode: pallet::SalesThresholdMode::Usdt,
                },
            ));
        });
    }

    #[test]
    fn f7_commission_awarded_event_emitted() {
        new_test_ext().execute_with(|| {
            clear_thread_locals();
            set_entity_owner(1, 100);
            setup_chain(1);
            set_stats(1, 40, 3, 10, 5000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 500 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _) = <CommissionTeam as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 1,
            );
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 500); // 10000 * 500 / 10000

            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TeamCommissionAwarded {
                    entity_id: 1,
                    beneficiary: 40,
                    tier_index: 0,
                    rate: 500,
                    amount: 500,
                    depth: 1,
                },
            ));
        });
    }

    // ====================================================================
    // F1/F3: inactive entity rejects tier CRUD
    // ====================================================================

    #[test]
    fn f1_inactive_entity_rejects_add_tier() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            set_entity_inactive(1);
            let new_tier = pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 10, rate: 300 };
            assert_noop!(
                CommissionTeam::add_tier(RuntimeOrigin::signed(100), 1, new_tier),
                Error::<Test>::EntityNotActive
            );
        });
    }

    #[test]
    fn f1_inactive_entity_rejects_pause() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            set_entity_inactive(1);
            assert_noop!(
                CommissionTeam::pause_team_performance(RuntimeOrigin::signed(100), 1),
                Error::<Test>::EntityNotActive
            );
        });
    }

    // ====================================================================
    // M1 审计修复: is_member 检查 — 非会员推荐人应跳过
    // ====================================================================

    #[test]
    fn m1_non_member_referrer_skipped_nex_path() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            // account 40 is non-member (removed but referral chain not cleaned)
            set_non_member(1, 40);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // non-stacking: account 40 non-member → skipped, account 30 is first eligible
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 30);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(remaining, 9500);
        });
    }

    #[test]
    fn m1_non_member_referrer_skipped_stacking() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);
            set_stats(1, 20, 3, 10, 5000);
            set_non_member(1, 30); // account 30 is non-member

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // stacking: 40 paid, 30 non-member→skipped, 20 paid
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[1].beneficiary, 20);
            assert_eq!(remaining, 10000 - 500 - 500);
        });
    }

    #[test]
    fn m1_non_member_referrer_skipped_token_path() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_non_member(1, 40);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::TokenCommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 0,
            );

            // Token path: account 40 non-member → skipped, account 30 first eligible
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 30);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(remaining, 9500);
        });
    }

    // ====================================================================
    // M3 审计修复: Token 路径使用 match_tier_with_index 一致性验证
    // ====================================================================

    #[test]
    fn m3_token_path_matches_nex_path_tier_selection() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            // account 40: matches tier1 (non-monotonic min_team_size scenario)
            set_stats(1, 40, 3, 10, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 50, rate: 100 },
                pallet::TeamPerformanceTier { sales_threshold: 5000, min_team_size: 5, rate: 300 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            // NEX path
            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (nex_outputs, nex_rem) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // Token path
            use pallet_commission_common::TokenCommissionPlugin;
            let (token_outputs, token_rem) = <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 0,
            );

            // Both paths should produce identical results
            assert_eq!(nex_outputs.len(), token_outputs.len());
            assert_eq!(nex_outputs[0].beneficiary, token_outputs[0].beneficiary);
            assert_eq!(nex_outputs[0].amount, token_outputs[0].amount);
            assert_eq!(nex_rem, token_rem);
            // Should match tier1 (rate=300), not tier0
            assert_eq!(nex_outputs[0].amount, 300); // 10000 * 300 / 10000
        });
    }

    // ====================================================================
    // M1-R5: is_member_active 检查 — 冻结/暂停会员应跳过
    // ====================================================================

    #[test]
    fn m1r5_frozen_ancestor_skipped_nex_path() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);
            set_member_frozen(1, 40);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // account 40 frozen → skipped, account 30 first eligible
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 30);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(remaining, 9500);
        });
    }

    #[test]
    fn m1r5_frozen_ancestor_skipped_stacking() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);
            set_stats(1, 20, 3, 10, 5000);
            set_member_frozen(1, 30);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::CommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 0,
            );

            // stacking: 40 paid, 30 frozen→skipped, 20 paid
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[1].beneficiary, 20);
            assert_eq!(remaining, 10000 - 500 - 500);
        });
    }

    #[test]
    fn m1r5_frozen_ancestor_skipped_token_path() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_stats(1, 40, 5, 20, 10000);
            set_stats(1, 30, 8, 50, 20000);
            set_member_frozen(1, 40);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::TokenCommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, remaining) = <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 0,
            );

            // Token path: account 40 frozen → skipped, account 30 first eligible
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].beneficiary, 30);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(remaining, 9500);
        });
    }

    // ====================================================================
    // 审计 R6: M1-R6 — Token 路径 TokenTeamTierMatched 事件
    // ====================================================================

    #[test]
    fn m1r6_token_path_emits_tier_matched_event() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_stats(1, 40, 3, 10, 5000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::TokenCommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _) = <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 0,
            );
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 500);

            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TokenTeamTierMatched {
                    entity_id: 1,
                    beneficiary: 40,
                    tier_index: 0,
                    rate: 500,
                    depth: 1,
                },
            ));
        });
    }

    #[test]
    fn m1r6_token_stacking_emits_multiple_tier_matched_events() {
        new_test_ext().execute_with(|| {
            setup_chain(1);
            set_stats(1, 40, 3, 10, 5000);
            set_stats(1, 30, 8, 50, 20000);

            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 5, rate: 200 },
                pallet::TeamPerformanceTier { sales_threshold: 10000, min_team_size: 20, rate: 500 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Nex,
            ));

            use pallet_commission_common::TokenCommissionPlugin;
            let modes = CommissionModes(CommissionModes::TEAM_PERFORMANCE);
            let (outputs, _) = <pallet::Pallet<Test> as TokenCommissionPlugin<u64, Balance>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 0,
            );
            assert_eq!(outputs.len(), 2);

            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TokenTeamTierMatched {
                    entity_id: 1, beneficiary: 40, tier_index: 0, rate: 200, depth: 1,
                },
            ));
            System::assert_has_event(RuntimeEvent::CommissionTeam(
                pallet::Event::TokenTeamTierMatched {
                    entity_id: 1, beneficiary: 30, tier_index: 1, rate: 500, depth: 2,
                },
            ));
        });
    }

    // ====================================================================
    // 审计 R6: L1-R6 — set_config 覆盖暂停状态回归测试
    // ====================================================================

    #[test]
    fn l1r6_set_config_re_enables_paused_state() {
        new_test_ext().execute_with(|| {
            set_entity_owner(1, 100);
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.clone().try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));

            // 暂停
            assert_ok!(CommissionTeam::pause_team_performance(RuntimeOrigin::signed(100), 1));
            assert!(!pallet::TeamPerformanceEnabled::<Test>::get(1));

            // 重新设置配置 → 自动重新启用（设计行为，非 bug）
            assert_ok!(CommissionTeam::set_team_performance_config(
                RuntimeOrigin::signed(100), 1, tiers.try_into().unwrap(), 10, true, pallet::SalesThresholdMode::Usdt,
            ));
            assert!(pallet::TeamPerformanceEnabled::<Test>::get(1));
        });
    }

    // ====================================================================
    // 审计 R6: L2-R6 — force_set 不检查实体存在
    // ====================================================================

    #[test]
    fn l2r6_force_set_works_on_nonexistent_entity() {
        new_test_ext().execute_with(|| {
            // entity 999 不存在，但 force_set (Root) 可以设置配置
            let tiers = vec![
                pallet::TeamPerformanceTier { sales_threshold: 1000, min_team_size: 0, rate: 100 },
            ];
            assert_ok!(CommissionTeam::force_set_team_performance_config(
                RuntimeOrigin::root(), 999, tiers.try_into().unwrap(), 5, false, pallet::SalesThresholdMode::Nex,
            ));
            assert!(pallet::TeamPerformanceConfigs::<Test>::get(999).is_some());
            assert!(pallet::TeamPerformanceEnabled::<Test>::get(999));
        });
    }

    // ====================================================================
    // 审计 R6: L3-R6 — integrity_test（MaxTeamTiers <= 255）
    // ====================================================================

    #[test]
    fn l3r6_integrity_test_passes() {
        use frame_support::traits::Get;
        let max_tiers: u32 = <Test as pallet::Config>::MaxTeamTiers::get();
        assert!(max_tiers > 0, "MaxTeamTiers must be > 0");
        assert!(max_tiers <= 255, "MaxTeamTiers must be <= 255");
    }
}
