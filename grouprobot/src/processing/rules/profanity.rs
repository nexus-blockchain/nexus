use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G8: 脏话过滤器
///
/// 多语言脏话检测，支持:
/// - 精确匹配 (小写)
/// - 简单防混淆: 常见字符替换 (0→o, 1→i, 3→e, @→a, $→s, ...)
/// - 管理员豁免
/// - 可配置动作: warn / delete / mute
///
/// 脏话列表通过 `profanity_words` 配置字段 (换行分隔) 加载。
pub struct ProfanityRule {
    words: Vec<String>,   // 小写脏话列表
    action: ProfanityAction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfanityAction {
    Warn,
    Delete,
    Mute,
}

impl ProfanityRule {
    pub fn new(words: Vec<String>, action: ProfanityAction) -> Self {
        let words: Vec<String> = words.into_iter()
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        Self { words, action }
    }

    /// 从换行分隔的文本加载
    pub fn from_text(text: &str, action: ProfanityAction) -> Self {
        let words: Vec<String> = text.lines()
            .map(|l| l.trim().to_lowercase())
            .filter(|l| !l.is_empty())
            .collect();
        Self { words, action }
    }

    /// 反混淆标准化: 将常见替换字符映射回原始字母
    fn normalize(text: &str) -> String {
        text.to_lowercase()
            .replace('0', "o")
            .replace('1', "i")
            .replace('3', "e")
            .replace('4', "a")
            .replace('5', "s")
            .replace('7', "t")
            .replace('8', "b")
            .replace('@', "a")
            .replace('$', "s")
            .replace('!', "i")
            .replace('|', "l")
            // 常见 Unicode 替换
            .replace('а', "a") // Cyrillic а → Latin a
            .replace('е', "e") // Cyrillic е → Latin e
            .replace('о', "o") // Cyrillic о → Latin o
            .replace('р', "p") // Cyrillic р → Latin p
            .replace('с', "c") // Cyrillic с → Latin c
            .replace('х', "x") // Cyrillic х → Latin x
    }

    /// 检查文本中是否包含脏话 (支持防混淆)
    fn contains_profanity(&self, text: &str) -> Option<&str> {
        let normalized = Self::normalize(text);
        // 按单词边界分割
        let words_in_text: Vec<&str> = normalized
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();

        for bad_word in &self.words {
            // 精确单词匹配
            for word in &words_in_text {
                if *word == bad_word.as_str() {
                    return Some(bad_word);
                }
            }
            // 子串匹配 (对于多词脏话)
            if bad_word.contains(' ') {
                let bad_normalized = Self::normalize(bad_word);
                if normalized.contains(&bad_normalized) {
                    return Some(bad_word);
                }
            }
        }
        None
    }

    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

#[async_trait]
impl Rule for ProfanityRule {
    fn name(&self) -> &'static str { "profanity" }

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
        if ctx.message_text.is_empty() || self.words.is_empty() {
            return None;
        }

        if let Some(matched) = self.contains_profanity(&ctx.message_text) {
            let reason = format!("Profanity filter: 匹配 '{}'", matched);
            return match self.action {
                ProfanityAction::Warn => {
                    Some(ActionDecision::warn(&ctx.sender_id, &reason))
                }
                ProfanityAction::Delete => {
                    if let Some(ref msg_id) = ctx.message_id {
                        Some(ActionDecision::delete_message(msg_id))
                    } else {
                        Some(ActionDecision::warn(&ctx.sender_id, &reason))
                    }
                }
                ProfanityAction::Mute => {
                    Some(ActionDecision::mute(&ctx.sender_id, 300, &reason))
                }
            };
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
        }
    }

    #[tokio::test]
    async fn basic_match() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["fuck".into(), "shit".into()], ProfanityAction::Warn);
        let d = rule.evaluate(&make_ctx("what the fuck"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn leet_speak_detected() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["shit".into()], ProfanityAction::Warn);
        // $h1t → shit after normalize
        let d = rule.evaluate(&make_ctx("that's $h1t"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn cyrillic_substitution() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["scam".into()], ProfanityAction::Warn);
        // сcаm (Cyrillic с and а) → scam after normalize
        let d = rule.evaluate(&make_ctx("this is а scаm"), &store).await;
        // "scаm" → normalize → "scam"
        assert!(d.is_some());
    }

    #[tokio::test]
    async fn no_match_passes() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["badword".into()], ProfanityAction::Warn);
        assert!(rule.evaluate(&make_ctx("hello world"), &store).await.is_none());
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["fuck".into()], ProfanityAction::Warn);
        let mut ctx = make_ctx("fuck");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn delete_action() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["spam".into()], ProfanityAction::Delete);
        let d = rule.evaluate(&make_ctx("spam here"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn mute_action() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["spam".into()], ProfanityAction::Mute);
        let d = rule.evaluate(&make_ctx("spam here"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Mute);
        assert_eq!(d.duration_secs, Some(300));
    }

    #[tokio::test]
    async fn case_insensitive() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec!["fuck".into()], ProfanityAction::Warn);
        assert!(rule.evaluate(&make_ctx("FUCK you"), &store).await.is_some());
    }

    #[tokio::test]
    async fn empty_words_no_match() {
        let store = LocalStore::new();
        let rule = ProfanityRule::new(vec![], ProfanityAction::Warn);
        assert!(rule.evaluate(&make_ctx("anything"), &store).await.is_none());
    }

    #[tokio::test]
    async fn from_text_works() {
        let rule = ProfanityRule::from_text("fuck\nshit\n\nasshole", ProfanityAction::Warn);
        assert_eq!(rule.word_count(), 3);
    }
}
