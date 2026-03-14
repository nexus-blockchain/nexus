# 9 模块组合业务流深度分析与全量测试方案

- 日期：2026-03-14（基于 2026-03-13 审计结论更新）
- 范围模块：
  - `pallet-commission-single-line`（15 extrinsics）
  - `pallet-commission-pool-reward`（18 extrinsics）
  - `pallet-commission-multi-level`（15 extrinsics）
  - `pallet-entity-shop`（22 extrinsics）
  - `pallet-entity-member`（31 extrinsics）
  - `pallet-entity-loyalty`（11 extrinsics）
  - `pallet-entity-product`（10 extrinsics）
  - `pallet-entity-order`（25 extrinsics，链上 pallet 名 `entityTransaction`）
  - `pallet-nex-market`（36 extrinsics）
- 辅助模块（非独立审计但参与组合流）：
  - `pallet-commission-core`（27 extrinsics，含 2 个 DISABLED）

---

## 目录

1. [当前覆盖现状](#1-当前覆盖现状)
2. [模块间依赖关系图](#2-模块间依赖关系图)
3. [全量组合业务流清单](#3-全量组合业务流清单)
4. [按模块拆分的 extrinsic 覆盖矩阵](#4-按模块拆分的-extrinsic-覆盖矩阵)
5. [测试方案分层规划](#5-测试方案分层规划)
6. [优先级与执行路线图](#6-优先级与执行路线图)

---

## 1. 当前覆盖现状

### 1.1 已实现 / 已执行的 7 条代表性业务流

| # | 流名称 | 来源 | 涉及模块 | 类型 |
|---|--------|------|----------|------|
| 1 | entity-commerce-commission-flow | e2e/suites/ | registry, shop, member, product, order, commission-core, single-line, multi-level, pool-reward, loyalty | 大组合主流 |
| 2 | nex-market-smoke | e2e/suites/ | nex-market | 单模块 smoke |
| 3 | nex-market-trade-flow | remote/ | nex-market | 单模块正向流 |
| 4 | entity-shop-flow | remote/ | shop | 单模块生命周期 |
| 5 | entity-member-loyalty-flow | remote/ | member, loyalty | 双模块组合 |
| 6 | entity-product-order-physical-flow | remote/ | product, order | 双模块组合 |
| 7 | commission-admin-controls | remote/ | single-line, multi-level, pool-reward | 控制面组合 |

### 1.2 已覆盖 extrinsic 统计

| 模块 | 总 extrinsics | 已覆盖 | 覆盖率 |
|------|--------------|--------|--------|
| pallet-commission-single-line | 15 | 5 | 33% |
| pallet-commission-pool-reward | 18 | 4 | 22% |
| pallet-commission-multi-level | 15 | 6 | 40% |
| pallet-entity-shop | 22 | 8 | 36% |
| pallet-entity-member | 31 | 7 | 23% |
| pallet-entity-loyalty | 11 | 5 | 45% |
| pallet-entity-product | 10 | 4 | 40% |
| pallet-entity-order | 25 | 8 | 32% |
| pallet-nex-market | 36 | 6 | 17% |
| **合计** | **183** | **53** | **29%** |

### 1.3 覆盖盲区分类

| 分类 | 未覆盖 extrinsic 数 | 典型代表 |
|------|---------------------|----------|
| 异常 / 争议 / 超时流 | ~28 | dispute_trade, process_timeout, force_refund, reject_refund |
| 治理 / 强制处置流 | ~25 | force_close_shop, ban_shop, force_pause_market, force_reset_single_line |
| 批量 / 清理 / 索引维护 | ~18 | cleanup_buyer_orders, batch_delete_products, force_cleanup_entity, continue_cleanup |
| 完整生命周期分支 | ~16 | close_shop→finalize, cancel_close_shop, leave_entity, transfer_shop |
| 配置 / 参数管理 | ~15 | set_level_based_levels, update_multi_level_params, set_points_ttl, set_points_max_supply |
| OCW / 链下工作者 | ~8 | submit_ocw_result, auto_confirm_payment, submit_underpaid_update, finalize_underpaid |

---

## 2. 模块间依赖关系图

```
                    ┌─────────────────┐
                    │  nex-market     │
                    │ (价格预言机 +   │
                    │  独立交易流)    │
                    └───────┬─────────┘
                            │ PricingProvider
                            ▼
┌──────────┐    ┌──────────────────┐    ┌────────────────┐
│ registry │───▶│    product       │───▶│    order        │
│ (实体)   │    │ (商品管理)       │    │ (entityTx)     │
└────┬─────┘    └──────────────────┘    └──┬──────┬──────┘
     │                                     │      │
     │ EntityProvider                      │      │ OnOrderCompleted / OnOrderCancelled (Hook)
     ▼                                     │      ▼
┌──────────┐    ┌──────────────────┐    ┌──────────────────────────────────┐
│   shop   │◀──▶│    member        │    │       commission-core            │
│ (店铺)   │    │ (会员/推荐链)    │    │  ┌─────────┬──────────┬────────┐ │
└────┬─────┘    └───────┬──────────┘    │  │single   │multi     │pool    │ │
     │                  │               │  │-line    │-level    │-reward │ │
     │ PointsCleanup    │ MemberProvider│  └─────────┴──────────┴────────┘ │
     ▼                  ▼               └──────────────┬───────────────────┘
┌──────────────────────────┐                           │ LoyaltyWritePort
│       loyalty            │◀──────────────────────────┘
│ (积分 + 购物余额)        │
└──────────────────────────┘
```

**关键依赖路径：**
- `order` → Hook → `commission-core` → plugins (single-line / multi-level / pool-reward) + `loyalty`（购物余额）
- `order` → Hook → `member`（更新 spent / order_count / auto-upgrade）
- `order` → Hook → `shop`（更新 total_sales / total_orders）
- `product` ← `PricingProvider` ← `nex-market`（商品价格计算）
- `shop` → `PointsCleanup` → `loyalty`（关店清理积分）
- `commission-core` → `LoyaltyWritePort` / `LoyaltyTokenWritePort` → `loyalty`（购物余额读写）

---

## 3. 全量组合业务流清单

### 分类说明

- **P0 — 核心正向流**：用户可感知的主要业务路径，必须优先覆盖
- **P1 — 异常 / 争议 / 超时流**：错误处理与纠纷解决，生产环境高频
- **P2 — 治理 / 强制处置流**：Root / Admin 干预，合规与安全保障
- **P3 — 批量 / 清理 / 维护流**：索引维护、存储清理、系统健康
- **P4 — 配置 / 参数管理流**：业务参数调整，运营日常

---

### 3.1 跨模块组合业务流（P0）

#### BF-001：实体商业全链路（数字商品 + 分佣 + 忠诚度）
> 已有覆盖：entity-commerce-commission-flow ✅

**前置**：entity 创建 → shop 创建 → member 注册 → commission 配置

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | createEntity | registry |
| 2 | fundOperating | shop |
| 3 | setMemberPolicy | member |
| 4 | registerMember × 3（含推荐链） | member |
| 5 | activateMember × 3 | member |
| 6 | setCommissionRate + setCommissionModes + enableCommission | commission-core |
| 7 | setWithdrawalConfig | commission-core |
| 8 | setSingleLineConfig | single-line |
| 9 | setMultiLevelConfig | multi-level |
| 10 | setPoolRewardConfig | pool-reward |
| 11 | createProduct（数字，会员专属） | product |
| 12 | publishProduct | product |
| 13 | placeOrder × 3（自动完成触发分佣） | order |
| 14 | withdrawCommission（50% 购物余额） | commission-core |
| 15 | placeOrder（使用购物余额抵扣） | order |
| 16 | claimPoolReward | pool-reward |

**验证点**：分佣金额正确、购物余额增减正确、pool 扣减正确、单线收入正确、多级收入正确

---

#### BF-002：实物商品全生命周期（下单 → 发货 → 确认 → 分佣结算）
> 已有部分覆盖 ✅，需补充分佣验证

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | createProduct（Physical, 有库存） | product |
| 2 | publishProduct | product |
| 3 | placeOrder（实物，含运费地址） | order |
| 4 | updateShippingAddress | order |
| 5 | shipOrder（含物流单号） | order |
| 6 | updateTracking | order |
| 7 | extendConfirmTimeout | order |
| 8 | confirmReceipt | order |
| 9 | **验证**：Hook 触发 → commission 分佣 + member spent 更新 + shop stats 更新 | commission-core, member, shop |
| 10 | withdrawCommission | commission-core |

---

#### BF-003：服务类订单全生命周期
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | createProduct（Service 类型） | product |
| 2 | publishProduct | product |
| 3 | placeOrder | order |
| 4 | startService | order |
| 5 | completeService | order |
| 6 | confirmService | order |
| 7 | **验证**：Hook 触发分佣 + 会员升级规则检查 | commission-core, member |

---

#### BF-004：代付订单流（第三方付款）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeOrderFor（payer ≠ buyer） | order |
| 2 | shipOrder | order |
| 3 | confirmReceipt | order |
| 4 | **验证**：payer 扣款、buyer 会员 spent 更新、PayerOrders 索引 | order, member |
| 5 | cleanupPayerOrders | order |

---

#### BF-005：Token 支付订单 + Token 分佣 + Token 购物余额
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeOrder（use_tokens=true） | order |
| 2 | confirmReceipt / 自动完成 | order |
| 3 | **验证**：Token 分佣记录 + Token 购物余额 credited | commission-core, loyalty |
| 4 | withdrawTokenCommission | commission-core |
| 5 | placeOrder（use_shopping_balance=true, Token） | order |

---

#### BF-006：NEX 市场卖单完整交易流
> 已有覆盖：nex-market-trade-flow ✅

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeSellOrder | nex-market |
| 2 | reserveSellOrder | nex-market |
| 3 | confirmPayment | nex-market |
| 4 | sellerConfirmReceived | nex-market |

---

#### BF-007：NEX 市场买单成交流
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeBuyOrder | nex-market |
| 2 | acceptBuyOrder（卖方接受） | nex-market |
| 3 | confirmPayment | nex-market |
| 4 | sellerConfirmReceived 或 OCW 自动确认 | nex-market |
| 5 | **验证**：trade 状态 Completed、NEX 释放、deposit 退还、TWAP 更新 | nex-market |

---

#### BF-008：会员注册 → 升级规则触发 → 等级变动（消费驱动）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | initLevelSystem（use_custom=true, AutoUpgrade） | member |
| 2 | addCustomLevel × N（含 threshold） | member |
| 3 | initUpgradeRuleSystem | member |
| 4 | addUpgradeRule（trigger=OrderSpent, target_level） | member |
| 5 | placeOrder + confirmReceipt（消费达到 threshold） | order |
| 6 | **验证**：Hook → member spent 更新 → upgrade_rule 评估 → level 自动提升 | member |
| 7 | **验证**：升级后 commission 等级差异分佣生效 | commission-core |

---

#### BF-009：店铺关闭全链路（grace period + 积分清理 + 资金退回）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | closeShop（进入 Closing grace period） | shop |
| 2 | **等待** grace period 到期 | — |
| 3 | finalizeCloseShop（清理 + 退款） | shop |
| 4 | **验证**：PointsCleanup 调用 loyalty 清理积分 | loyalty |
| 5 | **验证**：商品全部下架 / 删除 | product |
| 6 | **验证**：资金退回 entity owner | shop |

---

#### BF-010：店铺转让流（跨实体）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | requestTransferShop（source entity owner） | shop |
| 2 | acceptTransferShop（target entity owner, keep_managers?） | shop |
| 3 | **验证**：ShopEntity 索引更新、manager 清理（可选）、主店校验 | shop |

**异常分支**：
- cancelTransferShop（source 或 target 取消）
- 有活跃订单时拒绝转让

---

### 3.2 单模块 / 争议 / 异常流（P1）

#### BF-011：订单退款争议全链路
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeOrder + shipOrder | order |
| 2 | requestRefund（buyer 发起） | order |
| 3a | approveRefund（seller 同意） → Refunded | order |
| **或** | | |
| 3b | rejectRefund（seller 拒绝） | order |
| 4b | **等待** DisputeTimeout 到期 → 自动退款 | order |
| **或** | | |
| 3c | withdrawDispute（buyer 撤回争议） | order |

---

#### BF-012：订单取消流（buyer / seller / 强制）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1a | cancelOrder（buyer 取消 Paid 状态非数字商品） | order |
| 1b | sellerCancelOrder（seller 主动取消 + reason） | order |
| 1c | sellerRefundOrder（seller 主动退款已发货订单） | order |
| 2 | **验证**：资金退回、Hook 触发 OnOrderCancelled | order, commission-core |

---

#### BF-013：管理员强制订单处置
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | forceRefund（Root 强制退款） | order |
| 2 | forceComplete（Root 强制完成） | order |
| 3 | forcePartialRefund（Root 部分退款，1-9999 bps） | order |
| 4 | forceProcessExpirations（Root 手动处理超时队列） | order |

---

#### BF-014：NEX 市场超时与争议流
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeSellOrder → reserveSellOrder → **不付款** | nex-market |
| 2 | processTimeout（AwaitingPayment 超时） → 退还 NEX + 没收 deposit | nex-market |
| **或** | | |
| 3 | confirmPayment → **OCW 验证失败** → Refunded | nex-market |
| 4 | disputeTrade（buyer 对 Refunded 状态发起争议 + evidence_cid） | nex-market |
| 5 | submitCounterEvidence（counterparty 反证据） | nex-market |
| 6 | resolveDispute（MarketAdmin 裁决：ReleaseToBuyer / RefundToSeller） | nex-market |

---

#### BF-015：NEX 市场少付流（Underpaid）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeSellOrder → reserveSellOrder → confirmPayment | nex-market |
| 2 | submitOcwResult（verification=Underpaid, 50%-99.5%） | nex-market |
| 3 | submitUnderpaidUpdate（OCW 追踪新 TRON tx, 累加金额） | nex-market |
| 4a | 累加达到 99.5%+ → 自动结算 | nex-market |
| **或** | | |
| 4b | grace period 到期 → finalizeUnderpaid（pro-rata NEX 释放 + deposit 部分没收） | nex-market |

---

#### BF-016：NEX 市场 OCW 自动确认支付
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeSellOrder → reserveSellOrder → **buyer 忘记 confirmPayment** | nex-market |
| 2 | autoConfirmPayment（OCW 检测到 TRON 转账自动确认） | nex-market |
| 3 | sellerConfirmReceived 或 OCW 验证 → Completed | nex-market |

---

#### BF-017：NEX 市场改单流
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | placeSellOrder / placeBuyOrder | nex-market |
| 2 | updateOrderPrice（原子改价，无需撤单重挂） | nex-market |
| 3 | updateOrderAmount（改量，buy order 需重算 deposit） | nex-market |
| 4 | **验证**：无 active trades 时才允许改单 | nex-market |

---

#### BF-018：会员审批 + 批量处理流
> 已有部分覆盖（batchApprove ✅），需补充 reject / cleanup

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setMemberPolicy（APPROVAL_REQUIRED=4） | member |
| 2 | registerMember × N（进入 Pending 状态） | member |
| 3a | batchApproveMembers（批量通过） | member |
| 3b | batchRejectMembers（批量拒绝） | member |
| 3c | cancelPendingMember（用户自撤申请） | member |
| 4 | cleanupExpiredPending（公开清理过期申请） | member |

---

#### BF-019：会员封禁 / 解封 / 移除 / 离开
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | banMember（封禁 + reason） | member |
| 2 | **验证**：被封禁会员无法下单 / 提佣 | order, commission-core |
| 3 | unbanMember（解封） | member |
| 4 | deactivateMember（停用，不封禁） | member |
| 5 | activateMember（重新激活） | member |
| 6 | removeMember（移除，需无下线） | member |
| 7 | leaveEntity（会员主动离开，需无下线） | member |

---

#### BF-020：积分完整生命周期（TTL + 过期 + 销毁 + 上限）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | enablePoints | loyalty |
| 2 | setPointsTtl（设置过期时间） | loyalty |
| 3 | setPointsMaxSupply（设置上限） | loyalty |
| 4 | managerIssuePoints（发放积分） | loyalty |
| 5 | **等待 TTL 到期** | — |
| 6 | expirePoints（触发过期） | loyalty |
| 7 | managerBurnPoints（管理员销毁） | loyalty |
| 8 | disablePoints（关闭积分系统） | loyalty |
| 9 | continueCleanup（分批清理） | loyalty |

---

### 3.3 治理 / 强制处置流（P2）

#### BF-021：店铺治理全链路
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | forcePauseShop（Root 强制暂停） | shop |
| 2 | **验证**：暂停后无法接单 | order |
| 3 | forceCloseShop（Root 强制关闭，无 grace period） | shop |
| 4 | banShop（Root 封禁 + 商品全下架） | shop |
| 5 | unbanShop（Root 解封 + 恢复原状态） | shop |
| 6 | setShopType（变更店铺类型） | shop |

---

#### BF-022：分佣系统治理与紧急控制
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | forceDisableEntityCommission（Root 禁用分佣） | commission-core |
| 2 | forceGlobalPause（Root 全局暂停提佣） | commission-core |
| 3 | pauseWithdrawals（entity 级暂停提佣） | commission-core |
| 4 | forceResetSingleLine（Root 批量重置单线） | single-line |
| 5 | forceRemoveFromSingleLine（Root 逻辑移除成员） | single-line |
| 6 | forceRestoreToSingleLine（Root 恢复成员） | single-line |
| 7 | forcePauseMultiLevel / forceResumeMultiLevel | multi-level |
| 8 | forceCleanupEntity（Root 清理已删除实体的多级存储） | multi-level |
| 9 | setGlobalPoolRewardPaused（Root 全局暂停 pool reward） | pool-reward |
| 10 | forcePausePoolReward / forceResumePoolReward | pool-reward |
| 11 | correctTokenPoolDeficit（Root 修正 Token pool 缺口） | pool-reward |

---

#### BF-023：NEX 市场管理员干预
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | forcePauseMarket（紧急暂停交易） | nex-market |
| 2 | forceResumeMarket（恢复交易） | nex-market |
| 3 | forceSettleTrade（手动结算，OCW 宕机时） | nex-market |
| 4 | forceCancelTrade（无没收取消交易） | nex-market |
| 5 | batchForceSettle（批量结算，最多 20 笔） | nex-market |
| 6 | batchForceCancel（批量取消，最多 20 笔） | nex-market |
| 7 | banUser / unbanUser（用户黑名单 + 自动撤单） | nex-market |
| 8 | setTradingFee（设置交易手续费，0-1000 bps） | nex-market |
| 9 | setOcwAuthorities（配置 OCW 签名白名单） | nex-market |

---

#### BF-024：NEX 市场价格保护与熔断
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | configurePriceProtection（启用 TWAP 偏差检查） | nex-market |
| 2 | setInitialPrice（冷启动价格） | nex-market |
| 3 | placeSellOrder（价格偏离 TWAP 超过阈值）→ 触发 CircuitBreaker | nex-market |
| 4 | **验证**：熔断期间新单被拒 | nex-market |
| 5 | liftCircuitBreaker（冷却期后解除） | nex-market |

---

#### BF-025：商品治理（强制下架 / 强制删除）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | forceUnpublishProduct（Root 强制下架 + reason） | product |
| 2 | forceDeleteProduct（Root 强制删除任何状态商品） | product |
| 3 | **验证**：deposit 退还、CID unpin | product |

---

### 3.4 批量 / 清理 / 维护流（P3）

#### BF-026：订单索引清理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | 创建多个订单并完成 / 取消 | order |
| 2 | cleanupBuyerOrders（清理终态订单索引） | order |
| 3 | cleanupShopOrders（清理店铺终态订单索引） | order |
| 4 | cleanupPayerOrders（清理代付终态订单索引） | order |
| 5 | archiveOrderRecords（归档分佣记录） | commission-core |

---

#### BF-027：商品批量操作
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | createProduct × N | product |
| 2 | batchPublishProducts | product |
| 3 | batchUnpublishProducts | product |
| 4 | batchDeleteProducts（验证 deposit 退还 × N） | product |

---

#### BF-028：分佣延迟生效配置
> 已有部分覆盖（schedule ✅），需补充 apply / cancel

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | scheduleConfigChange（single-line） | single-line |
| 2 | **等待 ConfigChangeDelay** | — |
| 3a | applyPendingConfig | single-line |
| **或** | | |
| 3b | cancelPendingConfig | single-line |
| 4 | 同理 multi-level: scheduleConfigChange → apply/cancel | multi-level |
| 5 | 同理 pool-reward: schedulePoolRewardConfigChange → apply/cancel | pool-reward |

---

#### BF-029：会员等级系统完整管理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | initLevelSystem | member |
| 2 | addCustomLevel × N | member |
| 3 | updateCustomLevel | member |
| 4 | setUseCustomLevels（切换） | member |
| 5 | setUpgradeMode（AutoUpgrade / ManualUpgrade） | member |
| 6 | manualSetMemberLevel（手动调级） | member |
| 7 | removeCustomLevel（从末尾移除，需无成员） | member |
| 8 | resetLevelSystem（需所有成员 level=0） | member |

---

#### BF-030：会员升级规则系统完整管理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | initUpgradeRuleSystem（ConflictStrategy） | member |
| 2 | addUpgradeRule（OrderSpent / ReferralCount trigger） | member |
| 3 | updateUpgradeRule（enable/disable, priority） | member |
| 4 | removeUpgradeRule | member |
| 5 | setUpgradeRuleSystemEnabled（开关） | member |
| 6 | setConflictStrategy（HighestLevel / HighestPriority / LongestDuration / FirstMatch） | member |
| 7 | resetUpgradeRuleSystem | member |

---

### 3.5 配置 / 参数管理流（P4）

#### BF-031：分佣提现配置与冷却期
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setWithdrawalConfig（FullWithdrawal / FixedRate / LevelBased / MemberChoice） | commission-core |
| 2 | setTokenWithdrawalConfig（Token 独立配置） | commission-core |
| 3 | setWithdrawalCooldown（设置冷却期） | commission-core |
| 4 | setMinWithdrawalInterval（设置最小提现间隔） | commission-core |
| 5 | setCreatorRewardRate（创造者奖励比例） | commission-core |
| 6 | withdrawCommission → 触发冷却期 → 再次提现被拒 | commission-core |

---

#### BF-032：全局费率管理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setPlatformFeeRate（NEX 平台手续费） | order |
| 2 | setTokenPlatformFeeRate（Token 平台手续费） | commission-core |
| 3 | setGlobalMinRepurchaseRate（最低回购率） | commission-core |
| 4 | setGlobalMinTokenRepurchaseRate | commission-core |
| 5 | setGlobalMaxCommissionRate（最高分佣率） | commission-core |
| 6 | setGlobalMaxTokenCommissionRate | commission-core |

---

#### BF-033：单线收入高级配置
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setSingleLineConfig | single-line |
| 2 | setLevelBasedLevels（等级自定义上下线层级） | single-line |
| 3 | removeLevelBasedLevels | single-line |
| 4 | clearSingleLineConfig | single-line |
| 5 | **验证**：等级差异化层级在分佣时生效 | single-line, commission-core |

---

#### BF-034：多级分佣高级配置（tier 增删改）
> 已有部分覆盖（addTier / removeTier ✅）

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setMultiLevelConfig（初始 2 tier） | multi-level |
| 2 | addTier（插入新层级） | multi-level |
| 3 | updateMultiLevelParams（部分更新 max_total_rate / 单个 tier） | multi-level |
| 4 | removeTier | multi-level |
| 5 | clearMultiLevelConfig | multi-level |

---

#### BF-035：Pool Reward 完整管理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setPoolRewardConfig（level_ratios, round_duration） | pool-reward |
| 2 | setTokenPoolEnabled（启用 Token pool） | pool-reward |
| 3 | claimPoolReward（用户领取） | pool-reward |
| 4 | startNewRound（手动开启新轮次） | pool-reward |
| 5 | clearPoolRewardConfig | pool-reward |
| 6 | forceClearPoolRewardConfig（Root 全量清理，含用户记录） | pool-reward |

---

#### BF-036：NEX 市场流动性与参数管理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | fundSeedAccount（Root 从 treasury 注入种子资金） | nex-market |
| 2 | seedLiquidity（Root 创建免押金卖单） | nex-market |
| 3 | updateDepositExchangeRate（动态调整 deposit 计算汇率） | nex-market |
| 4 | setSeedTronAddress（更新种子 TRON 地址） | nex-market |
| 5 | claimVerificationReward（OCW 验证奖励领取） | nex-market |

---

#### BF-037：实体资金提取（NEX + Token）
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | withdrawEntityFunds（available = free - pending - shopping_total - pool_locked） | commission-core |
| 2 | withdrawEntityTokenFunds（Token 版本，含外部转入检测） | commission-core |
| 3 | **验证**：CommissionFundGuard 保护、购物余额不可提取 | commission-core, loyalty |

---

#### BF-038：店铺暂停 / 恢复 / 资金耗尽自动暂停
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | pauseShop（owner/manager 暂停） | shop |
| 2 | **验证**：暂停后商品不可售 | product, order |
| 3 | resumeShop（需余额 ≥ MinOperatingBalance） | shop |
| 4 | withdrawOperatingFund（提取至余额不足） → 触发 FundDepleted | shop |
| 5 | fundOperating（充值恢复 → 自动从 FundDepleted 恢复为 Active） | shop |

---

#### BF-039：店铺关闭取消流
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | closeShop（进入 Closing） | shop |
| 2 | cancelCloseShop（取消关闭，根据余额恢复 Active / FundDepleted） | shop |

---

#### BF-040：Manager 自辞流
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | addManager（owner 添加） | shop |
| 2 | resignManager（manager 自行辞职） | shop |
| 3 | **验证**：辞职后不再有管理权限 | shop |

---

#### BF-041：会员补绑推荐人
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | registerMember（不带 referrer） | member |
| 2 | bindReferrer（后续补绑） | member |
| 3 | **验证**：推荐链建立、team_size 更新、环形推荐检测 | member |

---

#### BF-042：会员策略管理
> **完全未覆盖** ❌

| 步骤 | extrinsic | 模块 |
|------|-----------|------|
| 1 | setMemberPolicy（policy_bits: 0=开放, 1=消费, 2=推荐, 4=审批） | member |
| 2 | setMemberStatsPolicy（推荐统计策略） | member |
| 3 | **验证**：不同 policy 下注册行为差异 | member |

---

## 4. 按模块拆分的 extrinsic 覆盖矩阵

### 4.1 pallet-commission-single-line（15 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| set_single_line_config | ✅ | BF-001, BF-007 admin |
| clear_single_line_config | ❌ | BF-033 |
| update_single_line_params | ✅ | BF-007 admin |
| set_level_based_levels | ❌ | BF-033 |
| remove_level_based_levels | ❌ | BF-033 |
| force_set_single_line_config | ❌ | BF-022 |
| force_clear_single_line_config | ❌ | BF-022 |
| force_reset_single_line | ❌ | BF-022 |
| pause_single_line | ✅ | BF-007 admin |
| resume_single_line | ✅ | BF-007 admin |
| schedule_config_change | ✅ | BF-028 |
| apply_pending_config | ❌ | BF-028 |
| cancel_pending_config | ❌ | BF-028 |
| force_remove_from_single_line | ❌ | BF-022 |
| force_restore_to_single_line | ❌ | BF-022 |

### 4.2 pallet-commission-pool-reward（18 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| set_pool_reward_config | ✅ | BF-001 |
| claim_pool_reward | ✅ | BF-001 |
| start_new_round | ❌ | BF-035 |
| set_token_pool_enabled | ❌ | BF-035 |
| force_set_pool_reward_config | ❌ | BF-022 |
| force_set_token_pool_enabled | ❌ | BF-022 |
| force_start_new_round | ❌ | BF-022 |
| clear_pool_reward_config | ❌ | BF-035 |
| force_clear_pool_reward_config | ❌ | BF-022 |
| pause_pool_reward | ✅ | BF-007 admin |
| resume_pool_reward | ✅ | BF-007 admin |
| set_global_pool_reward_paused | ❌ | BF-022 |
| force_pause_pool_reward | ❌ | BF-022 |
| force_resume_pool_reward | ❌ | BF-022 |
| schedule_pool_reward_config_change | ✅ | BF-028 |
| apply_pending_pool_reward_config | ❌ | BF-028 |
| cancel_pending_pool_reward_config | ❌ | BF-028 |
| correct_token_pool_deficit | ❌ | BF-022 |

### 4.3 pallet-commission-multi-level（15 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| set_multi_level_config | ✅ | BF-001 |
| clear_multi_level_config | ❌ | BF-034 |
| force_set_multi_level_config | ❌ | BF-022 |
| force_clear_multi_level_config | ❌ | BF-022 |
| update_multi_level_params | ❌ | BF-034 |
| add_tier | ✅ | BF-034 |
| remove_tier | ✅ | BF-034 |
| pause_multi_level | ✅ | BF-007 admin |
| resume_multi_level | ✅ | BF-007 admin |
| schedule_config_change | ✅ | BF-028 |
| apply_pending_config | ❌ | BF-028 |
| cancel_pending_config | ❌ | BF-028 |
| force_pause_multi_level | ❌ | BF-022 |
| force_resume_multi_level | ❌ | BF-022 |
| force_cleanup_entity | ❌ | BF-022 |

### 4.4 pallet-entity-shop（22 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| create_shop | ✅ | BF-001 |
| update_shop | ✅ | BF-004 shop |
| add_manager | ✅ | BF-004 shop |
| remove_manager | ✅ | BF-004 shop |
| fund_operating | ✅ | BF-001, BF-038 |
| pause_shop | ❌ | BF-038 |
| resume_shop | ❌ | BF-038 |
| set_location | ❌ | BF-038 |
| close_shop | ❌ | BF-009 |
| finalize_close_shop | ❌ | BF-009 |
| withdraw_operating_fund | ✅ | BF-004 shop |
| request_transfer_shop | ❌ | BF-010 |
| accept_transfer_shop | ❌ | BF-010 |
| cancel_transfer_shop | ❌ | BF-010 |
| set_primary_shop | ✅ | BF-004 shop |
| force_pause_shop | ❌ | BF-021 |
| force_close_shop | ❌ | BF-021 |
| set_shop_type | ❌ | BF-021 |
| cancel_close_shop | ❌ | BF-039 |
| resign_manager | ❌ | BF-040 |
| ban_shop | ❌ | BF-021 |
| unban_shop | ❌ | BF-021 |

### 4.5 pallet-entity-member（31 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| register_member | ✅ | BF-001 |
| bind_referrer | ❌ | BF-041 |
| init_level_system | ❌ | BF-029 |
| add_custom_level | ❌ | BF-029 |
| update_custom_level | ❌ | BF-029 |
| remove_custom_level | ❌ | BF-029 |
| manual_set_member_level | ❌ | BF-029 |
| set_use_custom_levels | ❌ | BF-029 |
| set_upgrade_mode | ❌ | BF-029 |
| init_upgrade_rule_system | ❌ | BF-030 |
| add_upgrade_rule | ❌ | BF-030 |
| update_upgrade_rule | ❌ | BF-030 |
| remove_upgrade_rule | ❌ | BF-030 |
| set_upgrade_rule_system_enabled | ❌ | BF-030 |
| set_conflict_strategy | ❌ | BF-030 |
| set_member_policy | ✅ | BF-001 |
| approve_member | ✅ | (via batch) |
| reject_member | ❌ | BF-018 |
| set_member_stats_policy | ❌ | BF-042 |
| cancel_pending_member | ❌ | BF-018 |
| cleanup_expired_pending | ❌ | BF-018 |
| batch_approve_members | ✅ | BF-005 member |
| batch_reject_members | ❌ | BF-018 |
| ban_member | ❌ | BF-019 |
| unban_member | ❌ | BF-019 |
| remove_member | ❌ | BF-019 |
| reset_level_system | ❌ | BF-029 |
| reset_upgrade_rule_system | ❌ | BF-030 |
| leave_entity | ❌ | BF-019 |
| activate_member | ✅ | BF-001 |
| deactivate_member | ❌ | BF-019 |

### 4.6 pallet-entity-loyalty（11 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| enable_points | ✅ | BF-005 member |
| disable_points | ❌ | BF-020 |
| update_points_config | ✅ | BF-005 member |
| transfer_points | ✅ | BF-005 member |
| manager_issue_points | ✅ | BF-005 member |
| manager_burn_points | ❌ | BF-020 |
| redeem_points | ✅ | BF-005 member |
| set_points_ttl | ❌ | BF-020 |
| expire_points | ❌ | BF-020 |
| set_points_max_supply | ❌ | BF-020 |
| continue_cleanup | ❌ | BF-020 |

### 4.7 pallet-entity-product（10 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| create_product | ✅ | BF-001, BF-002 |
| update_product | ✅ | BF-006 product |
| publish_product | ✅ | BF-001, BF-002 |
| unpublish_product | ✅ | BF-006 product |
| delete_product | ❌ | BF-027 |
| force_unpublish_product | ❌ | BF-025 |
| batch_publish_products | ❌ | BF-027 |
| batch_unpublish_products | ❌ | BF-027 |
| batch_delete_products | ❌ | BF-027 |
| force_delete_product | ❌ | BF-025 |

### 4.8 pallet-entity-order / entityTransaction（25 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| place_order | ✅ | BF-001, BF-002 |
| cancel_order | ❌ | BF-012 |
| ship_order | ✅ | BF-002 |
| confirm_receipt | ✅ | BF-002 |
| request_refund | ✅ | BF-006 product |
| approve_refund | ✅ | BF-006 product |
| start_service | ❌ | BF-003 |
| complete_service | ❌ | BF-003 |
| confirm_service | ❌ | BF-003 |
| set_platform_fee_rate | ❌ | BF-032 |
| cleanup_buyer_orders | ❌ | BF-026 |
| reject_refund | ❌ | BF-011 |
| seller_cancel_order | ❌ | BF-012 |
| force_refund | ❌ | BF-013 |
| force_complete | ❌ | BF-013 |
| update_shipping_address | ✅ | BF-002 |
| extend_confirm_timeout | ✅ | BF-002 |
| cleanup_shop_orders | ❌ | BF-026 |
| update_tracking | ✅ | BF-002 |
| seller_refund_order | ❌ | BF-012 |
| force_partial_refund | ❌ | BF-013 |
| withdraw_dispute | ❌ | BF-011 |
| force_process_expirations | ❌ | BF-013 |
| place_order_for | ❌ | BF-004 |
| cleanup_payer_orders | ❌ | BF-026 |

### 4.9 pallet-nex-market（36 extrinsics）

| extrinsic | 已覆盖 | 归属测试流 |
|-----------|--------|-----------|
| place_sell_order | ✅ | BF-006, smoke |
| place_buy_order | ✅ | smoke |
| cancel_order | ✅ | smoke |
| reserve_sell_order | ✅ | BF-006 |
| accept_buy_order | ❌ | BF-007 |
| confirm_payment | ✅ | BF-006 |
| process_timeout | ❌ | BF-014 |
| submit_ocw_result | ❌ | BF-015 |
| claim_verification_reward | ❌ | BF-036 |
| configure_price_protection | ❌ | BF-024 |
| set_initial_price | ❌ | BF-024 |
| lift_circuit_breaker | ❌ | BF-024 |
| fund_seed_account | ❌ | BF-036 |
| seed_liquidity | ❌ | BF-036 |
| auto_confirm_payment | ❌ | BF-016 |
| submit_underpaid_update | ❌ | BF-015 |
| finalize_underpaid | ❌ | BF-015 |
| force_pause_market | ❌ | BF-023 |
| force_resume_market | ❌ | BF-023 |
| force_settle_trade | ❌ | BF-023 |
| force_cancel_trade | ❌ | BF-023 |
| dispute_trade | ❌ | BF-014 |
| resolve_dispute | ❌ | BF-014 |
| set_trading_fee | ❌ | BF-023 |
| update_order_price | ❌ | BF-017 |
| update_deposit_exchange_rate | ❌ | BF-036 |
| seller_confirm_received | ✅ | BF-006 |
| ban_user | ❌ | BF-023 |
| unban_user | ❌ | BF-023 |
| submit_counter_evidence | ❌ | BF-014 |
| update_order_amount | ❌ | BF-017 |
| batch_force_settle | ❌ | BF-023 |
| batch_force_cancel | ❌ | BF-023 |
| set_ocw_authorities | ❌ | BF-023 |
| set_seed_tron_address | ❌ | BF-036 |

---

## 5. 测试方案分层规划

### 5.1 测试层次定义

| 层次 | 名称 | 说明 | 执行方式 |
|------|------|------|----------|
| L1 | 单模块正向流 | 单个模块的 happy path 完整走通 | 本地 / 远端 e2e |
| L2 | 跨模块组合流 | 2+ 模块协作的核心业务路径 | 远端 e2e |
| L3 | 异常 / 争议流 | 错误处理、超时、退款、争议解决 | 远端 e2e + 时间推进 |
| L4 | 治理 / 强制处置 | Root / Admin 紧急干预 | 远端 e2e（需 sudo） |
| L5 | 批量 / 清理 / 维护 | 存储清理、索引维护、批量操作 | 远端 e2e |
| L6 | 压力 / 边界 | 容量上限、并发、溢出保护 | 专项脚本 |

### 5.2 测试套件规划

#### Suite 1：核心组合流（L2, P0）— 新增 6 个 case

| Case ID | 名称 | 对应 BF | 新增 extrinsic 覆盖 |
|---------|------|---------|---------------------|
| S1-01 | 服务类订单全链路 | BF-003 | startService, completeService, confirmService |
| S1-02 | 代付订单流 | BF-004 | placeOrderFor, cleanupPayerOrders |
| S1-03 | Token 支付 + Token 分佣 | BF-005 | withdrawTokenCommission |
| S1-04 | 买单成交流 | BF-007 | acceptBuyOrder |
| S1-05 | 消费驱动等级升级 | BF-008 | initLevelSystem, addCustomLevel, initUpgradeRuleSystem, addUpgradeRule |
| S1-06 | 店铺关闭全链路 | BF-009 | closeShop, finalizeCloseShop |

#### Suite 2：争议与异常（L3, P1）— 新增 8 个 case

| Case ID | 名称 | 对应 BF | 新增 extrinsic 覆盖 |
|---------|------|---------|---------------------|
| S2-01 | 订单退款争议（approve + reject + withdraw） | BF-011 | rejectRefund, withdrawDispute |
| S2-02 | 订单取消（buyer + seller + sellerRefund） | BF-012 | cancelOrder, sellerCancelOrder, sellerRefundOrder |
| S2-03 | NEX 市场超时与争议 | BF-014 | processTimeout, disputeTrade, submitCounterEvidence, resolveDispute |
| S2-04 | NEX 市场少付流 | BF-015 | submitOcwResult, submitUnderpaidUpdate, finalizeUnderpaid |
| S2-05 | NEX 市场 OCW 自动确认 | BF-016 | autoConfirmPayment |
| S2-06 | NEX 市场改单 | BF-017 | updateOrderPrice, updateOrderAmount |
| S2-07 | 会员审批批量处理 | BF-018 | batchRejectMembers, cancelPendingMember, cleanupExpiredPending |
| S2-08 | 会员封禁 / 解封 / 移除 / 离开 | BF-019 | banMember, unbanMember, deactivateMember, removeMember, leaveEntity |

#### Suite 3：治理与强制处置（L4, P2）— 新增 5 个 case

| Case ID | 名称 | 对应 BF | 新增 extrinsic 覆盖 |
|---------|------|---------|---------------------|
| S3-01 | 店铺治理全链路 | BF-021 | forcePauseShop, forceCloseShop, banShop, unbanShop, setShopType |
| S3-02 | 分佣系统治理 | BF-022 | forceDisableEntityCommission, forceGlobalPause, forceResetSingleLine, forceRemoveFromSingleLine, forceRestoreToSingleLine, forcePauseMultiLevel, forceResumeMultiLevel, forceCleanupEntity, setGlobalPoolRewardPaused, correctTokenPoolDeficit |
| S3-03 | NEX 市场管理员干预 | BF-023 | forcePauseMarket, forceResumeMarket, forceSettleTrade, forceCancelTrade, batchForceSettle, batchForceCancel, banUser, unbanUser, setTradingFee, setOcwAuthorities |
| S3-04 | 价格保护与熔断 | BF-024 | configurePriceProtection, setInitialPrice, liftCircuitBreaker |
| S3-05 | 订单管理员强制处置 | BF-013 | forceRefund, forceComplete, forcePartialRefund, forceProcessExpirations |

#### Suite 4：批量与清理（L5, P3）— 新增 5 个 case

| Case ID | 名称 | 对应 BF | 新增 extrinsic 覆盖 |
|---------|------|---------|---------------------|
| S4-01 | 订单索引清理 | BF-026 | cleanupBuyerOrders, cleanupShopOrders, archiveOrderRecords |
| S4-02 | 商品批量操作 | BF-027 | deleteProduct, batchPublishProducts, batchUnpublishProducts, batchDeleteProducts, forceUnpublishProduct, forceDeleteProduct |
| S4-03 | 分佣延迟生效 | BF-028 | applyPendingConfig(×3), cancelPendingConfig(×3) |
| S4-04 | 积分完整生命周期 | BF-020 | disablePoints, managerBurnPoints, setPointsTtl, expirePoints, setPointsMaxSupply, continueCleanup |
| S4-05 | Pool Reward 完整管理 | BF-035 | startNewRound, setTokenPoolEnabled, clearPoolRewardConfig, forceClearPoolRewardConfig |

#### Suite 5：配置管理（L5, P4）— 新增 8 个 case

| Case ID | 名称 | 对应 BF | 新增 extrinsic 覆盖 |
|---------|------|---------|---------------------|
| S5-01 | 分佣提现配置与冷却期 | BF-031 | setTokenWithdrawalConfig, setWithdrawalCooldown, setMinWithdrawalInterval, setCreatorRewardRate |
| S5-02 | 全局费率管理 | BF-032 | setPlatformFeeRate, setTokenPlatformFeeRate, setGlobalMinRepurchaseRate, setGlobalMaxCommissionRate |
| S5-03 | 单线高级配置 | BF-033 | setLevelBasedLevels, removeLevelBasedLevels, clearSingleLineConfig |
| S5-04 | 多级 tier 管理 | BF-034 | updateMultiLevelParams, clearMultiLevelConfig |
| S5-05 | 会员等级系统 | BF-029 | initLevelSystem, addCustomLevel, updateCustomLevel, removeCustomLevel, manualSetMemberLevel, setUseCustomLevels, setUpgradeMode, resetLevelSystem |
| S5-06 | 会员升级规则系统 | BF-030 | initUpgradeRuleSystem, addUpgradeRule, updateUpgradeRule, removeUpgradeRule, setUpgradeRuleSystemEnabled, setConflictStrategy, resetUpgradeRuleSystem |
| S5-07 | 市场流动性管理 | BF-036 | fundSeedAccount, seedLiquidity, updateDepositExchangeRate, setSeedTronAddress, claimVerificationReward |
| S5-08 | 店铺扩展操作 | BF-010, 038-042 | requestTransferShop, acceptTransferShop, cancelTransferShop, pauseShop, resumeShop, cancelCloseShop, resignManager, setLocation, bindReferrer, setMemberPolicy, setMemberStatsPolicy |

---

## 6. 优先级与执行路线图

### 6.1 执行顺序

| 阶段 | Suite | 新增 case 数 | 新覆盖 extrinsic 数 | 覆盖率提升 |
|------|-------|-------------|---------------------|-----------|
| **Phase 1** | Suite 1（核心组合流） | 6 | +15 | 29% → 37% |
| **Phase 2** | Suite 2（争议与异常） | 8 | +22 | 37% → 49% |
| **Phase 3** | Suite 3（治理与强制处置） | 5 | +30 | 49% → 65% |
| **Phase 4** | Suite 4（批量与清理） | 5 | +20 | 65% → 76% |
| **Phase 5** | Suite 5（配置管理） | 8 | +43 | 76% → 100% |
| **合计** | — | **32** | **+130** | **29% → 100%** |

### 6.2 技术前置条件

| 条件 | 影响范围 | 当前状态 |
|------|----------|----------|
| 远端节点需支持 `sudo` 调用 | Suite 3, 4 部分 | 需确认 |
| 需要模拟区块时间推进（用于超时测试） | BF-011, 014, 015, 020, 024, 028 | 需 `system.setStorage` 或等待 |
| OCW 签名密钥对配置 | BF-015, 016 | 需 `setOcwAuthorities` + 密钥 |
| TRON 网络模拟 / mock（少付场景） | BF-015 | 可用链上 mock 或跳过 TRON 验证 |
| EntityToken pallet 可用 | BF-005 | 需确认 runtime 开启 |

### 6.3 验证点矩阵（跨模块集成验证）

| 集成点 | 源模块 | 目标模块 | 验证内容 |
|--------|--------|----------|----------|
| OnOrderCompleted Hook | order | commission-core | 分佣金额正确、commission records 创建 |
| OnOrderCompleted Hook | order | member | spent 更新、order_count 递增、auto-upgrade 触发 |
| OnOrderCompleted Hook | order | shop | total_sales 递增、total_orders 递增 |
| OnOrderCompleted Hook | order | loyalty | shopping balance 变动（如有） |
| OnOrderCancelled Hook | order | commission-core | pending commission 回退 |
| LoyaltyWritePort | commission-core | loyalty | 购物余额增减（提佣时 repurchase split） |
| LoyaltyTokenWritePort | commission-core | loyalty | Token 购物余额增减 |
| PricingProvider | product | nex-market | 商品 deposit 计算依赖 TWAP 价格 |
| PointsCleanup | shop | loyalty | 关店时清理积分 |
| CommissionFundGuard | shop, loyalty | commission-core | 提现 / 兑积分不可触碰被保护资金 |
| MemberProvider | order, commission-core | member | 会员资格、推荐人查询、auto-register |
| EntityProvider | 所有模块 | registry | 实体存在性、锁定状态、活跃状态 |

---

## 附录：术语表

| 术语 | 含义 |
|------|------|
| extrinsic | Substrate 链上可调用函数（类比智能合约方法） |
| Hook | 订单完成 / 取消后的回调接口（OnOrderCompleted / OnOrderCancelled） |
| Port | 跨 pallet 查询 / 写入接口（如 LoyaltyWritePort, AssetLedgerPort） |
| TWAP | 时间加权平均价格（Time-Weighted Average Price） |
| OCW | 链下工作者（Off-Chain Worker），用于 TRON 支付验证 |
| bps | 基点（basis points），10000 = 100% |
| grace period | 宽限期（如店铺关闭宽限期、少付追踪期） |
| deposit | NEX 保证金（买方交易保证金，防止恶意占单） |
