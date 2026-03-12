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
