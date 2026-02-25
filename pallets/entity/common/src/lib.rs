//! # 实体公共模块 (pallet-entity-common)
//!
//! 定义实体各子模块共享的类型和 Trait 接口

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

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
            Self::Merchant | Self::ServiceProvider => GovernanceMode::None,
            Self::Enterprise => GovernanceMode::DualTrack,
            Self::DAO => GovernanceMode::FullDAO,
            Self::Community => GovernanceMode::Advisory,
            Self::Project => GovernanceMode::DualTrack,
            Self::Fund => GovernanceMode::Committee,
            Self::Custom(_) => GovernanceMode::None,
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
    /// 无治理（管理员全权控制）
    #[default]
    None,
    /// 咨询型（提案不自动执行，仅收集意见）
    Advisory,
    /// 双轨制（管理员可快速执行，重大决策需投票）
    DualTrack,
    /// 委员会（委员会成员投票决策）
    Committee,
    /// 完全 DAO（所有决策需投票）
    FullDAO,
    /// 分层治理（不同级别决策不同阈值）
    Tiered,
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
    /// 待激活
    Pending,
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
                // 如果 Shop 自身已 Closed，优先显示 Closed
                if *shop_status == ShopOperatingStatus::Closed {
                    return Self::Closed;
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
            ShopOperatingStatus::Closing => Self::Closed,
            ShopOperatingStatus::Pending => Self::Pending,
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
    /// 待激活
    #[default]
    Pending,
    /// 营业中
    Active,
    /// 暂停营业
    Paused,
    /// 资金耗尽（自动暂停）
    FundDepleted,
    /// 关闭中
    Closing,
    /// 已关闭
    Closed,
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
}

/// 会员体系模式
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum MemberMode {
    /// 继承模式：会员数据存储在 Entity 级别，所有 Shop 共享
    #[default]
    Inherit,
    /// 独立模式：会员数据存储在 Shop 级别，各 Shop 独立
    Independent,
    /// 混合模式：Entity + Shop 双层会员体系
    Hybrid,
}

impl MemberMode {
    /// 是否在 Entity 级别存储会员
    pub fn uses_entity_members(&self) -> bool {
        matches!(self, Self::Inherit | Self::Hybrid)
    }
    
    /// 是否在 Shop 级别存储会员
    pub fn uses_shop_members(&self) -> bool {
        matches!(self, Self::Independent | Self::Hybrid)
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

    /// 检查是否设置了指定标记
    pub fn contains(&self, flag: u8) -> bool {
        self.0 & flag == flag
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
}

impl Default for MemberRegistrationPolicy {
    fn default() -> Self {
        Self::OPEN
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
    /// 0 = None, 1 = Whitelist, 2 = Blacklist, 3 = KycRequired, 4 = MembersOnly
    pub fn default_transfer_restriction(&self) -> u8 {
        match self {
            Self::Points => 0,       // None
            Self::Membership => 4,   // MembersOnly
            Self::Governance => 3,   // KycRequired
            Self::Share | Self::Bond => 3, // KycRequired
            Self::Equity => 1,       // Whitelist
            Self::Hybrid(_) => 0,    // None (可配置)
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
    /// 从 u8 转换
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Whitelist,
            2 => Self::Blacklist,
            3 => Self::KycRequired,
            4 => Self::MembersOnly,
            _ => Self::None,
        }
    }
}

/// 分红配置
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
    /// 其他
    Other,
}

// ============================================================================
// 会员相关类型
// ============================================================================

/// 会员等级
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum MemberLevel {
    #[default]
    Normal,     // 普通会员
    Silver,     // 银卡会员
    Gold,       // 金卡会员
    Platinum,   // 白金会员
    Diamond,    // 钻石会员
}

// ============================================================================
// 订单相关类型
// ============================================================================

/// 订单状态
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
pub enum MallOrderStatus {
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
    
    /// 更新实体统计（销售额、订单数）
    fn update_entity_stats(entity_id: u64, sales_amount: u128, order_count: u32) -> Result<(), DispatchError>;
    
    /// 更新实体评分
    fn update_entity_rating(entity_id: u64, rating: u8) -> Result<(), DispatchError>;
    
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
    
    /// 检查是否为 Entity 管理员（owner 或 admins 列表中）
    fn is_entity_admin(entity_id: u64, account: &AccountId) -> bool {
        let _ = (entity_id, account);
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
}

/// 向后兼容别名（旧 EntityProvider）
pub trait LegacyShopProvider<AccountId>: EntityProvider<AccountId> {}

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
    
    /// 获取 Shop 会员模式
    fn shop_member_mode(shop_id: u64) -> MemberMode;
    
    /// 检查是否为 Shop 管理员
    fn is_shop_manager(shop_id: u64, account: &AccountId) -> bool;
    
    // ==================== 统计更新 ====================
    
    /// 更新 Shop 统计（销售额、订单数）
    fn update_shop_stats(shop_id: u64, sales_amount: u128, order_count: u32) -> Result<(), DispatchError>;
    
    /// 更新 Shop 评分
    fn update_shop_rating(shop_id: u64, rating: u8) -> Result<(), DispatchError>;
    
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
        member_mode: MemberMode,
    ) -> Result<u64, DispatchError> {
        let _ = (entity_id, name, shop_type, member_mode);
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
    fn update_entity_rating(_entity_id: u64, _rating: u8) -> Result<(), DispatchError> { Ok(()) }
}

/// 向后兼容别名（旧 EntityProvider 的空实现）
pub type NullLegacyShopProvider = NullEntityProvider;

/// 空 Shop 提供者（测试用）
pub struct NullShopProvider;

impl<AccountId: Default> ShopProvider<AccountId> for NullShopProvider {
    fn shop_exists(_shop_id: u64) -> bool { false }
    fn is_shop_active(_shop_id: u64) -> bool { false }
    fn shop_entity_id(_shop_id: u64) -> Option<u64> { None }
    fn shop_owner(_shop_id: u64) -> Option<AccountId> { None }
    fn shop_account(_shop_id: u64) -> AccountId { AccountId::default() }
    fn shop_type(_shop_id: u64) -> Option<ShopType> { None }
    fn shop_member_mode(_shop_id: u64) -> MemberMode { MemberMode::default() }
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
}

/// 向后兼容别名
pub trait ShopTokenProvider<AccountId, Balance>: EntityTokenProvider<AccountId, Balance> {}

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
    fn get_cos_usdt_price() -> u64;
}

// ============================================================================
// 佣金资金保护接口
// ============================================================================

/// 佣金资金保护接口
///
/// 供 Shop 模块在扣费时查询已承诺（pending + shopping）的佣金资金，
/// 防止运营扣费侵占用户佣金。
pub trait CommissionFundGuard {
    /// 获取 shop 已承诺的佣金资金总额（pending_total + shopping_total）
    fn protected_funds(shop_id: u64) -> u128;
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
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        order_amount: Balance,
    ) -> Result<(), DispatchError>;

    /// 订单取消/退款时撤销佣金
    fn on_order_cancelled(order_id: u64) -> Result<(), DispatchError>;
}

/// 空佣金处理（无佣金系统时使用）
impl<AccountId, Balance> OrderCommissionHandler<AccountId, Balance> for () {
    fn on_order_completed(_: u64, _: u64, _: &AccountId, _: Balance) -> Result<(), DispatchError> { Ok(()) }
    fn on_order_cancelled(_: u64) -> Result<(), DispatchError> { Ok(()) }
}

/// 空定价提供者（测试用）
pub struct NullPricingProvider;

impl PricingProvider for NullPricingProvider {
    fn get_cos_usdt_price() -> u64 {
        // 默认价格：0.000001 USDT/NEX（精度 10^6 = 1）
        1
    }
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
}

/// 向后兼容别名
pub type NullShopTokenProvider = NullEntityTokenProvider;
