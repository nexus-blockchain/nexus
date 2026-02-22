use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 入群请求审批规则
#[allow(dead_code)]
pub struct JoinRequestRule {
    auto_approve: bool,
}

impl JoinRequestRule {
    pub fn new() -> Self {
        Self { auto_approve: true }
    }

    #[allow(dead_code)]
    pub fn with_policy(auto_approve: bool) -> Self {
        Self { auto_approve }
    }
}

#[async_trait]
impl Rule for JoinRequestRule {
    fn name(&self) -> &'static str { "join_request" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if !ctx.is_join_request {
            return None;
        }

        if self.auto_approve {
            Some(ActionDecision::approve_join(&ctx.sender_id))
        } else {
            // 待人工审批 — 不自动执行
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join_ctx() -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "new_user".into(),
            sender_name: "NewUser".into(),
            message_text: String::new(),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: true,
            is_admin: false,
        }
    }

    #[tokio::test]
    async fn auto_approve() {
        let store = LocalStore::new();
        let rule = JoinRequestRule::new();
        let result = rule.evaluate(&join_ctx(), &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn manual_approve_returns_none() {
        let store = LocalStore::new();
        let rule = JoinRequestRule::with_policy(false);
        assert!(rule.evaluate(&join_ctx(), &store).await.is_none());
    }

    #[tokio::test]
    async fn non_join_skipped() {
        let store = LocalStore::new();
        let rule = JoinRequestRule::new();
        let mut ctx = join_ctx();
        ctx.is_join_request = false;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
