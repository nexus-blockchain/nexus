use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G18: 违规追踪看板
///
/// 记录每个用户在每个群的违规历史 (按规则类型分类)。
/// 提供统计查询功能, 供管理员通过命令查看:
///   /violations @user — 查看用户违规统计
///   /leaderboard — 违规排行榜 (top 10)
///
/// 本模块作为后处理器 (post-processor) 运行:
/// - 不直接作为 Rule 参与规则链评估
/// - 由 RuleEngine 在规则链匹配后调用 `record_violation`
/// - 仅在管理员使用 /violations 或 /leaderboard 命令时作为 Rule 响应
///
/// 数据存储格式 (LocalStore):
/// - `violations:{group}:{user}:{rule}` → 违规计数
/// - `violations_total:{group}:{user}` → 用户总违规数
/// - `violations_users:{group}` → 逗号分隔的有违规记录的用户 ID 列表
/// - `violations_last:{group}:{user}` → 最后一次违规的规则名称
pub struct ViolationTracker;

impl ViolationTracker {
    /// 记录一次违规 (由 RuleEngine 在规则匹配后调用)
    pub fn record_violation(store: &LocalStore, group_id: &str, user_id: &str, rule_name: &str) {
        // 1. 按规则分类计数
        let rule_key = format!("violations:{}:{}:{}", group_id, user_id, rule_name);
        let rule_count: u64 = store.get_string(&rule_key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        store.set_string(&rule_key, &(rule_count + 1).to_string());

        // 2. 用户总违规计数
        let total_key = format!("violations_total:{}:{}", group_id, user_id);
        let total: u64 = store.get_string(&total_key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        store.set_string(&total_key, &(total + 1).to_string());

        // 3. 记录最后违规规则
        let last_key = format!("violations_last:{}:{}", group_id, user_id);
        store.set_string(&last_key, rule_name);

        // 4. 维护用户列表 (去重)
        let users_key = format!("violations_users:{}", group_id);
        let mut users: Vec<String> = store.get_string(&users_key)
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
            .unwrap_or_default();
        if !users.contains(&user_id.to_string()) {
            users.push(user_id.to_string());
            store.set_string(&users_key, &users.join(","));
        }
    }

    /// 获取用户总违规数
    pub fn get_total(store: &LocalStore, group_id: &str, user_id: &str) -> u64 {
        let key = format!("violations_total:{}:{}", group_id, user_id);
        store.get_string(&key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// 获取用户按规则分类的违规统计
    pub fn get_by_rule(store: &LocalStore, group_id: &str, user_id: &str) -> Vec<(String, u64)> {
        let rules = [
            "flood", "duplicate", "blacklist", "stop_word", "emoji",
            "link_limit", "similarity", "antiphishing", "lock",
            "profanity", "mention_flood", "new_member_audit",
            "cas", "homoglyph", "automod", "nsfw", "custom_filter",
        ];
        let mut result = vec![];
        for rule in &rules {
            let key = format!("violations:{}:{}:{}", group_id, user_id, rule);
            if let Some(count_str) = store.get_string(&key) {
                if let Ok(count) = count_str.parse::<u64>() {
                    if count > 0 {
                        result.push((rule.to_string(), count));
                    }
                }
            }
        }
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// 获取群组违规排行榜 (按总数降序, 取前 N 名)
    pub fn get_leaderboard(store: &LocalStore, group_id: &str, top_n: usize) -> Vec<(String, u64)> {
        let users_key = format!("violations_users:{}", group_id);
        let users: Vec<String> = store.get_string(&users_key)
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
            .unwrap_or_default();

        let mut entries: Vec<(String, u64)> = users.into_iter()
            .map(|uid| {
                let total = Self::get_total(store, group_id, &uid);
                (uid, total)
            })
            .filter(|(_, t)| *t > 0)
            .collect();

        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(top_n);
        entries
    }

    /// 重置用户的所有违规记录 (管理员 /resetviolations 命令)
    pub fn reset_user(store: &LocalStore, group_id: &str, user_id: &str) {
        let rules = [
            "flood", "duplicate", "blacklist", "stop_word", "emoji",
            "link_limit", "similarity", "antiphishing", "lock",
            "profanity", "mention_flood", "new_member_audit",
            "cas", "homoglyph", "automod", "nsfw", "custom_filter",
        ];
        for rule in &rules {
            let key = format!("violations:{}:{}:{}", group_id, user_id, rule);
            store.remove_string(&key);
        }
        let total_key = format!("violations_total:{}:{}", group_id, user_id);
        store.remove_string(&total_key);
        let last_key = format!("violations_last:{}:{}", group_id, user_id);
        store.remove_string(&last_key);
    }

    /// 格式化用户违规统计为可读文本
    pub fn format_user_stats(store: &LocalStore, group_id: &str, user_id: &str) -> String {
        let total = Self::get_total(store, group_id, user_id);
        if total == 0 {
            return format!("📊 用户 {} 无违规记录", user_id);
        }

        let by_rule = Self::get_by_rule(store, group_id, user_id);
        let last_key = format!("violations_last:{}:{}", group_id, user_id);
        let last_rule = store.get_string(&last_key).unwrap_or_default();

        let mut lines = vec![
            format!("📊 用户 {} 违规统计:", user_id),
            format!("  总计: {} 次", total),
        ];
        for (rule, count) in &by_rule {
            lines.push(format!("  • {}: {} 次", rule, count));
        }
        if !last_rule.is_empty() {
            lines.push(format!("  最近: {}", last_rule));
        }
        lines.join("\n")
    }

    /// 格式化排行榜为可读文本
    pub fn format_leaderboard(store: &LocalStore, group_id: &str) -> String {
        let entries = Self::get_leaderboard(store, group_id, 10);
        if entries.is_empty() {
            return "📊 暂无违规记录".to_string();
        }

        let mut lines = vec!["📊 违规排行榜 (Top 10):".to_string()];
        for (i, (uid, count)) in entries.iter().enumerate() {
            let medal = match i {
                0 => "🥇",
                1 => "🥈",
                2 => "🥉",
                _ => "  ",
            };
            lines.push(format!("{} {}. {} — {} 次", medal, i + 1, uid, count));
        }
        lines.join("\n")
    }
}

/// ViolationTracker 作为 Rule 仅响应管理员查询命令
/// (/violations, /leaderboard, /resetviolations)
#[async_trait]
impl Rule for ViolationTracker {
    fn name(&self) -> &'static str { "violation_tracker" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        // 仅响应管理员命令
        if !ctx.is_command || !ctx.is_admin {
            return None;
        }

        let cmd = ctx.command.as_deref().unwrap_or("");

        match cmd {
            "violations" => {
                let target = ctx.command_args.first()
                    .map(|s| s.trim_start_matches('@').to_string())
                    .unwrap_or_else(|| ctx.sender_id.clone());
                let text = Self::format_user_stats(store, &ctx.group_id, &target);
                Some(ActionDecision::send_message(&ctx.group_id, &text))
            }
            "leaderboard" => {
                let text = Self::format_leaderboard(store, &ctx.group_id);
                Some(ActionDecision::send_message(&ctx.group_id, &text))
            }
            "resetviolations" => {
                if let Some(target) = ctx.command_args.first() {
                    let uid = target.trim_start_matches('@');
                    Self::reset_user(store, &ctx.group_id, uid);
                    Some(ActionDecision::send_message(
                        &ctx.group_id,
                        &format!("✅ 已重置用户 {} 的违规记录", uid),
                    ))
                } else {
                    Some(ActionDecision::send_message(
                        &ctx.group_id,
                        "用法: /resetviolations @username",
                    ))
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn admin_cmd(cmd: &str, args: Vec<&str>) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "admin1".into(),
            sender_name: "Admin".into(),
            message_text: format!("/{} {}", cmd, args.join(" ")),
            message_id: Some("msg_1".into()),
            is_command: true,
            command: Some(cmd.into()),
            command_args: args.into_iter().map(|s| s.to_string()).collect(),
            is_join_request: false,
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin: true,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
        }
    }

    #[test]
    fn record_and_get_total() {
        let store = LocalStore::new();
        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 0);

        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 1);

        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "blacklist");
        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 3);
    }

    #[test]
    fn get_by_rule_sorted() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "blacklist");

        let by_rule = ViolationTracker::get_by_rule(&store, "g1", "u1");
        assert_eq!(by_rule.len(), 2);
        assert_eq!(by_rule[0], ("flood".to_string(), 3));
        assert_eq!(by_rule[1], ("blacklist".to_string(), 1));
    }

    #[test]
    fn leaderboard() {
        let store = LocalStore::new();
        // u1: 5 violations
        for _ in 0..5 {
            ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        }
        // u2: 3 violations
        for _ in 0..3 {
            ViolationTracker::record_violation(&store, "g1", "u2", "blacklist");
        }
        // u3: 1 violation
        ViolationTracker::record_violation(&store, "g1", "u3", "emoji");

        let lb = ViolationTracker::get_leaderboard(&store, "g1", 10);
        assert_eq!(lb.len(), 3);
        assert_eq!(lb[0], ("u1".to_string(), 5));
        assert_eq!(lb[1], ("u2".to_string(), 3));
        assert_eq!(lb[2], ("u3".to_string(), 1));
    }

    #[test]
    fn leaderboard_top_n() {
        let store = LocalStore::new();
        for i in 0..20 {
            let uid = format!("u{}", i);
            for _ in 0..(20 - i) {
                ViolationTracker::record_violation(&store, "g1", &uid, "flood");
            }
        }
        let lb = ViolationTracker::get_leaderboard(&store, "g1", 5);
        assert_eq!(lb.len(), 5);
    }

    #[test]
    fn reset_user_clears() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "blacklist");
        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 2);

        ViolationTracker::reset_user(&store, "g1", "u1");
        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 0);
        assert!(ViolationTracker::get_by_rule(&store, "g1", "u1").is_empty());
    }

    #[test]
    fn different_groups_independent() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g2", "u1", "flood");

        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 1);
        assert_eq!(ViolationTracker::get_total(&store, "g2", "u1"), 1);
    }

    #[test]
    fn format_user_stats_empty() {
        let store = LocalStore::new();
        let text = ViolationTracker::format_user_stats(&store, "g1", "u1");
        assert!(text.contains("无违规记录"));
    }

    #[test]
    fn format_user_stats_with_data() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");
        ViolationTracker::record_violation(&store, "g1", "u1", "blacklist");

        let text = ViolationTracker::format_user_stats(&store, "g1", "u1");
        assert!(text.contains("总计: 3 次"));
        assert!(text.contains("flood: 2 次"));
        assert!(text.contains("blacklist: 1 次"));
    }

    #[test]
    fn format_leaderboard_empty() {
        let store = LocalStore::new();
        let text = ViolationTracker::format_leaderboard(&store, "g1");
        assert!(text.contains("暂无违规记录"));
    }

    #[tokio::test]
    async fn violations_command() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "target_user", "flood");
        ViolationTracker::record_violation(&store, "g1", "target_user", "flood");

        let tracker = ViolationTracker;
        let d = tracker.evaluate(&admin_cmd("violations", vec!["target_user"]), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::SendMessage);
        assert!(d.message.unwrap().contains("总计: 2 次"));
    }

    #[tokio::test]
    async fn leaderboard_command() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");

        let tracker = ViolationTracker;
        let d = tracker.evaluate(&admin_cmd("leaderboard", vec![]), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::SendMessage);
        assert!(d.message.unwrap().contains("排行榜"));
    }

    #[tokio::test]
    async fn resetviolations_command() {
        let store = LocalStore::new();
        ViolationTracker::record_violation(&store, "g1", "u1", "flood");

        let tracker = ViolationTracker;
        let d = tracker.evaluate(&admin_cmd("resetviolations", vec!["u1"]), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::SendMessage);
        assert!(d.message.unwrap().contains("已重置"));
        assert_eq!(ViolationTracker::get_total(&store, "g1", "u1"), 0);
    }

    #[tokio::test]
    async fn non_admin_ignored() {
        let store = LocalStore::new();
        let tracker = ViolationTracker;
        let mut ctx = admin_cmd("violations", vec!["u1"]);
        ctx.is_admin = false;
        assert!(tracker.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn non_command_ignored() {
        let store = LocalStore::new();
        let tracker = ViolationTracker;
        let mut ctx = admin_cmd("violations", vec!["u1"]);
        ctx.is_command = false;
        assert!(tracker.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn unknown_command_ignored() {
        let store = LocalStore::new();
        let tracker = ViolationTracker;
        assert!(tracker.evaluate(&admin_cmd("unknown", vec![]), &store).await.is_none());
    }
}
