use crate::platform::{PlatformAdapter, PlatformEvent, MessageContext};

/// Telegram Webhook 事件适配器
pub struct TelegramAdapter;

impl TelegramAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl TelegramAdapter {
    /// 从 Telegram raw event 中检测消息类型
    fn detect_message_type(raw: &serde_json::Value) -> Option<String> {
        let msg = raw.get("message").or_else(|| raw.get("edited_message"))?;
        if msg.get("photo").is_some() { return Some("photo".into()); }
        if msg.get("video").is_some() { return Some("video".into()); }
        if msg.get("audio").is_some() { return Some("audio".into()); }
        if msg.get("document").is_some() { return Some("document".into()); }
        if msg.get("sticker").is_some() { return Some("sticker".into()); }
        if msg.get("animation").is_some() { return Some("animation".into()); }
        if msg.get("voice").is_some() { return Some("voice".into()); }
        if msg.get("video_note").is_some() { return Some("video_note".into()); }
        if msg.get("contact").is_some() { return Some("contact".into()); }
        if msg.get("location").is_some() { return Some("location".into()); }
        if msg.get("poll").is_some() { return Some("poll".into()); }
        if msg.get("game").is_some() { return Some("game".into()); }
        if msg.get("forward_from").is_some() || msg.get("forward_from_chat").is_some() {
            return Some("forward".into());
        }
        if msg.get("text").is_some() { return Some("text".into()); }
        None
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
        let callback_query = raw.get("callback_query");

        if let Some(cb) = callback_query {
            let from = cb.get("from")?;
            let msg = cb.get("message")?;
            let chat = msg.get("chat")?;
            let data = cb.get("data").and_then(|d| d.as_str()).unwrap_or("");
            return Some(PlatformEvent {
                platform: "telegram".into(),
                event_type: "callback_query".into(),
                group_id: chat.get("id")?.to_string(),
                sender_id: from.get("id")?.to_string(),
                sender_name: from.get("first_name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                message_id: msg.get("message_id").map(|id| id.to_string()),
                content: Some(data.to_string()),
                raw_event: raw.clone(),
                timestamp: msg.get("date").and_then(|d| d.as_u64()).unwrap_or(0),
            });
        }

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
        let is_callback = event.event_type == "callback_query";
        let is_command = !is_callback && text.starts_with('/');
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

        // 检测消息类型
        let message_type = Self::detect_message_type(&event.raw_event);

        // CallbackQuery
        let (callback_query_id, callback_data) = if is_callback {
            let cb = event.raw_event.get("callback_query");
            let qid = cb.and_then(|c| c.get("id")).and_then(|v| v.as_str()).map(|s| s.to_string());
            let data = Some(text.clone());
            (qid, data)
        } else {
            (None, None)
        };

        MessageContext {
            platform: "telegram".into(),
            group_id: event.group_id.clone(),
            sender_id: event.sender_id.clone(),
            sender_name: event.sender_name.clone(),
            message_text: text,
            message_id: event.message_id.clone(),
            is_command,
            command,
            command_args: args,
            is_join_request: event.event_type == "join_request",
            is_admin: false, // 需要通过 API 查询
            message_type,
            callback_query_id,
            callback_data,
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
