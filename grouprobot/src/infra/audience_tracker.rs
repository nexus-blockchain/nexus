use std::collections::HashMap;
use dashmap::DashMap;
use tracing::debug;

/// 活跃成员追踪器 — 统计每个群组过去 7 天内的有效活跃人数
///
/// 用于广告 CPM 计费的 audience_size 上报。
/// 多层过滤: L1 活跃度 + L2 新成员冷却 + L4 发言质量 + L6 互动指纹
pub struct AudienceTracker {
    /// (group_id, sender_id) → MemberActivity
    members: DashMap<(String, String), MemberActivity>,
    /// group_id → 最近一次计算的 audience_size 缓存
    cache: DashMap<String, CachedAudience>,
}

/// 单个成员的活跃记录
#[derive(Debug, Clone)]
struct MemberActivity {
    /// 最后有效发言时间 (unix secs)
    last_active: u64,
    /// 加入群组时间 (unix secs, 0=未知)
    joined_at: u64,
    /// 有效发言次数 (窗口内)
    message_count: u32,
    /// 有互动行为 (回复/引用/@) 的比例权重
    interaction_weight: f64,
}

/// 缓存的 audience 计算结果
#[derive(Debug, Clone)]
struct CachedAudience {
    pub size: u32,
    pub computed_at: u64,
}

/// 发言质量检查结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageQuality {
    Valid,
    TooShort,
    EmojiOnly,
    Duplicate,
    NewMemberCooldown,
}

/// 活跃度窗口 (7 天)
const ACTIVE_WINDOW_SECS: u64 = 7 * 24 * 3600;
/// 新成员冷却期 (48 小时)
const NEW_MEMBER_COOLDOWN_SECS: u64 = 48 * 3600;
/// 最短有效消息长度
const MIN_MESSAGE_LENGTH: usize = 3;
/// audience 缓存 TTL (5 分钟)
const CACHE_TTL_SECS: u64 = 300;
/// 无互动时的降权系数
const NO_INTERACTION_WEIGHT: f64 = 0.3;

impl AudienceTracker {
    pub fn new() -> Self {
        Self {
            members: DashMap::new(),
            cache: DashMap::new(),
        }
    }

    /// 收到群消息时调用 — 更新活跃记录
    ///
    /// 返回消息质量判定 (用于调用方日志/统计)
    pub fn on_message(
        &self,
        group_id: &str,
        sender_id: &str,
        text: &str,
        joined_at: u64,
        has_interaction: bool,
    ) -> MessageQuality {
        let now = now_secs();

        // L2: 新成员 48h 冷却
        if joined_at > 0 && now.saturating_sub(joined_at) < NEW_MEMBER_COOLDOWN_SECS {
            return MessageQuality::NewMemberCooldown;
        }

        // L4: 发言质量过滤
        let trimmed = text.trim();
        if trimmed.len() < MIN_MESSAGE_LENGTH {
            return MessageQuality::TooShort;
        }

        if is_emoji_only(trimmed) {
            return MessageQuality::EmojiOnly;
        }

        // L6: 互动权重
        let weight = if has_interaction { 1.0 } else { NO_INTERACTION_WEIGHT };

        let key = (group_id.to_string(), sender_id.to_string());
        let mut entry = self.members.entry(key).or_insert(MemberActivity {
            last_active: now,
            joined_at,
            message_count: 0,
            interaction_weight: weight,
        });

        let activity = entry.value_mut();
        activity.last_active = now;
        activity.message_count = activity.message_count.saturating_add(1);
        // 滑动平均: 逐步更新互动权重
        activity.interaction_weight =
            activity.interaction_weight * 0.8 + weight * 0.2;

        // 使缓存失效
        self.cache.remove(group_id);

        MessageQuality::Valid
    }

    /// 计算群组的有效活跃人数 (加权)
    ///
    /// 结果会被链上 audience_cap 截断, 此处仅统计 Bot 侧数据。
    pub fn compute_audience_size(&self, group_id: &str) -> u32 {
        let now = now_secs();

        // 检查缓存
        if let Some(cached) = self.cache.get(group_id) {
            if now.saturating_sub(cached.computed_at) < CACHE_TTL_SECS {
                return cached.size;
            }
        }

        let cutoff = now.saturating_sub(ACTIVE_WINDOW_SECS);

        let mut weighted_sum: f64 = 0.0;
        let mut raw_count: u32 = 0;

        for entry in self.members.iter() {
            let (gid, _sid) = entry.key();
            if gid != group_id {
                continue;
            }
            let activity = entry.value();
            if activity.last_active >= cutoff {
                weighted_sum += activity.interaction_weight;
                raw_count += 1;
            }
        }

        let size = weighted_sum.ceil() as u32;

        debug!(
            group_id,
            raw_count,
            weighted_sum,
            size,
            "audience_size 计算完成"
        );

        // 更新缓存
        self.cache.insert(group_id.to_string(), CachedAudience {
            size,
            computed_at: now,
        });

        size
    }

    /// 清理过期条目 (>= 2x 窗口)
    pub fn cleanup_expired(&self) {
        let cutoff = now_secs().saturating_sub(ACTIVE_WINDOW_SECS * 2);
        self.members.retain(|_, activity| activity.last_active >= cutoff);
        // 清理所有缓存
        self.cache.clear();
    }

    /// 获取所有有广告资格的群组 (audience >= min_audience)
    pub fn eligible_groups(&self, min_audience: u32) -> Vec<(String, u32)> {
        let mut group_sizes: HashMap<String, f64> = HashMap::new();
        let now = now_secs();
        let cutoff = now.saturating_sub(ACTIVE_WINDOW_SECS);

        for entry in self.members.iter() {
            let (gid, _) = entry.key();
            let activity = entry.value();
            if activity.last_active >= cutoff {
                *group_sizes.entry(gid.clone()).or_default() += activity.interaction_weight;
            }
        }

        group_sizes
            .into_iter()
            .filter_map(|(gid, w)| {
                let size = w.ceil() as u32;
                if size >= min_audience {
                    Some((gid, size))
                } else {
                    None
                }
            })
            .collect()
    }

    /// 统计信息 (metrics/debug)
    pub fn stats(&self) -> TrackerStats {
        TrackerStats {
            total_entries: self.members.len(),
            cached_groups: self.cache.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackerStats {
    pub total_entries: usize,
    pub cached_groups: usize,
}

/// 判断文本是否纯 emoji (简化版: 检测 ASCII 可打印字符比例)
fn is_emoji_only(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let ascii_printable = text.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    // 如果没有任何字母/数字 → 视为 emoji-only
    ascii_printable == 0
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

    #[test]
    fn basic_tracking() {
        let tracker = AudienceTracker::new();
        let q = tracker.on_message("g1", "u1", "hello world", 0, true);
        assert_eq!(q, MessageQuality::Valid);

        let q = tracker.on_message("g1", "u2", "hi there", 0, false);
        assert_eq!(q, MessageQuality::Valid);

        let size = tracker.compute_audience_size("g1");
        // u1: weight=1.0, u2: weight=0.3 → ceil(1.3) = 2
        assert_eq!(size, 2);
    }

    #[test]
    fn new_member_cooldown() {
        let tracker = AudienceTracker::new();
        let now = now_secs();
        // 加入 1 小时前 (< 48h)
        let q = tracker.on_message("g1", "u1", "hello world", now - 3600, false);
        assert_eq!(q, MessageQuality::NewMemberCooldown);

        assert_eq!(tracker.compute_audience_size("g1"), 0);
    }

    #[test]
    fn too_short_filtered() {
        let tracker = AudienceTracker::new();
        let q = tracker.on_message("g1", "u1", "hi", 0, false);
        assert_eq!(q, MessageQuality::TooShort);
    }

    #[test]
    fn emoji_only_filtered() {
        let tracker = AudienceTracker::new();
        let q = tracker.on_message("g1", "u1", "😀😀😀", 0, false);
        assert_eq!(q, MessageQuality::EmojiOnly);
    }

    #[test]
    fn multiple_messages_same_user() {
        let tracker = AudienceTracker::new();
        tracker.on_message("g1", "u1", "first msg", 0, false);
        tracker.on_message("g1", "u1", "second msg", 0, true);

        // 同一用户只算 1 次, 权重逐步更新
        let size = tracker.compute_audience_size("g1");
        assert_eq!(size, 1); // ceil of weight ~0.44
    }

    #[test]
    fn different_groups_independent() {
        let tracker = AudienceTracker::new();
        tracker.on_message("g1", "u1", "hello world", 0, true);
        tracker.on_message("g2", "u2", "hi there", 0, true);

        assert_eq!(tracker.compute_audience_size("g1"), 1);
        assert_eq!(tracker.compute_audience_size("g2"), 1);
    }

    #[test]
    fn eligible_groups_filter() {
        let tracker = AudienceTracker::new();
        // g1: 25 users
        for i in 0..25 {
            tracker.on_message("g1", &format!("u{}", i), "valid message", 0, true);
        }
        // g2: 5 users (below min)
        for i in 0..5 {
            tracker.on_message("g2", &format!("u{}", i), "valid msg", 0, true);
        }

        let eligible = tracker.eligible_groups(20);
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].0, "g1");
        assert!(eligible[0].1 >= 25);
    }

    #[test]
    fn cleanup_preserves_recent() {
        let tracker = AudienceTracker::new();
        tracker.on_message("g1", "u1", "valid msg", 0, true);
        tracker.cleanup_expired();
        // 刚添加的不会被清理
        assert_eq!(tracker.compute_audience_size("g1"), 1);
    }

    #[test]
    fn cache_used() {
        let tracker = AudienceTracker::new();
        tracker.on_message("g1", "u1", "hello world", 0, true);
        let s1 = tracker.compute_audience_size("g1");
        let s2 = tracker.compute_audience_size("g1"); // 应命中缓存
        assert_eq!(s1, s2);
    }

    #[test]
    fn stats_works() {
        let tracker = AudienceTracker::new();
        tracker.on_message("g1", "u1", "hello", 0, true);
        let stats = tracker.stats();
        assert_eq!(stats.total_entries, 1);
    }
}
