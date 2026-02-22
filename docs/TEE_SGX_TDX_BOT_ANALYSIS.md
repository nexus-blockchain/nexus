# Multi-TEE Active-Active + TDX+SGX 隐私 Bot 架构设计

> 日期: 2026-02-21
> 版本: v3.0 — 统一架构设计 (全面重整版)
>
> **最终方案: Multi-TEE Active-Active + TDX 全内存加密 + 最小化 SGX Enclave (8 ecall) + Shamir 跨节点密钥恢复 + RA-TLS 安全仪式 + 链上仪式审计**

---

## 目录

1. [现有架构回顾](#1-现有架构回顾)
2. [SGX 与 TDX 技术概述](#2-sgx-与-tdx-技术概述)
3. [核心架构设计](#3-核心架构设计) — 3.1~3.10
4. [挖矿奖励分配](#4-挖矿奖励分配) — 普通节点 vs TEE 节点
5. [跨节点密钥恢复](#5-跨节点密钥恢复) — Shamir's Secret Sharing
6. [安全仪式与链上审计](#6-安全仪式与链上审计) — RA-TLS + pallet-ceremony-audit
7. [合理性分析](#7-合理性分析)
8. [可行性分析](#8-可行性分析)
9. [现有代码迁移评估](#9-现有代码迁移评估)
10. [与现有方案对比](#10-与现有方案对比)
11. [风险评估](#11-风险评估)
12. [实施路线图](#12-实施路线图)
13. [结论](#13-结论)

---

## 1. 现有架构回顾

### 1.1 三层架构

```
┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐
│   nexus-agent    │────▶│   nexus-node     │────▶│   Substrate      │
│  (本地代理)       │     │  (验证节点 ×N)    │     │   Runtime        │
│ • TG/DC Webhook  │     │ • 四层验证        │     │ • bot-consensus  │
│ • Ed25519 签名   │     │ • Gossip 共识     │     │ • bot-registry   │
│ • 确定性多播      │     │ • 规则引擎        │     │ • bot-group-mgmt │
│ • 本地快速路径    │     │ • Leader 执行     │     │                  │
│ • 群配置管理      │     │ • 链上提交        │     │                  │
└──────────────────┘     └──────────────────┘     └──────────────────┘
```

### 1.2 核心职责分离

| 组件 | 职责 | 安全关切 |
|------|------|---------|
| **nexus-agent** | 持有 BOT_TOKEN, 接收平台事件, 签名+多播, 执行 TG/DC API | BOT_TOKEN 明文, Ed25519 私钥本地文件 |
| **nexus-node** (×N) | 四层验证, M/K Gossip 共识, 规则引擎, Leader 选举+执行 | 需多节点部署才有安全性 |
| **链上 Pallet** | 节点注册/质押, Bot 注册, 动作日志, Equivocation 惩罚 | 链上数据公开透明 |

### 1.3 现有安全模型

- **信任假设**: Agent 被信任, Node 通过 2/3 共识防止单点作恶
- **防篡改**: Agent 签名 → Node 验签 → M/K 共识 → Leader 执行
- **弱点**: Agent 单点信任, BOT_TOKEN 明文, Node 运营者可见消息内容

### 1.4 现有代码规模

| 组件 | 代码行数 | 测试数 |
|------|---------|--------|
| nexus-agent | ~8,500 | 127 |
| nexus-node | ~12,000 | 132 |
| 链上 Pallet (×3) | ~3,000 | ~49 |
| **合计** | **~23,500** | **~308** |

---

## 2. SGX 与 TDX 技术概述

### 2.1 Intel SGX

| 特性 | 说明 |
|------|------|
| **隔离粒度** | 进程级 Enclave (用户态飞地) |
| **内存限制** | EPC: SGX1 128MB, SGX2 可扩展 |
| **可信计算基** | CPU 硬件 + Enclave 代码 (不信任 OS/VMM) |
| **编程模型** | 需 Enclave 分区 (ecall/ocall), Teaclave/Gramine |
| **当前状态** | 桌面已移除, **服务器 Xeon 保留** |

### 2.2 Intel TDX

| 特性 | 说明 |
|------|------|
| **隔离粒度** | VM 级 Trust Domain |
| **内存** | 无 EPC 限制, TME-MK 全内存加密 |
| **编程模型** | **无需代码改造** — 标准 Linux 直接运行 |
| **性能** | ~2-5% 开销, 几乎原生 |
| **当前状态** | Xeon 4th Gen+, **Azure/GCP/阿里云 GA** |

### 2.3 本方案组合策略: TDX + 最小化 SGX

```
┌─────────────────────────────────────────────────┐
│                TDX Trust Domain                   │
│  ┌─────────────────────────────────────────────┐ │
│  │          nexus-tee-bot (Rust)                │ │
│  │  • 完整 bot 逻辑 (无需代码分区)              │ │
│  │  • TG/DC API / 规则引擎 / 群管逻辑          │ │
│  │  ┌──────────────────────────────┐           │ │
│  │  │  SGX Enclave (~500行, 8 ecall)│           │ │
│  │  │  • Ed25519 密钥密封/签名      │           │ │
│  │  │  • BOT_TOKEN 解密            │           │ │
│  │  │  • Shamir share 导入/导出     │           │ │
│  │  │  密钥永不离开 Enclave          │           │ │
│  │  └──────────────────────────────┘           │ │
│  └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

- **TDX**: VM 级全内存加密 → 保护消息、逻辑、状态
- **SGX** (~500 行, 8 ecall): 即使 Guest OS 被攻破密钥仍安全
- **Multi-TEE Active-Active** 消除单点故障
- **Shamir** 解决跨节点密钥恢复

---

## 3. 核心架构设计

### 3.1 架构概览

```
现有: Agent(1) ──multicast──▶ Node(N) ──gossip──▶ consensus ──▶ Chain
新:   nexus-tee-bot(×M, TDX+SGX) ──Active-Active──▶ Chain (去重)
```

**核心变化:** 合并为单一进程 / TDX 全加密 / SGX 8 ecall / Multi-TEE / Shamir 密钥恢复 / RA-TLS 安全仪式

### 3.2 Multi-TEE Active-Active 部署架构

```
  Telegram ──▶ LB (chat_id 哈希) ──▶ TEE Bot #1/#2/... (TDX+SGX)
                                           │ subxt
                                           ▼
                                  Substrate Runtime
                                  ├── pallet-bot-registry (NodeType + Quote)
                                  ├── pallet-bot-consensus (去重 + TEE 加权奖励)
                                  ├── pallet-bot-group-mgmt (NodeRequirement)
                                  └── pallet-ceremony-audit (仪式 + 白名单)
```

### 3.3 单实例内部架构

```
┌─── TDX Trust Domain ────────────────────────────────┐
│  Platform Adapters → Rule Engine → Executor → Chain  │
│  Local Processor (快速路径)                           │
│  ┌─ SGX Enclave (8 ecall) ────────────────────────┐ │
│  │ init_keys / sign / seal / auth_header / quote   │ │
│  │ import_shares / export_share / reshare          │ │
│  └─────────────────────────────────────────────────┘ │
│  Attestor: TDX Quote (MRTD) + SGX Quote (MRENCLAVE)  │
└──────────────────────────────────────────────────────┘
```

### 3.4 安全层级模型

| 攻击场景 | 现有方案 | TDX-Only | **本方案 (TDX+SGX)** |
|---------|---------|---------|---------------------|
| Host/VMM 读内存 | ❌ 可读 | ✅ 加密 | ✅ 双层加密 |
| Guest OS 内核漏洞 | ❌ 全暴露 | ❌ 全暴露 | ✅ **SGX 密钥仍安全** |
| 运营者偷看消息 | ❌ 明文 | ✅ 加密 | ✅ 加密 |
| BOT_TOKEN 泄露 | ❌ 明文 | ⚠️ TD 内明文 | ✅ **Enclave 密封** |
| 签名密钥被盗 | ❌ 文件系统 | ⚠️ TD 内存 | ✅ **仅 Enclave 内** |
| 代码被篡改 | ❌ 可改 | ✅ MRTD | ✅ MRTD+MRENCLAVE |
| 单点故障 | ⚠️ ≥3 Node | ❌ 单实例 | ✅ **Multi-TEE** |

### 3.5 信任模型

| 维度 | 现有方案 | 本方案 |
|------|---------|--------|
| **信任根** | 多节点 2/3 共识 | Intel CPU 硬件 (TDX+SGX) |
| **BOT_TOKEN** | Agent 明文 | SGX 密封 + Shamir 分片 |
| **代码完整性** | 无保证 | MRTD + MRENCLAVE 双证明 |
| **数据保密性** | Node 运营者可见 | TDX 加密内存 |
| **纵深防御** | 无 | Guest OS 被攻破 → SGX 仍安全 |
| **高可用** | ≥3 Node Gossip | Multi-TEE + LB 漂移 |

### 3.6 链上注册与节点区分

```rust
pub enum NodeType {
    StandardNode,
    TeeNode {
        tdx_quote: BoundedVec<u8, MaxQuoteSize>, mrtd: [u8; 48],
        sgx_quote: BoundedVec<u8, MaxQuoteSize>, mrenclave: [u8; 32],
        attested_at: BlockNumber, expires_at: BlockNumber,
    },
}

fn register_tee_bot(origin, bot_id_hash, tdx_quote, sgx_quote) -> DispatchResult {
    let tdx = parse_tdx_quote(&tdx_quote)?;
    ensure!(ApprovedMrtd::<T>::contains_key(&tdx.mrtd));
    let sgx = parse_sgx_quote(&sgx_quote)?;
    ensure!(ApprovedMrenclave::<T>::contains_key(&sgx.mrenclave));
    ensure!(verify_pubkey_binding(&sgx, &bot_public_key));
    RegisteredBots::<T>::insert(bot_id_hash, BotInfo { node_type: NodeType::TeeNode { .. }, .. });
    Ok(())
}
```

### 3.7 群主节点准入策略

```rust
pub enum NodeRequirement { Any, TeeOnly, MinTee(u32) }
```

| 群类型 | 策略 | 原因 |
|--------|------|------|
| 普通闲聊 | `Any` | 无隐私需求 |
| DAO 治理 | `TeeOnly` | 投票/提案保密 |
| 金融/交易 | `TeeOnly` | 密钥 = 资金安全 |
| 大型社区 | `MinTee(2)` | 隐私 + 高可用 |

### 3.8 Multi-TEE 去重与持续验证

```rust
pub type ProcessedSequences<T> = StorageMap<_, Blake2_128Concat, (BotIdHash, u64), BlockNumberFor<T>>;
```

| 机制 | 说明 |
|------|------|
| Quote 24h 过期 | 自动降级 Unverified |
| 动作附带 Report | TDX Report 签名 |
| Heartbeat | RTMR 链上比对 |
| on_initialize | 扫描过期 → 禁止服务 TeeOnly 群 |

### 3.9 SGX Enclave 8 个 ecall 接口

```rust
// 核心密钥操作 (5 个)
fn ecall_init_keys(sealed: &[u8]) -> SgxResult<PublicKey>;
fn ecall_sign(data: &[u8]) -> SgxResult<[u8; 64]>;
fn ecall_seal_secret(plain: &[u8]) -> SgxResult<Vec<u8>>;
fn ecall_get_auth_header() -> SgxResult<String>;
fn ecall_generate_quote(nonce: &[u8; 64]) -> SgxResult<Vec<u8>>;
// Shamir 跨节点恢复 (3 个, 见 §5)
fn ecall_import_shares(shares: &[EncryptedShare]) -> SgxResult<PublicKey>;
fn ecall_export_share(req: &ShareRequest) -> SgxResult<ShareResponse>;
fn ecall_reshare(k: u8, n: u8, peers: &[Quote]) -> SgxResult<Vec<EncryptedShare>>;
```

| 接口 | 频率 | EPC | 开销 |
|------|------|-----|------|
| init_keys | 启动 1 次 | ~64B | 一次性 |
| sign | ~100/s | ~256B | ~15µs |
| seal_secret | 极少 | ~4KB | 轮换时 |
| get_auth_header | ~50/s | ~256B | ~10µs |
| generate_quote | 每 24h | ~8KB | 一次性 |
| import_shares | 启动 1 次 | ~16KB | 一次性 |
| export_share | 极少 | ~8KB | 新节点时 |
| reshare | 极少 | ~16KB | 节点变更 |
| **合计** | | **< 1MB** | **远低于 EPC 128MB** |

### 3.10 双证明体系

```
TDX Quote → MRTD (OS+应用, 度量粗)
SGX Quote → MRENCLAVE (Enclave ~500 行, 度量细, 极稳定)
优势: MRTD 随 OS 补丁变化, SGX Quote 提供额外稳定锚点
```

---

## 4. 挖矿奖励分配: 普通节点 vs TEE 节点

### 4.1 分配原则

1. **TEE 节点获更高收益** — 补偿更高硬件成本 (~$15-25 vs ~$5-10) + 奖励安全贡献
2. **普通节点保留参与权** — 渐进迁移, 不立即抛弃
3. **群策略驱动** — `NodeRequirement` 直接影响奖励池分配
4. **链上透明** — 所有权重因子链上可验证

### 4.2 节点权重模型

```rust
fn compute_effective_weight(node: &NodeInfo) -> u128 {
    let base = node.reputation as u128 * node.uptime_factor as u128
        * node.leader_bonus as u128 / 1_000_000_000_000;
    let tee_factor = match &node.node_type {
        NodeType::TeeNode { mrenclave, .. } => {
            let mut f = TEE_REWARD_MULTIPLIER; // 15000 = 1.5x
            if mrenclave.is_some() { f += SGX_ENCLAVE_BONUS; }      // +1000
            if node.quote_is_fresh() { f += ATTESTATION_FRESHNESS_BONUS; } // +500
            f
        },
        NodeType::StandardNode => 10_000,
    };
    base * tee_factor as u128 / 10_000
}
```

### 4.3 奖励参数与有效倍数

| 参数 | 值 | 说明 |
|------|------|------|
| **TEE_REWARD_MULTIPLIER** | 15000 (1.5x) | TEE 基础倍数 |
| **SGX_ENCLAVE_BONUS** | 1000 (+10%) | SGX 双证明额外奖励 |
| **ATTESTATION_FRESHNESS_BONUS** | 500 (+5%) | Quote 持续有效奖励 |
| **TEE_EXCLUSIVE_BONUS** | 2000 (+20%) | 服务 TeeOnly 群额外奖励 |
| **MaxRewardShare** | 30% | 单节点上限 (防寡头) |

| 节点类型 | 合计倍数 |
|---------|---------|
| 普通节点 | **1.0x** |
| TEE (TDX-Only) | **1.55x** |
| TEE (TDX+SGX) | **1.65x** |

### 4.4 按群策略的奖励池分流

```
每 Era (1天) 总奖励 = 订阅收入(80%) + 通胀铸币
  TeeOnly 群收入 → 仅 TEE 节点 (独占池)
  MinTee/Any 群收入 + 通胀 → 所有节点 (TEE 加权, 混合池)
```

```rust
fn distribute_era_rewards(era: EraIndex) {
    let mut tee_pool: Balance = 0;
    let mut mixed_pool: Balance = 0;
    for sub in active_subscriptions() {
        match GroupNodeRequirement::<T>::get(sub.group_id) {
            NodeRequirement::TeeOnly => tee_pool += sub.fee,
            _ => mixed_pool += sub.fee,
        }
    }
    mixed_pool += T::InflationPerEra::get();
    distribute_weighted(tee_pool, tee_nodes_only());
    distribute_weighted(mixed_pool, all_active_nodes());
}
```

### 4.5 计算示例

```
1 Era 100 NEX (TeeOnly 群 20, 混合池 80), 3 节点:
  A (TEE+SGX, 权重 16335): 46.0 NEX (46%)
  B (TEE+SGX, 权重 12540): 35.3 NEX (35%)
  C (普通,    权重 8820):  18.7 NEX (19%)
```

### 4.6 链上 Storage

```rust
pub type TeeRewardMultiplier<T> = StorageValue<_, u32, ValueQuery>;  // 15000
pub type SgxEnclaveBonus<T> = StorageValue<_, u32, ValueQuery>;      // 1000
pub type GroupNodeRequirement<T> = StorageMap<_, Blake2_128Concat, GroupId, NodeRequirement, ValueQuery>;
pub type NodeTeeStatus<T> = StorageMap<_, Blake2_128Concat, T::AccountId, TeeStatusInfo>;
```

### 4.7 渐进激励时间表

| Phase | 时间 | TEE 倍数 | 普通节点影响 |
|-------|------|---------|------------|
| 0 | 0-6 月 | 1.2x | -10~15% |
| 1 | 6-12 月 | 1.5x | -20~35% |
| 2 | 12-24 月 | 2.0x | -40~50% |
| 3 | 24+ 月 | 2.5x | -50~60% |

> 治理可通过提案调整 `TeeRewardMultiplier`。

---

## 5. 跨节点密钥恢复: Shamir's Secret Sharing

### 5.1 问题: SGX 密封无法跨节点解密

`Seal Key = KDF(CPU_Root_Key, MRENCLAVE)` — 不同 CPU 的 Root Key 不同, TEE Node #1 密封的数据 Node #2 无法解密。

### 5.2 方案: 分布式密钥共享

BOT_TOKEN + Ed25519 私钥拆分为 N 份, 任意 K 份可重构。

```
初始化 (RA-TLS 仪式, 见 §6):
  密钥 → Shamir Split(K=2,N=3) → 各 TEE Node SGX seal

正常启动:
  unseal 本地 share → 互验 Quote 获取 peer share → K 份重构 → Enclave 内存

故障恢复:
  新节点 → SGX Quote → 请求 K 个 peer share → 重构 → seal 新 share
```

### 5.3 TEE 间互验证协议

```rust
struct ShareRequest { requester_quote: Vec<u8>, requester_pubkey: [u8; 32], nonce: [u8; 32] }
struct ShareResponse { encrypted_share: Vec<u8>, responder_quote: Vec<u8> }

fn ecall_handle_share_request(req: &ShareRequest) -> SgxResult<ShareResponse> {
    let parsed = verify_sgx_quote(&req.requester_quote)?;
    ensure!(parsed.mrenclave == APPROVED_MRENCLAVE);
    let (my_pk, shared_secret) = ecdh_agree(&req.requester_pubkey)?;
    let encrypted = aes_gcm_encrypt(&shared_secret, &local_share)?;
    Ok(ShareResponse { encrypted_share: encrypted, responder_quote: generate_quote_with_data(&my_pk)? })
}
```

**安全保证:** MRENCLAVE 验证 + ECDH+AES-GCM 加密 + Quote 绑定公钥防 MITM + nonce 防重放

### 5.4 ecall 实现

```rust
fn ecall_import_shares(shares: &[EncryptedShare]) -> SgxResult<PublicKey> {
    let decrypted: Vec<Share> = shares.iter()
        .map(|s| ecdh_decrypt(&s.session_key, &s.data)).collect::<Result<_,_>>()?;
    let bot_token = shamir::recover(&decrypted, SECRET_BOT_TOKEN)?;
    let ed25519_sk = shamir::recover(&decrypted, SECRET_SIGNING_KEY)?;
    ENCLAVE_STATE.lock().bot_token = Some(bot_token);
    ENCLAVE_STATE.lock().signing_key = Some(ed25519_sk);
    Ok(ed25519_sk.public_key())
}

fn ecall_reshare(new_k: u8, new_n: u8, peer_quotes: &[Quote]) -> SgxResult<Vec<EncryptedShare>> {
    let secrets = ENCLAVE_STATE.lock();
    encrypt_shares_for_peers(shamir::split(&secrets.bot_token, new_k, new_n)?, peer_quotes)
}
```

### 5.5 参数配置

| 部署规模 | K | N | 容错 |
|---------|---|---|------|
| 小型 | 2 | 3 | 1 |
| 中型 | 3 | 5 | 2 |
| 大型 | 5 | 9 | 4 |

### 5.6 与 Multi-TEE 集成

```
启动: TDX → SGX → unseal share → 互验 Quote → K 份重构 → 双 Quote 上链 → 服务
故障: LB 检测宕机 → 流量漂移 → 存活节点已有完整密钥 → 无缝接管
```

**关键: Shamir 解决 "持久化和恢复"; 运行时每节点独立签名, 无需通信。**

### 5.7 安全性

| 威胁 | 防御 |
|------|------|
| 非 TEE 请求 share | MRENCLAVE 拒绝 |
| 传输截获 | ECDH + AES-GCM |
| MITM | Quote 绑定 ECDH 公钥 |
| 重放 | nonce + 时间戳 |
| K-1 共谋 | 少于 K 无法重构 |
| Enclave 转储 | SGX 硬件防护 |
| share 磁盘泄露 | SGX Sealed 绑定 CPU |

---

## 6. 安全仪式与链上审计

### 6.1 威胁模型

| 威胁 | 传统后端 | RA-TLS Enclave |
|------|---------|---------------|
| 运营者可见 Token | ❌ 可见 | ✅ **不可见** |
| 前端篡改 | ❌ 无防护 | ✅ Quote 验证 |
| 中间件截获 | ❌ TLS 终止在 Nginx | ✅ TLS 终止在 Enclave |
| 数据库泄露 | ❌ 存储明文 | ✅ 不经数据库 |

### 6.2 RA-TLS 端到端方案

```
用户浏览器 ── RA-TLS ──▶ Nginx (TCP 透传) ──▶ SGX Enclave (TLS 终止)
  1. TLS 握手, 证书含 SGX Quote
  2. JS 验证 MRENCLAVE 匹配审计版本
  3. 用户输入 Token → 仅 Enclave 解密
  4. Enclave: Shamir Split → 分发 share → zeroize → 链上注册
```

### 6.3 前端验证 + Ceremony Enclave

```javascript
// 前端
async function verifyEnclaveAndSubmit(botToken) {
  const { quote, mrenclave, certificate } = await fetch('/attestation').then(r => r.json());
  if (!await verifySgxQuote(quote)) throw new Error('Quote 无效');
  if (mrenclave !== AUDITED_MRENCLAVE) throw new Error('MRENCLAVE 不匹配');
  await fetch('/submit', { method: 'POST', body: JSON.stringify({ bot_token: botToken }) });
  botToken = null;
}
```

```rust
// Ceremony Enclave (~500 行)
fn ecall_ceremony_server() -> SgxResult<()> {
    let (tls_sk, tls_pk) = generate_tls_keypair()?;
    let cert = create_ra_tls_cert(tls_sk, tls_pk, generate_sgx_quote(&sha256(&tls_pk))?)?;
    let listener = enclave_listen(443, TlsConfig::new(cert, tls_sk))?;
    loop {
        let conn = listener.accept()?;
        match conn.path() {
            "/attestation" => { /* 返回 Quote + MRENCLAVE */ },
            "/submit" => {
                let mut token = conn.read_json::<SubmitRequest>()?.bot_token;
                let (sk, pk) = ed25519_keygen();
                let shares = shamir::split(&encode_secrets(&token, &sk), K, N)?;
                // 验证各 TEE 节点 Quote → ECDH 分发 share
                for (i, node) in tee_nodes.iter().enumerate() {
                    verify_sgx_quote(&node.quote)?;
                    send_encrypted(node.endpoint, &shares[i], &ecdh_agree(&node.pubkey)?)?;
                }
                zeroize(&mut token); zeroize(&mut sk); zeroize(&mut shares);
                conn.respond_json(&CeremonyResult { public_key: pk, .. })?;
            },
            _ => {},
        }
    }
}
```

### 6.4 Nginx TCP 透传

```nginx
stream {
    server { listen 443; proxy_pass 127.0.0.1:8443; }
}
```

### 6.5 链上仪式审计 Pallet (pallet-ceremony-audit)

#### Storage

```rust
pub struct CeremonyRecord<AccountId, BlockNumber> {
    pub ceremony_mrenclave: [u8; 32],
    pub ceremony_quote: BoundedVec<u8, MaxQuoteSize>,
    pub participants: BoundedVec<AccountId, MaxParticipants>,
    pub participant_enclaves: BoundedVec<[u8; 32], MaxParticipants>,
    pub k: u8, pub n: u8,
    pub ceremony_hash: H256,
    pub bot_public_key: [u8; 32],
    pub initiator: AccountId,
    pub created_at: BlockNumber,
    pub status: CeremonyStatus,
}

pub enum CeremonyStatus {
    Active,
    Superseded { replaced_by: H256 },
    Revoked { reason: BoundedVec<u8, MaxReasonLen>, revoked_at: BlockNumber },
}

pub type ActiveCeremony<T> = StorageMap<_, Blake2_128Concat, H256, CeremonyRecord>;
pub type CeremonyHistory<T> = StorageMap<_, Blake2_128Concat, H256, BoundedVec<CeremonyRecord, MaxHistory>, ValueQuery>;
pub type ApprovedCeremonyEnclave<T> = StorageMap<_, Blake2_128Concat, [u8; 32], CeremonyEnclaveInfo>;
```

#### Extrinsic (5 个)

| 调用 | 权限 | 说明 |
|------|------|------|
| `record_ceremony` | 签名 | 记录仪式: 验证 Quote + Shamir 参数 + 参与节点 TEE 状态 |
| `revoke_ceremony` | 治理 | 撤销仪式: Bot 暂停, 需 re-ceremony |
| `approve_ceremony_enclave` | 治理 | 添加 Enclave 到白名单 |
| `remove_ceremony_enclave` | 治理 | 移除 Enclave (版本淘汰/漏洞) |
| `verify_ceremony` | 任何人 | 查询验证状态 |

#### 仪式生命周期

```
approve_enclave → record_ceremony → Active
  Active → re-ceremony → Superseded
  Active → revoke → Revoked (Bot 暂停)
```

#### 触发 re-ceremony

| 场景 | 触发者 |
|------|--------|
| Enclave 漏洞 | 治理 |
| 活跃节点 < K | 自动检测 |
| 用户更换 Token | 用户 |
| 定期轮换 (180 天) | 治理策略 |
| 安全事件 | 技术委员会 |

#### 自动风险检测

```rust
fn on_initialize(n: BlockNumberFor<T>) -> Weight {
    // 每 CeremonyCheckInterval 检查:
    // 活跃节点 < K → CeremonyAtRisk 事件
    // 仪式过期 → CeremonyExpired 事件
}
```

#### RPC 查询

```rust
pub fn ceremony_health(bot_id_hash) -> CeremonyHealth; // Active/NoCeremony/needs_renewal
pub fn ceremony_history(bot_id_hash) -> Vec<CeremonyRecord>;
pub fn approved_enclaves() -> Vec<([u8; 32], CeremonyEnclaveInfo)>;
```

#### 信任链闭环

```
开源审计 → 报告哈希上链 → 确定性编译 MRENCLAVE → 治理审批白名单
→ RA-TLS 验证 → 仪式执行 → 链上记录 → 持续验证 → 异常响应
每步链上不可篡改, 任何人可独立验证。
```

---

## 7. 合理性分析

### 7.1 TEE 替代多节点共识 ✅

| 攻击场景 | 多节点方案 | 本方案 |
|----------|-----------|--------|
| 单节点作恶 | 需 ≥1/3 共谋 | N/A — 硬件保证 |
| 运营者偷看 | ❌ 明文 | ✅ TDX 加密 |
| 运营者篡改 | ❌ 可改 | ✅ 双证明 |
| BOT_TOKEN 泄露 | ❌ 明文 | ✅ SGX 密封 + Shamir |
| OS 被攻破 | ❌ 全暴露 | ✅ SGX 仍安全 |
| 单点故障 | ✅ 多节点 | ✅ Multi-TEE |

### 7.2 架构简化

| 维度 | 多节点方案 | 本方案 |
|------|-----------|--------|
| 组件类型 | 2 种 | 1 种 |
| 网络协议 | 4 种 | 1 种 |
| 共识 | M/K Gossip | 无需 |
| Leader | Round-Robin | 无需 |
| 代码量 | ~23,500 行 | ~9,840 行 (**-58%**) |

### 7.3 行业验证

Oasis Network / Secret Network / Phala Network / Marlin Protocol / Flashbots SUAVE / Automata Network

### 7.4 链上组件角色

| Pallet | 现有 | 新 |
|--------|------|-----|
| **bot-registry** | Bot+公钥 | + NodeType + Quote + 白名单 |
| **bot-group-mgmt** | 动作日志 | + NodeRequirement |
| **bot-consensus** | 多节点共识+奖励 | 简化: 去重 + TEE 加权奖励 |
| **ceremony-audit** | 无 | **新增**: 仪式 + 白名单 + 风险检测 |

---

## 8. 可行性分析

### 8.1 技术可行性

| 技术点 | 状态 | 说明 |
|--------|------|------|
| TDX 运行 Rust | ✅ | 标准 Linux, 100% crate 兼容 |
| SGX Enclave | ✅ | 8 ecall ~500 行, EPC < 1MB |
| 双证明上链 | ✅ | 链下 DCAP + 链上存证 |
| Shamir 跨节点恢复 | ✅ | ECDH + Quote 互验 |
| RA-TLS 安全仪式 | ✅ | Gramine/Teaclave 原生支持 |
| 链上仪式审计 | ✅ | 标准 Substrate Pallet |
| Multi-TEE 去重 | ✅ | ProcessedSequences |

### 8.2 云部署可行性

| 云厂商 | TDX+SGX | 状态 | 实例 |
|--------|---------|------|------|
| Azure | ✅ DCesv5 | GA | Confidential VMs |
| GCP | ✅ C3 | GA | Confidential VMs |
| 阿里云 | ✅ g8i | GA | 加密计算实例 |
| AWS | ⚠️ Nitro | 非 TDX | 不直接支持 |

### 8.3 成本对比

```
现有: 1× Agent + 3× Node VPS = ~$40/月
本方案: 2× Azure DCesv5 = ~$30-50/月
大规模: 3× DCesv5 共享 10 Bot = ~$12/Bot/月
```

### 8.4 性能

| 操作 | 现有方案 | 本方案 |
|------|---------|--------|
| 事件→执行 | 300-1000ms | 5-20ms (**15-200× 更快**) |
| 内存 | Agent 50MB + Node 100MB×N | TEE Bot ~85MB |

---

## 9. 现有代码迁移评估

### 9.1 可直接复用

| 模块 | 来源 | 行数 |
|------|------|------|
| `platform/` (TG+DC) | agent | ~600 |
| `executor.rs` (TG API) | agent | ~1,200 |
| `discord_executor.rs` | agent | ~1,000 |
| `gateway/discord.rs` | agent | ~400 |
| `rule_engine.rs` | node | ~3,500 |
| `local_processor.rs` | agent | ~1,200 |
| `local_store.rs` | agent | ~400 |
| `group_config.rs` | agent | ~900 |
| `config.rs` | agent | ~500 |
| `signer.rs` | agent | ~400 |
| `types.rs` | 两者 | ~700 |
| **可复用合计** | | **~10,800** |

### 9.2 可删除

| 模块 | 行数 | 原因 |
|------|------|------|
| `multicaster.rs` | ~320 | 无需多播 |
| `gossip/` (4 文件) | ~2,000 | 无需 Gossip |
| `verifier.rs` | ~290 | 无需四层验证 |
| `leader.rs` | ~450 | 无需 Leader |
| `chain_submitter.rs` | ~400 | 简化 |
| `chain_cache.rs` | ~600 | 简化 |
| `api.rs` / `rate_limiter.rs` | ~150 | 合并 |
| **可删除合计** | **~4,210** | |

### 9.3 需要新增

| 模块 | 功能 | 行数 |
|------|------|------|
| `enclave/src/lib.rs` | SGX 8 ecall | ~500 |
| `enclave_bridge.rs` | ecall 封装 | ~200 |
| `attestation.rs` | TDX+SGX 双 Quote | ~350 |
| `sealed_storage.rs` | 密钥密封 | ~200 |
| `shamir_protocol.rs` | Shamir 分片+互验+恢复 | ~400 |
| `ra_tls_ceremony.rs` | RA-TLS 仪式服务端 | ~500 |
| `chain_client.rs` | subxt 直接提交 | ~250 |
| `main.rs` | 统一入口 | ~200 |
| `pallet-ceremony-audit` | 链上仪式审计 | ~750 |
| **新增合计** | | **~3,350** |

### 9.4 迁移后代码量

```
复用:   ~10,800 行 (改造部分 ~1,500 行)
删除:   ~4,210 行
新增:   ~3,350 行 (含 SGX Enclave + Shamir + RA-TLS + Ceremony Pallet)
────────────────────
最终:   ~9,840 行 (vs 现有 23,500 行, 减少 58%)
```

---

## 10. 与现有方案对比

### 10.1 综合对比矩阵

| 维度 | 现有方案 | 本方案 | 胜出 |
|------|---------|--------|------|
| **数据保密性** | ❌ Node 明文 | ✅ TDX 加密 | 本方案 |
| **密钥保护** | ❌ Agent 明文 | ✅ SGX 密封 + Shamir | 本方案 |
| **纵深防御** | ❌ 无 | ✅ OS 攻破密钥仍安全 | 本方案 |
| **代码完整性** | ❌ 可篡改 | ✅ 双证明 | 本方案 |
| **高可用** | ✅ ≥3 Node | ✅ Multi-TEE | 平 |
| **延迟** | ❌ 300-1000ms | ✅ 5-20ms | 本方案 |
| **部署复杂度** | ❌ 1+N 进程 | ✅ M 同构实例 | 本方案 |
| **运营成本** | ❌ ~$40/月 | ✅ ~$30-50/月 | 本方案 |
| **代码量** | ❌ ~23,500 | ✅ ~9,840 | 本方案 |
| **密钥恢复** | ❌ 无 | ✅ Shamir K/N | 本方案 |
| **仪式审计** | ❌ 无 | ✅ 链上不可篡改 | 本方案 |
| **硬件依赖** | ✅ 任意 | ❌ Intel TDX+SGX | 现有 |
| **供应商锁定** | ✅ 无 | ⚠️ Intel + 云厂商 | 现有 |

### 10.2 隐私评分

| 维度 | 现有 | TDX-Only | **本方案** |
|------|------|---------|-----------|
| 运行时数据保密 | 30% | 95% | **95%** |
| 密钥保护强度 | 40% | 80% | **98%** |
| 纵深防御 | 0% | 0% | **90%** |
| 证明可审计性 | 50% | 70% | **95%** |
| 密钥恢复能力 | 20% | 20% | **95%** |
| **总体隐私评分** | **30/100** | **85/100** | **95/100** |

---

## 11. 风险评估

### 11.1 技术风险

| 风险 | 严重度 | 概率 | 缓解 |
|------|--------|------|------|
| R1: TDX 侧信道 | 高 | 低 | 架构新, 攻击面小; Intel 持续修补 |
| R2: Intel 供应链 | 中 | 极低 | 未来可选 AMD SEV-SNP / ARM CCA |
| R3: Quote 验证 | 中 | 中 | 链下 DCAP + 链上存证 |
| R4: Secret Provisioning | 中 | 中 | Shamir K/N 分片 + TEE 互验 |
| R5: TD 崩溃恢复 | 中 | 中 | Shamir 任意 K 节点重构 |
| R6: 云厂商可用性 | 低 | 低 | Azure/GCP/阿里云均 GA |

### 11.2 架构风险

| 风险 | 严重度 | 概率 | 缓解 |
|------|--------|------|------|
| A1: 单点故障 | 低 | 低 | Multi-TEE + LB 漂移 |
| A2: 去中心化 | 低 | — | 多运营者各自部署 TEE |
| A3: Pallet 改造 | 低 | 低 | 主要简化 + 新增 ceremony-audit |
| A4: 测试失效 | 中 | 高 | 需重写集成测试 |

### 11.3 SGX 特有风险

| 风险 | 缓解 |
|------|------|
| EPC 128MB 限制 | Enclave < 1MB |
| ecall 性能 | 8 接口, ~150 次/s, 可忽略 |
| 桌面端弃用 | 仅服务器 Xeon |
| 编程复杂 | ~500 行, 审计面极小 |

### 11.4 新增风险 (Shamir + RA-TLS + 仪式)

| 风险 | 严重度 | 概率 | 缓解 |
|------|--------|------|------|
| S1: Ceremony Enclave 漏洞 | 高 | 低 | ~500 行最小代码 + 开源审计 + 链上白名单版本管理 |
| S2: Shamir 阈值不足 | 高 | 低 | on_initialize 自动检测活跃节点 < K → CeremonyAtRisk |
| S3: RA-TLS MITM | 高 | 极低 | TLS 证书绑定 SGX Quote, 前端验证 MRENCLAVE |
| S4: 仪式重放 | 中 | 低 | ceremony_hash 去重 + 链上时间戳 |
| S5: 全部节点同时宕机 | 高 | 极低 | 跨区域部署 + 跨云 Shamir 分片 |

---

## 12. 实施路线图

```
Phase 1 — 合并 + TDX 基础 (10 天)
├── Sprint T1 (3天): 合并 Agent+Node → nexus-tee-bot
│   ├── 复用: platform/, executor, rule_engine, local_processor, group_config
│   ├── 删除: multicaster, gossip/, verifier, leader
│   ├── 新建: main.rs 统一入口
│   └── 测试: 功能等价验证
│
├── Sprint T2 (3天): SGX Enclave + 密钥保护
│   ├── enclave/src/lib.rs: 8 个 ecall (~500 行)
│   ├── enclave_bridge.rs: ecall 封装 (~200 行)
│   ├── signer.rs 改造 → SGX 桥接
│   └── 测试: 密钥密封/解封/签名端到端
│
├── Sprint T3 (4天): 双证明 + 链上注册
│   ├── attestation.rs: TDX+SGX 双 Quote
│   ├── pallet-bot-registry: NodeType + Quote + 白名单
│   ├── chain_client.rs: subxt 直接提交
│   └── 测试: 证明流程端到端

Phase 2 — Multi-TEE + 生产化 (5 天)
├── Sprint T4 (3天): Active-Active + 高可用
│   ├── pallet-bot-consensus: ProcessedSequences + TEE 加权奖励
│   ├── pallet-bot-group-mgmt: NodeRequirement
│   ├── LB 配置 + Quote 24h 刷新
│   └── 测试: 多实例去重验证
│
├── Sprint T5 (2天): 监控 + pallet 简化
│   ├── Prometheus 指标 (TEE 状态/Quote 过期/ecall 延迟)
│   ├── pallet-bot-consensus 移除: Nodes/ActiveNodeList/MessageConfirmations
│   └── 测试: 链上回归

Phase 3 — Shamir + RA-TLS + 仪式审计 (8 天)
├── Sprint T6 (3天): Shamir 跨节点密钥恢复
│   ├── shamir_protocol.rs: 分片/恢复/互验 (~400 行)
│   ├── ecall_import_shares / ecall_export_share / ecall_reshare
│   └── 测试: 3 节点分片+恢复+故障切换
│
├── Sprint T7 (3天): RA-TLS 安全仪式
│   ├── ra_tls_ceremony.rs: Ceremony Enclave (~500 行)
│   ├── 前端 Quote 验证 JS
│   ├── Nginx TCP 透传配置
│   └── 测试: 端到端仪式流程
│
├── Sprint T8 (2天): 链上仪式审计
│   ├── pallet-ceremony-audit (~750 行): 5 extrinsic + Storage + Hook
│   ├── RPC 查询接口
│   └── 测试: 仪式记录/撤销/白名单/风险检测

Phase 4 — 可选增强 (按需)
├── 多厂商 TEE: AMD SEV-SNP 支持
├── 跨区域 Multi-TEE: DNS 故障转移
└── ZK 证明: TEE 执行结果 ZK 证明上链
```

### 12.1 工期与代码量

| Phase | 工期 | 新增 | 删除 | 改造 |
|-------|------|------|------|------|
| Phase 1 | 10 天 | ~1,650 行 | ~4,210 行 | ~1,500 行 |
| Phase 2 | 5 天 | ~600 行 | ~500 行 | ~300 行 |
| Phase 3 | 8 天 | ~1,650 行 | — | ~200 行 |
| **合计** | **23 天** | **~3,900 行** | **~4,710 行** | **~2,000 行** |

---

## 13. 结论

### 13.1 合理性 ✅ 高度合理

| 维度 | 判定 | 说明 |
|------|------|------|
| **安全性** | ✅ 显著提升 | TDX 保密 + SGX 纵深 + 双证明 + Shamir 恢复, 隐私 95/100 |
| **架构** | ✅ 大幅简化 | 代码 -58%, 消除 Gossip/共识/Leader, 1 种组件 |
| **高可用** | ✅ 保持 | Multi-TEE + LB 漂移 + Shamir K/N 容错 |
| **性能** | ✅ 大幅提升 | 5-20ms (vs 300-1000ms) |
| **密钥安全** | ✅ 本质提升 | SGX 密封 + Shamir 分片 + RA-TLS 仪式 + 链上审计 |
| **行业** | ✅ 符合趋势 | Oasis/Secret/Phala/Flashbots 等均采用 TEE |

### 13.2 可行性 ✅ 完全可行

| 维度 | 判定 | 说明 |
|------|------|------|
| **技术** | ✅ | TDX 无需改造; SGX 8 ecall ~500 行; Shamir/RA-TLS 成熟 |
| **云部署** | ✅ | Azure/GCP/阿里云 TDX+SGX GA |
| **迁移** | ✅ | 46% 代码直接复用, 新增 ~3,900 行 |
| **工期** | ✅ | 23 天完成 Phase 1+2+3 |

### 13.3 最终方案

**Multi-TEE Active-Active + TDX + 最小化 SGX Enclave + Shamir + RA-TLS + 链上审计:**

1. **TDX** 保护运行时数据 — 全内存加密, 主体逻辑零改造
2. **SGX Enclave** 保护密钥 — 8 个函数 ~500 行, 密钥永不出飞地, OS 被攻破仍安全
3. **Multi-TEE** 保证高可用 — Active-Active 多实例, LB 分发, 链上去重
4. **双证明** 保证可验证 — TDX Quote (MRTD) + SGX Quote (MRENCLAVE), 链上公开
5. **Shamir** 保证密钥恢复 — K/N 分片, 任意 K 节点重构, 容忍 N-K 故障
6. **RA-TLS** 保证安全注入 — Token 端到端加密直达 Enclave, 运营者不可见
7. **链上审计** 保证信任闭环 — 仪式记录不可篡改, 自动风险检测, 治理可撤销
8. **群主选择权** — `NodeRequirement` 策略, 按需选择隐私级别

---

> **附录: 术语表**
>
> | 术语 | 全称 | 含义 |
> |------|------|------|
> | SGX | Software Guard Extensions | Intel 进程级安全飞地 |
> | TDX | Trust Domain Extensions | Intel VM 级机密计算 |
> | TEE | Trusted Execution Environment | 可信执行环境 |
> | DCAP | Data Center Attestation Primitives | 数据中心远程证明 |
> | MRTD | Measurement of Trust Domain | TDX 信任域度量值 |
> | MRENCLAVE | Measurement of Enclave | SGX Enclave 代码哈希 |
> | RTMR | Runtime Measurement Register | TDX 运行时度量寄存器 |
> | TME-MK | Total Memory Encryption - Multi-Key | 全内存多密钥加密 |
> | EPC | Enclave Page Cache | SGX 飞地页缓存 |
> | Sealing | — | 数据密封 (加密绑定到特定 TEE) |
> | Quote | — | TEE 远程证明凭证 |
> | RA-TLS | Remote Attestation TLS | 远程证明 TLS (证书内嵌 SGX Quote) |
> | Shamir | Shamir's Secret Sharing | 门限密钥分片方案 |
> | SEV-SNP | Secure Encrypted Virtualization | AMD 机密 VM (替代方案) |
> | CCA | Confidential Compute Architecture | ARM 机密计算 (替代方案) |
