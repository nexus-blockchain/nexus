use crate::{mock::*, pallet::{TransferWhitelist, TransferBlacklist}, Error, Event};
use frame_support::{assert_noop, assert_ok};
use pallet_entity_common::{TokenType, TransferRestrictionMode};

const SHOP_ID: u64 = 1;
const OWNER: u64 = 1;
const USER_A: u64 = 2;
const USER_B: u64 = 3;

/// 创建一个默认代币
fn setup_token() {
    register_shop(SHOP_ID, OWNER);
    assert_ok!(EntityToken::create_shop_token(
        RuntimeOrigin::signed(OWNER),
        SHOP_ID,
        b"TestToken".to_vec(),
        b"TT".to_vec(),
        18,
        500,  // 5% reward
        1000, // 10% exchange
    ));
}

// ==================== create_shop_token ====================

#[test]
fn create_shop_token_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert!(EntityToken::entity_token_configs(SHOP_ID).is_some());
        System::assert_has_event(RuntimeEvent::EntityToken(Event::EntityTokenCreated {
            entity_id: SHOP_ID,
            asset_id: 1_000_001,
            name: b"TestToken".to_vec(),
            symbol: b"TT".to_vec(),
        }));
    });
}

#[test]
fn create_shop_token_fails_shop_not_found() {
    new_test_ext().execute_with(|| {
        // P0: 不存在的 entity 也不会通过 is_entity_active 检查
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                999,
                b"T".to_vec(),
                b"T".to_vec(),
                18, 500, 1000,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn create_shop_token_fails_shop_not_active() {
    new_test_ext().execute_with(|| {
        register_inactive_shop(SHOP_ID, OWNER);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                b"T".to_vec(),
                b"T".to_vec(),
                18, 500, 1000,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn create_shop_token_fails_not_owner() {
    new_test_ext().execute_with(|| {
        register_shop(SHOP_ID, OWNER);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(USER_A),
                SHOP_ID,
                b"T".to_vec(),
                b"T".to_vec(),
                18, 500, 1000,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn create_shop_token_fails_already_exists() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                b"T2".to_vec(),
                b"T2".to_vec(),
                18, 500, 1000,
            ),
            Error::<Test>::TokenAlreadyExists
        );
    });
}

#[test]
fn create_shop_token_fails_empty_name() {
    new_test_ext().execute_with(|| {
        register_shop(SHOP_ID, OWNER);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                vec![],
                b"T".to_vec(),
                18, 500, 1000,
            ),
            Error::<Test>::EmptyName
        );
    });
}

#[test]
fn create_shop_token_fails_empty_symbol() {
    new_test_ext().execute_with(|| {
        register_shop(SHOP_ID, OWNER);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                b"T".to_vec(),
                vec![],
                18, 500, 1000,
            ),
            Error::<Test>::EmptySymbol
        );
    });
}

#[test]
fn create_shop_token_fails_invalid_rate() {
    new_test_ext().execute_with(|| {
        register_shop(SHOP_ID, OWNER);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                b"T".to_vec(),
                b"T".to_vec(),
                18, 10001, 1000,
            ),
            Error::<Test>::InvalidRewardRate
        );
    });
}

// ==================== update_token_config ====================

#[test]
fn update_token_config_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            Some(800),
            None,
            None,
            None,
            None,
            None,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.reward_rate, 800);
    });
}

#[test]
fn update_config_min_gt_max_rejected() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::update_token_config(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                None, None,
                Some(200),
                Some(100),
                None, None,
            ),
            Error::<Test>::InvalidRedeemLimits
        );
    });
}

// ==================== mint_tokens ====================

#[test]
fn mint_tokens_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            USER_A,
            1000,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 1000);
    });
}

#[test]
fn mint_tokens_fails_exceeds_max_supply() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 设置 max_supply = 500
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            500,
        ));
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                USER_A,
                501,
            ),
            Error::<Test>::ExceedsMaxSupply
        );
        // 铸造刚好 500 应该成功
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            USER_A,
            500,
        ));
    });
}

// ==================== transfer_tokens ====================

#[test]
fn transfer_tokens_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 先设置可转让
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 400,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 600);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 400);
    });
}

#[test]
fn transfer_blocked_by_locked_tokens() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 锁仓 800
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 800, 100,
        ));
        // 尝试转 300 → 可用只有 200
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 300,
            ),
            Error::<Test>::InsufficientBalance
        );
        // 200 应该可以
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 200,
        ));
    });
}

#[test]
fn transfer_blocked_by_reserved_tokens() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 通过 EntityTokenProvider 预留 800
        use pallet_entity_common::EntityTokenProvider;
        assert_ok!(<EntityToken as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 800));
        // 尝试转 300 → 可用只有 200
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 300,
            ),
            Error::<Test>::InsufficientBalance
        );
    });
}

// ==================== configure_dividend ====================

#[test]
fn configure_dividend_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 先将类型改为 Equity（支持分红）
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 10,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert!(config.dividend_config.enabled);
        assert_eq!(config.dividend_config.min_period, 10);
    });
}

// ==================== distribute_dividend ====================

#[test]
fn distribute_dividend_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        let recipients = vec![(USER_A, 100u128), (USER_B, 200u128)];
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 300, recipients,
        ));
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 100);
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_B), 200);
        // 检查累计金额
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.dividend_config.accumulated, 300);
    });
}

#[test]
fn distribute_dividend_fails_amount_mismatch() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        let recipients = vec![(USER_A, 100u128), (USER_B, 200u128)];
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 999, recipients,
            ),
            Error::<Test>::DividendAmountMismatch
        );
    });
}

#[test]
fn distribute_dividend_fails_too_many_recipients() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // MaxDividendRecipients = 50, 创建 51 个
        let recipients: Vec<(u64, u128)> = (0..51).map(|i| (i + 10, 1u128)).collect();
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 51, recipients,
            ),
            Error::<Test>::TooManyRecipients
        );
    });
}

#[test]
fn distribute_dividend_fails_token_type_not_supported() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 先设置为 Equity 以便配置分红
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // 切回 Points 类型（不支持分红）
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Points,
        ));
        // distribute 应失败
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
            ),
            Error::<Test>::TokenTypeNotSupported
        );
    });
}

// ==================== claim_dividend ====================

#[test]
fn claim_dividend_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
        ));
        assert_ok!(EntityToken::claim_dividend(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 100);
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 0);
        assert_eq!(EntityToken::claimed_dividends(SHOP_ID, &USER_A), 100);
    });
}

#[test]
fn h2r3_distribute_dividend_rejects_exceeding_max_supply() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 50,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // H2-R3: distribute 时即检查 max_supply，100 > 50 容量 → 拒绝
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
            ),
            Error::<Test>::ExceedsMaxSupply
        );
    });
}

// ==================== lock_tokens / unlock_tokens ====================

#[test]
fn lock_and_unlock_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 10,
        ));
        // 解锁前不行
        assert_noop!(
            EntityToken::unlock_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID),
            Error::<Test>::UnlockTimeNotReached
        );
        // 推进区块
        run_to_block(12);
        assert_ok!(EntityToken::unlock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
    });
}

#[test]
fn lock_tokens_fails_zero_amount() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::lock_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 0, 10),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn lock_tokens_fails_zero_duration() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::lock_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 0),
            Error::<Test>::InvalidLockDuration
        );
    });
}

#[test]
fn lock_tokens_fails_token_not_enabled() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 禁用代币
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, None, Some(false),
        ));
        assert_noop!(
            EntityToken::lock_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 10),
            Error::<Test>::TokenNotEnabled
        );
    });
}

// ==================== change_token_type ====================

#[test]
fn change_token_type_updates_restrictions() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 默认 Points → Equity
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.token_type, TokenType::Equity);
        // Equity 的默认转账限制和 KYC 级别应被联动更新
        assert_eq!(config.transfer_restriction, TokenType::Equity.default_transfer_restriction());
        assert_eq!(config.min_receiver_kyc, TokenType::Equity.required_kyc_level().1);
    });
}

// ==================== set_max_supply ====================

#[test]
fn set_max_supply_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 10_000,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.max_supply, 10_000);
    });
}

#[test]
fn set_max_supply_fails_below_current() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::set_max_supply(RuntimeOrigin::signed(OWNER), SHOP_ID, 500),
            Error::<Test>::ExceedsMaxSupply
        );
    });
}

// ==================== set_transfer_restriction ====================

#[test]
fn set_transfer_restriction_clamped_kyc() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            TransferRestrictionMode::KycRequired, 99,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.min_receiver_kyc, 4); // clamped to 4
        // 事件也应该是 clamped 值
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TransferRestrictionSet {
            entity_id: SHOP_ID,
            mode: TransferRestrictionMode::KycRequired,
            min_receiver_kyc: 4,
        }));
    });
}

// ==================== whitelist / blacklist ====================

#[test]
fn whitelist_management_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::add_to_whitelist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A, USER_B],
        ));
        assert!(TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_A));
        assert!(TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_B));

        assert_ok!(EntityToken::remove_from_whitelist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        assert!(!TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_A));
        assert!(TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_B));
    });
}

#[test]
fn blacklist_management_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::add_to_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        assert!(TransferBlacklist::<Test>::contains_key(SHOP_ID, USER_A));

        assert_ok!(EntityToken::remove_from_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        assert!(!TransferBlacklist::<Test>::contains_key(SHOP_ID, USER_A));
    });
}

// ==================== transfer restriction modes ====================

#[test]
fn transfer_whitelist_mode_enforced() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 设置白名单模式
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            TransferRestrictionMode::Whitelist, 0,
        ));
        // USER_A 不在白名单（发送方检查）
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::SenderNotInWhitelist
        );
        // 加入 USER_A 到白名单，但 USER_B 不在
        assert_ok!(EntityToken::add_to_whitelist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverNotInWhitelist
        );
        // 双方都加入白名单后可以
        assert_ok!(EntityToken::add_to_whitelist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_B],
        ));
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
    });
}

#[test]
fn transfer_blacklist_mode_enforced() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            TransferRestrictionMode::Blacklist, 0,
        ));
        // 加入黑名单
        assert_ok!(EntityToken::add_to_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_B],
        ));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverInBlacklist
        );
    });
}

#[test]
fn transfer_kyc_mode_enforced() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            TransferRestrictionMode::KycRequired, 2,
        ));
        // USER_A KYC level = 0，发送方先被拦
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::SenderKycInsufficient
        );
        // 设置 USER_A KYC level = 3，USER_B 仍不达标
        set_kyc_level(USER_A, 3);
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverKycInsufficient
        );
        // 设置 USER_B KYC level = 3，双方都达标
        set_kyc_level(USER_B, 3);
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
    });
}

#[test]
fn transfer_members_only_mode_enforced() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            TransferRestrictionMode::MembersOnly, 0,
        ));
        // USER_A 不是成员，发送方先被拦
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::SenderNotMember
        );
        // 设置 USER_A 为成员，USER_B 仍不是
        set_member(1, USER_A, true);
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverNotMember
        );
        // 双方都是成员
        set_member(1, USER_B, true);
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
    });
}

// ==================== EntityTokenProvider trait ====================

#[test]
fn reserve_unreserve_repatriate_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));

        use pallet_entity_common::EntityTokenProvider;
        type Provider = EntityToken;

        // 预留 600
        assert_ok!(<Provider as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 600));
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 600);

        // 再预留 500 失败（可用 = 1000 - 600 = 400）
        assert!(<Provider as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 500).is_err());

        // 解锁 200
        let unreserved = <Provider as EntityTokenProvider<u64, u128>>::unreserve(SHOP_ID, &USER_A, 200);
        assert_eq!(unreserved, 200);
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 400);

        // repatriate_reserved 300 → USER_B
        let repatriated = <Provider as EntityTokenProvider<u64, u128>>::repatriate_reserved(SHOP_ID, &USER_A, &USER_B, 300).unwrap();
        assert_eq!(repatriated, 300);
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 100);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 300);
    });
}

// ==================== reward_on_purchase max_supply ====================

#[test]
fn reward_on_purchase_skips_when_max_supply_reached() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 100,
        ));
        // 先铸造 90
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 90,
        ));
        // reward_on_purchase 会尝试铸造 5% 的 10000 = 500
        // 但 max_supply = 100, current = 90, 所以应该跳过返回 0
        let reward = EntityToken::reward_on_purchase(SHOP_ID, &USER_B, 10000);
        assert_ok!(&reward);
        assert_eq!(reward.unwrap(), 0);
    });
}

// ==================== H1 回归: unlock_tokens 错误语义 ====================

#[test]
fn h1_unlock_tokens_no_locks_returns_no_locked_tokens() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 用户没有任何锁仓 → NoLockedTokens
        assert_noop!(
            EntityToken::unlock_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID),
            Error::<Test>::NoLockedTokens
        );
    });
}

#[test]
fn h1_unlock_tokens_all_unexpired_returns_unlock_time_not_reached() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 锁仓 500，到期 block 11
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 10,
        ));
        // block 1，全部未到期 → UnlockTimeNotReached
        assert_noop!(
            EntityToken::unlock_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID),
            Error::<Test>::UnlockTimeNotReached
        );
    });
}

#[test]
fn h1_unlock_partial_expired_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 两笔锁仓：300 到 block 5，400 到 block 20
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 300, 4,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 400, 19,
        ));
        // block 6: 第一笔到期，第二笔未到期
        run_to_block(6);
        assert_ok!(EntityToken::unlock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        // 应只解锁了 300，仍有 400 锁仓
        let locks = EntityToken::locked_tokens(SHOP_ID, &USER_A);
        assert_eq!(locks.len(), 1);
        assert_eq!(locks[0].amount, 400);
    });
}

// ==================== input length limits (M7) ====================

#[test]
fn whitelist_input_length_limited() {
    new_test_ext().execute_with(|| {
        setup_token();
        // MaxTransferListSize = 100, 传入 101 个
        let too_many: Vec<u64> = (0..101).collect();
        assert_noop!(
            EntityToken::add_to_whitelist(
                RuntimeOrigin::signed(OWNER), SHOP_ID, too_many,
            ),
            Error::<Test>::TransferListFull
        );
    });
}

// ==================== Audit v2 regression tests ====================

#[test]
fn h1_distribute_dividend_rejects_when_token_disabled() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // 禁用代币
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID, None, None,
            None, None, None, Some(false),
        ));
        // 分红应失败：代币已禁用
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
            ),
            Error::<Test>::TokenNotEnabled
        );
    });
}

#[test]
fn h2_change_token_type_rejects_same_type() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 默认类型是 Points，尝试再次设置为 Points
        assert_noop!(
            EntityToken::change_token_type(
                RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Points,
            ),
            Error::<Test>::SameTokenType
        );
        // 变更为 Equity 应成功
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        // 再次设置为 Equity 应失败
        assert_noop!(
            EntityToken::change_token_type(
                RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
            ),
            Error::<Test>::SameTokenType
        );
    });
}

#[test]
fn m1_transfer_tokens_rejects_zero_amount() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID, None, None,
            None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 0,
            ),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn m2_distribute_dividend_rejects_zero_total() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // 零总额分红应失败
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 0, vec![],
            ),
            Error::<Test>::ZeroDividendAmount
        );
    });
}

#[test]
fn m3_mint_tokens_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 停用实体
        deactivate_entity(SHOP_ID);
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn m4_unlock_tokens_cleans_empty_storage() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 5,
        ));
        // 推进到解锁时间后
        run_to_block(7);
        assert_ok!(EntityToken::unlock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        // 存储应已清理（不再包含空条目）
        let locks = EntityToken::locked_tokens(SHOP_ID, &USER_A);
        assert!(locks.is_empty());
        // 验证 LockedTokens 存储项已被完全移除
        assert!(!crate::LockedTokens::<Test>::contains_key(SHOP_ID, &USER_A));
    });
}

// ==================== Round 3 回归测试 ====================

#[test]
fn h1r3_repatriate_reserved_consistent_on_transfer_failure() {
    use pallet_entity_common::EntityTokenProvider;
    use frame_support::traits::fungibles::Mutate as FungiblesMutate;
    new_test_ext().execute_with(|| {
        setup_token();
        let asset_id = EntityToken::entity_to_asset_id(SHOP_ID);

        // 铸造 100 给 USER_A
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100,
        ));
        // 虚拟预留 80
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 80)
        );
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 80);

        // 通过 pallet-assets 直接转出 60，绕过我们的预留检查
        // USER_A: assets balance = 40, reserved(our pallet) = 80
        assert_ok!(<Assets as FungiblesMutate<u64>>::transfer(
            asset_id, &USER_A, &USER_B, 60,
            frame_support::traits::tokens::Preservation::Preserve,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 40);

        // repatriate_reserved 50 → transfer 应失败（USER_A 只有 40）
        let result = <EntityToken as EntityTokenProvider<u64, u128>>::repatriate_reserved(
            SHOP_ID, &USER_A, &USER_B, 50,
        );
        assert!(result.is_err());

        // H1-R3: ReservedTokens 应保持不变（80），不应被错误扣减
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 80);
    });
}

#[test]
fn h2r3_claim_dividend_succeeds_despite_later_mint() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 200,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // distribute 50（max_supply=200, current=0, 容量足够）
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 50, vec![(USER_A, 50)],
        ));
        // M1-R6: mint 现在计入 pending，最多铸 150（0+50+150=200）
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_B, 150,
        ));
        // H2-R3: claim 应成功（分红在 distribute 时已承诺，不应受后续铸造影响）
        assert_ok!(EntityToken::claim_dividend(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 50);
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 0);
    });
}

#[test]
fn m1r3_distribute_dividend_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        // 停用实体
        deactivate_entity(SHOP_ID);
        // M1-R3: 被停用的 Entity 不应能分发分红
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn l1r3_transfer_tokens_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        deactivate_entity(SHOP_ID);
        // L1-R3: 被停用 Entity 的代币不允许转账
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn l1r3_lock_tokens_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        deactivate_entity(SHOP_ID);
        // L1-R3: 被停用 Entity 的代币不允许新增锁仓
        assert_noop!(
            EntityToken::lock_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 10,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn l1r3_unlock_and_claim_still_work_when_inactive() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 先锁仓 + 分发分红（Entity 活跃时）
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 500, 5,
        ));
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 5,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
        ));

        // 停用 Entity
        deactivate_entity(SHOP_ID);
        run_to_block(7);

        // unlock 和 claim 应仍可执行（用户取回已有权益）
        assert_ok!(EntityToken::unlock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        assert_ok!(EntityToken::claim_dividend(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 1100); // 1000 + 100 dividend
    });
}

#[test]
fn m1r4_distribute_multiple_times_respects_max_supply_with_pending() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 设置 Equity 类型 + 分红 + max_supply
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 200,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));

        // 第一次分发 150，成功（0 + 0 + 150 <= 200）
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 150, vec![(USER_A, 150)],
        ));
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 150);

        // 第二次分发 60，应拒绝（0 + 150 + 60 > 200）
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 60, vec![(USER_B, 60)],
            ),
            Error::<Test>::ExceedsMaxSupply
        );

        // 第二次分发 50，成功（0 + 150 + 50 <= 200）
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 50, vec![(USER_B, 50)],
        ));
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 200);
    });
}

#[test]
fn m1r4_claim_reduces_total_pending_dividends() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 200,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));

        // 分发 150
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 150, vec![(USER_A, 100), (USER_B, 50)],
        ));
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 150);

        // A 领取 100 → pending 降为 50
        assert_ok!(EntityToken::claim_dividend(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 50);

        // 此时可再分发 150（supply=100 + pending=50 + 150 = 300? 不对，supply=100, pending=50, max=200）
        // supply=100 + pending=50 + new=50 = 200 ≤ 200 → OK
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 50, vec![(USER_A, 50)],
        ));
        // supply=100 + pending=100 + new=1 = 201 > 200 → FAIL
        assert_noop!(
            EntityToken::distribute_dividend(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 1, vec![(USER_A, 1)],
            ),
            Error::<Test>::ExceedsMaxSupply
        );
    });
}

#[test]
fn m1r4_set_max_supply_accounts_for_pending_dividends() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        // 铸造 100
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100,
        ));
        // 分发 50 分红（pending=50）
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 50, vec![(USER_A, 50)],
        ));
        // 尝试设 max_supply=140: supply(100) + pending(50) = 150 > 140 → 应拒绝
        assert_noop!(
            EntityToken::set_max_supply(
                RuntimeOrigin::signed(OWNER), SHOP_ID, 140,
            ),
            Error::<Test>::ExceedsMaxSupply
        );
        // 设 max_supply=150: 100 + 50 = 150 ≤ 150 → OK
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 150,
        ));
    });
}

// ==================== Round 6 回归测试 ====================

#[test]
fn m1r6_mint_tokens_respects_pending_dividends_in_max_supply() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 1000,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));

        // 分发 800 分红（pending=800）
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 800,
            vec![(USER_A, 400), (USER_B, 400)],
        ));
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 800);

        // M1-R6: mint 201 应拒绝（supply=0 + pending=800 + 201 = 1001 > 1000）
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 201,
            ),
            Error::<Test>::ExceedsMaxSupply
        );

        // mint 200 应成功（0 + 800 + 200 = 1000 ≤ 1000）
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 200,
        ));

        // 用户 claim 后，supply 上升但 pending 下降，可继续铸造
        assert_ok!(EntityToken::claim_dividend(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
        // supply=600(200+400), pending=400, mint 1 → 600+400+1=1001 > 1000 → 拒绝
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1,
            ),
            Error::<Test>::ExceedsMaxSupply
        );
    });
}

#[test]
fn m1r6_reward_on_purchase_skips_when_pending_fills_capacity() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 100,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));

        // 分发 95 分红（pending=95）
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 95, vec![(USER_A, 95)],
        ));

        // reward_on_purchase: 5% of 200 = 10, supply=0+pending=95+10=105 > 100 → skip
        let reward = EntityToken::reward_on_purchase(SHOP_ID, &USER_B, 200);
        assert_ok!(&reward);
        assert_eq!(reward.unwrap(), 0);

        // 5% of 80 = 4, 0+95+4=99 ≤ 100 → 应成功
        let reward = EntityToken::reward_on_purchase(SHOP_ID, &USER_B, 80);
        assert_ok!(&reward);
        assert_eq!(reward.unwrap(), 4);
    });
}

// ==================== P0: Admin 权限下放测试 ====================

const ADMIN: u64 = 4;
const NON_ADMIN: u64 = 5;

/// Admin 设置 helper：注册 shop + 给 ADMIN 设置 TOKEN_MANAGE 权限
fn setup_token_with_admin() {
    setup_token();
    set_entity_admin(SHOP_ID, ADMIN, pallet_entity_common::AdminPermission::TOKEN_MANAGE);
}

#[test]
fn p0_admin_can_create_shop_token() {
    new_test_ext().execute_with(|| {
        let entity_id = 2u64;
        register_shop(entity_id, OWNER);
        set_entity_admin(entity_id, ADMIN, pallet_entity_common::AdminPermission::TOKEN_MANAGE);
        assert_ok!(EntityToken::create_shop_token(
            RuntimeOrigin::signed(ADMIN),
            entity_id,
            b"AdminToken".to_vec(),
            b"AT".to_vec(),
            18, 500, 1000,
        ));
        assert!(EntityToken::entity_token_configs(entity_id).is_some());
    });
}

#[test]
fn p0_admin_can_update_token_config() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(ADMIN), SHOP_ID,
            Some(800), None, None, None, None, None,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.reward_rate, 800);
    });
}

#[test]
fn p0_admin_can_mint_tokens() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, USER_A, 500,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 500);
    });
}

#[test]
fn p0_admin_can_configure_dividend() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, true, 10,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert!(config.dividend_config.enabled);
    });
}

#[test]
fn p0_admin_can_distribute_dividend() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, 100, vec![(USER_A, 100)],
        ));
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 100);
    });
}

#[test]
fn p0_admin_can_change_token_type() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, TokenType::Governance,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.token_type, TokenType::Governance);
    });
}

#[test]
fn p0_admin_can_set_max_supply() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::set_max_supply(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, 50_000,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.max_supply, 50_000);
    });
}

#[test]
fn p0_admin_can_set_transfer_restriction() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(ADMIN), SHOP_ID,
            TransferRestrictionMode::KycRequired, 2,
        ));
        let config = EntityToken::entity_token_configs(SHOP_ID).unwrap();
        assert_eq!(config.transfer_restriction, TransferRestrictionMode::KycRequired);
        assert_eq!(config.min_receiver_kyc, 2);
    });
}

#[test]
fn p0_admin_can_manage_whitelist() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::add_to_whitelist(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, vec![USER_A, USER_B],
        ));
        assert!(TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_A));
        assert!(TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_B));
        assert_ok!(EntityToken::remove_from_whitelist(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, vec![USER_A],
        ));
        assert!(!TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_A));
        assert!(TransferWhitelist::<Test>::contains_key(SHOP_ID, USER_B));
    });
}

#[test]
fn p0_admin_can_manage_blacklist() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        assert_ok!(EntityToken::add_to_blacklist(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, vec![USER_A],
        ));
        assert!(TransferBlacklist::<Test>::contains_key(SHOP_ID, USER_A));
        assert_ok!(EntityToken::remove_from_blacklist(
            RuntimeOrigin::signed(ADMIN), SHOP_ID, vec![USER_A],
        ));
        assert!(!TransferBlacklist::<Test>::contains_key(SHOP_ID, USER_A));
    });
}

#[test]
fn p0_non_admin_rejected_for_all_owner_extrinsics() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        // NON_ADMIN 没有任何权限
        assert_noop!(
            EntityToken::update_token_config(
                RuntimeOrigin::signed(NON_ADMIN), SHOP_ID,
                Some(100), None, None, None, None, None,
            ),
            Error::<Test>::NotAuthorized
        );
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(NON_ADMIN), SHOP_ID, USER_A, 100,
            ),
            Error::<Test>::NotAuthorized
        );
        assert_noop!(
            EntityToken::set_max_supply(
                RuntimeOrigin::signed(NON_ADMIN), SHOP_ID, 999,
            ),
            Error::<Test>::NotAuthorized
        );
        assert_noop!(
            EntityToken::add_to_whitelist(
                RuntimeOrigin::signed(NON_ADMIN), SHOP_ID, vec![USER_A],
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn p0_wrong_permission_rejected() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 给 ADMIN 设置 SHOP_MANAGE 而非 TOKEN_MANAGE
        set_entity_admin(SHOP_ID, ADMIN, pallet_entity_common::AdminPermission::SHOP_MANAGE);
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(ADMIN), SHOP_ID, USER_A, 100,
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn p0_owner_still_works_without_admin_flag() {
    new_test_ext().execute_with(|| {
        setup_token();
        // OWNER 不在 ENTITY_ADMINS 中，但作为 owner 仍可操作
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 100);
    });
}

#[test]
fn p0_admin_rejected_for_nonexistent_entity() {
    new_test_ext().execute_with(|| {
        set_entity_admin(999, ADMIN, pallet_entity_common::AdminPermission::TOKEN_MANAGE);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(ADMIN), 999,
                b"T".to_vec(), b"T".to_vec(), 18, 500, 1000,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== EntityLocked 回归测试 ====================

#[test]
fn entity_locked_rejects_create_shop_token() {
    new_test_ext().execute_with(|| {
        register_shop(1, OWNER);
        set_entity_locked(1);
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER), 1,
                b"Token".to_vec(), b"TKN".to_vec(), 18, 1000, 10000,
            ),
            Error::<Test>::EntityLocked
        );
    });
}

// ==================== P1: force_disable_token ====================

#[test]
fn p1_force_disable_token_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert!(EntityToken::is_token_enabled(SHOP_ID));

        assert_ok!(EntityToken::force_disable_token(
            RuntimeOrigin::root(), SHOP_ID,
        ));

        assert!(!EntityToken::is_token_enabled(SHOP_ID));
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TokenForceDisabled {
            entity_id: SHOP_ID,
        }));
    });
}

#[test]
fn p1_force_disable_token_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_disable_token(RuntimeOrigin::signed(OWNER), SHOP_ID),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p1_force_disable_token_rejects_no_token() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityToken::force_disable_token(RuntimeOrigin::root(), 999),
            Error::<Test>::TokenNotEnabled
        );
    });
}

#[test]
fn p1_force_disable_token_rejects_already_disabled() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::force_disable_token(RuntimeOrigin::root(), SHOP_ID));
        assert_noop!(
            EntityToken::force_disable_token(RuntimeOrigin::root(), SHOP_ID),
            Error::<Test>::TokenAlreadyDisabled
        );
    });
}

// ==================== P1: force_freeze_transfers / force_unfreeze_transfers ====================

#[test]
fn p1_force_freeze_transfers_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));

        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TransfersFrozenEvent {
            entity_id: SHOP_ID,
        }));

        // 转账应被阻止
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::TokenTransfersFrozen
        );
    });
}

#[test]
fn p1_force_freeze_transfers_allows_claim_dividend() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 变更为 Equity 类型以支持分红
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        // 铸造代币并配置分红
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 500, vec![(USER_A, 500)],
        ));

        // 冻结转账
        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));

        // 分红领取不受影响
        assert_ok!(EntityToken::claim_dividend(
            RuntimeOrigin::signed(USER_A), SHOP_ID,
        ));
    });
}

#[test]
fn p1_force_freeze_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_freeze_transfers(RuntimeOrigin::signed(OWNER), SHOP_ID),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p1_force_freeze_rejects_no_token() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityToken::force_freeze_transfers(RuntimeOrigin::root(), 999),
            Error::<Test>::TokenNotEnabled
        );
    });
}

#[test]
fn p1_force_freeze_rejects_already_frozen() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));
        assert_noop!(
            EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID),
            Error::<Test>::TransfersAlreadyFrozen
        );
    });
}

#[test]
fn p1_force_unfreeze_transfers_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));

        // 冻结
        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::TokenTransfersFrozen
        );

        // 解冻
        assert_ok!(EntityToken::force_unfreeze_transfers(RuntimeOrigin::root(), SHOP_ID));
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TransfersUnfrozen {
            entity_id: SHOP_ID,
        }));

        // 转账恢复正常
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
    });
}

#[test]
fn p1_force_unfreeze_rejects_not_frozen() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_unfreeze_transfers(RuntimeOrigin::root(), SHOP_ID),
            Error::<Test>::TransfersNotFrozen
        );
    });
}

#[test]
fn p1_force_unfreeze_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));
        assert_noop!(
            EntityToken::force_unfreeze_transfers(RuntimeOrigin::signed(OWNER), SHOP_ID),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== P2: force_burn ====================

#[test]
fn p2_force_burn_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 1000);

        assert_ok!(EntityToken::force_burn(
            RuntimeOrigin::root(), SHOP_ID, USER_A, 300,
        ));

        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 700);
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TokensForceBurned {
            entity_id: SHOP_ID,
            from: USER_A,
            amount: 300,
        }));
    });
}

#[test]
fn p2_force_burn_full_balance() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 500,
        ));

        assert_ok!(EntityToken::force_burn(
            RuntimeOrigin::root(), SHOP_ID, USER_A, 500,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 0);
    });
}

#[test]
fn p2_force_burn_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::force_burn(RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p2_force_burn_rejects_zero_amount() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_burn(RuntimeOrigin::root(), SHOP_ID, USER_A, 0),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn p2_force_burn_rejects_no_token() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityToken::force_burn(RuntimeOrigin::root(), 999, USER_A, 100),
            Error::<Test>::TokenNotEnabled
        );
    });
}

#[test]
fn p2_force_burn_rejects_insufficient_balance() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100,
        ));
        // 尝试销毁超过余额的数量
        assert!(EntityToken::force_burn(
            RuntimeOrigin::root(), SHOP_ID, USER_A, 200,
        ).is_err());
    });
}

// ==================== P3: set_global_token_pause ====================

#[test]
fn p3_global_token_pause_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert!(EntityToken::global_token_paused());
        System::assert_has_event(RuntimeEvent::EntityToken(Event::GlobalTokenPauseSet {
            paused: true,
        }));
    });
}

#[test]
fn p3_global_token_unpause_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), false));
        assert!(!EntityToken::global_token_paused());
        System::assert_has_event(RuntimeEvent::EntityToken(Event::GlobalTokenPauseSet {
            paused: false,
        }));
    });
}

#[test]
fn p3_global_pause_rejects_non_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EntityToken::set_global_token_pause(RuntimeOrigin::signed(OWNER), true),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn p3_global_pause_blocks_create_token() {
    new_test_ext().execute_with(|| {
        register_shop(SHOP_ID, OWNER);
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER), SHOP_ID,
                b"T".to_vec(), b"T".to_vec(), 18, 500, 1000,
            ),
            Error::<Test>::GlobalPaused
        );
    });
}

#[test]
fn p3_global_pause_blocks_mint() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_noop!(
            EntityToken::mint_tokens(
                RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 100,
            ),
            Error::<Test>::GlobalPaused
        );
    });
}

#[test]
fn p3_global_pause_blocks_transfer() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::GlobalPaused
        );
    });
}

#[test]
fn p3_global_pause_blocks_claim_dividend() {
    new_test_ext().execute_with(|| {
        setup_token();
        // 变更为 Equity 类型以支持分红
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 500, vec![(USER_A, 500)],
        ));

        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_noop!(
            EntityToken::claim_dividend(RuntimeOrigin::signed(USER_A), SHOP_ID),
            Error::<Test>::GlobalPaused
        );
    });
}

#[test]
fn p3_global_unpause_restores_operations() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));

        // 暂停
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::GlobalPaused
        );

        // 恢复
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), false));
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 100);
    });
}

#[test]
fn p3_global_pause_reward_on_purchase_returns_zero() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));

        // reward_on_purchase 在全局暂停时静默返回 0
        let reward = EntityToken::reward_on_purchase(SHOP_ID, &USER_A, 10000).unwrap();
        assert_eq!(reward, 0);
    });
}

#[test]
fn p3_global_pause_redeem_for_discount_fails() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));

        assert_noop!(
            EntityToken::redeem_for_discount(SHOP_ID, &USER_A, 100),
            Error::<Test>::GlobalPaused
        );
    });
}

// ==================== burn_tokens ====================

#[test]
fn burn_tokens_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::burn_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 400,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 600);
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TokensBurned {
            entity_id: SHOP_ID,
            holder: USER_A,
            amount: 400,
        }));
    });
}

#[test]
fn burn_tokens_rejects_zero() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::burn_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 0),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn burn_tokens_rejects_insufficient_available() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 锁仓 800
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 800, 100,
        ));
        // 可用 200，尝试销毁 300
        assert_noop!(
            EntityToken::burn_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 300),
            Error::<Test>::InsufficientBalance
        );
        // 可用 200，销毁 200 成功
        assert_ok!(EntityToken::burn_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 800);
    });
}

#[test]
fn burn_tokens_rejects_global_pause() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert_noop!(
            EntityToken::burn_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 100),
            Error::<Test>::GlobalPaused
        );
    });
}

#[test]
fn burn_tokens_rejects_disabled_token() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::force_disable_token(RuntimeOrigin::root(), SHOP_ID));
        assert_noop!(
            EntityToken::burn_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, 100),
            Error::<Test>::TokenNotEnabled
        );
    });
}

// ==================== update_token_metadata ====================

#[test]
fn update_token_metadata_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_metadata(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            b"NewName".to_vec(), b"NN".to_vec(),
        ));
        let meta = EntityToken::entity_token_metadata(SHOP_ID).unwrap();
        assert_eq!(meta.0.to_vec(), b"NewName".to_vec());
        assert_eq!(meta.1.to_vec(), b"NN".to_vec());
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TokenMetadataUpdated {
            entity_id: SHOP_ID,
            name: b"NewName".to_vec(),
            symbol: b"NN".to_vec(),
        }));
    });
}

#[test]
fn update_token_metadata_rejects_non_owner() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::update_token_metadata(
                RuntimeOrigin::signed(USER_A), SHOP_ID,
                b"New".to_vec(), b"N".to_vec(),
            ),
            Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn update_token_metadata_rejects_empty_name() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::update_token_metadata(
                RuntimeOrigin::signed(OWNER), SHOP_ID,
                b"".to_vec(), b"NN".to_vec(),
            ),
            Error::<Test>::EmptyName
        );
    });
}

#[test]
fn update_token_metadata_rejects_empty_symbol() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::update_token_metadata(
                RuntimeOrigin::signed(OWNER), SHOP_ID,
                b"Name".to_vec(), b"".to_vec(),
            ),
            Error::<Test>::EmptySymbol
        );
    });
}

// ==================== force_transfer ====================

#[test]
fn force_transfer_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::force_transfer(
            RuntimeOrigin::root(), SHOP_ID, USER_A, USER_B, 500,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 500);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 500);
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TokensForceTransferred {
            entity_id: SHOP_ID,
            from: USER_A,
            to: USER_B,
            amount: 500,
        }));
    });
}

#[test]
fn force_transfer_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::force_transfer(
                RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, USER_B, 500,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_transfer_rejects_zero() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_transfer(RuntimeOrigin::root(), SHOP_ID, USER_A, USER_B, 0),
            Error::<Test>::ZeroAmount
        );
    });
}

// ==================== force_enable_token ====================

#[test]
fn force_enable_token_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::force_disable_token(RuntimeOrigin::root(), SHOP_ID));
        assert!(!EntityToken::is_token_enabled(SHOP_ID));

        assert_ok!(EntityToken::force_enable_token(RuntimeOrigin::root(), SHOP_ID));
        assert!(EntityToken::is_token_enabled(SHOP_ID));
        System::assert_has_event(RuntimeEvent::EntityToken(Event::TokenForceEnabled {
            entity_id: SHOP_ID,
        }));
    });
}

#[test]
fn force_enable_token_rejects_already_enabled() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_enable_token(RuntimeOrigin::root(), SHOP_ID),
            Error::<Test>::TokenAlreadyEnabled
        );
    });
}

#[test]
fn force_enable_token_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::force_disable_token(RuntimeOrigin::root(), SHOP_ID));
        assert_noop!(
            EntityToken::force_enable_token(RuntimeOrigin::signed(OWNER), SHOP_ID),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ==================== force_burn cleanup ====================

#[test]
fn force_burn_cleans_up_storage_on_zero_balance() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 添加锁仓
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200, 100,
        ));
        // 强制销毁全部（包括锁仓的）
        assert_ok!(EntityToken::force_burn(RuntimeOrigin::root(), SHOP_ID, USER_A, 1000));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 0);
        // 验证锁仓已清理
        assert!(EntityToken::get_lock_entries(SHOP_ID, &USER_A).is_empty());
    });
}

#[test]
fn force_burn_partial_does_not_cleanup() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200, 100,
        ));
        // 部分销毁
        assert_ok!(EntityToken::force_burn(RuntimeOrigin::root(), SHOP_ID, USER_A, 500));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 500);
        // 锁仓仍然存在
        assert!(!EntityToken::get_lock_entries(SHOP_ID, &USER_A).is_empty());
    });
}

// ==================== from-side transfer restriction ====================

#[test]
fn blacklist_sender_blocked() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_transfer_restriction(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            TransferRestrictionMode::Blacklist, 0,
        ));
        // 将发送方加入黑名单
        assert_ok!(EntityToken::add_to_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::SenderInBlacklist
        );
    });
}

// ==================== query functions ====================

#[test]
fn get_account_token_info_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 300, 100,
        ));

        let (balance, locked, reserved, pending, available) =
            EntityToken::get_account_token_info(SHOP_ID, &USER_A);
        assert_eq!(balance, 1000);
        assert_eq!(locked, 300);
        assert_eq!(reserved, 0);
        assert_eq!(pending, 0);
        assert_eq!(available, 700);
    });
}

#[test]
fn get_lock_entries_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200, 50,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 300, 100,
        ));
        let entries = EntityToken::get_lock_entries(SHOP_ID, &USER_A);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].amount, 200);
        assert_eq!(entries[1].amount, 300);
    });
}

#[test]
fn get_available_balance_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200, 100,
        ));
        assert_eq!(EntityToken::get_available_balance(SHOP_ID, &USER_A), 800);
    });
}

// ==================== EntityTokenProvider::available_balance ====================

#[test]
fn trait_available_balance_works() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::EntityTokenProvider;
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 400, 100,
        ));
        let avail = <EntityToken as EntityTokenProvider<u64, u128>>::available_balance(SHOP_ID, &USER_A);
        assert_eq!(avail, 600);
    });
}

// ==================== EntityTokenProvider::transfer with checks ====================

#[test]
fn trait_transfer_checks_global_pause() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::EntityTokenProvider;
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));
        assert!(<EntityToken as EntityTokenProvider<u64, u128>>::transfer(SHOP_ID, &USER_A, &USER_B, 100).is_err());
    });
}

#[test]
fn trait_transfer_checks_frozen() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::EntityTokenProvider;
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));
        assert!(<EntityToken as EntityTokenProvider<u64, u128>>::transfer(SHOP_ID, &USER_A, &USER_B, 100).is_err());
    });
}

#[test]
fn trait_transfer_checks_available_balance() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::EntityTokenProvider;
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 900, 100,
        ));
        // 可用 100，转 200 应失败
        assert!(<EntityToken as EntityTokenProvider<u64, u128>>::transfer(SHOP_ID, &USER_A, &USER_B, 200).is_err());
        // 可用 100，转 100 应成功
        assert_ok!(<EntityToken as EntityTokenProvider<u64, u128>>::transfer(SHOP_ID, &USER_A, &USER_B, 100));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 100);
    });
}

// ==================== 审计回归测试 ====================

#[test]
fn h3_admin_can_update_token_metadata() {
    new_test_ext().execute_with(|| {
        setup_token_with_admin();
        // H3: Admin（非资产创建者）应能更新元数据
        assert_ok!(EntityToken::update_token_metadata(
            RuntimeOrigin::signed(ADMIN), SHOP_ID,
            b"AdminUpdated".to_vec(), b"AU".to_vec(),
        ));
        let meta = EntityToken::entity_token_metadata(SHOP_ID).unwrap();
        assert_eq!(meta.0.to_vec(), b"AdminUpdated".to_vec());
        assert_eq!(meta.1.to_vec(), b"AU".to_vec());
    });
}

#[test]
fn m1_force_transfer_cleans_storage_on_zero_balance() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 设置锁仓和预留
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200, 100,
        ));
        assert_ok!(
            <EntityToken as pallet_entity_common::EntityTokenProvider<u64, u128>>::reserve(
                SHOP_ID, &USER_A, 300
            )
        );
        // 配置分红并分发
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 50, vec![(USER_A, 50)],
        ));
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 50);

        // force_transfer 转走全部余额
        assert_ok!(EntityToken::force_transfer(
            RuntimeOrigin::root(), SHOP_ID, USER_A, USER_B, 1000,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 0);

        // M1: 锁仓、预留、待领取分红应已清理
        assert!(EntityToken::get_lock_entries(SHOP_ID, &USER_A).is_empty());
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 0);
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 0);
        // TotalPendingDividends 应已递减
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 0);
    });
}

#[test]
fn m1_force_transfer_partial_does_not_cleanup() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::lock_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 200, 100,
        ));

        // 部分 force_transfer
        assert_ok!(EntityToken::force_transfer(
            RuntimeOrigin::root(), SHOP_ID, USER_A, USER_B, 500,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 500);
        // 锁仓仍存在
        assert!(!EntityToken::get_lock_entries(SHOP_ID, &USER_A).is_empty());
    });
}

#[test]
fn m2_trait_transfer_rejects_inactive_entity() {
    new_test_ext().execute_with(|| {
        use pallet_entity_common::EntityTokenProvider;
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        deactivate_entity(SHOP_ID);
        // M2: trait transfer 应与 extrinsic 一致，拒绝不活跃实体
        assert!(
            <EntityToken as EntityTokenProvider<u64, u128>>::transfer(
                SHOP_ID, &USER_A, &USER_B, 100
            ).is_err()
        );
    });
}

#[test]
fn m3_burn_tokens_works_when_entity_inactive() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        deactivate_entity(SHOP_ID);
        // M3: 用户应能在 Entity 不活跃时销毁自己的代币
        assert_ok!(EntityToken::burn_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, 400,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 600);
    });
}

// ==================== Round 2 审计回归测试 ====================

#[test]
fn h1r2_repatriate_reserved_works_when_entity_inactive() {
    use pallet_entity_common::EntityTokenProvider;
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // 预留 500
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 500)
        );
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 500);

        // 停用实体
        deactivate_entity(SHOP_ID);

        // H1-R2: repatriate_reserved 应绕过 EntityNotActive 检查
        let result = <EntityToken as EntityTokenProvider<u64, u128>>::repatriate_reserved(
            SHOP_ID, &USER_A, &USER_B, 300,
        );
        assert_ok!(&result);
        assert_eq!(result.unwrap(), 300);
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 200);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 300);
    });
}

#[test]
fn h1r2_repatriate_reserved_works_when_transfers_frozen() {
    use pallet_entity_common::EntityTokenProvider;
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 500)
        );

        // 冻结转账
        assert_ok!(EntityToken::force_freeze_transfers(RuntimeOrigin::root(), SHOP_ID));

        // H1-R2: repatriate_reserved 应绕过 TransfersFrozen 检查
        let result = <EntityToken as EntityTokenProvider<u64, u128>>::repatriate_reserved(
            SHOP_ID, &USER_A, &USER_B, 400,
        );
        assert_ok!(&result);
        assert_eq!(result.unwrap(), 400);
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 100);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 400);
    });
}

#[test]
fn h1r2_repatriate_reserved_works_when_global_paused() {
    use pallet_entity_common::EntityTokenProvider;
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 500)
        );

        // 全局暂停
        assert_ok!(EntityToken::set_global_token_pause(RuntimeOrigin::root(), true));

        // H1-R2: repatriate_reserved 应绕过 GlobalPaused 检查
        let result = <EntityToken as EntityTokenProvider<u64, u128>>::repatriate_reserved(
            SHOP_ID, &USER_A, &USER_B, 500,
        );
        assert_ok!(&result);
        assert_eq!(result.unwrap(), 500);
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 0);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 500);
    });
}

#[test]
fn h1r2_repatriate_reserved_works_with_full_reserved_balance() {
    use pallet_entity_common::EntityTokenProvider;
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 500,
        ));
        // 预留全部余额
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(SHOP_ID, &USER_A, 500)
        );
        // available = 500 - 500 = 0，旧代码 Self::transfer() 会因可用余额不足而失败
        // H1-R2: 直接 Assets::transfer 应成功（pallet-assets 中余额确实存在）
        let result = <EntityToken as EntityTokenProvider<u64, u128>>::repatriate_reserved(
            SHOP_ID, &USER_A, &USER_B, 500,
        );
        assert_ok!(&result);
        assert_eq!(result.unwrap(), 500);
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 0);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 0);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_B), 500);
    });
}

// ==================== Round 3 审计回归测试 (本轮) ====================

#[test]
fn l2r3_trait_transfer_rejects_zero_amount() {
    use pallet_entity_common::EntityTokenProvider;
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::update_token_config(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            None, None, None, None, Some(true), None,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // L2-R3: trait transfer 应拒绝零数量
        assert!(
            <EntityToken as EntityTokenProvider<u64, u128>>::transfer(
                SHOP_ID, &USER_A, &USER_B, 0
            ).is_err()
        );
        // 非零应成功
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::transfer(
                SHOP_ID, &USER_A, &USER_B, 100
            )
        );
    });
}

#[test]
fn l3r3_trait_reserve_rejects_zero_amount() {
    use pallet_entity_common::EntityTokenProvider;
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        // L3-R3: trait reserve 应拒绝零数量
        assert!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(
                SHOP_ID, &USER_A, 0
            ).is_err()
        );
        // 非零应成功
        assert_ok!(
            <EntityToken as EntityTokenProvider<u64, u128>>::reserve(
                SHOP_ID, &USER_A, 500
            )
        );
        assert_eq!(EntityToken::reserved_tokens(SHOP_ID, &USER_A), 500);
    });
}

// ==================== R4 审计新增测试 ====================

#[test]
fn r4_claim_dividend_blocked_when_token_disabled() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 100,
            vec![(USER_A, 100)],
        ));
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 100);

        // Root 强制禁用代币
        assert_ok!(EntityToken::force_disable_token(RuntimeOrigin::root(), SHOP_ID));

        // claim 应被拒绝
        assert_noop!(
            EntityToken::claim_dividend(RuntimeOrigin::signed(USER_A), SHOP_ID),
            Error::<Test>::TokenNotEnabled
        );
    });
}

#[test]
fn r4_transfer_tokens_rejects_self_transfer() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_noop!(
            EntityToken::transfer_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, USER_A, 100),
            Error::<Test>::SelfTransfer
        );
    });
}

#[test]
fn r4_insider_trading_blocked_during_blackout() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));

        // 非内幕人员在黑窗口期可以转账
        set_blackout_period(SHOP_ID);
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));

        // 注册 USER_A 为内幕人员
        set_insider(SHOP_ID, USER_A);

        // 内幕人员在黑窗口期不能转账
        assert_noop!(
            EntityToken::transfer_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100),
            Error::<Test>::InsiderTradingRestricted
        );

        // 解除黑窗口期后可以转账
        clear_blackout_period(SHOP_ID);
        assert_ok!(EntityToken::transfer_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
    });
}

#[test]
fn r4_force_cancel_pending_dividends_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::change_token_type(
            RuntimeOrigin::signed(OWNER), SHOP_ID, TokenType::Equity,
        ));
        assert_ok!(EntityToken::configure_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, true, 0,
        ));
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 200,
            vec![(USER_A, 100), (USER_B, 100)],
        ));
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 100);
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_B), 100);
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 200);

        // Root 取消 USER_A 的待领取分红
        assert_ok!(EntityToken::force_cancel_pending_dividends(
            RuntimeOrigin::root(), SHOP_ID, vec![USER_A],
        ));
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_A), 0);
        assert_eq!(EntityToken::pending_dividends(SHOP_ID, &USER_B), 100);
        assert_eq!(EntityToken::total_pending_dividends(SHOP_ID), 100);
    });
}

#[test]
fn r4_force_cancel_pending_dividends_rejects_non_root() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_cancel_pending_dividends(
                RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn r4_force_cancel_pending_dividends_rejects_nothing_to_cancel() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::force_cancel_pending_dividends(
                RuntimeOrigin::root(), SHOP_ID, vec![USER_A],
            ),
            Error::<Test>::NoPendingDividendsToCancel
        );
    });
}

#[test]
fn r4_whitelist_blacklist_query_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert!(!EntityToken::is_whitelisted(SHOP_ID, &USER_A));
        assert!(!EntityToken::is_blacklisted(SHOP_ID, &USER_A));

        assert_ok!(EntityToken::add_to_whitelist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        assert!(EntityToken::is_whitelisted(SHOP_ID, &USER_A));
        assert!(!EntityToken::is_whitelisted(SHOP_ID, &USER_B));

        assert_ok!(EntityToken::add_to_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_B],
        ));
        assert!(EntityToken::is_blacklisted(SHOP_ID, &USER_B));
        assert!(!EntityToken::is_blacklisted(SHOP_ID, &USER_A));
    });
}

#[test]
fn r4_approve_and_transfer_from_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));

        // USER_A 授权 USER_B 使用 500 代币
        assert_ok!(EntityToken::approve_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 500,
        ));
        assert_eq!(EntityToken::get_allowance(SHOP_ID, &USER_A, &USER_B), 500);

        // USER_B 通过授权将 USER_A 的代币转给 3(USER_B)
        let user_c: u64 = 4;
        assert_ok!(EntityToken::transfer_from(
            RuntimeOrigin::signed(USER_B), SHOP_ID, USER_A, user_c, 200,
        ));
        assert_eq!(EntityToken::token_balance(SHOP_ID, &USER_A), 800);
        assert_eq!(EntityToken::token_balance(SHOP_ID, &user_c), 200);
        assert_eq!(EntityToken::get_allowance(SHOP_ID, &USER_A, &USER_B), 300);
    });
}

#[test]
fn r4_transfer_from_rejects_insufficient_allowance() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::approve_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
        ));
        assert_noop!(
            EntityToken::transfer_from(
                RuntimeOrigin::signed(USER_B), SHOP_ID, USER_A, 4, 200,
            ),
            Error::<Test>::InsufficientAllowance
        );
    });
}

#[test]
fn r4_approve_rejects_self_approval() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_noop!(
            EntityToken::approve_tokens(RuntimeOrigin::signed(USER_A), SHOP_ID, USER_A, 500),
            Error::<Test>::SelfTransfer
        );
    });
}

#[test]
fn r4_transfer_from_rejects_self_transfer() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::mint_tokens(
            RuntimeOrigin::signed(OWNER), SHOP_ID, USER_A, 1000,
        ));
        assert_ok!(EntityToken::approve_tokens(
            RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 500,
        ));
        assert_noop!(
            EntityToken::transfer_from(
                RuntimeOrigin::signed(USER_B), SHOP_ID, USER_A, USER_A, 100,
            ),
            Error::<Test>::SelfTransfer
        );
    });
}
