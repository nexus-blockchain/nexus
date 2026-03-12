use crate as pallet_entity_governance;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU16, ConstU32, ConstU64},
};
use frame_system as system;
use pallet_entity_common::{
    DisclosureLevel, DisclosureProvider,
    EntityProvider, EntityStatus, EntityTokenProvider,
    GovernanceMode,
    ProductProvider, ProductCategory,
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
        ENTITY_ACTIVE.with(|e| {
            *e.borrow().get(&entity_id).unwrap_or(&(entity_id == 1 || entity_id == 2))
        })
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
    fn entity_shops(entity_id: u64) -> sp_std::vec::Vec<u64> {
        match entity_id {
            1 => sp_std::vec![SHOP_ID],
            2 => sp_std::vec![SHOP_ID_2],
            _ => sp_std::vec![],
        }
    }
    fn update_entity_stats(_: u64, _: u128, _: u32) -> Result<(), DispatchError> { Ok(()) }
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
    // H2: 追踪 reserve/unreserve 调用
    static RESERVED_BALANCES: RefCell<HashMap<(u64, u64), u128>> = RefCell::new(HashMap::new());
    // F5: 可配置实体活跃状态
    static ENTITY_ACTIVE: RefCell<HashMap<u64, bool>> = RefCell::new(HashMap::new());
    // F3: Mock 商品存储
    static PRODUCT_PRICES: RefCell<HashMap<u64, u128>> = RefCell::new(HashMap::new());
    static PRODUCT_STOCKS: RefCell<HashMap<u64, u32>> = RefCell::new(HashMap::new());
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

pub fn get_reserved_balance(entity_id: u64, who: u64) -> u128 {
    RESERVED_BALANCES.with(|b| *b.borrow().get(&(entity_id, who)).unwrap_or(&0))
}

/// F5: 设置实体活跃状态
pub fn set_entity_active(entity_id: u64, active: bool) {
    ENTITY_ACTIVE.with(|e| e.borrow_mut().insert(entity_id, active));
}

/// F3: 设置商品价格
pub fn set_product_price(product_id: u64, price: u128) {
    PRODUCT_PRICES.with(|p| p.borrow_mut().insert(product_id, price));
}

/// F3: 设置商品库存
pub fn set_product_stock(product_id: u64, stock: u32) {
    PRODUCT_STOCKS.with(|p| p.borrow_mut().insert(product_id, stock));
}

pub fn get_product_price(product_id: u64) -> Option<u128> {
    PRODUCT_PRICES.with(|p| p.borrow().get(&product_id).copied())
}

pub fn get_product_stock(product_id: u64) -> Option<u32> {
    PRODUCT_STOCKS.with(|p| p.borrow().get(&product_id).copied())
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
    fn reserve(entity_id: u64, who: &u64, amount: u128) -> Result<(), DispatchError> {
        RESERVED_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let reserved = map.entry((entity_id, *who)).or_insert(0);
            let balance = get_token_balance(entity_id, *who);
            let available = balance.saturating_sub(*reserved);
            if available < amount {
                return Err(DispatchError::Other("InsufficientBalance"));
            }
            *reserved = reserved.saturating_add(amount);
            Ok(())
        })
    }
    fn unreserve(entity_id: u64, who: &u64, amount: u128) -> u128 {
        RESERVED_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            let reserved = map.entry((entity_id, *who)).or_insert(0);
            let actual = amount.min(*reserved);
            *reserved = reserved.saturating_sub(actual);
            actual
        })
    }
    fn repatriate_reserved(_: u64, _: &u64, _: &u64, _: u128) -> Result<u128, DispatchError> { Ok(0) }
    fn get_token_type(_: u64) -> TokenType { TokenType::Governance }
    fn total_supply(_: u64) -> u128 { TOTAL_SUPPLY }
    fn available_balance(entity_id: u64, holder: &u64) -> u128 {
        TOKEN_BALANCES.with(|b| *b.borrow().get(&(entity_id, *holder)).unwrap_or(&0))
    }
    fn governance_burn(entity_id: u64, amount: u128) -> Result<(), DispatchError> {
        TOKEN_BALANCES.with(|b| {
            let mut map = b.borrow_mut();
            // Burn from entity's "treasury" — use entity_id as account for simplicity
            let key = (entity_id, entity_id);
            let balance = map.get(&key).copied().unwrap_or(0);
            if balance < amount {
                return Err(DispatchError::Other("InsufficientBalance"));
            }
            map.insert(key, balance.saturating_sub(amount));
            Ok(())
        })
    }
}

// ==================== Mock DisclosureProvider ====================

pub struct MockDisclosureProvider;
impl DisclosureProvider<u64> for MockDisclosureProvider {
    fn is_in_blackout(_entity_id: u64) -> bool { false }
    fn is_insider(_entity_id: u64, _account: &u64) -> bool { false }
    fn can_insider_trade(_entity_id: u64, _account: &u64) -> bool { true }
    fn get_disclosure_level(_entity_id: u64) -> DisclosureLevel { DisclosureLevel::Basic }
    fn is_disclosure_overdue(_entity_id: u64) -> bool { false }
}

// ==================== Mock ProductProvider ====================

pub struct MockProductProvider;
impl ProductProvider<u64, u128> for MockProductProvider {
    fn product_exists(product_id: u64) -> bool {
        PRODUCT_PRICES.with(|p| p.borrow().contains_key(&product_id))
    }
    fn is_product_on_sale(product_id: u64) -> bool {
        PRODUCT_STOCKS.with(|p| p.borrow().get(&product_id).copied().unwrap_or(0) > 0)
    }
    fn product_shop_id(product_id: u64) -> Option<u64> {
        if Self::product_exists(product_id) { Some(SHOP_ID) } else { None }
    }
    fn product_price(product_id: u64) -> Option<u128> {
        get_product_price(product_id)
    }
    fn product_stock(product_id: u64) -> Option<u32> {
        get_product_stock(product_id)
    }
    fn product_category(_: u64) -> Option<ProductCategory> { Some(ProductCategory::Physical) }
    fn deduct_stock(_: u64, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn restore_stock(_: u64, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn add_sold_count(_: u64, _: u32) -> Result<(), DispatchError> { Ok(()) }
    fn update_price(product_id: u64, new_price: u128) -> Result<(), DispatchError> {
        if new_price == 0 {
            return Err(DispatchError::Other("PriceCannotBeZero"));
        }
        PRODUCT_PRICES.with(|p| p.borrow_mut().insert(product_id, new_price));
        Ok(())
    }
    fn set_inventory(product_id: u64, new_inventory: u32) -> Result<(), DispatchError> {
        PRODUCT_STOCKS.with(|p| p.borrow_mut().insert(product_id, new_inventory));
        Ok(())
    }
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
    pub const MinVotingPeriod: u64 = 10;  // C3: 最小投票期 10 blocks
    pub const MinExecutionDelay: u64 = 5; // C3: 最小执行延迟 5 blocks
    pub const MaxVotingPeriod: u64 = 10000;  // S2: 最大投票期
    pub const MaxExecutionDelay: u64 = 5000; // S2: 最大执行延迟
    pub const ProposalCooldown: u64 = 0;     // P2: disabled for existing tests
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
    type MaxDelegatorsPerDelegate = ConstU32<50>;
    type MinVotingPeriod = MinVotingPeriod;
    type MinExecutionDelay = MinExecutionDelay;
    type MaxVotingPeriod = MaxVotingPeriod;
    type MaxExecutionDelay = MaxExecutionDelay;
    type TimeWeightFullPeriod = TimeWeightFullPeriod;
    type TimeWeightMaxMultiplier = TimeWeightMaxMultiplier;
    type ProductProvider = MockProductProvider;
    type DisclosureProvider = MockDisclosureProvider;
    type MultiLevelWriter = ();
    type TeamWriter = ();
    // Phase 4.2: 领域治理执行 Port（使用空实现）
    type MarketGovernance = ();
    type CommissionGovernance = ();
    type SingleLineGovernance = ();
    type KycGovernance = ();
    type ShopGovernance = ();
    type TokenGovernance = ();
    // Phase 4.3: 资金保护
    type TreasuryPort = ();
    type WeightInfo = ();
    type ProposalCooldown = ProposalCooldown;
    type EmergencyOrigin = frame_system::EnsureRoot<u64>;
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
            // R11-S1: 默认注册 product_id=1，供 PriceChange/InventoryAdjustment 等测试使用
            set_product_price(1, 500);
            set_product_stock(1, 100);
            // C1-audit: 默认设置 FullDAO 模式，大多数测试需要创建提案
            // 测试 None 模式行为的用例需显式覆盖此配置
            pallet_entity_governance::GovernanceConfigs::<Test>::insert(
                1u64,
                pallet_entity_governance::GovernanceConfig::<u64> {
                    mode: GovernanceMode::FullDAO,
                    voting_period: 0u64,
                    execution_delay: 0u64,
                    quorum_threshold: 0u8,
                    pass_threshold: 0u8,
                    proposal_threshold: 0u16,
                    admin_veto_enabled: false,
                },
            );
        });
        ext
    }
}

pub fn advance_blocks(n: u64) {
    let current = System::block_number();
    System::set_block_number(current + n);
}
