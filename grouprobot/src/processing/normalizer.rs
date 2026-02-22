use sha2::{Sha256, Digest};

/// 哈希群 ID
pub fn hash_group_id(group_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"community:");
    hasher.update(group_id.as_bytes());
    hasher.finalize().into()
}

/// 哈希用户 ID
pub fn hash_user_id(user_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"user:");
    hasher.update(user_id.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let h1 = hash_group_id("-100123");
        let h2 = hash_group_id("-100123");
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_ids_different_hashes() {
        assert_ne!(hash_group_id("aaa"), hash_group_id("bbb"));
        assert_ne!(hash_user_id("111"), hash_user_id("222"));
    }
}
