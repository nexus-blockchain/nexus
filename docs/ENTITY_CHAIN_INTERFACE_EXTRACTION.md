# Entity Pallet Chain-Side Interface Extraction

> Complete extraction of all dispatchable functions, storage items, events, and key types
> across all entity pallet modules. For frontend API mapping.

---

## 1. pallet-entity-common (Shared Types & Traits)

### Enums

| Enum | Variants |
|------|----------|
| `EntityType` | `Merchant` (default), `Enterprise`, `DAO`, `Community`, `Project`, `ServiceProvider`, `Fund`, `Custom(u8)` |
| `EntityStatus` | `Pending` (default), `Active`, `Suspended`, `Banned`, `Closed`, `PendingClose` |
| `GovernanceMode` | `None` (default), `FullDAO` |
| `ShopType` | `OnlineStore` (default), `PhysicalStore`, `ServicePoint`, `Warehouse`, `Franchise`, `Popup`, `Virtual` |
| `ShopOperatingStatus` | `Active` (default), `Paused`, `FundDepleted`, `Closed`, `Closing`, `Banned` |
| `EffectiveShopStatus` | `Active`, `PausedBySelf`, `PausedByEntity`, `FundDepleted`, `Closed`, `ClosedByEntity`, `Closing`, `Banned` |
| `TokenType` | `Points` (default), `Governance`, `Equity`, `Membership`, `Share`, `Bond`, `Hybrid(u8)` |
| `TransferRestrictionMode` | `None` (default), `Whitelist`, `Blacklist`, `KycRequired`, `MembersOnly` |
| `ProductStatus` | `Draft` (default), `OnSale`, `SoldOut`, `OffShelf` |
| `ProductVisibility` | `Public` (default), `MembersOnly`, `LevelGated(u8)` |
| `ProductCategory` | `Digital`, `Physical` (default), `Service`, `Subscription`, `Bundle`, `Other` |
| `OrderStatus` | `Created` (default), `Paid`, `Shipped`, `Completed`, `Cancelled`, `Disputed`, `Refunded`, `Expired` |
| `MemberStatus` | `Active` (default), `Pending`, `Frozen`, `Banned`, `Expired` |
| `DisputeStatus` | `None` (default), `Submitted`, `Responded`, `Mediating`, `Arbitrating`, `Resolved`, `Withdrawn`, `Expired` |
| `DisputeResolution` | `ComplainantWin`, `RespondentWin`, `Settlement` |
| `TokenSaleStatus` | `NotStarted` (default), `Active`, `Paused`, `Ended`, `Cancelled`, `Completed` |
| `PaymentAsset` | `Native` (default), `EntityToken` |
| `DisclosureLevel` | `Basic` (default), `Standard`, `Enhanced`, `Full` |
| `ServiceType` | `General` (default), `Consulting`, `Technical`, `Education`, `Creative`, `Other` |
| `ServiceStatus` | `Draft` (default), `Available`, `Suspended`, `Delisted` |

### Structs

| Struct | Key Fields |
|--------|------------|
| `PageRequest` | `offset: u32`, `limit: u32` |
| `PageResponse<T>` | `items: Vec<T>`, `total: u32`, `has_more: bool` |
| `MemberRegistrationPolicy(u8)` | Bit flags: `PURCHASE_REQUIRED=0x01`, `REFERRAL_REQUIRED=0x02`, `APPROVAL_REQUIRED=0x04`, `KYC_REQUIRED=0x08`, `KYC_UPGRADE_REQUIRED=0x10` |
| `MemberStatsPolicy(u8)` | Bit flags: `INCLUDE_REPURCHASE_DIRECT=0x01`, `INCLUDE_REPURCHASE_INDIRECT=0x02` |
| `DividendConfig<Balance, BlockNumber>` | `enabled: bool`, `min_period`, `last_distribution`, `accumulated` |
| `VestingSchedule` | `total: u128`, `released: u128`, `start_block: u64`, `cliff_blocks: u64`, `vesting_blocks: u64` |
| `MemberLevelInfo` | `level_id: u8`, `name: Vec<u8>`, `threshold: u64`, `discount_rate: u16`, `commission_bonus: u16` |

### AdminPermission (module, u32 bitmask)

| Permission | Value |
|-----------|-------|
| `SHOP_MANAGE` | `0x01` |
| `MEMBER_MANAGE` | `0x02` |
| `TOKEN_MANAGE` | `0x04` |
| `ADS_MANAGE` | `0x08` |
| `REVIEW_MANAGE` | `0x10` |
| `DISCLOSURE_MANAGE` | `0x20` |
| `ENTITY_MANAGE` | `0x40` |
| `KYC_MANAGE` | `0x80` |
| `GOVERNANCE_MANAGE` | `0x100` |
| `ORDER_MANAGE` | `0x200` |
| `COMMISSION_MANAGE` | `0x400` |
| `ALL` | `0xFFFFFFFF` |

---

## 2. pallet-entity-registry

### Dispatchable Functions (call_index)

| # | Function | Parameters | Origin |
|---|----------|-----------|--------|
| 0 | `create_entity` | `name: Vec<u8>`, `logo_cid: Option<Vec<u8>>`, `description_cid: Option<Vec<u8>>`, `referrer: Option<AccountId>` | Signed |
| 1 | `update_entity` | `entity_id: u64`, `name: Option<Vec<u8>>`, `logo_cid: Option<Vec<u8>>`, `description_cid: Option<Vec<u8>>`, `metadata_uri: Option<Vec<u8>>`, `contact_cid: Option<Vec<u8>>` | Signed (owner/admin) |
| 2 | `request_close_entity` | `entity_id: u64` | Signed (owner) |
| 3 | `top_up_fund` | `entity_id: u64`, `amount: Balance` | Signed |
| 4 | `approve_entity` | `entity_id: u64` | Governance |
| 5 | `approve_close_entity` | `entity_id: u64` | Governance |
| 6 | `suspend_entity` | `entity_id: u64`, `reason: Option<Vec<u8>>` | Governance |
| 7 | `resume_entity` | `entity_id: u64` | Governance |
| 8 | `ban_entity` | `entity_id: u64`, `confiscate_fund: bool`, `reason: Option<Vec<u8>>` | Governance |
| 9 | `add_admin` | `entity_id: u64`, `new_admin: AccountId`, `permissions: u32` | Signed (owner) |
| 10 | `remove_admin` | `entity_id: u64`, `admin: AccountId` | Signed (owner) |
| 11 | `transfer_ownership` | `entity_id: u64`, `new_owner: AccountId` | Signed (owner) |
| 12 | `upgrade_entity_type` | `entity_id: u64`, `new_type: EntityType`, `new_governance: GovernanceMode` | Governance or Signed |
| 14 | `verify_entity` | `entity_id: u64` | Governance |
| 15 | `reopen_entity` | `entity_id: u64` | Signed (owner) |
| 16 | `bind_entity_referrer` | `entity_id: u64`, `referrer: AccountId` | Signed (owner) |
| 17 | `update_admin_permissions` | `entity_id: u64`, `admin: AccountId`, `new_permissions: u32` | Signed (owner) |
| 18 | `unban_entity` | `entity_id: u64` | Governance |
| 19 | `unverify_entity` | `entity_id: u64` | Governance |
| 20 | `cancel_close_request` | `entity_id: u64` | Signed (owner) |
| 21 | `resign_admin` | `entity_id: u64` | Signed (admin self) |
| 22 | `set_primary_shop` | `entity_id: u64`, `shop_id: u64` | Signed (owner/admin) |
| 23 | `self_pause_entity` | `entity_id: u64` | Signed (owner) |
| 24 | `self_resume_entity` | `entity_id: u64` | Signed (owner) |
| 25 | `force_transfer_ownership` | `entity_id: u64`, `new_owner: AccountId` | Governance |
| 26 | `reject_close_request` | `entity_id: u64` | Governance |
| 27 | `execute_close_timeout` | `entity_id: u64` | Signed (anyone) |

### Storage Items

| Storage | Type | Key → Value |
|---------|------|-------------|
| `NextEntityId` | `StorageValue` | `→ u64` |
| `Entities` | `StorageMap` | `u64 → Entity` |
| `UserEntity` | `StorageMap` | `AccountId → BoundedVec<u64>` |
| `EntityStats` | `StorageValue` | `→ EntityStatistics` |
| `EntityCloseRequests` | `StorageMap` | `u64 → BlockNumber` |
| `GovernanceSuspended` | `StorageMap` | `u64 → bool` |
| `EntityReferrer` | `StorageMap` | `u64 → AccountId` |
| `OwnerPaused` | `StorageMap` | `u64 → bool` |
| `ReferrerEntities` | `StorageMap` | `AccountId → BoundedVec<u64>` |
| `EntitySales` | `StorageMap` | `u64 → EntitySalesData` |
| `EntityShops` | `StorageMap` | `u64 → BoundedVec<u64>` |
| `EntityNameIndex` | `StorageMap` | `BoundedVec<u8> → u64` |
| `SuspensionReasons` | `StorageMap` | `u64 → BoundedVec<u8, 256>` |

### Events

`EntityCreated`, `ShopAddedToEntity`, `EntityUpdated`, `EntityStatusChanged`, `FundToppedUp`, `OperatingFeeDeducted`, `FundWarning`, `EntitySuspendedLowFund`, `EntityResumedAfterFunding`, `EntityCloseRequested`, `EntityClosed`, `EntityBanned`, `FundConfiscated`, `AdminAdded`, `AdminRemoved`, `AdminPermissionsUpdated`, `EntityTypeUpgraded`, `GovernanceModeChanged`, `EntityVerified`, `EntityReopened`, `OwnershipTransferred`, `ShopCascadeFailed`, `EntityReferrerBound`, `FundRefundFailed`, `ShopRemovedFromEntity`, `EntityUnbanned`, `EntityUnverified`, `CloseRequestCancelled`, `AdminResigned`, `PrimaryShopChanged`, `EntityOwnerPaused`, `EntityOwnerResumed`, `OwnershipForceTransferred`, `CloseRequestRejected`, `EntitySuspendedWithReason`, `CloseRequestAutoExecuted`

---

## 3. pallet-entity-shop

### Dispatchable Functions

| # | Function | Parameters | Origin |
|---|----------|-----------|--------|
| 0 | `create_shop` | `entity_id: u64`, `name: BoundedVec`, `shop_type: ShopType`, `initial_fund: Balance` | Signed (entity owner) |
| 1 | `update_shop` | `shop_id: u64`, `name`, `logo_cid`, `description_cid` (all Option) | Signed (manager) |
| 2 | `add_manager` | `shop_id: u64`, `manager: AccountId` | Signed (entity owner) |
| 3 | `remove_manager` | `shop_id: u64`, `manager: AccountId` | Signed (entity owner) |
| 4 | `fund_operating` | `shop_id: u64`, `amount: Balance` | Signed (manager) |
| 5 | `pause_shop` | `shop_id: u64` | Signed (manager) |
| 6 | `resume_shop` | `shop_id: u64` | Signed (manager) |
| 7 | `set_location` | `shop_id: u64`, `location: Option<(i64,i64)>`, `address_cid`, `business_hours_cid` | Signed (manager) |
| 8 | `enable_points` | `shop_id: u64`, `name`, `symbol`, `reward_rate: u16`, `exchange_rate: u16`, `transferable: bool` | Signed (manager) |
| 9 | `close_shop` | `shop_id: u64` | Signed (entity owner) |
| 10 | `disable_points` | `shop_id: u64` | Signed (manager) |
| 11 | `update_points_config` | `shop_id: u64`, `reward_rate`, `exchange_rate`, `transferable` (all Option) | Signed (manager) |
| 12 | `transfer_points` | `shop_id: u64`, `to: AccountId`, `amount: Balance` | Signed (user) |
| 13 | `withdraw_operating_fund` | `shop_id: u64`, `amount: Balance` | Signed (entity owner) |
| 14 | `set_customer_service` | `shop_id: u64`, `customer_service: Option<AccountId>` | Signed (manager) |
| 15 | `finalize_close_shop` | `shop_id: u64` | Signed (anyone) |
| 16 | `manager_issue_points` | `shop_id: u64`, `to: AccountId`, `amount: Balance` | Signed (manager) |
| 17 | `manager_burn_points` | `shop_id: u64`, `from: AccountId`, `amount: Balance` | Signed (manager) |
| 18 | `redeem_points` | `shop_id: u64`, `amount: Balance` | Signed (user) |
| 19 | `transfer_shop` | `shop_id: u64`, `to_entity_id: u64` | Signed (entity owner) |
| 20 | `set_primary_shop` | `entity_id: u64`, `new_primary_shop_id: u64` | Signed (entity owner) |
| 21 | `force_pause_shop` | `shop_id: u64` | Root |
| 22 | `set_points_ttl` | `shop_id: u64`, `ttl_blocks: BlockNumber` | Signed (manager) |
| 23 | `expire_points` | `shop_id: u64`, `account: AccountId` | Signed (anyone) |
| 24 | `force_close_shop` | `shop_id: u64` | Root |
| 25 | `set_business_hours` | `shop_id: u64`, `business_hours_cid: Option<BoundedVec>` | Signed (manager) |
| 26 | `set_shop_policies` | `shop_id: u64`, `policies_cid: Option<BoundedVec>` | Signed (manager) |
| 27 | `set_shop_type` | `shop_id: u64`, `new_shop_type: ShopType` | Signed (entity owner) |
| 28 | `cancel_close_shop` | `shop_id: u64` | Signed (entity owner) |
| 29 | `set_points_max_supply` | `shop_id: u64`, `max_supply: Balance` | Signed (manager) |
| 30 | `resign_manager` | `shop_id: u64` | Signed (manager self) |
| 31 | `ban_shop` | `shop_id: u64` | Root |
| 32 | `unban_shop` | `shop_id: u64` | Root |

### Storage Items

`Shops` (u64→Shop), `ShopEntity` (u64→u64), `NextShopId`, `ShopClosingAt`, `ShopPointsConfigs`, `ShopPointsBalances` (u64,AccountId→Balance), `ShopPointsTotalSupply`, `ShopPointsTtl`, `ShopPointsExpiresAt`, `ShopPointsMaxSupply`, `EntityPrimaryShop`, `ShopStatusBeforeBan`

---

## 4. pallet-entity-product

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `create_product` | `shop_id`, `name_cid`, `images_cid`, `detail_cid`, `price`, `usdt_price: u64`, `stock: u32`, `category: ProductCategory`, `sort_weight: u32`, `tags_cid`, `sku_cid`, `min_order_quantity: u32`, `max_order_quantity: u32`, `visibility: ProductVisibility` |
| 1 | `update_product` | `product_id`, all fields as Option |
| 2 | `publish_product` | `product_id: u64` |
| 3 | `unpublish_product` | `product_id: u64` |
| 4 | `delete_product` | `product_id: u64` |
| 5 | `force_unpublish_product` | `product_id: u64`, `reason: Option<Vec<u8>>` (Root) |
| 6 | `batch_publish_products` | `product_ids: Vec<u64>` |
| 7 | `batch_unpublish_products` | `product_ids: Vec<u64>` |
| 8 | `batch_delete_products` | `product_ids: Vec<u64>` |

### Storage Items

`NextProductId`, `Products` (u64→Product), `ShopProducts` (u64→BoundedVec<u64>), `ProductStats`, `ProductDeposits`

---

## 5. pallet-entity-order

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `place_order` | `product_id: u64`, `quantity: u32`, `shipping_cid: Option<Vec<u8>>`, `use_tokens: Option<Balance>`, `use_shopping_balance: Option<Balance>`, `payment_asset: Option<PaymentAsset>`, `note_cid: Option<Vec<u8>>` |
| 1 | `cancel_order` | `order_id: u64` |
| 2 | `ship_order` | `order_id: u64`, `tracking_cid: Vec<u8>` |
| 3 | `confirm_receipt` | `order_id: u64` |
| 4 | `request_refund` | `order_id: u64`, `reason_cid: Vec<u8>` |
| 5 | `approve_refund` | `order_id: u64` |
| 6 | `start_service` | `order_id: u64` |
| 7 | `complete_service` | `order_id: u64` |
| 8 | `confirm_service` | `order_id: u64` |
| 9 | `set_platform_fee_rate` | `new_rate: u16` (Root) |
| 10 | `cleanup_buyer_orders` | (none) |
| 11 | `reject_refund` | `order_id: u64`, `reason_cid: Vec<u8>` |
| 12 | `seller_cancel_order` | `order_id: u64`, `reason_cid: Vec<u8>` |
| 13 | `force_refund` | `order_id: u64`, `reason_cid: Option<Vec<u8>>` (Root) |
| 14 | `force_complete` | `order_id: u64`, `reason_cid: Option<Vec<u8>>` (Root) |
| 15 | `update_shipping_address` | `order_id: u64`, `new_shipping_cid: Vec<u8>` |
| 16 | `extend_confirm_timeout` | `order_id: u64` |
| 17 | `cleanup_shop_orders` | `shop_id: u64` |
| 18 | `update_tracking` | `order_id: u64`, `new_tracking_cid: Vec<u8>` |

### Storage Items

`PlatformFeeRate` (u16, default 100), `NextOrderId`, `Orders`, `BuyerOrders` (AccountId→BoundedVec<u64,1000>), `ShopOrders` (u64→BoundedVec<u64,10000>), `OrderStats`, `ExpiryQueue` (BlockNumber→BoundedVec<u64,500>)

---

## 6. pallet-entity-review

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `submit_review` | `order_id: u64`, `rating: u8` (1-5), `content_cid: Option<Vec<u8>>` |
| 1 | `set_review_enabled` | `entity_id: u64`, `enabled: bool` |
| 2 | `remove_review` | `order_id: u64` (Root) |
| 3 | `reply_to_review` | `order_id: u64`, `content_cid: Vec<u8>` |
| 4 | `edit_review` | `order_id: u64`, `new_rating: u8`, `new_content_cid: Option<Vec<u8>>` |

### Storage Items

`Reviews` (u64→MallReview), `ReviewCount`, `ShopReviewCount`, `UserReviews`, `EntityReviewDisabled`, `ReviewReplies`, `ProductReviews`, `ProductReviewCount`, `ProductRatingSum`

---

## 7. pallet-entity-token

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `create_shop_token` | `entity_id: u64`, `name: Vec<u8>`, `symbol: Vec<u8>`, `decimals: u8`, `reward_rate: u16`, `exchange_rate: u16` |
| 1 | `update_token_config` | `entity_id: u64`, `reward_rate`, `exchange_rate`, `min_redeem`, `max_redeem_per_order`, `transferable`, `enabled` (all Option) |
| 2 | `mint_tokens` | `entity_id: u64`, `to: AccountId`, `amount: AssetBalance` |
| 3 | `transfer_tokens` | `entity_id: u64`, `to: AccountId`, `amount: AssetBalance` |
| 4 | `configure_dividend` | `entity_id: u64`, `enabled: bool`, `min_period: BlockNumber` |
| 5 | `distribute_dividend` | `entity_id: u64`, `total_amount: AssetBalance`, `recipients: Vec<(AccountId, AssetBalance)>` |
| 6 | `claim_dividend` | `entity_id: u64` |
| 7 | `lock_tokens` | `entity_id: u64`, `amount: AssetBalance`, `lock_duration: BlockNumber` |
| 8 | `unlock_tokens` | `entity_id: u64` |
| 9 | `change_token_type` | `entity_id: u64`, `new_type: TokenType` |
| 10 | `set_max_supply` | `entity_id: u64`, `max_supply: AssetBalance` |
| 11 | `set_transfer_restriction` | `entity_id: u64`, `mode: TransferRestrictionMode`, `min_receiver_kyc: u8` |
| 12 | `add_to_whitelist` | `entity_id: u64`, `accounts: Vec<AccountId>` |
| 13 | `remove_from_whitelist` | `entity_id: u64`, `accounts: Vec<AccountId>` |
| 14 | `add_to_blacklist` | `entity_id: u64`, `accounts: Vec<AccountId>` |
| 15 | `remove_from_blacklist` | `entity_id: u64`, `accounts: Vec<AccountId>` |
| 16 | `force_disable_token` | `entity_id: u64` (Root) |
| 17 | `force_freeze_transfers` | `entity_id: u64` (Root) |
| 18 | `force_unfreeze_transfers` | `entity_id: u64` (Root) |
| 19 | `force_burn` | `entity_id: u64`, `from: AccountId`, `amount: AssetBalance` (Root) |
| 20 | `set_global_token_pause` | `paused: bool` (Root) |
| 21 | `burn_tokens` | `entity_id: u64`, `amount: AssetBalance` |
| 22 | `update_token_metadata` | `entity_id: u64`, `name: Vec<u8>`, `symbol: Vec<u8>` |
| 23 | `force_transfer` | `entity_id: u64`, `from: AccountId`, `to: AccountId`, `amount: AssetBalance` (Root) |
| 24 | `force_enable_token` | `entity_id: u64` (Root) |

### Storage Items

`EntityTokenConfigs`, `EntityTokenMetadata`, `TotalEntityTokens`, `LockedTokens` (u64,AccountId→BoundedVec<LockEntry,10>), `PendingDividends`, `ClaimedDividends`, `TotalPendingDividends`, `TransferWhitelist`, `TransferBlacklist`, `ReservedTokens`, `TransfersFrozen`, `GlobalTokenPaused`

---

## 8. pallet-entity-market

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `place_sell_order` | `entity_id: u64`, `token_amount`, `price` |
| 1 | `place_buy_order` | `entity_id: u64`, `token_amount`, `price` |
| 2 | `take_order` | `order_id: u64`, `amount` |
| 3 | `cancel_order` | `order_id: u64` |
| 4 | `configure_market` | `entity_id: u64`, `nex_enabled: bool`, `min_order_amount: u128`, `order_ttl: u32` |
| 5 | `configure_price_protection` | `entity_id`, `max_deviation_bps`, `circuit_breaker_threshold`, `cooldown_blocks` |
| 6 | `lift_circuit_breaker` | `entity_id: u64` |
| 7 | `set_initial_price` | `entity_id: u64`, `price` |
| 8 | `market_buy` | `entity_id: u64`, `max_nex_spend`, `min_token_amount` |
| 9 | `market_sell` | `entity_id: u64`, `token_amount`, `min_nex_receive` |
| 10 | `force_cancel_order` | `order_id: u64` (Root) |
| 11 | `pause_market` | `entity_id: u64` |
| 12 | `resume_market` | `entity_id: u64` |
| 13 | `batch_cancel_orders` | `order_ids: BoundedVec<u64, 50>` |
| 14 | `cleanup_expired_orders` | `entity_id: u64`, `max_count: u32` |
| 15 | `modify_order` | `order_id: u64`, `new_price`, `new_amount` |
| 16 | `global_market_pause` | `paused: bool` (Root) |
| 17 | `set_kyc_requirement` | `entity_id: u64`, `min_kyc_level: u8` |
| 18 | `close_market` | `entity_id: u64` |
| 19 | `cancel_all_entity_orders` | `entity_id: u64` |
| 20 | `governance_configure_market` | `entity_id`, params |
| 21 | `force_close_market` | `entity_id: u64` (Root) |
| 22 | `place_ioc_order` | `entity_id`, `side`, `token_amount`, `price` |
| 23 | `place_fok_order` | `entity_id`, `side`, `token_amount`, `price` |
| 24 | `place_post_only_order` | `entity_id`, `side`, `token_amount`, `price` |

### Key Enums

- `OrderSide`: `Buy`, `Sell`
- `OrderType`: `Limit`, `Market`, `ImmediateOrCancel`, `FillOrKill`, `PostOnly`
- `OrderStatus`: `Open`, `PartiallyFilled`, `Filled`, `Cancelled`, `Expired`

### Storage Items

`NextOrderId`, `Orders`, `EntitySellOrders`, `EntityBuyOrders`, `UserOrders`, `MarketConfigs`, `MarketStatsStorage`, `BestAsk`, `BestBid`, `LastTradePrice`, `GlobalMarketPaused`, `TwapAccumulators`, `PriceProtection`, `NextTradeId`, `TradeRecords`, `UserTradeHistory`, `EntityTradeHistory`, `UserOrderHistory`, `EntityDailyStats`, `MarketKycRequirement`, `OnIdleCursor`, `MarketStatusStorage`, `GlobalStats`

---

## 9. pallet-entity-member

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `register_member` | `shop_id: u64`, `referrer: Option<AccountId>` |
| 1 | `bind_referrer` | `shop_id: u64`, `referrer: AccountId` |
| 2 | `init_level_system` | `shop_id: u64`, `use_custom: bool` |
| 3 | `add_custom_level` | `shop_id: u64`, `name`, `threshold`, `discount_rate`, `commission_bonus` |
| 4 | `update_custom_level` | `shop_id: u64`, `level_id: u8`, params... |
| 5 | `remove_custom_level` | `shop_id: u64`, `level_id: u8` |
| 6 | `manual_set_member_level` | `shop_id: u64`, `account: AccountId`, `level_id: u8` |
| 7 | `set_use_custom_levels` | `shop_id: u64`, `use_custom: bool` |
| 8 | `set_upgrade_mode` | `shop_id: u64`, `mode: u8` |
| 9 | `init_upgrade_rule_system` | `shop_id: u64` |
| 10 | `add_upgrade_rule` | `shop_id: u64`, rule params... |
| 11 | `update_upgrade_rule` | `shop_id: u64`, `rule_id: u8`, params... |
| 12 | `remove_upgrade_rule` | `shop_id: u64`, `rule_id: u8` |
| 13 | `set_upgrade_rule_system_enabled` | `shop_id: u64`, `enabled: bool` |
| 14 | `set_conflict_strategy` | `shop_id: u64`, `strategy: u8` |
| 15 | `set_member_policy` | `shop_id: u64`, `policy_bits: u8` |
| 16 | `approve_member` | `shop_id: u64`, `account: AccountId` |
| 17 | `reject_member` | `shop_id: u64`, `account: AccountId` |
| 18 | `cancel_pending_member` | `shop_id: u64` |
| 19 | `cleanup_expired_pending` | `entity_id: u64`, `max_count: u32` |
| 20 | `set_member_stats_policy` | `shop_id: u64`, `policy_bits: u8` |
| 21 | `batch_approve_members` | `shop_id: u64`, `accounts: Vec<AccountId>` |
| 22 | `batch_reject_members` | `shop_id: u64`, `accounts: Vec<AccountId>` |
| 23 | `ban_member` | `shop_id: u64`, `account: AccountId` |
| 24 | `unban_member` | `shop_id: u64`, `account: AccountId` |
| 25 | `remove_member` | `shop_id: u64`, `account: AccountId` |
| 26 | `reset_level_system` | `shop_id: u64` |
| 27 | `reset_upgrade_rule_system` | `shop_id: u64` |
| 28 | `leave_entity` | `entity_id: u64` |
| 29 | `activate_member` | `shop_id: u64`, `account: AccountId` |
| 30 | `deactivate_member` | `shop_id: u64`, `account: AccountId` |

### Storage Items

`EntityMembers` (u64,AccountId→EntityMember), `MemberCount`, `LevelMemberCount`, `DirectReferrals`, `EntityLevelSystems`, `EntityUpgradeRules`, `MemberLevelExpiry`, `MemberUpgradeHistory`, `MemberOrderCount`, `EntityMemberPolicy`, `EntityMemberStatsPolicy`, `PendingMembers`, `BannedMemberCount`

---

## 10. pallet-entity-governance

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `create_proposal` | `entity_id: u64`, `title: Vec<u8>`, `description_cid: Vec<u8>`, `proposal_type: ProposalType`, `actions: Vec<ProposalAction>` |
| 1 | `vote` | `proposal_id: u64`, `approve: bool` |
| 2 | `finalize_voting` | `proposal_id: u64` |
| 3 | `execute_proposal` | `proposal_id: u64` |
| 4 | `cancel_proposal` | `proposal_id: u64` |
| 5 | `configure_governance` | `entity_id: u64`, config params... |
| 6 | `lock_governance` | `entity_id: u64` |
| 7 | `cleanup_proposal` | `proposal_id: u64` |
| 8 | `delegate_vote` | `entity_id: u64`, `delegate: AccountId` |
| 9 | `undelegate_vote` | `entity_id: u64` |
| 10 | `veto_proposal` | `proposal_id: u64` |
| 11 | `change_vote` | `proposal_id: u64`, `approve: bool` |
| 12 | `pause_governance` | `entity_id: u64` (Root) |
| 13 | `resume_governance` | `entity_id: u64` (Root) |
| 14 | `batch_cancel_proposals` | `entity_id: u64`, `proposal_ids: Vec<u64>` |

### Storage Items

`NextProposalId`, `Proposals`, `EntityProposals`, `VoteRecords`, `FirstHoldTime`, `VotingPowerSnapshot`, `GovernanceConfigs`, `GovernanceLocked`, `VoterTokenLocks`, `GovernanceLockCount`, `GovernanceLockAmount`, `ProposalScanCursor`, `GovernancePaused`, `VoteDelegation`, `DelegatedVoters`

---

## 11. pallet-entity-disclosure

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `configure_disclosure` | `entity_id`, `level: DisclosureLevel`, `insider_trading_control: bool`, `blackout_period_after: BlockNumber` |
| 1 | `publish_disclosure` | `entity_id`, `title_cid`, `content_cid`, `disclosure_type`, `is_material: bool` |
| 2 | `create_draft_disclosure` | `entity_id`, draft params... |
| 3 | `update_draft` | `disclosure_id`, update params... |
| 4 | `delete_draft` | `disclosure_id` |
| 5 | `publish_draft` | `disclosure_id` |
| 6 | `withdraw_disclosure` | `disclosure_id` |
| 7 | `correct_disclosure` | `old_disclosure_id`, correction params... |
| 8 | `add_insider` | `entity_id`, `account`, `role: InsiderRole` |
| 9 | `update_insider_role` | `entity_id`, `account`, `new_role` |
| 10 | `remove_insider` | `entity_id`, `account` |
| 11 | `start_blackout` | `entity_id`, `end_block: BlockNumber` |
| 12 | `end_blackout` | `entity_id` |
| 13 | `publish_announcement` | `entity_id`, `title_cid`, `content_cid`, `category`, `expires_at` |
| 14 | `update_announcement` | `announcement_id`, update params... |
| 15 | `withdraw_announcement` | `announcement_id` |
| 16 | `pin_announcement` | `entity_id`, `announcement_id` |
| 17 | `unpin_announcement` | `entity_id`, `announcement_id` |
| 18 | `expire_announcement` | `announcement_id` |
| 19 | `force_configure_disclosure` | (Root) |
| 20 | `report_disclosure_violation` | (Root) |
| 21 | `cleanup_disclosure_history` | `entity_id`, `max_count` |
| 22 | `cleanup_announcement_history` | `entity_id`, `max_count` |
| 23 | `cleanup_entity_disclosure` | `entity_id` |
| 24 | `batch_add_insiders` | `entity_id`, `insiders: Vec<(AccountId, InsiderRole)>` |
| 25 | `batch_remove_insiders` | `entity_id`, `accounts: Vec<AccountId>` |
| 26 | `reset_violation_count` | `entity_id` (Root) |
| 27 | `expire_blackout` | `entity_id` |

### Storage Items

`NextDisclosureId`, `Disclosures`, `DisclosureConfigs`, `EntityDisclosures`, `Insiders`, `BlackoutPeriods`, `NextAnnouncementId`, `Announcements`, `EntityAnnouncements`, `ViolationRecords`, `PinnedAnnouncements`, `InsiderRoleHistory`, `RemovedInsiders`, `AutoViolationCursor`, `HighRiskEntities`

---

## 12. pallet-entity-kyc

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `submit_kyc` | `level: KycLevel`, `data_cid: Vec<u8>`, `country_code: Option<[u8;2]>` |
| 1 | `approve_kyc` | `account: AccountId`, `level: KycLevel`, `notes: Option<Vec<u8>>` |
| 2 | `reject_kyc` | `account: AccountId`, `reason: Vec<u8>` |
| 3 | `revoke_kyc` | `account: AccountId`, `reason: Vec<u8>` |
| 4 | `register_provider` | `provider_account: AccountId`, `name`, `api_endpoint` |
| 5 | `remove_provider` | `provider_account: AccountId` |
| 6 | `set_entity_requirement` | `entity_id: u64`, requirement params... |
| 7 | `update_high_risk_countries` | `countries: BoundedVec<[u8;2], 50>` |
| 8 | `expire_kyc` | `account: AccountId` |
| 9 | `cancel_kyc` | (self) |
| 10 | `force_set_entity_requirement` | (Root) |
| 11 | `update_risk_score` | `account`, `score: u8` |
| 12 | `update_provider` | provider params... |
| 13 | `suspend_provider` | `provider_account` |
| 14 | `resume_provider` | `provider_account` |
| 15 | `force_approve_kyc` | (Root) |
| 16 | `renew_kyc` | `account`, `new_data_cid` |
| 17 | `update_kyc_data` | `new_data_cid: Vec<u8>` |
| 18 | `purge_kyc_data` | (self) |
| 19 | `remove_entity_requirement` | `entity_id: u64` |
| 20 | `timeout_pending_kyc` | `account` |
| 21 | `batch_revoke_by_provider` | `provider_account`, `accounts: Vec<AccountId>` |
| 22 | `force_remove_provider` | (Root) |

### Key Enums

- `KycLevel`: `None(0)`, `Basic(1)`, `Standard(2)`, `Enhanced(3)`, `Full(4)`
- `KycStatus`: `NotSubmitted(0)`, `Pending(1)`, `Approved(2)`, `Rejected(3)`, `Expired(4)`, `Revoked(5)`

### Storage Items

`KycRecords`, `Providers`, `ProviderCount`, `EntityRequirements`, `HighRiskCountries`, `KycHistory`, `PendingKycCount`, `ApprovedKycCount`

---

## 13. pallet-entity-tokensale

### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `create_sale_round` | `entity_id`, `name`, `total_supply`, `price`, `start_block`, `end_block`, `min_buy`, `max_buy_per_user`, `soft_cap`, `hard_cap` |
| 1 | `add_payment_option` | `round_id`, `asset_id`, `price` |
| 2 | `set_vesting_config` | `round_id`, `cliff_blocks`, `vesting_blocks` |
| 3 | `configure_dutch_auction` | `round_id`, `start_price`, `end_price`, `decay_blocks` |
| 4 | `add_to_whitelist` | `round_id`, `accounts: Vec<AccountId>` |
| 5 | `start_sale` | `round_id` |
| 6 | `subscribe` | `round_id`, `amount`, `payment_asset_id: Option` |
| 7 | `end_sale` | `round_id` |
| 8 | `claim_tokens` | `round_id` |
| 9 | `unlock_tokens` | `round_id` |
| 10 | `cancel_sale` | `round_id` |
| 11 | `claim_refund` | `round_id` |
| 12 | `reclaim_unclaimed_tokens` | `round_id` |
| 13 | `withdraw_funds` | `round_id` |
| 14 | `force_cancel_sale` | `round_id` (Root) |
| 15 | `force_end_sale` | `round_id` (Root) |
| 16 | `force_refund` | `round_id`, `account` (Root) |
| 17 | `force_withdraw_funds` | `round_id` (Root) |
| 18 | `update_sale_round` | `round_id`, update params... |
| 19 | `increase_subscription` | `round_id`, `additional_amount` |
| 20 | `remove_from_whitelist` | `round_id`, `accounts` |
| 21 | `remove_payment_option` | `round_id`, `asset_id` |
| 22 | `extend_sale` | `round_id`, `new_end_block` |
| 23 | `pause_sale` | `round_id` |
| 24 | `resume_sale` | `round_id` |
| 25 | `cleanup_round` | `round_id` |
| 26 | `force_batch_refund` | `round_id`, `accounts: Vec<AccountId>` (Root) |

### Storage Items

`NextRoundId`, `SaleRounds`, `EntityRounds`, `Subscriptions`, `RoundParticipants`, `RaisedFunds`, `RoundPaymentOptions`, `ActiveRounds`, `RoundWhitelist`, `WhitelistCount`

---

## 14. Commission Sub-System

### 14a. pallet-commission-core

#### Dispatchable Functions

| # | Function | Parameters |
|---|----------|-----------|
| 0 | `set_commission_modes` | `entity_id`, `modes: CommissionModes` |
| 1 | `set_commission_rate` | `entity_id`, `rate: u16` |
| 2 | `enable_commission` | `entity_id`, `enabled: bool` |
| 3 | `withdraw_commission` | `entity_id` |
| 4 | `set_withdrawal_config` | `entity_id`, config params... |
| 5 | `init_commission_plan` | (placeholder) |
| 6 | `use_shopping_balance` | (placeholder) |
| 7 | `set_token_withdrawal_config` | `entity_id`, config params... |
| 8 | `set_global_min_token_repurchase_rate` | `entity_id`, `rate: u16` |
| 9 | `withdraw_token_commission` | `entity_id` |
| 10 | `withdraw_entity_funds` | `entity_id`, `amount` |
| 11 | `withdraw_entity_token_funds` | `entity_id`, `amount` |
| 12 | `set_creator_reward_rate` | `entity_id`, `rate: u16` |
| 13 | `set_token_platform_fee_rate` | `new_rate: u16` (Root) |
| 14 | `set_global_min_repurchase_rate` | `entity_id`, `rate: u16` |
| 15 | `set_withdrawal_cooldown` | `entity_id`, `cooldown_blocks` |
| 16 | `force_disable_entity_commission` | `entity_id` (Root) |
| 17 | `set_global_max_commission_rate` | `entity_id`, `rate: u16` |
| 18 | `clear_commission_config` | `entity_id` |
| 19 | `clear_withdrawal_config` | `entity_id` |
| 20 | `clear_token_withdrawal_config` | `entity_id` |
| 21 | `set_global_max_token_commission_rate` | `entity_id`, `rate` |
| 22 | `force_global_pause` | `paused: bool` (Root) |
| 23 | `pause_withdrawals` | `entity_id` |
| 24 | `archive_order_records` | `entity_id`, `order_ids` |

### 14b. pallet-commission-referral

#### Dispatchable Functions

`set_direct_reward_config`, `set_fixed_amount_config`, `set_first_order_config`, `set_repeat_purchase_config`, `clear_referral_config`, `force_set_*` variants, `set_referrer_guard_config`, `set_commission_cap_config`, `set_referral_validity_config`, `set_config_effective_after`

### 14c. pallet-commission-multi-level

#### Dispatchable Functions

`set_multi_level_config`, `clear_multi_level_config`, `force_set_multi_level_config`, `force_clear_multi_level_config`, `update_multi_level_params`, `add_tier`, `remove_tier`, `pause_multi_level`, `resume_multi_level`, `schedule_config_change`, `apply_pending_config`, `cancel_pending_config`

### 14d. pallet-commission-level-diff

#### Dispatchable Functions

`set_level_diff_config`, `clear_level_diff_config`, `update_level_diff_config`, `force_set_level_diff_config`, `force_clear_level_diff_config`

### 14e. pallet-commission-single-line

#### Dispatchable Functions

`set_single_line_config`, `clear_single_line_config`, `update_single_line_params`, `set_level_based_levels`, `remove_level_based_levels`, `force_set_single_line_config`, `force_clear_single_line_config`, `force_reset_single_line`, `pause_single_line`, `resume_single_line`

### 14f. pallet-commission-team

#### Dispatchable Functions

`set_team_performance_config`, `clear_team_performance_config`, `update_team_performance_params`, `force_set_team_performance_config`, `force_clear_team_performance_config`, `pause_team_performance`, `resume_team_performance`, `add_tier`, `update_tier`, `remove_tier`

### 14g. pallet-commission-pool-reward

#### Dispatchable Functions

`set_pool_reward_config`, `claim_pool_reward`, `force_new_round`, `set_token_pool_enabled`, `force_set_pool_reward_config`, `force_set_token_pool_enabled`, `clear_pool_reward_config`, `force_clear_pool_reward_config`, `force_start_new_round`, `pause_pool_reward`, `resume_pool_reward`, `set_global_pool_reward_paused`

---

## Key Cross-Module Traits (for frontend bridge understanding)

| Trait | Implemented By | Used By |
|-------|---------------|---------|
| `EntityProvider` | registry | shop, product, order, review, token, market, member, governance, disclosure, kyc, tokensale, commission |
| `ShopProvider` | shop | product, order, review, commission |
| `ProductProvider` | product | order |
| `OrderProvider` | order | review |
| `EntityTokenProvider` | token | order, market |
| `MemberProvider` | member | commission, order, governance |
| `KycProvider` | kyc | token, market |
| `DisclosureProvider` | disclosure | token, market |
| `GovernanceProvider` | governance | registry |
| `PricingProvider` | nex-market | registry, shop, product, order |
| `EntityTokenPriceProvider` | market | order |
| `CommissionFundGuard` | commission-core | shop |
| `OrderCommissionHandler` | commission-core | order |
| `TokenOrderCommissionHandler` | commission-core | order |
| `ShoppingBalanceProvider` | commission-core | order |
| `OrderMemberHandler` | member | order |
| `OnEntityStatusChange` | (tuple) | registry cascade |
| `OnOrderStatusChange` | (tuple) | order cascade |
