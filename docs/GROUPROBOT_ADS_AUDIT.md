# pallet-grouprobot-ads 深度审计报告

**日期:** 2026-03-03
**范围:** lib.rs (1697→1800行), mock.rs (210行), tests.rs (1268→1494行), Cargo.toml, README.md, off-chain integration (ad_delivery.rs, transactions.rs, queries.rs, types.rs), runtime config

## 发现汇总

| 级别 | ID | 描述 | 状态 |
|------|-----|------|------|
| Critical | C1 | `slash_community` 忽略 unreserve 返回值 + 静默丢弃转账错误 | ✅ 已修复 |
| High | H2 | `report_node_audience` 无调用者验证, 任何人可提交伪造报告 | ✅ 已修复 |
| High | H3 | `NextCampaignId` 溢出, u64::MAX 时覆盖已有 Campaign | ✅ 已修复 |
| High | H6 | 链下 `submit_delivery_receipt` node_id 类型不匹配 (u128 vs [u8;4]) | ✅ 已修复 |
| High | H7 | 链下 `query_campaign` 枚举解码错误, is_active/is_approved 始终 false | ✅ 已修复 |
| Medium | M1 | `create_campaign` 不验证 `expires_at > now` | ✅ 已修复 |
| Medium | M2 | `set_community_admin` 不发出事件 | ✅ 已修复 |
| Medium | M5 | 白名单无移除接口 (advertiser/community unprefer) | ✅ 已修复 |
| Medium | M3 | `EraAdRevenue` 从不清零, 与 CommunityTotalRevenue 重复 | 📝 记录 |
| Medium | M4 | 链下广告投放硬编码 delivery_type=0 (ScheduledPost) | 📝 记录 |
| Medium | M6 | 所有 extrinsic 硬编码 weight, 无 benchmark | 📝 记录 |
| Low | L1 | `flag_campaign` 复用 `AlreadyReviewed` 错误语义不当 | 📝 记录 |
| Low | L2 | Runtime 尚未接入 ads pallet (`type AdDelivery = ()`) | 📝 记录 |
| Low | L3 | `flag_community` 无防重复/限速, CommunityFlagCount 无链上后果 | 📝 记录 |

**修复 8 项, 记录 6 项**

---

## 修复详情

### C1: `slash_community` 资金核算错误 (Critical)

**问题:** `Currency::unreserve` 返回未能释放的余量,但代码忽略该返回值,继续用原始 `slash_amount` 做转账和存储更新。同时两个 `Currency::transfer` 调用用 `let _ =` 静默忽略错误。当 admin 实际 reserved 余额 < slash_amount 时:
- unreserve 只部分释放
- transfer 静默失败,reporter/treasury 实际未收到资金
- CommunityAdStake/CommunityStakers 被错误减少

**修复:** 使用 `unreserve` 返回值计算 `actual_slashed`,所有后续操作(转账、存储更新、事件)均使用 `actual_slashed`。转账错误改为 `map_err` 传播。

**文件:** `pallets/grouprobot/ads/src/lib.rs` L1329-1388

### H2: `report_node_audience` 无调用者验证 (High)

**问题:** 任何签名账户都可调用 `report_node_audience` 提交伪造的 audience 统计,毒化 L5 多节点交叉验证数据。对比 `check_audience_surge` 有 `is_tee_node_by_operator` 检查。

**修复:** 添加 `ensure!(T::NodeConsensus::is_tee_node_by_operator(&who), Error::<T>::NodeNotTee)`

**文件:** `pallets/grouprobot/ads/src/lib.rs` L1427-1429

### H3: `NextCampaignId` 溢出 (High)

**问题:** `saturating_add(1)` 在 u64::MAX 时不递增,下次 `create_campaign` 覆盖已有 Campaign 数据。

**修复:** 添加 `ensure!(id < u64::MAX, Error::<T>::CampaignIdOverflow)`,新增错误 `CampaignIdOverflow`。

**文件:** `pallets/grouprobot/ads/src/lib.rs` L631-633

### H6: 链下 node_id 类型不匹配 (High)

**问题:** `submit_delivery_receipt` 链下用 `Value::u128(node_id as u128)` 发送,但链上 `NodeId` 类型为 `[u8; 4]`(固定长度字节数组)。SCALE 编码不兼容,交易解码失败。

**修复:** 改为 `Value::from_bytes(node_id.to_le_bytes())`

**文件:** `grouprobot/src/chain/transactions.rs` L258-259

### H7: 链下枚举解码错误 (High)

**问题:** `query_campaign` 用 `as_u128()` 解码 `CampaignStatus` 和 `AdReviewStatus`,但 SCALE 枚举在 subxt 动态 API 中解码为命名 variant,`as_u128()` 始终返回 `None`,导致 `is_active`/`is_approved` 永远为 `false`。广告投放循环因此跳过所有 Campaign。

**修复:** 改用 `ValueDef::Variant` 匹配 variant 名称 ("Active"/"Approved")

**文件:** `grouprobot/src/chain/queries.rs` L601-614

### M1: `create_campaign` 不验证过期时间 (Medium)

**问题:** `expires_at` 可为过去或当前区块,创建即过期但资金已锁定。虽可通过 `cancel_campaign` 退款,但浪费链上资源。

**修复:** 添加 `ensure!(expires_at > now, Error::<T>::InvalidExpiry)`,新增错误 `InvalidExpiry`。检查在 `reserve` 之前执行,避免无效交易消耗 reserve gas。

**文件:** `pallets/grouprobot/ads/src/lib.rs` L627-629

### M2: `set_community_admin` 不发出事件 (Medium)

**问题:** Root 变更社区管理员时无事件发出,链下系统无法追踪管理员变更。

**修复:** 新增 `CommunityAdminUpdated { community_id_hash, new_admin }` 事件

**文件:** `pallets/grouprobot/ads/src/lib.rs` L1559-1564

### M5: 白名单无移除接口 (Medium)

**问题:** 广告主/社区可将对方加入白名单(prefer),但没有对应的移除接口。一旦加入白名单无法撤回。

**修复:** 新增两个 extrinsic:
- `advertiser_unprefer_community` (call_index 26)
- `community_unprefer_advertiser` (call_index 27)

新增两个事件:
- `AdvertiserUnpreferredCommunity`
- `CommunityUnpreferredAdvertiser`

**文件:** `pallets/grouprobot/ads/src/lib.rs` L1610-1658

---

## 记录但未修复

### M3: `EraAdRevenue` 语义混淆
`EraAdRevenue` 注释为"每 Era 社区广告收入",但实际只做 `saturating_add` 从不清零,与 `CommunityTotalRevenue` 功能完全重复。建议在 `settle_era_ads` 结算前先 `remove` 再写入,或直接移除此存储项。

### M4: 链下硬编码 ScheduledPost
`ad_delivery.rs` L217-225 所有投放均使用 `delivery_type=0` (ScheduledPost),未利用 ReplyFooter/WelcomeEmbed 的 CPM 折扣。建议从链上排期数据读取 campaign 的 delivery_types bitmask 并选择合适类型。

### M6: 硬编码 Weight
全部 27 个 extrinsic 使用硬编码 `Weight::from_parts(...)`,无 benchmark 框架。需引入完整 benchmark 基础设施。

### L1: `flag_campaign` 错误语义
当 campaign 已被 flag 或 reject 时返回 `AlreadyReviewed`,但实际并非"已审核",建议新增 `AlreadyFlagged` 错误。

### L2: Runtime 未接入
`runtime/src/configs/mod.rs` L1692-1693: `type AdDelivery = ();`。ads pallet 尚未接入 runtime,链下所有针对 `GroupRobotAds` 的交易/查询将在 runtime 升级前失败。

### L3: `flag_community` 无限制
`flag_community` 仅递增 `CommunityFlagCount` 计数器,无每账户去重、无冷却期、无链上后果。该计数器可能仅用于治理参考。

---

## 修改文件

| 文件 | 变更 |
|------|------|
| `pallets/grouprobot/ads/src/lib.rs` | C1, H2, H3, M1, M2, M5 修复; +2 错误, +3 事件, +2 extrinsic (1697→1800行) |
| `pallets/grouprobot/ads/src/tests.rs` | +10 回归测试, 更新 11 个既有测试 (68→78, 1268→1494行) |
| `grouprobot/src/chain/transactions.rs` | H6 修复 (node_id 类型) |
| `grouprobot/src/chain/queries.rs` | H7 修复 (枚举解码) |

## 新增

- **错误 (2):** `CampaignIdOverflow`, `InvalidExpiry`
- **事件 (3):** `CommunityAdminUpdated`, `AdvertiserUnpreferredCommunity`, `CommunityUnpreferredAdvertiser`
- **Extrinsics (2):** `advertiser_unprefer_community` (26), `community_unprefer_advertiser` (27)
- **测试 (10):** c1_slash_uses_actual_unreserved_amount, c1_slash_propagates_transfer_errors, h2_report_node_audience_rejects_non_tee, h3_create_campaign_rejects_at_max_id, m1_create_campaign_rejects_past_expiry, m2_set_community_admin_emits_event, m5_advertiser_unprefer_community_works, m5_advertiser_unprefer_fails_not_whitelisted, m5_community_unprefer_advertiser_works, m5_community_unprefer_rejects_non_admin

## 验证

- `cargo test -p pallet-grouprobot-ads`: **78/78 ✅**
- `cargo check -p pallet-grouprobot-ads`: ✅
