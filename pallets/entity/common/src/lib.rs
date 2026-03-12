//! # 实体公共模块 (pallet-entity-common)
//!
//! 定义实体各子模块共享的类型和 Trait 接口
//!
//! ## 模块结构
//!
//! - `types/`   — 领域枚举、DTO、位掩码（按域分文件）
//! - `traits/`  — 跨模块 Port trait（按角色分文件）
//! - `errors`   — CommonError 共享错误字符串
//! - `pagination` — 标准化分页类型

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod tests;

// ============================================================================
// 子模块声明
// ============================================================================

pub mod errors;
pub mod pagination;
pub mod types;
pub mod traits;

// ============================================================================
// 全量 Re-export — 保持外部 import 路径不变
// ============================================================================

// errors
pub use errors::*;

// pagination
pub use pagination::*;

// types (所有枚举、结构体、位掩码)
pub use types::*;

// traits (所有 trait、Null 实现、blanket impl)
pub use traits::*;
