# pallet-commission-common 深度分析（2026-03-13）

## 分析范围

本次分析重点覆盖以下文件与集成点：

- `entity/commission/common/src/lib.rs`
- `entity/commission/core/src/lib.rs`
- `entity/commission/core/src/engine.rs`
- `entity/commission/core/src/runtime_api.rs`
- `entity/commission/referral/src/lib.rs`
- `entity/governance/src/lib.rs`

并验证执行了：

- `cargo test -p pallet-commission-common`
- `cargo test -p pallet-commission-core f2_member_commission_order_ids_deduplicates`

---

## 总结结论

`pallet-commission-common` 虽然名义上只是“共享类型 + trait”层，但实际上已经是整套返佣系统的**契约层**。  
当前最大问题不在语法，而在于**接口契约设计不严**，并且已经在 `core / governance / referral` 中暴露出以下问题：

1. 真实功能失效
2. 重复记账风险
3. 业务语义漂移
4. NEX / Token 双资产接口不对称
5. 大量 fail-open / no-op 默认实现带来的高风险

---

# 一、从不同角色业务流视角看：还必须增加哪些功能

## 1）买家 / 普通会员视角

业务流：下单 → 触发返佣 → 以后提现 / 复购 / 消费购物余额

### 必须增加

- **提现预览接口**
  - 提现前应能看到：`可提金额 / 强制复购 / 自愿奖励 / 冷却剩余区块 / 被拒原因`
- **订单返佣明细查询**
  - 当前更偏聚合统计，缺标准化“我的返佣历史”
- **部分退款 / 售后调整接口**
  - 现仅有整单级：`process_commission / cancel_commission(order_id) / settle_order_commission(order_id)`
  - 缺少部分退款、部分逆向结算
- **复购判定解释接口**
  - 用户需要知道为什么这单被算作复购，或为什么没算作复购

### 当前问题

- `MemberCommissionStatsData.order_count` 被混入统计结构，但在 core 中它主要是给**买家自己**递增，不等于“该账号佣金关联订单数”
- `REPEAT_PURCHASE` 使用 `buyer_order_count`，而该值来自 core 的处理次数，不是 completed orders

---

## 2）推荐人 / 多级上级 / 团队长 / 单线上下线视角

业务流：他人下单 → 我获佣 → 查询 → 提现

### 必须增加

- **按返佣类型拆分查询**
  - 直推 / 多级 / 团队 / 极差 / 单线 / 创建人 / 招商推荐
- **按资产拆分查询**
  - NEX 和 Token 当前缺少统一的 per-type / per-asset 查询
- **资格失败原因查询**
  - 例如：未激活、KYC 不通过、封禁、层级不够、上限打满
- **历史流水接口**
  - 当前聚合多、流水少，不利于客服与对账

### 当前问题

- `ReferralQueryProvider` 仅覆盖 NEX 风格的 referral 查询，没有 token 版专门查询
- dashboard 中 `referral` 也是单 Balance，不是双资产

---

## 3）创建人 / Entity 招商推荐人视角

业务流：创建人收益 / 平台费分成 → 查询 → 提现

### 必须增加

- **CreatorReward 专属查询**
- **EntityReferral 专属查询**
- **资金来源快照**
  - 标记来自平台费池还是卖家佣金池

### 当前问题

- `CommissionType` 中已经有 `CreatorReward` / `EntityReferral`
- 但 common 层没有对应专门 `QueryProvider`
- 记录中也没有 `fund_source / rate_snapshot / config_version`

---

## 4）Entity Owner / Admin 视角

业务流：配置模式 → 调整费率 → 查看总览 → 售后撤销 / 归档 / 提现自由资金

### 必须增加

- **统一配置读取接口**
  - 目前写接口很多，读接口分散
- **配置版本 / 生效版本快照**
- **幂等保护**
  - 同一 `order_id` 不应允许重复 `process_commission`
- **资产能力校验**
  - 哪些模式仅支持 NEX，哪些同时支持 Token，需要明确失败而非静默跳过
- **售后调整接口**
  - 不能只支持整单 cancel

### 当前问题

- 同一 `CommissionModes` 同时驱动 NEX / Token
- 但 Token 实际并不支持固定金额等部分模式，Owner 很容易误配

---

## 5）治理 / DAO 视角

业务流：提案通过 → 链上执行配置变更

### 必须增加

- **强制实现的治理接口**
  - 不能默认 no-op
- **完整链上可执行的提现配置变更**
  - 不能只改 `enabled`
- **治理执行事件**
  - 必须可审计

### 当前严重 BUG

- `entity/commission/common/src/lib.rs:300-309`
  - `governance_set_commission_rate` / `governance_toggle_commission` 默认 `Ok(())`
- `entity/governance/src/lib.rs:3125-3129`
  - 治理执行中真的会调用这两个接口
- `entity/commission/core/src/lib.rs:2279-2398`
  - core 的 `CommissionProvider` 实现**没有 override 这两个方法**

### 实际结果

**DAO 提案可以显示“执行成功”，但实际上没有改任何配置。**

这是 **P0 级真实功能失效**。

---

## 6）客服 / 财务 / 审计视角

业务流：查账 → 撤销 → 归档 → 对账

### 必须增加

- **record_id**
- **状态时间戳**
  - `created / settled / paid_out / cancelled / archived`
- **配置快照**
  - 当时的 `rate / mode / depth / cap`
- **不可丢失的历史归档**
  - 不能只是简单 remove 原记录

### 当前问题

- `CommissionStatus::Withdrawn` 当前实际被当作“订单已完结，可归档”
- 它不是“会员真的提现过”
- `archive_order_records` 允许把 `Withdrawn / Cancelled` 记录直接删除

这会造成**审计语义错误**。

---

# 二、明确识别出的 BUG / 漏洞

## P0-1：治理提案实际 no-op

### 证据位置

- `entity/commission/common/src/lib.rs:300-309`
- `entity/governance/src/lib.rs:3125-3129`
- `entity/commission/core/src/lib.rs:2279-2398`

### 问题描述

治理改佣金率、总开关的提案，默认实现直接 `Ok(())`，而 core 没有覆盖实现。

### 影响

- 治理层以为修改成功
- 链上状态实际上不变
- 造成治理执行结果与真实业务状态分离

---

## P0-2：同一订单可重复处理，存在重复记账 / 重复扣款风险

### 证据位置

- `entity/commission/core/src/engine.rs:55-235`
  - `process_commission` 没有任何 `order_id` 幂等检查
- `entity/commission/core/src/tests.rs:4278-4291`
  - 同一个 `7010` 被处理两次，测试依然通过

### 问题描述

没有“订单已处理”标记，也没有“重复处理拒绝”逻辑。

### 影响

- 平台费、卖家池、pending、records 都可能被重复增加
- 这是**资金级风险**

---

## P0-3：复购奖励可被“取消订单刷次数”污染

### 证据位置

- `entity/commission/core/src/engine.rs:267-270`
  - 处理后立即 `order_count += 1`
- `entity/commission/core/src/engine.rs:724-760`
  - `cancel_commission` 路径没有回滚这个计数
- `entity/commission/referral/src/lib.rs:785-865`
  - `REPEAT_PURCHASE` 使用的正是这个 `buyer_order_count`
- `entity/commission/referral/src/lib.rs:840-841`
  - `FIRST_ORDER` 却改为使用 `completed_order_count`

### 问题描述

同一个系统内：

- 首单判断按“已完成订单”
- 复购判断按“已处理订单计数”

这两个标准不一致。

### 影响

买家可通过取消订单堆高次数，提前触发复购奖励阈值。  
这是**真实业务漏洞**。

---

## P1-1：`CommissionModes::contains()` 语义存在误导

### 证据位置

- `entity/commission/common/src/lib.rs:56-58`

### 问题描述

当前实现是：

- 只要有任意一位重叠就返回 true

这更像 `intersects(flag)`，并不是通常语义的“包含全部位”。

### 风险

当前很多调用传的是单 bit，暂时不炸。  
但未来如果传组合位判断，会产生隐蔽错误。

---

## P1-2：插件输出契约没有不变量校验

### 证据位置

- `CommissionPlugin` / `TokenCommissionPlugin` 定义在 `entity/commission/common/src/lib.rs`
- core 在 `entity/commission/core/src/engine.rs:174-234` 及 token 对称路径完全信任插件返回

### 问题描述

如果插件返回：

- `outputs` 总额大于原始 `remaining`
- 或 `new_remaining` 与 outputs 不一致

core 没有做一致性校验。

### 风险

这是典型的**模块间契约脆弱性**，某个插件升级后可能直接破坏主资金流。

---

## P1-3：状态命名错误，审计语义错位

### 证据位置

- `entity/commission/common/src/lib.rs:90-99`
- `entity/commission/core/src/engine.rs:25-45`
- `entity/commission/core/src/lib.rs:1988-2034`

### 问题描述

`Withdrawn` 现在实际含义更接近：

- “订单佣金已结算”
- “可归档”

而不是：

- “会员已提现”

### 风险

对账、审计、前端展示、历史查询都会出现概念混乱。

---

# 三、冗余 / 设计重复问题

## 1）NEX / Token trait 与数据结构重复过多

体现在：

- `CommissionPlugin` vs `TokenCommissionPlugin`
- `CommissionRecord` vs `TokenCommissionRecord`
- `MemberCommissionStatsData` vs `MemberTokenCommissionStatsData`

### 后果

- Token provider 能力缺失更容易出现
- Token referral 与 NEX referral 行为分叉
- 文档与前端更容易误判

---

## 2）治理接口碎片化

治理能力分散在：

- `CommissionProvider`
- 各种 `PlanWriter`
- `pallet-entity-common::CommissionGovernancePort`

### 后果

接口责任边界不清，已直接导致治理 no-op 这种严重问题。

---

## 3）`is_first_order` 参数基本冗余

在 common trait 中有：

- `is_first_order`
- `buyer_order_count`

但实际：

- referral 自己重算首单
- 其他插件大多忽略
- single-line 也未真正依赖这个值建立业务闭环

### 结论

当前公共上下文设计已经失真，更适合改为统一 `OrderCommissionContext`。

---

## 4）`Distributed` 状态基本是死状态

- 兼容 SCALE 编码保留可以理解
- 但业务层继续暴露它，会增加理解成本和前端状态分支复杂度

---

# 四、我认为必须增加的接口 / 能力

## P0 必加

1. **幂等处理**
   - `process_commission` / `process_token_commission` 必须防重复 `order_id`
2. **治理强制接口**
   - 去掉 `governance_set_commission_rate` / `governance_toggle_commission` 默认 no-op
3. **修正复购计数语义**
   - 改成 completed orders，或明确区分：
     - `processed_order_count`
     - `completed_order_count`
4. **插件不变量校验**
   - 强制校验：`sum(outputs) + new_remaining == old_remaining`
5. **修正 `contains()` 语义**
   - 改为全包含
   - 另增加 `intersects()`

## P1 必加

6. **统一订单上下文结构体**
   - 替换散乱参数
7. **统一双资产能力矩阵**
   - 明确哪些模式支持 Token，哪些不支持
8. **统一治理端口**
   - common 层不要再混用 provider / writer / external port
9. **标准化历史流水查询**
   - 覆盖 member / referrer / creator / entity_referrer / owner
10. **部分退款 / 部分逆向结算**
   - 不能只有整单 cancel

## P2 必加

11. **记录快照**
   - `fund_source`
   - `rate_bps`
   - `config_version`
   - `settled_at / paid_out_at / cancelled_at`
12. **LevelDiff / CreatorReward / EntityReferral 查询接口**
13. **Token referral 专属查询接口**

---

# 五、最值得马上修的 5 个点

1. **修 DAO no-op**
2. **加 `order_id` 幂等保护**
3. **修复 `repeat_purchase` 计数逻辑**
4. **修复 `CommissionModes::contains`**
5. **把 fail-open 默认实现改成 fail-closed，或仅 test 环境可用**

---

# 六、关键代码证据摘录（按问题聚类）

## 1）治理 no-op 问题

### common 默认实现

- `entity/commission/common/src/lib.rs:300-309`

问题：

- `governance_set_commission_rate`
- `governance_toggle_commission`

默认直接 `Ok(())`

### governance 实际调用

- `entity/governance/src/lib.rs:3125-3129`

说明：

- 治理提案执行时真的依赖这两个接口

### core 未覆盖实现

- `entity/commission/core/src/lib.rs:2279-2398`

说明：

- `CommissionProvider` impl 中没有 override 这两个方法

---

## 2）订单重复处理问题

### 处理路径

- `entity/commission/core/src/engine.rs:55-235`

问题：

- `process_commission` 无 order 幂等校验

### 已有测试证明系统允许重复调用

- `entity/commission/core/src/tests.rs:4278-4291`

说明：

- 同一个订单号重复处理，测试依然通过

---

## 3）复购计数问题

### core 中递增 order_count

- `entity/commission/core/src/engine.rs:267-270`

### referral 中复购奖励依赖 buyer_order_count

- `entity/commission/referral/src/lib.rs:782-865`

### cancel 路径未回滚 order_count

- `entity/commission/core/src/engine.rs:724-760`

---

## 4）Token 与 NEX 语义不对称问题

### common 共享模式位

- `entity/commission/common/src/lib.rs:19-49`

### token referral 明确跳过 fixed amount

- `entity/commission/referral/src/lib.rs:900-901`
- `entity/commission/referral/src/lib.rs:1035`

说明：

- `FIXED_AMOUNT` 在模式层是统一位标志
- 但 token 侧实际静默跳过

---

## 5）治理提现配置能力不足

### governance 执行时仅改 enabled

- `entity/governance/src/lib.rs:3018-3023`

说明：

- `WithdrawalConfigChange` 当前只调用 `set_withdrawal_config_by_governance(entity_id, enabled)`
- 不能完整下发 tier / mode / voluntary bonus 等配置

---

## 6）dashboard / 查询能力不够

### 会员仪表盘聚合逻辑

- `entity/commission/core/src/lib.rs:2484-2555`

### 直推详情逻辑

- `entity/commission/core/src/lib.rs:2604-2638`

说明：

- 有基础聚合
- 但缺少标准化流水、分类型、分资产、失败原因查询

---

# 七、最终判断

`pallet-commission-common` 当前不是“简单通用库”的问题，而是：

> 它已经成为整个返佣系统最核心的接口边界层，但这个边界层还没有做到：
>
> - 契约严格
> - 治理完备
> - 双资产对称
> - 审计友好
> - 幂等安全

因此，从所有不同角色用户视角出发，当前最急需补强的不是单一功能点，而是以下四类基础能力：

1. **强约束接口契约**
2. **安全幂等与正确计数**
3. **完整查询与审计快照**
4. **治理接口强制落地而非 no-op**

---

## 建议后续动作

建议下一步输出一份可直接排期的修复文档，按如下格式整理：

- 问题编号
- 风险等级
- 影响角色
- 触发路径
- 证据代码位置
- 修复方案
- patch 草案

这样可以直接转为研发任务单与审计修复计划。
