use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, debug, error};

use crate::chain::ChainClient;
use crate::chain::types::AdCampaignInfo;
use crate::infra::audience_tracker::AudienceTracker;
use crate::platform::{ActionType, ExecuteAction};
use crate::tee::key_manager::KeyManager;

/// 广告投放循环 — 后台定时查询链上排期 + 发送广告 + 上报收据
///
/// 独立 tokio::spawn, 每 5 分钟检查一次排期。
pub struct AdDeliveryLoop {
    /// 管理的群组列表 (community_id_hash hex → group_id on platform)
    managed_groups: Arc<dashmap::DashMap<String, ManagedGroup>>,
    /// audience 追踪器
    audience_tracker: Arc<AudienceTracker>,
    /// P5-fix: TEE 密钥管理器 (用于签名投放收据)
    key_manager: Arc<KeyManager>,
    /// 最低 audience 门槛
    min_audience: u32,
    /// 投放间隔 (秒)
    delivery_interval_secs: u64,
    /// C1 fix: 链上节点 ID (链上 submit_delivery_receipt 必填)
    node_id: u32,
}

/// 管理的群组信息
#[derive(Debug, Clone)]
pub struct ManagedGroup {
    pub community_id_hash: [u8; 32],
    pub platform_group_id: String,
    pub platform: String,
    pub ads_enabled: bool,
    /// 今天已投放次数
    pub delivered_today: u32,
    /// 每日上限
    pub daily_limit: u32,
}

/// 单次投放结果
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub campaign_id: u64,
    pub community_id_hash: [u8; 32],
    pub audience_size: u32,
    pub success: bool,
}

impl AdDeliveryLoop {
    pub fn new(
        audience_tracker: Arc<AudienceTracker>,
        key_manager: Arc<KeyManager>,
        min_audience: u32,
        delivery_interval_secs: u64,
        node_id: u32,
    ) -> Self {
        Self {
            managed_groups: Arc::new(dashmap::DashMap::new()),
            audience_tracker,
            key_manager,
            min_audience,
            delivery_interval_secs,
            node_id,
        }
    }

    /// 注册一个管理的群组
    pub fn register_group(&self, community_hash_hex: String, group: ManagedGroup) {
        self.managed_groups.insert(community_hash_hex, group);
    }

    /// 更新群组的 ads_enabled 状态
    pub fn set_ads_enabled(&self, community_hash_hex: &str, enabled: bool) {
        if let Some(mut entry) = self.managed_groups.get_mut(community_hash_hex) {
            entry.ads_enabled = enabled;
        }
    }

    /// 主循环 (在 tokio::spawn 中运行)
    ///
    /// 需要传入 chain_client 和一个 send_message 回调。
    /// send_message 返回 true 表示消息发送成功。
    pub async fn run_loop<F, Fut>(
        &self,
        chain: Arc<ChainClient>,
        send_message: F,
    ) where
        F: Fn(ExecuteAction) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send,
    {
        let mut interval = tokio::time::interval(
            Duration::from_secs(self.delivery_interval_secs)
        );

        loop {
            interval.tick().await;

            // 先清理 audience_tracker 过期条目
            self.audience_tracker.cleanup_expired();

            // 遍历有广告资格的群组
            for entry in self.managed_groups.iter() {
                let hash_hex = entry.key().clone();
                let group = entry.value().clone();

                if !group.ads_enabled {
                    continue;
                }

                if group.delivered_today >= group.daily_limit {
                    debug!(group = %hash_hex, "已达每日投放上限, 跳过");
                    continue;
                }

                // 检查 audience
                let audience = self.audience_tracker.compute_audience_size(&group.platform_group_id);
                if audience < self.min_audience {
                    debug!(
                        group = %hash_hex,
                        audience,
                        min = self.min_audience,
                        "活跃人数不足, 跳过广告"
                    );
                    continue;
                }

                // 查询链上排期
                match chain.query_ad_schedule(&group.community_id_hash).await {
                    Ok(Some(schedule)) => {
                        for campaign_id in &schedule.campaign_ids {
                            // 查询广告内容
                            match chain.query_campaign(*campaign_id).await {
                                Ok(Some(ad_info)) => {
                                    let result = self.deliver_ad(
                                        &chain,
                                        &send_message,
                                        &group,
                                        *campaign_id,
                                        &ad_info,
                                        audience,
                                    ).await;

                                    if result.success {
                                        // 递增今日计数
                                        if let Some(mut g) = self.managed_groups.get_mut(&hash_hex) {
                                            g.delivered_today = g.delivered_today.saturating_add(1);
                                        }
                                    }
                                }
                                Ok(None) => {
                                    warn!(campaign_id, "Campaign 不存在, 跳过");
                                }
                                Err(e) => {
                                    error!(campaign_id, err = %e, "查询 Campaign 失败");
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        debug!(group = %hash_hex, "无广告排期");
                    }
                    Err(e) => {
                        error!(group = %hash_hex, err = %e, "查询广告排期失败");
                    }
                }
            }
        }
    }

    /// 执行一次广告投放
    async fn deliver_ad<F, Fut>(
        &self,
        chain: &ChainClient,
        send_message: &F,
        group: &ManagedGroup,
        campaign_id: u64,
        campaign: &AdCampaignInfo,
        audience: u32,
    ) -> DeliveryResult
    where
        F: Fn(ExecuteAction) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = bool> + Send,
    {
        // 构造广告消息
        let ad_message = format_ad_message(&campaign.text, &campaign.url);

        let action = ExecuteAction {
            action_type: ActionType::SendMessage,
            group_id: group.platform_group_id.clone(),
            target_user: String::new(),
            reason: None,
            message: Some(ad_message),
            duration_secs: None,
            inline_keyboard: None,
            callback_query_id: None,
            channel_id: Some(group.platform_group_id.clone()),
        };

        let success = send_message(action).await;

        if success {
            info!(
                campaign_id,
                group = %group.platform_group_id,
                audience,
                "广告投放成功"
            );

            // P5-fix: TEE 签名投放收据
            // 签名消息 = campaign_id(u64 LE) || community_id_hash(32) || delivery_type(u8) || audience_size(u32 LE)
            let receipt_sig = {
                let mut msg = Vec::with_capacity(8 + 32 + 1 + 4);
                msg.extend_from_slice(&campaign_id.to_le_bytes());
                msg.extend_from_slice(&group.community_id_hash);
                msg.push(0u8); // ScheduledPost
                msg.extend_from_slice(&audience.to_le_bytes());
                self.key_manager.sign_receipt(&msg)
            };

            if let Err(e) = chain.submit_delivery_receipt(
                campaign_id,
                group.community_id_hash,
                0, // delivery_type = ScheduledPost
                audience,
                self.node_id,
                receipt_sig,
            ).await {
                warn!(campaign_id, err = %e, "上报投放收据失败");
            }
        } else {
            warn!(
                campaign_id,
                group = %group.platform_group_id,
                "广告投放失败 (消息发送失败)"
            );
        }

        DeliveryResult {
            campaign_id,
            community_id_hash: group.community_id_hash,
            audience_size: audience,
            success,
        }
    }

    /// 每日重置投放计数 (应在 midnight 调用)
    pub fn reset_daily_counts(&self) {
        for mut entry in self.managed_groups.iter_mut() {
            entry.delivered_today = 0;
        }
    }

    /// 统计信息
    pub fn stats(&self) -> AdDeliveryStats {
        let mut total_groups = 0u32;
        let mut ads_enabled_groups = 0u32;
        let mut total_delivered = 0u32;

        for entry in self.managed_groups.iter() {
            total_groups += 1;
            if entry.ads_enabled {
                ads_enabled_groups += 1;
            }
            total_delivered += entry.delivered_today;
        }

        AdDeliveryStats {
            total_groups,
            ads_enabled_groups,
            total_delivered_today: total_delivered,
        }
    }
}

/// 广告投放统计
#[derive(Debug, Clone)]
pub struct AdDeliveryStats {
    pub total_groups: u32,
    pub ads_enabled_groups: u32,
    pub total_delivered_today: u32,
}

/// 构造标准广告消息格式 (Discord Embed / TG HTML)
fn format_ad_message(text: &str, url: &str) -> String {
    format!(
        "📢 赞助推广\n{}\n🔗 {}\n────\n由 Nexus 广告网络提供 | 升级 Pro 去广告",
        text, url
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_delivery(tracker: Arc<AudienceTracker>, min_audience: u32, interval_secs: u64) -> AdDeliveryLoop {
        use crate::tee::enclave_bridge::EnclaveBridge;
        use crate::tee::key_manager::KeyManager;
        let dir = tempfile::tempdir().unwrap();
        let enclave = Arc::new(EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap());
        let km = Arc::new(KeyManager::new(enclave, [0u8; 32]));
        AdDeliveryLoop::new(tracker, km, min_audience, interval_secs, 0)
    }

    #[test]
    fn format_ad_message_works() {
        let msg = format_ad_message("Try Nexus DEX", "https://nexus.app");
        assert!(msg.contains("📢 赞助推广"));
        assert!(msg.contains("Try Nexus DEX"));
        assert!(msg.contains("https://nexus.app"));
        assert!(msg.contains("升级 Pro 去广告"));
    }

    #[test]
    fn register_and_stats() {
        let tracker = Arc::new(AudienceTracker::new());
        let delivery = make_test_delivery(tracker, 20, 300);

        delivery.register_group("aabb".into(), ManagedGroup {
            community_id_hash: [0xaa; 32],
            platform_group_id: "g1".into(),
            platform: "telegram".into(),
            ads_enabled: true,
            delivered_today: 0,
            daily_limit: 2,
        });

        delivery.register_group("ccdd".into(), ManagedGroup {
            community_id_hash: [0xcc; 32],
            platform_group_id: "g2".into(),
            platform: "discord".into(),
            ads_enabled: false,
            delivered_today: 0,
            daily_limit: 1,
        });

        let stats = delivery.stats();
        assert_eq!(stats.total_groups, 2);
        assert_eq!(stats.ads_enabled_groups, 1);
        assert_eq!(stats.total_delivered_today, 0);
    }

    #[test]
    fn set_ads_enabled() {
        let tracker = Arc::new(AudienceTracker::new());
        let delivery = make_test_delivery(tracker, 20, 300);

        delivery.register_group("aabb".into(), ManagedGroup {
            community_id_hash: [0xaa; 32],
            platform_group_id: "g1".into(),
            platform: "telegram".into(),
            ads_enabled: false,
            delivered_today: 0,
            daily_limit: 2,
        });

        assert_eq!(delivery.stats().ads_enabled_groups, 0);
        delivery.set_ads_enabled("aabb", true);
        assert_eq!(delivery.stats().ads_enabled_groups, 1);
    }

    #[test]
    fn reset_daily_counts() {
        let tracker = Arc::new(AudienceTracker::new());
        let delivery = make_test_delivery(tracker, 20, 300);

        delivery.register_group("aabb".into(), ManagedGroup {
            community_id_hash: [0xaa; 32],
            platform_group_id: "g1".into(),
            platform: "telegram".into(),
            ads_enabled: true,
            delivered_today: 5,
            daily_limit: 10,
        });

        assert_eq!(delivery.stats().total_delivered_today, 5);
        delivery.reset_daily_counts();
        assert_eq!(delivery.stats().total_delivered_today, 0);
    }

    #[test]
    fn delivery_result_struct() {
        let r = DeliveryResult {
            campaign_id: 42,
            community_id_hash: [1u8; 32],
            audience_size: 150,
            success: true,
        };
        assert!(r.success);
        assert_eq!(r.campaign_id, 42);
    }
}
