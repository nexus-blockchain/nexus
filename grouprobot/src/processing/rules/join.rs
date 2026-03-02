use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;
use super::text_utils::html_escape;

/// 入群请求审批规则 (含欢迎消息)
#[allow(dead_code)]
pub struct JoinRequestRule {
    auto_approve: bool,
    welcome_template: Option<String>,
}

impl JoinRequestRule {
    pub fn new() -> Self {
        Self { auto_approve: true, welcome_template: None }
    }

    #[allow(dead_code)]
    pub fn with_policy(auto_approve: bool) -> Self {
        Self { auto_approve, welcome_template: None }
    }

    pub fn with_welcome(auto_approve: bool, template: Option<String>) -> Self {
        let template = template.filter(|t| !t.is_empty());
        Self { auto_approve, welcome_template: template }
    }

    /// 渲染欢迎模板，替换变量
    /// L1 修复: 对用户可控变量进行 HTML 转义
    fn render_template(&self, template: &str, ctx: &MessageContext) -> String {
        template
            .replace("{user}", &html_escape(&ctx.sender_name))
            .replace("{user_id}", &html_escape(&ctx.sender_id))
            .replace("{group}", &html_escape(&ctx.group_id))
            .replace("{platform}", &ctx.platform)
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
            let mut decision = ActionDecision::approve_join(&ctx.sender_id);
            // 附带欢迎消息
            if let Some(ref template) = self.welcome_template {
                decision.message = Some(self.render_template(template, ctx));
            }
            Some(decision)
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
            message_id: None,
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: true,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
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

    #[tokio::test]
    async fn welcome_template_rendered() {
        let store = LocalStore::new();
        let rule = JoinRequestRule::with_welcome(
            true,
            Some("Welcome {user} to {group}!".into()),
        );
        let result = rule.evaluate(&join_ctx(), &store).await.unwrap();
        assert_eq!(result.action_type, crate::platform::ActionType::ApproveJoin);
        assert_eq!(result.message.unwrap(), "Welcome NewUser to g1!");
    }

    #[tokio::test]
    async fn no_welcome_template_no_message() {
        let store = LocalStore::new();
        let rule = JoinRequestRule::with_welcome(true, None);
        let result = rule.evaluate(&join_ctx(), &store).await.unwrap();
        assert!(result.message.is_none());
    }

    #[tokio::test]
    async fn empty_template_no_message() {
        let store = LocalStore::new();
        let rule = JoinRequestRule::with_welcome(true, Some("".into()));
        let result = rule.evaluate(&join_ctx(), &store).await.unwrap();
        assert!(result.message.is_none());
    }
}
