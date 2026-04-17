//! Benchmarks for `pallet-grouprobot-consensus`

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet_grouprobot_primitives::BotIdHash;

/// Seeds 20+ avoid collision with MockBotRegistry hardcoded values (1,2,10,11)
fn setup_node<T: Config>(seed: u32) -> (T::AccountId, NodeId) {
    let operator: T::AccountId = frame_benchmarking::account("operator", seed, 0);
    let node_id = T::BenchmarkHelper::node_id(seed);
    let stake = T::MinStake::get() * 10u32.into();
    T::BenchmarkHelper::fund_account(&operator, stake * 10u32.into());
    Pallet::<T>::register_node(RawOrigin::Signed(operator.clone()).into(), node_id, stake)
        .expect("register_node should succeed in benchmark setup");
    (operator, node_id)
}

fn setup_tee_node<T: Config>(seed: u32) -> (T::AccountId, NodeId, BotIdHash) {
    let (operator, node_id) = setup_node::<T>(seed);
    let bot_id_hash = T::BenchmarkHelper::bot_id_hash(seed);
    T::BenchmarkHelper::setup_tee_bot(&bot_id_hash, &operator);
    Pallet::<T>::verify_node_tee(
        RawOrigin::Signed(operator.clone()).into(),
        node_id,
        bot_id_hash,
    )
    .expect("verify_node_tee should succeed in benchmark setup");
    (operator, node_id, bot_id_hash)
}

fn setup_suspended_node<T: Config>(seed: u32) -> (T::AccountId, NodeId) {
    let (operator, node_id) = setup_node::<T>(seed);
    Pallet::<T>::force_suspend_node(RawOrigin::Root.into(), node_id)
        .expect("force_suspend should succeed");
    (operator, node_id)
}

fn setup_equivocation<T: Config>(node_seed: u32, seq: u64) -> (T::AccountId, NodeId) {
    let (_, node_id) = setup_node::<T>(node_seed);
    let reporter: T::AccountId = frame_benchmarking::account("reporter", node_seed, 1);
    T::BenchmarkHelper::fund_account(&reporter, T::MinStake::get() * 10u32.into());
    let msg_a = [b"bench-equivocation-a", seq.to_le_bytes().as_slice()].concat();
    let msg_b = [b"bench-equivocation-b", seq.to_le_bytes().as_slice()].concat();
    let (hash_a, sig_a) = T::BenchmarkHelper::sign_message(node_seed, &msg_a);
    let (hash_b, sig_b) = T::BenchmarkHelper::sign_message(node_seed, &msg_b);
    Pallet::<T>::report_equivocation(
        RawOrigin::Signed(reporter.clone()).into(),
        node_id,
        seq,
        hash_a,
        sig_a,
        hash_b,
        sig_b,
    )
    .expect("report_equivocation should succeed in benchmark setup");
    (reporter, node_id)
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn register_node() {
        let caller: T::AccountId = frame_benchmarking::whitelisted_caller();
        let node_id = T::BenchmarkHelper::node_id(100);
        let stake = T::MinStake::get() * 2u32.into();
        T::BenchmarkHelper::fund_account(&caller, stake * 10u32.into());
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), node_id, stake);
        assert!(Nodes::<T>::contains_key(node_id));
    }

    #[benchmark]
    fn request_exit() {
        let (operator, node_id) = setup_node::<T>(20);
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id);
        assert_eq!(
            Nodes::<T>::get(node_id).unwrap().status,
            NodeStatus::Exiting
        );
    }

    #[benchmark]
    fn finalize_exit() {
        let (operator, node_id) = setup_node::<T>(20);
        Pallet::<T>::request_exit(RawOrigin::Signed(operator.clone()).into(), node_id)
            .expect("request_exit should succeed");
        let target =
            frame_system::Pallet::<T>::block_number() + T::ExitCooldownPeriod::get() + 1u32.into();
        frame_system::Pallet::<T>::set_block_number(target);
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id);
        assert!(!Nodes::<T>::contains_key(node_id));
    }

    #[benchmark]
    fn increase_stake() {
        let (operator, node_id) = setup_node::<T>(20);
        let amount = T::MinStake::get();
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id, amount);
        assert!(Nodes::<T>::get(node_id).unwrap().stake > T::MinStake::get());
    }

    #[benchmark]
    fn reinstate_node() {
        let (operator, node_id) = setup_suspended_node::<T>(20);
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id);
        assert_eq!(Nodes::<T>::get(node_id).unwrap().status, NodeStatus::Active);
    }

    #[benchmark]
    fn force_suspend_node() {
        let (_, node_id) = setup_node::<T>(20);
        #[extrinsic_call]
        _(RawOrigin::Root, node_id);
        assert_eq!(
            Nodes::<T>::get(node_id).unwrap().status,
            NodeStatus::Suspended
        );
    }

    #[benchmark]
    fn force_remove_node() {
        let (_, node_id) = setup_node::<T>(20);
        #[extrinsic_call]
        _(RawOrigin::Root, node_id);
        assert!(!Nodes::<T>::contains_key(node_id));
    }

    #[benchmark]
    fn force_reinstate_node() {
        let (_, node_id) = setup_suspended_node::<T>(20);
        #[extrinsic_call]
        _(RawOrigin::Root, node_id);
        assert_eq!(Nodes::<T>::get(node_id).unwrap().status, NodeStatus::Active);
    }

    #[benchmark]
    fn verify_node_tee() {
        let (operator, node_id) = setup_node::<T>(50);
        let bot_id_hash = T::BenchmarkHelper::bot_id_hash(50);
        T::BenchmarkHelper::setup_tee_bot(&bot_id_hash, &operator);
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id, bot_id_hash);
        assert!(Nodes::<T>::get(node_id).unwrap().is_tee_node);
    }

    #[benchmark]
    fn unbind_bot() {
        let (operator, node_id, _) = setup_tee_node::<T>(51);
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id);
        assert!(!Nodes::<T>::get(node_id).unwrap().is_tee_node);
    }

    #[benchmark]
    fn report_equivocation() {
        let (_, node_id) = setup_node::<T>(30);
        let reporter: T::AccountId = frame_benchmarking::account("reporter", 30, 1);
        T::BenchmarkHelper::fund_account(&reporter, T::MinStake::get() * 10u32.into());
        let (ha, sa) = T::BenchmarkHelper::sign_message(30, b"bench-eq-a-42");
        let (hb, sb) = T::BenchmarkHelper::sign_message(30, b"bench-eq-b-42");
        #[extrinsic_call]
        _(RawOrigin::Signed(reporter), node_id, 42u64, ha, sa, hb, sb);
        assert!(EquivocationRecords::<T>::contains_key(node_id, 42u64));
    }

    #[benchmark]
    fn slash_equivocation() {
        let (_, node_id) = setup_equivocation::<T>(31, 42);
        #[extrinsic_call]
        _(RawOrigin::Root, node_id, 42u64);
        assert!(
            EquivocationRecords::<T>::get(node_id, 42u64)
                .unwrap()
                .resolved
        );
    }

    #[benchmark]
    fn cleanup_resolved_equivocation() {
        let (_, node_id) = setup_equivocation::<T>(32, 42);
        Pallet::<T>::slash_equivocation(RawOrigin::Root.into(), node_id, 42)
            .expect("slash should succeed");
        let caller: T::AccountId = frame_benchmarking::whitelisted_caller();
        T::BenchmarkHelper::fund_account(&caller, T::MinStake::get());
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), node_id, 42u64);
        assert!(!EquivocationRecords::<T>::contains_key(node_id, 42u64));
    }

    #[benchmark]
    fn batch_cleanup_equivocations() {
        let (_, node_id) = setup_equivocation::<T>(33, 1);
        Pallet::<T>::slash_equivocation(RawOrigin::Root.into(), node_id, 1)
            .expect("slash should succeed");
        let items = alloc::vec![(node_id, 1u64)];
        let bounded: BoundedVec<_, T::MaxActiveNodes> = items.try_into().expect("should fit");
        let caller: T::AccountId = frame_benchmarking::whitelisted_caller();
        T::BenchmarkHelper::fund_account(&caller, T::MinStake::get());
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bounded);
        assert!(!EquivocationRecords::<T>::contains_key(node_id, 1u64));
    }

    #[benchmark]
    fn mark_sequence_processed() {
        let caller: T::AccountId = frame_benchmarking::account("operator", 60, 0);
        T::BenchmarkHelper::fund_account(&caller, T::MinStake::get() * 10u32.into());
        let bot_id_hash = T::BenchmarkHelper::bot_id_hash(60);
        T::BenchmarkHelper::setup_paid_subscription(&bot_id_hash);
        T::BenchmarkHelper::setup_active_bot(&bot_id_hash, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bot_id_hash, 42u64);
        assert!(ProcessedSequences::<T>::contains_key(bot_id_hash, 42u64));
    }

    #[benchmark]
    fn replace_operator() {
        let (operator, node_id) = setup_node::<T>(40);
        let new_operator: T::AccountId = frame_benchmarking::account("new_op", 40, 2);
        T::BenchmarkHelper::fund_account(&new_operator, T::MinStake::get() * 20u32.into());
        #[extrinsic_call]
        _(RawOrigin::Signed(operator), node_id, new_operator.clone());
        assert_eq!(Nodes::<T>::get(node_id).unwrap().operator, new_operator);
    }

    #[benchmark]
    fn set_tee_reward_params() {
        #[extrinsic_call]
        _(RawOrigin::Root, 15_000u32, 2_000u32);
        assert_eq!(TeeRewardMultiplier::<T>::get(), 15_000);
    }

    #[benchmark]
    fn set_slash_percentage() {
        #[extrinsic_call]
        _(RawOrigin::Root, 25u32);
        assert_eq!(SlashPercentageOverride::<T>::get(), Some(25));
    }

    #[benchmark]
    fn set_reporter_reward_pct() {
        #[extrinsic_call]
        _(RawOrigin::Root, 1_000u32);
        assert_eq!(ReporterRewardPct::<T>::get(), 1_000);
    }

    #[benchmark]
    fn force_era_end() {
        let now = frame_system::Pallet::<T>::block_number();
        EraStartBlock::<T>::put(now);
        let era_before = CurrentEra::<T>::get();
        #[extrinsic_call]
        _(RawOrigin::Root);
        assert_eq!(CurrentEra::<T>::get(), era_before + 1);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);
}
