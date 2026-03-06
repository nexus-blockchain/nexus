# Nexus 测试计划 — Part 2

> 续 [NEXUS_TEST_PLAN.md](./NEXUS_TEST_PLAN.md)
> 更新日期: 2026-03-06

---

## 8. Entity Member — 会员管理

> Pallet: `pallet-entity-member` | Extrinsics: 28+
> 新增: ban_member, unban_member, activate_member, deactivate_member,
> remove_member, set_member_level, reset_level_system, reset_upgrade_rule_system,
> leave_entity, cancel_pending_member

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| MB-001 | register_member：正常注册会员 | R7 | 正向 | P0 |
| MB-002 | 带推荐人注册 | R7 | 正向 | P0 |
| MB-003 | bind_referrer：后绑定推荐人 | R7 | 正向 | P1 |
| MB-004 | 已有推荐人不可重复绑定 / 不能推荐自己 | R7 | 负向 | P1 |
| MB-005 | set_level_system：初始化等级（use_custom + upgrade_mode） | R2 | 正向 | P0 |
| MB-006 | add_custom_level / update_custom_level / remove_custom_level：等级 CRUD | R2 | 正向 | P0 |
| MB-007 | manually_upgrade_member：手动升级 | R2 | 正向 | P1 |
| MB-008 | init_upgrade_rule_system / add_upgrade_rule / update/remove：规则系统 | R2 | 正向 | P1 |
| MB-009 | set_member_policy：设置策略（开放/需购买/需推荐/需审批） | R2 | 正向 | P1 |
| MB-010 | set_member_stats_policy：统计策略 | R2 | 正向 | P2 |
| MB-011 | approve_member / reject_member / batch_approve / batch_reject：审批 | R2/R3 | 流程 | P1 |
| MB-012 | **ban_member / unban_member：封禁/解禁会员** | R2 | 正向 | P1 |
| MB-013 | **activate_member / deactivate_member：激活/停用** | R2 | 正向 | P1 |
| MB-014 | **remove_member：移除会员** | R2 | 正向 | P1 |
| MB-015 | **set_member_level：直接设置等级** | R2 | 正向 | P1 |
| MB-016 | **reset_level_system：重置等级系统** | R2 | 正向 | P2 |
| MB-017 | **reset_upgrade_rule_system：重置规则系统** | R2 | 正向 | P2 |
| MB-018 | **leave_entity：会员主动离开** | R7 | 正向 | P1 |
| MB-019 | **cancel_pending_member：取消待审批** | R7 | 正向 | P2 |
| MB-020 | 规则触发自动升级（订单路径） | 系统 | 流程 | P0 |
| MB-021 | 规则触发自动升级（推荐路径） | 系统 | 流程 | P0 |
| MB-022 | 等级过期→回退等级+MemberLevelExpired 事件 | 系统 | 功能 | P1 |
| MB-023 | USDT 消费累加正确性 | 系统 | 功能 | P0 |

## 9. Commission 模块群 — 返佣系统

### 9.1 Commission Core — 核心引擎

> Pallet: `pallet-commission-core` | Extrinsics: 26
> 新增: set_creator_reward_rate, set_token_platform_fee_rate, set_withdrawal_cooldown,
> force_disable_entity_commission, set_global_max_commission_rate, clear_*,
> force_global_pause, pause_withdrawals, archive_order_records, set_global_min_token_repurchase_rate

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-001 | init_commission_plan：一键初始化佣金方案 | R2 | 正向 | P0 |
| CM-002 | set_commission_modes：设置返佣模式组合 | R2 | 正向 | P0 |
| CM-003 | set_commission_rate / enable_commission：费率+开关 | R2 | 正向 | P1 |
| CM-004 | withdraw_commission：提现 NEX 佣金（4 种 WithdrawalMode） | R7 | 正向 | P0 |
| CM-005 | withdraw_token_commission：Token 佣金提现 | R7 | 正向 | P1 |
| CM-006 | use_shopping_balance 已禁用（购物余额仅可购物） | R7 | 安全 | P0 |
| CM-007 | set_withdrawal_config / set_token_withdrawal_config | R2 | 正向 | P1 |
| CM-008 | **set_creator_reward_rate：设置创建者奖励** | R2 | 正向 | P1 |
| CM-009 | **set_token_platform_fee_rate：Token 平台费** | R1 | 正向 | P1 |
| CM-010 | **set_global_min_repurchase_rate：全局最低复购比例** | R1 | 正向 | P2 |
| CM-011 | **set_withdrawal_cooldown：提现冷却期** | R2 | 正向 | P1 |
| CM-012 | withdraw_entity_funds / withdraw_entity_token_funds：提取资金 | R2 | 正向 | P1 |
| CM-013 | 提取后余额 ≥ PendingTotal + ShoppingTotal + UnallocatedPool | R2 | 安全 | P0 |
| CM-014 | **force_disable_entity_commission：Root 强制禁用** | R1 | 正向 | P1 |
| CM-015 | **set_global_max_commission_rate / set_global_max_token_commission_rate** | R1 | 正向 | P1 |
| CM-016 | **clear_commission_config / clear_withdrawal_config / clear_token_withdrawal_config** | R2 | 正向 | P2 |
| CM-017 | **force_global_pause：全局佣金暂停** | R1 | 正向 | P1 |
| CM-018 | **pause_withdrawals：暂停提现** | R2 | 正向 | P1 |
| CM-019 | **archive_order_records：归档订单佣金记录** | R2 | 正向 | P2 |
| CM-020 | **set_global_min_token_repurchase_rate** | R1 | 正向 | P2 |

### 9.2 佣金分配流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-021 | 池 A（平台费）→ Entity 推荐人分成 | 系统 | 流程 | P1 |
| CM-022 | 池 B → Referral 推荐佣金（多级） | 系统 | 流程 | P0 |
| CM-023 | 池 B → LevelDiff 级差佣金 | 系统 | 流程 | P1 |
| CM-024 | 池 B → SingleLine 单线佣金 | 系统 | 流程 | P1 |
| CM-025 | 池 B → Team 团队佣金 | 系统 | 流程 | P1 |
| CM-026 | 池 B → PoolReward 沉淀池分配 | 系统 | 流程 | P1 |
| CM-027 | 池 B → MultiLevel 多级佣金 | 系统 | 流程 | P1 |
| CM-028 | cancel_commission：订单取消→退佣 | 系统 | 流程 | P0 |
| CM-029 | Token 订单双路佣金分配 | 系统 | 流程 | P1 |

### 9.3 Commission 插件

> Referral | MultiLevel | LevelDiff | SingleLine | Team | PoolReward
> 新增各插件: force_set_*, force_clear_*, pause_*, resume_*, schedule_config_change,
> apply_pending_config, cancel_pending_config, add_tier, remove_tier, update_tier

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CM-030 | Referral: set_direct_reward_config / set_fixed_amount / set_first_order / set_repeat_purchase | R2 | 正向 | P1 |
| CM-031 | **Referral: clear_referral_config / set_referrer_guard_config / set_commission_cap_config** | R2 | 正向 | P1 |
| CM-032 | **Referral: set_referral_validity_config / set_config_effective_after** | R2 | 正向 | P2 |
| CM-033 | MultiLevel: set_multi_level_config / clear_multi_level_config | R2 | 正向 | P1 |
| CM-034 | **MultiLevel: update_multi_level_params / add_tier / remove_tier** | R2 | 正向 | P1 |
| CM-035 | **MultiLevel: pause/resume_multi_level** | R2 | 正向 | P1 |
| CM-036 | **MultiLevel: schedule_config_change / apply_pending_config / cancel_pending_config** | R2 | 正向 | P2 |
| CM-037 | LevelDiff: set_level_diff_config / clear / update | R2 | 正向 | P1 |
| CM-038 | SingleLine: set_single_line_config / update_params | R2 | 正向 | P1 |
| CM-039 | **SingleLine: set_level_based_levels / remove_level_based_levels** | R2 | 正向 | P2 |
| CM-040 | **SingleLine: pause/resume_single_line** | R2 | 正向 | P1 |
| CM-041 | Team: set_team_performance_config / clear / update_params | R2 | 正向 | P1 |
| CM-042 | **Team: add_tier / update_tier / remove_tier** | R2 | 正向 | P1 |
| CM-043 | **Team: pause/resume_team_performance** | R2 | 正向 | P1 |
| CM-044 | PoolReward: set_pool_reward_config / claim_pool_reward | R1/R7 | 正向 | P1 |
| CM-045 | **PoolReward: force_new_round / set_token_pool_enabled** | R1 | 正向 | P2 |
| CM-046 | **PoolReward: pause/resume_pool_reward / set_global_pool_reward_paused** | R1 | 正向 | P1 |
| CM-047 | **All plugins: force_set_* / force_clear_* (Root)** | R1 | 正向 | P1 |
| CM-048 | KYC 参与检查（KycParticipationGuard）阻止 claim | R7 | 安全 | P1 |

## 10. Entity Disclosure — 财务披露 & 公告

> Pallet: `pallet-entity-disclosure` | Extrinsics: 16+
> 新增: update_insider_role, report_disclosure_violation, force_configure_disclosure,
> clean_entity_disclosures

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| DC-001 | configure_disclosure：配置披露设置 | R2 | 正向 | P1 |
| DC-002 | create_draft / update_draft / delete_draft：草稿 CRUD | R2 | 正向 | P1 |
| DC-003 | publish_disclosure / withdraw_disclosure / correct_disclosure：发布/撤回/更正 | R2 | 正向 | P1 |
| DC-004 | add_insider / remove_insider：内幕人员管理（≤50） | R2 | 正向 | P1 |
| DC-005 | **update_insider_role：更新内幕人员角色** | R2 | 正向 | P1 |
| DC-006 | **report_disclosure_violation：报告违规** | R20 | 正向 | P1 |
| DC-007 | publish_announcement / update / withdraw / pin / unpin：公告管理 | R2 | 正向 | P1 |
| DC-008 | **force_configure_disclosure：Root 强制配置** | R1 | 正向 | P1 |
| DC-009 | **clean_entity_disclosures：Root 清理** | R1 | 正向 | P2 |
| DC-010 | 披露间隔未到不可发布 | R2 | 负向 | P2 |
| DC-011 | 非 Entity Owner/Admin 操作被拒绝 | R20 | 权限 | P1 |

## 11. Entity KYC — 认证管理

> Pallet: `pallet-entity-kyc` | Extrinsics: 23 (call_index 0-22)
> 新增: cancel_kyc, update_provider, suspend/resume_provider, update_risk_score,
> force_approve_kyc, update/purge_kyc_data, remove_entity_requirement,
> timeout_pending_kyc, revoke_provider_kycs, force_remove_provider,
> authorize/deauthorize_provider

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| KY-001 | submit_kyc(0)：提交 KYC（Basic/Standard/Enhanced/Institutional） | R20 | 正向 | P0 |
| KY-002 | 空 data_cid / 非法国家代码被拒绝 | R20 | 负向 | P1 |
| KY-003 | approve_kyc(1)：批准 KYC | R10 | 正向 | P0 |
| KY-004 | reject_kyc(2)：拒绝 KYC | R10 | 正向 | P1 |
| KY-005 | revoke_kyc(3)：撤销 KYC | R1 | 正向 | P1 |
| KY-006 | renew_kyc(4)：续期 KYC | R10 | 正向 | P1 |
| KY-007 | register_provider(5) / remove_provider(6) | R1 | 正向 | P1 |
| KY-008 | set_entity_requirement(7)：max_risk_score ≤ 100 | R2 | 正向+负向 | P1 |
| KY-009 | update_high_risk_countries(8)：自动 sort+dedup | R1 | 正向 | P2 |
| KY-010 | **cancel_kyc(9)：取消 KYC 申请** | R20 | 正向 | P1 |
| KY-011 | **update_provider(10)：更新 Provider 信息** | R1 | 正向 | P1 |
| KY-012 | **suspend_provider(11) / resume_provider(12)：暂停/恢复 Provider** | R1 | 正向 | P1 |
| KY-013 | **update_risk_score(13)：更新风险分** | R10 | 正向 | P1 |
| KY-014 | **force_approve_kyc(14)：Root 强制批准** | R1 | 正向 | P1 |
| KY-015 | **update_kyc_data(15)：更新 KYC 数据** | R20 | 正向 | P1 |
| KY-016 | **purge_kyc_data(16)：Admin 清除 KYC 数据** | R1 | 正向 | P1 |
| KY-017 | **remove_entity_requirement(17)：移除实体 KYC 要求** | R2 | 正向 | P1 |
| KY-018 | **timeout_pending_kyc(18)：超时待审 KYC** | R20 | 正向 | P2 |
| KY-019 | **revoke_provider_kycs(19)：撤销 Provider 下所有 KYC** | R1 | 正向 | P1 |
| KY-020 | **force_remove_provider(20)：Root 强制移除 Provider** | R1 | 正向 | P1 |
| KY-021 | **authorize_provider(21) / deauthorize_provider(22)：实体授权/取消 Provider** | R2 | 正向 | P1 |
| KY-022 | 过期后 can_participate_in_entity = false | 系统 | 功能 | P1 |
| KY-023 | 宽限期内仍可参与 | 系统 | 功能 | P1 |

## 12. Entity TokenSale — 代币发售

> Pallet: `pallet-entity-tokensale` | Extrinsics: 27 (call_index 0-26)
> 新增: force_cancel_sale, force_end_sale, force_refund, force_withdraw_funds,
> update_sale_round, increase_subscription, remove_from_whitelist, remove_payment_option,
> extend_sale, pause/resume_sale, cleanup_round, force_batch_refund

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| TS-001 | create_sale_round(0, FixedPrice/DutchAuction) | R2 | 正向 | P0 |
| TS-002 | add_payment_option(1) / remove_payment_option(21) | R2 | 正向 | P0 |
| TS-003 | set_vesting_config(2) / configure_dutch_auction(3) | R2 | 正向 | P1 |
| TS-004 | add_to_whitelist(4) / remove_from_whitelist(20) | R2 | 正向 | P2 |
| TS-005 | start_sale(5)：开始（锁 Token） | R2 | 正向 | P0 |
| TS-006 | subscribe(6)：认购 / increase_subscription(19)：增加 | R5 | 正向 | P0 |
| TS-007 | end_sale(7)：结束 | R2 | 正向 | P0 |
| TS-008 | claim_tokens(8)：领取代币 | R5 | 正向 | P0 |
| TS-009 | unlock_tokens(9)：锁仓期后解锁 | R5 | 正向 | P1 |
| TS-010 | cancel_sale(10) → claim_refund(11)：取消→退款 | R2→R5 | 流程 | P1 |
| TS-011 | withdraw_funds(12) / reclaim_unclaimed_tokens(13) | R2 | 正向 | P1 |
| TS-012 | **force_cancel_sale(14) / force_end_sale(15)：Root 强制** | R1 | 正向 | P1 |
| TS-013 | **force_refund(16)：Root 强制退款指定认购者** | R1 | 正向 | P1 |
| TS-014 | **force_withdraw_funds(17)：Root 强制提取** | R1 | 正向 | P1 |
| TS-015 | **update_sale_round(18)：更新发售参数** | R2 | 正向 | P1 |
| TS-016 | **extend_sale(22)：延长发售期** | R2 | 正向 | P1 |
| TS-017 | **pause_sale(23) / resume_sale(24)：暂停/恢复** | R2 | 正向 | P1 |
| TS-018 | **cleanup_round(25)：清理已完成轮次** | R20 | 正向 | P2 |
| TS-019 | **force_batch_refund(26)：Root 批量退款** | R1 | 正向 | P1 |
| TS-020 | KYC 等级不足 / 白名单限制被拒绝 | R5 | 负向 | P1 |
| TS-021 | 无支付选项 / DutchAuction 未配置不能开始 | R2 | 负向 | P0 |

## 13. 交易市场模块

### 13A. NEX Market — P2P NEX/USDT 交易所

> Pallet: `pallet-trading-nex-market` | Extrinsics: 27 (call_index 0-26)
> 新增: force_pause/resume_market, force_settle/cancel_trade, dispute_trade,
> resolve_dispute, set_trading_fee, update_order_price, update_deposit_exchange_rate

#### 13A.1 卖单流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-001 | place_sell_order(0)：挂卖单（锁 NEX） | R9 | 正向 | P0 |
| NM-002 | reserve_sell_order(3)：买家吃卖单（锁保证金） | R9 | 正向 | P0 |
| NM-003 | confirm_payment(5)：买家声明已付款 | R9 | 正向 | P0 |
| NM-004 | submit_ocw_result(7)：OCW 验证 USDT 到账（tx_hash 防重放） | R19 | 正向 | P0 |
| NM-005 | claim_verification_reward(8)：释放 NEX + 退保证金 + TWAP 更新 | R9 | 正向 | P0 |
| NM-006 | cancel_order(2)：取消（退锁定 NEX） | R9 | 正向 | P1 |

#### 13A.2 买单流程

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-007 | place_buy_order(1)：挂买单 | R9 | 正向 | P0 |
| NM-008 | accept_buy_order(4)：卖家接买单 | R9 | 正向 | P0 |
| NM-009 | 买家确认 → OCW → 结算完整流程 | R9→R19 | 流程 | P0 |
| NM-010 | 取消买单 | R9 | 正向 | P1 |

#### 13A.3 价格保护 & 多档判定

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-011 | 价格偏离 TWAP 超阈值被拒绝 | R9 | 安全 | P0 |
| NM-012 | 熔断触发→暂停 / lift_circuit_breaker(11) 解除 | 系统/R1 | 安全 | P0 |
| NM-013 | configure_price_protection(9) / set_initial_price(10) | R1 | 正向 | P1 |
| NM-014 | Exact(99.5%~100.5%)/Overpaid(≥100.5%)：全额释放 | R19 | 功能 | P0 |
| NM-015 | Underpaid(50%~99.5%)→补付窗口 / SeverelyUnderpaid(<50%)：按比例 | R19 | 功能 | P0 |

#### 13A.4 补付/超时/争议

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| NM-016 | submit_underpaid_update(16) / finalize_underpaid(17)：补付流程 | R19 | 流程 | P0 |
| NM-017 | process_timeout(6)：三种超时（AwaitingPayment/Verification/Underpaid） | R20 | 超时 | P0 |
| NM-018 | auto_confirm_payment(15)：OCW 代确认 | R19 | 正向 | P1 |
| NM-019 | **force_pause_market(18) / force_resume_market(19)：强制暂停/恢复** | R1 | 正向 | P1 |
| NM-020 | **force_settle_trade(20)：强制结算** | R1 | 正向 | P1 |
| NM-021 | **force_cancel_trade(21)：强制取消** | R1 | 正向 | P1 |
| NM-022 | **dispute_trade(22)：交易争议** | R9 | 正向 | P1 |
| NM-023 | **resolve_dispute(23)：解决争议** | R1 | 正向 | P1 |
| NM-024 | **set_trading_fee(24)：设置交易手续费** | R1 | 正向 | P1 |
| NM-025 | **update_order_price(25)：更新挂单价格** | R9 | 正向 | P1 |
| NM-026 | **update_deposit_exchange_rate(26)：更新保证金汇率** | R1 | 正向 | P1 |
| NM-027 | fund_seed_account(13) + seed_liquidity(14)：种子流动性 | R1 | 正向 | P2 |
| NM-028 | 市场暂停时所有交易被拒绝 | R9 | 安全 | P0 |
| NM-029 | 已争议交易不可重复争议 / 已关闭不可争议 | R9 | 负向 | P1 |
| NM-030 | 活跃交易订单不可取消（OrderHasActiveTrades） | R9 | 负向 | P1 |

### 13B. Entity Market — Entity Token 二级市场

> Pallet: `pallet-entity-market` | Extrinsics: 22+
> 支持 5 种订单类型: limit, market, ioc, fok, post_only

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| EM-001 | place_limit_order / place_market_order：限价/市价 | R8 | 正向 | P0 |
| EM-002 | place_ioc_order / place_fok_order / place_post_only_order：IOC/FOK/PostOnly | R8 | 正向 | P1 |
| EM-003 | cancel_order / take_order：取消/吃单 | R8 | 正向 | P0 |
| EM-004 | enable_market / disable_market / update_market_config | R2 | 正向 | P1 |
| EM-005 | set_price_protection / set_initial_price / trigger_circuit_breaker | R2 | 正向 | P1 |
| EM-006 | set_market_kyc_requirement：KYC 限制 | R2 | 正向 | P1 |
| EM-007 | force_cancel_order / force_close_market / set_global_market_paused (Root) | R1 | 正向 | P1 |
| EM-008 | 全局暂停时交易被拒绝 | R8 | 安全 | P0 |

## 14. Escrow — 托管模块

> Pallet: `pallet-escrow` | Extrinsics: 20 (call_index 0-19)
> 新增: refund_partial, release_partial, cleanup_closed, token_lock/release/refund,
> force_release, force_refund

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| ES-001 | lock(0) / lock_with_nonce(3)：锁定（nonce 递增） | 授权模块 | 正向 | P0 |
| ES-002 | release(1) / refund(2)：释放/退款 | 授权模块 | 正向 | P0 |
| ES-003 | release_split(4)：分账释放 | 授权模块 | 正向 | P1 |
| ES-004 | dispute(5)：进入争议 | 授权模块 | 正向 | P1 |
| ES-005 | 争议状态下 release/refund 被拒绝（DisputeActive） | 授权模块 | 安全 | P0 |
| ES-006 | apply_decision_release_all(6) / refund_all(7) / partial_bps(8) | R1/R11 | 正向 | P1 |
| ES-007 | set_pause(9)：全局暂停 | R1 | 安全 | P1 |
| ES-008 | schedule_expiry(10) / cancel_expiry(11)：到期调度 | 授权模块 | 正向 | P2 |
| ES-009 | **force_release(12) / force_refund(13)：Admin 强制** | R1 | 正向 | P1 |
| ES-010 | **refund_partial(14) / release_partial(15)：部分退款/释放** | 授权模块 | 正向 | P1 |
| ES-011 | **cleanup_closed(16)：清理已关闭** | R20 | 正向 | P2 |
| ES-012 | **token_lock(17) / token_release(18) / token_refund(19)：Token 托管** | 授权模块 | 正向 | P1 |
| ES-013 | 已关闭 ID 重复操作被拒绝（AlreadyClosed） | 授权模块 | 负向 | P1 |
| ES-014 | on_initialize 自动处理到期 | 系统 | 功能 | P1 |

## 15. Evidence — 证据模块

> Pallet: `pallet-evidence` | Extrinsics: 24 (call_index 0-23)
> 新增: append_evidence, update_evidence_manifest, request_access, update_access_policy,
> reveal_commitment, seal/unseal_evidence, force_remove, withdraw_evidence,
> delete_private_content, force_archive, revoke_public_key, cancel_access_request

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| EV-001 | commit(0)：提交证据（图片/视频/文档 CID） | R5/R6 | 正向 | P0 |
| EV-002 | commit_hash(1)：提交哈希承诺 | R5/R6 | 正向 | P1 |
| EV-003 | link(2) / unlink(4) / link_by_ns(3) / unlink_by_ns(5) | 授权模块 | 正向 | P1 |
| EV-004 | register_public_key(6)：注册公钥 | R20 | 正向 | P1 |
| EV-005 | store_private_content(7)：存储私密内容 | R20 | 正向 | P1 |
| EV-006 | grant_access(8) / revoke_access(9)：授予/撤销 | R20 | 正向 | P1 |
| EV-007 | rotate_content_keys(10)：轮换密钥 | R20 | 正向 | P2 |
| EV-008 | **append_evidence(11)：追加证据** | R5/R6 | 正向 | P1 |
| EV-009 | **update_evidence_manifest(12)：修改待处理证据** | R5/R6 | 正向 | P1 |
| EV-010 | **request_access(13)：请求访问** | R20 | 正向 | P1 |
| EV-011 | **update_access_policy(14)：更新访问策略** | R20 | 正向 | P2 |
| EV-012 | **reveal_commitment(15)：揭示承诺** | R5/R6 | 正向 | P1 |
| EV-013 | **seal_evidence(16) / unseal_evidence(17)：封存/解封** | R5/R6 | 正向 | P1 |
| EV-014 | **force_remove_evidence(18)：Root 强制删除** | R1 | 正向 | P1 |
| EV-015 | **withdraw_evidence(19)：撤回证据** | R5/R6 | 正向 | P1 |
| EV-016 | **delete_private_content(20)：删除私密内容** | R20 | 正向 | P2 |
| EV-017 | **force_archive_evidence(21)：Root 强制归档** | R1 | 正向 | P2 |
| EV-018 | **revoke_public_key(22)：撤销公钥** | R20 | 正向 | P2 |
| EV-019 | **cancel_access_request(23)：取消访问请求** | R20 | 正向 | P2 |
| EV-020 | 超过 MaxImg/MaxVid/MaxDoc 被拒绝 | R5/R6 | 负向 | P1 |
| EV-021 | 封存后无法修改/追加 | R5/R6 | 安全 | P1 |

详细测试续表见: [NEXUS_TEST_PLAN_PART3.md](./NEXUS_TEST_PLAN_PART3.md)
