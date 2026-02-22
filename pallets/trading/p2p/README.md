# P2P Trading Pallet（P2P 交易模块）

## 概述

`pallet-trading-p2p` 是 Nexus 交易系统的核心交易模块，统一 **Buy（USDT→NEX）** 和 **Sell（NEX→USDT）** 两方向的 P2P 交易。

> 本模块合并了原 `pallet-trading-otc`（Buy 方向）和 `pallet-trading-swap`（Sell 方向），于 2026-02-08 统一为单一模块。

### Runtime Index: **55**

### 主要功能

- **Buy 订单**：买家用 USDT 购买做市商的 NEX（链下 USDT 转账 + 链上 NEX 托管释放）
- **Sell 订单**：用户出售 NEX 换取做市商的 USDT（链上 NEX 托管 + OCW TRC20 验证）
- **首购机制**：新用户固定金额首购，免押金
- **KYC 验证**：可选 KYC 要求，支持豁免账户
- **买家押金**：基于信用的动态押金机制
- **争议仲裁**：Buy/Sell 双方向争议处理与仲裁桥接
- **自动过期**：Buy 订单超时自动过期处理
- **归档系统**：已完成订单自动归档，优化链上存储
- **TxHash 防重放**：Buy/Sell 双方向 TRON 交易哈希防重放

---

## 交易流程

### Buy 流程（USDT → NEX）

```
买家 create_buy_order ──→ 做市商 NEX 锁定到托管
         │
买家链下转 USDT ──→ mark_paid（标记已付款）
         │
做市商确认收款 ──→ release_nex（NEX 释放给买家）
         │
    ┌────┴────┐
    ↓         ↓
  完成     争议 → 仲裁
```

**状态流转**：`Created → PaidOrCommitted → Released / Disputed → Closed`

### Sell 流程（NEX → USDT）

```
用户 create_sell_order ──→ 用户 NEX 锁定到托管
         │
做市商链下转 USDT ──→ mark_sell_complete（提交 TRC20 tx hash）
         │
OCW 验证 TRC20 ──→ confirm_verification
         │
    ┌────┴────┐
    ↓         ↓
 NEX → 做市商  验证失败 → 争议/退款
```

**状态流转**：`Pending → AwaitingVerification → Completed / VerificationFailed / Refunded`

### 首购流程

```
新买家 create_first_purchase ──→ 固定 $10 USD
         │
    免押金，KYC 验证
         │
    同 Buy 后续流程
```

---

## 数据结构

### BuyOrder（Buy 订单）

| 字段 | 类型 | 说明 |
|------|------|------|
| `maker_id` | `u64` | 做市商 ID |
| `maker` | `AccountId` | 做市商账户 |
| `taker` | `AccountId` | 买家账户 |
| `price` | `BalanceOf<T>` | 单价（USDT/NEX，精度 10^6） |
| `qty` | `BalanceOf<T>` | NEX 数量 |
| `amount` | `BalanceOf<T>` | USDT 总金额（精度 10^6） |
| `state` | `BuyOrderState` | 订单状态 |
| `buyer_deposit` | `BalanceOf<T>` | 买家押金 |
| `deposit_status` | `DepositStatus` | 押金状态 |
| `is_first_purchase` | `bool` | 是否首购 |

### SellOrder（Sell 订单）

| 字段 | 类型 | 说明 |
|------|------|------|
| `sell_id` | `u64` | 订单 ID |
| `maker_id` | `u64` | 做市商 ID |
| `user` | `AccountId` | 卖家账户 |
| `nex_amount` | `BalanceOf<T>` | NEX 数量 |
| `usdt_amount` | `u64` | USDT 金额（精度 10^6） |
| `usdt_address` | `TronAddress` | USDT 接收地址 |
| `status` | `SellOrderStatus` | 订单状态 |
| `trc20_tx_hash` | `Option<BoundedVec>` | TRC20 交易哈希 |

### 状态枚举

**BuyOrderState**: `Created` | `PaidOrCommitted` | `Released` | `Refunded` | `Canceled` | `Disputed` | `Closed` | `Expired`

**SellOrderStatus**: `Pending` | `AwaitingVerification` | `Completed` | `VerificationFailed` | `UserReported` | `Arbitrating` | `ArbitrationApproved` | `ArbitrationRejected` | `Refunded` | `SeverelyDisputed`

**DepositStatus**: `None` | `Locked` | `Released` | `Forfeited` | `PartiallyForfeited`

---

## 存储项

### Buy-side

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextBuyOrderId` | `u64` | 下一个 Buy 订单 ID |
| `BuyOrders` | `Map<u64, BuyOrder>` | Buy 订单记录 |
| `BuyerOrderList` | `Map<AccountId, BoundedVec<u64>>` | 买家订单列表 |
| `MakerBuyOrderList` | `Map<u64, BoundedVec<u64>>` | 做市商 Buy 订单列表 |
| `BuyDisputes` | `Map<u64, BuyDispute>` | Buy 争议记录 |
| `HasFirstPurchased` | `Map<AccountId, bool>` | 是否已首购 |
| `MakerFirstPurchaseCount` | `Map<u64, u32>` | 做市商当前首购订单计数 |
| `BuyTronTxUsed` | `Map<H256, BlockNumber>` | TRON 交易哈希防重放 |
| `BuyerCompletedOrderCount` | `Map<AccountId, u32>` | 买家完成订单计数 |
| `TotalDepositPoolBalance` | `BalanceOf<T>` | 押金池总余额 |
| `BuyArchiveCursor` | `u64` | Buy 归档游标 |
| `BuyExpiryCursor` | `u64` | Buy 过期检查游标 |

### Sell-side

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `NextSellOrderId` | `u64` | 下一个 Sell 订单 ID |
| `SellOrders` | `Map<u64, SellOrder>` | Sell 订单记录 |
| `UserSellList` | `Map<AccountId, BoundedVec<u64>>` | 用户 Sell 订单列表 |
| `MakerSellList` | `Map<u64, BoundedVec<u64>>` | 做市商 Sell 订单列表 |
| `SellUsedTxHashes` | `Map<BoundedVec, BlockNumber>` | Sell TxHash 防重放 |
| `SellPendingVerifications` | `Map<u64, SellVerificationRequest>` | 待验证请求 |
| `SellOcwVerificationResults` | `Map<u64, (bool, Option<BoundedVec>)>` | OCW 验证结果 |
| `SellArchiveCursor` | `u64` | Sell 归档游标 |

### 共享

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `KycConfigStore` | `KycConfig` | KYC 配置 |
| `KycExemptAccounts` | `Map<AccountId, ()>` | KYC 豁免账户 |

---

## Extrinsics（可调用函数）

### Buy-side 用户调用

| 函数 | 说明 |
|------|------|
| `create_buy_order` | 创建 Buy 订单（买家购买 NEX） |
| `create_first_purchase` | 创建首购订单（固定 $10 USD） |
| `mark_paid` | 标记已付款（可附 TRON tx hash） |
| `release_nex` | 做市商释放 NEX 给买家 |
| `cancel_buy_order` | 取消 Buy 订单 |
| `dispute_buy_order` | 发起 Buy 争议 |

### Sell-side 用户调用

| 函数 | 说明 |
|------|------|
| `create_sell_order` | 创建 Sell 订单（用户出售 NEX） |
| `mark_sell_complete` | 做市商提交 TRC20 tx hash |
| `report_sell` | 用户举报 Sell 订单 |
| `file_sell_dispute` | 用户发起 Sell 争议 |
| `claim_sell_verification_reward` | 领取验证奖励 |
| `user_accept_partial_usdt` | 用户接受部分 USDT |
| `user_request_usdt_refund` | 用户请求 USDT 退款 |
| `maker_confirm_usdt_refund` | 做市商确认退款 |

### 治理/系统调用

| 函数 | 说明 |
|------|------|
| `update_kyc_config` | 更新 KYC 配置（需 CommitteeOrigin） |
| `update_kyc_min_judgment` | 更新 KYC 最低等级 |
| `add_kyc_exempt` | 添加 KYC 豁免账户 |
| `remove_kyc_exempt` | 移除 KYC 豁免账户 |
| `ocw_submit_sell_verification` | OCW 提交验证结果（unsigned） |
| `handle_sell_verification_timeout` | 处理 Sell 验证超时 |

---

## 配置参数

### Buy-side 常量

| 参数 | 类型 | Runtime 值 | 说明 |
|------|------|-----------|------|
| `BuyOrderTimeout` | `u64` | 3,600,000 ms（1h） | Buy 订单超时时间 |
| `EvidenceWindow` | `u64` | 86,400,000 ms（24h） | 证据窗口 |
| `FirstPurchaseUsdValue` | `u128` | 10,000,000（$10） | 首购 USD 价值 |
| `MinFirstPurchaseCosAmount` | `Balance` | 1 NEX | 首购最小 NEX |
| `MaxFirstPurchaseCosAmount` | `Balance` | 100M NEX | 首购最大 NEX |
| `MaxOrderUsdAmount` | `u64` | 200,000,000（$200） | 最大订单金额 |
| `MinOrderUsdAmount` | `u64` | 20,000,000（$20） | 最小订单金额 |
| `DepositRate` | `u16` | 1000（10%） | 押金比例 |
| `CancelPenaltyRate` | `u16` | 3000（30%） | 取消惩罚比例 |
| `MinMakerDepositUsd` | `u64` | 500,000,000（$500） | 做市商最低押金 |
| `DisputeResponseTimeout` | `u64` | 86,400 s（24h） | 争议响应超时 |
| `DisputeArbitrationTimeout` | `u64` | 172,800 s（48h） | 仲裁超时 |

### Sell-side 常量

| 参数 | 类型 | Runtime 值 | 说明 |
|------|------|-----------|------|
| `SellTimeoutBlocks` | `BlockNumber` | 1 HOURS | Sell 超时 |
| `VerificationTimeoutBlocks` | `BlockNumber` | 2 HOURS | 验证超时 |
| `MinSellAmount` | `Balance` | 10 NEX | 最小 Sell 金额 |
| `TxHashTtlBlocks` | `BlockNumber` | 30 DAYS | TxHash TTL |
| `VerificationReward` | `Balance` | 0.1 NEX | 验证奖励 |
| `SellFeeRateBps` | `u32` | 10（0.1%） | Sell 手续费率 |
| `MinSellFee` | `Balance` | 0.1 NEX | 最低手续费 |

---

## Hooks

### on_initialize

每 100 个区块执行一次 Buy 订单过期检查，使用持久化游标 `BuyExpiryCursor`，每次最多扫描 50 个订单、处理 10 个过期订单。

### on_idle

空闲时自动执行：
1. **Buy 归档**：每次最多 5 个终态订单
2. **Sell 归档**：每次最多 5 个终态订单
3. **Buy TxHash 清理**：每次最多清理 10 个过期 TxHash

---

## 依赖模块

| 模块 | 接口 | 用途 |
|------|------|------|
| `pallet-escrow` | `Escrow` trait | NEX 托管锁定/释放/退还 |
| `pallet-trading-credit` | `BuyerCreditInterface` + `BuyerQuotaInterface` | 买家信用和额度管理 |
| `pallet-trading-common` | `PricingProvider` + `MakerInterface` + `MakerCreditInterface` | 定价、做市商、信用接口 |
| `pallet-trading-trc20-verifier` | `verify_trc20_transaction` | TRC20 交易验证（Sell 侧 OCW） |
| `pallet-storage-service` | `CidLockManager` | 争议证据 CID 锁定 |
| `pallet-arbitration` | `Decision` | 仲裁裁决执行 |

---

## 安全考虑

1. **托管保护**：所有 NEX 通过 `pallet-escrow` 托管，双方无法直接操作
2. **KYC 验证**：Buy 侧可选 KYC 要求，防止未认证用户交易
3. **押金机制**：基于信用的买家押金，超时/取消按比例扣除
4. **TxHash 防重放**：Buy/Sell 双方向 TRON 交易哈希防重放
5. **仲裁桥接**：争议通过 `pallet-arbitration` 统一处理
6. **自动过期**：Buy 订单超时自动退还托管资金
7. **手续费**：Sell 侧收取 0.1% 手续费（最低 0.1 NEX）

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v0.1.0 | 2026-02-08 | 初始骨架（合并 OTC + Swap） |
| v1.0.0 | 2026-02-08 | Phase 1+2+3 完整实现，40 单元测试 |
| v1.0.1 | 2026-02-08 | 审计修复：仲裁语义、归档游标、过期扫描、TxHash TTL |

---

## License

Unlicense
