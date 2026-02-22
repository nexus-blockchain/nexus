# nexus-tee-bot 离链功能转 Pallet 模块：可行性与合理性分析

> 日期: 2026-02-22
> 版本: v1.0
> 关联文档: [GROUPROBOT_PALLET_DESIGN.md](./GROUPROBOT_PALLET_DESIGN.md) · [TEE_SGX_TDX_BOT_ANALYSIS.md](./TEE_SGX_TDX_BOT_ANALYSIS.md)

---

## 目录

1. [分析背景](#1-分析背景)
2. [Substrate Pallet 运行环境约束](#2-substrate-pallet-运行环境约束)
3. [逐模块可行性评估](#3-逐模块可行性评估)
4. [不可上链模块的技术原因](#4-不可上链模块的技术原因)
5. [可上链模块的设计方案](#5-可上链模块的设计方案)
6. [混合架构：Pallet + OCW + 离链客户端](#6-混合架构pallet--ocw--离链客户端)
7. [合理性分析](#7-合理性分析)
8. [替代方案对比](#8-替代方案对比)
9. [结论与建议](#9-结论与建议)

---

## 1. 分析背景

### 1.1 问题

`nexus-tee-bot` 是一个运行在 TDX+SGX 环境中的离链 Rust 二进制程序，包含 20+ 个功能模块、~199 个测试。
[GROUPROBOT_PALLET_DESIGN.md](./GROUPROBOT_PALLET_DESIGN.md) 将其**链上可映射部分**设计为 4 个 Pallet 子模块，但将大部分功能标记为"纯离链"。

本文档回答：**能否将 nexus-tee-bot 的全部或更多功能直接设计成 Pallet 模块？这样做是否合理？**

### 1.2 nexus-tee-bot 模块清单

| 模块 | 代码行数 | 核心功能 |
|------|---------|---------|
| `enclave_bridge.rs` | ~28KB | SGX Enclave 桥接 (8 ecall) |
| `enclave/` | — | SGX Enclave 代码 |
| `attestation.rs` | ~19KB | TDX+SGX 双证明生成 |
| `signer.rs` | ~15KB | Ed25519 签名 (Enclave 内) |
| `shamir.rs` | ~17KB | Shamir 秘密分享 (GF(256)) |
| `ceremony.rs` | ~20KB | RA-TLS 安全仪式服务 |
| `chain_client.rs` | ~11KB | subxt 链交互 |
| `executor.rs` | ~27KB | Telegram Bot API 执行 |
| `discord_executor.rs` | ~31KB | Discord Bot API 执行 |
| `webhook.rs` | ~23KB | HTTP Webhook 服务 |
| `local_processor.rs` | ~37KB | 消息处理 + 规则引擎 |
| `local_store.rs` | ~13KB | 本地状态缓存 |
| `group_config.rs` | ~16KB | 群配置管理 |
| `config.rs` | ~11KB | 运行时配置 |
| `metrics.rs` | ~11KB | Prometheus 监控 |
| `types.rs` | ~14KB | 共享类型定义 |
| `rate_limiter.rs` | ~2KB | 速率限制 |
| `crypto.rs` | ~4KB | 加密工具 |
| `platform/` | ~3 文件 | 平台适配器 |
| `gateway/` | ~2 文件 | 网关适配器 |

---

## 2. Substrate Pallet 运行环境约束

### 2.1 硬性约束

Substrate Runtime (Pallet 运行环境) 有以下**不可违反**的技术约束：

| 约束 | 说明 | 影响 |
|------|------|------|
| **`no_std`** | 不能使用 Rust 标准库 (`std`) | 无 `std::fs`、`std::net`、`std::time`、`std::thread` |
| **确定性执行** | 所有验证节点必须对同一区块产生完全相同的状态转换结果 | 禁止随机数、浮点运算、系统时间、任何非确定性操作 |
| **无网络 I/O** | Runtime 内不能发起任何网络请求 | 无法调用 Telegram/Discord API、无法发 HTTP 请求 |
| **无文件 I/O** | Runtime 内不能读写文件系统 | 无法读取配置文件、密封密钥、本地缓存 |
| **无硬件访问** | Runtime 内不能访问 SGX/TDX 硬件指令 | 无法执行 ecall、生成 Quote、密封/解封 |
| **计算受限** | 每个 extrinsic 有 Weight 上限 (每区块 ~2 秒) | 复杂计算 (GF(256) 大量运算) 可能超限 |
| **存储付费** | 链上存储需要支付押金 (Storage Deposit) | 大量消息/日志上链成本极高 |
| **公开透明** | 链上所有数据对所有节点可见 | **与 TEE 隐私保护目标直接矛盾** |

### 2.2 Off-chain Worker (OCW) 的能力与限制

Substrate 提供 Off-chain Worker 作为"半链上"扩展：

| 能力 | 说明 |
|------|------|
| ✅ 可发 HTTP 请求 | `sp_runtime::offchain::http` |
| ✅ 可访问本地存储 | `offchain::storage` (每节点独立) |
| ✅ 可获取系统时间 | `sp_io::offchain::timestamp()` |
| ✅ 可提交签名交易 | 通过 `SubmitTransaction` |

| 限制 | 说明 |
|------|------|
| ❌ 非确定性 | 每个节点独立执行，结果可能不同 |
| ❌ 不能直接写 Storage | 必须通过提交交易间接写入 |
| ❌ 无 SGX/TDX 支持 | 标准 Substrate OCW 无 TEE 硬件抽象 |
| ❌ 执行时机受限 | 仅在每个区块导入后触发，非实时 |
| ❌ 无长连接 | 不支持 WebSocket/Webhook 长连接 |

---

## 3. 逐模块可行性评估

### 3.1 评估矩阵

| 模块 | 转 Pallet | 转 OCW | 保持离链 | 原因 |
|------|:---------:|:------:|:--------:|------|
| `enclave_bridge.rs` | ❌ | ❌ | ✅ | 依赖 SGX 硬件指令 (ecall/ocall)，Pallet 和 OCW 均无法访问 |
| `enclave/` | ❌ | ❌ | ✅ | SGX Enclave 代码必须运行在 Intel CPU 安全区内 |
| `attestation.rs` | ⚠️ 部分 | ❌ | ✅ 主体 | Quote **生成**依赖硬件；Quote **验证+存证**可上链 |
| `signer.rs` | ❌ | ❌ | ✅ | 私钥必须在 SGX Enclave 内，签名操作不可外泄 |
| `shamir.rs` | ⚠️ 参数 | ❌ | ✅ 主体 | GF(256) 运算可在 `no_std` 实现，但**密钥分片不应上链**（泄露隐私） |
| `ceremony.rs` | ⚠️ 记录 | ❌ | ✅ 主体 | 仪式执行需 RA-TLS (网络+SGX)；仪式**记录**可上链 |
| `chain_client.rs` | ❌ | ❌ | ✅ | subxt 是离链 RPC 客户端，本身就是"调用 Pallet 的客户端" |
| `executor.rs` | ❌ | ⚠️ | ✅ | 需调用 Telegram HTTP API（发消息/踢人/封禁） |
| `discord_executor.rs` | ❌ | ⚠️ | ✅ | 需调用 Discord HTTP API |
| `webhook.rs` | ❌ | ❌ | ✅ | 需运行 HTTP 服务器 (Axum) 接收平台回调 |
| `local_processor.rs` | ⚠️ 规则 | ❌ | ✅ 主体 | 规则引擎逻辑可上链；但消息内容处理涉及隐私+实时性 |
| `local_store.rs` | ❌ | ❌ | ✅ | 内存缓存 (flood 计数、指纹去重)，需毫秒级响应 |
| `group_config.rs` | ✅ 部分 | ❌ | ⚠️ | 群规则配置可上链（已在 community pallet 设计）；实时同步保持离链 |
| `config.rs` | ❌ | ❌ | ✅ | 环境变量 + .env 文件加载，纯运行时配置 |
| `metrics.rs` | ❌ | ❌ | ✅ | Prometheus 注册表 + HTTP /metrics 端点 |
| `types.rs` | ✅ | — | — | 类型定义可上链（已在 primitives 设计） |
| `rate_limiter.rs` | ❌ | ❌ | ✅ | 需系统时间 + 毫秒精度，Pallet 无法实现 |
| `crypto.rs` | ⚠️ | — | ✅ | 部分密码学工具可在 `no_std`，但大多与 SGX 绑定 |
| `platform/` | ✅ 枚举 | ❌ | ✅ 主体 | Platform 枚举可上链；适配器逻辑需网络 I/O |
| `gateway/` | ❌ | ❌ | ✅ | WebSocket/HTTP 长连接 |

### 3.2 统计汇总

| 类别 | 模块数 | 占比 | 代码量 |
|------|--------|------|--------|
| **完全不可上链** | 12 | 60% | ~220KB |
| **仅参数/记录可上链** | 5 | 25% | ~93KB |
| **类型定义可上链** | 3 | 15% | ~30KB |
| **主体逻辑可上链** | 0 | 0% | 0 |

---

## 4. 不可上链模块的技术原因

### 4.1 SGX/TDX 硬件依赖 — 根本性障碍

```
nexus-tee-bot 核心安全模型:
  私钥 → SGX Enclave (ecall_sign) → 签名
  Bot Token → SGX Enclave (ecall_seal_secret) → 密封存储
  双证明 → TDX Quote + SGX Quote → 硬件生成

Substrate Pallet 运行环境:
  WASM 虚拟机 → 无硬件指令访问 → 无法执行 ecall
```

**结论：** `enclave_bridge.rs`、`enclave/`、`signer.rs`、`attestation.rs`（生成部分）**在技术层面完全不可能**转为 Pallet。这不是工程难度问题，而是**架构层面的不可能**。

### 4.2 网络 I/O — 违反确定性原则

```
executor.rs:       reqwest::Client → POST https://api.telegram.org/bot{token}/sendMessage
discord_executor:  reqwest::Client → POST https://discord.com/api/v10/...
webhook.rs:        axum::Router → 监听 HTTP 端口接收平台回调
```

Pallet 内发起 HTTP 请求会导致：
- **不同验证节点得到不同结果**（网络延迟、超时、API 错误不同）
- **状态转换不确定** → 分叉 → 链崩溃

OCW 虽然可以发 HTTP，但：
- 执行时机是**区块导入后**，非实时（6 秒延迟）
- **不能保证执行**（节点可以关闭 OCW）
- 不能运行 HTTP 服务器（只能发请求，不能收请求）

**结论：** `executor.rs`、`discord_executor.rs`、`webhook.rs` **不可上链，OCW 也无法替代**。

### 4.3 实时性要求 — 与区块时间矛盾

```
用户发消息 → webhook 接收 (~5ms) → 规则引擎判断 (~1ms) → 执行动作 (~10ms)
总延迟: ~16ms

如果改为 Pallet:
用户发消息 → 离链提交交易 → 等待出块 (6s) → Pallet 处理 → 离链读取结果 → 执行
总延迟: ~12-18s （恶化 750-1000 倍）
```

**结论：** `local_processor.rs`、`local_store.rs`、`rate_limiter.rs` 的核心逻辑**不应上链**，否则用户体验不可接受。

### 4.4 隐私保护 — 与链上透明性矛盾

| 数据 | 当前保护 | 如果上链 |
|------|---------|---------|
| Bot Token | SGX 密封，仅 Enclave 内可见 | ❌ 所有验证节点可见 |
| 用户消息内容 | TDX 全内存加密 | ❌ 链上永久公开 |
| Shamir 分片 | AES-GCM 加密 + SGX 密封 | ❌ 链上暴露分片数据 |
| 签名私钥 | 仅 SGX Enclave 内 | ❌ 不可能放入 Pallet |

**结论：** nexus-tee-bot 的安全模型**依赖硬件隐私保护**。将敏感操作上链会**彻底摧毁安全模型**。

---

## 5. 可上链模块的设计方案

虽然主体逻辑不可上链，但以下数据/参数确实适合上链，且已在 [GROUPROBOT_PALLET_DESIGN.md](./GROUPROBOT_PALLET_DESIGN.md) 中设计：

### 5.1 已纳入 Pallet 的部分

| 离链模块 | 上链内容 | 目标 Pallet |
|---|---|---|
| `attestation.rs` | Quote 摘要 + MRTD/MRENCLAVE 存证 | `grouprobot-registry` |
| `shamir.rs` | Shamir 参数 (K, N) + 参与节点列表 | `grouprobot-ceremony` |
| `ceremony.rs` | 仪式记录 + 状态 + 审计哈希 | `grouprobot-ceremony` |
| `group_config.rs` | 群规则配置 (NodeRequirement, 防刷屏阈值等) | `grouprobot-community` |
| `local_processor.rs` | 动作日志 (ActionLog) 存证 | `grouprobot-community` |
| `types.rs` | Platform 枚举, NodeType, 共享类型 | `grouprobot-primitives` |
| `chain_client.rs` | 消息序列去重 (ProcessedSequences) | `grouprobot-consensus` |

### 5.2 上链比例分析

```
nexus-tee-bot 总功能  ████████████████████████████████████████  100%
                      │                                      │
可上链（参数/记录）    ████████                                  ~20%
必须离链（核心逻辑）          ████████████████████████████████  ~80%
```

---

## 6. 混合架构：Pallet + OCW + 离链客户端

### 6.1 推荐的三层架构

```
┌─────────────────────────────────────────────────────────┐
│  Layer 1: 链上 Pallet (grouprobot/*)                     │
│  • 注册/证明存证/奖励/去重/仪式审计/群规则               │
│  • 确定性、公开透明、不可篡改                             │
│  • ~20% 功能                                             │
├─────────────────────────────────────────────────────────┤
│  Layer 2: Off-chain Worker (可选增强)                     │
│  • 证明过期扫描 + 提醒                                   │
│  • 仪式风险检测 (活跃节点 < K)                           │
│  • ~5% 功能                                              │
├─────────────────────────────────────────────────────────┤
│  Layer 3: nexus-tee-bot 离链客户端 (TDX+SGX)             │
│  • SGX Enclave: 密钥密封/签名/Shamir                    │
│  • TDX: 消息处理/规则引擎/API 调用                      │
│  • Webhook 服务/执行器/监控                              │
│  • ~75% 功能                                             │
└─────────────────────────────────────────────────────────┘
```

### 6.2 层间交互协议

```
                   离链 (Layer 3)                    链上 (Layer 1)
                   ─────────────                    ──────────────
nexus-tee-bot ──── subxt ──────────────────────────▶ grouprobot-registry
  │ register_bot(bot_id_hash, public_key)
  │ submit_attestation(tdx_quote_hash, sgx_quote_hash, mrtd, mrenclave)

nexus-tee-bot ──── subxt ──────────────────────────▶ grouprobot-consensus
  │ mark_sequence_processed(bot_id_hash, sequence)

nexus-tee-bot ──── subxt ──────────────────────────▶ grouprobot-community
  │ submit_action_log(community_id_hash, action, signature)

nexus-tee-bot ──── subxt ──────────────────────────▶ grouprobot-ceremony
  │ record_ceremony(ceremony_hash, mrenclave, bot_pk, k, n)
```

---

## 7. 合理性分析

### 7.1 为什么不应该把更多功能放入 Pallet

| 维度 | 全部上链 | 当前混合架构 | 分析 |
|------|---------|------------|------|
| **安全性** | ❌ 密钥暴露、隐私泄露 | ✅ SGX 保护密钥，TDX 保护数据 | 全部上链**降低**安全性 |
| **性能** | ❌ 6s 出块延迟 | ✅ 5-20ms 响应 | 全部上链**恶化** 300-1000 倍 |
| **成本** | ❌ 每条消息付 Gas | ✅ 仅关键操作上链 | 全部上链成本**不可承受** |
| **隐私** | ❌ 消息内容链上永久公开 | ✅ TDX 加密内存 | 全部上链**摧毁**隐私模型 |
| **可用性** | ❌ 依赖出块才能响应 | ✅ 独立运行、实时响应 | 全部上链**降低**可用性 |
| **去中心化** | ✅ 完全链上验证 | ⚠️ 依赖 TEE 硬件信任 | 唯一优势，但 TEE 已提供替代信任 |

### 7.2 "全部上链"的反模式分析

```
反模式: 把所有逻辑塞入 Pallet
  → 消息处理在链上执行
  → 每条群消息 = 一笔交易
  → 100 人群 × 10 条/分钟 = 1000 tx/分钟
  → 超过大多数 Substrate 链的吞吐量
  → 链拥塞 → 所有 Bot 停止响应
  → 系统崩溃
```

```
正确模式: 链上存证 + 离链执行
  → 消息处理在 TDX 内离链执行 (毫秒级)
  → 仅关键操作上链: 注册、证明、去重、动作日志
  → 链上负载: ~10 tx/分钟/Bot (可承受)
  → TEE 硬件提供信任保证 (替代多节点共识)
```

### 7.3 行业对比

| 项目 | 链上逻辑 | 离链逻辑 | 信任来源 |
|------|---------|---------|---------|
| **Oasis Network** | 状态/资产/治理 | Confidential VM 内执行 | TEE |
| **Secret Network** | 加密状态管理 | SGX Enclave 内计算 | TEE |
| **Phala Network** | 合约注册/调度 | TEE Worker 内执行 | TEE |
| **Flashbots SUAVE** | 拍卖结果上链 | TEE 内竞价计算 | TEE |
| **本方案** | 注册/证明/奖励/审计 | TDX+SGX 内消息处理 | TEE |

**所有 TEE 区块链项目都采用"链上存证 + 离链 TEE 执行"模式。没有项目将 TEE 逻辑全部放入 Pallet。**

### 7.4 唯一例外：链上轻量验证

以下逻辑**可以**考虑增加到 Pallet 中（作为可选增强）：

| 功能 | 当前位置 | 可上链形式 | 价值 |
|------|---------|-----------|------|
| Quote 格式验证 | 离链 | Pallet 内简单结构校验 | 拒绝明显无效的证明 |
| 签名验证 | 离链 | Pallet 内 `ed25519_verify` | 链上验证动作日志签名 |
| Shamir 参数校验 | 离链 | Pallet 内 `k > 0 && k <= n && n <= 254` | 拒绝无效参数 |
| 配置版本 CAS | 离链 | Pallet 内 `ensure!(new_version == old + 1)` | 防止并发冲突 |

这些已在 [GROUPROBOT_PALLET_DESIGN.md](./GROUPROBOT_PALLET_DESIGN.md) 的设计中包含。

---

## 8. 替代方案对比

### 8.1 方案 A：当前设计（推荐）— Pallet 存证 + 离链 TEE 执行

```
优点: 安全、高性能、低成本、隐私保护、行业验证
缺点: 信任依赖 TEE 硬件
适用: ✅ 本项目
```

### 8.2 方案 B：全部上链（不可行）

```
优点: 完全去中心化验证
缺点: 密钥暴露、隐私泄露、性能不可接受、成本极高
适用: ❌ 不适用
```

### 8.3 方案 C：智能合约 (ink! / EVM)

```
优点: 逻辑链上可验证
缺点: 同样受 no_std/确定性/无网络 I/O 约束；gas 成本更高；无法调用外部 API
适用: ❌ 不适用（约束与 Pallet 相同）
```

### 8.4 方案 D：更多逻辑放入 OCW

```
优点: 可发 HTTP 请求
缺点:
  - 不能运行 HTTP 服务器（无法接收 Webhook）
  - 执行不保证（节点可关闭 OCW）
  - 6 秒延迟（每区块触发一次）
  - 无 SGX/TDX 支持
  - 无法替代实时消息处理
适用: ⚠️ 仅适合辅助功能（证明过期扫描等）
```

### 8.5 方案汇总

| 方案 | 可行性 | 安全性 | 性能 | 成本 | 推荐 |
|------|--------|--------|------|------|------|
| **A: Pallet + 离链 TEE** | ✅ | ✅ 最高 | ✅ 最快 | ✅ 最低 | **✅ 推荐** |
| B: 全部上链 | ❌ | ❌ 最低 | ❌ 最慢 | ❌ 最高 | ❌ |
| C: 智能合约 | ❌ | ❌ | ❌ | ❌ | ❌ |
| D: 更多 OCW | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ 仅辅助 |

---

## 9. 结论与建议

### 9.1 核心结论

**将 nexus-tee-bot 的全部功能转为 Pallet 模块在技术上不可行，在安全上不合理。**

| 判定 | 理由 |
|------|------|
| **技术不可行** | 60% 模块依赖 SGX 硬件/网络 I/O/文件系统，Pallet (`no_std` + 确定性) 环境**根本无法执行** |
| **安全不合理** | 密钥、Token、消息上链会**彻底摧毁** TEE 隐私保护模型 |
| **性能不可接受** | 实时消息处理 (5-20ms) 退化为区块级延迟 (6-18s)，用户体验**不可接受** |
| **成本不可承受** | 每条消息上链需支付交易费，大群场景下**经济不可持续** |
| **行业无先例** | 所有 TEE 区块链项目均采用"链上存证 + 离链 TEE 执行"模式 |

### 9.2 推荐架构

维持 [GROUPROBOT_PALLET_DESIGN.md](./GROUPROBOT_PALLET_DESIGN.md) 的设计：

```
链上 Pallet (~20% 功能):
  grouprobot-registry    → Bot 注册 + TEE 证明存证
  grouprobot-consensus   → 节点奖励 + 消息去重
  grouprobot-community   → 群规则 + 动作日志存证
  grouprobot-ceremony    → 仪式审计 + Enclave 白名单

离链 TEE Client (~80% 功能):
  nexus-tee-bot          → SGX 密钥保护 + TDX 消息处理 + API 执行
```

### 9.3 可选增强建议

| 增强项 | 层级 | 优先级 | 说明 |
|---|---|---|---|
| 链上签名验证 | Pallet | 中 | 动作日志提交时 Pallet 内 `ed25519_verify` |
| OCW 证明过期扫描 | OCW | 低 | 辅助提醒 Bot 刷新证明 |
| 链上配置版本 CAS | Pallet | 中 | 群配置更新的乐观锁 |
| RPC 查询接口 | Runtime API | 中 | `ceremony_health()`、`bot_attestation_status()` |

---

*文档版本: v1.0 · 2026-02-22*
*维护者: Nexus Team*
