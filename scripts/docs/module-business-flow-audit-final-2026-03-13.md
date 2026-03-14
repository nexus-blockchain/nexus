# 9 模块业务流文档审计结论（最终版）

- 日期：2026-03-13
- 审计对象：
  - `docs/module-business-flow-mapping-2026-03-13.md`
  - `docs/module-business-flow-summary-2026-03-13.md`
  - `docs/module-by-module-flow-checklist-2026-03-13.md`
- 审计范围：
  - `pallet-commission-single-line`
  - `pallet-commission-pool-reward`
  - `pallet-commission-multi-level`
  - `pallet-entity-shop`
  - `pallet-entity-member`
  - `pallet-entity-loyalty`
  - `pallet-entity-product`
  - `pallet-entity-order`（链上实际订单 pallet 为 `entityTransaction`）
  - `pallet-nex-market`

---

## 1. 最终结论

**结论：这三份文档不包含全部“应测业务流”。**

如果标准是：

- **汇总当前已经实现 / 已执行过的核心流**：这三份文档**基本成立**；
- **覆盖这 9 个模块所有重要业务流 / 异常流 / 治理流**：这三份文档**明显不成立**。

更准确地说，当前文档集合描述的是：

> **截至 2026-03-13，仓库内已实现 / 已执行的 7 条代表性业务流（其中既有组合流，也有单模块 / 控制面流），以及它们对 9 个模块的部分覆盖情况。**

其中，`pallet-nex-market` **不应被表述为“承载其余模块组合业务流的主流程模块”**；它一方面有自己的独立交易流，另一方面又以**价格预言机依赖**的方式间接参与 `entity-commerce-commission-flow`。

它**不是**这 9 个模块的“全量待测清单”，也**不能直接等同于**“所有重要业务流已覆盖”。

---

## 2. 逐文件判断

| 文件 | 结论 | 说明 |
|---|---|---|
| `docs/module-business-flow-mapping-2026-03-13.md` | 部分成立 | 该文档在 `15-16` 行已明确声明“这里只列已实现 / 已测试的实际组合业务流，不是理论全量”，因此可作为“已测流映射”，不能作为“全量应测流清单”。 |
| `docs/module-business-flow-summary-2026-03-13.md` | 不成立 | `10-18` 行把 9 个模块全部标成 `✅`，`26-32` 行只列 7 条流，`70` 行又写成“已整理为 7 条实际可执行的组合业务流”；这更像“当前样本覆盖摘要”，不是“完整应测范围”。 |
| `docs/module-by-module-flow-checklist-2026-03-13.md` | 不成立 | 主体内容本质上仍是现有流归纳，但 `227-237` 行把每个模块都写成“已覆盖……”，容易被误读为“模块重要业务流已完整覆盖”。 |

---

## 3. 审计依据

### 3.1 当前文档实际围绕 7 条已实现 / 已执行流展开（并非全部都是组合流）

仓库当前业务流汇总，核心上来自以下本地 suite 与远端补测：

#### 本地主要 suite

- `e2e/suites/entity-commerce-commission-flow.ts`
- `e2e/suites/nex-market-smoke.ts`
- `e2e/suites/entity-lifecycle.ts`
- `e2e/suites/runtime-contracts.ts`

#### 远端新增业务流（5 组）

`remote-business-flows-20260313/run-remote-business-flows.mjs` 当前定义的业务 case 为：

- `entity-shop-flow`：`827-858`
- `entity-member-loyalty-flow`：`860-972`
- `entity-product-order-physical-flow`：`974-1158`
- `commission-admin-controls`：`1160-1317`
- `nex-market-trade-flow`：`1319-1403`

对应报告汇总见：

- `remote-business-flows-20260313/REPORT.md:303-311`

因此，当前三份文档本质上是在总结：

- 1 条**大组合主流**：`entity-commerce-commission-flow`
- 2 条**单模块市场流**：`nex-market-smoke`、`nex-market-trade-flow`
- 1 条**单模块店铺流**：`entity-shop-flow`
- 2 条**双模块组合流**：`entity-member-loyalty-flow`、`entity-product-order-physical-flow`
- 1 条**commission 控制面组合流**：`commission-admin-controls`

合计 **7 条代表性已执行流**。

### 3.2 `pallet-nex-market` 的准确定位：独立交易流 + 价格预言机依赖，不是其余模块组合流的“容器”

从实际代码看，`pallet-nex-market` 有两种参与方式：

1. **直接业务流**
   - `e2e/suites/nex-market-smoke.ts`：覆盖挂单 / 撤单 smoke
   - `remote-business-flows-20260313/run-remote-business-flows.mjs:1319-1403`：覆盖 `placeSellOrder -> reserveSellOrder -> confirmPayment -> sellerConfirmReceived`
2. **间接依赖角色**
   - `e2e/suites/entity-commerce-commission-flow.ts:242-245`：显式检查 `nex-market` price oracle 可用
   - `e2e/suites/entity-commerce-commission-flow.ts:519`：商品创建步骤直接写明 “priced via the active nex-market oracle”
   - `runtime/src/configs/mod.rs:333-355`：`TradingPricingProvider` 直接基于 `pallet_nex_market`
   - `runtime/src/configs/mod.rs:1315-1401`、`1452-1464`：`entity-registry`、`entity-product`、`entity-order` 通过 `EntityPricingProvider` 间接依赖 `pallet_nex_market`

因此，如果问题是“`pallet-nex-market` 是否**包含** `pallet-commission-*`、`pallet-entity-*` 这些模块的组合业务流”，答案应是：

> **不直接包含。**
>
> 更准确地说：**组合业务流主要由 `entity-commerce-commission-flow` 承载；`pallet-nex-market` 既有自己的独立交易流，又作为全链价格预言机基础设施间接参与该组合流。**

### 3.3 这些流不能代表 9 个模块的全量应测语义

从文档与执行脚本能看出，当前覆盖重点是：

- 若干 **happy path**
- 少量 **控制面 / 配置面**
- 少量 **退款 / 争议 / 延迟配置**

但尚未纳入大量：

- 异常分支
- 超时分支
- 治理分支
- 批量处理分支
- 清理 / 补偿 / 索引维护分支
- 某些与 OCW / sidecar 配套的链上收口分支

因此，把当前文档说成“9 个模块已覆盖完所有应测业务流”并不准确。

---

## 4. 明显漏掉的业务流（按模块）

以下项目不是在说“仓库完全没有任何相关代码”，而是在说：**这些重要业务流没有被当前三份文档纳入为已覆盖范围。**

### 4.1 `pallet-nex-market`

当前文档只覆盖了：

- 卖单 / 撤卖单
- 买单 / 撤买单
- `placeSellOrder -> reserveSellOrder -> confirmPayment -> sellerConfirmReceived`

需要单独澄清的是：

- **在“独立交易流”意义上**，当前文档对 `pallet-nex-market` 的覆盖主要停留在它自己的 market path；
- **在“跨模块依赖”意义上**，`pallet-nex-market` 还通过价格预言机参与了 `entity-commerce-commission-flow`；
- 但这**不等于**“`pallet-nex-market` 自身承载了 `entity-shop/member/product/order/loyalty/commission` 的组合业务主链”。

但模块内仍有大量关键路径未进入当前文档，例如：

- **买单成交正向流**：`accept_buy_order`（`../pallets/trading/nex-market/src/lib.rs:1567-1568`）
- **超时流**：`process_timeout`（`1719-1720`）
- **争议流**：`dispute_trade`（`2434`）、`resolve_dispute`（`2490`）
- **管理员干预流**：`force_settle_trade`（`2404`）、`force_cancel_trade`（`2420`）
- **改单流**：`update_order_price`（`2565`）、`update_order_amount`（`2807`）
- **少付 / OCW 相关收口流**：`submit_ocw_result`（`1859`）、`claim_verification_reward`（`1952`）、`auto_confirm_payment`（`2159`）、`submit_underpaid_update`（`2259`）、`finalize_underpaid`（`2327`）

其中最典型的漏项是：

- **`acceptBuyOrder` 买单成交路径未被当前文档纳入**；
- 旧脚本中其实已有完整示例：`reports/remote-business-flow-20260313-standalone/run-business-flows.mjs:805-874`。

### 4.2 `pallet-entity-order`（`entityTransaction`）

当前文档主要覆盖：

- 数字商品即时完成
- 实物商品下单、改地址、发货、物流、确认收货、退款

但仍遗漏至少以下重要业务流：

- **服务类订单流**：`start_service`（`578-579`）、`complete_service`（`612-613`）、`confirm_service`（`648-649`）
- **取消 / 退款分支**：`cancel_order`（`472-473`）、`reject_refund`（`714-715`）、`seller_cancel_order`（`726-727`）、`seller_refund_order`（`907-908`）
- **管理员处置**：`force_refund`（`759-760`）、`force_partial_refund`（`940-941`）、`force_complete`（`767-768`）
- **争议撤回**：`withdraw_dispute`（`955-956`）
- **补偿清理流**：`force_process_expirations`（`966-967`）
- **索引清理流**：`cleanup_buyer_orders`（`682-683`）、`cleanup_shop_orders`（`847-848`）、`cleanup_payer_orders`（`1007-1008`）
- **代付流**：`place_order_for`（`988-989`）

### 4.3 `pallet-entity-shop`

当前文档主要覆盖：

- 二级店
- manager 增删
- 主店切换
- 经营资金充值 / 提现

但完整店铺生命周期仍包含：

- **暂停 / 恢复**：`pause_shop`（`635-636`）、`resume_shop`（`661-662`）
- **关闭生命周期**：`close_shop`（`737-738`）、`finalize_close_shop`（`787-788`）、`cancel_close_shop`（`1160-1161`）
- **转让流**：`request_transfer_shop`（`886`）、`accept_transfer_shop`（`951`）、`cancel_transfer_shop`（`1014`）
- **治理流**：`force_pause_shop`（`1085-1086`）、`force_close_shop`（`1109-1110`）、`ban_shop`（`1224-1225`）、`unban_shop`（`1260-1261`）
- **属性变更**：`set_shop_type`（`1127-1128`）、`set_location`（`694-695`）
- **manager 自辞**：`resign_manager`（`1199-1200`）

### 4.4 `pallet-entity-member`

当前文档主要覆盖：

- 注册
- 审批入会
- 推荐链
- 激活

但 member 模块还有完整的等级、升级、治理、异常处理体系：

- **补绑推荐人**：`bind_referrer`（`1046-1047`）
- **等级体系**：`init_level_system`（`1110-1111`）、`add_custom_level`（`1153-1154`）、`update_custom_level`（`1211-1212`）、`remove_custom_level`（`1279-1280`）、`manual_set_member_level`（`1321-1322`）
- **升级规则体系**：`init_upgrade_rule_system`（`1442-1443`）、`add_upgrade_rule`（`1487-1488`）、`update_upgrade_rule`（`1555-1556`）、`remove_upgrade_rule`（`1594-1595`）、`set_upgrade_rule_system_enabled`（`1625-1626`）、`set_conflict_strategy`（`1651-1652`）
- **审批异常流**：`reject_member`（`1764-1765`）、`cancel_pending_member`（`1800-1801`）、`cleanup_expired_pending`（`1826-1827`）、`batch_reject_members`（`1957-1958`）
- **会员治理流**：`ban_member`（`1996-1997`）、`unban_member`（`2035-2036`）、`remove_member`（`2073-2074`）、`leave_entity`（`2166-2167`）、`deactivate_member`（`2232-2233`）
- **系统重置流**：`reset_level_system`（`2100-2101`）、`reset_upgrade_rule_system`（`2137-2138`）

### 4.5 `pallet-entity-loyalty`

当前文档主要覆盖：

- `enablePoints` / `updatePointsConfig`
- 发积分 / 转积分 / 兑积分
- shopping balance 消费

但 loyalty 还包含完整配置与清理生命周期：

- `disable_points`（`445-446`）
- `manager_burn_points`（`604-605`）
- `set_points_ttl`（`714-715`）
- `expire_points`（`748-749`）
- `set_points_max_supply`（`787-788`）
- `continue_cleanup`（`828-829`）

### 4.6 `pallet-entity-product`

当前文档主要覆盖：

- `create / update / publish / unpublish`

但 product 模块还有删除和批量治理流：

- `delete_product`（`824-825`）
- `force_unpublish_product`（`904-905`）
- `force_delete_product`（`1161-1162`）
- `batch_publish_products`（`951-952`）
- `batch_unpublish_products`（`1013-1014`）
- `batch_delete_products`（`1077-1078`）

### 4.7 三个 commission 模块

当前文档覆盖的是：

- 订单触发分佣
- 一部分控制面：配置、更新、暂停、恢复、延迟生效

但仍不是全量：

#### `pallet-commission-single-line`

- `clear_single_line_config`（`323-324`）
- `set_level_based_levels`（`392-393`）
- `remove_level_based_levels`（`419-420`）
- `apply_pending_config`（`568-569`）
- `cancel_pending_config`（`603-604`）
- `force_reset_single_line`（`483-484`）
- `force_remove_from_single_line`（`619-620`）
- `force_restore_to_single_line`（`634-635`）

#### `pallet-commission-multi-level`

- `clear_multi_level_config`（`461-462`）
- `update_multi_level_params`（`540-541`）
- `apply_pending_config`（`721-722`）
- `cancel_pending_config`（`742-743`）
- `force_cleanup_entity`（`800-802`）

#### `pallet-commission-pool-reward`

- `start_new_round`（`623-624`）
- `set_token_pool_enabled`（`656-657`）
- `set_global_pool_reward_paused`（`786-787`）
- `apply_pending_pool_reward_config`（`871-872`）
- `cancel_pending_pool_reward_config`（`893-894`）
- `correct_token_pool_deficit`（`911-912`）

---

## 5. 文档本身存在的两个问题

### 5.1 口径不一致

- `mapping` 文档承认自己只是“**已实现 / 已测试**”流：`docs/module-business-flow-mapping-2026-03-13.md:15-16`
- 但 `summary` 文档在 `10-18` 行把 9 个模块都直接标成 `✅`
- `checklist` 文档在 `227-237` 行又统一写成“已覆盖……”

这会让读者误以为：

- 当前文档已经代表了“9 个模块全量重要业务流覆盖”

而实际更接近：

- 当前文档只代表“截至 2026-03-13 已整理出的 7 条代表性已执行流”。

### 5.2 路径引用有误

以下位置写成了 `reports/...`，但实际文件位于 `docs/...`：

- `docs/module-business-flow-summary-2026-03-13.md:76-77`
- `docs/module-by-module-flow-checklist-2026-03-13.md:243-245`

---

## 6. 审计后的准确表述建议

### 6.1 更准确的标题建议

| 当前文件 | 建议标题 |
|---|---|
| `module-business-flow-mapping-2026-03-13.md` | **已实现 / 已测试组合业务流映射（非全量）** |
| `module-business-flow-summary-2026-03-13.md` | **当前已实现 / 已执行代表性业务流摘要（非全量）** |
| `module-by-module-flow-checklist-2026-03-13.md` | **按模块拆分的已验证动作清单（非完整覆盖）** |

### 6.2 更准确的一句话口径

可直接替换为：

> 截至 2026-03-13，仓库与远端补测共整理出 7 条已实现 / 已执行的代表性业务流（包含组合流、单模块流与控制面流），覆盖了 9 个模块的部分核心 happy path 以及少量控制面 / 治理动作；其中 `pallet-nex-market` 同时承担独立交易流与价格预言机依赖角色，但不应被表述为其余模块组合业务流的主承载模块。

### 6.3 更准确的“对外结论”

建议使用：

> 当前文档能够证明：这 9 个模块**都有落到实际脚本或远端补测中的代表性流**；其中 `pallet-nex-market` 既有独立交易流，也以价格预言机依赖的方式参与 entity / commission 组合流；但这仍**不能证明**这 9 个模块的所有重要业务流、异常流与治理流都已经被覆盖。

---

## 7. 最终判断

**最终判断：**

这三份文档可以作为：

- **当前已测 / 已执行业务流的整理结果**；
- **阶段性覆盖样本的汇总材料**；
- **后续补齐测试矩阵的基础输入**。

但它们**不能作为**：

- **9 个模块全量应测业务流清单**；
- **完整覆盖证明**；
- **所有重要异常流 / 治理流已覆盖的结论依据**。

如果后续目标是“形成完整测试矩阵”，下一步应该把本文第 4 节列出的缺口继续拆成：

1. **核心正向流**
2. **异常 / 争议 / 超时流**
3. **治理 / 强制处置流**
4. **批量 / 清理 / 补偿流**

再逐项补进 suite、远端 case 与文档口径。
