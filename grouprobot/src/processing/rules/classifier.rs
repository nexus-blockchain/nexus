use std::collections::HashMap;
use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;
use super::text_utils::tokenize;

/// 朴素贝叶斯垃圾消息分类器
///
/// 基于 Multinomial Naive Bayes 算法:
/// - 训练阶段: 从 spam/ham 样本中学习词频分布
/// - 推理阶段: 计算 P(spam|words) vs P(ham|words)
/// - 模型文件密封在 TEE Enclave 中，防止外部篡改
///
/// 设计参考: tg-spam 的 classifier 模块
/// Phase 3 实现: 完整的训练 + 推理 + 在线学习
pub struct BayesClassifierRule {
    model: BayesModel,
    /// 分类阈值 (spam 概率超过此值则标记)
    threshold: f64,
}

/// 朴素贝叶斯模型
#[derive(Debug, Clone)]
pub struct BayesModel {
    /// spam 类词频: word → count
    spam_word_counts: HashMap<String, u64>,
    /// ham 类词频: word → count
    ham_word_counts: HashMap<String, u64>,
    /// spam 样本总数
    spam_doc_count: u64,
    /// ham 样本总数
    ham_doc_count: u64,
    /// spam 类总词数
    spam_total_words: u64,
    /// ham 类总词数
    ham_total_words: u64,
    /// 词汇表大小 (用于 Laplace 平滑)
    vocab_size: u64,
}

impl BayesModel {
    pub fn new() -> Self {
        Self {
            spam_word_counts: HashMap::new(),
            ham_word_counts: HashMap::new(),
            spam_doc_count: 0,
            ham_doc_count: 0,
            spam_total_words: 0,
            ham_total_words: 0,
            vocab_size: 0,
        }
    }

    /// 从训练数据构建模型
    pub fn train(spam_samples: &[&str], ham_samples: &[&str]) -> Self {
        let mut model = Self::new();
        for sample in spam_samples {
            model.train_one(sample, true);
        }
        for sample in ham_samples {
            model.train_one(sample, false);
        }
        model.update_vocab_size();
        model
    }

    /// 在线学习: 增量训练一条样本
    pub fn train_one(&mut self, text: &str, is_spam: bool) {
        let tokens = tokenize(text);
        if is_spam {
            self.spam_doc_count += 1;
            for token in &tokens {
                *self.spam_word_counts.entry(token.clone()).or_default() += 1;
                self.spam_total_words += 1;
            }
        } else {
            self.ham_doc_count += 1;
            for token in &tokens {
                *self.ham_word_counts.entry(token.clone()).or_default() += 1;
                self.ham_total_words += 1;
            }
        }
    }

    fn update_vocab_size(&mut self) {
        let mut vocab: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for k in self.spam_word_counts.keys() {
            vocab.insert(k.as_str());
        }
        for k in self.ham_word_counts.keys() {
            vocab.insert(k.as_str());
        }
        self.vocab_size = vocab.len() as u64;
    }

    /// 预测: 返回 (spam 概率, ham 概率)
    /// 使用 log 概率避免下溢
    pub fn predict(&self, text: &str) -> (f64, f64) {
        let total_docs = self.spam_doc_count + self.ham_doc_count;
        if total_docs == 0 {
            return (0.5, 0.5);
        }

        let tokens = tokenize(text);
        let vocab = if self.vocab_size == 0 { 1 } else { self.vocab_size };

        // log 先验概率
        let log_prior_spam = (self.spam_doc_count as f64 / total_docs as f64).ln();
        let log_prior_ham = (self.ham_doc_count as f64 / total_docs as f64).ln();

        // log 似然 (Laplace 平滑: +1 / +vocab_size)
        let mut log_likelihood_spam = 0.0f64;
        let mut log_likelihood_ham = 0.0f64;

        for token in &tokens {
            let spam_count = self.spam_word_counts.get(token).copied().unwrap_or(0);
            let ham_count = self.ham_word_counts.get(token).copied().unwrap_or(0);

            log_likelihood_spam += ((spam_count as f64 + 1.0) / (self.spam_total_words as f64 + vocab as f64)).ln();
            log_likelihood_ham += ((ham_count as f64 + 1.0) / (self.ham_total_words as f64 + vocab as f64)).ln();
        }

        let log_spam = log_prior_spam + log_likelihood_spam;
        let log_ham = log_prior_ham + log_likelihood_ham;

        // 转换为概率 (softmax)
        let max_log = log_spam.max(log_ham);
        let exp_spam = (log_spam - max_log).exp();
        let exp_ham = (log_ham - max_log).exp();
        let total = exp_spam + exp_ham;

        (exp_spam / total, exp_ham / total)
    }

    /// 返回 spam 概率
    pub fn spam_probability(&self, text: &str) -> f64 {
        self.predict(text).0
    }

    /// 模型是否已训练 (至少有 spam 和 ham 各一条)
    pub fn is_trained(&self) -> bool {
        self.spam_doc_count > 0 && self.ham_doc_count > 0
    }

    /// 模型统计
    pub fn stats(&self) -> ModelStats {
        ModelStats {
            spam_docs: self.spam_doc_count,
            ham_docs: self.ham_doc_count,
            vocab_size: self.vocab_size,
            spam_words: self.spam_total_words,
            ham_words: self.ham_total_words,
        }
    }

    /// 序列化为 JSON (用于密封存储在 TEE 中)
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "spam_word_counts": self.spam_word_counts,
            "ham_word_counts": self.ham_word_counts,
            "spam_doc_count": self.spam_doc_count,
            "ham_doc_count": self.ham_doc_count,
            "spam_total_words": self.spam_total_words,
            "ham_total_words": self.ham_total_words,
            "vocab_size": self.vocab_size,
        }).to_string()
    }

    /// 从 JSON 反序列化
    pub fn from_json(json: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        Some(Self {
            spam_word_counts: serde_json::from_value(v.get("spam_word_counts")?.clone()).ok()?,
            ham_word_counts: serde_json::from_value(v.get("ham_word_counts")?.clone()).ok()?,
            spam_doc_count: v.get("spam_doc_count")?.as_u64()?,
            ham_doc_count: v.get("ham_doc_count")?.as_u64()?,
            spam_total_words: v.get("spam_total_words")?.as_u64()?,
            ham_total_words: v.get("ham_total_words")?.as_u64()?,
            vocab_size: v.get("vocab_size")?.as_u64()?,
        })
    }
}

/// 模型统计信息
#[derive(Debug)]
pub struct ModelStats {
    pub spam_docs: u64,
    pub ham_docs: u64,
    pub vocab_size: u64,
    pub spam_words: u64,
    pub ham_words: u64,
}

impl BayesClassifierRule {
    pub fn new(model: BayesModel, threshold: f64) -> Self {
        let threshold = if threshold <= 0.0 || threshold > 1.0 { 0.8 } else { threshold };
        Self { model, threshold }
    }

    /// 从训练数据直接构建
    pub fn from_training_data(spam: &[&str], ham: &[&str], threshold: f64) -> Self {
        let model = BayesModel::train(spam, ham);
        Self::new(model, threshold)
    }
}

#[async_trait]
impl Rule for BayesClassifierRule {
    fn name(&self) -> &'static str { "bayes_classifier" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }
        if !self.model.is_trained() {
            return None;
        }

        let spam_prob = self.model.spam_probability(&ctx.message_text);
        if spam_prob >= self.threshold {
            Some(ActionDecision::warn(
                &ctx.sender_id,
                &format!(
                    "Bayes classifier: {:.0}% spam probability (threshold: {:.0}%)",
                    spam_prob * 100.0,
                    self.threshold * 100.0,
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

    fn spam_samples() -> Vec<&'static str> {
        vec![
            "buy cheap bitcoin investment now limited offer",
            "free gift card amazon click link here claim prize",
            "earn money fast easy guaranteed profit bitcoin crypto",
            "congratulations you won lottery claim your prize now",
            "investment opportunity guaranteed returns crypto trading",
            "click here free money bitcoin doubling guaranteed profit",
            "limited time offer buy now discount investment crypto",
        ]
    }

    fn ham_samples() -> Vec<&'static str> {
        vec![
            "good morning everyone how is the weather today",
            "does anyone know a good restaurant nearby",
            "the meeting is scheduled for tomorrow afternoon",
            "happy birthday hope you have a great day",
            "can someone help me with this programming question",
            "i just finished reading a really good book",
            "the new movie was pretty entertaining actually",
            "what time does the store close on weekends",
            "lets organize a team event next month",
            "the project deadline has been extended by one week",
        ]
    }

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

    #[test]
    fn train_and_predict_spam() {
        let model = BayesModel::train(&spam_samples(), &ham_samples());
        assert!(model.is_trained());

        let prob = model.spam_probability("buy cheap bitcoin investment guaranteed profit");
        assert!(prob > 0.8, "spam should have high probability, got {}", prob);
    }

    #[test]
    fn train_and_predict_ham() {
        let model = BayesModel::train(&spam_samples(), &ham_samples());

        let prob = model.spam_probability("good morning how are you today");
        assert!(prob < 0.3, "ham should have low spam probability, got {}", prob);
    }

    #[test]
    fn untrained_model_returns_neutral() {
        let model = BayesModel::new();
        assert!(!model.is_trained());
        let (spam, ham) = model.predict("anything");
        assert!((spam - 0.5).abs() < 0.01);
        assert!((ham - 0.5).abs() < 0.01);
    }

    #[test]
    fn online_learning() {
        let mut model = BayesModel::new();
        for s in &spam_samples() { model.train_one(s, true); }
        for s in &ham_samples() { model.train_one(s, false); }
        model.update_vocab_size();

        assert!(model.is_trained());
        let prob = model.spam_probability("free bitcoin click here guaranteed");
        assert!(prob > 0.7);
    }

    #[test]
    fn model_stats() {
        let model = BayesModel::train(&spam_samples(), &ham_samples());
        let stats = model.stats();
        assert_eq!(stats.spam_docs, 7);
        assert_eq!(stats.ham_docs, 10);
        assert!(stats.vocab_size > 20);
        assert!(stats.spam_words > 0);
        assert!(stats.ham_words > 0);
    }

    #[test]
    fn serialize_deserialize() {
        let model = BayesModel::train(&spam_samples(), &ham_samples());
        let json = model.to_json();

        let restored = BayesModel::from_json(&json).unwrap();
        assert_eq!(restored.spam_doc_count, model.spam_doc_count);
        assert_eq!(restored.ham_doc_count, model.ham_doc_count);
        assert_eq!(restored.vocab_size, model.vocab_size);

        // 预测结果应一致
        let text = "buy bitcoin cheap";
        let (p1, _) = model.predict(text);
        let (p2, _) = restored.predict(text);
        assert!((p1 - p2).abs() < 0.001);
    }

    #[test]
    fn invalid_json_returns_none() {
        assert!(BayesModel::from_json("not json").is_none());
        assert!(BayesModel::from_json("{}").is_none());
    }

    #[tokio::test]
    async fn classifier_rule_triggers_on_spam() {
        let store = LocalStore::new();
        let rule = BayesClassifierRule::from_training_data(
            &spam_samples(), &ham_samples(), 0.7,
        );

        let ctx = make_ctx("buy cheap bitcoin investment guaranteed profit now");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn classifier_rule_passes_ham() {
        let store = LocalStore::new();
        let rule = BayesClassifierRule::from_training_data(
            &spam_samples(), &ham_samples(), 0.7,
        );

        let ctx = make_ctx("good morning everyone how is the weather");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn classifier_skips_commands() {
        let store = LocalStore::new();
        let rule = BayesClassifierRule::from_training_data(
            &spam_samples(), &ham_samples(), 0.1,
        );

        let mut ctx = make_ctx("buy bitcoin");
        ctx.is_command = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn untrained_model_skips() {
        let store = LocalStore::new();
        let rule = BayesClassifierRule::new(BayesModel::new(), 0.5);
        assert!(rule.evaluate(&make_ctx("anything"), &store).await.is_none());
    }

    #[test]
    fn threshold_default_on_invalid() {
        let rule = BayesClassifierRule::new(BayesModel::new(), 0.0);
        assert_eq!(rule.threshold, 0.8);
        let rule = BayesClassifierRule::new(BayesModel::new(), 1.5);
        assert_eq!(rule.threshold, 0.8);
    }

    #[test]
    fn tokenize_handles_special_chars() {
        let tokens = tokenize("Hello, World! Buy $$$bitcoin$$$ NOW!!!");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"bitcoin".to_string()));
        assert!(tokens.contains(&"now".to_string()));
    }
}
