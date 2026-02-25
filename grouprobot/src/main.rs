//! grouprobot — GroupRobot 离链 TEE Bot
//!
//! 统一入口: TDX+SGX 加密执行 + Webhook 接收 + 消息处理 + 链上交互
//! 与链上 grouprobot-* Pallet (index 150-153) 通过 subxt 交互

// ── jemalloc 全局分配器 (堆内存释放后自动清零) ──
// 防止 Token 等敏感数据在 free() 后残留在堆内存中
// reqwest/hyper/TLS 库内部分配的 String 也会受此保护
#[cfg(not(test))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use std::sync::Arc;
use std::time::Instant;

use axum::{Router, routing::{get, post}};
use tracing::{info, warn, error, debug};

mod config;
mod error;
mod chain;
mod tee;
mod platform;
mod processing;
mod infra;
mod webhook;

use crate::chain::ChainClient;
use crate::chain::transactions::ActionLogBatcher;
use crate::config::BotConfig;
use crate::infra::local_store::LocalStore;
use crate::infra::rate_limiter::RateLimiter;
use crate::infra::metrics::{self, SharedMetrics};
use crate::infra::group_config::ConfigManager;
use crate::platform::telegram::executor::TelegramExecutor;
use crate::platform::discord::executor::DiscordExecutor;
use crate::processing::router::MessageRouter;
use crate::processing::rule_engine::RuleEngine;
use crate::tee::enclave_bridge::EnclaveBridge;
use crate::tee::key_manager::{KeyManager, SequenceManager};
use crate::tee::attestor::Attestor;
use crate::tee::vault_client::VaultProvider;

/// 全局应用状态
pub struct AppState {
    pub config: BotConfig,
    pub enclave: Arc<EnclaveBridge>,
    pub attestor: Arc<Attestor>,
    pub metrics: SharedMetrics,
    pub local_store: Arc<LocalStore>,
    pub rate_limiter: RateLimiter,
    pub config_manager: Arc<ConfigManager>,
    pub start_time: Instant,
    pub router: Arc<MessageRouter>,
    pub telegram_executor: Option<Arc<TelegramExecutor>>,
    pub discord_executor: Option<Arc<DiscordExecutor>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── 内存安全加固 (必须最先执行, 在任何 Token 操作之前) ──
    let harden_report = tee::mem_security::harden_process_memory();

    // ── 初始化 ──
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "grouprobot=info".into()),
        )
        .init();

    info!("╔══════════════════════════════════════╗");
    info!("║   GroupRobot Bot v{}          ║", env!("CARGO_PKG_VERSION"));
    info!("║   TEE Off-chain Executor             ║");
    info!("╚══════════════════════════════════════╝");

    // ── jemalloc zero-on-free 检查 ──
    // jemalloc 在进程启动时 (#[global_allocator]) 即初始化, 此时 main() 尚未执行,
    // 因此 MALLOC_CONF 必须在进程启动 **前** 通过以下方式设置:
    //   - Dockerfile: ENV MALLOC_CONF="abort_conf:false,zero:true"
    //   - systemd:   Environment=MALLOC_CONF=abort_conf:false,zero:true
    //   - shell:     MALLOC_CONF="abort_conf:false,zero:true" ./grouprobot
    //
    // ⚠️ 运行时 set_var("MALLOC_CONF", ...) 对已初始化的 jemalloc 无效, 不要在此处设置。
    #[cfg(not(test))]
    {
        if std::env::var("MALLOC_CONF").is_err() {
            warn!("MALLOC_CONF 未设置 — jemalloc zero-on-free 未启用, 建议在部署配置中设置 MALLOC_CONF=abort_conf:false,zero:true");
        }
    }

    // 记录加固状态
    info!(
        core_dump_disabled = harden_report.core_dump_disabled,
        dumpable_cleared = harden_report.dumpable_cleared,
        mlock_raised = harden_report.mlock_limit_raised,
        "🔒 内存安全加固完成"
    );

    // ── 加载配置 ──
    let cfg = BotConfig::from_env()?;
    info!(
        platform = ?cfg.platform,
        tee_mode = %cfg.tee_mode,
        port = cfg.webhook_port,
        "配置加载完成"
    );

    // M2 修复: 安全配置缺失警告
    if cfg.webhook_secret.is_empty() && cfg.platform.needs_telegram() {
        warn!("⚠️ WEBHOOK_SECRET 未设置 — Telegram Webhook 鉴权已禁用, 任何人可伪造 Webhook 请求");
    }
    if cfg.provision_secret.is_empty() {
        warn!("⚠️ PROVISION_SECRET 未设置 — /provision/* 路由已禁用 (无法通过 RA-TLS 注入 Token)");
    }

    // ── Enclave 初始化 ──
    std::fs::create_dir_all(&cfg.data_dir).ok();
    let enclave = Arc::new(EnclaveBridge::init(&cfg.data_dir, &cfg.tee_mode)?);
    info!(
        mode = %enclave.mode(),
        public_key = %enclave.public_key_hex(),
        "Enclave 已初始化"
    );

    // ── 密钥管理 ──
    let key_manager = Arc::new(KeyManager::new(enclave.clone(), cfg.bot_id_hash));
    info!(public_key = %key_manager.public_key_hex(), "Ed25519 密钥就绪");

    // ── 序列号管理 ──
    let sequence = Arc::new(SequenceManager::load_or_init(&cfg.data_dir)?);

    // ── Prometheus 指标 ──
    let tee_metrics = metrics::init_metrics();

    // ── 双证明生成 ──
    let attestor = Arc::new(Attestor::new(enclave.clone()));
    match attestor.generate_attestation() {
        Ok(bundle) => {
            info!(simulated = bundle.is_simulated, "双证明已生成");
            tee_metrics.record_quote_refresh(true);
        }
        Err(e) => {
            warn!(error = %e, "双证明生成失败（不影响启动）");
            tee_metrics.record_quote_refresh(false);
        }
    }

    // ── ShareRecovery: 统一 Token 恢复 ──
    // 优先: 本地 Shamir share → K>1 peer 收集 → 环境变量 fallback (auto-seal)
    // 首次 env fallback 会 auto-seal, 后续启动直接从 share 恢复

    // R3 修复: K>1 且无静态 peer 时, 提前连接链以支持链上 peer 自动发现
    let early_chain: Option<Arc<ChainClient>> =
        if cfg.shamir_threshold > 1 && cfg.peer_endpoints.is_empty() {
            info!("K>1 且无静态 PEER_ENDPOINTS, 尝试提前连接链以发现 peer...");
            let signer = chain::client::load_or_generate_signer(
                &cfg.data_dir, cfg.chain_signer_seed.as_deref(),
            );
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                ChainClient::connect(&cfg.chain_rpc, signer),
            ).await {
                Ok(Ok(client)) => {
                    info!("提前链连接成功, 链上 peer 发现可用");
                    Some(Arc::new(client))
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "提前链连接失败, 将使用静态 peer 或 env fallback");
                    None
                }
                Err(_) => {
                    warn!("提前链连接超时 (10s), 将使用静态 peer 或 env fallback");
                    None
                }
            }
        } else {
            None
        };

    let recovery_config = tee::share_recovery::RecoveryConfig {
        threshold: cfg.shamir_threshold,
        needs_telegram: cfg.platform.needs_telegram(),
        needs_discord: cfg.platform.needs_discord(),
        peer_endpoints: cfg.peer_endpoints.clone(),
        ceremony_hash: cfg.bot_id_hash,
        chain_client: early_chain.clone(),
        bot_id_hash: Some(cfg.bot_id_hash),
    };

    // provision_vault: inprocess 模式下供 RA-TLS Provision 写入 Token 用
    let mut provision_vault: Option<Arc<tokio::sync::RwLock<tee::token_vault::TokenVault>>> = None;
    // provision_client: connect 模式下供 RA-TLS SGX 代理用 (Token 明文仅在 SGX enclave 内)
    let mut provision_client: Option<Arc<tee::vault_client::VaultClient>> = None;
    let vault: Arc<dyn VaultProvider> = match cfg.vault_mode.as_str() {
        "connect" => {
            // 连接到外部 vault 进程 (Gramine SGX 模式, Token 在远端)
            let sock = if cfg.vault_socket.is_empty() {
                tee::vault_ipc::default_socket_path(&cfg.data_dir)
            } else {
                cfg.vault_socket.clone()
            };
            // 加载 IPC 加密密钥 (如果存在)
            let ipc_key = tee::vault_ipc::ensure_ipc_key(&cfg.data_dir)?;
            info!(socket = %sock, encrypted = true, "连接外部 Vault 进程...");
            let client = Arc::new(tee::vault_client::VaultClient::connect_encrypted(&sock, ipc_key).await?);
            client.ping().await?;
            info!("✅ Vault IPC 连接成功 (加密通道, SGX 代理模式)");
            provision_client = Some(client.clone());
            client as Arc<dyn VaultProvider>
        }
        "spawn" => {
            // 恢复 Token → 启动内嵌 vault 服务端 → IPC 连接
            let sock = if cfg.vault_socket.is_empty() {
                tee::vault_ipc::default_socket_path(&cfg.data_dir)
            } else {
                cfg.vault_socket.clone()
            };
            let recovery = tee::share_recovery::recover_token(&enclave, &recovery_config).await?;
            info!(source = %recovery.source, "Token 恢复完成 (spawn 模式)");
            // 生成/加载 IPC 加密密钥
            let ipc_key = tee::vault_ipc::ensure_ipc_key(&cfg.data_dir)?;
            let server = tee::vault_server::VaultServer::with_encryption(recovery.vault, sock.clone(), ipc_key);
            tokio::spawn(async move {
                if let Err(e) = server.run().await {
                    error!(error = %e, "Vault 服务端异常退出");
                }
            });
            for _ in 0..50 {
                if std::path::Path::new(&sock).exists() { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            let client = tee::vault_client::VaultClient::connect_encrypted(&sock, ipc_key).await?;
            client.ping().await?;
            info!("✅ Vault spawn 模式就绪 (加密 IPC)");
            Arc::new(client)
        }
        _ => {
            // inprocess 模式 (默认): 恢复 Token → 直接使用
            let recovery = tee::share_recovery::recover_token(&enclave, &recovery_config).await?;
            info!(source = %recovery.source, "Token 恢复完成 (inprocess 模式)");
            // 如果 recovery 包含签名密钥 (非零), 可注入 EnclaveBridge
            // 当前 EnclaveBridge 已有自己的密钥, 未来 Ceremony 恢复时会用到
            let vault_rw = Arc::new(tokio::sync::RwLock::new(recovery.vault));
            provision_vault = Some(vault_rw.clone());
            vault_rw as Arc<dyn VaultProvider>
        }
    };
    info!(mode = %cfg.vault_mode, "TokenVault 已初始化");

    // ── HTTP 客户端 ──
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // ── Telegram 执行器 ──
    let tg_executor = if cfg.platform.needs_telegram() {
        Some(Arc::new(TelegramExecutor::new(vault.clone(), http_client.clone())))
    } else {
        None
    };

    // ── Discord 执行器 ──
    let dc_executor = if cfg.platform.needs_discord() {
        cfg.discord.as_ref().map(|dc_cfg| {
            Arc::new(DiscordExecutor::new(
                vault.clone(), dc_cfg.application_id.clone(), http_client.clone(),
            ))
        })
    } else {
        None
    };

    // ── 基础设施 ──
    let local_store = Arc::new(LocalStore::new());
    let rate_limiter = RateLimiter::new(cfg.webhook_rate_limit, 60);
    let config_manager = Arc::new(ConfigManager::new(30));

    // ── 规则引擎 + 消息路由器 ──
    let rule_engine = RuleEngine::new(local_store.clone(), true, 10);
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let audit_logger = Arc::new(crate::processing::audit_logger::AuditLogger::new(1000));
    let router = Arc::new(MessageRouter::new(
        rule_engine, key_manager.clone(), sequence.clone(), log_tx, audit_logger,
    ));

    // ── 构建 AppState ──
    let state = Arc::new(AppState {
        config: cfg.clone(),
        enclave: enclave.clone(),
        attestor: attestor.clone(),
        metrics: tee_metrics.clone(),
        local_store: local_store.clone(),
        rate_limiter,
        config_manager: config_manager.clone(),
        start_time: Instant::now(),
        router: router.clone(),
        telegram_executor: tg_executor.clone(),
        discord_executor: dc_executor.clone(),
    });

    // ── 定时清理 (60s) ──
    {
        let state_gc = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                state_gc.local_store.cleanup_expired();
                state_gc.rate_limiter.cleanup(); // M8 修复: 清理 per-key 条目防止内存泄漏
            }
        });
    }

    // ── Telegram Webhook 注册 ──
    if cfg.platform.needs_telegram() && !cfg.webhook_url.is_empty() {
        if let Some(ref executor) = tg_executor {
            let executor = executor.clone();
            let url = cfg.webhook_url.clone();
            let secret = cfg.webhook_secret.clone();
            tokio::spawn(async move {
                if let Err(e) = executor.register_webhook(&url, &secret).await {
                    error!(error = %e, "Telegram Webhook 注册失败");
                }
            });
        }
    }

    // ── Discord Gateway 启动 ──
    if cfg.platform.needs_discord() {
        if let Some(ref dc_cfg) = cfg.discord {
            let vault_gw = vault.clone();
            let dc_intents = dc_cfg.intents;
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(256);

            tokio::spawn(async move {
                let gw = platform::discord::gateway::DiscordGateway::new(
                    vault_gw, dc_intents, event_tx,
                );
                gw.run().await;
            });

            let state_dc = state.clone();
            tokio::spawn(async move {
                let adapter = platform::discord::adapter::DiscordAdapter::new();
                use crate::platform::PlatformAdapter;
                while let Some(event) = event_rx.recv().await {
                    // H2 修复: 全局限流 (与 Telegram webhook 对等)
                    if !state_dc.rate_limiter.allow() {
                        warn!("Discord 事件被全局限流");
                        continue;
                    }

                    // H2 修复: per-group 限流
                    if !state_dc.rate_limiter.allow_for(&event.group_id) {
                        warn!(group = %event.group_id, "Discord 事件被 per-group 限流");
                        continue;
                    }

                    // H2 修复: 指纹去重 (仅对有 message_id 的事件)
                    if let Some(ref mid) = event.message_id {
                        if !mid.is_empty() {
                            let fingerprint = format!("discord:{}:{}", event.group_id, mid);
                            if state_dc.local_store.check_fingerprint(&fingerprint, 300) {
                                debug!("Discord 重复事件，跳过");
                                continue;
                            }
                        }
                    }

                    state_dc.metrics.record_message();
                    let mut ctx = adapter.extract_context(&event);

                    // H1 修复: Discord 事件也需要查询管理员身份
                    if ctx.is_command {
                        if let Some(ref dc_exec) = state_dc.discord_executor {
                            match dc_exec.is_admin_in_guild(&ctx.group_id, &ctx.sender_id).await {
                                Ok(is_admin) => ctx.is_admin = is_admin,
                                Err(e) => debug!(error = %e, "Discord 管理员身份查询失败"),
                            }
                        }
                    }

                    if let Some(ref dc_exec) = state_dc.discord_executor {
                        if let Err(e) = state_dc.router.handle_event(&ctx, dc_exec.as_ref()).await {
                            warn!(error = %e, "Discord 事件处理失败");
                        }
                    }
                }
            });
        }
    }

    // ── 共享链客户端句柄 (Ceremony 路由用) ──
    let shared_chain = tee::ceremony::new_shared_chain();

    // ── 链客户端 + TEE 证明提交 (后台任务) ──
    {
        let chain_rpc = cfg.chain_rpc.clone();
        let data_dir = cfg.data_dir.clone();
        let chain_signer_seed = cfg.chain_signer_seed.clone();
        // ⚠️ 安全: 清除 CHAIN_SIGNER_SEED 环境变量, 防止 /proc/<pid>/environ 泄漏
        std::env::remove_var("CHAIN_SIGNER_SEED");
        let bot_id_hash = cfg.bot_id_hash;
        let attestor_bg = attestor.clone();
        let metrics_bg = tee_metrics.clone();
        let batch_interval = cfg.chain_log_batch_interval;
        let batch_size = cfg.chain_log_batch_size;
        let router_bg = router.clone();
        let shared_chain_bg = shared_chain.clone();
        let enclave_mode_bg = enclave.mode().clone();
        let config_manager_bg = config_manager.clone();
        // R3: 复用提前连接的链客户端 (如果有)
        let early_chain_bg = early_chain;

        tokio::spawn(async move {
            // R3: 优先复用 early_chain, 避免重复连接
            let connect_result = if let Some(existing) = early_chain_bg {
                info!("复用提前连接的链客户端");
                Ok(existing)
            } else {
                let signer = chain::client::load_or_generate_signer(
                    &data_dir, chain_signer_seed.as_deref(),
                );
                ChainClient::connect(&chain_rpc, signer).await
                    .map(Arc::new)
            };
            match connect_result {
                Ok(client) => {
                    info!("链客户端连接成功");

                    // 注入到 router
                    router_bg.set_chain(client.clone()).await;

                    // 注入到 ceremony 路由 (AttestationGuard 链上查询)
                    {
                        let mut guard = shared_chain_bg.write().await;
                        *guard = Some(client.clone());
                    }
                    info!("链客户端已注入 Ceremony 路由 (AttestationGuard 启用)");

                    // 启动 ConfigManager 链上同步循环
                    {
                        let cm = config_manager_bg.clone();
                        let chain_sync = client.clone();
                        tokio::spawn(async move {
                            cm.sync_loop(chain_sync).await;
                        });
                        info!("ConfigManager 链上同步循环已启动");
                    }

                    // 检测 TEE 模式 (启动证明 + 刷新循环共用)
                    let is_hardware = enclave_mode_bg.is_hardware();

                    // 启动时提交 TEE 证明
                    if is_hardware {
                        // Hardware 模式: 立即执行完整 nonce + DCAP Level 4 流程
                        // 避免使用 submit_attestation (quote_verified=false) 导致启动后
                        // 长时间处于未验证状态
                        match refresh_hardware_attestation(
                            &client, &attestor_bg, bot_id_hash,
                        ).await {
                            Ok(()) => {
                                info!("启动时硬件证明已提交 (DCAP Level 4)");
                                metrics_bg.record_chain_tx(true);
                            }
                            Err(e) => {
                                warn!(error = %e, "启动时硬件证明提交失败, 降级到软件证明");
                                // 降级: 提交软件模式证明作为兜底
                                if let Some(bundle) = attestor_bg.current_attestation() {
                                    if let Err(e2) = client.submit_attestation(bot_id_hash, &bundle).await {
                                        warn!(error = %e2, "降级软件证明也提交失败");
                                        metrics_bg.record_chain_tx(false);
                                    } else {
                                        info!("降级软件证明已提交 (quote_verified=false)");
                                        metrics_bg.record_chain_tx(true);
                                    }
                                }
                            }
                        }
                    } else if let Some(bundle) = attestor_bg.current_attestation() {
                        // Software 模式: 使用 submit_attestation
                        match client.submit_attestation(bot_id_hash, &bundle).await {
                            Ok(()) => {
                                info!("TEE 证明已提交链上 (软件模式)");
                                metrics_bg.record_chain_tx(true);
                            }
                            Err(e) => {
                                warn!(error = %e, "TEE 证明提交失败");
                                metrics_bg.record_chain_tx(false);
                            }
                        }
                    }

                    // 启动日志批量提交器
                    let batcher_chain = client.clone();
                    tokio::spawn(async move {
                        let batcher = ActionLogBatcher::new(
                            log_rx, batcher_chain, batch_interval, batch_size,
                        );
                        batcher.run().await;
                    });

                    // 24h 证明刷新循环
                    let refresh_secs = tee::attestor::QUOTE_VALIDITY_SECS
                        - tee::attestor::QUOTE_REFRESH_MARGIN_SECS;
                    let mut refresh_interval = tokio::time::interval(
                        std::time::Duration::from_secs(refresh_secs)
                    );
                    refresh_interval.tick().await;
                    loop {
                        refresh_interval.tick().await;
                        info!(hardware = is_hardware, "开始刷新 TEE 证明...");

                        if is_hardware {
                            // Hardware 模式: nonce 防重放流程
                            // 1. request_attestation_nonce → 链上存储 nonce
                            // 2. 读取 nonce
                            // 3. 生成带 nonce 的 Quote
                            // 4. submit_verified_attestation (链上解析 MRTD + 验证 nonce)
                            match refresh_hardware_attestation(
                                &client, &attestor_bg, bot_id_hash,
                            ).await {
                                Ok(()) => {
                                    info!("TEE 硬件证明刷新成功 (nonce 防重放)");
                                    metrics_bg.record_quote_refresh(true);
                                    metrics_bg.record_chain_tx(true);
                                }
                                Err(e) => {
                                    warn!(error = %e, "TEE 硬件证明刷新失败");
                                    metrics_bg.record_quote_refresh(false);
                                    metrics_bg.record_chain_tx(false);
                                }
                            }
                        } else {
                            // Software 模式: 直接生成 + refresh_attestation
                            match attestor_bg.generate_attestation() {
                                Ok(bundle) => {
                                    match client.refresh_attestation(bot_id_hash, &bundle).await {
                                        Ok(()) => {
                                            info!("TEE 证明刷新成功 (软件模式)");
                                            metrics_bg.record_quote_refresh(true);
                                            metrics_bg.record_chain_tx(true);
                                        }
                                        Err(e) => {
                                            warn!(error = %e, "TEE 证明刷新提交失败");
                                            metrics_bg.record_quote_refresh(false);
                                            metrics_bg.record_chain_tx(false);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "TEE 证明重新生成失败");
                                    metrics_bg.record_quote_refresh(false);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "链客户端连接失败（日志将不会提交）");
                }
            }
        });
    }

    // ── Prometheus /metrics 路由 ──
    let metrics_router = Router::new()
        .route("/metrics", get(metrics::metrics_handler))
        .with_state(tee_metrics);

    // ── Ceremony 路由 (Share 接收 + Share 服务) ──
    // shared_chain 已在前面声明, 链客户端异步连接后注入
    let ceremony_router = tee::ceremony::ceremony_routes(enclave.clone(), shared_chain.clone());

    // ── RA-TLS Provision 路由 (DApp → TEE 端到端加密 Token 注入) ──
    // connect 模式: SGX 代理 (provision_client), Token 明文仅在 SGX enclave
    // inprocess 模式: TDX 本地 (provision_vault), Token 在 TDX 进程内存
    let provision_router = tee::ra_tls::provision_routes(
        enclave.clone(), provision_vault, provision_client, cfg.provision_secret.clone(),
    );

    // ── HTTP 路由 ──
    let app = Router::new()
        .route("/webhook", post(webhook::handle_webhook))
        .route("/health", get(webhook::handle_health))
        .route("/v1/status", get(webhook::handle_status))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .with_state(state)
        .merge(metrics_router)
        .merge(ceremony_router)
        .merge(provision_router);

    // ── Ceremony 独立端口 (可选) ──
    if cfg.ceremony_port > 0 && cfg.ceremony_port != cfg.webhook_port {
        let ceremony_addr = format!("0.0.0.0:{}", cfg.ceremony_port);
        let ceremony_app = tee::ceremony::ceremony_routes(enclave.clone(), shared_chain.clone());
        let ceremony_listener = tokio::net::TcpListener::bind(&ceremony_addr).await?;
        info!(addr = %ceremony_addr, "Ceremony HTTP 端口已启动");
        tokio::spawn(async move {
            if let Err(e) = axum::serve(ceremony_listener, ceremony_app).await {
                error!(error = %e, "Ceremony HTTP 服务异常");
            }
        });
    }

    // ── 启动服务器 ──
    let addr = format!("0.0.0.0:{}", cfg.webhook_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "GroupRobot Bot HTTP 服务器启动");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

/// Hardware 模式证明刷新: nonce 防重放 + DCAP Level 4 全证书链验证
///
/// 1. request_attestation_nonce → 链上存储 nonce
/// 2. 等待 + 读取 nonce
/// 3. generate_attestation_with_nonce(nonce) — 同时提取证书链 (PEM→DER)
/// 4. submit_dcap_full_attestation (Level 4: 4 层 ECDSA 签名验证)
///    ↳ 证书链提取失败时降级到 submit_verified_attestation (Level 1)
async fn refresh_hardware_attestation(
    client: &Arc<ChainClient>,
    attestor: &Arc<tee::attestor::Attestor>,
    bot_id_hash: [u8; 32],
) -> anyhow::Result<()> {
    // Step 1: 请求 nonce
    client.request_attestation_nonce(bot_id_hash).await
        .map_err(|e| anyhow::anyhow!("request_nonce: {}", e))?;
    info!("已请求链上 Nonce");

    // Step 2: 等待一个区块后读取 nonce (6s)
    tokio::time::sleep(std::time::Duration::from_secs(7)).await;

    let nonce = client.query_attestation_nonce(&bot_id_hash).await
        .map_err(|e| anyhow::anyhow!("query_nonce: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("nonce not found on chain after request"))?;
    info!(nonce = %hex::encode(&nonce[..8]), "链上 Nonce 已获取");

    // Step 3: 生成带 nonce 的 TDX Quote (同时提取证书链)
    let bundle = attestor.generate_attestation_with_nonce(Some(nonce))
        .map_err(|e| anyhow::anyhow!("generate_with_nonce: {}", e))?;

    if bundle.tdx_quote_raw.is_none() {
        return Err(anyhow::anyhow!("hardware attestation missing tdx_quote_raw"));
    }

    // Step 4: 优先 Level 4, 降级到 Level 1
    if bundle.pck_cert_der.is_some() && bundle.intermediate_cert_der.is_some() {
        // Level 4: Intel Root CA → Intermediate → PCK → QE Report → AK → Body
        info!("提交 DCAP Level 4 全证书链证明...");
        client.submit_dcap_full_attestation(bot_id_hash, &bundle).await
            .map_err(|e| anyhow::anyhow!("submit_dcap_full (L4): {}", e))?;
        info!("✅ DCAP Level 4 证明已上链 (quote_verified=true, dcap_level=4)");
    } else {
        // Level 1 降级: 仅结构解析 + nonce 验证
        warn!("证书链不可用, 降级到 Level 1 (quote_verified=false)");
        client.submit_verified_attestation(bot_id_hash, &bundle).await
            .map_err(|e| anyhow::anyhow!("submit_verified (L1): {}", e))?;
    }

    Ok(())
}
