//! 管理员操作模块
//!
//! 包含添加/移除/更新管理员权限、转移所有权、管理员辞职等。

use crate::pallet::*;
use frame_support::{
    ensure,
    traits::Get,
};
use pallet_entity_common::{AdminPermission, EntityStatus, GovernanceProvider};

impl<T: Config> Pallet<T> {
    /// 添加管理员（指定权限位掩码）
    pub(crate) fn do_add_admin(
        who: T::AccountId,
        entity_id: u64,
        new_admin: T::AccountId,
        permissions: u32,
    ) -> sp_runtime::DispatchResult {
        ensure!(permissions != 0, Error::<T>::InvalidPermissions);
        ensure!(AdminPermission::is_valid(permissions), Error::<T>::InvalidPermissions);
        
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            
            // 只有所有者可以添加管理员
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            Self::ensure_entity_operable(&entity.status)?;
            
            // 检查是否已是管理员
            ensure!(!entity.admins.iter().any(|(a, _)| a == &new_admin), Error::<T>::AdminAlreadyExists);
            ensure!(new_admin != entity.owner, Error::<T>::AdminAlreadyExists);
            
            // 添加管理员
            entity.admins.try_push((new_admin.clone(), permissions))
                .map_err(|_| Error::<T>::MaxAdminsReached)?;
            
            Ok(())
        })?;

        Self::deposit_event(Event::AdminAdded {
            entity_id,
            admin: new_admin,
            permissions,
        });
        Ok(())
    }

    /// 移除管理员
    pub(crate) fn do_remove_admin(
        who: T::AccountId,
        entity_id: u64,
        admin: T::AccountId,
    ) -> sp_runtime::DispatchResult {
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            
            // 只有所有者可以移除管理员
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            Self::ensure_entity_operable(&entity.status)?;
            
            // 不能移除所有者
            ensure!(admin != entity.owner, Error::<T>::CannotRemoveOwner);
            
            // 查找并移除
            let pos = entity.admins.iter().position(|(a, _)| a == &admin)
                .ok_or(Error::<T>::AdminNotFound)?;
            entity.admins.remove(pos);
            
            Ok(())
        })?;

        Self::deposit_event(Event::AdminRemoved {
            entity_id,
            admin,
        });
        Ok(())
    }

    /// 更新管理员权限
    pub(crate) fn do_update_admin_permissions(
        who: T::AccountId,
        entity_id: u64,
        admin: T::AccountId,
        new_permissions: u32,
    ) -> sp_runtime::DispatchResult {
        ensure!(new_permissions != 0, Error::<T>::InvalidPermissions);
        ensure!(AdminPermission::is_valid(new_permissions), Error::<T>::InvalidPermissions);

        let old_permissions = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<u32, sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            Self::ensure_entity_operable(&entity.status)?;

            let entry = entity.admins.iter_mut()
                .find(|(a, _)| a == &admin)
                .ok_or(Error::<T>::AdminNotFound)?;
            let old = entry.1;
            entry.1 = new_permissions;
            Ok(old)
        })?;

        Self::deposit_event(Event::AdminPermissionsUpdated {
            entity_id,
            admin,
            old_permissions,
            new_permissions,
        });
        Ok(())
    }

    /// 转移所有权
    pub(crate) fn do_transfer_ownership(
        who: T::AccountId,
        entity_id: u64,
        new_owner: T::AccountId,
    ) -> sp_runtime::DispatchResult {
        // M1 审计修复: 禁止自我转移（避免无意义存储写入和误导性事件）
        ensure!(new_owner != who, Error::<T>::SameOwner);

        // H2 修复: 先验证新 owner 容量，避免 Entity 变更后 UserEntity 写入失败导致不一致
        let new_owner_entities = UserEntity::<T>::get(&new_owner);
        ensure!(
            (new_owner_entities.len() as u32) < T::MaxEntitiesPerUser::get(),
            Error::<T>::MaxEntitiesReached
        );
        
        let old_owner = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<T::AccountId, sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            
            // 只有所有者可以转移所有权
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            // H3+L2: 不允许对 Banned/Closed/PendingClose 实体转移所有权
            // PendingClose: 转移后 approve_close 退款给新 owner 而非关闭申请人
            ensure!(
                !matches!(entity.status, EntityStatus::Banned | EntityStatus::Closed | EntityStatus::PendingClose),
                Error::<T>::InvalidEntityStatus
            );
            
            let old = entity.owner.clone();
            entity.owner = new_owner.clone();
            
            // 如果新所有者在管理员列表中，移除
            if let Some(pos) = entity.admins.iter().position(|(a, _)| a == &new_owner) {
                entity.admins.remove(pos);
            }
            
            Ok(old)
        })?;

        // 更新用户实体索引（容量已预先验证，try_push 不会失败）
        UserEntity::<T>::mutate(&old_owner, |entities| {
            entities.retain(|&id| id != entity_id);
        });
        UserEntity::<T>::try_mutate(&new_owner, |entities| {
            entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)
        })?;

        Self::deposit_event(Event::OwnershipTransferred {
            entity_id,
            old_owner,
            new_owner,
        });
        Ok(())
    }

    /// 管理员主动辞职
    pub(crate) fn do_resign_admin(who: T::AccountId, entity_id: u64) -> sp_runtime::DispatchResult {
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;

            // Owner 不可辞职
            ensure!(entity.owner != who, Error::<T>::CannotRemoveOwner);
            // M1-R12: 与其他 admin 操作保持一致，检查治理锁
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);

            // 查找并移除
            let pos = entity.admins.iter().position(|(a, _)| a == &who)
                .ok_or(Error::<T>::NotAdminCaller)?;
            entity.admins.remove(pos);

            Ok(())
        })?;

        Self::deposit_event(Event::AdminResigned {
            entity_id,
            admin: who,
        });
        Ok(())
    }
}
