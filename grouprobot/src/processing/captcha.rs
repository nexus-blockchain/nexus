use rand::Rng;
use std::collections::HashMap;
use dashmap::DashMap;

/// CAPTCHA 验证系统
///
/// Level 1: 群内数学题 — 新用户入群后发送数学题，限时回答
/// Level 2: RA-TLS 网页验证 (Phase 3 实现，此处预留接口)
///
/// 设计参考:
/// - Gojo_Satoru: 图片 CAPTCHA + 超时踢出
/// - YAGPDB: Google reCAPTCHA 网页验证
/// - Nexus 独特优势: TEE 签名验证链接，结果上链

/// CAPTCHA 验证方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptchaMethod {
    /// Level 1: 群内数学题
    MathChallenge,
    /// Level 2: RA-TLS 网页 (预留)
    #[allow(dead_code)]
    RaTlsWeb,
}

/// 每个 CAPTCHA 最大尝试次数
const MAX_CAPTCHA_ATTEMPTS: u8 = 3;

/// CAPTCHA 挑战
#[derive(Debug, Clone)]
pub struct CaptchaChallenge {
    pub user_id: String,
    pub group_id: String,
    pub question: String,
    pub answer: String,
    pub method: CaptchaMethod,
    pub created_at: u64,
    pub timeout_secs: u64,
    /// 已尝试次数
    pub attempts: u8,
    /// 最大尝试次数
    pub max_attempts: u8,
}

/// CAPTCHA 验证结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptchaResult {
    /// 验证通过
    Passed,
    /// 答案错误 (剩余尝试次数)
    WrongAnswer { remaining: u8 },
    /// 挑战不存在或已过期
    NotFound,
    /// 超时
    Expired,
    /// 达到最大尝试次数
    MaxAttempts,
}

/// 数学题生成器
struct MathGenerator;

impl MathGenerator {
    /// 生成一道简单数学题 (加法/减法/乘法)
    fn generate() -> (String, String) {
        let mut rng = rand::thread_rng();
        let op = rng.gen_range(0..3);
        match op {
            0 => {
                // 加法
                let a = rng.gen_range(1..50);
                let b = rng.gen_range(1..50);
                (format!("{} + {} = ?", a, b), (a + b).to_string())
            }
            1 => {
                // 减法 (确保结果非负)
                let a = rng.gen_range(10..100);
                let b = rng.gen_range(1..=a);
                (format!("{} - {} = ?", a, b), (a - b).to_string())
            }
            _ => {
                // 乘法
                let a = rng.gen_range(2..13);
                let b = rng.gen_range(2..13);
                (format!("{} × {} = ?", a, b), (a * b).to_string())
            }
        }
    }

    /// 生成选项按钮 (含正确答案 + 3 个干扰项)
    fn generate_options(correct: &str) -> Vec<String> {
        let correct_num: i64 = correct.parse().unwrap_or(0);
        let mut rng = rand::thread_rng();
        let mut options = vec![correct_num];

        while options.len() < 4 {
            let offset = rng.gen_range(-10..=10);
            let candidate = correct_num + offset;
            if candidate != correct_num && candidate >= 0 && !options.contains(&candidate) {
                options.push(candidate);
            }
        }

        // 随机打乱
        for i in (1..options.len()).rev() {
            let j = rng.gen_range(0..=i);
            options.swap(i, j);
        }

        options.iter().map(|n| n.to_string()).collect()
    }
}

/// CAPTCHA 管理器
/// 管理所有待验证的 CAPTCHA 挑战
pub struct CaptchaManager {
    /// 待验证的挑战: key = "group_id:user_id"
    pending: DashMap<String, CaptchaChallenge>,
    /// 默认超时 (秒)
    timeout_secs: u64,
}

impl CaptchaManager {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            pending: DashMap::new(),
            timeout_secs: if timeout_secs == 0 { 120 } else { timeout_secs },
        }
    }

    fn key(group_id: &str, user_id: &str) -> String {
        format!("{}:{}", group_id, user_id)
    }

    /// 为新用户创建 CAPTCHA 挑战
    /// 返回 (问题文本, 选项按钮列表)
    pub fn create_challenge(&self, group_id: &str, user_id: &str) -> (String, Vec<String>) {
        let (question, answer) = MathGenerator::generate();
        let options = MathGenerator::generate_options(&answer);

        let challenge = CaptchaChallenge {
            user_id: user_id.to_string(),
            group_id: group_id.to_string(),
            question: question.clone(),
            answer,
            method: CaptchaMethod::MathChallenge,
            created_at: now_secs(),
            timeout_secs: self.timeout_secs,
            attempts: 0,
            max_attempts: MAX_CAPTCHA_ATTEMPTS,
        };

        let key = Self::key(group_id, user_id);
        self.pending.insert(key, challenge);

        (question, options)
    }

    /// 验证用户回答
    pub fn verify(&self, group_id: &str, user_id: &str, answer: &str) -> CaptchaResult {
        let key = Self::key(group_id, user_id);

        match self.pending.remove(&key) {
            Some((_, mut challenge)) => {
                let now = now_secs();
                if now - challenge.created_at > challenge.timeout_secs {
                    return CaptchaResult::Expired;
                }
                if answer.trim() == challenge.answer {
                    CaptchaResult::Passed
                } else {
                    challenge.attempts += 1;
                    if challenge.attempts >= challenge.max_attempts {
                        // 达到最大尝试次数, 不再重新插入
                        CaptchaResult::MaxAttempts
                    } else {
                        let remaining = challenge.max_attempts - challenge.attempts;
                        // 答错 → 重新插入 (有限重试)
                        self.pending.insert(key, challenge);
                        CaptchaResult::WrongAnswer { remaining }
                    }
                }
            }
            None => CaptchaResult::NotFound,
        }
    }

    /// 检查用户是否有待验证的 CAPTCHA
    pub fn has_pending(&self, group_id: &str, user_id: &str) -> bool {
        let key = Self::key(group_id, user_id);
        self.pending.contains_key(&key)
    }

    /// 移除过期的挑战，返回被移除的 (group_id, user_id) 列表
    pub fn cleanup_expired(&self) -> Vec<(String, String)> {
        let now = now_secs();
        let mut expired = vec![];

        self.pending.retain(|_, challenge| {
            if now - challenge.created_at > challenge.timeout_secs {
                expired.push((challenge.group_id.clone(), challenge.user_id.clone()));
                false
            } else {
                true
            }
        });

        expired
    }

    /// 手动取消挑战 (管理员 /cancel_captcha)
    pub fn cancel(&self, group_id: &str, user_id: &str) -> bool {
        let key = Self::key(group_id, user_id);
        self.pending.remove(&key).is_some()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// 获取统计信息
    pub fn stats(&self) -> HashMap<String, usize> {
        let mut by_group: HashMap<String, usize> = HashMap::new();
        for entry in self.pending.iter() {
            *by_group.entry(entry.value().group_id.clone()).or_default() += 1;
        }
        by_group
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
    fn math_generator_produces_valid() {
        for _ in 0..100 {
            let (q, a) = MathGenerator::generate();
            assert!(!q.is_empty());
            assert!(a.parse::<i64>().is_ok());
        }
    }

    #[test]
    fn options_contain_correct_answer() {
        for _ in 0..50 {
            let (_, answer) = MathGenerator::generate();
            let options = MathGenerator::generate_options(&answer);
            assert_eq!(options.len(), 4);
            assert!(options.contains(&answer));
        }
    }

    #[test]
    fn options_are_unique() {
        for _ in 0..50 {
            let (_, answer) = MathGenerator::generate();
            let options = MathGenerator::generate_options(&answer);
            let mut sorted = options.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 4);
        }
    }

    #[test]
    fn create_and_verify_correct() {
        let mgr = CaptchaManager::new(120);
        let (_, _) = mgr.create_challenge("g1", "u1");

        // Get the answer from the challenge
        let key = CaptchaManager::key("g1", "u1");
        let answer = mgr.pending.get(&key).unwrap().answer.clone();

        let result = mgr.verify("g1", "u1", &answer);
        assert_eq!(result, CaptchaResult::Passed);
        assert!(!mgr.has_pending("g1", "u1"));
    }

    #[test]
    fn verify_wrong_answer_allows_limited_retry() {
        let mgr = CaptchaManager::new(120);
        mgr.create_challenge("g1", "u1");

        // 第 1 次错误: 剩余 2 次
        let result = mgr.verify("g1", "u1", "definitely_wrong_99999");
        assert_eq!(result, CaptchaResult::WrongAnswer { remaining: 2 });
        assert!(mgr.has_pending("g1", "u1"));

        // 第 2 次错误: 剩余 1 次
        let result = mgr.verify("g1", "u1", "still_wrong");
        assert_eq!(result, CaptchaResult::WrongAnswer { remaining: 1 });
        assert!(mgr.has_pending("g1", "u1"));

        // 第 3 次错误: 达到最大尝试次数, 挑战被移除
        let result = mgr.verify("g1", "u1", "wrong_again");
        assert_eq!(result, CaptchaResult::MaxAttempts);
        assert!(!mgr.has_pending("g1", "u1"));
    }

    #[test]
    fn verify_not_found() {
        let mgr = CaptchaManager::new(120);
        assert_eq!(mgr.verify("g1", "u1", "42"), CaptchaResult::NotFound);
    }

    #[test]
    fn has_pending_works() {
        let mgr = CaptchaManager::new(120);
        assert!(!mgr.has_pending("g1", "u1"));
        mgr.create_challenge("g1", "u1");
        assert!(mgr.has_pending("g1", "u1"));
    }

    #[test]
    fn cancel_works() {
        let mgr = CaptchaManager::new(120);
        mgr.create_challenge("g1", "u1");
        assert!(mgr.cancel("g1", "u1"));
        assert!(!mgr.has_pending("g1", "u1"));
        assert!(!mgr.cancel("g1", "u1")); // already cancelled
    }

    #[test]
    fn stats_by_group() {
        let mgr = CaptchaManager::new(120);
        mgr.create_challenge("g1", "u1");
        mgr.create_challenge("g1", "u2");
        mgr.create_challenge("g2", "u3");

        let stats = mgr.stats();
        assert_eq!(*stats.get("g1").unwrap(), 2);
        assert_eq!(*stats.get("g2").unwrap(), 1);
    }

    #[test]
    fn pending_count() {
        let mgr = CaptchaManager::new(120);
        assert_eq!(mgr.pending_count(), 0);
        mgr.create_challenge("g1", "u1");
        mgr.create_challenge("g1", "u2");
        assert_eq!(mgr.pending_count(), 2);
    }

    #[test]
    fn different_groups_independent() {
        let mgr = CaptchaManager::new(120);
        mgr.create_challenge("g1", "u1");
        mgr.create_challenge("g2", "u1");
        assert!(mgr.has_pending("g1", "u1"));
        assert!(mgr.has_pending("g2", "u1"));
    }
}
