//! Owner 操作模块
//!
//! 包含充值、升级实体类型、绑定推荐人、撤销关闭申请、设置 Primary Shop、
//! Owner 主动暂停/恢复等。

use crate::pallet::*;
use frame_support::{
    ensure,
    traits::{Currency, ExistenceRequirement, Get},
};
use pallet_entity_common::{AdminPermission, EntityStatus, EntityType, GovernanceMode, GovernanceProvider, OnEntityStatusChange};
use sp_runtime::traits::Zero;

impl<T: Config> Pallet<T> {
    /// 充值金库资金
    pub(crate) fn do_top_up_fund(
        who: T::AccountId,
        entity_id: u64,
        amount: BalanceOf<T>,
    ) -> sp_runtime::DispatchResult {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        // b2: owner 或拥有 ENTITY_MANAGE 权限的 admin 均可充值
        ensure!(
            entity.owner == who || entity.admins.iter().any(|(a, perm)| a == &who && (perm & AdminPermission::ENTITY_MANAGE) != 0),
            Error::<T>::NotEntityOwner
        );
        ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
        ensure!(
            entity.status != EntityStatus::Closed && entity.status != EntityStatus::Banned,
            Error::<T>::InvalidEntityStatus
        );
        // M1: 充值金额不能为零
        ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

        let treasury_account = Self::entity_treasury_account(entity_id);

        T::Currency::transfer(
            &who,
            &treasury_account,
            amount,
            ExistenceRequirement::KeepAlive,
        )?;

        let new_balance = T::Currency::free_balance(&treasury_account);
        let min_balance = T::MinOperatingBalance::get();

        // 仅资金不足暂停可自动恢复；治理暂停/Owner 主动暂停需要显式恢复
        if entity.status == EntityStatus::Suspended
            && new_balance >= min_balance
            && !GovernanceSuspended::<T>::get(entity_id)
            && !OwnerPaused::<T>::get(entity_id)
        {
            Entities::<T>::mutate(entity_id, |s| {
                if let Some(e) = s {
                    e.status = EntityStatus::Active;
                }
            });
            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_add(1);
            });
            T::OnEntityStatusChange::on_entity_resumed(entity_id);
            Self::deposit_event(Event::EntityResumedAfterFunding { entity_id });
        }

        Self::deposit_event(Event::FundToppedUp {
            entity_id,
            amount,
            new_balance,
        });

        Ok(())
    }

    /// 升级实体类型（需治理批准或满足条件）
    pub(crate) fn do_upgrade_entity_type(
        who: Option<T::AccountId>,
        entity_id: u64,
        new_type: EntityType,
        new_governance: GovernanceMode,
    ) -> sp_runtime::DispatchResult {
        let (old_type, old_mode) = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<(EntityType, GovernanceMode), sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed),
                Error::<T>::InvalidEntityStatus
            );
            
            // M2 修复: 同类型且同治理模式不需要升级
            ensure!(
                entity.entity_type != new_type || entity.governance_mode != new_governance,
                Error::<T>::SameEntityType
            );

            // 非治理操作需要是所有者，且受升级路径限制
            if let Some(ref caller) = who {
                ensure!(entity.owner == *caller, Error::<T>::NotEntityOwner);
                ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
                Self::validate_entity_type_upgrade(&entity.entity_type, &new_type)?;
            }
            // 治理 origin 可任意升级类型，不受路径限制
            
            // DAO 类型需要治理模式
            if new_type == EntityType::DAO {
                ensure!(new_governance != GovernanceMode::None, Error::<T>::DAORequiresGovernance);
            }
            
            let old_t = entity.entity_type;
            let old_g = entity.governance_mode;
            entity.entity_type = new_type;
            entity.governance_mode = new_governance;
            
            Ok((old_t, old_g))
        })?;

        // M1 Round 8: 仅在类型实际变更时发射 EntityTypeUpgraded，
        // 避免仅治理模式变更时产生误导性 "类型升级" 事件
        if old_type != new_type {
            Self::deposit_event(Event::EntityTypeUpgraded {
                entity_id,
                old_type,
                new_type,
            });
        }
        
        if old_mode != new_governance {
            Self::deposit_event(Event::GovernanceModeChanged {
                entity_id,
                old_mode,
                new_mode: new_governance,
            });
        }
        
        Ok(())
    }

    /// 补绑 Entity 推荐人（仅限创建时未填的，一次性操作）
    pub(crate) fn do_bind_entity_referrer(
        who: T::AccountId,
        entity_id: u64,
        referrer: T::AccountId,
    ) -> sp_runtime::DispatchResult {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
        ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
        ensure!(entity.status == EntityStatus::Active, Error::<T>::EntityNotActive);

        // 不能已有推荐人
        ensure!(!EntityReferrer::<T>::contains_key(entity_id), Error::<T>::ReferrerAlreadyBound);

        // 验证推荐人（允许 Suspended/Pending 等非终态实体的 owner 作为推荐人）
        ensure!(referrer != who, Error::<T>::SelfReferral);
        ensure!(Self::has_operable_entity(&referrer), Error::<T>::InvalidReferrer);

        // 写入
        EntityReferrer::<T>::insert(entity_id, &referrer);
        // b8: 维护反向索引
        ReferrerEntities::<T>::try_mutate(&referrer, |entities| -> Result<(), sp_runtime::DispatchError> {
            entities.try_push(entity_id).map_err(|_| Error::<T>::ReferrerIndexFull)?;
            Ok(())
        })?;

        Self::deposit_event(Event::EntityReferrerBound {
            entity_id,
            referrer,
        });

        Ok(())
    }

    /// 撤销关闭申请（Owner，PendingClose → Active/Suspended）
    pub(crate) fn do_cancel_close_request(who: T::AccountId, entity_id: u64) -> sp_runtime::DispatchResult {
        // L3 审计修复: 使用共用 helper 消除重复逻辑
        let restore_to_active = Self::should_restore_to_active(entity_id);

        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(entity.status == EntityStatus::PendingClose, Error::<T>::InvalidEntityStatus);

            entity.status = if restore_to_active { EntityStatus::Active } else { EntityStatus::Suspended };
            Ok(())
        })?;

        Self::finalize_pending_close_restore(entity_id, restore_to_active);

        Self::deposit_event(Event::CloseRequestCancelled { entity_id });
        Ok(())
    }

    /// 设置 Primary Shop（owner 或 ENTITY_MANAGE admin）
    pub(crate) fn do_set_primary_shop(
        who: T::AccountId,
        entity_id: u64,
        shop_id: u64,
    ) -> sp_runtime::DispatchResult {
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(
                entity.owner == who || entity.admins.iter().any(|(a, perm)| a == &who && (perm & AdminPermission::ENTITY_MANAGE) != 0),
                Error::<T>::NotEntityOwner
            );
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            Self::ensure_entity_operable(&entity.status)?;

            // 幂等检查
            ensure!(entity.primary_shop_id != shop_id, Error::<T>::AlreadyPrimaryShop);

            // Shop 必须属于此 Entity
            let shops = EntityShops::<T>::get(entity_id);
            ensure!(shops.contains(&shop_id), Error::<T>::ShopNotInEntity);

            let old_shop_id = entity.primary_shop_id;
            entity.primary_shop_id = shop_id;

            Self::deposit_event(Event::PrimaryShopChanged {
                entity_id,
                old_shop_id,
                new_shop_id: shop_id,
            });

            Ok(())
        })
    }

    /// Owner 主动暂停实体
    pub(crate) fn do_self_pause_entity(who: T::AccountId, entity_id: u64) -> sp_runtime::DispatchResult {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
        ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
        ensure!(entity.status == EntityStatus::Active, Error::<T>::InvalidEntityStatus);
        ensure!(!OwnerPaused::<T>::get(entity_id), Error::<T>::AlreadyOwnerPaused);

        Entities::<T>::mutate(entity_id, |s| {
            if let Some(e) = s {
                e.status = EntityStatus::Suspended;
            }
        });

        EntityStats::<T>::mutate(|stats| {
            stats.active_entities = stats.active_entities.saturating_sub(1);
        });

        OwnerPaused::<T>::insert(entity_id, true);

        T::OnEntityStatusChange::on_entity_suspended(entity_id);

        Self::deposit_event(Event::EntityOwnerPaused { entity_id });
        Ok(())
    }

    /// Owner 恢复主动暂停的实体
    pub(crate) fn do_self_resume_entity(who: T::AccountId, entity_id: u64) -> sp_runtime::DispatchResult {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
        ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
        ensure!(entity.status == EntityStatus::Suspended, Error::<T>::InvalidEntityStatus);
        ensure!(OwnerPaused::<T>::get(entity_id), Error::<T>::NotOwnerPaused);
        // 治理暂停优先，owner 不能绕过治理
        ensure!(!GovernanceSuspended::<T>::get(entity_id), Error::<T>::InvalidEntityStatus);

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

        OwnerPaused::<T>::remove(entity_id);

        T::OnEntityStatusChange::on_entity_resumed(entity_id);

        Self::deposit_event(Event::EntityOwnerResumed { entity_id });
        Ok(())
    }
}
