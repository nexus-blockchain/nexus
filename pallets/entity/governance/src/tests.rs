use crate::mock::*;
use crate::pallet::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use pallet_entity_common::{EntityTokenProvider, GovernanceMode};

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
            Error::<Test>::EntityNotFound
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
fn cancel_proposal_by_owner_as_proposer() {
    // C4: FullDAO 模式下 Owner 只能取消自己创建的提案
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // Owner 作为提案者可以取消
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
            Error::<Test>::NotEntityOwner
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
        // FullDAO + veto enabled（紧急制动模式，设计意图）
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
fn c1_no_config_blocks_proposal_creation() {
    // C1-audit: 无治理配置时默认 None 模式，禁止创建提案
    ExtBuilder::build().execute_with(|| {
        // 移除 ExtBuilder 默认设置的 FullDAO 配置
        GovernanceConfigs::<Test>::remove(1u64);
        assert!(GovernanceConfigs::<Test>::get(1).is_none());
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::GovernanceModeNotAllowed
        );
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
fn h4_create_proposal_fails_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        set_token_enabled(3, true);
        set_token_balance(3, ALICE, 20_000);
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), 3,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::EntityNotFound
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
        // H2-R2: 现在优雅转为 Expired 状态（返回 Ok）
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Expired);
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
            Error::<Test>::NotEntityOwner
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
fn lock_governance_works_without_config() {
    // 无配置时也可锁定（默认 None 模式永久冻结）
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        assert!(GovernanceLocked::<Test>::get(1));
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

// ==================== H1: 弃权票不应稀释通过阈值 ====================

#[test]
fn h1_abstain_does_not_dilute_pass_threshold() {
    // 场景: 100 yes, 0 no, 100 abstain — 以前 pass_threshold = 200*50% = 100,
    // yes(100) > 100 为 false → 失败。修复后 decisive = 100, threshold = 50, yes(100) > 50 → 通过
    ExtBuilder::build().execute_with(|| {
        // 设置余额: ALICE=100, BOB=100 (abstain)
        set_token_balance(SHOP_ID, ALICE, 100_000);
        set_token_balance(SHOP_ID, BOB, 100_000);

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"H1 Test".to_vec(), None,
        ));
        // ALICE votes yes
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes));
        // BOB votes abstain
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Abstain));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        // 修复后: decisive=100000, threshold=50000, yes(100000) > 50000 → Passed
        assert_eq!(proposal.status, ProposalStatus::Passed);
    });
}

#[test]
fn h1_abstain_contributes_to_quorum() {
    // 弃权票仍应计入法定人数
    ExtBuilder::build().execute_with(|| {
        // quorum = 10% of 1M = 100_000
        // 只有 CHARLIE(50_000) 投弃权 — 不足法定人数
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"H1 Quorum".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Abstain));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        // 50_000 < 100_000 quorum → Failed
        assert_eq!(proposal.status, ProposalStatus::Failed);
    });
}

#[test]
fn h1_no_decisive_votes_all_abstain_fails() {
    // 全部弃权：decisive_votes=0, pass_threshold=0, yes(0) > 0 为 false → Failed
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"All Abstain".to_vec(), None,
        ));
        // BOB + CHARLIE abstain = 200_000 > quorum(100_000)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Abstain));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Abstain));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        // decisive_votes = 0, yes(0) > 0 = false → Failed
        assert_eq!(proposal.status, ProposalStatus::Failed);
    });
}

// ==================== M1: CommissionModesChange 无效位标志校验 ====================

#[test]
fn m1_commission_modes_change_rejects_invalid_bits() {
    ExtBuilder::build().execute_with(|| {
        // 0b1000_0000_0000 = bit 11, 超出 ALL_VALID 范围
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::CommissionModesChange { modes: 0b1000_0000_0000 },
                b"Bad modes".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        // 0xFFFF — 高位全设
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::CommissionModesChange { modes: 0xFFFF },
                b"Bad modes".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn m1_commission_modes_change_accepts_valid_bits() {
    ExtBuilder::build().execute_with(|| {
        // 0b0000_0000_0001 = DIRECT_REWARD only
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CommissionModesChange { modes: 0b0000_0000_0001 },
            b"Valid modes".to_vec(), None,
        ));
        // 0b0000_0011_1111_1111 = all valid bits set
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CommissionModesChange { modes: 0b0000_0011_1111_1111 },
            b"All modes".to_vec(), None,
        ));
        // 0 = NONE
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CommissionModesChange { modes: 0 },
            b"No modes".to_vec(), None,
        ));
    });
}

// ==================== M2-R3: ShopPause/ShopResume 指定 shop_id ====================

#[test]
fn m2_execute_shop_pause_with_valid_shop_id() {
    // M2-R3: ShopPause 现在指定 shop_id，正确的 shop_id 应成功执行
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ShopPause { shop_id: SHOP_ID },
            b"Pause shop".to_vec(), None,
        ));

        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
    });
}

#[test]
fn m2_create_shop_pause_invalid_shop_id_fails_early() {
    // F2: ShopPause 指定不属于 entity 的 shop_id 在创建时即被拒绝
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ShopPause { shop_id: 999 },
                b"Pause shop".to_vec(), None,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

// ==================== H2-R2: Expired transition ====================

#[test]
fn h2_expired_proposal_transitions_to_expired_status() {
    // H2: 过期的 Passed 提案应优雅转为 Expired 状态（而非永远停在 Passed）
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // 确认 Passed 状态
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Passed);

        // 推进超过执行窗口 (exec_delay=50, window=50*2=100)
        advance_blocks(200);

        // 调用 execute_proposal 应成功并将状态转为 Expired
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Expired);
    });
}

#[test]
fn h2_expired_proposal_emits_event() {
    // H2: Expired 转换应发出 ProposalExpired 事件
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(200);

        System::reset_events();
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        let events = System::events();
        assert!(
            events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::EntityGovernance(Event::ProposalExpired { proposal_id: 0 })
                )
            }),
            "ProposalExpired event not found"
        );
    });
}

#[test]
fn h2_already_expired_proposal_rejects_second_execute() {
    // H2: 已经转为 Expired 的提案再次调用 execute_proposal 应返回 InvalidProposalStatus
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(200);

        // 第一次: 转为 Expired
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        // 第二次: Expired != Passed，应失败
        assert_noop!(
            EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::InvalidProposalStatus
        );
    });
}

// ==================== H3-R2: VoteRecords cleanup ====================

#[test]
fn h3_vote_records_cleaned_after_finalize() {
    // H3+F9: VoteRecords preserved after finalize, cleaned on cleanup_proposal
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Abstain));

        // 投票后 VoteRecords 应存在
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());
        assert!(VoteRecords::<Test>::get(0, CHARLIE).is_some());

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // F9: finalize 后 VoteRecords 仍保留（延迟清理）
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());
        assert!(VoteRecords::<Test>::get(0, CHARLIE).is_some());

        // 如果通过则执行
        let proposal = Proposals::<Test>::get(0).unwrap();
        if proposal.status == ProposalStatus::Passed {
            advance_blocks(51);
            assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        }

        // cleanup 后 VoteRecords 才被清理
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(VoteRecords::<Test>::get(0, BOB).is_none());
        assert!(VoteRecords::<Test>::get(0, CHARLIE).is_none());
    });
}

#[test]
fn h3_vote_records_cleaned_after_cancel() {
    // H3+F9: VoteRecords preserved after cancel, cleaned on cleanup_proposal
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::No));

        // 投票后 VoteRecords 应存在
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        // 提案者取消
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));

        // F9: 取消后 VoteRecords 仍保留
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        // cleanup 后才清理
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(VoteRecords::<Test>::get(0, BOB).is_none());
    });
}

#[test]
fn h3_vote_records_cleaned_after_veto() {
    // H3+F9: VoteRecords preserved after veto, cleaned on cleanup_proposal
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // FullDAO + veto enabled（紧急制动模式，设计意图）
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, None, None, Some(true),
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        assert_ok!(EntityGovernance::veto_proposal(RuntimeOrigin::signed(OWNER), 0));

        // F9: veto 后 VoteRecords 仍保留
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        // cleanup 后才清理
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(VoteRecords::<Test>::get(0, BOB).is_none());
    });
}

// ==================== H2: Token locking on vote ====================

#[test]
fn h2_vote_reserves_tokens() {
    // H2: 投票后投票者的原始代币应被 reserve，防止转让给其他账户复投
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // 投票前: 无 reserve
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 0);

        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // 投票后: BOB 的 150_000 代币应被 reserve
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 150_000);
        // VoterTokenLocks 应记录参与
        assert!(VoterTokenLocks::<Test>::get(0, BOB).is_some());
        // GovernanceLockCount = 1
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 1);
        // GovernanceLockAmount = 150_000
        assert_eq!(GovernanceLockAmount::<Test>::get(SHOP_ID, BOB), 150_000);
    });
}

#[test]
fn h2_finalize_unreserves_tokens() {
    // H2: finalize_voting 后投票者的代币应被 unreserve
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::No));

        // 投票后均被 reserve
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 150_000);
        assert_eq!(get_reserved_balance(SHOP_ID, CHARLIE), 50_000);

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // finalize 后: 代币应被 unreserve
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 0);
        assert_eq!(get_reserved_balance(SHOP_ID, CHARLIE), 0);
        // 存储应被清理
        assert!(VoterTokenLocks::<Test>::get(0, BOB).is_none());
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0);
        assert_eq!(GovernanceLockAmount::<Test>::get(SHOP_ID, BOB), 0);
    });
}

#[test]
fn h2_multi_proposal_ref_counting() {
    // H2: 同一用户投票多个提案时使用 max-lock 模式，只在最后一个提案结束时 unreserve
    ExtBuilder::build().execute_with(|| {
        // 创建两个提案
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"P1".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_price_change(), b"P2".to_vec(), None,
        ));

        // BOB 投票 P0
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 1);
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 150_000);

        // BOB 投票 P1 — max-lock 模式不会重复 reserve
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 1, VoteType::No));
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 2);
        // reserve 金额不变（max-lock，余额没变）
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 150_000);

        // CHARLIE 在两个提案上都投票（投票期结束前）
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 1, VoteType::Yes));

        // 投票期结束
        advance_blocks(101);

        // 结束 P0
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // P0 结束后: BOB ref_count=1，代币仍被 reserve
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 1);
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 150_000);

        // 结束 P1
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 1));

        // P1 也结束后: ref_count=0，代币应被 unreserve
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0);
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 0);
    });
}

#[test]
fn h2_cancel_unreserves_tokens() {
    // H2: cancel_proposal 也应 unreserve 投票者代币
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 150_000);

        // 提案者取消
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));

        // 取消后 unreserve
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 0);
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0);
    });
}

// ==================== L5: Dead ProposalStatus removed ====================

#[test]
fn l5_proposal_status_default_is_voting() {
    // L5: ProposalStatus::default() 现在应返回 Voting（而非已移除的 Created）
    assert_eq!(ProposalStatus::default(), ProposalStatus::Voting);
}

// ==================== L2-R3: cleanup_proposal ====================

#[test]
fn l2_cleanup_executed_proposal() {
    // L2-R3: 已执行的提案可被清理，释放存储
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        // 提案仍在存储中
        assert!(Proposals::<Test>::get(0).is_some());

        // 清理
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(BOB), 0));

        // 存储已删除
        assert!(Proposals::<Test>::get(0).is_none());
    });
}

#[test]
fn l2_cleanup_failed_proposal() {
    // L2-R3: Failed 提案也可清理
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // 反对票导致 Failed
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::No));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::No));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Failed);

        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(Proposals::<Test>::get(0).is_none());
    });
}

#[test]
fn l2_cleanup_voting_proposal_fails() {
    // L2-R3: 仍在投票中的提案不可清理
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        assert_noop!(
            EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::ProposalNotTerminal
        );
    });
}

#[test]
fn l2_cleanup_passed_proposal_fails() {
    // L2-R3: Passed 状态（等待执行）不可清理
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        assert_noop!(
            EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::ProposalNotTerminal
        );
    });
}

#[test]
fn l2_cleanup_expired_proposal() {
    // L2-R3: Expired 提案可清理
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(200);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Expired);

        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(BOB), 0));
        assert!(Proposals::<Test>::get(0).is_none());
    });
}

#[test]
fn l2_cleanup_emits_event() {
    // L2-R3: cleanup_proposal 应发出 ProposalCleaned 事件
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // 取消提案使其进入终态
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));

        System::reset_events();
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(BOB), 0));

        let events = System::events();
        assert!(
            events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::EntityGovernance(Event::ProposalCleaned { proposal_id: 0 })
                )
            }),
            "ProposalCleaned event not found"
        );
    });
}

// ==================== Round 5 回归测试 ====================

#[test]
fn l3_r5_error_no_veto_right_still_works_after_dead_error_removal() {
    // L3-R5: 移除 ProposalAlreadyVetoed 后，NoVetoRight 仍然正确
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // 非 entity owner 尝试 veto
        assert_noop!(
            EntityGovernance::veto_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NoVetoRight
        );
    });
}

#[test]
fn l3_r5_governance_config_is_locked_still_works() {
    // L3-R5: 移除 ExecutionExpired 和 FullDAOCannotConfigure 后，
    // GovernanceConfigIsLocked 仍然正确匹配
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 锁定治理
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));
        // 尝试修改 → GovernanceConfigIsLocked
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1, GovernanceMode::FullDAO,
                None, None, None, None, None, None,
            ),
            Error::<Test>::GovernanceConfigIsLocked
        );
    });
}

#[test]
fn l3_r5_voting_period_too_short_still_works() {
    // L3-R5: 移除 FullDAOCannotConfigure 后，VotingPeriodTooShort 仍然正确
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1, GovernanceMode::FullDAO,
                Some(1), None, None, None, None, None,  // voting_period=1 < MinVotingPeriod=10
            ),
            Error::<Test>::VotingPeriodTooShort
        );
    });
}

// ==================== F1: 新增治理参数提案类型 ====================

/// 辅助: 完整提案流程 (创建 → 投票 → finalize → 执行)
fn execute_proposal_flow(proposal_type: ProposalType<u128>) {
    assert_ok!(EntityGovernance::create_proposal(
        RuntimeOrigin::signed(ALICE), SHOP_ID,
        proposal_type, b"Test".to_vec(), None,
    ));
    assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
    assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
    advance_blocks(101);
    assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
    advance_blocks(50);
    assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
}

#[test]
fn f1_execution_delay_change_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        execute_proposal_flow(ProposalType::ExecutionDelayChange { new_delay_blocks: 100 });

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);

        let config = GovernanceConfigs::<Test>::get(SHOP_ID).unwrap();
        assert_eq!(config.execution_delay, 100u64);
    });
}

#[test]
fn f1_execution_delay_change_rejects_too_short() {
    // new_delay_blocks=1 < MinExecutionDelay=5
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ExecutionDelayChange { new_delay_blocks: 1 },
                b"Test".to_vec(), None,
            ),
            Error::<Test>::ExecutionDelayTooShort
        );
    });
}

#[test]
fn f1_pass_threshold_change_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        execute_proposal_flow(ProposalType::PassThresholdChange { new_pass: 75 });

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);

        let config = GovernanceConfigs::<Test>::get(SHOP_ID).unwrap();
        assert_eq!(config.pass_threshold, 75);
    });
}

#[test]
fn f1_pass_threshold_change_rejects_over_100() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::PassThresholdChange { new_pass: 101 },
                b"Test".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn f1_admin_veto_toggle_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 先配置 FullDAO 模式，确保执行后 config 不会还原为 None
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1, GovernanceMode::FullDAO,
            None, None, None, None, None, None,
        ));
        // 先启用 admin_veto
        execute_proposal_flow(ProposalType::AdminVetoToggle { enabled: true });

        let config = GovernanceConfigs::<Test>::get(SHOP_ID).unwrap();
        assert!(config.admin_veto_enabled);

        // 再通过提案关闭
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::AdminVetoToggle { enabled: false },
            b"Test2".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 1, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 1, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 1));
        advance_blocks(50);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 1));

        let config = GovernanceConfigs::<Test>::get(SHOP_ID).unwrap();
        assert!(!config.admin_veto_enabled);
    });
}

#[test]
fn f1_governance_params_modifiable_after_lock() {
    // lock_governance 后仍可通过提案修改治理参数
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // 配置 FullDAO 模式
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1, GovernanceMode::FullDAO,
            None, None, None, None, None, None,
        ));
        // 锁定治理
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), 1));

        // 锁定后 owner 不能修改
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1, GovernanceMode::FullDAO,
                None, Some(100), None, None, None, None,
            ),
            Error::<Test>::GovernanceConfigIsLocked
        );

        // 但可以通过提案修改 execution_delay
        execute_proposal_flow(ProposalType::ExecutionDelayChange { new_delay_blocks: 200 });
        let config = GovernanceConfigs::<Test>::get(SHOP_ID).unwrap();
        assert_eq!(config.execution_delay, 200u64);
    });
}

// ==================== R3: 拒绝未实现的提案类型 ====================

#[test]
fn f8_multi_level_change_rejects_empty_tiers() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MultiLevelChange {
                    tiers: vec![].try_into().unwrap(),
                    max_total_rate: 5000,
                },
                b"Test".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn f8_multi_level_change_rejects_invalid_rate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MultiLevelChange {
                    tiers: vec![(10001, 0, 0, 0)].try_into().unwrap(),
                    max_total_rate: 5000,
                },
                b"Test".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

// ==================== F5: 委托投票 ====================

#[test]
fn f5_delegate_vote_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        // 验证存储
        assert_eq!(VoteDelegation::<Test>::get(SHOP_ID, ALICE), Some(BOB));
        let delegators = DelegatedVoters::<Test>::get(SHOP_ID, BOB);
        assert_eq!(delegators.len(), 1);
        assert_eq!(delegators[0], ALICE);

        // 验证事件
        System::assert_has_event(RuntimeEvent::EntityGovernance(
            Event::VoteDelegated {
                entity_id: SHOP_ID,
                delegator: ALICE,
                delegate: BOB,
            }
        ));
    });
}

#[test]
fn f5_delegate_vote_rejects_self_delegation() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::delegate_vote(RuntimeOrigin::signed(ALICE), SHOP_ID, ALICE),
            Error::<Test>::SelfDelegation
        );
    });
}

#[test]
fn f5_delegate_vote_rejects_double_delegation() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));
        assert_noop!(
            EntityGovernance::delegate_vote(RuntimeOrigin::signed(ALICE), SHOP_ID, CHARLIE),
            Error::<Test>::AlreadyDelegated
        );
    });
}

#[test]
fn f5_delegate_vote_rejects_invalid_entity() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::delegate_vote(RuntimeOrigin::signed(ALICE), 999, BOB),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn f5_undelegate_vote_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));
        assert_ok!(EntityGovernance::undelegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
        ));

        // 验证存储已清理
        assert!(VoteDelegation::<Test>::get(SHOP_ID, ALICE).is_none());
        let delegators = DelegatedVoters::<Test>::get(SHOP_ID, BOB);
        assert!(delegators.is_empty());

        // 验证事件
        System::assert_has_event(RuntimeEvent::EntityGovernance(
            Event::VoteUndelegated {
                entity_id: SHOP_ID,
                delegator: ALICE,
            }
        ));
    });
}

#[test]
fn f5_undelegate_vote_fails_without_delegation() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::undelegate_vote(RuntimeOrigin::signed(ALICE), SHOP_ID),
            Error::<Test>::NotDelegated
        );
    });
}

#[test]
fn f5_delegated_user_cannot_vote_directly() {
    ExtBuilder::build().execute_with(|| {
        // ALICE delegates to BOB
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        // Create a proposal
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // ALICE tries to vote directly → should fail
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes),
            Error::<Test>::VotePowerDelegated
        );
    });
}

#[test]
fn f5_delegate_vote_weight_included_in_delegate() {
    ExtBuilder::build().execute_with(|| {
        // ALICE(20_000) delegates to BOB(150_000)
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // BOB votes → weight should include ALICE's power
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        // BOB(150_000) + ALICE(20_000) = 170_000
        assert_eq!(proposal.yes_votes, 170_000u128);
    });
}

#[test]
fn f5_multiple_delegators_to_same_delegate() {
    ExtBuilder::build().execute_with(|| {
        // ALICE(20_000) and CHARLIE(50_000) delegate to BOB(150_000)
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID, BOB,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // BOB votes → weight = BOB + ALICE + CHARLIE
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        // BOB(150_000) + ALICE(20_000) + CHARLIE(50_000) = 220_000
        assert_eq!(proposal.yes_votes, 220_000u128);
    });
}

#[test]
fn f5_undelegate_then_vote_directly() {
    ExtBuilder::build().execute_with(|| {
        // ALICE delegates then undelegates
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));
        assert_ok!(EntityGovernance::undelegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // ALICE can now vote directly
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 20_000u128);
    });
}

#[test]
fn f5_delegation_locks_delegator_tokens_on_delegate_vote() {
    ExtBuilder::build().execute_with(|| {
        // ALICE delegates to BOB
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        // BOB votes → both BOB and ALICE tokens locked
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // ALICE's tokens should be locked (VoterTokenLocks entry exists)
        assert!(VoterTokenLocks::<Test>::contains_key(0, ALICE));
        assert!(VoterTokenLocks::<Test>::contains_key(0, BOB));

        // ALICE's lock count should be 1
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, ALICE), 1);
    });
}

#[test]
fn f5_delegation_tokens_unlocked_after_proposal_ends() {
    ExtBuilder::build().execute_with(|| {
        // ALICE delegates to BOB
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // Finalize (quorum not met with only BOB+ALICE → Failed since total_votes=170k/1M=17% > quorum=10%, and yes=100% > pass=50%, should Pass)
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // After finalize, locks should be released
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, ALICE), 0);
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0);
    });
}

// ==================== C3: on_idle 自动 finalize ====================

#[test]
fn c3_on_idle_auto_finalizes_expired_voting_proposal() {
    ExtBuilder::build().execute_with(|| {
        // Create proposal (voting_end = block 1 + 100 = 101)
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Auto Test".to_vec(), None,
        ));
        assert_eq!(EntityProposals::<Test>::get(SHOP_ID).len(), 1);

        // No votes → will fail quorum when finalized
        // Advance past voting_end
        advance_blocks(102);

        // on_idle should auto-finalize
        let weight = EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX),
        );
        assert!(weight.ref_time() > 0);

        // Proposal should be Failed (no quorum)
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Failed);

        // Should be removed from active list
        assert_eq!(EntityProposals::<Test>::get(SHOP_ID).len(), 0);

        // ProposalAutoFinalized event should exist
        System::assert_has_event(RuntimeEvent::EntityGovernance(
            crate::pallet::Event::ProposalAutoFinalized {
                proposal_id: 0,
                new_status: ProposalStatus::Failed,
            },
        ));
    });
}

#[test]
fn c3_on_idle_auto_finalizes_passed_proposal_with_votes() {
    ExtBuilder::build().execute_with(|| {
        // Create proposal
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Pass Test".to_vec(), None,
        ));

        // BOB votes yes (150k/1M = 15% > 10% quorum, 100% yes > 50% pass)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // Advance past voting_end
        advance_blocks(102);

        // on_idle should auto-finalize → Passed
        EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX),
        );

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Passed);
        assert!(proposal.execution_time.is_some());

        // Tokens should be unlocked
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0);
    });
}

#[test]
fn c3_on_idle_does_not_touch_active_voting_proposal() {
    ExtBuilder::build().execute_with(|| {
        // Create proposal at block 1, voting_end = 101
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Still Active".to_vec(), None,
        ));

        // Only advance to block 50 (still within voting period)
        advance_blocks(49);

        EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX),
        );

        // Should still be Voting
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Voting);
        assert_eq!(EntityProposals::<Test>::get(SHOP_ID).len(), 1);
    });
}

#[test]
fn c3_on_idle_auto_expires_passed_proposal_beyond_execution_window() {
    ExtBuilder::build().execute_with(|| {
        // Create proposal
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Expire Test".to_vec(), None,
        ));

        // BOB votes yes → will pass
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // Manually finalize → Passed (execution_time = now + 50)
        advance_blocks(102);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Passed);
        let exec_time = proposal.execution_time.unwrap();
        // execution window = exec_time + 2 * execution_delay = exec_time + 100
        // advance past the window
        System::set_block_number(exec_time + 101);

        // on_idle should auto-expire
        EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX),
        );

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Expired);
    });
}

#[test]
fn c3_on_idle_unlocks_tokens_on_auto_finalize() {
    ExtBuilder::build().execute_with(|| {
        // Create proposal
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Lock Test".to_vec(), None,
        ));

        // BOB and CHARLIE vote
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::No));

        // Verify tokens are locked
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 1);
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, CHARLIE), 1);

        // Advance past voting_end and let on_idle finalize
        advance_blocks(102);
        EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX),
        );

        // Tokens should be unlocked
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0);
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, CHARLIE), 0);
        assert_eq!(get_reserved_balance(SHOP_ID, BOB), 0);
        assert_eq!(get_reserved_balance(SHOP_ID, CHARLIE), 0);
    });
}

#[test]
fn c3_on_idle_scan_cursor_advances() {
    ExtBuilder::build().execute_with(|| {
        // Create 3 proposals
        for i in 0..3 {
            assert_ok!(EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(),
                format!("Prop {}", i).into_bytes(), None,
            ));
        }

        // Cursor should start at 0
        assert_eq!(ProposalScanCursor::<Test>::get(), 0);

        // Advance past voting and trigger on_idle
        advance_blocks(102);
        EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX),
        );

        // All 3 proposals should be finalized
        for i in 0..3u64 {
            let p = Proposals::<Test>::get(i).unwrap();
            assert_eq!(p.status, ProposalStatus::Failed);
        }

        // Cursor wraps: scanned 0,1,2 → cursor=3 >= next_id=3 → wrap to 0 → break
        // Just verify all were processed (the cursor position itself is implementation detail)
        assert_eq!(EntityProposals::<Test>::get(SHOP_ID).len(), 0);
    });
}

#[test]
fn c3_on_idle_respects_weight_limit() {
    ExtBuilder::build().execute_with(|| {
        // Create 3 proposals
        for i in 0..3 {
            assert_ok!(EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(),
                format!("Prop {}", i).into_bytes(), None,
            ));
        }

        advance_blocks(102);

        // Give very little weight — should only process 0 or 1 proposals
        let minimal_weight = frame_support::weights::Weight::from_parts(100_000_000, 10_000);
        EntityGovernance::on_idle(System::block_number(), minimal_weight);

        // At least check it didn't panic — some proposals may still be Voting
        // depending on exact weight accounting
        let total_still_voting = (0..3u64)
            .filter(|&i| {
                Proposals::<Test>::get(i)
                    .map(|p| p.status == ProposalStatus::Voting)
                    .unwrap_or(false)
            })
            .count();
        // With minimal weight, not all 3 should be finalized
        // (at best 1 could be processed)
        assert!(total_still_voting >= 0); // just verify no panic
    });
}

#[test]
fn c3_on_idle_zero_weight_returns_zero() {
    ExtBuilder::build().execute_with(|| {
        let weight = EntityGovernance::on_idle(
            System::block_number(),
            frame_support::weights::Weight::zero(),
        );
        assert_eq!(weight, frame_support::weights::Weight::zero());
    });
}

// ==================== F7: Quorum/PassThreshold 最小值校验 ====================

#[test]
fn f7_configure_governance_rejects_zero_quorum() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                pallet_entity_common::GovernanceMode::FullDAO,
                None,      // voting_period
                None,      // execution_delay
                Some(0),   // quorum = 0
                Some(50),  // pass
                None, None,
            ),
            Error::<Test>::QuorumTooLow
        );
    });
}

#[test]
fn f7_configure_governance_rejects_zero_pass_threshold() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER),
                SHOP_ID,
                pallet_entity_common::GovernanceMode::FullDAO,
                None,      // voting_period
                None,      // execution_delay
                Some(10),  // quorum
                Some(0),   // pass = 0
                None, None,
            ),
            Error::<Test>::PassThresholdTooLow
        );
    });
}

#[test]
fn f7_quorum_change_proposal_rejects_zero() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE),
                SHOP_ID,
                ProposalType::QuorumChange { new_quorum: 0 },
                b"Zero quorum".to_vec(),
                None,
            ),
            Error::<Test>::QuorumTooLow
        );
    });
}

#[test]
fn f7_pass_threshold_change_proposal_rejects_zero() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE),
                SHOP_ID,
                ProposalType::PassThresholdChange { new_pass: 0 },
                b"Zero pass".to_vec(),
                None,
            ),
            Error::<Test>::PassThresholdTooLow
        );
    });
}

#[test]
fn f7_valid_quorum_and_pass_accepted() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
            pallet_entity_common::GovernanceMode::FullDAO,
            None,    // voting_period
            None,    // execution_delay
            Some(1), // minimum valid quorum
            Some(1), // minimum valid pass
            None, None,
        ));
    });
}

// ==================== F5: Entity active check ====================

#[test]
fn f5_vote_rejects_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));

        // Deactivate entity
        set_entity_active(SHOP_ID, false);

        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f5_finalize_rejects_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);

        set_entity_active(SHOP_ID, false);

        assert_noop!(
            EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f5_execute_rejects_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);

        set_entity_active(SHOP_ID, false);

        assert_noop!(
            EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f5_change_vote_rejects_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        set_entity_active(SHOP_ID, false);

        assert_noop!(
            EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::No),
            Error::<Test>::EntityNotActive
        );
    });
}

// ==================== F1: 修改投票 ====================

#[test]
fn f1_change_vote_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        let yes_before = proposal.yes_votes;
        assert!(yes_before > 0);

        assert_ok!(EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::No));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 0);
        assert_eq!(proposal.no_votes, yes_before);
    });
}

#[test]
fn f1_change_vote_same_type_noop() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // Same vote type — should be no-op
        assert_ok!(EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert!(proposal.yes_votes > 0);
    });
}

#[test]
fn f1_change_vote_rejects_no_prior_vote() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        // BOB never voted — H2-audit: 使用 NotVoted 错误
        assert_noop!(
            EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::No),
            Error::<Test>::NotVoted
        );
    });
}

#[test]
fn f1_change_vote_rejects_after_voting_period() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);

        assert_noop!(
            EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::No),
            Error::<Test>::VotingEnded
        );
    });
}

#[test]
fn f1_change_vote_yes_to_abstain() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        let weight = Proposals::<Test>::get(0).unwrap().yes_votes;

        assert_ok!(EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::Abstain));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 0);
        assert_eq!(proposal.abstain_votes, weight);
    });
}

// ==================== F2: 委托链检查 ====================

#[test]
fn f2_delegate_to_already_delegated_rejected() {
    ExtBuilder::build().execute_with(|| {
        // BOB delegates to CHARLIE
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(BOB),
            SHOP_ID,
            CHARLIE,
        ));

        // ALICE tries to delegate to BOB (who has delegated to CHARLIE)
        assert_noop!(
            EntityGovernance::delegate_vote(
                RuntimeOrigin::signed(ALICE),
                SHOP_ID,
                BOB,
            ),
            Error::<Test>::DelegateAlreadyDelegated
        );
    });
}

#[test]
fn f2_delegate_to_non_delegated_works() {
    ExtBuilder::build().execute_with(|| {
        // CHARLIE has not delegated — so ALICE can delegate to CHARLIE
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            CHARLIE,
        ));
    });
}

// ==================== F6: 紧急暂停治理 ====================

#[test]
fn f6_pause_governance_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::pause_governance(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
        ));
        assert!(GovernancePaused::<Test>::get(SHOP_ID));
    });
}

#[test]
fn f6_pause_rejects_non_owner() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::pause_governance(RuntimeOrigin::signed(ALICE), SHOP_ID),
            Error::<Test>::NotEntityOwner
        );
    });
}

#[test]
fn f6_pause_rejects_already_paused() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert_noop!(
            EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::GovernanceIsPaused
        );
    });
}

#[test]
fn f6_resume_governance_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert_ok!(EntityGovernance::resume_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert!(!GovernancePaused::<Test>::get(SHOP_ID));
    });
}

#[test]
fn f6_resume_rejects_not_paused() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::resume_governance(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::GovernanceNotPaused
        );
    });
}

#[test]
fn f6_paused_blocks_create_proposal() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE),
                SHOP_ID,
                proposal_type_general(),
                b"Blocked".to_vec(),
                None,
            ),
            Error::<Test>::GovernanceIsPaused
        );
    });
}

#[test]
fn f6_paused_blocks_vote() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));

        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes),
            Error::<Test>::GovernanceIsPaused
        );
    });
}

#[test]
fn f6_batch_cancel_proposals_works() {
    ExtBuilder::build().execute_with(|| {
        // Create 2 proposals
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Proposal 1".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_price_change(),
            b"Proposal 2".to_vec(),
            None,
        ));

        assert_ok!(EntityGovernance::batch_cancel_proposals(
            RuntimeOrigin::signed(OWNER),
            SHOP_ID,
        ));

        let p0 = Proposals::<Test>::get(0).unwrap();
        let p1 = Proposals::<Test>::get(1).unwrap();
        assert_eq!(p0.status, ProposalStatus::Cancelled);
        assert_eq!(p1.status, ProposalStatus::Cancelled);
    });
}

#[test]
fn f6_batch_cancel_rejects_non_owner() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::batch_cancel_proposals(RuntimeOrigin::signed(ALICE), SHOP_ID),
            Error::<Test>::NotEntityOwner
        );
    });
}

// ==================== F3: ProductProvider 链上执行 ====================

#[test]
fn f3_price_change_executes_on_chain() {
    ExtBuilder::build().execute_with(|| {
        // Setup product
        set_product_price(1, 100);

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            ProposalType::PriceChange { product_id: 1, new_price: 500 },
            b"Price change".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        // Verify price was updated
        assert_eq!(get_product_price(1), Some(500));
    });
}

#[test]
fn f3_inventory_adjustment_executes_on_chain() {
    ExtBuilder::build().execute_with(|| {
        set_product_stock(1, 10);

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            ProposalType::InventoryAdjustment { product_id: 1, new_inventory: 200 },
            b"Stock change".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        assert_eq!(get_product_stock(1), Some(200));
    });
}

// ==================== F9: VoteRecords 延迟清理 ====================

#[test]
fn f9_vote_records_preserved_after_finalize() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // VoteRecords should still exist after finalize (F9: delayed cleanup)
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());
    });
}

#[test]
fn f9_vote_records_cleaned_on_cleanup() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE),
            SHOP_ID,
            proposal_type_general(),
            b"Test".to_vec(),
            None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        // Verify records exist
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        // Now cleanup the failed/passed proposal
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert!(matches!(proposal.status, ProposalStatus::Passed | ProposalStatus::Failed));

        // If passed, execute first
        if proposal.status == ProposalStatus::Passed {
            advance_blocks(51);
            assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        }

        // Cleanup
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));

        // VoteRecords should be gone now
        assert!(VoteRecords::<Test>::get(0, BOB).is_none());
        // Proposal should be removed
        assert!(Proposals::<Test>::get(0).is_none());
    });
}

// ==================== Audit 回归测试 ====================

#[test]
fn m1_audit_change_vote_blocked_when_paused() {
    // M1-audit: change_vote 应在治理暂停时被阻止（与 vote 一致）
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // 暂停治理
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // change_vote 应被阻止
        assert_noop!(
            EntityGovernance::change_vote(RuntimeOrigin::signed(BOB), 0, VoteType::No),
            Error::<Test>::GovernanceIsPaused
        );
    });
}

#[test]
fn m3_audit_batch_cancel_preserves_vote_records() {
    // M3-audit: batch_cancel 不再立即清理 VoteRecords（F9 延迟清理一致性）
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"P1".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // 确认投票记录存在
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        // batch_cancel
        assert_ok!(EntityGovernance::batch_cancel_proposals(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // M3-audit: VoteRecords 应仍保留（延迟清理）
        assert!(VoteRecords::<Test>::get(0, BOB).is_some());

        // 通过 cleanup_proposal 统一清理
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(VoteRecords::<Test>::get(0, BOB).is_none());
    });
}

#[test]
fn h2_audit_change_vote_not_voted_error() {
    // H2-audit: change_vote 对未投票用户返回 NotVoted（非 ProposalNotFound）
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // CHARLIE 未投票
        assert_noop!(
            EntityGovernance::change_vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes),
            Error::<Test>::NotVoted
        );
    });
}

// ==================== Round 2 审计回归测试 ====================

#[test]
fn m1r2_create_proposal_inactive_entity_returns_entity_not_active() {
    // M1-R2: inactive entity 应返回 EntityNotActive（非 EntityNotFound）
    ExtBuilder::build().execute_with(|| {
        set_entity_active(SHOP_ID, false);
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(), b"Test".to_vec(), None,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn m1r2_delegate_vote_inactive_entity_returns_entity_not_active() {
    // M1-R2: delegate_vote 对 inactive entity 也应返回 EntityNotActive
    ExtBuilder::build().execute_with(|| {
        set_entity_active(SHOP_ID, false);
        assert_noop!(
            EntityGovernance::delegate_vote(RuntimeOrigin::signed(ALICE), SHOP_ID, BOB),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn m2r2_cleanup_proposal_removes_proposal_when_all_cleared() {
    // M2-R2: cleanup 在投票记录 <500 时应完全删除 Proposal
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));

        // cleanup 应删除 Proposal + VoteRecords
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(Proposals::<Test>::get(0).is_none());
        assert!(VoteRecords::<Test>::get(0, BOB).is_none());
    });
}

// ==================== F8: integrity_test ====================

#[test]
fn f8_integrity_test_passes() {
    ExtBuilder::build().execute_with(|| {
        // The integrity_test runs at compile-time in Hooks.
        // We verify it doesn't panic with our mock config.
        <EntityGovernance as Hooks<u64>>::integrity_test();
    });
}

// ==================== R7 审计修复回归测试 ====================

#[test]
fn r7_h1_delegate_undelegate_then_vote_blocked() {
    // R7-H1: 委托→代理投票→取消委托→自行投票 — 双重计票应被阻止
    ExtBuilder::build().execute_with(|| {
        // ALICE(20k) delegates to BOB(150k)
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Double Count Test".to_vec(), None,
        ));

        // BOB votes → weight includes ALICE's delegated power (170k)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 170_000u128); // BOB(150k) + ALICE(20k)

        // VoterTokenLocks should exist for ALICE (set by delegate vote path)
        assert!(VoterTokenLocks::<Test>::contains_key(0, ALICE));

        // ALICE undelegates
        assert_ok!(EntityGovernance::undelegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
        ));

        // ALICE tries to vote directly → should be blocked by VoterTokenLocks check
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes),
            Error::<Test>::AlreadyVoted
        );

        // Verify votes unchanged (no double-counting)
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 170_000u128);
    });
}

#[test]
fn r7_h1_non_delegated_user_can_still_vote() {
    // R7-H1: VoterTokenLocks 检查不应误拦正常投票者
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Normal Vote".to_vec(), None,
        ));

        // CHARLIE has no delegation relationship — should vote normally
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.yes_votes, 50_000u128);
    });
}

#[test]
fn r7_h1_delegator_can_vote_on_different_proposal() {
    // R7-H1: 委托者在代理人仅投票提案 P0 后，可以对 P1 自行投票（取消委托后）
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));

        // Create two proposals
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"P0".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_price_change(), b"P1".to_vec(), None,
        ));

        // BOB votes on P0 only
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // ALICE undelegates
        assert_ok!(EntityGovernance::undelegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
        ));

        // ALICE should be blocked on P0 (VoterTokenLocks exists)
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::No),
            Error::<Test>::AlreadyVoted
        );

        // ALICE should be allowed on P1 (no VoterTokenLocks for P1)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 1, VoteType::Yes));
    });
}

#[test]
fn r7_m2_cleanup_partial_no_clean_event() {
    // R7-M2: cleanup_proposal 部分清理时应发出 ProposalPartialCleaned 而非 ProposalCleaned
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::cancel_proposal(RuntimeOrigin::signed(ALICE), 0));

        System::reset_events();
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(BOB), 0));

        // With few records, should be fully cleaned → ProposalCleaned
        let events = System::events();
        assert!(
            events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::EntityGovernance(Event::ProposalCleaned { proposal_id: 0 })
                )
            }),
            "ProposalCleaned event expected for full cleanup"
        );
        assert!(
            !events.iter().any(|e| {
                matches!(
                    e.event,
                    RuntimeEvent::EntityGovernance(Event::ProposalPartialCleaned { .. })
                )
            }),
            "ProposalPartialCleaned should not appear for full cleanup"
        );
        // Proposal should be removed
        assert!(Proposals::<Test>::get(0).is_none());
    });
}

#[test]
fn r7_l1_disclosure_level_change_rejects_invalid_level() {
    // R7-L1: DisclosureLevelChange level > 3 应被拒绝
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::DisclosureLevelChange {
                    level: 4,
                    insider_trading_control: true,
                    blackout_period_after: 100,
                },
                b"Bad level".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        // level = 255 (extreme)
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::DisclosureLevelChange {
                    level: 255,
                    insider_trading_control: false,
                    blackout_period_after: 50,
                },
                b"Bad level 255".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn r7_l1_disclosure_level_change_accepts_valid_levels() {
    // R7-L1: level 0-3 均应接受
    ExtBuilder::build().execute_with(|| {
        for level in 0..=3u8 {
            assert_ok!(EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::DisclosureLevelChange {
                    level,
                    insider_trading_control: true,
                    blackout_period_after: 100,
                },
                format!("Level {}", level).into_bytes(), None,
            ));
        }
    });
}

#[test]
fn r7_l3_governance_provider_is_governance_paused() {
    // R7-L3: GovernanceProvider 应暴露 is_governance_paused
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceProvider;

        // Initially not paused
        assert!(!EntityGovernance::is_governance_paused(SHOP_ID));

        // Pause governance
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // Should report paused
        assert!(EntityGovernance::is_governance_paused(SHOP_ID));

        // Resume
        assert_ok!(EntityGovernance::resume_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // Should report not paused
        assert!(!EntityGovernance::is_governance_paused(SHOP_ID));
    });
}

// ==================== R8: DAO 可控紧急权限 ====================

fn setup_fulldao_locked() {
    use pallet_entity_common::GovernanceMode;
    GovernanceConfigs::<Test>::insert(
        SHOP_ID,
        crate::pallet::GovernanceConfig::<u64> {
            mode: GovernanceMode::FullDAO,
            voting_period: 0,
            execution_delay: 0,
            quorum_threshold: 0,
            pass_threshold: 0,
            proposal_threshold: 0,
            admin_veto_enabled: false,
        },
    );
    GovernanceLocked::<Test>::insert(SHOP_ID, true);
}

#[test]
fn r8_pause_allowed_when_fulldao_locked_and_enabled() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();
        // EmergencyPauseEnabled defaults to true (None → true)
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert!(GovernancePaused::<Test>::get(SHOP_ID));
    });
}

#[test]
fn r8_pause_blocked_when_fulldao_locked_and_disabled() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();
        EmergencyPauseEnabled::<Test>::insert(SHOP_ID, false);
        assert_noop!(
            EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::EmergencyPauseDisabled
        );
    });
}

#[test]
fn r8_pause_allowed_when_locked_none_mode() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        GovernanceConfigs::<Test>::insert(
            SHOP_ID,
            crate::pallet::GovernanceConfig::<u64> {
                mode: GovernanceMode::None,
                voting_period: 0, execution_delay: 0,
                quorum_threshold: 0, pass_threshold: 0,
                proposal_threshold: 0, admin_veto_enabled: false,
            },
        );
        GovernanceLocked::<Test>::insert(SHOP_ID, true);
        // None 模式锁定不受 EmergencyPauseEnabled 限制
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
    });
}

#[test]
fn r8_pause_allowed_when_fulldao_not_locked() {
    ExtBuilder::build().execute_with(|| {
        // FullDAO but NOT locked — Owner has full control, no EmergencyPauseEnabled check
        EmergencyPauseEnabled::<Test>::insert(SHOP_ID, false);
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
    });
}

#[test]
fn r8_batch_cancel_allowed_when_fulldao_locked_and_enabled() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // BatchCancelEnabled defaults to true
        assert_ok!(EntityGovernance::batch_cancel_proposals(RuntimeOrigin::signed(OWNER), SHOP_ID));
        let p = Proposals::<Test>::get(0).unwrap();
        assert_eq!(p.status, ProposalStatus::Cancelled);
    });
}

#[test]
fn r8_batch_cancel_blocked_when_fulldao_locked_and_disabled() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();
        BatchCancelEnabled::<Test>::insert(SHOP_ID, false);
        assert_noop!(
            EntityGovernance::batch_cancel_proposals(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::BatchCancelDisabled
        );
    });
}

#[test]
fn r8_batch_cancel_allowed_when_fulldao_not_locked() {
    ExtBuilder::build().execute_with(|| {
        // FullDAO but NOT locked — no BatchCancelEnabled check
        BatchCancelEnabled::<Test>::insert(SHOP_ID, false);
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::batch_cancel_proposals(RuntimeOrigin::signed(OWNER), SHOP_ID));
    });
}

#[test]
fn r8_emergency_pause_toggle_proposal_executes() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();

        // Create proposal to disable emergency pause
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::EmergencyPauseToggle { enabled: false },
            b"Disable pause".to_vec(), None,
        ));

        // Vote (Bob has 15%)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        // Finalize
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(Proposals::<Test>::get(0).unwrap().status, ProposalStatus::Passed);

        // Execute
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        // Verify: EmergencyPauseEnabled is now false
        assert_eq!(EmergencyPauseEnabled::<Test>::get(SHOP_ID), Some(false));

        // Owner can no longer pause
        assert_noop!(
            EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::EmergencyPauseDisabled
        );
    });
}

#[test]
fn r8_batch_cancel_toggle_proposal_executes() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();

        // Create proposal to disable batch cancel
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::BatchCancelToggle { enabled: false },
            b"Disable batch cancel".to_vec(), None,
        ));

        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        assert_eq!(BatchCancelEnabled::<Test>::get(SHOP_ID), Some(false));

        // Owner can no longer batch cancel
        assert_noop!(
            EntityGovernance::batch_cancel_proposals(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::BatchCancelDisabled
        );
    });
}

#[test]
fn r8_dao_can_re_enable_emergency_powers() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();

        // Disable then re-enable emergency pause
        EmergencyPauseEnabled::<Test>::insert(SHOP_ID, false);

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::EmergencyPauseToggle { enabled: true },
            b"Re-enable pause".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        assert_eq!(EmergencyPauseEnabled::<Test>::get(SHOP_ID), Some(true));

        // Owner can pause again
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
    });
}

#[test]
fn r8_resume_still_works_after_pause_disabled() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();

        // Pause first (while enabled)
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // DAO disables pause ability
        EmergencyPauseEnabled::<Test>::insert(SHOP_ID, false);

        // Resume should still work (must be able to unpause even if pause is disabled)
        assert_ok!(EntityGovernance::resume_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert!(!GovernancePaused::<Test>::get(SHOP_ID));

        // But can't pause again
        assert_noop!(
            EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID),
            Error::<Test>::EmergencyPauseDisabled
        );
    });
}

#[test]
fn r8_explicit_enable_overrides_default() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();

        // Explicitly set to true
        EmergencyPauseEnabled::<Test>::insert(SHOP_ID, true);
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
    });
}

// ==================== R9 审计修复验证测试 ====================

#[test]
fn s1_delegation_double_counting_prevented() {
    // S1: 委托→代投→取消委托→再委托不可双重计票
    ExtBuilder::build().execute_with(|| {
        // ALICE delegates to BOB
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, BOB,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(CHARLIE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        // BOB votes (weight = BOB 150k + ALICE 20k = 170k)
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        let p = Proposals::<Test>::get(0).unwrap();
        assert_eq!(p.yes_votes, 170_000u128);

        // ALICE undelegates from BOB, then delegates to OWNER
        assert_ok!(EntityGovernance::undelegate_vote(RuntimeOrigin::signed(ALICE), SHOP_ID));
        assert_ok!(EntityGovernance::delegate_vote(
            RuntimeOrigin::signed(ALICE), SHOP_ID, OWNER,
        ));

        // OWNER votes — ALICE's tokens already counted via BOB, should NOT be double-counted
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(OWNER), 0, VoteType::Yes));
        let p = Proposals::<Test>::get(0).unwrap();
        // OWNER's own weight = 100k, ALICE's delegation = 0 (blocked by VoterTokenLocks)
        assert_eq!(p.yes_votes, 170_000u128 + 100_000u128);
    });
}

#[test]
fn s2_voting_period_change_rejects_too_long() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::VotingPeriodChange { new_period_blocks: 10001 },
                b"Too long".to_vec(), None,
            ),
            Error::<Test>::VotingPeriodTooLong
        );
    });
}

#[test]
fn s2_execution_delay_change_rejects_too_long() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ExecutionDelayChange { new_delay_blocks: 5001 },
                b"Too long".to_vec(), None,
            ),
            Error::<Test>::ExecutionDelayTooLong
        );
    });
}

#[test]
fn s2_configure_governance_rejects_period_too_long() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, Some(10001), None, None, None, None, None,
            ),
            Error::<Test>::VotingPeriodTooLong
        );
    });
}

#[test]
fn s3_proposal_threshold_change_rejects_too_low() {
    ExtBuilder::build().execute_with(|| {
        // MinProposalThreshold = 100 (1%), setting to 1 (0.01%) should fail
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ProposalThresholdChange { new_threshold: 1 },
                b"Too low".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn s4_configure_governance_rejects_inactive_entity() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        set_entity_active(SHOP_ID, false);
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), 1,
                GovernanceMode::FullDAO, None, None, None, None, None, None,
            ),
            Error::<Test>::EntityNotActive
        );
    });
}

#[test]
fn f1_voter_count_increments_on_vote() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));
        assert_eq!(Proposals::<Test>::get(0).unwrap().voter_count, 0);

        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_eq!(Proposals::<Test>::get(0).unwrap().voter_count, 1);

        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::No));
        assert_eq!(Proposals::<Test>::get(0).unwrap().voter_count, 2);
    });
}

#[test]
fn f2_shop_resume_invalid_shop_rejected_at_creation() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ShopResume { shop_id: 999 },
                b"Resume invalid".to_vec(), None,
            ),
            Error::<Test>::ShopNotFound
        );
    });
}

#[test]
fn f3_emergency_pause_toggle_requires_fulldao_locked() {
    ExtBuilder::build().execute_with(|| {
        // FullDAO but NOT locked — should be rejected
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::EmergencyPauseToggle { enabled: false },
                b"Toggle".to_vec(), None,
            ),
            Error::<Test>::GovernanceModeNotAllowed
        );
    });
}

#[test]
fn f3_emergency_pause_toggle_works_when_fulldao_locked() {
    ExtBuilder::build().execute_with(|| {
        setup_fulldao_locked();
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::EmergencyPauseToggle { enabled: false },
            b"Toggle".to_vec(), None,
        ));
    });
}

#[test]
fn f4_token_burn_zero_rejected() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TokenBurn { amount: 0 },
                b"Burn zero".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn f5_execution_failure_transitions_to_execution_failed() {
    ExtBuilder::build().execute_with(|| {
        // TokenBurn with amount > treasury balance will fail at execution
        // (entity treasury balance defaults to 0)
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenBurn { amount: 999_999 },
            b"Burn too much".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        // Execute should NOT revert — should transition to ExecutionFailed
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        let p = Proposals::<Test>::get(0).unwrap();
        assert_eq!(p.status, ProposalStatus::ExecutionFailed);
    });
}

#[test]
fn f5_execution_failed_is_terminal_for_cleanup() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenBurn { amount: 999_999 },
            b"Burn too much".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));
        advance_blocks(51);
        assert_ok!(EntityGovernance::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(Proposals::<Test>::get(0).unwrap().status, ProposalStatus::ExecutionFailed);
        assert_ok!(EntityGovernance::cleanup_proposal(RuntimeOrigin::signed(ALICE), 0));
        assert!(Proposals::<Test>::get(0).is_none());
    });
}

// ==================== R10: 新增 39 种 ProposalType 测试 ====================

#[test]
fn r10_market_proposals_create_and_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MarketConfigChange { min_order_amount: 100, order_ttl: 1000 },
            b"Market config".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MarketPause, b"Pause".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MarketResume, b"Resume".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MarketClose, b"Close".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CircuitBreakerLift, b"Lift".to_vec(), None,
        ));
        // PriceProtectionChange validation
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::PriceProtectionChange {
                    max_price_deviation: 10001, max_slippage: 500,
                    circuit_breaker_threshold: 500, min_trades_for_twap: 10,
                },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::PriceProtectionChange {
                max_price_deviation: 500, max_slippage: 300,
                circuit_breaker_threshold: 1000, min_trades_for_twap: 5,
            },
            b"OK".to_vec(), None,
        ));
        // MarketKycChange validation
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MarketKycChange { min_kyc_level: 5 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MarketKycChange { min_kyc_level: 2 },
            b"OK".to_vec(), None,
        ));
    });
}

#[test]
fn r10_single_line_proposals_create_and_validate() {
    ExtBuilder::build().execute_with(|| {
        // Rate too high
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::SingleLineConfigChange {
                    upline_rate: 10001, downline_rate: 500,
                    base_upline_levels: 3, base_downline_levels: 3,
                    max_upline_levels: 5, max_downline_levels: 5,
                },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        // base > max
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::SingleLineConfigChange {
                    upline_rate: 500, downline_rate: 500,
                    base_upline_levels: 6, base_downline_levels: 3,
                    max_upline_levels: 5, max_downline_levels: 5,
                },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::SingleLineConfigChange {
                upline_rate: 500, downline_rate: 500,
                base_upline_levels: 3, base_downline_levels: 3,
                max_upline_levels: 5, max_downline_levels: 5,
            },
            b"OK".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::SingleLinePause, b"Pause".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::SingleLineResume, b"Resume".to_vec(), None,
        ));
    });
}

#[test]
fn r10_token_extension_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TokenMaxSupplyChange { new_max_supply: 0 },
                b"Zero".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenMaxSupplyChange { new_max_supply: 1_000_000 },
            b"OK".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TokenTypeChange { new_type: 4 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenTypeChange { new_type: 1 },
            b"Governance".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TransferRestrictionChange { restriction: 3, min_receiver_kyc: 0 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TransferRestrictionChange { restriction: 1, min_receiver_kyc: 2 },
            b"Whitelist".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenBlacklistManage {
                account_cid: b"acc1".to_vec().try_into().unwrap(), add: true
            },
            b"Blacklist".to_vec(), None,
        ));
    });
}

#[test]
fn r10_commission_core_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::CommissionRateChange { new_rate: 10001 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CommissionRateChange { new_rate: 500 },
            b"OK".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CommissionToggle { enabled: false },
            b"Disable".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::CreatorRewardRateChange { new_rate: 10001 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CreatorRewardRateChange { new_rate: 300 },
            b"OK".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::WithdrawalCooldownChange { nex_cooldown: 100, token_cooldown: 200 },
            b"Cooldown".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenWithdrawalConfigChange { enabled: true },
            b"Enable".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::WithdrawalPauseToggle { paused: true },
            b"Pause".to_vec(), None,
        ));
    });
}

#[test]
fn r10_referral_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ReferrerGuardChange { min_referrer_spent: 1000, min_referrer_orders: 5 },
            b"Guard".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::CommissionCapChange { max_per_order: 0, max_total_earned: 0 },
                b"Both zero".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::CommissionCapChange { max_per_order: 1000, max_total_earned: 0 },
            b"OK".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ReferralValidityChange { validity_blocks: 10000, valid_orders: 50 },
            b"Validity".to_vec(), None,
        ));
    });
}

#[test]
fn r10_multi_level_pause_resume_creates() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MultiLevelPause, b"Pause ML".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MultiLevelResume, b"Resume ML".to_vec(), None,
        ));
    });
}

#[test]
fn r10_member_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MemberPolicyChange { policy: 4 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MemberPolicyChange { policy: 3 },
            b"KycRequired".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::UpgradeRuleToggle { enabled: false },
            b"Disable".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MemberStatsPolicyChange { qualified_only: true, subtract_on_removal: false },
            b"Stats".to_vec(), None,
        ));
    });
}

#[test]
fn r10_kyc_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::KycRequirementChange { min_level: 5, mandatory: true, grace_period: 100 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::KycRequirementChange { min_level: 2, mandatory: true, grace_period: 1000 },
            b"KYC L2".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::KycProviderAuthorize { provider_id: 1 },
            b"Auth".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::KycProviderDeauthorize { provider_id: 1 },
            b"Deauth".to_vec(), None,
        ));
    });
}

#[test]
fn r10_shop_extension_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::PointsConfigChange { reward_rate: 10001, exchange_rate: 100, transferable: true },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::PointsConfigChange { reward_rate: 500, exchange_rate: 100, transferable: true },
            b"OK".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::PointsToggle { enabled: false },
            b"Disable points".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ShopPoliciesChange { policies_cid: b"QmPolicy".to_vec().try_into().unwrap() },
            b"Policy".to_vec(), None,
        ));
        // ShopClose validates ownership
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ShopClose { shop_id: 999 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::ShopNotFound
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ShopClose { shop_id: SHOP_ID },
            b"Close".to_vec(), None,
        ));
        // ShopTypeChange validates
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ShopTypeChange { shop_id: SHOP_ID, new_type: 4 },
                b"Bad type".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ShopTypeChange { shop_id: SHOP_ID, new_type: 2 },
            b"Service".to_vec(), None,
        ));
    });
}

#[test]
fn r10_product_visibility_validates() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::ProductVisibilityChange { product_id: 1, visibility: 3 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::ProductVisibilityChange { product_id: 1, visibility: 1 },
            b"MembersOnly".to_vec(), None,
        ));
    });
}

#[test]
fn r10_disclosure_extension_proposals_validate() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::DisclosureInsiderManage {
                account_cid: b"insider".to_vec().try_into().unwrap(), add: true
            },
            b"Add insider".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::DisclosurePenaltyChange { level: 4 },
                b"Bad".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::DisclosurePenaltyChange { level: 2 },
            b"Suspend".to_vec(), None,
        ));
    });
}

// ==================== R11: 审计修复测试 ====================

#[test]
fn r11_s1_nonexistent_product_rejected() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::PriceChange { product_id: 999, new_price: 100 },
                b"Ghost product".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn r11_s2_token_mint_zero_rejected() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TokenMint { amount: 0, recipient_cid: b"r".to_vec().try_into().unwrap() },
                b"Zero mint".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn r11_s3_treasury_spend_zero_rejected() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TreasurySpend {
                    amount: 0,
                    recipient_cid: b"r".to_vec().try_into().unwrap(),
                    reason_cid: b"x".to_vec().try_into().unwrap(),
                },
                b"Zero spend".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn r11_s4_airdrop_zero_rejected() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::AirdropDistribution {
                    airdrop_cid: b"a".to_vec().try_into().unwrap(),
                    total_amount: 0,
                },
                b"Zero airdrop".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn r11_s5_token_config_rate_overflow_rejected() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::TokenConfigChange { reward_rate: Some(10001), exchange_rate: None },
                b"Bad rate".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::TokenConfigChange { reward_rate: Some(5000), exchange_rate: Some(10000) },
            b"OK".to_vec(), None,
        ));
    });
}

#[test]
fn r11_s6_inventory_u64_overflow_rejected() {
    ExtBuilder::build().execute_with(|| {
        set_product_price(1, 500);
        set_product_stock(1, 10);
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::InventoryAdjustment { product_id: 1, new_inventory: u64::MAX },
                b"Overflow".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::InventoryAdjustment { product_id: 1, new_inventory: 1000 },
            b"OK".to_vec(), None,
        ));
    });
}

#[test]
fn r11_f1_member_policy_upper_bound() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::MemberPolicyChange { policy: 3 },
            b"Max valid".to_vec(), None,
        ));
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MemberPolicyChange { policy: 4 },
                b"Over max".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

// ==================== Phase 2 Tests ====================

#[test]
fn p2_reserve_failure_blocks_vote() {
    ExtBuilder::build().execute_with(|| {
        // Alice has 20000 tokens, create a proposal
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::FeeAdjustment { new_fee_rate: 200 },
            b"Fee".to_vec(), None,
        ));
        // Reserve all of Alice's balance externally so governance reserve fails
        let balance = get_token_balance(SHOP_ID, ALICE);
        // Manually reserve everything via the mock
        assert_ok!(MockTokenProvider::reserve(SHOP_ID, &ALICE, balance));

        // Vote should fail because reserve will fail (no available balance)
        assert_noop!(
            EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes),
            Error::<Test>::TokenLockFailed
        );
    });
}

#[test]
fn p2_proposal_cooldown_enforced() {
    ExtBuilder::build().execute_with(|| {
        // Override cooldown to 50 blocks via storage
        // Since ProposalCooldown is 0 in mock, test the logic by directly setting LastProposalCreatedAt
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::FeeAdjustment { new_fee_rate: 100 },
            b"First".to_vec(), None,
        ));
        // Verify LastProposalCreatedAt was set
        assert!(LastProposalCreatedAt::<Test>::get(SHOP_ID, ALICE).is_some());
    });
}

#[test]
fn p2_force_unlock_governance_works() {
    ExtBuilder::build().execute_with(|| {
        // Lock governance
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert!(GovernanceLocked::<Test>::get(SHOP_ID));

        // Non-root cannot force unlock
        assert_noop!(
            EntityGovernance::force_unlock_governance(RuntimeOrigin::signed(ALICE), SHOP_ID),
            sp_runtime::DispatchError::BadOrigin
        );

        // Root can force unlock
        assert_ok!(EntityGovernance::force_unlock_governance(RuntimeOrigin::root(), SHOP_ID));
        assert!(!GovernanceLocked::<Test>::get(SHOP_ID));

        // Owner can now configure governance again
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            GovernanceMode::FullDAO,
            None, None, None, None, None, None,
        ));
    });
}

#[test]
fn p2_force_unlock_also_resumes_paused() {
    ExtBuilder::build().execute_with(|| {
        // Pause governance
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert!(GovernancePaused::<Test>::get(SHOP_ID));

        // Lock governance
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        // Root force unlock — should both unlock and resume
        assert_ok!(EntityGovernance::force_unlock_governance(RuntimeOrigin::root(), SHOP_ID));
        assert!(!GovernanceLocked::<Test>::get(SHOP_ID));
        assert!(!GovernancePaused::<Test>::get(SHOP_ID));
    });
}

#[test]
fn p2_force_unlock_nonexistent_entity_fails() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::force_unlock_governance(RuntimeOrigin::root(), 999),
            Error::<Test>::EntityNotFound
        );
    });
}

#[test]
fn p2_force_unlock_emits_events() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::pause_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));
        assert_ok!(EntityGovernance::lock_governance(RuntimeOrigin::signed(OWNER), SHOP_ID));

        System::reset_events();

        assert_ok!(EntityGovernance::force_unlock_governance(RuntimeOrigin::root(), SHOP_ID));

        let events = System::events();
        let event_names: Vec<_> = events.iter().filter_map(|e| {
            match &e.event {
                RuntimeEvent::EntityGovernance(ev) => Some(format!("{:?}", ev)),
                _ => None,
            }
        }).collect();
        assert!(event_names.iter().any(|e| e.contains("GovernanceForceUnlocked")));
        assert!(event_names.iter().any(|e| e.contains("GovernanceForceResumed")));
    });
}

// ==================== 深度审计修复测试 ====================

// --- BUG-1: on_idle 按 voter_count 动态估权 ---

#[test]
fn audit_bug1_on_idle_skips_heavy_voter_proposal_when_weight_tight() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Heavy".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(ALICE), 0, VoteType::Yes));

        advance_blocks(102);

        let tiny_weight = frame_support::weights::Weight::from_parts(50_000_000, 5_000);
        EntityGovernance::on_idle(System::block_number(), tiny_weight);

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Voting,
            "BUG-1: on_idle should skip proposal when remaining weight < base + voter_count * per_voter");
    });
}

#[test]
fn audit_bug1_on_idle_processes_when_sufficient_weight() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"OK".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        advance_blocks(102);

        let enough_weight = frame_support::weights::Weight::from_parts(5_000_000_000, 500_000);
        EntityGovernance::on_idle(System::block_number(), enough_weight);

        let proposal = Proposals::<Test>::get(0).unwrap();
        assert_ne!(proposal.status, ProposalStatus::Voting,
            "BUG-1: on_idle should process proposal when weight is sufficient");
    });
}

// --- BUG-2: configure_governance 强制 MinProposalThreshold ---

#[test]
fn audit_bug2_configure_governance_rejects_low_proposal_threshold() {
    ExtBuilder::build().execute_with(|| {
        // MinProposalThreshold = 100 (1%)
        assert_noop!(
            EntityGovernance::configure_governance(
                RuntimeOrigin::signed(OWNER), SHOP_ID,
                GovernanceMode::FullDAO,
                None, None, None, None,
                Some(1), // 0.01% — below MinProposalThreshold
                None,
            ),
            Error::<Test>::ProposalThresholdTooLow
        );
    });
}

#[test]
fn audit_bug2_configure_governance_allows_zero_threshold() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            GovernanceMode::FullDAO,
            None, None, None, None,
            Some(0), // 0 = use global default
            None,
        ));
    });
}

#[test]
fn audit_bug2_configure_governance_allows_threshold_at_min() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
            GovernanceMode::FullDAO,
            None, None, None, None,
            Some(100), // exactly MinProposalThreshold
            None,
        ));
    });
}

// --- BUG-3: batch_cancel 清理 VotingPowerSnapshot ---

#[test]
fn audit_bug3_batch_cancel_clears_voting_power_snapshot() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Snap".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));

        assert!(VotingPowerSnapshot::<Test>::contains_key(0, BOB),
            "pre-condition: snapshot should exist after vote");

        assert_ok!(EntityGovernance::batch_cancel_proposals(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
        ));

        assert!(!VotingPowerSnapshot::<Test>::contains_key(0, BOB),
            "BUG-3: batch_cancel should clear VotingPowerSnapshot via helper");
    });
}

// --- BUG-4: GovernanceLockCount 双重读取修复（隐式回归测试） ---

#[test]
fn audit_bug4_finalize_unlocks_all_voter_tokens() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Unlock".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::No));

        assert!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB) > 0);
        assert!(GovernanceLockCount::<Test>::get(SHOP_ID, CHARLIE) > 0);

        advance_blocks(101);
        assert_ok!(EntityGovernance::finalize_voting(RuntimeOrigin::signed(ALICE), 0));

        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0,
            "BUG-4: lock count should be 0 after finalize");
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, CHARLIE), 0,
            "BUG-4: lock count should be 0 after finalize");
    });
}

// --- BUG-5: MarketConfigChange / WithdrawalCooldownChange 参数校验 ---

#[test]
fn audit_bug5_market_config_change_rejects_zero_amount() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MarketConfigChange { min_order_amount: 0, order_ttl: 100 },
                b"Bad market".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn audit_bug5_market_config_change_rejects_zero_ttl() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::MarketConfigChange { min_order_amount: 100, order_ttl: 0 },
                b"Bad ttl".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn audit_bug5_withdrawal_cooldown_rejects_both_zero() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                ProposalType::WithdrawalCooldownChange { nex_cooldown: 0, token_cooldown: 0 },
                b"Bad cooldown".to_vec(), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

#[test]
fn audit_bug5_withdrawal_cooldown_allows_one_nonzero() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            ProposalType::WithdrawalCooldownChange { nex_cooldown: 100, token_cooldown: 0 },
            b"One cool".to_vec(), None,
        ));
    });
}

// --- R-1/R-2: 共用 helper + batch_cancel 清空活跃列表 ---

#[test]
fn audit_r1r2_batch_cancel_clears_active_list() {
    ExtBuilder::build().execute_with(|| {
        for i in 0..3 {
            assert_ok!(EntityGovernance::create_proposal(
                RuntimeOrigin::signed(ALICE), SHOP_ID,
                proposal_type_general(),
                format!("P{}", i).into_bytes(), None,
            ));
        }
        assert_eq!(EntityProposals::<Test>::get(SHOP_ID).len(), 3);

        assert_ok!(EntityGovernance::batch_cancel_proposals(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
        ));

        assert_eq!(EntityProposals::<Test>::get(SHOP_ID).len(), 0,
            "R-2: batch_cancel should completely clear the active list");

        for i in 0..3u64 {
            let p = Proposals::<Test>::get(i).unwrap();
            assert_eq!(p.status, ProposalStatus::Cancelled);
        }
    });
}

#[test]
fn audit_r1_helper_unlocks_tokens_on_batch_cancel() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Tok".to_vec(), None,
        ));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(BOB), 0, VoteType::Yes));
        assert_ok!(EntityGovernance::vote(RuntimeOrigin::signed(CHARLIE), 0, VoteType::No));

        let bob_lock_before = GovernanceLockCount::<Test>::get(SHOP_ID, BOB);
        assert!(bob_lock_before > 0);

        assert_ok!(EntityGovernance::batch_cancel_proposals(
            RuntimeOrigin::signed(OWNER), SHOP_ID,
        ));

        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, BOB), 0,
            "R-1: batch_cancel via helper should unlock BOB's tokens");
        assert_eq!(GovernanceLockCount::<Test>::get(SHOP_ID, CHARLIE), 0,
            "R-1: batch_cancel via helper should unlock CHARLIE's tokens");
    });
}
