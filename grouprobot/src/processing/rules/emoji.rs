use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// Emoji 数量限制规则
/// 消息中 emoji 超过阈值 → 删除 + 警告
pub struct EmojiRule {
    max_emoji: u16,
}

impl EmojiRule {
    pub fn new(max_emoji: u16) -> Self {
        Self { max_emoji }
    }
}

/// 判断一个字符是否为 emoji
/// M4 修复: Variation Selectors (FE00-FE0F) 和 ZWJ (200D) 不再计为 emoji
fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1F600}'..='\u{1F64F}' | // Emoticons
        '\u{1F300}'..='\u{1F5FF}' | // Misc Symbols and Pictographs
        '\u{1F680}'..='\u{1F6FF}' | // Transport and Map Symbols
        '\u{1F700}'..='\u{1F77F}' | // Alchemical Symbols
        '\u{1F780}'..='\u{1F7FF}' | // Geometric Shapes Extended
        '\u{1F800}'..='\u{1F8FF}' | // Supplemental Arrows-C
        '\u{1F900}'..='\u{1F9FF}' | // Supplemental Symbols and Pictographs
        '\u{1FA00}'..='\u{1FA6F}' | // Chess Symbols
        '\u{1FA70}'..='\u{1FAFF}' | // Symbols and Pictographs Extended-A
        '\u{1F1E0}'..='\u{1F1FF}' | // Regional Indicator Symbols (Flags)
        '\u{231A}'..='\u{231B}'   | // Watch, Hourglass
        '\u{23E9}'..='\u{23F3}'   | // Various media control
        '\u{23F8}'..='\u{23FA}'   | // Pause, Stop, Record
        '\u{25AA}'..='\u{25AB}'   | // Small squares
        '\u{25B6}' | '\u{25C0}'  | // Play buttons
        '\u{25FB}'..='\u{25FE}'   | // Squares
        '\u{2600}'..='\u{26FF}'   | // Misc Symbols (includes zodiac, weather, etc.)
        '\u{2700}'..='\u{27BF}'     // Dingbats (includes check marks, arrows, etc.)
    )
}

/// 统计消息中 emoji 的数量
fn count_emojis(text: &str) -> usize {
    text.chars().filter(|c| is_emoji(*c)).count()
}

#[async_trait]
impl Rule for EmojiRule {
    fn name(&self) -> &'static str { "emoji" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }

        let emoji_count = count_emojis(&ctx.message_text);
        if emoji_count > self.max_emoji as usize {
            Some(ActionDecision::warn(
                &ctx.sender_id,
                &format!(
                    "Too many emojis: {} (limit: {})",
                    emoji_count, self.max_emoji
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
    fn count_basic_emojis() {
        assert_eq!(count_emojis("hello 😀😁😂"), 3);
    }

    #[test]
    fn count_no_emojis() {
        assert_eq!(count_emojis("hello world"), 0);
    }

    #[test]
    fn count_mixed_emojis() {
        assert_eq!(count_emojis("🎉 party! 🎊"), 2);
    }

    #[test]
    fn zwj_and_variation_selectors_not_counted() {
        // ZWJ (U+200D) 和 Variation Selectors (U+FE0F) 不应计为 emoji
        // "❤️" = U+2764 + U+FE0F → 应计为 1 个
        assert_eq!(count_emojis("\u{2764}\u{FE0F}"), 1);
    }

    #[tokio::test]
    async fn under_limit_passes() {
        let store = LocalStore::new();
        let rule = EmojiRule::new(5);
        let ctx = make_ctx("hello 😀😁");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn over_limit_warns() {
        let store = LocalStore::new();
        let rule = EmojiRule::new(2);
        let ctx = make_ctx("😀😁😂😄");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn exact_limit_passes() {
        let store = LocalStore::new();
        let rule = EmojiRule::new(3);
        let ctx = make_ctx("hi 😀😁😂");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn empty_message_skipped() {
        let store = LocalStore::new();
        let rule = EmojiRule::new(0);
        let ctx = make_ctx("");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn commands_skipped() {
        let store = LocalStore::new();
        let rule = EmojiRule::new(1);
        let mut ctx = make_ctx("😀😀😀");
        ctx.is_command = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
