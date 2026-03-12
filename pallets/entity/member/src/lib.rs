//! # Entity 会员管理模块 (pallet-entity-member)
//!
//! ## 概述
//!
//! 本模块实现店铺会员推荐关系管理，支持：
//! - 每个店铺独立的会员体系
//! - 三级分销推荐返佣
//! - 会员等级管理
//! - 推荐统计查询
//!
//! ## 版本历史
//!
//! - v0.1.0: 初始版本，实现基础会员推荐关系

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub mod runtime_api;
pub use runtime_api::*;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

// ============================================================================
// KYC 检查接口
// ============================================================================

/// Entity 级别的 KYC 检查接口
///
/// 用于 `KYC_REQUIRED`（注册）和 `KYC_UPGRADE_REQUIRED`（升级）策略。
/// Runtime 通过 `pallet-entity-kyc::can_participate_in_entity` 桥接实现。
pub trait KycChecker<AccountId> {
    /// 检查账户是否通过了指定 Entity 的 KYC 要求
    fn is_kyc_passed(entity_id: u64, account: &AccountId) -> bool;
}

/// 默认空实现（无 KYC 系统时使用，所有账户均通过）
impl<AccountId> KycChecker<AccountId> for () {
    fn is_kyc_passed(_entity_id: u64, _account: &AccountId) -> bool {
        true
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityProvider, ShopProvider, MemberRegistrationPolicy, MemberStatsPolicy, AdminPermission};
    use sp_runtime::traits::{Saturating, Zero};

    // ============================================================================
    // 数据结构
    // ============================================================================

    /// 等级升级方式
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum LevelUpgradeMode {
        #[default]
        AutoUpgrade,      // 自动升级（消费达标即升）
        ManualUpgrade,    // 手动升级（需店主审批）
    }

    /// 自定义会员等级
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct CustomLevel {
        /// 等级 ID（0, 1, 2, ...）
        pub id: u8,
        /// 等级名称（如 "VIP", "黑卡"）
        pub name: BoundedVec<u8, ConstU32<32>>,
        /// 升级阈值（USDT 累计消费，精度 10^6）
        pub threshold: u64,
        /// 折扣率（基点，500 = 5% 折扣）
        pub discount_rate: u16,
        /// 返佣加成（基点，100 = 1% 额外返佣）
        pub commission_bonus: u16,
    }

    /// 实体会员等级系统配置
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxLevels))]
    pub struct EntityLevelSystem<MaxLevels: Get<u32>> {
        /// 自定义等级列表（按阈值升序排列）
        pub levels: BoundedVec<CustomLevel, MaxLevels>,
        /// 是否启用自定义等级（false 则使用全局默认）
        pub use_custom: bool,
        /// 等级升级方式
        pub upgrade_mode: LevelUpgradeMode,
    }

    impl<MaxLevels: Get<u32>> Default for EntityLevelSystem<MaxLevels> {
        fn default() -> Self {
            Self {
                levels: BoundedVec::default(),
                use_custom: false,
                upgrade_mode: LevelUpgradeMode::AutoUpgrade,
            }
        }
    }

    /// 实体等级系统类型别名
    pub type EntityLevelSystemOf<T> = EntityLevelSystem<<T as Config>::MaxCustomLevels>;

    // ============================================================================
    // 升级规则相关数据结构
    // ============================================================================

    /// 规则冲突处理策略
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
    pub enum ConflictStrategy {
        #[default]
        HighestLevel,     // 取最高等级
        HighestPriority,  // 取最高优先级规则
        LongestDuration,  // 取最长有效期
        FirstMatch,       // 第一个匹配的规则
    }

    /// 升级触发条件
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub enum UpgradeTrigger {
        /// 购买特定产品
        PurchaseProduct {
            product_id: u64,
        },
        /// 累计消费达标（USDT 精度 10^6）
        TotalSpent {
            threshold: u64,
        },
        /// 单笔消费达标（USDT 精度 10^6）
        SingleOrder {
            threshold: u64,
        },
        /// 推荐人数达标
        ReferralCount {
            count: u32,
        },
        /// 团队人数达标
        TeamSize {
            size: u32,
        },
        /// 订单数量达标
        OrderCount {
            count: u32,
        },
    }

    /// 升级规则
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct UpgradeRule<BlockNumber> {
        /// 规则 ID
        pub id: u32,
        /// 规则名称
        pub name: BoundedVec<u8, ConstU32<64>>,
        /// 触发条件
        pub trigger: UpgradeTrigger,
        /// 目标等级 ID
        pub target_level_id: u8,
        /// 有效期（区块数，None 表示永久）
        pub duration: Option<BlockNumber>,
        /// 是否启用
        pub enabled: bool,
        /// 优先级（数值越大优先级越高）
        pub priority: u8,
        /// 是否可叠加（多次触发是否延长有效期）
        pub stackable: bool,
        /// 最大触发次数（None 表示无限制）
        pub max_triggers: Option<u32>,
        /// 已触发次数
        pub trigger_count: u32,
    }

    /// 升级规则类型别名
    pub type UpgradeRuleOf<T> = UpgradeRule<BlockNumberFor<T>>;

    /// 实体升级规则系统
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    #[scale_info(skip_type_params(MaxRules))]
    pub struct EntityUpgradeRuleSystem<BlockNumber, MaxRules: Get<u32>> {
        /// 升级规则列表
        pub rules: BoundedVec<UpgradeRule<BlockNumber>, MaxRules>,
        /// 下一个规则 ID
        pub next_rule_id: u32,
        /// 是否启用规则系统
        pub enabled: bool,
        /// 规则冲突处理策略
        pub conflict_strategy: ConflictStrategy,
    }

    impl<BlockNumber, MaxRules: Get<u32>> Default for EntityUpgradeRuleSystem<BlockNumber, MaxRules> {
        fn default() -> Self {
            Self {
                rules: BoundedVec::default(),
                next_rule_id: 0,
                enabled: false,
                conflict_strategy: ConflictStrategy::HighestLevel,
            }
        }
    }

    /// 实体升级规则系统类型别名
    pub type EntityUpgradeRuleSystemOf<T> = EntityUpgradeRuleSystem<BlockNumberFor<T>, <T as Config>::MaxUpgradeRules>;

    /// 升级记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct UpgradeRecord<BlockNumber> {
        /// 触发的规则 ID
        pub rule_id: u32,
        /// 升级前等级
        pub from_level_id: u8,
        /// 升级后等级
        pub to_level_id: u8,
        /// 升级时间
        pub upgraded_at: BlockNumber,
        /// 过期时间
        pub expires_at: Option<BlockNumber>,
    }

    /// 升级记录类型别名
    pub type UpgradeRecordOf<T> = UpgradeRecord<BlockNumberFor<T>>;

    /// 实体会员信息
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct EntityMember<AccountId, BlockNumber> {
        /// 推荐人（上级）
        pub referrer: Option<AccountId>,
        /// 直接推荐人数（含所有来源：主动注册 + 复购赠与）
        pub direct_referrals: u32,
        /// 有效直推人数（仅含主动注册/购买触发，不含复购赠与注册）
        pub qualified_referrals: u32,
        /// 间接推荐人数（含所有来源）
        pub indirect_referrals: u32,
        /// 有效间接推荐人数（不含复购赠与注册）
        pub qualified_indirect_referrals: u32,
        /// 团队总人数
        pub team_size: u32,
        /// 累计消费金额（USDT 精度 10^6）
        pub total_spent: u64,
        /// 自定义等级 ID（店铺自定义体系，0 表示最低级）
        pub custom_level_id: u8,
        /// 加入时间
        pub joined_at: BlockNumber,
        /// 最后活跃时间（消费时更新，可用于活跃度查询）
        pub last_active_at: BlockNumber,
        /// 激活状态：首次消费达标后为 true（与封禁独立）
        pub activated: bool,
        /// 推荐来源标记：注册时是否为有效直推（主动注册/购买=true，复购赠与=false）
        pub is_qualified_referral: bool,
        /// 封禁状态：None=正常，Some(block)=封禁时间
        pub banned_at: Option<BlockNumber>,
        /// A2: 封禁原因（管理员填写，可选）
        pub ban_reason: Option<BoundedVec<u8, ConstU32<128>>>,
    }

    /// 实体会员类型别名
    pub type EntityMemberOf<T> = EntityMember<
        <T as frame_system::Config>::AccountId,
        BlockNumberFor<T>,
    >;

    // ============================================================================
    // Pallet 配置
    // ============================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// 实体查询接口
        type EntityProvider: EntityProvider<Self::AccountId>;

        /// Shop 查询接口（Entity-Shop 分离架构）
        type ShopProvider: pallet_entity_common::ShopProvider<Self::AccountId>;

        /// 最大直接推荐人数
        #[pallet::constant]
        type MaxDirectReferrals: Get<u32>;

        /// 最大自定义等级数量
        #[pallet::constant]
        type MaxCustomLevels: Get<u32>;

        /// 最大升级规则数量
        #[pallet::constant]
        type MaxUpgradeRules: Get<u32>;

        /// 最大升级历史记录数量
        #[pallet::constant]
        type MaxUpgradeHistory: Get<u32>;

        /// M6 修复: 待审批会员过期区块数（0 = 不过期）
        #[pallet::constant]
        type PendingMemberExpiry: Get<BlockNumberFor<Self>>;

        /// KYC 检查接口（用于 KYC_REQUIRED / KYC_UPGRADE_REQUIRED 策略）
        type KycChecker: KycChecker<Self::AccountId>;

        /// P2-14: 会员移除回调（通知佣金插件清理 per-user 存储）
        type OnMemberRemoved: pallet_entity_common::OnMemberRemoved<Self::AccountId>;

        /// Weight information for extrinsics
        type WeightInfo: WeightInfo;
    }

    /// 同步递归更新 team_size 的最大深度。
    /// 超过此深度的祖先通过 on_idle 异步补偿。
    pub(crate) const MAX_SYNC_DEPTH: u32 = 15;

    /// team_size 异步补偿记录
    #[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
    pub struct TeamSizeUpdate<AccountId> {
        /// 实体 ID
        pub entity_id: u64,
        /// 从此账户继续向上递归（此账户本身已更新，从其 referrer 开始）
        pub start_account: AccountId,
        /// +1（注册）或 -1（移除）
        pub delta: i8,
        /// 是否为有效直推
        pub qualified: bool,
        /// 已完成的同步深度（用于 indirect_referrals 判断）
        pub completed_depth: u32,
    }

    /// team_size 异步补偿记录类型别名
    pub type TeamSizeUpdateOf<T> = TeamSizeUpdate<<T as frame_system::Config>::AccountId>;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// A4: on_idle 自动清理过期待审批会员 + 方案 D team_size 异步补偿
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let mut weight_used = Weight::zero();

            // ── Phase 1: 过期待审批会员清理 ──
            let expiry = T::PendingMemberExpiry::get();
            if !expiry.is_zero() {
                let per_scan_weight = Weight::from_parts(10_000_000, 1_000);
                let per_remove_weight = Weight::from_parts(50_000_000, 5_000);
                let max_scan = 50u32;
                let max_clean = 10u32;
                let min_weight = per_scan_weight.saturating_mul(2);

                if !remaining_weight.any_lt(min_weight) {
                    let now = <frame_system::Pallet<T>>::block_number();
                    let mut scanned = 0u32;
                    let mut cleaned = 0u32;

                    let mut to_remove: alloc::vec::Vec<(u64, T::AccountId)> = alloc::vec::Vec::new();
                    for (entity_id, account, (_referrer, applied_at)) in PendingMembers::<T>::iter() {
                        scanned += 1;
                        weight_used = weight_used.saturating_add(per_scan_weight);
                        if scanned >= max_scan || cleaned >= max_clean {
                            break;
                        }
                        if remaining_weight.any_lt(weight_used.saturating_add(per_remove_weight)) {
                            break;
                        }
                        if now > applied_at.saturating_add(expiry) {
                            to_remove.push((entity_id, account));
                            cleaned += 1;
                        }
                    }

                    for (entity_id, account) in to_remove.iter() {
                        PendingMembers::<T>::remove(entity_id, account);
                        Self::deposit_event(Event::PendingMemberExpired {
                            entity_id: *entity_id,
                            account: account.clone(),
                        });
                        weight_used = weight_used.saturating_add(per_remove_weight);
                    }
                }
            }

            // ── Phase 2: team_size 异步补偿（方案 D）──
            // 每步：1 次 storage read + 1 次 storage write
            let per_step_weight = Weight::from_parts(30_000_000, 3_000);
            // 读取游标 + 读取记录 + 可能写回/删除
            let overhead_weight = Weight::from_parts(20_000_000, 2_000);

            let processed_id = ProcessedPendingUpdateId::<T>::get();
            let next_id = NextPendingUpdateId::<T>::get();

            if processed_id < next_id {
                weight_used = weight_used.saturating_add(overhead_weight);
                if !remaining_weight.any_lt(weight_used) {
                    if let Some(mut update) = PendingTeamSizeUpdates::<T>::get(processed_id) {
                        use alloc::collections::BTreeSet;
                        let mut visited = BTreeSet::new();
                        // 从 start_account 的 referrer 开始（start_account 本身已在同步阶段处理）
                        let mut current = EntityMembers::<T>::get(update.entity_id, &update.start_account)
                            .and_then(|m| m.referrer);
                        let mut depth = update.completed_depth;
                        let batch_limit = depth.saturating_add(MAX_SYNC_DEPTH);
                        let mut last_account = update.start_account.clone();

                        while let Some(ref curr_account) = current {
                            if depth >= batch_limit {
                                break;
                            }
                            if visited.contains(curr_account) {
                                // 循环检测，终止
                                current = None;
                                break;
                            }
                            let step_total = weight_used.saturating_add(per_step_weight);
                            if remaining_weight.any_lt(step_total) {
                                break;
                            }
                            visited.insert(curr_account.clone());
                            last_account = curr_account.clone();

                            EntityMembers::<T>::mutate(update.entity_id, curr_account, |maybe_member| {
                                if let Some(ref mut m) = maybe_member {
                                    if update.delta > 0 {
                                        m.team_size = m.team_size.saturating_add(1);
                                    } else {
                                        m.team_size = m.team_size.saturating_sub(1);
                                    }
                                    // depth >= 1 始终成立（异步阶段 depth 从 completed_depth 开始，>= MAX_SYNC_DEPTH >= 15）
                                    if update.delta > 0 {
                                        m.indirect_referrals = m.indirect_referrals.saturating_add(1);
                                        if update.qualified {
                                            m.qualified_indirect_referrals = m.qualified_indirect_referrals.saturating_add(1);
                                        }
                                    } else {
                                        m.indirect_referrals = m.indirect_referrals.saturating_sub(1);
                                        if update.qualified {
                                            m.qualified_indirect_referrals = m.qualified_indirect_referrals.saturating_sub(1);
                                        }
                                    }
                                }
                            });

                            weight_used = weight_used.saturating_add(per_step_weight);
                            current = EntityMembers::<T>::get(update.entity_id, curr_account)
                                .and_then(|m| m.referrer);
                            depth += 1;
                        }

                        if current.is_none() {
                            // 完成：无更多祖先或循环终止
                            PendingTeamSizeUpdates::<T>::remove(processed_id);
                            ProcessedPendingUpdateId::<T>::put(processed_id.saturating_add(1));
                            Self::deposit_event(Event::TeamSizeAsyncCompensationCompleted {
                                update_id: processed_id,
                                entity_id: update.entity_id,
                                total_depth: depth,
                            });
                        } else {
                            // 未完成：更新游标，下个 block 继续
                            update.start_account = last_account;
                            update.completed_depth = depth;
                            PendingTeamSizeUpdates::<T>::insert(processed_id, update);
                        }
                    } else {
                        // 记录不存在（不应发生），跳过
                        ProcessedPendingUpdateId::<T>::put(processed_id.saturating_add(1));
                    }
                }
            }

            weight_used
        }
    }

    // ============================================================================
    // 存储项
    // ============================================================================

    /// 实体会员存储 (entity_id, account) -> EntityMember
    #[pallet::storage]
    #[pallet::getter(fn entity_members)]
    pub type EntityMembers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        EntityMemberOf<T>,
    >;

    /// 实体会员数量 entity_id -> count
    #[pallet::storage]
    #[pallet::getter(fn member_count)]
    pub type MemberCount<T: Config> = StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;

    /// 每个等级的会员数量 (entity_id, level_id) -> count
    /// 用于沉淀池奖励 v2 等额分配计算
    #[pallet::storage]
    #[pallet::getter(fn level_member_count)]
    pub type LevelMemberCount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, u8,
        u32,
        ValueQuery,
    >;

    /// 推荐关系索引 (entity_id, referrer) -> Vec<AccountId>
    #[pallet::storage]
    #[pallet::getter(fn direct_referrals)]
    pub type DirectReferrals<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BoundedVec<T::AccountId, T::MaxDirectReferrals>,
        ValueQuery,
    >;

    /// 实体等级系统配置 entity_id -> EntityLevelSystem
    #[pallet::storage]
    #[pallet::getter(fn entity_level_system)]
    pub type EntityLevelSystems<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        EntityLevelSystemOf<T>,
    >;

    /// 实体升级规则系统 entity_id -> EntityUpgradeRuleSystem
    #[pallet::storage]
    #[pallet::getter(fn entity_upgrade_rules)]
    pub type EntityUpgradeRules<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        EntityUpgradeRuleSystemOf<T>,
    >;

    /// 会员等级过期时间 (entity_id, account) -> expires_at
    #[pallet::storage]
    #[pallet::getter(fn member_level_expiry)]
    pub type MemberLevelExpiry<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BlockNumberFor<T>,
    >;

    /// 会员升级历史 (entity_id, account) -> Vec<UpgradeRecord>
    #[pallet::storage]
    #[pallet::getter(fn member_upgrade_history)]
    pub type MemberUpgradeHistory<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        BoundedVec<UpgradeRecordOf<T>, T::MaxUpgradeHistory>,
        ValueQuery,
    >;

    /// 会员订单数量 (entity_id, account) -> order_count
    #[pallet::storage]
    #[pallet::getter(fn member_order_count)]
    pub type MemberOrderCount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        u32,
        ValueQuery,
    >;

    /// 会员注册策略 entity_id -> MemberRegistrationPolicy
    #[pallet::storage]
    #[pallet::getter(fn entity_member_policy)]
    pub type EntityMemberPolicy<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        MemberRegistrationPolicy,
        ValueQuery,
    >;

    /// 会员统计策略 entity_id -> MemberStatsPolicy
    #[pallet::storage]
    #[pallet::getter(fn entity_member_stats_policy)]
    pub type EntityMemberStatsPolicy<T: Config> = StorageMap<
        _,
        Blake2_128Concat, u64,
        MemberStatsPolicy,
        ValueQuery,
    >;

    /// M6 修复: 待审批会员 (entity_id, account) -> (referrer, applied_at)
    #[pallet::storage]
    pub type PendingMembers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat, u64,
        Blake2_128Concat, T::AccountId,
        (Option<T::AccountId>, BlockNumberFor<T>),
    >;

    /// S6: 封禁会员计数器 entity_id -> count
    #[pallet::storage]
    pub type BannedMemberCount<T: Config> = StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;

    // ============================================================================
    // team_size 异步补偿存储（方案 D: 有界递归 + 溢出异步补偿）
    // ============================================================================

    /// 待处理的 team_size 异步补偿记录 update_id -> TeamSizeUpdate
    #[pallet::storage]
    pub type PendingTeamSizeUpdates<T: Config> = StorageMap<
        _, Blake2_128Concat, u32, TeamSizeUpdateOf<T>,
    >;

    /// 下一个待分配的异步补偿 ID（自增）
    #[pallet::storage]
    pub type NextPendingUpdateId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// 已处理到的异步补偿 ID 游标
    #[pallet::storage]
    pub type ProcessedPendingUpdateId<T: Config> = StorageValue<_, u32, ValueQuery>;


    // ============================================================================
    // 事件
    // ============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 会员注册
        MemberRegistered {
            entity_id: u64,
            shop_id: Option<u64>,
            account: T::AccountId,
            referrer: Option<T::AccountId>,
        },
        /// 绑定推荐人
        ReferrerBound {
            shop_id: u64,
            account: T::AccountId,
            referrer: T::AccountId,
        },
        /// 自定义等级升级
        CustomLevelUpgraded {
            entity_id: u64,
            account: T::AccountId,
            old_level_id: u8,
            new_level_id: u8,
        },
        /// 等级系统初始化
        LevelSystemInitialized {
            shop_id: u64,
            use_custom: bool,
            upgrade_mode: LevelUpgradeMode,
        },
        /// 自定义等级添加
        CustomLevelAdded {
            shop_id: u64,
            level_id: u8,
            name: BoundedVec<u8, ConstU32<32>>,
            threshold: u64,
        },
        /// 自定义等级更新
        CustomLevelUpdated {
            shop_id: u64,
            level_id: u8,
        },
        /// 自定义等级删除
        CustomLevelRemoved {
            shop_id: u64,
            level_id: u8,
        },
        /// 升级规则系统初始化
        UpgradeRuleSystemInitialized {
            shop_id: u64,
            conflict_strategy: ConflictStrategy,
        },
        /// 升级规则添加
        UpgradeRuleAdded {
            shop_id: u64,
            rule_id: u32,
            name: BoundedVec<u8, ConstU32<64>>,
            target_level_id: u8,
        },
        /// 升级规则更新
        UpgradeRuleUpdated {
            shop_id: u64,
            rule_id: u32,
        },
        /// 升级规则删除
        UpgradeRuleRemoved {
            shop_id: u64,
            rule_id: u32,
        },
        /// 会员通过规则升级
        MemberUpgradedByRule {
            entity_id: u64,
            account: T::AccountId,
            rule_id: u32,
            from_level_id: u8,
            to_level_id: u8,
            expires_at: Option<BlockNumberFor<T>>,
        },
        /// 会员等级过期
        MemberLevelExpired {
            entity_id: u64,
            account: T::AccountId,
            expired_level_id: u8,
            new_level_id: u8,
        },
        /// 会员注册策略更新
        MemberPolicyUpdated {
            entity_id: u64,
            policy: MemberRegistrationPolicy,
        },
        /// 会员统计策略更新
        MemberStatsPolicyUpdated {
            entity_id: u64,
            policy: MemberStatsPolicy,
        },
        /// 会员待审批（APPROVAL_REQUIRED 模式）
        MemberPendingApproval {
            entity_id: u64,
            account: T::AccountId,
            referrer: Option<T::AccountId>,
        },
        /// 会员审批通过
        MemberApproved {
            entity_id: u64,
            shop_id: u64,
            account: T::AccountId,
        },
        /// 会员审批拒绝
        MemberRejected {
            entity_id: u64,
            account: T::AccountId,
        },
        /// R6-L1: 自定义等级开关变更
        UseCustomLevelsUpdated {
            shop_id: u64,
            use_custom: bool,
        },
        /// R6-L1: 升级模式变更
        UpgradeModeUpdated {
            shop_id: u64,
            upgrade_mode: LevelUpgradeMode,
        },
        /// R6-L1: 升级规则系统启用/禁用
        UpgradeRuleSystemToggled {
            shop_id: u64,
            enabled: bool,
        },
        /// R6-L1: 冲突策略变更
        ConflictStrategyUpdated {
            shop_id: u64,
            strategy: ConflictStrategy,
        },
        /// M6: 待审批会员撤回申请
        PendingMemberCancelled {
            entity_id: u64,
            account: T::AccountId,
        },
        /// M6: 待审批会员过期清理
        PendingMemberExpired {
            entity_id: u64,
            account: T::AccountId,
        },
        /// 会员被封禁
        MemberBanned {
            entity_id: u64,
            account: T::AccountId,
            /// A2: 封禁原因
            reason: Option<BoundedVec<u8, ConstU32<128>>>,
        },
        /// 会员被解封
        MemberUnbanned {
            entity_id: u64,
            account: T::AccountId,
        },
        /// 会员已激活（首次消费达标或管理员手动激活）
        MemberActivated {
            entity_id: u64,
            account: T::AccountId,
        },
        /// 会员已取消激活（管理员手动操作）
        MemberDeactivated {
            entity_id: u64,
            account: T::AccountId,
        },
        /// 批量审批通过
        BatchMembersApproved {
            entity_id: u64,
            count: u32,
        },
        /// 批量审批拒绝
        BatchMembersRejected {
            entity_id: u64,
            count: u32,
        },
        /// 会员被移除
        MemberRemoved {
            entity_id: u64,
            account: T::AccountId,
        },
        /// 手动设置会员等级（支持升降级）
        MemberLevelSet {
            entity_id: u64,
            account: T::AccountId,
            old_level_id: u8,
            new_level_id: u8,
        },
        /// 等级系统已重置
        LevelSystemReset {
            entity_id: u64,
        },
        /// 升级规则系统已重置
        UpgradeRuleSystemReset {
            entity_id: u64,
        },
        /// U1: 会员主动退出
        MemberLeft {
            entity_id: u64,
            account: T::AccountId,
        },
        /// G1: 治理路径更新注册策略
        GovernanceMemberPolicyUpdated {
            entity_id: u64,
            policy: MemberRegistrationPolicy,
        },
        /// G1: 治理路径更新统计策略
        GovernanceStatsPolicyUpdated {
            entity_id: u64,
            policy: MemberStatsPolicy,
        },
        /// G1: 治理路径切换升级规则系统开关
        GovernanceUpgradeRuleSystemToggled {
            entity_id: u64,
            enabled: bool,
        },
        /// 方案 D: team_size 异步补偿完成
        TeamSizeAsyncCompensationCompleted {
            update_id: u32,
            entity_id: u64,
            total_depth: u32,
        },
    }

    // ============================================================================
    // 错误
    // ============================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// 已是会员
        AlreadyMember,
        /// 不是会员
        NotMember,
        /// 已绑定推荐人
        ReferrerAlreadyBound,
        /// 无效推荐人
        InvalidReferrer,
        /// 不能推荐自己
        SelfReferral,
        /// 循环推荐
        CircularReferral,
        /// 店铺不存在
        ShopNotFound,
        /// 推荐人数已满
        ReferralsFull,
        /// 数值溢出
        Overflow,
        /// 等级系统未初始化
        LevelSystemNotInitialized,
        /// 等级不存在
        LevelNotFound,
        /// 等级数量已满
        LevelsFull,
        /// 无效等级 ID
        InvalidLevelId,
        /// 等级阈值无效（必须大于前一等级）
        InvalidThreshold,
        /// 等级名称为空
        EmptyLevelName,
        /// 不支持手动升级
        ManualUpgradeNotSupported,
        /// 等级有会员，无法删除
        LevelHasMembers,
        /// 升级规则系统未初始化
        UpgradeRuleSystemNotInitialized,
        /// 升级规则不存在
        UpgradeRuleNotFound,
        /// 升级规则数量已满
        UpgradeRulesFull,
        /// 规则名称为空
        EmptyRuleName,
        /// 无效目标等级
        InvalidTargetLevel,
        /// 需要先消费才能注册（PURCHASE_REQUIRED 策略）
        PurchaseRequiredForRegistration,
        /// 需要提供推荐人（REFERRAL_REQUIRED 策略）
        ReferralRequiredForRegistration,
        /// 会员待审批中（APPROVAL_REQUIRED 策略）
        MemberPendingApproval,
        /// 未找到待审批记录
        PendingMemberNotFound,
        /// 不是 Entity 管理员
        NotEntityAdmin,
        /// 基点值超出范围（最大 10000 = 100%）
        InvalidBasisPoints,
        /// 无效策略位标记
        InvalidPolicyBits,
        /// 无效升级模式值
        InvalidUpgradeMode,
        /// 等级系统已初始化（防止覆盖已有等级数据）
        LevelSystemAlreadyInitialized,
        /// 升级规则系统已初始化（防止覆盖已有规则）
        UpgradeRuleSystemAlreadyInitialized,
        /// 等级名称过长（超过 32 字节）
        NameTooLong,
        /// 规则 ID 溢出
        RuleIdOverflow,
        /// M6: 待审批记录已过期
        PendingMemberAlreadyExpired,
        /// 会员已被封禁
        MemberAlreadyBanned,
        /// 会员未被封禁
        MemberNotBanned,
        /// 会员已被封禁（操作被拒绝）
        MemberIsBanned,
        /// 批量操作超过上限
        BatchLimitExceeded,
        /// 未通过 KYC 认证（注册时 KYC_REQUIRED 策略）
        KycNotPassed,
        /// 未通过 KYC 认证（升级时 KYC_UPGRADE_REQUIRED 策略）
        KycRequiredForUpgrade,
        /// 会员有下线，无法移除
        MemberHasDownlines,
        /// 等级系统有非零等级会员，无法重置
        LevelSystemHasNonZeroMembers,
        /// 实体已被全局锁定，所有配置操作不可用
        EntityLocked,
        /// U1: 会员不属于该实体（或已被封禁无法主动退出）
        CannotLeave,
        /// 会员已激活
        AlreadyActivated,
        /// 会员未激活
        NotActivated,
        /// 推荐人自身未绑定上级（REFERRAL_REQUIRED 策略下不允许作为推荐人）
        ReferrerNotBound,
    }

    // ============================================================================
    // Extrinsics
    // ============================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 注册成为店铺会员
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `referrer`: 推荐人（可选）
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_member())]
        pub fn register_member(
            origin: OriginFor<T>,
            shop_id: u64,
            referrer: Option<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = Self::resolve_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 检查是否已注册（会员统一在 Entity 级别）
            ensure!(!EntityMembers::<T>::contains_key(entity_id, &who), Error::<T>::AlreadyMember);

            // ---- 注册策略检查 ----
            let policy = EntityMemberPolicy::<T>::get(entity_id);

            // PURCHASE_REQUIRED: 手动注册被拒（必须通过 auto_register 即下单触发）
            ensure!(!policy.requires_purchase(), Error::<T>::PurchaseRequiredForRegistration);

            // KYC_REQUIRED: 注册时需要通过 KYC
            if policy.requires_kyc() {
                ensure!(T::KycChecker::is_kyc_passed(entity_id, &who), Error::<T>::KycNotPassed);
            }

            // REFERRAL_REQUIRED: 必须提供推荐人
            if policy.requires_referral() {
                ensure!(referrer.is_some(), Error::<T>::ReferralRequiredForRegistration);
            }

            // 验证推荐人
            if let Some(ref ref_account) = referrer {
                ensure!(ref_account != &who, Error::<T>::SelfReferral);
                ensure!(
                    EntityMembers::<T>::contains_key(entity_id, ref_account),
                    Error::<T>::InvalidReferrer
                );
            }

            // APPROVAL_REQUIRED: 进入待审批状态
            if policy.requires_approval() {
                // 检查是否已在待审批列表
                ensure!(
                    !PendingMembers::<T>::contains_key(entity_id, &who),
                    Error::<T>::MemberPendingApproval
                );

                let now = <frame_system::Pallet<T>>::block_number();
                PendingMembers::<T>::insert(entity_id, &who, (referrer.clone(), now));

                Self::deposit_event(Event::MemberPendingApproval {
                    entity_id,
                    account: who,
                    referrer,
                });

                return Ok(());
            }

            // 正常注册
            Self::do_register_member(entity_id, &who, referrer.clone(), true)?;

            Self::deposit_event(Event::MemberRegistered {
                entity_id,
                shop_id: Some(shop_id),
                account: who,
                referrer,
            });

            Ok(())
        }

        /// 绑定推荐人（未绑定过的会员）
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `referrer`: 推荐人账户
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::bind_referrer())]
        pub fn bind_referrer(
            origin: OriginFor<T>,
            shop_id: u64,
            referrer: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = Self::resolve_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 验证是会员
            let member = Self::get_member_by_shop(shop_id, &who)
                .ok_or(Error::<T>::NotMember)?;

            // 验证未绑定推荐人
            ensure!(member.referrer.is_none(), Error::<T>::ReferrerAlreadyBound);

            // 验证推荐人
            ensure!(referrer != who, Error::<T>::SelfReferral);
            ensure!(
                Self::is_member_of_shop(shop_id, &referrer),
                Error::<T>::InvalidReferrer
            );

            // 检查循环推荐
            ensure!(
                !Self::is_circular_referral(shop_id, &who, &referrer),
                Error::<T>::CircularReferral
            );

            // REFERRAL_REQUIRED: 推荐人自身必须已绑定上级（owner 豁免）
            Self::ensure_referrer_bound(entity_id, &referrer)?;

            // 绑定推荐人
            EntityMembers::<T>::mutate(entity_id, &who, |maybe_member| {
                if let Some(ref mut m) = maybe_member {
                    m.referrer = Some(referrer.clone());
                }
            });

            // R6-M1 审计修复: 复用 mutate_member_referral 统一维护推荐人统计
            // 确保 DirectReferrals 容量检查在统计写入前执行（fail-fast），
            // 并与 do_register_member 保持一致的更新顺序
            Self::mutate_member_referral(entity_id, &referrer, &who, true)?;

            Self::deposit_event(Event::ReferrerBound {
                shop_id,
                account: who,
                referrer,
            });

            Ok(())
        }

        // call_index 2-3: reserved (历史预留，未来可用)

        /// 初始化店铺等级系统
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `use_custom`: 是否使用自定义等级
        /// - `upgrade_mode`: 等级升级方式
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::init_level_system())]
        pub fn init_level_system(
            origin: OriginFor<T>,
            shop_id: u64,
            use_custom: bool,
            upgrade_mode: LevelUpgradeMode,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // H3 审计修复: 防止覆盖已有等级系统（已有自定义等级的数据会丢失）
            ensure!(
                !EntityLevelSystems::<T>::contains_key(entity_id),
                Error::<T>::LevelSystemAlreadyInitialized
            );

            let system = EntityLevelSystem {
                levels: BoundedVec::default(),
                use_custom,
                upgrade_mode,
            };

            EntityLevelSystems::<T>::insert(entity_id, system);

            Self::deposit_event(Event::LevelSystemInitialized {
                shop_id,
                use_custom,
                upgrade_mode,
            });

            Ok(())
        }

        /// 添加自定义等级
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `name`: 等级名称
        /// - `threshold`: 升级阈值
        /// - `discount_rate`: 折扣率（基点）
        /// - `commission_bonus`: 返佣加成（基点）
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::add_custom_level())]
        pub fn add_custom_level(
            origin: OriginFor<T>,
            shop_id: u64,
            name: BoundedVec<u8, ConstU32<32>>,
            threshold: u64,
            discount_rate: u16,
            commission_bonus: u16,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            ensure!(!name.is_empty(), Error::<T>::EmptyLevelName);
            ensure!(discount_rate <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(commission_bonus <= 10000, Error::<T>::InvalidBasisPoints);

            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;

                // 验证阈值必须大于最后一个等级
                if let Some(last) = system.levels.last() {
                    ensure!(threshold > last.threshold, Error::<T>::InvalidThreshold);
                }

                let level_id = (system.levels.len() + 1) as u8;

                let level = CustomLevel {
                    id: level_id,
                    name: name.clone(),
                    threshold,
                    discount_rate,
                    commission_bonus,
                };

                system.levels.try_push(level).map_err(|_| Error::<T>::LevelsFull)?;

                Self::deposit_event(Event::CustomLevelAdded {
                    shop_id,
                    level_id,
                    name,
                    threshold,
                });

                Ok(())
            })
        }

        /// 更新自定义等级
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `level_id`: 等级 ID
        /// - `name`: 新名称（可选）
        /// - `threshold`: 新阈值（可选）
        /// - `discount_rate`: 新折扣率（可选）
        /// - `commission_bonus`: 新返佣加成（可选）
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::update_custom_level())]
        pub fn update_custom_level(
            origin: OriginFor<T>,
            shop_id: u64,
            level_id: u8,
            name: Option<BoundedVec<u8, ConstU32<32>>>,
            threshold: Option<u64>,
            discount_rate: Option<u16>,
            commission_bonus: Option<u16>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;

                ensure!(level_id > 0 && ((level_id - 1) as usize) < system.levels.len(), Error::<T>::LevelNotFound);

                // 验证阈值（先检查，再修改）
                if let Some(new_threshold) = threshold {
                    // 必须大于前一等级
                    if level_id > 1 {
                        if let Some(prev) = system.levels.get((level_id - 2) as usize) {
                            ensure!(new_threshold > prev.threshold, Error::<T>::InvalidThreshold);
                        }
                    }
                    // 必须小于后一等级
                    if let Some(next) = system.levels.get(level_id as usize) {
                        ensure!(new_threshold < next.threshold, Error::<T>::InvalidThreshold);
                    }
                }

                // 现在安全地获取可变引用并修改
                let level = system.levels.get_mut((level_id - 1) as usize)
                    .ok_or(Error::<T>::LevelNotFound)?;

                if let Some(new_threshold) = threshold {
                    level.threshold = new_threshold;
                }

                if let Some(new_name) = name {
                    ensure!(!new_name.is_empty(), Error::<T>::EmptyLevelName);
                    level.name = new_name;
                }

                if let Some(rate) = discount_rate {
                    ensure!(rate <= 10000, Error::<T>::InvalidBasisPoints);
                    level.discount_rate = rate;
                }

                if let Some(bonus) = commission_bonus {
                    ensure!(bonus <= 10000, Error::<T>::InvalidBasisPoints);
                    level.commission_bonus = bonus;
                }

                Self::deposit_event(Event::CustomLevelUpdated { shop_id, level_id });

                Ok(())
            })
        }

        /// 删除自定义等级（只能删除最后一个等级）
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `level_id`: 等级 ID
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::remove_custom_level())]
        pub fn remove_custom_level(
            origin: OriginFor<T>,
            shop_id: u64,
            level_id: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;

                // 只能删除最后一个等级
                ensure!(
                    level_id > 0 && (level_id - 1) as usize == system.levels.len().saturating_sub(1),
                    Error::<T>::InvalidLevelId
                );

                // H1 审计修复: 检查该等级是否还有会员，防止成员滞留在已删除等级
                ensure!(
                    LevelMemberCount::<T>::get(entity_id, level_id) == 0,
                    Error::<T>::LevelHasMembers
                );

                system.levels.pop();

                Self::deposit_event(Event::CustomLevelRemoved { shop_id, level_id });

                Ok(())
            })
        }

        /// 手动升级会员（仅 ManualUpgrade 模式）
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `member`: 会员账户
        /// - `target_level_id`: 目标等级 ID
        ///
        /// 支持升级和降级。手动设置为永久等级，清除规则升级残留的过期时间。
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::manual_set_member_level())]
        pub fn manual_set_member_level(
            origin: OriginFor<T>,
            shop_id: u64,
            member: T::AccountId,
            target_level_id: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 封禁会员不允许手动调整等级
            let m = EntityMembers::<T>::get(entity_id, &member).ok_or(Error::<T>::NotMember)?;
            ensure!(m.banned_at.is_none(), Error::<T>::MemberIsBanned);

            // KYC_UPGRADE_REQUIRED: 升级时需要通过 KYC（降级不检查）
            if target_level_id > m.custom_level_id {
                let policy = EntityMemberPolicy::<T>::get(entity_id);
                if policy.requires_kyc_for_upgrade() {
                    ensure!(T::KycChecker::is_kyc_passed(entity_id, &member), Error::<T>::KycRequiredForUpgrade);
                }
            }

            let system = EntityLevelSystems::<T>::get(entity_id)
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;

            ensure!(
                system.upgrade_mode == LevelUpgradeMode::ManualUpgrade,
                Error::<T>::ManualUpgradeNotSupported
            );

            ensure!(
                target_level_id == 0 || ((target_level_id - 1) as usize) < system.levels.len(),
                Error::<T>::InvalidLevelId
            );

            EntityMembers::<T>::mutate(entity_id, &member, |maybe_member| -> DispatchResult {
                let m = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;
                let old_level_id = m.custom_level_id;
                m.custom_level_id = target_level_id;

                // 维护 LevelMemberCount
                LevelMemberCount::<T>::mutate(entity_id, old_level_id, |c| *c = c.saturating_sub(1));
                LevelMemberCount::<T>::mutate(entity_id, target_level_id, |c| *c = c.saturating_add(1));

                // 手动设置为永久等级，清除之前规则升级残留的过期时间
                MemberLevelExpiry::<T>::remove(entity_id, &member);

                Self::deposit_event(Event::MemberLevelSet {
                    entity_id,
                    account: member.clone(),
                    old_level_id,
                    new_level_id: target_level_id,
                });

                Ok(())
            })
        }

        /// 切换等级系统模式
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `use_custom`: 是否使用自定义等级
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::set_use_custom_levels())]
        pub fn set_use_custom_levels(
            origin: OriginFor<T>,
            shop_id: u64,
            use_custom: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;
                system.use_custom = use_custom;

                Self::deposit_event(Event::UseCustomLevelsUpdated { shop_id, use_custom });

                Ok(())
            })
        }

        /// 设置等级升级模式
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `upgrade_mode`: 升级模式
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::set_upgrade_mode())]
        pub fn set_upgrade_mode(
            origin: OriginFor<T>,
            shop_id: u64,
            upgrade_mode: LevelUpgradeMode,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;
                system.upgrade_mode = upgrade_mode;

                Self::deposit_event(Event::UpgradeModeUpdated { shop_id, upgrade_mode });

                Ok(())
            })
        }

        // ========================================================================
        // 升级规则相关 Extrinsics
        // ========================================================================

        /// 初始化升级规则系统
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `conflict_strategy`: 规则冲突处理策略
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::init_upgrade_rule_system())]
        pub fn init_upgrade_rule_system(
            origin: OriginFor<T>,
            shop_id: u64,
            conflict_strategy: ConflictStrategy,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // H5 审计修复: 防止覆盖已有升级规则系统
            ensure!(
                !EntityUpgradeRules::<T>::contains_key(entity_id),
                Error::<T>::UpgradeRuleSystemAlreadyInitialized
            );

            let system = EntityUpgradeRuleSystem {
                rules: BoundedVec::default(),
                next_rule_id: 0,
                enabled: true,
                conflict_strategy,
            };

            EntityUpgradeRules::<T>::insert(entity_id, system);

            Self::deposit_event(Event::UpgradeRuleSystemInitialized {
                shop_id,
                conflict_strategy,
            });

            Ok(())
        }

        /// 添加升级规则
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `name`: 规则名称
        /// - `trigger`: 触发条件
        /// - `target_level_id`: 目标等级 ID
        /// - `duration`: 有效期（区块数，None 表示永久）
        /// - `priority`: 优先级
        /// - `stackable`: 是否可叠加
        /// - `max_triggers`: 最大触发次数
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::add_upgrade_rule())]
        pub fn add_upgrade_rule(
            origin: OriginFor<T>,
            shop_id: u64,
            name: BoundedVec<u8, ConstU32<64>>,
            trigger: UpgradeTrigger,
            target_level_id: u8,
            duration: Option<BlockNumberFor<T>>,
            priority: u8,
            stackable: bool,
            max_triggers: Option<u32>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            ensure!(!name.is_empty(), Error::<T>::EmptyRuleName);

            // H4 审计修复: 验证 target_level_id 对应的等级存在
            let level_system = EntityLevelSystems::<T>::get(entity_id)
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;
            ensure!(
                target_level_id > 0 && ((target_level_id - 1) as usize) < level_system.levels.len(),
                Error::<T>::InvalidTargetLevel
            );

            EntityUpgradeRules::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::UpgradeRuleSystemNotInitialized)?;

                let rule_id = system.next_rule_id;
                // M1 审计修复: checked_add 防止 u32 溢出导致重复 ID
                system.next_rule_id = system.next_rule_id.checked_add(1)
                    .ok_or(Error::<T>::RuleIdOverflow)?;

                let rule = UpgradeRule {
                    id: rule_id,
                    name: name.clone(),
                    trigger,
                    target_level_id,
                    duration,
                    enabled: true,
                    priority,
                    stackable,
                    max_triggers,
                    trigger_count: 0,
                };

                system.rules.try_push(rule).map_err(|_| Error::<T>::UpgradeRulesFull)?;

                Self::deposit_event(Event::UpgradeRuleAdded {
                    shop_id,
                    rule_id,
                    name,
                    target_level_id,
                });

                Ok(())
            })
        }

        /// 更新升级规则
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `rule_id`: 规则 ID
        /// - `enabled`: 是否启用
        /// - `priority`: 优先级
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::update_upgrade_rule())]
        pub fn update_upgrade_rule(
            origin: OriginFor<T>,
            shop_id: u64,
            rule_id: u32,
            enabled: Option<bool>,
            priority: Option<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityUpgradeRules::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::UpgradeRuleSystemNotInitialized)?;

                let rule = system.rules.iter_mut()
                    .find(|r| r.id == rule_id)
                    .ok_or(Error::<T>::UpgradeRuleNotFound)?;

                if let Some(e) = enabled {
                    rule.enabled = e;
                }

                if let Some(p) = priority {
                    rule.priority = p;
                }

                Self::deposit_event(Event::UpgradeRuleUpdated { shop_id, rule_id });

                Ok(())
            })
        }

        /// 删除升级规则
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `rule_id`: 规则 ID
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::remove_upgrade_rule())]
        pub fn remove_upgrade_rule(
            origin: OriginFor<T>,
            shop_id: u64,
            rule_id: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityUpgradeRules::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::UpgradeRuleSystemNotInitialized)?;

                let pos = system.rules.iter()
                    .position(|r| r.id == rule_id)
                    .ok_or(Error::<T>::UpgradeRuleNotFound)?;

                system.rules.remove(pos);

                Self::deposit_event(Event::UpgradeRuleRemoved { shop_id, rule_id });

                Ok(())
            })
        }

        /// 设置升级规则系统启用状态
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `enabled`: 是否启用
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::set_upgrade_rule_system_enabled())]
        pub fn set_upgrade_rule_system_enabled(
            origin: OriginFor<T>,
            shop_id: u64,
            enabled: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityUpgradeRules::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::UpgradeRuleSystemNotInitialized)?;
                system.enabled = enabled;

                Self::deposit_event(Event::UpgradeRuleSystemToggled { shop_id, enabled });

                Ok(())
            })
        }

        /// 设置规则冲突策略
        ///
        /// # 参数
        /// - `shop_id`: 店铺 ID
        /// - `conflict_strategy`: 冲突策略
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::set_conflict_strategy())]
        pub fn set_conflict_strategy(
            origin: OriginFor<T>,
            shop_id: u64,
            conflict_strategy: ConflictStrategy,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityUpgradeRules::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::UpgradeRuleSystemNotInitialized)?;
                system.conflict_strategy = conflict_strategy;

                Self::deposit_event(Event::ConflictStrategyUpdated { shop_id, strategy: conflict_strategy });

                Ok(())
            })
        }

        /// 设置会员注册策略（Entity 级别）
        ///
        /// # 参数
        /// - `shop_id`: 任意关联 Shop（用于定位 Entity 和权限校验）
        /// - `policy_bits`: 策略位标记（0=开放, 1=需购买, 2=需推荐人, 4=需审批，可组合）
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::set_member_policy())]
        pub fn set_member_policy(
            origin: OriginFor<T>,
            shop_id: u64,
            policy_bits: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 权限检查：Entity owner 或 admin
            ensure!(
                T::EntityProvider::entity_owner(entity_id).as_ref() == Some(&who)
                    || T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::MEMBER_MANAGE),
                Error::<T>::NotEntityAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // H2 审计修复: 只允许已定义的位
            let policy = MemberRegistrationPolicy(policy_bits);
            ensure!(policy.is_valid(), Error::<T>::InvalidPolicyBits);

            EntityMemberPolicy::<T>::insert(entity_id, policy);

            Self::deposit_event(Event::MemberPolicyUpdated { entity_id, policy });

            Ok(())
        }

        /// 审批通过待注册会员（APPROVAL_REQUIRED 模式）
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 待审批账户
        #[pallet::call_index(18)]
        #[pallet::weight(T::WeightInfo::approve_member())]
        pub fn approve_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 权限检查
            ensure!(
                T::EntityProvider::entity_owner(entity_id).as_ref() == Some(&who)
                    || T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::MEMBER_MANAGE),
                Error::<T>::NotEntityAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 取出待审批记录
            let (referrer, applied_at) = PendingMembers::<T>::take(entity_id, &account)
                .ok_or(Error::<T>::PendingMemberNotFound)?;

            // M6: 检查是否已过期
            let expiry = T::PendingMemberExpiry::get();
            if !expiry.is_zero() {
                let now = <frame_system::Pallet<T>>::block_number();
                ensure!(
                    now <= applied_at.saturating_add(expiry),
                    Error::<T>::PendingMemberAlreadyExpired
                );
            }

            // 正式注册
            Self::do_register_member(entity_id, &account, referrer, true)?;

            Self::deposit_event(Event::MemberApproved {
                entity_id,
                shop_id,
                account,
            });

            Ok(())
        }

        /// 拒绝待注册会员（APPROVAL_REQUIRED 模式）
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 待审批账户
        #[pallet::call_index(19)]
        #[pallet::weight(T::WeightInfo::reject_member())]
        pub fn reject_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 权限检查
            ensure!(
                T::EntityProvider::entity_owner(entity_id).as_ref() == Some(&who)
                    || T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::MEMBER_MANAGE),
                Error::<T>::NotEntityAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 删除待审批记录
            ensure!(
                PendingMembers::<T>::contains_key(entity_id, &account),
                Error::<T>::PendingMemberNotFound
            );
            PendingMembers::<T>::remove(entity_id, &account);

            Self::deposit_event(Event::MemberRejected {
                entity_id,
                account,
            });

            Ok(())
        }

        /// M6: 申请人撤回自己的待审批记录
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::cancel_pending_member())]
        pub fn cancel_pending_member(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            ensure!(
                PendingMembers::<T>::contains_key(entity_id, &who),
                Error::<T>::PendingMemberNotFound
            );
            PendingMembers::<T>::remove(entity_id, &who);

            Self::deposit_event(Event::PendingMemberCancelled {
                entity_id,
                account: who,
            });

            Ok(())
        }

        /// M6: 清理过期的待审批记录（任人可调用，按 entity 清理）
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::cleanup_expired_pending())]
        pub fn cleanup_expired_pending(
            origin: OriginFor<T>,
            entity_id: u64,
            max_clean: u32,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            let max_clean = max_clean.min(50);

            let expiry = T::PendingMemberExpiry::get();
            // 如果过期时间为 0，不执行清理（永不过期）
            if expiry.is_zero() {
                return Ok(());
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let mut cleaned = 0u32;
            let mut to_remove: alloc::vec::Vec<T::AccountId> = alloc::vec::Vec::new();

            for (account, (_referrer, applied_at)) in PendingMembers::<T>::iter_prefix(entity_id) {
                if cleaned >= max_clean {
                    break;
                }
                if now > applied_at.saturating_add(expiry) {
                    to_remove.push(account);
                    cleaned += 1;
                }
            }

            for account in to_remove {
                PendingMembers::<T>::remove(entity_id, &account);
                Self::deposit_event(Event::PendingMemberExpired {
                    entity_id,
                    account,
                });
            }

            Ok(())
        }

        /// 设置会员统计策略（Entity 级别）
        ///
        /// 控制推荐人数的计算口径：是否将复购赠与注册的账户计入直推/间推人数
        ///
        /// # 参数
        /// - `shop_id`: 任意关联 Shop（用于定位 Entity 和权限校验）
        /// - `policy_bits`: 统计策略位标记（0=排除复购, 1=直推含复购, 2=间推含复购，可组合）
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::set_member_stats_policy())]
        pub fn set_member_stats_policy(
            origin: OriginFor<T>,
            shop_id: u64,
            policy_bits: u8,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;

            // 权限检查：Entity owner 或 admin
            ensure!(
                T::EntityProvider::entity_owner(entity_id).as_ref() == Some(&who)
                    || T::EntityProvider::is_entity_admin(entity_id, &who, AdminPermission::MEMBER_MANAGE),
                Error::<T>::NotEntityAdmin
            );
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            // 只允许低 2 位 (INCLUDE_REPURCHASE_DIRECT=1, INCLUDE_REPURCHASE_INDIRECT=2)
            ensure!(policy_bits <= 0b0000_0011, Error::<T>::InvalidPolicyBits);

            let policy = MemberStatsPolicy(policy_bits);
            EntityMemberStatsPolicy::<T>::insert(entity_id, policy);

            Self::deposit_event(Event::MemberStatsPolicyUpdated { entity_id, policy });

            Ok(())
        }

        /// 批量审批待注册会员（最多 50 个）
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `accounts`: 待审批账户列表
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::batch_approve_members())]
        pub fn batch_approve_members(
            origin: OriginFor<T>,
            shop_id: u64,
            accounts: alloc::vec::Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(accounts.len() <= 50usize, Error::<T>::BatchLimitExceeded);

            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let expiry = T::PendingMemberExpiry::get();
            let now = <frame_system::Pallet<T>>::block_number();
            let mut approved_count: u32 = 0;

            for account in accounts.iter() {
                if let Some((referrer, applied_at)) = PendingMembers::<T>::take(entity_id, account) {
                    // H2 审计修复: 过期记录发出事件（take 已移除存储，需通知链下）
                    if !expiry.is_zero() && now > applied_at.saturating_add(expiry) {
                        Self::deposit_event(Event::PendingMemberExpired {
                            entity_id,
                            account: account.clone(),
                        });
                        continue;
                    }
                    // 尝试注册，失败则静默跳过（如已是会员）
                    if Self::do_register_member(entity_id, account, referrer, true).is_ok() {
                        approved_count = approved_count.saturating_add(1);
                    }
                }
            }

            Self::deposit_event(Event::BatchMembersApproved {
                entity_id,
                count: approved_count,
            });

            Ok(())
        }

        /// 批量拒绝待注册会员（最多 50 个）
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `accounts`: 待拒绝账户列表
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::batch_reject_members())]
        pub fn batch_reject_members(
            origin: OriginFor<T>,
            shop_id: u64,
            accounts: alloc::vec::Vec<T::AccountId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(accounts.len() <= 50usize, Error::<T>::BatchLimitExceeded);

            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let mut rejected_count: u32 = 0;

            for account in accounts.iter() {
                if PendingMembers::<T>::contains_key(entity_id, account) {
                    PendingMembers::<T>::remove(entity_id, account);
                    rejected_count = rejected_count.saturating_add(1);
                }
            }

            Self::deposit_event(Event::BatchMembersRejected {
                entity_id,
                count: rejected_count,
            });

            Ok(())
        }

        /// 封禁会员
        ///
        /// 封禁后会员不再享受等级权益，不计入推荐统计。
        /// 仅 Entity Owner 或 Admin(MEMBER_MANAGE) 可操作。
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 要封禁的会员账户
        /// - `reason`: A2: 封禁原因（可选，最长 128 字节）
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::ban_member())]
        pub fn ban_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
            reason: Option<BoundedVec<u8, ConstU32<128>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityMembers::<T>::try_mutate(entity_id, &account, |maybe_member| -> DispatchResult {
                let member = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;
                ensure!(member.banned_at.is_none(), Error::<T>::MemberAlreadyBanned);

                let now = <frame_system::Pallet<T>>::block_number();
                member.banned_at = Some(now);
                member.ban_reason = reason.clone();
                Ok(())
            })?;

            // S6: 维护封禁计数器
            BannedMemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));

            Self::deposit_event(Event::MemberBanned {
                entity_id,
                account,
                reason,
            });

            Ok(())
        }

        /// 解封会员
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 要解封的会员账户
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::unban_member())]
        pub fn unban_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityMembers::<T>::try_mutate(entity_id, &account, |maybe_member| -> DispatchResult {
                let member = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;
                ensure!(member.banned_at.is_some(), Error::<T>::MemberNotBanned);
                member.banned_at = None;
                member.ban_reason = None;
                Ok(())
            })?;

            // S6: 维护封禁计数器
            BannedMemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_sub(1));

            Self::deposit_event(Event::MemberUnbanned {
                entity_id,
                account,
            });

            Ok(())
        }

        /// 移除会员（仅无下线时允许）
        ///
        /// 彻底移除会员数据，级联清理推荐链、统计计数、USDT 消费记录等。
        /// 仅当会员无直推下线时允许操作，防止推荐链断裂。
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 要移除的会员账户
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::remove_member())]
        pub fn remove_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            Self::do_remove_member(entity_id, &account)?;

            Self::deposit_event(Event::MemberRemoved {
                entity_id,
                account,
            });

            Ok(())
        }

        /// 重置等级系统（仅当所有会员均为 level 0 时允许）
        ///
        /// 清除等级配置，允许 Owner 重新 init_level_system。
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        #[pallet::call_index(28)]
        #[pallet::weight(T::WeightInfo::reset_level_system())]
        pub fn reset_level_system(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            let system = EntityLevelSystems::<T>::get(entity_id)
                .ok_or(Error::<T>::LevelSystemNotInitialized)?;

            // 检查是否所有会员均为 level 0
            // 只要有非零等级的会员计数 > 0，就不允许重置
            for level in system.levels.iter() {
                let count = LevelMemberCount::<T>::get(entity_id, level.id);
                ensure!(count == 0, Error::<T>::LevelSystemHasNonZeroMembers);
            }

            // 清除等级系统
            EntityLevelSystems::<T>::remove(entity_id);

            // 清除所有 LevelMemberCount（保留 level 0 也清理，init 时会重建）
            let _ = LevelMemberCount::<T>::clear_prefix(entity_id, u32::MAX, None);

            Self::deposit_event(Event::LevelSystemReset { entity_id });

            Ok(())
        }

        /// 重置升级规则系统
        ///
        /// 清除升级规则配置，允许 Owner 重新 init_upgrade_rule_system。
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        #[pallet::call_index(29)]
        #[pallet::weight(T::WeightInfo::reset_upgrade_rule_system())]
        pub fn reset_upgrade_rule_system(
            origin: OriginFor<T>,
            shop_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            ensure!(
                EntityUpgradeRules::<T>::contains_key(entity_id),
                Error::<T>::UpgradeRuleSystemNotInitialized
            );

            EntityUpgradeRules::<T>::remove(entity_id);

            Self::deposit_event(Event::UpgradeRuleSystemReset { entity_id });

            Ok(())
        }

        /// U1: 会员主动退出实体
        ///
        /// 会员本人可主动退出实体，清理所有关联数据。
        /// 仅当会员未被封禁且无下线时允许操作。
        ///
        /// # 参数
        /// - `entity_id`: 实体 ID
        #[pallet::call_index(30)]
        #[pallet::weight(T::WeightInfo::leave_entity())]
        pub fn leave_entity(
            origin: OriginFor<T>,
            entity_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let member = EntityMembers::<T>::get(entity_id, &who)
                .ok_or(Error::<T>::NotMember)?;

            // 被封禁的会员不能主动退出（需管理员解封或移除）
            ensure!(member.banned_at.is_none(), Error::<T>::CannotLeave);

            Self::do_remove_member(entity_id, &who)?;

            Self::deposit_event(Event::MemberLeft {
                entity_id,
                account: who,
            });

            Ok(())
        }

        /// 手动激活会员（Owner/Admin 操作）
        ///
        /// 管理员可手动激活未消费的会员（白名单场景），
        /// 激活后会员可获得佣金分配。
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 要激活的会员账户
        #[pallet::call_index(31)]
        #[pallet::weight(T::WeightInfo::activate_member())]
        pub fn activate_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityMembers::<T>::try_mutate(entity_id, &account, |maybe_member| -> DispatchResult {
                let member = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;
                ensure!(!member.activated, Error::<T>::AlreadyActivated);
                member.activated = true;
                Ok(())
            })?;

            Self::deposit_event(Event::MemberActivated {
                entity_id,
                account,
            });

            Ok(())
        }

        /// 手动取消激活会员（Owner/Admin 操作）
        ///
        /// 管理员可手动取消会员激活状态（惩罚但不封禁），
        /// 取消激活后会员不再获得佣金分配。
        ///
        /// # 参数
        /// - `shop_id`: 关联 Shop
        /// - `account`: 要取消激活的会员账户
        #[pallet::call_index(32)]
        #[pallet::weight(T::WeightInfo::deactivate_member())]
        pub fn deactivate_member(
            origin: OriginFor<T>,
            shop_id: u64,
            account: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let entity_id = Self::ensure_shop_owner_or_admin(shop_id, &who)?;
            ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);

            EntityMembers::<T>::try_mutate(entity_id, &account, |maybe_member| -> DispatchResult {
                let member = maybe_member.as_mut().ok_or(Error::<T>::NotMember)?;
                ensure!(member.activated, Error::<T>::NotActivated);
                member.activated = false;
                Ok(())
            })?;

            Self::deposit_event(Event::MemberDeactivated {
                entity_id,
                account,
            });

            Ok(())
        }
    }

    // ============================================================================
    // 内部函数 — 按职责分文件（在 pallet 外部声明为 mod，使用 super::pallet::* 访问）
    // ============================================================================
}

// ============================================================================
// 内部函数 — 按职责分文件
// ============================================================================

/// 会员注册、审批、移除、自动注册、消费更新、查询
mod registration;

/// 推荐链管理、团队人数维护、循环检测
mod referral;

/// 等级系统管理、等级计算、有效等级查询、治理调用
mod level;

/// 升级规则系统：规则评估、冲突解决、升级应用
mod upgrade_rule;

// ============================================================================
// MemberProvider — 从 pallet-entity-common 统一导出
// ============================================================================

pub use pallet_entity_common::{MemberLevelInfo, MemberProvider};

/// MemberProvider 实现（统一使用 entity_id，无需 shop_id 解析）
impl<T: pallet::Config> MemberProvider<T::AccountId> for pallet::Pallet<T> {
    fn is_member(entity_id: u64, account: &T::AccountId) -> bool {
        pallet::EntityMembers::<T>::contains_key(entity_id, account)
    }

    fn custom_level_id(entity_id: u64, account: &T::AccountId) -> u8 {
        // H6 审计修复: 使用 get_effective_level_by_entity 检查等级过期
        Self::get_effective_level_by_entity(entity_id, account)
    }

    fn get_level_discount(entity_id: u64, level_id: u8) -> u16 {
        pallet::Pallet::<T>::get_level_discount_by_entity(entity_id, level_id)
    }

    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16 {
        pallet::Pallet::<T>::get_level_commission_bonus_by_entity(entity_id, level_id)
    }

    fn uses_custom_levels(entity_id: u64) -> bool {
        pallet::Pallet::<T>::uses_custom_levels_by_entity(entity_id)
    }

    fn get_referrer(entity_id: u64, account: &T::AccountId) -> Option<T::AccountId> {
        pallet::EntityMembers::<T>::get(entity_id, account).and_then(|m| m.referrer)
    }

    fn auto_register(entity_id: u64, account: &T::AccountId, referrer: Option<T::AccountId>) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::auto_register_by_entity(entity_id, account, referrer, true)
    }

    fn update_spent(entity_id: u64, account: &T::AccountId, amount_usdt: u64) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::update_spent_by_entity(entity_id, account, amount_usdt)
    }

    fn check_order_upgrade_rules(
        entity_id: u64,
        buyer: &T::AccountId,
        product_id: u64,
        amount_usdt: u64,
    ) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::check_order_upgrade_rules_by_entity(entity_id, buyer, product_id, amount_usdt)
    }

    fn get_effective_level(entity_id: u64, account: &T::AccountId) -> u8 {
        pallet::Pallet::<T>::get_effective_level_by_entity(entity_id, account)
    }

    fn get_member_stats(entity_id: u64, account: &T::AccountId) -> (u32, u32, u128) {
        let policy = pallet::EntityMemberStatsPolicy::<T>::get(entity_id);
        pallet::EntityMembers::<T>::get(entity_id, account)
            .map(|m| {
                let direct = if policy.include_repurchase_direct() {
                    m.direct_referrals
                } else {
                    m.qualified_referrals
                };
                (direct, m.team_size, m.total_spent as u128)
            })
            .unwrap_or((0, 0, 0))
    }

    fn member_count(entity_id: u64) -> u32 {
        pallet::MemberCount::<T>::get(entity_id)
    }

    fn requires_referral(entity_id: u64) -> bool {
        pallet::EntityMemberPolicy::<T>::get(entity_id).requires_referral()
    }

    fn is_banned(entity_id: u64, account: &T::AccountId) -> bool {
        pallet::EntityMembers::<T>::get(entity_id, account)
            .map(|m| m.banned_at.is_some())
            .unwrap_or(false)
    }

    fn is_activated(entity_id: u64, account: &T::AccountId) -> bool {
        pallet::EntityMembers::<T>::get(entity_id, account)
            .map(|m| m.activated)
            .unwrap_or(false)
    }

    fn last_active_at(entity_id: u64, account: &T::AccountId) -> u64 {
        pallet::EntityMembers::<T>::get(entity_id, account)
            .map(|m| sp_runtime::SaturatedConversion::saturated_into(m.last_active_at))
            .unwrap_or(0)
    }

    fn member_level(entity_id: u64, account: &T::AccountId) -> Option<MemberLevelInfo> {
        if !pallet::EntityMembers::<T>::contains_key(entity_id, account) {
            return None;
        }
        let level_id = Self::get_effective_level(entity_id, account);
        let info = pallet::Pallet::<T>::get_custom_level_info_by_entity(entity_id, level_id)?;
        Some(MemberLevelInfo {
            level_id: info.id,
            name: info.name.into_inner(),
            threshold: info.threshold as u128,
            discount_rate: info.discount_rate,
            commission_bonus: info.commission_bonus,
        })
    }

    fn custom_level_count(entity_id: u64) -> u8 {
        pallet::Pallet::<T>::custom_level_count_by_entity(entity_id)
    }

    fn member_count_by_level(entity_id: u64, level_id: u8) -> u32 {
        pallet::LevelMemberCount::<T>::get(entity_id, level_id)
    }

    fn get_member_spent_usdt(entity_id: u64, account: &T::AccountId) -> u64 {
        pallet::EntityMembers::<T>::get(entity_id, account)
            .map(|m| m.total_spent)
            .unwrap_or(0)
    }

    fn completed_order_count(entity_id: u64, account: &T::AccountId) -> u32 {
        pallet::MemberOrderCount::<T>::get(entity_id, account)
    }

    fn referral_registered_at(entity_id: u64, account: &T::AccountId) -> u64 {
        pallet::EntityMembers::<T>::get(entity_id, account)
            .map(|m| sp_runtime::SaturatedConversion::saturated_into(m.joined_at))
            .unwrap_or(0)
    }

    fn auto_register_qualified(entity_id: u64, account: &T::AccountId, referrer: Option<T::AccountId>, qualified: bool) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::auto_register_by_entity(entity_id, account, referrer, qualified)
    }

    fn set_custom_levels_enabled(entity_id: u64, enabled: bool) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_set_custom_levels_enabled(entity_id, enabled)
    }

    fn set_upgrade_mode(entity_id: u64, mode: u8) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_set_upgrade_mode(entity_id, mode)
    }

    fn add_custom_level(entity_id: u64, _level_id: u8, name: &[u8], threshold: u128, discount_rate: u16, commission_bonus: u16) -> sp_runtime::DispatchResult {
        // level_id 由 pallet 自动分配，忽略传入值
        pallet::Pallet::<T>::governance_add_custom_level(entity_id, name, threshold, discount_rate, commission_bonus)
    }

    fn update_custom_level(entity_id: u64, level_id: u8, name: Option<&[u8]>, threshold: Option<u128>, discount_rate: Option<u16>, commission_bonus: Option<u16>) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_update_custom_level(entity_id, level_id, name, threshold, discount_rate, commission_bonus)
    }

    fn remove_custom_level(entity_id: u64, level_id: u8) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_remove_custom_level(entity_id, level_id)
    }

    fn set_registration_policy(entity_id: u64, policy_bits: u8) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_set_registration_policy(entity_id, policy_bits)
    }

    fn set_stats_policy(entity_id: u64, policy_bits: u8) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_set_stats_policy(entity_id, policy_bits)
    }

    fn set_upgrade_rule_system_enabled(entity_id: u64, enabled: bool) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::governance_set_upgrade_rule_system_enabled(entity_id, enabled)
    }

    fn get_direct_referral_accounts(entity_id: u64, account: &T::AccountId) -> alloc::vec::Vec<T::AccountId> {
        pallet::DirectReferrals::<T>::get(entity_id, account).into_inner()
    }
}

/// 空实现 — 从 pallet-entity-common 统一导出
pub use pallet_entity_common::NullMemberProvider;
