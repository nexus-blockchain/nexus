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
        self.call_api_with_headers(method, path, body, None).await
    }

    /// M1+M2 修复: 支持自定义 headers + 429 限流自动重试
    async fn call_api_with_headers(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        extra_headers: Option<Vec<(&str, &str)>>,
    ) -> BotResult<serde_json::Value> {
        let mut retries = 0u8;
        loop {
            let url = self.api_url(path);
            let auth = self.vault.build_dc_auth_header().await?;
            let mut req = self.http.request(method.clone(), &url)
                .header("Authorization", auth.as_str());

            if let Some(ref headers) = extra_headers {
                for (k, v) in headers {
                    req = req.header(*k, *v);
                }
            }

            if let Some(ref b) = body {
                req = req.json(b);
            }

            let resp = req.send().await
                .map_err(|e| BotError::PlatformApi { platform: "discord".into(), message: format!("{}", e) })?;

            let status = resp.status();
            if status.as_u16() == 204 {
                return Ok(serde_json::json!({"ok": true}));
            }

            let resp_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));

            // M1 修复: 429 Too Many Requests — 解析 retry_after 并等待后重试
            if status.as_u16() == 429 && retries < 1 {
                // Discord retry_after 是浮点秒数
                let retry_after = resp_body.get("retry_after")
                    .and_then(|r| r.as_f64())
                    .unwrap_or(1.0);
                let wait_ms = (retry_after * 1000.0).min(30_000.0) as u64;
                warn!(path = path, retry_after_ms = wait_ms, "Discord API 429 限流, 等待后重试");
                tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                retries += 1;
                continue;
            }

            if !status.is_success() {
                let msg = resp_body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                warn!(path = path, status = %status, error = msg, "Discord API 失败");
                return Err(BotError::PlatformApi {
                    platform: "discord".into(),
                    message: format!("{}: {} {}", path, status, msg),
                });
            }
            return Ok(resp_body);
        }
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
        // M2 修复: 使用 call_api_with_headers 统一错误处理 + 429 重试
        let headers = reason.map(|r| vec![("X-Audit-Log-Reason", r)]);
        self.call_api_with_headers(
            reqwest::Method::PUT,
            &format!("/guilds/{}/bans/{}", guild_id, user_id),
            Some(serde_json::json!({"delete_message_seconds": 0})),
            headers,
        ).await?;
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
        // M3 修复: Discord timeout 上限 28 天 (2,419,200 秒) + 安全 i64 转换
        const MAX_TIMEOUT_SECS: u64 = 28 * 24 * 3600;
        let capped = duration_secs.min(MAX_TIMEOUT_SECS);
        let until = chrono::Utc::now() + chrono::Duration::seconds(capped as i64);
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
                    // M1 修复: 优先使用 channel_id (Discord 需要 channel_id 而非 guild_id)
                    let channel = action.channel_id.as_deref().unwrap_or(&action.group_id);
                    self.send_message(channel, msg).await
                } else {
                    Ok(())
                }
            }
            ActionType::Unban => self.unban_member(&action.group_id, &action.target_user).await,
            ActionType::DeleteMessage => {
                // M1 修复: 使用 channel_id + target_user(message_id) 正确删除消息
                if let Some(ref channel_id) = action.channel_id {
                    self.delete_message(channel_id, &action.target_user).await
                } else {
                    warn!(action = ?action.action_type, "Discord DeleteMessage 缺少 channel_id");
                    Err(BotError::PlatformApi {
                        platform: "discord".into(),
                        message: "DeleteMessage requires channel_id".into(),
                    })
                }
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
        // L2 修复: 移除双重日志 — call_api 内部已记录失败详情

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
