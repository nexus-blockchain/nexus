use async_trait::async_trait;
use regex::Regex;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 链接数量限制规则
/// 消息中 URL 超过阈值 → 删除 + 警告
pub struct LinkLimitRule {
    max_links: u16,
    url_regex: Regex,
}

impl LinkLimitRule {
    pub fn new(max_links: u16) -> Self {
        Self {
            max_links,
            url_regex: Regex::new(
                r"(?i)https?://[^\s<>\[\](){}]+|www\.[^\s<>\[\](){}]+"
            ).expect("invalid URL regex"),
        }
    }

    fn count_links(&self, text: &str) -> usize {
        self.url_regex.find_iter(text).count()
    }
}

#[async_trait]
impl Rule for LinkLimitRule {
    fn name(&self) -> &'static str { "link_limit" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }

        // 管理员豁免链接限制
        if ctx.is_admin {
            return None;
        }

        let link_count = self.count_links(&ctx.message_text);
        if link_count > self.max_links as usize {
            Some(ActionDecision::warn(
                &ctx.sender_id,
                &format!(
                    "Too many links: {} (limit: {})",
                    link_count, self.max_links
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

    #[test]
    fn count_http_links() {
        let rule = LinkLimitRule::new(3);
        assert_eq!(rule.count_links("visit http://example.com and https://foo.bar"), 2);
    }

    #[test]
    fn count_www_links() {
        let rule = LinkLimitRule::new(3);
        assert_eq!(rule.count_links("go to www.example.com"), 1);
    }

    #[test]
    fn no_links() {
        let rule = LinkLimitRule::new(3);
        assert_eq!(rule.count_links("hello world no links here"), 0);
    }

    #[tokio::test]
    async fn under_limit_passes() {
        let store = LocalStore::new();
        let rule = LinkLimitRule::new(2);
        let ctx = make_ctx("check https://example.com");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn over_limit_warns() {
        let store = LocalStore::new();
        let rule = LinkLimitRule::new(1);
        let ctx = make_ctx("http://a.com http://b.com http://c.com");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = LinkLimitRule::new(0);
        let mut ctx = make_ctx("http://a.com http://b.com");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn empty_message_skipped() {
        let store = LocalStore::new();
        let rule = LinkLimitRule::new(0);
        let ctx = make_ctx("");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
