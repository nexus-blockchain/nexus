use crate as pallet_nex_market;
use frame_support::{
    derive_impl,
    parameter_types,
    traits::{ConstU128, ConstU16, ConstU32, ConstU64},
};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        NexMarket: pallet_nex_market,
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

parameter_types! {
    pub const TreasuryAccountId: u64 = 99;
    pub const SeedLiquidityAccountId: u64 = 96;
    pub const RewardSourceId: u64 = 97;
    pub const SeedTronAddr: [u8; 34] = *b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWb1";
}

impl pallet_nex_market::Config for Test {
    type Currency = Balances;
    type WeightInfo = ();
    type DefaultOrderTTL = ConstU32<14400>;
    type MaxActiveOrdersPerUser = ConstU32<100>;
    type UsdtTimeout = ConstU32<7200>;                // 12h
    type BlocksPerHour = ConstU32<600>;
    type BlocksPerDay = ConstU32<14400>;
    type BlocksPerWeek = ConstU32<100800>;
    type CircuitBreakerDuration = ConstU32<600>;
    type VerificationReward = ConstU128<{ 100_000_000_000 }>; // 0.1 NEX
    type RewardSource = RewardSourceId;
    type BuyerDepositRate = ConstU16<1000>;            // 10%
    type MinBuyerDeposit = ConstU128<{ 10_000_000_000_000 }>; // 10 NEX
    type DepositForfeitRate = ConstU16<10000>;         // 100%
    type UsdtToNexRate = ConstU64<{ 10_000_000_000 }>; // 1 USDT = 10 NEX
    type TreasuryAccount = TreasuryAccountId;
    type SeedLiquidityAccount = SeedLiquidityAccountId;
    type MarketAdminOrigin = frame_system::EnsureRoot<u64>;
    type FirstOrderTimeout = ConstU32<600>;            // 1h (免保证金短超时)
    type MaxFirstOrderAmount = ConstU128<{ 100_000_000_000_000 }>; // 100 NEX
    type MaxWaivedSeedOrders = ConstU32<10>;
    type SeedPricePremiumBps = ConstU16<2000>;               // 20% 溢价
    type SeedOrderUsdtAmount = ConstU64<10_000_000>;             // 10 USDT
    type SeedTronAddress = SeedTronAddr;
    type VerificationGracePeriod = ConstU32<600>;  // 1h 宽限期
    type UnderpaidGracePeriod = ConstU32<1200>;    // 2h 补付窗口
    type MaxPendingTrades = ConstU32<100>;
    type MaxAwaitingPaymentTrades = ConstU32<100>;
    type MaxUnderpaidTrades = ConstU32<100>;
    type MaxExpiredOrdersPerBlock = ConstU32<10>;
    type TxHashTtlBlocks = ConstU32<100800>;     // ~7 days (600 blocks/h × 24h × 7d)
}

pub const ALICE: u64 = 1;   // 卖家
pub const BOB: u64 = 2;     // 买家
pub const CHARLIE: u64 = 3; // 第三方（领取奖励）
pub const DAVE: u64 = 4;    // 新用户（零余额，测试首单免保证金）

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000_000_000_000_000),   // 1000 NEX
            (BOB, 1_000_000_000_000_000),     // 1000 NEX
            (CHARLIE, 100_000_000_000_000),   // 100 NEX
            (97, 1_000_000_000_000_000),      // RewardSource: 1000 NEX
            (96, 500_000_000_000_000),        // SeedLiquidity: 500 NEX
            (99, 1_000_000_000_000_000),      // Treasury: 1000 NEX
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
