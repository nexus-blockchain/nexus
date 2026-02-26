# pallet-entity-review

> ⭐ Entity 订单评价模块 — 订单完成后提交评分与评价

## 概述

`pallet-entity-review` 是 Entity 商城系统的评价管理模块，负责订单完成后的评分提交和店铺评分更新。

- **Runtime pallet_index:** 123
- **版本:** 0.3.0-audit

### 核心功能

- **订单评价** — 买家在订单完成后提交 1-5 星评分
- **评价内容** — 支持通过 IPFS CID 关联评价详情（可选）
- **店铺评分联动** — 评价提交后通过 `ShopProvider::update_shop_rating` 更新店铺评分（失败则事务回滚）
- **一单一评** — 每个订单仅允许一次评价，以 `order_id` 为 key 存储

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
    type OrderProvider = EntityTransaction;   // 订单查询 — pallet-entity-transaction
    type ShopProvider = EntityShop;           // 店铺评分更新 — pallet-entity-shop
    type MaxCidLength = ConstU32<64>;         // CID 最大长度 64 字节
}
```

> **注意：** `RuntimeEvent` 已从 Config trait 中移除（polkadot-sdk #7229 自动追加）。

### Config 关联类型

| 类型 | 约束 | 说明 |
|------|------|------|
| `OrderProvider` | `OrderProvider<AccountId, u128>` | 订单查询接口 |
| `ShopProvider` | `ShopProvider<AccountId>` | 店铺更新接口 |
| `MaxCidLength` | `Get<u32>` (常量) | CID 最大长度 |
| `WeightInfo` | `WeightInfo` | 权重信息（benchmark 集成） |

## Extrinsics

### submit_review (call_index 0)

提交订单评价。

```rust
#[pallet::call_index(0)]
#[pallet::weight(Weight::from_parts(35_000_000, 5_000))]
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
5. CID 长度校验 → `CidTooLong`（`BoundedVec::try_into`）

**执行逻辑：**
1. 构建 `MallReview` 记录（含当前区块号）
2. 写入 `Reviews` 存储（key = `order_id`）
3. `ReviewCount` 递增（`saturating_add`）
4. 通过 `OrderProvider::order_shop_id` 获取店铺 ID，调用 `ShopProvider::update_shop_rating` 更新评分（失败传播错误，事务回滚）
5. 发出 `ReviewSubmitted` 事件（含 `shop_id`）

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Reviews` | `StorageMap<Blake2_128Concat, u64, MallReviewOf<T>>` | 订单 ID → 评价记录 |
| `ReviewCount` | `StorageValue<u64, ValueQuery>` | 全局评价总数（默认 0） |
| `ShopReviewCount` | `StorageMap<Blake2_128Concat, u64, u64, ValueQuery>` | 店铺 ID → 该店铺评价数量 |

> **L1 备注：** 当前无用户→评价索引（仅 `order_id → review`），按用户查询需遍历或链下索引。

## Events

| 事件 | 字段 | 说明 |
|------|------|------|
| `ReviewSubmitted` | `order_id: u64`, `reviewer: AccountId`, `shop_id: Option<u64>`, `rating: u8` | 评价已提交 |

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

## 依赖接口

### OrderProvider（来自 pallet-entity-common）

由 `pallet-entity-transaction` 实现。

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
    └── tests.rs    # 单元测试（29 tests）
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

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.3.0 | 2026-02-26 | 深度审计 Round 2：H1/H2/M1/M2 共 4 项修复，+7 测试（29 total） |
| v0.2.0 | 2026-02-09 | 深度审计：C3/H2/M3/L1 共 9 项修复，22 测试 |
| v0.1.0 | 2026-01-31 | 从 pallet-mall 拆分，独立评价模块 |
