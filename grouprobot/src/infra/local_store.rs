use dashmap::DashMap;

/// 内存缓存 — flood 计数、指纹去重
pub struct LocalStore {
    /// 计数器: key → (count, window_start_secs, window_duration_secs)
    counters: DashMap<String, (u64, u64, u64)>,
    /// 指纹去重: fingerprint → (timestamp, ttl_secs)
    fingerprints: DashMap<String, (u64, u64)>,
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
