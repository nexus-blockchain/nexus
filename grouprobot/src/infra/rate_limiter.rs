use std::sync::Mutex;
use std::collections::VecDeque;

/// 滑动窗口限流器
pub struct RateLimiter {
    max_requests: u32,
    window_secs: u64,
    timestamps: Mutex<VecDeque<u64>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            timestamps: Mutex::new(VecDeque::new()),
        }
    }

    /// 检查是否允许请求
    pub fn allow(&self) -> bool {
        let now = now_secs();
        let mut ts = self.timestamps.lock().unwrap();

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

    /// 当前窗口内的请求数
    pub fn current_count(&self) -> usize {
        let now = now_secs();
        let ts = self.timestamps.lock().unwrap();
        ts.iter().filter(|&&t| now - t < self.window_secs).count()
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
}
