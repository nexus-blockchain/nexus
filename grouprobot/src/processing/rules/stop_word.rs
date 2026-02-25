use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 停用词规则
/// 消息包含停用词 → 删除 + 警告
/// 停用词列表来自链上 ChainCommunityConfig.stop_words (逗号分隔)
pub struct StopWordRule {
    words: Vec<String>,
}

impl StopWordRule {
    /// 从逗号分隔的字符串构建停用词列表
    pub fn from_csv(csv: &str) -> Self {
        let words: Vec<String> = csv
            .split(',')
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        Self { words }
    }

    #[allow(dead_code)]
    pub fn from_list(words: Vec<String>) -> Self {
        let words = words.into_iter()
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        Self { words }
    }

    fn find_match(&self, text: &str) -> Option<&str> {
        let lower = text.to_lowercase();
        self.words.iter()
            .find(|word| lower.contains(word.as_str()))
            .map(|w| w.as_str())
    }
}

#[async_trait]
impl Rule for StopWordRule {
    fn name(&self) -> &'static str { "stop_word" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }

        self.find_match(&ctx.message_text).map(|matched| ActionDecision::warn(
            &ctx.sender_id,
            &format!("Message contains prohibited word: \"{}\"", matched),
        ))
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
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
        }
    }

    #[tokio::test]
    async fn no_stop_words_passes() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_csv("");
        assert!(rule.evaluate(&make_ctx("hello"), &store).await.is_none());
    }

    #[tokio::test]
    async fn matching_word_warns() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_csv("scam,phishing,crypto airdrop");
        let result = rule.evaluate(&make_ctx("free crypto airdrop!"), &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn case_insensitive_match() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_csv("SPAM");
        let result = rule.evaluate(&make_ctx("this is spam link"), &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn non_matching_passes() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_csv("scam,phishing");
        assert!(rule.evaluate(&make_ctx("normal message"), &store).await.is_none());
    }

    #[tokio::test]
    async fn from_list_works() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_list(vec!["bad".into(), "evil".into()]);
        assert!(rule.evaluate(&make_ctx("this is bad"), &store).await.is_some());
        assert!(rule.evaluate(&make_ctx("this is good"), &store).await.is_none());
    }

    #[tokio::test]
    async fn csv_trims_whitespace() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_csv(" scam , phishing , ");
        let result = rule.evaluate(&make_ctx("possible scam"), &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn commands_skipped() {
        let store = LocalStore::new();
        let rule = StopWordRule::from_csv("scam");
        let mut ctx = make_ctx("scam");
        ctx.is_command = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
