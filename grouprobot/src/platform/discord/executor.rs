use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Sha256, Digest};
use tracing::warn;

use crate::error::{BotError, BotResult};
use crate::platform::{PlatformExecutor, ExecuteAction, ExecutionReceipt, ActionType};
use crate::tee::vault_client::VaultProvider;

/// Discord REST API 执行器
#[allow(dead_code)]
pub struct DiscordExecutor {
    vault: Arc<dyn VaultProvider>,
    application_id: String,
    http: reqwest::Client,
}

impl DiscordExecutor {
    pub fn new(vault: Arc<dyn VaultProvider>, application_id: String, http: reqwest::Client) -> Self {
        Self { vault, application_id, http }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://discord.com/api/v10{}", path)
    }

    async fn call_api(&self, method: reqwest::Method, path: &str, body: Option<serde_json::Value>) -> BotResult<serde_json::Value> {
        let url = self.api_url(path);
        let auth = self.vault.build_dc_auth_header().await?;
        let mut req = self.http.request(method, &url)
            .header("Authorization", auth.as_str());

        if let Some(b) = body {
            req = req.json(&b);
        }

        let resp = req.send().await
            .map_err(|e| BotError::PlatformApi { platform: "discord".into(), message: format!("{}", e) })?;

        let status = resp.status();
        if status.as_u16() == 204 {
            return Ok(serde_json::json!({"ok": true}));
        }

        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
        if !status.is_success() {
            let msg = body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
            warn!(path = path, status = %status, error = msg, "Discord API 失败");
            return Err(BotError::PlatformApi {
                platform: "discord".into(),
                message: format!("{}: {} {}", path, status, msg),
            });
        }
        Ok(body)
    }

    pub async fn send_message(&self, channel_id: &str, content: &str) -> BotResult<()> {
        self.call_api(
            reqwest::Method::POST,
            &format!("/channels/{}/messages", channel_id),
            Some(serde_json::json!({"content": content})),
        ).await?;
        Ok(())
    }

    pub async fn ban_member(&self, guild_id: &str, user_id: &str, reason: Option<&str>) -> BotResult<()> {
        let url = self.api_url(&format!("/guilds/{}/bans/{}", guild_id, user_id));
        let auth = self.vault.build_dc_auth_header().await?;
        let body = serde_json::json!({"delete_message_seconds": 0});

        let mut req = self.http.request(reqwest::Method::PUT, &url)
            .header("Authorization", auth.as_str())
            .json(&body);

        if let Some(r) = reason {
            // Discord 支持通过 X-Audit-Log-Reason 记录 ban 原因到审计日志
            req = req.header("X-Audit-Log-Reason", r);
        }

        let resp = req.send().await
            .map_err(|e| BotError::PlatformApi { platform: "discord".into(), message: format!("{}", e) })?;

        let status = resp.status();
        if status.as_u16() != 204 && !status.is_success() {
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
            let msg = body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
            return Err(BotError::PlatformApi {
                platform: "discord".into(),
                message: format!("ban: {} {}", status, msg),
            });
        }
        Ok(())
    }

    pub async fn kick_member(&self, guild_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api(
            reqwest::Method::DELETE,
            &format!("/guilds/{}/members/{}", guild_id, user_id),
            None,
        ).await?;
        Ok(())
    }

    pub async fn timeout_member(&self, guild_id: &str, user_id: &str, duration_secs: u64) -> BotResult<()> {
        let until = chrono::Utc::now() + chrono::Duration::seconds(duration_secs as i64);
        self.call_api(
            reqwest::Method::PATCH,
            &format!("/guilds/{}/members/{}", guild_id, user_id),
            Some(serde_json::json!({
                "communication_disabled_until": until.to_rfc3339()
            })),
        ).await?;
        Ok(())
    }

    pub async fn remove_timeout(&self, guild_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api(
            reqwest::Method::PATCH,
            &format!("/guilds/{}/members/{}", guild_id, user_id),
            Some(serde_json::json!({"communication_disabled_until": null})),
        ).await?;
        Ok(())
    }

    pub async fn delete_message(&self, channel_id: &str, message_id: &str) -> BotResult<()> {
        self.call_api(
            reqwest::Method::DELETE,
            &format!("/channels/{}/messages/{}", channel_id, message_id),
            None,
        ).await?;
        Ok(())
    }

    pub async fn unban_member(&self, guild_id: &str, user_id: &str) -> BotResult<()> {
        self.call_api(
            reqwest::Method::DELETE,
            &format!("/guilds/{}/bans/{}", guild_id, user_id),
            None,
        ).await?;
        Ok(())
    }

    /// M1 修复: 正确查询用户是否为 guild 管理员
    ///
    /// Discord GET /guilds/{id}/members/{id} 不返回 permissions 字段,
    /// 需要: 1) 检查是否为 guild owner  2) 从 guild roles 聚合成员权限
    pub async fn is_admin_in_guild(&self, guild_id: &str, user_id: &str) -> BotResult<bool> {
        // 1. 获取 guild 信息 (检查 owner + 获取 roles 定义)
        let guild = self.call_api(
            reqwest::Method::GET,
            &format!("/guilds/{}", guild_id),
            None,
        ).await.map_err(|e| {
            warn!(error = %e, guild = guild_id, "获取 guild 信息失败");
            e
        })?;

        // Guild owner 拥有所有权限
        if let Some(owner_id) = guild.get("owner_id").and_then(|o| o.as_str()) {
            if owner_id == user_id {
                return Ok(true);
            }
        }

        // 2. 获取成员的 role ID 列表
        let member = self.call_api(
            reqwest::Method::GET,
            &format!("/guilds/{}/members/{}", guild_id, user_id),
            None,
        ).await.map_err(|e| {
            warn!(error = %e, guild = guild_id, user = user_id, "获取成员信息失败");
            e
        })?;

        let member_role_ids: Vec<&str> = member
            .get("roles")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        if member_role_ids.is_empty() {
            return Ok(false);
        }

        // 3. 从 guild roles 中查找成员拥有的 role, 聚合权限位
        let guild_roles = guild.get("roles").and_then(|r| r.as_array());
        if let Some(roles) = guild_roles {
            let mut aggregated_perms: u64 = 0;
            for role in roles {
                let role_id = role.get("id").and_then(|id| id.as_str()).unwrap_or("");
                // @everyone role (id == guild_id) 的权限对所有人生效
                let is_everyone = role_id == guild_id;
                let is_member_role = member_role_ids.contains(&role_id);

                if is_everyone || is_member_role {
                    // permissions 字段在 role 对象中是字符串类型的整数
                    if let Some(perms_str) = role.get("permissions").and_then(|p| p.as_str()) {
                        if let Ok(perms) = perms_str.parse::<u64>() {
                            aggregated_perms |= perms;
                        }
                    }
                }
            }
            // ADMINISTRATOR = 0x8
            return Ok(aggregated_perms & 0x8 != 0);
        }

        Ok(false)
    }
}

#[async_trait]
impl PlatformExecutor for DiscordExecutor {
    async fn execute(&self, action: &ExecuteAction) -> BotResult<ExecutionReceipt> {
        let now = chrono::Utc::now().timestamp() as u64;

        let result = match action.action_type {
            ActionType::Kick => self.kick_member(&action.group_id, &action.target_user).await,
            ActionType::Ban => self.ban_member(&action.group_id, &action.target_user, action.reason.as_deref()).await,
            ActionType::Mute => {
                let dur = action.duration_secs.unwrap_or(3600);
                self.timeout_member(&action.group_id, &action.target_user, dur).await
            }
            ActionType::Unmute => self.remove_timeout(&action.group_id, &action.target_user).await,
            ActionType::SendMessage => {
                if let Some(ref msg) = action.message {
                    self.send_message(&action.group_id, msg).await
                } else {
                    Ok(())
                }
            }
            ActionType::Unban => self.unban_member(&action.group_id, &action.target_user).await,
            ActionType::DeleteMessage => {
                // DeleteMessage 需要 channel_id + message_id, 当前 ExecuteAction 结构不支持
                // group_id 在 Discord 是 guild_id 而非 channel_id, 故无法正确执行
                warn!(action = ?action.action_type, "Discord DeleteMessage 需要 channel_id + message_id, 当前 ExecuteAction 结构不支持");
                Err(BotError::PlatformApi {
                    platform: "discord".into(),
                    message: "DeleteMessage not supported: ExecuteAction lacks channel_id and message_id fields".into(),
                })
            }
            other => {
                // H1 修复: 未实现的动作显式返回错误, 而非静默成功
                warn!(action = ?other, "Discord 不支持此动作类型");
                Err(BotError::PlatformApi {
                    platform: "discord".into(),
                    message: format!("unsupported action type: {:?}", other),
                })
            }
        };

        let success = result.is_ok();
        if !success {
            if let Err(ref e) = result {
                warn!(action = ?action.action_type, error = %e, "Discord 动作执行失败");
            }
        }

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
