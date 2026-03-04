use std::collections::HashSet;
use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// Anti-Phishing 规则
///
/// 检测消息中的钓鱼链接:
/// 1. 本地黑名单域名匹配
/// 2. 可疑 URL 模式检测 (仿冒知名域名)
/// 3. 链上黑名单同步 (任何 Nexus 节点举报后全网生效)
///
/// 设计参考:
/// - YAGPDB antiphishing/ 模块
/// - tg-spam CAS (Combot Anti-Spam) API
/// - Nexus 独特优势: 链上钓鱼 URL 黑名单，去中心化维护
pub struct AntiPhishingRule {
    /// 已知钓鱼域名黑名单
    blacklisted_domains: HashSet<String>,
    /// 受保护的合法域名 (用于检测仿冒)
    protected_domains: Vec<String>,
    /// URL 提取正则
    url_regex: regex::Regex,
}

impl AntiPhishingRule {
    pub fn new(blacklisted_domains: Vec<String>, protected_domains: Vec<String>) -> Self {
        let blacklisted_domains = blacklisted_domains.into_iter()
            .map(|d| d.to_lowercase())
            .collect();
        let protected_domains = protected_domains.into_iter()
            .map(|d| d.to_lowercase())
            .collect();
        Self {
            blacklisted_domains,
            protected_domains,
            url_regex: regex::Regex::new(
                r"(?i)https?://([^\s/<>\[\](){}]+)"
            ).expect("invalid URL regex"),
        }
    }

    /// 从链上黑名单 + 内置列表构建
    pub fn with_defaults() -> Self {
        Self::new(
            DEFAULT_PHISHING_DOMAINS.iter().map(|s| s.to_string()).collect(),
            DEFAULT_PROTECTED_DOMAINS.iter().map(|s| s.to_string()).collect(),
        )
    }

    /// 动态添加黑名单域名 (链上同步调用)
    pub fn add_blacklisted(&mut self, domain: &str) {
        self.blacklisted_domains.insert(domain.to_lowercase());
    }

    /// 检测 URL 中的域名
    fn check_url(&self, url_host: &str) -> Option<PhishingMatch> {
        let host = url_host.to_lowercase();

        // 1. 精确黑名单匹配
        if self.blacklisted_domains.contains(&host) {
            return Some(PhishingMatch {
                matched_domain: host,
                match_type: MatchType::Blacklisted,
            });
        }

        // 也检查父域名 (a.evil.com → evil.com)
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() >= 2 {
            let parent = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
            if self.blacklisted_domains.contains(&parent) {
                return Some(PhishingMatch {
                    matched_domain: parent,
                    match_type: MatchType::Blacklisted,
                });
            }
        }

        // 2. 仿冒域名检测 (Levenshtein 距离)
        for protected in &self.protected_domains {
            if host != *protected && is_lookalike(&host, protected) {
                return Some(PhishingMatch {
                    matched_domain: host.clone(),
                    match_type: MatchType::Lookalike(protected.clone()),
                });
            }
        }

        // 3. 可疑模式检测
        if has_suspicious_pattern(&host) {
            return Some(PhishingMatch {
                matched_domain: host,
                match_type: MatchType::SuspiciousPattern,
            });
        }

        None
    }
}

#[derive(Debug)]
struct PhishingMatch {
    matched_domain: String,
    match_type: MatchType,
}

#[derive(Debug)]
enum MatchType {
    /// 在黑名单中
    Blacklisted,
    /// 仿冒受保护域名
    Lookalike(String),
    /// 可疑 URL 模式
    SuspiciousPattern,
}

/// 检测域名是否为合法域名的仿冒版
/// 使用编辑距离 + 常见替换检测
fn is_lookalike(candidate: &str, protected: &str) -> bool {
    // 去掉 TLD 比较
    let c_base = candidate.split('.').next().unwrap_or(candidate);
    let p_base = protected.split('.').next().unwrap_or(protected);

    // 编辑距离 <= 2 视为仿冒
    if levenshtein(c_base, p_base) <= 2 && c_base != p_base {
        return true;
    }

    // 常见仿冒模式 (排除完全相同的 base)
    if c_base == p_base {
        return false;
    }
    let patterns = [
        // 域名包含受保护品牌名
        c_base.contains(p_base),
        // 前后加字符: e.g. "telegramm" vs "telegram"
        p_base.contains(c_base) && c_base.len() >= 4,
    ];

    patterns.iter().any(|&p| p)
}

/// 合法域名白名单: 主机名以这些域名结尾的不触发可疑模式检测
const PATTERN_WHITELIST: &[&str] = &[
    "google.com", "google.co",
    "microsoft.com", "microsoftonline.com", "live.com", "outlook.com",
    "apple.com", "icloud.com",
    "amazon.com", "aws.amazon.com",
    "github.com", "gitlab.com",
    "cloudflare.com",
    "telegram.org", "t.me",
    "discord.com", "discord.gg",
    "twitter.com", "x.com",
    "facebook.com", "instagram.com",
    "linkedin.com",
    "binance.com", "coinbase.com",
];

/// 可疑 URL 模式
fn has_suspicious_pattern(host: &str) -> bool {
    // IP 地址作为域名
    if host.chars().all(|c| c.is_ascii_digit() || c == '.') && host.contains('.') {
        return true;
    }
    // 超长子域名
    if host.split('.').any(|part| part.len() > 30) {
        return true;
    }
    // 白名单域名跳过可疑关键词检测
    for domain in PATTERN_WHITELIST {
        if host == *domain || host.ends_with(&format!(".{}", domain)) {
            return false;
        }
    }
    // 含 "login", "verify", "secure", "account" 等可疑路径关键词
    let suspicious_words = ["login", "verify", "secure", "account", "signin", "wallet-connect", "airdrop-claim"];
    suspicious_words.iter().any(|w| host.contains(w))
}

/// Levenshtein 编辑距离
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// 内置钓鱼域名黑名单 (常见 crypto/social 钓鱼域名)
const DEFAULT_PHISHING_DOMAINS: &[&str] = &[
    "metamask-verify.com",
    "wallet-connect-app.com",
    "opensea-nft-verify.com",
    "uniswap-claim.com",
    "pancakeswap-airdrop.com",
    "discord-nitro-free.com",
    "telegram-premium-free.com",
    "crypto-airdrop-claim.com",
    "binance-support-help.com",
    "coinbase-verify-account.com",
];

/// 受保护的合法域名 (检测仿冒)
const DEFAULT_PROTECTED_DOMAINS: &[&str] = &[
    "telegram.org",
    "discord.com",
    "metamask.io",
    "opensea.io",
    "uniswap.org",
    "binance.com",
    "coinbase.com",
    "ethereum.org",
    "bitcoin.org",
    "github.com",
];

#[async_trait]
impl Rule for AntiPhishingRule {
    fn name(&self) -> &'static str { "antiphishing" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_command || ctx.is_join_request || ctx.message_text.is_empty() {
            return None;
        }

        // 提取所有 URL 的 host 部分
        for caps in self.url_regex.captures_iter(&ctx.message_text) {
            if let Some(host_match) = caps.get(1) {
                let host = host_match.as_str();
                // 去掉路径部分，只保留域名
                let domain = host.split('/').next().unwrap_or(host);
                // 去掉端口号
                let domain = domain.split(':').next().unwrap_or(domain);

                if let Some(phishing) = self.check_url(domain) {
                    let reason = match &phishing.match_type {
                        MatchType::Blacklisted => format!(
                            "⚠️ Phishing link detected: {} (blacklisted domain)",
                            phishing.matched_domain,
                        ),
                        MatchType::Lookalike(real) => format!(
                            "⚠️ Suspicious link: {} looks like {} (possible phishing)",
                            phishing.matched_domain, real,
                        ),
                        MatchType::SuspiciousPattern => format!(
                            "⚠️ Suspicious URL pattern: {}",
                            phishing.matched_domain,
                        ),
                    };
                    return Some(ActionDecision::warn(&ctx.sender_id, &reason));
                }
            }
        }
        None
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
    fn levenshtein_distance() {
        assert_eq!(levenshtein("telegram", "telegramm"), 1);
        assert_eq!(levenshtein("telegram", "teIegram"), 1); // l→I
        assert_eq!(levenshtein("discord", "d1scord"), 1);
        assert_eq!(levenshtein("abc", "xyz"), 3);
    }

    #[test]
    fn lookalike_detection() {
        assert!(is_lookalike("telegramm.org", "telegram.org"));
        assert!(is_lookalike("d1scord.com", "discord.com"));
        assert!(!is_lookalike("telegram.org", "telegram.org")); // exact match
        assert!(!is_lookalike("google.com", "discord.com")); // too different
    }

    #[test]
    fn suspicious_patterns() {
        assert!(has_suspicious_pattern("192.168.1.1"));
        assert!(has_suspicious_pattern("login-verify.example.com"));
        assert!(has_suspicious_pattern("wallet-connect-verify.com"));
        assert!(!has_suspicious_pattern("example.com"));
        assert!(!has_suspicious_pattern("github.com"));
    }

    #[test]
    fn whitelisted_domains_not_suspicious() {
        // 合法域名包含可疑关键词但在白名单中
        assert!(!has_suspicious_pattern("account.google.com"));
        assert!(!has_suspicious_pattern("login.microsoftonline.com"));
        assert!(!has_suspicious_pattern("secure.amazon.com"));
        assert!(!has_suspicious_pattern("signin.aws.amazon.com"));
        // 非白名单域名仍然触发
        assert!(has_suspicious_pattern("login-evil.xyz"));
        assert!(has_suspicious_pattern("account-verify.phishing.com"));
    }

    #[tokio::test]
    async fn blacklisted_domain_triggers() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();

        let ctx = make_ctx("check out https://metamask-verify.com/claim");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
        assert!(result.unwrap().message.unwrap().contains("blacklisted"));
    }

    #[tokio::test]
    async fn lookalike_domain_triggers() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();

        let ctx = make_ctx("visit https://telegramm.org/premium");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
        assert!(result.unwrap().message.unwrap().contains("looks like"));
    }

    #[tokio::test]
    async fn suspicious_url_triggers() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();

        let ctx = make_ctx("go to https://login-verify-account.xyz/claim");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn legitimate_url_passes() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();

        let ctx = make_ctx("check https://telegram.org/blog for news");
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn no_url_passes() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();
        assert!(rule.evaluate(&make_ctx("just a normal message"), &store).await.is_none());
    }

    #[tokio::test]
    async fn empty_message_skipped() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();
        assert!(rule.evaluate(&make_ctx(""), &store).await.is_none());
    }

    #[tokio::test]
    async fn commands_skipped() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();
        let mut ctx = make_ctx("https://metamask-verify.com");
        ctx.is_command = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[test]
    fn add_blacklisted_domain() {
        let mut rule = AntiPhishingRule::with_defaults();
        rule.add_blacklisted("new-phishing-site.com");
        assert!(rule.blacklisted_domains.contains("new-phishing-site.com"));
    }

    #[tokio::test]
    async fn subdomain_of_blacklisted() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();

        // 子域名匹配父域名黑名单
        let ctx = make_ctx("go to https://app.metamask-verify.com/login");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn ip_address_url_suspicious() {
        let store = LocalStore::new();
        let rule = AntiPhishingRule::with_defaults();

        let ctx = make_ctx("visit http://192.168.1.100/phishing");
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
    }
}
