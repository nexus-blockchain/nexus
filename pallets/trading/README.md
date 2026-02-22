# Trading 模块集群

## 模块概述

Nexus 交易系统采用模块化设计，包含以下子模块：

| 模块 | 路径 | 说明 | Runtime Index |
|------|------|------|:---:|
| **pallet-trading-p2p** | `p2p/` | 统一 P2P 交易（Buy + Sell） | 55 |
| **pallet-trading-maker** | `maker/` | 做市商管理（申请、审核、押金、提现） | 52 |
| **pallet-trading-credit** | `credit/` | 做市商/买家信用分管理 | 51 |
| **pallet-trading-pricing** | `pricing/` | 实时定价与市场统计 | 50 |
| **pallet-trading-common** | `common/` | 公共类型与 Trait 定义 | — |
| **pallet-trading-trc20-verifier** | `trc20-verifier/` | TRC20 交易链下验证共享库 | — |

> **注意**: `pallet-trading-otc` 和 `pallet-trading-swap` 已于 2026-02-08 合并为 `pallet-trading-p2p`，源码已删除。

---

## 架构设计

```text
┌─────────────────────────────────────────────────┐
│                  Runtime (index)                │
│                                                 │
│  TradingPricing(50)  TradingCredit(51)          │
│  TradingMaker(52)    TradingP2p(55)             │
└────────┬────────────────────┬───────────────────┘
         │                    │
         ▼                    ▼
┌────────────────┐  ┌──────────────────────────┐
│ pallet-trading │  │    pallet-trading-p2p    │
│    -maker      │  │  Buy 方向 (原 OTC)       │
│  做市商管理     │◄─┤  Sell 方向 (原 Swap)     │
│  押金/提现      │  │  KYC / OCW 验证          │
│  溢价配置       │  │  仲裁桥接                │
└────────────────┘  │  归档 & L2 存储           │
                    └──────────┬───────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
     pallet-escrow    pallet-trading    pallet-trading
       托管服务         -credit           -pricing
                       信用分管理         实时定价

共享库（无 Runtime 实例）:
  pallet-trading-common       — Trait 定义 + 公共类型
  pallet-trading-trc20-verifier — TRC20 链下验证
```

### 模块依赖关系

```text
pallet-trading-p2p
  ├── T::MakerPallet  → pallet-trading-maker (MakerInterface)
  ├── T::Escrow       → pallet-escrow (锁定/释放资金)
  ├── T::MakerCredit  → pallet-trading-credit (MakerCreditInterface)
  ├── T::BuyerCredit  → pallet-trading-credit (BuyerCreditInterface)
  ├── T::Pricing      → pallet-trading-pricing (PricingProvider)
  ├── T::Identity     → IdentityVerificationProvider (KYC)
  └── OCW             → pallet-trading-trc20-verifier (TRC20 验证)
```

---

## pallet-trading-p2p（核心交易模块）

统一 Buy（USDT→NEX）和 Sell（NEX→USDT）两方向的 P2P 交易。

### Buy 方向（原 OTC）

买家用 USDT 购买 NEX，做市商 NEX 锁定到托管账户。

**流程：** `create_buy_order` → 买家付款 `mark_buy_paid` → 做市商释放 `release_nex` → 完成

**状态机 (`BuyOrderState`)：**
```
Created → PaidOrCommitted → Released (完成)
   │           │
   ▼           ▼
Canceled    Disputed → Refunded / Released
   │
   ▼
Expired
```

**主要 Extrinsics：**

| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `create_buy_order` | 买家创建购买订单，锁定做市商 NEX |
| 1 | `create_first_purchase` | 首购订单（固定 10 USD） |
| 2 | `mark_buy_paid` | 买家标记已付款 |
| 3 | `release_nex` | 做市商确认收款，释放 NEX |
| 4 | `cancel_buy_order` | 取消订单 |
| 5 | `dispute_buy_order` | 发起争议 |

### Sell 方向（原 Swap）

用户卖出 NEX 换取 USDT，做市商链下发送 USDT。

**流程：** `create_sell_order` → 做市商发送 USDT `submit_sell_tx_hash` → OCW 验证 → 完成

**状态机 (`SellOrderStatus`)：**
```
Pending → AwaitingVerification → Completed (完成)
   │           │
   ▼           ▼
Timeout    VerificationFailed → UserReported → Arbitrating
                                                  │
                                           Approved / Rejected
```

**主要 Extrinsics：**

| call_index | 方法 | 说明 |
|:---:|------|------|
| 6 | `create_sell_order` | 用户锁定 NEX，等待做市商发 USDT |
| 7 | `submit_sell_tx_hash` | 做市商提交 TRC20 交易哈希 |
| 8 | `report_sell_order` | 用户举报做市商未发送 USDT |
| 9 | `timeout_sell_order` | 超时自动退款 |

### 通用功能

| call_index | 方法 | 说明 |
|:---:|------|------|
| 10 | `set_kyc_config` | 管理员设置 KYC 要求 |
| 11 | `add_kyc_exempt` | 添加 KYC 豁免账户 |

### 仲裁桥接

P2P 模块通过 `apply_arbitration_decision(order_id, Decision)` 接收 `pallet-arbitration` 的裁决：
- **Release** — 放款给买家
- **Refund** — 退款给做市商
- **Partial { buyer_pct }** — 按比例分配

ArbitrationRouter 域映射：
- `OTC_ORDER` → `BuyOrders` 存储
- `SWAP` → `SellOrders` 存储

### 存储概览

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `BuyOrders` | `StorageMap<u64, BuyOrder>` | Buy 方向订单 |
| `SellOrders` | `StorageMap<u64, SellOrder>` | Sell 方向订单 |
| `NextBuyOrderId` | `StorageValue<u64>` | Buy 订单自增 ID |
| `NextSellOrderId` | `StorageValue<u64>` | Sell 订单自增 ID |
| `BuyerOrders` | `StorageMap<AccountId, Vec<u64>>` | 买家订单索引 |
| `MakerBuyOrders` | `StorageMap<u64, Vec<u64>>` | 做市商 Buy 订单索引 |
| `MakerSellOrders` | `StorageMap<u64, Vec<u64>>` | 做市商 Sell 订单索引 |
| `ArchivedOrdersL2Store` | `StorageMap<u64, ArchivedOrderL2>` | 归档订单（L2 摘要） |
| `KycConfigStore` | `StorageValue<KycConfig>` | KYC 配置 |
| `KycExemptAccounts` | `StorageMap<AccountId, ()>` | KYC 豁免账户 |
| `UsedTronTxHashes` | `StorageMap<BoundedVec, ()>` | TRC20 交易哈希防重放 |
| `PermanentStats` | `StorageValue<P2pPermanentStats>` | 平台统计 |

### 配置参数

```rust
impl pallet_trading_p2p::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Escrow = EscrowPallet;
    type MakerPallet = P2pMakerAdapter;
    type MakerCredit = P2pMakerCreditAdapter;
    type BuyerCredit = TradingCredit;
    type BuyerQuota = TradingCredit;
    type Pricing = TradingPricing;
    type Timestamp = Timestamp;
    type Identity = P2pIdentityProvider;

    // Buy 方向参数
    type BuyOrderTimeout = ConstU64<3_600_000>;           // 1 小时（毫秒）
    type BuyEvidenceWindow = ConstU64<86_400_000>;        // 24 小时
    type FirstPurchaseUsdAmount = ConstU64<10_000_000>;   // 10 USD
    type MinBuyUsdAmount = ConstU64<20_000_000>;          // 20 USD
    type MaxBuyUsdAmount = ConstU64<200_000_000>;         // 200 USD
    type MinFirstPurchaseNex = ConstU128<1_000_000_000_000>;
    type MaxFirstPurchaseNex = ConstU128<1_000_000_000_000_000>;
    type MaxFirstPurchasePerMaker = ConstU32<5>;
    type AmountTolerance = ConstU16<100>;                 // 1%

    // Sell 方向参数
    type SellTimeoutBlocks = ConstU32<14_400>;            // 1 天（区块数）
    type TxHashTtlBlocks = ConstU32<432_000>;             // 30 天
    type MinSellNex = ConstU128<100_000_000_000>;         // 100 NEX

    // 通用参数
    type DepositRate = ConstU32<500>;                     // 5% 押金率
    type PlatformFeeRate = ConstU32<30>;                  // 0.3% 手续费
    type MaxArchiveAge = ConstU32<100_800>;               // 7 天归档保留

    type WeightInfo = pallet_trading_p2p::weights::SubstrateWeight<Runtime>;
}
```

### 测试

```bash
cargo test -p pallet-trading-p2p    # 40 个单元测试
```

---

## pallet-trading-maker（做市商管理）

管理做市商生命周期：申请 → 审核 → 激活 → 提现/退出。

**主要 Extrinsics：**

| call_index | 方法 | 说明 |
|:---:|------|------|
| 0 | `lock_deposit` | 锁定押金，创建申请 |
| 1 | `submit_info` | 提交资料（脱敏后上链） |
| 2 | `approve_maker` | 治理审批 |
| 3 | `reject_maker` | 治理驳回 |
| 4 | `cancel_application` | 取消申请 |
| 5 | `pause_service` / `resume_service` | 暂停/恢复服务 |
| 6 | `request_withdrawal` | 申请提现 |
| 7 | `execute_withdrawal` | 执行提现（冷却期后） |

**数据脱敏规则（pallet-trading-common）：**
- 姓名：`"张三" → "×三"`，`"李四五" → "李×五"`
- 身份证：`"110101199001011234" → "1101**********1234"`
- 生日：`"1990-01-01" → "1990-xx-xx"`

---

## 前端调用示例

### Buy 订单（USDT → NEX）

```typescript
import { ApiPromise } from '@polkadot/api';

// 创建首购订单
const tx = api.tx.tradingP2p.createFirstPurchase(makerId, paymentCommit, contactCommit);
await tx.signAndSend(buyer);

// 买家标记已付款
await api.tx.tradingP2p.markBuyPaid(orderId, tronTxHash).signAndSend(buyer);

// 做市商释放 NEX
await api.tx.tradingP2p.releaseNex(orderId).signAndSend(maker);

// 查询 Buy 订单
const order = await api.query.tradingP2p.buyOrders(orderId);
```

### Sell 订单（NEX → USDT）

```typescript
// 用户锁定 NEX 发起卖出
const tx = api.tx.tradingP2p.createSellOrder(makerId, nexAmount, usdtAddress);
await tx.signAndSend(seller);

// 做市商提交 TRC20 交易哈希
await api.tx.tradingP2p.submitSellTxHash(sellId, trc20TxHash).signAndSend(maker);

// 查询 Sell 订单
const sell = await api.query.tradingP2p.sellOrders(sellId);
```

### 做市商管理

```typescript
// 锁定押金
await api.tx.tradingMaker.lockDeposit().signAndSend(account);

// 提交资料
await api.tx.tradingMaker.submitInfo(
  realName, idCard, birthday, tronAddress,
  publicCid, privateCid, direction,
  buyPremiumBps, sellPremiumBps, minAmount,
  wechatId, paymentMethodsJson
).signAndSend(account);

// 查询做市商
const maker = await api.query.tradingMaker.makerApplications(makerId);
```

---

## 安全设计

- **资金托管**：所有交易资金通过 `pallet-escrow` 锁定，非任一方可单方面操作
- **防重放**：`UsedTronTxHashes` 防止同一 TRC20 交易哈希重复使用
- **KYC 验证**：可配置强制 KYC，通过 `IdentityVerificationProvider` 对接身份系统
- **数据脱敏**：做市商敏感资料链上只存脱敏版本
- **OCW 验证**：Sell 方向 USDT 转账通过链下工作机自动验证 TRC20 交易
- **仲裁集成**：通过 `ArbitrationRouter` 支持第三方仲裁裁决

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v1.0.0 | 2026-02-08 | `pallet-trading-p2p` 合并 OTC + Swap，移除旧模块 |
| v0.3.0 | 2026-01-18 | `pallet-trading-swap` 重命名 bridge → swap |
| v0.2.0 | 2026-01-18 | `pallet-trading-swap` 移除官方桥接，仅保留做市商兑换 |
| v0.1.0 | 2025-11-03 | 拆分为 4 个子模块（maker/otc/swap/common） |

---

**License**: Unlicense
