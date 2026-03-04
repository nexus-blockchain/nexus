use std::collections::HashMap;
use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;
use super::text_utils::tokenize;

/// 消息相似度检测规则
///
/// 借鉴 tg-spam 的 similarity detector:
/// - 将已知 spam 样本 token 化并计算 TF-IDF 向量
/// - 对新消息计算 TF-IDF 向量
/// - 计算余弦相似度，超过阈值则标记为 spam
///
/// Phase 2 实现: 简化版 (词频向量 + 余弦相似度)
/// Phase 3 可升级为完整 TF-IDF (需要 IDF 语料统计)
pub struct SimilarityRule {
    /// 已知 spam 样本的 token 向量
    samples: Vec<TokenVector>,
    /// 相似度阈值 (0.0 - 1.0)
    threshold: f64,
}

/// Token 向量 (词频表示)
#[derive(Debug, Clone)]
struct TokenVector {
    tokens: HashMap<String, f64>,
    norm: f64,
}

impl TokenVector {
    fn from_text(text: &str) -> Self {
        let mut tokens: HashMap<String, f64> = HashMap::new();
        for word in tokenize(text) {
            *tokens.entry(word).or_default() += 1.0;
        }
        let norm = tokens.values().map(|v| v * v).sum::<f64>().sqrt();
        Self { tokens, norm }
    }

    /// 余弦相似度
    fn cosine_similarity(&self, other: &TokenVector) -> f64 {
        if self.norm == 0.0 || other.norm == 0.0 {
            return 0.0;
        }

        let dot: f64 = self.tokens.iter()
            .filter_map(|(k, v)| other.tokens.get(k).map(|ov| v * ov))
            .sum();

        dot / (self.norm * other.norm)
    }
}

impl SimilarityRule {
    pub fn new(spam_samples: Vec<String>, threshold: f64) -> Self {
        let samples: Vec<TokenVector> = spam_samples.iter()
            .map(|s| TokenVector::from_text(s))
            .collect();
        let threshold = if threshold <= 0.0 || threshold > 1.0 { 0.7 } else { threshold };
        Self { samples, threshold }
    }

    /// 动态添加 spam 样本
    #[allow(dead_code)]
    pub fn add_sample(&mut self, text: &str) {
        self.samples.push(TokenVector::from_text(text));
    }

    /// 计算消息与所有样本的最高相似度
    fn max_similarity(&self, text: &str) -> (f64, usize) {
        let msg_vec = TokenVector::from_text(text);
        let mut max_sim = 0.0f64;
        let mut best_idx = 0;
        for (i, sample) in self.samples.iter().enumerate() {
            let sim = msg_vec.cosine_similarity(sample);
            if sim > max_sim {
                max_sim = sim;
                best_idx = i;
            }
        }
        (max_sim, best_idx)
    }
}

#[async_trait]
impl Rule for SimilarityRule {
    fn name(&self) -> &'static str { "similarity" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }
        if self.samples.is_empty() {
            return None;
        }

        let (similarity, sample_idx) = self.max_similarity(&ctx.message_text);
        if similarity >= self.threshold {
            Some(ActionDecision::warn(
                &ctx.sender_id,
                &format!(
                    "Message similarity {:.0}% to known spam sample #{}",
                    similarity * 100.0,
                    sample_idx + 1,
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
    fn tokenize_basic() {
        let tokens = tokenize("Hello World! This is a test.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
        // "a" is too short (< 2 chars)
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn identical_texts_similarity_1() {
        let v1 = TokenVector::from_text("buy cheap bitcoin now");
        let v2 = TokenVector::from_text("buy cheap bitcoin now");
        let sim = v1.cosine_similarity(&v2);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn completely_different_similarity_0() {
        let v1 = TokenVector::from_text("hello world good morning");
        let v2 = TokenVector::from_text("xyz abc 123 456");
        let sim = v1.cosine_similarity(&v2);
        assert!(sim < 0.01);
    }

    #[test]
    fn similar_texts_high_score() {
        let v1 = TokenVector::from_text("buy cheap bitcoin investment opportunity");
        let v2 = TokenVector::from_text("buy bitcoin cheap investment now");
        let sim = v1.cosine_similarity(&v2);
        assert!(sim > 0.7);
    }

    #[test]
    fn empty_text_similarity_0() {
        let v1 = TokenVector::from_text("");
        let v2 = TokenVector::from_text("hello world");
        assert_eq!(v1.cosine_similarity(&v2), 0.0);
    }

    #[tokio::test]
    async fn similar_message_triggers() {
        let store = LocalStore::new();
        let rule = SimilarityRule::new(
            vec!["buy cheap bitcoin investment opportunity now".into()],
            0.6,
        );

        let ctx = make_ctx("buy bitcoin cheap investment opportunity");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn different_message_passes() {
        let store = LocalStore::new();
        let rule = SimilarityRule::new(
            vec!["buy cheap bitcoin investment".into()],
            0.7,
        );

        let ctx = make_ctx("good morning everyone, how is the weather today");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn no_samples_passes() {
        let store = LocalStore::new();
        let rule = SimilarityRule::new(vec![], 0.7);
        let ctx = make_ctx("anything");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn empty_message_skipped() {
        let store = LocalStore::new();
        let rule = SimilarityRule::new(vec!["spam".into()], 0.5);
        let ctx = make_ctx("");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn commands_skipped() {
        let store = LocalStore::new();
        let rule = SimilarityRule::new(vec!["spam".into()], 0.1);
        let mut ctx = make_ctx("spam");
        ctx.is_command = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn multiple_samples_best_match() {
        let store = LocalStore::new();
        let rule = SimilarityRule::new(
            vec![
                "earn money fast easy bitcoin".into(),
                "free gift card amazon click here".into(),
            ],
            0.5,
        );

        // Should match sample 2
        let ctx = make_ctx("free amazon gift card click link here");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
        let msg = result.unwrap().message.unwrap();
        assert!(msg.contains("sample #2"));
    }

    #[test]
    fn threshold_default_on_invalid() {
        let rule = SimilarityRule::new(vec![], 0.0);
        assert_eq!(rule.threshold, 0.7);
        let rule = SimilarityRule::new(vec![], 1.5);
        assert_eq!(rule.threshold, 0.7);
    }
}
