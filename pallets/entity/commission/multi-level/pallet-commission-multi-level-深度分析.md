# pallet-commission-multi-level 深度分析

审计时间：2026-03-13

我看了 `pallet-commission-multi-level` 源码、测试和 `pallet-commission-core` 的集成路径；并本地跑了：

`CARGO_TARGET_DIR=/tmp/nexus-target cargo test -p pallet-commission-multi-level --lib`

结果：**169 个单测全通过**。  
但这只能说明“模块内自测”比较完整，**不代表业务流和跨 pallet 集成没有问题**。

## 一句话结论

这个模块 **单体实现不错**，但从“不同角色用户的真实业务流”看，**还有 4 个高优先级问题必须先修**：

1. **多级分销统计根本没接入 core**
2. **EntityLocked 可被 Owner/Admin 的 pause/resume 绕过**
3. **待生效配置队列存在 DoS / 饥饿问题**
4. **即时配置变更不会清理 PendingConfig，旧待生效配置会反向覆盖新配置/治理配置**

---

# 一、按角色视角看问题

## 1) 买家视角

### 关键问题：`preview_commission` 预览结果不等于真实发放结果

- 代码：`pallets/entity/commission/multi-level/src/lib.rs:1042-1059`
- 真实发放流程：`pallets/entity/commission/core/src/engine.rs:166-196`

`preview_commission` 直接用：

- `remaining = order_amount`
- 只模拟 multi-level 自己

但真实链上发放前，`core` 还会先经过：

- creator reward
- referral plugin
- 全局 `max_commission_rate`
- 其它插件对 `remaining` 的消耗

所以前端如果拿这个函数做“预估收益”，**会高估**。

### 建议新增

- 做一个 **core 级别的真实预览 API**，返回：
  - 每个插件实际分到多少
  - multi-level 实际剩余额度下的发放结果
  - 被跳过原因

---

## 2) 推荐人 / 分销会员视角

### 关键问题：激活进度会“误报已激活”

- 真实发佣校验：`lib.rs:956-975` + `lib.rs:890-918`（`is_member / is_banned / is_activated / is_member_active`）
- 展示激活进度：`lib.rs:1011-1038`

`get_activation_progress()` 只看：

- directs
- team_size
- spent_usdt

**不看**

- 是否 member
- 是否 banned
- 是否 unactivated
- 是否 frozen / inactive

结果就是：

> 前端可能显示“你该层已激活”，但真实下单时你仍然拿不到佣金。

这对会员体验是很差的，容易引发投诉。

### 建议新增

- 增加统一的 eligibility API，返回：
  - `eligible: bool`
  - `is_member`
  - `is_banned`
  - `is_activated`
  - `is_member_active`
  - `failed_reason: enum`

---

## 3) Entity Owner / Admin 视角

### BUG：锁定后仍可 pause/resume，违反“锁定=所有配置操作不可用”

- 错误定义：`lib.rs:383-384`
- `pause_multi_level`：`lib.rs:650-660`
- `resume_multi_level`：`lib.rs:666-677`

这里 **没有** `EntityLocked` 检查。  
但模块自己定义的错误语义是：

> `EntityLocked`: 实体已被全局锁定，所有配置操作不可用

也就是说：

- `set/clear/update/add/remove/schedule` 会被锁阻止
- 但 `pause/resume` 不会

这就是**权限/状态绕过**。

### 建议修复

给 Owner/Admin 的 `pause_multi_level`、`resume_multi_level` 加上：

```rust
ensure!(!T::EntityProvider::is_entity_locked(entity_id), Error::<T>::EntityLocked);
```

### 设计缺口：没有“替换待生效配置”

目前 Owner/Admin 只能：

- schedule
- cancel
- apply

但不能：

- replace pending
- compare current vs pending
- 查看 pending 生效倒计时/差异

这对运营非常不友好。

### 建议新增

- `replace_pending_config`
- `get_current_and_pending_config`
- `preview_pending_effect`

---

## 4) Governance / Root / 风控运维视角

### 高风险 BUG：旧 PendingConfig 会覆盖新的即时配置 / 治理配置 / 强制配置

- `schedule` 写入 pending：`lib.rs:687-715`
- `set/clear/force_set/force_clear` 只改当前 config，不清 pending：`lib.rs:444-475`, `500-529`
- 治理写入也不清 pending：`lib.rs:1326-1417`

这意味着：

### 场景

1. Owner 先 schedule 了一个配置 A
2. Governance 或 Root 立即 set/clear 了配置 B
3. 到生效区块后，**旧的 pending A 仍会自动 apply**
4. B 被“未来的旧配置”反向覆盖

这是非常危险的。

### 建议修复

任何“即时生效的配置变更”都应：

- 自动取消 `PendingConfigs`
- 从 `PendingConfigQueue` 移除
- 或者引入 `config_version`，旧 pending 到期时版本不匹配则丢弃

---

# 二、真正的代码 BUG / 漏洞

## P0 / 高优先级

### 1. multi-level 统计没接入 core，线上基本等于失效

- 统计更新函数：`lib.rs:1065-1091`
- core 调用 multi-level 的地方：`core/src/engine.rs:186-196`
- core 仪表盘却又读取 multi-level stats：`core/src/lib.rs:2513-2514`

全局搜索后，`update_stats()` **只在测试里被调用**，生产代码没有接。

直接后果：

- `MemberMultiLevelStats` 基本不会更新
- `EntityMultiLevelStats` 基本不会更新
- `MultiLevelCommissionDistributed` 事件基本不会发
- Dashboard 里的 `multi_level_stats` 大概率长期为 `None`

这是**真实业务 BUG**，不是小优化。

### 2. Pending 队列可被占满，且存在饥饿/阻塞

- 队列容量：`lib.rs:322-327`（全局只有 100）
- 自动应用：`lib.rs:1163-1198`（每块只检查前 5 个）

问题有两个：

#### 2.1 全局容量太小，可被 DoS

攻击者如果能控制多个 entity，就能把 100 个槽位占满，让别的 entity 无法 schedule。

#### 2.2 前 5 个阻塞后面的 ready 项

自动应用只看队列前 5 个，而且不按 `effective_at` 排序。  
如果前面 5 个长期锁定/未来很远，后面的 ready 配置可能长期自动不生效。

### 建议修复

- 队列按 `effective_at` 排序
- 或用最小堆 / 有序索引
- 或引入轮转 cursor，避免固定检查队首
- 队列不要做全局 100 上限，至少要按 entity 分散/分层

### 3. EntityLocked 逻辑被 pause/resume 绕过

见上，属于状态保护漏洞。

## P1 / 中优先级

### 4. 激活展示逻辑与真实发佣逻辑分叉

- 发佣逻辑：`check_tier_activation + member status checks`
- 展示逻辑：`get_activation_progress()` 手写一套

这是典型的**重复实现导致语义漂移**。  
现在已经漂移了：展示“激活”不代表真实可拿佣金。

### 5. 统计模型与“双资产”设计冲突

- 统计存储：`MemberMultiLevelStats`, `EntityMultiLevelStats` 都是 `u128`
- 事件：`MultiLevelCommissionDistributed { total_amount: u128 }`：`lib.rs:361-362`
- 查询结构：`MultiLevelMemberStats { total_earned: u128 }`：`common/src/lib.rs:718-723`

但这个模块同时服务：

- NEX
- EntityToken

于是统计就出现语义问题：

- `u128` 到底是 NEX 还是 Token？
- 如果两种资产都记进去，就是**混币种统计**
- `total_orders` 也可能因为双资产被重复累加

### 建议修复

拆成：

- `multi_level_nex_stats`
- `multi_level_token_stats`

或至少：

- stats 带资产维度
- event 带 asset kind

### 6. audit log 的 actor 语义不准

Root / Governance 路径里，`who` 不是实际发起人，而是 `entity_account`：

- 例如 `force_set/force_clear/force_pause`：`lib.rs:510-511`, `527-528`, `772-773`

这对合规审计不够友好。  
建议把日志拆成：

- `actor_kind: Owner/Admin/Governance/Root/Auto`
- `actor_account: Option<AccountId>`

## P2 / 低到中优先级

### 7. 生产权重仍是手工估算

- `pallets/entity/commission/multi-level/src/weights.rs:22-25`

文件自己都写了：

> production 应替换成 benchmarking 生成

目前大量 signed extrinsic 还是手估 weight。  
这不一定立刻出漏洞，但在链上长期运行是风险点。

---

# 三、必须补的功能

我认为还**必须增加**这些：

## 1. Core 级真实预览接口

不是当前 `preview_commission()` 这种“插件局部预览”，而是：

- 结合 core 的 `remaining`
- 结合 enabled_modes
- 结合 creator reward / referral / 其它插件
- 返回真实可见结果

## 2. 待生效配置版本控制

至少选一个：

- 即时 set/clear 时自动 cancel pending
- pending 带 `version`
- apply 时检查 `base_version`

## 3. 会员“为什么没拿到佣金”的原因码

建议返回：

- not_member
- banned
- unactivated
- frozen
- insufficient_directs
- insufficient_team_size
- insufficient_spent
- capped_by_max_total_rate
- no_referrer
- cycle_detected

## 4. Owner/Admin 查询接口补全

至少要有：

- current config
- pending config
- 生效时间
- 最近变更日志
- entity 级 multi-level 统计
- 配置差异预览

## 5. 资产维度的统计和事件

否则双资产业务做不干净。

## 6. 自动应用队列重构

这是业务稳定性必须项，不是优化项。

---

# 四、冗余 / 半成品功能

## 1. `get_activation_status()` 基本冗余

搜索结果看，它只在测试里用，和 `get_activation_progress()` 高度重叠。  
建议：

- 要么删除
- 要么合并成统一 eligibility evaluator

## 2. `preview_commission()` 目前是“半成品功能”

只在测试里用，且语义不等于真实业务预览。  
要么：

- 提升成 core 级真实预览
- 要么别对外宣传它是业务预览能力

## 3. `get_recent_change_logs()` 也是半成品

也是只在 pallet 内测试使用，没有通过 runtime API 暴露。  
如果前端/治理后台需要，就正式暴露；否则就是维护负担。

## 4. 多套 set/clear/pause 逻辑重复过多

- owner/admin
- root
- governance

很多地方是复制同一套写法，最终导致状态检查不一致（比如 lock bypass）。  
建议抽成统一内部函数。

---

# 五、建议的修复优先级

## 第一批，必须先做

1. **core 接入 `update_stats`**
2. **pause/resume 增加 EntityLocked 检查**
3. **任何即时配置变更自动失效/清理 pending**
4. **重构 pending queue，解决 DoS 和饥饿**

## 第二批

5. 统一 eligibility 计算与展示
6. 上线真实预览 API
7. stats / event 按资产拆分

## 第三批

8. runtime API 暴露 current/pending config + logs + entity stats
9. 清理冗余 helper
10. 完整 benchmark
