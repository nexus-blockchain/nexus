# Nexus 测试计划 — Part 2

> 续 [NEXUS_TEST_PLAN.md](./NEXUS_TEST_PLAN.md)

---

## 8. Entity Member — 会员管理

> Pallet: `pallet-entity-member` | Extrinsics: 19 (call_index 0-1, 4-20, 2-3 reserved)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| MB-001 | register_member：正常注册会员 | R7 | 正向 | P0 |
| MB-002 | 带推荐人注册（referrer 参数） | R7 | 正向 | P0 |
| MB-003 | bind_referrer：后绑定推荐人 | R7 | 正向 | P1 |
| MB-004 | 已有推荐人不可重复绑定 | R7 | 负向 | P1 |
| MB-005 | 不能推荐自己 | R7 | 负向 | P1 |
| MB-006 | 下单时自动注册（ensure_member） | 系统 | 流程 | P0 |
| MB-007 | set_member_policy：设置会员策略（0=开放, 1=需购买, 2=需推荐人, 4=需审批，可组合） | R2 | 正向 | P1 |
| MB-008 | 需审批策略：注册→Pending→approve_member→Active | R2/R3 | 流程 | P1 |
| MB-009 | reject_member：拒绝审批 | R2/R3 | 正向 | P2 |
| MB-010 | set_member_stats_policy：设置统计策略（0=排除复购, 1=直推含复购, 2=间推含复购） | R2 | 正向 | P2 |
| MB-011 | init_level_system：初始化等级系统（use_custom + upgrade_mode） | R2 | 正向 | P0 |
| MB-012 | add_custom_level：添加自定义等级（阈值/折扣/佣金加成） | R2 | 正向 | P0 |
| MB-013 | 超过 MaxCustomLevels(10) 被拒绝 | R2 | 负向 | P1 |
| MB-014 | update_custom_level / remove_custom_level：更新/删除等级 | R2 | 正向 | P1 |
| MB-015 | manual_upgrade_member：手动升级会员 | R2 | 正向 | P1 |
| MB-016 | set_upgrade_mode / set_conflict_strategy：设置升级模式/冲突策略 | R2 | 正向 | P2 |
| MB-017 | set_use_custom_levels：启用/禁用自定义等级 | R2 | 正向 | P2 |
| MB-018 | init_upgrade_rule_system：初始化升级规则系统 | R2 | 正向 | P1 |
| MB-019 | add_upgrade_rule：添加规则（PurchaseProduct/ReferralCount/TeamSize/TotalSpent/OrderCount） | R2 | 正向 | P1 |
| MB-020 | 新触发器：ReferralLevelCount / TotalSpentUsdt / SingleOrderUsdt | R2 | 正向 | P1 |
| MB-021 | update_upgrade_rule / remove_upgrade_rule：更新/删除规则 | R2 | 正向 | P1 |
| MB-022 | set_upgrade_rule_system_enabled：启用/禁用规则系统 | R2 | 正向 | P2 |
| MB-023 | 规则触发自动升级（订单路径: update_spent → evaluate_rules） | 系统 | 流程 | P0 |
| MB-024 | 规则触发自动升级（推荐路径: bind_referrer → evaluate_rules） | 系统 | 流程 | P0 |
| MB-025 | 升级后级联检查推荐人 ReferralLevelCount 规则 | 系统 | 流程 | P1 |
| MB-026 | 等级过期 → get_effective_level 返回回退等级 | 系统 | 功能 | P1 |
| MB-027 | update_spent 检测过期等级并修正（审计 P4 回归） | 系统 | 功能 | P0 |
| MB-028 | USDT 消费独立追踪 MemberSpentUsdt（审计 P3 回归） | 系统 | 功能 | P0 |

## 9. Commission 模块群 — 返佣系统

### 9.1 Commission Core — 核心引擎

> Pallet: `pallet-entity-commission-core` | Extrinsics: 14 (call_index 0-6, 8, 10-13)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-001 | init_commission_plan：一键初始化佣金方案（CommissionPlan 枚举） | R2 | 正向 | P0 |
| CM-002 | set_commission_modes：设置返佣模式（Referral/LevelDiff/SingleLine/Team/PoolReward） | R2 | 正向 | P0 |
| CM-003 | set_commission_rate / enable_commission：设置返佣上限/启用禁用 | R2 | 正向 | P1 |
| CM-004 | withdraw_commission：提现 NEX 佣金（四种 WithdrawalMode） | R7 | 正向 | P0 |
| CM-005 | withdraw_token_commission：提现 Token 佣金 | R7 | 正向 | P1 |
| CM-006 | 自愿复购到购物余额 | R7 | 正向 | P1 |
| CM-007 | 复购目标为非会员 → 自动注册 | R7 | 流程 | P2 |
| CM-008 | 复购目标为已有会员 → 推荐人必须是出资人 | R7 | 负向 | P2 |
| CM-009 | use_shopping_balance 已禁用（购物余额仅可购物） | R7 | 安全 | P0 |
| CM-010 | set_withdrawal_config / set_token_withdrawal_config：设置 NEX/Token 提现配置 | R2 | 正向 | P1 |
| CM-011 | set_global_min_token_repurchase_rate：全局最低 Token 复购比例 | R1 | 正向 | P2 |
| CM-012 | withdraw_entity_funds：提取 Entity NEX 资金 | R2 | 正向 | P1 |
| CM-013 | 提取后余额 ≥ PendingTotal + ShoppingTotal + UnallocatedPool | R2 | 安全 | P0 |
| CM-014 | withdraw_entity_token_funds：提取 Entity Token 资金 | R2 | 正向 | P2 |

### 9.2 佣金分配流程（process_commission / process_token_commission）

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-015 | 池 A（平台费池）→ Entity 推荐人获得 ReferrerShareBps 分成 | 系统 | 流程 | P1 |
| CM-016 | 池 B（卖家池）→ Referral 推荐佣金（多级） | 系统 | 流程 | P0 |
| CM-017 | 池 B → LevelDiff 级差佣金 | 系统 | 流程 | P1 |
| CM-018 | 池 B → SingleLine 单线佣金（order_amount × rate / 10000） | 系统 | 流程 | P1 |
| CM-019 | 池 B → Team 团队佣金 | 系统 | 流程 | P1 |
| CM-020 | 池 B → PoolReward 沉淀池分配 | 系统 | 流程 | P1 |
| CM-021 | cancel_commission：订单取消 → 退佣（审计 H2: 先转账后清记录） | 系统 | 流程 | P0 |
| CM-022 | do_cancel_token_commission：Token 佣金独立取消 | 系统 | 流程 | P1 |
| CM-023 | Token 订单双路佣金分配（process_token_commission） | 系统 | 流程 | P1 |
| CM-024 | KYC 参与检查（KycParticipationGuard） | R7 | 安全 | P1 |

### 9.3 Commission 插件

> Referral: `pallet-entity-commission-referral` (5 extrinsics)
> LevelDiff: `pallet-entity-commission-level-diff` (1 extrinsic)
> SingleLine: `pallet-entity-commission-single-line` (3 extrinsics)
> Team: `pallet-entity-commission-team` (1 extrinsic)
> PoolReward: `pallet-entity-commission-pool-reward` (4 extrinsics)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-025 | Referral: set_direct_reward_config / set_multi_level_config / set_fixed_amount_config / set_first_order_config / set_repeat_purchase_config | R2 | 正向 | P1 |
| CM-026 | Referral: 多级推荐佣金费率验证 | 系统 | 功能 | P1 |
| CM-027 | LevelDiff: set_level_diff_config（级差比例 bps） | R2 | 正向 | P1 |
| CM-028 | LevelDiff: 级差计算（上级 - 下级差值） | 系统 | 功能 | P1 |
| CM-029 | SingleLine: set_single_line_config（upline/downline 费率 + 基础/最大层数） | R2 | 正向 | P1 |
| CM-030 | SingleLine: set_level_based_levels / remove_level_based_levels（等级自定义层数） | R2 | 正向 | P2 |
| CM-031 | SingleLine: calc_extra_levels 溢出保护（.min(255)） | 系统 | 安全 | P2 |
| CM-032 | Team: set_team_performance_config（团队业绩返佣配置） | R2 | 正向 | P1 |
| CM-033 | PoolReward: set_pool_reward_config（NEX + Token 双池配置） | R1 | 正向 | P1 |
| CM-034 | PoolReward: claim_pool_reward（用户领取沉淀池奖励，冷却期 ~7 天） | R7 | 正向 | P1 |
| CM-035 | PoolReward: force_new_round（Root 强制开启新轮次） | R1 | 正向 | P2 |
| CM-036 | PoolReward: set_token_pool_enabled（启用/禁用 Token 池） | R1 | 正向 | P2 |

## 10. Entity Disclosure — 财务披露 & 公告

> Pallet: `pallet-entity-disclosure` | Extrinsics: 12 (call_index 0-11)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| DC-001 | configure_disclosure：配置披露设置（级别/间隔） | R2 | 正向 | P1 |
| DC-002 | publish_disclosure：发布披露（年度/季度/月度/事件驱动） | R2 | 正向 | P1 |
| DC-003 | withdraw_disclosure / correct_disclosure：撤回/更正披露 | R2 | 正向 | P2 |
| DC-004 | 披露间隔未到不可发布 | R2 | 负向 | P2 |
| DC-005 | add_insider / remove_insider：添加/移除内幕人员（≤50 人） | R2 | 正向 | P1 |
| DC-006 | start_blackout / end_blackout：手动开始/结束黑窗口期 | R2 | 正向 | P1 |
| DC-007 | 黑窗口期 ≤ MaxBlackoutDuration（~7 天） | R2 | 边界 | P2 |
| DC-008 | publish_announcement / update_announcement / withdraw_announcement：公告 CRUD | R2 | 正向 | P1 |
| DC-009 | pin_announcement：置顶/取消置顶公告 | R2 | 正向 | P2 |

## 11. Entity KYC — 认证管理

> Pallet: `pallet-entity-kyc` | Extrinsics: 8 (call_index 0-7)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| KY-001 | submit_kyc：提交 KYC（Basic/Standard/Enhanced/Institutional） | R20 | 正向 | P0 |
| KY-002 | 空 data_cid 被拒绝（EmptyDataCid） | R20 | 负向 | P1 |
| KY-003 | 非法国家代码被拒绝（需大写 ASCII） | R20 | 负向 | P1 |
| KY-004 | approve_kyc：批准 KYC（设有效期 + risk_score） | R10 | 正向 | P0 |
| KY-005 | Provider 等级不够不能批准高级别 | R10 | 负向 | P1 |
| KY-006 | reject_kyc：拒绝 KYC（检查 provider 等级） | R10 | 正向 | P1 |
| KY-007 | revoke_kyc：撤销 KYC | R1 | 正向 | P1 |
| KY-008 | register_provider / remove_provider：注册/移除 Provider | R1 | 正向 | P1 |
| KY-009 | set_entity_requirement：设置实体 KYC 要求（max_risk_score ≤ 100） | R2 | 正向 | P1 |
| KY-010 | max_risk_score > 100 被拒绝 | R2 | 负向 | P1 |
| KY-011 | update_high_risk_countries：更新高风险国家列表 | R1 | 正向 | P2 |
| KY-012 | 各级别有效期验证（Basic ~1 年, Standard ~6 月, Institutional ~2 年） | 系统 | 功能 | P2 |
| KY-013 | 宽限期内仍可参与实体活动 | 系统 | 功能 | P1 |
| KY-014 | 过期后 can_participate_in_entity = false | 系统 | 功能 | P1 |

## 12. Entity TokenSale — 代币发售

> Pallet: `pallet-entity-tokensale` | Extrinsics: 14 (call_index 0-13)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| TS-001 | create_sale_round(FixedPrice)：创建固定价格发售轮次 | R2 | 正向 | P0 |
| TS-002 | create_sale_round(DutchAuction)：创建荷兰拍卖轮次 | R2 | 正向 | P1 |
| TS-003 | add_payment_option：添加支付选项 | R2 | 正向 | P0 |
| TS-004 | set_vesting_config / configure_dutch_auction：设置锁仓/拍卖配置 | R2 | 正向 | P1 |
| TS-005 | add_to_whitelist：添加白名单 | R2 | 正向 | P2 |
| TS-006 | start_sale：开始发售（锁定 Entity 代币） | R2 | 正向 | P0 |
| TS-007 | 无支付选项不能开始 | R2 | 负向 | P0 |
| TS-008 | DutchAuction 未配置不能开始 | R2 | 负向 | P1 |
| TS-009 | subscribe：认购（NEX 锁入托管） | R5 | 正向 | P0 |
| TS-010 | KYC 等级不足被拒绝 | R5 | 负向 | P1 |
| TS-011 | 白名单模式下非白名单被拒绝 | R5 | 负向 | P1 |
| TS-012 | end_sale：结束发售（now ≥ end_block 或已售罄） | R2 | 正向 | P0 |
| TS-013 | 提前结束（未到 end_block 且未售罄）被拒绝 | R2 | 负向 | P1 |
| TS-014 | claim_tokens：领取代币（初始解锁部分） | R5 | 正向 | P0 |
| TS-015 | unlock_tokens：解锁代币（锁仓期后） | R5 | 正向 | P1 |
| TS-016 | 锁仓期未到无法解锁 | R5 | 负向 | P1 |
| TS-017 | cancel_sale → claim_refund：取消发售 → 领取退款 | R2 → R5 | 流程 | P1 |
| TS-018 | reclaim_unclaimed_tokens：回收过期未领退款（宽限期后） | R2 | 正向 | P2 |
| TS-019 | withdraw_funds：提取募集资金（仅 Ended/Completed） | R2 | 正向 | P1 |

## 13. 交易市场模块

### 13A. NEX Market — P2P NEX/USDT 交易所

> Pallet: `pallet-trading-nex-market` | Extrinsics: 18 (call_index 0-17)

#### 13A.1 卖单流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-001 | place_sell_order：挂卖单（锁 NEX，提供 TRON 地址） | R9 | 正向 | P0 |
| NM-002 | reserve_sell_order：买家吃卖单（锁保证金） | R9 | 正向 | P0 |
| NM-003 | confirm_payment：买家声明已付款 | R9 | 正向 | P0 |
| NM-004 | submit_ocw_result：OCW 验证 USDT 到账（tx_hash 防重放） | R19 | 正向 | P0 |
| NM-005 | claim_verification_reward：领取奖励（释放 NEX + 退保证金 + 更新 TWAP） | R9 | 正向 | P0 |
| NM-006 | cancel_order：取消卖单（退还锁定 NEX） | R9 | 正向 | P1 |

#### 13A.2 买单流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-007 | place_buy_order：挂买单（预锁保证金） | R9 | 正向 | P0 |
| NM-008 | accept_buy_order：卖家接买单（锁 NEX + 锁买家保证金） | R9 | 正向 | P0 |
| NM-009 | 买家确认 → OCW 验证 → 结算 | R9 → R19 | 流程 | P0 |
| NM-010 | 取消买单（退保证金） | R9 | 正向 | P1 |

#### 13A.3 价格保护

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-011 | 价格偏离 TWAP 超阈值被拒绝 | R9 | 安全 | P0 |
| NM-012 | 熔断触发 → 所有交易暂停 | 系统 | 安全 | P0 |
| NM-013 | lift_circuit_breaker：手动解除熔断 | R1 | 正向 | P1 |
| NM-014 | configure_price_protection / set_initial_price：设置价格保护/基准价格 | R1 | 正向 | P1 |
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
| NM-021 | submit_underpaid_update：OCW 补付窗口内更新金额 | R19 | 正向 | P0 |
| NM-022 | 累计达 99.5% 自动升级为 Exact | R19 | 功能 | P1 |
| NM-023 | finalize_underpaid：补付终裁（梯度没收: ≥99.5%→0%, 95%~99.5%→20%, 80%~95%→50%, <80%→100%） | R19 | 功能 | P0 |

#### 13A.6 超时处理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-024 | process_timeout(AwaitingPayment)：退 NEX + 没收保证金 | R20 | 超时 | P0 |
| NM-025 | process_timeout(AwaitingVerification) + 宽限期(1h) | R20 | 超时 | P1 |
| NM-026 | process_timeout(UnderpaidPending) | R20 | 超时 | P1 |

#### 13A.7 OCW/管理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-027 | auto_confirm_payment：买家忘确认时 OCW 代付确认 | R19 | 正向 | P1 |
| NM-028 | 奖励转账失败不阻断结算（reward_paid=false） | R19 | 安全 | P2 |
| NM-029 | fund_seed_account + seed_liquidity：种子账户注资 + 批量挂免保证金卖单 | R1 | 正向 | P2 |
| NM-030 | 查询过滤已过期订单 | R20 | 功能 | P2 |

### 13B. Entity Market — Entity Token 二级市场

> Pallet: `pallet-entity-market` | Extrinsics: 21 (call_index 0-21)
> 支持 NEX 对价 + USDT 对价两种模式

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| EM-001 | place_sell_order / place_buy_order：NEX 对价挂卖单/买单 | R8 | 正向 | P0 |
| EM-002 | take_order：吃单（部分/全部成交） | R8 | 正向 | P0 |
| EM-003 | cancel_order：取消订单 | R8 | 正向 | P1 |
| EM-004 | market_buy / market_sell：市价买入/卖出（滑点保护 max_cost/min_receive） | R8 | 正向 | P1 |
| EM-005 | configure_market：配置实体市场（手续费/最小金额等） | R2 | 正向 | P1 |
| EM-006 | configure_price_protection：价格保护配置（max_slippage/circuit_breaker） | R2 | 正向 | P1 |
| EM-007 | set_initial_price / lift_circuit_breaker：初始价格/解除熔断 | R2 | 正向 | P2 |
| EM-008 | place_usdt_sell_order / place_usdt_buy_order：USDT 对价挂单 | R8 | 正向 | P1 |
| EM-009 | reserve_usdt_sell_order / accept_usdt_buy_order：USDT 吃单（锁定 Token + 保证金） | R8 | 正向 | P1 |
| EM-010 | confirm_usdt_payment → verify_usdt_payment(OCW) → 结算 | R8 → R19 | 流程 | P1 |
| EM-011 | submit_ocw_result / claim_verification_reward：OCW 结果提交 + 奖励领取 | R19 | 正向 | P1 |
| EM-012 | submit_underpaid_update / finalize_underpaid：USDT 少付补付流程 | R19 | 流程 | P2 |
| EM-013 | process_usdt_timeout：USDT 交易超时处理（买家保证金没收） | R20 | 超时 | P1 |

详细测试续表见: [NEXUS_TEST_PLAN_PART3.md](./NEXUS_TEST_PLAN_PART3.md)
