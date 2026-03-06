//! Runtime API 定义：用于前端查询 UserFunding 账户信息
//!
//! 提供以下接口：
//! - `get_user_funding_account`: 获取用户存储资金账户地址
//! - `get_user_funding_balance`: 获取用户存储资金余额
//! - `get_subject_usage`: 获取特定业务的费用消耗

use codec::Codec;
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
    /// IPFS 存储模块 Runtime API
    /// 
    /// 用于前端查询用户存储账户信息
    pub trait StorageServiceApi<AccountId, Balance> 
    where
        AccountId: Codec,
        Balance: Codec,
    {
        /// 获取用户存储资金账户地址（派生地址）
        /// 
        /// ### 参数
        /// - `user`: 用户账户
        /// 
        /// ### 返回
        /// - 派生的 UserFunding 账户地址
        fn get_user_funding_account(user: AccountId) -> AccountId;
        
        /// 获取用户存储资金余额
        /// 
        /// ### 参数
        /// - `user`: 用户账户
        /// 
        /// ### 返回
        /// - 用户存储资金账户的可用余额
        fn get_user_funding_balance(user: AccountId) -> Balance;
        
        /// 获取用户特定业务的费用消耗
        /// 
        /// ### 参数
        /// - `user`: 用户账户
        /// - `domain`: 业务域编号（0=Evidence, 10=Product, 11=Entity, 12=Shop, 98=General）
        /// - `subject_id`: 业务 ID
        /// 
        /// ### 返回
        /// - 该业务累计消耗的费用
        fn get_subject_usage(user: AccountId, domain: u8, subject_id: u64) -> Balance;
        
        /// 获取用户所有业务的费用消耗汇总
        /// 
        /// ### 参数
        /// - `user`: 用户账户
        /// 
        /// ### 返回
        /// - Vec<(domain, subject_id, amount)> 费用消耗列表
        fn get_user_all_usage(user: AccountId) -> Vec<(u8, u64, Balance)>;
    }
}
