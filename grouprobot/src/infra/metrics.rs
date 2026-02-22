use std::sync::Arc;
use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::family::Family;
use prometheus_client::registry::Registry;
use std::sync::Mutex;

/// 指标标签
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ActionLabel {
    pub action_type: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct StatusLabel {
    pub status: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct RuleLabel {
    pub rule: String,
}

/// 共享指标
#[derive(Clone)]
pub struct SharedMetrics {
    registry: Arc<Mutex<Registry>>,
    pub messages_total: Counter,
    pub actions_total: Family<ActionLabel, Counter>,
    pub chain_tx_total: Family<StatusLabel, Counter>,
    pub quote_refresh_total: Family<StatusLabel, Counter>,
    pub rule_matches_total: Family<RuleLabel, Counter>,
    pub active_groups: Gauge,
}

impl SharedMetrics {
    pub fn record_message(&self) {
        self.messages_total.inc();
    }

    pub fn record_action(&self, action_type: &str) {
        self.actions_total
            .get_or_create(&ActionLabel { action_type: action_type.into() })
            .inc();
    }

    pub fn record_chain_tx(&self, success: bool) {
        let status = if success { "success" } else { "failed" };
        self.chain_tx_total
            .get_or_create(&StatusLabel { status: status.into() })
            .inc();
    }

    pub fn record_quote_refresh(&self, success: bool) {
        let status = if success { "success" } else { "failed" };
        self.quote_refresh_total
            .get_or_create(&StatusLabel { status: status.into() })
            .inc();
    }

    pub fn record_rule_match(&self, rule: &str) {
        self.rule_matches_total
            .get_or_create(&RuleLabel { rule: rule.into() })
            .inc();
    }

    pub fn set_active_groups(&self, count: i64) {
        self.active_groups.set(count);
    }

    /// 渲染 Prometheus 文本格式
    pub fn render(&self) -> String {
        let registry = self.registry.lock().unwrap();
        let mut buf = String::new();
        encode(&mut buf, &registry).unwrap_or_default();
        buf
    }
}

/// 初始化指标
pub fn init_metrics() -> SharedMetrics {
    let mut registry = Registry::default();

    let messages_total = Counter::default();
    registry.register("grouprobot_messages", "Total messages processed", messages_total.clone());

    let actions_total = Family::<ActionLabel, Counter>::default();
    registry.register("grouprobot_actions", "Total actions executed", actions_total.clone());

    let chain_tx_total = Family::<StatusLabel, Counter>::default();
    registry.register("grouprobot_chain_tx", "Chain transactions", chain_tx_total.clone());

    let quote_refresh_total = Family::<StatusLabel, Counter>::default();
    registry.register("grouprobot_quote_refresh", "TEE quote refreshes", quote_refresh_total.clone());

    let rule_matches_total = Family::<RuleLabel, Counter>::default();
    registry.register("grouprobot_rule_matches", "Rule matches", rule_matches_total.clone());

    let active_groups = Gauge::default();
    registry.register("grouprobot_active_groups", "Active groups", active_groups.clone());

    SharedMetrics {
        registry: Arc::new(Mutex::new(registry)),
        messages_total,
        actions_total,
        chain_tx_total,
        quote_refresh_total,
        rule_matches_total,
        active_groups,
    }
}

/// Axum handler: GET /metrics
pub async fn metrics_handler(
    axum::extract::State(metrics): axum::extract::State<SharedMetrics>,
) -> String {
    metrics.render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_and_record() {
        let m = init_metrics();
        m.record_message();
        m.record_message();
        m.record_action("ban");
        m.record_chain_tx(true);
        m.record_quote_refresh(false);
        m.record_rule_match("flood");
        m.set_active_groups(5);
        let output = m.render();
        assert!(output.contains("grouprobot_messages"));
        assert!(output.contains("grouprobot_actions"));
    }
}
