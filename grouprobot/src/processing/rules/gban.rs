use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G2: 全局封禁规则
///
/// 维护一个跨群封禁名单 (从链上同步)。
/// 当封禁名单中的用户入群或发消息时，立即封禁。
pub struct GbanRule {
    /// 全局封禁用户 ID 集合 (线程安全, 可动态更新)
    banned_users: Arc<RwLock<HashSet<String>>>,
}

impl GbanRule {
    pub fn new() -> Self {
        Self {
            banned_users: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn with_users(users: Vec<String>) -> Self {
        let set: HashSet<String> = users.into_iter().filter(|u| !u.is_empty()).collect();
        Self {
            banned_users: Arc::new(RwLock::new(set)),
        }
    }

    /// 从 CSV 字符串加载封禁名单
    pub fn from_csv(csv: &str) -> Self {
        let users: Vec<String> = csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self::with_users(users)
    }

    /// 动态添加封禁用户
    pub fn add_user(&self, user_id: &str) {
        self.banned_users.write().unwrap().insert(user_id.to_string());
    }

    /// 动态移除封禁用户
    pub fn remove_user(&self, user_id: &str) {
        self.banned_users.write().unwrap().remove(user_id);
    }

    /// 检查用户是否在封禁名单中
    pub fn is_banned(&self, user_id: &str) -> bool {
        self.banned_users.read().unwrap().contains(user_id)
    }

    /// 获取封禁人数
    pub fn count(&self) -> usize {
        self.banned_users.read().unwrap().len()
    }

    /// 获取共享引用 (用于链上同步更新)
    pub fn shared_list(&self) -> Arc<RwLock<HashSet<String>>> {
        self.banned_users.clone()
    }
}

#[async_trait]
impl Rule for GbanRule {
    fn name(&self) -> &'static str { "gban" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        // 管理员豁免
        if ctx.is_admin {
            return None;
        }

        if !self.is_banned(&ctx.sender_id) {
            return None;
        }

        // 新成员入群 → 立即封禁
        if ctx.is_new_member || ctx.is_join_request {
            return Some(ActionDecision::ban(
                &ctx.sender_id,
                "Global ban: 用户在全局封禁名单中",
            ));
        }

        // 普通消息 → 删除消息 + 封禁
        // 返回 ban (executor 会同时处理)
        Some(ActionDecision::ban(
            &ctx.sender_id,
            "Global ban: 用户在全局封禁名单中",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn make_ctx(sender: &str, is_new: bool) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: sender.into(),
            sender_name: "Test".into(),
            message_text: "hello".into(),
            message_id: Some("msg_1".into()),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: is_new,
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
    async fn banned_user_gets_banned() {
        let store = LocalStore::new();
        let rule = GbanRule::with_users(vec!["bad_user".into()]);
        let d = rule.evaluate(&make_ctx("bad_user", false), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
    }

    #[tokio::test]
    async fn banned_new_member_gets_banned() {
        let store = LocalStore::new();
        let rule = GbanRule::with_users(vec!["bad_user".into()]);
        let d = rule.evaluate(&make_ctx("bad_user", true), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
    }

    #[tokio::test]
    async fn normal_user_passes() {
        let store = LocalStore::new();
        let rule = GbanRule::with_users(vec!["bad_user".into()]);
        assert!(rule.evaluate(&make_ctx("good_user", false), &store).await.is_none());
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = GbanRule::with_users(vec!["admin1".into()]);
        let mut ctx = make_ctx("admin1", false);
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn dynamic_add_remove() {
        let store = LocalStore::new();
        let rule = GbanRule::new();
        assert!(rule.evaluate(&make_ctx("user1", false), &store).await.is_none());

        rule.add_user("user1");
        assert!(rule.evaluate(&make_ctx("user1", false), &store).await.is_some());

        rule.remove_user("user1");
        assert!(rule.evaluate(&make_ctx("user1", false), &store).await.is_none());
    }

    #[tokio::test]
    async fn from_csv_works() {
        let store = LocalStore::new();
        let rule = GbanRule::from_csv("user1, user2, user3");
        assert_eq!(rule.count(), 3);
        assert!(rule.evaluate(&make_ctx("user2", false), &store).await.is_some());
        assert!(rule.evaluate(&make_ctx("user4", false), &store).await.is_none());
    }

    #[tokio::test]
    async fn empty_list_passes_all() {
        let store = LocalStore::new();
        let rule = GbanRule::new();
        assert_eq!(rule.count(), 0);
        assert!(rule.evaluate(&make_ctx("anyone", false), &store).await.is_none());
    }
}
