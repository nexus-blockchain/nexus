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
    pub warn_action: u8,
    pub welcome_enabled: bool,
    pub version: u32,
}

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

/// 证明包
#[derive(Debug, Clone)]
pub struct AttestationBundle {
    pub tdx_quote_hash: [u8; 32],
    pub sgx_quote_hash: [u8; 32],
    pub mrtd: [u8; 48],
    pub mrenclave: [u8; 32],
    pub is_simulated: bool,
    /// 原始 TDX Quote 字节 (硬件模式, 用于 submit_verified_attestation)
    pub tdx_quote_raw: Option<Vec<u8>>,
    /// 链上 nonce (嵌入 report_data[32..64])
    pub nonce: Option<[u8; 32]>,
}
