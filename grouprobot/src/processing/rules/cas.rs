use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G26: CAS (Combot Anti-Spam) API 集成
///
/// 新成员入群时，检查其是否在 CAS 数据库中被标记为垃圾用户。
/// CAS API: `https://api.cas.chat/check?user_id=<id>`
///
/// 由于规则引擎是同步评估模式 (不做 HTTP 请求)，
/// 本规则采用缓存策略:
/// - 维护一个已知垃圾用户 ID 的本地缓存 (LocalStore)
/// - 外部同步任务定期查询 CAS API 并更新缓存
/// - 规则本身只检查缓存
///
/// 也支持直接加载已知的 CAS 封禁 ID 列表。
pub struct CasRule {
    /// 是否对新成员执行检查
    check_on_join: bool,
    /// 是否对普通消息执行检查
    check_on_message: bool,
}

impl CasRule {
    pub fn new() -> Self {
        Self {
            check_on_join: true,
            check_on_message: false,
        }
    }

    pub fn with_message_check() -> Self {
        Self {
            check_on_join: true,
            check_on_message: true,
        }
    }

    /// 标记用户为 CAS 封禁 (由外部同步任务调用)
    pub fn mark_banned(store: &LocalStore, user_id: &str) {
        let key = format!("cas_banned:{}", user_id);
        store.set_string(&key, "1");
    }

    /// 移除 CAS 封禁标记
    pub fn unmark_banned(store: &LocalStore, user_id: &str) {
        let key = format!("cas_banned:{}", user_id);
        store.remove_string(&key);
    }

    /// 检查用户是否被 CAS 封禁
    pub fn is_cas_banned(store: &LocalStore, user_id: &str) -> bool {
        let key = format!("cas_banned:{}", user_id);
        store.get_string(&key).is_some()
    }

    /// 批量加载 CAS 封禁列表
    pub fn load_banned_list(store: &LocalStore, user_ids: &[&str]) {
        for id in user_ids {
            Self::mark_banned(store, id);
        }
    }
}

#[async_trait]
impl Rule for CasRule {
    fn name(&self) -> &'static str { "cas" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        // 管理员豁免
        if ctx.is_admin {
            return None;
        }

        // 回调不检查
        if ctx.callback_query_id.is_some() {
            return None;
        }

        // 检查场景
        let should_check = if ctx.is_new_member || ctx.is_join_request {
            self.check_on_join
        } else if !ctx.is_command && !ctx.is_left_member {
            self.check_on_message
        } else {
            false
        };

        if !should_check {
            return None;
        }

        if Self::is_cas_banned(store, &ctx.sender_id) {
            if ctx.is_new_member || ctx.is_join_request {
                return Some(ActionDecision::ban(
                    &ctx.sender_id,
                    "CAS: 用户在 Combot Anti-Spam 数据库中被标记为垃圾用户",
                ));
            }
            // 普通消息: 封禁
            return Some(ActionDecision::ban(
                &ctx.sender_id,
                "CAS: 用户在 Combot Anti-Spam 数据库中被标记为垃圾用户",
            ));
        }

        None
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

    fn msg_ctx(sender: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: sender.into(),
            sender_name: "User".into(),
            message_text: "hello".into(),
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
            channel_id: None,
        }
    }

    #[tokio::test]
    async fn cas_banned_new_member_gets_banned() {
        let store = LocalStore::new();
        CasRule::mark_banned(&store, "spammer123");
        let rule = CasRule::new();
        let d = rule.evaluate(&join_ctx("spammer123"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
        assert!(d.reason.unwrap().contains("CAS"));
    }

    #[tokio::test]
    async fn clean_new_member_passes() {
        let store = LocalStore::new();
        let rule = CasRule::new();
        assert!(rule.evaluate(&join_ctx("good_user"), &store).await.is_none());
    }

    #[tokio::test]
    async fn cas_banned_message_default_no_check() {
        let store = LocalStore::new();
        CasRule::mark_banned(&store, "spammer123");
        let rule = CasRule::new(); // check_on_message = false
        assert!(rule.evaluate(&msg_ctx("spammer123"), &store).await.is_none());
    }

    #[tokio::test]
    async fn cas_banned_message_with_check() {
        let store = LocalStore::new();
        CasRule::mark_banned(&store, "spammer123");
        let rule = CasRule::with_message_check();
        let d = rule.evaluate(&msg_ctx("spammer123"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Ban);
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        CasRule::mark_banned(&store, "admin1");
        let rule = CasRule::new();
        let mut ctx = join_ctx("admin1");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn unmark_works() {
        let store = LocalStore::new();
        CasRule::mark_banned(&store, "user1");
        assert!(CasRule::is_cas_banned(&store, "user1"));
        CasRule::unmark_banned(&store, "user1");
        assert!(!CasRule::is_cas_banned(&store, "user1"));
    }

    #[tokio::test]
    async fn bulk_load() {
        let store = LocalStore::new();
        CasRule::load_banned_list(&store, &["u1", "u2", "u3"]);
        assert!(CasRule::is_cas_banned(&store, "u1"));
        assert!(CasRule::is_cas_banned(&store, "u2"));
        assert!(CasRule::is_cas_banned(&store, "u3"));
        assert!(!CasRule::is_cas_banned(&store, "u4"));
    }
}
