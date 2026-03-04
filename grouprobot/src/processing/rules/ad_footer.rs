use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 广告 Footer 规则 — Bot 回复命令时底部附一行广告
///
/// 仅对 Free 层级群组生效。在 DefaultRule 前拦截,
/// 对非命令消息不做任何处理 (继续规则链)。
/// 对命令消息: 不阻断, 仅修改后续回复的文本, 附加广告 footer。
///
/// 注意: 此规则不直接发送消息, 而是返回 None 让规则链继续。
/// 广告 footer 的附加由 AdDeliveryLoop 管理的缓存来提供给 executor。
pub struct AdFooterRule {
    /// 当前活跃的广告文本 (由 AdDeliveryLoop 定期更新)
    footer_text: std::sync::RwLock<Option<String>>,
    /// 是否启用
    enabled: bool,
}

impl AdFooterRule {
    pub fn new(enabled: bool) -> Self {
        Self {
            footer_text: std::sync::RwLock::new(None),
            enabled,
        }
    }

    /// 由 AdDeliveryLoop 调用, 更新当前的广告 footer 文本
    pub fn set_footer(&self, text: Option<String>) {
        if let Ok(mut guard) = self.footer_text.write() {
            *guard = text;
        }
    }

    /// 获取当前 footer (供 executor 使用)
    pub fn get_footer(&self) -> Option<String> {
        self.footer_text.read().ok().and_then(|g| g.clone())
    }

    /// 构造标准广告 footer 格式
    pub fn format_footer(ad_text: &str, ad_url: &str) -> String {
        format!(
            "\n────\n📢 {}\n🔗 {}\n由 Nexus 广告网络提供 | 升级 Pro 去广告",
            ad_text, ad_url
        )
    }
}

#[async_trait]
impl Rule for AdFooterRule {
    fn name(&self) -> &'static str { "ad_footer" }

    async fn evaluate(&self, _ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        // AdFooterRule 不直接产生 action —
        // 它通过 get_footer() 供 executor 在发送回复时附加。
        // 规则链继续。
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_format() {
        let footer = AdFooterRule::format_footer("Try Nexus DEX", "https://nexus.app");
        assert!(footer.contains("Try Nexus DEX"));
        assert!(footer.contains("https://nexus.app"));
        assert!(footer.contains("升级 Pro 去广告"));
    }

    #[test]
    fn set_and_get_footer() {
        let rule = AdFooterRule::new(true);
        assert!(rule.get_footer().is_none());

        rule.set_footer(Some("Ad text".to_string()));
        assert_eq!(rule.get_footer().unwrap(), "Ad text");

        rule.set_footer(None);
        assert!(rule.get_footer().is_none());
    }

    #[test]
    fn disabled_rule() {
        let rule = AdFooterRule::new(false);
        assert!(!rule.enabled);
    }

    #[tokio::test]
    async fn evaluate_returns_none() {
        let rule = AdFooterRule::new(true);
        rule.set_footer(Some("Ad".to_string()));

        let store = LocalStore::new();
        let ctx = MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "User".into(),
            message_text: "/help".into(),
            message_id: None,
            is_command: true,
            command: Some("help".into()),
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
        };

        // AdFooterRule never blocks — always returns None
        let result = rule.evaluate(&ctx, &store).await;
        assert!(result.is_none());
    }
}
