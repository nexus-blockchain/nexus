# pallet-entity-member

Entity 会员管理模块 — 实现 Entity 级别的会员体系、推荐关系链、自定义等级系统和基于规则的自动升级。

## 概述

每个 Entity 拥有独立的会员体系，Shop 仅作为入口标识（`shop_id` → `entity_id` 解析），会员数据统一存储在 Entity 级别。模块支持：

- 多来源会员注册（手动注册、下单自动注册、治理审批）
- 推荐关系链管理（直推、间推、团队人数递归维护）
- 自定义等级系统（消费阈值自动升级 / 管理员手动升级）
- 基于规则的条件升级（6 种触发条件、4 种冲突策略、可叠加有效期）
- 注册与统计策略位（KYC、推荐人、审批、复购口径）
- 会员封禁 / 解封 / 移除 / 主动退出
- 会员激活状态管理
- 治理桥接函数（供 `pallet-entity-governance` 跨模块调用）

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│                       Runtime                                │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐ │
│  │ entity-order │  │ commission   │  │ entity-governance   │ │
│  │  (下单触发)  │  │  (佣金分配)  │  │  (提案桥接)         │ │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬──────────┘ │
│         │                 │                     │            │
│         ▼                 ▼                     ▼            │
│  ┌──────────────────────────────────────────────────────────┐│
│  │               pallet-entity-member                       ││
│  │  OrderMemberHandler  │ MemberProvider  │ governance_*()  ││
│  └──────────────────────────────────────────────────────────┘│
│         │                                                    │
│         ▼                                                    │
│  ┌─────────────────────────────┐                             │
│  │    pallet-entity-common     │                             │
│  │  EntityProvider / ShopProvider / MemberRegistrationPolicy │
│  └─────────────────────────────┘                             │
└──────────────────────────────────────────────────────────────┘
```

## Config 配置

```rust
impl pallet_entity_member::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EntityProvider = EntityRegistry;
    type ShopProvider = EntityShop;
    type MaxDirectReferrals = ConstU32<500>;
    type MaxCustomLevels = ConstU32<10>;
    type MaxUpgradeRules = ConstU32<20>;
    type MaxUpgradeHistory = ConstU32<50>;
    type PendingMemberExpiry = ConstU64<7200>; // ≈24h @12s/block
    type KycChecker = ();                      // 无 KYC 系统时默认全通过
}
```

| 参数 | 类型 | 说明 |
|------|------|------|
| `EntityProvider` | trait | Entity 查询接口（owner、admin、locked 状态） |
| `ShopProvider` | trait | Shop → Entity 映射 |
| `MaxDirectReferrals` | `u32` | 单会员最大直推人数 |
| `MaxCustomLevels` | `u32` | Entity 最大自定义等级数量 |
| `MaxUpgradeRules` | `u32` | Entity 最大升级规则数量 |
| `MaxUpgradeHistory` | `u32` | 单会员最大升级历史记录数量 |
| `PendingMemberExpiry` | `BlockNumber` | 待审批记录过期区块数（0 = 永不过期） |
| `KycChecker` | trait | KYC 检查接口（注册/升级时使用） |

## 数据结构

### EntityMember

会员核心数据，以 `(entity_id, account)` 为键存储。

| 字段 | 类型 | 说明 |
|------|------|------|
| `referrer` | `Option<AccountId>` | 推荐人 |
| `direct_referrals` | `u32` | 直推总人数（含所有来源） |
| `qualified_referrals` | `u32` | 有效直推人数（不含复购赠与） |
| `indirect_referrals` | `u32` | 间推总人数 |
| `qualified_indirect_referrals` | `u32` | 有效间推人数 |
| `team_size` | `u32` | 团队总人数（递归） |
| `total_spent` | `u64` | 累计消费（USDT，精度 10^6） |
| `custom_level_id` | `u8` | 当前等级 ID（0 = 基础） |
| `joined_at` | `BlockNumber` | 加入时间 |
| `last_active_at` | `BlockNumber` | 最后活跃时间（消费时更新） |
| `activated` | `bool` | 是否已激活（首次消费后 `true`） |
| `is_qualified_referral` | `bool` | 注册时是否为有效直推 |
| `banned_at` | `Option<BlockNumber>` | 封禁时间（`None` = 正常） |
| `ban_reason` | `Option<BoundedVec<u8, 128>>` | 封禁原因 |

### CustomLevel

自定义会员等级定义，按 `threshold` 升序排列。

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u8` | 等级 ID（自动分配，0, 1, 2, ...） |
| `name` | `BoundedVec<u8, 32>` | 等级名称（如 "VIP", "黑卡"） |
| `threshold` | `u64` | 升级阈值（USDT 累计消费） |
| `discount_rate` | `u16` | 折扣率（基点，500 = 5%） |
| `commission_bonus` | `u16` | 返佣加成（基点，100 = 1%） |

### UpgradeRule

升级规则定义。

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `u32` | 规则 ID（自动递增） |
| `name` | `BoundedVec<u8, 64>` | 规则名称 |
| `trigger` | `UpgradeTrigger` | 触发条件 |
| `target_level_id` | `u8` | 目标等级 |
| `duration` | `Option<BlockNumber>` | 有效期（`None` = 永久） |
| `enabled` | `bool` | 是否启用 |
| `priority` | `u8` | 优先级（越大越高） |
| `stackable` | `bool` | 是否可叠加延长 |
| `max_triggers` | `Option<u32>` | 最大触发次数（`None` = 无限） |
| `trigger_count` | `u32` | 已触发次数 |

### UpgradeTrigger（6 种触发条件）

| 变体 | 参数 | 说明 |
|------|------|------|
| `PurchaseProduct` | `product_id: u64` | 购买指定产品 |
| `TotalSpent` | `threshold: u64` | 累计消费达标 |
| `SingleOrder` | `threshold: u64` | 单笔消费达标 |
| `ReferralCount` | `count: u32` | 推荐人数达标 |
| `TeamSize` | `size: u32` | 团队人数达标 |
| `OrderCount` | `count: u32` | 订单数量达标 |

### ConflictStrategy（冲突策略）

| 值 | 说明 |
|----|------|
| `HighestLevel` | 取最高等级（默认） |
| `HighestPriority` | 取最高优先级规则 |
| `LongestDuration` | 取最长有效期（永久 > 有限） |
| `FirstMatch` | 第一个匹配的规则 |

### LevelUpgradeMode

| 值 | 说明 |
|----|------|
| `AutoUpgrade` | 消费达标自动升级（默认） |
| `ManualUpgrade` | 需管理员手动设置等级 |

### 可叠加升级矩阵

当 `stackable = true` 时，`apply_upgrade` 的行为：

| 当前等级 | 目标等级 | 当前有 expiry | 当前无 expiry (永久) | 行为 |
|----------|----------|---------------|----------------------|------|
| 1 | 2 | `exp + duration` | `now + duration` | 升级 + 叠加/起算 |
| 1 | 1 | `exp + duration` | 跳过（不操作） | 同级 + 延长 / 永久不降 |
| 2 | 1 | — | — | 跳过（绝不降级） |

## 存储项

| 存储 | 键 | 值 | 说明 |
|------|------|------|------|
| `EntityMembers` | `(entity_id, account)` | `EntityMember` | 会员核心数据 |
| `MemberCount` | `entity_id` | `u32` | 会员总数 |
| `LevelMemberCount` | `(entity_id, level_id)` | `u32` | 各等级会员数 |
| `DirectReferrals` | `(entity_id, account)` | `BoundedVec<AccountId>` | 直推列表索引 |
| `EntityLevelSystems` | `entity_id` | `EntityLevelSystem` | 等级系统配置 |
| `EntityUpgradeRules` | `entity_id` | `EntityUpgradeRuleSystem` | 升级规则系统 |
| `MemberLevelExpiry` | `(entity_id, account)` | `BlockNumber` | 等级过期时间 |
| `MemberUpgradeHistory` | `(entity_id, account)` | `BoundedVec<UpgradeRecord>` | 升级历史 |
| `MemberOrderCount` | `(entity_id, account)` | `u32` | 订单数量 |
| `EntityMemberPolicy` | `entity_id` | `MemberRegistrationPolicy` | 注册策略 |
| `EntityMemberStatsPolicy` | `entity_id` | `MemberStatsPolicy` | 统计策略 |
| `PendingMembers` | `(entity_id, account)` | `(Option<AccountId>, BlockNumber)` | 待审批队列 |
| `BannedMemberCount` | `entity_id` | `u32` | 封禁会员数（O(1) 计数器） |
| `PendingTeamSizeUpdates` | `u32` (update_id) | `TeamSizeUpdate` | 延迟团队人数更新队列 |
| `NextPendingUpdateId` | — | `u32` | 下一个延迟更新 ID |
| `ProcessedPendingUpdateId` | — | `u32` | 已处理的延迟更新 ID |

## 注册策略（MemberRegistrationPolicy）

位标记组合，Entity 级别生效：

| 位 | 常量 | 说明 |
|----|------|------|
| `0b0001` | `PURCHASE_REQUIRED` | 需通过消费（下单）触发注册，禁止手动 `register_member` |
| `0b0010` | `REFERRAL_REQUIRED` | 必须提供有效推荐人 |
| `0b0100` | `APPROVAL_REQUIRED` | 进入待审批状态，需管理员审批 |
| `0b1000` | `KYC_REQUIRED` | 注册时需通过 KYC 认证 |
| `0b10000` | `KYC_UPGRADE_REQUIRED` | 升级时需通过 KYC 认证 |

策略可自由组合，如 `0b0110` = 需要推荐人 + 需要审批。

## 统计策略（MemberStatsPolicy）

| 位 | 常量 | 说明 |
|----|------|------|
| `0b01` | `INCLUDE_REPURCHASE_DIRECT` | 直推人数包含复购赠与注册 |
| `0b10` | `INCLUDE_REPURCHASE_INDIRECT` | 间推人数包含复购赠与注册 |

默认 `0` = 统计时仅计入有效推荐（`qualified_referrals`）。

## Extrinsics（链上调用）

### 会员注册

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `register_member(shop_id, referrer)` | 任何人 | 手动注册（受策略约束） |
| 1 | `bind_referrer(shop_id, referrer)` | 会员本人 | 后绑定推荐人（循环检测） |
| 30 | `leave_entity(entity_id)` | 会员本人 | 主动退出（未封禁 + 无下线） |

### 等级系统管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 4 | `init_level_system(shop_id, use_custom, upgrade_mode)` | Owner/Admin | 初始化（不可重复） |
| 5 | `add_custom_level(shop_id, name, threshold, ...)` | Owner/Admin | 添加等级（阈值递增） |
| 6 | `update_custom_level(shop_id, level_id, ...)` | Owner/Admin | 更新等级属性 |
| 7 | `remove_custom_level(shop_id, level_id)` | Owner/Admin | 删除最后一个等级（需无会员） |
| 8 | `manual_set_member_level(shop_id, member, target_level_id)` | Owner/Admin | 手动设置等级（仅 ManualUpgrade 模式） |
| 9 | `set_use_custom_levels(shop_id, use_custom)` | Owner/Admin | 切换自定义等级开关 |
| 10 | `set_upgrade_mode(shop_id, upgrade_mode)` | Owner/Admin | 切换升级模式 |
| 28 | `reset_level_system(shop_id)` | Owner/Admin | 重置（需所有会员为 level 0） |

### 升级规则管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 11 | `init_upgrade_rule_system(shop_id, conflict_strategy)` | Owner/Admin | 初始化规则系统 |
| 12 | `add_upgrade_rule(shop_id, name, trigger, ...)` | Owner/Admin | 添加规则 |
| 13 | `update_upgrade_rule(shop_id, rule_id, enabled, priority)` | Owner/Admin | 更新规则 |
| 14 | `remove_upgrade_rule(shop_id, rule_id)` | Owner/Admin | 删除规则 |
| 15 | `set_upgrade_rule_system_enabled(shop_id, enabled)` | Owner/Admin | 启用/禁用规则系统 |
| 16 | `set_conflict_strategy(shop_id, conflict_strategy)` | Owner/Admin | 设置冲突策略 |
| 29 | `reset_upgrade_rule_system(shop_id)` | Owner/Admin | 重置规则系统 |

### 策略管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 17 | `set_member_policy(shop_id, policy_bits)` | Owner/Admin | 设置注册策略 |
| 20 | `set_member_stats_policy(shop_id, policy_bits)` | Owner/Admin | 设置统计策略 |

### 审批管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 18 | `approve_member(shop_id, account)` | Owner/Admin | 审批通过（检查过期） |
| 19 | `reject_member(shop_id, account)` | Owner/Admin | 审批拒绝 |
| 21 | `cancel_pending_member(shop_id)` | 申请人本人 | 撤回申请 |
| 22 | `cleanup_expired_pending(entity_id, max_clean)` | 任何人 | 清理过期记录（上限 50） |
| 23 | `batch_approve_members(shop_id, accounts)` | Owner/Admin | 批量审批（上限 50） |
| 24 | `batch_reject_members(shop_id, accounts)` | Owner/Admin | 批量拒绝（上限 50） |

### 会员管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 25 | `ban_member(shop_id, account, reason)` | Owner/Admin | 封禁（含原因） |
| 26 | `unban_member(shop_id, account)` | Owner/Admin | 解封 |
| 27 | `remove_member(shop_id, account)` | Owner/Admin | 移除（需无下线） |
| 31 | `activate_member(shop_id, account)` | Owner/Admin | 手动激活 |
| 32 | `deactivate_member(shop_id, account)` | Owner/Admin | 取消激活 |

所有 Owner/Admin 操作需持有 `MEMBER_MANAGE` 权限，且实体未被锁定（`EntityLocked` 检查）。

## 内部函数（系统调用）

| 函数 | 调用方 | 说明 |
|------|--------|------|
| `auto_register(shop_id, account, referrer)` | Order 模块 | 下单时自动注册（受策略约束） |
| `auto_register_by_entity(entity_id, account, referrer, qualified)` | Commission 模块 | Entity 直达注册（区分有效/复购） |
| `update_spent(shop_id, account, amount_usdt)` | Order 模块 | 更新消费金额（自动升级 + 激活） |
| `update_spent_by_entity(entity_id, account, amount_usdt)` | Commission 模块 | Entity 直达消费更新 |
| `check_order_upgrade_rules(shop_id, buyer, product_id, amount_usdt)` | Order 模块 | 订单完成后检查升级规则 |
| `check_order_upgrade_rules_by_entity(entity_id, buyer, product_id, amount_usdt)` | Commission 模块 | Entity 直达规则检查 |

## 治理桥接函数

供 `pallet-entity-governance` 通过提案调用，无 origin 检查但仍强制 `EntityLocked` 检查：

| 函数 | 说明 |
|------|------|
| `governance_set_custom_levels_enabled(entity_id, enabled)` | 启用/禁用自定义等级 |
| `governance_set_upgrade_mode(entity_id, mode)` | 设置升级模式 |
| `governance_add_custom_level(entity_id, ...)` | 添加自定义等级 |
| `governance_update_custom_level(entity_id, ...)` | 更新自定义等级 |
| `governance_remove_custom_level(entity_id, level_id)` | 删除自定义等级 |
| `governance_set_registration_policy(entity_id, policy_bits)` | 设置注册策略 |
| `governance_set_stats_policy(entity_id, policy_bits)` | 设置统计策略 |

## Hooks

### on_idle — 自动清理过期待审批记录

- 每区块空闲时自动扫描 `PendingMembers`
- 扫描上限 50 条，清理上限 10 条，精确计量 weight
- 仅当 `PendingMemberExpiry > 0` 时执行
- 发出 `PendingMemberExpired` 事件

## Trait 实现

### MemberProvider

实现 `pallet_entity_common::MemberProvider`，供 Commission / Order / Governance 等模块查询：

| 方法 | 说明 |
|------|------|
| `is_member` | 是否为会员 |
| `custom_level_id` | 有效等级（考虑过期） |
| `get_effective_level` | 有效等级（写穿模式） |
| `get_level_discount` | 等级折扣率 |
| `get_level_commission_bonus` | 等级返佣加成 |
| `uses_custom_levels` | 是否使用自定义等级 |
| `get_referrer` | 获取推荐人 |
| `get_member_stats` | 获取统计（直推、团队、消费） |
| `member_count` | 会员总数 |
| `member_count_by_level` | 各等级会员数 |
| `custom_level_count` | 自定义等级数量 |
| `is_banned` | 是否被封禁 |
| `is_activated` | 是否已激活 |
| `last_active_at` | 最后活跃时间 |
| `member_level` | 等级详情（名称、阈值、折扣、返佣） |
| `get_member_spent_usdt` | 累计消费金额 |
| `completed_order_count` | 已完成订单数 |
| `referral_registered_at` | 注册时间 |
| `auto_register` | 自动注册 |
| `auto_register_qualified` | 自动注册（区分有效/复购） |
| `update_spent` | 更新消费金额 |
| `check_order_upgrade_rules` | 检查升级规则 |
| `set_custom_levels_enabled` | 治理：启用等级 |
| `set_upgrade_mode` | 治理：设置升级模式 |
| `add_custom_level` | 治理：添加等级 |
| `update_custom_level` | 治理：更新等级 |
| `remove_custom_level` | 治理：删除等级 |
| `set_registration_policy` | 治理：设置注册策略 |
| `set_stats_policy` | 治理：设置统计策略 |

### OrderMemberHandler

实现 `pallet_entity_common::OrderMemberHandler`，供 Transaction 模块调用：

| 方法 | 说明 |
|------|------|
| `auto_register` | 自动注册会员 |
| `update_spent` | 更新消费金额 |
| `check_order_upgrade_rules` | 订单完成后检查升级规则 |

## Runtime API

通过 `MemberTeamApi` 提供链下查询：

| 接口 | 返回类型 | 说明 |
|------|----------|------|
| `get_member_info(entity_id, account)` | `MemberDashboardInfo` | 会员仪表盘（等级/消费/推荐/过期/升级历史） |
| `get_referral_team(entity_id, account, depth)` | `Vec<TeamMemberInfo>` | 推荐团队树（1-2 层深度） |
| `get_entity_member_overview(entity_id)` | `EntityMemberOverview` | Entity 总览（会员数/等级分布/待审批/封禁数） |
| `get_members_paginated(entity_id, page_size, page_index)` | `PaginatedMembersResult` | 分页查询会员列表（上限 100/页） |

### 返回数据结构

**MemberDashboardInfo**：referrer / custom_level_id / effective_level_id / total_spent / direct_referrals / qualified_referrals / indirect_referrals / team_size / order_count / joined_at / last_active_at / is_banned / banned_at / level_expires_at / upgrade_history

**TeamMemberInfo**：account / level_id / total_spent / direct_referrals / team_size / joined_at / last_active_at / is_banned / children（递归）

**PaginatedMemberInfo**：account / level_id / total_spent / direct_referrals / team_size / joined_at / is_banned / ban_reason

**EntityMemberOverview**：total_members / level_distribution / pending_count / banned_count

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `MemberRegistered` | `entity_id, shop_id: Option<u64>, account, referrer` | 会员注册成功 |
| `ReferrerBound` | `shop_id, account, referrer` | 绑定推荐人 |
| `CustomLevelUpgraded` | `entity_id, account, old_level_id, new_level_id` | 消费自动升级 |
| `LevelSystemInitialized` | `shop_id, use_custom, upgrade_mode` | 等级系统初始化 |
| `CustomLevelAdded` | `shop_id, level_id, name, threshold` | 添加自定义等级 |
| `CustomLevelUpdated` | `shop_id, level_id` | 更新自定义等级 |
| `CustomLevelRemoved` | `shop_id, level_id` | 删除自定义等级 |
| `UpgradeRuleSystemInitialized` | `shop_id, conflict_strategy` | 升级规则系统初始化 |
| `UpgradeRuleAdded` | `shop_id, rule_id, name, target_level_id` | 添加升级规则 |
| `UpgradeRuleUpdated` | `shop_id, rule_id` | 更新升级规则 |
| `UpgradeRuleRemoved` | `shop_id, rule_id` | 删除升级规则 |
| `MemberUpgradedByRule` | `entity_id, account, rule_id, from/to_level_id, expires_at` | 规则触发升级 |
| `MemberLevelExpired` | `entity_id, account, expired_level_id, new_level_id` | 等级过期回退 |
| `MemberPolicyUpdated` | `entity_id, policy` | 注册策略更新 |
| `MemberStatsPolicyUpdated` | `entity_id, policy` | 统计策略更新 |
| `MemberPendingApproval` | `entity_id, account, referrer` | 进入待审批 |
| `MemberApproved` | `entity_id, shop_id, account` | 审批通过 |
| `MemberRejected` | `entity_id, account` | 审批拒绝 |
| `UseCustomLevelsUpdated` | `shop_id, use_custom` | 自定义等级开关变更 |
| `UpgradeModeUpdated` | `shop_id, upgrade_mode` | 升级模式变更 |
| `UpgradeRuleSystemToggled` | `shop_id, enabled` | 规则系统启用/禁用 |
| `ConflictStrategyUpdated` | `shop_id, strategy` | 冲突策略变更 |
| `PendingMemberCancelled` | `entity_id, account` | 撤回申请 |
| `PendingMemberExpired` | `entity_id, account` | 待审批过期 |
| `MemberBanned` | `entity_id, account, reason` | 会员封禁 |
| `MemberUnbanned` | `entity_id, account` | 会员解封 |
| `MemberActivated` | `entity_id, account` | 会员激活 |
| `MemberDeactivated` | `entity_id, account` | 取消激活 |
| `BatchMembersApproved` | `entity_id, count` | 批量审批通过 |
| `BatchMembersRejected` | `entity_id, count` | 批量审批拒绝 |
| `MemberRemoved` | `entity_id, account` | 会员移除 |
| `MemberLevelSet` | `entity_id, account, old_level_id, new_level_id` | 手动设置等级 |
| `LevelSystemReset` | `entity_id` | 等级系统重置 |
| `UpgradeRuleSystemReset` | `entity_id` | 规则系统重置 |
| `MemberLeft` | `entity_id, account` | 会员主动退出 |
| `GovernanceMemberPolicyUpdated` | `entity_id, policy` | 治理更新注册策略 |
| `GovernanceStatsPolicyUpdated` | `entity_id, policy` | 治理更新统计策略 |

## Errors

| 错误 | 说明 |
|------|------|
| `AlreadyMember` | 已是会员 |
| `NotMember` | 不是会员 |
| `ReferrerAlreadyBound` | 已绑定推荐人 |
| `InvalidReferrer` | 无效推荐人 |
| `SelfReferral` | 不能推荐自己 |
| `CircularReferral` | 循环推荐 |
| `ShopNotFound` | 店铺不存在 |
| `ReferralsFull` | 推荐人数已满 |
| `Overflow` | 数值溢出 |
| `LevelSystemNotInitialized` | 等级系统未初始化 |
| `LevelNotFound` | 等级 ID 不存在 |
| `LevelsFull` | 等级数量已满 |
| `InvalidLevelId` | 无效等级 ID |
| `InvalidThreshold` | 等级阈值无效（需严格递增） |
| `EmptyLevelName` | 等级名称为空 |
| `ManualUpgradeNotSupported` | 当前模式不支持手动升级 |
| `LevelHasMembers` | 等级有会员，无法删除 |
| `UpgradeRuleSystemNotInitialized` | 升级规则系统未初始化 |
| `UpgradeRuleNotFound` | 升级规则不存在 |
| `UpgradeRulesFull` | 升级规则数量已满 |
| `EmptyRuleName` | 规则名称为空 |
| `InvalidTargetLevel` | 无效目标等级 |
| `PurchaseRequiredForRegistration` | 需先消费才能注册 |
| `ReferralRequiredForRegistration` | 需提供推荐人 |
| `MemberPendingApproval` | 会员待审批中 |
| `PendingMemberNotFound` | 未找到待审批记录 |
| `NotEntityAdmin` | 不是 Entity 管理员 |
| `InvalidBasisPoints` | 基点值超出范围（最大 10000） |
| `InvalidPolicyBits` | 无效策略位标记 |
| `InvalidUpgradeMode` | 无效升级模式值 |
| `LevelSystemAlreadyInitialized` | 等级系统已初始化 |
| `UpgradeRuleSystemAlreadyInitialized` | 升级规则系统已初始化 |
| `NameTooLong` | 名称过长（超过 32 字节） |
| `RuleIdOverflow` | 规则 ID 溢出 |
| `PendingMemberAlreadyExpired` | 待审批记录已过期 |
| `MemberAlreadyBanned` | 会员已被封禁 |
| `MemberNotBanned` | 会员未被封禁 |
| `MemberIsBanned` | 会员已被封禁（操作拒绝） |
| `BatchLimitExceeded` | 批量操作超过上限 |
| `KycNotPassed` | 未通过 KYC（注册时） |
| `KycRequiredForUpgrade` | 未通过 KYC（升级时） |
| `MemberHasDownlines` | 会员有下线，无法移除 |
| `LevelSystemHasNonZeroMembers` | 有非零等级会员，无法重置 |
| `EntityLocked` | 实体已锁定 |
| `CannotLeave` | 无法退出（被封禁或有下线） |
| `AlreadyActivated` | 会员已激活 |
| `NotActivated` | 会员未激活 |

## 核心业务逻辑

### 等级过期 — 写穿模式

当通过 `get_effective_level_by_entity` 查询等级时，若检测到 `MemberLevelExpiry` 已过期：

1. 基于 `total_spent` 重新计算等级
2. 立即修正 `EntityMembers.custom_level_id` 和 `LevelMemberCount`
3. 清除 `MemberLevelExpiry`
4. 发出 `MemberLevelExpired` 事件

所有跨模块查询（Commission、Order）均通过 `MemberProvider.get_effective_level` 触发此逻辑。

### 推荐链维护

- **注册时**：`mutate_member_referral` 更新推荐人的 `direct_referrals`、`qualified_referrals`，并递归向上更新 `team_size` 和 `indirect_referrals`（最大深度 100）
- **绑定推荐人时**：复用 `mutate_member_referral`，先执行 `DirectReferrals` 容量检查
- **移除/退出时**：`decrement_team_size_by_entity` 递归递减祖先的 `team_size` 和 `indirect_referrals`
- **防护**：循环引用检测（`BTreeSet` 已访问集合）、最大深度限制

### 封禁会员行为

- 封禁后：自动升级跳过、消费统计跳过、手动调级拒绝
- 封禁会员不能主动退出（需管理员解封或移除）
- `BannedMemberCount` O(1) 计数器维护

### 推荐类升级规则

注册/绑定推荐人后，`mutate_member_referral` 会自动调用 `check_referral_upgrade_rules_by_entity`，仅评估 `ReferralCount` 和 `TeamSize` 触发器。

## 依赖关系

| 依赖 | 用途 |
|------|------|
| `pallet-entity-common` | `EntityProvider` / `ShopProvider` / `MemberProvider` trait / 策略类型 |
| `frame-support` | Substrate 框架基础 |
| `frame-system` | 区块号、签名验证 |
| `sp-runtime` | 运行时类型 |
| `sp-api` | Runtime API 声明 |
| `frame-benchmarking` | 性能基准测试（可选） |

## 测试

```bash
cargo test -p pallet-entity-member
```

共 143 个测试用例覆盖：注册流程、推荐链维护、等级系统、升级规则、策略管理、审批流程、封禁/解封、激活/取消激活、主动退出、治理桥接等。
