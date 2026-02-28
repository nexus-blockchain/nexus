use std::sync::Arc;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use serde_json::{json, Value};
use tracing::{warn, debug};

use crate::platform::telegram::adapter::TelegramAdapter;
use crate::platform::PlatformAdapter;

/// 共享应用状态 (Axum State)
pub type AppStateRef = Arc<crate::AppState>;

/// POST /webhook — Telegram Webhook 入口
pub async fn handle_webhook(
    State(state): State<AppStateRef>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> StatusCode {
    // 验证 Webhook Secret
    if !state.config.webhook_secret.is_empty() {
        let secret = headers
            .get("x-telegram-bot-api-secret-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if secret != state.config.webhook_secret {
            warn!("Webhook secret 验证失败");
            return StatusCode::UNAUTHORIZED;
        }
    }

    // 全局限流 (防止总体过载)
    if !state.rate_limiter.allow() {
        warn!("Webhook 请求被全局限流");
        return StatusCode::TOO_MANY_REQUESTS;
    }

    // 解析平台事件
    let adapter = TelegramAdapter::new();
    let event = match adapter.parse_event(&body) {
        Some(e) => e,
        None => {
            debug!("无法解析的 Webhook 事件");
            return StatusCode::OK;
        }
    };

    // per-group 限流 (防止单个群耗尽全局配额)
    if !state.rate_limiter.allow_for(&event.group_id) {
        warn!(group = %event.group_id, "Webhook 请求被 per-group 限流");
        return StatusCode::TOO_MANY_REQUESTS;
    }

    // 记录指标
    state.metrics.record_message();

    // 提取上下文
    let mut ctx = adapter.extract_context(&event);

    // 查询管理员身份 (仅对命令消息查询, 减少 API 调用)
    if ctx.is_command {
        if let Some(ref tg_executor) = state.telegram_executor {
            match tg_executor.is_admin_in_chat(&ctx.group_id, &ctx.sender_id).await {
                Ok(is_admin) => ctx.is_admin = is_admin,
                Err(e) => debug!(error = %e, "管理员身份查询失败, 默认非管理员"),
            }
        }
    }

    // 指纹去重 (M7 修复: 仅对有 message_id 的事件去重, 避免 join request 等无 ID 事件被误去重)
    if let Some(ref mid) = event.message_id {
        if !mid.is_empty() {
            let fingerprint = format!("{}:{}:{}", ctx.platform, ctx.group_id, mid);
            if state.local_store.check_fingerprint(&fingerprint, 300) {
                debug!("重复消息，跳过");
                return StatusCode::OK;
            }
        }
    }

    // 路由处理
    if let Some(ref tg_executor) = state.telegram_executor {
        if let Err(e) = state.router.handle_event(&ctx, tg_executor.as_ref()).await {
            warn!(error = %e, "消息处理失败");
        }
    }

    StatusCode::OK
}

/// GET /health — 健康检查
pub async fn handle_health(
    State(state): State<AppStateRef>,
) -> Json<Value> {
    let uptime = state.start_time.elapsed().as_secs();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime,
        "platform": format!("{:?}", state.config.platform),
        "tee_mode": state.config.tee_mode,
        "seal_policy": state.config.seal_policy,
    }))
}

/// GET /v1/status — Bot 状态
pub async fn handle_status(
    State(state): State<AppStateRef>,
) -> Json<Value> {
    let uptime = state.start_time.elapsed().as_secs();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime,
        "bot_id_hash": state.config.bot_id_hash_hex(),
        "public_key": state.enclave.public_key_hex(),
        "tee_mode": format!("{}", state.enclave.mode()),
        "seal_policy": state.config.seal_policy,
        "upgrade_compat": {
            "mrsigner_key_available": state.enclave.has_mrsigner_key(),
            "cross_version_seal": state.config.seal_policy != "mrenclave",
        },
        "cached_groups": state.config_manager.cached_count(),
        "local_store_counters": state.local_store.counter_count(),
        "local_store_fingerprints": state.local_store.fingerprint_count(),
    }))
}
