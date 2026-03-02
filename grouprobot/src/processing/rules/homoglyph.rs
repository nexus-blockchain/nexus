use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G28: 同形字/混淆字符标准化规则
///
/// 将 Unicode 混淆字符 (Cyrillic, Greek, fullwidth, etc.) 标准化为 ASCII 后,
/// 再进行关键词匹配。防止用户通过 "Ĥéĺĺó" 绕过 "hello" 的词汇过滤。
///
/// 工作方式:
/// 1. 对消息文本进行同形字标准化
/// 2. 用标准化后的文本匹配被禁关键词列表
/// 3. 匹配则执行动作 (warn/delete)
///
/// 此规则增强 BlacklistRule 和 StopWordRule 的检测能力。
pub struct HomoglyphRule {
    /// 被禁关键词 (已标准化为小写 ASCII)
    keywords: Vec<String>,
}

impl HomoglyphRule {
    pub fn new(keywords: Vec<String>) -> Self {
        let keywords: Vec<String> = keywords.into_iter()
            .map(|k| Self::normalize(&k.to_lowercase()))
            .filter(|k| !k.is_empty())
            .collect();
        Self { keywords }
    }

    /// 从 CSV 加载
    pub fn from_csv(csv: &str) -> Self {
        let keywords: Vec<String> = csv.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self::new(keywords)
    }

    /// 同形字标准化: 将常见 Unicode 混淆字符映射到 ASCII
    pub fn normalize(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for ch in text.chars() {
            result.push(normalize_char(ch));
        }
        result
    }

    /// 关键词数
    pub fn count(&self) -> usize {
        self.keywords.len()
    }

    /// 检查标准化后的文本是否包含被禁关键词
    fn check_text(&self, text: &str) -> Option<&str> {
        let normalized = Self::normalize(&text.to_lowercase());
        // 按单词边界分割
        let words: Vec<&str> = normalized
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();

        for keyword in &self.keywords {
            // 单词匹配
            for word in &words {
                if *word == keyword.as_str() {
                    return Some(keyword);
                }
            }
            // 子串匹配 (多词关键词)
            if normalized.contains(keyword.as_str()) {
                return Some(keyword);
            }
        }
        None
    }
}

/// 单字符同形字映射
/// 覆盖: Cyrillic, Greek, fullwidth Latin, mathematical symbols, etc.
fn normalize_char(ch: char) -> char {
    match ch {
        // ── Cyrillic → Latin ──
        'А' | 'а' => 'a',
        'В' | 'в' => 'b',  // Cyrillic В looks like B
        'С' | 'с' => 'c',
        'Е' | 'е' => 'e',
        'Н' | 'н' => 'h',  // Cyrillic Н looks like H
        'І' | 'і' => 'i',  // Ukrainian І
        'К' | 'к' => 'k',
        'М' | 'м' => 'm',
        'О' | 'о' => 'o',
        'Р' | 'р' => 'p',
        'Т' | 'т' => 't',
        'Х' | 'х' => 'x',
        'У' | 'у' => 'y',
        // ── Greek → Latin ──
        'Α' | 'α' => 'a',
        'Β' | 'β' => 'b',
        'Ε' | 'ε' => 'e',
        'Η' | 'η' => 'h',
        'Ι' | 'ι' => 'i',
        'Κ' | 'κ' => 'k',
        'Μ' | 'μ' => 'm',
        'Ν' | 'ν' => 'n',
        'Ο' | 'ο' => 'o',
        'Ρ' | 'ρ' => 'p',
        'Τ' | 'τ' => 't',
        'Χ' | 'χ' => 'x',
        'Υ' | 'υ' => 'y',
        'Ζ' | 'ζ' => 'z',
        // ── Fullwidth → ASCII ──
        'Ａ'..='Ｚ' => ((ch as u32 - 'Ａ' as u32) as u8 + b'a') as char,
        'ａ'..='ｚ' => ((ch as u32 - 'ａ' as u32) as u8 + b'a') as char,
        '０'..='９' => ((ch as u32 - '０' as u32) as u8 + b'0') as char,
        // ── Accented Latin → base ──
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' | 'Ē' | 'Ė' | 'Ę' | 'Ě' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'ī' | 'į' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' | 'Ī' | 'Į' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' | 'ő' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' | 'Ō' | 'Ő' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' | 'Ū' | 'Ů' | 'Ű' => 'u',
        'ý' | 'ÿ' => 'y',
        'Ý' | 'Ÿ' => 'y',
        'ñ' | 'ń' | 'ň' => 'n',
        'Ñ' | 'Ń' | 'Ň' => 'n',
        'ç' | 'ć' | 'č' => 'c',
        'Ç' | 'Ć' | 'Č' => 'c',
        'ś' | 'š' | 'ş' => 's',
        'Ś' | 'Š' | 'Ş' => 's',
        'ź' | 'ž' | 'ż' => 'z',
        'Ź' | 'Ž' | 'Ż' => 'z',
        'đ' | 'ð' => 'd',
        'Đ' | 'Ð' => 'd',
        'ĺ' | 'ľ' | 'ł' => 'l',
        'Ĺ' | 'Ľ' | 'Ł' => 'l',
        'ŕ' | 'ř' => 'r',
        'Ŕ' | 'Ř' => 'r',
        'ť' | 'ţ' => 't',
        'Ť' | 'Ţ' => 't',
        'ĥ' => 'h',
        'Ĥ' => 'h',
        'ğ' => 'g',
        'Ğ' => 'g',
        // ── Leet speak / Symbol → letter ──
        // (已在 ProfanityRule 中处理, 这里保持纯 Unicode 标准化)
        // 默认: 保持原样
        other => other,
    }
}

#[async_trait]
impl Rule for HomoglyphRule {
    fn name(&self) -> &'static str { "homoglyph" }

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
        if ctx.message_text.is_empty() || self.keywords.is_empty() {
            return None;
        }

        if let Some(matched) = self.check_text(&ctx.message_text) {
            let reason = format!("Homoglyph filter: 标准化后匹配 '{}'", matched);
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
    fn normalize_cyrillic() {
        // "sсаm" with Cyrillic с and а
        assert_eq!(HomoglyphRule::normalize("sсаm"), "scam");
    }

    #[test]
    fn normalize_accented() {
        assert_eq!(HomoglyphRule::normalize("Ĥéĺĺó"), "hello");
    }

    #[test]
    fn normalize_fullwidth() {
        assert_eq!(HomoglyphRule::normalize("ｈｅｌｌｏ"), "hello");
    }

    #[test]
    fn normalize_greek() {
        // "sοαm" with Greek ο and α
        assert_eq!(HomoglyphRule::normalize("sοαm"), "soam");
    }

    #[test]
    fn normalize_mixed() {
        // Mix of Cyrillic а, accented é, fullwidth ｌ
        assert_eq!(HomoglyphRule::normalize("héｌｌо"), "hello");
    }

    #[test]
    fn normalize_plain_ascii() {
        assert_eq!(HomoglyphRule::normalize("hello world"), "hello world");
    }

    #[tokio::test]
    async fn detects_cyrillic_bypass() {
        let store = LocalStore::new();
        let rule = HomoglyphRule::new(vec!["scam".into()]);
        // "sсаm" → normalized → "scam"
        let d = rule.evaluate(&make_ctx("this is а sсаm"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn detects_accented_bypass() {
        let store = LocalStore::new();
        let rule = HomoglyphRule::new(vec!["hello".into()]);
        let d = rule.evaluate(&make_ctx("Ĥéĺĺó world"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn detects_fullwidth_bypass() {
        let store = LocalStore::new();
        let rule = HomoglyphRule::new(vec!["spam".into()]);
        let d = rule.evaluate(&make_ctx("ｓｐａｍ here"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn no_match_passes() {
        let store = LocalStore::new();
        let rule = HomoglyphRule::new(vec!["badword".into()]);
        assert!(rule.evaluate(&make_ctx("hello world"), &store).await.is_none());
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = HomoglyphRule::new(vec!["scam".into()]);
        let mut ctx = make_ctx("sсаm");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn from_csv_works() {
        let rule = HomoglyphRule::from_csv("scam, spam, hack");
        assert_eq!(rule.count(), 3);
    }

    #[tokio::test]
    async fn empty_keywords_passes() {
        let store = LocalStore::new();
        let rule = HomoglyphRule::new(vec![]);
        assert!(rule.evaluate(&make_ctx("anything"), &store).await.is_none());
    }
}
