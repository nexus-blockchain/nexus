//! Benchmarking for pallet-grouprobot-ceremony.
//!
//! Uses `frame_benchmarking::v2` macro style.
//! All 11 extrinsics are benchmarked.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet::*;

fn mrenclave_bench(n: u8) -> [u8; 32] {
    let mut m = [0u8; 32];
    m[0] = n;
    m[31] = 0xEE;
    m
}

fn ceremony_hash_bench(n: u8) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[0] = n;
    h[31] = 0xAA;
    h
}

fn bot_pk_bench(n: u8) -> [u8; 32] {
    let mut p = [0u8; 32];
    p[0] = n;
    p[31] = 0xBB;
    p
}

fn bot_id_bench(n: u8) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

fn setup_enclave<T: Config>() {
    let mr = mrenclave_bench(1);
    let desc: BoundedVec<u8, ConstU32<128>> = alloc::vec![0x41; 16].try_into().unwrap();
    let info = CeremonyEnclaveInfo {
        version: 1,
        approved_at: 0,
        description: desc,
    };
    ApprovedEnclaves::<T>::insert(mr, info);
}

fn setup_ceremony<T: Config>(caller: &T::AccountId) -> [u8; 32] {
    setup_enclave::<T>();
    let ch = ceremony_hash_bench(1);
    let bot_pk = bot_pk_bench(1);
    let bot_id = bot_id_bench(1);
    let participants: BoundedVec<[u8; 32], T::MaxParticipants> =
        alloc::vec![[10u8; 32], [11u8; 32]].try_into().unwrap();
    let now = frame_system::Pallet::<T>::block_number();
    let expires_at = now.saturating_add(T::CeremonyValidityBlocks::get());

    let record = CeremonyRecord::<T> {
        ceremony_mrenclave: mrenclave_bench(1),
        k: 2,
        n: 3,
        bot_public_key: bot_pk,
        participant_count: 2,
        participant_enclaves: participants,
        initiator: caller.clone(),
        created_at: now,
        status: CeremonyStatus::Active,
        expires_at,
        is_re_ceremony: false,
        supersedes: None,
        bot_id_hash: bot_id,
    };

    Ceremonies::<T>::insert(ch, record);
    ActiveCeremony::<T>::insert(bot_pk, ch);
    CeremonyHistory::<T>::mutate(bot_pk, |history| {
        let _ = history.try_push(ch);
    });
    CeremonyCount::<T>::mutate(|c| *c = c.saturating_add(1));

    ExpiryQueue::<T>::mutate(|queue| {
        let _ = queue.try_push((expires_at, bot_pk, ch));
    });

    ch
}

#[benchmarks]
mod benches {
    use super::*;

    #[benchmark]
    fn record_ceremony(p: Linear<2, 5>) {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        setup_enclave::<T>();

        let mut participants = alloc::vec::Vec::new();
        for i in 0..p {
            let mut enc = [0u8; 32];
            enc[0] = (i + 10) as u8;
            participants.push(enc);
        }

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            ceremony_hash_bench(50),
            mrenclave_bench(1),
            2,
            5,
            bot_pk_bench(1),
            participants,
            bot_id_bench(1),
        );

        assert!(Ceremonies::<T>::contains_key(ceremony_hash_bench(50)));
    }

    #[benchmark]
    fn revoke_ceremony() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let ch = setup_ceremony::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Root, ch);

        let record = Ceremonies::<T>::get(ch).unwrap();
        assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));
    }

    #[benchmark]
    fn approve_ceremony_enclave() {
        let mr = mrenclave_bench(99);

        #[extrinsic_call]
        _(RawOrigin::Root, mr, 1u32, alloc::vec![0x41; 64]);

        assert!(ApprovedEnclaves::<T>::contains_key(mr));
    }

    #[benchmark]
    fn remove_ceremony_enclave() {
        let mr = mrenclave_bench(99);
        let desc: BoundedVec<u8, ConstU32<128>> = alloc::vec![0x41; 16].try_into().unwrap();
        ApprovedEnclaves::<T>::insert(
            mr,
            CeremonyEnclaveInfo {
                version: 1,
                approved_at: 0,
                description: desc,
            },
        );

        #[extrinsic_call]
        _(RawOrigin::Root, mr);

        assert!(!ApprovedEnclaves::<T>::contains_key(mr));
    }

    #[benchmark]
    fn force_re_ceremony() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let ch = setup_ceremony::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Root, ch);

        let record = Ceremonies::<T>::get(ch).unwrap();
        assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));
    }

    #[benchmark]
    fn cleanup_ceremony() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let ch = setup_ceremony::<T>(&caller);
        Ceremonies::<T>::mutate(ch, |maybe| {
            if let Some(r) = maybe {
                r.status = CeremonyStatus::Expired;
            }
        });
        ActiveCeremony::<T>::remove(bot_pk_bench(1));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ch);

        assert!(!Ceremonies::<T>::contains_key(ch));
    }

    #[benchmark]
    fn owner_revoke_ceremony() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let ch = setup_ceremony::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ch);

        let record = Ceremonies::<T>::get(ch).unwrap();
        assert!(matches!(record.status, CeremonyStatus::Revoked { .. }));
    }

    #[benchmark]
    fn revoke_by_mrenclave() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let _ch = setup_ceremony::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Root, mrenclave_bench(1));
    }

    #[benchmark]
    fn trigger_expiry() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let ch = setup_ceremony::<T>(&caller);
        let expires_at = Ceremonies::<T>::get(ch).unwrap().expires_at;
        frame_system::Pallet::<T>::set_block_number(expires_at);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ch);

        let record = Ceremonies::<T>::get(ch).unwrap();
        assert!(matches!(record.status, CeremonyStatus::Expired));
    }

    #[benchmark]
    fn batch_cleanup_ceremonies(n: Linear<1, 10>) {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let mut hashes = alloc::vec::Vec::new();

        for i in 0..n {
            let mut ch = [0u8; 32];
            ch[0] = (i + 100) as u8;
            ch[31] = 0xCC;
            let bot_pk = bot_pk_bench(1);
            let bot_id = bot_id_bench(1);
            let participants: BoundedVec<[u8; 32], T::MaxParticipants> =
                alloc::vec![[10u8; 32], [11u8; 32]].try_into().unwrap();
            let now = frame_system::Pallet::<T>::block_number();

            let record = CeremonyRecord::<T> {
                ceremony_mrenclave: mrenclave_bench(1),
                k: 2,
                n: 3,
                bot_public_key: bot_pk,
                participant_count: 2,
                participant_enclaves: participants,
                initiator: caller.clone(),
                created_at: now,
                status: CeremonyStatus::Expired,
                expires_at: now,
                is_re_ceremony: false,
                supersedes: None,
                bot_id_hash: bot_id,
            };
            Ceremonies::<T>::insert(ch, record);
            hashes.push(ch);
        }

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), hashes);
    }

    #[benchmark]
    fn renew_ceremony() {
        let caller: T::AccountId = frame_benchmarking::account("caller", 0, 0);
        let ch = setup_ceremony::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ch);

        let record = Ceremonies::<T>::get(ch).unwrap();
        assert!(matches!(record.status, CeremonyStatus::Active));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);
}
