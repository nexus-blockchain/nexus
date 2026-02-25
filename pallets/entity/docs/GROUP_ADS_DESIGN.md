# GroupRobot 群组广告投放 + 付费/免费方案设计

> 版本: v0.2.0 | 2026-02-24
> 广告载体: **Discord / Telegram 群组消息** (由 GroupRobot Bot 投放)

## 1. 核心架构

```
广告主 ──► pallet-grouprobot-ads (链上竞价/排期/结算)
                    │
                    ▼
           CommunityAdSchedule (链上存储)
                    │
                    ▼
           GroupRobot Bot (定时查询排期 → SendMessage 广告 → 上报收据)
                    │
                    ▼
           Discord/Telegram 群组 (Free/Basic 层级接受广告)
```

## 2. 订阅层级 (扩展现有 SubscriptionTier)

现有 `SubscriptionTier { Basic, Pro, Enterprise }`, 新增 **Free** 作为默认层级(无链上订阅记录)。

| 特性 | Free (默认) | Basic (10 NEX/Era) | Pro (30) | Enterprise (80) |
|------|-------------|---------------------|----------|-----------------|
| **群管功能** | 踢/禁/静音 | + 黑名单 + 防刷屏 | + 分类器 + 反钓鱼 | 全部 |
| **规则引擎** | 3 条 | 10 条 | 50 条 | 无限 |
| **动作日志** | 7 天 | 30 天 | 90 天 | 永久 |
| **广告** | **强制 2 条/天** | **可选 1 条/天** | **无** | **无** |
| **广告收入分成** | 60% 群主 / 25% 国库 / 15% 节点 | 70/20/10 | N/A | N/A |
| **TEE 节点** | ❌ | 可选 | 优先 | 专属 |

计费复用已有 `pallet-grouprobot-consensus` 的 Era 机制 (7天/Era, Escrow 预存, PastDue→Suspended 降级链)。

## 3. 广告投放形式

| 类型 | 说明 | Free | Basic |
|------|------|------|-------|
| **ScheduledPost** | Bot 定时发送广告 Embed/Card 到群组 | 每12h一条 | 每24h(可关) |
| **ReplyFooter** | Bot 回复命令时底部附一行广告 | ✅ | ❌ |
| **WelcomeEmbed** | 新成员欢迎消息嵌入广告 | ✅ | ❌ |

**消息格式示例 (Discord Embed):**
```
📢 赞助推广
[广告文本 ≤280字]
🔗 了解更多
────
由 Nexus 广告网络提供 | 升级 Pro 去广告
```

## 4. 核心数据结构

```rust
// ── pallet-grouprobot-primitives 新增 ──
pub enum AdDeliveryType { ScheduledPost, ReplyFooter, WelcomeEmbed }
pub enum AdTargetTag { Platform(Platform), MinMembers(u32), Language([u8; 2]), All }

/// 双向偏好控制
pub enum AdPreference {
    Allow,              // 默认: 允许
    Blocked,            // 拉黑
    Preferred,          // 指定/白名单 (优先匹配)
}
pub enum CampaignStatus { Active, Paused, Exhausted, Expired, Cancelled }
pub enum AdReviewStatus { Pending, Approved, Rejected, Flagged }

// ── pallet-grouprobot-ads ──
pub struct AdCampaign<T: Config> {
    pub advertiser: T::AccountId,
    pub text: BoundedVec<u8, T::MaxAdTextLength>,       // ≤280
    pub url: BoundedVec<u8, T::MaxAdUrlLength>,          // ≤256
    pub image_cid: Option<BoundedVec<u8, T::MaxCidLength>>,
    pub cta_text: Option<BoundedVec<u8, ConstU32<32>>>,
    pub bid_per_mille: BalanceOf<T>,                      // 每千人触达出价 (CPM)
    pub daily_budget: BalanceOf<T>,
    pub total_budget: BalanceOf<T>,
    pub spent: BalanceOf<T>,
    pub target: AdTargetTag,
    pub delivery_types: u8,                               // bitmask
    pub status: CampaignStatus,
    pub review_status: AdReviewStatus,
    pub total_deliveries: u64,
    pub created_at: BlockNumberFor<T>,
    pub expires_at: BlockNumberFor<T>,
}

/// 社区广告排期 (每 Era 由 on_era_end 更新)
pub struct CommunityAdSchedule {
    pub community_id_hash: CommunityIdHash,
    pub scheduled_campaigns: BoundedVec<u64, ConstU32<10>>,
    pub daily_limit: u8,
    pub delivered_this_era: u32,
}

/// Bot 上报的投放收据
pub struct DeliveryReceipt<T: Config> {
    pub campaign_id: u64,
    pub community_id_hash: CommunityIdHash,
    pub delivery_type: AdDeliveryType,
    pub audience_size: u32,
    pub node_signature: [u8; 64],
    pub delivered_at: BlockNumberFor<T>,
}
```

## 5. pallet-grouprobot-ads 设计

**Storage:**

| 存储项 | 说明 |
|--------|------|
| `Campaigns<u64 → AdCampaign>` | 广告活动 |
| `CampaignEscrow<u64 → Balance>` | 锁定预算 |
| `CommunitySchedules<Hash → Schedule>` | 社区排期 |
| `DeliveryReceipts<Hash → BoundedVec<Receipt>>` | 投放收据 |
| `EraAdRevenue<Hash → Balance>` | 每Era社区收入 |
| `CommunityTotalRevenue<Hash → Balance>` | 累计收入 |
| `CampaignBids<Tag → BoundedVec<(id, bid)>>` | 竞价表 |
| `AdvertiserBlacklist<AccountId → BoundedVec<Hash>>` | 广告主拉黑的社区 |
| `AdvertiserWhitelist<AccountId → BoundedVec<Hash>>` | 广告主指定的社区 |
| `CommunityBlacklist<Hash → BoundedVec<AccountId>>` | 社区拉黑的广告主 |
| `CommunityWhitelist<Hash → BoundedVec<AccountId>>` | 社区指定的广告主 |

**Extrinsics (10 个):**

| # | 函数 | 权限 | 说明 |
|---|------|------|------|
| 0 | `create_campaign(...)` | signed | 创建活动, 锁预算 |
| 1 | `fund_campaign(id, amount)` | 广告主 | 追加预算 |
| 2 | `pause_campaign(id)` | 广告主 | 暂停 |
| 3 | `cancel_campaign(id)` | 广告主 | 取消, 退还 |
| 4 | `review_campaign(id, approved)` | Root/DAO | 审核内容 |
| 5 | `submit_delivery_receipt(...)` | Bot节点 | 上报投放 |
| 6 | `settle_era_ads()` | any | 触发Era结算 |
| 7 | `flag_campaign(id, reason)` | any | 举报 |
| 8 | `claim_ad_revenue(hash)` | 群主 | 提取收入 |
| 9 | `update_ad_prefs(hash, opt_out)` | 群主(Basic) | 关闭广告类型 |
| 10 | `advertiser_block_community(hash)` | 广告主 | 拉黑社区 |
| 11 | `advertiser_unblock_community(hash)` | 广告主 | 取消拉黑 |
| 12 | `advertiser_prefer_community(hash)` | 广告主 | 指定社区 |
| 13 | `community_block_advertiser(hash, who)` | 群主 | 拉黑广告主 |
| 14 | `community_unblock_advertiser(hash, who)` | 群主 | 取消拉黑 |
| 15 | `community_prefer_advertiser(hash, who)` | 群主 | 指定广告主 |

## 6. 双向偏好控制 (广告主 ⇄ 群组)

### 6.1 四种场景

| 操作 | 谁发起 | 效果 | 用例 |
|------|----------|------|------|
| **广告主拉黑群** | 广告主 | 该群永不展示此广告 | 群内容与品牌不符、历史欺诈 |
| **广告主指定群** | 广告主 | 优先匹配该群 (KOL合作) | 品牌赞助、精准投放 |
| **群组拉黑广告主** | 群主 | 该广告主永不进此群 | 竞品广告、用户投诉 |
| **群组指定广告主** | 群主 | 优先接收该广告主 (Basic可关广告但保留指定) | 长期合作 |

### 6.2 分配逻辑 (allocate_ads 更新)

```
allocate_ads(community_hash, campaigns):
  1. 过滤: 移除 AdvertiserBlacklist[广告主] 包含此社区的 campaign
  2. 过滤: 移除 CommunityBlacklist[社区] 包含此广告主的 campaign
  3. 优先: 双向指定 (广告主指定此群 AND 群指定此广告主) → 最高优先级
  4. 优先: 单向指定 (任一方指定) → 次高优先级
  5. 正常: 剩余 campaign 按 bid_per_mille 竞价排序
  6. 取 top-N 填入 CommunityAdSchedule

指定关系不免除竞价 — 仍然按 CPM 计费, 只是匹配优先级更高。
```

### 6.3 串通风险分析

```
场景: 广告主 A 和群主 B 互相指定, 自买自卖

广告主付: 1 NEX
平台抽成: 25-40% (0.25-0.40 NEX)
群主收: 0.60-0.70 NEX
净亏损: 0.30-0.40 NEX/每次

→ 自买自卖 = 向平台送钱, 经济上无利可图 ✅
→ 除非有链外返利, 但那属于正常商业行为 (KOL 合作)
```

### 6.4 限制条件

| 约束 | 说明 |
|------|------|
| 指定上限 | 每个广告主最多指定 20 个社区, 每个社区最多指定 10 个广告主 |
| 拉黑上限 | 各 50 个 (BoundedVec) |
| Free 层级 | 可拉黑不可指定 (避免 Free 群主用指定来过滤正常广告) |
| 指定不免费 | 指定关系只影响匹配优先级, 不影响计费 |

## 7. 群规模公平机制 (CPM 加权)

**问题**: 5000 人大群和 50 人小群投放一条广告，触达人数差 100 倍，固定单价不公平。

**解法**: 按 **CPM (Cost Per Mille, 千人触达成本)** 计费，实际扣费随群人数线性缩放。

```
实际扣费 = bid_per_mille × (audience_size / 1000)

示例 (广告主出价 0.5 NEX/千人):
┌──────────┬──────────────┬──────────┬──────────────┐
│ 群规模   │ audience_size│ 扣费     │ 群主收入(60%)│
├──────────┼──────────────┼──────────┼──────────────┤
│ 大群     │ 5,000        │ 2.5 NEX  │ 1.50 NEX     │
│ 中群     │ 500          │ 0.25 NEX │ 0.15 NEX     │
│ 小群     │ 50           │ 0.025NEX │ 0.015 NEX    │
└──────────┴──────────────┴──────────┴──────────────┘

公平性:
  ✅ 广告主: 每触达一个人的成本恒定, 大群小群 ROI 一致
  ✅ 大群主: 人多收入高, 激励发展社区
  ✅ 小群主: 也能获得按比例收入, 不被排斥
```

**最低门槛**: `audience_size ≥ 20` 才能接入广告

**❗ audience_size 定义 (防死号核心)**:

`audience_size` **不等于**群总人数，而是 **Bot 统计的活跃成员数**，定义为：

```
audience_size = 过去 7 天内发过至少 1 条消息的成员数
             × 新成员过滤 (加群 < 48h 的不计)

计算方: GroupRobot Bot (Off-chain)
存储方: Bot 本地 DashMap 跟踪 per-user 最后发言时间
上报方: 写入 DeliveryReceipt.audience_size, 链上结算以此为准
```

这意味着拉入 1000 个死号但从不发言 → audience_size 不变 → 广告费不变 → 攻击无效。

## 7. 竞价与分配

```
第二价格拍卖 (Vickrey) + CPM:
1. 广告主出价 bid_per_mille (每千人触达的出价)
2. on_era_end() → allocate_ads():
   - 遍历 ads_enabled 社区
   - 匹配 campaign.target ↔ 社区属性
   - 按 bid_per_mille 排序, 分配 top-N
   - 中标价 = 第 N+1 名出价 (Vickrey)
3. 写入 CommunityAdSchedule → Bot 查询执行
4. 结算时: 实际扣费 = vickrey_price × (audience_size / 1000)
```

## 8. 结算分成 (每 Era)

```
settle_era_ads():
  for receipt in DeliveryReceipts:
    cost = vickrey_price × (receipt.audience_size / 1000)
    扣除 CampaignEscrow
    ↓
    分配:
      Free:  60% 群主 + 25% 国库 + 15% 执行节点
      Basic: 70% 群主 + 20% 国库 + 10% 执行节点
```

**经济飞轮:**
- 广告主用 NEX 投放 → 群主赚 NEX → 可用于升级订阅
- 订阅费 → 国库 → 回购 NEX → 减少流通
- 节点收入 → 激励更多节点运行 Bot

## 9. Bot 侧实现 (Off-chain)

```rust
// grouprobot/src/processing/rules/ad_delivery.rs (新增)

// 1. 后台定时器 (独立 tokio::spawn)
async fn ad_delivery_loop(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        // 查询链上 CommunityAdSchedule
        // 检查是否到投放窗口
        // 构造 Embed/Card 消息
        // PlatformExecutor::execute(SendMessage)
        // 成功 → submit_delivery_receipt() 上链
    }
}

// 2. 回复附带广告 (规则引擎新增 AdFooterRule)
// 在 DefaultRule 前拦截, 检查 ads_enabled
// 追加广告 footer 到 action.message
```

## 10. CommunityConfig 扩展

```rust
pub struct CommunityConfig {
    // ... 现有字段 ...
    pub ads_enabled: bool,           // Free/Basic 自动开启
    pub ad_tags: BoundedVec<AdTargetTag, ConstU32<5>>,
    pub active_members: u32,         // Bot 定期更新
    pub language: [u8; 2],           // 广告定向用
}
```

## 11. 反作弊 (重点: 死号灌水攻击)

### 11.1 攻击模型

```
模型 A (死号): 拉入大量僵尸号 → 群总人数虚高 → 骗取 CPM

模型 B (脚本活跃): 管理员用脚本控制僵尸号发"正常消息" → 
  绕过活跃度过滤 → audience_size 虚高 → 骗取 CPM

模型 B 更危险 — 纯消息过滤是猫鼠博弈, 管理员总能找到绕过方式。
需要从经济机制层面让作恶无利可图。
```

### 11.2 核心防御: audience_size 上限锚定 + 质押

**关键洞察**: 不能仅靠 Bot 统计来定 audience_size，需要**链上硬约束**。

```
防御核心: 
  audience_size = min(Bot统计活跃数, 链上注册上限)

链上注册上限 = f(群主质押额)
  质押 10 NEX → 上限 200 人
  质押 50 NEX → 上限 1,000 人
  质押 200 NEX → 上限 5,000 人

群主想要更高的 audience_size 上限 → 必须质押更多 NEX
作弊被发现 → 扣除质押 (Slash)
→ 作恶成本 = 质押金额, 收益 = 虚增的广告分成
→ 只要 Slash 比例够高, 作恶就不划算
```

### 11.3 多层防御总览

| 层级 | 措施 | 防御目标 | 实现位置 |
|------|------|----------|----------|
| **E1 质押上限** | audience_size ≤ f(stake), 超出截断 | 限制作恶天花板 | **Pallet (链上)** |
| **E2 Slash 机制** | 举报成功 → 扣 30% 质押 | 经济威慑 | **Pallet (链上)** |
| **L1 活跃度过滤** | 7天内发过言的人数 | 过滤死号 | Bot (LocalStore) |
| **L2 新成员冷却** | 加群 < 48h 不计入 | 批量拉号延迟 | Bot (join timestamp) |
| **L3 异常检测** | audience 突增 > 100%/Era 暂停广告 | 异常自动熔断 | Pallet (on_era_end) |
| **L4 发言质量** | 重复/纯表情/< 3字不计 | 低质量绕过 | Bot (rule check) |
| **L5 多节点交叉** | 多节点独立统计, 偏差 > 20% 拒结 | 节点串通 | Pallet (settle) |
| **L6 互动指纹** | 消息回复关系/对话图谱分析 | 脚本单向发言 | Bot (高级) |

**E1 + E2 是经济层防御（根本解），L1-L6 是技术层防御（辅助）。**

### 11.4 质押与上限机制

```rust
// pallet-grouprobot-ads 新增 Storage:
CommunityAdStake<T>: StorageMap<CommunityIdHash, BalanceOf<T>>  // 群主质押
CommunityAudienceCap<T>: StorageMap<CommunityIdHash, u32>       // 上限

// 新增 Extrinsic:
fn stake_for_ads(community_hash, amount) {
    // 质押 NEX, 计算上限
    let cap = Self::compute_audience_cap(amount);
    CommunityAdStake::insert(hash, amount);
    CommunityAudienceCap::insert(hash, cap);
}

fn compute_audience_cap(stake: Balance) -> u32 {
    // 阶梯函数 (边际递减, 防巨鲸垄断):
    //   0-10 NEX:   20 人/NEX  → 质押 10 = 上限 200
    //   10-50 NEX:  20 人/NEX  → 质押 50 = 上限 1,000
    //   50-200 NEX: ~27 人/NEX → 质押 200 = 上限 5,000
    //   200+ NEX:   递减 ...   → 质押 500 = 上限 10,000
}

// 结算时:
fn effective_audience(receipt: &DeliveryReceipt) -> u32 {
    let cap = CommunityAudienceCap::get(&receipt.community_id_hash).unwrap_or(0);
    let bot_reported = receipt.audience_size;
    min(bot_reported, cap)  // 取较小值
}
```

### 11.5 Slash + 举报

```
举报流程:
  1. 广告主或任何人: flag_community(community_hash, evidence)
  2. DAO 投票或 Root 裁决
  3. 如成功:
     - 扣除 30% 质押 → 50% 给举报者, 50% 国库
     - audience_cap 降为当前值的 50%, 持续 2 个 Era
     - 连续 3 次 → 永久禁止接入广告

经济分析 (群主视角):
  质押 50 NEX, 上限 1000 人
  每 Era 广告收入 ≈ 0.5 × 1000/1000 × 14次 × 60% ≈ 4.2 NEX
  被 Slash 一次 → 损失 15 NEX
  → 需要 3.5 个 Era (约 25 天) 才能回本
  → 且上限被砍半, 后续收入减半
  → 作恶预期收益为负 ✅
```

### 11.6 Bot 侧活跃成员统计

```rust
// grouprobot/src/infra/audience_tracker.rs

/// 每次收到群消息时更新
fn on_message(group_id: &str, sender_id: &str, text: &str, joined_at: u64) {
    let now = unix_timestamp();

    // L2: 新成员 48h 冷却
    if now - joined_at < 48 * 3600 { return; }

    // L4: 发言质量过滤 (纯表情/太短/重复)
    if text.len() < 3 || is_emoji_only(text) || is_duplicate(group_id, sender_id, text) {
        return;
    }

    // L6: 互动指纹 (需要有回复/引用/@ 等互动行为, 纯单向发言权重降低)
    let weight = if has_interaction(group_id, sender_id) { 1.0 } else { 0.3 };
    active_members.insert((group_id, sender_id), (now, weight));
}

/// 投放广告时计算 audience_size (加权)
fn compute_audience_size(group_id: &str) -> u32 {
    let cutoff = unix_timestamp() - 7 * 86400;
    let weighted_sum: f64 = active_members.iter()
        .filter(|((gid, _), (ts, _))| gid == group_id && *ts >= cutoff)
        .map(|(_, (_, w))| w)
        .sum();
    weighted_sum.ceil() as u32
    // 注意: 最终还会被链上 audience_cap 截断
}
```

### 11.7 攻击无效性分析

```
场景 A: 拉入 1000 个死号 (不发言)
→ L1: 不计入活跃 → audience_size 不变 → 无效 ✅

场景 B: 1000 个号脚本发言
→ L4: 重复/低质量过滤
→ L6: 无互动 → 权重 0.3 → 1000×0.3=300 虚增
→ E1: 但 audience_cap = min(cap, reported) → 群主没多质押就被截断
→ 如果群主多质押来提高上限 → 被举报 Slash 损失更大
→ L3: 突增 > 100% 自动暂停 → 连广告都停了
→ 结论: 经济上不可行 ✅

场景 C: 群主质押 200 NEX, 买 5000 上限, 实际只有 500 真人
→ Bot 统计 audience_size = 500, cap = 5000 → 取 min = 500
→ 多余质押不产生额外收益, 只是浪费资金
→ 质押只是"保险金", 不能直接变现 ✅

根本博弈:
  作恶成本 = Slash 风险 (30%质押) + 脚本运行成本 + 被永封风险
  作恶收益 = 虚增 audience × bid_per_mille × 60% 分成
  只要 Slash 比例足够 → 作恶期望收益为负
```

### 11.8 其他风险

| 风险 | 对策 |
|------|------|
| Bot 虚报投放 | 节点签名验证 + 多节点交叉对比 |
| 广告主投诉 | 举报 → DAO 仲裁 → 扣诚信分 |
| 不良广告内容 | review_campaign 审核 + flag 举报 |
| 群主与节点串通 | L5 多节点独立统计, TEE 节点不可篡改 |
| 群主多账号互动 | L6 互动图谱: 固定几人互相回复→异常模式检测 |

## 12. 实施路径

| 阶段 | 内容 | 优先级 |
|------|------|--------|
| **P1** | primitives 新增类型 + CommunityConfig 扩展 ads 字段 | 🔴 高 |
| **P2** | `pallet-grouprobot-ads`: Campaign CRUD + 竞价 + 结算 | 🔴 高 |
| **P3** | Bot 侧: ad_delivery_loop + AdFooterRule | 🟡 中 |
| **P4** | consensus 扩展: Free 层级 + 功能门控 | 🟡 中 |
| **P5** | 反作弊 + 多节点交叉验证 | 🟢 低 |
