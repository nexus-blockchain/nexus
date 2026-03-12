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

pub mod core;
pub mod asset;
pub mod compliance;
pub mod member;
pub mod incentive;
pub mod hooks;
pub mod governance_ports;

// ============================================================================
// 全量 Re-export — 保持外部 import 路径不变
// ============================================================================

pub use self::core::*;
pub use asset::*;
pub use compliance::*;
pub use member::*;
pub use incentive::*;
pub use hooks::*;
pub use governance_ports::*;
