use subxt::dynamic::Value;
use subxt::ext::scale_value::At;
use tracing::debug;

use crate::chain::client::ChainClient;
use crate::chain::types::{BotInfoCache, TeeNodeStatus, ChainCommunityConfig};
use crate::error::{BotError, BotResult};

impl ChainClient {
    /// 从链上读取 Bot 注册信息
    pub async fn fetch_bot(&self, bot_id_hash: &[u8; 32]) -> BotResult<Option<BotInfoCache>> {
        let query = subxt::dynamic::storage(
            "GroupRobotRegistry", "Bots",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch bot: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode bot: {}", e)))?;
                debug!(raw = ?decoded, "Bot 原始数据");
                // TODO: 完整 SCALE 解码
                Ok(Some(BotInfoCache {
                    bot_id_hash: hex::encode(bot_id_hash),
                    owner: String::new(),
                    public_key: [0u8; 32],
                    is_active: true,
                    is_tee_node: false,
                }))
            }
            None => Ok(None),
        }
    }

    /// 查询 TEE 节点状态
    pub async fn query_tee_status(&self, bot_id_hash: &[u8; 32]) -> BotResult<Option<TeeNodeStatus>> {
        let query = subxt::dynamic::storage(
            "GroupRobotRegistry", "Attestations",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch attestation: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode attestation: {}", e)))?;
                debug!(raw = ?decoded, "TEE 状态原始数据");
                Ok(Some(TeeNodeStatus {
                    is_attested: true,
                    is_expired: false,
                    expires_at: None,
                }))
            }
            None => Ok(None),
        }
    }

    /// 检查序列号是否已被处理 (去重)
    pub async fn is_sequence_processed(
        &self,
        bot_id_hash: &[u8; 32],
        sequence: u64,
    ) -> BotResult<bool> {
        let query = subxt::dynamic::storage(
            "GroupRobotConsensus", "ProcessedSequences",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::u128(sequence as u128),
            ],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch sequence: {}", e)))?;
        Ok(result.is_some())
    }

    /// 查询群规则配置
    pub async fn fetch_community_config(
        &self,
        community_id_hash: &[u8; 32],
    ) -> BotResult<Option<ChainCommunityConfig>> {
        let query = subxt::dynamic::storage(
            "GroupRobotCommunity", "CommunityConfigs",
            vec![Value::from_bytes(community_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch config: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode config: {}", e)))?;
                debug!(raw = ?decoded, "社区配置原始数据");
                // TODO: 完整 SCALE 解码
                Ok(Some(ChainCommunityConfig {
                    node_requirement: 0,
                    anti_flood_enabled: true,
                    flood_limit: 10,
                    warn_limit: 3,
                    warn_action: 0,
                    welcome_enabled: false,
                    version: 0,
                }))
            }
            None => Ok(None),
        }
    }

    /// 查询 MRTD 是否在白名单
    pub async fn is_mrtd_approved(&self, mrtd: &[u8; 48]) -> BotResult<bool> {
        let query = subxt::dynamic::storage(
            "GroupRobotRegistry", "ApprovedMrtd",
            vec![Value::from_bytes(mrtd)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch mrtd: {}", e)))?;
        Ok(result.is_some())
    }

    /// 查询证明 Nonce (request_attestation_nonce 后从链上读取)
    ///
    /// 返回 nonce 32 bytes (已存储在 AttestationNonces 中)
    /// 存储格式: (nonce: [u8; 32], issued_at: BlockNumber)
    pub async fn query_attestation_nonce(&self, bot_id_hash: &[u8; 32]) -> BotResult<Option<[u8; 32]>> {
        let query = subxt::dynamic::storage(
            "GroupRobotRegistry", "AttestationNonces",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch nonce: {}", e)))?;

        match result {
            Some(val) => {
                // 使用 SCALE 解码: (nonce: [u8; 32], issued_at: u32)
                let bytes = val.encoded();
                if bytes.len() >= 32 {
                    let mut nonce = [0u8; 32];
                    nonce.copy_from_slice(&bytes[..32]);
                    debug!(nonce = %hex::encode(&nonce[..8]), "链上 Nonce 已读取");
                    return Ok(Some(nonce));
                }
                debug!(len = bytes.len(), "Nonce 原始字节长度不足");
                Ok(None)
            }
            None => Ok(None),
        }
    }

    /// 查询请求者的 TEE 证明状态 (用于 AttestationGuard)
    ///
    /// 通过 requester 公钥的 SHA256 作为 bot_id_hash 查询 Attestations 存储
    /// 返回 (is_verified_tee, quote_verified)
    pub async fn query_attestation_guard(&self, requester_pk_hex: &str) -> BotResult<(bool, bool)> {
        // requester_pk 是 32 bytes hex (64 chars) → SHA256 → bot_id_hash
        let pk_bytes = hex::decode(requester_pk_hex)
            .map_err(|e| BotError::QueryFailed(format!("invalid requester_pk hex: {}", e)))?;
        if pk_bytes.len() != 32 {
            return Ok((false, false));
        }

        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&pk_bytes);
        let bot_id_hash: [u8; 32] = hasher.finalize().into();

        // 查询 Attestations 存储
        let query = subxt::dynamic::storage(
            "GroupRobotRegistry", "Attestations",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch attestation: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode attestation: {}", e)))?;
                // 尝试提取 quote_verified 字段
                let quote_verified = decoded.at("quote_verified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                debug!(
                    requester = requester_pk_hex,
                    quote_verified,
                    "链上证明查询完成"
                );
                Ok((true, quote_verified))
            }
            None => {
                debug!(requester = requester_pk_hex, "链上无证明记录");
                Ok((false, false))
            }
        }
    }

    /// 查询活跃仪式
    pub async fn is_ceremony_active(&self, bot_public_key: &[u8; 32]) -> BotResult<bool> {
        let query = subxt::dynamic::storage(
            "GroupRobotCeremony", "ActiveCeremony",
            vec![Value::from_bytes(bot_public_key)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch ceremony: {}", e)))?;
        Ok(result.is_some())
    }
}
