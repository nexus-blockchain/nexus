//! # 验证函数模块
//!
//! 提供 TRON 地址验证（含 Base58Check 校验和）

/// Base58 字符集
const BASE58_CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// 函数级详细中文注释：验证 TRON 地址格式 + Base58Check 校验和
///
/// # 规则
/// - 长度：34 字符
/// - 开头：'T'
/// - 编码：Base58Check（字符集 + 4 字节 SHA256d 校验和）
/// - 解码后：0x41 前缀 + 20 字节地址 + 4 字节校验和
///
/// # 参数
/// - address: TRON 地址字节数组（ASCII）
///
/// # 返回
/// - bool: 有效返回 true，无效返回 false
pub fn is_valid_tron_address(address: &[u8]) -> bool {
    // 长度检查
    if address.len() != 34 {
        return false;
    }

    // 开头检查
    if address[0] != b'T' {
        return false;
    }

    // Base58 字符集检查
    for &byte in address {
        if !BASE58_CHARS.contains(&byte) {
            return false;
        }
    }

    // 🆕 L3修复: Base58Check 校验和验证
    if let Some(decoded) = base58_decode(address) {
        // TRON 地址解码后应为 25 字节：1(0x41) + 20(地址) + 4(校验和)
        if decoded.len() != 25 {
            return false;
        }
        // 前缀必须是 0x41（TRON 主网）
        if decoded[0] != 0x41 {
            return false;
        }
        // 验证校验和：SHA256(SHA256(payload))[0..4] == checksum
        let payload = &decoded[..21];
        let checksum = &decoded[21..25];
        let hash1 = sp_core::hashing::sha2_256(payload);
        let hash2 = sp_core::hashing::sha2_256(&hash1);
        hash2[..4] == *checksum
    } else {
        false
    }
}

/// Base58 解码（无外部依赖实现）
fn base58_decode(input: &[u8]) -> Option<sp_std::vec::Vec<u8>> {
    use sp_std::vec::Vec;

    let mut result: Vec<u8> = Vec::new();

    for &ch in input {
        let val = match BASE58_CHARS.iter().position(|&c| c == ch) {
            Some(v) => v as u32,
            None => return None,
        };

        let mut carry = val;
        for byte in result.iter_mut().rev() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            result.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    // 前导 '1' 对应前导 0x00 字节
    for &ch in input {
        if ch == b'1' {
            result.insert(0, 0x00);
        } else {
            break;
        }
    }

    Some(result)
}

// ===== 单元测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_tron_address() {
        // USDT 合约地址（已知有效）
        assert!(is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"));
        // 长度不对
        assert!(!is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6"));
        // 不是T开头
        assert!(!is_valid_tron_address(b"AR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"));
        // 包含0（非Base58）
        assert!(!is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj60"));
    }

    #[test]
    fn test_base58check_checksum() {
        // 有效 TRON 地址
        assert!(is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"));

        // 修改最后一个字符 → 校验和失败
        assert!(!is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6s"));
        // 修改中间字符 → 校验和失败
        assert!(!is_valid_tron_address(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj7t"));
    }

    #[test]
    fn test_base58_decode() {
        let decoded = base58_decode(b"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
        assert!(decoded.is_some());
        let bytes = decoded.unwrap();
        assert_eq!(bytes.len(), 25);
        assert_eq!(bytes[0], 0x41); // TRON 主网前缀
    }

    // ===== R2 回归测试: 边界值 =====

    #[test]
    fn r2_empty_address_rejected() {
        assert!(!is_valid_tron_address(b""));
    }

    #[test]
    fn r2_all_t_address_rejected() {
        // 34 个 'T' — Base58 合法字符但校验和不通过
        assert!(!is_valid_tron_address(b"TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT"));
    }

    #[test]
    fn r2_non_ascii_bytes_rejected() {
        // 含非 Base58 字符 (0xff) 的 34 字节输入
        let mut bad = [b'T'; 34];
        bad[10] = 0xff;
        assert!(!is_valid_tron_address(&bad));
    }
}
