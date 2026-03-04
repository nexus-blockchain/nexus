use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 重复消息检测规则
/// 同一用户在窗口内发送相同内容超过阈值 → 删除 + 警告
pub struct DuplicateRule {
    window_secs: u64,
    threshold: u16,
}

impl DuplicateRule {
    pub fn new(window_secs: u64, threshold: u16) -> Self {
        Self {
            window_secs: if window_secs == 0 { 300 } else { window_secs },
            threshold: if threshold == 0 { 3 } else { threshold },
        }
    }

    fn hash_text(text: &str) -> u64 {
        let normalized = text.trim().to_lowercase();
        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        hasher.finish()
    }
}

#[async_trait]
impl Rule for DuplicateRule {
    fn name(&self) -> &'static str { "duplicate" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }

        let text_hash = Self::hash_text(&ctx.message_text);
        let count = store.record_message_hash(&ctx.group_id, &ctx.sender_id, text_hash, self.window_secs);

        if count >= self.threshold as u64 {
            Some(ActionDecision::warn(
                &ctx.sender_id,
                &format!(
                    "Duplicate message detected: sent {} times in {}s window",
                    count, self.window_secs
                ),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(text: &str, sender: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: sender.into(),
            sender_name: "test".into(),
            message_text: text.to_string(),
            message_id: None,
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    #[tokio::test]
    async fn under_threshold_passes() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 3);
        let ctx = make_ctx("hello", "u1");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn at_threshold_warns() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 3);
        let ctx = make_ctx("spam spam", "u1");
        rule.evaluate(&ctx, &store).await;
        rule.evaluate(&ctx, &store).await;
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn different_messages_independent() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 2);
        let ctx1 = make_ctx("hello", "u1");
        let ctx2 = make_ctx("world", "u1");
        rule.evaluate(&ctx1, &store).await;
        assert!(rule.evaluate(&ctx2, &store).await.is_none());
    }

    #[tokio::test]
    async fn different_users_independent() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 2);
        let ctx1 = make_ctx("same msg", "u1");
        let ctx2 = make_ctx("same msg", "u2");
        rule.evaluate(&ctx1, &store).await;
        assert!(rule.evaluate(&ctx2, &store).await.is_none());
    }

    #[tokio::test]
    async fn case_insensitive() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 2);
        let ctx1 = make_ctx("Hello World", "u1");
        let ctx2 = make_ctx("hello world", "u1");
        rule.evaluate(&ctx1, &store).await;
        let result = rule.evaluate(&ctx2, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn commands_skipped() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 1);
        let mut ctx = make_ctx("/help", "u1");
        ctx.is_command = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn empty_message_skipped() {
        let store = LocalStore::new();
        let rule = DuplicateRule::new(300, 1);
        let ctx = make_ctx("", "u1");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
