use crate as pallet_entity_token;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU32, ConstU64},
};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// ==================== 构建 Mock Runtime ====================

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        EntityToken: pallet_entity_token,
    }
);

// ==================== frame_system ====================

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = sp_core::H256;
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
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

// ==================== pallet_balances ====================

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type RuntimeFreezeReason = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

frame_support::parameter_types! {
    pub const AssetDeposit: u128 = 0;
    pub const AssetAccountDeposit: u128 = 0;
    pub const ApprovalDeposit: u128 = 0;
    pub const MetadataDepositBase: u128 = 0;
    pub const MetadataDepositPerByte: u128 = 0;
    pub const StringLimit: u32 = 50;
}

// ==================== pallet_assets ====================

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type AssetId = u64;
    type AssetIdParameter = u64;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<u64>>;
    type ForceOrigin = frame_system::EnsureRoot<u64>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = StringLimit;
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = ();
    type RemoveItemsLimit = ConstU32<1000>;
    type Holder = ();
    type ReserveData = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

// ==================== Mock EntityProvider ====================

use pallet_entity_common::{
    EntityProvider as EntityProviderTrait,
    EntityStatus,
};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// (shop_id) -> (owner, active)
    static SHOPS: RefCell<HashMap<u64, (u64, bool)>> = RefCell::new(HashMap::new());
    /// (account) -> kyc_level
    static KYC_LEVELS: RefCell<HashMap<u64, u8>> = RefCell::new(HashMap::new());
    /// (entity_id, account) -> is_member
    static MEMBERS: RefCell<HashMap<(u64, u64), bool>> = RefCell::new(HashMap::new());
}

pub struct MockEntityProvider;
impl EntityProviderTrait<u64> for MockEntityProvider {
    fn entity_exists(entity_id: u64) -> bool {
        SHOPS.with(|s| s.borrow().contains_key(&entity_id))
    }
    fn is_entity_active(entity_id: u64) -> bool {
        SHOPS.with(|s| s.borrow().get(&entity_id).map(|(_, a)| *a).unwrap_or(false))
    }
    fn entity_status(_entity_id: u64) -> Option<EntityStatus> {
        Some(EntityStatus::Active)
    }
    fn entity_owner(entity_id: u64) -> Option<u64> {
        SHOPS.with(|s| s.borrow().get(&entity_id).map(|(o, _)| *o))
    }
    fn entity_account(_entity_id: u64) -> u64 { 999 }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
    fn update_entity_rating(_: u64, _: u8) -> Result<(), sp_runtime::DispatchError> { Ok(()) }
}

// ==================== Mock KYC & Member Providers ====================

pub struct MockKycProvider;
impl pallet_entity_token::KycLevelProvider<u64> for MockKycProvider {
    fn get_kyc_level(who: &u64) -> u8 {
        KYC_LEVELS.with(|k| k.borrow().get(who).copied().unwrap_or(0))
    }
    fn meets_kyc_requirement(who: &u64, min_level: u8) -> bool {
        Self::get_kyc_level(who) >= min_level
    }
}

pub struct MockMemberProvider;
impl pallet_entity_token::EntityMemberProvider<u64> for MockMemberProvider {
    fn is_member(entity_id: u64, who: &u64) -> bool {
        MEMBERS.with(|m| m.borrow().get(&(entity_id, *who)).copied().unwrap_or(false))
    }
}

// ==================== pallet_entity_token ====================

parameter_types! {
    pub const ShopTokenOffset: u64 = 1_000_000;
}

impl pallet_entity_token::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AssetId = u64;
    type AssetBalance = u128;
    type Assets = Assets;
    type EntityProvider = MockEntityProvider;
    type ShopTokenOffset = ShopTokenOffset;
    type MaxTokenNameLength = ConstU32<64>;
    type MaxTokenSymbolLength = ConstU32<8>;
    type MaxTransferListSize = ConstU32<100>;
    type MaxDividendRecipients = ConstU32<50>;
    type KycProvider = MockKycProvider;
    type MemberProvider = MockMemberProvider;
    type WeightInfo = ();
}

// ==================== 工具函数 ====================

/// 注册一个活跃店铺
pub fn register_shop(shop_id: u64, owner: u64) {
    SHOPS.with(|s| s.borrow_mut().insert(shop_id, (owner, true)));
}

/// 注册一个非活跃店铺
pub fn register_inactive_shop(shop_id: u64, owner: u64) {
    SHOPS.with(|s| s.borrow_mut().insert(shop_id, (owner, false)));
}

/// 停用实体（将 active 设为 false）
pub fn deactivate_entity(entity_id: u64) {
    SHOPS.with(|s| {
        if let Some(entry) = s.borrow_mut().get_mut(&entity_id) {
            entry.1 = false;
        }
    });
}

/// 设置 KYC 级别
pub fn set_kyc_level(who: u64, level: u8) {
    KYC_LEVELS.with(|k| k.borrow_mut().insert(who, level));
}

/// 设置成员资格
pub fn set_member(entity_id: u64, who: u64, is_member: bool) {
    MEMBERS.with(|m| m.borrow_mut().insert((entity_id, who), is_member));
}

// ==================== Test Externalities Builder ====================

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 10_000), (2, 10_000), (3, 10_000), (4, 10_000), (5, 10_000)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

/// 推进区块号
pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::set_block_number(System::block_number() + 1);
    }
}

