use subxt::dynamic::Value;
use subxt::ext::scale_value::At;
use tracing::{debug, warn};

use crate::chain::client::ChainClient;
use crate::chain::types::{BotInfoCache, TeeNodeStatus, ChainCommunityConfig, AdScheduleInfo, AdCampaignInfo, PeerInfo};
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

                // H3 fix: 链上字段是 status: BotStatus 枚举, 不是 is_active: bool
                let is_active = decoded.at("status")
                    .map(|v| {
                        // BotStatus 枚举: Active / Deactivated / Suspended
                        // subxt 动态查询返回变体名称字符串
                        use subxt::ext::scale_value::ValueDef;
                        match &v.value {
                            ValueDef::Variant(variant) => variant.name == "Active",
                            _ => v.as_u128().map(|n| n == 0).unwrap_or(false), // fallback: index 0 = Active
                        }
                    })
                    .unwrap_or(false);

                // H2 fix: NodeType 是枚举 (StandardNode / TeeNode / TeeNodeV2), 用变体名称匹配
                let is_tee_node = decoded.at("node_type")
                    .map(|v| {
                        use subxt::ext::scale_value::ValueDef;
                        match &v.value {
                            ValueDef::Variant(variant) => {
                                variant.name == "TeeNode" || variant.name == "TeeNodeV2"
                            }
                            _ => v.as_u128().map(|n| n >= 1).unwrap_or(false), // fallback
                        }
                    })
                    .unwrap_or(false);

                let public_key = extract_bytes_32(&decoded, "public_key")
                    .unwrap_or([0u8; 32]);

                let owner = decoded.at("owner")
                    .and_then(|v| {
                        // AccountId32 存储为 Composite 或 bytes
                        v.as_str().map(|s| s.to_string())
                    })
                    .unwrap_or_default();

                if !is_active {
                    warn!(bot = hex::encode(bot_id_hash), "链上 Bot 状态: is_active=false");
                }

                Ok(Some(BotInfoCache {
                    bot_id_hash: hex::encode(bot_id_hash),
                    owner,
                    public_key,
                    is_active,
                    is_tee_node,
                }))
            }
            None => Ok(None),
        }
    }

    /// 查询 TEE 节点状态
    ///
    /// H1-fix: 优先查询 AttestationsV2 (submit_tee_attestation 写入), 未找到则回退 Attestations
    pub async fn query_tee_status(&self, bot_id_hash: &[u8; 32]) -> BotResult<Option<TeeNodeStatus>> {
        // 先查 AttestationsV2
        let query_v2 = subxt::dynamic::storage(
            "GroupRobotRegistry", "AttestationsV2",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let storage = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?;

        let result = storage.fetch(&query_v2).await
            .map_err(|e| BotError::QueryFailed(format!("fetch attestation v2: {}", e)))?;

        // 未找到 V2 则回退 V1
        let result = match result {
            Some(v) => Some(v),
            None => {
                let query_v1 = subxt::dynamic::storage(
                    "GroupRobotRegistry", "Attestations",
                    vec![Value::from_bytes(bot_id_hash)],
                );
                storage.fetch(&query_v1).await
                    .map_err(|e| BotError::QueryFailed(format!("fetch attestation v1: {}", e)))?
            }
        };

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode attestation: {}", e)))?;
                debug!(raw = ?decoded, "TEE 状态原始数据");

                let expires_at = decoded.at("expires_at")
                    .and_then(|v| v.as_u128())
                    .map(|n| n as u64);

                // 获取当前区块号判断是否过期
                let current_block = self.api().blocks().at_latest().await
                    .map(|b| b.number() as u64)
                    .unwrap_or(0);

                let is_expired = expires_at
                    .map(|exp| current_block > exp)
                    .unwrap_or(false);

                debug!(
                    bot = hex::encode(bot_id_hash),
                    ?expires_at, current_block, is_expired,
                    "TEE 证明状态"
                );

                Ok(Some(TeeNodeStatus {
                    is_attested: true,
                    is_expired,
                    expires_at,
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

                let node_requirement = decoded.at("node_requirement")
                    .and_then(|v| {
                        use subxt::ext::scale_value::ValueDef;
                        match &v.value {
                            ValueDef::Variant(variant) => match variant.name.as_str() {
                                "Any" => Some(0u8),
                                "TeeOnly" => Some(1),
                                "TeePreferred" => Some(2),
                                "MinTee" => Some(3),
                                _ => Some(1), // default TeeOnly
                            },
                            _ => v.as_u128().map(|n| n as u8),
                        }
                    })
                    .unwrap_or(1); // default TeeOnly
                let anti_flood_enabled = decoded.at("anti_flood_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let flood_limit = decoded.at("flood_limit")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(10) as u16;
                let warn_limit = decoded.at("warn_limit")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(3) as u8;
                let warn_action = decoded.at("warn_action")
                    .and_then(|v| {
                        use subxt::ext::scale_value::ValueDef;
                        match &v.value {
                            ValueDef::Variant(variant) => match variant.name.as_str() {
                                "Kick" => Some(0u8),
                                "Ban" => Some(1),
                                "Mute" => Some(2),
                                _ => Some(0), // default Kick
                            },
                            _ => v.as_u128().map(|n| n as u8),
                        }
                    })
                    .unwrap_or(0); // default Kick (matches on-chain default)
                let welcome_enabled = decoded.at("welcome_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let version = decoded.at("version")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0) as u32;

                // Phase 1 新增字段 (链上尚未部署时使用默认值)
                let anti_duplicate_enabled = decoded.at("anti_duplicate_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let duplicate_window_secs = decoded.at("duplicate_window_secs")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(300) as u64;
                let duplicate_threshold = decoded.at("duplicate_threshold")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(3) as u16;
                let max_emoji = decoded.at("max_emoji")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0) as u16;
                let max_links = decoded.at("max_links")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0) as u16;
                let stop_words = decoded.at("stop_words")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let welcome_template = decoded.at("welcome_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let goodbye_template = decoded.at("goodbye_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let warn_mute_duration = decoded.at("warn_mute_duration")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(3600) as u64;

                // Phase 2 新增字段
                let spam_samples = decoded.at("spam_samples")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let similarity_threshold = decoded.at("similarity_threshold")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(70) as u8;
                let log_channel_id = decoded.at("log_channel_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let captcha_enabled = decoded.at("captcha_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let captcha_timeout_secs = decoded.at("captcha_timeout_secs")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(120) as u64;

                // Phase 3 新增字段
                let antiphishing_enabled = decoded.at("antiphishing_enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let bayes_threshold = decoded.at("bayes_threshold")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(80) as u8;
                let custom_commands_csv = decoded.at("custom_commands_csv")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Phase 4 新增字段
                let locked_types_csv = decoded.at("locked_types_csv")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Phase 4b: 订阅层级 + 功能门控
                let subscription_tier = decoded.at("subscription_tier")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0) as u8;
                let max_rules = decoded.at("max_rules")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(3) as u16;
                let forced_ads_per_day = decoded.at("forced_ads_per_day")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0) as u8;
                let can_disable_ads = decoded.at("can_disable_ads")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let community_id_hash_str = decoded.at("community_id_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                Ok(Some(ChainCommunityConfig {
                    node_requirement,
                    anti_flood_enabled,
                    flood_limit,
                    warn_limit,
                    warn_action,
                    welcome_enabled,
                    version,
                    anti_duplicate_enabled,
                    duplicate_window_secs,
                    duplicate_threshold,
                    max_emoji,
                    max_links,
                    stop_words,
                    welcome_template,
                    goodbye_template,
                    warn_mute_duration,
                    spam_samples,
                    similarity_threshold,
                    log_channel_id,
                    captcha_enabled,
                    captcha_timeout_secs,
                    antiphishing_enabled,
                    bayes_threshold,
                    custom_commands_csv,
                    locked_types_csv,
                    subscription_tier,
                    max_rules,
                    forced_ads_per_day,
                    can_disable_ads,
                    community_id_hash: community_id_hash_str,
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
                // L2 修复: SCALE 位置提取 — 存储格式 (nonce: [u8; 32], issued_at: u32)
                // nonce 作为定长 [u8; 32] 在 SCALE 编码中占据前 32 字节
                // 注意: 若链上存储元组字段重排 (如 issued_at 移到前面), 此处需同步更新
                let bytes = val.encoded();
                // 期望最小长度: 32 (nonce) + 4 (issued_at u32) = 36
                if bytes.len() >= 36 {
                    let mut nonce = [0u8; 32];
                    nonce.copy_from_slice(&bytes[..32]);
                    debug!(nonce = %hex::encode(&nonce[..8]), raw_len = bytes.len(), "链上 Nonce 已读取");
                    return Ok(Some(nonce));
                }
                warn!(len = bytes.len(), expected = 36, "Nonce SCALE 字节长度异常, 可能存储布局已变更");
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

        // H1-fix: 优先查询 AttestationsV2, 未找到则回退 Attestations
        let query_v2 = subxt::dynamic::storage(
            "GroupRobotRegistry", "AttestationsV2",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let storage = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?;

        let result = storage.fetch(&query_v2).await
            .map_err(|e| BotError::QueryFailed(format!("fetch attestation v2: {}", e)))?;

        let result = match result {
            Some(v) => Some(v),
            None => {
                let query_v1 = subxt::dynamic::storage(
                    "GroupRobotRegistry", "Attestations",
                    vec![Value::from_bytes(bot_id_hash)],
                );
                storage.fetch(&query_v1).await
                    .map_err(|e| BotError::QueryFailed(format!("fetch attestation v1: {}", e)))?
            }
        };

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

    /// 查询 Bot 的 Peer 注册表 (用于节点发现和 share recovery)
    ///
    /// 返回链上 PeerRegistry 中注册的所有 Peer 端点信息
    pub async fn query_peer_registry(&self, bot_id_hash: &[u8; 32]) -> BotResult<Vec<PeerInfo>> {
        let query = subxt::dynamic::storage(
            "GroupRobotRegistry", "PeerRegistry",
            vec![Value::from_bytes(bot_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch peer registry: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode peer registry: {}", e)))?;

                use subxt::ext::scale_value::ValueDef;
                let peers = match &decoded.value {
                    ValueDef::Composite(composite) => {
                        composite.values().filter_map(|item| {
                            let public_key = extract_bytes_32(item, "public_key")?;

                            let endpoint = item.at("endpoint")
                                .and_then(|v| match &v.value {
                                    ValueDef::Composite(c) => {
                                        let bytes: Vec<u8> = c.values()
                                            .filter_map(|b| b.as_u128().map(|n| n as u8))
                                            .collect();
                                        String::from_utf8(bytes).ok()
                                    }
                                    _ => v.as_str().map(|s| s.to_string()),
                                })
                                .unwrap_or_default();

                            let registered_at = item.at("registered_at")
                                .and_then(|v| v.as_u128())
                                .unwrap_or(0) as u64;

                            let last_seen = item.at("last_seen")
                                .and_then(|v| v.as_u128())
                                .unwrap_or(0) as u64;

                            Some(PeerInfo { public_key, endpoint, registered_at, last_seen })
                        }).collect()
                    }
                    _ => Vec::new(),
                };

                debug!(
                    bot = hex::encode(bot_id_hash),
                    peer_count = peers.len(),
                    "链上 Peer 注册表已读取"
                );
                Ok(peers)
            }
            None => Ok(Vec::new()),
        }
    }

    /// 查询活跃仪式
    #[allow(dead_code)]
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

    // ========================================================================
    // Ad System Queries (广告系统)
    // ========================================================================

    /// 查询社区广告排期
    pub async fn query_ad_schedule(
        &self,
        community_id_hash: &[u8; 32],
    ) -> BotResult<Option<AdScheduleInfo>> {
        let query = subxt::dynamic::storage(
            "GroupRobotAds", "CommunitySchedules",
            vec![Value::from_bytes(community_id_hash)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch schedule: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode schedule: {}", e)))?;

                let daily_limit = decoded.at("daily_limit")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(2) as u8;

                let delivered_this_era = decoded.at("delivered_this_era")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0) as u32;

                // Extract campaign IDs from BoundedVec
                let campaign_ids = decoded.at("scheduled_campaigns")
                    .and_then(|v| {
                        use subxt::ext::scale_value::ValueDef;
                        match &v.value {
                            ValueDef::Composite(composite) => {
                                let ids: Vec<u64> = composite.values()
                                    .filter_map(|item| item.as_u128().map(|n| n as u64))
                                    .collect();
                                Some(ids)
                            }
                            _ => None,
                        }
                    })
                    .unwrap_or_default();

                Ok(Some(AdScheduleInfo {
                    community_id_hash: *community_id_hash,
                    campaign_ids,
                    daily_limit,
                    delivered_this_era,
                }))
            }
            None => Ok(None),
        }
    }

    /// 查询广告活动信息
    pub async fn query_campaign(
        &self,
        campaign_id: u64,
    ) -> BotResult<Option<AdCampaignInfo>> {
        let query = subxt::dynamic::storage(
            "GroupRobotAds", "Campaigns",
            vec![Value::u128(campaign_id as u128)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch campaign: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode campaign: {}", e)))?;

                // Extract text from BoundedVec<u8>
                let text = Self::extract_bounded_string(&decoded, "text");
                let url = Self::extract_bounded_string(&decoded, "url");

                let bid_per_mille = decoded.at("bid_per_mille")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(0);

                let delivery_types = decoded.at("delivery_types")
                    .and_then(|v| v.as_u128())
                    .unwrap_or(1) as u8;

                // H7-fix: status/review_status 是 SCALE 枚举, 需匹配 variant 名称
                let is_active = decoded.at("status")
                    .map(|v| {
                        use subxt::ext::scale_value::ValueDef;
                        matches!(&v.value, ValueDef::Variant(var) if var.name == "Active")
                    })
                    .unwrap_or(false);

                let is_approved = decoded.at("review_status")
                    .map(|v| {
                        use subxt::ext::scale_value::ValueDef;
                        matches!(&v.value, ValueDef::Variant(var) if var.name == "Approved")
                    })
                    .unwrap_or(false);

                let advertiser = decoded.at("advertiser")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();

                Ok(Some(AdCampaignInfo {
                    campaign_id,
                    advertiser,
                    text,
                    url,
                    bid_per_mille,
                    delivery_types,
                    is_active,
                    is_approved,
                }))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // Reward System Queries (L1-fix: 链下奖励查询)
    // ========================================================================

    /// 查询节点待领取奖励
    pub async fn query_pending_rewards(&self, node_id: &[u8; 32]) -> BotResult<u128> {
        let query = subxt::dynamic::storage(
            "GroupRobotRewards", "NodePendingRewards",
            vec![Value::from_bytes(node_id)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch pending rewards: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode pending rewards: {}", e)))?;
                Ok(decoded.as_u128().unwrap_or(0))
            }
            None => Ok(0),
        }
    }

    /// 查询节点累计已领取奖励
    pub async fn query_total_earned(&self, node_id: &[u8; 32]) -> BotResult<u128> {
        let query = subxt::dynamic::storage(
            "GroupRobotRewards", "NodeTotalEarned",
            vec![Value::from_bytes(node_id)],
        );
        let result = self.api().storage().at_latest().await
            .map_err(|e| BotError::QueryFailed(format!("storage access: {}", e)))?
            .fetch(&query).await
            .map_err(|e| BotError::QueryFailed(format!("fetch total earned: {}", e)))?;

        match result {
            Some(val) => {
                let decoded = val.to_value()
                    .map_err(|e| BotError::QueryFailed(format!("decode total earned: {}", e)))?;
                Ok(decoded.as_u128().unwrap_or(0))
            }
            None => Ok(0),
        }
    }

    /// 从 BoundedVec<u8> 提取字符串
    fn extract_bounded_string<T>(
        parent: &subxt::ext::scale_value::Value<T>,
        field: &str,
    ) -> String {
        parent.at(field)
            .and_then(|v| {
                use subxt::ext::scale_value::ValueDef;
                match &v.value {
                    ValueDef::Composite(composite) => {
                        let bytes: Vec<u8> = composite.values()
                            .filter_map(|item| item.as_u128().map(|n| n as u8))
                            .collect();
                        String::from_utf8(bytes).ok()
                    }
                    _ => v.as_str().map(|s| s.to_string()),
                }
            })
            .unwrap_or_default()
    }
}

/// 从 scale_value 动态值中提取 32 字节数组
///
/// AccountId32 / [u8; 32] 在 subxt 动态查询中以 Composite::Unnamed([u128; 32]) 存储
fn extract_bytes_32<T>(
    parent: &subxt::ext::scale_value::Value<T>,
    field: &str,
) -> Option<[u8; 32]> {
    use subxt::ext::scale_value::ValueDef;

    let val = parent.at(field)?;
    match &val.value {
        ValueDef::Composite(composite) => {
            let bytes: Vec<u8> = composite
                .values()
                .filter_map(|v| v.as_u128().map(|n| n as u8))
                .collect();
            if bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                return Some(arr);
            }
            None
        }
        _ => None,
    }
}
