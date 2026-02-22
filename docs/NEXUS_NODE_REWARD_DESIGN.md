# Nexus 节点奖励设计 — 订阅费 + 通胀混合模型

> **核心原则：订阅费（A）提供市场驱动收入，通胀保底（B）解决冷启动，两者按阶段动态调整比例。**

---

## 一、现状分析

### 1.1 当前经济模型（仅惩罚，无奖励）

| 机制 | 类型 | 状态 |
|---|---|---|
| 质押 MinStake (100 NEX) | 经济安全 | ✅ 已实现 |
| 信誉系统 (0-10000) | 质量追踪 | ✅ 已实现 |
| Leader 成功 +1 信誉 | 微弱激励 | ✅ 已实现 |
| 离线举报 -10/条 | 惩罚 | ✅ 已实现 |
| 连续超时 -100 | 惩罚 | ✅ 已实现 |
| Equivocation Slash (10%) | 惩罚 | ✅ 已实现 |
| 举报者奖励 (Slash 50%) | 举报激励 | ✅ 已实现 |
| **节点服务奖励** | **正向激励** | **❌ 缺失** |

### 1.2 问题

节点运营者承担：服务器 + 带宽 + 运维 + 质押锁定成本，但**零收益**。纯靠"不被 Slash"无法吸引节点加入。

---

## 二、混合模型总览

### 2.1 双收入来源

```
节点收入 = 通胀保底 (B) + 订阅费分成 (A)

┌──────────────────────────────────────────────────┐
│                   节点总收入                       │
│                                                    │
│  ┌────────────────┐  ┌────────────────────────┐   │
│  │  通胀保底 (B)   │  │  订阅费分成 (A)         │   │
│  │                 │  │                         │   │
│  │  来源: 铸币     │  │  来源: 群主付费          │   │
│  │  固定/递减      │  │  市场驱动               │   │
│  │  解决冷启动     │  │  长期可持续             │   │
│  └────────────────┘  └────────────────────────┘   │
└──────────────────────────────────────────────────┘
```

### 2.2 阶段比例

| 阶段 | 时间 | 通胀 (B) | 订阅 (A) | 说明 |
|---|---|---|---|---|
| **Phase 0** | 0-6 月 | 80% | 20% | 冷启动，全免费试用 |
| **Phase 1** | 6-12 月 | 50% | 50% | Basic 免费，Pro/Enterprise 收费 |
| **Phase 2** | 12-24 月 | 20% | 80% | 全面订阅制 |
| **Phase 3** | 24+ 月 | 0% | 100% | 纯市场驱动 |

---

## 三、模型 A — 群主订阅费

### 3.1 价值主张

群主付费购买的不是"机器人功能"（中心化方案免费），而是：

| 价值 | 中心化 Bot | Nexus |
|---|---|---|
| 数据主权 | ❌ 第三方持有 | ✅ 群主自持 Agent |
| 审查抗性 | ❌ 可被封禁 | ✅ 去中心化节点 |
| 链上存证 | ❌ | ✅ ActionLog |
| Token-Gating | ❌ | ✅ 链上余额/身份门槛 |
| 自定义规则 | 有限 | ✅ 可扩展 Rule Chain |

**目标客户：** Web3 项目方、DAO 社区、Token 持有者社群。

### 3.2 分层定价

```rust
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum SubscriptionTier {
    /// 基础版: 单群、基础规则
    Basic,
    /// 专业版: 多群、高级规则、优先响应
    Pro,
    /// 企业版: 无限群、自定义规则、SLA 保障
    Enterprise,
}
```

| 层级 | 群数量 | 功能 | 月费 (NEX) |
|---|---|---|---|
| **Basic** | 1 群 | 基础过滤 + 反垃圾 + 欢迎 | 10 NEX |
| **Pro** | ≤5 群 | 全功能 + Captcha + 白名单 | 30 NEX |
| **Enterprise** | 无限 | 全功能 + SLA + 优先 Leader | 100 NEX |

定价考虑 USD 锚定（治理可调），避免 NEX 价格波动影响。

### 3.3 付费周期

```
支持: 月付 / 季付(9折) / 年付(8折)
实现: 预付制 — 群主预存 NEX 到链上 Escrow，按 Era 扣费
到期: 余额不足 → 1 Era 宽限 → BotStatus::Suspended → 节点停止服务
```

### 3.4 资金流转

```
群主钱包
    │
    │ subscribe_bot(tier, periods) / deposit_subscription(amount)
    ▼
┌─────────────────────────┐
│  SubscriptionEscrow      │  ← 预存订阅费 (链上锁定)
│  (bot_id_hash → Balance) │
└──────────┬──────────────┘
           │ on_era_end() 自动扣费
           ▼
┌─────────────────────────┐
│  总收入分配              │
│  ├─ 80% → 节点奖励池    │  → EraRewardPool
│  ├─ 10% → 协议国库      │  → Treasury
│  └─ 10% → Agent 补贴    │  → 返还群主 (抵扣 Agent 运营成本)
└──────────┬──────────────┘
           │ distribute_rewards()
           ▼
    ┌─────────────────┐
    │  NodePendingRewards │  ← 各节点待领取
    │  (node_id → Balance)│
    └─────────────────┘
           │ claim_rewards()
           ▼
      节点运营者钱包
```

---

## 四、模型 B — 通胀保底

### 4.1 通胀参数

```rust
parameter_types! {
    /// 每 Era 铸币量 (Phase 0)
    pub const InflationPerEra: Balance = 100 * UNIT;  // 100 NEX/天
    /// 通胀衰减率 (每 Phase 乘以此系数)
    pub const InflationDecayRate: Perbill = Perbill::from_percent(50);
    /// Era 长度 (区块数)
    pub const EraLength: BlockNumber = DAYS;  // 1 天
    /// 最低在线率要求 (低于此值不获得通胀奖励)
    pub const MinUptimeForReward: Perbill = Perbill::from_percent(80);
}
```

### 4.2 通胀时间表

| Phase | 每 Era 铸币 | 年通胀量 | 说明 |
|---|---|---|---|
| 0 (0-6月) | 100 NEX | 36,500 NEX | 冷启动保障 |
| 1 (6-12月) | 50 NEX | 18,250 NEX | 订阅收入增长 |
| 2 (12-24月) | 25 NEX | 9,125 NEX | 订阅为主 |
| 3 (24+月) | 0 NEX | 0 | 纯订阅驱动 |

### 4.3 通胀发放条件

```
节点获得通胀奖励的前提条件:
  ① status == Active (非 Probation/Suspended/Exiting)
  ② uptime >= 80% (confirmed / (confirmed + missed))
  ③ reputation >= SuspendThreshold (2000)
  ④ 该 Era 内有实际参与消息处理

不满足条件 → 该 Era 通胀份额销毁 (不重新分配)
```

---

## 五、节点奖励权重

### 5.1 权重计算

A 和 B 两部分使用相同的权重公式分配：

```rust
/// 计算节点奖励权重
/// 返回: 0 ~ 10000 (basis points)
pub fn compute_node_weight(
    node: &ProjectNode,
    stats: &LeaderStats,
) -> u128 {
    // 1. 信誉权重 (0 ~ 10000)
    let rep = node.reputation as u128;

    // 2. 在线率 (0 ~ 10000)
    let total = node.messages_confirmed + node.messages_missed;
    let uptime = if total == 0 {
        5000u128  // 新节点默认 50%
    } else {
        (node.messages_confirmed as u128 * 10000) / total as u128
    };

    // 3. Leader 成功率加成 (10000 ~ 15000)
    let leader_bonus = if stats.total_leads == 0 {
        10000u128
    } else {
        10000 + (stats.successful as u128 * 5000) / stats.total_leads as u128
    };

    // weight = rep × uptime × leader_bonus / 10^8
    // 范围: 0 ~ 10000 × 10000 × 15000 / 10^8 = 15000
    rep * uptime * leader_bonus / 100_000_000
}
```

### 5.2 权重因子说明

| 因子 | 权重 | 范围 | 说明 |
|---|---|---|---|
| **信誉** | 线性 | 0-10000 | 长期行为积累 |
| **在线率** | 线性 | 0-10000 | 本 Era 实际可用性 |
| **Leader 成功率** | 加成 | 1.0-1.5x | 执行能力奖励 |

### 5.3 奖励上限

```
单节点每 Era 最大奖励 = 总池 × 30%
目的: 防止大户垄断，保障小节点收益
```

---

## 六、链上实现

### 6.1 新增数据结构

```rust
/// 订阅信息
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Subscription<T: Config> {
    pub owner: T::AccountId,
    pub bot_id_hash: [u8; 32],
    pub tier: SubscriptionTier,
    pub fee_per_era: BalanceOf<T>,
    pub started_at: BlockNumberFor<T>,
    pub paid_until_era: u32,
    pub status: SubscriptionStatus,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum SubscriptionStatus {
    Active,       // 正常
    PastDue,      // 欠费宽限 (1 Era)
    Suspended,    // 欠费暂停
    Cancelled,    // 主动取消
}

/// Era 奖励快照
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EraRewardInfo<Balance> {
    pub subscription_income: Balance,   // 订阅费收入
    pub inflation_mint: Balance,        // 通胀铸币
    pub total_distributed: Balance,     // 实际分配给节点
    pub treasury_share: Balance,        // 国库份额
    pub node_count: u32,                // 参与分配的节点数
}
```

### 6.2 新增存储

```rust
/// 订阅信息: bot_id_hash → Subscription
#[pallet::storage]
pub type Subscriptions<T> = StorageMap<
    _, Blake2_128Concat, [u8; 32], Subscription<T>>;

/// 订阅 Escrow 余额: bot_id_hash → Balance
#[pallet::storage]
pub type SubscriptionEscrow<T> = StorageMap<
    _, Blake2_128Concat, [u8; 32], BalanceOf<T>, ValueQuery>;

/// 当前 Era
#[pallet::storage]
pub type CurrentEra<T> = StorageValue<_, u32, ValueQuery>;

/// Era 奖励信息: era → EraRewardInfo
#[pallet::storage]
pub type EraRewards<T> = StorageMap<
    _, Twox64Concat, u32, EraRewardInfo<BalanceOf<T>>>;

/// 节点待领取奖励: node_id → Balance
#[pallet::storage]
pub type NodePendingRewards<T> = StorageMap<
    _, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;

/// 节点历史总收益: node_id → Balance
#[pallet::storage]
pub type NodeTotalEarned<T> = StorageMap<
    _, Blake2_128Concat, NodeId, BalanceOf<T>, ValueQuery>;
```

### 6.3 新增 Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    // ... 现有配置 ...

    /// Era 长度 (区块数)
    #[pallet::constant]
    type EraLength: Get<BlockNumberFor<Self>>;

    /// 每 Era 基础通胀铸币量
    #[pallet::constant]
    type InflationPerEra: Get<BalanceOf<Self>>;

    /// 最低在线率 (获得奖励的门槛)
    #[pallet::constant]
    type MinUptimeForReward: Get<Perbill>;

    /// 节点单 Era 奖励上限 (占总池比例)
    #[pallet::constant]
    type MaxRewardShare: Get<Perbill>;

    /// 订阅费 Basic 层级每 Era 费用
    #[pallet::constant]
    type BasicFeePerEra: Get<BalanceOf<Self>>;

    /// 订阅费 Pro 层级每 Era 费用
    #[pallet::constant]
    type ProFeePerEra: Get<BalanceOf<Self>>;

    /// 订阅费 Enterprise 层级每 Era 费用
    #[pallet::constant]
    type EnterpriseFeePerEra: Get<BalanceOf<Self>>;
}
```

### 6.4 Runtime 参数

```rust
parameter_types! {
    pub const EraLength: BlockNumber = DAYS;          // 1 天
    pub const InflationPerEra: Balance = 100 * UNIT;  // 100 NEX/天
    pub const MinUptimeForReward: Perbill = Perbill::from_percent(80);
    pub const MaxRewardShare: Perbill = Perbill::from_percent(30);
    pub const BasicFeePerEra: Balance = 10 * UNIT / 30;      // ~0.33 NEX/天
    pub const ProFeePerEra: Balance = 30 * UNIT / 30;        // 1 NEX/天
    pub const EnterpriseFeePerEra: Balance = 100 * UNIT / 30; // ~3.33 NEX/天
}
```

### 6.5 新增 Extrinsic

```rust
// ─── 群主操作 ───

/// 开通订阅 (预存 N 个月费用)
#[pallet::call_index(9)]
pub fn subscribe(
    origin: OriginFor<T>,
    bot_id_hash: [u8; 32],
    tier: SubscriptionTier,
    deposit_amount: BalanceOf<T>,
) -> DispatchResult;

/// 充值订阅余额
#[pallet::call_index(10)]
pub fn deposit_subscription(
    origin: OriginFor<T>,
    bot_id_hash: [u8; 32],
    amount: BalanceOf<T>,
) -> DispatchResult;

/// 取消订阅 (剩余 Escrow 退还)
#[pallet::call_index(11)]
pub fn cancel_subscription(
    origin: OriginFor<T>,
    bot_id_hash: [u8; 32],
) -> DispatchResult;

/// 升级/降级层级
#[pallet::call_index(12)]
pub fn change_tier(
    origin: OriginFor<T>,
    bot_id_hash: [u8; 32],
    new_tier: SubscriptionTier,
) -> DispatchResult;

// ─── 节点操作 ───

/// 领取奖励
#[pallet::call_index(13)]
pub fn claim_rewards(
    origin: OriginFor<T>,
    node_id: NodeId,
) -> DispatchResult;
```

### 6.6 on_era_end 核心逻辑

```rust
fn on_era_end(era: u32) {
    // ═══ 1. 收取订阅费 ═══
    let mut subscription_income: BalanceOf<T> = Zero::zero();

    for (bot_hash, mut sub) in Subscriptions::<T>::iter() {
        if sub.status == SubscriptionStatus::Cancelled { continue; }

        let escrow = SubscriptionEscrow::<T>::get(&bot_hash);
        if escrow >= sub.fee_per_era {
            // 扣费成功
            SubscriptionEscrow::<T>::mutate(&bot_hash, |e| {
                *e = e.saturating_sub(sub.fee_per_era);
            });
            subscription_income = subscription_income.saturating_add(sub.fee_per_era);
            sub.paid_until_era = era;
            sub.status = SubscriptionStatus::Active;
        } else {
            // 欠费处理
            sub.status = match sub.status {
                SubscriptionStatus::Active => SubscriptionStatus::PastDue,
                SubscriptionStatus::PastDue => SubscriptionStatus::Suspended,
                other => other,
            };
        }
        Subscriptions::<T>::insert(&bot_hash, sub);
    }

    // ═══ 2. 订阅收入分配 ═══
    let node_share = subscription_income * 80u32.into() / 100u32.into();
    let treasury_share = subscription_income * 10u32.into() / 100u32.into();
    // 剩余 10% → Agent 补贴 (保留在 pool 由群主 claim)

    // ═══ 3. 通胀铸币 ═══
    let inflation = T::InflationPerEra::get();
    // T::Currency::deposit_creating(&reward_pool, inflation);

    // ═══ 4. 合并奖励池 ═══
    let total_pool = node_share.saturating_add(inflation);

    // ═══ 5. 按权重分配给节点 ═══
    let active_nodes = ActiveNodeList::<T>::get();
    let mut eligible: Vec<(NodeId, u128)> = Vec::new();
    let mut total_weight: u128 = 0;

    for node_id in &active_nodes {
        let node = Nodes::<T>::get(node_id).unwrap();
        let stats = LeaderStatsStore::<T>::get(node_id);

        // 在线率检查
        let total_msgs = node.messages_confirmed + node.messages_missed;
        let uptime_ok = if total_msgs == 0 { true } else {
            Perbill::from_rational(node.messages_confirmed, total_msgs)
                >= T::MinUptimeForReward::get()
        };

        if node.status == NodeStatus::Active && uptime_ok {
            let w = compute_node_weight(&node, &stats);
            total_weight += w;
            eligible.push((node_id.clone(), w));
        }
    }

    if total_weight > 0 {
        let max_per_node = T::MaxRewardShare::get() * total_pool;

        for (node_id, w) in &eligible {
            let raw_reward = total_pool * (*w as u128) / total_weight;
            let capped = raw_reward.min(max_per_node);

            NodePendingRewards::<T>::mutate(node_id, |p| {
                *p = p.saturating_add(capped);
            });
            NodeTotalEarned::<T>::mutate(node_id, |t| {
                *t = t.saturating_add(capped);
            });
        }
    }

    // ═══ 6. 记录 Era 信息 ═══
    EraRewards::<T>::insert(era, EraRewardInfo {
        subscription_income,
        inflation_mint: inflation,
        total_distributed: total_pool,
        treasury_share,
        node_count: eligible.len() as u32,
    });

    // ═══ 7. 推进 Era ═══
    CurrentEra::<T>::put(era + 1);
}
```

---

## 七、经济可持续性

### 7.1 节点盈亏分析

```
节点月成本:
  VPS (2C4G):          ~$20
  带宽 (1TB):          ~$5
  运维:                ~$10
  质押机会成本:        100 NEX × 5% / 12 ≈ 0.4 NEX
  ─────────────────────
  总计:                ≈ $35 ≈ 70 NEX (假设 1 NEX ≈ $0.5)
```

### 7.2 各阶段收益预估 (3 节点)

| 阶段 | 订阅数 | 订阅收入/Era | 通胀/Era | 节点池/Era | 节点月收益 |
|---|---|---|---|---|---|
| **Phase 0** | 0-5 | 0-5 NEX | 100 NEX | ~100 NEX | ~1000 NEX |
| **Phase 1** | 10-30 | 10-30 NEX | 50 NEX | ~60-80 NEX | ~600-800 NEX |
| **Phase 2** | 50-100 | 50-100 NEX | 25 NEX | ~65-105 NEX | ~650-1050 NEX |
| **Phase 3** | 100+ | 100+ NEX | 0 | ~80+ NEX | ~800+ NEX |

### 7.3 盈亏临界点

```
Phase 0: 通胀 100 NEX/天 ÷ 3 节点 = 33 NEX/节点/天 = 1000 NEX/月
         >> 成本 70 NEX/月  ✅ 始终盈利

Phase 3 (无通胀): 需要多少订阅?
  3 节点成本 = 210 NEX/月 = 7 NEX/天
  节点池占 80% → 总收入 = 7 / 0.8 = 8.75 NEX/天
  Pro 费率 1 NEX/天 → 至少 9 个 Pro 订阅
  Basic 费率 0.33 NEX/天 → 至少 27 个 Basic 订阅

  → Phase 3 最少 ~10 个付费群主即可盈亏平衡
```

### 7.4 通胀影响评估

```
Phase 0 年通胀: 36,500 NEX
假设初始总供应: 10,000,000 NEX
年通胀率: 0.365%  ← 极低，可忽略

全周期 (0-24月) 累计通胀: 36,500 + 18,250 + 9,125 = 63,875 NEX
占总供应: 0.64%  ← 完全可接受
```

---

## 八、冷启动方案

### 8.1 Phase 0 策略

```
┌─────────────────────────────────────────────────────┐
│  Phase 0 (0-6 月): 全面免费 + 通胀保底              │
├─────────────────────────────────────────────────────┤
│                                                      │
│  群主侧:                                             │
│  ├─ 注册 Bot 免费                                    │
│  ├─ 所有功能免费使用 (Basic tier)                    │
│  ├─ 免费试用 Pro 功能 30 天                          │
│  └─ 目标: 积累 50+ 活跃 Bot                         │
│                                                      │
│  节点侧:                                             │
│  ├─ 通胀 100 NEX/天 保证节点盈利                    │
│  ├─ 官方运营 3 个种子节点                            │
│  └─ 目标: 吸引 3-5 个社区节点                        │
│                                                      │
└─────────────────────────────────────────────────────┘
```

### 8.2 Phase 转换触发条件

```
Phase 0 → Phase 1:
  条件: 活跃 Bot >= 30 且 运行天数 >= 180
  动作: 通胀减半, 开启 Pro/Enterprise 收费, Basic 保持免费

Phase 1 → Phase 2:
  条件: 付费订阅 >= 50 且 运行天数 >= 365
  动作: 通胀再减半, Basic 开始收费 (可选免费 Tier 保留)

Phase 2 → Phase 3:
  条件: 付费订阅 >= 200 且 运行天数 >= 730
  动作: 通胀归零, 纯订阅驱动
```

### 8.3 免费 Tier（永久保留选项）

```
可选: 保留一个永久免费层级 (Starter)
  限制: 1 群, 仅基础反垃圾, 无 Captcha/白名单/欢迎消息
  目的: 降低入门门槛, 靠增值服务转化付费
  节点成本: 极低 (免费用户消耗资源少)
```

---

## 九、安全与治理

### 9.1 防滥用

| 风险 | 对策 |
|---|---|
| 群主注册大量 Bot 刷免费额度 | 每 owner 限制免费 Bot 数 (MaxFreeBotsPerOwner = 1) |
| 节点 Sybil 攻击刷奖励 | 质押门槛 + 信誉门槛 + 在线率门槛 |
| 节点卡特尔垄断奖励 | MaxRewardShare (30%) 上限 |
| 订阅费定价过低/过高 | 治理投票调整 fee 参数 |
| Agent 离线但仍扣费 | SLA 检查 — Agent 离线超 1 Era 自动暂停扣费 |

### 9.2 治理可调参数

```
以下参数可通过 Runtime Upgrade 或治理提案调整:

InflationPerEra          — 通胀速率
BasicFeePerEra           — Basic 订阅费
ProFeePerEra             — Pro 订阅费
EnterpriseFeePerEra      — Enterprise 订阅费
MaxRewardShare           — 单节点奖励上限
MinUptimeForReward       — 最低在线率
EraLength                — Era 长度
```

---

## 十、竞品对比

| 维度 | Combot | Rose | Group Help | **Nexus** |
|---|---|---|---|---|
| 定价 | $10-50/月 | 免费 | $5-30/月 | **10-100 NEX/月** |
| 节点奖励 | N/A (中心化) | N/A | N/A | **订阅+通胀混合** |
| 部署 | SaaS | SaaS | SaaS | **去中心化节点** |
| 数据 | 第三方持有 | 第三方持有 | 第三方持有 | **群主自持** |
| 审查 | 可被封 | 可被封 | 可被封 | **抗审查** |
| Token-Gate | ❌ | ❌ | ❌ | **✅ 原生** |
| 链上审计 | ❌ | ❌ | ❌ | **✅ ActionLog** |

---

## 十一、实现计划

### 11.1 开发步骤

| # | 内容 | 改动范围 | 预估 |
|---|---|---|---|
| 1 | 定义 Subscription/EraRewardInfo 结构 | pallet-bot-consensus | 0.5 天 |
| 2 | 新增 6 个 Storage 项 | pallet-bot-consensus | 0.5 天 |
| 3 | 实现 subscribe/deposit/cancel/change_tier | pallet-bot-consensus | 1 天 |
| 4 | 实现 compute_node_weight | pallet-bot-consensus | 0.5 天 |
| 5 | 实现 on_era_end Hook (扣费+铸币+分配) | pallet-bot-consensus | 1.5 天 |
| 6 | 实现 claim_rewards | pallet-bot-consensus | 0.5 天 |
| 7 | Runtime 参数配置 | runtime/configs | 0.5 天 |
| 8 | 单元测试 (≥20 tests) | pallet-bot-consensus/tests | 1.5 天 |
| 9 | Web DApp 订阅管理 UI | nexus-web/ | 2 天 |
| | **总计** | | **8.5 天** |

### 11.2 与现有 pallet 的关系

```
pallet-bot-consensus (扩展):
  新增: Subscription 管理 + Era 奖励系统 + claim_rewards
  依赖: pallet-bot-registry (查 Bot owner 验证订阅权限)

pallet-bot-registry (不变):
  提供: Bot owner 查询、Bot 状态管理

pallet-bot-group-mgmt (不变):
  提供: ActionLog 审计
```

### 11.3 `pallet-bot-consensus` 变更汇总

```
新增 Storage (6):
  Subscriptions, SubscriptionEscrow, CurrentEra,
  EraRewards, NodePendingRewards, NodeTotalEarned

新增 Call (5):
  subscribe(9), deposit_subscription(10), cancel_subscription(11),
  change_tier(12), claim_rewards(13)

新增 Event:
  Subscribed, SubscriptionDeposited, SubscriptionCancelled,
  TierChanged, EraCompleted, RewardsClaimed, SubscriptionSuspended

新增 Error:
  BotNotFound, NotBotOwner, InsufficientDeposit,
  SubscriptionAlreadyExists, SubscriptionNotFound,
  NoPendingRewards, InvalidTier

新增 Config:
  EraLength, InflationPerEra, MinUptimeForReward, MaxRewardShare,
  BasicFeePerEra, ProFeePerEra, EnterpriseFeePerEra

新增 Hook:
  on_initialize → 检查是否到达 Era 边界 → on_era_end()
```

---

*文档版本: v1.0 · 2026-02-08*
*架构: 订阅费 (A) + 通胀保底 (B) 混合模型*
*适用: pallet-bot-consensus 扩展*
*前置: NEXUS_LAYERED_STORAGE_DESIGN.md (全节点同步架构)*
