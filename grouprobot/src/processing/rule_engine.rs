use std::sync::Arc;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::RuleDecision;
use crate::processing::rules::Rule;
use crate::processing::rules::flood::FloodRule;
use crate::processing::rules::blacklist::BlacklistRule;
use crate::processing::rules::command::CommandRule;
use crate::processing::rules::join::JoinRequestRule;
use crate::processing::rules::default::DefaultRule;

/// 可插拔规则引擎
pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
    store: Arc<LocalStore>,
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
        if blacklist_patterns.is_empty() {
            rules.push(Box::new(BlacklistRule::new()));
        } else {
            rules.push(Box::new(BlacklistRule::with_patterns(blacklist_patterns)));
        }
        rules.push(Box::new(CommandRule::new()));
        rules.push(Box::new(JoinRequestRule::new()));
        rules.push(Box::new(DefaultRule));

        Self { rules, store }
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
    pub async fn evaluate(&self, ctx: &MessageContext) -> RuleDecision {
        for rule in &self.rules {
            if let Some(decision) = rule.evaluate(ctx, &self.store).await {
                return RuleDecision {
                    matched_rule: rule.name().to_string(),
                    action: Some(decision),
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

    fn make_ctx(text: &str, is_cmd: bool) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: text.into(),
            is_command: is_cmd,
            command: if is_cmd { Some(text.trim_start_matches('/').split(' ').next().unwrap_or("").into()) } else { None },
            command_args: if is_cmd {
                text.splitn(2, ' ').skip(1).flat_map(|s| s.split_whitespace()).map(|s| s.to_string()).collect()
            } else { vec![] },
            is_join_request: false,
            is_admin: false,
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
}
