# pallet-entity-tokensale 深度审计报告

- **审计对象**：`pallet-entity-tokensale`
- **代码位置**：`entity/tokensale/src/lib.rs`、`entity/tokensale/src/tests.rs`、`entity/tokensale/src/weights.rs`
- **审计日期**：2026-03-13
- **审计方式**：本地代码阅读 + 测试验证
- **测试验证命令**：

```bash
CARGO_TARGET_DIR=/tmp/nexus-target cargo test -p pallet-entity-tokensale --lib
```

- **测试结果**：`146/146 passed`

---

## 一、总体结论

`pallet-entity-tokensale` 已具备完整的 Token Sale 主干流程：

- 发售轮次创建
- 支付配置
- 白名单
- 荷兰拍卖
- 认购/追加认购
- 结束/取消
- 退款
- 锁仓/解锁
- 资金提取
- 存储清理
- Root 强制干预

但从**生产级安全上线**标准看，当前版本仍存在明显短板，主要集中在：

1. **权限模型错误绑定到 `round.creator`**
2. **关键资产路径缺少明确事务性保护**
3. **`Paused` / `SoldOut` 场景存在卡死**
4. **锁仓起点业务语义错误**
5. **清理后历史数据一致性不足**
6. **治理强制结束可能绕过 soft cap 经济保护**

### 综合评价

- **业务完整度**：7/10
- **生产可上线度**：4/10

一句话结论：

> 当前模块更接近“功能可跑 + 审计修补版”，还不是“可放心上线的生产级金融资产发行模块”。

---

## 二、按角色视角的业务流深度分析

---

## 1. Entity Owner / 当前实体所有者视角

### 1.1 当前业务流

Entity owner/admin 可创建发售轮次：

- `entity/tokensale/src/lib.rs:759-764`

创建时权限校验是正确的，基于：

- `entity_exists`
- `is_entity_active`
- `entity_owner`
- `is_entity_admin(...TOKEN_MANAGE)`
- `is_entity_locked`

### 1.2 关键问题：轮次控制权绑定到 `round.creator`

轮次创建完成后，大多数管理操作都不再校验“当前 entity owner/admin”，而是改成：

```rust
ensure!(round.creator == who, Error::<T>::Unauthorized);
```

典型位置：

- `entity/tokensale/src/lib.rs:846`
- `entity/tokensale/src/lib.rs:996`
- `entity/tokensale/src/lib.rs:1350`
- `entity/tokensale/src/lib.rs:1505`
- `entity/tokensale/src/lib.rs:1732`
- `entity/tokensale/src/lib.rs:2013`

### 1.3 风险

会直接导致以下问题：

1. **admin 创建的轮次，owner 反而无法管理**
2. **owner 转移后，新 owner 无法接管旧轮次**
3. **被撤权 admin 仍可继续控制旧轮次**
4. sale 管理权与 entity 所有权/治理权脱节

### 1.4 结论

这是本模块最严重的权限设计缺陷之一。

### 1.5 必须增加的功能

所有管理型 extrinsic 应改为：

- 校验**当前 entity owner**
- 或校验**当前 entity admin + `TOKEN_MANAGE`**

`round.creator` 建议保留为：

- 审计字段
- 操作来源字段

但**不应作为唯一控制权依据**。

---

## 2. Sale Admin / 被授权管理员视角

### 2.1 当前问题

管理员创建 round 后，会长期拥有 round 控制权，即使后续：

- 被 owner 撤权
- 被治理移除权限
- 不再属于实体运营团队

仍可执行：

- `start_sale`
- `end_sale`
- `cancel_sale`
- `withdraw_funds`
- `cleanup_round`

### 2.2 风险

这是典型的**权限撤销失效**问题。

### 2.3 必须增加的功能

建议：

1. 所有管理权限改为**动态查询当前权限**
2. 如确实存在“轮次负责人”业务概念，单独增加：
   - `round_manager`
   - `transfer_round_control`
3. 但即使有 round manager，也应允许当前 owner/governance 接管

---

## 3. Subscriber / 投资者视角

### 3.1 当前已支持流程

投资者可执行：

- `subscribe`
- `increase_subscription`
- `claim_tokens`
- `unlock_tokens`
- `claim_refund`

### 3.2 关键问题 A：售罄后不会自动结束

`subscribe` 认购成功后仅更新：

- `sold_amount`
- `remaining_amount`
- `participants_count`
- `RaisedFunds`

位置：

- `entity/tokensale/src/lib.rs:1105-1146`

但没有在 `remaining_amount == 0` 时自动将轮次转为 `Ended`。

### 后果

会出现：

1. sale 已经卖完
2. 投资者资金已支付
3. 但投资者仍不能 `claim_tokens`
4. 必须等待 creator/root 手动结束，或等自然到期

### 必须增加的功能

建议至少实现一种：

1. **sold-out auto finalize**
2. **permissionless finalize**（任何人可结束已售罄轮次）

---

### 3.3 关键问题 B：Paused 状态可长期卡死

`on_initialize` 仅自动处理：

- `status == Active && now > end_block`

位置：

- `entity/tokensale/src/lib.rs:707-712`

因此：

- `Paused` 轮次不会被自动结束
- 但 `Paused` 轮次仍保留在 ActiveRounds 语义中

### 后果

若 creator 在临近结束前暂停 sale：

1. 投资者不能继续认购
2. 也不能 claim token
3. 也不能走退款（若未 cancel）
4. 轮次可能长期停留在 `Paused`
5. Entity 关闭流程也会被阻塞

这是一个明显的**业务冻结漏洞**。

### 必须增加的功能

建议至少补其一：

1. `Paused` 超时自动 `Ended` / `Cancelled`
2. `Paused` 到达 end_block 后允许任何人 finalize
3. 增加 `max_pause_duration`

---

### 3.4 关键问题 C：锁仓起点使用 `subscribed_at`

`Subscription` 中记录：

- `subscribed_at`

位置：

- `entity/tokensale/src/lib.rs:233-240`

解锁计算时直接使用：

- `sub.subscribed_at`

位置：

- `entity/tokensale/src/lib.rs:1307-1313`
- `entity/tokensale/src/lib.rs:2397-2402`

### 后果

同一 sale round 中：

- 早认购的用户更早进入解锁
- 晚认购的用户更晚解锁

这通常不符合公开发售的锁仓逻辑。  
多数 IEO / 公募 round 应采用统一起点：

- sale end
- TGE
- 明确设定的 vesting start block

### 必须增加的功能

建议新增：

- `vesting_start_block`
- 或 `vesting_start_mode = SaleEnd | TGE | Manual`

---

## 4. Governance / Root 视角

### 4.1 当前能力

Root 当前可以：

- `force_cancel_sale`
- `force_end_sale`
- `force_refund`
- `force_withdraw_funds`
- `force_batch_refund`

### 4.2 高风险问题：`force_end_sale` 绕过 soft cap

`force_end_sale` 直接执行：

- 释放未售部分
- `status = Ended`

位置：

- `entity/tokensale/src/lib.rs:1587-1605`

但不检查：

- `soft_cap`

### 后果

本应因 soft cap 不达标而进入退款流程的轮次，可被 root 直接强制变成成功发售。

### 结论

这是一个**治理可绕过经济保护规则**的风险点。

### 建议

二选一：

1. **不允许绕过 soft cap**：在 `force_end_sale` 中补 soft cap 检查
2. **允许治理绕过**：则必须
   - 在文档中明确说明
   - 事件中显式标识 `soft_cap_bypassed`
   - 前端/审计系统可见

---

## 5. Compliance / 风控 / 运营视角

### 5.1 当前检查项

`subscribe` 与 `increase_subscription` 已检查：

- 时间窗口
- KYC
- Insider 黑窗限制
- 白名单

位置：

- `subscribe`: `entity/tokensale/src/lib.rs:1045-1103`
- `increase_subscription`: `entity/tokensale/src/lib.rs:1780-1821`

### 5.2 漏检项

没有在认购时重新校验：

- entity 当前是否仍 active
- entity 是否被 suspended / banned / governance locked
- token 是否仍 enabled
- disclosure penalty / high risk 是否升级

### 5.3 风险

若 sale 启动后 entity 被治理冻结：

- 理论上应停止继续吸纳资金
- 但当前代码未强制联动，仍可能继续认购

### 5.4 必须增加的功能

建议增加运行态联动：

1. 认购前重检 entity/token 状态
2. 治理状态变化时自动 pause/cancel sale
3. 高风险 penalty 生效时禁入新认购

---

## 6. Registry / 其他模块集成视角

Registry 在实体关闭前会检查：

- `!T::TokenSaleProvider::has_active_sale(entity_id)`

位置：

- `entity/registry/src/helpers.rs:366-388`

而 `TokenSaleProvider::has_active_sale` 默认基于：

- `active_sale_round(entity_id).is_some()`

位置：

- `entity/common/src/traits/incentive.rs:88-90`

tokensale 实现中将以下都视为活跃：

- `Active`
- `Paused`

位置：

- `entity/tokensale/src/lib.rs:2366-2377`
- `entity/tokensale/src/lib.rs:2412-2419`

### 风险

只要 round 长期 `Paused`：

- entity 就可能始终无法关闭
- 形成跨模块的僵尸依赖

---

## 三、必须增加的功能清单

---

## P0（上线前必须补）

### 1. 权限模型重构

把所有管理型 extrinsic 从：

- `round.creator == who`

改成：

- 当前 owner
- 或当前 admin + `TOKEN_MANAGE`

---

### 2. Permissionless finalize

必须允许：

- 售罄后任何人 finalize
- 到期后任何人 finalize
- `Paused` 到期/超时后任何人 finalize 或 cancel

---

### 3. 统一锁仓起点

新增：

- `vesting_start_block`
- 或 `vesting_start_mode`

默认不应按 `subscribed_at` 起算。

---

### 4. 事务性保护

当前代码中未看到 `#[transactional]`。

高风险路径包括：

- `create_sale_round`：`entity/tokensale/src/lib.rs:804-813`
- `start_sale`：`entity/tokensale/src/lib.rs:1016-1027`
- `claim_tokens`：`entity/tokensale/src/lib.rs:1263-1276`
- `unlock_tokens`：`entity/tokensale/src/lib.rs:1317-1328`
- `claim_refund`：`entity/tokensale/src/lib.rs:1398-1412`
- `force_refund`：`entity/tokensale/src/lib.rs:1637-1649`

建议：

1. 关键资金/代币路径加事务保护
2. 或彻底重构为“先校验、再原子写入”
3. 保证任何失败不会留下半成功状态

---

### 5. Paused 超时处理

必须补：

- `max_pause_duration`
- 或 `pause_deadline`
- 或到期自动结束/取消

---

## P1（强烈建议尽快补）

### 6. 单实体并发轮次策略

当前设计允许一个 entity 同时存在多个 active round，  
但 Provider 只返回一个：

- trait：`entity/common/src/traits/incentive.rs:72-104`
- impl：`entity/tokensale/src/lib.rs:2411-2454`

这会导致外部模块对“当前 round”认知不可靠。

建议二选一：

1. 限制一个 entity 同时仅允许 1 个 active round
2. Provider 改为返回活跃 round 列表

---

### 7. cleanup 后历史归档

`cleanup_round` 会删除：

- `RoundParticipants`
- `Subscriptions`
- `RoundWhitelist`
- `RoundPaymentOptions`
- `RaisedFunds`

位置：

- `entity/tokensale/src/lib.rs:2020-2035`

但不会删除 `SaleRounds` 主体。  
因此 cleanup 后再调用统计查询：

- `entity/tokensale/src/lib.rs:2352-2361`

会得到一个“半残留 round”。

#### 具体后果

- `raised_nex` 会变成 0
- `payment_options_count` 仍保留旧值
- round 看似存在，但明细已被清空

#### 建议

增加：

- `SaleRoundArchive`
- 或 cleanup 时彻底删除 round
- 或在 cleanup 前固化统计快照

---

### 8. 运行态风险联动

需要在 sale 运行过程中联动：

- entity governance lock
- entity suspended/banned
- token disabled
- disclosure penalty

---

### 9. 轮次级资金隔离

当前所有 round 共用：

- `pallet_account()`

位置：

- `entity/tokensale/src/lib.rs:2131-2133`

建议改为：

1. 每个 round 独立派生 escrow account
2. 或 round 级 hold/reserve 资金隔离

---

## P2（中期优化）

### 10. 投资者撤单 / 减仓能力

当前用户：

- 可以认购
- 可以加仓
- 不能减仓
- 不能在未结束前撤销认购

建议增加：

- `decrease_subscription`
- `cancel_subscription_before_end`

---

### 11. 更完整的只读查询接口

建议新增：

- round 是否可 finalize
- 用户是否可 claim
- 用户当前可 refund 金额
- 用户当前可 unlock 数量
- round 是否可 clean

---

### 12. 风险事件增强

建议增加事件：

- `SaleFinalizedByThirdParty`
- `SalePausedByGovernance`
- `SoftCapBypassedByRoot`
- `RoundControlTransferred`
- `SaleAutoCancelledDueToPauseTimeout`

---

## 四、冗余 / 半成品功能

---

## 1. `FCFS` 基本无独立业务逻辑

虽然定义了：

- `SaleMode::FCFS`

位置：

- `entity/tokensale/src/lib.rs:84-98`

但后续主要只有以下模式有专门逻辑：

- `DutchAuction`
- `WhitelistAllocation`

`FCFS` 实际上几乎等于 `FixedPrice` 的别名。

### 建议

二选一：

1. 删除 `FCFS`
2. 补齐 FCFS 专属逻辑（如售罄即止、抢购约束、排队等）

---

## 2. `VestingType::Custom` 未真正实现

定义位置：

- `entity/tokensale/src/lib.rs:118-145`

但在解锁计算中，`Custom` 没有独立处理，实质上走的是线性类路径：

- `entity/tokensale/src/lib.rs:2239-2250`

### 结论

这是一个典型“伪功能”。

### 建议

二选一：

1. 删除 `Custom`
2. 真正实现自定义释放曲线 / 分段计划

---

## 3. 多资产支付当前是伪实现

虽然代码保留了：

- `AssetId`
- `payment_asset`
- `RaisedFunds(round_id, asset_id)`

但添加支付项时强制：

- `asset_id.is_none()`

位置：

- `entity/tokensale/src/lib.rs:839-840`

### 结论

当前实际上只支持原生 NEX。  
多资产结构属于未完成脚手架。

### 建议

二选一：

1. 暂时删除多资产结构，降低复杂度
2. 正式实现多资产支付与退款/提取闭环

---

## 4. 部分错误码冗余

当前未实际使用的错误码包括：

- `RoundNotStarted`
- `RoundEnded`
- `RoundCancelled`
- `InsufficientBalance`
- `SoftCapNotMet`（仅 event 使用，未见实际 `return Err`）

建议清理。

---

## 五、明确的代码 Bug / 设计缺陷

---

## 高优先级

### Bug 1：权限粘附在 `creator`

**影响**：

- owner 转移后新 owner 无法接管
- admin 撤权后旧 admin 仍可控制

---

### Bug 2：售罄不自动结束

**影响**：

- 投资者付款后无法及时领取代币

---

### Bug 3：Paused 可永久冻结

**影响**：

- 轮次卡死
- 投资者卡死
- Entity 关闭卡死

---

### Bug 4：锁仓起点错误

**影响**：

- 同一轮不同投资者解锁节奏不一致

---

### Bug 5：cleanup 破坏历史统计

**影响**：

- 清理后历史查询失真

---

## 中优先级

### Bug 6：`force_end_sale` 绕过 soft cap

**影响**：

- 治理可修改 sale 成败逻辑

---

### Bug 7：多 active round 与 singular provider 冲突

**影响**：

- 外部模块无法准确表达“当前所有活跃 round”

---

### Bug 8：认购时不重检 entity/token 运行态风险

**影响**：

- sale 启动后 entity 风险升级，仍可能继续吸纳资金

---

### Bug 9：部分 `unreserve` 不完整时仅记录日志但继续流转

例如：

- `entity/tokensale/src/lib.rs:1214-1220`
- `entity/tokensale/src/lib.rs:1358-1366`
- `entity/tokensale/src/lib.rs:1557-1563`
- `entity/tokensale/src/lib.rs:2310-2316`

当前仅 `log::warn!`，但状态仍继续进入终态。  
这会造成账实不完全一致时业务继续推进。

---

## 六、安全漏洞 / 攻击面分析

---

## 1. 权限漂移漏洞

旧 admin / 旧 owner 保留对历史 round 的实际控制权。  
这是**权限撤销失效**漏洞。

---

## 2. 业务冻结漏洞

creator 可通过 `pause_sale` 长期冻结 round。  
属于**可利用卡死漏洞**。

---

## 3. 治理绕过经济规则漏洞

root 可通过 `force_end_sale` 绕过 soft cap。  
属于**治理强权限改变募资成败规则**风险。

---

## 4. 非原子 side-effect 风险

多条关键路径存在：

- 先动 token/NEX
- 后更新状态

若中途失败，存在半成功风险。  
这是金融资产模块中必须优先修复的问题。

---

## 5. 权重低估 / DoS 风险

`weights.rs` 中多个 O(n) 操作给的是近似固定权重：

- `add_to_whitelist`：`entity/tokensale/src/weights.rs:92-99`
- `remove_from_whitelist`：`entity/tokensale/src/weights.rs:225-230`
- `cleanup_round`：`entity/tokensale/src/weights.rs:265-273`

但真实代码存在循环/清前缀操作：

- `entity/tokensale/src/lib.rs:970-975`
- `entity/tokensale/src/lib.rs:1877-1883`
- `entity/tokensale/src/lib.rs:2021-2035`

建议重新 benchmark，并按规模参数化权重。

---

## 七、整改优先级建议

---

## P0（必须立即改）

1. 将权限模型从 `round.creator` 改为 entity 当前 owner/admin
2. 给关键资金/代币路径补事务性保护
3. 增加 sold-out auto finalize / permissionless finalize
4. 增加 paused timeout 机制
5. 将 vesting 起点改为 round/TGE 级统一起点

---

## P1（第二阶段）

6. 限制单实体单 active round，或 Provider 改为多 round
7. cleanup 改为 archive 模式
8. sale 与 entity/token/governance 风险状态联动
9. 引入 round 级资金隔离

---

## P2（优化阶段）

10. 支持减仓/撤单
11. 扩展只读 API
12. 增强事件与审计轨迹

---

## 八、建议新增测试清单

建议补充但当前未充分覆盖的测试：

1. **owner 转移后新 owner 可接管旧 round**
2. **admin 撤权后不可继续管理旧 round**
3. **sold out 后任意人可 finalize**
4. **paused 超时自动结束/取消**
5. **同 round 统一 vesting 起点**
6. **cleanup 后历史统计仍可正确查询**
7. **force_end_sale 遇 soft cap 的预期行为**
8. **entity 被 lock/suspend 后 subscribe 被拒**
9. **关键 extrinsic 的事务回滚测试**
10. **大白名单 / 大 cleanup 的 weight 边界测试**

---

## 九、最终结论

`pallet-entity-tokensale` 当前不是“不能用”，但也绝不是“可以放心上线”的状态。

它已经具备 sale 的核心骨架，但仍存在多个足以在真实发售中引发严重问题的设计缺陷：

- 权限主体与 entity 治理体系脱节
- 投资者资金/代币流程可被卡死
- 治理操作可能绕过经济保护规则
- 清理后历史统计不可完全信任
- 风险状态联动不完整

### 最终建议

上线前至少完成以下五项：

1. 权限模型重构
2. 事务性保护
3. sold-out / paused 卡死修复
4. 统一 vesting 起点
5. cleanup/history 语义修复

否则模块虽“功能可跑”，但在真实募资环境下会暴露出明显的治理、合规、用户资产与可运维性风险。

