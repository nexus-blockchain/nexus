//! 会员注册、审批、移除、自动注册相关内部函数

use crate::pallet::*;
use crate::{runtime_api, KycChecker};
use frame_support::pallet_prelude::*;
#[cfg(test)]
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_entity_common::{AdminPermission, EntityProvider, OnMemberRemoved, ShopProvider};

impl<T: Config> Pallet<T> {
    /// REFERRAL_REQUIRED 策略下，验证推荐人自身已绑定上级
    ///
    /// Entity owner 作为推荐树根节点豁免此检查。
    pub(crate) fn ensure_referrer_bound(
        entity_id: u64,
        ref_account: &T::AccountId,
    ) -> DispatchResult {
        let policy = EntityMemberPolicy::<T>::get(entity_id);
        if !policy.requires_referral() {
            return Ok(());
        }
        // Entity owner 是推荐树根节点，不需要有上级
        if T::EntityProvider::entity_owner(entity_id).as_ref() == Some(ref_account) {
            return Ok(());
        }
        // 推荐人自己必须有推荐人
        let ref_member = EntityMembers::<T>::get(entity_id, ref_account)
            .ok_or(Error::<T>::InvalidReferrer)?;
        ensure!(ref_member.referrer.is_some(), Error::<T>::ReferrerNotBound);
        Ok(())
    }
    /// 解析 shop_id 对应的 entity_id
    pub(crate) fn resolve_entity_id(shop_id: u64) -> Option<u64> {
        T::ShopProvider::shop_entity_id(shop_id)
    }

    /// 通过 shop_id 查询会员（会员统一存储在 Entity 级别）
    pub fn get_member_by_shop(shop_id: u64, account: &T::AccountId) -> Option<EntityMemberOf<T>> {
        let entity_id = Self::resolve_entity_id(shop_id)?;
        EntityMembers::<T>::get(entity_id, account)
    }

    /// 通过 shop_id 检查是否为会员
    pub fn is_member_of_shop(shop_id: u64, account: &T::AccountId) -> bool {
        match Self::resolve_entity_id(shop_id) {
            Some(entity_id) => EntityMembers::<T>::contains_key(entity_id, account),
            None => false,
        }
    }

    /// 移除会员内部实现（供 remove_member 和 leave_entity 共用）
    ///
    /// S5+S6: 完整清理所有关联存储，维护 BannedMemberCount
    pub(crate) fn do_remove_member(entity_id: u64, account: &T::AccountId) -> DispatchResult {
        let member = EntityMembers::<T>::get(entity_id, account)
            .ok_or(Error::<T>::NotMember)?;

        // 有下线不允许移除（推荐链会断裂）
        ensure!(member.direct_referrals == 0, Error::<T>::MemberHasDownlines);

        // 1. 维护 MemberCount / LevelMemberCount
        MemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
        LevelMemberCount::<T>::mutate(entity_id, member.custom_level_id, |c| *c = c.saturating_sub(1));

        // S6: 维护 BannedMemberCount
        if member.banned_at.is_some() {
            BannedMemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));
        }

        // 2. 从推荐人的 DirectReferrals 中移除自己，递减推荐人统计
        if let Some(ref referrer) = member.referrer {
            DirectReferrals::<T>::mutate(entity_id, referrer, |referrals| {
                referrals.retain(|a| a != account);
            });
            EntityMembers::<T>::mutate(entity_id, referrer, |maybe_ref| {
                if let Some(ref mut r) = maybe_ref {
                    r.direct_referrals = r.direct_referrals.saturating_sub(1);
                    if member.is_qualified_referral {
                        r.qualified_referrals = r.qualified_referrals.saturating_sub(1);
                    }
                }
            });
            Self::decrement_team_size_by_entity(entity_id, referrer, member.is_qualified_referral);
        }

        // 3. 清理关联存储
        EntityMembers::<T>::remove(entity_id, account);
        DirectReferrals::<T>::remove(entity_id, account);
        MemberLevelExpiry::<T>::remove(entity_id, account);
        MemberUpgradeHistory::<T>::remove(entity_id, account);
        MemberOrderCount::<T>::remove(entity_id, account);

        // 4. 回调下游模块（通知佣金插件清理 per-user 存储）
        T::OnMemberRemoved::on_member_removed(entity_id, account);

        Ok(())
    }

    /// 注册会员内部实现
    pub(crate) fn do_register_member(
        entity_id: u64,
        account: &T::AccountId,
        referrer: Option<T::AccountId>,
        qualified: bool,
    ) -> DispatchResult {
        ensure!(
            !EntityMembers::<T>::contains_key(entity_id, account),
            Error::<T>::AlreadyMember
        );

        let now = <frame_system::Pallet<T>>::block_number();

        let member = EntityMember {
            referrer: referrer.clone(),
            direct_referrals: 0,
            qualified_referrals: 0,
            indirect_referrals: 0,
            qualified_indirect_referrals: 0,
            team_size: 0,
            total_spent: 0,
            custom_level_id: 0,
            joined_at: now,
            last_active_at: now,
            activated: false,
            is_qualified_referral: qualified,
            banned_at: None,
            ban_reason: None,
        };

        EntityMembers::<T>::insert(entity_id, account, member);
        MemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));
        LevelMemberCount::<T>::mutate(entity_id, 0u8, |c| *c = c.saturating_add(1));

        // 更新推荐人统计
        if let Some(ref ref_account) = referrer {
            Self::mutate_member_referral(entity_id, ref_account, account, qualified)?;
        }

        Ok(())
    }

    /// 更新推荐人统计（新会员绑定推荐人时调用）
    pub(crate) fn mutate_member_referral(
        entity_id: u64,
        ref_account: &T::AccountId,
        new_member: &T::AccountId,
        qualified: bool,
    ) -> DispatchResult {
        // 添加到直推列表
        DirectReferrals::<T>::try_mutate(entity_id, ref_account, |referrals| {
            referrals.try_push(new_member.clone()).map_err(|_| Error::<T>::ReferralsFull)
        })?;

        // 更新推荐人的 direct_referrals + qualified_referrals
        EntityMembers::<T>::mutate(entity_id, ref_account, |maybe_member| {
            if let Some(ref mut m) = maybe_member {
                m.direct_referrals = m.direct_referrals.saturating_add(1);
                if qualified {
                    m.qualified_referrals = m.qualified_referrals.saturating_add(1);
                }
            }
        });

        // 更新团队人数 + 间接推荐人数（entity 级别）
        Self::update_team_size_by_entity(entity_id, ref_account, qualified);

        // M10 修复: 推荐人统计变更后立即检查推荐类升级规则
        Self::check_referral_upgrade_rules_by_entity(entity_id, ref_account)?;

        Ok(())
    }

    /// 验证店主或管理员权限（MEMBER_MANAGE），成功时返回 entity_id
    pub(crate) fn ensure_shop_owner_or_admin(shop_id: u64, who: &T::AccountId) -> Result<u64, DispatchError> {
        let entity_id = T::ShopProvider::shop_entity_id(shop_id)
            .ok_or(Error::<T>::ShopNotFound)?;
        ensure!(
            T::EntityProvider::entity_owner(entity_id).as_ref() == Some(who)
                || T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::MEMBER_MANAGE),
            Error::<T>::NotEntityAdmin
        );
        Ok(entity_id)
    }

    /// 更新会员消费金额（USDT 精度 10^6）
    pub fn update_spent(
        shop_id: u64,
        account: &T::AccountId,
        amount_usdt: u64,
    ) -> DispatchResult {
        let entity_id = Self::resolve_entity_id(shop_id)
            .ok_or(Error::<T>::ShopNotFound)?;
        Self::update_spent_by_entity(entity_id, account, amount_usdt)
    }

    /// 更新会员消费金额（entity_id 直达，USDT 精度 10^6）
    pub fn update_spent_by_entity(
        entity_id: u64,
        account: &T::AccountId,
        amount_usdt: u64,
    ) -> DispatchResult {
        // 封禁会员静默跳过消费统计（不中断订单流程）
        if let Some(ref m) = EntityMembers::<T>::get(entity_id, account) {
            if m.banned_at.is_some() {
                return Ok(());
            }
        }

        EntityMembers::<T>::mutate(entity_id, account, |maybe_member| -> DispatchResult {
            let member = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;

            member.total_spent = member.total_spent.saturating_add(amount_usdt);
            member.last_active_at = <frame_system::Pallet<T>>::block_number();

            // 首次消费达标 → 激活会员
            if !member.activated && amount_usdt > 0 {
                member.activated = true;
                Self::deposit_event(Event::MemberActivated {
                    entity_id,
                    account: account.clone(),
                });
            }

            // P4 修复: 检查自定义等级是否已过期，若过期则立即修正存储
            // 确保后续比较基于正确的 custom_level_id
            if let Some(expires_at) = MemberLevelExpiry::<T>::get(entity_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                if now > expires_at {
                    let recalculated = Self::calculate_custom_level_by_entity(entity_id, member.total_spent);
                    if recalculated != member.custom_level_id {
                        let expired_level_id = member.custom_level_id;
                        // 维护 LevelMemberCount
                        LevelMemberCount::<T>::mutate(entity_id, expired_level_id, |c| *c = c.saturating_sub(1));
                        LevelMemberCount::<T>::mutate(entity_id, recalculated, |c| *c = c.saturating_add(1));
                        member.custom_level_id = recalculated;

                        Self::deposit_event(Event::MemberLevelExpired {
                            entity_id,
                            account: account.clone(),
                            expired_level_id,
                            new_level_id: recalculated,
                        });
                    }
                    MemberLevelExpiry::<T>::remove(entity_id, account);
                }
            }

            // 计算自定义等级（如果启用且为自动升级模式，entity 级别）
            // 如果会员有活跃的规则升级（未过期），不执行自动降级
            let has_active_rule_upgrade = MemberLevelExpiry::<T>::get(entity_id, account)
                .map(|exp| <frame_system::Pallet<T>>::block_number() <= exp)
                .unwrap_or(false);
            if let Some(system) = EntityLevelSystems::<T>::get(entity_id) {
                if system.use_custom && system.upgrade_mode == LevelUpgradeMode::AutoUpgrade {
                    let new_custom_level = Self::calculate_custom_level_by_entity(entity_id, member.total_spent);
                    if new_custom_level != member.custom_level_id
                        && !(has_active_rule_upgrade && new_custom_level < member.custom_level_id) {
                        let old_level_id = member.custom_level_id;
                        // 维护 LevelMemberCount
                        LevelMemberCount::<T>::mutate(entity_id, old_level_id, |c| *c = c.saturating_sub(1));
                        LevelMemberCount::<T>::mutate(entity_id, new_custom_level, |c| *c = c.saturating_add(1));
                        member.custom_level_id = new_custom_level;

                        Self::deposit_event(Event::CustomLevelUpgraded {
                            entity_id,
                            account: account.clone(),
                            old_level_id,
                            new_level_id: new_custom_level,
                        });
                    }
                }
            }

            Ok(())
        })
    }

    /// 自动注册会员（系统调用：下单、复购赠送等）
    ///
    /// 策略行为：
    /// - PURCHASE_REQUIRED: 不阻拦（auto_register 本身由购买/赠送触发）
    /// - REFERRAL_REQUIRED: 无推荐人时静默跳过（不注册，不报错）
    /// - APPROVAL_REQUIRED: 进入待审批状态
    pub fn auto_register(
        shop_id: u64,
        account: &T::AccountId,
        referrer: Option<T::AccountId>,
    ) -> DispatchResult {
        let entity_id = Self::resolve_entity_id(shop_id)
            .ok_or(Error::<T>::ShopNotFound)?;

        if EntityMembers::<T>::contains_key(entity_id, account) {
            return Ok(()); // 已是会员
        }

        // 验证推荐人
        let valid_referrer = if let Some(ref ref_account) = referrer {
            if ref_account != account && EntityMembers::<T>::contains_key(entity_id, ref_account) {
                referrer
            } else {
                None
            }
        } else {
            None
        };

        // ---- 注册策略检查 ----
        let policy = EntityMemberPolicy::<T>::get(entity_id);

        // KYC_REQUIRED: 注册时需要通过 KYC
        if policy.requires_kyc() {
            ensure!(T::KycChecker::is_kyc_passed(entity_id, account), Error::<T>::KycNotPassed);
        }

        // REFERRAL_REQUIRED: 无有效推荐人时拒绝（先判断推荐人，没有推荐人不可购买）
        if policy.requires_referral() && valid_referrer.is_none() {
            return Err(Error::<T>::ReferralRequiredForRegistration.into());
        }

        // APPROVAL_REQUIRED: 进入待审批状态（购买触发也需审批）
        if policy.requires_approval() {
            if !PendingMembers::<T>::contains_key(entity_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                PendingMembers::<T>::insert(entity_id, account, (valid_referrer.clone(), now));

                Self::deposit_event(Event::MemberPendingApproval {
                    entity_id,
                    account: account.clone(),
                    referrer: valid_referrer,
                });
            }
            return Ok(());
        }

        Self::do_register_member(entity_id, account, valid_referrer.clone(), true)?;

        Self::deposit_event(Event::MemberRegistered {
            entity_id,
            shop_id: Some(shop_id),
            account: account.clone(),
            referrer: valid_referrer,
        });

        Ok(())
    }

    /// 自动注册会员（entity_id 直达版本，供 commission-core 等已持有 entity_id 的模块调用）
    ///
    /// 与 `auto_register` 逻辑一致，但跳过 shop_id → entity_id 解析。
    /// 事件中 entity_id 字段为实体 ID。
    ///
    /// `qualified`: 是否为有效直推（购买触发=true，复购赠与=false）
    pub fn auto_register_by_entity(
        entity_id: u64,
        account: &T::AccountId,
        referrer: Option<T::AccountId>,
        qualified: bool,
    ) -> DispatchResult {
        if EntityMembers::<T>::contains_key(entity_id, account) {
            return Ok(()); // 已是会员
        }

        // 验证推荐人
        let valid_referrer = if let Some(ref ref_account) = referrer {
            if ref_account != account && EntityMembers::<T>::contains_key(entity_id, ref_account) {
                referrer
            } else {
                None
            }
        } else {
            None
        };

        // ---- 注册策略检查 ----
        let policy = EntityMemberPolicy::<T>::get(entity_id);

        // KYC_REQUIRED: 注册时需要通过 KYC
        if policy.requires_kyc() {
            ensure!(T::KycChecker::is_kyc_passed(entity_id, account), Error::<T>::KycNotPassed);
        }

        if policy.requires_referral() && valid_referrer.is_none() {
            return Err(Error::<T>::ReferralRequiredForRegistration.into());
        }

        if policy.requires_approval() {
            if !PendingMembers::<T>::contains_key(entity_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                PendingMembers::<T>::insert(entity_id, account, (valid_referrer.clone(), now));

                Self::deposit_event(Event::MemberPendingApproval {
                    entity_id,
                    account: account.clone(),
                    referrer: valid_referrer,
                });
            }
            return Ok(());
        }

        Self::do_register_member(entity_id, account, valid_referrer.clone(), qualified)?;

        Self::deposit_event(Event::MemberRegistered {
            entity_id,
            shop_id: None,
            account: account.clone(),
            referrer: valid_referrer,
        });

        Ok(())
    }

    /// 测试辅助：暴露 apply_upgrade 给测试模块
    #[cfg(test)]
    pub fn apply_upgrade_for_test(
        entity_id: u64,
        account: &T::AccountId,
        rule_id: u32,
        target_level_id: u8,
        duration: Option<BlockNumberFor<T>>,
        stackable: bool,
    ) -> DispatchResult {
        Self::apply_upgrade(entity_id, account, rule_id, target_level_id, duration, stackable)
    }

    /// 查询指定会员的推荐团队树（供 Runtime API 调用）
    ///
    /// - `entity_id`: 实体 ID
    /// - `account`: 查询的会员账户
    /// - `depth`: 递归深度（1 = 仅直推，2 = 直推 + 二级，上限 clamp 到 2）
    pub fn get_referral_team(
        entity_id: u64,
        account: &T::AccountId,
        depth: u32,
    ) -> alloc::vec::Vec<runtime_api::TeamMemberInfo<T::AccountId>> {
        let depth = depth.min(2).max(1);
        Self::build_team_level(entity_id, account, depth)
    }

    fn build_team_level(
        entity_id: u64,
        parent: &T::AccountId,
        remaining_depth: u32,
    ) -> alloc::vec::Vec<runtime_api::TeamMemberInfo<T::AccountId>> {
        if remaining_depth == 0 {
            return alloc::vec::Vec::new();
        }

        let referrals = DirectReferrals::<T>::get(entity_id, parent);
        let mut result = alloc::vec::Vec::with_capacity(referrals.len());

        for child in referrals.iter() {
            let children = if remaining_depth > 1 {
                Self::build_team_level(entity_id, child, remaining_depth - 1)
            } else {
                alloc::vec::Vec::new()
            };

            let info = EntityMembers::<T>::get(entity_id, child);
            let (level_id, total_spent, direct_referrals, team_size, joined_at, last_active_at, is_banned) =
                match info {
                    Some(m) => {
                        let effective_level = Self::get_effective_level_by_entity(entity_id, child);
                        (
                            effective_level,
                            m.total_spent,
                            m.direct_referrals,
                            m.team_size,
                            sp_runtime::SaturatedConversion::saturated_into(m.joined_at),
                            sp_runtime::SaturatedConversion::saturated_into(m.last_active_at),
                            m.banned_at.is_some(),
                        )
                    }
                    None => (0, 0, 0, 0, 0u64, 0u64, false),
                };

            result.push(runtime_api::TeamMemberInfo {
                account: child.clone(),
                level_id,
                total_spent,
                direct_referrals,
                team_size,
                joined_at,
                last_active_at,
                is_banned,
                children,
            });
        }

        result
    }

    /// 查询会员仪表盘信息（供 Runtime API 调用）
    pub fn get_member_info(
        entity_id: u64,
        account: &T::AccountId,
    ) -> Option<runtime_api::MemberDashboardInfo<T::AccountId>> {
        let member = EntityMembers::<T>::get(entity_id, account)?;
        let effective_level_id = Self::get_effective_level_by_entity(entity_id, account);
        let order_count = MemberOrderCount::<T>::get(entity_id, account);
        let level_expires_at = MemberLevelExpiry::<T>::get(entity_id, account)
            .map(|exp| sp_runtime::SaturatedConversion::saturated_into(exp));
        let upgrade_history: alloc::vec::Vec<runtime_api::UpgradeRecordInfo> =
            MemberUpgradeHistory::<T>::get(entity_id, account)
                .into_iter()
                .map(|r| runtime_api::UpgradeRecordInfo {
                    rule_id: r.rule_id,
                    from_level_id: r.from_level_id,
                    to_level_id: r.to_level_id,
                    upgraded_at: sp_runtime::SaturatedConversion::saturated_into(r.upgraded_at),
                    expires_at: r.expires_at.map(|e| sp_runtime::SaturatedConversion::saturated_into(e)),
                })
                .collect();

        Some(runtime_api::MemberDashboardInfo {
            referrer: member.referrer,
            custom_level_id: member.custom_level_id,
            effective_level_id,
            total_spent: member.total_spent,
            direct_referrals: member.direct_referrals,
            qualified_referrals: member.qualified_referrals,
            indirect_referrals: member.indirect_referrals,
            team_size: member.team_size,
            order_count,
            joined_at: sp_runtime::SaturatedConversion::saturated_into(member.joined_at),
            last_active_at: sp_runtime::SaturatedConversion::saturated_into(member.last_active_at),
            is_banned: member.banned_at.is_some(),
            banned_at: member.banned_at.map(|b| sp_runtime::SaturatedConversion::saturated_into(b)),
            level_expires_at,
            upgrade_history,
        })
    }

    /// 查询 Entity 会员总览信息（供 Runtime API 调用）
    pub fn get_entity_member_overview(
        entity_id: u64,
    ) -> runtime_api::EntityMemberOverview {
        let total_members = MemberCount::<T>::get(entity_id);

        // 等级分布：从等级系统中读取已定义的等级，获取各等级计数
        let mut level_distribution = alloc::vec::Vec::new();
        // level 0 始终存在
        let level_0_count = LevelMemberCount::<T>::get(entity_id, 0u8);
        level_distribution.push((0u8, level_0_count));

        if let Some(system) = EntityLevelSystems::<T>::get(entity_id) {
            for level in system.levels.iter() {
                let count = LevelMemberCount::<T>::get(entity_id, level.id);
                level_distribution.push((level.id, count));
            }
        }

        // 待审批数量：遍历 PendingMembers prefix
        let pending_count = PendingMembers::<T>::iter_prefix(entity_id).count() as u32;

        // S6: 使用 BannedMemberCount 计数器（O(1) 替代 O(N) 遍历）
        let banned_count = BannedMemberCount::<T>::get(entity_id);

        runtime_api::EntityMemberOverview {
            total_members,
            level_distribution,
            pending_count,
            banned_count,
        }
    }

    /// O1: 分页查询实体会员列表（供 Runtime API 调用）
    pub fn get_members_paginated(
        entity_id: u64,
        page_size: u32,
        page_index: u32,
    ) -> runtime_api::PaginatedMembersResult<T::AccountId> {
        let page_size = page_size.min(100).max(1);
        let total = MemberCount::<T>::get(entity_id);
        let skip = (page_index as usize).saturating_mul(page_size as usize);

        let members: alloc::vec::Vec<runtime_api::PaginatedMemberInfo<T::AccountId>> =
            EntityMembers::<T>::iter_prefix(entity_id)
                .skip(skip)
                .take((page_size as usize).saturating_add(1)) // 多取一条判断 has_more
                .map(|(account, member)| {
                    let level_id = Self::get_effective_level_by_entity(entity_id, &account);
                    runtime_api::PaginatedMemberInfo {
                        account,
                        level_id,
                        total_spent: member.total_spent,
                        direct_referrals: member.direct_referrals,
                        team_size: member.team_size,
                        joined_at: sp_runtime::SaturatedConversion::saturated_into(member.joined_at),
                        is_banned: member.banned_at.is_some(),
                        ban_reason: member.ban_reason.map(|r| r.into_inner()),
                    }
                })
                .collect();

        let has_more = members.len() > page_size as usize;
        let members = if has_more {
            members.into_iter().take(page_size as usize).collect()
        } else {
            members
        };

        runtime_api::PaginatedMembersResult {
            members,
            total,
            has_more,
        }
    }

    // ========================================================================
    // 推荐链路可视化 API 实现
    // ========================================================================

    /// 上行链路：从 account 向上追溯到根节点（供 Runtime API 调用）
    ///
    /// 沿 referrer 链逐级上溯，返回完整路径 [account, referrer, grand_referrer, ..., root]。
    /// 使用 visited 集合防止异常数据导致的无限循环。
    pub fn get_upline_chain(
        entity_id: u64,
        account: &T::AccountId,
        max_depth: u32,
    ) -> runtime_api::UplineChainResult<T::AccountId> {
        use alloc::collections::BTreeSet;

        let max_depth = max_depth.min(100).max(1);
        let mut chain = alloc::vec::Vec::new();
        let mut visited = BTreeSet::new();
        let mut truncated = false;

        // 先把起始节点加入
        let mut current = Some(account.clone());

        while let Some(ref curr_account) = current {
            if visited.contains(curr_account) {
                break;
            }
            visited.insert(curr_account.clone());

            match EntityMembers::<T>::get(entity_id, curr_account) {
                Some(m) => {
                    let effective_level = Self::get_effective_level_by_entity(entity_id, curr_account);
                    chain.push(runtime_api::UplineNode {
                        account: curr_account.clone(),
                        level_id: effective_level,
                        team_size: m.team_size,
                        joined_at: sp_runtime::SaturatedConversion::saturated_into(m.joined_at),
                    });

                    if chain.len() as u32 >= max_depth {
                        // 还有上级但已达深度上限
                        if m.referrer.is_some() {
                            truncated = true;
                        }
                        break;
                    }

                    current = m.referrer;
                }
                None => {
                    // 会员不存在（起始账户不是会员或链断裂），停止
                    break;
                }
            }
        }

        let depth = chain.len() as u32;
        runtime_api::UplineChainResult { chain, truncated, depth }
    }

    /// 深层下行推荐树：递归展开推荐子树（供 Runtime API 调用）
    ///
    /// 以 account 为根节点，向下展开 depth 层推荐关系。
    /// depth 范围 [1, 10]，每节点的子节点数受 MaxDirectReferrals 限制。
    pub fn get_referral_tree(
        entity_id: u64,
        account: &T::AccountId,
        depth: u32,
    ) -> runtime_api::ReferralTreeNode<T::AccountId> {
        let depth = depth.min(10).max(1);
        Self::build_tree_node(entity_id, account, depth)
    }

    /// 递归构建推荐树节点
    fn build_tree_node(
        entity_id: u64,
        account: &T::AccountId,
        remaining_depth: u32,
    ) -> runtime_api::ReferralTreeNode<T::AccountId> {
        let (level_id, direct_referrals, team_size, total_spent, joined_at, is_banned) =
            match EntityMembers::<T>::get(entity_id, account) {
                Some(m) => {
                    let effective_level = Self::get_effective_level_by_entity(entity_id, account);
                    (
                        effective_level,
                        m.direct_referrals,
                        m.team_size,
                        m.total_spent,
                        sp_runtime::SaturatedConversion::saturated_into::<u64>(m.joined_at),
                        m.banned_at.is_some(),
                    )
                }
                None => (0, 0, 0, 0, 0u64, false),
            };

        let (children, has_more_children) = if remaining_depth > 0 {
            let referrals = DirectReferrals::<T>::get(entity_id, account);
            let child_nodes: alloc::vec::Vec<_> = referrals
                .iter()
                .map(|child| Self::build_tree_node(entity_id, child, remaining_depth - 1))
                .collect();
            // has_more_children: 深度已耗尽时，检查子节点是否还有下级
            let has_more = if remaining_depth == 1 {
                // 当前是最后一层展开，检查各子节点是否有自己的下级
                referrals.iter().any(|child| {
                    DirectReferrals::<T>::get(entity_id, child).len() > 0
                })
            } else {
                false
            };
            (child_nodes, has_more)
        } else {
            // 深度为 0，不展开子节点，检查是否存在子节点
            let has_more = DirectReferrals::<T>::get(entity_id, account).len() > 0;
            (alloc::vec::Vec::new(), has_more)
        };

        runtime_api::ReferralTreeNode {
            account: account.clone(),
            level_id,
            direct_referrals,
            team_size,
            total_spent,
            joined_at,
            is_banned,
            children,
            has_more_children,
        }
    }

    /// 按代分页查询：获取指定会员第 N 代下级（供 Runtime API 调用）
    ///
    /// generation=1 是直推，generation=2 是二代（直推的直推），以此类推。
    /// 采用 BFS 逐层展开至目标代，中间层只收集 account 列表用于下一层，
    /// 目标层做分页返回。
    pub fn get_referrals_by_generation(
        entity_id: u64,
        account: &T::AccountId,
        generation: u32,
        page_size: u32,
        page_index: u32,
    ) -> runtime_api::PaginatedGenerationResult<T::AccountId> {
        let generation = generation.min(20).max(1);
        let page_size = page_size.min(100).max(1);

        // BFS 逐层展开：每层收集 (child_account, parent_account) 对
        // 第 0 层 = [account 自己]，第 1 层 = account 的直推，第 N 层 = 第 N-1 层的直推
        let mut current_layer: alloc::vec::Vec<T::AccountId> = alloc::vec![account.clone()];

        for _gen in 0..generation {
            let mut next_layer = alloc::vec::Vec::new();
            for parent in current_layer.iter() {
                let referrals = DirectReferrals::<T>::get(entity_id, parent);
                for child in referrals.iter() {
                    next_layer.push(child.clone());
                }
            }
            current_layer = next_layer;
            if current_layer.is_empty() {
                break;
            }
        }

        // current_layer 现在是第 generation 代的所有成员
        let total_count = current_layer.len() as u32;
        let skip = (page_index as usize).saturating_mul(page_size as usize);

        // 为判断 has_more，多取一条
        let page_members: alloc::vec::Vec<runtime_api::GenerationMemberInfo<T::AccountId>> =
            current_layer
                .into_iter()
                .skip(skip)
                .take((page_size as usize).saturating_add(1))
                .filter_map(|child_account| {
                    let member = EntityMembers::<T>::get(entity_id, &child_account)?;
                    let effective_level = Self::get_effective_level_by_entity(entity_id, &child_account);
                    // referrer 一定存在（能出现在某人的 DirectReferrals 中就有 referrer）
                    let referrer = member.referrer.clone().unwrap_or_else(|| child_account.clone());
                    Some(runtime_api::GenerationMemberInfo {
                        account: child_account,
                        level_id: effective_level,
                        direct_referrals: member.direct_referrals,
                        team_size: member.team_size,
                        total_spent: member.total_spent,
                        joined_at: sp_runtime::SaturatedConversion::saturated_into(member.joined_at),
                        is_banned: member.banned_at.is_some(),
                        referrer,
                    })
                })
                .collect();

        let has_more = page_members.len() > page_size as usize;
        let members = if has_more {
            page_members.into_iter().take(page_size as usize).collect()
        } else {
            page_members
        };

        runtime_api::PaginatedGenerationResult {
            generation,
            members,
            total_count,
            page_size,
            page_index,
            has_more,
        }
    }
}
