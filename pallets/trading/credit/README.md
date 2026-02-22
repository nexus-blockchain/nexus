# Pallet Credit（统一信用管理系统）

## 📋 模块概述

`pallet-credit` 是 Nexus 区块链的 **统一信用管理系统**，整合了买家信用（Buyer Credit）和做市商信用（Maker Credit）两个子系统，并提供买家额度管理（Buyer Quota）功能。该模块通过多维度信任评估、动态风控机制和信用等级管理，为 P2P 交易市场提供完善的信用风控体系。

### 核心特性

- ✅ **买家信用管理**：5维度信任评估、新用户分层冷启动、信用等级体系
- ✅ **做市商信用管理**：800-1000分评分体系、五等级制度、履约率追踪
- ✅ **买家额度管理**：渐进式额度计算、首购限制、并发订单数量控制（方案C+）
- ✅ **快速学习机制**：前3笔订单5倍权重加速信用积累
- ✅ **社交信任网络**：推荐关系、背书机制、连带责任
- ✅ **违约惩罚机制**：连续违约指数级惩罚、7天3次自动封禁
- ✅ **动态保证金**：信用高的做市商保证金减50%
- ✅ **自动降级/禁用**：< 750分自动暂停服务
- ✅ **信用恢复机制**：30天无违约恢复10分、连续10单奖励5分

### 技术栈

- **Polkadot SDK**: stable2506
- **Rust Edition**: 2021
- **依赖**：frame-support, frame-system, pallet-timestamp

---

## 🔑 核心功能

### 1. 买家信用管理（Buyer Credit）

#### 1.1 多维度信任评估（5个维度）

买家初始风险分通过5个维度加权计算，每个维度0-100分：

**1) 资产信任（25%权重）**

```rust
pub fn calculate_asset_trust(account: &T::AccountId) -> u8 {
    // NEX 余额信任分
    let balance_score = if balance_multiplier >= 10000 {
        50  // >= 10000倍最小余额：高信任
    } else if balance_multiplier >= 1000 {
        30  // >= 1000倍：中等信任
    } else if balance_multiplier >= 100 {
        15  // >= 100倍：基础信任
    } else {
        0
    };

    // 预留余额额外加分（最多20分）
    let reserved_score = min(20, reserved / min_balance / 100);

    balance_score + reserved_score
}
```

**2) 账户年龄信任（20%权重）**

```rust
pub fn calculate_age_trust(account: &T::AccountId) -> u8 {
    match age_days {
        >= 180 => 100,  // 半年以上：完全信任
        >= 90  => 80,   // 3个月：高度信任
        >= 30  => 50,   // 1个月：中等信任
        >= 7   => 25,   // 1周：基础信任
        _      => 0,    // 新账户：无信任
    }
}
```

**3) 活跃度信任（20%权重）**

基于转账次数计算，每次转账+2分，上限40分。

**4) 社交信任（20%权重）**

```rust
pub fn calculate_social_trust(account: &T::AccountId) -> u8 {
    let mut score = 0u8;

    // 邀请人信誉（最多40分）
    if let Some(referrer) = BuyerReferrer::<T>::get(account) {
        score += match referrer_risk_score {
            0..=200   => 40,
            201..=400 => 25,
            401..=600 => 10,
            _         => 0,
        };
    }

    // 被推荐次数（最多30分）
    let endorsements = BuyerEndorsements::<T>::get(account);
    score += min(30, active_endorsements * 10);

    min(100, score)
}
```

**5) 身份信任（15%权重）**

预留扩展，用于身份验证、KYC等。

**综合风险分计算**：

```rust
pub fn calculate_new_user_risk_score(account: &T::AccountId) -> u16 {
    // 加权计算综合信任分（0-100）
    let weighted_trust = (
        asset_trust as u16 * 25 +      // 资产权重 25%
        age_trust as u16 * 20 +        // 年龄权重 20%
        activity_trust as u16 * 20 +   // 活跃度权重 20%
        social_trust as u16 * 20 +     // 社交权重 20%
        identity_trust as u16 * 15     // 身份权重 15%
    ) / 100;

    // 风险分 = 1000 - 综合信任分 × 10
    1000u16.saturating_sub(weighted_trust * 10)
}
```

#### 1.2 新用户分层冷启动

**4个等级**（基于初始风险分）：

```rust
pub enum NewUserTier {
    Premium,    // 优质新用户（风险分0-300）
    Standard,   // 标准新用户（风险分301-500）
    Basic,      // 基础新用户（风险分501-700）
    Restricted, // 受限新用户（风险分701-1000）
}
```

**限额配置**：

| 等级 | 风险分范围 | 单笔限额 | 日限额 | 冷却期 |
|------|-----------|---------|--------|--------|
| Premium | 0-300 | $5,000 | $20,000 | 0小时 |
| Standard | 301-500 | $1,000 | $5,000 | 12小时 |
| Basic | 501-700 | $500 | $2,000 | 24小时 |
| Restricted | 701-1000 | $100 | $500 | 48小时 |

**首笔订单10%折扣**：

```rust
let effective_single_limit = if credit.completed_orders == 0 {
    let discounted = single_limit / 10;
    core::cmp::max(discounted, 10)  // 最低10美元
} else {
    single_limit
};
```

#### 1.3 信用等级体系

**5个等级**（基于完成订单数）：

```rust
pub enum CreditLevel {
    Newbie,   // 新手（0-5笔）
    Bronze,   // 铜牌（6-20笔）
    Silver,   // 银牌（21-50笔）
    Gold,     // 金牌（51-100笔）
    Diamond,  // 钻石（101+笔）
}
```

**基础限额配置**：

| 等级 | 完成订单数 | 基础单笔限额 | 基础日限额 |
|------|-----------|------------|-----------|
| Newbie | 0-5 | $100 | $500 |
| Bronze | 6-20 | $500 | $2,000 |
| Silver | 21-50 | $2,000 | $10,000 |
| Gold | 51-100 | $10,000 | $50,000 |
| Diamond | 100+ | $50,000 | 无限制 |

**限额选择逻辑**：

- **前20笔**：使用新用户等级限额
- **20笔后**：切换到信用等级限额

#### 1.4 快速学习机制

**订单权重系数**（加速信用积累）：

```rust
pub fn get_order_weight(order_index: u32) -> u8 {
    match order_index {
        1..=3   => 50,  // 前3笔：权重 5.0x
        4..=5   => 30,  // 第4-5笔：权重 3.0x
        6..=10  => 20,  // 第6-10笔：权重 2.0x
        11..=20 => 15,  // 第11-20笔：权重 1.5x
        _       => 10,  // 21笔以上：权重 1.0x
    }
}
```

**应用示例**：

```rust
pub fn update_credit_on_success(buyer: &T::AccountId, amount_usdt: u64, payment_time_seconds: u64) {
    // 基础加分
    let base_score = 10u16;

    // 快速付款奖励
    let speed_bonus = if payment_time_seconds < 300 {
        10  // < 5分钟：+10分
    } else if payment_time_seconds < 600 {
        5   // < 10分钟：+5分
    } else {
        0
    };

    // 大额交易奖励
    let amount_bonus = if amount_usdt > 1000 {
        5
    } else {
        0
    };

    // 应用权重系数
    let weight = get_order_weight(order_index);
    let weighted_score = (base_score + speed_bonus + amount_bonus) * (weight as u16) / 10;

    // 降低风险分
    credit.risk_score = credit.risk_score.saturating_sub(weighted_score);
}
```

#### 1.5 违约惩罚机制

**连续违约指数级惩罚**：

```rust
pub fn penalize_default(buyer: &T::AccountId) {
    let consecutive_defaults = count_recent_defaults(buyer, 7);

    // 基础惩罚（根据等级）
    let base_penalty = match credit.level {
        CreditLevel::Newbie  => 50,
        CreditLevel::Bronze  => 30,
        CreditLevel::Silver  => 20,
        CreditLevel::Gold    => 10,
        CreditLevel::Diamond => 5,
    };

    // 连续违约指数级惩罚
    let multiplier = match consecutive_defaults {
        1 => 1,   // 首次违约：1×
        2 => 2,   // 第2次：2×
        3 => 4,   // 第3次：4×
        4 => 8,   // 第4次：8×
        _ => 16,  // 5+次：16×
    };

    let penalty = base_penalty.saturating_mul(multiplier);
    credit.risk_score = credit.risk_score.saturating_add(penalty);

    // 7天内连续违约 >= 3次，直接封禁
    if consecutive_defaults >= 3 {
        credit.risk_score = 1000;  // 最高风险，禁止交易
        Self::deposit_event(Event::UserBanned { account, reason });
    }
}
```

**违约冷却期**：

```rust
fn calculate_cooldown_period(buyer: &T::AccountId) -> BlockNumberFor<T> {
    let recent_defaults = count_recent_defaults(buyer, 30);

    let cooldown_days: u32 = match recent_defaults {
        0 => 0,
        1 => 1,   // 首次违约：1天
        2 => 3,   // 第2次：3天
        3 => 7,   // 第3次：7天
        4 => 14,  // 第4次：14天
        _ => 30,  // 5+次：30天
    };

    T::BlocksPerDay::get().saturating_mul(cooldown_days.into())
}
```

#### 1.6 风险分自然衰减

**每30天衰减50分**（仅限违约用户）：

```rust
fn calculate_risk_decay(buyer: &T::AccountId) -> u16 {
    let credit = BuyerCredits::<T>::get(buyer);

    if credit.default_count == 0 {
        return 0;  // 无违约用户不衰减
    }

    let blocks_since_last_default = current_block.saturating_sub(last_default_block);
    let blocks_per_30_days = T::BlocksPerDay::get().saturating_mul(30u32.into());

    let decay_cycles: u32 = (blocks_since_last_default / blocks_per_30_days).saturated_into();

    // 每30天衰减50分，但不低于初始风险分
    (decay_cycles as u16).saturating_mul(50)
}
```

#### 1.7 行为模式识别

**每5笔分析一次行为模式**：

```rust
fn analyze_and_adjust_behavior(account: &T::AccountId) {
    let history = BuyerOrderHistory::<T>::get(account);

    // 检查付款速度
    let avg_payment_time: u64 = history.iter()
        .map(|o| o.payment_time_seconds)
        .sum::<u64>() / history.len() as u64;
    let fast_payment = avg_payment_time < 600;  // < 10分钟

    // 检查金额稳定性
    let amounts: Vec<_> = history.iter().map(|o| o.amount_usdt).collect();
    let max_amount = *amounts.iter().max().unwrap_or(&0);
    let min_amount = *amounts.iter().min().unwrap_or(&1);
    let is_consistent = max_amount / min_amount < 3;  // 波动 < 3倍

    // 综合判断
    let (pattern, adjustment) = match (fast_payment, is_consistent) {
        (true, true)   => (BehaviorPattern::HighQuality, -200i16),  // 高质量：-200分
        (true, false)
        | (false, true) => (BehaviorPattern::Good, -100i16),         // 良好：-100分
        (false, false) => (BehaviorPattern::Normal, 0i16),           // 普通：0分
    };

    // 应用调整
    credit.risk_score = credit.risk_score.saturating_sub(adjustment.abs() as u16);
}
```

#### 1.8 社交信任网络

**推荐关系（Endorsement）**：

```rust
pub fn endorse_user(origin: OriginFor<T>, endorsee: T::AccountId) -> DispatchResult {
    let endorser = ensure_signed(origin)?;

    // 检查推荐人信用（风险分 <= 300）
    let endorser_credit = BuyerCredits::<T>::get(&endorser);
    ensure!(
        endorser_credit.risk_score <= 300,
        Error::<T>::InsufficientCreditToEndorse
    );

    // 添加推荐记录
    let endorsement = Endorsement {
        endorser: endorser.clone(),
        endorsed_at: <frame_system::Pallet<T>>::block_number(),
        is_active: true,
    };

    BuyerEndorsements::<T>::insert(&endorsee, endorsement);
    Ok(())
}
```

**连带责任**（被推荐人违约，推荐人受罚）：

```rust
// 使所有推荐失效
BuyerEndorsements::<T>::mutate(buyer, |endorsements| {
    for endorsement in endorsements.iter_mut() {
        endorsement.is_active = false;

        // 推荐人连带责任：风险分+50
        BuyerCredits::<T>::mutate(&endorsement.endorser, |endorser_credit| {
            endorser_credit.risk_score = endorser_credit.risk_score.saturating_add(50);
        });
    }
});
```

**邀请人关系（Referrer）**（仅能设置一次）：

```rust
pub fn set_referrer(origin: OriginFor<T>, referrer: T::AccountId) -> DispatchResult {
    let invitee = ensure_signed(origin)?;

    // 检查是否已设置
    ensure!(
        !BuyerReferrer::<T>::contains_key(&invitee),
        Error::<T>::ReferrerAlreadySet
    );

    BuyerReferrer::<T>::insert(&invitee, &referrer);
    Ok(())
}
```

---

### 2. 做市商信用管理（Maker Credit）

#### 2.1 信用评分体系

**评分范围**：800-1000分

**五个等级**：

```rust
pub enum CreditLevel {
    Diamond,  // 钻石（950-1000分）
    Platinum, // 白金（900-949分）
    Gold,     // 黄金（850-899分）
    Silver,   // 白银（820-849分）
    Bronze,   // 青铜（800-819分）
}
```

**等级配置**：

| 等级 | 分数范围 | 保证金倍数 | 服务状态 |
|------|---------|-----------|---------|
| 钻石 (Diamond) | 950-1000 | 0.5× | 活跃 |
| 白金 (Platinum) | 900-949 | 0.7× | 活跃 |
| 黄金 (Gold) | 850-899 | 0.8× | 活跃 |
| 白银 (Silver) | 820-849 | 0.9× | 活跃 |
| 青铜 (Bronze) | 800-819 | 1.0× | 活跃 |
| **警告** | 750-799 | 1.2× | 警告 |
| **暂停** | < 750 | 2.0× | 暂停 |

#### 2.2 信用分调整规则

**加分项**：

```rust
// 订单按时完成：+2分
record.credit_score = record.credit_score.saturating_add(2);

// 及时释放（< 24小时）：额外+1分
if response_time_seconds < 86400 {
    record.timely_release_orders += 1;
    bonus = bonus.saturating_add(1);
}

// 买家评价加分
let score_change = match stars {
    5 => 5i16,   // 5星：+5分
    4 => 2i16,   // 4星：+2分
    3 => 0i16,   // 3星：0分
    1 | 2 => -5i16, // 1-2星：-5分
    _ => 0i16,
};
```

**扣分项**：

```rust
// 订单超时：-10分
let penalty: u16 = T::MakerOrderTimeoutPenalty::get();  // 默认10
record.credit_score = record.credit_score.saturating_sub(penalty);

// 争议败诉：-20分
let penalty: u16 = T::MakerDisputeLossPenalty::get();  // 默认20
record.credit_score = record.credit_score.saturating_sub(penalty);
```

#### 2.3 履约率追踪

**关键指标**：

```rust
pub struct CreditRecord<BlockNumber> {
    // 履约数据
    pub total_orders: u32,               // 总订单数
    pub completed_orders: u32,           // 完成订单数
    pub timeout_orders: u32,             // 超时订单数
    pub cancelled_orders: u32,           // 取消订单数
    pub timely_release_orders: u32,      // 及时释放订单数（< 24h）

    // 服务质量
    pub rating_sum: u32,                 // 买家评分总和
    pub rating_count: u32,               // 评分次数
    pub avg_response_time: u32,          // 平均响应时间（秒）

    // 违约记录
    pub default_count: u16,              // 违约次数
    pub dispute_loss_count: u16,         // 争议败诉次数
    pub last_default_block: Option<BlockNumber>,
}
```

**履约率计算**：

```rust
// 完成率
let completion_rate = completed_orders * 100 / total_orders;

// 及时释放率
let timely_rate = timely_release_orders * 100 / completed_orders;

// 超时率
let timeout_rate = timeout_orders * 100 / total_orders;

// 争议败诉率
let dispute_loss_rate = dispute_loss_count * 100 / total_orders;
```

#### 2.4 动态保证金机制

**基础保证金**：1,000,000 NEX

**根据信用等级调整**：

```rust
pub fn calculate_required_deposit(maker_id: u64) -> BalanceOf<T> {
    let base_deposit = 1_000_000 * 1e18;  // 1,000,000 NEX

    let credit_score = Self::query_maker_credit_score(maker_id).unwrap_or(820);

    let multiplier_percent = match credit_score {
        950..=1000 => 50,   // Diamond: 0.5× = 500,000 NEX
        900..=949  => 70,   // Platinum: 0.7× = 700,000 NEX
        850..=899  => 80,   // Gold: 0.8× = 800,000 NEX
        820..=849  => 90,   // Silver: 0.9× = 900,000 NEX
        800..=819  => 100,  // Bronze: 1.0× = 1,000,000 NEX
        750..=799  => 120,  // Warning: 1.2× = 1,200,000 NEX
        _          => 200,  // Suspended: 2.0× = 2,000,000 NEX
    };

    base_deposit * multiplier_percent / 100
}
```

#### 2.5 服务状态自动切换

**状态定义**：

```rust
pub enum ServiceStatus {
    Active,    // 正常服务（>= 800分）
    Warning,   // 警告状态（750-799分）
    Suspended, // 暂停服务（< 750分）
}
```

**自动切换逻辑**：

```rust
fn update_maker_level_and_status(record: &mut CreditRecord<BlockNumberFor<T>>) {
    // 更新信用等级
    record.level = match record.credit_score {
        950..=1000 => CreditLevel::Diamond,
        900..=949  => CreditLevel::Platinum,
        850..=899  => CreditLevel::Gold,
        820..=849  => CreditLevel::Silver,
        _          => CreditLevel::Bronze,
    };

    // 更新服务状态
    record.status = match record.credit_score {
        0..=749   => ServiceStatus::Suspended,  // < 750：暂停
        750..=799 => ServiceStatus::Warning,    // 750-799：警告
        _         => ServiceStatus::Active,     // >= 800：正常
    };
}
```

#### 2.6 买家评价系统

**评价记录**：

```rust
pub struct Rating<AccountId> {
    pub buyer: AccountId,
    pub stars: u8,                           // 评分（1-5星）
    pub tags_codes: BoundedVec<u8, 5>,      // 标签代码（最多5个）
    pub rated_at: u32,                       // 评价时间
}

// 评价标签
pub enum RatingTag {
    FastRelease,       // 0: 快速释放
    GoodCommunication, // 1: 沟通良好
    FairPrice,         // 2: 价格合理
    SlowRelease,       // 3: 释放慢
    PoorCommunication, // 4: 沟通差
    Unresponsive,      // 5: 不回应
}
```

**评价接口**：

```rust
pub fn rate_maker(
    origin: OriginFor<T>,
    maker_id: u64,
    order_id: u64,
    stars: u8,
    tags_codes: BoundedVec<u8, ConstU32<5>>,
) -> DispatchResult {
    let buyer = ensure_signed(origin)?;

    // 验证评分范围（1-5星）
    ensure!(stars >= 1 && stars <= 5, Error::<T>::InvalidRating);

    // 检查是否已评价
    ensure!(
        !MakerRatings::<T>::contains_key(maker_id, order_id),
        Error::<T>::AlreadyRated
    );

    // 存储评价记录
    let rating = Rating {
        buyer: buyer.clone(),
        stars,
        tags_codes,
        rated_at: current_block_u32,
    };
    MakerRatings::<T>::insert(maker_id, order_id, rating);

    // 更新信用分
    let score_change = match stars {
        5 => 5i16,
        4 => 2i16,
        3 => 0i16,
        1 | 2 => -5i16,
        _ => 0i16,
    };

    Self::update_maker_credit_score(maker_id, score_change)?;
    Ok(())
}
```

---

### 3. 买家额度管理（Buyer Quota - 方案C+）

#### 3.1 设计理念

完全替代押金机制，通过信用额度控制买家行为，解决COS押金的逻辑矛盾。

**核心原则**：
- 首购10 USD起步
- 渐进式额度增长
- 并发订单数量控制
- 违约即减额度

#### 3.2 额度计算公式

**最大额度**：

```rust
pub fn calculate_max_quota(credit_score: u16, total_orders: u32) -> u64 {
    // 基础额度（根据信用分）
    let base_quota: u64 = match credit_score {
        900..=1000 => 5000_000_000,  // 5000 USD
        800..=899  => 2000_000_000,  // 2000 USD
        700..=799  => 1000_000_000,  // 1000 USD
        600..=699  =>  500_000_000,  // 500 USD
        500..=599  =>  200_000_000,  // 200 USD
        _          =>  100_000_000,  // 100 USD
    };

    // 新用户首购限制
    if total_orders == 0 {
        return 10_000_000; // 首购仅10 USD
    }

    // 根据订单历史动态调整（每10单增加50 USD）
    let history_boost = (total_orders / 10) as u64 * 50_000_000;

    // 计算最终额度，上限10000 USD
    base_quota.saturating_add(history_boost).min(10000_000_000)
}
```

**最大并发订单数**：

```rust
pub fn calculate_max_concurrent(total_orders: u32) -> u32 {
    match total_orders {
        0..=2   => 1,  // 前3单：仅1笔并发
        3..=9   => 2,  // 3-9单：2笔并发
        10..=49 => 3,  // 10-49单：3笔并发
        _       => 5,  // 50单以上：5笔并发
    }
}
```

#### 3.3 额度占用和释放

**占用额度**（创建订单时）：

```rust
fn occupy_quota(buyer: &T::AccountId, amount_usd: u64) -> DispatchResult {
    BuyerQuotas::<T>::try_mutate(buyer, |profile| {
        // 检查是否被暂停或拉黑
        ensure!(!profile.is_suspended, Error::<T>::BuyerSuspended);
        ensure!(!profile.is_blacklisted, Error::<T>::BuyerBlacklisted);

        // 检查可用额度是否充足
        ensure!(
            profile.available_quota >= amount_usd,
            Error::<T>::InsufficientQuota
        );

        // 检查并发订单数限制
        ensure!(
            profile.active_orders < profile.max_concurrent_orders,
            Error::<T>::ExceedConcurrentLimit
        );

        // 占用额度
        profile.available_quota -= amount_usd;
        profile.occupied_quota += amount_usd;
        profile.active_orders += 1;

        Ok(())
    })
}
```

**释放额度**（订单完成/取消时）：

```rust
fn release_quota(buyer: &T::AccountId, amount_usd: u64) -> DispatchResult {
    BuyerQuotas::<T>::try_mutate(buyer, |profile| {
        // 释放已占用额度
        profile.occupied_quota = profile.occupied_quota.saturating_sub(amount_usd);
        profile.available_quota += amount_usd;
        profile.active_orders = profile.active_orders.saturating_sub(1);

        Ok(())
    })
}
```

#### 3.4 违约惩罚

**惩罚参数计算**：

```rust
pub fn calculate_violation_penalty(
    violation_type: &ViolationType,
    total_violations: u32,
) -> (u16, u16, u32, bool) {
    match violation_type {
        ViolationType::OrderTimeout { .. } => {
            // 订单超时：-20分，额度减半7天
            let score_penalty = 20;
            let quota_reduction_bps = 5000;  // 50%
            let duration_days = 7;
            let suspend = total_violations >= 3;  // 3次超时暂停服务

            (score_penalty, quota_reduction_bps, duration_days, suspend)
        },

        ViolationType::DisputeLoss { .. } => {
            // 争议败诉：-50分，暂停30天
            let score_penalty = 50;
            let quota_reduction_bps = 10000;  // 100%（暂停期间）
            let duration_days = 30;
            let suspend = true;

            (score_penalty, quota_reduction_bps, duration_days, suspend)
        },

        ViolationType::MaliciousBehavior { violation_count } => {
            // 恶意行为：根据次数递增
            if *violation_count >= 3 {
                // 3次以上：永久拉黑
                (100, 10000, u32::MAX, true)
            } else {
                // 1-2次：严厉警告
                (30, 7000, 14, true)
            }
        },
    }
}
```

**惩罚执行**：

```rust
fn record_violation(buyer: &T::AccountId, violation_type: ViolationType) -> DispatchResult {
    BuyerQuotas::<T>::try_mutate(buyer, |profile| {
        // 计算惩罚参数
        let (score_penalty, quota_reduction_bps, penalty_duration_days, should_suspend) =
            calculate_violation_penalty(&violation_type, profile.total_violations);

        // 扣除信用分
        profile.credit_score = profile.credit_score.saturating_sub(score_penalty);

        // 减少额度（按比例）
        let quota_reduction = (profile.max_quota as u128)
            .saturating_mul(quota_reduction_bps as u128)
            .saturating_div(10000);
        profile.max_quota = profile.max_quota.saturating_sub(quota_reduction as u64);
        profile.available_quota = profile.available_quota.min(profile.max_quota);

        // 增加违约次数
        profile.total_violations += 1;
        profile.warnings += 1;
        profile.consecutive_good_orders = 0;  // 重置连续良好订单计数
        profile.last_violation_at = <frame_system::Pallet<T>>::block_number();

        // 是否暂停服务
        if should_suspend {
            profile.is_suspended = true;

            if penalty_duration_days < u32::MAX {
                // 临时暂停
                let suspension_blocks = blocks_per_day * penalty_duration_days;
                profile.suspension_until = Some(current_block + suspension_blocks);
            } else {
                // 永久拉黑
                profile.is_blacklisted = true;
                profile.suspension_until = None;
            }
        }

        Ok(())
    })
}
```

#### 3.5 信用恢复机制

**恢复条件检查**：

```rust
pub fn can_recover_credit<T: frame_system::Config>(
    profile: &BuyerQuotaProfile<T>,
    current_block: BlockNumberFor<T>,
    blocks_per_day: BlockNumberFor<T>,
) -> (bool, u16) {
    // 黑名单用户不可恢复
    if profile.is_blacklisted {
        return (false, 0);
    }

    // 计算距离上次违约的天数
    let blocks_since_violation = current_block - profile.last_violation_at;
    let days_since_violation = blocks_since_violation / blocks_per_day;

    // 恢复条件1：30天内无违约
    if days_since_violation >= 30 {
        return (true, 10);  // 每30天恢复10分
    }

    // 恢复条件2：连续10单无问题
    if profile.consecutive_good_orders >= 10 {
        return (true, 5);  // 连续10单奖励5分
    }

    (false, 0)
}
```

**自动恢复**（在查询暂停状态时触发）：

```rust
fn is_suspended(buyer: &T::AccountId) -> Result<bool, DispatchError> {
    let profile = BuyerQuotas::<T>::get(buyer);
    let current_block = <frame_system::Pallet<T>>::block_number();

    // 检查30天无违约恢复条件
    let (can_recover, recovery_points) = can_recover_credit(
        &profile,
        current_block,
        T::BlocksPerDay::get()
    );

    if can_recover && recovery_points > 0 {
        BuyerQuotas::<T>::mutate(buyer, |p| {
            p.credit_score = p.credit_score.saturating_add(recovery_points).min(1000);

            // 重新计算最大额度
            let old_max_quota = p.max_quota;
            p.max_quota = calculate_max_quota(p.credit_score, p.total_orders);

            // 如果额度提升，更新可用额度
            if p.max_quota > old_max_quota {
                let quota_increase = p.max_quota - old_max_quota;
                p.available_quota = p.available_quota.saturating_add(quota_increase);
            }
        });

        Self::deposit_event(Event::CreditRecovered {
            account: buyer.clone(),
            recovery_points,
            new_credit_score: profile.credit_score,
            recovery_reason: 0, // 30天无违约恢复
        });
    }

    // 检查暂停是否过期
    if profile.is_suspended {
        if let Some(suspension_until) = profile.suspension_until {
            if current_block >= suspension_until {
                // 自动解除暂停
                BuyerQuotas::<T>::mutate(buyer, |p| {
                    p.is_suspended = false;
                    p.suspension_until = None;
                });
                return Ok(false);
            }
        }
        return Ok(true);
    }

    Ok(false)
}
```

---

## 📊 数据结构

### 买家信用记录（CreditScore）

```rust
pub struct CreditScore<T: Config> {
    /// 信用等级（Newbie/Bronze/Silver/Gold/Diamond）
    pub level: CreditLevel,

    /// 新用户等级（前20笔有效）
    pub new_user_tier: Option<NewUserTier>,

    /// 完成订单数
    pub completed_orders: u32,

    /// 累计交易量（NEX）
    pub total_volume: BalanceOf<T>,

    /// 违约次数
    pub default_count: u32,

    /// 争议次数
    pub dispute_count: u32,

    /// 最后购买时间
    pub last_purchase_at: BlockNumberFor<T>,

    /// 风险分（0-1000，越低越好）
    pub risk_score: u16,

    /// 账户创建时间
    pub account_created_at: BlockNumberFor<T>,
}
```

### 做市商信用记录（CreditRecord）

```rust
pub struct CreditRecord<BlockNumber> {
    /// 信用分（800-1000）
    pub credit_score: u16,

    /// 信用等级
    pub level: CreditLevel,

    /// 服务状态
    pub status: ServiceStatus,

    // === 履约数据 ===
    pub total_orders: u32,
    pub completed_orders: u32,
    pub timeout_orders: u32,
    pub cancelled_orders: u32,
    pub timely_release_orders: u32,

    // === 服务质量 ===
    pub rating_sum: u32,
    pub rating_count: u32,
    pub avg_response_time: u32,

    // === 违约记录 ===
    pub default_count: u16,
    pub dispute_loss_count: u16,
    pub last_default_block: Option<BlockNumber>,

    // === 活跃度 ===
    pub last_order_block: BlockNumber,
    pub consecutive_days: u16,
}
```

### 买家额度配置（BuyerQuotaProfile）

```rust
pub struct BuyerQuotaProfile<T: frame_system::Config> {
    /// 信用分（500-1000）
    pub credit_score: u16,

    /// 总完成订单数
    pub total_orders: u32,

    /// 当前可用额度（USD）
    pub available_quota: u64,

    /// 最大额度上限（USD）
    pub max_quota: u64,

    /// 已占用额度（USD）
    pub occupied_quota: u64,

    /// 当前并发订单数
    pub active_orders: u32,

    /// 最大并发订单数
    pub max_concurrent_orders: u32,

    /// 上次违约时间
    pub last_violation_at: BlockNumberFor<T>,

    /// 连续无违约订单数
    pub consecutive_good_orders: u32,

    /// 总违约次数
    pub total_violations: u32,

    /// 警告次数
    pub warnings: u32,

    /// 是否被暂停服务
    pub is_suspended: bool,

    /// 暂停解除时间
    pub suspension_until: Option<BlockNumberFor<T>>,

    /// 是否被永久拉黑
    pub is_blacklisted: bool,
}
```

---

## 💾 存储项

### 买家信用存储

```rust
/// 买家信用记录
#[pallet::storage]
pub type BuyerCredits<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    buyer::CreditScore<T>,
    ValueQuery,
>;

/// 买家每日交易量
#[pallet::storage]
pub type BuyerDailyVolume<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat, T::AccountId,
    Blake2_128Concat, u32,  // 日期（天数）
    u64,  // 累计金额（USDT，精度6）
    ValueQuery,
>;

/// 买家订单历史（最近20笔）
#[pallet::storage]
pub type BuyerOrderHistory<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<buyer::OrderRecord, ConstU32<20>>,
    ValueQuery,
>;

/// 买家推荐人
#[pallet::storage]
pub type BuyerReferrer<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    T::AccountId,
    OptionQuery,
>;

/// 买家背书记录（最多10个）
#[pallet::storage]
pub type BuyerEndorsements<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<buyer::Endorsement<T>, ConstU32<10>>,
    ValueQuery,
>;

/// 转账计数（用于活跃度评估）
#[pallet::storage]
pub type TransferCount<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    u32,
    ValueQuery,
>;

/// 违约历史记录（最多50条）
#[pallet::storage]
pub type DefaultHistory<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<BlockNumberFor<T>, ConstU32<50>>,
    ValueQuery,
>;
```

### 做市商信用存储

```rust
/// 做市商信用记录
#[pallet::storage]
pub type MakerCredits<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    u64,  // maker_id
    maker::CreditRecord<BlockNumberFor<T>>,
    OptionQuery,
>;

/// 做市商买家评分记录
#[pallet::storage]
pub type MakerRatings<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat, u64,  // maker_id
    Blake2_128Concat, u64,  // order_id
    maker::Rating<T::AccountId>,
    OptionQuery,
>;

/// 做市商违约历史
#[pallet::storage]
pub type MakerDefaultHistory<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat, u64,  // maker_id
    Blake2_128Concat, u64,  // order_id
    maker::DefaultRecord<BlockNumberFor<T>>,
    OptionQuery,
>;

/// 做市商动态保证金要求
#[pallet::storage]
pub type MakerDynamicDeposit<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    u64,  // maker_id
    BalanceOf<T>,
    ValueQuery,
>;
```

### 买家额度管理存储

```rust
/// 买家额度配置记录
#[pallet::storage]
pub type BuyerQuotas<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    quota::BuyerQuotaProfile<T>,
    ValueQuery,
>;

/// 买家违约记录历史（最多20条）
#[pallet::storage]
pub type BuyerViolations<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<quota::ViolationRecord<T>, ConstU32<20>>,
    ValueQuery,
>;

/// 买家当前活跃订单列表（最多10个）
#[pallet::storage]
pub type BuyerActiveOrders<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<u64, ConstU32<10>>,
    ValueQuery,
>;
```

---

## 🎯 外部调用（Extrinsics）

### 买家信用调用

```rust
/// 推荐用户（老用户为新用户担保）
#[pallet::call_index(0)]
pub fn endorse_user(
    origin: OriginFor<T>,
    endorsee: T::AccountId,
) -> DispatchResult;

/// 设置邀请人（仅能设置一次）
#[pallet::call_index(1)]
pub fn set_referrer(
    origin: OriginFor<T>,
    referrer: T::AccountId,
) -> DispatchResult;
```

### 做市商信用调用

```rust
/// 买家评价做市商
#[pallet::call_index(2)]
pub fn rate_maker(
    origin: OriginFor<T>,
    maker_id: u64,
    order_id: u64,
    stars: u8,                              // 1-5星
    tags_codes: BoundedVec<u8, ConstU32<5>>, // 标签代码
) -> DispatchResult;
```

---

## 📡 事件定义

### 买家信用事件

```rust
/// 新用户初始化
NewUserInitialized {
    account: T::AccountId,
    tier_code: u8,        // 0=Premium, 1=Standard, 2=Basic, 3=Restricted
    risk_score: u16,
}

/// 买家信用更新
BuyerCreditUpdated {
    account: T::AccountId,
    new_risk_score: u16,
    new_level_code: u8,   // 0=Newbie, 1=Bronze, 2=Silver, 3=Gold, 4=Diamond
}

/// 买家等级升级
BuyerLevelUpgraded {
    account: T::AccountId,
    old_level_code: u8,
    new_level_code: u8,
}

/// 买家违约惩罚
BuyerDefaultPenalty {
    account: T::AccountId,
    penalty: u16,
    consecutive_defaults: u32,
    new_risk_score: u16,
}

/// 连续违约检测
ConsecutiveDefaultDetected {
    account: T::AccountId,
    consecutive_count: u32,
    within_days: u32,
}

/// 用户被封禁
UserBanned {
    account: T::AccountId,
    reason: BoundedVec<u8, ConstU32<128>>,
}

/// 用户推荐
UserEndorsed {
    endorser: T::AccountId,
    endorsee: T::AccountId,
}

/// 设置邀请人
ReferrerSet {
    invitee: T::AccountId,
    referrer: T::AccountId,
}

/// 行为模式识别
BehaviorPatternDetected {
    account: T::AccountId,
    pattern_code: u8,     // 0=HighQuality, 1=Good, 2=Normal, 3=Suspicious
    adjustment: i16,
}

/// 风险分自然衰减
RiskScoreDecayed {
    account: T::AccountId,
    decay_amount: u16,
    new_risk_score: u16,
}
```

### 做市商信用事件

```rust
/// 初始化做市商信用记录
MakerCreditInitialized {
    maker_id: u64,
    initial_score: u16,
}

/// 订单完成，信用分增加
MakerOrderCompleted {
    maker_id: u64,
    order_id: u64,
    new_score: u16,
    bonus: u16,
}

/// 订单超时，信用分减少
MakerOrderTimeout {
    maker_id: u64,
    order_id: u64,
    new_score: u16,
    penalty: u16,
}

/// 争议解决，影响信用分
MakerDisputeResolved {
    maker_id: u64,
    order_id: u64,
    maker_win: bool,
    new_score: u16,
}

/// 买家评价做市商
MakerRated {
    maker_id: u64,
    order_id: u64,
    buyer: T::AccountId,
    stars: u8,
    new_score: u16,
}

/// 服务状态变更
MakerStatusChanged {
    maker_id: u64,
    old_status_code: u8,  // 0=Active, 1=Warning, 2=Suspended
    new_status_code: u8,
    credit_score: u16,
}

/// 信用等级变更
MakerLevelChanged {
    maker_id: u64,
    old_level_code: u8,   // 0=Diamond, 1=Platinum, 2=Gold, 3=Silver, 4=Bronze
    new_level_code: u8,
    credit_score: u16,
}
```

### 买家额度管理事件

```rust
/// 买家额度初始化
BuyerQuotaInitialized {
    account: T::AccountId,
    initial_quota_usd: u64,
    credit_score: u16,
}

/// 占用额度（创建订单）
QuotaOccupied {
    account: T::AccountId,
    order_id: u64,
    amount_usd: u64,
    remaining_quota: u64,
}

/// 释放额度（订单完成/取消）
QuotaReleased {
    account: T::AccountId,
    order_id: u64,
    amount_usd: u64,
    new_available_quota: u64,
}

/// 额度提升
QuotaIncreased {
    account: T::AccountId,
    old_max_quota: u64,
    new_max_quota: u64,
    reason: BoundedVec<u8, ConstU32<64>>,
}

/// 额度降低
QuotaDecreased {
    account: T::AccountId,
    old_max_quota: u64,
    new_max_quota: u64,
    reduction_bps: u16,
    duration_days: u32,
}

/// 买家违约记录
BuyerViolationRecorded {
    account: T::AccountId,
    violation_type: u8,   // 0=Timeout, 1=DisputeLoss, 2=Malicious
    score_penalty: u16,
    new_credit_score: u16,
}

/// 买家服务暂停
BuyerSuspended {
    account: T::AccountId,
    reason: BoundedVec<u8, ConstU32<128>>,
    suspension_until: BlockNumberFor<T>,
}

/// 买家服务恢复
BuyerReinstated {
    account: T::AccountId,
    new_credit_score: u16,
    new_max_quota: u64,
}

/// 买家被永久拉黑
BuyerBlacklisted {
    account: T::AccountId,
    reason: BoundedVec<u8, ConstU32<128>>,
    total_violations: u32,
}

/// 信用恢复
CreditRecovered {
    account: T::AccountId,
    recovery_points: u16,
    new_credit_score: u16,
    recovery_reason: u8,  // 0=30DaysClean, 1=10OrdersBonus
}
```

---

## ❌ 错误定义

### 买家信用错误

```rust
/// 信用分过低（风险分 > 800）
CreditScoreTooLow,

/// 超过单笔限额
ExceedSingleLimit,

/// 超过每日限额
ExceedDailyLimit,

/// 新用户冷却期内不能交易
InCooldownPeriod,

/// 违约冷却期内不能交易
InDefaultCooldown,

/// 推荐人信用不足
InsufficientCreditToEndorse,

/// 不能推荐自己
CannotEndorseSelf,

/// 已经被推荐过
AlreadyEndorsed,

/// 邀请人已设置
ReferrerAlreadySet,

/// 不能邀请自己
CannotReferSelf,
```

### 做市商信用错误

```rust
/// 做市商不存在
MakerNotFound,

/// 信用记录不存在
CreditRecordNotFound,

/// 评分超出范围（必须1-5）
InvalidRating,

/// 已评价过此订单
AlreadyRated,

/// 不是订单买家，无权评价
NotOrderBuyer,

/// 订单未完成，无法评价
OrderNotCompleted,

/// 服务已暂停
ServiceSuspended,

/// 信用分计算溢出
ScoreOverflow,
```

### 买家额度管理错误

```rust
/// 可用额度不足
InsufficientQuota,

/// 超过并发订单数限制
ExceedConcurrentLimit,

/// 买家已被暂停服务
BuyerSuspended,

/// 买家已被拉黑
BuyerBlacklisted,

/// 订单未找到（无法释放额度）
OrderNotFoundForQuotaRelease,

/// 额度配置未初始化
QuotaProfileNotInitialized,

/// 违约记录过多（达到上限20条）
TooManyViolationRecords,

/// 活跃订单列表已满（达到上限10个）
ActiveOrderListFull,
```

---

## ⚙️ 配置参数

### Runtime 配置

```rust
impl pallet_credit::Config for Runtime {
    type Currency = Balances;

    // 买家信用配置
    type InitialBuyerCreditScore = ConstU16<500>;        // 初始信用分
    type OrderCompletedBonus = ConstU16<10>;             // 订单完成加分
    type OrderDefaultPenalty = ConstU16<50>;             // 订单违约扣分
    type BlocksPerDay = ConstU32<14400>;                 // 每日区块数（6秒一个块）
    type MinimumBalance = ConstU128<1_000_000_000_000>;  // 最小余额（用于资产信任）

    // 做市商信用配置
    type InitialMakerCreditScore = ConstU16<820>;        // 初始信用分
    type MakerOrderCompletedBonus = ConstU16<2>;         // 订单完成加分
    type MakerOrderTimeoutPenalty = ConstU16<10>;        // 订单超时扣分
    type MakerDisputeLossPenalty = ConstU16<20>;         // 争议败诉扣分
    type MakerSuspensionThreshold = ConstU16<750>;       // 服务暂停阈值
    type MakerWarningThreshold = ConstU16<800>;          // 服务警告阈值

    // 权重信息
    type CreditWeightInfo = ();
}
```

---

## 🔗 集成说明

### 与其他模块的集成

#### 1. pallet-trading-p2p 集成

**Buy 订单创建时检查买家限额**：

```rust
// p2p/src/lib.rs
use pallet_credit::Pallet as Credit;

pub fn do_create_buy_order(
    buyer: &T::AccountId,
    amount_usd: u64,
    // ...
) -> DispatchResult {
    // 检查买家限额
    Credit::<T>::check_buyer_limit(buyer, amount_usd)?;

    // 创建订单...
}
```

**订单完成时更新信用**：

```rust
pub fn do_release_nex(order_id: u64) -> DispatchResult {
    let order = BuyOrders::<T>::get(order_id)?;

    // 更新买家信用
    Credit::<T>::update_credit_on_success(
        &order.taker,
        order.amount_usdt,
        payment_time_seconds,
    );

    // 更新做市商信用
    Credit::<T>::record_maker_order_completed(
        order.maker_id,
        order_id,
        response_time_seconds,
    )?;

    Ok(())
}
```

**订单超时时惩罚**：

```rust
pub fn do_expire_buy_order(order_id: u64) -> DispatchResult {
    let order = BuyOrders::<T>::get(order_id)?;

    // 买家超时：违约惩罚
    Credit::<T>::penalize_default(&order.taker);

    // 做市商超时：扣信用分
    Credit::<T>::record_maker_order_timeout(order.maker_id, order_id)?;

    Ok(())
}
```

#### 2. pallet-maker 集成

**做市商申请时初始化信用**：

```rust
// maker/src/lib.rs
use pallet_credit::{Pallet as Credit, MakerCreditInterfaceLegacy};

pub fn apply_as_maker(origin: OriginFor<T>) -> DispatchResult {
    let maker = ensure_signed(origin)?;

    // 生成 maker_id
    let maker_id = NextMakerId::<T>::get();

    // 初始化信用记录
    Credit::<T>::initialize_credit(maker_id)?;

    // 保存做市商信息...
    Ok(())
}
```

#### 3. pallet-arbitration 集成

**争议裁决时更新信用**：

```rust
// arbitration/src/lib.rs
use pallet_credit::Pallet as Credit;

pub fn resolve_dispute(
    dispute_id: u64,
    buyer_win: bool,
) -> DispatchResult {
    let dispute = Disputes::<T>::get(dispute_id)?;

    // 更新做市商信用
    Credit::<T>::record_maker_dispute_result(
        dispute.maker_id,
        dispute.order_id,
        !buyer_win,  // maker_win = !buyer_win
    )?;

    Ok(())
}
```

### 买家额度管理接口（方案C+）

**供 pallet-trading-p2p 调用**：

```rust
use pallet_credit::quota::{BuyerQuotaInterface, ViolationType};

// Buy 订单创建时占用额度
pub fn do_create_buy_order(buyer: &T::AccountId, amount_usd: u64) -> DispatchResult {
    T::BuyerCredit::occupy_quota(buyer, amount_usd)?;
    // 创建订单...
    Ok(())
}

// 订单完成时释放额度并提升信用
pub fn do_release_nex(order_id: u64) -> DispatchResult {
    let order = BuyOrders::<T>::get(order_id)?;
    T::BuyerCredit::release_quota(&order.taker, order.amount_usd)?;
    T::BuyerCredit::record_order_completed(&order.taker, order_id)?;
    Ok(())
}

// 订单取消时释放额度
pub fn do_cancel_buy_order(order_id: u64) -> DispatchResult {
    let order = BuyOrders::<T>::get(order_id)?;
    T::BuyerCredit::release_quota(&order.taker, order.amount_usd)?;
    T::BuyerCredit::record_order_cancelled(&order.taker, order_id)?;
    Ok(())
}

// 订单超时时记录违约
pub fn do_expire_buy_order(order_id: u64) -> DispatchResult {
    let order = BuyOrders::<T>::get(order_id)?;
    T::BuyerCredit::record_violation(
        &order.taker,
        ViolationType::OrderTimeout { order_id, timeout_minutes: 120 }
    )?;
    Ok(())
}
```

---

## 📱 前端集成示例

### TypeScript 查询示例

```typescript
import { ApiPromise } from '@polkadot/api';

// 查询买家信用
async function getBuyerCredit(api: ApiPromise, buyerAccount: string) {
    const credit = await api.query.credit.buyerCredits(buyerAccount);

    console.log('风险分:', credit.risk_score.toNumber());
    console.log('信用等级:', credit.level.toString());
    console.log('完成订单数:', credit.completed_orders.toNumber());
    console.log('违约次数:', credit.default_count.toNumber());

    // 判断信用等级
    const levelCode = credit.level.toJSON();
    const levelName = ['Newbie', 'Bronze', 'Silver', 'Gold', 'Diamond'][levelCode];
    console.log('等级名称:', levelName);

    return credit;
}

// 查询做市商信用
async function getMakerCredit(api: ApiPromise, makerId: number) {
    const credit = await api.query.credit.makerCredits(makerId);

    if (credit.isNone) {
        console.log('做市商不存在');
        return null;
    }

    const record = credit.unwrap();
    console.log('信用分:', record.credit_score.toNumber());
    console.log('信用等级:', record.level.toString());
    console.log('服务状态:', record.status.toString());
    console.log('完成率:',
        (record.completed_orders.toNumber() / record.total_orders.toNumber() * 100).toFixed(2) + '%'
    );

    return record;
}

// 查询买家额度
async function getBuyerQuota(api: ApiPromise, buyerAccount: string) {
    const quota = await api.query.credit.buyerQuotas(buyerAccount);

    console.log('可用额度:', quota.available_quota.toNumber() / 1e6, 'USD');
    console.log('最大额度:', quota.max_quota.toNumber() / 1e6, 'USD');
    console.log('已占用额度:', quota.occupied_quota.toNumber() / 1e6, 'USD');
    console.log('活跃订单数:', quota.active_orders.toNumber());
    console.log('最大并发订单数:', quota.max_concurrent_orders.toNumber());
    console.log('是否被暂停:', quota.is_suspended.toHuman());
    console.log('是否被拉黑:', quota.is_blacklisted.toHuman());

    return quota;
}
```

### 买家推荐用户

```typescript
async function endorseUser(api: ApiPromise, endorser: KeyringPair, endorsee: string) {
    const tx = api.tx.credit.endorseUser(endorsee);

    await tx.signAndSend(endorser, ({ status, events }) => {
        if (status.isInBlock) {
            console.log('推荐成功，区块哈希:', status.asInBlock.toHex());

            events.forEach(({ event }) => {
                if (api.events.credit.UserEndorsed.is(event)) {
                    const [endorserAddr, endorseeAddr] = event.data;
                    console.log('推荐人:', endorserAddr.toString());
                    console.log('被推荐人:', endorseeAddr.toString());
                }
            });
        }
    });
}
```

### 买家评价做市商

```typescript
async function rateMaker(
    api: ApiPromise,
    buyer: KeyringPair,
    makerId: number,
    orderId: number,
    stars: number,
    tags: number[]
) {
    // 验证评分范围
    if (stars < 1 || stars > 5) {
        throw new Error('评分必须在1-5之间');
    }

    // 验证标签数量
    if (tags.length > 5) {
        throw new Error('最多5个标签');
    }

    const tx = api.tx.credit.rateMaker(makerId, orderId, stars, tags);

    await tx.signAndSend(buyer, ({ status, events }) => {
        if (status.isInBlock) {
            console.log('评价成功，区块哈希:', status.asInBlock.toHex());

            events.forEach(({ event }) => {
                if (api.events.credit.MakerRated.is(event)) {
                    const [mid, oid, buyerAddr, starsVal, newScore] = event.data;
                    console.log('做市商ID:', mid.toNumber());
                    console.log('订单ID:', oid.toNumber());
                    console.log('评分:', starsVal.toNumber(), '星');
                    console.log('新信用分:', newScore.toNumber());
                }
            });
        }
    });
}

// 评价标签定义
enum RatingTag {
    FastRelease = 0,       // 快速释放
    GoodCommunication = 1, // 沟通良好
    FairPrice = 2,         // 价格合理
    SlowRelease = 3,       // 释放慢
    PoorCommunication = 4, // 沟通差
    Unresponsive = 5,      // 不回应
}

// 使用示例
await rateMaker(
    api,
    buyerKeyring,
    makerId,
    orderId,
    5,  // 5星好评
    [RatingTag.FastRelease, RatingTag.GoodCommunication]
);
```

---

## 🔬 最佳实践

### 买家信用最佳实践

1. **新用户策略**
   - 鼓励新用户设置邀请人（提升社交信任）
   - 引导新用户完成小额首单（降低风险）
   - 前3笔快速学习期引导用户快速付款

2. **风险控制**
   - 限制高风险用户单笔和日限额
   - 违约冷却期防止连续违约
   - 7天3次违约自动封禁

3. **信用提升**
   - 快速付款获得额外加分
   - 大额交易获得额外加分
   - 连续良好行为获得行为模式加分

### 做市商信用最佳实践

1. **服务质量**
   - 及时释放订单（< 24小时）获得加分
   - 保持良好沟通获得高评价
   - 避免订单超时

2. **信用维护**
   - 保持信用分 >= 800（避免警告）
   - 保持信用分 >= 750（避免暂停）
   - 争议时积极提供证据

3. **保证金优化**
   - 提升信用分至950+可减50%保证金
   - 及时释放订单提升信用分
   - 获得买家5星好评提升信用分

### 买家额度管理最佳实践

1. **首购策略**
   - 从10 USD小额首购开始
   - 完成首购后额度逐步增长
   - 并发订单数量逐步放开

2. **额度提升**
   - 完成订单提升信用分
   - 每10单获得50 USD额外额度
   - 连续10单无问题奖励5分

3. **违约恢复**
   - 30天无违约自动恢复10分
   - 连续10单无问题奖励5分
   - 暂停期满自动恢复服务

---

## 📝 版本历史

### v0.2.0 (2026-02-08)

- ✅ 适配 P2P 统一模型，pallet-otc-order 引用更新为 pallet-trading-p2p

### v0.1.0

- ✅ 实现买家信用管理
- ✅ 实现做市商信用管理
- ✅ 实现买家额度管理（方案C+）
- ✅ 5维度信任评估
- ✅ 新用户分层冷启动
- ✅ 快速学习机制
- ✅ 违约惩罚和信用恢复
- ✅ 动态保证金计算
- ✅ 社交信任网络

---

## 📚 相关文档

- [Substrate Documentation](https://docs.substrate.io/)
- [Polkadot SDK stable2506](https://github.com/paritytech/polkadot-sdk/tree/stable2506)
- [FRAME Pallet Guide](https://docs.substrate.io/learn/runtime-development/)

---

## 📞 联系方式

- **GitHub**: https://github.com/lao-sha/cosmos
- **License**: Unlicense

---

## 🎉 致谢

感谢 Polkadot SDK 团队和 Substrate 社区的支持！
