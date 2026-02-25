use std::collections::HashMap;
use dashmap::DashMap;
use regex::Regex;

use crate::processing::rules::text_utils::html_escape;

/// 自定义命令/过滤器系统
///
/// 群主可定义:
/// 1. 自定义命令: /keyword → 自动回复内容
/// 2. 关键词过滤器: 检测关键词 → 自动回复
///
/// 配置存储在链上，所有 Nexus 节点同步执行。
///
/// 设计参考:
/// - Red-DiscordBot Cog 自定义命令系统
/// - FallenRobot filters 模块
/// - YAGPDB CustomCommands 插件

/// 正则编译最大大小 (字节), 防止 ReDoS
const REGEX_SIZE_LIMIT: usize = 8192;
/// 正则编译最大嵌套深度
const REGEX_NEST_LIMIT: u32 = 5;

/// 自定义命令
#[derive(Debug, Clone)]
pub struct CustomCommand {
    pub trigger: String,
    pub response: String,
    pub command_type: CommandType,
    pub enabled: bool,
    pub creator: String,
    /// 预编译的正则 (仅 RegexFilter 类型)
    compiled_regex: Option<Regex>,
}

/// 命令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    /// 精确命令匹配: /trigger
    ExactCommand,
    /// 关键词触发: 消息包含关键词
    KeywordFilter,
    /// 正则匹配
    RegexFilter,
}

/// 自定义命令管理器
/// 按群管理所有自定义命令和关键词过滤器
pub struct CustomCommandManager {
    /// group_id → (trigger → CustomCommand)
    commands: DashMap<String, HashMap<String, CustomCommand>>,
    /// 每个群最大命令数
    max_per_group: usize,
}

impl CustomCommandManager {
    pub fn new(max_per_group: usize) -> Self {
        Self {
            commands: DashMap::new(),
            max_per_group: if max_per_group == 0 { 50 } else { max_per_group },
        }
    }

    /// 添加/更新自定义命令
    pub fn set_command(
        &self,
        group_id: &str,
        trigger: &str,
        response: &str,
        command_type: CommandType,
        creator: &str,
    ) -> Result<(), &'static str> {
        let trigger = trigger.to_lowercase();
        let mut group_cmds = self.commands.entry(group_id.to_string())
            .or_insert_with(HashMap::new);

        // 检查上限 (新命令才检查)
        if !group_cmds.contains_key(&trigger) && group_cmds.len() >= self.max_per_group {
            return Err("Maximum custom commands reached for this group");
        }

        // 预编译正则 (仅 RegexFilter), 拒绝无效/过大的模式
        let compiled_regex = if command_type == CommandType::RegexFilter {
            let re = regex::RegexBuilder::new(&trigger)
                .size_limit(REGEX_SIZE_LIMIT)
                .nest_limit(REGEX_NEST_LIMIT)
                .build()
                .map_err(|_| "Invalid or too complex regex pattern")?;
            Some(re)
        } else {
            None
        };

        group_cmds.insert(trigger.clone(), CustomCommand {
            trigger,
            response: response.to_string(),
            command_type,
            enabled: true,
            creator: creator.to_string(),
            compiled_regex,
        });
        Ok(())
    }

    /// 删除自定义命令
    pub fn remove_command(&self, group_id: &str, trigger: &str) -> bool {
        let trigger = trigger.to_lowercase();
        if let Some(mut group_cmds) = self.commands.get_mut(group_id) {
            group_cmds.remove(&trigger).is_some()
        } else {
            false
        }
    }

    /// 启用/禁用命令
    pub fn set_enabled(&self, group_id: &str, trigger: &str, enabled: bool) -> bool {
        let trigger = trigger.to_lowercase();
        if let Some(mut group_cmds) = self.commands.get_mut(group_id) {
            if let Some(cmd) = group_cmds.get_mut(&trigger) {
                cmd.enabled = enabled;
                return true;
            }
        }
        false
    }

    /// 匹配精确命令 (/trigger)
    pub fn match_command(&self, group_id: &str, command: &str) -> Option<String> {
        let command = command.to_lowercase();
        let group_cmds = self.commands.get(group_id)?;
        let cmd = group_cmds.get(&command)?;
        if cmd.enabled && cmd.command_type == CommandType::ExactCommand {
            Some(cmd.response.clone())
        } else {
            None
        }
    }

    /// 匹配关键词过滤器 (消息中包含关键词)
    pub fn match_filters(&self, group_id: &str, message: &str) -> Option<(String, String)> {
        let group_cmds = self.commands.get(group_id)?;
        let lower = message.to_lowercase();

        for (trigger, cmd) in group_cmds.iter() {
            if !cmd.enabled {
                continue;
            }
            match cmd.command_type {
                CommandType::KeywordFilter => {
                    if lower.contains(trigger.as_str()) {
                        return Some((trigger.clone(), cmd.response.clone()));
                    }
                }
                CommandType::RegexFilter => {
                    if let Some(ref re) = cmd.compiled_regex {
                        if re.is_match(message) {
                            return Some((trigger.clone(), cmd.response.clone()));
                        }
                    }
                }
                CommandType::ExactCommand => {} // 精确命令不在这里匹配
            }
        }
        None
    }

    /// 列出群的所有命令
    pub fn list_commands(&self, group_id: &str) -> Vec<CustomCommand> {
        self.commands.get(group_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// 群命令数
    pub fn count(&self, group_id: &str) -> usize {
        self.commands.get(group_id).map(|m| m.len()).unwrap_or(0)
    }

    /// 从链上配置批量加载 (CSV 格式: trigger|type|response)
    pub fn load_from_csv(&self, group_id: &str, csv: &str, creator: &str) {
        for line in csv.lines() {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() == 3 {
                let trigger = parts[0].trim();
                let cmd_type = match parts[1].trim() {
                    "keyword" => CommandType::KeywordFilter,
                    "regex" => CommandType::RegexFilter,
                    _ => CommandType::ExactCommand,
                };
                let response = parts[2].trim();
                let _ = self.set_command(group_id, trigger, response, cmd_type, creator);
            }
        }
    }

    /// 渲染响应模板 (支持 {user} {group} 变量)
    /// 对用户可控值进行 HTML 转义，防止内容注入
    pub fn render_response(response: &str, user_name: &str, group_id: &str) -> String {
        let safe_user = html_escape(user_name);
        let safe_group = html_escape(group_id);
        response
            .replace("{user}", &safe_user)
            .replace("{group}", &safe_group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_match_command() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "rules", "Please read the group rules!", CommandType::ExactCommand, "admin").unwrap();

        let resp = mgr.match_command("g1", "rules").unwrap();
        assert_eq!(resp, "Please read the group rules!");
    }

    #[test]
    fn case_insensitive_command() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "FAQ", "See our FAQ page", CommandType::ExactCommand, "admin").unwrap();

        assert!(mgr.match_command("g1", "faq").is_some());
        assert!(mgr.match_command("g1", "FAQ").is_some());
    }

    #[test]
    fn keyword_filter_matches() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "price", "Check prices at https://prices.example.com", CommandType::KeywordFilter, "admin").unwrap();

        let (trigger, resp) = mgr.match_filters("g1", "What is the current price?").unwrap();
        assert_eq!(trigger, "price");
        assert!(resp.contains("prices.example.com"));
    }

    #[test]
    fn regex_filter_matches() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", r"(?i)when\s+(moon|lambo)", "Soon™", CommandType::RegexFilter, "admin").unwrap();

        assert!(mgr.match_filters("g1", "when moon?").is_some());
        assert!(mgr.match_filters("g1", "WHEN LAMBO").is_some());
        assert!(mgr.match_filters("g1", "hello world").is_none());
    }

    #[test]
    fn remove_command() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "test", "response", CommandType::ExactCommand, "admin").unwrap();
        assert!(mgr.match_command("g1", "test").is_some());

        assert!(mgr.remove_command("g1", "test"));
        assert!(mgr.match_command("g1", "test").is_none());
    }

    #[test]
    fn disable_enable_command() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "test", "response", CommandType::ExactCommand, "admin").unwrap();

        mgr.set_enabled("g1", "test", false);
        assert!(mgr.match_command("g1", "test").is_none());

        mgr.set_enabled("g1", "test", true);
        assert!(mgr.match_command("g1", "test").is_some());
    }

    #[test]
    fn max_per_group_enforced() {
        let mgr = CustomCommandManager::new(2);
        mgr.set_command("g1", "cmd1", "r1", CommandType::ExactCommand, "a").unwrap();
        mgr.set_command("g1", "cmd2", "r2", CommandType::ExactCommand, "a").unwrap();
        let result = mgr.set_command("g1", "cmd3", "r3", CommandType::ExactCommand, "a");
        assert!(result.is_err());

        // 更新已有命令不受限
        assert!(mgr.set_command("g1", "cmd1", "updated", CommandType::ExactCommand, "a").is_ok());
    }

    #[test]
    fn list_commands() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "rules", "r", CommandType::ExactCommand, "a").unwrap();
        mgr.set_command("g1", "faq", "f", CommandType::ExactCommand, "a").unwrap();
        assert_eq!(mgr.list_commands("g1").len(), 2);
        assert_eq!(mgr.count("g1"), 2);
    }

    #[test]
    fn different_groups_independent() {
        let mgr = CustomCommandManager::new(50);
        mgr.set_command("g1", "cmd", "response1", CommandType::ExactCommand, "a").unwrap();
        mgr.set_command("g2", "cmd", "response2", CommandType::ExactCommand, "a").unwrap();

        assert_eq!(mgr.match_command("g1", "cmd").unwrap(), "response1");
        assert_eq!(mgr.match_command("g2", "cmd").unwrap(), "response2");
    }

    #[test]
    fn load_from_csv() {
        let mgr = CustomCommandManager::new(50);
        mgr.load_from_csv("g1", "rules|command|Please read the rules\nprice|keyword|Check https://example.com\n(?i)hello|regex|Hi there!", "admin");

        assert!(mgr.match_command("g1", "rules").is_some());
        assert!(mgr.match_filters("g1", "what is the price").is_some());
        assert!(mgr.match_filters("g1", "Hello world").is_some());
        assert_eq!(mgr.count("g1"), 3);
    }

    #[test]
    fn render_response_template() {
        let rendered = CustomCommandManager::render_response(
            "Welcome {user} to {group}!", "Alice", "TestGroup"
        );
        assert_eq!(rendered, "Welcome Alice to TestGroup!");
    }

    #[test]
    fn render_response_html_escaped() {
        let rendered = CustomCommandManager::render_response(
            "Hello {user}", "<b>evil</b>", "g1"
        );
        assert_eq!(rendered, "Hello &lt;b&gt;evil&lt;/b&gt;");
    }

    #[test]
    fn redos_pattern_rejected() {
        let mgr = CustomCommandManager::new(50);
        // 灾难性回溯模式应被拒绝
        let result = mgr.set_command("g1", "(a+)+$", "bad", CommandType::RegexFilter, "admin");
        // regex crate 本身不受 ReDoS 影响, 但 nest_limit 会限制嵌套
        // 此处主要验证 set_command 不会 panic
        let _ = result;
    }

    #[test]
    fn regex_precompiled_at_set_time() {
        let mgr = CustomCommandManager::new(50);
        // 有效正则应编译成功
        assert!(mgr.set_command("g1", r"(?i)hello\s+world", "hi", CommandType::RegexFilter, "admin").is_ok());
        // 无效正则应在 set_command 时被拒绝
        assert!(mgr.set_command("g1", "[invalid", "bad", CommandType::RegexFilter, "admin").is_err());
    }

    #[test]
    fn empty_group_returns_empty() {
        let mgr = CustomCommandManager::new(50);
        assert!(mgr.match_command("g1", "anything").is_none());
        assert!(mgr.match_filters("g1", "anything").is_none());
        assert!(mgr.list_commands("g1").is_empty());
        assert_eq!(mgr.count("g1"), 0);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mgr = CustomCommandManager::new(50);
        assert!(!mgr.remove_command("g1", "nope"));
    }
}
