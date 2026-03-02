use std::sync::Arc;
use dashmap::DashSet;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn, debug};

use crate::chain::ChainClient;
use crate::chain::types::PendingActionLog;
use crate::error::{BotError, BotResult};
use crate::platform::{PlatformExecutor, MessageContext};
use crate::processing::audit_logger::{AuditLogger, AuditEntry};
use crate::processing::normalizer::{hash_group_id, hash_user_id};
use crate::infra::local_store::LocalStore;
use crate::processing::rule_engine::RuleEngine;
use crate::chain::types::ChainCommunityConfig;
use crate::tee::key_manager::{KeyManager, SequenceManager};

/// 消息路由器 — 核心处理流水线
pub struct MessageRouter {
    rule_engine: RwLock<RuleEngine>,
    /// 链客户端 (后台异步连接，初始为 None)
    chain: RwLock<Option<Arc<ChainClient>>>,
    key_manager: Arc<KeyManager>,
    sequence: Arc<SequenceManager>,
    log_sender: mpsc::Sender<PendingActionLog>,
    /// 审计日志 (Phase 2)
    audit_logger: Arc<AuditLogger>,
    /// 是否启用链上交互 (免注册模式=false, 跳过链上日志提交和序列号去重)
    chain_enabled: bool,
    /// P1-fix: 本地 pending 序列号集合,防止并发 mark_sequence_processed TOCTOU 竞争
    pending_sequences: DashSet<u64>,
}

impl MessageRouter {
    pub fn new(
        rule_engine: RuleEngine,
        key_manager: Arc<KeyManager>,
        sequence: Arc<SequenceManager>,
        log_sender: mpsc::Sender<PendingActionLog>,
        audit_logger: Arc<AuditLogger>,
        chain_enabled: bool,
    ) -> Self {
        Self {
            rule_engine: RwLock::new(rule_engine),
            chain: RwLock::new(None),
            key_manager,
            sequence,
            log_sender,
            audit_logger,
            chain_enabled,
            pending_sequences: DashSet::new(),
        }
    }

    /// 设置链客户端 (后台连接成功后调用)
    pub async fn set_chain(&self, chain: Arc<ChainClient>) {
        let mut guard = self.chain.write().await;
        *guard = Some(chain);
    }

    /// H5 修复: 动态重建规则引擎 (链上配置变更时调用)
    pub async fn rebuild_rule_engine(
        &self,
        store: Arc<LocalStore>,
        config: &ChainCommunityConfig,
        blacklist_patterns: Vec<String>,
    ) {
        let new_engine = RuleEngine::from_config(store, config, blacklist_patterns);
        let mut guard = self.rule_engine.write().await;
        *guard = new_engine;
        info!("规则引擎已根据链上配置重建");
    }

    /// 处理一条平台事件 (核心流水线)
    pub async fn handle_event(
        &self,
        ctx: &MessageContext,
        executor: &dyn PlatformExecutor,
    ) -> BotResult<()> {
        // 1. 规则引擎评估
        let decision = self.rule_engine.read().await.evaluate(ctx).await;
        debug!(rule = %decision.matched_rule, has_action = decision.action.is_some(), "规则评估完成");

        // 2. 执行动作
        if let Some(ref action_decision) = decision.action {
            let execute_action = action_decision.to_execute_action(&ctx.group_id, ctx.channel_id.as_deref());
            let receipt = executor.execute(&execute_action).await?;

            // 3. 签名 + 入队链上日志 (免注册模式跳过)
            if self.chain_enabled {
                let sequence = self.sequence.next()
                    .map_err(|e| BotError::Internal(e.into()))?;
                let timestamp = chrono::Utc::now().timestamp() as u64;

                let (signature, _msg_hash) = self.key_manager.sign_message(
                    &self.key_manager.bot_id_hash(),
                    sequence,
                    timestamp,
                    &serde_json::to_vec(&ctx.message_text).unwrap_or_default(),
                );

                let log = PendingActionLog {
                    community_id_hash: hash_group_id(&ctx.group_id),
                    action_type: action_decision.action_type.as_u8(),
                    target_hash: hash_user_id(&action_decision.target_user),
                    sequence,
                    message_hash: receipt.message_hash,
                    signature,
                };

                if let Err(e) = self.log_sender.send(log).await {
                    warn!(error = %e, "动作日志入队失败");
                }
            }

            // 4. 审计日志 (Phase 2)
            let audit_entry = AuditEntry::new(
                &ctx.group_id,
                action_decision.action_type,
                &action_decision.target_user,
                "bot",
                action_decision.reason.as_deref(),
                Some(&decision.matched_rule),
            );
            let audit_msg = self.audit_logger.log(audit_entry);

            // 转发到日志频道 (如果配置了)
            if let Some(log_channel) = self.audit_logger.get_log_channel(&ctx.group_id) {
                let log_action = crate::platform::ExecuteAction {
                    action_type: crate::platform::ActionType::SendMessage,
                    group_id: log_channel.clone(),
                    target_user: String::new(),
                    reason: None,
                    message: Some(audit_msg),
                    duration_secs: None,
                    inline_keyboard: None,
                    callback_query_id: None,
                    channel_id: Some(log_channel),
                };
                if let Err(e) = executor.execute(&log_action).await {
                    warn!(error = %e, "审计日志转发失败");
                }
            }

            info!(
                rule = %decision.matched_rule,
                action = ?action_decision.action_type,
                target = %action_decision.target_user,
                success = receipt.success,
                "动作执行完成"
            );
        }

        // P1-fix: 先在本地 pending set 中原子性占位,防止并发实例重复提交
        if self.chain_enabled {
            let seq = self.sequence.current();
            if self.pending_sequences.insert(seq) {
                let chain_opt = self.chain.read().await.clone();
                if let Some(chain) = chain_opt {
                    let bot_hash = self.key_manager.bot_id_hash();
                    let pending_ref = self.pending_sequences.clone();
                    tokio::spawn(async move {
                        match chain.mark_sequence_processed(bot_hash, seq).await {
                            Ok(_) => { /* 成功: pending 条目保留,防止重入 */ }
                            Err(e) => {
                                warn!(error = %e, seq = seq, "序列号去重标记失败");
                                pending_ref.remove(&seq);
                            }
                        }
                    });
                } else {
                    self.pending_sequences.remove(&seq);
                }
            } else {
                debug!(seq = seq, "序列号已在本地 pending 中, 跳过重复提交");
            }
        }

        Ok(())
    }
}
