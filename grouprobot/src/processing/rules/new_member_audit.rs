use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G25: 新成员消息审查
///
/// 仅对新加入的成员的前 N 条消息施加更严格的检查。
/// 使用 LocalStore 追踪每个用户在每个群组的消息计数。
///
/// 工作方式:
/// - 新成员入群时, 由 JoinRequestRule 或 CaptchaRule 标记
/// - 本规则检查用户消息计数 < audit_count 时, 对其消息进行额外检测:
///   - 包含链接 → 删除 (新用户不允许发链接)
///   - 包含 forward (转发消息) → 删除
///   - 消息过长 (>500 字符) → 删除 (常见垃圾特征)
/// - 消息计数 >= audit_count 后, 此规则不再触发
pub struct NewMemberAuditRule {
    /// 审查的消息数量 (前 N 条)
    audit_count: u16,
    /// 是否禁止新成员发链接
    block_links: bool,
    /// 是否禁止新成员转发消息
    block_forwards: bool,
    /// 消息最大长度 (0=不限)
    max_length: usize,
}

impl NewMemberAuditRule {
    pub fn new(audit_count: u16) -> Self {
        Self {
            audit_count: if audit_count == 0 { 5 } else { audit_count },
            block_links: true,
            block_forwards: true,
            max_length: 500,
        }
    }

    pub fn with_options(audit_count: u16, block_links: bool, block_forwards: bool, max_length: usize) -> Self {
        Self {
            audit_count: if audit_count == 0 { 5 } else { audit_count },
            block_links,
            block_forwards,
            max_length,
        }
    }

    /// 简单链接检测 (http:// https:// t.me/ 或 www.)
    fn contains_link(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("http://")
            || lower.contains("https://")
            || lower.contains("t.me/")
            || lower.contains("www.")
            || lower.contains("bit.ly/")
            || lower.contains("tinyurl.com/")
    }

    /// 获取并递增用户消息计数, 返回当前计数 (递增前的值)
    fn get_and_increment(store: &LocalStore, group_id: &str, user_id: &str) -> u64 {
        let key = format!("newmember_msgcount:{}:{}", group_id, user_id);
        // increment_counter 使用滑动窗口, 我们需要持久计数
        // 使用 get_string + set_string 实现简单计数器
        let current: u64 = store.get_string(&key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        store.set_string(&key, &(current + 1).to_string());
        current
    }
}

#[async_trait]
impl Rule for NewMemberAuditRule {
    fn name(&self) -> &'static str { "new_member_audit" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        // 仅审查普通消息
        if ctx.is_command || ctx.is_join_request || ctx.is_new_member || ctx.is_left_member {
            return None;
        }
        if ctx.is_admin {
            return None;
        }
        if ctx.callback_query_id.is_some() {
            return None;
        }
        if ctx.message_text.is_empty() {
            return None;
        }

        let msg_count = Self::get_and_increment(store, &ctx.group_id, &ctx.sender_id);

        // 已超过审查期 → 跳过
        if msg_count >= self.audit_count as u64 {
            return None;
        }

        // ── 对新成员的前 N 条消息执行严格检查 ──

        // 1. 链接检查
        if self.block_links && Self::contains_link(&ctx.message_text) {
            if let Some(ref msg_id) = ctx.message_id {
                let mut d = ActionDecision::delete_message(msg_id);
                d.message = Some(format!(
                    "⚠️ 新成员前 {} 条消息不允许包含链接",
                    self.audit_count
                ));
                return Some(d);
            }
        }

        // 2. 转发消息检查
        if self.block_forwards {
            if let Some(ref msg_type) = ctx.message_type {
                if msg_type == "forward" {
                    if let Some(ref msg_id) = ctx.message_id {
                        let mut d = ActionDecision::delete_message(msg_id);
                        d.message = Some(format!(
                            "⚠️ 新成员前 {} 条消息不允许转发消息",
                            self.audit_count
                        ));
                        return Some(d);
                    }
                }
            }
        }

        // 3. 消息长度检查
        if self.max_length > 0 && ctx.message_text.len() > self.max_length {
            if let Some(ref msg_id) = ctx.message_id {
                let mut d = ActionDecision::delete_message(msg_id);
                d.message = Some(format!(
                    "⚠️ 新成员前 {} 条消息长度不能超过 {} 字符",
                    self.audit_count, self.max_length
                ));
                return Some(d);
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
            sender_id: "new_user".into(),
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

    #[tokio::test]
    async fn blocks_link_for_new_member() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(5);
        let d = rule.evaluate(&make_ctx("check https://spam.com"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
        assert!(d.message.unwrap().contains("链接"));
    }

    #[tokio::test]
    async fn allows_link_after_audit_period() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(3);
        // 先发 3 条正常消息
        for _ in 0..3 {
            rule.evaluate(&make_ctx("normal message"), &store).await;
        }
        // 第 4 条含链接 → 不再审查
        assert!(rule.evaluate(&make_ctx("check https://link.com"), &store).await.is_none());
    }

    #[tokio::test]
    async fn blocks_forward() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(5);
        let mut ctx = make_ctx("forwarded content");
        ctx.message_type = Some("forward".into());
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
        assert!(d.message.unwrap().contains("转发"));
    }

    #[tokio::test]
    async fn blocks_long_message() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(5);
        let long_text = "a".repeat(501);
        let d = rule.evaluate(&make_ctx(&long_text), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
        assert!(d.message.unwrap().contains("长度"));
    }

    #[tokio::test]
    async fn normal_short_message_passes() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(5);
        assert!(rule.evaluate(&make_ctx("hello everyone"), &store).await.is_none());
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(5);
        let mut ctx = make_ctx("https://admin-link.com");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn different_users_independent() {
        let store = LocalStore::new();
        let rule = NewMemberAuditRule::new(2);
        // user1 发 2 条
        for _ in 0..2 {
            rule.evaluate(&make_ctx("normal"), &store).await;
        }
        // user1 审查期结束
        assert!(rule.evaluate(&make_ctx("https://link.com"), &store).await.is_none());
        // user2 仍在审查期
        let mut ctx = make_ctx("https://link.com");
        ctx.sender_id = "new_user_2".into();
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn zero_audit_count_defaults() {
        let rule = NewMemberAuditRule::new(0);
        assert_eq!(rule.audit_count, 5);
    }

    #[test]
    fn contains_link_variations() {
        assert!(NewMemberAuditRule::contains_link("http://evil.com"));
        assert!(NewMemberAuditRule::contains_link("https://evil.com"));
        assert!(NewMemberAuditRule::contains_link("join t.me/group"));
        assert!(NewMemberAuditRule::contains_link("www.evil.com"));
        assert!(NewMemberAuditRule::contains_link("click bit.ly/abc"));
        assert!(!NewMemberAuditRule::contains_link("hello world"));
    }
}
