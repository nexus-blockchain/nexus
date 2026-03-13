# pallet-commission-single-line 深度业务流审计分析

审计日期：2026-03-13  
模块路径：`entity/commission/single-line`

---

我看了 `entity/commission/single-line` 的 `README / lib.rs / tests.rs / mock.rs`，也看了 `commission-core` 和 runtime 接线，并跑了 `cargo test`：**163/163 通过**。  
但从“各角色业务流”看，这个模块还有几处**上线级 P0/P1 问题**。

## 总结结论

这个 pallet **代码完成度高**，但目前更像“可运行原型”，还不是严格符合“按首次消费顺序入链”的生产实现。  
最大的风险有 5 个：

---

## 一、按角色看，最关键的问题

### 1) 新买家视角

**P0：首单不会给既有上线发单线佣金。**  
README 说“用户首次消费时按顺序加入”“按首次消费时间排列”（`entity/commission/single-line/README.md:9-33`），  
但实际代码是：

- 先算佣金：`src/lib.rs:969-979`
- 最后才尝试入链：`src/lib.rs:998-1004`

也就是新买家首单时，`process_upline/process_downline` 先查不到自己的 index，直接返回（`src/lib.rs:747-749`, `794-796`）。  
**结果：首单只入链，不产出单线佣金。**

> 这会直接削弱“早期消费者吃后续消费”的核心激励。

---

### 2) 老会员 / 上下线收益者视角

**P0：入链与发佣强耦合，导致排位可被“暂停/未配置”阶段扭曲。**  
`do_calculate` 在以下情况直接 return，不会入链：

- Entity inactive：`src/lib.rs:952-954`
- 单线 pause：`src/lib.rs:955-957`
- 没配置：`src/lib.rs:958-960`
- mode 没开：`src/lib.rs:963-967`

这意味着实际排位不是“首次消费顺序”，而是**首次在 single-line 可用时的消费顺序**。  
Owner/Admin 甚至可以用 `pause/resume`（`src/lib.rs:500-530`）人为影响谁先入链。

> 这对老会员是核心公平性漏洞。

---

### 3) Entity Owner/Admin 视角

**P0：待生效配置会“复活/覆盖”当前配置。**  
`schedule_config_change` 写入 `PendingConfigChanges`（`src/lib.rs:558-562`），  
但后续：

- `set_single_line_config` 不清 pending（`src/lib.rs:312-318`）
- `clear_single_line_config` 不清 pending（`src/lib.rs:334-336`）
- `update_single_line_params` 也不清 pending（`src/lib.rs:364-388`）

而 `apply_pending_config` 又是**任何签名账户都能调用**（`src/lib.rs:569-598`）。

所以会出现：

1. 管理员先 schedule 老配置 A  
2. 后来直接 set/update 成新配置 B  
3. 到期后任何人都能 apply A，把 B 覆盖掉

这是很实质的业务漏洞。

---

### 4) Governance / Root / 运维视角

**P1：`force_reset_single_line` 不是“真正 reset”。**  
它只删 segment 和 index（`src/lib.rs:836-859`），  
但**不会清 `RemovedMembers`**。而收益计算时又会跳过 `RemovedMembers`（`src/lib.rs:734-739`）。

所以如果某用户之前被 `force_remove`（`src/lib.rs:620-642`），  
即使后来整条线 reset 后重新加入，仍可能一直被跳过。

> reset 后状态残留，运维会非常难查。

---

### 5) Token 用户视角

**P1：Token 单线的“动态层数增长”，实际读取的是 NEX 统计。**  
单线层数增长取自 `StatsProvider`（`src/lib.rs:753-755`, `801-803`）。  
runtime 里这个 provider 被接到 **`pallet_commission_core::MemberCommissionStats`**（NEX）：
`runtime/src/configs/mod.rs:2025-2031`。  
但 token 管线也直接复用 single-line：
`entity/commission/core/src/engine.rs:529-533`。

> 结果：Token 订单的单线层数成长，跟 NEX 返佣累计绑定，而不是 token 自己的累计。  
> 这会造成跨资产激励失真。

---

## 二、明确的代码 BUG / 漏洞

### P0

1. **首单不发单线佣金**  
   - 证据：`src/lib.rs:969-1004`, `747-749`
2. **排位不是“首次消费顺序”，而是“single-line 生效后的首次消费顺序”**  
   - 证据：`src/lib.rs:952-967`, `998-1004`
3. **待生效配置可覆盖新配置/清空后的配置**  
   - 证据：`src/lib.rs:558-562`, `573-589`，以及 set/clear/update 不清 pending
4. **`apply_pending_config` 不重新校验当前实体状态/当前规则**  
   - 证据：`src/lib.rs:573-589`
5. **等级覆盖校验可绕过**  
   - `set_level_based_levels` 只有“有 config 时”才校验 max：`src/lib.rs:406-413`
   - `update_single_line_params` 降低 max 时不回扫已有 overrides：`src/lib.rs:364-377`
   - `SingleLinePlanWriter::set_level_based_levels` 完全不校验 max：`src/lib.rs:1082-1094`

### P1

6. **reset 后 `RemovedMembers` 残留**
   - 证据：`src/lib.rs:620-642`, `734-739`, `836-859`
7. **preview API 会误导前端**
   - `preview_single_line_commission` 不看 mode、不看 paused、不看 entity active、不看真实 remaining：`src/lib.rs:923-934`
   - 真实分配顺序在 core 里还会受 creator/referral/multi-level/level-diff 抢预算：`entity/commission/core/src/engine.rs:157-234`
8. **文档/实现不一致**
   - README 说 10/12 都受 `EntityActive + EntityNotLocked` 守卫：`README.md:188-195`
   - 但 `cancel_pending_config` 实现没有这两个检查：`src/lib.rs:604-613`

---

## 三、必须补的功能

### 1. 把“入链登记”从“发佣计算”里拆出来

**这是第一优先级。**  
要新增一个独立的“消费入链 hook / 订单完成 hook”：

- 首次符合条件消费时就登记排位
- 不依赖 single-line 当前是否 pause / 是否开 mode / 是否已有 config
- 允许历史订单回填 / 批量导入

否则“首次消费顺序”这个业务前提本身就是假的。

---

### 2. 待生效配置要版本化/失效化

至少要补：

- `pending_id/version`
- 直接 `set/update/clear` 时自动废弃旧 pending
- `apply_pending_config` 时重新校验当前实体状态、当前 runtime 规则
- 最好记录 proposer / applier

---

### 3. 加入反女巫/占位门槛

现在任何消费都可入链，`add_to_single_line` 没有最低金额/KYC/激活门槛（`src/lib.rs:688-724`, `998-1004`）。  
必须至少有一个：

- 最低首单金额
- 会员激活后才入链
- KYC/实名后才入链
- 每身份只能占一个排位

否则很容易被小号/微额订单占坑。

---

### 4. 增加“压缩/重建/补位”能力

当前逻辑移除只会制造“死坑位”，不会整理。  
需要：

- 管理员可发起链重建/压缩
- 可选“跳过失效成员后继续向外搜索有效受益人”
- 满队列后的 waitlist / rebuild / hard-fail 策略

---

### 5. Governance 能力要补齐

现在治理口只支持设置 rate/base/max，**不支持 `level_increment_threshold`**，而实现还会强制把 threshold 设成 0：  
- trait：`entity/common/src/traits/governance_ports.rs:83-89`
- impl：`src/lib.rs:1133-1153`

这会让治理配置出来的单线，**动态层数增长直接失效**。

---

### 6. 审计与查询能力要补

`ConfigChangeLogEntry` 只有配置快照和区块号，没有：

- 谁改的
- 是 set/update/apply/cancel 哪种动作
- 对应 pending 的版本号

见：`src/lib.rs:80-89`, `870-881`

这对 Owner/Admin/审计都不够。

---

## 四、冗余/设计重复点

1. **配置写入路径重复太多**
   - `set_single_line_config`：`src/lib.rs:308-318`
   - `force_set_single_line_config`：`452-462`
   - `apply_pending_config`：`581-597`
   - `SingleLinePlanWriter::set_single_line_config`：`1059-1069`
   - `governance_set_single_line_config`：`1144-1159`

   这些路径都在“手写 insert + log + emit event”，已经造成校验不一致。

2. **等级来源重复**
   - single-line 自己定义了 `SingleLineMemberLevelProvider`：`src/lib.rs:140-142`
   - 系统已有 `MemberProvider::custom_level_id`：`entity/common/src/traits/member.rs:207-208`

   两套来源未来很容易漂移。

3. **上下线遍历代码几乎完全重复**
   - `process_upline` / `process_downline`：`src/lib.rs:741-834`

   维护成本高，修 bug 容易只修一边。

---

## 五、我建议的修复优先级

### P0 先修

1. 入链登记与发佣解耦
2. pending config 版本化 + 自动失效
3. 等级覆盖全路径统一校验
4. 首单单线收益补上

### P1 再修

5. reset/full-clear 清理残留状态
6. Token 与 NEX 分离层数统计
7. anti-sybil / 满队列策略 / 补位压缩
8. preview / audit / governance 能力补齐

---

## 附：本次核验

- 已阅读：
  - `entity/commission/single-line/README.md`
  - `entity/commission/single-line/src/lib.rs`
  - `entity/commission/single-line/src/tests.rs`
  - `entity/commission/single-line/src/mock.rs`
  - `entity/commission/core/src/engine.rs`
  - `runtime/src/configs/mod.rs`
  - `entity/common/src/traits/governance_ports.rs`
  - `entity/common/src/traits/member.rs`
- 已执行：
  - `cargo test`
- 结果：
  - **163 tests passed**

