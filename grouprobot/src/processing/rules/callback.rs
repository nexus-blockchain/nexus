use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// CallbackQuery 处理规则 — 处理 Inline 键盘回调
///
/// 回调数据格式: `action:target[:extra]`
/// 例如: `ban:12345`, `mute:12345:3600`, `cancel:12345`
pub struct CallbackRule;

impl CallbackRule {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Rule for CallbackRule {
    fn name(&self) -> &'static str { "callback" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        let callback_id = ctx.callback_query_id.as_deref()?;
        let data = ctx.callback_data.as_deref()?;

        if callback_id.is_empty() || data.is_empty() {
            return None;
        }

        // 仅管理员可操作回调
        if !ctx.is_admin {
            return Some(ActionDecision::answer_callback(callback_id, "⚠️ Admin only"));
        }

        let parts: Vec<&str> = data.splitn(3, ':').collect();
        if parts.len() < 2 {
            return Some(ActionDecision::answer_callback(callback_id, "Invalid callback data"));
        }

        let action = parts[0];
        let target = parts[1];

        match action {
            "ban" => {
                // 先回答回调, 然后执行 ban
                // 实际上我们需要返回 ban 动作, callback_query_id 由执行器处理
                Some(ActionDecision::ban(target, &format!("Confirmed ban by {}", ctx.sender_name)))
            }
            "kick" => {
                Some(ActionDecision::kick(target, &format!("Confirmed kick by {}", ctx.sender_name)))
            }
            "mute" => {
                let duration = parts.get(2)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(3600);
                Some(ActionDecision::mute(target, duration, &format!("Confirmed mute by {}", ctx.sender_name)))
            }
            "unban" => {
                Some(ActionDecision::unban(target, &format!("Confirmed unban by {}", ctx.sender_name)))
            }
            "cancel" => {
                Some(ActionDecision::answer_callback(callback_id, "✅ Action cancelled"))
            }
            _ => {
                Some(ActionDecision::answer_callback(callback_id, "Unknown action"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callback_ctx(data: &str, is_admin: bool) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "admin1".into(),
            sender_name: "Admin".into(),
            message_text: String::new(),
            message_id: Some("100".into()),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin,
            message_type: None,
            callback_query_id: Some("cb_123".into()),
            callback_data: Some(data.into()),
            channel_id: None,
        }
    }

    #[tokio::test]
    async fn ban_callback() {
        let store = LocalStore::new();
        let rule = CallbackRule::new();
        let d = rule.evaluate(&callback_ctx("ban:789", true), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Ban);
        assert_eq!(d.target_user, "789");
    }

    #[tokio::test]
    async fn mute_callback_with_duration() {
        let store = LocalStore::new();
        let rule = CallbackRule::new();
        let d = rule.evaluate(&callback_ctx("mute:789:600", true), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Mute);
        assert_eq!(d.duration_secs, Some(600));
    }

    #[tokio::test]
    async fn cancel_callback() {
        let store = LocalStore::new();
        let rule = CallbackRule::new();
        let d = rule.evaluate(&callback_ctx("cancel:789", true), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::AnswerCallback);
        assert!(d.message.unwrap().contains("cancelled"));
    }

    #[tokio::test]
    async fn non_admin_rejected() {
        let store = LocalStore::new();
        let rule = CallbackRule::new();
        let d = rule.evaluate(&callback_ctx("ban:789", false), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::AnswerCallback);
        assert!(d.message.unwrap().contains("Admin only"));
    }

    #[tokio::test]
    async fn no_callback_data_passes() {
        let store = LocalStore::new();
        let rule = CallbackRule::new();
        let ctx = MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: "normal".into(),
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
        };
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn unban_callback() {
        let store = LocalStore::new();
        let rule = CallbackRule::new();
        let d = rule.evaluate(&callback_ctx("unban:789", true), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Unban);
    }
}
