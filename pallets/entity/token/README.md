# pallet-entity-token

> NEXUS Entity 通证模块 — pallet-assets 桥接层，多类型通证、分红、锁仓、转账限制、内幕交易防护 | Runtime Index: 124

## 概述

`pallet-entity-token` 作为 `pallet-assets` 的桥接层，为每个 Entity 提供独立的通证系统。支持 7 种通证类型（积分、治理、股权、会员、份额、债券、混合），购物奖励/兑换、分红分发、代币锁仓、双向转账限制（白名单/黑名单/KYC/成员闭环）、内幕交易黑窗口期防护等功能。

同时实现 `EntityTokenProvider` trait（12 个方法），供 `pallet-entity-tokensale`、`pallet-entity-order`、`pallet-entity-governance`、`pallet-entity-market` 等模块调用 reserve/unreserve/repatriate/governance_burn。

## 架构

```
pallet-entity-token (pallet_index = 124, 桥接层, 25 extrinsics)
│
├── 外部依赖 (6 trait)
│   ├── pallet-assets           底层资产（Create/Inspect/Mutate/MetadataMutate）
│   ├── EntityProvider          Entity 查询（owner/active/locked/admin/account）
│   ├── KycLevelProvider        KYC 级别查询（可选，默认 NullKycProvider）
│   ├── EntityMemberProvider    成员查询（可选，默认 NullMemberProvider）
│   ├── DisclosureProvider      内幕交易黑窗口期查询
│   └── WeightInfo              权重函数（25 个）
│
├── 核心逻辑
│   ├── 通证管理                创建/配置/元数据/类型变更/启停
│   ├── 铸造 & 销毁             mint_tokens / burn_tokens / force_burn
│   ├── 转账                    transfer_tokens / force_transfer
│   ├── 购物奖励                reward_on_purchase（mint）
│   ├── 积分兑换                redeem_for_discount（burn）
│   ├── 分红分发                distribute → pending → claim（mint）
│   ├── 代币锁仓                lock / unlock（独立条目，最多 10 条）
│   ├── 转账限制                5 种模式 × 双向（sender + receiver）拦截
│   └── 紧急管控                force_disable / freeze / pause / burn / transfer
│
├── Trait 实现
│   └── EntityTokenProvider     12 方法
│
└── 查询函数 (6)
    ├── get_balance / get_total_supply / is_token_enabled
    ├── get_account_token_info   (balance, locked, reserved, pending, available)
    ├── get_lock_entries          锁仓条目列表
    └── get_available_balance    可用余额（总额 - 锁仓 - 预留）
```

## 通证类型 (TokenType)

定义在 `pallet-entity-common`，7 种类型：

| 类型 | 投票权 | 分红权 | 默认可转让 | 默认转账限制 | 默认 KYC |
|------|--------|--------|-----------|-------------|----------|
| `Points` | - | - | 是 | None | 0 |
| `Governance` | 是 | - | 是 | KycRequired | 2 |
| `Equity` | 是 | 是 | 是 | Whitelist | 3 |
| `Membership` | - | - | 否 | MembersOnly | 1 |
| `Share` | - | 是 | 是 | KycRequired | 2 |
| `Bond` | - | 是 | 是 | KycRequired | 2 |
| `Hybrid(u8)` | 是 | 是 | 是 | None | 2 |

## 转账限制 (TransferRestrictionMode)

**双向检查**：发送方和接收方均须满足限制条件。

| 模式 | 发送方检查 | 接收方检查 |
|------|-----------|-----------|
| `None` | - | - |
| `Whitelist` | `from` 须在白名单 | `to` 须在白名单 |
| `Blacklist` | `from` 不在黑名单 | `to` 不在黑名单 |
| `KycRequired` | `from` KYC ≥ min_kyc | `to` KYC ≥ min_kyc |
| `MembersOnly` | `from` 须是成员 | `to` 须是成员 |

额外：`DisclosureProvider::can_insider_trade` — 黑窗口期内幕人员禁止转账。

## 数据结构

### EntityTokenConfig

```rust
pub struct EntityTokenConfig<Balance, BlockNumber> {
    pub enabled: bool,
    pub reward_rate: u16,              // 基点，500 = 5%
    pub exchange_rate: u16,            // 基点，1000 = 10%
    pub min_redeem: Balance,
    pub max_redeem_per_order: Balance, // 0 = 无限
    pub transferable: bool,
    pub created_at: BlockNumber,
    pub token_type: TokenType,
    pub max_supply: Balance,           // 0 = 无限
    pub dividend_config: DividendConfig<Balance, BlockNumber>,
    pub transfer_restriction: TransferRestrictionMode,
    pub min_receiver_kyc: u8,          // 0-4
}
```

### LockEntry

```rust
pub struct LockEntry<Balance, BlockNumber> {
    pub amount: Balance,
    pub unlock_at: BlockNumber,
}
// 每用户每实体最多 10 条独立锁仓条目
```

## Config 配置

| 参数 | 类型 | 说明 |
|------|------|------|
| `Assets` | Create+Inspect+Mutate+MetadataMutate | pallet-assets 实例 |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 查询（owner/active/locked/admin/account） |
| `ShopTokenOffset` | `Get<u64>` | 资产 ID 偏移量（**必须 > 0**） |
| `MaxTokenNameLength` | `Get<u32>` | 代币名称最大字节长度（≥ 1） |
| `MaxTokenSymbolLength` | `Get<u32>` | 代币符号最大字节长度（≥ 1） |
| `MaxTransferListSize` | `Get<u32>` | 白/黑名单最大容量 + 批量输入长度限制 |
| `MaxDividendRecipients` | `Get<u32>` | 分红单次最大接收人数 |
| `KycProvider` | `KycLevelProvider<AccountId>` | KYC 查询（可用 NullKycProvider） |
| `MemberProvider` | `EntityMemberProvider<AccountId>` | 成员查询（可用 NullMemberProvider） |
| `DisclosureProvider` | `DisclosureProvider<AccountId>` | 内幕交易黑窗口期查询 |
| `WeightInfo` | `WeightInfo` | 权重函数 |

## 存储项 (12)

| 存储 | 类型 | 说明 |
|------|------|------|
| `EntityTokenConfigs` | `StorageMap<u64 → EntityTokenConfig>` | 通证配置主表 |
| `EntityTokenMetadata` | `StorageMap<u64 → (name, symbol, decimals)>` | 元数据 |
| `TotalEntityTokens` | `StorageValue<u64>` | 已创建通证总数 |
| `LockedTokens` | `StorageDoubleMap<u64, AccountId → BoundedVec<LockEntry, 10>>` | 锁仓条目 |
| `PendingDividends` | `StorageDoubleMap<u64, AccountId → Balance>` | 待领取分红 |
| `ClaimedDividends` | `StorageDoubleMap<u64, AccountId → Balance>` | 已领取分红总额 |
| `TotalPendingDividends` | `StorageMap<u64 → Balance>` | 实体级待领取总额 |
| `TransferWhitelist` | `StorageDoubleMap<u64, AccountId → ()>` | 白名单（O(1)） |
| `TransferBlacklist` | `StorageDoubleMap<u64, AccountId → ()>` | 黑名单（O(1)） |
| `ReservedTokens` | `StorageDoubleMap<u64, AccountId → Balance>` | 预留代币 |
| `TransfersFrozen` | `StorageMap<u64 → ()>` | 转账冻结标记 |
| `GlobalTokenPaused` | `StorageValue<bool>` | 全平台暂停开关 |

## Extrinsics (25)

### 通证管理

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 0 | `create_shop_token(entity_id, name, symbol, decimals, reward_rate, exchange_rate)` | Owner/Admin | 创建通证 |
| 1 | `update_token_config(entity_id, ...)` | Owner/Admin | 更新配置 |
| 9 | `change_token_type(entity_id, new_type)` | Owner/Admin | 变更类型（联动 transferable/restriction/kyc） |
| 10 | `set_max_supply(entity_id, max_supply)` | Owner/Admin | 设最大供应（≥ supply + pending） |
| 11 | `set_transfer_restriction(entity_id, mode, min_kyc)` | Owner/Admin | 设转账限制 |
| 22 | `update_token_metadata(entity_id, name, symbol)` | Owner/Admin | 更新名称/符号 |

### 铸造 & 销毁 & 转账

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 2 | `mint_tokens(entity_id, to, amount)` | Owner/Admin | 铸造（检查 max_supply 含 pending） |
| 3 | `transfer_tokens(entity_id, to, amount)` | 持有者 | 转让（全链路检查） |
| 21 | `burn_tokens(entity_id, amount)` | 持有者 | 销毁自己代币 |

### 分红

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 4 | `configure_dividend(entity_id, enabled, min_period)` | Owner/Admin | 配置分红 |
| 5 | `distribute_dividend(entity_id, total, recipients)` | Owner/Admin | 分发（需 active + max_supply 预检） |
| 6 | `claim_dividend(entity_id)` | 持有者 | 领取分红（承诺必兑付） |

### 锁仓

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 7 | `lock_tokens(entity_id, amount, duration)` | 持有者 | 锁仓（需 active） |
| 8 | `unlock_tokens(entity_id)` | 持有者 | 解锁到期条目 |

### 白/黑名单

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 12 | `add_to_whitelist(entity_id, accounts)` | Owner/Admin | 批量添加白名单 |
| 13 | `remove_from_whitelist(entity_id, accounts)` | Owner/Admin | 批量移除白名单 |
| 14 | `add_to_blacklist(entity_id, accounts)` | Owner/Admin | 批量添加黑名单 |
| 15 | `remove_from_blacklist(entity_id, accounts)` | Owner/Admin | 批量移除黑名单 |

### 紧急管控 (Root-only)

| # | 调用 | 说明 |
|---|------|------|
| 16 | `force_disable_token(entity_id)` | 强制禁用 |
| 17 | `force_freeze_transfers(entity_id)` | 冻结转账（claim 不受影响） |
| 18 | `force_unfreeze_transfers(entity_id)` | 解冻 |
| 19 | `force_burn(entity_id, from, amount)` | 强制销毁 + 零余额清理 |
| 20 | `set_global_token_pause(paused)` | 全平台暂停/恢复 |
| 23 | `force_transfer(entity_id, from, to, amount)` | 合规强制转账 + 零余额清理 |
| 24 | `force_enable_token(entity_id)` | 强制重新启用 |

## EntityTokenProvider 实现 (12 方法)

| 方法 | 说明 | 策略检查 |
|------|------|---------|
| `is_token_enabled` | 查 config.enabled | — |
| `token_balance` | Assets::balance | — |
| `available_balance` | balance - locked - reserved | — |
| `reward_on_purchase` | 计算奖励 + mint | GlobalPaused 静默跳过 |
| `redeem_for_discount` | 检查限额 + burn | GlobalPaused 拒绝 |
| `transfer` | Assets::transfer | zero/pause/frozen/active/enabled/transferable/restriction |
| `reserve` | 检查可用余额 → ReservedTokens | zero check |
| `unreserve` | 减少 ReservedTokens | 返回 min(amount, current) |
| `repatriate_reserved` | **直接** Assets::transfer(Expendable) | **绕过所有策略** |
| `get_token_type` | 查 config.token_type | 默认 Points |
| `total_supply` | Assets::total_issuance | — |
| `governance_burn` | entity_account burn_from | zero/enabled check |

> **repatriate_reserved**: 预留代币是已承诺资金，释放时直接调用底层 Assets::transfer，不受 GlobalPaused/TransfersFrozen/EntityNotActive 阻拦，使用 Expendable 允许账户清零。

## Events (25)

| 事件 | 字段 | 触发 |
|------|------|------|
| `EntityTokenCreated` | entity_id, asset_id, name, symbol | create_shop_token |
| `TokenConfigUpdated` | entity_id | update_token_config / set_max_supply |
| `RewardIssued` | entity_id, buyer, amount | reward_on_purchase |
| `TokensRedeemed` | entity_id, buyer, tokens, discount | redeem_for_discount |
| `TokensTransferred` | entity_id, from, to, amount | transfer_tokens |
| `TokensMinted` | entity_id, to, amount | mint_tokens |
| `DividendConfigured` | entity_id, enabled, min_period | configure_dividend |
| `DividendDistributed` | entity_id, total_amount, recipients_count | distribute_dividend |
| `DividendClaimed` | entity_id, holder, amount | claim_dividend |
| `TokensLocked` | entity_id, holder, amount, unlock_at | lock_tokens |
| `TokensUnlocked` | entity_id, holder, amount | unlock_tokens |
| `TokenTypeChanged` | entity_id, old_type, new_type | change_token_type |
| `TransferRestrictionSet` | entity_id, mode, min_receiver_kyc | set_transfer_restriction |
| `WhitelistUpdated` | entity_id, added, removed | add/remove whitelist |
| `BlacklistUpdated` | entity_id, added, removed | add/remove blacklist |
| `TokenForceDisabled` | entity_id | force_disable_token |
| `TransfersFrozenEvent` | entity_id | force_freeze_transfers |
| `TransfersUnfrozen` | entity_id | force_unfreeze_transfers |
| `TokensForceBurned` | entity_id, from, amount | force_burn |
| `GlobalTokenPauseSet` | paused | set_global_token_pause |
| `TokensGovernanceBurned` | entity_id, from, amount | governance_burn |
| `TokensBurned` | entity_id, holder, amount | burn_tokens |
| `TokenMetadataUpdated` | entity_id, name, symbol | update_token_metadata |
| `TokensForceTransferred` | entity_id, from, to, amount | force_transfer |
| `TokenForceEnabled` | entity_id | force_enable_token |

## Errors (49)

| 错误 | 说明 |
|------|------|
| `EntityNotFound` | 实体不存在 |
| `TokenNotEnabled` | 通证未启用或不存在 |
| `TokenAlreadyExists` | 通证已存在 |
| `InsufficientBalance` | 可用余额不足 |
| `BelowMinRedeem` | 低于最低兑换门槛 |
| `ExceedsMaxRedeem` | 超过单笔最大兑换 |
| `TransferNotAllowed` | transferable = false |
| `NameTooLong` / `SymbolTooLong` | 超过 MaxLength |
| `EmptyName` / `EmptySymbol` | 名称/符号为空 |
| `AssetCreationFailed` | pallet-assets 操作失败 |
| `InvalidRewardRate` / `InvalidExchangeRate` | > 10000 |
| `InvalidRedeemLimits` | min_redeem > max_redeem |
| `DividendNotEnabled` | 分红未启用 |
| `DividendPeriodNotReached` | 分红周期未到 |
| `NoDividendToClaim` | 无待领取分红 |
| `ZeroDividendAmount` | 分红总额为零 |
| `DividendAmountMismatch` | total ≠ sum(recipients) |
| `TooManyRecipients` | 超 MaxDividendRecipients |
| `NoLockedTokens` | 无锁仓记录 |
| `UnlockTimeNotReached` | 解锁时间未到 |
| `LocksFull` | 锁仓条目已满（10 条） |
| `ExceedsMaxSupply` | 超过最大供应量 |
| `TokenTypeNotSupported` | 类型不支持此操作 |
| `SameTokenType` | 变更为相同类型 |
| `ReceiverNotInWhitelist` / `SenderNotInWhitelist` | 白名单限制 |
| `ReceiverInBlacklist` / `SenderInBlacklist` | 黑名单限制 |
| `ReceiverKycInsufficient` / `SenderKycInsufficient` | KYC 级别不足 |
| `ReceiverNotMember` / `SenderNotMember` | 非成员 |
| `TransferListFull` | 白/黑名单已满 |
| `EntityNotActive` | 实体未激活 |
| `ZeroAmount` | 数量为零 |
| `InvalidLockDuration` | 锁仓时长为零 |
| `TokenCountOverflow` | 代币总数溢出 |
| `InsiderTradingRestricted` | 黑窗口期内幕交易 |
| `EntityLocked` | 实体已全局锁定 |
| `NotAuthorized` | 非 Owner 且无 TOKEN_MANAGE 权限 |
| `TokenTransfersFrozen` | 转账已冻结 |
| `GlobalPaused` | 全平台已暂停 |
| `TokenAlreadyDisabled` / `TokenAlreadyEnabled` | 幂等性检查 |
| `TransfersNotFrozen` / `TransfersAlreadyFrozen` | 幂等性检查 |

## 权限模型

| 权限层级 | 操作范围 |
|---------|---------|
| **Owner/Admin** | call_index 0-2, 4-5, 9-15, 22 — 通证配置/铸造/分红/名单管理 |
| **持有者** | call_index 3, 6-8, 21 — 转账/领取分红/锁仓解锁/销毁 |
| **Root** | call_index 16-20, 23-24 — 紧急管控（禁用/冻结/销毁/暂停/强制转账/启用） |

> **Admin 权限**: 拥有 `TOKEN_MANAGE` (`0b0000_0100`) 权限的实体管理员可执行所有 Owner/Admin 操作。由 `ensure_owner_or_admin` 统一校验。
>
> **Entity 锁定**: 所有 Owner/Admin 操作额外检查 `is_entity_locked`，全局锁定时拒绝配置变更。

## 与其他模块的集成

```
pallet-entity-order ──→ EntityTokenProvider ──→ pallet-entity-token
(订单完成/下单抵扣)     reward_on_purchase       │
                       redeem_for_discount       │ fungibles traits
                                                 ▼
pallet-entity-tokensale ──→ EntityTokenProvider ──→ pallet-assets
(代币发售)                  reserve/repatriate     (底层资产)

pallet-entity-governance ──→ token_balance / get_token_type / governance_burn
(投票权重/提案销毁)

pallet-entity-market ──→ token_balance / reserve / unreserve
(二级市场交易)

pallet-entity-disclosure ──→ DisclosureProvider ──→ can_insider_trade
(内幕交易限制)                                       (黑窗口期检查)
```

## 测试

```bash
cargo test -p pallet-entity-token
# 140 个单元测试
```

### 测试覆盖 (140 tests)

- **创建**: 正常创建、不存在/未激活/非所有者/重复/空名称符号/无效比率/Entity 锁定
- **配置**: 更新成功、min>max 拒绝
- **铸造**: 正常铸造、超 max_supply（含 pending_dividends）、Entity 不活跃
- **转账**: 正常转账、锁仓拦截、预留拦截、零数量拒绝、Entity 不活跃
- **分红**: 配置/分发/领取、金额不匹配、人数超限、类型不支持、超 max_supply、零总额、Entity 不活跃、二次 mint 不影响 claim
- **锁仓**: 锁仓+解锁、零量/零时长、未启用、部分到期解锁、Entity 不活跃时 unlock/claim 仍可用
- **销毁**: 正常销毁、全局暂停拒绝、零数量拒绝、余额不足
- **元数据**: 更新成功、空名称/符号拒绝、非所有者拒绝
- **类型**: 变更联动 transfer_restriction、相同类型拒绝
- **供应**: 设 max_supply（含 pending 检查）、低于当前拒绝
- **限制**: KYC clamped、5 种转账限制模式、白/黑名单 CRUD、输入长度限制
- **Trait**: reserve/unreserve/repatriate 完整流程、trait transfer zero 拒绝、trait reserve zero 拒绝、trait transfer 全局暂停/冻结/可用余额检查
- **P0 Admin**: 15 个管理员权限测试
- **P1 force**: disable/enable/freeze/unfreeze 幂等性
- **P2 force_burn**: 正常/全额/零额/非 Root/存储清理
- **P2 force_transfer**: 正常/零额/非 Root/存储清理
- **P3 global_pause**: 暂停/恢复、阻止 create/mint/transfer/claim/redeem、reward 静默 0
- **H1-R2 repatriate**: Entity 不活跃/转账冻结/全局暂停/全额预留 4 种场景

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 初始版本：创建/配置/铸造/转让/奖励/兑换 |
| v0.2.0 | 2026-02-03 | Phase 2：7 种通证类型、分红、锁仓/解锁、最大供应量 |
| v0.3.0 | 2026-02-04 | Phase 8：转账限制（5 种模式）、白/黑名单、KYC/成员集成 |
| v0.4.0 | 2026-02-09 | 审计 R1：DecodeWithMemTracking、Weight proof_size、max_supply 全链路校验、锁仓+预留余额拦截、ReservedTokens 真实实现、分红安全加固（41 tests） |
| v0.5.0 | 2026-03 | P0 Admin：12 个 extrinsic 支持 TOKEN_MANAGE 管理员、ensure_owner_or_admin、NotAuthorized 错误（75 tests） |
| v0.6.0 | 2026-03 | P1/P2/P3 紧急管控：force_disable/freeze/unfreeze/burn、set_global_token_pause、6 条 guard 检查（104 tests） |
| v0.7.0 | 2026-03 | 新增 extrinsic：burn_tokens(21)、update_token_metadata(22)、force_transfer(23)、force_enable_token(24)；LockEntry 独立条目；双向转账限制；DisclosureProvider 内幕交易防护；governance_burn trait 方法（138 tests） |
| v0.7.1 | 2026-03 | 审计 R2：repatriate_reserved 绕过策略检查(Expendable)、weights 修正(force_burn/force_transfer/burn_tokens)、mock thread-local 清理、Cargo.toml feature 传播（138 tests） |
| v0.7.2 | 2026-03 | 审计 R3：移除死代码 NotEntityOwner、trait transfer/reserve 零数量检查（140 tests） |

## 许可证

MIT License
