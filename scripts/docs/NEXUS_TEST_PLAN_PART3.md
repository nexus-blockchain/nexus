# Nexus 测试计划 — Part 3

> 续 [NEXUS_TEST_PLAN_PART2.md](./NEXUS_TEST_PLAN_PART2.md)
> 更新日期: 2026-03-06

---

## 16. Arbitration — 仲裁模块

> Pallet: `pallet-dispute-arbitration` | Extrinsics: 31 (call_index 0-5, 10-15, 20-30)
> 新增: request_default_judgment, supplement_complaint/response_evidence,
> settle_dispute, start_mediation, dismiss_dispute/complaint, set_paused,
> force_close_dispute/complaint, set_domain_penalty_rate

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AR-001 | dispute(0)：发起仲裁 | R5/R6 | 正向 | P0 |
| AR-002 | arbitrate(1)：仲裁裁决 | R1/R11 | 正向 | P0 |
| AR-003 | dispute_with_evidence_id(2)：带证据发起 | R5/R6 | 正向 | P1 |
| AR-004 | append_evidence_id(3)：补充证据（仅争议双方） | R5/R6 | 正向 | P1 |
| AR-005 | dispute_with_two_way_deposit(4)：双方押金仲裁 | R5 | 正向 | P1 |
| AR-006 | respond_to_dispute(5)：应诉方响应 | R6 | 正向 | P1 |
| AR-007 | file_complaint(10)：发起投诉（缴押金） | R5/R6 | 正向 | P0 |
| AR-008 | respond_to_complaint(11)：响应投诉 | R5/R6 | 正向 | P0 |
| AR-009 | withdraw_complaint(12)：撤销投诉+退押金 | R5/R6 | 正向 | P1 |
| AR-010 | settle_complaint(13)：和解+双方押金退还 | R5/R6 | 正向 | P1 |
| AR-011 | escalate_to_arbitration(14)：升级到仲裁 | R5/R6 | 正向 | P0 |
| AR-012 | resolve_complaint(15)：裁决投诉 | R1/R11 | 正向 | P0 |
| AR-013 | **request_default_judgment(20)：缺席裁决（对方未响应）** | R5/R6 | 正向 | P1 |
| AR-014 | **supplement_complaint_evidence(21)：补充投诉证据** | R5/R6 | 正向 | P1 |
| AR-015 | **supplement_response_evidence(22)：补充应诉证据** | R5/R6 | 正向 | P1 |
| AR-016 | **settle_dispute(23)：和解争议** | R5/R6 | 正向 | P1 |
| AR-017 | **start_mediation(24)：开始调解（DecisionOrigin）** | R1/R11 | 正向 | P1 |
| AR-018 | **dismiss_dispute(25)：驳回争议** | R1/R11 | 正向 | P1 |
| AR-019 | **dismiss_complaint(26)：驳回投诉** | R1/R11 | 正向 | P1 |
| AR-020 | **set_paused(27)：全局暂停** | R1/R11 | 正向 | P1 |
| AR-021 | **force_close_dispute(28) / force_close_complaint(29)：强制关闭** | R1/R11 | 正向 | P1 |
| AR-022 | **set_domain_penalty_rate(30)：设置域惩罚率** | R1/R11 | 正向 | P2 |
| AR-023 | 非争议方 append_evidence 被拒绝 | R20 | 权限 | P1 |
| AR-024 | 非授权无法裁决 | R20 | 权限 | P0 |
| AR-025 | 暂停时所有操作被拒绝 | R5/R6 | 安全 | P1 |
| AR-026 | 押金退还：Release→发起方 30% slash / Refund→应诉方 30% slash / Partial→双方 50% | 系统 | 功能 | P0 |

## 17. Storage Service — 存储服务

> Pallet: `pallet-storage-service` | Extrinsics: 35+ (call_index 1-51)
> 新增大量: renew_pin, upgrade_pin_tier, batch_unpin, top_up_bond, reduce_bond,
> cleanup_expired_locks, request_unpin, cleanup_expired_cids, fund_ipfs_pool,
> OCW 操作(40-43), 域名管理(25-27,33), migrate_operator_pins

### 17.1 用户操作

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-001 | fund_user_account(21)：充值用户账户 | R20 | 正向 | P0 |
| SS-002 | request_pin_for_subject(10)：请求 Pin（含 tier） | R20 | 正向 | P0 |
| SS-003 | **request_unpin(32)：请求取消 Pin** | R20 | 正向 | P0 |
| SS-004 | **renew_pin(45)：续期 Pin（1-52 periods）** | R20 | 正向 | P1 |
| SS-005 | **upgrade_pin_tier(46)：升级 Pin 层级** | R20 | 正向 | P1 |
| SS-006 | **batch_unpin(48)：批量取消 Pin（1-20 CIDs）** | R20 | 正向 | P1 |
| SS-007 | **cleanup_expired_cids(34)：清理过期 CID（limit≤50）** | R20 | 正向 | P2 |
| SS-008 | **cleanup_expired_locks(51)：清理过期 CID 锁** | R20 | 正向 | P2 |
| SS-009 | **fund_ipfs_pool(44)：充值 IPFS 池** | R20 | 正向 | P1 |
| SS-010 | 余额不足 Pin 被拒绝 | R20 | 负向 | P0 |
| SS-011 | 已 Pin 的 CID 重复请求被拒绝（CidAlreadyPinned） | R20 | 负向 | P1 |

### 17.2 运营者管理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-012 | join_operator(3)：加入（peer_id, capacity, endpoint, bond） | R14 | 正向 | P0 |
| SS-013 | 容量 < MinCapacityGiB / 保证金不足被拒绝 | R14 | 负向 | P1 |
| SS-014 | update_operator(4)：更新运营者信息 | R14 | 正向 | P1 |
| SS-015 | leave_operator(5)：退出（有 active pins→grace period） | R14 | 正向 | P1 |
| SS-016 | report_probe(7)：自证在线 | R14 | 正向 | P1 |
| SS-017 | operator_claim_rewards(16)：领取奖励 | R14 | 正向 | P1 |
| SS-018 | pause_operator(22) / resume_operator(23)：暂停/恢复 | R14 | 正向 | P1 |
| SS-019 | **top_up_bond(49)：追加保证金** | R14 | 正向 | P1 |
| SS-020 | **reduce_bond(50)：减少保证金** | R14 | 正向 | P1 |
| SS-021 | 减少后低于最低要求被拒绝 | R14 | 负向 | P1 |
| SS-022 | 已暂停重复暂停 / 未暂停恢复被拒绝 | R14 | 负向 | P1 |

### 17.3 OCW / Pin 报告

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-023 | mark_pinned(1)：标记 Pin 成功 | R14/R19 | 正向 | P0 |
| SS-024 | mark_pin_failed(2)：标记失败 | R14/R19 | 正向 | P1 |
| SS-025 | **ocw_mark_pinned(40) / ocw_mark_pin_failed(41)：OCW 报告** | R19 | 正向 | P0 |
| SS-026 | **ocw_submit_assignments(42)：OCW 提交分配** | R19 | 正向 | P1 |
| SS-027 | **ocw_report_health(43)：OCW 报告健康** | R19 | 正向 | P1 |

### 17.4 治理操作

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-028 | set_operator_status(6)：设置运营者状态（Active/Suspended/Banned） | R1 | 正向 | P1 |
| SS-029 | slash_operator(8)：扣罚保证金 | R1 | 正向 | P1 |
| SS-030 | charge_due(11)：处理到期扣费 | R1 | 正向 | P0 |
| SS-031 | set_billing_params(12)：设置计费参数 | R1 | 正向 | P1 |
| SS-032 | distribute_to_operators(13)：分配收益 | R1 | 正向 | P1 |
| SS-033 | set_replicas_config(14)：副本配置（1-10） | R1 | 正向 | P2 |
| SS-034 | update_tier_config(15)：层级配置 | R1 | 正向 | P2 |
| SS-035 | emergency_pause_billing(17) / resume_billing(18)：紧急暂停/恢复 | R1 | 安全 | P1 |
| SS-036 | set_storage_layer_config(19) / set_operator_layer(20)：分层存储 | R1 | 正向 | P2 |
| SS-037 | **register_domain(25) / update_domain_config(26) / set_domain_priority(27)：域管理** | R1 | 正向 | P1 |
| SS-038 | **governance_force_unpin(33)：强制取消 Pin** | R1 | 正向 | P1 |
| SS-039 | **migrate_operator_pins(47)：迁移运营者 Pin** | R1 | 正向 | P2 |
| SS-040 | 余额不足→宽限→过期→清理完整链路 | 系统 | 流程 | P1 |
| SS-041 | Pin 状态机：Requested→Pinning→Pinned/Degraded/Failed | 系统 | 功能 | P0 |

## 18. Storage Lifecycle — 存储生命周期（新增模块）

> Pallet: `pallet-storage-lifecycle` | Extrinsics: 9 (call_index 0-8)
> 全新 Pallet，管理数据归档 Active→L1→L2→Purge

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SL-001 | set_archive_config(0)：设置全局归档配置 | R1 | 正向 | P1 |
| SL-002 | pause_archival(1) / resume_archival(2)：暂停/恢复归档 | R1 | 正向 | P1 |
| SL-003 | set_archive_policy(3)：设置按类型归档策略 | R1 | 正向 | P1 |
| SL-004 | force_archive(4)：强制归档指定数据 | R1 | 正向 | P1 |
| SL-005 | protect_from_purge(5) / remove_purge_protection(6)：清除保护 | R1 | 正向 | P1 |
| SL-006 | extend_active_period(7)：延长活跃期（≥100 blocks） | R1 | 正向 | P1 |
| SL-007 | restore_from_archive(8)：从 L1 恢复到 Active | R1 | 正向 | P1 |
| SL-008 | 延长不足 100 blocks 被拒绝（ExtensionTooShort） | R1 | 负向 | P1 |
| SL-009 | L2 级别不可直接恢复（CannotRestoreFromLevel） | R1 | 负向 | P1 |
| SL-010 | on_idle 自动归档管道：Active→L1→L2→Purge | 系统 | 功能 | P0 |
| SL-011 | 受保护数据跳过 Purge | 系统 | 功能 | P1 |
| SL-012 | 延长活跃期的数据跳过 L1 归档 | 系统 | 功能 | P1 |

## 19. GroupRobot Registry — Bot 注册管理

> Pallet: `pallet-grouprobot-registry` | Extrinsics: 40+
> 新增: revoke_mrtd/mrenclave, update_peer_endpoint, suspend/unsuspend_operator,
> bot_ownership_transfer, force operations, clean_bot, etc.

### 19.1 Bot 生命周期

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-001 | register_bot(0)：注册 Bot | R12 | 正向 | P0 |
| GR-002 | update_public_key(1)：更换公钥 | R12 | 正向 | P1 |
| GR-003 | deactivate_bot(2)：停用 Bot（清理关联） | R12 | 正向 | P1 |
| GR-004 | bind_community(3) / unbind_community(4)：绑定/解绑社区 | R12 | 正向 | P0 |
| GR-005 | bind_user_platform(5)：绑定平台身份 | R20 | 正向 | P1 |

### 19.2 TEE 证明

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-006 | submit_attestation / refresh_attestation：软件模式 | R12 | 正向 | P0 |
| GR-007 | approve_mrtd(8) / approve_mrenclave(9)：白名单 | R1 | 正向 | P0 |
| GR-008 | **revoke_mrtd / revoke_mrenclave：撤销白名单** | R1 | 正向 | P1 |
| GR-009 | request_attestation_nonce(11)：请求 nonce | R12 | 正向 | P0 |
| GR-010 | submit_verified_attestation / submit_dcap_attestation / submit_dcap_dual | R12 | 正向 | P0 |
| GR-011 | submit_dcap_full_attestation / submit_sgx / submit_tee_attestation | R12 | 正向 | P1 |
| GR-012 | 篡改 MRTD / report_data / Quote → 拒绝 | R12 | 安全 | P0 |
| GR-013 | approve_api_server_mrtd / register_pck_key | R1 | 正向 | P1 |

### 19.3 Peer 网络

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-014 | register_peer：注册（需 TEE + 付费订阅） | R12 | 正向 | P0 |
| GR-015 | Free tier 被拒绝 | R12 | 安全 | P0 |
| GR-016 | deregister_peer：注销 | R12 | 正向 | P1 |
| GR-017 | heartbeat_peer：心跳 | R12 | 正向 | P0 |
| GR-018 | report_stale_peer：举报过期 | R20 | 正向 | P1 |
| GR-019 | **update_peer_endpoint：更新 Peer 端点** | R12 | 正向 | P1 |

### 19.4 运营商管理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-020 | register_operator / update_operator / deregister_operator | R18 | 正向 | P1 |
| GR-021 | set_operator_sla：设置 SLA（Root） | R1 | 正向 | P2 |
| GR-022 | assign_bot_to_operator / unassign | R12 | 正向 | P1 |
| GR-023 | **suspend_operator / unsuspend_operator** | R1 | 正向 | P1 |
| GR-024 | **bot_ownership_transfer：转移 Bot 所有权** | R12 | 正向 | P1 |
| GR-025 | 同平台 app_hash 重复被拒绝 | R18 | 负向 | P1 |

## 20. GroupRobot Consensus — 节点共识

> Pallet: `pallet-grouprobot-consensus` | Extrinsics: 25 (call_index 0-24)
> 新增: increase_stake, reinstate_node, force_suspend/remove_node, unbind_bot,
> replace_operator, set_slash_percentage, set_reporter_reward_pct, force_reinstate_node,
> batch_cleanup_equivocations, force_era_end

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CN-001 | register_node(0)：注册+质押 | R13 | 正向 | P0 |
| CN-002 | 质押不足被拒绝 | R13 | 负向 | P0 |
| CN-003 | request_exit(1) → finalize_exit(2)：退出流程 | R13 | 正向 | P1 |
| CN-004 | report_equivocation(3) / slash_equivocation(4)：举报+Slash | R13/R1 | 正向 | P1 |
| CN-005 | mark_sequence_processed(10)：序列去重 | R13 | 正向 | P0 |
| CN-006 | verify_node_tee(11)：验证 TEE（需 Active） | R13 | 正向 | P1 |
| CN-007 | set_tee_reward_params(12)：TEE 奖励参数 | R1 | 正向 | P2 |
| CN-008 | cleanup_resolved_equivocation(13)：清理 | R20 | 正向 | P2 |
| CN-009 | **increase_stake(14)：增加质押** | R13 | 正向 | P1 |
| CN-010 | **reinstate_node(15)：恢复被暂停节点** | R13 | 正向 | P1 |
| CN-011 | **force_suspend_node(16) / force_remove_node(17)：Root 强制** | R1 | 正向 | P1 |
| CN-012 | **unbind_bot(18)：解绑 Bot** | R13 | 正向 | P1 |
| CN-013 | **replace_operator(19)：替换运营者** | R13 | 正向 | P1 |
| CN-014 | **set_slash_percentage(20) / set_reporter_reward_pct(21)** | R1 | 正向 | P2 |
| CN-015 | **force_reinstate_node(22)：Root 强制恢复** | R1 | 正向 | P1 |
| CN-016 | **batch_cleanup_equivocations(23)：批量清理** | R20 | 正向 | P2 |
| CN-017 | **force_era_end(24)：强制结束 Era** | R1 | 正向 | P1 |
| CN-018 | Free tier 标记被拒绝 | R13 | 安全 | P0 |
| CN-019 | 非活跃节点操作被拒绝 | R13 | 负向 | P1 |
| CN-020 | on_era_end 结算+uptime+pruning | 系统 | 功能 | P1 |

## 21. GroupRobot Subscription — 订阅服务

> Pallet: `pallet-grouprobot-subscription` | Extrinsics: 21 (call_index 0-20)
> 新增: cleanup_ad_commitment, update_tier_feature_gate, force_cancel_subscription,
> withdraw_escrow, update_ad_commitment, force_suspend_subscription,
> operator_deposit, reset_tier_feature_gate, force_change_tier,
> pause/resume_subscription, batch_cleanup, update_tier_fee, force_cancel_ad_commitment

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SB-001 | subscribe(0)：订阅（Basic/Pro/Enterprise） | R17 | 正向 | P0 |
| SB-002 | deposit_subscription(1)：充值续期 | R17 | 正向 | P1 |
| SB-003 | cancel_subscription(2)：取消（退 escrow） | R17 | 正向 | P1 |
| SB-004 | change_tier(3)：变更层级（升级需 escrow 充足） | R17 | 正向 | P1 |
| SB-005 | commit_ads(4) / cancel_ad_commitment(5) | R17 | 正向 | P1 |
| SB-006 | cleanup_subscription(6) / **cleanup_ad_commitment(7)** | R20 | 正向 | P2 |
| SB-007 | **update_tier_feature_gate(8)：更新层级功能门** | R1 | 正向 | P2 |
| SB-008 | **force_cancel_subscription(9)：Root 强制取消** | R1 | 正向 | P1 |
| SB-009 | **withdraw_escrow(10)：提取 escrow** | R17 | 正向 | P1 |
| SB-010 | **update_ad_commitment(11)：更新广告承诺** | R17 | 正向 | P1 |
| SB-011 | **force_suspend_subscription(12)：Root 暂停** | R1 | 正向 | P1 |
| SB-012 | **operator_deposit_subscription(13)：运营商代充** | R18 | 正向 | P1 |
| SB-013 | **reset_tier_feature_gate(14)：重置功能门** | R1 | 正向 | P2 |
| SB-014 | **force_change_tier(15)：Root 强制变更** | R1 | 正向 | P1 |
| SB-015 | **pause_subscription(16) / resume_subscription(17)：暂停/恢复** | R17 | 正向 | P1 |
| SB-016 | **batch_cleanup(18)：批量清理** | R20 | 正向 | P2 |
| SB-017 | **update_tier_fee(19)：更新层级费用** | R1 | 正向 | P1 |
| SB-018 | **force_cancel_ad_commitment(20)：Root 强制取消承诺** | R1 | 正向 | P1 |
| SB-019 | 升级 escrow 不足被拒绝 | R17 | 负向 | P1 |
| SB-020 | 过期订阅自动降级为 Free | 系统 | 功能 | P1 |
| SB-021 | 提取导致资金不足被拒绝（WithdrawWouldUnderfund） | R17 | 负向 | P1 |

## 22. GroupRobot Community — 社区管理

> Pallet: `pallet-grouprobot-community` | Extrinsics: 16 (call_index 0-15)
> 新增: delete_community_config, force_remove_community, ban/unban_community,
> force_update_community_config, force_reset_community_reputation

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GC-001 | submit_action_log(0)：提交日志（Ed25519 签名，需 active Bot） | R12 | 正向 | P0 |
| GC-002 | set_node_requirement(1)：节点准入策略 | R15 | 正向 | P1 |
| GC-003 | update_community_config(2)：CAS 乐观锁更新 | R15 | 正向 | P1 |
| GC-004 | batch_submit_logs(3)：批量提交 | R12 | 正向 | P1 |
| GC-005 | clear_expired_logs(4)：清理过期日志 | R20 | 正向 | P1 |
| GC-006 | award_reputation(5) / deduct_reputation(6) / reset_reputation(7) | R12 | 正向 | P1 |
| GC-007 | update_active_members(8)：更新活跃数 | R12 | 正向 | P1 |
| GC-008 | cleanup_expired_cooldowns(9)：清理冷却 | R20 | 正向 | P2 |
| GC-009 | **delete_community_config(10)：删除配置** | R15 | 正向 | P1 |
| GC-010 | **force_remove_community(11)：Root 强制删除** | R1 | 正向 | P1 |
| GC-011 | **ban_community(12) / unban_community(13)：Root 封禁/解禁** | R1 | 正向 | P1 |
| GC-012 | **force_update_community_config(14)：Root 强制更新** | R1 | 正向 | P1 |
| GC-013 | **force_reset_community_reputation(15)：Root 重置声誉** | R1 | 正向 | P1 |
| GC-014 | Free tier / 停用 Bot / 签名无效被拒绝 | R12 | 安全 | P0 |
| GC-015 | 版本冲突（CAS）被拒绝 | R15 | 负向 | P1 |
| GC-016 | 已封禁社区操作被拒绝 | R12 | 负向 | P1 |

## 23. GroupRobot Ceremony — 密钥仪式

> Pallet: `pallet-grouprobot-ceremony` | Extrinsics: 11 (call_index 0-10)
> 新增: cleanup_ceremony, owner_revoke_ceremony, revoke_by_mrenclave,
> trigger_expiry, batch_cleanup_ceremonies, renew_ceremony

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CE-001 | record_ceremony(0)：记录仪式（Shamir k,n + Enclave 白名单） | R12 | 正向 | P0 |
| CE-002 | revoke_ceremony(1)：Root 撤销 | R1 | 正向 | P1 |
| CE-003 | approve_ceremony_enclave(2) / remove_ceremony_enclave(3)：白名单 | R1 | 正向 | P1 |
| CE-004 | force_re_ceremony(4)：强制重仪式 | R1 | 正向 | P1 |
| CE-005 | **cleanup_ceremony(5)：清理终态仪式** | R20 | 正向 | P2 |
| CE-006 | **owner_revoke_ceremony(6)：Owner 主动撤销** | R12 | 正向 | P1 |
| CE-007 | **revoke_by_mrenclave(7)：按 Enclave 撤销全部** | R1 | 正向 | P1 |
| CE-008 | **trigger_expiry(8)：手动触发过期** | R20 | 正向 | P2 |
| CE-009 | **batch_cleanup_ceremonies(9)：批量清理** | R20 | 正向 | P2 |
| CE-010 | **renew_ceremony(10)：续期仪式** | R12 | 正向 | P1 |
| CE-011 | participant_count < k / > n 被拒绝 | R12 | 安全 | P0 |
| CE-012 | Free tier / 停用 Bot 被拒绝 | R12 | 安全 | P0 |
| CE-013 | 非终态仪式 cleanup 被拒绝 | R20 | 负向 | P2 |
| CE-014 | CeremonyAtRisk 检测（on_initialize） | 系统 | 功能 | P1 |

## 24. GroupRobot Rewards — 节点奖励

> Pallet: `pallet-grouprobot-rewards` | Extrinsics: 11 (call_index 0-10)
> 新增: batch_claim_rewards, set_reward_recipient, force_slash_pending_rewards,
> set_reward_split, claim_owner_rewards, pause/resume_distribution,
> force_set_pending_rewards, force_prune_era_rewards

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| RW-001 | claim_rewards(0)：领取奖励 | R13 | 正向 | P0 |
| RW-002 | rescue_stranded_rewards(1)：Root 救援 | R1 | 正向 | P1 |
| RW-003 | **batch_claim_rewards(2)：批量领取** | R13 | 正向 | P1 |
| RW-004 | **set_reward_recipient(3)：设置奖励接收人** | R13 | 正向 | P1 |
| RW-005 | **force_slash_pending_rewards(4)：Root 扣除** | R1 | 正向 | P1 |
| RW-006 | **set_reward_split(5)：设置 Owner/Operator 分成** | R12 | 正向 | P1 |
| RW-007 | **claim_owner_rewards(6)：Owner 领取分成** | R12 | 正向 | P1 |
| RW-008 | **pause_distribution(7) / resume_distribution(8)** | R1 | 正向 | P1 |
| RW-009 | **force_set_pending_rewards(9)：Root 设置** | R1 | 正向 | P2 |
| RW-010 | **force_prune_era_rewards(10)：Root 清理** | R1 | 正向 | P2 |
| RW-011 | 无可领奖励 / 奖励池不足被拒绝 | R13 | 负向 | P1 |
| RW-012 | TEE 节点奖励倍数正确 | 系统 | 功能 | P1 |
| RW-013 | 分发暂停时领取被拒绝 | R13 | 安全 | P1 |

## 25. Ads Core — 广告核心引擎（新增模块）

> Pallet: `pallet-ads-core` | Extrinsics: 50 (call_index 0-49)
> 全新 Pallet，完整广告生命周期 + 投放结算 + 偏好系统 + 推荐系统

### 25.1 Campaign 生命周期

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AC-001 | create_campaign(0)：创建（text, url, bid, budget, type, targets） | R16 | 正向 | P0 |
| AC-002 | fund_campaign(1)：追加预算 | R16 | 正向 | P1 |
| AC-003 | pause_campaign(2) / resume_campaign(20)：暂停/恢复 | R16 | 正向 | P1 |
| AC-004 | cancel_campaign(3)：取消（退还剩余） | R16 | 正向 | P1 |
| AC-005 | review_campaign(4)：审核（Root） | R1 | 正向 | P1 |
| AC-006 | update_campaign(22)：更新参数 | R16 | 正向 | P1 |
| AC-007 | extend_campaign_expiry(23)：延期 | R16 | 正向 | P1 |
| AC-008 | expire_campaign(21)：标记过期 | R20 | 正向 | P2 |
| AC-009 | force_cancel_campaign(24)：Root 强制取消 | R1 | 正向 | P1 |
| AC-010 | suspend_campaign(28) / unsuspend_campaign(29)：Root 暂停/恢复 | R1 | 正向 | P1 |
| AC-011 | resubmit_campaign(31)：被拒后重提 | R16 | 正向 | P1 |
| AC-012 | set_campaign_targets(36) / clear_campaign_targets(37)：设置/清除目标 | R16 | 正向 | P1 |
| AC-013 | set_campaign_multiplier(38)：广告主倍率 | R16 | 正向 | P2 |
| AC-014 | cleanup_campaign(34)：清理已终止 | R20 | 正向 | P2 |

### 25.2 投放与结算

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AC-015 | submit_delivery_receipt(5)：CPM 投放收据 | R14/R15 | 正向 | P0 |
| AC-016 | submit_click_receipt(49)：CPC 点击收据 | R14/R15 | 正向 | P0 |
| AC-017 | settle_era_ads(6) / force_settle_era_ads(35)：结算 | R20/R1 | 正向 | P0 |
| AC-018 | claim_ad_revenue(8)：提取广告收入 | R15 | 正向 | P1 |
| AC-019 | confirm_receipt(43) / dispute_receipt(44) / auto_confirm(45)：确认/争议/自动确认 | R16/R20 | 正向 | P1 |
| AC-020 | audience_size 超 cap 被裁切 | 系统 | 功能 | P0 |
| AC-021 | 每日预算耗尽拒绝投放 | 系统 | 功能 | P1 |

### 25.3 偏好系统

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AC-022 | advertiser_blacklist/whitelist_placement(9-12) | R16 | 正向 | P1 |
| AC-023 | placement_blacklist/whitelist_advertiser(13-16) | R15 | 正向 | P1 |
| AC-024 | set_placement_delivery_types(32)：Placement 投放类型 | R15 | 正向 | P1 |
| AC-025 | set_placement_multiplier(39)：Placement 倍率 | R15 | 正向 | P2 |
| AC-026 | set_placement_approval_required(40)：Placement 审批要求 | R15 | 正向 | P1 |
| AC-027 | approve/reject_campaign_for_placement(41,42) | R15 | 正向 | P1 |
| AC-028 | 拉黑后投放被拒 | 系统 | 功能 | P1 |

### 25.4 Slash 与举报

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AC-029 | flag_campaign(7) / flag_placement(17)：举报 | R20 | 正向 | P2 |
| AC-030 | report_approved_campaign(30)：举报已审核广告 | R20 | 正向 | P2 |
| AC-031 | slash_placement(18)：Slash（3 次→ban） | R1 | 正向 | P1 |
| AC-032 | unban_placement(25) / reset_slash_count(26) / clear_flags(27) | R1 | 正向 | P1 |

### 25.5 推荐系统

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AC-033 | register_advertiser(46)：广告主注册（含推荐人） | R16 | 正向 | P1 |
| AC-034 | force_register_advertiser(47)：Root 注册 | R1 | 正向 | P1 |
| AC-035 | claim_referral_earnings(48)：领取推荐收益 | R16 | 正向 | P1 |
| AC-036 | 自推荐 / 推荐人非广告主被拒绝 | R16 | 负向 | P1 |

### 25.6 Private Ads

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AC-037 | register_private_ad(19) / unregister_private_ad(33) | R15 | 正向 | P2 |

## 26. Ads Entity — Entity 广告适配器（新增模块）

> Pallet: `pallet-ads-entity` | Extrinsics: 9 (call_index 0-8)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AE-001 | register_entity_placement(0) / register_shop_placement(1)：注册 Placement | R2/R4 | 正向 | P0 |
| AE-002 | deregister_placement(2)：注销 | R2/R4 | 正向 | P1 |
| AE-003 | set_placement_active(3)：激活/停用 | R2/R4 | 正向 | P1 |
| AE-004 | set_impression_cap(4) / set_click_cap(8)：曝光/点击上限 | R2/R4 | 正向 | P1 |
| AE-005 | set_entity_ad_share(5)：Entity 广告分成 | R2 | 正向 | P1 |
| AE-006 | ban_entity(6) / unban_entity(7)：Root 封禁/解禁 | R1 | 正向 | P1 |
| AE-007 | Entity 未激活 / Shop 不存在被拒绝 | R2 | 负向 | P1 |
| AE-008 | 每日上限达到后投放被拒绝 | 系统 | 功能 | P1 |

## 27. Ads GroupRobot — GroupRobot 广告适配器（新增模块）

> Pallet: `pallet-ads-grouprobot` | Extrinsics: 20 (call_index 0-19)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AG-001 | stake_for_ads(0)：质押获取 audience_cap | R21 | 正向 | P0 |
| AG-002 | unstake_for_ads(1)：取消质押 | R21 | 正向 | P1 |
| AG-003 | set_tee_ad_pct(2) / set_community_ad_pct(3)：分成配置 | R1 | 正向 | P1 |
| AG-004 | set_community_admin(4) / resign_community_admin(12) | R15/R1 | 正向 | P1 |
| AG-005 | report_node_audience(5)：节点上报 audience | R12 | 正向 | P1 |
| AG-006 | check_audience_surge(6) / resume_audience_surge(7)：突增检测/恢复 | R1 | 正向 | P1 |
| AG-007 | cross_validate_nodes(8)：交叉验证 | R1 | 正向 | P2 |
| AG-008 | slash_community(9)：扣质押+cap 砍半 | R1 | 正向 | P0 |
| AG-009 | admin_pause_ads(10) / admin_resume_ads(11)：管理员暂停/恢复 | R15 | 正向 | P1 |
| AG-010 | withdraw_unbonded(13)：提取解绑质押 | R21 | 正向 | P1 |
| AG-011 | set_stake_tiers(14)：设置质押层级 | R1 | 正向 | P2 |
| AG-012 | force_set_community_admin(15)：Root 强制设管理员 | R1 | 正向 | P1 |
| AG-013 | set_global_ads_pause(16)：全局暂停 | R1 | 正向 | P1 |
| AG-014 | set_bot_ads_enabled(17)：Bot 广告开关 | R12 | 正向 | P1 |
| AG-015 | claim_staker_reward(18)：领取质押奖励 | R21 | 正向 | P1 |
| AG-016 | force_unstake(19)：Root 强制取消质押 | R1 | 正向 | P1 |
| AG-017 | 连续 3 次 Slash → 永久封禁 | R1 | 功能 | P1 |
| AG-018 | 突增超阈值→自动暂停 2 个 Era | 系统 | 功能 | P1 |
| AG-019 | 全局暂停时所有操作被拒绝 | R21 | 安全 | P0 |

---

## 28. 跨模块集成测试

### 28.1 完整商业流程

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-001 | Entity → Shop → Product → 上架 → 下单 → 发货 → 确认 → 评价 | Registry+Shop+Product+Order+Review | P0 |
| INT-002 | 下单 → 托管锁资金 → 确认 → 释放 → 佣金 → 会员升级 | Order+Escrow+Commission+Member | P0 |
| INT-003 | 会员推荐 → 下单 → 三级 Referral → 提现 | Member+Order+Commission | P0 |
| INT-004 | Entity Token → 治理 → 提案 → 投票 → 执行 | Token+Governance | P1 |
| INT-005 | TokenSale → 认购 → 领取 → 二级市场交易 | TokenSale+Token+EntityMarket | P1 |
| INT-006 | KYC → 满足要求 → Token 转账（KycRequired） | KYC+Token | P1 |
| INT-007 | 争议 → 证据 → 投诉 → 仲裁 → 押金 → 托管 | Order+Evidence+Arbitration+Escrow | P0 |
| INT-008 | Entity 推荐人 → 下单平台费分成 | Registry+Order+Commission | P1 |
| INT-009 | Token 订单 → Token 佣金 → Token 提现 | Order+Commission(Token) | P1 |
| INT-010 | 店铺积分：发行→转移→兑换→过期→销毁 | Shop | P1 |

### 28.2 GroupRobot 完整流程

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-011 | Bot → 订阅 → DCAP → Peer → 心跳 → Ceremony | Registry+Subscription+Ceremony | P0 |
| INT-012 | 节点 → 质押 → TEE → 消息 → 领奖 | Consensus+Registry+Rewards | P0 |
| INT-013 | 订阅 → 社区 → 日志 → 声誉 | Subscription+Community | P1 |
| INT-014 | Tier 门控: Free tier 全面限制 | Subscription+Registry+Community+Consensus+Ceremony | P0 |
| INT-015 | inactive Bot 门控: 全面限制 | Registry+Community+Ceremony | P0 |

### 28.3 广告完整流程（新增）

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-016 | Entity Placement → Campaign → 投放 → 结算 → 收入 | AdsCore+AdsEntity | P0 |
| INT-017 | GR Placement → 质押 → Campaign → 投放 → TEE 验证 → 结算 | AdsCore+AdsGR+GRSubscription | P0 |
| INT-018 | 广告推荐系统：注册→推荐→结算→领取 | AdsCore | P1 |
| INT-019 | 双向偏好：黑名单+白名单 + Placement 审批 → 投放限制 | AdsCore | P1 |
| INT-020 | Slash → 3 次封禁 → unban → 恢复 | AdsCore+AdsGR | P1 |

### 28.4 存储完整流程

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-021 | 充值 → Pin → mark_pinned → 扣费 → 续期 → upgrade_tier | Storage-Service | P0 |
| INT-022 | 运营者加入 → 自证 → 分配 → 领取 → 退出 | Storage-Service | P1 |
| INT-023 | 余额不足 → 宽限 → 到期 → 清理 | Storage-Service | P1 |
| INT-024 | 域名注册 → 域名 Pin → 优先级 → 统计 | Storage-Service | P1 |
| INT-025 | 数据归档 Active→L1→L2→Purge 完整链路 | Storage-Lifecycle | P1 |
| INT-026 | 归档保护 + 恢复 | Storage-Lifecycle | P1 |

### 28.5 审计回归集成

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-027 | cancel_commission 先转账后清记录 | Commission+Order | P0 |
| INT-028 | expire_kyc → revoke_kyc 完整流程 | KYC | P1 |
| INT-029 | update_spent USDT 追踪 + 过期等级修正 | Member+Commission | P0 |
| INT-030 | claim_pool_reward KYC 参与检查 | PoolReward+KYC | P0 |
| INT-031 | restore_stock 对 OffShelf 商品正确恢复 | Product+Order | P1 |
| INT-032 | Token 托管 lock→release/refund | Escrow+Order | P1 |

---

## 29. 端到端流程测试（多用户）

| # | 场景 | 参与角色 | 优先级 |
|---|------|----------|--------|
| E2E-001 | **商家入驻全流程**: Entity→Shop→Product→Order→确认→评价→佣金 | R1,R2,R4,R5,R6,R7 | P0 |
| E2E-002 | **会员推荐裂变**: A→B→C→下单→三级 Referral→提现 | R5,R7 | P0 |
| E2E-003 | **争议解决全流程**: 下单→退款→拒绝→证据→投诉→调解→仲裁→押金 | R5,R6,R11 | P0 |
| E2E-004 | **代币发售到治理**: Token→发售→认购→领取→提案→投票→执行 | R2,R5,R8 | P1 |
| E2E-005 | **Bot 运营全链路**: Bot→订阅→TEE→Peer→节点→社区→日志→奖励 | R12,R13,R17 | P0 |
| E2E-006 | **广告投放与反作弊**: Campaign→质押→投放→结算→Slash | R12,R15,R16 | P1 |
| E2E-007 | **P2P 交易全流程**: 挂单→吃单→付款→OCW→多档→补付→结算→TWAP | R9,R19 | P0 |
| E2E-008 | **存储全流程**: 充值→Pin→确认→扣费→续期→升级→领取→退出 | R14,R20 | P1 |
| E2E-009 | **密钥仪式**: 记录→force_re_ceremony→新仪式→superseded→cleanup | R1,R12 | P1 |
| E2E-010 | **Token 二级市场**: Token→Sale→Market 5 种单→成交 | R2,R8 | P1 |
| E2E-011 | **KYC 全生命周期**: submit→approve→expire→revoke→resubmit | R10,R20 | P1 |
| E2E-012 | **Entity 广告全链路**: 注册 Placement→Campaign→投放→确认→结算→收入 | R2,R16 | P1 |
| E2E-013 | **GR 广告全链路**: 质押→Campaign→Bot 投放→Era 结算→收入→Slash | R12,R15,R16,R21 | P1 |
| E2E-014 | **店铺积分全流程**: 启用→发行→转移→设 TTL→过期→兑换 | R2,R4,R20 | P2 |
| E2E-015 | **数据归档全流程**: Active→L1→保护→L2→Purge→恢复 | R1 | P2 |
| E2E-016 | **NEX 交易争议**: 挂单→吃单→付款→dispute_trade→resolve_dispute | R9,R1 | P1 |

---

## 30. 性能与边界测试

| # | 测试用例 | 优先级 |
|---|---------|--------|
| PF-001 | Entity MaxShopsPerEntity 上限 | P2 |
| PF-002 | Shop MaxProductsPerShop 上限 | P2 |
| PF-003 | Admin 最大数(10) | P2 |
| PF-004 | MaxActiveProposals(10) 并发 | P2 |
| PF-005 | MaxCustomLevels(10) | P2 |
| PF-006 | TokenSale 大量认购 | P2 |
| PF-007 | Disclosure MaxInsiders(50) | P2 |
| PF-008 | NexMarket 大量并发挂单 | P1 |
| PF-009 | Community 大量日志批量提交 | P2 |
| PF-010 | Storage charge_due 大量到期 | P2 |
| PF-011 | EntityMarket OrderBook 深度 | P2 |
| PF-012 | Evidence 批量归档 | P2 |
| PF-013 | Arbitration 大量过期投诉 | P2 |
| PF-014 | **Ads 大量 Campaign 同时活跃** | P2 |
| PF-015 | **batch_unpin 边界：20 CID 上限** | P2 |
| PF-016 | **ActiveOperatorIndex 256 运营者上限** | P2 |
| PF-017 | **OwnerPinIndex 1000 Pin 上限** | P2 |

---

## 31. 安全性测试

| # | 测试用例 | 优先级 |
|---|---------|--------|
| SEC-001 | 所有 extrinsic 权限校验（非授权拒绝） | P0 |
| SEC-002 | 资金操作：不可取出超过可用余额 | P0 |
| SEC-003 | 双花防护：托管 AlreadyClosed | P0 |
| SEC-004 | 重放保护：nonce/序列号/tx_hash | P0 |
| SEC-005 | 溢出保护：saturating/checked 算术 | P0 |
| SEC-006 | TEE 证明伪造检测 | P0 |
| SEC-007 | TWAP 价格操纵保护+熔断 | P0 |
| SEC-008 | KYC 过期后权限降级 | P1 |
| SEC-009 | 佣金保护：Entity 资金 ≥ 承诺 | P0 |
| SEC-010 | 广告反作弊：audience_cap + 交叉验证 | P1 |
| SEC-011 | Free tier 门控 | P0 |
| SEC-012 | 等级过期佣金回退 | P1 |
| SEC-013 | 购物余额不可提取为 NEX | P0 |
| SEC-014 | Escrow 争议状态禁止释放 | P0 |
| SEC-015 | Entity 封禁后关联 Shop/Product 不可交易 | P1 |
| SEC-016 | validate_unsigned 拒绝 External | P0 |
| SEC-017 | SingleLine 佣金使用 order_amount | P0 |
| SEC-018 | inactive Bot 门控（6 个操作） | P0 |
| SEC-019 | KycParticipationGuard 阻止 claim | P0 |
| SEC-020 | slash_equivocation 防重复 | P0 |
| SEC-021 | **Token 托管 token_lock/release/refund 权限** | P0 |
| SEC-022 | **全局暂停开关覆盖：佣金/代币/市场/广告/仲裁/计费** | P0 |
| SEC-023 | **域惩罚率不超合法范围** | P1 |
| SEC-024 | **广告结算只处理 Confirmed/AutoConfirmed 收据** | P0 |
| SEC-025 | **Placement 审批模式下未审核 Campaign 无法投放** | P1 |

---

## 统计

| 模块 | 测试用例数 |
|------|:---------:|
| Entity Registry | 36 |
| Entity Shop | 34 |
| Entity Product | 13 |
| Entity Order | 21 |
| Entity Review | 7 |
| Entity Token | 21 |
| Entity Governance | 13 |
| Entity Member | 23 |
| Commission Core | 20 |
| Commission 分配 | 9 |
| Commission 插件 | 19 |
| Entity Disclosure | 11 |
| Entity KYC | 23 |
| Entity TokenSale | 21 |
| NEX Market | 30 |
| Entity Market | 8 |
| Escrow | 14 |
| Evidence | 21 |
| Arbitration | 26 |
| Storage Service | 41 |
| Storage Lifecycle | 12 |
| GR Registry | 25 |
| GR Consensus | 20 |
| GR Subscription | 21 |
| GR Community | 16 |
| GR Ceremony | 14 |
| GR Rewards | 13 |
| Ads Core | 37 |
| Ads Entity | 8 |
| Ads GroupRobot | 19 |
| 跨模块集成 | 32 |
| 端到端 | 16 |
| 性能/边界 | 17 |
| 安全性 | 25 |
| **合计** | **~926** |
