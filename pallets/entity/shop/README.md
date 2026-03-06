# pallet-entity-shop

> 实体店铺管理模块 · Runtime Index `120` · Crate `pallet-entity-shop`

## 概述

`pallet-entity-shop` 管理 Entity（组织）下属的业务经营单元 —— Shop。每个 Entity 可拥有多个 Shop（受 `MaxShopsPerEntity` 约束），每个 Shop 拥有独立的运营资金账户（`PalletId` 派生）、管理员列表、积分系统和生命周期状态。

```
Entity (组织层)                    Shop (业务层)
───────────────────                ───────────────────
• 所有权 / 治理                    • 日常运营
• 代币发行 / 分红                  • 商品管理 → entity-product
• KYC / 合规                       • 订单处理 → entity-order
• 组织金库                         • 运营资金 → shop_account (派生)
• 管理员权限                       • 门店管理员
       │                                  │
       └── register_shop(entity_id) ──────┘
           unregister_shop(close)
```

## 权限模型

| 角色 | 来源 | 能力范围 |
|------|------|----------|
| Entity Owner | `EntityProvider::entity_owner` | 全部操作：创建/关闭/转让/提取资金/管理员增删/设主店 |
| Entity Admin | `EntityProvider::is_entity_admin` (SHOP_MANAGE) | 管理类操作：更新信息/充值/暂停恢复/积分管理/位置设置 |
| Shop Manager | `Shop.managers` 列表 | 同 Entity Admin |
| 普通用户 | 已签名账户 | 积分转移/兑换/过期清除 |
| Root | `ensure_root` | 强制暂停/强制关闭/封禁/解封 |

> **Manager 权限** = Entity Owner ∪ Entity Admin ∪ Shop Managers

## 状态机

```
                fund_operating (余额恢复)
                      ┌────────────┐
                      ▼            │
  create_shop → Active ──── FundDepleted
                  │ ▲
         pause    │ │ resume (需余额 ≥ MinOperatingBalance)
                  ▼ │
                Paused
                  │
         close    │                  ban (Root, 需 reason)
                  ▼                       ▼
             Closing ─── finalize ──→ Closed (终态)
                  ▲                       ▲
     cancel_close │                force_close (Root)
                  │
                任何非终态 ── ban_shop ──→ Banned ── unban_shop ──→ 恢复原状态
```

| 状态 | `is_operational` | `can_resume` | `is_terminal_or_banned` |
|------|:----------------:|:------------:|:-----------------------:|
| `Active` | ✅ | — | ❌ |
| `Paused` | ❌ | ✅ (`resume_shop`) | ❌ |
| `FundDepleted` | ❌ | ✅ (`fund_operating` 余额达标自动恢复) | ❌ |
| `Closing` | ❌ | ✅ (`cancel_close_shop`) | ❌ |
| `Closed` | ❌ | ❌ | ✅ |
| `Banned` | ❌ | ✅ (`unban_shop`, 仅 Root) | ✅ |

### EffectiveShopStatus

聚合 Entity 状态与 Shop 自身状态，供外部模块判断实际可用性：

| Entity 状态 × Shop 状态 | Effective |
|--------------------------|-----------|
| Active × Active | `Active` |
| Active × Paused | `PausedBySelf` |
| Suspended × * | `PausedByEntity` |
| Pending × * | `EntityNotReady` |
| Active × Closed | `Closed` |
| Active × Banned | `Banned` |

## 数据结构

### Shop

```rust
pub struct Shop<AccountId, Balance, BlockNumber, MaxNameLen, MaxCidLen, MaxManagers> {
    pub id: u64,
    pub entity_id: u64,
    pub name: BoundedVec<u8, MaxNameLen>,
    pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub shop_type: ShopType,
    pub status: ShopOperatingStatus,
    pub managers: BoundedVec<AccountId, MaxManagers>,
    pub location: Option<(i64, i64)>,              // 经纬度 × 10^6
    pub address_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub business_hours_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub policies_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub created_at: BlockNumber,
    pub product_count: u32,
    pub total_sales: Balance,
    pub total_orders: u32,
    pub rating: u16,         // 0–500 → 0.0–5.0
    pub rating_total: u64,   // 累计 rating × 100
    pub rating_count: u32,
}
```

### ShopType

| 类型 | 说明 |
|------|------|
| `OnlineStore` | 线上商城（默认） |
| `PhysicalStore` | 实体门店 |
| `ServicePoint` | 服务网点 |
| `Warehouse` | 仓储/自提点 |
| `Franchise` | 加盟店 |
| `Popup` | 快闪店/临时店 |
| `Virtual` | 虚拟店铺（纯服务） |

### PointsConfig

```rust
pub struct PointsConfig<MaxNameLen, MaxSymbolLen> {
    pub name: BoundedVec<u8, MaxNameLen>,
    pub symbol: BoundedVec<u8, MaxSymbolLen>,
    pub reward_rate: u16,     // bps, 500 = 5%
    pub exchange_rate: u16,   // bps, 1000 = 10%
    pub transferable: bool,
}
```

## Extrinsics

| # | 函数签名 | 权限 | 说明 |
|:-:|----------|:----:|------|
| 0 | `create_shop(entity_id, name, shop_type, initial_fund)` | Owner | 创建 Shop，转入 initial_fund |
| 1 | `update_shop(shop_id, name?, logo_cid??, desc_cid??, hours_cid??, policies_cid??)` | Manager | 更新信息（三态 CID 语义） |
| 2 | `add_manager(shop_id, manager)` | Owner | 添加管理员 |
| 3 | `remove_manager(shop_id, manager)` | Owner | 移除管理员 |
| 4 | `fund_operating(shop_id, amount)` | Manager | 充值运营资金 |
| 5 | `pause_shop(shop_id)` | Manager | 暂停（仅 Active → Paused） |
| 6 | `resume_shop(shop_id)` | Manager | 恢复（需余额 ≥ MinOperatingBalance） |
| 7 | `set_location(shop_id, location?, address_cid??)` | Manager | 设置地理位置 |
| 8 | `enable_points(shop_id, name, symbol, reward_rate, exchange_rate, transferable)` | Manager | 启用积分系统 |
| 9 | `close_shop(shop_id)` | Owner | 发起关闭（→ Closing 宽限期） |
| 10 | `disable_points(shop_id)` | Manager | 禁用积分（清理所有余额） |
| 11 | `update_points_config(shop_id, reward_rate?, exchange_rate?, transferable?)` | Manager | 更新积分参数 |
| 12 | `transfer_points(shop_id, to, amount)` | 用户 | 转移积分（需 transferable） |
| 13 | `withdraw_operating_fund(shop_id, amount)` | Owner | 提取运营资金 |
| 15 | `finalize_close_shop(shop_id)` | 任何人 | 宽限期满后执行最终清理 |
| 16 | `manager_issue_points(shop_id, to, amount)` | Manager | 直接发放积分 |
| 17 | `manager_burn_points(shop_id, from, amount)` | Manager | 直接销毁积分 |
| 18 | `redeem_points(shop_id, amount)` | 用户 | 积分兑换货币 |
| 19 | `transfer_shop(shop_id, to_entity_id)` | Owner | 转让 Shop 至另一 Entity |
| 20 | `set_primary_shop(entity_id, shop_id)` | Owner | 变更 Entity 主 Shop |
| 21 | `force_pause_shop(shop_id)` | Root | 强制暂停 |
| 22 | `set_points_ttl(shop_id, ttl_blocks)` | Manager | 设置积分有效期（0=永不过期） |
| 23 | `expire_points(shop_id, account)` | 任何人 | 清除过期积分 |
| 24 | `force_close_shop(shop_id)` | Root | 强制关闭（跳过宽限期） |
| 27 | `set_shop_type(shop_id, shop_type)` | Owner | 变更 Shop 类型 |
| 28 | `cancel_close_shop(shop_id)` | Owner | 撤回关闭（Closing → Active/FundDepleted） |
| 29 | `set_points_max_supply(shop_id, max_supply)` | Manager | 设置积分总量上限（0=无上限） |
| 30 | `resign_manager(shop_id)` | 自身 Manager | 自我辞职 |
| 31 | `ban_shop(shop_id, reason)` | Root | 封禁（需提供原因） |
| 32 | `unban_shop(shop_id)` | Root | 解封（恢复封禁前状态） |

### 三态 CID 语义

`Option<Option<BoundedVec<u8>>>` 用于 CID 字段的更新：

| 值 | 含义 |
|----|------|
| `None` | 不修改 |
| `Some(None)` | 清除（unpin 旧 CID） |
| `Some(Some(cid))` | 设置新值（unpin 旧 → pin 新） |

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Shops` | `Map<u64, Shop>` | Shop 主数据 |
| `ShopEntity` | `Map<u64, u64>` | shop_id → entity_id 反向索引 |
| `NextShopId` | `Value<u64>` | 自增 ID（初始 1） |
| `EntityPrimaryShop` | `Map<u64, u64>` | entity_id → primary_shop_id |
| `ShopClosingAt` | `Map<u64, BlockNumber>` | 关闭发起时间 |
| `ShopStatusBeforeBan` | `Map<u64, ShopOperatingStatus>` | 封禁前状态（解封时恢复） |
| `ShopBanReason` | `Map<u64, BoundedVec<u8>>` | 封禁原因 |
| `ShopPointsConfigs` | `Map<u64, PointsConfig>` | 积分配置 |
| `ShopPointsBalances` | `DoubleMap<u64, AccountId, Balance>` | 积分余额 |
| `ShopPointsTotalSupply` | `Map<u64, Balance>` | 积分总供应量 |
| `ShopPointsMaxSupply` | `Map<u64, Balance>` | 积分总量上限 |
| `ShopPointsTtl` | `Map<u64, BlockNumber>` | 积分有效期（块数） |
| `ShopPointsExpiresAt` | `DoubleMap<u64, AccountId, BlockNumber>` | 用户积分到期时间 |

## Events

| 事件 | 字段 |
|------|------|
| `ShopCreated` | shop_id, entity_id, name, shop_type |
| `ShopUpdated` | shop_id |
| `ManagerAdded` | shop_id, manager |
| `ManagerRemoved` | shop_id, manager |
| `ManagerResigned` | shop_id, manager |
| `OperatingFundDeposited` | shop_id, amount, new_balance |
| `OperatingFundDeducted` | shop_id, amount, new_balance |
| `OperatingFundWithdrawn` | shop_id, to, amount, new_balance |
| `ShopPaused` | shop_id |
| `ShopResumed` | shop_id |
| `ShopClosing` | shop_id, grace_until |
| `ShopClosingCancelled` | shop_id |
| `ShopCloseFinalized` | shop_id |
| `ShopClosed` | shop_id |
| `ShopClosedFundRefunded` | shop_id, to, amount |
| `ShopLocationUpdated` | shop_id, location |
| `ShopTypeChanged` | shop_id, old_type, new_type |
| `ShopTransferred` | shop_id, from_entity_id, to_entity_id |
| `PrimaryShopChanged` | entity_id, old_shop_id, new_shop_id |
| `ShopForcePaused` | shop_id |
| `ShopForceClosedByRoot` | shop_id |
| `ShopBannedByRoot` | shop_id, reason |
| `ShopUnbannedByRoot` | shop_id, restored_status |
| `FundWarning` | shop_id, balance |
| `FundDepleted` | shop_id |
| `ShopPointsEnabled` | shop_id, name |
| `ShopPointsDisabled` | shop_id |
| `PointsConfigUpdated` | shop_id |
| `PointsIssued` | shop_id, to, amount |
| `PointsBurned` | shop_id, from, amount |
| `PointsTransferred` | shop_id, from, to, amount |
| `PointsRedeemed` | shop_id, who, points_burned, payout |
| `PointsTtlSet` | shop_id, ttl_blocks |
| `PointsExpired` | shop_id, account, amount |
| `PointsMaxSupplySet` | shop_id, max_supply |

## Errors

| 错误 | 触发场景 |
|------|----------|
| `EntityNotFound` | Entity 不存在 |
| `EntityNotActive` | Entity 未激活（Suspended/Pending） |
| `EntityLocked` | Entity 处于全局锁定 |
| `ShopNotFound` | shop_id 无效 |
| `ShopNotActive` | Shop 非 Active 状态（pause 需 Active） |
| `NotAuthorized` | 调用者无权限 |
| `NotManager` | 调用者不在 managers 列表中（resign_manager） |
| `ShopNameEmpty` | 名称为空 |
| `NameTooLong` | 名称超过 MaxShopNameLength |
| `EmptyCid` | CID 内容为空 |
| `ManagerAlreadyExists` | 管理员已存在 |
| `ManagerNotFound` | 管理员不存在 |
| `TooManyManagers` | 超过 MaxManagers 上限 |
| `InsufficientOperatingFund` | 运营资金不足 |
| `ShopAlreadyPaused` | 已暂停 |
| `ShopNotPaused` | 未暂停（resume 需 Paused/FundDepleted） |
| `ShopAlreadyClosed` | 已关闭或已封禁（管理类操作拒绝） |
| `ShopAlreadyClosing` | 已在关闭宽限期中 |
| `ShopNotClosing` | 未在关闭宽限期中（finalize/cancel 需 Closing） |
| `ClosingGracePeriodNotElapsed` | 宽限期未满 |
| `ShopBanned` | Shop 已被封禁 |
| `ShopNotBanned` | Shop 未被封禁（unban 需 Banned） |
| `PointsNotEnabled` | 积分未启用 |
| `PointsAlreadyEnabled` | 积分已启用 |
| `PointsNotTransferable` | 积分不可转让 |
| `InsufficientPointsBalance` | 积分余额不足 |
| `PointsNameEmpty` | 积分名称为空 |
| `PointsNotExpired` | 积分未过期（expire_points 需过期） |
| `PointsMaxSupplyExceeded` | 超过积分总量上限 |
| `RedeemPayoutZero` | 兑换金额为零（积分数量过小） |
| `InvalidLocation` | 经纬度超出范围 |
| `InvalidConfig` | 配置参数无效（rate > 10000、全 None 更新等） |
| `InvalidRating` | 评分超出 1-5 范围 |
| `CannotClosePrimaryShop` | 不可关闭主 Shop |
| `CannotTransferPrimaryShop` | 不可转让主 Shop |
| `WithdrawBelowMinimum` | 提取后余额低于 MinOperatingBalance |
| `ZeroWithdrawAmount` | 提取金额为零 |
| `ZeroFundAmount` | 充值金额为零 |
| `ShopIdOverflow` | Shop ID 溢出 |
| `SameEntity` | 目标 Entity 与源相同 / 积分自转账 |
| `ShopTypeSame` | Shop 类型未变更 |
| `ShopLimitReached` | Entity 的 Shop 数量已达上限 |

## Runtime 配置

```rust
impl pallet_entity_shop::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EntityProvider = EntityRegistry;
    type MaxShopNameLength = ConstU32<64>;
    type MaxCidLength = ConstU32<64>;
    type MaxManagers = ConstU32<10>;
    type MaxPointsNameLength = ConstU32<32>;
    type MaxPointsSymbolLength = ConstU32<8>;
    type MinOperatingBalance = MinOperatingBalance;
    type WarningThreshold = WarningThreshold;
    type CommissionFundGuard = CommissionCore;
    type ShopClosingGracePeriod = ShopClosingGracePeriod;
    type MaxShopsPerEntity = ConstU32<5>;
    type StoragePin = StorageService;
    type ProductProvider = EntityProduct;
}
```

### integrity_test 约束

- `MaxManagers >= 1`
- `MaxShopNameLength >= 1`
- `MaxCidLength >= 1`
- `MaxShopsPerEntity >= 1`
- `WarningThreshold >= MinOperatingBalance`

## ShopProvider Trait

供外部模块（entity-order, entity-product 等）查询和操作 Shop。

```rust
pub trait ShopProvider<AccountId> {
    fn shop_exists(shop_id: u64) -> bool;
    fn is_shop_active(shop_id: u64) -> bool;
    fn shop_entity_id(shop_id: u64) -> Option<u64>;
    fn shop_owner(shop_id: u64) -> Option<AccountId>;
    fn shop_account(shop_id: u64) -> AccountId;
    fn shop_type(shop_id: u64) -> Option<ShopType>;
    fn is_shop_manager(shop_id: u64, account: &AccountId) -> bool;
    fn is_primary_shop(shop_id: u64) -> bool;
    fn shop_own_status(shop_id: u64) -> Option<ShopOperatingStatus>;
    fn effective_status(shop_id: u64) -> Option<EffectiveShopStatus>;
    fn update_shop_stats(shop_id: u64, sales: u128, orders: u32) -> DispatchResult;
    fn update_shop_rating(shop_id: u64, rating: u8) -> DispatchResult;
    fn deduct_operating_fund(shop_id: u64, amount: u128) -> DispatchResult;
    fn operating_balance(shop_id: u64) -> u128;
    fn create_primary_shop(entity_id: u64, name: Vec<u8>, shop_type: ShopType) -> Result<u64, DispatchError>;
    fn pause_shop(shop_id: u64) -> DispatchResult;
    fn resume_shop(shop_id: u64) -> DispatchResult;
    fn force_close_shop(shop_id: u64) -> DispatchResult;
    fn force_pause_shop(shop_id: u64) -> DispatchResult;
    fn increment_product_count(shop_id: u64) -> DispatchResult;
    fn decrement_product_count(shop_id: u64) -> DispatchResult;
}
```

### 公共辅助函数

```rust
impl<T: Config> Pallet<T> {
    pub fn shop_account_id(shop_id: u64) -> T::AccountId;
    pub fn can_manage_shop(shop: &Shop, account: &AccountId) -> bool;
    pub fn issue_points(shop_id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    pub fn burn_points(shop_id: u64, from: &AccountId, amount: Balance) -> DispatchResult;
    pub fn get_operating_balance(shop_id: u64) -> Balance;
    pub fn get_points_balance(shop_id: u64, account: &AccountId) -> Balance;
    pub fn get_points_total_supply(shop_id: u64) -> Balance;
    pub fn get_points_config(shop_id: u64) -> Option<PointsConfig>;
    pub fn get_points_expiry(shop_id: u64, account: &AccountId) -> Option<BlockNumber>;
    pub fn get_points_max_supply(shop_id: u64) -> Balance;
}
```

## 安全机制

### 状态守卫

- **is_terminal_or_banned**: `add_manager`, `remove_manager`, `update_shop`, `enable_points`, `disable_points`, `update_points_config`, `set_location`, `set_shop_type`, `set_points_ttl`, `set_points_max_supply`, `manager_issue_points`, `set_primary_shop` (目标 Shop)
- **is_banned 单独检查**: `withdraw_operating_fund`, `transfer_shop`, `fund_operating`, `close_shop`, `resume_shop`, `transfer_points`, `redeem_points`, `manager_burn_points`, `deduct_operating_fund` (trait), `resume_shop` (trait)
- **is_entity_active**: `create_shop`, `update_shop`, `add_manager`, `fund_operating`, `set_location`, `enable_points`, `disable_points`, `update_points_config`, `set_shop_type`, `set_points_ttl`, `set_points_max_supply`, `manager_issue_points`, `manager_burn_points`, `transfer_shop` (目标 Entity)
- **is_entity_locked**: 所有 Owner/Manager 权限的 extrinsics

### 资金保护

- **佣金保护**: `withdraw_operating_fund`, `deduct_operating_fund`, `redeem_points` 均扣除 `CommissionFundGuard::protected_funds()` 后再检查可用余额
- **最低余额**: 活跃 Shop 提取运营资金后余额不得低于 `MinOperatingBalance`；已关闭 Shop 可全额提取
- **资金预警**: `deduct_operating_fund` 余额低于 `WarningThreshold` 时发射 `FundWarning`；低于 `MinOperatingBalance` 时自动切换为 `FundDepleted`

### 积分安全

- **TTL 防绕过**: `transfer_points` 延长接收方有效期（滑动窗口取最大值）
- **懒过期**: `transfer_points`, `manager_burn_points`, `redeem_points` 在操作前检查并清除过期积分
- **总量上限**: `issue_points`, `manager_issue_points` 检查 `ShopPointsMaxSupply`
- **评分校验**: `update_shop_rating` 严格限制 1-5 范围

### 关闭清理 (do_close_shop_cleanup)

`finalize_close_shop` 和 `force_close_shop` 共用统一清理逻辑：

1. Unpin Shop 所有 CID（logo, description, address, business_hours, policies）
2. 级联 unpin 关联 Product CID
3. 设置状态为 `Closed`
4. 移除关闭计时器
5. 注销 Entity 关联 + 清理 `ShopEntity` 索引
6. 清理主 Shop 索引（若当前 Shop 是主 Shop）
7. 清理封禁相关存储（`ShopStatusBeforeBan`, `ShopBanReason`）
8. 清理全部积分数据（config, balances, total_supply, ttl, expires_at, max_supply）
9. 退还剩余运营资金至 Entity Owner

### 主 Shop 保护

- 主 Shop 标识通过 `EntityPrimaryShop` 索引查询（单一数据源）
- `close_shop`, `transfer_shop` 拒绝操作主 Shop
- 关闭清理时自动移除主 Shop 索引

## 外部依赖

| Trait | 提供方 | 用途 |
|-------|--------|------|
| `EntityProvider<AccountId>` | pallet-entity-registry | Entity 存在性/状态/权限/注册注销 |
| `CommissionFundGuard` | pallet-commission-core | 查询已承诺佣金资金 |
| `StoragePin<AccountId>` | pallet-storage-service | IPFS CID pin/unpin |
| `ProductProvider<AccountId, Balance>` | pallet-entity-product | 关闭时级联 unpin Product CID |

## 已知局限

| 项目 | 说明 |
|------|------|
| Weight | 所有 extrinsic 使用硬编码占位值，需 benchmarking |
| clear_prefix | `disable_points` 和 `do_close_shop_cleanup` 使用 `u32::MAX` 清理积分数据，大量用户时可能超出区块权重 |
| unregister_shop 错误忽略 | `do_close_shop_cleanup` 中使用 `let _` 忽略 `unregister_shop` 返回值 |
