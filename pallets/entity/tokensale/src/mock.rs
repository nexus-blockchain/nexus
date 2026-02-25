//! 代币发售模块测试 mock

use crate as pallet_entity_tokensale;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use sp_runtime::BuildStorage;
use frame_support::traits::Currency;
use frame_support::sp_runtime::DispatchError;

type Block = frame_system::mocking::MockBlock<Test>;

pub const CREATOR: u64 = 1;
pub const BUYER: u64 = 2;
pub const BUYER2: u64 = 3;
pub const ENTITY_ID: u64 = 100;
pub const ENTITY_ACCOUNT: u64 = 1000;
pub const INITIAL_BALANCE: u128 = 1_000_000_000;
pub const TOKEN_SUPPLY: u128 = 10_000_000;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityTokenSale: pallet_entity_tokensale,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = u128;
}

// ==================== Mock EntityProvider ====================

pub struct MockEntityProvider;

impl pallet_entity_common::EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        entity_id == ENTITY_ID
    }
    fn is_entity_active(entity_id: u64) -> bool {
        entity_id == ENTITY_ID
    }
    fn entity_status(_entity_id: u64) -> Option<pallet_entity_common::EntityStatus> {
        Some(pallet_entity_common::EntityStatus::Active)
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        if entity_id == ENTITY_ID { Some(CREATOR) } else { None }
    }
    fn entity_account(entity_id: u64) -> u64 {
        if entity_id == ENTITY_ID { ENTITY_ACCOUNT } else { 0 }
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn update_entity_rating(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn is_entity_admin(entity_id: u64, account: &u64) -> bool {
        entity_id == ENTITY_ID && *account == CREATOR
    }
}

// ==================== Mock TokenProvider ====================

use core::cell::RefCell;
use alloc::collections::BTreeMap;

extern crate alloc;

thread_local! {
    static TOKEN_BALANCES: RefCell<BTreeMap<(u64, u64), u128>> = RefCell::new(BTreeMap::new());
    static TOKEN_RESERVED: RefCell<BTreeMap<(u64, u64), u128>> = RefCell::new(BTreeMap::new());
}

pub struct MockTokenProvider;

impl MockTokenProvider {
    pub fn set_balance(entity_id: u64, account: u64, amount: u128) {
        TOKEN_BALANCES.with(|b| b.borrow_mut().insert((entity_id, account), amount));
    }
}

impl pallet_entity_common::EntityTokenProvider<u64, u128> for MockTokenProvider {
    fn is_token_enabled(_entity_id: u64) -> bool { true }
    fn token_balance(entity_id: u64, holder: &u64) -> u128 {
        TOKEN_BALANCES.with(|b| *b.borrow().get(&(entity_id, *holder)).unwrap_or(&0))
    }
    fn reward_on_purchase(_: u64, _: &u64, _: u128) -> Result<u128, DispatchError> { Ok(0) }
    fn redeem_for_discount(_: u64, _: &u64, _: u128) -> Result<u128, DispatchError> { Ok(0) }
    fn transfer(_: u64, _: &u64, _: &u64, _: u128) -> Result<(), DispatchError> { Ok(()) }
    fn reserve(entity_id: u64, who: &u64, amount: u128) -> Result<(), DispatchError> {
        TOKEN_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let bal = map.entry((entity_id, *who)).or_insert(0);
            if *bal < amount {
                return Err(DispatchError::Other("InsufficientBalance"));
            }
            *bal -= amount;
            Ok(())
        })?;
        TOKEN_RESERVED.with(|r| {
            let mut map = r.borrow_mut();
            let reserved = map.entry((entity_id, *who)).or_insert(0);
            *reserved += amount;
        });
        Ok(())
    }
    fn unreserve(entity_id: u64, who: &u64, amount: u128) -> u128 {
        TOKEN_RESERVED.with(|r| {
            let mut map = r.borrow_mut();
            let reserved = map.entry((entity_id, *who)).or_insert(0);
            let actual = amount.min(*reserved);
            *reserved -= actual;
            TOKEN_BALANCES.with(|b| {
                let mut bmap = b.borrow_mut();
                let bal = bmap.entry((entity_id, *who)).or_insert(0);
                *bal += actual;
            });
            amount - actual
        })
    }
    fn repatriate_reserved(entity_id: u64, from: &u64, to: &u64, amount: u128) -> Result<u128, DispatchError> {
        TOKEN_RESERVED.with(|r| {
            let mut map = r.borrow_mut();
            let reserved = map.entry((entity_id, *from)).or_insert(0);
            let actual = amount.min(*reserved);
            *reserved -= actual;
            TOKEN_BALANCES.with(|b| {
                let mut bmap = b.borrow_mut();
                let bal = bmap.entry((entity_id, *to)).or_insert(0);
                *bal += actual;
            });
            Ok(amount - actual)
        })
    }
    fn get_token_type(_: u64) -> pallet_entity_common::TokenType { Default::default() }
    fn total_supply(_: u64) -> u128 { TOKEN_SUPPLY }
}

// ==================== Mock KycChecker ====================

thread_local! {
    static KYC_LEVELS: RefCell<BTreeMap<u64, u8>> = RefCell::new(BTreeMap::new());
}

pub struct MockKycChecker;

impl MockKycChecker {
    pub fn set_level(account: u64, level: u8) {
        KYC_LEVELS.with(|k| k.borrow_mut().insert(account, level));
    }
}

impl pallet_entity_tokensale::KycChecker<u64> for MockKycChecker {
    fn kyc_level(account: &u64) -> u8 {
        KYC_LEVELS.with(|k| *k.borrow().get(account).unwrap_or(&0))
    }
}

// ==================== Pallet Config ====================

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

parameter_types! {
    pub const RefundGracePeriod: u64 = 100; // 100 blocks grace period for tests
}

impl pallet_entity_tokensale::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type AssetId = u64;
    type EntityProvider = MockEntityProvider;
    type TokenProvider = MockTokenProvider;
    type KycChecker = MockKycChecker;
    type MaxPaymentOptions = ConstU32<5>;
    type MaxWhitelistSize = ConstU32<100>;
    type MaxRoundsHistory = ConstU32<50>;
    type MaxSubscriptionsPerRound = ConstU32<1000>;
    type MaxActiveRounds = ConstU32<10>;
    type RefundGracePeriod = RefundGracePeriod;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (CREATOR, INITIAL_BALANCE),
            (BUYER, INITIAL_BALANCE),
            (BUYER2, INITIAL_BALANCE),
            (ENTITY_ACCOUNT, INITIAL_BALANCE),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        // 设置 Entity 代币余额
        MockTokenProvider::set_balance(ENTITY_ID, ENTITY_ACCOUNT, TOKEN_SUPPLY);
    });
    ext
}
