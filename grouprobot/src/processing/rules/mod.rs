pub mod flood;
pub mod blacklist;
pub mod command;
pub mod join;
pub mod default;

use async_trait::async_trait;

use crate::infra::local_store::LocalStore;
use crate::platform::MessageContext;
use crate::processing::action::ActionDecision;

/// 规则 Trait — 可插拔规则链
#[async_trait]
pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    /// 返回 Some(decision) 终止规则链; None 继续下一条规则
    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision>;
}
