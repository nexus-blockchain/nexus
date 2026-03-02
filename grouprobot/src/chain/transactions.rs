use std::sync::Arc;
use std::time::Duration;
use subxt::dynamic::Value;
use tokio::sync::mpsc;
use tracing::{info, warn, debug};

use crate::chain::client::ChainClient;
use crate::chain::types::{AttestationBundle, PendingActionLog};
use crate::error::{BotError, BotResult};

impl ChainClient {
    /// 注册 Bot 到链上
    pub async fn register_bot(
        &self,
        bot_id_hash: [u8; 32],
        public_key: [u8; 32],
    ) -> BotResult<()> {
        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "register_bot",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(public_key),
            ],
        );

        self.submit_and_watch(tx, "register_bot").await
    }

    /// 提交 TEE 双证明 (⚠️ 仅软件模式, quote_verified=false)
    pub async fn submit_attestation(
        &self,
        bot_id_hash: [u8; 32],
        bundle: &AttestationBundle,
    ) -> BotResult<()> {
        // C2 fix: 链上期望 Option<[u8; 32]>, 需用 unnamed_variant 包装
        let sgx_quote_hash_val = if bundle.sgx_quote_hash == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.sgx_quote_hash)])
        };
        let mrenclave_val = if bundle.mrenclave == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.mrenclave)])
        };

        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "submit_attestation",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(bundle.tdx_quote_hash),
                sgx_quote_hash_val,
                Value::from_bytes(bundle.mrtd),
                mrenclave_val,
            ],
        );

        self.submit_and_watch(tx, "submit_attestation").await
    }

    /// 提交经过 Quote 结构验证的 TEE 证明 (硬件模式, quote_verified=true)
    ///
    /// 流程: request_attestation_nonce → 生成带 nonce 的 Quote → submit_verified_attestation
    /// MRTD 由链上从 raw quote 解析, 不可伪造
    pub async fn submit_verified_attestation(
        &self,
        bot_id_hash: [u8; 32],
        bundle: &AttestationBundle,
    ) -> BotResult<()> {
        let tdx_quote_raw = bundle.tdx_quote_raw.as_ref().ok_or_else(|| {
            BotError::AttestationFailed("tdx_quote_raw is required for verified attestation".into())
        })?;

        let sgx_quote_hash_val = if bundle.sgx_quote_hash == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.sgx_quote_hash)])
        };

        let mrenclave_val = if bundle.mrenclave == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.mrenclave)])
        };

        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "submit_verified_attestation",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(tdx_quote_raw),
                sgx_quote_hash_val,
                mrenclave_val,
            ],
        );

        self.submit_and_watch(tx, "submit_verified_attestation").await
    }

    /// 提交 DCAP Level 4 全证书链证明 (硬件模式, 最高安全级别)
    ///
    /// 链上验证: Intel Root CA → Intermediate CA → PCK → QE Report → AK → Body
    /// 4 层 ECDSA P-256 签名全部验证, 不依赖治理注册
    pub async fn submit_dcap_full_attestation(
        &self,
        bot_id_hash: [u8; 32],
        bundle: &AttestationBundle,
    ) -> BotResult<()> {
        let tdx_quote_raw = bundle.tdx_quote_raw.as_ref().ok_or_else(|| {
            BotError::AttestationFailed("tdx_quote_raw required for Level 4".into())
        })?;
        let pck_cert_der = bundle.pck_cert_der.as_ref().ok_or_else(|| {
            BotError::AttestationFailed("pck_cert_der required for Level 4".into())
        })?;
        let intermediate_cert_der = bundle.intermediate_cert_der.as_ref().ok_or_else(|| {
            BotError::AttestationFailed("intermediate_cert_der required for Level 4".into())
        })?;

        let mrenclave_val = if bundle.mrenclave == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.mrenclave)])
        };

        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "submit_dcap_full_attestation",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(tdx_quote_raw),
                Value::from_bytes(pck_cert_der),
                Value::from_bytes(intermediate_cert_der),
                mrenclave_val,
            ],
        );

        self.submit_and_watch(tx, "submit_dcap_full_attestation").await
    }

    /// 提交 TEE 证明 (三模式统一入口, 对应链上 call_index 21)
    ///
    /// 自动检测 Quote 类型 (SGX v3 / TDX v4), 链上验证 DCAP + 白名单
    /// 支持 Level 2/3/4 自适应
    pub async fn submit_tee_attestation(
        &self,
        bot_id_hash: [u8; 32],
        bundle: &AttestationBundle,
    ) -> BotResult<()> {
        let quote_raw = bundle.tdx_quote_raw.as_ref().ok_or_else(|| {
            BotError::AttestationFailed("quote_raw is required for submit_tee_attestation".into())
        })?;

        // platform_id: None (目前不支持 Level 3 via platform_id 在统一入口)
        let platform_id_val = Value::unnamed_variant("None", vec![]);

        // pck_cert_der: Option<Vec<u8>>
        let pck_val = match &bundle.pck_cert_der {
            Some(der) => Value::unnamed_variant("Some", vec![Value::from_bytes(der)]),
            None => Value::unnamed_variant("None", vec![]),
        };

        // intermediate_cert_der: Option<Vec<u8>>
        let inter_val = match &bundle.intermediate_cert_der {
            Some(der) => Value::unnamed_variant("Some", vec![Value::from_bytes(der)]),
            None => Value::unnamed_variant("None", vec![]),
        };

        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "submit_tee_attestation",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(quote_raw),
                platform_id_val,
                pck_val,
                inter_val,
            ],
        );

        self.submit_and_watch(tx, "submit_tee_attestation").await
    }

    /// 请求证明 Nonce (防重放, 硬件模式专用)
    ///
    /// 返回的 nonce 必须嵌入 TDX report_data[32..64]
    pub async fn request_attestation_nonce(
        &self,
        bot_id_hash: [u8; 32],
    ) -> BotResult<()> {
        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "request_attestation_nonce",
            vec![Value::from_bytes(bot_id_hash)],
        );

        self.submit_and_watch(tx, "request_attestation_nonce").await
    }

    /// 刷新 TEE 证明 (24h 周期, ⚠️ 仅软件模式)
    pub async fn refresh_attestation(
        &self,
        bot_id_hash: [u8; 32],
        bundle: &AttestationBundle,
    ) -> BotResult<()> {
        // C2 fix: 链上期望 Option<[u8; 32]>, 需用 unnamed_variant 包装
        let sgx_quote_hash_val = if bundle.sgx_quote_hash == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.sgx_quote_hash)])
        };
        let mrenclave_val = if bundle.mrenclave == [0u8; 32] {
            Value::unnamed_variant("None", vec![])
        } else {
            Value::unnamed_variant("Some", vec![Value::from_bytes(bundle.mrenclave)])
        };

        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "refresh_attestation",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(bundle.tdx_quote_hash),
                sgx_quote_hash_val,
                Value::from_bytes(bundle.mrtd),
                mrenclave_val,
            ],
        );

        self.submit_and_watch(tx, "refresh_attestation").await
    }

    // ========================================================================
    // Ad System Transactions (广告系统)
    // ========================================================================

    /// 上报广告投放收据
    pub async fn submit_delivery_receipt(
        &self,
        campaign_id: u64,
        community_id_hash: [u8; 32],
        delivery_type: u8,
        audience_size: u32,
        node_id: u32,
        node_signature: [u8; 64],
    ) -> BotResult<()> {
        let delivery_type_val = match delivery_type {
            0 => Value::unnamed_variant("ScheduledPost", vec![]),
            1 => Value::unnamed_variant("ReplyFooter", vec![]),
            2 => Value::unnamed_variant("WelcomeEmbed", vec![]),
            _ => {
                warn!(delivery_type, "未知广告投放类型，回退为 ScheduledPost");
                Value::unnamed_variant("ScheduledPost", vec![])
            }
        };

        let tx = subxt::dynamic::tx(
            "GroupRobotAds", "submit_delivery_receipt",
            vec![
                Value::u128(campaign_id as u128),
                Value::from_bytes(community_id_hash),
                delivery_type_val,
                Value::u128(audience_size as u128),
                Value::u128(node_id as u128),
                Value::from_bytes(node_signature),
            ],
        );

        self.submit_and_watch(tx, "submit_delivery_receipt").await
    }

    /// 更新社区活跃成员数 (由 Bot 定期上报)
    pub async fn update_active_members(
        &self,
        community_id_hash: [u8; 32],
        active_members: u32,
    ) -> BotResult<()> {
        let tx = subxt::dynamic::tx(
            "GroupRobotCommunity", "update_active_members",
            vec![
                Value::from_bytes(community_id_hash),
                Value::u128(active_members as u128),
            ],
        );

        self.submit_and_watch(tx, "update_active_members").await
    }

    /// 标记序列号已处理 (去重)
    pub async fn mark_sequence_processed(
        &self,
        bot_id_hash: [u8; 32],
        sequence: u64,
    ) -> BotResult<bool> {
        // 先查链上是否已处理
        if self.is_sequence_processed(&bot_id_hash, sequence).await? {
            debug!(sequence, "序列号已被其他实例处理，跳过");
            return Ok(false);
        }

        let tx = subxt::dynamic::tx(
            "GroupRobotConsensus", "mark_sequence_processed",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::u128(sequence as u128),
            ],
        );

        match self.submit_and_watch(tx, "mark_sequence_processed").await {
            Ok(()) => {
                info!(sequence, "序列号已标记处理");
                Ok(true)
            }
            Err(_) => {
                warn!(sequence, "序列号标记失败（可能已被处理）");
                Ok(false)
            }
        }
    }

    /// 提交单条动作日志
    pub async fn submit_action_log(&self, log: &PendingActionLog) -> BotResult<()> {
        let action_type_value = encode_action_type(log.action_type);

        let tx = subxt::dynamic::tx(
            "GroupRobotCommunity", "submit_action_log",
            vec![
                Value::from_bytes(log.community_id_hash),
                action_type_value,
                Value::from_bytes(log.target_hash),
                Value::u128(log.sequence as u128),
                Value::from_bytes(log.message_hash),
                Value::from_bytes(log.signature),
            ],
        );

        self.submit_and_watch(tx, "submit_action_log").await
    }

    /// 批量提交动作日志
    pub async fn batch_submit_logs(
        &self,
        community_id_hash: [u8; 32],
        logs: &[PendingActionLog],
    ) -> BotResult<()> {
        if logs.is_empty() {
            return Ok(());
        }

        let log_values: Vec<Value> = logs.iter().map(|log| {
            Value::unnamed_composite(vec![
                encode_action_type(log.action_type),
                Value::from_bytes(log.target_hash),
                Value::u128(log.sequence as u128),
                Value::from_bytes(log.message_hash),
                Value::from_bytes(log.signature),
            ])
        }).collect();

        let tx = subxt::dynamic::tx(
            "GroupRobotCommunity", "batch_submit_logs",
            vec![
                Value::from_bytes(community_id_hash),
                Value::unnamed_composite(log_values),
            ],
        );

        self.submit_and_watch(tx, "batch_submit_logs").await
    }

    /// 注册 Peer 端点到链上 (节点启动时调用)
    pub async fn register_peer(
        &self,
        bot_id_hash: [u8; 32],
        peer_public_key: [u8; 32],
        endpoint: &str,
    ) -> BotResult<()> {
        let endpoint_bytes: Vec<u8> = endpoint.as_bytes().to_vec();
        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "register_peer",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(peer_public_key),
                Value::from_bytes(endpoint_bytes),
            ],
        );

        self.submit_and_watch(tx, "register_peer").await
    }

    /// 注销 Peer 端点 (节点下线时调用)
    pub async fn deregister_peer(
        &self,
        bot_id_hash: [u8; 32],
        peer_public_key: [u8; 32],
    ) -> BotResult<()> {
        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "deregister_peer",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(peer_public_key),
            ],
        );

        self.submit_and_watch(tx, "deregister_peer").await
    }

    /// Peer 心跳 (定期调用, 更新 last_seen)
    pub async fn heartbeat_peer(
        &self,
        bot_id_hash: [u8; 32],
        peer_public_key: [u8; 32],
    ) -> BotResult<()> {
        let tx = subxt::dynamic::tx(
            "GroupRobotRegistry", "heartbeat_peer",
            vec![
                Value::from_bytes(bot_id_hash),
                Value::from_bytes(peer_public_key),
            ],
        );

        self.submit_and_watch(tx, "heartbeat_peer").await
    }

    /// 记录仪式到链上
    pub async fn record_ceremony(
        &self,
        ceremony_hash: [u8; 32],
        mrenclave: [u8; 32],
        k: u8,
        n: u8,
        bot_public_key: [u8; 32],
        participant_enclaves: Vec<[u8; 32]>,
        bot_id_hash: [u8; 32],
    ) -> BotResult<()> {
        let participants: Vec<Value> = participant_enclaves.iter()
            .map(Value::from_bytes)
            .collect();

        let tx = subxt::dynamic::tx(
            "GroupRobotCeremony", "record_ceremony",
            vec![
                Value::from_bytes(ceremony_hash),
                Value::from_bytes(mrenclave),
                Value::u128(k as u128),
                Value::u128(n as u128),
                Value::from_bytes(bot_public_key),
                Value::unnamed_composite(participants),
                Value::from_bytes(bot_id_hash),
            ],
        );

        self.submit_and_watch(tx, "record_ceremony").await
    }

    /// 通用交易提交 + 等待 finalized (M4 fix: 含指数退避重试)
    async fn submit_and_watch(
        &self,
        tx: subxt::tx::DynamicPayload,
        label: &str,
    ) -> BotResult<()> {
        const MAX_RETRIES: u32 = 3;
        let mut last_err = String::new();

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << attempt); // 2s, 4s, 8s
                warn!(call = label, attempt, delay_secs = ?delay, "重试交易提交...");
                tokio::time::sleep(delay).await;
            }

            match self.api().tx()
                .sign_and_submit_then_watch_default(&tx, self.signer())
                .await
            {
                Ok(progress) => {
                    match progress.wait_for_finalized_success().await {
                        Ok(_events) => {
                            info!(call = label, "交易已上链");
                            return Ok(());
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            // Dispatch 错误不可重试 (如 BotNotFound, NotBotOwner 等)
                            if err_str.contains("Module") || err_str.contains("dispatch") {
                                warn!(call = label, error = %e, "交易 dispatch 错误 (不可重试)");
                                return Err(BotError::TransactionFailed(format!("{}: {}", label, e)));
                            }
                            warn!(call = label, error = %e, attempt, "交易上链等待失败");
                            last_err = format!("{}: {}", label, e);
                        }
                    }
                }
                Err(e) => {
                    warn!(call = label, error = %e, attempt, "交易提交失败");
                    last_err = format!("{}: {}", label, e);
                }
            }
        }

        Err(BotError::TransactionFailed(format!("{} ({}次重试后)", last_err, MAX_RETRIES)))
    }
}

/// C3 fix: 将 u8 动作类型编码为链上 ActionType SCALE 枚举变体
///
/// 链上定义: enum ActionType { Kick, Ban, Mute, Warn, Unmute, Unban, Promote, Demote, Welcome, ConfigUpdate(ConfigUpdateAction) }
fn encode_action_type(action_type: u8) -> Value {
    match action_type {
        0 => Value::unnamed_variant("Kick", vec![]),
        1 => Value::unnamed_variant("Ban", vec![]),
        2 => Value::unnamed_variant("Mute", vec![]),
        3 => Value::unnamed_variant("Warn", vec![]),
        4 => Value::unnamed_variant("Unmute", vec![]),
        5 => Value::unnamed_variant("Unban", vec![]),
        6 => Value::unnamed_variant("Promote", vec![]),
        7 => Value::unnamed_variant("Demote", vec![]),
        8 => Value::unnamed_variant("Welcome", vec![]),
        // ConfigUpdate(ConfigUpdateAction) 需要内嵌子枚举, 链下暂不支持
        _ => {
            tracing::warn!(action_type, "未知 ActionType, 回退为 Kick");
            Value::unnamed_variant("Kick", vec![])
        }
    }
}

/// 动作日志批量提交器
pub struct ActionLogBatcher {
    receiver: mpsc::Receiver<PendingActionLog>,
    chain: Arc<ChainClient>,
    batch_interval: Duration,
    max_batch_size: usize,
}

impl ActionLogBatcher {
    pub fn new(
        receiver: mpsc::Receiver<PendingActionLog>,
        chain: Arc<ChainClient>,
        batch_interval_secs: u64,
        max_batch_size: usize,
    ) -> Self {
        Self {
            receiver,
            chain,
            batch_interval: Duration::from_secs(batch_interval_secs),
            max_batch_size,
        }
    }

    /// 运行批量提交循环
    pub async fn run(mut self) {
        let mut interval = tokio::time::interval(self.batch_interval);
        let mut buffer: Vec<PendingActionLog> = Vec::new();

        loop {
            tokio::select! {
                Some(log) = self.receiver.recv() => {
                    buffer.push(log);
                    if buffer.len() >= self.max_batch_size {
                        self.flush(&mut buffer).await;
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        self.flush(&mut buffer).await;
                    }
                }
            }
        }
    }

    async fn flush(&self, buffer: &mut Vec<PendingActionLog>) {
        // 按 community_id_hash 分组
        let mut groups: std::collections::HashMap<[u8; 32], Vec<PendingActionLog>> =
            std::collections::HashMap::new();

        for log in buffer.drain(..) {
            groups.entry(log.community_id_hash).or_default().push(log);
        }

        for (community_id_hash, logs) in groups {
            if logs.len() == 1 {
                if let Err(e) = self.chain.submit_action_log(&logs[0]).await {
                    warn!(error = %e, "单条日志提交失败");
                }
            } else {
                match self.chain.batch_submit_logs(community_id_hash, &logs).await {
                    Ok(()) => info!(count = logs.len(), "批量日志提交成功"),
                    Err(e) => {
                        warn!(error = %e, count = logs.len(), "批量日志提交失败，尝试逐条");
                        for log in &logs {
                            let _ = self.chain.submit_action_log(log).await;
                        }
                    }
                }
            }
        }
    }
}
