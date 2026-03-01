# 每节点独立 api_id/api_hash + 独立运营群机器人 — 可行性与合理性分析

> 日期: 2026-03-01
> 状态: 评估文档

---

## 一、Telegram API 底层约束

| 约束 | 描述 | 影响 |
|------|------|------|
| api_id/api_hash 1:1 绑定 | 每个 Telegram 开发者账号（手机号）只能获得 **一组** api_id/api_hash | 每个运营商需一个 Telegram 开发者账号 |
| Local Server 1:1 绑定 | 一个 bot 同时只能连接 **一个** Local Bot API Server | 同一 bot 不能多节点同时用 Local Server |
| logOut 冷却期 | 从 Local Server 切回官方 API 需 10 分钟冷却 | 不可能做热切换 |
| Bot Token 独立 | bot token 由 @BotFather 创建，与 api_id 无关 | 同一 api_id 可运行多个 bot |
| 文件限制差异 | 官方 API 50MB / Local Server 2GB | Local Server 是增值功能 |

---

## 二、两种部署模型对比

### 模型 A（当前设计）: 多节点、同一 Bot

```
Node A ─┐                         @CosmosGroupBot (单一 bot)
Node B ─┤ ── Shamir K-of-N ────►  serves: G1, G2, G3, ...
Node C ─┤    同一 bot_id_hash      PeerRegistry: [A, B, C, D]
Node D ─┘    同一 api_id/api_hash
```

- Shamir 跨节点分片保护 bot token
- 序列去重确保同一消息只处理一次
- 高可用: A 宕机，B/C/D 接管
- **问题**: Local Server 只能绑定一个实例 → 要么不用 Local Server，要么只能 active-standby

### 模型 B（提议）: 独立节点、不同 Bot

```
Operator A (phone_A) ── api_id_A ── @GroupBot_A ── groups: GA1, GA2
Operator B (phone_B) ── api_id_B ── @GroupBot_B ── groups: GB1, GB2
Operator C (phone_C) ── api_id_C ── @GroupBot_C ── groups: GC1, GC2
                                                   ↑ 各自独立链上注册
```

- 每个运营商完全独立
- 各自有自己的 bot token、api_id/api_hash、TEE enclave
- 链上各自注册、提交证明、提交日志

---

## 三、可行性分析

### 3.1 技术可行性: ✅ 完全可行

| 层面 | 评估 |
|------|------|
| Telegram API | 不同 bot + 不同 api_id，零冲突。每个 Local Server 独立运行 |
| TEE 层 | EnclaveBridge / SealedStorage / TokenVault 天然按进程隔离，无需修改 |
| api_id/api_hash 密封 | 已实现，每节点独立密封自己的 credentials |
| 链上注册 | 现有 `register_bot(bot_id_hash, public_key)` 直接适用，每 bot 独立注册 |
| TEE 证明 | 每节点独立提交 TDX/SGX Quote，已有的 Attestation 机制完全适用 |
| 群规则 | GroupRobotCommunity Pallet 按群配置，不绑定特定 bot → 多 bot 可服务同一群 |

### 3.2 Shamir / Ceremony 适配

| 机制 | 模型 A (多节点同 bot) | 模型 B (独立运营) |
|------|----------------------|-------------------|
| Shamir K-of-N | K=2, N=4 跨节点 | K=1 本地密封即可（单节点无需跨节点） |
| Ceremony | 节点间 share 分发/收集 | 不需要跨运营商 ceremony |
| PeerRegistry | 4 节点注册到同一 bot_id_hash | 每 bot 只有自己一个节点（或运营商自建多节点） |
| Migration | 旧→新 enclave 密钥传递 | 同上，但仅限运营商自己的节点 |
| RA-TLS Provision | 管理员注入到共享 enclave | 运营商自行注入到自己的 enclave |

**结论:** 模型 B 下，Shamir/Ceremony 机制不是跨运营商使用，而是运营商内部可选的高可用措施。现有代码零修改可支持 K=1 单节点模式。

### 3.3 api_id/api_hash 获取成本

| 步骤 | 难度 | 说明 |
|------|------|------|
| 注册 Telegram 账号 | 低 | 需要一个手机号 |
| 访问 my.telegram.org/apps | 低 | 填写 app 信息即获得 |
| 创建 Bot (@BotFather) | 低 | `/newbot` 命令即可 |
| **合计** | **低** | **10 分钟可完成** |

每个运营商只需做一次。api_id/api_hash 不会过期。

---

## 四、合理性分析

### 4.1 优势

| 维度 | 分析 |
|------|------|
| 真正去中心化 | 没有单一 bot 被封禁导致全网瘫痪的风险 |
| 安全隔离 | 一个运营商泄露 api_id/api_hash 不影响其他运营商 |
| Local Server 兼容 | 每个运营商独立运行自己的 Local Server，不存在 1:1 绑定冲突 |
| 监管合规 | 不同司法管辖区的运营商各自负责 |
| 弹性扩展 | 新运营商加入无需协调现有节点 |
| TEE 简化 | 无需跨运营商 ceremony，降低攻击面 |
| 经济激励 | 运营商自负盈亏，市场化竞争提高服务质量 |

### 4.2 挑战

| 挑战 | 严重程度 | 缓解方案 |
|------|----------|----------|
| 用户体验碎片化 | **高** | 链上运营商目录 + DApp 统一入口（用户选 bot，底层自动跳转） |
| 单节点无高可用 | 中 | 运营商可选自建多节点（K-of-N 仍可用）；或 SLA 惩罚机制 |
| 品牌不统一 | 中 | 链上认证标识（如 bot 描述统一包含 Cosmos 标志）+ 运营商评分 |
| 规则一致性 | 低 | Community Pallet 已按群配置规则，所有 bot 读取相同链上规则 |
| 代码版本碎片化 | 低 | MRTD/MRENCLAVE 白名单机制确保所有节点运行审计过的代码 |

### 4.3 链上架构适配

现有 4 个 Pallet 的兼容性:

| Pallet | 兼容性 | 需要的改动 |
|--------|--------|-----------|
| GroupRobotRegistry (150) | ✅ 完全兼容 | 新增 `OperatorInfo` 存储（api_id_hash、联系信息、SLA 等级） |
| GroupRobotConsensus (151) | ✅ 完全兼容 | `bot_id_hash` 已是 per-bot 键，序列去重、日志提交天然隔离 |
| GroupRobotCommunity (152) | ✅ 完全兼容 | 群规则按 `group_id` 存储，不绑定 bot → 不同 bot 服务同一群读同一规则 |
| GroupRobotCeremony (153) | ✅ 兼容（降级使用） | 跨运营商无需 ceremony；运营商内部多节点仍可用 |

**关键:** Community Pallet 的规则是按群存储的，不是按 bot。这天然支持"不同 bot 管理同一个群，遵守同一套规则"。

---

## 五、推荐架构: 混合模型

纯独立模型有碎片化风险，纯集中模型有单点故障。建议分层:

```
┌─────────────────────────────────────────────────────────┐
│                    链上治理层                              │
│  GroupRobotRegistry: 运营商注册 + TEE 证明 + MRTD 白名单  │
│  GroupRobotCommunity: 按群存储规则 (运营商无关)           │
│  GroupRobotConsensus: 按 bot_id_hash 记录日志 + 激励     │
├─────────────────────────────────────────────────────────┤
│                   运营商层 (独立)                          │
│                                                         │
│  ┌─ Operator A ───────────────┐  ┌─ Operator B ───┐    │
│  │  api_id_A / api_hash_A     │  │  api_id_B      │    │
│  │  @GroupBot_A               │  │  @GroupBot_B    │    │
│  │  ┌─────┐  ┌─────┐         │  │  ┌─────┐       │    │
│  │  │TEE-1│  │TEE-2│ (K=2)   │  │  │TEE-1│ (K=1) │    │
│  │  └─────┘  └─────┘         │  │  └─────┘       │    │
│  │  Local Bot API Server      │  │  官方 API       │    │
│  └────────────────────────────┘  └────────────────┘    │
│                                                         │
│  ┌─ Operator C (协议官方) ─────────────────────────┐    │
│  │  api_id_C / api_hash_C                          │    │
│  │  @CosmosOfficialBot (旗舰)                      │    │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  (K=2, N=4) │    │
│  │  │TEE-1│ │TEE-2│ │TEE-3│ │TEE-4│              │    │
│  │  └─────┘ └─────┘ └─────┘ └─────┘              │    │
│  │  Local Bot API Server (HA)                      │    │
│  └─────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────┤
│                    用户层                                 │
│  DApp 入口 → 按群选择/推荐运营商 → 跳转到对应 Bot        │
│  群管理员: 选择 @GroupBot_A 或 @CosmosOfficialBot 管群   │
└─────────────────────────────────────────────────────────┘
```

---

## 六、结论

| 评估维度 | 结论 |
|----------|------|
| 技术可行性 | **完全可行**。现有 TEE 架构 + 已实现的 api_id/api_hash 密封已支持。Shamir K=1 单节点零改动可用 |
| Telegram 约束 | **无冲突**。独立 bot + 独立 api credentials = 独立 Local Server，绕过了 1:1 绑定限制 |
| 链上兼容性 | **高度兼容**。Community Pallet 按群存规则、Registry 按 bot_id_hash 注册，天然支持多运营商 |
| 安全性 | **提升**。api_id/api_hash 泄露半径从全局降为单运营商 |
| 经济合理性 | **合理**。市场化运营商竞争 + 链上 SLA 惩罚 + Consensus Pallet 激励分配 |
| 需要补充 | 链上新增运营商注册/评分机制；DApp 层运营商发现/推荐 UI |

### 推荐策略

- **V1.0**: 先以协议官方运营商（模型 A，多节点高可用）上线
- **V1.1**: 开放第三方运营商注册（模型 B），两种模型共存，链上治理统一
