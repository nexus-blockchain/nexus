# GroupRobot

GroupRobot 离链 TEE 执行程序 — 在 TDX+SGX 可信执行环境中运行的群管理机器人，与链上 `grouprobot-*` Pallet (index 150-153) 通过 subxt 交互。

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│                     HTTP Entry (Axum)                        │
│  POST /webhook    GET /health      GET /v1/status            │
│  GET /metrics     POST /ceremony/* GET/POST /provision/*     │
├──────────────────────────────────────────────────────────────┤
│                Message Processing Layer                      │
│  Router → RuleEngine → [30+ 可插拔规则]                       │
│  AutoMod (Trigger→Condition→Effect) │ CAPTCHA │ AuditLogger  │
│  AdDeliveryLoop (排期→投放→链上收据) │ CustomCommands          │
├───────────────────┬──────────────────────────────────────────┤
│  Platform Layer   │      Chain Interaction (subxt)           │
│  ┌──────────────┐ │  ┌──────────────────────────────┐       │
│  │  Telegram    │ │  │  ChainClient (WebSocket RPC) │       │
│  │  Adapter     │ │  │  Queries (Bot/TEE/社区配置)   │       │
│  │  Executor    │ │  │  Transactions + LogBatcher    │       │
│  ├──────────────┤ │  │  DCAP Level 4 证明提交        │       │
│  │  Discord     │ │  └──────────────────────────────┘       │
│  │  Adapter     │ │                                         │
│  │  Executor    │ │      TEE Security Layer (17 文件)        │
│  │  Gateway     │ │  ┌──────────────────────────────┐       │
│  └──────────────┘ │  │  EnclaveBridge (Ed25519)      │       │
│                   │  │  SealedStorage (AES-256-GCM)  │       │
│                   │  │  TokenVault (Zeroizing+mlock)  │       │
│                   │  │  Attestor (DCAP L1-L4)        │       │
│                   │  │  Shamir (GF(256) K-of-N)      │       │
│                   │  │  Ceremony + RA-TLS Provision   │       │
│                   │  │  PeerMonitor (Re-ceremony)    │       │
│                   │  │  VaultIPC (加密 Unix socket)   │       │
│                   │  │  MemSecurity (mlock/coredump) │       │
│                   │  └──────────────────────────────┘       │
├───────────────────┴──────────────────────────────────────────┤
│                     Infrastructure                           │
│  Metrics (Prometheus) │ RateLimiter (全局+per-group)          │
│  LocalStore (DashMap)  │ ConfigManager (链上同步)              │
│  AudienceTracker (CPM audience 统计, 多层反作弊)               │
└──────────────────────────────────────────────────────────────┘
```

## 目录结构

```
grouprobot/
├── Cargo.toml
├── .env.example
├── README.md
├── docs/                               # 详细设计文档
│   ├── API_REFERENCE.md                #   HTTP API 完整参考
│   ├── ARCHITECTURE.md                 #   架构设计
│   ├── CHAIN_INTEGRATION.md            #   链上交互详解
│   ├── PLATFORM_ADAPTERS.md            #   多平台适配器
│   ├── RULE_ENGINE.md                  #   规则引擎设计
│   └── TEE_SECURITY.md                #   TEE 安全模型
├── gramine/                            # SGX Gramine 部署
│   ├── token-vault.manifest.template
│   └── README.md
├── scripts/                            # 运维脚本
│   ├── extract-mrtd.sh                 #   提取 MRTD 度量值
│   └── rolling-upgrade.sh             #   滚动升级自动化
└── src/                                # 86 个 Rust 源文件
    ├── main.rs                         # 入口 + AppState + 任务编排 (700 行)
    ├── config.rs                       # 环境变量配置 (敏感字段 Debug 脱敏)
    ├── error.rs                        # 统一错误类型 (thiserror)
    ├── webhook.rs                      # HTTP 入口 (Secret 验证/限流/去重/路由)
    │
    ├── chain/                          # Substrate 链客户端 (5 文件)
    │   ├── client.rs                   #   ChainClient 连接 + 签名者管理
    │   ├── queries.rs                  #   链上查询 (Bot/TEE/序列/社区配置/Peer)
    │   ├── transactions.rs             #   链上交易 + ActionLogBatcher + DCAP L4
    │   └── types.rs                    #   链交互类型 (BotInfoCache/PeerInfo 等)
    │
    ├── tee/                            # TEE 安全层 (17 文件)
    │   ├── enclave_bridge.rs           #   SGX/Software Enclave 桥接 (Ed25519)
    │   ├── sealed_storage.rs           #   AES-256-GCM 密封存储 (V0/V1 格式)
    │   ├── key_manager.rs              #   Ed25519 密钥 + 原子序列号
    │   ├── attestor.rs                 #   TDX+SGX 双证明 (nonce 防重放, 24h 有效)
    │   ├── token_vault.rs              #   Token 保险库 (Zeroizing + mlock)
    │   ├── shamir.rs                   #   GF(256) Shamir K-of-N 秘密分片
    │   ├── ceremony.rs                 #   RA-TLS 仪式 (Share 分发/接收/Migration)
    │   ├── share_recovery.rs           #   5 级 Token 恢复链
    │   ├── peer_client.rs              #   Peer TEE 节点 HTTP + RA-TLS 通信
    │   ├── peer_monitor.rs             #   Peer 健康监控 + 自动 Re-ceremony
    │   ├── ra_tls.rs                   #   RA-TLS Provision (DApp→TEE Token 注入)
    │   ├── dcap_verify.rs              #   DCAP Quote 链下验证框架
    │   ├── vault_ipc.rs                #   IPC 加密协议 (AES-GCM + 方向前缀)
    │   ├── vault_server.rs             #   Vault Unix socket 服务端
    │   ├── vault_client.rs             #   Vault Unix socket 客户端
    │   └── mem_security.rs             #   内存加固 (core dump/mlock/dumpable)
    │
    ├── platform/                       # 多平台适配层 (8 文件)
    │   ├── mod.rs                      #   PlatformEvent / PlatformAdapter trait
    │   │                               #   PlatformExecutor trait + 16 种 ActionType
    │   ├── telegram/
    │   │   ├── adapter.rs              #   TG Update → PlatformEvent
    │   │   └── executor.rs             #   TG Bot API 执行器 (Local Server 支持)
    │   └── discord/
    │       ├── adapter.rs              #   DC Gateway Event → PlatformEvent
    │       ├── executor.rs             #   DC REST API 执行器
    │       └── gateway.rs              #   DC WebSocket (IDENTIFY/RESUME/心跳)
    │
    ├── processing/                     # 消息处理层 (46 文件)
    │   ├── router.rs                   #   MessageRouter (事件→规则→执行→日志)
    │   ├── rule_engine.rs              #   可插拔规则引擎 (链上配置动态重建)
    │   ├── action.rs                   #   ActionDecision / RuleDecision
    │   ├── normalizer.rs               #   SHA256 哈希工具
    │   ├── ad_delivery.rs              #   广告投放循环 (排期→投放→链上收据)
    │   ├── captcha.rs                  #   CAPTCHA 验证 (数学题 / RA-TLS 网页)
    │   ├── audit_logger.rs             #   审计日志 (管理动作→日志频道+链上哈希)
    │   ├── custom_commands.rs          #   自定义命令/关键词过滤器 (链上同步)
    │   ├── automod/                    #   可组合自动审核引擎
    │   │   ├── triggers.rs             #     触发器 (message/join/regex)
    │   │   ├── conditions.rs           #     条件 (has_link/is_new_member/...)
    │   │   ├── effects.rs              #     效果 (delete/warn/mute/ban/kick)
    │   │   └── engine.rs               #     引擎 (JSON 配置→规则编译→执行)
    │   └── rules/                      #   30+ 可插拔规则
    │       ├── flood.rs                #     防刷屏 (滑动窗口计数)
    │       ├── blacklist.rs            #     正则黑名单
    │       ├── command.rs              #     管理指令 (/ban /mute /kick /warn)
    │       ├── join.rs                 #     入群审批 + 欢迎消息
    │       ├── default.rs              #     兜底放行
    │       ├── duplicate.rs            #     重复消息检测
    │       ├── emoji.rs                #     Emoji 数量限制
    │       ├── link_limit.rs           #     链接数量限制
    │       ├── stop_word.rs            #     停用词匹配
    │       ├── similarity.rs           #     TF-IDF 垃圾检测
    │       ├── classifier.rs           #     朴素贝叶斯垃圾分类器
    │       ├── antiphishing.rs         #     反钓鱼链接检测
    │       ├── lock.rs                 #     消息类型锁定
    │       ├── callback.rs             #     Inline 键盘回调
    │       ├── warn_tracker.rs         #     警告累积 + 自动升级
    │       ├── ad_footer.rs            #     免费层广告页脚
    │       ├── automod.rs              #     AutoMod 可组合规则桥接
    │       ├── captcha.rs              #     CAPTCHA 规则集成
    │       ├── cas.rs                  #     Combot Anti-Spam 集成
    │       ├── custom_filter.rs        #     自定义关键词过滤器
    │       ├── gban.rs                 #     全局封禁名单 (链上同步)
    │       ├── homoglyph.rs            #     形近字/Unicode 混淆检测
    │       ├── profanity.rs            #     脏话过滤
    │       ├── nsfw.rs                 #     NSFW 媒体检测 (TEE ML 审核队列)
    │       ├── mention_flood.rs        #     @mention 轰炸检测
    │       ├── new_member_audit.rs     #     新成员审查
    │       ├── raid.rs                 #     Raid 攻击检测
    │       ├── log_channel.rs          #     日志频道转发
    │       ├── approve.rs              #     入群审批规则
    │       ├── violation_tracker.rs    #     违规追踪 (/violations /leaderboard)
    │       └── text_utils.rs           #     文本工具 (HTML 转义/分词)
    │
    └── infra/                          # 基础设施 (6 文件)
        ├── metrics.rs                  #   Prometheus /metrics (OpenMetrics)
        ├── rate_limiter.rs             #   滑动窗口限流器 (全局 + per-group)
        ├── local_store.rs              #   DashMap 计数器 + 消息指纹去重
        ├── group_config.rs             #   群配置缓存 + 链上定期同步
        └── audience_tracker.rs         #   活跃成员追踪 (7天窗口, 多层反作弊)
```

## 模块概览

| 模块 | 文件数 | 说明 |
|------|:------:|------|
| `chain/` | 5 | Substrate 链客户端 (subxt)、查询、交易、DCAP L4 证明、ActionLogBatcher 批量提交 |
| `tee/` | 17 | Enclave 桥接、密封存储、密钥管理、DCAP 多级证明、Shamir、仪式、RA-TLS Provision、Peer 监控、Token 保险库、加密 IPC |
| `platform/` | 8 | PlatformAdapter/PlatformExecutor trait、16 种 ActionType、Telegram 适配+执行、Discord 适配+执行+Gateway |
| `processing/` | 46 | 消息路由、规则引擎 (30+ 可插拔规则)、AutoMod 引擎、CAPTCHA、审计日志、自定义命令、广告投放、违规追踪 |
| `infra/` | 6 | Prometheus 指标、限流器、本地缓存 (DashMap)、群配置链上同步、Audience 追踪 (CPM 反作弊) |
| 根 | 4 | main.rs (启动编排)、config.rs (环境变量)、error.rs (统一错误)、webhook.rs (HTTP 入口) |

**合计: 86 个 Rust 源文件**

## 快速开始

```bash
# 复制环境配置
cp .env.example .env
# 编辑 .env 填入 BOT_TOKEN 等

# 编译
cargo build --release

# 运行
cargo run --release

# 免注册模式 (无需链上注册, 使用默认规则 + 强制广告)
CHAIN_ENABLED=false cargo run --release
```

## 运行模式

### 标准模式 (CHAIN_ENABLED=true, 默认)

连接 Substrate 链, 完整功能:
- 链上群规则配置同步 → 动态重建规则引擎
- 动作日志批量上链 (Ed25519 签名 + 序列号去重)
- TEE 证明自动提交/刷新 (DCAP Level 4)
- 广告投放 + 链上收据上报
- 全部 30+ 规则可用

### 免注册模式 (CHAIN_ENABLED=false)

零门槛开箱即用, 无需链上注册和 GAS 费:
- 默认安全规则: 防刷屏 (10/min) → 管理命令 → 入群审批
- 强制展示 GroupRobot 广告页脚
- 设置 `CHAIN_ENABLED=true` 后解锁自定义规则

## 链上交互

与 `grouprobot-*` Pallet (index 150-153) 通过 subxt 动态调用交互:

| Pallet | Index | 功能 |
|--------|:-----:|------|
| **GroupRobotRegistry** | 150 | Bot 注册、TEE 证明提交/刷新、MRTD 白名单、DCAP Level 2-4、Dual Quote |
| **GroupRobotConsensus** | 151 | 序列去重、动作日志提交 (单条/批量)、订阅/奖励 |
| **GroupRobotCommunity** | 152 | 群规则配置查询、CAS 乐观锁更新 |
| **GroupRobotCeremony** | 153 | RA-TLS 仪式记录、Enclave 审批 |

### 证明提交流程

- **Hardware 模式**: `request_nonce` → 等 1 区块 → `generate_quote_with_nonce` → 提取证书链 → `submit_dcap_full_attestation` (Level 4) → 证书链不可用时降级 Level 1
- **Software 模式**: `generate_attestation` → `submit_attestation` (quote_verified=false)
- **刷新周期**: Hardware = 23h (24h 有效 - 1h 安全边际), Software = 7 天

## 规则引擎

可插拔规则链, 按顺序评估, 首个匹配终止:

| 序号 | 规则 | 说明 | 链上配置 |
|:----:|------|------|----------|
| 0 | CallbackRule | Inline 键盘回调 (最高优先级) | — |
| 1 | FloodRule | 防刷屏 (滑动窗口计数) | `anti_flood_enabled`, `flood_limit` |
| 2 | DuplicateRule | 重复消息检测 | `anti_duplicate_enabled`, `duplicate_window_secs` |
| 3 | BlacklistRule | 正则黑名单 | `blacklist_patterns` |
| 4 | StopWordRule | 停用词匹配 | `stop_words` |
| 5 | EmojiRule | Emoji 数量限制 | `max_emoji` |
| 6 | LinkLimitRule | 链接数量限制 | `max_links` |
| 7 | SimilarityRule | TF-IDF 垃圾检测 | `spam_samples`, `similarity_threshold` |
| 8 | AntiPhishingRule | 反钓鱼链接 | `antiphishing_enabled` |
| 9 | LockRule | 消息类型锁定 | `locked_types_csv` |
| 10 | CommandRule | 管理指令 (/ban /mute /kick /warn) | — |
| 11 | JoinRequestRule | 入群审批 + 欢迎消息 | `welcome_enabled`, `welcome_template` |
| 12 | AdFooterRule | 免费层广告页脚 (subscription_tier=0) | `subscription_tier` |
| ∞ | DefaultRule | 兜底放行 (始终最后) | — |

**后处理器**: WarnTracker — 警告累积, 超阈值自动升级 (warn → mute → ban)

**层级门控**: `max_rules` 控制规则数量上限, 超出截断 (CallbackRule + DefaultRule 不计入)

### 扩展规则

| 规则 | 说明 |
|------|------|
| AutoModRule | 可组合审核引擎 (JSON: Trigger → Condition → Effect) |
| BayesClassifierRule | 朴素贝叶斯垃圾分类器 (TEE 密封模型) |
| NsfwRule | NSFW 媒体检测 (TEE ML 审核队列) |
| CasRule | Combot Anti-Spam 集成 |
| GbanRule | 全局封禁名单 (链上同步) |
| HomoglyphRule | 形近字 / Unicode 混淆检测 |
| ProfanityRule | 脏话过滤 |
| MentionFloodRule | @mention 轰炸检测 |
| NewMemberAuditRule | 新成员审查 |
| RaidRule | Raid 攻击检测 |
| ViolationTracker | 违规追踪看板 (`/violations` `/leaderboard`) |
| CaptchaRule | CAPTCHA 验证 (数学题 / RA-TLS 网页) |
| LogChannelRule | 管理动作转发到日志频道 |

## TEE 模式

| 模式 | 说明 |
|------|------|
| `auto` | 自动检测 `/dev/attestation/quote` 决定硬件/软件 |
| `hardware` | 强制 TDX+SGX 硬件模式 (nonce 防重放, DCAP Level 4) |
| `software` | 纯软件模拟 (开发/测试用, quote_verified=false) |

## Vault 模式

Token 保护的三种进程隔离模式:

| 模式 | 说明 |
|------|------|
| `inprocess` | 默认。Token 在当前进程内 (Zeroizing + mlock), 供 RA-TLS Provision 直接写入 |
| `spawn` | 恢复 Token → 启动内嵌 Vault 服务端 → AES-GCM 加密 IPC 连接 |
| `connect` | 连接外部 Vault 进程 (Gramine SGX Enclave, Token 明文仅在 SGX 内, **推荐生产环境**) |

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| **平台** | | |
| `PLATFORM` | `telegram` | 平台模式: `telegram` / `discord` / `both` |
| `BOT_TOKEN` | — | Telegram Bot Token (启动后自动从 environ 清除) |
| `BOT_ID_HASH` | 自动派生 | 32 字节 hex (优先于 BOT_TOKEN 派生) |
| `DISCORD_BOT_TOKEN` | — | Discord Bot Token |
| `DISCORD_APPLICATION_ID` | — | Discord 应用 ID |
| `DISCORD_INTENTS` | `33281` | Discord Gateway Intents |
| **Webhook** | | |
| `WEBHOOK_PORT` | `3000` | HTTP 服务端口 |
| `WEBHOOK_URL` | — | Telegram Webhook 注册 URL |
| `WEBHOOK_SECRET` | — | Webhook Secret Token 验证 (空=禁用鉴权, 会输出警告) |
| **链上** | | |
| `CHAIN_ENABLED` | `true` | 是否启用链上交互 (`false`=免注册模式) |
| `CHAIN_RPC` | `ws://127.0.0.1:9944` | Substrate RPC 端点 |
| `CHAIN_SIGNER_SEED` | 自动生成 | sr25519 签名密钥种子 (启动后自动从 environ 清除) |
| **TEE** | | |
| `TEE_MODE` | `auto` | TEE 模式: `auto` / `hardware` / `software` |
| `DATA_DIR` | `./data` | 数据目录 (密封密钥/share/序列号) |
| `SEAL_POLICY` | `dual` | 密封策略: `mrenclave` / `mrsigner` / `dual` |
| **Vault** | | |
| `VAULT_MODE` | `inprocess` | Token 保险库模式: `inprocess` / `spawn` / `connect` |
| `VAULT_SOCKET` | 自动 | Vault Unix socket 路径 |
| **Shamir / Ceremony** | | |
| `SHAMIR_THRESHOLD` | `2` | Shamir 门限 K (默认 K=2, N=4) |
| `PEER_ENDPOINTS` | — | Peer TEE 节点端点 (逗号分隔; 空=链上自动发现) |
| `CEREMONY_PORT` | `0` | Ceremony 独立端口 (0=共用主端口) |
| `MIGRATION_SOURCE` | — | 旧版本 Enclave 端点 (Migration Ceremony) |
| **RA-TLS Provision** | | |
| `PROVISION_SECRET` | — | RA-TLS Bearer Token (空=禁用 `/provision/*` 路由) |
| **Telegram 扩展** | | |
| `TG_API_BASE_URL` | `https://api.telegram.org` | Telegram API Base URL (Local Server 改为 `http://127.0.0.1:8081`) |
| **性能** | | |
| `WEBHOOK_RATE_LIMIT` | `200` | Webhook 每分钟最大请求数 |
| `CHAIN_LOG_BATCH_INTERVAL` | `6` | 链上日志批量提交间隔 (秒) |
| `CHAIN_LOG_BATCH_SIZE` | `50` | 链上日志批量提交大小 |
| **监控** | | |
| `METRICS_ENABLED` | `true` | 是否启用 Prometheus /metrics |
| `RUST_LOG` | `grouprobot=info` | 日志级别 |

## HTTP API

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/webhook` | Telegram Webhook 入口 (Secret 验证 + 全局限流 + per-group 限流 + 指纹去重) |
| `GET` | `/health` | 健康检查 (`status`, `version`, `uptime_secs`, `platform`, `tee_mode`, `seal_policy`) |
| `GET` | `/v1/status` | 详细状态 (`bot_id_hash`, `public_key`, `upgrade_compat`, 缓存统计) |
| `GET` | `/metrics` | Prometheus 指标 (OpenMetrics 格式) |
| `POST` | `/ceremony/*` | RA-TLS 仪式 Share 接收/服务 |
| `GET` | `/provision/attestation` | RA-TLS: 获取 TEE Quote + Enclave X25519 公钥 + 会话 ID |
| `POST` | `/provision/inject-token` | RA-TLS: 提交 ECDH 加密的 Token (Bearer Token 鉴权) |

## 安全特性

- **jemalloc zero-on-free**: 全局分配器, 堆内存释放后自动清零 (需 `MALLOC_CONF=abort_conf:false,zero:true`)
- **内存加固**: `RLIMIT_CORE=0` + `PR_SET_DUMPABLE=0` + `RLIMIT_MEMLOCK` 提升 (禁用 core dump, 扩大 mlock 容量)
- **TokenVault**: 所有 token 使用 `Zeroizing<String>` + mlock 锁定内存页
- **Vault IPC**: Unix socket 0o600 权限 + AES-256-GCM 加密通道 + 方向前缀 + 单调 nonce
- **密封存储**: AES-256-GCM (Software) / SGX Seal Key (Hardware), V0→V1 自动迁移
- **Debug 脱敏**: BotConfig 手动实现 Debug, `webhook_secret`/`chain_signer_seed` 等显示 `<REDACTED>`
- **环境变量清除**: `BOT_TOKEN` / `CHAIN_SIGNER_SEED` 使用后立即 `remove_var`, 防止 `/proc/<pid>/environ` 泄漏
- **RA-TLS Provision**: X25519 ECDH 端到端加密 Token 注入, 中间代理仅见密文
- **DCAP 多级证明**: Level 1 (结构解析) → Level 2 (ECDSA 签名) → Level 3 (PCK 验证) → Level 4 (Intel Root CA 全证书链)
- **Peer 健康监控**: 自动检测 peer 数量, peer_count ≤ K 时 CRITICAL 告警, 新节点加入自动触发 Re-ceremony
- **证明刷新**: Hardware 24h 有效 / 23h 自动刷新 (nonce 防重放), Software 7 天刷新

## 密封策略 (SEAL_POLICY)

控制密封存储的密钥绑定方式, 直接影响升级兼容性:

| 策略 | 密钥绑定 | 升级兼容 | 安全性 | 适用场景 |
|------|----------|----------|--------|----------|
| `mrenclave` | 代码度量 (MRENCLAVE) | 代码变更后旧文件不可读 | 最高 | 安全性优先, 无跨版本需求 |
| `mrsigner` | 签名者 (MRSIGNER) | Gramine 签名密钥不变即兼容 | 高 | 频繁升级, 单签名密钥管控 |
| `dual` | 写 MRSIGNER / 读兼容两种 | 跨版本兼容 + V0 向后兼容 | 高 | **推荐生产环境** |

密封文件格式:
- V0 (旧): `[nonce:12][ciphertext]` — 仅 MRENCLAVE 密钥
- V1 (新): `[0x01][key_type:1][nonce:12][ciphertext]` — 标识密钥类型

启动时自动将 V0 文件迁移到 V1 (原文件备份为 `.v0.bak`)。

## 5 级恢复链

启动时按优先级尝试恢复 Token + 签名密钥:

| 级别 | 来源 | 条件 | 场景 |
|------|------|------|------|
| 1 | 本地密封 share (K=1) | 本地 `.sealed` 文件可解密 | 常规重启, MRSIGNER 跨版本 |
| 2 | 本地 + Peer share (K>1) | peer 端点可达, K 个 share 收集成功 | 滚动升级, 多节点部署 |
| 3 | Migration Ceremony | `MIGRATION_SOURCE` 已配置, 旧版本运行中 | Gramine 签名密钥变更 |
| 4 | RA-TLS Provision | DApp 管理员通过 `/provision/inject-token` 注入 | 管理员干预 |
| 5 | 环境变量 fallback | `BOT_TOKEN` 环境变量存在 | 紧急恢复, 首次部署 |

### Migration Ceremony 协议

用于 Gramine 签名密钥变更等 MRSIGNER 也变的场景:

```
旧 Enclave (运行中) ←── HTTP POST /migration/export-secret ──── 新 Enclave (启动中)
    │                                                              │
    │  1. 解密本地 share → 恢复 secret                              │
    │  2. ECDH(ephemeral_sk, new_pk) 加密 → 响应                   │
    │                                                              │
    │  ──────── { encrypted_secret, ephemeral_pk } ──────────►    │
    │                                                              │
    │                              3. ECDH 解密 → 获得 secret      │
    │                              4. Auto-seal → 本地密封保存      │
```

安全约束:
- 单次使用: 导出后标记 `exported`, 拒绝重复导出
- ECDH 端到端: secret 明文仅在 TEE 内存中
- 身份保持: 迁移的 Ed25519 签名密钥保持链上身份不变

### RA-TLS Provision 协议

管理员 DApp → TEE Enclave 端到端加密 Token 注入:

```
DApp (浏览器)                                    TEE Enclave
    │                                                │
    │  GET /provision/attestation                     │
    │  ────────────────────────────────────────►      │
    │                                                │
    │  ◄──── { quote, enclave_pk, tee_measurement,   │
    │          tee_mode, session_id }                 │
    │                                                │
    │  1. 验证 Quote (MRTD 白名单 / Intel PCCS)       │
    │  2. ECDH(ephemeral_sk, enclave_pk) → shared_key │
    │  3. AES-256-GCM 加密 Token                      │
    │                                                │
    │  POST /provision/inject-token                   │
    │  { ephemeral_pk, ciphertext, nonce, platform }  │
    │  ────────────────────────────────────────►      │
    │                                                │
    │                      4. ECDH 解密 → Token 明文  │
    │                      5. 注入 TokenVault         │
    │                      6. Auto-seal 为 Shamir share│
    │                      7. 销毁临时密钥对           │
```

安全属性:
- Enclave X25519 公钥绑定在 TDX Quote `report_data[0..32]` 中
- 中间代理 / CDN / 反向代理只能看到密文
- `PROVISION_SECRET` Bearer Token 鉴权防止未授权注入

## 测试

```bash
cargo test
```

## SGX 部署 (Gramine)

参见 [gramine/README.md](gramine/README.md)。

```bash
# 编译 + 复制到 gramine 目录
cargo build --release
cp target/release/grouprobot gramine/

# 生成 manifest + 签名
cd gramine
gramine-manifest -Dlog_level=error token-vault.manifest.template token-vault.manifest
gramine-sgx-sign --manifest token-vault.manifest --output token-vault.manifest.sgx
```

## 升级 SOP

### 场景 A: 小版本升级 (同 Gramine 签名密钥)

MRSIGNER 不变, `SEAL_POLICY=dual` 时 Level 1 直接解密旧密封文件:

```bash
# 1. 编译新版本
cargo build --release

# 2. 提取新 MRTD + 链上预批准
./scripts/extract-mrtd.sh
substrate-cli tx groupRobotRegistry approve_mrtd --mrtd <NEW_MRTD>

# 3. 滚动升级 (自动逐台)
./scripts/rolling-upgrade.sh upgrade-config.json

# 4. 验证: 所有节点 public_key 不变, 版本更新
curl http://node:3000/v1/status
```

### 场景 B: 多节点滚动升级 (K-of-N)

约束: 同时停止的节点数 ≤ N - K

```bash
# 1. 链上预批准新 MRTD (同上)
# 2. 编写 upgrade-config.json:
cat > upgrade-config.json <<'EOF'
{
  "nodes": [
    { "name": "node-a", "host": "10.0.1.1", "port": 3000, "ssh": "user@10.0.1.1" },
    { "name": "node-b", "host": "10.0.1.2", "port": 3000, "ssh": "user@10.0.1.2" },
    { "name": "node-c", "host": "10.0.1.3", "port": 3000, "ssh": "user@10.0.1.3" },
    { "name": "node-d", "host": "10.0.1.4", "port": 3000, "ssh": "user@10.0.1.4" }
  ],
  "shamir_k": 2,
  "binary_path": "/opt/grouprobot/grouprobot",
  "new_binary": "./target/release/grouprobot",
  "health_timeout": 60
}
EOF

# 3. 执行
./scripts/rolling-upgrade.sh upgrade-config.json
```

### 场景 C: Gramine 签名密钥变更 (Migration Ceremony)

MRSIGNER 也变, 需通过旧版本传递密钥:

```bash
# 1. 保持旧版本运行
# 2. 启动新版本, 指定 MIGRATION_SOURCE:
MIGRATION_SOURCE=https://old-node:3000 \
SEAL_POLICY=dual \
./grouprobot

# 新版本启动时自动:
#   Level 1: 本地 share 解密失败 (MRSIGNER 变了)
#   Level 3: 向旧版本发起 Migration Ceremony
#   → ECDH 获取 secret → auto-seal → 身份保持
#
# 3. 验证后停止旧版本
```

### CI/CD 集成

```yaml
# GitHub Actions 示例
- name: Build SGX Enclave
  run: |
    cargo build --release
    cp target/release/grouprobot gramine/
    cd gramine
    gramine-manifest -Dlog_level=error token-vault.manifest.template token-vault.manifest
    gramine-sgx-sign --manifest token-vault.manifest --output token-vault.manifest.sgx

- name: Extract MRTD
  run: ./scripts/extract-mrtd.sh

- name: Upload measurements
  uses: actions/upload-artifact@v4
  with:
    name: mrtd-measurements
    path: gramine/mrtd-measurements.json
```

### 升级验证清单

| 检查项 | 命令 | 期望 |
|--------|------|------|
| 节点健康 | `curl /health` | `{"status": "ok"}` |
| 版本更新 | `curl /health` | `.version` = 新版本 |
| 身份保持 | `curl /v1/status` | `.public_key` 不变 |
| 密封兼容 | `curl /v1/status` | `.upgrade_compat.cross_version_seal` = true |
| 链上证明 | `substrate-cli query attestations` | `quote_verified=true`, `mrtd=新值` |

## 关键依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tokio` | 1.35 | 异步运行时 |
| `axum` | 0.7 | HTTP 服务器 (Webhook + API) |
| `reqwest` | 0.12 | HTTP 客户端 (TG/DC API, rustls-tls) |
| `subxt` | 0.38 | Substrate 链客户端 (动态查询+交易) |
| `ed25519-dalek` | 2 | Ed25519 签名 (TEE 密钥) |
| `x25519-dalek` | 2 | X25519 ECDH (RA-TLS / Share 加密) |
| `aes-gcm` | 0.10 | AES-256-GCM (密封存储 / IPC 加密) |
| `zeroize` | 1.8 | 内存清零 (Token / 密钥 Drop 自动归零) |
| `tikv-jemallocator` | 0.6 | jemalloc 全局分配器 (zero-on-free) |
| `dashmap` | 6 | 并发 HashMap (限流 / 指纹去重 / Audience) |
| `tokio-tungstenite` | 0.24 | WebSocket (Discord Gateway) |
| `prometheus-client` | 0.22 | Prometheus 指标导出 |

## 详细文档

| 文档 | 说明 |
|------|------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 架构设计详解 |
| [docs/API_REFERENCE.md](docs/API_REFERENCE.md) | HTTP API 完整参考 |
| [docs/CHAIN_INTEGRATION.md](docs/CHAIN_INTEGRATION.md) | 链上交互详解 |
| [docs/PLATFORM_ADAPTERS.md](docs/PLATFORM_ADAPTERS.md) | 多平台适配器设计 |
| [docs/RULE_ENGINE.md](docs/RULE_ENGINE.md) | 规则引擎设计 |
| [docs/TEE_SECURITY.md](docs/TEE_SECURITY.md) | TEE 安全模型 |
| [gramine/README.md](gramine/README.md) | SGX Gramine 部署指南 |

## 许可证

Apache-2.0
