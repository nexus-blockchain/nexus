# 平台适配层

> 版本: 0.1.0 | 最后更新: 2026-02-23

## 1. 架构设计

平台适配层采用 **Adapter + Executor** 双 trait 模式，将平台差异完全封装，使规则引擎和消息路由器对平台无感知。

```
                    ┌──────────────────────┐
                    │   PlatformAdapter    │ ← 事件解析 (纯同步)
                    │  ├ platform_name()   │
                    │  ├ parse_event()     │   raw JSON → PlatformEvent
                    │  └ extract_context() │   PlatformEvent → MessageContext
                    └──────────────────────┘
                              ↓
                    ┌──────────────────────┐
                    │   MessageRouter      │ ← 平台无关的核心流水线
                    │   (RuleEngine)       │
                    └──────────────────────┘
                              ↓
                    ┌──────────────────────┐
                    │  PlatformExecutor    │ ← 动作执行 (async)
                    │  └ execute()         │   ExecuteAction → ExecutionReceipt
                    └──────────────────────┘
```

### 核心接口定义

```rust
// platform/mod.rs

pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &'static str;
    fn parse_event(&self, raw: &serde_json::Value) -> Option<PlatformEvent>;
    fn extract_context(&self, event: &PlatformEvent) -> MessageContext;
}

#[async_trait]
pub trait PlatformExecutor: Send + Sync {
    async fn execute(&self, action: &ExecuteAction) -> BotResult<ExecutionReceipt>;
}
```

## 2. 统一数据模型

### PlatformEvent — 标准化平台事件

| 字段 | 类型 | 说明 |
|------|------|------|
| `platform` | String | `"telegram"` / `"discord"` |
| `event_type` | String | `"message"`, `"command"`, `"join_request"`, `"callback_query"`, `"member_join"` |
| `group_id` | String | TG: chat.id; DC: guild_id |
| `sender_id` | String | TG: from.id; DC: author.id |
| `sender_name` | String | TG: first_name; DC: username |
| `message_id` | Option\<String\> | 消息 ID |
| `content` | Option\<String\> | 文本内容 |
| `raw_event` | serde_json::Value | 原始 JSON (规则可访问) |
| `timestamp` | u64 | Unix 时间戳 |

### MessageContext — 规则引擎输入

| 字段 | 类型 | 说明 |
|------|------|------|
| `platform` | String | 平台标识 |
| `group_id` | String | 群组 ID |
| `sender_id` | String | 发送者 ID |
| `sender_name` | String | 发送者名称 |
| `message_text` | String | 消息文本 |
| `message_id` | Option\<String\> | 消息 ID |
| `is_command` | bool | 是否为命令 |
| `command` | Option\<String\> | 命令名 (去掉 `/` 前缀和 `@bot` 后缀) |
| `command_args` | Vec\<String\> | 命令参数 |
| `is_join_request` | bool | 是否为入群请求 |
| `is_admin` | bool | 发送者是否管理员 (API 查询) |
| `message_type` | Option\<String\> | 消息类型标记 (仅 TG) |
| `callback_query_id` | Option\<String\> | 回调查询 ID (仅 TG) |
| `callback_data` | Option\<String\> | 回调数据 (仅 TG) |

### ExecuteAction — 执行动作

| 字段 | 类型 | 说明 |
|------|------|------|
| `action_type` | ActionType | 动作类型枚举 |
| `group_id` | String | 目标群组 |
| `target_user` | String | 目标用户 ID |
| `reason` | Option\<String\> | 原因 (DC 写入审计日志) |
| `message` | Option\<String\> | 消息内容 (SendMessage 用) |
| `duration_secs` | Option\<u64\> | 禁言时长 |
| `inline_keyboard` | Option\<Value\> | Inline 键盘 (仅 TG) |
| `callback_query_id` | Option\<String\> | 回调查询 ID (仅 TG) |

### ActionType 枚举 (16 种)

| 值 | 名称 | 说明 |
|----|------|------|
| 0 | `Kick` | 踢出 (允许重新加入) |
| 1 | `Ban` | 永久封禁 |
| 2 | `Mute` | 禁言 (限时) |
| 3 | `Warn` | 警告 (发送消息) |
| 4 | `Unmute` | 解除禁言 |
| 5 | `Unban` | 解除封禁 |
| 6 | `DeleteMessage` | 删除消息 |
| 7 | `SendMessage` | 发送消息 |
| 8 | `PinMessage` | 置顶消息 |
| 9 | `ApproveJoin` | 批准入群 |
| 10 | `DeclineJoin` | 拒绝入群 |
| 11 | `Promote` | 提升管理员 |
| 12 | `Demote` | 降级管理员 |
| 13 | `SetPermissions` | 设置群权限 |
| 14 | `EditMessage` | 编辑消息 |
| 15 | `AnswerCallback` | 回答回调查询 |

## 3. Telegram 适配

### 3.1 TelegramAdapter

**事件接收**: Webhook (`POST /webhook`)

**支持的事件类型**:

| Telegram Update 字段 | 映射 event_type | 说明 |
|----------------------|-----------------|------|
| `message` | `"message"` / `"command"` | 普通消息 / 命令 (以 `/` 开头) |
| `edited_message` | `"message"` / `"command"` | 编辑后的消息 |
| `chat_join_request` | `"join_request"` | 入群申请 |
| `callback_query` | `"callback_query"` | Inline 键盘回调 |

**消息类型检测** (`detect_message_type`):

支持 13 种消息类型识别: `photo`, `video`, `audio`, `document`, `sticker`, `animation`, `voice`, `video_note`, `contact`, `location`, `poll`, `game`, `forward`。用于 `LockRule` 消息类型锁定。

**命令解析**: 自动去除 `@botname` 后缀 (例如 `/ban@mybot 789` → command=`ban`, args=[`789`])。

### 3.2 TelegramExecutor

**认证方式**: Bot Token 嵌入 URL (`https://api.telegram.org/bot<TOKEN>/method`)

**Token 安全**: Token 由 `VaultProvider` 提供，URL 使用 `Zeroizing<String>` 自动清零。

**API 方法** (20 个):

| 类别 | 方法 | Telegram API |
|------|------|-------------|
| **基础** | `send_message` | `sendMessage` (HTML parse_mode) |
| | `delete_message` | `deleteMessage` |
| | `delete_messages` | `deleteMessages` (批量) |
| **用户管理** | `ban_user` | `banChatMember` |
| | `kick_user` | `banChatMember` → `unbanChatMember` |
| | `unban_user` | `unbanChatMember` (only_if_banned) |
| | `mute_user` | `restrictChatMember` (所有权限=false) |
| | `unmute_user` | `restrictChatMember` (所有权限=true) |
| **管理员** | `promote_member` | `promoteChatMember` (5 项权限) |
| | `demote_member` | `promoteChatMember` (所有权限=false) |
| | `is_admin_in_chat` | `getChatMember` (status=admin/creator) |
| | `get_chat_administrators` | `getChatAdministrators` |
| **群设置** | `set_chat_permissions` | `setChatPermissions` |
| | `register_webhook` | `setWebhook` (secret_token, 40 连接) |
| **入群审批** | `approve_join` | `approveChatJoinRequest` |
| | `decline_join` | `declineChatJoinRequest` |
| **交互** | `send_message_with_keyboard` | `sendMessage` + `reply_markup` |
| | `answer_callback_query` | `answerCallbackQuery` |
| | `edit_message_text` | `editMessageText` |

**Mute 权限细粒度控制**: 禁言时设置以下 9 项权限为 false:
`can_send_messages`, `can_send_audios`, `can_send_documents`, `can_send_photos`, `can_send_videos`, `can_send_video_notes`, `can_send_voice_notes`, `can_send_polls`, `can_send_other_messages`, `can_add_web_page_previews`

## 4. Discord 适配

### 4.1 DiscordAdapter

**事件接收**: Gateway WebSocket (实时推送)

**支持的事件类型**:

| Discord Gateway 事件 | 映射 event_type | 说明 |
|---------------------|-----------------|------|
| `MESSAGE_CREATE` | `"message"` / `"command"` | 文本消息 (自动过滤 Bot 消息) |
| `GUILD_MEMBER_ADD` | `"member_join"` | 新成员加入 |

**命令前缀**: 支持 `/` 和 `!` 双前缀 (TG 仅支持 `/`)。

**Bot 消息过滤**: `author.bot == true` 的消息自动跳过，防止 Bot 循环触发。

**ID 体系差异**: Discord 使用字符串 ID (`"123456"`), Telegram 使用数字 ID (但统一转为字符串存储)。`guild_id` 对应 TG 的 `chat.id`。

### 4.2 DiscordExecutor

**认证方式**: `Authorization: Bot <TOKEN>` HTTP Header (由 `VaultProvider::build_dc_auth_header` 提供)。

**API 基址**: `https://discord.com/api/v10`

**API 方法** (6 个):

| 类别 | 方法 | Discord REST API |
|------|------|-----------------|
| **消息** | `send_message` | `POST /channels/{id}/messages` |
| | `delete_message` | `DELETE /channels/{id}/messages/{id}` |
| **用户管理** | `ban_member` | `PUT /guilds/{id}/bans/{id}` + `X-Audit-Log-Reason` |
| | `kick_member` | `DELETE /guilds/{id}/members/{id}` |
| | `timeout_member` | `PATCH /guilds/{id}/members/{id}` (communication_disabled_until) |
| | `remove_timeout` | `PATCH /guilds/{id}/members/{id}` (null) |

**Discord 特有功能**:
- **审计日志原因**: Ban 操作通过 `X-Audit-Log-Reason` Header 记录原因到 Discord 原生审计日志
- **Timeout**: Discord 使用 ISO 8601 时间戳 (`communication_disabled_until`) 而非 Unix 时间戳

### 4.3 DiscordGateway

独立 WebSocket 连接到 Discord Gateway, 接收实时事件并通过 `mpsc::channel` 分发给消息处理器:

```rust
// main.rs 中的 Discord 事件循环
let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(256);
tokio::spawn(async move {
    let gw = DiscordGateway::new(vault, intents, event_tx);
    gw.run().await;
});
// 消费事件
while let Some(event) = event_rx.recv().await {
    let ctx = adapter.extract_context(&event);
    router.handle_event(&ctx, dc_executor).await;
}
```

## 5. Telegram vs Discord 对比

| 维度 | Telegram | Discord |
|------|----------|---------|
| **事件接收** | Webhook (HTTP POST) | Gateway (WebSocket) |
| **认证** | Token 嵌入 URL 路径 | Authorization Header |
| **API 风格** | 自定义 JSON RPC | REST (HTTP method + path) |
| **成功判断** | `{"ok": true}` | HTTP 2xx / 204 |
| **群组 ID** | 数字 (负数, 如 `-100123`) | Snowflake 字符串 |
| **禁言实现** | `restrictChatMember` (10 项权限) | `PATCH member` (timeout 时间戳) |
| **踢出实现** | Ban → 立即 Unban | DELETE member |
| **命令前缀** | `/` 仅此一种 | `/` 和 `!` 双前缀 |
| **消息类型检测** | 13 种媒体类型 | 暂未实现 (message_type=None) |
| **回调查询** | Inline Keyboard + callback_query | Interaction (未实现) |
| **入群审批** | `chat_join_request` (approve/decline) | 无原生审批 API |
| **审计日志** | 自建 (AuditLogger) | 原生 X-Audit-Log-Reason |
| **Bot 消息过滤** | Webhook 不推送自身消息 | 需手动检查 `author.bot` |
| **API 方法数** | 20 | 6 |
| **功能覆盖** | 完整 (所有 16 种 ActionType) | 基础 (5 种: Kick/Ban/Mute/Unmute/Send) |

### Discord 待实现功能

| ActionType | 当前状态 | 对应 Discord API |
|-----------|---------|-----------------|
| `Unban` | ❌ | `DELETE /guilds/{id}/bans/{id}` |
| `PinMessage` | ❌ | `PUT /channels/{id}/pins/{id}` |
| `Promote` | ❌ | `PUT /guilds/{id}/members/{id}/roles/{id}` |
| `Demote` | ❌ | `DELETE /guilds/{id}/members/{id}/roles/{id}` |
| `EditMessage` | ❌ | `PATCH /channels/{id}/messages/{id}` |
| `AnswerCallback` | ❌ | Discord Interactions API |
| `SetPermissions` | ❌ | Channel permission overwrites |

## 6. 扩展新平台

添加新平台 (如 Slack, Matrix) 只需实现两个 trait:

1. **创建目录** `platform/<name>/`
2. **实现 `PlatformAdapter`** — 解析平台特有的 JSON 事件格式
3. **实现 `PlatformExecutor`** — 调用平台 API 执行管理动作
4. **在 `main.rs` 中注册** — 创建执行器实例, 注入 AppState

规则引擎和消息路由器**无需任何修改**, 因为它们只依赖 `MessageContext` 和 `PlatformExecutor` 抽象接口。
