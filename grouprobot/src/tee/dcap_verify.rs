//! DCAP (Data Center Attestation Primitives) 验证存根
//!
//! 提供 TDX/SGX Quote 的链下签名验证框架。
//! 当前为存根实现, 生产环境需集成 Intel DCAP 库或第三方验证服务。
//!
//! 验证流程:
//! 1. 解析 Quote Header + Body
//! 2. 提取 QE (Quoting Enclave) 签名
//! 3. 验证 QE 证书链 → Intel Root CA
//! 4. 检查 TCB (Trusted Computing Base) 状态
//! 5. 返回验证结果 (包含 MRTD, MRENCLAVE, report_data)

use sha2::{Sha256, Digest};
use tracing::{info, warn};

use crate::error::{BotError, BotResult};

/// TDX Quote v4 结构偏移量 (与 pallet 侧保持一致)
#[allow(dead_code)]
pub const TDX_HEADER_LEN: usize = 48;
#[allow(dead_code)]
pub const TDX_MRTD_OFFSET: usize = 184;
#[allow(dead_code)]
pub const TDX_MRTD_LEN: usize = 48;
#[allow(dead_code)]
pub const TDX_REPORTDATA_OFFSET: usize = 568;
#[allow(dead_code)]
pub const TDX_REPORTDATA_LEN: usize = 64;
#[allow(dead_code)]
pub const TDX_MIN_QUOTE_LEN: usize = TDX_REPORTDATA_OFFSET + TDX_REPORTDATA_LEN;

/// DCAP 验证结果
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DcapVerifyResult {
    /// Quote 中提取的 MRTD (48 bytes)
    pub mrtd: [u8; 48],
    /// Quote 中提取的 report_data (64 bytes)
    pub report_data: [u8; 64],
    /// Quote 的 blake2_256 哈希
    pub quote_hash: [u8; 32],
    /// Intel 签名是否验证通过
    pub signature_valid: bool,
    /// TCB 状态 (UpToDate / OutOfDate / Revoked / ConfigNeeded)
    pub tcb_status: TcbStatus,
    /// 验证模式
    pub mode: VerifyMode,
}

/// TCB 安全状态
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcbStatus {
    /// TCB 是最新的
    UpToDate,
    /// TCB 需要更新但仍可信
    OutOfDate,
    /// TCB 需要配置更新
    ConfigNeeded,
    /// TCB 已被撤销 (不可信)
    Revoked,
    /// 未验证 (存根模式)
    Unknown,
}

/// 验证模式
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyMode {
    /// 完整 DCAP 验证 (Intel 签名 + 证书链)
    Full,
    /// 仅结构验证 (解析 Quote, 不验证签名)
    StructureOnly,
    /// 存根模式 (始终返回成功, 仅开发用)
    Stub,
}

/// DCAP 验证器
#[allow(dead_code)]
pub struct DcapVerifier {
    mode: VerifyMode,
}

#[allow(dead_code)]
impl DcapVerifier {
    /// 创建存根验证器 (开发/测试用)
    pub fn stub() -> Self {
        Self { mode: VerifyMode::Stub }
    }

    /// 创建结构验证器 (解析 Quote 但不验证 Intel 签名)
    pub fn structure_only() -> Self {
        Self { mode: VerifyMode::StructureOnly }
    }

    /// 创建完整 DCAP 验证器
    /// TODO: 集成 Intel DCAP QVL 或第三方服务 (如 Automata DCAP Attestation)
    pub fn full() -> Self {
        warn!("⚠️ DcapVerifier::full() 当前回退到 StructureOnly 模式");
        Self { mode: VerifyMode::StructureOnly }
    }

    /// 验证 TDX Quote
    pub fn verify_tdx_quote(&self, quote_raw: &[u8]) -> BotResult<DcapVerifyResult> {
        match self.mode {
            VerifyMode::Stub => self.verify_stub(quote_raw),
            VerifyMode::StructureOnly => self.verify_structure(quote_raw),
            VerifyMode::Full => {
                // TODO: 实现完整 DCAP 验证
                // 1. 解析 Quote Header (version, att_key_type, tee_type)
                // 2. 解析 Certification Data (QE Report + 证书链)
                // 3. 验证 ECDSA P-256 签名
                // 4. 验证 QE 身份 (Intel QE Identity)
                // 5. 验证 TCB Info (Intel TCB Info)
                // 6. 检查 CRL (证书吊销列表)
                warn!("完整 DCAP 验证尚未实现, 回退到结构验证");
                self.verify_structure(quote_raw)
            }
        }
    }

    /// 结构验证: 解析 Quote 提取 MRTD 和 report_data
    fn verify_structure(&self, quote_raw: &[u8]) -> BotResult<DcapVerifyResult> {
        if quote_raw.len() < TDX_MIN_QUOTE_LEN {
            return Err(BotError::AttestationFailed(format!(
                "TDX Quote too short: {} < {} bytes",
                quote_raw.len(), TDX_MIN_QUOTE_LEN
            )));
        }

        // 提取 MRTD
        let mut mrtd = [0u8; 48];
        mrtd.copy_from_slice(&quote_raw[TDX_MRTD_OFFSET..TDX_MRTD_OFFSET + TDX_MRTD_LEN]);

        // 提取 report_data
        let mut report_data = [0u8; 64];
        report_data.copy_from_slice(
            &quote_raw[TDX_REPORTDATA_OFFSET..TDX_REPORTDATA_OFFSET + TDX_REPORTDATA_LEN]
        );

        // 计算 Quote 哈希
        let mut hasher = Sha256::new();
        hasher.update(quote_raw);
        let hash_result = hasher.finalize();
        let mut quote_hash = [0u8; 32];
        quote_hash.copy_from_slice(&hash_result);

        info!(
            mrtd = %hex::encode(&mrtd[..8]),
            report_data_prefix = %hex::encode(&report_data[..8]),
            quote_len = quote_raw.len(),
            "TDX Quote 结构验证完成"
        );

        Ok(DcapVerifyResult {
            mrtd,
            report_data,
            quote_hash,
            signature_valid: false, // 结构验证不验签名
            tcb_status: TcbStatus::Unknown,
            mode: VerifyMode::StructureOnly,
        })
    }

    /// 存根验证: 始终成功 (仅开发/测试)
    fn verify_stub(&self, quote_raw: &[u8]) -> BotResult<DcapVerifyResult> {
        warn!("⚠️ 使用 DCAP 存根验证 — 仅限开发环境");

        let mut mrtd = [0u8; 48];
        let mut report_data = [0u8; 64];
        let mut quote_hash = [0u8; 32];

        if quote_raw.len() >= TDX_MIN_QUOTE_LEN {
            mrtd.copy_from_slice(&quote_raw[TDX_MRTD_OFFSET..TDX_MRTD_OFFSET + TDX_MRTD_LEN]);
            report_data.copy_from_slice(
                &quote_raw[TDX_REPORTDATA_OFFSET..TDX_REPORTDATA_OFFSET + TDX_REPORTDATA_LEN]
            );
        }

        let mut hasher = Sha256::new();
        hasher.update(quote_raw);
        let h = hasher.finalize();
        quote_hash.copy_from_slice(&h);

        Ok(DcapVerifyResult {
            mrtd,
            report_data,
            quote_hash,
            signature_valid: true, // 存根始终返回 true
            tcb_status: TcbStatus::UpToDate,
            mode: VerifyMode::Stub,
        })
    }

    /// 验证 report_data 绑定到指定公钥 + nonce
    pub fn verify_report_data_binding(
        result: &DcapVerifyResult,
        public_key: &[u8; 32],
        nonce: Option<&[u8; 32]>,
    ) -> BotResult<()> {
        let expected_pk_hash = {
            let mut h = Sha256::new();
            h.update(public_key);
            let r: [u8; 32] = h.finalize().into();
            r
        };

        if result.report_data[..32] != expected_pk_hash[..] {
            return Err(BotError::AttestationFailed(
                "report_data[0..32] does not match SHA256(public_key)".into()
            ));
        }

        if let Some(n) = nonce {
            if result.report_data[32..64] != n[..] {
                return Err(BotError::AttestationFailed(
                    "report_data[32..64] does not match expected nonce".into()
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fake_quote(mrtd: &[u8; 48], pk: &[u8; 32], nonce: &[u8; 32]) -> Vec<u8> {
        let mut quote = vec![0u8; TDX_MIN_QUOTE_LEN + 64];
        quote[TDX_MRTD_OFFSET..TDX_MRTD_OFFSET + 48].copy_from_slice(mrtd);
        let pk_hash: [u8; 32] = {
            let mut h = Sha256::new();
            h.update(pk);
            h.finalize().into()
        };
        quote[TDX_REPORTDATA_OFFSET..TDX_REPORTDATA_OFFSET + 32].copy_from_slice(&pk_hash);
        quote[TDX_REPORTDATA_OFFSET + 32..TDX_REPORTDATA_OFFSET + 64].copy_from_slice(nonce);
        quote
    }

    #[test]
    fn stub_verifier_always_succeeds() {
        let v = DcapVerifier::stub();
        let quote = make_fake_quote(&[1u8; 48], &[2u8; 32], &[3u8; 32]);
        let result = v.verify_tdx_quote(&quote).unwrap();
        assert!(result.signature_valid);
        assert_eq!(result.tcb_status, TcbStatus::UpToDate);
        assert_eq!(result.mode, VerifyMode::Stub);
        assert_eq!(result.mrtd, [1u8; 48]);
    }

    #[test]
    fn structure_verifier_extracts_fields() {
        let v = DcapVerifier::structure_only();
        let mrtd = [0xAA; 48];
        let pk = [0xBB; 32];
        let nonce = [0xCC; 32];
        let quote = make_fake_quote(&mrtd, &pk, &nonce);
        let result = v.verify_tdx_quote(&quote).unwrap();
        assert!(!result.signature_valid);
        assert_eq!(result.tcb_status, TcbStatus::Unknown);
        assert_eq!(result.mrtd, mrtd);
    }

    #[test]
    fn structure_verifier_rejects_short_quote() {
        let v = DcapVerifier::structure_only();
        let short = vec![0u8; 100];
        assert!(v.verify_tdx_quote(&short).is_err());
    }

    #[test]
    fn report_data_binding_valid() {
        let v = DcapVerifier::stub();
        let pk = [0x11; 32];
        let nonce = [0x22; 32];
        let quote = make_fake_quote(&[0u8; 48], &pk, &nonce);
        let result = v.verify_tdx_quote(&quote).unwrap();
        assert!(DcapVerifier::verify_report_data_binding(&result, &pk, Some(&nonce)).is_ok());
    }

    #[test]
    fn report_data_binding_wrong_pk() {
        let v = DcapVerifier::stub();
        let pk = [0x11; 32];
        let wrong_pk = [0x99; 32];
        let nonce = [0x22; 32];
        let quote = make_fake_quote(&[0u8; 48], &pk, &nonce);
        let result = v.verify_tdx_quote(&quote).unwrap();
        assert!(DcapVerifier::verify_report_data_binding(&result, &wrong_pk, Some(&nonce)).is_err());
    }

    #[test]
    fn report_data_binding_wrong_nonce() {
        let v = DcapVerifier::stub();
        let pk = [0x11; 32];
        let nonce = [0x22; 32];
        let wrong_nonce = [0xFF; 32];
        let quote = make_fake_quote(&[0u8; 48], &pk, &nonce);
        let result = v.verify_tdx_quote(&quote).unwrap();
        assert!(DcapVerifier::verify_report_data_binding(&result, &pk, Some(&wrong_nonce)).is_err());
    }

    #[test]
    fn report_data_binding_no_nonce_check() {
        let v = DcapVerifier::stub();
        let pk = [0x11; 32];
        let nonce = [0x22; 32];
        let quote = make_fake_quote(&[0u8; 48], &pk, &nonce);
        let result = v.verify_tdx_quote(&quote).unwrap();
        assert!(DcapVerifier::verify_report_data_binding(&result, &pk, None).is_ok());
    }
}
