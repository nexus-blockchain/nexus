/// 共享文本处理工具 (M5: 统一 tokenize 函数, L3: 统一 html_escape)

/// HTML 转义: 防止通过模板变量注入 HTML 标签
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

/// 简单分词: 小写 + 按空白/标点分割 + 去除短 token
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello World!");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_filters_short() {
        let tokens = tokenize("I am a test");
        assert_eq!(tokens, vec!["am", "test"]);
    }

    #[test]
    fn tokenize_handles_punctuation() {
        let tokens = tokenize("buy,sell;trade-crypto!");
        assert_eq!(tokens, vec!["buy", "sell", "trade", "crypto"]);
    }

    #[test]
    fn tokenize_empty() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("  ").is_empty());
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("<script>alert(1)</script>"), "&lt;script&gt;alert(1)&lt;/script&gt;");
        assert_eq!(html_escape("a&b"), "a&amp;b");
        assert_eq!(html_escape(r#"say "hi""#), "say &quot;hi&quot;");
    }

    #[test]
    fn html_escape_safe_passthrough() {
        assert_eq!(html_escape("hello world"), "hello world");
        assert_eq!(html_escape("12345"), "12345");
    }
}
