use std::sync::Mutex;
use std::collections::VecDeque;
use dashmap::DashMap;

/// 滑动窗口限流器
///
/// 支持两种模式:
/// - `allow()`: 全局限流 (所有来源共享配额)
/// - `allow_for(key)`: per-key 限流 (每个 key 独立配额, 防止单一来源耗尽全局配额)
pub struct RateLimiter {
    max_requests: u32,
    window_secs: u64,
    /// 全局限流窗口
    global: Mutex<VecDeque<u64>>,
    /// per-key 限流窗口 (key → timestamps)
    per_key: DashMap<String, VecDeque<u64>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            global: Mutex::new(VecDeque::new()),
            per_key: DashMap::new(),
        }
    }

    /// 全局限流: 检查是否允许请求
    pub fn allow(&self) -> bool {
        Self::check_window(&self.global, self.max_requests, self.window_secs)
    }

    /// per-key 限流: 检查指定 key 是否允许请求
    ///
    /// 每个 key 独立拥有 max_requests/window_secs 的配额
    pub fn allow_for(&self, key: &str) -> bool {
        let now = now_secs();
        let mut entry = self.per_key.entry(key.to_string())
            .or_insert_with(VecDeque::new);
        let ts = entry.value_mut();

        // 移除过期的时间戳
        while let Some(&front) = ts.front() {
            if now - front >= self.window_secs {
                ts.pop_front();
            } else {
                break;
            }
        }

        if ts.len() < self.max_requests as usize {
            ts.push_back(now);
            true
        } else {
            false
        }
    }

    /// 清理过期的 per-key 条目 (应定期调用)
    pub fn cleanup(&self) {
        let now = now_secs();
        self.per_key.retain(|_, ts| {
            // 保留最近 window 内有活动的 key
            ts.back().map(|&last| now - last < self.window_secs * 2).unwrap_or(false)
        });
    }

    /// 当前全局窗口内的请求数
    pub fn current_count(&self) -> usize {
        let now = now_secs();
        let ts = self.global.lock().unwrap();
        ts.iter().filter(|&&t| now - t < self.window_secs).count()
    }

    fn check_window(mutex: &Mutex<VecDeque<u64>>, max: u32, window: u64) -> bool {
        let now = now_secs();
        let mut ts = mutex.lock().unwrap();
        while let Some(&front) = ts.front() {
            if now - front >= window {
                ts.pop_front();
            } else {
                break;
            }
        }
        if ts.len() < max as usize {
            ts.push_back(now);
            true
        } else {
            false
        }
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
    fn allows_under_limit() {
        let limiter = RateLimiter::new(5, 60);
        for _ in 0..5 {
            assert!(limiter.allow());
        }
    }

    #[test]
    fn blocks_over_limit() {
        let limiter = RateLimiter::new(3, 60);
        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(!limiter.allow());
    }

    #[test]
    fn current_count_tracks() {
        let limiter = RateLimiter::new(10, 60);
        assert_eq!(limiter.current_count(), 0);
        limiter.allow();
        limiter.allow();
        assert_eq!(limiter.current_count(), 2);
    }

    #[test]
    fn per_key_independent() {
        let limiter = RateLimiter::new(2, 60);
        // key "a" 用完配额
        assert!(limiter.allow_for("a"));
        assert!(limiter.allow_for("a"));
        assert!(!limiter.allow_for("a"));
        // key "b" 不受影响
        assert!(limiter.allow_for("b"));
        assert!(limiter.allow_for("b"));
        assert!(!limiter.allow_for("b"));
    }

    #[test]
    fn per_key_does_not_affect_global() {
        let limiter = RateLimiter::new(3, 60);
        // per-key 限流
        limiter.allow_for("x");
        limiter.allow_for("x");
        // 全局仍然有自己的配额
        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(!limiter.allow());
    }
}
