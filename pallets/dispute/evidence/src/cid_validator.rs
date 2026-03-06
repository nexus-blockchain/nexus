extern crate alloc;
use alloc::vec::Vec;

pub const ENCRYPTED_PREFIXES: &[&[u8]] = &[
    b"enc-",
    b"sealed-",
    b"priv-",
    b"encrypted-",
];

pub trait CidValidator {
    fn is_encrypted(cid: &[u8]) -> bool {
        if cid.is_empty() {
            return false;
        }
        for prefix in ENCRYPTED_PREFIXES {
            if cid.starts_with(prefix) {
                return true;
            }
        }
        false
    }
}

pub struct DefaultCidValidator;
impl CidValidator for DefaultCidValidator {}

/// 剥离加密前缀，返回底层 IPFS CID 部分
pub fn strip_encrypted_prefix(cid: &[u8]) -> &[u8] {
    for prefix in ENCRYPTED_PREFIXES {
        if cid.starts_with(prefix) {
            return &cid[prefix.len()..];
        }
    }
    cid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_encrypted() {
        assert!(DefaultCidValidator::is_encrypted(b"enc-QmXxx"));
        assert!(DefaultCidValidator::is_encrypted(b"sealed-bafxxx"));
        assert!(DefaultCidValidator::is_encrypted(b"priv-bagxxx"));
        assert!(DefaultCidValidator::is_encrypted(b"encrypted-cidxxx"));
        assert!(!DefaultCidValidator::is_encrypted(b"QmXxx"));
        assert!(!DefaultCidValidator::is_encrypted(b"bafxxx"));
        assert!(!DefaultCidValidator::is_encrypted(b""));
        assert!(!DefaultCidValidator::is_encrypted(b"enc"));
    }

    #[test]
    fn test_strip_encrypted_prefix() {
        assert_eq!(strip_encrypted_prefix(b"enc-QmXxx"), b"QmXxx");
        assert_eq!(strip_encrypted_prefix(b"sealed-bafxxx"), b"bafxxx");
        assert_eq!(strip_encrypted_prefix(b"priv-bagxxx"), b"bagxxx");
        assert_eq!(strip_encrypted_prefix(b"encrypted-cidxxx"), b"cidxxx");
        assert_eq!(strip_encrypted_prefix(b"QmXxx"), b"QmXxx");
        assert_eq!(strip_encrypted_prefix(b""), b"");
    }
}
