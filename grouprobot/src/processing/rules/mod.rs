pub mod flood;
pub mod blacklist;
pub mod command;
pub mod join;
pub mod default;
pub mod duplicate;
pub mod emoji;
pub mod link_limit;
pub mod stop_word;
pub mod warn_tracker;
pub mod similarity;
pub mod classifier;
pub mod antiphishing;
pub mod lock;
pub mod callback;
pub mod text_utils;
pub mod ad_footer;

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
