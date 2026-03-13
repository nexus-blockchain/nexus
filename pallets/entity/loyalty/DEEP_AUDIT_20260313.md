# pallet-entity-loyalty 深度分析审计报告

- 模块：`pallet-entity-loyalty`
- 日期：2026-03-13
- 范围：从所有不同角色用户视角，分析业务流中还必须增加的功能、冗余功能、代码 BUG、漏洞

---

## 结论摘要

结论先说：**`pallet-entity-loyalty` 现在更像“账本半成品”，还不适合直接作为生产级会员/积分/购物余额核心模块上线。**

最大的问题不是功能少，而是**业务闭环、回滚闭环、清理闭环都没做完**。

---

## 一、按角色 / 业务流的核心结论

| 角色 | 业务流 | 结论 |
|---|---|---|
| 会员 / 买家 | 下单使用 `shopping balance` | **P0 漏洞**：下单前就先 `consume`，后续下单失败时可能把“不可提现购物余额”变相提现 |
| 会员 / 买家 | 下单使用 `token discount` | **P0 业务缺陷**：token 先烧，订单失败/退款没有恢复机制 |
| 会员 / 买家 | 购物后获得 Shop 积分 | **核心功能缺失**：`reward_rate` 只是配置，实际没有订单完成后自动发积分 |
| 会员 / 买家 | 订单取消 / 退款 | **核心功能缺失**：没有把已用购物余额 / 已烧 token 折扣返还回来 |
| Shop Manager | 暂停积分系统 | **设计错误**：现在只有“禁用并清库”，没有“暂停但保留用户权益” |
| Shop Manager / 财务 | 配置兑换比例 + 发积分 | **高风险权限过大**：等于能制造“可兑 NEX 的凭证” |
| Entity Owner / 治理 | Entity 级治理开关积分 | **BUG**：`PointsToggle(enabled=true)` 实际什么都不做 |
| 运维 / 升级 | 从旧模块迁移到 loyalty | **高风险**：代码里没看到真正的链上迁移 / 兼容方案 |
| 开发 / 审计 | 稳定性保障 | **严重不足**：当前 `cargo test -p pallet-entity-loyalty` 是 **0 tests** |

---

## 二、必须要增加的功能

### 1. 订单权益必须改成“两阶段”：`reserve -> commit / rollback`

这是最需要补的。

当前下单链路里：

- `order` 先调 `T::Loyalty::redeem_for_discount(...)` / `consume_shopping_balance(...)`
- 之后才 `Escrow::lock_from(...)`

参考：

- `entity/order/src/lib.rs:1198-1208`
- `entity/order/src/lib.rs:1248-1252`

这意味着 loyalty 权益已经动了，但订单还没真正落成功。

**必须新增：**

- `reserve_shopping_balance`
- `commit_shopping_balance`
- `rollback_shopping_balance`
- `reserve_token_discount`
- `commit_token_discount`
- `rollback_token_discount`

否则退款、取消、下单失败都没法严格对账。

### 2. `Order` 必须记录“用了多少权益”

当前 `Order` 结构只有：

- `total_amount`
- `token_payment_amount`

但**没有**：

- `shopping_balance_used`
- `token_discount_used`
- `points_used`
- `points_rewarded`

参考：

- `entity/order/src/lib.rs:60-95`

所以现在订单取消 / 退款时，系统根本不知道该返还哪些 loyalty 权益。  
这是退款闭环缺失的根因。

### 3. Shop 积分“购物返积分”闭环必须补齐

`PointsConfig.reward_rate` 目前只是存起来了，**没有任何生产路径真正使用它**。

证据：

- `reward_rate` 只出现在配置 / 更新逻辑里：`entity/loyalty/src/lib.rs`
- 仓内没有看到订单完成后调用 `issue_points(...)` 的生产代码
- runtime 的 `OrderLoyaltyHook` 实际调用的是 **token** 的 `reward_on_purchase`，不是 loyalty points  
  参考 `../runtime/src/configs/mod.rs:1660-1668`

所以从会员视角，“购物返积分”这个主功能其实没落地。

**必须增加：**

- 订单完成 -> 按 shop `reward_rate` 自动 `issue_points`
- 退款 / 取消 -> 反向撤销或冻结该订单产生的积分
- 最好带 `order_id` 溯源

### 4. 积分系统必须有“软暂停”，不能只有“删库式禁用”

现在 `disable_points` 是直接清：

- config
- balance
- total_supply
- ttl
- expires_at
- max_supply

参考：

- `entity/loyalty/src/lib.rs:428-433`

这对用户来说太危险。  
**应该至少拆成：**

- `pause_issue_points`
- `pause_transfer_points`
- `pause_redeem_points`
- `disable_points_after_grace_period`

否则 Shop Manager 或治理层一键就把用户积分清掉了。

### 5. Shop 关闭 / 禁用积分必须有“兑付宽限期”

当前：

- Shop 关闭时会调 `PointsCleanup`
- `cleanup_shop_points` 直接删数据

这意味着用户积分**没有任何提前通知、宽限期、兑付窗口、补偿策略**。

从用户和客服视角，这是重大缺口。

### 6. 审计字段必须补：来源、原因、关联单号

现在事件只有 amount，没有原因 / 来源单号，例如：

- `PointsIssued`
- `PointsBurned`
- `ShoppingBalanceCredited`

都缺少：

- `order_id`
- `commission_id`
- `reason_code`
- `operator`

这会让财务、客服、合规审计非常痛苦。

### 7. 大规模清理必须做成分页任务 / 作业状态机

当前只是 `clear_prefix(..., 500, None)`。

参考：

- `entity/loyalty/src/lib.rs:429`
- `entity/loyalty/src/lib.rs:432`

这不是完整清理，只是**最多清 500 条**。  
必须增加：

- cleanup cursor
- batch cleanup extrinsic
- cleanup job state
- 最终完成标志

### 8. 如果这是升级链，不是新链：必须补 migration

文档里写了“靠 `storage_prefix` 零迁移”：

- `entity/docs/INTEGRATED_DEV_PLAN.md:701-714`

但实际 loyalty 代码里没看到对应的迁移实现，也没看到这些 storage 真按文档声明做兼容：

- `entity/loyalty/src/lib.rs:140-228`

如果这是线上链升级，**这是 P0 / P1 级风险**。

---

## 三、冗余 / 死功能

### 1. `reward_rate` 现在是死配置

它存在于：

- enable / update / governance set config

但没看到真实发积分链路调用它。  
=> **典型死字段**。

### 2. `do_use_shopping_balance` 基本是死分支

README 说：

- 下单抵扣走 `do_use_shopping_balance`（纯记账）

参考：

- `entity/loyalty/README.md:167-175`

但 runtime 实际桥接的是：

- `consume_shopping_balance -> do_consume_shopping_balance`

参考：

- `../runtime/src/configs/mod.rs:1808-1819`

所以当前“use / consume”两套语义是分裂的，`do_use_shopping_balance` 基本没有成为真实主链路。

### 3. `Pallet` 自己实现了 Port，但 runtime 又写了一层 `LoyaltyBridge`

参考：

- loyalty pallet 自身实现 Port：`entity/loyalty/src/lib.rs:1178-1249`
- runtime 又写 `LoyaltyBridge`：`../runtime/src/configs/mod.rs:1792-1835`

这会导致：

- 逻辑重复
- 后续修改时双处漂移
- 审计成本翻倍

这是明显冗余。

### 4. 文档严重过时

README 开头写：

- 8 个存储项
- 12 个事件
- Token shopping balance 还留在 commission/core

参考：

- `entity/loyalty/README.md:7`
- `entity/loyalty/README.md:24`

但代码里 loyalty 已经有 Token shopping balance 两个 storage。  
=> 文档和代码明显漂移。

---

## 四、明确的代码 BUG

### 1. 清理不完整 BUG：禁用 / 关闭 / 治理禁用时只清前 500 条

位置：

- `entity/loyalty/src/lib.rs:428-433`
- `entity/loyalty/src/lib.rs:952-957`
- `entity/loyalty/src/lib.rs:1314-1320`

风险：

- 大店铺用户多时，清不干净
- 留下孤儿余额 / 过期记录
- 重新启用后可能“幽灵余额复活”
- total_supply 已删，但 balances 还在，**账不平**

这是 **P0 / P1 级**。

### 2. `set_points_ttl(0)` 没有清掉历史 `expires_at`

位置：

- `entity/loyalty/src/lib.rs:698-701`

这意味着 manager 以为“改成永不过期”了，  
但老用户的 `ShopPointsExpiresAt` 还在，后面还是会过期。

这是明确业务 BUG。

### 3. 过期判断有 off-by-one

位置：

- `entity/loyalty/src/lib.rs:729`
- `entity/loyalty/src/lib.rs:826`

现在是 `now > expiry`，不是 `now >= expiry`。  
会导致积分比设定多活 1 个 block。

### 4. `governance_toggle_points(enabled = true)` 是假成功

位置：

- `entity/loyalty/src/lib.rs:1310-1326`

现在开启时直接 `Ok(())`，但其实什么都没启。  
对治理系统来说，这会产生“提案已执行，但业务没变化”的严重歧义。

### 5. 兑换保护口径可疑：`shop_account` 出金，却扣 `entity` 级 protected funds

位置：

- `entity/loyalty/src/lib.rs:645-651`

即：

- 用的是 `shop_account` 余额
- 扣的是 `CommissionFundGuard::protected_funds(entity_id)`

如果一个 entity 多个 shop，这很可能：

- 过度保守
- 双重扣减
- 阻止本应成功的兑换

这是**高风险口径错误**，需要业务确认。

---

## 五、明确的漏洞 / 高风险设计缺陷

### 1. P0：可通过失败订单把 shopping balance 变相提现

链路：

1. 买家下单时先 `consume_shopping_balance`
   - `entity/order/src/lib.rs:1204-1208`
2. loyalty 里立刻从 entity 账户转 NEX 给买家
   - `entity/loyalty/src/lib.rs:1045-1052`
3. 之后订单才 `Escrow::lock_from`
   - `entity/order/src/lib.rs:1248-1252`

如果后面的 `lock_from` 或后续步骤失败，而调用链又没有 `#[transactional] / with_transaction` 保护，  
用户就可能把“购物余额”提前变成自己钱包里的 NEX。

这直接绕过了 commission README 里“购物余额不可直接提取为 NEX”的业务规则：

- `entity/commission/core/README.md:203-209`

### 2. P0：token 折扣先烧 token，失败 / 退款不回滚

位置：

- order 先用折扣：`entity/order/src/lib.rs:1198-1201`
- token 里立即 `burn_from`：`entity/token/src/lib.rs:2051-2060`

而订单退款只退 `order.total_amount`：

- `entity/order/src/lib.rs:1512-1520`

并且取消 hook 只取消佣金，不处理 loyalty 权益：

- `../runtime/src/configs/mod.rs:1675-1684`

结果：

- 下单失败：token 可能白烧
- 订单退款：token 折扣不会返还

### 3. Shop Manager 权限过大，可“印积分 -> 兑现金”

相关权限：

- 改兑换率：`entity/loyalty/src/lib.rs:440-479`
- 发积分：`entity/loyalty/src/lib.rs:531-563`
- 用户兑 NEX：`entity/loyalty/src/lib.rs:645-659`

如果 `shop manager` 不是强信任角色，这等于给了他一个**现金发放后门**：

- 把 `exchange_rate` 设高
- 给自己 / 关联账户发积分
- 直接兑 NEX

建议至少：

- `exchange_rate / max_supply / disable_points` 提升到 owner / governance
- `manager_issue_points` 做额度 / 预算 / 审批限制

### 4. Shop Manager / 治理层可直接“清空用户积分”

位置：

- `disable_points`：`entity/loyalty/src/lib.rs:428-433`
- governance disable：`entity/loyalty/src/lib.rs:1314-1320`
- shop close cleanup：`entity/loyalty/src/lib.rs:952-957`

这不是技术漏洞，但从用户权益看是**强中心化风险**，而且当前没有：

- 公告期
- 赎回窗口
- 转换补偿
- 风险提示

### 5. TTL 机制可被“微量转账续命”绕过

位置：

- `maybe_extend_points_expiry`：`entity/loyalty/src/lib.rs:856-868`
- `transfer_points` 接收方收到积分后会延长整个账户 expiry：`entity/loyalty/src/lib.rs:516-520`

由于 expiry 是**账户级单值**，不是按批次 / lot 管理，  
用户只要不断收到很少量积分，就能把整包旧积分一起续期。

这对“严格过期”的营销规则是不成立的。

---

## 六、建议修复优先级

### P0（必须先修）

1. 下单权益改成 `reserve / commit / rollback`
2. 订单记录 `shopping_balance_used / token_discount_used`
3. 退款 / 取消时返还 shopping balance / token discount
4. 清理逻辑改成可分页、可完成确认的 job / cursor
5. 若是升级链，补 migration

### P1（紧接着修）

1. 补“订单完成自动返 Shop 积分”
2. 补退款撤销积分
3. 把 `disable_points` 改成“pause + grace period + finalize”
4. 收紧 manager 权限
5. 修 `ttl=0`、off-by-one、治理 enable no-op

### P2（尽快修）

1. 去掉冗余 Bridge 或统一入口
2. 补完整单测 / 集成测试
3. 补真实 benchmark，替换占位权重  
   参考 `entity/loyalty/src/weights.rs:1-3`
4. 修正文档漂移

---

## 七、最终判断

**这个模块当前最缺的不是“更多功能”，而是“可回滚、可清理、可审计、可兑付”的闭环。**

如果只给一句总评：

> **`pallet-entity-loyalty` 现在能演示，但还不能放心承接真实资金 / 积分权益。**

---

## 八、附：本次检查的直接结论

- `pallet-entity-loyalty` 当前没有自带测试，`cargo test -p pallet-entity-loyalty` 结果为 0 tests。
- README、边界文档与实际代码已出现明显漂移。
- runtime 当前实际走的是 `LoyaltyBridge`，而非直接使用 pallet 内部 Port 实现。
- 购物余额与 token 折扣在订单生命周期中的回滚设计缺失，是最危险的问题。

