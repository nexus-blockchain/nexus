//! Cross-module trait interfaces
//!
//! All trait definitions organized into semantic submodules:
//!
//! - `core`             — EntityProvider, ShopProvider, ProductProvider, OrderProvider
//! - `asset`            — EntityTokenProvider, AssetLedgerPort, PricingProvider, EntityTokenPriceProvider, FeeConfigProvider
//! - `compliance`       — DisclosureProvider (+ Read/Write split), KycProvider, GovernanceProvider
//! - `member`           — MemberProvider (+ Query/Write split), CommissionFundGuard, EntityTreasuryPort,
//!                        ShopFundPort, FundProtectionPort, OrderCommissionHandler, TokenOrderCommissionHandler,
//!                        ShoppingBalanceProvider
//! - `incentive`        — LoyaltyReadPort, LoyaltyWritePort, LoyaltyTokenReadPort, LoyaltyTokenWritePort,
//!                        DisputeQueryProvider, TokenSaleProvider,
//!                        VestingProvider, DividendProvider, EmergencyProvider, ReviewProvider, MarketProvider
//! - `hooks`            — OnEntityStatusChange, OnOrderStatusChange, OnKycStatusChange,
//!                        OnDisclosureViolation, OnMemberRemoved, PointsCleanup
//! - `governance_ports` — MarketGovernancePort, CommissionGovernancePort, SingleLineGovernancePort,
//!                        KycGovernancePort, ShopGovernancePort, TokenGovernancePort
//! - `repurchase`       — AutoRepurchasePort, NullAutoRepurchasePort

pub mod asset;
pub mod compliance;
pub mod core;
pub mod governance_ports;
pub mod hooks;
pub mod incentive;
pub mod member;
pub mod repurchase;

// ============================================================================
// 全量 Re-export — 保持外部 import 路径不变
// ============================================================================

pub use self::core::*;
pub use asset::*;
pub use compliance::*;
pub use governance_ports::*;
pub use hooks::*;
pub use incentive::*;
pub use member::*;
pub use repurchase::*;
