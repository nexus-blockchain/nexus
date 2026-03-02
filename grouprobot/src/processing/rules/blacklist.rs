use async_trait::async_trait;
use regex::{Regex, RegexBuilder};

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
    /// 最大模式数量
    const MAX_PATTERNS: usize = 500;
    /// 每个正则最大编译大小 (字节)
    const REGEX_SIZE_LIMIT: usize = 8192;

    pub fn with_patterns(patterns: Vec<String>) -> Self {
        let compiled: Vec<Regex> = patterns.iter()
            .take(Self::MAX_PATTERNS)
            .filter_map(|p| {
                RegexBuilder::new(p)
                    .size_limit(Self::REGEX_SIZE_LIMIT)
                    .build()
                    .ok()
            })
            .collect();
        Self { patterns: compiled }
    }

    #[allow(dead_code)]
    pub fn add_pattern(&mut self, pattern: &str) {
        if self.patterns.len() >= Self::MAX_PATTERNS {
            return;
        }
        if let Ok(re) = RegexBuilder::new(pattern)
            .size_limit(Self::REGEX_SIZE_LIMIT)
            .build()
        {
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
            message_text: text.to_string(),
            message_id: None,
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    #[tokio::test]
    async fn no_patterns_passes() {
        let store = LocalStore::new();
        let rule = BlacklistRule::with_patterns(vec![]);
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
