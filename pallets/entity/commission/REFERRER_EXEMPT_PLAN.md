# 推荐人提成免除：基于推荐链深度的全局治理方案

## 实施目标

通过链上全局治理参数，当 Entity 的任意链式插件（multi-level / single-line / level-diff / team）推荐链深度超过指定阈值时，自动免除该 Entity 推荐人的招商提成（平台费全部归国库），防止深层分销团队成员拉队伍另立门户。

## 核心规则

```
ReferrerExemptThreshold = N（全局治理参数，默认 5）

每笔订单 Pool A 分配前:
  max_depth = max(
    multi-level tier_count,
    single-line max(max_upline_levels, max_downline_levels),
    level-diff max_depth,
    team max_depth,
  )
  IF  N > 0  AND  max_depth > N
  THEN  推荐人不拿提成，平台费 100% 归国库
  ELSE  推荐人正常拿 ReferrerShareBps% 的平台费
```

### 检测覆盖的 4 个插件

| 插件 | 深度字段 | 最大值 | QueryProvider 方法 |
|---|---|---|---|
| multi-level | `levels.len()` | 1–1000 | `tier_count` |
| single-line | `max(max_upline_levels, max_downline_levels)` | 1–255 | `chain_depth` |
| level-diff | `max_depth` | 1–20 | `chain_depth` |
| team | `max_depth` | 1–30 | `chain_depth` |

---

## 改动清单（7 个文件，~110 行）

### Step 1: MultiLevelQueryProvider 新增 tier_count 方法

**文件**: `commission/common/src/lib.rs`

在 `MultiLevelQueryProvider` trait 中新增默认方法：

```rust
pub trait MultiLevelQueryProvider<AccountId> {
    fn activation_progress(entity_id: u64, account: &AccountId) -> Vec<MultiLevelActivationInfo>;
    fn is_paused(entity_id: u64) -> bool;
    fn member_stats(entity_id: u64, account: &AccountId) -> Option<MultiLevelMemberStats>;

    // ── 新增 ──
    /// 查询 Entity 多级分销配置的层数（无配置返回 0）
    fn tier_count(entity_id: u64) -> u16 { 0 }
}
```

空实现 `()` 无需改动（默认方法返回 0）。

---

### Step 2: multi-level 插件实现 tier_count

**文件**: `commission/multi-level/src/lib.rs`

在 `impl<T: Config> MultiLevelQueryProvider<T::AccountId> for Pallet<T>` 块中新增：

```rust
fn tier_count(entity_id: u64) -> u16 {
    MultiLevelConfigs::<T>::get(entity_id)
        .map(|c| c.levels.len() as u16)
        .unwrap_or(0)
}
```

---

### Step 3: commission-core 新增存储项 + 事件 + 错误

**文件**: `commission/core/src/lib.rs`

#### 3.1 Storage — 在 `GlobalCommissionPaused` 后追加

```rust
/// 全局治理：多级分销层数超过此阈值的 Entity，免除推荐人招商提成
/// 平台费全部归国库，防止深层分销团队成员拉队伍另立门户
/// 默认值 5：多级分销超过 5 层即自动免除推荐人提成
/// 设为 0 可禁用此规则
#[pallet::storage]
#[pallet::getter(fn referrer_exempt_threshold)]
pub type ReferrerExemptThreshold<T: Config> = StorageValue<_, u16, ValueQuery, ConstU16<5>>;
```

#### 3.2 Event — 在 `GlobalCommissionPauseToggled` 后追加

```rust
/// 推荐人免除阈值已更新
ReferrerExemptThresholdChanged { old_threshold: u16, new_threshold: u16 },
```

#### 3.3 Extrinsic — 新增 call_index(33)

```rust
/// [Root] 设置推荐人免除阈值
///
/// 多级分销配置层数超过此阈值的 Entity，推荐人不获得招商提成。
/// threshold = 0 表示不启用此规则。
#[pallet::call_index(33)]
#[pallet::weight(T::WeightInfo::set_commission_rate())]
pub fn set_referrer_exempt_threshold(
    origin: OriginFor<T>,
    threshold: u16,
) -> DispatchResult {
    ensure_root(origin)?;
    let old = ReferrerExemptThreshold::<T>::get();
    ReferrerExemptThreshold::<T>::put(threshold);
    Self::deposit_event(Event::ReferrerExemptThresholdChanged {
        old_threshold: old,
        new_threshold: threshold,
    });
    Ok(())
}
```

---

### Step 4: engine.rs 引擎改动（NEX + Token 管线）

**文件**: `commission/core/src/engine.rs`

#### 4.1 NEX 管线 — process_commission 函数

**修改位置**: 当前 line 77-80（`let global_referrer_bps = ...` 区域）

将：

```rust
let global_referrer_bps = T::ReferrerShareBps::get();
let has_referrer = global_referrer_bps > 0
    && T::EntityReferrerProvider::entity_referrer(entity_id).is_some();
```

替换为：

```rust
let global_referrer_bps = T::ReferrerShareBps::get();
let exempt_threshold = ReferrerExemptThreshold::<T>::get();
let is_exempt = exempt_threshold > 0
    && T::MultiLevelQuery::tier_count(entity_id) > exempt_threshold;
let has_referrer = !is_exempt
    && global_referrer_bps > 0
    && T::EntityReferrerProvider::entity_referrer(entity_id).is_some();
```

逻辑说明：
- `exempt_threshold == 0` → 规则未启用 → 短路，不查 tier_count → 无性能影响
- `exempt_threshold > 0` → 查一次 `tier_count`（1 次 StorageMap::get）
- `is_exempt == true` → `has_referrer = false` → referrer_quota = 0 → 平台费 100% 归国库

#### 4.2 Token 管线 — process_token_commission 函数

**修改位置**: 当前 line 458-459（`let referrer_share_bps = ...` 区域）

将：

```rust
let referrer_share_bps = T::ReferrerShareBps::get();
if referrer_share_bps > 0 && !token_platform_fee.is_zero() {
    if let Some(referrer) = T::EntityReferrerProvider::entity_referrer(entity_id) {
```

替换为：

```rust
let referrer_share_bps = T::ReferrerShareBps::get();
let exempt_threshold = ReferrerExemptThreshold::<T>::get();
let token_is_exempt = exempt_threshold > 0
    && T::MultiLevelQuery::tier_count(entity_id) > exempt_threshold;
if !token_is_exempt && referrer_share_bps > 0 && !token_platform_fee.is_zero() {
    if let Some(referrer) = T::EntityReferrerProvider::entity_referrer(entity_id) {
```

---

### Step 5: CommissionGovernancePort 新增方法

**文件**: `pallets/entity/common/src/traits/governance_ports.rs`

在 `CommissionGovernancePort` trait 中追加：

```rust
/// 设置推荐人免除阈值（全局）
fn governance_set_referrer_exempt_threshold(threshold: u16) -> Result<(), DispatchError> {
    let _ = threshold;
    Err(DispatchError::Other("not implemented"))
}
```

空实现 `()` 中追加：

```rust
fn governance_set_referrer_exempt_threshold(_: u16) -> Result<(), DispatchError> {
    Err(DispatchError::Other("not implemented"))
}
```

commission-core 中 `impl CommissionGovernancePort` 追加实现：

```rust
fn governance_set_referrer_exempt_threshold(threshold: u16) -> Result<(), DispatchError> {
    let old = pallet::ReferrerExemptThreshold::<T>::get();
    pallet::ReferrerExemptThreshold::<T>::put(threshold);
    Pallet::<T>::deposit_event(pallet::Event::ReferrerExemptThresholdChanged {
        old_threshold: old,
        new_threshold: threshold,
    });
    Ok(())
}
```

---

### Step 6: Governance ProposalType 新增变体

**文件**: `pallets/entity/governance/src/lib.rs`

#### 6.1 ProposalType 枚举 — 在 `MultiLevelPause` / `MultiLevelResume` 后追加

```rust
/// 推荐人免除阈值变更（全局治理）
ReferrerExemptThresholdChange { threshold: u16 },
```

#### 6.2 domain() 归属 — Commission 域

在 `ProposalDomain::Commission` 匹配分支中追加：

```rust
| Self::ReferrerExemptThresholdChange { .. }
```

#### 6.3 validate — 参数校验

```rust
ProposalType::ReferrerExemptThresholdChange { threshold } => {
    // threshold = 0 合法（表示禁用规则）
    ensure!(*threshold <= 1000, Error::<T>::InvalidParameter);
},
```

#### 6.4 execute — 执行

```rust
ProposalType::ReferrerExemptThresholdChange { threshold } => {
    T::CommissionGovernance::governance_set_referrer_exempt_threshold(*threshold)
},
```

注意：这是一个**全局参数**（不按 entity_id），但治理提案本身是 per-entity 的。这里有两种处理方式：
- **方案 A**: 治理执行时忽略 entity_id，直接设全局值（任何 Entity 的治理都能发起）
- **方案 B**: 仅允许 Root 级治理或特定"平台治理 Entity"发起

推荐方案 A + 事件日志（记录哪个 entity_id 发起的变更），简单且可审计。

---

### Step 7: Mock + 测试

**文件**: `commission/core/src/mock.rs`

`type MultiLevelQuery = ();` 已有，空实现 `tier_count` 返回 0（默认方法）。

如需测试免除逻辑，新增 MockMultiLevelQuery 替换：

```rust
thread_local! {
    static MOCK_TIER_COUNT: RefCell<BTreeMap<u64, u16>> = RefCell::new(BTreeMap::new());
}

pub struct MockMultiLevelQuery;

impl MultiLevelQueryProvider<AccountId> for MockMultiLevelQuery {
    fn activation_progress(_: u64, _: &AccountId) -> Vec<MultiLevelActivationInfo> { Vec::new() }
    fn is_paused(_: u64) -> bool { false }
    fn member_stats(_: u64, _: &AccountId) -> Option<MultiLevelMemberStats> { None }
    fn tier_count(entity_id: u64) -> u16 {
        MOCK_TIER_COUNT.with(|m| m.borrow().get(&entity_id).copied().unwrap_or(0))
    }
}

pub fn set_mock_tier_count(entity_id: u64, count: u16) {
    MOCK_TIER_COUNT.with(|m| m.borrow_mut().insert(entity_id, count));
}
```

Config 中替换: `type MultiLevelQuery = MockMultiLevelQuery;`

**文件**: `commission/core/src/tests.rs`

新增测试用例（约 50 行）：

```
#[test] referrer_exempt_threshold_zero_does_not_affect()
  — threshold=0, entity 有 8 层多级 → 推荐人正常拿提成

#[test] referrer_exempt_when_tier_count_exceeds_threshold()
  — threshold=5, entity 有 8 层 → 推荐人无提成，平台费 100% 归国库

#[test] referrer_not_exempt_when_tier_count_at_threshold()
  — threshold=5, entity 有 5 层 → 推荐人正常拿提成（>5 才免除，=5 不免除）

#[test] referrer_not_exempt_when_no_multi_level_config()
  — threshold=5, entity 无多级配置(tier_count=0) → 推荐人正常拿提成

#[test] referrer_exempt_token_pipeline()
  — threshold=5, entity 有 8 层 → Token 管线推荐人也无提成

#[test] set_referrer_exempt_threshold_root_only()
  — 非 Root 调用 → 失败
  — Root 调用 → 成功，事件正确

#[test] referrer_exempt_dynamically_changes_with_tier_count()
  — threshold=5, entity 初始 3 层 → 推荐人正常拿
  — entity 改为 8 层 → 下一笔订单推荐人无提成
  — entity 改回 4 层 → 推荐人又恢复
```

---

## 执行顺序

```
Step 1 → Step 2 → Step 3 → Step 4 → Step 5 → Step 6 → Step 7
  │         │         │         │         │         │        │
  │         │         │         │         │         │        └ mock + tests
  │         │         │         │         │         └ governance 提案
  │         │         │         │         └ CommissionGovernancePort 扩展
  │         │         │         └ engine 引擎改动（核心）
  │         │         └ commission-core 存储/事件/extrinsic
  │         └ multi-level 实现 tier_count
  └ common trait 新增 tier_count
```

依赖关系：Step 1 → Step 2（实现依赖 trait）；Step 1 → Step 4（引擎调用 trait）；Step 3 → Step 4（引擎读取 Storage）；Step 5 → Step 6（治理执行依赖 Port）。

Step 1+2 可并行准备，Step 3+5 可并行准备，Step 7 在 Step 1-4 全部完成后执行。

---

## 不涉及 / 无需改动的部分

| 组件 | 原因 |
|------|------|
| cancel_commission | 退的是实际记录的金额，不受费率/阈值变化影响 |
| withdraw_commission | 提现逻辑与推荐人无关 |
| 其他佣金插件 (referral/single-line/team/level-diff/pool-reward) | 纯 Pool B 逻辑，不涉及 Pool A |
| order 模块 | 不涉及 |
| registry (EntityReferrer 存储) | 推荐人绑定关系不变，只是不发钱 |
| runtime/src/configs/mod.rs | 初始阶段无需改 runtime 配置（StorageValue 默认 5） |
| 存储迁移 | StorageValue 新增，ValueQuery 默认 5，无需迁移 |

---

## 事件流示例

### 治理设置阈值

```
Root → set_referrer_exempt_threshold(5)
  → Storage: ReferrerExemptThreshold = 5
  → Event: ReferrerExemptThresholdChanged { old: 0, new: 5 }
```

### 订单触发 — Entity 有 8 层多级分销

```
process_commission(entity_id=100, ...)
  → exempt_threshold = 5
  → tier_count(100) = 8
  → 8 > 5 → is_exempt = true
  → has_referrer = false
  → referrer_quota = 0
  → treasury_portion = platform_fee (全额)
  → Event: PlatformFeeToTreasury { order_id, amount: platform_fee }
  (无 CommissionDistributed EntityReferral 事件)
```

### 订单触发 — Entity 有 3 层多级分销

```
process_commission(entity_id=200, ...)
  → exempt_threshold = 5
  → tier_count(200) = 3
  → 3 ≤ 5 → is_exempt = false
  → has_referrer = true (假设有推荐人)
  → referrer_quota = platform_fee × 50%
  → treasury_portion = platform_fee × 50%
  → Event: PlatformFeeToTreasury { order_id, amount: 50% }
  → Event: CommissionDistributed { ..., EntityReferral, amount: 50% }
```
