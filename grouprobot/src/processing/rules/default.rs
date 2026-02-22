use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;
use super::Rule;

/// 兜底规则 — 不触发任何动作，仅记录
pub struct DefaultRule;

#[async_trait]
impl Rule for DefaultRule {
    fn name(&self) -> &'static str { "default" }

    async fn evaluate(&self, _ctx: &MessageContext, _store: &LocalStore) -> Option<ActionDecision> {
        None
    }
}
