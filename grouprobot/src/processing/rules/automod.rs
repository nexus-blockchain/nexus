use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G19: 可组合自动审核引擎
///
/// 用户通过链上 JSON 配置定义自定义审核规则:
///   触发器 (Trigger) → 条件 (Condition) → 效果 (Effect)
///
/// 配置格式 (JSON 数组, 存储在 `automod_rules_json` 字段):
/// ```json
/// [
///   {
///     "trigger": "message",          // message | join | regex:<pattern>
///     "conditions": ["has_link", "is_new_member:10"],  // 条件组合 (AND)
///     "effect": "delete",            // delete | warn:<msg> | mute:<secs> | ban | kick
///     "name": "新成员禁止发链接"
///   }
/// ]
/// ```
///
/// 支持的触发器:
/// - `message` — 所有普通消息
/// - `join` — 新成员入群
/// - `regex:<pattern>` — 正则匹配消息内容
///
/// 支持的条件:
/// - `has_link` — 消息包含链接
/// - `has_media` — 消息包含媒体 (photo/video/document)
/// - `is_new_member:<N>` — 用户消息计数 < N (新成员)
/// - `text_longer:<N>` — 消息长度 > N
/// - `has_mention:<N>` — @mention 数量 > N
/// - `text_matches:<keyword>` — 消息包含关键词 (小写匹配)
///
/// 支持的效果:
/// - `delete` — 删除消息
/// - `warn:<message>` — 发送警告
/// - `mute:<seconds>` — 禁言
/// - `ban` — 封禁
/// - `kick` — 踢出
pub struct AutoModRule {
    rules: Vec<AutoModEntry>,
}

#[derive(Debug, Clone)]
struct AutoModEntry {
    name: String,
    trigger: Trigger,
    conditions: Vec<Condition>,
    effect: Effect,
}

#[derive(Debug, Clone)]
enum Trigger {
    Message,
    Join,
    Regex(String),
}

#[derive(Debug, Clone)]
enum Condition {
    HasLink,
    HasMedia,
    IsNewMember(u64),
    TextLonger(usize),
    HasMention(u16),
    TextMatches(String),
}

#[derive(Debug, Clone)]
enum Effect {
    Delete,
    Warn(String),
    Mute(u64),
    Ban,
    Kick,
}

impl AutoModRule {
    pub fn new() -> Self {
        Self { rules: vec![] }
    }

    /// 从 JSON 字符串解析规则
    pub fn from_json(json: &str) -> Self {
        let rules = match serde_json::from_str::<Vec<serde_json::Value>>(json) {
            Ok(arr) => arr.into_iter().filter_map(|v| Self::parse_entry(&v)).collect(),
            Err(_) => vec![],
        };
        Self { rules }
    }

    fn parse_entry(v: &serde_json::Value) -> Option<AutoModEntry> {
        let name = v.get("name")?.as_str().unwrap_or("unnamed").to_string();
        let trigger = Self::parse_trigger(v.get("trigger")?.as_str()?)?;
        let conditions = v.get("conditions")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().filter_map(|c| c.as_str().and_then(Self::parse_condition)).collect())
            .unwrap_or_default();
        let effect = Self::parse_effect(v.get("effect")?.as_str()?)?;
        Some(AutoModEntry { name, trigger, conditions, effect })
    }

    fn parse_trigger(s: &str) -> Option<Trigger> {
        if s == "message" {
            Some(Trigger::Message)
        } else if s == "join" {
            Some(Trigger::Join)
        } else if let Some(pattern) = s.strip_prefix("regex:") {
            Some(Trigger::Regex(pattern.to_string()))
        } else {
            None
        }
    }

    fn parse_condition(s: &str) -> Option<Condition> {
        if s == "has_link" {
            Some(Condition::HasLink)
        } else if s == "has_media" {
            Some(Condition::HasMedia)
        } else if let Some(n) = s.strip_prefix("is_new_member:") {
            n.parse().ok().map(Condition::IsNewMember)
        } else if let Some(n) = s.strip_prefix("text_longer:") {
            n.parse().ok().map(Condition::TextLonger)
        } else if let Some(n) = s.strip_prefix("has_mention:") {
            n.parse().ok().map(Condition::HasMention)
        } else if let Some(kw) = s.strip_prefix("text_matches:") {
            Some(Condition::TextMatches(kw.to_lowercase()))
        } else {
            None
        }
    }

    fn parse_effect(s: &str) -> Option<Effect> {
        if s == "delete" {
            Some(Effect::Delete)
        } else if s == "ban" {
            Some(Effect::Ban)
        } else if s == "kick" {
            Some(Effect::Kick)
        } else if let Some(msg) = s.strip_prefix("warn:") {
            Some(Effect::Warn(msg.to_string()))
        } else if let Some(secs) = s.strip_prefix("mute:") {
            secs.parse().ok().map(Effect::Mute)
        } else {
            None
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// 检查触发器是否匹配
    fn trigger_matches(trigger: &Trigger, ctx: &MessageContext) -> bool {
        match trigger {
            Trigger::Message => {
                !ctx.is_new_member && !ctx.is_left_member && !ctx.is_join_request
                    && !ctx.message_text.is_empty()
            }
            Trigger::Join => ctx.is_new_member || ctx.is_join_request,
            Trigger::Regex(pattern) => {
                if ctx.message_text.is_empty() { return false; }
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(&ctx.message_text))
                    .unwrap_or(false)
            }
        }
    }

    /// 检查条件是否满足
    fn condition_matches(cond: &Condition, ctx: &MessageContext, store: &LocalStore) -> bool {
        match cond {
            Condition::HasLink => {
                let lower = ctx.message_text.to_lowercase();
                lower.contains("http://") || lower.contains("https://")
                    || lower.contains("t.me/") || lower.contains("www.")
            }
            Condition::HasMedia => {
                ctx.message_type.as_deref()
                    .map(|t| matches!(t, "photo" | "video" | "document" | "animation" | "audio" | "voice"))
                    .unwrap_or(false)
            }
            Condition::IsNewMember(n) => {
                let key = format!("automod_msgcount:{}:{}", ctx.group_id, ctx.sender_id);
                let count: u64 = store.get_string(&key)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                count < *n
            }
            Condition::TextLonger(n) => ctx.message_text.len() > *n,
            Condition::HasMention(n) => {
                let count = ctx.message_text.matches('@').count() as u16;
                count > *n
            }
            Condition::TextMatches(kw) => {
                ctx.message_text.to_lowercase().contains(kw.as_str())
            }
        }
    }

    /// 执行效果
    fn apply_effect(effect: &Effect, ctx: &MessageContext) -> Option<ActionDecision> {
        match effect {
            Effect::Delete => {
                if let Some(ref msg_id) = ctx.message_id {
                    Some(ActionDecision::delete_message(msg_id))
                } else {
                    Some(ActionDecision::warn(&ctx.sender_id, "AutoMod: 消息已被自动审核"))
                }
            }
            Effect::Warn(msg) => {
                Some(ActionDecision::warn(&ctx.sender_id, msg))
            }
            Effect::Mute(secs) => {
                Some(ActionDecision::mute(&ctx.sender_id, *secs, "AutoMod: 自动禁言"))
            }
            Effect::Ban => {
                Some(ActionDecision::ban(&ctx.sender_id, "AutoMod: 自动封禁"))
            }
            Effect::Kick => {
                Some(ActionDecision::kick(&ctx.sender_id, "AutoMod: 自动踢出"))
            }
        }
    }
}

#[async_trait]
impl Rule for AutoModRule {
    fn name(&self) -> &'static str { "automod" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_admin || ctx.is_command || self.rules.is_empty() {
            return None;
        }
        if ctx.callback_query_id.is_some() {
            return None;
        }

        // 递增消息计数 (用于 is_new_member 条件)
        if !ctx.is_new_member && !ctx.is_left_member && !ctx.is_join_request {
            let key = format!("automod_msgcount:{}:{}", ctx.group_id, ctx.sender_id);
            let count: u64 = store.get_string(&key)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            store.set_string(&key, &(count + 1).to_string());
        }

        for entry in &self.rules {
            // 1. 检查触发器
            if !Self::trigger_matches(&entry.trigger, ctx) {
                continue;
            }

            // 2. 检查所有条件 (AND)
            let all_conditions_met = entry.conditions.iter()
                .all(|c| Self::condition_matches(c, ctx, store));

            if !all_conditions_met {
                continue;
            }

            // 3. 执行效果
            if let Some(mut decision) = Self::apply_effect(&entry.effect, ctx) {
                // 附带规则名称
                let reason = format!("AutoMod [{}]", entry.name);
                if decision.reason.is_none() {
                    decision.reason = Some(reason);
                }
                return Some(decision);
            }
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

    fn sample_json() -> &'static str {
        r#"[
            {
                "name": "新成员禁止链接",
                "trigger": "message",
                "conditions": ["has_link", "is_new_member:5"],
                "effect": "delete"
            },
            {
                "name": "超长消息警告",
                "trigger": "message",
                "conditions": ["text_longer:100"],
                "effect": "warn:消息过长"
            },
            {
                "name": "入群自动检查",
                "trigger": "join",
                "conditions": [],
                "effect": "warn:欢迎新成员"
            },
            {
                "name": "禁止交易关键词",
                "trigger": "message",
                "conditions": ["text_matches:buy crypto"],
                "effect": "mute:60"
            }
        ]"#
    }

    #[test]
    fn parse_json_rules() {
        let rule = AutoModRule::from_json(sample_json());
        assert_eq!(rule.rule_count(), 4);
    }

    #[test]
    fn parse_invalid_json() {
        let rule = AutoModRule::from_json("not json");
        assert_eq!(rule.rule_count(), 0);
    }

    #[test]
    fn parse_empty_array() {
        let rule = AutoModRule::from_json("[]");
        assert_eq!(rule.rule_count(), 0);
    }

    #[tokio::test]
    async fn new_member_link_blocked() {
        let store = LocalStore::new();
        let rule = AutoModRule::from_json(sample_json());

        // 新成员 (msg_count=0 < 5) 发链接 → 删除
        let d = rule.evaluate(&make_ctx("check https://spam.com"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn veteran_user_link_passes() {
        let store = LocalStore::new();
        let rule = AutoModRule::from_json(sample_json());

        // 模拟 5 条消息后
        let key = format!("automod_msgcount:g1:u1");
        store.set_string(&key, "5");

        let d = rule.evaluate(&make_ctx("check https://link.com"), &store).await;
        // 链接规则不触发, 但可能匹配其他规则
        // 检查不是因为 "新成员禁止链接" 被触发
        if let Some(ref decision) = d {
            assert_ne!(decision.reason.as_deref().unwrap_or(""), "AutoMod [新成员禁止链接]");
        }
    }

    #[tokio::test]
    async fn long_message_warns() {
        let store = LocalStore::new();
        // 只有超长消息规则
        let json = r#"[{"name":"长消息","trigger":"message","conditions":["text_longer:10"],"effect":"warn:太长了"}]"#;
        let rule = AutoModRule::from_json(json);

        let d = rule.evaluate(&make_ctx("this is a very long message indeed"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
        assert!(d.message.unwrap().contains("太长了"));
    }

    #[tokio::test]
    async fn short_message_passes() {
        let store = LocalStore::new();
        let json = r#"[{"name":"长消息","trigger":"message","conditions":["text_longer:100"],"effect":"warn:太长了"}]"#;
        let rule = AutoModRule::from_json(json);

        assert!(rule.evaluate(&make_ctx("short"), &store).await.is_none());
    }

    #[tokio::test]
    async fn join_trigger() {
        let store = LocalStore::new();
        let json = r#"[{"name":"入群","trigger":"join","conditions":[],"effect":"warn:hello"}]"#;
        let rule = AutoModRule::from_json(json);

        let mut ctx = make_ctx("");
        ctx.is_new_member = true;
        ctx.message_text = String::new();
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn regex_trigger() {
        let store = LocalStore::new();
        let json = r#"[{"name":"数字检测","trigger":"regex:\\d{10,}","conditions":[],"effect":"delete"}]"#;
        let rule = AutoModRule::from_json(json);

        // 匹配
        let d = rule.evaluate(&make_ctx("call 1234567890"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);

        // 不匹配
        assert!(rule.evaluate(&make_ctx("hello world"), &store).await.is_none());
    }

    #[tokio::test]
    async fn text_matches_condition() {
        let store = LocalStore::new();
        let json = r#"[{"name":"关键词","trigger":"message","conditions":["text_matches:buy crypto"],"effect":"mute:60"}]"#;
        let rule = AutoModRule::from_json(json);

        let d = rule.evaluate(&make_ctx("want to buy crypto now"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Mute);
        assert_eq!(d.duration_secs, Some(60));
    }

    #[tokio::test]
    async fn ban_effect() {
        let store = LocalStore::new();
        let json = r#"[{"name":"封禁","trigger":"message","conditions":["text_matches:scam"],"effect":"ban"}]"#;
        let rule = AutoModRule::from_json(json);

        let d = rule.evaluate(&make_ctx("this is a scam"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
    }

    #[tokio::test]
    async fn kick_effect() {
        let store = LocalStore::new();
        let json = r#"[{"name":"踢出","trigger":"join","conditions":[],"effect":"kick"}]"#;
        let rule = AutoModRule::from_json(json);

        let mut ctx = make_ctx("");
        ctx.is_new_member = true;
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Kick);
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = AutoModRule::from_json(sample_json());

        let mut ctx = make_ctx("https://spam.com");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn multiple_conditions_and() {
        let store = LocalStore::new();
        // 同时需要链接 AND 关键词
        let json = r#"[{"name":"双条件","trigger":"message","conditions":["has_link","text_matches:spam"],"effect":"delete"}]"#;
        let rule = AutoModRule::from_json(json);

        // 只有链接, 无关键词
        assert!(rule.evaluate(&make_ctx("https://example.com"), &store).await.is_none());

        // 只有关键词, 无链接
        assert!(rule.evaluate(&make_ctx("spam here"), &store).await.is_none());

        // 两个都有
        let d = rule.evaluate(&make_ctx("spam https://evil.com"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn empty_rules_passes() {
        let store = LocalStore::new();
        let rule = AutoModRule::new();
        assert!(rule.evaluate(&make_ctx("anything"), &store).await.is_none());
    }

    #[tokio::test]
    async fn has_media_condition() {
        let store = LocalStore::new();
        let json = r#"[{"name":"禁止媒体","trigger":"message","conditions":["has_media"],"effect":"delete"}]"#;
        let rule = AutoModRule::from_json(json);

        let mut ctx = make_ctx("photo caption");
        ctx.message_type = Some("photo".into());
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);

        // 无媒体
        assert!(rule.evaluate(&make_ctx("just text"), &store).await.is_none());
    }
}
