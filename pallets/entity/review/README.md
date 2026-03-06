# pallet-entity-review

> 订单评价模块 — 买家评分 · 商家回复 · 商品评价索引

**pallet_index** `123` · **version** `0.10.0` · **tests** `95`

---

## 功能概览

| 功能 | 说明 |
|------|------|
| 订单评价 | 买家对已完成订单提交 1–5 星评分，可附 IPFS CID 文字内容 |
| 评价修改 | 买家在时间窗口内可修改一次评分和内容 |
| 商家回复 | Entity owner/admin 对评价发表一次回复 |
| 评价移除 | Root 可删除违规评价，买家可重新评价 |
| Entity 开关 | Entity 管理员可关闭/开启旗下所有 Shop 的评价功能 |
| 店铺评分联动 | 提交评价时 best-effort 更新 `ShopProvider` 评分，失败不回滚 |
| 商品评价索引 | 按 `product_id` 维护评价列表、计数、评分总和，支持商品平均分查询 |
| 用户评价索引 | 按账户维护已评价 order_id 列表，支持"我的评价"查询 |

### 设计约束

- **一单一评** — `order_id` 为主键，评价不可重复（删除后可重新提交）
- **评价时间窗口** — 订单完成后 `ReviewWindowBlocks` 区块内可评价，超时拒绝
- **修改时间窗口** — 评价提交后 `EditWindowBlocks` 区块内可修改，仅限一次
- **best-effort 副作用** — 店铺评分更新、ShopReviewCount、商品索引的失败不回滚主评价写入

---

## 数据结构

### MallReview

```rust
pub struct MallReview<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
    pub order_id: u64,
    pub reviewer: AccountId,
    pub rating: u8,                                     // 1–5
    pub content_cid: Option<BoundedVec<u8, MaxCidLen>>, // IPFS CID
    pub created_at: BlockNumber,
    pub product_id: Option<u64>,
    pub edited: bool,
}
```

### ReviewReply

```rust
pub struct ReviewReply<AccountId, BlockNumber, MaxCidLen: Get<u32>> {
    pub replier: AccountId,
    pub content_cid: BoundedVec<u8, MaxCidLen>,
    pub created_at: BlockNumber,
}
```

---

## Runtime 配置

```rust
impl pallet_entity_review::Config for Runtime {
    type EntityProvider    = EntityRegistry;              // pallet-entity-registry
    type OrderProvider     = EntityTransaction;           // pallet-entity-order
    type ShopProvider      = EntityShop;                  // pallet-entity-shop
    type MaxCidLength      = ConstU32<64>;
    type MaxReviewsPerUser = ConstU32<500>;
    type ReviewWindowBlocks = ConstU64<100800>;           // ~7 days
    type EditWindowBlocks   = ConstU64<14400>;            // ~1 day
    type MaxProductReviews  = ConstU32<10000>;
    type WeightInfo        = pallet_entity_review::weights::SubstrateWeight<Runtime>;
}
```

| 常量 | 类型 | 说明 |
|------|------|------|
| `MaxCidLength` | `u32` | IPFS CID 最大字节长度 |
| `MaxReviewsPerUser` | `u32` | 单用户可评价的最大订单数 |
| `ReviewWindowBlocks` | `u64` | 评价提交窗口（0 = 不限制） |
| `EditWindowBlocks` | `u64` | 评价修改窗口（0 = 不限制） |
| `MaxProductReviews` | `u32` | 单商品最大评价索引数 |

---

## Extrinsics

### `submit_review` — call_index 0

买家提交订单评价。

```
签名: Signed(buyer)
参数: order_id: u64, rating: u8, content_cid: Option<Vec<u8>>
```

**流程:**
1. `rating ∈ [1, 5]` · 调用者 == 订单买家 · 订单已完成 · 未争议 · 未评价
2. 评价时间窗口校验（`ReviewWindowBlocks`）
3. Entity 评价开关校验（order → shop → entity → `EntityReviewDisabled`）
4. CID 校验（非空 + 长度 ≤ `MaxCidLength`）
5. 写入 `UserReviews`（bounded push） → `Reviews` → `ReviewCount`（checked_add）
6. best-effort: `ShopProvider::update_shop_rating` → `ShopReviewCount` → 商品索引
7. 事件: `ReviewSubmitted { order_id, reviewer, shop_id, rating }`

### `set_review_enabled` — call_index 1

Entity 管理员控制评价开关。

```
签名: Signed(admin)
参数: entity_id: u64, enabled: bool
权限: Entity owner 或 admin (REVIEW_MANAGE)
前置: Entity 存在 · 活跃 · 未锁定
```

仅在状态实际变更时写入存储和发射事件 `ReviewConfigUpdated`。

### `remove_review` — call_index 2

Root 移除违规评价。

```
签名: Root
参数: order_id: u64
```

清理: `Reviews` · `ReviewCount` · `ShopReviewCount` · `UserReviews` · `ReviewReplies` · `ProductReviews` · `ProductReviewCount` · `ProductRatingSum`。删除后买家可重新评价。

### `reply_to_review` — call_index 3

商家回复评价（每条评价限一次回复）。

```
签名: Signed(admin)
参数: order_id: u64, content_cid: Vec<u8>
权限: Entity owner 或 admin (REVIEW_MANAGE)
前置: 评价存在 · 未回复 · 内容非空 · Entity 活跃
```

写入 `ReviewReplies`，事件: `ReviewReplied { order_id, replier }`。

### `edit_review` — call_index 4

买家修改评价（仅限一次，在时间窗口内）。

```
签名: Signed(reviewer)
参数: order_id: u64, new_rating: u8, new_content_cid: Option<Vec<u8>>
前置: 评价存在 · 调用者是评价者 · 未修改过 · Entity 评价未关闭 · 窗口内
```

更新 `Reviews`（`edited = true`）+ `ProductRatingSum`（精确差值修正）。
不更新店铺评分（`ShopProvider::update_shop_rating` 是追加模式，无减法 API）。
事件: `ReviewEdited { order_id, reviewer, old_rating, new_rating }`。

---

## Storage

| 存储项 | Key | Value | Query |
|--------|-----|-------|-------|
| `Reviews` | `u64` (order_id) | `MallReviewOf<T>` | Option |
| `ReviewCount` | — | `u64` | ValueQuery (0) |
| `ShopReviewCount` | `u64` (shop_id) | `u64` | ValueQuery (0) |
| `UserReviews` | `AccountId` | `BoundedVec<u64, MaxReviewsPerUser>` | ValueQuery |
| `EntityReviewDisabled` | `u64` (entity_id) | `()` | Option |
| `ReviewReplies` | `u64` (order_id) | `ReviewReplyOf<T>` | Option |
| `ProductReviews` | `u64` (product_id) | `BoundedVec<u64, MaxProductReviews>` | ValueQuery |
| `ProductReviewCount` | `u64` (product_id) | `u64` | ValueQuery (0) |
| `ProductRatingSum` | `u64` (product_id) | `u64` | ValueQuery (0) |

---

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ReviewSubmitted` | `order_id, reviewer, shop_id: Option<u64>, rating` | 评价提交成功 |
| `ShopRatingUpdateFailed` | `order_id, shop_id` | 店铺评分更新失败（评价仍已存储） |
| `ReviewConfigUpdated` | `entity_id, enabled` | Entity 评价开关变更 |
| `ReviewRemoved` | `order_id, reviewer` | Root 移除评价 |
| `ReviewReplied` | `order_id, replier` | 商家回复评价 |
| `ProductReviewIndexed` | `product_id, order_id` | 商品评价索引成功 |
| `ReviewEdited` | `order_id, reviewer, old_rating, new_rating` | 评价修改成功 |

---

## Errors

| 错误 | 触发场景 |
|------|----------|
| `InvalidRating` | 评分不在 1–5 |
| `OrderNotFound` | 订单不存在 |
| `NotOrderBuyer` | 调用者非订单买家 / 非评价者 |
| `OrderNotCompleted` | 订单未完成 |
| `OrderDisputed` | 订单处于争议中 |
| `AlreadyReviewed` | 订单已评价 |
| `ReviewNotFound` | 评价不存在 |
| `ReviewWindowExpired` | 超出评价提交窗口 |
| `EditWindowExpired` | 超出评价修改窗口 |
| `AlreadyEdited` | 评价已修改过（仅限一次） |
| `CidTooLong` | CID 超过 MaxCidLength |
| `EmptyCid` | CID 为空字节 |
| `UserReviewLimitReached` | 用户评价数达 MaxReviewsPerUser |
| `ReviewsDisabledForEntity` | Entity 关闭了评价功能 |
| `EntityNotFound` | Entity 不存在 |
| `EntityNotActive` | Entity 未激活 |
| `EntityLocked` | Entity 已被全局锁定 |
| `NotEntityAdmin` | 调用者非 Entity admin |
| `NotShopEntityAdmin` | 调用者非店铺关联 Entity admin |
| `AlreadyReplied` | 评价已有回复 |
| `ReplyContentEmpty` | 回复内容为空 |
| `ProductReviewsFull` | 商品评价索引达 MaxProductReviews |
| `ReviewCountOverflow` | 全局评价计数溢出 u64 |

---

## 权重估算

| Extrinsic | ref_time | proof_size | reads | writes |
|-----------|----------|------------|-------|--------|
| `submit_review` | 55M | 8K | 10 | 5 |
| `set_review_enabled` | 25M | 4K | 4 | 1 |
| `remove_review` | 45M | 7K | 3 | 8 |
| `reply_to_review` | 35M | 5K | 6 | 1 |
| `edit_review` | 40M | 6K | 5 | 2 |

> 预估值（pre-benchmark），实际权重将由 `frame-benchmarking` 生成。

---

## 依赖接口

本模块通过 3 个 trait 与外部 pallet 解耦：

### OrderProvider (pallet-entity-common → pallet-entity-order)

本模块使用的方法：

| 方法 | 返回 | 用途 |
|------|------|------|
| `order_buyer` | `Option<AccountId>` | 验证买家身份 |
| `is_order_completed` | `bool` | 订单完成校验 |
| `is_order_disputed` | `bool` | 争议状态校验 |
| `order_completed_at` | `Option<u64>` | 时间窗口计算 |
| `order_shop_id` | `Option<u64>` | 关联店铺 |
| `order_product_id` | `Option<u64>` | 关联商品 |

### ShopProvider (pallet-entity-common → pallet-entity-shop)

| 方法 | 说明 |
|------|------|
| `update_shop_rating(shop_id, rating)` | 追加模式更新店铺评分（sum += rating, count += 1） |
| `shop_entity_id(shop_id)` | 获取店铺所属 Entity ID |

### EntityProvider (pallet-entity-common → pallet-entity-registry)

| 方法 | 说明 |
|------|------|
| `entity_exists` | Entity 存在性 |
| `is_entity_active` | Entity 激活状态 |
| `is_entity_locked` | Entity 全局锁定 |
| `is_entity_admin(entity_id, account, permission)` | Admin 权限校验（REVIEW_MANAGE 位） |

---

## 文件结构

```
pallets/entity/review/
├── Cargo.toml          # v0.10.0
├── README.md
└── src/
    ├── lib.rs           # 645 行 — Config · Storage · Events · Errors · 5 Extrinsics
    ├── weights.rs       # 87 行 — WeightInfo trait + SubstrateWeight 预估
    ├── mock.rs          # Mock runtime (EntityProvider / OrderProvider / ShopProvider)
    └── tests.rs         # 95 tests (93 #[test] + 2 auto-generated)
```

---

## 审计历史

<details>
<summary>v0.2.0 — 初始审计（9 项修复，22 tests）</summary>

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| Critical | C1 | `MallReview` 缺 `DecodeWithMemTracking` | 添加 derive |
| Critical | C2 | 零测试覆盖 | 创建 mock + 22 tests |
| Critical | C3 | weight `proof_size=0` | 修复为 `(35M, 5K)` |
| High | H1 | `update_shop_rating` 失败被静默忽略 | 改为 `?` 传播 |
| High | H2 | `order_exists` 冗余调用 | 移除 |
| Medium | M1 | 未使用依赖 | 清理 |
| Medium | M2 | 事件缺 `shop_id` | 添加 |
| Medium | M3 | 弃用 `RuntimeEvent` | 移除 |
| Low | L1 | 无用户索引 | 记录 |

</details>

<details>
<summary>v0.3.0 — Round 2（4 项修复，29 tests）</summary>

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| High | H1 | 接受空 CID `Some(vec![])` | 添加 `EmptyCid` 校验 |
| High | H2 | 无 per-shop 计数 | 新增 `ShopReviewCount` |
| Medium | M1 | 权重硬编码无 trait | 新建 `weights.rs` |
| Medium | M2 | 测试计数错误 | 修正 |

</details>

<details>
<summary>v0.4.0 — Round 3（3 项修复，33 tests）</summary>

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| High | H1 | shop rating 失败导致整个评价回滚 | 改为 best-effort |
| High | H2 | 无用户评价索引 | 新增 `UserReviews` + `MaxReviewsPerUser` |
| Low | L2 | README weight 片段过时 | 修正 |

</details>

<details>
<summary>v0.5.0 — Entity 评价开关（41 tests）</summary>

新增 `EntityProvider` Config · `set_review_enabled` extrinsic · `EntityReviewDisabled` 存储 · `submit_review` 门控。

</details>

<details>
<summary>v0.6.0 — Round 5（4 项修复，48 tests）</summary>

争议状态检查 · 幂等 `set_review_enabled` · `ReviewCount` checked_add · `ShopReviewCount` best-effort overflow。

</details>

<details>
<summary>v0.7.0 — 评价时间窗口（52 tests）</summary>

新增 `ReviewWindowBlocks` · `order_completed_at` · `ReviewWindowExpired`。

</details>

<details>
<summary>v0.8.0 — Round 7（3 项修复，54 tests）</summary>

`ShopReviewCount` best-effort · `order_shop_id` 去重 · weight 注释更新。

</details>

<details>
<summary>v0.9.0 — Round 9（7 项修复，62→89 tests）</summary>

`integrity_test` · `remove_review` Root extrinsic · Mock admin 权限粒度 · `reply_to_review` (F1) · 商品评价索引 (F3) · `edit_review` (F6) · Entity rating 缓存 (R7) 。

</details>

<details>
<summary>v0.10.0 — Round 10（8 项修复，95 tests）</summary>

| 级别 | ID | 问题 | 修复 |
|------|-----|------|------|
| High | H1 | `edit_review` 调用追加模式 `update_shop_rating` 腐蚀店铺平均分 | 移除调用，仅更新 `ProductRatingSum` |
| Medium | M1 | `edit_review` 不检查 Entity 评价开关 | 添加 `EntityReviewDisabled` 校验 |
| Medium | M2 | `reply_to_review` 不检查 Entity 激活状态 | 添加 `is_entity_active` 校验 |
| Medium | M3 | `integrity_test` 不校验 `MaxProductReviews` | 添加断言 |
| Low | L1–L2 | weights.rs 注释/reads/writes 不一致 | 修正 3 个函数 |
| Low | L3 | Cargo.toml 缺 feature 传播 | 添加 `pallet-entity-common` flags |
| Low | L4 | README 严重过时 | 全面同步 |

</details>
