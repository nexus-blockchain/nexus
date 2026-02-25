//! # DCAP (Data Center Attestation Primitives) — 链上 TDX Quote v4 验证
//!
//! 实现 Intel TDX Quote v4 的完整结构解析和 ECDSA P-256 签名验证链。
//!
//! ## 验证链路
//!
//! ```text
//! Intel Root CA ──signs──→ PCK Cert ──certifies──→ QE
//!   QE ──signs──→ QE Report ──binds──→ Attestation Key
//!     Attestation Key ──signs──→ (Header || Body)
//! ```
//!
//! ## 验证级别
//!
//! - **Level 1 (Structure)**: 解析 Quote 结构，提取 MRTD/report_data
//! - **Level 2 (BodySig)**: + 验证 ECDSA 签名 (Header+Body) + AK 绑定
//! - **Level 3 (Full)**: + 验证 QE Report 签名 (需要 PCK 公钥)

extern crate alloc;
use alloc::vec::Vec;

// ============================================================================
// TDX Quote v4 结构偏移量
// ============================================================================

/// Header 长度 (48 bytes)
pub const HEADER_LEN: usize = 48;
/// TD Quote Body 长度 (584 bytes)
pub const BODY_LEN: usize = 584;
/// Header + Body 总长度 (632 bytes)
pub const HEADER_PLUS_BODY: usize = HEADER_LEN + BODY_LEN;

/// MRTD 在 Quote 中的偏移 (Header 48 + TEE_TCB_SVN 16 + MRSEAM 48 +
/// MRSIGNERSEAM 48 + SEAMATTR 8 + TDATTR 8 + XFAM 8 = 184)
pub const MRTD_OFFSET: usize = 184;
pub const MRTD_LEN: usize = 48;

/// REPORTDATA 在 Quote 中的偏移 (184 + MRTD 48 + MRCONFIGID 48 + MROWNER 48 +
/// MROWNERCONFIG 48 + RTMR0 48 + RTMR1 48 + RTMR2 48 + RTMR3 48 = 568)
pub const REPORTDATA_OFFSET: usize = 568;
pub const REPORTDATA_LEN: usize = 64;

// Signature Data 偏移 (从 Quote 起始)
/// sig_data_len (u32 LE) at offset 632
pub const SIG_DATA_LEN_OFFSET: usize = HEADER_PLUS_BODY;
/// ECDSA Signature over (Header || Body): 64 bytes at offset 636
pub const BODY_SIG_OFFSET: usize = SIG_DATA_LEN_OFFSET + 4;
pub const BODY_SIG_LEN: usize = 64;
/// Attestation Public Key: 64 bytes (x||y) at offset 700
pub const AK_OFFSET: usize = BODY_SIG_OFFSET + BODY_SIG_LEN;
pub const AK_LEN: usize = 64;
/// QE Report Body: 384 bytes at offset 764
pub const QE_REPORT_OFFSET: usize = AK_OFFSET + AK_LEN;
pub const QE_REPORT_LEN: usize = 384;
/// QE Report Signature: 64 bytes at offset 1148
pub const QE_REPORT_SIG_OFFSET: usize = QE_REPORT_OFFSET + QE_REPORT_LEN;
pub const QE_REPORT_SIG_LEN: usize = 64;
/// QE Auth Data Length: 2 bytes (u16 LE) at offset 1212
pub const QE_AUTH_LEN_OFFSET: usize = QE_REPORT_SIG_OFFSET + QE_REPORT_SIG_LEN;

/// 最小 Quote 长度 (到 QE Auth Data Length 字段)
pub const MIN_DCAP_QUOTE_LEN: usize = QE_AUTH_LEN_OFFSET + 2; // 1214

// QE Report 内部偏移 (相对于 QE Report 起始)
/// QE MRENCLAVE: 32 bytes at QE Report offset 64
pub const QE_MRENCLAVE_OFFSET: usize = 64;
/// QE MRSIGNER: 32 bytes at QE Report offset 128
pub const QE_MRSIGNER_OFFSET: usize = 128;
/// QE REPORTDATA: 64 bytes at QE Report offset 320
pub const QE_REPORTDATA_OFFSET: usize = 320;

// Header 字段偏移
pub const HEADER_VERSION_OFFSET: usize = 0;
pub const HEADER_ATT_KEY_TYPE_OFFSET: usize = 2;
pub const HEADER_TEE_TYPE_OFFSET: usize = 4;
pub const HEADER_VENDOR_ID_OFFSET: usize = 12;

/// Intel QE Vendor ID (标准值)
pub const INTEL_QE_VENDOR_ID: [u8; 16] = [
	0x93, 0x9A, 0x72, 0x33, 0xF7, 0x9C, 0x4C, 0xA9,
	0x94, 0x0A, 0x0D, 0xB3, 0x95, 0x7F, 0x06, 0x07,
];

/// TDX TEE Type
pub const TEE_TYPE_TDX: u32 = 0x00000081;
/// ECDSA-256-with-P-256 attestation key type
pub const ATT_KEY_TYPE_ECDSA_P256: u16 = 2;
/// Quote Version 4
pub const QUOTE_VERSION_4: u16 = 4;

// ============================================================================
// SGX Quote v3 结构偏移量
// ============================================================================

/// SGX Header 长度 (48 bytes, 与 TDX v4 相同)
pub const SGX_HEADER_LEN: usize = 48;
/// SGX ISV Enclave Report Body 长度 (384 bytes)
pub const SGX_BODY_LEN: usize = 384;
/// SGX Header + Body 总长度 (432 bytes)
pub const SGX_HEADER_PLUS_BODY: usize = SGX_HEADER_LEN + SGX_BODY_LEN;

/// MRENCLAVE 在 SGX Quote 中的偏移 (Header 48 + cpu_svn 16 + misc_select 4 +
/// reserved1 12 + isv_ext_prod_id 16 + attributes 16 = offset 112 in body → 48+64=112)
pub const SGX_MRENCLAVE_OFFSET: usize = 112;
pub const SGX_MRENCLAVE_LEN: usize = 32;

/// MRSIGNER 在 SGX Quote 中的偏移 (112 + MRENCLAVE 32 + reserved2 32 = 176)
pub const SGX_MRSIGNER_OFFSET: usize = 176;
pub const SGX_MRSIGNER_LEN: usize = 32;

/// SGX REPORTDATA 在 Quote 中的偏移 (Header 48 + Report Body 内偏移 320 = 368)
pub const SGX_REPORTDATA_OFFSET: usize = 368;
pub const SGX_REPORTDATA_LEN: usize = 64;

// SGX Signature Data 偏移 (从 Quote 起始, 基于 Header+Body=432)
/// sig_data_len (u32 LE) at offset 432
pub const SGX_SIG_DATA_LEN_OFFSET: usize = SGX_HEADER_PLUS_BODY;
/// ECDSA Signature over (Header || Body): 64 bytes at offset 436
pub const SGX_BODY_SIG_OFFSET: usize = SGX_SIG_DATA_LEN_OFFSET + 4;
/// Attestation Public Key: 64 bytes at offset 500
pub const SGX_AK_OFFSET: usize = SGX_BODY_SIG_OFFSET + BODY_SIG_LEN;
/// QE Report Body: 384 bytes at offset 564
pub const SGX_QE_REPORT_OFFSET: usize = SGX_AK_OFFSET + AK_LEN;
/// QE Report Signature: 64 bytes at offset 948
pub const SGX_QE_REPORT_SIG_OFFSET: usize = SGX_QE_REPORT_OFFSET + QE_REPORT_LEN;
/// QE Auth Data Length: 2 bytes (u16 LE) at offset 1012
pub const SGX_QE_AUTH_LEN_OFFSET: usize = SGX_QE_REPORT_SIG_OFFSET + QE_REPORT_SIG_LEN;

/// SGX 最小 Quote 长度 (到 QE Auth Data Length 字段)
pub const SGX_MIN_QUOTE_LEN: usize = SGX_QE_AUTH_LEN_OFFSET + 2; // 1014

/// SGX TEE Type (0x00000000)
pub const TEE_TYPE_SGX: u32 = 0x00000000;
/// SGX Quote Version 3
pub const QUOTE_VERSION_3: u16 = 3;

// ============================================================================
// Intel Root CA Trust Anchor
// ============================================================================

/// Intel SGX Root CA ECDSA P-256 公钥 (uncompressed, x || y, 64 bytes)
///
/// 来源: https://certificates.trustedservices.intel.com/IntelSGXRootCA.der
/// CN = Intel SGX Root CA, O = Intel Corporation, L = Santa Clara, ST = CA, C = US
/// Subject Key Identifier: 22:65:0C:D6:5A:9D:34:89:F3:83:B4:95:52:BF:50:1B:39:27:06:AC
///
/// 这是 DCAP 证明链的信任锚点。所有 Intel SGX/TDX 证明最终都追溯到此公钥。
pub const INTEL_ROOT_CA_PUBKEY: [u8; 64] = [
	// x (32 bytes)
	0x4f, 0xfa, 0x0f, 0xfd, 0x56, 0x1c, 0xda, 0xd6,
	0xc0, 0xf9, 0x8d, 0x30, 0x8c, 0x81, 0x28, 0xc5,
	0xb9, 0x27, 0xa2, 0x73, 0x32, 0xc8, 0xe8, 0xeb,
	0x13, 0xf6, 0xbe, 0x42, 0xb5, 0x71, 0xd6, 0x46,
	// y (32 bytes)
	0x6f, 0x53, 0xc6, 0x44, 0xff, 0xc2, 0xff, 0xc1,
	0x02, 0x82, 0x20, 0xe4, 0x9a, 0x49, 0x66, 0xcf,
	0x02, 0xf3, 0x2e, 0x2f, 0xb4, 0xd3, 0x49, 0xbb,
	0x2c, 0xba, 0xed, 0x28, 0x90, 0x37, 0xa0, 0x2d,
];

/// DER-encoded OID for P-256 curve (prime256v1): 1.2.840.10045.3.1.7
pub const OID_PRIME256V1: [u8; 10] = [
	0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07,
];

// ============================================================================
// Error Types
// ============================================================================

/// DCAP 验证错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DcapError {
	/// Quote 字节数不足
	QuoteTooShort,
	/// Quote 版本不是 4
	InvalidVersion,
	/// Attestation Key 类型不是 ECDSA P-256
	InvalidAttKeyType,
	/// TEE 类型不是 TDX (0x81)
	InvalidTeeType,
	/// QE Vendor ID 不是 Intel
	InvalidVendorId,
	/// Signature Data 长度字段与实际不匹配
	InvalidSigDataLen,
	/// ECDSA 公钥格式无效
	InvalidPublicKey,
	/// ECDSA 签名格式无效
	InvalidSignature,
	/// Body 签名验证失败 (Quote 被篡改或伪造)
	BodySignatureInvalid,
	/// Attestation Key 未绑定到 QE Report
	AttestationKeyBindingFailed,
	/// QE Report 签名验证失败
	QeReportSignatureInvalid,
	/// X.509 证书 DER 解析失败
	CertParsingFailed,
	/// 证书签名验证失败
	CertSignatureInvalid,
	/// 证书链验证失败
	CertChainInvalid,
	/// Root CA 签名 Intermediate CA 证书验证失败
	RootCaVerificationFailed,
	/// Intermediate CA 签名 PCK 证书验证失败
	IntermediateCaVerificationFailed,
}

// ============================================================================
// Parsed Quote
// ============================================================================

/// 解析后的 TDX Quote v4 结构
#[derive(Debug)]
pub struct ParsedQuote<'a> {
	/// 原始 Quote 字节
	pub raw: &'a [u8],
	// ── Header ──
	pub version: u16,
	pub att_key_type: u16,
	pub tee_type: u32,
	pub qe_vendor_id: [u8; 16],
	// ── Body ──
	pub mrtd: [u8; 48],
	pub report_data: [u8; 64],
	// ── Signature Data ──
	/// ECDSA P-256 签名 over (Header || Body), 64 bytes (r || s)
	pub body_signature: &'a [u8],
	/// ECDSA P-256 Attestation Public Key, 64 bytes (x || y)
	pub attestation_key: &'a [u8],
	/// QE Report Body, 384 bytes
	pub qe_report: &'a [u8],
	/// QE Report ECDSA P-256 签名, 64 bytes
	pub qe_report_signature: &'a [u8],
	/// QE Auth Data (variable length)
	pub qe_auth_data: &'a [u8],
	// ── QE Report 内部字段 ──
	/// QE MRENCLAVE (32 bytes)
	pub qe_mrenclave: [u8; 32],
	/// QE MRSIGNER (32 bytes)
	pub qe_mrsigner: [u8; 32],
	/// QE Report Data (64 bytes)
	pub qe_report_data: [u8; 64],
}

// ============================================================================
// Verification Result
// ============================================================================

/// DCAP 验证结果
#[derive(Debug, Clone)]
pub struct DcapVerifyResult {
	/// TDX Trust Domain 度量值 (48 bytes)
	pub mrtd: [u8; 48],
	/// report_data (64 bytes)
	pub report_data: [u8; 64],
	/// Quote 的 blake2_256 哈希
	pub quote_hash: [u8; 32],
	/// Body ECDSA 签名是否验证通过
	pub body_sig_valid: bool,
	/// Attestation Key 是否绑定到 QE Report
	pub ak_binding_valid: bool,
	/// QE Report 签名是否验证通过
	pub qe_sig_valid: bool,
	/// QE MRENCLAVE (32 bytes)
	pub qe_mrenclave: [u8; 32],
	/// QE MRSIGNER (32 bytes)
	pub qe_mrsigner: [u8; 32],
}

// ============================================================================
// Quote Parser
// ============================================================================

/// 解析 TDX Quote v4 原始字节
///
/// 验证 Header 字段 (version=4, att_key_type=2, tee_type=0x81, vendor_id)
/// 并提取所有关键字段的引用。
pub fn parse_quote(raw: &[u8]) -> Result<ParsedQuote<'_>, DcapError> {
	if raw.len() < MIN_DCAP_QUOTE_LEN {
		return Err(DcapError::QuoteTooShort);
	}

	// ── Header 验证 ──
	let version = u16::from_le_bytes([raw[HEADER_VERSION_OFFSET], raw[HEADER_VERSION_OFFSET + 1]]);
	if version != QUOTE_VERSION_4 {
		return Err(DcapError::InvalidVersion);
	}

	let att_key_type = u16::from_le_bytes([
		raw[HEADER_ATT_KEY_TYPE_OFFSET],
		raw[HEADER_ATT_KEY_TYPE_OFFSET + 1],
	]);
	if att_key_type != ATT_KEY_TYPE_ECDSA_P256 {
		return Err(DcapError::InvalidAttKeyType);
	}

	let tee_type = u32::from_le_bytes([
		raw[HEADER_TEE_TYPE_OFFSET],
		raw[HEADER_TEE_TYPE_OFFSET + 1],
		raw[HEADER_TEE_TYPE_OFFSET + 2],
		raw[HEADER_TEE_TYPE_OFFSET + 3],
	]);
	if tee_type != TEE_TYPE_TDX {
		return Err(DcapError::InvalidTeeType);
	}

	let mut qe_vendor_id = [0u8; 16];
	qe_vendor_id.copy_from_slice(&raw[HEADER_VENDOR_ID_OFFSET..HEADER_VENDOR_ID_OFFSET + 16]);
	if qe_vendor_id != INTEL_QE_VENDOR_ID {
		return Err(DcapError::InvalidVendorId);
	}

	// ── Signature Data Length 验证 ──
	let sig_data_len = u32::from_le_bytes([
		raw[SIG_DATA_LEN_OFFSET],
		raw[SIG_DATA_LEN_OFFSET + 1],
		raw[SIG_DATA_LEN_OFFSET + 2],
		raw[SIG_DATA_LEN_OFFSET + 3],
	]) as usize;
	if raw.len() < HEADER_PLUS_BODY + 4 + sig_data_len {
		return Err(DcapError::InvalidSigDataLen);
	}

	// ── Body 字段提取 ──
	let mut mrtd = [0u8; 48];
	mrtd.copy_from_slice(&raw[MRTD_OFFSET..MRTD_OFFSET + MRTD_LEN]);

	let mut report_data = [0u8; 64];
	report_data.copy_from_slice(&raw[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]);

	// ── QE Auth Data ──
	let qe_auth_len =
		u16::from_le_bytes([raw[QE_AUTH_LEN_OFFSET], raw[QE_AUTH_LEN_OFFSET + 1]]) as usize;
	let qe_auth_end = QE_AUTH_LEN_OFFSET + 2 + qe_auth_len;
	if raw.len() < qe_auth_end {
		return Err(DcapError::QuoteTooShort);
	}

	// ── QE Report 内部字段 ──
	let qe_report = &raw[QE_REPORT_OFFSET..QE_REPORT_OFFSET + QE_REPORT_LEN];

	let mut qe_mrenclave = [0u8; 32];
	qe_mrenclave.copy_from_slice(&qe_report[QE_MRENCLAVE_OFFSET..QE_MRENCLAVE_OFFSET + 32]);

	let mut qe_mrsigner = [0u8; 32];
	qe_mrsigner.copy_from_slice(&qe_report[QE_MRSIGNER_OFFSET..QE_MRSIGNER_OFFSET + 32]);

	let mut qe_report_data = [0u8; 64];
	qe_report_data.copy_from_slice(&qe_report[QE_REPORTDATA_OFFSET..QE_REPORTDATA_OFFSET + 64]);

	Ok(ParsedQuote {
		raw,
		version,
		att_key_type,
		tee_type,
		qe_vendor_id,
		mrtd,
		report_data,
		body_signature: &raw[BODY_SIG_OFFSET..BODY_SIG_OFFSET + BODY_SIG_LEN],
		attestation_key: &raw[AK_OFFSET..AK_OFFSET + AK_LEN],
		qe_report,
		qe_report_signature: &raw[QE_REPORT_SIG_OFFSET..QE_REPORT_SIG_OFFSET + QE_REPORT_SIG_LEN],
		qe_auth_data: &raw[QE_AUTH_LEN_OFFSET + 2..qe_auth_end],
		qe_mrenclave,
		qe_mrsigner,
		qe_report_data,
	})
}

// ============================================================================
// ECDSA P-256 Signature Verification
// ============================================================================

/// 验证 ECDSA P-256 签名
///
/// - `public_key`: 64 bytes, uncompressed (x || y), 不含 0x04 前缀
/// - `message`: 原始消息字节 (内部使用 SHA-256 哈希)
/// - `signature`: 64 bytes (r || s), 各 32 bytes big-endian
#[cfg(any(feature = "dcap-verify", test))]
pub fn verify_p256_ecdsa(
	public_key: &[u8],
	message: &[u8],
	signature: &[u8],
) -> Result<(), DcapError> {
	use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
	use p256::EncodedPoint;

	if public_key.len() != 64 {
		return Err(DcapError::InvalidPublicKey);
	}
	if signature.len() != 64 {
		return Err(DcapError::InvalidSignature);
	}

	// 构建 SEC1 uncompressed point: 0x04 || x[32] || y[32]
	let mut sec1 = [0u8; 65];
	sec1[0] = 0x04;
	sec1[1..65].copy_from_slice(public_key);

	let point =
		EncodedPoint::from_bytes(&sec1[..]).map_err(|_| DcapError::InvalidPublicKey)?;
	let vk = VerifyingKey::from_encoded_point(&point)
		.map_err(|_| DcapError::InvalidPublicKey)?;

	let sig = Signature::from_slice(signature).map_err(|_| DcapError::InvalidSignature)?;

	vk.verify(message, &sig)
		.map_err(|_| DcapError::BodySignatureInvalid)
}

/// ECDSA P-256 不可用时的存根 (编译时检查)
#[cfg(not(any(feature = "dcap-verify", test)))]
pub fn verify_p256_ecdsa(
	_public_key: &[u8],
	_message: &[u8],
	_signature: &[u8],
) -> Result<(), DcapError> {
	Err(DcapError::BodySignatureInvalid)
}

// ============================================================================
// DCAP Verification (完整验证链)
// ============================================================================

/// Level 2 验证: Body 签名 + AK 绑定
///
/// 1. 解析 Quote 结构 (Header 字段验证)
/// 2. ECDSA P-256 验证: AK 签名 (Header || Body)
/// 3. AK 绑定验证: SHA-256(AK || auth_data) == QE Report.report_data[0..32]
///
/// 此级别已能防止手工构造假 Quote 攻击 (因为需要 AK 私钥)。
/// 但尚未验证 AK 来源是否为 Intel 认证的 QE。
pub fn verify_quote_level2(raw: &[u8]) -> Result<DcapVerifyResult, DcapError> {
	let quote = parse_quote(raw)?;
	let quote_hash = sp_core::hashing::blake2_256(raw);

	// ── Step 1: 验证 Body 签名 ──
	let header_body = &raw[0..HEADER_PLUS_BODY];
	verify_p256_ecdsa(quote.attestation_key, header_body, quote.body_signature)?;

	// ── Step 2: 验证 AK 绑定到 QE Report ──
	// QE Report.report_data[0..32] = SHA-256(AK || qe_auth_data)
	let mut ak_auth_preimage =
		Vec::with_capacity(quote.attestation_key.len() + quote.qe_auth_data.len());
	ak_auth_preimage.extend_from_slice(quote.attestation_key);
	ak_auth_preimage.extend_from_slice(quote.qe_auth_data);
	let ak_hash = sp_core::hashing::sha2_256(&ak_auth_preimage);

	if quote.qe_report_data[..32] != ak_hash[..] {
		return Err(DcapError::AttestationKeyBindingFailed);
	}

	Ok(DcapVerifyResult {
		mrtd: quote.mrtd,
		report_data: quote.report_data,
		quote_hash,
		body_sig_valid: true,
		ak_binding_valid: true,
		qe_sig_valid: false,
		qe_mrenclave: quote.qe_mrenclave,
		qe_mrsigner: quote.qe_mrsigner,
	})
}

/// Level 3 验证: Body 签名 + AK 绑定 + QE Report 签名
///
/// 在 Level 2 的基础上，额外验证:
/// 4. ECDSA P-256 验证: PCK 签名 QE Report Body
///
/// `pck_public_key` 是链上注册的 PCK 公钥 (64 bytes, x || y)。
/// PCK (Provisioning Certification Key) 由 Intel 证书链认证，
/// 其公钥通过治理提案注册到链上。
pub fn verify_quote_level3(
	raw: &[u8],
	pck_public_key: &[u8; 64],
) -> Result<DcapVerifyResult, DcapError> {
	let mut result = verify_quote_level2(raw)?;

	let quote = parse_quote(raw)?;

	// ── Step 3: 验证 QE Report 签名 ──
	verify_p256_ecdsa(pck_public_key, quote.qe_report, quote.qe_report_signature)
		.map_err(|_| DcapError::QeReportSignatureInvalid)?;

	result.qe_sig_valid = true;
	Ok(result)
}

// ============================================================================
// Minimal DER/ASN.1 Parser
// ============================================================================

/// 读取 DER 元素: 返回 (tag, content_start_offset, content_length)
///
/// content_start_offset 是相对于 data 起始的偏移 (包含 tag + length 字节)
fn der_read_element(data: &[u8]) -> Result<(u8, usize, usize), DcapError> {
	if data.is_empty() {
		return Err(DcapError::CertParsingFailed);
	}
	let tag = data[0];
	if data.len() < 2 {
		return Err(DcapError::CertParsingFailed);
	}
	let (len, len_bytes) = if data[1] < 0x80 {
		(data[1] as usize, 1)
	} else if data[1] == 0x81 {
		if data.len() < 3 {
			return Err(DcapError::CertParsingFailed);
		}
		(data[2] as usize, 2)
	} else if data[1] == 0x82 {
		if data.len() < 4 {
			return Err(DcapError::CertParsingFailed);
		}
		(((data[2] as usize) << 8) | (data[3] as usize), 3)
	} else {
		return Err(DcapError::CertParsingFailed);
	};
	Ok((tag, 1 + len_bytes, len))
}

/// 从 DER 证书中提取 TBS (to-be-signed) 原始字节
///
/// TBS 是 Certificate SEQUENCE 内的第一个 SEQUENCE 元素 (含 tag + length)。
/// 签名验证时需要对 TBS 原始字节进行哈希。
pub fn extract_tbs_from_cert(cert: &[u8]) -> Result<&[u8], DcapError> {
	// 外层 SEQUENCE (Certificate)
	let (tag, content_start, _) = der_read_element(cert)?;
	if tag != 0x30 {
		return Err(DcapError::CertParsingFailed);
	}

	// 第一个内部元素: TBS SEQUENCE
	let tbs_start = content_start;
	if tbs_start >= cert.len() { return Err(DcapError::CertParsingFailed); }
	let (tbs_tag, tbs_hdr, tbs_len) = der_read_element(&cert[tbs_start..])?;
	if tbs_tag != 0x30 {
		return Err(DcapError::CertParsingFailed);
	}

	let tbs_total = tbs_hdr.checked_add(tbs_len).ok_or(DcapError::CertParsingFailed)?;
	let tbs_end = tbs_start.checked_add(tbs_total).ok_or(DcapError::CertParsingFailed)?;
	if cert.len() < tbs_end {
		return Err(DcapError::CertParsingFailed);
	}
	Ok(&cert[tbs_start..tbs_end])
}

/// 从 DER 证书中提取 ECDSA P-256 公钥 (64 bytes, x || y)
///
/// 通过搜索 P-256 OID 定位 SubjectPublicKeyInfo，然后读取 BIT STRING 中的公钥。
pub fn extract_p256_pubkey_from_cert(cert: &[u8]) -> Result<[u8; 64], DcapError> {
	// 搜索 P-256 OID: 06 08 2a 86 48 ce 3d 03 01 07
	let oid_pos = cert
		.windows(OID_PRIME256V1.len())
		.position(|w| w == OID_PRIME256V1)
		.ok_or(DcapError::CertParsingFailed)?;

	// OID 之后找到 BIT STRING (tag 0x03)
	let search_start = oid_pos + OID_PRIME256V1.len();
	let remaining = &cert[search_start..];

	let bit_string_offset = remaining
		.iter()
		.position(|&b| b == 0x03)
		.ok_or(DcapError::CertParsingFailed)?;

	let bs_abs = search_start + bit_string_offset;
	let (bs_tag, bs_hdr, _bs_len) = der_read_element(&cert[bs_abs..])?;
	if bs_tag != 0x03 {
		return Err(DcapError::CertParsingFailed);
	}

	// BIT STRING 内容: [padding_bits=0x00] [0x04 uncompressed] [x 32] [y 32]
	let key_start = bs_abs + bs_hdr;
	if cert.len() < key_start + 66 {
		return Err(DcapError::CertParsingFailed);
	}
	if cert[key_start] != 0x00 || cert[key_start + 1] != 0x04 {
		return Err(DcapError::CertParsingFailed);
	}

	let mut pubkey = [0u8; 64];
	pubkey.copy_from_slice(&cert[key_start + 2..key_start + 66]);
	Ok(pubkey)
}

/// 从 DER 证书中提取 ECDSA 签名并转换为 raw (r || s) 格式 (64 bytes)
///
/// 导航: outer SEQUENCE > skip TBS > skip sigAlg > BIT STRING (签名)
pub fn extract_ecdsa_sig_from_cert(cert: &[u8]) -> Result<[u8; 64], DcapError> {
	let (outer_tag, outer_start, _) = der_read_element(cert)?;
	if outer_tag != 0x30 {
		return Err(DcapError::CertParsingFailed);
	}

	let mut pos = outer_start;

	// Skip TBS SEQUENCE
	if pos >= cert.len() { return Err(DcapError::CertParsingFailed); }
	let (_, tbs_hdr, tbs_len) = der_read_element(&cert[pos..])?;
	pos = pos.checked_add(tbs_hdr).and_then(|p| p.checked_add(tbs_len))
		.ok_or(DcapError::CertParsingFailed)?;

	// Skip signatureAlgorithm SEQUENCE
	if pos >= cert.len() { return Err(DcapError::CertParsingFailed); }
	let (_, alg_hdr, alg_len) = der_read_element(&cert[pos..])?;
	pos = pos.checked_add(alg_hdr).and_then(|p| p.checked_add(alg_len))
		.ok_or(DcapError::CertParsingFailed)?;

	// Read signature BIT STRING
	if pos >= cert.len() { return Err(DcapError::CertParsingFailed); }
	let (sig_tag, sig_hdr, sig_len) = der_read_element(&cert[pos..])?;
	if sig_tag != 0x03 {
		return Err(DcapError::CertParsingFailed);
	}

	let sig_content_start = pos.checked_add(sig_hdr)
		.ok_or(DcapError::CertParsingFailed)?;
	if cert.len() < sig_content_start.checked_add(sig_len)
		.ok_or(DcapError::CertParsingFailed)? {
		return Err(DcapError::CertParsingFailed);
	}

	// BIT STRING 第一个字节是 padding (0x00)
	if cert[sig_content_start] != 0x00 {
		return Err(DcapError::CertParsingFailed);
	}

	// 剩余字节是 DER 编码的 ECDSA 签名: SEQUENCE { INTEGER r, INTEGER s }
	let sig_der = &cert[sig_content_start + 1..sig_content_start + sig_len];
	decode_ecdsa_der_sig(sig_der)
}

/// 将 DER 编码的 ECDSA 签名 (SEQUENCE { INTEGER r, INTEGER s }) 转换为 raw (r || s)
fn decode_ecdsa_der_sig(der: &[u8]) -> Result<[u8; 64], DcapError> {
	let (seq_tag, seq_hdr, _) = der_read_element(der)?;
	if seq_tag != 0x30 {
		return Err(DcapError::CertParsingFailed);
	}

	let mut pos = seq_hdr;

	// INTEGER r
	let (r_tag, r_hdr, r_len) = der_read_element(&der[pos..])?;
	if r_tag != 0x02 {
		return Err(DcapError::CertParsingFailed);
	}
	let r_start = pos + r_hdr;
	let r_bytes = &der[r_start..r_start + r_len];
	pos = r_start + r_len;

	// INTEGER s
	let (s_tag, s_hdr, s_len) = der_read_element(&der[pos..])?;
	if s_tag != 0x02 {
		return Err(DcapError::CertParsingFailed);
	}
	let s_start = pos + s_hdr;
	let s_bytes = &der[s_start..s_start + s_len];

	let mut result = [0u8; 64];
	copy_der_integer_to_fixed(&mut result[0..32], r_bytes);
	copy_der_integer_to_fixed(&mut result[32..64], s_bytes);
	Ok(result)
}

/// 将 DER INTEGER 字节拷贝到固定大小缓冲区 (去除前导零，右对齐)
fn copy_der_integer_to_fixed(dest: &mut [u8], src: &[u8]) {
	// DER INTEGER 可能有一个前导 0x00 (表示正数)
	let src = if src.len() > 32 && src[0] == 0x00 { &src[1..] } else { src };
	let start = dest.len().saturating_sub(src.len());
	let copy_len = src.len().min(dest.len());
	dest[start..start + copy_len].copy_from_slice(&src[..copy_len]);
}

// ============================================================================
// Certificate Chain Verification
// ============================================================================

/// 验证 DER 证书的签名 (使用发行方公钥)
#[cfg(any(feature = "dcap-verify", test))]
pub fn verify_cert_signature(cert_der: &[u8], issuer_pubkey: &[u8; 64]) -> Result<(), DcapError> {
	let tbs = extract_tbs_from_cert(cert_der)?;
	let sig = extract_ecdsa_sig_from_cert(cert_der)?;
	verify_p256_ecdsa(issuer_pubkey, tbs, &sig)
		.map_err(|_| DcapError::CertSignatureInvalid)
}

/// 验证证书链: Intel Root CA → Intermediate CA → PCK
///
/// 1. 使用硬编码的 Intel Root CA 公钥验证 Intermediate CA 证书签名
/// 2. 提取 Intermediate CA 公钥
/// 3. 使用 Intermediate CA 公钥验证 PCK 证书签名
/// 4. 提取并返回经过验证的 PCK 公钥
///
/// 返回值: 经过证书链验证的 PCK 公钥 (64 bytes, x || y)
#[cfg(any(feature = "dcap-verify", test))]
pub fn verify_cert_chain(
	pck_cert_der: &[u8],
	intermediate_cert_der: &[u8],
) -> Result<[u8; 64], DcapError> {
	// Step 1: Root CA (hardcoded) → signs → Intermediate CA cert
	verify_cert_signature(intermediate_cert_der, &INTEL_ROOT_CA_PUBKEY)
		.map_err(|_| DcapError::RootCaVerificationFailed)?;

	// Step 2: 提取 Intermediate CA 公钥
	let intermediate_pubkey = extract_p256_pubkey_from_cert(intermediate_cert_der)?;

	// Step 3: Intermediate CA → signs → PCK cert
	verify_cert_signature(pck_cert_der, &intermediate_pubkey)
		.map_err(|_| DcapError::IntermediateCaVerificationFailed)?;

	// Step 4: 提取经过验证的 PCK 公钥
	extract_p256_pubkey_from_cert(pck_cert_der)
}

#[cfg(not(any(feature = "dcap-verify", test)))]
pub fn verify_cert_chain(
	_pck_cert_der: &[u8],
	_intermediate_cert_der: &[u8],
) -> Result<[u8; 64], DcapError> {
	Err(DcapError::CertChainInvalid)
}

// ============================================================================
// Level 4 Verification (Full Certificate Chain)
// ============================================================================

/// Level 4 验证: Body 签名 + AK 绑定 + QE Report 签名 + 证书链验证
///
/// 与 Level 3 的区别:
/// - Level 3: 信任治理注册的 PCK 公钥
/// - Level 4: 通过 Intel Root CA 证书链验证 PCK 公钥的合法性
///
/// ```text
/// Intel Root CA (硬编码) ──signs──→ Intermediate CA ──signs──→ PCK Cert
///   PCK ──signs──→ QE Report ──binds──→ AK ──signs──→ (Header || Body)
/// ```
#[cfg(any(feature = "dcap-verify", test))]
pub fn verify_quote_with_cert_chain(
	raw: &[u8],
	pck_cert_der: &[u8],
	intermediate_cert_der: &[u8],
) -> Result<DcapVerifyResult, DcapError> {
	// 验证证书链，提取经过验证的 PCK 公钥
	let pck_pubkey = verify_cert_chain(pck_cert_der, intermediate_cert_der)?;

	// 使用经过验证的 PCK 公钥进行 Level 3 验证
	let mut result = verify_quote_level3(raw, &pck_pubkey)?;
	// Level 4 表示证书链也经过验证
	result.qe_sig_valid = true;
	Ok(result)
}

#[cfg(not(any(feature = "dcap-verify", test)))]
pub fn verify_quote_with_cert_chain(
	_raw: &[u8],
	_pck_cert_der: &[u8],
	_intermediate_cert_der: &[u8],
) -> Result<DcapVerifyResult, DcapError> {
	Err(DcapError::CertChainInvalid)
}

// ============================================================================
// SGX Quote v3 解析与验证
// ============================================================================

/// 解析后的 SGX Quote v3 结构
#[derive(Debug)]
pub struct ParsedSgxQuote<'a> {
	/// 原始 Quote 字节
	pub raw: &'a [u8],
	// ── Header ──
	pub version: u16,
	pub att_key_type: u16,
	pub tee_type: u32,
	pub qe_vendor_id: [u8; 16],
	// ── ISV Enclave Report Body ──
	pub mrenclave: [u8; 32],
	pub mrsigner: [u8; 32],
	pub report_data: [u8; 64],
	// ── Signature Data ──
	pub body_signature: &'a [u8],
	pub attestation_key: &'a [u8],
	pub qe_report: &'a [u8],
	pub qe_report_signature: &'a [u8],
	pub qe_auth_data: &'a [u8],
	// ── QE Report 内部字段 ──
	pub qe_mrenclave: [u8; 32],
	pub qe_mrsigner: [u8; 32],
	pub qe_report_data: [u8; 64],
}

/// SGX DCAP 验证结果
#[derive(Debug, Clone)]
pub struct SgxVerifyResult {
	/// SGX Enclave 度量值 (32 bytes)
	pub mrenclave: [u8; 32],
	/// SGX Enclave 签名者 (32 bytes)
	pub mrsigner: [u8; 32],
	/// report_data (64 bytes)
	pub report_data: [u8; 64],
	/// Quote 的 blake2_256 哈希
	pub quote_hash: [u8; 32],
	/// Body ECDSA 签名是否验证通过
	pub body_sig_valid: bool,
	/// Attestation Key 是否绑定到 QE Report
	pub ak_binding_valid: bool,
	/// QE Report 签名是否验证通过
	pub qe_sig_valid: bool,
	/// QE MRENCLAVE (32 bytes)
	pub qe_mrenclave: [u8; 32],
	/// QE MRSIGNER (32 bytes)
	pub qe_mrsigner: [u8; 32],
}

/// 解析 SGX Quote v3 原始字节
///
/// 验证 Header 字段 (version=3, att_key_type=2, tee_type=0x00)
/// 并提取所有关键字段。
pub fn parse_sgx_quote(raw: &[u8]) -> Result<ParsedSgxQuote<'_>, DcapError> {
	if raw.len() < SGX_MIN_QUOTE_LEN {
		return Err(DcapError::QuoteTooShort);
	}

	// ── Header 验证 ──
	let version = u16::from_le_bytes([raw[HEADER_VERSION_OFFSET], raw[HEADER_VERSION_OFFSET + 1]]);
	if version != QUOTE_VERSION_3 {
		return Err(DcapError::InvalidVersion);
	}

	let att_key_type = u16::from_le_bytes([
		raw[HEADER_ATT_KEY_TYPE_OFFSET],
		raw[HEADER_ATT_KEY_TYPE_OFFSET + 1],
	]);
	if att_key_type != ATT_KEY_TYPE_ECDSA_P256 {
		return Err(DcapError::InvalidAttKeyType);
	}

	let tee_type = u32::from_le_bytes([
		raw[HEADER_TEE_TYPE_OFFSET],
		raw[HEADER_TEE_TYPE_OFFSET + 1],
		raw[HEADER_TEE_TYPE_OFFSET + 2],
		raw[HEADER_TEE_TYPE_OFFSET + 3],
	]);
	if tee_type != TEE_TYPE_SGX {
		return Err(DcapError::InvalidTeeType);
	}

	let mut qe_vendor_id = [0u8; 16];
	qe_vendor_id.copy_from_slice(&raw[HEADER_VENDOR_ID_OFFSET..HEADER_VENDOR_ID_OFFSET + 16]);
	if qe_vendor_id != INTEL_QE_VENDOR_ID {
		return Err(DcapError::InvalidVendorId);
	}

	// ── Signature Data Length 验证 ──
	let sig_data_len = u32::from_le_bytes([
		raw[SGX_SIG_DATA_LEN_OFFSET],
		raw[SGX_SIG_DATA_LEN_OFFSET + 1],
		raw[SGX_SIG_DATA_LEN_OFFSET + 2],
		raw[SGX_SIG_DATA_LEN_OFFSET + 3],
	]) as usize;
	if raw.len() < SGX_HEADER_PLUS_BODY + 4 + sig_data_len {
		return Err(DcapError::InvalidSigDataLen);
	}

	// ── Body 字段提取 ──
	let mut mrenclave = [0u8; 32];
	mrenclave.copy_from_slice(&raw[SGX_MRENCLAVE_OFFSET..SGX_MRENCLAVE_OFFSET + SGX_MRENCLAVE_LEN]);

	let mut mrsigner = [0u8; 32];
	mrsigner.copy_from_slice(&raw[SGX_MRSIGNER_OFFSET..SGX_MRSIGNER_OFFSET + SGX_MRSIGNER_LEN]);

	let mut report_data = [0u8; 64];
	report_data.copy_from_slice(&raw[SGX_REPORTDATA_OFFSET..SGX_REPORTDATA_OFFSET + SGX_REPORTDATA_LEN]);

	// ── QE Auth Data ──
	let qe_auth_len =
		u16::from_le_bytes([raw[SGX_QE_AUTH_LEN_OFFSET], raw[SGX_QE_AUTH_LEN_OFFSET + 1]]) as usize;
	let qe_auth_end = SGX_QE_AUTH_LEN_OFFSET + 2 + qe_auth_len;
	if raw.len() < qe_auth_end {
		return Err(DcapError::QuoteTooShort);
	}

	// ── QE Report 内部字段 ──
	let qe_report = &raw[SGX_QE_REPORT_OFFSET..SGX_QE_REPORT_OFFSET + QE_REPORT_LEN];

	let mut qe_mrenclave = [0u8; 32];
	qe_mrenclave.copy_from_slice(&qe_report[QE_MRENCLAVE_OFFSET..QE_MRENCLAVE_OFFSET + 32]);

	let mut qe_mrsigner = [0u8; 32];
	qe_mrsigner.copy_from_slice(&qe_report[QE_MRSIGNER_OFFSET..QE_MRSIGNER_OFFSET + 32]);

	let mut qe_report_data = [0u8; 64];
	qe_report_data.copy_from_slice(&qe_report[QE_REPORTDATA_OFFSET..QE_REPORTDATA_OFFSET + 64]);

	Ok(ParsedSgxQuote {
		raw,
		version,
		att_key_type,
		tee_type,
		qe_vendor_id,
		mrenclave,
		mrsigner,
		report_data,
		body_signature: &raw[SGX_BODY_SIG_OFFSET..SGX_BODY_SIG_OFFSET + BODY_SIG_LEN],
		attestation_key: &raw[SGX_AK_OFFSET..SGX_AK_OFFSET + AK_LEN],
		qe_report,
		qe_report_signature: &raw[SGX_QE_REPORT_SIG_OFFSET..SGX_QE_REPORT_SIG_OFFSET + QE_REPORT_SIG_LEN],
		qe_auth_data: &raw[SGX_QE_AUTH_LEN_OFFSET + 2..qe_auth_end],
		qe_mrenclave,
		qe_mrsigner,
		qe_report_data,
	})
}

/// SGX Level 2 验证: Body 签名 + AK 绑定
///
/// 与 TDX Level 2 相同的验证逻辑，仅偏移量不同:
/// 1. ECDSA P-256: AK 签名 (Header || Body)
/// 2. AK 绑定: SHA-256(AK || auth_data) == QE Report.report_data[0..32]
pub fn verify_sgx_quote_level2(raw: &[u8]) -> Result<SgxVerifyResult, DcapError> {
	let quote = parse_sgx_quote(raw)?;
	let quote_hash = sp_core::hashing::blake2_256(raw);

	// ── Step 1: 验证 Body 签名 ──
	let header_body = &raw[0..SGX_HEADER_PLUS_BODY];
	verify_p256_ecdsa(quote.attestation_key, header_body, quote.body_signature)?;

	// ── Step 2: 验证 AK 绑定到 QE Report ──
	let mut ak_auth_preimage =
		Vec::with_capacity(quote.attestation_key.len() + quote.qe_auth_data.len());
	ak_auth_preimage.extend_from_slice(quote.attestation_key);
	ak_auth_preimage.extend_from_slice(quote.qe_auth_data);
	let ak_hash = sp_core::hashing::sha2_256(&ak_auth_preimage);

	if quote.qe_report_data[..32] != ak_hash[..] {
		return Err(DcapError::AttestationKeyBindingFailed);
	}

	Ok(SgxVerifyResult {
		mrenclave: quote.mrenclave,
		mrsigner: quote.mrsigner,
		report_data: quote.report_data,
		quote_hash,
		body_sig_valid: true,
		ak_binding_valid: true,
		qe_sig_valid: false,
		qe_mrenclave: quote.qe_mrenclave,
		qe_mrsigner: quote.qe_mrsigner,
	})
}

/// SGX Level 3 验证: Body 签名 + AK 绑定 + QE Report 签名
pub fn verify_sgx_quote_level3(
	raw: &[u8],
	pck_public_key: &[u8; 64],
) -> Result<SgxVerifyResult, DcapError> {
	let mut result = verify_sgx_quote_level2(raw)?;

	let quote = parse_sgx_quote(raw)?;

	// ── Step 3: 验证 QE Report 签名 ──
	verify_p256_ecdsa(pck_public_key, quote.qe_report, quote.qe_report_signature)
		.map_err(|_| DcapError::QeReportSignatureInvalid)?;

	result.qe_sig_valid = true;
	Ok(result)
}

/// SGX Level 4 验证: Body 签名 + AK 绑定 + QE Report 签名 + 证书链验证
#[cfg(any(feature = "dcap-verify", test))]
pub fn verify_sgx_quote_with_cert_chain(
	raw: &[u8],
	pck_cert_der: &[u8],
	intermediate_cert_der: &[u8],
) -> Result<SgxVerifyResult, DcapError> {
	let pck_pubkey = verify_cert_chain(pck_cert_der, intermediate_cert_der)?;
	let mut result = verify_sgx_quote_level3(raw, &pck_pubkey)?;
	result.qe_sig_valid = true;
	Ok(result)
}

#[cfg(not(any(feature = "dcap-verify", test)))]
pub fn verify_sgx_quote_with_cert_chain(
	_raw: &[u8],
	_pck_cert_der: &[u8],
	_intermediate_cert_der: &[u8],
) -> Result<SgxVerifyResult, DcapError> {
	Err(DcapError::CertChainInvalid)
}

// ============================================================================
// 辅助函数: 构建测试用 Quote
// ============================================================================

/// 构建一个用于测试的完整 TDX Quote v4 (含有效 ECDSA 签名)
///
/// 仅在 test 中可用。生成真实的 P-256 密钥对并签名。
#[cfg(test)]
pub mod test_utils {
	use super::*;
	use p256::ecdsa::{signature::Signer, SigningKey, VerifyingKey};

	/// 测试用 Quote 构建器
	pub struct TestQuoteBuilder {
		pub mrtd: [u8; 48],
		pub report_data: [u8; 64],
		pub ak_signing_key: SigningKey,
		pub pck_signing_key: SigningKey,
	}

	impl TestQuoteBuilder {
		/// 使用确定性种子创建
		pub fn new(seed: u8) -> Self {
			let mut ak_seed = [0u8; 32];
			ak_seed[0] = seed;
			ak_seed[1] = 0xAA;
			let ak_signing_key = SigningKey::from_slice(&ak_seed).unwrap();

			let mut pck_seed = [0u8; 32];
			pck_seed[0] = seed;
			pck_seed[1] = 0xBB;
			let pck_signing_key = SigningKey::from_slice(&pck_seed).unwrap();

			Self {
				mrtd: [0u8; 48],
				report_data: [0u8; 64],
				ak_signing_key,
				pck_signing_key,
			}
		}

		pub fn with_mrtd(mut self, mrtd: [u8; 48]) -> Self {
			self.mrtd = mrtd;
			self
		}

		pub fn with_report_data(mut self, rd: [u8; 64]) -> Self {
			self.report_data = rd;
			self
		}

		/// 构建带有效签名的 Quote 字节
		pub fn build(&self) -> Vec<u8> {
			let ak_vk = VerifyingKey::from(&self.ak_signing_key);
			let ak_point = ak_vk.to_encoded_point(false);
			let ak_bytes = &ak_point.as_bytes()[1..65]; // skip 0x04 prefix

			let _pck_vk = VerifyingKey::from(&self.pck_signing_key);

			// QE Auth Data (空)
			let qe_auth_data: Vec<u8> = Vec::new();

			// AK binding: SHA-256(AK || qe_auth_data)
			let mut ak_auth_preimage = Vec::new();
			ak_auth_preimage.extend_from_slice(ak_bytes);
			ak_auth_preimage.extend_from_slice(&qe_auth_data);
			let ak_hash = sp_core::hashing::sha2_256(&ak_auth_preimage);

			// ── 构建 Header (48 bytes) ──
			let mut quote = vec![0u8; 2048];
			// version = 4
			quote[0] = 4;
			quote[1] = 0;
			// att_key_type = 2
			quote[2] = 2;
			quote[3] = 0;
			// tee_type = 0x81 (TDX)
			quote[4] = 0x81;
			quote[5] = 0;
			quote[6] = 0;
			quote[7] = 0;
			// vendor_id
			quote[HEADER_VENDOR_ID_OFFSET..HEADER_VENDOR_ID_OFFSET + 16]
				.copy_from_slice(&INTEL_QE_VENDOR_ID);

			// ── Body (584 bytes at offset 48) ──
			// MRTD at offset 184
			quote[MRTD_OFFSET..MRTD_OFFSET + MRTD_LEN].copy_from_slice(&self.mrtd);
			// REPORTDATA at offset 568
			quote[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]
				.copy_from_slice(&self.report_data);

			// ── Sign Header+Body with AK ──
			let header_body = &quote[0..HEADER_PLUS_BODY];
			let body_sig: p256::ecdsa::Signature = self.ak_signing_key.sign(header_body);
			let body_sig_bytes = body_sig.to_bytes();

			// sig_data_len (placeholder, will set later)
			// Body signature at offset 636
			quote[BODY_SIG_OFFSET..BODY_SIG_OFFSET + BODY_SIG_LEN]
				.copy_from_slice(&body_sig_bytes);
			// AK at offset 700
			quote[AK_OFFSET..AK_OFFSET + AK_LEN].copy_from_slice(ak_bytes);

			// ── QE Report (384 bytes at offset 764) ──
			// QE MRENCLAVE at QE Report offset 64
			let qe_mrenclave = [0x51; 32]; // known test QE MRENCLAVE
			quote[QE_REPORT_OFFSET + QE_MRENCLAVE_OFFSET
				..QE_REPORT_OFFSET + QE_MRENCLAVE_OFFSET + 32]
				.copy_from_slice(&qe_mrenclave);
			// QE MRSIGNER at QE Report offset 128
			let qe_mrsigner = [0x52; 32]; // known test QE MRSIGNER
			quote[QE_REPORT_OFFSET + QE_MRSIGNER_OFFSET
				..QE_REPORT_OFFSET + QE_MRSIGNER_OFFSET + 32]
				.copy_from_slice(&qe_mrsigner);
			// QE Report Data at QE Report offset 320
			// [0..32] = SHA-256(AK || qe_auth_data)
			quote[QE_REPORT_OFFSET + QE_REPORTDATA_OFFSET
				..QE_REPORT_OFFSET + QE_REPORTDATA_OFFSET + 32]
				.copy_from_slice(&ak_hash);

			// ── Sign QE Report with PCK ──
			let qe_report_bytes =
				&quote[QE_REPORT_OFFSET..QE_REPORT_OFFSET + QE_REPORT_LEN];
			let qe_sig: p256::ecdsa::Signature = self.pck_signing_key.sign(qe_report_bytes);
			let qe_sig_bytes = qe_sig.to_bytes();
			quote[QE_REPORT_SIG_OFFSET..QE_REPORT_SIG_OFFSET + QE_REPORT_SIG_LEN]
				.copy_from_slice(&qe_sig_bytes);

			// ── QE Auth Data Length (0) ──
			quote[QE_AUTH_LEN_OFFSET] = 0;
			quote[QE_AUTH_LEN_OFFSET + 1] = 0;

			// ── sig_data_len ──
			let sig_data_total = BODY_SIG_LEN + AK_LEN + QE_REPORT_LEN + QE_REPORT_SIG_LEN + 2;
			let len_bytes = (sig_data_total as u32).to_le_bytes();
			quote[SIG_DATA_LEN_OFFSET..SIG_DATA_LEN_OFFSET + 4].copy_from_slice(&len_bytes);

			// 截断到实际使用长度
			let total_len = QE_AUTH_LEN_OFFSET + 2;
			quote.truncate(total_len);
			quote
		}

		/// 获取 AK 公钥 (64 bytes, x || y)
		pub fn ak_public_key(&self) -> [u8; 64] {
			let vk = VerifyingKey::from(&self.ak_signing_key);
			let point = vk.to_encoded_point(false);
			let mut key = [0u8; 64];
			key.copy_from_slice(&point.as_bytes()[1..65]);
			key
		}

		/// 获取 PCK 公钥 (64 bytes, x || y)
		pub fn pck_public_key(&self) -> [u8; 64] {
			let vk = VerifyingKey::from(&self.pck_signing_key);
			let point = vk.to_encoded_point(false);
			let mut key = [0u8; 64];
			key.copy_from_slice(&point.as_bytes()[1..65]);
			key
		}
	}

	/// SGX Quote v3 测试构建器 (含有效 ECDSA 签名)
	pub struct TestSgxQuoteBuilder {
		pub mrenclave: [u8; 32],
		pub mrsigner: [u8; 32],
		pub report_data: [u8; 64],
		pub ak_signing_key: SigningKey,
		pub pck_signing_key: SigningKey,
	}

	impl TestSgxQuoteBuilder {
		/// 使用确定性种子创建
		pub fn new(seed: u8) -> Self {
			let mut ak_seed = [0u8; 32];
			ak_seed[0] = seed;
			ak_seed[1] = 0xCC; // 区别于 TDX builder 的 0xAA
			let ak_signing_key = SigningKey::from_slice(&ak_seed).unwrap();

			let mut pck_seed = [0u8; 32];
			pck_seed[0] = seed;
			pck_seed[1] = 0xDD; // 区别于 TDX builder 的 0xBB
			let pck_signing_key = SigningKey::from_slice(&pck_seed).unwrap();

			Self {
				mrenclave: [0u8; 32],
				mrsigner: [0u8; 32],
				report_data: [0u8; 64],
				ak_signing_key,
				pck_signing_key,
			}
		}

		pub fn with_mrenclave(mut self, mrenclave: [u8; 32]) -> Self {
			self.mrenclave = mrenclave;
			self
		}

		pub fn with_mrsigner(mut self, mrsigner: [u8; 32]) -> Self {
			self.mrsigner = mrsigner;
			self
		}

		pub fn with_report_data(mut self, rd: [u8; 64]) -> Self {
			self.report_data = rd;
			self
		}

		/// 构建带有效签名的 SGX Quote v3 字节
		pub fn build(&self) -> Vec<u8> {
			let ak_vk = VerifyingKey::from(&self.ak_signing_key);
			let ak_point = ak_vk.to_encoded_point(false);
			let ak_bytes = &ak_point.as_bytes()[1..65];

			let qe_auth_data: Vec<u8> = Vec::new();

			// AK binding: SHA-256(AK || qe_auth_data)
			let mut ak_auth_preimage = Vec::new();
			ak_auth_preimage.extend_from_slice(ak_bytes);
			ak_auth_preimage.extend_from_slice(&qe_auth_data);
			let ak_hash = sp_core::hashing::sha2_256(&ak_auth_preimage);

			// ── 构建 Header (48 bytes) ──
			let mut quote = vec![0u8; 2048];
			// version = 3 (SGX)
			quote[0] = 3;
			quote[1] = 0;
			// att_key_type = 2
			quote[2] = 2;
			quote[3] = 0;
			// tee_type = 0x00000000 (SGX)
			quote[4] = 0;
			quote[5] = 0;
			quote[6] = 0;
			quote[7] = 0;
			// vendor_id
			quote[HEADER_VENDOR_ID_OFFSET..HEADER_VENDOR_ID_OFFSET + 16]
				.copy_from_slice(&INTEL_QE_VENDOR_ID);

			// ── ISV Enclave Report Body (384 bytes at offset 48) ──
			// MRENCLAVE at offset 112
			quote[SGX_MRENCLAVE_OFFSET..SGX_MRENCLAVE_OFFSET + SGX_MRENCLAVE_LEN]
				.copy_from_slice(&self.mrenclave);
			// MRSIGNER at offset 176
			quote[SGX_MRSIGNER_OFFSET..SGX_MRSIGNER_OFFSET + SGX_MRSIGNER_LEN]
				.copy_from_slice(&self.mrsigner);
			// REPORTDATA at offset 368
			quote[SGX_REPORTDATA_OFFSET..SGX_REPORTDATA_OFFSET + SGX_REPORTDATA_LEN]
				.copy_from_slice(&self.report_data);

			// ── Sign Header+Body with AK ──
			let header_body = &quote[0..SGX_HEADER_PLUS_BODY];
			let body_sig: p256::ecdsa::Signature = self.ak_signing_key.sign(header_body);
			let body_sig_bytes = body_sig.to_bytes();

			// Body signature at SGX offset
			quote[SGX_BODY_SIG_OFFSET..SGX_BODY_SIG_OFFSET + BODY_SIG_LEN]
				.copy_from_slice(&body_sig_bytes);
			// AK
			quote[SGX_AK_OFFSET..SGX_AK_OFFSET + AK_LEN].copy_from_slice(ak_bytes);

			// ── QE Report (384 bytes) ──
			let qe_mrenclave = [0x61; 32]; // known test QE MRENCLAVE (SGX)
			quote[SGX_QE_REPORT_OFFSET + QE_MRENCLAVE_OFFSET
				..SGX_QE_REPORT_OFFSET + QE_MRENCLAVE_OFFSET + 32]
				.copy_from_slice(&qe_mrenclave);
			let qe_mrsigner = [0x62; 32]; // known test QE MRSIGNER (SGX)
			quote[SGX_QE_REPORT_OFFSET + QE_MRSIGNER_OFFSET
				..SGX_QE_REPORT_OFFSET + QE_MRSIGNER_OFFSET + 32]
				.copy_from_slice(&qe_mrsigner);
			// QE Report Data: [0..32] = SHA-256(AK || qe_auth_data)
			quote[SGX_QE_REPORT_OFFSET + QE_REPORTDATA_OFFSET
				..SGX_QE_REPORT_OFFSET + QE_REPORTDATA_OFFSET + 32]
				.copy_from_slice(&ak_hash);

			// ── Sign QE Report with PCK ──
			let qe_report_bytes =
				&quote[SGX_QE_REPORT_OFFSET..SGX_QE_REPORT_OFFSET + QE_REPORT_LEN];
			let qe_sig: p256::ecdsa::Signature = self.pck_signing_key.sign(qe_report_bytes);
			let qe_sig_bytes = qe_sig.to_bytes();
			quote[SGX_QE_REPORT_SIG_OFFSET..SGX_QE_REPORT_SIG_OFFSET + QE_REPORT_SIG_LEN]
				.copy_from_slice(&qe_sig_bytes);

			// ── QE Auth Data Length (0) ──
			quote[SGX_QE_AUTH_LEN_OFFSET] = 0;
			quote[SGX_QE_AUTH_LEN_OFFSET + 1] = 0;

			// ── sig_data_len ──
			let sig_data_total = BODY_SIG_LEN + AK_LEN + QE_REPORT_LEN + QE_REPORT_SIG_LEN + 2;
			let len_bytes = (sig_data_total as u32).to_le_bytes();
			quote[SGX_SIG_DATA_LEN_OFFSET..SGX_SIG_DATA_LEN_OFFSET + 4]
				.copy_from_slice(&len_bytes);

			// 截断到实际使用长度
			let total_len = SGX_QE_AUTH_LEN_OFFSET + 2;
			quote.truncate(total_len);
			quote
		}

		/// 获取 PCK 公钥 (64 bytes, x || y)
		pub fn pck_public_key(&self) -> [u8; 64] {
			let vk = VerifyingKey::from(&self.pck_signing_key);
			let point = vk.to_encoded_point(false);
			let mut key = [0u8; 64];
			key.copy_from_slice(&point.as_bytes()[1..65]);
			key
		}
	}

	// ── DER Certificate Builder for Tests ──

	/// DER 长度编码 (写入到 Vec)
	fn der_encode_length_vec(buf: &mut Vec<u8>, len: usize) {
		if len < 0x80 {
			buf.push(len as u8);
		} else if len <= 0xFF {
			buf.push(0x81);
			buf.push(len as u8);
		} else {
			buf.push(0x82);
			buf.push((len >> 8) as u8);
			buf.push(len as u8);
		}
	}

	/// 将内容包装为 DER SEQUENCE
	fn der_wrap_sequence(content: &[u8]) -> Vec<u8> {
		let mut result = Vec::new();
		result.push(0x30); // SEQUENCE tag
		der_encode_length_vec(&mut result, content.len());
		result.extend_from_slice(content);
		result
	}

	/// 构建最小化的 DER X.509 测试证书
	///
	/// 包含有效的 SubjectPublicKeyInfo (P-256) 和 ECDSA-SHA256 签名。
	/// 用于测试 DER 解析和证书链验证。
	pub fn build_test_cert(subject_vk: &VerifyingKey, issuer_sk: &SigningKey) -> Vec<u8> {
		let point = subject_vk.to_encoded_point(false);
		let pubkey_bytes = point.as_bytes(); // 65 bytes: 04 || x || y

		// ── TBS 内容 ──
		let mut tbs_content = Vec::new();

		// Version [0] EXPLICIT INTEGER 2 (v3)
		tbs_content.extend_from_slice(&[0xa0, 0x03, 0x02, 0x01, 0x02]);
		// Serial number INTEGER 1
		tbs_content.extend_from_slice(&[0x02, 0x01, 0x01]);
		// Signature algorithm: ecdsa-with-SHA256 (OID 1.2.840.10045.4.3.2)
		tbs_content.extend_from_slice(&[
			0x30, 0x0a, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02,
		]);
		// Issuer: minimal SEQUENCE
		tbs_content.extend_from_slice(&[0x30, 0x00]);
		// Validity: minimal SEQUENCE
		tbs_content.extend_from_slice(&[0x30, 0x00]);
		// Subject: minimal SEQUENCE
		tbs_content.extend_from_slice(&[0x30, 0x00]);
		// SubjectPublicKeyInfo (89 bytes total)
		tbs_content.extend_from_slice(&[0x30, 0x59]); // SEQUENCE (89 bytes)
		tbs_content.extend_from_slice(&[0x30, 0x13]); // algorithm SEQUENCE (19 bytes)
		// OID id-ecPublicKey: 1.2.840.10045.2.1
		tbs_content.extend_from_slice(&[
			0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
		]);
		// OID prime256v1 (P-256)
		tbs_content.extend_from_slice(&OID_PRIME256V1);
		// BIT STRING: 0 padding + uncompressed point
		tbs_content.extend_from_slice(&[0x03, 0x42, 0x00]);
		tbs_content.extend_from_slice(pubkey_bytes); // 65 bytes

		// 包装 TBS 为 SEQUENCE
		let tbs_seq = der_wrap_sequence(&tbs_content);

		// ── 用 issuer 密钥签名 TBS ──
		let sig: p256::ecdsa::Signature = issuer_sk.sign(&tbs_seq);
		let sig_der = sig.to_der();
		let sig_bytes = sig_der.to_bytes();

		// ── 组装完整证书 ──
		let mut cert_content = Vec::new();
		cert_content.extend_from_slice(&tbs_seq);
		// Signature algorithm (同 TBS 中的)
		cert_content.extend_from_slice(&[
			0x30, 0x0a, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02,
		]);
		// Signature BIT STRING
		cert_content.push(0x03); // BIT STRING tag
		der_encode_length_vec(&mut cert_content, sig_bytes.len() + 1);
		cert_content.push(0x00); // 0 padding bits
		cert_content.extend_from_slice(&sig_bytes);

		// 包装为外层 SEQUENCE
		der_wrap_sequence(&cert_content)
	}

	/// 使用确定性种子创建测试用 SigningKey
	pub fn test_signing_key(seed: u8, salt: u8) -> SigningKey {
		let mut key_seed = [0u8; 32];
		key_seed[0] = seed;
		key_seed[1] = salt;
		SigningKey::from_slice(&key_seed).unwrap()
	}
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
	use super::*;
	use test_utils::TestQuoteBuilder;

	#[test]
	fn parse_valid_quote() {
		let builder = TestQuoteBuilder::new(1).with_mrtd([0xAA; 48]);
		let quote_bytes = builder.build();
		let parsed = parse_quote(&quote_bytes).unwrap();
		assert_eq!(parsed.version, 4);
		assert_eq!(parsed.att_key_type, 2);
		assert_eq!(parsed.tee_type, TEE_TYPE_TDX);
		assert_eq!(parsed.qe_vendor_id, INTEL_QE_VENDOR_ID);
		assert_eq!(parsed.mrtd, [0xAA; 48]);
	}

	#[test]
	fn parse_rejects_short_quote() {
		assert_eq!(parse_quote(&[0u8; 100]).unwrap_err(), DcapError::QuoteTooShort);
	}

	#[test]
	fn parse_rejects_wrong_version() {
		let builder = TestQuoteBuilder::new(1);
		let mut quote = builder.build();
		quote[0] = 3; // version 3 instead of 4
		assert_eq!(parse_quote(&quote).unwrap_err(), DcapError::InvalidVersion);
	}

	#[test]
	fn parse_rejects_wrong_tee_type() {
		let builder = TestQuoteBuilder::new(1);
		let mut quote = builder.build();
		quote[4] = 0x00; // SGX instead of TDX
		assert_eq!(parse_quote(&quote).unwrap_err(), DcapError::InvalidTeeType);
	}

	#[test]
	fn parse_rejects_wrong_vendor_id() {
		let builder = TestQuoteBuilder::new(1);
		let mut quote = builder.build();
		quote[HEADER_VENDOR_ID_OFFSET] = 0xFF;
		assert_eq!(parse_quote(&quote).unwrap_err(), DcapError::InvalidVendorId);
	}

	#[test]
	fn verify_level2_valid_quote() {
		let mut rd = [0u8; 64];
		rd[0] = 0x42;
		let builder = TestQuoteBuilder::new(1)
			.with_mrtd([0xBB; 48])
			.with_report_data(rd);
		let quote = builder.build();
		let result = verify_quote_level2(&quote).unwrap();
		assert_eq!(result.mrtd, [0xBB; 48]);
		assert_eq!(result.report_data, rd);
		assert!(result.body_sig_valid);
		assert!(result.ak_binding_valid);
		assert!(!result.qe_sig_valid);
	}

	#[test]
	fn verify_level2_rejects_tampered_body() {
		let builder = TestQuoteBuilder::new(1).with_mrtd([0xCC; 48]);
		let mut quote = builder.build();
		// Tamper with MRTD after signing
		quote[MRTD_OFFSET] = 0xFF;
		assert_eq!(
			verify_quote_level2(&quote).unwrap_err(),
			DcapError::BodySignatureInvalid
		);
	}

	#[test]
	fn verify_level2_rejects_tampered_report_data() {
		let builder = TestQuoteBuilder::new(1);
		let mut quote = builder.build();
		// Tamper with report_data after signing
		quote[REPORTDATA_OFFSET] = 0xFF;
		assert_eq!(
			verify_quote_level2(&quote).unwrap_err(),
			DcapError::BodySignatureInvalid
		);
	}

	#[test]
	fn verify_level2_rejects_wrong_ak_binding() {
		let builder = TestQuoteBuilder::new(1);
		let mut quote = builder.build();
		// Tamper with QE Report's report_data (AK binding hash)
		quote[QE_REPORT_OFFSET + QE_REPORTDATA_OFFSET] = 0xFF;
		// Body signature is still valid, but AK binding fails
		// Actually, this will also invalidate the QE Report, but body sig
		// is over header+body only, so body sig should still pass.
		// The AK binding check should fail.
		let err = verify_quote_level2(&quote).unwrap_err();
		assert_eq!(err, DcapError::AttestationKeyBindingFailed);
	}

	#[test]
	fn verify_level3_valid_quote() {
		let builder = TestQuoteBuilder::new(2).with_mrtd([0xDD; 48]);
		let quote = builder.build();
		let pck_key = builder.pck_public_key();
		let result = verify_quote_level3(&quote, &pck_key).unwrap();
		assert!(result.body_sig_valid);
		assert!(result.ak_binding_valid);
		assert!(result.qe_sig_valid);
		assert_eq!(result.qe_mrenclave, [0x51; 32]);
		assert_eq!(result.qe_mrsigner, [0x52; 32]);
	}

	#[test]
	fn verify_level3_rejects_wrong_pck_key() {
		let builder = TestQuoteBuilder::new(3);
		let quote = builder.build();
		let wrong_pck = [0x99; 64]; // invalid key
		assert!(verify_quote_level3(&quote, &wrong_pck).is_err());
	}

	#[test]
	fn verify_level3_rejects_tampered_qe_report() {
		let builder = TestQuoteBuilder::new(4);
		let mut quote = builder.build();
		let pck_key = builder.pck_public_key();
		// Tamper with QE MRSIGNER
		quote[QE_REPORT_OFFSET + QE_MRSIGNER_OFFSET] = 0xFF;
		// This breaks both: AK binding (report_data changed location? no, MRSIGNER is different offset)
		// Actually MRSIGNER is separate from report_data, so AK binding should still pass
		// but QE Report signature should fail
		let err = verify_quote_level3(&quote, &pck_key).unwrap_err();
		assert_eq!(err, DcapError::QeReportSignatureInvalid);
	}

	#[test]
	fn ecdsa_p256_basic_sign_verify() {
		use p256::ecdsa::{signature::Signer, SigningKey, VerifyingKey};

		let sk = SigningKey::from_slice(&[0x01; 32]).unwrap();
		let vk = VerifyingKey::from(&sk);
		let point = vk.to_encoded_point(false);
		let pk_bytes = &point.as_bytes()[1..65];

		let message = b"hello dcap";
		let sig: p256::ecdsa::Signature = sk.sign(message);
		let sig_bytes = sig.to_bytes();

		assert!(verify_p256_ecdsa(pk_bytes, message, &sig_bytes).is_ok());
		assert!(verify_p256_ecdsa(pk_bytes, b"wrong", &sig_bytes).is_err());
	}

	// ── DER Parsing Tests ──

	#[test]
	fn der_build_and_extract_pubkey() {
		use p256::ecdsa::{SigningKey, VerifyingKey};
		use test_utils::{build_test_cert, test_signing_key};

		let subject_sk = test_signing_key(10, 0xCC);
		let subject_vk = VerifyingKey::from(&subject_sk);
		let issuer_sk = test_signing_key(20, 0xDD);

		let cert = build_test_cert(&subject_vk, &issuer_sk);
		let extracted = extract_p256_pubkey_from_cert(&cert).unwrap();

		// 验证提取的公钥与原始公钥一致
		let point = subject_vk.to_encoded_point(false);
		let expected = &point.as_bytes()[1..65];
		assert_eq!(&extracted[..], expected);
	}

	#[test]
	fn der_extract_tbs_not_empty() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		let subject_sk = test_signing_key(11, 0xCC);
		let issuer_sk = test_signing_key(21, 0xDD);
		let cert = build_test_cert(&VerifyingKey::from(&subject_sk), &issuer_sk);

		let tbs = extract_tbs_from_cert(&cert).unwrap();
		assert!(!tbs.is_empty());
		// TBS 应以 SEQUENCE tag (0x30) 开头
		assert_eq!(tbs[0], 0x30);
	}

	#[test]
	fn der_extract_signature() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		let subject_sk = test_signing_key(12, 0xCC);
		let issuer_sk = test_signing_key(22, 0xDD);
		let cert = build_test_cert(&VerifyingKey::from(&subject_sk), &issuer_sk);

		let sig = extract_ecdsa_sig_from_cert(&cert).unwrap();
		// 签名应为 64 bytes (r || s)
		assert_eq!(sig.len(), 64);
		// 签名不应全为零
		assert!(sig.iter().any(|&b| b != 0));
	}

	#[test]
	fn der_verify_cert_signature_valid() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		let subject_sk = test_signing_key(13, 0xCC);
		let issuer_sk = test_signing_key(23, 0xDD);
		let issuer_vk = VerifyingKey::from(&issuer_sk);
		let cert = build_test_cert(&VerifyingKey::from(&subject_sk), &issuer_sk);

		// 使用正确的 issuer 公钥验证
		let issuer_point = issuer_vk.to_encoded_point(false);
		let mut issuer_pubkey = [0u8; 64];
		issuer_pubkey.copy_from_slice(&issuer_point.as_bytes()[1..65]);

		assert!(verify_cert_signature(&cert, &issuer_pubkey).is_ok());
	}

	#[test]
	fn der_verify_cert_signature_wrong_issuer() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		let subject_sk = test_signing_key(14, 0xCC);
		let issuer_sk = test_signing_key(24, 0xDD);
		let wrong_sk = test_signing_key(99, 0xEE);
		let wrong_vk = VerifyingKey::from(&wrong_sk);
		let cert = build_test_cert(&VerifyingKey::from(&subject_sk), &issuer_sk);

		// 使用错误的公钥验证
		let wrong_point = wrong_vk.to_encoded_point(false);
		let mut wrong_pubkey = [0u8; 64];
		wrong_pubkey.copy_from_slice(&wrong_point.as_bytes()[1..65]);

		assert!(verify_cert_signature(&cert, &wrong_pubkey).is_err());
	}

	// ── Certificate Chain Tests ──

	#[test]
	fn cert_chain_valid() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		// 创建 3 级证书链: Root → Intermediate → PCK
		let root_sk = test_signing_key(30, 0xAA);
		let intermediate_sk = test_signing_key(31, 0xBB);
		let pck_sk = test_signing_key(32, 0xCC);

		let intermediate_vk = VerifyingKey::from(&intermediate_sk);
		let pck_vk = VerifyingKey::from(&pck_sk);

		// Root 签发 Intermediate cert
		let intermediate_cert = build_test_cert(&intermediate_vk, &root_sk);
		// Intermediate 签发 PCK cert
		let pck_cert = build_test_cert(&pck_vk, &intermediate_sk);

		// 临时将 Root CA 公钥替换为测试用的 (因为 verify_cert_chain 使用硬编码的 INTEL_ROOT_CA_PUBKEY)
		// 这里我们直接测试各步骤

		// Step 1: 验证 Intermediate cert 签名 (用 Root 公钥)
		let root_vk = VerifyingKey::from(&root_sk);
		let root_point = root_vk.to_encoded_point(false);
		let mut root_pubkey = [0u8; 64];
		root_pubkey.copy_from_slice(&root_point.as_bytes()[1..65]);
		assert!(verify_cert_signature(&intermediate_cert, &root_pubkey).is_ok());

		// Step 2: 提取 Intermediate 公钥
		let extracted_intermediate = extract_p256_pubkey_from_cert(&intermediate_cert).unwrap();
		let expected_intermediate_point = intermediate_vk.to_encoded_point(false);
		assert_eq!(&extracted_intermediate[..], &expected_intermediate_point.as_bytes()[1..65]);

		// Step 3: 验证 PCK cert 签名 (用 Intermediate 公钥)
		assert!(verify_cert_signature(&pck_cert, &extracted_intermediate).is_ok());

		// Step 4: 提取 PCK 公钥
		let extracted_pck = extract_p256_pubkey_from_cert(&pck_cert).unwrap();
		let expected_pck_point = pck_vk.to_encoded_point(false);
		assert_eq!(&extracted_pck[..], &expected_pck_point.as_bytes()[1..65]);
	}

	#[test]
	fn cert_chain_wrong_intermediate_issuer() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		// Intermediate 不是被 Root 签发的
		let root_sk = test_signing_key(40, 0xAA);
		let fake_root_sk = test_signing_key(41, 0xFF);
		let intermediate_sk = test_signing_key(42, 0xBB);
		let intermediate_vk = VerifyingKey::from(&intermediate_sk);

		// 用假 Root 签发 Intermediate
		let intermediate_cert = build_test_cert(&intermediate_vk, &fake_root_sk);

		// 用真 Root 公钥验证 → 失败
		let root_vk = VerifyingKey::from(&root_sk);
		let root_point = root_vk.to_encoded_point(false);
		let mut root_pubkey = [0u8; 64];
		root_pubkey.copy_from_slice(&root_point.as_bytes()[1..65]);

		assert!(verify_cert_signature(&intermediate_cert, &root_pubkey).is_err());
	}

	#[test]
	fn cert_chain_tampered_cert_fails() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		let issuer_sk = test_signing_key(50, 0xAA);
		let subject_sk = test_signing_key(51, 0xBB);
		let issuer_vk = VerifyingKey::from(&issuer_sk);

		let mut cert = build_test_cert(&VerifyingKey::from(&subject_sk), &issuer_sk);

		// 篡改 TBS 中的某个字节 (公钥区域之外)
		cert[10] ^= 0xFF;

		let issuer_point = issuer_vk.to_encoded_point(false);
		let mut issuer_pubkey = [0u8; 64];
		issuer_pubkey.copy_from_slice(&issuer_point.as_bytes()[1..65]);

		assert!(verify_cert_signature(&cert, &issuer_pubkey).is_err());
	}

	#[test]
	fn der_rejects_empty_input() {
		assert_eq!(extract_tbs_from_cert(&[]).unwrap_err(), DcapError::CertParsingFailed);
		assert_eq!(extract_p256_pubkey_from_cert(&[]).unwrap_err(), DcapError::CertParsingFailed);
		assert_eq!(extract_ecdsa_sig_from_cert(&[]).unwrap_err(), DcapError::CertParsingFailed);
	}

	// ── Level 4 (Quote + Cert Chain) Tests ──

	#[test]
	fn verify_level4_with_test_chain() {
		use p256::ecdsa::VerifyingKey;
		use test_utils::{build_test_cert, test_signing_key};

		// 创建证书链
		let root_sk = test_signing_key(60, 0xAA);
		let intermediate_sk = test_signing_key(61, 0xBB);
		let pck_sk = test_signing_key(62, 0xCC);

		let intermediate_vk = VerifyingKey::from(&intermediate_sk);
		let pck_vk = VerifyingKey::from(&pck_sk);

		let intermediate_cert = build_test_cert(&intermediate_vk, &root_sk);
		let pck_cert = build_test_cert(&pck_vk, &intermediate_sk);

		// 提取 PCK 公钥用于 Quote 构建
		let pck_pubkey = extract_p256_pubkey_from_cert(&pck_cert).unwrap();

		// 构建一个使用该 PCK key 的 Quote
		let builder = TestQuoteBuilder {
			mrtd: [0xDD; 48],
			report_data: [0u8; 64],
			ak_signing_key: test_signing_key(63, 0xAA),
			pck_signing_key: pck_sk.clone(),
		};
		let quote = builder.build();

		// Level 3 (直接用 PCK 公钥) 应该通过
		let result3 = verify_quote_level3(&quote, &pck_pubkey).unwrap();
		assert!(result3.body_sig_valid);
		assert!(result3.ak_binding_valid);
		assert!(result3.qe_sig_valid);

		// Level 4 (证书链) — 这里 Root CA 不是真正的 Intel Root CA,
		// 所以 verify_quote_with_cert_chain 会失败 (因为硬编码的是 Intel Root CA)
		// 但我们可以验证各组件独立工作
		let root_vk = VerifyingKey::from(&root_sk);
		let root_point = root_vk.to_encoded_point(false);
		let mut root_pubkey = [0u8; 64];
		root_pubkey.copy_from_slice(&root_point.as_bytes()[1..65]);

		// 验证: Root → Intermediate 签名有效
		assert!(verify_cert_signature(&intermediate_cert, &root_pubkey).is_ok());
		// 验证: Intermediate → PCK 签名有效
		let extracted_intermediate = extract_p256_pubkey_from_cert(&intermediate_cert).unwrap();
		assert!(verify_cert_signature(&pck_cert, &extracted_intermediate).is_ok());
		// 验证: PCK → QE Report 签名有效
		assert_eq!(result3.mrtd, [0xDD; 48]);
	}

	// ── SGX Quote v3 Tests ──

	#[test]
	fn sgx_parse_valid_quote() {
		use test_utils::TestSgxQuoteBuilder;
		let builder = TestSgxQuoteBuilder::new(1).with_mrenclave([0xAA; 32]);
		let quote_bytes = builder.build();
		let parsed = parse_sgx_quote(&quote_bytes).unwrap();
		assert_eq!(parsed.version, 3);
		assert_eq!(parsed.att_key_type, 2);
		assert_eq!(parsed.tee_type, TEE_TYPE_SGX);
		assert_eq!(parsed.qe_vendor_id, INTEL_QE_VENDOR_ID);
		assert_eq!(parsed.mrenclave, [0xAA; 32]);
	}

	#[test]
	fn sgx_parse_rejects_short_quote() {
		assert_eq!(parse_sgx_quote(&[0u8; 100]).unwrap_err(), DcapError::QuoteTooShort);
	}

	#[test]
	fn sgx_parse_rejects_tdx_quote() {
		// A TDX Quote v4 should be rejected by SGX parser
		let tdx_builder = TestQuoteBuilder::new(1);
		let tdx_quote = tdx_builder.build();
		assert_eq!(parse_sgx_quote(&tdx_quote).unwrap_err(), DcapError::InvalidVersion);
	}

	#[test]
	fn sgx_verify_level2_valid() {
		use test_utils::TestSgxQuoteBuilder;
		let mut rd = [0u8; 64];
		rd[0] = 0x42;
		let builder = TestSgxQuoteBuilder::new(1)
			.with_mrenclave([0xBB; 32])
			.with_mrsigner([0xCC; 32])
			.with_report_data(rd);
		let quote = builder.build();
		let result = verify_sgx_quote_level2(&quote).unwrap();
		assert_eq!(result.mrenclave, [0xBB; 32]);
		assert_eq!(result.mrsigner, [0xCC; 32]);
		assert_eq!(result.report_data, rd);
		assert!(result.body_sig_valid);
		assert!(result.ak_binding_valid);
		assert!(!result.qe_sig_valid);
	}

	#[test]
	fn sgx_verify_level2_rejects_tampered_body() {
		use test_utils::TestSgxQuoteBuilder;
		let builder = TestSgxQuoteBuilder::new(1).with_mrenclave([0xCC; 32]);
		let mut quote = builder.build();
		// Tamper with MRENCLAVE after signing
		quote[SGX_MRENCLAVE_OFFSET] = 0xFF;
		assert_eq!(
			verify_sgx_quote_level2(&quote).unwrap_err(),
			DcapError::BodySignatureInvalid
		);
	}

	#[test]
	fn sgx_verify_level2_rejects_wrong_ak_binding() {
		use test_utils::TestSgxQuoteBuilder;
		let builder = TestSgxQuoteBuilder::new(1);
		let mut quote = builder.build();
		// Tamper with QE Report's report_data (AK binding hash)
		quote[SGX_QE_REPORT_OFFSET + QE_REPORTDATA_OFFSET] = 0xFF;
		let err = verify_sgx_quote_level2(&quote).unwrap_err();
		assert_eq!(err, DcapError::AttestationKeyBindingFailed);
	}

	#[test]
	fn sgx_verify_level3_valid() {
		use test_utils::TestSgxQuoteBuilder;
		let builder = TestSgxQuoteBuilder::new(2).with_mrenclave([0xDD; 32]);
		let quote = builder.build();
		let pck_key = builder.pck_public_key();
		let result = verify_sgx_quote_level3(&quote, &pck_key).unwrap();
		assert!(result.body_sig_valid);
		assert!(result.ak_binding_valid);
		assert!(result.qe_sig_valid);
		assert_eq!(result.qe_mrenclave, [0x61; 32]);
		assert_eq!(result.qe_mrsigner, [0x62; 32]);
	}

	#[test]
	fn sgx_verify_level3_rejects_wrong_pck() {
		use test_utils::TestSgxQuoteBuilder;
		let builder = TestSgxQuoteBuilder::new(3);
		let quote = builder.build();
		let wrong_pck = [0x99; 64];
		assert!(verify_sgx_quote_level3(&quote, &wrong_pck).is_err());
	}

	#[test]
	fn sgx_verify_level3_rejects_tampered_qe_report() {
		use test_utils::TestSgxQuoteBuilder;
		let builder = TestSgxQuoteBuilder::new(4);
		let mut quote = builder.build();
		let pck_key = builder.pck_public_key();
		// Tamper with QE MRSIGNER
		quote[SGX_QE_REPORT_OFFSET + QE_MRSIGNER_OFFSET] = 0xFF;
		let err = verify_sgx_quote_level3(&quote, &pck_key).unwrap_err();
		assert_eq!(err, DcapError::QeReportSignatureInvalid);
	}
}
