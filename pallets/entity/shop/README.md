# pallet-entity-shop v0.1.2

> 实体店铺管理模块 | Runtime Index: 120

## 概述

`pallet-entity-shop` 是 Entity-Shop 分离架构的业务层，管理具体经营场所或线上店铺。每个 Shop 归属于一个 Entity（组织），一个 Entity 可拥有多个 Shop（受 `MaxShopsPerEntity` 上限约束）。

### 核心能力

- **多类型店铺** — 线上/线下/服务网点/仓储/加盟/快闪/虚拟
- **运营资金管理** — 独立 shop_account 派生账户，自动资金预警与耗尽暂停
- **管理员体系** — Entity Owner + Entity Admin + Shop Manager 三级权限
- **Shop 积分** — 独立积分系统（发放/销毁/转移/兑换），可配置返积分比例、兑换比例、TTL 有效期、总量上限
- **佣金保护** — `withdraw_operating_fund` / `deduct_operating_fund` / `redeem_points` 均保护已承诺的佣金资金
- **关闭流程** — 宽限期 (`Closing`) + 最终清理 (`finalize_close_shop`)，自动退还余额、注销关联、清理积分
- **封禁机制** — Root 可封禁/解封 Shop，封禁状态阻止所有业务操作

## 架构

```
Entity (组织层)                    Shop (业务层)
───────────────────                ───────────────────
• 所有权 / 治理                    • 日常运营
• 代币发行 / 分红                  • 商品管理 (entity-product)
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
       close    │                  ban (Root)
                ▼                      ▼
           Closing ─── finalize ─→ Closed (终态)
                ▲                      ▲
   cancel_close │              force_close (Root)
                │
              任何非终态 ── ban_shop ──→ Banned ── unban_shop ──→ 恢复原状态
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

### ShopOperatingStatus

| 状态 | 可运营 | 可恢复 | 终态 |
|------|:------:|:------:|:----:|
| `Active` | ✅ | — | ❌ |
| `Paused` | ❌ | ✅ `resume_shop` | ❌ |
| `FundDepleted` | ❌ | ✅ `fund_operating`（余额 ≥ MinOperatingBalance） | ❌ |
| `Closing` | ❌ | ✅ `cancel_close_shop` | ❌ |
| `Closed` | ❌ | ❌ | ✅ |
| `Banned` | ❌ | ✅ `unban_shop` (Root) | ❌ |

## 数据结构

### Shop

```rust
pub struct Shop<AccountId, Balance, BlockNumber, MaxNameLen, MaxCidLen, MaxManagers> {
    pub id: u64,
    pub entity_id: u64,
    pub is_primary: bool,
    pub name: BoundedVec<u8, MaxNameLen>,
    pub logo_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub description_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub shop_type: ShopType,
    pub status: ShopOperatingStatus,
    pub managers: BoundedVec<AccountId, MaxManagers>,
    pub customer_service: Option<AccountId>,
    pub initial_fund: Balance,
    pub location: Option<(i64, i64)>,             // 经纬度，精度 10^6
    pub address_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub business_hours_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub policies_cid: Option<BoundedVec<u8, MaxCidLen>>,
    pub created_at: BlockNumber,
    pub product_count: u32,
    pub total_sales: Balance,
    pub total_orders: u32,
    pub rating: u16,           // 评分 (0-500, 精度 ×100)
    pub rating_total: u64,     // 累计评分总和
    pub rating_count: u32,     // 评分次数
}
```

### PointsConfig

```rust
pub struct PointsConfig<MaxNameLen, MaxSymbolLen> {
    pub name: BoundedVec<u8, MaxNameLen>,
    pub symbol: BoundedVec<u8, MaxSymbolLen>,
    pub reward_rate: u16,      // 购物返积分比例（bps，500 = 5%）
    pub exchange_rate: u16,    // 积分兑换比例（bps，1000 = 10%）
    pub transferable: bool,
}
```

## Extrinsics

| Index | 函数 | 权限 | 说明 |
|:-----:|------|------|------|
| 0 | `create_shop(entity_id, name, shop_type, initial_fund)` | Entity Owner | 创建 Shop（转账 initial_fund → shop_account） |
| 1 | `update_shop(shop_id, name, logo_cid, description_cid)` | Manager | 更新名称/Logo/描述 |
| 2 | `add_manager(shop_id, manager)` | Entity Owner | 添加管理员 |
| 3 | `remove_manager(shop_id, manager)` | Entity Owner | 移除管理员 |
| 4 | `fund_operating(shop_id, amount)` | Manager | 充值运营资金 |
| 5 | `pause_shop(shop_id)` | Manager | 暂停营业 |
| 6 | `resume_shop(shop_id)` | Manager | 恢复营业（需余额 ≥ MinOperatingBalance） |
| 7 | `set_location(shop_id, location, address_cid, hours_cid)` | Manager | 设置地理位置信息 |
| 8 | `enable_points(shop_id, name, symbol, reward_rate, exchange_rate, transferable)` | Manager | 启用积分系统 |
| 9 | `close_shop(shop_id)` | Entity Owner | 发起关闭（进入 Closing 宽限期） |
| 10 | `disable_points(shop_id)` | Manager | 禁用积分（清理余额和供应量） |
| 11 | `update_points_config(shop_id, reward_rate, exchange_rate, transferable)` | Manager | 更新积分配置 |
| 12 | `transfer_points(shop_id, to, amount)` | 用户 | 转移积分（需 transferable，延长接收方有效期） |
| 13 | `withdraw_operating_fund(shop_id, amount)` | Entity Owner | 提取运营资金（佣金保护 + 最低余额检查） |
| 14 | `set_customer_service(shop_id, account)` | Manager | 设置/清除客服账户 |
| 15 | `finalize_close_shop(shop_id)` | 任何人 | 宽限期满后执行最终清理（退款 + 注销关联） |
| 16 | `manager_issue_points(shop_id, to, amount)` | Manager | 直接发放积分给用户 |
| 17 | `manager_burn_points(shop_id, from, amount)` | Manager | 直接销毁用户积分 |
| 18 | `redeem_points(shop_id, amount)` | 用户 | 积分兑换货币（佣金保护） |
| 19 | `transfer_shop(shop_id, to_entity_id)` | Entity Owner | 转让 Shop 到另一 Entity（检查目标上限） |
| 20 | `set_primary_shop(entity_id, shop_id)` | Entity Owner | 变更 Entity 主 Shop |
| 21 | `force_pause_shop(shop_id)` | Root | 强制暂停 Shop |
| 22 | `set_points_ttl(shop_id, ttl_blocks)` | Manager | 设置积分有效期（0 = 永不过期） |
| 23 | `expire_points(shop_id, account)` | 任何人 | 清除过期积分 |
| 24 | `force_close_shop(shop_id)` | Root | 强制关闭（跳过宽限期） |
| 25 | `set_business_hours(shop_id, cid)` | Manager | 设置营业时间 CID |
| 26 | `set_shop_policies(shop_id, cid)` | Manager | 设置店铺政策 CID |
| 27 | `set_shop_type(shop_id, shop_type)` | Entity Owner | 变更 Shop 类型 |
| 28 | `cancel_close_shop(shop_id)` | Entity Owner | 撤回关闭（Closing → Active/FundDepleted） |
| 29 | `set_points_max_supply(shop_id, max_supply)` | Manager | 设置积分总量上限（0 = 无上限） |
| 30 | `resign_manager(shop_id)` | Shop Manager | 管理员自我辞职 |
| 31 | `ban_shop(shop_id)` | Root | 封禁 Shop |
| 32 | `unban_shop(shop_id)` | Root | 解封 Shop（恢复封禁前状态） |

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
| `ShopClosingAt` | `StorageMap<u64, BlockNumber>` | 关闭发起时间（宽限期起点） |
| `ShopPointsTtl` | `StorageMap<u64, BlockNumber>` | 积分有效期（块数） |
| `ShopPointsExpiresAt` | `StorageDoubleMap<u64, AccountId, BlockNumber>` | 用户积分到期时间 |
| `ShopPointsMaxSupply` | `StorageMap<u64, Balance>` | 积分总量上限 |
| `EntityPrimaryShop` | `StorageMap<u64, u64>` | Entity → 主 Shop 映射 |
| `ShopStatusBeforeBan` | `StorageMap<u64, ShopOperatingStatus>` | 封禁前状态（解封时恢复） |

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
| `ShopClosed` | Shop 已关闭（终态） |
| `ShopClosing` | Shop 进入关闭宽限期 (grace_until) |
| `ShopCloseFinalized` | 宽限期满，关闭完成 |
| `ShopClosingCancelled` | 关闭撤回 |
| `ShopClosedFundRefunded` | 关闭时余额退还 |
| `ShopLocationUpdated` | 位置更新 |
| `ShopPointsEnabled` / `ShopPointsDisabled` | 积分启用/禁用 |
| `PointsIssued` / `PointsBurned` / `PointsTransferred` | 积分变动 |
| `PointsRedeemed` | 积分兑换货币 |
| `PointsTtlSet` | 积分有效期设置 |
| `PointsExpired` | 积分过期清除 |
| `PointsMaxSupplySet` | 积分总量上限设置 |
| `FundWarning` | 资金低于预警阈值 |
| `FundDepleted` | 资金耗尽，Shop 自动暂停 |
| `CustomerServiceUpdated` | 客服账户变更 |
| `ShopTransferred` | Shop 转让 |
| `PrimaryShopChanged` | 主 Shop 变更 |
| `ShopForcePaused` | Root 强制暂停 |
| `ShopForceClosedByRoot` | Root 强制关闭 |
| `ShopBusinessHoursUpdated` | 营业时间更新 |
| `ShopPoliciesUpdated` | 店铺政策更新 |
| `ShopTypeChanged` | Shop 类型变更 |
| `ManagerResigned` | 管理员辞职 |
| `ShopBannedByRoot` | Root 封禁 |
| `ShopUnbannedByRoot` | Root 解封 (restored_status) |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` / `EntityNotActive` | Entity 不存在/未激活 |
| `ShopNotFound` / `ShopNotActive` | Shop 不存在/未激活 |
| `NotAuthorized` / `NotManager` | 无权限/不是管理员 |
| `ShopNameEmpty` / `NameTooLong` | 名称为空/过长 |
| `ManagerAlreadyExists` / `ManagerNotFound` / `TooManyManagers` | 管理员错误 |
| `InsufficientOperatingFund` | 运营资金不足 |
| `ShopAlreadyPaused` / `ShopNotPaused` / `ShopAlreadyClosed` | 状态错误 |
| `ShopAlreadyClosing` / `ShopNotClosing` / `ClosingGracePeriodNotElapsed` | 关闭流程错误 |
| `ShopBanned` / `ShopNotBanned` | 封禁状态错误 |
| `PointsNotEnabled` / `PointsAlreadyEnabled` / `PointsNotTransferable` | 积分错误 |
| `InsufficientPointsBalance` / `PointsMaxSupplyExceeded` | 积分余额/上限错误 |
| `PointsNameEmpty` / `PointsNotExpired` / `RedeemPayoutZero` | 积分参数错误 |
| `InvalidLocation` / `InvalidConfig` / `EmptyCid` | 参数无效 |
| `CannotClosePrimaryShop` / `CannotTransferPrimaryShop` | 主 Shop 保护 |
| `WithdrawBelowMinimum` / `ZeroWithdrawAmount` / `ZeroFundAmount` | 资金操作限制 |
| `ShopIdOverflow` / `SameEntity` / `ShopTypeSame` | 杂项 |
| `EntityLocked` / `ShopLimitReached` | Entity 锁定/Shop 数量上限 |

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
    type MinOperatingBalance = ...;          // 最低运营余额
    type WarningThreshold = ...;             // 资金预警阈值
    type CommissionFundGuard = CommissionCore; // 佣金资金保护
    type ShopClosingGracePeriod = ...;       // 关闭宽限期（块数）
    type MaxShopsPerEntity = ConstU32<5>;    // 每 Entity 最大 Shop 数量
}
```

## ShopProvider Trait

供外部模块（transaction、service 等）查询和操作 Shop 状态。

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
    /// 获取 Shop 派生账户（PalletId + shop_id）
    pub fn shop_account_id(shop_id: u64) -> T::AccountId;
    /// 检查是否有管理权限（owner / admin / manager）
    pub fn can_manage_shop(shop: &Shop, account: &AccountId) -> bool;
    /// 发放积分（供外部模块调用，检查 Closed/Closing/Banned）
    pub fn issue_points(shop_id: u64, to: &AccountId, amount: Balance) -> DispatchResult;
    /// 销毁积分（供外部模块调用，检查 Closed/Banned，Closing 允许）
    pub fn burn_points(shop_id: u64, from: &AccountId, amount: Balance) -> DispatchResult;
    /// 获取运营资金余额
    pub fn get_operating_balance(shop_id: u64) -> Balance;
    /// 积分查询
    pub fn get_points_balance(shop_id: u64, account: &AccountId) -> Balance;
    pub fn get_points_total_supply(shop_id: u64) -> Balance;
    pub fn get_points_config(shop_id: u64) -> Option<PointsConfig>;
    pub fn get_points_max_supply(shop_id: u64) -> Balance;
}
```

## 安全机制

- **已关闭/封禁 Shop 全面阻止** — 管理类 extrinsics 使用 `is_terminal_or_banned()` 检查
- **佣金资金保护** — `withdraw_operating_fund`、`deduct_operating_fund`、`redeem_points` 均扣除 `CommissionFundGuard::protected_funds()` 后再检查余额
- **封禁保护** — `close_shop` 拒绝 Banned 状态（防止 owner 绕过封禁取回资金），积分操作拒绝 Banned
- **积分 TTL** — `transfer_points` 延长接收方有效期（防止通过转账绕过 TTL）
- **积分清理** — `disable_points` 和 `close_shop`/`force_close_shop` 均清理 PointsConfigs + PointsBalances + PointsTotalSupply + TTL/Expiry/MaxSupply
- **主 Shop 保护** — `CannotClosePrimaryShop` / `CannotTransferPrimaryShop` 阻止关闭或转让主 Shop
- **Shop 数量上限** — `create_shop` 和 `transfer_shop` 均检查 `MaxShopsPerEntity`
- **参数验证** — 名称/符号不能为空、reward_rate/exchange_rate ≤ 10000、位置经纬度范围校验、CID 非空
- **级联统计** — `update_shop_stats` 同时更新 Entity 层统计
- **评分精度** — rating_total 累积 × 100 精度，除以 rating_count 避免精度损失

## 已知技术债

| 项目 | 状态 | 说明 |
|------|------|------|
| Weight benchmarking | 🟡 占位 | 所有 extrinsic 使用硬编码占位值 |
| `RuntimeEvent` 弃用 | 🟡 待处理 | Config 中仍有 `type RuntimeEvent`（polkadot-sdk 自动追加） |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-02-05 | 初始版本：创建/更新/暂停/关闭/积分/位置/运营资金 |
| v0.1.1-audit | 2026-02-26 | 审计 Round 1-4 修复 (H2-H4, M1, M3, M5 等) |
| v0.1.2-audit | 2026-03 | 审计 Round 5 修复 (H1, M1-M4)，README 全面同步 |

### 审计修复详情 (v0.1.2-audit, Round 5)

- **H1**: `redeem_points` 新增佣金保护 (`CommissionFundGuard::protected_funds`)
- **M1**: `close_shop` 拒绝 Banned 状态（防止 owner 绕过封禁）
- **M2**: `transfer_points` 延长接收方积分有效期（防止绕过 TTL）
- **M3**: `transfer_shop` 检查目标 Entity 的 `MaxShopsPerEntity` 上限
- **M4**: `redeem_points`/`transfer_points`/`manager_burn_points`/`issue_points`/`burn_points` 统一封禁状态检查
- **L1**: README 全面同步（Shop struct、extrinsics、events、errors、storage、trait）

## 相关模块

- [pallet-entity-common](../common/) — 共享类型 + Trait（EntityProvider, ShopProvider, EffectiveShopStatus）
- [pallet-entity-registry](../registry/) — 实体管理（EntityProvider 实现方）
- [pallet-entity-product](../product/) — 商品管理（通过 ShopProvider 查询）
- [pallet-commission-core](../commission/core/) — 佣金管理（CommissionFundGuard 实现方）
