use crate::{mock::*, Error, Event};
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
        assert_noop!(
            EntityToken::create_shop_token(
                RuntimeOrigin::signed(OWNER),
                999,
                b"T".to_vec(),
                b"T".to_vec(),
                18, 500, 1000,
            ),
            Error::<Test>::EntityNotFound
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
            Error::<Test>::NotEntityOwner
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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_A), 1000);
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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_A), 600);
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_B), 400);
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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_A), 100);
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
        let wl = EntityToken::transfer_whitelist(SHOP_ID);
        assert_eq!(wl.len(), 2);

        assert_ok!(EntityToken::remove_from_whitelist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        let wl = EntityToken::transfer_whitelist(SHOP_ID);
        assert_eq!(wl.len(), 1);
    });
}

#[test]
fn blacklist_management_works() {
    new_test_ext().execute_with(|| {
        setup_token();
        assert_ok!(EntityToken::add_to_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        let bl = EntityToken::transfer_blacklist(SHOP_ID);
        assert_eq!(bl.len(), 1);

        assert_ok!(EntityToken::remove_from_blacklist(
            RuntimeOrigin::signed(OWNER), SHOP_ID, vec![USER_A],
        ));
        let bl = EntityToken::transfer_blacklist(SHOP_ID);
        assert_eq!(bl.len(), 0);
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
        // USER_B 不在白名单
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverNotInWhitelist
        );
        // 加入白名单后可以
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
        // USER_B KYC level = 0
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverKycInsufficient
        );
        // 设置 KYC level = 3
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
        // USER_B 不是成员
        assert_noop!(
            EntityToken::transfer_tokens(
                RuntimeOrigin::signed(USER_A), SHOP_ID, USER_B, 100,
            ),
            Error::<Test>::ReceiverNotMember
        );
        // 设为成员
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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_B), 300);
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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_A), 40);

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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_A), 50);
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
        assert_eq!(EntityToken::get_balance(SHOP_ID, &USER_A), 1100); // 1000 + 100 dividend
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
