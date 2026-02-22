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
        assert_eq!(proposal.shop_id, SHOP_ID);
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
        let active = ShopProposals::<Test>::get(SHOP_ID);
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
            Some(20),  // quorum
            Some(500), // proposal threshold
            Some(false), // no veto
        ));
        let config = GovernanceConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.mode, GovernanceMode::FullDAO);
        assert_eq!(config.voting_period, 200);
        assert_eq!(config.quorum_threshold, 20);
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
                GovernanceMode::FullDAO, None, None, None, None,
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
                GovernanceMode::FullDAO, None, Some(101), None, None,
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
                GovernanceMode::FullDAO, None, None, Some(10001), None,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

// ==================== 分层治理阈值 ====================

#[test]
fn set_tiered_thresholds_works() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::set_tiered_thresholds(
            RuntimeOrigin::signed(OWNER), 1, 40, 55, 67, 80,
        ));
        let config = GovernanceConfigs::<Test>::get(1).unwrap();
        assert_eq!(config.operational_threshold, 40);
        assert_eq!(config.significant_threshold, 55);
        assert_eq!(config.critical_threshold, 67);
        assert_eq!(config.constitutional_threshold, 80);
    });
}

#[test]
fn set_tiered_thresholds_fails_invalid() {
    ExtBuilder::build().execute_with(|| {
        // H4: threshold > 100 should fail
        assert_noop!(
            EntityGovernance::set_tiered_thresholds(
                RuntimeOrigin::signed(OWNER), 1, 40, 55, 101, 80,
            ),
            Error::<Test>::InvalidParameter
        );
    });
}

// ==================== 委员会 ====================

#[test]
fn add_and_remove_committee_member() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::add_committee_member(
            RuntimeOrigin::signed(OWNER), 1, ALICE,
        ));
        let members = CommitteeMembers::<Test>::get(1);
        assert_eq!(members.len(), 1);
        assert!(members.contains(&ALICE));

        assert_ok!(EntityGovernance::remove_committee_member(
            RuntimeOrigin::signed(OWNER), 1, ALICE,
        ));
        let members = CommitteeMembers::<Test>::get(1);
        assert!(members.is_empty());
    });
}

#[test]
fn add_committee_member_fails_duplicate() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(EntityGovernance::add_committee_member(
            RuntimeOrigin::signed(OWNER), 1, ALICE,
        ));
        assert_noop!(
            EntityGovernance::add_committee_member(RuntimeOrigin::signed(OWNER), 1, ALICE),
            Error::<Test>::CommitteeMemberExists
        );
    });
}

// ==================== 否决 ====================

#[test]
fn veto_proposal_works() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // Configure DualTrack with veto
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::DualTrack, None, None, None, Some(true),
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
fn veto_fails_wrong_mode() {
    ExtBuilder::build().execute_with(|| {
        use pallet_entity_common::GovernanceMode;
        // Configure FullDAO (no veto)
        assert_ok!(EntityGovernance::configure_governance(
            RuntimeOrigin::signed(OWNER), 1,
            GovernanceMode::FullDAO, None, None, None, Some(true),
        ));

        assert_ok!(EntityGovernance::create_proposal(
            RuntimeOrigin::signed(ALICE), SHOP_ID,
            proposal_type_general(), b"Test".to_vec(), None,
        ));

        assert_noop!(
            EntityGovernance::veto_proposal(RuntimeOrigin::signed(OWNER), 0),
            Error::<Test>::GovernanceModeNotAllowed
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
            GovernanceMode::None, None, None, None, None,
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
