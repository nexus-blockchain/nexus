//! # OCW TRC20 验证模块
//!
//! 复用 `pallet-trading-trc20-verifier` 共享库，避免代码重复。
//!
//! ## 相比旧版本的改进（由共享库提供）
//! - 并行竞速模式（同时请求多端点，取最快响应）
//! - USDT 合约地址验证（防其他 TRC20 冒充）
//! - 最小确认数检查（防 TRON 链重组）
//! - 可配置端点管理
//! - 按 (from, to, amount) 搜索转账

pub use pallet_trading_trc20_verifier::{
    verify_trc20_transaction,
    verify_trc20_transaction_simple,
    verify_trc20_by_transfer,
    TronTxVerification,
    AmountStatus,
    TransferSearchResult,
    EndpointHealth,
    calculate_amount_status,
    bytes_to_hex,
    hex_to_bytes,
    get_endpoint_health,
    get_sorted_endpoints,
    add_endpoint,
    remove_endpoint,
    DEFAULT_ENDPOINTS,
    TRONGRID_MAINNET,
    USDT_CONTRACT,
    MIN_CONFIRMATIONS,
};
