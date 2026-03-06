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
    static KYC_BLOCKED: RefCell<BTreeMap<(u64, u64), bool>> = RefCell::new(BTreeMap::new());
    static ENTITY_INACTIVE: RefCell<BTreeMap<u64, bool>> = RefCell::new(BTreeMap::new());
    static ENTITY_ADMINS: RefCell<BTreeMap<(u64, u64), u32>> = RefCell::new(BTreeMap::new());
    static ENTITY_LOCKED: RefCell<BTreeMap<u64, bool>> = RefCell::new(BTreeMap::new());
    // M1-R8: 封禁/冻结会员
    static BANNED_MEMBERS: RefCell<BTreeMap<(u64, u64), bool>> = RefCell::new(BTreeMap::new());
    static FROZEN_MEMBERS: RefCell<BTreeMap<(u64, u64), bool>> = RefCell::new(BTreeMap::new());
}

fn clear_mocks() {
    MEMBERS.with(|m| m.borrow_mut().clear());
    CUSTOM_LEVEL_IDS.with(|c| c.borrow_mut().clear());
    LEVEL_MEMBER_COUNTS.with(|l| l.borrow_mut().clear());
    POOL_BALANCES.with(|p| p.borrow_mut().clear());
    TOKEN_POOL_BALANCES.with(|p| p.borrow_mut().clear());
    TOKEN_BALANCES.with(|m| m.borrow_mut().clear());
    KYC_BLOCKED.with(|k| k.borrow_mut().clear());
    ENTITY_INACTIVE.with(|e| e.borrow_mut().clear());
    ENTITY_ADMINS.with(|a| a.borrow_mut().clear());
    ENTITY_LOCKED.with(|l| l.borrow_mut().clear());
    BANNED_MEMBERS.with(|b| b.borrow_mut().clear());
    FROZEN_MEMBERS.with(|f| f.borrow_mut().clear());
}

fn set_kyc_blocked(entity_id: u64, account: u64) {
    KYC_BLOCKED.with(|k| k.borrow_mut().insert((entity_id, account), true));
}

fn set_entity_inactive(entity_id: u64) {
    ENTITY_INACTIVE.with(|e| e.borrow_mut().insert(entity_id, true));
}

fn set_entity_admin(entity_id: u64, account: u64, permissions: u32) {
    ENTITY_ADMINS.with(|a| a.borrow_mut().insert((entity_id, account), permissions));
}

fn set_entity_locked(entity_id: u64) {
    ENTITY_LOCKED.with(|l| l.borrow_mut().insert(entity_id, true));
}

fn ban_member(entity_id: u64, account: u64) {
    BANNED_MEMBERS.with(|b| b.borrow_mut().insert((entity_id, account), true));
}

fn freeze_member(entity_id: u64, account: u64) {
    FROZEN_MEMBERS.with(|f| f.borrow_mut().insert((entity_id, account), true));
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

fn get_token_pool_balance(entity_id: u64) -> Balance {
    TOKEN_POOL_BALANCES.with(|p| p.borrow().get(&entity_id).copied().unwrap_or(0))
}

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

// -- Mock ParticipationGuard --
pub struct MockParticipationGuard;

impl pallet_commission_common::ParticipationGuard<u64> for MockParticipationGuard {
    fn can_participate(entity_id: u64, account: &u64) -> bool {
        !KYC_BLOCKED.with(|k| k.borrow().get(&(entity_id, *account)).copied().unwrap_or(false))
    }
}

// -- Mock MemberProvider --
pub struct MockMemberProvider;

impl pallet_commission_common::MemberProvider<u64> for MockMemberProvider {
    fn is_member(entity_id: u64, account: &u64) -> bool {
        MEMBERS.with(|m| m.borrow().contains_key(&(entity_id, *account)))
    }
    fn get_referrer(_: u64, _: &u64) -> Option<u64> { None }
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
    fn is_banned(entity_id: u64, account: &u64) -> bool {
        BANNED_MEMBERS.with(|b| b.borrow().get(&(entity_id, *account)).copied().unwrap_or(false))
    }
    fn is_member_active(entity_id: u64, account: &u64) -> bool {
        !Self::is_banned(entity_id, account) &&
        !FROZEN_MEMBERS.with(|f| f.borrow().get(&(entity_id, *account)).copied().unwrap_or(false))
    }
}

// -- Mock EntityProvider --
pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        !ENTITY_INACTIVE.with(|e| e.borrow().get(&entity_id).copied().unwrap_or(false))
    }
    fn is_entity_active(entity_id: u64) -> bool {
        !ENTITY_INACTIVE.with(|e| e.borrow().get(&entity_id).copied().unwrap_or(false))
    }
    fn entity_status(_: u64) -> Option<pallet_entity_common::EntityStatus> { None }
    fn entity_owner(_: u64) -> Option<u64> { Some(OWNER) }
    fn entity_account(entity_id: u64) -> u64 { entity_id + 9000 }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn is_entity_admin(_entity_id: u64, account: &u64, required_permission: u32) -> bool {
        ENTITY_ADMINS.with(|a| {
            a.borrow().get(&(_entity_id, *account))
                .map(|perms| perms & required_permission == required_permission)
                .unwrap_or(false)
        })
    }
    fn is_entity_locked(entity_id: u64) -> bool {
        ENTITY_LOCKED.with(|l| l.borrow().get(&entity_id).copied().unwrap_or(false))
    }
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
    pub const MinRoundDuration: u64 = 10;
}

impl pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MemberProvider = MockMemberProvider;
    type EntityProvider = MockEntityProvider;
    type PoolBalanceProvider = MockPoolBalanceProvider;
    type MaxPoolRewardLevels = ConstU32<10>;
    type MaxClaimHistory = ConstU32<5>;
    type TokenBalance = u128;
    type TokenPoolBalanceProvider = MockTokenPoolBalanceProvider;
    type TokenTransferProvider = MockTokenTransferProvider;
    type ParticipationGuard = MockParticipationGuard;
    type WeightInfo = ();
    type MinRoundDuration = MinRoundDuration;
    type MaxRoundHistory = ConstU32<5>;
    type ClaimCallback = ();
}

/// Entity account = entity_id + 9000
const ENTITY_ACCOUNT: u64 = 9001; // entity_id=1
/// Entity owner (mock returns 999 for all entities)
const OWNER: u64 = 999;

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
        // Fund owner account
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(), OWNER, 1_000_000,
        );
    });
    ext
}

fn setup_config(entity_id: u64) {
    // level_1=5000bps(50%), level_2=5000bps(50%), sum=10000
    let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
        vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
    assert_ok!(CommissionPoolReward::set_pool_reward_config(
        RuntimeOrigin::signed(OWNER), entity_id, ratios, 100,
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
            RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 200,
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
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
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
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
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
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
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
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 0,
            ),
            Error::<Test>::InvalidRoundDuration
        );
    });
}

#[test]
fn set_config_rejects_unauthorized() {
    new_test_ext().execute_with(|| {
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(1), 1, ratios.try_into().unwrap(), 100,
            ),
            Error::<Test>::NotAuthorized
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
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 1);

        // Force again creates round 2
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
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
        RuntimeOrigin::signed(OWNER), entity_id, true,
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
            RuntimeOrigin::signed(OWNER), entity_id, true,
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
                RuntimeOrigin::signed(OWNER), 999, true,
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
            RuntimeOrigin::signed(OWNER), entity_id,
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
            RuntimeOrigin::signed(OWNER), entity_id,
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

// ====================================================================
// Regression tests (audit fixes)
// ====================================================================

#[test]
fn h1_plan_writer_rejects_invalid_ratio_sum() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        // sum = 6000, not 10000
        assert!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 3000), (2, 3000)], 100,
        ).is_err());
    });
}

#[test]
fn h1_plan_writer_rejects_zero_ratio() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        assert!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 0), (2, 10000)], 100,
        ).is_err());
    });
}

#[test]
fn h1_plan_writer_rejects_duplicate_level() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        assert!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 5000), (1, 5000)], 100,
        ).is_err());
    });
}

#[test]
fn h1_plan_writer_rejects_zero_duration() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        assert!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 10000)], 0,
        ).is_err());
    });
}

#[test]
fn h2_set_config_preserves_token_pool_enabled() {
    new_test_ext().execute_with(|| {
        // Set initial config
        let ratios = vec![(1u8, 5000u16), (2, 5000)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
        ));
        // Enable token pool
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(OWNER), 1, true,
        ));
        assert!(pallet::PoolRewardConfigs::<Test>::get(1).unwrap().token_pool_enabled);

        // Update config (change ratios) — token_pool_enabled should be preserved
        let new_ratios = vec![(1u8, 3000u16), (2, 7000)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), 1, new_ratios.try_into().unwrap(), 200,
        ));
        let config = pallet::PoolRewardConfigs::<Test>::get(1).unwrap();
        assert!(config.token_pool_enabled, "token_pool_enabled should be preserved after config update");
        assert_eq!(config.round_duration, 200);
    });
}

#[test]
fn h2_plan_writer_preserves_token_pool_enabled() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        // Set config via PlanWriter
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 10000)], 100,
        ));
        // Enable token pool
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_token_pool_enabled(1, true));
        assert!(pallet::PoolRewardConfigs::<Test>::get(1).unwrap().token_pool_enabled);

        // Update config via PlanWriter — token_pool_enabled should be preserved
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 5000), (2, 5000)], 200,
        ));
        assert!(
            pallet::PoolRewardConfigs::<Test>::get(1).unwrap().token_pool_enabled,
            "PlanWriter should preserve token_pool_enabled"
        );
    });
}

#[test]
fn h3_clear_config_resets_last_claimed_round() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Claim round 1
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 1);

        // Clear and re-create config
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::clear_config(entity_id));

        // LastClaimedRound should be cleared
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 0);

        // Re-setup and user should be able to claim again
        setup_config(entity_id);
        set_pool_balance(entity_id, 10_000);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 1);
    });
}

#[test]
fn force_new_round_rejects_unauthorized() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::force_new_round(
                RuntimeOrigin::signed(10), 1,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn force_new_round_rejects_no_config() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionPoolReward::force_new_round(
                RuntimeOrigin::signed(OWNER), 999,
            ),
            Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn set_token_pool_enabled_rejects_unauthorized() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::set_token_pool_enabled(
                RuntimeOrigin::signed(10), 1, true,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn claim_zero_pool_balance_nothing_to_claim() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 0);

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ),
            Error::<Test>::NothingToClaim
        );
    });
}

#[test]
fn claim_insufficient_pool_after_snapshot() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // 创建快照（per_member = 5000）
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));

        // 快照后将池余额降到不足（模拟外部消耗）
        set_pool_balance(entity_id, 100);

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ),
            Error::<Test>::InsufficientPool
        );
    });
}

#[test]
fn set_config_rejects_ratio_over_10000() {
    new_test_ext().execute_with(|| {
        let ratios = vec![(1u8, 10001u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
            ),
            Error::<Test>::InvalidRatio
        );
    });
}

#[test]
fn multi_entity_isolation() {
    new_test_ext().execute_with(|| {
        // Entity 1
        setup_config(1);
        set_member(1, 10, 1);
        set_level_count(1, 1, 1);
        set_level_count(1, 2, 1);
        set_pool_balance(1, 10_000);

        // Entity 2
        let ratios2 = vec![(1u8, 10000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), 2, ratios2.try_into().unwrap(), 50,
        ));
        set_member(2, 20, 1);
        set_level_count(2, 1, 1);
        set_pool_balance(2, 3_000);
        // Fund entity 2 account (2 + 9000 = 9002)
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(), 9002, 500_000,
        );

        let bal_10 = pallet_balances::Pallet::<Test>::free_balance(10);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), 1,
        ));
        assert_eq!(pallet_balances::Pallet::<Test>::free_balance(10) - bal_10, 5000);

        let bal_20 = pallet_balances::Pallet::<Test>::free_balance(20);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(20), 2,
        ));
        assert_eq!(pallet_balances::Pallet::<Test>::free_balance(20) - bal_20, 3000);

        // 互不影响
        assert_eq!(get_pool_balance(1), 5_000);
        assert_eq!(get_pool_balance(2), 0);
    });
}

#[test]
fn claim_after_round_rollover_allowed() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 100_000);

        // Round 1
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 1);

        // Round 2
        System::set_block_number(101);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 2);

        // Round 3
        System::set_block_number(201);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 3);
    });
}

#[test]
fn token_deduct_fail_rolls_back_transfer() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(OWNER), entity_id, true,
        ));
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        set_token_pool_balance(entity_id, 5_000);
        set_token_balance(entity_id, ENTITY_ACCOUNT, 5_000);

        // 创建快照（token per_member = 2500）
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        // 快照后清空 token pool → deduct_token_pool 会失败
        set_token_pool_balance(entity_id, 0);

        let nex_before = pallet_balances::Pallet::<Test>::free_balance(10);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));

        // NEX 正常领取
        assert_eq!(pallet_balances::Pallet::<Test>::free_balance(10) - nex_before, 5000);
        // Token 转账被回滚
        assert_eq!(get_token_balance(entity_id, 10), 0);
        assert_eq!(get_token_balance(entity_id, ENTITY_ACCOUNT), 5_000);
        // Claim record token_amount = 0
        let records = pallet::ClaimRecords::<Test>::get(entity_id, 10);
        assert_eq!(records[0].token_amount, 0);
    });
}

#[test]
fn snapshot_with_empty_pool_produces_zero_rewards() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_level_count(entity_id, 1, 5);
        set_level_count(entity_id, 2, 3);
        set_pool_balance(entity_id, 0);

        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));

        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.pool_snapshot, 0);
        for snap in round.level_snapshots.iter() {
            assert_eq!(snap.per_member_reward, 0);
        }
    });
}

#[test]
fn m1_round_id_overflow_rejected() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Directly insert a round with round_id = u64::MAX
        let config = pallet::PoolRewardConfigs::<Test>::get(entity_id).unwrap();
        pallet::CurrentRound::<Test>::insert(entity_id, RoundInfo {
            round_id: u64::MAX,
            start_block: 0u64,
            pool_snapshot: 0u128,
            level_snapshots: config.level_ratios.iter().map(|(id, _)| LevelSnapshot {
                level_id: *id,
                member_count: 1,
                per_member_reward: 0u128,
                claimed_count: 0,
            }).collect::<alloc::vec::Vec<_>>().try_into().unwrap(),
            token_pool_snapshot: None,
            token_level_snapshots: None,
        });

        // Advance past round_duration to force new round creation
        System::set_block_number(200);
        set_member(entity_id, 10, 1);

        // claim_pool_reward should fail with RoundIdOverflow
        assert_noop!(
            CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ),
            Error::<Test>::RoundIdOverflow
        );

        // force_new_round should also fail
        assert_noop!(
            CommissionPoolReward::force_new_round(
                RuntimeOrigin::signed(OWNER), entity_id,
            ),
            Error::<Test>::RoundIdOverflow
        );
    });
}

// ====================================================================
// PR-H1: ParticipationGuard blocks pool reward claim
// ====================================================================

#[test]
fn pr_h1_claim_blocked_when_participation_denied() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        let account = 10u64;
        setup_config(entity_id);
        set_member(entity_id, account, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Block account via KYC
        set_kyc_blocked(entity_id, account);

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(account), entity_id,
            ),
            Error::<Test>::ParticipationRequirementNotMet
        );
    });
}

// ====================================================================
// Round 2 审计回归测试
// ====================================================================

/// H2: set_pool_reward_config 更新配置后旧快照被清除，下次 claim 创建新快照
#[test]
fn h2_config_update_invalidates_current_round() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id); // level_1=5000, level_2=5000
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // 创建轮次快照
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 1);
        // per_member for level_1 = 10000 * 5000/10000 / 1 = 5000
        assert_eq!(round.level_snapshots[0].per_member_reward, 5000);

        // 更新配置: 移除 level_2, 添加 level_3
        let new_ratios = vec![(1u8, 3000u16), (3u8, 7000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, new_ratios.try_into().unwrap(), 100,
        ));

        // CurrentRound 应被清除
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_none());

        // 下次 claim 应创建新快照
        set_level_count(entity_id, 3, 2);
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        // M2-R3: round_id 单调递增（old=1 → new=2），不再重置为 1
        assert_eq!(round.round_id, 2);
        // 新快照应有 level_1 和 level_3（不是 level_2）
        assert_eq!(round.level_snapshots.len(), 2);
        assert_eq!(round.level_snapshots[0].level_id, 1);
        assert_eq!(round.level_snapshots[1].level_id, 3);
    });
}

/// H2: PlanWriter 更新配置也清除当前轮次
#[test]
fn h2_plan_writer_config_update_invalidates_round() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        let entity_id = 1u64;
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            entity_id, vec![(1, 5000), (2, 5000)], 100,
        ));
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // 创建快照
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_some());

        // PlanWriter 更新配置
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            entity_id, vec![(1, 3000), (2, 7000)], 200,
        ));

        // 快照应被清除
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_none());
    });
}

/// H2: 用户在旧快照中已 claim，配置更新后 LastClaimedRound 被清除，可立即 claim 新轮次
#[test]
fn h2_config_update_mid_round_allows_reclaim() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 20_000);

        // 用户在 round 1 领取
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 1);

        // 管理员更新配置（清除 CurrentRound + LastClaimedRound）
        let new_ratios = vec![(1u8, 10000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, new_ratios.try_into().unwrap(), 100,
        ));

        // M2-R3: LastClaimedRound 不再被 clear_prefix 清除，保留历史值
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 1);

        // 用户可以立即 claim 新轮次（round_id=2, last_claimed=1 → 1 < 2 通过）
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 2);
    });
}

/// M2: Banned/Closed Entity 的会员不能领取池奖励
#[test]
fn m2_claim_rejects_entity_not_active() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Entity 被封禁
        set_entity_inactive(entity_id);

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

/// M2: Entity 激活时领取正常
#[test]
fn m2_claim_works_when_entity_active() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Entity 默认活跃
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
    });
}

// ====================================================================
// Round 3 审计回归测试
// ====================================================================

/// M1-R3: set_token_pool_enabled 使当前轮次失效，启用后新轮次包含 token 快照
#[test]
fn m1_r3_token_enable_invalidates_round_and_adds_token_snapshot() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id); // token_pool_enabled = false
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        set_token_pool_balance(entity_id, 5_000);
        set_token_balance(entity_id, ENTITY_ACCOUNT, 5_000);

        // 创建轮次（无 token 快照）
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 1);
        assert!(round.token_level_snapshots.is_none());

        // 启用 token 池 → 当前轮次应失效
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(OWNER), entity_id, true,
        ));
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_none());

        // 下次 claim 创建新轮次，应包含 token 快照
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 2); // 单调递增
        assert!(round.token_level_snapshots.is_some());

        // 验证 token 已转入用户
        assert!(get_token_balance(entity_id, 10) > 0);
    });
}

/// M1-R3: set_token_pool_enabled 禁用后立即生效，新轮次无 token 快照
#[test]
fn m1_r3_token_disable_invalidates_round_removes_token_snapshot() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config_with_token(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        set_token_pool_balance(entity_id, 5_000);

        // 创建轮次（有 token 快照）
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert!(round.token_level_snapshots.is_some());

        // 禁用 token 池 → 当前轮次应失效
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(OWNER), entity_id, false,
        ));
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_none());

        // 下次 claim 创建新轮次，不应包含 token 快照
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert!(round.token_level_snapshots.is_none());
    });
}

/// M2-R3: 配置更新后 round_id 保持单调递增，LastClaimedRound 不被清除
#[test]
fn m2_r3_config_update_round_id_monotonic() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 100_000);

        // 连续 claim 3 轮
        for i in 0..3u64 {
            System::set_block_number(1 + i * 101);
            assert_ok!(CommissionPoolReward::claim_pool_reward(
                RuntimeOrigin::signed(10), entity_id,
            ));
        }
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 3);

        // 更新配置
        let new_ratios = vec![(1u8, 10000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, new_ratios.try_into().unwrap(), 100,
        ));

        // LastClaimedRound 保留（不再 clear_prefix）
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 3);

        // 新轮次 round_id = 4（old=3 → 3+1=4）
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 4);
        assert_eq!(pallet::LastClaimedRound::<Test>::get(entity_id, 10), 4);
    });
}

/// M2-R3: 多次配置更新 round_id 始终递增
#[test]
fn m2_r3_multiple_config_updates_round_id_keeps_increasing() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 100_000);

        // Round 1
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::CurrentRound::<Test>::get(entity_id).unwrap().round_id, 1);

        // Config update #1
        let ratios = vec![(1u8, 10000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, ratios.try_into().unwrap(), 100,
        ));

        // Config update #2 (no claim in between)
        let ratios2 = vec![(1u8, 4000u16), (2u8, 6000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, ratios2.try_into().unwrap(), 100,
        ));

        // LastRoundId should still be 1 from the original round
        // Next claim creates round 2
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));
        assert_eq!(pallet::CurrentRound::<Test>::get(entity_id).unwrap().round_id, 2);
    });
}

/// L1-R3: PlanWriter set_pool_reward_config 发出 PoolRewardConfigUpdated 事件
#[test]
fn l1_r3_plan_writer_emits_config_event() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        System::reset_events();

        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 10000)], 100,
        ));

        let events = System::events();
        assert!(
            events.iter().any(|e| matches!(
                &e.event,
                RuntimeEvent::CommissionPoolReward(pallet::Event::PoolRewardConfigUpdated { entity_id: 1 })
            )),
            "PlanWriter should emit PoolRewardConfigUpdated event"
        );
    });
}

/// L1-R3: PlanWriter set_token_pool_enabled 发出 TokenPoolEnabledUpdated 事件
#[test]
fn l1_r3_plan_writer_emits_token_event() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            1, vec![(1, 10000)], 100,
        ));

        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_token_pool_enabled(1, true));

        let events = System::events();
        assert!(
            events.iter().any(|e| matches!(
                &e.event,
                RuntimeEvent::CommissionPoolReward(pallet::Event::TokenPoolEnabledUpdated { entity_id: 1, enabled: true })
            )),
            "PlanWriter should emit TokenPoolEnabledUpdated event"
        );
    });
}

// ====================================================================
// Round 4 审计回归测试
// ====================================================================

/// L1-R4: 幂等 set_token_pool_enabled 不应使当前轮次失效
#[test]
fn l1_r4_idempotent_token_toggle_preserves_round() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config_with_token(entity_id); // token_pool_enabled = true
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        set_token_pool_balance(entity_id, 5_000);

        // 创建轮次
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 1);

        // 幂等调用: 已经是 true，再次设置 true → 轮次应保留
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(OWNER), entity_id, true,
        ));
        let round_after = pallet::CurrentRound::<Test>::get(entity_id);
        assert!(round_after.is_some(), "Idempotent toggle should NOT invalidate round");
        assert_eq!(round_after.unwrap().round_id, 1);
    });
}

/// L1-R4: PlanWriter 幂等 set_token_pool_enabled 不应使当前轮次失效
#[test]
fn l1_r4_plan_writer_idempotent_token_toggle_preserves_round() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        let entity_id = 1u64;
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_pool_reward_config(
            entity_id, vec![(1, 5000), (2, 5000)], 100,
        ));
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_token_pool_enabled(entity_id, true));

        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        set_token_pool_balance(entity_id, 5_000);

        // 创建轮次
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        assert_eq!(pallet::CurrentRound::<Test>::get(entity_id).unwrap().round_id, 1);

        // PlanWriter 幂等调用
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::set_token_pool_enabled(entity_id, true));
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_some(),
            "PlanWriter idempotent toggle should NOT invalidate round");
    });
}

/// L1-R4: 实际变更仍然正确失效轮次（非幂等时）
#[test]
fn l1_r4_actual_change_still_invalidates_round() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config_with_token(entity_id); // enabled = true
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        set_token_pool_balance(entity_id, 5_000);

        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_some());

        // 实际变更: true → false → 轮次应失效
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(OWNER), entity_id, false,
        ));
        assert!(pallet::CurrentRound::<Test>::get(entity_id).is_none(),
            "Actual change should invalidate round");
    });
}

// ====================================================================
// Round 5 审计回归测试
// ====================================================================

/// M1-R5: force_new_round 拒绝非活跃 Entity
#[test]
fn m1_r5_force_new_round_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Entity 被封禁
        set_entity_inactive(entity_id);

        assert_noop!(
            CommissionPoolReward::force_new_round(
                RuntimeOrigin::signed(OWNER), entity_id,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

/// M1-R5: force_new_round 对活跃 Entity 正常工作
#[test]
fn m1_r5_force_new_round_works_when_entity_active() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Entity 默认活跃
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(OWNER), entity_id,
        ));
        let round = pallet::CurrentRound::<Test>::get(entity_id).unwrap();
        assert_eq!(round.round_id, 1);
    });
}

/// M3-R5: clear_config 发出 PoolRewardConfigCleared 事件（非 Updated）
#[test]
fn m3_r5_clear_config_emits_cleared_event() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::PoolRewardPlanWriter;
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // Claim to create some state
        assert_ok!(CommissionPoolReward::claim_pool_reward(
            RuntimeOrigin::signed(10), entity_id,
        ));

        System::reset_events();
        assert_ok!(<pallet::Pallet<Test> as PoolRewardPlanWriter>::clear_config(entity_id));

        let events = System::events();
        // 应发出 Cleared，不应发出 Updated
        assert!(
            events.iter().any(|e| matches!(
                &e.event,
                RuntimeEvent::CommissionPoolReward(pallet::Event::PoolRewardConfigCleared { entity_id: 1 })
            )),
            "clear_config should emit PoolRewardConfigCleared"
        );
        assert!(
            !events.iter().any(|e| matches!(
                &e.event,
                RuntimeEvent::CommissionPoolReward(pallet::Event::PoolRewardConfigUpdated { .. })
            )),
            "clear_config should NOT emit PoolRewardConfigUpdated"
        );
    });
}

/// M1-R4: Weight 值合理性检查 — 非零且在预期范围内
#[test]
fn m1_r4_weight_values_are_reasonable() {
    use crate::weights::{WeightInfo, SubstrateWeight};

    let w1 = SubstrateWeight::set_pool_reward_config();
    assert!(w1.ref_time() >= 45_000_000, "set_pool_reward_config ref_time too low");
    assert!(w1.proof_size() >= 5_000, "set_pool_reward_config proof_size too low");

    let w2 = SubstrateWeight::claim_pool_reward();
    assert!(w2.ref_time() >= 150_000_000, "claim_pool_reward ref_time too low");
    assert!(w2.proof_size() >= 15_000, "claim_pool_reward proof_size too low");

    let w3 = SubstrateWeight::force_new_round();
    assert!(w3.ref_time() >= 100_000_000, "force_new_round ref_time too low");
    assert!(w3.proof_size() >= 10_000, "force_new_round proof_size too low");

    let w4 = SubstrateWeight::set_token_pool_enabled();
    assert!(w4.ref_time() >= 40_000_000, "set_token_pool_enabled ref_time too low");
    assert!(w4.proof_size() >= 4_000, "set_token_pool_enabled proof_size too low");
}

// ====================================================================
// P0: Owner/Admin 权限下放测试
// ====================================================================

/// P0: Admin(COMMISSION_MANAGE) 可以 set_pool_reward_config
#[test]
fn p0_admin_can_set_pool_reward_config() {
    new_test_ext().execute_with(|| {
        let admin = 888u64;
        // Fund admin
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            RuntimeOrigin::root(), admin, 100_000,
        );
        set_entity_admin(1, admin, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        let ratios = vec![(1u8, 10000u16)];
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(admin), 1, ratios.try_into().unwrap(), 100,
        ));
        assert!(pallet::PoolRewardConfigs::<Test>::get(1).is_some());
    });
}

/// P0: Admin 无 COMMISSION_MANAGE 权限被拒绝
#[test]
fn p0_admin_without_commission_manage_rejected() {
    new_test_ext().execute_with(|| {
        let admin = 888u64;
        // 仅有 ORDER_MANAGE 权限，无 COMMISSION_MANAGE
        set_entity_admin(1, admin, pallet_entity_common::AdminPermission::ORDER_MANAGE);
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(admin), 1, ratios.try_into().unwrap(), 100,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

/// P0: Admin(COMMISSION_MANAGE) 可以 force_new_round
#[test]
fn p0_admin_can_force_new_round() {
    new_test_ext().execute_with(|| {
        let admin = 888u64;
        set_entity_admin(1, admin, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        setup_config(1);
        set_level_count(1, 1, 1);
        set_level_count(1, 2, 1);
        set_pool_balance(1, 10_000);
        assert_ok!(CommissionPoolReward::force_new_round(
            RuntimeOrigin::signed(admin), 1,
        ));
        assert_eq!(pallet::CurrentRound::<Test>::get(1).unwrap().round_id, 1);
    });
}

/// P0: Admin(COMMISSION_MANAGE) 可以 set_token_pool_enabled
#[test]
fn p0_admin_can_set_token_pool_enabled() {
    new_test_ext().execute_with(|| {
        let admin = 888u64;
        set_entity_admin(1, admin, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        setup_config(1);
        assert_ok!(CommissionPoolReward::set_token_pool_enabled(
            RuntimeOrigin::signed(admin), 1, true,
        ));
        assert!(pallet::PoolRewardConfigs::<Test>::get(1).unwrap().token_pool_enabled);
    });
}

/// P0: Root origin 被 ensure_signed 拒绝（Root 通过 PlanWriter trait 操作）
#[test]
fn p0_root_origin_rejected() {
    new_test_ext().execute_with(|| {
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 100,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::force_new_round(RuntimeOrigin::root(), 1),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            CommissionPoolReward::set_token_pool_enabled(RuntimeOrigin::root(), 1, true),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// P0: 非活跃 Entity 的 Owner 不能配置
#[test]
fn p0_owner_rejected_for_inactive_entity() {
    new_test_ext().execute_with(|| {
        set_entity_inactive(1);
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ========================================================================
// P1: EntityLocked 检查 + Root force_* 紧急覆写
// ========================================================================

/// P1: 锁定 Entity 后 set_pool_reward_config 被拒绝
#[test]
fn p1_locked_entity_rejects_set_config() {
    new_test_ext().execute_with(|| {
        set_entity_locked(1);
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

/// P1: 锁定 Entity 后 force_new_round 被拒绝
#[test]
fn p1_locked_entity_rejects_force_new_round() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        set_entity_locked(1);
        assert_noop!(
            CommissionPoolReward::force_new_round(RuntimeOrigin::signed(OWNER), 1),
            Error::<Test>::EntityLocked
        );
    });
}

/// P1: 锁定 Entity 后 set_token_pool_enabled 被拒绝
#[test]
fn p1_locked_entity_rejects_set_token_pool_enabled() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        set_entity_locked(1);
        assert_noop!(
            CommissionPoolReward::set_token_pool_enabled(RuntimeOrigin::signed(OWNER), 1, true),
            Error::<Test>::EntityLocked
        );
    });
}

/// P1: Root force_set_pool_reward_config 绕过锁定
#[test]
fn p1_root_force_set_config_bypasses_lock() {
    new_test_ext().execute_with(|| {
        set_entity_locked(1);
        let ratios = vec![(1u8, 10000u16)];
        assert_ok!(CommissionPoolReward::force_set_pool_reward_config(
            RuntimeOrigin::root(), 1, ratios.try_into().unwrap(), 100,
        ));
        assert!(CommissionPoolReward::pool_reward_config(1).is_some());
    });
}

/// P1: Root force_set_token_pool_enabled 绕过锁定
#[test]
fn p1_root_force_set_token_pool_enabled_bypasses_lock() {
    new_test_ext().execute_with(|| {
        // 先设配置（未锁定时）
        setup_config(1);
        set_entity_locked(1);
        assert_ok!(CommissionPoolReward::force_set_token_pool_enabled(
            RuntimeOrigin::root(), 1, true,
        ));
        let config = CommissionPoolReward::pool_reward_config(1).unwrap();
        assert!(config.token_pool_enabled);
    });
}

/// P1: Root force_start_new_round 绕过锁定
#[test]
fn p1_root_force_start_new_round_bypasses_lock() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        set_pool_balance(1, 10_000);
        set_level_count(1, 1, 5);
        set_entity_locked(1);
        assert_ok!(CommissionPoolReward::force_start_new_round(
            RuntimeOrigin::root(), 1,
        ));
        assert!(CommissionPoolReward::current_round(1).is_some());
    });
}

/// P1: 非 Root 不能调用 force_set_pool_reward_config
#[test]
fn p1_force_set_config_rejects_non_root() {
    new_test_ext().execute_with(|| {
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::force_set_pool_reward_config(
                RuntimeOrigin::signed(OWNER), 1, ratios.try_into().unwrap(), 100,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// P1: 非 Root 不能调用 force_set_token_pool_enabled
#[test]
fn p1_force_set_token_pool_enabled_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::force_set_token_pool_enabled(
                RuntimeOrigin::signed(OWNER), 1, true,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// P1: 非 Root 不能调用 force_start_new_round
#[test]
fn p1_force_start_new_round_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::force_start_new_round(RuntimeOrigin::signed(OWNER), 1),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// P1: Admin 在锁定 Entity 上也被拒绝
#[test]
fn p1_admin_rejected_on_locked_entity() {
    new_test_ext().execute_with(|| {
        let admin = 888;
        set_entity_admin(1, admin, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), admin, 10_000_000).unwrap();
        set_entity_locked(1);
        let ratios = vec![(1u8, 10000u16)];
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(admin), 1, ratios.try_into().unwrap(), 100,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ========================================================================
// P2: clear_pool_reward_config + force_clear_pool_reward_config
// ========================================================================

/// P2: Owner 清除配置成功
#[test]
fn p2_clear_pool_reward_config_works() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert!(CommissionPoolReward::pool_reward_config(1).is_some());
        assert_ok!(CommissionPoolReward::clear_pool_reward_config(
            RuntimeOrigin::signed(OWNER), 1,
        ));
        assert!(CommissionPoolReward::pool_reward_config(1).is_none());
        // 当前轮次也被清除
        assert!(CommissionPoolReward::current_round(1).is_none());
    });
}

/// P2: 清除配置 — 无配置时返回 ConfigNotFound
#[test]
fn p2_clear_config_rejects_no_config() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionPoolReward::clear_pool_reward_config(RuntimeOrigin::signed(OWNER), 1),
            Error::<Test>::ConfigNotFound
        );
    });
}

/// P2: 清除配置 — 非授权用户被拒
#[test]
fn p2_clear_config_rejects_unauthorized() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::clear_pool_reward_config(RuntimeOrigin::signed(42), 1),
            Error::<Test>::NotAuthorized
        );
    });
}

/// P2: 清除配置 — 锁定 Entity 被拒
#[test]
fn p2_clear_config_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        set_entity_locked(1);
        assert_noop!(
            CommissionPoolReward::clear_pool_reward_config(RuntimeOrigin::signed(OWNER), 1),
            Error::<Test>::EntityLocked
        );
    });
}

/// P2: Admin 可清除配置
#[test]
fn p2_admin_can_clear_config() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        let admin = 888;
        set_entity_admin(1, admin, pallet_entity_common::AdminPermission::COMMISSION_MANAGE);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), admin, 10_000_000).unwrap();
        assert_ok!(CommissionPoolReward::clear_pool_reward_config(
            RuntimeOrigin::signed(admin), 1,
        ));
        assert!(CommissionPoolReward::pool_reward_config(1).is_none());
    });
}

/// P2: Root force_clear 成功（含锁定绕过）
#[test]
fn p2_root_force_clear_bypasses_lock() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        set_entity_locked(1);
        assert_ok!(CommissionPoolReward::force_clear_pool_reward_config(
            RuntimeOrigin::root(), 1,
        ));
        assert!(CommissionPoolReward::pool_reward_config(1).is_none());
    });
}

/// P2: Root force_clear 无配置时静默成功（X2: 防幻影事件）
#[test]
fn p2_root_force_clear_no_config_silent() {
    new_test_ext().execute_with(|| {
        // 无配置时调用 — 不 panic、不发事件
        assert_ok!(CommissionPoolReward::force_clear_pool_reward_config(
            RuntimeOrigin::root(), 1,
        ));
    });
}

/// P2: 非 Root 不能调用 force_clear
#[test]
fn p2_force_clear_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::force_clear_pool_reward_config(RuntimeOrigin::signed(OWNER), 1),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ====================================================================
// F1: get_claimable 可领取金额预查询
// ====================================================================

#[test]
fn f1_get_claimable_basic() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 5);
        set_level_count(entity_id, 2, 2);
        set_pool_balance(entity_id, 10_000);

        let (nex, token) = CommissionPoolReward::get_claimable(entity_id, &10);
        // level 1 = 50% of 10000 / 5 members = 1000
        assert_eq!(nex, 1000);
        assert_eq!(token, 0);
    });
}

#[test]
fn f1_get_claimable_returns_zero_when_paused() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 5);
        set_pool_balance(entity_id, 10_000);

        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));
        let (nex, _) = CommissionPoolReward::get_claimable(entity_id, &10);
        assert_eq!(nex, 0);
    });
}

#[test]
fn f1_get_claimable_returns_zero_when_global_paused() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 5);
        set_pool_balance(entity_id, 10_000);

        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true));
        let (nex, _) = CommissionPoolReward::get_claimable(entity_id, &10);
        assert_eq!(nex, 0);
    });
}

#[test]
fn f1_get_claimable_returns_zero_after_claim() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // Before claim, should be claimable
        let (nex_before, _) = CommissionPoolReward::get_claimable(entity_id, &10);
        assert!(nex_before > 0);

        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        // After claim, should be 0 for current round
        let (nex_after, _) = CommissionPoolReward::get_claimable(entity_id, &10);
        assert_eq!(nex_after, 0);
    });
}

#[test]
fn f1_get_claimable_non_member_returns_zero() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_pool_balance(entity_id, 10_000);
        // account 99 is not a member
        let (nex, token) = CommissionPoolReward::get_claimable(entity_id, &99);
        assert_eq!(nex, 0);
        assert_eq!(token, 0);
    });
}

// ====================================================================
// F3: 暂停/恢复分配 pause/resume
// ====================================================================

#[test]
fn f3_pause_pool_reward_works() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), 1));
        assert!(pallet::PoolRewardPaused::<Test>::get(1));

        // Check event
        System::assert_has_event(RuntimeEvent::CommissionPoolReward(
            pallet::Event::PoolRewardPausedEvent { entity_id: 1 }
        ));
    });
}

#[test]
fn f3_resume_pool_reward_works() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), 1));
        assert_ok!(CommissionPoolReward::resume_pool_reward(RuntimeOrigin::signed(OWNER), 1));
        assert!(!pallet::PoolRewardPaused::<Test>::get(1));

        System::assert_has_event(RuntimeEvent::CommissionPoolReward(
            pallet::Event::PoolRewardResumedEvent { entity_id: 1 }
        ));
    });
}

#[test]
fn f3_pause_rejects_already_paused() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), 1));
        assert_noop!(
            CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::PoolRewardIsPaused
        );
    });
}

#[test]
fn f3_resume_rejects_not_paused() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::resume_pool_reward(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::PoolRewardNotPaused
        );
    });
}

#[test]
fn f3_pause_rejects_no_config() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::ConfigNotFound
        );
    });
}

#[test]
fn f3_pause_rejects_locked_entity() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        set_entity_locked(1);
        assert_noop!(
            CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn f3_pause_rejects_unauthorized() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(42), 1),
            pallet::Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn f3_claim_blocked_when_paused() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id),
            pallet::Error::<Test>::PoolRewardIsPaused
        );
    });
}

#[test]
fn f3_claim_works_after_resume() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));
        assert_ok!(CommissionPoolReward::resume_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));
    });
}

// ====================================================================
// F4: MinRoundDuration 最小轮次间隔校验
// ====================================================================

#[test]
fn f4_set_config_rejects_duration_below_min() {
    new_test_ext().execute_with(|| {
        // MinRoundDuration = 10, so duration=9 should fail
        let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
            vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
        assert_noop!(
            CommissionPoolReward::set_pool_reward_config(
                RuntimeOrigin::signed(OWNER), 1, ratios, 9,
            ),
            pallet::Error::<Test>::RoundDurationTooShort
        );
    });
}

#[test]
fn f4_set_config_accepts_duration_at_min() {
    new_test_ext().execute_with(|| {
        let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
            vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), 1, ratios, 10,
        ));
    });
}

#[test]
fn f4_force_set_config_rejects_duration_below_min() {
    new_test_ext().execute_with(|| {
        let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
            vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
        assert_noop!(
            CommissionPoolReward::force_set_pool_reward_config(
                RuntimeOrigin::root(), 1, ratios, 5,
            ),
            pallet::Error::<Test>::RoundDurationTooShort
        );
    });
}

// ====================================================================
// F5: get_round_statistics 轮次领取进度查询
// ====================================================================

#[test]
fn f5_get_round_statistics_none_without_round() {
    new_test_ext().execute_with(|| {
        assert!(CommissionPoolReward::get_round_statistics(1).is_none());
    });
}

#[test]
fn f5_get_round_statistics_shows_progress() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_member(entity_id, 20, 1);
        set_level_count(entity_id, 1, 2);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // Trigger round creation via claim
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        let stats = CommissionPoolReward::get_round_statistics(entity_id).unwrap();
        // Level 1: 2 members, 1 claimed
        let level1 = stats.iter().find(|s| s.0 == 1).unwrap();
        assert_eq!(level1.1, 2); // member_count
        assert_eq!(level1.2, 1); // claimed_count
        assert!(level1.3 > 0);  // per_member_reward > 0
    });
}

// ====================================================================
// F8: 全局紧急暂停 GlobalPoolRewardPaused
// ====================================================================

#[test]
fn f8_global_pause_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true));
        assert!(pallet::GlobalPoolRewardPaused::<Test>::get());

        System::assert_has_event(RuntimeEvent::CommissionPoolReward(
            pallet::Event::GlobalPoolRewardPausedEvent
        ));
    });
}

#[test]
fn f8_global_resume_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true));
        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), false));
        assert!(!pallet::GlobalPoolRewardPaused::<Test>::get());

        System::assert_has_event(RuntimeEvent::CommissionPoolReward(
            pallet::Event::GlobalPoolRewardResumedEvent
        ));
    });
}

#[test]
fn f8_global_pause_rejects_already_paused() {
    new_test_ext().execute_with(|| {
        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true));
        assert_noop!(
            CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true),
            pallet::Error::<Test>::GlobalPaused
        );
    });
}

#[test]
fn f8_global_resume_rejects_not_paused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), false),
            pallet::Error::<Test>::GlobalNotPaused
        );
    });
}

#[test]
fn f8_global_pause_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::signed(OWNER), true),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn f8_claim_blocked_when_global_paused() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true));

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id),
            pallet::Error::<Test>::GlobalPaused
        );
    });
}

// ====================================================================
// F9: 累计分配统计 TotalDistributed
// ====================================================================

#[test]
fn f9_distribution_stats_updated_on_claim() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        let stats = pallet::DistributionStatistics::<Test>::get(entity_id);
        assert_eq!(stats.total_claims, 1);
        assert!(stats.total_nex_distributed > 0);
        assert_eq!(stats.total_token_distributed, 0);
    });
}

#[test]
fn f9_distribution_stats_accumulate() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_member(entity_id, 20, 2);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 20, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(20), entity_id));

        let stats = pallet::DistributionStatistics::<Test>::get(entity_id);
        assert_eq!(stats.total_claims, 2);
        // level1: 50% of 10000 / 1 = 5000, level2: 50% of 10000 / 1 = 5000
        assert_eq!(stats.total_nex_distributed, 10_000);
    });
}

#[test]
fn f9_rounds_completed_increments_on_new_round() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // First claim creates round 1
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        // Advance past round duration and claim again (creates round 2, archives round 1)
        System::set_block_number(200);
        set_pool_balance(entity_id, 10_000);
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        let stats = pallet::DistributionStatistics::<Test>::get(entity_id);
        assert_eq!(stats.total_rounds_completed, 1); // round 1 completed
        assert_eq!(stats.total_claims, 2);
    });
}

// ====================================================================
// F10: 轮次历史存储 RoundHistory
// ====================================================================

#[test]
fn f10_round_history_archived_on_new_round() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // Create round 1 via claim
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        // Advance and create round 2
        System::set_block_number(200);
        set_pool_balance(entity_id, 10_000);
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        let history = pallet::RoundHistory::<Test>::get(entity_id);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].round_id, 1);
        assert_eq!(history[0].start_block, 1);
        assert_eq!(history[0].end_block, 200);
    });
}

#[test]
fn f10_round_history_evicts_oldest_when_full() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // MaxRoundHistory = 5, create 7 rounds to trigger 2 evictions
        for i in 0..7u64 {
            set_pool_balance(entity_id, 10_000);
            System::set_block_number(1 + i * 200);
            assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));
        }

        let history = pallet::RoundHistory::<Test>::get(entity_id);
        assert_eq!(history.len(), 5); // capped at MaxRoundHistory
        // 7 claims → 6 archives (rounds 1-6). Round 1 evicted → oldest is round 2
        assert_eq!(history[0].round_id, 2);
        assert_eq!(history[4].round_id, 6);
    });
}

#[test]
fn f10_round_archived_event_emitted() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        System::set_block_number(200);
        set_pool_balance(entity_id, 10_000);
        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        System::assert_has_event(RuntimeEvent::CommissionPoolReward(
            pallet::Event::RoundArchived { entity_id, round_id: 1 }
        ));
    });
}

// ====================================================================
// F11: NewRoundDetails 事件信息扩展
// ====================================================================

#[test]
fn f11_new_round_details_event_emitted() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 3);
        set_level_count(entity_id, 2, 2);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));

        // Check that NewRoundDetails was emitted with correct level info
        let events: Vec<_> = System::events().into_iter()
            .filter_map(|e| {
                if let RuntimeEvent::CommissionPoolReward(pallet::Event::NewRoundDetails {
                    entity_id: eid, round_id, pool_snapshot, level_snapshots, ..
                }) = e.event {
                    Some((eid, round_id, pool_snapshot, level_snapshots))
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(events.len(), 1);
        let (eid, rid, pool, levels) = &events[0];
        assert_eq!(*eid, entity_id);
        assert_eq!(*rid, 1);
        assert_eq!(*pool, 10_000);
        assert_eq!(levels.len(), 2);
        // level 1: 3 members, per_member = 50% * 10000 / 3 = 1666
        let l1 = levels.iter().find(|l| l.0 == 1).unwrap();
        assert_eq!(l1.1, 3); // member_count
        assert_eq!(l1.2, 1666); // per_member_reward
    });
}

// ====================================================================
// F3/F8 interaction: per-entity pause + global pause
// ====================================================================

#[test]
fn f3_f8_global_pause_overrides_entity_resume() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // Entity is not paused, but global is
        assert_ok!(CommissionPoolReward::set_global_pool_reward_paused(RuntimeOrigin::root(), true));

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id),
            pallet::Error::<Test>::GlobalPaused
        );
    });
}

// ====================================================================
// PlanWriter clear_config cleans new storage
// ====================================================================

#[test]
fn plan_writer_clear_config_cleans_new_storage() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        // Set some new storage items
        pallet::PoolRewardPaused::<Test>::insert(entity_id, true);
        pallet::DistributionStatistics::<Test>::insert(entity_id, pallet::DistributionStats {
            total_nex_distributed: 1000u128,
            total_token_distributed: 500u128,
            total_rounds_completed: 5,
            total_claims: 10,
        });

        // clear via PlanWriter
        use pallet_commission_common::PoolRewardPlanWriter;
        assert_ok!(CommissionPoolReward::clear_config(entity_id));

        assert!(!pallet::PoolRewardPaused::<Test>::contains_key(entity_id));
        assert_eq!(pallet::DistributionStatistics::<Test>::get(entity_id).total_claims, 0);
        assert_eq!(pallet::RoundHistory::<Test>::get(entity_id).len(), 0);
    });
}

// ====================================================================
// R7 审计回归测试
// ====================================================================

/// M1-R7: clear_pool_reward_config 清除 PoolRewardPaused，防止 re-create 后残留
#[test]
fn m1_r7_clear_config_cleans_paused_state() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        // Pause the entity
        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));
        assert!(pallet::PoolRewardPaused::<Test>::get(entity_id));

        // Clear config — should also clear paused state
        assert_ok!(CommissionPoolReward::clear_pool_reward_config(RuntimeOrigin::signed(OWNER), entity_id));
        assert!(!pallet::PoolRewardPaused::<Test>::contains_key(entity_id));

        // Re-create config — should NOT be paused
        let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
            vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, ratios, 100,
        ));
        assert!(!pallet::PoolRewardPaused::<Test>::get(entity_id));
    });
}

/// M1-R7: force_clear_pool_reward_config 清除 PoolRewardPaused
#[test]
fn m1_r7_force_clear_config_cleans_paused_state() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        // Pause the entity
        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));
        assert!(pallet::PoolRewardPaused::<Test>::get(entity_id));

        // Force clear config — should also clear paused state
        assert_ok!(CommissionPoolReward::force_clear_pool_reward_config(RuntimeOrigin::root(), entity_id));
        assert!(!pallet::PoolRewardPaused::<Test>::contains_key(entity_id));
    });
}

/// M1-R7: clear 后 re-create 不会继承旧的暂停状态，用户可正常 claim
#[test]
fn m1_r7_recreate_after_clear_allows_claim() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        // Pause → clear → re-create
        assert_ok!(CommissionPoolReward::pause_pool_reward(RuntimeOrigin::signed(OWNER), entity_id));
        assert_ok!(CommissionPoolReward::clear_pool_reward_config(RuntimeOrigin::signed(OWNER), entity_id));

        let ratios: frame_support::BoundedVec<(u8, u16), ConstU32<10>> =
            vec![(1u8, 5000u16), (2, 5000)].try_into().unwrap();
        assert_ok!(CommissionPoolReward::set_pool_reward_config(
            RuntimeOrigin::signed(OWNER), entity_id, ratios, 100,
        ));

        // User should be able to claim (not blocked by orphaned pause)
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        assert_ok!(CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id));
    });
}

/// M2-R7: 新增 weight 函数值合理性检查
#[test]
fn m2_r7_new_weight_values_are_reasonable() {
    use crate::weights::{WeightInfo, SubstrateWeight};

    let w_pause = SubstrateWeight::pause_pool_reward();
    assert!(w_pause.ref_time() >= 25_000_000, "pause_pool_reward ref_time too low");
    assert!(w_pause.proof_size() >= 3_000, "pause_pool_reward proof_size too low");
    // Should be lighter than set_pool_reward_config
    let w_set = SubstrateWeight::set_pool_reward_config();
    assert!(w_pause.ref_time() < w_set.ref_time(), "pause should be lighter than set_config");

    let w_resume = SubstrateWeight::resume_pool_reward();
    assert!(w_resume.ref_time() >= 25_000_000, "resume_pool_reward ref_time too low");
    assert!(w_resume.ref_time() < w_set.ref_time(), "resume should be lighter than set_config");

    let w_global = SubstrateWeight::set_global_pool_reward_paused();
    assert!(w_global.ref_time() >= 10_000_000, "set_global_paused ref_time too low");
    assert!(w_global.ref_time() < w_pause.ref_time(), "global_pause should be lighter than entity pause");
}

// ====================================================================
// 审计 R8: M1 — 封禁/冻结会员不可领取池奖励
// ====================================================================

#[test]
fn m1_r8_banned_member_cannot_claim_pool_reward() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // 封禁会员
        ban_member(entity_id, 10);

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id),
            pallet::Error::<Test>::NotMember
        );
    });
}

#[test]
fn m1_r8_frozen_member_cannot_claim_pool_reward() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 10, 100_000).unwrap();

        // 冻结会员
        freeze_member(entity_id, 10);

        assert_noop!(
            CommissionPoolReward::claim_pool_reward(RuntimeOrigin::signed(10), entity_id),
            pallet::Error::<Test>::NotMember
        );
    });
}

#[test]
fn m1_r8_get_claimable_returns_zero_for_banned_member() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // 正常时可领取
        let (nex, _) = pallet::Pallet::<Test>::get_claimable(entity_id, &10);
        assert!(nex > 0, "should be claimable before ban");

        // 封禁后返 0
        ban_member(entity_id, 10);
        let (nex, token) = pallet::Pallet::<Test>::get_claimable(entity_id, &10);
        assert_eq!(nex, 0);
        assert_eq!(token, 0);
    });
}

#[test]
fn m1_r8_get_claimable_returns_zero_for_frozen_member() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        set_member(entity_id, 10, 1);
        set_level_count(entity_id, 1, 1);
        set_level_count(entity_id, 2, 1);
        set_pool_balance(entity_id, 10_000);

        // 正常时可领取
        let (nex, _) = pallet::Pallet::<Test>::get_claimable(entity_id, &10);
        assert!(nex > 0, "should be claimable before freeze");

        // 冻结后返 0
        freeze_member(entity_id, 10);
        let (nex, token) = pallet::Pallet::<Test>::get_claimable(entity_id, &10);
        assert_eq!(nex, 0);
        assert_eq!(token, 0);
    });
}
