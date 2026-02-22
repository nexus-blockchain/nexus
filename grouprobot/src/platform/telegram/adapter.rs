use crate::platform::{PlatformAdapter, PlatformEvent, MessageContext};

/// Telegram Webhook 事件适配器
pub struct TelegramAdapter;

impl TelegramAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for TelegramAdapter {
    fn platform_name(&self) -> &'static str {
        "telegram"
    }

    fn parse_event(&self, raw: &serde_json::Value) -> Option<PlatformEvent> {
        // Telegram Update 结构
        let message = raw.get("message").or_else(|| raw.get("edited_message"));
        let join_request = raw.get("chat_join_request");
        let _callback_query = raw.get("callback_query");

        if let Some(msg) = message {
            let chat = msg.get("chat")?;
            let from = msg.get("from")?;
            let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");

            Some(PlatformEvent {
                platform: "telegram".into(),
                event_type: if text.starts_with('/') { "command".into() } else { "message".into() },
                group_id: chat.get("id")?.to_string(),
                sender_id: from.get("id")?.to_string(),
                sender_name: from.get("first_name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                message_id: msg.get("message_id").map(|id| id.to_string()),
                content: Some(text.to_string()),
                raw_event: raw.clone(),
                timestamp: msg.get("date").and_then(|d| d.as_u64()).unwrap_or(0),
            })
        } else if let Some(jr) = join_request {
            let chat = jr.get("chat")?;
            let from = jr.get("from")?;

            Some(PlatformEvent {
                platform: "telegram".into(),
                event_type: "join_request".into(),
                group_id: chat.get("id")?.to_string(),
                sender_id: from.get("id")?.to_string(),
                sender_name: from.get("first_name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                message_id: None,
                content: jr.get("bio").and_then(|b| b.as_str()).map(|s| s.to_string()),
                raw_event: raw.clone(),
                timestamp: jr.get("date").and_then(|d| d.as_u64()).unwrap_or(0),
            })
        } else {
            None
        }
    }

    fn extract_context(&self, event: &PlatformEvent) -> MessageContext {
        let text = event.content.clone().unwrap_or_default();
        let is_command = text.starts_with('/');
        let (command, args) = if is_command {
            let parts: Vec<&str> = text.splitn(2, ' ').collect();
            let cmd = parts[0].strip_prefix('/').unwrap_or(parts[0]);
            // 去掉 @botname 后缀
            let cmd = cmd.split('@').next().unwrap_or(cmd);
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
            platform: "telegram".into(),
            group_id: event.group_id.clone(),
            sender_id: event.sender_id.clone(),
            sender_name: event.sender_name.clone(),
            message_text: text,
            is_command,
            command,
            command_args: args,
            is_join_request: event.event_type == "join_request",
            is_admin: false, // 需要通过 API 查询
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_text_message() {
        let adapter = TelegramAdapter::new();
        let update = json!({
            "message": {
                "message_id": 123,
                "date": 1700000000,
                "chat": {"id": -100123, "type": "supergroup"},
                "from": {"id": 456, "first_name": "Alice"},
                "text": "hello world"
            }
        });
        let event = adapter.parse_event(&update).unwrap();
        assert_eq!(event.platform, "telegram");
        assert_eq!(event.event_type, "message");
        assert_eq!(event.group_id, "-100123");
        assert_eq!(event.sender_id, "456");
        assert_eq!(event.content.unwrap(), "hello world");
    }

    #[test]
    fn parse_command() {
        let adapter = TelegramAdapter::new();
        let update = json!({
            "message": {
                "message_id": 124,
                "date": 1700000001,
                "chat": {"id": -100123, "type": "supergroup"},
                "from": {"id": 456, "first_name": "Alice"},
                "text": "/ban@mybot 789"
            }
        });
        let event = adapter.parse_event(&update).unwrap();
        assert_eq!(event.event_type, "command");

        let ctx = adapter.extract_context(&event);
        assert!(ctx.is_command);
        assert_eq!(ctx.command, Some("ban".into()));
        assert_eq!(ctx.command_args, vec!["789"]);
    }

    #[test]
    fn parse_join_request() {
        let adapter = TelegramAdapter::new();
        let update = json!({
            "chat_join_request": {
                "chat": {"id": -100123, "type": "supergroup"},
                "from": {"id": 789, "first_name": "Bob"},
                "date": 1700000002,
                "bio": "Hi there"
            }
        });
        let event = adapter.parse_event(&update).unwrap();
        assert_eq!(event.event_type, "join_request");
        assert!(adapter.extract_context(&event).is_join_request);
    }

    #[test]
    fn parse_empty_returns_none() {
        let adapter = TelegramAdapter::new();
        assert!(adapter.parse_event(&json!({})).is_none());
    }
}
