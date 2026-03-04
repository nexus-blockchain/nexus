//! # 实体公共模块 (pallet-entity-common)
//!
//! 定义实体各子模块共享的类型和 Trait 接口

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

#[cfg(test)]
mod tests;

// ============================================================================
// 实体类型枚举 (Phase 2 新增)
// ============================================================================

/// 实体类型
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum EntityType {
    /// 商户（原 Shop，默认类型）
    #[default]
    Merchant,
    /// 企业
    Enterprise,
    /// 去中心化自治组织
    DAO,
    /// 社区
    Community,
    /// 项目方
    Project,
    /// 服务提供商
    ServiceProvider,
    /// 基金
    Fund,
    /// 自定义类型
    Custom(u8),
}

impl EntityType {
    /// 默认治理模式（创建实体时的建议值）
    pub fn default_governance(&self) -> GovernanceMode {
        match self {
            Self::DAO | Self::Enterprise | Self::Project | Self::Fund => GovernanceMode::FullDAO,
            _ => GovernanceMode::None,
        }
    }
    
    /// 默认代币类型（创建实体时的建议值）
    pub fn default_token_type(&self) -> TokenType {
        match self {
            Self::Merchant | Self::ServiceProvider => TokenType::Points,
            Self::Enterprise => TokenType::Equity,
            Self::DAO => TokenType::Governance,
            Self::Community => TokenType::Membership,
            Self::Project => TokenType::Share,
            Self::Fund => TokenType::Share,
            Self::Custom(_) => TokenType::Points,
        }
    }
    
    /// 是否默认需要 KYC
    pub fn requires_kyc_by_default(&self) -> bool {
        matches!(self, Self::Enterprise | Self::Fund | Self::Project)
    }
    
    /// 检查代币类型是否与实体类型匹配（仅为建议，不强制）
    /// 返回 true 表示推荐组合，false 表示不常见组合但仍允许
    pub fn suggests_token_type(&self, token: &TokenType) -> bool {
        match (self, token) {
            // 商户/服务商通常不发行证券类代币
            (Self::Merchant | Self::ServiceProvider, TokenType::Equity | TokenType::Bond) => false,
            // DAO 通常使用治理代币
            (Self::DAO, TokenType::Points | TokenType::Membership) => false,
            // 基金通常使用份额类代币
            (Self::Fund, TokenType::Points | TokenType::Governance) => false,
            _ => true,
        }
    }
    
    /// 检查治理模式是否与实体类型匹配（仅为建议，不强制）
    /// 返回 true 表示推荐组合，false 表示不常见组合但仍允许
    pub fn suggests_governance(&self, mode: &GovernanceMode) -> bool {
        match (self, mode) {
            // DAO 通常需要治理机制
            (Self::DAO, GovernanceMode::None) => false,
            // 基金通常需要专业管理
            (Self::Fund, GovernanceMode::FullDAO) => false,
            _ => true,
        }
    }
    
    /// 默认转账限制模式
    pub fn default_transfer_restriction(&self) -> TransferRestrictionMode {
        match self {
            Self::Merchant | Self::ServiceProvider | Self::Community => TransferRestrictionMode::None,
            Self::Enterprise | Self::Fund => TransferRestrictionMode::Whitelist,
            Self::DAO => TransferRestrictionMode::None,
            Self::Project => TransferRestrictionMode::KycRequired,
            Self::Custom(_) => TransferRestrictionMode::None,
        }
    }
}

/// 治理模式
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum GovernanceMode {
    /// 无治理（管理员全权控制，可 lock_governance 锁定参数）
    #[default]
    None,
    /// 完全 DAO（所有决策需代币投票，可选管理员否决权）
    FullDAO,
}

// ============================================================================
// 实体相关类型
// ============================================================================

/// 实体状态（Entity 组织层状态）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum EntityStatus {
    /// 待审核（reopen_entity 重新开业时使用，新建 create_entity 跳过此状态直接 Active）
    #[default]
    Pending,
    /// 正常运营
    Active,
    /// 暂停运营（管理员主动）
    Suspended,
    /// 被封禁（治理处罚）
    Banned,
    /// 已关闭
    Closed,
    /// 待关闭审批（owner 申请关闭，等待治理批准）
    PendingClose,
}

impl EntityStatus {
    /// 是否正常运营
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// 是否为终态（不可恢复）
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Banned | Self::Closed)
    }

    /// 是否可正常运营（仅 Active）
    pub fn can_operate(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// 是否处于待定状态
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending | Self::PendingClose)
    }
}

// ============================================================================
// Shop 有效状态（实时计算，不存储）
// ============================================================================

/// Shop 有效运营状态（由 EntityStatus + ShopOperatingStatus 实时计算）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum EffectiveShopStatus {
    /// 正常营业
    Active,
    /// Shop 自身暂停（manager 主动）
    PausedBySelf,
    /// Entity 暂停导致不可运营
    PausedByEntity,
    /// Shop 资金耗尽
    FundDepleted,
    /// Shop 已关闭（自身关闭）
    Closed,
    /// Entity 已关闭/封禁，Shop 强制关闭
    ClosedByEntity,
    /// Shop 关闭中（宽限期）
    Closing,
    /// Shop 被治理层封禁
    Banned,
}

impl EffectiveShopStatus {
    /// 是否可运营（接单、上架等）
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// 是否因 Entity 导致不可用
    pub fn is_entity_caused(&self) -> bool {
        matches!(self, Self::PausedByEntity | Self::ClosedByEntity)
    }

    /// 计算有效状态
    pub fn compute(entity_status: &EntityStatus, shop_status: &ShopOperatingStatus) -> Self {
        // 1. Entity 终态优先（Banned/Closed → Shop 强制关闭）
        match entity_status {
            EntityStatus::Banned | EntityStatus::Closed => {
                return Self::ClosedByEntity;
            }
            EntityStatus::Suspended | EntityStatus::PendingClose | EntityStatus::Pending => {
                // Entity 非 Active → Shop 不可运营
                // 如果 Shop 自身已 Closed/Closing，优先显示对应状态（终态不可逆）
                if matches!(shop_status, ShopOperatingStatus::Closed) {
                    return Self::Closed;
                }
                if matches!(shop_status, ShopOperatingStatus::Closing) {
                    return Self::Closing;
                }
                return Self::PausedByEntity;
            }
            EntityStatus::Active => { /* 继续判断 Shop 自身状态 */ }
        }

        // 2. Entity Active → 看 Shop 自身状态
        match shop_status {
            ShopOperatingStatus::Active => Self::Active,
            ShopOperatingStatus::Paused => Self::PausedBySelf,
            ShopOperatingStatus::FundDepleted => Self::FundDepleted,
            ShopOperatingStatus::Closed => Self::Closed,
            ShopOperatingStatus::Closing => Self::Closing,
            ShopOperatingStatus::Banned => Self::Banned,
        }
    }
}

// ============================================================================
// Shop 类型枚举 (Entity-Shop 分离架构)
// ============================================================================

/// Shop 类型
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ShopType {
    /// 线上商城（默认）
    #[default]
    OnlineStore,
    /// 实体门店
    PhysicalStore,
    /// 服务网点
    ServicePoint,
    /// 仓储/自提点
    Warehouse,
    /// 加盟店
    Franchise,
    /// 快闪店/临时店
    Popup,
    /// 虚拟店铺（纯服务）
    Virtual,
}

impl ShopType {
    /// 是否需要地理位置
    pub fn requires_location(&self) -> bool {
        matches!(self, Self::PhysicalStore | Self::ServicePoint | Self::Warehouse | Self::Popup)
    }
    
    /// 是否支持实物商品
    pub fn supports_physical_products(&self) -> bool {
        matches!(self, Self::OnlineStore | Self::PhysicalStore | Self::Warehouse | Self::Franchise)
    }
    
    /// 是否支持服务类商品
    pub fn supports_services(&self) -> bool {
        matches!(self, Self::ServicePoint | Self::Virtual | Self::OnlineStore)
    }
}

/// Shop 状态（业务层状态）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ShopOperatingStatus {
    /// 营业中
    #[default]
    Active,
    /// 暂停营业
    Paused,
    /// 资金耗尽（自动暂停）
    FundDepleted,
    /// 已关闭
    Closed,
    /// 关闭中（宽限期内）
    Closing,
    /// 被治理层封禁（仅 Root 可解封）
    Banned,
}

impl ShopOperatingStatus {
    /// 是否可以进行业务操作
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Active)
    }
    
    /// 是否可以恢复营业
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Paused | Self::FundDepleted)
    }

    /// 是否处于关闭/关闭中状态（终态或准终态）
    pub fn is_closed_or_closing(&self) -> bool {
        matches!(self, Self::Closed | Self::Closing)
    }

    /// 是否被封禁
    pub fn is_banned(&self) -> bool {
        matches!(self, Self::Banned)
    }

    /// 是否处于不可管理状态（关闭/关闭中/封禁）
    pub fn is_terminal_or_banned(&self) -> bool {
        matches!(self, Self::Closed | Self::Closing | Self::Banned)
    }
}

// ============================================================================
// 会员注册策略（位标记）
// ============================================================================

/// 会员注册策略（位标记，可组合）
///
/// - `0b0000_0000` = 开放注册（默认）
/// - `PURCHASE_REQUIRED` = 必须先消费才能成为会员（手动注册被拒）
/// - `REFERRAL_REQUIRED` = 必须提供推荐人
/// - `APPROVAL_REQUIRED` = 需要 Entity owner 审批
///
/// 支持组合，例如 `PURCHASE_REQUIRED | REFERRAL_REQUIRED` = 必须消费且有推荐人
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct MemberRegistrationPolicy(pub u8);

impl MemberRegistrationPolicy {
    /// 开放注册（无限制）
    pub const OPEN: Self = Self(0);
    /// 必须先消费（auto_register 触发）才能成为会员
    pub const PURCHASE_REQUIRED: u8 = 0b0000_0001;
    /// 必须提供推荐人
    pub const REFERRAL_REQUIRED: u8 = 0b0000_0010;
    /// 需要 Entity owner 审批（注册后进入 Pending 状态）
    pub const APPROVAL_REQUIRED: u8 = 0b0000_0100;
    /// 注册时需要通过 KYC 认证
    pub const KYC_REQUIRED: u8 = 0b0000_1000;
    /// 等级升级时需要通过 KYC 认证
    pub const KYC_UPGRADE_REQUIRED: u8 = 0b0001_0000;
    /// 所有已定义标记位的并集
    pub const ALL_VALID: u8 = Self::PURCHASE_REQUIRED | Self::REFERRAL_REQUIRED | Self::APPROVAL_REQUIRED | Self::KYC_REQUIRED | Self::KYC_UPGRADE_REQUIRED;

    /// 检查是否设置了指定标记
    pub fn contains(&self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    /// 检查策略值是否仅包含已定义的位
    pub fn is_valid(&self) -> bool {
        self.0 & !Self::ALL_VALID == 0
    }

    /// 是否为开放注册（无任何限制）
    pub fn is_open(&self) -> bool {
        self.0 == 0
    }

    /// 是否要求购买
    pub fn requires_purchase(&self) -> bool {
        self.contains(Self::PURCHASE_REQUIRED)
    }

    /// 是否要求推荐人
    pub fn requires_referral(&self) -> bool {
        self.contains(Self::REFERRAL_REQUIRED)
    }

    /// 是否要求审批
    pub fn requires_approval(&self) -> bool {
        self.contains(Self::APPROVAL_REQUIRED)
    }

    /// 注册时是否要求 KYC
    pub fn requires_kyc(&self) -> bool {
        self.contains(Self::KYC_REQUIRED)
    }

    /// 升级时是否要求 KYC
    pub fn requires_kyc_for_upgrade(&self) -> bool {
        self.contains(Self::KYC_UPGRADE_REQUIRED)
    }
}

impl Default for MemberRegistrationPolicy {
    fn default() -> Self {
        Self::OPEN
    }
}

// ============================================================================
// 会员统计策略 (MemberStatsPolicy)
// ============================================================================

/// 会员统计策略（位标记）
///
/// 控制业务逻辑中推荐人数的计算口径（升级规则、佣金门槛等）
///
/// - 默认值 `0b00` = 使用有效推荐人数（排除复购赠与注册）
/// - 置位对应标记 = 将复购赠与注册的账户也计入推荐人数
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct MemberStatsPolicy(pub u8);

impl MemberStatsPolicy {
    /// 直推人数包含复购赠与（置位 = direct_referrals，不置位 = qualified_referrals）
    pub const INCLUDE_REPURCHASE_DIRECT: u8 = 0b0000_0001;
    /// 间接推荐人数包含复购赠与（置位 = indirect_referrals，不置位 = qualified_indirect_referrals）
    pub const INCLUDE_REPURCHASE_INDIRECT: u8 = 0b0000_0010;
    /// 所有已定义标记位的并集
    pub const ALL_VALID: u8 = Self::INCLUDE_REPURCHASE_DIRECT | Self::INCLUDE_REPURCHASE_INDIRECT;

    /// 检查策略值是否仅包含已定义的位
    pub fn is_valid(&self) -> bool {
        self.0 & !Self::ALL_VALID == 0
    }

    /// 直推人数是否包含复购赠与
    pub fn include_repurchase_direct(&self) -> bool {
        self.0 & Self::INCLUDE_REPURCHASE_DIRECT != 0
    }

    /// 间接推荐人数是否包含复购赠与
    pub fn include_repurchase_indirect(&self) -> bool {
        self.0 & Self::INCLUDE_REPURCHASE_INDIRECT != 0
    }
}

impl Default for MemberStatsPolicy {
    fn default() -> Self {
        Self(0) // 默认：排除复购（安全默认值）
    }
}

// ============================================================================
// 通证类型枚举 (Phase 2 新增)
// ============================================================================

/// 通证类型
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum TokenType {
    /// 积分（原默认类型，消费奖励）
    #[default]
    Points,
    /// 治理代币（投票权）
    Governance,
    /// 股权代币（分红权）
    Equity,
    /// 会员代币（会员资格）
    Membership,
    /// 份额代币（基金份额）
    Share,
    /// 债券代币（固定收益）
    Bond,
    /// 混合型（多种权益）
    Hybrid(u8),
}

impl TokenType {
    /// 是否具有投票权
    pub fn has_voting_power(&self) -> bool {
        matches!(self, Self::Governance | Self::Equity | Self::Hybrid(_))
    }
    
    /// 是否具有分红权
    pub fn has_dividend_rights(&self) -> bool {
        matches!(self, Self::Equity | Self::Share | Self::Hybrid(_))
    }
    
    /// 是否可转让（默认可转让）
    pub fn is_transferable_by_default(&self) -> bool {
        !matches!(self, Self::Membership)
    }
    
    /// 获取默认要求的 KYC 级别
    /// 返回 (持有者 KYC, 接收方 KYC)
    pub fn required_kyc_level(&self) -> (u8, u8) {
        match self {
            Self::Points => (0, 0),           // None, None
            Self::Membership => (1, 1),       // Basic, Basic
            Self::Governance => (2, 2),       // Standard, Standard
            Self::Share | Self::Bond => (2, 2), // Standard, Standard
            Self::Equity => (3, 3),           // Enhanced, Enhanced
            Self::Hybrid(_) => (2, 2),        // Standard, Standard (默认)
        }
    }
    
    /// 是否为证券类型（需要严格合规）
    pub fn is_security(&self) -> bool {
        matches!(self, Self::Equity | Self::Share | Self::Bond)
    }
    
    /// 是否需要强制披露
    pub fn requires_disclosure(&self) -> bool {
        matches!(self, Self::Equity | Self::Share | Self::Bond)
    }
    
    /// 默认转账限制模式
    pub fn default_transfer_restriction(&self) -> TransferRestrictionMode {
        match self {
            Self::Points => TransferRestrictionMode::None,
            Self::Membership => TransferRestrictionMode::MembersOnly,
            Self::Governance => TransferRestrictionMode::KycRequired,
            Self::Share | Self::Bond => TransferRestrictionMode::KycRequired,
            Self::Equity => TransferRestrictionMode::Whitelist,
            Self::Hybrid(_) => TransferRestrictionMode::None,
        }
    }
}

/// 转账限制模式
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum TransferRestrictionMode {
    /// 无限制（默认）
    #[default]
    None,
    /// 白名单模式 - 只能转给白名单地址
    Whitelist,
    /// 黑名单模式 - 禁止转给黑名单地址
    Blacklist,
    /// KYC 模式 - 接收方需满足 KYC 要求
    KycRequired,
    /// 闭环模式 - 只能在实体成员间转账
    MembersOnly,
}

impl TransferRestrictionMode {
    /// 从 u8 转换（未知值回退到 None）
    #[deprecated(note = "使用 try_from_u8 代替，from_u8 会将未知值静默回退到最宽松模式")]
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Whitelist,
            2 => Self::Blacklist,
            3 => Self::KycRequired,
            4 => Self::MembersOnly,
            _ => Self::None,
        }
    }

    /// 安全转换（未知值返回 None 而非静默回退）
    pub fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Whitelist),
            2 => Some(Self::Blacklist),
            3 => Some(Self::KycRequired),
            4 => Some(Self::MembersOnly),
            _ => Option::None,
        }
    }
}

/// 分红配置
///
/// **设计说明：** `last_distribution` 和 `accumulated` 为运行时状态字段，
/// 理想情况下应作为独立存储项放在 `pallet-entity-token` 中。
/// 因当前已嵌入 `EntityTokenConfig` 存储结构，修改需存储迁移，暂保留。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct DividendConfig<Balance, BlockNumber> {
    /// 是否启用分红
    pub enabled: bool,
    /// 最小分红周期（区块数）
    pub min_period: BlockNumber,
    /// 上次分红时间
    pub last_distribution: BlockNumber,
    /// 累计待分配金额
    pub accumulated: Balance,
}

// ============================================================================
// 服务/商品相关类型
// ============================================================================

/// 商品状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ProductStatus {
    /// 草稿（未上架）
    #[default]
    Draft,
    /// 在售
    OnSale,
    /// 售罄
    SoldOut,
    /// 已下架
    OffShelf,
}

/// 商品可见性
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ProductVisibility {
    /// 公开（所有人可见）
    #[default]
    Public,
    /// 仅会员可见/可购买
    MembersOnly,
    /// 等级门槛（达到指定等级才能购买）
    LevelGated(u8),
}

/// 商品类别
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum ProductCategory {
    /// 数字商品（虚拟物品）
    Digital,
    /// 实物商品
    #[default]
    Physical,
    /// 服务类
    Service,
    /// 订阅制商品（周期性付费）
    Subscription,
    /// 组合包（多个商品打包）
    Bundle,
    /// 其他
    Other,
}

// ============================================================================
// 订单相关类型
// ============================================================================

/// 订单状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum OrderStatus {
    /// 已创建，待支付
    #[default]
    Created,
    /// 已支付，待发货
    Paid,
    /// 已发货，待收货
    Shipped,
    /// 已完成
    Completed,
    /// 已取消（买家取消）
    Cancelled,
    /// 争议中
    Disputed,
    /// 已退款
    Refunded,
    /// 已过期（支付超时）
    Expired,
}

// ============================================================================
// Admin 权限位掩码
// ============================================================================

/// Admin 细粒度权限（位掩码模式）
///
/// 每个 admin 绑定一个 `u32` 权限值，通过 `&` 运算检查是否拥有特定权限。
/// Owner 天然拥有全部权限，不受此掩码限制。
#[allow(non_snake_case)]
pub mod AdminPermission {
    /// Shop 管理（创建/更新/暂停 Shop、产品管理）
    pub const SHOP_MANAGE: u32     = 0b0000_0001;
    /// 会员等级管理（等级系统、升级规则、会员审批）
    pub const MEMBER_MANAGE: u32   = 0b0000_0010;
    /// Token 发售管理（创建/结束 tokensale）
    pub const TOKEN_MANAGE: u32    = 0b0000_0100;
    /// 广告管理（广告位注册/广告活动管理）
    pub const ADS_MANAGE: u32      = 0b0000_1000;
    /// 评论管理（开关评论系统）
    pub const REVIEW_MANAGE: u32   = 0b0001_0000;
    /// 披露/公告管理（配置披露、发布公告、内幕人员管理）
    pub const DISCLOSURE_MANAGE: u32 = 0b0010_0000;
    /// 实体管理（更新实体信息、充值资金）
    pub const ENTITY_MANAGE: u32   = 0b0100_0000;
    /// KYC 要求管理（设置实体 KYC 要求配置）
    pub const KYC_MANAGE: u32      = 0b1000_0000;
    /// 治理提案管理（创建/投票/执行提案）
    pub const GOVERNANCE_MANAGE: u32 = 0b0001_0000_0000;
    /// 订单管理（退款审批、争议处理）
    pub const ORDER_MANAGE: u32      = 0b0010_0000_0000;
    /// 佣金配置管理（返佣模式、费率设置）
    pub const COMMISSION_MANAGE: u32 = 0b0100_0000_0000;
    /// 全部权限
    pub const ALL: u32             = 0xFFFF_FFFF;
    /// 所有已定义权限位的并集（用于校验合法性）
    pub const ALL_DEFINED: u32     = SHOP_MANAGE
        | MEMBER_MANAGE
        | TOKEN_MANAGE
        | ADS_MANAGE
        | REVIEW_MANAGE
        | DISCLOSURE_MANAGE
        | ENTITY_MANAGE
        | KYC_MANAGE
        | GOVERNANCE_MANAGE
        | ORDER_MANAGE
        | COMMISSION_MANAGE;

    /// 检查权限值是否仅包含已定义的位
    pub fn is_valid(permissions: u32) -> bool {
        permissions & !ALL_DEFINED == 0
    }
}

// ============================================================================
// 跨模块 Trait 接口
// ============================================================================

/// 实体查询接口
/// 
/// 供其他模块查询实体信息
pub trait EntityProvider<AccountId> {
    /// 检查实体是否存在
    fn entity_exists(entity_id: u64) -> bool;
    
    /// 检查实体是否激活
    fn is_entity_active(entity_id: u64) -> bool;
    
    /// 获取实体状态
    fn entity_status(entity_id: u64) -> Option<EntityStatus>;
    
    /// 获取实体所有者
    fn entity_owner(entity_id: u64) -> Option<AccountId>;
    
    /// 获取实体派生账户
    fn entity_account(entity_id: u64) -> AccountId;

    /// 获取实体类型
    fn entity_type(entity_id: u64) -> Option<EntityType> {
        let _ = entity_id;
        None
    }
    
    /// 更新实体统计（销售额、订单数）
    fn update_entity_stats(entity_id: u64, sales_amount: u128, order_count: u32) -> Result<(), DispatchError>;
    
    // ==================== Entity-Shop 关联接口 ====================
    
    /// 注册 Shop 到 Entity（Shop 创建时调用）
    fn register_shop(entity_id: u64, shop_id: u64) -> Result<(), DispatchError> {
        let _ = (entity_id, shop_id);
        Ok(())
    }
    
    /// 从 Entity 注销 Shop（Shop 关闭时调用）
    fn unregister_shop(entity_id: u64, shop_id: u64) -> Result<(), DispatchError> {
        let _ = (entity_id, shop_id);
        Ok(())
    }
    
    /// 检查是否为 Entity 管理员且拥有指定权限
    ///
    /// `required_permission` 为 `AdminPermission` 位掩码，owner 天然通过。
    fn is_entity_admin(entity_id: u64, account: &AccountId, required_permission: u32) -> bool {
        let _ = (entity_id, account, required_permission);
        false
    }
    
    /// 获取 Entity 下所有 Shop IDs
    fn entity_shops(entity_id: u64) -> sp_std::vec::Vec<u64> {
        let _ = entity_id;
        sp_std::vec::Vec::new()
    }
    
    // ==================== 治理调用接口 ====================
    
    /// 暂停实体（治理调用）
    fn pause_entity(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Ok(()) // 默认空实现
    }
    
    /// 恢复实体（治理调用）
    fn resume_entity(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Ok(())
    }

    /// 设置实体治理模式（治理 pallet 同步调用）
    fn set_governance_mode(entity_id: u64, mode: GovernanceMode) -> Result<(), DispatchError> {
        let _ = (entity_id, mode);
        Ok(())
    }

    /// 实体是否被全局锁定（governance lock 生效时返回 true）
    ///
    /// 锁定后所有 Owner/Admin 配置操作被拒绝，不可逆。
    fn is_entity_locked(entity_id: u64) -> bool {
        let _ = entity_id;
        false
    }
}


// ============================================================================
// Shop 查询接口 (Entity-Shop 分离架构)
// ============================================================================

/// Shop 查询接口
/// 
/// 供业务模块查询 Shop 信息（与 EntityProvider 区分）
pub trait ShopProvider<AccountId> {
    /// 检查 Shop 是否存在
    fn shop_exists(shop_id: u64) -> bool;
    
    /// 检查 Shop 是否营业中
    fn is_shop_active(shop_id: u64) -> bool;
    
    /// 获取 Shop 所属 Entity ID
    fn shop_entity_id(shop_id: u64) -> Option<u64>;
    
    /// 获取 Shop 所有者（通过 Entity 查询）
    fn shop_owner(shop_id: u64) -> Option<AccountId>;
    
    /// 获取 Shop 运营账户
    fn shop_account(shop_id: u64) -> AccountId;
    
    /// 获取 Shop 类型
    fn shop_type(shop_id: u64) -> Option<ShopType>;
    
    /// 检查是否为 Shop 管理员
    fn is_shop_manager(shop_id: u64, account: &AccountId) -> bool;
    
    // ==================== 统计更新 ====================
    
    /// 更新 Shop 统计（销售额、订单数）
    fn update_shop_stats(shop_id: u64, sales_amount: u128, order_count: u32) -> Result<(), DispatchError>;
    
    /// 更新 Shop 评分
    fn update_shop_rating(shop_id: u64, rating: u8) -> Result<(), DispatchError>;
    
    // ==================== 商品统计 ====================
    
    /// 增加 Shop 商品计数（创建商品时调用）
    fn increment_product_count(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }
    
    /// 减少 Shop 商品计数（删除商品时调用）
    fn decrement_product_count(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }
    
    // ==================== 运营资金 ====================
    
    /// 扣减运营资金
    fn deduct_operating_fund(shop_id: u64, amount: u128) -> Result<(), DispatchError>;
    
    /// 获取运营资金余额
    fn operating_balance(shop_id: u64) -> u128;
    
    // ==================== Primary Shop ====================
    
    /// 创建主 Shop（Entity 创建时自动调用，绕过 is_entity_active 检查）
    fn create_primary_shop(
        entity_id: u64,
        name: sp_std::vec::Vec<u8>,
        shop_type: ShopType,
    ) -> Result<u64, DispatchError> {
        let _ = (entity_id, name, shop_type);
        Err(DispatchError::Other("not implemented"))
    }
    
    /// 检查 Shop 是否为主 Shop
    fn is_primary_shop(shop_id: u64) -> bool {
        let _ = shop_id;
        false
    }
    
    // ==================== 控制接口 ====================
    
    /// 获取 Shop 自身状态（不考虑 Entity）
    fn shop_own_status(shop_id: u64) -> Option<ShopOperatingStatus> {
        let _ = shop_id;
        None
    }
    
    /// 获取 Shop 有效状态（考虑 Entity 状态）
    fn effective_status(shop_id: u64) -> Option<EffectiveShopStatus> {
        let _ = shop_id;
        None
    }
    
    /// 暂停 Shop
    fn pause_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }
    
    /// 恢复 Shop
    fn resume_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }
    
    /// 强制关闭 Shop（Entity 级联调用，绕过 is_primary 检查）
    fn force_close_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }

    /// 强制暂停 Shop（治理层调用，可被 owner 恢复）
    fn force_pause_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }
}

/// 商品查询接口
/// 
/// 供 order 模块查询和更新商品信息
pub trait ProductProvider<AccountId, Balance> {
    /// 检查商品是否存在
    fn product_exists(product_id: u64) -> bool;
    
    /// 检查商品是否在售
    fn is_product_on_sale(product_id: u64) -> bool;
    
    /// 获取商品所属店铺
    fn product_shop_id(product_id: u64) -> Option<u64>;
    
    /// 获取商品价格
    fn product_price(product_id: u64) -> Option<Balance>;
    
    /// 获取商品库存
    fn product_stock(product_id: u64) -> Option<u32>;
    
    /// 获取商品类别
    fn product_category(product_id: u64) -> Option<ProductCategory>;
    
    /// 扣减库存
    fn deduct_stock(product_id: u64, quantity: u32) -> Result<(), DispatchError>;
    
    /// 恢复库存
    fn restore_stock(product_id: u64, quantity: u32) -> Result<(), DispatchError>;
    
    /// 增加销量
    fn add_sold_count(product_id: u64, quantity: u32) -> Result<(), DispatchError>;

    // ==================== 扩展查询接口 ====================

    /// 获取商品状态
    fn product_status(product_id: u64) -> Option<ProductStatus> {
        let _ = product_id;
        None
    }

    /// 获取商品 USDT 价格（精度 10^6）
    fn product_usdt_price(product_id: u64) -> Option<u64> {
        let _ = product_id;
        None
    }

    /// 获取商品所有者（通过 Shop → Owner）
    fn product_owner(product_id: u64) -> Option<AccountId> {
        let _ = product_id;
        None
    }

    /// 获取店铺下所有商品 ID
    fn shop_product_ids(shop_id: u64) -> sp_std::vec::Vec<u64> {
        let _ = shop_id;
        sp_std::vec::Vec::new()
    }

    /// 获取商品可见性
    fn product_visibility(product_id: u64) -> Option<ProductVisibility> {
        let _ = product_id;
        None
    }

    /// 获取商品最小购买数量（0 表示不限，默认 1）
    fn product_min_order_quantity(product_id: u64) -> Option<u32> {
        let _ = product_id;
        None
    }

    /// 获取商品最大购买数量（0 表示不限）
    fn product_max_order_quantity(product_id: u64) -> Option<u32> {
        let _ = product_id;
        None
    }

    // ==================== 治理调用接口 ====================
    
    /// 更新商品价格（治理调用）
    fn update_price(product_id: u64, new_price: Balance) -> Result<(), DispatchError> {
        let _ = (product_id, new_price);
        Ok(())
    }
    
    /// 下架商品（治理调用）
    fn delist_product(product_id: u64) -> Result<(), DispatchError> {
        let _ = product_id;
        Ok(())
    }
    
    /// 调整库存（治理调用）
    fn set_inventory(product_id: u64, new_inventory: u32) -> Result<(), DispatchError> {
        let _ = (product_id, new_inventory);
        Ok(())
    }
}

/// 订单查询接口
/// 
/// 供 review 模块查询订单信息
pub trait OrderProvider<AccountId, Balance> {
    /// 检查订单是否存在
    fn order_exists(order_id: u64) -> bool;
    
    /// 获取订单买家
    fn order_buyer(order_id: u64) -> Option<AccountId>;
    
    /// 获取订单卖家
    fn order_seller(order_id: u64) -> Option<AccountId>;
    
    /// 获取订单总金额
    fn order_amount(order_id: u64) -> Option<Balance>;
    
    /// 获取订单店铺
    fn order_shop_id(order_id: u64) -> Option<u64>;
    
    /// 检查订单是否已完成
    fn is_order_completed(order_id: u64) -> bool;
    
    /// 检查订单是否处于争议状态
    fn is_order_disputed(order_id: u64) -> bool;
    
    /// 检查用户是否可以对该订单发起争议（必须是买家或卖家，且订单状态允许）
    fn can_dispute(order_id: u64, who: &AccountId) -> bool;

    /// 获取订单 Token 支付金额（u128，Token 订单返回实际值，NEX 订单返回 0）
    fn order_token_amount(order_id: u64) -> Option<u128> {
        let _ = order_id;
        None
    }

    /// 获取订单支付资产类型（Native / EntityToken）
    fn order_payment_asset(order_id: u64) -> Option<PaymentAsset> {
        let _ = order_id;
        None
    }

    /// 获取订单完成时间（区块号，u64 表示）
    fn order_completed_at(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }
}

// ============================================================================
// 空实现（用于测试）
// ============================================================================

/// 空实体提供者（测试用）
pub struct NullEntityProvider;

impl<AccountId: Default> EntityProvider<AccountId> for NullEntityProvider {
    fn entity_exists(_entity_id: u64) -> bool { false }
    fn is_entity_active(_entity_id: u64) -> bool { false }
    fn entity_status(_entity_id: u64) -> Option<EntityStatus> { None }
    fn entity_owner(_entity_id: u64) -> Option<AccountId> { None }
    fn entity_account(_entity_id: u64) -> AccountId { AccountId::default() }
    fn update_entity_stats(_entity_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), DispatchError> { Ok(()) }
}

/// 空 Shop 提供者（测试用）
pub struct NullShopProvider;

impl<AccountId: Default> ShopProvider<AccountId> for NullShopProvider {
    fn shop_exists(_shop_id: u64) -> bool { false }
    fn is_shop_active(_shop_id: u64) -> bool { false }
    fn shop_entity_id(_shop_id: u64) -> Option<u64> { None }
    fn shop_owner(_shop_id: u64) -> Option<AccountId> { None }
    fn shop_account(_shop_id: u64) -> AccountId { AccountId::default() }
    fn shop_type(_shop_id: u64) -> Option<ShopType> { None }
    fn is_shop_manager(_shop_id: u64, _account: &AccountId) -> bool { false }
    fn shop_own_status(_shop_id: u64) -> Option<ShopOperatingStatus> { None }
    fn effective_status(_shop_id: u64) -> Option<EffectiveShopStatus> { None }
    fn update_shop_stats(_shop_id: u64, _sales_amount: u128, _order_count: u32) -> Result<(), DispatchError> { Ok(()) }
    fn update_shop_rating(_shop_id: u64, _rating: u8) -> Result<(), DispatchError> { Ok(()) }
    fn deduct_operating_fund(_shop_id: u64, _amount: u128) -> Result<(), DispatchError> { Ok(()) }
    fn operating_balance(_shop_id: u64) -> u128 { 0 }
}

/// 空商品提供者（测试用）
pub struct NullProductProvider;

impl<AccountId, Balance> ProductProvider<AccountId, Balance> for NullProductProvider {
    fn product_exists(_product_id: u64) -> bool { false }
    fn is_product_on_sale(_product_id: u64) -> bool { false }
    fn product_shop_id(_product_id: u64) -> Option<u64> { None }
    fn product_price(_product_id: u64) -> Option<Balance> { None }
    fn product_stock(_product_id: u64) -> Option<u32> { None }
    fn product_category(_product_id: u64) -> Option<ProductCategory> { None }
    fn deduct_stock(_product_id: u64, _quantity: u32) -> Result<(), DispatchError> { Ok(()) }
    fn restore_stock(_product_id: u64, _quantity: u32) -> Result<(), DispatchError> { Ok(()) }
    fn add_sold_count(_product_id: u64, _quantity: u32) -> Result<(), DispatchError> { Ok(()) }
}

/// 空订单提供者（测试用）
pub struct NullOrderProvider;

impl<AccountId, Balance> OrderProvider<AccountId, Balance> for NullOrderProvider {
    fn order_exists(_order_id: u64) -> bool { false }
    fn order_buyer(_order_id: u64) -> Option<AccountId> { None }
    fn order_seller(_order_id: u64) -> Option<AccountId> { None }
    fn order_amount(_order_id: u64) -> Option<Balance> { None }
    fn order_shop_id(_order_id: u64) -> Option<u64> { None }
    fn is_order_completed(_order_id: u64) -> bool { false }
    fn is_order_disputed(_order_id: u64) -> bool { false }
    fn can_dispute(_order_id: u64, _who: &AccountId) -> bool { false }
    fn order_token_amount(_order_id: u64) -> Option<u128> { None }
    fn order_payment_asset(_order_id: u64) -> Option<PaymentAsset> { None }
}

// ============================================================================
// 实体代币接口
// ============================================================================

/// 实体代币接口
/// 
/// 供 order 模块调用，实现购物返积分和积分抵扣
pub trait EntityTokenProvider<AccountId, Balance> {
    /// 检查实体是否启用代币
    fn is_token_enabled(entity_id: u64) -> bool;
    
    /// 获取用户代币余额
    fn token_balance(entity_id: u64, holder: &AccountId) -> Balance;
    
    /// 购物奖励（订单完成时调用）
    fn reward_on_purchase(
        entity_id: u64,
        buyer: &AccountId,
        purchase_amount: Balance,
    ) -> Result<Balance, DispatchError>;
    
    /// 代币兑换折扣（下单时调用）
    fn redeem_for_discount(
        entity_id: u64,
        buyer: &AccountId,
        tokens: Balance,
    ) -> Result<Balance, DispatchError>;
    
    /// 转移代币（P2P 交易市场使用）
    fn transfer(
        entity_id: u64,
        from: &AccountId,
        to: &AccountId,
        amount: Balance,
    ) -> Result<(), DispatchError>;
    
    /// 锁定代币（挂单时使用）
    fn reserve(
        entity_id: u64,
        who: &AccountId,
        amount: Balance,
    ) -> Result<(), DispatchError>;
    
    /// 解锁代币（取消订单时使用）
    fn unreserve(
        entity_id: u64,
        who: &AccountId,
        amount: Balance,
    ) -> Balance;
    
    /// 从锁定中转移（成交时使用）
    fn repatriate_reserved(
        entity_id: u64,
        from: &AccountId,
        to: &AccountId,
        amount: Balance,
    ) -> Result<Balance, DispatchError>;
    
    /// Phase 8: 获取代币类型
    fn get_token_type(entity_id: u64) -> TokenType;
    
    /// Phase 8: 获取代币总供应量
    fn total_supply(entity_id: u64) -> Balance;

    /// H4: 治理提案销毁代币（从 entity 派生账户销毁）
    fn governance_burn(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;
}

/// 空实体代币提供者（测试用或未启用代币时）
pub struct NullEntityTokenProvider;

// ============================================================================
// 定价接口
// ============================================================================

/// NEX/USDT 价格查询接口
/// 
/// 供 shop 模块计算 USDT 等值的 NEX 押金
pub trait PricingProvider {
    /// 获取 NEX/USDT 加权平均价格
    /// 
    /// # 返回
    /// - `u64`: 价格（精度 10^6，即 1,000,000 = 1 USDT/NEX）
    /// - 返回 0 表示价格不可用
    fn get_nex_usdt_price() -> u64;

    /// 价格数据是否过时
    ///
    /// # 说明
    /// 若市场长期无交易，价格可能严重偏离真实值。
    /// 消费方应在使用价格前检查此标志，过时时使用兜底值。
    ///
    /// # 默认实现
    /// 返回 `false`（向后兼容，不影响现有模块）
    fn is_price_stale() -> bool { false }
}

// ============================================================================
// 实体代币价格查询接口
// ============================================================================

/// 实体代币当前价格查询接口
///
/// 供需要获取 Entity Token 价格的模块使用（佣金换算、分红定价、前端展示等）。
///
/// ## 价格单位
/// - `get_token_price`: NEX per Token（精度 10^12，链上原生代币单位）
/// - `get_token_price_usdt`: USDT per Token（精度 10^6，通过 NEX 价格间接换算）
///
/// ## 置信度等级
/// - 90-100: TWAP 可用 + 高交易量（≥100 笔）
/// - 60-89:  TWAP 或 LastTradePrice 可用
/// - 30-59:  仅 initial_price（冷启动期）
/// - 0-29:   价格过时或不可用
///
/// ## 注意
/// Entity Token 价格由 entity owner 可影响（set_initial_price + 低流动性自买自卖），
/// **不应用于安全关键的押金/保证金计算**，仅适用于展示和非关键换算。
pub trait EntityTokenPriceProvider {
    type Balance;

    /// 获取代币当前价格（NEX per Token, 精度 10^12）
    ///
    /// 优先级：1h TWAP → LastTradePrice → initial_price
    /// 返回 `None` 表示无任何价格数据
    fn get_token_price(entity_id: u64) -> Option<Self::Balance>;

    /// 获取代币 USDT 计价（精度 10^6）
    ///
    /// 通过 token_nex_price × nex_usdt_rate / 10^12 间接换算
    /// 返回 `None` 表示价格不可用（Token 或 NEX/USDT 价格缺失）
    fn get_token_price_usdt(entity_id: u64) -> Option<u64>;

    /// 价格置信度 (0-100)
    ///
    /// 基于数据来源、交易量和新鲜度综合评估
    fn token_price_confidence(entity_id: u64) -> u8;

    /// 价格数据是否过时（超过 max_age_blocks 个区块未更新）
    fn is_token_price_stale(entity_id: u64, max_age_blocks: u32) -> bool;

    /// 价格是否可信赖（置信度 >= 阈值）
    ///
    /// 默认阈值 30
    fn is_token_price_reliable(entity_id: u64) -> bool {
        Self::token_price_confidence(entity_id) >= 30
    }
}

/// EntityTokenPriceProvider 的空实现（无市场时使用）
impl EntityTokenPriceProvider for () {
    type Balance = u128;
    fn get_token_price(_entity_id: u64) -> Option<u128> { None }
    fn get_token_price_usdt(_entity_id: u64) -> Option<u64> { None }
    fn token_price_confidence(_entity_id: u64) -> u8 { 0 }
    fn is_token_price_stale(_entity_id: u64, _max_age_blocks: u32) -> bool { true }
}

// ============================================================================
// 披露接口
// ============================================================================

/// 披露级别（跨模块共享）
///
/// 由 pallet-entity-disclosure 设置，供 token/market 等模块查询
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default, PartialOrd, Ord)]
pub enum DisclosureLevel {
    /// 基础披露（年度简报）
    #[default]
    Basic,
    /// 标准披露（季度报告）
    Standard,
    /// 增强披露（月度报告 + 重大事件）
    Enhanced,
    /// 完全披露（实时 + 详细财务）
    Full,
}

/// 披露查询接口
///
/// 供 token/market 等模块在交易前检查黑窗口期和内幕人员限制，
/// 无需直接依赖 pallet-entity-disclosure。
pub trait DisclosureProvider<AccountId> {
    /// 检查实体是否处于黑窗口期
    fn is_in_blackout(entity_id: u64) -> bool;

    /// 检查账户是否是内幕人员
    fn is_insider(entity_id: u64, account: &AccountId) -> bool;

    /// 检查内幕人员是否可以交易
    ///
    /// 非内幕人员始终返回 true；内幕人员在黑窗口期内且启用控制时返回 false
    fn can_insider_trade(entity_id: u64, account: &AccountId) -> bool;

    /// 获取实体的披露级别
    fn get_disclosure_level(entity_id: u64) -> DisclosureLevel;

    /// 检查披露是否逾期
    fn is_disclosure_overdue(entity_id: u64) -> bool;
}

/// 空披露提供者（测试用或未启用披露时）
pub struct NullDisclosureProvider;

impl<AccountId> DisclosureProvider<AccountId> for NullDisclosureProvider {
    fn is_in_blackout(_entity_id: u64) -> bool { false }
    fn is_insider(_entity_id: u64, _account: &AccountId) -> bool { false }
    fn can_insider_trade(_entity_id: u64, _account: &AccountId) -> bool { true }
    fn get_disclosure_level(_entity_id: u64) -> DisclosureLevel { DisclosureLevel::Basic }
    fn is_disclosure_overdue(_entity_id: u64) -> bool { false }
}

// ============================================================================
// KYC 查询接口
// ============================================================================

/// KYC 查询接口
///
/// 供其他模块查询用户 KYC 状态，无需直接依赖 pallet-entity-kyc。
pub trait KycProvider<AccountId> {
    /// 获取用户在指定实体下的 KYC 级别（0 = 未认证）
    fn kyc_level(entity_id: u64, account: &AccountId) -> u8;

    /// 用户是否已通过 KYC 认证（level >= 1）
    fn is_kyc_approved(entity_id: u64, account: &AccountId) -> bool {
        Self::kyc_level(entity_id, account) >= 1
    }

    /// 用户是否满足指定 KYC 级别要求
    fn meets_kyc_requirement(entity_id: u64, account: &AccountId, required_level: u8) -> bool {
        Self::kyc_level(entity_id, account) >= required_level
    }
}

/// 空 KYC 提供者（测试用或未启用 KYC 时）
pub struct NullKycProvider;

impl<AccountId> KycProvider<AccountId> for NullKycProvider {
    fn kyc_level(_entity_id: u64, _account: &AccountId) -> u8 { 0 }
}

// ============================================================================
// 治理查询接口
// ============================================================================

/// 治理查询接口
///
/// 供其他模块查询实体治理状态，无需直接依赖 pallet-entity-governance。
pub trait GovernanceProvider {
    /// 获取实体治理模式
    fn governance_mode(entity_id: u64) -> GovernanceMode;

    /// 实体是否有活跃提案
    fn has_active_proposals(entity_id: u64) -> bool;

    /// 实体治理是否被锁定（例如重大变更期间）
    fn is_governance_locked(entity_id: u64) -> bool;
}

/// 空治理提供者（测试用或未启用治理时）
pub struct NullGovernanceProvider;

impl GovernanceProvider for NullGovernanceProvider {
    fn governance_mode(_entity_id: u64) -> GovernanceMode { GovernanceMode::None }
    fn has_active_proposals(_entity_id: u64) -> bool { false }
    fn is_governance_locked(_entity_id: u64) -> bool { false }
}

// ============================================================================
// 佣金资金保护接口
// ============================================================================

/// 佣金资金保护接口
///
/// 供 Shop 模块在扣费时查询已承诺（pending + shopping）的佣金资金，
/// 防止运营扣费侵占用户佣金。
pub trait CommissionFundGuard {
    /// 获取 entity 已承诺的佣金资金总额（pending_total + shopping_total）
    fn protected_funds(entity_id: u64) -> u128;
}

/// 空 CommissionFundGuard 实现（无佣金系统时使用）
impl CommissionFundGuard for () {
    fn protected_funds(_: u64) -> u128 { 0 }
}

/// 订单佣金处理接口
///
/// 供 Transaction 模块在订单完成时触发佣金计算，
/// 无需直接依赖 commission 模块。
pub trait OrderCommissionHandler<AccountId, Balance> {
    /// 订单完成时处理佣金
    fn on_order_completed(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        order_amount: Balance,
        platform_fee: Balance,
    ) -> Result<(), DispatchError>;

    /// 订单取消/退款时撤销佣金
    fn on_order_cancelled(order_id: u64) -> Result<(), DispatchError>;
}

/// 空佣金处理（无佣金系统时使用）
impl<AccountId, Balance> OrderCommissionHandler<AccountId, Balance> for () {
    fn on_order_completed(_: u64, _: u64, _: u64, _: &AccountId, _: Balance, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn on_order_cancelled(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// PaymentAsset — 支付资产类型
// ============================================================================

/// 订单支付资产类型
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum PaymentAsset {
    /// NEX 原生代币支付
    #[default]
    Native,
    /// Entity Token 支付
    EntityToken,
}

// ============================================================================
// TokenOrderCommissionHandler — Token 订单佣金处理接口
// ============================================================================

/// Token 订单佣金处理接口
///
/// 供 Order 模块在 Entity Token 订单完成时触发 Token 佣金计算，
/// 无需直接依赖 commission 模块。使用 u128 避免泛型膨胀。
pub trait TokenOrderCommissionHandler<AccountId> {
    /// Token 订单完成时处理 Token 佣金（双源：token_platform_fee 为 Pool A 资金）
    fn on_token_order_completed(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        token_amount: u128,
        token_platform_fee: u128,
    ) -> Result<(), DispatchError>;

    /// Token 订单取消时撤销 Token 佣金
    fn on_token_order_cancelled(order_id: u64) -> Result<(), DispatchError>;

    /// 获取 Entity 级 Token 平台费率（bps，供 entity-order 计算拆分）
    fn token_platform_fee_rate(entity_id: u64) -> u16;

    /// 获取 Entity 派生账户（Token 平台费转入目标）
    fn entity_account(entity_id: u64) -> AccountId;
}

/// 空 Token 佣金处理（无 Token 佣金系统时使用）
impl<AccountId: Default> TokenOrderCommissionHandler<AccountId> for () {
    fn on_token_order_completed(_: u64, _: u64, _: u64, _: &AccountId, _: u128, _: u128) -> Result<(), DispatchError> { Ok(()) }
    fn on_token_order_cancelled(_: u64) -> Result<(), DispatchError> { Ok(()) }
    fn token_platform_fee_rate(_: u64) -> u16 { 0 }
    fn entity_account(_: u64) -> AccountId { AccountId::default() }
}

// ============================================================================
// 购物余额接口
// ============================================================================

/// 购物余额提供者（供 Transaction 模块在下单时抵扣购物余额）
///
/// `consume_shopping_balance` 会：
/// 1. 扣减会员购物余额记账（MemberShoppingBalance / ShopShoppingTotal）
/// 2. 将等额 NEX 从 Entity 账户转入会员钱包（会员随后通过 Escrow 锁定）
pub trait ShoppingBalanceProvider<AccountId, Balance> {
    /// 查询会员在指定实体的购物余额
    fn shopping_balance(entity_id: u64, account: &AccountId) -> Balance;
    /// 消费购物余额：扣减记账 + 将 NEX 从 Entity 账户转入会员钱包
    fn consume_shopping_balance(entity_id: u64, account: &AccountId, amount: Balance) -> Result<(), DispatchError>;
}

/// 空购物余额提供者（无佣金系统时使用）
impl<AccountId, Balance: Default> ShoppingBalanceProvider<AccountId, Balance> for () {
    fn shopping_balance(_: u64, _: &AccountId) -> Balance { Balance::default() }
    fn consume_shopping_balance(_: u64, _: &AccountId, _: Balance) -> Result<(), DispatchError> { Ok(()) }
}

/// 订单会员处理接口
///
/// 供 Transaction 模块在订单完成时：
/// 1. 自动注册买家为会员（如果尚未注册）
/// 2. 更新消费金额（触发等级升级）
pub trait OrderMemberHandler<AccountId> {
    /// 自动注册会员（首次下单时，推荐人可选）
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> Result<(), DispatchError>;
    /// 更新消费金额（USDT 精度 10^6）
    fn update_spent(entity_id: u64, account: &AccountId, amount_usdt: u64) -> Result<(), DispatchError>;
    /// 检查订单完成时的升级规则（amount_usdt: USDT 精度 10^6）
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, amount_usdt: u64) -> Result<(), DispatchError>;
}

/// 空会员处理（无会员系统时使用）
impl<AccountId> OrderMemberHandler<AccountId> for () {
    fn auto_register(_: u64, _: &AccountId, _: Option<AccountId>) -> Result<(), DispatchError> { Ok(()) }
    fn update_spent(_: u64, _: &AccountId, _: u64) -> Result<(), DispatchError> { Ok(()) }
    fn check_order_upgrade_rules(_: u64, _: &AccountId, _: u64, _: u64) -> Result<(), DispatchError> { Ok(()) }
}

// ============================================================================
// 会员服务接口（统一定义）
// ============================================================================

/// 会员等级信息（无泛型，适合跨模块 trait 返回）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct MemberLevelInfo {
    /// 等级 ID
    pub level_id: u8,
    /// 等级名称（UTF-8 字节）
    pub name: sp_std::vec::Vec<u8>,
    /// 升级阈值（USDT 累计消费，精度 10^6）
    pub threshold: u64,
    /// 折扣率（基点）
    pub discount_rate: u16,
    /// 返佣加成（基点）
    pub commission_bonus: u16,
}

/// 会员服务接口（供返佣、治理、订单等模块统一调用）
///
/// 由 `pallet-entity-member` 实现，通过 runtime 桥接到各消费方。
/// 合并了原 `pallet-entity-member::MemberProvider` 和 `pallet-commission-common::MemberProvider`
/// 两个重复定义，消除运行时手动桥接的冗余。
pub trait MemberProvider<AccountId> {
    // ==================== 只读查询 ====================

    /// 检查是否为实体会员
    fn is_member(entity_id: u64, account: &AccountId) -> bool;

    /// 获取推荐人
    fn get_referrer(entity_id: u64, account: &AccountId) -> Option<AccountId>;

    /// 获取自定义等级 ID
    fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;

    /// 获取有效等级（考虑过期）
    fn get_effective_level(entity_id: u64, account: &AccountId) -> u8 {
        Self::custom_level_id(entity_id, account)
    }

    /// 获取等级折扣率
    fn get_level_discount(entity_id: u64, level_id: u8) -> u16 {
        let _ = (entity_id, level_id);
        0
    }

    /// 获取等级返佣加成
    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16;

    /// 检查实体是否使用自定义等级
    fn uses_custom_levels(entity_id: u64) -> bool;

    /// 获取会员统计信息 (直推人数, 团队人数, 累计消费USDT)
    fn get_member_stats(entity_id: u64, account: &AccountId) -> (u32, u32, u128);

    /// 查询 Entity 的会员总数
    fn member_count(entity_id: u64) -> u32 {
        let _ = entity_id;
        0
    }

    /// 查询会员是否被封禁
    fn is_banned(entity_id: u64, account: &AccountId) -> bool {
        let _ = (entity_id, account);
        false
    }

    /// 查询会员最后活跃时间（区块号，0 = 未知/非会员）
    fn last_active_at(entity_id: u64, account: &AccountId) -> u64 {
        let _ = (entity_id, account);
        0
    }

    /// 获取会员当前有效等级的完整信息
    fn member_level(entity_id: u64, account: &AccountId) -> Option<MemberLevelInfo> {
        let _ = (entity_id, account);
        None
    }

    /// 查询自定义等级数量
    fn custom_level_count(entity_id: u64) -> u8 {
        let _ = entity_id;
        0
    }

    /// 查询指定等级的会员数量
    fn member_count_by_level(entity_id: u64, level_id: u8) -> u32 {
        let _ = (entity_id, level_id);
        0
    }

    /// 查询会员 USDT 累计消费（精度 10^6）
    fn get_member_spent_usdt(entity_id: u64, account: &AccountId) -> u64 {
        let _ = (entity_id, account);
        0
    }

    // ==================== 会员注册/更新 ====================

    /// 自动注册会员（首次下单时）
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> Result<(), DispatchError>;

    /// 自动注册会员（qualified 控制是否为有效直推）
    fn auto_register_qualified(entity_id: u64, account: &AccountId, referrer: Option<AccountId>, qualified: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, account, referrer, qualified);
        Ok(())
    }

    /// 更新消费金额（USDT 精度 10^6）
    fn update_spent(entity_id: u64, account: &AccountId, amount_usdt: u64) -> Result<(), DispatchError> {
        let _ = (entity_id, account, amount_usdt);
        Ok(())
    }

    /// 检查订单完成时的升级规则
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, amount_usdt: u64) -> Result<(), DispatchError> {
        let _ = (entity_id, buyer, product_id, amount_usdt);
        Ok(())
    }

    // ==================== 治理写入 ====================

    /// 启用/禁用自定义等级系统
    fn set_custom_levels_enabled(entity_id: u64, enabled: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, enabled);
        Ok(())
    }

    /// 设置升级模式
    fn set_upgrade_mode(entity_id: u64, mode: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, mode);
        Ok(())
    }

    /// 添加自定义等级
    fn add_custom_level(entity_id: u64, level_id: u8, name: &[u8], threshold: u128, discount_rate: u16, commission_bonus: u16) -> Result<(), DispatchError> {
        let _ = (entity_id, level_id, name, threshold, discount_rate, commission_bonus);
        Ok(())
    }

    /// 更新自定义等级
    fn update_custom_level(entity_id: u64, level_id: u8, name: Option<&[u8]>, threshold: Option<u128>, discount_rate: Option<u16>, commission_bonus: Option<u16>) -> Result<(), DispatchError> {
        let _ = (entity_id, level_id, name, threshold, discount_rate, commission_bonus);
        Ok(())
    }

    /// 删除自定义等级
    fn remove_custom_level(entity_id: u64, level_id: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, level_id);
        Ok(())
    }

    /// G1: 设置注册策略（治理调用）
    fn set_registration_policy(entity_id: u64, policy_bits: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, policy_bits);
        Ok(())
    }

    /// G1: 设置统计策略（治理调用）
    fn set_stats_policy(entity_id: u64, policy_bits: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, policy_bits);
        Ok(())
    }
}

/// 空会员服务提供者（测试用或未启用会员系统时）
pub struct NullMemberProvider;

impl<AccountId> MemberProvider<AccountId> for NullMemberProvider {
    fn is_member(_: u64, _: &AccountId) -> bool { false }
    fn get_referrer(_: u64, _: &AccountId) -> Option<AccountId> { None }
    fn custom_level_id(_: u64, _: &AccountId) -> u8 { 0 }
    fn get_effective_level(_: u64, _: &AccountId) -> u8 { 0 }
    fn get_level_discount(_: u64, _: u8) -> u16 { 0 }
    fn get_level_commission_bonus(_: u64, _: u8) -> u16 { 0 }
    fn uses_custom_levels(_: u64) -> bool { false }
    fn get_member_stats(_: u64, _: &AccountId) -> (u32, u32, u128) { (0, 0, 0) }
    fn auto_register(_: u64, _: &AccountId, _: Option<AccountId>) -> Result<(), DispatchError> { Ok(()) }
    fn update_spent(_: u64, _: &AccountId, _: u64) -> Result<(), DispatchError> { Ok(()) }
    fn check_order_upgrade_rules(_: u64, _: &AccountId, _: u64, _: u64) -> Result<(), DispatchError> { Ok(()) }
    fn set_custom_levels_enabled(_: u64, _: bool) -> Result<(), DispatchError> { Ok(()) }
    fn set_upgrade_mode(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn add_custom_level(_: u64, _: u8, _: &[u8], _: u128, _: u16, _: u16) -> Result<(), DispatchError> { Ok(()) }
    fn update_custom_level(_: u64, _: u8, _: Option<&[u8]>, _: Option<u128>, _: Option<u16>, _: Option<u16>) -> Result<(), DispatchError> { Ok(()) }
    fn remove_custom_level(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn set_registration_policy(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
    fn set_stats_policy(_: u64, _: u8) -> Result<(), DispatchError> { Ok(()) }
}

/// 空定价提供者（测试用）
pub struct NullPricingProvider;

impl PricingProvider for NullPricingProvider {
    fn get_nex_usdt_price() -> u64 {
        // 默认价格：0.000001 USDT/NEX（精度 10^6 = 1）
        1
    }
    fn is_price_stale() -> bool { false }
}

impl<AccountId, Balance: Default> EntityTokenProvider<AccountId, Balance> for NullEntityTokenProvider {
    fn is_token_enabled(_entity_id: u64) -> bool { false }
    fn token_balance(_entity_id: u64, _holder: &AccountId) -> Balance { Default::default() }
    fn reward_on_purchase(_: u64, _: &AccountId, _: Balance) -> Result<Balance, DispatchError> { 
        Ok(Default::default()) 
    }
    fn redeem_for_discount(_: u64, _: &AccountId, _: Balance) -> Result<Balance, DispatchError> { 
        Ok(Default::default()) 
    }
    fn transfer(_: u64, _: &AccountId, _: &AccountId, _: Balance) -> Result<(), DispatchError> {
        Ok(())
    }
    fn reserve(_: u64, _: &AccountId, _: Balance) -> Result<(), DispatchError> {
        Ok(())
    }
    fn unreserve(_: u64, _: &AccountId, _: Balance) -> Balance {
        Default::default()
    }
    fn repatriate_reserved(_: u64, _: &AccountId, _: &AccountId, _: Balance) -> Result<Balance, DispatchError> {
        Ok(Default::default())
    }
    fn get_token_type(_entity_id: u64) -> TokenType {
        TokenType::default()
    }
    fn total_supply(_entity_id: u64) -> Balance {
        Default::default()
    }
    fn governance_burn(_: u64, _: Balance) -> Result<(), DispatchError> {
        Ok(())
    }
}

