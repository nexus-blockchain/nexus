use dashmap::DashMap;

/// 内存缓存 — flood 计数、指纹去重
pub struct LocalStore {
    /// 计数器: key → (count, window_start_secs)
    counters: DashMap<String, (u64, u64)>,
    /// 指纹去重: fingerprint → timestamp
    fingerprints: DashMap<String, u64>,
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
        let mut entry = self.counters.entry(key.to_string()).or_insert((0, now));
        let (count, window_start) = entry.value_mut();

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
        if let Some(ts) = self.fingerprints.get(fingerprint) {
            if now - *ts < ttl_secs {
                return true; // 重复
            }
        }
        self.fingerprints.insert(fingerprint.to_string(), now);
        false
    }

    /// 清理过期条目
    pub fn cleanup_expired(&self) {
        let now = now_secs();
        // 清理过期计数器 (> 5 分钟)
        self.counters.retain(|_, (_, start)| now - *start < 300);
        // 清理过期指纹 (> 10 分钟)
        self.fingerprints.retain(|_, ts| now - *ts < 600);
    }

    pub fn counter_count(&self) -> usize {
        self.counters.len()
    }

    pub fn fingerprint_count(&self) -> usize {
        self.fingerprints.len()
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
