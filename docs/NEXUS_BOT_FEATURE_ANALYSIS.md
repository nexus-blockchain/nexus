# Nexus GroupRobot 功能深度分析与建议

> 基于 `/home/xiaodong/桌面/cosmos/telegram/` 下 12 个开源项目的源码深度分析，
> 结合 Nexus 现有架构（grouprobot + 4 个链上 Pallet + TEE 安全层），
> 提出适合 Nexus 独特定位的功能建议。

## 一、Nexus 现有能力盘点

### 链下 (grouprobot)
| 模块 | 能力 |
|------|------|
| `processing/rules/` | 5 条规则链: flood → blacklist → command → join → default |
| `platform/` | Telegram Webhook + Discord Gateway 双平台 |
| `tee/` | TDX+SGX 双证明、Shamir 秘密分片、RA-TLS Token 注入、加密 IPC |
| `infra/` | RateLimiter、LocalStore (DashMap)、ConfigManager 链上同步 |
| `chain/` | subxt 动态查询/交易、ActionLogBatcher 批量提交 |

### 链上 (4 Pallets)
| Pallet | Index | 能力 |
|--------|:-----:|------|
| Registry | 150 | Bot 注册、DCAP Level 4 证明、MRTD/MRENCLAVE 白名单 |
| Consensus | 151 | 序列去重、动作日志、订阅/奖励 (设计中) |
| Community | 152 | 群规则配置、CAS 乐观锁 |
| Ceremony | 153 | RA-TLS 仪式记录 |

### 核心差异化
- **TEE 安全执行**: Token 永不明文暴露，jemalloc zero-on-free
- **链上可审计**: 所有管理动作上链，不可篡改
- **去中心化共识**: 多节点对等，无单点信任
- **经济模型**: 订阅费 + 通胀保底，节点激励

---

## 二、源码深度分析 — 值得借鉴的功能模式

### 2.1 tg-spam: 多层级垃圾检测流水线

**源码位置**: `tg-spam/lib/tgspam/detector.go`

tg-spam 的 `Detector.Check()` 是一个 **12 步顺序检测流水线**：

```
重复消息检测 → 已审批用户跳过 → 停用词 → Emoji 计数 → Meta 检查组
→ Lua 插件检查 → CAS API → 多语言混合 → 异常空格 → 消息长度过滤
→ 相似度比对 → 贝叶斯分类器 → (可选) OpenAI Veto
```

**关键设计模式**:
- `MetaCheck` 函数签名 `func(spamcheck.Request) spamcheck.Response` — 每个检查独立、可组合
- `LuaPluginEngine` 接口允许用户自定义检测逻辑
- `SampleUpdater` 接口支持运行时动态学习 (管理员标记 spam/ham 后在线更新)
- `duplicateDetector` 滑动窗口检测同一用户重复消息
- OpenAI 作为"否决权"层 — 可确认或推翻前面检查的结论

### 2.2 YAGPDB: Trigger → Condition → Effect 三段式 AutoMod

**源码位置**: `yagpdb/automod/rulepart.go`, `triggers.go`, `effects.go`

YAGPDB automod 是最成熟的 **可视化规则引擎**:

```
RulePartMap = {
    // 38 种 Trigger (1-38): AllCaps、Mentions、AnyLink、Violations、
    //   WordList(黑/白)、Domain(黑/白)、ServerInvite、SafeBrowsing、
    //   Slowmode(频道/全局)、Regex(正/反)、Spam、NicknameRegex、
    //   AntiPhishing、MessageLength、Attachments、MemberJoin ...
    
    // 21 种 Condition (200-220): MemberRoles(黑/白)、Channels(黑/白)、
    //   AccountAge、MemberAge、Bot(忽略/仅)、ChannelCategories、
    //   MessageEdited、Thread、Attachment、Forward ...
    
    // 16 种 Effect (300-315): DeleteMessage、AddViolation、Kick、Ban、
    //   Mute、Warn、SetNickname、ResetViolations、GiveRole、
    //   EnableSlowmode、RemoveRole、SendMessage、Timeout、Alert ...
}
```

**关键设计模式**:
- 每个规则由 `Trigger[] + Condition[] + Effect[]` 组成
- 所有规则持久化到 PostgreSQL (`sqlboiler` ORM)
- `Violations` 累积器 — 触发次数达到阈值后执行更严厉效果
- Web 控制面板可视化配置 (无需写代码)

### 2.3 YAGPDB: 声誉系统 (Reputation)

**源码位置**: `yagpdb/reputation/reputation.go`

- 用户互相 +rep/-rep，有冷却时间 (默认 120s)
- `RANK() OVER(ORDER BY points DESC)` SQL 窗口函数排名
- 可配置: 每次最大给/减点数、冷却时间、积分名称
- 排行榜: `TopUsers(guildID, offset, limit)`

### 2.4 Gojo_Satoru: CAPTCHA 验证入群

**源码位置**: `Gojo_Satoru/Powers/plugins/captcha.py`

- 新成员入群 → 限制权限 → 发送图片验证码 → Inline Keyboard 选择答案
- 超时未验证 → 自动踢出
- 支持 image / QR 两种模式
- `CAPTCHA_DATA` 存储每个验证会话状态

### 2.5 YAGPDB: Google reCAPTCHA 网页验证

**源码位置**: `yagpdb/verification/verification.go`

- 发送 DM 给新成员，包含验证链接
- 网页端解 reCAPTCHA → 通过后自动赋予角色
- 比群内验证码更强的反机器人能力

### 2.6 YAGPDB 插件架构

**源码位置**: `yagpdb/common/plugins.go`

```go
type Plugin interface {
    PluginInfo() *PluginInfo
}
// + PluginWithCommonRun, PluginWithBotStarted, PluginWithWebStarted ...
```

- 40+ 独立插件目录，每个实现 `Plugin` 接口
- `RegisterPlugin()` 全局注册，按 Category 分类 (Core/Moderation/Misc/Feeds)
- 插件可选实现 Bot/Web/Common 多个生命周期钩子

---

## 三、功能建议 — 按实现层分类

### P0: 高优先级 (直接增强核心竞争力)

#### 3.1 多层级垃圾检测流水线 ← tg-spam

**当前状态**: Nexus 仅有 `FloodRule` + `BlacklistRule` 两个内容检查规则

**建议**: 将 tg-spam 的检测算法移植为 Rust 规则模块

| 新规则 | 实现层 | 复杂度 | 说明 |
|--------|--------|:------:|------|
| `DuplicateRule` | 链下 LocalStore | 低 | 滑动窗口检测同用户重复消息 (tg-spam `duplicateDetector`) |
| `EmojiRule` | 链下 | 低 | 超过 N 个 emoji 标记为 spam |
| `LinkLimitRule` | 链下 | 低 | 限制消息中链接/提及数量 (tg-spam `meta.links-limit`) |
| `StopWordRule` | 链下 ConfigManager | 中 | 停用词列表，支持链上配置更新 |
| `SimilarityRule` | 链下 | 中 | 消息与已知 spam 样本的余弦相似度 (token 化 + TF-IDF) |
| `BayesClassifierRule` | 链下 TEE | 高 | 朴素贝叶斯分类器，模型文件密封在 Enclave 中 |

**关键设计**: 每个检查返回 `SpamCheckResult { name, is_spam, confidence, details }`, 最终由 Router 汇总决策。与 tg-spam 的 `spamcheck.Response` 完全对齐。

**链上联动**: 
- 群主可通过 `pallet-community` 配置启用/禁用每种检查、调整阈值
- 检测结果 (命中的规则名 + 置信度) 作为 ActionLog 提交上链
- **样本共享**: 多个群的 spam 样本可通过链上聚合，提升全网检测能力 (这是中心化 bot 做不到的)

```
建议文件: grouprobot/src/processing/rules/duplicate.rs
                                          /emoji.rs
                                          /link_limit.rs
                                          /stop_word.rs
                                          /similarity.rs   (Phase 2)
                                          /classifier.rs   (Phase 3)
```

#### 3.2 CAPTCHA 入群验证 ← Gojo + YAGPDB

**当前状态**: Nexus 有 `JoinRequestRule` 但仅做基本审批

**建议**: 实现两级验证

| 级别 | 方式 | 适用场景 |
|------|------|---------|
| Level 1 | 群内 Inline Keyboard 数学题 | 低风险群，体验好 |
| Level 2 | RA-TLS 网页 CAPTCHA | 高风险群，TEE 生成验证链接 |

**Nexus 独特优势**: 
- Level 2 验证链接由 TEE Enclave 签名生成，**验证结果不可伪造**
- 验证通过的证明可上链 (pallet-community 存储 `VerifiedMember`)
- 跨群复用: 用户在一个 Nexus 管理的群通过验证后，其他群可信任该验证

**链上存储**:
```rust
// pallet-community 新增
VerifiedMembers: StorageDoubleMap<CommunityId, UserId, VerificationRecord>
struct VerificationRecord {
    verified_at: BlockNumber,
    method: VerificationMethod,  // Captcha | RaTlsWeb | ManualApprove
    verified_by_bot: [u8; 32],   // bot_id_hash
}
```

#### 3.3 链上声誉系统 ← YAGPDB Reputation

**当前状态**: Nexus 无声誉系统

**建议**: 将 YAGPDB 的 reputation 模型提升为**链上原生**

| 维度 | YAGPDB (链下 PostgreSQL) | Nexus 建议 (链上 Pallet) |
|------|--------------------------|--------------------------|
| 存储 | 中心化 DB | `pallet-community` StorageMap |
| 给分权限 | 任何人 | 仅验证过的成员 |
| 防刷 | Redis cooldown | 链上 `LastGaveRep` + 冷却块数 |
| 排名 | SQL RANK() | 链下索引 + 链上验证 |
| 跨群 | 不支持 | **全局声誉** (跨所有 Nexus 群累积) |
| 经济联动 | 无 | 声誉影响 token 空投/治理权重 |

**链上新增**:
```rust
// pallet-community 扩展
ReputationPoints: StorageDoubleMap<CommunityId, UserId, i64>
GlobalReputation: StorageMap<UserId, i64>  // 跨群汇总
ReputationCooldown: StorageDoubleMap<SenderId, ReceiverId, BlockNumber>

// 新 extrinsic
fn give_reputation(origin, community_id, target_user, points: i8) -> Result
fn query_reputation(community_id, user_id) -> (local_rep, global_rep, rank)
```

**这是 Nexus 最大差异化之一**: 中心化 bot 的声誉数据锁在单个 bot 服务器中，Nexus 的声誉是**链上公共品**，任何 DApp 都可以读取和使用。

---

### P1: 中优先级 (功能完善)

#### 3.4 Trigger-Condition-Effect 可组合规则引擎 ← YAGPDB AutoMod

**当前状态**: Nexus 的 `RuleEngine` 是硬编码的线性规则链

**建议**: 重构为 YAGPDB 式的三段式可组合规则

```rust
// grouprobot/src/processing/rule_engine.rs 重构
pub struct AutoModRule {
    pub id: u64,
    pub name: String,
    pub triggers: Vec<Box<dyn Trigger>>,      // 什么时候触发
    pub conditions: Vec<Box<dyn Condition>>,   // 附加条件过滤
    pub effects: Vec<Box<dyn Effect>>,         // 触发后做什么
    pub enabled: bool,
}

pub trait Trigger: Send + Sync {
    fn check(&self, ctx: &MessageContext, store: &LocalStore) -> Option<TriggerResult>;
}

pub trait Condition: Send + Sync {
    fn check(&self, ctx: &MessageContext) -> bool;  // true = 通过
}

pub trait Effect: Send + Sync {
    fn apply(&self, ctx: &MessageContext) -> ActionDecision;
}
```

**规则配置存储在链上** (`pallet-community`), 通过 `ConfigManager` 同步到本地。群主在 DApp 前端可视化编辑规则。

**移植的 Trigger 类型** (参考 YAGPDB 38 种):

| 优先实现 | 来源 |
|---------|------|
| `WordListTrigger` (黑/白名单) | YAGPDB |
| `LinkTrigger` (任意链接/特定域名) | YAGPDB |
| `MentionsTrigger` (超过 N 个 @) | YAGPDB |
| `SlowmodeTrigger` (频率限制) | YAGPDB |
| `MessageLengthTrigger` | YAGPDB |
| `MemberJoinTrigger` | YAGPDB |
| `DuplicateMessageTrigger` | tg-spam |

**移植的 Effect 类型**:

| 优先实现 | 来源 |
|---------|------|
| `DeleteMessage` | 所有项目 |
| `WarnUser` | YAGPDB + FallenRobot |
| `MuteUser` (临时/永久) | YAGPDB |
| `KickUser` | 所有项目 |
| `BanUser` | 所有项目 |
| `AddViolation` (累积触发) | YAGPDB 独有 |
| `SendAlert` (通知管理员) | YAGPDB |

#### 3.5 警告系统 + 自动升级惩罚 ← FallenRobot + YAGPDB

**当前状态**: Nexus 的 command.rs 处理 `/warn` 但无累积逻辑

**建议**:

```
Warn 1 → 警告消息
Warn 2 → 禁言 1 小时  
Warn 3 → 踢出群
Warn N (可配置) → 封禁
```

- 警告计数存储在 `LocalStore` (即时) + 链上 `pallet-community` (持久)
- YAGPDB 的 `AddViolation` + `ViolationsTrigger` 组合模式值得采纳
- 链上存储使警告记录**跨节点同步**且**不可删除** (管理员问责)

#### 3.6 欢迎消息 + 告别消息 ← Gojo + DaisyBot

**当前状态**: 无

**建议**: 
- 链上配置 `WelcomeTemplate` / `GoodbyeTemplate` (Markdown 模板)
- 支持变量: `{user_name}`, `{chat_name}`, `{member_count}`, `{rules_link}`
- 新成员入群时由 TEE 节点渲染模板并发送
- 配合 CAPTCHA 验证: 欢迎消息包含验证按钮

---

### P2: 低优先级 (生态丰富)

#### 3.7 自定义命令/过滤器 ← Red-DiscordBot Cog + FallenRobot filters

群主可定义关键词 → 自动回复，存储在链上，所有节点同步执行。

#### 3.8 管理日志频道 ← grpmr-rs + YAGPDB logs

所有管理动作 (ban/mute/warn/delete) 自动转发到指定日志频道，附带链上交易哈希作为"审计凭证"。

#### 3.9 Notes / Rules 群公告 ← FallenRobot + Lucyna

`/rules` 显示链上存储的群规则，`/notes` 存储常用回复模板。

#### 3.10 Anti-Phishing ← YAGPDB + tg-spam CAS

- YAGPDB 的 `antiphishing/` 模块对接外部钓鱼链接数据库
- tg-spam 的 CAS (Combot Anti-Spam) API 集成
- Nexus 可建立**链上钓鱼 URL 黑名单** — 任何 Nexus 节点举报后全网生效

---

## 四、实现路线图

```
Phase 1 (2-3 周): 核心规则增强
├── DuplicateRule + EmojiRule + LinkLimitRule (3 个简单规则)
├── StopWordRule (链上配置联动)
├── 警告系统 (计数 + 自动升级)
└── 欢迎/告别消息

Phase 2 (3-4 周): AutoMod 重构 + CAPTCHA
├── Trigger-Condition-Effect 三段式规则引擎
├── CAPTCHA 入群验证 (Level 1 群内 + Level 2 RA-TLS 网页)
├── 管理日志频道
└── SimilarityRule (TF-IDF 相似度检测)

Phase 3 (4-6 周): 链上声誉 + 高级检测
├── pallet-community 声誉系统 (本地 + 全局声誉)
├── BayesClassifierRule (TEE 内模型训练)
├── Anti-Phishing (CAS + 链上黑名单)
├── 自定义命令/过滤器
└── Web 控制面板 (DApp 前端)
```

---

## 五、架构原则

### 5.1 链上 vs 链下决策矩阵

| 存在哪里 | 什么数据 | 原因 |
|---------|---------|------|
| **链上** | 群配置、规则定义、声誉积分、验证记录、警告历史、spam 样本哈希 | 需要跨节点共识、不可篡改、DApp 可读 |
| **链下 TEE** | 消息内容处理、Bayes 模型权重、Token 密钥、实时判定逻辑 | 隐私敏感、高频操作、不适合上链 |
| **链下 LocalStore** | 限流计数、消息指纹缓存、flood 窗口 | 纯临时状态、节点独立 |

### 5.2 与中心化 Bot 的差异化定位

| 维度 | 中心化 Bot (FallenRobot 等) | Nexus |
|------|---------------------------|-------|
| 信任模型 | 信任 Bot 运营者 | **TEE + 链上验证，无需信任** |
| 数据所有权 | Bot 运营者拥有 | **群主拥有，链上公开** |
| 审计能力 | 无 (运营者可篡改日志) | **链上不可篡改审计轨迹** |
| 声誉 | 孤岛 (每个 bot 独立) | **跨群全局声誉公共品** |
| 可用性 | 单点故障 | **多节点去中心化** |
| spam 检测 | 单点学习 | **全网协同学习** (链上样本聚合) |
| 经济激励 | 无 | **节点运营者赚取 NXS** |

### 5.3 不建议实现的功能

| 功能 | 原因 |
|------|------|
| AI 聊天 (chatbot) | 与群管核心能力无关，增加攻击面 |
| 娱乐命令 (dice/coin/fun) | 不需要 TEE/链上，价值低 |
| Google/Wiki/翻译 | 纯 API 代理，无差异化 |
| 音乐/流媒体 | Discord 特有，与 Nexus 定位不符 |
| RSS/社交媒体订阅 | 可作为 Phase 4 插件，非核心 |

---

## 六、总结

Nexus 的**核心差异化**在于 TEE + 链上 的独特组合。功能扩展应围绕这个优势展开：

1. **安全增强**: 移植 tg-spam 的多层检测算法 → 在 TEE 中执行 → 检测结果上链 → 全网学习
2. **信任增强**: CAPTCHA 验证 → TEE 签名 → 链上存储 → 跨群复用
3. **治理增强**: 声誉系统 → 链上公共品 → DApp 可组合
4. **自动化增强**: YAGPDB 三段式规则引擎 → 链上配置 → DApp 可视化编辑

每个新功能都应回答一个问题：**"链上/TEE 能为这个功能带来什么中心化 bot 做不到的价值？"** 如果答案是"没有"，就不要实现。
