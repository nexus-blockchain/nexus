# Phala pRuntime Worker (SGX) vs GroupRobot Bot Client (TDX+SGX) 深度对比

> 日期: 2026-02-22 · 版本: v1.0
> 关联: [TEE_BLOCKCHAIN_COMPARISON.md](./TEE_BLOCKCHAIN_COMPARISON.md) · [TEE_SGX_TDX_BOT_ANALYSIS.md](./TEE_SGX_TDX_BOT_ANALYSIS.md)

---

## 1. 项目定位对比

| 维度 | Phala pRuntime | GroupRobot Bot Client |
|------|---------------|----------------------|
| **定位** | 去中心化通用机密云计算 | 群管理 TEE Bot 客户端 |
| **计算类型** | 通用 ink!/Docker 合约 | 专用群管理逻辑 |
| **数据来源** | 链上交易 + 用户请求 | 平台 API (Telegram/Discord) |
| **输出目标** | 加密状态 → 链上存储 | API 调用 + 链上日志 |
| **节点规模** | ~100K Worker (无许可) | ~3-9 实例/Bot (半许可) |
| **主网** | 2021 | 2026 设计中 |

**核心差异**: Phala 是"计算密集型"(CPU 合约执行)，GroupRobot 是"I/O 密集型"(平台 API 交互)。

---

## 2. 整体架构对比

### 2.1 Phala pRuntime

```
┌──────────── SGX Enclave (全部 ~20,000+ 行) ──────────────┐
│  Light Validation Client │ pink-runtime (ink!) │ Key Mgmt │
│  Encrypted State Manager │ RA Quote Generator  │ ECDH     │
└──────────────────────────────────────────────────────────┘
  ← 区块数据                            加密状态 → 链上
```

### 2.2 GroupRobot Bot Client

```
┌─── TDX Trust Domain (VM 级加密, ~9,500 行) ──────────────┐
│  Platform Adapters │ Rule Engine │ Executor │ Chain Client │
│  ┌── SGX Enclave (最小化, ~500 行, 8 ecall) ──────────┐  │
│  │  Ed25519 密封/签名 │ BOT_TOKEN 解密 │ Shamir 分片  │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
  ← Webhook 事件      → API 调用 (TG/DC)    摘要 → 链上
```

### 2.3 关键差异

| 维度 | pRuntime | GroupRobot Bot |
|------|----------|---------------|
| **TEE 边界** | 整个运行时都在 SGX | 业务在 TDX，密钥在 SGX |
| **Enclave 代码量** | ~20,000+ 行 | ~500 行 (8 ecall) |
| **信任模型** | 单层 SGX | TDX + SGX 双层纵深 |
| **外部 I/O** | 仅链交互 | 链 + 平台 API (TG/DC) |

---

## 3. TEE 硬件使用模式

### 3.1 Phala: SGX-Only

- **全部代码**运行在 SGX Enclave 中
- 优点: 单一安全边界，概念简单
- 缺点: EPC 128MB 限制，编程复杂，大 TCB

### 3.2 GroupRobot: TDX + SGX 双层

- **TDX**: VM 级全内存加密，保护业务逻辑和运行时数据
- **SGX** (~500 行): 仅密钥操作，即使 Guest OS 被攻破密钥仍安全

### 3.3 攻击场景对比

| 攻击场景 | pRuntime (SGX-Only) | GroupRobot (TDX+SGX) | 优势方 |
|----------|--------------------|--------------------|--------|
| Host/VMM 读内存 | ✅ SGX 加密 | ✅ TDX 加密 | 平 |
| Guest OS 内核漏洞 | ✅ SGX 不信任 OS | ✅ TDX + SGX 双保护 | GroupRobot |
| SGX 侧信道 (ÆPIC 等) | ⚠️ **全部暴露** | ⚠️ 仅密钥暴露，业务数据受 TDX 保护 | **GroupRobot** |
| TDX 侧信道 | N/A | ⚠️ 业务暴露，密钥受 SGX 保护 | **GroupRobot** |
| EPC 换页攻击 | ⚠️ 大 Enclave 风险高 | ✅ Enclave < 1MB | **GroupRobot** |
| 代码完整性 | ✅ MRENCLAVE | ✅ MRTD + MRENCLAVE | GroupRobot |

> **关键结论**: 双层模式在 SGX 侧信道场景下显著优于 SGX-Only。攻击者需同时突破两层才能获取全部信息。

---

## 4. 密钥体系对比

### 4.1 Phala: 层级密钥树

```
MasterKey (Gatekeeper 共享, 全网信任根)
  ├── ClusterKey = KDF(MasterKey, cluster_id)
  │     └── ContractKey = KDF(ClusterKey, contract_id)
  └── WorkerKey (每 Worker 独立, sr25519)
```

- **特点**: 4 层派生，MasterKey 泄露 → 全网历史数据泄露
- **Gatekeeper**: 3-7 个高信任 Worker，持有 MasterKey，是特权角色
- **新节点**: Gatekeeper 通过 ECDH 自动分发 ClusterKey

### 4.2 GroupRobot: 扁平 Shamir 分片

```
Ed25519 SigningKey + BOT_TOKEN
  ├── Share₁ → TEE Node #1 (SGX Sealed)
  ├── Share₂ → TEE Node #2 (SGX Sealed)
  └── Share₃ → TEE Node #3 (SGX Sealed)
  任意 K(=2) 个 Share → 重构完整密钥
```

- **特点**: 1 层 + 分片，K-1 个 Share 泄露 → 无影响
- **无特权角色**: 所有节点对等
- **新节点**: K 个存活节点通过 RA-TLS 互验后分发 Share

### 4.3 对比矩阵

| 维度 | Phala Gatekeeper | GroupRobot Shamir |
|------|-----------------|-------------------|
| **特权角色** | ✅ 有 (Gatekeeper) | ❌ 无 (对等节点) |
| **单点风险** | ⚠️ MasterKey 泄露=全网 | ✅ K-1 Share 无影响 |
| **容错** | Gatekeeper 可替换 | K-of-N (如 2-of-3) |
| **扩展性** | ✅ 万级节点 | ⚠️ 十级节点 |
| **轮换成本** | 高 (全网重派生) | 低 (单次 Re-Ceremony) |
| **链上审计** | ❌ 无 | ✅ ceremony Pallet |
| **用户参与** | ❌ 自动 | ✅ 首次需用户输入 Token |

---

## 5. 链上 Pallet 对比

| Phala Pallet | 职责 | GroupRobot Pallet | 职责 |
|-------------|------|-------------------|------|
| `pallet-registry` | Worker 注册 + RA + Confidence Level | `grouprobot-registry` (150) | Bot 注册 + 双证明 + 白名单 |
| `pallet-compute` | 任务调度 + 状态根 | `grouprobot-community` (152) | 群管理日志 + 社区绑定 |
| `pallet-staking` | 质押 + 奖励 + Slash | `grouprobot-consensus` (151) | 质押 + TEE 加权 + 订阅 + 去重 |
| `KmsAuth` (合约) | 密钥管理授权 | `grouprobot-ceremony` (153) | **仪式审计** + Enclave 白名单 |
| — | — | `grouprobot-primitives` | 共享类型 + BotRegistryProvider trait |

| 维度 | Phala | GroupRobot |
|------|-------|-----------|
| 链上代码 | ~4,000 行 (3 Pallet) | ~3,900 行 (4 Pallet + primitives) |
| 离链代码 | ~20,000 行 | ~10,000 行 |
| 链上占比 | ~17% | ~28% |
| **独创** | 加密状态链上存储 | ceremony Pallet 链上仪式审计 |

---

## 6. 可信计算基 (TCB) 对比

| 项目 | SGX Enclave 代码量 | 符合 TCB 最小化 |
|------|:-----------------:|:--------------:|
| Flashbots Sirrah | ~2,000 行 | ✅ |
| **GroupRobot** | **~500 行** | ✅✅ (最小) |
| Phala pRuntime | ~20,000+ 行 | ❌ |

```
TCB 代码量        审计成本      漏洞概率      修复速度
  500 行   →   1 人·周   →   极低    →   小时级
20,000 行  →   数人·月   →   中等    →   周级
```

> GroupRobot 的 SGX Enclave 是所有 TEE 区块链项目中 TCB 最小的。pRuntime 大 TCB 是通用计算的必然代价。

---

## 7. 性能与资源对比

| 指标 | pRuntime (SGX) | GroupRobot Bot (TDX+SGX) |
|------|:-------------:|:------------------------:|
| 内存限制 | EPC 128MB~ | ❌ 无限制 (TDX) |
| CPU 开销 | ~5-15% | ~2-5% (TDX) + ecall ~15µs |
| 事件处理延迟 | ~100-500ms (区块确认) | ~5-20ms (直接 API) |
| 内存占用 | 50-200 MB | ~85 MB |
| 单实例月费 | $20-40 | $15-25 |

---

## 8. 安全评分对比

| 维度 | Phala | GroupRobot | 优势方 |
|------|:-----:|:---------:|--------|
| 安全纵深 | 70 | **95** | GroupRobot (双层) |
| TCB 最小化 | 50 | **98** | GroupRobot (500 行) |
| 密钥安全 | 80 | **95** | GroupRobot (Shamir 对等) |
| 通用性 | **95** | 40 | Phala (通用云) |
| 扩展性 | **95** | 60 | Phala (万级) |
| 生产成熟度 | **95** | 30 | Phala (5 年主网) |
| 可审计性 | 70 | **95** | GroupRobot (ceremony) |
| **综合** | **79** | **73** | 各有所长 |

---

## 9. 可借鉴的设计模式

### 9.1 GroupRobot 可从 Phala 借鉴

| 模式 | 说明 | 优先级 |
|------|------|:------:|
| **Confidence Level 分级** | 基于硬件漏洞分配信任等级 | 中 |
| **自动密钥分发** | Gatekeeper 无感分发，减少用户参与 | 中 |
| **多 Worker 对比** | 多 Worker 执行同一任务并对比结果 | 低 |
| **Encrypted State** | 加密状态链上存储 (可选: 加密群配置) | 低 |

### 9.2 Phala 可从 GroupRobot 借鉴

| 模式 | 说明 | 优先级 |
|------|------|:------:|
| **TDX+SGX 双层** | dstack 已在探索此方向 | 高 |
| **TCB 最小化** | 密钥操作拆分为独立小 Enclave | 中 |
| **链上仪式审计** | 为 Gatekeeper 轮换增加链上审计 | 中 |
| **双证明体系** | MRTD + MRENCLAVE 双锚点 | 中 |

---

## 10. 结论

### 10.1 定位决定架构

| 维度 | Phala pRuntime | GroupRobot Bot |
|------|:------------:|:-------------:|
| **本质** | 通用机密计算引擎 | 专用安全 Bot 客户端 |
| **TEE 策略** | "全包" (全在 SGX) | "分层" (TDX + SGX) |
| **密钥模型** | "层级" (Master→Cluster→Contract) | "扁平" (Shamir K-of-N) |
| **设计取舍** | 通用性 > TCB 最小化 | 安全纵深 > 通用性 |

### 10.2 各自的正确选择

- **Phala SGX-Only 是正确的**: 通用计算云需要在 Enclave 内运行任意合约，大 TCB 是通用性的必然代价。
- **GroupRobot TDX+SGX 是正确的**: 专用 Bot 业务逻辑可预先审计，将密钥操作最小化到 ~500 行 SGX 是最优安全策略。

### 10.3 互补关系

两者不是竞争关系，而是 TEE + 区块链模式在不同应用场景下的最优适配：

```
Phala:      通用计算 → SGX-Only → 层级密钥 → 大规模网络
GroupRobot: 专用 Bot → TDX+SGX → Shamir 分片 → 小规模高安全
```

**Phala 的 dstack 演进方向（TDX 支持）验证了 GroupRobot 双层架构的合理性。**
**GroupRobot 的 ceremony Pallet 是行业中少有的链上密钥仪式审计机制。**

---

*文档版本: v1.0 · 2026-02-22*
*维护者: GroupRobot Team*
