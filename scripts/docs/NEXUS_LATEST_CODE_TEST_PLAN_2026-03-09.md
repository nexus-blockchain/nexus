# Nexus 最新链端详细测试计划（2026-03-09）

> 目的：在现有 `E2E_TEST_BOT_ANALYSIS.md` 与 `NEXUS_TEST_PLAN*.md` 基础上，按最新链端代码重新校准测试范围、优先级与落地顺序。
> 代码基线：`runtime/src/lib.rs`、`pallets/**/src/lib.rs`、`scripts/e2e/**`
> 规划原则：先补接口面完整性，再补关键业务链路，最后补治理/清理/批处理/边界场景。

---

## 1. 基线结论

- Runtime 当前已接入 54 个 pallet，其中业务相关的可调用 pallet 为 34 个。
- 本地链端代码共识别到 741 个 `#[pallet::call_index]` 对应的 extrinsic。
- 现有 `NEXUS_TEST_PLAN*.md` 已覆盖大部分主流程，但按最新代码仍有 22 个 pallet、158 个 extrinsic 未在计划中显式出现。
- 现有 E2E 流程共 23 条，适合作为“关键业务链路回归层”，但不适合单独承担“全接口覆盖层”。
- 现有 `cargo-runner.ts` 只纳入 29 个 crate，尚未覆盖部分已经有 `tests.rs` 的新增/可测 pallet。

---

## 2. 当前测试体系的主要缺口

### 2.1 文档层缺口

1. `E2E_TEST_BOT_ANALYSIS.md` 的 pallet 覆盖矩阵仍按旧接口口径描述，已不能代表最新链端真实调用面。
2. `NEXUS_TEST_PLAN*.md` 对主业务流程覆盖较强，但对以下类型覆盖明显不足：
   - Root/Force 类治理接口
   - pause/resume/global pause 类开关
   - cleanup/archive/batch 类维护接口
   - 多签名/委托/申诉/复核等二级状态机接口
   - approve/transfer_from/withdraw_user_funding 等资产控制接口

### 2.2 自动化层缺口

1. `scripts/e2e/core/cargo-runner.ts` 尚未纳入以下已有测试价值的 crate：
   - `pallet-ads-core`
   - `pallet-ads-entity`
   - `pallet-ads-router`
   - `pallet-commission-multi-level`
   - `pallet-commission-pool-reward`
   - `pallet-entity-common`
   - `pallet-storage-lifecycle`
2. 现有 E2E Flow 更偏“演示主链路”，缺少“接口契约型 E2E”：
   - 一个 Flow 内覆盖多个 extrinsic，但缺少逐接口状态前后断言
   - 对失败路径、权限路径、事件路径、存储路径覆盖不均匀
3. Coverage Map 需要从“按 Flow 映射 case id”升级为“双层覆盖”：
   - `flow_coverage`：关键用户旅程
   - `interface_coverage`：extrinsic / runtime API / OCW 入口

---

## 3. 目标测试分层

建议把后续测试拆成 5 层，分别建设，不再让一层承担所有目标。

| 层级 | 目标 | 主要载体 | 通过标准 |
|------|------|----------|----------|
| L0 | 编译/元数据/RPC 冒烟 | `cargo check`、metadata diff、基础 RPC | 节点可启动，metadata 可解析，关键 RPC 可用 |
| L1 | Pallet 单测/状态机测试 | Rust `tests.rs` | 每个 pallet 的核心状态机和错误分支可本地稳定回归 |
| L2 | 接口契约测试 | 新增 TS/Rust interface spec tests | 每个 extrinsic 至少有成功/权限/状态/事件 4 类断言 |
| L3 | 跨 pallet E2E | `scripts/e2e/flows/**` | 关键业务链路 P0/P1 全量打通 |
| L4 | 非功能与韧性 | 压测、批处理、清理、暂停恢复、超时/OCW | 大批量、长周期、异常恢复可验证 |

---

## 4. 每个 extrinsic 的标准测试模板

从现在开始，未覆盖 extrinsic 建议统一按下列模板落用例，不再只写单一 happy path：

1. 正向：参数合法、角色合法、状态合法，验证事件 + storage 变化 + 余额变化
2. 权限：错误角色调用，验证错误码
3. 状态：前置状态不满足时调用，验证错误码
4. 幂等/重复：重复执行、重复提交、重复取消、重复清理
5. 边界：最小值、最大值、空列表、单元素、上限值
6. 联动：对关联 pallet 的影响（资金、索引、统计、生命周期）

P0/P1 接口必须至少覆盖前 4 类；P2/P3 接口可覆盖前 3 类。

---

## 5. 按模块补测清单（基于最新代码差量）

以下是现有计划外、但最新代码已存在的重点补测面。建议将其作为 2026-03-09 之后的新一轮测试 backlog。

### 5.1 Trading

#### `pallet-nex-market`

补充接口：
- `seller_confirm_received`
- `ban_user` / `unban_user`
- `submit_counter_evidence`
- `update_order_amount`
- `batch_force_settle`
- `batch_force_cancel`

重点场景：
- 争议后补充证据与二次裁决路径
- 用户被 ban 后不可下单/吃单/申诉
- 批量强制结算/取消时的原子性与部分失败行为
- 修改订单金额后 TWAP、保证金、可成交量一致性

#### `pallet-entity-market`

补充接口：
- `configure_market`
- `market_buy` / `market_sell`
- `batch_cancel_orders`
- `cleanup_expired_orders`
- `modify_order`
- `set_kyc_requirement`
- `cancel_all_entity_orders`
- `governance_configure_market`
- `governance_configure_price_protection`
- `force_lift_circuit_breaker`

重点场景：
- 治理模式与 owner 模式配置权限切换
- IOC/FOK/PostOnly 与普通限价单之间撮合边界
- KYC 要求变更后存量订单与新增订单行为
- 熔断后的恢复路径和全量撤单路径

### 5.2 Entity 核心业务

#### `pallet-entity-order`

补充接口：
- `confirm_service`
- `cleanup_buyer_orders`
- `cleanup_shop_orders`
- `seller_refund_order`
- `force_partial_refund`
- `withdraw_dispute`
- `force_process_expirations`

重点场景：
- 服务型订单从 `start_service -> complete_service -> confirm_service`
- 争议撤回后订单状态、托管状态、佣金状态回滚
- 强制部分退款与普通退款的资金对账
- 过期订单批处理时索引清理与库存恢复

#### `pallet-entity-product`

补充接口：
- `force_delete_product`

重点场景：
- Root 强删已上架/已下架/已关联历史订单商品
- 强删后押金、索引、店铺统计的影响

#### `pallet-entity-token`

补充接口：
- `force_cancel_pending_dividends`
- `approve_tokens`
- `transfer_from`

重点场景：
- allowance 模型完整验证
- 黑名单/冻结/全局暂停下 `approve/transfer_from` 的一致拦截
- 待分红取消后快照、未领取记录、资金池对账

#### `pallet-entity-governance`

补充接口：
- `finalize_voting`
- `cleanup_proposal`
- `veto_proposal`
- `change_vote`
- `pause_governance` / `resume_governance`
- `batch_cancel_proposals`
- `force_unlock_governance`

重点场景：
- 投票期间改票、委托票、撤销票的权重变化
- pause 后 create/vote/execute 的统一拒绝
- cleanup 对历史 proposal 与索引的一致清理

#### `pallet-entity-member`

补充接口：
- `init_level_system`
- `manual_set_member_level`
- `set_use_custom_levels`
- `set_upgrade_mode`
- `update_upgrade_rule`
- `remove_upgrade_rule`
- `set_upgrade_rule_system_enabled`
- `set_conflict_strategy`
- `cleanup_expired_pending`
- `batch_approve_members`
- `batch_reject_members`

重点场景：
- 自动升级与手动设级冲突规则
- 审批流批处理的部分成功/失败
- pending 会员过期清理与重复申请

#### `pallet-entity-disclosure`

补充接口：
- `create_draft_disclosure`
- `publish_draft`
- `start_blackout` / `end_blackout` / `expire_blackout`
- `update_announcement`
- `withdraw_announcement`
- `pin_announcement` / `unpin_announcement`
- `expire_announcement`
- `cleanup_disclosure_history`
- `cleanup_announcement_history`
- `cleanup_entity_disclosure`
- `batch_add_insiders` / `batch_remove_insiders`
- `reset_violation_count`
- `configure_approval_requirements`
- `approve_disclosure` / `reject_disclosure`
- `publish_emergency_disclosure`
- `report_insider_transaction`
- `configure_fiscal_year`
- `escalate_penalty` / `reset_penalty`
- `set_disclosure_metadata`
- `audit_disclosure`

重点场景：
- Draft -> PendingApproval -> Published -> Corrected -> Withdrawn 完整状态机
- 审批制披露与紧急披露双路径
- 内幕人员批量导入/移除与黑窗期联动
- 违规升级、处罚升级、处罚清零的连续行为

#### `pallet-entity-kyc`

补充接口：
- `force_set_entity_requirement`
- `batch_revoke_by_provider`
- `entity_revoke_kyc`

重点场景：
- Provider 维度批量撤销后的参与资格回收
- Entity 自主撤销与 Root 撤销的权限边界
- requirement 被 force 覆盖后的旧申请兼容性

### 5.3 Commission

#### `pallet-commission-core`

补充接口：
- `force_enable_entity_commission`
- `retry_cancel_commission`
- `set_min_withdrawal_interval`

重点场景：
- cancel 失败后重试补偿
- 强制恢复后旧订单继续结算
- 提现最小时间间隔与 cooldown 的交互

#### 插件补测重点

`pallet-commission-referral`
- `force_set_*` / `force_clear_*`

`pallet-commission-multi-level`
- `force_set_multi_level_config`
- `force_clear_multi_level_config`
- `force_pause_multi_level`
- `force_resume_multi_level`
- `force_cleanup_entity`

`pallet-commission-level-diff`
- `clear_level_diff_config`
- `update_level_diff_config`
- `force_set_level_diff_config`
- `force_clear_level_diff_config`

`pallet-commission-single-line`
- `clear_single_line_config`
- `force_set_single_line_config`
- `force_clear_single_line_config`
- `force_reset_single_line`
- `force_remove_from_single_line`
- `force_restore_to_single_line`

`pallet-commission-team`
- `clear_team_performance_config`
- `update_team_performance_params`
- `force_set_team_performance_config`
- `force_clear_team_performance_config`
- `pause_team_performance`

`pallet-commission-pool-reward`
- `start_new_round`
- `force_set_pool_reward_config`
- `force_set_token_pool_enabled`
- `force_start_new_round`
- `clear_pool_reward_config`
- `force_clear_pool_reward_config`
- `pause_pool_reward`
- `force_pause_pool_reward`
- `force_resume_pool_reward`
- `schedule_pool_reward_config_change`
- `apply_pending_pool_reward_config`
- `cancel_pending_pool_reward_config`
- `correct_token_pool_deficit`

重点场景：
- 配置切换生效窗口
- Root 强制写入与业务方普通写入优先级
- pause/resume 后订单分佣是否一致冻结
- cleanup/clear 对存量待领取记录的影响

### 5.4 Dispute

#### `pallet-dispute-arbitration`

补充接口：
- `appeal`
- `resolve_appeal`

重点场景：
- 一审 -> 申诉 -> 二审 的完整状态机
- appeal 期间保证金、证据补充、执行锁的联动

#### `pallet-dispute-escrow`

补充接口：
- `apply_decision_refund_all`
- `apply_decision_partial_bps`

重点场景：
- 仲裁裁决后 escrow 资金拆分正确性
- 与 order/dispute 的联动结算

#### `pallet-dispute-evidence`

补充接口：
- `commit_v2`

重点场景：
- 兼容旧版 `commit`
- 新版提交后的 manifest、权限、访问控制一致性

### 5.5 Storage

#### `pallet-storage-service`

补充接口：
- `withdraw_user_funding`
- `downgrade_pin_tier`
- `dispute_slash`
- `fund_subject_account`

重点场景：
- 用户资金充值/提现/被扣费三账一致
- 升级/降级 tier 后计费切换
- slash 争议期的冻结、恢复、二次处罚
- subject 维度资金池与 owner 维度资金池隔离

#### `pallet-storage-lifecycle`

建议从“新增模块补测”提升为“固定 nightly 回归模块”：
- 归档策略变更
- pause/resume archival
- restore 与 purge protection 冲突判定
- Active/L1/L2/Purge 全路径长时间状态迁移

### 5.6 GroupRobot

#### `pallet-grouprobot-registry`

补充接口：
- `submit_dcap_dual_attestation`
- `submit_sgx_attestation`
- `unassign_bot_from_operator`
- `suspend_bot` / `reactivate_bot`
- `unbind_user_platform`
- `transfer_bot_ownership`
- `revoke_api_server_mrtd`
- `revoke_pck_key`
- `force_deactivate_bot`
- `cleanup_deactivated_bot`
- `operator_unassign_bot`
- `force_expire_attestation`
- `force_transfer_bot_ownership`

重点场景：
- Bot 生命周期从 active 到 suspend/deactivate/cleanup 的完整闭环
- operator 与 owner 双边解绑行为
- TEE 证明失效、过期、强制作废后的 gating

### 5.7 Ads

#### `pallet-ads-core`

补充接口：
- `advertiser_block_placement`
- `advertiser_unblock_placement`
- `advertiser_prefer_placement`
- `advertiser_unprefer_placement`
- `placement_block_advertiser`
- `placement_unblock_advertiser`
- `placement_prefer_advertiser`
- `placement_unprefer_advertiser`
- `clear_placement_flags`
- `approve_campaign_for_placement`
- `auto_confirm_receipt`

重点场景：
- 双向偏好矩阵：黑名单优先级高于白名单
- placement 审批制下投放前 gating
- receipt 未人工确认时 auto confirm 的超时路径

#### `pallet-ads-entity`

现有计划虽然覆盖主接口，但建议新增单独 E2E 流：
- Entity placement 注册
- Shop placement 注册
- Cap 命中
- ban/unban entity
- 与 `ads-core` Campaign 生命周期串联结算

---

## 6. 新增测试交付形式

### 6.1 Cargo 层

第一批直接纳入 `ALL_PALLETS`：

- `pallet-ads-core`
- `pallet-ads-entity`
- `pallet-ads-router`
- `pallet-commission-multi-level`
- `pallet-commission-pool-reward`
- `pallet-entity-common`
- `pallet-storage-lifecycle`

说明：
- `pallet-commission-team` 当前无 `tests.rs`，先不强制纳入 runner，可单独补测后再接入。
- `pallet-ads-primitives`、`pallet-grouprobot-primitives`、`pallet-trading-common`、`pallet-trading-trc20-verifier` 更适合编译/静态校验，不必强行作为独立 cargo test 目标。

### 6.2 E2E 层

建议新增 8 条接口型/治理型 Flow：

1. `Flow-E10: Entity Review`
2. `Flow-E11: Governance Admin/Ops`
3. `Flow-E12: Disclosure Approval`
4. `Flow-T5: NexMarket Dispute+Admin`
5. `Flow-S2: Storage Billing Dispute`
6. `Flow-G8: Bot Suspension+Transfer`
7. `Flow-A1: Ads Entity Placement`
8. `Flow-C1: Commission Force/Pause/Cleanup`

### 6.3 覆盖率层

Coverage 应从“Flow 覆盖多少 case id”改为“三张表”：

1. `extrinsic_inventory.json`
   - pallet
   - call_index
   - extrinsic
   - priority
   - owner
   - has_unit_test
   - has_interface_test
   - has_e2e_test
2. `flow_coverage.json`
3. `risk_regression_suite.json`

---

## 7. 执行顺序（建议 4 个波次）

### Wave 1：P0 接口补齐（本周）

目标：
- 补齐所有资金、权限、暂停、争议、治理强制接口
- 修正 cargo runner 覆盖面

范围：
- NexMarket 新增 7 个接口
- Entity Order / Token / Governance 的缺口接口
- StorageService 用户资金与 slash dispute
- AdsCore 双向偏好和 auto confirm
- Arbitration 的 `appeal` / `resolve_appeal`

退出标准：
- 新增接口全部进入 inventory
- P0/P1 接口至少具备 4 类测试模板
- nightly 可稳定跑通

### Wave 2：状态机补齐（下周）

目标：
- 补 Draft/Approval/Appeal/Cleanup/Expire 等中间状态

范围：
- Disclosure
- Member
- KYC
- StorageLifecycle
- GroupRobot Registry 生命周期

退出标准：
- 所有“多阶段状态机”均有状态转移图 + 对应用例

### Wave 3：批处理与治理补齐

目标：
- 覆盖 batch/cleanup/archive/force 全家桶

范围：
- Commission plugins
- Entity Market batch/cleanup
- Storage batch_unpin / cleanup / migrate
- GroupRobot operator/bot 管理接口

退出标准：
- 所有 batch/cleanup 接口至少有边界测试

### Wave 4：性能与长周期回归

目标：
- 压测、超时、长周期归档、Era/epoch 相关路径稳定

范围：
- Ads 结算
- Storage 计费
- NexMarket 超时/补付
- GroupRobot era 结算

退出标准：
- 形成 PR 套件 + nightly 套件 + weekly 长跑套件

---

## 8. CI 建议

### PR 套件（10-15 分钟）

- 编译/类型检查
- 关键 pallet cargo tests
- P0 interface tests
- 8-10 条核心 E2E flow

### Nightly 套件

- 全量 cargo tests
- 全量 interface tests
- 全量 E2E
- coverage diff

### Weekly 套件

- 长周期归档/超时
- 批处理上限
- 大规模订单/广告/Pin 压测

---

## 9. 退出标准

满足以下条件后，可认为“测试计划已对齐最新链端代码”：

1. 最新 runtime 中 34 个业务 pallet 全部进入测试 inventory
2. 741 个 extrinsic 全部被分类为：
   - 已覆盖
   - 计划中
   - 明确豁免
3. 158 个当前未显式入计划的 extrinsic 全部落到 backlog 并赋予优先级
4. Cargo runner 至少覆盖全部“有 tests.rs 的关键 crate”
5. P0/P1 接口具备成功/权限/状态/事件四类断言
6. 每个业务域至少有 1 条治理/异常恢复型 E2E 流

---

## 10. 建议的下一步落地动作

1. 先更新 `cargo-runner.ts` 的 `ALL_PALLETS`，让单测覆盖面与代码基线对齐
2. 生成 `extrinsic_inventory.json`，把 741 个接口变成可追踪清单
3. 先补 Wave 1 的 6 个高风险模块，再回写 `NEXUS_TEST_PLAN*.md`
4. 把 `E2E_TEST_BOT_ANALYSIS.md` 中过期的 pallet 覆盖矩阵改成自动生成

---

## 11. 本地 dev 链执行校准（2026-03-09 实测）

> 执行环境：本地 dev 链已启动，脚本侧通过 `node --import tsx` 直连 `ws://127.0.0.1:9944`。
> 说明：以下内容来自 2026-03-09 的真实链上回归，不是静态推断。

### 11.1 运行入口修正

- `tsx e2e/nexus-test-agent.ts` 在本机会因 `EPERM`/pipe 问题不稳定，建议统一改为 `node --import tsx ...`
- `agent:wave1` 必须显式加 `--mode e2e`，否则会先进入 Cargo Phase，掩盖链上回归结果

### 11.2 已确认的最新接口/权限变化

- `pallet-nex-market`
  - `place_sell_order` 已为 4 参数：`(nex_amount, usdt_price, tron_address, min_fill_amount)`
  - `configure_price_protection / set_initial_price / ban_user / unban_user / batch_force_*` 不是 Root，而是 `MarketAdminOrigin`
  - Runtime 当前把 `MarketAdminOrigin` 配成 `TreasuryCollective 2/3`，本地 dev 链默认 sudo 账户无法替代该 origin
  - TRON 地址校验比旧脚本更严格，需使用链端测试中可通过的地址样式

- `pallet-entity-product`
  - `create_product` 已扩展为 14 参数：
    `shop_id, name_cid, images_cid, detail_cid, price, usdt_price, stock, category, sort_weight, tags_cid, sku_cid, min_order_quantity, max_order_quantity, visibility`
  - 旧的 7 参数脚本会直接报参数数量错误

- `pallet-entity-registry`
  - E2E 里不要再假设实体详情结构体内必然内嵌 `shop_ids`
  - 本地脚本应优先查询 `entityRegistry.entityShops(entity_id)` 取店铺列表

- `pallet-entity-governance`
  - Runtime 最小投票期已是 `14,400` blocks
  - Runtime 最小执行延迟已是 `2,400` blocks
  - `ProposalCooldown` 为 `1 * DAYS`，同一 proposer 在同一实体上连续建提案会命中冷却
  - 因此本地短回归不能再把 `finalize_voting` 作为短周期 happy path

- `pallet-dispute-arbitration`
  - Runtime 的路由器当前只真正开放 `entorder`
  - `DecisionOrigin` 已配置为 `ArbitrationCollective 2/3`
  - 本地 dev 链默认 sudo 无法替代仲裁委员会裁决 origin

- `pallet-ads-grouprobot` / `pallet-ads-core`
  - `set_community_admin` 仅当前管理员 / Bot Owner 可调用；Root 需要走 `force_set_community_admin`
  - `force_register_advertiser` 在账户已注册时会失败，但这不影响后续已注册广告主复用
  - `submit_delivery_receipt` 的 verifier 要求提交者具备 TEE 节点侧条件，单纯 community admin 不够

- `pallet-storage-service`
  - `request_pin_for_subject` 已是 4 参数：`(subject_id, cid, size_bytes, tier)`
  - `join_operator` 的 bond 约束高于旧脚本示例，低保证金会触发 `InsufficientBond`

### 11.3 对 Wave 1 计划的直接影响

- `T5`
  - 本地 dev 链应拆成两层：
    1. 普通用户/订单所有者路径（可直接回归）
    2. `MarketAdminOrigin` 权限边界校验（Root 应失败）
  - 真正的 admin happy path 需要额外注入 TreasuryCollective 成员私钥或专用本地链配置

- `E10`
  - 需要先补“最小实体/店铺上下文自举”
  - 同时必须按 14 参数重写商品创建调用

- `E11`
  - 需要改成“短周期可完成的治理维护流”：
    - 改票
    - 提前 finalize 失败
    - pause/resume
    - batch cancel
    - veto
    - force unlock
  - 多提案应换 proposer，避免 1 天冷却

- `D3`
  - 本地 dev 链可验证：
    - entity order 投诉
    - respond/escalate
    - Root 无法替代 ArbitrationCollective
  - 真正的 `resolve -> appeal -> resolve_appeal` 正向 happy path 需要委员会成员密钥

- `A1`
  - 偏好/黑白名单/placement approval 可在本地直接回归
  - delivery receipt 正向路径需要额外 TEE 节点前置，建议拆到 GroupRobot/Node 组合场景

- `S2`
  - 已确认接口签名和 bond 约束发生变化
  - 下一轮应重点验证 `join_operator -> request_pin_for_subject -> downgrade_pin_tier -> dispute_slash`

### 11.4 建议新增的环境标签

建议把 Wave 1 测试再按执行环境打标签，避免把“接口已对齐”和“本地默认链可正向打通”混为一谈：

- `local-dev-pass`
  - 使用默认 dev 链 + sudo + dev keyring 应可直接通过

- `local-dev-permission-boundary`
  - 在默认 dev 链上只能验证权限边界，无法做 admin happy path

- `committee-env-required`
  - 需要实际 collective/committee 成员密钥

- `tee-env-required`
  - 需要 TEE / node consensus / ads delivery verifier 前置
