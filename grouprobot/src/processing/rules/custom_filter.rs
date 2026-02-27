use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G4: 自定义过滤器规则
///
/// 关键词触发的自动回复/动作。
/// 配置格式 (CSV): `trigger|type|response` (每行一条)
///   - type: "reply" = 回复消息, "delete" = 删除消息, "warn" = 警告
///   - trigger: 大小写不敏感的关键词匹配
///
/// 例如:
///   `buy crypto|reply|请勿在群内发布交易信息`
///   `spam link|delete|`
///   `badword|warn|使用了禁止词汇`
pub struct CustomFilterRule {
    filters: Vec<FilterEntry>,
}

#[derive(Debug, Clone)]
struct FilterEntry {
    trigger: String,        // 小写关键词
    action_type: FilterAction,
    response: String,       // 回复文本 (可为空)
}

#[derive(Debug, Clone, PartialEq)]
enum FilterAction {
    Reply,
    Delete,
    Warn,
}

impl CustomFilterRule {
    pub fn new() -> Self {
        Self { filters: vec![] }
    }

    /// 从 CSV 格式加载过滤器
    /// 格式: `trigger|type|response` (换行分隔)
    pub fn from_csv(csv: &str) -> Self {
        let filters: Vec<FilterEntry> = csv.lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                if parts.len() < 2 {
                    return None;
                }
                let trigger = parts[0].trim().to_lowercase();
                if trigger.is_empty() {
                    return None;
                }
                let action_type = match parts[1].trim() {
                    "delete" => FilterAction::Delete,
                    "warn" => FilterAction::Warn,
                    _ => FilterAction::Reply, // 默认回复
                };
                let response = parts.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
                Some(FilterEntry { trigger, action_type, response })
            })
            .collect();
        Self { filters }
    }

    /// 过滤器数量
    pub fn count(&self) -> usize {
        self.filters.len()
    }
}

#[async_trait]
impl Rule for CustomFilterRule {
    fn name(&self) -> &'static str { "custom_filter" }

    async fn evaluate(&self, ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        // 命令/加入事件不检查
        if ctx.is_command || ctx.is_join_request || ctx.is_new_member || ctx.is_left_member {
            return None;
        }

        // 管理员豁免
        if ctx.is_admin {
            return None;
        }

        // 回调不检查
        if ctx.callback_query_id.is_some() {
            return None;
        }

        let text_lower = ctx.message_text.to_lowercase();
        if text_lower.is_empty() {
            return None;
        }

        for filter in &self.filters {
            if text_lower.contains(&filter.trigger) {
                return match filter.action_type {
                    FilterAction::Reply => {
                        if filter.response.is_empty() {
                            None
                        } else {
                            Some(ActionDecision::send_message(&ctx.group_id, &filter.response))
                        }
                    }
                    FilterAction::Delete => {
                        if let Some(ref msg_id) = ctx.message_id {
                            let mut d = ActionDecision::delete_message(msg_id);
                            if !filter.response.is_empty() {
                                d.message = Some(filter.response.clone());
                            }
                            Some(d)
                        } else {
                            None
                        }
                    }
                    FilterAction::Warn => {
                        let msg = if filter.response.is_empty() {
                            format!("⚠️ 触发过滤器: {}", filter.trigger)
                        } else {
                            filter.response.clone()
                        };
                        Some(ActionDecision::warn(&ctx.sender_id, &msg))
                    }
                };
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
            sender_id: "u1".into(),
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
    async fn reply_filter_triggers() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("buy crypto|reply|请勿发布交易信息");
        let d = rule.evaluate(&make_ctx("want to BUY CRYPTO here"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::SendMessage);
        assert_eq!(d.message.unwrap(), "请勿发布交易信息");
    }

    #[tokio::test]
    async fn delete_filter_triggers() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("spam link|delete|已删除垃圾消息");
        let d = rule.evaluate(&make_ctx("check this spam link"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
        assert_eq!(d.target_user, "msg_1");
        assert_eq!(d.message.unwrap(), "已删除垃圾消息");
    }

    #[tokio::test]
    async fn warn_filter_triggers() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("badword|warn|使用了禁止词汇");
        let d = rule.evaluate(&make_ctx("this is a badword"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
        assert_eq!(d.message.unwrap(), "使用了禁止词汇");
    }

    #[tokio::test]
    async fn warn_filter_default_message() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("trigger|warn|");
        let d = rule.evaluate(&make_ctx("has trigger here"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
        assert!(d.message.unwrap().contains("trigger"));
    }

    #[tokio::test]
    async fn no_match_passes() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("badword|warn|nope");
        assert!(rule.evaluate(&make_ctx("normal message"), &store).await.is_none());
    }

    #[tokio::test]
    async fn case_insensitive() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("SCAM|warn|scam detected");
        let d = rule.evaluate(&make_ctx("this is a scam"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::Warn);
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("badword|warn|nope");
        let mut ctx = make_ctx("badword here");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn multiple_filters_first_wins() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("foo|reply|hit foo\nbar|reply|hit bar");
        assert_eq!(rule.count(), 2);
        let d = rule.evaluate(&make_ctx("foo and bar"), &store).await.unwrap();
        assert_eq!(d.message.unwrap(), "hit foo"); // 第一条匹配
    }

    #[tokio::test]
    async fn empty_csv_no_filters() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("");
        assert_eq!(rule.count(), 0);
        assert!(rule.evaluate(&make_ctx("anything"), &store).await.is_none());
    }

    #[tokio::test]
    async fn malformed_lines_skipped() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("no_pipe\n|empty_trigger|reply\nvalid|warn|ok");
        assert_eq!(rule.count(), 1); // 只有 "valid|warn|ok" 有效
    }

    #[tokio::test]
    async fn reply_empty_response_passes() {
        let store = LocalStore::new();
        let rule = CustomFilterRule::from_csv("trigger|reply|");
        assert!(rule.evaluate(&make_ctx("trigger here"), &store).await.is_none());
    }
}
