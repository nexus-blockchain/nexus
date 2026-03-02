use async_trait::async_trait;
use std::collections::HashSet;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 消息类型锁定规则 — 禁止发送特定类型的消息
/// 支持锁定: photo, video, audio, document, sticker, animation, voice,
///           video_note, forward, contact, location, poll, game, text
pub struct LockRule {
    locked_types: HashSet<String>,
}

impl LockRule {
    pub fn new(locked_types_csv: &str) -> Self {
        let locked_types = locked_types_csv
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        Self { locked_types }
    }

    pub fn is_empty(&self) -> bool {
        self.locked_types.is_empty()
    }

    pub fn locked_types(&self) -> &HashSet<String> {
        &self.locked_types
    }
}

#[async_trait]
impl Rule for LockRule {
    fn name(&self) -> &'static str { "lock" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if self.locked_types.is_empty() {
            return None;
        }

        // 管理员豁免
        if ctx.is_admin {
            return None;
        }

        let msg_type = ctx.message_type.as_deref()?;

        if self.locked_types.contains(msg_type) {
            // 如果有 message_id, 删除消息; 否则发送警告
            if let Some(ref mid) = ctx.message_id {
                return Some(ActionDecision::delete_message(mid));
            }
            return Some(ActionDecision::send_message(
                &ctx.group_id,
                &format!("⚠️ Message type <b>{}</b> is currently locked in this group.", msg_type),
            ));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(msg_type: Option<&str>, is_admin: bool) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: String::new(),
            message_id: Some("123".into()),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_admin,
            message_type: msg_type.map(|s| s.to_string()),
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    #[tokio::test]
    async fn empty_lock_passes_all() {
        let store = LocalStore::new();
        let rule = LockRule::new("");
        assert!(rule.evaluate(&make_ctx(Some("photo"), false), &store).await.is_none());
    }

    #[tokio::test]
    async fn locked_type_deletes() {
        let store = LocalStore::new();
        let rule = LockRule::new("photo,sticker");
        let result = rule.evaluate(&make_ctx(Some("photo"), false), &store).await;
        assert!(result.is_some());
        let d = result.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn unlocked_type_passes() {
        let store = LocalStore::new();
        let rule = LockRule::new("photo,sticker");
        assert!(rule.evaluate(&make_ctx(Some("text"), false), &store).await.is_none());
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = LockRule::new("photo");
        assert!(rule.evaluate(&make_ctx(Some("photo"), true), &store).await.is_none());
    }

    #[tokio::test]
    async fn no_message_type_passes() {
        let store = LocalStore::new();
        let rule = LockRule::new("photo");
        assert!(rule.evaluate(&make_ctx(None, false), &store).await.is_none());
    }

    #[tokio::test]
    async fn case_insensitive_csv() {
        let store = LocalStore::new();
        let rule = LockRule::new("Photo, STICKER, Video");
        assert!(rule.locked_types().contains("photo"));
        assert!(rule.locked_types().contains("sticker"));
        assert!(rule.locked_types().contains("video"));
    }

    #[tokio::test]
    async fn multiple_types_locked() {
        let store = LocalStore::new();
        let rule = LockRule::new("forward,animation,voice");
        assert!(rule.evaluate(&make_ctx(Some("forward"), false), &store).await.is_some());
        assert!(rule.evaluate(&make_ctx(Some("animation"), false), &store).await.is_some());
        assert!(rule.evaluate(&make_ctx(Some("voice"), false), &store).await.is_some());
        assert!(rule.evaluate(&make_ctx(Some("text"), false), &store).await.is_none());
    }
}
