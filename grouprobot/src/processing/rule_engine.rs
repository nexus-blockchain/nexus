use std::sync::Arc;

use crate::chain::types::ChainCommunityConfig;
use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::RuleDecision;
use crate::processing::rules::Rule;
use crate::processing::rules::flood::FloodRule;
use crate::processing::rules::blacklist::BlacklistRule;
use crate::processing::rules::command::CommandRule;
use crate::processing::rules::join::JoinRequestRule;
use crate::processing::rules::default::DefaultRule;
use crate::processing::rules::duplicate::DuplicateRule;
use crate::processing::rules::emoji::EmojiRule;
use crate::processing::rules::link_limit::LinkLimitRule;
use crate::processing::rules::stop_word::StopWordRule;
use crate::processing::rules::similarity::SimilarityRule;
use crate::processing::rules::antiphishing::AntiPhishingRule;
use crate::processing::rules::lock::LockRule;
use crate::processing::rules::callback::CallbackRule;
use crate::processing::rules::warn_tracker::WarnTracker;
use crate::processing::rules::ad_footer::AdFooterRule;

/// 可插拔规则引擎
pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
    store: Arc<LocalStore>,
    warn_tracker: Option<WarnTracker>,
}

impl RuleEngine {
    /// 根据群配置构建规则链
    pub fn new(store: Arc<LocalStore>, anti_flood: bool, flood_limit: u16) -> Self {
        Self::with_blacklist(store, anti_flood, flood_limit, vec![])
    }

    /// 根据群配置 + 黑名单关键词构建规则链
    pub fn with_blacklist(
        store: Arc<LocalStore>,
        anti_flood: bool,
        flood_limit: u16,
        blacklist_patterns: Vec<String>,
    ) -> Self {
        let mut rules: Vec<Box<dyn Rule>> = vec![];

        if anti_flood {
            rules.push(Box::new(FloodRule::new(flood_limit)));
        }
        if !blacklist_patterns.is_empty() {
            rules.push(Box::new(BlacklistRule::with_patterns(blacklist_patterns)));
        }
        rules.push(Box::new(CommandRule::new()));
        rules.push(Box::new(JoinRequestRule::new()));
        rules.push(Box::new(DefaultRule));

        Self { rules, store, warn_tracker: None }
    }

    /// 从链上群配置构建完整规则链 (Phase 1)
    ///
    /// 规则执行顺序:
    /// 1. FloodRule        — 防刷屏 (频率限制)
    /// 2. DuplicateRule    — 重复消息检测
    /// 3. BlacklistRule    — 正则黑名单
    /// 4. StopWordRule     — 停用词匹配
    /// 5. EmojiRule        — Emoji 数量限制
    /// 6. LinkLimitRule    — 链接数量限制
    /// 7. CommandRule       — 管理指令 (/ban, /mute, /kick, /warn)
    /// 8. JoinRequestRule   — 入群审批 + 欢迎消息
    /// 9. DefaultRule       — 兜底 (无动作)
    pub fn from_config(
        store: Arc<LocalStore>,
        config: &ChainCommunityConfig,
        blacklist_patterns: Vec<String>,
    ) -> Self {
        let mut rules: Vec<Box<dyn Rule>> = vec![];

        // 0. CallbackQuery (Inline 键盘回调, 最高优先级)
        rules.push(Box::new(CallbackRule::new()));

        // 1. Flood
        if config.anti_flood_enabled {
            rules.push(Box::new(FloodRule::new(config.flood_limit)));
        }

        // 2. Duplicate
        if config.anti_duplicate_enabled {
            rules.push(Box::new(DuplicateRule::new(
                config.duplicate_window_secs,
                config.duplicate_threshold,
            )));
        }

        // 3. Blacklist (regex patterns)
        if !blacklist_patterns.is_empty() {
            rules.push(Box::new(BlacklistRule::with_patterns(blacklist_patterns)));
        }

        // 4. StopWord
        if !config.stop_words.is_empty() {
            rules.push(Box::new(StopWordRule::from_csv(&config.stop_words)));
        }

        // 5. Emoji
        if config.max_emoji > 0 {
            rules.push(Box::new(EmojiRule::new(config.max_emoji)));
        }

        // 6. LinkLimit
        if config.max_links > 0 {
            rules.push(Box::new(LinkLimitRule::new(config.max_links)));
        }

        // 7. Similarity (TF-IDF spam detection)
        if !config.spam_samples.is_empty() {
            let samples: Vec<String> = config.spam_samples
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !samples.is_empty() {
                let threshold = config.similarity_threshold as f64 / 100.0;
                rules.push(Box::new(SimilarityRule::new(samples, threshold)));
            }
        }

        // 8. AntiPhishing
        if config.antiphishing_enabled {
            rules.push(Box::new(AntiPhishingRule::with_defaults()));
        }

        // 9. LockRule (消息类型锁定)
        if !config.locked_types_csv.is_empty() {
            let lock_rule = LockRule::new(&config.locked_types_csv);
            if !lock_rule.is_empty() {
                rules.push(Box::new(lock_rule));
            }
        }

        // 10. Command
        rules.push(Box::new(CommandRule::new()));

        // 9. JoinRequest + Welcome
        let welcome_template = if config.welcome_enabled {
            Some(config.welcome_template.clone())
        } else {
            None
        };
        rules.push(Box::new(JoinRequestRule::with_welcome(true, welcome_template)));

        // 11. AdFooter (Free 层级: 回复附带广告)
        if config.subscription_tier == 0 {
            rules.push(Box::new(AdFooterRule::new(true)));
        }

        // P4: 层级门控 — 截断超出 max_rules 的规则 (保留 CallbackRule + DefaultRule)
        // CallbackRule 固定在 index 0, DefaultRule 固定在末尾, 不计入限额
        let max = config.max_rules as usize;
        if max > 0 && rules.len() > max + 1 {
            // 保留 CallbackRule (index 0) + 前 max 条规则
            rules.truncate(max + 1);
        }

        // 12. Default (兜底, 始终最后)
        rules.push(Box::new(DefaultRule));

        // WarnTracker (post-processor)
        let warn_tracker = if config.warn_limit > 0 {
            Some(WarnTracker::new(
                config.warn_limit,
                config.warn_action,
                config.warn_mute_duration,
            ))
        } else {
            None
        };

        Self { rules, store, warn_tracker }
    }

    /// 添加自定义规则
    pub fn add_rule(&mut self, rule: Box<dyn Rule>) {
        // 插入到 DefaultRule 之前
        let len = self.rules.len();
        if len > 0 {
            self.rules.insert(len - 1, rule);
        } else {
            self.rules.push(rule);
        }
    }

    /// 评估消息，返回第一个匹配的规则决策
    /// 如果结果是 Warn，经 WarnTracker 后处理 (累积计数 + 自动升级)
    pub async fn evaluate(&self, ctx: &MessageContext) -> RuleDecision {
        for rule in &self.rules {
            if let Some(decision) = rule.evaluate(ctx, &self.store).await {
                // WarnTracker 后处理
                let final_decision = if let Some(ref tracker) = self.warn_tracker {
                    tracker.process(decision, &self.store, &ctx.group_id)
                } else {
                    decision
                };

                return RuleDecision {
                    matched_rule: rule.name().to_string(),
                    action: Some(final_decision),
                };
            }
        }
        RuleDecision {
            matched_rule: "none".to_string(),
            action: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn make_ctx(text: &str, is_cmd: bool) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: text.into(),
            message_id: None,
            is_command: is_cmd,
            command: if is_cmd { Some(text.trim_start_matches('/').split(' ').next().unwrap_or("").into()) } else { None },
            command_args: if is_cmd {
                text.splitn(2, ' ').skip(1).flat_map(|s| s.split_whitespace()).map(|s| s.to_string()).collect()
            } else { vec![] },
            is_join_request: false,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
        }
    }

    fn default_config() -> ChainCommunityConfig {
        ChainCommunityConfig {
            node_requirement: 1,
            anti_flood_enabled: true,
            flood_limit: 100,
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
        }
    }

    #[tokio::test]
    async fn normal_message_no_action() {
        let store = Arc::new(LocalStore::new());
        let engine = RuleEngine::new(store, true, 100);
        let decision = engine.evaluate(&make_ctx("hello", false)).await;
        assert!(decision.action.is_none());
    }

    #[tokio::test]
    async fn ban_command_triggers() {
        let store = Arc::new(LocalStore::new());
        let engine = RuleEngine::new(store, true, 100);
        let decision = engine.evaluate(&make_ctx("/ban 789", true)).await;
        assert!(decision.action.is_some());
        assert_eq!(decision.matched_rule, "command");
    }

    #[tokio::test]
    async fn blacklist_from_config_triggers() {
        let store = Arc::new(LocalStore::new());
        let engine = RuleEngine::with_blacklist(store, false, 100, vec!["(?i)scam".into()]);
        let decision = engine.evaluate(&make_ctx("this is a SCAM link", false)).await;
        assert!(decision.action.is_some());
        assert_eq!(decision.matched_rule, "blacklist");
    }

    #[tokio::test]
    async fn flood_triggers_before_command() {
        let store = Arc::new(LocalStore::new());
        let engine = RuleEngine::new(store, true, 2);
        let ctx = make_ctx("hi", false);
        engine.evaluate(&ctx).await;
        engine.evaluate(&ctx).await;
        let decision = engine.evaluate(&ctx).await;
        assert!(decision.action.is_some());
        assert_eq!(decision.matched_rule, "flood");
    }

    // ── Phase 1 新增测试 ──

    #[tokio::test]
    async fn from_config_duplicate_rule() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.anti_duplicate_enabled = true;
        cfg.duplicate_threshold = 2;
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        let ctx = make_ctx("same message", false);
        assert!(engine.evaluate(&ctx).await.action.is_none());
        let d = engine.evaluate(&ctx).await;
        assert!(d.action.is_some());
        assert_eq!(d.matched_rule, "duplicate");
    }

    #[tokio::test]
    async fn from_config_emoji_rule() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.max_emoji = 2;
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        let d = engine.evaluate(&make_ctx("😀😁😂😄", false)).await;
        assert!(d.action.is_some());
        assert_eq!(d.matched_rule, "emoji");
    }

    #[tokio::test]
    async fn from_config_link_limit() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.max_links = 1;
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        let d = engine.evaluate(&make_ctx("http://a.com http://b.com", false)).await;
        assert!(d.action.is_some());
        assert_eq!(d.matched_rule, "link_limit");
    }

    #[tokio::test]
    async fn from_config_stop_word() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.stop_words = "scam,phishing".into();
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        let d = engine.evaluate(&make_ctx("this is a scam", false)).await;
        assert!(d.action.is_some());
        assert_eq!(d.matched_rule, "stop_word");
    }

    #[tokio::test]
    async fn from_config_warn_escalation() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.stop_words = "badword".into();
        cfg.warn_limit = 2;
        cfg.warn_action = 1; // Kick
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        // First warn
        let d = engine.evaluate(&make_ctx("badword here", false)).await;
        assert_eq!(d.action.as_ref().unwrap().action_type, ActionType::Warn);

        // Second warn → escalates to kick
        let d = engine.evaluate(&make_ctx("badword again", false)).await;
        assert_eq!(d.action.as_ref().unwrap().action_type, ActionType::Kick);
    }

    #[tokio::test]
    async fn from_config_welcome_message() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.welcome_enabled = true;
        cfg.welcome_template = "Hello {user}!".into();
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        let mut ctx = make_ctx("", false);
        ctx.is_join_request = true;
        ctx.sender_name = "Alice".into();

        let d = engine.evaluate(&ctx).await;
        let action = d.action.unwrap();
        assert_eq!(action.action_type, ActionType::ApproveJoin);
        assert_eq!(action.message.unwrap(), "Hello Alice!");
    }

    #[tokio::test]
    async fn from_config_similarity_rule() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.spam_samples = "buy cheap bitcoin investment opportunity\nfree gift card amazon click here".into();
        cfg.similarity_threshold = 60;
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        // Similar to spam sample → triggers
        let d = engine.evaluate(&make_ctx("buy bitcoin cheap investment opportunity now", false)).await;
        assert!(d.action.is_some());
        assert_eq!(d.matched_rule, "similarity");

        // Normal message passes
        assert!(engine.evaluate(&make_ctx("good morning everyone", false)).await.action.is_none());
    }

    // ── Phase 4: Tier gating tests ──

    #[tokio::test]
    async fn from_config_free_tier_adds_ad_footer() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.subscription_tier = 0; // Free
        cfg.max_rules = 3;
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        // Free tier: normal msg still passes (AdFooterRule returns None)
        assert!(engine.evaluate(&make_ctx("hello", false)).await.action.is_none());
    }

    #[tokio::test]
    async fn from_config_pro_tier_no_ad_footer() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.subscription_tier = 2; // Pro
        cfg.max_rules = 50;
        let engine = RuleEngine::from_config(store, &cfg, vec![]);

        assert!(engine.evaluate(&make_ctx("hello", false)).await.action.is_none());
    }

    #[tokio::test]
    async fn from_config_all_rules_combined() {
        let store = Arc::new(LocalStore::new());
        let mut cfg = default_config();
        cfg.anti_flood_enabled = true;
        cfg.flood_limit = 100;
        cfg.anti_duplicate_enabled = true;
        cfg.duplicate_threshold = 5;
        cfg.max_emoji = 10;
        cfg.max_links = 3;
        cfg.stop_words = "spam,scam".into();
        cfg.warn_limit = 5;
        cfg.welcome_enabled = true;
        cfg.welcome_template = "Welcome!".into();
        let engine = RuleEngine::from_config(store, &cfg, vec!["(?i)hack".into()]);

        // Normal message passes
        assert!(engine.evaluate(&make_ctx("hello world", false)).await.action.is_none());

        // Stop word triggers
        let d = engine.evaluate(&make_ctx("this is spam content", false)).await;
        assert_eq!(d.matched_rule, "stop_word");

        // Blacklist triggers
        let d = engine.evaluate(&make_ctx("trying to hack", false)).await;
        assert_eq!(d.matched_rule, "blacklist");
    }
}
