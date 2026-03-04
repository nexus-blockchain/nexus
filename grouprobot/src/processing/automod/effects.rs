use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use crate::processing::rules::text_utils::html_escape;
use super::triggers::TriggerResult;

/// 效果 Trait — 触发后做什么
pub trait Effect: Send + Sync {
    fn name(&self) -> &'static str;
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision;
}

// ── 具体效果实现 ──

/// 删除消息
pub struct DeleteMessageEffect;

impl Effect for DeleteMessageEffect {
    fn name(&self) -> &'static str { "delete_message" }
    fn apply(&self, ctx: &MessageContext, _trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::delete_message(&ctx.sender_id)
    }
}

/// 警告用户
pub struct WarnUserEffect;

impl Effect for WarnUserEffect {
    fn name(&self) -> &'static str { "warn_user" }
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::warn(
            &ctx.sender_id,
            &format!("[AutoMod] {}", trigger.details),
        )
    }
}

/// 禁言用户
pub struct MuteUserEffect {
    duration_secs: u64,
}

impl MuteUserEffect {
    pub fn new(duration_secs: u64) -> Self {
        Self { duration_secs }
    }
}

impl Effect for MuteUserEffect {
    fn name(&self) -> &'static str { "mute_user" }
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::mute(
            &ctx.sender_id,
            self.duration_secs,
            &format!("[AutoMod] {}", trigger.details),
        )
    }
}

/// 踢出用户
pub struct KickUserEffect;

impl Effect for KickUserEffect {
    fn name(&self) -> &'static str { "kick_user" }
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::kick(
            &ctx.sender_id,
            &format!("[AutoMod] {}", trigger.details),
        )
    }
}

/// 封禁用户
pub struct BanUserEffect;

impl Effect for BanUserEffect {
    fn name(&self) -> &'static str { "ban_user" }
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::ban(
            &ctx.sender_id,
            &format!("[AutoMod] {}", trigger.details),
        )
    }
}

/// 发送警报到管理员 (通知管理员频道)
pub struct SendAlertEffect {
    alert_prefix: String,
}

impl SendAlertEffect {
    pub fn new(prefix: &str) -> Self {
        Self { alert_prefix: prefix.to_string() }
    }
}

impl Effect for SendAlertEffect {
    fn name(&self) -> &'static str { "send_alert" }
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::send_message(
            &ctx.group_id,
            &format!("{} @{}: {}", self.alert_prefix, html_escape(&ctx.sender_name), html_escape(&trigger.details)),
        )
    }
}

/// 累积违规 (记录违规次数，配合 ViolationsTrigger 使用)
/// 这里只发出 Warn，由 WarnTracker 负责累积升级
pub struct AddViolationEffect {
    violation_type: String,
}

impl AddViolationEffect {
    pub fn new(violation_type: &str) -> Self {
        Self { violation_type: violation_type.to_string() }
    }
}

impl Effect for AddViolationEffect {
    fn name(&self) -> &'static str { "add_violation" }
    fn apply(&self, ctx: &MessageContext, trigger: &TriggerResult) -> ActionDecision {
        ActionDecision::warn(
            &ctx.sender_id,
            &format!("[{}] {}", self.violation_type, trigger.details),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn make_ctx() -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "TestUser".into(),
            message_text: "bad message".into(),
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

    fn make_trigger() -> TriggerResult {
        TriggerResult {
            trigger_name: "test".into(),
            details: "matched bad word".into(),
        }
    }

    #[test]
    fn delete_effect() {
        let e = DeleteMessageEffect;
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[test]
    fn warn_effect() {
        let e = WarnUserEffect;
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::Warn);
        assert!(d.message.unwrap().contains("AutoMod"));
    }

    #[test]
    fn mute_effect() {
        let e = MuteUserEffect::new(600);
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::Mute);
        assert_eq!(d.duration_secs, Some(600));
    }

    #[test]
    fn kick_effect() {
        let e = KickUserEffect;
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::Kick);
    }

    #[test]
    fn ban_effect() {
        let e = BanUserEffect;
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::Ban);
    }

    #[test]
    fn alert_effect() {
        let e = SendAlertEffect::new("⚠️ Alert");
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::SendMessage);
        assert!(d.message.unwrap().contains("TestUser"));
    }

    #[test]
    fn violation_effect() {
        let e = AddViolationEffect::new("spam");
        let d = e.apply(&make_ctx(), &make_trigger());
        assert_eq!(d.action_type, ActionType::Warn);
        assert!(d.message.unwrap().contains("[spam]"));
    }
}
