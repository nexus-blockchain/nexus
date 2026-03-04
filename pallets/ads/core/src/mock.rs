use crate as pallet_ads_core;
use frame_support::{
	assert_ok,
	derive_impl,
	parameter_types,
	traits::{ConstU32, ConstU64},
};
use pallet_ads_primitives::*;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		AdsCore: pallet_ads_core,
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
	type ExistentialDeposit = ConstU128<1>;
}

use frame_support::traits::ConstU128;

parameter_types! {
	pub const TreasuryAccountId: u64 = 999;
}

// ============================================================================
// Mock DeliveryVerifier — 直通验证, 裁切到 cap=500
// ============================================================================

pub struct MockDeliveryVerifier;

impl DeliveryVerifier<u64> for MockDeliveryVerifier {
	fn verify_and_cap_audience(
		_who: &u64,
		_placement_id: &PlacementId,
		audience_size: u32,
		_node_id: Option<[u8; 32]>,
	) -> Result<u32, sp_runtime::DispatchError> {
		// 简单裁切到 500
		Ok(core::cmp::min(audience_size, 500))
	}
}

// ============================================================================
// Mock PlacementAdminProvider
// ============================================================================

pub struct MockPlacementAdmin;

impl PlacementAdminProvider<u64> for MockPlacementAdmin {
	fn placement_admin(placement_id: &PlacementId) -> Option<u64> {
		// placement[0] == 1 → admin = PLACEMENT_ADMIN (10)
		// placement[0] == 2 → admin = PLACEMENT_ADMIN2 (11)
		match placement_id[0] {
			1 => Some(PLACEMENT_ADMIN),
			2 => Some(PLACEMENT_ADMIN2),
			_ => None,
		}
	}
	fn is_placement_banned(placement_id: &PlacementId) -> bool {
		placement_id[0] == 255
	}
	fn placement_status(placement_id: &PlacementId) -> PlacementStatus {
		if placement_id[0] == 255 {
			PlacementStatus::Banned
		} else if placement_id[0] == 1 || placement_id[0] == 2 {
			PlacementStatus::Active
		} else {
			PlacementStatus::Unknown
		}
	}
}

// ============================================================================
// Mock RevenueDistributor — 80% 给广告位
// ============================================================================

pub struct MockRevenueDistributor;

impl RevenueDistributor<u64, u128> for MockRevenueDistributor {
	fn distribute(
		_placement_id: &PlacementId,
		total_cost: u128,
		_advertiser: &u64,
	) -> Result<RevenueBreakdown<u128>, sp_runtime::DispatchError> {
		// 80% 给广告位, 20% 平台
		let placement_share = total_cost * 80 / 100;
		Ok(RevenueBreakdown {
			placement_share,
			node_share: 0,
			platform_share: total_cost - placement_share,
		})
	}
}

impl pallet_ads_core::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MaxAdTextLength = ConstU32<280>;
	type MaxAdUrlLength = ConstU32<256>;
	type MaxReceiptsPerPlacement = ConstU32<50>;
	type MaxAdvertiserBlacklist = ConstU32<50>;
	type MaxAdvertiserWhitelist = ConstU32<20>;
	type MaxPlacementBlacklist = ConstU32<50>;
	type MaxPlacementWhitelist = ConstU32<10>;
	type MinBidPerMille = ConstU128<100_000_000_000>; // 0.1 UNIT
	type MinAudienceSize = ConstU32<20>;
	type AdSlashPercentage = ConstU32<30>;
	type TreasuryAccount = TreasuryAccountId;
	type DeliveryVerifier = MockDeliveryVerifier;
	type PlacementAdmin = MockPlacementAdmin;
	type RevenueDistributor = MockRevenueDistributor;
	type PrivateAdRegistrationFee = ConstU128<1_000_000_000_000>; // 1 UNIT
	type SettlementIncentiveBps = ConstU32<10>; // 0.1%
	type MaxCampaignsPerAdvertiser = ConstU32<100>;
	type MaxTargetsPerCampaign = ConstU32<20>;
	type ReceiptConfirmationWindow = ConstU64<100>;
	type AdvertiserReferralRate = ConstU32<500>; // 5% of platform share
	type MaxReferredAdvertisers = ConstU32<100>;
}

pub const ADVERTISER: u64 = 1;
pub const ADVERTISER2: u64 = 2;
pub const PLACEMENT_ADMIN: u64 = 10;
pub const PLACEMENT_ADMIN2: u64 = 11;
pub const TREASURY: u64 = 999;
pub const REPORTER: u64 = 50;

pub fn placement_id(n: u8) -> PlacementId {
	let mut h = [0u8; 32];
	h[0] = n;
	h
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(ADVERTISER, 1_000_000_000_000_000),       // 1000 UNIT
			(ADVERTISER2, 500_000_000_000_000),         // 500 UNIT
			(PLACEMENT_ADMIN, 500_000_000_000_000),     // 500 UNIT
			(PLACEMENT_ADMIN2, 500_000_000_000_000),    // 500 UNIT
			(TREASURY, 1_000_000_000_000_000),          // 1000 UNIT
			(REPORTER, 100_000_000_000_000),            // 100 UNIT
		],
		dev_accounts: None,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		// Phase 6: 预注册种子广告主, 使现有测试不受影响
		assert_ok!(AdsCore::force_register_advertiser(RuntimeOrigin::root(), ADVERTISER));
		assert_ok!(AdsCore::force_register_advertiser(RuntimeOrigin::root(), ADVERTISER2));
	});
	ext
}
