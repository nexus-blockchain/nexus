# pallet-entity-disclosure 按优先级全部修复方案（2026-03-13）

## 1. 文档目的

本文档将 `pallet-entity-disclosure` 从多角色、多业务流视角梳理出的缺陷，整理为一份可执行的修复计划，按 **P0 / P1 / P2** 优先级排序，优先解决：

- 可被直接利用的业务绕过
- 可导致错误处罚的规则漏洞
- 审批、披露、处罚三条主链路未闭环问题
- 与 token / market / governance / runtime 的联动缺失

---

## 2. 审查范围

本轮分析基于以下代码与文档：

- `entity/disclosure/src/lib.rs`
- `entity/disclosure/src/tests.rs`
- `entity/common/src/traits/compliance.rs`
- `entity/common/src/traits/hooks.rs`
- `entity/token/src/lib.rs`
- `entity/market/src/lib.rs`
- `entity/governance/src/lib.rs`
- `../runtime/src/configs/mod.rs`

本地验证：

```bash
CARGO_TARGET_DIR=/tmp/nexus-target cargo test --manifest-path ../Cargo.toml -p pallet-entity-disclosure
```

结果：**234/234 tests passed**

结论：当前问题主要不是“编译/单测失败”，而是 **业务闭环、权限闭环、合规闭环不完整**。

---

## 3. 总体判断

`pallet-entity-disclosure` 当前属于：

- **功能面很宽**
- **规则面有不少半成品**
- **关键流程存在可绕过点**
- **跨模块联动弱**

最严重的 4 个问题：

1. **管理员可通过重新配置刷新披露 deadline，而不必真实披露**
   - `entity/disclosure/src/lib.rs:1250-1264`
2. **任何人都可在弱证据条件下直接累计违规并触发处罚升级**
   - `entity/disclosure/src/lib.rs:2304-2349`
   - `entity/disclosure/src/lib.rs:3216-3254`
3. **审批流可被绕过，且改稿后旧审批仍有效**
   - `entity/disclosure/src/lib.rs:1277-1368`
   - `entity/disclosure/src/lib.rs:1432-1465`
   - `entity/disclosure/src/lib.rs:1499-1523`
4. **处罚体系大多停留在“记录状态”，未形成真实业务约束**
   - `entity/common/src/traits/compliance.rs:106-109`
   - `entity/governance/src/lib.rs:3296-3299`
   - `../runtime/src/configs/mod.rs:2120`

---

## 4. 多角色 / 业务流视角问题归纳

### 4.1 实体 Owner / Admin / 披露管理员

主流程：

- 配置披露
- 创建/更新/发布披露
- 配审批
- 管理内幕人员
- 管理黑窗口期

关键问题：

1. `configure_disclosure` 可刷新 deadline，形成“续命”
   - `entity/disclosure/src/lib.rs:1250-1264`
2. 配了审批也可直接走 `publish_disclosure`
   - `entity/disclosure/src/lib.rs:1277-1368`
3. `withdraw_disclosure` 后系统仍保留此前履约痕迹
   - `entity/disclosure/src/lib.rs:1578-1605`
4. `end_blackout` 可被管理员手动提前结束，削弱黑窗控制
   - `entity/disclosure/src/lib.rs:1912-1929`

### 4.2 审批人 / 审计人 / 董事会

主流程：

- 审批草稿
- 拒绝草稿
- 审计签核

关键问题：

1. `update_draft` 改稿后不清空旧审批
   - `entity/disclosure/src/lib.rs:1432-1465`
2. 审批未绑定内容版本
3. 审计流程复用了 `DisclosureApproved/DisclosureRejected` 事件
   - `entity/disclosure/src/lib.rs:3046-3085`

### 4.3 内幕人 / 已移除内幕人 / 大股东

主流程：

- 加入内幕名单
- 黑窗受限
- 被移除后进入 cooldown
- 自行申报交易

关键问题：

1. `report_insider_transaction` 对 cooldown 只检查 `contains_key`
   - `entity/disclosure/src/lib.rs:2860-2863`
2. 大股东自动注册接口存在，但 token 侧未见完整自动接入
   - disclosure 侧：`entity/disclosure/src/lib.rs:3469-3477`
   - token 侧仅见黑窗交易检查：`entity/token/src/lib.rs:158-159`, `882-885`, `1807-1812`

### 4.4 普通投资者 / 公示信息读取者

主流程：

- 查询披露
- 查询公告
- 判断实体是否高风险/受罚

关键问题：

1. 任何人可清理实体历史索引中的终态记录
   - `entity/disclosure/src/lib.rs:2366-2425`
2. 公开索引与容量回收索引混用，前端可见性不稳
3. 未配置 disclosure 的实体也可直接发布披露，规则不统一
   - `entity/disclosure/src/lib.rs:3163-3166`

### 4.5 Governance / Root / 监管

主流程：

- 举报违规
- 强制配置
- 治理处罚
- 重置违规

关键问题：

1. `BlackoutTrading` / `UndisclosedMaterialEvent` 举报证据要求过弱
   - `entity/disclosure/src/lib.rs:2318-2331`
2. Governance 调用 `governance_set_penalty_level`，但 disclosure pallet 未实际实现
   - `entity/common/src/traits/compliance.rs:106-109`
   - `entity/governance/src/lib.rs:3296-3299`

### 4.6 Token / Market / Runtime 视角

关键问题：

1. token / market 主要只接入了 `can_insider_trade`
2. `is_penalty_active` / `get_penalty_level` 没有形成实质业务封锁
3. runtime `type OnDisclosureViolation = ();`，处罚升级无下游执行器
   - `../runtime/src/configs/mod.rs:2120`

---

## 5. P0：必须优先修复

---

### P0-1 禁止通过 reconfigure 刷新披露 deadline

#### 问题

当前 `configure_disclosure` / `force_configure_disclosure` 会根据 `now` 直接重算：

- `next_required_disclosure`

即使实体没有真实发布新披露，也可以通过重新配置把 deadline 向后推。

#### 证据位置

- `entity/disclosure/src/lib.rs:1250-1264`
- `entity/disclosure/src/lib.rs:2259-2297`
- `entity/disclosure/src/lib.rs:3430-3459`

#### 风险

- 直接绕过“逾期披露”监管逻辑
- 可规避自动违规检测与手工举报
- 破坏披露节奏可信度

#### 修复方案

1. 新增内部 helper：
   - `recompute_deadline_preserving_schedule(...)`
2. 普通 `configure_disclosure`：
   - **不得把 deadline 往后推**
   - 升级到更严格级别时只允许提前，不允许延后
3. `force_configure_disclosure` 默认也不重置 deadline
4. 如确需重置，单独新增 Root-only extrinsic：
   - `force_rebase_disclosure_deadline`

#### 影响文件

- `entity/disclosure/src/lib.rs`
- `entity/disclosure/src/tests.rs`

---

### P0-2 违规举报必须证据化，不能直接弱验证累计处罚

#### 问题

当前 `report_disclosure_violation` 对：

- `BlackoutTrading`
- `UndisclosedMaterialEvent`

基本没有足够证据门槛，但会直接递增 `violation_count`，进一步触发高风险和处罚升级。

#### 证据位置

- `entity/disclosure/src/lib.rs:2304-2349`
- `entity/disclosure/src/lib.rs:3216-3254`

#### 风险

- 恶意举报可直接影响实体处罚状态
- 对外部治理/监管流程形成误导
- 高风险标记可能被错误触发

#### 修复方案

1. 保留 `LateDisclosure` 的链上即时判定
2. 新增举报工单结构：
   - `ViolationReport`
3. 新增 storage：
   - `NextViolationReportId`
   - `ViolationReports`
4. 新增 extrinsic：
   - `submit_violation_report`
   - `confirm_violation_report`
   - `reject_violation_report`
5. 对 `BlackoutTrading` / `UndisclosedMaterialEvent`：
   - 只进入 `Pending`
   - 仅在 Root / governance 确认后才递增 `violation_count`
6. 去重 key 从：
   - `(entity_id, snapshot)`
   改为至少：
   - `(entity_id, violation_type, snapshot)`

#### 影响文件

- `entity/disclosure/src/lib.rs`
- `entity/disclosure/src/tests.rs`

---

### P0-3 审批流必须硬约束，不能被 direct publish 绕过

#### 问题

1. entity 配了 approval requirement 后，仍可直接 `publish_disclosure`
2. `update_draft` 改稿后，旧审批不失效
3. `publish_draft` 才检查审批数，形成绕过面

#### 证据位置

- `entity/disclosure/src/lib.rs:1277-1368`
- `entity/disclosure/src/lib.rs:1432-1465`
- `entity/disclosure/src/lib.rs:1499-1523`
- `entity/disclosure/src/lib.rs:2628-2708`
- `entity/disclosure/src/lib.rs:2714-2748`

#### 风险

- 审批规则名存实亡
- 审批结果与实际发布内容不一致
- 合规与审计链断裂

#### 修复方案

1. 一旦 entity 配置 `required_approvals > 0`
   - 禁止直接 `publish_disclosure`
   - 必须先 `create_draft_disclosure`
   - 再 `approve_disclosure`
   - 再 `publish_draft`
2. 草稿版本化：
   - 新增 `DraftRevision`
3. `update_draft`、草稿元数据修改后：
   - 清空 `DisclosureApprovals`
   - 清零 `DisclosureApprovalCounts`
   - revision + 1
4. 新错误：
   - `ApprovalRequiredUseDraftFlow`
5. `publish_emergency_disclosure` 不再作为默认绕过口：
   - 至少要求 owner 级权限
   - 自动打 `audit_status = Pending`

#### 影响文件

- `entity/disclosure/src/lib.rs`
- `entity/disclosure/src/tests.rs`

---

### P0-4 处罚要真正落地执行，不能只记录状态

#### 问题

`DisclosurePenaltyChange` 治理提案当前调用的是 provider 默认 no-op。

同时 runtime 里的 `OnDisclosureViolation` 也是空实现。

#### 证据位置

- `entity/common/src/traits/compliance.rs:106-109`
- `entity/governance/src/lib.rs:3296-3299`
- `../runtime/src/configs/mod.rs:2120`
- `entity/disclosure/src/lib.rs:3482-3488`

#### 风险

- 治理界面“看似处罚成功”，实际业务不受影响
- 处罚与交易/市场状态脱钩

#### 修复方案

1. 在 disclosure pallet 的 `DisclosureProvider impl` 中实现：
   - `governance_set_penalty_level`
2. 规则：
   - `0` => reset
   - `1..=4` => 设置到对应 `PenaltyLevel`
   - 非法值 => `InvalidPenaltyLevel`
3. token / market 所有交易型入口统一加：
   - `!is_penalty_active(entity_id)`
4. runtime 将：
   - `type OnDisclosureViolation = ();`
   替换为真实 handler
5. 最小联动要求：
   - `Restricted` 及以上：拒绝 token / market 交易入口

#### 影响文件

- `entity/disclosure/src/lib.rs`
- `entity/common/src/traits/compliance.rs`
- `entity/token/src/lib.rs`
- `entity/market/src/lib.rs`
- `../runtime/src/configs/mod.rs`

---

### P0-5 paused deadline 必须与 overdue / 举报逻辑一致

#### 问题

`on_idle` 已跳过暂停实体，但：

- `is_disclosure_overdue`
- `report_disclosure_violation(LateDisclosure)`

仍可能判逾期。

#### 证据位置

- `entity/disclosure/src/lib.rs:1155-1163`
- `entity/disclosure/src/lib.rs:2320-2323`
- `entity/disclosure/src/lib.rs:3170-3173`

#### 风险

- 同一个 entity 在不同入口看到不同“是否逾期”结论
- 暂停状态下仍可能被手工累计违规

#### 修复方案

1. `is_disclosure_overdue`：
   - 如 `PausedDeadlines` 存在，直接返回 `false`
2. `report_disclosure_violation(LateDisclosure)`：
   - paused 时拒绝
3. 新错误：
   - `DeadlinePaused`

#### 影响文件

- `entity/disclosure/src/lib.rs`
- `entity/disclosure/src/tests.rs`

---

### P0-6 修复 Full 级别“下一块即逾期”问题

#### 问题

当前：

- `DisclosureLevel::Full => interval = 0`

会导致：

- `next_required_disclosure = now`

之后几乎立刻被判逾期。

#### 证据位置

- `entity/disclosure/src/lib.rs:3095-3102`
- `entity/disclosure/src/lib.rs:3170-3173`

#### 风险

- Full 级别实体无法稳定运行
- 自动违规检测可能频繁误触发

#### 修复方案

短期兼容修复：

- `Full` 返回 `BlockNumber::max_value()` 作为“无固定下一次截止”

长期更优方案：

- 将 `next_required_disclosure` 改为 `Option<BlockNumber>`
- `Full => None`

#### 影响文件

- `entity/disclosure/src/lib.rs`
- `entity/disclosure/src/tests.rs`

---

## 6. P1：P0 后立即修复

---

### P1-1 撤回披露后必须进入“待补披露”或替代披露状态

#### 问题

`withdraw_disclosure` 只改状态，不恢复披露义务。

#### 证据位置

- `entity/disclosure/src/lib.rs:1578-1605`

#### 修复方案

1. 对当前有效披露执行撤回时：
   - 标记 `replacement_required`
   - 或恢复上次 deadline
   - 或设置短补披露宽限期
2. 禁止通过撤回维持“已履约”假象

---

### P1-2 修复 cooldown 判断错误

#### 问题

`report_insider_transaction` 只用 `contains_key` 判断 cooldown。

#### 证据位置

- `entity/disclosure/src/lib.rs:2860-2863`

#### 修复方案

- 改为读取 `until`
- 仅 `now <= until` 时视为 cooldown 中

---

### P1-3 `set_disclosure_metadata` 不得覆盖 emergency / audit 语义

#### 问题

当前会直接覆盖 metadata，可能把：

- `is_emergency = true` 改回 `false`
- 正在进行的审计状态重置

#### 证据位置

- `entity/disclosure/src/lib.rs:2808-2814`
- `entity/disclosure/src/lib.rs:3009-3038`

#### 修复方案

1. 改为 merge update
2. 未传入字段不改写
3. 草稿与已发布披露使用不同更新规则
4. 对已进入审计态的披露限制管理员直接覆盖

---

### P1-4 大股东自动内幕人要真正接入 token 余额变化

#### 问题

disclosure 提供了大股东注册/注销接口，但 token 侧未见完整自动接入。

#### 修复方案

在以下余额变化点重新判断持仓占比：

- mint
- burn
- transfer
- transfer_from

跨过阈值则：

- `register_major_holder`
- `deregister_major_holder`

---

### P1-5 公开历史索引与容量回收索引分离

#### 问题

当前任何人都可清理终态历史在实体索引中的条目，前端主索引可见性被破坏。

#### 证据位置

- `entity/disclosure/src/lib.rs:2366-2425`

#### 修复方案

拆分为：

1. `EntityDisclosuresPublicIndex`
2. `EntityDisclosuresCapacityIndex`
3. `EntityAnnouncementsPublicIndex`
4. `EntityAnnouncementsCapacityIndex`

cleanup 仅释放容量索引，不动公开审计索引。

---

## 7. P2：整理与重构

---

### P2-1 删除或落地未使用配置/错误

当前明显存在“声明了但未落地”的配置/错误：

- `MaxApprovers`
- `InsiderInCooldown`
- `InvalidPenaltyLevel`
- `DeadlineNotPaused`
- `DeadlineAlreadyPaused`
- `ZeroApprovalCount`
- `PenaltyRestricted`
- `MajorHolderAlreadyRegistered`

证据位置：

- `entity/disclosure/src/lib.rs:473`
- `entity/disclosure/src/lib.rs:1059-1096`

策略：

- 能落地就落地
- 短期确实不用就删除

---

### P2-2 按领域拆分 disclosure pallet

当前文件体量：

- `entity/disclosure/src/lib.rs`：3510 行
- `entity/disclosure/src/tests.rs`：4708 行

建议拆分：

- `disclosure.rs`
- `approval.rs`
- `insider.rs`
- `violation.rs`
- `announcement.rs`
- `penalty.rs`
- `hooks.rs`

---

### P2-3 让 FiscalYearConfig 真正参与披露调度

#### 问题

当前 `configure_fiscal_year` 仅存储，不参与核心规则计算。

#### 证据位置

- `entity/disclosure/src/lib.rs:700-704`
- `entity/disclosure/src/lib.rs:2892-2914`

#### 修复方案

将：

- 年报
- 季报
- 月报

的报告期间和调度 deadline 与财年配置绑定。

---

## 8. 推荐实施顺序

### Batch A

优先修真正能被立即利用的问题：

1. P0-1 deadline 防刷新
2. P0-5 paused 一致性
3. P0-6 Full 级别 bug
4. P1-2 cooldown 判断 bug

### Batch B

1. P0-3 审批硬化
2. P1-3 metadata merge update

### Batch C

1. P0-2 违规证据化
2. P1-5 历史索引分离

### Batch D

1. P0-4 处罚执行落地
2. P1-4 大股东接 token
3. runtime 联动

### Batch E

1. P1-1 撤回后补披露
2. P2 清理与模块重构

---

## 9. 最终结论

`pallet-entity-disclosure` 当前最需要的不是继续堆新功能，而是：

1. **补齐规则闭环**
2. **堵住审批/处罚/举报三类绕过口**
3. **把 disclosure 真正变成 token / market / governance 的上游约束源**

若按优先级推进，建议先完成：

- P0-1
- P0-2
- P0-3
- P0-4
- P0-5
- P0-6

完成后再进入 P1 / P2 的业务完善与重构。

