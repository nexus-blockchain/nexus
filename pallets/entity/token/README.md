# pallet-entity-token

> Entity 多类型通证模块 — pallet-assets 桥接层 | Runtime Index: 124

## 概述

`pallet-entity-token` 作为 `pallet-assets` 的桥接层，为每个 Entity 提供独立的通证系统。每个 Entity 拥有一种通证，通过 `ShopTokenOffset + entity_id` 映射为底层 `pallet-assets` 的 `AssetId`。

核心能力：
- **7 种通证类型** — 积分 / 治理 / 股权 / 会员 / 份额 / 债券 / 混合，各自携带默认的转让性、转账限制和 KYC 等级
- **购物奖励 & 兑换** — `reward_on_purchase`（铸造）与 `redeem_for_discount`（销毁），由 order 模块调用
- **分红** — `distribute → pending → claim` 三阶段，distribute 时承诺铸造空间，claim 时实际铸造
- **锁仓** — 独立条目（最多 10 条），各自独立到期
- **转账限制** — 5 种模式（None / Whitelist / Blacklist / KycRequired / MembersOnly）× 双向检查
- **授权转账** — `approve + transfer_from`，类 ERC-20 授权机制
- **内幕交易防护** — `DisclosureProvider` 黑窗口期自动拦截
- **紧急管控** — force_disable / freeze / pause / burn / transfer / cancel_dividends（Root-only）

同时实现 `EntityTokenProvider` trait（12 个方法），供 tokensale / order / governance / market 等模块跨 pallet 调用。

## 架构

```
pallet-entity-token (桥接层, 28 extrinsics, 13 存储项)
│
├── 外部依赖 (6 trait)
│   ├── pallet-assets              底层资产（Create / Inspect / Mutate / MetadataMutate）
│   ├── EntityProvider             Entity 查询（owner / active / locked / admin / account）
│   ├── KycLevelProvider           KYC 级别查询（可选，默认 NullKycProvider）
│   ├── EntityMemberProvider       成员查询（可选，默认 NullMemberProvider）
│   ├── DisclosureProvider         内幕交易黑窗口期查询
│   └── WeightInfo                 权重函数（28 个）
│
├── Extrinsics (28)
│   ├── 通证管理 (6)               create / update_config / change_type / set_max_supply / set_restriction / update_metadata
│   ├── 铸造 & 销毁 (2)            mint_tokens / burn_tokens
│   ├── 转账 (2)                   transfer_tokens / transfer_from
│   ├── 授权 (1)                   approve_tokens
│   ├── 购物积分 (内部)            reward_on_purchase / redeem_for_discount（由 order 模块通过 trait 调用）
│   ├── 分红 (2)                   configure_dividend / distribute_dividend / claim_dividend
│   ├── 锁仓 (2)                   lock_tokens / unlock_tokens
│   ├── 白/黑名单 (4)              add/remove whitelist / blacklist
│   └── 紧急管控 (8, Root)         disable / enable / freeze / unfreeze / burn / transfer / pause / cancel_dividends
│
├── EntityTokenProvider trait (12 方法)
│
└── 查询函数 (9)
    ├── token_balance / get_total_supply / is_token_enabled
    ├── get_account_token_info       (balance, locked, reserved, pending, available)
    ├── get_lock_entries             锁仓条目列表
    ├── get_available_balance        可用余额 = 总额 − 锁仓 − 预留
    ├── is_whitelisted / is_blacklisted
    └── get_allowance                授权额度查询
```

## 通证类型

定义在 `pallet-entity-common::TokenType`，7 种：

| 类型 | 投票权 | 分红权 | 默认可转让 | 默认转账限制 | 默认 KYC |
|------|:------:|:------:|:---------:|:-----------:|:--------:|
| `Points` | — | — | 是 | None | 0 |
| `Governance` | 是 | — | 是 | KycRequired | 2 |
| `Equity` | 是 | 是 | 是 | Whitelist | 3 |
| `Membership` | — | — | 否 | MembersOnly | 1 |
| `Share` | — | 是 | 是 | KycRequired | 2 |
| `Bond` | — | 是 | 是 | KycRequired | 2 |
| `Hybrid` | 是 | 是 | 是 | None | 2 |

`change_token_type` 时自动联动更新 `transferable`、`transfer_restriction`、`min_receiver_kyc`。

## 转账安全链路

`transfer_tokens` 和 `transfer_from` 执行完整的 **7 级检查链**：

```
① GlobalPaused?          ─ 全平台紧急暂停
② TransfersFrozen?       ─ 该 Entity 转账冻结
③ config.enabled?        ─ 代币已启用
④ config.transferable?   ─ 允许用户间转让
⑤ Entity active?         ─ Entity 未被封禁 / 暂停
⑥ TransferRestriction    ─ 5 种模式 × 双向（sender + receiver）
⑦ InsiderTrading         ─ DisclosureProvider 黑窗口期拦截
```

额外：`ZeroAmount` 拒绝 / `SelfTransfer` 拒绝 / 可用余额 ≥ 转账金额（扣除锁仓 + 预留）。

### 转账限制模式

| 模式 | 发送方检查 | 接收方检查 |
|------|-----------|-----------|
| `None` | — | — |
| `Whitelist` | `from` 须在白名单 | `to` 须在白名单 |
| `Blacklist` | `from` 不在黑名单 | `to` 不在黑名单 |
| `KycRequired` | `from` KYC ≥ min_kyc | `to` KYC ≥ min_kyc |
| `MembersOnly` | `from` 须是成员 | `to` 须是成员 |

## 可用余额模型

```
available = Assets::balance(asset_id, who)
          − total_locked_amount(entity_id, who)    // 未过期锁仓总额
          − ReservedTokens(entity_id, who)          // 跨模块预留（order / market / governance / tokensale）
```

所有涉及余额消费的操作（转账 / 锁仓 / 销毁 / 兑换 / 预留）统一通过 `ensure_available_balance` 校验。

## 数据结构

### EntityTokenConfig

```rust
pub struct EntityTokenConfig<Balance, BlockNumber> {
    pub enabled: bool,                              // 代币开关
    pub reward_rate: u16,                            // 购物奖励比例（基点，500 = 5%）
    pub exchange_rate: u16,                          // 积分兑换比例（基点，1000 = 10%）
    pub min_redeem: Balance,                         // 最低兑换门槛
    pub max_redeem_per_order: Balance,               // 单笔最大兑换（0 = 无限）
    pub transferable: bool,                          // 是否允许用户间转让
    pub created_at: BlockNumber,                     // 创建区块
    pub token_type: TokenType,                       // 通证类型
    pub max_supply: Balance,                         // 最大供应量（0 = 无限）
    pub dividend_config: DividendConfig<Balance, BlockNumber>,
    pub transfer_restriction: TransferRestrictionMode,
    pub min_receiver_kyc: u8,                        // 接收方最低 KYC (0-4)
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

## Config

| 参数 | 类型 | 说明 |
|------|------|------|
| `Assets` | `Create + Inspect + Mutate + MetadataMutate` | pallet-assets 实例 |
| `EntityProvider` | `EntityProvider<AccountId>` | Entity 查询 |
| `ShopTokenOffset` | `Get<u64>` | 资产 ID 偏移量（**必须 > 0**） |
| `MaxTokenNameLength` | `Get<u32>` | 代币名称最大字节长度（≥ 1） |
| `MaxTokenSymbolLength` | `Get<u32>` | 代币符号最大字节长度（≥ 1） |
| `MaxTransferListSize` | `Get<u32>` | 白/黑名单最大容量 + 批量输入限制 |
| `MaxDividendRecipients` | `Get<u32>` | 分红单次最大接收人数 |
| `KycProvider` | `KycLevelProvider<AccountId>` | KYC 查询（可用 `NullKycProvider`） |
| `MemberProvider` | `EntityMemberProvider<AccountId>` | 成员查询（可用 `NullMemberProvider`） |
| `DisclosureProvider` | `DisclosureProvider<AccountId>` | 内幕交易黑窗口期查询 |
| `WeightInfo` | `WeightInfo` | 权重函数 |

## 存储项 (13)

| 存储 | 类型 | 说明 |
|------|------|------|
| `EntityTokenConfigs` | `Map<u64 → EntityTokenConfig>` | 通证配置主表 |
| `EntityTokenMetadata` | `Map<u64 → (name, symbol, decimals)>` | 元数据 |
| `TotalEntityTokens` | `Value<u64>` | 已创建通证总数 |
| `LockedTokens` | `DoubleMap<u64, AccountId → BoundedVec<LockEntry, 10>>` | 锁仓条目 |
| `PendingDividends` | `DoubleMap<u64, AccountId → Balance>` | 待领取分红 |
| `ClaimedDividends` | `DoubleMap<u64, AccountId → Balance>` | 已领取分红总额 |
| `TotalPendingDividends` | `Map<u64 → Balance>` | 实体级已承诺分红总额 |
| `TransferWhitelist` | `DoubleMap<u64, AccountId → ()>` | 白名单 |
| `TransferBlacklist` | `DoubleMap<u64, AccountId → ()>` | 黑名单 |
| `ReservedTokens` | `DoubleMap<u64, AccountId → Balance>` | 跨模块预留代币 |
| `TransfersFrozen` | `Map<u64 → ()>` | 转账冻结标记 |
| `GlobalTokenPaused` | `Value<bool>` | 全平台暂停开关 |
| `TokenApprovals` | `NMap<(u64, AccountId, AccountId) → Balance>` | 授权额度 |

## Extrinsics (28)

### 通证管理 — Owner/Admin

| # | 调用 | 说明 |
|---|------|------|
| 0 | `create_shop_token(entity_id, name, symbol, decimals, reward_rate, exchange_rate)` | 创建通证，默认 Points 类型 |
| 1 | `update_token_config(entity_id, reward_rate?, exchange_rate?, min_redeem?, max_redeem?, transferable?, enabled?)` | 更新配置（部分字段可选） |
| 9 | `change_token_type(entity_id, new_type)` | 变更类型，联动更新 transferable / restriction / kyc |
| 10 | `set_max_supply(entity_id, max_supply)` | 设最大供应量（≥ 当前供应 + 已承诺分红） |
| 11 | `set_transfer_restriction(entity_id, mode, min_kyc)` | 设转账限制模式，KYC 级别 clamp 到 0-4 |
| 22 | `update_token_metadata(entity_id, name, symbol)` | 更新名称 / 符号 |

### 铸造 & 销毁

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 2 | `mint_tokens(entity_id, to, amount)` | Owner/Admin | 铸造（检查 max_supply 含 pending） |
| 21 | `burn_tokens(entity_id, amount)` | 持有者 | 销毁自己代币（Entity 不活跃时仍可用） |

### 转账 & 授权

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 3 | `transfer_tokens(entity_id, to, amount)` | 持有者 | 转让（7 级检查链，禁止自转账） |
| 26 | `approve_tokens(entity_id, spender, amount)` | 持有者 | 授权第三方使用额度（0 = 撤销） |
| 27 | `transfer_from(entity_id, owner, to, amount)` | 被授权者 | 授权转账（同 transfer_tokens 检查链 + 额度扣减） |

### 分红

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 4 | `configure_dividend(entity_id, enabled, min_period)` | Owner/Admin | 配置分红（需 Equity / Share / Hybrid 类型） |
| 5 | `distribute_dividend(entity_id, total, recipients)` | Owner/Admin | 分发（承诺铸造空间，计入 TotalPendingDividends） |
| 6 | `claim_dividend(entity_id)` | 持有者 | 领取分红（实际铸造，需代币已启用） |

### 锁仓

| # | 调用 | 权限 | 说明 |
|---|------|------|------|
| 7 | `lock_tokens(entity_id, amount, duration)` | 持有者 | 新增锁仓条目（需 Entity 活跃） |
| 8 | `unlock_tokens(entity_id)` | 持有者 | 批量解锁已到期条目 |

### 白/黑名单 — Owner/Admin

| # | 调用 | 说明 |
|---|------|------|
| 12 | `add_to_whitelist(entity_id, accounts)` | 批量添加白名单（幂等） |
| 13 | `remove_from_whitelist(entity_id, accounts)` | 批量移除白名单 |
| 14 | `add_to_blacklist(entity_id, accounts)` | 批量添加黑名单（幂等） |
| 15 | `remove_from_blacklist(entity_id, accounts)` | 批量移除黑名单 |

### 紧急管控 — Root-only

| # | 调用 | 说明 |
|---|------|------|
| 16 | `force_disable_token(entity_id)` | 强制禁用（阻止 mint / transfer / claim） |
| 24 | `force_enable_token(entity_id)` | 强制重新启用 |
| 17 | `force_freeze_transfers(entity_id)` | 冻结转账（claim 不受影响） |
| 18 | `force_unfreeze_transfers(entity_id)` | 解除转账冻结 |
| 19 | `force_burn(entity_id, from, amount)` | 强制销毁 + 零余额存储清理 |
| 23 | `force_transfer(entity_id, from, to, amount)` | 合规强制转账 + 零余额存储清理 |
| 20 | `set_global_token_pause(paused)` | 全平台暂停 / 恢复 |
| 25 | `force_cancel_pending_dividends(entity_id, accounts)` | 取消指定用户的待领取分红 |

## 权限模型

```
┌──────────────────────────────────────────────────────────┐
│  Root (sudo / council)                                   │
│  force_disable(16) / force_enable(24)                    │
│  force_freeze(17) / force_unfreeze(18)                   │
│  force_burn(19) / force_transfer(23)                     │
│  set_global_pause(20) / force_cancel_dividends(25)       │
├──────────────────────────────────────────────────────────┤
│  Owner / Admin (TOKEN_MANAGE 0b0000_0100)                │
│  create(0) / update_config(1) / mint(2)                  │
│  configure_dividend(4) / distribute(5)                   │
│  change_type(9) / set_max_supply(10)                     │
│  set_restriction(11) / whitelist(12-15)                  │
│  update_metadata(22)                                     │
│  + is_entity_locked 全局锁定检查                          │
├──────────────────────────────────────────────────────────┤
│  Holder (任何持有代币的账户)                               │
│  transfer(3) / burn(21) / claim_dividend(6)              │
│  lock(7) / unlock(8) / approve(26) / transfer_from(27)  │
└──────────────────────────────────────────────────────────┘
```

## EntityTokenProvider (12 方法)

供 order / tokensale / governance / market 跨 pallet 调用。

| 方法 | 说明 | 策略检查 |
|------|------|---------|
| `is_token_enabled` | 查 config.enabled | — |
| `token_balance` | Assets::balance | — |
| `available_balance` | balance − locked − reserved | — |
| `reward_on_purchase` | 计算奖励 + mint | GlobalPaused 静默跳过 |
| `redeem_for_discount` | 检查限额 + burn | GlobalPaused 拒绝 |
| `transfer` | Assets::transfer | zero / pause / frozen / active / enabled / transferable / restriction |
| `reserve` | 检查可用余额 → ReservedTokens | zero check |
| `unreserve` | 减少 ReservedTokens | min(amount, current) |
| `repatriate_reserved` | Assets::transfer(Expendable) | **绕过所有策略** |
| `get_token_type` | 查 config.token_type | 默认 Points |
| `total_supply` | Assets::total_issuance | — |
| `governance_burn` | entity_account burn_from | zero / enabled check |

> `repatriate_reserved` 直接调用底层 `Assets::transfer`，绕过 GlobalPaused / TransfersFrozen / EntityNotActive。预留代币是已承诺资金（佣金托管、订单押金），释放时不应被策略阻拦。使用 `Expendable` 允许账户清零。

## Events (28)

| 事件 | 字段 | 触发 |
|------|------|------|
| `EntityTokenCreated` | entity_id, asset_id, name, symbol | create_shop_token |
| `TokenConfigUpdated` | entity_id | update_token_config / set_max_supply |
| `TokensMinted` | entity_id, to, amount | mint_tokens |
| `TokensBurned` | entity_id, holder, amount | burn_tokens |
| `TokensTransferred` | entity_id, from, to, amount | transfer_tokens |
| `TokenApprovalSet` | entity_id, owner, spender, amount | approve_tokens |
| `TokensTransferredFrom` | entity_id, owner, spender, to, amount | transfer_from |
| `RewardIssued` | entity_id, buyer, amount | reward_on_purchase |
| `TokensRedeemed` | entity_id, buyer, tokens, discount | redeem_for_discount |
| `DividendConfigured` | entity_id, enabled, min_period | configure_dividend |
| `DividendDistributed` | entity_id, total_amount, recipients_count | distribute_dividend |
| `DividendClaimed` | entity_id, holder, amount | claim_dividend |
| `TokensLocked` | entity_id, holder, amount, unlock_at | lock_tokens |
| `TokensUnlocked` | entity_id, holder, amount | unlock_tokens |
| `TokenTypeChanged` | entity_id, old_type, new_type | change_token_type |
| `TransferRestrictionSet` | entity_id, mode, min_receiver_kyc | set_transfer_restriction |
| `WhitelistUpdated` | entity_id, added, removed | add/remove whitelist |
| `BlacklistUpdated` | entity_id, added, removed | add/remove blacklist |
| `TokenMetadataUpdated` | entity_id, name, symbol | update_token_metadata |
| `TokenForceDisabled` | entity_id | force_disable_token |
| `TokenForceEnabled` | entity_id | force_enable_token |
| `TransfersFrozenEvent` | entity_id | force_freeze_transfers |
| `TransfersUnfrozen` | entity_id | force_unfreeze_transfers |
| `TokensForceBurned` | entity_id, from, amount | force_burn |
| `TokensForceTransferred` | entity_id, from, to, amount | force_transfer |
| `GlobalTokenPauseSet` | paused | set_global_token_pause |
| `PendingDividendsCancelled` | entity_id, total_cancelled, accounts_affected | force_cancel_pending_dividends |
| `TokensGovernanceBurned` | entity_id, from, amount | governance_burn |

## Errors (53)

| 分类 | 错误 | 说明 |
|------|------|------|
| **通用** | `EntityNotFound` | 实体不存在 |
| | `EntityNotActive` | 实体未激活 |
| | `EntityLocked` | 实体已全局锁定 |
| | `NotAuthorized` | 非 Owner 且无 TOKEN_MANAGE 权限 |
| | `ZeroAmount` | 数量为零 |
| | `SelfTransfer` | 自转账（from == to） |
| **代币状态** | `TokenNotEnabled` | 代币未启用或不存在 |
| | `TokenAlreadyExists` | 代币已存在 |
| | `TokenAlreadyDisabled` | 已处于禁用状态 |
| | `TokenAlreadyEnabled` | 已处于启用状态 |
| | `AssetCreationFailed` | pallet-assets 创建失败 |
| | `AssetIdOverflow` | ShopTokenOffset + entity_id 溢出 |
| | `TokenCountOverflow` | 代币总数计数溢出 |
| **余额** | `InsufficientBalance` | 可用余额不足 |
| | `InsufficientAllowance` | 授权额度不足 |
| **配置** | `InvalidRewardRate` | 奖励率 > 10000 |
| | `InvalidExchangeRate` | 兑换率 > 10000 |
| | `InvalidRedeemLimits` | min_redeem > max_redeem |
| | `EmptyName` / `EmptySymbol` | 名称或符号为空 |
| | `NameTooLong` / `SymbolTooLong` | 超过 MaxLength |
| **转账** | `TransferNotAllowed` | transferable = false |
| | `TokenTransfersFrozen` | 转账已冻结 |
| | `GlobalPaused` | 全平台已暂停 |
| | `InsiderTradingRestricted` | 黑窗口期内幕交易 |
| | `TransfersNotFrozen` | 未冻结，无需解冻 |
| | `TransfersAlreadyFrozen` | 已冻结，无需重复冻结 |
| **限制** | `SenderNotInWhitelist` / `ReceiverNotInWhitelist` | 白名单限制 |
| | `SenderInBlacklist` / `ReceiverInBlacklist` | 黑名单限制 |
| | `SenderKycInsufficient` / `ReceiverKycInsufficient` | KYC 不足 |
| | `SenderNotMember` / `ReceiverNotMember` | 非成员 |
| | `TransferListFull` | 名单已满 |
| **分红** | `DividendNotEnabled` | 分红未启用 |
| | `DividendPeriodNotReached` | 分红周期未到 |
| | `NoDividendToClaim` | 无待领取分红 |
| | `ZeroDividendAmount` | 分红总额为零 |
| | `DividendAmountMismatch` | total ≠ sum(recipients) |
| | `TooManyRecipients` | 超 MaxDividendRecipients |
| | `NoPendingDividendsToCancel` | 无待取消的分红 |
| **锁仓** | `NoLockedTokens` | 无锁仓记录 |
| | `UnlockTimeNotReached` | 解锁时间未到 |
| | `LocksFull` | 锁仓条目已满（10 条） |
| | `InvalidLockDuration` | 锁仓时长为零 |
| **供应** | `ExceedsMaxSupply` | 超过最大供应量 |
| **类型** | `TokenTypeNotSupported` | 类型不支持此操作 |
| | `SameTokenType` | 变更为相同类型 |
| **兑换** | `BelowMinRedeem` | 低于最低兑换门槛 |
| | `ExceedsMaxRedeem` | 超过单笔最大兑换 |

## 与其他模块的集成

```
pallet-entity-order ──────→ EntityTokenProvider ──→ pallet-entity-token
  reward_on_purchase                                      │
  redeem_for_discount                                     │ fungibles traits
  reserve / unreserve / repatriate                        ▼
                                                    pallet-assets
pallet-entity-tokensale ──→ EntityTokenProvider
  reserve / unreserve / repatriate

pallet-entity-governance ─→ EntityTokenProvider
  token_balance / get_token_type
  reserve / unreserve / governance_burn

pallet-entity-market ─────→ EntityTokenProvider
  token_balance / reserve / unreserve / repatriate

pallet-entity-disclosure ─→ DisclosureProvider
  can_insider_trade（黑窗口期检查）
```

## 测试

```bash
cargo test -p pallet-entity-token
# 151 个单元测试
```

### 覆盖范围

| 分类 | 覆盖内容 |
|------|---------|
| 创建 | 正常创建、不存在 / 未激活 / 非所有者 / 重复 / 空名称符号 / 无效比率 / Entity 锁定 |
| 配置 | 更新成功、min > max 拒绝 |
| 铸造 | 正常铸造、超 max_supply（含 pending）、Entity 不活跃 |
| 转账 | 正常转账、锁仓拦截、预留拦截、零数量拒绝、**自转账拒绝**、Entity 不活跃 |
| 授权 | approve + transfer_from 完整流程、额度不足拒绝、自授权拒绝、**自转账拒绝** |
| 分红 | 配置 / 分发 / 领取、金额不匹配、人数超限、类型不支持、超 max_supply、零总额、Entity 不活跃、**禁用后 claim 拒绝** |
| 锁仓 | 锁仓 + 解锁、零量 / 零时长、未启用、部分到期解锁、Entity 不活跃时仍可 unlock/claim |
| 销毁 | 正常销毁、全局暂停拒绝、零数量拒绝、余额不足 |
| 元数据 | 更新成功、空名称 / 符号拒绝、非所有者拒绝 |
| 类型 | 变更联动 transfer_restriction、相同类型拒绝 |
| 供应 | set_max_supply（含 pending 检查）、低于当前拒绝 |
| 限制 | KYC clamped、5 种转账限制模式、白/黑名单 CRUD、输入长度限制 |
| 名单查询 | **is_whitelisted / is_blacklisted** |
| 内幕交易 | **黑窗口期拦截、非内幕人员放行、窗口期解除后放行** |
| Trait | reserve / unreserve / repatriate 完整流程、trait transfer zero 拒绝、trait reserve zero 拒绝、全局暂停 / 冻结 / 可用余额检查 |
| Admin | 15 个管理员权限测试 |
| force | disable / enable / freeze / unfreeze 幂等性 |
| force_burn | 正常 / 全额 / 零额 / 非 Root / 存储清理 |
| force_transfer | 正常 / 零额 / 非 Root / 存储清理 |
| global_pause | 暂停 / 恢复、阻止 create / mint / transfer / claim / redeem、reward 静默 0 |
| repatriate | Entity 不活跃 / 转账冻结 / 全局暂停 / 全额预留 4 种场景 |
| cancel_dividends | **正常取消 / 非 Root 拒绝 / 无可取消拒绝** |

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-01-31 | 初始版本：创建 / 配置 / 铸造 / 转让 / 奖励 / 兑换 |
| v0.2.0 | 2026-02-03 | 7 种通证类型、分红、锁仓 / 解锁、最大供应量 |
| v0.3.0 | 2026-02-04 | 转账限制（5 种模式）、白/黑名单、KYC / 成员集成 |
| v0.4.0 | 2026-02-09 | 审计 R1：max_supply 全链路校验、锁仓+预留余额拦截、ReservedTokens 真实实现、分红安全加固（41 tests） |
| v0.5.0 | 2026-03 | Admin 权限：TOKEN_MANAGE 管理员支持（75 tests） |
| v0.6.0 | 2026-03 | 紧急管控：force_disable / freeze / burn、set_global_token_pause（104 tests） |
| v0.7.0 | 2026-03 | burn_tokens / update_metadata / force_transfer / force_enable；LockEntry 独立条目；双向转账限制；DisclosureProvider 内幕交易防护（138 tests） |
| v0.7.1 | 2026-03 | 审计 R2：repatriate_reserved 绕过策略检查(Expendable)（138 tests） |
| v0.7.2 | 2026-03 | 审计 R3：移除死代码、trait transfer/reserve 零数量检查（140 tests） |
| v0.8.0 | 2026-03 | 审计 R4：claim_dividend 启用检查、自转账拒绝、资产 ID 溢出保护、force_cancel_pending_dividends、approve/transfer_from 授权转账、白/黑名单查询、可用余额统一 helper、Shop 别名清理、Hybrid 简化、内幕交易可配置 mock（151 tests） |

## 许可证

MIT License
