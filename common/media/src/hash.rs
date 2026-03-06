//! 哈希工具模块
//!
//! 提供各种哈希计算功能。


use sp_core::{blake2_128, blake2_256, H256};

/// 哈希工具集
pub struct HashHelper;

impl HashHelper {
	/// 计算内容的Blake2-256哈希
	///
	/// # 参数
	/// - `data`: 原始数据
	///
	/// # 返回
	/// - 32字节的Blake2-256哈希值
	///
	/// # 示例
	/// ```ignore
	/// let hash = HashHelper::content_hash(&image_data);
	/// ```
	pub fn content_hash(data: &[u8]) -> [u8; 32] {
		blake2_256(data)
	}

	/// 计算内容的Blake2-128哈希(用于快速校验)
	///
	/// # 参数
	/// - `data`: 原始数据
	///
	/// # 返回
	/// - 16字节的Blake2-128哈希值
	pub fn quick_hash(data: &[u8]) -> [u8; 16] {
		blake2_128(data)
	}

	/// 计算H256哈希(用于承诺)
	///
	/// # 参数
	/// - `data`: 原始数据
	///
	/// # 返回
	/// - H256类型的哈希值
	pub fn commitment_hash(data: &[u8]) -> H256 {
		H256::from(blake2_256(data))
	}

	/// 计算带盐的哈希
	///
	/// # 参数
	/// - `data`: 原始数据
	/// - `salt`: 盐值
	///
	/// # 返回
	/// - 32字节的Blake2-256哈希值
	pub fn salted_hash(data: &[u8], salt: &[u8]) -> [u8; 32] {
		let mut combined = sp_std::vec::Vec::with_capacity(data.len() + salt.len());
		combined.extend_from_slice(data);
		combined.extend_from_slice(salt);
		blake2_256(&combined)
	}

	/// 验证内容哈希
	///
	/// # 参数
	/// - `data`: 原始数据
	/// - `expected_hash`: 期望的哈希值
	///
	/// # 返回
	/// - `true` 如果哈希匹配, `false` 否则
	pub fn verify_hash(data: &[u8], expected_hash: &[u8; 32]) -> bool {
		&Self::content_hash(data) == expected_hash
	}

	/// 计算Evidence承诺哈希
	///
	/// 格式: H(ns || subject_id || cid || salt || version)
	///
	/// # 参数
	/// - `ns`: 命名空间(8字节)
	/// - `subject_id`: 主体ID
	/// - `cid`: IPFS CID
	/// - `salt`: 盐值
	/// - `version`: 版本号
	///
	/// # 返回
	/// - H256类型的承诺哈希
	///
	/// # 示例
	/// ```ignore
	/// let commit = HashHelper::evidence_commitment(
	///     &ns,
	///     target_id,
	///     &cid,
	///     &salt,
	///     1, // version
	/// );
	/// ```
	pub fn evidence_commitment(
		ns: &[u8; 8],
		subject_id: u64,
		cid: &[u8],
		salt: &[u8],
		version: u32,
	) -> H256 {
		let mut data = sp_std::vec::Vec::new();
		data.extend_from_slice(ns);
		data.extend_from_slice(&subject_id.to_le_bytes());
		data.extend_from_slice(cid);
		data.extend_from_slice(salt);
		data.extend_from_slice(&version.to_le_bytes());

		H256::from(blake2_256(&data))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_content_hash() {
		let data = b"hello world";
		let hash = HashHelper::content_hash(data);
		assert_eq!(hash.len(), 32);
	}

	#[test]
	fn test_quick_hash() {
		let data = b"hello world";
		let hash = HashHelper::quick_hash(data);
		assert_eq!(hash.len(), 16);
	}

	#[test]
	fn test_verify_hash() {
		let data = b"hello world";
		let hash = HashHelper::content_hash(data);
		assert!(HashHelper::verify_hash(data, &hash));
		assert!(!HashHelper::verify_hash(b"hello", &hash));
	}

	#[test]
	fn test_salted_hash() {
		let data = b"hello";
		let salt = b"world";
		let hash1 = HashHelper::salted_hash(data, salt);
		let hash2 = HashHelper::salted_hash(data, b"different");
		assert_ne!(hash1, hash2);
	}

	#[test]
	fn test_evidence_commitment() {
		let ns = [1u8; 8];
		let subject_id = 123u64;
		let cid = b"QmTest";
		let salt = b"salt";
		let version = 1u32;

		let commit = HashHelper::evidence_commitment(&ns, subject_id, cid, salt, version);
		assert_eq!(commit.as_bytes().len(), 32);
	}
}
