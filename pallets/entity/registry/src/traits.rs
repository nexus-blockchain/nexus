//! EntityProvider / EntityFunding trait 实现
//!
//! 为其他 pallet 提供实体数据查询和操作接口。

use crate::pallet::*;
use frame_support::{
    ensure,
    traits::{Currency, ExistenceRequirement, Get},
};
use pallet_entity_common::{EntityProvider, EntityStatus, EntityType, GovernanceMode, GovernanceProvider, OnEntityStatusChange};
use pallet_storage_service::types::EntityFunding;
use sp_runtime::{
    traits::Saturating,
    SaturatedConversion,
};

impl<T: Config> EntityProvider<T::AccountId> for Pallet<T> {
    fn entity_exists(entity_id: u64) -> bool {
        Entities::<T>::contains_key(entity_id)
    }

    fn is_entity_active(entity_id: u64) -> bool {
        Entities::<T>::get(entity_id)
            .map(|s| s.status == EntityStatus::Active)
            .unwrap_or(false)
    }

    fn entity_status(entity_id: u64) -> Option<EntityStatus> {
        Entities::<T>::get(entity_id).map(|s| s.status)
    }

    fn entity_owner(entity_id: u64) -> Option<T::AccountId> {
        Entities::<T>::get(entity_id).map(|s| s.owner)
    }

    fn entity_account(entity_id: u64) -> T::AccountId {
        Self::entity_treasury_account(entity_id)
    }

    fn entity_type(entity_id: u64) -> Option<EntityType> {
        Entities::<T>::get(entity_id).map(|e| e.entity_type)
    }

    fn update_entity_stats(entity_id: u64, sales_amount: u128, order_count: u32) -> Result<(), sp_runtime::DispatchError> {
        // L2 修复: 不允许对 Banned/Closed Entity 更新统计
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(
            !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
            Error::<T>::EntityNotActive
        );
        // 使用独立 EntitySales 存储，避免每次都读写整个 Entity struct
        EntitySales::<T>::mutate(entity_id, |data| {
            data.total_sales = data.total_sales.saturating_add(sales_amount.saturated_into());
            data.total_orders = data.total_orders.saturating_add(order_count as u64);
        });
        Ok(())
    }

    fn register_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        // M1: 验证 Entity 存在且状态允许注册 Shop（防御性检查，防止 Banned/Closed Entity 注册新 Shop）
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(
            !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
            Error::<T>::EntityNotActive
        );

        // 添加到 EntityShops 列表
        EntityShops::<T>::try_mutate(entity_id, |shops| -> Result<(), sp_runtime::DispatchError> {
            // H5 修复: 区分重复注册和容量已满两种错误
            ensure!(!shops.contains(&shop_id), Error::<T>::ShopAlreadyRegistered);
            shops.try_push(shop_id).map_err(|_| Error::<T>::ShopLimitReached)?;
            Ok(())
        })?;

        // 如果是第一个 Shop，设为 primary
        Entities::<T>::mutate(entity_id, |maybe_entity| {
            if let Some(entity) = maybe_entity {
                if entity.primary_shop_id == 0 {
                    entity.primary_shop_id = shop_id;
                }
            }
        });

        Self::deposit_event(Event::ShopAddedToEntity { entity_id, shop_id });
        Ok(())
    }

    fn unregister_shop(entity_id: u64, shop_id: u64) -> Result<(), sp_runtime::DispatchError> {
        // M2-R11: 在 try_mutate 中捕获剩余首个 shop_id，避免二次读取 EntityShops
        let new_first = EntityShops::<T>::try_mutate(entity_id, |shops| -> Result<Option<u64>, sp_runtime::DispatchError> {
            let pos = shops.iter().position(|&id| id == shop_id)
                .ok_or(Error::<T>::ShopNotRegistered)?;
            shops.remove(pos);
            Ok(shops.first().copied())
        })?;

        // 如果移除的是 primary，重新指定（取列表第一个，或清零）
        Entities::<T>::mutate(entity_id, |maybe_entity| {
            if let Some(entity) = maybe_entity {
                if entity.primary_shop_id == shop_id {
                    entity.primary_shop_id = new_first.unwrap_or(0);
                }
            }
        });

        // L3: 与 register_shop 的 ShopAddedToEntity 对称
        Self::deposit_event(Event::ShopRemovedFromEntity { entity_id, shop_id });
        Ok(())
    }

    fn is_entity_admin(entity_id: u64, account: &T::AccountId, required_permission: u32) -> bool {
        Self::has_permission(entity_id, account, required_permission)
    }

    fn entity_shops(entity_id: u64) -> sp_std::vec::Vec<u64> {
        EntityShops::<T>::get(entity_id).into_inner()
    }

    fn set_primary_shop_id(entity_id: u64, shop_id: u64) {
        Entities::<T>::mutate(entity_id, |maybe_entity| {
            if let Some(entity) = maybe_entity {
                entity.primary_shop_id = shop_id;
            }
        });
    }

    fn get_primary_shop_id(entity_id: u64) -> u64 {
        Entities::<T>::get(entity_id)
            .map(|e| e.primary_shop_id)
            .unwrap_or(0)
    }

    // H4 修复: 实现 pause_entity/resume_entity（供治理模块调用）
    fn pause_entity(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.status == EntityStatus::Active, Error::<T>::InvalidEntityStatus);

        Entities::<T>::mutate(entity_id, |s| {
            if let Some(e) = s {
                e.status = EntityStatus::Suspended;
            }
        });
        EntityStats::<T>::mutate(|stats| {
            stats.active_entities = stats.active_entities.saturating_sub(1);
        });
        GovernanceSuspended::<T>::insert(entity_id, true);

        T::OnEntityStatusChange::on_entity_suspended(entity_id);

        Self::deposit_event(Event::EntityStatusChanged {
            entity_id,
            status: EntityStatus::Suspended,
        });
        Ok(())
    }

    fn resume_entity(entity_id: u64) -> Result<(), sp_runtime::DispatchError> {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.status == EntityStatus::Suspended, Error::<T>::InvalidEntityStatus);

        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);
        let min_balance = T::MinOperatingBalance::get();
        ensure!(balance >= min_balance, Error::<T>::InsufficientOperatingFund);

        Entities::<T>::mutate(entity_id, |s| {
            if let Some(e) = s {
                e.status = EntityStatus::Active;
            }
        });
        EntityStats::<T>::mutate(|stats| {
            stats.active_entities = stats.active_entities.saturating_add(1);
        });
        GovernanceSuspended::<T>::remove(entity_id);
        OwnerPaused::<T>::remove(entity_id);
        SuspensionReasons::<T>::remove(entity_id);

        T::OnEntityStatusChange::on_entity_resumed(entity_id);

        Self::deposit_event(Event::EntityStatusChanged {
            entity_id,
            status: EntityStatus::Active,
        });
        Ok(())
    }

    fn is_entity_locked(entity_id: u64) -> bool {
        T::GovernanceProvider::is_governance_locked(entity_id)
    }

    // M1-R11: 实现元数据查询方法（替代默认空实现）
    fn entity_name(entity_id: u64) -> sp_std::vec::Vec<u8> {
        Entities::<T>::get(entity_id)
            .map(|e| e.name.into_inner())
            .unwrap_or_default()
    }

    fn entity_metadata_cid(entity_id: u64) -> Option<sp_std::vec::Vec<u8>> {
        Entities::<T>::get(entity_id)
            .and_then(|e| e.metadata_uri.map(|c| c.into_inner()))
    }

    fn entity_description(entity_id: u64) -> sp_std::vec::Vec<u8> {
        Entities::<T>::get(entity_id)
            .and_then(|e| e.description_cid.map(|c| c.into_inner()))
            .unwrap_or_default()
    }

    fn payment_config(entity_id: u64) -> pallet_entity_common::PaymentConfig {
        EntityPaymentConfigs::<T>::get(entity_id)
    }

    // C2: 治理 pallet 同步调用 — 更新 Entity.governance_mode 字段
    // M2: 添加状态校验，拒绝对 Banned/Closed 实体的修改
    fn set_governance_mode(entity_id: u64, mode: GovernanceMode) -> Result<(), sp_runtime::DispatchError> {
        let old_mode = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<GovernanceMode, sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                entity.status != EntityStatus::Banned && entity.status != EntityStatus::Closed,
                Error::<T>::InvalidEntityStatus
            );
            let old = entity.governance_mode;
            entity.governance_mode = mode;
            Ok(old)
        })?;

        // L4 审计修复: 发射事件以保留审计轨迹
        if old_mode != mode {
            Self::deposit_event(Event::GovernanceModeChanged {
                entity_id,
                old_mode,
                new_mode: mode,
            });
        }
        Ok(())
    }
}

impl<T: Config> EntityFunding<T::AccountId, BalanceOf<T>> for Pallet<T> {
    fn try_charge_entity(
        entity_id: u64,
        amount: BalanceOf<T>,
        dest: &T::AccountId,
    ) -> Result<bool, sp_runtime::DispatchError> {
        let entity = match Entities::<T>::get(entity_id) {
            Some(e) => e,
            None => return Ok(false),
        };

        if !matches!(entity.status, EntityStatus::Active | EntityStatus::Suspended) {
            return Ok(false);
        }

        let treasury = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury);
        if balance < amount {
            return Ok(false);
        }

        // H2 审计修复: 使用 AllowDeath 避免精确余额扣款因 KeepAlive 失败，
        // 转账失败时优雅返回 Ok(false) 而非向上传播 Err
        match T::Currency::transfer(
            &treasury,
            dest,
            amount,
            ExistenceRequirement::AllowDeath,
        ) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

// ============================================================================
// EntityTreasuryPort 实现
// ============================================================================

impl<T: Config> pallet_entity_common::EntityTreasuryPort for Pallet<T> {
    fn treasury_balance(entity_id: u64) -> u128 {
        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);
        SaturatedConversion::saturated_into(balance)
    }
}
