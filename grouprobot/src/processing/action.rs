use crate::platform::{ActionType, ExecuteAction};

/// 规则引擎判定结果
#[derive(Debug, Clone)]
pub struct ActionDecision {
    pub action_type: ActionType,
    pub target_user: String,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub duration_secs: Option<u64>,
}

impl ActionDecision {
    pub fn kick(target: &str, reason: &str) -> Self {
        Self {
            action_type: ActionType::Kick,
            target_user: target.to_string(),
            reason: Some(reason.to_string()),
            message: None,
            duration_secs: None,
        }
    }

    pub fn ban(target: &str, reason: &str) -> Self {
        Self {
            action_type: ActionType::Ban,
            target_user: target.to_string(),
            reason: Some(reason.to_string()),
            message: None,
            duration_secs: None,
        }
    }

    pub fn mute(target: &str, duration_secs: u64, reason: &str) -> Self {
        Self {
            action_type: ActionType::Mute,
            target_user: target.to_string(),
            reason: Some(reason.to_string()),
            message: None,
            duration_secs: Some(duration_secs),
        }
    }

    pub fn warn(target: &str, message: &str) -> Self {
        Self {
            action_type: ActionType::Warn,
            target_user: target.to_string(),
            reason: None,
            message: Some(message.to_string()),
            duration_secs: None,
        }
    }

    pub fn delete_message(message_id: &str) -> Self {
        Self {
            action_type: ActionType::DeleteMessage,
            target_user: message_id.to_string(),
            reason: None,
            message: None,
            duration_secs: None,
        }
    }

    pub fn send_message(group_id: &str, text: &str) -> Self {
        Self {
            action_type: ActionType::SendMessage,
            target_user: group_id.to_string(),
            reason: None,
            message: Some(text.to_string()),
            duration_secs: None,
        }
    }

    pub fn approve_join(user_id: &str) -> Self {
        Self {
            action_type: ActionType::ApproveJoin,
            target_user: user_id.to_string(),
            reason: None,
            message: None,
            duration_secs: None,
        }
    }

    pub fn unban(target: &str, reason: &str) -> Self {
        Self {
            action_type: ActionType::Unban,
            target_user: target.to_string(),
            reason: Some(reason.to_string()),
            message: None,
            duration_secs: None,
        }
    }

    pub fn promote(target: &str, reason: &str) -> Self {
        Self {
            action_type: ActionType::Promote,
            target_user: target.to_string(),
            reason: Some(reason.to_string()),
            message: None,
            duration_secs: None,
        }
    }

    pub fn demote(target: &str, reason: &str) -> Self {
        Self {
            action_type: ActionType::Demote,
            target_user: target.to_string(),
            reason: Some(reason.to_string()),
            message: None,
            duration_secs: None,
        }
    }

    pub fn answer_callback(callback_query_id: &str, text: &str) -> Self {
        Self {
            action_type: ActionType::AnswerCallback,
            target_user: callback_query_id.to_string(),
            reason: None,
            message: Some(text.to_string()),
            duration_secs: None,
        }
    }

    /// 转换为平台执行动作
    pub fn to_execute_action(&self, group_id: &str, channel_id: Option<&str>) -> ExecuteAction {
        ExecuteAction {
            action_type: self.action_type,
            group_id: group_id.to_string(),
            target_user: self.target_user.clone(),
            reason: self.reason.clone(),
            message: self.message.clone(),
            duration_secs: self.duration_secs,
            inline_keyboard: None,
            callback_query_id: None,
            channel_id: channel_id.map(|s| s.to_string()),
        }
    }
}

/// 规则判定
#[derive(Debug)]
pub struct RuleDecision {
    pub matched_rule: String,
    pub action: Option<ActionDecision>,
}
