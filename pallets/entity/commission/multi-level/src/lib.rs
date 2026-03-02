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
        /// 推荐链 + 统计 + USDT 消费数据
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 最大层级数（默认 15）
        #[pallet::constant]
        type MaxMultiLevels: Get<u32>;

        /// 权重
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
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
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置 Entity 多级分销配置
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
            ensure_root(origin)?;
            ensure!(!levels.is_empty(), Error::<T>::EmptyLevels);
            for tier in levels.iter() {
                ensure!(tier.rate <= 10000, Error::<T>::InvalidRate);
            }
            ensure!(max_total_rate > 0 && max_total_rate <= 10000, Error::<T>::InvalidRate);

            MultiLevelConfigs::<T>::insert(entity_id, MultiLevelConfig { levels, max_total_rate });

            Self::deposit_event(Event::MultiLevelConfigUpdated { entity_id });
            Ok(())
        }
    }

    // ========================================================================
    // 核心算法 — process_multi_level（泛型，NEX/Token 共用）
    // ========================================================================

    impl<T: Config> Pallet<T> {
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
