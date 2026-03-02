use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G16: @提及轰炸检测
///
/// 检测单条消息中大量 @mention，防止用户通过 @all 或逐一 @成员进行骚扰。
/// 超过阈值 → 删除消息 + 警告
pub struct MentionFloodRule {
    /// 单条消息允许的最大 @mention 数
    max_mentions: u16,
}

impl MentionFloodRule {
    pub fn new(max_mentions: u16) -> Self {
        Self {
            max_mentions: if max_mentions == 0 { 5 } else { max_mentions },
        }
    }

    /// 计算文本中的 @mention 数量
    /// 匹配 @username 模式 (排除 @后跟空白或位于邮箱中的情况)
    fn count_mentions(text: &str) -> u16 {
        let mut count: u16 = 0;
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            if chars[i] == '@' {
                // 排除邮箱: 检查 @ 前是否有字母数字 (无空白分隔)
                let is_email = i > 0 && chars[i - 1].is_alphanumeric();
                if !is_email {
                    // 检查 @ 后跟至少一个字母数字字符
                    if i + 1 < len && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_') {
                        count = count.saturating_add(1);
                        // 跳过 username 部分
                        i += 1;
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            i += 1;
                        }
                        continue;
                    }
                }
            }
            i += 1;
        }

        count
    }
}

#[async_trait]
impl Rule for MentionFloodRule {
    fn name(&self) -> &'static str { "mention_flood" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.is_new_member || ctx.is_left_member {
            return None;
        }
        if ctx.is_admin {
            return None;
        }
        if ctx.callback_query_id.is_some() {
            return None;
        }
        if ctx.message_text.is_empty() {
            return None;
        }

        let mention_count = Self::count_mentions(&ctx.message_text);
        if mention_count > self.max_mentions {
            let reason = format!(
                "Mention flood: {} 个@提及 (上限: {})",
                mention_count, self.max_mentions
            );
            // 删除消息 + 返回警告
            if let Some(ref msg_id) = ctx.message_id {
                return Some(ActionDecision::delete_message(msg_id));
            }
            return Some(ActionDecision::warn(&ctx.sender_id, &reason));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn make_ctx(text: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: text.into(),
            message_id: Some("msg_1".into()),
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
    fn count_mentions_basic() {
        assert_eq!(MentionFloodRule::count_mentions("hello @alice @bob"), 2);
    }

    #[test]
    fn count_mentions_excludes_email() {
        assert_eq!(MentionFloodRule::count_mentions("email user@example.com"), 0);
    }

    #[test]
    fn count_mentions_at_start() {
        assert_eq!(MentionFloodRule::count_mentions("@admin check this"), 1);
    }

    #[test]
    fn count_mentions_no_mentions() {
        assert_eq!(MentionFloodRule::count_mentions("hello world"), 0);
    }

    #[test]
    fn count_mentions_bare_at() {
        assert_eq!(MentionFloodRule::count_mentions("@ nothing @ here"), 0);
    }

    #[test]
    fn count_mentions_underscore_user() {
        assert_eq!(MentionFloodRule::count_mentions("@user_name hi"), 1);
    }

    #[test]
    fn count_mentions_many() {
        let text = "@a @b @c @d @e @f @g @h @i @j";
        assert_eq!(MentionFloodRule::count_mentions(text), 10);
    }

    #[tokio::test]
    async fn under_limit_passes() {
        let store = LocalStore::new();
        let rule = MentionFloodRule::new(3);
        assert!(rule.evaluate(&make_ctx("@a @b @c hello"), &store).await.is_none());
    }

    #[tokio::test]
    async fn over_limit_deletes() {
        let store = LocalStore::new();
        let rule = MentionFloodRule::new(2);
        let d = rule.evaluate(&make_ctx("@a @b @c spam"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = MentionFloodRule::new(1);
        let mut ctx = make_ctx("@a @b @c @d @e");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn no_text_passes() {
        let store = LocalStore::new();
        let rule = MentionFloodRule::new(1);
        assert!(rule.evaluate(&make_ctx(""), &store).await.is_none());
    }

    #[tokio::test]
    async fn zero_default_to_5() {
        let rule = MentionFloodRule::new(0);
        assert_eq!(rule.max_mentions, 5);
    }
}
