//! # 时间转换工具模块
//!
//! ## 概述
//!
//! 提供区块号与时间戳之间的转换工具，用于：
//! - 链上区块号存储 → 前端可读时间显示
//! - 统一时间表示方式
//!
//! ## 设计原则
//!
//! - 链上使用区块号（高效、确定性）
//! - 前端通过转换函数显示人可读时间
//! - 假设每区块 6 秒（可配置）
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use pallet_trading_common::time::*;
//!
//! // 区块号转秒数
//! let seconds = blocks_to_seconds(100); // 600 秒
//!
//! // 秒数转区块数
//! let blocks = seconds_to_blocks(3600); // 600 块（1小时）
//!
//! // 预估未来区块的时间戳
//! let future_ts = estimate_timestamp_from_block(
//!     12345,      // 目标区块
//!     12000,      // 当前区块
//!     1705500000, // 当前时间戳
//! );
//!
//! // 计算剩余时间
//! let remaining = estimate_remaining_seconds(12345, 12000); // 2070 秒
//! ```

use sp_std::vec::Vec;

/// 默认区块时间（秒）
/// Substrate 默认配置通常为 6 秒
pub const DEFAULT_BLOCK_TIME_SECS: u64 = 6;

/// 函数级详细中文注释：区块数转换为秒数
///
/// ## 参数
/// - `blocks`: 区块数量
///
/// ## 返回
/// - `u64`: 对应的秒数
///
/// ## 示例
/// ```rust,ignore
/// let seconds = blocks_to_seconds(100); // 600 秒 = 10 分钟
/// ```
#[inline]
pub fn blocks_to_seconds(blocks: u64) -> u64 {
    blocks.saturating_mul(DEFAULT_BLOCK_TIME_SECS)
}

/// 函数级详细中文注释：秒数转换为区块数
///
/// ## 参数
/// - `seconds`: 秒数
///
/// ## 返回
/// - `u64`: 对应的区块数（向上取整）
///
/// ## 示例
/// ```rust,ignore
/// let blocks = seconds_to_blocks(3600); // 600 块 = 1 小时
/// ```
#[inline]
pub fn seconds_to_blocks(seconds: u64) -> u64 {
    // 向上取整，确保超时时间足够
    seconds.saturating_add(DEFAULT_BLOCK_TIME_SECS - 1) / DEFAULT_BLOCK_TIME_SECS
}

/// 函数级详细中文注释：根据区块号预估 Unix 时间戳
///
/// ## 参数
/// - `target_block`: 目标区块号
/// - `current_block`: 当前区块号
/// - `current_timestamp`: 当前 Unix 时间戳（秒）
///
/// ## 返回
/// - `u64`: 预估的 Unix 时间戳（秒）
///
/// ## 算法
/// ```text
/// 时间差 = (目标区块 - 当前区块) × 区块时间
/// 预估时间 = 当前时间 + 时间差
/// ```
///
/// ## 注意
/// - 支持目标区块在过去或未来
/// - 预估值会有轻微误差（区块时间不完全恒定）
#[inline]
pub fn estimate_timestamp_from_block(
    target_block: u64,
    current_block: u64,
    current_timestamp: u64,
) -> u64 {
    if target_block >= current_block {
        // 目标在未来
        let block_diff = target_block.saturating_sub(current_block);
        let time_diff = blocks_to_seconds(block_diff);
        current_timestamp.saturating_add(time_diff)
    } else {
        // 目标在过去
        let block_diff = current_block.saturating_sub(target_block);
        let time_diff = blocks_to_seconds(block_diff);
        current_timestamp.saturating_sub(time_diff)
    }
}

/// 函数级详细中文注释：计算剩余秒数
///
/// ## 参数
/// - `target_block`: 目标区块号（如超时区块）
/// - `current_block`: 当前区块号
///
/// ## 返回
/// - `u64`: 剩余秒数，如果已过期返回 0
///
/// ## 用途
/// - 前端显示倒计时
/// - 判断是否即将超时
#[inline]
pub fn estimate_remaining_seconds(target_block: u64, current_block: u64) -> u64 {
    if target_block > current_block {
        let remaining_blocks = target_block.saturating_sub(current_block);
        blocks_to_seconds(remaining_blocks)
    } else {
        0 // 已过期
    }
}

/// 函数级详细中文注释：格式化时间间隔为可读字符串
///
/// ## 参数
/// - `seconds`: 秒数
///
/// ## 返回
/// - `Vec<u8>`: UTF-8 编码的可读字符串
///
/// ## 输出示例
/// - `< 1m` (小于1分钟)
/// - `5m` (5分钟)
/// - `1h 30m` (1小时30分钟)
/// - `2d 5h` (2天5小时)
///
/// ## 注意
/// 返回 `Vec<u8>` 以支持 no_std 环境
pub fn format_duration(seconds: u64) -> Vec<u8> {
    if seconds < 60 {
        return b"< 1m".to_vec();
    }

    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        let remaining_hours = hours % 24;
        format_days_hours(days, remaining_hours)
    } else if hours > 0 {
        let remaining_minutes = minutes % 60;
        format_hours_minutes(hours, remaining_minutes)
    } else {
        format_minutes(minutes)
    }
}

/// 内部函数：格式化天和小时
fn format_days_hours(days: u64, hours: u64) -> Vec<u8> {
    let mut result = Vec::with_capacity(16);
    append_number(&mut result, days);
    result.extend_from_slice(b"d");
    if hours > 0 {
        result.extend_from_slice(b" ");
        append_number(&mut result, hours);
        result.extend_from_slice(b"h");
    }
    result
}

/// 内部函数：格式化小时和分钟
fn format_hours_minutes(hours: u64, minutes: u64) -> Vec<u8> {
    let mut result = Vec::with_capacity(16);
    append_number(&mut result, hours);
    result.extend_from_slice(b"h");
    if minutes > 0 {
        result.extend_from_slice(b" ");
        append_number(&mut result, minutes);
        result.extend_from_slice(b"m");
    }
    result
}

/// 内部函数：格式化分钟
fn format_minutes(minutes: u64) -> Vec<u8> {
    let mut result = Vec::with_capacity(8);
    append_number(&mut result, minutes);
    result.extend_from_slice(b"m");
    result
}

/// 内部函数：将数字追加到 Vec<u8>
fn append_number(buf: &mut Vec<u8>, n: u64) {
    if n == 0 {
        buf.push(b'0');
        return;
    }
    
    let mut digits = Vec::with_capacity(20);
    let mut num = n;
    while num > 0 {
        digits.push(b'0' + (num % 10) as u8);
        num /= 10;
    }
    digits.reverse();
    buf.extend_from_slice(&digits);
}

// ===== 便捷常量 =====

/// 1 分钟的区块数
pub const BLOCKS_PER_MINUTE: u64 = 60 / DEFAULT_BLOCK_TIME_SECS; // 10 块

/// 1 小时的区块数
pub const BLOCKS_PER_HOUR: u64 = 3600 / DEFAULT_BLOCK_TIME_SECS; // 600 块

/// 1 天的区块数
pub const BLOCKS_PER_DAY: u64 = 86400 / DEFAULT_BLOCK_TIME_SECS; // 14400 块

// ===== 单元测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_to_seconds() {
        assert_eq!(blocks_to_seconds(0), 0);
        assert_eq!(blocks_to_seconds(1), 6);
        assert_eq!(blocks_to_seconds(10), 60);
        assert_eq!(blocks_to_seconds(100), 600);
        assert_eq!(blocks_to_seconds(600), 3600); // 1 小时
    }

    #[test]
    fn test_seconds_to_blocks() {
        assert_eq!(seconds_to_blocks(0), 0);
        assert_eq!(seconds_to_blocks(6), 1);
        assert_eq!(seconds_to_blocks(7), 2);  // 向上取整
        assert_eq!(seconds_to_blocks(60), 10);
        assert_eq!(seconds_to_blocks(3600), 600); // 1 小时
    }

    #[test]
    fn test_estimate_timestamp_from_block() {
        let current_block = 1000u64;
        let current_ts = 1705500000u64;

        // 未来区块
        let future = estimate_timestamp_from_block(1100, current_block, current_ts);
        assert_eq!(future, 1705500600); // +600秒

        // 过去区块
        let past = estimate_timestamp_from_block(900, current_block, current_ts);
        assert_eq!(past, 1705499400); // -600秒

        // 当前区块
        let now = estimate_timestamp_from_block(1000, current_block, current_ts);
        assert_eq!(now, 1705500000);
    }

    #[test]
    fn test_estimate_remaining_seconds() {
        assert_eq!(estimate_remaining_seconds(1100, 1000), 600);
        assert_eq!(estimate_remaining_seconds(1000, 1000), 0);
        assert_eq!(estimate_remaining_seconds(900, 1000), 0); // 已过期
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), b"< 1m".to_vec());
        assert_eq!(format_duration(59), b"< 1m".to_vec());
        assert_eq!(format_duration(60), b"1m".to_vec());
        assert_eq!(format_duration(300), b"5m".to_vec());
        assert_eq!(format_duration(3600), b"1h".to_vec());
        assert_eq!(format_duration(5400), b"1h 30m".to_vec());
        assert_eq!(format_duration(86400), b"1d".to_vec());
        assert_eq!(format_duration(90000), b"1d 1h".to_vec());
    }

    #[test]
    fn test_constants() {
        assert_eq!(BLOCKS_PER_MINUTE, 10);
        assert_eq!(BLOCKS_PER_HOUR, 600);
        assert_eq!(BLOCKS_PER_DAY, 14400);
    }

    // ===== R2 回归测试: 边界值 =====

    #[test]
    fn r2_seconds_to_blocks_max_u64_no_overflow() {
        // saturating_add 防止溢出
        let result = seconds_to_blocks(u64::MAX);
        assert_eq!(result, u64::MAX / DEFAULT_BLOCK_TIME_SECS);
    }

    #[test]
    fn r2_blocks_to_seconds_max_u64_saturates() {
        // u64::MAX * 6 会溢出，saturating_mul 返回 u64::MAX
        let result = blocks_to_seconds(u64::MAX);
        assert_eq!(result, u64::MAX);
    }

    #[test]
    fn r2_estimate_timestamp_underflow_saturates() {
        // 目标在远古过去，current_timestamp 很小 → saturating_sub 保护
        let result = estimate_timestamp_from_block(0, 1_000_000, 100);
        assert_eq!(result, 0); // 不会 underflow 到 u64::MAX
    }

    #[test]
    fn r2_format_duration_max_u64() {
        // 极大值不应 panic
        let result = format_duration(u64::MAX);
        assert!(!result.is_empty());
        // 应包含 "d"（天数格式）
        assert!(result.windows(1).any(|w| w == b"d"));
    }
}
