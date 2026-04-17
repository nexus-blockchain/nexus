//! # 公共宏定义
//!
//! 本模块定义 Trading 相关的公共宏，供多个 pallet 共享。
//!
//! ## 版本历史
//! - v0.5.0 (2026-01-25): 初始版本，添加 define_balance_of 宏

/// 定义 BalanceOf 类型别名的宏
///
/// ## 说明
/// 避免在每个 pallet 中重复定义相同的类型别名
///
/// ## 使用示例
/// ```ignore
/// pallet_trading_common::define_balance_of!();
/// ```
#[macro_export]
macro_rules! define_balance_of {
    () => {
        /// 货币余额类型别名
        pub type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::Currency<
            <T as frame_system::Config>::AccountId,
        >>::Balance;
    };
}
