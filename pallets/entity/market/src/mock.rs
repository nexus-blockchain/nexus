use crate as pallet_entity_market;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU16, ConstU32, ConstU64},
};
use frame_system as system;
use pallet_entity_common::{
    EntityProvider, EntityStatus, EntityTokenProvider, TokenType,
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, DispatchError,
};
use sp_std::vec::Vec;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityMarket: pallet_entity_market,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type WeightInfo = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
}

parameter_types! {
    pub const ConstU128One: u128 = 1;
}

type ConstU128<const N: u128> = frame_support::traits::ConstU128<N>;

// ==================== Mock EntityProvider ====================

pub struct MockEntityProvider;
impl EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        entity_id == 1 || entity_id == 2
    }
    fn is_entity_active(entity_id: u64) -> bool {
        ENTITY_ACTIVE.with(|e| *e.borrow().get(&entity_id).unwrap_or(&(entity_id == 1 || entity_id == 2)))
    }
    fn entity_status(entity_id: u64) -> Option<EntityStatus> {
        if entity_id == 1 || entity_id == 2 {
            Some(EntityStatus::Active)
        } else {
            None
        }
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        match entity_id {
            1 => Some(ENTITY_OWNER),
            2 => Some(ENTITY_OWNER_2),
            _ => None,
        }
    }
    fn entity_account(entity_id: u64) -> u64 {
        100 + entity_id
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> {
        Ok(())
    }
    fn update_entity_rating(_: u64, _: u8) -> Result<(), DispatchError> {
        Ok(())
    }
}

// ==================== Mock EntityTokenProvider ====================

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static TOKEN_BALANCES: RefCell<HashMap<(u64, u64), u128>> = RefCell::new(HashMap::new());
    static TOKEN_RESERVED: RefCell<HashMap<(u64, u64), u128>> = RefCell::new(HashMap::new());
    static TOKEN_ENABLED: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
    static ENTITY_ACTIVE: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
}

pub fn set_entity_active(entity_id: u64, active: bool) {
    ENTITY_ACTIVE.with(|e| e.borrow_mut().insert(entity_id, active));
}

pub fn set_token_balance(entity_id: u64, who: u64, amount: u128) {
    TOKEN_BALANCES.with(|b| b.borrow_mut().insert((entity_id, who), amount));
}

pub fn set_token_enabled(entity_id: u64, enabled: bool) {
    TOKEN_ENABLED.with(|e| e.borrow_mut().insert(entity_id, enabled));
}

pub fn get_token_balance(entity_id: u64, who: u64) -> u128 {
    TOKEN_BALANCES.with(|b| *b.borrow().get(&(entity_id, who)).unwrap_or(&0))
}

pub fn get_token_reserved(entity_id: u64, who: u64) -> u128 {
    TOKEN_RESERVED.with(|b| *b.borrow().get(&(entity_id, who)).unwrap_or(&0))
}

pub struct MockTokenProvider;
impl EntityTokenProvider<u64, u128> for MockTokenProvider {
    fn is_token_enabled(entity_id: u64) -> bool {
        TOKEN_ENABLED.with(|e| *e.borrow().get(&entity_id).unwrap_or(&false))
    }
    fn token_balance(entity_id: u64, holder: &u64) -> u128 {
        get_token_balance(entity_id, *holder)
    }
    fn reward_on_purchase(_: u64, _: &u64, _: u128) -> Result<u128, DispatchError> {
        Ok(0)
    }
    fn redeem_for_discount(_: u64, _: &u64, _: u128) -> Result<u128, DispatchError> {
        Ok(0)
    }
    fn transfer(entity_id: u64, from: &u64, to: &u64, amount: u128) -> Result<(), DispatchError> {
        let from_bal = get_token_balance(entity_id, *from);
        if from_bal < amount {
            return Err(DispatchError::Other("InsufficientTokenBalance"));
        }
        set_token_balance(entity_id, *from, from_bal - amount);
        let to_bal = get_token_balance(entity_id, *to);
        set_token_balance(entity_id, *to, to_bal + amount);
        Ok(())
    }
    fn reserve(entity_id: u64, who: &u64, amount: u128) -> Result<(), DispatchError> {
        let bal = get_token_balance(entity_id, *who);
        if bal < amount {
            return Err(DispatchError::Other("InsufficientTokenBalance"));
        }
        TOKEN_BALANCES.with(|b| b.borrow_mut().insert((entity_id, *who), bal - amount));
        let reserved = get_token_reserved(entity_id, *who);
        TOKEN_RESERVED.with(|r| r.borrow_mut().insert((entity_id, *who), reserved + amount));
        Ok(())
    }
    fn unreserve(entity_id: u64, who: &u64, amount: u128) -> u128 {
        let reserved = get_token_reserved(entity_id, *who);
        let actual = amount.min(reserved);
        TOKEN_RESERVED.with(|r| r.borrow_mut().insert((entity_id, *who), reserved - actual));
        let bal = get_token_balance(entity_id, *who);
        TOKEN_BALANCES.with(|b| b.borrow_mut().insert((entity_id, *who), bal + actual));
        actual
    }
    fn repatriate_reserved(
        entity_id: u64,
        from: &u64,
        to: &u64,
        amount: u128,
    ) -> Result<u128, DispatchError> {
        let reserved = get_token_reserved(entity_id, *from);
        let actual = amount.min(reserved);
        TOKEN_RESERVED.with(|r| r.borrow_mut().insert((entity_id, *from), reserved - actual));
        let to_bal = get_token_balance(entity_id, *to);
        TOKEN_BALANCES.with(|b| b.borrow_mut().insert((entity_id, *to), to_bal + actual));
        Ok(actual)
    }
    fn get_token_type(_: u64) -> TokenType {
        TokenType::Points
    }
    fn total_supply(_: u64) -> u128 {
        1_000_000_000
    }
}

// ==================== 测试常量 ====================

pub const ENTITY_OWNER: u64 = 1;
pub const ENTITY_OWNER_2: u64 = 2;
pub const ALICE: u64 = 10;
pub const BOB: u64 = 11;
pub const CHARLIE: u64 = 12;
pub const ENTITY_ID: u64 = 1;
pub const ENTITY_ID_2: u64 = 2;
pub const TREASURY: u64 = 99;
pub const REWARD_SOURCE: u64 = 98;

parameter_types! {
    pub const DefaultOrderTTL: u32 = 1000;
    pub const MaxActiveOrdersPerUser: u32 = 50;
    pub const DefaultFeeRate: u16 = 100; // 1%
    pub const DefaultUsdtTimeout: u32 = 300;
    pub const BlocksPerHour: u32 = 600;
    pub const BlocksPerDay: u32 = 14400;
    pub const BlocksPerWeek: u32 = 100800;
    pub const CircuitBreakerDuration: u32 = 600;
    pub const VerificationReward: u128 = 100_000; // 0.1 NEX
    pub const RewardSourceAccount: u64 = REWARD_SOURCE;
    pub const BuyerDepositRate: u16 = 1000; // 10%
    pub const MinBuyerDeposit: u128 = 10_000; // 0.01 NEX
    pub const DepositForfeitRate: u16 = 10000; // 100%
    pub const UsdtToNexRate: u64 = 1_000_000; // 1 USDT = 1 NEX
    pub const TreasuryAccountId: u64 = TREASURY;
}

impl pallet_entity_market::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Balance = u128;
    type TokenBalance = u128;
    type EntityProvider = MockEntityProvider;
    type TokenProvider = MockTokenProvider;
    type DefaultOrderTTL = DefaultOrderTTL;
    type MaxActiveOrdersPerUser = MaxActiveOrdersPerUser;
    type DefaultFeeRate = DefaultFeeRate;
    type DefaultUsdtTimeout = DefaultUsdtTimeout;
    type BlocksPerHour = BlocksPerHour;
    type BlocksPerDay = BlocksPerDay;
    type BlocksPerWeek = BlocksPerWeek;
    type CircuitBreakerDuration = CircuitBreakerDuration;
    type VerificationReward = VerificationReward;
    type RewardSource = RewardSourceAccount;
    type BuyerDepositRate = BuyerDepositRate;
    type MinBuyerDeposit = MinBuyerDeposit;
    type DepositForfeitRate = DepositForfeitRate;
    type UsdtToNexRate = UsdtToNexRate;
    type TreasuryAccount = TreasuryAccountId;
    type VerificationGracePeriod = ConstU32<600>;  // 1h 宽限期
    type UnderpaidGracePeriod = ConstU32<1200>;    // 2h 补付窗口
    type NexUsdtPrice = MockNexUsdtPrice;
}

/// Mock NEX/USDT 价格: 0.5 USDT/NEX (500_000, 精度 10^6)
pub struct MockNexUsdtPrice;
impl pallet_entity_common::PricingProvider for MockNexUsdtPrice {
    fn get_nex_usdt_price() -> u64 { 500_000 }
}

// ==================== 测试构建器 ====================

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (ENTITY_OWNER, 100_000_000_000),
                (ENTITY_OWNER_2, 100_000_000_000),
                (ALICE, 100_000_000_000),
                (BOB, 100_000_000_000),
                (CHARLIE, 100_000_000_000),
                (TREASURY, 100_000_000_000),
                (REWARD_SOURCE, 100_000_000_000),
            ],
            dev_accounts: None,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
            // 启用 Token
            set_token_enabled(ENTITY_ID, true);
            set_token_enabled(ENTITY_ID_2, true);
            // 给用户分配 Token
            set_token_balance(ENTITY_ID, ALICE, 10_000_000);
            set_token_balance(ENTITY_ID, BOB, 10_000_000);
            set_token_balance(ENTITY_ID, CHARLIE, 10_000_000);
            set_token_balance(ENTITY_ID, ENTITY_OWNER, 10_000_000);
        });
        ext
    }
}

/// 配置市场（启用 NEX + USDT）
pub fn configure_market_enabled(entity_id: u64) {
    assert!(EntityMarket::configure_market(
        RuntimeOrigin::signed(if entity_id == ENTITY_ID { ENTITY_OWNER } else { ENTITY_OWNER_2 }),
        entity_id,
        true,  // cos_enabled
        true,  // usdt_enabled
        100,   // fee_rate = 1%
        1,     // min_order_amount
        1000,  // order_ttl
        300,   // usdt_timeout
    ).is_ok());
}
