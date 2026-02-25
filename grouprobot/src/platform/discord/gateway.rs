use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn, debug};

use crate::platform::PlatformEvent;
use crate::platform::discord::adapter::DiscordAdapter;
use crate::platform::PlatformAdapter;
use crate::tee::vault_client::VaultProvider;

/// Discord Gateway 会话状态 (用于 RESUME)
struct SessionState {
    session_id: Option<String>,
    sequence: Option<u64>,
    resume_gateway_url: Option<String>,
}

/// Discord Gateway WebSocket 客户端
pub struct DiscordGateway {
    vault: Arc<dyn VaultProvider>,
    intents: u64,
    event_tx: mpsc::Sender<PlatformEvent>,
    session: Mutex<SessionState>,
}

impl DiscordGateway {
    pub fn new(vault: Arc<dyn VaultProvider>, intents: u64, event_tx: mpsc::Sender<PlatformEvent>) -> Self {
        Self {
            vault,
            intents,
            event_tx,
            session: Mutex::new(SessionState {
                session_id: None,
                sequence: None,
                resume_gateway_url: None,
            }),
        }
    }

    /// 运行 Gateway 连接 (自动重连 + RESUME + 指数退避)
    pub async fn run(&self) {
        let default_url = "wss://gateway.discord.gg/?v=10&encoding=json";
        let adapter = DiscordAdapter::new();
        let mut backoff_secs: u64 = 5;
        const MAX_BACKOFF: u64 = 60;

        loop {
            // 优先使用 resume_gateway_url (Discord 在 READY 中提供)
            let url = {
                let session = self.session.lock().await;
                session.resume_gateway_url.clone()
                    .unwrap_or_else(|| default_url.to_string())
            };

            info!(url = %url, "连接 Discord Gateway...");
            match self.connect_and_listen(&url, &adapter).await {
                Ok(()) => {
                    info!("Discord Gateway 正常断开");
                    backoff_secs = 5; // 正常断开重置退避
                }
                Err(e) => {
                    warn!(error = %e, backoff = backoff_secs, "Discord Gateway 连接断开");
                }
            }
            info!(delay = backoff_secs, "Discord Gateway 重连中...");
            tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF);
        }
    }

    async fn connect_and_listen(
        &self,
        url: &str,
        adapter: &DiscordAdapter,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        // 等待 HELLO
        let hello = read.next().await
            .ok_or("no hello")??;
        let hello: serde_json::Value = serde_json::from_str(hello.to_text()?)?;
        let heartbeat_interval = hello["d"]["heartbeat_interval"].as_u64().unwrap_or(41250);

        // 决定 IDENTIFY 还是 RESUME
        let should_resume = {
            let session = self.session.lock().await;
            session.session_id.is_some() && session.sequence.is_some()
        };

        if should_resume {
            let session = self.session.lock().await;
            let token = self.vault.build_dc_auth_header().await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            // RESUME payload: 从 "Bot <token>" 中提取纯 token
            // ⚠️ 安全: 使用 Zeroizing 包裹 resume JSON, 发送后自动清零
            let raw_token = token.strip_prefix("Bot ").unwrap_or(&token);
            let resume_json = zeroize::Zeroizing::new(format!(
                r#"{{"op":6,"d":{{"token":"{}","session_id":"{}","seq":{}}}}}"#,
                raw_token,
                session.session_id.as_deref().unwrap_or(""),
                session.sequence.map(|s| s.to_string()).unwrap_or_else(|| "null".to_string()),
            ));
            info!(
                session_id = session.session_id.as_deref().unwrap_or(""),
                seq = ?session.sequence,
                "发送 RESUME"
            );
            write.send(tokio_tungstenite::tungstenite::Message::Text(resume_json.to_string())).await?;
            // resume_json (Zeroizing<String>) 在此 drop, 内存清零
            // token (Zeroizing<String> from build_dc_auth_header) 同样自动清零
        } else {
            // 首次连接: 发送 IDENTIFY (通过 VaultProvider 构建)
            let identify_payload = self.vault.build_dc_identify_payload(self.intents).await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            write.send(tokio_tungstenite::tungstenite::Message::Text(identify_payload.to_string())).await?;
            info!("发送 IDENTIFY");
        }

        // 心跳 + 事件循环
        let mut heartbeat = tokio::time::interval(
            std::time::Duration::from_millis(heartbeat_interval)
        );

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let seq = self.session.lock().await.sequence;
                    let hb = json!({"op": 1, "d": seq});
                    write.send(tokio_tungstenite::tungstenite::Message::Text(hb.to_string())).await?;
                    debug!("Discord heartbeat sent");
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(m)) => {
                            if let Ok(text) = m.to_text() {
                                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(text) {
                                    // 更新序列号
                                    if let Some(s) = payload.get("s").and_then(|s| s.as_u64()) {
                                        self.session.lock().await.sequence = Some(s);
                                    }

                                    let op = payload.get("op").and_then(|o| o.as_u64()).unwrap_or(0);
                                    match op {
                                        0 => {
                                            // DISPATCH: 处理特殊事件 + 转发
                                            let event_type = payload.get("t")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("");

                                            // READY: 保存 session_id + resume_gateway_url
                                            if event_type == "READY" {
                                                if let Some(d) = payload.get("d") {
                                                    let mut session = self.session.lock().await;
                                                    if let Some(sid) = d.get("session_id").and_then(|s| s.as_str()) {
                                                        session.session_id = Some(sid.to_string());
                                                        info!(session_id = sid, "Discord session 已建立");
                                                    }
                                                    if let Some(url) = d.get("resume_gateway_url").and_then(|s| s.as_str()) {
                                                        session.resume_gateway_url = Some(
                                                            format!("{}/?v=10&encoding=json", url.trim_end_matches('/'))
                                                        );
                                                    }
                                                }
                                            }

                                            // RESUMED: 确认恢复成功
                                            if event_type == "RESUMED" {
                                                info!("Discord session RESUME 成功");
                                            }

                                            // 转发平台事件
                                            if let Some(event) = adapter.parse_event(&payload) {
                                                if self.event_tx.send(event).await.is_err() {
                                                    warn!("事件通道已关闭");
                                                    return Ok(());
                                                }
                                            }
                                        }
                                        11 => {
                                            debug!("Discord heartbeat ACK");
                                        }
                                        7 => {
                                            // RECONNECT: 保留 session, 用于 RESUME
                                            info!("Discord 要求重连 (将尝试 RESUME)");
                                            return Ok(());
                                        }
                                        9 => {
                                            // INVALID SESSION: d=true 可 RESUME, d=false 需新 IDENTIFY
                                            let resumable = payload.get("d")
                                                .and_then(|d| d.as_bool())
                                                .unwrap_or(false);
                                            if !resumable {
                                                let mut session = self.session.lock().await;
                                                session.session_id = None;
                                                session.sequence = None;
                                                warn!("Discord session 已失效 (不可恢复, 将重新 IDENTIFY)");
                                            } else {
                                                warn!("Discord session 已失效 (可恢复, 将尝试 RESUME)");
                                            }
                                            return Ok(());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            warn!(error = %e, "WebSocket 错误");
                            return Err(e.into());
                        }
                        None => {
                            info!("WebSocket 流结束");
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}
