//! # pallet-commission-multi-level
//!
//! 多级分销返佣插件 — N 层推荐链 + 三维激活条件 + 总佣金上限。
//!
//! 作为 `pallet-commission-core` 的 `CommissionPlugin` 插件运行，支持 NEX / EntityToken 双轨佣金。
//!
//! ## 功能
//!
//! - N 层推荐链遍历（`MaxMultiLevels` 上限）
//! - 每层独立激活条件（直推人数 / 团队规模 / USDT 累计消费，AND 逻辑）
//! - 总佣金上限（`max_total_rate` 基点制截断）
//! - 循环检测（`BTreeSet<AccountId>` 防止环形推荐链）
//! - 泛型佣金计算，NEX 和 EntityToken 共用 `process_multi_level`

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

use pallet_entity_common::EntityProvider as _;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::collections::BTreeSet;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
    };
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        CommissionOutput, CommissionType, MemberProvider,
    };
    use pallet_entity_common::{AdminPermission, EntityProvider};

    // ========================================================================
    // 数据结构
    // ========================================================================

    /// 多级分销层级配置
    ///
    /// 每层包含佣金比率和三维激活条件（AND 逻辑，值为 0 的条件自动跳过）。
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub struct MultiLevelTier {
        /// 佣金比率（基点制，10000 = 100%），0 = 跳过（占位层）
        pub rate: u16,
        /// 最低直推人数，0 = 无要求
        pub required_directs: u32,
        /// 最低团队规模，0 = 无要求
        pub required_team_size: u32,
        /// 最低累计消费 USDT（精度 10^6），0 = 无要求
        pub required_spent: u128,
    }

    /// 多级分销配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct MultiLevelConfig<MaxLevels: Get<u32>> {
        /// 各层配置，索引 0 = L1
        pub levels: BoundedVec<MultiLevelTier, MaxLevels>,
        /// 佣金总和上限（基点制，默认 1500 = 15%）
        pub max_total_rate: u16,
    }

    impl<MaxLevels: Get<u32>> Default for MultiLevelConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                levels: BoundedVec::default(),
                max_total_rate: 1500,
            }
        }
    }

    pub type MultiLevelConfigOf<T> = MultiLevelConfig<<T as Config>::MaxMultiLevels>;

    // ========================================================================
    // Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 推荐链 + 统计 + USDT 消费数据
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// 最大层级数（默认 15）
        #[pallet::constant]
        type MaxMultiLevels: Get<u32>;

        /// 权重
        type WeightInfo: WeightInfo;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ========================================================================
    // Storage
    // ========================================================================

    /// Entity → 多级分销配置
    #[pallet::storage]
    #[pallet::getter(fn multi_level_config)]
    pub type MultiLevelConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        MultiLevelConfigOf<T>,
    >;

    // ========================================================================
    // Events
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        MultiLevelConfigUpdated { entity_id: u64 },
        MultiLevelConfigCleared { entity_id: u64 },
        /// 单层配置已更新
        TierUpdated { entity_id: u64, tier_index: u32 },
        /// max_total_rate 已更新
        MaxTotalRateUpdated { entity_id: u64, new_rate: u16 },
        /// 层级已插入
        TierInserted { entity_id: u64, tier_index: u32 },
        /// 层级已移除
        TierRemoved { entity_id: u64, tier_index: u32 },
    }

    // ========================================================================
    // Errors
    // ========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// rate 超过 10000
        InvalidRate,
        /// levels 为空
        EmptyLevels,
        /// 实体不存在
        EntityNotFound,
        /// 非实体所有者或无 COMMISSION_MANAGE 权限
        NotEntityOwnerOrAdmin,
        /// 配置不存在（清除时）
        ConfigNotFound,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// 部分更新时所有字段均为 None
        NothingToUpdate,
        /// tier_index 超出 levels 范围
        TierIndexOutOfBounds,
        /// 层级数已达 MaxMultiLevels 上限
        TierLimitExceeded,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置 Entity 多级分销配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        ///
        /// 校验：levels 非空，每层 `rate ≤ 10000`，`0 < max_total_rate ≤ 10000`。
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_multi_level_config(levels.len() as u32))]
        pub fn set_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
            levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            Self::validate_config(&levels, max_total_rate)?;

            MultiLevelConfigs::<T>::insert(entity_id, MultiLevelConfig { levels, max_total_rate });

            Self::deposit_event(Event::MultiLevelConfigUpdated { entity_id });
            Ok(())
        }

        /// 清除 Entity 多级分销配置（Entity Owner 或持有 COMMISSION_MANAGE 权限的 Admin）
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_multi_level_config())]
        pub fn clear_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(MultiLevelConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            MultiLevelConfigs::<T>::remove(entity_id);
            Self::deposit_event(Event::MultiLevelConfigCleared { entity_id });
            Ok(())
        }

        /// [Root] 强制设置 Entity 多级分销配置
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::set_multi_level_config(levels.len() as u32))]
        pub fn force_set_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
            levels: BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::validate_config(&levels, max_total_rate)?;

            MultiLevelConfigs::<T>::insert(entity_id, MultiLevelConfig { levels, max_total_rate });

            Self::deposit_event(Event::MultiLevelConfigUpdated { entity_id });
            Ok(())
        }

        /// [Root] 强制清除 Entity 多级分销配置（幂等，配置不存在时静默成功）
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::clear_multi_level_config())]
        pub fn force_clear_multi_level_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            if MultiLevelConfigs::<T>::contains_key(entity_id) {
                MultiLevelConfigs::<T>::remove(entity_id);
                Self::deposit_event(Event::MultiLevelConfigCleared { entity_id });
            }
            Ok(())
        }

        /// F3: 部分更新多级分销参数（Owner/Admin）
        ///
        /// 支持单独更新 `max_total_rate` 和/或指定层的配置，无需重提整个 levels 数组。
        /// 全部 None 返回 `NothingToUpdate`。
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::update_multi_level_params())]
        pub fn update_multi_level_params(
            origin: OriginFor<T>,
            entity_id: u64,
            max_total_rate: Option<u16>,
            tier_index: Option<u32>,
            tier_update: Option<MultiLevelTier>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(
                max_total_rate.is_some() || (tier_index.is_some() && tier_update.is_some()),
                Error::<T>::NothingToUpdate
            );

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;

                if let Some(new_rate) = max_total_rate {
                    ensure!(new_rate > 0 && new_rate <= 10000, Error::<T>::InvalidRate);
                    config.max_total_rate = new_rate;
                    Self::deposit_event(Event::MaxTotalRateUpdated { entity_id, new_rate });
                }

                if let (Some(idx), Some(tier)) = (tier_index, tier_update) {
                    let idx = idx as usize;
                    ensure!(idx < config.levels.len(), Error::<T>::TierIndexOutOfBounds);
                    ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
                    config.levels[idx] = tier;
                    Self::deposit_event(Event::TierUpdated { entity_id, tier_index: idx as u32 });
                }

                Ok(())
            })
        }

        /// F4: 在指定位置插入新层级（Owner/Admin）
        ///
        /// `index` 为插入位置（0-indexed），现有层级从 index 开始后移。
        /// index = levels.len() 表示追加到末尾。
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::add_tier())]
        pub fn add_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            index: u32,
            tier: MultiLevelTier,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = index as usize;
                ensure!(idx <= config.levels.len(), Error::<T>::TierIndexOutOfBounds);

                // 构建新 Vec 并插入
                let mut v = config.levels.to_vec();
                v.insert(idx, tier);
                config.levels = v.try_into().map_err(|_| Error::<T>::TierLimitExceeded)?;

                Self::deposit_event(Event::TierInserted { entity_id, tier_index: index });
                Ok(())
            })
        }

        /// F4: 移除指定位置的层级（Owner/Admin）
        ///
        /// 移除后 levels 不可为空（至少保留 1 层，否则应使用 clear）。
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::remove_tier())]
        pub fn remove_tier(
            origin: OriginFor<T>,
            entity_id: u64,
            index: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            MultiLevelConfigs::<T>::try_mutate(entity_id, |maybe_config| -> DispatchResult {
                let config = maybe_config.as_mut().ok_or(Error::<T>::ConfigNotFound)?;
                let idx = index as usize;
                ensure!(idx < config.levels.len(), Error::<T>::TierIndexOutOfBounds);
                ensure!(config.levels.len() > 1, Error::<T>::EmptyLevels);

                let mut v = config.levels.to_vec();
                v.remove(idx);
                config.levels = v.try_into().map_err(|_| Error::<T>::TierLimitExceeded)?;

                Self::deposit_event(Event::TierRemoved { entity_id, tier_index: index });
                Ok(())
            })
        }
    }

    // ========================================================================
    // 核心算法 — process_multi_level（泛型，NEX/Token 共用）
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

        /// 校验 levels + max_total_rate 参数合法性
        fn validate_config(
            levels: &BoundedVec<MultiLevelTier, T::MaxMultiLevels>,
            max_total_rate: u16,
        ) -> DispatchResult {
            ensure!(!levels.is_empty(), Error::<T>::EmptyLevels);
            for tier in levels.iter() {
                ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            }
            ensure!(max_total_rate > 0 && max_total_rate <= 10000, Error::<T>::InvalidRate);
            Ok(())
        }

        /// 多级分销佣金计算
        ///
        /// 逐层遍历推荐链，每层执行：
        /// 1. rate = 0 → 跳过（占位层），向上移动 referrer
        /// 2. 无推荐人 → 终止
        /// 3. 循环检测（BTreeSet）→ 命中则终止
        /// 4. 激活条件不满足 → 跳过该层，继续下一层
        /// 5. 计算佣金 = order_amount × rate / 10000，取 min(commission, remaining)
        /// 6. 总额上限检查 — 累计超过 max_total_rate 时截断最后一笔并终止
        pub fn process_multi_level<B>(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: B,
            remaining: &mut B,
            config: &MultiLevelConfigOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, B>>,
        ) where
            T::AccountId: Ord,
            B: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
        {
            if config.levels.is_empty() { return; }

            let mut visited = BTreeSet::new();
            visited.insert(buyer.clone());

            let mut current_referrer = T::MemberProvider::get_referrer(entity_id, buyer);
            let mut total_commission = B::zero();
            let max_commission = order_amount
                .saturating_mul(B::from(config.max_total_rate as u32))
                / B::from(10000u32);

            for (level_idx, tier) in config.levels.iter().enumerate() {
                if tier.rate == 0 {
                    if let Some(ref r) = current_referrer {
                        visited.insert(r.clone());
                    }
                    current_referrer = current_referrer.and_then(|r| T::MemberProvider::get_referrer(entity_id, &r));
                    continue;
                }

                let Some(ref referrer) = current_referrer else { break };

                if visited.contains(referrer) { break; }
                visited.insert(referrer.clone());

                // F10: 跳过非会员（已退出/未注册的推荐人不应获得佣金）
                // M1-R4: is_member/is_banned 廉价检查提前，避免 check_tier_activation 的多余 DB read
                if !T::MemberProvider::is_member(entity_id, referrer) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                // F9: 跳过被封禁的推荐人（与 referral X1 修复一致）
                if T::MemberProvider::is_banned(entity_id, referrer) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                if !Self::check_tier_activation(entity_id, referrer, tier) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                let commission = order_amount.saturating_mul(B::from(tier.rate as u32)) / B::from(10000u32);
                let actual = commission.min(*remaining);
                if actual.is_zero() { break; }

                let level = (level_idx + 1).min(255) as u8;

                let new_total = total_commission.saturating_add(actual);
                if new_total > max_commission {
                    let can_distribute = max_commission.saturating_sub(total_commission);
                    if !can_distribute.is_zero() {
                        *remaining = remaining.saturating_sub(can_distribute);
                        outputs.push(CommissionOutput {
                            beneficiary: referrer.clone(),
                            amount: can_distribute,
                            commission_type: CommissionType::MultiLevel,
                            level,
                        });
                    }
                    break;
                }

                *remaining = remaining.saturating_sub(actual);
                total_commission = total_commission.saturating_add(actual);
                outputs.push(CommissionOutput {
                    beneficiary: referrer.clone(),
                    amount: actual,
                    commission_type: CommissionType::MultiLevel,
                    level,
                });

                current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
            }
        }

        /// 三维激活条件检查（AND 逻辑，值为 0 自动跳过）
        ///
        /// | 条件 | 数据来源 | 精度 |
        /// |------|----------|------|
        /// | `required_directs` | `MemberProvider::get_member_stats().0` | 人数 |
        /// | `required_team_size` | `MemberProvider::get_member_stats().1` | 人数 |
        /// | `required_spent` | `MemberProvider::get_member_spent_usdt()` | USDT × 10^6 |
        pub fn check_tier_activation(
            entity_id: u64,
            account: &T::AccountId,
            tier: &MultiLevelTier,
        ) -> bool {
            if tier.required_directs == 0 && tier.required_team_size == 0 && tier.required_spent == 0 {
                return true;
            }
            // L1-R3: 仅在需要时读取 get_member_stats，避免 required_spent-only 场景的多余 DB read
            if tier.required_directs > 0 || tier.required_team_size > 0 {
                let (direct_referrals, team_size, _) = T::MemberProvider::get_member_stats(entity_id, account);
                if tier.required_directs > 0 && direct_referrals < tier.required_directs { return false; }
                if tier.required_team_size > 0 && team_size < tier.required_team_size { return false; }
            }
            if tier.required_spent > 0 {
                let spent_usdt: u128 = T::MemberProvider::get_member_spent_usdt(entity_id, account).into();
                if spent_usdt < tier.required_spent { return false; }
            }
            true
        }

        /// F11: 查询指定账户在各层级的激活状态
        ///
        /// 返回 `Vec<bool>`，长度 = levels.len()，true = 该层已激活。
        /// 配置不存在时返回空 Vec。
        pub fn get_activation_status(entity_id: u64, account: &T::AccountId) -> alloc::vec::Vec<bool> {
            let config = match MultiLevelConfigs::<T>::get(entity_id) {
                Some(c) => c,
                None => return alloc::vec::Vec::new(),
            };
            config.levels.iter().map(|tier| Self::check_tier_activation(entity_id, account, tier)).collect()
        }
    }
}

// ============================================================================
// CommissionPlugin — NEX（供 core 通过 type MultiLevelPlugin 调用）
// ============================================================================

impl<T: pallet::Config, B> pallet_commission_common::CommissionPlugin<T::AccountId, B> for pallet::Pallet<T>
where
    B: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
    T::AccountId: Ord,
{
    fn calculate(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: B,
        remaining: B,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, B>>, B) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::MULTI_LEVEL) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F12: Entity 未激活时跳过佣金计算
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::MultiLevelConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_multi_level(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// TokenCommissionPlugin — EntityToken（供 core 通过 type TokenMultiLevelPlugin 调用）
// ============================================================================

impl<T: pallet::Config, TB> pallet_commission_common::TokenCommissionPlugin<T::AccountId, TB>
    for pallet::Pallet<T>
where
    TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug,
    T::AccountId: Ord,
{
    fn calculate_token(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: TB,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::MULTI_LEVEL) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // F12: Entity 未激活时跳过佣金计算
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let config = match pallet::MultiLevelConfigs::<T>::get(entity_id) {
            Some(c) => c,
            None => return (alloc::vec::Vec::new(), remaining),
        };

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_multi_level(
            entity_id, buyer, order_amount, &mut remaining, &config, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// MultiLevelPlanWriter — 治理路径
//
// PlanWriter 校验 rate / max_total_rate / 层数上限。
// 限制：PlanWriter 创建的 tiers 激活条件全为 0，需通过 Root extrinsic 配置完整条件。
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::MultiLevelPlanWriter for pallet::Pallet<T> {
    fn set_multi_level(entity_id: u64, level_rates: alloc::vec::Vec<u16>, max_total_rate: u16) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(!level_rates.is_empty(), sp_runtime::DispatchError::Other("EmptyLevels"));
        frame_support::ensure!(max_total_rate > 0 && max_total_rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        for &rate in level_rates.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = level_rates
            .into_iter()
            .map(|rate| pallet::MultiLevelTier { rate, required_directs: 0, required_team_size: 0, required_spent: 0 })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        pallet::MultiLevelConfigs::<T>::insert(entity_id, pallet::MultiLevelConfig { levels: bounded, max_total_rate });
        // M1-R2 审计修复: PlanWriter 路径也需 emit 事件，供 off-chain indexer 感知
        Self::deposit_event(pallet::Event::MultiLevelConfigUpdated { entity_id });
        Ok(())
    }

    fn set_multi_level_full(
        entity_id: u64,
        tiers: alloc::vec::Vec<(u16, u32, u32, u128)>,
        max_total_rate: u16,
    ) -> Result<(), sp_runtime::DispatchError> {
        frame_support::ensure!(!tiers.is_empty(), sp_runtime::DispatchError::Other("EmptyLevels"));
        frame_support::ensure!(max_total_rate > 0 && max_total_rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        for &(rate, _, _, _) in tiers.iter() {
            frame_support::ensure!(rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        let bounded: frame_support::BoundedVec<pallet::MultiLevelTier, T::MaxMultiLevels> = tiers
            .into_iter()
            .map(|(rate, required_directs, required_team_size, required_spent)| {
                pallet::MultiLevelTier { rate, required_directs, required_team_size, required_spent }
            })
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        pallet::MultiLevelConfigs::<T>::insert(entity_id, pallet::MultiLevelConfig { levels: bounded, max_total_rate });
        Self::deposit_event(pallet::Event::MultiLevelConfigUpdated { entity_id });
        Ok(())
    }

    fn clear_multi_level_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        pallet::MultiLevelConfigs::<T>::remove(entity_id);
        // M2-R2 审计修复: emit 事件
        Self::deposit_event(pallet::Event::MultiLevelConfigCleared { entity_id });
        Ok(())
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
