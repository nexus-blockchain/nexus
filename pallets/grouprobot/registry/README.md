# pallet-grouprobot-registry

> 路径：`pallets/grouprobot/registry/`

Bot 注册中心 + TEE 节点管理 + 平台身份绑定 + 多级 DCAP 证明系统 + 运营商管理 + Peer 网络。

## 设计理念

- **身份中心**：所有 Bot 以 `bot_id_hash`（32 字节）注册，绑定 Ed25519 公钥
- **TEE 分级证明**：5 级安全验证（Level 0→4），从软件模拟到完整证书链
- **三模式统一入口**：`submit_tee_attestation` 自动检测 SGX v3 / TDX v4 Quote，统一 DCAP 分级
- **白名单机制**：MRTD / MRENCLAVE / API Server MRTD 均需治理审批，支持撤销
- **防重放**：Nonce 机制（`request_attestation_nonce` → 嵌入 report_data → 提交验证）
- **自动降级**：`on_initialize` 从 `AttestationExpiryQueue` 游标化扫描过期证明，降级为 StandardNode
- **运营商系统**：独立运营商注册，多平台身份，Bot 分配与 SLA 等级管理
- **Peer 网络**：TEE 节点端点注册、心跳保活、过期举报、Era Uptime 快照

## Extrinsics

### Bot 管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 0 | `register_bot` | Signed | 注册 Bot（bot_id_hash + 公钥） |
| 1 | `update_public_key` | Signed | 密钥轮换（自动清除 V1+V2 证明，降级为 StandardNode） |
| 2 | `deactivate_bot` | Signed | 停用 Bot（清理证明、Nonce、Peer、运营商关联） |

### 社区 / 平台绑定
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 3 | `bind_community` | Signed | 绑定社区到 Bot（已停用 Bot 的旧绑定自动解绑） |
| 4 | `unbind_community` | Signed | 解绑社区（-community_count） |
| 5 | `bind_user_platform` | Signed | 用户绑定平台身份（覆盖时发出 `UserPlatformBindingUpdated`） |

### TEE 证明提交
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 6 | `submit_attestation` | Signed | 软件模式提交（Level 0，⚠️ 无签名验证） |
| 7 | `refresh_attestation` | Signed | 刷新证明（24h 周期，V1/V2 分别刷新） |
| 10 | `submit_verified_attestation` | Signed | 结构验证（Level 1，解析 Quote + Nonce 绑定） |
| 11 | `request_attestation_nonce` | Signed | 请求 Nonce（防重放，嵌入 report_data[32..64]） |
| 14 | `submit_dcap_attestation` | Signed | DCAP 单 Quote（Level 2/3，ECDSA P-256 签名验证） |
| 15 | `submit_dcap_dual_attestation` | Signed | DCAP 双 Quote（Bot + API Server，同一公钥绑定） |
| 16 | `submit_dcap_full_attestation` | Signed | 完整证书链（Level 4，Intel Root CA → PCK） |
| 20 | `submit_sgx_attestation` | Signed | 补充 SGX Enclave 证明（要求已有 TDX 证明，Level 2/3/4） |
| 21 | `submit_tee_attestation` | Signed | **三模式统一入口**（自动检测 SGX v3 / TDX v4，写入 V2 记录） |

### 白名单管理（Root）
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 8 | `approve_mrtd` | Root | 审批 MRTD 到白名单 |
| 9 | `approve_mrenclave` | Root | 审批 MRENCLAVE 到白名单 |
| 12 | `approve_api_server_mrtd` | Root | 审批 API Server MRTD |
| 13 | `register_pck_key` | Root | 注册 PCK 公钥（Level 3 需要，防重复注册） |
| 29 | `revoke_mrtd` | Root | 撤销 MRTD 白名单（固件漏洞响应） |
| 30 | `revoke_mrenclave` | Root | 撤销 MRENCLAVE 白名单（enclave 漏洞响应） |

### Peer 网络
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 17 | `register_peer` | Signed | 注册 Peer 端点（需付费订阅，Tier gate） |
| 18 | `deregister_peer` | Signed | 注销 Peer 端点（清理心跳计数） |
| 19 | `heartbeat_peer` | Signed | Peer 心跳保活（更新 last_seen + 累加心跳计数） |
| 22 | `report_stale_peer` | Signed | 举报过期 Peer（任何人可调用，被动清理替代全表扫描） |

### 运营商管理
| call_index | 方法 | Origin | 说明 |
|:---:|------|:---:|------|
| 23 | `register_operator` | Signed | 注册运营商（平台 + app_hash 全局唯一） |
| 24 | `update_operator` | Signed | 更新运营商信息（名称/联系方式） |
| 25 | `deregister_operator` | Signed | 注销运营商（要求无活跃 Bot） |
| 26 | `set_operator_sla` | Root | 设置运营商 SLA 等级（0-3） |
| 27 | `assign_bot_to_operator` | Signed | 将 Bot 分配给运营商（需同时为 Bot 所有者和运营商） |
| 28 | `unassign_bot_from_operator` | Signed | 取消 Bot 与运营商的关联 |

## DCAP 验证级别

| Level | 方法 | 验证内容 | quote_verified |
|:---:|------|---------|:---:|
| 0 | `submit_attestation` | 仅哈希提交，无结构验证 | false |
| 1 | `submit_verified_attestation` | Quote 结构解析 + Nonce + 公钥绑定 | false |
| 2 | `submit_dcap_attestation` | + ECDSA Body 签名 + AK 绑定 QE Report | false |
| 3 | `submit_dcap_attestation` + platform_id | + QE Report 签名（PCK 公钥验证） | **true** |
| 4 | `submit_dcap_full_attestation` | + Intel Root CA → Intermediate → PCK 证书链 | **true** |

> `submit_tee_attestation` (call_index 21) 为统一入口，自动检测 Quote version (3=SGX, 4=TDX)，根据提供的参数选择 Level 2/3/4。

## 存储

### Bot 与绑定
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Bots` | `Map<BotIdHash, BotInfo>` | Bot 注册表 |
| `OwnerBots` | `Map<AccountId, BoundedVec<BotIdHash>>` | 所有者 Bot 列表 |
| `CommunityBindings` | `Map<CommunityIdHash, CommunityBinding>` | 社区绑定记录 |
| `UserPlatformBindings` | `DoubleMap<AccountId, Platform, [u8;32]>` | 用户平台身份 |
| `BotCount` | `u64` | Bot 总数 |

### TEE 证明
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Attestations` | `Map<BotIdHash, AttestationRecord>` | V1 TEE 证明记录 |
| `AttestationsV2` | `Map<BotIdHash, AttestationRecordV2>` | V2 三模式统一证明记录 |
| `AttestationNonces` | `Map<BotIdHash, ([u8;32], BlockNumber)>` | 防重放 Nonce |
| `AttestationExpiryQueue` | `BoundedVec<(BlockNumber, BotIdHash, bool), 1000>` | 过期队列（按 expires_at 排序） |

### 白名单
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `ApprovedMrtd` | `Map<[u8;48], u32>` | MRTD 白名单（mrtd → version） |
| `ApprovedMrenclave` | `Map<[u8;32], u32>` | MRENCLAVE 白名单 |
| `ApprovedApiServerMrtd` | `Map<[u8;48], u32>` | API Server MRTD 白名单 |
| `RegisteredPckKeys` | `Map<[u8;32], ([u8;64], BlockNumber)>` | PCK 公钥（Level 3） |

### Peer 网络
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `PeerRegistry` | `Map<BotIdHash, BoundedVec<PeerEndpoint>>` | Peer 端点注册表 |
| `PeerHeartbeatCount` | `DoubleMap<BotIdHash, [u8;32], u32>` | 当前 Era Peer 心跳计数 |
| `PeerEraUptime` | `DoubleMap<BotIdHash, [u8;32], BoundedVec<(u64, u32)>>` | Peer 历史 Uptime 快照（滚动窗口） |

### 运营商
| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Operators` | `DoubleMap<AccountId, Platform, OperatorInfo>` | 运营商注册表 |
| `OperatorBots` | `DoubleMap<AccountId, Platform, BoundedVec<BotIdHash>>` | 运营商 Bot 列表 |
| `BotOperator` | `Map<BotIdHash, (AccountId, Platform)>` | Bot → 运营商映射 |
| `PlatformAppHashIndex` | `DoubleMap<Platform, [u8;32], AccountId>` | App 哈希反向索引（防重复） |
| `OperatorCount` | `u32` | 运营商总数 |

## 主要类型

### BotInfo
```rust
pub struct BotInfo<T: Config> {
    pub owner: T::AccountId,
    pub bot_id_hash: BotIdHash,
    pub public_key: [u8; 32],
    pub status: BotStatus,
    pub registered_at: BlockNumberFor<T>,
    pub node_type: NodeType,          // StandardNode / TeeNode / TeeNodeV2
    pub community_count: u32,
}
```

### AttestationRecord (V1)
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

### AttestationRecordV2 (三模式统一)
```rust
pub struct AttestationRecordV2<T: Config> {
    pub bot_id_hash: BotIdHash,
    pub primary_quote_hash: [u8; 32],        // TDX v4 或 SGX v3 Quote hash
    pub secondary_quote_hash: Option<[u8; 32]>,
    pub primary_measurement: [u8; 48],        // MRTD(48B) 或 MRENCLAVE(32B+16B pad)
    pub mrenclave: Option<[u8; 32]>,
    pub tee_type: TeeType,                    // Tdx / Sgx / TdxPlusSgx
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

### PeerEndpoint
```rust
pub struct PeerEndpoint<T: Config> {
    pub public_key: [u8; 32],                // Ed25519 Peer 公钥
    pub endpoint: BoundedVec<u8, T::MaxEndpointLen>,
    pub registered_at: BlockNumberFor<T>,
    pub last_seen: BlockNumberFor<T>,        // 最后心跳区块
}
```

### OperatorInfo
```rust
pub struct OperatorInfo<T: Config> {
    pub owner: T::AccountId,
    pub platform: Platform,
    pub platform_app_hash: [u8; 32],         // SHA256(api_id)
    pub name: BoundedVec<u8, T::MaxOperatorNameLen>,
    pub contact: BoundedVec<u8, T::MaxOperatorContactLen>,
    pub status: OperatorStatus,              // Active / Suspended / Deactivated
    pub registered_at: BlockNumberFor<T>,
    pub bot_count: u32,
    pub sla_level: u8,                       // 0-3
    pub reputation_score: u32,               // 初始 100
}
```

### CommunityBinding
```rust
pub struct CommunityBinding<T: Config> {
    pub community_id_hash: CommunityIdHash,
    pub platform: Platform,
    pub bot_id_hash: BotIdHash,
    pub bound_by: T::AccountId,
    pub bound_at: BlockNumberFor<T>,
}
```

## 事件

| 事件 | 说明 |
|------|------|
| `BotRegistered` | Bot 注册成功 |
| `PublicKeyUpdated` | 公钥已更换 |
| `BotDeactivated` | Bot 已停用 |
| `BotSuspended` | Bot 已暂停 |
| `BotReactivated` | Bot 已重新激活 |
| `CommunityBound` | 社区已绑定 |
| `CommunityUnbound` | 社区已解绑 |
| `UserPlatformBound` | 用户首次绑定平台身份 |
| `UserPlatformBindingUpdated` | 用户平台身份已覆盖（含 old_hash 审计线索） |
| `AttestationSubmitted` | TEE 证明已提交（Level 0） |
| `AttestationRefreshed` | 证明已刷新 |
| `AttestationExpired` | 证明已过期（自动降级） |
| `NonceIssued` | 证明 Nonce 已签发 |
| `MrtdApproved` / `MrtdRevoked` | MRTD 白名单审批/撤销 |
| `MrenclaveApproved` / `MrenclaveRevoked` | MRENCLAVE 白名单审批/撤销 |
| `ApiServerMrtdApproved` | API Server MRTD 已审批 |
| `PckKeyRegistered` | PCK 公钥已注册 |
| `DcapAttestationSubmitted` | DCAP 证明已提交（含 dcap_level + has_api_server） |
| `SgxAttestationSubmitted` | SGX 补充证明已提交 |
| `TeeAttestationSubmitted` | 统一入口 TEE 证明已提交（含 tee_type + dcap_level） |
| `PeerRegistered` / `PeerDeregistered` | Peer 注册/注销 |
| `PeerHeartbeat` | Peer 心跳 |
| `StalePeerReported` | 过期 Peer 已举报移除 |
| `OperatorRegistered` / `OperatorUpdated` / `OperatorDeregistered` | 运营商生命周期 |
| `OperatorSlaUpdated` | 运营商 SLA 等级已更新 |
| `BotAssignedToOperator` / `BotUnassignedFromOperator` | Bot 运营商分配/取消 |

## 错误

| 错误 | 说明 |
|------|------|
| `BotAlreadyRegistered` | Bot 已注册 |
| `BotNotFound` | Bot 不存在 |
| `NotBotOwner` | 不是 Bot 所有者 |
| `BotNotActive` | Bot 不活跃 |
| `BotAlreadyDeactivated` | Bot 已停用 |
| `MaxBotsReached` | 所有者 Bot 数量已满 |
| `CommunityAlreadyBound` | 社区已绑定到活跃 Bot |
| `CommunityNotBound` | 社区未绑定 |
| `MrtdNotApproved` | MRTD 未在白名单中 |
| `MrenclaveNotApproved` | MRENCLAVE 未在白名单中 |
| `AttestationNotFound` | 证明不存在 |
| `AttestationExpired` | 证明已过期 |
| `MrtdAlreadyApproved` | MRTD 已在白名单中 |
| `MrenclaveAlreadyApproved` | MRENCLAVE 已在白名单中 |
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
| `PckKeyAlreadyRegistered` | PCK 公钥已注册 |
| `ApiServerMrtdNotApproved` | API Server MRTD 未审批 |
| `ApiServerMrtdAlreadyApproved` | API Server MRTD 已在白名单 |
| `ApiServerReportDataMismatch` | API Server report_data 不匹配 |
| `DcapCertChainInvalid` | 证书链验证失败 |
| `DcapRootCaVerificationFailed` | Root CA 验证失败 |
| `DcapIntermediateCaVerificationFailed` | Intermediate CA 验证失败 |
| `PeerAlreadyRegistered` | Peer 已注册（相同公钥） |
| `PeerNotFound` | Peer 不存在 |
| `MaxPeersReached` | Peer 数量已满 |
| `EndpointEmpty` | 端点 URL 为空 |
| `PeerNotStale` | Peer 尚未过期 |
| `FreeTierNotAllowed` | Free 层级不允许使用此功能 |
| `OperatorAlreadyRegistered` | 运营商已注册 |
| `OperatorNotFound` | 运营商不存在 |
| `ApiIdHashAlreadyUsed` | app_hash 已被其他运营商使用 |
| `OperatorNameEmpty` | 运营商名称为空 |
| `MaxBotsPerOperatorReached` | 运营商下 Bot 数量已满 |
| `BotAlreadyAssigned` | Bot 已分配给运营商 |
| `BotNotAssigned` | Bot 未分配给运营商 |
| `OperatorNotActive` | 运营商不活跃 |
| `OperatorHasActiveBots` | 运营商下仍有 Bot，无法注销 |
| `InvalidSlaLevel` | SLA 等级无效（0-3） |
| `ExpiryQueueFull` | 证明过期队列已满（1000 上限） |
| `MrtdNotFound` | MRTD 不在白名单中（撤销时） |
| `MrenclaveNotFound` | MRENCLAVE 不在白名单中（撤销时） |

## 配置参数

| 参数 | 说明 |
|------|------|
| `MaxBotsPerOwner` | 单个所有者最大 Bot 数 |
| `MaxPlatformsPerCommunity` | 单个社区最大平台绑定数 |
| `MaxPlatformBindingsPerUser` | 用户最大平台绑定数 |
| `AttestationValidityBlocks` | TEE 证明有效期（区块数） |
| `AttestationCheckInterval` | 过期扫描间隔（区块数） |
| `MaxQuoteLen` | TDX/SGX Quote 最大字节长度 |
| `MaxPeersPerBot` | 单个 Bot 最大 Peer 数 |
| `MaxEndpointLen` | Peer 端点 URL 最大长度 |
| `PeerHeartbeatTimeout` | Peer 心跳过期阈值（区块数） |
| `MaxOperatorNameLen` | 运营商名称最大长度 |
| `MaxOperatorContactLen` | 运营商联系方式最大长度 |
| `MaxBotsPerOperator` | 单个运营商最大 Bot 数 |
| `MaxUptimeEraHistory` | Peer Uptime 历史保留 Era 数 |
| `Subscription` | 订阅层级查询（`SubscriptionProvider`，Tier gate） |

## Hooks

- **`on_initialize`**：每 `AttestationCheckInterval` 个区块，从 `AttestationExpiryQueue` 头部弹出已过期条目（最多 10 条/块），验证证明确实已过期后移除记录并将 Bot 降级为 `StandardNode`。刷新导致的陈旧队列条目自动跳过。

## Trait 实现

### BotRegistryProvider\<AccountId\>

供 ceremony / community / consensus / subscription / ads 子 pallet 查询：

- `is_bot_active` / `is_tee_node` / `has_dual_attestation` / `is_attestation_fresh`
- `bot_owner` / `bot_public_key` / `peer_count` / `bot_operator`

### PeerUptimeRecorder

由 consensus pallet 的 `on_era_end` 调用：

- `record_era_uptime(era)` — 将 `PeerHeartbeatCount` drain 快照到 `PeerEraUptime`（滚动窗口），batch limit 500 条/次

## 公共查询方法

- `is_bot_active` / `is_tee_node` / `has_dual_attestation` / `is_attestation_fresh`
- `bot_owner` / `bot_public_key` / `peer_count` / `get_peers`
- `bot_operator_account` / `bot_operator_full` / `operator_info` / `operator_bots`
- `peer_heartbeat_count` / `peer_era_uptime`

## 子模块

- **`dcap.rs`**：TDX Quote v4 + SGX Quote v3 完整解析器 + ECDSA P-256 签名验证（`p256` crate，`no_std` 兼容），支持 Level 2/3/4 验证

## 相关模块

- [primitives/](../primitives/) — 共享类型定义（BotStatus, NodeType, TeeType, Platform 等）
- [ceremony/](../ceremony/) — RA-TLS 仪式（依赖 BotRegistryProvider）
- [consensus/](../consensus/) — 节点共识（依赖 BotRegistryProvider + PeerUptimeRecorder）
- [subscription/](../subscription/) — 订阅管理（依赖 BotRegistryProvider）
- [community/](../community/) — 社区管理（依赖 BotRegistryProvider）
