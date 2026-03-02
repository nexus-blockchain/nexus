# pallet-entity-member

Entity 会员管理模块 — 会员注册、推荐关系、双轨等级体系、升级规则引擎、直推溢出安置（Forced Matrix）

## 概述

`pallet-entity-member` 管理 Entity 级别的会员体系。会员数据按 **entity_id** 统一存储（非 shop_id），同一 Entity 下的所有 Shop 共享会员池。

### 核心能力

- **会员注册** — 开放/需购买/需推荐/需审批四种策略可组合
- **推荐关系** — 绑定上级，递归维护团队人数和间接推荐计数
- **双轨等级** — 全局 5 级体系（USDT 阈值驱动）+ Entity 自定义等级体系（折扣率/返佣加成）
- **升级规则引擎** — 9 种触发条件、4 种冲突策略、可叠加有效期、自动/手动升级
- **等级过期** — 规则驱动的限时等级，过期后自动回退到消费对应等级
- **直推溢出安置** — 可选的强制矩阵（Forced Matrix）模式，直推达上限时 BFS 自动安置到子树空位
- **统计策略** — 可配置推荐人数统计口径（是否包含复购赠与注册）
- **治理桥接** — 5 个 governance 内部函数供 `pallet-entity-governance` 提案执行

### 模块关系

```
                    pallet-entity-member
                   ┌────────────────────┐
                   │ 会员注册/推荐关系   │
                   │ 双轨等级管理        │
                   │ 升级规则引擎        │
                   │ 直推溢出安置        │
                   │ 注册/统计策略       │
                   └──┬──────────┬──────┘
        依赖          │          │  对外 Trait
    ┌─────────────────┤          ├──────────────────┐
    ▼                 ▼          ▼                  ▼
 EntityProvider  ShopProvider  MemberProvider   OrderMemberHandler
 (entity-registry) (entity-shop)  (commission 等)   (transaction)
```

## 安装

```toml
# Cargo.toml
[dependencies]
pallet-entity-member = { path = "pallets/entity/member", default-features = false }

[features]
std = ["pallet-entity-member/std"]
```

## Runtime 配置

```rust
impl pallet_entity_member::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EntityProvider = EntityRegistry;       // Entity 所有权查询
    type ShopProvider = EntityShop;             // Shop → Entity 映射
    type MaxDirectReferrals = ConstU32<1000>;   // 单会员最大直推人数（硬上限）
    type MaxCustomLevels = ConstU32<10>;        // 单 Entity 最大自定义等级数
    type SilverThreshold = ConstU64<100_000_000>;    // 100 USDT (6 位精度)
    type GoldThreshold = ConstU64<500_000_000>;      // 500 USDT
    type PlatinumThreshold = ConstU64<2000_000_000>; // 2,000 USDT
    type DiamondThreshold = ConstU64<10000_000_000>; // 10,000 USDT
    type MaxUpgradeRules = ConstU32<50>;        // 单 Entity 最大升级规则数
    type MaxUpgradeHistory = ConstU32<100>;     // 单会员最大升级历史记录数
}
```

### Config 常量一览

| 常量 | 类型 | Runtime 值 | 说明 |
|------|------|-----------|------|
| `MaxDirectReferrals` | `u32` | 1000 | 单会员最大直推人数（BoundedVec 硬上限） |
| `MaxCustomLevels` | `u32` | 10 | 单 Entity 最大自定义等级数 |
| `SilverThreshold` | `u64` | 100_000_000 | 银卡阈值 (100 USDT, 6 位精度) |
| `GoldThreshold` | `u64` | 500_000_000 | 金卡阈值 (500 USDT) |
| `PlatinumThreshold` | `u64` | 2000_000_000 | 白金阈值 (2,000 USDT) |
| `DiamondThreshold` | `u64` | 10000_000_000 | 钻石阈值 (10,000 USDT) |
| `MaxUpgradeRules` | `u32` | 50 | 单 Entity 最大升级规则数 |
| `MaxUpgradeHistory` | `u32` | 100 | 单会员最大升级历史记录数 |

## 数据结构

### EntityMember — 会员信息

```rust
pub struct EntityMember<AccountId, Balance, BlockNumber> {
    pub referrer: Option<AccountId>,           // 推荐人（上级，溢出时为 actual_referrer）
    pub direct_referrals: u32,                 // 直接推荐人数（含所有来源）
    pub qualified_referrals: u32,              // 有效直推人数（不含复购赠与）
    pub indirect_referrals: u32,               // 间接推荐人数（含所有来源）
    pub qualified_indirect_referrals: u32,     // 有效间接推荐人数
    pub team_size: u32,                        // 团队总人数（递归向上累加）
    pub total_spent: Balance,                  // 累计消费金额（NEX 原生精度）
    pub level: MemberLevel,                    // 全局等级（USDT 阈值驱动）
    pub custom_level_id: u8,                   // 自定义等级 ID（Entity 体系）
    pub joined_at: BlockNumber,                // 加入区块
    pub last_active_at: BlockNumber,           // 最后活跃区块
    pub activated: bool,                       // 是否已激活
}
```

### MemberLevel — 全局 5 级体系

```rust
pub enum MemberLevel {
    Normal,    // 无门槛
    Silver,    // ≥ 100 USDT
    Gold,      // ≥ 500 USDT
    Platinum,  // ≥ 2,000 USDT
    Diamond,   // ≥ 10,000 USDT
}
```

全局等级由独立的 `MemberSpentUsdt` 存储驱动（USDT 精度），而非 `total_spent`（NEX 精度），避免跨币种精度不匹配。

### CustomLevel — 自定义等级

```rust
pub struct CustomLevel<Balance> {
    pub id: u8,                              // 等级 ID（0, 1, 2, ... 自增）
    pub name: BoundedVec<u8, ConstU32<32>>,  // 名称（≤32 字节，如 "VIP"）
    pub threshold: Balance,                  // 升级阈值（累计消费）
    pub discount_rate: u16,                  // 折扣率（基点，500 = 5%）
    pub commission_bonus: u16,               // 返佣加成（基点，100 = 1%）
}
```

### EntityLevelSystem — 等级系统配置

```rust
pub struct EntityLevelSystem<Balance, MaxLevels: Get<u32>> {
    pub levels: BoundedVec<CustomLevel<Balance>, MaxLevels>,
    pub use_custom: bool,              // false 时使用全局 5 级体系
    pub upgrade_mode: LevelUpgradeMode,
}

pub enum LevelUpgradeMode {
    AutoUpgrade,   // 消费达标自动升级
    ManualUpgrade, // 需店主审批
}
```

### UpgradeRule — 升级规则

```rust
pub struct UpgradeRule<Balance, BlockNumber> {
    pub id: u32,
    pub name: BoundedVec<u8, ConstU32<64>>,
    pub trigger: UpgradeTrigger<Balance>,
    pub target_level_id: u8,
    pub duration: Option<BlockNumber>,  // None = 永久
    pub enabled: bool,
    pub priority: u8,
    pub stackable: bool,               // 多次触发是否延长有效期
    pub max_triggers: Option<u32>,      // None = 无限
    pub trigger_count: u32,
}

pub enum UpgradeTrigger<Balance> {
    PurchaseProduct { product_id: u64 },           // 购买特定产品
    TotalSpent { threshold: Balance },             // 累计消费（NEX 计价）
    SingleOrder { threshold: Balance },            // 单笔消费（NEX 计价）
    ReferralCount { count: u32 },                  // 直推人数达标
    TeamSize { size: u32 },                        // 团队人数达标
    OrderCount { count: u32 },                     // 订单数量达标
    ReferralLevelCount { level_id: u8, count: u32 }, // 直推中达到指定等级的人数
    TotalSpentUsdt { threshold: u64 },             // 累计消费（USDT 10^6 精度）
    SingleOrderUsdt { threshold: u64 },            // 单笔消费（USDT 10^6 精度）
}
```

### SpilloverSettings — 溢出安置配置

```rust
pub struct SpilloverSettings {
    pub max_direct: u32,  // 每个会员最大直推数（软限制，≤ MaxDirectReferrals）
    pub enabled: bool,    // 是否启用溢出安置
}
```

### MemberRegistrationPolicy — 注册策略

位标记组合（`u8`，低 3 位有效）：

| 位 | 值 | 标志 | 说明 |
|----|-----|------|------|
| 0 | 1 | `PURCHASE_REQUIRED` | 必须通过购买触发注册（手动 `register_member` 被拒） |
| 1 | 2 | `REFERRAL_REQUIRED` | 必须提供有效推荐人 |
| 2 | 4 | `APPROVAL_REQUIRED` | 进入待审批状态，需管理员 `approve_member` |

可组合，例如 `0b011 = 3` 表示同时要求购买和推荐人。默认 `0` = 开放注册。

### MemberStatsPolicy — 统计策略

位标记（`u8`，低 2 位有效）：

| 位 | 值 | 标志 | 说明 |
|----|-----|------|------|
| 0 | 1 | `INCLUDE_REPURCHASE_DIRECT` | 直推人数包含复购赠与注册 |
| 1 | 2 | `INCLUDE_REPURCHASE_INDIRECT` | 间推人数包含复购赠与注册 |

默认 `0` = 统计口径排除复购赠与。

## 存储项

| 存储 | 类型 | 键 | 说明 |
|------|------|-----|------|
| `EntityMembers` | DoubleMap | `(entity_id, AccountId)` | 会员信息 |
| `MemberCount` | Map | `entity_id` | 会员总数 |
| `LevelMemberCount` | DoubleMap | `(entity_id, level_id)` | 各等级会员数（用于沉淀池等额分配） |
| `DirectReferrals` | DoubleMap | `(entity_id, referrer)` | 直推列表 `BoundedVec<AccountId>` |
| `EntityLevelSystems` | Map | `entity_id` | 自定义等级系统配置 |
| `EntityUpgradeRules` | Map | `entity_id` | 升级规则系统 |
| `MemberLevelExpiry` | DoubleMap | `(entity_id, AccountId)` | 等级过期区块号 |
| `MemberUpgradeHistory` | DoubleMap | `(entity_id, AccountId)` | 升级历史 `BoundedVec<UpgradeRecord>` |
| `MemberSpentUsdt` | DoubleMap | `(entity_id, AccountId)` | 累计 USDT 消费（独立于 `total_spent`） |
| `MemberOrderCount` | DoubleMap | `(entity_id, AccountId)` | 订单数量 |
| `EntityMemberPolicy` | Map | `entity_id` | 注册策略 |
| `EntityMemberStatsPolicy` | Map | `entity_id` | 统计策略 |
| `PendingMembers` | DoubleMap | `(entity_id, AccountId)` | 待审批会员 |
| `SpilloverConfig` | Map | `entity_id` | 溢出安置配置 `SpilloverSettings` |
| `MemberIntroducedBy` | DoubleMap | `(entity_id, AccountId)` | 真实推荐人（溢出时记录 intended_referrer） |

## Extrinsics

### 会员注册与推荐

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 0 | `register_member(shop_id, referrer)` | 任意用户 | 注册会员，受注册策略约束 |
| 1 | `bind_referrer(shop_id, referrer)` | 已注册会员 | 补充绑定推荐人（仅限未绑定） |

### 等级系统管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 4 | `init_level_system(shop_id, use_custom, upgrade_mode)` | 店主 | 初始化等级系统（不可重复） |
| 5 | `add_custom_level(shop_id, name, threshold, discount_rate, commission_bonus)` | 店主 | 添加自定义等级（阈值须递增） |
| 6 | `update_custom_level(shop_id, level_id, ...)` | 店主 | 更新等级属性（阈值须保持单调） |
| 7 | `remove_custom_level(shop_id, level_id)` | 店主 | 删除等级（只能删最后一个） |
| 8 | `manual_upgrade_member(shop_id, member, target_level_id)` | 店主 | 手动升级（仅 ManualUpgrade 模式） |
| 9 | `set_use_custom_levels(shop_id, use_custom)` | 店主 | 切换全局/自定义等级模式 |
| 10 | `set_upgrade_mode(shop_id, upgrade_mode)` | 店主 | 切换 Auto/Manual 升级模式 |

### 升级规则管理

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 11 | `init_upgrade_rule_system(shop_id, conflict_strategy)` | 店主 | 初始化规则系统（不可重复） |
| 12 | `add_upgrade_rule(shop_id, name, trigger, target_level_id, duration, priority, stackable, max_triggers)` | 店主 | 添加升级规则 |
| 13 | `update_upgrade_rule(shop_id, rule_id, enabled, priority)` | 店主 | 更新规则（启用/优先级） |
| 14 | `remove_upgrade_rule(shop_id, rule_id)` | 店主 | 删除规则 |
| 15 | `set_upgrade_rule_system_enabled(shop_id, enabled)` | 店主 | 启用/禁用规则系统 |
| 16 | `set_conflict_strategy(shop_id, conflict_strategy)` | 店主 | 设置冲突策略 |

### 策略与审批

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 17 | `set_member_policy(shop_id, policy_bits)` | Entity Owner/Admin | 设置注册策略 |
| 18 | `approve_member(shop_id, account)` | Entity Owner/Admin | 审批通过待注册会员 |
| 19 | `reject_member(shop_id, account)` | Entity Owner/Admin | 拒绝待注册会员 |
| 20 | `set_member_stats_policy(shop_id, policy_bits)` | Entity Owner/Admin | 设置统计策略 |

### 溢出安置（Spillover）

| call_index | 函数 | 权限 | 说明 |
|------------|------|------|------|
| 21 | `configure_spillover(shop_id, max_direct, enabled)` | Entity Owner | 配置溢出安置参数 |
| 22 | `register_member_with_spillover(shop_id, intended_referrer, actual_referrer)` | 任意用户 | 带溢出的会员注册（链下 BFS + 链上验证） |
| 23 | `set_referrer_with_spillover(shop_id, intended_referrer, actual_referrer)` | 已注册会员 | 带溢出的绑定推荐人 |

> call_index 2-3 为历史预留，未使用。

## Events

| 事件 | 说明 |
|------|------|
| `MemberRegistered` | 会员注册成功 |
| `ReferrerBound` | 绑定推荐人 |
| `MemberLevelUpgraded` | 全局等级升级（USDT 消费驱动） |
| `CustomLevelUpgraded` | 自定义等级升级（消费自动升级） |
| `LevelSystemInitialized` | 等级系统初始化 |
| `CustomLevelAdded` | 自定义等级添加 |
| `CustomLevelUpdated` | 自定义等级更新 |
| `CustomLevelRemoved` | 自定义等级删除 |
| `MemberManuallyUpgraded` | 手动升级会员 |
| `UpgradeRuleSystemInitialized` | 升级规则系统初始化 |
| `UpgradeRuleAdded` | 升级规则添加 |
| `UpgradeRuleUpdated` | 升级规则更新 |
| `UpgradeRuleRemoved` | 升级规则删除 |
| `MemberUpgradedByRule` | 会员通过规则触发升级 |
| `MemberLevelExpired` | 会员等级过期回退 |
| `MemberPolicyUpdated` | 注册策略更新 |
| `MemberStatsPolicyUpdated` | 统计策略更新 |
| `MemberPendingApproval` | 会员进入待审批状态 |
| `MemberApproved` | 会员审批通过 |
| `MemberRejected` | 会员审批拒绝 |
| `MemberActivated` | 代注册会员首次消费激活 |
| `SpilloverConfigured` | 溢出安置配置更新 |
| `SpilloverPlaced` | 溢出安置：新会员被安置到 actual_referrer 下（而非 intended_referrer） |

## Errors

| 错误 | 说明 |
|------|------|
| `AlreadyMember` | 已是会员 |
| `NotMember` | 不是会员 |
| `ReferrerAlreadyBound` | 已绑定推荐人 |
| `InvalidReferrer` | 推荐人不是会员或不存在 |
| `SelfReferral` | 不能推荐自己 |
| `CircularReferral` | 检测到循环推荐链 |
| `NotShopOwner` | 非店铺所有者 |
| `ShopNotFound` | 店铺不存在或未营业 |
| `ReferralsFull` | 直推人数达上限 (MaxDirectReferrals 硬上限或 spillover 软限制) |
| `Overflow` | 数值溢出 |
| `LevelSystemNotInitialized` | 等级系统未初始化 |
| `LevelSystemAlreadyInitialized` | 等级系统已初始化（禁止覆盖） |
| `LevelAlreadyExists` | 等级已存在 |
| `LevelNotFound` | 等级 ID 不存在 |
| `LevelsFull` | 等级数达上限 (MaxCustomLevels) |
| `LevelHasMembers` | 等级有会员，无法删除 |
| `InvalidLevelId` | 无效等级 ID（删除时必须是最后一个） |
| `InvalidThreshold` | 阈值不满足单调递增约束 |
| `EmptyLevelName` | 等级名称为空 |
| `NameTooLong` | 等级名称超过 32 字节 |
| `InvalidBasisPoints` | 基点值超出 0-10000 范围 |
| `ManualUpgradeNotSupported` | 当前为 AutoUpgrade 模式 |
| `UpgradeRuleSystemNotInitialized` | 升级规则系统未初始化 |
| `UpgradeRuleSystemAlreadyInitialized` | 升级规则系统已初始化（禁止覆盖） |
| `UpgradeRuleNotFound` | 规则 ID 不存在 |
| `UpgradeRulesFull` | 规则数达上限 (MaxUpgradeRules) |
| `EmptyRuleName` | 规则名称为空 |
| `InvalidTargetLevel` | 目标等级不存在 |
| `PurchaseRequiredForRegistration` | 需通过购买触发注册 |
| `ReferralRequiredForRegistration` | 需提供推荐人 |
| `MemberPendingApproval` | 已在待审批列表中 |
| `PendingMemberNotFound` | 未找到待审批记录 |
| `NotEntityAdmin` | 非 Entity Owner 或 Admin |
| `InvalidPolicyBits` | 策略位超出有效范围 |
| `InvalidUpgradeMode` | 无效升级模式值 |
| `SpilloverNotEnabled` | 溢出安置未启用 |
| `NotInReferrerSubtree` | 溢出目标不在推荐人子树中 |
| `SpilloverTargetFull` | 溢出目标直推已满（无空位） |
| `SpilloverNoSlotFound` | 有限 BFS 搜索无空位（auto_register 路径） |
| `InvalidSpilloverConfig` | 无效溢出配置（max_direct 须 > 0 且 ≤ MaxDirectReferrals） |

## 双轨等级体系

本模块同时维护两套等级：

```
                    ┌──────────────────────────┐
                    │      update_spent()      │
                    └─────┬──────────┬─────────┘
                          │          │
            ┌─────────────▼──┐  ┌───▼───────────────┐
            │  全局等级       │  │  自定义等级         │
            │  (MemberLevel)  │  │  (custom_level_id) │
            ├─────────────────┤  ├────────────────────┤
            │ MemberSpentUsdt │  │ EntityLevelSystems  │
            │ 驱动 (USDT)    │  │ 驱动 (Balance)      │
            │                 │  │                     │
            │ Normal→Silver→  │  │ 阈值自定义          │
            │ Gold→Platinum→  │  │ 折扣率 + 返佣加成   │
            │ Diamond         │  │ 支持手动/自动升级    │
            │                 │  │ 支持限时等级过期     │
            └─────────────────┘  └─────────────────────┘
```

- **全局等级** — 按 USDT 累计消费自动升级，佣金模块用于基础判断
- **自定义等级** — Entity 自行定义，佣金模块用于等级差/折扣/返佣加成计算

## 升级规则引擎

规则引擎允许 Entity 配置条件化的等级升级逻辑——在订单完成时自动评估规则、解决冲突、应用升级，并管理限时等级的过期回退。

### 架构总览

```
订单完成
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ commission-core (外部调用方)                                      │
│   1. update_spent(entity_id, buyer, amount, amount_usdt)        │
│   2. check_order_upgrade_rules(entity_id, buyer, pid, amt, usd) │
└─────┬───────────────────────────────────┬───────────────────────┘
      │                                   │
      ▼                                   ▼
┌──────────────────┐           ┌──────────────────────────┐
│  update_spent    │           │ check_order_upgrade_rules │
│  (消费驱动升级)   │           │ (规则驱动升级)             │
├──────────────────┤           ├──────────────────────────┤
│ ① 累加 total_spent          │ ① 递增 MemberOrderCount   │
│ ② 全局等级重算 (USDT)        │ ② 遍历规则匹配触发条件    │
│ ③ P4: 过期等级修正           │ ③ 收集匹配规则            │
│ ④ H12: 跳过有效规则升级      │ ④ resolve_conflict 选择   │
│ ⑤ AutoUpgrade 消费等级重算   │ ⑤ apply_upgrade 执行升级  │
└──────────────────┘           └──────────────────────────┘
```

> **关键交互**: `update_spent` 先于 `check_order_upgrade_rules` 执行。H12 审计修复确保 `update_spent` 的 AutoUpgrade 路径**不会覆盖**有效的规则升级（检查 `MemberLevelExpiry` 是否未过期）。

### 数据结构

#### EntityUpgradeRuleSystem — 规则系统容器

```rust
pub struct EntityUpgradeRuleSystem<Balance, BlockNumber, MaxRules> {
    pub rules: BoundedVec<UpgradeRule<Balance, BlockNumber>, MaxRules>,
    pub next_rule_id: u32,          // 自增 ID 分配器（不因删除回退）
    pub enabled: bool,              // 系统总开关
    pub conflict_strategy: ConflictStrategy,
}
```

每个 Entity 最多一个规则系统实例，存储在 `EntityUpgradeRules<T>` StorageMap 中。

#### UpgradeRule — 单条升级规则

```rust
pub struct UpgradeRule<Balance, BlockNumber> {
    pub id: u32,                           // 唯一标识（系统自增）
    pub name: BoundedVec<u8, ConstU32<64>>,// 规则名称
    pub trigger: UpgradeTrigger<Balance>,  // 触发条件
    pub target_level_id: u8,              // 目标自定义等级 ID
    pub duration: Option<BlockNumber>,    // 有效期（None = 永久）
    pub enabled: bool,                    // 单规则开关
    pub priority: u8,                     // 冲突时的优先级权重
    pub stackable: bool,                  // 重复触发是否叠加有效期
    pub max_triggers: Option<u32>,        // 全局最大触发次数（None = 无限）
    pub trigger_count: u32,              // 已触发次数
}
```

**注意**: `update_upgrade_rule` 仅可修改 `enabled` 和 `priority`。修改触发条件、目标等级、有效期等需 remove + re-add（ID 和 trigger_count 会重置）。

### 完整生命周期

#### Phase 1: 初始化

```
init_level_system(shop_id, use_custom=true, mode)
       │  等级系统必须先于规则系统存在 (H8)
       ▼
add_custom_level(shop_id, "VIP1", threshold=500, ...)
add_custom_level(shop_id, "VIP2", threshold=2000, ...)
       │  至少需要目标等级对应的 level_id 存在
       ▼
init_upgrade_rule_system(shop_id, HighestLevel)
       │  不可重复调用 (H5)
       ▼
add_upgrade_rule(shop_id, ...)  ×N
       │  H4+H8: target_level_id 必须 < levels.len()
       ▼
  规则系统就绪，enabled=true
```

#### Phase 2: 触发评估 (`check_order_upgrade_rules_by_entity`)

```
订单完成 → check_order_upgrade_rules(entity_id, buyer, product_id, order_amount, amount_usdt)
    │
    ├─ 会员不存在? → return Ok(())
    │
    ├─ M18: 无条件递增 MemberOrderCount (不受系统开关影响)
    │
    ├─ 规则系统不存在 / enabled=false? → return Ok(())
    │
    ├─ 遍历 system.rules:
    │     ├─ rule.enabled == false → skip
    │     ├─ C3: trigger_count >= max_triggers → skip
    │     └─ 匹配触发条件? → 加入 matched_rules
    │
    ├─ matched_rules 为空? → return Ok(())
    │
    ├─ resolve_conflict(matched_rules, strategy) → selected_rule
    │
    └─ apply_upgrade(entity_id, buyer, selected_rule)
```

#### Phase 3: 冲突解决 (`resolve_conflict`)

当多条规则同时匹配时，根据 `ConflictStrategy` 选择唯一获胜规则：

| 策略 | 选择逻辑 | 适用场景 |
|------|---------|---------|
| `HighestLevel` | `max_by_key(target_level_id)` | 优先让会员获得最高等级（默认） |
| `HighestPriority` | `max_by_key(priority)` | 精细控制规则优先级 |
| `LongestDuration` | `max_by_key(duration)`, `None`(永久) 视为最大值 | 优先给予最长期限 |
| `FirstMatch` | `rules.first()` | 规则顺序决定优先级（插入顺序） |

> **Tie-breaking**: 等值时 `max_by_key` 返回**最后**匹配的元素（即后添加的规则优先）。每次评估只选出**一条**规则执行。

#### Phase 4: 升级应用 (`apply_upgrade`)

```
apply_upgrade(entity_id, account, rule_id, target_level_id, duration, stackable)
    │
    ├─ H7: 验证目标等级仍存在于 EntityLevelSystems
    │     └─ 已删除? → 静默跳过
    │
    ├─ H10: target_level_id < 当前等级? → 禁止降级，跳过
    │
    ├─ target_level_id == 当前等级 && !stackable? → 无意义，跳过
    │
    ├─ 计算过期时间 (expires_at):
    │     ├─ stackable=false → now + duration (或 None=永久)
    │     └─ stackable=true  → 见下方叠加矩阵
    │
    ├─ 更新 LevelMemberCount (old -1, new +1)
    ├─ member.custom_level_id = target_level_id
    ├─ 写入/清除 MemberLevelExpiry
    ├─ 追加 MemberUpgradeHistory (满时静默丢弃)
    ├─ rule.trigger_count += 1
    └─ emit MemberUpgradedByRule
```

### 9 种触发条件

| 触发类型 | 参数 | 判定表达式 | 评估路径 |
|---------|------|-----------|---------|
| `PurchaseProduct` | `product_id: u64` | `pid == product_id` | 订单 |
| `TotalSpent` | `threshold: Balance` | `member.total_spent >= threshold` | 订单 |
| `SingleOrder` | `threshold: Balance` | `order_amount >= threshold` | 订单 |
| `OrderCount` | `count: u32` | `order_count >= count` | 订单 |
| `TotalSpentUsdt` | `threshold: u64` | `spent_usdt >= threshold` | 订单 |
| `SingleOrderUsdt` | `threshold: u64` | `amount_usdt >= threshold` | 订单 |
| `ReferralCount` | `count: u32` | `referrals >= count` | 推荐 |
| `TeamSize` | `size: u32` | `member.team_size >= size` | 推荐 |
| `ReferralLevelCount` | `level_id: u8, count: u32` | 直推中达到指定等级人数 ≥ count | 推荐（级联触发） |

**评估路径分配**:
- **订单路径** (`check_order_upgrade_rules_by_entity`): PurchaseProduct, TotalSpent, SingleOrder, OrderCount, TotalSpentUsdt, SingleOrderUsdt
- **推荐路径** (`check_referral_upgrade_rules_by_entity`): ReferralCount, TeamSize, ReferralLevelCount

**ReferralLevelCount 级联**: 会员升级后自动检查其推荐人的 `ReferralLevelCount` 规则，实现级联升级触发。

**ReferralCount 统计口径**: 根据 `EntityMemberStatsPolicy` 决定使用 `direct_referrals`（含复购赠与）还是 `qualified_referrals`（排除复购赠与）。

### 叠加 (Stackable) 语义

`stackable` 字段控制**同一规则重复触发**时的行为：

#### 过期时间叠加矩阵

| 现有过期状态 | 规则 duration | stackable=true 结果 | 说明 |
|-------------|-------------|-------------------|------|
| `Some(exp)` 未过期 | `Some(d)` | `Some(exp + d)` | 在现有过期时间上追加 |
| `Some(exp)` 未过期 | `None` (永久) | `None` | 升为永久 |
| `None` (永久/首次) | `Some(d)` | `Some(now + d)` | 首次叠加，从当前区块开始 |
| `None` (永久/首次) | `None` (永久) | `None` | 保持永久 |

> **M14 审计修复**: 旧逻辑用 `unwrap_or(now)` 将"永久"误作"从现在开始"，导致永久升级被错误转为限时。

#### 叠加的等级约束

```
stackable + target > current → 升级等级 + 设置过期时间
stackable + target == current → 保持等级 + 延长有效期 ← 核心用途
stackable + target < current → 跳过（H10: 禁止降级）
```

**典型场景**: "购买月卡→VIP2(30天，stackable)" — 会员每次购买月卡，VIP2 有效期延长 30 天。

### 等级过期与回退

#### 过期检测时机

等级过期由**两个路径**触发检测：

```
路径 A: update_spent (主动修正)          路径 B: get_effective_level (只读查询)
    │                                       │
    ├─ 读取 MemberLevelExpiry               ├─ 读取 MemberLevelExpiry
    ├─ now > expires_at?                    ├─ now > expires_at?
    │   ├─ 重算消费等级                      │   └─ 返回 calculate_custom_level
    │   ├─ 修正 custom_level_id             │       (不修改存储)
    │   ├─ 更新 LevelMemberCount            │
    │   ├─ 清除 MemberLevelExpiry           └─ 返回 member.custom_level_id
    │   └─ emit MemberLevelExpired
    │
    └─ H12: 有效期内? → 跳过 AutoUpgrade
```

- **路径 A** 修正存储（写操作），确保下次消费时数据一致
- **路径 B** 不修改存储（只读），用于佣金计算等场景的即时查询

#### 过期回退目标

过期后等级回退到 `calculate_custom_level_by_entity(entity_id, total_spent)` — 即基于累计消费重新匹配最高达标等级。

### update_spent 与规则引擎的交互 (H12)

```
update_spent 执行流程:
    │
    ├─ ① 累加 total_spent + MemberSpentUsdt
    ├─ ② 全局等级 (MemberLevel) 重算
    ├─ ③ P4: 过期等级修正（若已过期）
    │      └─ 清除 MemberLevelExpiry
    ├─ ④ H12 检查: MemberLevelExpiry 是否仍有效?
    │      ├─ 有效 (now <= exp) → 跳过步骤 ⑤
    │      └─ 无效或不存在 → 继续步骤 ⑤
    └─ ⑤ AutoUpgrade: 基于消费重算自定义等级
```

**H12 保护的场景**:
1. 规则在区块 10 将会员升至 VIP3，过期时间 = 区块 110
2. 区块 50 发生新订单，`update_spent` 触发
3. 消费只够 VIP1，但 H12 检测到 `MemberLevelExpiry(110)` 有效
4. 跳过 AutoUpgrade → VIP3 保持不变
5. 区块 111 再次消费，P4 检测过期 → 清除过期标记 → AutoUpgrade 回退到 VIP1

### 存储写入摘要

| 操作 | 写入的 StorageItem |
|------|-------------------|
| `init_upgrade_rule_system` | `EntityUpgradeRules` |
| `add_upgrade_rule` | `EntityUpgradeRules` (mutate rules + next_rule_id) |
| `check_order_upgrade_rules` | `MemberOrderCount` + 可能的 apply_upgrade 写入 |
| `apply_upgrade` | `EntityMembers`, `MemberLevelExpiry`, `LevelMemberCount` ×2, `MemberUpgradeHistory`, `EntityUpgradeRules` (trigger_count) |
| 过期修正 (update_spent) | `EntityMembers`, `MemberLevelExpiry` (remove), `LevelMemberCount` ×2 |

### 安全保障

| 编号 | 保障 | 说明 |
|------|------|------|
| H4 | 目标等级验证 | `add_upgrade_rule` 校验 `target_level_id < levels.len()` |
| H5 | 初始化幂等 | `init_upgrade_rule_system` 不可重复调用 |
| H7 | 删除等级保护 | `apply_upgrade` 跳过已被删除的目标等级 |
| H8 | 依赖前置 | `add_upgrade_rule` 要求等级系统已初始化 |
| H10 | 禁止降级 | `apply_upgrade` 禁止 `target < current`（含 stackable） |
| H12 | 规则升级保护 | `update_spent` 的 AutoUpgrade 不覆盖有效规则升级 |
| M14 | 永久升级保护 | stackable 叠加正确处理永久升级 |
| M18 | 订单计数连续 | `MemberOrderCount` 不受规则系统开关影响 |
| C3 | 触发次数限制 | `trigger_count >= max_triggers` 时跳过规则 |

## 直推溢出安置（Spillover / Forced Matrix）

可选的强制矩阵模式。启用后每个会员的直推数受 `max_direct` 软限制，超出时新会员自动"溢出"到推荐人子树中有空位的后代节点下。

### 设计原理

```
未启用溢出（传统模式）:              启用溢出（max_direct=2）:

Alice                               Alice (max_direct=2)
├── Bob                             ├── Bob
├── Charlie                         └── Charlie
├── Dave     ← 无限制                    ├── Dave   ← 溢出安置
└── Eve                                  └── Eve    ← 溢出安置
```

### 两种注册路径

```
路径 A: 前端主导（链下 BFS + 链上验证）
┌──────────┐     ┌──────────────────┐     ┌─────────────────────────────┐
│ 前端/DApp │────▶│ 链下 BFS 搜索    │────▶│ register_member_with_spillover │
│          │     │ 计算 actual_ref  │     │ 链上验证子树+空位            │
└──────────┘     └──────────────────┘     └─────────────────────────────┘

路径 B: 系统自动（auto_register，订单触发）
┌──────────────┐     ┌──────────────────────┐
│ 订单完成触发  │────▶│ auto_register         │
│ commission   │     │ 内置有限 BFS (≤50步)  │
│              │     │ resolve_spillover_ref │
└──────────────┘     └──────────────────────┘
```

### 配置方式

```rust
// 启用: 每人最多 3 个直推，超出自动溢出
configure_spillover(shop_id, max_direct=3, enabled=true);

// 关闭: 回到传统自由推荐模式
configure_spillover(shop_id, max_direct=0, enabled=false);
```

- **完全可选** — 未配置的 Entity 行为完全不变
- **随时可开关** — 关闭后新注册恢复自由模式，已有溢出记录保留
- **参数可调** — `max_direct` 随时修改，立即生效

### 行为对比

| 场景 | 未配置 / disabled | enabled |
|------|-------------------|---------|
| 直推上限 | 仅 `MaxDirectReferrals`（硬上限 1000） | `max_direct` 软限制 |
| 普通 `register_member` | 正常绑定推荐人 | 超限时 `ReferralsFull` |
| `auto_register`（订单触发） | 直接绑定 | 自动 BFS 找空位溢出 |
| `register_member_with_spillover` | `SpilloverNotEnabled` | 链下算位 + 链上验证 |
| `MemberIntroducedBy` | 不写入 | 溢出时记录真实推荐人 |

### 辅助函数

| 函数 | 说明 |
|------|------|
| `is_descendant_of(entity_id, descendant, ancestor)` | 向上遍历验证祖先关系（最多 100 层） |
| `has_referral_capacity(entity_id, account, max_direct)` | 检查会员是否有直推空位 |
| `find_spillover_target(entity_id, root, max_direct)` | BFS 搜索子树中第一个有空位的节点（最多 50 步） |
| `resolve_spillover_referrer(entity_id, referrer)` | auto_register 路径的溢出解析封装 |

### 存储写入

| 操作 | 写入的 StorageItem |
|------|-------------------|
| `configure_spillover` | `SpilloverConfig` |
| `register_member_with_spillover` | `EntityMembers`, `DirectReferrals`, `MemberCount`, `MemberIntroducedBy`（溢出时） |
| `set_referrer_with_spillover` | `EntityMembers`, `DirectReferrals`, `MemberIntroducedBy`（溢出时） |
| `auto_register`（溢出路径） | 同上 + `SpilloverPlaced` 事件 |

## MemberProvider Trait

供 `commission`、`transaction`、`governance` 等模块跨 pallet 调用（参数均为 `entity_id`）：

```rust
pub trait MemberProvider<AccountId, Balance> {
    fn is_member(entity_id: u64, account: &AccountId) -> bool;
    fn member_level(entity_id: u64, account: &AccountId) -> Option<MemberLevel>;
    fn custom_level_id(entity_id: u64, account: &AccountId) -> u8;
    fn get_level_discount(entity_id: u64, level_id: u8) -> u16;
    fn get_level_commission_bonus(entity_id: u64, level_id: u8) -> u16;
    fn uses_custom_levels(entity_id: u64) -> bool;
    fn get_referrer(entity_id: u64, account: &AccountId) -> Option<AccountId>;
    fn auto_register(entity_id: u64, account: &AccountId, referrer: Option<AccountId>) -> DispatchResult;
    fn update_spent(entity_id: u64, account: &AccountId, amount: Balance, amount_usdt: u64) -> DispatchResult;
    fn check_order_upgrade_rules(entity_id: u64, buyer: &AccountId, product_id: u64, order_amount: Balance, amount_usdt: u64) -> DispatchResult;
    fn get_effective_level(entity_id: u64, account: &AccountId) -> u8;
    fn get_member_stats(entity_id: u64, account: &AccountId) -> (u32, u32, u128);
    fn is_activated(entity_id: u64, account: &AccountId) -> bool;
    fn get_introduced_by(entity_id: u64, account: &AccountId) -> Option<AccountId>;
}
```

- `custom_level_id` 内部调用 `get_effective_level`，自动处理等级过期
- `get_introduced_by` 返回溢出安置场景下的真实推荐人（无溢出时返回 `None`，此时 `get_referrer` 即为真实推荐人）

同时实现 `OrderMemberHandler` trait 供 `pallet-entity-transaction` 调用（`auto_register` + `update_spent`）。

另提供 `NullMemberProvider` 空实现，用于测试或不需要会员功能的场景。

## 治理桥接

以下内部函数无 origin 检查，通过 `MemberProviderBridge` 供 `pallet-entity-governance` 提案执行调用：

| 函数 | 说明 |
|------|------|
| `governance_set_custom_levels_enabled(entity_id, bool)` | 启用/禁用自定义等级 |
| `governance_set_upgrade_mode(entity_id, u8)` | 设置升级模式 (0=Auto, 1=Manual) |
| `governance_add_custom_level(entity_id, ...)` | 添加自定义等级 |
| `governance_update_custom_level(entity_id, ...)` | 更新自定义等级 |
| `governance_remove_custom_level(entity_id, level_id)` | 删除自定义等级 |

## 推荐关系与团队

```
Entity A 的会员推荐关系（溢出模式 max_direct=2）：

Alice (referrer: None)
├── Bob (referrer: Alice)         Alice: direct=2, team=5
│   ├── Dave (referrer: Bob)      Bob: direct=2, team=3
│   │   └── Frank (ref: Dave)     Dave: direct=1, team=1
│   │         introduced_by: Alice  ← 溢出安置（Alice 推荐，安置到 Dave 下）
│   └── Eve (referrer: Bob)
└── Carol (referrer: Alice)
    └── Grace (ref: Carol)        Carol: direct=1, team=1
```

- 新会员注册时递归向上更新 `team_size` 和 `indirect_referrals`（最大深度 100 层）
- `qualified_referrals` 仅计入主动注册/购买触发的直推，排除复购赠与
- 统计口径受 `MemberStatsPolicy` 控制
- 溢出安置时 `referrer` 记录 actual_referrer，`MemberIntroducedBy` 记录 intended_referrer

## 安全机制

- **循环推荐检测** — BTreeSet 已访问集合 + 最大 100 层深度，防止无限循环
- **自我推荐拦截** — `ensure!(referrer != who)`
- **推荐人验证** — 推荐人必须是同 Entity 的已注册会员
- **初始化幂等保护** — `init_level_system` / `init_upgrade_rule_system` 不可重复调用
- **规则目标验证** — 添加规则时校验 `target_level_id` 存在于等级系统
- **升级跳过保护** — `apply_upgrade` 在等级被删除后静默跳过（不升到不存在的等级）
- **基点范围校验** — `discount_rate` / `commission_bonus` ≤ 10000
- **策略位掩码** — 注册策略仅低 3 位、统计策略仅低 2 位有效
- **Entity 级存储** — 会员按 `entity_id` 存储，避免 shop_id=0 回退污染真实数据
- **溢出子树验证** — `register_member_with_spillover` 验证 actual_referrer 在 intended_referrer 子树中
- **溢出容量检查** — 链上验证目标节点确实有空位（`has_referral_capacity`）
- **有限 BFS 搜索** — auto_register 路径最多 50 步防止 DoS

## 测试

```bash
cargo test -p pallet-entity-member
```

81 个测试覆盖：注册流程、推荐关系、循环检测、等级升降、规则引擎生命周期、过期回退、叠加语义、冲突策略、降级防护、AutoUpgrade 交互、策略组合、治理桥接、USDT 精度、溢出安置（配置/注册/绑定/BFS 搜索/深层溢出/禁用回退）等。
