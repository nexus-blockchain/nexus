//! Storage migration v0 → v1: 为 Order 新增 shopping_balance_used + token_discount_tokens_burned 字段
//!
//! 所有已存在的订单默认 shopping_balance_used = 0, token_discount_tokens_burned = 0

use crate::pallet::*;
use frame_support::{
    pallet_prelude::*,
    traits::{Get, OnRuntimeUpgrade, StorageVersion},
    BoundedVec,
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_entity_common::{OrderStatus, PaymentAsset, ProductCategory};
use sp_runtime::traits::Zero;

/// v0 Order 结构体（不含 shopping_balance_used / token_discount_tokens_burned）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
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

pub struct MigrateV0ToV1<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateV0ToV1<T>
where
    T::AccountId: Clone,
    BalanceOf<T>: Clone + Zero,
{
    fn on_runtime_upgrade() -> Weight {
        let on_chain_version = StorageVersion::get::<Pallet<T>>();
        if on_chain_version != 0 {
            log::info!("order migration: skipping, on-chain version is {:?}", on_chain_version);
            return Weight::zero();
        }

        let mut count = 0u64;
        Orders::<T>::translate::<OldOrder<T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::MaxCidLength>, _>(
            |_key, old| {
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
                    token_discount_tokens_burned: 0,
                })
            },
        );

        StorageVersion::new(1).put::<Pallet<T>>();
        log::info!("order migration v0→v1: migrated {} orders", count);

        T::DbWeight::get().reads_writes(count + 1, count + 1)
    }
}
