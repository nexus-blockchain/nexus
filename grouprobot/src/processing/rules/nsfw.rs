use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// G15: NSFW 图片检测
///
/// 检测图片/视频/GIF 消息, 标记为待审核并执行动作。
///
/// TEE 集成架构:
/// 1. 规则引擎检测到媒体消息 → 先删除 (安全优先)
/// 2. 将媒体 file_id 写入 LocalStore 审核队列
/// 3. TEE 内的 ML 分类器异步处理队列中的图片
/// 4. 如果分类为安全 → 恢复消息 (可选)
/// 5. 如果分类为 NSFW → 保持删除 + 警告/封禁
///
/// 当前实现: 基于启发式规则的初步检测 + TEE 审核队列框架。
/// ML 模型集成将在 TEE enclave 内完成 (Nexus 独特优势)。
pub struct NsfwRule {
    /// 检测模式
    mode: NsfwMode,
    /// 是否启用审核队列 (异步 TEE 分类)
    queue_for_review: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NsfwMode {
    /// 仅删除 + 警告 (不等待分类结果)
    DeleteFirst,
    /// 标记待审核 (不立即删除, 等待 TEE 分类)
    ReviewOnly,
    /// 删除 + 提交审核 + 根据结果决定后续动作
    DeleteAndReview,
}

/// 审核队列中的条目
#[derive(Debug, Clone)]
pub struct NsfwQueueEntry {
    pub group_id: String,
    pub sender_id: String,
    pub message_id: String,
    pub media_type: String,
    pub timestamp: u64,
}

impl NsfwRule {
    pub fn new(mode: NsfwMode) -> Self {
        Self {
            mode,
            queue_for_review: matches!(mode, NsfwMode::DeleteAndReview | NsfwMode::ReviewOnly),
        }
    }

    /// 从配置值创建 (0=关闭由调用方处理, 1=DeleteFirst, 2=ReviewOnly, 3=DeleteAndReview)
    pub fn from_mode(mode_value: u8) -> Self {
        let mode = match mode_value {
            2 => NsfwMode::ReviewOnly,
            3 => NsfwMode::DeleteAndReview,
            _ => NsfwMode::DeleteFirst, // 1 或其他
        };
        Self::new(mode)
    }

    /// 检查消息是否包含可能的 NSFW 媒体
    fn is_media_message(ctx: &MessageContext) -> bool {
        ctx.message_type.as_deref()
            .map(|t| matches!(t, "photo" | "video" | "animation" | "document" | "sticker"))
            .unwrap_or(false)
    }

    /// 基于文件名/标题的启发式 NSFW 检测
    /// (简单关键词匹配, 真正的 ML 检测在 TEE 中完成)
    fn heuristic_check(ctx: &MessageContext) -> bool {
        let text = ctx.message_text.to_lowercase();
        // 检查图片标题/文件名中的 NSFW 关键词
        let nsfw_keywords = [
            "nsfw", "nude", "naked", "porn", "xxx", "sex",
            "hentai", "lewd", "explicit", "18+", "adult",
        ];
        for kw in &nsfw_keywords {
            if text.contains(kw) {
                return true;
            }
        }
        false
    }

    /// 将媒体消息加入审核队列
    fn enqueue_for_review(store: &LocalStore, ctx: &MessageContext) {
        if let Some(ref msg_id) = ctx.message_id {
            let key = format!("nsfw_queue:{}:{}", ctx.group_id, msg_id);
            let entry = format!(
                "{}|{}|{}|{}",
                ctx.sender_id,
                msg_id,
                ctx.message_type.as_deref().unwrap_or("unknown"),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );
            store.set_string(&key, &entry);

            // 维护队列索引
            let idx_key = format!("nsfw_queue_idx:{}", ctx.group_id);
            let mut ids: Vec<String> = store.get_string(&idx_key)
                .map(|s| s.split(',').map(|x| x.to_string()).collect())
                .unwrap_or_default();
            ids.push(msg_id.clone());
            // 限制队列大小 (保留最近 100 条)
            if ids.len() > 100 {
                ids.drain(0..ids.len() - 100);
            }
            store.set_string(&idx_key, &ids.join(","));
        }
    }

    /// 获取待审核队列 (供 TEE 分类器读取)
    pub fn get_pending_queue(store: &LocalStore, group_id: &str) -> Vec<String> {
        let idx_key = format!("nsfw_queue_idx:{}", group_id);
        store.get_string(&idx_key)
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect())
            .unwrap_or_default()
    }

    /// 标记审核完成 (由 TEE 分类器调用)
    pub fn mark_reviewed(store: &LocalStore, group_id: &str, message_id: &str, is_nsfw: bool) {
        let key = format!("nsfw_result:{}:{}", group_id, message_id);
        store.set_string(&key, if is_nsfw { "nsfw" } else { "safe" });

        // 从队列中移除
        let queue_key = format!("nsfw_queue:{}:{}", group_id, message_id);
        store.remove_string(&queue_key);
    }

    /// 查询审核结果
    pub fn get_result(store: &LocalStore, group_id: &str, message_id: &str) -> Option<bool> {
        let key = format!("nsfw_result:{}:{}", group_id, message_id);
        store.get_string(&key).map(|s| s == "nsfw")
    }
}

#[async_trait]
impl Rule for NsfwRule {
    fn name(&self) -> &'static str { "nsfw" }

    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision> {
        if ctx.is_admin || ctx.is_command {
            return None;
        }
        if ctx.callback_query_id.is_some() {
            return None;
        }

        // 仅检查媒体消息
        if !Self::is_media_message(ctx) {
            return None;
        }

        // 启发式检测 (基于消息文本/标题)
        let heuristic_hit = Self::heuristic_check(ctx);

        // 加入审核队列
        if self.queue_for_review {
            Self::enqueue_for_review(store, ctx);
        }

        match self.mode {
            NsfwMode::DeleteFirst => {
                // 所有媒体消息如果启发式命中 → 立即删除
                if heuristic_hit {
                    if let Some(ref msg_id) = ctx.message_id {
                        return Some(ActionDecision::delete_message(msg_id));
                    }
                    return Some(ActionDecision::warn(
                        &ctx.sender_id,
                        "NSFW: 检测到可能的不当内容",
                    ));
                }
            }
            NsfwMode::ReviewOnly => {
                // 仅标记, 不采取动作 (等待 TEE 分类)
                // 已在上面 enqueue_for_review 中处理
            }
            NsfwMode::DeleteAndReview => {
                // 启发式命中 → 立即删除, 并等待 TEE 确认
                if heuristic_hit {
                    if let Some(ref msg_id) = ctx.message_id {
                        return Some(ActionDecision::delete_message(msg_id));
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;

    fn photo_ctx(caption: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: caption.into(),
            message_id: Some("msg_1".into()),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin: false,
            message_type: Some("photo".into()),
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    fn text_ctx(text: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: text.into(),
            message_id: Some("msg_1".into()),
            is_command: false,
            command: None,
            command_args: vec![],
            is_join_request: false,
            is_new_member: false,
            is_left_member: false,
            service_message_id: None,
            is_admin: false,
            message_type: None,
            callback_query_id: None,
            callback_data: None,
            channel_id: None,
        }
    }

    #[tokio::test]
    async fn text_message_passes() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteFirst);
        assert!(rule.evaluate(&text_ctx("hello world"), &store).await.is_none());
    }

    #[tokio::test]
    async fn photo_without_nsfw_caption_passes() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteFirst);
        assert!(rule.evaluate(&photo_ctx("my cat"), &store).await.is_none());
    }

    #[tokio::test]
    async fn photo_with_nsfw_caption_deleted() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteFirst);
        let d = rule.evaluate(&photo_ctx("nsfw content here"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn review_only_no_action() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::ReviewOnly);
        // 即使有 NSFW 标题, ReviewOnly 模式不执行动作
        assert!(rule.evaluate(&photo_ctx("nsfw stuff"), &store).await.is_none());
        // 但会加入队列
        let queue = NsfwRule::get_pending_queue(&store, "g1");
        assert_eq!(queue.len(), 1);
    }

    #[tokio::test]
    async fn delete_and_review_deletes() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteAndReview);
        let d = rule.evaluate(&photo_ctx("nude image"), &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
        // 也加入了审核队列
        let queue = NsfwRule::get_pending_queue(&store, "g1");
        assert_eq!(queue.len(), 1);
    }

    #[tokio::test]
    async fn admin_exempt() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteFirst);
        let mut ctx = photo_ctx("nsfw");
        ctx.is_admin = true;
        assert!(rule.evaluate(&ctx, &store).await.is_none());
    }

    #[tokio::test]
    async fn mark_reviewed_and_get_result() {
        let store = LocalStore::new();
        NsfwRule::mark_reviewed(&store, "g1", "msg_1", true);
        assert_eq!(NsfwRule::get_result(&store, "g1", "msg_1"), Some(true));

        NsfwRule::mark_reviewed(&store, "g1", "msg_2", false);
        assert_eq!(NsfwRule::get_result(&store, "g1", "msg_2"), Some(false));

        assert_eq!(NsfwRule::get_result(&store, "g1", "msg_3"), None);
    }

    #[tokio::test]
    async fn from_mode_works() {
        assert_eq!(NsfwRule::from_mode(1).mode, NsfwMode::DeleteFirst);
        assert_eq!(NsfwRule::from_mode(2).mode, NsfwMode::ReviewOnly);
        assert_eq!(NsfwRule::from_mode(3).mode, NsfwMode::DeleteAndReview);
        assert_eq!(NsfwRule::from_mode(99).mode, NsfwMode::DeleteFirst);
    }

    #[tokio::test]
    async fn heuristic_keywords() {
        let keywords = ["nsfw", "nude", "naked", "porn", "xxx", "hentai", "18+"];
        for kw in &keywords {
            let mut ctx = photo_ctx(kw);
            assert!(NsfwRule::heuristic_check(&ctx), "should detect: {}", kw);
        }

        let safe_ctx = photo_ctx("beautiful sunset");
        assert!(!NsfwRule::heuristic_check(&safe_ctx));
    }

    #[tokio::test]
    async fn video_detected() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteFirst);
        let mut ctx = photo_ctx("xxx video");
        ctx.message_type = Some("video".into());
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }

    #[tokio::test]
    async fn animation_detected() {
        let store = LocalStore::new();
        let rule = NsfwRule::new(NsfwMode::DeleteFirst);
        let mut ctx = photo_ctx("porn gif");
        ctx.message_type = Some("animation".into());
        let d = rule.evaluate(&ctx, &store).await.unwrap();
        assert_eq!(d.action_type, ActionType::DeleteMessage);
    }
}
