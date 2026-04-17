//! Runtime API definitions for pallet-entity-order
//!
//! Provides off-chain query endpoints for order data.

use codec::{Codec, Decode, Encode};
use pallet_entity_common::OrderStatus;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

/// 订单摘要（runtime API 返回用）
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct OrderSummary<AccountId, Balance> {
    pub order_id: u64,
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
    pub token_payment_amount: u128,
    pub token_platform_fee: u128,
    pub status: OrderStatus,
    pub payment_asset: pallet_entity_common::PaymentAsset,
    pub created_at: u64,
    pub shipped_at: Option<u64>,
    pub completed_at: Option<u64>,
}

/// 分页订单查询结果
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct PaginatedOrdersResult<AccountId, Balance> {
    pub orders: Vec<OrderSummary<AccountId, Balance>>,
    pub has_next: bool,
    pub total_count: u32,
}

/// 订单统计信息
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
pub struct OrderStatsInfo<Balance> {
    pub total_orders: u64,
    pub completed_orders: u64,
    pub total_volume: Balance,
    pub total_platform_fees: Balance,
    pub total_token_volume: u128,
    pub total_token_platform_fees: u128,
}

sp_api::decl_runtime_apis! {
    pub trait OrderQueryApi<AccountId, Balance>
    where
        AccountId: Codec,
        Balance: Codec,
    {
        fn get_order(order_id: u64) -> Option<OrderSummary<AccountId, Balance>>;
        fn get_buyer_orders(buyer: AccountId, status_filter: Option<OrderStatus>, page_size: u32, page_index: u32) -> PaginatedOrdersResult<AccountId, Balance>;
        fn get_shop_orders(shop_id: u64, status_filter: Option<OrderStatus>, page_size: u32, page_index: u32) -> PaginatedOrdersResult<AccountId, Balance>;
        fn get_payer_orders(payer: AccountId, status_filter: Option<OrderStatus>, page_size: u32, page_index: u32) -> PaginatedOrdersResult<AccountId, Balance>;
        fn get_order_stats() -> OrderStatsInfo<Balance>;
    }
}
