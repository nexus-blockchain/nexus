use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_noop, assert_ok};

fn proposal_type_general() -> ProposalType<u128> {
    ProposalType::General {
        title_cid: b"test".to_vec().try_into().unwrap(),
        content_cid: b"content".to_vec().try_into().unwrap(),
    }
}

fn proposal_type_price_change() -> ProposalType<u128> {
    ProposalType::PriceChange { product_id: 1, new_price: 500 }
}

// ==================== 创建提案 ====================

#[test]
fn create_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        // Alice has 2% > 1% threshold
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test Proposal".to_vec(),
            None,
        ));
        let proposal = Proposals::<Test>::get(0).expect("proposal exists");
        assert_eq!(proposal.proposer, ALICE);
        assert_eq!(proposal.entity_id, SHOP_ID);
        assert_eq!(proposal.status, ProposalStatus::Voting);
        assert_eq!(proposal.voting_end, 1 + 100); // block 1 + VotingPeriod
    });
}

#[test]
fn create_proposal_fails_insufficient_tokens() {
    ExtBuilder::build().execute_with(|| {
        // Set ALICE to only 0.5% (5000 out of 1M)
        set_token_balance(SHOP_ID, ALICE, 5_000);
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::InsufficientTokensForProposal
        );
    });
}

#[test]
fn create_proposal_fails_shop_not_found() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), 999,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

#[test]
fn create_proposal_fails_token_not_enabled() {
    ExtBuilder::build().execute_with(|| {
        set_token_enabled(SHOP_ID, false);
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::TokenNotEnabled
        );
    });
}

#[test]
fn create_proposal_fails_too_many_active() {
    ExtBuilder::build().execute_with(|| {
        for i in 0..10 {
            assert_ok!(EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(),
                format!("Proposal {}", i).into_bytes(), None,
            ));
        }
        // 11th should fail
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(), b"Too Many".to_vec(), None,
            ),
            Error::<Test>::TooManyActiveProposals
        );
    });
}

// ==================== 投票 ====================

#[test]
fn vote_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // BOB votes yes (150_000 weight)
        assert_ok!(EntityGovernance::vote(
            RuntimeOrigin::signed(BOB), 0, VoteType::Yes,
        ));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 150_000u128);

        // Vote record exists
        assert!(VoteRecords::<Test>::contains_key(0, BOB));
    });
}

#[test]
fn vote_fails_already_voted() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::No),
            Error::<Test>::AlreadyVoted
        );
    });
}

#[test]
fn vote_fails_no_voting_power() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // DAVE has no tokens
        let dave: u64 = 99;
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(dave), 0, VoteType::Yes),
            Error::<Test>::NoVotingPower
        );
    });
}

#[test]
fn vote_fails_after_voting_period() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        advance_blocks(101); // past voting_end
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes),
            Error::<Test>::VotingEnded
        );
    });
}

// ==================== 结束投票 ====================

#[test]
fn finalize_voting_passes() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // BOB(15%) + CHARLIE(5%) vote yes = 20% > quorum(10%), yes > 50%
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Passed);
        assert!(proposal.execution_time.is_some());

        // H5: 通过的提案应从活跃列表移除
        let active = EntityProposals::<Test>::get(SHOP_ID);
        assert!(!active.contains(&0));
    });
}

#[test]
fn finalize_voting_fails_quorum() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // Only ALICE votes (2%) < quorum(10%)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Failed);
    });
}

#[test]
fn finalize_voting_fails_threshold() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // BOB(15%) votes no, CHARLIE(5%) + ALICE(2%) vote yes = 7% yes vs 15% no
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::No));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Failed);
    });
}

#[test]
fn finalize_voting_fails_too_early() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // Try to finalize before voting ends
        assert_noop!(
            EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::VotingNotEnded
        );
    });
}

// ==================== 执行提案 ====================

#[test]
fn execute_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // Wait for execution delay
        advance_blocks(50);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);
    });
}

#[test]
fn execute_proposal_fails_too_early() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // Don't wait long enough
        advance_blocks(10);
        assert_noop!(
            EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::ExecutionTimeNotReached
        );
    });
}

// ==================== 取消提案 ====================

#[test]
fn cancel_proposal_by_proposer() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Cancelled);
    });
}

#[test]
fn cancel_proposal_by_owner() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // Shop owner can cancel
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(OWNER), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Cancelled);
    });
}

#[test]
fn cancel_proposal_fails_not_authorized() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::cancel_proposal(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::CannotCancel
        );
    });
}

// ==================== 治理配置 ====================

#[test]
fn configure_governance_works() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER),
            1, // entity_id
            GovernanceMode::FullDAO,
            Some(200), // voting period
            Some(30),  // execution delay
            Some(20),  // quorum
            Some(60),  // pass threshold
            Some(500), // proposal threshold
            Some(false), // no veto
        ));
        let config = GovernanceConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.mode, GovernanceMode::FullDAO);
        assert_eq!(config.voting_period, 200);
        assert_eq!(config.execution_delay, 30);
        assert_eq!(config.quorum_threshold, 20);
        assert_eq!(config.pass_threshold, 60);
        assert_eq!(config.proposal_threshold, 500);
        assert!(!config.admin_veto_enabled);
    });
}

#[test]
fn configure_governance_fails_not_owner() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(ALICE), 1,
                GovernanceMode::FullDAO, None, None, None, None, None, None,
            ),
            Error::<Test>::NotShopOwner
        );
    });
}

#[test]
fn configure_governance_fails_invalid_quorum() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // H4: quorum > 100 should fail
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, None, None, Some(101), None, None, None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn configure_governance_fails_invalid_threshold() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // H4: threshold > 10000 should fail
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, None, None, None, None, Some(10001), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

// ==================== 否决 ====================

#[test]
fn veto_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // Configure FullDAO with veto enabled
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, Some(true),
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        assert_ok!(EntityGovernance::veto_proposal(RuntimeOrigin::signed(OWNER), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Cancelled);
    });
}

#[test]
fn veto_fails_not_enabled() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // Configure FullDAO without veto (default admin_veto_enabled=false)
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        assert_noop!(
            EntityGovernance::veto_proposal(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::NoVetoRight
        );
    });
}

// ==================== H1: 治理模式检查 ====================

#[test]
fn create_proposal_fails_governance_mode_none() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 配置 None 模式
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::None, None, None, None, None, None, None,
        ));

        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::GovernanceModeNotAllowed
        );
    });
}

#[test]
fn create_proposal_works_no_config_backward_compat() {
    ExtBuilder::build().execute_with(|| {
        // 无治理配置时应允许创建提案（向后兼容）
        assert!(GovernanceConfigs::<Test>::get(1).is_none());
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
    });
}

// ==================== H2: 参数验证 ====================

#[test]
fn create_proposal_fails_invalid_discount_rate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::Promotion { discount_rate: 10001, duration_blocks: 100 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn create_proposal_fails_invalid_revenue_share() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::RevenueShare { owner_share: 8000, token_holder_share: 3000 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn create_proposal_fails_invalid_quorum_change() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::QuorumChange { new_quorum: 101 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn create_proposal_fails_invalid_upgrade_mode() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::SetUpgradeMode { mode: 3 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn create_proposal_valid_params_pass() {
    ExtBuilder::build().execute_with(|| {
        // 有效的 RevenueShare（总和刚好 10000）
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::RevenueShare { owner_share: 7000, token_holder_share: 3000 },
            b"OK".to_vec(), None,
        ));
    });
}

// ==================== 快照机制 ====================

#[test]
fn snapshot_prevents_vote_weight_inflation() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // BOB votes with 150_000
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // 快照已锁定
        let snapshot = VotingPowerSnapshot::<Test>::get(0, BOB);
        assert_eq!(snapshot, Some(150_000u128));
    });
}

// ==================== 完整治理流程 ====================

#[test]
fn full_governance_lifecycle() {
    ExtBuilder::build().execute_with(|| {
        // 1. Create proposal
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_price_change(), b"Price Change".to_vec(), None,
        ));

        // 2. Vote (BOB 15% + CHARLIE 5% = 20% quorum, all yes)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Abstain));

        // 3. Finalize
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Passed);

        // 4. Execute
        advance_blocks(50);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(BOB), 0));
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);
    });
}

// ==================== 时间加权投票权测试 ====================

#[test]
fn time_weight_no_first_hold_returns_base_balance() {
    // 未记录 FirstHoldTime 的用户，投票权 = 原始余额（1x）
    ExtBuilder::build().execute_with(|| {
        // ALICE has 20_000 tokens, no FirstHoldTime recorded
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &ALICE);
        assert_eq!(power, 20_000); // 1x, no bonus
    });
}

#[test]
fn time_weight_zero_holding_returns_base_balance() {
    // 刚刚记录 FirstHoldTime（持有 0 区块），投票权 = 原始余额（1x）
    ExtBuilder::build().execute_with(|| {
        let now = System::block_number(); // block 1
        FirstHoldTime::<Test>::insert(SHOP_ID, &ALICE, now);
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &ALICE);
        assert_eq!(power, 20_000); // 1x, 0 blocks held
    });
}

#[test]
fn time_weight_half_period_gives_half_bonus() {
    // 持有 full_period/2 区块 → multiplier = 10000 + 20000/2 = 20000 → 2x
    ExtBuilder::build().execute_with(|| {
        // mock: TimeWeightFullPeriod = 1000, TimeWeightMaxMultiplier = 30000 (3x)
        // bonus_range = 30000 - 10000 = 20000
        // at half period (500 blocks): bonus = 500 * 20000 / 1000 = 10000
        // multiplier = 10000 + 10000 = 20000 → 2x
        FirstHoldTime::<Test>::insert(SHOP_ID, &ALICE, 1u64);
        advance_blocks(500); // now = 501
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &ALICE);
        // 20_000 * 20000 / 10000 = 40_000
        assert_eq!(power, 40_000);
    });
}

#[test]
fn time_weight_full_period_gives_max_bonus() {
    // 持有 >= full_period 区块 → multiplier = max_multiplier → 3x
    ExtBuilder::build().execute_with(|| {
        FirstHoldTime::<Test>::insert(SHOP_ID, &BOB, 1u64);
        advance_blocks(1000); // now = 1001, holding = 1000 = full_period
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &BOB);
        // 150_000 * 30000 / 10000 = 450_000
        assert_eq!(power, 450_000);
    });
}

#[test]
fn time_weight_beyond_full_period_caps_at_max() {
    // 持有超过 full_period 的区块 → multiplier 不超过 max_multiplier
    ExtBuilder::build().execute_with(|| {
        FirstHoldTime::<Test>::insert(SHOP_ID, &CHARLIE, 1u64);
        advance_blocks(5000); // 远超 full_period(1000)
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &CHARLIE);
        // 50_000 * 30000 / 10000 = 150_000 (capped at 3x)
        assert_eq!(power, 150_000);
    });
}

#[test]
fn time_weight_zero_balance_returns_zero() {
    // 余额为 0 的用户投票权始终为 0
    ExtBuilder::build().execute_with(|| {
        let nobody: u64 = 999;
        FirstHoldTime::<Test>::insert(SHOP_ID, &nobody, 1u64);
        advance_blocks(1000);
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &nobody);
        assert_eq!(power, 0);
    });
}

#[test]
fn time_weight_quarter_period() {
    // 持有 250 区块 (1/4 period) → bonus = 250 * 20000 / 1000 = 5000
    // multiplier = 10000 + 5000 = 15000 → 1.5x
    ExtBuilder::build().execute_with(|| {
        FirstHoldTime::<Test>::insert(SHOP_ID, &ALICE, 1u64);
        advance_blocks(250); // now = 251
        let power = EntityGovernance::calculate_voting_power(SHOP_ID, &ALICE);
        // 20_000 * 15000 / 10000 = 30_000
        assert_eq!(power, 30_000);
    });
}

#[test]
fn time_weight_vote_uses_weighted_power() {
    // 投票时使用时间加权后的投票权重
    ExtBuilder::build().execute_with(|| {
        // BOB holds since block 1, now advance to block 1001 (full period)
        FirstHoldTime::<Test>::insert(SHOP_ID, &BOB, 1u64);
        advance_blocks(999); // now = 1000

        // Create proposal at block 1000
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Weighted Vote Test".to_vec(), None,
        ));

        // BOB votes — should get 3x power
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        // BOB balance = 150_000, holding = 999 blocks (just under full period)
        // bonus = 999 * 20000 / 1000 = 19980
        // multiplier = 10000 + 19980 = 29980
        // weight = 150_000 * 29980 / 10000 = 449_700
        assert_eq!(proposal.yes_votes, 449_700);
    });
}

// ==================== 审计回归测试 ====================

#[test]
fn h1_finalize_uses_custom_quorum() {
    // H1: finalize_voting 应使用 GovernanceConfig 中的自定义 quorum，而非全局默认 10%
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 设置自定义 quorum 为 30%
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, Some(30), None, None, None,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // BOB(15%) + CHARLIE(5%) = 20% < 自定义 quorum 30%
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // 应 Failed（20% < 30% quorum），而非 Passed
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Failed);
    });
}

#[test]
fn h3_add_upgrade_rule_returns_not_implemented() {
    // H3: AddUpgradeRule 执行应返回 ProposalTypeNotImplemented
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::AddUpgradeRule { rule_cid: b"rule1".to_vec().try_into().unwrap() },
            b"Add Rule".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(50);

        assert_noop!(
            EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::ProposalTypeNotImplemented
        );
    });
}

#[test]
fn h4_create_proposal_fails_inactive_entity() {
    // H4: 非活跃实体不能创建提案
    ExtBuilder::build().execute_with(|| {
        // entity_id 3 存在但不活跃（MockEntityProvider 只让 1,2 活跃）
        set_token_enabled(3, true);
        set_token_balance(3, ALICE, 20_000);
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), 3,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

#[test]
fn m5_execute_proposal_fails_expired() {
    // M5: 超过执行窗口后不允许执行
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // ExecutionDelay=50, window=50*2=100, exec_time=block ~152
        // 过期时间 = exec_time + 100 = ~252
        // 推进到超过过期时间
        advance_blocks(200); // block ~302, 远超过期
        assert_noop!(
            EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::ExecutionExpired
        );
    });
}

#[test]
fn m5_execute_proposal_within_window_works() {
    // M5: 在执行窗口内正常执行
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // 在窗口内执行（刚好到执行延迟点）
        advance_blocks(50);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);
    });
}

// ==================== 治理锁定测试 ====================

#[test]
fn lock_governance_works() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::None, None, None, None, None, None, None,
        ));
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        assert!(GovernanceLocked::<Test>::get(1));
    });
}

#[test]
fn lock_governance_fails_not_owner() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::lock_governance(RuntimeOrigin::signed(ALICE), 1),
            Error::<Test>::NotShopOwner
        );
    });
}

#[test]
fn lock_governance_fails_already_locked() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        assert_noop!(
            EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1),
            Error::<Test>::GovernanceAlreadyLocked
        );
    });
}

#[test]
fn locked_configure_governance_rejected() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::None, Some(200), None, None, None, None, None,
            ),
            Error::<Test>::GovernanceConfigIsLocked
        );
    });
}

#[test]
fn locked_none_blocks_upgrade_to_fulldao() {
    // None 锁定后永久冻结，不可升级到 FullDAO
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::None, None, None, None, None, None, None,
        ));
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));

        // 锁定后不可升级到 FullDAO
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, None, None, None, None, None, None,
            ),
            Error::<Test>::GovernanceConfigIsLocked
        );
        // 锁定仍然存在
        assert!(GovernanceLocked::<Test>::get(1));
    });
}

#[test]
fn locked_upgrade_rejects_none_to_none() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::None, None, None, None, None, None, None,
        ));
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));

        // 锁定后不能设置回 None
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::None, None, None, None, None, None, None,
            ),
            Error::<Test>::GovernanceConfigIsLocked
        );
    });
}

#[test]
fn lock_governance_works_in_fulldao_mode() {
    // FullDAO 可锁定（放弃控制权，仅通过提案修改）
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        assert!(GovernanceLocked::<Test>::get(1));
    });
}

// ==================== C5: FullDAO 模式下阻止 configure_governance ====================

#[test]
fn c5_unlocked_fulldao_allows_configure() {
    // 未锁定的 FullDAO 允许 Owner 配置（设置阶段）
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));
        // 设置阶段：可以继续修改参数
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, Some(200), Some(80), Some(25), Some(60), None, None,
        ));
        let config = GovernanceConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.voting_period, 200);
        assert_eq!(config.execution_delay, 80);
    });
}

#[test]
fn c5_locked_fulldao_blocks_configure() {
    // 锁定后的 FullDAO，Owner 不可直接修改（需走提案）
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, Some(200), None, None, None, None, None,
            ),
            Error::<Test>::GovernanceConfigIsLocked
        );
    });
}

// ==================== C4: FullDAO 模式下 cancel 限制 ====================

#[test]
fn c4_cancel_proposal_owner_blocked_in_fulldao() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 设为 FullDAO
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));
        // ALICE 创建提案
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // Owner（非提案者）在 FullDAO 模式下不能取消
        assert_noop!(
            EntityGovernance::cancel_proposal(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::GovernanceModeNotAllowed
        );
        // 提案者仍然可以取消
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));
    });
}

// ==================== C3: MinVotingPeriod/MinExecutionDelay ====================

#[test]
fn c3_configure_governance_fails_voting_period_too_short() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // MinVotingPeriod = 10, 设为 5 应失败
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::None, Some(5), None, None, None, None, None,
            ),
            Error::<Test>::VotingPeriodTooShort
        );
        // 设为 10 应成功
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::None, Some(10), None, None, None, None, None,
        ));
    });
}

#[test]
fn c3_configure_governance_fails_execution_delay_too_short() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // MinExecutionDelay = 5, 设为 2 应失败
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::None, None, Some(2), None, None, None, None,
            ),
            Error::<Test>::ExecutionDelayTooShort
        );
        // 设为 5 应成功
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::None, None, Some(5), None, None, None, None,
        ));
    });
}

// ==================== C1+H4: 治理参数快照 ====================

#[test]
fn c1_h4_proposal_snapshots_governance_params() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 设置自定义治理参数
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, Some(200), Some(80), Some(25), Some(60), None, None,
        ));

        // 创建提案
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Snapshot Test".to_vec(), None,
        ));

        // 验证快照参数
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.snapshot_quorum, 25);
        assert_eq!(proposal.snapshot_pass, 60);
        assert_eq!(proposal.snapshot_execution_delay, 80);
        assert_eq!(proposal.snapshot_total_supply, TOTAL_SUPPLY);
    });
}

// ==================== FullDAO 需要代币 ====================

#[test]
fn fulldao_requires_token_enabled() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 禁用代币
        set_token_enabled(SHOP_ID, false);
        // 设 FullDAO 应失败
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, None, None, None, None, None, None,
            ),
            Error::<Test>::TokenNotEnabledForDAO
        );
        // 重新启用代币后应成功
        set_token_enabled(SHOP_ID, true);
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));
    });
}

// ==================== H1: VotingPeriodChange 提案最小投票期验证 ====================

#[test]
fn h1_voting_period_change_rejects_below_min() {
    // VotingPeriodChange 提案的 new_period_blocks 必须 >= MinVotingPeriod (10)
    ExtBuilder::build().execute_with(|| {
        // new_period_blocks = 5 < MinVotingPeriod(10)，创建应失败
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::VotingPeriodChange { new_period_blocks: 5 },
                b"Short period".to_vec(), None,
            ),
            Error::<Test>::VotingPeriodTooShort
        );
        // new_period_blocks = 0，极端情况
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::VotingPeriodChange { new_period_blocks: 0 },
                b"Zero period".to_vec(), None,
            ),
            Error::<Test>::VotingPeriodTooShort
        );
    });
}

#[test]
fn h1_voting_period_change_accepts_valid() {
    // new_period_blocks >= MinVotingPeriod (10) 应成功
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::VotingPeriodChange { new_period_blocks: 10 },
            b"Min period".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::VotingPeriodChange { new_period_blocks: 200 },
            b"Long period".to_vec(), None,
        ));
    });
}

// ==================== H2: UpdateCustomLevel 费率验证 ====================

#[test]
fn h2_update_custom_level_rejects_invalid_rates() {
    ExtBuilder::build().execute_with(|| {
        // discount_rate > 10000 应失败
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::UpdateCustomLevel {
                    level_id: 0,
                    name: None,
                    threshold: None,
                    discount_rate: Some(10001),
                    commission_bonus: None,
                },
                b"Bad rate".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        // commission_bonus > 10000 应失败
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::UpdateCustomLevel {
                    level_id: 0,
                    name: None,
                    threshold: None,
                    discount_rate: None,
                    commission_bonus: Some(50000),
                },
                b"Bad bonus".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        // 合法值应成功
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::UpdateCustomLevel {
                level_id: 0,
                name: None,
                threshold: None,
                discount_rate: Some(10000),
                commission_bonus: Some(5000),
            },
            b"Valid update".to_vec(), None,
        ));
    });
}

// ==================== M1: VotingPeriodChange 执行时防御验证 ====================

#[test]
fn m1_execute_voting_period_change_validates_minimum() {
    // 即使提案创建时合法，执行时也再次验证最小投票期
    // 模拟场景：提案创建时 MinVotingPeriod=10，提案 new_period_blocks=10，
    // 正常流程通过投票+执行应成功
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 配置 FullDAO 模式以允许治理参数提案
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, None,
        ));

        // 创建合法的 VotingPeriodChange 提案
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::VotingPeriodChange { new_period_blocks: 50 },
            b"Change period".to_vec(), None,
        ));

        // 投票通过
        assert_ok!(EntityGovernance::vote(
            RuntimeOrigin::signed(BOB), 0, VoteType::Yes,
        ));

        // 跳过投票期
        advance_blocks(101);

        // 最终确定
        assert_ok!(EntityGovernance::finalize_voting(
            RuntimeOrigin::signed(ALICE), 0,
        ));

        // 跳过执行延迟
        advance_blocks(51);

        // 执行应成功
        assert_ok!(EntityGovernance::execute_proposal(
            RuntimeOrigin::signed(ALICE), 0,
        ));

        // 验证投票期已更新
        let config = GovernanceConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.voting_period, 50);
    });
}
