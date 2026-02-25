use serde::{Deserialize, Serialize};

/// 链上 Bot 信息缓存
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfoCache {
    pub bot_id_hash: String,
    pub owner: String,
    pub public_key: [u8; 32],
    pub is_active: bool,
    pub is_tee_node: bool,
}

/// TEE 节点链上状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeNodeStatus {
    pub is_attested: bool,
    pub is_expired: bool,
    pub expires_at: Option<u64>,
}

/// 群规则配置 (链上同步)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainCommunityConfig {
    pub node_requirement: u8,
    pub anti_flood_enabled: bool,
    pub flood_limit: u16,
    pub warn_limit: u8,
    /// 警告达到上限后的动作: 0=mute, 1=kick, 2=ban
    pub warn_action: u8,
    pub welcome_enabled: bool,
    pub version: u32,
    // ── Phase 1 新增字段 ──
    /// 重复消息检测
    #[serde(default)]
    pub anti_duplicate_enabled: bool,
    #[serde(default = "default_duplicate_window")]
    pub duplicate_window_secs: u64,
    #[serde(default = "default_duplicate_threshold")]
    pub duplicate_threshold: u16,
    /// Emoji 数量限制 (0=不限制)
    #[serde(default)]
    pub max_emoji: u16,
    /// 链接数量限制 (0=不限制)
    #[serde(default)]
    pub max_links: u16,
    /// 停用词列表 (逗号分隔)
    #[serde(default)]
    pub stop_words: String,
    /// 欢迎消息模板 (支持 {user} {group} 变量)
    #[serde(default)]
    pub welcome_template: String,
    /// 告别消息模板
    #[serde(default)]
    pub goodbye_template: String,
    /// 警告升级后禁言时长 (秒), warn_action=0 时使用
    #[serde(default = "default_warn_mute_duration")]
    pub warn_mute_duration: u64,
    // ── Phase 2 新增字段 ──
    /// Spam 样本列表 (换行分隔), 用于 SimilarityRule
    #[serde(default)]
    pub spam_samples: String,
    /// 相似度阈值 (0-100, 转换为 0.0-1.0)
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: u8,
    /// 日志频道 ID (管理日志转发目标)
    #[serde(default)]
    pub log_channel_id: String,
    /// CAPTCHA 验证是否启用
    #[serde(default)]
    pub captcha_enabled: bool,
    /// CAPTCHA 超时秒数
    #[serde(default = "default_captcha_timeout")]
    pub captcha_timeout_secs: u64,
    // ── Phase 3 新增字段 ──
    /// Anti-Phishing 是否启用
    #[serde(default)]
    pub antiphishing_enabled: bool,
    /// 贝叶斯分类器阈值 (0-100)
    #[serde(default = "default_bayes_threshold")]
    pub bayes_threshold: u8,
    /// 自定义命令 CSV (trigger|type|response 格式)
    #[serde(default)]
    pub custom_commands_csv: String,
    // ── Phase 4 新增字段 ──
    /// 锁定的消息类型 CSV (photo,video,sticker,...)
    #[serde(default)]
    pub locked_types_csv: String,
    // ── Phase 4b: 订阅层级 + 功能门控 ──
    /// 订阅层级 (0=Free, 1=Basic, 2=Pro, 3=Enterprise)
    #[serde(default)]
    pub subscription_tier: u8,
    /// 最大可启用规则数 (由 tier 决定)
    #[serde(default = "default_max_rules")]
    pub max_rules: u16,
    /// 每日强制广告数 (0=无强制)
    #[serde(default)]
    pub forced_ads_per_day: u8,
    /// 是否可关闭广告
    #[serde(default)]
    pub can_disable_ads: bool,
    /// 社区 ID hash (hex)
    #[serde(default)]
    pub community_id_hash: String,
}

fn default_duplicate_window() -> u64 { 300 }
fn default_duplicate_threshold() -> u16 { 3 }
fn default_warn_mute_duration() -> u64 { 3600 }
fn default_similarity_threshold() -> u8 { 70 }
fn default_captcha_timeout() -> u64 { 120 }
fn default_bayes_threshold() -> u8 { 80 }
fn default_max_rules() -> u16 { 3 } // Free tier default

/// 待提交的动作日志
#[derive(Debug, Clone)]
pub struct PendingActionLog {
    pub community_id_hash: [u8; 32],
    pub action_type: u8,
    pub target_hash: [u8; 32],
    pub sequence: u64,
    pub message_hash: [u8; 32],
    pub signature: [u8; 64],
}

// ============================================================================
// Ad System Types (广告系统)
// ============================================================================

/// 链上广告排期信息
#[derive(Debug, Clone)]
pub struct AdScheduleInfo {
    pub community_id_hash: [u8; 32],
    pub campaign_ids: Vec<u64>,
    pub daily_limit: u8,
    pub delivered_this_era: u32,
}

/// 链上广告活动信息
#[derive(Debug, Clone)]
pub struct AdCampaignInfo {
    pub campaign_id: u64,
    pub advertiser: String,
    pub text: String,
    pub url: String,
    pub bid_per_mille: u128,
    pub delivery_types: u8,
    pub is_active: bool,
    pub is_approved: bool,
}

/// 链上 Peer 端点信息 (从 PeerRegistry 查询)
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub public_key: [u8; 32],
    pub endpoint: String,
    pub registered_at: u64,
    pub last_seen: u64,
}

/// 证明包
#[derive(Debug, Clone)]
pub struct AttestationBundle {
    pub tdx_quote_hash: [u8; 32],
    pub sgx_quote_hash: [u8; 32],
    pub mrtd: [u8; 48],
    pub mrenclave: [u8; 32],
    pub is_simulated: bool,
    /// 原始 TDX Quote 字节 (硬件模式)
    pub tdx_quote_raw: Option<Vec<u8>>,
    /// 链上 nonce (嵌入 report_data[32..64])
    pub nonce: Option<[u8; 32]>,
    /// PCK 证书 DER (Level 4, 从 Quote Certification Data 提取)
    pub pck_cert_der: Option<Vec<u8>>,
    /// Intermediate CA 证书 DER (Level 4, 从 Quote Certification Data 提取)
    pub intermediate_cert_der: Option<Vec<u8>>,
}
