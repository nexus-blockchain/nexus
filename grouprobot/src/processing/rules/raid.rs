use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G9: 突袭防护规则
///
/// 检测短时间内大量新成员入群 (Y 秒内 X 人) → 自动封禁后续新成员
/// 使用 LocalStore 计数器追踪入群频率
pub struct RaidRule {
    /// 窗口时间 (秒)
    window_secs: u64,
    /// 窗口内入群人数阈值
    join_threshold: u16,
}

impl RaidRule {
    pub fn new(window_secs: u64, join_threshold: u16) -> Self {
        Self {
            window_secs: if window_secs == 0 { 60 } else { window_secs },
            join_threshold: if join_threshold == 0 { 10 } else { join_threshold },
        }
    }
}

#[async_trait]
impl Rule for RaidRule {
    fn name(&self) -> &'static str { "raid" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        // 仅对新成员入群事件触发
        if !ctx.is_new_member && !ctx.is_join_request {
            return None;
        }

        // 递增群组入群计数器
        let key = format!("raid_joins:{}", ctx.group_id);
        let count = store.increment_counter(&key, self.window_secs);

        if count > self.join_threshold as u64 {
            // 阈值已超 → 封禁此新成员 (疑似突袭)
            Some(ActionDecision::ban(
                &ctx.sender_id,
                &format!(
                    "Raid protection: {} 人在 {}s 内入群 (阈值: {})",
                    count, self.window_secs, self.join_threshold
                ),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn join_ctx(sender: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: sender.into(),
            sender_name: "User".into(),
            message_text: String::new(),
            message_id: None,
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: true,
            is_left_member: false,
            service_message_id: None,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    fn normal_ctx() -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "User".into(),
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
        }
    }

    #[tokio::test]
    async fn under_threshold_passes() {
        let store = LocalStore::new();
        let rule = RaidRule::new(60, 5);
        for i in 0..5 {
            let ctx = join_ctx(&format!("user_{}", i));
            assert!(rule.evaluate(&ctx, &store).await.is_none());
        }
    }

    #[tokio::test]
    async fn over_threshold_bans() {
        let store = LocalStore::new();
        let rule = RaidRule::new(60, 3);
        // 前 3 个通过
        for i in 0..3 {
            rule.evaluate(&join_ctx(&format!("u{}", i)), &store).await;
        }
        // 第 4 个被封禁
        let d = rule.evaluate(&join_ctx("u_raider"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
        assert!(d.reason.unwrap().contains("Raid"));
    }

    #[tokio::test]
    async fn normal_message_ignored() {
        let store = LocalStore::new();
        let rule = RaidRule::new(60, 1);
        assert!(rule.evaluate(&normal_ctx(), &store).await.is_none());
    }

    #[tokio::test]
    async fn different_groups_independent() {
        let store = LocalStore::new();
        let rule = RaidRule::new(60, 2);

        // 群 g1: 2 人入群
        rule.evaluate(&join_ctx("u1"), &store).await;
        rule.evaluate(&join_ctx("u2"), &store).await;

        // 群 g2: 1 人入群 — 不应触发
        let mut ctx = join_ctx("u3");
        ctx.group_id = "g2".into();
        assert!(rule.evaluate(&ctx, &store).await.is_none());

        // 群 g1: 第 3 人 — 应触发
        let d = rule.evaluate(&join_ctx("u4"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
    }

    #[tokio::test]
    async fn zero_defaults() {
        let rule = RaidRule::new(0, 0);
        assert_eq!(rule.window_secs, 60);
        assert_eq!(rule.join_threshold, 10);
    }
}
