pub mod telegram;
pub mod discord;

use async_trait::async_trait;

use crate::error::BotResult;

/// 标准化平台事件
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PlatformEvent {
    pub platform: String,
    pub event_type: String,
    pub group_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub message_id: Option<String>,
    pub content: Option<String>,
    pub raw_event: serde_json::Value,
    pub timestamp: u64,
}

/// 消息上下文 (规则引擎输入)
#[derive(Debug, Clone)]
pub struct MessageContext {
    pub platform: String,
    pub group_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub message_text: String,
    pub message_id: Option<String>,
    pub is_command: bool,
    pub command: Option<String>,
    pub command_args: Vec<String>,
    pub is_join_request: bool,
    pub is_admin: bool,
    /// 消息类型标记 (photo, video, audio, document, sticker, animation, voice, forward, contact, location, poll)
    pub message_type: Option<String>,
    /// CallbackQuery ID (Inline 键盘回调)
    pub callback_query_id: Option<String>,
    /// CallbackQuery data
    pub callback_data: Option<String>,
}

/// 执行动作
#[derive(Debug, Clone)]
pub struct ExecuteAction {
    pub action_type: ActionType,
    pub group_id: String,
    pub target_user: String,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub duration_secs: Option<u64>,
    /// Inline 键盘 JSON (可选)
    pub inline_keyboard: Option<serde_json::Value>,
    /// CallbackQuery ID (用于 answerCallbackQuery)
    pub callback_query_id: Option<String>,
}

/// 动作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Kick = 0,
    Ban = 1,
    Mute = 2,
    Warn = 3,
    Unmute = 4,
    Unban = 5,
    DeleteMessage = 6,
    SendMessage = 7,
    PinMessage = 8,
    ApproveJoin = 9,
    DeclineJoin = 10,
    // Phase 4 新增
    Promote = 11,
    Demote = 12,
    SetPermissions = 13,
    EditMessage = 14,
    AnswerCallback = 15,
}

impl ActionType {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// 执行收据
#[derive(Debug, Clone)]
pub struct ExecutionReceipt {
    pub success: bool,
    pub action_type: ActionType,
    pub message_hash: [u8; 32],
    pub timestamp: u64,
}

/// 平台适配器 — 统一不同平台的事件解析
#[async_trait]
#[allow(dead_code)]
pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &'static str;
    fn parse_event(&self, raw: &serde_json::Value) -> Option<PlatformEvent>;
    fn extract_context(&self, event: &PlatformEvent) -> MessageContext;
}

/// 平台执行器 — 统一不同平台的管理动作
#[async_trait]
pub trait PlatformExecutor: Send + Sync {
    async fn execute(&self, action: &ExecuteAction) -> BotResult<ExecutionReceipt>;
}
