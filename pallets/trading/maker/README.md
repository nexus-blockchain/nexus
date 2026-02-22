# Maker Pallet（做市商管理模块）

## 概述

`pallet-trading-maker` 是 Nexus 交易系统的核心模块之一，负责做市商的完整生命周期管理。做市商是 P2P Buy（USDT→NEX）和 Sell（NEX→USDT）交易的核心参与者，本模块提供了从申请、审核、押金管理到服务运营的全流程支持。

### 主要功能

- **做市商申请与审核**：支持用户申请成为做市商，通过治理流程进行审核
- **押金管理**：锁定/解锁押金，支持动态价格调整和自动补充机制
- **提现管理**：带冷却期的提现机制，保障系统安全
- **溢价配置**：支持 Buy/Sell 方向的溢价设置
- **服务暂停/恢复**：做市商可暂停或恢复服务
- **押金扣除与申诉**：违规行为的惩罚机制和申诉流程
- **惩罚记录归档**：自动归档历史惩罚记录，优化链上存储

### 版本历史

- **v0.1.0** (2025-11-03): 从 `pallet-trading` 拆分而来
- **v0.2.0** (2026-02-08): 适配 P2P 统一模型，OTC/Bridge 术语更新为 Buy/Sell

---

## 核心功能

### 1. 做市商申请与审核流程

```
用户锁定押金 → 提交资料 → 等待审核 → 审核通过/驳回
     ↓              ↓           ↓            ↓
DepositLocked → PendingReview → Active/Rejected/Cancelled
```

**流程说明**：
1. 用户调用 `lock_deposit` 锁定押金（默认 1000 NEX）
2. 在 1 小时内调用 `submit_info` 提交个人资料
3. 治理委员会在 24 小时内审核申请
4. 审核通过后做市商状态变为 `Active`，可开始提供服务

### 2. 押金管理

#### 动态押金机制

押金以 NEX 代币锁定，但目标价值以 USD 计价（默认 1000 USD）。系统会：
- 定期检查押金的 USD 价值
- 当价值低于阈值（950 USD）时触发补充警告
- 支持做市商主动补充或系统自动补充

#### 押金扣除

当做市商发生违规行为时，系统会从押金中扣除相应金额：
- Buy 订单超时：50 USD
- Sell 兑换超时：30 USD
- 争议仲裁败诉：损失金额的 10% + 20 USD 仲裁费
- 信用分过低：每日 1 USD
- 恶意行为：50-200 USD（根据严重程度）

### 3. 提现管理

做市商可申请提现部分或全部押金，但需遵守冷却期机制：
- 申请提现后需等待 7 天冷却期
- 冷却期内可取消提现请求
- 冷却期满后可执行提现

### 4. 申诉机制

做市商对押金扣除有异议时，可在 7 天内发起申诉：
- 提交申诉证据（IPFS CID）
- 进入仲裁流程
- 申诉成功则退还扣除金额

---

## 数据结构

### ApplicationStatus（申请状态）

```rust
pub enum ApplicationStatus {
    /// 押金已锁定，等待提交资料
    DepositLocked,
    /// 资料已提交，等待审核
    PendingReview,
    /// 审核通过，做市商已激活
    Active,
    /// 审核驳回
    Rejected,
    /// 申请已取消
    Cancelled,
    /// 申请已超时
    Expired,
}
```

### Direction（业务方向）

```rust
pub enum Direction {
    /// 仅 Buy 方向 — 做市商出售 NEX，收取 USDT
    Buy = 0,
    /// 仅 Sell 方向 — 做市商购买 NEX，支付 USDT
    Sell = 1,
    /// 双向（Buy + Sell）— 既可以买入也可以卖出
    BuyAndSell = 2,
}
```

### WithdrawalStatus（提现状态）

```rust
pub enum WithdrawalStatus {
    /// 待执行（冷却期中）
    Pending,
    /// 已执行
    Executed,
    /// 已取消
    Cancelled,
}
```

### PenaltyType（惩罚类型）

```rust
pub enum PenaltyType {
    /// Buy 订单超时
    BuyTimeout { order_id: u64, timeout_hours: u32 },
    /// Sell 兑换超时
    SellTimeout { swap_id: u64, timeout_hours: u32 },
    /// 争议败诉
    ArbitrationLoss { case_id: u64, loss_amount: u64 },
    /// 信用分过低
    LowCreditScore { current_score: u32, days_below_threshold: u32 },
    /// 恶意行为
    MaliciousBehavior { behavior_type: u8, evidence_cid: BoundedVec<u8, ConstU32<64>> },
}
```

### MakerApplication（做市商申请记录）

```rust
pub struct MakerApplication<T: Config> {
    /// 所有者账户
    pub owner: T::AccountId,
    /// 押金金额
    pub deposit: BalanceOf<T>,
    /// 申请状态
    pub status: ApplicationStatus,
    /// 业务方向
    pub direction: Direction,
    /// TRON地址（统一用于OTC收款和Bridge发款）
    pub tron_address: TronAddress,
    /// 公开资料CID（IPFS，加密）
    pub public_cid: Cid,
    /// 私密资料CID（IPFS，加密）
    pub private_cid: Cid,
    /// Buy溢价（基点，-500 ~ 500）
    pub buy_premium_bps: i16,
    /// Sell溢价（基点，-500 ~ 500）
    pub sell_premium_bps: i16,
    /// 最小交易金额
    pub min_amount: BalanceOf<T>,
    /// 创建时间（Unix时间戳，秒）
    pub created_at: u32,
    /// 资料提交截止时间（Unix时间戳，秒）
    pub info_deadline: u32,
    /// 审核截止时间（Unix时间戳，秒）
    pub review_deadline: u32,
    /// 服务暂停状态
    pub service_paused: bool,
    /// 已服务用户数量
    pub users_served: u32,
    /// 脱敏姓名
    pub masked_full_name: BoundedVec<u8, ConstU32<64>>,
    /// 脱敏身份证号
    pub masked_id_card: BoundedVec<u8, ConstU32<32>>,
    /// 脱敏生日
    pub masked_birthday: BoundedVec<u8, ConstU32<16>>,
    /// 脱敏收款方式信息（JSON格式）
    pub masked_payment_info: BoundedVec<u8, ConstU32<512>>,
    /// 微信号
    pub wechat_id: BoundedVec<u8, ConstU32<64>>,
    /// 押金目标USD价值
    pub target_deposit_usd: u64,
    /// 上次价格检查时间
    pub last_price_check: BlockNumberFor<T>,
    /// 押金不足警告状态
    pub deposit_warning: bool,
}
```

### WithdrawalRequest（提现请求记录）

```rust
pub struct WithdrawalRequest<Balance> {
    /// 提现金额
    pub amount: Balance,
    /// 申请时间（Unix时间戳，秒）
    pub requested_at: u32,
    /// 可执行时间（Unix时间戳，秒）
    pub executable_at: u32,
    /// 请求状态
    pub status: WithdrawalStatus,
}
```

### PenaltyRecord（惩罚记录）

```rust
pub struct PenaltyRecord<T: Config> {
    /// 做市商ID
    pub maker_id: u64,
    /// 扣除类型
    pub penalty_type: PenaltyType,
    /// 扣除的NEX数量
    pub deducted_amount: BalanceOf<T>,
    /// 扣除时的USD价值
    pub usd_value: u64,
    /// 受益人账户（如果有）
    pub beneficiary: Option<T::AccountId>,
    /// 扣除时间
    pub deducted_at: BlockNumberFor<T>,
    /// 是否已申诉
    pub appealed: bool,
    /// 申诉结果
    pub appeal_result: Option<bool>,
}
```

### ArchivedPenaltyL2（归档惩罚记录）

```rust
pub struct ArchivedPenaltyL2 {
    /// 惩罚记录ID
    pub penalty_id: u64,
    /// 做市商ID
    pub maker_id: u64,
    /// 扣除的USD价值
    pub usd_value: u64,
    /// 惩罚类型代码 (0=BuyTimeout, 1=SellTimeout, 2=ArbitrationLoss, 3=LowCredit, 4=Malicious)
    pub penalty_type_code: u8,
    /// 申诉结果 (0=未申诉, 1=申诉成功, 2=申诉失败)
    pub appeal_status: u8,
}
```

---

## 存储项

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextMakerId` | `StorageValue<u64>` | 下一个做市商ID |
| `MakerApplications` | `StorageMap<u64, MakerApplication>` | 做市商申请记录 |
| `AccountToMaker` | `StorageMap<AccountId, u64>` | 账户到做市商ID映射 |
| `WithdrawalRequests` | `StorageMap<u64, WithdrawalRequest>` | 提现请求记录 |
| `NextPenaltyId` | `StorageValue<u64>` | 下一个惩罚记录ID |
| `PenaltyRecords` | `StorageMap<u64, PenaltyRecord>` | 惩罚记录 |
| `MakerPenalties` | `StorageMap<u64, BoundedVec<u64>>` | 做市商惩罚记录列表 |
| `DepositCheckCursor` | `StorageValue<u64>` | 押金自动补充检查游标 |
| `PenaltyArchiveCursor` | `StorageValue<u64>` | 惩罚记录归档游标 |
| `ArchivedPenalties` | `StorageMap<u32, BoundedVec<ArchivedPenaltyL2>>` | 已归档惩罚记录（按年月索引） |

---

## Extrinsics（可调用函数）

### 用户调用

| 函数 | 参数 | 说明 |
|------|------|------|
| `lock_deposit` | - | 锁定做市商押金，开始申请流程 |
| `submit_info` | `real_name`, `id_card_number`, `birthday`, `tron_address`, `wechat_id` | 提交做市商资料 |
| `cancel_maker` | - | 取消做市商申请（仅限 DepositLocked/PendingReview 状态） |
| `request_withdrawal` | `amount` | 申请提现押金 |
| `execute_withdrawal` | - | 执行提现（冷却期满后） |
| `cancel_withdrawal` | - | 取消提现请求 |
| `replenish_deposit` | - | 主动补充押金 |
| `appeal_penalty` | `penalty_id`, `evidence_cid` | 申诉押金扣除 |

### 治理调用

| 函数 | 参数 | 说明 |
|------|------|------|
| `approve_maker` | `maker_id` | 审批做市商申请（需治理权限） |
| `reject_maker` | `maker_id` | 驳回做市商申请（需治理权限） |
| `emergency_withdrawal` | `maker_id`, `to` | 紧急提现（需治理权限） |
| `force_replenish_deposit` | `maker_id` | 强制补充押金（需治理权限） |

---

## 事件

| 事件 | 参数 | 说明 |
|------|------|------|
| `MakerDepositLocked` | `maker_id`, `who`, `amount` | 押金已锁定 |
| `MakerInfoSubmitted` | `maker_id`, `who` | 资料已提交 |
| `MakerApproved` | `maker_id`, `approved_by` | 做市商已批准 |
| `MakerRejected` | `maker_id`, `rejected_by` | 做市商已驳回 |
| `MakerCancelled` | `maker_id`, `who` | 做市商申请已取消 |
| `WithdrawalRequested` | `maker_id`, `amount` | 提现已申请 |
| `WithdrawalExecuted` | `maker_id`, `amount` | 提现已执行 |
| `WithdrawalCancelled` | `maker_id` | 提现已取消 |
| `EmergencyWithdrawalExecuted` | `maker_id`, `to`, `amount` | 紧急提现已执行 |
| `DepositReplenished` | `maker_id`, `amount`, `total_deposit` | 押金已补充 |
| `DepositInsufficient` | `maker_id`, `current_usd_value` | 押金不足警告 |
| `DepositCheckCompleted` | `checked_count`, `insufficient_count` | 押金检查完成 |
| `DepositDeducted` | `maker_id`, `penalty_id`, `deducted_amount`, `usd_value`, `reason`, `beneficiary` | 押金已扣除 |
| `DepositReplenishmentRequired` | `maker_id`, `current_usd_value`, `required_usd_value` | 需要补充押金 |
| `PenaltyAppealed` | `maker_id`, `penalty_id`, `appeal_case_id` | 押金扣除申诉 |
| `AppealResultProcessed` | `penalty_id`, `maker_id`, `appeal_granted` | 申诉结果处理 |
| `PenaltyRefunded` | `penalty_id`, `maker_id`, `refunded_amount` | 押金已退还 |
| `PenaltyArchived` | `penalty_id`, `maker_id`, `year_month` | 惩罚记录已归档 |

---

## 错误

| 错误 | 说明 |
|------|------|
| `MakerAlreadyExists` | 已经申请过做市商 |
| `MakerNotFound` | 做市商不存在 |
| `InvalidMakerStatus` | 状态不正确 |
| `InsufficientDeposit` | 押金不足 |
| `MakerNotActive` | 做市商未激活 |
| `InsufficientBalance` | 余额不足 |
| `InvalidTronAddress` | 无效的 TRON 地址 |
| `EncodingError` | 编码错误 |
| `WithdrawalRequestNotFound` | 提现请求不存在 |
| `WithdrawalCooldownNotMet` | 提现冷却期未满足 |
| `NotAuthorized` | 未授权 |
| `PriceNotAvailable` | 价格不可用 |
| `DepositCalculationOverflow` | 押金计算溢出 |
| `CannotReplenishDeposit` | 押金不足且无法补充 |
| `PenaltyRecordNotFound` | 惩罚记录不存在 |
| `AlreadyAppealed` | 已经申诉过 |
| `AppealDeadlineExpired` | 申诉期限已过 |
| `EvidenceTooLong` | 证据太长 |
| `OrderNotFound` | Buy 订单不存在 |
| `SwapNotFound` | Sell 订单不存在 |
| `CalculationOverflow` | 计算溢出 |

---

## 配置参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `MakerDepositAmount` | `BalanceOf<T>` | 1000 NEX | 做市商押金金额 |
| `TargetDepositUsd` | `u64` | 1,000,000,000 (1000 USD) | 押金目标USD价值（精度10^6） |
| `DepositReplenishThreshold` | `u64` | 950,000,000 (950 USD) | 押金补充触发阈值（精度10^6） |
| `DepositReplenishTarget` | `u64` | 1,050,000,000 (1050 USD) | 押金补充目标（精度10^6） |
| `PriceCheckInterval` | `BlockNumberFor<T>` | 每小时 | 价格检查间隔（区块数） |
| `AppealDeadline` | `BlockNumberFor<T>` | 7天 | 申诉时限（区块数） |
| `MakerApplicationTimeout` | `BlockNumberFor<T>` | - | 申请超时时间（区块数） |
| `WithdrawalCooldown` | `BlockNumberFor<T>` | 7天 | 提现冷却期（区块数） |

---

## 使用示例

### 1. 申请成为做市商

```rust
// 步骤1：锁定押金
Maker::lock_deposit(origin)?;

// 步骤2：提交资料（1小时内）
Maker::submit_info(
    origin,
    b"张三".to_vec(),
    b"110101199001011234".to_vec(),
    b"1990-01-01".to_vec(),
    b"TXxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_vec(),
    b"wechat_id_123".to_vec(),
)?;

// 步骤3：等待治理审核
// 治理委员会调用 approve_maker 或 reject_maker
```

### 2. 提现押金

```rust
// 步骤1：申请提现
let amount = 500_000_000_000_000u128; // 500 NEX
Maker::request_withdrawal(origin, amount)?;

// 步骤2：等待7天冷却期

// 步骤3：执行提现
Maker::execute_withdrawal(origin)?;
```

### 3. 补充押金

```rust
// 主动补充押金（当收到 DepositReplenishmentRequired 事件时）
Maker::replenish_deposit(origin)?;
```

### 4. 申诉押金扣除

```rust
// 在7天内对押金扣除发起申诉
let penalty_id = 123u64;
let evidence_cid = b"QmXxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_vec();
Maker::appeal_penalty(origin, penalty_id, evidence_cid)?;
```

---

## 公共查询接口

| 函数 | 参数 | 返回值 | 说明 |
|------|------|--------|------|
| `is_maker` | `who: &AccountId` | `bool` | 检查账户是否是做市商 |
| `is_maker_active` | `maker_id: u64` | `bool` | 检查做市商是否活跃 |
| `get_maker_id` | `who: &AccountId` | `Option<u64>` | 获取做市商ID |
| `get_deposit_usd_value` | `maker_id: u64` | `Result<u64, DispatchError>` | 查询押金的USD价值 |
| `needs_deposit_replenishment` | `maker_id: u64` | `Result<bool, DispatchError>` | 检查是否需要补充押金 |

---

## Hooks

### on_idle

模块在空闲时自动执行以下任务：

1. **押金自动补充检查**：每次检查最多 5 个做市商的押金状态，如果押金不足且账户余额充足，自动触发补充
2. **惩罚记录归档**：每次归档最多 3 条超过 30 天的惩罚记录，减少链上存储占用

---

## 依赖模块

- `frame_support`: Substrate 框架支持
- `frame_system`: 系统模块
- `pallet_trading_common`: 交易公共类型和接口
- `pallet_nexus_ipfs`: IPFS 内容注册（用于自动 Pin 做市商资料）

---

## 安全考虑

1. **押金保护**：押金通过 `ReservableCurrency` 锁定，确保资金安全
2. **冷却期机制**：提现需等待 7 天冷却期，防止恶意提现
3. **治理审核**：做市商申请需通过治理委员会审核
4. **申诉机制**：押金扣除可在 7 天内申诉，保障做市商权益
5. **紧急提现**：治理可在紧急情况下强制提现，保护用户资金

---

## License

Apache-2.0
