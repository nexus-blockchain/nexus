# 远程业务流测试报告

- 日期：2026-03-13
- 节点：`wss://202.140.140.202`
- 输出目录：`remote-business-flows-20260313/`

## 1. 测试目标

按用户要求，针对以下模块在远程节点做**未被现有 E2E 覆盖**的业务流验证，并把结果放到单独目录：

- `pallet-commission-single-line`
- `pallet-commission-pool-reward`
- `pallet-commission-multi-level`
- `pallet-entity-shop`
- `pallet-entity-member`
- `pallet-entity-loyalty`
- `pallet-entity-product`
- `pallet-entity-order`（链上 pallet 名实际为 `entityTransaction`）
- `pallet-nex-market`

## 2. 测试环境

- Chain：`Nexus Development`
- Node：`Nexus Node 0.1.0-unknown`
- Runtime：`nexus v100`
- 执行时链头：best `2464` / finalized `2462`
- 实际可用 API：`@polkadot/api 16.5.4`（`/home/xiaodong/node_modules/@polkadot/api`）

关键说明：

1. 仓库本地 `scripts/node_modules/@polkadot/api` 为旧版 `12.x`，**不兼容**远程链 extrinsic version 5。
2. 因此本次远程写链验证改为：**逐 case 手工执行**，而不是直接依赖仓库旧 runner。
3. 最终结果以 `remote-business-flows-20260313/artifacts/*.json` 为准。

## 3. 已按要求跳过的既有流

这些流仓库已有 E2E 覆盖，本次不重复：

| 模块 | 已有流 | 现有 suite |
|---|---|---|
| `pallet-entity-shop` | 自动主店 + 基础运营资金充值 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-entity-member` | 注册 + 推荐链 + 激活会员 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-entity-loyalty` | 佣金提现到 shopping balance + 消费 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-entity-product` | 数字商品创建/发布 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-entity-order` | 数字商品即时完成订单 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-commission-single-line` | 订单驱动的单线分润 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-commission-multi-level` | 订单驱动的多级分佣 | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-commission-pool-reward` | pool 累积 + `claimPoolReward` | `e2e/suites/entity-commerce-commission-flow.ts` |
| `pallet-nex-market` | 简单挂单/撤单 smoke | `e2e/suites/nex-market-smoke.ts` |
| `entity registry` | `createEntity` / `updateEntity` | `e2e/suites/entity-lifecycle.ts` |

## 4. 本次新增远程业务流结果

### 4.1 `pallet-entity-shop`

基准上下文：

- entity：`100007`
- secondary shop：`9`
- owner：Alice

执行流：

1. `addManager`
2. manager `updateShop`
3. `setPrimaryShop`
4. `fundOperating`
5. `withdrawOperatingFund`
6. `removeManager`

结果：**通过**

关键结果：

- `entityShop.ManagerAdded`
- `entityShop.ShopUpdated`
- `entityShop.PrimaryShopChanged`
- `entityShop.OperatingFundDeposited`
- `entityShop.OperatingFundWithdrawn`
- `entityShop.ManagerRemoved`

最终状态：

- entity `100007` 的 `primaryShopId = 9`
- shop `9` 的 `managers = []`

产物：

- `artifacts/base-context.json`
- `artifacts/entity-shop-flow.json`

### 4.2 `pallet-entity-member` + `pallet-entity-loyalty`

执行流：

1. `setMemberPolicy(shopId=9, 4)` 切到审批制
2. Charlie / Dave `registerMember`
3. owner `batchApproveMembers`
4. `enablePoints`
5. `updatePointsConfig`
6. `managerIssuePoints`
7. `transferPoints`
8. `redeemPoints`

结果：**通过**

关键事件：

- `entityMember.MemberPolicyUpdated`
- `entityMember.MemberPendingApproval`
- `entityMember.BatchMembersApproved`
- `entityLoyalty.ShopPointsEnabled`
- `entityLoyalty.PointsConfigUpdated`
- `entityLoyalty.PointsIssued`
- `entityLoyalty.PointsTransferred`
- `entityLoyalty.PointsRedeemed`

最终状态：

- `memberCount(entity=100007) = 2`
- Charlie points：`15000000000000`（15 NEX 等值）
- Dave points：`3000000000000`（3 NEX 等值）
- Dave redeem 2 NEX 等值积分后，自由余额净增加约 `1.998186308409 NEX`（扣除了手续费）
- Dave `memberShoppingBalance` 未增加，说明本链 `redeemPoints` 直接回到自由余额而不是 shopping balance

产物：

- `artifacts/entity-member-loyalty-flow.json`

### 4.3 `pallet-entity-product` + `pallet-entity-order`

执行流：

1. 创建实物商品
2. 更新商品
3. 发布商品
4. Charlie 下单
5. `updateShippingAddress`
6. `shipOrder`
7. `updateTracking`
8. `extendConfirmTimeout`
9. `confirmReceipt`
10. Dave 第二单
11. `requestRefund`
12. seller `approveRefund`
13. `unpublishProduct`

结果：**通过**

关键事件：

- `entityProduct.ProductCreated`
- `entityProduct.ProductUpdated`
- `entityProduct.ProductStatusChanged`
- `entityTransaction.OrderCreated`
- `entityTransaction.ShippingAddressUpdated`
- `entityTransaction.OrderShipped`
- `entityTransaction.TrackingInfoUpdated`
- `entityTransaction.ConfirmTimeoutExtended`
- `entityTransaction.OrderCompleted`
- `entityTransaction.OrderDisputed`
- `entityTransaction.OrderRefunded`

关键对象：

- product：`1`
- 正常履约订单：`5`
- 退款订单：`6`

最终状态：

- 商品发布后：`OnSale`
- 订单 `5`：`Completed`
- 订单 `6`：`Disputed -> Refunded`
- 商品下架后：`OffShelf`

备注：

- 本链 `unpublishProduct` 下架后的状态是 `OffShelf`，不是 `Draft`

产物：

- `artifacts/entity-product-order-physical-flow.json`

### 4.4 `pallet-commission-single-line` / `multi-level` / `pool-reward`

执行流：

#### single-line

1. `setSingleLineConfig`
2. `updateSingleLineParams`
3. `pauseSingleLine`
4. `resumeSingleLine`
5. `scheduleConfigChange`

结果：**通过**

当前配置：

- `uplineRate = 120`
- `downlineRate = 150`
- `maxUplineLevels = 4`
- `maxDownlineLevels = 4`

已写入 pending：

- `applyAfter = 2492`

#### multi-level

1. `setMultiLevelConfig`
2. `addTier`
3. `removeTier`
4. `pauseMultiLevel`
5. `resumeMultiLevel`
6. `scheduleConfigChange`

结果：**通过**

当前配置：

- levels：`[200, 100]`
- `maxTotalRate = 300`

已写入 pending：

- `effectiveAt = 2510`

#### pool-reward

1. `setPoolRewardConfig`
2. `pausePoolReward`
3. `resumePoolReward`
4. `schedulePoolRewardConfigChange`

结果：**通过**

当前配置：

- `levelRatios = [[0, 10000]]`
- `roundDuration = 14400`

已写入 pending：

- `applyAfter = 16822`

关键事件：

- `commissionSingleLine.SingleLineConfigUpdated`
- `commissionSingleLine.ConfigChangeScheduled`
- `commissionMultiLevel.MultiLevelConfigUpdated`
- `commissionMultiLevel.PendingConfigScheduled`
- `commissionPoolReward.PoolRewardConfigUpdated`
- `commissionPoolReward.PoolRewardConfigScheduled`

产物：

- `artifacts/commission-admin-controls.json`

### 4.5 `pallet-nex-market`

执行流：

1. 给 Charlie 补足保证金资金
2. Bob `placeSellOrder`
3. Charlie `reserveSellOrder`
4. Charlie `confirmPayment`
5. Bob `sellerConfirmReceived`

结果：**通过**

关键事件：

- `nexMarket.OrderCreated`
- `nexMarket.UsdtTradeCreated`
- `nexMarket.BuyerDepositLocked`
- `nexMarket.UsdtPaymentSubmitted`
- `nexMarket.BuyerDepositReleased`
- `nexMarket.UsdtTradeCompleted`
- `nexMarket.SellerConfirmedReceived`

关键状态：

- order `3`：`Open -> Filled`
- trade `1`：`AwaitingPayment -> Completed`

远端节点关键观察：

- `priceProtection.initialPrice = 10`
- 本次 trade 的 `buyerDeposit = 30000000000000000`（即 `30000 NEX`）
- 因此买家侧保证金要求异常高，执行前需要先给 Charlie 补资金
- 有效 TRON 地址：
  - seller：`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`
  - buyer：`TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE`

产物：

- `artifacts/nex-market-trade-flow.json`

## 5. 总结

本次“跳过已有流”后新增的 5 组远程业务流，已全部实测通过：

| Case | 覆盖模块 | 结果 |
|---|---|---|
| `entity-shop-flow` | `pallet-entity-shop` | ✅ 通过 |
| `entity-member-loyalty-flow` | `pallet-entity-member`, `pallet-entity-loyalty` | ✅ 通过 |
| `entity-product-order-physical-flow` | `pallet-entity-product`, `pallet-entity-order` | ✅ 通过 |
| `commission-admin-controls` | `pallet-commission-single-line`, `pallet-commission-multi-level`, `pallet-commission-pool-reward` | ✅ 通过 |
| `nex-market-trade-flow` | `pallet-nex-market` | ✅ 通过 |

另外，远端只读检查也已通过：

- `npm run e2e:remote:inspect`
- `npm run e2e:remote:contracts`

## 6. 目录内最终交付

- 报告：`remote-business-flows-20260313/REPORT.md`
- 汇总：`remote-business-flows-20260313/artifacts/latest.json`
- 执行状态：`remote-business-flows-20260313/artifacts/execution-status.json`
- 各 case 明细：
  - `artifacts/entity-shop-flow.json`
  - `artifacts/entity-member-loyalty-flow.json`
  - `artifacts/entity-product-order-physical-flow.json`
  - `artifacts/commission-admin-controls.json`
  - `artifacts/nex-market-trade-flow.json`

