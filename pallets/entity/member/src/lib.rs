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

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

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
        traits::{Currency, Get},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use pallet_entity_common::{EntityProvider, ShopProvider, MemberRegistrationPolicy, MemberStatsPolicy, AdminPermission};
    use sp_runtime::traits::{Saturating, Zero};

    /// 货币余额类型别名
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
    pub trait Config: frame_system::Config {
        /// 运行时事件类型
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// 货币类型
        type Currency: Currency<Self::AccountId>;

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
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// A4: on_idle 自动清理过期待审批会员
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            let expiry = T::PendingMemberExpiry::get();
            if expiry.is_zero() {
                return Weight::zero();
            }

            // 每次最多清理 10 条，避免占用过多区块权重
            let per_item_weight = Weight::from_parts(50_000_000, 5_000);
            let max_items = 10u32;
            let min_weight = per_item_weight.saturating_mul(2); // 至少能清理 2 条才值得执行
            if remaining_weight.any_lt(min_weight) {
                return Weight::zero();
            }

            let now = <frame_system::Pallet<T>>::block_number();
            let mut cleaned = 0u32;
            let mut weight_used = Weight::zero();

            // 遍历所有 entity 的 PendingMembers（cursor-free, 利用 iter() 的惰性）
            let mut to_remove: alloc::vec::Vec<(u64, T::AccountId)> = alloc::vec::Vec::new();
            for (entity_id, account, (_referrer, applied_at)) in PendingMembers::<T>::iter() {
                if cleaned >= max_items {
                    break;
                }
                if remaining_weight.any_lt(weight_used.saturating_add(per_item_weight)) {
                    break;
                }
                if now > applied_at.saturating_add(expiry) {
                    to_remove.push((entity_id, account));
                    cleaned += 1;
                }
                weight_used = weight_used.saturating_add(Weight::from_parts(10_000_000, 1_000));
            }

            for (entity_id, account) in to_remove.iter() {
                PendingMembers::<T>::remove(entity_id, account);
                Self::deposit_event(Event::PendingMemberExpired {
                    entity_id: *entity_id,
                    account: account.clone(),
                });
                weight_used = weight_used.saturating_add(per_item_weight);
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
    // 事件
    // ============================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 会员注册
        MemberRegistered {
            entity_id: u64,
            shop_id: u64,
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
        /// 手动升级会员
        MemberManuallyUpgraded {
            shop_id: u64,
            account: T::AccountId,
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
        /// A4: 待审批过期自动清理（on_idle）
        PendingMembersCleanedUp {
            entity_id: u64,
            count: u32,
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
        /// 等级已存在
        LevelAlreadyExists,
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
        /// M6: 不是待审批申请人
        NotPendingApplicant,
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
        #[pallet::weight(Weight::from_parts(375_000_000, 12_000))]
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
                shop_id,
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
        #[pallet::weight(Weight::from_parts(400_000_000, 16_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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

                let level_id = system.levels.len() as u8;

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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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

                ensure!((level_id as usize) < system.levels.len(), Error::<T>::LevelNotFound);

                // 验证阈值（先检查，再修改）
                if let Some(new_threshold) = threshold {
                    // 必须大于前一等级
                    if level_id > 0 {
                        if let Some(prev) = system.levels.get((level_id - 1) as usize) {
                            ensure!(new_threshold > prev.threshold, Error::<T>::InvalidThreshold);
                        }
                    }
                    // 必须小于后一等级
                    if let Some(next) = system.levels.get((level_id + 1) as usize) {
                        ensure!(new_threshold < next.threshold, Error::<T>::InvalidThreshold);
                    }
                }

                // 现在安全地获取可变引用并修改
                let level = system.levels.get_mut(level_id as usize)
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
                    level_id as usize == system.levels.len().saturating_sub(1),
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
        #[pallet::weight(Weight::from_parts(175_000_000, 12_000))]
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
                (target_level_id as usize) < system.levels.len(),
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
                (target_level_id as usize) < level_system.levels.len(),
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(375_000_000, 12_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(100_000_000, 6_000))]
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
        #[pallet::weight(Weight::from_parts(500_000_000, 20_000))]
        pub fn cleanup_expired_pending(
            origin: OriginFor<T>,
            entity_id: u64,
            max_clean: u32,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(500_000_000, 30_000))]
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
        #[pallet::weight(Weight::from_parts(500_000_000, 30_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(500_000_000, 20_000))]
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
        #[pallet::weight(Weight::from_parts(200_000_000, 12_000))]
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
            for level_id in 1..system.levels.len() {
                let count = LevelMemberCount::<T>::get(entity_id, level_id as u8);
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
        #[pallet::weight(Weight::from_parts(200_000_000, 12_000))]
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
        #[pallet::weight(Weight::from_parts(500_000_000, 20_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
        #[pallet::weight(Weight::from_parts(150_000_000, 8_000))]
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
    // 内部函数
    // ============================================================================

    impl<T: Config> Pallet<T> {
        /// 解析 shop_id 对应的 entity_id
        fn resolve_entity_id(shop_id: u64) -> Option<u64> {
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
        fn do_remove_member(entity_id: u64, account: &T::AccountId) -> DispatchResult {
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
                        // M1 审计修复: 同步递减 qualified_referrals（与 decrement_team_size_by_entity 对称）
                        r.qualified_referrals = r.qualified_referrals.saturating_sub(1);
                    }
                });
                // 递减推荐链上所有祖先的 team_size
                Self::decrement_team_size_by_entity(entity_id, referrer);
            }

            // 3. 清理关联存储
            EntityMembers::<T>::remove(entity_id, account);
            DirectReferrals::<T>::remove(entity_id, account);
            MemberLevelExpiry::<T>::remove(entity_id, account);
            MemberUpgradeHistory::<T>::remove(entity_id, account);
            MemberOrderCount::<T>::remove(entity_id, account);

            Ok(())
        }

        /// 注册会员内部实现（统一写入 EntityMembers[entity_id]）
        ///
        /// `qualified`: 是否为有效直推（主动注册/购买触发=true，复购赠与=false）
        fn do_register_member(
            entity_id: u64,
            account: &T::AccountId,
            referrer: Option<T::AccountId>,
            qualified: bool,
        ) -> DispatchResult {
            let now = <frame_system::Pallet<T>>::block_number();

            let member = EntityMember {
                referrer: referrer.clone(),
                direct_referrals: 0,
                qualified_referrals: 0,
                indirect_referrals: 0,
                qualified_indirect_referrals: 0,
                team_size: 0,
                total_spent: 0u64,
                custom_level_id: 0,
                joined_at: now,
                last_active_at: now,
                activated: false,
                banned_at: None,
                ban_reason: None,
            };

            EntityMembers::<T>::insert(entity_id, account, member);

            MemberCount::<T>::mutate(entity_id, |count| *count = count.saturating_add(1));
            LevelMemberCount::<T>::mutate(entity_id, 0u8, |count| *count = count.saturating_add(1));
            if let Some(ref ref_account) = referrer {
                Self::mutate_member_referral(entity_id, ref_account, account, qualified)?;
            }

            Ok(())
        }

        /// 更新推荐人统计（entity 级别）
        ///
        /// `qualified`: 是否为有效直推（主动注册/购买触发=true，复购赠与=false）
        fn mutate_member_referral(
            entity_id: u64,
            ref_account: &T::AccountId,
            new_member: &T::AccountId,
            qualified: bool,
        ) -> DispatchResult {
            // 先检查推荐索引容量（失败则回滚，不产生不一致状态）
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

        /// 检查是否存在循环推荐
        /// P2 安全修复: 增加已访问集合检测，防止无限循环
        fn is_circular_referral(
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
        ///
        /// depth=0 是直接推荐人（team_size++，直推已在 mutate_member_referral 中单独处理）
        /// depth>=1 是间接推荐人（team_size++ 且 indirect_referrals++）
        fn update_team_size_by_entity(entity_id: u64, account: &T::AccountId, qualified: bool) {
            use alloc::collections::BTreeSet;

            let mut current = Some(account.clone());
            let mut depth = 0u32;
            let mut visited = BTreeSet::new();
            const MAX_DEPTH: u32 = 100;

            while let Some(ref curr_account) = current {
                if depth >= MAX_DEPTH {
                    break;
                }

                if visited.contains(curr_account) {
                    break;
                }
                visited.insert(curr_account.clone());

                EntityMembers::<T>::mutate(entity_id, curr_account, |maybe_member| {
                    if let Some(ref mut m) = maybe_member {
                        m.team_size = m.team_size.saturating_add(1);
                        // depth >= 1: 间接推荐人（depth=0 是直接推荐人，已单独计数）
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
        }

        /// 递减推荐链上所有祖先的 team_size（remove_member 时使用）
        fn decrement_team_size_by_entity(entity_id: u64, account: &T::AccountId) {
            use alloc::collections::BTreeSet;

            let mut current = Some(account.clone());
            let mut visited = BTreeSet::new();
            const MAX_DEPTH: u32 = 100;
            let mut depth = 0u32;

            while let Some(ref curr_account) = current {
                if depth >= MAX_DEPTH {
                    break;
                }
                if visited.contains(curr_account) {
                    break;
                }
                visited.insert(curr_account.clone());

                EntityMembers::<T>::mutate(entity_id, curr_account, |maybe_member| {
                    if let Some(ref mut m) = maybe_member {
                        m.team_size = m.team_size.saturating_sub(1);
                        if depth >= 1 {
                            m.indirect_referrals = m.indirect_referrals.saturating_sub(1);
                            // S4 修复: 同步递减 qualified_indirect_referrals（与 update_team_size_by_entity 对称）
                            // 注意: remove_member 仅在无下线时允许，因此此处 qualified 状态不影响
                            // 但为保证统计一致性，仍进行递减
                            m.qualified_indirect_referrals = m.qualified_indirect_referrals.saturating_sub(1);
                        }
                    }
                });

                current = EntityMembers::<T>::get(entity_id, curr_account)
                    .and_then(|m| m.referrer);
                depth += 1;
            }
        }

        /// 验证店主或管理员权限（MEMBER_MANAGE），成功时返回 entity_id
        fn ensure_shop_owner_or_admin(shop_id: u64, who: &T::AccountId) -> Result<u64, DispatchError> {
            let entity_id = T::ShopProvider::shop_entity_id(shop_id)
                .ok_or(Error::<T>::ShopNotFound)?;
            ensure!(
                T::EntityProvider::entity_owner(entity_id).as_ref() == Some(who)
                    || T::EntityProvider::is_entity_admin(entity_id, who, AdminPermission::MEMBER_MANAGE),
                Error::<T>::NotEntityAdmin
            );
            Ok(entity_id)
        }

        /// 计算自定义等级（根据 USDT 消费金额）
        pub fn calculate_custom_level(shop_id: u64, total_spent: u64) -> u8 {
            let entity_id = match Self::resolve_entity_id(shop_id) {
                Some(id) => id,
                None => return 0,
            };
            Self::calculate_custom_level_by_entity(entity_id, total_spent)
        }

        /// 计算自定义等级（entity_id 直达，避免重复解析）
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

        // ========================================================================
        // 升级规则相关内部函数
        // ========================================================================

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
        fn check_referral_upgrade_rules_by_entity(
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
        fn resolve_conflict(
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
        fn apply_upgrade(
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
                    let current_expiry = MemberLevelExpiry::<T>::get(entity_id, account)
                        .unwrap_or(now);
                    duration.map(|d| current_expiry.saturating_add(d))
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
        pub fn get_effective_level_by_entity(entity_id: u64, account: &T::AccountId) -> u8 {
            let member = match EntityMembers::<T>::get(entity_id, account) {
                Some(m) => m,
                None => return 0,
            };

            if let Some(expires_at) = MemberLevelExpiry::<T>::get(entity_id, account) {
                let now = <frame_system::Pallet<T>>::block_number();
                if now > expires_at {
                    let recalculated = Self::calculate_custom_level_by_entity(entity_id, member.total_spent);
                    // S1: 写穿修正存储
                    if recalculated != member.custom_level_id {
                        LevelMemberCount::<T>::mutate(entity_id, member.custom_level_id, |c| *c = c.saturating_sub(1));
                        LevelMemberCount::<T>::mutate(entity_id, recalculated, |c| *c = c.saturating_add(1));
                        EntityMembers::<T>::mutate(entity_id, account, |maybe| {
                            if let Some(ref mut m) = maybe {
                                m.custom_level_id = recalculated;
                            }
                        });
                        Self::deposit_event(Event::MemberLevelExpired {
                            entity_id,
                            account: account.clone(),
                            expired_level_id: member.custom_level_id,
                            new_level_id: recalculated,
                        });
                    }
                    MemberLevelExpiry::<T>::remove(entity_id, account);
                    return recalculated;
                }
            }

            // M9 审计修复: 验证 custom_level_id 对应的等级仍然存在
            // 等级可能被 remove_custom_level 删除，此时回退到基于消费的计算
            if let Some(system) = EntityLevelSystems::<T>::get(entity_id) {
                if system.use_custom && (member.custom_level_id as usize) >= system.levels.len() {
                    return Self::calculate_custom_level_by_entity(entity_id, member.total_spent);
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

        // ========================================================================
        // 治理调用内部函数（供跨模块桥接使用，无 origin 检查）
        // ========================================================================

        /// 启用/禁用自定义等级（治理调用）
        pub fn governance_set_custom_levels_enabled(entity_id: u64, enabled: bool) -> DispatchResult {
            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;
                system.use_custom = enabled;
                Ok(())
            })
        }

        /// 设置升级模式（治理调用）
        /// mode: 0=AutoUpgrade, 1=ManualUpgrade
        pub fn governance_set_upgrade_mode(entity_id: u64, mode: u8) -> DispatchResult {
            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;
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
            let name: BoundedVec<u8, ConstU32<32>> = name.to_vec().try_into()
                .map_err(|_| Error::<T>::NameTooLong)?;
            ensure!(!name.is_empty(), Error::<T>::EmptyLevelName);
            ensure!(discount_rate <= 10000, Error::<T>::InvalidBasisPoints);
            ensure!(commission_bonus <= 10000, Error::<T>::InvalidBasisPoints);

            let threshold_u64: u64 = threshold.try_into()
                .map_err(|_| Error::<T>::Overflow)?;

            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;

                if let Some(last) = system.levels.last() {
                    ensure!(threshold_u64 > last.threshold, Error::<T>::InvalidThreshold);
                }

                let level_id = system.levels.len() as u8;

                let level = CustomLevel {
                    id: level_id,
                    name,
                    threshold: threshold_u64,
                    discount_rate,
                    commission_bonus,
                };

                system.levels.try_push(level).map_err(|_| Error::<T>::LevelsFull)?;
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
            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;
                ensure!((level_id as usize) < system.levels.len(), Error::<T>::LevelNotFound);

                if let Some(new_threshold_u128) = threshold {
                    let new_threshold: u64 = new_threshold_u128.try_into()
                        .map_err(|_| Error::<T>::Overflow)?;
                    if level_id > 0 {
                        if let Some(prev) = system.levels.get((level_id - 1) as usize) {
                            ensure!(new_threshold > prev.threshold, Error::<T>::InvalidThreshold);
                        }
                    }
                    if let Some(next) = system.levels.get((level_id + 1) as usize) {
                        ensure!(new_threshold < next.threshold, Error::<T>::InvalidThreshold);
                    }
                }

                let level = system.levels.get_mut(level_id as usize)
                    .ok_or(Error::<T>::LevelNotFound)?;

                if let Some(new_threshold_u128) = threshold {
                    let new_threshold: u64 = new_threshold_u128.try_into()
                        .map_err(|_| Error::<T>::Overflow)?;
                    level.threshold = new_threshold;
                }
                if let Some(new_name) = name {
                    let bounded_name: BoundedVec<u8, ConstU32<32>> = new_name.to_vec().try_into()
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
            EntityLevelSystems::<T>::try_mutate(entity_id, |maybe_system| -> DispatchResult {
                let system = maybe_system.as_mut().ok_or(Error::<T>::LevelSystemNotInitialized)?;
                ensure!(
                    level_id as usize == system.levels.len().saturating_sub(1),
                    Error::<T>::InvalidLevelId
                );
                // H1 审计修复: 检查该等级是否还有会员
                ensure!(
                    LevelMemberCount::<T>::get(entity_id, level_id) == 0,
                    Error::<T>::LevelHasMembers
                );
                system.levels.pop();
                Ok(())
            })
        }

        /// G1: 设置注册策略（治理调用）
        pub fn governance_set_registration_policy(entity_id: u64, policy_bits: u8) -> DispatchResult {
            let policy = MemberRegistrationPolicy(policy_bits);
            ensure!(policy.is_valid(), Error::<T>::InvalidPolicyBits);
            EntityMemberPolicy::<T>::insert(entity_id, policy);
            Self::deposit_event(Event::GovernanceMemberPolicyUpdated { entity_id, policy });
            Ok(())
        }

        /// G1: 设置统计策略（治理调用）
        pub fn governance_set_stats_policy(entity_id: u64, policy_bits: u8) -> DispatchResult {
            let policy = MemberStatsPolicy(policy_bits);
            ensure!(policy.is_valid(), Error::<T>::InvalidPolicyBits);
            EntityMemberStatsPolicy::<T>::insert(entity_id, policy);
            Self::deposit_event(Event::GovernanceStatsPolicyUpdated { entity_id, policy });
            Ok(())
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
                shop_id,
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
                shop_id: 0,
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
                for level_id in 1..system.levels.len() {
                    let count = LevelMemberCount::<T>::get(entity_id, level_id as u8);
                    level_distribution.push((level_id as u8, count));
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
    }
}

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
            threshold: info.threshold,
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
}

/// OrderMemberHandler 实现（供 Transaction 模块调用，统一使用 entity_id）
impl<T: pallet::Config> pallet_entity_common::OrderMemberHandler<T::AccountId> for pallet::Pallet<T> {
    fn auto_register(entity_id: u64, account: &T::AccountId, referrer: Option<T::AccountId>) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::auto_register_by_entity(entity_id, account, referrer, true)
    }

    fn update_spent(entity_id: u64, account: &T::AccountId, amount_usdt: u64) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::update_spent_by_entity(entity_id, account, amount_usdt)
    }

    fn check_order_upgrade_rules(entity_id: u64, buyer: &T::AccountId, product_id: u64, amount_usdt: u64) -> sp_runtime::DispatchResult {
        pallet::Pallet::<T>::check_order_upgrade_rules_by_entity(entity_id, buyer, product_id, amount_usdt)
    }
}

/// 空实现 — 从 pallet-entity-common 统一导出
pub use pallet_entity_common::NullMemberProvider;
