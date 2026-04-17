//! Storage migration v1 → v2: 为 Order 新增 shopping_balance_used + token_discount_tokens_burned 字段
//!
//! 所有已存在的订单默认 shopping_balance_used = 0, token_discount_tokens_burned = 0

use crate::pallet::*;
use frame_support::{pallet_prelude::*, traits::OnRuntimeUpgrade, BoundedVec};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_entity_common::{OrderStatus, PaymentAsset, ProductCategory};
use sp_runtime::traits::Zero;

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

/// v1 Order 结构体（含 usdt_total/nex_usdt_rate/token_nex_rate，不含 shopping_balance_used / token_discount_tokens_burned）
#[derive(
    Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxCidLen))]
pub struct OldOrder<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
    pub id: u64,
    pub entity_id: u64,
    pub shop_id: u64,
    pub product_id: u64,
    pub buyer: AccountId,
    pub seller: AccountId,
    pub payer: Option<AccountId>,
    pub quantity: u32,
    pub unit_price: Balance,
    pub total_amount: Balance,
    pub platform_fee: Balance,
    pub usdt_total: u64,
    pub nex_usdt_rate: u64,
    pub token_nex_rate: u128,
    pub product_category: ProductCategory,
    pub shipping_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub tracking_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub status: OrderStatus,
    pub created_at: BlockNumber,
    pub shipped_at: Option<BlockNumber>,
    pub completed_at: Option<BlockNumber>,
    pub service_started_at: Option<BlockNumber>,
    pub service_completed_at: Option<BlockNumber>,
    pub payment_asset: PaymentAsset,
    pub token_payment_amount: u128,
    pub confirm_extended: bool,
    pub dispute_rejected: bool,
    pub dispute_deadline: Option<BlockNumber>,
    pub note_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub refund_reason_cid: Option<BoundedVec<u8, MaxCidLen>>,
}

pub struct MigrateV1ToV2<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateV1ToV2<T>
where
    T::AccountId: Clone,
    BalanceOf<T>: Clone + Zero,
{
    fn on_runtime_upgrade() -> Weight {
        let on_chain_version = Pallet::<T>::on_chain_storage_version();
        if on_chain_version != 1 {
            log::info!(
                "order::MigrateV1ToV2: skipping, on-chain version is {:?}",
                on_chain_version
            );
            return Weight::zero();
        }

        let mut count = 0u64;
        Orders::<T>::translate::<
            OldOrder<T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::MaxCidLength>,
            _,
        >(|_key, old| {
            count += 1;
            Some(Order {
                id: old.id,
                entity_id: old.entity_id,
                shop_id: old.shop_id,
                product_id: old.product_id,
                buyer: old.buyer,
                seller: old.seller,
                payer: old.payer,
                quantity: old.quantity,
                unit_price: old.unit_price,
                total_amount: old.total_amount,
                platform_fee: old.platform_fee,
                usdt_total: old.usdt_total,
                nex_usdt_rate: old.nex_usdt_rate,
                token_nex_rate: old.token_nex_rate,
                product_category: old.product_category,
                shipping_cid: old.shipping_cid,
                tracking_cid: old.tracking_cid,
                status: old.status,
                created_at: old.created_at,
                shipped_at: old.shipped_at,
                completed_at: old.completed_at,
                service_started_at: old.service_started_at,
                service_completed_at: old.service_completed_at,
                payment_asset: old.payment_asset,
                token_payment_amount: old.token_payment_amount,
                confirm_extended: old.confirm_extended,
                dispute_rejected: old.dispute_rejected,
                dispute_deadline: old.dispute_deadline,
                note_cid: old.note_cid,
                refund_reason_cid: old.refund_reason_cid,
                shopping_balance_used: Zero::zero(),
                token_discount_tokens_burned: 0u128,
            })
        });

        StorageVersion::new(2).put::<Pallet<T>>();
        log::info!("order::MigrateV1ToV2: migrated {} orders to v2", count);

        T::DbWeight::get().reads_writes(count + 1, count + 1)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, TryRuntimeError> {
        let count = Orders::<T>::iter().count() as u64;
        Ok(count.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: alloc::vec::Vec<u8>) -> Result<(), TryRuntimeError> {
        let old_count = u64::decode(&mut &state[..])
            .map_err(|_| TryRuntimeError::from("Failed to decode pre_upgrade state"))?;
        let new_count = Orders::<T>::iter().count() as u64;
        assert_eq!(old_count, new_count, "order count mismatch after migration");
        assert_eq!(Pallet::<T>::on_chain_storage_version(), 2);
        Ok(())
    }
}
