use crate::platform::MessageContext;

/// 条件 Trait — 附加过滤条件，决定触发后是否真正执行
pub trait Condition: Send + Sync {
    fn name(&self) -> &'static str;
    /// 返回 true 表示条件满足（规则继续），false 表示跳过
    fn check(&self, ctx: &MessageContext) -> bool;
}

// ── 具体条件实现 ──

/// 仅非管理员触发
pub struct NotAdminCondition;

impl Condition for NotAdminCondition {
    fn name(&self) -> &'static str { "not_admin" }
    fn check(&self, ctx: &MessageContext) -> bool { !ctx.is_admin }
}

/// 仅非命令消息触发
pub struct NotCommandCondition;

impl Condition for NotCommandCondition {
    fn name(&self) -> &'static str { "not_command" }
    fn check(&self, ctx: &MessageContext) -> bool { !ctx.is_command }
}

/// 仅特定平台触发
pub struct PlatformCondition {
    platform: String,
}

impl PlatformCondition {
    pub fn new(platform: &str) -> Self {
        Self { platform: platform.to_lowercase() }
    }
}

impl Condition for PlatformCondition {
    fn name(&self) -> &'static str { "platform" }
    fn check(&self, ctx: &MessageContext) -> bool {
        ctx.platform.to_lowercase() == self.platform
    }
}

/// 消息非空条件
pub struct NonEmptyMessageCondition;

impl Condition for NonEmptyMessageCondition {
    fn name(&self) -> &'static str { "non_empty_message" }
    fn check(&self, ctx: &MessageContext) -> bool { !ctx.message_text.is_empty() }
}

/// 账号年龄条件 (基于 sender_id 的数值大小近似判断 — Telegram 特有)
/// 新账号的 user_id 通常较大
pub struct MinAccountAgeCondition {
    min_user_id: u64,
}

impl MinAccountAgeCondition {
    pub fn new(min_user_id: u64) -> Self {
        Self { min_user_id }
    }
}

impl Condition for MinAccountAgeCondition {
    fn name(&self) -> &'static str { "min_account_age" }
    fn check(&self, ctx: &MessageContext) -> bool {
        // 如果 sender_id 是数字且大于阈值，认为是新账号
        ctx.sender_id.parse::<u64>()
            .map(|id| id < self.min_user_id)
            .unwrap_or(true) // 非数字 ID 默认通过
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(admin: bool, cmd: bool) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "12345".into(),
            sender_name: "test".into(),
            message_text: "hello".into(),
            message_id: None,
            is_command: cmd,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin: admin,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    #[test]
    fn not_admin_condition() {
        let c = NotAdminCondition;
        assert!(c.check(&make_ctx(false, false)));
        assert!(!c.check(&make_ctx(true, false)));
    }

    #[test]
    fn not_command_condition() {
        let c = NotCommandCondition;
        assert!(c.check(&make_ctx(false, false)));
        assert!(!c.check(&make_ctx(false, true)));
    }

    #[test]
    fn platform_condition() {
        let c = PlatformCondition::new("telegram");
        assert!(c.check(&make_ctx(false, false)));
        let c2 = PlatformCondition::new("discord");
        assert!(!c2.check(&make_ctx(false, false)));
    }

    #[test]
    fn non_empty_message() {
        let c = NonEmptyMessageCondition;
        assert!(c.check(&make_ctx(false, false)));
        let mut ctx = make_ctx(false, false);
        ctx.message_text = String::new();
        assert!(!c.check(&ctx));
    }

    #[test]
    fn account_age_condition() {
        let c = MinAccountAgeCondition::new(999999);
        let mut ctx = make_ctx(false, false);
        ctx.sender_id = "500000".into(); // old account
        assert!(c.check(&ctx));
        ctx.sender_id = "1500000".into(); // new account
        assert!(!c.check(&ctx));
    }
}
