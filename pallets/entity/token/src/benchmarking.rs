//! Benchmarking for pallet-entity-token
//!
//! 全部 28 个 extrinsics 均有 benchmark。
//! benchmark 通过直接写入存储来构造前置状态，绕过外部 pallet 依赖。

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_support::traits::fungibles::{Create, Mutate, metadata::Mutate as MetadataMutate};
use frame_system::RawOrigin;
use pallet::*;
use pallet_entity_common::{DividendConfig, TokenType, TransferRestrictionMode};
use sp_runtime::traits::{Saturating, Zero};

const ENTITY_1: u64 = 1;

// ==================== Helper 函数 ====================

/// 在 test 环境下设置 mock Entity 状态
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
        crate::mock::register_shop(_eid, id);
    }
}

/// 创建代币并写入存储（绕过 extrinsic 调用，直接构造状态）
fn seed_token<T: Config>(entity_id: u64, owner: &T::AccountId) {
    setup_entity_for::<T>(entity_id, owner);

    let now = frame_system::Pallet::<T>::block_number();
    let config = EntityTokenConfig {
        enabled: true,
        reward_rate: 500,
        exchange_rate: 1000,
        min_redeem: T::AssetBalance::zero(),
        max_redeem_per_order: T::AssetBalance::zero(),
        transferable: true,
        created_at: now,
        token_type: TokenType::Points,
        max_supply: T::AssetBalance::zero(),
        dividend_config: DividendConfig {
            enabled: false,
            min_period: Zero::zero(),
            last_distribution: Zero::zero(),
            accumulated: Zero::zero(),
        },
        transfer_restriction: TransferRestrictionMode::None,
        min_receiver_kyc: 0,
    };
    EntityTokenConfigs::<T>::insert(entity_id, config);

    let name: BoundedVec<u8, T::MaxTokenNameLength> =
        b"BenchToken".to_vec().try_into().unwrap();
    let symbol: BoundedVec<u8, T::MaxTokenSymbolLength> =
        b"BT".to_vec().try_into().unwrap();
    EntityTokenMetadata::<T>::insert(entity_id, (name, symbol, 18u8));
    TotalEntityTokens::<T>::mutate(|n| *n = n.saturating_add(1));

    // 通过 pallet-assets 创建底层资产
    let asset_id = Pallet::<T>::entity_to_asset_id(entity_id);
    let _ = T::Assets::create(asset_id, owner.clone(), true, 1u32.into());
    let _ = T::Assets::set(
        asset_id,
        owner,
        b"BenchToken".to_vec(),
        b"BT".to_vec(),
        18,
    );
}

/// 创建代币并铸造指定数量给目标账户
fn seed_token_with_balance<T: Config>(
    entity_id: u64,
    owner: &T::AccountId,
    holder: &T::AccountId,
    amount: T::AssetBalance,
) {
    seed_token::<T>(entity_id, owner);
    let asset_id = Pallet::<T>::entity_to_asset_id(entity_id);
    let _ = T::Assets::mint_into(asset_id, holder, amount);
}

/// 配置分红（需要 Equity 类型）
fn seed_dividend_config<T: Config>(entity_id: u64) {
    EntityTokenConfigs::<T>::mutate(entity_id, |maybe| {
        if let Some(config) = maybe {
            config.token_type = TokenType::Equity;
            config.dividend_config.enabled = true;
            config.dividend_config.min_period = Zero::zero();
        }
    });
}

/// 生成一个与 caller 不同的 AccountId
fn make_account<T: Config>(seed: u32) -> T::AccountId {
    account("user", seed, 0)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ==================== call_index(0): create_shop_token ====================
    #[benchmark]
    fn create_shop_token() {
        let caller: T::AccountId = whitelisted_caller();
        setup_entity_for::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            ENTITY_1,
            b"BenchToken".to_vec(),
            b"BT".to_vec(),
            18,
            500,
            1000,
        );
    }

    // ==================== call_index(1): update_token_config ====================
    #[benchmark]
    fn update_token_config() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            ENTITY_1,
            Some(800u16),
            Some(2000u16),
            Some(10u128.into()),
            Some(1000u128.into()),
            Some(false),
            Some(true),
        );
    }

    // ==================== call_index(2): mint_tokens ====================
    #[benchmark]
    fn mint_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let recipient: T::AccountId = make_account::<T>(99);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, recipient, 10_000u128.into());
    }

    // ==================== call_index(3): transfer_tokens ====================
    #[benchmark]
    fn transfer_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token_with_balance::<T>(ENTITY_1, &caller, &caller, 50_000u128.into());
        let recipient: T::AccountId = make_account::<T>(99);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, recipient, 10_000u128.into());
    }

    // ==================== call_index(4): configure_dividend ====================
    #[benchmark]
    fn configure_dividend() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        // 需要支持分红的类型
        EntityTokenConfigs::<T>::mutate(ENTITY_1, |maybe| {
            if let Some(config) = maybe {
                config.token_type = TokenType::Equity;
            }
        });
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, true, 100u32.into());
    }

    // ==================== call_index(5): distribute_dividend ====================
    #[benchmark]
    fn distribute_dividend(r: Linear<1, 50>) {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        seed_dividend_config::<T>(ENTITY_1);

        let mut recipients = Vec::new();
        let per_amount: T::AssetBalance = 100u128.into();
        for i in 0..r {
            let acct: T::AccountId = account("recipient", i, 0);
            recipients.push((acct, per_amount));
        }
        let total: T::AssetBalance = (r as u128 * 100u128).into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, total, recipients);
    }

    // ==================== call_index(6): claim_dividend ====================
    #[benchmark]
    fn claim_dividend() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        seed_dividend_config::<T>(ENTITY_1);
        // 写入待领取分红
        let amount: T::AssetBalance = 5_000u128.into();
        PendingDividends::<T>::insert(ENTITY_1, &caller, amount);
        TotalPendingDividends::<T>::insert(ENTITY_1, amount);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(7): lock_tokens ====================
    #[benchmark]
    fn lock_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token_with_balance::<T>(ENTITY_1, &caller, &caller, 50_000u128.into());
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 10_000u128.into(), 100u32.into());
    }

    // ==================== call_index(8): unlock_tokens ====================
    #[benchmark]
    fn unlock_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token_with_balance::<T>(ENTITY_1, &caller, &caller, 50_000u128.into());
        // 插入一条已过期的锁仓条目
        let now = frame_system::Pallet::<T>::block_number();
        let entry = LockEntry {
            amount: 10_000u128.into(),
            unlock_at: now, // 已到期
        };
        let entries: BoundedVec<_, frame_support::traits::ConstU32<10>> =
            vec![entry].try_into().unwrap();
        LockedTokens::<T>::insert(ENTITY_1, &caller, entries);
        // 推进区块使其过期
        frame_system::Pallet::<T>::set_block_number(now.saturating_add(1u32.into()));
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1);
    }

    // ==================== call_index(9): change_token_type ====================
    #[benchmark]
    fn change_token_type() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, TokenType::Governance);
    }

    // ==================== call_index(10): set_max_supply ====================
    #[benchmark]
    fn set_max_supply() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 1_000_000u128.into());
    }

    // ==================== call_index(11): set_transfer_restriction ====================
    #[benchmark]
    fn set_transfer_restriction() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, TransferRestrictionMode::KycRequired, 2u8);
    }

    // ==================== call_index(12): add_to_whitelist ====================
    #[benchmark]
    fn add_to_whitelist(n: Linear<1, 100>) {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let mut accounts = Vec::new();
        for i in 0..n {
            accounts.push(account::<T::AccountId>("wl", i, 0));
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, accounts);
    }

    // ==================== call_index(13): remove_from_whitelist ====================
    #[benchmark]
    fn remove_from_whitelist(n: Linear<1, 100>) {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let mut accounts = Vec::new();
        for i in 0..n {
            let acct: T::AccountId = account("wl", i, 0);
            TransferWhitelist::<T>::insert(ENTITY_1, &acct, ());
            accounts.push(acct);
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, accounts);
    }

    // ==================== call_index(14): add_to_blacklist ====================
    #[benchmark]
    fn add_to_blacklist(n: Linear<1, 100>) {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let mut accounts = Vec::new();
        for i in 0..n {
            accounts.push(account::<T::AccountId>("bl", i, 0));
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, accounts);
    }

    // ==================== call_index(15): remove_from_blacklist ====================
    #[benchmark]
    fn remove_from_blacklist(n: Linear<1, 100>) {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let mut accounts = Vec::new();
        for i in 0..n {
            let acct: T::AccountId = account("bl", i, 0);
            TransferBlacklist::<T>::insert(ENTITY_1, &acct, ());
            accounts.push(acct);
        }
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, accounts);
    }

    // ==================== call_index(16): force_disable_token ====================
    #[benchmark]
    fn force_disable_token() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(17): force_freeze_transfers ====================
    #[benchmark]
    fn force_freeze_transfers() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(18): force_unfreeze_transfers ====================
    #[benchmark]
    fn force_unfreeze_transfers() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        TransfersFrozen::<T>::insert(ENTITY_1, ());
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(19): force_burn ====================
    #[benchmark]
    fn force_burn() {
        let caller: T::AccountId = whitelisted_caller();
        let victim: T::AccountId = make_account::<T>(99);
        seed_token_with_balance::<T>(ENTITY_1, &caller, &victim, 50_000u128.into());
        // 设置关联存储以测试 worst-case 清理路径
        let entry = LockEntry {
            amount: 1_000u128.into(),
            unlock_at: frame_system::Pallet::<T>::block_number().saturating_add(1000u32.into()),
        };
        let entries: BoundedVec<_, frame_support::traits::ConstU32<10>> =
            vec![entry].try_into().unwrap();
        LockedTokens::<T>::insert(ENTITY_1, &victim, entries);
        ReservedTokens::<T>::insert(ENTITY_1, &victim, T::AssetBalance::from(5_000u128));
        PendingDividends::<T>::insert(ENTITY_1, &victim, T::AssetBalance::from(2_000u128));
        TotalPendingDividends::<T>::insert(ENTITY_1, T::AssetBalance::from(2_000u128));
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, victim, 50_000u128.into());
    }

    // ==================== call_index(20): set_global_token_pause ====================
    #[benchmark]
    fn set_global_token_pause() {
        #[extrinsic_call]
        _(RawOrigin::Root, true);
    }

    // ==================== call_index(21): burn_tokens ====================
    #[benchmark]
    fn burn_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token_with_balance::<T>(ENTITY_1, &caller, &caller, 50_000u128.into());
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, 10_000u128.into());
    }

    // ==================== call_index(22): update_token_metadata ====================
    #[benchmark]
    fn update_token_metadata() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            ENTITY_1,
            b"NewName".to_vec(),
            b"NN".to_vec(),
        );
    }

    // ==================== call_index(23): force_transfer ====================
    #[benchmark]
    fn force_transfer() {
        let caller: T::AccountId = whitelisted_caller();
        let from: T::AccountId = make_account::<T>(98);
        let to: T::AccountId = make_account::<T>(99);
        seed_token_with_balance::<T>(ENTITY_1, &caller, &from, 50_000u128.into());
        // 设置关联存储以测试 worst-case 清理路径
        let entry = LockEntry {
            amount: 1_000u128.into(),
            unlock_at: frame_system::Pallet::<T>::block_number().saturating_add(1000u32.into()),
        };
        let entries: BoundedVec<_, frame_support::traits::ConstU32<10>> =
            vec![entry].try_into().unwrap();
        LockedTokens::<T>::insert(ENTITY_1, &from, entries);
        ReservedTokens::<T>::insert(ENTITY_1, &from, T::AssetBalance::from(5_000u128));
        PendingDividends::<T>::insert(ENTITY_1, &from, T::AssetBalance::from(2_000u128));
        TotalPendingDividends::<T>::insert(ENTITY_1, T::AssetBalance::from(2_000u128));
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, from, to, 50_000u128.into());
    }

    // ==================== call_index(24): force_enable_token ====================
    #[benchmark]
    fn force_enable_token() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        // 先禁用
        EntityTokenConfigs::<T>::mutate(ENTITY_1, |maybe| {
            if let Some(config) = maybe {
                config.enabled = false;
            }
        });
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1);
    }

    // ==================== call_index(25): force_cancel_pending_dividends ====================
    #[benchmark]
    fn force_cancel_pending_dividends(n: Linear<1, 50>) {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let mut accounts = Vec::new();
        let mut total = T::AssetBalance::zero();
        for i in 0..n {
            let acct: T::AccountId = account("div", i, 0);
            let amount: T::AssetBalance = 1_000u128.into();
            PendingDividends::<T>::insert(ENTITY_1, &acct, amount);
            total = total.saturating_add(amount);
            accounts.push(acct);
        }
        TotalPendingDividends::<T>::insert(ENTITY_1, total);
        #[extrinsic_call]
        _(RawOrigin::Root, ENTITY_1, accounts);
    }

    // ==================== call_index(26): approve_tokens ====================
    #[benchmark]
    fn approve_tokens() {
        let caller: T::AccountId = whitelisted_caller();
        seed_token::<T>(ENTITY_1, &caller);
        let spender: T::AccountId = make_account::<T>(99);
        #[extrinsic_call]
        _(RawOrigin::Signed(caller), ENTITY_1, spender, 10_000u128.into());
    }

    // ==================== call_index(27): transfer_from ====================
    #[benchmark]
    fn transfer_from() {
        let owner: T::AccountId = make_account::<T>(98);
        let spender: T::AccountId = whitelisted_caller();
        let to: T::AccountId = make_account::<T>(99);
        seed_token_with_balance::<T>(ENTITY_1, &owner, &owner, 50_000u128.into());
        // 设置授权额度
        TokenApprovals::<T>::insert((ENTITY_1, &owner, &spender), T::AssetBalance::from(20_000u128));
        #[extrinsic_call]
        _(RawOrigin::Signed(spender), ENTITY_1, owner, to, 10_000u128.into());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
