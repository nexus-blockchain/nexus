// Peer Monitor — 监控可用 Peer 节点数 + 自动触发 Re-ceremony
//
// 功能:
// 1. 定期从链上 PeerRegistry 查询活跃 Peer 数
// 2. 当 peer_count <= K 时发出 CRITICAL 告警
// 3. 当 peer_count <= K+1 时发出 WARNING 告警
// 4. 跟踪 Peer 集合变化, 检测新节点加入
// 5. 新节点加入且 peer_count >= 目标 N 时, 自动触发 Re-ceremony
// 6. Leader 选举: 公钥最小的节点发起 (确定性, 无需协调)
// 7. 冷却机制: 防止短时间内重复触发

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn, error};

use crate::chain::ChainClient;
use crate::chain::types::PeerInfo;
use crate::error::BotResult;
use crate::tee::ceremony::{Participant, ReCeremonyConfig, run_re_ceremony};
use crate::tee::enclave_bridge::EnclaveBridge;

/// Peer 监控配置
pub struct PeerMonitorConfig {
    /// Bot ID Hash (链上查询 PeerRegistry)
    pub bot_id_hash: [u8; 32],
    /// Shamir 门限 K
    pub threshold_k: u8,
    /// 目标 N (期望的总节点数)
    pub desired_n: u8,
    /// 检查间隔 (秒)
    pub check_interval_secs: u64,
    /// 告警 Webhook URL (可选, 为空则仅日志告警)
    pub alert_webhook_url: Option<String>,
    /// 是否启用 Re-ceremony 自动触发
    pub auto_re_ceremony: bool,
    /// Re-ceremony 冷却时间 (秒, 默认 600 = 10 分钟)
    pub re_ceremony_cooldown_secs: u64,
}

/// Peer 监控状态
#[derive(Debug, Clone, PartialEq)]
pub enum PeerHealthStatus {
    /// 健康: peer_count > K+1
    Healthy { peer_count: u32, threshold: u8 },
    /// 警告: peer_count == K+1 (仅剩 1 个冗余)
    Warning { peer_count: u32, threshold: u8 },
    /// 危险: peer_count <= K (无冗余, 可能丢失 share)
    Critical { peer_count: u32, threshold: u8 },
}

impl std::fmt::Display for PeerHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy { peer_count, threshold } => {
                write!(f, "HEALTHY: {} peers (K={})", peer_count, threshold)
            }
            Self::Warning { peer_count, threshold } => {
                write!(f, "WARNING: only {} peers remaining (K={}), 1 more loss = critical", peer_count, threshold)
            }
            Self::Critical { peer_count, threshold } => {
                write!(f, "CRITICAL: only {} peers remaining (K={}), share recovery at risk!", peer_count, threshold)
            }
        }
    }
}

/// 评估 Peer 健康状态
pub fn evaluate_peer_health(peer_count: u32, threshold_k: u8) -> PeerHealthStatus {
    let k = threshold_k as u32;
    if peer_count <= k {
        PeerHealthStatus::Critical { peer_count, threshold: threshold_k }
    } else if peer_count <= k + 1 {
        PeerHealthStatus::Warning { peer_count, threshold: threshold_k }
    } else {
        PeerHealthStatus::Healthy { peer_count, threshold: threshold_k }
    }
}

/// Re-ceremony 自动触发条件评估结果
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerDecision {
    /// 不触发
    Skip { reason: String },
    /// 应触发 Re-ceremony
    Trigger {
        new_peers: Vec<[u8; 32]>,
        total_peers: usize,
    },
}

/// 评估是否应触发 Re-ceremony
///
/// 触发条件 (全部满足):
/// 1. auto_re_ceremony 已启用
/// 2. 检测到新的 peer (不在之前的已知集合中)
/// 3. 当前 peer 总数 >= 目标 N
/// 4. 当前 peer 总数 >= K (能恢复 secret)
/// 5. 本节点是 Leader (公钥最小)
/// 6. 冷却时间已过
pub fn evaluate_trigger(
    current_peers: &[PeerInfo],
    known_peer_pks: &HashSet<[u8; 32]>,
    my_pk: &[u8; 32],
    threshold_k: u8,
    desired_n: u8,
) -> TriggerDecision {
    let current_pks: HashSet<[u8; 32]> = current_peers.iter().map(|p| p.public_key).collect();
    let total = current_peers.len();

    // 条件 2: 有新 peer
    let new_peers: Vec<[u8; 32]> = current_pks.difference(known_peer_pks).copied().collect();
    if new_peers.is_empty() {
        return TriggerDecision::Skip {
            reason: "no new peers detected".into(),
        };
    }

    // 条件 3: peer 总数 >= 目标 N
    if total < desired_n as usize {
        return TriggerDecision::Skip {
            reason: format!("peer count ({}) < desired N ({})", total, desired_n),
        };
    }

    // 条件 4: peer 总数 >= K (能恢复 secret)
    if total < threshold_k as usize {
        return TriggerDecision::Skip {
            reason: format!("peer count ({}) < K ({}), cannot recover secret", total, threshold_k),
        };
    }

    // 条件 5: 本节点是 Leader (公钥最小)
    if !is_leader(my_pk, current_peers) {
        return TriggerDecision::Skip {
            reason: "not the leader (smallest pk)".into(),
        };
    }

    TriggerDecision::Trigger {
        new_peers,
        total_peers: total,
    }
}

/// 判断本节点是否为 Leader (公钥字典序最小)
///
/// 前置条件: 本节点必须在 peers 列表中, 否则返回 false (M1 审计修复)
pub fn is_leader(my_pk: &[u8; 32], peers: &[PeerInfo]) -> bool {
    let self_registered = peers.iter().any(|p| p.public_key == *my_pk);
    if !self_registered {
        return false;
    }
    for peer in peers {
        if peer.public_key < *my_pk {
            return false;
        }
    }
    true
}

/// Peer 监控器 (含 Re-ceremony 自动触发)
pub struct PeerMonitor {
    config: PeerMonitorConfig,
    chain: Arc<ChainClient>,
    enclave: Arc<EnclaveBridge>,
    last_status: Option<PeerHealthStatus>,
    /// 已知 peer 公钥集合 (用于检测新节点)
    known_peer_pks: HashSet<[u8; 32]>,
    /// 上次 Re-ceremony 时间 (Unix secs)
    last_re_ceremony_at: u64,
}

impl PeerMonitor {
    pub fn new(
        config: PeerMonitorConfig,
        chain: Arc<ChainClient>,
        enclave: Arc<EnclaveBridge>,
    ) -> Self {
        Self {
            config,
            chain,
            enclave,
            last_status: None,
            known_peer_pks: HashSet::new(),
            last_re_ceremony_at: 0,
        }
    }

    /// 执行单次检查 (健康评估 + Re-ceremony 触发)
    pub async fn check_once(&mut self) -> BotResult<PeerHealthStatus> {
        let peers = self.chain.query_peer_registry(&self.config.bot_id_hash).await?;
        let peer_count = peers.len() as u32;
        let status = evaluate_peer_health(peer_count, self.config.threshold_k);

        // 状态变化时输出日志 + 告警
        let status_changed = self.last_status.as_ref() != Some(&status);
        if status_changed {
            match &status {
                PeerHealthStatus::Critical { peer_count, threshold } => {
                    error!(
                        peer_count,
                        threshold_k = threshold,
                        "🚨 CRITICAL: Peer 数量 ({}) 已达到或低于 Shamir 门限 K ({})!",
                        peer_count, threshold
                    );
                    error!("🚨 如果再有节点离线, share recovery 将无法完成!");
                    self.send_alert(&status).await;
                }
                PeerHealthStatus::Warning { peer_count, threshold } => {
                    warn!(
                        peer_count,
                        threshold_k = threshold,
                        "⚠️  WARNING: Peer 数量 ({}) 接近 Shamir 门限 K ({}), 仅剩 1 个冗余",
                        peer_count, threshold
                    );
                    self.send_alert(&status).await;
                }
                PeerHealthStatus::Healthy { peer_count, threshold } => {
                    info!(
                        peer_count,
                        threshold_k = threshold,
                        "✅ Peer 状态健康: {} 个活跃节点 (K={})",
                        peer_count, threshold
                    );
                }
            }
            self.last_status = Some(status.clone());
        }

        // ── Re-ceremony 自动触发评估 ──
        if self.config.auto_re_ceremony {
            self.evaluate_and_trigger_re_ceremony(&peers).await;
        }

        // 更新已知 peer 集合
        self.known_peer_pks = peers.iter().map(|p| p.public_key).collect();

        Ok(status)
    }

    /// 评估并可能触发 Re-ceremony
    async fn evaluate_and_trigger_re_ceremony(&mut self, peers: &[PeerInfo]) {
        let my_pk = self.enclave.public_key_bytes();

        let decision = evaluate_trigger(
            peers,
            &self.known_peer_pks,
            &my_pk,
            self.config.threshold_k,
            self.config.desired_n,
        );

        match decision {
            TriggerDecision::Skip { reason } => {
                // 仅在首次初始化后有已知 peer 时 debug 输出, 避免噪音
                if !self.known_peer_pks.is_empty() {
                    tracing::debug!(reason, "Re-ceremony 触发评估: 跳过");
                }
            }
            TriggerDecision::Trigger { new_peers, total_peers } => {
                // 冷却检查
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                if now - self.last_re_ceremony_at < self.config.re_ceremony_cooldown_secs {
                    info!(
                        cooldown_remaining = self.config.re_ceremony_cooldown_secs - (now - self.last_re_ceremony_at),
                        "Re-ceremony 触发: 冷却中, 跳过"
                    );
                    return;
                }

                info!(
                    new_peers = new_peers.len(),
                    total_peers,
                    desired_n = self.config.desired_n,
                    "🔄 检测到新节点加入, 本节点为 Leader, 触发 Re-ceremony"
                );

                // 构建参与者列表
                let participants: Vec<Participant> = peers.iter().take(self.config.desired_n as usize)
                    .map(|p| Participant {
                        endpoint: p.endpoint.clone(),
                        public_key: p.public_key,
                    })
                    .collect();

                let re_config = ReCeremonyConfig {
                    current_k: self.config.threshold_k,
                    new_k: self.config.threshold_k,
                    new_n: participants.len() as u8,
                    timeout_secs: 60,
                };

                match run_re_ceremony(
                    &self.enclave,
                    &self.chain,
                    &re_config,
                    &participants,
                    &self.config.bot_id_hash,
                ).await {
                    Ok(result) => {
                        self.last_re_ceremony_at = now;
                        info!(
                            hash = %hex::encode(result.ceremony_hash),
                            k = result.new_k, n = result.new_n,
                            distributed = result.distributed,
                            "✅ Re-ceremony 自动完成"
                        );
                        self.send_alert_re_ceremony(&result).await;
                    }
                    Err(e) => {
                        error!(error = %e, "❌ Re-ceremony 自动触发失败");
                        // 不更新 last_re_ceremony_at, 允许下次重试
                    }
                }
            }
        }
    }

    /// 运行监控循环 (后台任务)
    pub async fn run(mut self) {
        let interval = Duration::from_secs(self.config.check_interval_secs);
        info!(
            interval_secs = self.config.check_interval_secs,
            threshold_k = self.config.threshold_k,
            desired_n = self.config.desired_n,
            auto_re_ceremony = self.config.auto_re_ceremony,
            "Peer 监控器已启动"
        );

        loop {
            match self.check_once().await {
                Ok(_status) => {}
                Err(e) => {
                    warn!(error = %e, "Peer 监控检查失败");
                }
            }
            tokio::time::sleep(interval).await;
        }
    }

    /// 发送告警 (webhook 或仅日志)
    async fn send_alert(&self, status: &PeerHealthStatus) {
        let url = match &self.config.alert_webhook_url {
            Some(u) if !u.is_empty() => u,
            _ => return,
        };

        let payload = serde_json::json!({
            "text": format!("[GroupRobot PeerMonitor] {}", status),
            "bot_id_hash": hex::encode(self.config.bot_id_hash),
        });

        match reqwest::Client::new()
            .post(url)
            .json(&payload)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => {
                info!(status_code = resp.status().as_u16(), "告警 Webhook 已发送");
            }
            Err(e) => {
                warn!(error = %e, url, "告警 Webhook 发送失败");
            }
        }
    }

    /// 发送 Re-ceremony 完成通知
    async fn send_alert_re_ceremony(&self, result: &crate::tee::ceremony::ReCeremonyResult) {
        let url = match &self.config.alert_webhook_url {
            Some(u) if !u.is_empty() => u,
            _ => return,
        };

        let payload = serde_json::json!({
            "text": format!(
                "[GroupRobot] ✅ Re-ceremony 完成: K={}, N={}, hash={}",
                result.new_k, result.new_n, hex::encode(result.ceremony_hash)
            ),
            "bot_id_hash": hex::encode(self.config.bot_id_hash),
        });

        let _ = reqwest::Client::new()
            .post(url)
            .json(&payload)
            .timeout(Duration::from_secs(10))
            .send()
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_health_critical() {
        let status = evaluate_peer_health(2, 2);
        assert!(matches!(status, PeerHealthStatus::Critical { .. }));

        let status = evaluate_peer_health(1, 2);
        assert!(matches!(status, PeerHealthStatus::Critical { .. }));

        let status = evaluate_peer_health(0, 2);
        assert!(matches!(status, PeerHealthStatus::Critical { .. }));
    }

    #[test]
    fn evaluate_health_warning() {
        let status = evaluate_peer_health(3, 2);
        assert!(matches!(status, PeerHealthStatus::Warning { .. }));
    }

    #[test]
    fn evaluate_health_healthy() {
        let status = evaluate_peer_health(4, 2);
        assert!(matches!(status, PeerHealthStatus::Healthy { .. }));

        let status = evaluate_peer_health(10, 3);
        assert!(matches!(status, PeerHealthStatus::Healthy { .. }));
    }

    #[test]
    fn evaluate_health_k1() {
        // K=1: 1 peer = critical, 2 = warning, 3+ = healthy
        assert!(matches!(evaluate_peer_health(1, 1), PeerHealthStatus::Critical { .. }));
        assert!(matches!(evaluate_peer_health(2, 1), PeerHealthStatus::Warning { .. }));
        assert!(matches!(evaluate_peer_health(3, 1), PeerHealthStatus::Healthy { .. }));
    }

    #[test]
    fn status_display() {
        let s = PeerHealthStatus::Critical { peer_count: 2, threshold: 2 };
        let display = format!("{}", s);
        assert!(display.contains("CRITICAL"));
        assert!(display.contains("2"));

        let s = PeerHealthStatus::Warning { peer_count: 3, threshold: 2 };
        let display = format!("{}", s);
        assert!(display.contains("WARNING"));

        let s = PeerHealthStatus::Healthy { peer_count: 5, threshold: 2 };
        let display = format!("{}", s);
        assert!(display.contains("HEALTHY"));
    }

    fn make_peer(pk: [u8; 32], endpoint: &str) -> PeerInfo {
        PeerInfo {
            public_key: pk,
            endpoint: endpoint.to_string(),
            registered_at: 0,
            last_seen: 0,
        }
    }

    #[test]
    fn trigger_skip_no_new_peers() {
        let pk_a = [0x01; 32];
        let peers = vec![make_peer(pk_a, "https://a:8443")];
        let known: HashSet<[u8; 32]> = [pk_a].into();

        let decision = evaluate_trigger(&peers, &known, &pk_a, 2, 4);
        assert!(matches!(decision, TriggerDecision::Skip { .. }));
    }

    #[test]
    fn trigger_skip_not_enough_peers() {
        let pk_a = [0x01; 32];
        let pk_b = [0x02; 32];
        let peers = vec![
            make_peer(pk_a, "https://a:8443"),
            make_peer(pk_b, "https://b:8443"),
        ];
        let known: HashSet<[u8; 32]> = [pk_a].into(); // pk_b is new
        // desired_n = 4, but only 2 peers
        let decision = evaluate_trigger(&peers, &known, &pk_a, 2, 4);
        assert!(matches!(decision, TriggerDecision::Skip { reason } if reason.contains("desired N")));
    }

    #[test]
    fn trigger_skip_not_leader() {
        let pk_a = [0x01; 32]; // smaller pk → leader
        let pk_b = [0x02; 32];
        let pk_c = [0x03; 32];
        let pk_d = [0x04; 32]; // new peer
        let peers = vec![
            make_peer(pk_a, "https://a:8443"),
            make_peer(pk_b, "https://b:8443"),
            make_peer(pk_c, "https://c:8443"),
            make_peer(pk_d, "https://d:8443"),
        ];
        let known: HashSet<[u8; 32]> = [pk_a, pk_b, pk_c].into();

        // pk_b is NOT leader (pk_a is smaller)
        let decision = evaluate_trigger(&peers, &known, &pk_b, 2, 4);
        assert!(matches!(decision, TriggerDecision::Skip { reason } if reason.contains("leader")));
    }

    #[test]
    fn trigger_fires_when_conditions_met() {
        let pk_a = [0x01; 32]; // smallest → leader
        let pk_b = [0x02; 32];
        let pk_c = [0x03; 32];
        let pk_d = [0x04; 32]; // new peer
        let peers = vec![
            make_peer(pk_a, "https://a:8443"),
            make_peer(pk_b, "https://b:8443"),
            make_peer(pk_c, "https://c:8443"),
            make_peer(pk_d, "https://d:8443"),
        ];
        let known: HashSet<[u8; 32]> = [pk_a, pk_b, pk_c].into();

        // pk_a is leader, 4 peers >= desired_n=4, pk_d is new
        let decision = evaluate_trigger(&peers, &known, &pk_a, 2, 4);
        match decision {
            TriggerDecision::Trigger { new_peers, total_peers } => {
                assert_eq!(total_peers, 4);
                assert_eq!(new_peers.len(), 1);
                assert_eq!(new_peers[0], pk_d);
            }
            TriggerDecision::Skip { reason } => {
                panic!("expected Trigger, got Skip: {}", reason);
            }
        }
    }

    #[test]
    fn leader_election_smallest_pk() {
        let pk_a = [0x01; 32];
        let pk_b = [0x02; 32];
        let pk_c = [0xFF; 32];

        let peers = vec![
            make_peer(pk_a, "a"),
            make_peer(pk_b, "b"),
            make_peer(pk_c, "c"),
        ];

        assert!(is_leader(&pk_a, &peers));  // smallest → leader
        assert!(!is_leader(&pk_b, &peers)); // not smallest
        assert!(!is_leader(&pk_c, &peers)); // not smallest
    }

    #[test]
    fn leader_election_equal_pk() {
        let pk = [0x05; 32];
        let peers = vec![make_peer(pk, "self")];
        assert!(is_leader(&pk, &peers)); // only peer → leader
    }
}
