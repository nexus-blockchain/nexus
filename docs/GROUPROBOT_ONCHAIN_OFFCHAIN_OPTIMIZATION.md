# GroupRobot 链上/链下分工优化方案

> **日期:** 2026-02-27
> **范围:** pallet-grouprobot-* (8 个链上 Pallet) ↔ grouprobot (离链 TEE Bot)
> **目标:** 减少链上冗余、降低 GAS 成本、提升吞吐量、保持安全审计能力

---

## 1. 现状总览

### 1.1 链上 Pallet 职责 (8 个模块)

| Pallet | 存储项数 | Extrinsic 数 | 核心职责 |
|--------|---------|-------------|---------|
| **registry** | 12 | ~18 | Bot 注册、TEE 证明 (DCAP V1/V2)、Peer 注册、社区绑定 |
| **consensus** | 10 | 7 | 节点注册/质押/退出、序列号去重、Equivocation、Era 奖励分配 |
| **community** | 7 | 6 | 社区配置 (40+ 字段)、动作日志存证、用户声誉 |
| **ceremony** | 5 | 5 | Shamir 仪式记录、Enclave 白名单、Re-ceremony |
| **subscription** | 4 | 4 | 订阅层级、Escrow、Era 结算 |
| **ads** | 8+ | 10+ | 广告活动 CRUD、投放收据、结算 |
| **rewards** | 4 | 1 | 节点奖励存储、Claim |
| **primitives** | — | — | 共享类型/Trait |

### 1.2 链下 Bot 职责

| 模块 | 文件数 | 核心职责 |
|------|-------|---------|
| **platform/** | 8 | Telegram/Discord 适配器 + 执行器 (20+ API 方法) |
| **processing/** | 15+ | 规则引擎 (15 种规则)、AutoMod、CAPTCHA、自定义命令、审计日志 |
| **chain/** | 4 | subxt 链客户端、查询 (10 个)、交易 (15 个)、类型定义 |
| **tee/** | 17 | Enclave、Attestor、Shamir、Ceremony、Vault、RA-TLS、DCAP |
| **infra/** | 4 | LocalStore、ConfigManager、RateLimiter、Metrics |

### 1.3 链上↔链下交互矩阵 (当前)

```
链下 Bot                              链上 Pallet
─────────                             ──────────
消息接收 (Webhook/Gateway)
    │
    ▼
规则评估 (RuleEngine) ◄────── 读取 ── CommunityConfigs (community)
    │
    ▼
平台执行 (TG/DC API)
    │
    ├── 签名+序列号 ────── 写入 ── submit_action_log (community)    ← 每条动作
    ├── 序列号去重 ──────── 写入 ── mark_sequence_processed (consensus) ← 每条动作
    │
    ▼
TEE 证明刷新 (24h) ────── 写入 ── submit_*_attestation (registry)
Peer 心跳 ─────────────── 写入 ── heartbeat_peer (registry)
广告投放 ──────────────── 写入 ── submit_delivery_receipt (ads)
活跃人数上报 ─────────── 写入 ── update_active_members (community)
```

---

## 2. 问题诊断

### 2.1 链上过重 — CommunityConfig 存储膨胀

**问题:** `CommunityConfigs` 存储了 40+ 字段的群规则配置，包括纯展示/执行层面的数据：

| 字段类别 | 示例 | 是否需要链上存证? |
|---------|------|-----------------|
| 安全参数 | `flood_limit`, `warn_limit`, `warn_action` | ✅ 需要 — 管理策略可审计 |
| 模板文本 | `welcome_template`, `goodbye_template` | ❌ 不需要 — 纯展示层 |
| 检测样本 | `spam_samples`, `profanity_words`, `homoglyph_keywords` | ❌ 不需要 — 执行层数据 |
| AutoMod JSON | `automod_rules_json` | ⚠️ 争议 — hash 存证即可 |
| 功能开关 | `captcha_enabled`, `cas_enabled`, `nsfw_mode` | ✅ 需要 — 策略可审计 |
| 订阅门控 | `subscription_tier`, `max_rules`, `forced_ads_per_day` | ✅ 需要 — 经济模型 |

**影响:** 每次 `update_community_config` 交易的 proof_size 过大 (几 KB 的模板文本 + JSON)，GAS 昂贵且浪费链上存储空间。

### 2.2 链上过重 — 每条动作双写

**问题:** 每次 Bot 执行管理动作，链下发起 **两笔交易**：

1. `submit_action_log` (community) — 动作日志存证
2. `mark_sequence_processed` (consensus) — 序列号去重

对于活跃群 (如 1000 msg/min)，即使批量提交，GAS 成本也很高。

**根因:** 序列号去重本可与日志存证合并，或由链上在处理日志时自动标记。

### 2.3 链上过重 — 声誉系统全量上链

**问题:** `MemberReputation` + `GlobalReputation` 在链上为每个用户每个社区维护声誉分数。每次声誉变更都是一笔交易。

**实际需求:** 声誉分数主要供 Bot 本地决策使用 (如 新成员审查)，链上仅需可审计性。

### 2.4 链下缺失 — ConfigManager 同步效率

**问题:** `ConfigManager` 使用定时轮询 (30min) 同步链上配置。配置变更后最长 30 分钟才生效。

**应有方案:** 订阅链上事件 (subscription)，配置变更时实时推送。

### 2.5 链下缺失 — 广告投放逻辑在链上

**问题:** `pallet-grouprobot-ads` 维护了复杂的广告排期 (`CommunitySchedules`)、受众报告 (`NodeAudienceReports`)、结算游标 (`AdSettlementCursor`) 等。这些本质上是**执行层编排**，不需要放在链上。

链上只需要：广告活动 CRUD + 预算锁定 + 结算 (资金流转)。

### 2.6 on_initialize 计算过重

**问题:** 三个 Pallet 在 `on_initialize` 中做全表扫描：
- `registry`: 遍历 `Attestations` + `AttestationsV2` + `PeerRegistry` 清理过期
- `ceremony`: 遍历 `ActiveCeremony` 检查过期 + CeremonyAtRisk
- `consensus`: `cleanup_expired_sequences` + `on_era_end` (含奖励分配)

当 Bot/Peer/Ceremony 数量增长时，单块计算量可能超出限制。

---

## 3. 优化方案

### 3.1 CommunityConfig 瘦身 — 链上哈希 + 链下 IPFS/本地

**目标:** 链上只存 **策略哈希 + 关键参数**，大文本/样本数据存链下。

#### 链上保留 (CommunityConfigOnChain)

```rust
pub struct CommunityConfigOnChain {
    // ── 安全策略参数 (可审计) ──
    pub anti_flood_enabled: bool,
    pub flood_limit: u16,
    pub warn_limit: u8,
    pub warn_action: u8,
    pub warn_mute_duration: u64,
    pub anti_duplicate_enabled: bool,
    pub duplicate_window_secs: u64,
    pub duplicate_threshold: u16,
    pub max_emoji: u16,
    pub max_links: u16,
    pub max_mentions: u16,
    pub captcha_enabled: bool,
    pub captcha_timeout_secs: u64,
    pub antiphishing_enabled: bool,
    pub approve_enabled: bool,
    pub raid_window_secs: u64,
    pub raid_threshold: u16,
    pub cas_enabled: bool,
    pub nsfw_mode: u8,
    pub violation_tracking_enabled: bool,
    pub welcome_enabled: bool,
    pub new_member_audit_count: u16,
    // ── 订阅层级 (经济模型) ──
    pub subscription_tier: u8,
    pub max_rules: u16,
    pub forced_ads_per_day: u8,
    pub can_disable_ads: bool,
    // ── 链下扩展配置的哈希 ──
    pub extended_config_hash: [u8; 32],  // SHA256(链下完整配置 JSON)
    // ── 版本 ──
    pub version: u32,
}
```

#### 链下存储 (ExtendedConfig)

```rust
// 存储在 IPFS / 本地 LocalStore / 链下数据库
pub struct ExtendedConfig {
    pub welcome_template: String,
    pub goodbye_template: String,
    pub stop_words: String,
    pub spam_samples: String,
    pub profanity_words: String,
    pub homoglyph_keywords: String,
    pub gban_list_csv: String,
    pub custom_commands_csv: String,
    pub automod_rules_json: String,
    pub locked_types_csv: String,
    pub log_channel_id: String,
    pub similarity_threshold: u8,
    pub bayes_threshold: u8,
    pub profanity_action: u8,
}
```

**验证机制:** Bot 在使用链下配置时，计算 SHA256 与链上 `extended_config_hash` 对比。如果不匹配，拒绝使用并告警。

**收益:**
- 链上存储从 ~2KB/社区 降至 ~200 bytes/社区
- `update_community_config` GAS 降低 80%+
- 模板/样本更新不再需要链上交易

### 3.2 动作日志 — 合并去重 + Merkle 批量存证

**目标:** 每条动作不再单独上链，改为 **Merkle Root 批量存证**。

#### 当前流程 (每条 2 笔 TX)
```
动作执行 → submit_action_log → mark_sequence_processed
```

#### 优化流程 (N 条 1 笔 TX)
```
动作执行 → 本地签名 + 序列号 → 累积到 Merkle 树
                                        │
                              定时/满量触发 (如 5min/100条)
                                        │
                                        ▼
                              submit_action_batch(
                                  community_id_hash,
                                  merkle_root,      // 批量日志的 Merkle Root
                                  log_count,         // 日志条数
                                  latest_sequence,   // 最新序列号
                                  signature,         // 整批签名
                              )
```

#### 链上变更
```rust
// 新增: 批量存证记录
#[pallet::storage]
pub type ActionBatches<T: Config> = StorageMap<
    _, Blake2_128Concat, (CommunityIdHash, u64), // (社区, batch_id)
    ActionBatchRecord<T>,
>;

pub struct ActionBatchRecord<T: Config> {
    pub merkle_root: [u8; 32],
    pub log_count: u32,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub operator: T::AccountId,
    pub submitted_at: BlockNumberFor<T>,
    pub signature: [u8; 64],
}
```

**收益:**
- 链上交易数从 O(N) 降至 O(N/batch_size)
- 序列号去重隐含在 batch 的 sequence 范围内
- 可审计性不变 — 任何人可用完整日志验证 Merkle Root

### 3.3 声誉系统 — 链上摘要 + 链下明细

**当前:** 每次声誉变更 = 1 笔链上交易 (update_reputation)

**优化:**
- 链下维护完整声誉明细 (LocalStore + 持久化)
- 链上仅存 **周期性摘要** (每 Era 或每 N 次变更提交一次)

```rust
// 链上: 声誉快照 (替代逐条记录)
#[pallet::storage]
pub type ReputationSnapshot<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, CommunityIdHash,
    Blake2_128Concat, [u8; 32], // user_hash
    ReputationSummary,
>;

pub struct ReputationSummary {
    pub score: i64,
    pub last_updated_era: u64,
    pub change_count: u32,        // 本 Era 变更次数
    pub detail_merkle_root: [u8; 32], // 明细 Merkle Root
}
```

**收益:**
- 声誉相关交易从 O(变更次数) 降至 O(活跃用户数/Era)

### 3.4 ConfigManager — 事件订阅替代轮询

**当前:** 定时轮询 (30min interval)

**优化:** 使用 subxt 的 `subscribe_finalized_blocks` + 事件过滤

```rust
// 链下: ConfigManager 增加事件监听
impl ConfigManager {
    pub async fn event_sync_loop(&self, chain: Arc<ChainClient>) {
        let mut blocks = chain.api().blocks().subscribe_finalized().await.unwrap();
        while let Some(block) = blocks.next().await {
            if let Ok(block) = block {
                let events = block.events().await.unwrap_or_default();
                for event in events.iter() {
                    if let Ok(ev) = event {
                        // 监听 CommunityConfigUpdated 事件
                        if ev.pallet_name() == "GroupRobotCommunity"
                            && ev.variant_name() == "CommunityConfigUpdated"
                        {
                            // 立即刷新缓存
                            self.invalidate_cache(&community_id_hash);
                        }
                    }
                }
            }
        }
    }
}
```

**收益:**
- 配置变更实时生效 (6s 出块延迟 vs 30min 轮询延迟)
- 减少无用的 RPC 查询 (仅在变更时拉取)

### 3.5 广告系统 — 编排下沉链下

**当前链上:**
- `CommunitySchedules` (排期)
- `NodeAudienceReports` (受众报告)
- `AdSettlementCursor` (结算游标)
- `submit_delivery_receipt` (每次投放 1 笔 TX)

**优化:** 链上只保留资金流转，编排逻辑下沉

| 功能 | 当前位置 | 优化后位置 | 说明 |
|------|---------|-----------|------|
| 广告活动 CRUD | 链上 | **链上** | 涉及资金锁定 |
| 预算锁定/释放 | 链上 | **链上** | 涉及资金流转 |
| 广告排期/选择 | 链上 | **链下** | 纯编排逻辑 |
| 受众报告收集 | 链上 | **链下** | 执行层数据 |
| 投放收据 | 链上(逐条) | **链上(批量)** | Merkle Root 批量 |
| 结算 (资金分配) | 链上 | **链上** | 涉及资金流转 |
| 黑名单/白名单 | 链上 | **链上** | 权限管控 |

**收益:**
- `submit_delivery_receipt` 从每次投放 1 笔降至每 Era 1 笔批量
- 移除 `CommunitySchedules`、`NodeAudienceReports` 链上存储
- 排期逻辑由 Bot 本地执行，更灵活

### 3.6 on_initialize 优化 — 游标化 + 懒清理

**当前问题:** registry 的 `on_initialize` 遍历所有 Attestations + Peers。

**优化策略:**

| Pallet | 当前方式 | 优化方式 |
|--------|---------|---------|
| registry (Attestations) | 全表扫描 | 按 `expires_at` 索引 + 游标清理 |
| registry (PeerRegistry) | 全表扫描 | 心跳超时由链下触发 `report_stale_peer` |
| ceremony | 全表 ActiveCeremony 扫描 | 已优化 (CH2-fix)，可增加游标 |
| consensus (Sequences) | 游标清理 (已优化) | ✅ 已有 MaxSequenceCleanupPerBlock |

```rust
// 新增: Attestation 过期索引
#[pallet::storage]
pub type AttestationExpiryQueue<T: Config> = StorageValue<
    _, BoundedVec<(BlockNumberFor<T>, BotIdHash), ConstU32<1000>>, ValueQuery,
>;

// on_initialize: 只检查队列头部
fn on_initialize(n: BlockNumberFor<T>) -> Weight {
    let mut cleaned = 0u32;
    AttestationExpiryQueue::<T>::mutate(|queue| {
        while let Some((expires_at, bot_id_hash)) = queue.first() {
            if n < *expires_at { break; }
            // 清理过期证明
            Attestations::<T>::remove(bot_id_hash);
            queue.remove(0);
            cleaned += 1;
            if cleaned >= MAX_CLEANUP_PER_BLOCK { break; }
        }
    });
    // ...
}
```

**Peer 清理:** 改为被动触发 — Bot 下线不心跳 → 其他 Peer 或 Bot Owner 调用 `report_stale_peer` extrinsic 主动清理，而非链上每块轮询。

---

## 4. 优化后架构

```
链下 Bot (TEE)                          链上 Pallet
──────────────                          ──────────
消息接收 + 规则评估 + 平台执行
    │
    ├── 本地签名 → Merkle 树累积         
    │       │                           
    │       └── 定时批量 ──── 写入 ── submit_action_batch (community)
    │                                     └── 隐含序列号去重
    │
    ├── 声誉变更 (本地维护)              
    │       │                           
    │       └── 每 Era 快照 ── 写入 ── update_reputation_snapshot (community)
    │
    ├── 配置变更 ◄──── 事件订阅 ── CommunityConfigUpdated (community)
    │
    ├── 广告排期 (本地决策)              
    │       │                           
    │       └── 批量收据 ──── 写入 ── submit_delivery_batch (ads)
    │
    ├── TEE 证明 (24h) ──── 写入 ── submit_*_attestation (registry)
    │
    └── Peer 心跳 ─────── 写入 ── heartbeat_peer (registry)
```

---

## 5. 交易频率对比

假设活跃群 1000 msg/min, 10% 触发管理动作 (100 actions/min):

| 操作 | 当前频率 | 优化后频率 | 降幅 |
|------|---------|-----------|------|
| submit_action_log | 100/min (逐条) | 1/5min (批量) | **~99.8%** |
| mark_sequence_processed | 100/min | 0 (合并到 batch) | **100%** |
| update_reputation | ~50/min | 1/Era | **~99.99%** |
| submit_delivery_receipt | ~10/min | 1/Era | **~99.99%** |
| update_community_config | 偶发 (含大文本) | 偶发 (仅参数) | proof_size **~80%** |
| **总计** | ~260 TX/min | ~1 TX/5min | **~99.6%** |

---

## 6. 实施优先级

| 优先级 | 优化项 | 复杂度 | 收益 | 涉及改动 |
|--------|-------|--------|------|---------|
| **P0** | 动作日志 Merkle 批量存证 | 中 | 极高 (GAS 降 99%) | community pallet + chain/transactions.rs |
| **P0** | 移除 mark_sequence_processed 双写 | 低 | 高 (TX 减半) | consensus pallet + router.rs |
| **P1** | CommunityConfig 瘦身 | 中 | 高 (存储降 80%) | community pallet + chain/queries.rs + types.rs |
| **P1** | ConfigManager 事件订阅 | 低 | 中 (实时生效) | infra/group_config.rs |
| **P2** | 声誉系统摘要化 | 中 | 中 | community pallet + processing/ |
| **P2** | 广告编排下沉 | 高 | 中 | ads pallet + processing/ad_delivery.rs |
| **P3** | on_initialize 游标化 | 中 | 中 (可扩展性) | registry pallet + ceremony pallet |
| **P3** | Peer 清理被动化 | 低 | 低 | registry pallet |

---

## 7. 迁移策略

### 7.1 向后兼容

- **链上新增存储项** (`ActionBatches`, `ReputationSnapshot`, `AttestationExpiryQueue`) 不影响现有存储
- **链上保留旧 extrinsic** (`submit_action_log`, `mark_sequence_processed`) 但标记 deprecated
- **链下优先升级:** Bot 先实现 Merkle 批量提交，链上同时支持新旧两种方式
- **新增 `extended_config_hash` 字段** 添加到 `CommunityConfig` 结构体尾部 (SCALE 向后兼容)

### 7.2 分阶段实施

```
Phase 1 (P0): 动作日志批量 + 移除序列号双写
  ├── 链上: 新增 submit_action_batch extrinsic
  ├── 链上: submit_action_log 内部自动标记序列号
  └── 链下: ActionLogBatcher 改为 Merkle 树 + 批量提交

Phase 2 (P1): 配置瘦身 + 事件订阅
  ├── 链上: CommunityConfig 拆分为 OnChain + ExtendedHash
  ├── 链下: ExtendedConfig 存储在 LocalStore + IPFS
  └── 链下: ConfigManager 增加事件订阅模式

Phase 3 (P2+P3): 声誉摘要 + 广告下沉 + on_initialize 优化
  ├── 链上: ReputationSnapshot 替代逐条 update_reputation
  ├── 链上: 广告排期存储移除, 保留资金相关
  └── 链上: AttestationExpiryQueue + 被动 Peer 清理
```

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| Merkle 批量日志丢失 (Bot 崩溃) | 部分动作无链上记录 | ✅ 已实现: 本地 WAL (`action_log.wal`, bincode) + 重启自动重传 |
| 链下 ExtendedConfig 篡改 | 规则被恶意修改 | SHA256 哈希验证 + TEE 密封存储 |
| 声誉摘要精度损失 | 单次变更不可单独审计 | Merkle 明细保留在链下 IPFS (可验证) |
| 事件订阅断连 | 配置同步中断 | 保留定时轮询作为 fallback (降级到 5min) |
| 广告排期链下化后公平性 | Bot 可能偏袒某些广告 | TEE 执行保证 + 投放收据可审计 |

---

## 9. 总结

**核心原则: 链上管资金和信任锚，链下管执行和效率。**

| 维度 | 链上 (Pallet) | 链下 (TEE Bot) |
|------|-------------|---------------|
| **资金** | ✅ Escrow、质押、奖励、结算 | ❌ 不碰资金 |
| **身份** | ✅ Bot 注册、TEE 证明、Peer 注册 | 读取验证 |
| **策略** | ✅ 安全参数 + 功能开关 (精简) | 完整配置 + 模板 + 样本 |
| **执行** | ❌ 不做规则评估/消息处理 | ✅ 规则引擎、平台 API |
| **存证** | ✅ Merkle Root 批量 (信任锚) | ✅ 完整日志明细 (可验证) |
| **编排** | ❌ 不做广告排期/调度 | ✅ 本地编排决策 |
| **声誉** | ✅ Era 摘要 (可审计) | ✅ 实时明细 (决策用) |
