use crate::platform::{PlatformAdapter, PlatformEvent, MessageContext};

/// Discord Gateway 事件适配器
pub struct DiscordAdapter;

impl DiscordAdapter {
    pub fn new() -> Self { Self }
}

impl PlatformAdapter for DiscordAdapter {
    fn platform_name(&self) -> &'static str { "discord" }

    fn parse_event(&self, raw: &serde_json::Value) -> Option<PlatformEvent> {
        let event_type = raw.get("t")?.as_str()?;
        let d = raw.get("d")?;

        match event_type {
            "MESSAGE_CREATE" => {
                let author = d.get("author")?;
                // 忽略 Bot 消息
                if author.get("bot").and_then(|b| b.as_bool()) == Some(true) {
                    return None;
                }
                let content = d.get("content").and_then(|c| c.as_str()).unwrap_or("");
                Some(PlatformEvent {
                    platform: "discord".into(),
                    event_type: if content.starts_with('/') { "command".into() } else { "message".into() },
                    group_id: d.get("guild_id")?.as_str()?.to_string(),
                    sender_id: author.get("id")?.as_str()?.to_string(),
                    sender_name: author.get("username")?.as_str()?.to_string(),
                    message_id: d.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()),
                    content: Some(content.to_string()),
                    raw_event: raw.clone(),
                    timestamp: 0,
                })
            }
            "GUILD_MEMBER_ADD" => {
                let user = d.get("user")?;
                Some(PlatformEvent {
                    platform: "discord".into(),
                    event_type: "member_join".into(),
                    group_id: d.get("guild_id")?.as_str()?.to_string(),
                    sender_id: user.get("id")?.as_str()?.to_string(),
                    sender_name: user.get("username")?.as_str()?.to_string(),
                    message_id: None,
                    content: None,
                    raw_event: raw.clone(),
                    timestamp: 0,
                })
            }
            _ => None,
        }
    }

    fn extract_context(&self, event: &PlatformEvent) -> MessageContext {
        let text = event.content.clone().unwrap_or_default();
        let is_command = text.starts_with('/') || text.starts_with('!');
        let (command, args) = if is_command {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            let cmd = parts[0].trim_start_matches('/').trim_start_matches('!');
            let args = if parts.len() > 1 {
                parts[1].split_whitespace().map(|s| s.to_string()).collect()
            } else {
                vec![]
            };
            (Some(cmd.to_string()), args)
        } else {
            (None, vec![])
        };

        MessageContext {
            platform: "discord".into(),
            group_id: event.group_id.clone(),
            sender_id: event.sender_id.clone(),
            sender_name: event.sender_name.clone(),
            message_text: text,
            message_id: event.message_id.clone(),
            is_command,
            command,
            command_args: args,
            is_join_request: event.event_type == "member_join",
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_message_create() {
        let adapter = DiscordAdapter::new();
        let raw = json!({
            "t": "MESSAGE_CREATE",
            "d": {
                "id": "111",
                "guild_id": "222",
                "content": "hello",
                "author": {"id": "333", "username": "alice", "bot": false}
            }
        });
        let event = adapter.parse_event(&raw).unwrap();
        assert_eq!(event.platform, "discord");
        assert_eq!(event.event_type, "message");
        assert_eq!(event.group_id, "222");
    }

    #[test]
    fn skip_bot_messages() {
        let adapter = DiscordAdapter::new();
        let raw = json!({
            "t": "MESSAGE_CREATE",
            "d": {
                "id": "111", "guild_id": "222", "content": "bot msg",
                "author": {"id": "999", "username": "mybot", "bot": true}
            }
        });
        assert!(adapter.parse_event(&raw).is_none());
    }
}
