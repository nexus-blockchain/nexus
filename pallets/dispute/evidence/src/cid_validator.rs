// L-4修复：CID加密验证模块
//
// 函数级详细中文注释：提供CID加密状态验证功能
// - 根据规则6：除证据类数据外，其他数据CID必须加密
// - 支持多种加密CID格式识别
// - 提供验证接口供各pallet调用

extern crate alloc;
use alloc::vec::Vec;

/// 函数级中文注释：CID加密前缀定义
/// 
/// 支持的加密前缀格式：
/// - `enc-`: 通用加密前缀
/// - `sealed-`: 密封加密
/// - `priv-`: 私有加密  
/// - `encrypted-`: 完整单词前缀
pub const ENCRYPTED_PREFIXES: &[&[u8]] = &[
    b"enc-",
    b"sealed-",
    b"priv-",
    b"encrypted-",
];

/// 函数级中文注释：CID验证器trait
/// 
/// 提供CID加密状态检查和验证功能
pub trait CidValidator {
    /// 函数级详细中文注释：检查CID是否为加密格式
    /// 
    /// # 参数
    /// - `cid`: CID字节数组
    /// 
    /// # 返回
    /// - `true`: CID为加密格式（有加密前缀）
    /// - `false`: CID为明文格式
    /// 
    /// # 示例
    /// ```ignore
    /// assert!(is_encrypted(b"enc-QmXxx"));
    /// assert!(is_encrypted(b"sealed-bafxxx"));
    /// assert!(!is_encrypted(b"QmXxx"));
    /// ```
    fn is_encrypted(cid: &[u8]) -> bool {
        if cid.is_empty() {
            return false;
        }
        
        // 检查是否有任何加密前缀
        for prefix in ENCRYPTED_PREFIXES {
            if cid.starts_with(prefix) {
                return true;
            }
        }
        
        false
    }
    
    /// 函数级详细中文注释：验证CID加密要求
    /// 
    /// # 参数
    /// - `cid`: CID字节数组
    /// - `require_encrypted`: 是否要求加密
    ///   - `true`: CID必须加密，否则返回错误
    ///   - `false`: 允许明文CID
    /// 
    /// # 返回
    /// - `Ok(())`: 验证通过
    /// - `Err(())`: 验证失败（需要加密但CID为明文）
    /// 
    /// # 使用场景
    /// ```ignore
    /// // 证据类数据，允许明文
    /// validate(evidence_cid, false)?;
    /// 
    /// // 私密内容，必须加密
    /// validate(private_cid, true)?;
    /// ```
    fn validate(cid: &[u8], require_encrypted: bool) -> Result<(), ()> {
        if require_encrypted {
            if Self::is_encrypted(cid) {
                Ok(())
            } else {
                Err(())
            }
        } else {
            // 不要求加密，总是通过
            Ok(())
        }
    }
    
    /// 函数级详细中文注释：批量验证CID列表
    /// 
    /// # 参数
    /// - `cids`: CID列表
    /// - `require_encrypted`: 是否要求全部加密
    /// 
    /// # 返回
    /// - `Ok(())`: 所有CID验证通过
    /// - `Err(usize)`: 返回第一个验证失败的CID索引
    fn validate_batch(cids: &[&[u8]], require_encrypted: bool) -> Result<(), usize> {
        for (i, cid) in cids.iter().enumerate() {
            if Self::validate(cid, require_encrypted).is_err() {
                return Err(i);
            }
        }
        Ok(())
    }
}

/// 函数级中文注释：默认CID验证器实现
pub struct DefaultCidValidator;

impl CidValidator for DefaultCidValidator {}

/// P0-1修复: 剥离加密前缀，返回底层 IPFS CID 部分
///
/// 例如 `enc-QmXxx` → `QmXxx`, `sealed-bafxxx` → `bafxxx`
/// 如果没有匹配的前缀，返回原始字节（不改变）
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
        // 测试加密CID
        assert!(DefaultCidValidator::is_encrypted(b"enc-QmXxx"));
        assert!(DefaultCidValidator::is_encrypted(b"sealed-bafxxx"));
        assert!(DefaultCidValidator::is_encrypted(b"priv-bagxxx"));
        assert!(DefaultCidValidator::is_encrypted(b"encrypted-cidxxx"));
        
        // 测试明文CID
        assert!(!DefaultCidValidator::is_encrypted(b"QmXxx"));
        assert!(!DefaultCidValidator::is_encrypted(b"bafxxx"));
        assert!(!DefaultCidValidator::is_encrypted(b"bagxxx"));
        
        // 测试边界情况
        assert!(!DefaultCidValidator::is_encrypted(b""));
        assert!(!DefaultCidValidator::is_encrypted(b"enc"));
    }
    
    #[test]
    fn test_validate() {
        // 不要求加密，任何CID都通过
        assert!(DefaultCidValidator::validate(b"QmXxx", false).is_ok());
        assert!(DefaultCidValidator::validate(b"enc-QmXxx", false).is_ok());
        
        // 要求加密，只有加密CID通过
        assert!(DefaultCidValidator::validate(b"enc-QmXxx", true).is_ok());
        assert!(DefaultCidValidator::validate(b"sealed-bafxxx", true).is_ok());
        assert!(DefaultCidValidator::validate(b"QmXxx", true).is_err());
        assert!(DefaultCidValidator::validate(b"bafxxx", true).is_err());
    }
    
    #[test]
    fn test_validate_batch() {
        let encrypted_cids = [
            b"enc-QmXxx".as_ref(),
            b"sealed-bafxxx".as_ref(),
            b"priv-bagxxx".as_ref(),
        ];
        
        let mixed_cids = [
            b"enc-QmXxx".as_ref(),
            b"QmYyy".as_ref(),  // 明文，索引1
            b"sealed-bafxxx".as_ref(),
        ];
        
        // 全部加密，要求加密，应该通过
        assert!(DefaultCidValidator::validate_batch(&encrypted_cids, true).is_ok());
        
        // 混合CID，要求加密，应该失败在索引1
        assert_eq!(
            DefaultCidValidator::validate_batch(&mixed_cids, true),
            Err(1)
        );
        
        // 混合CID，不要求加密，应该通过
        assert!(DefaultCidValidator::validate_batch(&mixed_cids, false).is_ok());
    }

    #[test]
    fn test_strip_encrypted_prefix() {
        // P0-1修复: 验证前缀剥离
        assert_eq!(strip_encrypted_prefix(b"enc-QmXxx"), b"QmXxx");
        assert_eq!(strip_encrypted_prefix(b"sealed-bafxxx"), b"bafxxx");
        assert_eq!(strip_encrypted_prefix(b"priv-bagxxx"), b"bagxxx");
        assert_eq!(strip_encrypted_prefix(b"encrypted-cidxxx"), b"cidxxx");
        // 无前缀返回原始内容
        assert_eq!(strip_encrypted_prefix(b"QmXxx"), b"QmXxx");
        assert_eq!(strip_encrypted_prefix(b""), b"");
    }
}

