use std::collections::VecDeque;
use dashmap::DashMap;
use serde::{Serialize, Deserialize};

use crate::platform::ActionType;

/// 审计日志条目
///
/// 所有管理动作 (ban/mute/warn/delete) 自动记录，
/// 可转发到指定日志频道，附带链上交易哈希作为审计凭证。
///
/// 设计参考:
/// - YAGPDB: modlog 插件，每个动作记录到指定频道
/// - grpmr-rs: 操作审计 + 链上哈希
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub group_id: String,
    pub action_type: u8,
    pub action_name: String,
    pub target_user: String,
    pub executor: String,
    pub reason: Option<String>,
    pub rule_name: Option<String>,
    /// 链上交易哈希 (提交后填充)
    pub tx_hash: Option<String>,
    /// 链上序列号
    pub sequence: Option<u64>,
}

impl AuditEntry {
    pub fn new(
        group_id: &str,
        action_type: ActionType,
        target_user: &str,
        executor: &str,
        reason: Option<&str>,
        rule_name: Option<&str>,
    ) -> Self {
        Self {
            timestamp: now_secs(),
            group_id: group_id.to_string(),
            action_type: action_type.as_u8(),
            action_name: action_type_name(action_type),
            target_user: target_user.to_string(),
            executor: executor.to_string(),
            reason: reason.map(|s| s.to_string()),
            rule_name: rule_name.map(|s| s.to_string()),
            tx_hash: None,
            sequence: None,
        }
    }

    /// 格式化为人类可读的日志消息
    pub fn format_message(&self) -> String {
        let mut msg = format!(
            "📋 **{}** | {} → {}",
            self.action_name,
            self.executor,
            self.target_user,
        );
        if let Some(ref reason) = self.reason {
            msg.push_str(&format!("\n📝 Reason: {}", reason));
        }
        if let Some(ref rule) = self.rule_name {
            msg.push_str(&format!("\n🔧 Rule: {}", rule));
        }
        if let Some(ref tx) = self.tx_hash {
            msg.push_str(&format!("\n🔗 Tx: {}", tx));
        }
        msg
    }
}

/// 审计日志管理器
///
/// 每个群维护一个有界日志队列，支持:
/// - 实时转发到日志频道 (通过回调)
/// - 查询最近 N 条日志
/// - 按用户过滤
pub struct AuditLogger {
    /// group_id → 日志队列
    logs: DashMap<String, VecDeque<AuditEntry>>,
    /// 每个群最大日志条数
    max_per_group: usize,
    /// 日志频道映射: group_id → log_channel_id
    log_channels: DashMap<String, String>,
}

impl AuditLogger {
    pub fn new(max_per_group: usize) -> Self {
        Self {
            logs: DashMap::new(),
            max_per_group: if max_per_group == 0 { 1000 } else { max_per_group },
            log_channels: DashMap::new(),
        }
    }

    /// 设置群的日志频道
    pub fn set_log_channel(&self, group_id: &str, channel_id: &str) {
        self.log_channels.insert(group_id.to_string(), channel_id.to_string());
    }

    /// 获取群的日志频道
    pub fn get_log_channel(&self, group_id: &str) -> Option<String> {
        self.log_channels.get(group_id).map(|v| v.clone())
    }

    /// 记录一条审计日志
    /// 返回格式化的日志消息 (可用于转发到日志频道)
    pub fn log(&self, entry: AuditEntry) -> String {
        let msg = entry.format_message();
        let group_id = entry.group_id.clone();

        let mut queue = self.logs.entry(group_id).or_insert_with(VecDeque::new);
        if queue.len() >= self.max_per_group {
            queue.pop_front();
        }
        queue.push_back(entry);

        msg
    }

    /// 查询群的最近 N 条日志
    pub fn recent(&self, group_id: &str, count: usize) -> Vec<AuditEntry> {
        match self.logs.get(group_id) {
            Some(queue) => {
                let start = queue.len().saturating_sub(count);
                queue.iter().skip(start).cloned().collect()
            }
            None => vec![],
        }
    }

    /// 查询针对特定用户的日志
    pub fn by_user(&self, group_id: &str, target_user: &str, count: usize) -> Vec<AuditEntry> {
        match self.logs.get(group_id) {
            Some(queue) => {
                queue.iter()
                    .rev()
                    .filter(|e| e.target_user == target_user)
                    .take(count)
                    .cloned()
                    .collect()
            }
            None => vec![],
        }
    }

    /// 群日志总数
    pub fn count(&self, group_id: &str) -> usize {
        self.logs.get(group_id).map(|q| q.len()).unwrap_or(0)
    }

    /// 全局日志总数
    pub fn total_count(&self) -> usize {
        self.logs.iter().map(|e| e.value().len()).sum()
    }
}

fn action_type_name(action_type: ActionType) -> String {
    match action_type {
        ActionType::Kick => "KICK",
        ActionType::Ban => "BAN",
        ActionType::Mute => "MUTE",
        ActionType::Warn => "WARN",
        ActionType::Unmute => "UNMUTE",
        ActionType::Unban => "UNBAN",
        ActionType::DeleteMessage => "DELETE",
        ActionType::SendMessage => "MESSAGE",
        ActionType::PinMessage => "PIN",
        ActionType::ApproveJoin => "APPROVE",
        ActionType::DeclineJoin => "DECLINE",
        ActionType::Promote => "PROMOTE",
        ActionType::Demote => "DEMOTE",
        ActionType::SetPermissions => "SET_PERMS",
        ActionType::EditMessage => "EDIT_MSG",
        ActionType::AnswerCallback => "CALLBACK",
    }.to_string()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(group: &str, action: ActionType, target: &str) -> AuditEntry {
        AuditEntry::new(group, action, target, "admin1", Some("test reason"), Some("blacklist"))
    }

    #[test]
    fn log_and_recent() {
        let logger = AuditLogger::new(100);
        logger.log(make_entry("g1", ActionType::Ban, "u1"));
        logger.log(make_entry("g1", ActionType::Mute, "u2"));
        logger.log(make_entry("g1", ActionType::Warn, "u3"));

        let recent = logger.recent("g1", 2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].target_user, "u2");
        assert_eq!(recent[1].target_user, "u3");
    }

    #[test]
    fn by_user_filter() {
        let logger = AuditLogger::new(100);
        logger.log(make_entry("g1", ActionType::Warn, "u1"));
        logger.log(make_entry("g1", ActionType::Ban, "u2"));
        logger.log(make_entry("g1", ActionType::Mute, "u1"));

        let u1_logs = logger.by_user("g1", "u1", 10);
        assert_eq!(u1_logs.len(), 2);
    }

    #[test]
    fn max_per_group_enforced() {
        let logger = AuditLogger::new(3);
        for i in 0..5 {
            logger.log(make_entry("g1", ActionType::Warn, &format!("u{}", i)));
        }
        assert_eq!(logger.count("g1"), 3);
        let recent = logger.recent("g1", 10);
        assert_eq!(recent[0].target_user, "u2"); // oldest kept
    }

    #[test]
    fn different_groups_independent() {
        let logger = AuditLogger::new(100);
        logger.log(make_entry("g1", ActionType::Ban, "u1"));
        logger.log(make_entry("g2", ActionType::Kick, "u2"));

        assert_eq!(logger.count("g1"), 1);
        assert_eq!(logger.count("g2"), 1);
        assert_eq!(logger.total_count(), 2);
    }

    #[test]
    fn log_channel_management() {
        let logger = AuditLogger::new(100);
        assert!(logger.get_log_channel("g1").is_none());

        logger.set_log_channel("g1", "ch_123");
        assert_eq!(logger.get_log_channel("g1").unwrap(), "ch_123");
    }

    #[test]
    fn format_message() {
        let mut entry = make_entry("g1", ActionType::Ban, "spammer");
        entry.tx_hash = Some("0xabc123".into());

        let msg = entry.format_message();
        assert!(msg.contains("BAN"));
        assert!(msg.contains("spammer"));
        assert!(msg.contains("test reason"));
        assert!(msg.contains("blacklist"));
        assert!(msg.contains("0xabc123"));
    }

    #[test]
    fn empty_group_returns_empty() {
        let logger = AuditLogger::new(100);
        assert!(logger.recent("g1", 10).is_empty());
        assert!(logger.by_user("g1", "u1", 10).is_empty());
        assert_eq!(logger.count("g1"), 0);
    }

    #[test]
    fn entry_fields_populated() {
        let entry = AuditEntry::new("g1", ActionType::Mute, "u1", "bot", None, None);
        assert_eq!(entry.action_name, "MUTE");
        assert_eq!(entry.action_type, ActionType::Mute.as_u8());
        assert!(entry.timestamp > 0);
        assert!(entry.reason.is_none());
        assert!(entry.rule_name.is_none());
        assert!(entry.tx_hash.is_none());
    }
}
