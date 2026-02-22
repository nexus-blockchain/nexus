# Nexus 群配置存储设计 — 全节点同步架构

> **核心原则：群主配置数据完全不上链。Agent 为唯一数据源，签名后同步到所有 Node。**

---

## 一、概述

### 1.1 设计目标

| # | 目标 | 说明 |
|---|---|---|
| 1 | **全部私有** | 配置数据不出现在任何链上存储 |
| 2 | **节点一致** | 所有 Node 持有相同的群规则副本 |
| 3 | **可审计** | 动作日志（ActionLog）仍上链存证 |
| 4 | **去中心化** | 不引入中心化后端服务器 |
| 5 | **即时生效** | Gossip 广播，无需等区块确认 |
| 6 | **简洁** | 无链上锚定哈希，无加解密分发流程 |

### 1.2 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                        区块链（链上）                         │
│                                                              │
│  bot-consensus: 节点注册、质押、ActiveNodeList               │
│  bot-registry:  Bot 注册、Ed25519 公钥、社区绑定             │
│  bot-group-mgmt: 仅 ActionLog（ban/mute/delete 动作存证）    │
│                                                              │
│  ✗ 不存储任何群配置数据                                      │
└──────────────────────────────┬──────────────────────────────┘
                               │ 公钥查询 / 日志提交
                               │
┌──────────────────────────────┴──────────────────────────────┐
│                    Agent（唯一配置源）                        │
│                                                              │
│  持有: GroupConfig（所有群规则） + AgentLocalConfig（私密）   │
│  职责: 接收群主 Web DApp 配置 → 签名 → Gossip 广播          │
└───────────┬──────────────────┬──────────────────┬───────────┘
            │ ConfigSync       │ ConfigSync        │ ConfigSync
            ▼                  ▼                   ▼
       ┌─────────┐       ┌─────────┐        ┌─────────┐
       │  Node 1  │       │  Node 2  │        │  Node 3  │
       │ 内存缓存  │       │ 内存缓存  │        │ 内存缓存  │
       │ 本地文件  │       │ 本地文件  │        │ 本地文件  │
       └─────────┘       └─────────┘        └─────────┘
       （所有节点持有完全相同的 GroupConfig 副本）
```

### 1.3 两层存储

| 层 | 名称 | 数据 | 可见性 | 存储 |
|---|---|---|---|---|
| **L1** | 全节点同步 | GroupConfig（所有群规则） | Agent + 所有 Node | Node 内存 + JSON 文件 |
| **L2** | Agent 本地私有 | Bot Token、签名私钥、自动回复 | 仅 Agent 进程 | AES-256-GCM 加密文件 |

**分层判断：** 该数据是否需要被 Node 读取来执行决策？是 → L1，否 → L2。

---

## 二、数据结构

### 2.1 L1 — GroupConfig（全节点同步）

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GroupConfig {
    pub community_id_hash: String,       // 社区 ID 哈希（加盐）

    // 入群策略
    pub join_policy: JoinApprovalPolicy,
    pub join_balance_threshold: Option<u128>,

    // 内容过滤
    pub filter_links: bool,
    pub filter_media: bool,
    pub restrict_mentions: bool,
    pub keyword_blacklist: Vec<String>,
    pub regex_filters: Vec<String>,

    // 反垃圾
    pub rate_limit_per_minute: u16,
    pub rate_limit_window_seconds: u16,
    pub auto_mute_duration_seconds: u64,
    pub warn_threshold: u8,
    pub warn_expire_seconds: u64,

    // 新成员限制
    pub new_member_restrict_seconds: u64,
    pub new_member_deny_links: bool,
    pub new_member_deny_media: bool,
    pub new_member_deny_mentions: bool,

    // 入群验证
    pub captcha_question: Option<String>,
    pub captcha_answer: Option<String>,
    pub captcha_timeout_seconds: u32,

    // 欢迎消息
    pub welcome_message: Option<String>,
    pub welcome_image_url: Option<String>,

    // 白名单 & 管理员
    pub whitelist_user_ids: Vec<String>,
    pub co_admins: Vec<String>,

    // 静默模式
    pub quiet_hours_start: Option<u8>,
    pub quiet_hours_end: Option<u8>,

    // 版本控制
    pub version: u32,                    // 单调递增
    pub updated_at_unix: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum JoinApprovalPolicy {
    AutoApprove,            // 自动通过
    ManualApproval,         // 管理员手动审批
    BalanceThreshold,       // 链上余额 >= 阈值
    RequirePlatformBinding, // 需要链上身份绑定
    CaptchaVerification,    // 回答验证问题
}
```

**配置项默认值：**

| 分类 | 配置项 | 类型 | 默认值 |
|---|---|---|---|
| 入群 | `join_policy` | enum | AutoApprove |
| 入群 | `join_balance_threshold` | Option | None |
| 过滤 | `filter_links` / `filter_media` / `restrict_mentions` | bool | false |
| 过滤 | `keyword_blacklist` / `regex_filters` | Vec | [] |
| 反垃圾 | `rate_limit_per_minute` | u16 | 0（不限制） |
| 反垃圾 | `rate_limit_window_seconds` | u16 | 60 |
| 反垃圾 | `auto_mute_duration_seconds` | u64 | 600 |
| 反垃圾 | `warn_threshold` / `warn_expire_seconds` | u8 / u64 | 3 / 86400 |
| 新成员 | `new_member_restrict_seconds` | u64 | 0（不限制） |
| 验证 | `captcha_question` / `captcha_answer` | Option | None |
| 欢迎 | `welcome_message` | Option | None |
| 权限 | `whitelist_user_ids` / `co_admins` | Vec | [] |
| 静默 | `quiet_hours_start` / `quiet_hours_end` | Option | None |

### 2.2 SignedGroupConfig（传输格式）

Agent 通过 Gossip 发送的不是裸 `GroupConfig`，而是签名包装：

```rust
#[derive(Serialize, Deserialize)]
pub struct SignedGroupConfig {
    pub config_bytes: Vec<u8>,   // JSON 序列化的 GroupConfig
    pub signature: [u8; 64],     // Agent Ed25519 签名
    pub bot_id_hash: String,     // 用于查找链上公钥
    pub version: u32,            // 冗余，便于快速比对
}
```

### 2.3 L2 — AgentLocalConfig（Agent 私有）

```rust
#[derive(Serialize, Deserialize)]
pub struct AgentLocalConfig {
    pub bot_token: String,              // TG API 全权凭证
    pub signing_key_seed: [u8; 32],     // Ed25519 私钥种子
    pub last_sequence: u64,             // 序列号
    pub auto_replies: Vec<AutoReplyRule>,
    pub command_menu: Vec<BotCommand>,
}
```

**Agent 本地文件布局：**

```
/data/
├── agent.key                ← Ed25519 私钥（chmod 600）
├── sequence.dat             ← 序列号（原子写入）
└── config/
    ├── local.json.enc       ← AES-256-GCM 加密的 AgentLocalConfig
    └── group_configs/
        └── {hash}.json.enc  ← GroupConfig 加密备份
```

加密密钥派生：`HKDF-SHA256(signing_key, "config-encryption")`

---

## 三、同步机制

### 3.1 配置更新流程

```
群主 Web DApp              Agent                       所有 Node
    │                        │                            │
    │  1. 钱包签名 challenge │                            │
    │  ─────────────────────>│                            │
    │  <── JWT ──────────────│                            │
    │                        │                            │
    │  2. POST /v1/group-config (携带 JWT)                │
    │  ─────────────────────>│                            │
    │                        │  3. 验证 JWT               │
    │                        │     version++              │
    │                        │     Ed25519 签名           │
    │                        │     本地加密持久化          │
    │  4. { version, ok }    │                            │
    │  <─────────────────────│                            │
    │                        │  5. Gossip ConfigSync ────>│
    │                        │                      N1 ✅ │
    │                        │                      N2 ✅ │
    │                        │                      N3 ✅ │
    │                        │                            │
    │                        │  6. 每个 Node:             │
    │                        │     查链上公钥 → 验签      │
    │                        │     检查 version > 本地    │
    │                        │     持久化 + 更新内存      │
```

### 3.2 Gossip 新增消息类型

```rust
pub enum GossipType {
    // 现有
    MessageSeen, MessagePull, PullResponse, DecisionVote,
    EquivocationAlert, ExecutionResult, LeaderTakeover, Heartbeat,
    // 新增
    ConfigSync,         // Agent 广播配置更新
    ConfigPull,         // Node 请求配置（启动/恢复时）
    ConfigPullResponse, // Agent 响应配置请求
}
```

### 3.3 节点启动恢复

1. 从本地 JSON 文件加载（可能是旧版本）
2. 向 Agent 发送 `ConfigPull`
3. Agent 回复 `ConfigPullResponse`（签名的最新配置）
4. Node 验证签名 + 版本号 → 更新缓存
5. Heartbeat 中携带 `config_version`，版本落后的节点收到后触发补齐

---

## 四、一致性保证

### 4.1 四层保障

| # | 机制 | 说明 |
|---|---|---|
| ① | **Agent 单源签名** | Agent 是唯一写入者，每次更新有 Ed25519 签名 + 递增版本号 |
| ② | **Gossip 广播 + 转发** | Agent 广播到所有节点；节点收到后转发给版本落后的节点 |
| ③ | **版本号单调递增** | 节点拒绝 `version <= local_version` 的更新，防降级和重放 |
| ④ | **启动时主动拉取** | 节点重启/新加入时向 Agent 请求最新配置 |

### 4.2 决策一致性

Gossip Seen 阶段每个节点附带 `config_version`：

```rust
pub struct SeenPayload {
    pub msg_id: String,
    pub msg_hash: String,
    pub node_id: String,
    pub config_version: u32,  // 该节点当前的 GroupConfig 版本
}
```

```
所有节点 config_version 一致 → 正常执行（常态）
少数节点版本落后 → ConfigSync 补齐 → 短暂延迟后重投票
Agent 离线 → 用已有最高版本执行（无人能更新，已有即最新）
```

**Leader 选举不受影响**（仍是 `seq % K`，K 来自链上 ActiveNodeList）。

---

## 五、节点存储

### 5.1 ChainCache 扩展

```rust
pub struct ChainCache {
    bots: RwLock<HashMap<String, BotInfoCache>>,          // 链上
    nodes: RwLock<HashMap<String, NodeInfoCache>>,         // 链上
    group_configs: RwLock<HashMap<String, GroupConfig>>,    // 新增: Agent 同步
}
```

### 5.2 本地持久化

```
/data/
├── node.key                 ← Ed25519 私钥
├── sequence.dat             ← 序列号
└── configs/
    └── {community_hash}.json  ← GroupConfig（明文 JSON）
```

> Node 侧不加密 GroupConfig — Node 本身是受信服务器，被入侵后内存数据同样暴露，加密文件无实质安全收益。

### 5.3 RuleEngine 使用

```rust
impl RuleEngine {
    pub fn get_rules(&self, community_id_hash: &str) -> Option<GroupConfig> {
        self.chain_cache.get_group_config(community_id_hash)
    }
}
```

单一数据源，无需 `MergedGroupRules` 合并逻辑。

---

## 六、Agent API

### 6.1 认证

```
POST /v1/auth/challenge
  Body: { wallet_address: "5G..." }
  Response: { challenge: "random_nonce_hex", expires_at: 1707400000 }

POST /v1/auth/verify
  Body: { wallet_address: "5G...", challenge: "...", signature: "wallet_sig" }
  Response: { token: "jwt_token", expires_in: 3600 }
```

### 6.2 群配置管理

```
POST /v1/group-config
  Auth: JWT（群主钱包签名获取）
  Body: GroupConfig (JSON)
  Response: { version: 3, ok: true }

GET /v1/group-config/{community_id_hash}
  Auth: JWT 或节点签名
  Response: SignedGroupConfig
```

### 6.3 节点配置同步

```
POST /v1/config-pull
  Headers: X-Node-Id, X-Node-Signature
  Body: { community_id_hash: "0x...", current_version: 2 }
  Auth: 验证节点签名 + 检查链上 ActiveNodeList
  Response: SignedGroupConfig（如果有更新版本）
```

---

## 七、安全设计

### 7.1 哈希碰撞修复

当前 `community_id_hash = SHA256(chat_id)` 不安全（chat_id 范围 ~10^13，几分钟可暴力碰撞）。

**修复：加盐哈希**

```rust
/// salt = SHA256(owner_account + "nexus-community-salt")
/// hash = SHA256(platform + chat_id + salt)
pub fn compute_community_id_hash(
    platform: &str, chat_id: i64, owner_account: &[u8; 32],
) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let salt = Sha256::new()
        .chain_update(owner_account)
        .chain_update(b"nexus-community-salt")
        .finalize();
    let result = Sha256::new()
        .chain_update(platform.as_bytes())
        .chain_update(&chat_id.to_le_bytes())
        .chain_update(&salt)
        .finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}
```

`platform_user_id_hash` 同理，使用 `"nexus-user-salt"` 加盐。前端 JS 实现相同算法。

### 7.2 安全审查清单

| # | 检查项 | 保证方式 |
|---|---|---|
| 1 | Bot Token 不出现在任何网络传输中 | L2 设计保证 |
| 2 | 群规则配置不出现在链上存储 | 全部 L1 链下 |
| 3 | 关键词黑名单不出现在链上 | 同上 |
| 4 | `community_id_hash` 加盐哈希 | 7.1 方案 |
| 5 | `platform_user_id_hash` 加盐哈希 | 同上 |
| 6 | 配置传输 Ed25519 签名保护 | SignedGroupConfig |
| 7 | 签名公钥来自链上 bot-registry | Node 验签流程 |
| 8 | 版本号单调递增，防降级攻击 | version 校验 |
| 9 | Agent 本地 AES-256-GCM 加密 | L2 加密存储 |
| 10 | Web DApp 钱包签名认证 | challenge-response |
| 11 | 动作日志上链存证可审计 | ActionLog |

---

## 八、Web DApp 配置界面

所有配置统一一个表单，发送到 Agent。**无需链上交易，无需 Gas 费。**

```
┌──────────────────────────────────────────────────────────┐
│  群规则配置  ·  My TG Group                               │
│  community: 0xabcd...1234                                 │
├──────────────────────────────────────────────────────────┤
│                                                           │
│  ═══ 入群策略 ═══                                         │
│  策略  [▼ 余额门槛]    最低余额  [100] NEX               │
│                                                           │
│  ═══ 内容过滤 ═══                                         │
│  ☑ 过滤链接  ☐ 过滤媒体  ☑ 限制 @提及                   │
│  关键词黑名单  "casino" [✕]  "免费领取" [✕]  [+ 添加]   │
│                                                           │
│  ═══ 反垃圾 ═══                                           │
│  频率限制 [10] 条/分  窗口 [60] 秒  禁言 [600] 秒        │
│  警告 [3] 次  过期 [86400] 秒                             │
│                                                           │
│  ═══ 新成员 ═══                                           │
│  限制 [600] 秒  禁止: ☑链接 ☑媒体 ☐@提及               │
│                                                           │
│  ═══ 欢迎 & 管理 ═══                                      │
│  欢迎消息  "欢迎 {username} 加入！请阅读置顶群规。"       │
│  白名单  @trusted_user [✕]   协管理员  5Grw...NehX [✕]  │
│                                                           │
│        [ 保存配置 ] ← 钱包签名 + 发送到 Agent             │
│                                                           │
│  同步状态: Agent v3 ✅  节点 3/3 ✅                       │
└──────────────────────────────────────────────────────────┘
```

---

## 九、实现计划

### 9.1 开发步骤

| # | 内容 | 改动范围 | 预估 |
|---|---|---|---|
| 1 | 精简 `pallet-bot-group-mgmt`（移除 GroupRules） | pallet, runtime | 0.5 天 |
| 2 | 哈希碰撞修复（加盐） | bot-registry, nexus-agent | 0.5 天 |
| 3 | 定义 `GroupConfig` + `SignedGroupConfig` | nexus-node/types, nexus-agent/types | 0.5 天 |
| 4 | Gossip 新增 ConfigSync / ConfigPull | nexus-node/gossip, types | 0.5 天 |
| 5 | Agent 配置管理 API + 签名分发 | nexus-agent/group_config.rs | 1 天 |
| 6 | Agent 认证 API（钱包 challenge） | nexus-agent/auth.rs | 0.5 天 |
| 7 | Node ChainCache 扩展 + 配置接收验证 | nexus-node/chain_cache | 1 天 |
| 8 | RuleEngine 对接 GroupConfig | nexus-node/rule_engine.rs | 1 天 |
| 9 | Web DApp 前端 | nexus-web/ | 4-5 天 |
| | **总计** | | **9-10 天** |

### 9.2 `pallet-bot-group-mgmt` 变更明细

```
移除: GroupRulesStore, GroupRules, JoinApprovalPolicy,
      set_group_rules, remove_group_rules,
      GroupRulesUpdated, GroupRulesRemoved, RulesAlreadyExist,
      get_join_policy

保留: ActionLogs (DoubleMap), LogCount, ActionLog, ActionType,
      log_action, ActionLogged
```

### 9.3 与旧方案对比

| 维度 | 旧方案（三层分离） | 新方案（全节点同步） |
|---|---|---|
| **复杂度** | 高（3 层 + 哈希锚定 + 加解密） | **低（Agent→Node 签名同步）** |
| **Gas 成本** | 每次改规则需链上交易 | **零** |
| **更新速度** | ~6 秒（等区块确认） | **即时（Gossip）** |
| **数据安全** | 部分公开，部分加密 | **全部私有** |
| **一致性** | L1 保证，L2 可能不一致 | **Agent 单源 + 版本号** |
| **节点代码** | L1/L2 分别处理 + 合并 | **单一 GroupConfig** |
| **实现周期** | 11-12 天 | **9-10 天** |

---

*文档版本: v2.0 · 2026-02-08*
*架构: 全节点同步（替代 v1.0 三层分离方案）*
*适用: nexus-node · nexus-agent · pallet-bot-group-mgmt*
