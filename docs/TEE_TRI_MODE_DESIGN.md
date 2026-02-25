# TEE 三模式互通兼容设计文档

> SGX2-Only / TDX-Only / TDX+SGX — 互切换、可兼容、可迁移、可并行

**版本**: v1.0
**日期**: 2026-02-25
**状态**: RFC (Request for Comments)

---

## 目录

1. [概述与目标](#1-概述与目标)
2. [现状分析](#2-现状分析)
3. [链侧重构方案](#3-链侧重构方案)
4. [Bot 侧重构方案](#4-bot-侧重构方案)
5. [Shamir 分片跨模式兼容](#5-shamir-分片跨模式兼容)
6. [安全模型对比](#6-安全模型对比)
7. [迁移风险矩阵](#7-迁移风险矩阵)
8. [实施路线图](#8-实施路线图)
9. [附录：改动文件清单](#9-附录改动文件清单)

---

## 1. 概述与目标

### 1.1 背景

当前系统的 TEE 架构以 TDX 为核心，SGX 作为补充（双证明模式）。这导致：

- **SGX-Only 节点无法独立注册**：链侧所有证明入口都要求 TDX Quote
- **平台锁定**：只能部署在支持 TDX 的平台（Azure、自建），无法利用 SGX-Only 平台（阿里云 g7t 等）
- **单点依赖**：Intel TDX 技术路线变化会影响整个网络

### 1.2 目标

| 目标 | 描述 |
|------|------|
| **三模式并行** | SGX-Only、TDX-Only、TDX+SGX 节点可同时存在于网络中 |
| **互切换** | 节点可从一种 TEE 模式迁移到另一种，无需链上特殊操作 |
| **向后兼容** | 现有 TDX+SGX 节点零改动继续运行 |
| **统一抽象** | 链侧和 Bot 侧使用统一的 TEE 抽象，消除硬编码耦合 |
| **分片互通** | 不同 TEE 模式的节点可混合组成 Shamir 分片集群 |

### 1.3 核心设计原则

```
1. Quote-Type-Agnostic  — 自动检测 Quote 类型 (version=3 → SGX, version=4 → TDX)
2. Measurement-Polymorphic — 统一度量值容器 [u8; 48]，兼容 MRTD(48B) 和 MRENCLAVE(32B+pad)
3. Backward-Compatible — 新增 enum variant + 新增 extrinsic，零破坏性变更
4. Zero-Config Detection — Bot 启动时自动检测 TEE 类型，现有配置无需修改
```

---

## 2. 现状分析

### 2.1 链侧耦合点 (7 处 TDX 锁定)

| ID | 文件 | 耦合描述 | 影响 |
|----|------|---------|------|
| C1 | `primitives/src/lib.rs:72-88` | `NodeType::TeeNode` 强制 `mrtd: [u8; 48]` + `tdx_attested_at: u64` | SGX-Only 无法表示 |
| C2 | `registry/src/lib.rs:88-106` | `AttestationRecord.mrtd` 非 Option 必填 | SGX-Only 无 MRTD |
| C3 | `registry/src/lib.rs:90` | `AttestationRecord.tdx_quote_hash` 非 Option 必填 | SGX-Only 无 TDX Quote |
| C4 | `registry/src/lib.rs:588-589` | 所有路径硬检查 `ApprovedMrtd` | SGX-Only 无 MRTD |
| C5 | `registry/src/lib.rs:713-805` | `submit_verified_attestation` 只接受 TDX Quote | 阻塞 SGX-Only |
| C6 | `registry/src/lib.rs:903-991` | `submit_dcap_attestation` 调用 TDX-only DCAP | 阻塞 SGX-Only |
| C7 | `registry/src/lib.rs:1217+` | `submit_sgx_attestation` 要求先有 TDX 证明 | SGX 不能独立 |

### 2.2 链侧已有 SGX 基础设施 (可复用)

| 组件 | 描述 |
|------|------|
| `dcap.rs` — `parse_sgx_quote()` | SGX Quote v3 完整解析 |
| `dcap.rs` — `verify_sgx_quote_level2/3/4()` | SGX DCAP L2/L3/L4 验证 |
| `dcap.rs` — `SgxVerifyResult` | SGX 验证结果类型 |
| `ApprovedMrenclave` StorageMap | MRENCLAVE 白名单 (已有) |
| `verify_p256_ecdsa()` | SGX/TDX 共享的 ECDSA 验证 |

### 2.3 Bot 侧耦合点 (8 处 TDX 锁定)

| ID | 文件 | 耦合描述 |
|----|------|---------|
| B1 | `enclave_bridge.rs:8-14` | `TeeMode` 只有 `Hardware`/`Software`，不区分 SGX/TDX |
| B2 | `enclave_bridge.rs:195-234` | `read_tdx_seal_entropy()` 名称和 fallback 假设 TDX |
| B3 | `attestor.rs:62-115` | `generate_hardware_attestation` 只生成 TDX Quote |
| B4 | `attestor.rs:176-183` | `extract_mrtd` 硬编码 offset 184 (TDX) |
| B5 | `attestor.rs:150-162` | `read_tdx_quote_full` 函数名和语义绑定 TDX |
| B6 | `ra_tls.rs:410-443` | `generate_provision_quote` 假设 TDX Quote |
| B7 | `ra_tls.rs:446-453` | `extract_mrtd_from_quote` 硬编码 TDX offset |
| B8 | `types.rs:162-176` | `AttestationBundle` 只有 `tdx_quote_raw`，无 `sgx_quote_raw` |

### 2.4 Bot 侧已有 SGX 兼容点 (零改动)

| 模块 | 兼容原因 |
|------|---------|
| `sealed_storage.rs` | **优先** SGX seal key，自动 fallback TDX MRTD |
| `vault_server.rs` | `generate_tee_quote` 已 auto-detect SGX v3 / TDX v4 |
| `shamir.rs` | 纯密码学 (ECDH/AES-GCM)，TEE 无关 |
| `ceremony.rs` | share 分发基于 Ed25519 密钥，TEE 无关 |
| `share_recovery.rs` | ECDH 解密 peer share，TEE 无关 |
| `token_vault.rs` | 纯内存操作 |
| 全部 `processing/` `platform/` `infra/` | 业务逻辑层，TEE 无关 |

---

## 3. 链侧重构方案

### 3.1 新增 `TeeType` 枚举

**文件**: `pallets/grouprobot/primitives/src/lib.rs`

```rust
/// TEE 硬件类型
#[derive(
    Encode, Decode, codec::DecodeWithMemTracking, Clone, Copy,
    RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
pub enum TeeType {
    /// TDX-Only: primary_measurement = MRTD (48 bytes)
    Tdx,
    /// SGX-Only: primary_measurement = MRENCLAVE (32 bytes, padded to 48)
    Sgx,
    /// TDX + SGX 双证明: primary_measurement = MRTD, mrenclave = Some(...)
    TdxPlusSgx,
}

impl Default for TeeType {
    fn default() -> Self {
        Self::Tdx
    }
}
```

### 3.2 新增 `TeeNodeV2` variant

**文件**: `pallets/grouprobot/primitives/src/lib.rs`

**策略**: 保留旧 `TeeNode` variant 确保 SCALE 编码向后兼容，新增 `TeeNodeV2`。

```rust
pub enum NodeType {
    /// 普通节点 (无 TEE 证明)
    StandardNode,

    /// V1: TDX + 可选 SGX (向后兼容, 不修改)
    TeeNode {
        mrtd: [u8; 48],
        mrenclave: Option<[u8; 32]>,
        tdx_attested_at: u64,
        sgx_attested_at: Option<u64>,
        expires_at: u64,
    },

    /// V2: 三模式统一 (新节点使用)
    TeeNodeV2 {
        /// 统一度量值: MRTD(48B) 或 MRENCLAVE(32B + 16B zero-pad)
        primary_measurement: [u8; 48],
        /// TEE 类型
        tee_type: TeeType,
        /// SGX MRENCLAVE (原始 32B; TDX+SGX 双证明时有值, SGX-Only 时同 primary[..32])
        mrenclave: Option<[u8; 32]>,
        /// 主证明提交区块
        attested_at: u64,
        /// 补充 SGX 证明提交区块 (仅 TDX+SGX)
        sgx_attested_at: Option<u64>,
        /// 证明过期区块
        expires_at: u64,
    },
}
```

**度量值编码规则**:

```
TeeType::Tdx       → primary_measurement = MRTD[48]
TeeType::Sgx       → primary_measurement = MRENCLAVE[32] || 0x00[16]
TeeType::TdxPlusSgx → primary_measurement = MRTD[48], mrenclave = Some(MRENCLAVE[32])
```

优势: `[u8; 48]` 统一大小 → SCALE MaxEncodedLen 不变 → 无存储迁移。

### 3.3 `AttestationRecord` 扩展

**文件**: `pallets/grouprobot/registry/src/lib.rs`

不修改现有 `AttestationRecord` (避免存储迁移)，新增 `AttestationRecordV2`:

```rust
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct AttestationRecordV2<T: Config> {
    pub bot_id_hash: BotIdHash,
    /// 主 Quote hash (TDX v4 或 SGX v3)
    pub primary_quote_hash: [u8; 32],
    /// 补充 Quote hash (TDX+SGX 双证明时的第二个 Quote)
    pub secondary_quote_hash: Option<[u8; 32]>,
    /// 统一度量值
    pub primary_measurement: [u8; 48],
    /// SGX MRENCLAVE (原始 32B)
    pub mrenclave: Option<[u8; 32]>,
    /// TEE 类型
    pub tee_type: TeeType,
    pub attester: T::AccountId,
    pub attested_at: BlockNumberFor<T>,
    pub expires_at: BlockNumberFor<T>,
    pub is_dual_attestation: bool,
    pub quote_verified: bool,
    pub dcap_level: u8,
    pub api_server_mrtd: Option<[u8; 48]>,
    pub api_server_quote_hash: Option<[u8; 32]>,
}
```

**存储**: 新增 `AttestationsV2` StorageMap，与旧 `Attestations` 并存。

```rust
#[pallet::storage]
pub type AttestationsV2<T: Config> =
    StorageMap<_, Blake2_128Concat, BotIdHash, AttestationRecordV2<T>>;
```

### 3.4 统一白名单检查 Helper

```rust
/// 根据 TEE 类型检查度量值白名单
fn check_measurement_approved(
    tee_type: &TeeType,
    primary_measurement: &[u8; 48],
    mrenclave: &Option<[u8; 32]>,
) -> DispatchResult {
    match tee_type {
        TeeType::Tdx | TeeType::TdxPlusSgx => {
            ensure!(
                ApprovedMrtd::<T>::contains_key(primary_measurement),
                Error::<T>::MrtdNotApproved
            );
        }
        TeeType::Sgx => {
            let mut mre = [0u8; 32];
            mre.copy_from_slice(&primary_measurement[..32]);
            ensure!(
                ApprovedMrenclave::<T>::contains_key(&mre),
                Error::<T>::MrenclaveNotApproved
            );
        }
    }
    // 双证明模式: 额外检查 MRENCLAVE
    if let Some(ref mre) = mrenclave {
        if matches!(tee_type, TeeType::TdxPlusSgx) {
            ensure!(
                ApprovedMrenclave::<T>::contains_key(mre),
                Error::<T>::MrenclaveNotApproved
            );
        }
    }
    Ok(())
}
```

### 3.5 新增统一 Extrinsic: `submit_tee_attestation`

**call_index(21)** — 三模式统一入口

```rust
/// 提交 TEE 证明 (SGX v3 / TDX v4 自动检测)
///
/// 自动检测 Quote 类型 (version 字段):
/// - version=3 → SGX Quote v3 → 提取 MRENCLAVE
/// - version=4 → TDX Quote v4 → 提取 MRTD
///
/// DCAP 验证级别:
/// - 无 platform_id, 无 certs → Level 2 (Body sig + AK binding)
/// - 有 platform_id          → Level 3 (+ QE Report sig via PCK)
/// - 有 certs                → Level 4 (+ Intel Root CA cert chain)
#[pallet::call_index(21)]
#[pallet::weight(Weight::from_parts(250_000_000, 40_000))]
pub fn submit_tee_attestation(
    origin: OriginFor<T>,
    bot_id_hash: BotIdHash,
    quote_raw: BoundedVec<u8, T::MaxQuoteLen>,
    platform_id: Option<[u8; 32]>,
    pck_cert_der: Option<BoundedVec<u8, T::MaxQuoteLen>>,
    intermediate_cert_der: Option<BoundedVec<u8, T::MaxQuoteLen>>,
) -> DispatchResult {
    let who = ensure_signed(origin)?;
    let mut bot = Bots::<T>::get(&bot_id_hash).ok_or(Error::<T>::BotNotFound)?;
    ensure!(bot.owner == who, Error::<T>::NotBotOwner);
    ensure!(bot.status == BotStatus::Active, Error::<T>::BotNotActive);

    let quote = quote_raw.as_slice();
    ensure!(quote.len() >= 48, Error::<T>::QuoteTooShort); // 最小 header

    // ── Step 1: 自动检测 Quote 类型 ──
    let version = u16::from_le_bytes([quote[0], quote[1]]);
    let (tee_type, primary_measurement, mrenclave, report_data, dcap_level, quote_verified)
        = match version {
        4 => {
            // TDX Quote v4 → 现有 DCAP 验证逻辑
            Self::verify_tdx_quote(quote, &platform_id, &pck_cert_der, &intermediate_cert_der)?
        }
        3 => {
            // SGX Quote v3 → 现有 SGX DCAP 验证逻辑
            Self::verify_sgx_quote(quote, &platform_id, &pck_cert_der, &intermediate_cert_der)?
        }
        _ => return Err(Error::<T>::DcapQuoteInvalid.into()),
    };

    // ── Step 2: report_data binding ──
    let expected_hash = sp_core::hashing::sha2_256(&bot.public_key);
    ensure!(report_data[..32] == expected_hash[..], Error::<T>::QuoteReportDataMismatch);

    // ── Step 3: Nonce 验证 (防重放) ──
    let now = frame_system::Pallet::<T>::block_number();
    let (stored_nonce, issued_at) = AttestationNonces::<T>::get(&bot_id_hash)
        .ok_or(Error::<T>::NonceMissing)?;
    let nonce_deadline = issued_at.saturating_add(T::AttestationValidityBlocks::get());
    ensure!(now <= nonce_deadline, Error::<T>::NonceExpired);
    ensure!(report_data[32..64] == stored_nonce[..], Error::<T>::NonceMismatch);
    AttestationNonces::<T>::remove(&bot_id_hash);

    // ── Step 4: 白名单检查 ──
    Self::check_measurement_approved(&tee_type, &primary_measurement, &mrenclave)?;

    // ── Step 5: 写入 AttestationRecordV2 ──
    let quote_hash = sp_core::hashing::blake2_256(quote);
    let expires_at = now.saturating_add(T::AttestationValidityBlocks::get());

    let record = AttestationRecordV2::<T> {
        bot_id_hash,
        primary_quote_hash: quote_hash,
        secondary_quote_hash: None,
        primary_measurement,
        mrenclave,
        tee_type,
        attester: who,
        attested_at: now,
        expires_at,
        is_dual_attestation: matches!(tee_type, TeeType::TdxPlusSgx),
        quote_verified,
        dcap_level,
        api_server_mrtd: None,
        api_server_quote_hash: None,
    };
    AttestationsV2::<T>::insert(&bot_id_hash, record);

    // ── Step 6: 更新 NodeType → TeeNodeV2 ──
    let now_u64: u64 = now.unique_saturated_into();
    let expires_u64: u64 = expires_at.unique_saturated_into();
    bot.node_type = NodeType::TeeNodeV2 {
        primary_measurement,
        tee_type,
        mrenclave,
        attested_at: now_u64,
        sgx_attested_at: None,
        expires_at: expires_u64,
    };
    Bots::<T>::insert(&bot_id_hash, bot);

    Self::deposit_event(Event::TeeAttestationSubmitted {
        bot_id_hash,
        tee_type,
        dcap_level,
    });
    Ok(())
}
```

**内部 helper — TDX/SGX 分发验证**:

```rust
/// TDX Quote v4 DCAP 验证 (复用现有 dcap:: 函数)
fn verify_tdx_quote(
    quote: &[u8],
    platform_id: &Option<[u8; 32]>,
    pck_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
    intermediate_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
) -> Result<(TeeType, [u8; 48], Option<[u8; 32]>, [u8; 64], u8, bool), DispatchError> {
    // Level 4 > Level 3 > Level 2 自动选择
    let (dcap_result, dcap_level) = if let (Some(pck), Some(inter)) =
        (pck_cert_der, intermediate_cert_der)
    {
        (dcap::verify_quote_with_cert_chain(quote, pck.as_slice(), inter.as_slice())
            .map_err(|e| Self::dcap_error_to_dispatch(e))?, 4u8)
    } else if let Some(ref pid) = platform_id {
        let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
            .ok_or(Error::<T>::PckKeyNotRegistered)?;
        (dcap::verify_quote_level3(quote, &pck_key)
            .map_err(|e| Self::dcap_error_to_dispatch(e))?, 3u8)
    } else {
        (dcap::verify_quote_level2(quote)
            .map_err(|e| Self::dcap_error_to_dispatch(e))?, 2u8)
    };

    let quote_verified = dcap_level >= 3;
    Ok((TeeType::Tdx, dcap_result.mrtd, None, dcap_result.report_data, dcap_level, quote_verified))
}

/// SGX Quote v3 DCAP 验证 (复用现有 dcap:: 函数)
fn verify_sgx_quote(
    quote: &[u8],
    platform_id: &Option<[u8; 32]>,
    pck_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
    intermediate_cert_der: &Option<BoundedVec<u8, T::MaxQuoteLen>>,
) -> Result<(TeeType, [u8; 48], Option<[u8; 32]>, [u8; 64], u8, bool), DispatchError> {
    let (sgx_result, dcap_level) = if let (Some(pck), Some(inter)) =
        (pck_cert_der, intermediate_cert_der)
    {
        (dcap::verify_sgx_quote_with_cert_chain(quote, pck.as_slice(), inter.as_slice())
            .map_err(|e| Self::dcap_error_to_dispatch(e))?, 4u8)
    } else if let Some(ref pid) = platform_id {
        let (pck_key, _) = RegisteredPckKeys::<T>::get(pid)
            .ok_or(Error::<T>::PckKeyNotRegistered)?;
        (dcap::verify_sgx_quote_level3(quote, &pck_key)
            .map_err(|e| Self::dcap_error_to_dispatch(e))?, 3u8)
    } else {
        (dcap::verify_sgx_quote_level2(quote)
            .map_err(|e| Self::dcap_error_to_dispatch(e))?, 2u8)
    };

    // MRENCLAVE → padded to 48B as primary_measurement
    let mut primary = [0u8; 48];
    primary[..32].copy_from_slice(&sgx_result.mrenclave);
    let mrenclave = Some(sgx_result.mrenclave);

    let quote_verified = dcap_level >= 3;
    Ok((TeeType::Sgx, primary, mrenclave, sgx_result.report_data, dcap_level, quote_verified))
}
```

### 3.6 `is_tee_node` 等 Helper 适配

**无需修改**。现有实现检查 `!StandardNode`，V1 和 V2 都通过:

```rust
// 现有代码 — 已兼容 TeeNodeV2
fn is_tee_node(bot_id_hash: &BotIdHash) -> bool {
    Bots::<T>::get(bot_id_hash)
        .map(|b| !matches!(b.node_type, NodeType::StandardNode))
        .unwrap_or(false)
}
```

### 3.7 `on_initialize` 适配

`on_initialize` 中的过期检查需要处理 `TeeNodeV2`:

```rust
// 需要新增的匹配分支:
NodeType::TeeNodeV2 { expires_at, .. } => {
    if now_u64 > expires_at {
        bot.node_type = NodeType::StandardNode;
        Bots::<T>::insert(&bot_id_hash, bot);
        AttestationsV2::<T>::remove(&bot_id_hash);
    }
}
```

### 3.8 新增 Event

```rust
TeeAttestationSubmitted {
    bot_id_hash: BotIdHash,
    tee_type: TeeType,
    dcap_level: u8,
},
```

### 3.9 向后兼容策略

| 组件 | 策略 |
|------|------|
| `NodeType::TeeNode` (V1) | **保留不修改**, SCALE 编码不变 |
| `AttestationRecord` (V1) | **保留不修改**, 旧 extrinsic 继续写入 |
| `Attestations` StorageMap | **保留**, 旧节点继续使用 |
| `AttestationsV2` StorageMap | **新增**, 新节点使用 |
| call_index 6-20 | **全部保留**, 签名不变 |
| call_index 21 | **新增** `submit_tee_attestation` |
| `is_tee_node` | 自动兼容 V1 + V2 |
| `on_initialize` | 新增 `TeeNodeV2` 分支 |

---

## 4. Bot 侧重构方案

### 4.1 `TeeMode` 三态化

**文件**: `grouprobot/src/tee/enclave_bridge.rs`

```rust
// 现有:
pub enum TeeMode { Hardware, Software }

// 重构:
pub enum TeeMode {
    /// TDX 硬件 (Quote v4, 度量值 MRTD)
    Tdx,
    /// SGX 硬件 (Quote v3, 度量值 MRENCLAVE) — 通过 Gramine-SGX
    Sgx,
    /// 纯软件模拟 (开发/测试)
    Software,
}

impl TeeMode {
    /// 是否为硬件 TEE 模式
    pub fn is_hardware(&self) -> bool {
        matches!(self, TeeMode::Tdx | TeeMode::Sgx)
    }
}
```

### 4.2 自动检测逻辑

**文件**: `grouprobot/src/tee/enclave_bridge.rs`

```rust
fn detect_mode(tee_mode_str: &str) -> TeeMode {
    match tee_mode_str {
        "tdx" => TeeMode::Tdx,
        "sgx" => TeeMode::Sgx,
        "hardware" => Self::auto_detect_hardware(),
        "software" => TeeMode::Software,
        _ => {
            if Path::new("/dev/attestation/quote").exists() {
                Self::auto_detect_hardware()
            } else {
                warn!("未检测到 TEE 硬件，使用软件模式");
                TeeMode::Software
            }
        }
    }
}

/// 读取一次 Quote 检测 version → SGX or TDX
fn auto_detect_hardware() -> TeeMode {
    let _ = std::fs::write("/dev/attestation/user_report_data", &[0u8; 64]);
    match std::fs::read("/dev/attestation/quote") {
        Ok(quote) if quote.len() >= 2 => {
            let version = u16::from_le_bytes([quote[0], quote[1]]);
            match version {
                4 => { info!("检测到 TDX 硬件 (Quote v4)"); TeeMode::Tdx }
                3 => { info!("检测到 SGX 硬件 (Quote v3)"); TeeMode::Sgx }
                v => { warn!("未知 Quote version={v}, 回退 TDX"); TeeMode::Tdx }
            }
        }
        _ => { warn!("Quote 读取失败, 回退 Software"); TeeMode::Software }
    }
}
```

**关键**: `TEE_MODE=hardware` (现有配置) 自动触发 `auto_detect_hardware()`，**零配置变更**。

### 4.3 现有代码适配 `is_hardware()`

所有 `match self.mode { Hardware => ..., Software => ... }` 需改为:

```rust
// 方案 A: 使用 is_hardware() (大多数场景)
if self.mode.is_hardware() { ... } else { ... }

// 方案 B: 三路分发 (Attestor 等需要区分 SGX/TDX 的场景)
match self.mode {
    TeeMode::Tdx => { ... }
    TeeMode::Sgx => { ... }
    TeeMode::Software => { ... }
}
```

### 4.4 `AttestationBundle` 统一化

**文件**: `grouprobot/src/chain/types.rs`

```rust
/// Quote 类型
#[derive(Debug, Clone, Copy)]
pub enum QuoteType {
    /// TDX Quote v4
    Tdx,
    /// SGX Quote v3
    Sgx,
    /// 软件模拟
    Simulated,
}

/// 证明包 (V2: 三模式统一)
#[derive(Debug, Clone)]
pub struct AttestationBundle {
    /// 主 Quote 原始字节 (SGX v3 或 TDX v4)
    pub quote_raw: Option<Vec<u8>>,
    /// Quote 类型
    pub quote_type: QuoteType,
    /// 主 Quote hash
    pub quote_hash: [u8; 32],
    /// 主度量值 (MRTD 48B 或 MRENCLAVE 32B padded to 48B)
    pub primary_measurement: [u8; 48],
    /// SGX MRENCLAVE (原始 32B)
    pub mrenclave: [u8; 32],
    pub is_simulated: bool,
    /// 链上 nonce
    pub nonce: Option<[u8; 32]>,
    /// PCK 证书 DER (Level 4)
    pub pck_cert_der: Option<Vec<u8>>,
    /// Intermediate CA 证书 DER (Level 4)
    pub intermediate_cert_der: Option<Vec<u8>>,

    // === 向后兼容字段 (旧 extrinsic 使用) ===
    pub tdx_quote_hash: [u8; 32],
    pub sgx_quote_hash: [u8; 32],
    pub mrtd: [u8; 48],
    pub tdx_quote_raw: Option<Vec<u8>>,
}
```

### 4.5 `Attestor` 三模式分发

**文件**: `grouprobot/src/tee/attestor.rs`

```rust
pub fn generate_attestation_with_nonce(
    &self,
    nonce: Option<[u8; 32]>,
) -> BotResult<AttestationBundle> {
    let public_key = self.enclave.public_key_bytes();
    match self.enclave.mode() {
        TeeMode::Tdx => self.generate_tdx_attestation(&public_key, nonce),
        TeeMode::Sgx => self.generate_sgx_attestation(&public_key, nonce),
        TeeMode::Software => Ok(self.generate_simulated_attestation(&public_key)),
    }
}

/// 通用: 构建 report_data (SGX 和 TDX 共用)
fn build_report_data(pk: &[u8; 32], nonce: Option<[u8; 32]>) -> [u8; 64] {
    let pk_hash = sha2_256(pk);
    let mut report_data = [0u8; 64];
    report_data[..32].copy_from_slice(&pk_hash);
    if let Some(n) = nonce {
        report_data[32..64].copy_from_slice(&n);
    }
    report_data
}

/// 通用: 读取 Gramine Quote (SGX 和 TDX 使用同一接口)
fn read_quote(report_data: &[u8; 64]) -> BotResult<Vec<u8>> {
    std::fs::write("/dev/attestation/user_report_data", report_data)
        .map_err(|e| BotError::EnclaveError(format!("write report_data: {e}")))?;
    std::fs::read("/dev/attestation/quote")
        .map_err(|e| BotError::EnclaveError(format!("read quote: {e}")))
}

/// TDX: 生成 TDX Quote v4 (现有 generate_hardware_attestation 逻辑)
fn generate_tdx_attestation(
    &self,
    pk: &[u8; 32],
    nonce: Option<[u8; 32]>,
) -> BotResult<AttestationBundle> {
    let report_data = Self::build_report_data(pk, nonce);
    let quote = Self::read_quote(&report_data)?;
    let mrtd = Self::extract_mrtd(&quote);        // offset 184, 48B
    let quote_hash = blake2_256(&quote);
    // ... 构建 bundle (与现有逻辑一致)
    // quote_type = QuoteType::Tdx
    // primary_measurement = mrtd
    todo!()
}

/// SGX: 生成 SGX Quote v3
fn generate_sgx_attestation(
    &self,
    pk: &[u8; 32],
    nonce: Option<[u8; 32]>,
) -> BotResult<AttestationBundle> {
    let report_data = Self::build_report_data(pk, nonce);
    let quote = Self::read_quote(&report_data)?;   // Gramine-SGX 返回 SGX Quote v3
    let mrenclave = Self::extract_sgx_mrenclave(&quote); // offset 112, 32B
    let mut primary = [0u8; 48];
    primary[..32].copy_from_slice(&mrenclave);     // padded to 48B
    let quote_hash = blake2_256(&quote);
    // ... 构建 bundle
    // quote_type = QuoteType::Sgx
    // primary_measurement = primary
    todo!()
}

/// 提取 SGX MRENCLAVE (offset 112, 32 bytes)
fn extract_sgx_mrenclave(quote: &[u8]) -> [u8; 32] {
    let mut mre = [0u8; 32];
    if quote.len() >= 144 {
        mre.copy_from_slice(&quote[112..144]);
    }
    mre
}
```

### 4.6 `transactions.rs` 新增统一提交

**文件**: `grouprobot/src/chain/transactions.rs`

```rust
/// 统一 TEE 证明提交 (SGX/TDX auto-detect, call_index 21)
pub async fn submit_tee_attestation(
    &self,
    bot_id_hash: [u8; 32],
    bundle: &AttestationBundle,
) -> BotResult<()> {
    let quote_raw = bundle.quote_raw.as_ref().ok_or_else(|| {
        BotError::AttestationFailed("quote_raw required for tee attestation".into())
    })?;

    let platform_id_val = Value::unnamed_variant("None", vec![]);
    let pck_val = match &bundle.pck_cert_der {
        Some(pck) => Value::unnamed_variant("Some", vec![Value::from_bytes(pck)]),
        None => Value::unnamed_variant("None", vec![]),
    };
    let inter_val = match &bundle.intermediate_cert_der {
        Some(inter) => Value::unnamed_variant("Some", vec![Value::from_bytes(inter)]),
        None => Value::unnamed_variant("None", vec![]),
    };

    let tx = subxt::dynamic::tx(
        "GroupRobotRegistry", "submit_tee_attestation",
        vec![
            Value::from_bytes(bot_id_hash),
            Value::from_bytes(quote_raw),
            platform_id_val,
            pck_val,
            inter_val,
        ],
    );

    self.submit_and_watch(tx, "submit_tee_attestation").await
}
```

### 4.7 `ra_tls.rs` 度量值提取统一化

**文件**: `grouprobot/src/tee/ra_tls.rs`

```rust
// 替换 extract_mrtd_from_quote:
fn extract_measurement_from_quote(quote: &[u8]) -> (Vec<u8>, &'static str) {
    if quote.len() >= 2 {
        let version = u16::from_le_bytes([quote[0], quote[1]]);
        match version {
            4 if quote.len() >= 232 => (quote[184..232].to_vec(), "mrtd"),
            3 if quote.len() >= 144 => (quote[112..144].to_vec(), "mrenclave"),
            _ => (vec![], "unknown"),
        }
    } else {
        (vec![], "empty")
    }
}
```

### 4.8 `main.rs` 证明刷新循环适配

```rust
// 现有:
let is_hardware = matches!(enclave.mode(), TeeMode::Hardware);

// 重构:
let is_hardware = enclave.mode().is_hardware();

// 硬件刷新路径: 统一使用 submit_tee_attestation
if is_hardware {
    refresh_hardware_attestation_v2(&chain, &attestor, &bot_id_hash).await?;
} else {
    // 软件路径不变
}
```

### 4.9 `sealed_storage.rs` 和 `enclave_bridge.rs` 的 seal_key

**无需修改**。现有逻辑已经是 SGX-first fallback TDX:

```
SGX seal key (/dev/attestation/keys/_sgx_mrenclave) 可用? → 使用
  ↓ 否
TDX MRTD 可用? → 使用
  ↓ 否
错误 (拒绝降级)
```

- SGX-Only: 走 SGX seal key 路径 ✅
- TDX-Only: 走 TDX MRTD 路径 ✅
- TDX+SGX: 走 SGX seal key 路径 ✅

---

## 5. Shamir 分片跨模式兼容

### 5.1 兼容性矩阵

| 操作 | SGX↔TDX | SGX↔TDX+SGX | TDX↔TDX+SGX | 原因 |
|------|---------|-------------|-------------|------|
| Ceremony 分发 | ✅ | ✅ | ✅ | ECDH (X25519) 加密，TEE 无关 |
| Peer share 恢复 | ✅ | ✅ | ✅ | ECDH 解密，TEE 无关 |
| 本地 share 解密 | N/A | N/A | N/A | 各节点独立 seal_key |
| AttestationGuard | ✅ | ✅ | ✅ | `is_tee_node` 检查 `!StandardNode` |

### 5.2 加密路径分析

```
                          TEE-AGNOSTIC (纯密码学)
                    ┌────────────────────────────────┐
                    │                                │
  SGX Node ─── Ed25519 SK ─── X25519 SK ───┐        │
                                            │ ECDH   │
  TDX Node ─── Ed25519 SK ─── X25519 PK ───┘        │
                                            │        │
                                    shared_secret    │
                                            │        │
                                  AES-256-GCM encrypt │
                                            │        │
                                    encrypted_share   │
                    └────────────────────────────────┘
```

Ed25519 密钥对在所有 TEE 模式下以相同方式生成并存储在 `sealed_storage` 中，ECDH 操作纯数学运算。

### 5.3 模式切换时的分片恢复

**场景**: Node B 从 TDX 切换到 SGX

```
1. Node B 本地 share 用旧 TDX seal_key 加密 → 新 SGX seal_key 无法解密 ❌
2. Node B 发起 peer share recovery:
   → 请求 Node A (SGX) 的 share → ECDH 解密 ✅
   → 请求 Node C (TDX+SGX) 的 share → ECDH 解密 ✅
   → K=2 满足 → token 重建 ✅
3. auto_seal_token() → 用新 SGX seal_key 重新加密本地 share ✅
4. 完成，后续恢复可直接用本地 share
```

### 5.4 关键约束

```
在 TEE 模式切换期间，至少 K-1 个 peer 必须在线。
如果所有节点同时切换 TEE 模式，所有本地 share 失效，token 不可恢复。

推荐: 滚动迁移 (rolling migration)
  - 一次只迁移一个节点
  - 确认新节点 share 重建完成后再迁移下一个
```

---

## 6. 安全模型对比

### 6.1 核心属性对比

| 安全属性 | SGX-Only | TDX-Only | TDX+SGX |
|---------|----------|----------|---------|
| **内存隔离粒度** | 进程级 (最小 TCB) | VM 级 | VM + 进程级 |
| **Host Root 对 token 可见性** | ❌ 不可见 | ⚠️ VM 内 root 可见 | ❌ Token 在 SGX enclave 内 |
| **TCB 大小** | SGX 微码 (最小) | TDX 模块 (中) | TDX + SGX (最大) |
| **代码度量** | MRENCLAVE (32B) | MRTD (48B) | MRTD + MRENCLAVE |
| **Seal key 来源** | CPU MRENCLAVE-bound | MRTD 派生 (较弱) | SGX seal key |
| **侧信道防护** | SGX2 硬件缓解 | TDX 内核隔离 | 双重防护 |
| **远程证明强度** | SGX DCAP L2-L4 | TDX DCAP L2-L4 | 双 Quote DCAP |
| **已知攻击面** | SGX 侧信道历史 | 较新，攻击较少 | 两个 TCB |

### 6.2 对 GroupRobot 核心资产的保护排序

```
Token 明文保护:      SGX-Only ≈ TDX+SGX > TDX-Only
Seal key 保护:       SGX-Only > TDX+SGX > TDX-Only
代码完整性:          TDX+SGX > SGX-Only ≈ TDX-Only
TCB 最小化 (攻击面): SGX-Only > TDX-Only > TDX+SGX
整体推荐:           SGX-Only (日常) > TDX+SGX (高安全) > TDX-Only (灾备)
```

### 6.3 各模式推荐场景

| 模式 | 推荐场景 | 平台示例 |
|------|---------|---------|
| **SGX-Only** | 日常运营、成本敏感 | 阿里云 g7t、AWS Nitro Enclaves |
| **TDX-Only** | 灾备、平台多样性 | Azure DCesv5、GCP C3 |
| **TDX+SGX** | 高安全需求、最高链上激励 | 自建服务器 |

---

## 7. 迁移风险矩阵

### 7.1 迁移路径

| 从 ↓ 到 → | SGX-Only | TDX-Only | TDX+SGX |
|------------|----------|----------|---------|
| **SGX-Only** | — | 重新部署 + 重新证明 + share 恢复 | 部署 TDX VM + 补充证明 |
| **TDX-Only** | 重新部署 Gramine-SGX + 重新证明 + share 恢复 | — | 部署 SGX vault + 补充 SGX 证明 |
| **TDX+SGX** | 停止 SGX vault + 切换到 Gramine-SGX | 停止 SGX vault | — |

### 7.2 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| **Seal key 变化导致本地 share 失效** | 确定 (模式切换时) | 高 | 滚动迁移 + peer share recovery |
| **SCALE 编码变化** | 零 | 高 | 新增 variant，不修改旧 variant |
| **链上数据不兼容** | 零 | 高 | 新增 StorageMap，不修改旧 Map |
| **SGX EPC 内存不足** | 低 | 中 | GroupRobot <500MB << 4GB EPC |
| **Gramine SGX/TDX 行为差异** | 中 | 中 | 同一 /dev/attestation/* 接口 |
| **auto-detect 误判** | 极低 | 中 | Quote version 字段确定性; 可 TEE_MODE=sgx 强制 |
| **K 个 peer 不足** | 低 | 高 | 迁移前确认 peer 在线数 ≥ K |
| **Intel 弃用某 TEE 技术** | 低 | 高 | 三模式架构本身即对冲 |

### 7.3 迁移操作步骤 (TDX → SGX 示例)

```
准备阶段:
  1. 确认当前 Shamir K-of-N 参数 (需要 K-1 个 peer 在线)
  2. 确认 MRENCLAVE 已在链上 ApprovedMrenclave 白名单
  3. 准备 Gramine-SGX manifest 文件

执行阶段:
  4. 停止旧 TDX 节点 (但不注销 peer)
  5. 部署新 Gramine-SGX 环境
  6. 启动 Bot → auto_detect_hardware() → TeeMode::Sgx
  7. Bot 自动: request_attestation_nonce → 生成 SGX Quote → submit_tee_attestation
  8. Bot 自动: peer share recovery (ECDH) → token 恢复
  9. Bot 自动: auto_seal_token → 用新 SGX seal_key 保存本地 share

验证阶段:
  10. 确认链上 NodeType = TeeNodeV2 { tee_type: Sgx }
  11. 确认 token 可用 (build_tg_api_url 成功)
  12. 确认 peer heartbeat 正常
  13. 注销旧节点 peer 记录
```

---

## 8. 实施路线图

### Phase 1: 链侧 (3 天)

| 天 | 任务 | 文件 | 改动量 |
|----|------|------|--------|
| D1 | `TeeType` enum + `TeeNodeV2` variant | `primitives/src/lib.rs` | ~30 行 |
| D1 | `AttestationRecordV2` + `AttestationsV2` Storage | `registry/src/lib.rs` | ~40 行 |
| D2 | `submit_tee_attestation` extrinsic + helpers | `registry/src/lib.rs` | ~120 行 |
| D2 | `on_initialize` 适配 TeeNodeV2 | `registry/src/lib.rs` | ~10 行 |
| D2 | 新 Event + Error variants | `registry/src/lib.rs` | ~10 行 |
| D3 | 测试: SGX-Only 证明 | `registry/src/tests.rs` | ~50 行 |
| D3 | 测试: TDX-Only 证明 | `registry/src/tests.rs` | ~30 行 |
| D3 | 测试: 模式切换 (V1→V2) | `registry/src/tests.rs` | ~40 行 |

### Phase 2: Bot 侧 (2 天)

| 天 | 任务 | 文件 | 改动量 |
|----|------|------|--------|
| D4 | `TeeMode` 三态化 + `auto_detect_hardware` | `tee/enclave_bridge.rs` | ~40 行 |
| D4 | `QuoteType` + `AttestationBundle` V2 | `chain/types.rs` | ~25 行 |
| D4 | `Attestor` 三模式分发 + `generate_sgx_attestation` | `tee/attestor.rs` | ~65 行 |
| D5 | `submit_tee_attestation` transaction | `chain/transactions.rs` | ~40 行 |
| D5 | `ra_tls.rs` 度量提取统一化 | `tee/ra_tls.rs` | ~25 行 |
| D5 | `main.rs` 刷新循环适配 | `main.rs` | ~15 行 |

### Phase 3: 集成与验证 (2 天)

| 天 | 任务 |
|----|------|
| D6 | Gramine-SGX manifest 编写 + SGX-sim 本地测试 |
| D6 | 端到端集成测试 (chain + bot) |
| D7 | 阿里云 g7t SGX 真机验证 |
| D7 | 文档最终版 + 迁移指南 |

### 改动量汇总

| 层 | 新增/修改行数 | 零改动行数 |
|----|-------------|-----------|
| 链侧 | ~330 行 | ~2500 行 (dcap.rs 全部复用) |
| Bot 侧 | ~210 行 | ~9000+ 行 (business logic 全部不变) |
| 测试 | ~120 行 | — |
| Manifest | ~50 行 | — |
| **总计** | **~710 行** | **~11500+ 行** |

---

## 9. 附录：改动文件清单

### 链侧 (pallets/)

| 文件 | 操作 | 描述 |
|------|------|------|
| `grouprobot/primitives/src/lib.rs` | 修改 | 新增 `TeeType`、`TeeNodeV2` |
| `grouprobot/registry/src/lib.rs` | 修改 | 新增 `AttestationRecordV2`、`submit_tee_attestation`、helpers |
| `grouprobot/registry/src/tests.rs` | 修改 | 新增三模式测试 |
| `grouprobot/registry/src/dcap.rs` | **不修改** | SGX v3 DCAP 已完整实现 |

### Bot 侧 (grouprobot/src/)

| 文件 | 操作 | 描述 |
|------|------|------|
| `tee/enclave_bridge.rs` | 修改 | `TeeMode` 三态化 + `auto_detect_hardware` |
| `tee/attestor.rs` | 修改 | 三模式分发 + `generate_sgx_attestation` |
| `chain/types.rs` | 修改 | `QuoteType` + `AttestationBundle` V2 |
| `chain/transactions.rs` | 修改 | `submit_tee_attestation` |
| `tee/ra_tls.rs` | 修改 | `extract_measurement_from_quote` |
| `main.rs` | 修改 | 刷新循环 `is_hardware()` |
| `tee/sealed_storage.rs` | **不修改** | 已 SGX-first fallback |
| `tee/vault_server.rs` | **不修改** | 已 auto-detect |
| `tee/shamir.rs` | **不修改** | TEE 无关 |
| `tee/ceremony.rs` | **不修改** | TEE 无关 |
| `tee/share_recovery.rs` | **不修改** | TEE 无关 |
| `tee/token_vault.rs` | **不修改** | TEE 无关 |
| `tee/dcap_verify.rs` | **不修改** | 已支持 SGX+TDX |

---

> **核心结论**: 三模式互通架构总改动 ~710 行，90%+ 代码零改动，向后 100% 兼容，
> Shamir 分片天然跨模式互通。实施周期 7 工作日。
