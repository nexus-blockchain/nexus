# pallet-grouprobot-registry

> 路径：`pallets/grouprobot/registry/`

Bot 注册中心 + TEE 节点管理 + 平台身份绑定 + 多级 DCAP 证明系统。

## 设计理念

- **身份中心**：所有 Bot 以 `bot_id_hash`（32 字节）注册，绑定 Ed25519 公钥
- **TEE 分级证明**：4 级安全验证（Level 0→4），从软件模拟到完整证书链
- **白名单机制**：MRTD / MRENCLAVE / API Server MRTD 均需治理审批
- **防重放**：Nonce 机制（`request_attestation_nonce` → 嵌入 report_data → 提交验证）
- **自动降级**：`on_initialize` 周期扫描过期证明，降级为 StandardNode

## Extrinsics

### Bot 管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `register_bot` | Signed | 注册 Bot（bot_id_hash + 公钥） |
| 1 | `update_public_key` | Signed | 密钥轮换（自动清除旧证明，降级为 StandardNode） |
| 2 | `deactivate_bot` | Signed | 停用 Bot |

### 社区 / 平台绑定
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 3 | `bind_community` | Signed | 绑定社区到 Bot（+community_count） |
| 4 | `unbind_community` | Signed | 解绑社区（-community_count） |
| 5 | `bind_user_platform` | Signed | 用户绑定平台身份（platform + hash） |

### TEE 证明提交
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 6 | `submit_attestation` | Signed | 软件模式提交（Level 0，⚠️ 无签名验证） |
| 7 | `refresh_attestation` | Signed | 刷新证明（24h 周期） |
| 10 | `submit_verified_attestation` | Signed | 结构验证（Level 1，解析 Quote + Nonce 绑定） |
| 11 | `request_attestation_nonce` | Signed | 请求 Nonce（防重放，嵌入 report_data[32..64]） |
| 14 | `submit_dcap_attestation` | Signed | DCAP 验证（Level 2/3，ECDSA P-256 签名验证） |
| 15 | `submit_dcap_dual_attestation` | Signed | 双 Quote（Bot + API Server，同一公钥绑定） |
| 16 | `submit_dcap_full_attestation` | Signed | 完整证书链（Level 4，Intel Root CA → PCK） |

### 白名单管理（Root）
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 8 | `approve_mrtd` | Root | 审批 MRTD 到白名单 |
| 9 | `approve_mrenclave` | Root | 审批 MRENCLAVE 到白名单 |
| 12 | `approve_api_server_mrtd` | Root | 审批 API Server MRTD |
| 13 | `register_pck_key` | Root | 注册 PCK 公钥（Level 3 需要） |

## DCAP 验证级别

| Level | 方法 | 验证内容 | quote_verified |
|:---:|------|---------|:---:|
| 0 | `submit_attestation` | 仅哈希提交，无结构验证 | false |
| 1 | `submit_verified_attestation` | Quote 结构解析 + Nonce + 公钥绑定 | false |
| 2 | `submit_dcap_attestation` | + ECDSA Body 签名 + AK 绑定 QE Report | false |
| 3 | `submit_dcap_attestation` + platform_id | + QE Report 签名（PCK 公钥验证） | **true** |
| 4 | `submit_dcap_full_attestation` | + Intel Root CA → Intermediate → PCK 证书链 | **true** |

## 存储

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Bots` | `Map<BotIdHash, BotInfo>` | Bot 注册表 |
| `OwnerBots` | `Map<AccountId, BoundedVec<BotIdHash>>` | 所有者 Bot 列表 |
| `CommunityBindings` | `Map<CommunityIdHash, CommunityBinding>` | 社区绑定记录 |
| `UserPlatformBindings` | `DoubleMap<AccountId, Platform, [u8;32]>` | 用户平台身份 |
| `Attestations` | `Map<BotIdHash, AttestationRecord>` | TEE 证明记录 |
| `AttestationNonces` | `Map<BotIdHash, ([u8;32], BlockNumber)>` | 防重放 Nonce |
| `ApprovedMrtd` | `Map<[u8;48], u32>` | MRTD 白名单 |
| `ApprovedMrenclave` | `Map<[u8;32], u32>` | MRENCLAVE 白名单 |
| `ApprovedApiServerMrtd` | `Map<[u8;48], u32>` | API Server MRTD 白名单 |
| `RegisteredPckKeys` | `Map<[u8;32], ([u8;64], BlockNumber)>` | PCK 公钥（Level 3） |
| `BotCount` | `u64` | Bot 总数 |

## 主要类型

### BotInfo
```rust
pub struct BotInfo<T: Config> {
    pub owner: T::AccountId,
    pub bot_id_hash: BotIdHash,
    pub public_key: [u8; 32],
    pub status: BotStatus,
    pub registered_at: BlockNumberFor<T>,
    pub node_type: NodeType,
    pub community_count: u32,
}
```

### AttestationRecord
```rust
pub struct AttestationRecord<T: Config> {
    pub bot_id_hash: BotIdHash,
    pub tdx_quote_hash: [u8; 32],
    pub sgx_quote_hash: Option<[u8; 32]>,
    pub mrtd: [u8; 48],
    pub mrenclave: Option<[u8; 32]>,
    pub attester: T::AccountId,
    pub attested_at: BlockNumberFor<T>,
    pub expires_at: BlockNumberFor<T>,
    pub is_dual_attestation: bool,
    pub quote_verified: bool,      // Level 3+ 才为 true
    pub dcap_level: u8,            // 0-4
    pub api_server_mrtd: Option<[u8; 48]>,
    pub api_server_quote_hash: Option<[u8; 32]>,
}
```

## 错误

| 错误 | 说明 |
|------|------|
| `BotAlreadyRegistered` | Bot 已注册 |
| `BotNotFound` | Bot 不存在 |
| `NotBotOwner` | 不是 Bot 所有者 |
| `BotNotActive` | Bot 不活跃 |
| `BotAlreadyDeactivated` | Bot 已停用 |
| `MaxBotsReached` | 所有者 Bot 数量已满 |
| `CommunityAlreadyBound` | 社区已绑定 |
| `CommunityNotBound` | 社区未绑定 |
| `MrtdNotApproved` | MRTD 未在白名单中 |
| `MrenclaveNotApproved` | MRENCLAVE 未在白名单中 |
| `AttestationNotFound` | 证明不存在 |
| `SamePublicKey` | 新公钥不能与旧公钥相同 |
| `QuoteTooShort` | TDX Quote 太短 |
| `QuoteReportDataMismatch` | report_data 与公钥不匹配 |
| `NonceMissing` | 未请求 Nonce |
| `NonceExpired` | Nonce 已过期 |
| `NonceMismatch` | Nonce 不匹配（疑似重放） |
| `DcapQuoteInvalid` | DCAP Quote 结构无效 |
| `DcapBodySignatureInvalid` | ECDSA Body 签名无效 |
| `DcapAkBindingFailed` | AK 未绑定到 QE Report |
| `DcapQeSignatureInvalid` | QE Report 签名无效 |
| `PckKeyNotRegistered` | PCK 公钥未注册 |
| `ApiServerMrtdNotApproved` | API Server MRTD 未审批 |
| `ApiServerReportDataMismatch` | API Server report_data 不匹配 |
| `DcapCertChainInvalid` | 证书链验证失败 |
| `DcapRootCaVerificationFailed` | Root CA 验证失败 |
| `DcapIntermediateCaVerificationFailed` | Intermediate CA 验证失败 |

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxBotsPerOwner` | 单个所有者最大 Bot 数 |
| `MaxPlatformsPerCommunity` | 单个社区最大平台绑定数 |
| `MaxPlatformBindingsPerUser` | 用户最大平台绑定数 |
| `AttestationValidityBlocks` | TEE 证明有效期（区块数，runtime: 14400 ≈ 24h） |
| `AttestationCheckInterval` | 过期扫描间隔（区块数） |
| `MaxQuoteLen` | TDX Quote 最大字节长度 |

## Hooks

- **`on_initialize`**：每 `AttestationCheckInterval` 个区块扫描过期证明，移除记录并将 Bot 降级为 `StandardNode`

## Trait 实现

实现 `BotRegistryProvider<AccountId>`，供 ceremony / community / consensus 子 pallet 查询。

## 子模块

- **`dcap.rs`**：TDX Quote v4 完整解析器 + ECDSA P-256 签名验证（`p256` crate，`no_std` 兼容）

## 相关模块

- [primitives/](../primitives/) — 共享类型定义
- [ceremony/](../ceremony/) — RA-TLS 仪式（依赖 BotRegistryProvider）
- [consensus/](../consensus/) — 节点共识（依赖 BotRegistryProvider）
