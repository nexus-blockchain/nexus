use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn, debug};

use crate::chain::ChainClient;
use crate::chain::types::PendingActionLog;
use crate::error::{BotError, BotResult};
use crate::platform::{PlatformExecutor, MessageContext};
use crate::processing::normalizer::{hash_group_id, hash_user_id};
use crate::processing::rule_engine::RuleEngine;
use crate::tee::key_manager::{KeyManager, SequenceManager};

/// 消息路由器 — 核心处理流水线
pub struct MessageRouter {
    rule_engine: RuleEngine,
    /// 链客户端 (后台异步连接，初始为 None)
    chain: RwLock<Option<Arc<ChainClient>>>,
    key_manager: Arc<KeyManager>,
    sequence: Arc<SequenceManager>,
    log_sender: mpsc::Sender<PendingActionLog>,
}

impl MessageRouter {
    pub fn new(
        rule_engine: RuleEngine,
        key_manager: Arc<KeyManager>,
        sequence: Arc<SequenceManager>,
        log_sender: mpsc::Sender<PendingActionLog>,
    ) -> Self {
        Self {
            rule_engine,
            chain: RwLock::new(None),
            key_manager,
            sequence,
            log_sender,
        }
    }

    /// 设置链客户端 (后台连接成功后调用)
    pub async fn set_chain(&self, chain: Arc<ChainClient>) {
        let mut guard = self.chain.write().await;
        *guard = Some(chain);
    }

    /// 处理一条平台事件 (核心流水线)
    pub async fn handle_event(
        &self,
        ctx: &MessageContext,
        executor: &dyn PlatformExecutor,
    ) -> BotResult<()> {
        // 1. 规则引擎评估
        let decision = self.rule_engine.evaluate(ctx).await;
        debug!(rule = %decision.matched_rule, has_action = decision.action.is_some(), "规则评估完成");

        // 2. 执行动作
        if let Some(ref action_decision) = decision.action {
            let execute_action = action_decision.to_execute_action(&ctx.group_id);
            let receipt = executor.execute(&execute_action).await?;

            // 3. 签名 + 入队链上日志
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

            info!(
                rule = %decision.matched_rule,
                action = ?action_decision.action_type,
                target = %action_decision.target_user,
                success = receipt.success,
                "动作执行完成"
            );
        }

        // 4. 去重标记 (异步, 不阻塞响应)
        let chain_opt = self.chain.read().await.clone();
        if let Some(chain) = chain_opt {
            let bot_hash = self.key_manager.bot_id_hash();
            let seq = self.sequence.current();
            tokio::spawn(async move {
                let _ = chain.mark_sequence_processed(bot_hash, seq).await;
            });
        }

        Ok(())
    }
}
