//! # Commission Level-Diff Plugin (pallet-commission-level-diff)
//!
//! 等级极差返佣插件，支持自定义等级体系（Entity 自定义等级 + 返佣率）

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use pallet_commission_common::MemberProvider as _;
use pallet_entity_common::EntityProvider as _;

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
        traits::{Currency, Get},
    };
    use frame_system::pallet_prelude::*;
    use pallet_commission_common::{
        CommissionOutput, CommissionType, MemberProvider,
    };
    use pallet_entity_common::EntityProvider;
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ========================================================================
    // Config structs
    // ========================================================================

    /// 等级极差配置（统一使用自定义等级体系）
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct CustomLevelDiffConfig<MaxLevels: Get<u32>> {
        pub level_rates: BoundedVec<u16, MaxLevels>,
        pub max_depth: u8,
    }

    impl<MaxLevels: Get<u32>> Default for CustomLevelDiffConfig<MaxLevels> {
        fn default() -> Self {
            Self {
                level_rates: BoundedVec::default(),
                max_depth: 10,
            }
        }
    }

    pub type CustomLevelDiffConfigOf<T> = CustomLevelDiffConfig<<T as Config>::MaxCustomLevels>;

    // ========================================================================
    // Pallet Config
    // ========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;

        #[pallet::constant]
        type MaxCustomLevels: Get<u32>;

        /// 权重
        type WeightInfo: WeightInfo;

        /// Benchmark helper for setting up external state.
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self::AccountId>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// F7: 运行时常量校验
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        #[cfg(feature = "std")]
        fn integrity_test() {
            assert!(
                T::MaxCustomLevels::get() > 0,
                "MaxCustomLevels must be > 0"
            );
        }
    }

    // ========================================================================
    // Storage
    // ========================================================================

    /// 等级极差配置 entity_id -> CustomLevelDiffConfig
    #[pallet::storage]
    #[pallet::getter(fn custom_level_diff_config)]
    pub type CustomLevelDiffConfigs<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        CustomLevelDiffConfigOf<T>,
    >;

    // ========================================================================
    // Events / Errors
    // ========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        LevelDiffConfigUpdated { entity_id: u64, levels_count: u32 },
        LevelDiffConfigCleared { entity_id: u64 },
        /// F5: 佣金分配明细事件（每笔极差佣金的计算细节）
        LevelDiffCommissionDetail {
            entity_id: u64,
            beneficiary: T::AccountId,
            referrer_rate: u16,
            prev_rate: u16,
            diff_rate: u16,
            amount: BalanceOf<T>,
            level: u8,
        },
        /// M1-R7: Token 路径佣金分配明细事件（与 NEX 版对称）
        LevelDiffTokenCommissionDetail {
            entity_id: u64,
            beneficiary: T::AccountId,
            referrer_rate: u16,
            prev_rate: u16,
            diff_rate: u16,
            token_amount: u128,
            level: u8,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidRate,
        InvalidMaxDepth,
        EmptyLevelRates,
        EntityNotFound,
        NotEntityOwnerOrAdmin,
        EntityLocked,
        ConfigNotFound,
        /// F1: Entity 未激活
        EntityNotActive,
        /// F3: 等级率必须弱单调递增
        RatesNotMonotonic,
    }

    // ========================================================================
    // Extrinsics
    // ========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 设置等级极差配置（Entity Owner / Admin(COMMISSION_MANAGE)）
        ///
        /// level_rates: 每个自定义等级对应的极差比例（bps），索引 = custom_level_id
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_level_diff_config(level_rates.len() as u32))]
        pub fn set_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: BoundedVec<u16, T::MaxCustomLevels>,
            max_depth: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            // F1: Entity 活跃状态检查
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            Self::do_set_config(entity_id, level_rates, max_depth)
        }

        /// 清除等级极差配置（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::clear_level_diff_config())]
        pub fn clear_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            // F1: Entity 活跃状态检查
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(CustomLevelDiffConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            CustomLevelDiffConfigs::<T>::remove(entity_id);
            Self::deposit_event(Event::LevelDiffConfigCleared { entity_id });
            Ok(())
        }

        /// F2: 部分更新等级极差配置（Entity Owner / Admin(COMMISSION_MANAGE)）
        ///
        /// level_rates / max_depth 均为 Option，None 表示保留原值
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::update_level_diff_config(level_rates.as_ref().map(|r| r.len() as u32).unwrap_or(0)))]
        pub fn update_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: Option<BoundedVec<u16, T::MaxCustomLevels>>,
            max_depth: Option<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(T::EntityProvider::is_entity_active(entity_id), Error::<T>::EntityNotActive);
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let existing = CustomLevelDiffConfigs::<T>::get(entity_id)
                .ok_or(Error::<T>::ConfigNotFound)?;

            let new_rates = level_rates.unwrap_or(existing.level_rates);
            let new_depth = max_depth.unwrap_or(existing.max_depth);

            Self::do_set_config(entity_id, new_rates, new_depth)
        }

        /// [Root] 强制设置等级极差配置
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::force_set_level_diff_config(level_rates.len() as u32))]
        pub fn force_set_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: BoundedVec<u16, T::MaxCustomLevels>,
            max_depth: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // L1-R7: Root 也需确认实体存在，防止孤立存储
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            Self::do_set_config(entity_id, level_rates, max_depth)
        }

        /// [Root] 强制清除等级极差配置
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::force_clear_level_diff_config())]
        pub fn force_clear_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // L1-R7: Root 也需确认实体存在
            ensure!(T::EntityProvider::entity_exists(entity_id), Error::<T>::EntityNotFound);
            ensure!(CustomLevelDiffConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            CustomLevelDiffConfigs::<T>::remove(entity_id);
            Self::deposit_event(Event::LevelDiffConfigCleared { entity_id });
            Ok(())
        }
    }

    // ========================================================================
    // Internal helpers
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
                T::EntityProvider::is_entity_admin(entity_id, who, pallet_entity_common::AdminPermission::COMMISSION_MANAGE),
                Error::<T>::NotEntityOwnerOrAdmin
            );
            Ok(())
        }

        /// 共享的配置写入逻辑（extrinsic + force 共用）
        fn do_set_config(
            entity_id: u64,
            level_rates: BoundedVec<u16, T::MaxCustomLevels>,
            max_depth: u8,
        ) -> DispatchResult {
            ensure!(!level_rates.is_empty(), Error::<T>::EmptyLevelRates);
            for rate in level_rates.iter() {
                ensure!(*rate <= 10000, Error::<T>::InvalidRate);
            }
            // F3: 等级率弱单调递增校验
            for w in level_rates.windows(2) {
                ensure!(w[1] >= w[0], Error::<T>::RatesNotMonotonic);
            }
            ensure!(max_depth > 0 && max_depth <= 20, Error::<T>::InvalidMaxDepth);

            // F8: 事件包含 levels_count 供 indexer 比对
            let levels_count = level_rates.len() as u32;

            CustomLevelDiffConfigs::<T>::insert(entity_id, CustomLevelDiffConfig {
                level_rates,
                max_depth,
            });

            Self::deposit_event(Event::LevelDiffConfigUpdated { entity_id, levels_count });
            Ok(())
        }
    }

    // ========================================================================
    // Internal calculation
    // ========================================================================

    impl<T: Config> Pallet<T> {
        pub fn process_level_diff(
            entity_id: u64,
            buyer: &T::AccountId,
            order_amount: BalanceOf<T>,
            remaining: &mut BalanceOf<T>,
            outputs: &mut Vec<CommissionOutput<T::AccountId, BalanceOf<T>>>,
        ) where T::AccountId: Ord {
            let config = CustomLevelDiffConfigs::<T>::get(entity_id);
            let max_depth = config.as_ref().map(|c| c.max_depth).unwrap_or(10);

            let mut current_referrer = T::MemberProvider::get_referrer(entity_id, buyer);
            let mut prev_rate: u16 = 0;
            let mut level: u8 = 0;
            // H1 审计修复: 循环检测，防止推荐链有环时无限循环
            let mut visited = BTreeSet::new();

            while let Some(ref referrer) = current_referrer {
                if !visited.insert(referrer.clone()) { break; }
                level += 1;
                if level > max_depth { break; }
                // M2 审计修复: 额度耗尽后提前退出，避免无意义的 storage read
                if remaining.is_zero() { break; }

                // F4: 跳过非会员推荐人（会员被移除但推荐链未清理的边缘情况）
                if !T::MemberProvider::is_member(entity_id, referrer) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                // X8: 跳过被封禁或未激活推荐人
                if T::MemberProvider::is_banned(entity_id, referrer)
                    || !T::MemberProvider::is_activated(entity_id, referrer)
                {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                // M2-R7: 跳过冻结/暂停的推荐人（与 referral 插件一致）
                if !T::MemberProvider::is_member_active(entity_id, referrer) {
                    current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                    continue;
                }

                let level_id = T::MemberProvider::custom_level_id(entity_id, referrer);
                // P1/P2 修复: CustomLevelDiffConfig 优先；无配置或 level_id 越界时
                // 回退到 CustomLevel.commission_bonus（通过 MemberProvider）
                let referrer_rate = config.as_ref()
                    .and_then(|c| c.level_rates.get(level_id as usize).copied())
                    .unwrap_or_else(|| T::MemberProvider::get_level_commission_bonus(entity_id, level_id));

                if referrer_rate > prev_rate {
                    let diff_rate = referrer_rate - prev_rate;
                    let commission = order_amount
                        .saturating_mul(diff_rate.into())
                        / 10000u32.into();
                    let actual = commission.min(*remaining);

                    if !actual.is_zero() {
                        *remaining = remaining.saturating_sub(actual);
                        outputs.push(CommissionOutput {
                            beneficiary: referrer.clone(),
                            amount: actual,
                            commission_type: CommissionType::LevelDiff,
                            level,
                        });
                        // F5: 佣金分配明细事件
                        Self::deposit_event(Event::LevelDiffCommissionDetail {
                            entity_id,
                            beneficiary: referrer.clone(),
                            referrer_rate,
                            prev_rate,
                            diff_rate,
                            amount: actual,
                            level,
                        });
                    }

                    prev_rate = referrer_rate;
                }

                current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
            }
        }
    }
}

// ============================================================================
// CommissionPlugin implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::CommissionPlugin<T::AccountId, pallet::BalanceOf<T>> for pallet::Pallet<T> {
    fn calculate(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: pallet::BalanceOf<T>,
        remaining: pallet::BalanceOf<T>,
        enabled_modes: pallet_commission_common::CommissionModes,
        _is_first_order: bool,
        _buyer_order_count: u32,
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, pallet::BalanceOf<T>>>, pallet::BalanceOf<T>) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::LEVEL_DIFF) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // M1-R8: 与 referral/multi-level 插件一致，未激活实体不计算
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_level_diff(
            entity_id, buyer, order_amount, &mut remaining, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// Token 多资产 — TokenCommissionPlugin implementation
// ============================================================================

impl<T: pallet::Config> pallet::Pallet<T> {
    /// Token 版等级极差计算（泛型，rate-based）
    fn process_level_diff_token<TB>(
        entity_id: u64,
        buyer: &T::AccountId,
        order_amount: TB,
        remaining: &mut TB,
        outputs: &mut alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>,
    ) where
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Into<u128>,
        T::AccountId: Ord,
    {
        let config = pallet::CustomLevelDiffConfigs::<T>::get(entity_id);
        let max_depth = config.as_ref().map(|c| c.max_depth).unwrap_or(10);

        let mut current_referrer = T::MemberProvider::get_referrer(entity_id, buyer);
        let mut prev_rate: u16 = 0;
        let mut level: u8 = 0;
        // H1 审计修复: 循环检测
        let mut visited = alloc::collections::BTreeSet::new();

        while let Some(ref referrer) = current_referrer {
            if !visited.insert(referrer.clone()) { break; }
            level += 1;
            if level > max_depth { break; }
            if remaining.is_zero() { break; }

            // F4: 跳过非会员推荐人
            if !T::MemberProvider::is_member(entity_id, referrer) {
                current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                continue;
            }

            // X8: 跳过被封禁或未激活推荐人
            if T::MemberProvider::is_banned(entity_id, referrer)
                || !T::MemberProvider::is_activated(entity_id, referrer)
            {
                current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                continue;
            }

            // M2-R7: 跳过冻结/暂停的推荐人（与 referral 插件一致）
            if !T::MemberProvider::is_member_active(entity_id, referrer) {
                current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
                continue;
            }

            let level_id = T::MemberProvider::custom_level_id(entity_id, referrer);
            let referrer_rate = config.as_ref()
                .and_then(|c| c.level_rates.get(level_id as usize).copied())
                .unwrap_or_else(|| T::MemberProvider::get_level_commission_bonus(entity_id, level_id));

            if referrer_rate > prev_rate {
                let diff_rate = referrer_rate - prev_rate;
                let commission = order_amount
                    .saturating_mul(TB::from(diff_rate as u32))
                    / TB::from(10000u32);
                let actual = commission.min(*remaining);

                if !actual.is_zero() {
                    *remaining = remaining.saturating_sub(actual);
                    outputs.push(pallet_commission_common::CommissionOutput {
                        beneficiary: referrer.clone(),
                        amount: actual,
                        commission_type: pallet_commission_common::CommissionType::LevelDiff,
                        level,
                    });
                    // M1-R7: Token 路径佣金明细事件（与 NEX 版对称）
                    Self::deposit_event(pallet::Event::LevelDiffTokenCommissionDetail {
                        entity_id,
                        beneficiary: referrer.clone(),
                        referrer_rate,
                        prev_rate,
                        diff_rate,
                        token_amount: actual.into(),
                        level,
                    });
                }

                prev_rate = referrer_rate;
            }

            current_referrer = T::MemberProvider::get_referrer(entity_id, referrer);
        }
    }
}

impl<T: pallet::Config, TB> pallet_commission_common::TokenCommissionPlugin<T::AccountId, TB>
    for pallet::Pallet<T>
where
    TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy + Default + core::fmt::Debug + Into<u128>,
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

        if !enabled_modes.contains(CommissionModes::LEVEL_DIFF) {
            return (alloc::vec::Vec::new(), remaining);
        }

        // M1-R8: 与 referral/multi-level 插件一致，未激活实体不计算
        if !T::EntityProvider::is_entity_active(entity_id) {
            return (alloc::vec::Vec::new(), remaining);
        }

        let mut remaining = remaining;
        let mut outputs = alloc::vec::Vec::new();

        pallet::Pallet::<T>::process_level_diff_token::<TB>(
            entity_id, buyer, order_amount, &mut remaining, &mut outputs,
        );

        (outputs, remaining)
    }
}

// ============================================================================
// LevelDiffPlanWriter implementation
// ============================================================================

impl<T: pallet::Config> pallet_commission_common::LevelDiffPlanWriter for pallet::Pallet<T> {
    fn set_level_rates(entity_id: u64, level_rates: alloc::vec::Vec<u16>, max_depth: u8) -> Result<(), sp_runtime::DispatchError> {
        // F6: entity 存在性防御
        frame_support::ensure!(
            <T::EntityProvider as pallet_entity_common::EntityProvider<T::AccountId>>::entity_exists(entity_id),
            sp_runtime::DispatchError::Other("EntityNotFound")
        );
        // M2-R3 审计修复: trait 路径也校验空 level_rates（与 extrinsic 一致）
        frame_support::ensure!(!level_rates.is_empty(), sp_runtime::DispatchError::Other("EmptyLevelRates"));
        for rate in level_rates.iter() {
            frame_support::ensure!(*rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        // F3: 等级率弱单调递增校验（与 extrinsic 一致）
        for w in level_rates.windows(2) {
            frame_support::ensure!(w[1] >= w[0], sp_runtime::DispatchError::Other("RatesNotMonotonic"));
        }
        frame_support::ensure!(max_depth > 0 && max_depth <= 20, sp_runtime::DispatchError::Other("InvalidMaxDepth"));
        let bounded_rates: frame_support::BoundedVec<u16, T::MaxCustomLevels> =
            level_rates.try_into().map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        // F8: 事件包含 levels_count
        let levels_count = bounded_rates.len() as u32;
        pallet::CustomLevelDiffConfigs::<T>::insert(entity_id, pallet::CustomLevelDiffConfig {
            level_rates: bounded_rates,
            max_depth,
        });
        // M1 审计修复: trait 路径也发出事件
        pallet::Pallet::<T>::deposit_event(pallet::Event::LevelDiffConfigUpdated { entity_id, levels_count });
        Ok(())
    }

    fn clear_config(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        // X10: 幻影事件守卫 — 配置不存在时不 remove + 不 emit（与 team/referral 一致）
        if pallet::CustomLevelDiffConfigs::<T>::contains_key(entity_id) {
            pallet::CustomLevelDiffConfigs::<T>::remove(entity_id);
            pallet::Pallet::<T>::deposit_event(pallet::Event::LevelDiffConfigCleared { entity_id });
        }
        Ok(())
    }
}

// ============================================================================
// External modules
// ============================================================================

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
