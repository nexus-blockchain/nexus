//! # Commission Level-Diff Plugin (pallet-commission-level-diff)
//!
//! 等级极差返佣插件，支持自定义等级体系（Entity 自定义等级 + 返佣率）

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use pallet_commission_common::MemberProvider as _;

pub use pallet::*;

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
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;
        type MemberProvider: MemberProvider<Self::AccountId>;

        /// 实体查询接口（权限校验、Owner/Admin 判断）
        type EntityProvider: EntityProvider<Self::AccountId>;

        #[pallet::constant]
        type MaxCustomLevels: Get<u32>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

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
        LevelDiffConfigUpdated { entity_id: u64 },
        LevelDiffConfigCleared { entity_id: u64 },
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
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn set_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: BoundedVec<u16, T::MaxCustomLevels>,
            max_depth: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            Self::do_set_config(entity_id, level_rates, max_depth)
        }

        /// 清除等级极差配置（Entity Owner / Admin(COMMISSION_MANAGE)）
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
        pub fn clear_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_owner_or_admin(entity_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(CustomLevelDiffConfigs::<T>::contains_key(entity_id), Error::<T>::ConfigNotFound);

            CustomLevelDiffConfigs::<T>::remove(entity_id);
            Self::deposit_event(Event::LevelDiffConfigCleared { entity_id });
            Ok(())
        }

        /// [Root] 强制设置等级极差配置
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(45_000_000, 4_000))]
        pub fn force_set_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
            level_rates: BoundedVec<u16, T::MaxCustomLevels>,
            max_depth: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_set_config(entity_id, level_rates, max_depth)
        }

        /// [Root] 强制清除等级极差配置
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(35_000_000, 4_000))]
        pub fn force_clear_level_diff_config(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
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
            ensure!(max_depth > 0 && max_depth <= 20, Error::<T>::InvalidMaxDepth);

            CustomLevelDiffConfigs::<T>::insert(entity_id, CustomLevelDiffConfig {
                level_rates,
                max_depth,
            });

            Self::deposit_event(Event::LevelDiffConfigUpdated { entity_id });
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

                // X8: 跳过被封禁推荐人（与 team pallet 一致）
                if T::MemberProvider::is_banned(entity_id, referrer) {
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
        TB: sp_runtime::traits::AtLeast32BitUnsigned + Copy,
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

            // X8: 跳过被封禁推荐人（与 team pallet 一致）
            if T::MemberProvider::is_banned(entity_id, referrer) {
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
    ) -> (alloc::vec::Vec<pallet_commission_common::CommissionOutput<T::AccountId, TB>>, TB) {
        use pallet_commission_common::CommissionModes;

        if !enabled_modes.contains(CommissionModes::LEVEL_DIFF) {
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
        // M2-R3 审计修复: trait 路径也校验空 level_rates（与 extrinsic 一致）
        frame_support::ensure!(!level_rates.is_empty(), sp_runtime::DispatchError::Other("EmptyLevelRates"));
        for rate in level_rates.iter() {
            frame_support::ensure!(*rate <= 10000, sp_runtime::DispatchError::Other("InvalidRate"));
        }
        frame_support::ensure!(max_depth > 0 && max_depth <= 20, sp_runtime::DispatchError::Other("InvalidMaxDepth"));
        let bounded_rates: frame_support::BoundedVec<u16, T::MaxCustomLevels> =
            level_rates.try_into().map_err(|_| sp_runtime::DispatchError::Other("TooManyLevels"))?;
        pallet::CustomLevelDiffConfigs::<T>::insert(entity_id, pallet::CustomLevelDiffConfig {
            level_rates: bounded_rates,
            max_depth,
        });
        // M1 审计修复: trait 路径也发出事件
        pallet::Pallet::<T>::deposit_event(pallet::Event::LevelDiffConfigUpdated { entity_id });
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::{derive_impl, parameter_types, assert_ok, assert_noop};
    use sp_runtime::BuildStorage;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
    use pallet_commission_common::CommissionModes;

    type Balance = u128;
    const OWNER: u64 = 100;
    const ADMIN: u64 = 101;
    const NON_OWNER: u64 = 999;

    // ---- Thread-local mock state ----
    thread_local! {
        static REFERRERS: RefCell<BTreeMap<(u64, u64), u64>> = RefCell::new(BTreeMap::new());
        static CUSTOM_LEVEL_IDS: RefCell<BTreeMap<(u64, u64), u8>> = RefCell::new(BTreeMap::new());
        static LEVEL_BONUSES: RefCell<BTreeMap<(u64, u8), u16>> = RefCell::new(BTreeMap::new());
        static BANNED_MEMBERS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
        static ENTITY_OWNERS: RefCell<BTreeMap<u64, u64>> = RefCell::new(BTreeMap::new());
        static ENTITY_ADMINS: RefCell<BTreeSet<(u64, u64)>> = RefCell::new(BTreeSet::new());
        static ENTITY_LOCKED: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    }

    fn clear_mocks() {
        REFERRERS.with(|r| r.borrow_mut().clear());
        CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().clear());
        LEVEL_BONUSES.with(|l| l.borrow_mut().clear());
        BANNED_MEMBERS.with(|b| b.borrow_mut().clear());
        ENTITY_OWNERS.with(|o| o.borrow_mut().clear());
        ENTITY_ADMINS.with(|a| a.borrow_mut().clear());
        ENTITY_LOCKED.with(|l| l.borrow_mut().clear());
    }

    fn set_entity_owner(entity_id: u64, owner: u64) {
        ENTITY_OWNERS.with(|o| o.borrow_mut().insert(entity_id, owner));
    }

    fn set_entity_admin(entity_id: u64, admin: u64) {
        ENTITY_ADMINS.with(|a| a.borrow_mut().insert((entity_id, admin)));
    }

    fn lock_entity(entity_id: u64) {
        ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id));
    }

    fn ban_member(entity_id: u64, account: u64) {
        BANNED_MEMBERS.with(|b| b.borrow_mut().insert((entity_id, account)));
    }

    pub struct MockMemberProvider;

    impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
        fn is_member(_: u64, _: &u64) -> bool { true }
        fn get_referrer(entity_id: u64, account: &u64) -> Option<u64> {
            REFERRERS.with(|r| r.borrow().get(&(entity_id, *account)).copied())
        }
        fn get_member_stats(_: u64, _: &u64) -> (u32, u32, u128) { (0, 0, 0) }
        fn uses_custom_levels(_entity_id: u64) -> bool { true }
        fn custom_level_id(entity_id: u64, account: &u64) -> u8 {
            CUSTOM_LEVEL_IDS.with(|c| c.borrow().get(&(entity_id, *account)).copied().unwrap_or(0))
        }
        fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16 {
            LEVEL_BONUSES.with(|l| l.borrow().get(&(entity_id, level_id)).copied().unwrap_or(0))
        }
        fn is_banned(entity_id: u64, account: &u64) -> bool {
            BANNED_MEMBERS.with(|b| b.borrow().contains(&(entity_id, *account)))
        }
        fn auto_register(_: u64, _: &u64, _: Option<u64>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn set_upgrade_mode(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn remove_custom_level(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn custom_level_count(_: u64) -> u8 { 0 }
    }

    // -- Mock EntityProvider --
    pub struct MockEntityProvider;

    impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
        fn entity_exists(entity_id: u64) -> bool {
            ENTITY_OWNERS.with(|o| o.borrow().contains_key(&entity_id))
        }
        fn is_entity_active(_entity_id: u64) -> bool { true }
        fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> {
            Some(pallet_entity_common::EntityStatus::Active)
        }
        fn entity_owner(entity_id: u64) -> Option<u64> {
            ENTITY_OWNERS.with(|o| o.borrow().get(&entity_id).copied())
        }
        fn entity_account(_entity_id: u64) -> u64 { 0 }
        fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
        fn is_entity_admin(entity_id: u64, who: &u64, _perm: u32) -> bool {
            ENTITY_ADMINS.with(|a| a.borrow().contains(&(entity_id, *who)))
        }
        fn is_entity_locked(entity_id: u64) -> bool {
            ENTITY_LOCKED.with(|l| l.borrow().contains(&entity_id))
        }
    }

    // ---- Mock Runtime ----
    frame_support::construct_runtime!(
        pub enum Test {
            System: frame_system,
            Balances: pallet_balances,
            CommissionLevelDiff: pallet,
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

    parameter_types! {
        pub const MaxCustomLevels: u32 = 10;
    }

    impl pallet::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type MemberProvider = MockMemberProvider;
        type EntityProvider = MockEntityProvider;
        type MaxCustomLevels = MaxCustomLevels;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        clear_mocks();
        let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
            set_entity_owner(1, OWNER);
        });
        ext
    }

    // Helper: setup chain buyer(50) → 40 → 30 → 20 → 10
    fn setup_chain(entity_id: u64) {
        REFERRERS.with(|r| {
            let mut m = r.borrow_mut();
            m.insert((entity_id, 50), 40);
            m.insert((entity_id, 40), 30);
            m.insert((entity_id, 30), 20);
            m.insert((entity_id, 20), 10);
        });
    }

    // ========================================================================
    // P1: commission_bonus 回退测试
    // ========================================================================

    #[test]
    fn p1_fallback_to_commission_bonus_when_no_custom_config() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 推荐链: 50 → 40 → 30
            setup_chain(entity_id);

            // 设置等级: 40=level_0, 30=level_1
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
            });

            // 设置 commission_bonus（来自 CustomLevel 定义）
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((entity_id, 0), 300);  // 3%
                m.insert((entity_id, 1), 600);  // 6%
            });

            // 不设置 CustomLevelDiffConfig — 应回退到 commission_bonus
            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=300, prev=0, diff=300 → 10000×300/10000 = 300
            // 30: rate=600, prev=300, diff=300 → 10000×300/10000 = 300
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300);
            assert_eq!(remaining, 10000 - 600);
        });
    }

    #[test]
    fn p1_custom_config_takes_priority_over_commission_bonus() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            setup_chain(entity_id);

            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
            });

            // commission_bonus = 100, 200
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((entity_id, 0), 100);
                m.insert((entity_id, 1), 200);
            });

            // CustomLevelDiffConfig 配置 = 500, 800 → 应优先使用
            let level_rates = frame_support::BoundedVec::try_from(vec![500u16, 800]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id,
                level_rates,
                10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=500, prev=0, diff=500 → 500
            // 30: rate=800, prev=500, diff=300 → 300
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(outputs[1].amount, 300);
        });
    }

    // ========================================================================
    // P2: 等级数量不匹配测试
    // ========================================================================

    #[test]
    fn p2_level_id_out_of_bounds_falls_back_to_commission_bonus() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            setup_chain(entity_id);

            // 3 个自定义等级（id=0,1,2），但 CustomLevelDiffConfig 只有 2 条 rate
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);  // 越界！level_rates 只有 [0] 和 [1]
            });

            // commission_bonus 回退
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((entity_id, 2), 900);  // level_id=2 的回退
            });

            // level_rates 只有 2 个元素: [300, 600]
            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id,
                level_rates,
                10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=300 (from level_rates[0]), prev=0, diff=300 → 300
            // 30: rate=600 (from level_rates[1]), prev=300, diff=300 → 300
            // 20: rate=900 (fallback commission_bonus for level_id=2), prev=600, diff=300 → 300
            assert_eq!(outputs.len(), 3);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300);
            assert_eq!(outputs[2].beneficiary, 20);
            assert_eq!(outputs[2].amount, 300);
        });
    }

    #[test]
    fn p2_level_id_out_of_bounds_no_bonus_yields_zero() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 50 → 40
            REFERRERS.with(|r| r.borrow_mut().insert((entity_id, 50), 40));

            // level_id=2 超出 level_rates（空配置）
            CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((entity_id, 40), 2));
            // 不设置 commission_bonus → 回退为 0

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id,
                level_rates,
                10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();

            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // rate=0 (fallback, no bonus), prev=0 → no diff → no commission
            assert_eq!(outputs.len(), 0);
            assert_eq!(remaining, 10000);
        });
    }

    // ========================================================================
    // CommissionPlugin trait 测试
    // ========================================================================

    #[test]
    fn plugin_skips_when_level_diff_mode_not_enabled() {
        new_test_ext().execute_with(|| {
            let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
                1, &50, 10000, 10000, modes, false, 1,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    #[test]
    fn plugin_works_with_level_diff_mode_enabled() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            REFERRERS.with(|r| r.borrow_mut().insert((entity_id, 50), 40));
            CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((entity_id, 40), 0));
            LEVEL_BONUSES.with(|l| l.borrow_mut().insert((entity_id, 0), 500));

            let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::CommissionPlugin<u64, Balance>>::calculate(
                entity_id, &50, 10000, 10000, modes, false, 1,
            );

            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(remaining, 9500);
        });
    }

    // ========================================================================
    // Extrinsic 校验测试
    // ========================================================================

    #[test]
    fn set_config_validates_rates() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![10001u16]).unwrap();
            assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 10,
            ).is_err());
        });
    }

    #[test]
    fn set_config_validates_depth() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![100u16]).unwrap();
            // depth=0 invalid
            assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates.clone(), 0,
            ).is_err());
            // depth=21 invalid
            assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 21,
            ).is_err());
        });
    }

    // ========================================================================
    // set_level_rates trait 路径校验
    // ========================================================================

    #[test]
    fn set_level_rates_rejects_invalid_rate() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;

            assert!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
                1, vec![10001], 5
            ).is_err());
            // valid
            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
                1, vec![100, 200, 300], 5
            ));
            let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.level_rates.len(), 3);
            assert_eq!(config.max_depth, 5);
        });
    }

    // ========================================================================
    // 自定义等级体系基础测试
    // ========================================================================

    #[test]
    fn custom_level_diff_basic() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 推荐链: buyer(50) → A(40,level0) → B(30,level1) → C(20,level2)
            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);
            });

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id, level_rates, 10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // A: rate=300, prev=0, diff=300 → 300
            // B: rate=600, prev=300, diff=300 → 300
            // C: rate=900, prev=600, diff=300 → 300
            assert_eq!(outputs.len(), 3);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300);
            assert_eq!(outputs[2].beneficiary, 20);
            assert_eq!(outputs[2].amount, 300);
            assert_eq!(remaining, 10000 - 900);
        });
    }

    // ========================================================================
    // 额度耗尽提前退出测试
    // ========================================================================

    #[test]
    fn remaining_exhaustion_caps_commission() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 推荐链: 50 → 40 → 30 → 20
            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);
            });
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((entity_id, 0), 500);   // 5%
                m.insert((entity_id, 1), 1000);  // 10%
                m.insert((entity_id, 2), 1500);  // 15%
            });

            // 订单 10000，但 remaining 只有 600
            let mut remaining: Balance = 600;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=500, diff=500 → 10000×500/10000=500, actual=min(500,600)=500, remaining=100
            // 30: rate=1000, diff=500 → 500, actual=min(500,100)=100, remaining=0
            // 20: remaining=0 → break
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].amount, 500);
            assert_eq!(outputs[1].amount, 100); // capped by remaining
            assert_eq!(remaining, 0);
        });
    }

    // ========================================================================
    // 相同等级跳过测试
    // ========================================================================

    #[test]
    fn same_level_referrers_skipped() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 50 → 40(level1) → 30(level1) → 20(level2)
            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 1);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);
            });

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id, level_rates, 10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=600(level1), prev=0, diff=600 → 600
            // 30: rate=600(level1), prev=600, diff=0 → skipped
            // 20: rate=900(level2), prev=600, diff=300 → 300
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 600);
            assert_eq!(outputs[1].beneficiary, 20);
            assert_eq!(outputs[1].amount, 300);
        });
    }

    // ========================================================================
    // max_depth 限制测试
    // ========================================================================

    #[test]
    fn max_depth_limits_traversal() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 长链: 50 → 40 → 30 → 20 → 10
            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);
                m.insert((entity_id, 10), 3);
            });
            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((entity_id, 0), 300);
                m.insert((entity_id, 1), 600);
                m.insert((entity_id, 2), 900);
                m.insert((entity_id, 3), 1200);
            });

            // max_depth=2 → 只遍历前 2 层
            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900, 1200]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id, level_rates, 2,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 只有 40(depth=1) 和 30(depth=2)，20(depth=3) 被 max_depth=2 截断
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[1].beneficiary, 30);
        });
    }

    // ========================================================================
    // clear_config 清除测试
    // ========================================================================

    #[test]
    fn clear_config_removes_config() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;
            let entity_id = 1u64;

            // 设置配置
            let level_rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(), entity_id, level_rates, 5,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_some());

            // clear_config 应清除配置
            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(entity_id));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_none());
        });
    }

    // ========================================================================
    // 空推荐链测试
    // ========================================================================

    #[test]
    fn empty_referral_chain_produces_no_output() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 不设置推荐链 → buyer 无推荐人
            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id, level_rates, 10,
            ));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000);
        });
    }

    // ========================================================================
    // H1: 推荐链循环检测
    // ========================================================================

    #[test]
    fn h1_referral_cycle_does_not_loop_forever() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 循环推荐链: 50 → 40 → 30 → 40 (cycle!)
            REFERRERS.with(|r| {
                let mut m = r.borrow_mut();
                m.insert((entity_id, 50), 40);
                m.insert((entity_id, 40), 30);
                m.insert((entity_id, 30), 40); // cycle back to 40
            });

            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
            });

            LEVEL_BONUSES.with(|l| {
                let mut m = l.borrow_mut();
                m.insert((entity_id, 0), 300);
                m.insert((entity_id, 1), 600);
            });

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=300, diff=300 → 300
            // 30: rate=600, diff=300 → 300
            // 40 again → visited, break (cycle detected)
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300);
            assert_eq!(remaining, 10000 - 600);
        });
    }

    #[test]
    fn h1_self_referral_cycle_breaks_immediately() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 自推荐: 50 → 40 → 40 (self-cycle)
            REFERRERS.with(|r| {
                let mut m = r.borrow_mut();
                m.insert((entity_id, 50), 40);
                m.insert((entity_id, 40), 40); // self-referral
            });

            CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((entity_id, 40), 0));
            LEVEL_BONUSES.with(|l| l.borrow_mut().insert((entity_id, 0), 500));

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=500, diff=500 → 500
            // 40 again → visited, break
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 500);
        });
    }

    // ========================================================================
    // H2: 空 level_rates 拒绝
    // ========================================================================

    #[test]
    fn h2_set_config_rejects_empty_level_rates() {
        new_test_ext().execute_with(|| {
            let empty_rates = frame_support::BoundedVec::try_from(vec![]).unwrap();
            assert!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, empty_rates, 10,
            ).is_err());

            // 确认存储未被写入
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
        });
    }

    // ========================================================================
    // M1: trait 路径发出事件
    // ========================================================================

    #[test]
    fn m1_set_level_rates_trait_emits_event() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;
            let entity_id = 42u64;

            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
                entity_id, vec![100, 200], 5
            ));

            // 检查事件
            let events = frame_system::Pallet::<Test>::events();
            let found = events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigUpdated { entity_id: 42 })
                )
            });
            assert!(found, "LevelDiffConfigUpdated event should be emitted via trait path");
        });
    }

    // ========================================================================
    // M1-R3: clear_config 发出事件
    // ========================================================================

    #[test]
    fn m1r3_clear_config_emits_event() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;
            let entity_id = 7u64;

            // 先设置配置
            let level_rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(), entity_id, level_rates, 5,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_some());

            // 清除
            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(entity_id));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(entity_id).is_none());

            // 验证事件
            let events = frame_system::Pallet::<Test>::events();
            let found = events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigCleared { entity_id: 7 })
                )
            });
            assert!(found, "LevelDiffConfigCleared event should be emitted");
        });
    }

    // ========================================================================
    // M2-R4: TokenCommissionPlugin 测试覆盖
    // ========================================================================

    #[test]
    fn m2r4_token_plugin_basic_calculation() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 推荐链: 50 → 40(level0) → 30(level1)
            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
            });

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id, level_rates, 10,
            ));

            let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
                entity_id, &50, 10000u128, 10000u128, modes, false, 1,
            );

            // 40: rate=300, prev=0, diff=300 → 300
            // 30: rate=600, prev=300, diff=300 → 300
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300u128);
            assert_eq!(outputs[1].beneficiary, 30);
            assert_eq!(outputs[1].amount, 300u128);
            assert_eq!(remaining, 10000 - 600);
        });
    }

    #[test]
    fn m2r4_token_plugin_skips_when_mode_not_enabled() {
        new_test_ext().execute_with(|| {
            let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
                1, &50, 10000u128, 10000u128, modes, false, 1,
            );
            assert!(outputs.is_empty());
            assert_eq!(remaining, 10000u128);
        });
    }

    #[test]
    fn m2r4_token_plugin_cycle_detection() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 循环: 50 → 40 → 30 → 40
            REFERRERS.with(|r| {
                let mut m = r.borrow_mut();
                m.insert((entity_id, 50), 40);
                m.insert((entity_id, 40), 30);
                m.insert((entity_id, 30), 40);
            });
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
            });

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(pallet::Pallet::<Test>::force_set_level_diff_config(
                RuntimeOrigin::root(),
                entity_id, level_rates, 10,
            ));

            let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
                entity_id, &50, 10000u128, 10000u128, modes, false, 1,
            );

            // 40 → 30 → cycle back to 40, break
            assert_eq!(outputs.len(), 2);
            assert_eq!(remaining, 10000u128 - 600);
        });
    }

    // ========================================================================
    // M2-R3: trait 路径拒绝空 level_rates
    // ========================================================================

    #[test]
    fn m2r3_set_level_rates_trait_rejects_empty() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;

            let result = <pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(
                1, vec![], 5
            );
            assert!(result.is_err());

            // 确认存储未被写入
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
        });
    }

    // ========================================================================
    // X1: Owner/Admin 权限模型
    // ========================================================================

    #[test]
    fn x1_owner_can_set_config() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16, 600]).unwrap();
            assert_ok!(CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1, rates, 10,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
        });
    }

    #[test]
    fn x1_admin_can_set_config() {
        new_test_ext().execute_with(|| {
            set_entity_admin(1, ADMIN);
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::set_level_diff_config(
                RuntimeOrigin::signed(ADMIN), 1, rates, 5,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
        });
    }

    #[test]
    fn x1_non_owner_rejected() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_noop!(
                CommissionLevelDiff::set_level_diff_config(
                    RuntimeOrigin::signed(NON_OWNER), 1, rates, 5,
                ),
                Error::<Test>::NotEntityOwnerOrAdmin
            );
        });
    }

    #[test]
    fn x1_entity_not_found_rejected() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_noop!(
                CommissionLevelDiff::set_level_diff_config(
                    RuntimeOrigin::signed(OWNER), 999, rates, 5,
                ),
                Error::<Test>::EntityNotFound
            );
        });
    }

    // ========================================================================
    // X2: force_set (Root-only)
    // ========================================================================

    #[test]
    fn x2_force_set_works_for_root() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![500u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 10,
            ));
            let config = pallet::CustomLevelDiffConfigs::<Test>::get(1).unwrap();
            assert_eq!(config.level_rates[0], 500);
        });
    }

    #[test]
    fn x2_force_set_rejects_non_root() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![500u16]).unwrap();
            assert_noop!(
                CommissionLevelDiff::force_set_level_diff_config(
                    RuntimeOrigin::signed(OWNER), 1, rates, 10,
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    // ========================================================================
    // X3: clear_level_diff_config (signed, Owner/Admin)
    // ========================================================================

    #[test]
    fn x3_owner_can_clear_config() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 5,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());

            assert_ok!(CommissionLevelDiff::clear_level_diff_config(
                RuntimeOrigin::signed(OWNER), 1,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
        });
    }

    #[test]
    fn x3_clear_rejects_non_owner() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 5,
            ));

            assert_noop!(
                CommissionLevelDiff::clear_level_diff_config(
                    RuntimeOrigin::signed(NON_OWNER), 1,
                ),
                Error::<Test>::NotEntityOwnerOrAdmin
            );
        });
    }

    #[test]
    fn x3_clear_rejects_absent_config() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                CommissionLevelDiff::clear_level_diff_config(
                    RuntimeOrigin::signed(OWNER), 1,
                ),
                Error::<Test>::ConfigNotFound
            );
        });
    }

    // ========================================================================
    // X4: force_clear (Root-only)
    // ========================================================================

    #[test]
    fn x4_force_clear_works() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 5,
            ));
            assert_ok!(CommissionLevelDiff::force_clear_level_diff_config(
                RuntimeOrigin::root(), 1,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_none());
        });
    }

    #[test]
    fn x4_force_clear_rejects_non_root() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 5,
            ));
            assert_noop!(
                CommissionLevelDiff::force_clear_level_diff_config(
                    RuntimeOrigin::signed(OWNER), 1,
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn x4_force_clear_rejects_absent() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                CommissionLevelDiff::force_clear_level_diff_config(
                    RuntimeOrigin::root(), 1,
                ),
                Error::<Test>::ConfigNotFound
            );
        });
    }

    // ========================================================================
    // X6: 治理锁检查
    // ========================================================================

    #[test]
    fn x6_locked_entity_rejects_set() {
        new_test_ext().execute_with(|| {
            lock_entity(1);
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_noop!(
                CommissionLevelDiff::set_level_diff_config(
                    RuntimeOrigin::signed(OWNER), 1, rates, 5,
                ),
                Error::<Test>::EntityLocked
            );
        });
    }

    #[test]
    fn x6_locked_entity_rejects_clear() {
        new_test_ext().execute_with(|| {
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 5,
            ));
            lock_entity(1);
            assert_noop!(
                CommissionLevelDiff::clear_level_diff_config(
                    RuntimeOrigin::signed(OWNER), 1,
                ),
                Error::<Test>::EntityLocked
            );
        });
    }

    #[test]
    fn x6_force_set_bypasses_lock() {
        new_test_ext().execute_with(|| {
            lock_entity(1);
            let rates = frame_support::BoundedVec::try_from(vec![300u16]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), 1, rates, 5,
            ));
            assert!(pallet::CustomLevelDiffConfigs::<Test>::get(1).is_some());
        });
    }

    // ========================================================================
    // X8: is_banned 守卫
    // ========================================================================

    #[test]
    fn x8_banned_referrer_skipped_in_nex() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            // 推荐链: 50 → 40 → 30 → 20
            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);
            });

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), entity_id, level_rates, 10,
            ));

            // 封禁 30 → 应跳过
            ban_member(entity_id, 30);

            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            // 40: rate=300, diff=300 → 300
            // 30: banned → skipped
            // 20: rate=900, prev=300, diff=600 → 600
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300);
            assert_eq!(outputs[1].beneficiary, 20);
            assert_eq!(outputs[1].amount, 600);
        });
    }

    #[test]
    fn x8_banned_referrer_skipped_in_token() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            setup_chain(entity_id);
            CUSTOM_LEVEL_IDS.with(|c| {
                let mut m = c.borrow_mut();
                m.insert((entity_id, 40), 0);
                m.insert((entity_id, 30), 1);
                m.insert((entity_id, 20), 2);
            });

            let level_rates = frame_support::BoundedVec::try_from(vec![300u16, 600, 900]).unwrap();
            assert_ok!(CommissionLevelDiff::force_set_level_diff_config(
                RuntimeOrigin::root(), entity_id, level_rates, 10,
            ));

            ban_member(entity_id, 30);

            let modes = CommissionModes(CommissionModes::LEVEL_DIFF);
            let (outputs, remaining) = <pallet::Pallet<Test> as pallet_commission_common::TokenCommissionPlugin<u64, u128>>::calculate_token(
                entity_id, &50, 10000u128, 10000u128, modes, false, 1,
            );

            // 40: rate=300 → 300, 30: banned, 20: rate=900, diff=600 → 600
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].beneficiary, 40);
            assert_eq!(outputs[0].amount, 300u128);
            assert_eq!(outputs[1].beneficiary, 20);
            assert_eq!(outputs[1].amount, 600u128);
            assert_eq!(remaining, 10000u128 - 900);
        });
    }

    #[test]
    fn x8_non_banned_still_receives() {
        new_test_ext().execute_with(|| {
            let entity_id = 1u64;

            REFERRERS.with(|r| r.borrow_mut().insert((entity_id, 50), 40));
            CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().insert((entity_id, 40), 0));
            LEVEL_BONUSES.with(|l| l.borrow_mut().insert((entity_id, 0), 500));

            // 不封禁 → 正常获得佣金
            let mut remaining: Balance = 10000;
            let mut outputs = alloc::vec::Vec::new();
            pallet::Pallet::<Test>::process_level_diff(
                entity_id, &50, 10000, &mut remaining, &mut outputs,
            );

            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0].amount, 500);
        });
    }

    // ========================================================================
    // X10: 幻影事件守卫
    // ========================================================================

    #[test]
    fn x10_trait_clear_no_phantom_event_when_absent() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;

            // 配置不存在时 clear 不应发射事件
            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(1));

            let events = frame_system::Pallet::<Test>::events();
            let found = events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigCleared { .. })
                )
            });
            assert!(!found, "No LevelDiffConfigCleared event should be emitted when config absent");
        });
    }

    #[test]
    fn x10_trait_clear_emits_event_when_present() {
        new_test_ext().execute_with(|| {
            use pallet_commission_common::LevelDiffPlanWriter;

            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::set_level_rates(1, vec![100], 5));
            assert_ok!(<pallet::Pallet<Test> as LevelDiffPlanWriter>::clear_config(1));

            let events = frame_system::Pallet::<Test>::events();
            let found = events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::CommissionLevelDiff(pallet::Event::LevelDiffConfigCleared { entity_id: 1 })
                )
            });
            assert!(found, "LevelDiffConfigCleared should be emitted when config exists");
        });
    }
}
