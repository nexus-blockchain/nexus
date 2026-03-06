//! 实体生命周期管理
//!
//! 包含创建、更新、关闭申请、审批关闭、重开、超时关闭等操作。

use crate::pallet::*;
use alloc::vec::Vec;
use frame_support::{
    ensure,
    traits::{Currency, ExistenceRequirement, Get},
    BoundedVec,
};
use pallet_entity_common::{EntityStatus, EntityType, GovernanceMode, GovernanceProvider, OnEntityStatusChange, ShopProvider, ShopType};
use sp_runtime::traits::{Saturating, Zero};

impl<T: Config> Pallet<T> {
    /// 创建 Entity（组织身份）— 付费即激活
    pub(crate) fn do_create_entity(
        who: T::AccountId,
        name: Vec<u8>,
        logo_cid: Option<Vec<u8>>,
        description_cid: Option<Vec<u8>>,
        referrer: Option<T::AccountId>,
    ) -> sp_runtime::DispatchResult {
        // 检查用户实体数量是否达到上限
        let user_entities = UserEntity::<T>::get(&who);
        ensure!(
            (user_entities.len() as u32) < T::MaxEntitiesPerUser::get(),
            Error::<T>::MaxEntitiesReached
        );

        ensure!(!name.is_empty(), Error::<T>::NameEmpty);
        // H2: 名称内容校验（UTF-8 + 无控制字符 + 非全空白）
        let name_str = core::str::from_utf8(&name).map_err(|_| Error::<T>::InvalidName)?;
        ensure!(!name_str.trim().is_empty(), Error::<T>::NameEmpty);
        ensure!(!name_str.chars().any(|c| c.is_control()), Error::<T>::InvalidName);
        let name: BoundedVec<u8, T::MaxEntityNameLength> =
            name.try_into().map_err(|_| Error::<T>::NameTooLong)?;

        // F1: 名称唯一性校验
        let normalized_name = Self::normalize_entity_name(&name)?;
        ensure!(
            !EntityNameIndex::<T>::contains_key(&normalized_name),
            Error::<T>::NameAlreadyTaken
        );

        // H1: 空 CID 转为 None
        let logo_cid: Option<BoundedVec<u8, T::MaxCidLength>> = logo_cid
            .filter(|c| !c.is_empty())
            .map(|c| c.try_into().map_err(|_| Error::<T>::CidTooLong))
            .transpose()?;
        let description_cid: Option<BoundedVec<u8, T::MaxCidLength>> = description_cid
            .filter(|c| !c.is_empty())
            .map(|c| c.try_into().map_err(|_| Error::<T>::CidTooLong))
            .transpose()?;

        // 验证推荐人（允许 Suspended/Pending 等非终态实体的 owner 作为推荐人）
        if let Some(ref ref_account) = referrer {
            ensure!(ref_account != &who, Error::<T>::SelfReferral);
            ensure!(Self::has_operable_entity(ref_account), Error::<T>::InvalidReferrer);
            // M1 审计修复: 预检推荐人容量（避免创建后 try_push 失败导致浪费 gas）
            let current = ReferrerEntities::<T>::get(ref_account);
            ensure!(
                (current.len() as u32) < T::MaxReferralsPerReferrer::get(),
                Error::<T>::ReferrerIndexFull
            );
        }

        // 计算初始金库资金（50 USDT 等值 NEX）
        let initial_fund = Self::calculate_initial_fund()?;
        
        // H1: Entity ID 溢出保护（与 pallet-entity-shop H5 同类问题）
        let entity_id = NextEntityId::<T>::get();
        ensure!(entity_id < u64::MAX, Error::<T>::ArithmeticOverflow);
        let treasury_account = Self::entity_treasury_account(entity_id);
        
        // 转入金库账户
        T::Currency::transfer(
            &who,
            &treasury_account,
            initial_fund,
            ExistenceRequirement::KeepAlive,
        ).map_err(|_| Error::<T>::InsufficientBalanceForInitialFund)?;

        let now = <frame_system::Pallet<T>>::block_number();

        // 创建 Entity（Shop 列表初始为空，用户按需创建）
        let entity = Entity {
            id: entity_id,
            owner: who.clone(),
            name,
            logo_cid,
            description_cid,
            status: EntityStatus::Active,
            created_at: now,
            entity_type: EntityType::Merchant,
            admins: BoundedVec::default(),
            governance_mode: GovernanceMode::None,
            verified: false,
            metadata_uri: None,
            contact_cid: None,
            primary_shop_id: 0,
        };

        Entities::<T>::insert(entity_id, &entity);
        // F1: 插入名称索引
        EntityNameIndex::<T>::insert(&normalized_name, entity_id);
        UserEntity::<T>::try_mutate(&who, |entities| {
            entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)
        })?;
        NextEntityId::<T>::put(entity_id.saturating_add(1));

        // 自动创建 Primary Shop（继承 Entity 名称，默认线上商城）
        let primary_shop_id = T::ShopProvider::create_primary_shop(
            entity_id,
            entity.name.to_vec(),
            ShopType::OnlineStore,
        )?;

        // M3 审计修复: 防御性设置 primary_shop_id
        // 正常路径由 ShopProvider → register_shop 回调设置；此处兜底未回调的 ShopProvider 实现
        if primary_shop_id > 0 {
            Entities::<T>::mutate(entity_id, |e| {
                if let Some(e) = e {
                    if e.primary_shop_id == 0 && EntityShops::<T>::get(entity_id).contains(&primary_shop_id) {
                        e.primary_shop_id = primary_shop_id;
                    }
                }
            });
        }

        // 更新统计（付费即激活，无需审批）
        EntityStats::<T>::mutate(|stats| {
            stats.total_entities = stats.total_entities.saturating_add(1);
            stats.active_entities = stats.active_entities.saturating_add(1);
        });

        // 记录推荐人（如果有）
        if let Some(ref ref_account) = referrer {
            EntityReferrer::<T>::insert(entity_id, ref_account);
            // b8: 维护反向索引
            ReferrerEntities::<T>::try_mutate(ref_account, |entities| -> Result<(), sp_runtime::DispatchError> {
                entities.try_push(entity_id).map_err(|_| Error::<T>::ReferrerIndexFull)?;
                Ok(())
            })?;
            Self::deposit_event(Event::EntityReferrerBound {
                entity_id,
                referrer: ref_account.clone(),
            });
        }

        // IPFS Pin: best-effort pin 实体元数据 CID
        Self::pin_optional_cid(&entity.owner, entity_id, &entity.logo_cid);
        Self::pin_optional_cid(&entity.owner, entity_id, &entity.description_cid);

        Self::deposit_event(Event::EntityCreated {
            entity_id,
            owner: who,
            treasury_account,
            initial_fund,
        });

        Ok(())
    }

    /// 更新实体信息
    pub(crate) fn do_update_entity(
        who: T::AccountId,
        entity_id: u64,
        name: Option<Vec<u8>>,
        logo_cid: Option<Vec<u8>>,
        description_cid: Option<Vec<u8>>,
        metadata_uri: Option<Vec<u8>>,
        contact_cid: Option<Vec<u8>>,
    ) -> sp_runtime::DispatchResult {
        Entities::<T>::try_mutate(entity_id, |maybe_entity| -> sp_runtime::DispatchResult {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            // b2: owner 或拥有 ENTITY_MANAGE 权限的 admin 均可更新
            ensure!(
                entity.owner == who || entity.admins.iter().any(|(a, perm)| a == &who && (perm & pallet_entity_common::AdminPermission::ENTITY_MANAGE) != 0),
                Error::<T>::NotEntityOwner
            );
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            Self::ensure_entity_operable(&entity.status)?;

            // 捕获旧 CID 用于 ΔPin
            let old_logo = entity.logo_cid.clone();
            let old_desc = entity.description_cid.clone();
            let old_meta = entity.metadata_uri.clone();
            let old_contact = entity.contact_cid.clone();

            // H2: 名称更新时校验内容
            if let Some(n) = name {
                ensure!(!n.is_empty(), Error::<T>::NameEmpty);
                let n_str = core::str::from_utf8(&n).map_err(|_| Error::<T>::InvalidName)?;
                ensure!(!n_str.trim().is_empty(), Error::<T>::NameEmpty);
                ensure!(!n_str.chars().any(|c| c.is_control()), Error::<T>::InvalidName);
                let new_name: BoundedVec<u8, T::MaxEntityNameLength> = n.try_into().map_err(|_| Error::<T>::NameTooLong)?;
                // F1: 名称唯一性校验
                let new_normalized = Self::normalize_entity_name(&new_name)?;
                let old_normalized = Self::normalize_entity_name(&entity.name)?;
                if new_normalized != old_normalized {
                    ensure!(
                        !EntityNameIndex::<T>::contains_key(&new_normalized),
                        Error::<T>::NameAlreadyTaken
                    );
                    // M1-R13: 仅当索引条目属于当前实体时才移除，
                    // 防止 ban→unban 后旧名称被其他实体占用时误删他人索引
                    if EntityNameIndex::<T>::get(&old_normalized) == Some(entity_id) {
                        EntityNameIndex::<T>::remove(&old_normalized);
                    }
                    EntityNameIndex::<T>::insert(&new_normalized, entity_id);
                }
                entity.name = new_name;
            }
            // H1: 空 CID 转为 None
            if let Some(c) = logo_cid {
                entity.logo_cid = if c.is_empty() { None } else { Some(c.try_into().map_err(|_| Error::<T>::CidTooLong)?) };
            }
            if let Some(c) = description_cid {
                entity.description_cid = if c.is_empty() { None } else { Some(c.try_into().map_err(|_| Error::<T>::CidTooLong)?) };
            }
            // M2: 支持更新 metadata_uri（M1: 空值转 None，与 logo/desc 一致）
            if let Some(uri) = metadata_uri {
                entity.metadata_uri = if uri.is_empty() { None } else { Some(uri.try_into().map_err(|_| Error::<T>::CidTooLong)?) };
            }
            // F4: 联系方式 CID
            if let Some(c) = contact_cid {
                entity.contact_cid = if c.is_empty() { None } else { Some(c.try_into().map_err(|_| Error::<T>::CidTooLong)?) };
            }

            // IPFS ΔPin: Unpin 旧 CID + Pin 新 CID（仅变更时操作）
            let owner = entity.owner.clone();
            Self::update_cid_pin(&owner, entity_id, &old_logo, &entity.logo_cid);
            Self::update_cid_pin(&owner, entity_id, &old_desc, &entity.description_cid);
            Self::update_cid_pin(&owner, entity_id, &old_meta, &entity.metadata_uri);
            Self::update_cid_pin(&owner, entity_id, &old_contact, &entity.contact_cid);

            Ok(())
        })?;

        Self::deposit_event(Event::EntityUpdated { entity_id });
        Ok(())
    }

    /// 申请关闭实体（需治理审批）
    pub(crate) fn do_request_close_entity(who: T::AccountId, entity_id: u64) -> sp_runtime::DispatchResult {
        let was_active = Entities::<T>::try_mutate(entity_id, |maybe_entity| -> Result<bool, sp_runtime::DispatchError> {
            let entity = maybe_entity.as_mut().ok_or(Error::<T>::EntityNotFound)?;
            ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
            ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
            ensure!(
                entity.status == EntityStatus::Active || entity.status == EntityStatus::Suspended,
                Error::<T>::InvalidEntityStatus
            );

            let was_active = entity.status == EntityStatus::Active;
            entity.status = EntityStatus::PendingClose;
            Ok(was_active)
        })?;

        // 记录申请时间
        let now = <frame_system::Pallet<T>>::block_number();
        EntityCloseRequests::<T>::insert(entity_id, now);

        // 修正统计：Active → PendingClose 时递减
        if was_active {
            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_sub(1);
            });
        }

        Self::deposit_event(Event::EntityCloseRequested { entity_id });
        Ok(())
    }

    /// 审批关闭实体（治理，退还全部余额）
    pub(crate) fn do_approve_close_entity(entity_id: u64) -> sp_runtime::DispatchResult {
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.status == EntityStatus::PendingClose, Error::<T>::InvalidEntityStatus);

        let fund_refunded = Self::do_finalize_close(entity_id, entity);

        Self::deposit_event(Event::EntityClosed { entity_id, fund_refunded });
        Ok(())
    }

    /// 关闭实体的核心逻辑（审批关闭 / 超时关闭共用）
    ///
    /// 前置条件：调用方已验证实体状态为 PendingClose（及超时条件）。
    /// 返回实际退还的资金金额。
    fn do_finalize_close(entity_id: u64, entity: EntityOf<T>) -> BalanceOf<T> {
        Self::unpin_all_entity_cids(&entity);

        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);

        let fund_refunded = if !balance.is_zero() {
            match T::Currency::transfer(
                &treasury_account,
                &entity.owner,
                balance,
                ExistenceRequirement::AllowDeath,
            ) {
                Ok(_) => balance,
                Err(_) => {
                    Self::deposit_event(Event::FundRefundFailed { entity_id, amount: balance });
                    Zero::zero()
                },
            }
        } else {
            Zero::zero()
        };

        Entities::<T>::mutate(entity_id, |s| {
            if let Some(e) = s {
                e.status = EntityStatus::Closed;
            }
        });

        EntityCloseRequests::<T>::remove(entity_id);
        GovernanceSuspended::<T>::remove(entity_id);
        OwnerPaused::<T>::remove(entity_id);
        SuspensionReasons::<T>::remove(entity_id);

        if let Ok(normalized) = Self::normalize_entity_name(&entity.name) {
            EntityNameIndex::<T>::remove(&normalized);
        }

        UserEntity::<T>::mutate(&entity.owner, |entities| {
            entities.retain(|&id| id != entity_id);
        });

        for sid in EntityShops::<T>::get(entity_id).iter() {
            if T::ShopProvider::force_close_shop(*sid).is_err() {
                Self::deposit_event(Event::ShopCascadeFailed { entity_id, shop_id: *sid });
            }
        }
        EntityShops::<T>::remove(entity_id);
        EntitySales::<T>::remove(entity_id);

        // M2 审计修复: 清理推荐关系（释放推荐人的 MaxReferralsPerReferrer 配额）
        if let Some(referrer) = EntityReferrer::<T>::take(entity_id) {
            ReferrerEntities::<T>::mutate(&referrer, |entities| {
                entities.retain(|&id| id != entity_id);
            });
        }

        T::OnEntityStatusChange::on_entity_closed(entity_id);

        fund_refunded
    }

    /// 重新开业（owner 申请，Closed → Pending，需重新缴纳押金，等待治理审批）
    pub(crate) fn do_reopen_entity(who: T::AccountId, entity_id: u64) -> sp_runtime::DispatchResult {
        // 验证实体存在且处于 Closed 状态
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.owner == who, Error::<T>::NotEntityOwner);
        ensure!(!T::GovernanceProvider::is_governance_locked(entity_id), Error::<T>::EntityLocked);
        ensure!(entity.status == EntityStatus::Closed, Error::<T>::InvalidEntityStatus);

        // H1 审计修复: 预占名称索引（防止 Pending 期间名称被抢注）
        let normalized_name = Self::normalize_entity_name(&entity.name)?;
        ensure!(
            !EntityNameIndex::<T>::contains_key(&normalized_name),
            Error::<T>::NameAlreadyTaken
        );

        // M2 审计修复: 预检查用户容量，避免已满时白付 gas
        let user_entities = UserEntity::<T>::get(&who);
        if !user_entities.contains(&entity_id) {
            ensure!(
                (user_entities.len() as u32) < T::MaxEntitiesPerUser::get(),
                Error::<T>::MaxEntitiesReached
            );
        }

        // 重新计算并缴纳押金（50 USDT 等值 NEX）
        let initial_fund_amount = Self::calculate_initial_fund()?;

        let treasury_account = Self::entity_treasury_account(entity_id);
        T::Currency::transfer(
            &who,
            &treasury_account,
            initial_fund_amount,
            ExistenceRequirement::KeepAlive,
        )?;

        // 更新状态为 Pending，等待治理审批
        Entities::<T>::mutate(entity_id, |s| {
            if let Some(e) = s {
                e.status = EntityStatus::Pending;
            }
        });

        // H1 审计修复: 写入名称索引预占
        EntityNameIndex::<T>::insert(&normalized_name, entity_id);

        // M2: 清除 OwnerPaused 标记（新生命周期干净起步）
        OwnerPaused::<T>::remove(entity_id);

        // 恢复 UserEntity 索引（防御性去重：避免异常路径导致重复索引）
        UserEntity::<T>::try_mutate(&who, |entities| -> sp_runtime::DispatchResult {
            if !entities.contains(&entity_id) {
                entities.try_push(entity_id).map_err(|_| Error::<T>::MaxEntitiesReached)?;
            }
            Ok(())
        })?;

        Self::deposit_event(Event::EntityReopened {
            entity_id,
            owner: who,
            initial_fund: initial_fund_amount,
        });

        Ok(())
    }

    /// 执行超时关闭申请（任何人可调用）
    pub(crate) fn do_execute_close_timeout(entity_id: u64) -> sp_runtime::DispatchResult {
        let request_at = EntityCloseRequests::<T>::get(entity_id)
            .ok_or(Error::<T>::InvalidEntityStatus)?;
        let now = <frame_system::Pallet<T>>::block_number();
        let timeout = T::CloseRequestTimeout::get();
        ensure!(now >= request_at.saturating_add(timeout), Error::<T>::CloseRequestNotExpired);

        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(entity.status == EntityStatus::PendingClose, Error::<T>::InvalidEntityStatus);

        let fund_refunded = Self::do_finalize_close(entity_id, entity);

        Self::deposit_event(Event::CloseRequestAutoExecuted { entity_id, fund_refunded });
        Ok(())
    }
}
