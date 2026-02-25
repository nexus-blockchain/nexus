#![cfg_attr(not(feature = "std"), no_std)]

//! # Trading Common (交易公共工具库)
//!
//! ## 概述
//!
//! 本 crate 提供交易相关的公共工具函数和统一接口，包括：
//! - 公共类型定义（TronAddress, MomentOf 等）
//! - 公共 Trait 定义（PricingProvider, MakerInterface, MakerCreditInterface）
//! - 脱敏函数（姓名、身份证、生日）
//! - TRON 地址验证
//! - 时间转换工具
//!
//! ## 特点
//!
//! - ✅ 纯 Rust crate，无链上存储
//! - ✅ 可被多个 pallet 共享
//! - ✅ no_std 兼容
//!
//! ## 版本历史
//!
//! - v0.1.0: 初始版本
//! - v0.2.0 (2026-01-18): 添加统一的 MakerCreditInterface trait
//! - v0.3.0 (2026-01-18): 添加时间转换工具函数
//! - v0.4.0 (2026-01-18): 统一公共类型和 Trait 定义

pub mod types;
pub mod traits;
pub mod mask;
pub mod validation;
pub mod time;
pub mod macros;

// ===== 重新导出公共类型 =====
pub use types::{
    TronAddress,
    TronTxHash,
    MomentOf,
    Cid,
    TxHash,
    UsdtTradeStatus,
    BuyerDepositStatus,
    PaymentVerificationResult,
    calculate_payment_verification_result,
    calculate_deposit_forfeit_rate,
};

// ===== 重新导出公共 Trait =====
pub use traits::{
    PricingProvider,
    DepositCalculator,
    DepositCalculatorImpl,
    PriceOracle,
    TwapWindow,
};

// 重新导出工具函数
pub use mask::{mask_name, mask_id_card, mask_birthday};
pub use validation::is_valid_tron_address;
pub use time::{
    blocks_to_seconds,
    seconds_to_blocks,
    estimate_timestamp_from_block,
    estimate_remaining_seconds,
    format_duration,
    DEFAULT_BLOCK_TIME_SECS,
};
