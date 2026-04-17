# pallet-entity-shop

> 实体店铺管理模块 · Runtime Index `120` · Crate `pallet-entity-shop`

## 概述

`pallet-entity-shop` 管理 Entity（组织）下属的业务经营单元 -- Shop。每个 Entity 可拥有多个 Shop（受 `MaxShopsPerEntity` 约束），每个 Shop 拥有独立的运营资金账户（`PalletId` 派生）、管理员列表和生命周期状态。

> **积分系统已迁移**: Shop 积分（Points）相关的存储、Extrinsics 和事件已迁移至 `pallet-entity-loyalty` 模块。Shop 关闭时通过 `PointsCleanup` trait 委托 loyalty 模块清理积分数据。

```
Entity (组织层)                    Shop (业务层)
-------------------                -------------------
  所有权 / 治理                      日常运营
  代币发行 / 分红                    商品管理 -> entity-product
  KYC / 合规                         订单处理 -> entity-order
  组织金库                           运营资金 -> shop_account (派生)
  管理员权限                         门店管理员
       |                                  |
       +-- register_shop(entity_id) ------+
           unregister_shop(close)
```

## 权限模型

| 角色 | 来源 | 能力范围 |
|------|------|----------|
| Entity Owner | `EntityProvider::entity_owner` | 全部操作：创建/关闭/转让/提取资金/管理员增删/设主店/变更类型 |
| Entity Admin | `EntityProvider::is_entity_admin` (SHOP_MANAGE) | 管理类操作：更新信息/充值/暂停恢复/位置设置 |
| Shop Manager | `Shop.managers` 列表 | 同 Entity Admin |
| 普通用户 | 已签名账户 | `finalize_close_shop`（宽限期满后任何人可调用） |
| Root | `ensure_root` | 强制暂停/强制关闭/封禁/解封 |

> **Manager 权限** = Entity Owner + Entity Admin + Shop Managers

## 状态机

```
                fund_operating (余额恢复)
                      +------------+
                      v            |
  create_shop -> Active ---- FundDepleted
                  | ^
         pause    | | resume (需余额 >= MinOperatingBalance)
                  v |
                Paused
                  |
         close    |                  ban (Root, 需 reason)
                  v                       v
             Closing --- finalize --> Closed (终态)
                  ^                       ^
     cancel_close |                force_close (Root)
                  |
                任何非终态 -- ban_shop --> Banned -- unban_shop --> 恢复原状态
```

| 状态 | `is_operational` | `can_resume` | `is_terminal_or_banned` |
|------|:----------------:|:------------:|:-----------------------:|
| `Active` | Yes | -- | No |
| `Paused` | No | Yes (`resume_shop`) | No |
| `FundDepleted` | No | Yes (`fund_operating` 余额达标自动恢复) | No |
| `Closing` | No | Yes (`cancel_close_shop`) | No |
| `Closed` | No | No | Yes |
| `Banned` | No | Yes (`unban_shop`, 仅 Root) | Yes |

### EffectiveShopStatus

聚合 Entity 状态与 Shop 自身状态，供外部模块判断实际可用性：

| Entity 状态 x Shop 状态 | Effective |
|--------------------------|-----------|
| Active x Active | `Active` |
| Active x Paused | `PausedBySelf` |
| Suspended x * | `PausedByEntity` |
| Pending x * | `EntityNotReady` |
| Active x Closed | `Closed` |
| Active x Banned | `Banned` |

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
    pub location: Option<(i64, i64)>,              // 经纬度 x 10^6
    pub address_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub business_hours_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub policies_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub created_at: BlockNumber,
    pub product_count: u32,
    pub total_sales: Balance,
    pub total_orders: u32,
    pub rating: u16,         // 0-500 -> 0.0-5.0
    pub rating_total: u64,   // 累计 rating x 100
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

## Extrinsics（19 个）

| # | 函数签名 | 权限 | 说明 |
|:-:|----------|:----:|------|
| 0 | `create_shop(entity_id, name, shop_type, initial_fund)` | Owner | 创建 Shop，转入 initial_fund |
| 1 | `update_shop(shop_id, name?, logo_cid??, desc_cid??, hours_cid??, policies_cid??)` | Manager | 更新信息（三态 CID 语义） |
| 2 | `add_manager(shop_id, manager)` | Owner | 添加管理员 |
| 3 | `remove_manager(shop_id, manager)` | Owner | 移除管理员 |
| 4 | `fund_operating(shop_id, amount)` | Manager | 充值运营资金 |
| 5 | `pause_shop(shop_id)` | Manager | 暂停（仅 Active -> Paused） |
| 6 | `resume_shop(shop_id)` | Manager | 恢复（需余额 >= MinOperatingBalance） |
| 7 | `set_location(shop_id, location?, address_cid??)` | Manager | 设置地理位置 |
| 9 | `close_shop(shop_id)` | Owner | 发起关闭（-> Closing 宽限期） |
| 13 | `withdraw_operating_fund(shop_id, amount)` | Owner | 提取运营资金 |
| 15 | `finalize_close_shop(shop_id)` | 任何人 | 宽限期满后执行最终清理 |
| 19 | `transfer_shop(shop_id, to_entity_id)` | Owner | 转让 Shop 至另一 Entity |
| 20 | `set_primary_shop(entity_id, shop_id)` | Owner | 变更 Entity 主 Shop |
| 21 | `force_pause_shop(shop_id)` | Root | 强制暂停 |
| 24 | `force_close_shop(shop_id)` | Root | 强制关闭（跳过宽限期） |
| 27 | `set_shop_type(shop_id, shop_type)` | Owner | 变更 Shop 类型 |
| 28 | `cancel_close_shop(shop_id)` | Owner | 撤回关闭（Closing -> Active/FundDepleted） |
| 30 | `resign_manager(shop_id)` | 自身 Manager | 自我辞职 |
| 31 | `ban_shop(shop_id, reason)` | Root | 封禁（需提供原因） |
| 32 | `unban_shop(shop_id)` | Root | 解封（恢复封禁前状态） |

### 三态 CID 语义

`Option<Option<BoundedVec<u8>>>` 用于 CID 字段的更新：

| 值 | 含义 |
|----|------|
| `None` | 不修改 |
| `Some(None)` | 清除（unpin 旧 CID） |
| `Some(Some(cid))` | 设置新值（unpin 旧 -> pin 新） |

## Storage（7 项）

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Shops` | `Map<u64, Shop>` | Shop 主数据 |
| `ShopEntity` | `Map<u64, u64>` | shop_id -> entity_id 反向索引 |
| `NextShopId` | `Value<u64>` | 自增 ID（初始 1） |
| `EntityPrimaryShop` | `Map<u64, u64>` | entity_id -> primary_shop_id |
| `ShopClosingAt` | `Map<u64, BlockNumber>` | 关闭发起时间 |
| `ShopStatusBeforeBan` | `Map<u64, ShopOperatingStatus>` | 封禁前状态（解封时恢复） |
| `ShopBanReason` | `Map<u64, BoundedVec<u8>>` | 封禁原因 |

## Events（22 个）

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

## Errors（27 个）

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
| `InvalidLocation` | 经纬度超出范围 |
| `InvalidConfig` | 配置参数无效（全 None 更新等） |
| `InvalidRating` | 评分超出 1-5 范围 |
| `CannotClosePrimaryShop` | 不可关闭主 Shop |
| `CannotTransferPrimaryShop` | 不可转让主 Shop |
| `WithdrawBelowMinimum` | 提取后余额低于 MinOperatingBalance |
| `ZeroWithdrawAmount` | 提取金额为零 |
| `ZeroFundAmount` | 充值金额为零 |
| `ShopIdOverflow` | Shop ID 溢出 |
| `SameEntity` | 目标 Entity 与源相同 |
| `ShopTypeSame` | Shop 类型未变更 |
| `ShopLimitReached` | Entity 的 Shop 数量已达上限 |

## Runtime 配置

```rust
impl pallet_entity_shop::Config for Runtime {
    type Currency = Balances;
    type EntityProvider = EntityRegistry;
    type MaxShopNameLength = ConstU32<64>;
    type MaxCidLength = ConstU32<64>;
    type MaxManagers = ConstU32<10>;
    type MinOperatingBalance = ConstU128<{ UNIT / 10 }>;
    type WarningThreshold = ConstU128<{ UNIT }>;
    type CommissionFundGuard = CommissionCore;
    type ShopClosingGracePeriod = ConstU32<100800>;  // 7 days @ 6s/block
    type MaxShopsPerEntity = ConstU32<16>;
    type StoragePin = pallet_storage_service::Pallet<Runtime>;
    type ProductProvider = EntityProduct;
    type PointsCleanup = EntityLoyalty;               // 委托 loyalty 模块清理积分
    type WeightInfo = pallet_entity_shop::weights::SubstrateWeight<Runtime>;
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
    fn revert_shop_rating(shop_id: u64, old_rating: u8, new_rating: Option<u8>) -> DispatchResult;
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
    pub fn get_operating_balance(shop_id: u64) -> Balance;
}
```

## 安全机制

### 状态守卫

- **is_terminal_or_banned**: `add_manager`, `remove_manager`, `update_shop`, `set_location`, `set_shop_type`, `set_primary_shop` (目标 Shop)
- **is_banned 单独检查**: `withdraw_operating_fund`, `transfer_shop`, `fund_operating`, `close_shop`, `resume_shop`
- **is_entity_active**: `create_shop`, `update_shop`, `add_manager`, `fund_operating`, `set_location`, `set_shop_type`, `transfer_shop` (目标 Entity)
- **is_entity_locked**: 所有 Owner/Manager 权限的 extrinsics

### 资金保护

- **佣金保护**: `withdraw_operating_fund` 和 `deduct_operating_fund`（trait）均扣除 `CommissionFundGuard::protected_funds()` 后再检查可用余额
- **最低余额**: 活跃 Shop 提取运营资金后余额不得低于 `MinOperatingBalance`；已关闭 Shop 可全额提取
- **资金预警**: `deduct_operating_fund` 余额低于 `WarningThreshold` 时发射 `FundWarning`；低于 `MinOperatingBalance` 时自动切换为 `FundDepleted`

### 关闭清理 (do_close_shop_cleanup)

`finalize_close_shop` 和 `force_close_shop` 共用统一清理逻辑：

1. Unpin Shop 所有 CID（logo, description, address, business_hours, policies）
2. 级联移除关联 Product（退还押金 + unpin CID + 清理存储）
3. 设置状态为 `Closed`
4. 移除关闭计时器
5. 注销 Entity 关联 + 清理 `ShopEntity` 索引
6. 清理主 Shop 索引（若当前 Shop 是主 Shop）
7. 清理封禁相关存储（`ShopStatusBeforeBan`, `ShopBanReason`）
8. 委托 `T::PointsCleanup::cleanup_shop_points()` 清理积分数据
9. 退还剩余运营资金至 Entity Owner

### 主 Shop 保护

- 主 Shop 标识通过 `EntityPrimaryShop` 索引查询（单一数据源）
- `close_shop`, `transfer_shop` 拒绝操作主 Shop
- 关闭清理时自动移除主 Shop 索引

### 封禁联动

- `ban_shop` 封禁时自动调用 `ProductProvider::force_delist_all_shop_products()` 下架全部在售商品
- `unban_shop` 恢复封禁前状态（Active/Paused/FundDepleted）

## 外部依赖

| Trait | 提供方 | 用途 |
|-------|--------|------|
| `EntityProvider<AccountId>` | pallet-entity-registry | Entity 存在性/状态/权限/注册注销 |
| `CommissionFundGuard` | pallet-commission-core | 查询已承诺佣金资金 |
| `StoragePin<AccountId>` | pallet-storage-service | IPFS CID pin/unpin |
| `ProductProvider<AccountId, Balance>` | pallet-entity-product | 关闭时级联移除 Product / 封禁时下架商品 |
| `PointsCleanup` | pallet-entity-loyalty | Shop 关闭时清理积分数据 |

## 已知局限

| 项目 | 说明 |
|------|------|
| Weight | 使用基于 DB 读写次数的保守估计权重，待实际 benchmark 运行后替换为精确值 |
