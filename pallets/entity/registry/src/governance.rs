//! 治理操作模块
//!
//! 包含审批、暂停、恢复、封禁、解禁、认证、撤销认证、拒绝关闭、强制转移所有权等。

use crate::pallet::*;
use alloc::vec::Vec;
use frame_support::{
    ensure,
    traits::{Currency, ExistenceRequirement, Get},
    BoundedVec,
};
use pallet_entity_common::{EntityStatus, OnEntityStatusChange, ShopProvider};
use sp_runtime::traits::{ConstU32, Zero};

impl<T: Config> Pallet<T> {
    // do_approve_entity 已移除：付费即激活，reopen/unban 直接 Active

    /// 暂停实体（治理，可附原因）
    pub(crate) fn do_suspend_entity(entity_id: u64, reason: Option<Vec<u8>>) -> sp_runtime::DispatchResult {
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

        // 标记为治理暂停（防止 top_up_fund 绕过治理自动恢复）
        GovernanceSuspended::<T>::insert(entity_id, true);

        T::OnEntityStatusChange::on_entity_suspended(entity_id);

        // L1: 截断超长原因（而非静默丢弃）
        let bounded_reason: Option<BoundedVec<u8, ConstU32<256>>> = reason
            .filter(|r| !r.is_empty())
            .map(|mut r| { r.truncate(256); r.try_into().unwrap_or_default() });

        // F7: 存储暂停原因（供 owner 和前端查询）
        if let Some(ref r) = bounded_reason {
            SuspensionReasons::<T>::insert(entity_id, r);
            Self::deposit_event(Event::EntitySuspendedWithReason {
                entity_id,
                reason: r.clone(),
            });
        }

        Self::deposit_event(Event::EntityStatusChanged {
            entity_id,
            status: EntityStatus::Suspended,
        });
        Ok(())
    }

    /// 恢复实体（治理，需资金充足）
    pub(crate) fn do_resume_entity(entity_id: u64) -> sp_runtime::DispatchResult {
        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);
        let min_balance = T::MinOperatingBalance::get();
        ensure!(balance >= min_balance, Error::<T>::InsufficientOperatingFund);

        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.status == EntityStatus::Suspended, Error::<T>::InvalidEntityStatus);

        Entities::<T>::mutate(entity_id, |s| {
            if let Some(e) = s {
                e.status = EntityStatus::Active;
            }
        });

        EntityStats::<T>::mutate(|stats| {
            stats.active_entities = stats.active_entities.saturating_add(1);
        });

        // 清除治理暂停标记
        GovernanceSuspended::<T>::remove(entity_id);
        // M2: 清除 OwnerPaused 标记（治理恢复覆盖 owner 暂停）
        OwnerPaused::<T>::remove(entity_id);
        // F7: 清除暂停原因
        SuspensionReasons::<T>::remove(entity_id);

        T::OnEntityStatusChange::on_entity_resumed(entity_id);

        Self::deposit_event(Event::EntityStatusChanged {
            entity_id,
            status: EntityStatus::Active,
        });
        Ok(())
    }

    /// 封禁实体（治理，可选没收资金，可附原因）
    pub(crate) fn do_ban_entity(
        entity_id: u64,
        confiscate_fund: bool,
        reason: Option<Vec<u8>>,
    ) -> sp_runtime::DispatchResult {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        // 仅允许 ban Active/Suspended/PendingClose 实体
        ensure!(
            matches!(entity.status, EntityStatus::Active | EntityStatus::Suspended | EntityStatus::PendingClose),
            Error::<T>::InvalidEntityStatus
        );

        // IPFS Unpin: 封禁时释放所有实体 CID
        Self::unpin_all_entity_cids(&entity);

        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);

        // M1 审计修复: 跟踪没收是否实际成功，而非仅报告请求意图
        let mut fund_actually_confiscated = false;
        if !balance.is_zero() {
            if confiscate_fund {
                // H1: 没收资金转入平台账户，仅成功时发射事件
                if T::Currency::transfer(
                    &treasury_account,
                    &T::PlatformAccount::get(),
                    balance,
                    ExistenceRequirement::AllowDeath,
                ).is_ok() {
                    Self::deposit_event(Event::FundConfiscated { entity_id, amount: balance });
                    fund_actually_confiscated = true;
                } else {
                    // M1 审计修复: 没收失败时发射事件（与 refund 路径一致）
                    Self::deposit_event(Event::FundRefundFailed { entity_id, amount: balance });
                }
            } else {
                // H2 修复: 退还失败不应阻止封禁（owner 账户可能已被 reaped）
                if T::Currency::transfer(
                    &treasury_account,
                    &entity.owner,
                    balance,
                    ExistenceRequirement::AllowDeath,
                ).is_err() {
                    Self::deposit_event(Event::FundRefundFailed { entity_id, amount: balance });
                }
            }
        }

        Entities::<T>::mutate(entity_id, |maybe| {
            if let Some(e) = maybe {
                e.status = EntityStatus::Banned;
            }
        });

        // 清理用户实体索引
        UserEntity::<T>::mutate(&entity.owner, |entities| {
            entities.retain(|&id| id != entity_id);
        });

        // 清理 PendingClose 关闭申请（若有）
        EntityCloseRequests::<T>::remove(entity_id);

        // H3 修复: 清除治理暂停标记（防止残留影响后续 reopen 流程）
        GovernanceSuspended::<T>::remove(entity_id);
        // M2: 清除 OwnerPaused 标记
        OwnerPaused::<T>::remove(entity_id);
        // F7: 清除暂停原因
        SuspensionReasons::<T>::remove(entity_id);
        // F1: 清理名称唯一性索引
        if let Ok(normalized) = Self::normalize_entity_name(&entity.name) {
            EntityNameIndex::<T>::remove(&normalized);
        }

        // M2-R12: 清理销售数据（banned 实体不应保留统计）
        EntitySales::<T>::remove(entity_id);

        // M2 审计修复: 清理推荐关系（释放推荐人的 MaxReferralsPerReferrer 配额）
        if let Some(referrer) = EntityReferrer::<T>::take(entity_id) {
            ReferrerEntities::<T>::mutate(&referrer, |entities| {
                entities.retain(|&id| id != entity_id);
            });
        }

        if entity.status == EntityStatus::Active {
            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_sub(1);
            });
        }

        // 级联关闭所有 Shop（绕过 is_primary 保护）
        // 注意: 保留 EntityShops 关联列表，以便 unban_entity 恢复时可 resume_shop
        for sid in EntityShops::<T>::get(entity_id).iter() {
            if T::ShopProvider::force_close_shop(*sid).is_err() {
                Self::deposit_event(Event::ShopCascadeFailed { entity_id, shop_id: *sid });
            }
        }

        T::OnEntityStatusChange::on_entity_banned(entity_id);

        // b6: 绑定原因到事件（L1: 截断超长原因而非静默丢弃）
        let bounded_reason: Option<BoundedVec<u8, ConstU32<256>>> = reason
            .filter(|r| !r.is_empty())
            .map(|mut r| { r.truncate(256); r.try_into().unwrap_or_default() });

        // M1 审计修复: fund_confiscated 报告实际结果而非请求意图
        Self::deposit_event(Event::EntityBanned {
            entity_id,
            fund_confiscated: fund_actually_confiscated,
            reason: bounded_reason,
        });
        Ok(())
    }

    /// 解除封禁（治理，Banned → Active，需资金充足，直接激活）
    pub(crate) fn do_unban_entity(entity_id: u64) -> sp_runtime::DispatchResult {
        let owner = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<T::AccountId, sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::Banned, Error::<T>::InvalidEntityStatus);

            // 名称索引恢复（先检查名称可用性，再检查资金）
            let normalized = Self::normalize_entity_name(&entity.name)?;
            ensure!(
                !EntityNameIndex::<T>::contains_key(&normalized),
                Error::<T>::NameAlreadyTaken
            );

            // 资金校验：ban 时可能已没收，需 owner 先 top_up_fund
            let treasury_account = Self::entity_treasury_account(entity_id);
            let balance = T::Currency::free_balance(&treasury_account);
            ensure!(balance >= T::MinOperatingBalance::get(), Error::<T>::InsufficientOperatingFund);

            EntityNameIndex::<T>::insert(&normalized, entity_id);

            // 直接激活（不再经过 Pending → approve 流程）
            entity.status = EntityStatus::Active;
            Ok(entity.owner.clone())
        })?;

        // 恢复 UserEntity 索引（ban_entity 时已移除）
        UserEntity::<T>::try_mutate(&owner, |entities| -> sp_runtime::DispatchResult {
            if !entities.contains(&entity_id) {
                entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)?;
            }
            Ok(())
        })?;

        // 清除遗留标记
        GovernanceSuspended::<T>::remove(entity_id);
        OwnerPaused::<T>::remove(entity_id);
        SuspensionReasons::<T>::remove(entity_id);

        // 更新统计
        EntityStats::<T>::mutate(|stats| {
            stats.active_entities = stats.active_entities.saturating_add(1);
        });

        // 恢复关联 Shop
        for sid in EntityShops::<T>::get(entity_id).iter() {
            if T::ShopProvider::resume_shop(*sid).is_err() {
                Self::deposit_event(Event::ShopCascadeFailed { entity_id, shop_id: *sid });
            }
        }

        T::OnEntityStatusChange::on_entity_resumed(entity_id);

        Self::deposit_event(Event::EntityUnbanned { entity_id });
        Self::deposit_event(Event::EntityStatusChanged {
            entity_id,
            status: EntityStatus::Active,
        });
        Ok(())
    }

    /// 验证实体（治理）
    pub(crate) fn do_verify_entity(entity_id: u64) -> sp_runtime::DispatchResult {
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            Self::ensure_entity_operable(&entity.status)?;
            ensure!(!entity.verified, Error::<T>::AlreadyVerified);
            entity.verified = true;
            Ok(())
        })?;

        Self::deposit_event(Event::EntityVerified { entity_id });
        Ok(())
    }

    /// 撤销认证（治理）
    pub(crate) fn do_unverify_entity(entity_id: u64) -> sp_runtime::DispatchResult {
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            Self::ensure_entity_operable(&entity.status)?;
            ensure!(entity.verified, Error::<T>::NotVerified);
            entity.verified = false;
            Ok(())
        })?;

        Self::deposit_event(Event::EntityUnverified { entity_id });
        Ok(())
    }

    /// 治理强制转移所有权
    ///
    /// 设计决策：不限制 Entity 状态。治理可对任何状态的 Entity 强制转移所有权，
    /// 包括 Banned（修正 owner 后再 unban）和 Closed（修正 owner 后再 reopen）。
    pub(crate) fn do_force_transfer_ownership(
        entity_id: u64,
        new_owner: T::AccountId,
    ) -> sp_runtime::DispatchResult {
        let new_owner_entities = UserEntity::<T>::get(&new_owner);
        ensure!(
            (new_owner_entities.len() as u32) < T::MaxEntitiesPerUser::get(),
            Error::<T>::MaxEntitiesReached
        );

        let old_owner = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<T::AccountId, sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner != new_owner, Error::<T>::SameOwner);

            let old = entity.owner.clone();
            entity.owner = new_owner.clone();

            // 如果新 owner 在管理员列表中，移除
            if let Some(pos) = entity.admins.iter().position(|(a, _)| a == &new_owner) {
                entity.admins.remove(pos);
            }

            Ok(old)
        })?;

        // 更新用户实体索引
        UserEntity::<T>::mutate(&old_owner, |entities| {
            entities.retain(|&id| id != entity_id);
        });
        UserEntity::<T>::try_mutate(&new_owner, |entities| {
            entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)
        })?;

        Self::deposit_event(Event::OwnershipForceTransferred {
            entity_id,
            old_owner,
            new_owner,
        });
        Ok(())
    }

    /// 治理拒绝关闭申请（PendingClose → Active/Suspended）
    pub(crate) fn do_reject_close_request(entity_id: u64) -> sp_runtime::DispatchResult {
        // L3 审计修复: 使用共用 helper 消除重复逻辑
        let restore_to_active = Self::should_restore_to_active(entity_id);

        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.status == EntityStatus::PendingClose, Error::<T>::InvalidEntityStatus);

            entity.status = if restore_to_active { EntityStatus::Active } else { EntityStatus::Suspended };
            Ok(())
        })?;

        Self::finalize_pending_close_restore(entity_id, restore_to_active);

        Self::deposit_event(Event::CloseRequestRejected { entity_id });
        Ok(())
    }
}
