# pallet-entity-shop v0.1.1

> 实体店铺管理模块 | Runtime Index: 120

## 概述

`pallet-entity-shop` 是 Entity-Shop 分离架构的业务层，管理具体经营场所或线上店铺。每个 Shop 归属于一个 Entity（组织），一个 Entity 可拥有多个 Shop。

### 核心能力

- **多类型店铺** — 线上/线下/服务网点/仓储/加盟/快闪/虚拟
- **运营资金管理** — 独立 shop_account 派生账户，自动资金预警与耗尽暂停
- **管理员体系** — Entity Owner + Entity Admin + Shop Manager 三级权限
- **Shop 积分** — 独立积分系统（发放/销毁/转移），可配置返积分比例和兑换比例
- **佣金保护** — `withdraw_operating_fund` / `deduct_operating_fund` 均保护已承诺的佣金资金
- **关闭清理** — 关闭 Shop 时自动退还余额、注销 Entity 关联、清理积分数据

## 架构

```
Entity (组织层)                    Shop (业务层)
───────────────────                ───────────────────
• 所有权 / 治理                    • 日常运营
• 代币发行 / 分红                  • 商品管理 (entity-service)
• KYC / 合规                       • 订单处理 (entity-order)
• 组织金库                         • 运营资金 (shop_account)
• 管理员权限                       • 门店管理员
       │                                  │
       └── register_shop(entity_id) ──────┘
           unregister_shop(close)
```

### Shop 状态流转

```
              fund_operating (余额恢复)
                    ┌────────────┐
                    ▼            │
create_shop → Active ──── FundDepleted
                │ ▲
       pause    │ │ resume
                ▼ │
              Paused
                │
       close    │
                ▼
              Closed (不可恢复)
```

## 类型定义

### ShopType

| 类型 | 说明 | 需要位置 |
|------|------|:-------:|
| `OnlineStore` | 线上商城（默认） | ❌ |
| `PhysicalStore` | 实体门店 | ✅ |
| `ServicePoint` | 服务网点 | ✅ |
| `Warehouse` | 仓储/自提点 | ✅ |
| `Franchise` | 加盟店 | ❌ |
| `Popup` | 快闪店/临时店 | ✅ |
| `Virtual` | 虚拟店铺（纯服务） | ❌ |

### MemberMode

| 模式 | 说明 |
|------|------|
| `Inherit` | 继承 Entity 会员体系，所有 Shop 共享会员 |
| `Independent` | 独立会员体系，各 Shop 独立管理 |
| `Hybrid` | 混合模式，Entity + Shop 双层会员 |

### ShopOperatingStatus

| 状态 | 可运营 | 可恢复 |
|------|:------:|:------:|
| `Active` | ✅ | — |
| `Paused` | ❌ | ✅ `resume_shop` |
| `FundDepleted` | ❌ | ✅ `fund_operating`（余额 ≥ MinOperatingBalance） |
| `Closed` | ❌ | ❌ |

## 数据结构

### Shop

```rust
pub struct Shop<T: Config> {
    pub id: u64,
    pub entity_id: u64,
    pub is_primary: bool,
    pub name: BoundedVec<u8, MaxShopNameLength>,
    pub logo_cid: Option<BoundedVec<u8, MaxCidLength>>,
    pub description_cid: Option<BoundedVec<u8, MaxCidLength>>,
    pub shop_type: ShopType,
    pub status: ShopOperatingStatus,
    pub managers: BoundedVec<AccountId, MaxManagers>,
    pub customer_service: Option<BoundedVec<u8, MaxCidLength>>,
    pub initial_fund: Balance,
    pub member_mode: MemberMode,
    pub location: Option<(i64, i64)>,        // 经纬度，精度 10^6
    pub address_cid: Option<BoundedVec<u8, MaxCidLength>>,
    pub business_hours_cid: Option<BoundedVec<u8, MaxCidLength>>,
    pub created_at: BlockNumber,
    pub product_count: u32,
    pub total_sales: Balance,
    pub total_orders: u32,
    pub rating: u16,          // 评分 (0-500, 精度 ×100)
    pub rating_total: u64,    // 累计评分总和
    pub rating_count: u32,    // 评分次数
}
```

### PointsConfig

```rust
pub struct PointsConfig<T: Config> {
    pub name: BoundedVec<u8, MaxPointsNameLength>,
    pub symbol: BoundedVec<u8, MaxPointsSymbolLength>,
    pub reward_rate: u16,     // 购物返积分比例（bps，500 = 5%）
    pub exchange_rate: u16,   // 积分兑换比例（bps，1000 = 10%）
    pub transferable: bool,
    pub enabled: bool,
}
```

## Extrinsics

| Index | 函数 | 权限 | 说明 |
|:-----:|------|------|------|
| 0 | `create_shop(entity_id, name, shop_type, member_mode, initial_fund)` | Entity Owner | 创建 Shop（转账 initial_fund → shop_account） |
| 1 | `update_shop(shop_id, name, logo_cid, description_cid)` | Manager | 更新名称/Logo/描述 |
| 2 | `add_manager(shop_id, manager)` | Entity Owner | 添加管理员 |
| 3 | `remove_manager(shop_id, manager)` | Entity Owner | 移除管理员 |
| 4 | `fund_operating(shop_id, amount)` | Manager | 充值运营资金 |
| 5 | `pause_shop(shop_id)` | Manager | 暂停营业 |
| 6 | `resume_shop(shop_id)` | Manager | 恢复营业（需余额 ≥ MinOperatingBalance） |
| 7 | `set_location(shop_id, location, address_cid, hours_cid)` | Manager | 设置地理位置信息 |
| 8 | `enable_points(shop_id, name, symbol, reward_rate, exchange_rate, transferable)` | Manager | 启用积分系统 |
| 9 | `close_shop(shop_id)` | Entity Owner | 关闭 Shop（退还余额 + 清理积分 + 注销关联） |
| 10 | `disable_points(shop_id)` | Manager | 禁用积分（清理余额和供应量） |
| 11 | `update_points_config(shop_id, reward_rate, exchange_rate, transferable)` | Manager | 更新积分配置 |
| 12 | `transfer_points(shop_id, to, amount)` | 用户 | 转移积分（需 transferable） |
| 13 | `withdraw_operating_fund(shop_id, amount)` | Entity Owner | 提取运营资金（佣金保护 + 最低余额检查） |

> **Manager 权限** = Entity Owner ∪ Entity Admin ∪ Shop Managers 列表

## Storage

| 存储项 | 类型 | 说明 |
|--------|------|------|
| `Shops` | `StorageMap<u64, Shop>` | Shop 主数据 |
| `ShopEntity` | `StorageMap<u64, u64>` | Shop → Entity 反向索引 |
| `NextShopId` | `StorageValue<u64>` | 自增 Shop ID（从 1 开始） |
| `ShopPointsConfigs` | `StorageMap<u64, PointsConfig>` | 积分配置 |
| `ShopPointsBalances` | `StorageDoubleMap<u64, AccountId, Balance>` | 积分余额 |
| `ShopPointsTotalSupply` | `StorageMap<u64, Balance>` | 积分总供应量 |

## Events

| 事件 | 说明 |
|------|------|
| `ShopCreated` | Shop 已创建 (shop_id, entity_id, name, shop_type) |
| `ShopUpdated` | Shop 信息已更新 |
| `ManagerAdded` / `ManagerRemoved` | 管理员变更 |
| `OperatingFundDeposited` | 运营资金充值 |
| `OperatingFundDeducted` | 运营资金扣减（trait 调用） |
| `OperatingFundWithdrawn` | 运营资金提取 |
| `ShopPaused` / `ShopResumed` | 暂停/恢复 |
| `ShopClosed` | Shop 已关闭 |
| `ShopClosedFundRefunded` | 关闭时余额退还 |
| `ShopLocationUpdated` | 位置更新 |
| `ShopPointsEnabled` / `ShopPointsDisabled` | 积分启用/禁用 |
| `PointsIssued` / `PointsBurned` / `PointsTransferred` | 积分变动 |
| `FundWarning` | 资金低于预警阈值 |
| `FundDepleted` | 资金耗尽，Shop 自动暂停 |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` / `EntityNotActive` | Entity 不存在/未激活 |
| `ShopNotFound` / `ShopNotActive` | Shop 不存在/未激活 |
| `NotAuthorized` | 无权限 |
| `ShopNameEmpty` / `NameTooLong` | 名称为空/过长 |
| `ManagerAlreadyExists` / `ManagerNotFound` / `TooManyManagers` | 管理员错误 |
| `InsufficientBalance` / `InsufficientOperatingFund` | 余额不足 |
| `ShopAlreadyPaused` / `ShopNotPaused` / `ShopAlreadyClosed` | 状态错误 |
| `PointsNotEnabled` / `PointsAlreadyEnabled` / `PointsNotTransferable` | 积分错误 |
| `InsufficientPointsBalance` | 积分余额不足 |
| `InvalidLocation` / `InvalidConfig` | 参数无效 |
| `CannotClosePrimaryShop` | 主 Shop 不可关闭 |
| `WithdrawBelowMinimum` / `ZeroWithdrawAmount` | 提取限制 |

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
    type MinOperatingBalance = ...;       // 最低运营余额
    type WarningThreshold = ...;          // 资金预警阈值
    type CommissionFundGuard = CommissionCore; // 佣金资金保护
}
```

## ShopProvider Trait

供外部模块（transaction、service 等）查询和操作 Shop 状态。

```rust
pub trait ShopProvider<AccountId> {
    fn shop_exists(shop_id: u64) -> bool;
    fn is_shop_active(shop_id: u64) -> bool;       // Shop + Entity 均 active
    fn shop_entity_id(shop_id: u64) -> Option<u64>;
    fn shop_owner(shop_id: u64) -> Option<AccountId>;
    fn shop_account(shop_id: u64) -> AccountId;     // 派生账户
    fn shop_type(shop_id: u64) -> Option<ShopType>;
    fn shop_member_mode(shop_id: u64) -> MemberMode;
    fn is_shop_manager(shop_id: u64, account: &AccountId) -> bool;
    fn is_primary_shop(shop_id: u64) -> bool;
    fn shop_own_status(shop_id: u64) -> Option<ShopOperatingStatus>;
    fn effective_status(shop_id: u64) -> Option<EffectiveShopStatus>;
    fn update_shop_stats(shop_id: u64, sales: u128, orders: u32) -> DispatchResult;
    fn update_shop_rating(shop_id: u64, rating: u8) -> DispatchResult;
    fn deduct_operating_fund(shop_id: u64, amount: u128) -> DispatchResult;
    fn operating_balance(shop_id: u64) -> u128;
    fn create_primary_shop(entity_id: u64, name: Vec<u8>, ...) -> Result<u64, DispatchError>;
    fn pause_shop(shop_id: u64) -> DispatchResult;
    fn resume_shop(shop_id: u64) -> DispatchResult;
    fn force_close_shop(shop_id: u64) -> DispatchResult;
}
```

### 公共辅助函数

```rust
impl<T: Config> Pallet<T> {
    /// 获取 Shop 派生账户（PalletId + shop_id）
    pub fn shop_account_id(shop_id: u64) -> T::AccountId;
    /// 检查是否有管理权限（owner / admin / manager）
    pub fn can_manage_shop(shop: &Shop, account: &AccountId) -> bool;
    /// 发放积分（供外部模块调用）
    pub fn issue_points(shop_id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    /// 销毁积分（供外部模块调用）
    pub fn burn_points(shop_id: u64, from: &AccountId, amount: Balance) -> DispatchResult;
    /// 获取运营资金余额
    pub fn get_operating_balance(shop_id: u64) -> Balance;
}
```

## 安全机制

- **已关闭 Shop 全面阻止** — update/add_manager/remove_manager/fund_operating/set_location/enable_points/deduct_operating_fund 均检查 `ShopAlreadyClosed`
- **佣金资金保护** — `withdraw_operating_fund` 和 `deduct_operating_fund` 均扣除 `CommissionFundGuard::protected_funds()` 后再检查余额
- **积分清理** — `disable_points` 和 `close_shop`/`force_close_shop` 均清理 PointsConfigs + PointsBalances + PointsTotalSupply
- **主 Shop 保护** — `CannotClosePrimaryShop` 阻止关闭主 Shop
- **参数验证** — 名称/符号不能为空、reward_rate/exchange_rate ≤ 10000、位置经纬度范围校验
- **级联统计** — `update_shop_stats` 同时更新 Entity 层统计
- **评分精度** — rating_total 累积 × 100 精度，除以 rating_count 避免精度损失

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Weight benchmarking | 🟡 占位 | 所有 extrinsic 使用硬编码占位值 |
| `RuntimeEvent` 弃用 | 🟡 待处理 | Config 中仍有 `type RuntimeEvent`（polkadot-sdk 自动追加） |
| mock.rs 未用导入 | 🟡 低优 | `ConstU32` / `ConstU64` 未使用 |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-05 | 初始版本：创建/更新/暂停/关闭/积分/位置/运营资金 |
| v0.1.1-audit | 2026-02-26 | 深度审计修复 (H2-H4, M1, M3, M5) |

### 审计修复详情 (v0.1.1-audit)

- **H2**: `disable_points` 清理积分余额和总供应量，防止残留数据
- **H3**: `add_manager`/`remove_manager`/`update_shop`/`fund_operating` 拒绝已关闭 Shop
- **H4**: `deduct_operating_fund`（trait）拒绝已关闭 Shop
- **M1**: `enable_points` 拒绝空名称和空符号
- **M3**: `close_shop`/`force_close_shop` 清理积分数据
- **M5**: `issue_points`/`burn_points` 拒绝已关闭 Shop

## 相关模块

- [pallet-entity-common](../common/) — 共享类型 + Trait（EntityProvider, ShopProvider）
- [pallet-entity-registry](../registry/) — 实体管理（EntityProvider 实现方）
- [pallet-entity-service](../service/) — 商品管理（通过 ShopProvider 查询）
- [pallet-commission-core](../commission/core/) — 佣金管理（CommissionFundGuard 实现方）
