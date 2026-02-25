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
            Error::<Test>::ShopNotFound
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
            Error::<Test>::ShopNotActive
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
            Error::<Test>::NotShopOwner
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
fn claim_dividend_fails_exceeds_max_supply() {
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
        assert_ok!(EntityToken::distribute_dividend(
            RuntimeOrigin::signed(OWNER), SHOP_ID, 100, vec![(USER_A, 100)],
        ));
        assert_noop!(
            EntityToken::claim_dividend(RuntimeOrigin::signed(USER_A), SHOP_ID),
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
        assert_eq!(config.transfer_restriction, TransferRestrictionMode::from_u8(TokenType::Equity.default_transfer_restriction()));
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
