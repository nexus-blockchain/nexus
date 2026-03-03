# pallet-commission-multi-level

多级分销返佣插件 — N 层推荐链 + 三维激活条件 + 总佣金上限。

作为 `pallet-commission-core` 的 `CommissionPlugin` 插件运行，支持 NEX / EntityToken 双轨佣金。

---

## 一、架构定位

```
pallet-commission-core (调度中心)
  ├── MultiLevelPlugin      ──→ 本模块 (NEX)
  ├── TokenMultiLevelPlugin ──→ 本模块 (Token)
  ├── ReferralPlugin        ──→ pallet-commission-referral
  ├── LevelDiffPlugin       ──→ pallet-commission-level-diff
  └── SingleLinePlugin      ──→ pallet-commission-single-line
```

触发条件：`enabled_modes` 含 `MULTI_LEVEL` 标志位 且 Entity 存在 `MultiLevelConfigs`。

---

## 二、数据结构

### MultiLevelTier

```rust
pub struct MultiLevelTier {
    pub rate: u16,              // 佣金比率（基点制，10000 = 100%），0 = 跳过
    pub required_directs: u32,  // 最低直推人数，0 = 无要求
    pub required_team_size: u32,// 最低团队规模，0 = 无要求
    pub required_spent: u128,   // 最低累计消费 USDT（精度 10^6），0 = 无要求
}
```

### MultiLevelConfig

```rust
pub struct MultiLevelConfig<MaxLevels: Get<u32>> {
    pub levels: BoundedVec<MultiLevelTier, MaxLevels>,  // 各层配置，索引 0 = L1
    pub max_total_rate: u16,                            // 佣金总和上限（基点制，默认 1500 = 15%）
}
```

---

## 三、激活条件

`check_tier_activation` 对推荐人执行三维 **AND** 检查，值为 0 的条件自动跳过：

| 条件 | 数据来源 | 精度 |
|------|----------|------|
| `required_directs` | `MemberProvider::get_member_stats().0` | 人数 |
| `required_team_size` | `MemberProvider::get_member_stats().1` | 人数 |
| `required_spent` | `MemberProvider::get_member_spent_usdt()` | USDT × 10^6 |

**不满足条件时：** 跳过该层推荐人，**遍历继续向上**。被跳过的佣金留在 `remaining` 返还 core，不重分配。

---

## 四、核心算法 `process_multi_level`

逐层遍历推荐链，每层执行：

1. **rate = 0** → 跳过（占位层），向上移动 referrer
2. **无推荐人** → 终止
3. **循环检测**（`BTreeSet<AccountId>`）→ 命中则终止
4. **激活条件不满足** → 跳过该层，继续下一层
5. **计算佣金** `commission = order_amount × rate / 10000`，取 `min(commission, remaining)`
6. **总额上限检查** — 累计超过 `max_total_rate` 时截断最后一笔并终止

### 终止条件汇总

| 情况 | 行为 |
|------|------|
| `rate == 0` / 激活条件不满足 | 跳过，继续 |
| 无推荐人 / 循环检测 / remaining = 0 / 超总额上限 | 终止 |

---

## 五、配置示例

### 3 层递减（典型电商）

```
L1: rate=1000 (10%), directs=0               ← 无门槛
L2: rate=500  (5%),  directs=3               ← 需 ≥3 直推
L3: rate=200  (2%),  directs=5               ← 需 ≥5 直推
max_total_rate = 2000 (20%)
```

买家下单 10,000 NEX（Alice → Bob → Carol → Dave）：

| 层级 | 推荐人 | 满足？ | 佣金 | 累计 |
|------|--------|--------|------|------|
| L1 | Bob (5推) | ✅ | 1,000 | 1,000 |
| L2 | Carol (4推) | ✅ | 500 | 1,500 |
| L3 | Dave (6推) | ✅ | 200 | 1,700 |

总佣金 1,700 (17%)，remaining 8,300 返还 core。

若 Carol 仅 1 直推（不合格）→ 跳过，Dave 仍获 L3 佣金，总佣金 1,200 (12%)。

### 5 层 + max_total_rate 截断

```
L1=800(8%) L2=500(5%) L3=300(3%) L4=200(2%) L5=100(1%)
max_total_rate = 1500 (15%)
```

全部合格时：L1=800, L2=500, L3=**200**（截断，原 300 会超 1500），L4/L5 不执行。总佣金精确 1,500。

### 配置建议

推荐 **L1 > L2 > L3 递减**。若 L2 > L1（二级高于直推），属裂变激励特征，需注意合规风险。

---

## 六、Pallet API

### Config

| 关联类型 | 说明 |
|----------|------|
| `MemberProvider` | 推荐链 + 统计 + USDT 消费数据 |
| `MaxMultiLevels` | 最大层级数（`Get<u32>`，默认 15） |
| `WeightInfo` | 权重（`weights.rs`） |

### Storage

| 名称 | 类型 | 说明 |
|------|------|------|
| `MultiLevelConfigs` | `StorageMap<u64, MultiLevelConfigOf<T>>` | Entity → 多级分销配置 |

### Extrinsic

| idx | 名称 | Origin | 说明 |
|-----|------|--------|------|
| 0 | `set_multi_level_config` | Root | 设置 Entity 多级分销配置 |

校验：levels 非空，每层 `rate ≤ 10000`，`0 < max_total_rate ≤ 10000`。

### Events / Errors

| 事件 | 说明 |
|------|------|
| `MultiLevelConfigUpdated { entity_id }` | 配置已更新（extrinsic + PlanWriter 均发出） |
| `MultiLevelConfigCleared { entity_id }` | 配置已清除（PlanWriter 路径） |

| 错误 | 说明 |
|------|------|
| `InvalidRate` | rate 超过 10000 或 max_total_rate 为 0 |
| `EmptyLevels` | levels 为空 |

---

## 七、Trait 实现

- **CommissionPlugin / TokenCommissionPlugin** — 供 core 调用，共用 `process_multi_level` 泛型逻辑（仅 Balance 类型不同）
- **MultiLevelPlanWriter** — 治理路径，支持 `set_multi_level` / `clear_multi_level_config`
  - PlanWriter 校验 rate / max_total_rate / 层数上限 / 非空
  - 两个方法均 emit 事件（R2 审计修复）
  - **限制：** PlanWriter 创建的 tiers 激活条件全为 0，需通过 Root extrinsic 配置完整条件

---

## 八、边界安全

| 情况 | 处理 |
|------|------|
| 空 levels | 直接返回 |
| 链短于配置层数 | break，已分佣保留 |
| 全部不合格 | 佣金 = 0，remaining 不变 |
| 环形推荐链 | BTreeSet visited 检测 → break |
| level_idx > 255 | `.min(255) as u8` |
| 单层佣金 > remaining | `min(commission, remaining)` |
| 累计超 max_total_rate | 截断最后一笔 |
| NEX / Token 隔离 | 泛型参数 `B`，独立调用 |

---

## 九、测试覆盖（31 个）

- **Extrinsic (4):** 设置成功、rate 超限拒绝、非 Root 拒绝
- **佣金计算 (8):** 基础 3 层、总额截断、激活条件跳过、循环检测、标志未启用、无配置、团队规模、三条件组合
- **激活条件回归 (3):** USDT vs NEX Balance、USDT 充足通过、单条件不满足
- **PlanWriter (5):** 创建、rate 校验、层数上限、清除
- **Round 2 回归 (6):** PlanWriter 事件发出、清除事件、空 levels 拒绝、零 max_total_rate 拒绝（extrinsic + PlanWriter 各一）
- **Round 4 回归 (2):** is_activated 跳过停用会员链继续、全部激活不影响
- **Round 5 回归 (3):** rate=0 占位层跳过推荐人、链短于配置层数提前终止、TokenCommissionPlugin 路径验证

---

## 十、审计修复

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| H1 | High | `required_spent` 误用 NEX Balance（10^12）对比 — 改用 `get_member_spent_usdt()`（10^6） | ✅ |
| H2 | High | PlanWriter 缺 rate 校验 | ✅ |
| H3 | High | PlanWriter 超层数时静默清空 — 改返回 `TooManyLevels` | ✅ |
| M1 | Medium | 硬编码 Weight → WeightInfo trait | ✅ |
| M2 | Medium | 激活条件零测试 → +5 回归测试 | ✅ |
| L1 | Low | try-runtime feature 缺 sp-runtime | ✅ |
| M1-R2 | Medium | PlanWriter `set_multi_level` 不 emit 事件 — 添加 `deposit_event` | ✅ |
| M2-R2 | Medium | PlanWriter `clear_multi_level_config` 无事件 — 新增 `MultiLevelConfigCleared` + emit | ✅ |
| L1-R2 | Low | `set_multi_level_config` 接受空 levels — 添加 `EmptyLevels` 校验 | ✅ |
| L2-R2 | Low | `max_total_rate = 0` 静默禁用佣金 — 添加 `> 0` 校验 | ✅ |
| L1-R3 | Low | `check_tier_activation` 仅 `required_spent` 非零时仍调用 `get_member_stats`（多余 DB read）— 改为懒加载 | ✅ |
| L2-R3 | Low | Extrinsic 文档注释未反映 R2 新增校验（EmptyLevels / max_total_rate > 0）| ✅ |
| H1-R4 | High | `process_multi_level` 缺 `is_activated` 检查 — 停用会员仍获佣金（与 team H2 同类）| ✅ |
| M1-R4 | Medium | Cargo.toml 缺 `sp-runtime/runtime-benchmarks` feature 传播 | ✅ |
| L1-R5 | Low | 死 dev-dependency `pallet-balances` — mock/tests 从未引用 | ✅ |
| L2-R5 | Low | `rate=0` 占位层代码路径无测试覆盖 | ✅ |
| L3-R5 | Low | 推荐链短于配置层数（提前 break）无测试覆盖 | ✅ |
| L4-R5 | Low | `TokenCommissionPlugin` 路径无测试覆盖 | ✅ |

---

## 依赖

```
pallet-commission-multi-level
├── pallet-commission-common  (CommissionPlugin, MemberProvider, MultiLevelPlanWriter)
├── frame-support / frame-system / sp-runtime / sp-std
```

## License

MIT
