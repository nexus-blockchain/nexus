use async_trait::async_trait;
use regex::Regex;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 关键词黑名单规则
#[allow(dead_code)]
pub struct BlacklistRule {
    patterns: Vec<Regex>,
}

impl BlacklistRule {
    pub fn new() -> Self {
        Self { patterns: vec![] }
    }

    pub fn with_patterns(patterns: Vec<String>) -> Self {
        let compiled: Vec<Regex> = patterns.iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();
        Self { patterns: compiled }
    }

    #[allow(dead_code)]
    pub fn add_pattern(&mut self, pattern: &str) {
        if let Ok(re) = Regex::new(pattern) {
            self.patterns.push(re);
        }
    }
}

#[async_trait]
impl Rule for BlacklistRule {
    fn name(&self) -> &'static str { "blacklist" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.message_text.is_empty() {
            return None;
        }

        for pattern in &self.patterns {
            if pattern.is_match(&ctx.message_text) {
                return Some(ActionDecision::warn(
                    &ctx.sender_id,
                    &format!("Message contains blacklisted content (matched: {})", pattern.as_str()),
                ));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(text: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: text.into(),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_admin: false,
        }
    }

    #[tokio::test]
    async fn no_patterns_passes() {
        let store = LocalStore::new();
        let rule = BlacklistRule::new();
        assert!(rule.evaluate(&make_ctx("hello"), &store).await.is_none());
    }

    #[tokio::test]
    async fn matching_pattern_warns() {
        let store = LocalStore::new();
        let rule = BlacklistRule::with_patterns(vec!["(?i)spam".into()]);
        let result = rule.evaluate(&make_ctx("this is SPAM"), &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn non_matching_passes() {
        let store = LocalStore::new();
        let rule = BlacklistRule::with_patterns(vec!["badword".into()]);
        assert!(rule.evaluate(&make_ctx("good message"), &store).await.is_none());
    }
}
