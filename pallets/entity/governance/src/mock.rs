use crate as pallet_entity_governance;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU16, ConstU32, ConstU64},
};
use frame_system as system;
use pallet_entity_common::{
    EntityProvider, EntityStatus, EntityTokenProvider, MemberMode,
    ShopProvider, ShopType, TokenType,
};
use pallet_entity_commission::{NullCommissionProvider, NullMemberProvider};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, DispatchError,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        EntityGovernance: pallet_entity_governance,
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

type ConstU128<const N: u128> = frame_support::traits::ConstU128<N>;

// ==================== Mock EntityProvider ====================

pub struct MockEntityProvider;
impl EntityProvider<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        entity_id == 1 || entity_id == 2
    }
    fn is_entity_active(entity_id: u64) -> bool {
        entity_id == 1 || entity_id == 2
    }
    fn entity_status(entity_id: u64) -> Option<EntityStatus> {
        if entity_id <= 2 { Some(EntityStatus::Active) } else { None }
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        match entity_id {
            1 => Some(OWNER),
            2 => Some(OWNER_2),
            _ => None,
        }
    }
    fn entity_account(entity_id: u64) -> u64 {
        100 + entity_id
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn update_entity_rating(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
}

// ==================== Mock ShopProvider ====================

pub struct MockShopProvider;
impl ShopProvider<u64> for MockShopProvider {
    fn shop_exists(shop_id: u64) -> bool { shop_id == SHOP_ID || shop_id == SHOP_ID_2 }
    fn is_shop_active(shop_id: u64) -> bool { shop_id == SHOP_ID || shop_id == SHOP_ID_2 }
    fn shop_entity_id(shop_id: u64) -> Option<u64> {
        match shop_id {
            SHOP_ID => Some(1),
            SHOP_ID_2 => Some(2),
            _ => None,
        }
    }
    fn shop_owner(shop_id: u64) -> Option<u64> {
        match shop_id {
            SHOP_ID => Some(OWNER),
            SHOP_ID_2 => Some(OWNER_2),
            _ => None,
        }
    }
    fn shop_account(shop_id: u64) -> u64 { 200 + shop_id }
    fn shop_type(_: u64) -> Option<ShopType> { Some(ShopType::OnlineStore) }
    fn shop_member_mode(_: u64) -> MemberMode { MemberMode::Inherit }
    fn is_shop_manager(shop_id: u64, account: &u64) -> bool {
        Self::shop_owner(shop_id) == Some(*account)
    }
    fn update_shop_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn update_shop_rating(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn deduct_operating_fund(_: u64, _: u128) -> Result<(), DispatchError> { Ok(()) }
    fn operating_balance(_: u64) -> u128 { 1_000_000 }
}

// ==================== Mock TokenProvider ====================

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static TOKEN_BALANCES: RefCell<HashMap<(u64, u64), u128>> = RefCell::new(HashMap::new());
    static TOKEN_ENABLED: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
}

pub fn set_token_balance(shop_id: u64, who: u64, amount: u128) {
    TOKEN_BALANCES.with(|b| b.borrow_mut().insert((shop_id, who), amount));
}

pub fn set_token_enabled(shop_id: u64, enabled: bool) {
    TOKEN_ENABLED.with(|e| e.borrow_mut().insert(shop_id, enabled));
}

pub fn get_token_balance(shop_id: u64, who: u64) -> u128 {
    TOKEN_BALANCES.with(|b| *b.borrow().get(&(shop_id, who)).unwrap_or(&0))
}

pub struct MockTokenProvider;
impl EntityTokenProvider<u64, u128> for MockTokenProvider {
    fn is_token_enabled(entity_id: u64) -> bool {
        TOKEN_ENABLED.with(|e| *e.borrow().get(&entity_id).unwrap_or(&false))
    }
    fn token_balance(entity_id: u64, holder: &u64) -> u128 {
        get_token_balance(entity_id, *holder)
    }
    fn reward_on_purchase(_: u64, _: &u64, _: u128) -> Result<u128, DispatchError> { Ok(0) }
    fn redeem_for_discount(_: u64, _: &u64, _: u128) -> Result<u128, DispatchError> { Ok(0) }
    fn transfer(_: u64, _: &u64, _: &u64, _: u128) -> Result<(), DispatchError> { Ok(()) }
    fn reserve(_: u64, _: &u64, _: u128) -> Result<(), DispatchError> { Ok(()) }
    fn unreserve(_: u64, _: &u64, _: u128) -> u128 { 0 }
    fn repatriate_reserved(_: u64, _: &u64, _: &u64, _: u128) -> Result<u128, DispatchError> { Ok(0) }
    fn get_token_type(_: u64) -> TokenType { TokenType::Governance }
    fn total_supply(_: u64) -> u128 { TOTAL_SUPPLY }
}

// ==================== 常量 ====================

pub const OWNER: u64 = 1;
pub const OWNER_2: u64 = 2;
pub const ALICE: u64 = 10;
pub const BOB: u64 = 11;
pub const CHARLIE: u64 = 12;
pub const SHOP_ID: u64 = 1;
pub const SHOP_ID_2: u64 = 2;
pub const TOTAL_SUPPLY: u128 = 1_000_000;

parameter_types! {
    pub const VotingPeriod: u64 = 100;
    pub const ExecutionDelay: u64 = 50;
    pub const PassThreshold: u8 = 50;
    pub const QuorumThreshold: u8 = 10;
    pub const MinProposalThreshold: u16 = 100; // 1%
    pub const TimeWeightFullPeriod: u64 = 1000; // 1000 blocks to reach max multiplier
    pub const TimeWeightMaxMultiplier: u32 = 30000; // 3x max voting power
}

impl pallet_entity_governance::Config for Test {
    type Balance = u128;
    type EntityProvider = MockEntityProvider;
    type ShopProvider = MockShopProvider;
    type TokenProvider = MockTokenProvider;
    type CommissionProvider = NullCommissionProvider;
    type MemberProvider = NullMemberProvider;
    type VotingPeriod = VotingPeriod;
    type ExecutionDelay = ExecutionDelay;
    type PassThreshold = PassThreshold;
    type QuorumThreshold = QuorumThreshold;
    type MinProposalThreshold = MinProposalThreshold;
    type MaxTitleLength = ConstU32<128>;
    type MaxCidLength = ConstU32<64>;
    type MaxActiveProposals = ConstU32<10>;
    type MaxCommitteeSize = ConstU32<10>;
    type TimeWeightFullPeriod = TimeWeightFullPeriod;
    type TimeWeightMaxMultiplier = TimeWeightMaxMultiplier;
}

// ==================== 构建器 ====================

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (OWNER, 100_000_000),
                (OWNER_2, 100_000_000),
                (ALICE, 100_000_000),
                (BOB, 100_000_000),
                (CHARLIE, 100_000_000),
            ],
            dev_accounts: None,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
            set_token_enabled(SHOP_ID, true);
            set_token_enabled(SHOP_ID_2, true);
            // Alice: 2% (20000), Bob: 15% (150000), Charlie: 5% (50000)
            set_token_balance(SHOP_ID, ALICE, 20_000);
            set_token_balance(SHOP_ID, BOB, 150_000);
            set_token_balance(SHOP_ID, CHARLIE, 50_000);
            set_token_balance(SHOP_ID, OWNER, 100_000);
        });
        ext
    }
}

pub fn advance_blocks(n: u64) {
    let current = System::block_number();
    System::set_block_number(current + n);
}
