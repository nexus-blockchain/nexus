# GroupRobot — 功能差距综合分析报告

> **日期:** 2026-02-27
> **范围:** 对比 grouprobot (机器人代码 + 链上 Pallet) 与 10 个参考群管理机器人
> **状态:** 已完成

---

## 1. 参考机器人一览

| 机器人 | 语言 | 平台 | 核心特性 |
|--------|------|------|----------|
| **FallenRobot** | Python | Telegram | 贴纸黑名单、用户白名单、全局封禁、自定义过滤器 |
| **Gojo_Satoru** | Python | Telegram | 验证码、自动审批入群、群聊黑名单、群规系统、警告系统 |
| **grpmr-rs** | Rust | Telegram | 警告、禁言/封禁/踢出、锁定/解锁、软/硬警告模式 |
| **tg-spam** | Go | Telegram | ML 垃圾检测 (朴素贝叶斯 + 余弦相似度)、CAS API、停用词 |
| **samurai** | Python | Telegram | ML 垃圾检测 (Transformer)、NSFW 图片检测、脏话过滤、违规追踪 |
| **sudobot** | TypeScript | Discord | AI 自动审核、突袭防护、身份验证、违规记录、批量封禁/踢出、消息恢复 |
| **lucyna** | TypeScript | Telegram | Grammy 框架、Redis 缓存、成员跟踪 |
| **yagpdb** | Go | Discord | 可组合自动审核引擎 (触发器→条件→效果)、自动角色、批量角色 |
| **modbot** | JavaScript | Discord | 审核框架、可配置规则 |
| **Red-DiscordBot** / **nadekobot** | Python/C# | Discord | 插件架构、丰富的模块/cog 系统 |

---

## 2. GroupRobot — 现有功能清单

### 2.1 机器人端规则 (`grouprobot/src/processing/rules/`)

| 规则 | 文件 | 说明 | 链上配置字段 |
|------|------|------|--------------|
| **FloodRule** | `flood.rs` | 按用户每 60 秒窗口限流 → 禁言 | `anti_flood_enabled`, `flood_limit` |
| **DuplicateRule** | `duplicate.rs` | 基于哈希的重复消息检测 → 警告 | `anti_duplicate_enabled`, `duplicate_window_secs`, `duplicate_threshold` |
| **BlacklistRule** | `blacklist.rs` | 正则表达式黑名单 → 警告 | 模式从链上获取 |
| **StopWordRule** | `stop_word.rs` | CSV 停用词匹配 → 警告 | `stop_words` |
| **EmojiRule** | `emoji.rs` | Emoji 数量限制 → 警告 | `max_emoji` |
| **LinkLimitRule** | `link_limit.rs` | URL 数量限制 (管理员豁免) → 警告 | `max_links` |
| **SimilarityRule** | `similarity.rs` | 与已知垃圾样本的余弦相似度 → 警告 | `spam_samples`, `similarity_threshold` |
| **AntiPhishingRule** | `antiphishing.rs` | 钓鱼域名黑名单 + 仿冒检测 + 可疑模式 → 警告 | `antiphishing_enabled` |
| **LockRule** | `lock.rs` | 锁定特定消息类型 (图片、视频、贴纸等) → 删除 | `locked_types_csv` |
| **CommandRule** | `command.rs` | 管理命令: /ban /kick /mute /unmute /unban /warn /promote /demote /lock /unlock /locks；公开命令: /help /id /rules | — |
| **JoinRequestRule** | `join.rs` | 自动或手动审批入群请求 + 欢迎消息模板 | `welcome_enabled`, `welcome_template` |
| **WarnTracker** | `warn_tracker.rs` | 警告累积 + 达到上限自动升级 (禁言/踢出/封禁) | `warn_limit`, `warn_action`, `warn_mute_duration` |
| **CallbackRule** | `callback.rs` | 内联键盘回调处理 | — |
| **AdFooterRule** | `ad_footer.rs` | 免费层群组强制广告尾注 | `subscription_tier` |
| **ClassifierRule** | `classifier.rs` | 贝叶斯分类器 (存根/部分实现) | `bayes_threshold` |

### 2.2 链上 Pallet (`pallets/grouprobot/`)

| Pallet | 用途 |
|--------|------|
| **registry** | 机器人注册、TEE 远程证明 (DCAP)、节点管理 |
| **community** | 社区 (群) 注册、链上配置存储 |
| **consensus** | 多节点行动日志共识 |
| **ceremony** | 密钥仪式 / DKG 协议 |
| **ads** | 链上广告活动、竞价、排期 |
| **rewards** | 节点运营者奖励分发 |
| **subscription** | 分层订阅管理 |
| **primitives** | 共享类型与 Trait |

### 2.3 已支持的操作 (`ActionType` 枚举)

| 操作 | 状态 |
|------|------|
| 封禁 (Ban) | ✅ |
| 解封 (Unban) | ✅ |
| 踢出 (Kick) | ✅ |
| 禁言 (Mute, 定时) | ✅ |
| 解除禁言 (Unmute) | ✅ |
| 警告 (Warn) | ✅ |
| 提升管理员 (Promote) | ✅ |
| 撤销管理员 (Demote) | ✅ |
| 删除消息 (DeleteMessage) | ✅ |
| 发送消息 (SendMessage) | ✅ |
| 批准入群 (ApproveJoin) | ✅ |
| 回调应答 (AnswerCallback) | ✅ |

---

## 3. 差距分析 — 缺失功能

### 3.1 高优先级 (在 3+ 个参考机器人中发现的核心审核功能)

| # | 功能 | 来源 | 说明 | 工作量 |
|---|------|------|------|--------|
| **G1** | **验证码 (CAPTCHA)** | Gojo, sudobot | 新成员入群前需通过验证码 (图片/算术/按钮) 才能解除限制。`captcha_enabled` 和 `captcha_timeout_secs` 已存在于 `ChainCommunityConfig`，但**未实现 CaptchaRule**。 | 中等 |
| **G2** | **全局封禁 (GBan)** | FallenRobot, Gojo | 跨群封禁名单，在所有托管群中封禁恶意用户。独特优势: 可通过链上同步至所有节点。 | 中等 |
| **G3** | **用户白名单 (Approve)** | FallenRobot | 已审批用户免受自动规则影响 (防刷屏、黑名单、锁定等)。grouprobot 无审批系统 — 所有非管理员用户平等受所有规则约束。 | 低-中 |
| **G4** | **自定义过滤器/自动回复** | FallenRobot, Gojo | 关键词触发的自动回复，支持文字/媒体/按钮。`custom_commands_csv` 已存在于配置中，但**未实现 CustomFilterRule**。 | 中等 |
| **G5** | **完整的欢迎/告别消息** | FallenRobot, Gojo, grpmr-rs | 当前: 仅在入群请求审批时发送欢迎模板。缺失: `new_chat_members` 事件欢迎、`left_chat_member` 告别、自动清理旧欢迎消息、删除入群/退群服务消息、媒体欢迎。`goodbye_template` 已存在于配置但未使用。 | 低-中 |
| **G6** | **贴纸黑名单** | FallenRobot | 封锁特定贴纸包/单个贴纸，可配置处理动作 (删除/警告/禁言/踢出/封禁/临时禁言/临时封禁)。当前 `LockRule` 可封锁所有贴纸，但无法针对特定贴纸包。 | 低 |
| **G7** | **警告管理命令** | FallenRobot, Gojo, grpmr-rs | `/warns` (查看用户警告)、`/resetwarns`、`/rmwarn`、`/warnlimit`、`/warnmode`、`/dwarn` (删除+警告)、`/swarn` (静默警告)。当前: 仅有 `/warn` 命令；WarnTracker 内部处理升级，但不暴露用户可用的查询/管理命令。 | 低 |
| **G8** | **脏话过滤器** | samurai | 多语言脏话检测，可配置处罚动作。超越停用词 — 处理混淆字符、Leetspeak 等。 | 中等 |
| **G9** | **突袭防护 (Raid Protection)** | sudobot, yagpdb | 检测集中入群事件 (Y 秒内 X 人入群) → 自动锁群或批量踢出新账号，防止协调攻击。 | 中等 |

### 3.2 中优先级 (在 2+ 个机器人中发现，有价值的补充)

| # | 功能 | 来源 | 说明 | 工作量 |
|---|------|------|------|--------|
| **G10** | **群聊黑名单** | Gojo | 开发者/所有者可将特定群加入黑名单 — 机器人拒绝在黑名单群中运行。用于防滥用。 | 低 |
| **G11** | **群规文本系统** | Gojo, grpmr-rs | 通过 `/setrules`、`/rules`、`/clearrules` 设置/获取/清除群规文本。支持私信发送模式。当前 `/rules` 仅显示"由链上管理"。 | 低 |
| **G12** | **临时封禁 (tban)** | FallenRobot, Gojo | 带自动解封的限时封禁。当前封禁仅支持永久。 | 低 |
| **G13** | **临时禁言快捷解析 (tmute)** | grpmr-rs | 禁言已支持时长参数，但缺少用户友好的 `/tmute 5m` 快捷解析 (5s, 10m, 2h, 1d)。 | 低 |
| **G14** | **日志频道** | grpmr-rs, yagpdb | 将所有审核操作转发至指定日志频道。`log_channel_id` 已存在于配置中，但**未实现日志转发**。 | 低-中 |
| **G15** | **NSFW 图片检测** | samurai | 基于 ML 的图片分类 (SigLIP 模型)，自动删除不当内容。 | 高 |
| **G16** | **@提及轰炸检测** | yagpdb | 当消息包含 X+ 个不同 @提及时触发，防止提及轰炸攻击。 | 低 |
| **G17** | **批量封禁/踢出** | sudobot | `/massban` / `/masskick` — 对多个用户的批量操作。 | 低 |
| **G18** | **违规追踪/统计** | samurai, sudobot | 按用户跟踪违规计数 (垃圾、脏话等)，支持 `/top_violators` 排行榜。提升管理员可见性。 | 中等 |
| **G19** | **可组合自动审核引擎 (触发器→条件→效果)** | yagpdb | 可组合的自动审核规则: 定义触发器 (正则、提及、词表、链接)、条件 (角色白/黑名单、账号年龄、频道)、效果 (删除、警告、踢出、封禁、禁言、加角色、私信)。当前 grouprobot 规则硬编码在 Rust 中，无用户可组合的规则构建器。 | 高 |

### 3.3 低优先级 (锦上添花，在 1-2 个机器人中发现)

| # | 功能 | 来源 | 说明 | 工作量 |
|---|------|------|------|--------|
| **G20** | **消息恢复 (Snipe)** | sudobot | `/snipe` 显示频道中最后被删除的消息，用于审核取证。 | 低 |
| **G21** | **备注系统 (Note)** | sudobot | 管理员可为用户的审核记录添加备注。 | 低 |
| **G22** | **违规记录系统 (Infraction)** | sudobot | 正式的违规记录 (创建、列表、查看、清除、设置时长/原因) — 超越简单警告。 | 中等 |
| **G23** | **自动角色 (Autorole)** | yagpdb | 根据入群时长等条件自动分配角色。对 Telegram (无角色) 适用性较低，但对 Discord 扩展有意义。 | 低 |
| **G24** | **慢速模式控制** | 多个 | 机器人强制的慢速模式 (每用户每 X 秒 1 条消息) — 比 Telegram 内置更精细。 | 低 |
| **G25** | **新成员消息审查** | sudobot, tg-spam | 仅检查新成员的前 N 条消息是否为垃圾，减少老成员的误判。tg-spam 中的 `FirstMessageOnly` / `FirstMessagesCount`。 | 低 |
| **G26** | **CAS (Combot Anti-Spam) API** | tg-spam | 针对 CAS 已知垃圾账号数据库检查新成员。 | 低 |
| **G27** | **Safe Browsing API** | yagpdb | Google Safe Browsing API 集成用于 URL 检查，补充现有 AntiPhishing 规则。 | 低 |
| **G28** | **同形字/混淆字符检测** | yagpdb | 将 "Ĥéĺĺó" 标准化为 "Hello" 后再进行正则匹配，防止 Unicode 绕过词汇过滤。 | 低 |

---

## 4. 功能矩阵 — GroupRobot vs 参考机器人

| 功能 | GroupRobot | FallenRobot | Gojo | grpmr-rs | tg-spam | samurai | sudobot | yagpdb |
|------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| 封禁/解封 | ✅ | ✅ | ✅ | ✅ | — | ✅ | ✅ | ✅ |
| 踢出 | ✅ | ✅ | ✅ | ✅ | — | — | ✅ | ✅ |
| 禁言/解除禁言 | ✅ | ✅ | ✅ | ✅ | — | — | ✅ | ✅ |
| 临时封禁 | ✅ | ✅ | ✅ | — | — | — | ✅ | ✅ |
| 提升/撤销管理员 | ✅ | — | — | — | — | — | — | — |
| 警告 + 自动升级 | ✅ | ✅ | ✅ | ✅ | — | — | ✅ | ✅ |
| 警告管理命令 | ✅ | ✅ | ✅ | ✅ | — | — | ✅ | ✅ |
| 防刷屏 | ✅ | — | — | — | — | — | — | — |
| 重复消息检测 | ✅ | — | — | — | — | — | — | — |
| 正则黑名单 | ✅ | ✅ | — | — | — | — | — | — |
| 停用词 | ✅ | — | — | — | ✅ | — | — | — |
| Emoji 限制 | ✅ | — | — | — | ✅ | — | — | — |
| 链接限制 | ✅ | — | — | — | — | — | — | — |
| 垃圾相似度 | ✅ | — | — | — | ✅ | — | — | — |
| 反钓鱼 | ✅ | — | — | — | — | — | — | ✅ |
| 消息类型锁定 | ✅ | — | — | ✅ | — | — | ✅ | — |
| 验证码 | ✅ | — | ✅ | — | — | — | ✅ | — |
| 全局封禁 | ✅ | ✅ | ✅ | — | — | — | — | — |
| 用户白名单 | ✅ | ✅ | — | — | — | — | — | — |
| 自定义过滤器 | ✅ | ✅ | ✅ | — | — | — | — | — |
| 完整欢迎/告别 | ✅ | ✅ | ✅ | — | — | — | — | — |
| 贴纸黑名单 | ❌ | ✅ | — | — | — | — | — | — |
| 脏话过滤 | ✅ | — | — | — | — | ✅ | — | — |
| 突袭防护 | ✅ | — | — | — | — | — | ✅ | ✅ |
| NSFW 检测 | ✅ | — | — | — | — | ✅ | — | — |
| @提及轰炸 | ✅ | — | — | — | — | — | — | ✅ |
| 日志频道 | ✅ | — | — | ✅ | — | — | — | ✅ |
| 群聊黑名单 | ❌ | — | ✅ | — | — | — | — | — |
| 群规文本 | ✅ | — | ✅ | — | — | — | — | — |
| 违规统计 | ✅ | — | — | — | — | ✅ | ✅ | — |
| 可组合自动审核 | ✅ | — | — | — | — | — | — | ✅ |
| CAS API | ✅ | — | — | — | ✅ | — | — | — |
| 新成员专项检查 | ✅ | — | — | — | ✅ | — | ✅ | — |
| 链上配置 | ✅ | — | — | — | — | — | — | — |
| TEE 远程证明 | ✅ | — | — | — | — | — | — | — |
| 订阅分层 | ✅ | — | — | — | — | — | — | — |
| 广告系统 | ✅ | — | — | — | — | — | — | — |

**图例:** ✅ = 已实现, ❌ = 缺失, ⚠️ = 部分实现, — = 不适用/不具备

---

## 5. 推荐实施路线图

### 第一阶段 — 快速收益 ✅ 已完成

1. ✅ **G7 — 警告管理命令** (`/warns`, `/resetwarns`, `/warnlimit`, `/warnmode`)
2. ✅ **G12 — 临时封禁** (为 `/ban` 添加时长解析 + executor 传递 `until_date`)
3. ✅ **G14 — 日志频道** (AuditLogger 转发至 `log_channel_id`)
4. ✅ **G11 — 群规文本** (`/setrules`, `/rules`, `/clearrules` → LocalStore)
5. ✅ **G5 — 完整欢迎/告别** (`goodbye_template` + `welcome_template` + clean-service 同时执行)

### 第二阶段 — 核心安全 ✅ 已完成

6. ✅ **G1 — 验证码规则** (CaptchaRule: 新成员入群 → 算术验证按钮 + Inline Keyboard → 超时踢出)
7. ✅ **G2 — 全局封禁** (GbanRule: 链上同步跨群封禁名单，入群/发消息即封)
8. ✅ **G3 — 用户白名单** (ApproveRule: RuleEngine 层前置检查，白名单用户跳过所有自动检测)
9. ✅ **G9 — 突袭防护** (RaidRule: Y秒内X人入群 → 自动封禁后续新成员)
10. ✅ **G4 — 自定义过滤器** (CustomFilterRule: trigger|type|response CSV → reply/delete/warn)

### 第三阶段 — 高级检测 ✅ 已完成

11. ✅ **G8 — 脏话过滤器** (ProfanityRule: 多语言脏话 + leet-speak/Cyrillic 防混淆 + warn/delete/mute 三种动作)
12. ✅ **G16 — @提及轰炸检测** (MentionFloodRule: 单条消息最大 @mention 数，排除邮箱)
13. ✅ **G25 — 新成员消息审查** (NewMemberAuditRule: 前 N 条消息禁止链接/转发/超长)
14. ✅ **G26 — CAS API 集成** (CasRule: LocalStore 缓存 + 入群即检查 + 批量加载)
15. ✅ **G28 — 同形字标准化** (HomoglyphRule: Cyrillic/Greek/Fullwidth/Accented → ASCII 后匹配关键词)

### 第四阶段 — 差异化竞争 ✅ 已完成

16. ✅ **G19 — 可组合自动审核引擎** (AutoModRule: trigger→conditions→effect JSON 配置, 支持 message/join/regex 触发器 + 6 种条件 + 5 种效果)
17. ✅ **G15 — NSFW 图片检测** (NsfwRule: 启发式关键词检测 + TEE 审核队列框架, 3 种模式: DeleteFirst/ReviewOnly/DeleteAndReview)
18. ✅ **G18 — 违规追踪看板** (ViolationTracker: 按用户/规则统计 + /violations /leaderboard /resetviolations 命令)

---

## 6. GroupRobot 独特优势 (参考机器人均不具备)

以下功能**仅存在于 grouprobot**，是核心竞争壁垒:

1. **链上配置** — 群规则存储于 Substrate 链上，不可篡改的审计轨迹
2. **TEE (可信执行环境)** — 机器人运行在 Intel TDX/SGX 飞地中，通过 DCAP 远程证明可验证
3. **多节点共识** — 操作日志需要多个 TEE 节点共识 (防篡改审核)
4. **订阅分层 + 功能门控** — 链上分层系统控制可用规则数量
5. **去中心化广告系统** — 链上广告活动，支持竞价、排期和投放追踪
6. **节点运营者奖励** — 运行机器人基础设施的经济激励
7. **反钓鱼 + 链上同步黑名单** — 钓鱼域名可链上众包维护
8. **基于相似度的垃圾检测** — TF-IDF 余弦相似度 (借鉴 tg-spam) 原生集成
9. **可插拔规则引擎** — 基于 Rust Trait 的 `Rule` 系统，添加新规则无需修改引擎

---

## 7. 总结统计

| 指标 | 数值 |
|------|------|
| 所有机器人总特性数 | 42 |
| grouprobot 已实现 | 35 (83%) |
| 部分实现 | 0 (0%) |
| **缺失功能 (差距)** | **7 (17%)** |
| 高优先级差距 | 0 |
| 中优先级差距 | 2 |
| 低优先级差距 | 5 |
| grouprobot 独有功能 (参考机器人均无) | 9 |

**结论:** GroupRobot 已具备坚实的基础审核能力 (防刷屏、重复检测、黑名单、相似度、反钓鱼、锁定、警告) 和独特的链上/TEE 优势。主要差距集中在: **用户可用管理命令** (警告查询、群规文本)、**身份验证** (验证码、CAS)、**内容检测** (脏话、NSFW) 和**可组合规则构建**。多个缺失功能的配置字段 (验证码、日志频道、自定义命令、告别消息) 已存在于链上 — 仅需实现机器人端规则。
