use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;
use super::text_utils::html_escape;

/// 管理指令规则
///
/// 支持的命令:
/// - 需要目标 + 管理员: /ban /kick /mute /unmute /unban /warn /promote /demote
/// - 无需目标 (管理员): /lock /unlock /locks
/// - 无需目标 (所有人): /help /id /rules
pub struct CommandRule;

impl CommandRule {
    pub fn new() -> Self { Self }

    /// 管理员专用命令 (需要目标)
    const ADMIN_TARGET_CMDS: &'static [&'static str] = &[
        "ban", "kick", "mute", "unmute", "unban", "warn", "promote", "demote",
    ];

    /// 管理员专用命令 (无需目标)
    const ADMIN_NO_TARGET_CMDS: &'static [&'static str] = &[
        "lock", "unlock", "locks",
    ];

    /// 所有人可用命令
    const PUBLIC_CMDS: &'static [&'static str] = &[
        "help", "id", "rules",
    ];

    fn is_known_cmd(cmd: &str) -> bool {
        Self::ADMIN_TARGET_CMDS.contains(&cmd)
            || Self::ADMIN_NO_TARGET_CMDS.contains(&cmd)
            || Self::PUBLIC_CMDS.contains(&cmd)
    }
}

#[async_trait]
impl Rule for CommandRule {
    fn name(&self) -> &'static str { "command" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        if !ctx.is_command {
            return None;
        }

        let cmd = ctx.command.as_deref()?;

        if !Self::is_known_cmd(cmd) {
            return None;
        }

        // ── 公开命令 (无需管理员) ──
        match cmd {
            "help" => return Some(ActionDecision::send_message(
                &ctx.group_id,
                "<b>📋 Available Commands</b>\n\n\
                 <b>Admin:</b>\n\
                 /ban &lt;user&gt; — Ban user\n\
                 /unban &lt;user&gt; — Unban user\n\
                 /kick &lt;user&gt; — Kick user\n\
                 /mute &lt;user&gt; [secs] — Mute user\n\
                 /unmute &lt;user&gt; — Unmute user\n\
                 /warn &lt;user&gt; [reason] — Warn user\n\
                 /promote &lt;user&gt; — Promote to admin\n\
                 /demote &lt;user&gt; — Demote from admin\n\
                 /lock &lt;type&gt; — Lock message type\n\
                 /unlock &lt;type&gt; — Unlock message type\n\
                 /locks — Show locked types\n\n\
                 <b>Public:</b>\n\
                 /help — Show this help\n\
                 /id — Show your user ID\n\
                 /rules — Show group rules",
            )),
            "id" => return Some(ActionDecision::send_message(
                &ctx.group_id,
                &format!("🆔 Your ID: <code>{}</code>\nGroup: <code>{}</code>", html_escape(&ctx.sender_id), html_escape(&ctx.group_id)),
            )),
            "rules" => return Some(ActionDecision::send_message(
                &ctx.group_id,
                "📜 Group rules are managed on-chain. Contact an admin for details.",
            )),
            _ => {}
        }

        // ── 管理员权限守卫 ──
        if !ctx.is_admin {
            return Some(ActionDecision::send_message(
                &ctx.group_id,
                "⚠️ You need admin privileges to use this command.",
            ));
        }

        // ── 管理员无目标命令 ──
        if Self::ADMIN_NO_TARGET_CMDS.contains(&cmd) {
            let arg = ctx.command_args.first().map(|s| s.as_str()).unwrap_or("");
            return match cmd {
                "lock" => {
                    if arg.is_empty() {
                        Some(ActionDecision::send_message(&ctx.group_id,
                            "Usage: /lock &lt;type&gt;\nTypes: photo, video, audio, document, sticker, animation, voice, forward, contact, location, poll"))
                    } else {
                        Some(ActionDecision::send_message(&ctx.group_id,
                            &format!("🔒 Type <b>{}</b> locked. Update group config to persist.", html_escape(arg))))
                    }
                }
                "unlock" => {
                    if arg.is_empty() {
                        Some(ActionDecision::send_message(&ctx.group_id, "Usage: /unlock &lt;type&gt;"))
                    } else {
                        Some(ActionDecision::send_message(&ctx.group_id,
                            &format!("🔓 Type <b>{}</b> unlocked. Update group config to persist.", html_escape(arg))))
                    }
                }
                "locks" => Some(ActionDecision::send_message(&ctx.group_id,
                    "🔒 Locked types are defined in the on-chain group config.")),
                _ => None,
            };
        }

        // ── 管理员目标命令 ──
        let target = ctx.command_args.first().map(|s| s.as_str()).unwrap_or("");
        if target.is_empty() {
            return Some(ActionDecision::send_message(
                &ctx.group_id,
                &format!("Usage: /{} &lt;user&gt;", cmd),
            ));
        }

        match cmd {
            "ban" => Some(ActionDecision::ban(target, &format!("Banned by {}", ctx.sender_name))),
            "kick" => Some(ActionDecision::kick(target, &format!("Kicked by {}", ctx.sender_name))),
            "mute" => {
                let duration = ctx.command_args.get(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(3600);
                Some(ActionDecision::mute(target, duration, &format!("Muted by {}", ctx.sender_name)))
            }
            "unmute" => Some(ActionDecision {
                action_type: crate::platform::ActionType::Unmute,
                target_user: target.to_string(),
                reason: Some(format!("Unmuted by {}", ctx.sender_name)),
                message: None,
                duration_secs: None,
            }),
            "unban" => Some(ActionDecision::unban(target, &format!("Unbanned by {}", ctx.sender_name))),
            "warn" => {
                let reason = if ctx.command_args.len() > 1 {
                    ctx.command_args[1..].join(" ")
                } else {
                    "No reason specified".to_string()
                };
                Some(ActionDecision::warn(target, &reason))
            }
            "promote" => Some(ActionDecision::promote(target, &format!("Promoted by {}", ctx.sender_name))),
            "demote" => Some(ActionDecision::demote(target, &format!("Demoted by {}", ctx.sender_name))),
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
            message_id: None,
            is_command: true,
            command: Some(cmd.into()),
            command_args: args.into_iter().map(|s| s.to_string()).collect(),
            is_join_request: false,
            is_admin: true,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
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
        assert!(rule.evaluate(&cmd_ctx("foobar", vec!["me"]), &store).await.is_none());
    }

    #[tokio::test]
    async fn no_target_shows_usage() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let ctx = cmd_ctx("ban", vec![]);
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_some());
        let d = result.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::SendMessage);
        assert!(d.message.unwrap().contains("Usage"));
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
        assert!(d.message.unwrap().contains("admin privileges"));
    }

    #[tokio::test]
    async fn unban_command() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let d = rule.evaluate(&cmd_ctx("unban", vec!["789"]), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Unban);
        assert_eq!(d.target_user, "789");
    }

    #[tokio::test]
    async fn promote_command() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let d = rule.evaluate(&cmd_ctx("promote", vec!["789"]), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Promote);
    }

    #[tokio::test]
    async fn demote_command() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let d = rule.evaluate(&cmd_ctx("demote", vec!["789"]), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::Demote);
    }

    #[tokio::test]
    async fn help_no_admin_required() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let mut ctx = cmd_ctx("help", vec![]);
        ctx.is_admin = false;
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::SendMessage);
        assert!(d.message.unwrap().contains("Available Commands"));
    }

    #[tokio::test]
    async fn id_shows_user_id() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let mut ctx = cmd_ctx("id", vec![]);
        ctx.is_admin = false;
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert!(d.message.unwrap().contains("admin1"));
    }

    #[tokio::test]
    async fn lock_command() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let d = rule.evaluate(&cmd_ctx("lock", vec!["photo"]), &store).await.unwrap();
        assert_eq!(d.action_type, crate::platform::ActionType::SendMessage);
        assert!(d.message.unwrap().contains("locked"));
    }

    #[tokio::test]
    async fn locks_command() {
        let store = LocalStore::new();
        let rule = CommandRule::new();
        let d = rule.evaluate(&cmd_ctx("locks", vec![]), &store).await.unwrap();
        assert!(d.message.unwrap().contains("Locked types"));
    }
}
