use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G1: CAPTCHA 验证规则
///
/// 新成员入群 → 发送带 Inline Keyboard 的验证消息 → 等待点击正确按钮
/// 回调 `captcha_pass:<user_id>` → 解除限制
/// 超时由外部定时器处理 (检查 `captcha_pending:<group>:<user>` 未消费 → 踢出)
pub struct CaptchaRule {
    timeout_secs: u64,
}

impl CaptchaRule {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs: if timeout_secs == 0 { 120 } else { timeout_secs } }
    }

    /// 生成简单算术验证题
    fn generate_challenge() -> (String, String, Vec<(String, String)>) {
        // 使用时间戳低位生成伪随机数
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let a = (now % 10 + 1) as u32;
        let b = ((now / 10) % 10 + 1) as u32;
        let correct = a + b;
        let question = format!("🔒 请点击 {} + {} 的结果以通过验证:", a, b);

        // 生成 4 个选项 (含正确答案)
        let mut options: Vec<u32> = vec![correct];
        let mut offset = 1u32;
        while options.len() < 4 {
            let candidate = if offset % 2 == 0 { correct + offset } else { correct.saturating_sub(offset).max(1) };
            if !options.contains(&candidate) {
                options.push(candidate);
            }
            offset += 1;
        }
        // 简单洗牌 (基于时间戳)
        let seed = (now / 100) as usize;
        for i in 0..options.len() {
            let j = (seed + i * 7) % options.len();
            options.swap(i, j);
        }

        let buttons: Vec<(String, String)> = options.iter().map(|&v| {
            let data = if v == correct {
                format!("captcha_pass:{}", v)
            } else {
                format!("captcha_fail:{}", v)
            };
            (v.to_string(), data)
        }).collect();

        (question, correct.to_string(), buttons)
    }
}

#[async_trait]
impl Rule for CaptchaRule {
    fn name(&self) -> &'static str { "captcha" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        // ── 处理回调: captcha_pass / captcha_fail ──
        if let (Some(ref cb_id), Some(ref cb_data)) = (&ctx.callback_query_id, &ctx.callback_data) {
            if cb_data.starts_with("captcha_pass:") {
                // 验证通过 — 移除待验证标记
                let pending_key = format!("captcha_pending:{}:{}", ctx.group_id, ctx.sender_id);
                store.remove_string(&pending_key);
                return Some(ActionDecision::answer_callback(cb_id, "✅ 验证通过!"));
            }
            if cb_data.starts_with("captcha_fail:") {
                return Some(ActionDecision::answer_callback(cb_id, "❌ 回答错误，请重试"));
            }
            return None;
        }

        // ── 新成员入群 → 发送验证题 ──
        if !ctx.is_new_member {
            // 检查是否是待验证用户发消息 → 删除 (验证通过前不允许发言)
            let pending_key = format!("captcha_pending:{}:{}", ctx.group_id, ctx.sender_id);
            if store.get_string(&pending_key).is_some() && !ctx.message_text.is_empty() {
                if let Some(ref msg_id) = ctx.message_id {
                    return Some(ActionDecision::delete_message(msg_id));
                }
            }
            return None;
        }

        // 标记为待验证
        let pending_key = format!("captcha_pending:{}:{}", ctx.group_id, ctx.sender_id);
        store.set_string(&pending_key, &self.timeout_secs.to_string());

        // 生成验证题
        let (question, _answer, buttons) = Self::generate_challenge();
        let text = format!(
            "👋 欢迎 {}!\n\n{}\n\n⏰ 请在 {} 秒内完成验证，否则将被移出群聊。",
            ctx.sender_name, question, self.timeout_secs
        );

        // 构建 Inline Keyboard JSON
        let keyboard_buttons: Vec<serde_json::Value> = buttons.iter().map(|(label, data)| {
            serde_json::json!({ "text": label, "callback_data": data })
        }).collect();
        let keyboard = serde_json::json!({
            "inline_keyboard": [keyboard_buttons]
        });

        let mut decision = ActionDecision::send_message(&ctx.group_id, &text);
        decision.duration_secs = Some(self.timeout_secs); // 供外部定时器参考
        // 将 keyboard 附加到 ExecuteAction (通过 action.rs 的 inline_keyboard 字段传递)
        // 我们在这里用一种约定: 将 keyboard JSON 编码到 reason 字段, router 转换时提取
        // 更好的方式: 直接在 ActionDecision 中增加 inline_keyboard 字段
        decision.reason = Some(serde_json::to_string(&keyboard).unwrap_or_default());

        Some(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn new_member_ctx() -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "new_user".into(),
            sender_name: "NewUser".into(),
            message_text: String::new(),
            message_id: Some("svc_100".into()),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: true,
            is_left_member: false,
            service_message_id: Some("svc_100".into()),
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
        }
    }

    fn normal_ctx(sender: &str) -> MessageContext {
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
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
        }
    }

    fn callback_ctx(data: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "new_user".into(),
            sender_name: "NewUser".into(),
            message_text: String::new(),
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
            callback_query_id: Some("cb_1".into()),
            callback_data: Some(data.into()),
        }
    }

    #[tokio::test]
    async fn new_member_sends_captcha() {
        let store = LocalStore::new();
        let rule = CaptchaRule::new(120);
        let d = rule.evaluate(&new_member_ctx(), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::SendMessage);
        assert!(d.message.unwrap().contains("验证"));
        // 应标记为 pending
        assert!(store.get_string("captcha_pending:g1:new_user").is_some());
    }

    #[tokio::test]
    async fn pending_user_message_deleted() {
        let store = LocalStore::new();
        store.set_string("captcha_pending:g1:new_user", "120");
        let rule = CaptchaRule::new(120);
        let d = rule.evaluate(&normal_ctx("new_user"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn non_pending_user_passes() {
        let store = LocalStore::new();
        let rule = CaptchaRule::new(120);
        assert!(rule.evaluate(&normal_ctx("other_user"), &store).await.is_none());
    }

    #[tokio::test]
    async fn captcha_pass_callback() {
        let store = LocalStore::new();
        store.set_string("captcha_pending:g1:new_user", "120");
        let rule = CaptchaRule::new(120);
        let d = rule.evaluate(&callback_ctx("captcha_pass:5"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::AnswerCallback);
        assert!(d.message.unwrap().contains("通过"));
        // pending 应被清除
        assert!(store.get_string("captcha_pending:g1:new_user").is_none());
    }

    #[tokio::test]
    async fn captcha_fail_callback() {
        let store = LocalStore::new();
        let rule = CaptchaRule::new(120);
        let d = rule.evaluate(&callback_ctx("captcha_fail:3"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::AnswerCallback);
        assert!(d.message.unwrap().contains("错误"));
    }

    #[tokio::test]
    async fn zero_timeout_defaults_to_120() {
        let rule = CaptchaRule::new(0);
        assert_eq!(rule.timeout_secs, 120);
    }
}
