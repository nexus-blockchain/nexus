//! IPFS工具模块
//!
//! 提供IPFS CID计算、验证和处理功能。

extern crate alloc;

use alloc::{vec::Vec, string::{String, ToString}};
use sp_core::blake2_256;
use crate::error::MediaError;

/// IPFS工具集
pub struct IpfsHelper;

impl IpfsHelper {
    /// 计算简化的CID（基于Blake2-256）
    ///
    /// 注意: 这是一个简化实现，生产环境应使用完整的IPFS库
    ///
    /// # 参数
    /// - `data`: 原始数据
    ///
    /// # 返回
    /// - `Ok(String)`: 十六进制编码的简化CID
    /// - `Err(MediaError)`: 计算失败
    ///
    /// # 示例
    /// ```ignore
    /// let cid = IpfsHelper::compute_cid(&image_data)?;
    /// ```
    pub fn compute_cid(data: &[u8]) -> Result<String, MediaError> {
        // 简化实现：使用 "bm" 前缀 + Blake2-256哈希的十六进制表示
        // bm = blake2-media (自定义前缀)
        let hash = blake2_256(data);
        let hex_hash = Self::bytes_to_hex(&hash);
        Ok(alloc::format!("bm{}", hex_hash))
    }

    /// 验证CID格式
    ///
    /// # 参数
    /// - `cid`: CID字符串
    ///
    /// # 返回
    /// - `Ok(())`: CID格式正确
    /// - `Err(MediaError)`: CID格式错误
    pub fn validate_cid(cid: &str) -> Result<(), MediaError> {
        // 1. 检查长度
        if cid.len() > 128 {
            return Err(MediaError::CidTooLong);
        }

        if cid.len() < 10 {
            return Err(MediaError::InvalidCidLength);
        }

        // 2. 检查我们的简化CID格式（bm + 64位十六进制）
        if cid.starts_with("bm") {
            if cid.len() != 66 { // "bm" + 64个十六进制字符
                return Err(MediaError::InvalidCid);
            }

            // 验证剩余部分是否为有效的十六进制
            let hex_part = &cid[2..];
            if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(MediaError::InvalidCidEncoding);
            }

            return Ok(());
        }

        // 3. 检查传统CIDv0格式（以Qm开头）
        if cid.starts_with("Qm") {
            if cid.len() != 46 {
                return Err(MediaError::InvalidCidV0);
            }

            // 验证Base58字符
            if !Self::is_valid_base58(cid) {
                return Err(MediaError::InvalidCidEncoding);
            }

            return Ok(());
        }

        // 4. 检查其他CIDv1格式
        if cid.starts_with('b') || cid.starts_with('z') || cid.starts_with('f') || cid.starts_with('m') {
            if cid.len() < 20 || cid.len() > 100 {
                return Err(MediaError::InvalidCidV1);
            }

            return Ok(());
        }

        Err(MediaError::InvalidCidPrefix)
    }

    /// 从CID中提取哈希值（仅适用于我们生成的简化CID）
    ///
    /// # 参数
    /// - `cid`: CID字符串
    ///
    /// # 返回
    /// - `Ok([u8; 32])`: 提取的Blake2-256哈希
    /// - `Err(MediaError)`: 提取失败
    pub fn extract_hash_from_cid(cid: &str) -> Result<[u8; 32], MediaError> {
        // 1. 验证CID格式
        Self::validate_cid(cid)?;

        // 2. 检查是否为我们的简化CID格式
        if !cid.starts_with("bm") || cid.len() != 66 {
            return Err(MediaError::InvalidCid);
        }

        // 3. 提取十六进制部分
        let hex_part = &cid[2..];

        // 4. 转换为字节数组
        let mut hash = [0u8; 32];
        for (i, chunk) in hex_part.as_bytes().chunks(2).enumerate() {
            if i >= 32 {
                break;
            }

            let hex_str = core::str::from_utf8(chunk).map_err(|_| MediaError::InvalidCidEncoding)?;
            hash[i] = u8::from_str_radix(hex_str, 16).map_err(|_| MediaError::InvalidCidEncoding)?;
        }

        Ok(hash)
    }

    /// 验证数据与CID是否匹配
    ///
    /// # 参数
    /// - `data`: 原始数据
    /// - `cid`: CID字符串
    ///
    /// # 返回
    /// - `true`: 匹配
    /// - `false`: 不匹配
    pub fn verify_content(data: &[u8], cid: &str) -> bool {
        match Self::extract_hash_from_cid(cid) {
            Ok(expected_hash) => {
                let actual_hash = blake2_256(data);
                actual_hash == expected_hash
            },
            Err(_) => false,
        }
    }

    /// 生成IPFS网关URL
    ///
    /// # 参数
    /// - `cid`: IPFS CID
    /// - `gateway`: 网关地址（可选，默认使用ipfs.io）
    ///
    /// # 返回
    /// - IPFS网关URL
    pub fn gateway_url(cid: &str, gateway: Option<&str>) -> String {
        let base_url = gateway.unwrap_or("https://ipfs.io");
        alloc::format!("{}/ipfs/{}", base_url, cid)
    }

    /// 将字节数组转换为十六进制字符串
    fn bytes_to_hex(bytes: &[u8]) -> String {
        let mut hex_string = String::with_capacity(bytes.len() * 2);
        for &byte in bytes {
            hex_string.push_str(&alloc::format!("{:02x}", byte));
        }
        hex_string
    }

    /// 验证Base58字符串
    fn is_valid_base58(s: &str) -> bool {
        const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

        s.chars().all(|c| {
            if c as u32 > 255 {
                return false;
            }
            ALPHABET.contains(&(c as u8))
        })
    }
}

/// CID信息结构
#[derive(Clone, Debug)]
pub struct CidInfo {
    /// CID版本
    pub version: u8,
    /// 编解码器
    pub codec: String,
    /// 哈希算法
    pub hash_algorithm: String,
    /// 哈希值
    pub hash: Vec<u8>,
}

impl CidInfo {
    /// 解析CID信息（简化实现）
    pub fn parse(cid: &str) -> Result<Self, MediaError> {
        IpfsHelper::validate_cid(cid)?;

        if cid.starts_with("Qm") {
            // CIDv0
            Ok(CidInfo {
                version: 0,
                codec: "dag-pb".to_string(),
                hash_algorithm: "sha2-256".to_string(),
                hash: Vec::new(), // 简化实现，不提取实际哈希
            })
        } else {
            // CIDv1 (简化)
            Ok(CidInfo {
                version: 1,
                codec: "raw".to_string(),
                hash_algorithm: "blake2b-256".to_string(),
                hash: Vec::new(), // 简化实现
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::blake2_256;

    #[test]
    fn test_compute_cid() {
        let data = b"hello world";
        let cid = IpfsHelper::compute_cid(data).unwrap();
        assert!(!cid.is_empty());
        assert!(cid.starts_with("bm"));
        assert_eq!(cid.len(), 66); // "bm" + 64 hex characters
    }

    #[test]
    fn test_validate_cid_v0() {
        let valid_cidv0 = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
        assert!(IpfsHelper::validate_cid(valid_cidv0).is_ok());

        let invalid_cidv0 = "QmInvalid";
        assert!(IpfsHelper::validate_cid(invalid_cidv0).is_err());
    }

    #[test]
    fn test_validate_cid_v1() {
        let valid_cidv1 = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        assert!(IpfsHelper::validate_cid(valid_cidv1).is_ok());

        // Test our simplified CID format - generate a real one
        let test_data = b"test";
        let simplified_cid = IpfsHelper::compute_cid(test_data).unwrap();
        assert!(IpfsHelper::validate_cid(&simplified_cid).is_ok());
    }

    #[test]
    fn test_cid_too_long() {
        let long_cid = "Q".repeat(150);
        assert_eq!(IpfsHelper::validate_cid(&long_cid), Err(MediaError::CidTooLong));
    }

    #[test]
    fn test_verify_content() {
        let data = b"test data";
        let cid = IpfsHelper::compute_cid(data).unwrap();
        assert!(IpfsHelper::verify_content(data, &cid));
        assert!(!IpfsHelper::verify_content(b"different data", &cid));
    }

    #[test]
    fn test_gateway_url() {
        let cid = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
        let url = IpfsHelper::gateway_url(cid, None);
        assert_eq!(url, "https://ipfs.io/ipfs/QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

        let custom_url = IpfsHelper::gateway_url(cid, Some("https://gateway.pinata.cloud"));
        assert_eq!(custom_url, "https://gateway.pinata.cloud/ipfs/QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    }

    #[test]
    fn test_extract_hash_from_cid() {
        let data = b"test data";
        let cid = IpfsHelper::compute_cid(data).unwrap();
        let extracted_hash = IpfsHelper::extract_hash_from_cid(&cid).unwrap();
        let expected_hash = blake2_256(data);
        assert_eq!(extracted_hash, expected_hash);
    }

    #[test]
    fn test_is_valid_base58() {
        assert!(IpfsHelper::is_valid_base58("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"));
        assert!(!IpfsHelper::is_valid_base58("0OIl")); // Invalid characters
    }

    #[test]
    fn test_cid_info_parse() {
        let cidv0 = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
        let info = CidInfo::parse(cidv0).unwrap();
        assert_eq!(info.version, 0);
        assert_eq!(info.codec, "dag-pb");

        let cidv1 = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let info = CidInfo::parse(cidv1).unwrap();
        assert_eq!(info.version, 1);
        assert_eq!(info.codec, "raw");
    }
}