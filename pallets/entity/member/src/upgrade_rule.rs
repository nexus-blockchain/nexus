//! 升级规则系统：规则评估、冲突解决、升级应用

use crate::pallet::*;
use crate::KycChecker;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::Saturating;

impl<T: Config> Pallet<T> {
    /// 检查订单完成时的升级规则
    pub fn check_order_upgrade_rules(
        shop_id: u64,
        buyer: &T::AccountId,
        product_id: u64,
        amount_usdt: u64,
    ) -> DispatchResult {
        let entity_id = Self::resolve_entity_id(shop_id)
            .ok_or(Error::<T>::ShopNotFound)?;
        Self::check_order_upgrade_rules_by_entity(entity_id, buyer, product_id, amount_usdt)
    }

    /// 检查订单完成时的升级规则（entity_id 直达）
    pub fn check_order_upgrade_rules_by_entity(
        entity_id: u64,
        buyer: &T::AccountId,
        product_id: u64,
        amount_usdt: u64,
    ) -> DispatchResult {
        // 无论规则系统是否存在，始终追踪订单数量
        if EntityMembers::<T>::contains_key(entity_id, buyer) {
            MemberOrderCount::<T>::mutate(entity_id, buyer, |count| {
                *count = count.saturating_add(1);
            });
        }

        let system = match EntityUpgradeRules::<T>::get(entity_id) {
            Some(s) if s.enabled => s,
            _ => return Ok(()),
        };

        let member = match EntityMembers::<T>::get(entity_id, buyer) {
            Some(m) => m,
            None => return Ok(()),
        };

        let order_count = MemberOrderCount::<T>::get(entity_id, buyer);

        // 收集匹配的规则
        let mut matched_rules: alloc::vec::Vec<(u32, u8, Option<BlockNumberFor<T>>, u8, bool)> = alloc::vec::Vec::new();

        for rule in system.rules.iter() {
            if !rule.enabled {
                continue;
            }

            // C3 fix: 检查是否已达到最大触发次数
            if let Some(max) = rule.max_triggers {
                if rule.trigger_count >= max {
                    continue;
                }
            }

            let matches = match &rule.trigger {
                UpgradeTrigger::PurchaseProduct { product_id: pid } => {
                    *pid == product_id
                },
                UpgradeTrigger::SingleOrder { threshold } => {
                    amount_usdt >= *threshold
                },
                UpgradeTrigger::TotalSpent { threshold } => {
                    member.total_spent >= *threshold
                },
                UpgradeTrigger::ReferralCount { count } => {
                    let policy = EntityMemberStatsPolicy::<T>::get(entity_id);
                    let referrals = if policy.include_repurchase_direct() {
                        member.direct_referrals
                    } else {
                        member.qualified_referrals
                    };
                    referrals >= *count
                },
                UpgradeTrigger::TeamSize { size } => {
                    member.team_size >= *size
                },
                UpgradeTrigger::OrderCount { count } => {
                    order_count >= *count
                },
            };

            if matches {
                matched_rules.push((
                    rule.id,
                    rule.target_level_id,
                    rule.duration,
                    rule.priority,
                    rule.stackable,
                ));
            }
        }

        if matched_rules.is_empty() {
            return Ok(());
        }

        // 根据冲突策略选择规则
        let selected = Self::resolve_conflict(&matched_rules, &system.conflict_strategy);

        if let Some((rule_id, target_level_id, duration, _, stackable)) = selected {
            Self::apply_upgrade(entity_id, buyer, rule_id, target_level_id, duration, stackable)?;
        }

        Ok(())
    }

    /// M10 修复: 检查推荐类升级规则（在推荐人统计变更后调用）
    ///
    /// 仅评估 ReferralCount 和 TeamSize 触发器，避免在注册路径上
    /// 重复评估订单类触发器（PurchaseProduct, SingleOrder, TotalSpent, OrderCount）。
    pub(crate) fn check_referral_upgrade_rules_by_entity(
        entity_id: u64,
        account: &T::AccountId,
    ) -> DispatchResult {
        let system = match EntityUpgradeRules::<T>::get(entity_id) {
            Some(s) if s.enabled => s,
            _ => return Ok(()),
        };

        let member = match EntityMembers::<T>::get(entity_id, account) {
            Some(m) => m,
            None => return Ok(()),
        };

        let mut matched_rules: alloc::vec::Vec<(u32, u8, Option<BlockNumberFor<T>>, u8, bool)> = alloc::vec::Vec::new();

        for rule in system.rules.iter() {
            if !rule.enabled {
                continue;
            }

            if let Some(max) = rule.max_triggers {
                if rule.trigger_count >= max {
                    continue;
                }
            }

            let matches = match &rule.trigger {
                UpgradeTrigger::ReferralCount { count } => {
                    let policy = EntityMemberStatsPolicy::<T>::get(entity_id);
                    let referrals = if policy.include_repurchase_direct() {
                        member.direct_referrals
                    } else {
                        member.qualified_referrals
                    };
                    referrals >= *count
                },
                UpgradeTrigger::TeamSize { size } => {
                    member.team_size >= *size
                },
                _ => false,
            };

            if matches {
                matched_rules.push((
                    rule.id,
                    rule.target_level_id,
                    rule.duration,
                    rule.priority,
                    rule.stackable,
                ));
            }
        }

        if matched_rules.is_empty() {
            return Ok(());
        }

        let selected = Self::resolve_conflict(&matched_rules, &system.conflict_strategy);

        if let Some((rule_id, target_level_id, duration, _, stackable)) = selected {
            Self::apply_upgrade(entity_id, account, rule_id, target_level_id, duration, stackable)?;
        }

        Ok(())
    }

    /// 解决规则冲突
    pub(crate) fn resolve_conflict(
        rules: &[(u32, u8, Option<BlockNumberFor<T>>, u8, bool)],
        strategy: &ConflictStrategy,
    ) -> Option<(u32, u8, Option<BlockNumberFor<T>>, u8, bool)> {
        if rules.is_empty() {
            return None;
        }

        match strategy {
            ConflictStrategy::HighestLevel => {
                rules.iter().max_by_key(|r| r.1).cloned()
            },
            ConflictStrategy::HighestPriority => {
                rules.iter().max_by_key(|r| r.3).cloned()
            },
            ConflictStrategy::LongestDuration => {
                // None = 永久，应视为最长；用 Bounded::max_value 替代 None 参与比较
                use sp_runtime::traits::Bounded;
                rules.iter().max_by_key(|r| match r.2 {
                    None => <BlockNumberFor<T>>::max_value(),
                    Some(d) => d,
                }).cloned()
            },
            ConflictStrategy::FirstMatch => {
                rules.first().cloned()
            },
        }
    }

    /// 应用升级（entity 级别存储）
    pub(crate) fn apply_upgrade(
        entity_id: u64,
        account: &T::AccountId,
        rule_id: u32,
        target_level_id: u8,
        duration: Option<BlockNumberFor<T>>,
        stackable: bool,
    ) -> DispatchResult {
        // 封禁会员静默跳过升级（自动升级路径不应返回错误中断订单流程）
        if let Some(ref m) = EntityMembers::<T>::get(entity_id, account) {
            if m.banned_at.is_some() {
                return Ok(());
            }
        }

        // KYC_UPGRADE_REQUIRED: 升级时需要通过 KYC
        let policy = EntityMemberPolicy::<T>::get(entity_id);
        if policy.requires_kyc_for_upgrade() {
            if !T::KycChecker::is_kyc_passed(entity_id, account) {
                return Ok(()); // 静默跳过（自动升级路径不应返回错误中断订单流程）
            }
        }

        // H1 审计修复: 等级系统不存在时静默跳过（reset_level_system 后规则仍可能触发）
        let level_system = match EntityLevelSystems::<T>::get(entity_id) {
            Some(s) => s,
            None => return Ok(()),
        };

        let now = <frame_system::Pallet<T>>::block_number();

        EntityMembers::<T>::mutate(entity_id, account, |maybe_member| -> DispatchResult {
            let member = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;

            // H7 审计修复: 验证目标等级仍然存在（等级可能在规则创建后被删除）
            if level_system.use_custom && (target_level_id as usize) >= level_system.levels.len() {
                return Ok(());
            }

            let old_level_id = member.custom_level_id;

            // 检查是否需要升级（绝不降级）
            if target_level_id < old_level_id {
                return Ok(());
            }
            if target_level_id == old_level_id && !stackable {
                return Ok(());
            }

            // 计算过期时间
            let expires_at = if stackable {
                match MemberLevelExpiry::<T>::get(entity_id, account) {
                    Some(exp) => duration.map(|d| exp.saturating_add(d)),
                    None => {
                        if target_level_id == old_level_id {
                            return Ok(());
                        }
                        duration.map(|d| now.saturating_add(d))
                    }
                }
            } else {
                duration.map(|d| now.saturating_add(d))
            };

            // 维护 LevelMemberCount
            LevelMemberCount::<T>::mutate(entity_id, old_level_id, |c| *c = c.saturating_sub(1));
            LevelMemberCount::<T>::mutate(entity_id, target_level_id, |c| *c = c.saturating_add(1));

            // 升级等级
            member.custom_level_id = target_level_id;

            // 设置过期时间
            if let Some(exp) = expires_at {
                MemberLevelExpiry::<T>::insert(entity_id, account, exp);
            } else {
                MemberLevelExpiry::<T>::remove(entity_id, account);
            }

            // 记录升级历史
            let _ = MemberUpgradeHistory::<T>::try_mutate(entity_id, account, |history| {
                let record = UpgradeRecord {
                    rule_id,
                    from_level_id: old_level_id,
                    to_level_id: target_level_id,
                    upgraded_at: now,
                    expires_at,
                };
                // L2 审计修复: 历史记录满时记录警告，而非静默丢弃
                if history.try_push(record).is_err() {
                    log::warn!(
                        "MemberUpgradeHistory full for entity={}, rule_id={}, from={} to={}",
                        entity_id, rule_id, old_level_id, target_level_id,
                    );
                }
                Ok::<_, Error<T>>(())
            });

            // 更新规则触发计数
            EntityUpgradeRules::<T>::mutate(entity_id, |maybe_system| {
                if let Some(system) = maybe_system {
                    if let Some(r) = system.rules.iter_mut().find(|r| r.id == rule_id) {
                        r.trigger_count = r.trigger_count.saturating_add(1);
                    }
                }
            });

            Self::deposit_event(Event::MemberUpgradedByRule {
                entity_id,
                account: account.clone(),
                rule_id,
                from_level_id: old_level_id,
                to_level_id: target_level_id,
                expires_at,
            });

            Ok(())
        })
    }
}
