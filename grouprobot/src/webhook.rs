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

    // 限流
    if !state.rate_limiter.allow() {
        warn!("Webhook 请求被限流");
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

    // 记录指标
    state.metrics.record_message();

    // 提取上下文
    let ctx = adapter.extract_context(&event);

    // 指纹去重
    let fingerprint = format!("{}:{}:{}", ctx.platform, ctx.group_id, event.message_id.as_deref().unwrap_or(""));
    if state.local_store.check_fingerprint(&fingerprint, 300) {
        debug!("重复消息，跳过");
        return StatusCode::OK;
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
        "cached_groups": state.config_manager.cached_count(),
        "local_store_counters": state.local_store.counter_count(),
        "local_store_fingerprints": state.local_store.fingerprint_count(),
    }))
}
