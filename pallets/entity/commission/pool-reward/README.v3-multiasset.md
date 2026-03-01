# 沉淀池奖励 — 多资产扩展设计 (v3-multiasset)

> 基于 pool-reward v2（周期性等额分配模型），扩展支持 Entity Token 沉淀池，  
> 使 Entity Token 支付的订单也能产生佣金沉淀并供会员领取。

---

## 1. 现状分析

### 1.1 当前资金流（NEX 单币种）

```
订单支付 (NEX)
  │
  ▼
process_commission(order_amount, available_pool, platform_fee)   ← 全部 NEX
  │
  ├── Phase 1: 4 个插件分配 (Referral / LevelDiff / SingleLine / Team)
  │       seller NEX → beneficiary NEX
  │
  ├── Phase 1.5: 未分配佣金 → UnallocatedPool
  │       seller NEX → entity_account NEX
  │       UnallocatedPool[entity_id] += amount
  │
  └── Phase 2 (v2): 用户主动 claim
          entity_account NEX → claimant NEX
          UnallocatedPool[entity_id] -= amount
```

### 1.2 Entity Token 在订单中的角色

| 环节 | 当前实现 | 说明 |
|---|---|---|
| `create_order(use_tokens)` | `EntityToken::redeem_for_discount()` | 积分抵扣，减少 NEX 应付额 |
| 订单完成后 | `EntityToken::reward_on_purchase()` | 购物返积分 |
| 直接用 Entity Token 支付全单 | **不支持** | 订单必须以 NEX 结算 |

### 1.3 关键限制

1. **`UnallocatedPool`** 是 `StorageMap<u64, BalanceOf<T>>`，单键 = entity_id，只存 NEX
2. **`T::Currency`** 绑定 `pallet_balances`（原生币），无法操作 `pallet_assets`
3. **`PoolBalanceProvider`** trait 只有一个余额维度
4. **pool-reward v2** 的 `claim_pool_reward` 只用 `T::Currency::transfer` 转账

---

## 2. 目标

**允许 Entity Token 订单产生佣金 → 沉淀入 Entity Token 池 → 会员可领取 Entity Token 奖励。**

设计原则：
- **双池并行**：NEX 池 + Entity Token 池独立运行，互不干扰
- **向后兼容**：现有 NEX-only Entity 无需任何迁移
- **统一领取入口**：用户调用一次 `claim_pool_reward` 同时领取 NEX 和 Entity Token 两种奖励
- **最小侵入**：尽量复用 v2 轮次/快照机制，不改变核心分配公式

---

## 3. 架构总览

### 3.1 扩展后的资金流

```
订单支付
  │
  ├── NEX 支付 ──────────────► process_commission(NEX) ── 原有流程不变
  │                                └── UnallocatedPool[entity_id] (NEX)
  │
  └── Entity Token 支付 ────► process_token_commission(Token)
                                   └── UnallocatedTokenPool[entity_id] (Token)

claim_pool_reward(entity_id)
  │
  ├── NEX 部分:  entity_account → claimant (Currency::transfer)
  │              UnallocatedPool[entity_id] -= nex_reward
  │
  └── Token 部分: Assets::transfer(entity_token_id, entity_account → claimant)
                  UnallocatedTokenPool[entity_id] -= token_reward
```

### 3.2 分层职责变更

| 层 | 当前 | 扩展后 |
|---|---|---|
| `transaction` | 订单只支持 NEX | 新增 Entity Token 全额支付模式 |
| `commission-common` | `PoolBalanceProvider<Balance>` 单币种 | 新增 `TokenPoolBalanceProvider<AssetBalance>` |
| `commission-core` | `UnallocatedPool` NEX 单键 | 新增 `UnallocatedTokenPool` 存储 |
| `pool-reward` | 只分配 NEX | 双池快照 + 双币种转账 |
| `entity-token` | 积分/折扣功能 | 新增池操作接口（mint/burn 或 transfer） |

---

## 4. 详细设计

### 4.1 Layer 0: Entity Token 订单支付（transaction 层）

#### 4.1.1 Order 结构扩展

```rust
pub struct Order<AccountId, Balance, BlockNumber, MaxCidLen: Get<u32>> {
    // ... 现有字段不变 ...

    /// 支付方式标识
    pub payment_asset: PaymentAsset,
    /// Entity Token 支付金额（当 payment_asset = Token 时有效）
    pub token_payment_amount: Balance,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum PaymentAsset {
    /// 原生 NEX
    Native,
    /// Entity Token（entity_id 对应的 pallet-assets 资产）
    EntityToken,
}
```

#### 4.1.2 新增 place_order 参数

```rust
pub fn place_order(
    origin: OriginFor<T>,
    shop_id: u64,
    product_id: u64,
    quantity: u32,
    shipping_cid: Option<Vec<u8>>,
    use_tokens: Option<BalanceOf<T>>,
    use_shopping_balance: Option<BalanceOf<T>>,
    pay_with_entity_token: bool,  // 新增：是否用 Entity Token 全额支付
) -> DispatchResult
```

当 `pay_with_entity_token = true` 时：
1. 查询 Entity Token 是否启用且类型允许商品支付
2. 将 NEX 标价按 `exchange_rate` 换算为 Token 数量
3. 从买家 Token 余额扣除（通过 `EntityTokenProvider::transfer` 或 `Assets::transfer`）
4. 资金进入 Entity 托管（Token Escrow）
5. 订单完成时，调用 `process_token_commission` 而非 `process_commission`

#### 4.1.3 Token 汇率

Entity Token 与 NEX 的换算由 `EntityTokenConfig.exchange_rate` 决定（已有字段）。

```
token_amount = nex_amount * 10000 / exchange_rate
```

示例：`exchange_rate = 1000`（10%），100 NEX 等价需要 1000 Token。

---

### 4.2 Layer 1: commission-common trait 扩展

#### 4.2.1 新增 TokenPoolBalanceProvider

```rust
/// Entity Token 沉淀池读写接口
pub trait TokenPoolBalanceProvider<Balance> {
    /// 查询 Entity Token 沉淀池余额
    fn token_pool_balance(entity_id: u64) -> Balance;

    /// 扣减 Entity Token 沉淀池
    fn deduct_token_pool(entity_id: u64, amount: Balance) -> Result<(), DispatchError>;
}

/// 空实现
impl<Balance: Default + PartialOrd + core::ops::SubAssign> TokenPoolBalanceProvider<Balance> for () {
    fn token_pool_balance(_: u64) -> Balance { Default::default() }
    fn deduct_token_pool(_: u64, _: Balance) -> Result<(), DispatchError> { Ok(()) }
}
```

#### 4.2.2 扩展 PoolRewardPlanWriter

```rust
pub trait PoolRewardPlanWriter {
    fn set_pool_reward_config(
        entity_id: u64,
        level_ratios: Vec<(u8, u16)>,
        round_duration: u32,
    ) -> Result<(), DispatchError>;

    fn clear_config(entity_id: u64) -> Result<(), DispatchError>;

    /// 新增：设置 Entity Token 池奖励是否启用
    fn set_token_pool_enabled(entity_id: u64, enabled: bool) -> Result<(), DispatchError> {
        let _ = (entity_id, enabled);
        Ok(())
    }
}
```

#### 4.2.3 新增 CommissionTokenProvider

```rust
/// Entity Token 佣金结算接口（supply 给 commission-core 使用）
pub trait CommissionTokenProvider<AccountId, Balance> {
    /// 处理 Entity Token 订单的佣金结算
    fn process_token_commission(
        entity_id: u64,
        shop_id: u64,
        order_id: u64,
        buyer: &AccountId,
        token_amount: Balance,
        available_token_pool: Balance,
    ) -> Result<(), DispatchError>;
}
```

---

### 4.3 Layer 2: commission-core 存储扩展

#### 4.3.1 新增 UnallocatedTokenPool

```rust
/// Entity Token 未分配佣金沉淀池 entity_id -> AssetBalance
#[pallet::storage]
pub type UnallocatedTokenPool<T: Config> = StorageMap<
    _,
    Blake2_128Concat, u64,
    TokenBalanceOf<T>,     // 新增类型别名
    ValueQuery,
>;
```

其中：

```rust
pub type TokenBalanceOf<T> = <T as Config>::TokenBalance;
```

#### 4.3.2 Config 新增关联类型

```rust
pub trait Config: frame_system::Config {
    // ... 现有类型 ...

    /// Entity Token 余额类型（与 pallet-assets 对齐）
    type TokenBalance: Member + Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;

    /// Entity Token 转账接口
    type TokenTransfer: EntityTokenTransfer<Self::AccountId, Self::TokenBalance>;
}
```

#### 4.3.3 新增 process_token_commission

核心逻辑与 `process_commission` 类似，但：
- 不处理平台费（Token 订单是否收平台费需另行决定）
- Plugin 分配仅处理 `POOL_REWARD` 模式（Token 佣金不走 Referral/LevelDiff 等 NEX 插件）
- 未分配部分直接进入 `UnallocatedTokenPool`

```rust
pub fn process_token_commission(
    entity_id: u64,
    order_id: u64,
    token_order_amount: TokenBalanceOf<T>,
    max_commission_rate: u16,    // 与 NEX 共用佣金比例配置
) -> DispatchResult {
    let config = CommissionConfigs::<T>::get(entity_id)
        .filter(|c| c.enabled)
        .ok_or(Error::<T>::CommissionNotConfigured)?;

    let enabled_modes = config.enabled_modes;

    if enabled_modes.contains(CommissionModes::POOL_REWARD) {
        let pool_amount = token_order_amount
            .saturating_mul(max_commission_rate.into())
            / 10000u32.into();

        // Entity Token 佣金全部进沉淀池
        UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
            *pool = pool.saturating_add(pool_amount);
        });

        Self::deposit_event(Event::TokenCommissionPooled {
            entity_id,
            order_id,
            amount: pool_amount,
        });
    }

    Ok(())
}
```

#### 4.3.4 实现 TokenPoolBalanceProvider

```rust
impl<T: pallet::Config> TokenPoolBalanceProvider<TokenBalanceOf<T>> for Pallet<T> {
    fn token_pool_balance(entity_id: u64) -> TokenBalanceOf<T> {
        UnallocatedTokenPool::<T>::get(entity_id)
    }

    fn deduct_token_pool(entity_id: u64, amount: TokenBalanceOf<T>) -> Result<(), DispatchError> {
        UnallocatedTokenPool::<T>::try_mutate(entity_id, |pool| {
            ensure!(*pool >= amount, DispatchError::Other("InsufficientTokenPool"));
            *pool -= amount;
            Ok(())
        })
    }
}
```

---

### 4.4 Layer 3: pool-reward 多资产扩展

#### 4.4.1 Config 新增关联类型

```rust
pub trait Config: frame_system::Config {
    // ... v2 现有类型 ...

    /// Entity Token 余额类型
    type TokenBalance: Member
        + Parameter
        + AtLeast32BitUnsigned
        + Default
        + Copy
        + MaxEncodedLen
        + From<u128>
        + Into<u128>;

    /// Entity Token Asset ID 类型
    type AssetId: Member + Parameter + Copy + MaxEncodedLen + From<u64> + Into<u64>;

    /// Entity Token 池余额读写
    type TokenPoolBalanceProvider: TokenPoolBalanceProvider<Self::TokenBalance>;

    /// Entity Token 转账接口（fungibles::Transfer 或自定义）
    type TokenTransfer: fungibles::Transfer<
        Self::AccountId,
        AssetId = Self::AssetId,
        Balance = Self::TokenBalance,
    >;

    /// Entity Token 配置查询（查 asset_id）
    type EntityTokenProvider: EntityTokenProvider<Self::AccountId, BalanceOf<Self>>;
}
```

#### 4.4.2 RoundInfo 扩展

```rust
pub struct RoundInfo<MaxLevels: Get<u32>, Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub start_block: BlockNumber,

    /// NEX 池快照
    pub pool_snapshot: Balance,
    pub level_snapshots: BoundedVec<LevelSnapshot<Balance>, MaxLevels>,

    /// Entity Token 池快照（None = 该 entity 未启用 Token 池）
    pub token_pool_snapshot: Option<TokenBalance>,
    pub token_level_snapshots: Option<BoundedVec<LevelSnapshot<TokenBalance>, MaxLevels>>,
}
```

#### 4.4.3 ClaimRecord 扩展

```rust
pub struct ClaimRecord<Balance, TokenBalance, BlockNumber> {
    pub round_id: u64,
    pub level_id: u8,
    pub claimed_at: BlockNumber,

    /// NEX 领取数量
    pub nex_amount: Balance,
    /// Entity Token 领取数量（0 = 无 Token 奖励）
    pub token_amount: TokenBalance,
}
```

#### 4.4.4 PoolRewardConfig 扩展

```rust
pub struct PoolRewardConfig<MaxLevels: Get<u32>, BlockNumber> {
    pub level_ratios: BoundedVec<(u8, u16), MaxLevels>,
    pub round_duration: BlockNumber,

    /// 是否启用 Entity Token 池分配（默认 false）
    pub token_pool_enabled: bool,
    /// Token 池是否使用独立分配比例（None = 共用 level_ratios）
    pub token_level_ratios: Option<BoundedVec<(u8, u16), MaxLevels>>,
}
```

> **设计决策**：NEX 池和 Token 池共用同一套 `level_ratios` 作为默认行为，  
> 但允许通过 `token_level_ratios` 为 Token 池设置独立比例。  
> 例如：NEX 池 level_1=50%, level_2=50%；Token 池 level_1=30%, level_2=70%。

#### 4.4.5 claim_pool_reward 扩展

```rust
pub fn claim_pool_reward(
    origin: OriginFor<T>,
    entity_id: u64,
) -> DispatchResult {
    let who = ensure_signed(origin)?;

    // 1-5: 资格/轮次/防双领/快照/配额检查（与 v2 相同）
    // ...

    // 6a. NEX 部分转账
    let nex_reward = snapshot.per_member_reward;
    if !nex_reward.is_zero() {
        let pool = T::PoolBalanceProvider::pool_balance(entity_id);
        ensure!(pool >= nex_reward, Error::<T>::InsufficientPool);
        let entity_account = T::EntityProvider::entity_account(entity_id);
        T::Currency::transfer(&entity_account, &who, nex_reward, ExistenceRequirement::KeepAlive)?;
        T::PoolBalanceProvider::deduct_pool(entity_id, nex_reward)?;
    }

    // 6b. Token 部分转账（如果启用且有快照）
    let mut token_reward = Self::TokenBalance::zero();
    if let Some(ref token_snapshots) = round.token_level_snapshots {
        if let Some(token_snap) = token_snapshots.iter().find(|s| s.level_id == user_level) {
            token_reward = token_snap.per_member_reward;
            if !token_reward.is_zero() {
                let token_pool = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
                ensure!(token_pool >= token_reward, Error::<T>::InsufficientTokenPool);

                let asset_id: T::AssetId = entity_token_asset_id(entity_id);
                let entity_account = T::EntityProvider::entity_account(entity_id);
                T::TokenTransfer::transfer(asset_id, &entity_account, &who, token_reward, Preservation::Preserve)?;
                T::TokenPoolBalanceProvider::deduct_token_pool(entity_id, token_reward)?;
            }
        }
    }

    // 7-9: 状态更新、写入历史（扩展包含 token_amount）
    // ...

    Self::deposit_event(Event::PoolRewardClaimed {
        entity_id,
        account: who,
        nex_amount: nex_reward,
        token_amount: token_reward,
        round_id,
        level_id: user_level,
    });

    Ok(())
}
```

#### 4.4.6 create_new_round 扩展

```rust
fn create_new_round(...) -> Result<RoundInfoOf<T>, DispatchError> {
    // NEX 快照（与 v2 完全相同）
    let pool_balance = T::PoolBalanceProvider::pool_balance(entity_id);
    let level_snapshots = Self::build_level_snapshots(config, entity_id, pool_balance);

    // Token 快照（仅当 token_pool_enabled = true）
    let (token_pool_snapshot, token_level_snapshots) = if config.token_pool_enabled {
        let token_balance = T::TokenPoolBalanceProvider::token_pool_balance(entity_id);
        let ratios = config.token_level_ratios.as_ref()
            .unwrap_or(&config.level_ratios);
        let snaps = Self::build_token_level_snapshots(ratios, entity_id, token_balance);
        (Some(token_balance), Some(snaps))
    } else {
        (None, None)
    };

    let new_round = RoundInfo {
        round_id: old_round_id + 1,
        start_block: now,
        pool_snapshot: pool_balance,
        level_snapshots,
        token_pool_snapshot,
        token_level_snapshots,
    };

    // ...
}
```

---

### 4.5 Entity Token 转账机制

Entity Token 存储在 `pallet-assets` 中，资产 ID = `entity_id + ShopTokenOffset`。

#### 转账路径

```
Entity Token 沉淀池 (记账)  ←→  pallet-assets 实际余额
```

- **入池**：`process_token_commission` 时，Token 从买家转入 entity_account（通过 escrow 释放），同时 `UnallocatedTokenPool` 记账 += amount
- **出池**：`claim_pool_reward` 时，从 entity_account 的 Token 余额转给 claimant（通过 `fungibles::Transfer`），同时 `UnallocatedTokenPool` 记账 -= amount

entity_account 的 Token 余额必须 >= `UnallocatedTokenPool` 的记账值（与 NEX 池的 invariant 一致）。

---

## 5. 存储迁移

### 5.1 需迁移的存储项

| 存储 | 变更 | 迁移策略 |
|---|---|---|
| `PoolRewardConfig` | 新增 `token_pool_enabled`, `token_level_ratios` | 默认 `false` / `None`，零迁移 |
| `RoundInfo` | 新增 `token_pool_snapshot`, `token_level_snapshots` | 默认 `None`，零迁移 |
| `ClaimRecord` | 新增 `token_amount` | 默认 `0`，零迁移 |
| `UnallocatedTokenPool` (core) | 新增存储项 | 无旧数据，零迁移 |

所有新增字段使用 `Option` 或 `Default`，**无需链上迁移**。

### 5.2 Codec 兼容

旧数据 decode 时新增字段会取默认值。确保所有新增字段位于 struct 尾部且实现 `Default`。

---

## 6. Event 变更

```rust
pub enum Event<T: Config> {
    // v2 原有 Events 保持不变 ...

    /// Entity Token 佣金进入沉淀池
    TokenCommissionPooled {
        entity_id: u64,
        order_id: u64,
        amount: TokenBalanceOf<T>,
    },

    /// 领取事件扩展（原 PoolRewardClaimed 加字段）
    PoolRewardClaimed {
        entity_id: u64,
        account: T::AccountId,
        nex_amount: BalanceOf<T>,
        token_amount: TokenBalanceOf<T>,  // 新增
        round_id: u64,
        level_id: u8,
    },
}
```

---

## 7. Error 变更

```rust
pub enum Error<T> {
    // v2 原有 Errors 保持不变 ...

    /// Entity Token 沉淀池余额不足
    InsufficientTokenPool,
    /// Entity Token 未启用
    EntityTokenNotEnabled,
    /// Entity Token 转账失败
    TokenTransferFailed,
}
```

---

## 8. 安全考量

### 8.1 Entity Token 价格波动

Entity Token 价格可能波动。沉淀池快照记录的是 **Token 数量**，不是价值。
会员领取的是 Token 数量份额，价格风险由持有者自行承担。

### 8.2 双池独立性

NEX 池和 Token 池完全独立：
- 各自有独立的余额快照
- 各自有独立的 per_member_reward 计算
- 一个池不足不影响另一个池的领取
- 但共享同一个 `claimed_count`（防止一人领两次）

### 8.3 Entity Token 供应量

Entity Token 有 `max_supply` 限制。沉淀池分配只涉及已有 Token 的转移，不涉及铸造，
因此不会违反供应量上限。

### 8.4 Token 池为空时的行为

当 `UnallocatedTokenPool[entity_id] == 0` 时：
- Token 快照的 `per_member_reward` = 0
- 用户 claim 时 Token 部分跳过，只领取 NEX
- 不报错

---

## 9. 实施计划

### Phase A: 基础设施（可独立上线）

| 步骤 | 模块 | 内容 |
|---|---|---|
| A1 | `commission-common` | 新增 `TokenPoolBalanceProvider` trait |
| A2 | `commission-core` | 新增 `UnallocatedTokenPool` 存储 + `TokenPoolBalanceProvider` 实现 |
| A3 | `pool-reward` | Config 新增 Token 关联类型；`PoolRewardConfig` 新增 `token_pool_enabled` 字段（默认 false） |
| A4 | `pool-reward` | `RoundInfo` / `ClaimRecord` 扩展 Token 字段（Option/Default） |
| A5 | `pool-reward` | `create_new_round` 支持双池快照 |
| A6 | `pool-reward` | `claim_pool_reward` 支持双币种领取 |
| A7 | `pool-reward` | `set_pool_reward_config` 支持 `token_pool_enabled` 参数 |
| A8 | runtime | 适配新 Config 类型 |
| A9 | tests | 新增多资产测试用例 |

### Phase B: Entity Token 订单支付（依赖 Phase A）

| 步骤 | 模块 | 内容 |
|---|---|---|
| B1 | `transaction` | `Order` 结构新增 `payment_asset` 字段 |
| B2 | `transaction` | `place_order` 支持 `pay_with_entity_token` |
| B3 | `transaction` | Token Escrow 集成 |
| B4 | `commission-core` | 新增 `process_token_commission` |
| B5 | `transaction` | 订单完成时按 `payment_asset` 调用对应佣金处理 |

### Phase C: 高级功能（可选）

| 步骤 | 内容 |
|---|---|
| C1 | Token 池独立分配比例 (`token_level_ratios`) |
| C2 | Token 池 + NEX 池混合汇率快照（按价格折算统一分配） |
| C3 | 治理提案管理 Token 池参数 |

---

## 10. 影响范围

### 需修改的文件

| 文件 | Phase | 改动量 |
|---|---|---|
| `commission-common/src/lib.rs` | A1 | 小（新增 trait） |
| `commission-core/src/lib.rs` | A2, B4 | 中（新增存储 + 方法） |
| `commission-core/Cargo.toml` | A2 | 小（可能需 pallet-assets dep） |
| `pool-reward/src/lib.rs` | A3-A7 | 大（数据结构 + 逻辑扩展） |
| `pool-reward/Cargo.toml` | A3 | 小（新增 pallet-assets dep） |
| `transaction/src/lib.rs` | B1-B3, B5 | 大（订单支付重构） |
| `entity-common/src/lib.rs` | B1 | 小（PaymentAsset 枚举） |
| `runtime/src/configs/mod.rs` | A8 | 中（新增 Config 类型） |

### 不需修改的文件

- `pallet-entity-token`（已有足够接口）
- `pallet-entity-member`（v2 已完成 LevelMemberCount）
- `commission-referral` / `commission-level-diff` 等插件（Token 佣金不走这些插件）

---

## 11. 替代方案对比

### 方案 A: 双池并行（本文推荐）

- NEX 池和 Token 池独立快照、独立分配
- 优点：简单、可预测、向后兼容
- 缺点：两种奖励体验不同

### 方案 B: 统一汇率折算

- Token 佣金按实时汇率折算为 NEX 等值，统一进入 NEX 池
- 优点：用户只需关心一种奖励
- 缺点：汇率波动风险大、需要可靠的预言机、Token 持有激励弱

### 方案 C: 仅 Token 池（不动 NEX）

- NEX 佣金流程不变，仅新增独立的 Token 沉淀池
- 优点：改动最小
- 缺点：两套独立的 claim 入口，UX 割裂

**推荐方案 A**：兼顾灵活性和兼容性，用户一次 claim 同时领取双币种。

---

## 12. 测试场景

| 场景 | 预期 |
|---|---|
| Entity 未启用 Token 池，claim 只领 NEX | ✅ 向后兼容 |
| Entity 启用 Token 池，Token 池为空，claim 只领 NEX | ✅ Token 部分跳过 |
| Entity 启用 Token 池，两池都有余额，claim 领取双币种 | ✅ 两笔转账 |
| Token 池余额不足覆盖全部 per_member_reward | ❌ InsufficientTokenPool |
| NEX 池空但 Token 池有余额 | ✅ NEX=0, Token>0 |
| Entity Token 未启用但 config 中 token_pool_enabled=true | ❌ EntityTokenNotEnabled |
| 双池使用不同 level_ratios | ✅ 各自独立计算 |
| 轮次过期后新轮快照包含双池 | ✅ 双池同时快照 |
| claim_history 记录包含 nex_amount + token_amount | ✅ |
| Token 订单佣金正确进入 UnallocatedTokenPool | ✅ |
| 订单取消时 Token 佣金退回 | ✅ UnallocatedTokenPool -= amount |

---

## 附录 A: 深度分析 — Token 佣金跳过插件直接入池

### A.1 现有 NEX 佣金分配流程（代码级追踪）

```
process_commission(entity_id, buyer, order_amount, available_pool, platform_fee)
│
├── max_commission = available_pool × max_commission_rate / 10000
├── remaining = min(max_commission, seller_transferable_balance)
│
├── Plugin 1 (Referral):
│   ├── DIRECT_REWARD:  referrer ← order_amount × rate / 10000
│   ├── MULTI_LEVEL:    L1..Ln referrers ← 各级费率
│   ├── FIXED_AMOUNT:   referrer ← 固定金额
│   ├── FIRST_ORDER:    referrer ← 首单奖励
│   └── REPEAT_PURCHASE: referrer ← 复购奖励
│   → remaining -= Σ(plugin_outputs)
│
├── Plugin 2 (LevelDiff):  级差返佣 → remaining -= distributed
├── Plugin 3 (SingleLine): 单线收益 → remaining -= distributed
├── Plugin 4 (Team):       团队业绩 → remaining -= distributed
│
├── credit_commission(beneficiary, amount)   ← 仅记账，不转账
│   ├── CommissionRecord { status: Pending }
│   ├── MemberCommissionStats.pending += amount
│   └── ShopPendingTotal += amount
│
├── Phase 1.5: remaining → UnallocatedPool
│   └── T::Currency::transfer(seller → entity_account)  ← 实际 NEX 转账
│
└── 后续: withdraw_commission()
    └── T::Currency::transfer(entity_account → beneficiary)  ← 实际 NEX 转账
```

**关键发现**：

1. **插件只做计算，不做转账**。`CommissionPlugin::calculate()` 返回 `Vec<CommissionOutput>` 纯数据。
2. **`credit_commission()` 只做记账**。将金额记入 `Pending` 状态，不触发 `T::Currency::transfer`。
3. **实际转账发生在两处**：Phase 1.5（seller → entity_account 入池）和 `withdraw_commission`（entity_account → beneficiary 提现）。
4. **所有记账和转账都绑定 `BalanceOf<T>` = NEX 单一币种**。

---

### A.2 技术约束：为什么插件无法直接处理 Token

#### 约束 1: 类型系统锁死

```rust
// commission-common/src/lib.rs:171
pub trait CommissionPlugin<AccountId, Balance> {
    fn calculate(..., remaining: Balance, ...) -> (Vec<CommissionOutput<AccountId, Balance>>, Balance);
}

// commission-core Config:
type ReferralPlugin: CommissionPlugin<Self::AccountId, BalanceOf<Self>>;
//                                                     ^^^^^^^^^^^^^^^^^^
//                                                     BalanceOf = NEX，编译期绑定
```

`Balance` 泛型在 runtime 被实例化为 `BalanceOf<T>` = NEX。要让插件输出 Token 金额，需要：
- 引入第二个余额泛型 `CommissionPlugin<AccountId, NexBalance, TokenBalance>`
- 或使用枚举 `enum AssetAmount { Nex(u128), Token(u128) }`

两种方式都需要修改 **trait 定义 + 所有 5 个插件实现 + core 调度逻辑**。

#### 约束 2: 记账系统单币种

```rust
// commission-core/src/lib.rs:1267-1269
MemberCommissionStats::<T>::mutate(entity_id, beneficiary, |stats| {
    stats.total_earned = stats.total_earned.saturating_add(amount);  // amount: BalanceOf<T> = NEX
    stats.pending = stats.pending.saturating_add(amount);
});
```

`MemberCommissionStats` 的 `total_earned` / `pending` / `total_withdrawn` 全部是 NEX。
如果 Token 佣金也走这条路，需要：
- `MemberCommissionStats` 新增 `token_pending` / `token_total_earned` 字段
- `ShopPendingTotal` 拆分为 NEX + Token 双键
- `CommissionRecord` 新增 `asset_type` 字段
- 偿付安全检查（entity_balance >= pending + shopping + unallocated）需要双币种版本

#### 约束 3: 提现系统深度耦合 NEX

```rust
// withdraw_commission() 核心逻辑:
let split = Self::calc_withdrawal_split(entity_id, &who, total_amount, rate);
// split.withdrawal → T::Currency::transfer(entity_account → who)     [NEX]
// split.repurchase → ShoppingBalance[entity_id][target] += amount    [NEX]
// split.bonus     → ShoppingBalance[entity_id][target] += amount     [NEX]
```

提现系统包含：
- 复购比例（withdrawal vs shopping_balance 分流）
- 购物余额管理（ShopShoppingTotal）
- 冻结期检查（MemberLastCredited）
- 偿付安全检查（entity_balance >= required_reserve）

Token 佣金如果走提现系统，**复购比例对 Token 是否有意义？** Token 购物余额是什么概念？
这引出了一个更深层的设计问题。

#### 约束总结

| 改动项 | 影响范围 | 估计工作量 |
|---|---|---|
| `CommissionPlugin` trait 泛型扩展 | 5 个插件 + common + core | 大 |
| `CommissionOutput` 多资产 | common + core + 所有插件 | 中 |
| `credit_commission` 多资产 | core | 中 |
| `MemberCommissionStats` 双币种 | core + runtime + 查询 API | 中 |
| `ShopPendingTotal` 双币种 | core 偿付安全 | 中 |
| `withdraw_commission` Token 分支 | core（最复杂的函数） | 大 |
| `CommissionRecord` 多资产 | core + 历史查询 | 小 |
| **总计** | **~7 个文件，核心逻辑重构** | **~2-3 周** |

---

### A.3 激励分析：Token 佣金跳过插件的影响

#### 场景模拟

假设 Entity 配置：
- `max_commission_rate` = 10%（从货款中扣）
- Referral: 直推 5%
- Pool: level_1 = 50%, level_2 = 50%
- level_1 有 10 人, level_2 有 5 人

**NEX 订单 1000 NEX**：
```
可分配 = 1000 × 10% = 100 NEX
→ Referral (直推 5%): referrer 获得 50 NEX
→ remaining = 50 NEX → UnallocatedPool
→ Pool 分配:
    level_1: 50 × 50% / 10 = 2.5 NEX/人
    level_2: 50 × 50% / 5  = 5.0 NEX/人

推荐人（假设 level_1）总收益: 50 + 2.5 = 52.5 NEX
普通 level_1 会员收益: 2.5 NEX
```

**Token 订单 10000 Token（等值 1000 NEX）**：
```
可分配 = 10000 × 10% = 1000 Token
→ 跳过所有插件，全部入池
→ Pool 分配:
    level_1: 1000 × 50% / 10 = 50 Token/人
    level_2: 1000 × 50% / 5  = 100 Token/人

推荐人（level_1）总收益: 50 Token（≈ 5 NEX 等值）
普通 level_1 会员收益: 50 Token（≈ 5 NEX 等值）
```

#### 激励失衡表

| 角色 | NEX 订单收益 | Token 订单收益 | 差异 |
|---|---|---|---|
| **直推推荐人** | 50 NEX + 2.5 NEX pool = **52.5 NEX** | 50 Token (≈5 NEX) | **↓ 90%** |
| **普通 level_1 会员** | 2.5 NEX | 50 Token (≈5 NEX) | **↑ 100%** |
| **level_2 会员** | 5 NEX | 100 Token (≈10 NEX) | **↑ 100%** |

#### 核心问题

1. **推荐人激励崩塌**：直推推荐人是拉新的核心驱动力。Token 订单使其收益暴跌 90%，推荐人会主动劝阻买家使用 Token 支付。

2. **搭便车效应放大**：没有推荐贡献的高等级会员，从 Token 订单获得的收益反而更高（全部进池 = 更大的池子 = 更高的等额分配）。

3. **Entity 目标冲突**：Entity 通常希望推广 Token 使用（增强生态粘性），但推荐人的理性选择是抵制 Token 支付。

4. **套利空间**：大户可以用 Token 大额购物（全部进池），然后作为高等级会员从池中领取不成比例的份额。

---

### A.4 六种替代方案对比

#### 方案 E: Token 全额入池（原设计）

```
Token 佣金 100% → UnallocatedTokenPool → 等额分配
```

| 维度 | 评分 |
|---|---|
| 实现复杂度 | ⭐ 最简单 |
| 推荐人激励 | ❌ 严重不足 |
| 公平性 | ⚠️ 搭便车问题 |
| 向后兼容 | ⭐ 完全兼容 |

#### 方案 F: 混合分流 — 推荐人直得 + 余额入池 ⭐ 推荐

```
Token 佣金
├── X% → 直推推荐人（1 层，简单 Token 转账）
└── (100-X)% → UnallocatedTokenPool → 等额分配
```

实现方式：**不走插件管线**，在 `process_token_commission` 中硬编码一级推荐人分配。

```rust
pub fn process_token_commission(
    entity_id: u64,
    order_id: u64,
    buyer: &T::AccountId,
    token_amount: TokenBalanceOf<T>,
    max_commission_rate: u16,
) -> DispatchResult {
    let commission = token_amount * max_commission_rate / 10000;

    // 直推推荐人拿 X%（X 来自 ReferralConfig.direct_reward.rate）
    let referrer_share = if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
        let referral_rate = Self::get_token_referral_rate(entity_id); // 从配置读取
        let share = commission * referral_rate / 10000;
        if !share.is_zero() {
            // 直接 Token 转账：entity_account → referrer
            T::TokenTransfer::transfer(asset_id, &entity_account, &referrer, share)?;
        }
        share
    } else {
        Zero::zero()
    };

    // 剩余入池
    let pool_amount = commission - referrer_share;
    UnallocatedTokenPool::<T>::mutate(entity_id, |pool| *pool += pool_amount);

    Ok(())
}
```

| 维度 | 评分 |
|---|---|
| 实现复杂度 | ⭐⭐ 简单（~100 行新增，不动插件） |
| 推荐人激励 | ✅ 保留直推激励 |
| 公平性 | ✅ 推荐人得直推奖，池奖惠及全员 |
| 向后兼容 | ⭐ 完全兼容 |

**注意**：只做 1 层直推，不做多级/级差/团队。Token 佣金的多级分配过于复杂且价值有限。

#### 方案 A: 全插件管线多资产化

```
Token 佣金 → Referral → LevelDiff → SingleLine → Team → 剩余入池
（与 NEX 完全对称，但 Balance = TokenBalance）
```

| 维度 | 评分 |
|---|---|
| 实现复杂度 | ❌ 极高（~2-3 周，重构 trait + 5 个插件 + core + 提现） |
| 推荐人激励 | ⭐ 完美对称 |
| 公平性 | ⭐ 完美对称 |
| 向后兼容 | ❌ 破坏性变更，需全面迁移 |

#### 方案 B: Token → NEX 折算后走 NEX 管线

```
Token 订单金额 → 按汇率换算为 NEX 等值 → 走现有 NEX 佣金管线
```

**致命缺陷**：订单用 Token 支付，seller 收到的是 Token 不是 NEX。插件输出的 `CommissionOutput.amount` 是 NEX，但 entity_account 里没有对应的 NEX 可转。

除非：先把 Token 在 DEX 卖成 NEX → 引入外部依赖 + 滑点风险。**不可行。**

| 维度 | 评分 |
|---|---|
| 实现复杂度 | ❌ 需要 DEX 集成 |
| 推荐人激励 | ⭐ 对称 |
| 可行性 | ❌ 不可行 |

#### 方案 C: 仅 Token 池，独立 claim 入口

```
Token 佣金 100% → TokenPool
独立 claim_token_pool_reward() extrinsic
```

与方案 E 类似，但用独立的 extrinsic。UX 割裂（用户需要调用两个不同的 claim）。

| 维度 | 评分 |
|---|---|
| 实现复杂度 | ⭐ 简单 |
| 推荐人激励 | ❌ 同方案 E |
| UX | ❌ 割裂 |

#### 方案 D: 插件输出 Token，跳过记账/提现系统

```
Token 佣金 → 调用现有插件（Balance 重解释为 TokenBalance）
→ 每条 output 直接 Token 转账给 beneficiary（不进 pending）
→ 剩余入池
```

插件代码不变，但绕过 `credit_commission()` 和整个 pending/withdrawal 系统。
Token 佣金即时到账，不走冻结期/复购分流。

| 维度 | 评分 |
|---|---|
| 实现复杂度 | ⚠️ 中等（需要 core 新增 Token 调度路径，但不改插件 trait） |
| 推荐人激励 | ⭐ 对称 |
| 一致性 | ⚠️ Token 佣金即时到账 vs NEX 佣金有冻结期/复购，行为不对称 |
| 风险 | ⚠️ Balance 类型不匹配（NEX u128 vs Token u128 精度可能不同） |

---

### A.5 综合评估矩阵

| 方案 | 复杂度 | 推荐人激励 | 搭便车防护 | 兼容性 | 推荐 |
|---|---|---|---|---|---|
| **E (全入池)** | ⭐⭐⭐ | ❌ | ❌ | ⭐⭐⭐ | 仅适用于无推荐体系的 Entity |
| **F (直推+入池)** | ⭐⭐ | ✅ | ✅ | ⭐⭐⭐ | **⭐ 推荐** |
| A (全插件多资产) | 无 | ⭐⭐⭐ | ⭐⭐⭐ | 无 | 远期目标，短期不现实 |
| B (折算为 NEX) | — | — | — | — | ❌ 不可行 |
| C (独立 claim) | ⭐⭐⭐ | ❌ | ❌ | ⭐⭐ | 不推荐 |
| D (插件+即时到账) | ⭐ | ⭐⭐ | ⭐⭐ | ⭐ | 可行但行为不对称 |

---

### A.6 推荐结论：方案 F（混合分流）

**Token 佣金 = 直推推荐人奖 + 剩余入沉淀池**

理由：
1. **保留推荐人激励**：直推推荐人从 Token 订单仍可获得即时奖励，不会抵制 Token 支付
2. **实现极简**：不改 `CommissionPlugin` trait，不改 5 个插件，不改记账/提现系统。只需在 `process_token_commission` 中加 ~30 行代码
3. **经济合理**：Token 是内部货币，只做 1 层直推奖励符合其定位；多级/级差/团队业绩用 NEX 激励更合理
4. **向后兼容**：Entity 未启用 Token 时无任何影响
5. **可配置**：推荐人分成比例可由 Entity 配置（`token_referral_rate`），甚至可以设为 0 退化为方案 E

#### 对设计文档 §4.3.3 的修订

将 `process_token_commission` 从"全额入池"改为"直推分流 + 入池"：

```rust
pub fn process_token_commission(
    entity_id: u64,
    order_id: u64,
    buyer: &T::AccountId,
    token_order_amount: TokenBalanceOf<T>,
    max_commission_rate: u16,
) -> DispatchResult {
    let commission = token_order_amount
        .saturating_mul(max_commission_rate.into()) / 10000u32.into();

    let entity_account = T::EntityProvider::entity_account(entity_id);
    let asset_id = Self::entity_token_asset_id(entity_id);
    let mut pool_amount = commission;

    // ── 直推推荐人奖励（Token 即时到账） ──
    if let Some(referrer) = T::MemberProvider::get_referrer(entity_id, buyer) {
        let referral_rate = TokenReferralRate::<T>::get(entity_id); // 默认 0 = 全入池
        if referral_rate > 0 {
            let referrer_reward = commission
                .saturating_mul(referral_rate.into()) / 10000u32.into();
            if !referrer_reward.is_zero() {
                T::TokenTransfer::transfer(
                    asset_id, &entity_account, &referrer, referrer_reward,
                    Preservation::Preserve,
                )?;
                pool_amount = pool_amount.saturating_sub(referrer_reward);

                Self::deposit_event(Event::TokenReferralRewarded {
                    entity_id, order_id, referrer, amount: referrer_reward,
                });
            }
        }
    }

    // ── 剩余入沉淀池 ──
    if !pool_amount.is_zero() {
        UnallocatedTokenPool::<T>::mutate(entity_id, |pool| {
            *pool = pool.saturating_add(pool_amount);
        });
        Self::deposit_event(Event::TokenCommissionPooled {
            entity_id, order_id, amount: pool_amount,
        });
    }

    Ok(())
}
```

#### 新增配置项

```rust
/// Token 佣金中直推推荐人分成比例（基点）
/// 默认 0 = 全部入池；5000 = 50% 给推荐人
#[pallet::storage]
pub type TokenReferralRate<T: Config> = StorageMap<_, Blake2_128Concat, u64, u16, ValueQuery>;
```

#### 修订后的 Token 佣金流

```
Token 订单 10000 Token（等值 1000 NEX）
├── commission = 10000 × 10% = 1000 Token
├── TokenReferralRate = 5000（50%）
├── → 直推推荐人: 500 Token（即时到账）
└── → UnallocatedTokenPool: 500 Token（等额分配）

推荐人（level_1, 10人）: 500 + 500×50%/10 = 500 + 25 = 525 Token（≈52.5 NEX）
普通 level_1 会员: 25 Token（≈2.5 NEX）
```

**与 NEX 订单激励结构对齐** ✅
