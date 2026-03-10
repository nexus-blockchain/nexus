//! Benchmarking for pallet-entity-tokensale
//!
//! 全部 27 个 extrinsics 均有 benchmark。
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_support::traits::{Currency, Get};
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::{EntityProvider, EntityTokenProvider};
use sp_runtime::traits::{Saturating, Zero};
use sp_runtime::SaturatedConversion;

const ENTITY_1: u64 = 100;

/// 便捷函数：u128 → BalanceOf<T>
fn bal<T: Config>(v: u128) -> BalanceOf<T> {
    v.saturated_into()
}

// ==================== Helper 函数 ====================

/// 在 test 环境下设置 mock 状态（Entity owner + 代币余额）
fn setup_entity_for<T: Config>(_eid: u64, _owner: &T::AccountId) {
    #[cfg(test)]
    {
        use codec::Encode;
        let bytes = _owner.encode();
        let id = if bytes.len() >= 8 {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            0u64
        };
        // 设置 Entity owner 为 caller
        crate::mock::set_entity_owner(_eid, id);
        // 设置 Entity 代币余额给 entity_account
        let entity_account = crate::mock::ENTITY_ACCOUNT;
        crate::mock::MockTokenProvider::set_balance(
            _eid, entity_account, crate::mock::TOKEN_SUPPLY,
        );
        // 给 caller 充值 NEX（通过 deposit_creating）
        let _ = <T::Currency as Currency<T::AccountId>>::deposit_creating(
            _owner, bal::<T>(crate::mock::INITIAL_BALANCE),
        );
    }
}

/// 创建一个 NotStarted 轮次并写入存储
fn seed_not_started_round<T: Config>(entity_id: u64, creator: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let round_id = NextRoundId::<T>::get();
    SaleRounds::<T>::insert(round_id, SaleRound {
        id: round_id, entity_id, mode: SaleMode::FixedPrice,
        status: RoundStatus::NotStarted,
        total_supply: bal::<T>(1_000_000), sold_amount: Zero::zero(),
        remaining_amount: bal::<T>(1_000_000),
        participants_count: 0, payment_options_count: 0,
        vesting_config: VestingConfig {
            vesting_type: VestingType::None, initial_unlock_bps: 10000,
            cliff_duration: Zero::zero(), total_duration: Zero::zero(),
            unlock_interval: Zero::zero(),
        },
        kyc_required: false, min_kyc_level: 0,
        start_block: now.saturating_add(10u32.into()),
        end_block: now.saturating_add(1000u32.into()),
        dutch_start_price: None, dutch_end_price: None,
        creator: creator.clone(), created_at: now,
        funds_withdrawn: false, cancelled_at: None,
        total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
        soft_cap: Zero::zero(),
    });
    NextRoundId::<T>::put(round_id.saturating_add(1));
    EntityRounds::<T>::mutate(entity_id, |r| { let _ = r.try_push(round_id); });
    round_id
}

/// 创建一个 Active 轮次（含支付选项）
fn seed_active_round<T: Config>(entity_id: u64, creator: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let round_id = NextRoundId::<T>::get();
    SaleRounds::<T>::insert(round_id, SaleRound {
        id: round_id, entity_id, mode: SaleMode::FixedPrice,
        status: RoundStatus::Active,
        total_supply: bal::<T>(1_000_000), sold_amount: Zero::zero(),
        remaining_amount: bal::<T>(1_000_000),
        participants_count: 0, payment_options_count: 1,
        vesting_config: VestingConfig {
            vesting_type: VestingType::None, initial_unlock_bps: 10000,
            cliff_duration: Zero::zero(), total_duration: Zero::zero(),
            unlock_interval: Zero::zero(),
        },
        kyc_required: false, min_kyc_level: 0,
        start_block: now, end_block: now.saturating_add(1000u32.into()),
        dutch_start_price: None, dutch_end_price: None,
        creator: creator.clone(), created_at: now,
        funds_withdrawn: false, cancelled_at: None,
        total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
        soft_cap: Zero::zero(),
    });
    NextRoundId::<T>::put(round_id.saturating_add(1));
    EntityRounds::<T>::mutate(entity_id, |r| { let _ = r.try_push(round_id); });
    RoundPaymentOptions::<T>::mutate(round_id, |opts| {
        let _ = opts.try_push(PaymentConfig {
            asset_id: None, price: bal::<T>(100), min_purchase: bal::<T>(10),
            max_purchase_per_account: bal::<T>(100_000), enabled: true,
        });
    });
    ActiveRounds::<T>::mutate(|a| { let _ = a.try_push(round_id); });
    round_id
}

/// 创建一个 Ended 轮次
fn seed_ended_round<T: Config>(entity_id: u64, creator: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let round_id = NextRoundId::<T>::get();
    SaleRounds::<T>::insert(round_id, SaleRound {
        id: round_id, entity_id, mode: SaleMode::FixedPrice,
        status: RoundStatus::Ended,
        total_supply: bal::<T>(1_000_000), sold_amount: bal::<T>(500_000),
        remaining_amount: Zero::zero(),
        participants_count: 1, payment_options_count: 1,
        vesting_config: VestingConfig {
            vesting_type: VestingType::None, initial_unlock_bps: 10000,
            cliff_duration: Zero::zero(), total_duration: Zero::zero(),
            unlock_interval: Zero::zero(),
        },
        kyc_required: false, min_kyc_level: 0,
        start_block: now, end_block: now,
        dutch_start_price: None, dutch_end_price: None,
        creator: creator.clone(), created_at: now,
        funds_withdrawn: false, cancelled_at: None,
        total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
        soft_cap: Zero::zero(),
    });
    NextRoundId::<T>::put(round_id.saturating_add(1));
    EntityRounds::<T>::mutate(entity_id, |r| { let _ = r.try_push(round_id); });
    round_id
}

/// 创建一个 Cancelled 轮次
fn seed_cancelled_round<T: Config>(entity_id: u64, creator: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let round_id = NextRoundId::<T>::get();
    SaleRounds::<T>::insert(round_id, SaleRound {
        id: round_id, entity_id, mode: SaleMode::FixedPrice,
        status: RoundStatus::Cancelled,
        total_supply: bal::<T>(1_000_000), sold_amount: bal::<T>(100_000),
        remaining_amount: Zero::zero(),
        participants_count: 1, payment_options_count: 1,
        vesting_config: VestingConfig {
            vesting_type: VestingType::None, initial_unlock_bps: 10000,
            cliff_duration: Zero::zero(), total_duration: Zero::zero(),
            unlock_interval: Zero::zero(),
        },
        kyc_required: false, min_kyc_level: 0,
        start_block: now, end_block: now,
        dutch_start_price: None, dutch_end_price: None,
        creator: creator.clone(), created_at: now,
        funds_withdrawn: false, cancelled_at: Some(now),
        total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
        soft_cap: Zero::zero(),
    });
    NextRoundId::<T>::put(round_id.saturating_add(1));
    EntityRounds::<T>::mutate(entity_id, |r| { let _ = r.try_push(round_id); });
    round_id
}

/// 创建一个 Paused 轮次
fn seed_paused_round<T: Config>(entity_id: u64, creator: &T::AccountId) -> u64 {
    let now = frame_system::Pallet::<T>::block_number();
    let round_id = NextRoundId::<T>::get();
    SaleRounds::<T>::insert(round_id, SaleRound {
        id: round_id, entity_id, mode: SaleMode::FixedPrice,
        status: RoundStatus::Paused,
        total_supply: bal::<T>(1_000_000), sold_amount: Zero::zero(),
        remaining_amount: bal::<T>(1_000_000),
        participants_count: 0, payment_options_count: 1,
        vesting_config: VestingConfig {
            vesting_type: VestingType::None, initial_unlock_bps: 10000,
            cliff_duration: Zero::zero(), total_duration: Zero::zero(),
            unlock_interval: Zero::zero(),
        },
        kyc_required: false, min_kyc_level: 0,
        start_block: now, end_block: now.saturating_add(1000u32.into()),
        dutch_start_price: None, dutch_end_price: None,
        creator: creator.clone(), created_at: now,
        funds_withdrawn: false, cancelled_at: None,
        total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
        soft_cap: Zero::zero(),
    });
    NextRoundId::<T>::put(round_id.saturating_add(1));
    EntityRounds::<T>::mutate(entity_id, |r| { let _ = r.try_push(round_id); });
    ActiveRounds::<T>::mutate(|a| { let _ = a.try_push(round_id); });
    round_id
}

/// 插入认购记录
fn seed_subscription<T: Config>(round_id: u64, subscriber: &T::AccountId, amount: u128, payment: u128) {
    let now = frame_system::Pallet::<T>::block_number();
    Subscriptions::<T>::insert(round_id, subscriber, Subscription {
        subscriber: subscriber.clone(), round_id,
        amount: bal::<T>(amount), payment_asset: None,
        payment_amount: bal::<T>(payment), subscribed_at: now,
        claimed: false, unlocked_amount: Zero::zero(), refunded: false,
    });
    RoundParticipants::<T>::mutate(round_id, |p| { let _ = p.try_push(subscriber.clone()); });
}

/// 插入已 claimed 的认购记录
fn seed_claimed_subscription<T: Config>(round_id: u64, subscriber: &T::AccountId, amount: u128, payment: u128) {
    let now = frame_system::Pallet::<T>::block_number();
    Subscriptions::<T>::insert(round_id, subscriber, Subscription {
        subscriber: subscriber.clone(), round_id,
        amount: bal::<T>(amount), payment_asset: None,
        payment_amount: bal::<T>(payment), subscribed_at: now,
        claimed: true, unlocked_amount: Zero::zero(), refunded: false,
    });
    RoundParticipants::<T>::mutate(round_id, |p| { let _ = p.try_push(subscriber.clone()); });
}

/// 创建支付选项的便捷函数
fn make_payment<T: Config>() -> PaymentConfig<AssetIdOf<T>, BalanceOf<T>> {
    PaymentConfig {
        asset_id: None, price: bal::<T>(100), min_purchase: bal::<T>(10),
        max_purchase_per_account: bal::<T>(100_000), enabled: true,
    }
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn create_sale_round() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, SaleMode::FixedPrice,
          bal::<T>(1_000_000), 10u32.into(), 1000u32.into(), false, 0, Zero::zero());
    }

    #[benchmark]
    fn add_payment_option() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, None, bal::<T>(100), bal::<T>(10), bal::<T>(100_000));
    }

    #[benchmark]
    fn set_vesting_config() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, VestingType::Linear, 5000u16,
          100u32.into(), 1000u32.into(), 10u32.into());
    }

    #[benchmark]
    fn configure_dutch_auction() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        let rid = NextRoundId::<T>::get();
        SaleRounds::<T>::insert(rid, SaleRound {
            id: rid, entity_id: ENTITY_1, mode: SaleMode::DutchAuction,
            status: RoundStatus::NotStarted,
            total_supply: bal::<T>(1_000_000), sold_amount: Zero::zero(),
            remaining_amount: bal::<T>(1_000_000),
            participants_count: 0, payment_options_count: 0,
            vesting_config: VestingConfig {
                vesting_type: VestingType::None, initial_unlock_bps: 10000,
                cliff_duration: Zero::zero(), total_duration: Zero::zero(),
                unlock_interval: Zero::zero(),
            },
            kyc_required: false, min_kyc_level: 0,
            start_block: now.saturating_add(10u32.into()),
            end_block: now.saturating_add(1000u32.into()),
            dutch_start_price: None, dutch_end_price: None,
            creator: caller.clone(), created_at: now,
            funds_withdrawn: false, cancelled_at: None,
            total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
            soft_cap: Zero::zero(),
        });
        NextRoundId::<T>::put(rid.saturating_add(1));
        EntityRounds::<T>::mutate(ENTITY_1, |r| { let _ = r.try_push(rid); });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, bal::<T>(1000), bal::<T>(100));
    }

    #[benchmark]
    fn add_to_whitelist() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        let a1: T::AccountId = account("wl", 0, 0);
        let a2: T::AccountId = account("wl", 1, 0);
        let list: BoundedVec<(T::AccountId, Option<BalanceOf<T>>), T::MaxWhitelistSize> =
            vec![(a1, None), (a2, Some(bal::<T>(5000)))].try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, list);
    }

    #[benchmark]
    fn start_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        RoundPaymentOptions::<T>::mutate(rid, |o| { let _ = o.try_push(make_payment::<T>()); });
        // 更新 payment_options_count 以通过校验
        SaleRounds::<T>::mutate(rid, |r| {
            if let Some(round) = r { round.payment_options_count = 1; }
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn subscribe() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(1_000_000));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, bal::<T>(100), None);
    }

    #[benchmark]
    fn end_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        SaleRounds::<T>::mutate(rid, |r| {
            if let Some(round) = r {
                round.remaining_amount = Zero::zero();
                round.sold_amount = round.total_supply;
            }
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn claim_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_ended_round::<T>(ENTITY_1, &caller);
        seed_subscription::<T>(rid, &caller, 500_000, 50_000_000);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(500_000));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn unlock_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        let rid = NextRoundId::<T>::get();
        SaleRounds::<T>::insert(rid, SaleRound {
            id: rid, entity_id: ENTITY_1, mode: SaleMode::FixedPrice,
            status: RoundStatus::Ended,
            total_supply: bal::<T>(1_000_000), sold_amount: bal::<T>(500_000),
            remaining_amount: Zero::zero(),
            participants_count: 1, payment_options_count: 1,
            vesting_config: VestingConfig {
                vesting_type: VestingType::Linear, initial_unlock_bps: 5000,
                cliff_duration: Zero::zero(), total_duration: 100u32.into(),
                unlock_interval: 10u32.into(),
            },
            kyc_required: false, min_kyc_level: 0,
            start_block: now, end_block: now,
            dutch_start_price: None, dutch_end_price: None,
            creator: caller.clone(), created_at: now,
            funds_withdrawn: false, cancelled_at: None,
            total_refunded_tokens: Zero::zero(), total_refunded_nex: Zero::zero(),
            soft_cap: Zero::zero(),
        });
        NextRoundId::<T>::put(rid.saturating_add(1));
        EntityRounds::<T>::mutate(ENTITY_1, |r| { let _ = r.try_push(rid); });
        seed_claimed_subscription::<T>(rid, &caller, 500_000, 50_000_000);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(500_000));
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(200u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn cancel_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(1_000_000));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn claim_refund() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_cancelled_round::<T>(ENTITY_1, &caller);
        seed_subscription::<T>(rid, &caller, 100_000, 10_000_000);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(100_000));
        let pa = Pallet::<T>::pallet_account();
        let _ = T::Currency::deposit_creating(&pa, bal::<T>(10_000_000));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn withdraw_funds() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_ended_round::<T>(ENTITY_1, &caller);
        RaisedFunds::<T>::insert(rid, Option::<AssetIdOf<T>>::None, bal::<T>(50_000_000));
        let pa = Pallet::<T>::pallet_account();
        let _ = T::Currency::deposit_creating(&pa, bal::<T>(50_000_000));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn reclaim_unclaimed_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let now = frame_system::Pallet::<T>::block_number();
        let rid = seed_cancelled_round::<T>(ENTITY_1, &caller);
        SaleRounds::<T>::mutate(rid, |r| {
            if let Some(round) = r { round.cancelled_at = Some(now); }
        });
        let sub: T::AccountId = account("sub", 0, 0);
        seed_subscription::<T>(rid, &sub, 100_000, 10_000_000);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(100_000));
        let pa = Pallet::<T>::pallet_account();
        let _ = T::Currency::deposit_creating(&pa, bal::<T>(10_000_000));
        frame_system::Pallet::<T>::set_block_number(
            now.saturating_add(T::RefundGracePeriod::get()).saturating_add(1u32.into())
        );
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn force_cancel_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(1_000_000));
        #[extrinsic_call]
        _(RawOrigin::Root, rid);
    }

    #[benchmark]
    fn force_end_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(1_000_000));
        #[extrinsic_call]
        _(RawOrigin::Root, rid);
    }

    #[benchmark]
    fn force_refund() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_cancelled_round::<T>(ENTITY_1, &caller);
        let sub: T::AccountId = account("sub", 0, 0);
        seed_subscription::<T>(rid, &sub, 100_000, 10_000_000);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(100_000));
        let pa = Pallet::<T>::pallet_account();
        let _ = T::Currency::deposit_creating(&pa, bal::<T>(10_000_000));
        #[extrinsic_call]
        _(RawOrigin::Root, rid, sub);
    }

    #[benchmark]
    fn force_withdraw_funds() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_ended_round::<T>(ENTITY_1, &caller);
        RaisedFunds::<T>::insert(rid, Option::<AssetIdOf<T>>::None, bal::<T>(50_000_000));
        let pa = Pallet::<T>::pallet_account();
        let _ = T::Currency::deposit_creating(&pa, bal::<T>(50_000_000));
        #[extrinsic_call]
        _(RawOrigin::Root, rid);
    }

    #[benchmark]
    fn update_sale_round() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid,
          Some(bal::<T>(2_000_000)), None, None, None, None);
    }

    #[benchmark]
    fn increase_subscription() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(1_000_000));
        seed_subscription::<T>(rid, &caller, 100, 10_000);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, bal::<T>(50), None);
    }

    #[benchmark]
    fn remove_from_whitelist() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        let a1: T::AccountId = account("wl", 0, 0);
        RoundWhitelist::<T>::insert(rid, &a1, Option::<BalanceOf<T>>::None);
        WhitelistCount::<T>::insert(rid, 1u32);
        let list: BoundedVec<T::AccountId, T::MaxWhitelistSize> =
            vec![a1].try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, list);
    }

    #[benchmark]
    fn remove_payment_option() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_not_started_round::<T>(ENTITY_1, &caller);
        RoundPaymentOptions::<T>::mutate(rid, |o| { let _ = o.try_push(make_payment::<T>()); });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, 0u32);
    }

    #[benchmark]
    fn extend_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        let end = SaleRounds::<T>::get(rid).unwrap().end_block;
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid, end.saturating_add(500u32.into()));
    }

    #[benchmark]
    fn pause_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_active_round::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn resume_sale() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_paused_round::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn cleanup_round() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_ended_round::<T>(ENTITY_1, &caller);
        SaleRounds::<T>::mutate(rid, |r| {
            if let Some(round) = r { round.funds_withdrawn = true; }
        });
        let sub: T::AccountId = account("sub", 0, 0);
        seed_subscription::<T>(rid, &sub, 100_000, 10_000_000);
        RoundPaymentOptions::<T>::mutate(rid, |o| { let _ = o.try_push(make_payment::<T>()); });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), rid);
    }

    #[benchmark]
    fn force_batch_refund() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        let rid = seed_cancelled_round::<T>(ENTITY_1, &caller);
        let s1: T::AccountId = account("sub", 0, 0);
        let s2: T::AccountId = account("sub", 1, 0);
        seed_subscription::<T>(rid, &s1, 50_000, 5_000_000);
        seed_subscription::<T>(rid, &s2, 50_000, 5_000_000);
        let ea = T::EntityProvider::entity_account(ENTITY_1);
        let _ = T::TokenProvider::reserve(ENTITY_1, &ea, bal::<T>(100_000));
        let pa = Pallet::<T>::pallet_account();
        let _ = T::Currency::deposit_creating(&pa, bal::<T>(10_000_000));
        let subs: BoundedVec<T::AccountId, T::MaxBatchRefund> =
            vec![s1, s2].try_into().unwrap();
        #[extrinsic_call]
        _(RawOrigin::Root, rid, subs);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
