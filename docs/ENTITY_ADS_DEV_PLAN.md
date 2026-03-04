# Entity DApp 广告投放 — 开发方案

## 现状分析

### 已有能力 ✅

| 模块 | 能力 |
|------|------|
| `pallet-ads-core` | Campaign CRUD、Escrow 托管、收据提交、Era 结算、双向黑白名单、Slash、私域广告 |
| `pallet-ads-entity` | Entity/Shop 广告位注册、展示量验证+上限、二方分成、Entity 分成治理 |
| `pallet-ads-router` | 根据 PlacementId 路由到 Entity/GroupRobot 适配层 |
| `pallet-ads-primitives` | `AdPolicyProvider`/`PlacementConfigProvider` trait 已定义但未接入 |

### 关键缺口分析

| # | 缺口 | 影响 | 严重度 |
|---|------|------|--------|
| G1 | **Campaign 无投放定向** — 广告主创建 Campaign 不指定目标广告位，任何 Entity admin 都可为任何 Approved Campaign 提交收据 | 广告主无法控制广告展示位置，预算可能被非目标广告位消耗 | **Critical** |
| G2 | **广告位级审核缺失** — 仅 Root/DAO 可审核 Campaign，Entity Owner 无权决定自己广告位上展示哪些广告 | Entity Owner 失去广告内容控制权，不符合 DApp 运营需求 | **High** |
| G3 | **cpm_multiplier_bps 可操纵** — 由 Entity admin（收据提交者）自行填写，可随意设置高倍率 | Entity 侧可通过高 multiplier 过度消耗广告主预算 | **High** |
| G4 | **广告发现机制缺失** — 无链上索引供 Entity DApp 查询可用 Campaign | DApp 前端无法自动匹配和展示广告 | **Medium** |
| G5 | **投放确认单方面** — 仅 Entity 侧提交收据，广告主无法验证真实展示量 | 存在虚报展示量的风险 | **Medium** |
| G6 | **竞价排序缺失** — 多个 Campaign 竞争同一广告位时无优先级机制 | 高出价广告主无法获得优先展示 | **Low** |

---

## 开发方案 — 5 个 Phase

### Phase 1: Campaign 投放定向 (解决 G1)

**目标:** 广告主指定 Campaign 投放的目标广告位，非目标广告位不可提交收据。

**改动范围:** `pallet-ads-core`

#### 1.1 新增存储

```rust
/// Campaign 目标广告位列表 (空 = 全网投放, 由黑白名单过滤)
#[pallet::storage]
pub type CampaignTargets<T: Config> = StorageMap<
    _, Blake2_128Concat, u64, // campaign_id
    BoundedVec<PlacementId, T::MaxTargetsPerCampaign>,
>;
```

#### 1.2 修改 `create_campaign`

新增可选参数 `targets: Option<BoundedVec<PlacementId, T::MaxTargetsPerCampaign>>`：
- `Some(list)` → 定向投放，仅 list 中的广告位可接收
- `None` → 开放投放（保持现有行为，由黑白名单过滤）

#### 1.3 修改 `submit_delivery_receipt`

在双向黑名单检查之后、`DeliveryVerifier` 调用之前，增加定向检查：

```rust
// 定向投放: Campaign 指定了目标广告位时，仅目标可投放
if let Some(targets) = CampaignTargets::<T>::get(campaign_id) {
    ensure!(targets.contains(&placement_id), Error::<T>::PlacementNotTargeted);
}
```

#### 1.4 新增 Extrinsics

```rust
/// 广告主更新 Campaign 目标广告位
pub fn set_campaign_targets(
    origin, campaign_id, targets: BoundedVec<PlacementId, T::MaxTargetsPerCampaign>
) -> DispatchResult;

/// 广告主清除定向（恢复全网投放）
pub fn clear_campaign_targets(origin, campaign_id) -> DispatchResult;
```

#### 1.5 Config 常量

```rust
#[pallet::constant]
type MaxTargetsPerCampaign: Get<u32>; // 建议默认 20
```

**预估工作量:** ~150 行代码 + ~100 行测试

---

### Phase 2: 广告位级审核 (解决 G2)

**目标:** Entity Owner/Admin 可以审核投向自己广告位的 Campaign，形成"双层审核"机制。

**改动范围:** `pallet-ads-entity` (主要) + `pallet-ads-core` (少量)

#### 2.1 审核模型

```
广告主 create_campaign → review_status = Pending

全局审核 (Root/DAO):
  review_campaign(approved=true) → review_status = Approved

广告位级审核 (Entity Owner):
  approve_campaign_for_placement(campaign_id, placement_id) → 写入 PlacementApprovals
  reject_campaign_for_placement(campaign_id, placement_id) → 写入 PlacementRejections

投放时双重检查:
  campaign.review_status == Approved  (全局通过)
  AND PlacementApprovals 包含 (campaign_id, placement_id)  (广告位通过)
  OR  广告位未启用审核 (auto_approve = true)
```

#### 2.2 新增存储 (pallet-ads-entity)

```rust
/// 广告位是否要求投放前审核 (默认 false = auto-approve)
#[pallet::storage]
pub type PlacementRequiresReview<T: Config> =
    StorageMap<_, Blake2_128Concat, PlacementId, bool, ValueQuery>;

/// 广告位已批准的 Campaign 列表
#[pallet::storage]
pub type PlacementApprovals<T: Config> = StorageDoubleMap<
    _, Blake2_128Concat, PlacementId,
    Blake2_128Concat, u64,  // campaign_id
    bool, ValueQuery,
>;
```

#### 2.3 新增 Extrinsics (pallet-ads-entity)

```rust
/// Entity Owner 开启/关闭广告位级审核
pub fn set_placement_review_required(
    origin, placement_id, required: bool
) -> DispatchResult;

/// Entity Owner 批准 Campaign 在指定广告位投放
pub fn approve_campaign_for_placement(
    origin, placement_id, campaign_id
) -> DispatchResult;

/// Entity Owner 撤销批准
pub fn revoke_campaign_approval(
    origin, placement_id, campaign_id
) -> DispatchResult;
```

#### 2.4 修改 DeliveryVerifier (pallet-ads-entity)

在 `verify_and_cap_audience` 中增加审核检查：

```rust
// 广告位级审核 (如果启用)
if PlacementRequiresReview::<T>::get(placement_id) {
    // 需要从 caller context 获取 campaign_id
    // 方案: 扩展 DeliveryVerifier trait 签名，或使用 thread-local 传递
}
```

**trait 签名变更方案:**

方案 A (推荐): 扩展 `DeliveryVerifier` trait，增加 `campaign_id` 参数：

```rust
fn verify_and_cap_audience(
    who: &AccountId,
    placement_id: &PlacementId,
    campaign_id: u64,        // ← 新增
    audience_size: u32,
    node_id: Option<[u8; 32]>,
) -> Result<u32, DispatchError>;
```

这需要同步修改 `pallet-ads-grouprobot` 的实现和 `pallet-ads-router`。

方案 B: 在 `pallet-ads-core::submit_delivery_receipt` 中直接调用 Entity 审核检查（通过新 trait `PlacementReviewProvider`）。

**预估工作量:** ~250 行代码 + ~150 行测试

---

### Phase 3: CPM Multiplier 治理化 (解决 G3)

**目标:** 移除提交者自由填写 multiplier 的能力，改为链上配置。

**改动范围:** `pallet-ads-core` + `pallet-ads-entity`

#### 3.1 方案

将 `cpm_multiplier_bps` 从 `submit_delivery_receipt` 参数中移除，改为链上查询：

```rust
// 优先级: Campaign 指定 > 广告位配置 > 全局默认 (100 = 1x)
let multiplier = CampaignMultiplier::<T>::get(campaign_id)
    .or_else(|| PlacementMultiplier::<T>::get(&placement_id))
    .unwrap_or(100u32);  // 默认 1x
```

#### 3.2 新增存储

```rust
// pallet-ads-core
/// Campaign 级 CPM 倍率 (广告主设置, 基点 100 = 1x)
pub type CampaignMultiplier<T: Config> =
    StorageMap<_, Blake2_128Concat, u64, u32>;

// pallet-ads-entity
/// 广告位级 CPM 倍率 (Entity Owner 设置, 基点 100 = 1x, 如高流量时段溢价)
pub type PlacementMultiplier<T: Config> =
    StorageMap<_, Blake2_128Concat, PlacementId, u32>;
```

#### 3.3 Extrinsics

```rust
// pallet-ads-core
pub fn set_campaign_multiplier(origin, campaign_id, multiplier_bps: u32) -> DispatchResult;

// pallet-ads-entity
pub fn set_placement_multiplier(origin, placement_id, multiplier_bps: u32) -> DispatchResult;
```

#### 3.4 Breaking Change

`submit_delivery_receipt` 签名变更：移除 `cpm_multiplier_bps` 参数。

**预估工作量:** ~120 行代码 + ~80 行测试

---

### Phase 4: 广告发现索引 (解决 G4)

**目标:** 提供链上索引和 Runtime API，让 Entity DApp 前端能查询匹配的 Campaign。

**改动范围:** `pallet-ads-core` (索引) + `runtime` (API)

#### 4.1 新增存储索引

```rust
/// 按状态索引: Active + Approved 的 Campaign 列表
/// 结算/取消/过期时自动移除
#[pallet::storage]
pub type ActiveApprovedCampaigns<T: Config> = StorageValue<
    _, BoundedVec<u64, ConstU32<10000>>, ValueQuery,
>;

/// 按投放类型索引: delivery_type → Vec<campaign_id>
#[pallet::storage]
pub type CampaignsByDeliveryType<T: Config> = StorageMap<
    _, Blake2_128Concat, u8, BoundedVec<u64, ConstU32<5000>>, ValueQuery,
>;

/// Campaign 定向到的广告位的反向索引: placement_id → Vec<campaign_id>
#[pallet::storage]
pub type CampaignsForPlacement<T: Config> = StorageMap<
    _, Blake2_128Concat, PlacementId,
    BoundedVec<u64, ConstU32<1000>>, ValueQuery,
>;
```

#### 4.2 Runtime API

```rust
/// 查询指定广告位可投放的 Campaign 列表
fn available_campaigns_for_placement(
    placement_id: PlacementId,
    max_results: u32,
) -> Vec<CampaignSummary>;

/// 查询 Campaign 详情 (前端展示用)
fn campaign_details(campaign_id: u64) -> Option<CampaignDetail>;
```

#### 4.3 维护逻辑

在以下 extrinsic 中维护索引：
- `review_campaign` (Approved 时加入 ActiveApprovedCampaigns)
- `cancel_campaign` / `expire_campaign` / `force_cancel_campaign` (移除)
- `set_campaign_targets` (维护 CampaignsForPlacement 反向索引)

**预估工作量:** ~300 行代码 + ~120 行测试

---

### Phase 5: 投放确认与争议 (解决 G5)

**目标:** 广告主可对投放收据发起争议，引入确认窗口。

**改动范围:** `pallet-ads-core` + 可选集成 `pallet-arbitration`

#### 5.1 方案: 收据确认窗口

```
Entity 提交收据 → 状态: Pending
                   ↓ (确认窗口内)
广告主确认   → 状态: Confirmed → 正常结算
广告主争议   → 状态: Disputed  → 进入争议流程
超时未响应   → 状态: AutoConfirmed → 正常结算
```

#### 5.2 新增存储

```rust
/// 收据确认状态 (campaign_id, placement_id, receipt_index) → status
pub type ReceiptConfirmation<T: Config> = StorageNMap<
    _, (
        NMapKey<Blake2_128Concat, u64>,        // campaign_id
        NMapKey<Blake2_128Concat, PlacementId>,
        NMapKey<Blake2_128Concat, u32>,        // receipt_index
    ),
    ReceiptStatus,
>;

pub enum ReceiptStatus {
    Pending,
    Confirmed,
    Disputed,
    AutoConfirmed,
}
```

#### 5.3 确认窗口

```rust
#[pallet::constant]
type ReceiptConfirmationWindow: Get<BlockNumberFor<Self>>; // 建议 7200 blocks ≈ 12h
```

#### 5.4 Extrinsics

```rust
/// 广告主确认收据
pub fn confirm_receipt(origin, campaign_id, placement_id, receipt_index) -> DispatchResult;

/// 广告主争议收据
pub fn dispute_receipt(origin, campaign_id, placement_id, receipt_index) -> DispatchResult;
```

#### 5.5 结算修改

`settle_era_ads` 跳过 `Disputed` 状态的收据，仅结算 `Confirmed` 和 `AutoConfirmed`。

**预估工作量:** ~400 行代码 + ~200 行测试

---

## 实施优先级与依赖关系

```
Phase 1 (Campaign 定向)   ←─── 无依赖，可独立开发
    ↓
Phase 2 (广告位审核)      ←─── 依赖 Phase 1 (审核针对定向的 Campaign)
    ↓
Phase 3 (Multiplier 治理) ←─── 无依赖，可与 Phase 1 并行
    ↓
Phase 4 (广告发现)        ←─── 依赖 Phase 1 (需要定向索引)
    ↓
Phase 5 (投放确认)        ←─── 依赖 Phase 1-3 完成后
```

## 推荐开发顺序

| 顺序 | Phase | 理由 |
|------|-------|------|
| 第 1 批 | Phase 1 + Phase 3 (并行) | 解决最关键的安全和控制问题 |
| 第 2 批 | Phase 2 | 建立 Entity Owner 的广告内容控制权 |
| 第 3 批 | Phase 4 | 支撑前端集成 |
| 第 4 批 | Phase 5 | 完善信任机制 (可延后) |

## 工作量估算

| Phase | 代码 | 测试 | 总计 | 工期 (天) |
|-------|------|------|------|----------|
| Phase 1 | ~150 行 | ~100 行 | ~250 行 | 1 |
| Phase 2 | ~250 行 | ~150 行 | ~400 行 | 1.5 |
| Phase 3 | ~120 行 | ~80 行 | ~200 行 | 0.5 |
| Phase 4 | ~300 行 | ~120 行 | ~420 行 | 1.5 |
| Phase 5 | ~400 行 | ~200 行 | ~600 行 | 2 |
| **合计** | **~1220 行** | **~650 行** | **~1870 行** | **~6.5** |

## 改动文件清单

| 文件 | Phase 1 | Phase 2 | Phase 3 | Phase 4 | Phase 5 |
|------|---------|---------|---------|---------|---------|
| `pallets/ads/primitives/src/lib.rs` | | ✏️ trait 扩展 | | ✏️ CampaignSummary | |
| `pallets/ads/core/src/lib.rs` | ✏️ 定向存储+检查 | | ✏️ multiplier 重构 | ✏️ 索引维护 | ✏️ 确认窗口 |
| `pallets/ads/core/src/tests.rs` | ✏️ | | ✏️ | ✏️ | ✏️ |
| `pallets/ads/entity/src/lib.rs` | | ✏️ 审核存储+extrinsic | ✏️ multiplier 配置 | | |
| `pallets/ads/entity/src/tests.rs` | | ✏️ | ✏️ | | |
| `pallets/ads/grouprobot/src/lib.rs` | | ✏️ trait 签名同步 | | | |
| `pallets/ads/router/src/lib.rs` | | ✏️ trait 签名同步 | | | |
| `runtime/src/configs/mod.rs` | ✏️ 常量配置 | ✏️ | | ✏️ Runtime API | ✏️ |

## 风险与注意事项

1. **Phase 2 trait 签名变更** — `DeliveryVerifier::verify_and_cap_audience` 增加 `campaign_id` 参数会影响 grouprobot 和 router，需同步修改并确保向后兼容
2. **Phase 3 Breaking Change** — 移除 `submit_delivery_receipt` 的 `cpm_multiplier_bps` 参数是链上升级 breaking change，需要 Runtime Migration
3. **Phase 4 索引维护** — `ActiveApprovedCampaigns` 等索引需要在所有状态变更路径上正确维护，否则索引与实际状态不一致
4. **Phase 5 结算复杂度** — 确认窗口引入异步性，`settle_era_ads` 需要处理部分收据尚在确认窗口中的情况
5. **Storage Migration** — Phase 3 的签名变更和 Phase 5 的 Receipt 结构变更可能需要存储迁移
