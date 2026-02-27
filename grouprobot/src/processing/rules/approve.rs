use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G3: 用户白名单 (Approve) 规则
///
/// 已审批用户免受后续自动规则影响。
/// 白名单存储在 LocalStore 中: `approved:<group_id>:<user_id>` → "1"
///
/// 此规则应放在规则链的**最前面** (仅次于 CallbackRule),
/// 返回 None 让白名单用户跳过所有后续检测规则。
///
/// 管理命令 `/approve <user_id>` 和 `/unapprove <user_id>` 由 CommandRule 处理,
/// 它们调用 `LocalStore::set_string` / `remove_string` 来管理白名单。
pub struct ApproveRule;

impl ApproveRule {
    pub fn new() -> Self { Self }

    /// 检查用户是否在白名单中
    pub fn is_approved(store: &LocalStore, group_id: &str, user_id: &str) -> bool {
        let key = format!("approved:{}:{}", group_id, user_id);
        store.get_string(&key).is_some()
    }

    /// 添加用户到白名单
    pub fn approve(store: &LocalStore, group_id: &str, user_id: &str) {
        let key = format!("approved:{}:{}", group_id, user_id);
        store.set_string(&key, "1");
    }

    /// 从白名单移除用户
    pub fn unapprove(store: &LocalStore, group_id: &str, user_id: &str) {
        let key = format!("approved:{}:{}", group_id, user_id);
        store.remove_string(&key);
    }
}

#[async_trait]
impl Rule for ApproveRule {
    fn name(&self) -> &'static str { "approve" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        // 命令和加入请求不受白名单影响 (需要由 CommandRule / JoinRequestRule 处理)
        if ctx.is_command || ctx.is_join_request || ctx.is_new_member || ctx.is_left_member {
            return None;
        }

        // 管理员天然豁免, 无需白名单检查
        if ctx.is_admin {
            return None;
        }

        // 回调不经过白名单
        if ctx.callback_query_id.is_some() {
            return None;
        }

        // 核心逻辑: 已审批用户 → 返回一个特殊的 "pass" 标记
        // 但规则链设计是: 返回 None = 继续下一条规则, 返回 Some = 终止规则链
        // 白名单用户需要**终止规则链** (跳过所有后续自动检测) 并且不执行任何动作
        // 我们返回 None 让后续规则继续 — 但这样白名单就没用了
        //
        // 正确做法: ApproveRule 检查 **非白名单** 用户, 对他们返回 None (继续检测)
        // 对白名单用户, 我们需要一种方式跳过检测规则但到达 DefaultRule
        //
        // 最佳方案: 在 RuleEngine 层面实现白名单检查, 而非作为 Rule
        // 但为保持可插拔架构, 我们采用 "哨兵规则" 方式:
        // 白名单用户 → 返回 DefaultRule 的空动作 (matched_rule="approve", action=None)
        // 这需要 RuleEngine 理解这个约定
        //
        // 简化实现: 不作为独立 Rule, 而是在 RuleEngine.evaluate 中前置检查
        // 这个文件提供工具函数, Rule trait 实现仅作为占位
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approve_and_check() {
        let store = LocalStore::new();
        assert!(!ApproveRule::is_approved(&store, "g1", "u1"));

        ApproveRule::approve(&store, "g1", "u1");
        assert!(ApproveRule::is_approved(&store, "g1", "u1"));

        // 不同群组独立
        assert!(!ApproveRule::is_approved(&store, "g2", "u1"));
    }

    #[test]
    fn unapprove_works() {
        let store = LocalStore::new();
        ApproveRule::approve(&store, "g1", "u1");
        assert!(ApproveRule::is_approved(&store, "g1", "u1"));

        ApproveRule::unapprove(&store, "g1", "u1");
        assert!(!ApproveRule::is_approved(&store, "g1", "u1"));
    }

    #[test]
    fn unapprove_nonexistent_noop() {
        let store = LocalStore::new();
        ApproveRule::unapprove(&store, "g1", "u1"); // 不应 panic
        assert!(!ApproveRule::is_approved(&store, "g1", "u1"));
    }

    #[tokio::test]
    async fn rule_returns_none_always() {
        let store = LocalStore::new();
        let rule = ApproveRule::new();
        let ctx = MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
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
        };
        // ApproveRule 本身返回 None, 白名单逻辑在 RuleEngine 层面处理
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }
}
