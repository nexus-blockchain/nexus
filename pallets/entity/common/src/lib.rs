//! # 实体公共模块 (pallet-entity-common)
//!
//! 定义实体各子模块共享的类型和 Trait 接口

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

#[cfg(test)]
mod tests;

// ============================================================================
// 跨模块共享错误码
// ============================================================================

/// 跨模块共享错误字符串常量
///
/// 用法: `Err(DispatchError::Other(CommonError::ENTITY_NOT_FOUND))`
/// 或配合 `ensure!`: `ensure!(cond, DispatchError::Other(CommonError::ENTITY_NOT_FOUND))`
///
/// 统一错误语义，避免 13+ 个模块各自定义 "EntityNotFound" 导致前端/监控无法聚合。
/// 注意: 这些是 `&str` 常量，仅通过 `DispatchError::Other` 使用，
/// 不如 `#[pallet::error]` 枚举支持元数据索引，适用于跨 pallet 的通用错误。
#[allow(non_snake_case)]
pub mod CommonError {
    pub const ENTITY_NOT_FOUND: &str = "EntityNotFound";
    pub const ENTITY_NOT_ACTIVE: &str = "EntityNotActive";
    pub const ENTITY_LOCKED: &str = "EntityLocked";
    pub const SHOP_NOT_FOUND: &str = "ShopNotFound";
    pub const SHOP_NOT_ACTIVE: &str = "ShopNotActive";
    pub const PRODUCT_NOT_FOUND: &str = "ProductNotFound";
    pub const ORDER_NOT_FOUND: &str = "OrderNotFound";
    pub const NOT_ENTITY_OWNER: &str = "NotEntityOwner";
    pub const NOT_ENTITY_ADMIN: &str = "NotEntityAdmin";
    pub const NOT_SHOP_MANAGER: &str = "NotShopManager";
    pub const INSUFFICIENT_PERMISSION: &str = "InsufficientPermission";
    pub const MEMBER_NOT_FOUND: &str = "MemberNotFound";
    pub const MEMBER_BANNED: &str = "MemberBanned";
    pub const KYC_REQUIRED: &str = "KycRequired";
    pub const KYC_EXPIRED: &str = "KycExpired";
    pub const TOKEN_NOT_ENABLED: &str = "TokenNotEnabled";
    pub const INSUFFICIENT_BALANCE: &str = "InsufficientBalance";
    pub const EMERGENCY_PAUSED: &str = "EmergencyPaused";
    pub const INVALID_STATUS_TRANSITION: &str = "InvalidStatusTransition";
    pub const PRICE_UNAVAILABLE: &str = "PriceUnavailable";
}

// ============================================================================
// 标准化分页类型
// ============================================================================

/// 分页请求参数
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct PageRequest {
    /// 起始偏移量（0-indexed）
    pub offset: u32,
    /// 每页数量（上限由各接口自行限制）
    pub limit: u32,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self { offset: 0, limit: 20 }
    }
}

impl PageRequest {
    /// 创建分页请求
    pub fn new(offset: u32, limit: u32) -> Self {
        Self { offset, limit }
    }

    /// 限制 limit 不超过最大值
    pub fn capped(self, max_limit: u32) -> Self {
        Self {
            offset: self.offset,
            limit: self.limit.min(max_limit),
        }
    }
}

/// 分页响应
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct PageResponse<T> {
    /// 当前页数据
    pub items: sp_std::vec::Vec<T>,
    /// 总记录数
    pub total: u32,
    /// 是否有更多数据
    pub has_more: bool,
}

impl<T> PageResponse<T> {
    /// 创建空分页响应
    pub fn empty() -> Self {
        Self { items: sp_std::vec::Vec::new(), total: 0, has_more: false }
    }

    /// 从完整列表构建分页响应
    pub fn from_slice(all_items: sp_std::vec::Vec<T>, page: &PageRequest) -> Self {
        let total = all_items.len() as u32;
        let start = (page.offset as usize).min(all_items.len());
        let end = start.saturating_add(page.limit as usize).min(all_items.len());
        let has_more = end < all_items.len();
        let items = all_items.into_iter().skip(start).take(end - start).collect();
        Self { items, total, has_more }
    }
}

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
    ///
    /// **已废弃**: Custom(u8) 无注册/验证机制，所有辅助方法均回退到最宽松默认值，
    /// 相当于绕过策略的后门。请使用 7 种内置类型代替。
    #[deprecated(note = "Custom(u8) 无验证机制，所有辅助方法回退到最宽松默认值。请使用内置类型")]
    Custom(u8),
}

impl EntityType {
    /// 默认治理模式（创建实体时的建议值）
    pub fn default_governance(&self) -> GovernanceMode {
        match self {
            Self::DAO => GovernanceMode::FullDAO,
            Self::Enterprise => GovernanceMode::MultiSig,
            Self::Fund | Self::Community => GovernanceMode::Council,
            Self::Project => GovernanceMode::FullDAO,
            _ => GovernanceMode::None,
        }
    }
    
    /// 默认代币类型（创建实体时的建议值）
    #[allow(deprecated)]
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
    
    /// 是否默认需要 KYC（与治理模式无关）
    ///
    /// Enterprise/Fund/Project 因合规要求默认启用 KYC，
    /// 不论使用 FullDAO、MultiSig 或 Council 治理模式。
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
            (Self::DAO, GovernanceMode::None) => false,
            (Self::Fund, GovernanceMode::FullDAO) => false,
            (Self::Enterprise, GovernanceMode::FullDAO) => false,
            _ => true,
        }
    }
    
    /// 默认转账限制模式
    #[allow(deprecated)]
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
    /// 多签治理（N-of-M 签名者共同决策，适合 Enterprise）
    MultiSig,
    /// 理事会治理（选举/任命理事会成员投票，适合 Fund/Community）
    Council,
}

// ============================================================================
// 实体相关类型
// ============================================================================

/// 实体状态（Entity 组织层状态）
///
/// **Default = Pending**：`create_entity` 内部显式设为 `Active`，
/// `reopen_entity` 使用 `Default` 即 `Pending` 进入审核流程。
/// 不要依赖 `EntityStatus::default()` 初始化新建实体。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
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
                if matches!(shop_status, ShopOperatingStatus::Banned) {
                    return Self::Banned;
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
    ///
    /// **v0.9.0 废弃预告**: 初期上线低频类型，建议使用 PhysicalStore + 链下标记替代。
    Warehouse,
    /// 加盟店
    ///
    /// **v0.9.0 废弃预告**: 初期上线低频类型，建议使用 OnlineStore + 链下标记替代。
    Franchise,
    /// 快闪店/临时店
    ///
    /// **v0.9.0 废弃预告**: 初期上线低频类型，建议使用 PhysicalStore + 链下标记替代。
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
/// - `KYC_REQUIRED` = 注册时需要通过 KYC 认证
/// - `KYC_UPGRADE_REQUIRED` = 等级升级时需要通过 KYC 认证
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
    /// 混合型（投票权 + 分红权）
    Hybrid,
}

impl TokenType {
    /// 是否具有投票权
    pub fn has_voting_power(&self) -> bool {
        matches!(self, Self::Governance | Self::Equity | Self::Hybrid)
    }
    
    /// 是否具有分红权
    pub fn has_dividend_rights(&self) -> bool {
        matches!(self, Self::Equity | Self::Share | Self::Hybrid)
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
            Self::Hybrid => (2, 2),        // Standard, Standard (默认)
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
            Self::Hybrid => TransferRestrictionMode::None,
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
/// 新代码应使用 `DividendState` 存储运行时状态。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct DividendConfig<Balance, BlockNumber> {
    /// 是否启用分红
    pub enabled: bool,
    /// 最小分红周期（区块数）
    pub min_period: BlockNumber,
    /// 上次分红时间
    ///
    /// **迁移提示**: 运行时状态不应嵌入配置结构体。
    /// 新代码请使用 `DividendState::last_distribution`，此字段保留仅为存储兼容。
    pub last_distribution: BlockNumber,
    /// 累计待分配金额
    ///
    /// **迁移提示**: 运行时状态不应嵌入配置结构体。
    /// 新代码请使用 `DividendState::accumulated`，此字段保留仅为存储兼容。
    pub accumulated: Balance,
}

/// 分红运行时状态（与 DividendConfig 配置分离）
///
/// 新模块应将配置（enabled/min_period）和状态（last_distribution/accumulated）
/// 存储在不同的 StorageMap 中，避免频繁写入完整配置。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub struct DividendState<Balance, BlockNumber> {
    /// 上次分红区块号
    pub last_distribution: BlockNumber,
    /// 累计待分配金额
    pub accumulated: Balance,
    /// 累计已分配总额
    pub total_distributed: Balance,
    /// 分红轮次计数
    pub round_count: u32,
}

// ============================================================================
// 服务/商品相关类型
// ============================================================================

/// 商品状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
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
///
/// **编码兼容性**: SCALE 按声明顺序分配序号（Created=0, Paid=1, ...）。
/// 新增变体必须追加到末尾，禁止插入中间，否则已存储的状态值将解码错误。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
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
    /// 已退款（全额）
    Refunded,
    /// 已过期（支付超时）
    Expired,
    // ---- v0.9.0 新增（追加到末尾，保持 SCALE 编码兼容） ----
    /// 处理中（数字商品/服务类：Paid 后自动流转，无需 Shipped）
    Processing,
    /// 待确认收货（买家确认窗口，超时自动 Completed）
    AwaitingConfirmation,
    /// 部分退款（从 Completed 状态发起部分退款后的终态）
    PartiallyRefunded,
}

// ============================================================================
// 会员状态枚举
// ============================================================================

/// 会员状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum MemberStatus {
    /// 正常活跃
    #[default]
    Active,
    /// 待审批（APPROVAL_REQUIRED 策略时）
    Pending,
    /// 暂时冻结（管理员操作）
    Frozen,
    /// 永久封禁
    Banned,
    /// 有效期已到期
    Expired,
}

impl MemberStatus {
    /// 是否可正常参与实体活动
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// 是否为受限状态（不可参与活动）
    pub fn is_restricted(&self) -> bool {
        matches!(self, Self::Frozen | Self::Banned | Self::Expired)
    }

    /// 是否可恢复为活跃
    pub fn can_reactivate(&self) -> bool {
        matches!(self, Self::Frozen | Self::Expired)
    }

    /// 是否为待审批状态
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

// ============================================================================
// 争议相关类型
// ============================================================================

/// 争议状态（跨模块共享）
///
/// 由 dispute 模块设置，供 order/commission/review 等模块查询
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum DisputeStatus {
    /// 无争议
    #[default]
    None,
    /// 已提交，等待响应
    Submitted,
    /// 被投诉方已响应
    Responded,
    /// 调解中
    Mediating,
    /// 仲裁中
    Arbitrating,
    /// 已解决
    Resolved,
    /// 已撤销
    Withdrawn,
    /// 已过期
    Expired,
}

impl DisputeStatus {
    /// 是否处于活跃争议状态（未解决）
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Submitted | Self::Responded | Self::Mediating | Self::Arbitrating)
    }

    /// 是否已终结
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Resolved | Self::Withdrawn | Self::Expired)
    }
}

/// 争议裁决结果
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum DisputeResolution {
    /// 投诉方胜诉（全额退款）
    ComplainantWin,
    /// 被投诉方胜诉（全额放款）
    RespondentWin,
    /// 和解（双方协商，无具体比例）
    Settlement,
    /// 按比例裁决（仲裁员指定投诉方获得的比例）
    ///
    /// `complainant_share_bps`: 投诉方获得比例（基点，0..=10000，10000 = 100%）
    PartialSettlement { complainant_share_bps: u16 },
}

impl DisputeResolution {
    /// 校验裁决参数是否合法
    ///
    /// `PartialSettlement` 的 `complainant_share_bps` 必须在 0..=10000 范围内。
    pub fn is_valid(&self) -> bool {
        match self {
            Self::PartialSettlement { complainant_share_bps } => *complainant_share_bps <= 10000,
            _ => true,
        }
    }

    /// 获取投诉方获赔比例（基点）
    ///
    /// - `ComplainantWin` → 10000 (100%)
    /// - `RespondentWin` → 0
    /// - `Settlement` → 5000 (50%，默认对半)
    /// - `PartialSettlement` → 指定值
    pub fn complainant_share_bps(&self) -> u16 {
        match self {
            Self::ComplainantWin => 10000,
            Self::RespondentWin => 0,
            Self::Settlement => 5000,
            Self::PartialSettlement { complainant_share_bps } => *complainant_share_bps,
        }
    }
}

// ============================================================================
// Token Sale 相关类型
// ============================================================================

/// Token Sale 状态（跨模块共享）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum TokenSaleStatus {
    /// 未开始
    #[default]
    NotStarted,
    /// 发售进行中
    Active,
    /// 已暂停
    Paused,
    /// 已结束
    Ended,
    /// 已取消
    Cancelled,
    /// 已完成（全部售出或手动完成）
    Completed,
}

impl TokenSaleStatus {
    /// 是否可以购买
    pub fn is_purchasable(&self) -> bool {
        matches!(self, Self::Active)
    }
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
    /// 商品管理（定价、库存、上下架，独立于 SHOP_MANAGE）
    pub const PRODUCT_MANAGE: u32    = 0b1000_0000_0000;
    /// 市场/交易管理（挂单管理、交易对配置）
    pub const MARKET_MANAGE: u32     = 0b0001_0000_0000_0000;
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
        | COMMISSION_MANAGE
        | PRODUCT_MANAGE
        | MARKET_MANAGE;

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

    // ==================== 所有权转移 ====================

    /// 发起所有权转移请求（owner 调用，new_owner 需 accept）
    fn initiate_ownership_transfer(entity_id: u64, new_owner: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, new_owner);
        Err(DispatchError::Other("not implemented"))
    }

    /// 接受所有权转移（new_owner 调用）
    fn accept_ownership_transfer(entity_id: u64, new_owner: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, new_owner);
        Err(DispatchError::Other("not implemented"))
    }

    /// 取消所有权转移请求（owner 调用）
    fn cancel_ownership_transfer(entity_id: u64) -> Result<(), DispatchError> {
        let _ = entity_id;
        Err(DispatchError::Other("not implemented"))
    }

    /// 获取待处理的所有权转移目标（None = 无待转移）
    fn pending_ownership_transfer(entity_id: u64) -> Option<AccountId> {
        let _ = entity_id;
        None
    }

    // ==================== P6: 元数据查询 ====================

    /// 获取实体名称（UTF-8 字节）
    fn entity_name(entity_id: u64) -> sp_std::vec::Vec<u8> {
        let _ = entity_id;
        sp_std::vec::Vec::new()
    }

    /// 获取实体元数据 IPFS CID
    fn entity_metadata_cid(entity_id: u64) -> Option<sp_std::vec::Vec<u8>> {
        let _ = entity_id;
        None
    }

    /// 获取实体描述（UTF-8 字节）
    fn entity_description(entity_id: u64) -> sp_std::vec::Vec<u8> {
        let _ = entity_id;
        sp_std::vec::Vec::new()
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
    
    /// 更新 Shop 评分（0-100 分制，与 `ReviewProvider::shop_average_rating` 同刻度）
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

    // ==================== #7 补充: 治理封禁接口 ====================

    /// 封禁 Shop（治理调用，不可被 owner 恢复，需通过治理解封）
    fn ban_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }

    /// 解除 Shop 封禁（治理调用）
    fn unban_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }

    // ==================== R10: 治理提案链上执行接口 ====================

    /// 关闭 Shop（治理提案执行，不可逆）
    fn governance_close_shop(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }

    /// 变更 Shop 类型（治理提案执行）
    fn governance_set_shop_type(shop_id: u64, new_type: ShopType) -> Result<(), DispatchError> {
        let _ = (shop_id, new_type);
        Ok(())
    }
}

/// 商品聚合查询信息（单次存储读取返回下单所需全部字段）
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug)]
pub struct ProductQueryInfo<Balance> {
    pub shop_id: u64,
    pub price: Balance,
    pub usdt_price: u64,
    pub stock: u32,
    pub status: ProductStatus,
    pub category: ProductCategory,
    pub visibility: ProductVisibility,
    pub min_order_quantity: u32,
    pub max_order_quantity: u32,
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

    /// 聚合查询：一次存储读取返回下单所需全部字段
    fn get_product_info(product_id: u64) -> Option<ProductQueryInfo<Balance>> {
        let _ = product_id;
        None
    }

    // ==================== 治理调用接口 ====================
    
    /// 更新商品价格（治理调用）
    fn update_price(product_id: u64, new_price: Balance) -> Result<(), DispatchError> {
        let _ = (product_id, new_price);
        Ok(())
    }
    
    /// 级联 unpin 某 Shop 下所有 Product 的 CID（Shop 关闭/封禁时调用）。
    fn force_unpin_shop_products(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }

    /// 强制移除某 Shop 下所有商品（Shop 关闭时调用）：
    /// 删除全部商品存储、退还押金、unpin CID、更新统计。
    fn force_remove_all_shop_products(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
        Ok(())
    }

    /// 强制下架某 Shop 下所有在售/售罄商品（Shop 封禁时调用）：
    /// OnSale/SoldOut → OffShelf，更新统计。
    fn force_delist_all_shop_products(shop_id: u64) -> Result<(), DispatchError> {
        let _ = shop_id;
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

    /// 设置商品可见性（治理提案执行）
    fn governance_set_visibility(product_id: u64, visibility: ProductVisibility) -> Result<(), DispatchError> {
        let _ = (product_id, visibility);
        Ok(())
    }
}

/// 订单查询接口
/// 
/// 供 review 模块查询订单信息
/// 订单聚合查询信息（一次 storage read 获取常用字段）
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct OrderQueryInfo<AccountId, Balance> {
    pub order_id: u64,
    pub entity_id: u64,
    pub shop_id: u64,
    pub product_id: u64,
    pub buyer: AccountId,
    pub seller: AccountId,
    pub quantity: u32,
    pub total_amount: Balance,
    pub token_payment_amount: u128,
    pub payment_asset: PaymentAsset,
    pub status: OrderStatus,
    pub product_category: ProductCategory,
    pub created_at: u64,
    pub shipped_at: Option<u64>,
    pub completed_at: Option<u64>,
}

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

    // ==================== #1 补充: 关键查询方法 ====================

    /// 获取订单状态
    fn order_status(order_id: u64) -> Option<OrderStatus> {
        let _ = order_id;
        None
    }

    /// 获取订单所属 Entity ID（通过 shop_id 间接获取或直接存储）
    fn order_entity_id(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }

    /// 获取订单商品 ID
    fn order_product_id(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }

    /// 获取订单购买数量
    fn order_quantity(order_id: u64) -> Option<u32> {
        let _ = order_id;
        None
    }

    // ==================== P3: 时间戳查询补全 ====================

    /// 获取订单创建时间（区块号）
    fn order_created_at(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }

    /// 获取订单支付时间（区块号）
    fn order_paid_at(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }

    /// 获取订单发货时间（区块号）
    fn order_shipped_at(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }

    /// 聚合查询：一次 storage read 获取订单常用字段（替代多次独立查询）
    fn get_order_info(order_id: u64) -> Option<OrderQueryInfo<AccountId, Balance>> {
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

// ============================================================================
// P5: Entity 状态变更级联通知
// ============================================================================

/// Entity 状态变更通知接口
///
/// 供下游模块（Shop、Token、Market、Ads 等）在 Entity 状态变更时做出响应。
/// 当 Entity 被暂停/封禁/关闭/恢复时，级联通知所有关联模块执行清理逻辑。
pub trait OnEntityStatusChange {
    /// Entity 被暂停时触发
    fn on_entity_suspended(entity_id: u64);
    /// Entity 被封禁时触发
    fn on_entity_banned(entity_id: u64);
    /// Entity 恢复运营时触发
    fn on_entity_resumed(entity_id: u64);
    /// Entity 被关闭时触发
    fn on_entity_closed(entity_id: u64);
}

/// 空 Entity 状态变更通知（测试用或无下游模块时）
impl OnEntityStatusChange for () {
    fn on_entity_suspended(_entity_id: u64) {}
    fn on_entity_banned(_entity_id: u64) {}
    fn on_entity_resumed(_entity_id: u64) {}
    fn on_entity_closed(_entity_id: u64) {}
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
// P2: 订单状态变更通知
// ============================================================================

/// 订单状态变更通知接口
///
/// 供下游模块（佣金、会员、物流、保险等）在订单状态变更时做出响应。
/// 实现开闭原则：新增下游模块无需修改 order pallet。
pub trait OnOrderStatusChange<AccountId, Balance> {
    /// 订单状态变更时触发
    fn on_order_status_changed(
        order_id: u64,
        entity_id: u64,
        shop_id: u64,
        buyer: &AccountId,
        amount: Balance,
        old_status: &OrderStatus,
        new_status: &OrderStatus,
    );
}

impl<AccountId, Balance> OnOrderStatusChange<AccountId, Balance> for () {
    fn on_order_status_changed(
        _order_id: u64,
        _entity_id: u64,
        _shop_id: u64,
        _buyer: &AccountId,
        _amount: Balance,
        _old_status: &OrderStatus,
        _new_status: &OrderStatus,
    ) {}
}

// ============================================================================
// 实体代币接口
// ============================================================================

/// 实体代币接口
/// 
/// 供 order 模块调用，实现购物返积分和积分抵扣
pub trait EntityTokenProvider<AccountId, Balance: Default> {
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

    // ==================== #11 补充: 元数据查询 ====================

    /// 获取代币名称（UTF-8 字节）
    fn token_name(entity_id: u64) -> sp_std::vec::Vec<u8> {
        let _ = entity_id;
        sp_std::vec::Vec::new()
    }

    /// 获取代币符号（UTF-8 字节）
    fn token_symbol(entity_id: u64) -> sp_std::vec::Vec<u8> {
        let _ = entity_id;
        sp_std::vec::Vec::new()
    }

    /// 获取代币精度
    fn token_decimals(entity_id: u64) -> u8 {
        let _ = entity_id;
        0
    }

    /// 代币是否可自由转让（检查 TransferRestrictionMode）
    fn is_token_transferable(entity_id: u64) -> bool {
        let _ = entity_id;
        false
    }

    /// 获取代币持有人数量
    fn token_holder_count(entity_id: u64) -> u32 {
        let _ = entity_id;
        0
    }

    /// 获取可用余额（总余额 - 锁仓 - 预留）
    fn available_balance(entity_id: u64, holder: &AccountId) -> Balance {
        let _ = (entity_id, holder);
        Default::default()
    }

    // ==================== R10: 治理提案链上执行接口 ====================

    /// 设置代币最大供应量（治理提案执行）
    fn governance_set_max_supply(entity_id: u64, new_max_supply: Balance) -> Result<(), DispatchError> {
        let _ = (entity_id, new_max_supply);
        Ok(())
    }

    /// 设置代币类型（治理提案执行）
    fn governance_set_token_type(entity_id: u64, new_type: TokenType) -> Result<(), DispatchError> {
        let _ = (entity_id, new_type);
        Ok(())
    }

    /// 设置转账限制模式（治理提案执行）
    fn governance_set_transfer_restriction(entity_id: u64, restriction: u8, min_receiver_kyc: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, restriction, min_receiver_kyc);
        Ok(())
    }
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

/// 价格可靠性等级（简化的置信度判断）
///
/// 替代 `token_price_confidence() -> u8` 的数值型判断，
/// 下游消费方只需匹配枚举即可决策，无需记忆置信度区间。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum PriceReliability {
    /// 价格可靠（TWAP 可用 + 足够交易量）
    Reliable,
    /// 价格低可信（仅 initial_price 或低交易量）
    Low,
    /// 价格不可用或过时
    Unavailable,
}

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

    /// 获取简化的价格可靠性等级
    ///
    /// 基于 confidence 数值自动映射：>=60 Reliable, >=30 Low, <30 Unavailable。
    /// 下游代码应优先使用此方法，避免硬编码置信度数值。
    fn price_reliability(entity_id: u64) -> PriceReliability {
        let c = Self::token_price_confidence(entity_id);
        if c >= 60 {
            PriceReliability::Reliable
        } else if c >= 30 {
            PriceReliability::Low
        } else {
            PriceReliability::Unavailable
        }
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
// P7: 手续费配置查询接口
// ============================================================================

/// 手续费配置查询接口
///
/// 统一跨模块手续费查询，避免费率逻辑碎片化。
/// 费率单位: 基点 (bps)，100 = 1%。
pub trait FeeConfigProvider {
    /// 获取全局 NEX 平台费率（bps）
    fn platform_fee_rate() -> u16;

    /// 获取 Entity 级平台费率覆盖（None = 使用全局默认）
    fn entity_fee_override(entity_id: u64) -> Option<u16> {
        let _ = entity_id;
        None
    }

    /// 获取 Entity Token 交易费率（bps）
    fn token_fee_rate(entity_id: u64) -> u16 {
        let _ = entity_id;
        0
    }

    /// 获取 Entity 有效费率（优先 entity_fee_override，回退 platform_fee_rate）
    fn effective_fee_rate(entity_id: u64) -> u16 {
        Self::entity_fee_override(entity_id).unwrap_or_else(Self::platform_fee_rate)
    }
}

/// 空手续费配置提供者（测试用）
pub struct NullFeeConfigProvider;

impl FeeConfigProvider for NullFeeConfigProvider {
    fn platform_fee_rate() -> u16 { 100 }
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

    /// F7: 获取实体违规次数
    fn get_violation_count(_entity_id: u64) -> u32 { 0 }

    /// F7: 获取内幕人员角色（返回 InsiderRole 的 u8 表示）
    ///
    /// 0=Owner, 1=Admin, 2=Auditor, 3=Advisor, 4=MajorHolder
    fn get_insider_role(_entity_id: u64, _account: &AccountId) -> Option<u8> { None }

    /// F7: 检查实体是否已配置披露
    fn is_disclosure_configured(_entity_id: u64) -> bool { false }

    /// F6/F7: 检查实体是否被标记为高风险（违规超阈值）
    fn is_high_risk(_entity_id: u64) -> bool { false }

    // ==================== F10: 治理写入接口 ====================

    /// F10: 治理提案配置披露级别
    fn governance_configure_disclosure(
        _entity_id: u64,
        _level: DisclosureLevel,
        _insider_trading_control: bool,
        _blackout_period_after: u64,
    ) -> sp_runtime::DispatchResult {
        Err(sp_runtime::DispatchError::Other("not implemented"))
    }

    /// F10: 治理提案重置违规记录
    fn governance_reset_violations(_entity_id: u64) -> sp_runtime::DispatchResult {
        Err(sp_runtime::DispatchError::Other("not implemented"))
    }

    // ==================== v0.6: 大股东自动注册 ====================

    /// 将账户注册为大股东内幕人员（供 token 模块在持仓超过阈值时调用）
    fn register_major_holder(_entity_id: u64, _account: &AccountId) -> sp_runtime::DispatchResult {
        Ok(())
    }

    /// 注销大股东内幕人员身份（供 token 模块在持仓低于阈值时调用）
    fn deregister_major_holder(_entity_id: u64, _account: &AccountId) -> sp_runtime::DispatchResult {
        Ok(())
    }

    // ==================== v0.6: 渐进式处罚 ====================

    /// 获取实体当前处罚级别 (0=None, 1=Warning, 2=Restricted, 3=Suspended, 4=Delisted)
    fn get_penalty_level(_entity_id: u64) -> u8 { 0 }

    /// 检查实体是否受到活跃处罚（Restricted 及以上）
    fn is_penalty_active(_entity_id: u64) -> bool { false }

    // ==================== R10: 治理提案链上执行接口 ====================

    /// 设置处罚级别（治理提案执行）
    fn governance_set_penalty_level(_entity_id: u64, _level: u8) -> sp_runtime::DispatchResult {
        Ok(())
    }
}

/// 披露违规回调 — 供下游模块（token/market）响应披露违规事件
pub trait OnDisclosureViolation {
    /// 违规达到阈值或处罚升级时调用
    fn on_violation_threshold_reached(entity_id: u64, violation_count: u32, penalty_level: u8);
}

impl OnDisclosureViolation for () {
    fn on_violation_threshold_reached(_: u64, _: u32, _: u8) {}
}

/// 空披露提供者（测试用或未启用披露时）
pub struct NullDisclosureProvider;

impl<AccountId> DisclosureProvider<AccountId> for NullDisclosureProvider {
    fn is_in_blackout(_entity_id: u64) -> bool { false }
    fn is_insider(_entity_id: u64, _account: &AccountId) -> bool { false }
    fn can_insider_trade(_entity_id: u64, _account: &AccountId) -> bool { true }
    fn get_disclosure_level(_entity_id: u64) -> DisclosureLevel { DisclosureLevel::Basic }
    fn is_disclosure_overdue(_entity_id: u64) -> bool { false }
    fn get_violation_count(_entity_id: u64) -> u32 { 0 }
    fn get_insider_role(_entity_id: u64, _account: &AccountId) -> Option<u8> { None }
    fn is_disclosure_configured(_entity_id: u64) -> bool { false }
    fn is_high_risk(_entity_id: u64) -> bool { false }
    fn get_penalty_level(_entity_id: u64) -> u8 { 0 }
    fn is_penalty_active(_entity_id: u64) -> bool { false }
}

// ============================================================================
// 披露接口职责拆分（DisclosureProvider 的精简替代）
// ============================================================================

/// 披露只读查询接口（DisclosureProvider 读取子集）
///
/// 新模块应优先使用此 trait，仅关注只读查询，无需 mock 写入方法。
pub trait DisclosureReadProvider<AccountId> {
    fn is_in_blackout(entity_id: u64) -> bool;
    fn is_insider(entity_id: u64, account: &AccountId) -> bool;
    fn can_insider_trade(entity_id: u64, account: &AccountId) -> bool;
    fn get_disclosure_level(entity_id: u64) -> DisclosureLevel;
    fn is_disclosure_overdue(entity_id: u64) -> bool;
    fn get_violation_count(entity_id: u64) -> u32 { let _ = entity_id; 0 }
    fn get_insider_role(entity_id: u64, account: &AccountId) -> Option<u8> { let _ = (entity_id, account); None }
    fn is_disclosure_configured(entity_id: u64) -> bool { let _ = entity_id; false }
    fn is_high_risk(entity_id: u64) -> bool { let _ = entity_id; false }
    fn get_penalty_level(entity_id: u64) -> u8 { let _ = entity_id; 0 }
    fn is_penalty_active(entity_id: u64) -> bool { let _ = entity_id; false }
}

/// 披露治理写入接口（DisclosureProvider 写入子集）
///
/// 仅供 governance 模块使用，其他模块无需依赖写入方法。
pub trait DisclosureWriteProvider<AccountId> {
    fn governance_configure_disclosure(entity_id: u64, level: DisclosureLevel, insider_trading_control: bool, blackout_period_after: u64) -> sp_runtime::DispatchResult;
    fn governance_reset_violations(entity_id: u64) -> sp_runtime::DispatchResult;
    fn register_major_holder(entity_id: u64, account: &AccountId) -> sp_runtime::DispatchResult;
    fn deregister_major_holder(entity_id: u64, account: &AccountId) -> sp_runtime::DispatchResult;
    fn governance_set_penalty_level(entity_id: u64, level: u8) -> sp_runtime::DispatchResult;
}

/// 空只读披露提供者 — `NullDisclosureProvider` 的类型别名
///
/// 通过 blanket impl，`NullDisclosureProvider` 自动实现 `DisclosureReadProvider`
/// 和 `DisclosureWriteProvider`，无需单独定义独立类型。
pub type NullDisclosureReadProvider = NullDisclosureProvider;

/// 空写入披露提供者 — `NullDisclosureProvider` 的类型别名（无操作模式）
///
/// 写入方法来自 `DisclosureProvider` 的默认实现：
/// - `register_major_holder` / `deregister_major_holder` → `Ok(())`
/// - `governance_configure_disclosure` / `governance_reset_violations` → `Err("not implemented")`
///   （表示未完成 override，而非功能关闭）
pub type NullDisclosureWriteProvider = NullDisclosureProvider;

// ---- 桥接: DisclosureProvider 自动实现 DisclosureReadProvider / WriteProvider ----

impl<AccountId, T: DisclosureProvider<AccountId>> DisclosureReadProvider<AccountId> for T {
    fn is_in_blackout(entity_id: u64) -> bool {
        <T as DisclosureProvider<AccountId>>::is_in_blackout(entity_id)
    }
    fn is_insider(entity_id: u64, account: &AccountId) -> bool {
        <T as DisclosureProvider<AccountId>>::is_insider(entity_id, account)
    }
    fn can_insider_trade(entity_id: u64, account: &AccountId) -> bool {
        <T as DisclosureProvider<AccountId>>::can_insider_trade(entity_id, account)
    }
    fn get_disclosure_level(entity_id: u64) -> DisclosureLevel {
        <T as DisclosureProvider<AccountId>>::get_disclosure_level(entity_id)
    }
    fn is_disclosure_overdue(entity_id: u64) -> bool {
        <T as DisclosureProvider<AccountId>>::is_disclosure_overdue(entity_id)
    }
    fn get_violation_count(entity_id: u64) -> u32 {
        <T as DisclosureProvider<AccountId>>::get_violation_count(entity_id)
    }
    fn get_insider_role(entity_id: u64, account: &AccountId) -> Option<u8> {
        <T as DisclosureProvider<AccountId>>::get_insider_role(entity_id, account)
    }
    fn is_disclosure_configured(entity_id: u64) -> bool {
        <T as DisclosureProvider<AccountId>>::is_disclosure_configured(entity_id)
    }
    fn is_high_risk(entity_id: u64) -> bool {
        <T as DisclosureProvider<AccountId>>::is_high_risk(entity_id)
    }
    fn get_penalty_level(entity_id: u64) -> u8 {
        <T as DisclosureProvider<AccountId>>::get_penalty_level(entity_id)
    }
    fn is_penalty_active(entity_id: u64) -> bool {
        <T as DisclosureProvider<AccountId>>::is_penalty_active(entity_id)
    }
}

impl<AccountId, T: DisclosureProvider<AccountId>> DisclosureWriteProvider<AccountId> for T {
    fn governance_configure_disclosure(entity_id: u64, level: DisclosureLevel, insider_trading_control: bool, blackout_period_after: u64) -> sp_runtime::DispatchResult {
        <T as DisclosureProvider<AccountId>>::governance_configure_disclosure(entity_id, level, insider_trading_control, blackout_period_after)
    }
    fn governance_reset_violations(entity_id: u64) -> sp_runtime::DispatchResult {
        <T as DisclosureProvider<AccountId>>::governance_reset_violations(entity_id)
    }
    fn register_major_holder(entity_id: u64, account: &AccountId) -> sp_runtime::DispatchResult {
        <T as DisclosureProvider<AccountId>>::register_major_holder(entity_id, account)
    }
    fn deregister_major_holder(entity_id: u64, account: &AccountId) -> sp_runtime::DispatchResult {
        <T as DisclosureProvider<AccountId>>::deregister_major_holder(entity_id, account)
    }
    fn governance_set_penalty_level(entity_id: u64, level: u8) -> sp_runtime::DispatchResult {
        <T as DisclosureProvider<AccountId>>::governance_set_penalty_level(entity_id, level)
    }
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

    // ==================== #9 补充: 过期与参与检查 ====================

    /// KYC 认证是否已过期
    fn is_kyc_expired(entity_id: u64, account: &AccountId) -> bool {
        let _ = (entity_id, account);
        false
    }

    /// 用户是否可以参与实体活动（综合 KYC 状态 + 宽限期 + 封禁状态）
    fn can_participate(entity_id: u64, account: &AccountId) -> bool {
        Self::is_kyc_approved(entity_id, account)
    }

    /// 获取 KYC 过期时间（区块号，0 = 永不过期或无记录）
    fn kyc_expires_at(entity_id: u64, account: &AccountId) -> u64 {
        let _ = (entity_id, account);
        0
    }
}

/// 空 KYC 提供者（测试用或未启用 KYC 时）
pub struct NullKycProvider;

impl<AccountId> KycProvider<AccountId> for NullKycProvider {
    fn kyc_level(_entity_id: u64, _account: &AccountId) -> u8 { 0 }
}

/// KYC 状态变更通知接口
///
/// 供下游模块（订单、交易等）在 KYC 状态变更时做出响应。
/// old_status / new_status 使用 u8 编码避免跨 pallet 类型依赖:
/// 0=NotSubmitted, 1=Pending, 2=Approved, 3=Rejected, 4=Expired, 5=Revoked
pub trait OnKycStatusChange<AccountId> {
    fn on_kyc_status_changed(entity_id: u64, account: &AccountId, old_status: u8, new_status: u8, level: u8);
}

impl<AccountId> OnKycStatusChange<AccountId> for () {
    fn on_kyc_status_changed(_entity_id: u64, _account: &AccountId, _old_status: u8, _new_status: u8, _level: u8) {}
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

    // ==================== #10 补充: 治理查询扩展 ====================

    /// 获取活跃提案数量
    fn active_proposal_count(entity_id: u64) -> u32 {
        let _ = entity_id;
        0
    }

    /// 检查实体治理是否已初始化
    fn is_governance_initialized(entity_id: u64) -> bool {
        let _ = entity_id;
        false
    }

    /// 获取实体治理配置中的执行延迟（区块数）
    fn execution_delay(entity_id: u64) -> u32 {
        let _ = entity_id;
        0
    }

    /// 获取通过阈值（百分比 0-100）
    fn pass_threshold(entity_id: u64) -> u8 {
        let _ = entity_id;
        0
    }

    /// 实体治理是否被暂停
    fn is_governance_paused(entity_id: u64) -> bool {
        let _ = entity_id;
        false
    }
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
    ///
    /// **已废弃**: 与 `EntityProvider::entity_account()` 功能重复。
    /// 新代码应通过 `EntityProvider` 获取 Entity 派生账户。
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
/// **已废弃**: 此 trait 是 `MemberProvider` 的子集，方法签名完全相同。
/// 新代码应直接使用 `MemberProvider::auto_register` / `update_spent` / `check_order_upgrade_rules`。
///
/// 供 Transaction 模块在订单完成时：
/// 1. 自动注册买家为会员（如果尚未注册）
/// 2. 更新消费金额（触发等级升级）
#[deprecated(note = "OrderMemberHandler 是 MemberProvider 的子集，请迁移到 MemberProvider")]
pub trait OrderMemberHandler<AccountId> {
    /// 自动注册会员（首次下单时，推荐人可选）
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> Result<(), DispatchError>;
    /// 更新消费金额（USDT 精度 10^6）
    fn update_spent(entity_id: u64, account: &AccountId, amount_usdt: u64) -> Result<(), DispatchError>;
    /// 检查订单完成时的升级规则（amount_usdt: USDT 精度 10^6）
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, amount_usdt: u64) -> Result<(), DispatchError>;
}

/// 空会员处理（无会员系统时使用）
#[allow(deprecated)]
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
    pub threshold: u128,
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

    /// 查询会员是否已激活（如首次消费达标）
    ///
    /// "激活"与"未封禁"是两个独立概念：
    /// - 新注册但未消费的会员 → is_banned=false 但 is_activated=false
    /// - 未激活会员不应获得佣金
    fn is_activated(entity_id: u64, account: &AccountId) -> bool {
        let _ = (entity_id, account);
        true // 默认实现: 向后兼容，所有会员视为已激活
    }

    /// F6: 查询会员是否处于活跃状态（非冻结/非封禁/非过期）
    fn is_member_active(entity_id: u64, account: &AccountId) -> bool {
        // 默认实现: 非 banned 即为 active（向后兼容）
        !Self::is_banned(entity_id, account)
    }

    /// F5: 获取推荐关系建立时间（区块号，0 = 未知/不支持）
    fn referral_registered_at(entity_id: u64, account: &AccountId) -> u64 {
        let _ = (entity_id, account);
        0
    }

    /// F7: 获取已完成的成功订单数（排除取消/退款的订单）
    fn completed_order_count(entity_id: u64, account: &AccountId) -> u32 {
        let _ = (entity_id, account);
        0
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

    // ==================== #5 补充: 溢出推荐人查询 ====================

    /// 获取真实推荐人（溢出安置时记录的原始推荐人）
    ///
    /// 溢出场景下 `get_referrer` 返回的是实际安置节点，
    /// `get_introduced_by` 返回原始推荐人。无溢出时返回 None。
    fn get_introduced_by(entity_id: u64, account: &AccountId) -> Option<AccountId> {
        let _ = (entity_id, account);
        None
    }

    /// 获取直推会员账户列表
    fn get_direct_referral_accounts(entity_id: u64, account: &AccountId) -> alloc::vec::Vec<AccountId> {
        let _ = (entity_id, account);
        alloc::vec::Vec::new()
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

    // ==================== #6 补充: 治理封禁/移除接口 ====================

    /// 封禁会员（治理调用，禁止参与实体活动）
    fn ban_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, account);
        Ok(())
    }

    /// 解除会员封禁（治理调用）
    fn unban_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, account);
        Ok(())
    }

    /// 移除会员（治理调用，从实体会员列表中删除）
    fn remove_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, account);
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

// ============================================================================
// 会员接口职责拆分（MemberProvider 的精简替代）
// ============================================================================

/// 会员只读查询接口（MemberProvider 读取子集）
///
/// 新模块仅需读取会员信息时，应优先使用此 trait。
pub trait MemberQueryProvider<AccountId> {
    fn is_member(entity_id: u64, account: &AccountId) -> bool;
    fn get_referrer(entity_id: u64, account: &AccountId) -> Option<AccountId>;
    fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;
    fn get_effective_level(entity_id: u64, account: &AccountId) -> u8 { Self::custom_level_id(entity_id, account) }
    fn get_level_discount(entity_id: u64, level_id: u8) -> u16 { let _ = (entity_id, level_id); 0 }
    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16;
    fn uses_custom_levels(entity_id: u64) -> bool;
    fn get_member_stats(entity_id: u64, account: &AccountId) -> (u32, u32, u128);
    fn member_count(entity_id: u64) -> u32 { let _ = entity_id; 0 }
    fn is_banned(entity_id: u64, account: &AccountId) -> bool { let _ = (entity_id, account); false }
    fn is_activated(entity_id: u64, account: &AccountId) -> bool { let _ = (entity_id, account); true }
    fn is_member_active(entity_id: u64, account: &AccountId) -> bool { !Self::is_banned(entity_id, account) }
    fn referral_registered_at(entity_id: u64, account: &AccountId) -> u64 { let _ = (entity_id, account); 0 }
    fn completed_order_count(entity_id: u64, account: &AccountId) -> u32 { let _ = (entity_id, account); 0 }
    fn last_active_at(entity_id: u64, account: &AccountId) -> u64 { let _ = (entity_id, account); 0 }
    fn member_level(entity_id: u64, account: &AccountId) -> Option<MemberLevelInfo> { let _ = (entity_id, account); None }
    fn custom_level_count(entity_id: u64) -> u8 { let _ = entity_id; 0 }
    fn member_count_by_level(entity_id: u64, level_id: u8) -> u32 { let _ = (entity_id, level_id); 0 }
    fn get_member_spent_usdt(entity_id: u64, account: &AccountId) -> u64 { let _ = (entity_id, account); 0 }
    fn get_introduced_by(entity_id: u64, account: &AccountId) -> Option<AccountId> { let _ = (entity_id, account); None }
    fn get_direct_referral_accounts(entity_id: u64, account: &AccountId) -> alloc::vec::Vec<AccountId> { let _ = (entity_id, account); alloc::vec::Vec::new() }
}

/// 会员写入接口（MemberProvider 写入子集）
///
/// 仅供 order/governance 模块使用。
pub trait MemberWriteProvider<AccountId> {
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> Result<(), DispatchError>;
    fn auto_register_qualified(entity_id: u64, account: &AccountId, referrer: Option<AccountId>, qualified: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, account, referrer, qualified); Ok(())
    }
    fn update_spent(entity_id: u64, account: &AccountId, amount_usdt: u64) -> Result<(), DispatchError> {
        let _ = (entity_id, account, amount_usdt); Ok(())
    }
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, amount_usdt: u64) -> Result<(), DispatchError> {
        let _ = (entity_id, buyer, product_id, amount_usdt); Ok(())
    }
    fn set_custom_levels_enabled(entity_id: u64, enabled: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, enabled); Ok(())
    }
    fn set_upgrade_mode(entity_id: u64, mode: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, mode); Ok(())
    }
    fn add_custom_level(entity_id: u64, level_id: u8, name: &[u8], threshold: u128, discount_rate: u16, commission_bonus: u16) -> Result<(), DispatchError> {
        let _ = (entity_id, level_id, name, threshold, discount_rate, commission_bonus); Ok(())
    }
    fn update_custom_level(entity_id: u64, level_id: u8, name: Option<&[u8]>, threshold: Option<u128>, discount_rate: Option<u16>, commission_bonus: Option<u16>) -> Result<(), DispatchError> {
        let _ = (entity_id, level_id, name, threshold, discount_rate, commission_bonus); Ok(())
    }
    fn remove_custom_level(entity_id: u64, level_id: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, level_id); Ok(())
    }
    fn set_registration_policy(entity_id: u64, policy_bits: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, policy_bits); Ok(())
    }
    fn set_stats_policy(entity_id: u64, policy_bits: u8) -> Result<(), DispatchError> {
        let _ = (entity_id, policy_bits); Ok(())
    }
    fn ban_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, account); Ok(())
    }
    fn unban_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, account); Ok(())
    }
    fn remove_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        let _ = (entity_id, account); Ok(())
    }
}

/// 空会员查询提供者 — `NullMemberProvider` 的类型别名
///
/// 通过 blanket impl，`NullMemberProvider` 自动实现 `MemberQueryProvider`。
pub type NullMemberQueryProvider = NullMemberProvider;

/// 空会员写入提供者 — `NullMemberProvider` 的类型别名
///
/// 通过 blanket impl，`NullMemberProvider` 自动实现 `MemberWriteProvider`。
pub type NullMemberWriteProvider = NullMemberProvider;

// ---- 桥接: MemberProvider 自动实现 MemberQueryProvider / MemberWriteProvider ----

impl<AccountId, T: MemberProvider<AccountId>> MemberQueryProvider<AccountId> for T {
    fn is_member(entity_id: u64, account: &AccountId) -> bool {
        <T as MemberProvider<AccountId>>::is_member(entity_id, account)
    }
    fn get_referrer(entity_id: u64, account: &AccountId) -> Option<AccountId> {
        <T as MemberProvider<AccountId>>::get_referrer(entity_id, account)
    }
    fn custom_level_id(entity_id: u64, account: &AccountId) -> u8 {
        <T as MemberProvider<AccountId>>::custom_level_id(entity_id, account)
    }
    fn get_effective_level(entity_id: u64, account: &AccountId) -> u8 {
        <T as MemberProvider<AccountId>>::get_effective_level(entity_id, account)
    }
    fn get_level_discount(entity_id: u64, level_id: u8) -> u16 {
        <T as MemberProvider<AccountId>>::get_level_discount(entity_id, level_id)
    }
    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16 {
        <T as MemberProvider<AccountId>>::get_level_commission_bonus(entity_id, level_id)
    }
    fn uses_custom_levels(entity_id: u64) -> bool {
        <T as MemberProvider<AccountId>>::uses_custom_levels(entity_id)
    }
    fn get_member_stats(entity_id: u64, account: &AccountId) -> (u32, u32, u128) {
        <T as MemberProvider<AccountId>>::get_member_stats(entity_id, account)
    }
    fn member_count(entity_id: u64) -> u32 {
        <T as MemberProvider<AccountId>>::member_count(entity_id)
    }
    fn is_banned(entity_id: u64, account: &AccountId) -> bool {
        <T as MemberProvider<AccountId>>::is_banned(entity_id, account)
    }
    fn is_activated(entity_id: u64, account: &AccountId) -> bool {
        <T as MemberProvider<AccountId>>::is_activated(entity_id, account)
    }
    fn is_member_active(entity_id: u64, account: &AccountId) -> bool {
        <T as MemberProvider<AccountId>>::is_member_active(entity_id, account)
    }
    fn referral_registered_at(entity_id: u64, account: &AccountId) -> u64 {
        <T as MemberProvider<AccountId>>::referral_registered_at(entity_id, account)
    }
    fn completed_order_count(entity_id: u64, account: &AccountId) -> u32 {
        <T as MemberProvider<AccountId>>::completed_order_count(entity_id, account)
    }
    fn last_active_at(entity_id: u64, account: &AccountId) -> u64 {
        <T as MemberProvider<AccountId>>::last_active_at(entity_id, account)
    }
    fn member_level(entity_id: u64, account: &AccountId) -> Option<MemberLevelInfo> {
        <T as MemberProvider<AccountId>>::member_level(entity_id, account)
    }
    fn custom_level_count(entity_id: u64) -> u8 {
        <T as MemberProvider<AccountId>>::custom_level_count(entity_id)
    }
    fn member_count_by_level(entity_id: u64, level_id: u8) -> u32 {
        <T as MemberProvider<AccountId>>::member_count_by_level(entity_id, level_id)
    }
    fn get_member_spent_usdt(entity_id: u64, account: &AccountId) -> u64 {
        <T as MemberProvider<AccountId>>::get_member_spent_usdt(entity_id, account)
    }
    fn get_introduced_by(entity_id: u64, account: &AccountId) -> Option<AccountId> {
        <T as MemberProvider<AccountId>>::get_introduced_by(entity_id, account)
    }
    fn get_direct_referral_accounts(entity_id: u64, account: &AccountId) -> alloc::vec::Vec<AccountId> {
        <T as MemberProvider<AccountId>>::get_direct_referral_accounts(entity_id, account)
    }
}

impl<AccountId, T: MemberProvider<AccountId>> MemberWriteProvider<AccountId> for T {
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::auto_register(entity_id, account, referrer)
    }
    fn auto_register_qualified(entity_id: u64, account: &AccountId, referrer: Option<AccountId>, qualified: bool) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::auto_register_qualified(entity_id, account, referrer, qualified)
    }
    fn update_spent(entity_id: u64, account: &AccountId, amount_usdt: u64) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::update_spent(entity_id, account, amount_usdt)
    }
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, amount_usdt: u64) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::check_order_upgrade_rules(entity_id, buyer, product_id, amount_usdt)
    }
    fn set_custom_levels_enabled(entity_id: u64, enabled: bool) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::set_custom_levels_enabled(entity_id, enabled)
    }
    fn set_upgrade_mode(entity_id: u64, mode: u8) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::set_upgrade_mode(entity_id, mode)
    }
    fn add_custom_level(entity_id: u64, level_id: u8, name: &[u8], threshold: u128, discount_rate: u16, commission_bonus: u16) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::add_custom_level(entity_id, level_id, name, threshold, discount_rate, commission_bonus)
    }
    fn update_custom_level(entity_id: u64, level_id: u8, name: Option<&[u8]>, threshold: Option<u128>, discount_rate: Option<u16>, commission_bonus: Option<u16>) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::update_custom_level(entity_id, level_id, name, threshold, discount_rate, commission_bonus)
    }
    fn remove_custom_level(entity_id: u64, level_id: u8) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::remove_custom_level(entity_id, level_id)
    }
    fn set_registration_policy(entity_id: u64, policy_bits: u8) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::set_registration_policy(entity_id, policy_bits)
    }
    fn set_stats_policy(entity_id: u64, policy_bits: u8) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::set_stats_policy(entity_id, policy_bits)
    }
    fn ban_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::ban_member(entity_id, account)
    }
    fn unban_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::unban_member(entity_id, account)
    }
    fn remove_member(entity_id: u64, account: &AccountId) -> Result<(), DispatchError> {
        <T as MemberProvider<AccountId>>::remove_member(entity_id, account)
    }
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
    fn available_balance(_: u64, _: &AccountId) -> Balance {
        Default::default()
    }
}

// ============================================================================
// #2 争议查询接口（跨模块）
// ============================================================================

/// 争议查询接口
///
/// 供 order/commission/review 等模块查询争议状态，
/// 无需直接依赖 pallet-arbitration。
pub trait DisputeQueryProvider<AccountId> {
    /// 获取订单的争议状态
    fn order_dispute_status(order_id: u64) -> DisputeStatus;

    /// 获取争议的裁决结果（仅已解决的争议）
    fn dispute_resolution(dispute_id: u64) -> Option<DisputeResolution>;

    /// 查询账户在指定域下的活跃争议数量
    fn active_dispute_count(domain: u8, account: &AccountId) -> u32;

    /// 检查订单是否有活跃争议
    fn has_active_dispute(order_id: u64) -> bool {
        Self::order_dispute_status(order_id).is_active()
    }

    /// 获取争议 ID（通过订单 ID 查找）
    fn dispute_id_by_order(order_id: u64) -> Option<u64> {
        let _ = order_id;
        None
    }

    /// 获取争议涉及金额
    fn dispute_amount(dispute_id: u64) -> Option<u128> {
        let _ = dispute_id;
        None
    }
}

/// 空争议查询提供者（测试用或未启用争议系统时）
pub struct NullDisputeQueryProvider;

impl<AccountId> DisputeQueryProvider<AccountId> for NullDisputeQueryProvider {
    fn order_dispute_status(_order_id: u64) -> DisputeStatus { DisputeStatus::None }
    fn dispute_resolution(_dispute_id: u64) -> Option<DisputeResolution> { None }
    fn active_dispute_count(_domain: u8, _account: &AccountId) -> u32 { 0 }
}

// ============================================================================
// #8 Token Sale 查询接口
// ============================================================================

/// Token Sale 查询接口
///
/// 供 entity/governance/frontend 等模块查询 Token Sale 状态，
/// 无需直接依赖 pallet-entity-tokensale。
pub trait TokenSaleProvider<Balance> {
    /// 获取实体当前活跃的发售轮次 ID
    fn active_sale_round(entity_id: u64) -> Option<u64>;

    /// 获取发售轮次状态
    fn sale_round_status(round_id: u64) -> Option<TokenSaleStatus>;

    /// 获取轮次已售数量
    fn sold_amount(round_id: u64) -> Option<Balance>;

    /// 获取轮次剩余数量
    fn remaining_amount(round_id: u64) -> Option<Balance>;

    /// 获取轮次参与人数
    fn participants_count(round_id: u64) -> Option<u32>;

    /// 检查实体是否有活跃的发售
    fn has_active_sale(entity_id: u64) -> bool {
        Self::active_sale_round(entity_id).is_some()
    }

    /// 获取轮次总供应量
    fn sale_total_supply(round_id: u64) -> Option<Balance> {
        let _ = round_id;
        None
    }

    /// 获取轮次所属实体 ID
    fn sale_entity_id(round_id: u64) -> Option<u64> {
        let _ = round_id;
        None
    }
}

/// 空 Token Sale 提供者（测试用或未启用 Token Sale 时）
pub struct NullTokenSaleProvider;

impl<Balance> TokenSaleProvider<Balance> for NullTokenSaleProvider {
    fn active_sale_round(_entity_id: u64) -> Option<u64> { None }
    fn sale_round_status(_round_id: u64) -> Option<TokenSaleStatus> { None }
    fn sold_amount(_round_id: u64) -> Option<Balance> { None }
    fn remaining_amount(_round_id: u64) -> Option<Balance> { None }
    fn participants_count(_round_id: u64) -> Option<u32> { None }
}

// ============================================================================
// P8: 锁仓/归属 (Vesting) 接口
// ============================================================================

/// 锁仓/归属计划
///
/// 定义代币的线性释放规则：悬崖期 + 线性释放期。
/// 用于 Token Sale 锁仓、团队分配、投资者保护等场景。
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct VestingSchedule {
    /// 锁仓总量
    pub total: u128,
    /// 已释放量
    pub released: u128,
    /// 开始区块
    pub start_block: u64,
    /// 悬崖期（区块数，悬崖期内不释放）
    pub cliff_blocks: u64,
    /// 线性释放期（区块数，悬崖期后线性释放）
    pub vesting_blocks: u64,
}

impl VestingSchedule {
    /// 计算在指定区块时可释放的数量（尚未领取的部分）
    pub fn releasable_at(&self, current_block: u64) -> u128 {
        let cliff_end = self.start_block.saturating_add(self.cliff_blocks);
        if current_block < cliff_end {
            return 0;
        }
        let elapsed = current_block.saturating_sub(cliff_end);
        let total_vested = if self.vesting_blocks == 0 || elapsed >= self.vesting_blocks {
            self.total
        } else {
            self.total.saturating_mul(elapsed as u128) / (self.vesting_blocks as u128)
        };
        total_vested.saturating_sub(self.released)
    }

    /// 是否已完全释放
    pub fn is_fully_released(&self) -> bool {
        self.released >= self.total
    }
}

/// 锁仓/归属查询接口
///
/// 供 token sale / governance / frontend 等模块查询和操作锁仓计划，
/// 无需直接依赖 vesting 实现模块。
pub trait VestingProvider<AccountId> {
    /// 获取账户在指定实体下的锁仓余额
    fn vesting_balance(entity_id: u64, account: &AccountId) -> u128;

    /// 获取当前可释放的余额
    fn releasable_balance(entity_id: u64, account: &AccountId) -> u128;

    /// 释放已到期的锁仓代币，返回实际释放数量
    fn release(entity_id: u64, account: &AccountId) -> Result<u128, DispatchError>;

    /// 获取锁仓计划详情
    fn vesting_schedule(entity_id: u64, account: &AccountId) -> Option<VestingSchedule> {
        let _ = (entity_id, account);
        None
    }

    /// 检查账户是否有活跃锁仓
    fn has_vesting(entity_id: u64, account: &AccountId) -> bool {
        Self::vesting_balance(entity_id, account) > 0
    }
}

/// 空锁仓提供者（测试用或未启用锁仓时）
pub struct NullVestingProvider;

impl<AccountId> VestingProvider<AccountId> for NullVestingProvider {
    fn vesting_balance(_: u64, _: &AccountId) -> u128 { 0 }
    fn releasable_balance(_: u64, _: &AccountId) -> u128 { 0 }
    fn release(_: u64, _: &AccountId) -> Result<u128, DispatchError> { Ok(0) }
}

// ============================================================================
// P9: 分红查询接口
// ============================================================================

/// 分红查询接口
///
/// 供 governance / frontend 等模块查询和领取分红，
/// 无需直接依赖 token 模块的分红实现。
pub trait DividendProvider<AccountId, Balance: Default> {
    /// 查询待领取分红
    fn pending_dividend(entity_id: u64, account: &AccountId) -> Balance;

    /// 领取分红
    fn claim_dividend(entity_id: u64, account: &AccountId) -> Result<Balance, DispatchError>;

    /// 检查分红是否已激活
    fn is_dividend_active(entity_id: u64) -> bool;

    /// 获取下次分红时间（区块号，None = 未配置或未激活）
    fn next_distribution_at(entity_id: u64) -> Option<u64> {
        let _ = entity_id;
        None
    }

    /// 获取累计已分红总额
    fn total_distributed(entity_id: u64) -> Balance {
        let _ = entity_id;
        Default::default()
    }
}

/// 空分红提供者（测试用或未启用分红时）
pub struct NullDividendProvider;

impl<AccountId, Balance: Default> DividendProvider<AccountId, Balance> for NullDividendProvider {
    fn pending_dividend(_: u64, _: &AccountId) -> Balance { Default::default() }
    fn claim_dividend(_: u64, _: &AccountId) -> Result<Balance, DispatchError> { Ok(Default::default()) }
    fn is_dividend_active(_: u64) -> bool { false }
}

// ============================================================================
// P12: 紧急暂停接口
// ============================================================================

/// 紧急暂停接口
///
/// 全局紧急暂停机制，用于发现严重漏洞或遭受攻击时一键暂停核心操作。
/// 由 Root 调用，影响所有交易、订单、Token 操作。
pub trait EmergencyProvider {
    /// 检查系统是否处于紧急暂停状态
    fn is_emergency_paused() -> bool;

    /// 检查指定模块是否被暂停（模块 ID 由各 pallet 自定义）
    ///
    /// 默认行为：跟随全局暂停状态
    fn is_module_paused(module_id: u8) -> bool {
        let _ = module_id;
        Self::is_emergency_paused()
    }

    /// 暂停系统（仅 Root）
    fn pause_system() -> Result<(), DispatchError> {
        Err(DispatchError::Other("not implemented"))
    }

    /// 恢复系统（仅 Root）
    fn resume_system() -> Result<(), DispatchError> {
        Err(DispatchError::Other("not implemented"))
    }
}

/// 空紧急暂停提供者（测试用，系统永不暂停）
pub struct NullEmergencyProvider;

impl EmergencyProvider for NullEmergencyProvider {
    fn is_emergency_paused() -> bool { false }
}

// ============================================================================
// 评价查询接口
// ============================================================================

/// 评价查询接口
///
/// 供 shop/product/order/governance 等模块查询评价信息，
/// 无需直接依赖 pallet-entity-review。
pub trait ReviewProvider<AccountId> {
    /// 获取 Shop 平均评分（0-100，0 = 无评分）
    fn shop_average_rating(shop_id: u64) -> u8;

    /// 获取 Shop 评价总数
    fn shop_review_count(shop_id: u64) -> u32;

    /// 获取 Product 平均评分（0-100，0 = 无评分）
    fn product_average_rating(product_id: u64) -> u8;

    /// 获取 Product 评价总数
    fn product_review_count(product_id: u64) -> u32;

    /// 检查用户是否已评价某订单
    fn has_reviewed_order(order_id: u64, reviewer: &AccountId) -> bool;

    /// 检查 Entity 是否启用评价系统
    fn is_review_enabled(entity_id: u64) -> bool {
        let _ = entity_id;
        true
    }

    /// 获取用户在某 Entity 下的总评价数
    fn user_review_count(entity_id: u64, reviewer: &AccountId) -> u32 {
        let _ = (entity_id, reviewer);
        0
    }
}

/// 空评价提供者（测试用或未启用评价系统时）
pub struct NullReviewProvider;

impl<AccountId> ReviewProvider<AccountId> for NullReviewProvider {
    fn shop_average_rating(_: u64) -> u8 { 0 }
    fn shop_review_count(_: u64) -> u32 { 0 }
    fn product_average_rating(_: u64) -> u8 { 0 }
    fn product_review_count(_: u64) -> u32 { 0 }
    fn has_reviewed_order(_: u64, _: &AccountId) -> bool { false }
}

// ============================================================================
// 市场查询接口
// ============================================================================

/// 市场/交易查询接口
///
/// 供 token/governance/frontend 等模块查询 Entity Token 二级市场信息，
/// 无需直接依赖 pallet-entity-market。
pub trait MarketProvider<AccountId, Balance> {
    /// 检查 Entity Token 是否有活跃的交易对
    fn has_active_market(entity_id: u64) -> bool;

    /// 获取 Entity Token 近期交易量（原生代币单位）
    ///
    /// "24h" 基于区块数估算（假设 6s/block ≈ 14400 blocks），
    /// 实现方应使用滑动窗口或最近 N 个区块的交易量统计。
    fn trading_volume_24h(entity_id: u64) -> Balance;

    /// 获取当前最佳买价（最高买单价格）
    fn best_bid(entity_id: u64) -> Option<Balance>;

    /// 获取当前最佳卖价（最低卖单价格）
    fn best_ask(entity_id: u64) -> Option<Balance>;

    /// 获取某用户的活跃挂单数量
    fn user_active_order_count(entity_id: u64, account: &AccountId) -> u32 {
        let _ = (entity_id, account);
        0
    }

    /// 市场是否被暂停交易
    fn is_market_paused(entity_id: u64) -> bool {
        let _ = entity_id;
        false
    }
}

/// 空市场提供者（测试用或未启用市场时）
pub struct NullMarketProvider;

impl<AccountId, Balance: Default> MarketProvider<AccountId, Balance> for NullMarketProvider {
    fn has_active_market(_: u64) -> bool { false }
    fn trading_volume_24h(_: u64) -> Balance { Default::default() }
    fn best_bid(_: u64) -> Option<Balance> { None }
    fn best_ask(_: u64) -> Option<Balance> { None }
}

// ============================================================================
// 会员生命周期回调
// ============================================================================

/// 会员移除回调（通知依赖方清理关联存储）
///
/// 由 `pallet-entity-member` 在 `do_remove_member` 末尾调用。
/// 各佣金插件实现此 trait 以清理各自的 per-user 存储（如 claim 记录等）。
///
/// 支持通过元组组合多个实现：`type OnMemberRemoved = (PluginA, PluginB);`
pub trait OnMemberRemoved<AccountId> {
    fn on_member_removed(entity_id: u64, account: &AccountId);
}

impl<AccountId> OnMemberRemoved<AccountId> for () {
    fn on_member_removed(_: u64, _: &AccountId) {}
}

macro_rules! impl_on_member_removed_tuple {
    ( $first:ident $(, $rest:ident )* ) => {
        impl<AccountId, $first: OnMemberRemoved<AccountId>, $( $rest: OnMemberRemoved<AccountId> ),*>
            OnMemberRemoved<AccountId> for ($first, $( $rest ),*)
        {
            fn on_member_removed(entity_id: u64, account: &AccountId) {
                $first::on_member_removed(entity_id, account);
                $( $rest::on_member_removed(entity_id, account); )*
            }
        }
        impl_on_member_removed_tuple!( $( $rest ),* );
    };
    () => {};
}

impl_on_member_removed_tuple!(A, B, C, D, E, F, G, H);
