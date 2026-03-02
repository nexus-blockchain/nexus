use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;
use super::text_utils::html_escape;

/// G14: 日志频道规则 — 将审核操作转发至指定日志频道
///
/// 此规则不阻断规则链 (始终返回 None)。
/// 它通过 LocalStore 记录待发送的日志条目，
/// 由上层 executor 在执行完主操作后读取并转发。
pub struct LogChannelRule {
    log_channel_id: String,
}

impl LogChannelRule {
    pub fn new(log_channel_id: &str) -> Self {
        Self {
            log_channel_id: log_channel_id.to_string(),
        }
    }

    /// 格式化审核日志消息
    pub fn format_log(ctx: &MessageContext, action_desc: &str) -> String {
        format!(
            "📝 <b>Mod Log</b>\n\
             👤 User: <code>{}</code> ({})\n\
             💬 Group: <code>{}</code>\n\
             ⚡ Action: {}\n\
             📄 Message: <code>{}</code>",
            html_escape(&ctx.sender_id),
            html_escape(&ctx.sender_name),
            html_escape(&ctx.group_id),
            action_desc,
            html_escape(&ctx.message_text.chars().take(100).collect::<String>()),
        )
    }

    /// 获取日志频道 ID
    pub fn channel_id(&self) -> &str {
        &self.log_channel_id
    }

    /// 将日志条目写入 store (供 executor 读取)
    pub fn queue_log(store: &LocalStore, group_id: &str, log_channel_id: &str, log_text: &str) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let key = format!("pending_log:{}:{}:{}", group_id, now_millis(), seq);
        store.set_string(&key, &format!("{}|{}", log_channel_id, log_text));
    }

    /// 从 store 中取出所有待发送日志 (消费式)
    pub fn drain_logs(store: &LocalStore, group_id: &str) -> Vec<(String, String)> {
        let prefix = format!("pending_log:{}:", group_id);
        store.drain_strings_with_prefix(&prefix)
            .into_iter()
            .filter_map(|(_key, val)| {
                let (channel, text) = val.split_once('|')?;
                Some((channel.to_string(), text.to_string()))
            })
            .collect()
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[async_trait]
impl Rule for LogChannelRule {
    fn name(&self) -> &'static str { "log_channel" }

    async fn evaluate(&self, _ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        // LogChannelRule 不直接产生 action —
        // 日志转发由 RuleEngine::evaluate 的后处理阶段完成。
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_log_message() {
        let ctx = MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "Alice".into(),
            message_text: "bad content".into(),
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
        };
        let log = LogChannelRule::format_log(&ctx, "Warned for spam");
        assert!(log.contains("Alice"));
        assert!(log.contains("Warned for spam"));
        assert!(log.contains("bad content"));
    }

    #[test]
    fn queue_and_drain_logs() {
        let store = LocalStore::new();
        LogChannelRule::queue_log(&store, "g1", "log_ch_1", "test log 1");
        LogChannelRule::queue_log(&store, "g1", "log_ch_1", "test log 2");
        LogChannelRule::queue_log(&store, "g2", "log_ch_2", "other group");

        let logs = LogChannelRule::drain_logs(&store, "g1");
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].0, "log_ch_1");
        assert!(logs[0].1.contains("test log"));

        // g2 logs unaffected
        let logs2 = LogChannelRule::drain_logs(&store, "g2");
        assert_eq!(logs2.len(), 1);

        // drained — should be empty now
        assert!(LogChannelRule::drain_logs(&store, "g1").is_empty());
    }

    #[tokio::test]
    async fn evaluate_returns_none() {
        let store = LocalStore::new();
        let rule = LogChannelRule::new("log_ch_1");
        let ctx = MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "Alice".into(),
            message_text: "hello".into(),
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
        };
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
