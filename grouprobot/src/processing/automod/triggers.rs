use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;

/// 触发器检查结果
#[derive(Debug, Clone)]
pub struct TriggerResult {
    pub trigger_name: String,
    pub details: String,
}

/// 触发器 Trait — 什么时候触发规则
pub trait Trigger: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, ctx: &MessageContext, store: &LocalStore) -> Option<TriggerResult>;
}

// ── 具体触发器实现 ──

/// 关键词列表触发器 (黑名单/白名单)
pub struct WordListTrigger {
    words: Vec<String>,
    is_blacklist: bool,
}

impl WordListTrigger {
    pub fn blacklist(words: Vec<String>) -> Self {
        let words = words.into_iter().map(|w| w.to_lowercase()).collect();
        Self { words, is_blacklist: true }
    }

    #[allow(dead_code)]
    pub fn whitelist(words: Vec<String>) -> Self {
        let words = words.into_iter().map(|w| w.to_lowercase()).collect();
        Self { words, is_blacklist: false }
    }
}

impl Trigger for WordListTrigger {
    fn name(&self) -> &'static str {
        if self.is_blacklist { "word_blacklist" } else { "word_whitelist" }
    }

    fn check(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<TriggerResult> {
        let lower = ctx.message_text.to_lowercase();
        for word in &self.words {
            let matched = lower.contains(word.as_str());
            if self.is_blacklist && matched {
                return Some(TriggerResult {
                    trigger_name: self.name().into(),
                    details: format!("Matched blacklist word: \"{}\"", word),
                });
            }
            if !self.is_blacklist && !matched {
                // Whitelist: trigger if NOT in list (message doesn't contain required word)
                // This is typically used with conditions, skip for now
            }
        }
        None
    }
}

/// 链接触发器 (任意链接 / 特定域名)
pub struct LinkTrigger {
    url_regex: regex::Regex,
    blocked_domains: Vec<String>,
}

impl LinkTrigger {
    pub fn any_link() -> Self {
        Self {
            url_regex: regex::Regex::new(
                r"(?i)https?://[^\s<>\[\](){}]+|www\.[^\s<>\[\](){}]+"
            ).expect("invalid URL regex"),
            blocked_domains: vec![],
        }
    }

    pub fn with_blocked_domains(domains: Vec<String>) -> Self {
        let domains = domains.into_iter().map(|d| d.to_lowercase()).collect();
        Self {
            url_regex: regex::Regex::new(
                r"(?i)https?://[^\s<>\[\](){}]+|www\.[^\s<>\[\](){}]+"
            ).expect("invalid URL regex"),
            blocked_domains: domains,
        }
    }
}

impl Trigger for LinkTrigger {
    fn name(&self) -> &'static str { "link" }

    fn check(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<TriggerResult> {
        for mat in self.url_regex.find_iter(&ctx.message_text) {
            let url = mat.as_str().to_lowercase();

            if self.blocked_domains.is_empty() {
                // Any link triggers
                return Some(TriggerResult {
                    trigger_name: self.name().into(),
                    details: format!("Contains link: {}", mat.as_str()),
                });
            }

            // Check specific blocked domains
            for domain in &self.blocked_domains {
                if url.contains(domain.as_str()) {
                    return Some(TriggerResult {
                        trigger_name: self.name().into(),
                        details: format!("Blocked domain: {} in {}", domain, mat.as_str()),
                    });
                }
            }
        }
        None
    }
}

/// @提及过多触发器
pub struct MentionsTrigger {
    max_mentions: u16,
}

impl MentionsTrigger {
    pub fn new(max_mentions: u16) -> Self {
        Self { max_mentions }
    }
}

impl Trigger for MentionsTrigger {
    fn name(&self) -> &'static str { "mentions" }

    fn check(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<TriggerResult> {
        let count = ctx.message_text.matches('@').count();
        if count > self.max_mentions as usize {
            Some(TriggerResult {
                trigger_name: self.name().into(),
                details: format!("Too many mentions: {} (limit: {})", count, self.max_mentions),
            })
        } else {
            None
        }
    }
}

/// 消息频率触发器 (慢速模式)
pub struct SlowmodeTrigger {
    max_messages: u16,
    window_secs: u64,
}

impl SlowmodeTrigger {
    pub fn new(max_messages: u16, window_secs: u64) -> Self {
        Self { max_messages, window_secs }
    }
}

impl Trigger for SlowmodeTrigger {
    fn name(&self) -> &'static str { "slowmode" }

    fn check(&self, ctx: &MessageContext, store: &LocalStore) -> Option<TriggerResult> {
        let key = format!("slowmode:{}:{}", ctx.group_id, ctx.sender_id);
        let count = store.increment_counter(&key, self.window_secs);
        if count > self.max_messages as u64 {
            Some(TriggerResult {
                trigger_name: self.name().into(),
                details: format!("Slowmode: {} msgs in {}s (limit: {})", count, self.window_secs, self.max_messages),
            })
        } else {
            None
        }
    }
}

/// 消息长度触发器
pub struct MessageLengthTrigger {
    min_length: Option<usize>,
    max_length: Option<usize>,
}

impl MessageLengthTrigger {
    pub fn max(max_length: usize) -> Self {
        Self { min_length: None, max_length: Some(max_length) }
    }

    #[allow(dead_code)]
    pub fn min(min_length: usize) -> Self {
        Self { min_length: Some(min_length), max_length: None }
    }
}

impl Trigger for MessageLengthTrigger {
    fn name(&self) -> &'static str { "message_length" }

    fn check(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<TriggerResult> {
        let len = ctx.message_text.chars().count();
        if let Some(max) = self.max_length {
            if len > max {
                return Some(TriggerResult {
                    trigger_name: self.name().into(),
                    details: format!("Message too long: {} chars (max: {})", len, max),
                });
            }
        }
        if let Some(min) = self.min_length {
            if len < min && len > 0 {
                return Some(TriggerResult {
                    trigger_name: self.name().into(),
                    details: format!("Message too short: {} chars (min: {})", len, min),
                });
            }
        }
        None
    }
}

/// 新成员入群触发器
pub struct MemberJoinTrigger;

impl Trigger for MemberJoinTrigger {
    fn name(&self) -> &'static str { "member_join" }

    fn check(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<TriggerResult> {
        if ctx.is_join_request {
            Some(TriggerResult {
                trigger_name: self.name().into(),
                details: format!("New member join: {}", ctx.sender_name),
            })
        } else {
            None
        }
    }
}

/// 重复消息触发器
pub struct DuplicateMessageTrigger {
    window_secs: u64,
    threshold: u16,
}

impl DuplicateMessageTrigger {
    pub fn new(window_secs: u64, threshold: u16) -> Self {
        Self {
            window_secs: if window_secs == 0 { 300 } else { window_secs },
            threshold: if threshold == 0 { 3 } else { threshold },
        }
    }
}

impl Trigger for DuplicateMessageTrigger {
    fn name(&self) -> &'static str { "duplicate_message" }

    fn check(&self, ctx: &MessageContext, store: &LocalStore) -> Option<TriggerResult> {
        if ctx.message_text.is_empty() {
            return None;
        }
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        ctx.message_text.trim().to_lowercase().hash(&mut hasher);
        let text_hash = hasher.finish();

        let count = store.record_message_hash(&ctx.group_id, &ctx.sender_id, text_hash, self.window_secs);
        if count >= self.threshold as u64 {
            Some(TriggerResult {
                trigger_name: self.name().into(),
                details: format!("Duplicate: sent {} times in {}s", count, self.window_secs),
            })
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
            message_text: text.into(),
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
    fn word_blacklist_matches() {
        let store = LocalStore::new();
        let t = WordListTrigger::blacklist(vec!["scam".into(), "spam".into()]);
        assert!(t.check(&make_ctx("this is a SCAM"), &store).is_some());
        assert!(t.check(&make_ctx("normal message"), &store).is_none());
    }

    #[test]
    fn link_any_triggers() {
        let store = LocalStore::new();
        let t = LinkTrigger::any_link();
        assert!(t.check(&make_ctx("visit https://evil.com"), &store).is_some());
        assert!(t.check(&make_ctx("no links here"), &store).is_none());
    }

    #[test]
    fn link_blocked_domain() {
        let store = LocalStore::new();
        let t = LinkTrigger::with_blocked_domains(vec!["evil.com".into()]);
        assert!(t.check(&make_ctx("see https://evil.com/page"), &store).is_some());
        assert!(t.check(&make_ctx("see https://good.com"), &store).is_none());
    }

    #[test]
    fn mentions_trigger() {
        let store = LocalStore::new();
        let t = MentionsTrigger::new(2);
        assert!(t.check(&make_ctx("@a @b @c"), &store).is_some());
        assert!(t.check(&make_ctx("@a @b"), &store).is_none());
    }

    #[test]
    fn slowmode_trigger() {
        let store = LocalStore::new();
        let t = SlowmodeTrigger::new(2, 60);
        let ctx = make_ctx("hi");
        assert!(t.check(&ctx, &store).is_none()); // 1
        assert!(t.check(&ctx, &store).is_none()); // 2
        assert!(t.check(&ctx, &store).is_some()); // 3 > limit
    }

    #[test]
    fn message_length_max() {
        let store = LocalStore::new();
        let t = MessageLengthTrigger::max(10);
        assert!(t.check(&make_ctx("short"), &store).is_none());
        assert!(t.check(&make_ctx("this is a very long message exceeding limit"), &store).is_some());
    }

    #[test]
    fn member_join_trigger() {
        let store = LocalStore::new();
        let t = MemberJoinTrigger;
        let mut ctx = make_ctx("");
        assert!(t.check(&ctx, &store).is_none());
        ctx.is_join_request = true;
        assert!(t.check(&ctx, &store).is_some());
    }

    #[test]
    fn duplicate_trigger() {
        let store = LocalStore::new();
        let t = DuplicateMessageTrigger::new(300, 2);
        let ctx = make_ctx("repeat me");
        assert!(t.check(&ctx, &store).is_none()); // 1
        assert!(t.check(&ctx, &store).is_some()); // 2 >= threshold
    }
}
