use crate::mock::*;
use crate::pallet;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use pallet_commission_common::{CommissionModes, CommissionPlugin, CommissionType, TokenCommissionPlugin};
use pallet_entity_common::AdminPermission;

type SingleLine = pallet::Pallet<Test>;

const OWNER: u64 = 100;
const ADMIN: u64 = 101;
const NOBODY: u64 = 999;

// ============================================================================
// Helper: 构建单链 [1, 2, 3, 4, 5]
// ============================================================================

fn setup_single_line(entity_id: u64, accounts: &[u64]) {
    for acc in accounts {
        assert_ok!(SingleLine::add_to_single_line(entity_id, acc));
    }
}

fn setup_entity(entity_id: u64) {
    set_entity_owner(entity_id, OWNER);
    set_entity_admin(entity_id, ADMIN, AdminPermission::COMMISSION_MANAGE);
}

fn setup_config(entity_id: u64) {
    setup_entity(entity_id);
    assert_ok!(SingleLine::set_single_line_config(
        RuntimeOrigin::signed(OWNER),
        entity_id,
        100,  // upline_rate = 1%
        100,  // downline_rate = 1%
        10,   // base_upline_levels
        15,   // base_downline_levels
        1000, // level_increment_threshold
        150,  // max_upline_levels
        200,  // max_downline_levels
    ));
}

// ============================================================================
// set_single_line_config 测试
// ============================================================================

#[test]
fn set_config_works() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.upline_rate, 100);
        assert_eq!(config.downline_rate, 100);
        assert_eq!(config.base_upline_levels, 10);
        assert_eq!(config.base_downline_levels, 15);
        assert_eq!(config.level_increment_threshold, 1000);
        assert_eq!(config.max_upline_levels, 150);
        assert_eq!(config.max_downline_levels, 200);
    });
}

#[test]
fn set_config_by_admin_works() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(ADMIN),
            1, 100, 100, 10, 15, 1000, 150, 200,
        ));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_some());
    });
}

#[test]
fn set_config_rejects_no_entity() {
    new_test_ext().execute_with(|| {
        // entity not registered
        assert_noop!(
            SingleLine::set_single_line_config(
                RuntimeOrigin::signed(NOBODY),
                1, 100, 100, 10, 15, 1000, 150, 200,
            ),
            pallet::Error::<Test>::EntityNotFound,
        );
    });
}

#[test]
fn set_config_rejects_non_owner_non_admin() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            SingleLine::set_single_line_config(
                RuntimeOrigin::signed(NOBODY),
                1, 100, 100, 10, 15, 1000, 150, 200,
            ),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin,
        );
    });
}

#[test]
fn set_config_rejects_invalid_upline_rate() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            SingleLine::set_single_line_config(
                RuntimeOrigin::signed(OWNER),
                1, 1001, 100, 10, 15, 1000, 150, 200,
            ),
            pallet::Error::<Test>::InvalidRate,
        );
    });
}

#[test]
fn set_config_rejects_invalid_downline_rate() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            SingleLine::set_single_line_config(
                RuntimeOrigin::signed(OWNER),
                1, 100, 1001, 10, 15, 1000, 150, 200,
            ),
            pallet::Error::<Test>::InvalidRate,
        );
    });
}

#[test]
fn set_config_boundary_rate_1000_ok() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER),
            1, 1000, 1000, 10, 15, 1000, 150, 200,
        ));
    });
}

#[test]
fn set_config_emits_event() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

// ============================================================================
// add_to_single_line 测试
// ============================================================================

#[test]
fn add_to_single_line_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(SingleLine::add_to_single_line(1, &10));
        assert_ok!(SingleLine::add_to_single_line(1, &20));
        assert_ok!(SingleLine::add_to_single_line(1, &30));

        assert_eq!(pallet::SingleLineIndex::<Test>::get(1, 10), Some(0));
        assert_eq!(pallet::SingleLineIndex::<Test>::get(1, 20), Some(1));
        assert_eq!(pallet::SingleLineIndex::<Test>::get(1, 30), Some(2));

        let line = pallet::SingleLines::<Test>::get(1);
        assert_eq!(line.len(), 3);
        assert_eq!(line[0], 10);
        assert_eq!(line[1], 20);
        assert_eq!(line[2], 30);
    });
}

#[test]
fn add_to_single_line_idempotent() {
    new_test_ext().execute_with(|| {
        assert_ok!(SingleLine::add_to_single_line(1, &10));
        assert_ok!(SingleLine::add_to_single_line(1, &10));
        let line = pallet::SingleLines::<Test>::get(1);
        assert_eq!(line.len(), 1);
    });
}

#[test]
fn add_to_single_line_cross_entity_isolation() {
    new_test_ext().execute_with(|| {
        assert_ok!(SingleLine::add_to_single_line(1, &10));
        assert_ok!(SingleLine::add_to_single_line(2, &10));

        assert_eq!(pallet::SingleLineIndex::<Test>::get(1, 10), Some(0));
        assert_eq!(pallet::SingleLineIndex::<Test>::get(2, 10), Some(0));

        let line1 = pallet::SingleLines::<Test>::get(1);
        let line2 = pallet::SingleLines::<Test>::get(2);
        assert_eq!(line1.len(), 1);
        assert_eq!(line2.len(), 1);
    });
}

// ============================================================================
// calc_extra_levels 测试
// ============================================================================

#[test]
fn calc_extra_levels_zero_threshold() {
    new_test_ext().execute_with(|| {
        assert_eq!(SingleLine::calc_extra_levels(0u128, 5000), 0);
    });
}

#[test]
fn calc_extra_levels_basic() {
    new_test_ext().execute_with(|| {
        assert_eq!(SingleLine::calc_extra_levels(1000u128, 0), 0);
        assert_eq!(SingleLine::calc_extra_levels(1000u128, 999), 0);
        assert_eq!(SingleLine::calc_extra_levels(1000u128, 1000), 1);
        assert_eq!(SingleLine::calc_extra_levels(1000u128, 5500), 5);
    });
}

#[test]
fn calc_extra_levels_capped_at_255() {
    new_test_ext().execute_with(|| {
        // H4 审计修复: 大值不溢出 u8
        assert_eq!(SingleLine::calc_extra_levels(1u128, 1000), 255);
    });
}

// ============================================================================
// process_upline 测试
// ============================================================================

#[test]
fn process_upline_basic() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        // 单链: [10, 20, 30, 40, 50]
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();

        // buyer=50 (index=4), upline_rate=100 (1%), base_upline_levels=10
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &50, &config);
        SingleLine::process_upline(entity_id, &50, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);

        // 向上 4 层: 40,30,20,10，每层 100000 * 100 / 10000 = 1000
        assert_eq!(outputs.len(), 4);
        assert_eq!(outputs[0].beneficiary, 40);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[0].commission_type, CommissionType::SingleLineUpline);
        assert_eq!(outputs[0].level, 1);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(outputs[1].amount, 1000);
        assert_eq!(outputs[2].beneficiary, 20);
        assert_eq!(outputs[2].amount, 1000);
        assert_eq!(outputs[3].beneficiary, 10);
        assert_eq!(outputs[3].amount, 1000);
        assert_eq!(remaining, 100_000 - 4000);
    });
}

#[test]
fn process_upline_buyer_at_index_0_no_output() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 10000;
        let mut outputs = alloc::vec::Vec::new();
        // buyer=10 是第一个人 (index=0)，没有上线
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &10, &config);
        SingleLine::process_upline(entity_id, &10, 10000, &mut remaining, &config, base_up, &line, &mut outputs);

        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn process_upline_buyer_not_in_line_no_output() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 10000;
        let mut outputs = alloc::vec::Vec::new();
        // buyer=99 不在单链中
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &99, &config);
        SingleLine::process_upline(entity_id, &99, 10000, &mut remaining, &config, base_up, &line, &mut outputs);

        assert!(outputs.is_empty());
    });
}

#[test]
fn process_upline_capped_by_remaining() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        // remaining 只有 1500，每层 1000
        let mut remaining: u128 = 1500;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &50, &config);
        SingleLine::process_upline(entity_id, &50, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);

        // 40: 1000, remaining=500
        // 30: min(1000, 500)=500, remaining=0
        // 后续 actual=0 → 不输出
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].amount, 500);
        assert_eq!(remaining, 0);
    });
}

#[test]
fn process_upline_dynamic_levels_with_extra() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base_upline_levels=2, max=150, threshold=1000
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 2, 1000, 150, 200,
        ));
        // 单链: [1, 2, 3, 4, 5, 6]
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6]);

        // buyer=6 (index=5), base=2, extra=3 (earned=3000/1000), effective=min(2+3,150)=5
        set_member_stats(entity_id, 6, 3000);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &6, &config);
        SingleLine::process_upline(entity_id, &6, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);

        // 向上最多 5 层，但只有 5 个人在上面 (index 0..4)
        assert_eq!(outputs.len(), 5);
        assert_eq!(outputs[0].beneficiary, 5); // level=1
        assert_eq!(outputs[4].beneficiary, 1); // level=5
    });
}

// ============================================================================
// process_downline 测试
// ============================================================================

#[test]
fn process_downline_basic() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        // 单链: [10, 20, 30, 40, 50]
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();

        // buyer=20 (index=1), downline_rate=100 (1%), base_downline_levels=15
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &20, &config);
        SingleLine::process_downline(entity_id, &20, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);

        // 向下 3 层: 30,40,50，每层 1000
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].beneficiary, 30);
        assert_eq!(outputs[0].commission_type, CommissionType::SingleLineDownline);
        assert_eq!(outputs[0].level, 1);
        assert_eq!(outputs[1].beneficiary, 40);
        assert_eq!(outputs[2].beneficiary, 50);
        assert_eq!(remaining, 100_000 - 3000);
    });
}

#[test]
fn process_downline_zero_rate_no_output() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 0, 10, 15, 1000, 150, 200,
        ));
        setup_single_line(entity_id, &[10, 20, 30]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 10000;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &10, &config);
        SingleLine::process_downline(entity_id, &10, 10000, &mut remaining, &config, base_down, &line, &mut outputs);

        assert!(outputs.is_empty());
    });
}

#[test]
fn process_downline_last_in_line_no_output() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 10000;
        let mut outputs = alloc::vec::Vec::new();
        // buyer=30 是最后一个人，没有下线
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &30, &config);
        SingleLine::process_downline(entity_id, &30, 10000, &mut remaining, &config, base_down, &line, &mut outputs);

        assert!(outputs.is_empty());
    });
}

#[test]
fn process_downline_capped_by_remaining() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 2500;
        let mut outputs = alloc::vec::Vec::new();
        // buyer=10 (index=0), 下线: 20,30,40,50 每层 1000
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &10, &config);
        SingleLine::process_downline(entity_id, &10, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);

        // 20: 1000, remaining=1500
        // 30: 1000, remaining=500
        // 40: min(1000,500)=500, remaining=0
        // 50: actual=0 → 不输出
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].amount, 1000);
        assert_eq!(outputs[1].amount, 1000);
        assert_eq!(outputs[2].amount, 500);
        assert_eq!(remaining, 0);
    });
}

#[test]
fn process_downline_dynamic_levels_with_extra() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base_downline_levels=1, threshold=500
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 1, 500, 150, 200,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5]);

        // buyer=1 (index=0), earned=1500 → extra=3, effective=min(1+3,200)=4
        set_member_stats(entity_id, 1, 1500);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &1, &config);
        SingleLine::process_downline(entity_id, &1, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);

        // 下线有 4 个人 (index 1..4)，effective=4 → 全部覆盖
        assert_eq!(outputs.len(), 4);
        assert_eq!(outputs[0].beneficiary, 2);
        assert_eq!(outputs[3].beneficiary, 5);
    });
}

// ============================================================================
// CommissionPlugin trait 测试
// ============================================================================

#[test]
fn plugin_no_config_returns_remaining() {
    new_test_ext().execute_with(|| {
        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE | CommissionModes::SINGLE_LINE_DOWNLINE);
        let (outputs, remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            1, &10, 10000, 10000, modes, false, 1,
        );
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn plugin_mode_not_enabled_returns_remaining() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        // 只启用 DIRECT_REWARD，不含单线模式
        let modes = CommissionModes(CommissionModes::DIRECT_REWARD);
        let (outputs, remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            1, &10, 10000, 10000, modes, false, 1,
        );
        assert!(outputs.is_empty());
        assert_eq!(remaining, 10000);
    });
}

#[test]
fn plugin_upline_only() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30]);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        let (outputs, remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &30, 100_000, 100_000, modes, false, 1,
        );

        // 上线: 20(level=1), 10(level=2), 每层 1000
        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|o| o.commission_type == CommissionType::SingleLineUpline));
        assert_eq!(remaining, 100_000 - 2000);
    });
}

#[test]
fn plugin_downline_only() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30]);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_DOWNLINE);
        let (outputs, remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &10, 100_000, 100_000, modes, false, 1,
        );

        // 下线: 20(level=1), 30(level=2), 每层 1000
        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|o| o.commission_type == CommissionType::SingleLineDownline));
        assert_eq!(remaining, 100_000 - 2000);
    });
}

#[test]
fn plugin_both_upline_and_downline() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE | CommissionModes::SINGLE_LINE_DOWNLINE);
        let (outputs, remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &30, 100_000, 100_000, modes, false, 1,
        );

        // 上线: 20,10 → 2 outputs
        // 下线: 40,50 → 2 outputs
        // 每层 1000，共 4000
        let upline_count = outputs.iter().filter(|o| o.commission_type == CommissionType::SingleLineUpline).count();
        let downline_count = outputs.iter().filter(|o| o.commission_type == CommissionType::SingleLineDownline).count();
        assert_eq!(upline_count, 2);
        assert_eq!(downline_count, 2);
        assert_eq!(remaining, 100_000 - 4000);
    });
}

#[test]
fn plugin_first_order_adds_to_single_line() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        // is_first_order=true → 应加入单链
        let _ = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &10, 100_000, 100_000, modes, true, 1,
        );

        assert_eq!(pallet::SingleLineIndex::<Test>::get(entity_id, 10), Some(0));
        let line = pallet::SingleLines::<Test>::get(entity_id);
        assert_eq!(line.len(), 1);
        assert_eq!(line[0], 10);
    });
}

#[test]
fn plugin_not_first_order_does_not_add() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        // is_first_order=false → 不应加入单链
        let _ = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &10, 100_000, 100_000, modes, false, 1,
        );

        assert_eq!(pallet::SingleLineIndex::<Test>::get(entity_id, 10), None);
    });
}

// ============================================================================
// TokenCommissionPlugin trait 测试
// ============================================================================

#[test]
fn token_plugin_upline_works() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30]);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        let (outputs, remaining) = <SingleLine as TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &30, 100_000u128, 100_000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 20);
        assert_eq!(outputs[0].amount, 1000u128);
        assert_eq!(outputs[1].beneficiary, 10);
        assert_eq!(remaining, 100_000 - 2000);
    });
}

#[test]
fn token_plugin_downline_works() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30]);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_DOWNLINE);
        let (outputs, remaining) = <SingleLine as TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &10, 100_000u128, 100_000u128, modes, false, 1,
        );

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 20);
        assert_eq!(outputs[1].beneficiary, 30);
        assert_eq!(remaining, 100_000 - 2000);
    });
}

#[test]
fn token_plugin_first_order_adds_to_single_line() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        let _ = <SingleLine as TokenCommissionPlugin<u64, u128>>::calculate_token(
            entity_id, &99, 10000u128, 10000u128, modes, true, 1,
        );

        assert_eq!(pallet::SingleLineIndex::<Test>::get(entity_id, 99), Some(0));
    });
}

// ============================================================================
// 边界 / 集成测试
// ============================================================================

#[test]
fn upline_levels_capped_by_max_upline_levels() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // M2 审计修复后 base 不能超过 max，改用直接写入存储测试运行时钳位行为
        pallet::SingleLineConfigs::<Test>::insert(entity_id, pallet::SingleLineConfig {
            upline_rate: 100,
            downline_rate: 100,
            base_upline_levels: 10,
            base_downline_levels: 15,
            level_increment_threshold: 1000u128,
            max_upline_levels: 2,
            max_downline_levels: 200,
        });
        setup_single_line(entity_id, &[1, 2, 3, 4, 5]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        // buyer=5 (index=4), base=10 but max=2 → clamped to 2
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &5, &config);
        SingleLine::process_upline(entity_id, &5, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].beneficiary, 4); // level=1
        assert_eq!(outputs[1].beneficiary, 3); // level=2
    });
}

#[test]
fn downline_levels_capped_by_max_downline_levels() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // M2 审计修复后 base 不能超过 max，改用直接写入存储测试运行时钳位行为
        pallet::SingleLineConfigs::<Test>::insert(entity_id, pallet::SingleLineConfig {
            upline_rate: 100,
            downline_rate: 100,
            base_upline_levels: 10,
            base_downline_levels: 15,
            level_increment_threshold: 1000u128,
            max_upline_levels: 150,
            max_downline_levels: 1,
        });
        setup_single_line(entity_id, &[1, 2, 3, 4, 5]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        // buyer=1 (index=0), base=15 but max=1 → clamped to 1
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &1, &config);
        SingleLine::process_downline(entity_id, &1, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 2);
    });
}

#[test]
fn commission_based_on_order_amount_not_remaining() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        // order_amount=100000, remaining=50000
        let mut remaining: u128 = 50_000;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &20, &config);
        SingleLine::process_upline(entity_id, &20, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);

        // commission = 100000 * 100 / 10000 = 1000 (基于 order_amount)
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].amount, 1000);
    });
}

#[test]
fn single_line_full_emits_event() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        // MaxSingleLineLength=100，填满单链
        for i in 0..100u64 {
            assert_ok!(SingleLine::add_to_single_line(entity_id, &i));
        }

        // 第 101 个应失败
        assert!(SingleLine::add_to_single_line(entity_id, &200).is_err());

        // 通过 plugin 路径触发 SingleLineJoinFailed 事件
        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        let _ = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &201, 10000, 10000, modes, true, 1,
        );
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineJoinFailed {
                entity_id,
                account: 201,
            }
            .into(),
        );
    });
}

#[test]
fn default_config_values() {
    new_test_ext().execute_with(|| {
        let config = pallet::SingleLineConfig::<u128>::default();
        assert_eq!(config.upline_rate, 10);
        assert_eq!(config.downline_rate, 10);
        assert_eq!(config.base_upline_levels, 10);
        assert_eq!(config.base_downline_levels, 15);
        assert_eq!(config.level_increment_threshold, 0);
        assert_eq!(config.max_upline_levels, 20);
        assert_eq!(config.max_downline_levels, 30);
    });
}

// ============================================================================
// 按会员等级自定义层数测试
// ============================================================================

#[test]
fn set_level_based_levels_works() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), 1, 2, 10, 12,
        ));
        let overrides = pallet::SingleLineCustomLevelOverrides::<Test>::get(1, 2).unwrap();
        assert_eq!(overrides.upline_levels, 10);
        assert_eq!(overrides.downline_levels, 12);
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::LevelBasedLevelsUpdated { entity_id: 1, level_id: 2 }.into(),
        );
    });
}

#[test]
fn set_level_based_levels_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            SingleLine::set_level_based_levels(RuntimeOrigin::signed(NOBODY), 1, 0, 20, 25),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin,
        );
    });
}

#[test]
fn set_level_based_levels_rejects_both_zero() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            SingleLine::set_level_based_levels(RuntimeOrigin::signed(OWNER), 1, 0, 0, 0),
            pallet::Error::<Test>::InvalidLevels,
        );
    });
}

#[test]
fn remove_level_based_levels_works() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), 1, 3, 8, 10,
        ));
        assert_ok!(SingleLine::remove_level_based_levels(
            RuntimeOrigin::signed(OWNER), 1, 3,
        ));
        assert_eq!(pallet::SingleLineCustomLevelOverrides::<Test>::get(1, 3), None);
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::LevelBasedLevelsRemoved { entity_id: 1, level_id: 3 }.into(),
        );
    });
}

#[test]
fn remove_nonexistent_level_no_event() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        // M3 修复: 不存在的覆盖不应发出事件
        assert_ok!(SingleLine::remove_level_based_levels(
            RuntimeOrigin::signed(OWNER), 1, 99,
        ));
        // 只有 setup_entity 不产生 pallet 事件
        let pallet_events: alloc::vec::Vec<_> = System::events().into_iter()
            .filter(|e| matches!(e.event, RuntimeEvent::CommissionSingleLine(_)))
            .collect();
        assert_eq!(pallet_events.len(), 0);
    });
}

#[test]
fn level_override_affects_upline_levels() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base_upline_levels=2, max=150
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 2, 1000, 150, 200,
        ));
        // 自定义等级 1 上线层数=5
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 1, 5, 2,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6, 7]);

        // buyer=7 (index=6) 无等级 → 使用 base=2
        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &7, &config);
        SingleLine::process_upline(entity_id, &7, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);
        assert_eq!(outputs.len(), 2); // base=2

        // buyer=7 设为自定义等级 1 → 使用 override=5
        set_custom_level(entity_id, 7, 1);
        remaining = 100_000;
        outputs.clear();
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &7, &config);
        SingleLine::process_upline(entity_id, &7, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);
        assert_eq!(outputs.len(), 5); // override=5
        assert_eq!(outputs[0].beneficiary, 6);
        assert_eq!(outputs[4].beneficiary, 2);
    });
}

#[test]
fn level_override_affects_downline_levels() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base_downline_levels=1, max=200
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 1, 1000, 150, 200,
        ));
        // 自定义等级 2 下线层数=4
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 2, 2, 4,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6]);

        // buyer=1 (index=0) 无等级 → base=1
        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &1, &config);
        SingleLine::process_downline(entity_id, &1, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);
        assert_eq!(outputs.len(), 1); // base=1

        // buyer=1 设为自定义等级 2 → override=4
        set_custom_level(entity_id, 1, 2);
        remaining = 100_000;
        outputs.clear();
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &1, &config);
        SingleLine::process_downline(entity_id, &1, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);
        assert_eq!(outputs.len(), 4); // override=4
        assert_eq!(outputs[0].beneficiary, 2);
        assert_eq!(outputs[3].beneficiary, 5);
    });
}

#[test]
fn level_override_fallback_when_no_override_for_level() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base=2(config), threshold=1000, max=150
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 2, 1000, 150, 200,
        ));
        // 自定义等级 1 override upline=3
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 1, 3, 2,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6, 7]);

        // buyer=7 自定义等级 3（无 override）→ 回退到 base=2
        set_custom_level(entity_id, 7, 3);
        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &7, &config);
        SingleLine::process_upline(entity_id, &7, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);
        assert_eq!(outputs.len(), 2); // fallback base=2
    });
}

#[test]
fn level_override_still_capped_by_max() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // max_upline_levels=3
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 2, 1000, 3, 200,
        ));
        // 自定义等级 0 override upline=10（超过 max=3）
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 0, 10, 2,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6, 7]);

        set_custom_level(entity_id, 7, 0);
        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &7, &config);
        SingleLine::process_upline(entity_id, &7, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);
        // override=10 但 max=3 → 只有 3 层
        assert_eq!(outputs.len(), 3);
    });
}

#[test]
fn level_override_combined_with_extra_levels() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base=2(config), threshold=1000, max=150
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 2, 2, 1000, 150, 200,
        ));
        // 自定义等级 1 override upline=3
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 1, 3, 2,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6, 7, 8]);

        // buyer=8 (index=7), 自定义等级 1 → override=3, earned=2000 → extra=2
        // effective = min(3+2, 150) = 5
        set_custom_level(entity_id, 8, 1);
        set_member_stats(entity_id, 8, 2000);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &8, &config);
        SingleLine::process_upline(entity_id, &8, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);
        assert_eq!(outputs.len(), 5); // override(3) + extra(2) = 5
        assert_eq!(outputs[0].beneficiary, 7);
        assert_eq!(outputs[4].beneficiary, 3);
    });
}

#[test]
fn plugin_with_level_override_integration() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        // base=1, max=150/200
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 1, 1, 1000, 150, 200,
        ));
        // 自定义等级 2 → upline=3, downline=4
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 2, 3, 4,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        // buyer=5 (index=4), 自定义等级 2
        set_custom_level(entity_id, 5, 2);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE | CommissionModes::SINGLE_LINE_DOWNLINE);
        let (outputs, remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &5, 100_000, 100_000, modes, false, 1,
        );

        // 上线: 3层 (4,3,2)
        // 下线: 4层 (6,7,8,9) — 有5个下线但只取4层
        let upline_count = outputs.iter().filter(|o| o.commission_type == CommissionType::SingleLineUpline).count();
        let downline_count = outputs.iter().filter(|o| o.commission_type == CommissionType::SingleLineDownline).count();
        assert_eq!(upline_count, 3);
        assert_eq!(downline_count, 4);
        // 每层 1000, 共 7000
        assert_eq!(remaining, 100_000 - 7000);
    });
}

/// 测试不同自定义等级的覆盖互不干扰
#[test]
fn different_custom_level_overrides_are_isolated() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_entity(entity_id);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), entity_id, 100, 100, 1, 1, 1000, 150, 200,
        ));
        // 自定义等级 1 → upline=6
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 1, 6, 1,
        ));
        // 自定义等级 2 → upline=3
        assert_ok!(SingleLine::set_level_based_levels(
            RuntimeOrigin::signed(OWNER), entity_id, 2, 3, 1,
        ));
        setup_single_line(entity_id, &[1, 2, 3, 4, 5, 6, 7, 8]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();

        // buyer=8 自定义等级 1 → 应查 CustomOverrides 得 6
        set_custom_level(entity_id, 8, 1);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &8, &config);
        assert_eq!(base_up, 6);

        // 切换为自定义等级 2 → 应查 CustomOverrides 得 3
        set_custom_level(entity_id, 8, 2);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &8, &config);
        assert_eq!(base_up, 3);
    });
}

// ============================================================================
// 深度审计回归测试
// ============================================================================

#[test]
fn m1_deep_add_to_single_line_emits_event() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);

        // 加入单链应发射 AddedToSingleLine 事件
        System::reset_events();
        assert_ok!(SingleLine::add_to_single_line(entity_id, &10));
        assert_ok!(SingleLine::add_to_single_line(entity_id, &20));

        let events: alloc::vec::Vec<_> = System::events().into_iter()
            .filter_map(|e| {
                if let RuntimeEvent::CommissionSingleLine(inner) = e.event { Some(inner) } else { None }
            })
            .collect();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], pallet::Event::AddedToSingleLine { entity_id, account: 10, index: 0 });
        assert_eq!(events[1], pallet::Event::AddedToSingleLine { entity_id, account: 20, index: 1 });

        // 重复加入不发射事件
        System::reset_events();
        assert_ok!(SingleLine::add_to_single_line(entity_id, &10));
        let events: alloc::vec::Vec<_> = System::events().into_iter()
            .filter_map(|e| {
                if let RuntimeEvent::CommissionSingleLine(inner) = e.event { Some(inner) } else { None }
            })
            .collect();
        assert_eq!(events.len(), 0);
    });
}

#[test]
fn m2_deep_set_config_rejects_base_upline_exceeds_max() {
    new_test_ext().execute_with(|| {
        // base_upline_levels=20 > max_upline_levels=10 → 应拒绝
        setup_entity(1);
        assert_noop!(
            SingleLine::set_single_line_config(
                RuntimeOrigin::signed(OWNER), 1, 100, 100, 20, 5, 1000, 10, 200,
            ),
            pallet::Error::<Test>::BaseLevelsExceedMax
        );
    });
}

#[test]
fn m2_deep_set_config_rejects_base_downline_exceeds_max() {
    new_test_ext().execute_with(|| {
        // base_downline_levels=30 > max_downline_levels=5 → 应拒绝
        setup_entity(1);
        assert_noop!(
            SingleLine::set_single_line_config(
                RuntimeOrigin::signed(OWNER), 1, 100, 100, 5, 30, 1000, 150, 5,
            ),
            pallet::Error::<Test>::BaseLevelsExceedMax
        );
    });
}

#[test]
fn m1_r3_shared_line_upline_downline_consistent() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        // 单链: [1, 2, 3, 4, 5]
        setup_single_line(entity_id, &[1, 2, 3, 4, 5]);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        // M1-R3: 一次加载 line，传给 upline + downline
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, base_down) = SingleLine::effective_base_levels(entity_id, &3, &config);

        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();

        // buyer=3 (index=2): upline=[2,1], downline=[4,5]
        SingleLine::process_upline(entity_id, &3, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);
        SingleLine::process_downline(entity_id, &3, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);

        let up_out: alloc::vec::Vec<_> = outputs.iter().filter(|o| o.commission_type == CommissionType::SingleLineUpline).collect();
        let dn_out: alloc::vec::Vec<_> = outputs.iter().filter(|o| o.commission_type == CommissionType::SingleLineDownline).collect();

        assert_eq!(up_out.len(), 2); // 2,1
        assert_eq!(up_out[0].beneficiary, 2);
        assert_eq!(up_out[1].beneficiary, 1);
        assert_eq!(dn_out.len(), 2); // 4,5
        assert_eq!(dn_out[0].beneficiary, 4);
        assert_eq!(dn_out[1].beneficiary, 5);
        // 每层 1000, 共 4 层 = 4000
        assert_eq!(remaining, 100_000 - 4000);

        // 验证与 CommissionPlugin::calculate 结果一致
        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE | CommissionModes::SINGLE_LINE_DOWNLINE);
        let (plugin_outputs, plugin_remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &3, 100_000, 100_000, modes, false, 1,
        );
        assert_eq!(plugin_outputs.len(), outputs.len());
        assert_eq!(plugin_remaining, remaining);
    });
}

#[test]
fn m2_deep_set_config_base_equals_max_ok() {
    new_test_ext().execute_with(|| {
        // base == max 应该成功
        setup_entity(1);
        assert_ok!(SingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), 1, 100, 100, 10, 15, 1000, 10, 15,
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1u64).unwrap();
        assert_eq!(config.base_upline_levels, 10);
        assert_eq!(config.max_upline_levels, 10);
        assert_eq!(config.base_downline_levels, 15);
        assert_eq!(config.max_downline_levels, 15);
    });
}

// ============================================================================
// force_set_single_line_config 测试
// ============================================================================

#[test]
fn force_set_config_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(SingleLine::force_set_single_line_config(
            RuntimeOrigin::root(), 1, 200, 300, 5, 10, 500, 50, 100,
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.upline_rate, 200);
        assert_eq!(config.downline_rate, 300);
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn force_set_config_rejects_signed() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            SingleLine::force_set_single_line_config(
                RuntimeOrigin::signed(OWNER), 1, 100, 100, 10, 15, 1000, 150, 200,
            ),
            frame_support::error::BadOrigin,
        );
    });
}

#[test]
fn force_set_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            SingleLine::force_set_single_line_config(
                RuntimeOrigin::root(), 1, 1001, 100, 10, 15, 1000, 150, 200,
            ),
            pallet::Error::<Test>::InvalidRate,
        );
    });
}

// ============================================================================
// force_clear_single_line_config 测试
// ============================================================================

#[test]
fn force_clear_config_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(SingleLine::force_set_single_line_config(
            RuntimeOrigin::root(), 1, 100, 100, 10, 15, 1000, 150, 200,
        ));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_some());

        assert_ok!(SingleLine::force_clear_single_line_config(
            RuntimeOrigin::root(), 1,
        ));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_none());
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigCleared { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn force_clear_nonexistent_is_noop() {
    new_test_ext().execute_with(|| {
        // 不存在配置时 force_clear 不报错也不发事件
        assert_ok!(SingleLine::force_clear_single_line_config(
            RuntimeOrigin::root(), 99,
        ));
        let pallet_events: alloc::vec::Vec<_> = System::events().into_iter()
            .filter(|e| matches!(e.event, RuntimeEvent::CommissionSingleLine(_)))
            .collect();
        assert_eq!(pallet_events.len(), 0);
    });
}

#[test]
fn force_clear_rejects_signed() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            SingleLine::force_clear_single_line_config(
                RuntimeOrigin::signed(OWNER), 1,
            ),
            frame_support::error::BadOrigin,
        );
    });
}

// ============================================================================
// clear_single_line_config 测试（Owner/Admin）
// ============================================================================

#[test]
fn clear_config_by_owner_works() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_some());

        assert_ok!(SingleLine::clear_single_line_config(
            RuntimeOrigin::signed(OWNER), 1,
        ));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_none());
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigCleared { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn clear_config_by_admin_works() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(SingleLine::clear_single_line_config(
            RuntimeOrigin::signed(ADMIN), 1,
        ));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_none());
    });
}

#[test]
fn clear_config_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            SingleLine::clear_single_line_config(RuntimeOrigin::signed(NOBODY), 1),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin,
        );
    });
}

#[test]
fn clear_config_rejects_not_found() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        // 未设置配置
        assert_noop!(
            SingleLine::clear_single_line_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::ConfigNotFound,
        );
    });
}

// ============================================================================
// update_single_line_params 测试
// ============================================================================

#[test]
fn update_params_upline_rate_only() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(SingleLine::update_single_line_params(
            RuntimeOrigin::signed(OWNER), 1, Some(500), None, None,
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.upline_rate, 500);
        assert_eq!(config.downline_rate, 100); // unchanged
    });
}

#[test]
fn update_params_downline_rate_only() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(SingleLine::update_single_line_params(
            RuntimeOrigin::signed(OWNER), 1, None, Some(800), None,
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.upline_rate, 100); // unchanged
        assert_eq!(config.downline_rate, 800);
    });
}

#[test]
fn update_params_threshold_only() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(SingleLine::update_single_line_params(
            RuntimeOrigin::signed(OWNER), 1, None, None, Some(9999),
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.level_increment_threshold, 9999);
        assert_eq!(config.upline_rate, 100); // unchanged
    });
}

#[test]
fn update_params_multiple() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_ok!(SingleLine::update_single_line_params(
            RuntimeOrigin::signed(OWNER), 1, Some(200), Some(300), Some(5000),
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.upline_rate, 200);
        assert_eq!(config.downline_rate, 300);
        assert_eq!(config.level_increment_threshold, 5000);
    });
}

#[test]
fn update_params_rejects_nothing_to_update() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            SingleLine::update_single_line_params(
                RuntimeOrigin::signed(OWNER), 1, None, None, None,
            ),
            pallet::Error::<Test>::NothingToUpdate,
        );
    });
}

#[test]
fn update_params_rejects_config_not_found() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        assert_noop!(
            SingleLine::update_single_line_params(
                RuntimeOrigin::signed(OWNER), 1, Some(100), None, None,
            ),
            pallet::Error::<Test>::ConfigNotFound,
        );
    });
}

#[test]
fn update_params_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            SingleLine::update_single_line_params(
                RuntimeOrigin::signed(OWNER), 1, Some(1001), None, None,
            ),
            pallet::Error::<Test>::InvalidRate,
        );
        assert_noop!(
            SingleLine::update_single_line_params(
                RuntimeOrigin::signed(OWNER), 1, None, Some(1001), None,
            ),
            pallet::Error::<Test>::InvalidRate,
        );
    });
}

#[test]
fn update_params_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        assert_noop!(
            SingleLine::update_single_line_params(
                RuntimeOrigin::signed(NOBODY), 1, Some(100), None, None,
            ),
            pallet::Error::<Test>::NotEntityOwnerOrAdmin,
        );
    });
}

#[test]
fn update_params_emits_event() {
    new_test_ext().execute_with(|| {
        setup_config(1);
        System::reset_events();
        assert_ok!(SingleLine::update_single_line_params(
            RuntimeOrigin::signed(OWNER), 1, Some(200), None, None,
        ));
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

// ============================================================================
// is_banned 受益人检查测试
// ============================================================================

#[test]
fn banned_upline_skipped() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        // 单链: [10, 20, 30, 40, 50]
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        // ban 40 (index=3)
        set_banned(entity_id, 40);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (base_up, _) = SingleLine::effective_base_levels(entity_id, &50, &config);
        SingleLine::process_upline(entity_id, &50, 100_000, &mut remaining, &config, base_up, &line, &mut outputs);

        // 上线: 40(banned→skip), 30, 20, 10 → 3 outputs
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].beneficiary, 30); // 40 was skipped
        assert_eq!(outputs[1].beneficiary, 20);
        assert_eq!(outputs[2].beneficiary, 10);
    });
}

#[test]
fn banned_downline_skipped() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30, 40, 50]);

        // ban 30 (index=2)
        set_banned(entity_id, 30);

        let config = pallet::SingleLineConfigs::<Test>::get(entity_id).unwrap();
        let mut remaining: u128 = 100_000;
        let mut outputs = alloc::vec::Vec::new();
        let line = pallet::SingleLines::<Test>::get(entity_id);
        let (_, base_down) = SingleLine::effective_base_levels(entity_id, &10, &config);
        SingleLine::process_downline(entity_id, &10, 100_000, &mut remaining, &config, base_down, &line, &mut outputs);

        // 下线: 20, 30(banned→skip), 40, 50 → 3 outputs
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].beneficiary, 20);
        assert_eq!(outputs[1].beneficiary, 40); // 30 was skipped
        assert_eq!(outputs[2].beneficiary, 50);
    });
}

#[test]
fn banned_via_plugin_integration() {
    new_test_ext().execute_with(|| {
        let entity_id = 1u64;
        setup_config(entity_id);
        setup_single_line(entity_id, &[10, 20, 30]);

        set_banned(entity_id, 20);

        let modes = CommissionModes(CommissionModes::SINGLE_LINE_UPLINE);
        let (outputs, _remaining) = <SingleLine as CommissionPlugin<u64, u128>>::calculate(
            entity_id, &30, 100_000, 100_000, modes, false, 1,
        );

        // 上线: 20(banned), 10 → only 10
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].beneficiary, 10);
    });
}

// ============================================================================
// SingleLinePlanWriter 测试
// ============================================================================

#[test]
fn plan_writer_set_config_works() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::SingleLinePlanWriter;

        assert_ok!(<SingleLine as SingleLinePlanWriter>::set_single_line_config(
            1, 200, 300, 5, 10, 2000, 50, 100,
        ));
        let config = pallet::SingleLineConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.upline_rate, 200);
        assert_eq!(config.downline_rate, 300);
        assert_eq!(config.base_upline_levels, 5);
        assert_eq!(config.base_downline_levels, 10);
        assert_eq!(config.level_increment_threshold, 2000);
        assert_eq!(config.max_upline_levels, 50);
        assert_eq!(config.max_downline_levels, 100);
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigUpdated { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn plan_writer_set_config_rejects_invalid_rate() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::SingleLinePlanWriter;

        assert!(<SingleLine as SingleLinePlanWriter>::set_single_line_config(
            1, 1001, 100, 5, 10, 2000, 50, 100,
        ).is_err());
    });
}

#[test]
fn plan_writer_set_config_rejects_base_exceeds_max() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::SingleLinePlanWriter;

        assert!(<SingleLine as SingleLinePlanWriter>::set_single_line_config(
            1, 100, 100, 20, 10, 2000, 10, 100,
        ).is_err());
    });
}

#[test]
fn plan_writer_clear_config_works() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::SingleLinePlanWriter;

        assert_ok!(<SingleLine as SingleLinePlanWriter>::set_single_line_config(
            1, 100, 100, 5, 10, 2000, 50, 100,
        ));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_some());

        assert_ok!(<SingleLine as SingleLinePlanWriter>::clear_config(1));
        assert!(pallet::SingleLineConfigs::<Test>::get(1).is_none());
        frame_system::Pallet::<Test>::assert_has_event(
            pallet::Event::<Test>::SingleLineConfigCleared { entity_id: 1 }.into(),
        );
    });
}

#[test]
fn plan_writer_clear_nonexistent_is_noop() {
    new_test_ext().execute_with(|| {
        use pallet_commission_common::SingleLinePlanWriter;

        assert_ok!(<SingleLine as SingleLinePlanWriter>::clear_config(99));
        let pallet_events: alloc::vec::Vec<_> = System::events().into_iter()
            .filter(|e| matches!(e.event, RuntimeEvent::CommissionSingleLine(_)))
            .collect();
        assert_eq!(pallet_events.len(), 0);
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_set_single_line_config() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        set_entity_locked(1);
        assert_noop!(
            CommissionSingleLine::set_single_line_config(
                RuntimeOrigin::signed(OWNER), 1,
                100, 100, 10, 15, 1000, 150, 200,
            ),
            pallet::Error::<Test>::EntityLocked
        );
    });
}

#[test]
fn entity_locked_rejects_clear_single_line_config() {
    new_test_ext().execute_with(|| {
        setup_entity(1);
        // 先设置配置
        assert_ok!(CommissionSingleLine::set_single_line_config(
            RuntimeOrigin::signed(OWNER), 1,
            100, 100, 10, 15, 1000, 150, 200,
        ));
        // 锁定后无法清除
        set_entity_locked(1);
        assert_noop!(
            CommissionSingleLine::clear_single_line_config(RuntimeOrigin::signed(OWNER), 1),
            pallet::Error::<Test>::EntityLocked
        );
    });
}
