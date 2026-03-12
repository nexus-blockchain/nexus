//! 推荐链管理、团队人数维护、循环检测

use crate::pallet::*;

impl<T: Config> Pallet<T> {
    /// 检查是否存在循环推荐
    /// P2 安全修复: 增加已访问集合检测，防止无限循环
    pub(crate) fn is_circular_referral(
        shop_id: u64,
        account: &T::AccountId,
        referrer: &T::AccountId,
    ) -> bool {
        use alloc::collections::BTreeSet;

        let mut current = Some(referrer.clone());
        let mut depth = 0u32;
        let mut visited = BTreeSet::new();
        const MAX_DEPTH: u32 = 100;

        while let Some(ref curr_account) = current {
            // 检查是否回到了要绑定的账户
            if curr_account == account {
                return true;
            }

            // 检查是否已访问过（检测链中的其他循环）
            if visited.contains(curr_account) {
                // 链中存在循环，但不涉及 account，安全
                break;
            }
            visited.insert(curr_account.clone());

            if depth >= MAX_DEPTH {
                break;
            }
            current = Self::get_member_by_shop(shop_id, curr_account)
                .and_then(|m| m.referrer);
            depth += 1;
        }

        false
    }

    /// 更新团队人数 + 间接推荐人数（递归向上更新，entity 级别）
    /// H1 审计修复: 添加 visited 集合防止循环引用导致重复 +1
    /// 方案 D: 有界递归（MAX_SYNC_DEPTH=15），溢出部分异步补偿
    ///
    /// depth=0 是直接推荐人（team_size++，直推已在 mutate_member_referral 中单独处理）
    /// depth>=1 是间接推荐人（team_size++ 且 indirect_referrals++）
    pub(crate) fn update_team_size_by_entity(entity_id: u64, account: &T::AccountId, qualified: bool) {
        use alloc::collections::BTreeSet;

        let mut current = Some(account.clone());
        let mut depth = 0u32;
        let mut visited = BTreeSet::new();
        let mut last_account = account.clone();

        while let Some(ref curr_account) = current {
            if depth >= MAX_SYNC_DEPTH {
                break;
            }

            if visited.contains(curr_account) {
                break;
            }

            visited.insert(curr_account.clone());
            last_account = curr_account.clone();

            EntityMembers::<T>::mutate(entity_id, curr_account, |maybe_member| {
                if let Some(ref mut m) = maybe_member {
                    m.team_size = m.team_size.saturating_add(1);
                    if depth >= 1 {
                        m.indirect_referrals = m.indirect_referrals.saturating_add(1);
                        if qualified {
                            m.qualified_indirect_referrals = m.qualified_indirect_referrals.saturating_add(1);
                        }
                    }
                }
            });

            current = EntityMembers::<T>::get(entity_id, curr_account)
                .and_then(|m| m.referrer);
            depth += 1;
        }

        // 溢出部分写入异步补偿队列
        if current.is_some() && depth >= MAX_SYNC_DEPTH {
            let update_id = NextPendingUpdateId::<T>::get();
            PendingTeamSizeUpdates::<T>::insert(update_id, TeamSizeUpdate {
                entity_id,
                start_account: last_account,
                delta: 1,
                qualified,
                completed_depth: depth,
            });
            NextPendingUpdateId::<T>::put(update_id.saturating_add(1));
        }
    }

    /// 递减团队人数（会员移除时向上递归更新）
    /// 方案 D: 有界递归 + 溢出异步补偿
    pub(crate) fn decrement_team_size_by_entity(entity_id: u64, account: &T::AccountId, qualified: bool) {
        use alloc::collections::BTreeSet;

        let mut current = Some(account.clone());
        let mut depth = 0u32;
        let mut visited = BTreeSet::new();
        let mut last_account = account.clone();

        while let Some(ref curr_account) = current {
            if depth >= MAX_SYNC_DEPTH {
                break;
            }

            if visited.contains(curr_account) {
                break;
            }

            visited.insert(curr_account.clone());
            last_account = curr_account.clone();

            EntityMembers::<T>::mutate(entity_id, curr_account, |maybe_member| {
                if let Some(ref mut m) = maybe_member {
                    m.team_size = m.team_size.saturating_sub(1);
                    if depth >= 1 {
                        m.indirect_referrals = m.indirect_referrals.saturating_sub(1);
                        if qualified {
                            m.qualified_indirect_referrals = m.qualified_indirect_referrals.saturating_sub(1);
                        }
                    }
                }
            });

            current = EntityMembers::<T>::get(entity_id, curr_account)
                .and_then(|m| m.referrer);
            depth += 1;
        }

        // 溢出部分写入异步补偿队列
        if current.is_some() && depth >= MAX_SYNC_DEPTH {
            let update_id = NextPendingUpdateId::<T>::get();
            PendingTeamSizeUpdates::<T>::insert(update_id, TeamSizeUpdate {
                entity_id,
                start_account: last_account,
                delta: -1,
                qualified,
                completed_depth: depth,
            });
            NextPendingUpdateId::<T>::put(update_id.saturating_add(1));
        }
    }
}
