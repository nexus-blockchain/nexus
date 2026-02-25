use std::sync::Arc;
use dashmap::DashMap;
use tracing::{info, warn, debug};

use crate::chain::ChainClient;
use crate::chain::types::ChainCommunityConfig;

/// 群配置管理器 — 本地缓存 + 链上定期同步
pub struct ConfigManager {
    /// 本地缓存: community_id_hash → (config, last_sync_timestamp)
    cache: DashMap<[u8; 32], (ChainCommunityConfig, u64)>,
    sync_interval_secs: u64,
}

impl ConfigManager {
    pub fn new(sync_interval_secs: u64) -> Self {
        Self {
            cache: DashMap::new(),
            sync_interval_secs,
        }
    }

    /// 获取群配置 (优先本地缓存)
    pub fn get_config(&self, community_id_hash: &[u8; 32]) -> Option<ChainCommunityConfig> {
        self.cache.get(community_id_hash).map(|v| v.0.clone())
    }

    /// 设置本地缓存
    pub fn set_config(&self, community_id_hash: [u8; 32], config: ChainCommunityConfig) {
        let now = now_secs();
        self.cache.insert(community_id_hash, (config, now));
    }

    /// 检查是否需要同步
    pub fn needs_sync(&self, community_id_hash: &[u8; 32]) -> bool {
        match self.cache.get(community_id_hash) {
            Some(entry) => {
                let (_, last_sync) = entry.value();
                now_secs() - last_sync >= self.sync_interval_secs
            }
            None => true,
        }
    }

    /// 从链上同步单个群配置
    pub async fn sync_one(&self, chain: &ChainClient, community_id_hash: [u8; 32]) {
        match chain.fetch_community_config(&community_id_hash).await {
            Ok(Some(config)) => {
                let old_version = self.cache.get(&community_id_hash)
                    .map(|v| v.0.version)
                    .unwrap_or(0);

                if config.version != old_version {
                    info!(
                        community = hex::encode(community_id_hash),
                        old_version,
                        new_version = config.version,
                        "群配置已更新"
                    );
                }
                self.set_config(community_id_hash, config);
            }
            Ok(None) => {
                debug!(community = hex::encode(community_id_hash), "链上无群配置");
            }
            Err(e) => {
                warn!(error = %e, "同步群配置失败");
            }
        }
    }

    /// 后台同步循环
    pub async fn sync_loop(self: Arc<Self>, chain: Arc<ChainClient>) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.sync_interval_secs)
        );

        loop {
            interval.tick().await;
            let keys: Vec<[u8; 32]> = self.cache.iter()
                .map(|entry| *entry.key())
                .collect();

            for key in keys {
                if self.needs_sync(&key) {
                    self.sync_one(&chain, key).await;
                }
            }
        }
    }

    pub fn cached_count(&self) -> usize {
        self.cache.len()
    }

    /// M6 修复: 注册新群组 (触发下次 sync_loop 同步其配置)
    /// 如果缓存中已存在则不覆盖
    pub fn register_group(&self, community_id_hash: [u8; 32]) {
        if !self.cache.contains_key(&community_id_hash) {
            // 插入空配置占位，needs_sync 会返回 true 触发同步
            self.cache.entry(community_id_hash).or_insert_with(|| {
                (ChainCommunityConfig {
                    node_requirement: 0, anti_flood_enabled: false,
                    flood_limit: 10, warn_limit: 3, warn_action: 0,
                    welcome_enabled: false, version: 0,
                    anti_duplicate_enabled: false,
                    duplicate_window_secs: 300, duplicate_threshold: 3,
                    max_emoji: 0, max_links: 0,
                    stop_words: String::new(),
                    welcome_template: String::new(),
                    goodbye_template: String::new(),
                    warn_mute_duration: 3600,
                    spam_samples: String::new(),
                    similarity_threshold: 70,
                    log_channel_id: String::new(),
                    captcha_enabled: false,
                    captcha_timeout_secs: 120,
                    antiphishing_enabled: false,
                    bayes_threshold: 80,
                    custom_commands_csv: String::new(),
                    locked_types_csv: String::new(),
                    subscription_tier: 0,
                    max_rules: 3,
                    forced_ads_per_day: 2,
                    can_disable_ads: false,
                    community_id_hash: String::new(),
                }, 0) // last_sync = 0 → needs_sync 立即返回 true
            });
            debug!(community = hex::encode(community_id_hash), "新群组已注册, 等待同步");
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_set_get() {
        let mgr = ConfigManager::new(30);
        let hash = [1u8; 32];
        assert!(mgr.get_config(&hash).is_none());

        mgr.set_config(hash, ChainCommunityConfig {
            node_requirement: 0,
            anti_flood_enabled: true,
            flood_limit: 10,
            warn_limit: 3,
            warn_action: 0,
            welcome_enabled: false,
            version: 1,
            anti_duplicate_enabled: false,
            duplicate_window_secs: 300,
            duplicate_threshold: 3,
            max_emoji: 0,
            max_links: 0,
            stop_words: String::new(),
            welcome_template: String::new(),
            goodbye_template: String::new(),
            warn_mute_duration: 3600,
            spam_samples: String::new(),
            similarity_threshold: 70,
            log_channel_id: String::new(),
            captcha_enabled: false,
            captcha_timeout_secs: 120,
            antiphishing_enabled: false,
            bayes_threshold: 80,
            custom_commands_csv: String::new(),
            locked_types_csv: String::new(),
            subscription_tier: 0,
            max_rules: 50,
            forced_ads_per_day: 0,
            can_disable_ads: true,
            community_id_hash: String::new(),
        });

        let config = mgr.get_config(&hash).unwrap();
        assert_eq!(config.version, 1);
        assert!(config.anti_flood_enabled);
    }

    #[test]
    fn needs_sync_for_unknown() {
        let mgr = ConfigManager::new(30);
        assert!(mgr.needs_sync(&[2u8; 32]));
    }

    #[test]
    fn fresh_cache_no_sync() {
        let mgr = ConfigManager::new(30);
        let hash = [3u8; 32];
        mgr.set_config(hash, ChainCommunityConfig {
            node_requirement: 0, anti_flood_enabled: false,
            flood_limit: 5, warn_limit: 3, warn_action: 0,
            welcome_enabled: false, version: 1,
            anti_duplicate_enabled: false,
            duplicate_window_secs: 300,
            duplicate_threshold: 3,
            max_emoji: 0,
            max_links: 0,
            stop_words: String::new(),
            welcome_template: String::new(),
            goodbye_template: String::new(),
            warn_mute_duration: 3600,
            spam_samples: String::new(),
            similarity_threshold: 70,
            log_channel_id: String::new(),
            captcha_enabled: false,
            captcha_timeout_secs: 120,
            antiphishing_enabled: false,
            bayes_threshold: 80,
            custom_commands_csv: String::new(),
            locked_types_csv: String::new(),
            subscription_tier: 0,
            max_rules: 50,
            forced_ads_per_day: 0,
            can_disable_ads: true,
            community_id_hash: String::new(),
        });
        assert!(!mgr.needs_sync(&hash));
    }
}
