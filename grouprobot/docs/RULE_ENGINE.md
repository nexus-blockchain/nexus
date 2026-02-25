# 规则引擎

> 版本: 0.1.0 | 最后更新: 2026-02-23

## 1. 设计理念

GroupRobot 的规则引擎采用**可插拔责任链模式**: 每条消息按优先级依次经过所有规则，第一个匹配的规则产生 `ActionDecision`，后续规则不再评估。

```
消息 → [CallbackRule] → [FloodRule] → [DuplicateRule] → [BlacklistRule]
     → [StopWordRule] → [EmojiRule] → [LinkLimitRule] → [SimilarityRule]
     → [AntiPhishingRule] → [LockRule] → [CommandRule] → [JoinRequestRule]
     → [DefaultRule] → 无动作
                ↓ (如果匹配)
         ActionDecision
                ↓
         WarnTracker 后处理 (累积计数 + 自动升级)
                ↓
         RuleDecision { matched_rule, action }
```

### Rule Trait

```rust
// processing/rules/mod.rs
#[async_trait]
pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    async fn evaluate(&self, ctx: &MessageContext, store: &LocalStore) -> Option<ActionDecision>;
}
```

- 返回 `Some(ActionDecision)` → 规则匹配，执行动作
- 返回 `None` → 不匹配，传递给下一个规则

## 2. 规则列表 (按执行顺序)

| # | 规则 | 文件 | 优先级 | 触发条件 | 默认动作 |
|---|------|------|--------|---------|---------|
| 0 | CallbackRule | `callback.rs` | 最高 | `callback_data` 非空 | AnswerCallback |
| 1 | FloodRule | `flood.rs` | 高 | 同一用户短时间内发送超过 `flood_limit` 条消息 | Mute 60s |
| 2 | DuplicateRule | `duplicate.rs` | 高 | 同一用户在 `window_secs` 内发送 ≥ `threshold` 条相同消息 | Warn |
| 3 | BlacklistRule | `blacklist.rs` | 高 | 消息匹配任一正则表达式 | DeleteMessage |
| 4 | StopWordRule | `stop_word.rs` | 中 | 消息包含停用词 (逗号分隔列表) | DeleteMessage |
| 5 | EmojiRule | `emoji.rs` | 中 | Emoji 数量超过 `max_emoji` | DeleteMessage |
| 6 | LinkLimitRule | `link_limit.rs` | 中 | URL 数量超过 `max_links` | DeleteMessage |
| 7 | SimilarityRule | `similarity.rs` | 中 | TF-IDF 余弦相似度超过阈值 (与 spam 样本比较) | DeleteMessage + Warn |
| 8 | AntiPhishingRule | `antiphishing.rs` | 中 | 多层检测: URL 域名仿冒 + 内容模式 + 紧急度评分 | Ban / DeleteMessage |
| 9 | LockRule | `lock.rs` | 中 | 消息类型在锁定列表中 (如 photo, sticker) | DeleteMessage |
| 10 | CommandRule | `command.rs` | 低 | 消息以 `/` 开头 | 依命令而定 |
| 11 | JoinRequestRule | `join.rs` | 低 | `is_join_request == true` | ApproveJoin + SendMessage |
| 12 | DefaultRule | `default.rs` | 最低 | 始终不匹配 | None (兜底) |

## 3. 规则详解

### 3.1 CallbackRule — Inline 键盘回调

**触发**: `ctx.callback_data` 非空 (来自 Telegram Inline Keyboard 按钮点击)

**数据格式**: `action:target[:extra]`
- `warn:123456` — 对用户 123456 发出警告
- `ban:123456` — 封禁用户 123456
- `approve:123456` — 批准用户 123456

**权限**: 解析 `action` 后执行对应管理动作，仅管理员有效 (由 `is_admin` 校验)。

### 3.2 FloodRule — 防刷屏

**配置**: `anti_flood_enabled`, `flood_limit` (默认 100)

**机制**: 使用 `LocalStore` 的滑动窗口计数器，键为 `flood:{group_id}:{sender_id}`。每条消息 +1，超过 `flood_limit` 则触发。

**动作**: `Mute(sender_id, 60s, "flood detected")`

### 3.3 DuplicateRule — 重复消息检测

**配置**: `anti_duplicate_enabled`, `duplicate_window_secs` (默认 300), `duplicate_threshold` (默认 3)

**机制**: 对消息文本计算 SHA-256 哈希，存入 `LocalStore`，键为 `dup:{group_id}:{sender_id}:{hash}`。同一哈希在窗口内出现 ≥ threshold 次触发。

**动作**: `Warn(sender_id, "重复消息")`

### 3.4 BlacklistRule — 正则黑名单

**配置**: 正则表达式列表 (从链上 `ChainCommunityConfig` 获取)

**机制**: 对每条消息逐一匹配所有正则。使用 `(?i)` 标志忽略大小写。

**动作**: `DeleteMessage(message_id)`

### 3.5 StopWordRule — 停用词

**配置**: `stop_words` (逗号分隔字符串, 如 `"scam,fraud,fake"`)

**机制**: 将消息转小写后检查是否包含任一停用词 (子串匹配)。

**动作**: `DeleteMessage(message_id)`

### 3.6 EmojiRule — Emoji 数量限制

**配置**: `max_emoji` (0 = 不限制)

**机制**: 使用 Unicode 属性检测 Emoji 字符数量。

**动作**: `DeleteMessage(message_id)` (超限时)

### 3.7 LinkLimitRule — 链接数量限制

**配置**: `max_links` (0 = 不限制)

**机制**: 使用正则 `https?://\S+` 计数 URL 数量。

**动作**: `DeleteMessage(message_id)` (超限时)

### 3.8 SimilarityRule — TF-IDF 垃圾消息检测

**配置**: `spam_samples` (换行分隔的 spam 样本), `similarity_threshold` (0-100, 默认 70)

**算法**:
1. 对 spam 样本集构建 TF-IDF 向量
2. 对每条消息计算 TF-IDF 向量
3. 计算余弦相似度
4. 相似度 ≥ threshold / 100.0 → 触发

**动作**: `DeleteMessage` + `Warn`

### 3.9 AntiPhishingRule — 反钓鱼

**配置**: `antiphishing_enabled`

**多层检测**:
1. **URL 域名分析**: 检测 Levenshtein 距离相近的仿冒域名 (如 `te1egram.org`)
2. **已知钓鱼域名列表**: 内置常见钓鱼域名黑名单
3. **内容模式匹配**: 检测紧急行动号召 + 凭证请求等典型钓鱼话术
4. **综合评分**: 多维度加权评分，超过阈值触发

**动作**: 高风险 → `Ban`; 中风险 → `DeleteMessage` + `Warn`

### 3.10 LockRule — 消息类型锁定

**配置**: `locked_types_csv` (逗号分隔, 如 `"photo,video,sticker"`)

**支持锁定的类型**: `photo`, `video`, `audio`, `document`, `sticker`, `animation`, `voice`, `forward`, `contact`, `location`, `poll`, `game`

**豁免**: 管理员消息不受锁定限制 (`ctx.is_admin == true` 时跳过)。

**动作**: `DeleteMessage(message_id)`

### 3.11 CommandRule — 管理命令

**三级命令体系**:

| 级别 | 命令 | 权限 | 说明 |
|------|------|------|------|
| 公开 | `/help` | 所有人 | 显示帮助 |
| | `/id` | 所有人 | 显示群/用户 ID |
| | `/rules` | 所有人 | 显示群规则 |
| 管理员 (无目标) | `/locks` | admin | 查看锁定的消息类型 |
| | `/lock <type>` | admin | 锁定消息类型 |
| | `/unlock <type>` | admin | 解锁消息类型 |
| 管理员 (有目标) | `/ban <user_id>` | admin | 永久封禁 |
| | `/kick <user_id>` | admin | 踢出 |
| | `/mute <user_id> [秒数]` | admin | 禁言 (默认 3600s) |
| | `/unmute <user_id>` | admin | 解除禁言 |
| | `/unban <user_id>` | admin | 解封 |
| | `/warn <user_id>` | admin | 警告 |
| | `/promote <user_id>` | admin | 提升管理员 |
| | `/demote <user_id>` | admin | 降级管理员 |

**权限校验**: 管理员命令检查 `ctx.is_admin`，非管理员发出的管理命令被忽略 (返回 None，传递给下一个规则)。

### 3.12 JoinRequestRule — 入群审批

**配置**: `welcome_enabled`, `welcome_template`

**行为**:
- 收到 `join_request` 事件 → 自动批准 (`ApproveJoin`)
- 如果启用欢迎消息 → 发送模板消息 (`SendMessage`)
- 模板变量: `{user}` (用户名), `{group}` (群名)

## 4. WarnTracker — 警告累积与升级

`WarnTracker` 不是规则，而是**后处理器**，对所有产生 `Warn` 动作的规则结果进行二次处理。

**配置**:
- `warn_limit` (累积上限, 0 = 禁用)
- `warn_action` (达到上限后的升级动作: 0=Mute, 1=Kick, 2=Ban)
- `warn_mute_duration` (Mute 时长, 默认 3600s)

**流程**:
```
规则评估 → Warn 动作
    ↓
WarnTracker.process()
    ├─ 累积计数 +1 (存储在 LocalStore, 键: warn:{group}:{user})
    ├─ 未达上限 → 返回原 Warn 动作 (附带 "⚠️ 警告 N/M")
    └─ 达到上限 → 升级为 Mute/Kick/Ban + 清零计数
```

## 5. AutoMod 三段式引擎

除了基础规则引擎，GroupRobot 还实现了**三段式 AutoMod 引擎** (借鉴 YAGPDB):

```
Trigger (触发器)  →  Condition (条件)  →  Effect (效果)
   任一匹配           全部满足            依序执行
```

### Trigger 类型 (`automod/triggers.rs`)

| 触发器 | 说明 |
|--------|------|
| `MessageTrigger` | 消息内容匹配 (正则/关键词/前缀) |
| `JoinTrigger` | 新成员加入 |
| `MessageCountTrigger` | 消息数量达到阈值 |
| `UserAgeTrigger` | 账号年龄检查 |

### Condition 类型 (`automod/conditions.rs`)

| 条件 | 说明 |
|------|------|
| `RoleCondition` | 用户角色过滤 (管理员豁免) |
| `ChannelCondition` | 频道/群组过滤 |
| `CooldownCondition` | 冷却时间 (防止频繁触发) |

### Effect 类型 (`automod/effects.rs`)

| 效果 | 说明 |
|------|------|
| `WarnEffect` | 发送警告 |
| `MuteEffect` | 禁言 |
| `KickEffect` | 踢出 |
| `BanEffect` | 封禁 |
| `DeleteEffect` | 删除消息 |
| `SendMessageEffect` | 发送消息 |

### 组合示例

```rust
AutoModRule::new(1, "anti-spam-link")
    .trigger(Box::new(MessageTrigger::contains("t.me/")))
    .condition(Box::new(RoleCondition::not_admin()))
    .condition(Box::new(CooldownCondition::seconds(60)))
    .effect(Box::new(DeleteEffect))
    .effect(Box::new(WarnEffect::new("请勿发送推广链接")))
```

## 6. 自定义命令系统

群主可通过链上配置定义自定义命令和关键词过滤器。

**配置**: `custom_commands_csv` (格式: `trigger|type|response`)

### 命令类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `ExactCommand` | `/trigger` 精确匹配 | `/faq\|exact\|请查看 FAQ 页面` |
| `KeywordFilter` | 消息包含关键词 | `discord\|keyword\|请勿发送 Discord 链接` |
| `RegexFilter` | 正则匹配 | `t\.me/\S+\|regex\|请勿发送推广链接` |

**限制**: 每个群最多 50 个自定义命令。

## 7. CAPTCHA 验证系统

**配置**: `captcha_enabled`, `captcha_timeout_secs` (默认 120s)

### Level 1: 数学题验证

新用户入群后，Bot 发送随机数学题 (加减法)，用户需在超时前回答正确:

```
Bot: 🔒 欢迎！请在 120 秒内回答: 15 + 27 = ?
User: 42
Bot: ✅ 验证通过！
```

超时未回答 → Kick

### Level 2: RA-TLS 网页验证 (预留)

接口已定义，未来实现。利用 TEE RA-TLS 签名验证链接，结果上链。

## 8. 链上配置映射

`RuleEngine::from_config()` 从 `ChainCommunityConfig` 构建规则链:

| 链上字段 | 规则 | 条件 |
|---------|------|------|
| `anti_flood_enabled` + `flood_limit` | FloodRule | enabled=true |
| `anti_duplicate_enabled` + `duplicate_window_secs` + `duplicate_threshold` | DuplicateRule | enabled=true |
| (外部传入 blacklist_patterns) | BlacklistRule | patterns 非空 |
| `stop_words` | StopWordRule | 非空字符串 |
| `max_emoji` | EmojiRule | > 0 |
| `max_links` | LinkLimitRule | > 0 |
| `spam_samples` + `similarity_threshold` | SimilarityRule | samples 非空 |
| `antiphishing_enabled` | AntiPhishingRule | enabled=true |
| `locked_types_csv` | LockRule | 非空字符串 |
| (始终添加) | CommandRule | — |
| `welcome_enabled` + `welcome_template` | JoinRequestRule | — |
| (始终添加) | DefaultRule | — |
| `warn_limit` + `warn_action` + `warn_mute_duration` | WarnTracker | warn_limit > 0 |

## 9. 测试覆盖

每个规则均有独立单元测试，覆盖正向/反向/边界场景:

| 模块 | 测试数 | 关键场景 |
|------|--------|---------|
| `rule_engine.rs` | ~10 | 基本流水线、命令触发、黑名单 |
| `flood.rs` | ~4 | 频率触发、正常通过 |
| `duplicate.rs` | ~6 | 窗口内重复、窗口外重置 |
| `blacklist.rs` | ~4 | 正则匹配、大小写 |
| `command.rs` | ~12 | 三级命令、@bot 后缀、权限校验 |
| `lock.rs` | ~7 | 类型锁定、管理员豁免 |
| `callback.rs` | ~6 | 回调解析、action 路由 |
| `similarity.rs` | ~8 | TF-IDF 精度、阈值边界 |
| `antiphishing.rs` | ~10 | 域名仿冒、模式匹配 |
| `warn_tracker.rs` | ~6 | 累积计数、升级动作 |
| `join.rs` | ~5 | 审批、欢迎模板 |
