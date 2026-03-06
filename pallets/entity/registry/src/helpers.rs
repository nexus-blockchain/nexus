//! 辅助函数模块
//!
//! 包含实体注册 pallet 的内部辅助函数、权限检查、Runtime API 查询等。

use crate::pallet::*;
use alloc::vec::Vec;
use codec::Encode;
use frame_support::{
    ensure,
    traits::{Currency, ExistenceRequirement, Get},
    BoundedVec,
};
use pallet_entity_common::{AdminPermission, EntityStatus, EntityType, GovernanceMode, OnEntityStatusChange, PricingProvider};
use pallet_storage_service::{StoragePin, PinTier};
use sp_runtime::{
    traits::{AccountIdConversion, Zero},
    SaturatedConversion,
};

impl<T: Config> Pallet<T> {
    /// 规范化实体名称（小写 ASCII + trim，用于唯一性索引 key）
    pub fn normalize_entity_name(name: &[u8]) -> Result<BoundedVec<u8, T::MaxEntityNameLength>, sp_runtime::DispatchError> {
        let s = core::str::from_utf8(name).map_err(|_| Error::<T>::InvalidName)?;
        let trimmed = s.trim();
        let normalized: Vec<u8> = trimmed.as_bytes().iter().map(|b| b.to_ascii_lowercase()).collect();
        BoundedVec::try_from(normalized).map_err(|_| Error::<T>::NameTooLong.into())
    }

    // ==================== Runtime API 辅助函数 ====================

    /// 查询实体信息（供 Runtime API 使用）
    pub fn api_get_entity(entity_id: u64) -> Option<crate::runtime_api::EntityInfo<T::AccountId, BalanceOf<T>>> {
        let entity = Entities::<T>::get(entity_id)?;
        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);
        let fund_health = Self::get_fund_health(balance);
        Some(crate::runtime_api::EntityInfo {
            id: entity.id,
            owner: entity.owner,
            name: entity.name.into_inner(),
            entity_type: entity.entity_type,
            status: entity.status,
            verified: entity.verified,
            governance_mode: entity.governance_mode,
            primary_shop_id: entity.primary_shop_id,
            fund_balance: balance,
            fund_health: fund_health.encode()[0],
            contact_cid: entity.contact_cid.map(|c| c.into_inner()),
            metadata_uri: entity.metadata_uri.map(|c| c.into_inner()),
            created_at: entity.created_at.saturated_into::<u64>(),
        })
    }

    /// 查询 Owner 的所有 Entity ID（供 Runtime API 使用）
    pub fn api_get_entities_by_owner(account: &T::AccountId) -> Vec<u64> {
        UserEntity::<T>::get(account).into_inner()
    }

    /// 查询实体资金信息（供 Runtime API 使用）
    pub fn api_get_entity_fund_info(entity_id: u64) -> Option<crate::runtime_api::EntityFundInfo<BalanceOf<T>>> {
        if !Entities::<T>::contains_key(entity_id) {
            return None;
        }
        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);
        let health = Self::get_fund_health(balance);
        Some(crate::runtime_api::EntityFundInfo {
            balance,
            health: health.encode()[0],
            min_operating: T::MinOperatingBalance::get(),
            warning_threshold: T::FundWarningThreshold::get(),
        })
    }

    /// 查询已验证的活跃实体列表（供 Runtime API 使用，带分页，按 entity_id 升序）
    pub fn api_get_verified_entities(offset: u32, limit: u32) -> Vec<u64> {
        let max_limit = limit.min(100) as usize;
        let mut ids: Vec<u64> = Entities::<T>::iter()
            .filter(|(_, e)| e.verified && e.status == EntityStatus::Active)
            .map(|(_, e)| e.id)
            .collect();
        ids.sort_unstable();
        ids.into_iter()
            .skip(offset as usize)
            .take(max_limit)
            .collect()
    }

    /// 获取 Entity 金库派生账户
    pub fn entity_treasury_account(entity_id: u64) -> T::AccountId {
        ENTITY_PALLET_ID.into_sub_account_truncating(entity_id)
    }

    /// 计算初始运营资金（USDT 等值 NEX）
    /// 
    /// # 算法
    /// 1. 获取 NEX/USDT 价格（精度 10^6）
    /// 2. 计算所需 NEX 数量 = USDT 金额 * 10^12 / 价格
    /// 3. 限制在 [MinInitialFundCos, MaxInitialFundCos] 范围内
    pub fn calculate_initial_fund() -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
        let price = T::PricingProvider::get_nex_usdt_price();
        ensure!(price > 0, Error::<T>::PriceUnavailable);

        let min_fund = T::MinInitialFundCos::get();
        let max_fund = T::MaxInitialFundCos::get();

        // 价格过时时使用保守兜底值，避免基于过期数据计算押金
        if T::PricingProvider::is_price_stale() {
            return Ok(min_fund);
        }

        let usdt_amount = T::InitialFundUsdt::get();

        // nex_amount = usdt_amount * 10^12 / price
        let nex_amount_u128 = (usdt_amount as u128)
            .checked_mul(1_000_000_000_000u128)
            .ok_or(Error::<T>::ArithmeticOverflow)?
            .checked_div(price as u128)
            .ok_or(Error::<T>::ArithmeticOverflow)?;

        let nex_amount: BalanceOf<T> = nex_amount_u128.saturated_into();

        let final_fund = nex_amount.max(min_fund).min(max_fund);

        Ok(final_fund)
    }

    /// 获取资金健康状态
    pub fn get_fund_health(balance: BalanceOf<T>) -> FundHealth {
        let min_balance = T::MinOperatingBalance::get();
        let warning_threshold = T::FundWarningThreshold::get();

        if balance.is_zero() {
            FundHealth::Depleted
        } else if balance <= min_balance {
            FundHealth::Critical
        } else if balance <= warning_threshold {
            FundHealth::Warning
        } else {
            FundHealth::Healthy
        }
    }

    /// 获取实体金库资金余额
    pub fn get_entity_fund_balance(entity_id: u64) -> BalanceOf<T> {
        let treasury_account = Self::entity_treasury_account(entity_id);
        T::Currency::free_balance(&treasury_account)
    }

    /// 扣除运营费用（供其他模块调用）
    pub fn deduct_operating_fee(
        entity_id: u64,
        fee: BalanceOf<T>,
        fee_type: FeeType,
    ) -> sp_runtime::DispatchResult {
        // L1: 零费用无意义，拒绝以避免误导性事件
        ensure!(!fee.is_zero(), Error::<T>::ZeroAmount);

        // H4+M4 修复: 检查 Entity 状态，仅 Active/Suspended 允许扣费
        let entity = Entities::<T>::get(entity_id).ok_or(Error::<T>::EntityNotFound)?;
        ensure!(
            matches!(entity.status, EntityStatus::Active | EntityStatus::Suspended),
            Error::<T>::EntityNotActive
        );

        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);

        ensure!(balance >= fee, Error::<T>::InsufficientOperatingFund);

        // 运营费用转入平台账户
        T::Currency::transfer(
            &treasury_account,
            &T::PlatformAccount::get(),
            fee,
            ExistenceRequirement::AllowDeath,
        )?;

        let new_balance = T::Currency::free_balance(&treasury_account);
        let min_balance = T::MinOperatingBalance::get();
        let warning_threshold = T::FundWarningThreshold::get();

        // 检查资金健康状态
        if new_balance <= min_balance {
            // 低于最低余额，暂停实体
            // M3 修复: 仅在实际发生状态变更（Active → Suspended）时发射事件
            let mut status_changed = false;
            Entities::<T>::mutate(entity_id, |s| {
                if let Some(entity) = s {
                    if entity.status == EntityStatus::Active {
                        entity.status = EntityStatus::Suspended;
                        status_changed = true;
                        EntityStats::<T>::mutate(|stats| {
                            stats.active_entities = stats.active_entities.saturating_sub(1);
                        });
                    }
                }
            });
            if status_changed {
                T::OnEntityStatusChange::on_entity_suspended(entity_id);
                Self::deposit_event(Event::EntitySuspendedLowFund {
                    entity_id,
                    current_balance: new_balance,
                    minimum_balance: min_balance,
                });
            }
        } else if new_balance <= warning_threshold {
            // 发出预警
            Self::deposit_event(Event::FundWarning {
                entity_id,
                current_balance: new_balance,
                warning_threshold,
            });
        }

        Self::deposit_event(Event::OperatingFeeDeducted {
            entity_id,
            fee,
            fee_type,
            remaining_balance: new_balance,
        });

        Ok(())
    }

    /// 获取当前初始资金金额（供前端查询）
    pub fn get_current_initial_fund() -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
        Self::calculate_initial_fund()
    }

    /// 获取初始资金计算详情（供前端查询）
    pub fn get_initial_fund_details() -> (u64, u64, u128) {
        let price = T::PricingProvider::get_nex_usdt_price();
        let usdt_amount = T::InitialFundUsdt::get();

        let nex_amount = if price > 0 {
            (usdt_amount as u128)
                .saturating_mul(1_000_000_000_000u128)
                .checked_div(price as u128)
                .unwrap_or(0)
        } else {
            0
        };

        (usdt_amount, price, nex_amount)
    }

    // ==================== Phase 3 新增辅助函数 ====================

    /// 检查是否拥有指定权限（owner 天然通过，admin 需权限位匹配）
    pub fn has_permission(entity_id: u64, who: &T::AccountId, required: u32) -> bool {
        Entities::<T>::get(entity_id)
            .map(|entity| {
                if entity.owner == *who {
                    return true;
                }
                entity.admins.iter().any(|(a, perm)| a == who && (perm & required) == required)
            })
            .unwrap_or(false)
    }

    /// 检查是否是 owner 或管理员（任意权限位即通过）
    ///
    /// owner 天然通过。admin 只要在列表中即返回 true，不要求特定权限位。
    /// 如需检查具体权限，请使用 `has_permission(entity_id, who, required)`。
    pub fn is_admin(entity_id: u64, who: &T::AccountId) -> bool {
        Self::has_permission(entity_id, who, 0)
    }

    /// 确保调用者拥有指定权限
    pub fn ensure_permission(entity_id: u64, who: &T::AccountId, required: u32) -> sp_runtime::DispatchResult {
        ensure!(Self::has_permission(entity_id, who, required), Error::<T>::NotAdmin);
        Ok(())
    }

    /// 验证 owner 发起的升级路径（治理 origin 绕过此函数，可任意升级）
    /// - Merchant → 任何类型
    /// - Community → DAO
    /// - Project → DAO / Enterprise
    /// - 其他类型 → 仅治理可操作
    pub fn validate_entity_type_upgrade(
        current: &EntityType,
        new: &EntityType,
    ) -> sp_runtime::DispatchResult {
        // 相同类型不需要升级
        if current == new {
            return Ok(());
        }

        // 允许的升级路径
        let allowed = match current {
            EntityType::Merchant => true, // 商户可升级为任何类型
            EntityType::Community => matches!(new, EntityType::DAO), // 社区可升级为 DAO
            EntityType::Project => matches!(new, EntityType::DAO | EntityType::Enterprise), // 项目可升级为 DAO 或企业
            _ => false, // 其他类型需要治理特殊批准
        };

        ensure!(allowed, Error::<T>::InvalidEntityTypeUpgrade);
        Ok(())
    }

    /// 获取实体类型
    pub fn get_entity_type(entity_id: u64) -> Option<EntityType> {
        Entities::<T>::get(entity_id).map(|e| e.entity_type)
    }

    /// 获取治理模式
    pub fn get_governance_mode(entity_id: u64) -> Option<GovernanceMode> {
        Entities::<T>::get(entity_id).map(|e| e.governance_mode)
    }

    /// 检查实体是否已验证
    pub fn is_verified(entity_id: u64) -> bool {
        Entities::<T>::get(entity_id)
            .map(|e| e.verified)
            .unwrap_or(false)
    }

    /// 获取管理员列表（含权限位掩码）
    pub fn get_admins(entity_id: u64) -> Vec<(T::AccountId, u32)> {
        Entities::<T>::get(entity_id)
            .map(|e| e.admins.into_inner())
            .unwrap_or_default()
    }

    /// 检查账户是否拥有至少一个 Active Entity
    pub fn has_active_entity(account: &T::AccountId) -> bool {
        UserEntity::<T>::get(account)
            .iter()
            .any(|&eid| {
                Entities::<T>::get(eid)
                    .map(|e| e.status == EntityStatus::Active)
                    .unwrap_or(false)
            })
    }

    /// 检查账户是否拥有至少一个非终态 Entity（Active/Suspended/Pending/PendingClose）
    ///
    /// 用于推荐人校验：临时暂停不应取消推荐资格
    pub fn has_operable_entity(account: &T::AccountId) -> bool {
        UserEntity::<T>::get(account)
            .iter()
            .any(|&eid| {
                Entities::<T>::get(eid)
                    .map(|e| !matches!(e.status, EntityStatus::Banned | EntityStatus::Closed))
                    .unwrap_or(false)
            })
    }

    /// 确保实体状态非终态（Banned/Closed 拒绝操作）
    pub(crate) fn ensure_entity_operable(status: &EntityStatus) -> sp_runtime::DispatchResult {
        ensure!(
            !matches!(status, EntityStatus::Banned | EntityStatus::Closed),
            Error::<T>::InvalidEntityStatus
        );
        Ok(())
    }

    // ==================== PendingClose 恢复共用辅助函数 ====================

    /// PendingClose 恢复目标状态判定
    /// 资金充足 + 无治理暂停 + 无 Owner 暂停 → Active，否则 → Suspended
    pub(crate) fn should_restore_to_active(entity_id: u64) -> bool {
        let treasury_account = Self::entity_treasury_account(entity_id);
        let balance = T::Currency::free_balance(&treasury_account);
        balance >= T::MinOperatingBalance::get()
            && !GovernanceSuspended::<T>::get(entity_id)
            && !OwnerPaused::<T>::get(entity_id)
    }

    /// 完成 PendingClose 恢复：清理关闭申请 + 更新活跃统计
    pub(crate) fn finalize_pending_close_restore(entity_id: u64, restored_to_active: bool) {
        EntityCloseRequests::<T>::remove(entity_id);
        if restored_to_active {
            EntityStats::<T>::mutate(|stats| {
                stats.active_entities = stats.active_entities.saturating_add(1);
            });
        }
    }

    // ==================== IPFS Pin/Unpin 辅助函数 ====================

    pub(crate) fn pin_cid(
        caller: &T::AccountId,
        entity_id: u64,
        cid: &BoundedVec<u8, T::MaxCidLength>,
    ) {
        if cid.is_empty() { return; }
        if let Err(e) = T::StoragePin::pin(caller.clone(), b"entity", entity_id, Some(entity_id), cid.to_vec(), cid.len() as u64, PinTier::Standard) {
            log::warn!(
                target: "entity-registry",
                "IPFS Pin failed for entity {}: {:?}", entity_id, e
            );
        }
    }

    pub(crate) fn unpin_cid(caller: &T::AccountId, cid: &BoundedVec<u8, T::MaxCidLength>) {
        if cid.is_empty() { return; }
        if let Err(e) = T::StoragePin::unpin(caller.clone(), cid.to_vec()) {
            log::warn!(
                target: "entity-registry",
                "IPFS Unpin failed: {:?}", e
            );
        }
    }

    pub(crate) fn pin_optional_cid(
        caller: &T::AccountId,
        entity_id: u64,
        cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
    ) {
        if let Some(c) = cid {
            Self::pin_cid(caller, entity_id, c);
        }
    }

    pub(crate) fn unpin_optional_cid(
        caller: &T::AccountId,
        cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
    ) {
        if let Some(c) = cid {
            Self::unpin_cid(caller, c);
        }
    }

    /// ΔPin: Unpin 旧值 + Pin 新值（仅当实际变更时）
    pub(crate) fn update_cid_pin(
        caller: &T::AccountId,
        entity_id: u64,
        old_cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
        new_cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
    ) {
        if old_cid != new_cid {
            Self::unpin_optional_cid(caller, old_cid);
            Self::pin_optional_cid(caller, entity_id, new_cid);
        }
    }

    /// Unpin 实体的所有 CID
    pub(crate) fn unpin_all_entity_cids(entity: &EntityOf<T>) {
        Self::unpin_optional_cid(&entity.owner, &entity.logo_cid);
        Self::unpin_optional_cid(&entity.owner, &entity.description_cid);
        Self::unpin_optional_cid(&entity.owner, &entity.contact_cid);
        Self::unpin_optional_cid(&entity.owner, &entity.metadata_uri);
    }

    // ==================== 补充 Runtime API 辅助函数 ====================

    /// 按名称查找 Entity（normalized 匹配）
    pub fn api_get_entity_by_name(name: Vec<u8>) -> Option<u64> {
        let bounded: BoundedVec<u8, T::MaxEntityNameLength> = name.try_into().ok()?;
        let normalized = Self::normalize_entity_name(&bounded).ok()?;
        EntityNameIndex::<T>::get(&normalized)
    }

    /// 查询实体管理员列表（含权限位掩码）
    pub fn api_get_entity_admins(entity_id: u64) -> Vec<(T::AccountId, u32)> {
        Self::get_admins(entity_id)
    }

    /// 查询实体暂停原因
    pub fn api_get_entity_suspension_reason(entity_id: u64) -> Option<Vec<u8>> {
        SuspensionReasons::<T>::get(entity_id).map(|r| r.into_inner())
    }

    /// 查询实体销售统计
    pub fn api_get_entity_sales(entity_id: u64) -> Option<(BalanceOf<T>, u64)> {
        if !Entities::<T>::contains_key(entity_id) {
            return None;
        }
        let data = EntitySales::<T>::get(entity_id);
        Some((data.total_sales, data.total_orders))
    }

    /// 查询实体推荐人
    pub fn api_get_entity_referrer(entity_id: u64) -> Option<T::AccountId> {
        EntityReferrer::<T>::get(entity_id)
    }

    /// 查询推荐人的所有推荐实体
    pub fn api_get_referrer_entities(account: &T::AccountId) -> Vec<u64> {
        ReferrerEntities::<T>::get(account).into_inner()
    }

    /// 按状态查询实体列表（分页，按 entity_id 升序确保确定性分页）
    pub fn api_get_entities_by_status(status: EntityStatus, offset: u32, limit: u32) -> Vec<u64> {
        let max_limit = limit.min(100) as usize;
        let mut ids: Vec<u64> = Entities::<T>::iter()
            .filter(|(_, e)| e.status == status)
            .map(|(_, e)| e.id)
            .collect();
        ids.sort_unstable();
        ids.into_iter()
            .skip(offset as usize)
            .take(max_limit)
            .collect()
    }

    /// 查询账户在实体中的权限（owner 返回 ALL_DEFINED，admin 返回其权限位，非成员返回 None）
    pub fn api_check_admin_permission(entity_id: u64, account: &T::AccountId) -> Option<u32> {
        let entity = Entities::<T>::get(entity_id)?;
        if entity.owner == *account {
            return Some(AdminPermission::ALL_DEFINED);
        }
        entity.admins.iter()
            .find(|(a, _)| a == account)
            .map(|(_, perm)| *perm)
    }
}
