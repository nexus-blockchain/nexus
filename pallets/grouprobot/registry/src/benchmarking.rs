//! Benchmarking setup for pallet-grouprobot-registry

#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as GroupRobotRegistry;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register_bot() {
        let caller: T::AccountId = whitelisted_caller();
        let bot_id_hash: BotIdHash = [1u8; 32];
        let public_key: [u8; 32] = [2u8; 32];

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), bot_id_hash, public_key);

        assert!(Bots::<T>::contains_key(&bot_id_hash));
    }

    impl_benchmark_test_suite!(
        GroupRobotRegistry,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
