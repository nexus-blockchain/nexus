use dashmap::DashMap;

/// 内存缓存 — flood 计数、指纹去重
pub struct LocalStore {
    /// 计数器: key → (count, window_start_secs, window_duration_secs)
    counters: DashMap<String, (u64, u64, u64)>,
    /// 指纹去重: fingerprint → (timestamp, ttl_secs)
    fingerprints: DashMap<String, (u64, u64)>,
    /// L1 修复: 双桶滑动窗口计数器: key → (prev_count, curr_count, curr_window_start, window_secs)
    sliding_counters: DashMap<String, (u64, u64, u64, u64)>,
    /// 通用字符串 KV 存储 (规则模块用于状态管理)
    strings: DashMap<String, String>,
}

impl Default for LocalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalStore {
    pub fn new() -> Self {
        Self {
            counters: DashMap::new(),
            fingerprints: DashMap::new(),
            sliding_counters: DashMap::new(),
            strings: DashMap::new(),
        }
    }

    /// 递增计数器 (滑动窗口)
    /// 返回窗口内的当前计数
    pub fn increment_counter(&self, key: &str, window_secs: u64) -> u64 {
        let now = now_secs();
        let mut entry = self.counters.entry(key.to_string()).or_insert((0, now, window_secs));
        let (count, window_start, stored_window) = entry.value_mut();

        // 更新存储的窗口时长 (如果调用方传入了更大的值)
        if window_secs > *stored_window {
            *stored_window = window_secs;
        }

        if now - *window_start >= window_secs {
            // 窗口过期，重置
            *count = 1;
            *window_start = now;
        } else {
            *count += 1;
        }

        *count
    }

    /// 检查指纹是否存在 (去重)
    pub fn check_fingerprint(&self, fingerprint: &str, ttl_secs: u64) -> bool {
        let now = now_secs();
        if let Some(entry) = self.fingerprints.get(fingerprint) {
            let (ts, _) = *entry.value();
            if now - ts < ttl_secs {
                return true; // 重复
            }
        }
        self.fingerprints.insert(fingerprint.to_string(), (now, ttl_secs));
        false
    }

    /// L1 修复: 双桶滑动窗口计数器
    ///
    /// 使用前一窗口和当前窗口的加权估算，避免窗口边界突发:
    ///   estimated = prev_count * (1 - elapsed_fraction) + curr_count
    /// 参考: Cloudflare / Redis 滑动窗口算法
    pub fn increment_counter_sliding(&self, key: &str, window_secs: u64) -> u64 {
        let now = now_secs();
        let mut entry = self.sliding_counters.entry(key.to_string())
            .or_insert((0, 0, now, window_secs));
        let (prev, curr, win_start, _ws) = entry.value_mut();

        if now - *win_start >= window_secs * 2 {
            // 两个窗口都过期了，完全重置
            *prev = 0;
            *curr = 1;
            *win_start = now;
        } else if now - *win_start >= window_secs {
            // 当前窗口过期，滑动: curr → prev
            *prev = *curr;
            *curr = 1;
            *win_start = now;
        } else {
            *curr += 1;
        }

        // 计算加权估算值
        let elapsed = now.saturating_sub(*win_start);
        let fraction = if window_secs > 0 {
            (elapsed as f64) / (window_secs as f64)
        } else {
            1.0
        };
        let estimated = (*prev as f64) * (1.0 - fraction) + (*curr as f64);
        estimated.ceil() as u64
    }

    /// 清理过期条目
    /// 使用每个条目自带的窗口时长 (H3 修复: 防止 WarnTracker 30 天计数器被误清理)
    pub fn cleanup_expired(&self) {
        let now = now_secs();
        // 清理过期计数器: 超过窗口时长 2 倍的才移除
        self.counters.retain(|_, (_, start, window)| {
            now - *start < (*window).max(60) * 2
        });
        // 清理过期指纹: 超过 TTL 2 倍的才移除
        self.fingerprints.retain(|_, (ts, ttl)| {
            now - *ts < (*ttl).max(60) * 2
        });
        // 清理过期滑动窗口计数器
        self.sliding_counters.retain(|_, (_, _, start, window)| {
            now - *start < (*window).max(60) * 2
        });
    }

    pub fn counter_count(&self) -> usize {
        self.counters.len()
    }

    pub fn fingerprint_count(&self) -> usize {
        self.fingerprints.len()
    }

    /// 读取计数器当前值 (不递增)
    pub fn get_counter(&self, key: &str, window_secs: u64) -> u64 {
        let now = now_secs();
        match self.counters.get(key) {
            Some(entry) => {
                let (count, window_start, _) = *entry.value();
                if now - window_start >= window_secs {
                    0 // 窗口已过期
                } else {
                    count
                }
            }
            None => 0,
        }
    }

    /// 重置计数器
    pub fn reset_counter(&self, key: &str) {
        self.counters.remove(key);
    }

    /// 存储消息哈希用于重复检测
    /// 返回该用户在窗口内发送相同内容的次数
    pub fn record_message_hash(&self, group_id: &str, sender_id: &str, text_hash: u64, window_secs: u64) -> u64 {
        let key = format!("msghash:{}:{}:{}", group_id, sender_id, text_hash);
        self.increment_counter(&key, window_secs)
    }

    /// 读取字符串值
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.strings.get(key).map(|v| v.value().clone())
    }

    /// 设置字符串值
    pub fn set_string(&self, key: &str, value: &str) {
        self.strings.insert(key.to_string(), value.to_string());
    }

    /// 删除字符串值
    pub fn remove_string(&self, key: &str) {
        self.strings.remove(key);
    }

    /// 移除并返回所有以指定前缀开头的字符串条目
    pub fn drain_strings_with_prefix(&self, prefix: &str) -> Vec<(String, String)> {
        let keys: Vec<String> = self.strings.iter()
            .filter(|entry| entry.key().starts_with(prefix))
            .map(|entry| entry.key().clone())
            .collect();
        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some((k, v)) = self.strings.remove(&key) {
                result.push((k, v));
            }
        }
        result
    }
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
    fn counter_increments() {
        let store = LocalStore::new();
        assert_eq!(store.increment_counter("k1", 60), 1);
        assert_eq!(store.increment_counter("k1", 60), 2);
        assert_eq!(store.increment_counter("k1", 60), 3);
    }

    #[test]
    fn different_keys_independent() {
        let store = LocalStore::new();
        assert_eq!(store.increment_counter("a", 60), 1);
        assert_eq!(store.increment_counter("b", 60), 1);
        assert_eq!(store.increment_counter("a", 60), 2);
    }

    #[test]
    fn fingerprint_dedup() {
        let store = LocalStore::new();
        assert!(!store.check_fingerprint("fp1", 60)); // 首次
        assert!(store.check_fingerprint("fp1", 60));  // 重复
        assert!(!store.check_fingerprint("fp2", 60)); // 不同指纹
    }

    #[test]
    fn cleanup_works() {
        let store = LocalStore::new();
        store.increment_counter("k1", 60);
        store.check_fingerprint("fp1", 60);
        assert_eq!(store.counter_count(), 1);
        assert_eq!(store.fingerprint_count(), 1);
        // cleanup 不会删除新鲜的条目
        store.cleanup_expired();
        assert_eq!(store.counter_count(), 1);
        assert_eq!(store.fingerprint_count(), 1);
    }
}
