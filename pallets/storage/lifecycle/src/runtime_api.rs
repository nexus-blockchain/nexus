//! Runtime API 定义：存储生命周期仪表盘
//!
//! 提供以下接口：
//! - `get_archive_stats`: 获取指定数据类型的归档统计
//! - `get_archive_config`: 获取当前有效归档配置
//! - `get_data_status`: 查询单条数据的归档级别
//! - `is_archival_paused`: 查询归档是否暂停

use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
    /// 存储生命周期 Runtime API
    ///
    /// 提供归档仪表盘查询接口
    pub trait StorageLifecycleApi {
        /// 获取指定数据类型的归档统计信息
        ///
        /// ### 参数
        /// - `data_type`: 数据类型标识（UTF-8 字节）
        ///
        /// ### 返回
        /// - (total_l1, total_l2, total_purged, bytes_saved, last_archive_at)
        fn get_archive_stats(data_type: Vec<u8>) -> (u64, u64, u64, u64, u64);

        /// 获取当前有效归档配置
        ///
        /// ### 返回
        /// - (l1_delay, l2_delay, purge_delay, purge_enabled, max_batch_size)
        fn get_archive_config() -> (u32, u32, u32, bool, u32);

        /// 查询单条数据的归档级别
        ///
        /// ### 参数
        /// - `data_type`: 数据类型标识
        /// - `data_id`: 数据 ID
        ///
        /// ### 返回
        /// - 归档级别 (0=Active, 1=L1, 2=L2, 3=Purged)
        fn get_data_status(data_type: Vec<u8>, data_id: u64) -> u8;

        /// 查询归档是否暂停
        ///
        /// ### 返回
        /// - true 表示归档已暂停
        fn is_archival_paused() -> bool;
    }
}
