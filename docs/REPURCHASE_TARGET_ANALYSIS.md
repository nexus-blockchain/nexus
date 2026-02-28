# 复购账户代注册/代购可行性与合理性分析

> **审计范围**: `pallet-commission-core` 的 `withdraw_commission` 中 `repurchase_target` 机制
> **相关文件**: `pallets/entity/commission/core/src/lib.rs`, `pallets/entity/member/src/lib.rs`, `pallets/entity/common/src/lib.rs`

---

## 1. 机制概述

`withdraw_commission` 的 `repurchase_target: Option<T::AccountId>` 参数允许会员 A（出资人）将提现时产生的复购金额转入另一个账户 B（目标）的购物余额。

### 完整流程

```text
A 调用 withdraw_commission(entity_id, amount, rate, target=B)
  │
  ├─ 1. target = B（若未指定则默认为 A 自己）
  │
  ├─ 2. 身份与关系校验:
  │     ├─ B 已是会员 → 校验 A 是 B 的直推人 (NotDirectReferral)
  │     └─ B 非会员  → auto_register_by_entity(entity_id, B, referrer=A)
  │           ├─ REFERRAL_REQUIRED 且 A 无效 → 返回错误，整体中止
  │           ├─ APPROVAL_REQUIRED → B 进入 PendingMembers，返回 Ok(())
  │           └─ 无特殊策略 → B 立即注册为会员，A 为推荐人
  │
  ├─ 3. 计算分配: calc_withdrawal_split(entity_id, A, amount, rate)
  │     → WithdrawalSplit { withdrawal, repurchase, bonus }
  │
  ├─ 4. 偿付安全检查 (Entity 账户余额 ≥ withdrawal + remaining_pending + new_shopping)
  │
  ├─ 5. 资金分配:
  │     ├─ withdrawal → NEX 从 Entity 转入 A 钱包
  │     └─ repurchase + bonus → MemberShoppingBalance[entity_id][B] 记账增加
  │
  ├─ 6. 统计更新: A.stats.repurchased += repurchase + bonus
  │
  └─ 7. 事件: TieredWithdrawal { account: A, repurchase_target: B, ... }

B 后续使用购物余额:
  ├─ use_shopping_balance(entity_id, amount) → do_consume_shopping_balance
  │     → 扣减记账 + NEX 从 Entity 转入 B 钱包
  └─ place_order 时通过 ShoppingBalanceProvider::consume_shopping_balance
        → 扣减记账 + NEX 从 Entity 转入 B 钱包（B 随后通过 Escrow 支付订单）
```

---

## 2. 可行性分析

### 2.1 已有的安全约束

| 约束 | 实现位置 | 说明 |
|------|---------|------|
| **直推关系校验** | `lib.rs:527-531` | B 已是会员时，A 必须是 B 的直推人；防止跨线赠送 |
| **自动注册绑定推荐关系** | `lib.rs:534` | B 非会员时自动注册，A 成为 B 的推荐人；确保推荐树一致性 |
| **偿付安全检查** | `lib.rs:574-588` | `withdraw_commission` 中检查 Entity 余额覆盖 `withdrawal + pending + shopping` |
| **WithdrawalConfig 启用检查** | `lib.rs:551-556` | H1 审计修复，disabled 时拒绝提现 |
| **APPROVAL_REQUIRED 后验** | `lib.rs:536-541` | H1 修复，auto_register 后验证 target 已成为正式会员 |
| **未激活会员提现限制** | `lib.rs:847-851` | M2 方案 B，`do_consume_shopping_balance` 检查 `activated` |
| **KYC 参与权检查 (target)** | `lib.rs:544-548` | H3 修复，`withdraw_commission` 中 target 写入购物余额前检查 `ParticipationGuard` |
| **KYC 参与权检查 (提现)** | `lib.rs:853-857` | H3 修复，`do_consume_shopping_balance` 中 NEX 转账前检查 `ParticipationGuard` |
| **事件追踪** | `lib.rs:622-628` | M3 审计修复，TieredWithdrawal 包含 `repurchase_target` 字段 |

### 2.2 发现的问题

#### H1 (High): APPROVAL_REQUIRED 策略下购物余额提前发放 — ✅ 已修复

**路径**: `withdraw_commission` → `auto_register_by_entity` → `requires_approval()` → `PendingMembers::insert` → `return Ok(())`

> **修复状态**: 已在 `commission/core/src/lib.rs:536-541` 添加 `TargetNotApprovedMember` 检查。

当 Entity 配置了 `APPROVAL_REQUIRED` 注册策略时，`auto_register_by_entity` 将 B 放入 `PendingMembers` 并返回 `Ok(())`。修复前，`withdraw_commission` 不检查返回后 B 的实际会员状态，继续将购物余额写入 B：

```rust
// auto_register_by_entity (member/src/lib.rs:2184-2194)
if policy.requires_approval() {
    if !PendingMembers::<T>::contains_key(entity_id, account) {
        PendingMembers::<T>::insert(entity_id, account, valid_referrer.clone());
    }
    return Ok(());  // ← 返回成功，但 B 未成为正式会员
}

// withdraw_commission (commission/core/src/lib.rs:560-568)
// ↓ 无论 B 是否已通过审批，购物余额照常发放
if !total_to_shopping.is_zero() {
    MemberShoppingBalance::<T>::mutate(entity_id, &target, |balance| {
        *balance = balance.saturating_add(total_to_shopping);
    });
}
```

**后果**:
- B 未通过审批但已获得购物余额
- B 调用 `use_shopping_balance` 提取 NEX（该 extrinsic 无会员资格检查）
- **Entity 资金流出给未审批账户，完全绕过 APPROVAL_REQUIRED 策略**

**建议修复**:
```rust
// 在 withdraw_commission 中，auto_register 之后检查 B 是否为正式会员
if target != who {
    if T::MemberProvider::is_member_by_entity(entity_id, &target) {
        // 已是会员（原已是 or 刚注册成功）→ 继续
        let referrer = T::MemberProvider::get_referrer_by_entity(entity_id, &target);
        ensure!(referrer.as_ref() == Some(&who), Error::<T>::NotDirectReferral);
    } else {
        T::MemberProvider::auto_register_by_entity(entity_id, &target, Some(who.clone()))
            .map_err(|_| Error::<T>::AutoRegisterFailed)?;
        // 注册后再次检查是否为正式会员（排除 APPROVAL_REQUIRED 的待审批状态）
        ensure!(
            T::MemberProvider::is_member_by_entity(entity_id, &target),
            Error::<T>::TargetNotApprovedMember  // 新错误码
        );
    }
}
```

---

#### H2 (High): `do_consume_shopping_balance` 缺少偿付安全检查

`do_consume_shopping_balance`（用户修改后新增的函数）执行实际的 NEX 转账，但不检查转账后 Entity 余额是否仍能覆盖剩余义务（`ShopPendingTotal + 剩余 ShopShoppingTotal`）。

```rust
// commission/core/src/lib.rs:801-833
pub fn do_consume_shopping_balance(entity_id, account, amount) {
    // 仅检查会员购物余额 ≥ amount
    ensure!(*balance >= amount, InsufficientShoppingBalance);
    *balance -= amount;
    ShopShoppingTotal -= amount;
    // 直接转账，未检查 Entity 偿付能力
    Currency::transfer(&entity_account, account, amount, KeepAlive)?;
}
```

**竞态场景**:
```text
初始: Entity余额=100, PendingTotal=40, ShoppingTotal=80

1. A 消费 60 购物余额:
   Entity余额 → 40, ShoppingTotal → 20
   
2. 此时: Entity余额(40) < PendingTotal(40) + ShoppingTotal(20) = 60
   → 偿付缺口 20，后续提现可能失败
```

虽然 `withdraw_commission` 在分配时做了偿付检查，但 `do_consume_shopping_balance` 减少了 Entity 余额但同时也减少了 `ShopShoppingTotal`，所以实际上：

| 操作 | Entity余额变化 | 义务变化 | 净影响 |
|------|---------------|---------|--------|
| consume_shopping_balance(60) | -60 | ShoppingTotal -60 | 0（平衡）|

**更正**: 仔细分析后，`do_consume_shopping_balance` 的资产转出和义务减少是等额的——Entity 余额减少 X，`ShopShoppingTotal` 也减少 X。因此偿付比率不变。**此项降级为 Info**，但仍建议添加防御性检查以应对极端情况（Entity 余额被外部操作减少）：

```rust
// 建议添加的防御性检查
let entity_account = T::EntityProvider::entity_account(entity_id);
let entity_balance = T::Currency::free_balance(&entity_account);
let remaining_obligations = ShopPendingTotal::<T>::get(entity_id)
    .saturating_add(ShopShoppingTotal::<T>::get(entity_id)); // 已扣减后的值
ensure!(
    entity_balance.saturating_sub(amount) >= remaining_obligations,
    Error::<T>::EntityInsolvencyRisk
);
```

---

#### M1 (Medium): 统计归属不一致

`stats.repurchased` 记在 A（出资人）名下，但购物余额记在 B（target）名下：

```rust
// A 的统计
stats.repurchased += split.repurchase + split.bonus;

// B 的购物余额
MemberShoppingBalance[entity_id][B] += split.repurchase + split.bonus;
```

**影响**:
- `sum(所有会员的 stats.repurchased)` ≠ `ShopShoppingTotal`（因为 B 的购物余额不体现在 B 的 `repurchased` 中）
- Entity 运营方无法仅从 `MemberCommissionStats` 推导出每个会员的实际购物余额来源
- 链下审计/对账困难

**建议**: 在 B 的统计中新增 `received_repurchase` 字段，或在事件中明确区分资金归属。最小化修复方案为在 `TieredWithdrawal` 事件中已包含 `repurchase_target`（M3 已修复），链下系统可据此重建正确归属。

---

#### M2 (Medium→High): 代注册产生"幽灵会员" — 零消费会员与等级体系的深层冲突 — ✅ 已修复 (方案 B)

##### B 注册后的精确状态

`auto_register_by_entity` → `do_register_member` 创建的 B 会员记录：

```rust
// member/src/lib.rs — do_register_member (activated=false 路径)
EntityMember {
    referrer: Some(A),              // A 是推荐人
    direct_referrals: 0,
    indirect_referrals: None,       // 预留字段
    team_size: 0,
    total_spent: Zero::zero(),      // ← 零消费
    level: MemberLevel::Normal,     // ← 全局最低等级
    custom_level_id: 0,             // ← 自定义等级体系的"无等级"
    joined_at: now,
    last_active_at: now,
    activated: false,               // ← 方案 B: 代注册未激活
}
```

> **注**: `activated: false` 和统计延迟已在代码中实现（方案 B）。B 不计入 `MemberCount`、推荐人的 `direct_referrals` / `team_size`，直到首次 `update_spent` 触发激活。

##### 两套等级体系的状态

| 等级体系 | B 的等级 | 含义 | 升级条件 |
|---------|---------|------|---------|
| **全局等级** (`MemberLevel`) | `Normal` (0) | 普通会员 | `total_spent_usdt >= SilverThreshold` |
| **自定义等级** (`custom_level_id`) | `0` | 低于任何已定义等级 | `total_spent >= levels[0].threshold` |

全局等级 `Normal` 不需要消费即可获得（默认值）。但自定义等级体系中，即使最低的等级（如 `id=0, name="VIP1", threshold=100`）也需要 `total_spent >= threshold`。B 的 `total_spent = 0` 意味着 **B 在自定义等级体系中处于所有已定义等级之下**。

##### `PURCHASE_REQUIRED` 策略的语义冲突

```rust
// common/src/lib.rs:298-299
/// 必须先消费（auto_register 触发）才能成为会员
pub const PURCHASE_REQUIRED: u8 = 0b0000_0001;
```

策略定义明确说 **"必须先消费才能成为会员"**。两条注册路径的对比：

| 路径 | 是否消费 | PURCHASE_REQUIRED 检查 | 结果 |
|------|---------|----------------------|------|
| `register_member`（手动注册） | 否 | `ensure!(!policy.requires_purchase())` → **拒绝** | ✅ 正确 |
| `auto_register`（下单触发） | 是（正在下单） | 跳过（注释："auto_register 由购买触发"） | ✅ 正确 |
| `auto_register_by_entity`（repurchase_target） | **否**（B 未消费） | 跳过（同上逻辑） | ❌ **绕过** |

`auto_register` / `auto_register_by_entity` 跳过 `PURCHASE_REQUIRED` 检查的前提是"调用方已确认是购买行为"。但 `repurchase_target` 场景中 **B 从未购买任何商品**——是 A 的佣金复购赠送。

##### "幽灵会员"的生命周期

```text
阶段 1: 代注册（零消费会员诞生）
  A 调用 withdraw_commission(target=B)
  → B 成为会员: total_spent=0, level=Normal, custom_level_id=0, activated=false
  → B 获得购物余额（如 3000 NEX）
  → A.stats.repurchased += 3000
  → [方案 B] A.direct_referrals 和 team_size 不变（延迟到激活时补偿）
  → [方案 B] MemberCount 不增加（延迟到激活时补偿）

阶段 2: 待激活期（B 持有购物余额但未消费）
  → B 在 EntityMembers 中存在，但 activated=false
  → [方案 B] B 不计入 MemberCount（已修复）
  → [方案 B] B 不计入推荐人统计（已修复）
  → [方案 B] B 调用 use_shopping_balance → 被 MemberNotActivated 拒绝（已修复）
  → B 的 total_spent 始终为 0 → 全局等级永远是 Normal

阶段 3a: B 使用购物余额下单（激活）
  → place_order 使用购物余额
  → 订单完成 → update_spent(B, order_amount)
  → [方案 B] activated: false → true，补偿 MemberCount +1 和推荐人统计
  → total_spent > 0 → 全局等级可能升级（Normal → Silver 等）
  → 自定义等级可能升级（custom_level_id 0 → 1 等）
  → 此时 B 才真正符合 PURCHASE_REQUIRED 的语义

阶段 3b: B 尝试直接提取 NEX（已阻断）
  → [方案 B] use_shopping_balance → MemberNotActivated 错误，拒绝提现
  → B 必须先通过阶段 3a 激活后才能提取 NEX
```

##### 对佣金系统的具体影响

1. **LevelBased 提现模式**: B 如果后续也赚取佣金并提现，`custom_level_id=0` 没有对应的 `level_overrides` 配置 → 回退到 `default_tier`。这可能导致 B 的复购比率与 Entity 的期望不符。

2. **统计膨胀**: A 的 `direct_referrals` 和 `team_size` 计入 B，即使 B 从未消费。在 `LevelBased` 或升级规则中，如果有"团队人数"指标，幽灵会员会虚增数据。

3. **MemberCount 失真**: Entity 的 `MemberCount` 包含幽灵会员，运营方看到的"会员数"高于实际活跃/消费会员数。

##### 建议

**>>> 采纳方案 B: 标记为"待激活"会员 <<<**

**核心思路**: 在 `EntityMember` 中新增 `activated: bool` 字段，区分"正常注册的会员"和"通过 repurchase_target 代注册的会员"。未激活会员可持有购物余额，但不计入统计、不参与佣金分配，直到首次真实消费后自动激活。

**1. 数据结构变更 (`pallet-entity-member`)**

```rust
// member/src/lib.rs — EntityMember 结构体
pub struct EntityMember<AccountId, Balance, BlockNumber> {
    pub referrer: Option<AccountId>,
    pub direct_referrals: u32,
    pub indirect_referrals: Option<u32>,  // +++ NEW: 间接推荐人数（预留，后期扩展）
    pub team_size: u32,
    pub total_spent: Balance,
    pub level: MemberLevel,
    pub custom_level_id: u8,
    pub joined_at: BlockNumber,
    pub last_active_at: BlockNumber,
    pub activated: bool,                  // +++ NEW: false = pending activation
}
```

**2. 注册路径行为变更**

| 注册路径 | `activated` 值 | 原因 |
|---------|---------------|------|
| `register_member` (手动注册) | `true` | 用户主动注册，符合正常流程 |
| `auto_register` (下单触发) | `true` | 由真实购买行为触发 |
| `auto_register_by_entity` (repurchase_target) | **`false`** | B 未消费，代注册 |
| `approve_member` (审批通过) | `true` | 管理员已审批 |

**3. 激活逻辑 (`update_spent`)**

```rust
// member/src/lib.rs — update_spent 中添加:
if !member.activated {
    member.activated = true;
    // 补偿注册时跳过的统计
    MemberCount::<T>::mutate(entity_id, |c| *c = c.saturating_add(1));
    if let Some(ref ref_account) = member.referrer {
        Self::mutate_member_referral(entity_id, ref_account, account)?;
    }
    Self::deposit_event(Event::MemberActivated {
        entity_id,
        account: account.clone(),
    });
}
```

**4. `do_register_member` 变更**

```rust
// member/src/lib.rs — do_register_member 新增 activated 参数:
fn do_register_member(
    entity_id: u64,
    shop_id: u64,
    account: &T::AccountId,
    referrer: Option<T::AccountId>,
    activated: bool,                // +++ NEW
) -> DispatchResult {
    let member = EntityMember {
        // ... existing fields ...
        activated,
    };
    EntityMembers::<T>::insert(entity_id, account, member);

    if activated {
        // 正常流程: 立即计入统计
        MemberCount::<T>::mutate(entity_id, |count| *count = count.saturating_add(1));
        if let Some(ref ref_account) = referrer {
            Self::mutate_member_referral(entity_id, ref_account, account)?;
        }
    }
    // activated=false: 延迟统计，等待首次消费激活

    Ok(())
}
```

**5. 未激活会员的权限隔离**

| 能力 | 未激活 (`activated=false`) | 已激活 (`activated=true`) |
|------|--------------------------|--------------------------|
| 持有购物余额 | YES | YES |
| 使用购物余额下单 | YES (触发激活) | YES |
| `use_shopping_balance` 提取 NEX | **NO** (需先消费激活) | YES |
| 计入 `MemberCount` | NO | YES |
| 计入推荐人 `direct_referrals` / `team_size` | NO | YES |
| 参与佣金分配 (作为推荐人) | NO | YES |
| `LevelBased` 提现模式 | N/A (无佣金) | YES |
| `is_member_by_entity` 返回值 | `true` (存储中存在) | `true` |

**6. `use_shopping_balance` 限制**

```rust
// commission/core/src/lib.rs — use_shopping_balance / do_consume_shopping_balance:
// 未激活会员不能直接提取 NEX，必须先通过下单消费激活
ensure!(
    T::MemberProvider::is_activated_by_entity(entity_id, &who),
    Error::<T>::MemberNotActivated
);
```

这确保 B 不能跳过消费直接套现购物余额为 NEX，必须先下单（触发 `update_spent` -> `activated = true`）。

**7. 新增 MemberProvider trait 方法**

```rust
// common/src/lib.rs — MemberProvider trait:
fn is_activated_by_entity(entity_id: u64, account: &AccountId) -> bool;
```

**8. 新增事件和错误**

```rust
// 事件:
MemberActivated { entity_id: u64, account: T::AccountId }

// 错误:
MemberNotActivated  // use_shopping_balance 时未激活会员被拒
```

**9. 存储迁移**

需要 migration 为所有已有 `EntityMember` 添加 `activated: true`（历史会员均视为已激活）。

**方案优势**:
- B 可以接受复购赠送并持有购物余额（业务灵活性保留）
- B 必须真实消费后才计入统计和参与佣金（消除幽灵会员）
- B 不能直接 `use_shopping_balance` 套现（关闭套现漏洞）
- 现有会员不受影响（migration 默认 `activated=true`）

**备选方案 (未采纳)**:
- 方案 A（严格）: repurchase_target 仅允许已注册会员 — 过于限制，不支持"拉新赠送"场景
- 方案 C（最小改动）: `auto_register_by_entity` 检查 PURCHASE_REQUIRED — 仅在有该策略的 Entity 生效，无策略时仍有幽灵会员

---

#### H3 (High): 已配置 KYC 的 Entity 的要求被复购路径绕过 — ✅ 已修复

> **修复状态**: 已引入 `ParticipationGuard` trait 到 `pallet-commission-core`，在 `withdraw_commission`（target 写入购物余额前）和 `do_consume_shopping_balance`（NEX 转账前）两处添加检查。Runtime 通过 `KycParticipationGuard` 桥接 `pallet-entity-kyc::can_participate_in_entity`。新增错误码 `TargetParticipationDenied` 和 `ParticipationRequirementNotMet`。3 个回归测试已通过。

**背景**: KYC 是 **Entity 可选配置**，并非全局强制。系统通过 `EntityRequirements` 存储让每个 Entity 自主决定是否启用 KYC：

```rust
// pallet-entity-kyc: can_participate_in_entity (kyc/src/lib.rs:797-838)
pub fn can_participate_in_entity(account, entity_id) -> bool {
    if let Some(requirement) = EntityRequirements::<T>::get(entity_id) {
        if !requirement.mandatory {
            return true;  // Entity 未强制 → 任何人可参与
        }
        // mandatory=true → 检查 KYC 状态、级别、国家、风险评分、过期
        ...
    }
    true  // 无 EntityRequirements → 默认允许（不需要 KYC）
}
```

即：**大多数 Entity 不配置 KYC 时，此问题不存在**。问题仅影响那些 **主动配置了 `EntityRequirements { mandatory: true, ... }` 的 Entity**。

系统通过两套独立机制实施 KYC 管控：

| 机制 | 位置 | 作用域 | 调用情况 |
|------|------|--------|---------|
| `can_participate_in_entity()` | `pallet-entity-kyc` | Entity 参与权（按 `EntityRequirements` 配置，Entity 可选） | **全项目零外部调用** — 仅在 kyc 测试中使用 |
| `KycLevelProvider::meets_kyc_requirement()` | `pallet-entity-token` | Entity Token 转账限制（`TransferRestrictionMode::KycRequired`） | 仅对 Entity Token 转账生效 |

**问题**: 当 Entity **已配置 mandatory KYC** 时，`pallet-commission-core` 的复购流程仍 **不调用 `can_participate_in_entity()`**：

```text
前提: Entity 配置了 EntityRequirements { mandatory: true, min_level: Enhanced }

withdraw_commission(target=B)
  ├─ auto_register_by_entity(B)     ← 无 KYC 检查（不调用 can_participate_in_entity）
  ├─ MemberShoppingBalance[B] += X  ← 无 KYC 检查
  └─ 后续: B 调用 use_shopping_balance
       └─ do_consume_shopping_balance
            └─ Currency::transfer(Entity → B)  ← NEX（原生代币），不走 Entity Token 的 KYC 检查
```

关键路径分析：

1. **`auto_register_by_entity`** (`member/src/lib.rs:2157-2206`) — 注册流程不查询 `EntityRequirements` 的 KYC 配置，不调用 `can_participate_in_entity()`
2. **`withdraw_commission`** (`commission/core/src/lib.rs:486-593`) — 无 `KycProvider` 依赖，Config 中未声明 KYC 接口
3. **`do_consume_shopping_balance`** (`commission/core/src/lib.rs:801-833`) — 使用 `Currency::transfer`（原生 NEX），不经过 `pallet-entity-token` 的 `TransferRestrictionMode` 检查
4. **`can_participate_in_entity`** (`kyc/src/lib.rs:797-838`) — 函数已实现且包含完善的 Entity 级可选 KYC 逻辑，但 **零外部调用**，完全悬空

**攻击场景**（仅在 Entity 配置了 mandatory KYC 时成立）:

```text
Entity 配置: EntityRequirements { mandatory: true, min_level: KycLevel::Enhanced }

1. A（已 KYC）有 10000 待提现佣金
2. B（未 KYC，无任何 KYC 记录）
3. A 调用 withdraw_commission(entity_id, 10000, rate, target=B)
   → B 自动注册为会员，activated=false
   → [H3 第一层阻断] TargetParticipationDenied 错误，B 未通过 KYC，整个交易回滚
   → B 不会获得购物余额（攻击在此终止）

--- 以下为纵深防御（即使第一层被绕过） ---
4a. B 调用 use_shopping_balance(entity_id, amount)
   → [方案 B 阻断] MemberNotActivated 错误（未激活会员不能提现）
   → [H3 第二层阻断] ParticipationRequirementNotMet 错误（未 KYC 不能提现）
4b. B 先下单消费激活，再调用 use_shopping_balance
   → [H3 第二层阻断] ParticipationRequirementNotMet 错误，激活后仍被 KYC 拦截
   → B 必须完成 KYC 验证后才能提取 NEX
```

> **修复状态**: 方案 B（`MemberNotActivated`）+ H3（`ParticipationGuard`）双重防护。方案 B 阻断未激活会员直接提现，`ParticipationGuard` 在 `withdraw_commission`（target 写入购物余额前）和 `do_consume_shopping_balance`（NEX 转账前）两处检查 KYC，从根本上阻断未 KYC 账户接收资金。

**注意**: 如果 Entity **未配置** `EntityRequirements` 或设置 `mandatory: false`，则 `can_participate_in_entity()` 返回 `true`，此攻击场景不成立，所有账户均可正常参与。

**影响**（仅限已配置 mandatory KYC 的 Entity）:
- **合规风险**: Entity 明确要求 KYC 才能参与，但通过 repurchase_target 路径完全绕过
- **AML 风险**: 未实名账户可通过此路径接收资金，违反该 Entity 的反洗钱要求
- **监管暴露**: Entity 因合规要求开启 KYC，此漏洞使其 KYC 策略形同虚设

**根因**: `can_participate_in_entity()` 是一个"建好但没接上"的函数。KYC 模块定义了完善的 Entity 级可选参与要求检查（不配置即不强制），但没有任何模块在关键业务路径上调用它。

**建议修复（两层）**:

**第一层: commission-core 侧（直接防护）**

在 `pallet-commission-core` 的 Config 中引入 KYC 检查接口：

```rust
// commission/core/src/lib.rs — Config trait
#[pallet::config]
pub trait Config: frame_system::Config {
    // ... 现有配置 ...

    /// KYC / 实体参与检查（默认空实现允许所有操作）
    type ParticipationGuard: ParticipationGuard<Self::AccountId>;
}

/// 参与权守卫 trait
pub trait ParticipationGuard<AccountId> {
    fn can_participate(entity_id: u64, account: &AccountId) -> bool;
}

/// 默认空实现（无 KYC 系统时使用）
impl<AccountId> ParticipationGuard<AccountId> for () {
    fn can_participate(_: u64, _: &AccountId) -> bool { true }
}
```

在 `withdraw_commission` 中添加检查：

```rust
// 在 target != who 分支中，auto_register 之后、写入购物余额之前
ensure!(
    T::ParticipationGuard::can_participate(entity_id, &target),
    Error::<T>::TargetParticipationDenied  // 新错误码
);
```

在 `do_consume_shopping_balance` 中添加检查：

```rust
// 在 Currency::transfer 之前
ensure!(
    T::ParticipationGuard::can_participate(entity_id, account),
    Error::<T>::ParticipationRequirementNotMet  // 新错误码
);
```

**第二层: Runtime 桥接**

在 Runtime 配置中将 `pallet-entity-kyc::can_participate_in_entity` 接入：

```rust
// runtime/src/configs/mod.rs
pub struct KycParticipationGuard;
impl pallet_commission_core::ParticipationGuard<AccountId> for KycParticipationGuard {
    fn can_participate(entity_id: u64, account: &AccountId) -> bool {
        pallet_entity_kyc::Pallet::<Runtime>::can_participate_in_entity(account, entity_id)
    }
}

impl pallet_commission_core::Config for Runtime {
    // ...
    type ParticipationGuard = KycParticipationGuard;
}
```

---

## 3. 合理性评估

### 3.1 业务合理性

| 场景 | 合理性 | 说明 |
|------|--------|------|
| 上级为直推下线充值购物余额 | ✅ 高 | 分销系统常见需求，鼓励下线消费 |
| 拉新注册（赠送购物余额吸引新用户） | ✅ 高 | 降低新用户入门门槛 |
| 家庭账户（父母→子女） | ✅ 中 | 需直推关系约束，合理 |

### 3.2 经济模型合理性

| 维度 | 评估 | 说明 |
|------|------|------|
| **复购率强制** | ✅ 安全 | 复购率由三层约束模型决定，与 target 无关 |
| **Governance 底线** | ✅ 安全 | `calc_withdrawal_split` 的 `gov_min_rate` 不受 target 影响 |
| **自愿奖励** | ✅ 安全 | `voluntary_bonus_rate` 按超额部分计算，target 不影响计算 |
| **防套现** | ✅ 已缓解 | 方案 B: 未激活会员不能直接 `use_shopping_balance` 提现，必须先消费激活 (`MemberNotActivated` 检查) |

### 3.3 安全性评估

| 攻击向量 | 风险 | 现有防护 | 缺口 |
|----------|------|---------|------|
| 跨线赠送（绕过推荐树） | 低 | `NotDirectReferral` 校验 | 无 |
| 批量建号套利 | 中 | 直推关系约束 | A 可创建多个 sybil 账户作为“下线”分散购物余额 |
| 未审批会员提取资金 | ~~高~~ | `TargetNotApprovedMember` 检查 | **H1 已修复**: auto_register 后验证会员状态 |
| 幽灵会员绕过 PURCHASE_REQUIRED | ~~高~~ | `activated` + `MemberNotActivated` | **M2 已修复 (方案 B)**: 代注册会员未激活，不计入统计，不能提现 |
| 未 KYC 账户接收 NEX | ~~高~~ | `ParticipationGuard` + `KycParticipationGuard` | **H3 已修复**: withdraw_commission + do_consume_shopping_balance 两处检查 |
| Entity 偿付不足 | 低 | `withdraw_commission` 偿付检查 | H2：`do_consume_shopping_balance` 无独立检查（已降级为 Info） |

---

## 4. 总结

### 4.1 可行性结论

`repurchase_target` 机制在**无特殊注册/KYC 策略**的 Entity 中是可行且安全的。推荐关系约束、偿付检查和事件追踪提供了基本的安全保障。

已修复的漏洞：
- **H1** ✅: APPROVAL_REQUIRED 策略下购物余额发放给未审批账户 — 已通过 `TargetNotApprovedMember` 后验修复
- **M2→High** ✅: 代注册产生“幽灵会员”绕过 PURCHASE_REQUIRED — 已通过方案 B（`activated` 字段 + 统计延迟 + 提现限制）修复
- **H3** ✅: Entity 配置了 mandatory KYC 要求时，repurchase_target 路径绕过 KYC 检查 — 已通过 `ParticipationGuard` trait + `KycParticipationGuard` runtime 桥接修复

**所有阻断性漏洞（H1、H3、M2→High）均已修复，该机制可安全使用。**

### 4.2 发现汇总

| ID | 严重度 | 问题 | 状态 |
|----|--------|------|------|
| H1 | **High** | APPROVAL_REQUIRED 下购物余额提前发放给未审批账户 | **已修复** |
| H3 | **High** | KYC 要求被完全绕过 — `can_participate_in_entity()` 悬空，commission-core 无 KYC 接口 | **已修复** |
| H2→Info | Info | `do_consume_shopping_balance` 偿付检查缺失（经分析义务等额减少，降级） | 建议防御性检查 |
| M1 | Medium | 统计归属不一致（repurchased 记在出资人，余额在 target） | 建议文档说明 |
| M2→High | **Medium→High** | 代注册产生"幽灵会员" — 零消费 `total_spent=0` 的会员绕过 PURCHASE_REQUIRED，膨胀统计 | **已修复 (方案 B)** |

### 4.3 建议

1. ~~**H1**~~: ✅ **已修复** — `withdraw_commission` 中 `auto_register_by_entity` 后添加 `TargetNotApprovedMember` 检查。
2. ~~**H3**~~: ✅ **已修复** — 引入 `ParticipationGuard` trait + `KycParticipationGuard` runtime 桥接。`withdraw_commission`（target 写入购物余额前）和 `do_consume_shopping_balance`（NEX 转账前）两处添加检查。3 个回归测试已通过。
3. ~~**M2→High**~~: ✅ **已修复 (方案 B)** — `EntityMember` 新增 `activated: bool` 字段，代注册会员 `activated=false`，首次消费后自动激活。待完成存储迁移（历史会员默认 `activated=true`）。
4. **建议添加 H2 防御性检查**: 在 `do_consume_shopping_balance` 中添加偿付安全断言。
5. **文档补充**: 统计归属(M1)的链下对账说明。

### 4.4 修复优先级路线图

```text
Phase 1 (阻断性修复 — 上线前必须完成):
  [DONE] H1: withdraw_commission 检查 target 会员状态（TargetNotApprovedMember）
  [DONE] H3: 引入 ParticipationGuard + Runtime KYC 桥接（仅影响 mandatory KYC 的 Entity）
  [DONE] M2→High (方案 B): 待激活会员机制
    ✔ EntityMember 新增 activated: bool + indirect_referrals: Option<u32>
    ✔ do_register_member 新增 activated 参数
    ✔ auto_register_by_entity (repurchase_target) → activated=false
    ✔ register_member / auto_register / approve_member → activated=true
    ✔ update_spent 首次消费时自动激活 + 补偿统计
    ✔ do_consume_shopping_balance 检查 activated (MemberNotActivated)
    ✔ MemberProvider trait 新增 is_activated_by_entity() + runtime bridge
    ✔ 新事件 MemberActivated + 新错误 MemberNotActivated
    ○ 待完成: 存储迁移 (历史会员默认 activated=true)

Phase 2 (加固):
  H2→Info: do_consume_shopping_balance 防御性偿付检查

Phase 3 (完善):
  M1: 统计归属优化或文档说明
  can_participate_in_entity 接入其他关键路径（member 注册、transaction 下单等）
