//! 争议处理子模块
//!
//! 从 lib.rs 提取的 7 个争议/退款相关 extrinsic 实现：
//! - request_refund (call_index 4)
//! - approve_refund (call_index 5)
//! - reject_refund (call_index 11)
//! - force_refund (call_index 13)
//! - force_complete (call_index 14)
//! - force_partial_refund (call_index 20)
//! - withdraw_dispute (call_index 21)

use crate::pallet::*;
use alloc::vec::Vec;
use frame_support::pallet_prelude::*;
use pallet_dispute_escrow::pallet::Escrow as EscrowTrait;
use pallet_entity_common::{OrderStatus, PaymentAsset, ProductCategory};
use sp_runtime::traits::Saturating;

impl<T: Config> Pallet<T> {
    /// 申请退款核心逻辑（数字商品不可退款）
    pub(crate) fn do_request_refund(
        who: T::AccountId,
        order_id: u64,
        reason_cid: Vec<u8>,
    ) -> DispatchResult {
        let bounded_reason = Self::validate_reason_cid(reason_cid)?;

        let now = <frame_system::Pallet<T>>::block_number();
        let expiry_block = now.saturating_add(T::DisputeTimeout::get());

        let payment_asset = Orders::<T>::try_mutate(order_id, |maybe_order| -> Result<PaymentAsset, DispatchError> {
            let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
            ensure!(Self::is_order_participant(order, &who), Error::<T>::NotOrderParticipant);
            ensure!(
                order.product_category != ProductCategory::Digital,
                Error::<T>::DigitalProductCannotRefund
            );
            ensure!(
                order.status == OrderStatus::Paid || order.status == OrderStatus::Shipped,
                Error::<T>::InvalidOrderStatus
            );

            let asset = order.payment_asset.clone();
            order.status = OrderStatus::Disputed;
            order.refund_reason_cid = Some(bounded_reason);
            order.dispute_deadline = Some(expiry_block);
            Ok(asset)
        })?;

        ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
            ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
        })?;

        if payment_asset == PaymentAsset::Native {
            T::Escrow::set_disputed(order_id)?;
        }

        Self::deposit_event(Event::OrderDisputed { order_id });
        Ok(())
    }

    /// 同意退款核心逻辑（卖家）
    pub(crate) fn do_approve_refund(
        who: T::AccountId,
        order_id: u64,
    ) -> DispatchResult {
        let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
        ensure!(order.seller == who, Error::<T>::NotOrderSeller);
        ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);

        if order.payment_asset == PaymentAsset::Native {
            T::Escrow::set_resolved(order_id)?;
        }

        Self::do_cancel_or_refund(&order, order_id, OrderStatus::Refunded)?;

        Self::deposit_event(Event::OrderRefunded {
            order_id,
            amount: order.total_amount,
            token_amount: order.token_payment_amount,
        });
        Ok(())
    }

    /// 拒绝退款核心逻辑（卖家）— 订单保持 Disputed，写入争议超时队列
    pub(crate) fn do_reject_refund(
        who: T::AccountId,
        order_id: u64,
        reason_cid: Vec<u8>,
    ) -> DispatchResult {
        let bounded_reason = Self::validate_reason_cid(reason_cid)?;

        let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
        ensure!(order.seller == who, Error::<T>::NotOrderSeller);
        ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);
        ensure!(!order.dispute_rejected, Error::<T>::DisputeAlreadyRejected);

        let now = <frame_system::Pallet<T>>::block_number();
        let expiry_block = now.saturating_add(T::DisputeTimeout::get());

        // H4: 清理 request_refund 创建的旧超时条目
        if let Some(old_deadline) = order.dispute_deadline {
            ExpiryQueue::<T>::mutate(old_deadline, |ids| {
                ids.retain(|&id| id != order_id);
            });
        }

        // H3-fix: ExpiryQueue 写入先于 Orders 更新，确保队列满时不会产生不一致状态
        ExpiryQueue::<T>::try_mutate(expiry_block, |ids| {
            ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
        })?;

        Orders::<T>::mutate(order_id, |maybe_order| {
            if let Some(o) = maybe_order {
                o.dispute_rejected = true;
                o.dispute_deadline = Some(expiry_block);
            }
        });

        Self::deposit_event(Event::RefundRejected { order_id, reason_cid: bounded_reason.into_inner() });
        Ok(())
    }

    /// 管理员强制退款核心逻辑（Root / 治理）
    pub(crate) fn do_force_refund(
        order_id: u64,
        reason_cid: Option<Vec<u8>>,
    ) -> DispatchResult {
        let reason = Self::validate_optional_reason_cid(&reason_cid)?;

        let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
        ensure!(
            matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
            Error::<T>::CannotForceOrder
        );

        // M4-fix: 传播 set_resolved 错误，避免后续 transfer_from_escrow 因 disputed 状态失败
        if order.status == OrderStatus::Disputed && order.payment_asset == PaymentAsset::Native {
            T::Escrow::set_resolved(order_id)?;
        }

        Self::do_cancel_or_refund(&order, order_id, OrderStatus::Refunded)?;

        Self::deposit_event(Event::OrderForceRefunded { order_id, reason_cid: reason });
        Self::deposit_event(Event::OrderRefunded {
            order_id,
            amount: order.total_amount,
            token_amount: order.token_payment_amount,
        });
        Ok(())
    }

    /// 管理员强制完成订单核心逻辑（Root / 治理）
    pub(crate) fn do_force_complete(
        order_id: u64,
        reason_cid: Option<Vec<u8>>,
    ) -> DispatchResult {
        let reason = Self::validate_optional_reason_cid(&reason_cid)?;

        let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
        ensure!(
            matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
            Error::<T>::CannotForceOrder
        );

        // M4-fix: 传播 set_resolved 错误
        if order.status == OrderStatus::Disputed && order.payment_asset == PaymentAsset::Native {
            T::Escrow::set_resolved(order_id)?;
        }

        Self::do_complete_order(order_id, &order)?;
        Self::deposit_event(Event::OrderForceCompleted { order_id, reason_cid: reason });
        Ok(())
    }

    /// 管理员部分退款核心逻辑（Root，仅 NEX 订单）
    pub(crate) fn do_force_partial_refund(
        order_id: u64,
        refund_bps: u16,
        reason_cid: Option<Vec<u8>>,
    ) -> DispatchResult {
        ensure!(refund_bps >= 1 && refund_bps <= 9999, Error::<T>::InvalidRefundBps);
        let reason = Self::validate_optional_reason_cid(&reason_cid)?;

        let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
        ensure!(
            order.payment_asset == PaymentAsset::Native,
            Error::<T>::PartialRefundNotSupported
        );
        ensure!(
            matches!(order.status, OrderStatus::Paid | OrderStatus::Shipped | OrderStatus::Disputed),
            Error::<T>::CannotForceOrder
        );

        if order.status == OrderStatus::Disputed {
            let _ = T::Escrow::set_resolved(order_id);
        }

        let release_bps = 10000u16.saturating_sub(refund_bps);
        let fund_acct = Self::fund_account(&order);
        T::Escrow::split_partial(order_id, &order.seller, fund_acct, release_bps)?;

        Self::notify_order_cancelled(&order, order_id);

        Orders::<T>::mutate(order_id, |maybe_order| {
            if let Some(o) = maybe_order {
                o.status = OrderStatus::PartiallyRefunded;
            }
        });

        OrderReferrer::<T>::remove(order_id);

        Self::deposit_event(Event::OrderPartialRefunded { order_id, refund_bps, reason_cid: reason });
        Ok(())
    }

    /// 买家撤回争议核心逻辑（仅卖家尚未拒绝时可用）
    pub(crate) fn do_withdraw_dispute(
        who: T::AccountId,
        order_id: u64,
    ) -> DispatchResult {
        let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;
        ensure!(Self::is_order_participant(&order, &who), Error::<T>::NotOrderParticipant);
        ensure!(order.status == OrderStatus::Disputed, Error::<T>::InvalidOrderStatus);
        ensure!(!order.dispute_rejected, Error::<T>::DisputeAlreadyRejected);

        let restored_status = if order.shipped_at.is_some() {
            OrderStatus::Shipped
        } else {
            OrderStatus::Paid
        };

        if order.payment_asset == PaymentAsset::Native {
            T::Escrow::set_resolved(order_id)?;
        }

        // 清理争议超时队列条目
        if let Some(deadline) = order.dispute_deadline {
            ExpiryQueue::<T>::mutate(deadline, |ids| {
                ids.retain(|&id| id != order_id);
            });
        }

        let now = <frame_system::Pallet<T>>::block_number();
        let new_expiry = match restored_status {
            OrderStatus::Shipped => {
                if Self::is_service_like(&order.product_category) {
                    now.saturating_add(T::ServiceConfirmTimeout::get())
                } else {
                    now.saturating_add(T::ConfirmTimeout::get())
                }
            },
            _ => now.saturating_add(T::ShipTimeout::get()),
        };

        ExpiryQueue::<T>::try_mutate(new_expiry, |ids| {
            ids.try_push(order_id).map_err(|_| Error::<T>::ExpiryQueueFull)
        })?;

        Orders::<T>::mutate(order_id, |maybe_order| {
            if let Some(o) = maybe_order {
                o.status = restored_status;
                o.dispute_deadline = None;
                o.dispute_rejected = false;
                o.refund_reason_cid = None;
            }
        });

        Self::deposit_event(Event::DisputeWithdrawn { order_id });
        Ok(())
    }
}
