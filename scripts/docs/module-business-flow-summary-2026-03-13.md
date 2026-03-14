# 9 模块业务流测试方案摘要

- 日期：2026-03-14（基于审计结论重建）
- 配套详细文档：`docs/module-business-flow-mapping-2026-03-13.md`

---

## 一、审计范围

| 模块 | Pallet 名 | extrinsic 数 | 已覆盖 | 覆盖率 |
|------|-----------|-------------|--------|--------|
| 单线分佣 | pallet-commission-single-line | 15 | 5 | 33% |
| 奖金池 | pallet-commission-pool-reward | 18 | 4 | 22% |
| 多级分佣 | pallet-commission-multi-level | 15 | 6 | 40% |
| 店铺 | pallet-entity-shop | 22 | 8 | 36% |
| 会员 | pallet-entity-member | 31 | 7 | 23% |
| 忠诚度 | pallet-entity-loyalty | 11 | 5 | 45% |
| 商品 | pallet-entity-product | 10 | 4 | 40% |
| 订单 | pallet-entity-order (entityTransaction) | 25 | 8 | 32% |
| NEX 市场 | pallet-nex-market | 36 | 6 | 17% |
| **合计** | — | **183** | **53** | **29%** |

---

## 二、当前覆盖（7 条已执行流）

1. ✅ **entity-commerce-commission-flow** — 数字商品 + 全链路分佣 + 购物余额
2. ✅ **nex-market-smoke** — 挂单 / 撤单 smoke
3. ✅ **nex-market-trade-flow** — 卖单 → 锁定 → 确认付款 → 卖方确认
4. ✅ **entity-shop-flow** — 二级店、manager、主店切换、资金充提
5. ✅ **entity-member-loyalty-flow** — 审批注册 + 积分发放 / 转让 / 兑换
6. ✅ **entity-product-order-physical-flow** — 实物商品 + 发货 + 退款
7. ✅ **commission-admin-controls** — 分佣插件配置 + 暂停/恢复 + 延迟生效

---

## 三、全量业务流规划（42 条）

### P0 — 核心正向流（10 条）

| BF | 名称 | 状态 | 关键新覆盖 |
|----|------|------|-----------|
| 001 | 数字商品 + 分佣 + 忠诚度 | ✅ 已覆盖 | — |
| 002 | 实物商品全生命周期 + 分佣验证 | ⚠️ 需补分佣验证 | Hook 验证 |
| 003 | 服务类订单全链路 | ❌ | startService, completeService, confirmService |
| 004 | 代付订单流 | ❌ | placeOrderFor, cleanupPayerOrders |
| 005 | Token 支付 + Token 分佣 | ❌ | withdrawTokenCommission |
| 006 | NEX 市场卖单交易流 | ✅ 已覆盖 | — |
| 007 | NEX 市场买单成交流 | ❌ | acceptBuyOrder |
| 008 | 消费驱动等级升级 | ❌ | initLevelSystem, addUpgradeRule |
| 009 | 店铺关闭全链路 | ❌ | closeShop, finalizeCloseShop |
| 010 | 店铺转让流 | ❌ | requestTransferShop, acceptTransferShop |

### P1 — 异常 / 争议 / 超时流（9 条）

| BF | 名称 | 状态 | 关键新覆盖 |
|----|------|------|-----------|
| 011 | 订单退款争议全链路 | ❌ | rejectRefund, withdrawDispute |
| 012 | 订单取消流 | ❌ | cancelOrder, sellerCancelOrder, sellerRefundOrder |
| 013 | 管理员强制订单处置 | ❌ | forceRefund, forceComplete, forcePartialRefund |
| 014 | NEX 市场超时与争议 | ❌ | processTimeout, disputeTrade, resolveDispute |
| 015 | NEX 市场少付流 | ❌ | submitOcwResult, submitUnderpaidUpdate, finalizeUnderpaid |
| 016 | NEX 市场 OCW 自动确认 | ❌ | autoConfirmPayment |
| 017 | NEX 市场改单 | ❌ | updateOrderPrice, updateOrderAmount |
| 018 | 会员审批批量处理 | ⚠️ 部分 | batchRejectMembers, cleanupExpiredPending |
| 019 | 会员封禁 / 移除 / 离开 | ❌ | banMember, removeMember, leaveEntity |

### P2 — 治理 / 强制处置流（5 条）

| BF | 名称 | 状态 | 关键新覆盖 |
|----|------|------|-----------|
| 021 | 店铺治理全链路 | ❌ | forcePauseShop, banShop, unbanShop |
| 022 | 分佣系统治理 | ❌ | forceResetSingleLine, forceCleanupEntity, correctTokenPoolDeficit |
| 023 | NEX 市场管理员干预 | ❌ | forceSettleTrade, batchForceSettle, banUser |
| 024 | 价格保护与熔断 | ❌ | configurePriceProtection, liftCircuitBreaker |
| 025 | 商品治理 | ❌ | forceUnpublishProduct, forceDeleteProduct |

### P3 — 批量 / 清理 / 维护流（6 条）

| BF | 名称 | 状态 | 关键新覆盖 |
|----|------|------|-----------|
| 020 | 积分完整生命周期 | ❌ | disablePoints, expirePoints, continueCleanup |
| 026 | 订单索引清理 | ❌ | cleanupBuyerOrders, cleanupShopOrders |
| 027 | 商品批量操作 | ❌ | batchPublishProducts, batchDeleteProducts |
| 028 | 分佣延迟生效 | ⚠️ 需补 apply/cancel | applyPendingConfig × 3 |
| 035 | Pool Reward 完整管理 | ❌ | startNewRound, clearPoolRewardConfig |

### P4 — 配置 / 参数管理流（12 条）

| BF | 名称 | 状态 |
|----|------|------|
| 029 | 会员等级系统完整管理 | ❌ |
| 030 | 会员升级规则系统 | ❌ |
| 031 | 分佣提现配置与冷却期 | ❌ |
| 032 | 全局费率管理 | ❌ |
| 033 | 单线高级配置 | ❌ |
| 034 | 多级 tier 管理 | ⚠️ 部分 |
| 036 | NEX 市场流动性管理 | ❌ |
| 037 | 实体资金提取 | ❌ |
| 038 | 店铺暂停/恢复/资金耗尽 | ❌ |
| 039 | 店铺关闭取消 | ❌ |
| 040 | Manager 自辞 | ❌ |
| 041 | 会员补绑推荐人 | ❌ |
| 042 | 会员策略管理 | ❌ |

---

## 四、执行路线图

| 阶段 | 内容 | 新增 case | 新覆盖 extrinsic | 目标覆盖率 |
|------|------|----------|-----------------|-----------|
| Phase 1 | 核心组合流 | 6 | +15 | 37% |
| Phase 2 | 争议与异常 | 8 | +22 | 49% |
| Phase 3 | 治理与强制处置 | 5 | +30 | 65% |
| Phase 4 | 批量与清理 | 5 | +20 | 76% |
| Phase 5 | 配置管理 | 8 | +43 | 100% |
| **合计** | — | **32** | **+130** | **100%** |

---

## 五、关键集成验证点

| 集成点 | 源 → 目标 | 验证内容 |
|--------|-----------|----------|
| OnOrderCompleted Hook | order → commission-core | 分佣金额、commission records |
| OnOrderCompleted Hook | order → member | spent 更新、order_count、auto-upgrade |
| OnOrderCompleted Hook | order → shop | total_sales、total_orders |
| OnOrderCancelled Hook | order → commission-core | pending commission 回退 |
| LoyaltyWritePort | commission-core → loyalty | 购物余额增减 |
| PricingProvider | product → nex-market | 商品 deposit 计算 |
| PointsCleanup | shop → loyalty | 关店清理积分 |
| CommissionFundGuard | shop/loyalty → commission-core | 资金保护 |

---

## 六、口径声明

> 本文档是对 2026-03-13 审计结论（`module-business-flow-audit-final-2026-03-13.md`）的执行响应。
>
> 文档目标：**规划 9 个模块 183 个 extrinsic 的全量业务流测试方案**，将覆盖率从当前 29%（53/183）提升至 100%。
>
> 当前标记为 ✅ 的流仅代表"已有脚本执行过"，不代表所有验证点均已充分断言。后续实施时需同步加强断言覆盖。
