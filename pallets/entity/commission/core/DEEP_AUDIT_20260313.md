# pallet-commission-core 深度业务流审计（2026-03-13）

我把 `pallet-commission-core`、`referral`、`order`、`runtime hook` 一起看了，并跑了 `cargo test -p pallet-commission-core --lib`，**208 个测试全过**。  
但从“真实业务流 + 多角色视角”看，**还有 5 个高风险问题、4 个必须补的能力、3 个明显冗余/设计债**。

---

## 一、先给结论

### P0 级 BUG / 漏洞
1. **返佣/提现/取消流程不是原子事务，且运行时把错误吞掉了**
   - `process_commission` 里先转国库、先写佣金记录，再在后面继续转账：`engine.rs:87-107, 136-234, 281-297`
   - `withdraw_commission` 里先 `auto_register_qualified`、后面才继续做 cooldown/偿付检查：`lib.rs:1115-1205`
   - 运行时 hook 直接 `let _ = ...process_commission(...)` / `let _ = ...cancel_commission(...)`：`runtime/src/configs/mod.rs:1641-1653, 1680-1683`
   - **结果**：可能出现“订单完成了，但返佣只做了一半 / 退款只做了一半 / 提现失败但目标会员已经被注册”的脏状态。

2. **Root 紧急禁用可被 Owner/Admin 直接绕过，连 POOL_REWARD cooldown 都会被清掉**
   - Root 禁用只是 `config.enabled=false`：`lib.rs:1830-1844`
   - Owner/Admin 仍可 `enable_commission(true)`：`lib.rs:1042-1070`
   - 且重新启用时会 `PoolRewardDisabledAt::remove`，等于把 cooldown 一起绕过。
   - **这是实打实的治理失效漏洞。**

3. **归档会把“尚未补偿完成”的退款凭证直接删掉**
   - 取消失败时，`OrderTreasuryTransfer` / `OrderUnallocated` / `OrderTokenUnallocated` / `OrderTokenPlatformRetention` 会保留，等待后续 retry：`engine.rs:765-810, 846-880`
   - 但 `archive_order_records` 无条件把这些都删掉：`lib.rs:2033-2039`
   - **结果**：如果先 cancel 部分失败、后 archive，就可能把后续补偿依据永久删掉。**

4. **治理提案“看起来成功，实际上没生效”**
   - `governance_set_referrer_guard / governance_set_commission_cap / governance_set_referral_validity` 全是 `Ok(())` 空实现：`lib.rs:2726-2752`
   - 但 governance 确实会调用这些：`entity/governance/src/lib.rs:3222-3228`
   - 而 referral pallet 明明已经有真实配置能力：`entity/commission/referral/src/lib.rs:482-541`
   - **这是“治理假成功”问题。**

5. **Token 平台费转入 entity_account 失败时，后续 token 返佣仍按“成功到账”继续记账**
   - order 完成时，平台费转入失败只记事件，不回滚：`entity/order/src/lib.rs:1395-1405`
   - runtime 仍把 `info.token_platform_fee` 原样喂给 `process_token_commission`：`runtime/src/configs/mod.rs:1647-1653`
   - core 又会按这个 nominal fee 做 Pool A 记账：`engine.rs:410-448`
   - **结果**：Token 佣金可能“账上有承诺，实际上没钱”。**

---

## 二、按角色看，必须补什么

### 1）平台 / Root / Governance
必须补：
- **Sticky force disable 标志**
  - 不是简单 `enabled=false`
  - 需要单独 `ForceDisabledByRoot`，Owner/Admin 不能自行恢复，只能 Root 清除
- **治理配置真正落地到 referral pallet**
  - 推荐人门槛
  - 单笔/累计返佣 cap
  - 推荐关系有效期
- **失败补偿队列**
  - 至少能查询哪些订单 cancel 失败、卡在哪一步
  - 不能只靠 Root 手动猜测 `retry_cancel_commission`

### 2）Entity Owner / Admin
必须补：
- **失败订单补偿面板 / runtime API**
  - 当前 `CommissionRefundFailed` 连 `order_id` 都没有：`lib.rs:750-755`
  - 运维几乎没法定位具体失败单
- **订单归档前的硬校验**
  - 只有当 `OrderTreasuryTransfer/OrderUnallocated/...` 全清空后才能 archive
- **资产维度拆分配置**
  - 现在只有一个 `max_commission_rate`，但又有 `GlobalMaxTokenCommissionRate`
  - 设计上很混乱，建议要么拆成 NEX/Token 两套 rate，要么删掉 token 专属 cap

### 3）Seller / Shop
必须补：
- **取消失败后的非 Root 重试通道**
  - 现在只有 Root 能 `retry_cancel_commission`
  - 实际业务里 seller/entity admin 也需要处理自己的失败单
- **取消流程状态机可视化**
  - 现在 order 侧可能已经 cancelled/refunded，但 commission 侧仍部分失败，卖家很难知道资金是否真的回补

### 4）会员 / 推荐人 / 被赠与目标
必须补：
- **提现预览接口**
  - 返回：withdraw / repurchase / bonus / earliest_block / 拒绝原因
- **赠与提现的安全顺序**
  - 先做 cooldown/偿付检查，再 auto-register target
  - 否则失败提现也可能白白创建会员关系：`lib.rs:1115-1145`
- **会员流水查询接口**
  - 现在 `MemberWithdrawalHistory`、`MemberCommissionOrderIds` 只看到写入，几乎没对外读取接口
  - 从用户视角，这些链上存储目前业务价值接近 0

---

## 三、还存在的重要逻辑 BUG

1. **`settle_order_commission` 写了，但没接进真实运行时**
   - core 明确说应由订单模块在完结时调用：`engine.rs:25-29`
   - 但全局检索后，运行时/订单里没接这条链路，只在 tests 里用了
   - **结果**：生产环境里订单记录大概率长期停留在 `Pending`，归档流转不完整。**

2. **RepeatPurchase 的“累计订单数”口径不对**
   - referral README 写的是“累计订单数 ≥ min_orders”：`referral/README.md:16`
   - 但实际判断用的是 core 自己维护的 `buyer_order_count`：`referral/src/lib.rs:776-785`
   - 而这个计数只在 commission 成功处理后才 `+1`：`core/engine.rs:267-270`
   - **判断**：当佣金关闭、处理失败、或 NEX/Token 分资产计数时，复购判定会失真。

3. **取消成功统计不真实**
   - `CommissionCancelled.refund_failed` 只统计 `refund_groups`，**不包含** treasury refund / unallocated refund / token pool refund 失败：`engine.rs:815-817`
   - 这会造成“事件显示成功，但实际还有财务尾账”。

4. **缺少幂等保护**
   - `process_commission/process_token_commission` 没看到按 `order_id` 的“已处理”防重
   - 如果 hook 重放/重复调用，会重复记账、重复加 pending、重复转账。

---

## 四、冗余 / 设计债

1. **`WithdrawalTierConfig.withdrawal_rate` 基本是冗余字段**
   - 真正计算几乎都只看 `repurchase_rate`
   - `withdrawal_rate = 10000 - repurchase_rate` 可推导

2. **`MemberCommissionOrderIds` / `MemberTokenCommissionOrderIds` / 提现历史存储，当前更像“写入型垃圾桶”**
   - 只看到写入点，几乎没看到对外读取/分页/清理
   - 这会带来链上存储膨胀，但业务端拿不到价值

3. **README 的“双资产并行双管线”与真实 runtime 不一致**
   - runtime 实际是按支付资产二选一分发：`runtime/src/configs/mod.rs:1638-1653`
   - 不是 README 里说的“每单同时跑 NEX + Token 两条管线”
   - 这要么是文档错误，要么是功能没落地

---

## 五、建议修复优先级

### 第一批，必须立刻做
- 给 `force_disable_entity_commission` 增加 **不可绕过的 force-disabled 状态**
- 所有关键流程加 **事务性保护** 或改为 **可恢复任务队列**
- runtime hook **不能吞错**
- `archive_order_records` 增加“补偿尾账必须清零”的前置条件
- 治理接口真正接到 referral pallet

### 第二批，尽快做
- 接通 `settle_order_commission`
- 加 `order_id` 维度的失败事件/失败查询
- 修正 RepeatPurchase 的累计订单口径
- 给 `process_commission` 增加幂等保护

### 第三批，优化体验
- 提现预览 API
- 会员/推荐人流水分页 API
- 清理或真正利用 `order ids / withdrawal history` 存储
- 统一文档与 runtime 真实现

---

## 六、补充说明

- 已执行：`cargo test -p pallet-commission-core --lib`
- 结果：**208 tests passed**
- 额外尝试：`clippy` 检查被上游 `pallet-entity-common` 的既有 warning/error 阻断，不是 `pallet-commission-core` 本文件直接独占导致

