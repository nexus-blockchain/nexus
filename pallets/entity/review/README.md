# pallet-entity-review

> ⭐ Entity 订单评价模块 — 订单完成后提交评分与评价

## 概述

`pallet-entity-review` 是 Entity 商城系统的评价管理模块，负责订单完成后的评分提交和店铺评分更新。

- **Runtime pallet_index:** 123
- **版本:** 0.5.0

### 核心功能

- **订单评价** — 买家在订单完成后提交 1-5 星评分
- **评价内容** — 支持通过 IPFS CID 关联评价详情（可选）
- **店铺评分联动** — 评价提交后通过 `ShopProvider::update_shop_rating` 更新店铺评分（best-effort，失败不回滚评价）
- **一单一评** — 每个订单仅允许一次评价，以 `order_id` 为 key 存储
- **用户评价索引** — 每个用户的评价 order_id 列表，支持“我的评价”查询
- **Entity 评价开关** — Entity owner/admin 可控制是否开启评价功能，关闭后该 Entity 下所有 Shop 的订单不可提交评价

## 数据结构

### MallReview — 评价记录

```rust
#[derive(Encode, Decode, codec::DecodeWithMemTracking, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct MallReview<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
    pub order_id: u64,                                  // 订单 ID
    pub reviewer: AccountId,                            // 评价者账户
    pub rating: u8,                                     // 评分（1-5）
    pub content_cid: Option<BoundedVec<u8, MaxCidLen>>, // 评价内容 IPFS CID（可选）
    pub created_at: BlockNumber,                        // 评价区块高度
}
```

类型别名：

```rust
pub type MallReviewOf<T> = MallReview<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::MaxCidLength,
>;
```

## Runtime 配置

```rust
// runtime/src/configs/mod.rs
impl pallet_entity_review::Config for Runtime {
    type EntityProvider = EntityRegistry;     // 实体查询 — pallet-entity-registry
    type OrderProvider = EntityTransaction;   // 订单查询 — pallet-entity-order
    type ShopProvider = EntityShop;           // 店铺评分更新 — pallet-entity-shop
    type MaxCidLength = ConstU32<64>;         // CID 最大长度 64 字节
    type MaxReviewsPerUser = ConstU32<500>;   // 每用户最大评价数
    type WeightInfo = pallet_entity_review::weights::SubstrateWeight<Runtime>;
}
```

> **注意：** `RuntimeEvent` 已从 Config trait 中移除（polkadot-sdk #7229 自动追加）。

### Config 关联类型

| 类型 | 约束 | 说明 |
|------|------|------|
| `EntityProvider` | `EntityProvider<AccountId>` | 实体查询接口（存在性、状态、admin 校验） |
| `OrderProvider` | `OrderProvider<AccountId, u128>` | 订单查询接口 |
| `ShopProvider` | `ShopProvider<AccountId>` | 店铺更新接口 |
| `MaxCidLength` | `Get<u32>` (常量) | CID 最大长度 |
| `MaxReviewsPerUser` | `Get<u32>` (常量) | 每用户最大评价数 |
| `WeightInfo` | `WeightInfo` | 权重信息（benchmark 集成） |

## Extrinsics

### set_review_enabled (call_index 1)

设置 Entity 评价开关。

```rust
#[pallet::call_index(1)]
#[pallet::weight(T::WeightInfo::set_review_enabled())]
pub fn set_review_enabled(
    origin: OriginFor<T>,
    entity_id: u64,
    enabled: bool,
) -> DispatchResult
```

**权限：** Entity owner 或 admin

**验证流程：**
1. Entity 存在 → `EntityNotFound`
2. Entity 已激活 → `EntityNotActive`
3. 调用者是 Entity admin → `NotEntityAdmin`

**执行逻辑：**
- `enabled=true` → 移除 `EntityReviewDisabled` key（开启评价）
- `enabled=false` → 写入 `EntityReviewDisabled` key（关闭评价）
- 发出 `ReviewConfigUpdated` 事件

### submit_review (call_index 0)

提交订单评价。

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::WeightInfo::submit_review())]
pub fn submit_review(
    origin: OriginFor<T>,
    order_id: u64,
    rating: u8,                    // 1-5 星
    content_cid: Option<Vec<u8>>,  // IPFS CID（可选）
) -> DispatchResult
```

**权限：** 仅订单买家（signed extrinsic）

**验证流程：**
1. `rating` 在 1-5 范围内 → `InvalidRating`
2. 获取订单买家并验证调用者身份 → `OrderNotFound` / `NotOrderBuyer`（`OrderProvider::order_buyer`）
3. 订单已完成 → `OrderNotCompleted`（`OrderProvider::is_order_completed`）
4. 该订单尚未评价 → `AlreadyReviewed`（`Reviews` 不含该 key）
5. CID 非空且长度校验 → `EmptyCid` / `CidTooLong`

**执行逻辑：**
1. 检查 Entity 评价开关：order → shop_id → entity_id → `EntityReviewDisabled` → `ReviewsDisabledForEntity`
2. 转换并校验 CID
3. 更新 `UserReviews` 索引（`try_push`，达上限返回 `UserReviewLimitReached`）
4. 构建 `MallReview` 记录（含当前区块号），写入 `Reviews` 存储（key = `order_id`）
5. `ReviewCount` 递增（`saturating_add`）
6. 通过 `OrderProvider::order_shop_id` 获取店铺 ID，调用 `ShopProvider::update_shop_rating` 更新评分（**best-effort**，失败发出 `ShopRatingUpdateFailed` 事件但不回滚评价）
7. 发出 `ReviewSubmitted` 事件（含 `shop_id`）

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Reviews` | `StorageMap<Blake2_128Concat, u64, MallReviewOf<T>>` | 订单 ID → 评价记录 |
| `ReviewCount` | `StorageValue<u64, ValueQuery>` | 全局评价总数（默认 0） |
| `ShopReviewCount` | `StorageMap<Blake2_128Concat, u64, u64, ValueQuery>` | 店铺 ID → 该店铺评价数量 |
| `UserReviews` | `StorageMap<Blake2_128Concat, AccountId, BoundedVec<u64, MaxReviewsPerUser>, ValueQuery>` | 用户 → 已评价 order_id 列表 |
| `EntityReviewDisabled` | `StorageMap<Blake2_128Concat, u64, (), OptionQuery>` | Entity 评价关闭标记（存在=关闭，不存在=开启） |

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `ReviewSubmitted` | `order_id: u64`, `reviewer: AccountId`, `shop_id: Option<u64>`, `rating: u8` | 评价已提交 |
| `ShopRatingUpdateFailed` | `order_id: u64`, `shop_id: u64` | 店铺评分更新失败（评价仍已记录） |
| `ReviewConfigUpdated` | `entity_id: u64`, `enabled: bool` | Entity 评价配置已更新 |

## Errors

| 错误 | 说明 |
|------|------|
| `OrderNotFound` | 订单不存在 |
| `NotOrderBuyer` | 调用者不是订单买家 |
| `OrderNotCompleted` | 订单尚未完成 |
| `AlreadyReviewed` | 该订单已评价 |
| `InvalidRating` | 评分不在 1-5 范围 |
| `CidTooLong` | CID 超过 `MaxCidLength` 限制 |
| `EmptyCid` | CID 为空（`Some(vec![])` 无意义） |
| `UserReviewLimitReached` | 用户评价数已达 `MaxReviewsPerUser` 上限 |
| `ReviewsDisabledForEntity` | 该 Entity 已关闭评价功能 |
| `NotEntityAdmin` | 调用者不是 Entity owner/admin |
| `EntityNotFound` | Entity 不存在 |
| `EntityNotActive` | Entity 未激活 |

## 依赖接口

### OrderProvider（来自 pallet-entity-common）

由 `pallet-entity-order` 实现。

```rust
pub trait OrderProvider<AccountId, Balance> {
    fn order_exists(order_id: u64) -> bool;
    fn order_buyer(order_id: u64) -> Option<AccountId>;
    fn order_shop_id(order_id: u64) -> Option<u64>;
    fn is_order_completed(order_id: u64) -> bool;
}
```

### ShopProvider（来自 pallet-entity-common）

由 `pallet-entity-shop` 实现，本模块使用 `update_shop_rating(shop_id, rating)` 方法。

## 文件结构

```
pallets/entity/review/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs      # 主模块（Config、Storage、Extrinsics、Events、Errors）
    ├── weights.rs  # 权重定义（WeightInfo trait + SubstrateWeight）
    ├── mock.rs     # 测试 mock runtime
    └── tests.rs    # 单元测试（41 tests）
```

## 审计记录 (v0.2.0)

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| Critical | C1 | `MallReview` 缺少 `DecodeWithMemTracking` | 添加 derive |
| Critical | C2 | `mock.rs` / `tests.rs` 不存在，零测试覆盖 | 创建 22 个测试 |
| Critical | C3 | `submit_review` weight `proof_size=0` | 修复为 `(35_000_000, 5_000)` |
| High | H1 | `update_shop_rating` 失败被 `let _ =` 静默忽略 | 改为传播错误 `?` |
| High | H2 | `order_exists` 冗余调用（`order_buyer` 已隐含检查） | 移除 |
| Medium | M1 | Cargo.toml 有未使用依赖（`log`/`sp-runtime`/`sp-std`） | 移除，dev-deps 加 `std` features |
| Medium | M2 | `ReviewSubmitted` 事件缺少 `shop_id` | 添加 `shop_id: Option<u64>` |
| Medium | M3 | Config 中 `RuntimeEvent` 已弃用 | 移除，使用 bound 语法 |
| Low | L1 | 无用户→评价索引 | 文档标注，暂不实现 |

## 审计记录 (v0.3.0)

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| High | H1 | `submit_review` 接受空 CID `Some(vec![])` — 无语义价值，浪费存储 | 添加 `ensure!(!c.is_empty(), EmptyCid)` |
| High | H2 | 无 per-shop 评价计数 — 查询店铺评价数量需全表扫描 | 新增 `ShopReviewCount` StorageMap |
| Medium | M1 | 权重硬编码 `(35M, 5K)` 无 `WeightInfo` trait — 不支持 benchmark 集成 | 新建 `weights.rs` + Config `WeightInfo` |
| Medium | M2 | README 说 "24 tests" 实际 22 个 | 修正 |
| Low | L1 | 无评价时间窗口 — 订单完成后可无限期评价 | 记录（需 OrderProvider 扩展） |
| Low | L2 | 无评价审核/删除机制 — 管理员无法处理虚假评价 | 记录（设计决策） |
| Low | L3 | CID 格式不验证（仅长度检查，不校验 IPFS 编码） | 记录（链上验证成本过高） |

## 审计记录 (v0.4.0)

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| High | H1 | `update_shop_rating` 失败导致整个评价回滚 — 评价与店铺评分强耦合，shop 关闭/删除时无法提交评价 | 改为 best-effort，失败发出 `ShopRatingUpdateFailed` 事件但不回滚 |
| High | H2 | 无用户评价索引 — 按用户查询评价需全表扫描 O(N) | 新增 `UserReviews` StorageMap + `MaxReviewsPerUser` Config 常量 |
| Medium | M1 | `ReviewCount` 全局计数器可被 `ShopReviewCount` 各店铺值之和替代 | 记录（保留，快速查询全局总数仍有价值） |
| Medium | M2 | 无评价修改/追评机制 | 记录（设计决策） |
| Medium | M3 | 纯评分无内容评价业务价值低 | 记录（应用层引导） |
| Low | L1 | `MallReview` 命名与 pallet 名不一致（历史遗留） | 记录（重命名需存储迁移） |
| Low | L2 | README weight 代码片段与实际代码不一致 | 已修正 |
| Low | L3 | `ReviewCount` 与 `ShopReviewCount` 存储冗余 | 记录（保留便捷查询） |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.5.0 | 2026-03-02 | Entity 评价开关：+EntityProvider Config、+set_review_enabled extrinsic、+EntityReviewDisabled 存储、+submit_review 门控、+8 测试（41 total） |
| v0.4.0 | 2026-03-03 | 深度审计 Round 3：H1/H2/L2 共 3 项修复，+4 测试（33 total） |
| v0.3.0 | 2026-02-26 | 深度审计 Round 2：H1/H2/M1/M2 共 4 项修复，+7 测试（29 total） |
| v0.2.0 | 2026-02-09 | 深度审计：C3/H2/M3/L1 共 9 项修复，22 测试 |
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，独立评价模块 |
