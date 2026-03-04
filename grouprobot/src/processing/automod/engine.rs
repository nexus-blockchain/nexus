use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::triggers::Trigger;
use super::conditions::Condition;
use super::effects::Effect;

/// AutoMod 规则 — 三段式可组合规则
/// 借鉴 YAGPDB AutoMod: Trigger → Condition → Effect
pub struct AutoModRule {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    /// 触发器列表 (任一触发即匹配)
    pub triggers: Vec<Box<dyn Trigger>>,
    /// 条件列表 (全部满足才执行)
    pub conditions: Vec<Box<dyn Condition>>,
    /// 效果列表 (依序执行，返回第一个效果的 ActionDecision)
    pub effects: Vec<Box<dyn Effect>>,
}

impl AutoModRule {
    pub fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            enabled: true,
            triggers: vec![],
            conditions: vec![],
            effects: vec![],
        }
    }

    pub fn trigger(mut self, t: Box<dyn Trigger>) -> Self {
        self.triggers.push(t);
        self
    }

    pub fn condition(mut self, c: Box<dyn Condition>) -> Self {
        self.conditions.push(c);
        self
    }

    pub fn effect(mut self, e: Box<dyn Effect>) -> Self {
        self.effects.push(e);
        self
    }

    #[allow(dead_code)]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// AutoMod 引擎 — 管理多条 AutoMod 规则
pub struct AutoModEngine {
    rules: Vec<AutoModRule>,
}

/// AutoMod 评估结果
#[derive(Debug)]
pub struct AutoModResult {
    pub matched_rule_id: u64,
    pub matched_rule_name: String,
    pub trigger_name: String,
    pub trigger_details: String,
    pub action: ActionDecision,
}

impl AutoModEngine {
    pub fn new() -> Self {
        Self { rules: vec![] }
    }

    pub fn add_rule(&mut self, rule: AutoModRule) {
        self.rules.push(rule);
    }

    /// 按优先级评估所有规则，返回第一个匹配的
    pub fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<AutoModResult> {
        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            // 1. 检查触发器 (任一触发即可)
            let trigger_result = rule.triggers.iter()
                .find_map(|t| t.check(ctx, store));

            let trigger_result = match trigger_result {
                Some(r) => r,
                None => continue,
            };

            // 2. 检查条件 (全部满足)
            let all_conditions_met = rule.conditions.iter()
                .all(|c| c.check(ctx));

            if !all_conditions_met {
                continue;
            }

            // 3. 应用第一个效果
            if let Some(effect) = rule.effects.first() {
                let action = effect.apply(ctx, &trigger_result);
                return Some(AutoModResult {
                    matched_rule_id: rule.id,
                    matched_rule_name: rule.name.clone(),
                    trigger_name: trigger_result.trigger_name,
                    trigger_details: trigger_result.details,
                    action,
                });
            }
        }
        None
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ActionType;
    use crate::processing::automod::triggers::*;
    use crate::processing::automod::conditions::*;
    use crate::processing::automod::effects::*;

    fn make_ctx(text: &str) -> MessageContext {
        MessageContext {
            platform: "telegram".into(),
            group_id: "g1".into(),
            sender_id: "u1".into(),
            sender_name: "test".into(),
            message_text: text.into(),
            message_id: None,
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

    #[test]
    fn basic_trigger_condition_effect() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "anti-scam")
                .trigger(Box::new(WordListTrigger::blacklist(vec!["scam".into()])))
                .condition(Box::new(NotAdminCondition))
                .effect(Box::new(WarnUserEffect))
        );

        // Trigger fires
        let result = engine.evaluate(&make_ctx("this is a scam"), &store);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.matched_rule_id, 1);
        assert_eq!(r.action.action_type, ActionType::Warn);

        // No trigger
        assert!(engine.evaluate(&make_ctx("normal message"), &store).is_none());
    }

    #[test]
    fn condition_blocks_admin() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "no-links")
                .trigger(Box::new(LinkTrigger::any_link()))
                .condition(Box::new(NotAdminCondition))
                .effect(Box::new(DeleteMessageEffect))
        );

        let mut ctx = make_ctx("visit https://example.com");
        ctx.is_admin = true;
        // Admin bypasses
        assert!(engine.evaluate(&ctx, &store).is_none());

        ctx.is_admin = false;
        // Non-admin triggers
        assert!(engine.evaluate(&ctx, &store).is_some());
    }

    #[test]
    fn multiple_rules_first_match_wins() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "scam-ban")
                .trigger(Box::new(WordListTrigger::blacklist(vec!["scam".into()])))
                .effect(Box::new(BanUserEffect))
        );
        engine.add_rule(
            AutoModRule::new(2, "any-link-warn")
                .trigger(Box::new(LinkTrigger::any_link()))
                .effect(Box::new(WarnUserEffect))
        );

        // "scam" triggers rule 1 (ban)
        let r = engine.evaluate(&make_ctx("scam https://evil.com"), &store).unwrap();
        assert_eq!(r.matched_rule_id, 1);
        assert_eq!(r.action.action_type, ActionType::Ban);
    }

    #[test]
    fn disabled_rule_skipped() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "disabled")
                .trigger(Box::new(WordListTrigger::blacklist(vec!["test".into()])))
                .effect(Box::new(BanUserEffect))
                .enabled(false)
        );

        assert!(engine.evaluate(&make_ctx("test message"), &store).is_none());
    }

    #[test]
    fn multiple_triggers_any_fires() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "spam-detection")
                .trigger(Box::new(WordListTrigger::blacklist(vec!["spam".into()])))
                .trigger(Box::new(LinkTrigger::any_link()))
                .effect(Box::new(WarnUserEffect))
        );

        // Word matches
        assert!(engine.evaluate(&make_ctx("spam here"), &store).is_some());
        // Link matches
        assert!(engine.evaluate(&make_ctx("visit http://x.com"), &store).is_some());
        // Neither
        assert!(engine.evaluate(&make_ctx("normal msg"), &store).is_none());
    }

    #[test]
    fn multiple_conditions_all_required() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "strict-rule")
                .trigger(Box::new(WordListTrigger::blacklist(vec!["bad".into()])))
                .condition(Box::new(NotAdminCondition))
                .condition(Box::new(NotCommandCondition))
                .condition(Box::new(NonEmptyMessageCondition))
                .effect(Box::new(WarnUserEffect))
        );

        // All conditions met
        let r = engine.evaluate(&make_ctx("bad word"), &store);
        assert!(r.is_some());

        // Command condition fails
        let mut ctx = make_ctx("bad word");
        ctx.is_command = true;
        assert!(engine.evaluate(&ctx, &store).is_none());
    }

    #[test]
    fn slowmode_with_mute() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "slowmode")
                .trigger(Box::new(SlowmodeTrigger::new(2, 60)))
                .condition(Box::new(NotAdminCondition))
                .effect(Box::new(MuteUserEffect::new(300)))
        );

        let ctx = make_ctx("msg");
        assert!(engine.evaluate(&ctx, &store).is_none()); // 1
        assert!(engine.evaluate(&ctx, &store).is_none()); // 2
        let r = engine.evaluate(&ctx, &store).unwrap();    // 3 → mute
        assert_eq!(r.action.action_type, ActionType::Mute);
        assert_eq!(r.action.duration_secs, Some(300));
    }

    #[test]
    fn member_join_alert() {
        let store = LocalStore::new();
        let mut engine = AutoModEngine::new();

        engine.add_rule(
            AutoModRule::new(1, "join-alert")
                .trigger(Box::new(MemberJoinTrigger))
                .effect(Box::new(SendAlertEffect::new("📢 New member")))
        );

        let mut ctx = make_ctx("");
        ctx.is_join_request = true;
        ctx.sender_name = "Alice".into();

        let r = engine.evaluate(&ctx, &store).unwrap();
        assert_eq!(r.action.action_type, ActionType::SendMessage);
        assert!(r.action.message.unwrap().contains("Alice"));
    }

    #[test]
    fn empty_engine_returns_none() {
        let store = LocalStore::new();
        let engine = AutoModEngine::new();
        assert!(engine.evaluate(&make_ctx("anything"), &store).is_none());
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn builder_pattern() {
        let rule = AutoModRule::new(42, "test-rule")
            .trigger(Box::new(WordListTrigger::blacklist(vec!["x".into()])))
            .condition(Box::new(NotAdminCondition))
            .effect(Box::new(WarnUserEffect))
            .enabled(true);

        assert_eq!(rule.id, 42);
        assert_eq!(rule.name, "test-rule");
        assert_eq!(rule.triggers.len(), 1);
        assert_eq!(rule.conditions.len(), 1);
        assert_eq!(rule.effects.len(), 1);
        assert!(rule.enabled);
    }
}
