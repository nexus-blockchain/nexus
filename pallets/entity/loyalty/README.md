# pallet-entity-loyalty

> NEXUS Entity 会员忠诚度模块 — 统一管理 Shop 积分系统 + NEX 购物余额 + Token 奖励/折扣入口 | Runtime Index: 139

## 概述

`pallet-entity-loyalty` 是 NEXUS Entity 会员忠诚度体系的统一管理模块，整合了从 shop 和 commission 模块迁入的积分与购物余额功能。覆盖 10 个 extrinsics、8 个存储项、12 个事件、18 个错误码。

**核心能力：**

- **Shop 积分系统** — 每个 Shop 独立的积分体系，支持发放、销毁、转移、兑换、过期管理、总量上限
- **NEX 购物余额** — 佣金结算产生的 NEX 消费额度，按 Entity 维度管理
- **Token 奖励/折扣入口** — 通过 `TokenProvider` 委托 token 模块执行 `reward_on_purchase` / `redeem_for_discount`
- **跨模块集成** — 实现 `LoyaltyReadPort` + `LoyaltyWritePort` 供 order/commission 模块调用
- **Shop 关闭联动** — 实现 `PointsCleanup` trait，Shop 关闭时自动清理全部积分数据

## 设计原则

| 原则 | 说明 |
|------|------|
| **积分 100% 搬入** | Shop 积分的全部 storage + 逻辑从 shop 模块迁入 loyalty |
| **Token 操作委托** | Token 奖励/兑换通过 `Config::TokenProvider` 委托，不搬 token 内部 storage |
| **NEX 购物余额搬入** | `MemberShoppingBalance` + `ShopShoppingTotal` 从 commission/core 迁入 |
| **Token 购物余额留原处** | Token 购物余额（`MemberTokenShoppingBalance`）留在 commission/core（仅内部使用） |
| **佣金保护** | 积分兑换时通过 `CommissionFundGuard` 检查已承诺的佣金资金，防止侵占 |
| **懒过期** | 积分过期检查采用懒模式，在转移/销毁/兑换时自动触发，无需定时扫描 |

## 架构依赖

```
pallet-entity-loyalty
├── pallet-entity-common
│   ├── ShopProvider ────────── Shop 查询（exists / entity_id / manager / status / account）
│   ├── EntityProvider ──────── Entity 查询（active / locked / account）
│   ├── EntityTokenProvider ─── Token 操作委托（reward_on_purchase / redeem_for_discount / token_balance / is_token_enabled）
│   ├── CommissionFundGuard ──── 佣金资金保护（protected_funds 查询）
│   ├── LoyaltyReadPort ─────── 只读 Port（本模块实现）
│   ├── LoyaltyWritePort ────── 读写 Port（本模块实现）
│   └── PointsCleanup ──────── 积分清理回调（本模块实现）
├── pallet-commission-common
│   └── ParticipationGuard ──── KYC 参与检查（购物余额消费时校验）
└── pallet-balances ──────────── 原生代币余额（积分兑换时 NEX 转账）
```

## 数据结构

### PointsConfig

Shop 积分配置，每个 Shop 独立一份：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `BoundedVec<u8, MaxPointsNameLength>` | 积分名称 |
| `symbol` | `BoundedVec<u8, MaxPointsSymbolLength>` | 积分符号 |
| `reward_rate` | `u16` | 购物返积分比例（基点，500 = 5%，上限 10000） |
| `exchange_rate` | `u16` | 积分兑换 NEX 比例（基点，1000 = 10%，上限 10000） |
| `transferable` | `bool` | 积分是否可在用户之间转让 |

## 配置项

| 参数 | 类型 | 说明 |
|------|------|------|
| `Currency` | `Currency + ReservableCurrency` | 原生代币（积分兑换时 NEX 转账） |
| `ShopProvider` | `ShopProvider<AccountId>` | Shop 查询（权限校验、状态检查、运营账户） |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 查询（活跃状态、锁定状态、账户） |
| `TokenProvider` | `EntityTokenProvider<AccountId, Balance>` | Token 操作委托（奖励/兑换/余额/启用状态） |
| `CommissionFundGuard` | `CommissionFundGuard` | 佣金资金保护（查询 Entity 已承诺的佣金资金总额） |
| `ParticipationGuard` | `ParticipationGuard<AccountId>` | KYC 参与检查（购物余额消费时验证资格） |
| `MaxPointsNameLength` | `Get<u32>` | 积分名称最大字节长度 |
| `MaxPointsSymbolLength` | `Get<u32>` | 积分符号最大字节长度 |
| `WeightInfo` | `WeightInfo` | 权重函数（10 个） |

## 存储项 (8)

### 积分系统（从 shop 模块迁入）

| 存储 | 类型 | 说明 |
|------|------|------|
| `ShopPointsConfigs` | `Map<u64 -> PointsConfig>` | Shop 积分配置（启用/禁用的核心标志） |
| `ShopPointsBalances` | `DoubleMap<u64, AccountId -> Balance>` | 用户在指定 Shop 的积分余额 |
| `ShopPointsTotalSupply` | `Map<u64 -> Balance>` | Shop 积分总供应量 |
| `ShopPointsTtl` | `Map<u64 -> BlockNumber>` | Shop 积分有效期（区块数，0 = 永不过期） |
| `ShopPointsExpiresAt` | `DoubleMap<u64, AccountId -> BlockNumber>` | 用户积分到期区块（滑动窗口延长） |
| `ShopPointsMaxSupply` | `Map<u64 -> Balance>` | Shop 积分总量上限（0 = 无上限） |

### NEX 购物余额（从 commission/core 迁入）

| 存储 | 类型 | 说明 |
|------|------|------|
| `ShopShoppingTotal` | `Map<u64 -> Balance>` | Entity 级购物余额总额（供 solvency check 使用） |
| `MemberShoppingBalance` | `DoubleMap<u64, AccountId -> Balance>` | 会员在指定 Entity 的 NEX 购物余额 |

## Extrinsics (10)

### 积分管理 — Shop Manager

| # | 名称 | 调用者 | 说明 |
|---|------|--------|------|
| 0 | `enable_points` | Shop Manager | 启用积分系统，设置名称/符号/奖励率/兑换率/可转让性 |
| 1 | `disable_points` | Shop Manager | 禁用积分系统，清除所有积分数据（余额/供应量/TTL/过期/上限） |
| 2 | `update_points_config` | Shop Manager | 更新积分配置（reward_rate / exchange_rate / transferable，至少修改一项） |
| 4 | `manager_issue_points` | Shop Manager | 直接向指定用户发放积分（检查总量上限） |
| 5 | `manager_burn_points` | Shop Manager | 直接销毁指定用户的积分（含懒过期检查） |
| 7 | `set_points_ttl` | Shop Manager | 设置积分有效期（0 = 永不过期，移除 TTL 限制） |
| 9 | `set_points_max_supply` | Shop Manager | 设置积分总量上限（0 = 无上限，当前供应量不得超过新上限） |

### 积分操作 — 用户

| # | 名称 | 调用者 | 说明 |
|---|------|--------|------|
| 3 | `transfer_points` | 用户 | 转移积分给其他用户（需 transferable=true，含懒过期检查） |
| 6 | `redeem_points` | 用户 | 兑换积分为 NEX（从 Shop 运营账户转出，含佣金资金保护检查） |
| 8 | `expire_points` | 任何人 | 清除已过期积分（任何人可调用，必须积分确实已过期） |

## 积分兑换流程

```
用户调用 redeem_points(shop_id, amount)
│
├─ 校验: Shop 存在 / 未关闭 / 未封禁 / 积分已启用 / exchange_rate > 0
├─ 懒过期检查: check_points_expiry(shop_id, who)
├─ 余额校验: balance >= amount
│
├─ 计算兑换金额:
│   payout = amount * exchange_rate / 10000
│   ensure!(payout > 0)   ── 积分数量太小会导致 RedeemPayoutZero
│
├─ 佣金资金保护:
│   shop_balance = Currency::free_balance(shop_account)
│   protected = CommissionFundGuard::protected_funds(entity_id)
│   available = shop_balance - protected
│   ensure!(available >= payout)   ── InsufficientOperatingFund
│
├─ 转账: Currency::transfer(shop_account -> who, payout)
│
├─ 销毁积分: balance -= amount, total_supply -= amount
│
└─ emit PointsRedeemed { shop_id, who, points_burned, payout }
```

## 积分过期机制

积分过期采用 **懒过期 + 滑动窗口** 设计：

| 机制 | 说明 |
|------|------|
| **TTL 设置** | `set_points_ttl(ttl_blocks)` 设置 Shop 级积分有效期，0 = 永不过期 |
| **滑动窗口** | 每次发放/接收积分时，到期时间取 `max(当前到期时间, now + ttl)` |
| **懒过期检查** | `transfer_points` / `manager_burn_points` / `redeem_points` 操作前自动检查并清除过期积分 |
| **主动清除** | 任何人可调用 `expire_points(shop_id, account)` 清除已过期积分 |
| **清除时回收** | 过期积分从余额和总供应量中扣除 |

## NEX 购物余额流程

购物余额由 commission 结算产生，供 order 下单时抵扣使用。

### 写入（credit）

```
commission 结算完成
  └─ T::Loyalty::credit_shopping_balance(entity_id, account, amount)
       ├─ MemberShoppingBalance += amount
       ├─ ShopShoppingTotal += amount
       └─ emit ShoppingBalanceCredited
```

### 使用（use） — 纯记账

```
order 下单抵扣
  └─ do_use_shopping_balance(entity_id, account, amount)
       ├─ ensure balance >= amount
       ├─ MemberShoppingBalance -= amount
       ├─ ShopShoppingTotal -= amount
       └─ emit ShoppingBalanceUsed
```

### 消费（consume） — 记账 + NEX 转账

```
do_consume_shopping_balance(entity_id, account, amount)
  ├─ ensure amount > 0
  ├─ KYC 检查: ParticipationGuard::can_participate(entity_id, account)
  ├─ ensure balance >= amount
  ├─ MemberShoppingBalance -= amount
  ├─ ShopShoppingTotal -= amount
  ├─ Currency::transfer(entity_account -> account, amount, KeepAlive)
  └─ emit ShoppingBalanceUsed
```

## Events (12)

| 事件 | 字段 | 触发时机 |
|------|------|---------|
| `ShopPointsEnabled` | shop_id, name | 启用 Shop 积分 |
| `ShopPointsDisabled` | shop_id | 禁用 Shop 积分 |
| `PointsIssued` | shop_id, to, amount | 发放积分（manager_issue_points / issue_points） |
| `PointsBurned` | shop_id, from, amount | 销毁积分（manager_burn_points / burn_points / redeem_points） |
| `PointsTransferred` | shop_id, from, to, amount | 用户之间转移积分 |
| `PointsConfigUpdated` | shop_id | 更新积分配置 |
| `PointsRedeemed` | shop_id, who, points_burned, payout | 积分兑换为 NEX |
| `PointsTtlSet` | shop_id, ttl_blocks | 设置积分有效期 |
| `PointsExpired` | shop_id, account, amount | 积分过期被清除（懒过期或主动清除） |
| `PointsMaxSupplySet` | shop_id, max_supply | 设置积分总量上限 |
| `ShoppingBalanceUsed` | entity_id, account, amount | 购物余额被使用/消费 |
| `ShoppingBalanceCredited` | entity_id, account, amount | 购物余额写入（commission 结算后） |

## Errors (18)

| 错误 | 说明 |
|------|------|
| `PointsNotEnabled` | 积分未启用（无 ShopPointsConfigs 记录） |
| `PointsAlreadyEnabled` | 积分已启用，不可重复启用 |
| `PointsNotTransferable` | 积分配置为不可转让 |
| `InsufficientPointsBalance` | 积分余额不足 |
| `PointsNameEmpty` | 积分名称不能为空 |
| `PointsNotExpired` | 积分未过期，无法执行过期清除 |
| `RedeemPayoutZero` | 兑换金额为零（积分数量太小，按比例计算结果为 0） |
| `PointsMaxSupplyExceeded` | 发行将导致积分总量超过上限 |
| `InvalidConfig` | 无效配置（symbol 为空 / rate > 10000 / 无修改字段） |
| `ShopNotFound` | Shop 不存在 |
| `NotAuthorized` | 非 Shop Manager，无权限操作 |
| `EntityNotActive` | Entity 未激活 |
| `EntityLocked` | Entity 已被全局锁定 |
| `ShopAlreadyClosed` | Shop 已关闭或处于终态 |
| `ShopBanned` | Shop 已被封禁 |
| `SameAccount` | 不能转给自己 |
| `InsufficientOperatingFund` | Shop 运营资金不足（扣除佣金保护后可用余额不足） |
| `InsufficientShoppingBalance` | 购物余额不足 |
| `ZeroAmount` | 金额为零 |
| `ParticipationRequirementNotMet` | 不满足参与要求（KYC 检查未通过） |

## 权限模型

```
┌──────────────────────────────────────────────────────────┐
│  Shop Manager（Shop 积分管理）                            │
│  enable_points(0) / disable_points(1)                    │
│  update_points_config(2)                                 │
│  manager_issue_points(4) / manager_burn_points(5)        │
│  set_points_ttl(7) / set_points_max_supply(9)            │
│  + Entity active + Entity not locked + Shop 非终态       │
├──────────────────────────────────────────────────────────┤
│  用户（积分持有者）                                        │
│  transfer_points(3) / redeem_points(6)                   │
│  + Shop 存在 + Shop 未关闭 + Shop 未封禁                  │
├──────────────────────────────────────────────────────────┤
│  任何人                                                   │
│  expire_points(8) — 仅当积分确实已过期时才执行             │
└──────────────────────────────────────────────────────────┘
```

## Trait 实现

### LoyaltyReadPort (4 方法)

供 order / commission 模块只读查询：

| 方法 | 说明 | 数据来源 |
|------|------|---------|
| `is_token_enabled(entity_id)` | 查询 Entity 是否启用了 Token | 委托 `TokenProvider` |
| `token_discount_balance(entity_id, who)` | 查询 Token 折扣可用余额 | 委托 `TokenProvider` |
| `shopping_balance(entity_id, who)` | 查询会员 NEX 购物余额 | `MemberShoppingBalance` |
| `shopping_total(entity_id)` | 查询 Entity 级购物余额总额 | `ShopShoppingTotal` |

### LoyaltyWritePort (4 方法)

供 order / commission 模块写入操作：

| 方法 | 说明 | 实现 |
|------|------|------|
| `redeem_for_discount(entity_id, who, tokens)` | Token 折扣抵扣 | 委托 `TokenProvider` |
| `consume_shopping_balance(entity_id, who, amount)` | 消费购物余额（记账 + NEX 转账） | `do_consume_shopping_balance` |
| `reward_on_purchase(entity_id, who, purchase_amount)` | 购物后 Token 奖励 | 委托 `TokenProvider` |
| `credit_shopping_balance(entity_id, who, amount)` | 写入购物余额 | `do_credit_shopping_balance` |

### PointsCleanup (1 方法)

供 shop 模块在 Shop 关闭时调用：

| 方法 | 说明 |
|------|------|
| `cleanup_shop_points(shop_id)` | 清理指定 Shop 的全部积分数据（Config / Balances / TotalSupply / TTL / ExpiresAt / MaxSupply） |

## 公开辅助函数

供外部模块（如 order）直接调用：

| 函数 | 说明 |
|------|------|
| `issue_points(shop_id, to, amount)` | 发放积分（检查 Shop 状态 / 积分启用 / 总量上限） |
| `burn_points(shop_id, from, amount)` | 销毁积分（含懒过期检查） |
| `cleanup_shop_points(shop_id)` | 清理 Shop 全部积分数据 |
| `get_points_balance(shop_id, account)` | 查询用户积分余额 |
| `get_points_total_supply(shop_id)` | 查询 Shop 积分总供应量 |
| `get_points_config(shop_id)` | 查询 Shop 积分配置 |
| `get_points_expiry(shop_id, account)` | 查询用户积分到期区块 |
| `get_points_max_supply(shop_id)` | 查询 Shop 积分总量上限 |
| `shopping_total(entity_id)` | 查询 Entity 级购物余额总额 |

## 与其他模块的集成

```
pallet-entity-shop ──────────→ PointsCleanup
  Shop 关闭时调用                 cleanup_shop_points(shop_id)

pallet-entity-order ─────────→ LoyaltyWritePort / LoyaltyReadPort
  下单: redeem_for_discount        Token 积分抵扣
  下单: consume_shopping_balance   购物余额抵扣
  完成: reward_on_purchase         购物后 Token 奖励
  查询: shopping_balance           购物余额余额
  查询: is_token_enabled           Token 是否启用

pallet-commission-core ──────→ LoyaltyWritePort
  结算: credit_shopping_balance    写入购物余额
  使用: consume_shopping_balance   消费购物余额

pallet-entity-loyalty ───────→ EntityTokenProvider (委托)
  Token 奖励/兑换/余额查询         pallet-entity-token
```

## 安全设计

| 防护 | 说明 |
|------|------|
| **佣金资金保护** | `redeem_points` 兑换前检查 `CommissionFundGuard::protected_funds`，防止侵占已承诺的佣金资金 |
| **总量上限** | 发放积分前通过 `check_points_max_supply` 校验，当前供应量 + 新增量不得超过上限 |
| **懒过期安全** | 转移/销毁/兑换前自动检查过期，避免使用已过期的积分 |
| **滑动窗口** | 积分到期时间取 max(当前到期, now + ttl)，新获得积分不会缩短已有积分寿命 |
| **自购保护** | `transfer_points` 拒绝 from == to |
| **零金额拒绝** | 发放/销毁/转移/消费均检查金额非零 |
| **KYC 检查** | `do_consume_shopping_balance` 调用 `ParticipationGuard::can_participate` |
| **Shop 状态检查** | Manager 操作检查 Entity active + not locked + Shop 非终态/非封禁 |
| **清理上限** | `clear_prefix` 使用 `POINTS_CLEANUP_LIMIT = 500` 防止单次清理超出区块权重 |
| **ExistenceRequirement** | 兑换使用 `AllowDeath`（允许 Shop 账户清零），消费使用 `KeepAlive`（保持 Entity 账户存活） |

## 常量

| 常量 | 值 | 说明 |
|------|------|------|
| `POINTS_CLEANUP_LIMIT` | 500 | 单次 `clear_prefix` 最大清理条目数 |
| `STORAGE_VERSION` | 0 | 存储版本 |

## 测试

```bash
cargo test -p pallet-entity-loyalty
```

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-03 | 初始版本：Shop 积分系统 + NEX 购物余额从 shop/commission 迁入，LoyaltyReadPort/WritePort/PointsCleanup 实现 |

## 许可证

MIT License
