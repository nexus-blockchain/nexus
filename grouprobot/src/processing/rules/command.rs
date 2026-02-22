use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 管理指令规则 (/ban, /mute, /kick, /warn, /unmute, /unban)
pub struct CommandRule;

impl CommandRule {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Rule for CommandRule {
    fn name(&self) -> &'static str { "command" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if !ctx.is_command {
            return None;
        }

        let cmd = ctx.command.as_deref()?;
        let target = ctx.command_args.first().map(|s| s.as_str()).unwrap_or("");

        if target.is_empty() {
            return None;
        }

        // 管理员权限守卫: 非管理员不可执行管理指令
        if !ctx.is_admin {
            return Some(ActionDecision::send_message(
                &ctx.group_id,
                "⚠️ You need admin privileges to use this command.",
            ));
        }

        match cmd {
            "ban" => Some(ActionDecision::ban(target, &format!("Banned by {}", ctx.sender_name))),
            "kick" => Some(ActionDecision::kick(target, &format!("Kicked by {}", ctx.sender_name))),
            "mute" => {
                let duration = ctx.command_args.get(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(3600); // 默认 1h
                Some(ActionDecision::mute(target, duration, &format!("Muted by {}", ctx.sender_name)))
            }
            "unmute" => Some(ActionDecision {
                action_type: crate::platform::ActionType::Unmute,
                target_user: target.to_string(),
                reason: Some(format!("Unmuted by {}", ctx.sender_name)),
                message: None,
                duration_secs: None,
            }),
            "warn" => {
                let reason = if ctx.command_args.len() > 1 {
                    ctx.command_args[1..].join(" ")
                } else {
                    "No reason specified".to_string()
                };
                Some(ActionDecision::warn(target, &reason))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd_ctx(cmd: &str, args: Vec<&str>) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "admin1".into(),
            sender_name: "Admin".into(),
            message_text: format!("/{} {}", cmd, args.join(" ")),
            is_command: true,
            command: Some(cmd.into()),
            command_args: args.into_iter().map(|s| s.to_string()).collect(),
            is_join_request: false,
            is_admin: true,
        }
    }

    #[tokio::test]
    async fn ban_command() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let result = rule.evaluate(&cmd_ctx("ban", vec!["789"]), &store).await;
        assert!(result.is_some());
        let d = result.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Ban);
        assert_eq!(d.target_user, "789");
    }

    #[tokio::test]
    async fn mute_with_duration() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let result = rule.evaluate(&cmd_ctx("mute", vec!["789", "600"]), &store).await;
        let d = result.unwrap();
        assert_eq!(d.duration_secs, Some(600));
    }

    #[tokio::test]
    async fn unknown_command_passes() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        assert!(rule.evaluate(&cmd_ctx("help", vec!["me"]), &store).await.is_none());
    }

    #[tokio::test]
    async fn no_target_passes() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let mut ctx = cmd_ctx("ban", vec![]);
        ctx.command_args.clear();
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn non_admin_blocked() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let mut ctx = cmd_ctx("ban", vec!["789"]);
        ctx.is_admin = false;
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
        let d = result.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::SendMessage);
    }
}
