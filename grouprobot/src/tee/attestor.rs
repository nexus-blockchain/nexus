use std::sync::Arc;
use base64::Engine;
use sha2::{Sha256, Digest};
use tracing::{info, warn};

use crate::chain::types::AttestationBundle;
use crate::error::{BotError, BotResult};
use crate::tee::enclave_bridge::{EnclaveBridge, TeeMode};

/// TEE 证明有效期 (秒)
pub const QUOTE_VALIDITY_SECS: u64 = 24 * 3600; // 24h
/// 证明刷新提前量 (秒)
pub const QUOTE_REFRESH_MARGIN_SECS: u64 = 3600; // 1h

/// TDX+SGX 双证明生成器
pub struct Attestor {
    enclave: Arc<EnclaveBridge>,
    current: std::sync::Mutex<Option<AttestationBundle>>,
}

impl Attestor {
    pub fn new(enclave: Arc<EnclaveBridge>) -> Self {
        Self {
            enclave,
            current: std::sync::Mutex::new(None),
        }
    }

    /// 生成双证明 (软件模式)
    pub fn generate_attestation(&self) -> BotResult<AttestationBundle> {
        self.generate_attestation_with_nonce(None)
    }

    /// 生成双证明, 可选链上 nonce (硬件模式防重放)
    ///
    /// 硬件模式: report_data = SHA256(pk) || nonce, 保留 raw quote
    /// 软件模式: 模拟证明, 忽略 nonce
    pub fn generate_attestation_with_nonce(&self, nonce: Option<[u8; 32]>) -> BotResult<AttestationBundle> {
        let public_key = self.enclave.public_key_bytes();

        let bundle = match self.enclave.mode() {
            TeeMode::Hardware => self.generate_hardware_attestation(&public_key, nonce)?,
            TeeMode::Software => self.generate_simulated_attestation(&public_key),
        };

        // 缓存
        let mut current = self.current.lock().unwrap();
        *current = Some(bundle.clone());

        Ok(bundle)
    }

    /// 获取当前缓存的证明
    pub fn current_attestation(&self) -> Option<AttestationBundle> {
        self.current.lock().unwrap().clone()
    }

    /// 硬件模式: 读取 TDX/SGX Quote
    ///
    /// report_data[0..32] = SHA256(public_key)
    /// report_data[32..64] = nonce (如果提供) 或全零
    fn generate_hardware_attestation(
        &self,
        public_key: &[u8; 32],
        nonce: Option<[u8; 32]>,
    ) -> BotResult<AttestationBundle> {
        // report_data = SHA256(public_key) || nonce
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        let pk_hash: [u8; 32] = hasher.finalize().into();

        let mut report_data_full = [0u8; 64];
        report_data_full[..32].copy_from_slice(&pk_hash);
        if let Some(ref n) = nonce {
            report_data_full[32..64].copy_from_slice(n);
        }

        // TDX Quote (写入完整 64 bytes report_data)
        let tdx_quote = Self::read_tdx_quote_full(&report_data_full)?;
        let tdx_quote_hash = Self::hash_bytes(&tdx_quote);

        // SGX Quote (可选)
        let sgx_quote = Self::generate_sgx_quote(&pk_hash).unwrap_or_default();
        let sgx_quote_hash = Self::hash_bytes(&sgx_quote);

        // 提取 MRTD/MRENCLAVE
        let mrtd = Self::extract_mrtd(&tdx_quote);
        let mrenclave = Self::extract_mrenclave(&sgx_quote);

        // Level 4: 从 Quote 尾部提取证书链 (PEM→DER)
        let (pck_cert_der, intermediate_cert_der) = match Self::extract_cert_chain_from_quote(&tdx_quote) {
            Ok((pck, inter)) => {
                info!(pck_len = pck.len(), inter_len = inter.len(), "证书链提取成功 (Level 4 就绪)");
                (Some(pck), Some(inter))
            }
            Err(e) => {
                warn!(error = %e, "证书链提取失败, 将降级到 Level 1");
                (None, None)
            }
        };

        info!("硬件双证明生成成功 (nonce={}, level4={})", nonce.is_some(), pck_cert_der.is_some());

        Ok(AttestationBundle {
            tdx_quote_hash,
            sgx_quote_hash,
            mrtd,
            mrenclave,
            is_simulated: false,
            tdx_quote_raw: Some(tdx_quote),
            nonce,
            pck_cert_der,
            intermediate_cert_der,
        })
    }

    /// 软件模式: 生成模拟证明
    fn generate_simulated_attestation(&self, public_key: &[u8; 32]) -> AttestationBundle {
        let mut hasher = Sha256::new();
        hasher.update(b"simulated-tdx-quote-");
        hasher.update(public_key);
        let tdx_quote_hash: [u8; 32] = hasher.finalize().into();

        let mut hasher = Sha256::new();
        hasher.update(b"simulated-sgx-quote-");
        hasher.update(public_key);
        let sgx_quote_hash: [u8; 32] = hasher.finalize().into();

        let mut mrtd = [0u8; 48];
        mrtd[..32].copy_from_slice(&tdx_quote_hash);

        let mut mrenclave = [0u8; 32];
        mrenclave.copy_from_slice(&sgx_quote_hash);

        warn!("使用模拟证明 (软件模式)");

        AttestationBundle {
            tdx_quote_hash,
            sgx_quote_hash,
            mrtd,
            mrenclave,
            is_simulated: true,
            tdx_quote_raw: None,
            nonce: None,
            pck_cert_der: None,
            intermediate_cert_der: None,
        }
    }

    /// 写入完整 64 字节 report_data 并读取 TDX Quote
    fn read_tdx_quote_full(report_data: &[u8; 64]) -> BotResult<Vec<u8>> {
        let user_data_path = "/dev/attestation/user_report_data";
        let quote_path = "/dev/attestation/quote";

        std::fs::write(user_data_path, report_data)
            .map_err(|e| BotError::AttestationFailed(format!("write report_data: {}", e)))?;

        let quote = std::fs::read(quote_path)
            .map_err(|e| BotError::AttestationFailed(format!("read quote: {}", e)))?;

        Ok(quote)
    }

    fn generate_sgx_quote(_report_data: &[u8; 32]) -> BotResult<Vec<u8>> {
        // SGX Quote 生成 (需要 SGX SDK)
        // 当前简化: 返回 report_data 的哈希作为占位
        Err(BotError::AttestationFailed("SGX not available".into()))
    }

    fn hash_bytes(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    fn extract_mrtd(tdx_quote: &[u8]) -> [u8; 48] {
        // TDX Quote v4: MRTD at offset 184, 48 bytes (与 pallet 侧一致)
        let mut mrtd = [0u8; 48];
        if tdx_quote.len() >= 232 {
            mrtd.copy_from_slice(&tdx_quote[184..232]);
        }
        mrtd
    }

    /// 从 TDX Quote v4 尾部提取 Certification Data 中的 PEM 证书链
    ///
    /// Quote 结构 (QE Auth Data 之后):
    /// ```text
    /// [QE Auth Len: 2] [QE Auth Data: variable]
    /// [CertDataType: 2 LE] [CertDataSize: 4 LE] [CertData: variable]
    /// ```
    /// CertDataType=5 → CertData 是 PEM 拼接的证书链:
    /// PCK Cert + Intermediate CA + (可选) Root CA
    fn extract_cert_chain_from_quote(raw: &[u8]) -> BotResult<(Vec<u8>, Vec<u8>)> {
        // QE Auth Data 偏移量 (与 pallet dcap.rs 一致)
        const QE_AUTH_LEN_OFFSET: usize = 1212;

        if raw.len() < QE_AUTH_LEN_OFFSET + 2 {
            return Err(BotError::AttestationFailed("quote too short for QE auth".into()));
        }

        let qe_auth_len = u16::from_le_bytes([raw[QE_AUTH_LEN_OFFSET], raw[QE_AUTH_LEN_OFFSET + 1]]) as usize;
        let cert_data_start = QE_AUTH_LEN_OFFSET + 2 + qe_auth_len;

        // CertDataType (2) + CertDataSize (4) = 6 bytes header
        if raw.len() < cert_data_start + 6 {
            return Err(BotError::AttestationFailed("quote too short for cert data header".into()));
        }

        let cert_data_type = u16::from_le_bytes([raw[cert_data_start], raw[cert_data_start + 1]]);
        let cert_data_size = u32::from_le_bytes([
            raw[cert_data_start + 2], raw[cert_data_start + 3],
            raw[cert_data_start + 4], raw[cert_data_start + 5],
        ]) as usize;

        if cert_data_type != 5 {
            return Err(BotError::AttestationFailed(
                format!("unsupported CertDataType: {} (expected 5=PEM chain)", cert_data_type)
            ));
        }

        let cert_data_end = cert_data_start + 6 + cert_data_size;
        if raw.len() < cert_data_end {
            return Err(BotError::AttestationFailed("cert data truncated".into()));
        }

        let pem_chain = std::str::from_utf8(&raw[cert_data_start + 6..cert_data_end])
            .map_err(|_| BotError::AttestationFailed("cert data not valid UTF-8".into()))?;

        // 拆分 PEM 证书: 按 "-----BEGIN CERTIFICATE-----" 分割
        let certs_der = Self::split_pem_to_der(pem_chain)?;

        if certs_der.len() < 2 {
            return Err(BotError::AttestationFailed(
                format!("expected ≥2 certs in chain, got {}", certs_der.len())
            ));
        }

        // 顺序: certs_der[0]=PCK, certs_der[1]=Intermediate CA, [2]=Root CA(可选)
        Ok((certs_der[0].clone(), certs_der[1].clone()))
    }

    /// 将 PEM 拼接的证书链拆分并逐个转换为 DER
    fn split_pem_to_der(pem_chain: &str) -> BotResult<Vec<Vec<u8>>> {
        let mut certs = Vec::new();
        let mut in_cert = false;
        let mut b64_buf = String::new();

        for line in pem_chain.lines() {
            let trimmed = line.trim();
            if trimmed == "-----BEGIN CERTIFICATE-----" {
                in_cert = true;
                b64_buf.clear();
            } else if trimmed == "-----END CERTIFICATE-----" {
                in_cert = false;
                let der = base64::engine::general_purpose::STANDARD.decode(&b64_buf)
                    .map_err(|e| BotError::AttestationFailed(
                        format!("PEM base64 decode failed: {}", e)
                    ))?;
                certs.push(der);
            } else if in_cert {
                b64_buf.push_str(trimmed);
            }
        }

        Ok(certs)
    }

    fn extract_mrenclave(sgx_quote: &[u8]) -> [u8; 32] {
        let mut mrenclave = [0u8; 32];
        if sgx_quote.len() >= 144 {
            mrenclave.copy_from_slice(&sgx_quote[112..144]);
        }
        mrenclave
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_attestation() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = Arc::new(
            EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap()
        );
        let attestor = Attestor::new(enclave);
        let bundle = attestor.generate_attestation().unwrap();
        assert!(bundle.is_simulated);
        assert_ne!(bundle.tdx_quote_hash, [0u8; 32]);
        assert_ne!(bundle.sgx_quote_hash, [0u8; 32]);
    }

    #[test]
    fn attestation_cached() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = Arc::new(
            EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap()
        );
        let attestor = Attestor::new(enclave);
        assert!(attestor.current_attestation().is_none());
        attestor.generate_attestation().unwrap();
        assert!(attestor.current_attestation().is_some());
    }

    #[test]
    fn attestation_deterministic_for_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let enclave = Arc::new(
            EnclaveBridge::init(dir.path().to_str().unwrap(), "software").unwrap()
        );
        let attestor = Attestor::new(enclave);
        let b1 = attestor.generate_attestation().unwrap();
        let b2 = attestor.generate_attestation().unwrap();
        assert_eq!(b1.tdx_quote_hash, b2.tdx_quote_hash);
    }
}
