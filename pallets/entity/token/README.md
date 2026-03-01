# pallet-entity-token

> NEXUS Entity 通证模块 — pallet-assets 桥接层，多类型通证、分红、锁仓、转账限制 | Runtime Index: 124

## 概述

`pallet-entity-token` 作为 `pallet-assets` 的桥接层，为每个 Entity 提供独立的通证系统。支持 7 种通证类型（积分、治理、股权、会员、份额、债券、混合），购物奖励/兑换、分红分发、代币锁仓、转账限制（白名单/黑名单/KYC/成员闭环）等功能。

同时实现 `EntityTokenProvider` trait，供 `pallet-entity-tokensale`、`pallet-entity-order` 等模块调用 reserve/unreserve/repatriate。

## 架构

```
pallet-entity-token (pallet_index = 124, 桥接层)
│
├── 外部依赖
│   ├── pallet-assets         底层资产（Create/Inspect/Mutate/MetadataMutate）
│   ├── EntityProvider        Entity 查询
│   ├── ShopProvider          Shop 权限/所属关系
│   ├── KycLevelProvider      KYC 级别查询（可选，默认 NullKycProvider）
│   └── EntityMemberProvider  成员查询（可选，默认 NullMemberProvider）
│
├── 核心逻辑
│   ├── 通证创建              shop_id → asset_id (offset + shop_id)
│   ├── 购物奖励              reward_on_purchase (mint)
│   ├── 积分兑换              redeem_for_discount (burn)
│   ├── 分红分发              distribute → pending → claim (mint)
│   ├── 代币锁仓              lock / unlock
│   └── 转账限制              5 种模式 × transfer_tokens 拦截
│
├── Trait 实现
│   └── EntityTokenProvider   is_token_enabled / token_balance / reserve
│                              unreserve / repatriate_reserved / transfer
│
└── 查询函数
    ├── get_balance            用户余额
    ├── get_total_supply       总供应量
    └── is_token_enabled       启用状态
```

## 通证类型 (TokenType)

定义在 `pallet-entity-common`，7 种类型：

| 类型 | 投票权 | 分红权 | 默认可转让 | 默认转账限制 | 默认接收方 KYC |
|------|--------|--------|-----------|-------------|---------------|
| `Points` | - | - | 是 | None | 0 (无) |
| `Governance` | 是 | - | 是 | KycRequired | 2 (Standard) |
| `Equity` | 是 | 是 | 是 | Whitelist | 3 (Enhanced) |
| `Membership` | - | - | 否 | MembersOnly | 1 (Basic) |
| `Share` | - | 是 | 是 | KycRequired | 2 (Standard) |
| `Bond` | - | 是 | 是 | KycRequired | 2 (Standard) |
| `Hybrid(u8)` | 是 | 是 | 是 | None | 2 (Standard) |

## 转账限制 (TransferRestrictionMode)

| 模式 | 说明 | 拦截逻辑 |
|------|------|----------|
| `None` | 无限制（默认） | 不拦截 |
| `Whitelist` | 白名单模式 | `to` 须在 `TransferWhitelist` 中 |
| `Blacklist` | 黑名单模式 | `to` 不在 `TransferBlacklist` 中 |
| `KycRequired` | KYC 模式 | `KycProvider::meets_kyc_requirement(to, min_kyc)` |
| `MembersOnly` | 成员闭环 | `MemberProvider::is_member(entity_id, to)` |

## 数据结构

### EntityTokenConfig

```rust
pub struct EntityTokenConfig<Balance, BlockNumber> {
    pub enabled: bool,
    pub reward_rate: u16,              // 购物奖励比例（基点，500 = 5%）
    pub exchange_rate: u16,            // 兑换折扣比例（基点，1000 = 10%）
    pub min_redeem: Balance,           // 最低兑换门槛
    pub max_redeem_per_order: Balance, // 单笔最大兑换（0 = 无限）
    pub transferable: bool,            // 是否允许转让
    pub created_at: BlockNumber,
    // Phase 2
    pub token_type: TokenType,         // 通证类型
    pub max_supply: Balance,           // 最大供应量（0 = 无限）
    pub dividend_config: DividendConfig<Balance, BlockNumber>,
    // Phase 8
    pub transfer_restriction: TransferRestrictionMode,
    pub min_receiver_kyc: u8,          // 接收方最低 KYC (0-4)
}
```

### DividendConfig

```rust
pub struct DividendConfig<Balance, BlockNumber> {
    pub enabled: bool,
    pub min_period: BlockNumber,       // 最小分红周期
    pub last_distribution: BlockNumber,
    pub accumulated: Balance,
}
```

## Config 配置

```rust
impl pallet_entity_token::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetId = u64;
    type AssetBalance = Balance;
    type Assets = Assets;                          // pallet-assets 实例
    type EntityProvider = EntityRegistry;
    type ShopProvider = EntityShop;
    type ShopTokenOffset = ConstU64<1_000_000>;    // asset_id = 1_000_000 + shop_id
    type MaxTokenNameLength = ConstU32<64>;
    type MaxTokenSymbolLength = ConstU32<8>;
    type MaxTransferListSize = ConstU32<1000>;
    type MaxDividendRecipients = ConstU32<500>;
    type KycProvider = TokenKycProvider;             // 或 NullKycProvider
    type MemberProvider = TokenMemberProvider;       // 或 NullMemberProvider
}
```

| 参数 | 说明 |
|------|------|
| `Assets` | pallet-assets 实例（Create + Inspect + Mutate + MetadataMutate） |
| `EntityProvider` | Entity 存在性/状态查询 |
| `ShopProvider` | Shop 存在性/owner/active 查询 |
| `ShopTokenOffset` | 资产 ID 偏移量（避免与其他资产冲突，**必须 > 0**） |
| `KycProvider` | KYC 级别查询（`KycLevelProvider` trait） |
| `MemberProvider` | Entity 成员查询（`EntityMemberProvider` trait） |
| `MaxTransferListSize` | 白名单/黑名单最大容量（也用于限制批量输入长度） |
| `MaxDividendRecipients` | 分红单次最大接收人数 |

## 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `ShopTokenConfigs` | `StorageMap<u64, EntityTokenConfig>` | 通证配置主表 |
| `ShopTokenMetadata` | `StorageMap<u64, (name, symbol, decimals)>` | 元数据 |
| `TotalShopTokens` | `StorageValue<u64>` | 已创建通证总数 |
| `LockedTokens` | `StorageDoubleMap<u64, AccountId, (Balance, BlockNumber)>` | 锁仓记录 |
| `PendingDividends` | `StorageDoubleMap<u64, AccountId, Balance>` | 待领取分红 |
| `ClaimedDividends` | `StorageDoubleMap<u64, AccountId, Balance>` | 已领取分红总额 |
| `TransferWhitelist` | `StorageMap<u64, BoundedVec<AccountId>>` | 转账白名单 |
| `TransferBlacklist` | `StorageMap<u64, BoundedVec<AccountId>>` | 转账黑名单 |
| `ReservedTokens` | `StorageDoubleMap<u64, AccountId, Balance>` | 预留代币（reserve/unreserve） |

## Extrinsics

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 0 | `create_shop_token(entity_id, name, symbol, decimals, reward_rate, exchange_rate)` | Entity owner | 创建通证（需 Entity active，name/symbol 非空） |
| 1 | `update_token_config(entity_id, reward_rate?, exchange_rate?, min_redeem?, max_redeem?, transferable?, enabled?)` | Entity owner | 更新配置（校验 min_redeem ≤ max_redeem） |
| 2 | `mint_tokens(entity_id, to, amount)` | Entity owner | 铸造代币（检查 max_supply） |
| 3 | `transfer_tokens(entity_id, to, amount)` | 持有者 | 转让（扣除锁仓+预留后检查可用余额，含转账限制） |
| 4 | `configure_dividend(entity_id, enabled, min_period)` | Owner | 配置分红（需通证类型支持） |
| 5 | `distribute_dividend(entity_id, total_amount, recipients)` | Owner | 分发分红（校验 total=sum，检查 token_type，限制人数） |
| 6 | `claim_dividend(entity_id)` | 持有者 | 领取分红（检查 max_supply，mint 给持有人） |
| 7 | `lock_tokens(entity_id, amount, lock_duration)` | 持有者 | 锁仓（需 enabled，amount>0，duration>0，扣除预留） |
| 8 | `unlock_tokens(entity_id)` | 持有者 | 解锁（到期后） |
| 9 | `change_token_type(entity_id, new_type)` | Owner | 变更类型（联动 transferable + transfer_restriction + min_kyc） |
| 10 | `set_max_supply(entity_id, max_supply)` | Owner | 设置最大供应量（须 >= 当前供应） |
| 11 | `set_transfer_restriction(entity_id, mode, min_receiver_kyc)` | Owner | 设置转账限制模式 |
| 12 | `add_to_whitelist(entity_id, accounts)` | Owner | 批量添加白名单（去重） |
| 13 | `remove_from_whitelist(entity_id, accounts)` | Owner | 批量移除白名单 |
| 14 | `add_to_blacklist(entity_id, accounts)` | Owner | 批量添加黑名单 |
| 15 | `remove_from_blacklist(entity_id, accounts)` | Owner | 批量移除黑名单 |

## 购物奖励 / 积分兑换

由 `pallet-entity-order`（订单模块）通过 `EntityTokenProvider` trait 调用：

```
订单完成 → reward_on_purchase(entity_id, buyer, purchase_amount)
         → reward = amount × reward_rate / 10000
         → 检查 max_supply（超出则返回 0，不报错）
         → mint_into(buyer, reward)

下单抵扣 → redeem_for_discount(entity_id, buyer, tokens_to_use)
         → discount = tokens × exchange_rate / 10000
         → burn_from(buyer, tokens)
         → 返回折扣金额
```

## 分红机制

```
configure_dividend → 启用分红 + 设置最小周期
                         │
distribute_dividend → 按 recipients 列表分配到 PendingDividends
(owner 调用)              │ (检查 min_period + token_type.has_dividend_rights)
                         │
claim_dividend → 从 PendingDividends 领取 → mint 给持有人
(持有者调用)
```

支持分红的通证类型：`Equity`、`Share`、`Hybrid`

## 锁仓机制

```
lock_tokens(amount, duration) → LockedTokens += (amount, unlock_at)
                                  │ 合并锁仓：金额累加，取较晚的解锁时间
                                  │
unlock_tokens → 检查 now >= unlock_at → 移除锁仓记录
```

## 资产 ID 映射

```
asset_id = ShopTokenOffset + shop_id
         = 1_000_000 + shop_id  (runtime 配置)
```

双向转换：`shop_to_asset_id()` / `asset_to_shop_id()`

## EntityTokenProvider 实现

本模块实现了 `pallet-entity-common::EntityTokenProvider` trait，供其他模块调用：

| 方法 | 实现 |
|------|------|
| `is_token_enabled` | 查 `ShopTokenConfigs` |
| `token_balance` | `Assets::balance(asset_id, holder)` |
| `reward_on_purchase` | 计算奖励 + mint |
| `redeem_for_discount` | 检查限额 + burn |
| `transfer` | `Assets::transfer` |
| `reserve` | 检查可用余额（扣除锁仓+已预留）→ 记录 ReservedTokens |
| `unreserve` | 减少 ReservedTokens，返回实际解除量 |
| `repatriate_reserved` | 减少 from 预留 → Assets::transfer → to |
| `get_token_type` | 查 config.token_type |
| `total_supply` | `Assets::total_issuance` |

## Events

| 事件 | 字段 | 触发时机 |
|------|------|----------|
| `ShopTokenCreated` | shop_id, asset_id, name, symbol | create_shop_token |
| `TokenConfigUpdated` | shop_id | update_token_config / set_max_supply |
| `RewardIssued` | shop_id, buyer, amount | reward_on_purchase |
| `TokensRedeemed` | shop_id, buyer, tokens, discount | redeem_for_discount |
| `TokensTransferred` | shop_id, from, to, amount | transfer_tokens |
| `TokensMinted` | shop_id, to, amount | mint_tokens |
| `DividendConfigured` | entity_id, enabled, min_period | configure_dividend |
| `DividendDistributed` | entity_id, total_amount, recipients_count | distribute_dividend |
| `DividendClaimed` | entity_id, holder, amount | claim_dividend |
| `TokensLocked` | entity_id, holder, amount, unlock_at | lock_tokens |
| `TokensUnlocked` | entity_id, holder, amount | unlock_tokens |
| `TokenTypeChanged` | entity_id, old_type, new_type | change_token_type |
| `TransferRestrictionSet` | entity_id, mode, min_receiver_kyc | set_transfer_restriction |
| `WhitelistUpdated` | entity_id, added, removed | add/remove_from_whitelist |
| `BlacklistUpdated` | entity_id, added, removed | add/remove_from_blacklist |

## Errors

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `NotEntityOwner` | 非实体所有者 |
| `TokenNotEnabled` | 通证未启用或不存在 |
| `TokenAlreadyExists` | 通证已存在 |
| `InsufficientBalance` | 余额不足 |
| `BelowMinRedeem` | 低于最低兑换门槛 |
| `ExceedsMaxRedeem` | 超过单笔最大兑换 |
| `TransferNotAllowed` | transferable = false |
| `NameTooLong` | 名称超过 MaxTokenNameLength |
| `SymbolTooLong` | 符号超过 MaxTokenSymbolLength |
| `AssetCreationFailed` | pallet-assets 创建失败 |
| `InvalidRewardRate` | reward_rate > 10000 |
| `InvalidExchangeRate` | exchange_rate > 10000 |
| `DividendNotEnabled` | 分红未启用 |
| `DividendPeriodNotReached` | 分红周期未到 |
| `NoDividendToClaim` | 无待领取分红 |
| `NoLockedTokens` | 无锁仓记录 |
| `UnlockTimeNotReached` | 解锁时间未到 |
| `ExceedsMaxSupply` | 超过最大供应量 |
| `TokenTypeNotSupported` | 通证类型不支持此操作 |
| `TokenTypeNotAllowed` | 不允许该通证类型 |
| `ReceiverNotInWhitelist` | 接收方不在白名单 |
| `ReceiverInBlacklist` | 接收方在黑名单 |
| `ReceiverKycInsufficient` | 接收方 KYC 级别不足 |
| `ReceiverNotMember` | 接收方非 Entity 成员 |
| `TransferListFull` | 白/黑名单已满 |
| `EntityNotActive` | 实体未激活 |
| `ZeroAmount` | 数量为零 |
| `InvalidLockDuration` | 锁仓时长为零 |
| `TooManyRecipients` | 分红接收人超过 MaxDividendRecipients |
| `DividendAmountMismatch` | total_amount ≠ sum(recipients) |
| `InvalidRedeemLimits` | min_redeem > max_redeem_per_order |
| `EmptyName` | 名称为空 |
| `EmptySymbol` | 符号为空 |

## 权限模型

| 操作 | 调用方 | 前置条件 |
|------|--------|----------|
| `create_shop_token` | Entity owner | Entity 存在 + **Entity active** + 通证不存在 + name/symbol 非空 |
| `update_token_config` | Entity owner | 通证已创建 |
| `mint_tokens` | Entity owner | 通证 enabled + max_supply 检查 |
| `transfer_tokens` | 持有者 | transferable + 可用余额（扣除锁仓+预留）+ 转账限制 |
| `configure_dividend` | Owner | token_type 支持分红 |
| `distribute_dividend` | Owner | 分红 enabled + 周期满足 + token_type 支持分红 + 人数限制 + total=sum |
| `claim_dividend` | 持有者 | PendingDividends > 0 |
| `lock_tokens` | 持有者 | 通证 enabled + amount>0 + duration>0 + 可用余额（扣除预留） |
| `unlock_tokens` | 持有者 | now >= unlock_at |
| `change_token_type` | Owner | 通证已创建 |
| `set_max_supply` | Owner | max_supply >= current_supply |
| `set_transfer_restriction` | Owner | 通证已创建 |
| `add/remove_*_list` | Owner | — |

## 与其他模块的集成

```
pallet-entity-order ──→ EntityTokenProvider ──→ pallet-entity-token
(订单完成/下单抵扣)             reward_on_purchase       │
                              redeem_for_discount       │ fungibles traits
                                                        ▼
pallet-entity-tokensale ──→ EntityTokenProvider ──→ pallet-assets
(代币发售 reserve/unreserve)   reserve/repatriate     (底层资产)

pallet-entity-governance ──→ token_balance / get_token_type
(投票权重计算)
```

## 测试

```bash
cargo test -p pallet-entity-token
```

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 初始版本：创建/配置/铸造/转让/奖励/兑换 |
| v0.2.0 | 2026-02-03 | Phase 2：7 种通证类型、分红配置/分发/领取、锁仓/解锁、最大供应量 |
| v0.3.0 | 2026-02-04 | Phase 8：转账限制（5 种模式）、白名单/黑名单、KYC/成员查询集成 |
| v0.4.0 | 2026-02-09 | 深度审计修复：DecodeWithMemTracking、Weight proof_size、max_supply 全链路校验、锁仓+预留余额拦截、ReservedTokens 真实实现、分红安全加固、mock+tests 41 用例 |

## 测试覆盖

41 个单元测试覆盖：

- **创建**: 正常创建、Shop 不存在、Shop 未激活、非店主、重复创建、空名称/符号、无效比率
- **配置**: 更新成功、min>max 拒绝
- **铸造**: 正常铸造、超过 max_supply
- **转账**: 正常转账、锁仓拦截、预留拦截
- **分红**: 配置/分发/领取、金额不匹配、人数超限、类型不支持、超 max_supply
- **锁仓**: 锁仓+解锁、零量/零时长、未启用
- **类型**: 变更联动 transfer_restriction
- **供应**: 设置 max_supply、低于当前拒绝
- **限制**: KYC clamped 事件、白/黑名单增删、输入长度限制
- **Trait**: reserve/unreserve/repatriate 完整流程、reward 超供应量跳过

## 许可证

MIT License
