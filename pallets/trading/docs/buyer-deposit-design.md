# OTC 买家押金机制设计方案

> 版本：v1.0  
> 日期：2026-01-18  
> 状态：设计中

---

## 1. 背景与目标

### 1.1 问题描述

当前 OTC 流程中，买家下单后做市商立即锁定 NEX。如果买家不付款：
- 做市商 NEX 被占用 1-2 小时
- 订单超时后才能解锁
- 恶意买家可发起 DoS 攻击，占用做市商流动性

### 1.2 设计目标

1. **保护做市商**：防止恶意下单，补偿资金占用损失
2. **保护新用户**：首购用户免押金，降低入金门槛
3. **激励诚信交易**：信用分高的用户享受更低押金

---

## 2. 用户分类

| 用户类型 | 定义 | 押金要求 |
|----------|------|----------|
| **首购用户** | 从未成功完成过 OTC 订单 | ❌ 免押金 |
| **普通用户** | 已完成 1+ 笔订单，信用分 < 70 | ✅ 需押金 |
| **信用用户** | 已完成 5+ 笔订单，信用分 ≥ 70 | ❌ 免押金 |

---

## 3. 押金规则

### 3.1 押金比例

| 买家信用分 | 押金比例 | 说明 |
|------------|----------|------|
| 首购用户 | 0% | 降低入金门槛 |
| ≥ 70 分 | 0% | 信用良好免押金 |
| 50-69 分 | 3% | 正常用户 |
| 30-49 分 | 5% | 需要约束 |
| < 30 分 | 10% | 高风险用户 |

### 3.2 押金资产类型

押金支持以下资产类型（按优先级）：

| 优先级 | 资产类型 | 说明 |
|--------|----------|------|
| 1 | NEX | 优先使用 NEX 押金 |
| 2 | 原生币 | NEX 不足时，可用原生币等值押金 |

> **注意**：如买家 NEX 余额不足，系统自动尝试使用原生币押金。

### 3.3 押金计算

```
押金金额 = max(订单COS金额 × 押金比例, 最小押金)

示例：
- 订单金额: 1000 NEX
- 买家信用分: 65
- 押金比例: 3%
- 押金金额: 1000 × 3% = 30 NEX
```

### 3.4 配置参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `FirstPurchaseFixedAmount` | Balance | 10 USDT | 首购固定金额 |
| `MinDeposit` | Balance | 1 NEX | 最小押金金额 |
| `DepositRateLow` | u16 | 300 (3%) | 低风险押金比例(bps) |
| `DepositRateMedium` | u16 | 500 (5%) | 中风险押金比例(bps) |
| `CancelPenaltyRate` | u16 | 3000 (30%) | 取消订单押金扣除比例(bps) |
| `CreditScoreExempt` | u16 | **70** | 免押金信用分阈值 |
| `MinOrdersForExempt` | u32 | 5 | 免押金最少订单数 |

---

## 4. 押金生命周期

### 4.1 流程图

```
┌─────────────────────────────────────────────────────────────────┐
│                        买家下单                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  是否首购用户？   │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │ 是                          │ 否
              ▼                             ▼
    ┌─────────────────┐           ┌─────────────────┐
    │ 免押金，标记首购 │           │ 查询买家信用分   │
    └────────┬────────┘           └────────┬────────┘
             │                             │
             │                             ▼
             │                   ┌─────────────────┐
             │                   │ 信用分 ≥ 70 且   │
             │                   │ 订单数 ≥ 5？     │
             │                   └────────┬────────┘
             │                            │
             │             ┌──────────────┴──────────────┐
             │             │ 是                          │ 否
             │             ▼                             ▼
             │   ┌─────────────────┐           ┌─────────────────┐
             │   │     免押金      │           │ 计算并锁定押金   │
             │   └────────┬────────┘           └────────┬────────┘
             │            │                             │
             └────────────┴──────────────┬──────────────┘
                                         │
                                         ▼
                              ┌─────────────────┐
                              │ 做市商锁定 NEX  │
                              └────────┬────────┘
                                       │
                                       ▼
                              ┌─────────────────┐
                              │   等待买家付款   │
                              └────────┬────────┘
                                       │
          ┌────────────────────────────┼────────────────────────────┐
          │                            │                            │
          ▼                            ▼                            ▼
  ┌───────────────┐          ┌───────────────┐          ┌───────────────┐
  │   订单完成     │          │  买家主动取消  │          │   订单超时    │
  └───────┬───────┘          └───────┬───────┘          └───────┬───────┘
          │                          │                          │
          ▼                          ▼                          ▼
  ┌───────────────┐          ┌───────────────┐          ┌───────────────┐
  │ 押金全额退还   │          │ 扣30%给做市商  │          │ 扣100%给做市商 │
  │ 提升信用分     │          │ 70%退还买家    │          │ 降低信用分     │
  └───────────────┘          │ 轻微降信用分   │          └───────────────┘
                             └───────────────┘
```

### 4.2 押金处理规则

| 场景 | 押金处理 | 信用分影响 |
|------|----------|------------|
| **订单完成** | 100% 退还买家 | +5 分 |
| **买家主动取消** | **30%** 赔付做市商，**70%** 退还 | -3 分 |
| **订单超时** | 100% 赔付做市商 | -10 分 |
| **做市商取消** | 100% 退还买家 | 买家不受影响 |
| **争议-买家胜** | 100% 退还买家 + 争议押金 | 不变 |
| **争议-做市商胜** | 100% 赔付做市商 + 争议押金 | -15 分 |

---

## 5. 争议押金机制

### 5.1 设计目标

防止恶意争议，保护双方权益：
- **防止买家滥用争议**：发起争议需要成本
- **防止做市商拖延**：争议有时间限制
- **公平判定**：败诉方承担争议成本

### 5.2 争议押金参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `DisputeDeposit` | Balance | 10 USDT | 发起争议所需押金 |
| `DisputeTimeout` | MomentOf | 48 小时 | 争议处理超时时间 |
| `DisputeEvidenceTimeout` | MomentOf | 24 小时 | 提交证据截止时间 |

### 5.3 争议流程

```
订单状态异常（买家已付款但做市商未确认）
                │
                ▼
┌─────────────────────────────────────┐
│ 买家发起争议                         │
│ - 锁定争议押金 10 USDT               │
│ - 提交付款凭证                       │
└─────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────┐
│ 做市商响应（24小时内）               │
│ - 锁定争议押金 10 USDT               │
│ - 提交反驳证据                       │
└─────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────┐
│ 仲裁判定（48小时内）                 │
│ - 链上治理投票 或                    │
│ - 指定仲裁员判定                     │
└─────────────────────────────────────┘
                │
        ┌───────┴───────┐
        ▼               ▼
  买家胜诉          做市商胜诉
  ┌────────┐        ┌────────┐
  │退还买家:│        │赔付做市商:│
  │- 订单押金│        │- 订单押金 │
  │- 争议押金│        │- 争议押金 │
  │- 做市商争│        │- 买家争议 │
  │  议押金  │        │  押金    │
  └────────┘        └────────┘
```

### 5.4 争议押金处理规则

| 场景 | 买家争议押金 | 做市商争议押金 | 订单押金 |
|------|-------------|---------------|----------|
| **买家胜诉** | 退还买家 | 赔付买家 | 退还买家 |
| **做市商胜诉** | 赔付做市商 | 退还做市商 | 赔付做市商 |
| **做市商未响应** | 退还买家 | - | 退还买家 |
| **仲裁超时** | 各自退还 | 各自退还 | 退还买家 |

### 5.5 数据结构

```rust
/// 争议状态
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum DisputeStatus {
    /// 等待做市商响应
    WaitingMakerResponse,
    /// 等待仲裁
    WaitingArbitration,
    /// 买家胜诉
    BuyerWon,
    /// 做市商胜诉
    MakerWon,
    /// 已取消
    Cancelled,
}

/// 争议记录
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct Dispute<T: Config> {
    /// 订单ID
    pub order_id: u64,
    /// 发起方（买家）
    pub initiator: T::AccountId,
    /// 被告方（做市商）
    pub respondent: T::AccountId,
    /// 买家争议押金
    pub buyer_dispute_deposit: BalanceOf<T>,
    /// 做市商争议押金
    pub maker_dispute_deposit: BalanceOf<T>,
    /// 发起时间
    pub created_at: MomentOf,
    /// 做市商响应截止时间
    pub response_deadline: MomentOf,
    /// 仲裁截止时间
    pub arbitration_deadline: MomentOf,
    /// 争议状态
    pub status: DisputeStatus,
    /// 买家证据 CID
    pub buyer_evidence: Option<Cid>,
    /// 做市商证据 CID
    pub maker_evidence: Option<Cid>,
}
```

### 5.6 核心函数

```rust
/// 买家发起争议
#[pallet::call_index(10)]
#[pallet::weight(...)]
pub fn initiate_dispute(
    origin: OriginFor<T>,
    order_id: u64,
    evidence_cid: Cid,
) -> DispatchResult {
    let buyer = ensure_signed(origin)?;
    
    // 1. 验证订单状态（必须是已付款待确认）
    let order = Orders::<T>::get(order_id)?;
    ensure!(order.state == OrderState::BuyerPaid, Error::<T>::InvalidOrderState);
    ensure!(order.taker == buyer, Error::<T>::NotOrderBuyer);
    
    // 2. 锁定争议押金
    T::Currency::reserve(&buyer, T::DisputeDeposit::get())?;
    
    // 3. 创建争议记录
    let now = T::Time::now();
    let dispute = Dispute {
        order_id,
        initiator: buyer.clone(),
        respondent: order.maker.clone(),
        buyer_dispute_deposit: T::DisputeDeposit::get(),
        maker_dispute_deposit: Zero::zero(),
        created_at: now,
        response_deadline: now + T::DisputeEvidenceTimeout::get(),
        arbitration_deadline: now + T::DisputeTimeout::get(),
        status: DisputeStatus::WaitingMakerResponse,
        buyer_evidence: Some(evidence_cid),
        maker_evidence: None,
    };
    
    Disputes::<T>::insert(order_id, dispute);
    
    // 4. 更新订单状态
    Orders::<T>::mutate(order_id, |o| {
        if let Some(order) = o {
            order.state = OrderState::Disputed;
        }
    });
    
    Self::deposit_event(Event::DisputeInitiated { order_id, buyer });
    Ok(())
}

/// 做市商响应争议
#[pallet::call_index(11)]
#[pallet::weight(...)]
pub fn respond_dispute(
    origin: OriginFor<T>,
    order_id: u64,
    evidence_cid: Cid,
) -> DispatchResult {
    let maker = ensure_signed(origin)?;
    
    // 1. 验证争议状态
    let mut dispute = Disputes::<T>::get(order_id)?;
    ensure!(dispute.status == DisputeStatus::WaitingMakerResponse, Error::<T>::InvalidDisputeState);
    ensure!(dispute.respondent == maker, Error::<T>::NotDisputeRespondent);
    
    // 2. 检查响应截止时间
    let now = T::Time::now();
    ensure!(now <= dispute.response_deadline, Error::<T>::DisputeResponseTimeout);
    
    // 3. 锁定做市商争议押金
    T::Currency::reserve(&maker, T::DisputeDeposit::get())?;
    
    // 4. 更新争议记录
    dispute.maker_dispute_deposit = T::DisputeDeposit::get();
    dispute.maker_evidence = Some(evidence_cid);
    dispute.status = DisputeStatus::WaitingArbitration;
    Disputes::<T>::insert(order_id, dispute);
    
    Self::deposit_event(Event::DisputeResponded { order_id, maker });
    Ok(())
}

/// 仲裁判定（仅限仲裁员/治理调用）
#[pallet::call_index(12)]
#[pallet::weight(...)]
pub fn resolve_dispute(
    origin: OriginFor<T>,
    order_id: u64,
    buyer_wins: bool,
) -> DispatchResult {
    // 验证调用者是仲裁员
    T::ArbitratorOrigin::ensure_origin(origin)?;
    
    let dispute = Disputes::<T>::get(order_id)?;
    let order = Orders::<T>::get(order_id)?;
    
    if buyer_wins {
        // 买家胜诉：退还买家所有押金 + 做市商争议押金
        Self::handle_buyer_wins(&order, &dispute)?;
    } else {
        // 做市商胜诉：没收买家所有押金
        Self::handle_maker_wins(&order, &dispute)?;
    }
    
    Ok(())
}
```

---

## 6. 首购用户特殊规则

### 6.1 首购定义

```rust
/// 判断是否为首购用户
fn is_first_purchase(buyer: &AccountId) -> bool {
    // 条件1：从未有过成功完成的订单
    BuyerCompletedOrderCount::<T>::get(buyer) == 0
}
```

### 6.2 首购限制

| 限制项 | 值 | 说明 |
|--------|-----|------|
| `FirstPurchaseFixedAmount` | **10 USDT** | 首购固定金额（不可调整） |
| `MaxFirstPurchasePerMaker` | 5 | 每个做市商同时接受的首购订单数 |
| `FirstPurchaseTimeout` | 30 分钟 | 首购订单超时时间（更短） |

> **注意**：首购订单金额固定为 10 USDT，不允许用户自定义金额。
> 这样可以：
> - 降低做市商风险（恶意用户最多占用少量资金）
> - 让新用户体验完整流程
> - 完成首购后即可解锁正常购买额度

### 6.3 首购订单标记

```rust
pub struct Order<T: Config> {
    // ... 现有字段 ...
    
    /// 🆕 是否为首购订单
    pub is_first_purchase: bool,
}
```

### 6.4 做市商首购配额

```rust
/// 做市商当前接受的首购订单数
#[pallet::storage]
pub type MakerFirstPurchaseCount<T> = StorageMap<_, Blake2_128Concat, u64, u32, ValueQuery>;
```

---

## 7. 数据结构变更

### 7.1 Order 结构新增字段

```rust
pub struct Order<T: Config> {
    // === 现有字段 ===
    pub maker_id: u64,
    pub maker: T::AccountId,
    pub taker: T::AccountId,
    pub price: BalanceOf<T>,
    pub qty: BalanceOf<T>,
    pub amount: BalanceOf<T>,
    pub created_at: MomentOf,
    pub expire_at: MomentOf,
    pub state: OrderState,
    // ...
    
    // === 🆕 押金相关字段 ===
    /// 买家押金金额（0 表示免押金）
    pub buyer_deposit: BalanceOf<T>,
    /// 押金状态
    pub deposit_status: DepositStatus,
    /// 是否为首购订单
    pub is_first_purchase: bool,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum DepositStatus {
    /// 无押金（首购/信用免押）
    None,
    /// 押金已锁定
    Locked,
    /// 押金已释放（订单完成）
    Released,
    /// 押金已没收（超时/取消/争议败诉）
    Forfeited,
    /// 押金部分没收
    PartiallyForfeited,
}
```

### 7.2 新增存储项

```rust
/// 买家已完成订单计数
#[pallet::storage]
pub type BuyerCompletedOrderCount<T> = StorageMap<
    _, Blake2_128Concat, T::AccountId, u32, ValueQuery
>;

/// 做市商当前首购订单数
#[pallet::storage]
pub type MakerFirstPurchaseCount<T> = StorageMap<
    _, Blake2_128Concat, u64, u32, ValueQuery
>;
```

### 7.3 新增配置项

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    // === 🆕 押金配置 ===
    
    /// 最小押金金额
    #[pallet::constant]
    type MinDeposit: Get<BalanceOf<Self>>;
    
    /// 低风险押金比例（bps，如 300 = 3%）
    #[pallet::constant]
    type DepositRateLow: Get<u16>;
    
    /// 中风险押金比例（bps）
    #[pallet::constant]
    type DepositRateMedium: Get<u16>;
    
    /// 高风险押金比例（bps）
    #[pallet::constant]
    type DepositRateHigh: Get<u16>;
    
    /// 免押金信用分阈值
    #[pallet::constant]
    type CreditScoreExempt: Get<u16>;
    
    /// 免押金最少完成订单数
    #[pallet::constant]
    type MinOrdersForExempt: Get<u32>;
    
    /// 取消订单押金扣除比例（bps，如 5000 = 50%）
    #[pallet::constant]
    type CancelPenaltyRate: Get<u16>;
}
```

---

## 8. 核心函数

### 8.1 计算押金金额

```rust
/// 计算买家应缴押金
fn calculate_buyer_deposit(
    buyer: &T::AccountId,
    order_amount: BalanceOf<T>,
) -> BalanceOf<T> {
    // 1. 首购用户免押金
    if Self::is_first_purchase(buyer) {
        return Zero::zero();
    }
    
    // 2. 获取买家信用分
    let credit_score = T::Credit::get_buyer_credit_score(buyer);
    let completed_orders = BuyerCompletedOrderCount::<T>::get(buyer);
    
    // 3. 信用用户免押金
    if credit_score >= T::CreditScoreExempt::get() 
        && completed_orders >= T::MinOrdersForExempt::get() 
    {
        return Zero::zero();
    }
    
    // 4. 根据信用分计算押金比例
    let deposit_rate_bps = if credit_score >= 60 {
        T::DepositRateLow::get()      // 3%
    } else if credit_score >= 40 {
        T::DepositRateMedium::get()   // 5%
    } else {
        T::DepositRateHigh::get()     // 10%
    };
    
    // 5. 计算押金金额
    let deposit = order_amount
        .saturating_mul(deposit_rate_bps.into())
        .saturating_div(10000u32.into());
    
    // 6. 确保不低于最小押金
    deposit.max(T::MinDeposit::get())
}
```

### 8.2 处理订单取消

```rust
/// 处理买家主动取消订单
fn handle_buyer_cancel(order_id: u64) -> DispatchResult {
    let order = Orders::<T>::get(order_id)?;
    
    // 1. 释放做市商锁定的 NEX
    T::Escrow::refund_all(order_id, &order.maker)?;
    
    // 2. 处理买家押金
    if order.buyer_deposit > Zero::zero() {
        let penalty = order.buyer_deposit
            .saturating_mul(T::CancelPenaltyRate::get().into())
            .saturating_div(10000u32.into());
        
        let refund = order.buyer_deposit.saturating_sub(penalty);
        
        // 扣除部分赔付给做市商
        if penalty > Zero::zero() {
            T::Currency::transfer(
                &Self::deposit_account(),
                &order.maker,
                penalty,
                ExistenceRequirement::KeepAlive,
            )?;
        }
        
        // 剩余退还买家
        if refund > Zero::zero() {
            T::Currency::transfer(
                &Self::deposit_account(),
                &order.taker,
                refund,
                ExistenceRequirement::KeepAlive,
            )?;
        }
    }
    
    // 3. 降低买家信用分
    T::Credit::record_order_cancelled(&order.taker, order_id);
    
    // 4. 如是首购订单，减少做市商首购计数
    if order.is_first_purchase {
        MakerFirstPurchaseCount::<T>::mutate(order.maker_id, |c| *c = c.saturating_sub(1));
    }
    
    Ok(())
}
```

### 8.3 处理订单超时

```rust
/// 处理订单超时
fn handle_order_timeout(order_id: u64) -> DispatchResult {
    let order = Orders::<T>::get(order_id)?;
    
    // 1. 释放做市商锁定的 NEX
    T::Escrow::refund_all(order_id, &order.maker)?;
    
    // 2. 没收全部买家押金给做市商
    if order.buyer_deposit > Zero::zero() {
        T::Currency::transfer(
            &Self::deposit_account(),
            &order.maker,
            order.buyer_deposit,
            ExistenceRequirement::KeepAlive,
        )?;
    }
    
    // 3. 大幅降低买家信用分
    T::Credit::record_order_timeout(&order.taker, order_id);
    
    // 4. 如是首购订单，减少做市商首购计数
    if order.is_first_purchase {
        MakerFirstPurchaseCount::<T>::mutate(order.maker_id, |c| *c = c.saturating_sub(1));
    }
    
    Ok(())
}
```

---

## 9. 事件定义

```rust
#[pallet::event]
pub enum Event<T: Config> {
    // === 🆕 押金相关事件 ===
    
    /// 买家押金已锁定
    BuyerDepositLocked {
        order_id: u64,
        buyer: T::AccountId,
        deposit_amount: BalanceOf<T>,
    },
    
    /// 买家押金已释放
    BuyerDepositReleased {
        order_id: u64,
        buyer: T::AccountId,
        refund_amount: BalanceOf<T>,
    },
    
    /// 买家押金已没收
    BuyerDepositForfeited {
        order_id: u64,
        buyer: T::AccountId,
        maker_id: u64,
        forfeited_amount: BalanceOf<T>,
        reason: ForfeitReason,
    },
    
    /// 买家押金部分没收
    BuyerDepositPartiallyForfeited {
        order_id: u64,
        buyer: T::AccountId,
        maker_id: u64,
        forfeited_amount: BalanceOf<T>,
        refund_amount: BalanceOf<T>,
    },
}

#[derive(Encode, Decode, Clone, TypeInfo)]
pub enum ForfeitReason {
    Timeout,
    ArbitrationLoss,
}
```

---

## 10. 安全考虑

### 10.1 防止押金池被盗

```rust
/// 押金池账户（PDA，无私钥）
fn deposit_account() -> T::AccountId {
    T::PalletId::get().into_sub_account_truncating(b"deposit")
}
```

### 10.2 防止做市商恶意不确认

- 做市商不确认 → 订单超时 → NEX 自动退回做市商
- 买家押金：如有证据表明已付款，走争议流程
- 争议结果决定押金归属

### 10.3 防止买家恶意争议

- 争议败诉：没收全部押金
- 多次争议败诉：限制下单能力

---

## 11. 迁移计划

### 11.1 存储迁移

```rust
#[pallet::hooks]
impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_runtime_upgrade() -> Weight {
        // 迁移现有订单：设置默认押金字段
        // buyer_deposit: 0
        // deposit_status: None
        // is_first_purchase: false (历史订单不视为首购)
    }
}
```

### 11.2 版本兼容

| 版本 | 变更 |
|------|------|
| v1.0 | 现有版本，无押金机制 |
| v1.1 | 新增押金机制，首购免押 |

---

## 12. 测试用例

| 测试场景 | 预期结果 |
|----------|----------|
| 首购用户下单 | 押金 = 0，标记 is_first_purchase = true |
| 信用分 85 用户下单（已完成 5 单） | 押金 = 0 |
| 信用分 65 用户下单 1000 NEX | 押金 = 30 NEX (3%) |
| 信用分 45 用户下单 1000 NEX | 押金 = 50 NEX (5%) |
| 信用分 30 用户下单 1000 NEX | 押金 = 100 NEX (10%) |
| 订单完成 | 押金全额退还 |
| 买家主动取消 | 押金 50% 赔付做市商 |
| 订单超时 | 押金 100% 赔付做市商 |
| 争议-买家胜 | 押金全额退还 |
| 争议-做市商胜 | 押金 100% 赔付做市商 |

---

## 13. 附录：Runtime 配置示例

```rust
parameter_types! {
    pub const MinDeposit: Balance = 1 * NEX;
    pub const DepositRateLow: u16 = 300;      // 3%
    pub const DepositRateMedium: u16 = 500;   // 5%
    pub const DepositRateHigh: u16 = 1000;    // 10%
    pub const CreditScoreExempt: u16 = 80;
    pub const MinOrdersForExempt: u32 = 5;
    pub const CancelPenaltyRate: u16 = 5000;  // 50%
}

impl pallet_trading_otc::Config for Runtime {
    // ... 现有配置 ...
    
    type MinDeposit = MinDeposit;
    type DepositRateLow = DepositRateLow;
    type DepositRateMedium = DepositRateMedium;
    type DepositRateHigh = DepositRateHigh;
    type CreditScoreExempt = CreditScoreExempt;
    type MinOrdersForExempt = MinOrdersForExempt;
    type CancelPenaltyRate = CancelPenaltyRate;
}
```

---

## 14. 总结

| 用户类型 | 押金 | 金额限制 | 超时时间 |
|----------|------|----------|----------|
| 首购用户 | 0% | **固定 10 USDT** | 30 分钟 |
| 信用用户 (≥70分, ≥5单) | 0% | 无限制 | 1 小时 |
| 普通用户 (50-69分) | 3% | 无限制 | 1 小时 |
| 低信用用户 (30-49分) | 5% | 无限制 | 1 小时 |
| 高风险用户 (<30分) | 10% | 无限制 | 1 小时 |

**核心原则**：
1. ✅ 新用户零门槛入金（首购免押金）
2. ✅ 诚信用户免押金（信用激励）
3. ✅ 恶意用户高成本（押金约束）
4. ✅ 做市商有保障（超时/取消可获赔）
