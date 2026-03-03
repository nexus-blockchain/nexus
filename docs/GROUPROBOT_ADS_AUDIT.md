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

---
---

# Round 2: pallet-ads-grouprobot (适配层) 深度审计

**日期:** 2026-03-04
**范围:** `pallets/ads/grouprobot/` — lib.rs (585→655行), mock.rs (180行), tests.rs (440→641行), Cargo.toml (55→51行)

> Round 1 审计的是原 monolithic `pallet-grouprobot-ads`。
> Round 2 审计的是重构后的 `pallet-ads-grouprobot` 适配层 (仅 GroupRobot 专属逻辑，Campaign CRUD 已移至 ads-core)。

## 发现汇总

| 级别 | ID | 描述 | 状态 |
|------|-----|------|------|
| High | H1 | `RevenueDistributor::distribute` 仅返回社区份额，不执行实际转账 — 节点奖励从未分配 | ✅ 已修复 |
| High | H2 | `set_tee_ad_pct` / `set_community_ad_pct` "0=默认值" 语义导致校验与存储不一致 | ✅ 已修复 |
| High | H3 | `check_audience_surge` 暂停后无恢复机制 — 社区永久无法投放广告 | ✅ 已修复 |
| Medium | M1 | `unstake_for_ads` 全部取消质押后不清理 `CommunityStakers` / `CommunityAdmin` | ✅ 已修复 |
| Medium | M2 | `report_node_audience` 同一节点可重复提交消耗全部槽位 | ✅ 已修复 |
| Medium | M3 | 死事件/错误: `CommunitySlashed` / `NodeDeviationRejected` 从未发射 | ✅ 已修复 |
| Medium | M4 | `distribute` 忽略 `placement_id`，不检查社区暂停状态 | ✅ 已修复 |
| Low | L1 | `pallet-ads-core` 列为依赖但从未引用 (死依赖) | ✅ 已修复 |
| Low | L2 | Config `AdSlashPercentage` 声明但从未使用 | ✅ 已修复 (M3 slash_community 使用) |
| Low | L3 | README 权限描述错误 + 列出不存在的错误码 | ✅ 已修复 |

**修复 10 项, 记录 0 项**

---

## 修复详情

### H1: `RevenueDistributor::distribute` 不执行实际转账 (High)

**问题:** `distribute()` 只计算社区份额并返回，**从不执行任何转账**。节点奖励未通过 `RewardAccruer::accrue_node_reward` 写入奖励池，`NodeAdRewardAccrued` 事件从未发射。`RewardPoolAccount` 和 `TreasuryAccount` Config 项未被使用。

**修复:** `distribute` 中实际执行三方分成：
- 社区份额 → 通过返回值记入 `PlacementClaimable` (ads-core 管理)
- 节点份额 → 从国库转入 `RewardPoolAccount` + `accrue_node_reward` + 发射 `NodeAdRewardAccrued`
- 国库 → 保留剩余 (100% - community% - tee%)

**文件:** `pallets/ads/grouprobot/src/lib.rs` L605-653

### H2: `set_tee_ad_pct` / `set_community_ad_pct` 校验不一致 (High)

**问题:** extrinsic 中 `let effective_tee = if tee_pct == 0 { 15 } else { tee_pct }` 将 0 扩展为默认值做校验，但存储写入 0，事件报告 0。读取时 `effective_tee_pct()` 再次扩展 0→15。设置者以为设了 0% 但实际是 15%。

**修复:** 移除 extrinsic 中的 "0=默认" 扩展，直接用传入值做校验。`effective_*_pct()` 的 0→default fallback 仅用于 genesis 未初始化场景。

**文件:** `pallets/ads/grouprobot/src/lib.rs` L331-367

### H3: 社区暂停后无恢复路径 (High)

**问题:** `check_audience_surge` 设置 `AudienceSurgePaused = 1` 后无代码路径清零。`AudienceSurgeResumed` 事件定义但从未发射。一旦暂停，`verify_and_cap_audience` 永远返回 `CommunityAdsPaused`。

**修复:** 新增 `resume_audience_surge` extrinsic (call_index 7, Root)，验证暂停状态后清零并发射 `AudienceSurgeResumed`。新增 `CommunityNotPaused` 错误。

**文件:** `pallets/ads/grouprobot/src/lib.rs` L456-471

### M1: `unstake_for_ads` 不清理零余额条目 (Medium)

**问题:** 用户全部取消质押后:
- `CommunityStakers` 保留 0 余额条目 (存储浪费)
- `CommunityAdmin` 不清理 (零质押社区仍有管理员)

**修复:** 质押降为 0 时 `CommunityStakers::remove`；总质押降为 0 时 `CommunityAdmin::remove`。

**文件:** `pallets/ads/grouprobot/src/lib.rs` L303-322

### M2: `report_node_audience` 同节点重复提交 (Medium)

**问题:** 同一 node prefix 可多次 `try_push` 消耗全部 10 个报告槽位，挤占其他节点报告空间。

**修复:** 查找已有相同 prefix 的条目 → 更新 audience_size 而非追加。

**文件:** `pallets/ads/grouprobot/src/lib.rs` L412-421

### L1: 死依赖 `pallet-ads-core` (Low)

**问题:** Cargo.toml 依赖 `pallet-ads-core` 但 lib.rs 从未引用。同时 `try-runtime` 和 `runtime-benchmarks` features 也传播了此死依赖。

**修复:** 移除 Cargo.toml 中的 `pallet-ads-core` 依赖及其 feature 传播。

**文件:** `pallets/ads/grouprobot/Cargo.toml`

### M3: 死事件/错误 — `CommunitySlashed` / `NodeDeviationRejected` 从未发射 (Medium)

**问题:**
- `CommunitySlashed`: 事件定义但无代码路径发射
- `NodeDeviationRejected`: 事件定义但 `validate_node_reports` 仅返回 `Err((min, max))`，无 extrinsic 调用它
- `NodeDeviationTooHigh`: 错误定义但仅在 helper 中隐含

**修复:** 新增两个 extrinsic:
- `cross_validate_nodes` (call_index 8, Root): 调用 `validate_node_reports`，偏差过大时发射 `NodeDeviationRejected` 事件。始终返回 `Ok(())` 因为 Substrate 事务层在 `Err` 时回滚所有存储(含事件)。验证后清理报告数据。
- `slash_community` (call_index 9, Root): 使用 `T::AdSlashPercentage::get()` 计算 slash 金额，按比例从各质押者 unreserve 并转入国库。发射 `CommunitySlashed` 事件。自动清理零余额质押者和零总质押管理员。

**文件:** `pallets/ads/grouprobot/src/lib.rs` L473-584

### M4: `distribute` 不检查暂停状态 (Medium)

**问题:** 被暂停的社区如有未结算收据仍可通过 settle 获得收入分配。

**修复:** 在 `distribute` 入口添加 `AudienceSurgePaused` 检查，暂停社区返回 `CommunityAdsPaused` 错误。

**文件:** `pallets/ads/grouprobot/src/lib.rs` L729-733

### L2: `AdSlashPercentage` 未使用 (Low)

**问题:** Config 声明 `AdSlashPercentage` 但代码从未调用 `T::AdSlashPercentage::get()`。

**修复:** 由 M3 的 `slash_community` extrinsic 使用。

### L3: README 不一致 (Low)

**问题:**
- `check_audience_surge` 权限描述为 "Signed (any)" 但代码要求 Root
- 列出 `CommunityBanned` 错误但代码中不存在
- 缺少 `resume_audience_surge`/`cross_validate_nodes`/`slash_community` extrinsic
- 缺少 `CommunityNotPaused` 错误
- 测试数量过时
- 依赖树列出已移除的 `pallet-ads-core`

**修复:** 全面同步 README：权限、extrinsics 表、错误表、测试数量、依赖树。

**文件:** `pallets/ads/grouprobot/README.md`

---

## 修改文件

| 文件 | 变更 |
|------|------|
| `pallets/ads/grouprobot/src/lib.rs` | H1, H2, H3, M1, M2, M3, M4 修复; +3 extrinsic, +1 错误 (585→770行) |
| `pallets/ads/grouprobot/src/tests.rs` | +19 回归测试 (45 user + 2 auto = 47 total, was 28) |
| `pallets/ads/grouprobot/Cargo.toml` | L1: 移除死依赖 pallet-ads-core (55→51行) |
| `pallets/ads/grouprobot/README.md` | L3: 全面同步权限、extrinsic、错误、测试数量、依赖树 |

## 新增

- **Extrinsic (3):** `resume_audience_surge` (call_index 7, Root), `cross_validate_nodes` (call_index 8, Root), `slash_community` (call_index 9, Root)
- **错误 (1):** `CommunityNotPaused`
- **测试 (19):** h1_distribute_transfers_node_share_to_reward_pool, h1_distribute_emits_node_ad_reward_event, h2_set_tee_pct_zero_validates_with_zero, h2_set_community_pct_zero_validates_with_zero, h3_resume_audience_surge_works, h3_resume_audience_surge_fails_not_paused, h3_resume_audience_surge_requires_root, m1_unstake_all_removes_staker_entry, m1_unstake_partial_keeps_admin, m2_report_node_audience_deduplicates_same_node, m3_cross_validate_nodes_emits_event_on_deviation, m3_cross_validate_nodes_passes_and_clears_reports, m3_cross_validate_nodes_requires_root, m3_slash_community_works, m3_slash_community_requires_root, m3_slash_community_fails_no_stake, m4_distribute_rejects_paused_community, m4_distribute_works_when_not_paused

## 验证

- `cargo test -p pallet-ads-grouprobot`: **47/47 ✅** (was 28)
- `cargo check -p pallet-ads-grouprobot`: ✅
- `cargo check -p pallet-ads-core`: ✅
- `cargo check -p nexus-runtime`: 预存在的 `pallet-grouprobot-ceremony` Box import 错误 (非本次变更)
