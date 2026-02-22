use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 防刷屏规则
pub struct FloodRule {
    limit: u16,
}

impl FloodRule {
    pub fn new(limit: u16) -> Self {
        Self { limit }
    }
}

#[async_trait]
impl Rule for FloodRule {
    fn name(&self) -> &'static str { "flood" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request {
            return None;
        }

        let key = format!("flood:{}:{}", ctx.group_id, ctx.sender_id);
        let count = store.increment_counter(&key, 60); // 60 秒窗口

        if count > self.limit as u64 {
            Some(ActionDecision::mute(
                &ctx.sender_id,
                300, // 5 分钟
                &format!("Flood detected: {} messages/min (limit: {})", count, self.limit),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(group: &str, sender: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: group.into(),
            sender_id: sender.into(),
            sender_name: "test".into(),
            message_text: "hi".into(),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_admin: false,
        }
    }

    #[tokio::test]
    async fn under_limit_passes() {
        let store = LocalStore::new();
        let rule = FloodRule::new(10);
        let ctx = make_ctx("g1", "u1");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn over_limit_mutes() {
        let store = LocalStore::new();
        let rule = FloodRule::new(3);
        let ctx = make_ctx("g1", "u1");
        for _ in 0..3 {
            rule.evaluate(&ctx, &store).await;
        }
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn commands_skip_flood() {
        let store = LocalStore::new();
        let rule = FloodRule::new(1);
        let mut ctx = make_ctx("g1", "u1");
        ctx.is_command = true;
        // Even after many evaluations, commands shouldn't trigger flood
        for _ in 0..10 {
            assert!(rule.evaluate(&ctx, &store).await.is_none());
        }
    }
}
