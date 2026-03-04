# Nexus 测试计划 — Part 2

> 续 [NEXUS_TEST_PLAN.md](./NEXUS_TEST_PLAN.md)
> 更新日期: 2026-03-03

---

## 8. Entity Member — 会员管理

> Pallet: `pallet-entity-member` | Extrinsics: 22 (call_index 0-1, 4-22; 2-3 reserved)
> 审计修复: P4(update_spent 过期等级自动修正+MemberLevelExpired 事件)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| MB-001 | register_member(0)：正常注册会员 | R7 | 正向 | P0 |
| MB-002 | 带推荐人注册（referrer 参数） | R7 | 正向 | P0 |
| MB-003 | bind_referrer(1)：后绑定推荐人 | R7 | 正向 | P1 |
| MB-004 | 已有推荐人不可重复绑定 | R7 | 负向 | P1 |
| MB-005 | 不能推荐自己 | R7 | 负向 | P1 |
| MB-006 | 下单时自动注册（ensure_member） | 系统 | 流程 | P0 |
| MB-007 | set_member_policy(17)：设置会员策略（0=开放, 1=需购买, 2=需推荐人, 4=需审批，位掩码可组合） | R2 | 正向 | P1 |
| MB-008 | 需审批策略：注册→Pending→approve_member(18)→Active | R2/R3 | 流程 | P1 |
| MB-009 | reject_member(19)：拒绝审批 | R2/R3 | 正向 | P2 |
| MB-010 | cancel_pending_member(21)：取消待审批会员 | R7 | 正向 | P2 |
| MB-011 | cleanup_expired_pending(22)：清理过期待审批（任何人可调用） | R20 | 正向 | P2 |
| MB-012 | set_member_stats_policy(20)：设置统计策略（0=排除复购, 1=直推含复购, 2=间推含复购） | R2 | 正向 | P2 |
| MB-013 | init_level_system(4)：初始化等级系统（use_custom + upgrade_mode） | R2 | 正向 | P0 |
| MB-014 | add_custom_level(5)：添加自定义等级（阈值/折扣/佣金加成） | R2 | 正向 | P0 |
| MB-015 | 超过 MaxCustomLevels(10) 被拒绝 | R2 | 负向 | P1 |
| MB-016 | update_custom_level(6) / remove_custom_level(7)：更新/删除等级 | R2 | 正向 | P1 |
| MB-017 | manual_upgrade_member(8)：手动升级会员（指定 custom_level_id） | R2 | 正向 | P1 |
| MB-018 | set_upgrade_mode(10) / set_conflict_strategy(16)：设置升级模式/冲突策略 | R2 | 正向 | P2 |
| MB-019 | set_use_custom_levels(9)：启用/禁用自定义等级 | R2 | 正向 | P2 |
| MB-020 | init_upgrade_rule_system(11)：初始化升级规则系统 | R2 | 正向 | P1 |
| MB-021 | add_upgrade_rule(12)：添加规则（PurchaseProduct/ReferralCount/TeamSize/TotalSpent/OrderCount） | R2 | 正向 | P1 |
| MB-022 | 新触发器：ReferralLevelCount / TotalSpentUsdt / SingleOrderUsdt | R2 | 正向 | P1 |
| MB-023 | update_upgrade_rule(13) / remove_upgrade_rule(14)：更新/删除规则 | R2 | 正向 | P1 |
| MB-024 | set_upgrade_rule_system_enabled(15)：启用/禁用规则系统 | R2 | 正向 | P2 |
| MB-025 | 规则触发自动升级（订单路径: update_spent → evaluate_rules） | 系统 | 流程 | P0 |
| MB-026 | 规则触发自动升级（推荐路径: bind_referrer → evaluate_rules） | 系统 | 流程 | P0 |
| MB-027 | 升级后级联检查推荐人 ReferralLevelCount 规则 | 系统 | 流程 | P1 |
| MB-028 | 等级过期 → get_effective_level 返回回退等级 | 系统 | 功能 | P1 |
| MB-029 | update_spent 检测过期等级并修正+发出 MemberLevelExpired 事件（审计 P4 回归） | 系统 | 功能 | P0 |
| MB-030 | USDT 消费累加到 total_spent，自定义等级阈值基于 USDT 精度 | 系统 | 功能 | P0 |
| MB-031 | zero_usdt 金额不改变自定义等级 | 系统 | 负向 | P1 |
| MB-032 | USDT 跨订单累计正确性 | 系统 | 功能 | P1 |

## 9. Commission 模块群 — 返佣系统

### 9.1 Commission Core — 核心引擎

> Pallet: `pallet-entity-commission-core` | Extrinsics: 14 (call_index 0-6, 8, 10-13)
> 审计修复: H2(cancel_commission 先转账后清记录)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-001 | init_commission_plan(6)：一键初始化佣金方案（CommissionPlan 枚举） | R2 | 正向 | P0 |
| CM-002 | set_commission_modes(0)：设置返佣模式（Referral/LevelDiff/SingleLine/Team/PoolReward/MultiLevel） | R2 | 正向 | P0 |
| CM-003 | set_commission_rate(1) / enable_commission(2)：设置返佣上限/启用禁用 | R2 | 正向 | P1 |
| CM-004 | withdraw_commission(3)：提现 NEX 佣金（四种 WithdrawalMode） | R7 | 正向 | P0 |
| CM-005 | withdraw_token_commission(8)：提现 Token 佣金 | R7 | 正向 | P1 |
| CM-006 | 自愿复购到购物余额（WithdrawalMode::ToShoppingBalance） | R7 | 正向 | P1 |
| CM-007 | 复购目标为非会员 → 自动注册 | R7 | 流程 | P2 |
| CM-008 | 复购目标为已有会员 → 推荐人必须是出资人 | R7 | 负向 | P2 |
| CM-009 | use_shopping_balance(5) 已禁用（购物余额仅可购物） | R7 | 安全 | P0 |
| CM-010 | set_withdrawal_config(4) / set_token_withdrawal_config(10)：设置 NEX/Token 提现配置 | R2 | 正向 | P1 |
| CM-011 | set_global_min_token_repurchase_rate(11)：全局最低 Token 复购比例 | R1 | 正向 | P2 |
| CM-012 | withdraw_entity_funds(12)：提取 Entity NEX 资金 | R2 | 正向 | P1 |
| CM-013 | 提取后余额 ≥ PendingTotal + ShoppingTotal + UnallocatedPool（安全检查） | R2 | 安全 | P0 |
| CM-014 | withdraw_entity_token_funds(13)：提取 Entity Token 资金 | R2 | 正向 | P2 |

### 9.2 佣金分配流程（process_commission / process_token_commission）

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-015 | 池 A（平台费池）→ Entity 推荐人获得 ReferrerShareBps 分成 | 系统 | 流程 | P1 |
| CM-016 | 池 B（卖家池）→ Referral 推荐佣金（多级） | 系统 | 流程 | P0 |
| CM-017 | 池 B → LevelDiff 级差佣金 | 系统 | 流程 | P1 |
| CM-018 | 池 B → SingleLine 单线佣金（order_amount × rate / 10000） | 系统 | 流程 | P1 |
| CM-019 | 池 B → Team 团队佣金 | 系统 | 流程 | P1 |
| CM-020 | 池 B → PoolReward 沉淀池分配 | 系统 | 流程 | P1 |
| CM-021 | 池 B → MultiLevel 多级佣金 | 系统 | 流程 | P1 |
| CM-022 | cancel_commission：订单取消 → 退佣（审计 H2: 先转账后清记录，回归测试） | 系统 | 流程 | P0 |
| CM-023 | do_cancel_token_commission：Token 佣金独立取消 | 系统 | 流程 | P1 |
| CM-024 | Token 订单双路佣金分配（process_token_commission） | 系统 | 流程 | P1 |
| CM-025 | KYC 参与检查（KycParticipationGuard） | R7 | 安全 | P1 |

### 9.3 Commission 插件

> Referral: `pallet-entity-commission-referral` | 4 extrinsics (call_index 0, 2-4)
> MultiLevel: `pallet-entity-commission-multi-level` | 1 extrinsic (call_index 0)
> LevelDiff: `pallet-entity-commission-level-diff` | 1 extrinsic (call_index 1)
> SingleLine: `pallet-entity-commission-single-line` | 3 extrinsics (call_index 0-2)
> Team: `pallet-entity-commission-team` | 1 extrinsic (call_index 0)
> PoolReward: `pallet-entity-commission-pool-reward` | 4 extrinsics (call_index 0-3)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-026 | Referral: set_direct_reward_config(0) / set_fixed_amount_config(2) / set_first_order_config(3) / set_repeat_purchase_config(4) | R2 | 正向 | P1 |
| CM-027 | Referral: 多级推荐佣金费率验证 | 系统 | 功能 | P1 |
| CM-028 | MultiLevel: set_multi_level_config(0)（多级佣金配置） | R2 | 正向 | P1 |
| CM-029 | LevelDiff: set_level_diff_config(1)（级差比例 bps） | R2 | 正向 | P1 |
| CM-030 | LevelDiff: 级差计算（上级 - 下级差值） | 系统 | 功能 | P1 |
| CM-031 | SingleLine: set_single_line_config(0)（upline/downline 费率 + 基础/最大层数） | R2 | 正向 | P1 |
| CM-032 | SingleLine: set_level_based_levels(1) / remove_level_based_levels(2)（等级自定义层数） | R2 | 正向 | P2 |
| CM-033 | SingleLine: calc_extra_levels 溢出保护（.min(255)） | 系统 | 安全 | P2 |
| CM-034 | Team: set_team_performance_config(0)（团队业绩返佣配置） | R2 | 正向 | P1 |
| CM-035 | PoolReward: set_pool_reward_config(0)（NEX + Token 双池配置） | R1 | 正向 | P1 |
| CM-036 | PoolReward: claim_pool_reward(1)（用户领取沉淀池奖励，冷却期 ~7 天） | R7 | 正向 | P1 |
| CM-037 | PoolReward: force_new_round(2)（Root 强制开启新轮次） | R1 | 正向 | P2 |
| CM-038 | PoolReward: set_token_pool_enabled(3)（启用/禁用 Token 池） | R1 | 正向 | P2 |

## 10. Entity Disclosure — 财务披露 & 公告

> Pallet: `pallet-entity-disclosure` | Extrinsics: 15 (call_index 0-14)
> 审计新增: cleanup_disclosure_history(13), cleanup_announcement_history(14)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| DC-001 | configure_disclosure(0)：配置披露设置（级别/间隔） | R2 | 正向 | P1 |
| DC-002 | publish_disclosure(1)：发布披露（年度/季度/月度/事件驱动） | R2 | 正向 | P1 |
| DC-003 | withdraw_disclosure(2) / correct_disclosure(3)：撤回/更正披露 | R2 | 正向 | P2 |
| DC-004 | 披露间隔未到不可发布 | R2 | 负向 | P2 |
| DC-005 | add_insider(4) / remove_insider(5)：添加/移除内幕人员（≤50 人） | R2 | 正向 | P1 |
| DC-006 | start_blackout(6) / end_blackout(7)：手动开始/结束黑窗口期 | R2 | 正向 | P1 |
| DC-007 | 黑窗口期 ≤ MaxBlackoutDuration（~7 天） | R2 | 边界 | P2 |
| DC-008 | publish_announcement(8) / update_announcement(9) / withdraw_announcement(10)：公告 CRUD | R2 | 正向 | P1 |
| DC-009 | pin_announcement(11)：置顶/取消置顶公告 | R2 | 正向 | P2 |
| DC-010 | expire_announcement(12)：过期公告 | R2 | 正向 | P2 |
| DC-011 | cleanup_disclosure_history(13)：清理已撤回披露记录（任何人可调用） | R20 | 正向 | P2 |
| DC-012 | cleanup_announcement_history(14)：清理已撤回公告记录（任何人可调用） | R20 | 正向 | P2 |

## 11. Entity KYC — 认证管理

> Pallet: `pallet-entity-kyc` | Extrinsics: 9 (call_index 0-8)
> 审计修复: M1-R4(高风险国家去重), M2-R4(reject 计入 verifications_count), M3-R4(expire_kyc 新增)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| KY-001 | submit_kyc(0)：提交 KYC（Basic/Standard/Enhanced/Institutional） | R20 | 正向 | P0 |
| KY-002 | 空 data_cid 被拒绝（EmptyDataCid） | R20 | 负向 | P1 |
| KY-003 | 非法国家代码被拒绝（需大写 ASCII） | R20 | 负向 | P1 |
| KY-004 | approve_kyc(1)：批准 KYC（设有效期 + risk_score） | R10 | 正向 | P0 |
| KY-005 | Provider 等级不够不能批准高级别 | R10 | 负向 | P1 |
| KY-006 | reject_kyc(2)：拒绝 KYC + 递增 verifications_count（审计 M2-R4 回归） | R10 | 正向 | P1 |
| KY-007 | revoke_kyc(3)：撤销 KYC | R1 | 正向 | P1 |
| KY-008 | register_provider(4) / remove_provider(5)：注册/移除 Provider | R1 | 正向 | P1 |
| KY-009 | set_entity_requirement(6)：设置实体 KYC 要求（max_risk_score ≤ 100） | R2 | 正向 | P1 |
| KY-010 | max_risk_score > 100 被拒绝 | R2 | 负向 | P1 |
| KY-011 | update_high_risk_countries(7)：更新高风险国家列表（自动 sort+dedup，审计 M1-R4） | R1 | 正向 | P2 |
| KY-012 | expire_kyc(8)：任何人过期已到期 KYC → KycExpired 事件（审计 M3-R4 回归） | R20 | 正向 | P1 |
| KY-013 | expire_kyc 对未过期/非 Approved 记录被拒绝 | R20 | 负向 | P1 |
| KY-014 | expire → revoke 完整流程可达（审计 M3-R4 回归） | R20→R1 | 流程 | P1 |
| KY-015 | 各级别有效期验证（Basic ~1 年, Standard ~6 月, Institutional ~2 年） | 系统 | 功能 | P2 |
| KY-016 | 宽限期内仍可参与实体活动 | 系统 | 功能 | P1 |
| KY-017 | 过期后 can_participate_in_entity = false | 系统 | 功能 | P1 |

## 12. Entity TokenSale — 代币发售

> Pallet: `pallet-entity-tokensale` | Extrinsics: 14 (call_index 0-13)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| TS-001 | create_sale_round(0, FixedPrice)：创建固定价格发售轮次 | R2 | 正向 | P0 |
| TS-002 | create_sale_round(0, DutchAuction)：创建荷兰拍卖轮次 | R2 | 正向 | P1 |
| TS-003 | add_payment_option(1)：添加支付选项 | R2 | 正向 | P0 |
| TS-004 | set_vesting_config(2) / configure_dutch_auction(3)：设置锁仓/拍卖配置 | R2 | 正向 | P1 |
| TS-005 | add_to_whitelist(4)：添加白名单 | R2 | 正向 | P2 |
| TS-006 | start_sale(5)：开始发售（锁定 Entity 代币） | R2 | 正向 | P0 |
| TS-007 | 无支付选项不能开始 | R2 | 负向 | P0 |
| TS-008 | DutchAuction 未配置不能开始 | R2 | 负向 | P1 |
| TS-009 | subscribe(6)：认购（NEX 锁入托管） | R5 | 正向 | P0 |
| TS-010 | KYC 等级不足被拒绝 | R5 | 负向 | P1 |
| TS-011 | 白名单模式下非白名单被拒绝 | R5 | 负向 | P1 |
| TS-012 | end_sale(7)：结束发售（now ≥ end_block 或已售罄） | R2 | 正向 | P0 |
| TS-013 | 提前结束（未到 end_block 且未售罄）被拒绝 | R2 | 负向 | P1 |
| TS-014 | claim_tokens(8)：领取代币（初始解锁部分） | R5 | 正向 | P0 |
| TS-015 | unlock_tokens(9)：解锁代币（锁仓期后） | R5 | 正向 | P1 |
| TS-016 | 锁仓期未到无法解锁 | R5 | 负向 | P1 |
| TS-017 | cancel_sale(10) → claim_refund(11)：取消发售 → 领取退款 | R2 → R5 | 流程 | P1 |
| TS-018 | reclaim_unclaimed_tokens(13)：回收过期未领退款（宽限期后） | R2 | 正向 | P2 |
| TS-019 | withdraw_funds(12)：提取募集资金（仅 Ended/Completed） | R2 | 正向 | P1 |

## 13. 交易市场模块

### 13A. NEX Market — P2P NEX/USDT 交易所

> Pallet: `pallet-trading-nex-market` | Extrinsics: 18 (call_index 0-17)
> C3 修复: tx_hash 防重放

#### 13A.1 卖单流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-001 | place_sell_order(0)：挂卖单（锁 NEX，提供 TRON 地址） | R9 | 正向 | P0 |
| NM-002 | reserve_sell_order(3)：买家吃卖单（锁保证金） | R9 | 正向 | P0 |
| NM-003 | confirm_payment(5)：买家声明已付款 | R9 | 正向 | P0 |
| NM-004 | submit_ocw_result(7)：OCW 验证 USDT 到账（tx_hash 防重放，审计 C3） | R19 | 正向 | P0 |
| NM-005 | claim_verification_reward(8)：领取奖励（释放 NEX + 退保证金 + 更新 TWAP） | R9 | 正向 | P0 |
| NM-006 | cancel_order(2)：取消卖单（退还锁定 NEX） | R9 | 正向 | P1 |

#### 13A.2 买单流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-007 | place_buy_order(1)：挂买单（预锁保证金） | R9 | 正向 | P0 |
| NM-008 | accept_buy_order(4)：卖家接买单（锁 NEX + 锁买家保证金） | R9 | 正向 | P0 |
| NM-009 | 买家确认 → OCW 验证 → 结算 | R9 → R19 | 流程 | P0 |
| NM-010 | 取消买单（退保证金） | R9 | 正向 | P1 |

#### 13A.3 价格保护

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-011 | 价格偏离 TWAP 超阈值被拒绝 | R9 | 安全 | P0 |
| NM-012 | 熔断触发 → 所有交易暂停 | 系统 | 安全 | P0 |
| NM-013 | lift_circuit_breaker(11)：手动解除熔断 | R1 | 正向 | P1 |
| NM-014 | configure_price_protection(9) / set_initial_price(10)：设置价格保护/基准价格 | R1 | 正向 | P1 |
| NM-015 | TWAP 三阶段：冷启动(InitialPrice) → 过渡期(max(24h,Initial)) → 成熟期(7d TWAP) | 系统 | 功能 | P1 |

#### 13A.4 多档判定

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-016 | Exact(99.5%~100.5%)：全额释放 + 退保证金 | R19 | 功能 | P0 |
| NM-017 | Overpaid(≥100.5%)：全额释放 + 退保证金 | R19 | 功能 | P1 |
| NM-018 | Underpaid(50%~99.5%) → 补付窗口(2h) | R19 | 功能 | P0 |
| NM-019 | SeverelyUnderpaid(<50%)：按比例释放 + 没收保证金 | R19 | 功能 | P1 |
| NM-020 | Invalid(=0)：不释放 + 没收保证金 | R19 | 功能 | P1 |

#### 13A.5 补付流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-021 | submit_underpaid_update(16)：OCW 补付窗口内更新金额 | R19 | 正向 | P0 |
| NM-022 | 累计达 99.5% 自动升级为 Exact | R19 | 功能 | P1 |
| NM-023 | finalize_underpaid(17)：补付终裁（梯度没收: ≥99.5%→0%, 95%~99.5%→20%, 80%~95%→50%, <80%→100%） | R19 | 功能 | P0 |

#### 13A.6 超时处理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-024 | process_timeout(6, AwaitingPayment)：退 NEX + 没收保证金 | R20 | 超时 | P0 |
| NM-025 | process_timeout(6, AwaitingVerification) + 宽限期(1h) | R20 | 超时 | P1 |
| NM-026 | process_timeout(6, UnderpaidPending) | R20 | 超时 | P1 |

#### 13A.7 OCW/管理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-027 | auto_confirm_payment(15)：买家忘确认时 OCW 代付确认 | R19 | 正向 | P1 |
| NM-028 | 奖励转账失败不阻断结算（reward_paid=false） | R19 | 安全 | P2 |
| NM-029 | fund_seed_account(13) + seed_liquidity(14)：种子账户注资 + 批量挂免保证金卖单 | R1 | 正向 | P2 |
| NM-030 | 查询过滤已过期订单 | R20 | 功能 | P2 |

### 13B. Entity Market — Entity Token 二级市场

> Pallet: `pallet-entity-market` | Extrinsics: 22 (call_index 0-21)
> 支持 NEX 对价 + USDT 对价两种模式 + 市价单

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| EM-001 | place_sell_order(0) / place_buy_order(1)：NEX 对价挂卖单/买单 | R8 | 正向 | P0 |
| EM-002 | take_order(2)：吃单（部分/全部成交） | R8 | 正向 | P0 |
| EM-003 | cancel_order(3)：取消订单 | R8 | 正向 | P1 |
| EM-004 | market_buy(12) / market_sell(13)：市价买入/卖出（滑点保护 max_cost/min_receive） | R8 | 正向 | P1 |
| EM-005 | configure_market(4)：配置实体市场（手续费/最小金额等） | R2 | 正向 | P1 |
| EM-006 | configure_price_protection(15)：价格保护配置（max_slippage/circuit_breaker） | R2 | 正向 | P1 |
| EM-007 | set_initial_price(17) / lift_circuit_breaker(16)：初始价格/解除熔断 | R2 | 正向 | P2 |
| EM-008 | place_usdt_sell_order(5) / place_usdt_buy_order(6)：USDT 对价挂单 | R8 | 正向 | P1 |
| EM-009 | reserve_usdt_sell_order(7) / accept_usdt_buy_order(8)：USDT 吃单（锁定 Token + NEX 保证金） | R8 | 正向 | P1 |
| EM-010 | confirm_usdt_payment(9) → verify_usdt_payment(10, OCW) → 结算 | R8 → R19 | 流程 | P1 |
| EM-011 | submit_ocw_result(18) / claim_verification_reward(19)：OCW 结果提交 + 奖励领取 | R19 | 正向 | P1 |
| EM-012 | submit_underpaid_update(20) / finalize_underpaid(21)：USDT 少付补付流程 | R19 | 流程 | P2 |
| EM-013 | process_usdt_timeout(11)：USDT 交易超时处理（买家保证金按 DepositForfeitRate 没收给卖家） | R20 | 超时 | P1 |
| EM-014 | fund_user_account(21)：用户充值账户 | R20 | 正向 | P2 |

详细测试续表见: [NEXUS_TEST_PLAN_PART3.md](./NEXUS_TEST_PLAN_PART3.md)
