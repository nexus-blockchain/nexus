use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Sha256, Digest};
use tracing::{info, warn};

use crate::error::{BotError, BotResult};
use crate::platform::{PlatformExecutor, ExecuteAction, ExecutionReceipt, ActionType};
use crate::tee::vault_client::VaultProvider;

/// Telegram Bot API 执行器
pub struct TelegramExecutor {
    vault: Arc<dyn VaultProvider>,
    http: reqwest::Client,
}

impl TelegramExecutor {
    pub fn new(vault: Arc<dyn VaultProvider>, http: reqwest::Client) -> Self {
        Self { vault, http }
    }

    /// 注册 Webhook
    pub async fn register_webhook(&self, url: &str, secret: &str) -> BotResult<()> {
        let api_url = self.vault.build_tg_api_url("setWebhook").await?;
        let resp = self.http.post(api_url.as_str())
            .json(&serde_json::json!({
                "url": url,
                "secret_token": secret,
                "allowed_updates": ["message", "edited_message", "chat_join_request", "callback_query"],
                "max_connections": 40,
            }))
            .send().await
            .map_err(|e| BotError::PlatformApi { platform: "telegram".into(), message: format!("{}", e) })?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| BotError::PlatformApi { platform: "telegram".into(), message: format!("{}", e) })?;

        if body["ok"].as_bool() != Some(true) {
            return Err(BotError::PlatformApi {
                platform: "telegram".into(),
                message: format!("setWebhook failed: {}", body),
            });
        }

        info!(url = url, "Telegram Webhook 注册成功");
        Ok(())
    }

    /// 调用 Telegram Bot API
    ///
    /// ⚠️ 安全注意: Telegram Bot API 要求 token 嵌入 URL 路径
    /// (`https://api.telegram.org/bot<TOKEN>/method`)。
    /// `api_url` 是 `Zeroizing<String>`, drop 后自动清零。
    /// 但 reqwest 内部可能在连接池/重定向历史中缓存 URL 片段。
    /// 缓解措施:
    /// - 使用 jemalloc zero-on-free (全局分配器)
    /// - 仅通过 HTTPS 传输 (TLS 加密)
    /// - 不在日志中输出 api_url
    /// - 生产环境建议使用 Telegram Bot API 本地服务器 (无需远程传输 token)
    async fn call_api(&self, method: &str, params: serde_json::Value) -> BotResult<serde_json::Value> {
        let api_url = self.vault.build_tg_api_url(method).await?;
        let resp = self.http.post(api_url.as_str())
            .json(&params)
            .send().await
            .map_err(|e| BotError::PlatformApi { platform: "telegram".into(), message: format!("{}", e) })?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| BotError::PlatformApi { platform: "telegram".into(), message: format!("{}", e) })?;

        if body["ok"].as_bool() != Some(true) {
            let desc = body["description"].as_str().unwrap_or("unknown error");
            warn!(method = method, error = desc, "Telegram API 调用失败");
            return Err(BotError::PlatformApi {
                platform: "telegram".into(),
                message: format!("{}: {}", method, desc),
            });
        }

        Ok(body)
    }

    /// 查询用户是否为群管理员或创建者
    pub async fn is_admin_in_chat(&self, chat_id: &str, user_id: &str) -> BotResult<bool> {
        let result = self.call_api("getChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        })).await;

        match result {
            Ok(body) => {
                let status = body["result"]["status"].as_str().unwrap_or("");
                Ok(status == "administrator" || status == "creator")
            }
            Err(_) => Ok(false), // API 失败时默认非管理员 (安全侧)
        }
    }

    /// 发送消息
    pub async fn send_message(&self, chat_id: &str, text: &str) -> BotResult<()> {
        self.call_api("sendMessage", serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
        })).await?;
        Ok(())
    }

    /// 封禁用户
    pub async fn ban_user(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("banChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        })).await?;
        Ok(())
    }

    /// 禁言用户
    pub async fn mute_user(&self, chat_id: &str, user_id: &str, duration_secs: u64) -> BotResult<()> {
        let until = chrono::Utc::now().timestamp() as u64 + duration_secs;
        self.call_api("restrictChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
            "permissions": {
                "can_send_messages": false,
                "can_send_audios": false,
                "can_send_documents": false,
                "can_send_photos": false,
                "can_send_videos": false,
                "can_send_video_notes": false,
                "can_send_voice_notes": false,
                "can_send_polls": false,
                "can_send_other_messages": false,
                "can_add_web_page_previews": false,
            },
            "until_date": until,
        })).await?;
        Ok(())
    }

    /// 解除禁言
    pub async fn unmute_user(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("restrictChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
            "permissions": {
                "can_send_messages": true,
                "can_send_audios": true,
                "can_send_documents": true,
                "can_send_photos": true,
                "can_send_videos": true,
                "can_send_video_notes": true,
                "can_send_voice_notes": true,
                "can_send_polls": true,
                "can_send_other_messages": true,
                "can_add_web_page_previews": true,
            },
        })).await?;
        Ok(())
    }

    /// 踢出用户
    pub async fn kick_user(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.ban_user(chat_id, user_id).await?;
        // 立即解封以允许重新加入
        let _ = self.call_api("unbanChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
            "only_if_banned": true,
        })).await;
        Ok(())
    }

    /// 删除消息
    pub async fn delete_message(&self, chat_id: &str, message_id: &str) -> BotResult<()> {
        self.call_api("deleteMessage", serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
        })).await?;
        Ok(())
    }

    /// 审批入群请求
    pub async fn approve_join(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("approveChatJoinRequest", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        })).await?;
        Ok(())
    }

    /// 拒绝入群请求
    pub async fn decline_join(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("declineChatJoinRequest", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        })).await?;
        Ok(())
    }

    // ── Phase 4 新增 API 方法 ──

    /// 解封用户
    pub async fn unban_user(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("unbanChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
            "only_if_banned": true,
        })).await?;
        Ok(())
    }

    /// 提升为管理员
    pub async fn promote_member(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("promoteChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
            "can_manage_chat": true,
            "can_delete_messages": true,
            "can_restrict_members": true,
            "can_pin_messages": true,
            "can_invite_users": true,
        })).await?;
        Ok(())
    }

    /// 降级管理员
    pub async fn demote_member(&self, chat_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api("promoteChatMember", serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
            "can_manage_chat": false,
            "can_delete_messages": false,
            "can_restrict_members": false,
            "can_pin_messages": false,
            "can_invite_users": false,
            "can_promote_members": false,
            "can_change_info": false,
            "can_manage_video_chats": false,
        })).await?;
        Ok(())
    }

    /// 发送消息 (带 Inline 键盘)
    pub async fn send_message_with_keyboard(&self, chat_id: &str, text: &str, keyboard: &serde_json::Value) -> BotResult<()> {
        self.call_api("sendMessage", serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
            "reply_markup": {
                "inline_keyboard": keyboard,
            },
        })).await?;
        Ok(())
    }

    /// 回答回调查询
    pub async fn answer_callback_query(&self, callback_query_id: &str, text: &str) -> BotResult<()> {
        self.call_api("answerCallbackQuery", serde_json::json!({
            "callback_query_id": callback_query_id,
            "text": text,
        })).await?;
        Ok(())
    }

    /// 编辑消息文本
    pub async fn edit_message_text(&self, chat_id: &str, message_id: &str, text: &str) -> BotResult<()> {
        self.call_api("editMessageText", serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
            "parse_mode": "HTML",
        })).await?;
        Ok(())
    }

    /// 获取群管理员列表
    pub async fn get_chat_administrators(&self, chat_id: &str) -> BotResult<Vec<serde_json::Value>> {
        let body = self.call_api("getChatAdministrators", serde_json::json!({
            "chat_id": chat_id,
        })).await?;
        Ok(body["result"].as_array().cloned().unwrap_or_default())
    }

    /// 设置群权限
    pub async fn set_chat_permissions(&self, chat_id: &str, permissions: serde_json::Value) -> BotResult<()> {
        self.call_api("setChatPermissions", serde_json::json!({
            "chat_id": chat_id,
            "permissions": permissions,
        })).await?;
        Ok(())
    }

    /// 批量删除消息
    pub async fn delete_messages(&self, chat_id: &str, message_ids: &[i64]) -> BotResult<()> {
        self.call_api("deleteMessages", serde_json::json!({
            "chat_id": chat_id,
            "message_ids": message_ids,
        })).await?;
        Ok(())
    }
}

#[async_trait]
impl PlatformExecutor for TelegramExecutor {
    async fn execute(&self, action: &ExecuteAction) -> BotResult<ExecutionReceipt> {
        let now = chrono::Utc::now().timestamp() as u64;

        let result = match action.action_type {
            ActionType::Kick => self.kick_user(&action.group_id, &action.target_user).await,
            ActionType::Ban => self.ban_user(&action.group_id, &action.target_user).await,
            ActionType::Mute => {
                let dur = action.duration_secs.unwrap_or(3600);
                self.mute_user(&action.group_id, &action.target_user, dur).await
            }
            ActionType::Unmute => self.unmute_user(&action.group_id, &action.target_user).await,
            ActionType::SendMessage => {
                if let Some(ref msg) = action.message {
                    if let Some(ref kb) = action.inline_keyboard {
                        self.send_message_with_keyboard(&action.group_id, msg, kb).await
                    } else {
                        self.send_message(&action.group_id, msg).await
                    }
                } else {
                    Ok(())
                }
            }
            ActionType::DeleteMessage => {
                self.delete_message(&action.group_id, &action.target_user).await
            }
            ActionType::ApproveJoin => {
                self.approve_join(&action.group_id, &action.target_user).await
            }
            ActionType::DeclineJoin => {
                self.decline_join(&action.group_id, &action.target_user).await
            }
            ActionType::Unban => {
                self.unban_user(&action.group_id, &action.target_user).await
            }
            ActionType::Promote => {
                self.promote_member(&action.group_id, &action.target_user).await
            }
            ActionType::Demote => {
                self.demote_member(&action.group_id, &action.target_user).await
            }
            ActionType::AnswerCallback => {
                if let Some(ref cb_id) = action.callback_query_id {
                    let text = action.message.as_deref().unwrap_or("");
                    self.answer_callback_query(cb_id, text).await
                } else {
                    // target_user 存放 callback_query_id (from ActionDecision::answer_callback)
                    let text = action.message.as_deref().unwrap_or("");
                    self.answer_callback_query(&action.target_user, text).await
                }
            }
            ActionType::EditMessage => {
                if let Some(ref msg) = action.message {
                    self.edit_message_text(&action.group_id, &action.target_user, msg).await
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };

        let success = result.is_ok();
        if let Err(ref e) = result {
            warn!(action = ?action.action_type, error = %e, "Telegram 执行失败");
        }

        // 计算 message_hash
        let mut hasher = Sha256::new();
        hasher.update(action.group_id.as_bytes());
        hasher.update(action.target_user.as_bytes());
        hasher.update([action.action_type.as_u8()]);
        hasher.update(now.to_le_bytes());
        let message_hash: [u8; 32] = hasher.finalize().into();

        Ok(ExecutionReceipt {
            success,
            action_type: action.action_type,
            message_hash,
            timestamp: now,
        })
    }
}
