//! 自定义等级系统管理、等级计算、有效等级查询、治理调用

use crate::pallet::*;
use frame_support::pallet_prelude::*;
use pallet_entity_common::{
    EntityProvider, MemberRegistrationPolicy, OnLevelRemoved, OnMemberLevelChanged,
};

impl<T: Config> Pallet<T> {
    /// 计算自定义等级（根据 USDT 消费金额）
    pub fn calculate_custom_level(shop_id: u64, total_spent: u64) -> u8 {
        let entity_id = match Self::resolve_entity_id(shop_id) {
            Some(id) => id,
            None => return 0,
        };
        Self::calculate_custom_level_by_entity(entity_id, total_spent)
    }

    /// 计算自定义等级（entity_id 直达，避免重复解析）
    ///
    /// 仅根据 USDT 消费阈值判断。推荐/团队门槛由 `calculate_custom_level_full` 处理。
    pub fn calculate_custom_level_by_entity(entity_id: u64, total_spent: u64) -> u8 {
        let system = match EntityLevelSystems::<T>::get(entity_id) {
            Some(s) if s.use_custom && !s.levels.is_empty() => s,
            _ => return 0,
        };

        let mut current_level = 0u8;
        for level in system.levels.iter() {
            if total_spent >= level.threshold {
                current_level = level.id;
            } else {
                break;
            }
        }
        current_level
    }

    /// 计算自定义等级（完整版：消费阈值 + 推荐/团队门槛，AND 逻辑）
    ///
    /// 等级严格递进：所有非零门槛均需满足才能达到该等级，
    /// 不满足当前等级条件则不会继续评估更高等级。
    pub fn calculate_custom_level_full(
        entity_id: u64,
        total_spent: u64,
        direct_referrals: u32,
        team_size: u32,
        indirect_referrals: u32,
    ) -> u8 {
        let system = match EntityLevelSystems::<T>::get(entity_id) {
            Some(s) if s.use_custom && !s.levels.is_empty() => s,
            _ => return 0,
        };

        let mut current_level = 0u8;
        for level in system.levels.iter() {
            if total_spent >= level.threshold
                && (level.min_direct_referrals == 0
                    || direct_referrals >= level.min_direct_referrals)
                && (level.min_team_size == 0 || team_size >= level.min_team_size)
                && (level.min_indirect_referrals == 0
                    || indirect_referrals >= level.min_indirect_referrals)
            {
                current_level = level.id;
            } else {
                break;
            }
        }
        current_level
    }

    /// 获取等级信息
    pub fn get_custom_level_info(shop_id: u64, level_id: u8) -> Option<CustomLevel> {
        let entity_id = Self::resolve_entity_id(shop_id)?;
        Self::get_custom_level_info_by_entity(entity_id, level_id)
    }

    /// 获取等级信息（entity_id 直达）
    pub fn get_custom_level_info_by_entity(entity_id: u64, level_id: u8) -> Option<CustomLevel> {
        EntityLevelSystems::<T>::get(entity_id)
            .and_then(|s| s.levels.iter().find(|l| l.id == level_id).cloned())
    }

    /// 获取等级折扣率
    pub fn get_level_discount(shop_id: u64, level_id: u8) -> u16 {
        Self::get_custom_level_info(shop_id, level_id)
            .map(|l| l.discount_rate)
            .unwrap_or(0)
    }

    /// 获取等级折扣率（entity_id 直达）
    pub fn get_level_discount_by_entity(entity_id: u64, level_id: u8) -> u16 {
        Self::get_custom_level_info_by_entity(entity_id, level_id)
            .map(|l| l.discount_rate)
            .unwrap_or(0)
    }

    /// 获取等级返佣加成
    pub fn get_level_commission_bonus(shop_id: u64, level_id: u8) -> u16 {
        Self::get_custom_level_info(shop_id, level_id)
            .map(|l| l.commission_bonus)
            .unwrap_or(0)
    }

    /// 获取等级返佣加成（entity_id 直达）
    pub fn get_level_commission_bonus_by_entity(entity_id: u64, level_id: u8) -> u16 {
        Self::get_custom_level_info_by_entity(entity_id, level_id)
            .map(|l| l.commission_bonus)
            .unwrap_or(0)
    }

    /// 获取有效等级（考虑过期）
    pub fn get_effective_level(shop_id: u64, account: &T::AccountId) -> u8 {
        let entity_id = match Self::resolve_entity_id(shop_id) {
            Some(id) => id,
            None => return 0,
        };
        Self::get_effective_level_by_entity(entity_id, account)
    }

    /// 获取有效等级（entity_id 直达，避免重复解析）
    ///
    /// S1 修复: 写穿模式 — 检测到过期时立即修正存储（EntityMembers、LevelMemberCount、MemberLevelExpiry），
    /// 确保所有 MemberProvider 路径（commission、order 等）都能触发惰性回退。
    /// S2 修复: AutoUpgrade 模式下，如果发现历史漏升导致 storage 等级低于当前应得等级，
    /// 则仅做惰性补升写穿（绝不在查询路径隐式降级）。
    pub fn get_effective_level_by_entity(entity_id: u64, account: &T::AccountId) -> u8 {
        let member = match EntityMembers::<T>::get(entity_id, account) {
            Some(m) => m,
            None => return 0,
        };

        if let Some(expires_at) = MemberLevelExpiry::<T>::get(entity_id, account) {
            let now = <frame_system::Pallet<T>>::block_number();
            if now > expires_at {
                let recalculated = Self::calculate_custom_level_full(
                    entity_id,
                    member.upgrade_eligible_spent,
                    member.direct_referrals,
                    member.team_size,
                    member.indirect_referrals,
                );
                // S1: 写穿修正存储
                if recalculated != member.custom_level_id {
                    let old_level = member.custom_level_id;
                    LevelMemberCount::<T>::mutate(entity_id, old_level, |c| {
                        *c = c.saturating_sub(1)
                    });
                    LevelMemberCount::<T>::mutate(entity_id, recalculated, |c| {
                        *c = c.saturating_add(1)
                    });
                    EntityMembers::<T>::mutate(entity_id, account, |maybe| {
                        if let Some(ref mut m) = maybe {
                            m.custom_level_id = recalculated;
                        }
                    });
                    T::OnMemberLevelChanged::on_level_changed(
                        entity_id,
                        account,
                        old_level,
                        recalculated,
                    );
                    Self::deposit_event(Event::MemberLevelExpired {
                        entity_id,
                        account: account.clone(),
                        expired_level_id: old_level,
                        new_level_id: recalculated,
                    });
                }
                MemberLevelExpiry::<T>::remove(entity_id, account);
                return recalculated;
            }
        }

        let system = match EntityLevelSystems::<T>::get(entity_id) {
            Some(s) => s,
            None => return member.custom_level_id,
        };

        // M9 审计修复: 验证 custom_level_id 对应的等级仍然存在
        // 等级可能被 remove_custom_level 删除，此时回退到基于消费的计算
        if system.use_custom
            && member.custom_level_id > 0
            && ((member.custom_level_id - 1) as usize) >= system.levels.len()
        {
            let recalculated = Self::calculate_custom_level_full(
                entity_id,
                member.upgrade_eligible_spent,
                member.direct_referrals,
                member.team_size,
                member.indirect_referrals,
            );
            if recalculated != member.custom_level_id {
                let old_level = member.custom_level_id;
                LevelMemberCount::<T>::mutate(entity_id, old_level, |c| *c = c.saturating_sub(1));
                LevelMemberCount::<T>::mutate(entity_id, recalculated, |c| {
                    *c = c.saturating_add(1)
                });
                EntityMembers::<T>::mutate(entity_id, account, |maybe| {
                    if let Some(ref mut m) = maybe {
                        m.custom_level_id = recalculated;
                    }
                });
                T::OnMemberLevelChanged::on_level_changed(
                    entity_id,
                    account,
                    old_level,
                    recalculated,
                );
                Self::deposit_event(Event::CustomLevelUpgraded {
                    entity_id,
                    account: account.clone(),
                    old_level_id: old_level,
                    new_level_id: recalculated,
                });
            }
            return recalculated;
        }

        // S2: 惰性补升写穿 —— 仅修复历史漏升，不在查询路径隐式降级
        if system.use_custom && system.upgrade_mode == LevelUpgradeMode::AutoUpgrade {
            let recalculated = Self::calculate_custom_level_full(
                entity_id,
                member.upgrade_eligible_spent,
                member.direct_referrals,
                member.team_size,
                member.indirect_referrals,
            );
            if recalculated > member.custom_level_id {
                let old_level = member.custom_level_id;
                LevelMemberCount::<T>::mutate(entity_id, old_level, |c| *c = c.saturating_sub(1));
                LevelMemberCount::<T>::mutate(entity_id, recalculated, |c| {
                    *c = c.saturating_add(1)
                });
                EntityMembers::<T>::mutate(entity_id, account, |maybe| {
                    if let Some(ref mut m) = maybe {
                        m.custom_level_id = recalculated;
                    }
                });
                T::OnMemberLevelChanged::on_level_changed(
                    entity_id,
                    account,
                    old_level,
                    recalculated,
                );
                Self::deposit_event(Event::CustomLevelUpgraded {
                    entity_id,
                    account: account.clone(),
                    old_level_id: old_level,
                    new_level_id: recalculated,
                });
                return recalculated;
            }
        }

        member.custom_level_id
    }

    /// 检查店铺是否使用自定义等级
    pub fn uses_custom_levels(shop_id: u64) -> bool {
        let entity_id = match Self::resolve_entity_id(shop_id) {
            Some(id) => id,
            None => return false,
        };
        Self::uses_custom_levels_by_entity(entity_id)
    }

    /// 检查实体是否使用自定义等级（entity_id 直达）
    pub fn uses_custom_levels_by_entity(entity_id: u64) -> bool {
        EntityLevelSystems::<T>::get(entity_id)
            .map(|s| s.use_custom)
            .unwrap_or(false)
    }

    /// 获取自定义等级数量
    pub fn custom_level_count(shop_id: u64) -> u8 {
        let entity_id = match Self::resolve_entity_id(shop_id) {
            Some(id) => id,
            None => return 0,
        };
        EntityLevelSystems::<T>::get(entity_id)
            .map(|s| s.levels.len() as u8)
            .unwrap_or(0)
    }

    /// 获取自定义等级数量（entity_id 直达）
    pub fn custom_level_count_by_entity(entity_id: u64) -> u8 {
        EntityLevelSystems::<T>::get(entity_id)
            .map(|s| s.levels.len() as u8)
            .unwrap_or(0)
    }

    // ========================================================================
    // 治理调用内部函数（供跨模块桥接使用，无 origin 检查）
    // ========================================================================

    /// 启用/禁用自定义等级（治理调用）
    pub fn governance_set_custom_levels_enabled(entity_id: u64, enabled: bool) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
            let system = maybe_system
                .as_mut()
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;
            system.use_custom = enabled;
            Ok(())
        })
    }

    /// 设置升级模式（治理调用）
    /// mode: 0=AutoUpgrade, 1=ManualUpgrade
    pub fn governance_set_upgrade_mode(entity_id: u64, mode: u8) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
            let system = maybe_system
                .as_mut()
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;
            system.upgrade_mode = match mode {
                0 => LevelUpgradeMode::AutoUpgrade,
                1 => LevelUpgradeMode::ManualUpgrade,
                _ => return Err(Error::<T>::InvalidUpgradeMode.into()),
            };
            Ok(())
        })
    }

    /// 添加自定义等级（治理调用）
    /// level_id 自动分配（= levels.len()），与 extrinsic add_custom_level 行为一致
    pub fn governance_add_custom_level(
        entity_id: u64,
        name: &[u8],
        threshold: u128,
        discount_rate: u16,
        commission_bonus: u16,
    ) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        let name: BoundedVec<u8, ConstU32<32>> = name
            .to_vec()
            .try_into()
            .map_err(|_| Error::<T>::NameTooLong)?;
        ensure!(!name.is_empty(), Error::<T>::EmptyLevelName);
        ensure!(discount_rate <= 10000, Error::<T>::InvalidBasisPoints);
        ensure!(commission_bonus <= 10000, Error::<T>::InvalidBasisPoints);

        let threshold_u64: u64 = threshold.try_into().map_err(|_| Error::<T>::Overflow)?;

        EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
            let system = maybe_system
                .as_mut()
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;

            if let Some(last) = system.levels.last() {
                ensure!(threshold_u64 > last.threshold, Error::<T>::InvalidThreshold);
            }

            let level_id = (system.levels.len() + 1) as u8;

            let level = CustomLevel {
                id: level_id,
                name,
                threshold: threshold_u64,
                discount_rate,
                commission_bonus,
                min_direct_referrals: 0,
                min_team_size: 0,
                min_indirect_referrals: 0,
            };

            system
                .levels
                .try_push(level)
                .map_err(|_| Error::<T>::LevelsFull)?;
            Ok(())
        })
    }

    /// 更新自定义等级（治理调用）
    pub fn governance_update_custom_level(
        entity_id: u64,
        level_id: u8,
        name: Option<&[u8]>,
        threshold: Option<u128>,
        discount_rate: Option<u16>,
        commission_bonus: Option<u16>,
    ) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
            let system = maybe_system
                .as_mut()
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;
            ensure!(
                level_id > 0 && ((level_id - 1) as usize) < system.levels.len(),
                Error::<T>::LevelNotFound
            );

            if let Some(new_threshold_u128) = threshold {
                let new_threshold: u64 = new_threshold_u128
                    .try_into()
                    .map_err(|_| Error::<T>::Overflow)?;
                if level_id > 1 {
                    if let Some(prev) = system.levels.get((level_id - 2) as usize) {
                        ensure!(new_threshold > prev.threshold, Error::<T>::InvalidThreshold);
                    }
                }
                if let Some(next) = system.levels.get(level_id as usize) {
                    ensure!(new_threshold < next.threshold, Error::<T>::InvalidThreshold);
                }
            }

            let level = system
                .levels
                .get_mut((level_id - 1) as usize)
                .ok_or(Error::<T>::LevelNotFound)?;

            if let Some(new_threshold_u128) = threshold {
                let new_threshold: u64 = new_threshold_u128
                    .try_into()
                    .map_err(|_| Error::<T>::Overflow)?;
                level.threshold = new_threshold;
            }
            if let Some(new_name) = name {
                let bounded_name: BoundedVec<u8, ConstU32<32>> = new_name
                    .to_vec()
                    .try_into()
                    .map_err(|_| Error::<T>::NameTooLong)?;
                ensure!(!bounded_name.is_empty(), Error::<T>::EmptyLevelName);
                level.name = bounded_name;
            }
            if let Some(rate) = discount_rate {
                ensure!(rate <= 10000, Error::<T>::InvalidBasisPoints);
                level.discount_rate = rate;
            }
            if let Some(bonus) = commission_bonus {
                ensure!(bonus <= 10000, Error::<T>::InvalidBasisPoints);
                level.commission_bonus = bonus;
            }
            Ok(())
        })
    }

    /// 删除自定义等级（治理调用）
    pub fn governance_remove_custom_level(entity_id: u64, level_id: u8) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
            let system = maybe_system
                .as_mut()
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;
            ensure!(
                level_id > 0 && (level_id - 1) as usize == system.levels.len().saturating_sub(1),
                Error::<T>::InvalidLevelId
            );
            // H1 审计修复: 检查该等级是否还有会员
            ensure!(
                LevelMemberCount::<T>::get(entity_id, level_id) == 0,
                Error::<T>::LevelHasMembers
            );
            system.levels.pop();
            T::OnLevelRemoved::on_level_removed(entity_id, level_id);
            Ok(())
        })
    }

    /// G1: 设置注册策略（治理调用）
    pub fn governance_set_registration_policy(entity_id: u64, policy_bits: u8) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        let policy = MemberRegistrationPolicy(policy_bits);
        ensure!(policy.is_valid(), Error::<T>::InvalidPolicyBits);
        EntityMemberPolicy::<T>::insert(entity_id, policy);
        Self::deposit_event(Event::GovernanceMemberPolicyUpdated { entity_id, policy });
        Ok(())
    }

    /// G1: 设置升级规则系统开关（治理调用）
    pub fn governance_set_upgrade_rule_system_enabled(
        entity_id: u64,
        enabled: bool,
    ) -> DispatchResult {
        ensure!(
            !T::EntityProvider::is_entity_locked(entity_id),
            Error::<T>::EntityLocked
        );
        EntityUpgradeRules::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
            let system = maybe_system
                .as_mut()
                .ok_or(Error::<T>::UpgradeRuleSystemNotInitialized)?;
            system.enabled = enabled;
            Self::deposit_event(Event::GovernanceUpgradeRuleSystemToggled { entity_id, enabled });
            Ok(())
        })
    }
}
