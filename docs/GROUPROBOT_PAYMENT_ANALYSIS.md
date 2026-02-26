# GroupRobot 群组付费功能逻辑分析

> 分析日期: 2026-02-26

## 一、整体架构

付费体系由 **8 个 pallet** 协同构成，围绕两条收入流:

```
┌─────────────────────────────────────────────────────────────┐
│                      收入流 ①: 订阅费                        │
│  Bot Owner → subscribe() → Escrow → Era 结算 → 节点+国库     │
├─────────────────────────────────────────────────────────────┤
│                      收入流 ②: 广告费                        │
│  Advertiser → create_campaign() → CPM投放 → 社区+节点+国库   │
└─────────────────────────────────────────────────────────────┘
```

**涉及 Pallet:**

| Pallet | 职责 |
|--------|------|
| `pallet-grouprobot-primitives` | 共享类型 + Trait: SubscriptionTier, TierFeatureGate, SubscriptionProvider, RewardAccruer 等 |
| `pallet-grouprobot-consensus` | 节点管理 + Equivocation + TEE 验证 + 序列去重 + Era 编排 (委托 subscription/rewards) |
| `pallet-grouprobot-subscription` | 订阅 CRUD + Escrow 管理 + 游标分页结算 + SubscriptionProvider 实现 |
| `pallet-grouprobot-rewards` | 统一奖励池 (订阅+广告+通胀) + claim_rewards + Era 记录 |
| `pallet-grouprobot-ads` | 广告 Campaign CRUD, CPM 计费, 投放收据, 结算, 反作弊 |
| `pallet-grouprobot-community` | 社区配置, 动作日志, 声誉系统 |
| `pallet-grouprobot-registry` | Bot 注册, TEE 证明 (TDX/SGX DCAP), Peer 管理 |
| `pallet-grouprobot-ceremony` | Ceremony 链上记录 + 历史管理 |

---

## 二、订阅层级体系 (`primitives` + `subscription`)

### 2.1 四级订阅

定义在 `pallets/grouprobot/primitives/src/lib.rs:166-231`:

| 层级 | 费用/Era | max_rules ¹ | 日志保留 ² | 强制广告 ¹ | 可关广告 | TEE |
|------|---------|-----------|---------|---------|---------|-----|
| **Free** | 0 | 3 | 7天 | 2/天 | ❌ | ❌ |
| **Basic** | `BasicFeePerEra` (mock=10) | 10 | 30天 | 0 | ✅ | ❌ |
| **Pro** | `ProFeePerEra` (mock=30) | 50 | 90天 | 0 | ✅ | ✅ |
| **Enterprise** | `EnterpriseFeePerEra` (mock=100) | 65535 | 永久 | 0 | ✅ | ✅ |

> ¹ **off-chain 执行:** `max_rules` 和 `forced_ads_per_day` 仅定义在 `TierFeatureGate` struct 中，链上无执行逻辑，由 Bot 离链客户端执行。
> ² **链上执行:** `log_retention_days` 在 `community` pallet 的 `clear_expired_logs` 中强制执行最低保留期。

### 2.2 层级完整功能对比

| 功能维度 | Free | Basic | Pro | Enterprise | 执行位置 |
|---------|------|-------|-----|------------|----------|
| **费用** | 0 | BasicFeePerEra | ProFeePerEra | EnterpriseFeePerEra | 链上 (subscription) |
| **链上注册** | ❌ 不需要 | ✅ 必须 | ✅ 必须 | ✅ 必须 | 链上 (registry) |
| **Shamir K** | 1 | 2 | 3 | ≥3 (可定制) | off-chain |
| **Shamir N** | 1 | 3 | 5 | ≥5 (可定制) | off-chain |
| **节点架构** | 单节点, 本地 seal | 3 节点冗余 | 5 节点高可用 | 多节点集群, 定制拓扑 | off-chain |
| **Token 保护** | TEE seal (单点) | Shamir 2-of-3 分片 | Shamir 3-of-5 分片 | 定制门限 | off-chain |
| **节点容灾** | 无 (宕机需重填 Token) | 允许 1 节点离线 | 允许 2 节点离线 | 按需配置 | off-chain |
| **Ceremony** | ❌ 无 (无链无 peer) | ✅ 链上记录 | ✅ 链上记录 | ✅ 链上记录 | 链上 (ceremony) |
| **Re-ceremony** | ❌ 不支持 | ✅ 自动触发 | ✅ 自动触发 | ✅ 自动触发 | 链上 (ceremony) |
| **Peer 监控** | ❌ 无 | ✅ 健康检测 + 告警 | ✅ 健康检测 + 告警 | ✅ 健康检测 + 告警 | 链上 (registry) |
| **TEE 节点** | ❌ 不参与共识网络 | ❌ 无 TEE 要求 | ✅ TEE 节点 + 1.2x 奖励 | ✅ TEE 节点 + 1.2x 奖励 | 链上 (registry+consensus) |
| **规则引擎** | 默认 3 条固定规则 | 10 条可配置规则 | 50 条可配置规则 | 无限制 | off-chain |
| **广告** | 强制 2 条/天 ¹, 不可关闭 | 可选, 可关闭 | 无广告 | 无广告 | off-chain ¹ / 链上 (ads) |
| **日志保留** | 7 天 | 30 天 | 90 天 | 永久 | 链上 (community) |
| **链上日志** | ❌ 不提交 | ✅ 批量提交 | ✅ 批量提交 | ✅ 批量提交 | 链上 (community) |
| **序列号去重** | ❌ 无 (本地去重) | ✅ 链上 ProcessedSequences | ✅ 链上 ProcessedSequences | ✅ 链上 ProcessedSequences | 链上 (consensus) |
| **广告收入分成** | 社区60/国库25/节点15 | 社区70/国库20/节点10 | — (无广告) | — (无广告) | 链上 (ads, 可治理调整) |
| **定位** | 零门槛体验 → 转化入口 | 基础冗余 → 付费起步 | 高可用 → 专业用户 | 定制方案 → 大客户 | — |

> ¹ `forced_ads_per_day` 仅定义在 `TierFeatureGate` struct，链上无验证逻辑，由 off-chain TEE Bot 执行。

**K/N 递增的核心价值:**

- **Free K=1, N=1** — `CHAIN_ENABLED=false`, 无链无 peer, 单节点是技术必然; 用户填 TOKEN 即用, 3 秒启动
- **Basic K=2, N=3** — 最基础的 Shamir 分片, 允许 1 个节点故障不丢 Token, 入门级付费
- **Pro K=3, N=5** — 5 节点中任意 3 个可恢复, 专业级容灾能力, TEE 节点参与共识获额外奖励
- **Enterprise K≥3, N≥5+** — 按需定制 K/N 参数, 支持跨数据中心部署, 最高安全等级

### 2.3 功能限制 (`TierFeatureGate`)

```rust
pub struct TierFeatureGate {
    pub max_rules: u16,           // 最大可启用规则数
    pub log_retention_days: u16,  // 日志保留天数 (0 = 永久)
    pub forced_ads_per_day: u8,   // 每日强制广告数 (0 = 无强制)
    pub can_disable_ads: bool,    // 是否可关闭广告
    pub tee_access: bool,         // 是否可使用 TEE 节点
    pub ad_revenue_community_pct: u8,  // 广告收入分成: 社区 %
    pub ad_revenue_treasury_pct: u8,   // 广告收入分成: 国库 %
    pub ad_revenue_node_pct: u8,       // 广告收入分成: 节点 %
}
```

### 2.3 订阅生命周期

```
subscribe(bot_id_hash, tier, deposit)
    │  ← 检查: Bot 已注册且 Active, 调用者是 Owner, 无重复订阅
    │  ← deposit >= tier_fee, Currency::reserve(deposit)
    ▼
  Active ──(Era 结束)──→ Escrow 够 → 扣费, 保持 Active
    │                    Escrow 不足 → PastDue
    │                                    │
    │                              (下个Era仍不足)
    │                                    ▼
    │                                 Suspended → FreeTierFallback 事件
    │
    ├── deposit_subscription(amount) ← 充值, PastDue/Suspended 自动恢复
    ├── change_tier(new_tier)        ← 升降级 (不能降到 Free)
    └── cancel_subscription()        ← 按比例扣除当期费用, 退还剩余, 状态→Cancelled
```

**关键 Extrinsics** (定义在 `pallets/grouprobot/subscription/src/lib.rs`):

| call_index | 函数 | 说明 |
|-----------|------|------|
| 0 | `subscribe` | 创建订阅, 锁定 deposit |
| 1 | `deposit_subscription` | 充值 (仅 owner), 自动恢复 PastDue/Suspended |
| 2 | `cancel_subscription` | 取消, 按比例扣除当期费用, 退还剩余 |
| 3 | `change_tier` | 升降级 (不允许降到 Free) |

### 2.4 Era 结算流程 (`on_era_end`)

编排在 `pallets/grouprobot/consensus/src/lib.rs` 的 `on_era_end`，委托 subscription 和 rewards pallet 执行:

```
1. 委托 subscription pallet 结算订阅费 (游标分页, 每块最多处理 MaxSubscriptionSettlePerEra 条)
   Escrow >= fee → unreserve + transfer 到国库, 否则 PastDue → Suspended
2. 收入拆分: subscription_income × 80% → 节点, 10% → 国库, 10% → agent
3. 铸币通胀: InflationPerEra (固定增发)
4. 可分配池: node_share(80%) + inflation
5. TEE 证明检查: 过期节点降级 is_tee_node=false
6. 委托 rewards pallet 按权重分配 (TEE 节点有 TeeRewardMultiplier 1.2x 加成)
7. 委托 rewards pallet 记录 EraRewardInfo + 清理过期历史
```

**层级费用查询:**

```rust
pub fn tier_fee(tier: &SubscriptionTier) -> BalanceOf<T> {
    match tier {
        SubscriptionTier::Free => BalanceOf::<T>::zero(),
        SubscriptionTier::Basic => T::BasicFeePerEra::get(),
        SubscriptionTier::Pro => T::ProFeePerEra::get(),
        SubscriptionTier::Enterprise => T::EnterpriseFeePerEra::get(),
    }
}
```

**有效层级降级逻辑:**

```rust
pub fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
    match Subscriptions::<T>::get(bot_id_hash) {
        Some(sub) => match sub.status {
            SubscriptionStatus::Active => sub.tier,
            SubscriptionStatus::PastDue => sub.tier,    // 宽限期内保持
            _ => SubscriptionTier::Free,                 // Suspended/Cancelled → 降级
        },
        None => SubscriptionTier::Free,
    }
}
```

---

## 三、广告竞价系统 (`ads`)

### 3.1 广告主侧 — Campaign 管理

定义在 `pallets/grouprobot/ads/src/lib.rs:559-706`:

```
create_campaign(text, url, bid_per_mille, daily_budget, total_budget, target, delivery_types, expires_at)
    │  ← text非空, delivery_types∈[1,7], bid >= MinBidPerMille, budget > 0
    │  ← Currency::reserve(total_budget) → 全额锁定
    ▼
  Active + Pending(审核) → review_campaign(Root) → Approved → 可投放
    │
    ├── fund_campaign(amount)     ← 追加预算 (Active/Paused/Exhausted)
    ├── pause_campaign()          ← 暂停
    └── cancel_campaign()         ← 取消, unreserve 剩余
```

**投放类型 (bitmask):**

| bit | 类型 | 说明 |
|-----|------|------|
| bit0 | `ScheduledPost` | 定时推送到群组 |
| bit1 | `ReplyFooter` | Bot 回复底部附带广告 |
| bit2 | `WelcomeEmbed` | 嵌入欢迎消息 |

**CPM 计费公式:**

```
cost = bid_per_mille × audience / 1000
```

**广告审核状态流转:**

```
Pending → Approved (Root 审核通过)
        → Rejected (Root 审核拒绝)
        → Flagged  (社区举报)
```

### 3.2 社区侧 — 质押准入

定义在 `pallets/grouprobot/ads/src/lib.rs:1009-1082`:

**质押获取 audience_cap:**

```
stake_for_ads(community_id_hash, amount)
    │  ← Currency::reserve(amount)
    │  ← 首个质押者自动成为 CommunityAdmin
    │  ← audience_cap = compute_audience_cap(total_stake)
    ▼
  阶梯函数:
    0-50 UNIT  → 20人/UNIT (max 1000)
    50-200     → ~27人/UNIT (max 5000)
    200+       → ~17人/UNIT (max 10000, 硬上限)
```

**收入提取:** 仅 `CommunityAdmin` 可调用 `claim_ad_revenue()`, 从国库转出。

### 3.3 投放收据提交

```
submit_delivery_receipt(campaign_id, community_id_hash, delivery_type, audience_size, node_id, node_signature)
    ← Campaign Active + Approved + 未过期
    ← 节点 Active + TEE (非 TEE 不允许提交)
    ← 社区未被 Banned + 未因突增被暂停
```

### 3.4 结算流程 (`settle_era_ads`)

定义在 `pallets/grouprobot/ads/src/lib.rs:810-946`:

```
settle_era_ads(community_id_hash) ← 任何人可触发
    │
    ├── L3: 检查 AudienceSurgePaused (突增暂停)
    ├── L5: validate_node_reports (多节点交叉验证, 偏差>阈值则拒结)
    │
    ▼ 遍历 DeliveryReceipts:
    cost = min(cpm_cost, escrow_remaining)
    │
    ├── CommunityAdPct% → community (CommunityClaimable, 群主可提取) [默认 80%, 可治理调整]
    ├── TeeNodeAdPct%  → TEE node  (通过 RewardAccruer 写入统一奖励池) [默认 15%]
    └── 剩余%          → treasury   (国库, 100 - community_pct - tee_pct)
    
    全部 actual_cost: advertiser → 国库 (统一结算)
    社区收入从国库提取, 节点收入通过 rewards pallet 统一 claim
```

### 3.5 三方分成对比

| 分成方 | 订阅费 | 广告费 | 说明 |
|--------|-------|-------|------|
| **节点** | 80% + 通胀铸币 | TeeNodeAdPct (默认 15%) | 统一通过 rewards pallet claim |
| **国库** | 10% | 100-community-tee (默认 5%) | 订阅费通过 unreserve+transfer 实际转入 |
| **社区** | — | CommunityAdPct (默认 80%) | 可治理调整 (set_community_ad_percentage) |
| **agent** | 10% | — | 订阅费拆分 |

---

## 四、反作弊机制 (5 层防御)

| 层级 | 机制 | 触发条件 | 后果 |
|------|------|---------|------|
| **L1** | 质押锚定 audience_cap | audience > cap | 截断到 cap |
| **L2** | 广告审核 (Root/DAO) | `review_campaign` | Approved/Rejected |
| **L3** | audience 突增检测 | > previous × (1+threshold%) | 暂停 2 Era |
| **L4** | Slash 机制 | Root 调用 `slash_community` | 扣质押 30%, 3次永久禁止 |
| **L5** | 多节点交叉验证 | 偏差 > threshold% | 拒绝结算 |

**Slash 详情:**

```
slash_community(community_id_hash, reporter)  ← Root only
    │
    ├── slash_amount = stake × AdSlashPercentage / 100
    ├── 50% → reporter (举报奖励)
    ├── 50% → treasury (国库)
    ├── audience_cap 砍半
    ├── SlashCount++
    └── SlashCount >= 3 → BannedCommunities (永久禁止)
```

**Audience 突增检测 (`check_audience_surge`):**

```
仅 TEE 节点运营者可触发
previous = 0 → 记录并通过 (首次)
current > previous × (1 + threshold/100) → 暂停 2 Era
正常 → 递减暂停计数 (0 时恢复)
```

**多节点交叉验证 (`validate_node_reports`):**

```
< 2 个节点报告 → 跳过验证
偏差 = (max - min) / min × 100
偏差 > NodeDeviationThresholdPct → 拒结 + NodeDeviationRejected 事件
通过 → 取中位数
```

---

## 五、双向偏好控制

### 广告主侧

| call_index | 函数 | 说明 |
|-----------|------|------|
| 11 | `advertiser_block_community` | 拉黑社区 |
| 12 | `advertiser_unblock_community` | 取消拉黑 |
| 13 | `advertiser_prefer_community` | 白名单社区 |

### 社区侧 (仅 CommunityAdmin)

| call_index | 函数 | 说明 |
|-----------|------|------|
| 14 | `community_block_advertiser` | 拉黑广告主 |
| 15 | `community_unblock_advertiser` | 取消拉黑 |
| 16 | `community_prefer_advertiser` | 白名单广告主 |

---

## 六、社区配置 (`community`)

### CommunityConfig 结构

定义在 `pallets/grouprobot/community/src/lib.rs:33-48`:

```rust
pub struct CommunityConfig {
    pub node_requirement: NodeRequirement,  // 节点准入策略
    pub anti_flood_enabled: bool,           // 防刷屏
    pub flood_limit: u16,                   // 刷屏阈值
    pub warn_limit: u8,                     // 警告次数上限
    pub warn_action: WarnAction,            // 警告达限动作 (Kick/Ban/Mute)
    pub welcome_enabled: bool,              // 欢迎消息
    pub ads_enabled: bool,                  // 是否接受广告 (Free/Basic 自动开启)
    pub active_members: u32,                // 活跃成员数 (Bot 更新, CPM 计费用)
    pub language: [u8; 2],                  // 社区语言 (ISO 639-1, 广告定向)
    pub version: u32,                       // CAS 乐观锁版本
}
```

**CAS 乐观锁:** `update_community_config()` 需传 `expected_version`, 防并发冲突。

**活跃人数独立更新:** Bot 通过 `update_active_members()` 上报, 不受 `update_community_config` 覆盖。

### 声誉系统

- **社区内声誉:** `MemberReputation<(community_hash, user_hash)> → ReputationRecord`
- **全局声誉:** `GlobalReputation<user_hash> → i64` (所有社区之和)
- **冷却期:** `ReputationCooldown` 防止频繁操作
- **最大变更:** `MaxReputationDelta` 限制单次变更幅度

---

## 七、资金流向总图

```
                    ┌──────────────┐
                    │   广告主      │
                    │ (Advertiser) │
                    └──────┬───────┘
                  reserve(budget)
                           │
                    ┌──────▼───────┐         ┌──────────────┐
                    │  Campaign    │         │  Bot Owner   │
                    │  Escrow      │         │              │
                    │  (ads pallet)│         └──────┬───────┘
                    └──────┬───────┘         reserve(deposit)
                   settle  │                        │
                           │                 ┌──────▼───────┐
                           │                 │ Subscription │
                           │                 │   Escrow     │
                           │                 │(subscription │
                           │                 │   pallet)    │
                           │                 └──────┬───────┘
                           │              unreserve  │
                           │              +transfer  │
                    ┌──────▼───────┐         ┌──────▼───────┐
                    │              │◄────────│ 订阅费实际    │
                    │   国 库      │  扣费    │ 转入国库     │
                    │  (Treasury)  │         └──────────────┘
                    │              │
                    └─┬──────┬──┬─┘
              claim   │      │  │  slash
           ┌──────────┘      │  └──────────┐
           ▼                 ▼             ▼
    ┌──────────┐    ┌──────────────┐ ┌──────────┐
    │ 社区管理员 │    │ rewards pallet│ │ 举报者    │
    │(community│    │ (统一奖励池)  │ │(slash 50%)│
    │ _pct ad) │    │              │ └──────────┘
    └──────────┘    │ 订阅80%+广告 │
                    │ tee_pct+通胀 │
                    └──────┬───────┘
                     claim │
                    ┌──────▼───────┐
                    │   TEE 节点   │
                    │  (operator)  │
                    └──────────────┘
```

---

## 八、关键设计特点

1. **Escrow 预付制** — 订阅费和广告预算都通过 `Currency::reserve` 锁定, Era 结算时 `unreserve + transfer` 到国库, 保证资金安全
2. **渐进降级** — 订阅欠费 Active→PastDue→Suspended, 充值可恢复, 不立即取消
3. **国库中转** — 订阅费和广告费均实际转入国库, 社区从国库提取, 节点通过统一奖励池 claim
4. **质押+反作弊** — 社区需质押才能接入广告, 质押决定 audience_cap, 作弊被 Slash (5 层防御)
5. **TEE 激励** — 节点 TEE 认证获得 1.2x 奖励加成 + 广告投放收据提交权
6. **双向偏好** — 广告主和社区都可互相拉黑/白名单, 保护双方利益
7. **统一奖励池** — subscription 和 ads 都通过 `RewardAccruer` trait 写入 rewards pallet, 节点单一 `claim_rewards()` 入口
8. **Tier 门控** — Free 层级链上限制: 无 Ceremony/Peer/日志/序列去重, 付费层级通过 `SubscriptionProvider` trait 链上验证
9. **关注点分离** — consensus 仅编排, 订阅 (subscription) 和奖励 (rewards) 独立 pallet, 可独立迭代

---

## 九、Free 订阅 K=1, N=1 限制分析

### 9.1 术语

| 参数 | 含义 | 当前默认 |
|------|------|---------|
| **K** (Shamir threshold) | 恢复 secret 需要的最少 share 数 | `SHAMIR_THRESHOLD=2` |
| **N** (total nodes) | 持有 share 的节点总数 | 由 Ceremony 决定 |

**K=1, N=1** = 单节点、无分片、无冗余。BOT-TOKEN sealed 在本机 TEE，无需 peer 收集 share。

### 9.2 技术可行性 ✅

- **代码已完整支持 K=1**: `share_recovery.rs` 中 `threshold <= 1` 路径直接恢复
- **免注册模式下 K>1 不可能**: `CHAIN_ENABLED=false` 不连接链, 无法 PeerRegistry 发现 peer
- **N=1 自然成立**: K=1 时 `split(secret, 1, 1)` 产生唯一 share = secret 本身

### 9.3 安全性

| 威胁 | K=1 | K≥2 |
|------|-----|-----|
| 物理内存读取 | TEE seal 保护 | 同 |
| 节点宕机 | Token 丢失 (重填即可) | K-1 节点存活即可恢复 |
| 节点被入侵 | Token 泄露 (单点故障) | 需入侵 K 个节点 |

**Free 用户可接受:** Token 由用户自持 (BotFather), 可随时 revoke + 重新生成。

### 9.4 商业分层

```
Free       (K=1, N=1): 零门槛体验 → 转化入口
Basic      (K=2, N=3): 基础冗余 → 付费起步
Pro        (K=3, N=5): 高可用 → 专业用户
Enterprise (K≥3, N≥5+): 定制 → 大客户
```

### 9.5 与 TierFeatureGate 对齐

- `tee_access: false` (Free) → 不参与 TEE 节点网络 → K=1 单节点是自然推论
- `max_rules: 3` → 固定规则, 无需分布式配置同步
- 无 Ceremony / 无 Peer Monitor / 无 Re-ceremony / 无 Share 加密传输

### 9.6 结论

| 维度 | 评估 |
|------|------|
| 技术可行 | ✅ 代码完整支持, K=1 路径有测试覆盖 |
| 逻辑自洽 | ✅ chain_enabled=false 下 K>1 根本不可能工作 |
| 安全可接受 | ✅ TEE seal 保护 + 用户自持 Token 可 revoke |
| 商业合理 | ✅ K/N 递增作为层级差异化天然合适 |
| 运维友好 | ✅ 最简启动路径, 零额外依赖 |

---

## 十、架构深度审查 — 问题与优化方案

> 审查日期: 2026-02-26

### 10.1 Critical: 订阅费未实际转移 — 经济模型空转 ✅ 已修复

**原位置:** `consensus/src/lib.rs` (已迁移到 `subscription/src/lib.rs:375`)

```rust
// on_era_end 收取订阅费:
T::Currency::unreserve(&sub.owner, sub.fee_per_era);  // ← 解锁回 owner 的 free balance!
subscription_income = subscription_income.saturating_add(sub.fee_per_era);  // ← 仅记账
```

```rust
// 节点领取奖励:
let _ = T::Currency::deposit_creating(&who, pending);  // ← 凭空铸币!
```

**问题:** `unreserve` 将订阅费退回 owner 的 free balance, 而节点奖励通过 `deposit_creating` 凭空铸造。**订阅费从未真正流向任何人** — owner 付了 reserve 后原样拿回, 节点奖励完全来自通胀铸币。`subscription_income` 和 `treasury_share` 仅为 EraRewardInfo 记录的纸面数字。

**对比广告 pallet (正确做法):**

```rust
// ads settle_era_ads:
T::Currency::unreserve(advertiser, *actual_cost);           // 解锁
T::Currency::transfer(advertiser, &treasury, *actual_cost, ...)?;  // 实际转账!
```

**影响:**
- 订阅费完全无经济意义 — owner 质押后拿回, 等于免费使用 Pro/Enterprise
- 节点奖励 100% 来自通胀, 无论有无订阅收入
- `treasury_share` (10%) 从未实际进入国库

**修复方案:**

```rust
// 方案 A: unreserve + transfer (与 ads 一致)
T::Currency::unreserve(&sub.owner, sub.fee_per_era);
T::Currency::transfer(&sub.owner, &treasury, sub.fee_per_era, AllowDeath)?;

// 方案 B: slash_reserved (直接从 reserve 扣除, 不经过 free balance)
let (_, remaining) = T::Currency::slash_reserved(&sub.owner, sub.fee_per_era);
// 然后 deposit_creating 给 treasury
```

---

### 10.2 High: consensus pallet 过度膨胀 (God Pallet) ✅ 已拆分

**拆分前 consensus 职责 (7 项):**

| 职责 | 行数 | 拆分去向 |
|------|------|-------|
| 节点注册 + 质押 | ~100 | 保留 (consensus) |
| 节点退出 + 冷却期 | ~60 | 保留 (consensus) |
| Equivocation 举报 + Slash | ~80 | 保留 (consensus) |
| 订阅 CRUD | ~120 | → subscription pallet |
| Era 奖励分配 | ~120 | → rewards pallet |
| 消息序列去重 | ~40 | 保留 (consensus) |
| TEE 证明验证 | ~30 | 保留 (consensus) |

**已完成拆分为 3 个 pallet:**

```
pallet-grouprobot-consensus  (瘦身后)
├── 节点注册/质押/退出
├── Equivocation 举报/Slash
├── TEE 证明验证
├── 消息序列去重
└── on_era_end 编排 (委托 SubscriptionSettler + EraRewardDistributor)

pallet-grouprobot-subscription  (runtime index 154)
├── subscribe / deposit / cancel / change_tier
├── Escrow 管理 + 游标分页结算
├── effective_tier() / effective_feature_gate() 查询
└── 实现 SubscriptionProvider + SubscriptionSettler trait

pallet-grouprobot-rewards  (runtime index 155)
├── 统一奖励池 (订阅费 + 广告费 + 通胀)
├── claim_rewards() 单一入口
├── 实现 RewardAccruer + EraRewardDistributor trait
└── Era 奖励记录 + 过期清理
```

**收益 (已实现):**
- 订阅逻辑独立迭代, 不影响共识安全
- 奖励系统统一, 节点通过 rewards pallet 单一 claim
- 测试粒度更细: subscription 18, rewards 11, consensus 27

---

### 10.3 High: on_era_end 全表扫描 O(N) — 不可扩展 ✅ 已修复 (游标分页)

**原位置:** `consensus/src/lib.rs` (已迁移到 `subscription/src/lib.rs:341-411`)

```rust
for (_bot_hash, sub) in Subscriptions::<T>::iter() {  // O(N) 全量遍历!
```

**问题:** 每个 Era 结束时遍历所有 Subscription 记录。如果有 10,000 个付费 Bot, 单次 `on_initialize` 可能超出区块 weight 限制。

**优化方案:**

| 方案 | 复杂度 | 说明 |
|------|--------|------|
| **A: 游标分页** | O(K) per block | `SubscriptionCursor` 每块处理 50 条, 多块完成 |
| **B: 活跃订阅索引** | O(M) | 维护 `ActiveSubscriptions: BoundedVec<BotIdHash>`, 仅遍历活跃的 |
| **C: 惰性扣费** | O(1) per query | 不在 Era 结束扣费, 而是在 `effective_tier()` 查询时检查 `paid_until_era < current_era` 按需扣费 |

**推荐方案 C (惰性扣费):**

```rust
pub fn effective_tier(bot_id_hash: &BotIdHash) -> SubscriptionTier {
    let current_era = CurrentEra::<T>::get();
    match Subscriptions::<T>::get(bot_id_hash) {
        Some(sub) => {
            if sub.paid_until_era >= current_era {
                return sub.tier;  // 已付费
            }
            // 惰性扣费: 尝试从 escrow 扣除
            let eras_owed = current_era - sub.paid_until_era;
            let total_owed = sub.fee_per_era * eras_owed;
            let escrow = SubscriptionEscrow::<T>::get(bot_id_hash);
            if escrow >= total_owed {
                // 扣费 + 更新 paid_until_era
                sub.tier
            } else {
                SubscriptionTier::Free  // 余额不足
            }
        }
        None => SubscriptionTier::Free,
    }
}
```

- **优点:** `on_era_end` 不再需要遍历订阅, 仅更新 Era 计数器
- **缺点:** 扣费时机不确定, 但对于付费用户, 每次消息处理都会查询 tier, 自然触发

---

### 10.4 High: 双重奖励系统 — 节点需 claim 两次 

**现状:**

| Pallet | Storage | Claim 函数 |
|--------|---------|-----------|
| consensus | `NodePendingRewards` | `claim_rewards` (铸币) |
| ads | `NodeAdPendingRewards` | `claim_node_ad_revenue` (从国库转) |

**问题:** 节点运营者需要调用两个不同 pallet 的 claim 函数, 且资金来源不同 (一个铸币, 一个从国库转)。

**优化: 统一奖励 Trait**

```rust
pub trait RewardAccruer<AccountId, Balance> {
    fn accrue_node_reward(node_id: &NodeId, amount: Balance);
}

// ads pallet 和 subscription 都通过此 trait 写入同一 storage
// 单一 claim 入口
```

---

### 10.5 Medium: 广告分成比例硬编码 ✅ 已参数化

**原位置:** `ads/src/lib.rs`

**已修复:** `community_pct` 从硬编码 80 改为 `CommunityAdPct` StorageValue (默认 80), 可通过 `set_community_ad_percentage` (Root) 治理调整。约束: `community_pct >= 50`, `community_pct + tee_pct <= 100`。

```rust
#[pallet::storage]
pub type CommunityAdPct<T: Config> = StorageValue<_, u32, ValueQuery>;  // 默认 80
```

---

### 10.6 Medium: 广告 pallet 不检查订阅层级 ✅ 已添加

**已修复:** `submit_delivery_receipt` 现在通过 `T::Subscription::effective_feature_gate()` 检查:

1. `gate.tee_access` — Free/Basic 层级无 TEE 权限, 拒绝收据提交 (`TeeNotAvailableForTier`)
2. `gate.can_disable_ads` — Pro/Enterprise 可禁用广告, 若社区无质押则拒绝 (`AdsDisabledByTier`)

> 注: `forced_ads_per_day` 仍为 off-chain TEE 执行, 链上未验证每日强制广告数。

---

### 10.7 Medium: cancel_subscription 可逃避当期费用 ✅ 已修复

**原位置:** `consensus/src/lib.rs` (已迁移到 `subscription/src/lib.rs:231-282`)

```rust
pub fn cancel_subscription(...) {
    sub.status = SubscriptionStatus::Cancelled;
    let escrow = SubscriptionEscrow::<T>::take(&bot_id_hash);
    T::Currency::unreserve(&who, escrow);  // 全额退还!
}
```

**问题:** 用户可以在 Era 中途 cancel, 退还全部 escrow (包括当期费用)。相当于免费使用了部分 Era 的服务。

**优化:** cancel 时扣除当期按比例费用:

```rust
let blocks_used = now - EraStartBlock::<T>::get();
let era_length = T::EraLength::get();
let prorated_fee = sub.fee_per_era * blocks_used / era_length;
let refundable = escrow.saturating_sub(prorated_fee);
```

---

### 10.8 Medium: deposit_subscription 不验证调用者身份 ✅ 已修复

**原位置:** `consensus/src/lib.rs` (已迁移到 `subscription/src/lib.rs:200-226`)

```rust
pub fn deposit_subscription(origin, bot_id_hash, amount) {
    let who = ensure_signed(origin)?;
    // ← 不检查 who == sub.owner!
    T::Currency::reserve(&who, amount)?;
}
```

**影响:** 任何人都能为任何 Bot 的订阅充值。可能是有意的 (允许赞助), 但也意味着:
- 恶意用户可以为 Suspended Bot 充值使其 **重新激活** (改变 owner 的预期)
- 充值者的 reserve 被锁定, 但 cancel 时退还给 `sub.owner` 而非充值者

**优化:** 至少记录充值来源, 或限制仅 owner 可充值:

```rust
ensure!(sub.owner == who, Error::<T>::NotBotOwner);
```

---

### 10.9 Medium: CommunityAdmin 确定方式脆弱 ✅ 已修复

**位置:** `ads/src/lib.rs` `stake_for_ads`

```rust
if !CommunityAdmin::<T>::contains_key(&community_id_hash) {
    CommunityAdmin::<T>::insert(&community_id_hash, who.clone());  // 首个质押者!
}
```

**问题:** 社区管理员 = 第一个质押的人, 无法转移 (除 Root)。如果错误的人先质押, 真正的群主无法提取广告收入。

**优化:** 管理员应由链上社区注册流程决定, 或允许质押者投票选举:

```rust
// 方案 A: 绑定 Bot Owner
let bot_owner = T::BotRegistry::bot_owner(&associated_bot)?;
CommunityAdmin::<T>::insert(&community_id_hash, bot_owner);

// 方案 B: 多签管理
pub struct CommunityAdminConfig {
    admin: AccountId,
    transfer_pending: Option<(AccountId, BlockNumber)>,  // 待确认转移
}
```

---

### 10.10 Low: TierFeatureGate 包含已废弃字段

**位置:** `primitives/src/lib.rs:249-254`

```rust
/// 广告收入分成: 社区 % (deprecated: 实际由 ads pallet 治理配置)
pub ad_revenue_community_pct: u8,
/// 广告收入分成: 国库 % (deprecated: 实际由 ads pallet 治理配置)
pub ad_revenue_treasury_pct: u8,
/// 广告收入分成: 节点 % (deprecated: 实际由 ads pallet 治理配置)
pub ad_revenue_node_pct: u8,
```

这三个字段已标注 deprecated 但仍在 struct 中, 每次 `feature_gate()` 都计算并返回。

---

### 10.11 架构优化总览

**优先级排序:**

| ID | 严重度 | 问题 | 状态 | 说明 |
|----|--------|------|------|------|
| **10.1** | 🔴 Critical | 订阅费未转移, 经济空转 | ✅ 已修复 | unreserve+transfer 到国库 |
| **10.2** | 🟠 High | consensus 过度膨胀 | ✅ 已拆分 | subscription + rewards 独立 pallet |
| **10.3** | 🔴 High | on_era_end O(N) 不可扩展 | ✅ 已修复 | 游标分页 (SubscriptionSettleCursor) |
| **10.4** | � High | 双重奖励系统 | ✅ 已统一 | rewards pallet 单一 claim |
| **10.5** | 🟡 Medium | 广告分成硬编码 | ✅ 已参数化 | CommunityAdPct StorageValue |
| **10.6** | � Medium | 广告不检查层级 | ✅ 已添加 | tee_access + can_disable_ads 检查 |
| **10.7** | 🟡 Medium | cancel 逃避当期费用 | ✅ 已修复 | 按比例扣除 (blocks_used/era_length) |
| **10.8** | 🟡 Medium | deposit 不验证身份 | ✅ 已修复 | ensure!(sub.owner == who) |
| **10.9** | 🟡 Medium | CommunityAdmin 脆弱 | ✅ 已修复 | 绑定 Bot Owner (BotRegistryProvider) |
| **10.10** | 🟢 Low | 废弃字段未移除 | → 待处理 | 需 storage migration, 低优先级 |

---

### 10.12 当前架构 (已实现)

```
┌─────────────────────────────────────────────────────────┐
│                     primitives (共享类型)                  │
│  SubscriptionTier, TierFeatureGate, NodeId, BotIdHash   │
│  + SubscriptionProvider trait (查询接口)                   │
│  + RewardAccruer trait (统一写入接口)                      │
└─────────────────────────────────────────────────────────┘
         │                    │                    │
    ┌────▼─────┐      ┌──────▼──────┐     ┌──────▼──────┐
    │  node    │      │subscription │     │   rewards   │
    │ (共识)   │      │  (订阅)      │     │  (奖励)     │
    ├──────────┤      ├─────────────┤     ├─────────────┤
    │ 注册/质押 │      │ subscribe   │     │ 统一奖励池   │
    │ 退出/Slash│      │ deposit     │     │ 订阅+广告+   │
    │ TEE 验证  │      │ cancel      │     │ 通胀铸币    │
    │ 序列去重  │      │ change_tier │     │ claim_all   │
    │          │      │ 惰性扣费     │     │ Era 记录    │
    └──────────┘      └─────────────┘     └─────────────┘
                              │
    ┌──────────┐      ┌──────▼──────┐     ┌─────────────┐
    │community │      │    ads      │     │  registry   │
    │ (社区)   │      │  (广告)      │     │  (Bot注册)  │
    ├──────────┤      ├─────────────┤     ├─────────────┤
    │ 配置/CAS │      │ Campaign    │     │ register_bot│
    │ 日志/声誉 │      │ CPM + 结算  │     │ 证明/Peer   │
    │          │      │ 反作弊      │     │ Ceremony    │
    │          │      │ 层级检查 ←──│─────│ tier query  │
    └──────────┘      └─────────────┘     └─────────────┘
```

**已完成的核心变更:**

1. **subscription 独立 pallet** — 订阅 CRUD + 游标分页结算, 暴露 `SubscriptionProvider` + `SubscriptionSettler` trait
2. **rewards 统一 pallet** — ads 和 consensus 都通过 `RewardAccruer` trait 写入同一奖励池
3. **ads 查询 subscription tier** — `submit_delivery_receipt` 检查 `tee_access` + `can_disable_ads`, 链上执行功能限制
4. **订阅费实际转移** — `unreserve + transfer` 到国库, 不再空转
5. **on_era_end 轻量化** — 委托 `SubscriptionSettler::settle_era()` + `EraRewardDistributor::distribute_and_record()`, 不遍历订阅
6. **Tier 门控** — ceremony/registry/community/consensus 4 pallets 添加 `ensure!(tier.is_paid())` 检查

---

## 11. 实施状态 (Implementation Status)

> 以下修复已全部实施并通过测试验证。

### 11.1 已完成修复

| 编号 | 优先级 | 描述 | 修改文件 | 状态 |
|------|--------|------|----------|------|
| 10.1 | **Critical** | `on_era_end` 订阅费 `unreserve→transfer` 到国库 | consensus/lib.rs | ✅ |
| 10.3 | **High** | `on_era_end` 游标分页替代全表 `iter()` | consensus/lib.rs | ✅ |
| 10.5 | **Medium** | ads `community_pct` 从硬编码 80 改为 `CommunityAdPct` StorageValue | ads/lib.rs | ✅ |
| 10.7 | **Medium** | `cancel_subscription` 按比例扣除当期费用 | consensus/lib.rs | ✅ |
| 10.8 | **Medium** | `deposit_subscription` 添加 owner 身份验证 | consensus/lib.rs | ✅ |
| Trait | **Medium** | primitives 添加 `SubscriptionProvider` + `RewardAccruer` trait | primitives/lib.rs | ✅ |

### 11.2 修改文件清单

**pallet-grouprobot-consensus** (`pallets/grouprobot/consensus/src/`):
- `lib.rs`: 10.1 + 10.3 + 10.7 + 10.8 修复
  - 新增 Config: `TreasuryAccount`, `MaxSubscriptionSettlePerEra`
  - 新增 Storage: `SubscriptionSettleCursor`, `SubscriptionSettlePending`
  - 新增 Event: `SubscriptionFeeCollected`, `SubscriptionCancelledWithProration`
  - 新增 Error: `SubscriptionFeeTransferFailed`, `NotSubscriptionOwner`
  - `on_era_end`: unreserve+transfer 替代空转, 游标分页
  - `cancel_subscription`: 按比例扣费 (blocks_used / era_length)
  - `deposit_subscription`: 仅 owner 可充值
- `mock.rs`: 新增 `TREASURY` 账户, `TreasuryAccount`, `MaxSubscriptionSettlePerEra`
- `tests.rs`: 3 新测试 (54 total, was 51)
  - `era_end_subscription_fee_transfers_to_treasury` — 验证 10.1 国库收款
  - `cancel_subscription_prorated_fee_mid_era` — 验证 10.7 按比例扣费
  - `deposit_subscription_fails_non_owner` — 验证 10.8 owner 检查

**pallet-grouprobot-ads** (`pallets/grouprobot/ads/src/`):
- `lib.rs`: 10.5 修复
  - 新增 Storage: `CommunityAdPct` (默认 80, 可治理调整)
  - 新增 Event: `CommunityAdPercentUpdated`
  - 新增 Extrinsic: `set_community_ad_percentage` (call_index 24, Root origin)
  - `settle_era_ads`: `community_pct` 从 `CommunityAdPct` 读取
  - `set_tee_ad_percentage`: 约束改为动态 `community_pct + tee_pct <= 100`

**pallet-grouprobot-primitives** (`pallets/grouprobot/primitives/src/`):
- `lib.rs`: 新增 2 个 trait
  - `SubscriptionProvider`: `effective_tier()`, `effective_feature_gate()`
  - `RewardAccruer`: `accrue_node_reward()`
  - 均含 `()` 空实现用于测试

**runtime** (`runtime/src/configs/mod.rs`):
- consensus Config 新增: `TreasuryAccount = TreasuryAccountId`, `MaxSubscriptionSettlePerEra = ConstU32<200>`

### 11.3 验证结果

```
cargo test -p pallet-grouprobot-consensus: 54/54 ✅
cargo test -p pallet-grouprobot-ads:       63/63 ✅
cargo check -p nexus-runtime:              ✅ (clean)
```

### 11.4 Phase 2: Trait 桥接 + ads 集成 (已完成)

| 编号 | 描述 | 修改文件 | 状态 |
|------|------|----------|------|
| 10.2 | consensus `impl SubscriptionProvider` | consensus/lib.rs | ✅ |
| 10.4 | consensus `impl RewardAccruer` | consensus/lib.rs | ✅ |
| 10.6 | ads 添加 `type Subscription: SubscriptionProvider` + `type RewardPool: RewardAccruer` | ads/lib.rs | ✅ |
| 10.6 | ads `settle_era_ads` 通过 `RewardPool` trait 写入统一奖励池 | ads/lib.rs | ✅ |
| 10.6 | ads mock: `MockSubscription` + `MockRewardPool` | ads/mock.rs | ✅ |

**验证:** consensus 54/54 ✅, ads 63/63 ✅, nexus-runtime check ✅

### 11.5 Phase 3: 物理 Pallet 拆分 (已完成)

> 实施日期: 2026-02-26

| 编号 | 描述 | 修改文件 | 状态 |
|------|------|----------|------|
| 10.2 | subscription 物理独立 pallet 拆分 | subscription/ (新建) + consensus/lib.rs (瘦身) | ✅ |
| 10.2 | rewards 物理独立 pallet 拆分 | rewards/ (新建) + consensus/lib.rs (瘦身) | ✅ |
| 10.4 | ads `claim_node_ad_revenue` → 统一 rewards claim | ads/lib.rs + rewards/lib.rs | ✅ |
| 10.9 | CommunityAdmin 绑定 Bot Owner | ads/lib.rs | ✅ |

**新建 Pallet:**

- **`pallet-grouprobot-subscription`** (runtime index 154)
  - Extrinsics: `subscribe`, `deposit_subscription`, `cancel_subscription`, `change_tier`
  - Storage: `Subscriptions`, `SubscriptionEscrow`, `SubscriptionSettleCursor`, `SubscriptionSettlePending`
  - 实现: `SubscriptionProvider`, `SubscriptionSettler`
  - 测试: 18/18 ✅

- **`pallet-grouprobot-rewards`** (runtime index 155)
  - Extrinsics: `claim_rewards`
  - Storage: `NodePendingRewards`, `NodeTotalEarned`, `EraRewards`, `EraCleanupCursor`
  - 实现: `RewardAccruer`, `EraRewardDistributor`
  - 测试: 11/11 ✅

**10.4 Ads 统一 Claim:**
- 移除: `NodeAdPendingRewards`, `NodeAdTotalEarned` 存储, `claim_node_ad_revenue` extrinsic (call_index 21)
- 移除: `NodeAdRevenueClaimed` 事件, `NoNodeAdReward`/`NotNodeOperator` 错误
- `settle_era_ads` 仅通过 `T::RewardPool::accrue_node_reward()` 写入统一奖励池
- 节点运营者通过 rewards pallet `claim_rewards` 统一领取
- 测试: 65/65 ✅ (was 63, +5 新 10.9 测试, -3 移除的 claim 测试)

**10.9 CommunityAdmin 绑定 Bot Owner:**
- ads Config 新增: `type BotRegistry: BotRegistryProvider<Self::AccountId>`
- `stake_for_ads` 首次设置管理员时: 优先查询 `T::BotRegistry::bot_owner()`, 无 owner 时回退到质押者
- 新增 5 个回归测试: `stake_sets_admin_to_bot_owner`, `stake_falls_back_to_staker_when_no_bot_owner`, `second_staker_does_not_change_admin`, `bot_owner_can_manage_community_preferences`, `non_bot_owner_cannot_manage_community`

**Consensus Pallet 瘦身:**
- 移除: 订阅 extrinsics (call_index 5-8), claim_rewards (call_index 9), 订阅/奖励存储/事件/错误
- `on_era_end` 改为委托: `T::SubscriptionSettler::settle_era()` + `T::RewardDistributor::distribute_and_record()`
- 保留: 节点管理、举报/惩罚、TEE 验证、序列去重
- 测试: 25/25 ✅

**Tier 门控实现 (4 pallets):**
- **ceremony**: `record_ceremony` 新增 `ensure!(tier.is_paid())`, 新错误 `FreeTierNotAllowed`
- **registry**: `register_peer` + `heartbeat_peer` 新增 `ensure!(tier.is_paid())`, 新错误 `FreeTierNotAllowed`
- **community**: `submit_action_log` + `batch_submit_logs` 新增 `ensure!(tier.is_paid())`, 新错误 `FreeTierNotAllowed`
- **community**: `clear_expired_logs` 强制 `max_age_blocks >= log_retention_days * 14400`, Enterprise(0)禁止清理, 新错误 `RetentionPeriodNotExpired`
- **consensus**: `mark_sequence_processed` 新增 `ensure!(tier.is_paid())`, 新错误 `FreeTierNotAllowed`
- 4 pallets Config 新增: `type Subscription: SubscriptionProvider`
- Runtime: 4 pallets 均使用 `pallet_grouprobot_subscription::Pallet<Runtime>` 作为 Subscription 提供者
- 新增测试 8 个, 总计 119 测试全部通过
- `max_rules` 为纯 off-chain Bot 规则引擎配置, 无需链上门控

**Runtime 集成:**
- 新增桥接: `GrEraStartBlockProvider`, `GrCurrentEraProvider`, `GrNodeConsensusBridge`
- `cargo check -p nexus-runtime` ✅

### 11.6 未实施项 (留待后续)

| 编号 | 描述 | 原因 |
|------|------|------|
| 10.10 | TierFeatureGate 废弃字段移除 | 需 storage migration, 低优先级 |
