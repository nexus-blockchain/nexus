# Trading 模块集群

## 模块概述

Nexus 交易系统采用模块化设计，包含以下子模块：

| 模块 | 路径 | 版本 | 说明 | Runtime Index |
|------|------|------|------|:---:|
| **pallet-nex-market** | `nex-market/` | v0.1.0 | NEX/USDT 无做市商 P2P 订单簿 + 买家信用系统 | 56 |
| **pallet-trading-common** | `common/` | v0.8.1 | 公共 Trait + 工具库（PricingProvider, PriceOracle, ExchangeRateProvider, DepositCalculator） | — |
| **pallet-trading-trc20-verifier** | `trc20-verifier/` | v0.4.0 | TRC20 USDT 链下验证共享库 | — |

> **历史变更**：`pallet-trading-pricing`、`pallet-trading-credit`、`pallet-trading-maker`、`pallet-trading-p2p` 已于 2026-02-23 废弃删除，由 `pallet-nex-market` 替代。做市商模式已移除，所有用户均可自由挂单/吃单。

---

## 架构设计

### 模块拓扑

```text
┌──────────────────────────────────────────────────────────────────┐
│                          Runtime                                │
│                                                                  │
│  TradingPricingProvider ───→ 1h TWAP → LastTrade → InitialPrice │
│  EntityPricingProvider  ───→ TradingPricingProvider + 陈旧检测    │
│  NexExchangeRateProvider ──→ 汇率 + 置信度评估 (0-100)          │
└──────┬───────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│                    pallet-nex-market (56)                    │
│                                                              │
│  订单簿 · USDT 交易(OCW) · TWAP 预言机 · 价格保护/熔断      │
│  买家保证金 · 信用系统 · 多档判定 · 补付窗口 · seed_liquidity │
│  争议仲裁 · 用户封禁 · 批量管理 · 过期 GC                    │
└──────┬──────────────────────────────┬───────────────────────┘
       │                              │
       ▼                              ▼
  pallet-trading-common         pallet-trading-trc20-verifier
  (Trait + 共享类型 + 工具)       (TronGrid API 验证)
```

### 跨模块依赖

```text
pallet-nex-market
  ├── Currency (原生 NEX 锁定/转账)
  ├── OCW → pallet-trading-trc20-verifier (TRC20 验证)
  ├── DepositCalculator → pallet-trading-common (保证金 USDT→NEX 实时换算)
  └── 实现 PriceOracle trait（TWAP + LastTradePrice + 陈旧检测）

pallet-trading-common（被外部模块使用）
  ├── pallet-dispute-arbitration      → PricingProvider (投诉押金换算)
  ├── pallet-storage-service  → PricingProvider + DepositCalculator (保证金)
  ├── pallet-entity-registry  → PricingProvider (开店初始资金)
  ├── pallet-entity-market    → PricingProvider (商品押金)
  └── pallet-entity-product   → PricingProvider (服务押金)

Runtime 适配器（mod.rs 中实现）:
  TradingPricingProvider   → 优先 1h TWAP → LastTradePrice → initial_price
  EntityPricingProvider    → 委托 TradingPricingProvider + is_price_stale(2400 blocks ≈ 4h)
  NexExchangeRateProvider  → 汇率 + 置信度评估 (0-100)
```

---

## pallet-nex-market（核心交易模块）

无做市商的 NEX/USDT P2P 订单簿。任何人可挂单/吃单，USDT 通过 TRC20 链下支付，OCW 自动验证。

### 核心特性

| 特性 | 说明 |
|------|------|
| **订单簿** | 限价卖单 + 限价买单，支持部分成交、原子改价、改量、最低成交量 |
| **USDT 通道** | TRC20 链下支付 + OCW 三阶段自动验证 + 自动结算 |
| **买家信用系统** | 0-1000 信用分，保证金折扣、并发控制、自动暂停/永久封禁 |
| **多档判定** | Exact / Overpaid / Underpaid / SeverelyUnderpaid / Invalid |
| **补付窗口** | 少付 50%~99.5% 给予 2h 补付时间，避免网络延迟误判 |
| **TWAP 预言机** | 1h / 24h / 7d 三周期时间加权平均价，`on_idle` 每区块推进 |
| **价格保护** | 挂单/吃单偏离检查 + 7d TWAP 熔断机制 |
| **争议仲裁** | 双方举证 + 争议窗口锚定终态时间 + 管理员裁决 |
| **seed_liquidity** | 瀑布式定价 + 溢价 + 四层防御，冷启动引流 |

### 交易流程

#### 卖 NEX（卖家锁 NEX，收 USDT）

```text
卖家                      买家                     OCW
  │                        │                        │
  │ place_sell_order ──→ 锁 NEX                     │
  │           reserve_sell_order ──→ 锁买家保证金    │
  │                 链下转 USDT → confirm_payment    │
  │                        │           submit_ocw_result (unsigned)
  │               claim_verification_reward         │
  │                 ← 释放 NEX + 退保证金           │
```

#### 买 NEX（买家挂单，卖家接单）

```text
买家                      卖家                     OCW
  │                        │                        │
  │ place_buy_order ──→ 锁买家保证金                │
  │               accept_buy_order ──→ 锁卖家 NEX   │
  │ 链下转 USDT → confirm_payment                   │
  │                        │           submit_ocw_result
  │               claim_verification_reward         │
```

#### 少付补付流程（50% ~ 99.5%）

```text
OCW 检测到少付 → UnderpaidPending → 补付窗口(2h)
  ├─ 窗口内 OCW 持续扫描 → submit_underpaid_update（更新金额）
  │   └─ 累计 ≥ 99.5% → 升级为 Exact，正常结算
  └─ 窗口到期 → finalize_underpaid（终裁）
      └─ 按最终比例释放 NEX + 梯度没收保证金
```

### 买家信用系统

在保证金体系之上增加信用分 + 并发控制 + 保证金折扣 + 自动封禁。

#### 信用分规则

| 事件 | 变化 | 说明 |
|------|------|------|
| 交易完成（前 3 笔） | +50 | 快速提升新用户信用 |
| 交易完成（第 4 笔起） | +10 | 稳步提升 |
| 每 10 笔连续完成 | 额外 +5 | 连续成功奖励 |
| 超时违约 | -50 / -100 / -200 / -400 | 连续违约指数递增 |
| 少付违约 | -30 | 固定扣分（较轻） |
| 30 天无违约 | +10 | Lazy 自然恢复 |

#### 信用驱动机制

| 信用分 | 保证金折扣 | 并发交易上限 |
|--------|-----------|-------------|
| 900-1000 | 50% | 基于完成数 |
| 800-899 | 70% | 基于完成数 |
| 700-799 | 90% | 基于完成数 |
| <700 | 无折扣 | 基于完成数 |

| 累计完成数 | 并发交易上限 |
|-----------|-------------|
| 0-2 | 1 |
| 3-9 | 2 |
| 10-49 | 3 |
| 50+ | 5 |

#### 自动处罚

- **连续 3 次违约** → 暂停 7 天（解除需信用分 ≥ 500）
- **信用分 = 0** → 永久封禁
- **信用分 < 500** → 无法发起交易

### 多档判定 & 保证金没收

#### 付款金额判定

| 实际 / 应付比例 | 判定结果 | NEX 释放 | 保证金 |
|----------------|----------|----------|--------|
| >= 100.5% | Overpaid | 全额释放 | 退还 |
| 99.5% ~ 100.5% | Exact | 全额释放 | 退还 |
| 50% ~ 99.5% | Underpaid | 进入补付窗口 | 待定 |
| < 50% | SeverelyUnderpaid | 按比例释放 | 没收 |
| = 0 | Invalid | 不释放 | 没收 |

#### 保证金梯度没收（补付终裁）

| 最终付款比例 | 没收比例 |
|-------------|---------|
| >= 99.5% | 0% |
| 95% ~ 99.5% | 20% |
| 80% ~ 95% | 50% |
| < 80% | 100% |

### Extrinsics（36 个，call_index 0-35）

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 0 | `place_sell_order` | 签名 | 挂卖单：锁 NEX，提供 TRON 收款地址 |
| 1 | `place_buy_order` | 签名 | 挂买单：预锁保证金，提供 TRON 付款地址 |
| 2 | `cancel_order` | Owner | 取消订单：退还锁定资产 |
| 3 | `reserve_sell_order` | 签名 | 买家吃卖单：创建 UsdtTrade |
| 4 | `accept_buy_order` | 签名 | 卖家接买单：锁 NEX，按比例分配保证金 |
| 5 | `confirm_payment` | 买家 | 确认付款 |
| 6 | `process_timeout` | 参与方/Admin | 分阶段超时处理 |
| 7 | `submit_ocw_result` | Unsigned+签名 | OCW 验证结果（含 authority 签名验证） |
| 8 | `claim_verification_reward` | 任何人 | 手动结算兜底 |
| 9 | `configure_price_protection` | MarketAdmin | 价格保护配置 |
| 10 | `set_initial_price` | MarketAdmin | 初始基准价格 |
| 11 | `lift_circuit_breaker` | MarketAdmin | 解除熔断 |
| 13 | `fund_seed_account` | MarketAdmin | 种子账户注资 |
| 14 | `seed_liquidity` | MarketAdmin | 批量挂免保证金卖单 |
| 15 | `auto_confirm_payment` | Unsigned+签名 | OCW 预检兜底 |
| 16 | `submit_underpaid_update` | Unsigned+签名 | 补付窗口内更新金额 |
| 17 | `finalize_underpaid` | 任何人 | 补付终裁 |
| 18 | `force_pause_market` | MarketAdmin | 紧急暂停 |
| 19 | `force_resume_market` | MarketAdmin | 恢复交易 |
| 20 | `force_settle_trade` | MarketAdmin | 强制结算 |
| 21 | `force_cancel_trade` | MarketAdmin | 强制取消 |
| 22 | `dispute_trade` | 参与方 | 发起争议 |
| 23 | `resolve_dispute` | MarketAdmin | 裁决争议 |
| 24 | `set_trading_fee` | MarketAdmin | 设置手续费率 |
| 25 | `update_order_price` | Owner | 修改价格 |
| 26 | ~~`update_deposit_exchange_rate`~~ | — | **[已废弃]** |
| 27 | `seller_confirm_received` | 卖家 | 手动确认收款 |
| 28 | `ban_user` | MarketAdmin | 封禁用户 |
| 29 | `unban_user` | MarketAdmin | 解封用户 |
| 30 | `submit_counter_evidence` | 对方 | 提交反驳证据 |
| 31 | `update_order_amount` | Owner | 修改数量 |
| 32 | `batch_force_settle` | MarketAdmin | 批量结算 |
| 33 | `batch_force_cancel` | MarketAdmin | 批量取消 |
| 34 | `set_ocw_authorities` | MarketAdmin | OCW 授权列表 |
| 35 | `set_seed_tron_address` | MarketAdmin | 种子 TRON 地址 |

### OCW 三阶段工作流

| 阶段 | 扫描队列 | 触发条件 | 动作 |
|------|---------|---------|------|
| 1. 正常验证 | `PendingUsdtTrades` | AwaitingVerification | `submit_ocw_result` |
| 2. 补付扫描 | `PendingUnderpaidTrades` | UnderpaidPending | `submit_underpaid_update` |
| 3. 预检兜底 | `AwaitingPaymentTrades` | AwaitingPayment 超 50% 超时 | `auto_confirm_payment` |

OCW extrinsics 采用 Authority 签名验证（`authority` + `signature` 参数），拒绝 `TransactionSource::External`，仅接受 Local/InBlock。

### 测试

```bash
cargo test -p pallet-nex-market    # 211 个单元测试
```

详见 [nex-market/README.md](nex-market/README.md)。

---

## pallet-trading-common（公共 Trait + 工具库）

纯 Rust crate（非 FRAME pallet，`no_std` 兼容），提供跨模块共享的 Trait 接口、共享类型和工具函数。

### Trait 接口

| Trait | 方法 | 说明 |
|-------|------|------|
| `PricingProvider<Balance>` | `get_nex_to_usd_rate()`, `report_p2p_trade()` | NEX/USD 底层汇率（精度 10^6） |
| `PriceOracle` | `get_twap()`, `get_last_trade_price()`, `is_price_stale()`, `get_trade_count()` | TWAP 预言机 |
| `ExchangeRateProvider` | `get_nex_usdt_rate()`, `price_confidence()`, `is_rate_reliable()` | 带置信度统一接口 |
| `DepositCalculator<Balance>` | `calculate_deposit(usd_amount, fallback)` | USD→NEX 保证金换算 |

### 共享类型

| 类型 | 说明 |
|------|------|
| `TronAddress` / `TronTxHash` / `TxHash` / `MomentOf` / `Cid` | 基础类型 |
| `UsdtTradeStatus` | 交易状态枚举（7 变体） |
| `BuyerDepositStatus` | 保证金状态枚举（5 变体） |
| `PaymentVerificationResult` | 多档判定（5 变体） |

### 工具模块

| 模块 | 说明 |
|------|------|
| `validation.rs` | TRON 地址 Base58Check 校验 |
| `mask.rs` | 数据脱敏（姓名/身份证/生日） |
| `time.rs` | 区块数 <-> 秒数转换 |

### 测试

```bash
cargo test -p pallet-trading-common    # 54 个单元测试 + 3 个 doc-tests
```

详见 [common/README.md](common/README.md)。

---

## pallet-trading-trc20-verifier（TRC20 验证库）

纯 Rust crate（`no_std` 兼容），供 OCW 在链下验证 USDT TRC20 转账。

- **TRC20 交易验证**：通过 TronGrid API 验证 USDT 转账
- **端点健康评分**：动态评估 API 端点，自动排序
- **多策略请求**：并行竞速 / 串行故障转移
- **tx_hash 防重放**：offchain 本地 + 链上双重过滤
- **全局 kill switch**：`enabled = false` 立即禁止所有验证
- **审计日志**：每次验证自动记录

### 测试

```bash
cargo test -p pallet-trading-trc20-verifier    # 190 个单元测试
```

详见 [trc20-verifier/README.md](trc20-verifier/README.md)。

---

## 价格服务架构

### 三层价格接口

```text
ExchangeRateProvider    ← 高级聚合：汇率 + 置信度 + 可信判断
  ├── PricingProvider   ← 底层汇率 + 喂价
  └── PriceOracle       ← TWAP 窗口 + 陈旧检测
        └── pallet-nex-market  ← 唯一数据源
```

### Runtime 适配器

| 适配器 | 接口 | 价格优先级 | 消费方 |
|--------|------|-----------|--------|
| `TradingPricingProvider` | `PricingProvider<Balance>` | 1h TWAP → LastTrade → initial_price | 全局底层 |
| `EntityPricingProvider` | `entity_common::PricingProvider` | 同上 + `is_price_stale(2400)` | entity-registry |
| `NexExchangeRateProvider` | `ExchangeRateProvider` | 同上 + 置信度评估 | 佣金/打赏 |

---

## 安全设计

### 交易安全

- **买家保证金**：通过实时市场价格换算 NEX 保证金，超时/少付梯度没收
- **信用系统**：连续违约自动暂停/永久封禁，高信用享保证金折扣
- **补付窗口**：少付 50%-99.5% 给予 2h 补付时间
- **Unsigned 安全**：Authority 签名验证 + 拒绝 External 来源，仅接受 Local/InBlock

### 价格安全

- **TWAP 预言机**：三周期时间加权平均价格
- **价格偏离检查**：挂单和吃单均校验
- **熔断机制**：偏离超阈值自动暂停
- **陈旧保护**：超过 4h 无交易使用保守兜底值

### seed_liquidity 四层防御

| 层级 | 机制 |
|------|------|
| L0 定价 | 瀑布式基准价 + 20% 溢价 |
| L1 资金隔离 | 独立种子账户 |
| L2 防 Grief | 100 NEX 上限 + 1 笔活跃 + 1h 短超时 |
| L3 防 Sybil | 完成首单后不再免保证金 |

### OCW 端点安全

- HTTPS 强制 + SSRF 防护
- API Key 精确匹配
- 端点健康评分 + 熔断隔离
- CAS 并发锁 + 令牌验证

---

## 代码统计

| 组件 | 类型 | 源文件 | 代码行数 | 测试数 |
|------|------|--------|---------|--------|
| pallet-trading-common | 库 | 7 | 1,522 | 54+3 |
| pallet-nex-market | FRAME Pallet | 7 | 11,420 | 211 |
| pallet-trading-trc20-verifier | 库 | 1 | 5,633 | 190 |
| **合计** | — | **15** | **18,575** | **458** |

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v2.6.0 | 2026-03-19 | 买家信用系统（信用分/并发控制/保证金折扣/自动暂停）、OCW Authority 签名验证 |
| v2.5.0 | 2026-03-19 | 保证金统一实时市场价格（移除 UsdtToNexRate/DepositExchangeRate） |
| v2.4.0 | 2026-02-26 | 补付窗口、OCW 预检、fund_seed_account |
| v2.3.0 | 2026-02-26 | 价格服务升级：TWAP 优先 + 陈旧保护 + ExchangeRateProvider |
| v2.2.0 | 2026-02-24 | 保护性瀑布式 seed 定价 + 独立种子账户 |
| v2.1.0 | 2026-02-24 | seed_liquidity 流动性注入 + 委员会治理 |
| v2.0.0 | 2026-02-23 | 架构重构：新增 pallet-nex-market 替代旧模块 |

---

**License**: Unlicense
