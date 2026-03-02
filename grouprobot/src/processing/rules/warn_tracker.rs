use crate::infra::local_store::LocalStore;
use crate::platform::ActionType;
use crate::processing::action::ActionDecision;

/// 警告升级动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarnAction {
    Kick = 0,
    Ban = 1,
    Mute = 2,
}

impl WarnAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Kick,
            1 => Self::Ban,
            2 => Self::Mute,
            _ => Self::Kick,
        }
    }
}

/// 警告追踪器 — 累积警告并在达到阈值时自动升级惩罚
///
/// 借鉴 YAGPDB 的 AddViolation + ViolationsTrigger 模式:
/// Warn 1..N-1 → 警告消息 (附带当前计数)
/// Warn N → 根据配置自动升级为 mute/kick/ban
pub struct WarnTracker {
    warn_limit: u8,
    warn_action: WarnAction,
    mute_duration: u64,
}

impl WarnTracker {
    pub fn new(warn_limit: u8, warn_action: u8, mute_duration: u64) -> Self {
        Self {
            warn_limit: if warn_limit == 0 { 3 } else { warn_limit },
            warn_action: WarnAction::from_u8(warn_action),
            mute_duration: if mute_duration == 0 { 3600 } else { mute_duration },
        }
    }

    /// 处理一个 ActionDecision，如果是 Warn 则追踪并可能升级
    /// 返回经过处理后的 ActionDecision
    pub fn process(
        &self,
        decision: ActionDecision,
        store: &LocalStore,
        group_id: &str,
    ) -> ActionDecision {
        if decision.action_type != ActionType::Warn {
            return decision;
        }

        let key = format!("warn:{}:{}", group_id, decision.target_user);
        // 30 天窗口累积警告
        let count = store.increment_counter(&key, 86400 * 30);

        if count >= self.warn_limit as u64 {
            // 达到阈值 → 升级惩罚 + 重置计数
            store.reset_counter(&key);
            let base_reason = decision.message.as_deref().unwrap_or("Warnings exceeded");
            let escalate_reason = format!(
                "{} — Warn limit reached ({}/{}), auto-escalated",
                base_reason, count, self.warn_limit
            );

            match self.warn_action {
                WarnAction::Mute => ActionDecision::mute(
                    &decision.target_user,
                    self.mute_duration,
                    &escalate_reason,
                ),
                WarnAction::Kick => ActionDecision::kick(
                    &decision.target_user,
                    &escalate_reason,
                ),
                WarnAction::Ban => ActionDecision::ban(
                    &decision.target_user,
                    &escalate_reason,
                ),
            }
        } else {
            // 未达阈值 → 附带当前计数信息
            let original_msg = decision.message.unwrap_or_default();
            ActionDecision::warn(
                &decision.target_user,
                &format!("{} ⚠️ Warning {}/{}", original_msg, count, self.warn_limit),
            )
        }
    }

    /// 查询用户当前警告计数
    pub fn get_warn_count(&self, store: &LocalStore, group_id: &str, user_id: &str) -> u64 {
        let key = format!("warn:{}:{}", group_id, user_id);
        store.get_counter(&key, 86400 * 30)
    }

    /// 重置用户警告计数 (用于 /unwarn 命令)
    pub fn reset_warns(&self, store: &LocalStore, group_id: &str, user_id: &str) {
        let key = format!("warn:{}:{}", group_id, user_id);
        store.reset_counter(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_limit_adds_count() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(3, 0, 3600);
        let decision = ActionDecision::warn("u1", "bad behavior");
        let result = tracker.process(decision, &store, "g1");
        assert_eq!(result.action_type, ActionType::Warn);
        assert!(result.message.unwrap().contains("1/3"));
    }

    #[test]
    fn at_limit_escalates_to_mute() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(3, 0, 600);

        // Warn 1 and 2
        for _ in 0..2 {
            let d = ActionDecision::warn("u1", "spam");
            let r = tracker.process(d, &store, "g1");
            assert_eq!(r.action_type, ActionType::Warn);
        }

        // Warn 3 → escalate to mute
        let d = ActionDecision::warn("u1", "spam");
        let r = tracker.process(d, &store, "g1");
        assert_eq!(r.action_type, ActionType::Mute);
        assert_eq!(r.duration_secs, Some(600));
        assert!(r.reason.unwrap().contains("auto-escalated"));
    }

    #[test]
    fn escalate_to_kick() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(2, 1, 0);

        let d = ActionDecision::warn("u1", "x");
        tracker.process(d, &store, "g1");

        let d = ActionDecision::warn("u1", "x");
        let r = tracker.process(d, &store, "g1");
        assert_eq!(r.action_type, ActionType::Kick);
    }

    #[test]
    fn escalate_to_ban() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(2, 2, 0);

        let d = ActionDecision::warn("u1", "x");
        tracker.process(d, &store, "g1");

        let d = ActionDecision::warn("u1", "x");
        let r = tracker.process(d, &store, "g1");
        assert_eq!(r.action_type, ActionType::Ban);
    }

    #[test]
    fn counter_resets_after_escalation() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(2, 0, 600);

        // Trigger escalation
        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");
        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");

        // Counter should be reset → next warn starts from 1
        let d = ActionDecision::warn("u1", "again");
        let r = tracker.process(d, &store, "g1");
        assert_eq!(r.action_type, ActionType::Warn);
        assert!(r.message.unwrap().contains("1/2"));
    }

    #[test]
    fn non_warn_passes_through() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(1, 2, 0);
        let d = ActionDecision::kick("u1", "rule violation");
        let r = tracker.process(d, &store, "g1");
        assert_eq!(r.action_type, ActionType::Kick);
    }

    #[test]
    fn different_users_independent() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(2, 2, 0);

        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");
        let r = tracker.process(ActionDecision::warn("u2", "x"), &store, "g1");
        assert_eq!(r.action_type, ActionType::Warn);
        assert!(r.message.unwrap().contains("1/2"));
    }

    #[test]
    fn different_groups_independent() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(2, 2, 0);

        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");
        let r = tracker.process(ActionDecision::warn("u1", "x"), &store, "g2");
        assert_eq!(r.action_type, ActionType::Warn);
        assert!(r.message.unwrap().contains("1/2"));
    }

    #[test]
    fn get_warn_count_works() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(5, 0, 0);

        assert_eq!(tracker.get_warn_count(&store, "g1", "u1"), 0);
        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");
        assert_eq!(tracker.get_warn_count(&store, "g1", "u1"), 1);
    }

    #[test]
    fn reset_warns_works() {
        let store = LocalStore::new();
        let tracker = WarnTracker::new(5, 0, 0);

        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");
        tracker.process(ActionDecision::warn("u1", "x"), &store, "g1");
        assert_eq!(tracker.get_warn_count(&store, "g1", "u1"), 2);

        tracker.reset_warns(&store, "g1", "u1");
        assert_eq!(tracker.get_warn_count(&store, "g1", "u1"), 0);
    }

    #[test]
    fn warn_action_from_u8() {
        assert_eq!(WarnAction::from_u8(0), WarnAction::Kick);
        assert_eq!(WarnAction::from_u8(1), WarnAction::Ban);
        assert_eq!(WarnAction::from_u8(2), WarnAction::Mute);
        assert_eq!(WarnAction::from_u8(99), WarnAction::Kick);
    }
}
